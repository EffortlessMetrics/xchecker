use anyhow::{Context, Result};
use blake3::Hasher;
use camino::Utf8PathBuf;
use std::fs;
use std::path::Path;

use crate::atomic_write::{AtomicWriteResult, write_file_atomic};
use crate::lock::{FileLock, LockError};
use crate::paths::{SandboxConfig, SandboxRoot};
use crate::types::PhaseId;

/// Manages artifact storage with atomic writes and directory structure
///
/// # Security
///
/// `ArtifactManager` uses `SandboxRoot` to validate all paths within the spec directory.
/// This ensures that:
/// - Paths cannot escape the spec root via `..` traversal
/// - Absolute paths are rejected
/// - Symlinks are rejected by default (configurable)
/// - Hardlinks are rejected by default (configurable)
///
/// All path validation happens through `SandboxRoot::join()` which provides
/// comprehensive security checks before any file I/O.
pub struct ArtifactManager {
    base_path: Utf8PathBuf,
    /// Sandboxed root directory for validating paths within the spec directory
    sandbox_root: Option<SandboxRoot>,
    _lock: Option<FileLock>,
}

/// Represents an artifact to be stored
#[derive(Debug, Clone)]
pub struct Artifact {
    pub name: String,
    pub content: String,
    pub artifact_type: ArtifactType,
    #[allow(dead_code)] // Hash field for future content verification
    pub blake3_hash: String,
}

impl Artifact {
    /// Create a new artifact with computed BLAKE3 hash
    #[must_use]
    #[allow(dead_code)] // API constructor for artifact creation
    pub fn new(name: String, content: String, artifact_type: ArtifactType) -> Self {
        let mut hasher = Hasher::new();
        hasher.update(content.as_bytes());
        let blake3_hash = hasher.finalize().to_hex().to_string();

        Self {
            name,
            content,
            artifact_type,
            blake3_hash,
        }
    }
}

/// Result of storing an artifact with atomic write metadata
#[derive(Debug, Clone)]
pub struct ArtifactStoreResult {
    pub path: Utf8PathBuf,
    pub atomic_write_result: AtomicWriteResult,
}

/// Types of artifacts that can be stored
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactType {
    /// Markdown files (.md)
    Markdown,
    /// Core YAML files (.core.yaml)
    CoreYaml,
    /// Partial files from failed phases (.partial.md)
    Partial,
    /// Context files for debugging (.txt)
    #[allow(dead_code)] // Reserved for debugging artifacts
    Context,
}

impl ArtifactType {
    /// Get the file extension for this artifact type
    #[must_use]
    pub const fn extension(&self) -> &'static str {
        match self {
            Self::Markdown => "md",
            Self::CoreYaml => "core.yaml",
            Self::Partial => "partial.md",
            Self::Context => "txt",
        }
    }
}

impl ArtifactManager {
    /// Create a new `ArtifactManager` for the given spec ID
    ///
    /// This will acquire an exclusive lock for the spec directory to prevent
    /// concurrent execution. The lock is held for the lifetime of the `ArtifactManager`.
    #[allow(dead_code)] // API constructor for artifact manager
    pub fn new(spec_id: &str) -> Result<Self> {
        Self::new_with_force(spec_id, false)
    }

    /// Create a new `ArtifactManager` with optional force flag for lock override
    pub fn new_with_force(spec_id: &str, force: bool) -> Result<Self> {
        // Ensure spec directory tree exists before acquiring lock
        let base_path = crate::paths::spec_root(spec_id);
        Self::ensure_spec_dirs(&base_path)?;

        // Acquire exclusive lock first
        let lock = FileLock::acquire(spec_id, force, None)
            .map_err(|e| match e {
                LockError::ConcurrentExecution { spec_id, pid, created_ago } => {
                    anyhow::anyhow!(
                        "Another xchecker process is already running for spec '{spec_id}' (PID {pid}, started {created_ago}). \
                        Wait for it to complete or use --force if the process is stuck."
                    )
                }
                LockError::StaleLock { spec_id, pid, age_secs } => {
                    anyhow::anyhow!(
                        "Stale lock detected for spec '{spec_id}' (PID {pid}, age {age_secs}s). \
                        Use --force to override if you're sure the process is no longer running."
                    )
                }
                other => anyhow::anyhow!("Failed to acquire lock: {other}"),
            })?;

        // Create sandbox root for path validation
        // Use permissive config since we're operating within our own state directory
        let sandbox_root = SandboxRoot::new(base_path.as_std_path(), SandboxConfig::permissive())
            .map_err(|e| {
            anyhow::anyhow!("Failed to create sandbox root for spec directory: {e}")
        })?;

        let manager = Self {
            base_path,
            sandbox_root: Some(sandbox_root),
            _lock: Some(lock),
        };
        manager.ensure_directory_structure()?;

        Ok(manager)
    }

    /// Ensure spec directory tree exists (called before lock acquisition)
    fn ensure_spec_dirs(base_path: &Utf8PathBuf) -> Result<()> {
        // Create base path first (ignore benign races)
        crate::paths::ensure_dir_all(base_path)
            .with_context(|| format!("Failed to create base directory: {base_path}"))?;

        crate::paths::ensure_dir_all(base_path.join("artifacts"))
            .with_context(|| format!("Failed to create artifacts directory: {base_path}"))?;
        crate::paths::ensure_dir_all(base_path.join("receipts"))
            .with_context(|| format!("Failed to create receipts directory: {base_path}"))?;
        crate::paths::ensure_dir_all(base_path.join("context"))
            .with_context(|| format!("Failed to create context directory: {base_path}"))?;
        Ok(())
    }

    /// Create a read-only `ArtifactManager` that doesn't acquire locks
    /// This is used for status and inspection operations that don't modify state
    pub fn new_readonly(spec_id: &str) -> Result<Self> {
        let base_path = crate::paths::spec_root(spec_id);

        // Create sandbox root if the directory exists, otherwise None
        // Read-only managers may be created for non-existent specs (e.g., status check)
        let sandbox_root = if base_path.exists() {
            Some(
                SandboxRoot::new(base_path.as_std_path(), SandboxConfig::permissive()).map_err(
                    |e| anyhow::anyhow!("Failed to create sandbox root for spec directory: {e}"),
                )?,
            )
        } else {
            None
        };

        let manager = Self {
            base_path,
            sandbox_root,
            _lock: None, // No lock for read-only access
        };

        Ok(manager)
    }

    /// Create the required directory structure: artifacts/, receipts/, context/, .partial/
    fn ensure_directory_structure(&self) -> Result<()> {
        let directories = ["artifacts", "receipts", "context", ".partial"];

        for dir in &directories {
            let dir_path = self.base_path.join(dir);
            crate::paths::ensure_dir_all(&dir_path)
                .with_context(|| format!("Failed to create directory: {dir_path}"))?;
        }

        Ok(())
    }

    /// Validate and resolve a relative path within the sandbox.
    ///
    /// This method uses `SandboxRoot::join()` to validate the path, ensuring:
    /// - The path is relative (not absolute)
    /// - The path doesn't contain `..` traversal components
    /// - The path doesn't escape the spec root
    /// - Symlinks/hardlinks are handled according to config
    ///
    /// # Arguments
    ///
    /// * `rel_path` - A relative path within the spec directory
    ///
    /// # Returns
    ///
    /// The validated full path as a `std::path::PathBuf`.
    ///
    /// # Errors
    ///
    /// Returns an error if the path validation fails (e.g., path escape attempt).
    fn validate_path(&self, rel_path: &str) -> Result<std::path::PathBuf> {
        if let Some(ref sandbox) = self.sandbox_root {
            let sandbox_path = sandbox
                .join(rel_path)
                .map_err(|e| anyhow::anyhow!("Path validation failed for '{}': {}", rel_path, e))?;
            Ok(sandbox_path.to_path_buf())
        } else {
            // Fallback for read-only managers without sandbox (non-existent spec)
            // Still do basic validation
            if rel_path.contains("..") {
                anyhow::bail!("Path contains parent directory traversal: {}", rel_path);
            }
            if Path::new(rel_path).is_absolute() {
                anyhow::bail!("Absolute path not allowed: {}", rel_path);
            }
            Ok(self.base_path.join(rel_path).into_std_path_buf())
        }
    }

    /// Remove stale .partial/ directory (FR-ORC-003, FR-ORC-007)
    /// This is called at the start of phase execution to clean up any leftover
    /// partial artifacts from previous failed runs.
    pub fn remove_stale_partial_dir(&self) -> Result<()> {
        // Validate the .partial path through sandbox
        let partial_dir = self.validate_path(".partial")?;

        if partial_dir.exists() {
            // Best-effort removal - don't fail if we can't remove it
            if let Err(e) = fs::remove_dir_all(&partial_dir) {
                eprintln!("Warning: Failed to remove stale .partial/ directory: {e}");
                // Don't propagate the error - this is best-effort cleanup
            }
        }

        Ok(())
    }

    /// Store an artifact to the .partial/ staging directory
    /// This is used during phase execution before promoting to final location
    pub fn store_partial_staged_artifact(
        &self,
        artifact: &Artifact,
    ) -> Result<ArtifactStoreResult> {
        // Validate the .partial directory path
        let partial_dir = self.validate_path(".partial")?;
        crate::paths::ensure_dir_all(&partial_dir).with_context(|| {
            format!(
                "Failed to create .partial directory: {}",
                partial_dir.display()
            )
        })?;

        // Validate the full artifact path within .partial
        let rel_path = format!(".partial/{}", artifact.name);
        let file_path = self.validate_path(&rel_path)?;
        let file_path_utf8 = Utf8PathBuf::from_path_buf(file_path)
            .map_err(|p| anyhow::anyhow!("Invalid UTF-8 path: {}", p.display()))?;

        let atomic_result = self.write_file_atomic(&file_path_utf8, &artifact.content)?;
        Ok(ArtifactStoreResult {
            path: file_path_utf8,
            atomic_write_result: atomic_result,
        })
    }

    /// Promote a partial staged artifact to its final location (FR-ORC-004)
    /// This atomically moves the artifact from .partial/ to artifacts/
    pub fn promote_staged_to_final(&self, artifact_name: &str) -> Result<Utf8PathBuf> {
        // Validate both source and destination paths through sandbox
        let partial_rel = format!(".partial/{}", artifact_name);
        let final_rel = format!("artifacts/{}", artifact_name);

        let partial_path = self.validate_path(&partial_rel)?;
        let final_path = self.validate_path(&final_rel)?;

        if !partial_path.exists() {
            anyhow::bail!(
                "Partial artifact does not exist: {}",
                partial_path.display()
            );
        }

        // Ensure parent directory exists
        if let Some(parent) = final_path.parent() {
            crate::paths::ensure_dir_all(parent).with_context(|| {
                format!("Failed to create parent directory: {}", parent.display())
            })?;
        }

        // Atomic rename from .partial/ to artifacts/
        fs::rename(&partial_path, &final_path).with_context(|| {
            format!("Failed to promote artifact from .partial/ to final: {artifact_name}")
        })?;

        Utf8PathBuf::from_path_buf(final_path)
            .map_err(|p| anyhow::anyhow!("Invalid UTF-8 path: {}", p.display()))
    }

    /// Store an artifact using atomic write operations
    pub fn store_artifact(&self, artifact: &Artifact) -> Result<ArtifactStoreResult> {
        let file_path = self.get_artifact_path_validated(&artifact.name, artifact.artifact_type)?;
        let atomic_result = self.write_file_atomic(&file_path, &artifact.content)?;
        Ok(ArtifactStoreResult {
            path: file_path,
            atomic_write_result: atomic_result,
        })
    }

    /// Store a phase artifact with automatic naming
    #[allow(dead_code)] // Test harness/utility method
    pub fn store_phase_artifact(
        &self,
        phase: PhaseId,
        content: &str,
        artifact_type: ArtifactType,
    ) -> Result<Utf8PathBuf> {
        let normalized_content = self.normalize_line_endings(content);
        let name = self.get_phase_filename(phase, artifact_type);

        let artifact = Artifact::new(name, normalized_content, artifact_type);

        let result = self.store_artifact(&artifact)?;
        Ok(result.path)
    }

    /// Store a partial artifact from a failed phase
    #[allow(dead_code)] // Test harness/utility method
    pub fn store_partial_artifact(&self, phase: PhaseId, content: &str) -> Result<Utf8PathBuf> {
        self.store_phase_artifact(phase, content, ArtifactType::Partial)
    }

    /// Store a context file for debugging
    pub fn store_context_file(&self, name: &str, content: &str) -> Result<Utf8PathBuf> {
        // Validate the context file path through sandbox
        let rel_path = format!("context/{}.txt", name);
        let file_path = self.validate_path(&rel_path)?;
        let file_path_utf8 = Utf8PathBuf::from_path_buf(file_path)
            .map_err(|p| anyhow::anyhow!("Invalid UTF-8 path: {}", p.display()))?;

        let normalized_content = self.normalize_line_endings(content);
        let _atomic_result = self.write_file_atomic(&file_path_utf8, &normalized_content)?;
        Ok(file_path_utf8)
    }

    /// Write content to a file using atomic operations (tempfile → fsync → rename)
    /// Returns the atomic write result with retry/fallback information
    fn write_file_atomic(&self, path: &Utf8PathBuf, content: &str) -> Result<AtomicWriteResult> {
        write_file_atomic(path, content)
            .with_context(|| format!("Failed to atomically write file: {path}"))
    }

    /// Normalize line endings to \n for all content
    fn normalize_line_endings(&self, content: &str) -> String {
        content.replace("\r\n", "\n").replace('\r', "\n")
    }

    /// Get the full path for an artifact with sandbox validation
    ///
    /// # Security
    ///
    /// This method validates the artifact path through the sandbox to ensure:
    /// - The artifact name is a relative path (not absolute)
    /// - The artifact name has no `..` traversal components
    /// - The final path doesn't escape the spec root
    fn get_artifact_path_validated(
        &self,
        name: &str,
        artifact_type: ArtifactType,
    ) -> Result<Utf8PathBuf> {
        // Validate artifact name BEFORE prefixing with "artifacts/" or "context/"
        // This prevents absolute paths from being "contained" after prefixing
        // (e.g., "/tmp/evil.md" becoming "artifacts//tmp/evil.md" -> "artifacts/tmp/evil.md")
        let name_path = Path::new(name);
        if name_path.is_absolute() {
            anyhow::bail!(
                "Artifact name must be a relative path, got absolute path: {}",
                name
            );
        }
        if name_path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            anyhow::bail!(
                "Artifact name contains parent directory traversal: {}",
                name
            );
        }

        let rel_path = match artifact_type {
            ArtifactType::Context => format!("context/{}", name),
            _ => format!("artifacts/{}", name),
        };

        let validated_path = self.validate_path(&rel_path)?;
        Utf8PathBuf::from_path_buf(validated_path)
            .map_err(|p| anyhow::anyhow!("Invalid UTF-8 path: {}", p.display()))
    }

    /// Generate filename for a phase artifact
    fn get_phase_filename(&self, phase: PhaseId, artifact_type: ArtifactType) -> String {
        let phase_number = self.get_phase_number(phase);
        let phase_name = phase.as_str();
        let extension = artifact_type.extension();

        format!("{phase_number:02}-{phase_name}.{extension}")
    }

    /// Get the numeric prefix for a phase
    const fn get_phase_number(&self, phase: PhaseId) -> u8 {
        match phase {
            PhaseId::Requirements => 0,
            PhaseId::Design => 10,
            PhaseId::Tasks => 20,
            PhaseId::Review => 30,
            PhaseId::Fixup => 40,
            PhaseId::Final => 50,
        }
    }

    /// Get the base path for this spec
    #[must_use]
    pub const fn base_path(&self) -> &Utf8PathBuf {
        &self.base_path
    }

    /// Get the artifacts directory path
    #[must_use]
    pub fn artifacts_path(&self) -> Utf8PathBuf {
        self.base_path.join("artifacts")
    }

    /// Get the receipts directory path
    #[must_use]
    #[allow(dead_code)] // Test harness/utility method
    pub fn receipts_path(&self) -> Utf8PathBuf {
        self.base_path.join("receipts")
    }

    /// Get the context directory path
    #[must_use]
    pub fn context_path(&self) -> Utf8PathBuf {
        self.base_path.join("context")
    }

    /// Check if an artifact exists.
    ///
    /// Returns `false` if:
    /// - the artifact does not exist, or
    /// - path validation fails (e.g., traversal attempt or invalid name)
    #[must_use]
    pub fn artifact_exists(&self, name: &str, artifact_type: ArtifactType) -> bool {
        match self.get_artifact_path_validated(name, artifact_type) {
            Ok(path) => path.exists(),
            Err(_) => false,
        }
    }

    /// Read an existing artifact
    #[allow(dead_code)] // Test harness/utility method
    pub fn read_artifact(&self, name: &str, artifact_type: ArtifactType) -> Result<String> {
        let path = self.get_artifact_path_validated(name, artifact_type)?;
        fs::read_to_string(path.as_std_path())
            .with_context(|| format!("Failed to read artifact: {path}"))
    }

    /// Check if a partial artifact exists for a phase
    #[must_use]
    pub fn has_partial_artifact(&self, phase: PhaseId) -> bool {
        let partial_name = self.get_phase_filename(phase, ArtifactType::Partial);
        self.artifact_exists(&partial_name, ArtifactType::Partial)
    }

    /// Read a partial artifact for a phase
    #[allow(dead_code)] // Test harness/utility method
    pub fn read_partial_artifact(&self, phase: PhaseId) -> Result<String> {
        let partial_name = self.get_phase_filename(phase, ArtifactType::Partial);
        self.read_artifact(&partial_name, ArtifactType::Partial)
    }

    /// Delete a partial artifact for a phase
    pub fn delete_partial_artifact(&self, phase: PhaseId) -> Result<()> {
        let partial_name = self.get_phase_filename(phase, ArtifactType::Partial);
        let partial_path =
            self.get_artifact_path_validated(&partial_name, ArtifactType::Partial)?;

        if partial_path.exists() {
            fs::remove_file(partial_path.as_std_path())
                .with_context(|| format!("Failed to delete partial artifact: {partial_path}"))?;
        }

        Ok(())
    }

    /// Promote a partial artifact to final artifact (used on successful resume)
    #[allow(dead_code)] // Test harness/utility method
    pub fn promote_partial_to_final(
        &self,
        phase: PhaseId,
        artifact_type: ArtifactType,
    ) -> Result<Utf8PathBuf> {
        let partial_name = self.get_phase_filename(phase, ArtifactType::Partial);
        let final_name = self.get_phase_filename(phase, artifact_type);

        // Validate both paths through sandbox
        let partial_path =
            self.get_artifact_path_validated(&partial_name, ArtifactType::Partial)?;
        let final_path = self.get_artifact_path_validated(&final_name, artifact_type)?;

        if !partial_path.exists() {
            return Err(anyhow::anyhow!(
                "Partial artifact does not exist: {partial_path}"
            ));
        }

        // Read partial content
        let content = fs::read_to_string(partial_path.as_std_path())
            .with_context(|| format!("Failed to read partial artifact: {partial_path}"))?;

        // Write to final location atomically
        let _atomic_result = self.write_file_atomic(&final_path, &content)?;

        // Delete the partial
        fs::remove_file(partial_path.as_std_path()).with_context(|| {
            format!("Failed to delete partial artifact after promotion: {partial_path}")
        })?;

        Ok(final_path)
    }

    /// Check if a phase has completed successfully (has final artifacts)
    #[must_use]
    pub fn phase_completed(&self, phase: PhaseId) -> bool {
        let md_name = self.get_phase_filename(phase, ArtifactType::Markdown);
        let yaml_name = self.get_phase_filename(phase, ArtifactType::CoreYaml);

        self.artifact_exists(&md_name, ArtifactType::Markdown)
            && self.artifact_exists(&yaml_name, ArtifactType::CoreYaml)
    }

    /// Get the latest completed phase by checking for artifacts
    #[must_use]
    pub fn get_latest_completed_phase(&self) -> Option<PhaseId> {
        let phases = [
            PhaseId::Final,
            PhaseId::Fixup,
            PhaseId::Review,
            PhaseId::Tasks,
            PhaseId::Design,
            PhaseId::Requirements,
        ];

        phases
            .into_iter()
            .find(|&phase| self.phase_completed(phase))
    }

    /// List all artifacts in the artifacts directory
    ///
    /// # Security
    ///
    /// This method validates the artifacts directory path through the sandbox
    /// before listing its contents.
    pub fn list_artifacts(&self) -> Result<Vec<String>> {
        // Validate the artifacts directory path
        let artifacts_dir = self.validate_path("artifacts")?;

        if !artifacts_dir.exists() {
            return Ok(Vec::new());
        }

        let mut artifacts = Vec::new();

        for entry in fs::read_dir(&artifacts_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file()
                && let Some(name) = entry.file_name().to_str()
            {
                artifacts.push(name.to_string());
            }
        }

        artifacts.sort();
        Ok(artifacts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_manager_with_id(spec_id: &str) -> (ArtifactManager, TempDir) {
        let temp_dir = crate::paths::with_isolated_home();

        let manager = ArtifactManager::new(spec_id).unwrap();

        (manager, temp_dir)
    }

    #[test]
    fn test_directory_structure_creation() {
        let (manager, _temp_dir) = create_test_manager_with_id("test-spec-directory");

        assert!(manager.artifacts_path().exists());
        assert!(manager.receipts_path().exists());
        assert!(manager.context_path().exists());
    }

    #[test]
    fn test_line_ending_normalization() {
        let (manager, _temp_dir) = create_test_manager_with_id("test-spec-line-ending");

        let content_with_crlf = "line1\r\nline2\r\nline3";
        let content_with_cr = "line1\rline2\rline3";
        let content_with_lf = "line1\nline2\nline3";

        assert_eq!(
            manager.normalize_line_endings(content_with_crlf),
            "line1\nline2\nline3"
        );
        assert_eq!(
            manager.normalize_line_endings(content_with_cr),
            "line1\nline2\nline3"
        );
        assert_eq!(
            manager.normalize_line_endings(content_with_lf),
            "line1\nline2\nline3"
        );
    }

    #[test]
    fn test_atomic_write() {
        let (manager, _temp_dir) = create_test_manager_with_id("test-spec-atomic");

        let content = "test content\nwith multiple lines";
        let result =
            manager.store_phase_artifact(PhaseId::Requirements, content, ArtifactType::Markdown);

        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.exists());

        let read_content = fs::read_to_string(path.as_std_path()).unwrap();
        assert_eq!(read_content, content);
    }

    #[test]
    fn test_phase_filename_generation() {
        let (manager, _temp_dir) = create_test_manager_with_id("test-spec-filename");

        assert_eq!(
            manager.get_phase_filename(PhaseId::Requirements, ArtifactType::Markdown),
            "00-requirements.md"
        );
        assert_eq!(
            manager.get_phase_filename(PhaseId::Design, ArtifactType::CoreYaml),
            "10-design.core.yaml"
        );
        assert_eq!(
            manager.get_phase_filename(PhaseId::Tasks, ArtifactType::Partial),
            "20-tasks.partial.md"
        );
    }

    #[test]
    fn test_hash_computation() {
        let (_manager, _temp_dir) = create_test_manager_with_id("test-spec-hash");

        let content = "test content".to_string();
        let artifact1 = Artifact::new(
            "test.md".to_string(),
            content.clone(),
            ArtifactType::Markdown,
        );
        let artifact2 = Artifact::new(
            "test.md".to_string(),
            content.clone(),
            ArtifactType::Markdown,
        );

        // Same content should produce same hash
        assert_eq!(artifact1.blake3_hash, artifact2.blake3_hash);

        // Different content should produce different hash
        let different_content = "different content".to_string();
        let artifact3 = Artifact::new(
            "test.md".to_string(),
            different_content,
            ArtifactType::Markdown,
        );
        assert_ne!(artifact1.blake3_hash, artifact3.blake3_hash);
    }

    #[test]
    fn test_context_file_storage() {
        let (manager, _temp_dir) = create_test_manager_with_id("test-spec-context");

        let content = "debug context information";
        let result = manager.store_context_file("debug-info", content);

        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.exists());
        assert!(path.to_string().contains("context"));
        assert!(path.to_string().ends_with("debug-info.txt"));
    }

    #[test]
    fn test_partial_artifact_handling() {
        let (manager, _temp_dir) = create_test_manager_with_id("test-spec-partial");

        // Initially no partial artifacts
        assert!(!manager.has_partial_artifact(PhaseId::Requirements));

        // Store a partial artifact
        let partial_content = "Partial requirements content...";
        let result = manager.store_partial_artifact(PhaseId::Requirements, partial_content);
        assert!(result.is_ok());

        // Now should have partial artifact
        assert!(manager.has_partial_artifact(PhaseId::Requirements));

        // Should be able to read it
        let read_content = manager
            .read_partial_artifact(PhaseId::Requirements)
            .unwrap();
        assert_eq!(read_content, partial_content);

        // Should be able to delete it
        let delete_result = manager.delete_partial_artifact(PhaseId::Requirements);
        assert!(delete_result.is_ok());

        // Should no longer exist
        assert!(!manager.has_partial_artifact(PhaseId::Requirements));
    }

    #[test]
    fn test_promote_partial_to_final() {
        let (manager, _temp_dir) = create_test_manager_with_id("test-spec-promote");

        // Store a partial artifact
        let partial_content = "# Requirements\n\nPartial content that will be promoted\n";
        manager
            .store_partial_artifact(PhaseId::Requirements, partial_content)
            .unwrap();

        // Promote to final markdown artifact
        let result =
            manager.promote_partial_to_final(PhaseId::Requirements, ArtifactType::Markdown);
        assert!(result.is_ok());

        let final_path = result.unwrap();
        assert!(final_path.exists());
        assert!(final_path.to_string().contains("00-requirements.md"));

        // Partial should be deleted
        assert!(!manager.has_partial_artifact(PhaseId::Requirements));

        // Final should have the content
        let final_content = manager
            .read_artifact("00-requirements.md", ArtifactType::Markdown)
            .unwrap();
        assert_eq!(final_content, partial_content);
    }

    #[test]
    fn test_phase_completion_tracking() {
        let (manager, _temp_dir) = create_test_manager_with_id("test-spec-completion");

        // Initially no phases completed
        assert!(!manager.phase_completed(PhaseId::Requirements));
        assert_eq!(manager.get_latest_completed_phase(), None);

        // Store requirements artifacts
        manager
            .store_phase_artifact(
                PhaseId::Requirements,
                "# Requirements",
                ArtifactType::Markdown,
            )
            .unwrap();
        manager
            .store_phase_artifact(
                PhaseId::Requirements,
                "spec_id: test",
                ArtifactType::CoreYaml,
            )
            .unwrap();

        // Now requirements should be completed
        assert!(manager.phase_completed(PhaseId::Requirements));
        assert_eq!(
            manager.get_latest_completed_phase(),
            Some(PhaseId::Requirements)
        );

        // Store design artifacts
        manager
            .store_phase_artifact(PhaseId::Design, "# Design", ArtifactType::Markdown)
            .unwrap();
        manager
            .store_phase_artifact(PhaseId::Design, "spec_id: test", ArtifactType::CoreYaml)
            .unwrap();

        // Now design should be the latest
        assert!(manager.phase_completed(PhaseId::Design));
        assert_eq!(manager.get_latest_completed_phase(), Some(PhaseId::Design));
    }

    #[test]
    fn test_delete_nonexistent_partial() {
        let (manager, _temp_dir) = create_test_manager_with_id("test-spec-delete");

        // Deleting non-existent partial should not fail
        let result = manager.delete_partial_artifact(PhaseId::Requirements);
        assert!(result.is_ok());
    }

    #[test]
    fn test_promote_nonexistent_partial() {
        let (manager, _temp_dir) = create_test_manager_with_id("test-spec-promote-nonexistent");

        // Promoting non-existent partial should fail
        let result =
            manager.promote_partial_to_final(PhaseId::Requirements, ArtifactType::Markdown);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }

    #[test]
    fn test_sandbox_path_validation() {
        let (manager, _temp_dir) = create_test_manager_with_id("test-spec-sandbox");

        // Valid paths should work
        let result = manager.validate_path("artifacts/test.md");
        assert!(result.is_ok());

        let result = manager.validate_path("context/debug.txt");
        assert!(result.is_ok());

        let result = manager.validate_path(".partial/temp.md");
        assert!(result.is_ok());

        // Path traversal should be rejected
        let result = manager.validate_path("../escape.txt");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("traversal") || err_msg.contains("parent"),
            "Expected error about traversal or parent, got: {}",
            err_msg
        );

        let result = manager.validate_path("artifacts/../../escape.txt");
        assert!(result.is_err());
    }

    #[test]
    fn test_list_artifacts_uses_validated_paths() {
        let (manager, _temp_dir) = create_test_manager_with_id("test-spec-list");

        // Store some artifacts
        manager
            .store_phase_artifact(
                PhaseId::Requirements,
                "# Requirements",
                ArtifactType::Markdown,
            )
            .unwrap();
        manager
            .store_phase_artifact(
                PhaseId::Requirements,
                "spec_id: test",
                ArtifactType::CoreYaml,
            )
            .unwrap();

        // List should work and return the artifacts
        let artifacts = manager.list_artifacts().unwrap();
        assert_eq!(artifacts.len(), 2);
        assert!(artifacts.contains(&"00-requirements.md".to_string()));
        assert!(artifacts.contains(&"00-requirements.core.yaml".to_string()));
    }
}

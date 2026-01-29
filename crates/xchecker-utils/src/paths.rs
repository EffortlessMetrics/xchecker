use camino::Utf8PathBuf;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use thiserror::Error;

// Thread-local override used only in tests to avoid process-global env races.
thread_local! {
    static THREAD_HOME: RefCell<Option<Utf8PathBuf>> = const { RefCell::new(None) };
}

// ============================================================================
// Platform-Specific Hardlink Detection
// ============================================================================

/// Get the link count for a file.
///
/// Returns the number of hard links pointing to the file. A regular file
/// without hardlinks has a link count of 1. A link count > 1 indicates
/// the file has hard links.
///
/// # Fail-Closed Behavior
///
/// If the link count cannot be determined (e.g., permission denied, file
/// cannot be opened), this function returns `Err`. Callers should treat
/// errors as potential hardlinks for security (fail closed).
///
/// # Platform Behavior
///
/// - **Unix**: Uses `metadata.nlink()` via `MetadataExt`
/// - **Windows**: Uses `GetFileInformationByHandle` Win32 API to get `nNumberOfLinks`
///
/// # Arguments
///
/// * `path` - The path to check. Must be a regular file (not a directory).
///
/// # Returns
///
/// * `Ok(n)` - The file has `n` hard links
/// * `Err(e)` - Could not determine link count (treat as potential hardlink)
#[cfg(unix)]
pub fn link_count(path: &Path) -> Result<u32, std::io::Error> {
    use std::os::unix::fs::MetadataExt;
    let metadata = path.metadata()?;
    // nlink() returns u64 on Unix, but link counts > u32::MAX are unrealistic
    Ok(metadata.nlink() as u32)
}

#[cfg(windows)]
pub fn link_count(path: &Path) -> Result<u32, std::io::Error> {
    use std::fs::File;
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };

    // Open the file to get a handle
    let file = File::open(path)?;

    let handle = HANDLE(file.as_raw_handle());
    let mut file_info = BY_HANDLE_FILE_INFORMATION::default();

    // Get file information including nNumberOfLinks
    let result = unsafe { GetFileInformationByHandle(handle, &mut file_info) };

    match result {
        Ok(()) => Ok(file_info.nNumberOfLinks),
        Err(e) => Err(std::io::Error::other(format!(
            "GetFileInformationByHandle failed: {e}"
        ))),
    }
}

// ============================================================================
// Sandbox Error Types
// ============================================================================

/// Errors that can occur during path sandbox operations.
///
/// These errors indicate security violations when paths attempt to escape
/// their designated sandbox root.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum SandboxError {
    /// The sandbox root path does not exist
    #[error("Sandbox root does not exist: {path}")]
    RootNotFound { path: String },

    /// The sandbox root path is not a directory
    #[error("Sandbox root is not a directory: {path}")]
    RootNotDirectory { path: String },

    /// Failed to canonicalize the sandbox root path
    #[error("Failed to canonicalize sandbox root '{path}': {reason}")]
    RootCanonicalizationFailed { path: String, reason: String },

    /// Path contains ".." traversal components
    #[error("Path contains parent directory traversal: {path}")]
    ParentTraversal { path: String },

    /// Path is absolute and not within the sandbox root
    #[error("Absolute path not allowed: {path}")]
    AbsolutePath { path: String },

    /// Path resolves outside the sandbox root
    #[error("Path escapes sandbox root: {path} resolves outside {root}")]
    EscapeAttempt { path: String, root: String },

    /// Path is or contains a symlink (when symlinks are not allowed)
    #[error("Symlink not allowed: {path}")]
    SymlinkNotAllowed { path: String },

    /// Path is or contains a hardlink (when hardlinks are not allowed)
    #[error("Hardlink not allowed: {path}")]
    HardlinkNotAllowed { path: String },

    /// Failed to canonicalize the joined path
    #[error("Failed to canonicalize path '{path}': {reason}")]
    PathCanonicalizationFailed { path: String, reason: String },
}

// ============================================================================
// Sandbox Configuration
// ============================================================================

/// Configuration for sandbox path validation behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SandboxConfig {
    /// Whether to allow symlinks within the sandbox
    pub allow_symlinks: bool,
    /// Whether to allow hardlinks within the sandbox (files with link count > 1)
    pub allow_hardlinks: bool,
}

impl SandboxConfig {
    /// Create a permissive config that allows symlinks and hardlinks
    #[must_use]
    pub fn permissive() -> Self {
        Self {
            allow_symlinks: true,
            allow_hardlinks: true,
        }
    }
}

// ============================================================================
// SandboxRoot - Validated root directory for sandboxed operations
// ============================================================================

/// A validated root directory for sandboxed operations.
///
/// All paths derived from this root are guaranteed to stay within it.
/// `SandboxRoot` canonicalizes the root path at construction time and
/// validates all joined paths to prevent directory traversal attacks.
///
/// # Security Guarantees
///
/// - The root path is canonicalized (resolved to absolute, symlinks followed)
/// - Joined paths cannot escape the root via `..` traversal
/// - Absolute paths are rejected unless they're within the root
/// - Symlinks can be optionally rejected to prevent escape via symlink
///
/// # Example
///
/// ```rust,no_run
/// use xchecker_utils::paths::{SandboxRoot, SandboxConfig};
///
/// let root = SandboxRoot::new("/path/to/workspace", SandboxConfig::default())?;
/// let file = root.join("src/main.rs")?;
/// println!("Safe path: {}", file.as_path().display());
/// # Ok::<(), xchecker_utils::paths::SandboxError>(())
/// ```
#[derive(Debug, Clone)]
pub struct SandboxRoot {
    /// Canonicalized absolute path to the root
    root: PathBuf,
    /// Configuration for path validation
    config: SandboxConfig,
}

impl SandboxRoot {
    /// Create a new sandbox root from a path.
    ///
    /// Canonicalizes the path and verifies it exists as a directory.
    ///
    /// # Arguments
    ///
    /// * `root` - The path to use as the sandbox root
    /// * `config` - Configuration for symlink/hardlink handling
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The path does not exist
    /// - The path is not a directory
    /// - The path cannot be canonicalized
    pub fn new(root: impl AsRef<Path>, config: SandboxConfig) -> Result<Self, SandboxError> {
        let root_path = root.as_ref();

        // Check existence
        if !root_path.exists() {
            return Err(SandboxError::RootNotFound {
                path: root_path.display().to_string(),
            });
        }

        // Check it's a directory
        if !root_path.is_dir() {
            return Err(SandboxError::RootNotDirectory {
                path: root_path.display().to_string(),
            });
        }

        // Canonicalize to get absolute path with symlinks resolved
        let canonical =
            root_path
                .canonicalize()
                .map_err(|e| SandboxError::RootCanonicalizationFailed {
                    path: root_path.display().to_string(),
                    reason: e.to_string(),
                })?;

        Ok(Self {
            root: canonical,
            config,
        })
    }

    /// Create a sandbox root with default (restrictive) configuration.
    ///
    /// This is a convenience method equivalent to `SandboxRoot::new(root, SandboxConfig::default())`.
    pub fn new_default(root: impl AsRef<Path>) -> Result<Self, SandboxError> {
        Self::new(root, SandboxConfig::default())
    }

    /// Join a relative path, validating it stays within the sandbox.
    ///
    /// # Arguments
    ///
    /// * `rel` - A relative path to join to the sandbox root
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The path contains `..` traversal components
    /// - The path is absolute
    /// - The resolved path escapes the sandbox root
    /// - The path is or contains a symlink (when symlinks are not allowed)
    /// - The path is or contains a hardlink (when hardlinks are not allowed)
    pub fn join(&self, rel: impl AsRef<Path>) -> Result<SandboxPath, SandboxError> {
        let rel_path = rel.as_ref();

        // Reject absolute paths
        if rel_path.is_absolute() {
            return Err(SandboxError::AbsolutePath {
                path: rel_path.display().to_string(),
            });
        }

        // Reject paths with ".." components (before any filesystem operations)
        if rel_path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(SandboxError::ParentTraversal {
                path: rel_path.display().to_string(),
            });
        }

        // Build the full path
        let full_path = self.root.join(rel_path);

        // Check symlink status before canonicalization if symlinks are not allowed
        if !self.config.allow_symlinks {
            self.check_symlinks_in_path(&full_path)?;
        }

        // If the path exists, canonicalize and verify it's within the root
        if full_path.exists() {
            let canonical =
                full_path
                    .canonicalize()
                    .map_err(|e| SandboxError::PathCanonicalizationFailed {
                        path: full_path.display().to_string(),
                        reason: e.to_string(),
                    })?;

            // Verify the canonical path is within the sandbox root
            if !canonical.starts_with(&self.root) {
                return Err(SandboxError::EscapeAttempt {
                    path: rel_path.display().to_string(),
                    root: self.root.display().to_string(),
                });
            }

            // Check hardlink status if hardlinks are not allowed
            if !self.config.allow_hardlinks {
                self.check_hardlink(&canonical)?;
            }

            Ok(SandboxPath {
                full: canonical,
                rel: rel_path.to_path_buf(),
            })
        } else {
            // For non-existent paths, we need to ensure that existing ancestor
            // directories don't escape the sandbox via symlinks.
            // This is critical when allow_symlinks is true - a symlink directory
            // in the path could redirect to outside the sandbox.
            if self.config.allow_symlinks {
                self.validate_ancestor_within_sandbox(&full_path, rel_path)?;
            }

            // The full path is root + rel, which is now guaranteed to be within root
            Ok(SandboxPath {
                full: full_path,
                rel: rel_path.to_path_buf(),
            })
        }
    }

    /// Check if any component in the path is a symlink.
    fn check_symlinks_in_path(&self, path: &Path) -> Result<(), SandboxError> {
        let mut current = PathBuf::new();

        for component in path.components() {
            current.push(component);

            // Only check if the path exists
            if current.exists() {
                // Check if this component is a symlink
                if current
                    .symlink_metadata()
                    .map(|m| m.is_symlink())
                    .unwrap_or(false)
                {
                    return Err(SandboxError::SymlinkNotAllowed {
                        path: current.display().to_string(),
                    });
                }
            }
        }

        Ok(())
    }

    /// Check if a file is a hardlink (has link count > 1).
    ///
    /// Uses fail-closed behavior: if link count cannot be determined,
    /// treats the file as a potential hardlink and rejects it.
    fn check_hardlink(&self, path: &Path) -> Result<(), SandboxError> {
        // Only check files, not directories
        if path.is_file() {
            match link_count(path) {
                Ok(count) if count > 1 => {
                    return Err(SandboxError::HardlinkNotAllowed {
                        path: path.display().to_string(),
                    });
                }
                Ok(_) => {
                    // Link count is 1, not a hardlink
                }
                Err(_) => {
                    // Fail closed: if we can't determine link count, assume it might be a hardlink
                    return Err(SandboxError::HardlinkNotAllowed {
                        path: path.display().to_string(),
                    });
                }
            }
        }

        Ok(())
    }

    /// Validate that the nearest existing ancestor of a non-existent path
    /// stays within the sandbox when canonicalized.
    ///
    /// This prevents symlink traversal attacks where a symlinked directory
    /// in the path points outside the sandbox, allowing creation of files
    /// outside the intended directory.
    ///
    /// # Security
    ///
    /// When `allow_symlinks` is true and a path doesn't exist, we must verify
    /// that existing parent directories don't escape via symlinks. Without this
    /// check, `root.join("symlinked_dir/new_file.txt")` could create a file
    /// outside the sandbox if `symlinked_dir` points elsewhere.
    fn validate_ancestor_within_sandbox(
        &self,
        full_path: &Path,
        rel_path: &Path,
    ) -> Result<(), SandboxError> {
        // Find the longest prefix of full_path that exists
        let mut ancestor = full_path.to_path_buf();
        while !ancestor.exists() {
            if !ancestor.pop() {
                // We've popped all the way to the root, nothing to check
                return Ok(());
            }
        }

        // Canonicalize the existing ancestor and verify containment
        let canonical_ancestor =
            ancestor
                .canonicalize()
                .map_err(|e| SandboxError::PathCanonicalizationFailed {
                    path: ancestor.display().to_string(),
                    reason: e.to_string(),
                })?;

        // Verify the canonical ancestor is within the sandbox root
        if !canonical_ancestor.starts_with(&self.root) {
            return Err(SandboxError::EscapeAttempt {
                path: rel_path.display().to_string(),
                root: self.root.display().to_string(),
            });
        }

        Ok(())
    }

    /// Get the canonicalized root path.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.root
    }

    /// Get the sandbox configuration.
    #[must_use]
    pub fn config(&self) -> &SandboxConfig {
        &self.config
    }
}

// ============================================================================
// SandboxPath - A validated path within a SandboxRoot
// ============================================================================

/// A path that has been validated to be within a `SandboxRoot`.
///
/// Cannot be constructed directly; must come from [`SandboxRoot::join()`].
/// This type guarantees that the path:
/// - Does not escape the sandbox root
/// - Does not contain `..` traversal components
/// - Is not an absolute path outside the root
/// - Does not contain symlinks (if configured)
///
/// # Example
///
/// ```rust,no_run
/// use xchecker_utils::paths::{SandboxRoot, SandboxConfig};
///
/// let root = SandboxRoot::new("/workspace", SandboxConfig::default())?;
/// let path = root.join("src/lib.rs")?;
///
/// // Use the full path for I/O operations
/// let content = std::fs::read_to_string(path.as_path())?;
///
/// // Use the relative path for display or storage
/// println!("File: {}", path.relative().display());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone)]
pub struct SandboxPath {
    /// Full path (root + relative)
    full: PathBuf,
    /// Relative path from root
    rel: PathBuf,
}

impl SandboxPath {
    /// Get the full path for I/O operations.
    ///
    /// This returns the complete path including the sandbox root,
    /// suitable for use with `std::fs` operations.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.full
    }

    /// Get the relative portion of the path.
    ///
    /// This returns the path relative to the sandbox root,
    /// suitable for display or storage in artifacts.
    #[must_use]
    pub fn relative(&self) -> &Path {
        &self.rel
    }

    /// Convert to a `PathBuf` for ownership.
    #[must_use]
    pub fn to_path_buf(&self) -> PathBuf {
        self.full.clone()
    }

    /// Convert the relative path to a `PathBuf`.
    #[must_use]
    pub fn relative_to_path_buf(&self) -> PathBuf {
        self.rel.clone()
    }
}

impl AsRef<Path> for SandboxPath {
    fn as_ref(&self) -> &Path {
        &self.full
    }
}

/// Resolve xchecker home:
/// 1) thread-local override (tests use this)
/// 2) env `XCHECKER_HOME` (opt-in for users/CI)
/// 3) default ".xchecker"
#[must_use]
pub fn xchecker_home() -> Utf8PathBuf {
    if let Some(tl) = THREAD_HOME.with(|tl| tl.borrow().clone()) {
        return tl;
    }
    if let Ok(p) = std::env::var("XCHECKER_HOME") {
        return Utf8PathBuf::from(p);
    }
    Utf8PathBuf::from(".xchecker")
}

/// Returns `<XCHECKER_HOME>/specs/<spec_id>`
#[must_use]
pub fn spec_root(spec_id: &str) -> Utf8PathBuf {
    xchecker_home().join("specs").join(spec_id)
}

/// Returns `<XCHECKER_HOME>/cache`
#[must_use]
pub fn cache_dir() -> Utf8PathBuf {
    xchecker_home().join("cache")
}

/// mkdir -p; treat `AlreadyExists` as success (removes TOCTTOU races)
pub fn ensure_dir_all<P: AsRef<std::path::Path>>(p: P) -> std::io::Result<()> {
    match std::fs::create_dir_all(&p) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(e),
    }
}

/// Test helper: provides isolated workspace testing; not part of public API stability guarantees.
///
/// Give this test a unique home under the system temp dir.
/// Hold the `TempDir` for the test's duration so the directory stays alive.
#[cfg(any(test, feature = "test-utils"))]
#[cfg_attr(not(test), allow(dead_code))]
#[must_use]
pub fn with_isolated_home() -> tempfile::TempDir {
    let td = tempfile::TempDir::new().expect("create temp home");
    let p = Utf8PathBuf::from_path_buf(td.path().to_path_buf()).unwrap();
    THREAD_HOME.with(|tl| *tl.borrow_mut() = Some(p.clone()));
    #[cfg(feature = "test-utils")]
    {
        xchecker_lock::set_thread_home_for_tests(p);
    }
    td
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_dir() -> TempDir {
        TempDir::new().expect("Failed to create temp dir")
    }

    // ========================================================================
    // SandboxRoot::new() tests
    // ========================================================================

    #[test]
    fn test_sandbox_root_new_valid_directory() {
        let temp = create_test_dir();
        let root = SandboxRoot::new(temp.path(), SandboxConfig::default());
        assert!(root.is_ok());
        let root = root.unwrap();
        assert!(root.as_path().is_absolute());
    }

    #[test]
    fn test_sandbox_root_new_nonexistent_path() {
        let result = SandboxRoot::new(
            "/nonexistent/path/that/does/not/exist",
            SandboxConfig::default(),
        );
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SandboxError::RootNotFound { .. }
        ));
    }

    #[test]
    fn test_sandbox_root_new_file_not_directory() {
        let temp = create_test_dir();
        let file_path = temp.path().join("file.txt");
        std::fs::write(&file_path, "content").unwrap();

        let result = SandboxRoot::new(&file_path, SandboxConfig::default());
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SandboxError::RootNotDirectory { .. }
        ));
    }

    #[test]
    fn test_sandbox_root_new_default() {
        let temp = create_test_dir();
        let root = SandboxRoot::new_default(temp.path());
        assert!(root.is_ok());
    }

    // ========================================================================
    // SandboxRoot::join() - basic functionality
    // ========================================================================

    #[test]
    fn test_sandbox_join_simple_relative_path() {
        let temp = create_test_dir();
        let subdir = temp.path().join("subdir");
        std::fs::create_dir(&subdir).unwrap();
        let file = subdir.join("file.txt");
        std::fs::write(&file, "content").unwrap();

        let root = SandboxRoot::new_default(temp.path()).unwrap();
        let result = root.join("subdir/file.txt");
        assert!(result.is_ok());
        let sandbox_path = result.unwrap();
        assert_eq!(sandbox_path.relative(), Path::new("subdir/file.txt"));
    }

    #[test]
    fn test_sandbox_join_nonexistent_path_allowed() {
        let temp = create_test_dir();
        let root = SandboxRoot::new_default(temp.path()).unwrap();

        // Non-existent paths are allowed (for creating new files)
        let result = root.join("new/path/to/file.txt");
        assert!(result.is_ok());
    }

    // ========================================================================
    // SandboxRoot::join() - rejection of ".." traversal
    // ========================================================================

    #[test]
    fn test_sandbox_join_rejects_parent_traversal() {
        let temp = create_test_dir();
        let root = SandboxRoot::new_default(temp.path()).unwrap();

        let result = root.join("../escape");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SandboxError::ParentTraversal { .. }
        ));
    }

    #[test]
    fn test_sandbox_join_rejects_hidden_parent_traversal() {
        let temp = create_test_dir();
        let root = SandboxRoot::new_default(temp.path()).unwrap();

        let result = root.join("subdir/../../../escape");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SandboxError::ParentTraversal { .. }
        ));
    }

    #[test]
    fn test_sandbox_join_rejects_parent_at_end() {
        let temp = create_test_dir();
        let root = SandboxRoot::new_default(temp.path()).unwrap();

        let result = root.join("subdir/..");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SandboxError::ParentTraversal { .. }
        ));
    }

    // ========================================================================
    // SandboxRoot::join() - rejection of absolute paths
    // ========================================================================

    #[test]
    fn test_sandbox_join_rejects_absolute_path() {
        let temp = create_test_dir();
        let root = SandboxRoot::new_default(temp.path()).unwrap();

        #[cfg(unix)]
        let result = root.join("/etc/passwd");
        #[cfg(windows)]
        let result = root.join("C:\\Windows\\System32");

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SandboxError::AbsolutePath { .. }
        ));
    }

    // ========================================================================
    // SandboxRoot::join() - symlink handling
    // ========================================================================

    #[cfg(unix)]
    #[test]
    fn test_sandbox_join_rejects_symlink_by_default() {
        let temp = create_test_dir();
        let target = temp.path().join("target.txt");
        std::fs::write(&target, "content").unwrap();

        let link = temp.path().join("link.txt");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let root = SandboxRoot::new_default(temp.path()).unwrap();
        let result = root.join("link.txt");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SandboxError::SymlinkNotAllowed { .. }
        ));
    }

    #[cfg(unix)]
    #[test]
    fn test_sandbox_join_allows_symlink_when_configured() {
        let temp = create_test_dir();
        let target = temp.path().join("target.txt");
        std::fs::write(&target, "content").unwrap();

        let link = temp.path().join("link.txt");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let config = SandboxConfig::permissive();
        let root = SandboxRoot::new(temp.path(), config).unwrap();
        let result = root.join("link.txt");
        assert!(result.is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn test_sandbox_join_rejects_symlink_escape() {
        let temp = create_test_dir();
        let outside = TempDir::new().unwrap();
        let outside_file = outside.path().join("secret.txt");
        std::fs::write(&outside_file, "secret").unwrap();

        // Create a symlink inside the sandbox pointing outside
        let link = temp.path().join("escape_link");
        std::os::unix::fs::symlink(&outside_file, &link).unwrap();

        // Even with symlinks allowed, escape should be detected
        let config = SandboxConfig::permissive();
        let root = SandboxRoot::new(temp.path(), config).unwrap();
        let result = root.join("escape_link");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SandboxError::EscapeAttempt { .. }
        ));
    }

    /// Regression test for symlink traversal via non-existent paths.
    ///
    /// This tests the vulnerability where a symlinked directory inside the sandbox
    /// points outside, and the attacker creates a non-existent file under it.
    /// Since the final path doesn't exist, the old code skipped canonicalization,
    /// allowing the escape.
    ///
    /// Attack scenario:
    /// 1. Sandbox at /sandbox
    /// 2. /sandbox/escape_dir -> /tmp/attacker (symlink to outside)
    /// 3. Attacker calls root.join("escape_dir/malicious.txt")
    /// 4. Old code: path doesn't exist, skip canonicalization, allow it
    /// 5. Fixed code: canonicalize ancestor (escape_dir), detect escape
    #[cfg(unix)]
    #[test]
    fn test_sandbox_join_rejects_symlink_dir_escape_via_nonexistent_path() {
        let temp = create_test_dir();
        let outside = TempDir::new().unwrap();

        // Create a directory outside the sandbox
        let outside_dir = outside.path().join("attacker_controlled");
        std::fs::create_dir(&outside_dir).unwrap();

        // Create a symlink inside the sandbox pointing to the outside directory
        let escape_link = temp.path().join("escape_dir");
        std::os::unix::fs::symlink(&outside_dir, &escape_link).unwrap();

        // With symlinks allowed, try to join a NON-EXISTENT file under the symlinked dir
        // This is the vulnerability: the final path doesn't exist, so old code
        // would skip canonicalization and allow it
        let config = SandboxConfig::permissive();
        let root = SandboxRoot::new(temp.path(), config).unwrap();

        // This should be rejected - escape_dir resolves outside the sandbox
        let result = root.join("escape_dir/nonexistent_malicious_file.txt");
        assert!(
            result.is_err(),
            "Expected escape to be detected for non-existent path through symlinked directory"
        );
        assert!(matches!(
            result.unwrap_err(),
            SandboxError::EscapeAttempt { .. }
        ));
    }

    /// Test that safe symlinked directories work with non-existent paths.
    #[cfg(unix)]
    #[test]
    fn test_sandbox_join_allows_safe_symlink_dir_with_nonexistent_path() {
        let temp = create_test_dir();

        // Create a subdirectory inside the sandbox
        let inside_dir = temp.path().join("real_subdir");
        std::fs::create_dir(&inside_dir).unwrap();

        // Create a symlink inside the sandbox pointing to the inside directory
        let safe_link = temp.path().join("link_to_subdir");
        std::os::unix::fs::symlink(&inside_dir, &safe_link).unwrap();

        // With symlinks allowed, joining a non-existent file under a SAFE symlink should work
        let config = SandboxConfig::permissive();
        let root = SandboxRoot::new(temp.path(), config).unwrap();

        // This should succeed - link_to_subdir resolves to inside the sandbox
        let result = root.join("link_to_subdir/new_file.txt");
        assert!(
            result.is_ok(),
            "Expected safe symlink with non-existent path to succeed"
        );
    }

    // ========================================================================
    // SandboxRoot::join() - hardlink handling (Unix only)
    // ========================================================================

    #[cfg(unix)]
    #[test]
    fn test_sandbox_join_rejects_hardlink_by_default() {
        let temp = create_test_dir();
        let original = temp.path().join("original.txt");
        std::fs::write(&original, "content").unwrap();

        let hardlink = temp.path().join("hardlink.txt");
        std::fs::hard_link(&original, &hardlink).unwrap();

        let root = SandboxRoot::new_default(temp.path()).unwrap();
        let result = root.join("hardlink.txt");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SandboxError::HardlinkNotAllowed { .. }
        ));
    }

    #[cfg(unix)]
    #[test]
    fn test_sandbox_join_allows_hardlink_when_configured() {
        let temp = create_test_dir();
        let original = temp.path().join("original.txt");
        std::fs::write(&original, "content").unwrap();

        let hardlink = temp.path().join("hardlink.txt");
        std::fs::hard_link(&original, &hardlink).unwrap();

        let config = SandboxConfig::permissive();
        let root = SandboxRoot::new(temp.path(), config).unwrap();
        let result = root.join("hardlink.txt");
        assert!(result.is_ok());
    }

    // ========================================================================
    // SandboxPath tests
    // ========================================================================

    #[test]
    fn test_sandbox_path_as_path() {
        let temp = create_test_dir();
        let file = temp.path().join("file.txt");
        std::fs::write(&file, "content").unwrap();

        let root = SandboxRoot::new_default(temp.path()).unwrap();
        let sandbox_path = root.join("file.txt").unwrap();

        // as_path should return the full path
        assert!(sandbox_path.as_path().ends_with("file.txt"));
        assert!(sandbox_path.as_path().is_absolute());
    }

    #[test]
    fn test_sandbox_path_relative() {
        let temp = create_test_dir();
        let subdir = temp.path().join("a/b/c");
        std::fs::create_dir_all(&subdir).unwrap();
        let file = subdir.join("file.txt");
        std::fs::write(&file, "content").unwrap();

        let root = SandboxRoot::new_default(temp.path()).unwrap();
        let sandbox_path = root.join("a/b/c/file.txt").unwrap();

        // relative should return just the relative portion
        assert_eq!(sandbox_path.relative(), Path::new("a/b/c/file.txt"));
    }

    #[test]
    fn test_sandbox_path_to_path_buf() {
        let temp = create_test_dir();
        let file = temp.path().join("file.txt");
        std::fs::write(&file, "content").unwrap();

        let root = SandboxRoot::new_default(temp.path()).unwrap();
        let sandbox_path = root.join("file.txt").unwrap();

        let path_buf = sandbox_path.to_path_buf();
        assert!(path_buf.is_absolute());
        assert!(path_buf.ends_with("file.txt"));
    }

    #[test]
    fn test_sandbox_path_as_ref() {
        let temp = create_test_dir();
        let file = temp.path().join("file.txt");
        std::fs::write(&file, "content").unwrap();

        let root = SandboxRoot::new_default(temp.path()).unwrap();
        let sandbox_path = root.join("file.txt").unwrap();

        // Test AsRef<Path> implementation
        let path_ref: &Path = sandbox_path.as_ref();
        assert!(path_ref.ends_with("file.txt"));
    }

    // ========================================================================
    // SandboxConfig tests
    // ========================================================================

    #[test]
    fn test_sandbox_config_default() {
        let config = SandboxConfig::default();
        assert!(!config.allow_symlinks);
        assert!(!config.allow_hardlinks);
    }

    #[test]
    fn test_sandbox_config_permissive() {
        let config = SandboxConfig::permissive();
        assert!(config.allow_symlinks);
        assert!(config.allow_hardlinks);
    }

    // ========================================================================
    // SandboxError tests
    // ========================================================================

    #[test]
    fn test_sandbox_error_display() {
        let err = SandboxError::ParentTraversal {
            path: "../escape".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("parent directory traversal"));
        assert!(msg.contains("../escape"));
    }

    #[test]
    fn test_sandbox_error_equality() {
        let err1 = SandboxError::AbsolutePath {
            path: "/etc/passwd".to_string(),
        };
        let err2 = SandboxError::AbsolutePath {
            path: "/etc/passwd".to_string(),
        };
        assert_eq!(err1, err2);
    }
}

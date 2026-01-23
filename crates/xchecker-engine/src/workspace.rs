//! Workspace management for xchecker
//!
//! This module provides workspace registry functionality for managing multiple specs
//! within a project. A workspace is defined by a `workspace.yaml` file that contains
//! metadata about registered specs.
//!
//! Requirements:
//! - 4.3.1: `xchecker project init <name>` creates workspace registry
//! - 4.3.6: Workspace discovery searches upward from CWD

use crate::atomic_write::write_file_atomic;
use anyhow::{Context, Result};
use camino::Utf8Path;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Workspace configuration file name
pub const WORKSPACE_FILE_NAME: &str = "workspace.yaml";

/// Current workspace schema version
pub const WORKSPACE_SCHEMA_VERSION: &str = "1";

/// Workspace registry containing metadata about specs in a project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    /// Schema version for forward compatibility
    pub version: String,
    /// Human-readable name for the workspace
    pub name: String,
    /// List of registered specs
    #[serde(default)]
    pub specs: Vec<WorkspaceSpec>,
}

/// Metadata for a spec registered in the workspace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSpec {
    /// Unique identifier for the spec
    pub id: String,
    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,
    /// Timestamp when the spec was added to the workspace
    pub added: DateTime<Utc>,
}

impl Workspace {
    /// Create a new workspace with the given name
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self {
            version: WORKSPACE_SCHEMA_VERSION.to_string(),
            name: name.to_string(),
            specs: Vec::new(),
        }
    }

    /// Load a workspace from a file
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read workspace file: {}", path.display()))?;

        let workspace: Self = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse workspace file: {}", path.display()))?;

        Ok(workspace)
    }

    /// Save the workspace to a file
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = serde_yaml::to_string(self).context("Failed to serialize workspace")?;
        let path_utf8 = Utf8Path::from_path(path)
            .ok_or_else(|| anyhow::anyhow!("Path is not valid UTF-8: {}", path.display()))?;

        write_file_atomic(path_utf8, &content)
            .with_context(|| format!("Failed to write workspace file: {}", path.display()))?;

        Ok(())
    }

    /// Add a spec to the workspace
    ///
    /// Returns an error if the spec already exists and `force` is false.
    pub fn add_spec(&mut self, id: &str, tags: Vec<String>, force: bool) -> Result<()> {
        // Check if spec already exists
        if let Some(existing) = self.specs.iter().position(|s| s.id == id) {
            if force {
                // Remove existing spec to replace it
                self.specs.remove(existing);
            } else {
                anyhow::bail!(
                    "Spec '{}' already exists in workspace. Use --force to override.",
                    id
                );
            }
        }

        self.specs.push(WorkspaceSpec {
            id: id.to_string(),
            tags,
            added: Utc::now(),
        });

        Ok(())
    }

    /// Test seam; not part of public API stability guarantees.
    ///
    /// Get a spec by ID.
    #[allow(dead_code)] // Test seam; not part of public API stability guarantees
    #[must_use]
    pub fn get_spec(&self, id: &str) -> Option<&WorkspaceSpec> {
        self.specs.iter().find(|s| s.id == id)
    }

    /// List all specs in the workspace
    #[must_use]
    pub fn list_specs(&self) -> &[WorkspaceSpec] {
        &self.specs
    }
}

/// Discover workspace by searching upward from the given directory
///
/// This function searches upward from `start_dir` looking for a `workspace.yaml` file.
/// The first workspace found is returned (no merging of multiple workspaces).
///
/// # Arguments
/// * `start_dir` - Directory to start searching from
///
/// # Returns
/// * `Ok(Some(path))` - Path to the discovered workspace file
/// * `Ok(None)` - No workspace file found
/// * `Err(_)` - Error during discovery
pub fn discover_workspace(start_dir: &Path) -> Result<Option<PathBuf>> {
    let mut current_dir = start_dir.to_path_buf();

    // Canonicalize to handle relative paths and symlinks
    if current_dir.is_relative() {
        current_dir = std::env::current_dir()
            .context("Failed to get current directory")?
            .join(&current_dir);
    }

    loop {
        let workspace_path = current_dir.join(WORKSPACE_FILE_NAME);
        if workspace_path.exists() && workspace_path.is_file() {
            return Ok(Some(workspace_path));
        }

        // Move to parent directory
        match current_dir.parent() {
            Some(parent) => {
                current_dir = parent.to_path_buf();
            }
            None => {
                // Reached filesystem root, no workspace found
                return Ok(None);
            }
        }
    }
}

/// Discover workspace from current working directory
///
/// Convenience function that starts discovery from CWD.
pub fn discover_workspace_from_cwd() -> Result<Option<PathBuf>> {
    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    discover_workspace(&cwd)
}

/// Resolve workspace path with optional override
///
/// If `override_path` is provided, it is used directly.
/// Otherwise, workspace discovery is performed from CWD.
///
/// # Arguments
/// * `override_path` - Optional explicit path to workspace file
///
/// # Returns
/// * `Ok(Some(path))` - Resolved workspace path
/// * `Ok(None)` - No workspace found and no override provided
/// * `Err(_)` - Error during resolution
pub fn resolve_workspace(override_path: Option<&Path>) -> Result<Option<PathBuf>> {
    if let Some(path) = override_path {
        // Validate that the override path exists
        if !path.exists() {
            anyhow::bail!("Workspace file not found: {}", path.display());
        }
        Ok(Some(path.to_path_buf()))
    } else {
        discover_workspace_from_cwd()
    }
}

/// Initialize a new workspace in the given directory
///
/// Creates a `workspace.yaml` file with the given name.
///
/// # Arguments
/// * `dir` - Directory to create the workspace in
/// * `name` - Name for the workspace
///
/// # Returns
/// * `Ok(path)` - Path to the created workspace file
/// * `Err(_)` - Error during creation
pub fn init_workspace(dir: &Path, name: &str) -> Result<PathBuf> {
    let workspace_path = dir.join(WORKSPACE_FILE_NAME);

    // Check if workspace already exists
    if workspace_path.exists() {
        anyhow::bail!("Workspace already exists at: {}", workspace_path.display());
    }

    // Create the workspace
    let workspace = Workspace::new(name);
    workspace.save(&workspace_path)?;

    Ok(workspace_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_workspace_new() {
        let workspace = Workspace::new("test-project");
        assert_eq!(workspace.version, WORKSPACE_SCHEMA_VERSION);
        assert_eq!(workspace.name, "test-project");
        assert!(workspace.specs.is_empty());
    }

    #[test]
    fn test_workspace_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_path = temp_dir.path().join(WORKSPACE_FILE_NAME);

        // Create and save workspace
        let mut workspace = Workspace::new("test-project");
        workspace
            .add_spec("spec-1", vec!["tag1".to_string()], false)
            .unwrap();
        workspace.save(&workspace_path).unwrap();

        // Load and verify
        let loaded = Workspace::load(&workspace_path).unwrap();
        assert_eq!(loaded.name, "test-project");
        assert_eq!(loaded.specs.len(), 1);
        assert_eq!(loaded.specs[0].id, "spec-1");
        assert_eq!(loaded.specs[0].tags, vec!["tag1"]);
    }

    #[test]
    fn test_workspace_add_spec_duplicate() {
        let mut workspace = Workspace::new("test");
        workspace.add_spec("spec-1", vec![], false).unwrap();

        // Should fail without force
        let result = workspace.add_spec("spec-1", vec![], false);
        assert!(result.is_err());

        // Should succeed with force
        workspace
            .add_spec("spec-1", vec!["new-tag".to_string()], true)
            .unwrap();
        assert_eq!(workspace.specs.len(), 1);
        assert_eq!(workspace.specs[0].tags, vec!["new-tag"]);
    }

    #[test]
    fn test_discover_workspace_in_current_dir() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_path = temp_dir.path().join(WORKSPACE_FILE_NAME);

        // Create workspace file
        let workspace = Workspace::new("test");
        workspace.save(&workspace_path).unwrap();

        // Discover from same directory
        let discovered = discover_workspace(temp_dir.path()).unwrap();
        assert!(discovered.is_some());
        assert_eq!(discovered.unwrap(), workspace_path);
    }

    #[test]
    fn test_discover_workspace_in_parent_dir() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_path = temp_dir.path().join(WORKSPACE_FILE_NAME);

        // Create workspace file in root
        let workspace = Workspace::new("test");
        workspace.save(&workspace_path).unwrap();

        // Create subdirectory
        let sub_dir = temp_dir.path().join("subdir").join("nested");
        std::fs::create_dir_all(&sub_dir).unwrap();

        // Discover from subdirectory
        let discovered = discover_workspace(&sub_dir).unwrap();
        assert!(discovered.is_some());
        assert_eq!(discovered.unwrap(), workspace_path);
    }

    #[test]
    fn test_discover_workspace_not_found() {
        let temp_dir = TempDir::new().unwrap();

        // No workspace file exists
        let discovered = discover_workspace(temp_dir.path()).unwrap();
        assert!(discovered.is_none());
    }

    #[test]
    fn test_discover_workspace_first_found() {
        let temp_dir = TempDir::new().unwrap();

        // Create workspace in root
        let root_workspace_path = temp_dir.path().join(WORKSPACE_FILE_NAME);
        let root_workspace = Workspace::new("root");
        root_workspace.save(&root_workspace_path).unwrap();

        // Create subdirectory with its own workspace
        let sub_dir = temp_dir.path().join("subdir");
        std::fs::create_dir_all(&sub_dir).unwrap();
        let sub_workspace_path = sub_dir.join(WORKSPACE_FILE_NAME);
        let sub_workspace = Workspace::new("sub");
        sub_workspace.save(&sub_workspace_path).unwrap();

        // Discover from subdirectory should find the subdirectory workspace first
        let discovered = discover_workspace(&sub_dir).unwrap();
        assert!(discovered.is_some());
        assert_eq!(discovered.unwrap(), sub_workspace_path);
    }

    #[test]
    fn test_init_workspace() {
        let temp_dir = TempDir::new().unwrap();

        let workspace_path = init_workspace(temp_dir.path(), "my-project").unwrap();
        assert!(workspace_path.exists());

        let workspace = Workspace::load(&workspace_path).unwrap();
        assert_eq!(workspace.name, "my-project");
    }

    #[test]
    fn test_init_workspace_already_exists() {
        let temp_dir = TempDir::new().unwrap();

        // Create first workspace
        init_workspace(temp_dir.path(), "first").unwrap();

        // Second init should fail
        let result = init_workspace(temp_dir.path(), "second");
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_workspace_with_override() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_path = temp_dir.path().join(WORKSPACE_FILE_NAME);

        // Create workspace
        let workspace = Workspace::new("test");
        workspace.save(&workspace_path).unwrap();

        // Resolve with explicit path
        let resolved = resolve_workspace(Some(&workspace_path)).unwrap();
        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap(), workspace_path);
    }

    #[test]
    fn test_resolve_workspace_override_not_found() {
        let result = resolve_workspace(Some(Path::new("/nonexistent/workspace.yaml")));
        assert!(result.is_err());
    }
}

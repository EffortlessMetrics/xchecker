use std::path::PathBuf;

use crate::error::FixupError;
use crate::paths::{SandboxConfig, SandboxError, SandboxRoot};

/// Validates that a fixup target path is safe to apply patches to.
///
/// This function ensures that:
/// - The path is not absolute
/// - The path does not contain parent directory (`..`) components
/// - The path is not a symlink (unless `allow_links` is true)
/// - The path is not a hardlink (unless `allow_links` is true)
/// - After symlink resolution, the path resolves within the repository root
///
/// This function delegates validation to `SandboxRoot` to keep path policy
/// consistent across fixup parsing and application.
///
/// # Arguments
///
/// * `path` - The target path to validate (relative to repo root)
/// * `repo_root` - The repository root directory
/// * `allow_links` - Whether to allow symlinks and hardlinks (default: false)
///
/// # Returns
///
/// Returns `Ok(())` if the path is valid, or a `FixupError` describing why it's invalid.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use xchecker_engine::fixup::validate_fixup_target;
///
/// let repo_root = Path::new("/home/user/project");
/// let target = Path::new("src/main.rs");
///
/// // Valid path
/// assert!(validate_fixup_target(target, repo_root, false).is_ok());
///
/// // Invalid: absolute path
/// let absolute = Path::new("/etc/passwd");
/// assert!(validate_fixup_target(absolute, repo_root, false).is_err());
///
/// // Invalid: parent directory escape
/// let escape = Path::new("../../../etc/passwd");
/// assert!(validate_fixup_target(escape, repo_root, false).is_err());
/// ```
pub fn validate_fixup_target(
    path: &std::path::Path,
    repo_root: &std::path::Path,
    allow_links: bool,
) -> Result<(), FixupError> {
    let config = SandboxConfig {
        allow_symlinks: allow_links,
        allow_hardlinks: allow_links,
    };

    let sandbox_root = SandboxRoot::new(repo_root, config).map_err(map_root_err)?;
    let sandbox_path = sandbox_root.join(path).map_err(map_join_err)?;
    if !sandbox_path.as_path().exists() {
        return Err(FixupError::TargetFileNotFound {
            path: path.display().to_string(),
        });
    }

    Ok(())
}

fn map_root_err(err: SandboxError) -> FixupError {
    match err {
        SandboxError::RootNotFound { path } | SandboxError::RootNotDirectory { path } => {
            FixupError::CanonicalizationError(format!("Invalid repo root: {path}"))
        }
        SandboxError::RootCanonicalizationFailed { path, reason }
        | SandboxError::PathCanonicalizationFailed { path, reason } => {
            FixupError::CanonicalizationError(format!(
                "Failed to canonicalize repo root {path}: {reason}"
            ))
        }
        SandboxError::AbsolutePath { path } => FixupError::AbsolutePath(PathBuf::from(path)),
        SandboxError::ParentTraversal { path } => FixupError::ParentDirEscape(PathBuf::from(path)),
        SandboxError::EscapeAttempt { path, .. } => FixupError::OutsideRepo(PathBuf::from(path)),
        SandboxError::SymlinkNotAllowed { path } => {
            FixupError::SymlinkNotAllowed(PathBuf::from(path))
        }
        SandboxError::HardlinkNotAllowed { path } => {
            FixupError::HardlinkNotAllowed(PathBuf::from(path))
        }
    }
}

fn map_join_err(err: SandboxError) -> FixupError {
    match err {
        SandboxError::AbsolutePath { path } => FixupError::AbsolutePath(PathBuf::from(path)),
        SandboxError::ParentTraversal { path } => FixupError::ParentDirEscape(PathBuf::from(path)),
        SandboxError::EscapeAttempt { path, .. } => FixupError::OutsideRepo(PathBuf::from(path)),
        SandboxError::SymlinkNotAllowed { path } => {
            FixupError::SymlinkNotAllowed(PathBuf::from(path))
        }
        SandboxError::HardlinkNotAllowed { path } => {
            FixupError::HardlinkNotAllowed(PathBuf::from(path))
        }
        SandboxError::RootNotFound { path } | SandboxError::RootNotDirectory { path } => {
            FixupError::CanonicalizationError(format!("Invalid repo root: {path}"))
        }
        SandboxError::RootCanonicalizationFailed { path, reason }
        | SandboxError::PathCanonicalizationFailed { path, reason } => {
            FixupError::CanonicalizationError(format!("Failed to canonicalize {path}: {reason}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::validate_fixup_target;
    use crate::error::FixupError;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_validate_fixup_target_rejects_absolute_paths() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();

        // Create a test file in the repo
        let test_file = repo_root.join("test.txt");
        fs::write(&test_file, "test content").unwrap();

        // Test absolute path rejection - use platform-appropriate absolute path
        #[cfg(unix)]
        let absolute_path = std::path::Path::new("/etc/passwd");

        #[cfg(windows)]
        let absolute_path = std::path::Path::new("C:\\Windows\\System32\\config\\sam");

        let result = validate_fixup_target(absolute_path, repo_root, false);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), FixupError::AbsolutePath(_)));
    }

    #[test]
    fn test_validate_fixup_target_rejects_parent_dir_escapes() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();

        // Create a test file in the repo
        let test_file = repo_root.join("test.txt");
        fs::write(&test_file, "test content").unwrap();

        // Test parent directory escape rejection
        let escape_path = std::path::Path::new("../../../etc/passwd");
        let result = validate_fixup_target(escape_path, repo_root, false);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            FixupError::ParentDirEscape(_)
        ));

        // Test another escape pattern
        let escape_path2 = std::path::Path::new("subdir/../../outside.txt");
        let result2 = validate_fixup_target(escape_path2, repo_root, false);
        assert!(result2.is_err());
        assert!(matches!(
            result2.unwrap_err(),
            FixupError::ParentDirEscape(_)
        ));
    }

    #[test]
    fn test_validate_fixup_target_accepts_valid_relative_paths() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();

        // Create test files in the repo
        let test_file = repo_root.join("test.txt");
        fs::write(&test_file, "test content").unwrap();

        let subdir = repo_root.join("subdir");
        fs::create_dir(&subdir).unwrap();
        let nested_file = subdir.join("nested.txt");
        fs::write(&nested_file, "nested content").unwrap();

        // Test valid relative paths
        let valid_path1 = std::path::Path::new("test.txt");
        assert!(validate_fixup_target(valid_path1, repo_root, false).is_ok());

        let valid_path2 = std::path::Path::new("subdir/nested.txt");
        assert!(validate_fixup_target(valid_path2, repo_root, false).is_ok());
    }

    #[test]
    fn test_validate_fixup_target_rejects_symlinks_by_default() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();

        // Create a regular file in the repo
        let target_file = repo_root.join("target.txt");
        fs::write(&target_file, "target content").unwrap();

        // Create a symlink inside the repo pointing to the target file
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let symlink_path = repo_root.join("link_to_target");
            symlink(&target_file, &symlink_path).unwrap();

            // Test that symlink is rejected by default
            let result =
                validate_fixup_target(std::path::Path::new("link_to_target"), repo_root, false);
            assert!(result.is_err());
            assert!(matches!(
                result.unwrap_err(),
                FixupError::SymlinkNotAllowed(_)
            ));
        }

        #[cfg(windows)]
        {
            use std::os::windows::fs::symlink_file;
            let symlink_path = repo_root.join("link_to_target");
            // Windows symlinks require admin privileges, so we skip if it fails
            if symlink_file(&target_file, &symlink_path).is_ok() {
                let result =
                    validate_fixup_target(std::path::Path::new("link_to_target"), repo_root, false);
                assert!(result.is_err());
                assert!(matches!(
                    result.unwrap_err(),
                    FixupError::SymlinkNotAllowed(_)
                ));
            }
        }
    }

    #[test]
    fn test_validate_fixup_target_allows_symlinks_with_flag() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();

        // Create a regular file in the repo
        let target_file = repo_root.join("target.txt");
        fs::write(&target_file, "target content").unwrap();

        // Create a symlink inside the repo pointing to the target file
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let symlink_path = repo_root.join("link_to_target");
            symlink(&target_file, &symlink_path).unwrap();

            // Test that symlink is allowed with allow_links=true
            let result =
                validate_fixup_target(std::path::Path::new("link_to_target"), repo_root, true);
            assert!(result.is_ok());
        }

        #[cfg(windows)]
        {
            use std::os::windows::fs::symlink_file;
            let symlink_path = repo_root.join("link_to_target");
            // Windows symlinks require admin privileges, so we skip if it fails
            if symlink_file(&target_file, &symlink_path).is_ok() {
                let result =
                    validate_fixup_target(std::path::Path::new("link_to_target"), repo_root, true);
                assert!(result.is_ok());
            }
        }
    }

    #[test]
    fn test_validate_fixup_target_rejects_hardlinks_by_default() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();

        // Create a regular file in the repo
        let target_file = repo_root.join("target.txt");
        fs::write(&target_file, "target content").unwrap();

        // Create a hardlink to the target file
        #[cfg(unix)]
        {
            let hardlink_path = repo_root.join("hardlink_to_target");
            std::fs::hard_link(&target_file, &hardlink_path).unwrap();

            // Test that hardlink is rejected by default
            let result =
                validate_fixup_target(std::path::Path::new("hardlink_to_target"), repo_root, false);
            assert!(result.is_err());
            assert!(matches!(
                result.unwrap_err(),
                FixupError::HardlinkNotAllowed(_)
            ));
        }

        #[cfg(windows)]
        {
            use std::fs::hard_link;
            let hardlink_path = repo_root.join("hardlink_to_target");
            // Try to create hardlink, skip if it fails (requires permissions)
            if hard_link(&target_file, &hardlink_path).is_ok() {
                // Test that hardlink is rejected by default
                let result = validate_fixup_target(
                    std::path::Path::new("hardlink_to_target"),
                    repo_root,
                    false,
                );
                assert!(result.is_err());
                assert!(matches!(
                    result.unwrap_err(),
                    FixupError::HardlinkNotAllowed(_)
                ));
            } else {
                println!(
                    "Skipping hardlink rejection test on Windows (creating hardlink requires elevated permissions)"
                );
            }
        }
    }

    #[test]
    fn test_validate_fixup_target_allows_hardlinks_with_flag() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();

        // Create a regular file in the repo
        let target_file = repo_root.join("target.txt");
        fs::write(&target_file, "target content").unwrap();

        // Create a hardlink to the target file
        #[cfg(unix)]
        {
            let hardlink_path = repo_root.join("hardlink_to_target");
            std::fs::hard_link(&target_file, &hardlink_path).unwrap();

            // Test that hardlink is allowed with allow_links=true
            let result =
                validate_fixup_target(std::path::Path::new("hardlink_to_target"), repo_root, true);
            assert!(result.is_ok());
        }

        #[cfg(windows)]
        {
            use std::fs::hard_link;
            let hardlink_path = repo_root.join("hardlink_to_target");
            // Try to create hardlink, skip if it fails (requires permissions)
            if hard_link(&target_file, &hardlink_path).is_ok() {
                // Test that hardlink is allowed with allow_links=true
                let result = validate_fixup_target(
                    std::path::Path::new("hardlink_to_target"),
                    repo_root,
                    true,
                );
                assert!(result.is_ok());
            } else {
                println!(
                    "Skipping hardlink allow test on Windows (creating hardlink requires elevated permissions)"
                );
            }
        }
    }

    #[test]
    fn test_validate_fixup_target_symlink_escape() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();

        // Create a directory outside the repo
        let outside_dir = temp_dir.path().parent().unwrap().join("outside");
        fs::create_dir_all(&outside_dir).unwrap();
        let outside_file = outside_dir.join("secret.txt");
        fs::write(&outside_file, "secret content").unwrap();

        // Create a symlink inside the repo pointing outside
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let symlink_path = repo_root.join("escape_link");
            let _ = symlink(&outside_file, &symlink_path);

            // Test that symlink is rejected by default (before checking if it escapes)
            let result =
                validate_fixup_target(std::path::Path::new("escape_link"), repo_root, false);
            assert!(result.is_err());
            // Should fail with SymlinkNotAllowed before checking OutsideRepo
            assert!(matches!(
                result.unwrap_err(),
                FixupError::SymlinkNotAllowed(_)
            ));

            // Test that symlink escape is detected when allow_links=true
            let result_with_links =
                validate_fixup_target(std::path::Path::new("escape_link"), repo_root, true);
            assert!(result_with_links.is_err());
            assert!(matches!(
                result_with_links.unwrap_err(),
                FixupError::OutsideRepo(_)
            ));
        }

        #[cfg(windows)]
        {
            use std::os::windows::fs::symlink_file;
            let symlink_path = repo_root.join("escape_link");
            // Windows symlinks require admin privileges, so we skip if it fails
            if symlink_file(&outside_file, &symlink_path).is_ok() {
                // Test that symlink is rejected by default
                let result =
                    validate_fixup_target(std::path::Path::new("escape_link"), repo_root, false);
                assert!(result.is_err());
                assert!(matches!(
                    result.unwrap_err(),
                    FixupError::SymlinkNotAllowed(_)
                ));

                // Test that symlink escape is detected when allow_links=true
                let result_with_links =
                    validate_fixup_target(std::path::Path::new("escape_link"), repo_root, true);
                assert!(result_with_links.is_err());
                assert!(matches!(
                    result_with_links.unwrap_err(),
                    FixupError::OutsideRepo(_)
                ));
            }
        }
    }

    #[test]
    #[cfg(windows)]
    fn test_validate_fixup_target_windows_case_insensitive() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();

        // Create a test file
        let test_file = repo_root.join("Test.txt");
        fs::write(&test_file, "test content").unwrap();

        // Test that different case variations are accepted (Windows is case-insensitive)
        let lower_case = std::path::Path::new("test.txt");
        let result = validate_fixup_target(lower_case, repo_root, false);
        // This should succeed because Windows paths are case-insensitive
        assert!(result.is_ok());

        let upper_case = std::path::Path::new("TEST.TXT");
        let result2 = validate_fixup_target(upper_case, repo_root, false);
        assert!(result2.is_ok());
    }

    #[test]
    fn test_validate_fixup_target_nonexistent_file() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();

        // Test with a file that doesn't exist
        let nonexistent = std::path::Path::new("does_not_exist.txt");
        let result = validate_fixup_target(nonexistent, repo_root, false);

        // Should fail with missing file error since the file doesn't exist
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            FixupError::TargetFileNotFound { .. }
        ));
    }

    #[test]
    fn test_validate_fixup_target_with_dot_components() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();

        // Create a test file
        let test_file = repo_root.join("test.txt");
        fs::write(&test_file, "test content").unwrap();

        // Test that paths with . components are accepted (they don't escape)
        let dot_path = std::path::Path::new("./test.txt");
        let result = validate_fixup_target(dot_path, repo_root, false);
        assert!(result.is_ok());

        // Test nested . components
        let nested_dot = std::path::Path::new("./subdir/../test.txt");
        // This should fail because it contains .. component
        let result2 = validate_fixup_target(nested_dot, repo_root, false);
        assert!(result2.is_err());
    }
}

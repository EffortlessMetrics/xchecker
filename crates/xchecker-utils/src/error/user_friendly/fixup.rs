use super::UserFriendlyError;
use crate::error::{ErrorCategory, FixupError};

impl UserFriendlyError for FixupError {
    fn user_message(&self) -> String {
        match self {
            Self::NoFixupMarkersFound => {
                "No fixup changes were found in the review output".to_string()
            }
            Self::InvalidDiffFormat {
                block_index,
                reason,
            } => {
                format!("Diff block {block_index} has invalid format: {reason}")
            }
            Self::GitApplyValidationFailed {
                target_file,
                reason,
            } => {
                format!("Cannot apply changes to '{target_file}': {reason}")
            }
            Self::GitApplyExecutionFailed {
                target_file,
                reason,
            } => {
                format!("Failed to apply changes to '{target_file}': {reason}")
            }
            Self::TargetFileNotFound { path } => {
                format!("Target file '{path}' does not exist")
            }
            Self::TempCopyFailed { file, reason } => {
                format!("Could not create temporary copy of '{file}': {reason}")
            }
            Self::DiffParsingFailed { reason } => {
                format!("Could not parse diff content: {reason}")
            }
            Self::NoValidDiffBlocks => "No valid diff blocks found in the fixup plan".to_string(),
            Self::AbsolutePath(path) => {
                format!("Absolute paths are not allowed: {}", path.display())
            }
            Self::ParentDirEscape(path) => {
                format!(
                    "Path attempts to escape parent directory: {}",
                    path.display()
                )
            }
            Self::OutsideRepo(path) => {
                format!("Path resolves outside repository root: {}", path.display())
            }
            Self::CanonicalizationError(reason) => {
                format!("Could not resolve file path: {reason}")
            }
            Self::SymlinkNotAllowed(path) => {
                format!(
                    "Symlinks are not allowed: {} (use --allow-links to permit)",
                    path.display()
                )
            }
            Self::HardlinkNotAllowed(path) => {
                format!(
                    "Hardlinks are not allowed: {} (use --allow-links to permit)",
                    path.display()
                )
            }
            Self::FuzzyMatchFailed {
                file,
                expected_line,
                search_window,
            } => {
                format!(
                    "Could not find matching context for diff hunk at line {} in '{}' (searched ±{} lines)",
                    expected_line, file, search_window
                )
            }
        }
    }

    fn context(&self) -> Option<String> {
        match self {
            Self::NoFixupMarkersFound => {
                Some("The review phase should produce a 'FIXUP PLAN:' section with unified diffs for files that need changes.".to_string())
            }
            Self::InvalidDiffFormat { .. } => {
                Some("Fixup diffs must follow the unified diff format with proper headers and hunks.".to_string())
            }
            Self::GitApplyValidationFailed { .. } | Self::GitApplyExecutionFailed { .. } => {
                Some("Git apply is used to safely apply diff patches to files with validation.".to_string())
            }
            Self::TargetFileNotFound { .. } => {
                Some("Fixup targets must exist in the repository before changes can be applied.".to_string())
            }
            Self::TempCopyFailed { .. } => {
                Some("Temporary copies are created to safely test changes before applying them.".to_string())
            }
            Self::DiffParsingFailed { .. } | Self::NoValidDiffBlocks => {
                Some("Fixup plans contain unified diff blocks that describe file changes.".to_string())
            }
            Self::AbsolutePath(_) | Self::ParentDirEscape(_) | Self::OutsideRepo(_) => {
                Some("Fixup paths are validated to prevent directory traversal and ensure changes stay within the repository.".to_string())
            }
            Self::CanonicalizationError(_) => {
                Some("Path canonicalization resolves symlinks and relative paths to absolute paths for validation.".to_string())
            }
            Self::SymlinkNotAllowed(_) | Self::HardlinkNotAllowed(_) => {
                Some("Symlinks and hardlinks are blocked by default for security. Use --allow-links to permit them.".to_string())
            }
            Self::FuzzyMatchFailed { .. } => {
                Some("The diff hunk's context lines couldn't be matched to the file, which may indicate the file has changed since the diff was generated.".to_string())
            }
        }
    }

    fn suggestions(&self) -> Vec<String> {
        match self {
            Self::NoFixupMarkersFound => vec![
                "Check the review output in .xchecker/specs/<id>/artifacts/review.md".to_string(),
                "Ensure the review phase completed successfully".to_string(),
                "The review phase may not have identified any changes needed".to_string(),
                "Try running the review phase again if it failed".to_string(),
            ],
            Self::InvalidDiffFormat {
                block_index,
                reason,
            } => vec![
                format!("Review diff block {} in the review output", block_index),
                "Ensure the diff follows unified diff format (--- and +++ headers)".to_string(),
                "Check that hunk headers use @@ format".to_string(),
                format!("Specific issue: {}", reason),
            ],
            Self::GitApplyValidationFailed {
                target_file,
                reason,
            } => vec![
                format!("Check the current state of '{}'", target_file),
                "The file may have been modified since the review phase".to_string(),
                "Try running the review phase again to generate fresh diffs".to_string(),
                format!("Git apply error: {}", reason),
                "Use --dry-run to preview changes without applying them".to_string(),
            ],
            Self::GitApplyExecutionFailed {
                target_file,
                reason,
            } => vec![
                format!("Check file permissions for '{}'", target_file),
                "Ensure the file is writable".to_string(),
                "Check available disk space".to_string(),
                format!("Git apply error: {}", reason),
                "Try running with --verbose for more details".to_string(),
            ],
            Self::TargetFileNotFound { path } => vec![
                format!("Verify that '{}' exists in the repository", path),
                "The file may have been deleted or moved since the review phase".to_string(),
                "Check the file path is correct and relative to the repository root".to_string(),
                "Run the review phase again to generate fresh fixup plans".to_string(),
            ],
            Self::TempCopyFailed { file, reason } => vec![
                "Check available disk space for temporary files".to_string(),
                "Ensure you have write permissions in the temp directory".to_string(),
                format!("File: {}", file),
                format!("Reason: {}", reason),
            ],
            Self::DiffParsingFailed { reason } => vec![
                "Check the review output format".to_string(),
                "Ensure the FIXUP PLAN section contains valid unified diffs".to_string(),
                format!("Parsing error: {}", reason),
                "Try running the review phase again".to_string(),
            ],
            Self::NoValidDiffBlocks => vec![
                "Check the review output for FIXUP PLAN section".to_string(),
                "Ensure diff blocks follow unified diff format".to_string(),
                "The review phase may not have generated any valid changes".to_string(),
                "Try running the review phase again".to_string(),
            ],
            Self::AbsolutePath(path) => vec![
                format!("Use relative paths instead of absolute: {}", path.display()),
                "Fixup paths must be relative to the repository root".to_string(),
                "Remove leading '/' or drive letters from paths".to_string(),
            ],
            Self::ParentDirEscape(path) => vec![
                format!("Remove '..' components from path: {}", path.display()),
                "Fixup paths must not escape the repository directory".to_string(),
                "Use paths relative to the repository root".to_string(),
            ],
            Self::OutsideRepo(path) => vec![
                format!("Path resolves outside repository: {}", path.display()),
                "Ensure all fixup targets are within the repository".to_string(),
                "Check for symlinks that point outside the repository".to_string(),
                "Use --allow-links if you need to modify symlinked files".to_string(),
            ],
            Self::CanonicalizationError(reason) => vec![
                "Check that the file path exists and is accessible".to_string(),
                "Verify file permissions allow reading the path".to_string(),
                format!("Error: {}", reason),
            ],
            Self::SymlinkNotAllowed(path) => vec![
                format!("Symlink detected: {}", path.display()),
                "Use --allow-links flag to permit symlink modifications".to_string(),
                "Consider modifying the symlink target directly instead".to_string(),
                "Symlinks are blocked by default for security".to_string(),
            ],
            Self::HardlinkNotAllowed(path) => vec![
                format!("Hardlink detected: {}", path.display()),
                "Use --allow-links flag to permit hardlink modifications".to_string(),
                "Consider modifying one of the linked files directly".to_string(),
                "Hardlinks are blocked by default for security".to_string(),
            ],
            Self::FuzzyMatchFailed { file, .. } => vec![
                format!(
                    "The file '{}' may have changed since the review phase",
                    file
                ),
                "Run the review phase again to generate fresh diffs".to_string(),
                "Check if the file has been modified by another process".to_string(),
                "Use 'xchecker resume <id> --phase review' to regenerate fixups".to_string(),
            ],
        }
    }

    fn category(&self) -> ErrorCategory {
        match self {
            Self::NoFixupMarkersFound | Self::NoValidDiffBlocks => ErrorCategory::Validation,
            Self::InvalidDiffFormat { .. } | Self::DiffParsingFailed { .. } => {
                ErrorCategory::Validation
            }
            Self::AbsolutePath(_) | Self::ParentDirEscape(_) | Self::OutsideRepo(_) => {
                ErrorCategory::Security
            }
            Self::SymlinkNotAllowed(_) | Self::HardlinkNotAllowed(_) => ErrorCategory::Security,
            Self::TargetFileNotFound { .. } | Self::TempCopyFailed { .. } => {
                ErrorCategory::FileSystem
            }
            Self::CanonicalizationError(_) => ErrorCategory::FileSystem,
            Self::GitApplyValidationFailed { .. } | Self::GitApplyExecutionFailed { .. } => {
                ErrorCategory::PhaseExecution
            }
            Self::FuzzyMatchFailed { .. } => ErrorCategory::PhaseExecution,
        }
    }
}

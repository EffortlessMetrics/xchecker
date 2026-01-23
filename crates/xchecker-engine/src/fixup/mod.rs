//! Fixup detection and parsing system with preview/apply modes
//!
//! This module implements the fixup system that detects "FIXUP PLAN:" markers in review output,
//! parses unified diff blocks, and provides preview/apply modes for safe fixup application.
//!
//! # Security
//!
//! All path operations use the sandboxed path types from `crate::paths` to prevent:
//! - Directory traversal attacks via `..` components
//! - Absolute path escapes
//! - Symlink-based escapes (configurable)
//! - Hardlink-based escapes (configurable)
//!
//! The `FixupParser` uses `SandboxRoot` to validate all target paths before any file operations.
//! This ensures that diff application cannot escape the workspace root.

mod apply;
mod match_context;
mod model;
mod parse;
mod paths;
mod report;

pub use crate::error::FixupError;
pub use apply::normalize_line_endings_for_diff;
pub use model::{
    AppliedFile, ChangeSummary, DiffHunk, FixupMode, FixupPreview, FixupResult, UnifiedDiff,
};
pub use parse::FixupParser;
pub use paths::validate_fixup_target;
pub use report::{
    PendingFixupsResult, PendingFixupsStats, pending_fixups_for_spec, pending_fixups_from_handle,
    pending_fixups_result_for_spec, pending_fixups_result_from_handle,
};

#[cfg(test)]
mod tests {
    use super::FixupError;
    use crate::error::UserFriendlyError;
    use std::path::PathBuf;

    #[test]
    fn test_fixup_error_user_friendly() {
        let no_markers_err = FixupError::NoFixupMarkersFound;
        assert!(!no_markers_err.user_message().is_empty());
        assert!(no_markers_err.context().is_some());
        assert!(!no_markers_err.suggestions().is_empty());

        let invalid_diff_err = FixupError::InvalidDiffFormat {
            block_index: 1,
            reason: "missing hunk header".to_string(),
        };
        assert!(invalid_diff_err.user_message().contains("block 1"));
        assert!(invalid_diff_err.context().is_some());
        assert!(!invalid_diff_err.suggestions().is_empty());

        let symlink_err = FixupError::SymlinkNotAllowed(PathBuf::from("test/file.txt"));
        assert!(symlink_err.user_message().contains("--allow-links"));
        assert!(symlink_err.context().is_some());
        let suggestions = symlink_err.suggestions();
        assert!(suggestions.iter().any(|s| s.contains("--allow-links")));

        let abs_path_err = FixupError::AbsolutePath(PathBuf::from("/absolute/path"));
        assert!(abs_path_err.user_message().contains("Absolute paths"));
        assert!(abs_path_err.context().is_some());
        assert!(!abs_path_err.suggestions().is_empty());
    }
}

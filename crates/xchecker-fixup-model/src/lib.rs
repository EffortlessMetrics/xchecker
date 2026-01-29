//! Fixup model types for spec generation workflows
//!
//! This crate provides core types and models for detecting and applying changes
//! to specification artifacts through fixup plans.

use std::collections::HashMap;

/// Fixup execution mode
///
/// Determines whether fixup changes are previewed or applied to files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixupMode {
    /// Preview mode - show what would change without applying
    Preview,
    /// Apply mode - actually apply changes to files
    Apply,
}

impl FixupMode {
    /// Returns the string representation of the mode
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Preview => "preview",
            Self::Apply => "apply",
        }
    }
}

/// A single hunk in a unified diff
///
/// Represents a contiguous block of changes with context lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffHunk {
    /// Starting line number in the original file
    pub start: usize,
    /// Number of lines to remove from the original file
    pub remove_count: usize,
    /// Number of lines to add to the new file
    pub add_count: usize,
    /// Lines to remove (without the '-' prefix)
    pub remove_lines: Vec<String>,
    /// Lines to add (without the '+' prefix)
    pub add_lines: Vec<String>,
    /// Original file line range: (start, count)
    pub old_range: (usize, usize),
    /// New file line range: (start, count)
    pub new_range: (usize, usize),
    /// Full hunk content including header and context
    pub content: String,
}

/// A unified diff for a single file
///
/// Represents a complete diff with all hunks for a target file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnifiedDiff {
    /// Path from the diff header (may include a/ or b/ prefix)
    pub path: String,
    /// Target file path (normalized, without a/ or b/ prefix)
    pub target_file: String,
    /// Full diff content as a string
    pub diff_content: String,
    /// All hunks in this diff
    pub hunks: Vec<DiffHunk>,
}

/// Summary of changes for a single file
///
/// Provides statistics and validation results for a file's changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeSummary {
    /// Number of hunks in the diff
    pub hunk_count: usize,
    /// Number of lines added
    pub lines_added: usize,
    /// Number of lines removed
    pub lines_removed: usize,
    /// Whether validation passed for this file
    pub validation_passed: bool,
    /// Validation messages (errors, warnings, etc.)
    pub validation_messages: Vec<String>,
}

/// Result of applying a single file
///
/// Contains information about a successfully applied file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedFile {
    /// Path to the file that was applied
    pub path: String,
    /// First 8 characters of BLAKE3 hash of new content
    pub blake3_first8: String,
    /// Whether the file was successfully applied
    pub applied: bool,
    /// Any warnings generated during application
    pub warnings: Vec<String>,
}

/// Preview of fixup changes
///
/// Shows what would change without actually applying changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixupPreview {
    /// List of target files in the diffs
    pub target_files: Vec<String>,
    /// Change summary per file
    pub change_summary: HashMap<String, ChangeSummary>,
    /// Any warnings generated during preview
    pub warnings: Vec<String>,
    /// Whether all diffs validated successfully
    pub all_valid: bool,
}

/// Result of applying fixup changes
///
/// Contains information about applied and failed files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixupResult {
    /// Files that were successfully applied
    pub applied_files: Vec<AppliedFile>,
    /// Files that failed to apply
    pub failed_files: Vec<String>,
    /// Any warnings generated during application
    pub warnings: Vec<String>,
    /// Whether 3-way merge was used (legacy git apply mode)
    pub three_way_used: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixup_mode() {
        let preview = FixupMode::Preview;
        let apply = FixupMode::Apply;

        assert_eq!(preview, FixupMode::Preview);
        assert_eq!(apply, FixupMode::Apply);
        assert_ne!(preview, apply);
    }

    #[test]
    fn test_diff_hunk() {
        let hunk = DiffHunk {
            start: 10,
            remove_count: 2,
            add_count: 3,
            remove_lines: vec!["old line 1".to_string(), "old line 2".to_string()],
            add_lines: vec![
                "new line 1".to_string(),
                "new line 2".to_string(),
                "new line 3".to_string(),
            ],
            old_range: (10, 2),
            new_range: (10, 3),
            content:
                "@@ -10,2 +10,3 @@\n-old line 1\n-old line 2\n+new line 1\n+new line 2\n+new line 3"
                    .to_string(),
        };

        assert_eq!(hunk.start, 10);
        assert_eq!(hunk.remove_count, 2);
        assert_eq!(hunk.add_count, 3);
        assert_eq!(hunk.remove_lines.len(), 2);
        assert_eq!(hunk.add_lines.len(), 3);
        assert_eq!(hunk.old_range, (10, 2));
        assert_eq!(hunk.new_range, (10, 3));
    }

    #[test]
    fn test_unified_diff() {
        let hunk = DiffHunk {
            start: 1,
            remove_count: 1,
            add_count: 1,
            remove_lines: vec!["old".to_string()],
            add_lines: vec!["new".to_string()],
            old_range: (1, 1),
            new_range: (1, 1),
            content: "@@ -1,1 +1,1 @@\n-old\n+new".to_string(),
        };

        let diff = UnifiedDiff {
            path: "a/src/main.rs".to_string(),
            target_file: "src/main.rs".to_string(),
            diff_content: "--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,1 +1,1 @@\n-old\n+new"
                .to_string(),
            hunks: vec![hunk],
        };

        assert_eq!(diff.path, "a/src/main.rs");
        assert_eq!(diff.target_file, "src/main.rs");
        assert_eq!(diff.hunks.len(), 1);
    }

    #[test]
    fn test_change_summary() {
        let summary = ChangeSummary {
            hunk_count: 2,
            lines_added: 5,
            lines_removed: 3,
            validation_passed: true,
            validation_messages: vec![],
        };

        assert_eq!(summary.hunk_count, 2);
        assert_eq!(summary.lines_added, 5);
        assert_eq!(summary.lines_removed, 3);
        assert!(summary.validation_passed);
    }

    #[test]
    fn test_applied_file() {
        let applied = AppliedFile {
            path: "src/main.rs".to_string(),
            blake3_first8: "a1b2c3d4".to_string(),
            applied: true,
            warnings: vec![],
        };

        assert_eq!(applied.path, "src/main.rs");
        assert_eq!(applied.blake3_first8, "a1b2c3d4");
        assert!(applied.applied);
    }

    #[test]
    fn test_fixup_preview() {
        let mut summary = HashMap::new();
        summary.insert(
            "src/main.rs".to_string(),
            ChangeSummary {
                hunk_count: 1,
                lines_added: 2,
                lines_removed: 1,
                validation_passed: true,
                validation_messages: vec![],
            },
        );

        let preview = FixupPreview {
            target_files: vec!["src/main.rs".to_string()],
            change_summary: summary,
            warnings: vec![],
            all_valid: true,
        };

        assert_eq!(preview.target_files.len(), 1);
        assert!(preview.all_valid);
        assert!(preview.change_summary.contains_key("src/main.rs"));
    }

    #[test]
    fn test_fixup_result() {
        let result = FixupResult {
            applied_files: vec![AppliedFile {
                path: "src/main.rs".to_string(),
                blake3_first8: "a1b2c3d4".to_string(),
                applied: true,
                warnings: vec![],
            }],
            failed_files: vec![],
            warnings: vec![],
            three_way_used: false,
        };

        assert_eq!(result.applied_files.len(), 1);
        assert_eq!(result.failed_files.len(), 0);
        assert!(!result.three_way_used);
    }
}

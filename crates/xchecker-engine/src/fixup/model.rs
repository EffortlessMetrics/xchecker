use std::collections::HashMap;

/// Mode for fixup operations - preview or apply
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixupMode {
    /// Preview mode: parse & validate unified diffs, list targets, no writes
    Preview,
    /// Apply mode: run git apply --check first, then apply changes
    Apply,
}

impl FixupMode {
    #[must_use]
    #[allow(dead_code)] // String conversion for serialization/display
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Preview => "preview",
            Self::Apply => "apply",
        }
    }
}

/// Represents a unified diff for a single file
#[derive(Debug, Clone)]
pub struct UnifiedDiff {
    /// Target file path relative to project root
    pub target_file: String,
    /// The complete diff content including headers
    pub diff_content: String,
    /// Individual hunks in the diff
    pub hunks: Vec<DiffHunk>,
}

/// Represents a single hunk within a unified diff
#[derive(Debug, Clone)]
pub struct DiffHunk {
    /// Original file line range (start, count)
    pub old_range: (usize, usize),
    /// New file line range (start, count)
    #[allow(dead_code)] // Diff analysis metadata
    pub new_range: (usize, usize),
    /// The hunk content including context and changes
    pub content: String,
}

/// Result of fixup preview operation
#[derive(Debug, Clone)]
pub struct FixupPreview {
    /// List of target files that would be modified
    pub target_files: Vec<String>,
    /// Summary of changes per file
    pub change_summary: HashMap<String, ChangeSummary>,
    /// Warnings encountered during parsing/validation
    pub warnings: Vec<String>,
    /// Whether all diffs passed validation
    pub all_valid: bool,
}

/// Summary of changes for a single file
#[derive(Debug, Clone)]
pub struct ChangeSummary {
    /// Number of hunks in the diff
    #[allow(dead_code)] // Diff reporting metadata
    pub hunk_count: usize,
    /// Estimated lines added
    pub lines_added: usize,
    /// Estimated lines removed
    pub lines_removed: usize,
    /// Whether the diff passed git apply --check validation
    pub validation_passed: bool,
    /// Validation warnings or errors
    #[allow(dead_code)] // Future diff reporting
    pub validation_messages: Vec<String>,
}

/// Result of fixup apply operation
#[derive(Debug, Clone)]
pub struct FixupResult {
    /// Files that were successfully modified with their details
    pub applied_files: Vec<AppliedFile>,
    /// Files that failed to apply
    pub failed_files: Vec<String>,
    /// Warnings encountered during application
    pub warnings: Vec<String>,
    /// Whether 3-way merge was used for any files
    pub three_way_used: bool,
}

/// Details of a successfully applied file
#[derive(Debug, Clone)]
pub struct AppliedFile {
    /// Path to the file that was modified
    pub path: String,
    /// BLAKE3 hash (first 8 chars) of the new content
    pub blake3_first8: String,
    /// Whether the file was actually applied (always true for `AppliedFile`)
    pub applied: bool,
    /// Warnings specific to this file (e.g., permission preservation issues)
    pub warnings: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixup_preview_structure() {
        let diff = UnifiedDiff {
            target_file: "test.txt".to_string(),
            diff_content: "--- a/test.txt\n+++ b/test.txt\n@@ -1,1 +1,2 @@\n line1\n+line2"
                .to_string(),
            hunks: vec![DiffHunk {
                old_range: (1, 1),
                new_range: (1, 2),
                content: "@@ -1,1 +1,2 @@\n line1\n+line2".to_string(),
            }],
        };

        let summary = ChangeSummary {
            hunk_count: diff.hunks.len(),
            lines_added: 1,
            lines_removed: 0,
            validation_passed: true,
            validation_messages: vec![],
        };

        assert_eq!(summary.hunk_count, 1);
        assert_eq!(summary.lines_added, 1);
        assert_eq!(summary.lines_removed, 0);
        assert!(summary.validation_passed);
    }

    #[test]
    fn test_fixup_result_structure() {
        let result = FixupResult {
            applied_files: vec![
                AppliedFile {
                    path: "file1.txt".to_string(),
                    blake3_first8: "abcd1234".to_string(),
                    applied: true,
                    warnings: Vec::new(),
                },
                AppliedFile {
                    path: "file2.txt".to_string(),
                    blake3_first8: "efgh5678".to_string(),
                    applied: true,
                    warnings: vec!["Permission warning".to_string()],
                },
            ],
            failed_files: vec!["file3.txt".to_string()],
            warnings: vec!["Warning: file3.txt had conflicts".to_string()],
            three_way_used: false,
        };

        assert_eq!(result.applied_files.len(), 2);
        assert_eq!(result.failed_files.len(), 1);
        assert_eq!(result.warnings.len(), 1);
        assert!(!result.three_way_used);
        assert_eq!(result.applied_files[0].blake3_first8, "abcd1234");
        assert_eq!(result.applied_files[1].warnings.len(), 1);
    }

    #[test]
    fn test_unified_diff_structure() {
        let hunk = DiffHunk {
            old_range: (1, 3),
            new_range: (1, 4),
            content: "@@ -1,3 +1,4 @@\n line1\n+line2\n line3".to_string(),
        };

        let diff = UnifiedDiff {
            target_file: "src/main.rs".to_string(),
            diff_content:
                "--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,3 +1,4 @@\n line1\n+line2\n line3"
                    .to_string(),
            hunks: vec![hunk],
        };

        assert_eq!(diff.target_file, "src/main.rs");
        assert_eq!(diff.hunks.len(), 1);
        assert_eq!(diff.hunks[0].old_range, (1, 3));
        assert_eq!(diff.hunks[0].new_range, (1, 4));
    }
}

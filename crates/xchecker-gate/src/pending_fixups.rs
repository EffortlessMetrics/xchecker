//! Pending fixups detection for gate evaluation
//!
//! This module provides functionality to detect pending fixups in review artifacts.

use anyhow::Result;
use std::path::Path;

use crate::types::PendingFixupsStats;

/// Get pending fixups for a spec
///
/// Analyzes the review artifact to determine if there are pending fixups.
pub fn pending_fixups_for_spec(base_path: &Path) -> PendingFixupsStats {
    let review_md_path = base_path.join("artifacts").join("30-review.md");

    if !review_md_path.exists() {
        return PendingFixupsStats::default();
    }

    // Read the review content
    let review_content = match std::fs::read_to_string(&review_md_path) {
        Ok(content) => content,
        Err(_) => return PendingFixupsStats::default(),
    };

    // Check for fixup markers
    let has_markers = has_fixup_markers(&review_content);

    if !has_markers {
        return PendingFixupsStats::default();
    }

    // Try to parse diffs to count targets
    match parse_fixup_targets(&review_content) {
        Ok(stats) => stats,
        Err(_) => {
            // Failed to parse, but we know there are markers
            PendingFixupsStats {
                targets: 1,
                est_added: 0,
                est_removed: 0,
            }
        }
    }
}

/// Check if review content contains fixup markers
fn has_fixup_markers(content: &str) -> bool {
    // Look for common fixup marker patterns
    content.contains("```diff")
        || content.contains("<<<<<<<")
        || content.contains("=======")
        || content.contains(">>>>>>>")
}

/// Parse fixup targets from review content
///
/// Attempts to extract diff blocks and count unique target files.
fn parse_fixup_targets(content: &str) -> Result<PendingFixupsStats> {
    let mut targets = std::collections::HashSet::new();
    let mut total_added = 0;
    let mut total_removed = 0;

    // Simple diff parser - look for file headers in diff format
    for line in content.lines() {
        // Look for diff file headers (e.g., "--- a/file.rs" or "+++ b/file.rs")
        if line.starts_with("--- ") || line.starts_with("+++ ") {
            // Extract filename
            let filename = extract_filename_from_diff_line(line);
            if let Some(name) = filename {
                targets.insert(name);
            }
        }

        // Count added/removed lines
        if line.starts_with('+') && !line.starts_with("+++") {
            total_added += 1;
        } else if line.starts_with('-') && !line.starts_with("---") {
            total_removed += 1;
        }
    }

    Ok(PendingFixupsStats {
        targets: targets.len() as u32,
        est_added: total_added as u32,
        est_removed: total_removed as u32,
    })
}

/// Extract filename from a diff header line
fn extract_filename_from_diff_line(line: &str) -> Option<String> {
    // Only process lines that start with "--- " or "+++ "
    if !(line.starts_with("--- ") || line.starts_with("+++ ")) {
        return None;
    }

    // Handle "--- a/path/to/file" or "+++ b/path/to/file"
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 2 {
        let path_part = parts[1];
        // Remove "a/" or "b/" prefix
        let filename = path_part
            .strip_prefix("a/")
            .or_else(|| path_part.strip_prefix("b/"))
            .unwrap_or(path_part);

        Some(filename.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_has_fixup_markers() {
        assert!(!has_fixup_markers("No markers here"));
        assert!(has_fixup_markers(
            "```diff\n--- a/file.rs\n+++ b/file.rs\n```"
        ));
        assert!(has_fixup_markers("<<<<<<< HEAD\n=======\n>>>>>>> branch"));
    }

    #[test]
    fn test_extract_filename() {
        assert_eq!(
            extract_filename_from_diff_line("--- a/src/main.rs"),
            Some("src/main.rs".to_string())
        );
        assert_eq!(
            extract_filename_from_diff_line("+++ b/src/main.rs"),
            Some("src/main.rs".to_string())
        );
        assert_eq!(extract_filename_from_diff_line("invalid line"), None);
    }

    #[test]
    fn test_pending_fixups_no_review() {
        let temp = tempfile::tempdir().unwrap();
        let base_path = temp.path().join("spec");
        std::fs::create_dir_all(base_path.join("artifacts")).unwrap();

        let stats = pending_fixups_for_spec(&base_path);
        assert_eq!(stats.targets, 0);
        assert_eq!(stats.est_added, 0);
        assert_eq!(stats.est_removed, 0);
    }

    #[test]
    fn test_pending_fixups_with_markers() {
        let temp = tempfile::tempdir().unwrap();
        let base_path = temp.path().join("spec");
        std::fs::create_dir_all(base_path.join("artifacts")).unwrap();

        let review_content = r#"```diff
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,4 @@
 fn main() {
     println!("Hello");
+    println!("World");
 }
```"#;
        std::fs::write(
            base_path.join("artifacts").join("30-review.md"),
            review_content,
        )
        .unwrap();

        let stats = pending_fixups_for_spec(&base_path);
        assert_eq!(stats.targets, 1);
        assert!(stats.est_added > 0);
    }
}

use std::path::PathBuf;

use regex::Regex;

use crate::error::FixupError;
use crate::paths::{SandboxConfig, SandboxError, SandboxPath, SandboxRoot};

use super::model::{DiffHunk, FixupMode, UnifiedDiff};

/// Main fixup parser that handles detection and parsing of fixup plans
///
/// # Security
///
/// `FixupParser` uses `SandboxRoot` to validate all target paths before any file operations.
/// This ensures that:
/// - Paths cannot escape the workspace root via `..` traversal
/// - Absolute paths are rejected
/// - Symlinks are rejected by default (configurable via `SandboxConfig`)
/// - Hardlinks are rejected by default (configurable via `SandboxConfig`)
///
/// All path validation happens through `SandboxRoot::join()` which provides
/// comprehensive security checks before any file I/O.
pub struct FixupParser {
    /// Operating mode (preview or apply)
    pub mode: FixupMode,
    /// Sandboxed root directory for resolving and validating relative paths
    sandbox_root: SandboxRoot,
}

impl FixupParser {
    /// Create a new fixup parser with a sandboxed root directory.
    ///
    /// # Arguments
    ///
    /// * `mode` - The operating mode (preview or apply)
    /// * `base_dir` - The base directory to use as the sandbox root
    ///
    /// # Errors
    ///
    /// Returns an error if the base directory cannot be used as a sandbox root
    /// (e.g., doesn't exist, isn't a directory, or can't be canonicalized).
    pub fn new(mode: FixupMode, base_dir: PathBuf) -> Result<Self, FixupError> {
        let sandbox_root = SandboxRoot::new(&base_dir, SandboxConfig::default()).map_err(|e| {
            FixupError::CanonicalizationError(format!("Failed to create sandbox root: {e}"))
        })?;
        Ok(Self { mode, sandbox_root })
    }

    /// Create a new fixup parser with custom sandbox configuration.
    ///
    /// # Arguments
    ///
    /// * `mode` - The operating mode (preview or apply)
    /// * `base_dir` - The base directory to use as the sandbox root
    /// * `config` - Custom sandbox configuration (e.g., to allow symlinks)
    ///
    /// # Errors
    ///
    /// Returns an error if the base directory cannot be used as a sandbox root.
    pub fn with_config(
        mode: FixupMode,
        base_dir: PathBuf,
        config: SandboxConfig,
    ) -> Result<Self, FixupError> {
        let sandbox_root = SandboxRoot::new(&base_dir, config).map_err(|e| {
            FixupError::CanonicalizationError(format!("Failed to create sandbox root: {e}"))
        })?;
        Ok(Self { mode, sandbox_root })
    }

    /// Get the sandbox root path.
    #[must_use]
    pub fn base_dir(&self) -> &std::path::Path {
        self.sandbox_root.as_path()
    }

    /// Validate and resolve a target path within the sandbox.
    ///
    /// This method uses `SandboxRoot::join()` to validate the path, ensuring:
    /// - The path is relative (not absolute)
    /// - The path doesn't contain `..` traversal components
    /// - The path doesn't escape the sandbox root
    /// - The path isn't a symlink (unless configured to allow)
    /// - The path isn't a hardlink (unless configured to allow)
    ///
    /// # Arguments
    ///
    /// * `target_file` - The relative path to validate
    ///
    /// # Returns
    ///
    /// A `SandboxPath` that is guaranteed to be within the sandbox root.
    pub(super) fn validate_target_path(
        &self,
        target_file: &str,
    ) -> Result<SandboxPath, FixupError> {
        self.sandbox_root.join(target_file).map_err(|e| match e {
            SandboxError::AbsolutePath { path } => FixupError::AbsolutePath(PathBuf::from(path)),
            SandboxError::ParentTraversal { path } => {
                FixupError::ParentDirEscape(PathBuf::from(path))
            }
            SandboxError::EscapeAttempt { path, .. } => {
                FixupError::OutsideRepo(PathBuf::from(path))
            }
            SandboxError::SymlinkNotAllowed { path } => {
                FixupError::SymlinkNotAllowed(PathBuf::from(path))
            }
            SandboxError::HardlinkNotAllowed { path } => {
                FixupError::HardlinkNotAllowed(PathBuf::from(path))
            }
            SandboxError::RootNotFound { path } | SandboxError::RootNotDirectory { path } => {
                FixupError::CanonicalizationError(format!("Invalid sandbox root: {path}"))
            }
            SandboxError::RootCanonicalizationFailed { path, reason }
            | SandboxError::PathCanonicalizationFailed { path, reason } => {
                FixupError::CanonicalizationError(format!(
                    "Failed to canonicalize {path}: {reason}"
                ))
            }
        })
    }

    /// Detect if the review output contains fixup markers
    #[must_use]
    pub fn has_fixup_markers(&self, content: &str) -> bool {
        self.detect_fixup_markers(content).is_some()
    }

    /// Detect fixup markers in review output
    /// Returns the content after the first marker if found
    #[must_use]
    pub fn detect_fixup_markers(&self, content: &str) -> Option<String> {
        // Look for "FIXUP PLAN:" or "needs fixups" markers
        let fixup_plan_regex = Regex::new(r"(?i)FIXUP PLAN:").unwrap();
        let needs_fixups_regex = Regex::new(r"(?i)needs fixups").unwrap();

        if let Some(mat) = fixup_plan_regex.find(content) {
            return Some(content[mat.end()..].to_string());
        }

        if let Some(mat) = needs_fixups_regex.find(content) {
            return Some(content[mat.end()..].to_string());
        }

        None
    }

    /// Parse unified diff blocks from fixup content
    pub fn parse_diffs(&self, content: &str) -> Result<Vec<UnifiedDiff>, FixupError> {
        let fixup_content = self
            .detect_fixup_markers(content)
            .ok_or(FixupError::NoFixupMarkersFound)?;

        let diffs = self.extract_diff_blocks(&fixup_content)?;

        if diffs.is_empty() {
            return Err(FixupError::NoValidDiffBlocks);
        }

        Ok(diffs)
    }

    /// Extract diff blocks from fenced code blocks
    fn extract_diff_blocks(&self, content: &str) -> Result<Vec<UnifiedDiff>, FixupError> {
        let mut diffs = Vec::new();

        // Regex to match fenced diff blocks: ```diff ... ```
        // Use (?s) flag to make . match newlines
        let diff_block_regex = Regex::new(r"(?s)```diff\n(.*?)\n```").unwrap();

        for (block_index, captures) in diff_block_regex.captures_iter(content).enumerate() {
            let diff_content = captures
                .get(1)
                .ok_or_else(|| FixupError::InvalidDiffFormat {
                    block_index,
                    reason: "No diff content found in block".to_string(),
                })?
                .as_str();

            match self.parse_unified_diff(diff_content, block_index) {
                Ok(diff) => diffs.push(diff),
                Err(e) => {
                    // Log the error but continue processing other blocks
                    tracing::warn!("Failed to parse diff block {block_index}: {e}");
                }
            }
        }

        Ok(diffs)
    }

    /// Parse a single unified diff block
    fn parse_unified_diff(
        &self,
        diff_content: &str,
        block_index: usize,
    ) -> Result<UnifiedDiff, FixupError> {
        let lines: Vec<&str> = diff_content.lines().collect();

        if lines.is_empty() {
            return Err(FixupError::InvalidDiffFormat {
                block_index,
                reason: "Empty diff block".to_string(),
            });
        }

        // Find the --- and +++ headers
        let mut old_file = None;
        let mut new_file = None;
        let mut header_end = 0;

        for (i, line) in lines.iter().enumerate() {
            if let Some(rest) = line.strip_prefix("--- ") {
                old_file = Some(rest.trim());
            } else if let Some(rest) = line.strip_prefix("+++ ") {
                new_file = Some(rest.trim());
                header_end = i + 1;
                break;
            }
        }

        let target_file = new_file
            .or(old_file)
            .ok_or_else(|| FixupError::InvalidDiffFormat {
                block_index,
                reason: "No --- or +++ headers found".to_string(),
            })?;

        // Remove a/ and b/ prefixes if present (common in git diffs)
        let target_file = if target_file.starts_with("a/") || target_file.starts_with("b/") {
            &target_file[2..]
        } else {
            target_file
        };

        // Parse hunks
        let hunks = self.parse_hunks(&lines[header_end..], block_index)?;

        Ok(UnifiedDiff {
            target_file: target_file.to_string(),
            diff_content: diff_content.to_string(),
            hunks,
        })
    }

    /// Parse hunks from diff lines
    fn parse_hunks(&self, lines: &[&str], block_index: usize) -> Result<Vec<DiffHunk>, FixupError> {
        let mut hunks = Vec::new();
        let mut current_hunk_lines = Vec::new();
        let mut current_hunk_header = None;

        // Regex to match hunk headers: @@ -old_start,old_count +new_start,new_count @@
        let hunk_header_regex = Regex::new(r"^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@").unwrap();

        for line in lines {
            if let Some(captures) = hunk_header_regex.captures(line) {
                // Save previous hunk if exists
                if let Some((old_range, new_range)) = current_hunk_header {
                    hunks.push(DiffHunk {
                        old_range,
                        new_range,
                        content: current_hunk_lines.join("\n"),
                    });
                }

                // Parse new hunk header
                let old_start: usize = captures.get(1).unwrap().as_str().parse().map_err(|_| {
                    FixupError::InvalidDiffFormat {
                        block_index,
                        reason: "Invalid old start line number".to_string(),
                    }
                })?;

                let old_count: usize = captures
                    .get(2)
                    .map_or(1, |m| m.as_str().parse().unwrap_or(1));

                let new_start: usize = captures.get(3).unwrap().as_str().parse().map_err(|_| {
                    FixupError::InvalidDiffFormat {
                        block_index,
                        reason: "Invalid new start line number".to_string(),
                    }
                })?;

                let new_count: usize = captures
                    .get(4)
                    .map_or(1, |m| m.as_str().parse().unwrap_or(1));

                current_hunk_header = Some(((old_start, old_count), (new_start, new_count)));
                current_hunk_lines = vec![(*line).to_string()];
            } else {
                // Add line to current hunk
                current_hunk_lines.push((*line).to_string());
            }
        }

        // Save last hunk if exists
        if let Some((old_range, new_range)) = current_hunk_header {
            hunks.push(DiffHunk {
                old_range,
                new_range,
                content: current_hunk_lines.join("\n"),
            });
        }

        Ok(hunks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_detect_fixup_markers() {
        let temp_dir = TempDir::new().unwrap();
        let parser = FixupParser::new(FixupMode::Preview, temp_dir.path().to_path_buf()).unwrap();

        // Test FIXUP PLAN: marker
        let content1 = "Some review content\nFIXUP PLAN:\nHere are the fixes needed...";
        assert!(parser.has_fixup_markers(content1));

        // Test needs fixups marker
        let content2 = "The review shows that this needs fixups in several areas...";
        assert!(parser.has_fixup_markers(content2));

        // Test no markers
        let content3 = "This is a clean review with no issues found.";
        assert!(!parser.has_fixup_markers(content3));
    }

    #[test]
    fn test_parse_simple_diff() {
        let temp_dir = TempDir::new().unwrap();
        let parser = FixupParser::new(FixupMode::Preview, temp_dir.path().to_path_buf()).unwrap();

        let content = r#"
FIXUP PLAN:
The following changes are needed:

```diff
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,4 @@
 fn main() {
+    println!("Hello, world!");
     // TODO: implement
 }
```
"#;

        let diffs = parser.parse_diffs(content).unwrap();
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].target_file, "src/main.rs");
        assert_eq!(diffs[0].hunks.len(), 1);
    }

    #[test]
    fn test_parse_multiple_hunks() {
        let temp_dir = TempDir::new().unwrap();
        let parser = FixupParser::new(FixupMode::Preview, temp_dir.path().to_path_buf()).unwrap();

        let content = r#"
FIXUP PLAN:
Multiple changes needed:

```diff
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,3 +1,4 @@
 pub fn foo() {
+    println!("Starting foo");
     // implementation
 }
@@ -10,2 +11,3 @@
 pub fn bar() {
+    println!("Starting bar");
     // implementation
 }
```
"#;

        let diffs = parser.parse_diffs(content).unwrap();
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].target_file, "src/lib.rs");
        assert_eq!(diffs[0].hunks.len(), 2);

        // Verify first hunk
        let hunk1 = &diffs[0].hunks[0];
        assert_eq!(hunk1.old_range, (1, 3));
        assert_eq!(hunk1.new_range, (1, 4));

        // Verify second hunk
        let hunk2 = &diffs[0].hunks[1];
        assert_eq!(hunk2.old_range, (10, 2));
        assert_eq!(hunk2.new_range, (11, 3));
    }

    #[test]
    fn test_parse_multiple_diffs() {
        let temp_dir = TempDir::new().unwrap();
        let parser = FixupParser::new(FixupMode::Preview, temp_dir.path().to_path_buf()).unwrap();

        let content = r#"
FIXUP PLAN:
Changes needed in multiple files:

```diff
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,2 +1,3 @@
 fn main() {
+    println!("Hello");
 }
```

```diff
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,2 +1,3 @@
 pub fn test() {
+    println!("Test");
 }
```
"#;

        let diffs = parser.parse_diffs(content).unwrap();
        assert_eq!(diffs.len(), 2);
        assert_eq!(diffs[0].target_file, "src/main.rs");
        assert_eq!(diffs[1].target_file, "src/lib.rs");
    }

    #[test]
    fn test_parse_diff_without_git_prefix() {
        let temp_dir = TempDir::new().unwrap();
        let parser = FixupParser::new(FixupMode::Preview, temp_dir.path().to_path_buf()).unwrap();

        let content = r#"
FIXUP PLAN:

```diff
--- src/main.rs
+++ src/main.rs
@@ -1,2 +1,3 @@
 fn main() {
+    println!("Hello");
 }
```
"#;

        let diffs = parser.parse_diffs(content).unwrap();
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].target_file, "src/main.rs");
    }

    #[test]
    fn test_parse_diff_with_git_prefix() {
        let temp_dir = TempDir::new().unwrap();
        let parser = FixupParser::new(FixupMode::Preview, temp_dir.path().to_path_buf()).unwrap();

        let content = r#"
FIXUP PLAN:

```diff
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,2 +1,3 @@
 fn main() {
+    println!("Hello");
 }
```
"#;

        let diffs = parser.parse_diffs(content).unwrap();
        assert_eq!(diffs.len(), 1);
        // Should strip a/ and b/ prefixes
        assert_eq!(diffs[0].target_file, "src/main.rs");
    }

    #[test]
    fn test_hunk_range_parsing() {
        let temp_dir = TempDir::new().unwrap();
        let parser = FixupParser::new(FixupMode::Preview, temp_dir.path().to_path_buf()).unwrap();

        let content = r"
FIXUP PLAN:

```diff
--- a/test.txt
+++ b/test.txt
@@ -5,3 +5,4 @@
 line 5
+new line
 line 6
 line 7
@@ -10 +11,2 @@
 line 10
+another new line
```
";

        let diffs = parser.parse_diffs(content).unwrap();
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].hunks.len(), 2);

        // First hunk: @@ -5,3 +5,4 @@
        let hunk1 = &diffs[0].hunks[0];
        assert_eq!(hunk1.old_range, (5, 3));
        assert_eq!(hunk1.new_range, (5, 4));

        // Second hunk: @@ -10 +11,2 @@ (implicit count of 1)
        let hunk2 = &diffs[0].hunks[1];
        assert_eq!(hunk2.old_range, (10, 1));
        assert_eq!(hunk2.new_range, (11, 2));
    }

    #[test]
    fn test_empty_diff_block() {
        let temp_dir = TempDir::new().unwrap();
        let parser = FixupParser::new(FixupMode::Preview, temp_dir.path().to_path_buf()).unwrap();

        let content = r"
FIXUP PLAN:

```diff
```
";

        let result = parser.parse_diffs(content);
        // Should fail with NoValidDiffBlocks since the diff block is empty
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), FixupError::NoValidDiffBlocks));
    }

    #[test]
    fn test_malformed_hunk_header() {
        let temp_dir = TempDir::new().unwrap();
        let parser = FixupParser::new(FixupMode::Preview, temp_dir.path().to_path_buf()).unwrap();

        let content = r"
FIXUP PLAN:

```diff
--- a/test.txt
+++ b/test.txt
@@ invalid hunk header @@
 some content
```
";

        let diffs = parser.parse_diffs(content).unwrap();
        // Parser should handle malformed hunks gracefully
        // The diff should be parsed but with 0 hunks since the header is invalid
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].hunks.len(), 0);
    }

    #[test]
    fn test_no_fixup_markers() {
        let temp_dir = TempDir::new().unwrap();
        let parser = FixupParser::new(FixupMode::Preview, temp_dir.path().to_path_buf()).unwrap();

        let content = "This is just regular review content without any fixup markers.";

        let result = parser.parse_diffs(content);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            FixupError::NoFixupMarkersFound
        ));
    }

    #[test]
    fn test_case_insensitive_fixup_markers() {
        let temp_dir = TempDir::new().unwrap();
        let parser = FixupParser::new(FixupMode::Preview, temp_dir.path().to_path_buf()).unwrap();

        // Test various case variations
        let content1 = "fixup plan:\nSome content";
        assert!(parser.has_fixup_markers(content1));

        let content2 = "FIXUP PLAN:\nSome content";
        assert!(parser.has_fixup_markers(content2));

        let content3 = "Fixup Plan:\nSome content";
        assert!(parser.has_fixup_markers(content3));

        let content4 = "needs FIXUPS in several places";
        assert!(parser.has_fixup_markers(content4));
    }
}

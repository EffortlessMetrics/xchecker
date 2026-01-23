use std::collections::HashMap;

use camino::Utf8Path;
use tempfile::TempDir;

use crate::atomic_write::write_file_atomic;
use crate::error::FixupError;
use crate::runner::CommandSpec;

use super::model::{AppliedFile, ChangeSummary, FixupMode, FixupPreview, FixupResult, UnifiedDiff};
use super::parse::FixupParser;

impl FixupParser {
    /// Preview changes without applying them
    ///
    /// Line endings are normalized before calculating diff statistics (FR-FIX-010)
    ///
    /// # Security
    ///
    /// All target paths are validated through `SandboxRoot::join()` to ensure:
    /// - Paths cannot escape the workspace root via `..` traversal
    /// - Absolute paths are rejected
    /// - Symlinks are rejected by default (configurable via `SandboxConfig`)
    /// - Hardlinks are rejected by default (configurable via `SandboxConfig`)
    pub fn preview_changes(&self, diffs: &[UnifiedDiff]) -> Result<FixupPreview, FixupError> {
        let mut target_files = Vec::new();
        let mut change_summary = HashMap::new();
        let mut warnings = Vec::new();
        let mut all_valid = true;

        for diff in diffs {
            target_files.push(diff.target_file.clone());

            let mut validation_messages = Vec::new();
            let mut validation_passed = true;

            // 1. Path validation using SandboxRoot::join() via validate_target_path
            // This replaces the legacy validate_fixup_target function with the
            // centralized sandbox validation that provides comprehensive security checks.
            if let Err(e) = self.validate_target_path(&diff.target_file) {
                validation_passed = false;
                all_valid = false;
                let msg = format!("Invalid target path: {e}");
                validation_messages.push(msg.clone());
                warnings.push(format!("{}: {}", diff.target_file, msg));
            }

            // 2. Validate diff against temp copy (only if path validation passed)
            if validation_passed {
                match self.validate_diff_with_git_apply(diff) {
                    Ok(messages) => {
                        validation_messages.extend(messages);
                    }
                    Err(e) => {
                        validation_passed = false;
                        all_valid = false;
                        warnings.push(format!("Validation failed for {}: {}", diff.target_file, e));
                        validation_messages.push(e.to_string());
                    }
                }
            }

            // Calculate change statistics (with line ending normalization - FR-FIX-010)
            let (lines_added, lines_removed) = self.calculate_change_stats(diff);

            change_summary.insert(
                diff.target_file.clone(),
                ChangeSummary {
                    hunk_count: diff.hunks.len(),
                    lines_added,
                    lines_removed,
                    validation_passed,
                    validation_messages,
                },
            );
        }

        Ok(FixupPreview {
            target_files,
            change_summary,
            warnings,
            all_valid,
        })
    }

    /// Apply changes to files using atomic writes (FR-FIX-005, FR-FIX-006, FR-FIX-008)
    ///
    /// This method implements the atomic write pattern:
    /// 1. Validate target path using SandboxPath (security check)
    /// 2. Write to .tmp file with fsync
    /// 3. Create .bak backup if file exists
    /// 4. Atomic rename with Windows retry
    /// 5. Preserve file permissions (Unix) or attributes (Windows)
    /// 6. Record warnings if permission preservation fails
    ///
    /// # Security
    ///
    /// All target paths are validated through `SandboxRoot::join()` to ensure:
    /// - Paths cannot escape the workspace root via `..` traversal
    /// - Absolute paths are rejected
    /// - Symlinks are rejected by default (configurable via `SandboxConfig`)
    /// - Hardlinks are rejected by default (configurable via `SandboxConfig`)
    pub fn apply_changes(&self, diffs: &[UnifiedDiff]) -> Result<FixupResult, FixupError> {
        if self.mode != FixupMode::Apply {
            return Err(FixupError::DiffParsingFailed {
                reason: "Cannot apply changes in preview mode".to_string(),
            });
        }

        let mut applied_files = Vec::new();
        let mut failed_files = Vec::new();
        let mut warnings = Vec::new();

        for diff in diffs {
            // Validate target path before applying using SandboxPath (hard error in apply mode)
            // This replaces the legacy validate_fixup_target function with the
            // centralized sandbox validation that provides comprehensive security checks.
            if let Err(e) = self.validate_target_path(&diff.target_file) {
                failed_files.push(diff.target_file.clone());
                warnings.push(format!(
                    "Path validation failed for {}: {}",
                    diff.target_file, e
                ));
                continue;
            }

            match self.apply_single_diff_atomic(diff) {
                Ok(applied_file) => {
                    // Collect file-specific warnings
                    for warning in &applied_file.warnings {
                        warnings.push(format!("{}: {}", diff.target_file, warning));
                    }
                    applied_files.push(applied_file);
                }
                Err(e) => {
                    failed_files.push(diff.target_file.clone());
                    warnings.push(format!("Failed to apply {}: {}", diff.target_file, e));
                }
            }
        }

        Ok(FixupResult {
            applied_files,
            failed_files,
            warnings,
            three_way_used: false, // Not using git apply anymore
        })
    }

    /// Apply a single diff using atomic writes with backup and permission preservation
    ///
    /// This method implements FR-FIX-005, FR-FIX-006, FR-FIX-007, and FR-FIX-008:
    /// - FR-FIX-005: Atomic write with temp file + fsync + rename
    /// - FR-FIX-006: Create .bak backup and preserve file permissions
    /// - FR-FIX-007: Cross-filesystem fallback (copy+fsync+replace)
    /// - FR-FIX-008: Record warnings for permission preservation failures
    ///
    /// # Security
    ///
    /// The target path is validated through `SandboxRoot::join()` before any file operations.
    fn apply_single_diff_atomic(&self, diff: &UnifiedDiff) -> Result<AppliedFile, FixupError> {
        use std::fs;

        // Validate and get the sandboxed target path
        // This ensures the path is within the sandbox root and passes all security checks
        let sandbox_path = self.validate_target_path(&diff.target_file)?;
        let target_path = sandbox_path.as_path();

        if !target_path.exists() {
            return Err(FixupError::TargetFileNotFound {
                path: diff.target_file.clone(),
            });
        }

        let mut file_warnings = Vec::new();

        // Read original content with CRLF tolerance (FR-FS-005)
        // Line endings will be normalized during diff application
        let original_content =
            fs::read_to_string(target_path).map_err(|e| FixupError::TempCopyFailed {
                file: diff.target_file.clone(),
                reason: format!("Failed to read original file: {e}"),
            })?;

        // Get original file permissions/attributes before modification
        let original_metadata =
            fs::metadata(target_path).map_err(|e| FixupError::TempCopyFailed {
                file: diff.target_file.clone(),
                reason: format!("Failed to get file metadata: {e}"),
            })?;

        #[cfg(unix)]
        let original_permissions = {
            use std::os::unix::fs::PermissionsExt;
            Some(original_metadata.permissions().mode())
        };

        #[cfg(windows)]
        let original_readonly = original_metadata.permissions().readonly();

        // Apply the diff to get new content
        let new_content = self.apply_diff_to_content(&original_content, diff)?;

        // Compute BLAKE3 hash of new content
        let blake3_hash = self.compute_blake3_hash(&new_content);
        let blake3_first8 = blake3_hash[..8].to_string();

        // Create .bak backup (FR-FIX-006)
        let backup_path = target_path.with_extension("bak");
        fs::copy(target_path, &backup_path).map_err(|e| FixupError::TempCopyFailed {
            file: diff.target_file.clone(),
            reason: format!("Failed to create .bak backup: {e}"),
        })?;

        // Convert PathBuf to Utf8Path for atomic_write
        let target_utf8_path =
            Utf8Path::from_path(target_path).ok_or_else(|| FixupError::TempCopyFailed {
                file: diff.target_file.clone(),
                reason: "Path contains invalid UTF-8".to_string(),
            })?;

        // Use centralized atomic write with cross-filesystem fallback (FR-FIX-005, FR-FIX-007)
        let write_result = write_file_atomic(target_utf8_path, &new_content).map_err(|e| {
            FixupError::TempCopyFailed {
                file: diff.target_file.clone(),
                reason: format!("Failed to write file atomically: {e}"),
            }
        })?;

        // Collect warnings from atomic write (FR-FIX-007, FR-FIX-008)
        file_warnings.extend(write_result.warnings);

        // Preserve file permissions (FR-FIX-006, FR-FIX-008)
        #[cfg(unix)]
        {
            if let Some(mode) = original_permissions {
                use std::os::unix::fs::PermissionsExt;
                let permissions = fs::Permissions::from_mode(mode);
                if let Err(e) = fs::set_permissions(target_path, permissions) {
                    file_warnings.push(format!("Failed to preserve file permissions: {}", e));
                }
            }
        }

        #[cfg(windows)]
        {
            if let Ok(metadata) = fs::metadata(target_path) {
                let mut permissions = metadata.permissions();
                permissions.set_readonly(original_readonly);
                if let Err(e) = fs::set_permissions(target_path, permissions) {
                    file_warnings.push(format!("Failed to preserve file attributes: {e}"));
                }
            }
        }

        Ok(AppliedFile {
            path: diff.target_file.clone(),
            blake3_first8,
            applied: true,
            warnings: file_warnings,
        })
    }

    /// Apply a diff to content string with fuzzy matching support
    ///
    /// Line endings are normalized to LF before applying the diff (FR-FIX-010).
    /// If the hunk's expected line position doesn't match, we search within a
    /// window (+/- FUZZY_SEARCH_WINDOW lines) to find the best matching context.
    fn apply_diff_to_content(
        &self,
        content: &str,
        diff: &UnifiedDiff,
    ) -> Result<String, FixupError> {
        const FUZZY_SEARCH_WINDOW: usize = 50;
        const MIN_CONTEXT_MATCH_RATIO: f64 = 0.7;

        // Normalize line endings before applying diff (FR-FIX-010, FR-FS-005)
        let normalized_content = normalize_line_endings_for_diff(content);
        let mut lines: Vec<String> = normalized_content
            .lines()
            .map(std::string::ToString::to_string)
            .collect();

        // Track cumulative offset from previous hunks' additions/deletions
        let mut cumulative_offset: i64 = 0;

        // Apply each hunk in order
        for hunk in &diff.hunks {
            let (old_start, _old_count) = hunk.old_range;
            let hunk_lines: Vec<&str> = hunk.content.lines().collect();

            // Extract OLD lines from hunk (context lines + removed lines, NOT added lines)
            // These represent what exists in the original file and should be used for matching
            let context_lines: Vec<&str> = hunk_lines
                .iter()
                .skip(1) // Skip @@ header
                .filter(|line| {
                    // Include context lines (starting with ' ') and removed lines (starting with '-')
                    // but NOT added lines (starting with '+') as those don't exist in the original file
                    line.starts_with(' ')
                        || line.starts_with('-')
                        || (!line.starts_with('+') && !line.starts_with("@@"))
                })
                .filter(|line| !line.starts_with("---")) // Exclude --- header
                .map(|line| {
                    // Strip the prefix character (' ' or '-')
                    if line.starts_with(' ') || line.starts_with('-') {
                        &line[1..]
                    } else {
                        *line
                    }
                })
                .collect();

            // Calculate expected position with cumulative offset
            let expected_pos = ((old_start as i64 - 1) + cumulative_offset).max(0) as usize;

            // Try exact match first, then fuzzy match if needed
            let actual_start = if self.context_matches_at(&lines, expected_pos, &context_lines) {
                expected_pos
            } else {
                // Fuzzy search within window
                match self.find_best_context_match(
                    &lines,
                    expected_pos,
                    &context_lines,
                    FUZZY_SEARCH_WINDOW,
                    MIN_CONTEXT_MATCH_RATIO,
                ) {
                    Some((pos, _confidence)) => {
                        tracing::warn!(
                            "Fuzzy match: hunk at line {} shifted to line {} in '{}'",
                            old_start,
                            pos + 1,
                            diff.target_file
                        );
                        pos
                    }
                    None => {
                        return Err(FixupError::FuzzyMatchFailed {
                            file: diff.target_file.clone(),
                            expected_line: old_start,
                            search_window: FUZZY_SEARCH_WINDOW,
                        });
                    }
                }
            };

            // Track additions and deletions for offset calculation
            let mut additions = 0i64;
            let mut deletions = 0i64;

            // Apply hunk at actual_start position
            let mut hunk_idx = 1; // Start after @@ line
            let mut file_idx = actual_start;

            while hunk_idx < hunk_lines.len() {
                let line = hunk_lines[hunk_idx];

                if line.starts_with('+') && !line.starts_with("+++") {
                    // Add line
                    let new_line = line[1..].to_string();
                    if file_idx <= lines.len() {
                        lines.insert(file_idx, new_line);
                    } else {
                        lines.push(new_line);
                    }
                    file_idx += 1;
                    additions += 1;
                } else if line.starts_with('-') && !line.starts_with("---") {
                    // Remove line
                    if file_idx < lines.len() {
                        lines.remove(file_idx);
                        deletions += 1;
                    }
                } else if line.starts_with(' ') {
                    // Context line - just advance
                    file_idx += 1;
                } else if !line.starts_with("@@") {
                    // Context line without leading space
                    file_idx += 1;
                }

                hunk_idx += 1;
            }

            // Update cumulative offset for subsequent hunks
            cumulative_offset += additions - deletions;
        }

        Ok(lines.join("\n") + "\n")
    }

    /// Compute BLAKE3 hash of content
    fn compute_blake3_hash(&self, content: &str) -> String {
        let hash = blake3::hash(content.as_bytes());
        hash.to_hex().to_string()
    }

    /// Apply changes to files (legacy git apply method - kept for compatibility)
    #[allow(dead_code)]
    pub fn apply_changes_with_git(&self, diffs: &[UnifiedDiff]) -> Result<FixupResult, FixupError> {
        if self.mode != FixupMode::Apply {
            return Err(FixupError::DiffParsingFailed {
                reason: "Cannot apply changes in preview mode".to_string(),
            });
        }

        let mut applied_files = Vec::new();
        let mut failed_files = Vec::new();
        let mut warnings = Vec::new();
        let mut three_way_used = false;

        for diff in diffs {
            match self.apply_single_diff(diff) {
                Ok(used_three_way) => {
                    applied_files.push(AppliedFile {
                        path: diff.target_file.clone(),
                        blake3_first8: "00000000".to_string(), // Not computed in git apply mode
                        applied: true,
                        warnings: Vec::new(),
                    });
                    if used_three_way {
                        three_way_used = true;
                        warnings.push(format!("Used 3-way merge for {}", diff.target_file));
                    }
                }
                Err(e) => {
                    failed_files.push(diff.target_file.clone());
                    warnings.push(format!("Failed to apply {}: {}", diff.target_file, e));
                }
            }
        }

        Ok(FixupResult {
            applied_files,
            failed_files,
            warnings,
            three_way_used,
        })
    }

    /// Validate a diff using git apply --check
    ///
    /// # Security
    ///
    /// The target path is validated through `SandboxRoot::join()` before any file operations.
    fn validate_diff_with_git_apply(&self, diff: &UnifiedDiff) -> Result<Vec<String>, FixupError> {
        // Validate and get the sandboxed target path
        let sandbox_path = self.validate_target_path(&diff.target_file)?;
        let target_path = sandbox_path.as_path();

        if !target_path.exists() {
            return Err(FixupError::TargetFileNotFound {
                path: diff.target_file.clone(),
            });
        }

        // Create temporary directory and copy target file
        let temp_dir = TempDir::new().map_err(|e| FixupError::TempCopyFailed {
            file: diff.target_file.clone(),
            reason: e.to_string(),
        })?;

        let temp_file = temp_dir.path().join("target_file");
        std::fs::copy(target_path, &temp_file).map_err(|e| FixupError::TempCopyFailed {
            file: diff.target_file.clone(),
            reason: e.to_string(),
        })?;

        // Write diff to temporary file
        let diff_file = temp_dir.path().join("changes.diff");
        std::fs::write(&diff_file, &diff.diff_content).map_err(|e| FixupError::TempCopyFailed {
            file: "diff".to_string(),
            reason: e.to_string(),
        })?;

        // Run git apply --check using CommandSpec for secure argv-style execution
        let output = CommandSpec::new("git")
            .args(["apply", "--check", "--verbose"])
            .arg(&diff_file)
            .cwd(temp_dir.path())
            .to_command()
            .output()
            .map_err(|e| FixupError::GitApplyValidationFailed {
                target_file: diff.target_file.clone(),
                reason: format!("Failed to run git apply: {e}"),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(FixupError::GitApplyValidationFailed {
                target_file: diff.target_file.clone(),
                reason: stderr.to_string(),
            });
        }

        // Return any warnings from stdout/stderr
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let mut messages = Vec::new();
        if !stdout.is_empty() {
            messages.push(stdout.to_string());
        }
        if !stderr.is_empty() {
            messages.push(stderr.to_string());
        }

        Ok(messages)
    }

    /// Apply a single diff to its target file
    ///
    /// # Security
    ///
    /// The target path is validated through `SandboxRoot::join()` before any file operations.
    fn apply_single_diff(&self, diff: &UnifiedDiff) -> Result<bool, FixupError> {
        // Validate and get the sandboxed target path
        let sandbox_path = self.validate_target_path(&diff.target_file)?;
        let target_path = sandbox_path.as_path();

        if !target_path.exists() {
            return Err(FixupError::TargetFileNotFound {
                path: diff.target_file.clone(),
            });
        }

        // Write diff to temporary file
        let temp_dir = TempDir::new().map_err(|e| FixupError::TempCopyFailed {
            file: diff.target_file.clone(),
            reason: e.to_string(),
        })?;

        let diff_file = temp_dir.path().join("changes.diff");
        std::fs::write(&diff_file, &diff.diff_content).map_err(|e| FixupError::TempCopyFailed {
            file: "diff".to_string(),
            reason: e.to_string(),
        })?;

        // First try git apply --check using CommandSpec for secure argv-style execution
        let check_output = CommandSpec::new("git")
            .args(["apply", "--check"])
            .arg(&diff_file)
            .cwd(self.base_dir())
            .to_command()
            .output()
            .map_err(|e| FixupError::GitApplyValidationFailed {
                target_file: diff.target_file.clone(),
                reason: format!("Failed to run git apply --check: {e}"),
            })?;

        if !check_output.status.success() {
            // Try with 3-way merge as last resort using CommandSpec
            let three_way_output = CommandSpec::new("git")
                .args(["apply", "--3way"])
                .arg(&diff_file)
                .cwd(self.base_dir())
                .to_command()
                .output()
                .map_err(|e| FixupError::GitApplyExecutionFailed {
                    target_file: diff.target_file.clone(),
                    reason: format!("Failed to run git apply --3way: {e}"),
                })?;

            if !three_way_output.status.success() {
                let stderr = String::from_utf8_lossy(&three_way_output.stderr);
                return Err(FixupError::GitApplyExecutionFailed {
                    target_file: diff.target_file.clone(),
                    reason: stderr.to_string(),
                });
            }

            return Ok(true); // Used 3-way merge
        }

        // Apply the diff normally using CommandSpec for secure argv-style execution
        let apply_output = CommandSpec::new("git")
            .args(["apply"])
            .arg(&diff_file)
            .cwd(self.base_dir())
            .to_command()
            .output()
            .map_err(|e| FixupError::GitApplyExecutionFailed {
                target_file: diff.target_file.clone(),
                reason: format!("Failed to run git apply: {e}"),
            })?;

        if !apply_output.status.success() {
            let stderr = String::from_utf8_lossy(&apply_output.stderr);
            return Err(FixupError::GitApplyExecutionFailed {
                target_file: diff.target_file.clone(),
                reason: stderr.to_string(),
            });
        }

        Ok(false) // Did not use 3-way merge
    }

    /// Calculate change statistics from a diff
    fn calculate_change_stats(&self, diff: &UnifiedDiff) -> (usize, usize) {
        let mut lines_added = 0;
        let mut lines_removed = 0;

        for hunk in &diff.hunks {
            for line in hunk.content.lines() {
                if line.starts_with('+') && !line.starts_with("+++") {
                    lines_added += 1;
                } else if line.starts_with('-') && !line.starts_with("---") {
                    lines_removed += 1;
                }
            }
        }

        (lines_added, lines_removed)
    }
}

/// Normalize line endings to LF (FR-FIX-010, FR-FS-004, FR-FS-005)
///
/// This function converts all line ending styles (CRLF, CR, LF) to LF.
/// This ensures consistent diff calculation regardless of the source file's
/// line ending style, which is especially important on Windows where files
/// may have CRLF line endings.
///
/// # Arguments
///
/// * `content` - The content to normalize
///
/// # Returns
///
/// A string with all line endings normalized to LF (\n)
///
/// # Examples
///
/// ```
/// use xchecker_engine::fixup::normalize_line_endings_for_diff;
///
/// let crlf_content = "line1\r\nline2\r\n";
/// let normalized = normalize_line_endings_for_diff(crlf_content);
/// assert_eq!(normalized, "line1\nline2\n");
/// ```
#[must_use]
pub fn normalize_line_endings_for_diff(content: &str) -> String {
    content.replace("\r\n", "\n").replace('\r', "\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_calculate_change_stats() {
        let temp_dir = TempDir::new().unwrap();
        let parser = FixupParser::new(FixupMode::Preview, temp_dir.path().to_path_buf()).unwrap();

        let content = r#"
FIXUP PLAN:

```diff
--- a/src/test.rs
+++ b/src/test.rs
@@ -1,5 +1,6 @@
 fn test() {
+    let x = 1;
     let y = 2;
-    let z = 3;
+    let z = 4;
     println!("test");
 }
```
"#;

        let diffs = parser.parse_diffs(content).unwrap();
        assert_eq!(diffs.len(), 1);

        let (added, removed) = parser.calculate_change_stats(&diffs[0]);
        assert_eq!(added, 2); // +let x = 1; and +let z = 4;
        assert_eq!(removed, 1); // -let z = 3;
    }
}

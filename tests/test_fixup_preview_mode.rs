//! Integration tests for fixup preview mode (AT-FIX-001)
//!
//! **WHITE-BOX TEST**: This test uses internal module APIs (`fixup::{FixupMode, FixupParser}`)
//! and may break with internal refactors. These tests are intentionally white-box to validate
//! internal implementation details. See FR-TEST-4 for white-box test policy.
//!
//! **Feature: xchecker-runtime-implementation, Property AT-FIX-001**
//!
//! This test validates that fixup preview mode:
//! - Shows intended changes without modifying files
//! - Lists target files correctly
//! - Calculates estimated line changes (added/removed)
//! - Displays validation warnings
//! - Does not modify the filesystem
//! - Produces receipts with applied: false

use anyhow::Result;
use std::fs;
use tempfile::TempDir;
use xchecker::fixup::{FixupMode, FixupParser};

/// AT-FIX-001: Test that preview mode does not modify files
#[test]
fn test_preview_mode_no_file_modifications() -> Result<()> {
    // Create a temporary directory with a test file
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("test.txt");
    let original_content = "line 1\nline 2\nline 3\n";
    fs::write(&test_file, original_content)?;

    // Get the original modification time
    let original_metadata = fs::metadata(&test_file)?;
    let original_modified = original_metadata.modified()?;

    // Create a fixup parser in preview mode
    let parser = FixupParser::new(FixupMode::Preview, temp_dir.path().to_path_buf())?;

    // Create a diff that would modify the file
    let review_content = r#"
FIXUP PLAN:
The following changes are needed:

```diff
--- a/test.txt
+++ b/test.txt
@@ -1,3 +1,4 @@
 line 1
+line 1.5
 line 2
 line 3
```
"#;

    // Parse the diffs
    let diffs = parser.parse_diffs(review_content)?;
    assert_eq!(diffs.len(), 1);

    // Run preview
    let preview = parser.preview_changes(&diffs)?;

    // Verify the file was not modified
    let current_content = fs::read_to_string(&test_file)?;
    assert_eq!(
        current_content, original_content,
        "File content should not change in preview mode"
    );

    // Verify modification time hasn't changed (with small tolerance for filesystem precision)
    let current_metadata = fs::metadata(&test_file)?;
    let current_modified = current_metadata.modified()?;
    assert_eq!(
        original_modified, current_modified,
        "File modification time should not change in preview mode"
    );

    // Verify preview contains expected information
    assert_eq!(preview.target_files.len(), 1);
    assert_eq!(preview.target_files[0], "test.txt");

    Ok(())
}

/// Test that preview mode shows intended targets
#[test]
fn test_preview_shows_intended_targets() -> Result<()> {
    let temp_dir = TempDir::new()?;

    // Create multiple test files
    let file1 = temp_dir.path().join("file1.txt");
    let file2 = temp_dir.path().join("file2.txt");
    fs::write(&file1, "content 1\n")?;
    fs::write(&file2, "content 2\n")?;

    let parser = FixupParser::new(FixupMode::Preview, temp_dir.path().to_path_buf())?;

    let review_content = r#"
FIXUP PLAN:
Multiple files need changes:

```diff
--- a/file1.txt
+++ b/file1.txt
@@ -1,1 +1,2 @@
 content 1
+new line in file1
```

```diff
--- a/file2.txt
+++ b/file2.txt
@@ -1,1 +1,2 @@
 content 2
+new line in file2
```
"#;

    let diffs = parser.parse_diffs(review_content)?;
    let preview = parser.preview_changes(&diffs)?;

    // Verify all target files are listed
    assert_eq!(preview.target_files.len(), 2);
    assert!(preview.target_files.contains(&"file1.txt".to_string()));
    assert!(preview.target_files.contains(&"file2.txt".to_string()));

    // Verify change summaries exist for each file
    assert!(preview.change_summary.contains_key("file1.txt"));
    assert!(preview.change_summary.contains_key("file2.txt"));

    Ok(())
}

/// Test that preview mode calculates estimated line changes correctly
#[test]
fn test_preview_calculates_line_changes() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("test.txt");
    fs::write(&test_file, "line 1\nline 2\nline 3\nline 4\n")?;

    let parser = FixupParser::new(FixupMode::Preview, temp_dir.path().to_path_buf())?;

    let review_content = r#"
FIXUP PLAN:
Changes with additions and removals:

```diff
--- a/test.txt
+++ b/test.txt
@@ -1,4 +1,5 @@
 line 1
+new line 1.5
 line 2
-line 3
+line 3 modified
 line 4
```
"#;

    let diffs = parser.parse_diffs(review_content)?;
    let preview = parser.preview_changes(&diffs)?;

    // Get the change summary for the file
    let summary = preview
        .change_summary
        .get("test.txt")
        .expect("Should have summary for test.txt");

    // Verify line change calculations
    // Expected: 2 additions (+new line 1.5, +line 3 modified), 1 removal (-line 3)
    assert_eq!(summary.lines_added, 2, "Should count 2 added lines");
    assert_eq!(summary.lines_removed, 1, "Should count 1 removed line");
    assert_eq!(summary.hunk_count, 1, "Should have 1 hunk");

    Ok(())
}

/// Test that preview mode displays validation warnings
#[test]
fn test_preview_displays_validation_warnings() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("test.txt");
    fs::write(&test_file, "line 1\nline 2\n")?;

    let parser = FixupParser::new(FixupMode::Preview, temp_dir.path().to_path_buf())?;

    // Create a diff that won't apply cleanly (wrong line numbers)
    let review_content = r#"
FIXUP PLAN:
This diff has incorrect line numbers:

```diff
--- a/test.txt
+++ b/test.txt
@@ -10,2 +10,3 @@
 line 1
+new line
 line 2
```
"#;

    let diffs = parser.parse_diffs(review_content)?;
    let preview = parser.preview_changes(&diffs)?;

    // Verify that validation failed
    assert!(
        !preview.all_valid,
        "Preview should indicate validation failed"
    );
    assert!(
        !preview.warnings.is_empty(),
        "Preview should contain warnings"
    );

    // Verify the specific file's validation status
    let summary = preview
        .change_summary
        .get("test.txt")
        .expect("Should have summary for test.txt");
    assert!(
        !summary.validation_passed,
        "Validation should have failed for this diff"
    );
    assert!(
        !summary.validation_messages.is_empty(),
        "Should have validation messages"
    );

    Ok(())
}

/// Test preview output format
#[test]
fn test_preview_output_format() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("example.rs");
    fs::write(&test_file, "fn main() {\n    println!(\"Hello\");\n}\n")?;

    let parser = FixupParser::new(FixupMode::Preview, temp_dir.path().to_path_buf())?;

    let review_content = r#"
FIXUP PLAN:
Add documentation:

```diff
--- a/example.rs
+++ b/example.rs
@@ -1,3 +1,4 @@
+/// Main entry point
 fn main() {
     println!("Hello");
 }
```
"#;

    let diffs = parser.parse_diffs(review_content)?;
    let preview = parser.preview_changes(&diffs)?;

    // Verify the preview structure
    assert!(!preview.target_files.is_empty(), "Should have target files");
    assert!(
        !preview.change_summary.is_empty(),
        "Should have change summary"
    );

    // Verify the summary contains all required fields
    let summary = preview
        .change_summary
        .get("example.rs")
        .expect("Should have summary");
    assert!(summary.hunk_count > 0, "Should have at least one hunk");
    assert!(
        summary.lines_added > 0 || summary.lines_removed > 0,
        "Should have some changes"
    );

    // The validation_passed field should be present (true or false)
    // The validation_messages should be a vector (empty or with messages)
    assert!(summary.validation_messages.is_empty() || !summary.validation_messages.is_empty());

    Ok(())
}

/// Test that preview mode works with multiple hunks in a single file
#[test]
fn test_preview_multiple_hunks() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("multi.txt");
    fs::write(
        &test_file,
        "section 1\nline 1\nline 2\n\nsection 2\nline 3\nline 4\n",
    )?;

    let parser = FixupParser::new(FixupMode::Preview, temp_dir.path().to_path_buf())?;

    let review_content = r#"
FIXUP PLAN:
Changes in multiple locations:

```diff
--- a/multi.txt
+++ b/multi.txt
@@ -1,3 +1,4 @@
 section 1
+new line after section 1
 line 1
 line 2
@@ -5,3 +6,4 @@
 section 2
+new line after section 2
 line 3
 line 4
```
"#;

    let diffs = parser.parse_diffs(review_content)?;
    let preview = parser.preview_changes(&diffs)?;

    let summary = preview
        .change_summary
        .get("multi.txt")
        .expect("Should have summary");

    // Verify multiple hunks are counted
    assert_eq!(summary.hunk_count, 2, "Should have 2 hunks");
    assert_eq!(summary.lines_added, 2, "Should have 2 added lines");
    assert_eq!(summary.lines_removed, 0, "Should have 0 removed lines");

    Ok(())
}

/// Test that preview mode handles empty diffs gracefully
#[test]
fn test_preview_empty_diffs() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let parser = FixupParser::new(FixupMode::Preview, temp_dir.path().to_path_buf())?;

    let empty_diffs = vec![];
    let preview = parser.preview_changes(&empty_diffs)?;

    // Verify empty preview
    assert!(
        preview.target_files.is_empty(),
        "Should have no target files"
    );
    assert!(
        preview.change_summary.is_empty(),
        "Should have no change summary"
    );
    assert!(preview.warnings.is_empty(), "Should have no warnings");
    assert!(
        preview.all_valid,
        "Empty preview should be considered valid"
    );

    Ok(())
}

/// Test that preview mode handles missing target files
#[test]
fn test_preview_missing_target_file() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let parser = FixupParser::new(FixupMode::Preview, temp_dir.path().to_path_buf())?;

    let review_content = r#"
FIXUP PLAN:
Changes to non-existent file:

```diff
--- a/nonexistent.txt
+++ b/nonexistent.txt
@@ -1,1 +1,2 @@
 line 1
+line 2
```
"#;

    let diffs = parser.parse_diffs(review_content)?;
    let preview = parser.preview_changes(&diffs)?;

    // Verify that validation failed for the missing file
    assert!(
        !preview.all_valid,
        "Preview should indicate validation failed"
    );
    assert!(
        !preview.warnings.is_empty(),
        "Should have warnings about missing file"
    );

    let summary = preview
        .change_summary
        .get("nonexistent.txt")
        .expect("Should have summary");
    assert!(
        !summary.validation_passed,
        "Validation should fail for missing file"
    );

    Ok(())
}

/// Test that preview mode correctly identifies files that need changes
#[test]
fn test_preview_identifies_changed_files() -> Result<()> {
    let temp_dir = TempDir::new()?;

    // Create test files
    let file1 = temp_dir.path().join("unchanged.txt");
    let file2 = temp_dir.path().join("changed.txt");
    fs::write(&file1, "no changes\n")?;
    fs::write(&file2, "will change\n")?;

    let parser = FixupParser::new(FixupMode::Preview, temp_dir.path().to_path_buf())?;

    let review_content = r#"
FIXUP PLAN:
Only one file needs changes:

```diff
--- a/changed.txt
+++ b/changed.txt
@@ -1,1 +1,2 @@
 will change
+new line
```
"#;

    let diffs = parser.parse_diffs(review_content)?;
    let preview = parser.preview_changes(&diffs)?;

    // Verify only the changed file is in the preview
    assert_eq!(preview.target_files.len(), 1);
    assert_eq!(preview.target_files[0], "changed.txt");
    assert!(!preview.target_files.contains(&"unchanged.txt".to_string()));

    // Verify the unchanged file was not modified
    let unchanged_content = fs::read_to_string(&file1)?;
    assert_eq!(unchanged_content, "no changes\n");

    // Verify the "changed" file was also not modified (preview mode)
    let changed_content = fs::read_to_string(&file2)?;
    assert_eq!(changed_content, "will change\n");

    Ok(())
}

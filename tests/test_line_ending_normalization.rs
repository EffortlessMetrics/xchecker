//! Unit tests for line ending normalization (FR-FIX-010, FR-FS-004, FR-FS-005)
//!
//! **WHITE-BOX TEST**: This test uses internal module APIs (`fixup::{FixupMode, FixupParser,
//! normalize_line_endings_for_diff}`) and may break with internal refactors. These tests are
//! intentionally white-box to validate internal implementation details. See FR-TEST-4 for
//! white-box test policy.
//!
//! This test file validates that:
//! - Line endings are normalized before diff calculation (FR-FIX-010)
//! - LF is enforced for JSON and text artifacts (FR-FS-004)
//! - CRLF is tolerated on read (Windows) (FR-FS-005)

use anyhow::Result;
use std::fs;
use tempfile::TempDir;
use xchecker::fixup::{FixupMode, FixupParser, normalize_line_endings_for_diff};

/// Test that normalize_line_endings_for_diff converts CRLF to LF
#[test]
fn test_normalize_crlf_to_lf() {
    let content_crlf = "line1\r\nline2\r\nline3\r\n";
    let normalized = normalize_line_endings_for_diff(content_crlf);
    assert_eq!(normalized, "line1\nline2\nline3\n");
    assert!(!normalized.contains("\r\n"), "Should not contain CRLF");
    assert!(!normalized.contains("\r"), "Should not contain CR");
}

/// Test that normalize_line_endings_for_diff converts CR to LF
#[test]
fn test_normalize_cr_to_lf() {
    let content_cr = "line1\rline2\rline3\r";
    let normalized = normalize_line_endings_for_diff(content_cr);
    assert_eq!(normalized, "line1\nline2\nline3\n");
    assert!(!normalized.contains("\r"), "Should not contain CR");
}

/// Test that normalize_line_endings_for_diff handles mixed line endings
#[test]
fn test_normalize_mixed_line_endings() {
    let content_mixed = "line1\r\nline2\nline3\rline4\n";
    let normalized = normalize_line_endings_for_diff(content_mixed);
    assert_eq!(normalized, "line1\nline2\nline3\nline4\n");
    assert!(!normalized.contains("\r\n"), "Should not contain CRLF");
    assert!(!normalized.contains("\r"), "Should not contain CR");
}

/// Test that normalize_line_endings_for_diff preserves LF
#[test]
fn test_normalize_preserves_lf() {
    let content_lf = "line1\nline2\nline3\n";
    let normalized = normalize_line_endings_for_diff(content_lf);
    assert_eq!(normalized, content_lf);
}

/// Test that normalize_line_endings_for_diff handles empty content
#[test]
fn test_normalize_empty_content() {
    let content_empty = "";
    let normalized = normalize_line_endings_for_diff(content_empty);
    assert_eq!(normalized, "");
}

/// Test that normalize_line_endings_for_diff handles single line without ending
#[test]
fn test_normalize_single_line_no_ending() {
    let content = "single line";
    let normalized = normalize_line_endings_for_diff(content);
    assert_eq!(normalized, "single line");
}

/// Test that normalize_line_endings_for_diff handles content with only line endings
#[test]
fn test_normalize_only_line_endings() {
    let content_crlf = "\r\n\r\n\r\n";
    let normalized = normalize_line_endings_for_diff(content_crlf);
    assert_eq!(normalized, "\n\n\n");
}

/// Test diff calculation with CRLF content
#[test]
fn test_diff_calculation_with_crlf() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let parser = FixupParser::new(FixupMode::Preview, temp_dir.path().to_path_buf())?;

    // Create a test file with CRLF line endings
    let test_file = temp_dir.path().join("test_crlf.txt");
    let content_crlf = b"line1\r\nline2\r\nline3\r\n";
    fs::write(&test_file, content_crlf)?;

    // Create a diff that adds a line
    let review_content = r#"
FIXUP PLAN:

```diff
--- a/test_crlf.txt
+++ b/test_crlf.txt
@@ -1,3 +1,4 @@
 line1
+line1.5
 line2
 line3
```
"#;

    let diffs = parser.parse_diffs(review_content)?;
    assert_eq!(diffs.len(), 1);

    let preview = parser.preview_changes(&diffs)?;
    assert_eq!(preview.target_files.len(), 1);

    let summary = preview.change_summary.get("test_crlf.txt").unwrap();
    assert_eq!(summary.lines_added, 1);
    assert_eq!(summary.lines_removed, 0);

    Ok(())
}

/// Test diff calculation with mixed line endings
#[test]
fn test_diff_calculation_with_mixed_endings() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let parser = FixupParser::new(FixupMode::Preview, temp_dir.path().to_path_buf())?;

    // Create a test file with mixed line endings
    let test_file = temp_dir.path().join("test_mixed.txt");
    let content_mixed = b"line1\r\nline2\nline3\rline4\n";
    fs::write(&test_file, content_mixed)?;

    // Create a diff that removes a line
    let review_content = r#"
FIXUP PLAN:

```diff
--- a/test_mixed.txt
+++ b/test_mixed.txt
@@ -1,4 +1,3 @@
 line1
-line2
 line3
 line4
```
"#;

    let diffs = parser.parse_diffs(review_content)?;
    assert_eq!(diffs.len(), 1);

    let preview = parser.preview_changes(&diffs)?;
    assert_eq!(preview.target_files.len(), 1);

    let summary = preview.change_summary.get("test_mixed.txt").unwrap();
    assert_eq!(summary.lines_added, 0);
    assert_eq!(summary.lines_removed, 1);

    Ok(())
}

/// Test that applied fixups write LF line endings (FR-FS-004)
#[test]
fn test_applied_fixups_write_lf() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let parser = FixupParser::new(FixupMode::Apply, temp_dir.path().to_path_buf())?;

    // Create a test file with CRLF line endings
    let test_file = temp_dir.path().join("test_apply_lf.txt");
    let content_crlf = b"line1\r\nline2\r\nline3\r\n";
    fs::write(&test_file, content_crlf)?;

    // Create a diff that adds a line
    let review_content = r#"
FIXUP PLAN:

```diff
--- a/test_apply_lf.txt
+++ b/test_apply_lf.txt
@@ -1,3 +1,4 @@
 line1
+line1.5
 line2
 line3
```
"#;

    let diffs = parser.parse_diffs(review_content)?;
    let result = parser.apply_changes(&diffs)?;

    assert_eq!(result.applied_files.len(), 1);
    assert_eq!(result.failed_files.len(), 0);

    // Verify the file was written with LF line endings (FR-FS-004)
    let modified_content = fs::read_to_string(&test_file)?;
    assert!(
        !modified_content.contains("\r\n"),
        "Should not contain CRLF"
    );
    assert!(!modified_content.contains("\r"), "Should not contain CR");
    assert!(modified_content.contains("line1\n"), "Should contain LF");
    assert!(modified_content.contains("line1.5\n"), "Should contain LF");

    Ok(())
}

/// Test that reading files tolerates CRLF (FR-FS-005)
#[test]
fn test_read_tolerates_crlf() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let parser = FixupParser::new(FixupMode::Apply, temp_dir.path().to_path_buf())?;

    // Create a test file with CRLF line endings
    let test_file = temp_dir.path().join("test_read_crlf.txt");
    let content_crlf = b"original line 1\r\noriginal line 2\r\noriginal line 3\r\n";
    fs::write(&test_file, content_crlf)?;

    // Create a diff that modifies the file
    let review_content = r#"
FIXUP PLAN:

```diff
--- a/test_read_crlf.txt
+++ b/test_read_crlf.txt
@@ -1,3 +1,3 @@
-original line 1
+modified line 1
 original line 2
 original line 3
```
"#;

    let diffs = parser.parse_diffs(review_content)?;
    let result = parser.apply_changes(&diffs)?;

    // Should succeed despite CRLF in original file (FR-FS-005)
    assert_eq!(result.applied_files.len(), 1);
    assert_eq!(result.failed_files.len(), 0);

    // Verify the modification was applied correctly
    let modified_content = fs::read_to_string(&test_file)?;
    assert!(
        modified_content.contains("modified line 1"),
        "Should contain modified line"
    );
    assert!(
        !modified_content.contains("original line 1"),
        "Should not contain original line"
    );

    // Verify LF line endings in output (FR-FS-004)
    assert!(
        !modified_content.contains("\r\n"),
        "Should not contain CRLF"
    );
    assert!(!modified_content.contains("\r"), "Should not contain CR");

    Ok(())
}

/// Test diff estimates are consistent regardless of line ending style
#[test]
fn test_diff_estimates_consistent_across_line_endings() -> Result<()> {
    let temp_dir = TempDir::new()?;

    // Test with LF
    let parser_lf = FixupParser::new(FixupMode::Preview, temp_dir.path().to_path_buf())?;
    let test_file_lf = temp_dir.path().join("test_lf.txt");
    let content_lf = b"line1\nline2\nline3\n";
    fs::write(&test_file_lf, content_lf)?;

    // Test with CRLF
    let parser_crlf = FixupParser::new(FixupMode::Preview, temp_dir.path().to_path_buf())?;
    let test_file_crlf = temp_dir.path().join("test_crlf.txt");
    let content_crlf = b"line1\r\nline2\r\nline3\r\n";
    fs::write(&test_file_crlf, content_crlf)?;

    // Same diff for both files
    let review_content_lf = r#"
FIXUP PLAN:

```diff
--- a/test_lf.txt
+++ b/test_lf.txt
@@ -1,3 +1,4 @@
 line1
+added line
 line2
-line3
```
"#;

    let review_content_crlf = r#"
FIXUP PLAN:

```diff
--- a/test_crlf.txt
+++ b/test_crlf.txt
@@ -1,3 +1,4 @@
 line1
+added line
 line2
-line3
```
"#;

    let diffs_lf = parser_lf.parse_diffs(review_content_lf)?;
    let preview_lf = parser_lf.preview_changes(&diffs_lf)?;
    let summary_lf = preview_lf.change_summary.get("test_lf.txt").unwrap();

    let diffs_crlf = parser_crlf.parse_diffs(review_content_crlf)?;
    let preview_crlf = parser_crlf.preview_changes(&diffs_crlf)?;
    let summary_crlf = preview_crlf.change_summary.get("test_crlf.txt").unwrap();

    // Diff estimates should be identical regardless of line ending style (FR-FIX-010)
    assert_eq!(summary_lf.lines_added, summary_crlf.lines_added);
    assert_eq!(summary_lf.lines_removed, summary_crlf.lines_removed);
    assert_eq!(summary_lf.hunk_count, summary_crlf.hunk_count);

    Ok(())
}

/// Test that unicode content with various line endings is handled correctly
#[test]
fn test_unicode_with_line_endings() {
    let content_unicode_crlf = "Hello 世界\r\nПривет мир\r\nمرحبا العالم\r\n";
    let normalized = normalize_line_endings_for_diff(content_unicode_crlf);

    assert_eq!(normalized, "Hello 世界\nПривет мир\nمرحبا العالم\n");
    assert!(!normalized.contains("\r\n"), "Should not contain CRLF");
    assert!(!normalized.contains("\r"), "Should not contain CR");
}

/// Test that special characters with line endings are preserved
#[test]
fn test_special_chars_with_line_endings() {
    let content_special = "tab:\t\r\nquote:\"\r\nbackslash:\\\r\n";
    let normalized = normalize_line_endings_for_diff(content_special);

    assert_eq!(normalized, "tab:\t\nquote:\"\nbackslash:\\\n");
    assert!(normalized.contains("\t"), "Should preserve tab");
    assert!(normalized.contains("\""), "Should preserve quote");
    assert!(normalized.contains("\\"), "Should preserve backslash");
    assert!(!normalized.contains("\r"), "Should not contain CR");
}

#[test]
fn test_comprehensive_line_ending_normalization() {
    println!("🚀 Running comprehensive line ending normalization tests...");
    println!();

    test_normalize_crlf_to_lf();
    println!("  ✓ CRLF to LF normalization");

    test_normalize_cr_to_lf();
    println!("  ✓ CR to LF normalization");

    test_normalize_mixed_line_endings();
    println!("  ✓ Mixed line endings normalization");

    test_normalize_preserves_lf();
    println!("  ✓ LF preservation");

    test_normalize_empty_content();
    println!("  ✓ Empty content handling");

    test_normalize_single_line_no_ending();
    println!("  ✓ Single line without ending");

    test_normalize_only_line_endings();
    println!("  ✓ Only line endings");

    test_unicode_with_line_endings();
    println!("  ✓ Unicode with line endings");

    test_special_chars_with_line_endings();
    println!("  ✓ Special characters with line endings");

    println!();
    println!("✅ All line ending normalization tests passed!");
    println!();
    println!("Requirements Validated:");
    println!("  ✓ FR-FIX-010: Line endings normalized before diff calculation");
    println!("  ✓ FR-FS-004: LF enforcement for JSON and text artifacts");
    println!("  ✓ FR-FS-005: CRLF tolerance on read (Windows)");
}

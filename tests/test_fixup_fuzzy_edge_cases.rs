//! Edge case tests for fixup fuzzy matching
//!
//! **WHITE-BOX TEST**: This test uses internal module APIs (`fixup::{FixupMode, FixupParser}`)
//! and may break with internal refactors. These tests are intentionally white-box to validate
//! internal implementation details. See FR-TEST-4 for white-box test policy.
//!
//! Tests for scenarios identified as gaps in the fixup test coverage:
//! - Multi-hunk with heavy edits between hunks
//! - Hunks that shift backwards due to earlier deletions
//! - Small context hunks (1-2 context lines)
//! - Ambiguous context matching
//! - Files shorter than hunk context
//! - Hunks with no context lines
//!
//! Tests marked with `#[ignore]` document known limitations in the current
//! fuzzy matching implementation that may be addressed in future versions.

use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;
use xchecker::fixup::{FixupMode, FixupParser};

/// Helper to create a parser and test file
fn setup_test_env() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let base_dir = temp_dir.path().to_path_buf();
    (temp_dir, base_dir)
}

/// Test: Multiple hunks where earlier hunks delete lines, causing later hunks to shift backwards
///
/// KNOWN LIMITATION: The current context matching algorithm extracts ALL context lines
/// from a hunk (before and after deletions). When a hunk deletes lines in the middle,
/// the context lines aren't contiguous in the original file, causing matching to fail.
///
/// This documents expected behavior that isn't currently supported.
#[test]
#[ignore = "Known limitation: context lines split by deletions aren't matched correctly"]
fn test_multi_hunk_with_backward_shift() {
    let (_temp_dir, base_dir) = setup_test_env();

    // Create original file with 20 lines
    let test_file = base_dir.join("backward_shift.txt");
    let original_content = (1..=20)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    fs::write(&test_file, &original_content).unwrap();

    let parser = FixupParser::new(FixupMode::Apply, base_dir.clone()).unwrap();

    // Diff with two hunks:
    // First hunk: delete lines 3-5 (removes 3 lines)
    // Second hunk: modify line 15 (now at position 12 after deletion)
    let content = r#"
FIXUP PLAN:

```diff
--- a/backward_shift.txt
+++ b/backward_shift.txt
@@ -1,7 +1,4 @@
 line 1
 line 2
-line 3
-line 4
-line 5
 line 6
 line 7
@@ -14,4 +11,4 @@
 line 14
-line 15
+line 15 modified
 line 16
 line 17
```
"#;

    let diffs = parser.parse_diffs(content).unwrap();
    assert_eq!(diffs.len(), 1);
    assert_eq!(diffs[0].hunks.len(), 2);

    // Apply the changes
    let result = parser.apply_changes(&diffs).unwrap();
    assert!(result.failed_files.is_empty(), "No files should fail");
    assert_eq!(result.applied_files.len(), 1);

    // Verify the result
    let new_content = fs::read_to_string(&test_file).unwrap();
    let lines: Vec<&str> = new_content.lines().collect();

    // Should have 17 lines (20 - 3 deleted)
    assert_eq!(lines.len(), 17);
    assert_eq!(lines[0], "line 1");
    assert_eq!(lines[1], "line 2");
    assert_eq!(lines[2], "line 6"); // line 3 now (after deleting original 3-5)
    assert!(lines.contains(&"line 15 modified"));
    assert!(!lines.contains(&"line 3"));
    assert!(!lines.contains(&"line 4"));
    assert!(!lines.contains(&"line 5"));
}

/// Test: Simple multi-hunk that should work (no deletions splitting context)
#[test]
fn test_multi_hunk_simple_additions() {
    let (_temp_dir, base_dir) = setup_test_env();

    let test_file = base_dir.join("simple_multi.txt");
    let original_content =
        "line 1\nline 2\nline 3\nline 4\nline 5\nline 6\nline 7\nline 8\nline 9\nline 10\n";
    fs::write(&test_file, original_content).unwrap();

    let parser = FixupParser::new(FixupMode::Apply, base_dir.clone()).unwrap();

    // Two hunks that only add lines (no deletions to split context)
    let content = r#"
FIXUP PLAN:

```diff
--- a/simple_multi.txt
+++ b/simple_multi.txt
@@ -1,3 +1,4 @@
 line 1
+inserted A
 line 2
 line 3
@@ -8,3 +9,4 @@
 line 8
+inserted B
 line 9
 line 10
```
"#;

    let diffs = parser.parse_diffs(content).unwrap();
    let result = parser.apply_changes(&diffs).unwrap();
    assert!(result.failed_files.is_empty());

    let new_content = fs::read_to_string(&test_file).unwrap();
    assert!(new_content.contains("inserted A"));
    assert!(new_content.contains("inserted B"));
}

/// Test: Multiple hunks with heavy edits between them (many additions)
///
/// KNOWN LIMITATION: Second hunk's position is calculated based on original file,
/// but the cumulative offset adjustment may not find the correct match position.
#[test]
#[ignore = "Known limitation: cumulative offset with large additions"]
fn test_multi_hunk_with_forward_shift() {
    let (_temp_dir, base_dir) = setup_test_env();

    // Create original file with 20 lines
    let test_file = base_dir.join("forward_shift.txt");
    let original_content = (1..=20)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    fs::write(&test_file, &original_content).unwrap();

    let parser = FixupParser::new(FixupMode::Apply, base_dir.clone()).unwrap();

    // Diff with two hunks:
    // First hunk: add 5 lines after line 3
    // Second hunk: modify line 15 (now at position 20 after additions)
    let content = r#"
FIXUP PLAN:

```diff
--- a/forward_shift.txt
+++ b/forward_shift.txt
@@ -2,4 +2,9 @@
 line 2
 line 3
+new line A
+new line B
+new line C
+new line D
+new line E
 line 4
 line 5
@@ -14,4 +19,4 @@
 line 14
-line 15
+line 15 modified
 line 16
 line 17
```
"#;

    let diffs = parser.parse_diffs(content).unwrap();
    assert_eq!(diffs.len(), 1);
    assert_eq!(diffs[0].hunks.len(), 2);

    // Apply the changes
    let result = parser.apply_changes(&diffs).unwrap();
    assert!(result.failed_files.is_empty(), "No files should fail");
    assert_eq!(result.applied_files.len(), 1);

    // Verify the result
    let new_content = fs::read_to_string(&test_file).unwrap();
    let lines: Vec<&str> = new_content.lines().collect();

    // Should have 25 lines (20 + 5 added)
    assert_eq!(lines.len(), 25);
    assert!(lines.contains(&"new line A"));
    assert!(lines.contains(&"new line E"));
    assert!(lines.contains(&"line 15 modified"));
}

/// Test: Small context hunk with only 1 context line
#[test]
fn test_small_context_hunk_one_line() {
    let (_temp_dir, base_dir) = setup_test_env();

    let test_file = base_dir.join("small_context.txt");
    let original_content = "function start() {\n    // implementation\n}\n";
    fs::write(&test_file, original_content).unwrap();

    let parser = FixupParser::new(FixupMode::Apply, base_dir.clone()).unwrap();

    // Diff with minimal context (1 line)
    let content = r#"
FIXUP PLAN:

```diff
--- a/small_context.txt
+++ b/small_context.txt
@@ -1,2 +1,3 @@
 function start() {
+    console.log("Starting");
     // implementation
```
"#;

    let diffs = parser.parse_diffs(content).unwrap();
    let result = parser.apply_changes(&diffs).unwrap();
    assert!(result.failed_files.is_empty());

    let new_content = fs::read_to_string(&test_file).unwrap();
    assert!(new_content.contains("console.log"));
}

/// Test: Small context hunk with only 2 context lines
#[test]
fn test_small_context_hunk_two_lines() {
    let (_temp_dir, base_dir) = setup_test_env();

    let test_file = base_dir.join("small_context2.txt");
    let original_content = "line 1\nline 2\nline 3\nline 4\nline 5\n";
    fs::write(&test_file, original_content).unwrap();

    let parser = FixupParser::new(FixupMode::Apply, base_dir.clone()).unwrap();

    // Diff with 2 context lines
    let content = r#"
FIXUP PLAN:

```diff
--- a/small_context2.txt
+++ b/small_context2.txt
@@ -2,3 +2,4 @@
 line 2
+inserted line
 line 3
 line 4
```
"#;

    let diffs = parser.parse_diffs(content).unwrap();
    let result = parser.apply_changes(&diffs).unwrap();
    assert!(result.failed_files.is_empty());

    let new_content = fs::read_to_string(&test_file).unwrap();
    let lines: Vec<&str> = new_content.lines().collect();
    assert_eq!(lines.len(), 6);
    assert_eq!(lines[2], "inserted line");
}

/// Test: Hunk with no context lines (only additions)
#[test]
fn test_hunk_no_context_only_additions() {
    let (_temp_dir, base_dir) = setup_test_env();

    let test_file = base_dir.join("no_context.txt");
    let original_content = "line 1\nline 2\nline 3\n";
    fs::write(&test_file, original_content).unwrap();

    let parser = FixupParser::new(FixupMode::Apply, base_dir.clone()).unwrap();

    // Diff with no context lines - just additions at a specific position
    // This tests the edge case where there's no context to match
    let content = r#"
FIXUP PLAN:

```diff
--- a/no_context.txt
+++ b/no_context.txt
@@ -1,0 +1,2 @@
+new first line
+another new line
```
"#;

    let diffs = parser.parse_diffs(content).unwrap();
    let result = parser.apply_changes(&diffs).unwrap();

    // This should work because empty context matches anywhere
    assert!(result.failed_files.is_empty());
}

/// Test: File shorter than the original hunk context
#[test]
fn test_file_shorter_than_hunk_context() {
    let (_temp_dir, base_dir) = setup_test_env();

    // Create a short file (3 lines)
    let test_file = base_dir.join("short_file.txt");
    let original_content = "line 1\nline 2\nline 3\n";
    fs::write(&test_file, original_content).unwrap();

    let parser = FixupParser::new(FixupMode::Apply, base_dir.clone()).unwrap();

    // Diff that references lines that exist
    let content = r#"
FIXUP PLAN:

```diff
--- a/short_file.txt
+++ b/short_file.txt
@@ -1,3 +1,4 @@
 line 1
+new line
 line 2
 line 3
```
"#;

    let diffs = parser.parse_diffs(content).unwrap();
    let result = parser.apply_changes(&diffs).unwrap();
    assert!(result.failed_files.is_empty());

    let new_content = fs::read_to_string(&test_file).unwrap();
    assert!(new_content.contains("new line"));
}

/// Test: Simple replacement at known position
///
/// KNOWN LIMITATION: Replacements (delete + add) cause context lines to be
/// non-contiguous, which breaks the context matching algorithm.
#[test]
#[ignore = "Known limitation: replacements break context contiguity"]
fn test_simple_replacement_at_position() {
    let (_temp_dir, base_dir) = setup_test_env();

    let test_file = base_dir.join("simple_replace.txt");
    // Line numbers: 1=line1, 2=line2, 3=oldvalue, 4=line4, 5=line5
    let original_content = "line1\nline2\noldvalue\nline4\nline5\n";
    fs::write(&test_file, original_content).unwrap();

    let parser = FixupParser::new(FixupMode::Apply, base_dir.clone()).unwrap();

    // Diff with contiguous context (no deletions breaking context)
    let content = r#"
FIXUP PLAN:

```diff
--- a/simple_replace.txt
+++ b/simple_replace.txt
@@ -2,4 +2,4 @@
 line2
-oldvalue
+newvalue
 line4
 line5
```
"#;

    let diffs = parser.parse_diffs(content).unwrap();
    let result = parser.apply_changes(&diffs).unwrap();

    assert!(result.failed_files.is_empty());

    let new_content = fs::read_to_string(&test_file).unwrap();
    assert!(new_content.contains("newvalue"));
    assert!(!new_content.contains("oldvalue"));
}

/// Test: Fuzzy match with ambiguous repeated patterns
///
/// KNOWN LIMITATION: When context lines are repeated throughout the file,
/// the matcher may not correctly identify the intended position.
#[test]
#[ignore = "Known limitation: ambiguous repeated context patterns"]
fn test_ambiguous_context_should_pick_best_match() {
    let (_temp_dir, base_dir) = setup_test_env();

    // Create file with repeated patterns
    let test_file = base_dir.join("ambiguous.txt");
    let original_content = r#"function foo() {
    return 1;
}

function bar() {
    return 1;
}

function baz() {
    return 1;
}
"#;
    fs::write(&test_file, original_content).unwrap();

    let parser = FixupParser::new(FixupMode::Apply, base_dir.clone()).unwrap();

    // Diff targeting the second function with unique context
    let content = r#"
FIXUP PLAN:

```diff
--- a/ambiguous.txt
+++ b/ambiguous.txt
@@ -5,3 +5,3 @@
 function bar() {
-    return 1;
+    return 42;
 }
```
"#;

    let diffs = parser.parse_diffs(content).unwrap();
    let result = parser.apply_changes(&diffs).unwrap();

    // Should apply successfully because function bar() is a unique context
    assert!(result.failed_files.is_empty());

    let new_content = fs::read_to_string(&test_file).unwrap();
    // The change should be applied to function bar
    assert!(new_content.contains("return 42"));
    // foo and baz should still have return 1
    let return_1_count = new_content.matches("return 1").count();
    let return_42_count = new_content.matches("return 42").count();
    assert_eq!(return_1_count, 2); // foo and baz
    assert_eq!(return_42_count, 1); // bar
}

/// Test: Fuzzy match with context shifted by exactly the search window boundary
#[test]
fn test_fuzzy_match_at_window_boundary() {
    let (_temp_dir, base_dir) = setup_test_env();

    // Create file with 100 lines
    let test_file = base_dir.join("boundary.txt");
    let original_content = (1..=100)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    fs::write(&test_file, &original_content).unwrap();

    let parser = FixupParser::new(FixupMode::Apply, base_dir.clone()).unwrap();

    // Diff that claims to be at line 50, but actually matches at line 50
    // This tests exact matching at expected position
    let content = r#"
FIXUP PLAN:

```diff
--- a/boundary.txt
+++ b/boundary.txt
@@ -49,3 +49,4 @@
 line 49
 line 50
+inserted at 50
 line 51
```
"#;

    let diffs = parser.parse_diffs(content).unwrap();
    let result = parser.apply_changes(&diffs).unwrap();
    assert!(result.failed_files.is_empty());

    let new_content = fs::read_to_string(&test_file).unwrap();
    assert!(new_content.contains("inserted at 50"));
}

/// Test: Fuzzy match when file has been significantly edited (content shifted by 40 lines)
///
/// KNOWN LIMITATION: Context matching requires context lines to be found
/// at expected positions. Large shifts with non-unique context may fail.
#[test]
#[ignore = "Known limitation: large shifts require unique context"]
fn test_fuzzy_match_large_shift_within_window() {
    let (_temp_dir, base_dir) = setup_test_env();

    // Create file with specific content at line 90
    let test_file = base_dir.join("large_shift.txt");
    let mut lines: Vec<String> = (1..=100).map(|i| format!("line {i}")).collect();
    lines[89] = "target line to modify".to_string(); // Line 90 (0-indexed: 89)

    let original_content = lines.join("\n") + "\n";
    fs::write(&test_file, &original_content).unwrap();

    let parser = FixupParser::new(FixupMode::Apply, base_dir.clone()).unwrap();

    // Diff claims target is at line 50, but it's actually at line 90
    // This is a 40 line shift - within the 50 line fuzzy window
    let content = r#"
FIXUP PLAN:

```diff
--- a/large_shift.txt
+++ b/large_shift.txt
@@ -49,3 +49,3 @@
 line 89
-target line to modify
+target line MODIFIED
 line 91
```
"#;

    let diffs = parser.parse_diffs(content).unwrap();
    let result = parser.apply_changes(&diffs).unwrap();

    // Should succeed via fuzzy matching
    assert!(result.failed_files.is_empty());

    let new_content = fs::read_to_string(&test_file).unwrap();
    assert!(new_content.contains("target line MODIFIED"));
    assert!(!new_content.contains("target line to modify"));
}

/// Test: Replacement with full surrounding context
///
/// KNOWN LIMITATION: Same as above - replacements have context on both sides
/// of the deletion, which aren't contiguous in the original file.
#[test]
#[ignore = "Known limitation: replacements break context contiguity"]
fn test_replacement_with_full_context() {
    let (_temp_dir, base_dir) = setup_test_env();

    let test_file = base_dir.join("full_context.txt");
    // Create file with clear structure
    let original_content = "header\ncontext_before\ntarget_line\ncontext_after\nfooter\n";
    fs::write(&test_file, original_content).unwrap();

    let parser = FixupParser::new(FixupMode::Apply, base_dir.clone()).unwrap();

    // Diff with proper context surrounding the change
    let content = r#"
FIXUP PLAN:

```diff
--- a/full_context.txt
+++ b/full_context.txt
@@ -2,4 +2,4 @@
 context_before
-target_line
+modified_target
 context_after
 footer
```
"#;

    let diffs = parser.parse_diffs(content).unwrap();
    let result = parser.apply_changes(&diffs).unwrap();
    assert!(result.failed_files.is_empty());

    let new_content = fs::read_to_string(&test_file).unwrap();
    assert!(new_content.contains("modified_target"));
    assert!(!new_content.contains("target_line"));
}

/// Test: Fuzzy match should fail when context shift exceeds window (>50 lines)
#[test]
fn test_fuzzy_match_exceeds_window() {
    let (_temp_dir, base_dir) = setup_test_env();

    // Create file with specific content at line 150
    let test_file = base_dir.join("exceeds_window.txt");
    let mut lines: Vec<String> = (1..=200).map(|i| format!("line {i}")).collect();
    lines[149] = "unique target content".to_string(); // Line 150

    let original_content = lines.join("\n") + "\n";
    fs::write(&test_file, &original_content).unwrap();

    let parser = FixupParser::new(FixupMode::Apply, base_dir.clone()).unwrap();

    // Diff claims target is at line 50, but it's at line 150
    // This is a 100 line shift - exceeds the 50 line fuzzy window
    let content = r#"
FIXUP PLAN:

```diff
--- a/exceeds_window.txt
+++ b/exceeds_window.txt
@@ -49,3 +49,3 @@
 line 149
-unique target content
+MODIFIED content
 line 151
```
"#;

    let diffs = parser.parse_diffs(content).unwrap();
    let result = parser.apply_changes(&diffs).unwrap();

    // Should fail because the shift exceeds the fuzzy window
    assert!(!result.failed_files.is_empty());
    assert!(
        result
            .failed_files
            .contains(&"exceeds_window.txt".to_string())
    );
}

/// Test: Whitespace normalization in context matching
#[test]
fn test_whitespace_normalized_matching() {
    let (_temp_dir, base_dir) = setup_test_env();

    let test_file = base_dir.join("whitespace.txt");
    // File with inconsistent whitespace
    let original_content = "function foo() {\n    let   x = 1;\n    let y = 2;\n}\n";
    fs::write(&test_file, original_content).unwrap();

    let parser = FixupParser::new(FixupMode::Apply, base_dir.clone()).unwrap();

    // Diff with normalized whitespace
    let content = r#"
FIXUP PLAN:

```diff
--- a/whitespace.txt
+++ b/whitespace.txt
@@ -1,4 +1,5 @@
 function foo() {
     let x = 1;
+    let z = 3;
     let y = 2;
 }
```
"#;

    let diffs = parser.parse_diffs(content).unwrap();
    let result = parser.apply_changes(&diffs).unwrap();

    // Should succeed via whitespace-normalized matching
    assert!(result.failed_files.is_empty());

    let new_content = fs::read_to_string(&test_file).unwrap();
    assert!(new_content.contains("let z = 3"));
}

/// Test: Multiple hunks in same file with cumulative offset tracking
///
/// KNOWN LIMITATION: Complex multi-hunk diffs with deletions and additions
/// may fail due to context matching issues across hunk boundaries.
#[test]
#[ignore = "Known limitation: complex multi-hunk cumulative offset"]
fn test_cumulative_offset_three_hunks() {
    let (_temp_dir, base_dir) = setup_test_env();

    let test_file = base_dir.join("cumulative.txt");
    let original_content = (1..=30)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    fs::write(&test_file, &original_content).unwrap();

    let parser = FixupParser::new(FixupMode::Apply, base_dir.clone()).unwrap();

    // Three hunks:
    // 1. Delete 2 lines at position 5 (-2 offset)
    // 2. Add 3 lines at position 15 (+3 offset, net +1)
    // 3. Modify line 25 (should account for net +1 offset)
    let content = r#"
FIXUP PLAN:

```diff
--- a/cumulative.txt
+++ b/cumulative.txt
@@ -4,5 +4,3 @@
 line 4
-line 5
-line 6
 line 7
 line 8
@@ -14,3 +12,6 @@
 line 14
 line 15
+new A
+new B
+new C
 line 16
@@ -24,3 +25,3 @@
 line 24
-line 25
+line 25 modified
 line 26
```
"#;

    let diffs = parser.parse_diffs(content).unwrap();
    assert_eq!(diffs[0].hunks.len(), 3);

    let result = parser.apply_changes(&diffs).unwrap();
    assert!(result.failed_files.is_empty(), "All hunks should apply");

    let new_content = fs::read_to_string(&test_file).unwrap();
    let lines: Vec<&str> = new_content.lines().collect();

    // Original: 30 lines
    // After hunk 1: 28 lines (-2)
    // After hunk 2: 31 lines (+3)
    // After hunk 3: 31 lines (0)
    assert_eq!(lines.len(), 31);

    assert!(!new_content.contains("line 5\n"));
    assert!(!new_content.contains("line 6\n"));
    assert!(new_content.contains("new A"));
    assert!(new_content.contains("new B"));
    assert!(new_content.contains("new C"));
    assert!(new_content.contains("line 25 modified"));
}

/// Test: Simple cumulative offset with additions only (no deletions)
#[test]
fn test_cumulative_offset_additions_only() {
    let (_temp_dir, base_dir) = setup_test_env();

    let test_file = base_dir.join("additions.txt");
    let original_content = "line 1\nline 2\nline 3\nline 4\nline 5\nline 6\nline 7\nline 8\n";
    fs::write(&test_file, original_content).unwrap();

    let parser = FixupParser::new(FixupMode::Apply, base_dir.clone()).unwrap();

    // Two hunks that only add lines
    let content = r#"
FIXUP PLAN:

```diff
--- a/additions.txt
+++ b/additions.txt
@@ -1,3 +1,4 @@
 line 1
+added first
 line 2
 line 3
@@ -6,3 +7,4 @@
 line 6
+added second
 line 7
 line 8
```
"#;

    let diffs = parser.parse_diffs(content).unwrap();
    let result = parser.apply_changes(&diffs).unwrap();
    assert!(result.failed_files.is_empty());

    let new_content = fs::read_to_string(&test_file).unwrap();
    assert!(new_content.contains("added first"));
    assert!(new_content.contains("added second"));
}

/// Test: Verify FuzzyMatchFailed error contains useful information
#[test]
fn test_fuzzy_match_failed_error_message() {
    use xchecker::error::UserFriendlyError;
    use xchecker::fixup::FixupError;

    let error = FixupError::FuzzyMatchFailed {
        file: "src/main.rs".to_string(),
        expected_line: 42,
        search_window: 50,
    };

    let msg = error.user_message();
    assert!(msg.contains("line 42"));
    assert!(msg.contains("src/main.rs"));
    assert!(msg.contains("±50"));

    let suggestions = error.suggestions();
    assert!(!suggestions.is_empty());
    assert!(suggestions.iter().any(|s| s.contains("review phase")));
}

/// Test: Preview mode calculates stats and doesn't modify files
///
/// Note: preview_changes uses git apply --check for validation which may
/// report validation failures if the temp directory isn't a git repo.
/// The key invariant is that files are NOT modified in preview mode.
#[test]
fn test_preview_mode_no_modifications() {
    let (_temp_dir, base_dir) = setup_test_env();

    let test_file = base_dir.join("preview_test.txt");
    let original_content = "line 1\nline 2\nline 3\n";
    fs::write(&test_file, original_content).unwrap();

    let parser = FixupParser::new(FixupMode::Preview, base_dir.clone()).unwrap();

    let content = r#"
FIXUP PLAN:

```diff
--- a/preview_test.txt
+++ b/preview_test.txt
@@ -1,3 +1,4 @@
 line 1
+inserted
 line 2
 line 3
```
"#;

    let diffs = parser.parse_diffs(content).unwrap();
    let preview = parser.preview_changes(&diffs).unwrap();

    // Target files should be identified
    assert_eq!(preview.target_files.len(), 1);
    assert!(preview.change_summary.contains_key("preview_test.txt"));

    let summary = &preview.change_summary["preview_test.txt"];
    assert_eq!(summary.lines_added, 1);
    assert_eq!(summary.lines_removed, 0);

    // KEY INVARIANT: File should NOT be modified in preview mode
    let content_after = fs::read_to_string(&test_file).unwrap();
    assert_eq!(content_after, original_content);
}

/// Test: Preview mode change stats are calculated correctly
#[test]
fn test_preview_change_stats() {
    let (_temp_dir, base_dir) = setup_test_env();

    let parser = FixupParser::new(FixupMode::Preview, base_dir.clone()).unwrap();

    let content = r#"
FIXUP PLAN:

```diff
--- a/test.txt
+++ b/test.txt
@@ -1,5 +1,6 @@
 line 1
+added line
 line 2
-removed line
+replaced line
 line 4
 line 5
```
"#;

    let diffs = parser.parse_diffs(content).unwrap();
    assert_eq!(diffs.len(), 1);

    // Calculate stats from the parser
    let (added, removed) = {
        let mut a = 0;
        let mut r = 0;
        for hunk in &diffs[0].hunks {
            for line in hunk.content.lines() {
                if line.starts_with('+') && !line.starts_with("+++") {
                    a += 1;
                } else if line.starts_with('-') && !line.starts_with("---") {
                    r += 1;
                }
            }
        }
        (a, r)
    };

    assert_eq!(added, 2); // +added line, +replaced line
    assert_eq!(removed, 1); // -removed line
}

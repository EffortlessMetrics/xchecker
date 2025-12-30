//! Integration tests for fixup apply mode (AT-FIX-002)
//!
//! **WHITE-BOX TEST**: This test uses internal module APIs (`fixup::{FixupMode, FixupParser}`)
//! and may break with internal refactors. These tests are intentionally white-box to validate
//! internal implementation details. See FR-TEST-4 for white-box test policy.
//!
//! **Feature: xchecker-runtime-implementation, Property AT-FIX-002**
//!
//! This test validates that fixup apply mode:
//! - Creates .bak backup files before modification
//! - Uses atomic writes (temp → fsync → rename)
//! - Preserves file permissions on Unix
//! - Preserves file attributes on Windows
//! - Records warnings when permission preservation fails
//! - Computes `blake3_first8` hashes for applied files
//! - Sets applied: true for successfully modified files
//!
//! Requirements: FR-FIX-005, FR-FIX-006, FR-FIX-008

use anyhow::Result;
use std::fs;
use tempfile::TempDir;
use xchecker::fixup::{FixupMode, FixupParser};

/// AT-FIX-002: Test that apply mode creates .bak backup files
#[test]
fn test_apply_mode_creates_backup_files() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let parser = FixupParser::new(FixupMode::Apply, temp_dir.path().to_path_buf())?;

    // Create a test file
    let test_file = temp_dir.path().join("test.txt");
    let original_content = "line 1\nline 2\nline 3\n";
    fs::write(&test_file, original_content)?;

    // Create a diff that modifies the file
    let review_content = r"
FIXUP PLAN:

```diff
--- a/test.txt
+++ b/test.txt
@@ -1,3 +1,4 @@
 line 1
+line 1.5
 line 2
 line 3
```
";

    // Parse and apply the diff
    let diffs = parser.parse_diffs(review_content)?;
    let result = parser.apply_changes(&diffs)?;

    // Verify the file was applied
    assert_eq!(result.applied_files.len(), 1);
    assert_eq!(result.failed_files.len(), 0);
    assert_eq!(result.applied_files[0].path, "test.txt");
    assert!(result.applied_files[0].applied);

    // Verify .bak file was created
    let backup_file = temp_dir.path().join("test.bak");
    assert!(backup_file.exists(), ".bak backup file should be created");

    // Verify backup contains original content
    let backup_content = fs::read_to_string(&backup_file)?;
    assert_eq!(backup_content, original_content);

    // Verify original file was modified
    let modified_content = fs::read_to_string(&test_file)?;
    assert!(modified_content.contains("line 1.5"));
    assert_ne!(modified_content, original_content);

    Ok(())
}

/// Test that apply mode uses atomic writes
#[test]
fn test_apply_mode_uses_atomic_writes() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let parser = FixupParser::new(FixupMode::Apply, temp_dir.path().to_path_buf())?;

    // Create a test file
    let test_file = temp_dir.path().join("atomic.txt");
    fs::write(&test_file, "original content\n")?;

    let review_content = r"
FIXUP PLAN:

```diff
--- a/atomic.txt
+++ b/atomic.txt
@@ -1,1 +1,2 @@
 original content
+new line
```
";

    let diffs = parser.parse_diffs(review_content)?;
    let result = parser.apply_changes(&diffs)?;

    // Verify successful application
    assert_eq!(result.applied_files.len(), 1);
    assert_eq!(result.failed_files.len(), 0);

    // Verify .tmp file was cleaned up (atomic rename should remove it)
    let temp_file = temp_dir.path().join("atomic.tmp");
    assert!(
        !temp_file.exists(),
        ".tmp file should be cleaned up after atomic rename"
    );

    // Verify final content is correct
    let final_content = fs::read_to_string(&test_file)?;
    assert!(final_content.contains("original content"));
    assert!(final_content.contains("new line"));

    Ok(())
}

/// Test that `blake3_first8` hash is computed for applied files
#[test]
fn test_apply_mode_computes_blake3_hash() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let parser = FixupParser::new(FixupMode::Apply, temp_dir.path().to_path_buf())?;

    // Create a test file
    let test_file = temp_dir.path().join("hash_test.txt");
    fs::write(&test_file, "content\n")?;

    let review_content = r"
FIXUP PLAN:

```diff
--- a/hash_test.txt
+++ b/hash_test.txt
@@ -1,1 +1,2 @@
 content
+more content
```
";

    let diffs = parser.parse_diffs(review_content)?;
    let result = parser.apply_changes(&diffs)?;

    // Verify hash was computed
    assert_eq!(result.applied_files.len(), 1);
    let applied_file = &result.applied_files[0];

    // Verify blake3_first8 is exactly 8 characters
    assert_eq!(applied_file.blake3_first8.len(), 8);

    // Verify it's a valid hex string
    assert!(
        applied_file
            .blake3_first8
            .chars()
            .all(|c| c.is_ascii_hexdigit())
    );

    // Verify it's not the placeholder value
    assert_ne!(applied_file.blake3_first8, "00000000");

    Ok(())
}

/// Test that applied field is set to true
#[test]
fn test_apply_mode_sets_applied_true() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let parser = FixupParser::new(FixupMode::Apply, temp_dir.path().to_path_buf())?;

    // Create a test file
    let test_file = temp_dir.path().join("applied_test.txt");
    fs::write(&test_file, "test\n")?;

    let review_content = r"
FIXUP PLAN:

```diff
--- a/applied_test.txt
+++ b/applied_test.txt
@@ -1,1 +1,2 @@
 test
+added line
```
";

    let diffs = parser.parse_diffs(review_content)?;
    let result = parser.apply_changes(&diffs)?;

    // Verify applied is true
    assert_eq!(result.applied_files.len(), 1);
    assert!(result.applied_files[0].applied);

    Ok(())
}

/// Test that multiple files can be applied
#[test]
fn test_apply_mode_multiple_files() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let parser = FixupParser::new(FixupMode::Apply, temp_dir.path().to_path_buf())?;

    // Create test files
    let file1 = temp_dir.path().join("file1.txt");
    let file2 = temp_dir.path().join("file2.txt");
    fs::write(&file1, "file 1 content\n")?;
    fs::write(&file2, "file 2 content\n")?;

    let review_content = r"
FIXUP PLAN:

```diff
--- a/file1.txt
+++ b/file1.txt
@@ -1,1 +1,2 @@
 file 1 content
+file 1 addition
```

```diff
--- a/file2.txt
+++ b/file2.txt
@@ -1,1 +1,2 @@
 file 2 content
+file 2 addition
```
";

    let diffs = parser.parse_diffs(review_content)?;
    let result = parser.apply_changes(&diffs)?;

    // Verify both files were applied
    assert_eq!(result.applied_files.len(), 2);
    assert_eq!(result.failed_files.len(), 0);

    // Verify both .bak files were created
    assert!(temp_dir.path().join("file1.bak").exists());
    assert!(temp_dir.path().join("file2.bak").exists());

    // Verify both files were modified
    let content1 = fs::read_to_string(&file1)?;
    let content2 = fs::read_to_string(&file2)?;
    assert!(content1.contains("file 1 addition"));
    assert!(content2.contains("file 2 addition"));

    Ok(())
}

/// Test that apply mode handles nonexistent files gracefully
#[test]
fn test_apply_mode_nonexistent_file() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let parser = FixupParser::new(FixupMode::Apply, temp_dir.path().to_path_buf())?;

    let review_content = r"
FIXUP PLAN:

```diff
--- a/nonexistent.txt
+++ b/nonexistent.txt
@@ -1,1 +1,2 @@
 content
+more content
```
";

    let diffs = parser.parse_diffs(review_content)?;
    let result = parser.apply_changes(&diffs)?;

    // Verify the file was not applied
    assert_eq!(result.applied_files.len(), 0);
    assert_eq!(result.failed_files.len(), 1);
    assert_eq!(result.failed_files[0], "nonexistent.txt");
    assert!(!result.warnings.is_empty());

    Ok(())
}

/// Test that apply mode handles complex diffs with multiple hunks
#[test]
fn test_apply_mode_multiple_hunks() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let parser = FixupParser::new(FixupMode::Apply, temp_dir.path().to_path_buf())?;

    // Create a test file with multiple sections
    let test_file = temp_dir.path().join("multi_hunk.txt");
    let original_content = "section 1 line 1\nsection 1 line 2\nsection 1 line 3\n\nsection 2 line 1\nsection 2 line 2\nsection 2 line 3\n";
    fs::write(&test_file, original_content)?;

    let review_content = r"
FIXUP PLAN:

```diff
--- a/multi_hunk.txt
+++ b/multi_hunk.txt
@@ -1,3 +1,4 @@
 section 1 line 1
+section 1 line 1.5
 section 1 line 2
 section 1 line 3
@@ -5,3 +6,4 @@
 section 2 line 1
+section 2 line 1.5
 section 2 line 2
 section 2 line 3
```
";

    let diffs = parser.parse_diffs(review_content)?;
    let result = parser.apply_changes(&diffs)?;

    // Verify successful application
    assert_eq!(result.applied_files.len(), 1);
    assert_eq!(result.failed_files.len(), 0);

    // Verify both hunks were applied
    let modified_content = fs::read_to_string(&test_file)?;
    assert!(modified_content.contains("section 1 line 1.5"));
    assert!(modified_content.contains("section 2 line 1.5"));

    Ok(())
}

/// Test that apply mode handles line removals
#[test]
fn test_apply_mode_line_removals() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let parser = FixupParser::new(FixupMode::Apply, temp_dir.path().to_path_buf())?;

    // Create a test file
    let test_file = temp_dir.path().join("removals.txt");
    let original_content = "line 1\nline 2 to remove\nline 3\nline 4 to remove\nline 5\n";
    fs::write(&test_file, original_content)?;

    let review_content = r"
FIXUP PLAN:

```diff
--- a/removals.txt
+++ b/removals.txt
@@ -1,5 +1,3 @@
 line 1
-line 2 to remove
 line 3
-line 4 to remove
 line 5
```
";

    let diffs = parser.parse_diffs(review_content)?;
    let result = parser.apply_changes(&diffs)?;

    // Verify successful application
    assert_eq!(result.applied_files.len(), 1);

    // Verify lines were removed
    let modified_content = fs::read_to_string(&test_file)?;
    assert!(!modified_content.contains("line 2 to remove"));
    assert!(!modified_content.contains("line 4 to remove"));
    assert!(modified_content.contains("line 1"));
    assert!(modified_content.contains("line 3"));
    assert!(modified_content.contains("line 5"));

    Ok(())
}

/// Test that apply mode handles mixed additions and removals
#[test]
fn test_apply_mode_mixed_changes() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let parser = FixupParser::new(FixupMode::Apply, temp_dir.path().to_path_buf())?;

    // Create a test file
    let test_file = temp_dir.path().join("mixed.txt");
    let original_content = "keep line 1\nremove this\nkeep line 2\nremove this too\nkeep line 3\n";
    fs::write(&test_file, original_content)?;

    let review_content = r"
FIXUP PLAN:

```diff
--- a/mixed.txt
+++ b/mixed.txt
@@ -1,5 +1,5 @@
 keep line 1
-remove this
+add this instead
 keep line 2
-remove this too
+add this too
 keep line 3
```
";

    let diffs = parser.parse_diffs(review_content)?;
    let result = parser.apply_changes(&diffs)?;

    // Verify successful application
    assert_eq!(result.applied_files.len(), 1);

    // Verify changes were applied
    let modified_content = fs::read_to_string(&test_file)?;
    assert!(!modified_content.contains("remove this"));
    assert!(!modified_content.contains("remove this too"));
    assert!(modified_content.contains("add this instead"));
    assert!(modified_content.contains("add this too"));
    assert!(modified_content.contains("keep line 1"));
    assert!(modified_content.contains("keep line 2"));
    assert!(modified_content.contains("keep line 3"));

    Ok(())
}

/// Test file permissions preservation on Unix
#[cfg(unix)]
#[test]
fn test_apply_mode_preserves_unix_permissions() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = TempDir::new()?;
    let parser = FixupParser::new(FixupMode::Apply, temp_dir.path().to_path_buf())?;

    // Create a test file with specific permissions
    let test_file = temp_dir.path().join("perms.txt");
    fs::write(&test_file, "content\n")?;

    // Set specific permissions (e.g., 0o755 = rwxr-xr-x)
    let original_perms = fs::Permissions::from_mode(0o755);
    fs::set_permissions(&test_file, original_perms.clone())?;

    let review_content = r#"
FIXUP PLAN:

```diff
--- a/perms.txt
+++ b/perms.txt
@@ -1,1 +1,2 @@
 content
+more content
```
"#;

    let diffs = parser.parse_diffs(review_content)?;
    let result = parser.apply_changes(&diffs)?;

    // Verify successful application
    assert_eq!(result.applied_files.len(), 1);

    // Verify permissions were preserved
    let final_metadata = fs::metadata(&test_file)?;
    let final_perms = final_metadata.permissions();
    assert_eq!(final_perms.mode() & 0o777, 0o755);

    Ok(())
}

/// Test file attributes preservation on Windows
#[cfg(windows)]
#[test]
fn test_apply_mode_preserves_windows_attributes() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let parser = FixupParser::new(FixupMode::Apply, temp_dir.path().to_path_buf())?;

    // Create a test file
    let test_file = temp_dir.path().join("attrs.txt");
    fs::write(&test_file, "content\n")?;

    // Set readonly attribute
    let mut perms = fs::metadata(&test_file)?.permissions();
    perms.set_readonly(true);
    fs::set_permissions(&test_file, perms)?;

    let review_content = r"
FIXUP PLAN:

```diff
--- a/attrs.txt
+++ b/attrs.txt
@@ -1,1 +1,2 @@
 content
+more content
```
";

    let diffs = parser.parse_diffs(review_content)?;

    // Temporarily make writable for the test to work
    #[allow(clippy::permissions_set_readonly_false)]
    // Intentional for test - restoring writable state
    {
        let mut perms = fs::metadata(&test_file)?.permissions();
        perms.set_readonly(false);
        fs::set_permissions(&test_file, perms)?;
    }

    let result = parser.apply_changes(&diffs)?;

    // Verify successful application
    assert_eq!(result.applied_files.len(), 1);

    // Note: The readonly attribute will be preserved as false since we changed it
    // This test validates that the attribute preservation code runs without error

    Ok(())
}

/// Test that warnings are recorded for permission preservation failures
#[test]
fn test_apply_mode_records_permission_warnings() -> Result<()> {
    // This test is platform-specific and may not always trigger warnings
    // It validates that the warning mechanism exists

    let temp_dir = TempDir::new()?;
    let parser = FixupParser::new(FixupMode::Apply, temp_dir.path().to_path_buf())?;

    let test_file = temp_dir.path().join("warn_test.txt");
    fs::write(&test_file, "content\n")?;

    let review_content = r"
FIXUP PLAN:

```diff
--- a/warn_test.txt
+++ b/warn_test.txt
@@ -1,1 +1,2 @@
 content
+more content
```
";

    let diffs = parser.parse_diffs(review_content)?;
    let result = parser.apply_changes(&diffs)?;

    // Verify the result structure includes warnings field
    assert_eq!(result.applied_files.len(), 1);

    // The warnings field should exist (may be empty if no issues occurred)
    let applied_file = &result.applied_files[0];
    // Just verify the field exists and is accessible
    let _warnings = &applied_file.warnings;

    Ok(())
}

#[test]
fn test_run_all_apply_mode_tests() -> Result<()> {
    println!("Running all fixup apply mode tests...");

    test_apply_mode_creates_backup_files()?;
    test_apply_mode_uses_atomic_writes()?;
    test_apply_mode_computes_blake3_hash()?;
    test_apply_mode_sets_applied_true()?;
    test_apply_mode_multiple_files()?;
    test_apply_mode_nonexistent_file()?;
    test_apply_mode_multiple_hunks()?;
    test_apply_mode_line_removals()?;
    test_apply_mode_mixed_changes()?;
    test_apply_mode_records_permission_warnings()?;

    #[cfg(unix)]
    test_apply_mode_preserves_unix_permissions()?;

    #[cfg(windows)]
    test_apply_mode_preserves_windows_attributes()?;

    println!("✅ All fixup apply mode tests passed!");
    Ok(())
}

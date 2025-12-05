//! Integration tests for cross-platform line ending handling (FR-FS-004, FR-FS-005)
//!
//! This test file validates that:
//! - CRLF tolerance on read (Windows) - FR-FS-005
//! - LF enforcement on write (all platforms) - FR-FS-004
//! - Line ending normalization works correctly
//! - JSON files are written with LF
//! - Text files are written with LF
//!
//! Requirements tested:
//! - FR-FS-004: UTF-8 encoding with LF line endings
//! - FR-FS-005: CRLF tolerance on read (Windows)

use anyhow::Result;
use camino::Utf8Path;
use std::fs;
use tempfile::TempDir;
use xchecker::atomic_write::{read_file_with_crlf_tolerance, write_file_atomic};
use xchecker::canonicalization::Canonicalizer;
use xchecker::receipt::ReceiptManager;
use xchecker::types::{PhaseId, Receipt};

// ============================================================================
// Unit Tests: Line Ending Normalization
// ============================================================================

#[test]
fn test_atomic_write_normalizes_crlf_to_lf() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let path_buf = temp_dir.path().join("test_crlf.txt");
    let file_path = Utf8Path::from_path(path_buf.as_path()).unwrap();

    // Write content with CRLF line endings
    let content_with_crlf = "line1\r\nline2\r\nline3\r\n";
    write_file_atomic(file_path, content_with_crlf)?;

    // Read back and verify LF line endings
    let read_content = fs::read_to_string(file_path.as_std_path())?;
    assert_eq!(read_content, "line1\nline2\nline3\n");
    assert!(!read_content.contains("\r\n"), "Should not contain CRLF");
    assert!(!read_content.contains("\r"), "Should not contain CR");

    Ok(())
}

#[test]
fn test_atomic_write_normalizes_cr_to_lf() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let path_buf = temp_dir.path().join("test_cr.txt");
    let file_path = Utf8Path::from_path(path_buf.as_path()).unwrap();

    // Write content with CR line endings (old Mac style)
    let content_with_cr = "line1\rline2\rline3\r";
    write_file_atomic(file_path, content_with_cr)?;

    // Read back and verify LF line endings
    let read_content = fs::read_to_string(file_path.as_std_path())?;
    assert_eq!(read_content, "line1\nline2\nline3\n");
    assert!(!read_content.contains("\r"), "Should not contain CR");

    Ok(())
}

#[test]
fn test_atomic_write_normalizes_mixed_line_endings() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let path_buf = temp_dir.path().join("test_mixed.txt");
    let file_path = Utf8Path::from_path(path_buf.as_path()).unwrap();

    // Write content with mixed line endings
    let content_mixed = "line1\r\nline2\nline3\rline4\n";
    write_file_atomic(file_path, content_mixed)?;

    // Read back and verify all normalized to LF
    let read_content = fs::read_to_string(file_path.as_std_path())?;
    assert_eq!(read_content, "line1\nline2\nline3\nline4\n");
    assert!(!read_content.contains("\r\n"), "Should not contain CRLF");
    assert!(!read_content.contains("\r"), "Should not contain CR");

    Ok(())
}

#[test]
fn test_atomic_write_preserves_lf() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let path_buf = temp_dir.path().join("test_lf.txt");
    let file_path = Utf8Path::from_path(path_buf.as_path()).unwrap();

    // Write content with LF line endings
    let content_lf = "line1\nline2\nline3\n";
    write_file_atomic(file_path, content_lf)?;

    // Read back and verify LF preserved
    let read_content = fs::read_to_string(file_path.as_std_path())?;
    assert_eq!(read_content, content_lf);

    Ok(())
}

// ============================================================================
// Integration Tests: CRLF Tolerance on Read
// ============================================================================

#[test]
fn test_read_file_with_crlf_tolerance_basic() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let path_buf = temp_dir.path().join("crlf_file.txt");
    let file_path = Utf8Path::from_path(path_buf.as_path()).unwrap();

    // Write file with CRLF directly (bypassing atomic_write)
    let content_with_crlf = b"line1\r\nline2\r\nline3\r\n";
    fs::write(file_path.as_std_path(), content_with_crlf)?;

    // Read with CRLF tolerance
    let content = read_file_with_crlf_tolerance(file_path)?;

    // Should be normalized to LF
    assert_eq!(content, "line1\nline2\nline3\n");
    assert!(!content.contains("\r"), "Should not contain CR");

    Ok(())
}

#[test]
fn test_read_file_with_crlf_tolerance_mixed() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let path_buf = temp_dir.path().join("mixed_file.txt");
    let file_path = Utf8Path::from_path(path_buf.as_path()).unwrap();

    // Write file with mixed line endings directly
    let content_mixed = b"line1\r\nline2\nline3\rline4\n";
    fs::write(file_path.as_std_path(), content_mixed)?;

    // Read with CRLF tolerance
    let content = read_file_with_crlf_tolerance(file_path)?;

    // Should be normalized to LF
    assert_eq!(content, "line1\nline2\nline3\nline4\n");
    assert!(!content.contains("\r"), "Should not contain CR");

    Ok(())
}

#[test]
fn test_read_file_with_crlf_tolerance_empty() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let path_buf = temp_dir.path().join("empty_file.txt");
    let file_path = Utf8Path::from_path(path_buf.as_path()).unwrap();

    // Write empty file
    fs::write(file_path.as_std_path(), b"")?;

    // Read with CRLF tolerance
    let content = read_file_with_crlf_tolerance(file_path)?;

    assert_eq!(content, "");

    Ok(())
}

// ============================================================================
// Integration Tests: JSON Files Written with LF
// ============================================================================

#[test]
fn test_json_receipt_written_with_lf() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let spec_base_path_buf = temp_dir.path().join("spec");
    let spec_base_path = Utf8Path::from_path(spec_base_path_buf.as_path()).unwrap();

    // Create receipt manager
    let receipt_manager = ReceiptManager::new(&spec_base_path.to_path_buf());

    // Create a test receipt
    let receipt = Receipt {
        schema_version: "1".to_string(),
        emitted_at: chrono::Utc::now(),
        spec_id: "test-spec".to_string(),
        phase: PhaseId::Requirements.as_str().to_string(),
        xchecker_version: "0.1.0".to_string(),
        claude_cli_version: "1.0.0".to_string(),
        model_full_name: "haiku".to_string(),
        model_alias: None,
        canonicalization_version: "1".to_string(),
        canonicalization_backend: "jcs-rfc8785".to_string(),
        flags: std::collections::HashMap::new(),
        runner: "test-runner".to_string(),
        runner_distro: None,
        packet: xchecker::types::PacketEvidence {
            files: vec![],
            max_bytes: 65536,
            max_lines: 1200,
        },
        outputs: vec![],
        exit_code: 0,
        error_kind: None,
        error_reason: None,
        stderr_tail: None,
        stderr_redacted: None,
        warnings: vec![],
        fallback_used: None,
        diff_context: None,

        llm: None,
        pipeline: None,
    };

    // Write receipt
    let receipt_path = receipt_manager.write_receipt(&receipt)?;

    // Read the raw bytes
    let raw_bytes = fs::read(receipt_path.as_std_path())?;

    // Verify no CRLF in the file
    let has_crlf = raw_bytes.windows(2).any(|w| w == b"\r\n");
    assert!(
        !has_crlf,
        "JSON receipt should not contain CRLF line endings"
    );

    // Verify the file contains LF (if it has multiple lines)
    let content = String::from_utf8(raw_bytes)?;
    if content.contains('\n') {
        assert!(!content.contains("\r\n"), "Should not contain CRLF");
        assert!(!content.contains('\r'), "Should not contain CR");
    }

    Ok(())
}

#[test]
fn test_json_canonicalization_produces_lf() -> Result<()> {
    let _canonicalizer = Canonicalizer::new();

    // Create JSON with potential line ending issues
    let json_value = serde_json::json!({
        "field1": "value1",
        "field2": "value2",
        "field3": "value3",
    });

    // Canonicalize
    let canonical_bytes = serde_json_canonicalizer::to_vec(&json_value)?;
    let canonical_str = String::from_utf8(canonical_bytes)?;

    // Verify no CRLF
    assert!(
        !canonical_str.contains("\r\n"),
        "Canonical JSON should not contain CRLF"
    );
    assert!(
        !canonical_str.contains('\r'),
        "Canonical JSON should not contain CR"
    );

    Ok(())
}

#[test]
fn test_json_file_roundtrip_preserves_lf() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let path_buf = temp_dir.path().join("test.json");
    let file_path = Utf8Path::from_path(path_buf.as_path()).unwrap();

    // Create JSON content
    let json_content = r#"{
  "key1": "value1",
  "key2": "value2",
  "key3": "value3"
}"#;

    // Write using atomic_write
    write_file_atomic(file_path, json_content)?;

    // Read back
    let read_content = fs::read_to_string(file_path.as_std_path())?;

    // Verify LF line endings
    assert!(!read_content.contains("\r\n"), "Should not contain CRLF");
    assert!(!read_content.contains('\r'), "Should not contain CR");
    assert!(read_content.contains('\n'), "Should contain LF");

    Ok(())
}

// ============================================================================
// Integration Tests: Text Files Written with LF
// ============================================================================

#[test]
fn test_text_file_written_with_lf() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let path_buf = temp_dir.path().join("test.txt");
    let file_path = Utf8Path::from_path(path_buf.as_path()).unwrap();

    // Write text content with various line endings
    let text_content = "This is line 1\r\nThis is line 2\nThis is line 3\r";
    write_file_atomic(file_path, text_content)?;

    // Read raw bytes
    let raw_bytes = fs::read(file_path.as_std_path())?;

    // Verify no CRLF in the file
    let has_crlf = raw_bytes.windows(2).any(|w| w == b"\r\n");
    assert!(!has_crlf, "Text file should not contain CRLF line endings");

    // Verify content
    let content = String::from_utf8(raw_bytes)?;
    assert_eq!(content, "This is line 1\nThis is line 2\nThis is line 3\n");

    Ok(())
}

#[test]
fn test_markdown_file_written_with_lf() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let path_buf = temp_dir.path().join("test.md");
    let file_path = Utf8Path::from_path(path_buf.as_path()).unwrap();

    // Write markdown content
    let markdown_content = "# Header\r\n\r\nParagraph 1\r\n\r\nParagraph 2\n";
    write_file_atomic(file_path, markdown_content)?;

    // Read back
    let content = fs::read_to_string(file_path.as_std_path())?;

    // Verify LF line endings
    assert!(!content.contains("\r\n"), "Should not contain CRLF");
    assert!(!content.contains('\r'), "Should not contain CR");
    assert_eq!(content, "# Header\n\nParagraph 1\n\nParagraph 2\n");

    Ok(())
}

#[test]
fn test_yaml_file_written_with_lf() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let path_buf = temp_dir.path().join("test.yaml");
    let file_path = Utf8Path::from_path(path_buf.as_path()).unwrap();

    // Write YAML content
    let yaml_content = "key1: value1\r\nkey2: value2\r\nkey3:\r\n  - item1\r\n  - item2\n";
    write_file_atomic(file_path, yaml_content)?;

    // Read back
    let content = fs::read_to_string(file_path.as_std_path())?;

    // Verify LF line endings
    assert!(!content.contains("\r\n"), "Should not contain CRLF");
    assert!(!content.contains('\r'), "Should not contain CR");
    assert_eq!(
        content,
        "key1: value1\nkey2: value2\nkey3:\n  - item1\n  - item2\n"
    );

    Ok(())
}

// ============================================================================
// Integration Tests: Unicode and Special Characters
// ============================================================================

#[test]
fn test_unicode_content_with_line_endings() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let path_buf = temp_dir.path().join("unicode.txt");
    let file_path = Utf8Path::from_path(path_buf.as_path()).unwrap();

    // Write unicode content with CRLF
    let unicode_content = "Hello 世界\r\nПривет мир\r\nمرحبا العالم\r\n🌍🌎🌏\r\n";
    write_file_atomic(file_path, unicode_content)?;

    // Read back
    let content = fs::read_to_string(file_path.as_std_path())?;

    // Verify LF line endings and unicode preserved
    assert!(!content.contains("\r\n"), "Should not contain CRLF");
    assert!(!content.contains('\r'), "Should not contain CR");
    assert_eq!(content, "Hello 世界\nПривет мир\nمرحبا العالم\n🌍🌎🌏\n");

    Ok(())
}

#[test]
fn test_special_characters_with_line_endings() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let path_buf = temp_dir.path().join("special.txt");
    let file_path = Utf8Path::from_path(path_buf.as_path()).unwrap();

    // Write content with special characters and CRLF
    let special_content = "tab:\t\r\nquote:\"\r\nbackslash:\\\r\nnewline in string: \\n\r\n";
    write_file_atomic(file_path, special_content)?;

    // Read back
    let content = fs::read_to_string(file_path.as_std_path())?;

    // Verify LF line endings and special chars preserved
    assert!(!content.contains("\r\n"), "Should not contain CRLF");
    assert!(!content.contains('\r'), "Should not contain CR");
    assert!(content.contains('\t'), "Should preserve tab");
    assert!(content.contains('"'), "Should preserve quote");
    assert!(content.contains('\\'), "Should preserve backslash");

    Ok(())
}

// ============================================================================
// Integration Tests: Large Files
// ============================================================================

#[test]
fn test_large_file_line_ending_normalization() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let path_buf = temp_dir.path().join("large.txt");
    let file_path = Utf8Path::from_path(path_buf.as_path()).unwrap();

    // Create large content with CRLF (1000 lines)
    let mut large_content = String::new();
    for i in 0..1000 {
        large_content.push_str(&format!("Line {}\r\n", i));
    }

    write_file_atomic(file_path, &large_content)?;

    // Read back
    let content = fs::read_to_string(file_path.as_std_path())?;

    // Verify no CRLF in output
    assert!(!content.contains("\r\n"), "Should not contain CRLF");
    assert!(!content.contains('\r'), "Should not contain CR");

    // Verify correct number of lines
    let line_count = content.lines().count();
    assert_eq!(line_count, 1000, "Should have 1000 lines");

    Ok(())
}

// ============================================================================
// Integration Tests: Cross-Platform Compatibility
// ============================================================================

#[test]
fn test_write_then_read_with_tolerance() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let path_buf = temp_dir.path().join("roundtrip.txt");
    let file_path = Utf8Path::from_path(path_buf.as_path()).unwrap();

    // Write content with CRLF
    let original_content = "line1\r\nline2\r\nline3\r\n";
    write_file_atomic(file_path, original_content)?;

    // Read with tolerance
    let read_content = read_file_with_crlf_tolerance(file_path)?;

    // Should be normalized
    assert_eq!(read_content, "line1\nline2\nline3\n");

    Ok(())
}

#[test]
fn test_multiple_writes_preserve_lf() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let path_buf = temp_dir.path().join("multi_write.txt");
    let file_path = Utf8Path::from_path(path_buf.as_path()).unwrap();

    // First write with CRLF
    write_file_atomic(file_path, "content1\r\n")?;

    // Second write with CRLF
    write_file_atomic(file_path, "content2\r\n")?;

    // Third write with CRLF
    write_file_atomic(file_path, "content3\r\n")?;

    // Read back
    let content = fs::read_to_string(file_path.as_std_path())?;

    // Should always be LF
    assert_eq!(content, "content3\n");
    assert!(!content.contains("\r"), "Should not contain CR");

    Ok(())
}

#[test]
fn test_empty_lines_with_different_endings() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let path_buf = temp_dir.path().join("empty_lines.txt");
    let file_path = Utf8Path::from_path(path_buf.as_path()).unwrap();

    // Write content with empty lines and mixed endings
    let content = "line1\r\n\r\nline3\n\nline5\r";
    write_file_atomic(file_path, content)?;

    // Read back
    let read_content = fs::read_to_string(file_path.as_std_path())?;

    // Verify normalization
    assert_eq!(read_content, "line1\n\nline3\n\nline5\n");
    assert!(!read_content.contains("\r"), "Should not contain CR");

    Ok(())
}

// ============================================================================
// Comprehensive Test Runner
// ============================================================================

#[test]
fn test_comprehensive_cross_platform_line_endings() {
    println!("🚀 Running comprehensive cross-platform line ending tests...");
    println!();

    println!("Unit Tests:");
    test_atomic_write_normalizes_crlf_to_lf().unwrap();
    println!("  ✓ Atomic write normalizes CRLF to LF");

    test_atomic_write_normalizes_cr_to_lf().unwrap();
    println!("  ✓ Atomic write normalizes CR to LF");

    test_atomic_write_normalizes_mixed_line_endings().unwrap();
    println!("  ✓ Atomic write normalizes mixed line endings");

    test_atomic_write_preserves_lf().unwrap();
    println!("  ✓ Atomic write preserves LF");

    println!();
    println!("CRLF Tolerance Tests:");
    test_read_file_with_crlf_tolerance_basic().unwrap();
    println!("  ✓ Read file with CRLF tolerance (basic)");

    test_read_file_with_crlf_tolerance_mixed().unwrap();
    println!("  ✓ Read file with CRLF tolerance (mixed)");

    test_read_file_with_crlf_tolerance_empty().unwrap();
    println!("  ✓ Read file with CRLF tolerance (empty)");

    println!();
    println!("JSON File Tests:");
    test_json_receipt_written_with_lf().unwrap();
    println!("  ✓ JSON receipt written with LF");

    test_json_canonicalization_produces_lf().unwrap();
    println!("  ✓ JSON canonicalization produces LF");

    test_json_file_roundtrip_preserves_lf().unwrap();
    println!("  ✓ JSON file roundtrip preserves LF");

    println!();
    println!("Text File Tests:");
    test_text_file_written_with_lf().unwrap();
    println!("  ✓ Text file written with LF");

    test_markdown_file_written_with_lf().unwrap();
    println!("  ✓ Markdown file written with LF");

    test_yaml_file_written_with_lf().unwrap();
    println!("  ✓ YAML file written with LF");

    println!();
    println!("Unicode and Special Characters:");
    test_unicode_content_with_line_endings().unwrap();
    println!("  ✓ Unicode content with line endings");

    test_special_characters_with_line_endings().unwrap();
    println!("  ✓ Special characters with line endings");

    println!();
    println!("Large File Tests:");
    test_large_file_line_ending_normalization().unwrap();
    println!("  ✓ Large file line ending normalization");

    println!();
    println!("Cross-Platform Compatibility:");
    test_write_then_read_with_tolerance().unwrap();
    println!("  ✓ Write then read with tolerance");

    test_multiple_writes_preserve_lf().unwrap();
    println!("  ✓ Multiple writes preserve LF");

    test_empty_lines_with_different_endings().unwrap();
    println!("  ✓ Empty lines with different endings");

    println!();
    println!("✅ All cross-platform line ending tests passed!");
    println!();
    println!("Requirements Validated:");
    println!("  ✓ FR-FS-004: UTF-8 encoding with LF line endings");
    println!("  ✓ FR-FS-005: CRLF tolerance on read (Windows)");
}

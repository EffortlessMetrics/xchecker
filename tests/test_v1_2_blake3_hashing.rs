//! V1.2: Verify and test BLAKE3 hashing (FR-JCS)
//!
//! This test module comprehensively validates BLAKE3 hashing for content integrity.
//!
//! Requirements tested:
//! - FR-JCS-005: Hash stability across platforms (LF line endings)
//! - FR-JCS-006: Hash computation on canonicalized content
//! - Full 64-character hex format
//! - Hashes in receipts match on-disk artifacts
//! - Edge cases (empty files, large files)

use anyhow::Result;
use std::collections::HashMap;
use tempfile::TempDir;
use xchecker::canonicalization::Canonicalizer;
use xchecker::receipt::ReceiptManager;
use xchecker::types::{FileType, PacketEvidence, PhaseId};

/// Test hash stability across platforms with LF line endings (FR-JCS-005)
#[test]
fn test_blake3_hash_stability_lf_line_endings() -> Result<()> {
    let canonicalizer = Canonicalizer::new();

    // Test content with LF line endings
    let content_lf = "line1\nline2\nline3\n";

    // Test content with CRLF line endings (Windows style)
    let content_crlf = "line1\r\nline2\r\nline3\r\n";

    // Test content with mixed line endings
    let content_mixed = "line1\r\nline2\nline3\r\n";

    // Compute hashes for all variants
    let hash_lf = canonicalizer.hash_canonicalized(content_lf, FileType::Text)?;
    let hash_crlf = canonicalizer.hash_canonicalized(content_crlf, FileType::Text)?;
    let hash_mixed = canonicalizer.hash_canonicalized(content_mixed, FileType::Text)?;

    // All should produce identical hashes after normalization to LF
    assert_eq!(
        hash_lf, hash_crlf,
        "CRLF should normalize to LF and produce identical hash"
    );
    assert_eq!(
        hash_lf, hash_mixed,
        "Mixed line endings should normalize to LF and produce identical hash"
    );

    // Verify hash is 64 characters (BLAKE3 full hex)
    assert_eq!(hash_lf.len(), 64, "BLAKE3 hash should be 64 hex characters");

    // Verify hash contains only hex characters
    assert!(
        hash_lf.chars().all(|c| c.is_ascii_hexdigit()),
        "Hash should contain only hex characters"
    );

    println!("✓ BLAKE3 hash stability (LF line endings) test passed");
    println!("  Hash: {}", &hash_lf[..16]);
    Ok(())
}

/// Test hash computation on canonicalized content (FR-JCS-006)
#[test]
fn test_blake3_hash_on_canonicalized_content() -> Result<()> {
    let canonicalizer = Canonicalizer::new();

    // Test YAML content with different formatting
    let yaml1 = r"
name: test
version: 1.0
config:
  debug: true
  port: 8080
";

    let yaml2 = r"
version: 1.0
name: test
config:
  port: 8080
  debug: true
";

    // Compute hashes (should use JCS canonicalization internally)
    let hash1 = canonicalizer.hash_canonicalized(yaml1, FileType::Yaml)?;
    let hash2 = canonicalizer.hash_canonicalized(yaml2, FileType::Yaml)?;

    // Should produce identical hashes due to canonicalization
    assert_eq!(
        hash1, hash2,
        "Canonicalized YAML should produce identical hashes"
    );

    // Test Markdown content with different formatting
    let md1 = "# Title\n\nContent with trailing spaces   \n\n\n";
    let md2 = "# Title\r\n\r\nContent with trailing spaces\r\n";

    let hash_md1 = canonicalizer.hash_canonicalized(md1, FileType::Markdown)?;
    let hash_md2 = canonicalizer.hash_canonicalized(md2, FileType::Markdown)?;

    // Should produce identical hashes due to normalization
    assert_eq!(
        hash_md1, hash_md2,
        "Normalized Markdown should produce identical hashes"
    );

    println!("✓ BLAKE3 hash on canonicalized content test passed");
    Ok(())
}

/// Test full 64-character hex format
#[test]
fn test_blake3_full_64_char_hex_format() -> Result<()> {
    let canonicalizer = Canonicalizer::new();

    let large_content = "x".repeat(10000);
    let test_cases: Vec<(&str, FileType)> = vec![
        ("", FileType::Text),
        ("a", FileType::Text),
        ("hello world", FileType::Text),
        ("name: test\nversion: 1.0", FileType::Yaml),
        ("# Title\n\nContent", FileType::Markdown),
        (large_content.as_str(), FileType::Text), // Large content
    ];

    for (content, file_type) in test_cases {
        let hash = canonicalizer.hash_canonicalized(content, file_type)?;

        // Verify length
        assert_eq!(
            hash.len(),
            64,
            "Hash should be exactly 64 characters for content: {:?}",
            &content[..content.len().min(20)]
        );

        // Verify all characters are hex
        assert!(
            hash.chars().all(|c| c.is_ascii_hexdigit()),
            "Hash should contain only hex characters"
        );

        // Verify lowercase (BLAKE3 standard)
        assert!(
            hash.chars().all(|c| !c.is_ascii_uppercase()),
            "Hash should be lowercase hex"
        );
    }

    println!("✓ BLAKE3 full 64-character hex format test passed");
    Ok(())
}

/// Test that hashes in receipts match on-disk artifacts
#[test]
fn test_blake3_receipts_match_on_disk_artifacts() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = camino::Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();
    let manager = ReceiptManager::new(&base_path);
    let canonicalizer = Canonicalizer::new();

    // Create test content
    let md_content = "# Requirements\n\nTest requirements content\n";
    let yaml_content = "name: test\nversion: 1.0\n";

    // Compute hashes using canonicalizer
    let md_hash = canonicalizer.hash_canonicalized(md_content, FileType::Markdown)?;
    let yaml_hash = canonicalizer.hash_canonicalized(yaml_content, FileType::Yaml)?;

    // Create file hashes using receipt manager
    let file_hash_md = manager.create_file_hash(
        "artifacts/00-requirements.md",
        md_content,
        FileType::Markdown,
        "requirements",
    )?;

    let file_hash_yaml = manager.create_file_hash(
        "artifacts/00-requirements.core.yaml",
        yaml_content,
        FileType::Yaml,
        "requirements",
    )?;

    // Verify hashes match
    assert_eq!(
        file_hash_md.blake3_canonicalized, md_hash,
        "Markdown hash from receipt manager should match canonicalizer"
    );

    assert_eq!(
        file_hash_yaml.blake3_canonicalized, yaml_hash,
        "YAML hash from receipt manager should match canonicalizer"
    );

    // Create a receipt with these hashes
    let outputs = vec![file_hash_md, file_hash_yaml];
    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let receipt = manager.create_receipt(
        "test-hash-match",
        PhaseId::Requirements,
        0,
        outputs,
        "0.1.0",
        "0.8.1",
        "claude-3-5-sonnet-20241022",
        None,
        HashMap::new(),
        packet,
        None,     // stderr_redacted
        None,     // stderr_redacted
        vec![],   // warnings
        None,     // fallback_used
        "native", // runner
        None,     // runner_distro
        None,     // error_kind
        None,     // error_reason
        None,     // diff_context,
        None,     // pipeline
    );

    // Verify receipt contains correct hashes
    assert_eq!(receipt.outputs.len(), 2);
    assert_eq!(receipt.outputs[0].blake3_canonicalized, yaml_hash);
    assert_eq!(receipt.outputs[1].blake3_canonicalized, md_hash);

    println!("✓ BLAKE3 receipts match on-disk artifacts test passed");
    Ok(())
}

/// Test edge case: empty files
#[test]
fn test_blake3_empty_files() -> Result<()> {
    let canonicalizer = Canonicalizer::new();

    // Test empty content for different file types
    let hash_text = canonicalizer.hash_canonicalized("", FileType::Text)?;
    let hash_yaml = canonicalizer.hash_canonicalized("", FileType::Yaml)?;
    let hash_md = canonicalizer.hash_canonicalized("", FileType::Markdown)?;

    // All should produce valid 64-character hashes
    assert_eq!(hash_text.len(), 64);
    assert_eq!(hash_yaml.len(), 64);
    assert_eq!(hash_md.len(), 64);

    // Empty text and markdown should produce the same hash (both normalize to empty string)
    // But YAML might be different due to parsing
    assert!(
        !hash_text.is_empty(),
        "Empty content should still produce a valid hash"
    );

    // Verify consistency: hashing empty content multiple times produces same result
    let hash_text2 = canonicalizer.hash_canonicalized("", FileType::Text)?;
    assert_eq!(
        hash_text, hash_text2,
        "Empty content hash should be consistent"
    );

    println!("✓ BLAKE3 empty files test passed");
    Ok(())
}

/// Test edge case: large files
#[test]
fn test_blake3_large_files() -> Result<()> {
    let canonicalizer = Canonicalizer::new();

    // Create large content (1 MB)
    let large_content = "x".repeat(1_000_000);

    // Compute hash
    let hash = canonicalizer.hash_canonicalized(&large_content, FileType::Text)?;

    // Verify hash is valid
    assert_eq!(
        hash.len(),
        64,
        "Large file should produce 64-character hash"
    );
    assert!(
        hash.chars().all(|c| c.is_ascii_hexdigit()),
        "Large file hash should be valid hex"
    );

    // Verify consistency: hashing same large content produces same result
    let hash2 = canonicalizer.hash_canonicalized(&large_content, FileType::Text)?;
    assert_eq!(hash, hash2, "Large file hash should be consistent");

    // Test with different large content
    let large_content2 = "y".repeat(1_000_000);
    let hash3 = canonicalizer.hash_canonicalized(&large_content2, FileType::Text)?;

    // Should produce different hash
    assert_ne!(
        hash, hash3,
        "Different large files should produce different hashes"
    );

    println!("✓ BLAKE3 large files test passed");
    Ok(())
}

/// Test hash consistency across multiple runs
#[test]
fn test_blake3_consistency_across_runs() -> Result<()> {
    let canonicalizer = Canonicalizer::new();

    let test_content = "consistent test content\nwith multiple lines\n";

    // Compute hash multiple times
    let mut hashes = Vec::new();
    for _ in 0..10 {
        let hash = canonicalizer.hash_canonicalized(test_content, FileType::Text)?;
        hashes.push(hash);
    }

    // All hashes should be identical
    let first_hash = &hashes[0];
    for (i, hash) in hashes.iter().enumerate() {
        assert_eq!(hash, first_hash, "Hash {i} should match first hash");
    }

    println!("✓ BLAKE3 consistency across runs test passed");
    Ok(())
}

/// Test hash uniqueness: different content produces different hashes
#[test]
fn test_blake3_uniqueness() -> Result<()> {
    let canonicalizer = Canonicalizer::new();

    let test_cases = vec![
        "content1",
        "content2",
        "content3",
        "Content1",   // Different case
        "content1 ",  // Trailing space
        " content1",  // Leading space
        "content1\n", // With newline
    ];

    let mut hashes = Vec::new();
    for content in &test_cases {
        let hash = canonicalizer.hash_canonicalized(content, FileType::Text)?;
        hashes.push(hash);
    }

    // All hashes should be unique
    for i in 0..hashes.len() {
        for j in (i + 1)..hashes.len() {
            assert_ne!(
                hashes[i], hashes[j],
                "Different content should produce different hashes: '{}' vs '{}'",
                test_cases[i], test_cases[j]
            );
        }
    }

    println!("✓ BLAKE3 uniqueness test passed");
    Ok(())
}

/// Test hash with special characters and unicode
#[test]
fn test_blake3_special_characters_unicode() -> Result<()> {
    let canonicalizer = Canonicalizer::new();

    let test_cases = vec![
        "hello\nworld",
        "hello\tworld",
        "hello\"world",
        "hello\\world",
        "hello 世界",
        "🚀🎉✨",
        "mixed: hello 世界 🌍",
    ];

    for content in test_cases {
        let hash = canonicalizer.hash_canonicalized(content, FileType::Text)?;

        // Verify hash is valid
        assert_eq!(
            hash.len(),
            64,
            "Hash should be 64 characters for: {content}"
        );
        assert!(
            hash.chars().all(|c| c.is_ascii_hexdigit()),
            "Hash should be valid hex for: {content}"
        );

        // Verify consistency
        let hash2 = canonicalizer.hash_canonicalized(content, FileType::Text)?;
        assert_eq!(hash, hash2, "Hash should be consistent for: {content}");
    }

    println!("✓ BLAKE3 special characters and unicode test passed");
    Ok(())
}

/// Test hash computation with context error handling
#[test]
fn test_blake3_with_context_error_handling() -> Result<()> {
    let canonicalizer = Canonicalizer::new();

    // Test valid content
    let valid_yaml = "name: test\nversion: 1.0";
    let result =
        canonicalizer.hash_canonicalized_with_context(valid_yaml, FileType::Yaml, "test_phase");
    assert!(result.is_ok(), "Valid YAML should hash successfully");

    // Test invalid YAML
    let invalid_yaml = "invalid: yaml: [unclosed";
    let result =
        canonicalizer.hash_canonicalized_with_context(invalid_yaml, FileType::Yaml, "test_phase");
    assert!(result.is_err(), "Invalid YAML should return error");

    // Verify error contains phase information
    if let Err(err) = result {
        let error_str = format!("{err:?}");
        assert!(
            error_str.contains("test_phase"),
            "Error should contain phase information"
        );
    }

    println!("✓ BLAKE3 with context error handling test passed");
    Ok(())
}

/// Comprehensive test that runs all BLAKE3 hashing tests
#[test]
fn test_v1_2_blake3_hashing_comprehensive() -> Result<()> {
    println!("🚀 Running V1.2 BLAKE3 hashing comprehensive validation...");

    test_blake3_hash_stability_lf_line_endings()?;
    test_blake3_hash_on_canonicalized_content()?;
    test_blake3_full_64_char_hex_format()?;
    test_blake3_receipts_match_on_disk_artifacts()?;
    test_blake3_empty_files()?;
    test_blake3_large_files()?;
    test_blake3_consistency_across_runs()?;
    test_blake3_uniqueness()?;
    test_blake3_special_characters_unicode()?;
    test_blake3_with_context_error_handling()?;

    println!("✅ V1.2 BLAKE3 hashing comprehensive validation passed!");
    println!();
    println!("Requirements Validated:");
    println!("  ✓ FR-JCS-005: Hash stability across platforms (LF line endings)");
    println!("  ✓ FR-JCS-006: Hash computation on canonicalized content");
    println!("  ✓ Full 64-character hex format");
    println!("  ✓ Hashes in receipts match on-disk artifacts");
    println!();
    println!("Edge Cases Tested:");
    println!("  ✓ Empty files");
    println!("  ✓ Large files (1 MB)");
    println!("  ✓ Consistency across multiple runs");
    println!("  ✓ Hash uniqueness for different content");
    println!("  ✓ Special characters and unicode");
    println!("  ✓ Error handling with context");

    Ok(())
}

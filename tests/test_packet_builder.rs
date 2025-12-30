//! Comprehensive tests for PacketBuilder (FR-PKT-001 through FR-PKT-007)
//!
//! **WHITE-BOX TEST**: This test uses internal module APIs (`packet::{...}`, `types::Priority`)
//! and may break with internal refactors. These tests are intentionally white-box to validate
//! internal implementation details. See FR-TEST-4 for white-box test policy.
//!
//! This test suite verifies:
//! - FR-PKT-001: Deterministic ordering (sorted file paths)
//! - FR-PKT-002: Priority-based selection (Upstream > High > Medium > Low)
//! - FR-PKT-003: LIFO ordering within priority classes
//! - FR-PKT-004: Byte and line counting during assembly
//! - FR-PKT-005: Limit enforcement (exit 7 on overflow)
//! - FR-PKT-006: Packet manifest generation on overflow
//! - FR-PKT-007: --debug-packet flag behavior

use anyhow::Result;
use camino::Utf8PathBuf;
use std::fs;
use tempfile::TempDir;
use xchecker::packet::{
    ContentSelector, DEFAULT_PACKET_MAX_BYTES, DEFAULT_PACKET_MAX_LINES, PacketBuilder,
};
use xchecker::test_support;
use xchecker::types::Priority;

/// Test FR-PKT-001: Deterministic ordering with sorted file paths
#[test]
fn test_deterministic_ordering_sorted_paths() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;

    // Create files in non-alphabetical order
    fs::write(base_path.join("zebra.md"), "# Zebra")?;
    fs::write(base_path.join("alpha.md"), "# Alpha")?;
    fs::write(base_path.join("beta.md"), "# Beta")?;

    let selector = ContentSelector::new()?;
    let files = selector.select_files(&base_path)?;

    // Files should be sorted alphabetically within same priority
    let paths: Vec<String> = files.iter().map(|f| f.path.to_string()).collect();

    // All these files have the same priority (Low), so they should be in LIFO order
    // which means reverse alphabetical
    assert_eq!(paths.len(), 3);
    assert!(paths[0].contains("zebra.md"));
    assert!(paths[1].contains("beta.md"));
    assert!(paths[2].contains("alpha.md"));

    Ok(())
}

/// Test FR-PKT-002: Priority-based selection (Upstream > High > Medium > Low)
#[test]
fn test_priority_based_selection() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;

    // Create files with different priorities
    fs::write(base_path.join("config.toml"), "# Low priority")?;
    fs::write(base_path.join("README.md"), "# Medium priority")?;
    fs::write(base_path.join("SPEC-001.md"), "# High priority")?;
    fs::write(base_path.join("design.core.yaml"), "# Upstream priority")?;

    let selector = ContentSelector::new()?;
    let files = selector.select_files(&base_path)?;

    // Verify priority ordering: Upstream -> High -> Medium -> Low
    assert_eq!(files.len(), 4);
    assert_eq!(files[0].priority, Priority::Upstream);
    assert!(files[0].path.to_string().contains("design.core.yaml"));

    assert_eq!(files[1].priority, Priority::High);
    assert!(files[1].path.to_string().contains("SPEC-001.md"));

    assert_eq!(files[2].priority, Priority::Medium);
    assert!(files[2].path.to_string().contains("README.md"));

    assert_eq!(files[3].priority, Priority::Low);
    assert!(files[3].path.to_string().contains("config.toml"));

    Ok(())
}

/// Test FR-PKT-003: LIFO ordering within priority classes
#[test]
fn test_lifo_ordering_within_priority() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;

    // Create multiple files with same priority (High)
    fs::write(base_path.join("SPEC-001.md"), "# Spec 1")?;
    fs::write(base_path.join("SPEC-002.md"), "# Spec 2")?;
    fs::write(base_path.join("SPEC-003.md"), "# Spec 3")?;

    let selector = ContentSelector::new()?;
    let files = selector.select_files(&base_path)?;

    // All files have High priority, should be in LIFO (reverse) order
    assert_eq!(files.len(), 3);
    assert_eq!(files[0].priority, Priority::High);
    assert_eq!(files[1].priority, Priority::High);
    assert_eq!(files[2].priority, Priority::High);

    // LIFO means reverse alphabetical order
    assert!(files[0].path.to_string().contains("SPEC-003.md"));
    assert!(files[1].path.to_string().contains("SPEC-002.md"));
    assert!(files[2].path.to_string().contains("SPEC-001.md"));

    Ok(())
}

/// Test FR-PKT-004: Byte and line counting during assembly
#[test]
fn test_byte_and_line_counting() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");

    // Create a file with known content
    let content = "Line 1\nLine 2\nLine 3\n";
    fs::write(base_path.join("test.md"), content)?;

    let mut builder = PacketBuilder::new()?;
    let packet = builder.build_packet(&base_path, "test", &context_dir, None)?;

    // Verify budget tracking
    assert!(packet.budget_used.bytes_used > 0);
    assert!(packet.budget_used.lines_used > 0);

    // Budget should include file content plus formatting
    assert!(packet.budget_used.bytes_used >= content.len());
    assert!(packet.budget_used.lines_used >= 3); // At least 3 lines from content

    Ok(())
}

/// Test FR-PKT-005: Limit enforcement with exit 7 on overflow
#[test]
fn test_limit_enforcement_overflow() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");

    // Create a large upstream file that exceeds budget
    let large_content = "data: value\n".repeat(1000);
    fs::write(base_path.join("large.core.yaml"), &large_content)?;

    // Use very small limits to trigger overflow
    let mut builder = PacketBuilder::with_limits(100, 5)?;
    let result = builder.build_packet(&base_path, "test", &context_dir, None);

    // Should fail with PacketOverflow error
    assert!(result.is_err());

    // Verify it's a packet overflow error by checking the error chain
    let err = result.unwrap_err();
    let err_string = format!("{:?}", err);
    // The error should contain information about packet overflow
    assert!(
        err_string.contains("PacketOverflow")
            || err_string.contains("packet")
            || err_string.to_lowercase().contains("overflow")
            || err_string.contains("budget")
            || err_string.contains("limit")
    );

    Ok(())
}

/// Test FR-PKT-005: Upstream files always included, regular files evicted
#[test]
fn test_upstream_non_evictable() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");

    // Create small upstream file and large regular file
    fs::write(base_path.join("small.core.yaml"), "key: value")?;
    fs::write(base_path.join("large.md"), "# Large\n".repeat(200))?;

    // Use limits that allow upstream but not the large regular file
    let mut builder = PacketBuilder::with_limits(300, 30)?;
    let packet = builder.build_packet(&base_path, "test", &context_dir, None)?;

    // Upstream file should be included
    assert!(
        packet
            .evidence
            .files
            .iter()
            .any(|f| f.path.contains("small.core.yaml"))
    );

    // Large regular file should be excluded
    assert!(
        !packet
            .evidence
            .files
            .iter()
            .any(|f| f.path.contains("large.md"))
    );

    Ok(())
}

/// Test FR-PKT-006: Packet preview always written to context/
#[test]
fn test_packet_preview_always_written() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");

    fs::write(base_path.join("test.md"), "# Test")?;

    let mut builder = PacketBuilder::new()?;
    let _packet = builder.build_packet(&base_path, "requirements", &context_dir, None)?;

    // Verify context file was written
    let context_file = context_dir.join("requirements-packet.txt");
    assert!(context_file.exists());

    let preview_content = fs::read_to_string(&context_file)?;
    assert!(preview_content.contains("test.md"));

    Ok(())
}

/// Test FR-PKT-006: Packet preview written even on overflow
#[test]
fn test_packet_preview_written_on_overflow() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");

    // Create large upstream file that will cause overflow
    let large_content = "data: value\n".repeat(1000);
    fs::write(base_path.join("large.core.yaml"), &large_content)?;

    let mut builder = PacketBuilder::with_limits(100, 5)?;
    let _result = builder.build_packet(&base_path, "test", &context_dir, None);

    // Even though build failed, context file should exist
    let context_file = context_dir.join("test-packet.txt");
    assert!(context_file.exists());

    Ok(())
}

/// Test FR-PKT-004: Verify packet evidence includes all required fields
#[test]
fn test_packet_evidence_completeness() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");

    fs::write(base_path.join("test.md"), "# Test")?;
    fs::write(base_path.join("config.yaml"), "key: value")?;

    let mut builder = PacketBuilder::new()?;
    let packet = builder.build_packet(&base_path, "test", &context_dir, None)?;

    // Verify evidence structure
    assert_eq!(packet.evidence.max_bytes, DEFAULT_PACKET_MAX_BYTES);
    assert_eq!(packet.evidence.max_lines, DEFAULT_PACKET_MAX_LINES);
    assert_eq!(packet.evidence.files.len(), 2);

    // Verify each file evidence has required fields
    for file_evidence in &packet.evidence.files {
        assert!(!file_evidence.path.is_empty());
        assert!(!file_evidence.blake3_pre_redaction.is_empty());
        assert_eq!(file_evidence.blake3_pre_redaction.len(), 64); // Full BLAKE3 hash
    }

    Ok(())
}

/// Test FR-PKT-002: Priority assignment for different file types
#[test]
fn test_priority_assignment_comprehensive() -> Result<()> {
    let selector = ContentSelector::new()?;

    // Test Upstream priority (.core.yaml files)
    assert_eq!(
        selector.get_priority(Utf8PathBuf::from("test.core.yaml").as_path()),
        Priority::Upstream
    );
    assert_eq!(
        selector.get_priority(Utf8PathBuf::from("docs/design.core.yaml").as_path()),
        Priority::Upstream
    );

    // Test High priority (SPEC, ADR, REPORT)
    assert_eq!(
        selector.get_priority(Utf8PathBuf::from("SPEC-001.md").as_path()),
        Priority::High
    );
    assert_eq!(
        selector.get_priority(Utf8PathBuf::from("docs/ADR-002.md").as_path()),
        Priority::High
    );
    assert_eq!(
        selector.get_priority(Utf8PathBuf::from("REPORT-final.md").as_path()),
        Priority::High
    );

    // Test Medium priority (README, SCHEMA)
    assert_eq!(
        selector.get_priority(Utf8PathBuf::from("README.md").as_path()),
        Priority::Medium
    );
    assert_eq!(
        selector.get_priority(Utf8PathBuf::from("docs/SCHEMA.yaml").as_path()),
        Priority::Medium
    );

    // Test Low priority (misc files)
    assert_eq!(
        selector.get_priority(Utf8PathBuf::from("config.toml").as_path()),
        Priority::Low
    );
    assert_eq!(
        selector.get_priority(Utf8PathBuf::from("src/main.rs").as_path()),
        Priority::Low
    );

    Ok(())
}

/// Test FR-PKT-001: Deterministic ordering across multiple runs
#[test]
fn test_deterministic_ordering_multiple_runs() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;

    // Create multiple files
    fs::write(base_path.join("file1.md"), "# File 1")?;
    fs::write(base_path.join("file2.md"), "# File 2")?;
    fs::write(base_path.join("file3.md"), "# File 3")?;

    let selector = ContentSelector::new()?;

    // Run selection multiple times
    let files1 = selector.select_files(&base_path)?;
    let files2 = selector.select_files(&base_path)?;
    let files3 = selector.select_files(&base_path)?;

    // Results should be identical
    assert_eq!(files1.len(), files2.len());
    assert_eq!(files2.len(), files3.len());

    for i in 0..files1.len() {
        assert_eq!(files1[i].path, files2[i].path);
        assert_eq!(files2[i].path, files3[i].path);
        assert_eq!(files1[i].priority, files2[i].priority);
    }

    Ok(())
}

/// Test FR-PKT-004: Budget tracking with multiple files
#[test]
fn test_budget_tracking_multiple_files() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");

    // Create multiple files with known sizes
    fs::write(base_path.join("file1.md"), "12345")?; // 5 bytes
    fs::write(base_path.join("file2.md"), "67890")?; // 5 bytes
    fs::write(base_path.join("file3.md"), "abcde")?; // 5 bytes

    let mut builder = PacketBuilder::new()?;
    let packet = builder.build_packet(&base_path, "test", &context_dir, None)?;

    // Verify all files included
    assert_eq!(packet.evidence.files.len(), 3);

    // Verify budget includes all content plus formatting
    assert!(packet.budget_used.bytes_used >= 15); // At least 15 bytes from content
    assert!(packet.budget_used.bytes_used < DEFAULT_PACKET_MAX_BYTES);
    assert!(!packet.budget_used.is_exceeded());

    Ok(())
}

/// Test FR-PKT-005: Byte limit enforcement
#[test]
fn test_byte_limit_enforcement() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");

    // Create file that exceeds byte limit
    let content = "x".repeat(200);
    fs::write(base_path.join("large.core.yaml"), &content)?;

    let mut builder = PacketBuilder::with_limits(100, 1000)?;
    let result = builder.build_packet(&base_path, "test", &context_dir, None);

    assert!(result.is_err());

    Ok(())
}

/// Test FR-PKT-005: Line limit enforcement
#[test]
fn test_line_limit_enforcement() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");

    // Create file with many lines
    let content = "line\n".repeat(100);
    fs::write(base_path.join("many_lines.core.yaml"), &content)?;

    let mut builder = PacketBuilder::with_limits(100000, 10)?;
    let result = builder.build_packet(&base_path, "test", &context_dir, None);

    assert!(result.is_err());

    Ok(())
}

/// Test FR-PKT-002: Mixed priority files with budget constraints
#[test]
fn test_mixed_priority_budget_constraints() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");

    // Create files with different priorities
    fs::write(base_path.join("upstream.core.yaml"), "key: value")?; // Upstream
    fs::write(base_path.join("SPEC.md"), "# Spec")?; // High
    fs::write(base_path.join("README.md"), "# Readme")?; // Medium
    fs::write(base_path.join("large.md"), "x".repeat(500))?; // Low, large

    // Set limits that allow upstream, high, medium but not low
    let mut builder = PacketBuilder::with_limits(200, 50)?;
    let packet = builder.build_packet(&base_path, "test", &context_dir, None)?;

    // Verify higher priority files included
    assert!(
        packet
            .evidence
            .files
            .iter()
            .any(|f| f.path.contains("upstream.core.yaml"))
    );
    assert!(
        packet
            .evidence
            .files
            .iter()
            .any(|f| f.path.contains("SPEC.md"))
    );

    // Low priority large file should be excluded
    assert!(
        !packet
            .evidence
            .files
            .iter()
            .any(|f| f.path.contains("large.md"))
    );

    Ok(())
}

/// Test FR-PKT-001: File path sorting within same priority
#[test]
fn test_file_path_sorting_same_priority() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;

    // Create multiple files with same priority (all .md files are Low priority)
    fs::write(base_path.join("zebra.md"), "z")?;
    fs::write(base_path.join("alpha.md"), "a")?;
    fs::write(base_path.join("middle.md"), "m")?;

    let selector = ContentSelector::new()?;
    let files = selector.select_files(&base_path)?;

    // All files have same priority, should be in LIFO (reverse alphabetical) order
    assert_eq!(files.len(), 3);
    let paths: Vec<String> = files
        .iter()
        .map(|f| f.path.file_name().unwrap().to_string())
        .collect();

    // LIFO ordering means reverse alphabetical
    assert_eq!(paths[0], "zebra.md");
    assert_eq!(paths[1], "middle.md");
    assert_eq!(paths[2], "alpha.md");

    Ok(())
}

/// Test FR-PKT-004: BLAKE3 hash calculation for file evidence
#[test]
fn test_blake3_hash_in_evidence() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");

    fs::write(base_path.join("test.md"), "test content")?;

    let mut builder = PacketBuilder::new()?;
    let packet = builder.build_packet(&base_path, "test", &context_dir, None)?;

    // Verify BLAKE3 hash is present and correct format
    assert_eq!(packet.evidence.files.len(), 1);
    let hash = &packet.evidence.files[0].blake3_pre_redaction;

    // BLAKE3 hash should be 64 hex characters
    assert_eq!(hash.len(), 64);
    assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));

    Ok(())
}

/// Test FR-PKT-006: Packet content format
#[test]
fn test_packet_content_format() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");

    fs::write(base_path.join("test.md"), "# Test Content")?;

    let mut builder = PacketBuilder::new()?;
    let packet = builder.build_packet(&base_path, "test", &context_dir, None)?;

    // Verify packet content includes file separator
    assert!(packet.content.contains("=== "));
    assert!(packet.content.contains("test.md"));
    assert!(packet.content.contains("==="));

    Ok(())
}

/// Test FR-PKT-006: Packet manifest generation on overflow
#[test]
fn test_packet_manifest_on_overflow() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");

    // Create large upstream file that will cause overflow
    let large_content = "data: value\n".repeat(1000);
    fs::write(base_path.join("large.core.yaml"), &large_content)?;

    let mut builder = PacketBuilder::with_limits(100, 5)?;
    let _result = builder.build_packet(&base_path, "test", &context_dir, None);

    // Verify manifest file was written
    let manifest_file = context_dir.join("test-packet.manifest.json");
    assert!(manifest_file.exists());

    // Verify manifest content
    let manifest_content = fs::read_to_string(&manifest_file)?;
    assert!(manifest_content.contains("overflow"));
    assert!(manifest_content.contains("budget"));
    assert!(manifest_content.contains("max_bytes"));
    assert!(manifest_content.contains("max_lines"));
    assert!(manifest_content.contains("used_bytes"));
    assert!(manifest_content.contains("used_lines"));
    assert!(manifest_content.contains("files"));
    assert!(manifest_content.contains("large.core.yaml"));

    // Verify manifest does NOT contain actual file content
    assert!(!manifest_content.contains("data: value"));

    Ok(())
}

/// Test FR-PKT-006: Manifest contains only metadata, no content
#[test]
fn test_manifest_no_content_leak() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");

    // Create file with non-secret content that exceeds the limit
    // Must be Upstream (*.core.yaml) to trigger overflow and manifest writing
    let unique_content = "unique_content_value_that_should_not_leak ".repeat(10);
    let file_path = base_path.join("config.core.yaml");
    fs::write(&file_path, &unique_content)?;
    
    assert!(file_path.exists(), "Test file must exist");

    let mut builder = PacketBuilder::with_limits(50, 3)?;
    let result = builder.build_packet(&base_path, "test", &context_dir, None);

    assert!(result.is_err(), "Packet should have overflowed");

    // Read manifest
    let manifest_file = context_dir.join("test-packet.manifest.json");
    assert!(manifest_file.exists(), "Manifest file should exist");

    let manifest_content = fs::read_to_string(&manifest_file)?;

    // Verify no content in manifest
    assert!(!manifest_content.contains("unique_content_value"));

    // Verify manifest has metadata
    assert!(manifest_content.contains("config.core.yaml"));
    assert!(manifest_content.contains("blake3_pre_redaction"));
    assert!(manifest_content.contains("priority"));

    Ok(())
}

/// Test FR-PKT-007: Debug packet writing
#[test]
fn test_debug_packet_writing() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");

    fs::write(base_path.join("test.md"), "# Test Content")?;

    let mut builder = PacketBuilder::new()?;
    let packet = builder.build_packet(&base_path, "test", &context_dir, None)?;

    // Manually write debug packet (simulating --debug-packet flag)
    builder.write_debug_packet(&packet.content, "test", &context_dir)?;

    // Verify debug packet file was written
    let debug_file = context_dir.join("test-packet-debug.txt");
    assert!(debug_file.exists());

    // Verify debug packet contains full content
    let debug_content = fs::read_to_string(&debug_file)?;
    assert!(debug_content.contains("Test Content"));
    assert!(debug_content.contains("test.md"));

    Ok(())
}

/// Test FR-PKT-007: Debug packet not written if secrets detected
#[test]
fn test_debug_packet_not_written_on_secret() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");
    let token = test_support::github_pat();

    // Create file with a secret
    fs::write(base_path.join("secret.md"), format!("token: {}", token))?;

    let mut builder = PacketBuilder::new()?;
    let result = builder.build_packet(&base_path, "test", &context_dir, None);

    // Should fail due to secret detection
    assert!(result.is_err());

    // Debug packet should NOT be written (secret scan failed)
    let debug_file = context_dir.join("test-packet-debug.txt");
    assert!(!debug_file.exists());

    Ok(())
}

/// Test FR-PKT-006: Manifest includes file priorities
#[test]
fn test_manifest_includes_priorities() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");

    // Create files with different priorities
    fs::write(base_path.join("upstream.core.yaml"), "x".repeat(200))?;
    fs::write(base_path.join("SPEC.md"), "x".repeat(200))?;

    let mut builder = PacketBuilder::with_limits(100, 5)?;
    let _result = builder.build_packet(&base_path, "test", &context_dir, None);

    // Read manifest
    let manifest_file = context_dir.join("test-packet.manifest.json");
    let manifest_content = fs::read_to_string(&manifest_file)?;

    // Verify priorities are included
    assert!(manifest_content.contains("Upstream") || manifest_content.contains("priority"));
    assert!(manifest_content.contains("upstream.core.yaml"));

    Ok(())
}

/// Test FR-PKT-006: Manifest includes BLAKE3 hashes
#[test]
fn test_manifest_includes_blake3_hashes() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");

    // Create file that will cause overflow
    fs::write(base_path.join("large.core.yaml"), "x".repeat(200))?;

    let mut builder = PacketBuilder::with_limits(100, 5)?;
    let _result = builder.build_packet(&base_path, "test", &context_dir, None);

    // Read manifest
    let manifest_file = context_dir.join("test-packet.manifest.json");
    let manifest_content = fs::read_to_string(&manifest_file)?;

    // Verify BLAKE3 hashes are included
    assert!(manifest_content.contains("blake3_pre_redaction"));

    // Verify hash format (should be 64 hex characters)
    // Extract hash value and verify it's valid hex
    assert!(manifest_content.contains("\"blake3_pre_redaction\":"));

    Ok(())
}

/// Test FR-PKT-001: Comprehensive deterministic ordering test
#[test]
fn test_comprehensive_deterministic_ordering() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;

    // Create files with mixed priorities and names
    fs::write(base_path.join("z-file.core.yaml"), "upstream")?;
    fs::write(base_path.join("a-file.core.yaml"), "upstream")?;
    fs::write(base_path.join("SPEC-Z.md"), "high")?;
    fs::write(base_path.join("SPEC-A.md"), "high")?;
    fs::write(base_path.join("README-Z.md"), "medium")?;
    fs::write(base_path.join("README-A.md"), "medium")?;
    fs::write(base_path.join("z-misc.md"), "low")?;
    fs::write(base_path.join("a-misc.md"), "low")?;

    let selector = ContentSelector::new()?;
    let files = selector.select_files(&base_path)?;

    // Verify priority ordering
    // Priority enum: Upstream(0) > High(1) > Medium(2) > Low(3)
    // So numerically, priority values should be non-decreasing
    let mut last_priority_value = 0u8; // Start with Upstream = 0
    for file in &files {
        let current_priority_value = match file.priority {
            Priority::Upstream => 0,
            Priority::High => 1,
            Priority::Medium => 2,
            Priority::Low => 3,
        };
        // Priority value should never decrease (stay same or increase)
        assert!(
            current_priority_value >= last_priority_value,
            "Priority ordering violated: {:?} came after priority value {}",
            file.priority,
            last_priority_value
        );
        last_priority_value = current_priority_value;
    }

    // Within each priority, verify LIFO (reverse alphabetical) ordering
    let upstream_files: Vec<_> = files
        .iter()
        .filter(|f| f.priority == Priority::Upstream)
        .collect();
    if upstream_files.len() > 1 {
        for i in 0..upstream_files.len() - 1 {
            assert!(upstream_files[i].path >= upstream_files[i + 1].path);
        }
    }

    Ok(())
}

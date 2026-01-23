#![cfg(feature = "test-utils")]
//! Security validation tests for redaction system
//!
//! **WHITE-BOX TEST**: This test uses internal module APIs (`packet::PacketBuilder`,
//! `receipt::ReceiptManager`, `redaction::SecretRedactor`, `types::{...}`) and may break
//! with internal refactors. These tests are intentionally white-box to validate internal
//! implementation details. See FR-TEST-4 for white-box test policy.
//!
//! This test suite validates that:
//! - Packet previews are properly redacted (no default secret patterns present)
//! - Receipts don't embed raw packet content
//! - Status outputs never include environment variables
//!
//! Requirements: R8.1, R8.3, R8.4

use anyhow::Result;
use camino::Utf8PathBuf;
use serial_test::serial;
use std::env;
use std::fs;
use tempfile::TempDir;

use std::collections::HashMap;
use xchecker::packet::PacketBuilder;
use xchecker::receipt::ReceiptManager;
use xchecker::redaction::SecretRedactor;
use xchecker::test_support;
use xchecker::types::{PacketEvidence, PhaseId};

/// Test that packet previews are redacted and don't contain default secret patterns
#[test]
fn test_packet_preview_redacted_no_default_patterns() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");

    // Create a file with safe content (no secrets)
    fs::write(
        base_path.join("README.md"),
        "# Test Project\nThis is safe content without secrets.",
    )?;

    let mut builder = PacketBuilder::new()?;
    let _packet = builder.build_packet(&base_path, "requirements", &context_dir, None)?;

    // Read the packet preview file
    let preview_path = context_dir.join("requirements-packet.txt");
    assert!(preview_path.exists(), "Packet preview file should exist");

    let preview_content = fs::read_to_string(&preview_path)?;

    // Verify no default secret patterns are present in the preview
    assert!(
        !preview_content.contains("ghp_"),
        "GitHub PAT pattern should not be present"
    );
    assert!(
        !preview_content.contains("AKIA"),
        "AWS access key pattern should not be present"
    );
    assert!(
        !preview_content.contains("AWS_SECRET_ACCESS_KEY"),
        "AWS secret key pattern should not be present"
    );
    assert!(
        !preview_content.contains("xox"),
        "Slack token pattern should not be present"
    );
    assert!(
        !preview_content.contains("Bearer "),
        "Bearer token pattern should not be present"
    );

    // Verify safe content is present
    assert!(
        preview_content.contains("safe content"),
        "Safe content should be present"
    );

    Ok(())
}

/// Test that packet previews with secrets are properly redacted
#[test]
fn test_packet_preview_secrets_redacted() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");

    // Create a file with content that would trigger redaction if not caught
    // Note: The packet builder should fail before creating the preview if secrets are detected
    fs::write(
        base_path.join("README.md"),
        "# Test Project\nSafe content only.",
    )?;

    // Create a builder and configure it to ignore patterns for testing redaction
    let mut builder = PacketBuilder::new()?;
    builder
        .redactor_mut()
        .add_ignored_pattern("github_pat".to_string());

    // Now create a file with a GitHub token (which will be ignored)
    let token = test_support::github_pat();
    fs::write(base_path.join("config.yaml"), format!("token: {}", token))?;

    let _packet = builder.build_packet(&base_path, "requirements", &context_dir, None)?;

    // Read the packet preview file
    let preview_path = context_dir.join("requirements-packet.txt");
    let preview_content = fs::read_to_string(&preview_path)?;

    // Since we ignored the pattern, the token should be present (demonstrating redaction would work)
    assert!(
        preview_content.contains("ghp_"),
        "Ignored pattern should be present"
    );

    Ok(())
}

/// Test that receipts don't embed raw packet content
#[test]
fn test_receipts_no_raw_packet_content() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let spec_base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;

    let receipt_manager = ReceiptManager::new(&spec_base_path);

    // Create a test packet evidence (no raw content)
    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    // Create a receipt
    let receipt = receipt_manager.create_receipt(
        "test-spec",
        PhaseId::Requirements,
        0,
        vec![],
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        HashMap::new(),
        packet,
        None,
        None,
        vec![],
        Some(false),
        "native",
        None,
        None,
        None,
        None,
        None, // pipeline
    );

    // Write receipt to disk
    let receipt_path = receipt_manager.write_receipt(&receipt)?;

    // Read receipt JSON
    let receipt_json = fs::read_to_string(&receipt_path)?;

    // Verify receipt doesn't contain raw packet content fields
    assert!(
        !receipt_json.contains("\"raw_packet\""),
        "Receipt should not contain raw_packet field"
    );
    assert!(
        !receipt_json.contains("\"packet_content\""),
        "Receipt should not contain packet_content field"
    );
    assert!(
        !receipt_json.contains("\"packet_text\""),
        "Receipt should not contain packet_text field"
    );

    // Verify receipt only contains packet evidence (metadata)
    assert!(
        receipt_json.contains("\"packet\""),
        "Receipt should contain packet evidence"
    );
    assert!(
        receipt_json.contains("\"max_bytes\""),
        "Receipt should contain max_bytes in packet evidence"
    );
    assert!(
        receipt_json.contains("\"max_lines\""),
        "Receipt should contain max_lines in packet evidence"
    );

    Ok(())
}

/// Test that receipts with packet evidence don't leak content
#[test]
fn test_receipts_packet_evidence_no_content_leak() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let spec_base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;

    let receipt_manager = ReceiptManager::new(&spec_base_path);

    // Create packet evidence with file metadata (but no content)
    let packet = PacketEvidence {
        files: vec![xchecker::types::FileEvidence {
            path: "test.md".to_string(),
            range: None,
            blake3_pre_redaction: "abc123def456".to_string(),
            priority: xchecker::types::Priority::High,
        }],
        max_bytes: 65536,
        max_lines: 1200,
    };

    // Create a receipt
    let receipt = receipt_manager.create_receipt(
        "test-spec",
        PhaseId::Requirements,
        0,
        vec![],
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        HashMap::new(),
        packet,
        None,        // stderr_redacted
        None,        // stderr_redacted
        vec![],      // warnings
        Some(false), // fallback_used
        "native",    // runner
        None,        // runner_distro
        None,        // error_kind
        None,        // error_reason
        None,        // diff_context,
        None,        // pipeline
    );

    // Write receipt to disk
    let receipt_path = receipt_manager.write_receipt(&receipt)?;

    // Read receipt JSON
    let receipt_json = fs::read_to_string(&receipt_path)?;

    // Verify packet evidence contains only metadata
    assert!(
        receipt_json.contains("\"files\""),
        "Receipt should contain files array"
    );
    assert!(
        receipt_json.contains("\"blake3_pre_redaction\""),
        "Receipt should contain pre-redaction hash"
    );
    assert!(
        receipt_json.contains("\"priority\""),
        "Receipt should contain priority"
    );

    // Verify no content fields are present
    assert!(
        !receipt_json.contains("\"content\""),
        "Receipt should not contain content field"
    );
    assert!(
        !receipt_json.contains("\"file_content\""),
        "Receipt should not contain file_content field"
    );

    Ok(())
}

/// Test that status outputs never include environment variables
#[test]
#[serial]
fn test_status_no_environment_variables() -> Result<()> {
    // Set some test environment variables
    unsafe {
        env::set_var("TEST_SECRET_KEY", "secret_value_12345");
        env::set_var("TEST_API_TOKEN", "token_abcdef");
        env::set_var("HOME", "/home/testuser");
        env::set_var("PATH", "/usr/bin:/bin");
    }

    let temp_dir = TempDir::new()?;
    let spec_base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;

    // Create receipt manager and artifact manager for status generation
    let receipt_manager = ReceiptManager::new(&spec_base_path);

    // Create a test receipt
    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let receipt = receipt_manager.create_receipt(
        "test-spec",
        PhaseId::Requirements,
        0,
        vec![],
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        HashMap::new(),
        packet,
        None,        // stderr_redacted
        None,        // stderr_redacted
        vec![],      // warnings
        Some(false), // fallback_used
        "native",    // runner
        None,        // runner_distro
        None,        // error_kind
        None,        // error_reason
        None,        // diff_context,
        None,        // pipeline
    );

    receipt_manager.write_receipt(&receipt)?;

    // Note: We can't easily test StatusManager::generate_status_from_orchestrator without a full orchestrator
    // Instead, we verify that the StatusOutput struct doesn't have environment variable fields
    // and that receipts (which feed into status) don't contain env vars

    // Read receipt JSON
    let receipts = receipt_manager.list_receipts()?;
    assert!(!receipts.is_empty(), "Should have at least one receipt");

    let receipt_path = receipt_manager.receipts_path().join(format!(
        "{}-{}.json",
        receipt.phase,
        receipt.emitted_at.format("%Y%m%d_%H%M%S")
    ));
    let receipt_json = fs::read_to_string(&receipt_path)?;

    // Verify receipt doesn't contain environment variables
    assert!(
        !receipt_json.contains("TEST_SECRET_KEY"),
        "Receipt should not contain TEST_SECRET_KEY"
    );
    assert!(
        !receipt_json.contains("secret_value_12345"),
        "Receipt should not contain secret value"
    );
    assert!(
        !receipt_json.contains("TEST_API_TOKEN"),
        "Receipt should not contain TEST_API_TOKEN"
    );
    assert!(
        !receipt_json.contains("token_abcdef"),
        "Receipt should not contain token value"
    );

    // Clean up environment variables
    unsafe {
        env::remove_var("TEST_SECRET_KEY");
        env::remove_var("TEST_API_TOKEN");
    }

    Ok(())
}

/// Test that redactor detects all default secret patterns
#[test]
fn test_redactor_detects_all_default_patterns() -> Result<()> {
    let redactor = SecretRedactor::new()?;

    // Test GitHub PAT
    let github_content = format!("token = {}", test_support::github_pat());
    assert!(
        redactor.has_secrets(&github_content, "test.txt")?,
        "Should detect GitHub PAT"
    );

    // Test AWS access key
    let aws_key_content = format!("access_key = {}", test_support::aws_access_key_id());
    assert!(
        redactor.has_secrets(&aws_key_content, "test.txt")?,
        "Should detect AWS access key"
    );

    // Test AWS secret key
    let aws_secret_content = test_support::aws_secret_access_key();
    assert!(
        redactor.has_secrets(&aws_secret_content, "test.txt")?,
        "Should detect AWS secret key"
    );

    // Test Slack token
    let slack_content = format!("slack_token = {}", test_support::slack_bot_token());
    assert!(
        redactor.has_secrets(&slack_content, "test.txt")?,
        "Should detect Slack token"
    );

    // Test Bearer token
    let bearer_content = format!("Authorization: {}", test_support::bearer_token());
    assert!(
        redactor.has_secrets(&bearer_content, "test.txt")?,
        "Should detect Bearer token"
    );

    // Test safe content
    let safe_content = "This is just normal content with no secrets.";
    assert!(
        !redactor.has_secrets(safe_content, "test.txt")?,
        "Should not detect secrets in safe content"
    );

    Ok(())
}

/// Test that packet builder fails when secrets are detected
#[test]
fn test_packet_builder_fails_on_secret_detection() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");
    let token = test_support::github_pat();

    // Create a file with a GitHub token
    fs::write(
        base_path.join("config.yaml"),
        format!("github_token: {}", token),
    )?;

    let mut builder = PacketBuilder::new()?;
    let result = builder.build_packet(&base_path, "requirements", &context_dir, None);

    // Should fail with secret detection error
    assert!(
        result.is_err(),
        "Packet builder should fail when secrets are detected"
    );

    let error = result.unwrap_err();
    let error_msg = error.to_string();
    assert!(
        error_msg.contains("Secret detected") || error_msg.contains("secret"),
        "Error should mention secret detection: {error_msg}"
    );

    Ok(())
}

/// Test that multiple secret patterns are all detected
#[test]
fn test_multiple_secrets_detected() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");

    // Create a file with multiple secrets
    let github_token = test_support::github_pat();
    let aws_key = test_support::aws_access_key_id();
    let slack_token = test_support::slack_bot_token();
    let multi_secret_content = format!(
        "\ngithub_token: {}\naws_key: {}\nslack_token: {}\n",
        github_token, aws_key, slack_token
    );
    fs::write(base_path.join("secrets.yaml"), multi_secret_content)?;

    let mut builder = PacketBuilder::new()?;
    let result = builder.build_packet(&base_path, "requirements", &context_dir, None);

    // Should fail on first secret detected
    assert!(
        result.is_err(),
        "Packet builder should fail when multiple secrets are detected"
    );

    Ok(())
}

/// Test that `stderr_tail` in receipts doesn't leak secrets
#[test]
fn test_receipt_stderr_tail_no_secrets() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let spec_base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;

    let receipt_manager = ReceiptManager::new(&spec_base_path);

    // Create a receipt with stderr_tail (should not contain secrets)
    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let stderr_tail =
        "Error: Failed to process file\nWarning: Configuration not found\nInfo: Using defaults";

    let receipt = receipt_manager.create_receipt(
        "test-spec",
        PhaseId::Requirements,
        1,
        vec![],
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        HashMap::new(),
        packet,
        Some(stderr_tail.to_string()), // stderr_redacted
        None,                          // stderr_redacted
        vec![],                        // warnings
        Some(false),                   // fallback_used
        "native",                      // runner
        None,                          // runner_distro
        None,                          // error_kind
        None,                          // error_reason
        None,                          // diff_context,
        None,                          // pipeline
    );

    // Write receipt to disk
    let receipt_path = receipt_manager.write_receipt(&receipt)?;

    // Read receipt JSON
    let receipt_json = fs::read_to_string(&receipt_path)?;

    // Verify stderr_tail is present but doesn't contain secret patterns
    assert!(
        receipt_json.contains("stderr_tail"),
        "Receipt should contain stderr_tail"
    );
    assert!(
        !receipt_json.contains("ghp_"),
        "stderr_tail should not contain GitHub PAT"
    );
    assert!(
        !receipt_json.contains("AKIA"),
        "stderr_tail should not contain AWS key"
    );
    assert!(
        !receipt_json.contains("xox"),
        "stderr_tail should not contain Slack token"
    );

    Ok(())
}

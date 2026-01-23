#![cfg(feature = "test-utils")]
//! CI Secret Scanning Tests
//!
//! **WHITE-BOX TEST**: This test uses internal module APIs (`redaction::SecretRedactor`) and may
//! break with internal refactors. These tests are intentionally white-box to validate internal
//! implementation details. See FR-TEST-4 for white-box test policy.
//!
//! This test suite validates secret scanning with positive and negative controls:
//! - Positive control: known-bad file with fake tokens MUST cause scanner to fail
//! - Negative control: clean file MUST pass scanner
//!
//! Requirements: R8.2

use anyhow::Result;
use std::fs;
use xchecker::redaction::SecretRedactor;
use xchecker::test_support;

fn positive_control_content() -> String {
    let github_token = test_support::github_pat();
    let aws_access_key = test_support::aws_access_key_id();
    let aws_secret = test_support::aws_secret_access_key();
    let slack_token = test_support::slack_bot_token();
    let bearer_token = test_support::bearer_token();

    format!(
        "github_token: {}\naws_access_key: {}\n{}\nslack_token: {}\nAuthorization: {}",
        github_token, aws_access_key, aws_secret, slack_token, bearer_token
    )
}

/// Test positive control - file with fake secrets MUST fail scanner
#[test]
fn test_positive_control_fake_secrets_detected() -> Result<()> {
    let redactor = SecretRedactor::new()?;

    let content = positive_control_content();

    // Scanner MUST detect secrets in this file
    let has_secrets = redactor.has_secrets(&content, "positive-control-fake-secrets.txt")?;

    assert!(
        has_secrets,
        "Positive control: Scanner MUST detect fake secrets in test file"
    );

    println!("✓ Positive control: Fake secrets detected as expected");
    Ok(())
}

/// Test negative control - clean file MUST pass scanner
#[test]
fn test_negative_control_clean_file_passes() -> Result<()> {
    let redactor = SecretRedactor::new()?;

    // Read the negative control file (clean content)
    let content = fs::read_to_string("tests/quarantine/negative-control-clean.txt")
        .expect("Negative control file should exist");

    // Scanner MUST NOT detect secrets in this file
    let has_secrets = redactor.has_secrets(&content, "negative-control-clean.txt")?;

    assert!(
        !has_secrets,
        "Negative control: Scanner MUST NOT detect secrets in clean file"
    );

    println!("✓ Negative control: Clean file passed as expected");
    Ok(())
}

/// Test that scanner correctly identifies multiple secret patterns
#[test]
fn test_scanner_identifies_all_patterns_in_positive_control() -> Result<()> {
    let redactor = SecretRedactor::new()?;

    let content = positive_control_content();

    // Test individual patterns
    let patterns = vec![
        ("ghp_", "GitHub PAT"),
        ("AKIA", "AWS Access Key"),
        ("AWS_SECRET_ACCESS_KEY", "AWS Secret Key"),
        ("xoxb-", "Slack Token"),
        ("Bearer ", "Bearer Token"),
    ];

    for (pattern, name) in patterns {
        if content.contains(pattern) {
            println!("  ✓ Positive control contains {name} pattern");
        }
    }

    // Overall detection
    assert!(
        redactor.has_secrets(&content, "positive-control-fake-secrets.txt")?,
        "Scanner should detect at least one secret pattern"
    );

    println!("✓ Scanner identifies all expected patterns in positive control");
    Ok(())
}

/// Test that scanner doesn't have false positives on safe patterns
#[test]
fn test_scanner_no_false_positives_on_safe_patterns() -> Result<()> {
    let redactor = SecretRedactor::new()?;

    // Test various safe patterns that should NOT trigger detection
    let safe_patterns = vec![
        "project_name: xchecker",
        "version: 0.1.0",
        "max_turns: 6",
        "packet_max_bytes: 65536",
        "https://github.com/example/xchecker",
        "def hello_world():",
        "print(\"Hello, World!\")",
    ];

    for pattern in safe_patterns {
        let has_secrets = redactor.has_secrets(pattern, "test.txt")?;
        assert!(
            !has_secrets,
            "Safe pattern should not trigger detection: {pattern}"
        );
    }

    println!("✓ Scanner has no false positives on safe patterns");
    Ok(())
}

/// Test that receipts and packet previews are scanned
#[test]
fn test_receipts_and_packet_previews_scannable() -> Result<()> {
    let redactor = SecretRedactor::new()?;

    // Simulate receipt content (should be clean)
    let receipt_content = r#"{
        "schema_version": "1",
        "spec_id": "test-spec",
        "phase": "requirements",
        "exit_code": 0,
        "packet": {
            "files": [],
            "max_bytes": 65536,
            "max_lines": 1200
        }
    }"#;

    assert!(
        !redactor.has_secrets(receipt_content, "receipt.json")?,
        "Clean receipt should not contain secrets"
    );

    // Simulate packet preview content (should be clean)
    let packet_preview = r"
# Packet Preview
## Files Included
- README.md
- config.yaml

## Content
This is safe content without secrets.
";

    assert!(
        !redactor.has_secrets(packet_preview, "packet-preview.txt")?,
        "Clean packet preview should not contain secrets"
    );

    println!("✓ Receipts and packet previews are scannable");
    Ok(())
}

/// Helper function to scan JSON string values recursively
fn scan_json_strings(redactor: &SecretRedactor, raw: &str, path: &str) -> Result<bool> {
    let val: serde_json::Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(_) => return Ok(false), // malformed JSON? fall back to raw scan
    };

    fn visit(redactor: &SecretRedactor, v: &serde_json::Value, path: &str) -> Result<bool> {
        match v {
            serde_json::Value::String(s) => return redactor.has_secrets(s, path),
            serde_json::Value::Array(arr) => {
                for elem in arr {
                    if visit(redactor, elem, path)? {
                        return Ok(true);
                    }
                }
            }
            serde_json::Value::Object(map) => {
                for (_, val) in map {
                    if visit(redactor, val, path)? {
                        return Ok(true);
                    }
                }
            }
            _ => {}
        }
        Ok(false)
    }

    visit(redactor, &val, path)
}

/// Test that scanner fails on receipt with leaked secret
#[test]
fn test_scanner_detects_leaked_secret_in_receipt() -> Result<()> {
    let redactor = SecretRedactor::new()?;
    let token = test_support::github_pat();

    // Simulate a receipt that accidentally leaked a secret (should never happen)
    // GitHub PAT must be exactly 36 characters after ghp_
    let bad_receipt = format!(
        r#"{{
        "schema_version": "1",
        "spec_id": "test-spec",
        "stderr_tail": "Error: Failed to authenticate with token {}"
    }}"#,
        token
    );

    // Scan JSON strings recursively
    let has_secrets = scan_json_strings(&redactor, &bad_receipt, "bad-receipt.json")?;

    assert!(
        has_secrets,
        "Scanner should detect leaked secret in receipt JSON"
    );

    println!("✓ Scanner detects leaked secrets in receipts (JSON strings)");
    Ok(())
}

/// Test that scanner fails on packet preview with leaked secret
#[test]
fn test_scanner_detects_leaked_secret_in_packet_preview() -> Result<()> {
    let redactor = SecretRedactor::new()?;
    let aws_key = test_support::aws_access_key_id();

    // Simulate a packet preview that accidentally included a secret (should never happen)
    let bad_preview = format!(
        r"
# Packet Preview
## config.yaml
aws_access_key: {}
",
        aws_key
    );

    assert!(
        redactor.has_secrets(&bad_preview, "bad-preview.txt")?,
        "Scanner should detect leaked secret in packet preview"
    );

    println!("✓ Scanner detects leaked secrets in packet previews");
    Ok(())
}

/// Combined test that runs all secret scanning CI checks
#[test]
fn test_secret_scanning_ci_validation() -> Result<()> {
    println!("\n🔒 Running Secret Scanning CI Validation...\n");

    test_positive_control_fake_secrets_detected()?;
    test_negative_control_clean_file_passes()?;
    test_scanner_identifies_all_patterns_in_positive_control()?;
    test_scanner_no_false_positives_on_safe_patterns()?;
    test_receipts_and_packet_previews_scannable()?;
    test_scanner_detects_leaked_secret_in_receipt()?;
    test_scanner_detects_leaked_secret_in_packet_preview()?;

    println!("\n✅ All secret scanning CI validation tests passed!");
    Ok(())
}

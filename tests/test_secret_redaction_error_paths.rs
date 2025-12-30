//! Integration tests for secret redaction in error paths (AT-SEC-003)
//!
//! **WHITE-BOX TEST**: This test uses internal module APIs (`receipt::ReceiptManager`,
//! `redaction::{...}`, `types::{...}`) and may break with internal refactors.
//! These tests are intentionally white-box to validate internal implementation details.
//! See FR-TEST-4 for white-box test policy.
//!
//! These tests verify that secrets are redacted in all error-related fields
//! before they are persisted to receipts:
//! - error_reason
//! - stderr_tail
//! - context lines
//! - warning messages
//!
//! This ensures FR-SEC-005 and FR-SEC-006 compliance.

use std::collections::HashMap;
use xchecker::error::XCheckerError;
use xchecker::receipt::ReceiptManager;
use xchecker::redaction::{redact_user_optional, redact_user_string, redact_user_strings};
use xchecker::test_support;
use xchecker::types::{ErrorKind, PacketEvidence, PhaseId};

#[test]
fn test_at_sec_003_secret_in_error_reason() {
    // AT-SEC-003: Secret appears in error_reason → receipt text is redacted

    // Create a test spec directory
    let temp_dir = xchecker::paths::with_isolated_home();
    let spec_base_path = xchecker::paths::xchecker_home()
        .join("specs")
        .join("test-secret-error");

    let manager = ReceiptManager::new(&spec_base_path);

    // Create an error with a secret in the error message
    let token = test_support::github_pat();
    let error_with_secret = format!("Authentication failed with token {}", token);

    // Create a receipt with the error
    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let receipt = manager.create_receipt(
        "test-secret-error",
        PhaseId::Requirements,
        1, // Non-zero exit code for error
        vec![],
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        HashMap::new(),
        packet,
        None,                                // stderr_tail
        None,                                // stderr_redacted
        vec![],                              // warnings
        None,                                // fallback_used
        "native",                            // runner
        None,                                // runner_distro
        Some(ErrorKind::ClaudeFailure),      // error_kind
        Some(error_with_secret.to_string()), // error_reason
        None,                                // diff_context,
        None,                                // pipeline
    );

    // Verify the error_reason is redacted
    assert!(receipt.error_reason.is_some());
    let error_reason = receipt.error_reason.unwrap();

    // Should contain the error message but not the secret
    assert!(error_reason.contains("Authentication failed"));
    assert!(error_reason.contains("***"));
    assert!(!error_reason.contains("ghp_"));
    assert!(!error_reason.contains(&token));

    drop(temp_dir);
}

#[test]
fn test_secret_in_stderr_tail() {
    // Test that secrets in stderr_tail are redacted before persistence

    let temp_dir = xchecker::paths::with_isolated_home();
    let spec_base_path = xchecker::paths::xchecker_home()
        .join("specs")
        .join("test-stderr-secret");

    let manager = ReceiptManager::new(&spec_base_path);

    // Create stderr with a secret
    let token = test_support::github_pat();
    let stderr_with_secret = format!(
        "Error: Failed to connect\nToken: {}\nConnection refused",
        token
    );

    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let receipt = manager.create_receipt(
        "test-stderr-secret",
        PhaseId::Design,
        70, // Claude failure exit code
        vec![],
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        HashMap::new(),
        packet,
        Some(stderr_with_secret.to_string()),  // stderr_redacted
        None,                                  // stderr_redacted
        vec![],                                // warnings
        None,                                  // fallback_used
        "native",                              // runner
        None,                                  // runner_distro
        Some(ErrorKind::ClaudeFailure),        // error_kind
        Some("Claude CLI failed".to_string()), // error_reason
        None,                                  // diff_context,
        None,                                  // pipeline
    );

    // Verify stderr_tail is redacted
    assert!(receipt.stderr_tail.is_some());
    let stderr = receipt.stderr_tail.unwrap();

    assert!(stderr.contains("Error: Failed to connect"));
    assert!(stderr.contains("***"));
    assert!(!stderr.contains("ghp_"));
    assert!(stderr.contains("Connection refused"));

    drop(temp_dir);
}

#[test]
fn test_secret_in_warning_messages() {
    // Test that secrets in warning messages are redacted

    let temp_dir = xchecker::paths::with_isolated_home();
    let spec_base_path = xchecker::paths::xchecker_home()
        .join("specs")
        .join("test-warning-secret");

    let manager = ReceiptManager::new(&spec_base_path);

    // Create warnings with secrets
    let github_token = test_support::github_pat();
    let aws_key = test_support::aws_access_key_id();
    let warnings_with_secrets = vec![
        format!("Warning: Deprecated token {} used", github_token),
        "Warning: Rate limit exceeded".to_string(),
        format!("Warning: AWS key {} is invalid", aws_key),
    ];

    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let receipt = manager.create_receipt(
        "test-warning-secret",
        PhaseId::Tasks,
        0,
        vec![],                // outputs
        "0.1.0",               // xchecker_version
        "0.8.1",               // claude_cli_version
        "haiku",               // model_full_name
        None,                  // model_alias
        HashMap::new(),        // flags
        packet,                // packet
        None,                  // stderr_tail
        None,                  // stderr_redacted
        warnings_with_secrets, // warnings
        None,                  // fallback_used
        "native",              // runner
        None,                  // runner_distro
        None,                  // error_kind
        None,                  // error_reason
        None,                  // diff_context,
        None,                  // pipeline
    );

    // Verify warnings are redacted
    assert_eq!(receipt.warnings.len(), 3);

    // First warning should have GitHub token redacted
    assert!(receipt.warnings[0].contains("Warning: Deprecated token"));
    assert!(receipt.warnings[0].contains("***"));
    assert!(!receipt.warnings[0].contains("ghp_"));

    // Second warning has no secrets, should be unchanged
    assert_eq!(receipt.warnings[1], "Warning: Rate limit exceeded");

    // Third warning should have AWS key redacted
    assert!(receipt.warnings[2].contains("Warning: AWS key"));
    assert!(receipt.warnings[2].contains("***"));
    assert!(!receipt.warnings[2].contains("AKIA"));

    drop(temp_dir);
}

#[test]
fn test_multiple_secrets_in_error_reason() {
    // Test that multiple different secrets in error_reason are all redacted

    let temp_dir = xchecker::paths::with_isolated_home();
    let spec_base_path = xchecker::paths::xchecker_home()
        .join("specs")
        .join("test-multi-secret");

    let manager = ReceiptManager::new(&spec_base_path);

    let github_token = test_support::github_pat();
    let aws_key = test_support::aws_access_key_id();
    let error_with_multiple_secrets = format!(
        "Failed: GitHub token {} and AWS key {} both invalid",
        github_token, aws_key
    );

    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let receipt = manager.create_receipt(
        "test-multi-secret",
        PhaseId::Review,
        1,
        vec![],
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        HashMap::new(),
        packet,
        None,                                          // stderr_tail
        None,                                          // stderr_redacted
        vec![],                                        // warnings
        None,                                          // fallback_used
        "native",                                      // runner
        None,                                          // runner_distro
        Some(ErrorKind::Unknown),                      // error_kind
        Some(error_with_multiple_secrets.to_string()), // error_reason
        None,                                          // diff_context,
        None,                                          // pipeline
    );

    let error_reason = receipt.error_reason.unwrap();

    // Both secrets should be redacted
    assert!(error_reason.contains("Failed:"));
    assert!(error_reason.contains("***"));
    assert!(!error_reason.contains("ghp_"));
    assert!(!error_reason.contains("AKIA"));
    assert!(error_reason.contains("both invalid"));

    drop(temp_dir);
}

#[test]
fn test_secret_in_bearer_token_error() {
    // Test that Bearer tokens in errors are redacted

    let temp_dir = xchecker::paths::with_isolated_home();
    let spec_base_path = xchecker::paths::xchecker_home()
        .join("specs")
        .join("test-bearer-secret");

    let manager = ReceiptManager::new(&spec_base_path);

    let bearer_token = test_support::bearer_token();
    let error_with_bearer = format!("Authorization failed: {} was rejected", bearer_token);

    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let receipt = manager.create_receipt(
        "test-bearer-secret",
        PhaseId::Requirements,
        8, // Secret detected exit code
        vec![],
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        HashMap::new(),
        packet,
        None,                                // stderr_tail
        None,                                // stderr_redacted
        vec![],                              // warnings
        None,                                // fallback_used
        "native",                            // runner
        None,                                // runner_distro
        Some(ErrorKind::SecretDetected),     // error_kind
        Some(error_with_bearer.to_string()), // error_reason
        None,                                // diff_context,
        None,                                // pipeline
    );

    let error_reason = receipt.error_reason.unwrap();

    assert!(error_reason.contains("Authorization failed:"));
    assert!(error_reason.contains("***"));
    assert!(!error_reason.contains("Bearer eyJ"));
    assert!(error_reason.contains("was rejected"));

    drop(temp_dir);
}

#[test]
fn test_secret_in_slack_token_warning() {
    // Test that Slack tokens in warnings are redacted

    let temp_dir = xchecker::paths::with_isolated_home();
    let spec_base_path = xchecker::paths::xchecker_home()
        .join("specs")
        .join("test-slack-secret");

    let manager = ReceiptManager::new(&spec_base_path);

    let slack_token = test_support::slack_bot_token();
    let warnings = vec![format!("Slack token {} found in config", slack_token)];

    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let receipt = manager.create_receipt(
        "test-slack-secret",
        PhaseId::Design,
        0,
        vec![],         // outputs
        "0.1.0",        // xchecker_version
        "0.8.1",        // claude_cli_version
        "haiku",        // model_full_name
        None,           // model_alias
        HashMap::new(), // flags
        packet,         // packet
        None,           // stderr_tail
        None,           // stderr_redacted
        warnings,       // warnings
        None,           // fallback_used
        "native",       // runner
        None,           // runner_distro
        None,           // error_kind
        None,           // error_reason
        None,           // diff_context,
        None,           // pipeline
    );

    assert_eq!(receipt.warnings.len(), 1);
    assert!(receipt.warnings[0].contains("Slack token"));
    assert!(receipt.warnings[0].contains("***"));
    assert!(!receipt.warnings[0].contains("xoxb-"));

    drop(temp_dir);
}

#[test]
fn test_aws_secret_key_in_stderr() {
    // Test that AWS_SECRET_ACCESS_KEY in stderr is redacted

    let temp_dir = xchecker::paths::with_isolated_home();
    let spec_base_path = xchecker::paths::xchecker_home()
        .join("specs")
        .join("test-aws-stderr");

    let manager = ReceiptManager::new(&spec_base_path);

    let aws_secret = test_support::aws_secret_access_key();
    let stderr = format!(
        "Configuration error\n{}\nPlease check your environment",
        aws_secret
    );

    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let receipt = manager.create_receipt(
        "test-aws-stderr",
        PhaseId::Requirements,
        1,
        vec![],
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        HashMap::new(),
        packet,
        Some(stderr.to_string()),                // stderr_redacted
        None,                                    // stderr_redacted
        vec![],                                  // warnings
        None,                                    // fallback_used
        "native",                                // runner
        None,                                    // runner_distro
        Some(ErrorKind::Unknown),                // error_kind
        Some("Configuration error".to_string()), // error_reason
        None,                                    // diff_context,
        None,                                    // pipeline
    );

    let stderr_tail = receipt.stderr_tail.unwrap();

    assert!(stderr_tail.contains("Configuration error"));
    assert!(stderr_tail.contains("***"));
    assert!(!stderr_tail.contains("AWS_SECRET_ACCESS_KEY="));
    assert!(stderr_tail.contains("Please check your environment"));

    drop(temp_dir);
}

#[test]
fn test_no_secrets_no_redaction() {
    // Test that text without secrets is not modified

    let temp_dir = xchecker::paths::with_isolated_home();
    let spec_base_path = xchecker::paths::xchecker_home()
        .join("specs")
        .join("test-no-secret");

    let manager = ReceiptManager::new(&spec_base_path);

    let safe_error = "File not found: /path/to/file.txt";
    let safe_stderr = "Error: Connection timeout\nRetry in 5 seconds";
    let safe_warnings = vec![
        "Warning: Deprecated API usage".to_string(),
        "Warning: Large file detected".to_string(),
    ];

    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let receipt = manager.create_receipt(
        "test-no-secret",
        PhaseId::Tasks,
        1,
        vec![],
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        HashMap::new(),
        packet,
        Some(safe_stderr.to_string()), // stderr_redacted
        None,                          // stderr_redacted
        safe_warnings.clone(),         // warnings
        None,                          // fallback_used
        "native",                      // runner
        None,                          // runner_distro
        Some(ErrorKind::Unknown),      // error_kind
        Some(safe_error.to_string()),  // error_reason
        None,                          // diff_context,
        None,                          // pipeline
    );

    // Text without secrets should be unchanged
    assert_eq!(receipt.error_reason.unwrap(), safe_error);
    assert_eq!(receipt.stderr_tail.unwrap(), safe_stderr);
    assert_eq!(receipt.warnings, safe_warnings);

    drop(temp_dir);
}

#[test]
fn test_receipt_write_persists_redacted_content() {
    // Test that when a receipt is written to disk, redacted content is persisted

    let temp_dir = xchecker::paths::with_isolated_home();
    let spec_base_path = xchecker::paths::xchecker_home()
        .join("specs")
        .join("test-persist-redacted");

    let manager = ReceiptManager::new(&spec_base_path);

    let token = test_support::github_pat();
    let error_with_secret = format!("Failed with token {}", token);

    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let receipt = manager.create_receipt(
        "test-persist-redacted",
        PhaseId::Requirements,
        1,
        vec![],
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        HashMap::new(),
        packet,
        None,                                // stderr_tail
        None,                                // stderr_redacted
        vec![],                              // warnings
        None,                                // fallback_used
        "native",                            // runner
        None,                                // runner_distro
        Some(ErrorKind::Unknown),            // error_kind
        Some(error_with_secret.to_string()), // error_reason
        None,                                // diff_context,
        None,                                // pipeline
    );

    // Write receipt to disk
    let receipt_path = manager.write_receipt(&receipt).unwrap();

    // Read the file content directly
    let file_content = std::fs::read_to_string(receipt_path.as_std_path()).unwrap();

    // Verify the file content does not contain the secret
    assert!(!file_content.contains(&token));
    assert!(file_content.contains("***"));
    assert!(file_content.contains("Failed with token"));

    drop(temp_dir);
}

#[test]
fn test_error_receipt_creation_with_secrets() {
    // Test create_error_receipt method with secrets in error

    let temp_dir = xchecker::paths::with_isolated_home();
    let spec_base_path = xchecker::paths::xchecker_home()
        .join("specs")
        .join("test-error-receipt");

    let manager = ReceiptManager::new(&spec_base_path);

    // Create an error with a secret
    let error = XCheckerError::SecretDetected {
        pattern: "github_pat".to_string(),
        location: "config.yaml:5:10".to_string(),
    };

    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let token = test_support::github_pat();
    let stderr_with_secret = format!("Found token: {}", token);

    let receipt = manager.create_error_receipt(
        "test-error-receipt",
        PhaseId::Requirements,
        &error,
        "0.1.0",                              // xchecker_version
        "0.8.1",                              // claude_cli_version
        "haiku",                              // model_full_name
        None,                                 // model_alias
        HashMap::new(),                       // flags
        packet,                               // packet
        Some(stderr_with_secret.to_string()), // stderr_tail
        None,                                 // stderr_redacted
        vec![],                               // warnings
        None,                                 // fallback_used
        "native",                             // runner
        None,                                 // runner_distro
        None,                                 // diff_context,
        None,                                 // pipeline
    );

    // Verify error_reason is set and redacted (though this error doesn't contain a secret in the message)
    assert!(receipt.error_reason.is_some());
    assert_eq!(receipt.exit_code, 8); // Secret detected exit code
    assert_eq!(receipt.error_kind, Some(ErrorKind::SecretDetected));

    // Verify stderr is redacted
    assert!(receipt.stderr_tail.is_some());
    let stderr = receipt.stderr_tail.unwrap();
    assert!(stderr.contains("***"));
    assert!(!stderr.contains("ghp_"));

    drop(temp_dir);
}

#[test]
fn test_redaction_helper_functions() {
    // Test the global redaction helper functions

    // Test redact_user_string
    let token = test_support::github_pat();
    let text_with_secret = format!("Error: {} invalid", token);
    let redacted = redact_user_string(&text_with_secret);
    assert!(redacted.contains("***"));
    assert!(!redacted.contains("ghp_"));

    // Test redact_user_optional with Some
    let aws_key = test_support::aws_access_key_id();
    let some_text = Some(format!("Token {} expired", aws_key));
    let redacted_some = redact_user_optional(&some_text);
    assert!(redacted_some.is_some());
    assert!(redacted_some.unwrap().contains("***"));

    // Test redact_user_optional with None
    let none_text: Option<String> = None;
    let redacted_none = redact_user_optional(&none_text);
    assert!(redacted_none.is_none());

    // Test redact_user_strings
    let token = test_support::github_pat();
    let strings = vec![format!("Warning: {}", token), "Safe warning".to_string()];
    let redacted_strings = redact_user_strings(&strings);
    assert_eq!(redacted_strings.len(), 2);
    assert!(redacted_strings[0].contains("***"));
    assert!(!redacted_strings[0].contains("ghp_"));
    assert_eq!(redacted_strings[1], "Safe warning");
}

#[test]
fn test_context_lines_redaction() {
    // Test that context lines (error context) are redacted
    // Context lines are typically part of error_reason or warnings

    let temp_dir = xchecker::paths::with_isolated_home();
    let spec_base_path = xchecker::paths::xchecker_home()
        .join("specs")
        .join("test-context-redact");

    let manager = ReceiptManager::new(&spec_base_path);

    // Simulate context lines with secrets
    let token = test_support::github_pat();
    let context_with_secret = format!(
        "Context: Failed at line 42\nToken used: {}\nRetry failed",
        token
    );

    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let receipt = manager.create_receipt(
        "test-context-redact",
        PhaseId::Design,
        1,
        vec![],
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        HashMap::new(),
        packet,
        None,                                  // stderr_tail
        None,                                  // stderr_redacted
        vec![],                                // warnings
        None,                                  // fallback_used
        "native",                              // runner
        None,                                  // runner_distro
        Some(ErrorKind::Unknown),              // error_kind
        Some(context_with_secret.to_string()), // error_reason
        None,                                  // diff_context,
        None,                                  // pipeline
    );

    let error_reason = receipt.error_reason.unwrap();

    // Context should be redacted
    assert!(error_reason.contains("Context: Failed at line 42"));
    assert!(error_reason.contains("***"));
    assert!(!error_reason.contains("ghp_"));
    assert!(error_reason.contains("Retry failed"));

    drop(temp_dir);
}

#[test]
fn test_all_error_fields_redacted_together() {
    // Test that when all error fields contain secrets, they are all redacted

    let temp_dir = xchecker::paths::with_isolated_home();
    let spec_base_path = xchecker::paths::xchecker_home()
        .join("specs")
        .join("test-all-fields");

    let manager = ReceiptManager::new(&spec_base_path);

    let github_token = test_support::github_pat();
    let aws_key = test_support::aws_access_key_id();
    let slack_token = test_support::slack_bot_token();
    let error_with_secret = format!("Error: {}", github_token);
    let stderr_with_secret = format!("Stderr: {}", aws_key);
    let warnings_with_secrets = vec![format!("Warning: {}", slack_token)];

    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let receipt = manager.create_receipt(
        "test-all-fields",
        PhaseId::Tasks,
        1,
        vec![],
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        HashMap::new(),
        packet,
        Some(stderr_with_secret.to_string()), // stderr_redacted
        None,                                 // stderr_redacted
        warnings_with_secrets,                // warnings
        None,                                 // fallback_used
        "native",                             // runner
        None,                                 // runner_distro
        Some(ErrorKind::Unknown),             // error_kind
        Some(error_with_secret.to_string()),  // error_reason
        None,                                 // diff_context,
        None,                                 // pipeline
    );

    // All fields should be redacted
    let error_reason = receipt.error_reason.unwrap();
    assert!(error_reason.contains("***"));
    assert!(!error_reason.contains("ghp_"));

    let stderr = receipt.stderr_tail.unwrap();
    assert!(stderr.contains("***"));
    assert!(!stderr.contains("AKIA"));

    assert_eq!(receipt.warnings.len(), 1);
    assert!(receipt.warnings[0].contains("***"));
    assert!(!receipt.warnings[0].contains("xoxb-"));

    drop(temp_dir);
}

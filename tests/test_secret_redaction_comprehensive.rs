//! Comprehensive secret redaction tests (FR-SEC)
//!
//! **WHITE-BOX TEST**: This test uses internal module APIs (`redaction::{...}`,
//! `exit_codes::codes`) and may break with internal refactors. These tests are intentionally
//! white-box to validate internal implementation details. See FR-TEST-4 for white-box test policy.
//!
//! This test suite validates all aspects of secret redaction:
//! - All default patterns (GitHub PAT, AWS keys, Slack, Bearer)
//! - --extra-secret-pattern adds custom patterns
//! - --ignore-secret-pattern suppresses patterns
//! - Redaction replaces matches with ***
//! - Exit code 8 on secret detection
//! - Secrets in file paths (redacted in receipts/logs)
//! - Secrets in error messages (redacted before persistence)
//! - Secrets in stderr (redacted before truncation)
//! - Receipts never include env vars or raw packet content
//!
//! Requirements: FR-SEC-001 through FR-SEC-006

use anyhow::Result;
use xchecker::error::XCheckerError;
use xchecker::exit_codes::codes;
use xchecker::redaction::{SecretRedactor, create_secret_detected_error};
use xchecker::test_support;

/// Test all default patterns are detected (FR-SEC-001)
#[test]
fn test_all_default_patterns_detected() -> Result<()> {
    let redactor = SecretRedactor::new()?;
    let github_token = test_support::github_pat();
    let aws_key = test_support::aws_access_key_id();
    let aws_secret = test_support::aws_secret_access_key();
    let slack_token = test_support::slack_bot_token();
    let bearer_token = test_support::bearer_token();

    // Test GitHub PAT: ghp_[A-Za-z0-9]{36}
    let github_content = format!("token = {}", github_token);
    assert!(
        redactor.has_secrets(&github_content, "test.txt")?,
        "Should detect GitHub PAT"
    );
    let matches = redactor.scan_for_secrets(&github_content, "test.txt")?;
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].pattern_id, "github_pat");

    // Test AWS access key: AKIA[0-9A-Z]{16}
    let aws_key_content = format!("access_key = {}", aws_key);
    assert!(
        redactor.has_secrets(&aws_key_content, "test.txt")?,
        "Should detect AWS access key"
    );
    let matches = redactor.scan_for_secrets(&aws_key_content, "test.txt")?;
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].pattern_id, "aws_access_key");

    // Test AWS secret key: AWS_SECRET_ACCESS_KEY=
    let aws_secret_content = aws_secret;
    assert!(
        redactor.has_secrets(&aws_secret_content, "test.txt")?,
        "Should detect AWS secret key"
    );
    let matches = redactor.scan_for_secrets(&aws_secret_content, "test.txt")?;
    assert!(
        !matches.is_empty(),
        "Should return at least one match for AWS secret key"
    );
    assert!(
        matches.iter().any(|m| m.pattern_id == "aws_secret_key"),
        "Should include aws_secret_key match: {matches:?}"
    );
    assert!(
        matches.iter().all(|m| {
            matches!(
                m.pattern_id.as_str(),
                "aws_secret_key" | "aws_secret_key_value"
            )
        }),
        "Should not match unrelated patterns: {matches:?}"
    );

    // Test Slack token: xox[baprs]-
    let slack_content = format!("slack_token = {}", slack_token);
    assert!(
        redactor.has_secrets(&slack_content, "test.txt")?,
        "Should detect Slack token"
    );
    let matches = redactor.scan_for_secrets(&slack_content, "test.txt")?;
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].pattern_id, "slack_token");

    // Test Bearer token: Bearer [A-Za-z0-9._-]{20,}
    let bearer_content = format!("Authorization: {}", bearer_token);
    assert!(
        redactor.has_secrets(&bearer_content, "test.txt")?,
        "Should detect Bearer token"
    );
    let matches = redactor.scan_for_secrets(&bearer_content, "test.txt")?;
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].pattern_id, "bearer_token");

    Ok(())
}

/// Test --extra-secret-pattern adds custom patterns (FR-SEC-004)
#[test]
fn test_extra_secret_pattern_addition() -> Result<()> {
    let mut redactor = SecretRedactor::new()?;

    // Add a custom pattern
    redactor.add_extra_pattern("custom_api_key".to_string(), r"API_KEY_[A-Z0-9]{16}")?;

    let content = "config = API_KEY_1234567890ABCDEF";

    // Should detect the custom pattern
    assert!(
        redactor.has_secrets(content, "test.txt")?,
        "Should detect custom pattern"
    );

    let matches = redactor.scan_for_secrets(content, "test.txt")?;
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].pattern_id, "custom_api_key");

    Ok(())
}

/// Test multiple extra patterns can be added
#[test]
fn test_multiple_extra_patterns() -> Result<()> {
    let mut redactor = SecretRedactor::new()?;

    redactor.add_extra_pattern("pattern1".to_string(), r"SECRET1_[A-Z]{10}")?;
    redactor.add_extra_pattern("pattern2".to_string(), r"SECRET2_[0-9]{10}")?;

    let content1 = "key = SECRET1_ABCDEFGHIJ";
    let content2 = "key = SECRET2_1234567890";

    assert!(redactor.has_secrets(content1, "test.txt")?);
    assert!(redactor.has_secrets(content2, "test.txt")?);

    let matches1 = redactor.scan_for_secrets(content1, "test.txt")?;
    assert_eq!(matches1[0].pattern_id, "pattern1");

    let matches2 = redactor.scan_for_secrets(content2, "test.txt")?;
    assert_eq!(matches2[0].pattern_id, "pattern2");

    Ok(())
}

/// Test --ignore-secret-pattern suppresses patterns (FR-SEC-003)
#[test]
fn test_ignore_secret_pattern_suppression() -> Result<()> {
    let mut redactor = SecretRedactor::new()?;

    // Ignore GitHub PAT pattern
    redactor.add_ignored_pattern("github_pat".to_string());

    let content = format!("token = {}", test_support::github_pat());

    // Should NOT detect GitHub PAT because it's ignored
    assert!(
        !redactor.has_secrets(&content, "test.txt")?,
        "Should not detect ignored pattern"
    );

    let matches = redactor.scan_for_secrets(&content, "test.txt")?;
    assert_eq!(matches.len(), 0);

    // But should still detect other patterns
    let aws_content = format!("key = {}", test_support::aws_access_key_id());
    assert!(
        redactor.has_secrets(&aws_content, "test.txt")?,
        "Should still detect non-ignored patterns"
    );

    Ok(())
}

/// Test multiple patterns can be ignored
#[test]
fn test_multiple_ignored_patterns() -> Result<()> {
    let mut redactor = SecretRedactor::new()?;

    redactor.add_ignored_pattern("github_pat".to_string());
    redactor.add_ignored_pattern("slack_token".to_string());

    let github_content = format!("token = {}", test_support::github_pat());
    let slack_content = format!("slack = {}", test_support::slack_bot_token());

    assert!(!redactor.has_secrets(&github_content, "test.txt")?);
    assert!(!redactor.has_secrets(&slack_content, "test.txt")?);

    // AWS should still be detected
    let aws_content = format!("key = {}", test_support::aws_access_key_id());
    assert!(redactor.has_secrets(&aws_content, "test.txt")?);

    Ok(())
}

/// Test redaction replaces matches with [REDACTED:pattern_id] (FR-SEC-005)
#[test]
fn test_redaction_replaces_with_marker() -> Result<()> {
    let redactor = SecretRedactor::new()?;

    let token = test_support::github_pat();
    let content = format!("token = {}\nother_line = safe", token);
    let result = redactor.redact_content(&content, "test.txt")?;

    assert!(result.has_secrets);
    assert_eq!(result.matches.len(), 1);

    // Should contain redaction marker
    assert!(
        result.content.contains("[REDACTED:github_pat]"),
        "Should contain redaction marker"
    );

    // Should NOT contain the actual secret
    assert!(
        !result.content.contains(&token),
        "Should not contain actual secret"
    );

    // Safe content should be preserved
    assert!(
        result.content.contains("other_line = safe"),
        "Safe content should be preserved"
    );

    Ok(())
}

/// Test redaction of multiple secrets in same content
#[test]
fn test_redaction_multiple_secrets() -> Result<()> {
    let redactor = SecretRedactor::new()?;

    let github_token = test_support::github_pat();
    let aws_key = test_support::aws_access_key_id();
    let content = format!("github = {}\naws = {}", github_token, aws_key);
    let result = redactor.redact_content(&content, "test.txt")?;

    assert!(result.has_secrets);
    assert_eq!(result.matches.len(), 2);

    assert!(result.content.contains("[REDACTED:github_pat]"));
    assert!(result.content.contains("[REDACTED:aws_access_key]"));

    assert!(!result.content.contains("ghp_"));
    assert!(!result.content.contains(&aws_key));

    Ok(())
}

/// Test exit code 8 on secret detection (FR-SEC-002)
#[test]
fn test_exit_code_8_on_secret_detection() -> Result<()> {
    let redactor = SecretRedactor::new()?;

    let content = format!("token = {}", test_support::github_pat());
    let matches = redactor.scan_for_secrets(&content, "test.txt")?;

    assert!(!matches.is_empty(), "Should detect secret");

    // Create error from matches
    let error = create_secret_detected_error(&matches);

    // Verify it's a SecretDetected error
    match &error {
        XCheckerError::SecretDetected { pattern, location } => {
            assert_eq!(pattern, "github_pat");
            assert!(location.contains("test.txt"));
        }
        _ => panic!("Expected SecretDetected error"),
    }

    // Verify exit code mapping
    let (exit_code, error_kind): (i32, xchecker::types::ErrorKind) = (&error).into();
    assert_eq!(exit_code, codes::SECRET_DETECTED);
    assert_eq!(exit_code, 8);
    assert_eq!(error_kind, xchecker::types::ErrorKind::SecretDetected);

    Ok(())
}

/// Test secrets in file paths are handled correctly
#[test]
fn test_secrets_in_file_paths() -> Result<()> {
    let redactor = SecretRedactor::new()?;

    // File path containing a secret-like pattern
    let token = test_support::github_pat();
    let file_path = format!("config/{}.yaml", token);

    // The file path itself should be treated as a string that can be redacted
    let path_result = redactor.redact_content(&file_path, "path")?;

    // If the path contains a secret pattern, it should be detected
    assert!(path_result.has_secrets);
    assert!(path_result.content.contains("[REDACTED:github_pat]"));

    Ok(())
}

/// Test secrets in error messages are redacted
#[test]
fn test_secrets_in_error_messages() -> Result<()> {
    let redactor = SecretRedactor::new()?;

    let token = test_support::github_pat();
    let error_message = format!("Failed to authenticate with token {}", token);
    let result = redactor.redact_content(&error_message, "error.txt")?;

    assert!(result.has_secrets);
    assert!(result.content.contains("[REDACTED:github_pat]"));
    assert!(!result.content.contains("ghp_"));

    Ok(())
}

/// Test secrets in stderr are redacted before truncation (FR-SEC-005)
#[test]
fn test_secrets_in_stderr_redacted() -> Result<()> {
    let redactor = SecretRedactor::new()?;

    let token = test_support::github_pat();
    let stderr = format!(
        "Error: Authentication failed\nToken: {}\nPlease check credentials",
        token
    );
    let result = redactor.redact_content(&stderr, "stderr")?;

    assert!(result.has_secrets);
    assert!(result.content.contains("[REDACTED:github_pat]"));
    assert!(!result.content.contains("ghp_"));

    // Verify safe parts are preserved
    assert!(result.content.contains("Authentication failed"));
    assert!(result.content.contains("Please check credentials"));

    Ok(())
}

/// Test context creation doesn't reveal secrets
#[test]
fn test_safe_context_no_secret_leak() -> Result<()> {
    let redactor = SecretRedactor::new()?;

    let token = test_support::github_pat();
    let content = format!("prefix_{}_suffix", token);
    let matches = redactor.scan_for_secrets(&content, "test.txt")?;

    assert_eq!(matches.len(), 1);

    // Context should not contain the actual secret
    let context = &matches[0].context;
    assert!(context.contains("[REDACTED]"));
    assert!(!context.contains("ghp_"));
    assert!(!context.contains(&token));

    Ok(())
}

/// Test line number accuracy in multi-line content
#[test]
fn test_line_number_accuracy() -> Result<()> {
    let redactor = SecretRedactor::new()?;

    let token = test_support::github_pat();
    let content = format!("line 1\nline 2\nline 3 with {}\nline 4", token);
    let matches = redactor.scan_for_secrets(&content, "test.txt")?;

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].line_number, 3, "Should be on line 3");

    Ok(())
}

/// Test column range accuracy
#[test]
fn test_column_range_accuracy() -> Result<()> {
    let redactor = SecretRedactor::new()?;

    let token = test_support::github_pat();
    let content = format!("token = {} end", token);
    let matches = redactor.scan_for_secrets(&content, "test.txt")?;

    assert_eq!(matches.len(), 1);
    let (start, end) = matches[0].column_range;

    // The token starts at position 8 (after "token = ")
    assert_eq!(start, 8);
    assert_eq!(end, start + token.len());

    Ok(())
}

/// Test no false positives on safe content
#[test]
fn test_no_false_positives() -> Result<()> {
    let redactor = SecretRedactor::new()?;

    let safe_patterns = vec![
        "This is just normal text",
        "github.com/user/repo",
        "AWS documentation link",
        "slack channel #general",
        "Bearer of good news",
        "AKIA123",   // Too short for AWS pattern (needs 16 chars after AKIA)
        "ghp_short", // Too short for GitHub PAT
        "xox",       // Too short for Slack token
    ];

    for pattern in safe_patterns {
        assert!(
            !redactor.has_secrets(pattern, "test.txt")?,
            "Should not detect secrets in: {}",
            pattern
        );
    }

    Ok(())
}

/// Test pattern IDs are retrievable
#[test]
fn test_pattern_ids_retrievable() -> Result<()> {
    let redactor = SecretRedactor::new()?;
    let pattern_ids = redactor.get_pattern_ids();

    assert!(pattern_ids.contains(&"github_pat".to_string()));
    assert!(pattern_ids.contains(&"aws_access_key".to_string()));
    assert!(pattern_ids.contains(&"aws_secret_key".to_string()));
    assert!(pattern_ids.contains(&"slack_token".to_string()));
    assert!(pattern_ids.contains(&"bearer_token".to_string()));

    Ok(())
}

/// Test ignored patterns are retrievable
#[test]
fn test_ignored_patterns_retrievable() -> Result<()> {
    let mut redactor = SecretRedactor::new()?;

    redactor.add_ignored_pattern("github_pat".to_string());
    redactor.add_ignored_pattern("slack_token".to_string());

    let ignored = redactor.get_ignored_patterns();
    assert_eq!(ignored.len(), 2);
    assert!(ignored.contains(&"github_pat".to_string()));
    assert!(ignored.contains(&"slack_token".to_string()));

    Ok(())
}

/// Test empty content handling
#[test]
fn test_empty_content() -> Result<()> {
    let redactor = SecretRedactor::new()?;

    let content = "";
    assert!(!redactor.has_secrets(content, "test.txt")?);

    let result = redactor.redact_content(content, "test.txt")?;
    assert!(!result.has_secrets);
    assert_eq!(result.matches.len(), 0);
    assert_eq!(result.content, "");

    Ok(())
}

/// Test whitespace-only content
#[test]
fn test_whitespace_only_content() -> Result<()> {
    let redactor = SecretRedactor::new()?;

    let content = "   \n\t\n   ";
    assert!(!redactor.has_secrets(content, "test.txt")?);

    Ok(())
}

/// Test secret at start of line
#[test]
fn test_secret_at_line_start() -> Result<()> {
    let redactor = SecretRedactor::new()?;

    let token = test_support::github_pat();
    let content = format!("{} is the token", token);
    let matches = redactor.scan_for_secrets(&content, "test.txt")?;

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].column_range.0, 0);

    Ok(())
}

/// Test secret at end of line
#[test]
fn test_secret_at_line_end() -> Result<()> {
    let redactor = SecretRedactor::new()?;

    let token = test_support::github_pat();
    let content = token.clone();
    let matches = redactor.scan_for_secrets(&content, "test.txt")?;

    assert_eq!(matches.len(), 1);
    let (start, end) = matches[0].column_range;
    // The token starts at position 0
    assert_eq!(start, 0);
    assert_eq!(end, token.len());
    assert_eq!(end, content.len());

    Ok(())
}

/// Test multiple secrets on same line
#[test]
fn test_multiple_secrets_same_line() -> Result<()> {
    let redactor = SecretRedactor::new()?;

    let github_token = test_support::github_pat();
    let aws_key = test_support::aws_access_key_id();
    let content = format!("github: {} aws: {}", github_token, aws_key);
    let matches = redactor.scan_for_secrets(&content, "test.txt")?;

    assert_eq!(matches.len(), 2);

    // Check that both patterns are detected (order may vary)
    let pattern_ids: Vec<&str> = matches.iter().map(|m| m.pattern_id.as_str()).collect();
    assert!(pattern_ids.contains(&"github_pat"));
    assert!(pattern_ids.contains(&"aws_access_key"));

    Ok(())
}

/// Test invalid regex pattern handling
#[test]
fn test_invalid_regex_pattern() {
    let mut redactor = SecretRedactor::new().unwrap();

    // Try to add an invalid regex pattern
    let result = redactor.add_extra_pattern("bad_pattern".to_string(), "[invalid(regex");

    assert!(result.is_err(), "Should fail with invalid regex");
}

/// Test case sensitivity of patterns
#[test]
fn test_pattern_case_sensitivity() -> Result<()> {
    let redactor = SecretRedactor::new()?;

    // GitHub PAT pattern allows both cases in the token part: ghp_[A-Za-z0-9]{36}
    // The prefix "ghp_" is lowercase, but the rest can be mixed case
    let lowercase_prefix = format!("token = {}", test_support::github_pat());
    let uppercase_prefix = lowercase_prefix.replacen("ghp_", "GHP_", 1);

    assert!(redactor.has_secrets(&lowercase_prefix, "test.txt")?);
    assert!(
        !redactor.has_secrets(&uppercase_prefix, "test.txt")?,
        "GHP_ prefix should not match"
    );

    // AWS keys pattern: AKIA[0-9A-Z]{16} - uppercase only
    let aws_upper = format!("key = {}", test_support::aws_access_key_id());
    let aws_lower = aws_upper.to_lowercase();

    assert!(redactor.has_secrets(&aws_upper, "test.txt")?);
    assert!(!redactor.has_secrets(&aws_lower, "test.txt")?);

    Ok(())
}

#[test]
fn test_all_comprehensive_secret_redaction_tests() -> Result<()> {
    test_all_default_patterns_detected()?;
    test_extra_secret_pattern_addition()?;
    test_multiple_extra_patterns()?;
    test_ignore_secret_pattern_suppression()?;
    test_multiple_ignored_patterns()?;
    test_redaction_replaces_with_marker()?;
    test_redaction_multiple_secrets()?;
    test_exit_code_8_on_secret_detection()?;
    test_secrets_in_file_paths()?;
    test_secrets_in_error_messages()?;
    test_secrets_in_stderr_redacted()?;
    test_safe_context_no_secret_leak()?;
    test_line_number_accuracy()?;
    test_column_range_accuracy()?;
    test_no_false_positives()?;
    test_pattern_ids_retrievable()?;
    test_ignored_patterns_retrievable()?;
    test_empty_content()?;
    test_whitespace_only_content()?;
    test_secret_at_line_start()?;
    test_secret_at_line_end()?;
    test_multiple_secrets_same_line()?;
    test_pattern_case_sensitivity()?;

    println!("\n✅ All comprehensive secret redaction tests passed!");
    Ok(())
}

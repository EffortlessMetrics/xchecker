#![cfg(feature = "test-utils")]
//! Integration tests for comprehensive redaction coverage
//!
//! **WHITE-BOX TEST**: This test uses internal module APIs (`redaction::{...}`) and may break
//! with internal refactors. These tests are intentionally white-box to validate internal
//! implementation details. See FR-TEST-4 for white-box test policy.
//!
//! These tests verify that secrets are redacted in all user-facing output:
//! - Error messages
//! - Context strings
//! - Preview text
//! - Receipts (`stderr_tail`, warnings, `error_reason`)
//! - Status output
//! - Logs
use xchecker::redaction::{
    SecretRedactor, redact_user_optional, redact_user_string, redact_user_strings,
};
use xchecker::test_support;

#[test]
fn test_redaction_in_error_messages() {
    // Test that error messages with secrets are redacted
    let token = test_support::github_pat();
    let error_msg = format!("Failed to authenticate with token {}", token);
    let redacted = redact_user_string(&error_msg);

    assert!(redacted.contains("Failed to authenticate"));
    assert!(redacted.contains("***"));
    assert!(!redacted.contains("ghp_"));
    assert!(!redacted.contains(&token));
}

#[test]
fn test_redaction_in_context_strings() {
    // Test that context strings with secrets are redacted
    let token = test_support::bearer_token();
    let context = format!("Request to API failed with {}", token);
    let redacted = redact_user_string(&context);

    assert!(redacted.contains("Request to API failed"));
    assert!(redacted.contains("***"));
    assert!(!redacted.contains(&token));
}

#[test]
fn test_redaction_in_warnings() {
    // Test that warnings with secrets are redacted
    let github_token = test_support::github_pat();
    let aws_key = test_support::aws_access_key_id();
    let warnings = vec![
        format!("Warning: deprecated token {}", github_token),
        "Warning: rate limit exceeded".to_string(),
        format!("Warning: AWS key {} is invalid", aws_key),
    ];

    let redacted = redact_user_strings(&warnings);

    assert_eq!(redacted.len(), 3);
    assert!(redacted[0].contains("***"));
    assert!(!redacted[0].contains("ghp_"));
    assert_eq!(redacted[1], "Warning: rate limit exceeded");
    assert!(redacted[2].contains("***"));
    assert!(!redacted[2].contains("AKIA"));
}

#[test]
fn test_redaction_in_stderr() {
    // Test that stderr output with secrets is redacted
    let token = test_support::github_pat();
    let stderr = format!(
        "Error: Authentication failed\nToken: {}\nPlease check your credentials",
        token
    );
    let redacted = redact_user_string(&stderr);

    assert!(redacted.contains("Error: Authentication failed"));
    assert!(redacted.contains("***"));
    assert!(!redacted.contains("ghp_"));
    assert!(redacted.contains("Please check your credentials"));
}

#[test]
fn test_redaction_preserves_structure() {
    // Test that redaction preserves the structure of the text
    let token = test_support::github_pat();
    let text = format!("Line 1: safe\nLine 2: {}\nLine 3: safe", token);
    let redacted = redact_user_string(&text);

    assert!(redacted.contains("Line 1: safe"));
    assert!(redacted.contains("Line 2:"));
    assert!(redacted.contains("***"));
    assert!(redacted.contains("Line 3: safe"));
    assert!(!redacted.contains("ghp_"));
}

#[test]
fn test_redaction_with_optional_none() {
    // Test that None values are handled correctly
    let none_value: Option<String> = None;
    let redacted = redact_user_optional(&none_value);

    assert!(redacted.is_none());
}

#[test]
fn test_redaction_with_optional_some() {
    // Test that Some values with secrets are redacted
    let token = test_support::github_pat();
    let some_value = Some(format!("Error with token {}", token));
    let redacted = redact_user_optional(&some_value);

    assert!(redacted.is_some());
    let redacted_str = redacted.unwrap();
    assert!(redacted_str.contains("***"));
    assert!(!redacted_str.contains("ghp_"));
}

#[test]
fn test_all_default_patterns_redacted() {
    let redactor = SecretRedactor::new().unwrap();

    // Test all default patterns
    let github_token = test_support::github_pat();
    let aws_key = test_support::aws_access_key_id();
    let aws_secret = test_support::aws_secret_access_key();
    let slack_token = test_support::slack_bot_token();
    let bearer_token = test_support::bearer_token();
    let test_cases = vec![
        ("GitHub PAT", format!("token: {}", github_token), "ghp_"),
        ("AWS Access Key", format!("key: {}", aws_key), "AKIA"),
        ("AWS Secret Key", aws_secret, "AWS_SECRET_ACCESS_KEY"),
        ("Slack Token", format!("slack: {}", slack_token), "xoxb-"),
        (
            "Bearer Token",
            format!("Authorization: {}", bearer_token),
            "Bearer eyJ",
        ),
    ];

    for (name, input, secret_part) in test_cases {
        let redacted = redactor.redact_string(&input);
        assert!(redacted.contains("***"), "Failed to redact {name}");
        assert!(
            !redacted.contains(secret_part),
            "Secret {name} still present in: {redacted}"
        );
    }
}

#[test]
fn test_redaction_in_file_paths() {
    // Test that secrets in file paths are redacted
    let token = test_support::github_pat();
    let path = format!("/home/user/.config/{}/config.yaml", token);
    let redacted = redact_user_string(&path);

    assert!(redacted.contains("/home/user/.config/"));
    assert!(redacted.contains("***"));
    assert!(!redacted.contains("ghp_"));
    assert!(redacted.contains("/config.yaml"));
}

#[test]
fn test_redaction_in_json_like_strings() {
    // Test that secrets in JSON-like strings are redacted
    let token = test_support::github_pat();
    let json_str = format!(r#"{{"token": "{}", "user": "test"}}"#, token);
    let redacted = redact_user_string(&json_str);

    assert!(redacted.contains(r#"{"token": "***"#));
    assert!(!redacted.contains("ghp_"));
    assert!(redacted.contains(r#""user": "test""#));
}

#[test]
fn test_redaction_empty_string() {
    // Test that empty strings are handled correctly
    let empty = "";
    let redacted = redact_user_string(empty);

    assert_eq!(redacted, "");
}

#[test]
fn test_redaction_whitespace_only() {
    // Test that whitespace-only strings are handled correctly
    let whitespace = "   \n\t  ";
    let redacted = redact_user_string(whitespace);

    assert_eq!(redacted, whitespace);
}

#[test]
fn test_redaction_multiple_occurrences() {
    // Test that multiple occurrences of the same secret are all redacted
    let token = test_support::github_pat();
    let text = format!("First: {token}, Second: {token}");
    let redacted = redact_user_string(&text);

    assert!(!redacted.contains("ghp_"));
    // Count occurrences of ***
    let count = redacted.matches("***").count();
    assert_eq!(count, 2, "Expected 2 redactions, got {count}");
}

#[test]
fn test_redaction_mixed_secrets() {
    // Test that different types of secrets in the same string are all redacted
    let github_token = test_support::github_pat();
    let aws_key = test_support::aws_access_key_id();
    let slack_token = test_support::slack_bot_token();
    let text = format!(
        "GitHub: {}, AWS: {}, Slack: {}",
        github_token, aws_key, slack_token
    );
    let redacted = redact_user_string(&text);

    assert!(!redacted.contains("ghp_"));
    assert!(!redacted.contains("AKIA"));
    assert!(!redacted.contains("xoxb-"));
    assert!(redacted.contains("***"));
}

#[test]
fn test_redaction_case_sensitivity() {
    // Test that patterns are case-sensitive where appropriate
    let redactor = SecretRedactor::new().unwrap();

    // AWS keys are uppercase
    let aws_upper = test_support::aws_access_key_id();
    let redacted_upper = redactor.redact_string(&aws_upper);
    assert!(redacted_upper.contains("***"));

    // Lowercase should not match (AWS keys are always uppercase)
    let aws_lower = aws_upper.to_lowercase();
    let redacted_lower = redactor.redact_string(&aws_lower);
    assert_eq!(redacted_lower, aws_lower); // Should not be redacted
}

#[test]
fn test_redaction_partial_matches() {
    // Test that partial matches are not redacted
    let text = "This is not a secret: ghp_short or AKIA_short";
    let redacted = redact_user_string(text);

    // These should not be redacted because they don't match the full pattern
    assert_eq!(redacted, text);
}

#[test]
fn test_redaction_in_urls() {
    // Test that secrets in URLs are redacted
    let token = test_support::github_pat();
    let url = format!("https://api.github.com/repos/owner/repo?token={}", token);
    let redacted = redact_user_string(&url);

    assert!(redacted.contains("https://api.github.com/repos/owner/repo?token="));
    assert!(redacted.contains("***"));
    assert!(!redacted.contains("ghp_"));
}

#[test]
fn test_redaction_in_command_output() {
    // Test that secrets in command output are redacted
    let token = test_support::bearer_token();
    let output = format!(
        "$ curl -H 'Authorization: {}' https://api.example.com\nHTTP/1.1 200 OK",
        token
    );
    let redacted = redact_user_string(&output);

    assert!(redacted.contains("$ curl -H 'Authorization:"));
    assert!(redacted.contains("***"));
    assert!(!redacted.contains("Bearer eyJ"));
    assert!(redacted.contains("HTTP/1.1 200 OK"));
}

#[test]
fn test_redaction_performance() {
    // Test that redaction performs reasonably on large strings
    let token = test_support::github_pat();
    let large_text = "safe text ".repeat(1000) + &token;

    let start = std::time::Instant::now();
    let redacted = redact_user_string(&large_text);
    let duration = start.elapsed();

    assert!(redacted.contains("***"));
    assert!(!redacted.contains("ghp_"));
    assert!(
        duration.as_millis() < 100,
        "Redaction took too long: {duration:?}"
    );
}

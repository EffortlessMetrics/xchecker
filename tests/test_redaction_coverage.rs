/// Integration tests for comprehensive redaction coverage
///
/// These tests verify that secrets are redacted in all user-facing output:
/// - Error messages
/// - Context strings
/// - Preview text
/// - Receipts (`stderr_tail`, warnings, `error_reason`)
/// - Status output
/// - Logs
use xchecker::redaction::{
    SecretRedactor, redact_user_optional, redact_user_string, redact_user_strings,
};

#[test]
fn test_redaction_in_error_messages() {
    // Test that error messages with secrets are redacted
    let error_msg =
        "Failed to authenticate with token ghp_1234567890123456789012345678901234567890";
    let redacted = redact_user_string(error_msg);

    assert!(redacted.contains("Failed to authenticate"));
    assert!(redacted.contains("***"));
    assert!(!redacted.contains("ghp_"));
    assert!(!redacted.contains("1234567890"));
}

#[test]
fn test_redaction_in_context_strings() {
    // Test that context strings with secrets are redacted
    let context = "Request to API failed with Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0";
    let redacted = redact_user_string(context);

    assert!(redacted.contains("Request to API failed"));
    assert!(redacted.contains("***"));
    assert!(!redacted.contains("Bearer eyJ"));
}

#[test]
fn test_redaction_in_warnings() {
    // Test that warnings with secrets are redacted
    let warnings = vec![
        "Warning: deprecated token ghp_1234567890123456789012345678901234567890".to_string(),
        "Warning: rate limit exceeded".to_string(),
        "Warning: AWS key AKIA1234567890123456 is invalid".to_string(),
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
    let stderr = "Error: Authentication failed\nToken: ghp_1234567890123456789012345678901234567890\nPlease check your credentials";
    let redacted = redact_user_string(stderr);

    assert!(redacted.contains("Error: Authentication failed"));
    assert!(redacted.contains("***"));
    assert!(!redacted.contains("ghp_"));
    assert!(redacted.contains("Please check your credentials"));
}

#[test]
fn test_redaction_preserves_structure() {
    // Test that redaction preserves the structure of the text
    let text = "Line 1: safe\nLine 2: ghp_1234567890123456789012345678901234567890\nLine 3: safe";
    let redacted = redact_user_string(text);

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
    let some_value =
        Some("Error with token ghp_1234567890123456789012345678901234567890".to_string());
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
    let test_cases = vec![
        (
            "GitHub PAT",
            "token: ghp_1234567890123456789012345678901234567890",
            "ghp_",
        ),
        ("AWS Access Key", "key: AKIA1234567890123456", "AKIA"),
        (
            "AWS Secret Key",
            "AWS_SECRET_ACCESS_KEY=mysecret",
            "AWS_SECRET_ACCESS_KEY",
        ),
        (
            "Slack Token",
            "slack: xoxb-1234567890-abcdefghijklmnop",
            "xoxb-",
        ),
        (
            "Bearer Token",
            "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9",
            "Bearer eyJ",
        ),
    ];

    for (name, input, secret_part) in test_cases {
        let redacted = redactor.redact_string(input);
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
    let path = "/home/user/.config/ghp_1234567890123456789012345678901234567890/config.yaml";
    let redacted = redact_user_string(path);

    assert!(redacted.contains("/home/user/.config/"));
    assert!(redacted.contains("***"));
    assert!(!redacted.contains("ghp_"));
    assert!(redacted.contains("/config.yaml"));
}

#[test]
fn test_redaction_in_json_like_strings() {
    // Test that secrets in JSON-like strings are redacted
    let json_str = r#"{"token": "ghp_1234567890123456789012345678901234567890", "user": "test"}"#;
    let redacted = redact_user_string(json_str);

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
    let text = "First: ghp_1234567890123456789012345678901234567890, Second: ghp_1234567890123456789012345678901234567890";
    let redacted = redact_user_string(text);

    assert!(!redacted.contains("ghp_"));
    // Count occurrences of ***
    let count = redacted.matches("***").count();
    assert_eq!(count, 2, "Expected 2 redactions, got {count}");
}

#[test]
fn test_redaction_mixed_secrets() {
    // Test that different types of secrets in the same string are all redacted
    let text = "GitHub: ghp_1234567890123456789012345678901234567890, AWS: AKIA1234567890123456, Slack: xoxb-test-token";
    let redacted = redact_user_string(text);

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
    let aws_upper = "AKIA1234567890123456";
    let redacted_upper = redactor.redact_string(aws_upper);
    assert!(redacted_upper.contains("***"));

    // Lowercase should not match (AWS keys are always uppercase)
    let aws_lower = "akia1234567890123456";
    let redacted_lower = redactor.redact_string(aws_lower);
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
    let url = "https://api.github.com/repos/owner/repo?token=ghp_1234567890123456789012345678901234567890";
    let redacted = redact_user_string(url);

    assert!(redacted.contains("https://api.github.com/repos/owner/repo?token="));
    assert!(redacted.contains("***"));
    assert!(!redacted.contains("ghp_"));
}

#[test]
fn test_redaction_in_command_output() {
    // Test that secrets in command output are redacted
    let output = "$ curl -H 'Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9' https://api.example.com\nHTTP/1.1 200 OK";
    let redacted = redact_user_string(output);

    assert!(redacted.contains("$ curl -H 'Authorization:"));
    assert!(redacted.contains("***"));
    assert!(!redacted.contains("Bearer eyJ"));
    assert!(redacted.contains("HTTP/1.1 200 OK"));
}

#[test]
fn test_redaction_performance() {
    // Test that redaction performs reasonably on large strings
    let large_text = "safe text ".repeat(1000) + "ghp_1234567890123456789012345678901234567890";

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

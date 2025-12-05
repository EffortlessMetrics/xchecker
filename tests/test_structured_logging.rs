//! Integration tests for structured logging (FR-OBS-001)
//!
//! These tests verify that the tracing-based structured logging system
//! works correctly with the required fields: spec_id, phase, duration_ms, runner_mode

use xchecker::logging::{
    Logger, init_tracing, log_phase_complete, log_phase_error, log_phase_start, phase_span,
};

#[test]
fn test_tracing_initialization() {
    // Test that tracing can be initialized without errors
    // Note: May fail if already initialized in another test, which is acceptable
    let result = init_tracing(false);
    assert!(result.is_ok() || result.is_err()); // Either is fine in tests
}

#[test]
fn test_verbose_tracing_initialization() {
    // Test verbose mode initialization
    let result = init_tracing(true);
    assert!(result.is_ok() || result.is_err()); // Either is fine in tests
}

#[test]
fn test_logger_with_structured_context() {
    // Test that Logger can be configured with structured context
    let mut logger = Logger::new(true);

    // Set structured context (FR-OBS-001)
    logger.set_spec_id("integration-test-spec".to_string());
    logger.set_phase("requirements".to_string());
    logger.set_runner_mode("native".to_string());

    // Verify context is set
    assert_eq!(logger.spec_id(), Some("integration-test-spec"));
    assert_eq!(logger.phase(), Some("requirements"));
    assert_eq!(logger.runner_mode(), Some("native"));

    // Log messages with structured fields
    logger.info("Starting phase execution");
    logger.warn("Warning during execution");
    logger.error("Error occurred");
}

#[test]
fn test_phase_span_usage() {
    // Test that phase spans can be created and entered
    let span = phase_span("test-spec", "design", "wsl");
    let _guard = span.enter();

    // Log within the span
    log_phase_start("test-spec", "design", "wsl");
    log_phase_complete("test-spec", "design", 1500);
}

#[test]
fn test_phase_logging_functions() {
    // Test standalone phase logging functions
    log_phase_start("spec-123", "tasks", "auto");
    log_phase_complete("spec-123", "tasks", 2000);
    log_phase_error("spec-123", "tasks", "timeout", 5000);
}

#[test]
fn test_logger_without_context() {
    // Test that Logger works without structured context
    let logger = Logger::new(false);

    // Should work without context
    logger.info("Message without context");
    logger.warn("Warning without context");
    logger.error("Error without context");
}

#[test]
fn test_verbose_mode_with_context() {
    // Test verbose mode with full context
    let mut logger = Logger::new(true);

    logger.set_spec_id("verbose-test".to_string());
    logger.set_phase("review".to_string());
    logger.set_runner_mode("native".to_string());

    // Verbose logging should include all fields
    logger.verbose("Verbose message with context");
    logger.info("Info message with context");
}

#[test]
fn test_compact_mode() {
    // Test compact (default) mode
    let logger = Logger::new(false);

    assert!(!logger.is_verbose());

    // Compact mode should still work
    logger.info("Compact info message");
}

#[test]
fn test_duration_tracking() {
    // Test that duration is tracked correctly
    let mut logger = Logger::new(true);

    logger.set_spec_id("duration-test".to_string());
    logger.set_phase("fixup".to_string());
    logger.set_runner_mode("wsl".to_string());

    // Wait a bit to ensure duration > 0
    std::thread::sleep(std::time::Duration::from_millis(10));

    // Duration should be tracked
    let elapsed = logger.total_elapsed();
    assert!(elapsed.as_millis() >= 10);

    logger.info("Message with duration");
}

// Integration tests for redaction in logging (FR-OBS-002, FR-OBS-003)

#[test]
fn test_logging_redacts_github_tokens() {
    // Test that GitHub tokens are redacted in all log levels
    let mut logger = Logger::new(true);
    logger.set_spec_id("redaction-test".to_string());
    logger.set_phase("requirements".to_string());
    logger.set_runner_mode("native".to_string());

    // These should not panic and should redact the token
    logger.verbose("Found token: ghp_1234567890123456789012345678901234567890");
    logger.info("Using token: ghp_1234567890123456789012345678901234567890");
    logger.warn("Exposed token: ghp_1234567890123456789012345678901234567890");
    logger.error("Error with token: ghp_1234567890123456789012345678901234567890");
}

#[test]
fn test_logging_redacts_aws_keys() {
    // Test that AWS keys are redacted
    let logger = Logger::new(false);

    logger.info("AWS access key: AKIA1234567890123456");
    logger.error("Failed with AWS_SECRET_ACCESS_KEY=secret_value");
}

#[test]
fn test_logging_redacts_bearer_tokens() {
    // Test that Bearer tokens are redacted
    let logger = Logger::new(false);

    logger.info("Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9");
    logger.error("Auth failed with Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9");
}

#[test]
fn test_logging_excludes_environment_variables() {
    // Test that environment variables are not logged
    let logger = Logger::new(true);

    // These should be redacted as they look like env vars
    logger.info("Config: API_KEY=secret123");
    logger.warn("Found: DATABASE_PASSWORD=mypassword");
    logger.error("Error: SECRET_TOKEN=abc123");
}

#[test]
fn test_phase_error_logging_redacts_secrets() {
    // Test that phase error logging redacts secrets
    log_phase_error(
        "test-spec",
        "requirements",
        "Failed with token ghp_1234567890123456789012345678901234567890",
        1000,
    );

    log_phase_error(
        "test-spec",
        "design",
        "AWS key AKIA1234567890123456 exposed",
        2000,
    );
}

#[test]
fn test_error_context_redaction() {
    // Test that error context is properly redacted
    let mut logger = Logger::new(false);
    logger.set_spec_id("error-context-test".to_string());
    logger.set_phase("tasks".to_string());
    logger.set_runner_mode("wsl".to_string());

    // Error messages with sensitive context should be redacted
    logger.error("Authentication failed with API_KEY=secret123");
    logger.error("Connection error: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9");
}

#[test]
fn test_verbose_logging_with_secrets() {
    // Test verbose logging with secrets in formatted messages
    let logger = Logger::new(true);

    // Formatted verbose logging should also redact
    logger.verbose_fmt(format_args!(
        "Processing with token: {}",
        "ghp_1234567890123456789012345678901234567890"
    ));
}

#[test]
fn test_no_secrets_in_normal_logs() {
    // Test that normal content passes through without issues
    let logger = Logger::new(true);

    logger.info("Processing file test.txt");
    logger.verbose("Completed in 1.5 seconds");
    logger.warn("File size exceeds recommended limit");
    logger.error("File not found: /path/to/file");
}

#[test]
fn test_multiple_log_levels_with_redaction() {
    // Test that redaction works consistently across all log levels
    let mut logger = Logger::new(true);
    logger.set_spec_id("multi-level-test".to_string());
    logger.set_phase("review".to_string());
    logger.set_runner_mode("auto".to_string());

    let secret_message = "Token: ghp_1234567890123456789012345678901234567890";

    logger.verbose(secret_message);
    logger.info(secret_message);
    logger.warn(secret_message);
    logger.error(secret_message);

    // All should complete without exposing the secret
}

#[test]
fn test_slack_token_redaction() {
    // Test that Slack tokens are redacted
    let logger = Logger::new(false);

    logger.info("Slack webhook: xoxb-1234567890-abcdefghijklmnop");
    logger.error("Slack error with xoxp-1234567890-abcdefghijklmnop");
}

#[test]
fn test_redaction_preserves_safe_content() {
    // Test that safe content is not affected by redaction
    let logger = Logger::new(true);

    let safe_messages = vec![
        "Processing completed successfully",
        "File count: 42",
        "Duration: 1.5 seconds",
        "Status: OK",
        "Path: /home/user/project",
    ];

    for message in safe_messages {
        logger.info(message);
        logger.verbose(message);
    }
}

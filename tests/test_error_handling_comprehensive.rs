//! Comprehensive Error Handling Tests (FR-EXIT)
//!
//! **WHITE-BOX TEST**: This test uses internal module APIs (`error::{...}`, `exit_codes::codes`,
//! `lock::LockError`, `receipt::ReceiptManager`, `types::{...}`) and may break with internal
//! refactors. These tests are intentionally white-box to validate internal implementation details.
//! See FR-TEST-4 for white-box test policy.
//!
//! This test suite validates FR-EXIT-001 through FR-EXIT-009:
//! 1. Each ErrorKind maps to correct exit code
//! 2. error_kind and error_reason are present in receipts
//! 3. Receipts are written on all error paths
//! 4. Exit code matches receipt exit_code field
//! 5. Error context and suggestions are user-friendly
//! 6. Integration tests for each error scenario

use std::collections::HashMap;
use std::fs;
use tempfile::TempDir;
use xchecker::error::{
    ClaudeError, ConfigError, PhaseError, RunnerError, SourceError, UserFriendlyError,
    XCheckerError,
};
use xchecker::exit_codes::codes;
use xchecker::lock::LockError;
use xchecker::receipt::ReceiptManager;
use xchecker::types::{ErrorKind, PacketEvidence, PhaseId};

/// Test FR-EXIT-001: Success exit code
#[test]
fn test_success_exit_code() {
    assert_eq!(codes::SUCCESS, 0, "Success should be exit code 0");
}

/// Test FR-EXIT-002: CLI arguments error exit code
#[test]
fn test_cli_args_exit_code_mapping() {
    let test_cases = vec![
        XCheckerError::Config(ConfigError::InvalidFile("bad syntax".to_string())),
        XCheckerError::Config(ConfigError::MissingRequired("model".to_string())),
        XCheckerError::Config(ConfigError::InvalidValue {
            key: "packet_max_bytes".to_string(),
            value: "invalid".to_string(),
        }),
        XCheckerError::Config(ConfigError::NotFound {
            path: "/nonexistent/config.toml".to_string(),
        }),
    ];

    for err in test_cases {
        let (code, kind) = (&err).into();
        assert_eq!(
            code,
            codes::CLI_ARGS,
            "ConfigError should map to CLI_ARGS exit code for: {}",
            err
        );
        assert_eq!(
            kind,
            ErrorKind::CliArgs,
            "ConfigError should map to CliArgs error kind for: {}",
            err
        );
    }
}

/// Test FR-EXIT-003: Packet overflow error exit code
#[test]
fn test_packet_overflow_exit_code_mapping() {
    let err = XCheckerError::PacketOverflow {
        used_bytes: 100000,
        used_lines: 2000,
        limit_bytes: 65536,
        limit_lines: 1200,
    };

    let (code, kind) = (&err).into();
    assert_eq!(
        code,
        codes::PACKET_OVERFLOW,
        "PacketOverflow should map to exit code 7"
    );
    assert_eq!(
        kind,
        ErrorKind::PacketOverflow,
        "PacketOverflow should map to PacketOverflow error kind"
    );
}

/// Test FR-EXIT-004: Secret detected error exit code
#[test]
fn test_secret_detected_exit_code_mapping() {
    let test_patterns = vec!["ghp_", "AKIA", "AWS_SECRET_ACCESS_KEY", "xoxb-", "Bearer"];

    for pattern in test_patterns {
        let err = XCheckerError::SecretDetected {
            pattern: pattern.to_string(),
            location: "test.txt".to_string(),
        };

        let (code, kind) = (&err).into();
        assert_eq!(
            code,
            codes::SECRET_DETECTED,
            "SecretDetected with pattern '{}' should map to exit code 8",
            pattern
        );
        assert_eq!(
            kind,
            ErrorKind::SecretDetected,
            "SecretDetected should map to SecretDetected error kind"
        );
    }
}

/// Test FR-EXIT-005: Lock held error exit code
#[test]
fn test_lock_held_exit_code_mapping() {
    // Test ConcurrentExecution variant
    let err1 = XCheckerError::ConcurrentExecution {
        id: "test-spec".to_string(),
    };
    let (code1, kind1) = (&err1).into();
    assert_eq!(
        code1,
        codes::LOCK_HELD,
        "ConcurrentExecution should map to exit code 9"
    );
    assert_eq!(
        kind1,
        ErrorKind::LockHeld,
        "ConcurrentExecution should map to LockHeld error kind"
    );

    // Test Lock error variant
    let lock_err = LockError::ConcurrentExecution {
        spec_id: "test-spec".to_string(),
        pid: 12345,
        created_ago: "5m".to_string(),
    };
    let err2 = XCheckerError::Lock(lock_err);
    let (code2, kind2) = (&err2).into();
    assert_eq!(
        code2,
        codes::LOCK_HELD,
        "Lock(ConcurrentExecution) should map to exit code 9"
    );
    assert_eq!(
        kind2,
        ErrorKind::LockHeld,
        "Lock error should map to LockHeld error kind"
    );
}

/// Test FR-EXIT-006: Phase timeout error exit code
#[test]
fn test_phase_timeout_exit_code_mapping() {
    let phases = vec!["REQUIREMENTS", "DESIGN", "TASKS", "REVIEW", "FIXUP"];

    for phase in phases {
        let phase_err = PhaseError::Timeout {
            phase: phase.to_string(),
            timeout_seconds: 600,
        };
        let err = XCheckerError::Phase(phase_err);

        let (code, kind) = (&err).into();
        assert_eq!(
            code,
            codes::PHASE_TIMEOUT,
            "Timeout in {} phase should map to exit code 10",
            phase
        );
        assert_eq!(
            kind,
            ErrorKind::PhaseTimeout,
            "Phase timeout should map to PhaseTimeout error kind"
        );
    }
}

/// Test FR-EXIT-007: Claude failure error exit code
#[test]
fn test_claude_failure_exit_code_mapping() {
    // Test ClaudeError variants
    let claude_errors = vec![
        XCheckerError::Claude(ClaudeError::ExecutionFailed {
            stderr: "API error".to_string(),
        }),
        XCheckerError::Claude(ClaudeError::ParseError {
            reason: "malformed JSON".to_string(),
        }),
        XCheckerError::Claude(ClaudeError::ModelNotAvailable {
            model: "invalid-model".to_string(),
        }),
        XCheckerError::Claude(ClaudeError::NotFound),
        XCheckerError::Claude(ClaudeError::AuthenticationFailed {
            reason: "invalid token".to_string(),
        }),
    ];

    for err in claude_errors {
        let (code, kind) = (&err).into();
        assert_eq!(
            code,
            codes::CLAUDE_FAILURE,
            "ClaudeError should map to exit code 70 for: {}",
            err
        );
        assert_eq!(
            kind,
            ErrorKind::ClaudeFailure,
            "Claude error should map to ClaudeFailure error kind"
        );
    }

    // Test RunnerError variants
    let runner_errors = vec![
        XCheckerError::Runner(RunnerError::NativeExecutionFailed {
            reason: "command not found".to_string(),
        }),
        XCheckerError::Runner(RunnerError::WslExecutionFailed {
            reason: "WSL not running".to_string(),
        }),
        XCheckerError::Runner(RunnerError::ClaudeNotFoundInRunner {
            runner: "native".to_string(),
        }),
    ];

    for err in runner_errors {
        let (code, kind) = (&err).into();
        assert_eq!(
            code,
            codes::CLAUDE_FAILURE,
            "RunnerError should map to exit code 70 for: {}",
            err
        );
        assert_eq!(
            kind,
            ErrorKind::ClaudeFailure,
            "Runner error should map to ClaudeFailure error kind"
        );
    }
}

/// Test FR-EXIT-008: Error receipts contain error_kind and error_reason
#[test]
fn test_error_receipts_contain_required_fields() {
    let temp_dir = TempDir::new().unwrap();
    let spec_base = camino::Utf8PathBuf::from_path_buf(
        temp_dir.path().join(".xchecker/specs/test-error-fields"),
    )
    .unwrap();

    fs::create_dir_all(&spec_base).unwrap();

    let receipt_manager = ReceiptManager::new(&spec_base);

    let test_errors = vec![
        (
            XCheckerError::PacketOverflow {
                used_bytes: 100000,
                used_lines: 2000,
                limit_bytes: 65536,
                limit_lines: 1200,
            },
            ErrorKind::PacketOverflow,
        ),
        (
            XCheckerError::SecretDetected {
                pattern: "ghp_".to_string(),
                location: "test.txt".to_string(),
            },
            ErrorKind::SecretDetected,
        ),
        (
            XCheckerError::ConcurrentExecution {
                id: "test-spec".to_string(),
            },
            ErrorKind::LockHeld,
        ),
        (
            XCheckerError::Phase(PhaseError::Timeout {
                phase: "REQUIREMENTS".to_string(),
                timeout_seconds: 600,
            }),
            ErrorKind::PhaseTimeout,
        ),
    ];

    for (error, expected_kind) in test_errors {
        let receipt = receipt_manager.create_error_receipt(
            "test-error-fields",
            PhaseId::Requirements,
            &error,
            "0.1.0",
            "0.8.1",
            "haiku",
            None,
            HashMap::new(),
            PacketEvidence {
                files: vec![],
                max_bytes: 65536,
                max_lines: 1200,
            },
            None,     // stderr_tail
            None,     // stderr_redacted
            vec![],   // warnings
            None,     // fallback_used
            "native", // runner
            None,     // runner_distro
            None,     // diff_context,
            None,     // pipeline
        );

        // FR-EXIT-008: Verify error_kind is present
        assert!(
            receipt.error_kind.is_some(),
            "Receipt should have error_kind for error: {}",
            error
        );
        assert_eq!(
            receipt.error_kind.unwrap(),
            expected_kind,
            "Receipt error_kind should match expected for error: {}",
            error
        );

        // FR-EXIT-008: Verify error_reason is present
        assert!(
            receipt.error_reason.is_some(),
            "Receipt should have error_reason for error: {}",
            error
        );
        assert!(
            !receipt.error_reason.as_ref().unwrap().is_empty(),
            "Receipt error_reason should not be empty for error: {}",
            error
        );
    }
}

/// Test FR-EXIT-009: Exit code matches receipt exit_code field
#[test]
fn test_exit_code_matches_receipt_field() {
    let temp_dir = TempDir::new().unwrap();
    let spec_base = camino::Utf8PathBuf::from_path_buf(
        temp_dir.path().join(".xchecker/specs/test-exit-code-match"),
    )
    .unwrap();

    fs::create_dir_all(&spec_base).unwrap();

    let receipt_manager = ReceiptManager::new(&spec_base);

    let test_cases = vec![
        (
            XCheckerError::Config(ConfigError::InvalidFile("test".to_string())),
            codes::CLI_ARGS,
        ),
        (
            XCheckerError::PacketOverflow {
                used_bytes: 100000,
                used_lines: 2000,
                limit_bytes: 65536,
                limit_lines: 1200,
            },
            codes::PACKET_OVERFLOW,
        ),
        (
            XCheckerError::SecretDetected {
                pattern: "ghp_".to_string(),
                location: "test.txt".to_string(),
            },
            codes::SECRET_DETECTED,
        ),
        (
            XCheckerError::ConcurrentExecution {
                id: "test-spec".to_string(),
            },
            codes::LOCK_HELD,
        ),
        (
            XCheckerError::Phase(PhaseError::Timeout {
                phase: "REQUIREMENTS".to_string(),
                timeout_seconds: 600,
            }),
            codes::PHASE_TIMEOUT,
        ),
        (
            XCheckerError::Claude(ClaudeError::ExecutionFailed {
                stderr: "API error".to_string(),
            }),
            codes::CLAUDE_FAILURE,
        ),
    ];

    for (error, expected_exit_code) in test_cases {
        // Get exit code from error mapping
        let (mapped_exit_code, _): (i32, ErrorKind) = (&error).into();

        // Create receipt
        let receipt = receipt_manager.create_error_receipt(
            "test-exit-code-match",
            PhaseId::Requirements,
            &error,
            "0.1.0",
            "0.8.1",
            "haiku",
            None,
            HashMap::new(),
            PacketEvidence {
                files: vec![],
                max_bytes: 65536,
                max_lines: 1200,
            },
            None,     // stderr_tail
            None,     // stderr_redacted
            vec![],   // warnings
            None,     // fallback_used
            "native", // runner
            None,     // runner_distro
            None,     // diff_context,
            None,     // pipeline
        );

        // FR-EXIT-009: Verify receipt exit_code matches expected
        assert_eq!(
            receipt.exit_code, expected_exit_code,
            "Receipt exit_code should match expected for error: {}",
            error
        );

        // FR-EXIT-009: Verify receipt exit_code matches mapped exit code
        assert_eq!(
            receipt.exit_code, mapped_exit_code,
            "Receipt exit_code should match mapped exit code for error: {}",
            error
        );
    }
}

/// Test error context and suggestions are user-friendly
#[test]
fn test_user_friendly_error_messages() {
    let test_cases: Vec<Box<dyn UserFriendlyError>> = vec![
        Box::new(ConfigError::InvalidFile("bad syntax".to_string())),
        Box::new(ConfigError::MissingRequired("model".to_string())),
        Box::new(PhaseError::ExecutionFailed {
            phase: "REQUIREMENTS".to_string(),
            code: 1,
        }),
        Box::new(PhaseError::Timeout {
            phase: "DESIGN".to_string(),
            timeout_seconds: 600,
        }),
        Box::new(ClaudeError::ExecutionFailed {
            stderr: "API error".to_string(),
        }),
        Box::new(RunnerError::NativeExecutionFailed {
            reason: "command not found".to_string(),
        }),
        Box::new(SourceError::GitHubRepoNotFound {
            owner: "test".to_string(),
            repo: "repo".to_string(),
        }),
    ];

    for error in test_cases {
        // Verify user message is not empty
        let user_message = error.user_message();
        assert!(!user_message.is_empty(), "User message should not be empty");
        assert!(
            user_message.len() > 10,
            "User message should be descriptive"
        );

        // Verify context is provided
        let context = error.context();
        if let Some(ctx) = context {
            assert!(!ctx.is_empty(), "Context should not be empty if provided");
        }

        // Verify suggestions are actionable
        let suggestions = error.suggestions();
        assert!(
            !suggestions.is_empty(),
            "Suggestions should be provided for all errors"
        );
        for suggestion in suggestions {
            assert!(
                !suggestion.is_empty(),
                "Each suggestion should be non-empty"
            );
            assert!(
                suggestion.len() > 10,
                "Each suggestion should be descriptive"
            );
        }
    }
}

/// Test error kind serialization to snake_case
#[test]
fn test_error_kind_serialization() {
    let test_cases = vec![
        (ErrorKind::CliArgs, "cli_args"),
        (ErrorKind::PacketOverflow, "packet_overflow"),
        (ErrorKind::SecretDetected, "secret_detected"),
        (ErrorKind::LockHeld, "lock_held"),
        (ErrorKind::PhaseTimeout, "phase_timeout"),
        (ErrorKind::ClaudeFailure, "claude_failure"),
        (ErrorKind::Unknown, "unknown"),
    ];

    for (kind, expected) in test_cases {
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(
            json,
            format!(r#""{}""#, expected),
            "ErrorKind::{:?} should serialize to {}",
            kind,
            expected
        );
    }
}

/// Test that all error paths write receipts
#[test]
fn test_all_error_paths_write_receipts() {
    let temp_dir = TempDir::new().unwrap();
    let spec_base = camino::Utf8PathBuf::from_path_buf(
        temp_dir.path().join(".xchecker/specs/test-error-paths"),
    )
    .unwrap();

    fs::create_dir_all(&spec_base).unwrap();

    let receipt_manager = ReceiptManager::new(&spec_base);

    // Test all major error types
    let errors = vec![
        XCheckerError::Config(ConfigError::InvalidFile("test".to_string())),
        XCheckerError::PacketOverflow {
            used_bytes: 100000,
            used_lines: 2000,
            limit_bytes: 65536,
            limit_lines: 1200,
        },
        XCheckerError::SecretDetected {
            pattern: "ghp_".to_string(),
            location: "test.txt".to_string(),
        },
        XCheckerError::ConcurrentExecution {
            id: "test-spec".to_string(),
        },
        XCheckerError::Phase(PhaseError::Timeout {
            phase: "REQUIREMENTS".to_string(),
            timeout_seconds: 600,
        }),
        XCheckerError::Claude(ClaudeError::ExecutionFailed {
            stderr: "API error".to_string(),
        }),
        XCheckerError::Runner(RunnerError::NativeExecutionFailed {
            reason: "command not found".to_string(),
        }),
    ];

    for error in errors {
        let receipt = receipt_manager.create_error_receipt(
            "test-error-paths",
            PhaseId::Requirements,
            &error,
            "0.1.0",
            "0.8.1",
            "haiku",
            None,
            HashMap::new(),
            PacketEvidence {
                files: vec![],
                max_bytes: 65536,
                max_lines: 1200,
            },
            None,     // stderr_tail
            None,     // stderr_redacted
            vec![],   // warnings
            None,     // fallback_used
            "native", // runner
            None,     // runner_distro
            None,     // diff_context,
            None,     // pipeline
        );

        // Write receipt to disk
        let receipt_path = receipt_manager.write_receipt(&receipt).unwrap();

        // Verify receipt was written
        assert!(
            receipt_path.exists(),
            "Receipt should be written for error: {}",
            error
        );

        // Read and verify receipt content
        let content = fs::read_to_string(receipt_path.as_std_path()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

        // Verify required fields
        assert!(
            parsed["schema_version"].is_string(),
            "Receipt should have schema_version"
        );
        assert!(
            parsed["emitted_at"].is_string(),
            "Receipt should have emitted_at"
        );
        assert!(
            parsed["exit_code"].is_number(),
            "Receipt should have exit_code"
        );
        assert!(
            parsed["error_kind"].is_string(),
            "Receipt should have error_kind for error: {}",
            error
        );
        assert!(
            parsed["error_reason"].is_string(),
            "Receipt should have error_reason for error: {}",
            error
        );
    }
}

/// Test non-timeout phase errors map to Unknown
#[test]
fn test_non_timeout_phase_errors() {
    // ExecutionFailed maps to Unknown (generic phase failure)
    let err = XCheckerError::Phase(PhaseError::ExecutionFailed {
        phase: "REQUIREMENTS".to_string(),
        code: 1,
    });
    let (code, kind) = (&err).into();
    assert_eq!(code, 1, "ExecutionFailed should map to exit code 1");
    assert_eq!(
        kind,
        ErrorKind::Unknown,
        "ExecutionFailed should map to Unknown error kind"
    );

    // DependencyNotSatisfied and InvalidTransition map to CLI_ARGS (user error)
    // These are considered CLI argument errors per FR-ORC-001, FR-ORC-002
    let user_errors = vec![
        XCheckerError::Phase(PhaseError::DependencyNotSatisfied {
            phase: "DESIGN".to_string(),
            dependency: "REQUIREMENTS".to_string(),
        }),
        XCheckerError::Phase(PhaseError::InvalidTransition {
            from: "REQUIREMENTS".to_string(),
            to: "FIXUP".to_string(),
        }),
    ];

    for err in user_errors {
        let (code, kind) = (&err).into();
        assert_eq!(
            code, 2,
            "DependencyNotSatisfied/InvalidTransition should map to exit code 2 (CLI_ARGS) for: {}",
            err
        );
        assert_eq!(
            kind,
            ErrorKind::CliArgs,
            "DependencyNotSatisfied/InvalidTransition should map to CliArgs error kind"
        );
    }
}

/// Test IO errors map to Unknown
#[test]
fn test_io_errors_map_to_unknown() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let err = XCheckerError::Io(io_err);

    let (code, kind) = (&err).into();
    assert_eq!(code, 1, "IO errors should map to exit code 1");
    assert_eq!(kind, ErrorKind::Unknown, "IO errors should map to Unknown");
}

/// Test that error messages don't contain sensitive information
#[test]
fn test_error_messages_no_sensitive_info() {
    // Create errors with potentially sensitive data
    let secret_value = format!("ghp_{}", "a".repeat(16));
    let err1 = XCheckerError::SecretDetected {
        pattern: secret_value.clone(),
        location: "/home/user/.env".to_string(),
    };

    let user_msg = err1.user_message();

    // Verify the actual secret value is not in the message
    // (pattern name is OK, but not the actual matched value)
    assert!(
        !user_msg.contains(&secret_value),
        "Error message should not contain actual secret value"
    );
}

/// Test error categories are correctly assigned
#[test]
fn test_error_categories() {
    use xchecker::error::{ErrorCategory, UserFriendlyError};

    let test_cases: Vec<(Box<dyn UserFriendlyError>, ErrorCategory)> = vec![
        (
            Box::new(ConfigError::InvalidFile("test".to_string())),
            ErrorCategory::Configuration,
        ),
        (
            Box::new(PhaseError::ExecutionFailed {
                phase: "REQUIREMENTS".to_string(),
                code: 1,
            }),
            ErrorCategory::PhaseExecution,
        ),
        (
            Box::new(ClaudeError::ExecutionFailed {
                stderr: "error".to_string(),
            }),
            ErrorCategory::ClaudeIntegration,
        ),
        (
            Box::new(RunnerError::NativeExecutionFailed {
                reason: "error".to_string(),
            }),
            ErrorCategory::ClaudeIntegration,
        ),
    ];

    for (error, expected_category) in test_cases {
        let category = error.category();
        assert_eq!(
            category, expected_category,
            "Error should have correct category"
        );
    }
}

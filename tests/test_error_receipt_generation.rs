//! Comprehensive Error Receipt Generation Tests (Task 7.1 - FR-EXIT)
//!
//! This test suite validates that:
//! 1. error_to_exit_code_and_kind() mapping function works correctly
//! 2. write_error_receipt_and_exit() function creates receipts with matching exit codes
//! 3. ReceiptManager::create_error_receipt() method populates all required fields
//! 4. exit_code field matches process exit code for all error types
//! 5. error_kind and error_reason are populated correctly
//! 6. All error types map to correct exit codes and error kinds
//! 7. Receipts are written on all error paths
//!
//! Requirements: FR-EXIT-001 through FR-EXIT-009

use std::collections::HashMap;
use std::fs;
use tempfile::TempDir;
use xchecker::error::{ClaudeError, ConfigError, PhaseError, RunnerError, XCheckerError};
use xchecker::exit_codes::codes;
use xchecker::lock::LockError;
use xchecker::receipt::ReceiptManager;
use xchecker::types::{ErrorKind, PacketEvidence, PhaseId};

/// Helper to create a test receipt manager
fn create_test_manager() -> (ReceiptManager, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let spec_base = camino::Utf8PathBuf::from_path_buf(
        temp_dir.path().join(".xchecker/specs/test-error-receipt"),
    )
    .unwrap();

    fs::create_dir_all(&spec_base).unwrap();

    let manager = ReceiptManager::new(&spec_base);
    (manager, temp_dir)
}

/// Test FR-EXIT-001: CliArgs error maps to exit code 2
#[test]
fn test_cli_args_error_mapping() {
    let (manager, _temp_dir) = create_test_manager();

    let error = XCheckerError::Config(ConfigError::InvalidFile("test.toml".to_string()));

    // Test error mapping
    let (exit_code, error_kind): (i32, ErrorKind) = (&error).into();
    assert_eq!(exit_code, codes::CLI_ARGS);
    assert_eq!(error_kind, ErrorKind::CliArgs);

    // Test receipt creation
    let receipt = manager.create_error_receipt(
        "test-cli-args",
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

    assert_eq!(receipt.exit_code, codes::CLI_ARGS);
    assert_eq!(receipt.error_kind, Some(ErrorKind::CliArgs));
    assert!(receipt.error_reason.is_some());
    assert!(receipt.error_reason.as_ref().unwrap().contains("test.toml"));
}

/// Test FR-EXIT-002: PacketOverflow error maps to exit code 7
#[test]
fn test_packet_overflow_error_mapping() {
    let (manager, _temp_dir) = create_test_manager();

    let error = XCheckerError::PacketOverflow {
        used_bytes: 100000,
        used_lines: 2000,
        limit_bytes: 65536,
        limit_lines: 1200,
    };

    // Test error mapping
    let (exit_code, error_kind): (i32, ErrorKind) = (&error).into();
    assert_eq!(exit_code, codes::PACKET_OVERFLOW);
    assert_eq!(error_kind, ErrorKind::PacketOverflow);

    // Test receipt creation
    let receipt = manager.create_error_receipt(
        "test-packet-overflow",
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

    assert_eq!(receipt.exit_code, codes::PACKET_OVERFLOW);
    assert_eq!(receipt.error_kind, Some(ErrorKind::PacketOverflow));
    assert!(receipt.error_reason.is_some());
    assert!(receipt.error_reason.as_ref().unwrap().contains("100000"));
    assert!(receipt.error_reason.as_ref().unwrap().contains("65536"));
}

/// Test FR-EXIT-003: SecretDetected error maps to exit code 8
#[test]
fn test_secret_detected_error_mapping() {
    let (manager, _temp_dir) = create_test_manager();

    let error = XCheckerError::SecretDetected {
        pattern: "ghp_".to_string(),
        location: "test.txt:42".to_string(),
    };

    // Test error mapping
    let (exit_code, error_kind): (i32, ErrorKind) = (&error).into();
    assert_eq!(exit_code, codes::SECRET_DETECTED);
    assert_eq!(error_kind, ErrorKind::SecretDetected);

    // Test receipt creation
    let receipt = manager.create_error_receipt(
        "test-secret-detected",
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

    assert_eq!(receipt.exit_code, codes::SECRET_DETECTED);
    assert_eq!(receipt.error_kind, Some(ErrorKind::SecretDetected));
    assert!(receipt.error_reason.is_some());
    assert!(receipt.error_reason.as_ref().unwrap().contains("ghp_"));
}

/// Test FR-EXIT-004: Lock error maps to exit code 9
#[test]
fn test_lock_held_error_mapping() {
    let (manager, _temp_dir) = create_test_manager();

    // Test ConcurrentExecution variant
    let error1 = XCheckerError::ConcurrentExecution {
        id: "test-spec".to_string(),
    };

    let (exit_code1, error_kind1): (i32, ErrorKind) = (&error1).into();
    assert_eq!(exit_code1, codes::LOCK_HELD);
    assert_eq!(error_kind1, ErrorKind::LockHeld);

    // Test Lock variant
    let error2 = XCheckerError::Lock(LockError::ConcurrentExecution {
        spec_id: "test-spec".to_string(),
        pid: 12345,
        created_ago: "5m".to_string(),
    });

    let (exit_code2, error_kind2): (i32, ErrorKind) = (&error2).into();
    assert_eq!(exit_code2, codes::LOCK_HELD);
    assert_eq!(error_kind2, ErrorKind::LockHeld);

    // Test receipt creation
    let receipt = manager.create_error_receipt(
        "test-lock-held",
        PhaseId::Requirements,
        &error1,
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

    assert_eq!(receipt.exit_code, codes::LOCK_HELD);
    assert_eq!(receipt.error_kind, Some(ErrorKind::LockHeld));
    assert!(receipt.error_reason.is_some());
}

/// Test FR-EXIT-005: PhaseTimeout error maps to exit code 10
#[test]
fn test_phase_timeout_error_mapping() {
    let (manager, _temp_dir) = create_test_manager();

    let error = XCheckerError::Phase(PhaseError::Timeout {
        phase: "REQUIREMENTS".to_string(),
        timeout_seconds: 600,
    });

    // Test error mapping
    let (exit_code, error_kind): (i32, ErrorKind) = (&error).into();
    assert_eq!(exit_code, codes::PHASE_TIMEOUT);
    assert_eq!(error_kind, ErrorKind::PhaseTimeout);

    // Test receipt creation
    let receipt = manager.create_error_receipt(
        "test-phase-timeout",
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

    assert_eq!(receipt.exit_code, codes::PHASE_TIMEOUT);
    assert_eq!(receipt.error_kind, Some(ErrorKind::PhaseTimeout));
    assert!(receipt.error_reason.is_some());
    assert!(receipt.error_reason.as_ref().unwrap().contains("600"));
}

/// Test FR-EXIT-006: Claude/Runner error maps to exit code 70
#[test]
fn test_claude_failure_error_mapping() {
    let (manager, _temp_dir) = create_test_manager();

    // Test ClaudeError variant
    let error1 = XCheckerError::Claude(ClaudeError::ExecutionFailed {
        stderr: "API error".to_string(),
    });

    let (exit_code1, error_kind1): (i32, ErrorKind) = (&error1).into();
    assert_eq!(exit_code1, codes::CLAUDE_FAILURE);
    assert_eq!(error_kind1, ErrorKind::ClaudeFailure);

    // Test RunnerError variant
    let error2 = XCheckerError::Runner(RunnerError::NativeExecutionFailed {
        reason: "command not found".to_string(),
    });

    let (exit_code2, error_kind2): (i32, ErrorKind) = (&error2).into();
    assert_eq!(exit_code2, codes::CLAUDE_FAILURE);
    assert_eq!(error_kind2, ErrorKind::ClaudeFailure);

    // Test receipt creation
    let receipt = manager.create_error_receipt(
        "test-claude-failure",
        PhaseId::Requirements,
        &error1,
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

    assert_eq!(receipt.exit_code, codes::CLAUDE_FAILURE);
    assert_eq!(receipt.error_kind, Some(ErrorKind::ClaudeFailure));
    assert!(receipt.error_reason.is_some());
}

/// Test FR-EXIT-007: Unknown error maps to exit code 1
#[test]
fn test_unknown_error_mapping() {
    let (manager, _temp_dir) = create_test_manager();

    // Test IO error (maps to Unknown)
    let error = XCheckerError::Io(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "file not found",
    ));

    // Test error mapping
    let (exit_code, error_kind): (i32, ErrorKind) = (&error).into();
    assert_eq!(exit_code, 1);
    assert_eq!(error_kind, ErrorKind::Unknown);

    // Test receipt creation
    let receipt = manager.create_error_receipt(
        "test-unknown",
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

    assert_eq!(receipt.exit_code, 1);
    assert_eq!(receipt.error_kind, Some(ErrorKind::Unknown));
    assert!(receipt.error_reason.is_some());
}

/// Test FR-EXIT-008: Error receipts contain error_kind and error_reason
#[test]
fn test_error_receipts_contain_required_fields() {
    let (manager, _temp_dir) = create_test_manager();

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
        let receipt = manager.create_error_receipt(
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
    let (manager, _temp_dir) = create_test_manager();

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
                stderr: "error".to_string(),
            }),
            codes::CLAUDE_FAILURE,
        ),
        (
            XCheckerError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "not found",
            )),
            1, // Unknown error
        ),
    ];

    for (error, expected_exit_code) in test_cases {
        // Test error mapping
        let (exit_code, _): (i32, ErrorKind) = (&error).into();
        assert_eq!(
            exit_code, expected_exit_code,
            "Exit code should match expected for error: {}",
            error
        );

        // Test receipt creation
        let receipt = manager.create_error_receipt(
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
    }
}

/// Test that receipts are written to disk correctly
#[test]
fn test_receipt_written_on_error_path() {
    let (manager, _temp_dir) = create_test_manager();

    let error = XCheckerError::PacketOverflow {
        used_bytes: 100000,
        used_lines: 2000,
        limit_bytes: 65536,
        limit_lines: 1200,
    };

    let receipt = manager.create_error_receipt(
        "test-write-receipt",
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
    let receipt_path = manager.write_receipt(&receipt).unwrap();

    // Verify receipt file exists
    assert!(receipt_path.exists(), "Receipt file should exist on disk");

    // Verify receipt file contains expected content
    let receipt_content = fs::read_to_string(receipt_path.as_std_path()).unwrap();
    assert!(receipt_content.contains("\"exit_code\":7"));
    assert!(receipt_content.contains("\"error_kind\":\"packet_overflow\""));
    assert!(receipt_content.contains("\"error_reason\""));
}

/// Test that error receipts have no outputs
#[test]
fn test_error_receipts_have_no_outputs() {
    let (manager, _temp_dir) = create_test_manager();

    let error = XCheckerError::PacketOverflow {
        used_bytes: 100000,
        used_lines: 2000,
        limit_bytes: 65536,
        limit_lines: 1200,
    };

    let receipt = manager.create_error_receipt(
        "test-no-outputs",
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

    // Error receipts should have no outputs
    assert_eq!(receipt.outputs.len(), 0);
}

/// Test that invalid phase transitions map to CLI_ARGS error
#[test]
fn test_invalid_transition_maps_to_cli_args() {
    let (manager, _temp_dir) = create_test_manager();

    let error = XCheckerError::Phase(PhaseError::InvalidTransition {
        from: "none".to_string(),
        to: "design".to_string(),
    });

    // Test error mapping
    let (exit_code, error_kind): (i32, ErrorKind) = (&error).into();
    assert_eq!(exit_code, codes::CLI_ARGS);
    assert_eq!(error_kind, ErrorKind::CliArgs);

    // Test receipt creation
    let receipt = manager.create_error_receipt(
        "test-invalid-transition",
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

    assert_eq!(receipt.exit_code, codes::CLI_ARGS);
    assert_eq!(receipt.error_kind, Some(ErrorKind::CliArgs));
}

/// Test that dependency not satisfied maps to CLI_ARGS error
#[test]
fn test_dependency_not_satisfied_maps_to_cli_args() {
    let (manager, _temp_dir) = create_test_manager();

    let error = XCheckerError::Phase(PhaseError::DependencyNotSatisfied {
        phase: "design".to_string(),
        dependency: "requirements".to_string(),
    });

    // Test error mapping
    let (exit_code, error_kind): (i32, ErrorKind) = (&error).into();
    assert_eq!(exit_code, codes::CLI_ARGS);
    assert_eq!(error_kind, ErrorKind::CliArgs);

    // Test receipt creation
    let receipt = manager.create_error_receipt(
        "test-dependency-not-satisfied",
        PhaseId::Design,
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

    assert_eq!(receipt.exit_code, codes::CLI_ARGS);
    assert_eq!(receipt.error_kind, Some(ErrorKind::CliArgs));
}

/// Test that error receipts include all standard fields
#[test]
fn test_error_receipts_include_standard_fields() {
    let (manager, _temp_dir) = create_test_manager();

    let error = XCheckerError::PacketOverflow {
        used_bytes: 100000,
        used_lines: 2000,
        limit_bytes: 65536,
        limit_lines: 1200,
    };

    let receipt = manager.create_error_receipt(
        "test-standard-fields",
        PhaseId::Requirements,
        &error,
        "0.1.0",                      // xchecker_version
        "0.8.1",                      // claude_cli_version
        "haiku", // model_full_name
        Some("sonnet".to_string()),   // model_alias
        HashMap::new(),               // flags
        PacketEvidence {
            files: vec![],
            max_bytes: 65536,
            max_lines: 1200,
        }, // packet
        Some("stderr output".to_string()), // stderr_tail
        None,                         // stderr_redacted
        vec!["warning1".to_string()], // warnings
        Some(false),                  // fallback_used
        "wsl",                        // runner
        None,                         // runner_distro
        None,                         // diff_context,
        None,                         // pipeline
    );

    // Verify all standard fields are present
    assert_eq!(receipt.schema_version, "1");
    assert_eq!(receipt.spec_id, "test-standard-fields");
    assert_eq!(receipt.phase, "requirements");
    assert_eq!(receipt.xchecker_version, "0.1.0");
    assert_eq!(receipt.claude_cli_version, "0.8.1");
    assert_eq!(receipt.model_full_name, "haiku");
    assert_eq!(receipt.model_alias, Some("sonnet".to_string()));
    assert_eq!(receipt.canonicalization_backend, "jcs-rfc8785");
    assert_eq!(receipt.runner, "wsl");
    assert_eq!(receipt.runner_distro, None); // We passed None, so expect None
    assert_eq!(receipt.exit_code, codes::PACKET_OVERFLOW);
    assert_eq!(receipt.error_kind, Some(ErrorKind::PacketOverflow));
    assert!(receipt.error_reason.is_some());
    assert_eq!(receipt.stderr_tail, Some("stderr output".to_string()));
    assert_eq!(receipt.warnings.len(), 1);
    assert_eq!(receipt.fallback_used, Some(false));
}

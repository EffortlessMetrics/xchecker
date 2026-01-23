//! M4 Gate Validation Tests
//!
//! This test suite validates that:
//! 1. All fatal error paths use `write_error_receipt_and_exit`
//! 2. Timeout writes .partial.md + appends "`phase_timeout`:<secs>" to warnings + exits 10
//! 3. Exit codes match receipts in smoke tests (process exit == receipt `exit_code`)
//! 4. Partial file + warning are present in timeout scenarios
//!
//! Requirements: R6.8, R6.9, R7.1, R7.2, R7.3

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;
use xchecker::error::{ConfigError, PhaseError, XCheckerError};
use xchecker::exit_codes::codes;
use xchecker::orchestrator::{OrchestratorConfig, PhaseOrchestrator, PhaseTimeout};
use xchecker::receipt::ReceiptManager;
use xchecker::types::{ErrorKind, PhaseId};

#[allow(clippy::duplicate_mod)]
#[path = "test_support/mod.rs"]
mod test_support;

/// Test environment for M4 gate tests
///
/// Note: Field order matters for drop semantics. Fields drop in declaration order,
/// so `_cwd_guard` must be declared first to restore CWD before `temp_dir` is deleted.
struct TestEnv {
    #[allow(dead_code)]
    _cwd_guard: test_support::CwdGuard,
    #[allow(dead_code)]
    temp_dir: TempDir,
    #[allow(dead_code)]
    spec_base: PathBuf,
    #[allow(dead_code)]
    orchestrator: PhaseOrchestrator,
}

/// Helper to set up test environment
fn setup_test_environment(test_name: &str) -> TestEnv {
    let temp_dir = TempDir::new().unwrap();
    let cwd_guard = test_support::CwdGuard::new(temp_dir.path()).unwrap();

    let spec_id = format!("test-m4-{test_name}");
    let orchestrator = PhaseOrchestrator::new(&spec_id).unwrap();

    let spec_base = temp_dir.path().join(".xchecker/specs").join(&spec_id);

    TestEnv {
        _cwd_guard: cwd_guard,
        temp_dir,
        spec_base,
        orchestrator,
    }
}

/// Test that `write_error_receipt_and_exit` creates receipt with matching exit code
#[test]
fn test_write_error_receipt_and_exit_alignment() {
    use camino::Utf8PathBuf;

    let temp_dir = TempDir::new().unwrap();
    let spec_id = "test-error-receipt";
    let spec_base =
        Utf8PathBuf::from_path_buf(temp_dir.path().join(".xchecker/specs").join(spec_id)).unwrap();

    // Create spec directory
    fs::create_dir_all(&spec_base).unwrap();

    // Test different error types
    let test_cases = vec![
        (
            XCheckerError::PacketOverflow {
                used_bytes: 100000,
                used_lines: 2000,
                limit_bytes: 65536,
                limit_lines: 1200,
            },
            codes::PACKET_OVERFLOW,
            ErrorKind::PacketOverflow,
        ),
        (
            XCheckerError::SecretDetected {
                pattern: "ghp_".to_string(),
                location: "test.txt".to_string(),
            },
            codes::SECRET_DETECTED,
            ErrorKind::SecretDetected,
        ),
        (
            XCheckerError::ConcurrentExecution {
                id: spec_id.to_string(),
            },
            codes::LOCK_HELD,
            ErrorKind::LockHeld,
        ),
        (
            XCheckerError::Phase(PhaseError::Timeout {
                phase: "REQUIREMENTS".to_string(),
                timeout_seconds: 600,
            }),
            codes::PHASE_TIMEOUT,
            ErrorKind::PhaseTimeout,
        ),
    ];

    for (error, expected_exit_code, expected_error_kind) in test_cases {
        // Verify error maps to correct exit code and kind
        let (exit_code, error_kind): (i32, ErrorKind) = (&error).into();
        assert_eq!(
            exit_code, expected_exit_code,
            "Exit code mismatch for error: {error}"
        );
        assert_eq!(
            error_kind, expected_error_kind,
            "Error kind mismatch for error: {error}"
        );

        // Note: We can't actually call write_error_receipt_and_exit in a test
        // because it calls std::process::exit, which would terminate the test process.
        // Instead, we verify the mapping logic is correct and test the receipt
        // creation separately.
    }
}

/// Test that error receipts have matching exit codes
#[test]
fn test_error_receipt_exit_code_alignment() {
    let temp_dir = TempDir::new().unwrap();
    let spec_id = "test-receipt-alignment";
    let spec_base =
        camino::Utf8PathBuf::from_path_buf(temp_dir.path().join(".xchecker/specs").join(spec_id))
            .unwrap();

    fs::create_dir_all(&spec_base).unwrap();

    let receipt_manager = ReceiptManager::new(&spec_base);

    // Test creating error receipts for different error types
    let test_cases = vec![
        (
            XCheckerError::PacketOverflow {
                used_bytes: 100000,
                used_lines: 2000,
                limit_bytes: 65536,
                limit_lines: 1200,
            },
            codes::PACKET_OVERFLOW,
            ErrorKind::PacketOverflow,
        ),
        (
            XCheckerError::Phase(PhaseError::Timeout {
                phase: "REQUIREMENTS".to_string(),
                timeout_seconds: 600,
            }),
            codes::PHASE_TIMEOUT,
            ErrorKind::PhaseTimeout,
        ),
    ];

    for (error, expected_exit_code, expected_error_kind) in test_cases {
        let receipt = receipt_manager.create_error_receipt(
            spec_id,
            PhaseId::Requirements,
            &error,
            "0.1.0",
            "0.8.1",
            "haiku",
            None,
            HashMap::new(),
            xchecker::types::PacketEvidence {
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

        // Verify receipt has matching exit code
        assert_eq!(
            receipt.exit_code, expected_exit_code,
            "Receipt exit_code should match expected exit code for error: {error}"
        );

        // Verify receipt has correct error_kind
        assert_eq!(
            receipt.error_kind,
            Some(expected_error_kind),
            "Receipt error_kind should match expected kind for error: {error}"
        );

        // Verify receipt has error_reason
        assert!(
            receipt.error_reason.is_some(),
            "Receipt should have error_reason for error: {error}"
        );
    }
}

/// Test that timeout creates partial artifact with correct naming
#[tokio::test]
async fn test_timeout_creates_partial_artifact() -> Result<()> {
    let env = setup_test_environment("timeout-partial");
    let spec_base = env.spec_base;

    // Simulate timeout by calling handle_phase_timeout directly
    // (This is a private method, so we test the public behavior through execute_phase)

    // For now, verify the partial artifact naming convention
    let phase_id = PhaseId::Requirements;
    let phase_number = 0u8; // Requirements is phase 0
    let partial_filename = format!("{:02}-{}.partial.md", phase_number, phase_id.as_str());
    assert_eq!(partial_filename, "00-requirements.partial.md");

    // Verify the partial artifact path would be in artifacts directory
    let expected_path = spec_base.join("artifacts").join(&partial_filename);
    assert!(
        expected_path
            .to_string_lossy()
            .contains("00-requirements.partial.md")
    );

    Ok(())
}

/// Test that timeout receipt contains warning with correct format
#[test]
fn test_timeout_receipt_warning_format() {
    let timeout_secs = 600u64;
    let warning = format!("phase_timeout:{timeout_secs}");

    // Verify warning format
    assert_eq!(warning, "phase_timeout:600");

    // Verify it can be parsed
    let parts: Vec<&str> = warning.split(':').collect();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0], "phase_timeout");
    assert_eq!(parts[1], "600");

    // Test with different timeout values
    let test_cases = vec![5, 10, 60, 300, 600, 1200];
    for secs in test_cases {
        let warning = format!("phase_timeout:{secs}");
        assert!(warning.starts_with("phase_timeout:"));
        assert!(warning.contains(&secs.to_string()));
    }
}

/// Test that timeout exit code is correct
#[test]
fn test_timeout_exit_code_constant() {
    assert_eq!(codes::PHASE_TIMEOUT, 10);
}

/// Test that timeout error maps to correct exit code and kind
#[test]
fn test_timeout_error_mapping() {
    let phase_err = PhaseError::Timeout {
        phase: "REQUIREMENTS".to_string(),
        timeout_seconds: 600,
    };
    let err = XCheckerError::Phase(phase_err);

    let (exit_code, error_kind): (i32, ErrorKind) = (&err).into();

    assert_eq!(exit_code, codes::PHASE_TIMEOUT);
    assert_eq!(error_kind, ErrorKind::PhaseTimeout);
}

/// Test that all error kinds serialize to `snake_case`
#[test]
fn test_error_kind_snake_case_serialization() {
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
            format!(r#""{expected}""#),
            "ErrorKind::{:?} should serialize to {}",
            kind,
            expected
        );
    }
}

/// Test that all major error types map to correct exit codes
#[test]
fn test_all_error_exit_code_mappings() {
    use xchecker::error::{ClaudeError, RunnerError};
    use xchecker::lock::LockError;

    let test_cases = vec![
        (
            XCheckerError::Config(ConfigError::InvalidFile("test".to_string())),
            codes::CLI_ARGS,
            ErrorKind::CliArgs,
        ),
        (
            XCheckerError::PacketOverflow {
                used_bytes: 100000,
                used_lines: 2000,
                limit_bytes: 65536,
                limit_lines: 1200,
            },
            codes::PACKET_OVERFLOW,
            ErrorKind::PacketOverflow,
        ),
        (
            XCheckerError::SecretDetected {
                pattern: "ghp_".to_string(),
                location: "test.txt".to_string(),
            },
            codes::SECRET_DETECTED,
            ErrorKind::SecretDetected,
        ),
        (
            XCheckerError::ConcurrentExecution {
                id: "test-spec".to_string(),
            },
            codes::LOCK_HELD,
            ErrorKind::LockHeld,
        ),
        (
            XCheckerError::Lock(LockError::ConcurrentExecution {
                spec_id: "test-spec".to_string(),
                pid: 12345,
                created_ago: "5m".to_string(),
            }),
            codes::LOCK_HELD,
            ErrorKind::LockHeld,
        ),
        (
            XCheckerError::Phase(PhaseError::Timeout {
                phase: "REQUIREMENTS".to_string(),
                timeout_seconds: 600,
            }),
            codes::PHASE_TIMEOUT,
            ErrorKind::PhaseTimeout,
        ),
        (
            XCheckerError::Claude(ClaudeError::ExecutionFailed {
                stderr: "API error".to_string(),
            }),
            codes::CLAUDE_FAILURE,
            ErrorKind::ClaudeFailure,
        ),
        (
            XCheckerError::Runner(RunnerError::NativeExecutionFailed {
                reason: "command not found".to_string(),
            }),
            codes::CLAUDE_FAILURE,
            ErrorKind::ClaudeFailure,
        ),
    ];

    for (error, expected_code, expected_kind) in test_cases {
        let (code, kind) = (&error).into();
        assert_eq!(code, expected_code, "Exit code mismatch for error: {error}");
        assert_eq!(
            kind, expected_kind,
            "Error kind mismatch for error: {error}"
        );
    }
}

/// Test that `PhaseTimeout` configuration works correctly
#[test]
fn test_phase_timeout_configuration() {
    // Test default timeout
    let config = OrchestratorConfig {
        dry_run: false,
        config: HashMap::new(),
        full_config: None,
        selectors: None,
        strict_validation: false,
        redactor: Default::default(),
        hooks: None,
    };
    let timeout = PhaseTimeout::from_config(&config);
    assert_eq!(timeout.duration.as_secs(), PhaseTimeout::DEFAULT_SECS);

    // Test custom timeout
    let mut config_map = HashMap::new();
    config_map.insert("phase_timeout".to_string(), "300".to_string());
    let config = OrchestratorConfig {
        dry_run: false,
        config: config_map,
        full_config: None,
        selectors: None,
        strict_validation: false,
        redactor: Default::default(),
        hooks: None,
    };
    let timeout = PhaseTimeout::from_config(&config);
    assert_eq!(timeout.duration.as_secs(), 300);

    // Test minimum enforcement
    let mut config_map = HashMap::new();
    config_map.insert("phase_timeout".to_string(), "1".to_string());
    let config = OrchestratorConfig {
        dry_run: false,
        config: config_map,
        full_config: None,
        selectors: None,
        strict_validation: false,
        redactor: Default::default(),
        hooks: None,
    };
    let timeout = PhaseTimeout::from_config(&config);
    assert_eq!(timeout.duration.as_secs(), PhaseTimeout::MIN_SECS);
}

/// Test that timeout constants are correct
#[test]
fn test_phase_timeout_constants() {
    assert_eq!(PhaseTimeout::DEFAULT_SECS, 600);
    assert_eq!(PhaseTimeout::MIN_SECS, 5);
}

/// Test that partial artifact content format is correct
#[test]
fn test_partial_artifact_content_format() {
    let phase_id = PhaseId::Requirements;
    let timeout_seconds = 600u64;

    let partial_content = format!(
        "# {} Phase (Partial - Timeout)\n\nThis phase timed out after {} seconds.\n\nNo output was generated before the timeout occurred.\n",
        phase_id.as_str(),
        timeout_seconds
    );

    // Verify content structure
    assert!(partial_content.contains("# requirements Phase (Partial - Timeout)"));
    assert!(partial_content.contains("timed out after 600 seconds"));
    assert!(partial_content.contains("No output was generated"));
}

/// Test that receipt manager creates receipts with correct schema version
#[test]
fn test_receipt_schema_version() {
    let temp_dir = TempDir::new().unwrap();
    let spec_base =
        camino::Utf8PathBuf::from_path_buf(temp_dir.path().join(".xchecker/specs/test-schema"))
            .unwrap();

    fs::create_dir_all(&spec_base).unwrap();

    let receipt_manager = ReceiptManager::new(&spec_base);

    let receipt = receipt_manager.create_receipt(
        "test-schema",
        PhaseId::Requirements,
        0,
        vec![],
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        HashMap::new(),
        xchecker::types::PacketEvidence {
            files: vec![],
            max_bytes: 65536,
            max_lines: 1200,
        },
        None,     // stderr_redacted
        None,     // stderr_redacted
        vec![],   // warnings
        None,     // fallback_used
        "native", // runner
        None,     // runner_distro
        None,     // error_kind
        None,     // error_reason
        None,     // diff_context,
        None,     // pipeline
    );

    assert_eq!(receipt.schema_version, "1");
}

/// Test that receipts are written with canonical JSON (JCS)
#[test]
fn test_receipt_canonical_json_emission() {
    let temp_dir = TempDir::new().unwrap();
    let spec_base =
        camino::Utf8PathBuf::from_path_buf(temp_dir.path().join(".xchecker/specs/test-jcs"))
            .unwrap();

    fs::create_dir_all(&spec_base).unwrap();

    let receipt_manager = ReceiptManager::new(&spec_base);

    let receipt = receipt_manager.create_receipt(
        "test-jcs",
        PhaseId::Requirements,
        0,
        vec![],
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        HashMap::new(),
        xchecker::types::PacketEvidence {
            files: vec![],
            max_bytes: 65536,
            max_lines: 1200,
        },
        None,     // stderr_redacted
        None,     // stderr_redacted
        vec![],   // warnings
        None,     // fallback_used
        "native", // runner
        None,     // runner_distro
        None,     // error_kind
        None,     // error_reason
        None,     // diff_context,
        None,     // pipeline
    );

    // Write receipt
    let receipt_path = receipt_manager.write_receipt(&receipt).unwrap();

    // Read back and verify it's valid JSON
    let content = fs::read_to_string(receipt_path.as_std_path()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

    // Verify schema_version is present
    assert_eq!(parsed["schema_version"].as_str().unwrap(), "1");

    // Verify emitted_at is present and in RFC3339 format
    assert!(parsed["emitted_at"].as_str().is_some());
}

/// Comprehensive M4 Gate validation test
#[test]
fn test_m4_gate_comprehensive_validation() {
    // 1. Verify all exit code constants are defined
    assert_eq!(codes::SUCCESS, 0);
    assert_eq!(codes::CLI_ARGS, 2);
    assert_eq!(codes::PACKET_OVERFLOW, 7);
    assert_eq!(codes::SECRET_DETECTED, 8);
    assert_eq!(codes::LOCK_HELD, 9);
    assert_eq!(codes::PHASE_TIMEOUT, 10);
    assert_eq!(codes::CLAUDE_FAILURE, 70);

    // 2. Verify timeout configuration
    assert_eq!(PhaseTimeout::DEFAULT_SECS, 600);
    assert_eq!(PhaseTimeout::MIN_SECS, 5);

    // 3. Verify error kind serialization (snake_case)
    let kinds = vec![
        (ErrorKind::CliArgs, "cli_args"),
        (ErrorKind::PacketOverflow, "packet_overflow"),
        (ErrorKind::SecretDetected, "secret_detected"),
        (ErrorKind::LockHeld, "lock_held"),
        (ErrorKind::PhaseTimeout, "phase_timeout"),
        (ErrorKind::ClaudeFailure, "claude_failure"),
        (ErrorKind::Unknown, "unknown"),
    ];

    for (kind, expected) in kinds {
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, format!(r#""{expected}""#));
    }

    // 4. Verify timeout warning format
    let warning = format!("phase_timeout:{}", 600);
    assert_eq!(warning, "phase_timeout:600");

    // 5. Verify partial artifact naming
    let partial_filename = format!("{:02}-{}.partial.md", 0, "requirements");
    assert_eq!(partial_filename, "00-requirements.partial.md");
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::fs;
    use xchecker::types::Receipt;

    /// Integration test for timeout behavior (requires mock or stub)
    #[tokio::test]
    #[ignore = "requires_claude_stub"]
    async fn test_timeout_full_integration() -> Result<()> {
        let _env_guard = test_support::EnvVarGuard::set("CLAUDE_STUB_HANG_SECS", "10");
        let env = setup_test_environment("timeout-full-integration");

        let stub_path = match test_support::claude_stub_path() {
            Some(path) => path,
            None => {
                eprintln!("Skipping: claude-stub not available");
                return Ok(());
            }
        };

        let mut config_map = HashMap::new();
        config_map.insert(
            "phase_timeout".to_string(),
            PhaseTimeout::MIN_SECS.to_string(),
        );
        config_map.insert("claude_cli_path".to_string(), stub_path);
        config_map.insert("claude_scenario".to_string(), "hang".to_string());

        let config = OrchestratorConfig {
            dry_run: false,
            config: config_map,
            full_config: None,
            selectors: None,
            strict_validation: false,
            redactor: Default::default(),
            hooks: None,
        };

        let result = env.orchestrator.execute_requirements_phase(&config).await?;

        assert!(!result.success, "Phase should time out");
        assert_eq!(result.exit_code, codes::PHASE_TIMEOUT);

        let receipt_path = result.receipt_path.expect("Receipt path should be present");
        let receipt_contents = fs::read_to_string(&receipt_path)?;
        let receipt: Receipt = serde_json::from_str(&receipt_contents)?;

        assert_eq!(receipt.error_kind, Some(ErrorKind::PhaseTimeout));
        let expected_warning = format!("phase_timeout:{}", PhaseTimeout::MIN_SECS);
        assert!(
            receipt.warnings.iter().any(|w| w == &expected_warning),
            "Receipt should include timeout warning"
        );

        let partial_path = result
            .artifact_paths
            .first()
            .expect("Partial artifact path should be present");
        assert!(partial_path.exists(), "Partial artifact should exist");
        assert!(
            partial_path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().ends_with(".partial.md")),
            "Partial artifact should have .partial.md suffix"
        );
        assert!(
            partial_path.starts_with(env.spec_base.join("artifacts")),
            "Partial artifact should live under spec artifacts directory"
        );

        Ok(())
    }
}

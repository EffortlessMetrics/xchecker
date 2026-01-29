#![cfg(feature = "test-utils")]
//! Smoke tests for exit code and receipt alignment
//!
//! These tests deliberately trigger each major error type and verify that:
//! 1. The process exit code matches the receipt exit_code field
//! 2. The error_kind is set correctly in the receipt

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;
use xchecker::test_support;

/// Helper to run xchecker command and capture exit code
fn run_xchecker_command(args: &[&str], temp_dir: &TempDir) -> (i32, PathBuf) {
    let output = Command::new(env!("CARGO_BIN_EXE_xchecker"))
        .args(args)
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to execute xchecker");

    let exit_code = output.status.code().unwrap_or(1);
    let spec_base = temp_dir.path().join(".xchecker/specs");

    (exit_code, spec_base)
}

/// Helper to read the latest receipt for a spec
fn read_latest_receipt(spec_base: &Path, spec_id: &str) -> Option<Value> {
    let receipts_dir = spec_base.join(spec_id).join("receipts");

    if !receipts_dir.exists() {
        return None;
    }

    // Find all receipt files
    let mut receipt_files: Vec<_> = fs::read_dir(&receipts_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s == "json")
                .unwrap_or(false)
        })
        .collect();

    if receipt_files.is_empty() {
        return None;
    }

    // Sort by filename to get the latest
    receipt_files.sort_by_key(|e| e.file_name());
    let latest = receipt_files.last()?;

    // Read and parse the receipt
    let content = fs::read_to_string(latest.path()).ok()?;
    serde_json::from_str(&content).ok()
}

#[test]
#[ignore = "requires_xchecker_binary"]
fn test_cli_args_error_alignment_requires_xchecker_binary() {
    let temp_dir = TempDir::new().unwrap();

    // Trigger CLI args error by providing invalid spec ID
    let (exit_code, _spec_base) = run_xchecker_command(&["spec", "", "--dry-run"], &temp_dir);

    // Should exit with CLI_ARGS code (2)
    assert_eq!(exit_code, 2, "Expected CLI_ARGS exit code");

    // Note: For CLI args errors, we may not have a receipt since spec_id is invalid
    // This is acceptable as the error occurs before spec initialization
}

#[test]
#[ignore = "requires_xchecker_binary"]
fn test_packet_overflow_error_alignment_requires_xchecker_binary() {
    let temp_dir = TempDir::new().unwrap();

    // Create a large file that will cause packet overflow
    let large_file = temp_dir.path().join("large_file.txt");
    let large_content = "x".repeat(100_000); // 100KB of content
    fs::write(&large_file, large_content).unwrap();

    // Trigger packet overflow with very small limits
    let (exit_code, spec_base) = run_xchecker_command(
        &[
            "spec",
            "test-overflow",
            "--source",
            "fs",
            "--repo",
            temp_dir.path().to_str().unwrap(),
            "--packet-max-bytes",
            "1000",
            "--dry-run",
        ],
        &temp_dir,
    );

    // Should exit with PACKET_OVERFLOW code (7)
    assert_eq!(exit_code, 7, "Expected PACKET_OVERFLOW exit code");

    // Read receipt and verify alignment
    if let Some(receipt) = read_latest_receipt(&spec_base, "test-overflow") {
        assert_eq!(
            receipt["exit_code"].as_i64().unwrap(),
            7,
            "Receipt exit_code should match process exit code"
        );
        assert_eq!(
            receipt["error_kind"].as_str().unwrap(),
            "packet_overflow",
            "Receipt should have packet_overflow error_kind"
        );
        assert!(
            receipt["error_reason"].as_str().is_some(),
            "Receipt should have error_reason"
        );
    }
}

#[test]
#[ignore = "requires_xchecker_binary"]
fn test_lock_held_error_alignment_requires_xchecker_binary() {
    let temp_dir = TempDir::new().unwrap();

    // Create spec directory and lock file manually
    let spec_dir = temp_dir.path().join(".xchecker/specs/test-lock");
    fs::create_dir_all(&spec_dir).unwrap();

    let lock_file = spec_dir.join(".lock");
    fs::write(
        &lock_file,
        format!(
            r#"{{"pid": 99999, "created_at": "{}", "hostname": "test"}}"#,
            chrono::Utc::now().to_rfc3339()
        ),
    )
    .unwrap();

    // Try to run spec command (should fail with lock held)
    let (exit_code, spec_base) =
        run_xchecker_command(&["spec", "test-lock", "--dry-run"], &temp_dir);

    // Should exit with LOCK_HELD code (9)
    assert_eq!(exit_code, 9, "Expected LOCK_HELD exit code");

    // Read receipt and verify alignment
    if let Some(receipt) = read_latest_receipt(&spec_base, "test-lock") {
        assert_eq!(
            receipt["exit_code"].as_i64().unwrap(),
            9,
            "Receipt exit_code should match process exit code"
        );
        assert_eq!(
            receipt["error_kind"].as_str().unwrap(),
            "lock_held",
            "Receipt should have lock_held error_kind"
        );
        assert!(
            receipt["error_reason"].as_str().is_some(),
            "Receipt should have error_reason"
        );
    }
}

#[test]
#[ignore = "requires_real_claude"]
fn test_claude_failure_error_alignment_requires_real_claude() {
    if !test_support::llm_tests_enabled() {
        println!(
            "Skipping real LLM exit alignment test. Set XCHECKER_REAL_LLM_TESTS=1 to enable (and ensure XCHECKER_SKIP_LLM_TESTS is unset)."
        );
        return;
    }

    let temp_dir = TempDir::new().unwrap();

    // Trigger Claude failure by using invalid model
    let (exit_code, spec_base) = run_xchecker_command(
        &[
            "spec",
            "test-claude-fail",
            "--model",
            "invalid-model-name-that-does-not-exist",
            "--dry-run",
        ],
        &temp_dir,
    );

    // Should exit with CLAUDE_FAILURE code (70)
    assert_eq!(exit_code, 70, "Expected CLAUDE_FAILURE exit code");

    // Read receipt and verify alignment
    if let Some(receipt) = read_latest_receipt(&spec_base, "test-claude-fail") {
        assert_eq!(
            receipt["exit_code"].as_i64().unwrap(),
            70,
            "Receipt exit_code should match process exit code"
        );
        assert_eq!(
            receipt["error_kind"].as_str().unwrap(),
            "claude_failure",
            "Receipt should have claude_failure error_kind"
        );
        assert!(
            receipt["error_reason"].as_str().is_some(),
            "Receipt should have error_reason"
        );
    }
}

#[test]
fn test_error_kind_serialization() {
    // Test that ErrorKind serializes to snake_case as expected
    use xchecker::types::ErrorKind;

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
        assert_eq!(json, format!(r#""{}""#, expected));
    }
}

#[test]
fn test_exit_code_mapping() {
    // Test that XCheckerError maps to correct exit codes
    use xchecker::error::{ConfigError, PhaseError, XCheckerError};
    use xchecker::exit_codes::codes;

    // Test CLI args error
    let err = XCheckerError::Config(ConfigError::InvalidFile("test".to_string()));
    let (code, kind) = (&err).into();
    assert_eq!(code, codes::CLI_ARGS);
    assert_eq!(kind, xchecker::types::ErrorKind::CliArgs);

    // Test packet overflow error
    let err = XCheckerError::PacketOverflow {
        used_bytes: 100000,
        used_lines: 2000,
        limit_bytes: 65536,
        limit_lines: 1200,
    };
    let (code, kind) = (&err).into();
    assert_eq!(code, codes::PACKET_OVERFLOW);
    assert_eq!(kind, xchecker::types::ErrorKind::PacketOverflow);

    // Test secret detected error
    let err = XCheckerError::SecretDetected {
        pattern: "ghp_".to_string(),
        location: "test.txt".to_string(),
    };
    let (code, kind) = (&err).into();
    assert_eq!(code, codes::SECRET_DETECTED);
    assert_eq!(kind, xchecker::types::ErrorKind::SecretDetected);

    // Test concurrent execution error
    let err = XCheckerError::ConcurrentExecution {
        id: "test-spec".to_string(),
    };
    let (code, kind) = (&err).into();
    assert_eq!(code, codes::LOCK_HELD);
    assert_eq!(kind, xchecker::types::ErrorKind::LockHeld);

    // Test phase timeout error
    let phase_err = PhaseError::Timeout {
        phase: "REQUIREMENTS".to_string(),
        timeout_seconds: 600,
    };
    let err = XCheckerError::Phase(phase_err);
    let (code, kind) = (&err).into();
    assert_eq!(code, codes::PHASE_TIMEOUT);
    assert_eq!(kind, xchecker::types::ErrorKind::PhaseTimeout);
}

#[test]
fn test_receipt_exit_code_alignment() {
    // Test that receipts created with error information have matching exit codes
    use xchecker::error::{PhaseError, XCheckerError};
    use xchecker::exit_codes::codes;
    use xchecker::types::ErrorKind;

    // Test that error_to_exit_code_and_kind produces consistent results
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
                id: "test-spec".to_string(),
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

    for (error, expected_code, expected_kind) in test_cases {
        let (code, kind) = (&error).into();
        assert_eq!(
            code, expected_code,
            "Exit code mismatch for error: {}",
            error
        );
        assert_eq!(
            kind, expected_kind,
            "Error kind mismatch for error: {}",
            error
        );
    }
}

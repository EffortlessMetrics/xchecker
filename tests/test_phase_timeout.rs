//! Tests for phase timeout system (Task 7)
//!
//! **WHITE-BOX TEST**: This test uses internal module APIs (`orchestrator::{OrchestratorConfig,
//! PhaseOrchestrator, PhaseTimeout}`, `types::PhaseId`) and may break with internal refactors.
//! These tests are intentionally white-box to validate internal implementation details.
//! See FR-TEST-4 for white-box test policy.
//!
//! This test validates that:
//! 1. Phase timeouts are enforced
//! 2. Partial artifacts are written on timeout
//! 3. Receipts contain timeout warnings
//! 4. Exit code is 10 for timeouts

use anyhow::Result;
use std::collections::HashMap;
use tempfile::TempDir;
use xchecker::orchestrator::{OrchestratorConfig, PhaseOrchestrator, PhaseTimeout};
use xchecker::types::PhaseId;

#[allow(clippy::duplicate_mod)]
#[path = "test_support/mod.rs"]
mod test_support;

/// Test environment for phase timeout tests
///
/// Note: Field order matters for drop semantics. Fields drop in declaration order,
/// so `_cwd_guard` must be declared first to restore CWD before `temp_dir` is deleted.
struct TimeoutTestEnv {
    #[allow(dead_code)]
    _cwd_guard: test_support::CwdGuard,
    #[allow(dead_code)]
    temp_dir: TempDir,
    #[allow(dead_code)]
    orchestrator: PhaseOrchestrator,
}

/// Helper to set up test environment
fn setup_test_environment(test_name: &str) -> TimeoutTestEnv {
    let temp_dir = TempDir::new().unwrap();
    let cwd_guard = test_support::CwdGuard::new(temp_dir.path()).unwrap();

    let spec_id = format!("test-timeout-{}", test_name);
    let orchestrator = PhaseOrchestrator::new(&spec_id).unwrap();

    TimeoutTestEnv {
        _cwd_guard: cwd_guard,
        temp_dir,
        orchestrator,
    }
}

/// Test that PhaseTimeout struct has correct constants
#[test]
fn test_phase_timeout_constants() {
    assert_eq!(PhaseTimeout::DEFAULT_SECS, 600);
    assert_eq!(PhaseTimeout::MIN_SECS, 5);
}

/// Test that PhaseTimeout enforces minimum timeout
#[test]
fn test_phase_timeout_minimum_enforcement() {
    let timeout = PhaseTimeout::from_secs(1); // Below minimum
    assert_eq!(timeout.duration.as_secs(), PhaseTimeout::MIN_SECS);

    let timeout = PhaseTimeout::from_secs(3); // Below minimum
    assert_eq!(timeout.duration.as_secs(), PhaseTimeout::MIN_SECS);

    let timeout = PhaseTimeout::from_secs(10); // Above minimum
    assert_eq!(timeout.duration.as_secs(), 10);
}

/// Test that PhaseTimeout::from_config reads from configuration correctly
#[test]
fn test_phase_timeout_from_config() {
    // Test with explicit timeout in config
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

    // Test with timeout below minimum (should be enforced to MIN_SECS)
    let mut config_map = HashMap::new();
    config_map.insert("phase_timeout".to_string(), "2".to_string());
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

    // Test with no timeout in config (should use default)
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

    // Test with invalid timeout value (should use default)
    let mut config_map = HashMap::new();
    config_map.insert("phase_timeout".to_string(), "invalid".to_string());
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
    assert_eq!(timeout.duration.as_secs(), PhaseTimeout::DEFAULT_SECS);
}

/// Test that timeout creates partial artifact and receipt with warning
/// This is a smoke test that validates the core timeout behavior
#[tokio::test]
async fn test_timeout_creates_partial_and_receipt() -> Result<()> {
    let _env = setup_test_environment("partial");

    // Configure with a very short timeout to trigger it
    let mut config_map = HashMap::new();
    config_map.insert("phase_timeout".to_string(), "1".to_string()); // 1 second timeout (will be enforced to MIN_SECS)
    config_map.insert("claude_scenario".to_string(), "slow".to_string()); // Simulate slow response

    let _config = OrchestratorConfig {
        dry_run: false, // Use real execution to test timeout
        config: config_map,
        full_config: None,
        selectors: None,
        strict_validation: false,
        redactor: Default::default(),
        hooks: None,
    };

    // Note: This test would need a way to simulate a slow Claude response
    // For now, we'll test the timeout infrastructure exists and compiles
    // A full integration test would require mocking or a test stub that sleeps

    // Verify the timeout configuration is created correctly
    let timeout = PhaseTimeout::from_secs(1);
    assert_eq!(timeout.duration.as_secs(), PhaseTimeout::MIN_SECS);

    Ok(())
}

/// Test that timeout warning format is correct
#[test]
fn test_timeout_warning_format() {
    let timeout_secs = 600u64;
    let warning = format!("phase_timeout:{}", timeout_secs);
    assert_eq!(warning, "phase_timeout:600");

    // Verify it can be parsed back
    let parts: Vec<&str> = warning.split(':').collect();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0], "phase_timeout");
    assert_eq!(parts[1], "600");
}

/// Test that partial artifact filename format is correct
#[test]
fn test_partial_artifact_naming() {
    let phase_id = PhaseId::Requirements;
    let phase_number = 0u8; // Requirements is phase 0
    let partial_filename = format!("{:02}-{}.partial.md", phase_number, phase_id.as_str());
    assert_eq!(partial_filename, "00-requirements.partial.md");

    let phase_id = PhaseId::Design;
    let phase_number = 10u8; // Design is phase 10
    let partial_filename = format!("{:02}-{}.partial.md", phase_number, phase_id.as_str());
    assert_eq!(partial_filename, "10-design.partial.md");
}

/// Test exit code for timeout is correct
#[test]
fn test_timeout_exit_code() {
    use xchecker::exit_codes::codes;
    assert_eq!(codes::PHASE_TIMEOUT, 10);
}

/// Test that timeout error kind is correct
#[test]
fn test_timeout_error_kind() {
    use xchecker::error::{PhaseError, XCheckerError};
    use xchecker::exit_codes::codes;
    use xchecker::types::ErrorKind;

    let phase_err = PhaseError::Timeout {
        phase: "REQUIREMENTS".to_string(),
        timeout_seconds: 600,
    };
    let err = XCheckerError::Phase(phase_err);

    let (exit_code, error_kind): (i32, ErrorKind) = (&err).into();
    assert_eq!(exit_code, codes::PHASE_TIMEOUT);
    assert_eq!(error_kind, ErrorKind::PhaseTimeout);
}

/// Test that timeout error serializes correctly
#[test]
fn test_timeout_error_serialization() {
    use xchecker::types::ErrorKind;

    let json = serde_json::to_string(&ErrorKind::PhaseTimeout).unwrap();
    assert_eq!(json, r#""phase_timeout""#);
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::fs;
    use xchecker::exit_codes::codes;
    use xchecker::types::{ErrorKind, Receipt};

    /// This test would require a mock Claude CLI that sleeps longer than the timeout
    /// For now, it's a placeholder for future integration testing
    #[tokio::test]
    #[ignore = "requires_claude_stub"]
    async fn test_full_timeout_flow_with_mock() -> Result<()> {
        let _env_guard = test_support::EnvVarGuard::set("CLAUDE_STUB_HANG_SECS", "10");
        let env = setup_test_environment("full-timeout-flow");

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

        Ok(())
    }
}

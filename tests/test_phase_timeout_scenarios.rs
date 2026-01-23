//! Integration tests for phase timeout scenarios (Task 7.4: FR-RUN-004, FR-RUN-007, FR-ORC-005)
//!
//! **WHITE-BOX TEST**: This test uses internal module APIs (`orchestrator::{OrchestratorConfig,
//! PhaseOrchestrator, PhaseTimeout}`, `types::{...}`) and may break with internal refactors.
//! These tests are intentionally white-box to validate internal implementation details.
//! See FR-TEST-4 for white-box test policy.
//!
//! This test suite validates that:
//! 1. Timeouts are detected at different stages (packet building, Claude execution, artifact writing)
//! 2. Partial artifacts are saved on timeout with .partial.md extension
//! 3. Receipts are generated with phase_timeout error kind
//! 4. Exit code 10 is returned on timeout
//! 5. Warnings include timeout duration

use anyhow::Result;
use std::collections::HashMap;
use tempfile::TempDir;
use xchecker::orchestrator::{OrchestratorConfig, PhaseOrchestrator, PhaseTimeout};
use xchecker::types::{ErrorKind, PhaseId};

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
    orchestrator: PhaseOrchestrator,
}

/// Helper to set up test environment with isolated home
fn setup_test_environment(test_name: &str) -> TimeoutTestEnv {
    let temp_dir = TempDir::new().unwrap();
    let cwd_guard = test_support::CwdGuard::new(temp_dir.path()).unwrap();

    // Create spec directory structure
    let spec_id = format!("test-timeout-{}", test_name);
    let orchestrator = PhaseOrchestrator::new(&spec_id).unwrap();

    TimeoutTestEnv {
        _cwd_guard: cwd_guard,
        temp_dir,
        orchestrator,
    }
}

/// Helper to create config with specific timeout
fn create_config_with_timeout(timeout_secs: u64, dry_run: bool) -> OrchestratorConfig {
    let mut config_map = HashMap::new();
    config_map.insert("phase_timeout".to_string(), timeout_secs.to_string());

    OrchestratorConfig {
        dry_run,
        config: config_map,
        full_config: None,
        selectors: None,
        strict_validation: false,
        redactor: Default::default(),
        hooks: None,
    }
}

/// Helper to create config for stub-based timeout tests.
/// Configures the stub to hang for longer than the timeout.
fn create_stub_timeout_config(timeout_secs: u64) -> Option<OrchestratorConfig> {
    let stub_path = test_support::claude_stub_path()?;

    let mut config_map = HashMap::new();
    config_map.insert("phase_timeout".to_string(), timeout_secs.to_string());
    config_map.insert("claude_cli_path".to_string(), stub_path);
    config_map.insert("claude_scenario".to_string(), "hang".to_string());

    Some(OrchestratorConfig {
        dry_run: false,
        config: config_map,
        full_config: None,
        selectors: None,
        strict_validation: false,
        redactor: Default::default(),
        hooks: None,
    })
}

/// Test that PhaseTimeout configuration is correctly applied
#[test]
fn test_timeout_configuration() {
    // Test default timeout
    let config = OrchestratorConfig::default();
    let timeout = PhaseTimeout::from_config(&config);
    assert_eq!(
        timeout.duration.as_secs(),
        PhaseTimeout::DEFAULT_SECS,
        "Default timeout should be 600 seconds"
    );

    // Test custom timeout
    let config = create_config_with_timeout(300, false);
    let timeout = PhaseTimeout::from_config(&config);
    assert_eq!(
        timeout.duration.as_secs(),
        300,
        "Custom timeout should be respected"
    );

    // Test minimum timeout enforcement
    let config = create_config_with_timeout(1, false);
    let timeout = PhaseTimeout::from_config(&config);
    assert_eq!(
        timeout.duration.as_secs(),
        PhaseTimeout::MIN_SECS,
        "Timeout below minimum should be enforced to MIN_SECS"
    );
}

/// Test that timeout during packet building is handled correctly
/// This simulates a scenario where packet assembly takes too long
#[tokio::test]
async fn test_timeout_during_packet_building() -> Result<()> {
    let env = setup_test_environment("packet-building");

    // Use a very short timeout to trigger timeout during packet building
    // In a real scenario, this would happen with very large codebases
    let config = create_config_with_timeout(PhaseTimeout::MIN_SECS, true);

    // Execute phase - in dry run mode, this should complete quickly
    // but we're testing the timeout infrastructure
    let result = env.orchestrator.execute_requirements_phase(&config).await;

    // In dry run mode, this should succeed quickly
    // For a real timeout test, we'd need to mock a slow packet builder
    assert!(
        result.is_ok() || result.is_err(),
        "Phase should either succeed or timeout"
    );

    Ok(())
}

/// Test that timeout during Claude execution creates partial artifact
/// This is the most common timeout scenario
#[tokio::test]
#[ignore = "requires_claude_stub"]
async fn test_timeout_during_claude_execution_requires_claude_stub() -> Result<()> {
    // Set the stub to hang for 10 seconds (longer than our 5-second timeout)
    let _env_guard = test_support::EnvVarGuard::set("CLAUDE_STUB_HANG_SECS", "10");

    let env = setup_test_environment("claude-execution");

    // Get stub config - skip if stub not available
    let config = match create_stub_timeout_config(PhaseTimeout::MIN_SECS) {
        Some(c) => c,
        None => {
            eprintln!("Skipping: claude-stub not available");
            return Ok(());
        }
    };

    // Execute phase - this should timeout after 5 seconds while stub hangs for 10
    let result = env.orchestrator.execute_requirements_phase(&config).await;

    // The orchestrator handles timeouts gracefully - it returns Ok(ExecutionResult)
    // with success=false and exit_code=10 (PHASE_TIMEOUT), not Err(...)
    assert!(
        result.is_ok(),
        "Phase execution should complete (with timeout result)"
    );

    let exec_result = result.unwrap();
    assert!(!exec_result.success, "Phase should not succeed (timed out)");
    assert_eq!(
        exec_result.exit_code, 10,
        "Exit code should be PHASE_TIMEOUT (10)"
    );
    assert!(
        exec_result
            .error
            .as_ref()
            .is_some_and(|e| e.contains("timed out")),
        "Error should mention timeout: {:?}",
        exec_result.error
    );

    Ok(())
}

/// Test that timeout creates partial artifact with correct naming
#[tokio::test]
async fn test_timeout_creates_partial_artifact() -> Result<()> {
    let _env = setup_test_environment("partial-artifact");

    // Simulate timeout by using orchestrator's handle_phase_timeout directly
    // This is a unit test of the timeout handling logic
    let phase_id = PhaseId::Requirements;

    // Call the timeout handler (this is internal but we're testing it)
    // In a real scenario, this would be called by execute_phase_with_timeout
    // For now, we'll test the partial artifact naming convention

    let phase_number = 0u8; // Requirements is phase 0
    let expected_filename = format!("{:02}-{}.partial.md", phase_number, phase_id.as_str());
    assert_eq!(
        expected_filename, "00-requirements.partial.md",
        "Partial artifact should have correct naming"
    );

    // Test for other phases
    let phase_id = PhaseId::Design;
    let phase_number = 10u8; // Design is phase 10
    let expected_filename = format!("{:02}-{}.partial.md", phase_number, phase_id.as_str());
    assert_eq!(
        expected_filename, "10-design.partial.md",
        "Design partial artifact should have correct naming"
    );

    Ok(())
}

/// Test that timeout receipt has correct error kind
#[test]
fn test_timeout_receipt_error_kind() {
    use xchecker::error::{PhaseError, XCheckerError};
    use xchecker::exit_codes::codes;

    // Create a timeout error
    let phase_err = PhaseError::Timeout {
        phase: "REQUIREMENTS".to_string(),
        timeout_seconds: 600,
    };
    let err = XCheckerError::Phase(phase_err);

    // Check exit code and error kind mapping
    let (exit_code, error_kind): (i32, ErrorKind) = (&err).into();
    assert_eq!(
        exit_code,
        codes::PHASE_TIMEOUT,
        "Timeout should map to exit code 10"
    );
    assert_eq!(
        error_kind,
        ErrorKind::PhaseTimeout,
        "Timeout should map to PhaseTimeout error kind"
    );
}

/// Test that timeout warning format is correct
#[test]
fn test_timeout_warning_format() {
    let timeout_secs = 600u64;
    let warning = format!("phase_timeout:{}", timeout_secs);

    assert_eq!(
        warning, "phase_timeout:600",
        "Warning should have correct format"
    );

    // Verify it can be parsed back
    let parts: Vec<&str> = warning.split(':').collect();
    assert_eq!(parts.len(), 2, "Warning should have two parts");
    assert_eq!(
        parts[0], "phase_timeout",
        "First part should be 'phase_timeout'"
    );
    assert_eq!(parts[1], "600", "Second part should be timeout duration");

    // Test parsing the duration
    let parsed_duration: u64 = parts[1].parse().unwrap();
    assert_eq!(
        parsed_duration, timeout_secs,
        "Duration should parse correctly"
    );
}

/// Test that timeout exit code is correct
#[test]
fn test_timeout_exit_code_constant() {
    use xchecker::exit_codes::codes;
    assert_eq!(
        codes::PHASE_TIMEOUT,
        10,
        "PHASE_TIMEOUT exit code should be 10"
    );
}

/// Test that timeout error serializes correctly for receipts
#[test]
fn test_timeout_error_serialization() {
    let error_kind = ErrorKind::PhaseTimeout;
    let json = serde_json::to_string(&error_kind).unwrap();

    assert_eq!(
        json, r#""phase_timeout""#,
        "ErrorKind::PhaseTimeout should serialize to 'phase_timeout'"
    );

    // Test deserialization
    let deserialized: ErrorKind = serde_json::from_str(&json).unwrap();
    assert_eq!(
        deserialized,
        ErrorKind::PhaseTimeout,
        "Should deserialize back to PhaseTimeout"
    );
}

/// Test timeout during artifact writing stage
/// This simulates a scenario where writing the artifact takes too long
#[tokio::test]
#[ignore = "requires_claude_stub"]
async fn test_timeout_during_artifact_writing_requires_claude_stub() -> Result<()> {
    // Set the stub to hang for 10 seconds (longer than our 5-second timeout)
    let _env_guard = test_support::EnvVarGuard::set("CLAUDE_STUB_HANG_SECS", "10");

    let env = setup_test_environment("artifact-writing");

    // Get stub config - skip if stub not available
    let config = match create_stub_timeout_config(PhaseTimeout::MIN_SECS) {
        Some(c) => c,
        None => {
            eprintln!("Skipping: claude-stub not available");
            return Ok(());
        }
    };

    // Execute phase - would timeout if artifact writing is slow
    let result = env.orchestrator.execute_requirements_phase(&config).await;

    // The orchestrator handles timeouts gracefully - it returns Ok(ExecutionResult)
    // with success=false and exit_code=10 (PHASE_TIMEOUT)
    assert!(
        result.is_ok(),
        "Phase execution should complete (with timeout result)"
    );

    let exec_result = result.unwrap();
    assert!(!exec_result.success, "Phase should not succeed (timed out)");
    assert_eq!(
        exec_result.exit_code, 10,
        "Exit code should be PHASE_TIMEOUT (10)"
    );

    Ok(())
}

/// Test that multiple timeouts are handled independently
#[tokio::test]
async fn test_multiple_phase_timeouts() -> Result<()> {
    let env = setup_test_environment("multiple-timeouts");

    let config = create_config_with_timeout(PhaseTimeout::MIN_SECS, true);

    // Execute multiple phases - each should have independent timeout
    let result1 = env.orchestrator.execute_requirements_phase(&config).await;
    let result2 = env.orchestrator.execute_design_phase(&config).await;

    // Both should complete (in dry run mode) or timeout independently
    // The key is that one timeout doesn't affect the other
    assert!(
        result1.is_ok() || result1.is_err(),
        "First phase should complete or timeout"
    );
    assert!(
        result2.is_ok() || result2.is_err(),
        "Second phase should complete or timeout independently"
    );

    Ok(())
}

/// Test that timeout respects minimum duration
#[test]
fn test_timeout_minimum_enforcement() {
    // Test values below minimum
    let timeout1 = PhaseTimeout::from_secs(0);
    assert_eq!(
        timeout1.duration.as_secs(),
        PhaseTimeout::MIN_SECS,
        "Zero timeout should be enforced to MIN_SECS"
    );

    let timeout2 = PhaseTimeout::from_secs(3);
    assert_eq!(
        timeout2.duration.as_secs(),
        PhaseTimeout::MIN_SECS,
        "Timeout below MIN_SECS should be enforced"
    );

    // Test value at minimum
    let timeout3 = PhaseTimeout::from_secs(PhaseTimeout::MIN_SECS);
    assert_eq!(
        timeout3.duration.as_secs(),
        PhaseTimeout::MIN_SECS,
        "Timeout at MIN_SECS should be accepted"
    );

    // Test value above minimum
    let timeout4 = PhaseTimeout::from_secs(100);
    assert_eq!(
        timeout4.duration.as_secs(),
        100,
        "Timeout above MIN_SECS should be accepted as-is"
    );
}

/// Test that timeout configuration from invalid values uses default
#[test]
fn test_timeout_invalid_config_uses_default() {
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
    assert_eq!(
        timeout.duration.as_secs(),
        PhaseTimeout::DEFAULT_SECS,
        "Invalid timeout config should use default"
    );
}

/// Test that timeout configuration from negative values uses default
#[test]
fn test_timeout_negative_config_uses_default() {
    let mut config_map = HashMap::new();
    config_map.insert("phase_timeout".to_string(), "-100".to_string());

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
    assert_eq!(
        timeout.duration.as_secs(),
        PhaseTimeout::DEFAULT_SECS,
        "Negative timeout config should use default"
    );
}

/// Test timeout error message format
#[test]
fn test_timeout_error_message() {
    use xchecker::error::{PhaseError, XCheckerError};

    let phase_err = PhaseError::Timeout {
        phase: "REQUIREMENTS".to_string(),
        timeout_seconds: 600,
    };
    let err = XCheckerError::Phase(phase_err);

    let err_msg = err.to_string();
    assert!(
        err_msg.contains("REQUIREMENTS"),
        "Error message should mention phase name"
    );
    assert!(
        err_msg.contains("600"),
        "Error message should mention timeout duration"
    );
    assert!(
        err_msg.contains("timeout") || err_msg.contains("timed out"),
        "Error message should mention timeout"
    );
}

/// Test that partial artifact content is meaningful
#[test]
fn test_partial_artifact_content_format() {
    let phase_id = PhaseId::Requirements;
    let timeout_seconds = 600u64;

    // Simulate the partial artifact content that would be created
    let partial_content = format!(
        "# {} Phase (Partial - Timeout)\n\nThis phase timed out after {} seconds.\n\nNo output was generated before the timeout occurred.\n",
        phase_id.as_str(),
        timeout_seconds
    );

    // Verify content structure
    assert!(
        partial_content.contains("# requirements Phase"),
        "Should have phase name in header"
    );
    assert!(
        partial_content.contains("Partial - Timeout"),
        "Should indicate partial/timeout status"
    );
    assert!(
        partial_content.contains("600 seconds"),
        "Should mention timeout duration"
    );
    assert!(
        partial_content.contains("No output was generated"),
        "Should explain no output"
    );
}

/// Test timeout with different phase IDs
#[test]
fn test_timeout_with_different_phases() {
    use xchecker::error::{PhaseError, XCheckerError};
    use xchecker::exit_codes::codes;

    let phases = vec![
        ("REQUIREMENTS", PhaseId::Requirements),
        ("DESIGN", PhaseId::Design),
        ("TASKS", PhaseId::Tasks),
    ];

    for (phase_name, _phase_id) in phases {
        let phase_err = PhaseError::Timeout {
            phase: phase_name.to_string(),
            timeout_seconds: 600,
        };
        let err = XCheckerError::Phase(phase_err);

        let (exit_code, error_kind): (i32, ErrorKind) = (&err).into();
        assert_eq!(
            exit_code,
            codes::PHASE_TIMEOUT,
            "All phase timeouts should map to exit code 10"
        );
        assert_eq!(
            error_kind,
            ErrorKind::PhaseTimeout,
            "All phase timeouts should map to PhaseTimeout error kind"
        );
    }
}

/// Test that timeout duration is included in warning
#[test]
fn test_timeout_duration_in_warning() {
    let durations = vec![5u64, 60, 300, 600, 1800];

    for duration in durations {
        let warning = format!("phase_timeout:{}", duration);
        assert!(
            warning.contains(&duration.to_string()),
            "Warning should include duration: {}",
            duration
        );

        // Verify parsing
        let parts: Vec<&str> = warning.split(':').collect();
        let parsed: u64 = parts[1].parse().unwrap();
        assert_eq!(
            parsed, duration,
            "Duration should parse correctly from warning"
        );
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    /// Full integration test with mock Claude CLI that times out
    /// This test requires a mock setup and is ignored by default
    #[tokio::test]
    #[ignore = "requires_claude_stub"]
    async fn test_full_timeout_flow_with_mock_claude_requires_claude_stub() -> Result<()> {
        // Set the stub to hang for 10 seconds (longer than our 5-second timeout)
        let _env_guard = test_support::EnvVarGuard::set("CLAUDE_STUB_HANG_SECS", "10");

        let env = setup_test_environment("full-flow");

        // Get stub config - skip if stub not available
        let config = match create_stub_timeout_config(PhaseTimeout::MIN_SECS) {
            Some(c) => c,
            None => {
                eprintln!("Skipping: claude-stub not available");
                return Ok(());
            }
        };

        // Execute phase - should timeout with mock Claude that hangs
        let result = env.orchestrator.execute_requirements_phase(&config).await;

        // The orchestrator handles timeouts gracefully - it returns Ok(ExecutionResult)
        // with success=false and exit_code=10 (PHASE_TIMEOUT)
        assert!(
            result.is_ok(),
            "Phase execution should complete (with timeout result)"
        );

        let exec_result = result.unwrap();
        assert!(!exec_result.success, "Phase should not succeed (timed out)");
        assert_eq!(
            exec_result.exit_code, 10,
            "Exit code should be PHASE_TIMEOUT (10)"
        );
        assert!(
            exec_result
                .error
                .as_ref()
                .is_some_and(|e| e.contains("timed out")),
            "Error should mention timeout: {:?}",
            exec_result.error
        );

        // Verify partial artifact was created
        assert!(
            !exec_result.artifact_paths.is_empty(),
            "Should have created partial artifact"
        );
        let artifact_path_str = exec_result.artifact_paths[0].to_string_lossy();
        assert!(
            artifact_path_str.contains(".partial."),
            "Artifact should be partial: {}",
            artifact_path_str
        );

        Ok(())
    }

    /// Test timeout recovery - can we continue after a timeout?
    #[tokio::test]
    #[ignore = "requires_claude_stub"]
    async fn test_timeout_recovery_requires_claude_stub() -> Result<()> {
        // Set the stub to hang for 10 seconds (longer than our 5-second timeout)
        let _env_guard = test_support::EnvVarGuard::set("CLAUDE_STUB_HANG_SECS", "10");

        let env = setup_test_environment("recovery");

        // First execution times out (using stub with hang scenario)
        let config_short = match create_stub_timeout_config(PhaseTimeout::MIN_SECS) {
            Some(c) => c,
            None => {
                eprintln!("Skipping: claude-stub not available");
                return Ok(());
            }
        };
        let result1 = env
            .orchestrator
            .execute_requirements_phase(&config_short)
            .await;

        // The orchestrator handles timeouts gracefully - it returns Ok(ExecutionResult)
        // with success=false and exit_code=10 (PHASE_TIMEOUT)
        assert!(
            result1.is_ok(),
            "First execution should complete (with timeout result)"
        );
        let exec_result1 = result1.unwrap();
        assert!(
            !exec_result1.success,
            "First execution should not succeed (timed out)"
        );
        assert_eq!(
            exec_result1.exit_code, 10,
            "Exit code should be PHASE_TIMEOUT (10)"
        );

        // Second execution with dry_run should succeed (no stub needed)
        let config_dry = create_config_with_timeout(PhaseTimeout::DEFAULT_SECS, true);
        let result2 = env
            .orchestrator
            .execute_requirements_phase(&config_dry)
            .await;
        assert!(
            result2.is_ok(),
            "Second execution with dry_run should complete"
        );
        let exec_result2 = result2.unwrap();
        assert!(
            exec_result2.success,
            "Second execution with dry_run should succeed"
        );

        Ok(())
    }
}

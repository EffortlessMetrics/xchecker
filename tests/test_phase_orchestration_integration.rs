//! Integration tests for phase orchestration (FR-ORC-001, FR-ORC-002)
//!
//! Tests the complete phase execution flow with transition validation.
//!
//! **White-box testing approach**: These tests directly use `PhaseOrchestrator`
//! to validate internal phase orchestration behavior. This is intentional and
//! appropriate for testing internal logic. Production code should use `OrchestratorHandle`.

use anyhow::Result;
use tempfile::TempDir;
use xchecker::orchestrator::{OrchestratorConfig, PhaseOrchestrator};
use xchecker::types::PhaseId;

/// Helper to set up test environment
fn setup_test_environment(test_name: &str) -> (PhaseOrchestrator, TempDir) {
    // Use isolated home for each test to avoid conflicts
    let temp_dir = xchecker::paths::with_isolated_home();

    // Create spec directory structure
    let spec_id = format!("test-integration-{test_name}");
    let orchestrator = PhaseOrchestrator::new(&spec_id).unwrap();

    (orchestrator, temp_dir)
}

/// Test that executing phases in wrong order fails with proper error
#[tokio::test]
async fn test_execute_phases_wrong_order_fails() -> Result<()> {
    let (orchestrator, _temp_dir) = setup_test_environment("wrong-order");

    let config = OrchestratorConfig {
        dry_run: true,
        ..Default::default()
    };

    // Try to execute Design without Requirements
    let result = orchestrator.execute_design_phase(&config).await;
    assert!(
        result.is_err(),
        "Design phase should fail without Requirements"
    );

    // Verify error is about invalid transition
    let err = result.unwrap_err();
    let err_msg = err.to_string();
    assert!(
        err_msg.contains("dependency")
            || err_msg.contains("transition")
            || err_msg.contains("Requirements"),
        "Error should mention dependency or transition issue: {err_msg}"
    );

    Ok(())
}

/// Test that executing phases in correct order succeeds
#[tokio::test]
async fn test_execute_phases_correct_order_succeeds() -> Result<()> {
    let (orchestrator, _temp_dir) = setup_test_environment("correct-order");

    let config = OrchestratorConfig {
        dry_run: true,
        ..Default::default()
    };

    // Execute in correct order: Requirements -> Design -> Tasks
    let result1 = orchestrator.execute_requirements_phase(&config).await;
    assert!(result1.is_ok(), "Requirements phase should succeed");
    assert!(
        result1.as_ref().unwrap().success,
        "Requirements should be successful"
    );

    let result2 = orchestrator.execute_design_phase(&config).await;
    assert!(
        result2.is_ok(),
        "Design phase should succeed after Requirements"
    );
    assert!(
        result2.as_ref().unwrap().success,
        "Design should be successful"
    );

    let result3 = orchestrator.execute_tasks_phase(&config).await;
    assert!(result3.is_ok(), "Tasks phase should succeed after Design");
    assert!(
        result3.as_ref().unwrap().success,
        "Tasks should be successful"
    );

    Ok(())
}

/// Test resume from phase with validation
#[tokio::test]
async fn test_resume_from_phase_validates_transition() -> Result<()> {
    let (orchestrator, _temp_dir) = setup_test_environment("resume-validation");

    let config = OrchestratorConfig {
        dry_run: true,
        ..Default::default()
    };

    // Try to resume Design without Requirements
    let result = orchestrator
        .resume_from_phase(PhaseId::Design, &config)
        .await;
    assert!(
        result.is_err(),
        "Resume Design should fail without Requirements"
    );

    // Execute Requirements first
    orchestrator.execute_requirements_phase(&config).await?;

    // Now resume Design should work
    let result = orchestrator
        .resume_from_phase(PhaseId::Design, &config)
        .await;
    assert!(
        result.is_ok(),
        "Resume Design should succeed after Requirements"
    );

    Ok(())
}

/// Test that receipts are written with correct exit codes for transition errors
#[tokio::test]
async fn test_transition_error_exit_codes() -> Result<()> {
    let (orchestrator, _temp_dir) = setup_test_environment("exit-codes");

    let config = OrchestratorConfig {
        dry_run: true,
        ..Default::default()
    };

    // Try to execute Design without Requirements
    let result = orchestrator.execute_design_phase(&config).await;
    assert!(result.is_err());

    // Check that error maps to exit code 2
    let err = result.unwrap_err();

    // Downcast to XCheckerError to check exit code
    if let Some(xchecker_err) = err.downcast_ref::<xchecker::error::XCheckerError>() {
        let (exit_code, _) = xchecker_err.into();
        assert_eq!(exit_code, 2, "Transition error should map to exit code 2");
    } else {
        panic!("Expected XCheckerError, got: {err:?}");
    }

    Ok(())
}

/// Test dependency checking with multiple phases
#[tokio::test]
async fn test_dependency_checking_multiple_phases() -> Result<()> {
    let (orchestrator, _temp_dir) = setup_test_environment("multi-dep");

    let config = OrchestratorConfig {
        dry_run: true,
        ..Default::default()
    };

    // Execute Requirements and Design
    orchestrator.execute_requirements_phase(&config).await?;
    orchestrator.execute_design_phase(&config).await?;

    // Tasks should work (depends on Design, which depends on Requirements)
    let result = orchestrator.execute_tasks_phase(&config).await;
    assert!(
        result.is_ok(),
        "Tasks should succeed when all dependencies are met"
    );

    Ok(())
}

/// Test that validation happens before any phase execution
#[tokio::test]
async fn test_validation_before_execution() -> Result<()> {
    let (orchestrator, _temp_dir) = setup_test_environment("validation-first");

    let config = OrchestratorConfig {
        dry_run: true,
        ..Default::default()
    };

    // Try to execute Tasks without any prior phases
    let result = orchestrator.execute_tasks_phase(&config).await;
    assert!(
        result.is_err(),
        "Tasks should fail validation before execution"
    );

    // Verify no partial artifacts were created (validation failed before execution)
    let has_partial = orchestrator
        .artifact_manager()
        .has_partial_artifact(PhaseId::Tasks);
    assert!(
        !has_partial,
        "No partial artifacts should exist when validation fails"
    );

    Ok(())
}

/// Test actionable guidance in error messages
#[tokio::test]
async fn test_actionable_guidance_in_errors() -> Result<()> {
    let (orchestrator, _temp_dir) = setup_test_environment("guidance");

    let config = OrchestratorConfig {
        dry_run: true,
        ..Default::default()
    };

    // Try to execute Design without Requirements
    let result = orchestrator.execute_design_phase(&config).await;
    assert!(result.is_err());

    let err = result.unwrap_err();
    let err_msg = err.to_string();

    // Check that error provides actionable information
    assert!(!err_msg.is_empty(), "Error message should not be empty");

    // Error should mention what's wrong
    assert!(
        err_msg.contains("Design")
            || err_msg.contains("design")
            || err_msg.contains("Requirements")
            || err_msg.contains("requirements")
            || err_msg.contains("dependency")
            || err_msg.contains("transition"),
        "Error should mention the phase or dependency issue: {err_msg}"
    );

    Ok(())
}

/// Test that successful phases update state correctly
#[tokio::test]
async fn test_successful_phases_update_state() -> Result<()> {
    let (orchestrator, _temp_dir) = setup_test_environment("state-update");

    let config = OrchestratorConfig {
        dry_run: true,
        ..Default::default()
    };

    // Execute Requirements
    let result = orchestrator.execute_requirements_phase(&config).await?;
    assert!(result.success);
    assert_eq!(result.exit_code, 0);

    // Verify receipt was written
    assert!(
        result.receipt_path.is_some(),
        "Receipt should be written for successful phase"
    );

    // Verify we can now execute Design
    let validation = orchestrator.validate_transition(PhaseId::Design);
    assert!(
        validation.is_ok(),
        "Design should be allowed after successful Requirements"
    );

    Ok(())
}

/// Test that failed phases don't allow progression
#[tokio::test]
async fn test_failed_phases_block_progression() -> Result<()> {
    let (orchestrator, _temp_dir) = setup_test_environment("failed-block");

    // Create a failed receipt for Requirements
    let receipt_manager =
        xchecker::receipt::ReceiptManager::new(orchestrator.artifact_manager().base_path());

    let failed_receipt = receipt_manager.create_receipt(
        &"test-integration-failed-block".to_string(),
        PhaseId::Requirements,
        1, // Failed
        vec![],
        "0.1.0",
        "0.8.1",
        "claude-3-5-sonnet-20241022",
        None,
        std::collections::HashMap::new(),
        xchecker::types::PacketEvidence {
            files: vec![],
            max_bytes: 65536,
            max_lines: 1200,
        },
        None,                                      // stderr_redacted
        None,                                      // stderr_tail_excerpt
        vec![],                                    // warnings
        None,                                      // fallback_used
        "native",                                  // runner
        None,                                      // runner_distro
        Some(xchecker::types::ErrorKind::Unknown), // error_kind
        Some("Test failure".to_string()),          // error_reason
        None,                                      // diff_context,
        None,                                      // pipeline
    );

    receipt_manager.write_receipt(&failed_receipt)?;

    // Try to execute Design - should fail
    let config = OrchestratorConfig {
        dry_run: true,
        ..Default::default()
    };

    let result = orchestrator.execute_design_phase(&config).await;
    assert!(
        result.is_err(),
        "Design should fail when Requirements dependency failed"
    );

    Ok(())
}

//! Unit tests for phase transition validation (FR-ORC-001, FR-ORC-002)
//!
//! Tests that the orchestrator properly validates phase transitions and provides
//! actionable guidance for illegal transitions.
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
    let spec_id = format!("test-transition-{}", test_name);
    let orchestrator = PhaseOrchestrator::new(&spec_id).unwrap();

    (orchestrator, temp_dir)
}

/// Test that fresh spec can only start with Requirements phase
#[test]
fn test_fresh_spec_can_only_start_with_requirements() -> Result<()> {
    let (orchestrator, _temp_dir) = setup_test_environment("fresh-requirements");

    // Requirements should be allowed
    let result = orchestrator.validate_transition(PhaseId::Requirements);
    assert!(result.is_ok(), "Fresh spec should allow Requirements phase");

    // Design should not be allowed without Requirements
    let result = orchestrator.validate_transition(PhaseId::Design);
    assert!(
        result.is_err(),
        "Fresh spec should not allow Design phase without Requirements"
    );

    // Tasks should not be allowed without Design
    let result = orchestrator.validate_transition(PhaseId::Tasks);
    assert!(
        result.is_err(),
        "Fresh spec should not allow Tasks phase without Design"
    );

    // Review should not be allowed without Tasks
    let result = orchestrator.validate_transition(PhaseId::Review);
    assert!(
        result.is_err(),
        "Fresh spec should not allow Review phase without Tasks"
    );

    // Fixup should not be allowed without Review
    let result = orchestrator.validate_transition(PhaseId::Fixup);
    assert!(
        result.is_err(),
        "Fresh spec should not allow Fixup phase without Review"
    );

    // Final should not be allowed without Tasks
    let result = orchestrator.validate_transition(PhaseId::Final);
    assert!(
        result.is_err(),
        "Fresh spec should not allow Final phase without Tasks"
    );

    Ok(())
}

/// Test legal transition from Requirements to Design
#[tokio::test]
async fn test_legal_transition_requirements_to_design() -> Result<()> {
    let (orchestrator, _temp_dir) = setup_test_environment("req-to-design");

    let config = OrchestratorConfig {
        dry_run: true,
        ..Default::default()
    };

    // Execute Requirements phase
    let result = orchestrator.execute_requirements_phase(&config).await;
    assert!(result.is_ok(), "Requirements phase should succeed");

    // Now Design should be allowed
    let validation = orchestrator.validate_transition(PhaseId::Design);
    assert!(
        validation.is_ok(),
        "Design phase should be allowed after Requirements"
    );

    // Requirements can be re-run
    let validation = orchestrator.validate_transition(PhaseId::Requirements);
    assert!(validation.is_ok(), "Requirements phase can be re-run");

    // Tasks should still not be allowed (needs Design first)
    let validation = orchestrator.validate_transition(PhaseId::Tasks);
    assert!(
        validation.is_err(),
        "Tasks phase should not be allowed without Design"
    );

    Ok(())
}

/// Test legal transition from Design to Tasks
#[tokio::test]
async fn test_legal_transition_design_to_tasks() -> Result<()> {
    let (orchestrator, _temp_dir) = setup_test_environment("design-to-tasks");

    let config = OrchestratorConfig {
        dry_run: true,
        ..Default::default()
    };

    // Execute Requirements and Design phases
    orchestrator.execute_requirements_phase(&config).await?;
    orchestrator.execute_design_phase(&config).await?;

    // Now Tasks should be allowed
    let validation = orchestrator.validate_transition(PhaseId::Tasks);
    assert!(
        validation.is_ok(),
        "Tasks phase should be allowed after Design"
    );

    // Design can be re-run
    let validation = orchestrator.validate_transition(PhaseId::Design);
    assert!(validation.is_ok(), "Design phase can be re-run");

    // Review should still not be allowed (needs Tasks first)
    let validation = orchestrator.validate_transition(PhaseId::Review);
    assert!(
        validation.is_err(),
        "Review phase should not be allowed without Tasks"
    );

    Ok(())
}

/// Test legal transition from Tasks to Review or Final
#[tokio::test]
async fn test_legal_transition_tasks_to_review_or_final() -> Result<()> {
    let (orchestrator, _temp_dir) = setup_test_environment("tasks-to-review-final");

    let config = OrchestratorConfig {
        dry_run: true,
        ..Default::default()
    };

    // Execute Requirements, Design, and Tasks phases
    orchestrator.execute_requirements_phase(&config).await?;
    orchestrator.execute_design_phase(&config).await?;
    orchestrator.execute_tasks_phase(&config).await?;

    // Review should be allowed
    let validation = orchestrator.validate_transition(PhaseId::Review);
    assert!(
        validation.is_ok(),
        "Review phase should be allowed after Tasks"
    );

    // Final should also be allowed (can skip Review/Fixup)
    let validation = orchestrator.validate_transition(PhaseId::Final);
    assert!(
        validation.is_ok(),
        "Final phase should be allowed after Tasks (can skip Review/Fixup)"
    );

    // Tasks can be re-run
    let validation = orchestrator.validate_transition(PhaseId::Tasks);
    assert!(validation.is_ok(), "Tasks phase can be re-run");

    // Fixup should not be allowed (needs Review first)
    let validation = orchestrator.validate_transition(PhaseId::Fixup);
    assert!(
        validation.is_err(),
        "Fixup phase should not be allowed without Review"
    );

    Ok(())
}

/// Test illegal transition with actionable guidance
#[test]
fn test_illegal_transition_provides_guidance() -> Result<()> {
    let (orchestrator, _temp_dir) = setup_test_environment("illegal-guidance");

    // Try to run Design without Requirements
    let result = orchestrator.validate_transition(PhaseId::Design);
    assert!(result.is_err(), "Design without Requirements should fail");

    let err = result.unwrap_err();
    let err_msg = err.to_string();

    // Check that error message contains useful information
    // The error could be either about invalid transition or dependency not satisfied
    assert!(
        err_msg.contains("Design")
            || err_msg.contains("design")
            || err_msg.contains("Requirements")
            || err_msg.contains("requirements")
            || err_msg.contains("dependency")
            || err_msg.contains("transition"),
        "Error should mention Design, Requirements, dependency, or transition: {}",
        err_msg
    );

    Ok(())
}

/// Test dependency not satisfied error
#[tokio::test]
async fn test_dependency_not_satisfied_error() -> Result<()> {
    let (orchestrator, _temp_dir) = setup_test_environment("dep-not-satisfied");

    // Try to run Design without Requirements
    let result = orchestrator.validate_transition(PhaseId::Design);
    assert!(
        result.is_err(),
        "Design should fail without Requirements dependency"
    );

    let err = result.unwrap_err();
    let err_msg = err.to_string();

    // Check error message - could be either invalid transition or dependency not satisfied
    assert!(
        err_msg.contains("dependency")
            || err_msg.contains("Requirements")
            || err_msg.contains("requirements")
            || err_msg.contains("transition")
            || err_msg.contains("Design")
            || err_msg.contains("design"),
        "Error should mention dependency, Requirements, or transition: {}",
        err_msg
    );

    Ok(())
}

/// Test that failed dependency prevents transition
#[tokio::test]
async fn test_failed_dependency_prevents_transition() -> Result<()> {
    let (orchestrator, _temp_dir) = setup_test_environment("failed-dep");

    // Create a failed receipt for Requirements phase
    let receipt_manager =
        xchecker::receipt::ReceiptManager::new(orchestrator.artifact_manager().base_path());

    let failed_receipt = receipt_manager.create_receipt(
        &format!("test-transition-failed-dep"),
        PhaseId::Requirements,
        1, // Non-zero exit code indicates failure
        vec![],
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        std::collections::HashMap::new(),
        xchecker::types::PacketEvidence {
            files: vec![],
            max_bytes: 65536,
            max_lines: 1200,
        },
        None,                                      // stderr_tail
        None,                                      // stderr_redacted
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

    // Try to run Design - should fail because Requirements failed
    let result = orchestrator.validate_transition(PhaseId::Design);
    assert!(
        result.is_err(),
        "Design should fail when Requirements dependency failed"
    );

    Ok(())
}

/// Test all legal phase transitions in sequence
#[tokio::test]
async fn test_complete_legal_workflow() -> Result<()> {
    let (orchestrator, _temp_dir) = setup_test_environment("complete-workflow");

    let config = OrchestratorConfig {
        dry_run: true,
        ..Default::default()
    };

    // 1. Requirements phase
    assert!(
        orchestrator
            .validate_transition(PhaseId::Requirements)
            .is_ok(),
        "Requirements should be allowed on fresh spec"
    );
    orchestrator.execute_requirements_phase(&config).await?;

    // 2. Design phase
    assert!(
        orchestrator.validate_transition(PhaseId::Design).is_ok(),
        "Design should be allowed after Requirements"
    );
    orchestrator.execute_design_phase(&config).await?;

    // 3. Tasks phase
    assert!(
        orchestrator.validate_transition(PhaseId::Tasks).is_ok(),
        "Tasks should be allowed after Design"
    );
    orchestrator.execute_tasks_phase(&config).await?;

    // 4. Can go to Review or Final
    assert!(
        orchestrator.validate_transition(PhaseId::Review).is_ok(),
        "Review should be allowed after Tasks"
    );
    assert!(
        orchestrator.validate_transition(PhaseId::Final).is_ok(),
        "Final should be allowed after Tasks"
    );

    Ok(())
}

/// Test that phases can be re-run
#[tokio::test]
async fn test_phases_can_be_rerun() -> Result<()> {
    let (orchestrator, _temp_dir) = setup_test_environment("rerun-phases");

    let config = OrchestratorConfig {
        dry_run: true,
        ..Default::default()
    };

    // Execute Requirements twice
    orchestrator.execute_requirements_phase(&config).await?;
    assert!(
        orchestrator
            .validate_transition(PhaseId::Requirements)
            .is_ok(),
        "Requirements can be re-run"
    );
    orchestrator.execute_requirements_phase(&config).await?;

    // Execute Design twice
    orchestrator.execute_design_phase(&config).await?;
    assert!(
        orchestrator.validate_transition(PhaseId::Design).is_ok(),
        "Design can be re-run"
    );
    orchestrator.execute_design_phase(&config).await?;

    // Execute Tasks twice
    orchestrator.execute_tasks_phase(&config).await?;
    assert!(
        orchestrator.validate_transition(PhaseId::Tasks).is_ok(),
        "Tasks can be re-run"
    );

    Ok(())
}

/// Test exit code for invalid transition (should be 2 per FR-ORC-001)
#[test]
fn test_invalid_transition_exit_code() -> Result<()> {
    let (orchestrator, _temp_dir) = setup_test_environment("exit-code");

    // Try invalid transition
    let result = orchestrator.validate_transition(PhaseId::Design);
    assert!(result.is_err());

    let err = result.unwrap_err();

    // Check that this maps to exit code 2
    let (exit_code, error_kind) = (&err).into();
    assert_eq!(exit_code, 2, "Invalid transition should map to exit code 2");
    assert_eq!(
        error_kind,
        xchecker::types::ErrorKind::CliArgs,
        "Invalid transition should map to CliArgs error kind"
    );

    Ok(())
}

/// Test calling run_phase(Design) on fresh spec with InvalidTransition error
#[tokio::test]
async fn test_design_on_fresh_spec_error_specificity() -> Result<()> {
    let (orchestrator, _temp_dir) = setup_test_environment("design-fresh-error");

    let config = OrchestratorConfig {
        dry_run: true,
        ..Default::default()
    };

    // Try to run Design phase on fresh spec without Requirements
    let result = orchestrator.execute_design_phase(&config).await;

    // Should return Err (validation fails before execution)
    assert!(result.is_err(), "Should return Err for invalid transition");
    let err = result.unwrap_err();

    // Downcast to XCheckerError
    let xchecker_err = err
        .downcast_ref::<xchecker::error::XCheckerError>()
        .expect("Error should be XCheckerError");

    // Verify error kind and exit code mapping
    let (exit_code, error_kind) = xchecker_err.into();
    assert_eq!(
        exit_code, 2,
        "Invalid transition should map to exit code 2 (CLI_ARGS)"
    );
    assert_eq!(
        error_kind,
        xchecker::types::ErrorKind::CliArgs,
        "error_kind should be CliArgs"
    );

    // Verify error message contains appropriate context
    let error_msg = err.to_string();
    assert!(
        error_msg.contains("transition")
            || error_msg.contains("Requirements")
            || error_msg.contains("requirements"),
        "Error should mention transition or Requirements: {}",
        error_msg
    );

    // Verify it's specifically an InvalidTransition error
    match xchecker_err {
        xchecker::error::XCheckerError::Phase(phase_err) => match phase_err {
            xchecker::error::PhaseError::InvalidTransition { from, to } => {
                assert_eq!(to, "design", "Target phase should be design");
                assert!(
                    from.contains("none") || from.contains("fresh"),
                    "From phase should indicate fresh spec: {}",
                    from
                );
            }
            _ => panic!("Expected InvalidTransition error, got: {:?}", phase_err),
        },
        _ => panic!("Expected Phase error, got: {:?}", xchecker_err),
    }

    Ok(())
}

/// Test resuming from Tasks without completing Design first (DependencyNotSatisfied)
#[tokio::test]
async fn test_tasks_without_design_dependency_error() -> Result<()> {
    let (orchestrator, _temp_dir) = setup_test_environment("tasks-no-design");

    let config = OrchestratorConfig {
        dry_run: true,
        ..Default::default()
    };

    // Execute Requirements phase only
    orchestrator.execute_requirements_phase(&config).await?;

    // Try to run Tasks phase without Design
    let result = orchestrator.execute_tasks_phase(&config).await;

    // Should return Err (validation fails before execution)
    assert!(
        result.is_err(),
        "Should return Err for dependency not satisfied"
    );
    let err = result.unwrap_err();

    // Downcast to XCheckerError
    let xchecker_err = err
        .downcast_ref::<xchecker::error::XCheckerError>()
        .expect("Error should be XCheckerError");

    // Verify error kind and exit code mapping
    let (exit_code, error_kind) = xchecker_err.into();
    assert_eq!(
        exit_code, 2,
        "Dependency not satisfied should map to exit code 2 (CLI_ARGS)"
    );
    assert_eq!(
        error_kind,
        xchecker::types::ErrorKind::CliArgs,
        "error_kind should be CliArgs"
    );

    // Verify error message contains appropriate context
    let error_msg = err.to_string();
    assert!(
        error_msg.contains("dependency")
            || error_msg.contains("Design")
            || error_msg.contains("design")
            || error_msg.contains("transition"),
        "Error should mention dependency, Design, or transition: {}",
        error_msg
    );

    // Verify it's a phase transition error (either InvalidTransition or DependencyNotSatisfied)
    // The actual error type depends on whether transition validation or dependency check triggers first
    match xchecker_err {
        xchecker::error::XCheckerError::Phase(phase_err) => match phase_err {
            xchecker::error::PhaseError::DependencyNotSatisfied { phase, dependency } => {
                assert_eq!(phase, "tasks", "Phase should be tasks");
                assert_eq!(dependency, "design", "Dependency should be design");
            }
            xchecker::error::PhaseError::InvalidTransition { from, to } => {
                // Transition from Requirements to Tasks is invalid without Design
                assert_eq!(to, "tasks", "Target phase should be tasks");
                assert!(
                    from.contains("requirements"),
                    "From phase should be requirements: {}",
                    from
                );
            }
            _ => panic!(
                "Expected DependencyNotSatisfied or InvalidTransition error, got: {:?}",
                phase_err
            ),
        },
        _ => panic!("Expected Phase error, got: {:?}", xchecker_err),
    }

    Ok(())
}

//! Black-box sentinel test for `OrchestratorHandle` facade
//!
//! **WHITE-BOX TEST**: This test uses internal module APIs (`orchestrator::{OrchestratorConfig,
//! OrchestratorHandle}`, `paths::with_isolated_home`, `types::PhaseId`) and may break with
//! internal refactors. These tests are intentionally white-box to validate internal
//! implementation details. See FR-TEST-4 for white-box test policy.
//!
//! ┌─────────────────────────────────────────────────────────────────────────────┐
//! │ CANARY TEST: This is the sentinel for the OrchestratorHandle contract.      │
//! │                                                                             │
//! │ Any change to the handle semantics should start by updating this file.      │
//! │ If these tests fail, external consumers (CLI, Kiro, MCP tools) may break.   │
//! └─────────────────────────────────────────────────────────────────────────────┘
//!
//! This test validates the invariants that external consumers can rely on when
//! using `OrchestratorHandle`. It uses ONLY the public handle API and validates:
//!
//! - Phase execution produces `ExecutionResult` with expected fields
//! - Receipts are always written (even in dry-run)
//! - `pipeline.execution_strategy` is set to "controlled"
//! - Handle methods behave according to documented contracts
//!
//! **Important**: This test uses `dry_run: true` to avoid external dependencies.
//! It serves as a canary for the facade contract.

use anyhow::Result;
use std::collections::HashMap;

use xchecker::orchestrator::{OrchestratorConfig, OrchestratorHandle};
use xchecker::paths::with_isolated_home;
use xchecker::types::PhaseId;

/// Create a dry-run config for testing
fn dry_run_config() -> OrchestratorConfig {
    OrchestratorConfig {
        dry_run: true,
        config: HashMap::new(),
        full_config: None,
        selectors: None,
        strict_validation: false,
        redactor: Default::default(),
        hooks: None,
    }
}

/// Create a unique spec ID for test isolation
fn unique_spec_id(test_name: &str) -> String {
    format!("handle-smoke-{}-{}", test_name, std::process::id())
}

/// Test 1: Handle runs Requirements phase in dry-run mode
///
/// Validates:
/// - `run_phase` returns `ExecutionResult` with success
/// - Exit code is 0
/// - Receipt path is populated
/// - Artifact paths are populated
#[tokio::test]
async fn handle_runs_requirements_in_dry_run() -> Result<()> {
    let _home = with_isolated_home();
    let spec_id = unique_spec_id("requirements");
    let config = dry_run_config();

    let mut handle = OrchestratorHandle::with_config_and_force(&spec_id, config, false)?;
    let result = handle.run_phase(PhaseId::Requirements).await?;

    // Verify ExecutionResult invariants
    assert!(
        result.success,
        "Requirements phase should succeed in dry-run"
    );
    assert_eq!(result.exit_code, 0, "Exit code should be 0");
    assert_eq!(result.phase, PhaseId::Requirements, "Phase should match");
    assert!(
        result.receipt_path.is_some(),
        "Receipt path should be populated"
    );
    assert!(
        !result.artifact_paths.is_empty(),
        "Artifact paths should be populated"
    );
    assert!(result.error.is_none(), "No error on success");

    Ok(())
}

/// Test 2: Receipt has correct pipeline metadata
///
/// Validates:
/// - Receipt file exists on disk
/// - `pipeline.execution_strategy` is "controlled"
/// - Receipt JSON is valid
#[tokio::test]
async fn handle_receipt_has_pipeline_metadata() -> Result<()> {
    let _home = with_isolated_home();
    let spec_id = unique_spec_id("pipeline-meta");
    let config = dry_run_config();

    let mut handle = OrchestratorHandle::with_config_and_force(&spec_id, config, false)?;
    let result = handle.run_phase(PhaseId::Requirements).await?;

    // Read and parse receipt
    let receipt_path = result.receipt_path.expect("Should have receipt path");
    let receipt_content = std::fs::read_to_string(&receipt_path)?;
    let receipt: serde_json::Value = serde_json::from_str(&receipt_content)?;

    // Verify pipeline metadata
    let pipeline = &receipt["pipeline"];
    assert!(!pipeline.is_null(), "Receipt should have pipeline field");
    assert_eq!(
        pipeline["execution_strategy"].as_str(),
        Some("controlled"),
        "Execution strategy should be 'controlled'"
    );

    Ok(())
}

/// Test 3: Handle respects current_phase and legal_next_phases
///
/// Validates:
/// - `current_phase()` returns None before any execution
/// - After Requirements, `current_phase()` returns Some(Requirements)
/// - `legal_next_phases()` returns correct transitions
#[tokio::test]
async fn handle_phase_state_tracking() -> Result<()> {
    let _home = with_isolated_home();
    let spec_id = unique_spec_id("phase-state");
    let config = dry_run_config();

    let mut handle = OrchestratorHandle::with_config_and_force(&spec_id, config, false)?;

    // Before execution, no current phase
    let current = handle.current_phase()?;
    assert!(current.is_none(), "No current phase before execution");

    // Legal next phases should start with Requirements
    let legal = handle.legal_next_phases()?;
    assert_eq!(
        legal,
        vec![PhaseId::Requirements],
        "Should only allow Requirements initially"
    );

    // Execute Requirements
    let result = handle.run_phase(PhaseId::Requirements).await?;
    assert!(result.success, "Requirements should succeed");

    // After Requirements, current phase should be Requirements
    let current_after = handle.current_phase()?;
    assert_eq!(
        current_after,
        Some(PhaseId::Requirements),
        "Current phase should be Requirements after execution"
    );

    // Legal next phases should include Design
    let legal_after = handle.legal_next_phases()?;
    assert!(
        legal_after.contains(&PhaseId::Design),
        "Should allow Design after Requirements"
    );

    Ok(())
}

/// Test 4: Handle can_run_phase validates dependencies
///
/// Validates:
/// - `can_run_phase(Requirements)` is true initially
/// - `can_run_phase(Design)` is false before Requirements
/// - `can_run_phase(Design)` is true after Requirements
#[tokio::test]
async fn handle_can_run_phase_validates_dependencies() -> Result<()> {
    let _home = with_isolated_home();
    let spec_id = unique_spec_id("can-run");
    let config = dry_run_config();

    let mut handle = OrchestratorHandle::with_config_and_force(&spec_id, config, false)?;

    // Requirements should be runnable initially
    assert!(
        handle.can_run_phase(PhaseId::Requirements)?,
        "Requirements should be runnable initially"
    );

    // Design should NOT be runnable before Requirements
    assert!(
        !handle.can_run_phase(PhaseId::Design)?,
        "Design should not be runnable before Requirements"
    );

    // Execute Requirements
    let result = handle.run_phase(PhaseId::Requirements).await?;
    assert!(result.success, "Requirements should succeed");

    // Now Design should be runnable
    assert!(
        handle.can_run_phase(PhaseId::Design)?,
        "Design should be runnable after Requirements"
    );

    Ok(())
}

/// Test 5: Handle readonly mode does not acquire locks
///
/// Validates:
/// - `readonly()` succeeds even without prior state
/// - Readonly handle can read managers
/// - Multiple readonly handles can coexist (no lock contention)
#[tokio::test]
async fn handle_readonly_does_not_lock() -> Result<()> {
    let _home = with_isolated_home();
    let spec_id = unique_spec_id("readonly");

    // Create readonly handle
    let handle1 = OrchestratorHandle::readonly(&spec_id)?;
    let handle2 = OrchestratorHandle::readonly(&spec_id)?;

    // Both handles should work
    assert_eq!(handle1.spec_id(), spec_id);
    assert_eq!(handle2.spec_id(), spec_id);

    // Can access managers
    let _artifact_manager = handle1.artifact_manager();
    let _receipt_manager = handle2.receipt_manager();

    Ok(())
}

/// Test 6: Handle config accessors work correctly
///
/// Validates:
/// - `set_config` and `get_config` work as expected
/// - `set_dry_run` modifies config
/// - `spec_id` returns correct value
#[tokio::test]
async fn handle_config_accessors() -> Result<()> {
    let _home = with_isolated_home();
    let spec_id = unique_spec_id("config-access");
    let config = dry_run_config();

    let mut handle = OrchestratorHandle::with_config_and_force(&spec_id, config, false)?;

    // Test spec_id
    assert_eq!(handle.spec_id(), spec_id);

    // Test config accessors
    assert!(handle.get_config("model").is_none());
    handle.set_config("model", "test-model");
    assert_eq!(handle.get_config("model"), Some(&"test-model".to_string()));

    // Test dry_run accessor
    assert!(
        handle.orchestrator_config().dry_run,
        "Should start in dry-run mode"
    );
    handle.set_dry_run(false);
    assert!(
        !handle.orchestrator_config().dry_run,
        "Should be changed to non-dry-run"
    );

    Ok(())
}

/// Test 7: Handle sequential phase execution works
///
/// Validates:
/// - Can execute Requirements then Design in sequence
/// - Each phase produces artifacts and receipts
/// - Phase dependencies are respected
#[tokio::test]
async fn handle_sequential_phase_execution() -> Result<()> {
    let _home = with_isolated_home();
    let spec_id = unique_spec_id("sequential");
    let config = dry_run_config();

    let mut handle = OrchestratorHandle::with_config_and_force(&spec_id, config, false)?;

    // Execute Requirements
    let req_result = handle.run_phase(PhaseId::Requirements).await?;
    assert!(req_result.success, "Requirements should succeed");
    assert!(req_result.receipt_path.is_some(), "Should have receipt");

    // Execute Design
    let design_result = handle.run_phase(PhaseId::Design).await?;
    assert!(design_result.success, "Design should succeed");
    assert!(design_result.receipt_path.is_some(), "Should have receipt");

    // Verify both receipts exist
    let req_receipt = std::fs::read_to_string(req_result.receipt_path.unwrap())?;
    let design_receipt = std::fs::read_to_string(design_result.receipt_path.unwrap())?;

    let req_json: serde_json::Value = serde_json::from_str(&req_receipt)?;
    let design_json: serde_json::Value = serde_json::from_str(&design_receipt)?;

    assert_eq!(req_json["phase"].as_str(), Some("requirements"));
    assert_eq!(design_json["phase"].as_str(), Some("design"));

    Ok(())
}

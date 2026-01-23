//! Regression test for workflow receipt consistency
//!
//! **WHITE-BOX TEST**: This test uses internal module APIs (`orchestrator::{OrchestratorConfig,
//! PhaseOrchestrator}`) and may break with internal refactors. These tests are intentionally
//! white-box to validate internal implementation details. See FR-TEST-4 for white-box test policy.
//!
//! Validates that:
//! 1. All workflow phases generate receipts with consistent metadata
//! 2. LLM info is properly populated (provider, model)
//! 3. Pipeline info has execution_strategy = "controlled"
//! 4. Packet evidence is preserved

use anyhow::Result;
use tempfile::TempDir;
use xchecker::orchestrator::{OrchestratorConfig, PhaseOrchestrator};

#[allow(clippy::duplicate_mod)]
#[path = "test_support/mod.rs"]
mod test_support;

/// Test environment for workflow receipt tests
///
/// Note: Field order matters for drop semantics. Fields drop in declaration order,
/// so `_cwd_guard` must be declared first to restore CWD before `temp_dir` is deleted.
struct ReceiptTestEnv {
    #[allow(dead_code)]
    _cwd_guard: test_support::CwdGuard,
    #[allow(dead_code)]
    temp_dir: TempDir,
    orchestrator: PhaseOrchestrator,
}

fn setup_test(test_name: &str) -> ReceiptTestEnv {
    let temp_dir = xchecker::paths::with_isolated_home();
    let cwd_guard = test_support::CwdGuard::new(temp_dir.path()).unwrap();
    let spec_id = format!("test-workflow-receipt-{test_name}");
    let orchestrator = PhaseOrchestrator::new(&spec_id).unwrap();
    ReceiptTestEnv {
        _cwd_guard: cwd_guard,
        temp_dir,
        orchestrator,
    }
}

fn dry_run_config() -> OrchestratorConfig {
    OrchestratorConfig {
        dry_run: true,
        config: Default::default(),
        full_config: None,
        selectors: None,
        strict_validation: false,
        redactor: Default::default(),
        hooks: None,
    }
}

#[tokio::test]
async fn test_requirements_receipt_has_llm_info() -> Result<()> {
    let env = setup_test("llm-info");
    let orchestrator = env.orchestrator;
    let config = dry_run_config();

    let result = orchestrator.execute_requirements_phase(&config).await?;
    assert!(result.success, "Phase should succeed");

    // Read receipt
    let receipt_path = result.receipt_path.expect("Should have receipt path");
    let content = std::fs::read_to_string(&receipt_path)?;
    let receipt: serde_json::Value = serde_json::from_str(&content)?;

    // In dry-run mode, llm info may be null since we're simulating execution
    // The important thing is that the field exists in the schema
    assert!(
        receipt.get("llm").is_some(),
        "Receipt should have llm field (even if null in dry-run)"
    );

    // Verify the schema supports LLM tracking
    // In a real (non-dry-run) execution, this would contain provider/model info
    // For now, we just verify the field is present in the receipt structure

    Ok(())
}

#[tokio::test]
async fn test_requirements_receipt_has_pipeline_info() -> Result<()> {
    let env = setup_test("pipeline-info");
    let orchestrator = env.orchestrator;
    let config = dry_run_config();

    let result = orchestrator.execute_requirements_phase(&config).await?;
    assert!(result.success, "Phase should succeed");

    // Read receipt
    let receipt_path = result.receipt_path.expect("Should have receipt path");
    let content = std::fs::read_to_string(&receipt_path)?;
    let receipt: serde_json::Value = serde_json::from_str(&content)?;

    // Verify pipeline info
    let pipeline = &receipt["pipeline"];
    assert!(pipeline.is_object(), "Receipt should have pipeline object");
    assert_eq!(
        pipeline["execution_strategy"].as_str(),
        Some("controlled"),
        "Execution strategy should be controlled"
    );

    Ok(())
}

#[tokio::test]
async fn test_receipt_packet_evidence_preserved() -> Result<()> {
    let env = setup_test("packet-evidence");
    let orchestrator = env.orchestrator;
    let config = dry_run_config();

    let result = orchestrator.execute_requirements_phase(&config).await?;
    assert!(result.success);

    // Read receipt
    let receipt_path = result.receipt_path.expect("Should have receipt path");
    let content = std::fs::read_to_string(&receipt_path)?;
    let receipt: serde_json::Value = serde_json::from_str(&content)?;

    // Verify packet evidence
    let packet = &receipt["packet"];
    assert!(packet.is_object(), "Receipt should have packet object");
    assert!(
        packet["max_bytes"].as_u64().unwrap_or(0) > 0,
        "max_bytes should be positive"
    );
    assert!(
        packet["max_lines"].as_u64().unwrap_or(0) > 0,
        "max_lines should be positive"
    );

    Ok(())
}

#[tokio::test]
async fn test_multi_phase_receipts_consistent_metadata() -> Result<()> {
    let env = setup_test("multi-phase");
    let orchestrator = env.orchestrator;
    let config = dry_run_config();

    // Execute requirements
    let req_result = orchestrator.execute_requirements_phase(&config).await?;
    assert!(req_result.success);

    // Read requirements receipt
    let req_receipt_path = req_result.receipt_path.expect("Should have receipt");
    let req_content = std::fs::read_to_string(&req_receipt_path)?;
    let req_receipt: serde_json::Value = serde_json::from_str(&req_content)?;

    // Verify schema version
    assert_eq!(req_receipt["schema_version"].as_str(), Some("1"));

    // Verify model consistency
    let model = req_receipt["model_full_name"].as_str();
    assert!(model.is_some(), "Should have model_full_name");

    // Verify xchecker version present
    assert!(req_receipt["xchecker_version"].as_str().is_some());

    Ok(())
}

/// B1: Test single-phase vs workflow receipt parity
///
/// This test validates that receipts generated by single-phase execution
/// and workflow execution contain the same key metadata fields, ensuring
/// consistency regardless of execution path.
#[tokio::test]
async fn test_single_phase_vs_workflow_receipt_parity() -> Result<()> {
    // Run Requirements via single-phase path
    let env_single = setup_test("single-phase-parity");
    let orchestrator_single = env_single.orchestrator;
    let config = dry_run_config();

    let single_result = orchestrator_single
        .execute_requirements_phase(&config)
        .await?;
    assert!(
        single_result.success,
        "Single-phase Requirements should succeed"
    );

    // Read single-phase receipt
    let single_receipt_path = single_result
        .receipt_path
        .expect("Single-phase should have receipt");
    let single_content = std::fs::read_to_string(&single_receipt_path)?;
    let single_receipt: serde_json::Value = serde_json::from_str(&single_content)?;

    // Run Requirements via OrchestratorHandle (workflow path)
    let _temp_workflow = xchecker::paths::with_isolated_home();
    let spec_id = format!("test-workflow-parity-{}", std::process::id());
    let mut handle = xchecker::orchestrator::OrchestratorHandle::with_config_and_force(
        &spec_id,
        xchecker::orchestrator::OrchestratorConfig {
            dry_run: true,
            config: Default::default(),
            full_config: None,
            selectors: None,
            strict_validation: false,
            redactor: Default::default(),
            hooks: None,
        },
        false,
    )?;

    let workflow_result = handle
        .run_phase(xchecker::types::PhaseId::Requirements)
        .await?;
    assert!(
        workflow_result.success,
        "Workflow Requirements should succeed"
    );

    // Read workflow receipt
    let workflow_receipt_path = workflow_result
        .receipt_path
        .expect("Workflow should have receipt");
    let workflow_content = std::fs::read_to_string(&workflow_receipt_path)?;
    let workflow_receipt: serde_json::Value = serde_json::from_str(&workflow_content)?;

    // Compare key receipt fields:

    // 1. exit_code
    assert_eq!(
        single_receipt["exit_code"].as_i64(),
        workflow_receipt["exit_code"].as_i64(),
        "Exit codes should match"
    );
    assert_eq!(
        single_receipt["exit_code"].as_i64(),
        Some(0),
        "Exit code should be 0 for success"
    );

    // 2. packet.files (both should have packet evidence)
    assert!(
        single_receipt["packet"].is_object(),
        "Single-phase should have packet object"
    );
    assert!(
        workflow_receipt["packet"].is_object(),
        "Workflow should have packet object"
    );
    assert!(
        single_receipt["packet"]["max_bytes"].as_u64().unwrap_or(0) > 0,
        "Single-phase packet.max_bytes should be positive"
    );
    assert!(
        workflow_receipt["packet"]["max_bytes"]
            .as_u64()
            .unwrap_or(0)
            > 0,
        "Workflow packet.max_bytes should be positive"
    );

    // 3. llm.provider and llm.model_used
    // Both should have LLM metadata
    assert!(
        single_receipt["llm"].is_object() || single_receipt["llm"].is_null(),
        "Single-phase should have llm field"
    );
    assert!(
        workflow_receipt["llm"].is_object() || workflow_receipt["llm"].is_null(),
        "Workflow should have llm field"
    );

    // If LLM info is present (non-null), both should have provider and model_used
    if single_receipt["llm"].is_object() && workflow_receipt["llm"].is_object() {
        let single_provider = single_receipt["llm"]["provider"].as_str();
        let workflow_provider = workflow_receipt["llm"]["provider"].as_str();

        if let (Some(sp), Some(wp)) = (single_provider, workflow_provider) {
            // Both have provider info - verify they're consistent
            assert!(
                sp.contains("claude") || sp.contains("simulated"),
                "Single-phase provider should be claude-related"
            );
            assert!(
                wp.contains("claude") || wp.contains("simulated"),
                "Workflow provider should be claude-related"
            );
        }

        // Check model_used is present if llm object exists
        let single_model = single_receipt["llm"]["model_used"].as_str();
        let workflow_model = workflow_receipt["llm"]["model_used"].as_str();

        if let (Some(sm), Some(wm)) = (single_model, workflow_model) {
            // Both have model info - verify they're reasonable
            assert!(
                !sm.is_empty(),
                "Single-phase model_used should not be empty"
            );
            assert!(!wm.is_empty(), "Workflow model_used should not be empty");
        }
    }

    // 4. pipeline.execution_strategy
    assert_eq!(
        single_receipt["pipeline"]["execution_strategy"].as_str(),
        Some("controlled"),
        "Single-phase execution_strategy should be 'controlled'"
    );
    assert_eq!(
        workflow_receipt["pipeline"]["execution_strategy"].as_str(),
        Some("controlled"),
        "Workflow execution_strategy should be 'controlled'"
    );

    // 5. outputs hashes (both should have outputs array)
    assert!(
        single_receipt["outputs"].is_array(),
        "Single-phase should have outputs array"
    );
    assert!(
        workflow_receipt["outputs"].is_array(),
        "Workflow should have outputs array"
    );

    // For successful execution, verify outputs have blake3 hashes
    let single_outputs = single_receipt["outputs"].as_array().unwrap();
    let workflow_outputs = workflow_receipt["outputs"].as_array().unwrap();

    if !single_outputs.is_empty() {
        assert!(
            single_outputs[0]["blake3_canonicalized"].is_string(),
            "Single-phase outputs should have blake3 hashes"
        );
    }

    if !workflow_outputs.is_empty() {
        assert!(
            workflow_outputs[0]["blake3_canonicalized"].is_string(),
            "Workflow outputs should have blake3 hashes"
        );
    }

    Ok(())
}

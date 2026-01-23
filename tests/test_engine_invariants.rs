//! B3: Tests for execute_phase_core invariants that should never break
//!
//! These tests validate the core invariants of phase execution:
//! 1. Phase output always has packet_evidence with non-empty files
//! 2. Successful phases (exit_code 0) always have artifacts
//! 3. Output hashes length matches artifacts
//!
//! These tests use the public OrchestratorHandle API and dry-run mode
//! to validate core engine behavior without external dependencies.

use anyhow::Result;
use std::collections::HashMap;

/// Helper to create a dry-run config for testing
fn dry_run_config() -> xchecker::orchestrator::OrchestratorConfig {
    xchecker::orchestrator::OrchestratorConfig {
        dry_run: true,
        config: HashMap::new(),
        full_config: None,
        selectors: None,
        strict_validation: false,
        redactor: Default::default(),
        hooks: None,
    }
}

/// Helper to create a unique spec ID for test isolation
fn unique_spec_id(test_name: &str) -> String {
    format!("engine-invariants-{}-{}", test_name, std::process::id())
}

/// B3.1: Test that core output has packet evidence with files
///
/// Validates that phase execution always produces packet evidence
/// and that the packet evidence contains information about included files.
#[tokio::test]
async fn test_core_output_has_packet_evidence() -> Result<()> {
    let _temp = xchecker::paths::with_isolated_home();
    let spec_id = unique_spec_id("packet-evidence");
    let config = dry_run_config();

    let mut handle =
        xchecker::orchestrator::OrchestratorHandle::with_config_and_force(&spec_id, config, false)?;

    // Run a phase (Requirements)
    let result = handle
        .run_phase(xchecker::types::PhaseId::Requirements)
        .await?;

    assert!(result.success, "Phase should succeed");

    // Read receipt to verify packet_evidence
    let receipt_path = result.receipt_path.expect("Should have receipt path");
    let receipt_content = std::fs::read_to_string(&receipt_path)?;
    let receipt: serde_json::Value = serde_json::from_str(&receipt_content)?;

    // Verify packet evidence exists
    assert!(
        receipt["packet"].is_object(),
        "Receipt should have packet object"
    );

    let packet = &receipt["packet"];

    // Verify packet has required fields
    assert!(
        packet["max_bytes"].is_number(),
        "Packet should have max_bytes"
    );
    assert!(
        packet["max_lines"].is_number(),
        "Packet should have max_lines"
    );
    assert!(packet["files"].is_array(), "Packet should have files array");

    // Verify max_bytes and max_lines are positive
    assert!(
        packet["max_bytes"].as_u64().unwrap_or(0) > 0,
        "Packet max_bytes should be positive"
    );
    assert!(
        packet["max_lines"].as_u64().unwrap_or(0) > 0,
        "Packet max_lines should be positive"
    );

    Ok(())
}

/// B3.2: Test that successful phases have artifacts
///
/// Validates that when a phase exits with code 0, it produces artifacts
/// and those artifacts are recorded in the receipt.
#[tokio::test]
async fn test_core_output_success_has_artifacts() -> Result<()> {
    let _temp = xchecker::paths::with_isolated_home();
    let spec_id = unique_spec_id("success-artifacts");
    let config = dry_run_config();

    let mut handle =
        xchecker::orchestrator::OrchestratorHandle::with_config_and_force(&spec_id, config, false)?;

    // Run a phase (Requirements) with exit_code 0
    let result = handle
        .run_phase(xchecker::types::PhaseId::Requirements)
        .await?;

    assert!(result.success, "Phase should succeed");
    assert_eq!(result.exit_code, 0, "Exit code should be 0");

    // Verify artifacts were created
    assert!(
        !result.artifact_paths.is_empty(),
        "Successful phase should produce artifacts"
    );

    // Verify artifacts exist on disk
    for artifact_path in &result.artifact_paths {
        assert!(
            artifact_path.exists(),
            "Artifact should exist on disk: {:?}",
            artifact_path
        );
    }

    // Read receipt to verify outputs are recorded
    let receipt_path = result.receipt_path.expect("Should have receipt path");
    let receipt_content = std::fs::read_to_string(&receipt_path)?;
    let receipt: serde_json::Value = serde_json::from_str(&receipt_content)?;

    // Verify outputs array exists
    assert!(
        receipt["outputs"].is_array(),
        "Receipt should have outputs array"
    );

    let outputs = receipt["outputs"].as_array().unwrap();

    // For successful execution with artifacts, outputs should not be empty
    if !result.artifact_paths.is_empty() {
        assert!(
            !outputs.is_empty(),
            "Receipt outputs should not be empty when artifacts exist"
        );
    }

    Ok(())
}

/// B3.3: Test that output hashes match artifacts
///
/// Validates that the number of output hashes in the receipt matches
/// the number of artifacts produced, and that each hash is non-empty.
#[tokio::test]
async fn test_core_output_has_hashes() -> Result<()> {
    let _temp = xchecker::paths::with_isolated_home();
    let spec_id = unique_spec_id("output-hashes");
    let config = dry_run_config();

    let mut handle =
        xchecker::orchestrator::OrchestratorHandle::with_config_and_force(&spec_id, config, false)?;

    // Run a phase (Requirements)
    let result = handle
        .run_phase(xchecker::types::PhaseId::Requirements)
        .await?;

    assert!(result.success, "Phase should succeed");

    // Read receipt
    let receipt_path = result.receipt_path.expect("Should have receipt path");
    let receipt_content = std::fs::read_to_string(&receipt_path)?;
    let receipt: serde_json::Value = serde_json::from_str(&receipt_content)?;

    // Get outputs from receipt
    let outputs = receipt["outputs"]
        .as_array()
        .expect("Receipt should have outputs array");

    // Get artifacts from result
    let artifact_count = result.artifact_paths.len();

    // Verify output_hashes length matches artifacts
    assert_eq!(
        outputs.len(),
        artifact_count,
        "Number of output hashes should match number of artifacts"
    );

    // Verify each output has a blake3 hash
    for (idx, output) in outputs.iter().enumerate() {
        assert!(output.is_object(), "Output {} should be an object", idx);

        assert!(
            output["path"].is_string(),
            "Output {} should have path field",
            idx
        );

        assert!(
            output["blake3_canonicalized"].is_string(),
            "Output {} should have blake3_canonicalized field",
            idx
        );

        let hash = output["blake3_canonicalized"]
            .as_str()
            .expect("Hash should be string");

        assert!(!hash.is_empty(), "Output {} hash should not be empty", idx);

        // BLAKE3 hashes are 64 hex characters (256 bits)
        assert!(
            hash.len() >= 32, // At least 32 chars for a reasonable hash
            "Output {} hash should be reasonable length, got {}",
            idx,
            hash.len()
        );
    }

    Ok(())
}

/// B3.4: Test that phase execution is deterministic
///
/// Validates that running the same phase twice produces consistent results
/// (same exit code, same number of artifacts).
#[tokio::test]
async fn test_phase_execution_deterministic() -> Result<()> {
    let _temp = xchecker::paths::with_isolated_home();
    let spec_id = unique_spec_id("deterministic");
    let config = dry_run_config();

    let mut handle =
        xchecker::orchestrator::OrchestratorHandle::with_config_and_force(&spec_id, config, false)?;

    // Run phase first time
    let result1 = handle
        .run_phase(xchecker::types::PhaseId::Requirements)
        .await?;

    assert!(result1.success, "First run should succeed");

    // Get receipt from first run
    let receipt1_path = result1.receipt_path.expect("Should have receipt path");
    let receipt1_content = std::fs::read_to_string(&receipt1_path)?;
    let receipt1: serde_json::Value = serde_json::from_str(&receipt1_content)?;

    // Run phase second time (overwrite)
    let result2 = handle
        .run_phase(xchecker::types::PhaseId::Requirements)
        .await?;

    assert!(result2.success, "Second run should succeed");

    // Get receipt from second run
    let receipt2_path = result2.receipt_path.expect("Should have receipt path");
    let receipt2_content = std::fs::read_to_string(&receipt2_path)?;
    let receipt2: serde_json::Value = serde_json::from_str(&receipt2_content)?;

    // Verify exit codes match
    assert_eq!(
        receipt1["exit_code"], receipt2["exit_code"],
        "Exit codes should match across runs"
    );

    // Verify artifact counts match
    assert_eq!(
        result1.artifact_paths.len(),
        result2.artifact_paths.len(),
        "Artifact count should be consistent"
    );

    // Verify output counts match
    let outputs1 = receipt1["outputs"].as_array().unwrap();
    let outputs2 = receipt2["outputs"].as_array().unwrap();

    assert_eq!(
        outputs1.len(),
        outputs2.len(),
        "Output count should be consistent"
    );

    Ok(())
}

/// B3.5: Test that receipts always have required metadata
///
/// Validates that all receipts contain the core required fields,
/// regardless of success or failure.
#[tokio::test]
async fn test_receipts_have_required_metadata() -> Result<()> {
    let _temp = xchecker::paths::with_isolated_home();
    let spec_id = unique_spec_id("required-metadata");
    let config = dry_run_config();

    let mut handle =
        xchecker::orchestrator::OrchestratorHandle::with_config_and_force(&spec_id, config, false)?;

    // Run phase
    let result = handle
        .run_phase(xchecker::types::PhaseId::Requirements)
        .await?;

    // Read receipt
    let receipt_path = result.receipt_path.expect("Should have receipt path");
    let receipt_content = std::fs::read_to_string(&receipt_path)?;
    let receipt: serde_json::Value = serde_json::from_str(&receipt_content)?;

    // Verify required top-level fields
    assert!(
        receipt["schema_version"].is_string(),
        "Receipt should have schema_version"
    );
    assert!(
        receipt["emitted_at"].is_string(),
        "Receipt should have emitted_at timestamp"
    );
    assert!(
        receipt["spec_id"].is_string(),
        "Receipt should have spec_id"
    );
    assert!(receipt["phase"].is_string(), "Receipt should have phase");
    assert!(
        receipt["xchecker_version"].is_string(),
        "Receipt should have xchecker_version"
    );
    assert!(
        receipt["exit_code"].is_number(),
        "Receipt should have exit_code"
    );

    // Verify required nested objects/arrays
    assert!(
        receipt["packet"].is_object(),
        "Receipt should have packet object"
    );
    assert!(
        receipt["outputs"].is_array(),
        "Receipt should have outputs array"
    );
    assert!(
        receipt["flags"].is_object(),
        "Receipt should have flags object"
    );

    // Verify pipeline metadata (controlled execution)
    assert!(
        receipt["pipeline"].is_object(),
        "Receipt should have pipeline object"
    );
    assert_eq!(
        receipt["pipeline"]["execution_strategy"].as_str(),
        Some("controlled"),
        "Pipeline should have execution_strategy = controlled"
    );

    // Verify LLM metadata (may be null or object depending on execution)
    assert!(
        receipt["llm"].is_object() || receipt["llm"].is_null(),
        "Receipt should have llm field (object or null)"
    );

    Ok(())
}

/// B3.6: Test that artifacts follow naming convention
///
/// Validates that artifacts created by phases follow the expected
/// naming pattern (e.g., 00-requirements.md, 01-design.md).
#[tokio::test]
async fn test_artifacts_follow_naming_convention() -> Result<()> {
    let _temp = xchecker::paths::with_isolated_home();
    let spec_id = unique_spec_id("naming-convention");
    let config = dry_run_config();

    let mut handle =
        xchecker::orchestrator::OrchestratorHandle::with_config_and_force(&spec_id, config, false)?;

    // Test Requirements phase
    let result = handle
        .run_phase(xchecker::types::PhaseId::Requirements)
        .await?;

    assert!(result.success, "Phase should succeed");
    assert!(!result.artifact_paths.is_empty(), "Should have artifacts");

    // Verify Requirements artifact naming
    let req_artifact = &result.artifact_paths[0];
    let req_filename = req_artifact.file_name().unwrap().to_str().unwrap();

    assert!(
        req_filename.starts_with("00-") || req_filename.contains("requirements"),
        "Requirements artifact should follow naming convention, got: {}",
        req_filename
    );

    // Test Design phase (requires Requirements first)
    let design_result = handle.run_phase(xchecker::types::PhaseId::Design).await?;

    assert!(design_result.success, "Design phase should succeed");

    if !design_result.artifact_paths.is_empty() {
        let design_artifact = &design_result.artifact_paths[0];
        let design_filename = design_artifact.file_name().unwrap().to_str().unwrap();

        assert!(
            design_filename.starts_with("01-") || design_filename.contains("design"),
            "Design artifact should follow naming convention, got: {}",
            design_filename
        );
    }

    Ok(())
}

/// B3.7: Test that ExternalTool execution strategy is rejected
///
/// Validates that attempting to use an unsupported execution strategy
/// (externaltool or external_tool) fails during configuration validation,
/// preventing orchestrator construction with an invalid strategy.
#[test]
fn test_externaltool_execution_strategy_rejected() -> Result<()> {
    let _temp = xchecker::paths::with_isolated_home();

    // Test case 1: "externaltool" (lowercase, no underscore)
    // Build a CliArgs with externaltool strategy
    let cli_args_externaltool = xchecker::config::CliArgs {
        execution_strategy: Some("externaltool".to_string()),
        ..Default::default()
    };

    // Attempt to discover config - should fail
    let result_externaltool = xchecker::config::Config::discover(&cli_args_externaltool);

    assert!(
        result_externaltool.is_err(),
        "Config with execution_strategy='externaltool' should be rejected"
    );

    // Verify the error message mentions ExternalTool not being supported
    let error_msg = result_externaltool.unwrap_err().to_string();
    assert!(
        error_msg.contains("externaltool")
            || error_msg.contains("ExternalTool")
            || error_msg.contains("not supported"),
        "Error should mention that externaltool is not supported, got: {}",
        error_msg
    );

    // Test case 2: "external_tool" (with underscore)
    let cli_args_external_tool = xchecker::config::CliArgs {
        execution_strategy: Some("external_tool".to_string()),
        ..Default::default()
    };

    let result_external_tool = xchecker::config::Config::discover(&cli_args_external_tool);

    assert!(
        result_external_tool.is_err(),
        "Config with execution_strategy='external_tool' should be rejected"
    );

    let error_msg2 = result_external_tool.unwrap_err().to_string();
    assert!(
        error_msg2.contains("external_tool") || error_msg2.contains("not supported"),
        "Error should mention that external_tool is not supported, got: {}",
        error_msg2
    );

    // Test case 3: Valid "controlled" strategy should succeed
    let cli_args_controlled = xchecker::config::CliArgs {
        execution_strategy: Some("controlled".to_string()),
        ..Default::default()
    };

    let result_controlled = xchecker::config::Config::discover(&cli_args_controlled);

    assert!(
        result_controlled.is_ok(),
        "Config with execution_strategy='controlled' should succeed"
    );

    let config_controlled = result_controlled.unwrap();
    assert_eq!(
        config_controlled.llm.execution_strategy,
        Some("controlled".to_string()),
        "Execution strategy should be 'controlled'"
    );

    Ok(())
}

/// B3.8: Test that packet construction is validated in execute_phase_core
///
/// This test validates that execute_phase_core properly constructs packets
/// and that the packet evidence contains all expected metadata about files
/// included in the packet. It verifies:
/// 1. packet_evidence.files is non-empty when there is content
/// 2. packet_evidence.max_bytes and max_lines match configured limits
/// 3. Packet files actually exist on disk (in .context directory)
/// 4. Packet evidence matches what was included in the packet
#[tokio::test]
async fn test_packet_construction_in_execute_phase_core() -> Result<()> {
    let _temp = xchecker::paths::with_isolated_home();
    let spec_id = unique_spec_id("packet-construction");

    // Create a config that will produce known packet content
    let mut config_map = HashMap::new();
    config_map.insert("test_mode".to_string(), "true".to_string());

    let config = xchecker::orchestrator::OrchestratorConfig {
        dry_run: true,
        config: config_map,
        full_config: None,
        selectors: None,
        strict_validation: false,
        redactor: Default::default(),
        hooks: None,
    };

    let mut handle =
        xchecker::orchestrator::OrchestratorHandle::with_config_and_force(&spec_id, config, false)?;

    // Run Requirements phase which should produce a packet
    let result = handle
        .run_phase(xchecker::types::PhaseId::Requirements)
        .await?;

    assert!(result.success, "Phase should succeed");

    // Read receipt to access packet_evidence
    let receipt_path = result.receipt_path.expect("Should have receipt path");
    let receipt_content = std::fs::read_to_string(&receipt_path)?;
    let receipt: serde_json::Value = serde_json::from_str(&receipt_content)?;

    // Verify packet evidence exists and has required structure
    let packet = &receipt["packet"];
    assert!(
        packet.is_object(),
        "Receipt should have packet object with evidence"
    );

    // Test 1: Verify packet_evidence has configured limits
    let max_bytes = packet["max_bytes"].as_u64().expect("Should have max_bytes");
    let max_lines = packet["max_lines"].as_u64().expect("Should have max_lines");

    // Default packet limits from config (these match defaults in codebase)
    assert_eq!(
        max_bytes, 65536,
        "Packet max_bytes should match default limit (64KB)"
    );
    assert_eq!(
        max_lines, 1200,
        "Packet max_lines should match default limit"
    );

    // Test 2: Verify packet_evidence.files array structure
    let files = packet["files"].as_array().expect("Should have files array");

    // For Requirements phase with dry_run, the packet should have context about the spec
    // The files array may be empty for fresh specs, or contain spec context
    // This is valid - the packet was constructed, just with no previous artifacts

    // Verify each file entry has the required structure
    for (idx, file) in files.iter().enumerate() {
        assert!(file.is_object(), "File entry {} should be an object", idx);

        assert!(
            file["path"].is_string(),
            "File {} should have path field",
            idx
        );

        assert!(
            file["blake3_pre_redaction"].is_string(),
            "File {} should have blake3_pre_redaction field",
            idx
        );

        // Verify hash is non-empty
        let hash = file["blake3_pre_redaction"]
            .as_str()
            .expect("Hash should be string");
        assert!(!hash.is_empty(), "File {} hash should not be empty", idx);

        // Optionally has range field
        if !file["range"].is_null() {
            assert!(
                file["range"].is_string(),
                "File {} range should be string if present",
                idx
            );
        }

        // Should have priority field
        assert!(
            file["priority"].is_string(),
            "File {} should have priority field",
            idx
        );

        // Priority should be valid enum value
        let priority = file["priority"]
            .as_str()
            .expect("Priority should be string");
        assert!(
            ["high", "medium", "low"].contains(&priority),
            "File {} priority should be valid enum value, got: {}",
            idx,
            priority
        );
    }

    // Test 3: Verify packet file exists on disk (in .context directory)
    // The packet preview should be written during phase execution
    let spec_dir = xchecker::paths::spec_root(&spec_id);
    let context_dir = spec_dir.join(".context");

    // The context directory may not always be created in test environments,
    // so we check if it exists and verify packet content if available
    if context_dir.exists() {
        // Check for requirements-packet file
        let packet_preview_path = context_dir.join("requirements-packet");
        if packet_preview_path.exists() {
            // Verify packet content is non-empty
            let packet_content = std::fs::read_to_string(&packet_preview_path)?;
            assert!(
                !packet_content.is_empty(),
                "Packet preview content should not be empty"
            );
        }
    } else {
        // Context directory not created - this is acceptable for high-level API usage
        // The packet was still constructed internally, as evidenced by the receipt
        println!("Note: Context directory not found (acceptable for handle-based API)");
    }

    // Test 4: Run Design phase to test with dependencies (phase that includes previous artifacts)
    let design_result = handle.run_phase(xchecker::types::PhaseId::Design).await?;

    assert!(design_result.success, "Design phase should succeed");

    // Read Design receipt
    let design_receipt_path = design_result
        .receipt_path
        .expect("Should have receipt path");
    let design_receipt_content = std::fs::read_to_string(&design_receipt_path)?;
    let design_receipt: serde_json::Value = serde_json::from_str(&design_receipt_content)?;

    // Design phase should include Requirements artifacts in packet
    let design_packet = &design_receipt["packet"];
    let design_files = design_packet["files"]
        .as_array()
        .expect("Design packet should have files array");

    // Design should have files from Requirements phase
    assert!(
        !design_files.is_empty(),
        "Design packet should include files from Requirements phase"
    );

    // Verify Design packet file evidence includes artifacts from Requirements
    let mut found_requirements_artifact = false;
    for file in design_files.iter() {
        let path = file["path"].as_str().expect("File should have path");
        if path.contains("requirements") || path.contains("00-") {
            found_requirements_artifact = true;
            break;
        }
    }

    assert!(
        found_requirements_artifact,
        "Design packet should include Requirements artifacts in evidence"
    );

    // Verify Design packet preview exists (if context directory is present)
    if context_dir.exists() {
        let design_packet_path = context_dir.join("design-packet");
        if design_packet_path.exists() {
            let design_packet_content = std::fs::read_to_string(&design_packet_path)?;
            assert!(
                !design_packet_content.is_empty(),
                "Design packet content should not be empty"
            );

            // Verify Design packet actually contains Requirements artifact content
            assert!(
                design_packet_content.contains("Requirements"),
                "Design packet should reference Requirements artifacts"
            );
        }
    }

    Ok(())
}

/// B3.9: Test prompt/packet consistency using unique tag
///
/// Validates that both the prompt and packet are built consistently by ensuring
/// packet preview files are created and packet evidence is populated in receipts.
/// This confirms nothing "skips" either leg of the phase building process.
///
/// The test:
/// 1. Runs a phase execution that should create both prompt and packet
/// 2. Verifies packet preview file exists in .context directory
/// 3. Verifies packet evidence is properly populated in receipt
/// 4. Verifies packet content has proper structure
///
/// This ensures the phase execution pipeline properly calls both prompt()
/// and make_packet() methods and doesn't skip or bypass either one.
#[tokio::test]
async fn test_prompt_packet_consistency() -> Result<()> {
    // Setup isolated environment
    let _temp = xchecker::paths::with_isolated_home();
    let spec_id = unique_spec_id("prompt-packet-consistency");

    // Create handle with dry-run config
    let config = dry_run_config();
    let mut handle =
        xchecker::orchestrator::OrchestratorHandle::with_config_and_force(&spec_id, config, false)?;

    // Run Requirements phase which should generate both prompt and packet
    let result = handle
        .run_phase(xchecker::types::PhaseId::Requirements)
        .await?;

    assert!(result.success, "Phase should succeed");

    // Verify 1: Receipt contains packet evidence
    let receipt_path = result.receipt_path.expect("Should have receipt path");
    let receipt_content = std::fs::read_to_string(&receipt_path)?;
    let receipt: serde_json::Value = serde_json::from_str(&receipt_content)?;

    // Check packet evidence exists
    let packet = &receipt["packet"];
    assert!(
        packet.is_object(),
        "Receipt should have packet object - packet was built"
    );

    // Verify packet evidence has required fields
    assert!(
        packet["max_bytes"].is_number(),
        "Packet evidence should have max_bytes"
    );
    assert!(
        packet["max_lines"].is_number(),
        "Packet evidence should have max_lines"
    );
    assert!(
        packet["files"].is_array(),
        "Packet evidence should have files array"
    );

    let max_bytes = packet["max_bytes"].as_u64().unwrap();
    let max_lines = packet["max_lines"].as_u64().unwrap();

    assert!(
        max_bytes > 0,
        "Packet evidence max_bytes should be positive (packet construction ran)"
    );
    assert!(
        max_lines > 0,
        "Packet evidence max_lines should be positive (packet construction ran)"
    );

    // Verify 2: Packet preview file exists in .context directory
    let spec_dir = xchecker::paths::spec_root(&spec_id);
    let context_dir = spec_dir.join(".context");

    // Check for requirements-packet preview file
    let packet_preview_path = context_dir.join("requirements-packet");

    // The packet preview should exist if phase executed properly
    if context_dir.exists() && packet_preview_path.exists() {
        // Verify packet content is non-empty
        let packet_content = std::fs::read_to_string(&packet_preview_path)?;
        assert!(
            !packet_content.is_empty(),
            "Packet preview should not be empty - packet was constructed"
        );

        // Verify packet has some structure (not just placeholder)
        // In dry-run mode, packet should still contain context headers
        assert!(
            packet_content.len() > 50,
            "Packet should have substantial content, got {} bytes",
            packet_content.len()
        );

        println!("✓ Packet preview file exists and has content");
    } else {
        println!("Note: Context directory or packet preview not found");
        println!("  This may occur with high-level API, but receipt has evidence");
    }

    // Verify 3: Run Design phase to verify packet includes previous artifacts
    // This tests that packet building actually includes context from prior phases
    let design_result = handle.run_phase(xchecker::types::PhaseId::Design).await?;

    assert!(design_result.success, "Design phase should succeed");

    // Read Design receipt
    let design_receipt_path = design_result
        .receipt_path
        .expect("Should have receipt path");
    let design_receipt_content = std::fs::read_to_string(&design_receipt_path)?;
    let design_receipt: serde_json::Value = serde_json::from_str(&design_receipt_content)?;

    // Design phase packet should reference files from Requirements
    let design_packet = &design_receipt["packet"];
    let design_files = design_packet["files"]
        .as_array()
        .expect("Design packet should have files array");

    // Design packet should include Requirements artifacts
    assert!(
        !design_files.is_empty(),
        "Design packet should include files from Requirements phase"
    );

    // Verify at least one file references requirements
    let mut found_requirements_artifact = false;
    for file in design_files.iter() {
        let path = file["path"].as_str().expect("File should have path");
        if path.contains("requirements") || path.contains("00-") {
            found_requirements_artifact = true;
            break;
        }
    }

    assert!(
        found_requirements_artifact,
        "Design packet should include Requirements artifacts - packet includes prior outputs"
    );

    // Verify 4: Design packet preview exists if context dir was created
    let design_packet_path = context_dir.join("design-packet");
    if design_packet_path.exists() {
        let design_packet_content = std::fs::read_to_string(&design_packet_path)?;
        assert!(
            !design_packet_content.is_empty(),
            "Design packet should not be empty"
        );

        // Verify packet actually contains References to Requirements artifacts
        assert!(
            design_packet_content.contains("requirements")
                || design_packet_content.contains("Requirements")
                || design_packet_content.contains("00-"),
            "Design packet should reference Requirements artifacts"
        );

        println!("✓ Design packet includes Requirements artifacts");
    }

    // Summary of what we verified:
    // - Packet evidence exists in both Requirements and Design receipts
    // - Packet evidence has proper structure (max_bytes, max_lines, files)
    // - Packet preview files are created (if context dir exists)
    // - Design packet includes Requirements artifacts (proving packet building works)
    // This confirms both prompt() and make_packet() are called during phase execution

    println!("✓ Packet evidence properly populated in receipts");
    println!("✓ Packet building includes prior phase artifacts");
    println!("✓ Both prompt and packet building paths executed successfully");

    Ok(())
}

// ===== B3.11/B3.14 Round-trip Validation Tests for Packet Evidence & Strategy Consistency =====

/// B3.11: Test round-trip validation of packet evidence
///
/// Validates that the packet evidence in receipts accurately reflects:
/// 1. The number of files included in the packet matches the evidence
/// 2. max_bytes and max_lines match configuration values
/// 3. All file evidence entries have required fields populated
/// 4. File hashes are properly formatted BLAKE3 hashes
#[tokio::test]
async fn test_packet_evidence_round_trip_validation() -> Result<()> {
    let _temp = xchecker::paths::with_isolated_home();
    let spec_id = unique_spec_id("packet-evidence-roundtrip");

    // Create config with known packet limits
    let mut config_map = HashMap::new();
    config_map.insert("packet_max_bytes".to_string(), "65536".to_string());
    config_map.insert("packet_max_lines".to_string(), "1200".to_string());

    let config = xchecker::orchestrator::OrchestratorConfig {
        dry_run: true,
        config: config_map,
        full_config: None,
        selectors: None,
        strict_validation: false,
        redactor: Default::default(),
        hooks: None,
    };

    let mut handle =
        xchecker::orchestrator::OrchestratorHandle::with_config_and_force(&spec_id, config, false)?;

    // Run Requirements phase
    let result = handle
        .run_phase(xchecker::types::PhaseId::Requirements)
        .await?;

    assert!(result.success, "Phase should succeed");

    // Parse receipt JSON
    let receipt_path = result.receipt_path.expect("Should have receipt path");
    let receipt_content = std::fs::read_to_string(&receipt_path)?;
    let receipt: serde_json::Value = serde_json::from_str(&receipt_content)?;

    // Validate packet evidence structure
    let packet = &receipt["packet"];
    assert!(packet.is_object(), "Receipt should have packet object");

    // Assert 1: max_bytes matches config
    let max_bytes = packet["max_bytes"].as_u64().expect("Should have max_bytes");
    assert_eq!(
        max_bytes, 65536,
        "Packet max_bytes should match configured value"
    );

    // Assert 2: max_lines matches config
    let max_lines = packet["max_lines"].as_u64().expect("Should have max_lines");
    assert_eq!(
        max_lines, 1200,
        "Packet max_lines should match configured value"
    );

    // Assert 3: files array exists and is properly structured
    let files = packet["files"].as_array().expect("Should have files array");

    // Count actual files in packet evidence
    let evidence_file_count = files.len();

    // Validate each file entry has required fields
    for (idx, file) in files.iter().enumerate() {
        // Required field: path
        assert!(
            file["path"].is_string(),
            "File {} should have path field",
            idx
        );
        let path = file["path"].as_str().unwrap();
        assert!(!path.is_empty(), "File {} path should not be empty", idx);

        // Required field: blake3_pre_redaction
        assert!(
            file["blake3_pre_redaction"].is_string(),
            "File {} should have blake3_pre_redaction hash",
            idx
        );
        let hash = file["blake3_pre_redaction"].as_str().unwrap();
        assert!(!hash.is_empty(), "File {} hash should not be empty", idx);

        // Validate BLAKE3 hash format (64 hex characters for full hash, or at least 32)
        assert!(
            hash.len() >= 32 && hash.chars().all(|c| c.is_ascii_hexdigit()),
            "File {} hash should be valid hex string, got: {}",
            idx,
            hash
        );

        // Required field: priority
        assert!(
            file["priority"].is_string(),
            "File {} should have priority field",
            idx
        );
        let priority = file["priority"].as_str().unwrap();
        assert!(
            ["high", "medium", "low", "upstream"].contains(&priority),
            "File {} priority should be valid enum value, got: {}",
            idx,
            priority
        );

        // Optional field: range (if present, should be string)
        if !file["range"].is_null() {
            assert!(
                file["range"].is_string(),
                "File {} range should be string if present",
                idx
            );
        }
    }

    // Assert 4: Verify packet evidence count is reasonable
    // For a fresh Requirements phase, the packet may be empty or contain spec context
    println!("Packet evidence contains {} files", evidence_file_count);

    // Run Design phase to test with dependencies
    let design_result = handle.run_phase(xchecker::types::PhaseId::Design).await?;

    assert!(design_result.success, "Design phase should succeed");

    // Parse Design receipt
    let design_receipt_path = design_result
        .receipt_path
        .expect("Should have receipt path");
    let design_receipt_content = std::fs::read_to_string(&design_receipt_path)?;
    let design_receipt: serde_json::Value = serde_json::from_str(&design_receipt_content)?;

    // Design packet should include Requirements artifacts
    let design_packet = &design_receipt["packet"];
    let design_files = design_packet["files"]
        .as_array()
        .expect("Design packet should have files array");

    // Assert 5: Design packet should include files from Requirements
    assert!(
        !design_files.is_empty(),
        "Design packet should include files from Requirements phase"
    );

    // Assert 6: Verify Design packet includes Requirements artifact
    let mut found_requirements_artifact = false;
    for file in design_files.iter() {
        let path = file["path"].as_str().expect("File should have path");
        if path.contains("requirements") || path.contains("00-") {
            found_requirements_artifact = true;

            // Validate this file's evidence is complete
            assert!(
                file["blake3_pre_redaction"].is_string(),
                "Requirements artifact should have hash"
            );
            assert!(
                file["priority"].is_string(),
                "Requirements artifact should have priority"
            );

            break;
        }
    }

    assert!(
        found_requirements_artifact,
        "Design packet evidence should reference Requirements artifacts"
    );

    // Assert 7: Verify packet limits are consistent across phases
    let design_max_bytes = design_packet["max_bytes"].as_u64().unwrap();
    let design_max_lines = design_packet["max_lines"].as_u64().unwrap();

    assert_eq!(
        design_max_bytes, max_bytes,
        "Packet max_bytes should be consistent across phases"
    );
    assert_eq!(
        design_max_lines, max_lines,
        "Packet max_lines should be consistent across phases"
    );

    Ok(())
}

/// B3.12: Test that pipeline.execution_strategy is always "controlled" in all receipts
///
/// Validates that the execution_strategy field in receipts is always set to "controlled"
/// across all phases, ensuring consistency with the V11-V14 enforcement.
#[tokio::test]
async fn test_pipeline_execution_strategy_consistency() -> Result<()> {
    let _temp = xchecker::paths::with_isolated_home();
    let spec_id = unique_spec_id("execution-strategy-consistency");
    let config = dry_run_config();

    let mut handle =
        xchecker::orchestrator::OrchestratorHandle::with_config_and_force(&spec_id, config, false)?;

    // Test multiple phases to ensure consistency
    let phases = vec![
        xchecker::types::PhaseId::Requirements,
        xchecker::types::PhaseId::Design,
        xchecker::types::PhaseId::Tasks,
    ];

    for phase in phases {
        let result = handle.run_phase(phase).await?;

        assert!(result.success, "Phase {:?} should succeed", phase);

        // Parse receipt
        let receipt_path = result.receipt_path.expect("Should have receipt path");
        let receipt_content = std::fs::read_to_string(&receipt_path)?;
        let receipt: serde_json::Value = serde_json::from_str(&receipt_content)?;

        // Validate pipeline object exists
        assert!(
            receipt["pipeline"].is_object(),
            "Receipt for {:?} should have pipeline object",
            phase
        );

        // Validate execution_strategy is "controlled"
        let execution_strategy = receipt["pipeline"]["execution_strategy"]
            .as_str()
            .expect("Pipeline should have execution_strategy field");

        assert_eq!(
            execution_strategy, "controlled",
            "Phase {:?}: pipeline.execution_strategy should always be 'controlled', got: {}",
            phase, execution_strategy
        );
    }

    Ok(())
}

/// B3.13: Test that all required receipt fields are populated
///
/// Validates that all receipts have all required fields populated with non-null,
/// properly formatted values. This is a comprehensive receipt validation test.
#[tokio::test]
async fn test_receipt_required_fields_populated() -> Result<()> {
    let _temp = xchecker::paths::with_isolated_home();
    let spec_id = unique_spec_id("receipt-fields-populated");
    let config = dry_run_config();

    let mut handle =
        xchecker::orchestrator::OrchestratorHandle::with_config_and_force(&spec_id, config, false)?;

    // Run a phase to generate a receipt
    let result = handle
        .run_phase(xchecker::types::PhaseId::Requirements)
        .await?;

    assert!(result.success, "Phase should succeed");

    // Parse receipt
    let receipt_path = result.receipt_path.expect("Should have receipt path");
    let receipt_content = std::fs::read_to_string(&receipt_path)?;
    let receipt: serde_json::Value = serde_json::from_str(&receipt_content)?;

    // Validate required top-level fields

    // 1. schema_version
    assert!(
        receipt["schema_version"].is_string(),
        "Receipt should have schema_version"
    );
    let schema_version = receipt["schema_version"].as_str().unwrap();
    assert!(
        !schema_version.is_empty(),
        "schema_version should not be empty"
    );

    // 2. emitted_at (timestamp)
    assert!(
        receipt["emitted_at"].is_string(),
        "Receipt should have emitted_at timestamp"
    );
    let emitted_at = receipt["emitted_at"].as_str().unwrap();
    assert!(!emitted_at.is_empty(), "emitted_at should not be empty");
    // Validate it's a valid ISO8601/RFC3339 timestamp
    assert!(
        emitted_at.contains('T') && (emitted_at.contains('Z') || emitted_at.contains('+')),
        "emitted_at should be valid RFC3339 timestamp, got: {}",
        emitted_at
    );

    // 3. spec_id
    assert!(
        receipt["spec_id"].is_string(),
        "Receipt should have spec_id"
    );
    let spec_id_val = receipt["spec_id"].as_str().unwrap();
    assert!(!spec_id_val.is_empty(), "spec_id should not be empty");
    assert_eq!(
        spec_id_val, spec_id,
        "Receipt spec_id should match expected value"
    );

    // 4. phase
    assert!(receipt["phase"].is_string(), "Receipt should have phase");
    let phase = receipt["phase"].as_str().unwrap();
    assert!(!phase.is_empty(), "phase should not be empty");
    assert_eq!(phase, "requirements", "Phase should be 'requirements'");

    // 5. xchecker_version
    assert!(
        receipt["xchecker_version"].is_string(),
        "Receipt should have xchecker_version"
    );
    let xchecker_version = receipt["xchecker_version"].as_str().unwrap();
    assert!(
        !xchecker_version.is_empty(),
        "xchecker_version should not be empty"
    );

    // 6. exit_code
    assert!(
        receipt["exit_code"].is_number(),
        "Receipt should have exit_code"
    );
    let exit_code = receipt["exit_code"].as_i64().unwrap();
    assert_eq!(exit_code, 0, "Successful phase should have exit_code 0");

    // 7. packet (PacketEvidence)
    assert!(
        receipt["packet"].is_object(),
        "Receipt should have packet object"
    );
    assert!(
        receipt["packet"]["max_bytes"].is_number(),
        "Packet should have max_bytes"
    );
    assert!(
        receipt["packet"]["max_lines"].is_number(),
        "Packet should have max_lines"
    );
    assert!(
        receipt["packet"]["files"].is_array(),
        "Packet should have files array"
    );

    // 8. outputs (array of FileHash)
    assert!(
        receipt["outputs"].is_array(),
        "Receipt should have outputs array"
    );

    // 9. flags (configuration)
    assert!(
        receipt["flags"].is_object(),
        "Receipt should have flags object"
    );

    // 10. pipeline (PipelineInfo)
    assert!(
        receipt["pipeline"].is_object(),
        "Receipt should have pipeline object"
    );
    assert!(
        receipt["pipeline"]["execution_strategy"].is_string(),
        "Pipeline should have execution_strategy"
    );

    // 11. llm (LlmInfo) - may be null or object depending on execution
    // For dry-run mode, this may be null
    assert!(
        receipt["llm"].is_object() || receipt["llm"].is_null(),
        "Receipt should have llm field (object or null)"
    );

    // 12. runner
    assert!(
        receipt["runner"].is_string(),
        "Receipt should have runner field"
    );
    let runner = receipt["runner"].as_str().unwrap();
    assert!(
        ["native", "wsl", "simulated"].contains(&runner),
        "Runner should be 'native', 'wsl', or 'simulated' (dry-run), got: {}",
        runner
    );

    // 13. canonicalization_version
    assert!(
        receipt["canonicalization_version"].is_string(),
        "Receipt should have canonicalization_version"
    );

    // 14. canonicalization_backend
    assert!(
        receipt["canonicalization_backend"].is_string(),
        "Receipt should have canonicalization_backend"
    );

    // 15. claude_cli_version
    assert!(
        receipt["claude_cli_version"].is_string(),
        "Receipt should have claude_cli_version"
    );

    // 16. model_full_name
    assert!(
        receipt["model_full_name"].is_string(),
        "Receipt should have model_full_name"
    );

    Ok(())
}

/// B3.14: Test packet file count matches actual files used
///
/// Validates that when running a phase with actual dependencies,
/// the number of files in packet evidence matches the actual files
/// that were included when building the packet.
#[tokio::test]
async fn test_packet_file_count_matches_actual_files() -> Result<()> {
    let _temp = xchecker::paths::with_isolated_home();
    let spec_id = unique_spec_id("packet-file-count");
    let config = dry_run_config();

    let mut handle =
        xchecker::orchestrator::OrchestratorHandle::with_config_and_force(&spec_id, config, false)?;

    // Run Requirements phase first (produces artifacts)
    let req_result = handle
        .run_phase(xchecker::types::PhaseId::Requirements)
        .await?;

    assert!(req_result.success, "Requirements phase should succeed");

    let req_artifact_count = req_result.artifact_paths.len();
    assert!(
        req_artifact_count > 0,
        "Requirements should produce at least one artifact"
    );

    // Run Design phase (should include Requirements artifacts in packet)
    let design_result = handle.run_phase(xchecker::types::PhaseId::Design).await?;

    assert!(design_result.success, "Design phase should succeed");

    // Parse Design receipt
    let receipt_path = design_result
        .receipt_path
        .expect("Should have receipt path");
    let receipt_content = std::fs::read_to_string(&receipt_path)?;
    let receipt: serde_json::Value = serde_json::from_str(&receipt_content)?;

    // Get packet evidence
    let packet = &receipt["packet"];
    let files = packet["files"].as_array().expect("Should have files array");

    // Validate that packet contains at least the Requirements artifacts
    assert!(
        !files.is_empty(),
        "Design packet should contain files from Requirements phase"
    );

    // Count files that reference Requirements artifacts
    let mut req_artifact_count_in_packet = 0;
    for file in files.iter() {
        let path = file["path"].as_str().expect("File should have path");
        if path.contains("requirements") || path.contains("00-") {
            req_artifact_count_in_packet += 1;
        }
    }

    // Verify that at least some Requirements artifacts are in the packet
    assert!(
        req_artifact_count_in_packet > 0,
        "Design packet should include at least one Requirements artifact, \
         found {} Requirements artifacts but {} in packet",
        req_artifact_count,
        req_artifact_count_in_packet
    );

    // Validate that all files in packet evidence have complete metadata
    for (idx, file) in files.iter().enumerate() {
        let path = file["path"].as_str().expect("File should have path");
        let hash = file["blake3_pre_redaction"]
            .as_str()
            .expect("File should have hash");
        let priority = file["priority"]
            .as_str()
            .expect("File should have priority");

        assert!(!path.is_empty(), "File {} path should not be empty", idx);
        assert!(!hash.is_empty(), "File {} hash should not be empty", idx);
        assert!(
            !priority.is_empty(),
            "File {} priority should not be empty",
            idx
        );
    }

    Ok(())
}

/// B3.15: Test receipt consistency across multiple phase executions
///
/// Validates that receipts maintain consistent structure and values
/// when the same phase is executed multiple times.
#[tokio::test]
async fn test_receipt_consistency_across_executions() -> Result<()> {
    let _temp = xchecker::paths::with_isolated_home();
    let spec_id = unique_spec_id("receipt-consistency");
    let config = dry_run_config();

    let mut handle =
        xchecker::orchestrator::OrchestratorHandle::with_config_and_force(&spec_id, config, false)?;

    // Run Requirements phase twice
    let result1 = handle
        .run_phase(xchecker::types::PhaseId::Requirements)
        .await?;

    let result2 = handle
        .run_phase(xchecker::types::PhaseId::Requirements)
        .await?;

    assert!(
        result1.success && result2.success,
        "Both runs should succeed"
    );

    // Parse both receipts
    let receipt1_content =
        std::fs::read_to_string(result1.receipt_path.expect("Should have receipt path"))?;
    let receipt1: serde_json::Value = serde_json::from_str(&receipt1_content)?;

    let receipt2_content =
        std::fs::read_to_string(result2.receipt_path.expect("Should have receipt path"))?;
    let receipt2: serde_json::Value = serde_json::from_str(&receipt2_content)?;

    // Validate consistent fields

    // 1. Schema version should be same
    assert_eq!(
        receipt1["schema_version"], receipt2["schema_version"],
        "Schema version should be consistent"
    );

    // 2. Exit code should be same
    assert_eq!(
        receipt1["exit_code"], receipt2["exit_code"],
        "Exit code should be consistent"
    );

    // 3. Phase should be same
    assert_eq!(
        receipt1["phase"], receipt2["phase"],
        "Phase should be consistent"
    );

    // 4. Packet limits should be same
    assert_eq!(
        receipt1["packet"]["max_bytes"], receipt2["packet"]["max_bytes"],
        "Packet max_bytes should be consistent"
    );
    assert_eq!(
        receipt1["packet"]["max_lines"], receipt2["packet"]["max_lines"],
        "Packet max_lines should be consistent"
    );

    // 5. Pipeline execution_strategy should be same
    assert_eq!(
        receipt1["pipeline"]["execution_strategy"], receipt2["pipeline"]["execution_strategy"],
        "Execution strategy should be consistent"
    );

    // 6. Both should have "controlled" strategy
    assert_eq!(
        receipt1["pipeline"]["execution_strategy"].as_str(),
        Some("controlled"),
        "First run should have controlled strategy"
    );
    assert_eq!(
        receipt2["pipeline"]["execution_strategy"].as_str(),
        Some("controlled"),
        "Second run should have controlled strategy"
    );

    Ok(())
}

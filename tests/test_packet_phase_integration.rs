//! Integration tests for PacketBuilder and Phase system wiring (Task 9.2)
//!
//! **WHITE-BOX TEST**: This test uses internal module APIs (`orchestrator::{OrchestratorConfig,
//! PhaseOrchestrator}`, `types::PhaseId`) and may break with internal refactors. These tests are
//! intentionally white-box to validate internal implementation details. See FR-TEST-4 for
//! white-box test policy.
//!
//! Tests the complete integration of PacketBuilder, Phase trait system,
//! and orchestrator with proper packet evidence tracking.

use anyhow::Result;
use std::fs;
use tempfile::TempDir;
use xchecker::orchestrator::{OrchestratorConfig, PhaseOrchestrator};
use xchecker::test_support;
use xchecker::types::PhaseId;

/// Helper to set up test environment with sample files
fn setup_test_environment_with_files(test_name: &str) -> (PhaseOrchestrator, TempDir) {
    // Use isolated home for each test to avoid conflicts
    let temp_dir = xchecker::paths::with_isolated_home();

    // Create spec directory structure
    let spec_id = format!("test-packet-phase-{}", test_name);
    let orchestrator = PhaseOrchestrator::new(&spec_id).unwrap();

    // Create some sample files in the spec directory for packet building
    let spec_dir = orchestrator.artifact_manager().base_path();

    // Create a README file (medium priority)
    fs::write(
        spec_dir.join("README.md"),
        "# Test Spec\n\nThis is a test specification for packet building.",
    )
    .unwrap();

    // Create a SPEC file (high priority)
    fs::write(
        spec_dir.join("SPEC-001.md"),
        "# Specification Document\n\nDetailed specification content.",
    )
    .unwrap();

    // Create a core YAML file (upstream priority - non-evictable)
    fs::write(
        spec_dir.join("config.core.yaml"),
        "version: 1.0\nname: test-spec\n",
    )
    .unwrap();

    (orchestrator, temp_dir)
}

/// Test that PacketBuilder is properly integrated into phase execution
#[tokio::test]
async fn test_packet_builder_integration() -> Result<()> {
    let (orchestrator, _temp_dir) = setup_test_environment_with_files("builder-integration");

    let config = OrchestratorConfig {
        dry_run: true,
        ..Default::default()
    };

    // Execute Requirements phase
    let result = orchestrator.execute_requirements_phase(&config).await?;

    assert!(result.success, "Requirements phase should succeed");
    assert_eq!(result.exit_code, 0, "Exit code should be 0 for success");

    // Verify receipt was written
    assert!(result.receipt_path.is_some(), "Receipt should be written");

    // Read the receipt and verify packet evidence is populated
    let receipt_path = result.receipt_path.unwrap();
    let receipt_content = fs::read_to_string(&receipt_path)?;

    // Receipt should contain packet evidence
    assert!(
        receipt_content.contains("packet_evidence") || receipt_content.contains("files"),
        "Receipt should contain packet evidence"
    );

    Ok(())
}

/// Test that packet evidence includes file information
#[tokio::test]
async fn test_packet_evidence_includes_files() -> Result<()> {
    let (orchestrator, _temp_dir) = setup_test_environment_with_files("evidence-files");

    let config = OrchestratorConfig {
        dry_run: true,
        ..Default::default()
    };

    // Execute Requirements phase
    let result = orchestrator.execute_requirements_phase(&config).await?;
    assert!(result.success);

    // Read the receipt
    let receipt_manager =
        xchecker::receipt::ReceiptManager::new(orchestrator.artifact_manager().base_path());

    let receipt = receipt_manager.read_latest_receipt(PhaseId::Requirements)?;
    assert!(receipt.is_some(), "Receipt should exist");

    let receipt = receipt.unwrap();

    // Verify packet evidence has files
    assert!(
        !receipt.packet.files.is_empty(),
        "Packet evidence should include files"
    );

    // Verify files have required fields
    for file_evidence in &receipt.packet.files {
        assert!(
            !file_evidence.path.is_empty(),
            "File path should not be empty"
        );
        assert!(
            !file_evidence.blake3_pre_redaction.is_empty(),
            "BLAKE3 hash should not be empty"
        );
        // Priority should be set
        assert!(
            matches!(
                file_evidence.priority,
                xchecker::types::Priority::Upstream
                    | xchecker::types::Priority::High
                    | xchecker::types::Priority::Medium
                    | xchecker::types::Priority::Low
            ),
            "Priority should be valid"
        );
    }

    Ok(())
}

/// Test that secret scanning is integrated before Claude invocation
#[tokio::test]
async fn test_secret_scanning_integration() -> Result<()> {
    let (orchestrator, _temp_dir) = setup_test_environment_with_files("secret-scan");

    // Create a file with a fake secret
    let spec_dir = orchestrator.artifact_manager().base_path();
    let token = test_support::github_pat();
    fs::write(spec_dir.join("secrets.txt"), format!("API Key: {}", token))?;

    let config = OrchestratorConfig {
        dry_run: true, // Use dry_run to avoid nested runtime issues
        ..Default::default()
    };

    // Execute Requirements phase - should fail due to secret detection
    let result = orchestrator.execute_requirements_phase(&config).await;

    // Should fail with secret detection error
    assert!(
        result.is_err(),
        "Phase should fail when secrets are detected"
    );

    let err = result.unwrap_err();
    let err_msg = err.to_string();

    // Error should mention secret detection
    assert!(
        err_msg.contains("secret") || err_msg.contains("Secret"),
        "Error should mention secret detection: {}",
        err_msg
    );

    Ok(())
}

/// Test that packet overflow is detected before Claude invocation
/// Note: This test is currently skipped because packet limit configuration
/// needs to be wired through the phase context to the PacketBuilder
#[tokio::test]
#[ignore = "requires_future_api"]
async fn test_packet_overflow_detection() -> Result<()> {
    let (orchestrator, _temp_dir) = setup_test_environment_with_files("overflow");

    // Create a very large file that will exceed packet limits
    let spec_dir = orchestrator.artifact_manager().base_path();
    let large_content = "x".repeat(100_000); // 100KB of content
    fs::write(spec_dir.join("large.txt"), &large_content)?;

    let mut config = OrchestratorConfig {
        dry_run: true,
        ..Default::default()
    };

    // Set very small packet limits to trigger overflow
    config
        .config
        .insert("packet_max_bytes".to_string(), "1000".to_string());
    config
        .config
        .insert("packet_max_lines".to_string(), "10".to_string());

    // Execute Requirements phase - should fail due to packet overflow
    let result = orchestrator.execute_requirements_phase(&config).await;

    // Should fail with packet overflow error
    assert!(
        result.is_err(),
        "Phase should fail when packet exceeds limits"
    );

    let err = result.unwrap_err();
    let err_msg = err.to_string();

    // Error should mention packet overflow
    assert!(
        err_msg.contains("overflow")
            || err_msg.contains("Overflow")
            || err_msg.contains("exceeded")
            || err_msg.contains("limit"),
        "Error should mention packet overflow: {}",
        err_msg
    );

    Ok(())
}

/// Test end-to-end Requirements -> Design -> Tasks progression
#[tokio::test]
async fn test_end_to_end_phase_progression() -> Result<()> {
    let (orchestrator, _temp_dir) = setup_test_environment_with_files("e2e-progression");

    let config = OrchestratorConfig {
        dry_run: true,
        ..Default::default()
    };

    // Execute Requirements phase
    let result1 = orchestrator.execute_requirements_phase(&config).await?;
    assert!(result1.success, "Requirements phase should succeed");
    assert!(
        !result1.artifact_paths.is_empty(),
        "Should create artifacts"
    );

    // Verify Requirements artifacts were created
    assert!(
        result1
            .artifact_paths
            .iter()
            .any(|p| p.to_string_lossy().contains("requirements")),
        "Should create requirements artifacts"
    );

    // Execute Design phase
    let result2 = orchestrator.execute_design_phase(&config).await?;
    assert!(result2.success, "Design phase should succeed");
    assert!(
        !result2.artifact_paths.is_empty(),
        "Should create artifacts"
    );

    // Verify Design artifacts were created
    assert!(
        result2
            .artifact_paths
            .iter()
            .any(|p| p.to_string_lossy().contains("design")),
        "Should create design artifacts"
    );

    // Execute Tasks phase
    let result3 = orchestrator.execute_tasks_phase(&config).await?;
    assert!(result3.success, "Tasks phase should succeed");
    assert!(
        !result3.artifact_paths.is_empty(),
        "Should create artifacts"
    );

    // Verify Tasks artifacts were created
    assert!(
        result3
            .artifact_paths
            .iter()
            .any(|p| p.to_string_lossy().contains("tasks")),
        "Should create tasks artifacts"
    );

    Ok(())
}

/// Test that phase dependency enforcement works
#[tokio::test]
async fn test_phase_dependency_enforcement() -> Result<()> {
    let (orchestrator, _temp_dir) = setup_test_environment_with_files("dependency");

    let config = OrchestratorConfig {
        dry_run: true,
        ..Default::default()
    };

    // Try to execute Design without Requirements - should fail
    let result = orchestrator.execute_design_phase(&config).await;
    assert!(result.is_err(), "Design should fail without Requirements");

    // Execute Requirements first
    orchestrator.execute_requirements_phase(&config).await?;

    // Now Design should succeed
    let result = orchestrator.execute_design_phase(&config).await;
    assert!(result.is_ok(), "Design should succeed after Requirements");

    // Try to execute Tasks without Design - should fail
    // (Requirements is done but Design is not)
    // Actually, we just executed Design above, so let's test a different scenario

    // Create a new orchestrator for a fresh test
    let (orchestrator2, _temp_dir2) = setup_test_environment_with_files("dependency2");

    // Execute only Requirements
    orchestrator2.execute_requirements_phase(&config).await?;

    // Try to execute Tasks without Design - should fail
    let result = orchestrator2.execute_tasks_phase(&config).await;
    assert!(result.is_err(), "Tasks should fail without Design");

    Ok(())
}

/// Test that packet preview is written to context directory
#[tokio::test]
async fn test_packet_preview_written() -> Result<()> {
    let (orchestrator, _temp_dir) = setup_test_environment_with_files("preview");

    let config = OrchestratorConfig {
        dry_run: true,
        ..Default::default()
    };

    // Execute Requirements phase
    orchestrator.execute_requirements_phase(&config).await?;

    // Check that packet preview was written
    let context_dir = orchestrator.artifact_manager().context_path();
    let preview_path = context_dir.join("requirements-packet.txt");

    assert!(
        preview_path.exists(),
        "Packet preview should be written to context directory"
    );

    // Verify preview contains file content markers
    let preview_content = fs::read_to_string(&preview_path)?;
    assert!(
        preview_content.contains("===") || !preview_content.is_empty(),
        "Packet preview should contain content"
    );

    Ok(())
}

/// Test that artifacts are generated (markdown + core YAML)
#[tokio::test]
async fn test_artifacts_generated() -> Result<()> {
    let (orchestrator, _temp_dir) = setup_test_environment_with_files("artifacts");

    let config = OrchestratorConfig {
        dry_run: true,
        ..Default::default()
    };

    // Execute Requirements phase
    let result = orchestrator.execute_requirements_phase(&config).await?;

    // Verify both markdown and YAML artifacts were created
    let has_markdown = result
        .artifact_paths
        .iter()
        .any(|p| p.to_string_lossy().ends_with(".md"));
    let has_yaml = result
        .artifact_paths
        .iter()
        .any(|p| p.to_string_lossy().ends_with(".yaml"));

    assert!(has_markdown, "Should create markdown artifact");
    assert!(has_yaml, "Should create YAML artifact");

    // Verify artifacts exist on disk
    for artifact_path in &result.artifact_paths {
        assert!(
            artifact_path.exists(),
            "Artifact should exist: {:?}",
            artifact_path
        );
    }

    Ok(())
}

/// Test that receipts include accurate packet evidence
#[tokio::test]
async fn test_receipt_packet_evidence_accuracy() -> Result<()> {
    let (orchestrator, _temp_dir) = setup_test_environment_with_files("evidence-accuracy");

    let config = OrchestratorConfig {
        dry_run: true,
        ..Default::default()
    };

    // Execute Requirements phase
    orchestrator.execute_requirements_phase(&config).await?;

    // Read the receipt
    let receipt_manager =
        xchecker::receipt::ReceiptManager::new(orchestrator.artifact_manager().base_path());

    let receipt = receipt_manager
        .read_latest_receipt(PhaseId::Requirements)?
        .expect("Receipt should exist");

    // Verify packet evidence has correct structure
    assert!(receipt.packet.max_bytes > 0, "Max bytes should be set");
    assert!(receipt.packet.max_lines > 0, "Max lines should be set");

    // Verify files in evidence match files in spec directory
    let expected_files = vec!["README.md", "SPEC-001.md", "config.core.yaml"];

    for expected_file in expected_files {
        let found = receipt
            .packet
            .files
            .iter()
            .any(|f| f.path.contains(expected_file));
        assert!(
            found,
            "Packet evidence should include file: {}",
            expected_file
        );
    }

    Ok(())
}

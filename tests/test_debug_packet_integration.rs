#![cfg(feature = "test-utils")]
//! Integration tests for --debug-packet flag behavior (FR-PKT-006, FR-PKT-007)
//!
//! **WHITE-BOX TEST**: This test uses internal module APIs (`orchestrator::{OrchestratorConfig,
//! PhaseOrchestrator}`, `paths`) and may break with internal refactors. These tests are
//! intentionally white-box to validate internal implementation details. See FR-TEST-4 for
//! white-box test policy.
//!
//! This test suite verifies that:
//! - Debug packet is written when --debug-packet flag is set and secret scan passes
//! - Debug packet is NOT written if secrets are detected
//! - Debug packet file is excluded from receipts
//! - Debug packet content is redacted if later reported

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use xchecker::orchestrator::{OrchestratorConfig, PhaseOrchestrator};
use xchecker::paths;
use xchecker::test_support;

/// Test that debug packet is written when --debug-packet flag is set (FR-PKT-006)
#[tokio::test]
async fn test_debug_packet_written_with_flag() -> Result<()> {
    // Use thread-local isolated home to prevent cross-test contamination
    let _home_guard = paths::with_isolated_home();
    let spec_id = "test-debug-packet-enabled";

    // Create a source file in the spec's context directory so the packet has content
    let spec_context_dir = paths::spec_root(spec_id).join("context");
    fs::create_dir_all(&spec_context_dir)?;
    fs::write(
        spec_context_dir.join("problem-statement.md"),
        "# Problem Statement\n\nThis is a test problem statement for the debug packet test.",
    )?;

    // Create orchestrator
    let orchestrator = PhaseOrchestrator::new(spec_id)?;

    // Create config with debug_packet enabled
    let mut config_map = HashMap::new();
    config_map.insert("debug_packet".to_string(), "true".to_string());

    let config = OrchestratorConfig {
        dry_run: true, // Use dry-run to avoid actual Claude invocation
        config: config_map,
        full_config: None,
        selectors: None,
        strict_validation: false,
        redactor: Default::default(),
        hooks: None,
    };

    // Execute requirements phase
    let result = orchestrator.execute_requirements_phase(&config).await;

    // Verify execution succeeded
    assert!(result.is_ok(), "Phase execution should succeed");
    let execution_result = result.unwrap();
    assert!(
        execution_result.success,
        "Phase should complete successfully"
    );

    // Verify debug packet file was written
    let debug_packet_path = spec_context_dir.join("requirements-packet-debug.txt");

    assert!(
        debug_packet_path.exists(),
        "Debug packet file should exist at: {debug_packet_path:?}"
    );

    // Verify debug packet contains content (at minimum the problem statement)
    let debug_content = fs::read_to_string(&debug_packet_path)?;
    assert!(
        !debug_content.is_empty(),
        "Debug packet should contain content"
    );

    Ok(())
}

/// Test that debug packet is NOT written when flag is not set (FR-PKT-006)
#[tokio::test]
async fn test_debug_packet_not_written_without_flag() -> Result<()> {
    // Use thread-local isolated home to prevent cross-test contamination
    let _home_guard = paths::with_isolated_home();
    let spec_id = "test-debug-packet-disabled";

    // Create orchestrator
    let orchestrator = PhaseOrchestrator::new(spec_id)?;

    // Create config WITHOUT debug_packet flag
    let config = OrchestratorConfig {
        dry_run: true,
        config: HashMap::new(),
        full_config: None,
        selectors: None,
        strict_validation: false,
        redactor: Default::default(),
        hooks: None,
    };

    // Execute requirements phase
    let result = orchestrator.execute_requirements_phase(&config).await;

    // Verify execution succeeded
    assert!(result.is_ok(), "Phase execution should succeed");

    // Verify debug packet file was NOT written
    let debug_packet_path = paths::spec_root(spec_id)
        .join("context")
        .join("requirements-packet-debug.txt");

    assert!(
        !debug_packet_path.exists(),
        "Debug packet file should NOT exist when flag is not set"
    );

    Ok(())
}

/// Test that debug packet is NOT written if secrets are detected (FR-PKT-007)
#[tokio::test]
async fn test_debug_packet_not_written_on_secret_detection() -> Result<()> {
    // Use thread-local isolated home to prevent cross-test contamination
    let _home_guard = paths::with_isolated_home();
    let spec_id = "test-debug-packet-secret";

    // Create a file containing a secret in the spec's context directory
    // This is where the packet builder looks for files to include
    let spec_context_dir = paths::spec_root(spec_id).join("context");
    fs::create_dir_all(&spec_context_dir)?;
    let token = test_support::github_pat();
    fs::write(
        spec_context_dir.join("problem-statement.md"),
        format!(
            "# Problem Statement\n\nThis file contains a secret: {}",
            token
        ),
    )?;

    // Create orchestrator
    let orchestrator = PhaseOrchestrator::new(spec_id)?;

    // Create config with debug_packet enabled
    let mut config_map = HashMap::new();
    config_map.insert("debug_packet".to_string(), "true".to_string());

    let config = OrchestratorConfig {
        dry_run: true,
        config: config_map,
        full_config: None,
        selectors: None,
        strict_validation: false,
        redactor: Default::default(),
        hooks: None,
    };

    // Execute requirements phase (will fail due to secret detection)
    let result = orchestrator.execute_requirements_phase(&config).await;

    // Secret detection returns an error from the orchestrator
    // The key assertion is that the debug packet file was NOT written
    // before the secret was detected

    // Verify debug packet file was NOT written (secret scan failed before debug packet could be written)
    let debug_packet_path = spec_context_dir.join("requirements-packet-debug.txt");

    assert!(
        !debug_packet_path.exists(),
        "Debug packet file should NOT exist when secrets are detected"
    );

    // The result should be an error (secret detection fails the phase)
    // This is the expected behavior - secrets cause the phase to fail
    assert!(
        result.is_err(),
        "Secret detection should cause the phase to fail with an error"
    );

    Ok(())
}

/// Test that debug packet file is not cross-linked in receipts (FR-PKT-007)
#[tokio::test]
async fn test_debug_packet_not_in_receipts() -> Result<()> {
    // Use thread-local isolated home to prevent cross-test contamination
    let _home_guard = paths::with_isolated_home();
    let spec_id = "test-debug-packet-receipt";

    // Create a source file in the spec's context directory so the packet has content
    let spec_context_dir = paths::spec_root(spec_id).join("context");
    fs::create_dir_all(&spec_context_dir)?;
    fs::write(
        spec_context_dir.join("problem-statement.md"),
        "# Problem Statement\n\nThis is a test problem statement.",
    )?;

    // Create orchestrator
    let orchestrator = PhaseOrchestrator::new(spec_id)?;

    // Create config with debug_packet enabled
    let mut config_map = HashMap::new();
    config_map.insert("debug_packet".to_string(), "true".to_string());

    let config = OrchestratorConfig {
        dry_run: true,
        config: config_map,
        full_config: None,
        selectors: None,
        strict_validation: false,
        redactor: Default::default(),
        hooks: None,
    };

    // Execute requirements phase
    let result = orchestrator.execute_requirements_phase(&config).await;

    // Verify execution succeeded
    assert!(result.is_ok(), "Phase execution should succeed");
    let execution_result = result.unwrap();
    assert!(
        execution_result.success,
        "Phase should complete successfully"
    );

    // Read the receipt
    let receipt_path = execution_result.receipt_path.expect("Receipt should exist");
    let receipt_content = fs::read_to_string(&receipt_path)?;

    // Verify receipt does NOT mention the debug packet file specifically
    // Note: We only check for the specific debug packet filename, not the word "debug"
    // because the receipt may contain "debug" in other contexts (like dry_run extensions)
    assert!(
        !receipt_content.contains("packet-debug.txt"),
        "Receipt should not reference debug packet file"
    );
    assert!(
        !receipt_content.contains("requirements-packet-debug"),
        "Receipt should not reference debug packet file path"
    );

    Ok(())
}

/// Test that regular packet preview is still written (FR-PKT)
#[tokio::test]
async fn test_packet_preview_always_written() -> Result<()> {
    // Use thread-local isolated home to prevent cross-test contamination
    let _home_guard = paths::with_isolated_home();
    let spec_id = "test-packet-preview";

    // Create orchestrator
    let orchestrator = PhaseOrchestrator::new(spec_id)?;

    // Create config WITHOUT debug_packet flag
    let config = OrchestratorConfig {
        dry_run: true,
        config: HashMap::new(),
        full_config: None,
        selectors: None,
        strict_validation: false,
        redactor: Default::default(),
        hooks: None,
    };

    // Execute requirements phase
    let result = orchestrator.execute_requirements_phase(&config).await;

    // Verify execution succeeded
    assert!(result.is_ok(), "Phase execution should succeed");

    // Verify regular packet preview file was written (not debug packet)
    let preview_packet_path = paths::spec_root(spec_id)
        .join("context")
        .join("requirements-packet.txt");

    assert!(
        preview_packet_path.exists(),
        "Regular packet preview should always be written"
    );

    // Verify debug packet was NOT written
    let debug_packet_path = paths::spec_root(spec_id)
        .join("context")
        .join("requirements-packet-debug.txt");

    assert!(
        !debug_packet_path.exists(),
        "Debug packet should not be written without flag"
    );

    Ok(())
}

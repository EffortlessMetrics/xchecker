//! Comprehensive tests for status reporting (FR-STA)
//!
//! **WHITE-BOX TEST**: This test uses internal module APIs (`artifact::ArtifactManager`,
//! `receipt::ReceiptManager`, `status::StatusManager`, `types::{...}`) and may break with
//! internal refactors. These tests are intentionally white-box to validate internal
//! implementation details. See FR-TEST-4 for white-box test policy.
//!
//! This module tests all aspects of status generation and reporting:
//! - FR-STA-001: Status generation with effective_config
//! - FR-STA-002: Source attribution (cli/config/default)
//! - FR-STA-003: Artifact enumeration with blake3 hashes
//! - FR-STA-004: Fresh spec (no prior receipts)
//! - FR-STA-005: Lock drift reporting
//! - JCS emission for status outputs

use anyhow::Result;
use std::collections::BTreeMap;
use std::collections::HashMap;
use xchecker::artifact::ArtifactManager;
use xchecker::paths;
use xchecker::receipt::ReceiptManager;
use xchecker::status::StatusManager;
use xchecker::types::{ConfigSource, DriftPair, FileHash, LockDrift, PacketEvidence, PhaseId};

/// Helper to generate status using the internal method
fn generate_status(
    artifact_manager: &ArtifactManager,
    receipt_manager: &ReceiptManager,
    effective_config: BTreeMap<String, (String, String)>,
    lock_drift: Option<LockDrift>,
) -> Result<xchecker::types::StatusOutput> {
    StatusManager::generate_status_internal(
        artifact_manager,
        receipt_manager,
        effective_config,
        lock_drift,
        None, // pending_fixups
        None, // secret_redactor
    )
}

/// Helper to create test managers with isolated environment
fn create_test_managers(spec_id: &str) -> (ArtifactManager, ReceiptManager, tempfile::TempDir) {
    let temp_dir = paths::with_isolated_home();
    let artifact_manager = ArtifactManager::new(spec_id).unwrap();
    let base_path = paths::spec_root(spec_id);
    let receipt_manager = ReceiptManager::new(&base_path);
    (artifact_manager, receipt_manager, temp_dir)
}

/// Helper to create a test receipt
fn create_test_receipt(
    receipt_manager: &ReceiptManager,
    spec_id: &str,
    phase: PhaseId,
    runner: &str,
    runner_distro: Option<String>,
) {
    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let receipt = receipt_manager.create_receipt(
        spec_id,
        phase,
        0,
        vec![],
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        HashMap::new(),
        packet,
        None,          // stderr_tail
        None,          // stderr_redacted
        vec![],        // warnings
        Some(false),   // fallback_used
        runner,        // runner
        runner_distro, // runner_distro
        None,          // error_kind
        None,          // error_reason
        None,          // diff_context,
        None,          // pipeline
    );

    receipt_manager.write_receipt(&receipt).unwrap();
}

/// Test FR-STA-001: Status generation with effective_config
#[test]
fn test_status_generation_with_effective_config() -> Result<()> {
    let (artifact_manager, receipt_manager, _temp_dir) =
        create_test_managers("test-status-effective-config");

    // Create a test receipt
    create_test_receipt(
        &receipt_manager,
        "test-status-effective-config",
        PhaseId::Requirements,
        "native",
        None,
    );

    // Create effective config with multiple sources
    let mut effective_config = BTreeMap::new();
    effective_config.insert(
        "model".to_string(),
        ("haiku".to_string(), "defaults".to_string()),
    );
    effective_config.insert(
        "max_turns".to_string(),
        ("10".to_string(), "CLI".to_string()),
    );
    effective_config.insert(
        "timeout".to_string(),
        ("600".to_string(), "config file".to_string()),
    );

    // Generate status
    let status = generate_status(&artifact_manager, &receipt_manager, effective_config, None)?;

    // Verify effective_config is populated
    assert!(
        status.effective_config.len() >= 3,
        "Should have at least 3 config values"
    );

    // Verify config values are present
    assert!(status.effective_config.contains_key("model"));
    assert!(status.effective_config.contains_key("max_turns"));
    assert!(status.effective_config.contains_key("timeout"));

    println!("✓ Status generation with effective_config test passed");
    Ok(())
}

/// Test FR-STA-002: Source attribution (cli/config/default)
#[test]
fn test_source_attribution() -> Result<()> {
    let (artifact_manager, receipt_manager, _temp_dir) =
        create_test_managers("test-status-source-attribution");

    // Create a test receipt
    create_test_receipt(
        &receipt_manager,
        "test-status-source-attribution",
        PhaseId::Requirements,
        "native",
        None,
    );

    // Create effective config with different sources
    let mut effective_config = BTreeMap::new();
    effective_config.insert(
        "model".to_string(),
        ("haiku".to_string(), "defaults".to_string()),
    );
    effective_config.insert(
        "max_turns".to_string(),
        ("10".to_string(), "CLI".to_string()),
    );
    effective_config.insert(
        "timeout".to_string(),
        ("600".to_string(), "config file".to_string()),
    );

    // Generate status
    let status = generate_status(&artifact_manager, &receipt_manager, effective_config, None)?;

    // Verify source attribution
    let model_config = status.effective_config.get("model").unwrap();
    assert_eq!(
        model_config.source,
        ConfigSource::Default,
        "Model should be from defaults"
    );

    let max_turns_config = status.effective_config.get("max_turns").unwrap();
    assert_eq!(
        max_turns_config.source,
        ConfigSource::Cli,
        "Max turns should be from CLI"
    );

    let timeout_config = status.effective_config.get("timeout").unwrap();
    assert_eq!(
        timeout_config.source,
        ConfigSource::Config,
        "Timeout should be from config file"
    );

    // Verify values are correct types
    assert_eq!(model_config.value.as_str().unwrap(), "haiku");
    assert_eq!(max_turns_config.value.as_i64().unwrap(), 10);
    assert_eq!(timeout_config.value.as_i64().unwrap(), 600);

    println!("✓ Source attribution test passed");
    Ok(())
}

/// Test FR-STA-003: Artifact enumeration with blake3 hashes
#[test]
fn test_artifact_enumeration_with_blake3() -> Result<()> {
    let (artifact_manager, receipt_manager, _temp_dir) =
        create_test_managers("test-status-artifacts");

    // Create a test receipt with outputs
    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let outputs = vec![
        FileHash {
            path: "artifacts/00-requirements.md".to_string(),
            blake3_canonicalized:
                "abcd1234567890abcdef1234567890abcdef1234567890abcdef1234567890ab".to_string(),
        },
        FileHash {
            path: "artifacts/00-requirements.core.yaml".to_string(),
            blake3_canonicalized:
                "1234abcd567890abcdef1234567890abcdef1234567890abcdef1234567890cd".to_string(),
        },
    ];

    let receipt = receipt_manager.create_receipt(
        "test-status-artifacts",
        PhaseId::Requirements,
        0,
        outputs,
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        HashMap::new(),
        packet,
        None,        // stderr_tail
        None,        // stderr_redacted
        vec![],      // warnings
        Some(false), // fallback_used
        "native",    // runner
        None,        // runner_distro
        None,        // error_kind
        None,        // error_reason
        None,        // diff_context,
        None,        // pipeline
    );

    receipt_manager.write_receipt(&receipt)?;

    // Create the actual artifact files so they can be enumerated
    let artifacts_dir = artifact_manager.base_path().join("artifacts");
    std::fs::create_dir_all(&artifacts_dir)?;
    std::fs::write(artifacts_dir.join("00-requirements.md"), "# Requirements\n")?;
    std::fs::write(
        artifacts_dir.join("00-requirements.core.yaml"),
        "version: 1\n",
    )?;

    // Create effective config
    let mut effective_config = BTreeMap::new();
    effective_config.insert(
        "model".to_string(),
        ("haiku".to_string(), "defaults".to_string()),
    );

    // Generate status
    let status = generate_status(&artifact_manager, &receipt_manager, effective_config, None)?;

    // Verify artifacts are enumerated
    assert_eq!(
        status.artifacts.len(),
        2,
        "Should have 2 artifacts enumerated"
    );

    // Verify artifacts are sorted by path
    assert_eq!(
        status.artifacts[0].path,
        "artifacts/00-requirements.core.yaml"
    );
    assert_eq!(status.artifacts[1].path, "artifacts/00-requirements.md");

    // Verify blake3_first8 is populated (first 8 chars of hash)
    assert_eq!(status.artifacts[0].blake3_first8, "1234abcd");
    assert_eq!(status.artifacts[1].blake3_first8, "abcd1234");

    println!("✓ Artifact enumeration with blake3 hashes test passed");
    Ok(())
}

/// Test FR-STA-004: Fresh spec (no prior receipts)
///
/// Fresh specs with no receipts should return a valid status with sensible defaults.
/// This allows users to check status even before any phases have been run.
#[test]
fn test_fresh_spec_no_receipts() -> Result<()> {
    let (artifact_manager, receipt_manager, _temp_dir) =
        create_test_managers("test-status-fresh-spec");

    // Don't create any receipts - this is a fresh spec

    // Create minimal effective config
    let mut effective_config = BTreeMap::new();
    effective_config.insert(
        "model".to_string(),
        ("haiku".to_string(), "defaults".to_string()),
    );

    // Generate status - should succeed with sensible defaults
    let result = generate_status(&artifact_manager, &receipt_manager, effective_config, None);

    // Fresh specs should return a valid status with defaults, not an error
    assert!(
        result.is_ok(),
        "Fresh spec with no receipts should return valid status, got error: {:?}",
        result.as_ref().err()
    );

    let status = result.unwrap();

    // Verify sensible defaults for fresh spec
    assert!(
        status.artifacts.is_empty(),
        "Fresh spec should have no artifacts"
    );
    assert!(
        status.last_receipt_path.is_empty(),
        "Fresh spec should have empty receipt path"
    );
    assert_eq!(
        status.runner, "auto",
        "Fresh spec should default to auto runner"
    );
    assert_eq!(
        status.canonicalization_version, "1.0.0",
        "Should use default canonicalization version"
    );
    assert_eq!(
        status.canonicalization_backend, "jcs-rfc8785",
        "Should use default canonicalization backend"
    );
    assert!(
        !status.fallback_used,
        "Fresh spec should not have used fallback"
    );

    // Verify effective config is present
    assert!(
        status.effective_config.contains_key("model"),
        "Effective config should contain model"
    );

    println!("✓ Fresh spec (no prior receipts) test passed");
    Ok(())
}

/// Test FR-STA-005: Lock drift reporting
#[test]
fn test_lock_drift_reporting() -> Result<()> {
    let (artifact_manager, receipt_manager, _temp_dir) =
        create_test_managers("test-status-lock-drift");

    // Create a test receipt
    create_test_receipt(
        &receipt_manager,
        "test-status-lock-drift",
        PhaseId::Requirements,
        "native",
        None,
    );

    // Create effective config
    let mut effective_config = BTreeMap::new();
    effective_config.insert(
        "model".to_string(),
        ("haiku".to_string(), "defaults".to_string()),
    );

    // Create lock drift with all three fields
    let lock_drift = Some(LockDrift {
        model_full_name: Some(DriftPair {
            locked: "haiku".to_string(),
            current: "sonnet-20250101".to_string(),
        }),
        claude_cli_version: Some(DriftPair {
            locked: "0.8.0".to_string(),
            current: "0.8.1".to_string(),
        }),
        schema_version: Some(DriftPair {
            locked: "1".to_string(),
            current: "2".to_string(),
        }),
    });

    // Generate status with drift
    let status = generate_status(
        &artifact_manager,
        &receipt_manager,
        effective_config,
        lock_drift,
    )?;

    // Verify lock drift is present
    assert!(
        status.lock_drift.is_some(),
        "Lock drift should be present in status"
    );

    let drift = status.lock_drift.unwrap();

    // Verify model drift
    assert!(drift.model_full_name.is_some());
    let model_drift = drift.model_full_name.unwrap();
    assert_eq!(model_drift.locked, "haiku");
    assert_eq!(model_drift.current, "sonnet-20250101");

    // Verify CLI version drift
    assert!(drift.claude_cli_version.is_some());
    let cli_drift = drift.claude_cli_version.unwrap();
    assert_eq!(cli_drift.locked, "0.8.0");
    assert_eq!(cli_drift.current, "0.8.1");

    // Verify schema version drift
    assert!(drift.schema_version.is_some());
    let schema_drift = drift.schema_version.unwrap();
    assert_eq!(schema_drift.locked, "1");
    assert_eq!(schema_drift.current, "2");

    println!("✓ Lock drift reporting test passed");
    Ok(())
}

/// Test JCS emission for status outputs
#[test]
fn test_status_jcs_emission() -> Result<()> {
    let (artifact_manager, receipt_manager, _temp_dir) = create_test_managers("test-status-jcs");

    // Create a test receipt
    create_test_receipt(
        &receipt_manager,
        "test-status-jcs",
        PhaseId::Requirements,
        "native",
        None,
    );

    // Create effective config
    let mut effective_config = BTreeMap::new();
    effective_config.insert(
        "model".to_string(),
        ("haiku".to_string(), "defaults".to_string()),
    );

    // Generate status
    let status = generate_status(&artifact_manager, &receipt_manager, effective_config, None)?;

    // Emit as canonical JSON
    let json = StatusManager::emit_json(&status)?;

    // Verify it's valid JSON
    let parsed: serde_json::Value = serde_json::from_str(&json)?;
    assert_eq!(parsed["schema_version"], "1");
    assert_eq!(parsed["runner"], "native");

    // Verify canonical JSON properties (no extra whitespace, no newlines)
    assert!(!json.contains("  "), "JCS should not have extra spaces");
    assert!(!json.contains("\n"), "JCS should not have newlines");

    // Verify re-serialization produces identical output
    let parsed2: serde_json::Value = serde_json::from_str(&json)?;
    let json_bytes2 = serde_json_canonicalizer::to_vec(&parsed2)?;
    let json2 = String::from_utf8(json_bytes2)?;

    assert_eq!(
        json, json2,
        "Re-serialization should produce identical output"
    );

    println!("✓ Status JCS emission test passed");
    Ok(())
}

/// Test status with WSL runner
#[test]
fn test_status_with_wsl_runner() -> Result<()> {
    let (artifact_manager, receipt_manager, _temp_dir) = create_test_managers("test-status-wsl");

    // Create a test receipt with WSL runner
    create_test_receipt(
        &receipt_manager,
        "test-status-wsl",
        PhaseId::Requirements,
        "wsl",
        Some("Ubuntu-22.04".to_string()),
    );

    // Create effective config
    let mut effective_config = BTreeMap::new();
    effective_config.insert(
        "model".to_string(),
        ("haiku".to_string(), "defaults".to_string()),
    );

    // Generate status
    let status = generate_status(&artifact_manager, &receipt_manager, effective_config, None)?;

    // Verify WSL runner info
    assert_eq!(status.runner, "wsl");
    assert_eq!(status.runner_distro, Some("Ubuntu-22.04".to_string()));

    println!("✓ Status with WSL runner test passed");
    Ok(())
}

/// Test status with no lock drift
#[test]
fn test_status_no_lock_drift() -> Result<()> {
    let (artifact_manager, receipt_manager, _temp_dir) =
        create_test_managers("test-status-no-drift");

    // Create a test receipt
    create_test_receipt(
        &receipt_manager,
        "test-status-no-drift",
        PhaseId::Requirements,
        "native",
        None,
    );

    // Create effective config
    let mut effective_config = BTreeMap::new();
    effective_config.insert(
        "model".to_string(),
        ("haiku".to_string(), "defaults".to_string()),
    );

    // Generate status without drift
    let status = generate_status(&artifact_manager, &receipt_manager, effective_config, None)?;

    // Verify no lock drift
    assert!(
        status.lock_drift.is_none(),
        "Lock drift should be None when not provided"
    );

    println!("✓ Status with no lock drift test passed");
    Ok(())
}

/// Test status artifacts are sorted by path
#[test]
fn test_status_artifacts_sorted() -> Result<()> {
    let (artifact_manager, receipt_manager, _temp_dir) = create_test_managers("test-status-sorted");

    // Create a test receipt with multiple outputs in non-alphabetical order
    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let outputs = vec![
        FileHash {
            path: "artifacts/99-final.md".to_string(),
            blake3_canonicalized:
                "zzzz1234567890abcdef1234567890abcdef1234567890abcdef1234567890ab".to_string(),
        },
        FileHash {
            path: "artifacts/00-requirements.md".to_string(),
            blake3_canonicalized:
                "aaaa1234567890abcdef1234567890abcdef1234567890abcdef1234567890ab".to_string(),
        },
        FileHash {
            path: "artifacts/50-middle.md".to_string(),
            blake3_canonicalized:
                "mmmm1234567890abcdef1234567890abcdef1234567890abcdef1234567890ab".to_string(),
        },
    ];

    let receipt = receipt_manager.create_receipt(
        "test-status-sorted",
        PhaseId::Requirements,
        0,
        outputs,
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        HashMap::new(),
        packet,
        None,        // stderr_tail
        None,        // stderr_redacted
        vec![],      // warnings
        Some(false), // fallback_used
        "native",    // runner
        None,        // runner_distro
        None,        // error_kind
        None,        // error_reason
        None,        // diff_context,
        None,        // pipeline
    );

    receipt_manager.write_receipt(&receipt)?;

    // Create the actual artifact files
    let artifacts_dir = artifact_manager.base_path().join("artifacts");
    std::fs::create_dir_all(&artifacts_dir)?;
    std::fs::write(artifacts_dir.join("99-final.md"), "# Final\n")?;
    std::fs::write(artifacts_dir.join("00-requirements.md"), "# Requirements\n")?;
    std::fs::write(artifacts_dir.join("50-middle.md"), "# Middle\n")?;

    // Create effective config
    let mut effective_config = BTreeMap::new();
    effective_config.insert(
        "model".to_string(),
        ("haiku".to_string(), "defaults".to_string()),
    );

    // Generate status
    let status = generate_status(&artifact_manager, &receipt_manager, effective_config, None)?;

    // Verify artifacts are sorted by path
    assert_eq!(status.artifacts.len(), 3);
    assert_eq!(status.artifacts[0].path, "artifacts/00-requirements.md");
    assert_eq!(status.artifacts[1].path, "artifacts/50-middle.md");
    assert_eq!(status.artifacts[2].path, "artifacts/99-final.md");

    println!("✓ Status artifacts sorted test passed");
    Ok(())
}

/// Test status with empty effective config
#[test]
fn test_status_empty_effective_config() -> Result<()> {
    let (artifact_manager, receipt_manager, _temp_dir) =
        create_test_managers("test-status-empty-config");

    // Create a test receipt
    create_test_receipt(
        &receipt_manager,
        "test-status-empty-config",
        PhaseId::Requirements,
        "native",
        None,
    );

    // Create empty effective config
    let effective_config = BTreeMap::new();

    // Generate status
    let status = generate_status(&artifact_manager, &receipt_manager, effective_config, None)?;

    // Verify status is generated successfully with empty config
    assert_eq!(status.schema_version, "1");
    assert_eq!(status.effective_config.len(), 0);

    println!("✓ Status with empty effective config test passed");
    Ok(())
}

/// Test AT-STA-004: Pending fixup summary (counts only)
#[test]
fn test_pending_fixup_summary() -> Result<()> {
    use xchecker::types::PendingFixupsSummary;

    let (artifact_manager, receipt_manager, _temp_dir) =
        create_test_managers("test-status-pending-fixups");

    // Create a test receipt
    create_test_receipt(
        &receipt_manager,
        "test-status-pending-fixups",
        PhaseId::Requirements,
        "native",
        None,
    );

    // Create effective config
    let mut effective_config = BTreeMap::new();
    effective_config.insert(
        "model".to_string(),
        ("haiku".to_string(), "defaults".to_string()),
    );

    // Create pending fixups summary
    let pending_fixups = Some(PendingFixupsSummary {
        targets: 3,
        est_added: 42,
        est_removed: 15,
    });

    // Generate status with pending fixups
    let status = StatusManager::generate_status_internal(
        &artifact_manager,
        &receipt_manager,
        effective_config,
        None,
        pending_fixups,
        None,
    )?;

    // Verify pending fixups are present
    assert!(
        status.pending_fixups.is_some(),
        "Pending fixups should be present in status"
    );

    let fixups = status.pending_fixups.as_ref().unwrap();
    assert_eq!(fixups.targets, 3, "Should have 3 target files");
    assert_eq!(fixups.est_added, 42, "Should have 42 estimated added lines");
    assert_eq!(
        fixups.est_removed, 15,
        "Should have 15 estimated removed lines"
    );

    // Verify JCS emission includes pending_fixups
    let json = StatusManager::emit_json(&status)?;
    assert!(
        json.contains("pending_fixups"),
        "JSON should contain pending_fixups field"
    );
    assert!(
        json.contains("\"targets\":3"),
        "JSON should contain targets count"
    );
    assert!(
        json.contains("\"est_added\":42"),
        "JSON should contain est_added count"
    );
    assert!(
        json.contains("\"est_removed\":15"),
        "JSON should contain est_removed count"
    );

    println!("✓ Pending fixup summary test passed");
    Ok(())
}

/// Test that pending_fixups is omitted when None
#[test]
fn test_pending_fixups_omitted_when_none() -> Result<()> {
    let (artifact_manager, receipt_manager, _temp_dir) =
        create_test_managers("test-status-no-pending-fixups");

    // Create a test receipt
    create_test_receipt(
        &receipt_manager,
        "test-status-no-pending-fixups",
        PhaseId::Requirements,
        "native",
        None,
    );

    // Create effective config
    let mut effective_config = BTreeMap::new();
    effective_config.insert(
        "model".to_string(),
        ("haiku".to_string(), "defaults".to_string()),
    );

    // Generate status without pending fixups
    let status = StatusManager::generate_status_internal(
        &artifact_manager,
        &receipt_manager,
        effective_config,
        None,
        None, // No pending fixups
        None,
    )?;

    // Verify pending fixups are not present
    assert!(
        status.pending_fixups.is_none(),
        "Pending fixups should be None when not provided"
    );

    // Verify JCS emission omits pending_fixups field
    let json = StatusManager::emit_json(&status)?;
    assert!(
        !json.contains("pending_fixups"),
        "JSON should not contain pending_fixups field when None"
    );

    println!("✓ Pending fixups omitted when None test passed");
    Ok(())
}

/// Meta-test that runs all status tests sequentially.
///
/// NOTE: This test is ignored because:
/// 1. All individual tests already run as part of the test suite
/// 2. This meta-test can cause race conditions when run in parallel
///    because several tests manipulate process-wide state (XCHECKER_HOME, env vars)
/// 3. To run manually: cargo test run_all_status_tests -- --ignored
#[test]
#[ignore = "meta-test - individual tests already run; can cause races in parallel execution"]
fn run_all_status_tests() {
    println!("\n=== Running Status Reporting Tests (FR-STA) ===\n");

    test_status_generation_with_effective_config().unwrap();
    test_source_attribution().unwrap();
    test_artifact_enumeration_with_blake3().unwrap();
    test_fresh_spec_no_receipts().unwrap();
    test_lock_drift_reporting().unwrap();
    test_status_jcs_emission().unwrap();
    test_status_with_wsl_runner().unwrap();
    test_status_no_lock_drift().unwrap();
    test_status_artifacts_sorted().unwrap();
    test_status_empty_effective_config().unwrap();
    test_pending_fixup_summary().unwrap();
    test_pending_fixups_omitted_when_none().unwrap();

    println!("\n=== All Status Reporting Tests Passed ===\n");
}

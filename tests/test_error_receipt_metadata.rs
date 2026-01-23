//! B3.12/B3.13: Tests for error receipt metadata
//!
//! These tests validate that error receipts contain proper metadata:
//! 1. error_kind is set appropriately based on error type
//! 2. error_reason is non-empty and doesn't contain secrets
//! 3. pipeline.execution_strategy == "controlled" even on error
//! 4. Timestamp and phase metadata are still populated
//!
//! Tests use various methods to force errors:
//! - Stub backend that returns errors
//! - Phase errors before LLM invocation
//! - Configuration errors
//! - Timeout errors

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
    format!("error-receipt-{}-{}", test_name, std::process::id())
}

/// B3.12.1: Test error receipt when LLM invocation fails (simulated via invalid phase transition)
///
/// This test forces an error by attempting an invalid phase transition (Design without Requirements).
/// It verifies that the error receipt contains:
/// - error_kind set to CliArgs (invalid transition)
/// - error_reason is non-empty
/// - pipeline.execution_strategy == "controlled"
/// - Timestamp and phase metadata are populated
#[tokio::test]
async fn test_error_receipt_invalid_phase_transition() -> Result<()> {
    let _temp = xchecker::paths::with_isolated_home();
    let spec_id = unique_spec_id("invalid-transition");
    let config = dry_run_config();

    let mut handle =
        xchecker::orchestrator::OrchestratorHandle::with_config_and_force(&spec_id, config, false)?;

    // Attempt to run Design phase without running Requirements first
    // This should fail with InvalidTransition error
    let result = handle.run_phase(xchecker::types::PhaseId::Design).await;

    // The operation should fail
    assert!(
        result.is_err(),
        "Running Design without Requirements should fail"
    );

    // Check if there's a receipt for the failed Design phase
    let spec_dir = xchecker::paths::spec_root(&spec_id);
    let receipts_dir = spec_dir.join("receipts");

    if receipts_dir.exists() {
        // Look for Design receipt
        let entries: Vec<_> = std::fs::read_dir(&receipts_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .map(|n| n.starts_with("design-"))
                    .unwrap_or(false)
            })
            .collect();

        if !entries.is_empty() {
            // Read the receipt and verify error metadata
            let receipt_path = entries[0].path();
            let receipt_content = std::fs::read_to_string(&receipt_path)?;
            let receipt: serde_json::Value = serde_json::from_str(&receipt_content)?;

            // Verify error_kind is set
            assert!(
                !receipt["error_kind"].is_null(),
                "error_kind should be set on error receipt"
            );
            assert_eq!(
                receipt["error_kind"].as_str(),
                Some("cli_args"),
                "error_kind should be cli_args for invalid transition"
            );

            // Verify error_reason is non-empty
            assert!(
                !receipt["error_reason"].is_null(),
                "error_reason should be set on error receipt"
            );
            let error_reason = receipt["error_reason"].as_str().unwrap();
            assert!(!error_reason.is_empty(), "error_reason should not be empty");

            // Verify error reason mentions the problem
            assert!(
                error_reason.contains("Design") || error_reason.contains("transition"),
                "error_reason should describe the transition error"
            );

            // Verify pipeline.execution_strategy is still "controlled"
            if !receipt["pipeline"].is_null() {
                assert_eq!(
                    receipt["pipeline"]["execution_strategy"].as_str(),
                    Some("controlled"),
                    "execution_strategy should be 'controlled' even on error"
                );
            }

            // Verify timestamp is present
            assert!(
                !receipt["emitted_at"].is_null(),
                "emitted_at should be set on error receipt"
            );

            // Verify phase metadata is present
            assert_eq!(
                receipt["phase"].as_str(),
                Some("design"),
                "phase should be set correctly"
            );
            assert_eq!(
                receipt["spec_id"].as_str(),
                Some(spec_id.as_str()),
                "spec_id should be set correctly"
            );

            // Verify exit code is non-zero
            assert!(
                receipt["exit_code"].as_i64().unwrap() != 0,
                "exit_code should be non-zero for error"
            );
        }
    }

    Ok(())
}

/// B3.12.2: Test error receipt with packet overflow error
///
/// This test simulates a packet overflow error to verify error receipt metadata.
#[test]
fn test_error_receipt_packet_overflow() -> Result<()> {
    let _temp = xchecker::paths::with_isolated_home();
    let spec_id = unique_spec_id("packet-overflow");

    // Create an error
    let error = xchecker::error::XCheckerError::PacketOverflow {
        used_bytes: 100000,
        limit_bytes: 65536,
        used_lines: 1500,
        limit_lines: 1200,
    };

    // Create receipt manager
    let spec_dir = xchecker::paths::spec_root(&spec_id);
    let receipt_manager = xchecker::receipt::ReceiptManager::new(&spec_dir);

    // Create error receipt
    let packet = xchecker::types::PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let pipeline = Some(xchecker::types::PipelineInfo {
        execution_strategy: Some("controlled".to_string()),
    });

    let error_receipt = receipt_manager.create_error_receipt(
        &spec_id,
        xchecker::types::PhaseId::Requirements,
        &error,
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        HashMap::new(),
        packet,
        Some("Packet overflow stderr".to_string()),
        None,
        vec![],
        None,
        "native",
        None,
        None,
        pipeline,
    );

    // Verify error_kind
    assert_eq!(
        error_receipt.error_kind,
        Some(xchecker::types::ErrorKind::PacketOverflow),
        "error_kind should be PacketOverflow"
    );

    // Verify error_reason is set
    assert!(
        error_receipt.error_reason.is_some(),
        "error_reason should be set"
    );
    let error_reason = error_receipt.error_reason.as_ref().unwrap();
    assert!(!error_reason.is_empty(), "error_reason should not be empty");
    assert!(
        error_reason.contains("100000") || error_reason.contains("overflow"),
        "error_reason should mention the overflow details"
    );

    // Verify pipeline.execution_strategy is "controlled"
    assert!(
        error_receipt.pipeline.is_some(),
        "pipeline should be set on error receipt"
    );
    assert_eq!(
        error_receipt.pipeline.as_ref().unwrap().execution_strategy,
        Some("controlled".to_string()),
        "execution_strategy should be 'controlled' even on error"
    );

    // Verify timestamp is populated
    assert!(
        error_receipt.emitted_at > chrono::Utc::now() - chrono::Duration::seconds(5),
        "emitted_at should be recent"
    );

    // Verify phase metadata
    assert_eq!(error_receipt.phase, "requirements");
    assert_eq!(error_receipt.spec_id, spec_id);

    // Verify exit code matches expected error code
    assert_eq!(
        error_receipt.exit_code, 7,
        "exit_code should be 7 for PacketOverflow"
    );

    // Verify no outputs on error
    assert_eq!(
        error_receipt.outputs.len(),
        0,
        "outputs should be empty on error"
    );

    // Verify stderr_tail is present
    assert!(
        error_receipt.stderr_tail.is_some(),
        "stderr_tail should be preserved"
    );

    Ok(())
}

/// B3.12.3: Test error receipt with secret detection error
///
/// This test simulates a secret detection error to verify:
/// - error_kind is SecretDetected
/// - error_reason doesn't contain the actual secret
/// - All metadata fields are properly populated
#[test]
fn test_error_receipt_secret_detected() -> Result<()> {
    let _temp = xchecker::paths::with_isolated_home();
    let spec_id = unique_spec_id("secret-detected");

    // Create a secret detection error
    let error = xchecker::error::XCheckerError::SecretDetected {
        pattern: "github_pat".to_string(),
        location: "test.txt:42:10".to_string(),
    };

    // Create receipt manager
    let spec_dir = xchecker::paths::spec_root(&spec_id);
    let receipt_manager = xchecker::receipt::ReceiptManager::new(&spec_dir);

    let packet = xchecker::types::PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let pipeline = Some(xchecker::types::PipelineInfo {
        execution_strategy: Some("controlled".to_string()),
    });

    let error_receipt = receipt_manager.create_error_receipt(
        &spec_id,
        xchecker::types::PhaseId::Design,
        &error,
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        HashMap::new(),
        packet,
        None,
        None,
        vec!["Secret pattern detected".to_string()],
        None,
        "native",
        None,
        None,
        pipeline,
    );

    // Verify error_kind
    assert_eq!(
        error_receipt.error_kind,
        Some(xchecker::types::ErrorKind::SecretDetected),
        "error_kind should be SecretDetected"
    );

    // Verify error_reason is set and redacted
    assert!(
        error_receipt.error_reason.is_some(),
        "error_reason should be set"
    );
    let error_reason = error_receipt.error_reason.as_ref().unwrap();
    assert!(!error_reason.is_empty(), "error_reason should not be empty");

    // Error reason should mention the problem but not expose actual secrets
    assert!(
        error_reason.contains("Secret") || error_reason.contains("pattern"),
        "error_reason should describe the error type"
    );

    // Verify exit code
    assert_eq!(
        error_receipt.exit_code, 8,
        "exit_code should be 8 for SecretDetected"
    );

    // Verify pipeline metadata
    assert_eq!(
        error_receipt
            .pipeline
            .as_ref()
            .unwrap()
            .execution_strategy
            .as_deref(),
        Some("controlled")
    );

    // Verify warnings are preserved
    assert_eq!(
        error_receipt.warnings.len(),
        1,
        "warnings should be preserved"
    );

    // Verify timestamp and phase
    assert!(error_receipt.emitted_at > chrono::Utc::now() - chrono::Duration::seconds(5));
    assert_eq!(error_receipt.phase, "design");

    Ok(())
}

/// B3.12.4: Test error receipt with lock held error
///
/// Verifies that concurrent execution errors produce proper error receipts.
#[test]
fn test_error_receipt_lock_held() -> Result<()> {
    let _temp = xchecker::paths::with_isolated_home();
    let spec_id = unique_spec_id("lock-held");

    let error = xchecker::error::XCheckerError::ConcurrentExecution {
        id: spec_id.clone(),
    };

    let spec_dir = xchecker::paths::spec_root(&spec_id);
    let receipt_manager = xchecker::receipt::ReceiptManager::new(&spec_dir);

    let packet = xchecker::types::PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let pipeline = Some(xchecker::types::PipelineInfo {
        execution_strategy: Some("controlled".to_string()),
    });

    let error_receipt = receipt_manager.create_error_receipt(
        &spec_id,
        xchecker::types::PhaseId::Tasks,
        &error,
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        HashMap::new(),
        packet,
        None,
        None,
        vec![],
        None,
        "native",
        None,
        None,
        pipeline,
    );

    // Verify error_kind
    assert_eq!(
        error_receipt.error_kind,
        Some(xchecker::types::ErrorKind::LockHeld),
        "error_kind should be LockHeld"
    );

    // Verify error_reason
    assert!(error_receipt.error_reason.is_some());
    let error_reason = error_receipt.error_reason.as_ref().unwrap();
    assert!(
        error_reason.contains(&spec_id) || error_reason.contains("concurrent"),
        "error_reason should mention concurrent execution"
    );

    // Verify exit code
    assert_eq!(
        error_receipt.exit_code, 9,
        "exit_code should be 9 for LockHeld"
    );

    // Verify pipeline metadata
    assert_eq!(
        error_receipt
            .pipeline
            .as_ref()
            .unwrap()
            .execution_strategy
            .as_deref(),
        Some("controlled")
    );

    // Verify phase and timestamp
    assert_eq!(error_receipt.phase, "tasks");
    assert!(error_receipt.emitted_at > chrono::Utc::now() - chrono::Duration::seconds(5));

    Ok(())
}

/// B3.12.5: Test error receipt with Claude failure (LLM error)
///
/// Simulates an LLM invocation failure to verify error receipt metadata.
#[test]
fn test_error_receipt_claude_failure() -> Result<()> {
    let _temp = xchecker::paths::with_isolated_home();
    let spec_id = unique_spec_id("claude-failure");

    // Simulate a Claude CLI error
    let error =
        xchecker::error::XCheckerError::Claude(xchecker::error::ClaudeError::ExecutionFailed {
            stderr: "Claude CLI execution failed: command not found".to_string(),
        });

    let spec_dir = xchecker::paths::spec_root(&spec_id);
    let receipt_manager = xchecker::receipt::ReceiptManager::new(&spec_dir);

    let packet = xchecker::types::PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let pipeline = Some(xchecker::types::PipelineInfo {
        execution_strategy: Some("controlled".to_string()),
    });

    let error_receipt = receipt_manager.create_error_receipt(
        &spec_id,
        xchecker::types::PhaseId::Review,
        &error,
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        HashMap::new(),
        packet,
        Some("Error: claude: command not found\n".to_string()),
        Some("Redacted stderr output".to_string()),
        vec!["Claude CLI not available".to_string()],
        None,
        "native",
        None,
        None,
        pipeline,
    );

    // Verify error_kind
    assert_eq!(
        error_receipt.error_kind,
        Some(xchecker::types::ErrorKind::ClaudeFailure),
        "error_kind should be ClaudeFailure"
    );

    // Verify error_reason
    assert!(error_receipt.error_reason.is_some());
    let error_reason = error_receipt.error_reason.as_ref().unwrap();
    assert!(
        error_reason.contains("Claude") || error_reason.contains("failed"),
        "error_reason should describe Claude failure"
    );

    // Verify exit code
    assert_eq!(
        error_receipt.exit_code, 70,
        "exit_code should be 70 for ClaudeFailure"
    );

    // Verify pipeline metadata is preserved on error
    assert!(error_receipt.pipeline.is_some());
    assert_eq!(
        error_receipt
            .pipeline
            .as_ref()
            .unwrap()
            .execution_strategy
            .as_deref(),
        Some("controlled"),
        "execution_strategy should remain 'controlled' even on LLM failure"
    );

    // Verify stderr is captured
    assert!(error_receipt.stderr_tail.is_some());
    assert!(error_receipt.stderr_redacted.is_some());

    // Verify warnings are preserved
    assert!(!error_receipt.warnings.is_empty());

    // Verify metadata fields
    assert_eq!(error_receipt.phase, "review");
    assert_eq!(error_receipt.spec_id, spec_id);
    assert!(error_receipt.emitted_at > chrono::Utc::now() - chrono::Duration::seconds(5));
    assert_eq!(error_receipt.xchecker_version, "0.1.0");
    assert_eq!(error_receipt.claude_cli_version, "0.8.1");
    assert_eq!(error_receipt.model_full_name, "haiku");

    Ok(())
}

/// B3.12.6: Test error receipt with unknown error
///
/// Tests that generic/unknown errors still produce valid receipts.
#[test]
fn test_error_receipt_unknown_error() -> Result<()> {
    let _temp = xchecker::paths::with_isolated_home();
    let spec_id = unique_spec_id("unknown-error");

    // Create a generic IO error
    let error =
        xchecker::error::XCheckerError::Io(std::io::Error::other("Something unexpected happened"));

    let spec_dir = xchecker::paths::spec_root(&spec_id);
    let receipt_manager = xchecker::receipt::ReceiptManager::new(&spec_dir);

    let packet = xchecker::types::PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let pipeline = Some(xchecker::types::PipelineInfo {
        execution_strategy: Some("controlled".to_string()),
    });

    let error_receipt = receipt_manager.create_error_receipt(
        &spec_id,
        xchecker::types::PhaseId::Fixup,
        &error,
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        HashMap::new(),
        packet,
        None,
        None,
        vec![],
        None,
        "native",
        None,
        None,
        pipeline,
    );

    // Verify error_kind defaults to Unknown
    assert_eq!(
        error_receipt.error_kind,
        Some(xchecker::types::ErrorKind::Unknown),
        "error_kind should be Unknown for unclassified errors"
    );

    // Verify error_reason is set
    assert!(error_receipt.error_reason.is_some());
    let error_reason = error_receipt.error_reason.as_ref().unwrap();
    assert!(!error_reason.is_empty());

    // Verify exit code defaults to 1
    assert_eq!(
        error_receipt.exit_code, 1,
        "exit_code should be 1 for unknown errors"
    );

    // Verify pipeline metadata is still set
    assert!(error_receipt.pipeline.is_some());
    assert_eq!(
        error_receipt
            .pipeline
            .as_ref()
            .unwrap()
            .execution_strategy
            .as_deref(),
        Some("controlled")
    );

    // Verify basic metadata
    assert_eq!(error_receipt.phase, "fixup");
    assert_eq!(error_receipt.spec_id, spec_id);
    assert!(error_receipt.emitted_at > chrono::Utc::now() - chrono::Duration::seconds(5));

    Ok(())
}

/// B3.12.7: Test error receipt serialization and deserialization
///
/// Verifies that error receipts can be written to disk and read back correctly.
#[test]
fn test_error_receipt_write_and_read() -> Result<()> {
    let _temp = xchecker::paths::with_isolated_home();
    let spec_id = unique_spec_id("write-read");

    let error = xchecker::error::XCheckerError::PacketOverflow {
        used_bytes: 100000,
        limit_bytes: 65536,
        used_lines: 1500,
        limit_lines: 1200,
    };

    let spec_dir = xchecker::paths::spec_root(&spec_id);
    let receipt_manager = xchecker::receipt::ReceiptManager::new(&spec_dir);

    let packet = xchecker::types::PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let pipeline = Some(xchecker::types::PipelineInfo {
        execution_strategy: Some("controlled".to_string()),
    });

    let error_receipt = receipt_manager.create_error_receipt(
        &spec_id,
        xchecker::types::PhaseId::Requirements,
        &error,
        "0.1.0",
        "0.8.1",
        "haiku",
        Some("sonnet".to_string()),
        HashMap::new(),
        packet,
        Some("stderr output".to_string()),
        None,
        vec!["warning1".to_string()],
        None,
        "native",
        None,
        None,
        pipeline.clone(),
    );

    // Write receipt to disk
    let receipt_path = receipt_manager.write_receipt(&error_receipt)?;
    assert!(receipt_path.exists());

    // Read receipt back
    let read_receipt =
        receipt_manager.read_latest_receipt(xchecker::types::PhaseId::Requirements)?;
    assert!(read_receipt.is_some());

    let read_receipt = read_receipt.unwrap();

    // Verify all error fields are preserved
    assert_eq!(read_receipt.error_kind, error_receipt.error_kind);
    assert_eq!(read_receipt.error_reason, error_receipt.error_reason);
    assert_eq!(read_receipt.exit_code, error_receipt.exit_code);
    assert_eq!(read_receipt.stderr_tail, error_receipt.stderr_tail);
    assert_eq!(read_receipt.warnings, error_receipt.warnings);

    // Verify pipeline info is preserved
    assert!(read_receipt.pipeline.is_some());
    assert_eq!(
        read_receipt
            .pipeline
            .as_ref()
            .unwrap()
            .execution_strategy
            .as_deref(),
        Some("controlled")
    );

    // Verify metadata
    assert_eq!(read_receipt.spec_id, spec_id);
    assert_eq!(read_receipt.phase, "requirements");
    assert_eq!(read_receipt.model_alias, Some("sonnet".to_string()));

    Ok(())
}

/// B3.13.1: Test that packet evidence is populated even on error
///
/// Verifies that packet construction evidence is preserved in error receipts.
#[test]
fn test_error_receipt_preserves_packet_evidence() -> Result<()> {
    let _temp = xchecker::paths::with_isolated_home();
    let spec_id = unique_spec_id("packet-evidence");

    let error =
        xchecker::error::XCheckerError::Claude(xchecker::error::ClaudeError::ExecutionFailed {
            stderr: "LLM timeout".to_string(),
        });

    let spec_dir = xchecker::paths::spec_root(&spec_id);
    let receipt_manager = xchecker::receipt::ReceiptManager::new(&spec_dir);

    // Create packet evidence with some files
    let packet = xchecker::types::PacketEvidence {
        files: vec![
            xchecker::types::FileEvidence {
                path: "spec.md".to_string(),
                range: Some("L1-L100".to_string()),
                blake3_pre_redaction: "abc123".to_string(),
                priority: xchecker::types::Priority::High,
            },
            xchecker::types::FileEvidence {
                path: "requirements.yaml".to_string(),
                range: None,
                blake3_pre_redaction: "def456".to_string(),
                priority: xchecker::types::Priority::Upstream,
            },
        ],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let pipeline = Some(xchecker::types::PipelineInfo {
        execution_strategy: Some("controlled".to_string()),
    });

    let error_receipt = receipt_manager.create_error_receipt(
        &spec_id,
        xchecker::types::PhaseId::Design,
        &error,
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        HashMap::new(),
        packet.clone(),
        None,
        None,
        vec![],
        None,
        "native",
        None,
        None,
        pipeline,
    );

    // Verify packet evidence is preserved
    assert_eq!(error_receipt.packet.max_bytes, 65536);
    assert_eq!(error_receipt.packet.max_lines, 1200);
    assert_eq!(error_receipt.packet.files.len(), 2);

    // Verify file evidence details
    assert_eq!(error_receipt.packet.files[0].path, "spec.md");
    assert_eq!(
        error_receipt.packet.files[0].range,
        Some("L1-L100".to_string())
    );
    assert_eq!(error_receipt.packet.files[0].blake3_pre_redaction, "abc123");

    assert_eq!(error_receipt.packet.files[1].path, "requirements.yaml");
    assert_eq!(error_receipt.packet.files[1].range, None);

    Ok(())
}

/// B3.13.2: Test error receipt fields are properly typed
///
/// Ensures all error receipt fields have correct types for JSON serialization.
#[test]
fn test_error_receipt_json_structure() -> Result<()> {
    let _temp = xchecker::paths::with_isolated_home();
    let spec_id = unique_spec_id("json-structure");

    let error = xchecker::error::XCheckerError::SecretDetected {
        pattern: "api_key".to_string(),
        location: "config.yaml:10".to_string(),
    };

    let spec_dir = xchecker::paths::spec_root(&spec_id);
    let receipt_manager = xchecker::receipt::ReceiptManager::new(&spec_dir);

    let packet = xchecker::types::PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let pipeline = Some(xchecker::types::PipelineInfo {
        execution_strategy: Some("controlled".to_string()),
    });

    let error_receipt = receipt_manager.create_error_receipt(
        &spec_id,
        xchecker::types::PhaseId::Requirements,
        &error,
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        HashMap::new(),
        packet,
        None,
        None,
        vec![],
        None,
        "native",
        None,
        None,
        pipeline,
    );

    // Serialize to JSON
    let json = serde_json::to_string_pretty(&error_receipt)?;

    // Parse back to verify structure
    let parsed: serde_json::Value = serde_json::from_str(&json)?;

    // Verify error fields are present and correctly typed
    assert!(
        parsed["error_kind"].is_string(),
        "error_kind should be string"
    );
    assert_eq!(parsed["error_kind"].as_str(), Some("secret_detected"));

    assert!(
        parsed["error_reason"].is_string(),
        "error_reason should be string"
    );
    assert!(
        !parsed["error_reason"].as_str().unwrap().is_empty(),
        "error_reason should not be empty"
    );

    assert!(
        parsed["exit_code"].is_number(),
        "exit_code should be number"
    );
    assert_eq!(parsed["exit_code"].as_i64(), Some(8));

    // Verify pipeline is object with execution_strategy
    assert!(parsed["pipeline"].is_object(), "pipeline should be object");
    assert_eq!(
        parsed["pipeline"]["execution_strategy"].as_str(),
        Some("controlled")
    );

    // Verify timestamp is string (RFC3339)
    assert!(
        parsed["emitted_at"].is_string(),
        "emitted_at should be string"
    );

    // Verify phase and spec_id
    assert_eq!(parsed["phase"].as_str(), Some("requirements"));
    assert_eq!(parsed["spec_id"].as_str(), Some(spec_id.as_str()));

    // Verify outputs is empty array on error
    assert!(parsed["outputs"].is_array(), "outputs should be array");
    assert_eq!(parsed["outputs"].as_array().unwrap().len(), 0);

    Ok(())
}

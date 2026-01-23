#![cfg(feature = "legacy_claude")]
//! Simple M1 Gate Validation Test
//!
//! **WHITE-BOX TEST**: This test uses internal module APIs (`claude::ClaudeWrapper`,
//! `orchestrator::{OrchestratorConfig, PhaseOrchestrator}`, `runner::{...}`, `types::{...}`)
//! and may break with internal refactors. These tests are intentionally white-box to validate
//! Claude stub integration. Prefer `OrchestratorHandle` for new tests. See FR-TEST-4 for
//! white-box test policy.
//!
//! This test validates the M1 Gate requirements by running individual tests
//! for the key functionality without complex orchestration.

use anyhow::Result;
use tempfile::TempDir;

use xchecker::claude::ClaudeWrapper;
use xchecker::orchestrator::{OrchestratorConfig, PhaseOrchestrator};
use xchecker::runner::{Runner, WslOptions};
use xchecker::types::{PhaseId, RunnerMode};

#[allow(clippy::duplicate_mod)]
#[path = "test_support/mod.rs"]
mod test_support;

/// Test the complete Requirements phase with Claude integration
#[tokio::test]
#[ignore = "requires_claude_stub"]
async fn test_m1_gate_requirements_phase_integration() -> Result<()> {
    // Setup test environment
    let temp_dir = TempDir::new()?;
    let _cwd_guard = test_support::CwdGuard::new(temp_dir.path())?;

    let spec_id = "m1-gate-test";
    let orchestrator = PhaseOrchestrator::new(spec_id)?;

    // Configure to use claude-stub
    let config = OrchestratorConfig {
        dry_run: false,
        config: {
            let mut map = std::collections::HashMap::new();
            map.insert(
                "claude_cli_path".to_string(),
                "cargo run --bin claude-stub --".to_string(),
            );
            map.insert("claude_scenario".to_string(), "success".to_string());
            map.insert("verbose".to_string(), "true".to_string());
            map
        },
        full_config: None,
        selectors: None,
        strict_validation: false,
        redactor: Default::default(),
        hooks: None,
    };

    // Execute Requirements phase
    let result = orchestrator.execute_requirements_phase(&config).await?;

    // Validate M1 Gate requirements

    // R4.1 & R4.4: Complete Requirements phase with Claude CLI integration
    assert!(
        result.success,
        "Requirements phase should complete successfully"
    );
    assert_eq!(result.exit_code, 0, "Exit code should be 0 for success");
    assert_eq!(
        result.phase,
        PhaseId::Requirements,
        "Phase should be Requirements"
    );

    // Verify artifacts were created
    assert!(!result.artifact_paths.is_empty(), "Should create artifacts");
    assert_eq!(
        result.artifact_paths.len(),
        2,
        "Should create 2 artifacts (.md and .core.yaml)"
    );

    // Verify receipt was created
    assert!(result.receipt_path.is_some(), "Should create receipt");

    // R2.1: Verify receipt contains all required metadata
    let receipt_path = result.receipt_path.unwrap();
    assert!(receipt_path.exists(), "Receipt file should exist");

    let receipt_content = std::fs::read_to_string(&receipt_path)?;
    let receipt: xchecker::types::Receipt = serde_json::from_str(&receipt_content)?;

    // Validate required metadata fields (R2.1)
    assert_eq!(
        receipt.spec_id, spec_id,
        "Receipt should have correct spec_id"
    );
    assert_eq!(
        receipt.phase, "requirements",
        "Receipt should have correct phase"
    );
    assert!(
        !receipt.xchecker_version.is_empty(),
        "Receipt should have xchecker version"
    );
    assert!(
        !receipt.claude_cli_version.is_empty(),
        "Receipt should have Claude CLI version"
    );
    assert!(
        !receipt.model_full_name.is_empty(),
        "Receipt should have model full name"
    );
    assert!(
        !receipt.canonicalization_version.is_empty(),
        "Receipt should have canonicalization version"
    );
    assert_eq!(
        receipt.exit_code, 0,
        "Receipt should record successful exit code"
    );

    // Validate runner information (R12.1, R12.2)
    assert!(
        !receipt.runner.is_empty(),
        "Receipt should have runner information"
    );
    assert!(
        receipt.runner == "native" || receipt.runner == "wsl",
        "Runner should be native or wsl"
    );

    // Validate output hashes
    assert!(
        !receipt.outputs.is_empty(),
        "Receipt should have output file hashes"
    );
    for output in &receipt.outputs {
        assert!(!output.path.is_empty(), "Output path should not be empty");
        assert!(
            !output.blake3_canonicalized.is_empty(),
            "Output hash should not be empty"
        );
        assert_eq!(
            output.blake3_canonicalized.len(),
            64,
            "BLAKE3 hash should be 64 characters"
        );
    }

    // Verify artifacts have proper content
    let spec_dir = temp_dir.path().join(".xchecker/specs").join(spec_id);
    let artifacts_dir = spec_dir.join("artifacts");

    let requirements_md = artifacts_dir.join("00-requirements.md");
    let requirements_yaml = artifacts_dir.join("00-requirements.core.yaml");

    assert!(
        requirements_md.exists(),
        "Requirements markdown should exist"
    );
    assert!(requirements_yaml.exists(), "Requirements YAML should exist");

    // Verify content quality
    let md_content = std::fs::read_to_string(&requirements_md)?;
    assert!(
        md_content.contains("# Requirements Document"),
        "Should have proper title"
    );
    assert!(
        md_content.contains("## Introduction"),
        "Should have introduction"
    );
    assert!(
        md_content.contains("**User Story:**"),
        "Should have user stories"
    );
    assert!(
        md_content.contains("#### Acceptance Criteria"),
        "Should have acceptance criteria"
    );
    assert!(
        md_content.contains("WHEN"),
        "Should have EARS format criteria"
    );
    assert!(
        md_content.contains("THEN"),
        "Should have EARS format criteria"
    );
    assert!(
        md_content.contains("SHALL"),
        "Should have EARS format criteria"
    );

    println!("✓ M1 Gate Requirements phase integration test passed");
    println!("✓ R4.1: Claude CLI integration validated");
    println!("✓ R4.4: Structured output handling validated");
    println!("✓ R2.1: Receipt metadata completeness validated");
    println!("✓ R12.1, R12.2: Runner system validated");

    Ok(())
}

/// Test Claude wrapper stream-json parsing capabilities
#[tokio::test]
#[ignore = "requires_claude_stub"]
async fn test_m1_gate_claude_wrapper_parsing() -> Result<()> {
    let runner = Runner::new(RunnerMode::Native, WslOptions::default());
    let wrapper = ClaudeWrapper::new(Some("haiku".to_string()), runner)?;

    // Test stream-json parsing with sample data
    let sample_json = concat!(
        r#"{"type": "conversation_start", "conversation": {"id": "conv_123"}}"#,
        "\n",
        r#"{"type": "message_start", "message": {"id": "msg_123", "role": "assistant"}}"#,
        "\n",
        r#"{"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}"#,
        "\n",
        r#"{"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "Hello"}}"#,
        "\n",
        r#"{"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": " World"}}"#,
        "\n",
        r#"{"type": "content_block_stop", "index": 0}"#,
        "\n",
        r#"{"type": "message_stop", "message": {"id": "msg_123", "model": "haiku", "stop_reason": "end_turn", "usage": {"input_tokens": 10, "output_tokens": 5}}}"#
    );

    let (content, metadata) = wrapper.parse_stream_json(sample_json)?;

    // Validate parsed content
    assert_eq!(content, "Hello World", "Should parse content correctly");
    assert_eq!(metadata.input_tokens, Some(10), "Should parse input tokens");
    assert_eq!(
        metadata.output_tokens,
        Some(5),
        "Should parse output tokens"
    );
    assert_eq!(
        metadata.model,
        Some("haiku".to_string()),
        "Should parse model"
    );
    assert_eq!(
        metadata.stop_reason,
        Some("end_turn".to_string()),
        "Should parse stop reason"
    );

    println!("✓ Claude wrapper stream-json parsing test passed");

    Ok(())
}

/// Test model resolution and version capture
#[tokio::test]
#[ignore = "requires_claude_stub"]
async fn test_m1_gate_model_resolution() -> Result<()> {
    // Test model alias resolution
    let runner = Runner::new(RunnerMode::Native, WslOptions::default());
    let wrapper_with_alias = ClaudeWrapper::new(Some("sonnet".to_string()), runner.clone())?;
    let (alias, full_name) = wrapper_with_alias.get_model_info();

    assert_eq!(
        alias,
        Some("sonnet".to_string()),
        "Should preserve model alias"
    );
    assert_eq!(full_name, "haiku", "Should resolve alias to full name");

    // Test version capture
    let version = wrapper_with_alias.get_version();
    assert!(!version.is_empty(), "Should capture Claude CLI version");

    println!("✓ Model resolution and version capture test passed");

    Ok(())
}

/// Test runner auto-detection
#[tokio::test]
#[ignore = "requires_claude_stub"]
async fn test_m1_gate_runner_auto_detection() -> Result<()> {
    // Test auto-detection logic
    let detected_mode = Runner::detect_auto()?;

    // On non-Windows platforms, should always be Native
    if !cfg!(target_os = "windows") {
        assert_eq!(
            detected_mode,
            RunnerMode::Native,
            "Non-Windows should use Native runner"
        );
    } else {
        // On Windows, should be either Native or Wsl
        assert!(
            detected_mode == RunnerMode::Native || detected_mode == RunnerMode::Wsl,
            "Windows should detect Native or WSL runner"
        );
    }

    // Test runner creation with auto mode
    let runner = Runner::auto()?;
    assert_eq!(
        runner.mode, detected_mode,
        "Auto runner should use detected mode"
    );

    // Test runner validation
    runner.validate()?;

    println!(
        "✓ Runner auto-detection test passed (detected: {:?})",
        detected_mode
    );

    Ok(())
}

/// Test fallback behavior from stream-json to text format
#[tokio::test]
#[ignore = "requires_claude_stub"]
async fn test_m1_gate_fallback_behavior() -> Result<()> {
    // Test that the Claude wrapper can detect parse errors
    let runner = Runner::new(RunnerMode::Native, WslOptions::default());
    let wrapper = ClaudeWrapper::new(None, runner)?;

    let malformed_json = r#"{"type": "conversation_start", "conversation": {"id": "conv_123"}}
{"type": "message_start", "message": {"id": "msg_123"#;

    let parse_result = wrapper.parse_stream_json(malformed_json);
    assert!(parse_result.is_err(), "Should fail to parse malformed JSON");

    // Verify error type is ParseError
    if let Err(e) = parse_result {
        let error_str = format!("{:?}", e);
        assert!(error_str.contains("ParseError"), "Should be a parse error");
    }

    println!("✓ Claude wrapper fallback behavior test passed");

    Ok(())
}

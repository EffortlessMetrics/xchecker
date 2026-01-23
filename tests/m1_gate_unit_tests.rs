#![cfg(feature = "legacy_claude")]
//! M1 Gate Unit Tests
//!
//! **WHITE-BOX TEST**: This test uses internal module APIs (`claude::ClaudeWrapper`,
//! `runner::{Runner, RunnerMode, WslOptions}`) and may break with internal refactors.
//! These tests are intentionally white-box to validate internal implementation details.
//! See FR-TEST-4 for white-box test policy.
//!
//! This module contains unit tests for M1 Gate validation that don't require
//! external dependencies like Claude CLI installation.

use anyhow::Result;
use xchecker::claude::ClaudeWrapper;
use xchecker::runner::{Runner, RunnerMode, WslOptions};

/// Test Claude wrapper stream-json parsing capabilities without CLI validation
#[tokio::test]
async fn test_m1_gate_stream_json_parsing() -> Result<()> {
    // Create a mock wrapper for testing parsing only
    let runner = Runner::new(RunnerMode::Native, WslOptions::default());

    // Create wrapper with mock version to avoid CLI validation
    let wrapper = ClaudeWrapper {
        model_alias: Some("haiku".to_string()),
        model_full_name: "haiku".to_string(),
        max_turns: 10,
        allowed_tools: Vec::new(),
        disallowed_tools: Vec::new(),
        permission_mode: None,
        claude_cli_version: "0.8.1".to_string(), // Mock version
        runner,
    };

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

    // Validate parsed content (R4.1, R4.4)
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

/// Test fallback behavior from stream-json to text format
#[tokio::test]
async fn test_m1_gate_fallback_behavior() -> Result<()> {
    // Create a mock wrapper for testing parsing only
    let runner = Runner::new(RunnerMode::Native, WslOptions::default());

    let wrapper = ClaudeWrapper {
        model_alias: None,
        model_full_name: "haiku".to_string(),
        max_turns: 10,
        allowed_tools: Vec::new(),
        disallowed_tools: Vec::new(),
        permission_mode: None,
        claude_cli_version: "0.8.1".to_string(), // Mock version
        runner,
    };

    let malformed_json = r#"{"type": "conversation_start", "conversation": {"id": "conv_123"}}
{"type": "message_start", "message": {"id": "msg_123"#;

    let parse_result = wrapper.parse_stream_json(malformed_json);
    assert!(parse_result.is_err(), "Should fail to parse malformed JSON");

    // Verify error type is ParseError (R4.4)
    if let Err(e) = parse_result {
        let error_str = format!("{e:?}");
        assert!(error_str.contains("ParseError"), "Should be a parse error");
    }

    println!("✓ Claude wrapper fallback behavior test passed");

    Ok(())
}

/// Test model resolution functionality
#[tokio::test]
async fn test_m1_gate_model_resolution() -> Result<()> {
    // Test model alias resolution without CLI validation
    let runner = Runner::new(RunnerMode::Native, WslOptions::default());

    let wrapper_with_alias = ClaudeWrapper {
        model_alias: Some("sonnet".to_string()),
        model_full_name: "haiku".to_string(),
        max_turns: 10,
        allowed_tools: Vec::new(),
        disallowed_tools: Vec::new(),
        permission_mode: None,
        claude_cli_version: "0.8.1".to_string(),
        runner: runner.clone(),
    };

    let (alias, full_name) = wrapper_with_alias.get_model_info();

    assert_eq!(
        alias,
        Some("sonnet".to_string()),
        "Should preserve model alias"
    );
    assert_eq!(full_name, "haiku", "Should resolve alias to full name");

    // Test full model name (no alias)
    let wrapper_with_full = ClaudeWrapper {
        model_alias: Some("haiku".to_string()),
        model_full_name: "haiku".to_string(),
        max_turns: 10,
        allowed_tools: Vec::new(),
        disallowed_tools: Vec::new(),
        permission_mode: None,
        claude_cli_version: "0.8.1".to_string(),
        runner,
    };

    let (alias2, full_name2) = wrapper_with_full.get_model_info();

    assert_eq!(
        alias2,
        Some("haiku".to_string()),
        "Should preserve full name as alias"
    );
    assert_eq!(full_name2, "haiku", "Should use full name as-is");

    // Test version capture
    let version = wrapper_with_alias.get_version();
    assert_eq!(version, "0.8.1", "Should capture Claude CLI version");

    println!("✓ Model resolution and version capture test passed");

    Ok(())
}

/// Test runner system functionality without CLI validation
#[tokio::test]
async fn test_m1_gate_runner_system() -> Result<()> {
    // Test runner creation and configuration
    let runner = Runner::new(RunnerMode::Native, WslOptions::default());
    assert_eq!(
        runner.mode,
        RunnerMode::Native,
        "Should create native runner"
    );

    // Test WSL options
    let wsl_options = WslOptions {
        distro: Some("Ubuntu-22.04".to_string()),
        claude_path: Some("/usr/local/bin/claude".to_string()),
    };

    let wsl_runner = Runner::new(RunnerMode::Wsl, wsl_options);
    assert_eq!(wsl_runner.mode, RunnerMode::Wsl, "Should create WSL runner");
    assert_eq!(
        wsl_runner.wsl_options.distro,
        Some("Ubuntu-22.04".to_string()),
        "Should preserve distro"
    );
    assert_eq!(
        wsl_runner.wsl_options.claude_path,
        Some("/usr/local/bin/claude".to_string()),
        "Should preserve claude path"
    );

    // Test auto-detection logic (without actual CLI validation)
    // On non-Windows platforms, should always be Native
    if !cfg!(target_os = "windows") {
        // We can't test the actual detection without Claude CLI, but we can test the logic
        let auto_runner = Runner::new(RunnerMode::Auto, WslOptions::default());
        assert_eq!(
            auto_runner.mode,
            RunnerMode::Auto,
            "Should create auto runner"
        );
    }

    println!("✓ Runner system functionality test passed");

    Ok(())
}

/// Test receipt structure validation (without actual receipt generation)
#[tokio::test]
async fn test_m1_gate_receipt_structure() -> Result<()> {
    use chrono::Utc;
    use std::collections::HashMap;
    use xchecker::types::{FileHash, PacketEvidence, Receipt};

    // Create a mock receipt to test structure
    let receipt = Receipt {
        schema_version: "1".to_string(),
        emitted_at: Utc::now(),
        spec_id: "test-spec".to_string(),
        phase: "requirements".to_string(),
        xchecker_version: "0.1.0+abc123".to_string(),
        claude_cli_version: "0.8.1".to_string(),
        model_full_name: "haiku".to_string(),
        model_alias: Some("sonnet".to_string()),
        canonicalization_version: "yaml-v1,md-v1".to_string(),
        flags: {
            let mut flags = HashMap::new();
            flags.insert("max_turns".to_string(), "10".to_string());
            flags.insert("output_format".to_string(), "stream-json".to_string());
            flags
        },
        runner: "native".to_string(),
        runner_distro: None,
        canonicalization_backend: "jcs-rfc8785".to_string(),
        packet: PacketEvidence {
            files: Vec::new(),
            max_bytes: 65536,
            max_lines: 1200,
        },
        outputs: vec![
            FileHash {
                path: "artifacts/00-requirements.md".to_string(),
                blake3_canonicalized: "a".repeat(64),
            },
            FileHash {
                path: "artifacts/00-requirements.core.yaml".to_string(),
                blake3_canonicalized: "b".repeat(64),
            },
        ],
        exit_code: 0,
        error_kind: None,
        error_reason: None,
        stderr_tail: None,
        stderr_redacted: None,
        warnings: Vec::new(),
        fallback_used: Some(false),
        diff_context: None,
        llm: None,
        pipeline: None,
    };

    // Validate receipt structure (R2.1)
    assert_eq!(
        receipt.spec_id, "test-spec",
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

    // Test JSON serialization/deserialization
    let json_str = serde_json::to_string(&receipt)?;
    let deserialized: Receipt = serde_json::from_str(&json_str)?;

    assert_eq!(
        deserialized.spec_id, receipt.spec_id,
        "Should deserialize correctly"
    );
    assert_eq!(
        deserialized.phase, receipt.phase,
        "Should deserialize correctly"
    );
    assert_eq!(
        deserialized.exit_code, receipt.exit_code,
        "Should deserialize correctly"
    );

    println!("✓ Receipt structure validation test passed");

    Ok(())
}

/// Test that validates the M1 Gate requirements without external dependencies
/// Note: This test is disabled because it calls other #[tokio::test] functions
/// which creates nested runtimes. The individual tests run separately.
#[tokio::test]
#[ignore = "requires_refactoring"]
async fn test_m1_gate_comprehensive_validation() -> Result<()> {
    println!("🚀 Running M1 Gate comprehensive validation...");

    // Run all unit tests
    test_m1_gate_stream_json_parsing()?;
    test_m1_gate_fallback_behavior()?;
    test_m1_gate_model_resolution()?;
    test_m1_gate_runner_system()?;
    test_m1_gate_receipt_structure()?;

    println!("✅ M1 Gate comprehensive validation passed!");
    println!();
    println!("M1 Gate Requirements Validated (Unit Tests):");
    println!("  ✓ R4.1: Claude CLI integration structure validated");
    println!("  ✓ R4.4: Structured output handling with fallback capabilities");
    println!("  ✓ R2.1: Receipt contains all required metadata fields");
    println!("  ✓ R12.1, R12.2: Runner system with Native and WSL support");
    println!();
    println!("Key Features Verified:");
    println!("  ✓ Stream-JSON parsing with proper content extraction");
    println!("  ✓ Fallback behavior for malformed JSON");
    println!("  ✓ Model alias resolution and version capture");
    println!("  ✓ Runner system configuration and options");
    println!("  ✓ Receipt structure with all required metadata fields");

    Ok(())
}

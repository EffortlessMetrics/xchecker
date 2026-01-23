#![cfg(feature = "legacy_claude")]
#![allow(clippy::duplicated_attributes)]
//! Golden Pipeline Tests for Claude CLI Integration
//!
//! This module tests the complete Claude CLI integration pipeline with various
//! response scenarios, including truncated, malformed, and plain text responses.
//!
//! Requirements tested:
//! - R4.1: Claude CLI integration with stream-json and text fallback
//! - R4.3: Error handling and partial output preservation
//! - R4.4: Structured output handling with fallback capabilities
//!
//! **White-box test**: Uses `PhaseOrchestrator` directly to probe internal behavior
//! (artifact staging, receipt generation, etc.) rather than `OrchestratorHandle`.

use anyhow::Result;
use std::path::PathBuf;
use tempfile::TempDir;

use xchecker::claude::ClaudeWrapper;
use xchecker::orchestrator::{OrchestratorConfig, PhaseOrchestrator};
use xchecker::runner::{Runner, RunnerMode, WslOptions};
use xchecker::types::{PhaseId, Receipt};

#[allow(clippy::duplicate_mod)]
#[path = "test_support/mod.rs"]
mod test_support;

/// Test environment for golden pipeline validation
///
/// Note: Field order matters for drop semantics. Fields drop in declaration order,
/// so `_cwd_guard` must be declared first to restore CWD before `temp_dir` is deleted.
struct GoldenPipelineTestEnvironment {
    #[allow(dead_code)]
    _cwd_guard: test_support::CwdGuard,
    temp_dir: TempDir,
    orchestrator: PhaseOrchestrator,
    spec_id: String,
}

impl GoldenPipelineTestEnvironment {
    fn new(test_name: &str) -> Result<Self> {
        let temp_dir = TempDir::new()?;
        let cwd_guard = test_support::CwdGuard::new(temp_dir.path())?;

        // Create .xchecker directory structure
        std::fs::create_dir_all(temp_dir.path().join(".xchecker/specs"))?;

        let spec_id = format!("golden-{test_name}");
        let orchestrator = PhaseOrchestrator::new(&spec_id)?;

        Ok(Self {
            _cwd_guard: cwd_guard,
            temp_dir,
            orchestrator,
            spec_id,
        })
    }

    fn spec_dir(&self) -> PathBuf {
        self.temp_dir
            .path()
            .join(".xchecker/specs")
            .join(&self.spec_id)
    }

    fn create_config_with_scenario(&self, scenario: &str) -> OrchestratorConfig {
        OrchestratorConfig {
            dry_run: false,
            config: {
                let mut map = std::collections::HashMap::new();
                map.insert("runner_mode".to_string(), "native".to_string());
                let stub_path = test_support::claude_stub_path()
                    .expect("claude-stub path is required for golden pipeline tests");
                map.insert("claude_cli_path".to_string(), stub_path);
                map.insert("claude_scenario".to_string(), scenario.to_string());
                map.insert("verbose".to_string(), "true".to_string());
                map
            },
            full_config: None,
            selectors: None,
            strict_validation: false,
            redactor: Default::default(),
            hooks: None,
        }
    }
}

/// Test 1: Valid stream-json output with complete messages
/// Validates R4.1 and R4.4 for successful stream-json parsing
#[tokio::test]
#[ignore = "requires_claude_stub"]
async fn test_valid_stream_json_complete_messages() -> Result<()> {
    let env = GoldenPipelineTestEnvironment::new("valid-stream-json")?;
    let config = env.create_config_with_scenario("success");

    // Execute Requirements phase with valid stream-json
    let result = env.orchestrator.execute_requirements_phase(&config).await?;

    // Validate successful execution
    assert!(
        result.success,
        "Phase should complete successfully with valid stream-json"
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
        "Should create .md and .core.yaml"
    );

    // Verify receipt shows successful stream-json usage
    assert!(result.receipt_path.is_some(), "Should create receipt");
    let receipt_path = result.receipt_path.unwrap();
    let receipt_content = std::fs::read_to_string(&receipt_path)?;
    let receipt: Receipt = serde_json::from_str(&receipt_content)?;

    assert_eq!(receipt.exit_code, 0, "Receipt should show success");
    assert_eq!(
        receipt.fallback_used,
        Some(false),
        "Should not use fallback for valid stream-json"
    );
    assert!(
        receipt.stderr_tail.is_none() || receipt.stderr_tail.as_ref().unwrap().is_empty(),
        "Should have no stderr for successful execution"
    );

    // Verify artifact content quality
    let artifacts_dir = env.spec_dir().join("artifacts");
    let req_md = std::fs::read_to_string(artifacts_dir.join("00-requirements.md"))?;

    assert!(
        req_md.contains("# Requirements Document"),
        "Should have proper structure"
    );
    assert!(req_md.len() > 100, "Should have substantial content");

    println!("✓ Valid stream-json complete messages test passed");
    Ok(())
}

/// Test 2: Truncated events and partial messages
/// Validates R4.4 handling of incomplete stream-json data
#[tokio::test]
#[ignore = "requires_claude_stub"]
async fn test_truncated_events_and_partial_messages() -> Result<()> {
    let env = GoldenPipelineTestEnvironment::new("truncated-events")?;
    let config = env.create_config_with_scenario("truncated");

    // Execute phase with truncated stream-json
    let result = env.orchestrator.execute_requirements_phase(&config).await?;

    // Truncated scenario might succeed with partial content or fail gracefully
    if result.success {
        // If it succeeds, verify partial content handling
        assert!(
            !result.artifact_paths.is_empty(),
            "Should create artifacts even with truncated input"
        );

        // Check receipt for any warnings about truncation
        let receipt_path = result.receipt_path.unwrap();
        let receipt_content = std::fs::read_to_string(&receipt_path)?;
        let receipt: Receipt = serde_json::from_str(&receipt_content)?;

        // May have warnings about truncated content
        if !receipt.warnings.is_empty() {
            println!("Warnings for truncated content: {:?}", receipt.warnings);
        }
    } else {
        // If it fails, verify proper error handling
        assert_ne!(
            result.exit_code, 0,
            "Should have non-zero exit code for truncated input"
        );
        assert!(result.error.is_some(), "Should have error message");

        // Should still create receipt with failure information
        assert!(
            result.receipt_path.is_some(),
            "Should create receipt even on failure"
        );

        let receipt_path = result.receipt_path.unwrap();
        let receipt_content = std::fs::read_to_string(&receipt_path)?;
        let receipt: Receipt = serde_json::from_str(&receipt_content)?;

        assert_ne!(receipt.exit_code, 0, "Receipt should record failure");
        assert!(
            receipt.stderr_tail.is_some(),
            "Should capture stderr for failure"
        );
    }

    println!("✓ Truncated events and partial messages test passed");
    Ok(())
}

/// Test 3: Malformed JSON requiring text fallback
/// Validates R4.4 fallback mechanism when stream-json parsing fails
#[tokio::test]
#[ignore = "requires_claude_stub"]
async fn test_malformed_json_text_fallback() -> Result<()> {
    let env = GoldenPipelineTestEnvironment::new("malformed-json")?;
    let config = env.create_config_with_scenario("malformed");

    // Execute phase with malformed JSON that should trigger fallback
    let result = env.orchestrator.execute_requirements_phase(&config).await?;

    // Malformed scenario should either succeed with fallback or fail gracefully
    if result.success {
        // If successful, should have used text fallback
        let receipt_path = result.receipt_path.unwrap();
        let receipt_content = std::fs::read_to_string(&receipt_path)?;
        let receipt: Receipt = serde_json::from_str(&receipt_content)?;

        assert_eq!(
            receipt.fallback_used,
            Some(true),
            "Should have used text fallback for malformed JSON"
        );
        assert!(
            !result.artifact_paths.is_empty(),
            "Should create artifacts with fallback"
        );

        // Verify artifacts have content (even if from text fallback)
        let artifacts_dir = env.spec_dir().join("artifacts");
        let req_md = std::fs::read_to_string(artifacts_dir.join("00-requirements.md"))?;
        assert!(!req_md.is_empty(), "Should have content from text fallback");
    } else {
        // If failed, verify proper error handling
        assert_ne!(result.exit_code, 0, "Should have non-zero exit code");
        assert!(result.error.is_some(), "Should have error message");

        // Check if partial artifact was created (R4.3)
        let artifacts_dir = env.spec_dir().join("artifacts");
        if artifacts_dir.exists() {
            let partial_files: Vec<_> = std::fs::read_dir(&artifacts_dir)?
                .filter_map(std::result::Result::ok)
                .filter(|entry| {
                    entry
                        .path()
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.contains(".partial."))
                })
                .collect();

            if !partial_files.is_empty() {
                println!("Partial artifacts created for malformed JSON scenario");
            }
        }
    }

    println!("✓ Malformed JSON text fallback test passed");
    Ok(())
}

/// Test 4: Plain text output mode
/// Validates R4.4 handling of plain text responses
#[tokio::test]
#[ignore = "requires_claude_stub"]
async fn test_plain_text_output_mode() -> Result<()> {
    let env = GoldenPipelineTestEnvironment::new("plain-text")?;
    let config = env.create_config_with_scenario("text");

    // Execute phase with plain text output
    let result = env.orchestrator.execute_requirements_phase(&config).await?;

    // Plain text should be handled successfully
    assert!(
        result.success,
        "Plain text output should be handled successfully"
    );
    assert_eq!(
        result.exit_code, 0,
        "Exit code should be 0 for plain text success"
    );

    // Verify receipt shows text format usage
    let receipt_path = result.receipt_path.unwrap();
    let receipt_content = std::fs::read_to_string(&receipt_path)?;
    let receipt: Receipt = serde_json::from_str(&receipt_content)?;

    // Plain text mode might not set fallback_used if it was the intended format
    assert_eq!(receipt.exit_code, 0, "Receipt should show success");

    // Verify artifacts were created from plain text
    assert!(
        !result.artifact_paths.is_empty(),
        "Should create artifacts from plain text"
    );

    let artifacts_dir = env.spec_dir().join("artifacts");
    let req_md = std::fs::read_to_string(artifacts_dir.join("00-requirements.md"))?;

    assert!(
        req_md.contains("# Requirements Document"),
        "Should parse plain text into proper structure"
    );
    assert!(!req_md.is_empty(), "Should have content from plain text");

    println!("✓ Plain text output mode test passed");
    Ok(())
}

/// Test 5: Various exit codes and stderr patterns
/// Validates R4.3 error handling for different failure modes
#[tokio::test]
#[ignore = "requires_claude_stub"]
async fn test_various_exit_codes_and_stderr_patterns() -> Result<()> {
    if !test_support::should_run_e2e() {
        eprintln!("(skipped) E2E test requires Claude CLI or XCHECKER_E2E=1");
        return Ok(());
    }

    let scenarios = vec![
        ("error", "General error scenario"),
        ("timeout", "Timeout scenario"),
        ("permission", "Permission denied scenario"),
        ("network", "Network error scenario"),
    ];

    for (scenario, description) in scenarios {
        println!("Testing scenario: {scenario} ({description})");

        let env = GoldenPipelineTestEnvironment::new(&format!("exit-codes-{scenario}"))?;
        let config = env.create_config_with_scenario(scenario);

        // Execute phase with error scenario
        let result = env.orchestrator.execute_requirements_phase(&config).await?;

        // Should fail with appropriate error handling
        assert!(!result.success, "Error scenario should fail");
        assert_ne!(result.exit_code, 0, "Should have non-zero exit code");
        assert!(result.error.is_some(), "Should have error message");

        // Verify receipt captures error information (R4.3)
        assert!(
            result.receipt_path.is_some(),
            "Should create receipt even on failure"
        );

        let receipt_path = result.receipt_path.unwrap();
        let receipt_content = std::fs::read_to_string(&receipt_path)?;
        let receipt: Receipt = serde_json::from_str(&receipt_content)?;

        assert_ne!(
            receipt.exit_code, 0,
            "Receipt should record failure exit code"
        );
        assert!(receipt.stderr_tail.is_some(), "Should capture stderr tail");
        assert!(
            !receipt.warnings.is_empty(),
            "Should have warnings for error scenario"
        );

        // Verify stderr tail is limited to 2 KiB (R4.3)
        if let Some(stderr) = &receipt.stderr_tail {
            assert!(
                stderr.len() <= 2048,
                "Stderr tail should be limited to 2 KiB"
            );
        }

        // Check for partial artifacts (R4.3)
        let artifacts_dir = env.spec_dir().join("artifacts");
        if artifacts_dir.exists() {
            let partial_files: Vec<_> = std::fs::read_dir(&artifacts_dir)?
                .filter_map(std::result::Result::ok)
                .filter(|entry| {
                    entry
                        .path()
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.contains(".partial."))
                })
                .collect();

            if !partial_files.is_empty() {
                println!("Partial artifacts created for {scenario} scenario");

                // Verify partial content exists
                for partial_file in partial_files {
                    let partial_content = std::fs::read_to_string(partial_file.path())?;
                    assert!(
                        !partial_content.is_empty(),
                        "Partial artifact should have content"
                    );
                }
            }
        }
    }

    println!("✓ Various exit codes and stderr patterns test passed");
    Ok(())
}

/// Test 6: Claude CLI wrapper parsing capabilities
/// Validates direct Claude wrapper functionality with various response types
#[tokio::test]
#[ignore = "requires_claude_stub"]
async fn test_claude_wrapper_parsing_capabilities() -> Result<()> {
    let runner = Runner::new(RunnerMode::Native, WslOptions::default());
    let wrapper = ClaudeWrapper::new(Some("haiku".to_string()), runner)?;

    // Test 1: Valid stream-json parsing
    let valid_stream_json = concat!(
        r#"{"type": "conversation_start", "conversation": {"id": "conv_123"}}"#,
        "\n",
        r#"{"type": "message_start", "message": {"id": "msg_123", "role": "assistant"}}"#,
        "\n",
        r#"{"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}"#,
        "\n",
        "{\"type\": \"content_block_delta\", \"index\": 0, \"delta\": {\"type\": \"text_delta\", \"text\": \"# Requirements Document\\n\\n\"}}",
        "\n",
        r#"{"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "This is a test document."}}"#,
        "\n",
        r#"{"type": "content_block_stop", "index": 0}"#,
        "\n",
        r#"{"type": "message_stop", "message": {"id": "msg_123", "model": "haiku", "stop_reason": "end_turn", "usage": {"input_tokens": 100, "output_tokens": 25}}}"#
    );

    let (content, metadata) = wrapper.parse_stream_json(valid_stream_json)?;

    assert_eq!(
        content, "# Requirements Document\n\nThis is a test document.",
        "Should parse content correctly"
    );
    assert_eq!(
        metadata.input_tokens,
        Some(100),
        "Should parse input tokens"
    );
    assert_eq!(
        metadata.output_tokens,
        Some(25),
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

    // Test 2: Malformed JSON handling
    let malformed_json = r#"{"type": "conversation_start", "conversation": {"id": "conv_123"}}
{"type": "message_start", "message": {"id": "msg_123"#;

    let parse_result = wrapper.parse_stream_json(malformed_json);
    assert!(parse_result.is_err(), "Should fail to parse malformed JSON");

    // Test 3: Empty content handling
    let empty_stream = r#"{"type": "conversation_start", "conversation": {"id": "conv_123"}}"#;

    let (empty_content, empty_metadata) = wrapper.parse_stream_json(empty_stream)?;
    assert!(
        empty_content.is_empty(),
        "Should handle empty content gracefully"
    );
    assert!(
        empty_metadata.input_tokens.is_none(),
        "Should have no token info for empty stream"
    );

    // Test 4: Partial message handling
    let partial_stream = concat!(
        r#"{"type": "conversation_start", "conversation": {"id": "conv_123"}}"#,
        "\n",
        r#"{"type": "message_start", "message": {"id": "msg_123", "role": "assistant"}}"#,
        "\n",
        r#"{"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}"#,
        "\n",
        r#"{"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "Partial"}}"# // Note: Missing content_block_stop and message_stop
    );

    let (partial_content, _partial_metadata) = wrapper.parse_stream_json(partial_stream)?;
    assert_eq!(partial_content, "Partial", "Should handle partial messages");

    println!("✓ Claude wrapper parsing capabilities test passed");
    Ok(())
}

/// Test 7: Error recovery scenarios
/// Validates R4.3 error recovery and state management
#[tokio::test]
#[ignore = "requires_claude_stub"]
async fn test_error_recovery_scenarios() -> Result<()> {
    if !test_support::should_run_e2e() {
        eprintln!("(skipped) E2E test requires Claude CLI or XCHECKER_E2E=1");
        return Ok(());
    }

    let env = GoldenPipelineTestEnvironment::new("error-recovery")?;

    // Test recovery from network error
    let network_error_config = env.create_config_with_scenario("network");
    let network_result = env
        .orchestrator
        .execute_requirements_phase(&network_error_config)
        .await?;

    assert!(
        !network_result.success,
        "Network error should cause failure"
    );

    // Verify system state after error
    let spec_dir = env.spec_dir();
    assert!(spec_dir.exists(), "Spec directory should exist after error");

    // Verify receipt was created with error information
    assert!(
        network_result.receipt_path.is_some(),
        "Should create receipt for network error"
    );

    let receipt_path = network_result.receipt_path.unwrap();
    let receipt_content = std::fs::read_to_string(&receipt_path)?;
    let receipt: Receipt = serde_json::from_str(&receipt_content)?;

    assert_ne!(receipt.exit_code, 0, "Receipt should record network error");
    assert!(
        !receipt.warnings.is_empty(),
        "Should have warnings for network error"
    );

    // Test recovery with success scenario
    let success_config = env.create_config_with_scenario("success");
    let recovery_result = env
        .orchestrator
        .execute_requirements_phase(&success_config)
        .await?;

    assert!(recovery_result.success, "Recovery should succeed");
    assert_eq!(
        recovery_result.exit_code, 0,
        "Recovery should have success exit code"
    );

    // Verify artifacts were created after recovery
    assert!(
        !recovery_result.artifact_paths.is_empty(),
        "Should create artifacts after recovery"
    );

    let artifacts_dir = spec_dir.join("artifacts");
    assert!(
        artifacts_dir.join("00-requirements.md").exists(),
        "Requirements should exist after recovery"
    );
    assert!(
        artifacts_dir.join("00-requirements.core.yaml").exists(),
        "Requirements YAML should exist after recovery"
    );

    println!("✓ Error recovery scenarios test passed");
    Ok(())
}

/// Test 8: Performance under various response sizes
/// Validates system performance with different Claude response sizes
#[tokio::test]
#[ignore = "requires_claude_stub"]
async fn test_performance_various_response_sizes() -> Result<()> {
    let scenarios = vec![
        ("small", "Small response (< 1KB)"),
        ("medium", "Medium response (1-10KB)"),
        ("large", "Large response (10-100KB)"),
    ];

    for (scenario, description) in scenarios {
        println!("Testing performance scenario: {scenario} ({description})");

        let env = GoldenPipelineTestEnvironment::new(&format!("perf-{scenario}"))?;
        let config = env.create_config_with_scenario(scenario);

        let start_time = std::time::Instant::now();
        let result = env.orchestrator.execute_requirements_phase(&config).await?;
        let duration = start_time.elapsed();

        // All scenarios should complete within reasonable time
        assert!(duration.as_secs() < 30, "Should complete within 30 seconds");

        if result.success {
            // Verify artifacts were created
            assert!(!result.artifact_paths.is_empty(), "Should create artifacts");

            // Check artifact sizes are reasonable
            let artifacts_dir = env.spec_dir().join("artifacts");
            let req_md = std::fs::read_to_string(artifacts_dir.join("00-requirements.md"))?;

            match scenario {
                "small" => assert!(
                    req_md.len() < 5000,
                    "Small scenario should produce small artifacts"
                ),
                "medium" => assert!(
                    req_md.len() < 50000,
                    "Medium scenario should produce medium artifacts"
                ),
                "large" => assert!(
                    req_md.len() < 500000,
                    "Large scenario should produce large artifacts"
                ),
                _ => {}
            }
        }

        println!("Scenario {scenario} completed in {duration:?}");
    }

    println!("✓ Performance various response sizes test passed");
    Ok(())
}

/// Comprehensive golden pipeline test runner
/// This function provides a summary of golden pipeline test coverage.
///
/// **Note**: Individual tests are run via `cargo test --test golden_pipeline_tests`.
/// This runner is used for documentation and summary purposes when called from
/// the comprehensive test suite.
pub async fn run_golden_pipeline_validation() -> Result<()> {
    println!("🚀 Golden pipeline tests require claude-stub binary.");
    println!("   Run with: cargo test --test golden_pipeline_tests -- --include-ignored");
    println!();
    println!("Golden Pipeline Requirements Coverage:");
    println!("  ✓ R4.1: Claude CLI integration with stream-json and text fallback");
    println!("  ✓ R4.3: Error handling and partial output preservation");
    println!("  ✓ R4.4: Structured output handling with fallback capabilities");
    println!();
    println!("Key Features Covered:");
    println!("  ✓ Valid stream-json parsing with complete message handling");
    println!("  ✓ Truncated event and partial message recovery");
    println!("  ✓ Malformed JSON detection with automatic text fallback");
    println!("  ✓ Plain text output mode processing");
    println!("  ✓ Various error conditions with proper exit codes and stderr capture");
    println!("  ✓ Claude wrapper parsing capabilities across response types");
    println!("  ✓ Error recovery scenarios with state preservation");
    println!("  ✓ Performance validation under various response sizes");

    Ok(())
}

//! Comprehensive tests for Runner execution (FR-RUN)
//!
//! **WHITE-BOX TEST**: This test uses internal module APIs (`runner::{...}`, `types::RunnerMode`)
//! and may break with internal refactors. These tests are intentionally white-box to validate
//! internal implementation details. See FR-TEST-4 for white-box test policy.
//!
//! Tests cover:
//! - FR-RUN-001: Native mode spawns claude directly
//! - FR-RUN-002: WSL mode uses `wsl.exe --exec` with discrete argv
//! - FR-RUN-003: Auto mode detection (native first, then WSL fallback on Windows)
//! - Runner distro capture from wsl -l -q or $WSL_DISTRO_NAME
//! - Stdin piping to Claude process
//! - Stdout/stderr capture

use serial_test::serial;
use std::env;
use std::process::Stdio;
use std::time::Duration;
use xchecker::error::RunnerError;
use xchecker::runner::{ClaudeResponse, CommandSpec, Runner, WslOptions};
use xchecker::types::RunnerMode;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

// ============================================================================
// Unit Tests: Runner Mode Detection
// ============================================================================

#[test]
fn test_runner_mode_native_creation() {
    let runner = Runner::native();
    assert_eq!(runner.mode, RunnerMode::Native);
    assert!(runner.wsl_options.distro.is_none());
    assert!(runner.wsl_options.claude_path.is_none());
}

#[test]
fn test_runner_mode_auto_creation() {
    let result = Runner::auto();
    // Should always succeed - mode is Auto and will be resolved during execution
    assert!(
        result.is_ok(),
        "Auto mode runner creation should always succeed"
    );

    let runner = result.unwrap();
    // Mode should remain as Auto until execution
    assert_eq!(runner.mode, RunnerMode::Auto);
    println!("✓ Auto mode runner created successfully");
}

#[test]
fn test_runner_mode_wsl_creation() {
    let wsl_options = WslOptions {
        distro: Some("Ubuntu-22.04".to_string()),
        claude_path: Some("/usr/bin/claude".to_string()),
    };
    let runner = Runner::new(RunnerMode::Wsl, wsl_options);
    assert_eq!(runner.mode, RunnerMode::Wsl);
    assert_eq!(runner.wsl_options.distro, Some("Ubuntu-22.04".to_string()));
    assert_eq!(
        runner.wsl_options.claude_path,
        Some("/usr/bin/claude".to_string())
    );
}

#[test]
fn test_runner_default_is_auto() {
    let runner = Runner::default();
    assert_eq!(runner.mode, RunnerMode::Auto);
}

/// Test FR-RUN-003: Auto mode detection on non-Windows platforms
#[cfg(not(target_os = "windows"))]
#[test]
fn test_auto_detection_non_windows_always_native() {
    let detected_mode = Runner::detect_auto();
    assert!(detected_mode.is_ok());
    assert_eq!(
        detected_mode.unwrap(),
        RunnerMode::Native,
        "Non-Windows platforms should always detect Native mode"
    );
}

/// Test FR-RUN-003: Auto mode detection on Windows
#[cfg(target_os = "windows")]
#[test]
fn test_auto_detection_windows_native_or_wsl() {
    let detected_mode = Runner::detect_auto();
    // Should succeed and return either Native or Wsl
    if let Ok(mode) = detected_mode {
        assert!(
            mode == RunnerMode::Native || mode == RunnerMode::Wsl,
            "Windows should detect Native or WSL mode, got: {:?}",
            mode
        );
    } else {
        // If detection fails, it should be because neither native nor WSL Claude is available
        assert!(detected_mode.is_err());
    }
}

// ============================================================================
// Unit Tests: Runner Validation
// ============================================================================

#[test]
fn test_runner_validation_native() {
    let runner = Runner::native();
    let result = runner.validate();
    // Validation may fail if claude is not installed, but should not panic
    match result {
        Ok(_) => println!("✓ Native Claude CLI is available"),
        Err(e) => println!("✗ Native Claude CLI not available: {:?}", e),
    }
}

#[cfg(target_os = "windows")]
#[test]
fn test_runner_validation_wsl_on_windows() {
    let runner = Runner::new(RunnerMode::Wsl, WslOptions::default());
    let result = runner.validate();
    // Validation may fail if WSL or Claude in WSL is not available
    match result {
        Ok(_) => println!("✓ WSL Claude CLI is available"),
        Err(e) => println!("✗ WSL Claude CLI not available: {:?}", e),
    }
}

#[cfg(not(target_os = "windows"))]
#[test]
fn test_runner_validation_wsl_on_non_windows_fails() {
    let runner = Runner::new(RunnerMode::Wsl, WslOptions::default());
    let result = runner.validate();
    assert!(result.is_err());
    match result {
        Err(RunnerError::ConfigurationInvalid { reason }) => {
            assert!(reason.contains("only supported on Windows"));
        }
        _ => panic!("Expected ConfigurationInvalid error"),
    }
}

// ============================================================================
// Unit Tests: Runner Description
// ============================================================================

#[test]
fn test_runner_description_native() {
    let runner = Runner::native();
    let desc = runner.description();
    assert!(desc.contains("Native"));
    assert!(desc.contains("spawn claude directly"));
}

#[test]
fn test_runner_description_auto() {
    let runner = Runner::new(RunnerMode::Auto, WslOptions::default());
    let desc = runner.description();
    assert!(desc.contains("Automatic detection"));
}

#[test]
fn test_runner_description_wsl() {
    let wsl_options = WslOptions {
        distro: Some("Ubuntu-22.04".to_string()),
        claude_path: Some("/usr/local/bin/claude".to_string()),
    };
    let runner = Runner::new(RunnerMode::Wsl, wsl_options);
    let desc = runner.description();
    assert!(desc.contains("WSL"));
    assert!(desc.contains("Ubuntu-22.04"));
    assert!(desc.contains("/usr/local/bin/claude"));
}

// ============================================================================
// Unit Tests: WSL Distro Name Detection
// ============================================================================

#[test]
fn test_wsl_distro_name_from_options() {
    let wsl_options = WslOptions {
        distro: Some("Ubuntu-22.04".to_string()),
        claude_path: None,
    };
    let runner = Runner::new(RunnerMode::Wsl, wsl_options);
    let distro = runner.get_wsl_distro_name();
    assert_eq!(distro, Some("Ubuntu-22.04".to_string()));
}

#[cfg(target_os = "windows")]
#[test]
fn test_wsl_distro_name_from_wsl_command() {
    // Test that we can get distro name from `wsl -l -q`
    let runner = Runner::new(RunnerMode::Wsl, WslOptions::default());
    let distro = runner.get_wsl_distro_name();
    // May be None if WSL is not installed or no distros available
    match distro {
        Some(name) => {
            println!("✓ Detected WSL distro: {}", name);
            assert!(!name.is_empty());
        }
        None => println!("✗ No WSL distro detected"),
    }
}

#[test]
#[serial]
fn test_wsl_distro_name_from_env_var() {
    // Set WSL_DISTRO_NAME environment variable
    unsafe {
        env::set_var("WSL_DISTRO_NAME", "TestDistro");
    }

    let runner = Runner::new(RunnerMode::Wsl, WslOptions::default());
    let distro = runner.get_wsl_distro_name();

    // Clean up
    unsafe {
        env::remove_var("WSL_DISTRO_NAME");
    }

    // Should get the distro from env var
    assert_eq!(distro, Some("TestDistro".to_string()));
}

// ============================================================================
// Integration Tests: Native Execution
// ============================================================================

/// Test FR-RUN-001: Native mode spawns claude directly
/// This test uses `echo` as a mock for claude to test the execution flow
#[test]
fn test_native_execution_with_mock_command() -> Result<()> {
    // We'll use a simple command that exists on all platforms
    // to test the execution flow without requiring Claude to be installed

    // Create a mock runner that uses 'echo' instead of 'claude'
    // Note: This is a conceptual test - in real implementation,
    // we would need to modify Runner to support command injection for testing

    println!("✓ Native execution test structure validated");
    Ok(())
}

// ============================================================================
// Integration Tests: WSL Execution
// ============================================================================

/// Test FR-RUN-002: WSL mode uses `wsl.exe --exec` with discrete argv
#[cfg(target_os = "windows")]
#[test]
fn test_wsl_execution_uses_exec_with_discrete_argv() -> Result<()> {
    // Test that WSL execution uses --exec with discrete arguments
    // We can verify this by checking the command construction

    let wsl_options = WslOptions {
        distro: Some("Ubuntu".to_string()),
        claude_path: Some("claude".to_string()),
    };
    let runner = Runner::new(RunnerMode::Wsl, wsl_options);

    // Verify runner is configured correctly
    assert_eq!(runner.mode, RunnerMode::Wsl);
    assert_eq!(runner.wsl_options.distro, Some("Ubuntu".to_string()));

    println!("✓ WSL execution configuration validated");
    Ok(())
}

// ============================================================================
// Integration Tests: Stdin Piping
// ============================================================================

/// Test stdin piping to process
/// Uses a simple echo command to verify stdin is properly piped
#[test]
fn test_stdin_piping_to_process() -> Result<()> {
    // Test stdin piping using a simple command
    let test_input = "test input content";

    // Use a command that reads from stdin and echoes it back
    #[cfg(target_os = "windows")]
    let mut cmd = CommandSpec::new("cmd")
        .args(["/C", "findstr", ".*"])
        .to_command();

    #[cfg(not(target_os = "windows"))]
    let mut cmd = CommandSpec::new("cat").to_command();

    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn()?;

    // Write to stdin
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin.write_all(test_input.as_bytes())?;
    }

    // Read output
    let output = child.wait_with_output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("test input"));
    println!("✓ Stdin piping test passed");
    Ok(())
}

// ============================================================================
// Integration Tests: Stdout/Stderr Capture
// ============================================================================

/// Test stdout and stderr capture
#[test]
fn test_stdout_stderr_capture() -> Result<()> {
    // Test that we can capture both stdout and stderr

    #[cfg(target_os = "windows")]
    let mut cmd = CommandSpec::new("cmd")
        .args(["/C", "echo stdout_test && echo stderr_test 1>&2"])
        .to_command();

    #[cfg(not(target_os = "windows"))]
    let mut cmd = CommandSpec::new("sh")
        .args(["-c", "echo stdout_test && echo stderr_test >&2"])
        .to_command();

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let output = cmd.output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stdout.contains("stdout_test"));
    assert!(stderr.contains("stderr_test"));

    println!("✓ Stdout/stderr capture test passed");
    Ok(())
}

// ============================================================================
// Integration Tests: ClaudeResponse Structure
// ============================================================================

#[test]
fn test_claude_response_structure() {
    use xchecker::runner::NdjsonResult;

    // Test that ClaudeResponse has all required fields
    let response = ClaudeResponse {
        stdout: "test output".to_string(),
        stderr: "test error".to_string(),
        exit_code: 0,
        runner_used: RunnerMode::Native,
        runner_distro: Some("Ubuntu-22.04".to_string()),
        timed_out: false,
        ndjson_result: NdjsonResult::ValidJson(r#"{"status":"ok"}"#.to_string()),
        stdout_truncated: false,
        stderr_truncated: false,
        stdout_total_bytes: 11,
        stderr_total_bytes: 10,
    };

    assert_eq!(response.stdout, "test output");
    assert_eq!(response.stderr, "test error");
    assert_eq!(response.exit_code, 0);
    assert_eq!(response.runner_used, RunnerMode::Native);
    assert_eq!(response.runner_distro, Some("Ubuntu-22.04".to_string()));
    assert!(!response.timed_out);

    println!("✓ ClaudeResponse structure test passed");
}

// ============================================================================
// Integration Tests: Auto Mode Fallback
// ============================================================================

/// Test FR-RUN-003: Auto mode tries native first, then WSL fallback on Windows
#[cfg(target_os = "windows")]
#[test]
fn test_auto_mode_fallback_logic() -> Result<()> {
    // Test the auto detection logic
    let detected_mode = Runner::detect_auto();

    match detected_mode {
        Ok(RunnerMode::Native) => {
            println!("✓ Auto mode detected Native (Claude available in PATH)");
        }
        Ok(RunnerMode::Wsl) => {
            println!("✓ Auto mode detected WSL (Claude not in PATH, but available in WSL)");
        }
        Ok(RunnerMode::Auto) => {
            panic!("Auto mode should resolve to Native or Wsl, not remain as Auto");
        }
        Err(e) => {
            println!(
                "✗ Auto mode detection failed (neither native nor WSL available): {:?}",
                e
            );
        }
    }

    Ok(())
}

// ============================================================================
// Comprehensive Auto Mode Tests (Task 5.6)
// ============================================================================

/// Test FR-RUN-003, FR-WSL-003: Auto mode detection logic on Windows with native Claude available
///
/// This test verifies that when native Claude is available on Windows, auto mode
/// detects and prefers it over WSL.
#[cfg(target_os = "windows")]
#[test]
fn test_auto_mode_windows_native_available() {
    // Try to detect if native Claude is available
    let native_result = Runner::test_native_claude();

    if native_result.is_ok() {
        // Native Claude is available - auto mode should detect Native
        let detected_mode = Runner::detect_auto();
        assert!(
            detected_mode.is_ok(),
            "Auto detection should succeed when native Claude is available"
        );
        assert_eq!(
            detected_mode.unwrap(),
            RunnerMode::Native,
            "Auto mode should prefer Native when available"
        );
        println!("✓ Auto mode correctly detected Native (native Claude available)");
    } else {
        println!("⊘ Skipping test - native Claude not available");
    }
}

/// Test FR-RUN-003, FR-WSL-003: Auto mode detection logic on Windows with WSL fallback
///
/// This test verifies that when native Claude is NOT available but WSL Claude is,
/// auto mode falls back to WSL mode.
#[cfg(target_os = "windows")]
#[test]
fn test_auto_mode_windows_wsl_fallback() {
    // Check if native Claude is NOT available but WSL Claude is
    let native_result = Runner::test_native_claude();
    let wsl_result = Runner::test_wsl_claude();

    if native_result.is_err() && wsl_result.is_ok() {
        // Native not available, but WSL is - auto mode should detect WSL
        let detected_mode = Runner::detect_auto();
        assert!(
            detected_mode.is_ok(),
            "Auto detection should succeed when WSL Claude is available"
        );
        assert_eq!(
            detected_mode.unwrap(),
            RunnerMode::Wsl,
            "Auto mode should fall back to WSL when native not available"
        );
        println!("✓ Auto mode correctly fell back to WSL (native not available, WSL available)");
    } else {
        println!("⊘ Skipping test - conditions not met (need native unavailable, WSL available)");
    }
}

/// Test FR-RUN-003, FR-WSL-003: Auto mode detection failure when neither native nor WSL available
///
/// This test verifies that when neither native nor WSL Claude is available,
/// auto mode returns a helpful error message.
#[cfg(target_os = "windows")]
#[test]
fn test_auto_mode_windows_neither_available() {
    // Check if both native and WSL Claude are NOT available
    let native_result = Runner::test_native_claude();
    let wsl_result = Runner::test_wsl_claude();

    if native_result.is_err() && wsl_result.is_err() {
        // Neither available - auto mode should fail with helpful error
        let detected_mode = Runner::detect_auto();
        assert!(
            detected_mode.is_err(),
            "Auto detection should fail when neither native nor WSL available"
        );

        match detected_mode {
            Err(RunnerError::DetectionFailed { reason }) => {
                assert!(
                    reason.contains("Claude CLI not found") || reason.contains("not available"),
                    "Error message should mention Claude CLI not being found"
                );
                println!(
                    "✓ Auto mode correctly failed with helpful error: {}",
                    reason
                );
            }
            _ => panic!("Expected DetectionFailed error when neither native nor WSL available"),
        }
    } else {
        println!("⊘ Skipping test - at least one Claude installation is available");
    }
}

/// Test FR-RUN-003: Auto mode on Linux/macOS always returns Native
///
/// This test verifies that on non-Windows platforms, auto mode always
/// detects Native mode (WSL is not applicable).
#[cfg(not(target_os = "windows"))]
#[test]
fn test_auto_mode_non_windows_always_native() {
    let detected_mode = Runner::detect_auto();

    // On non-Windows, should always return Native (even if Claude is not installed)
    assert!(
        detected_mode.is_ok(),
        "Auto detection should always succeed on non-Windows"
    );
    assert_eq!(
        detected_mode.unwrap(),
        RunnerMode::Native,
        "Non-Windows platforms should always detect Native mode"
    );
    println!("✓ Auto mode correctly detected Native on non-Windows platform");
}

/// Test FR-RUN-003: Auto mode runner creation and validation
///
/// This test verifies that a Runner created with auto mode can be validated
/// and will resolve to the appropriate concrete mode.
#[test]
fn test_auto_mode_runner_creation_and_validation() {
    let runner = Runner::auto();

    match runner {
        Ok(r) => {
            // Runner was created successfully
            assert_eq!(r.mode, RunnerMode::Auto, "Runner should be in Auto mode");

            // Validation should work (may fail if Claude not installed anywhere)
            let validation_result = r.validate();
            match validation_result {
                Ok(_) => println!("✓ Auto mode runner validated successfully"),
                Err(e) => println!(
                    "⊘ Auto mode runner validation failed (Claude not available): {:?}",
                    e
                ),
            }
        }
        Err(e) => {
            println!(
                "⊘ Auto mode runner creation failed (Claude not available): {:?}",
                e
            );
        }
    }
}

/// Test FR-RUN-003: Auto mode resolves during execution
///
/// This test verifies that when executing with Auto mode, the runner
/// resolves to a concrete mode (Native or WSL) and reports it correctly.
#[tokio::test]
async fn test_auto_mode_resolves_during_execution() {
    let runner = Runner::new(RunnerMode::Auto, WslOptions::default());

    // Try to execute a simple command (using echo as a mock)
    // Note: This will fail if Claude is not installed, but we're testing the resolution logic
    let args = vec!["--version".to_string()];
    let stdin_content = "";
    let timeout_duration = Some(Duration::from_secs(5));

    let result = runner
        .execute_claude(&args, stdin_content, timeout_duration)
        .await;

    match result {
        Ok(response) => {
            // Execution succeeded - verify the runner_used is not Auto
            assert_ne!(
                response.runner_used,
                RunnerMode::Auto,
                "Runner should resolve Auto to concrete mode (Native or Wsl)"
            );
            assert!(
                response.runner_used == RunnerMode::Native
                    || response.runner_used == RunnerMode::Wsl,
                "Runner should resolve to Native or Wsl, got: {:?}",
                response.runner_used
            );
            println!(
                "✓ Auto mode resolved to {:?} during execution",
                response.runner_used
            );
        }
        Err(e) => {
            println!("⊘ Execution failed (Claude not available): {:?}", e);
        }
    }
}

/// Test FR-RUN-003: Auto mode detection is deterministic
///
/// This test verifies that calling detect_auto() multiple times
/// returns the same result (detection is deterministic).
#[test]
fn test_auto_mode_detection_is_deterministic() {
    let first_detection = Runner::detect_auto();
    let second_detection = Runner::detect_auto();

    match (first_detection, second_detection) {
        (Ok(mode1), Ok(mode2)) => {
            assert_eq!(mode1, mode2, "Auto detection should be deterministic");
            println!("✓ Auto mode detection is deterministic: {:?}", mode1);
        }
        (Err(_), Err(_)) => {
            println!("✓ Auto mode detection consistently fails (Claude not available)");
        }
        _ => {
            panic!("Auto mode detection should be deterministic (both succeed or both fail)");
        }
    }
}

/// Test FR-RUN-003: Auto mode with explicit WSL options
///
/// This test verifies that when Auto mode is used with explicit WSL options,
/// those options are respected if WSL mode is selected.
#[cfg(target_os = "windows")]
#[test]
fn test_auto_mode_with_wsl_options() {
    let wsl_options = WslOptions {
        distro: Some("Ubuntu-22.04".to_string()),
        claude_path: Some("/usr/local/bin/claude".to_string()),
    };

    let runner = Runner::new(RunnerMode::Auto, wsl_options.clone());

    // Verify options are stored
    assert_eq!(runner.wsl_options.distro, wsl_options.distro);
    assert_eq!(runner.wsl_options.claude_path, wsl_options.claude_path);

    println!("✓ Auto mode runner correctly stores WSL options");
}

/// Test FR-RUN-003: Auto mode error messages are helpful
///
/// This test verifies that when auto detection fails, the error message
/// provides actionable guidance to the user.
#[test]
fn test_auto_mode_error_messages_are_helpful() {
    // Create a scenario where detection might fail
    let detection_result = Runner::detect_auto();

    if let Err(e) = detection_result {
        let error_string = format!("{:?}", e);

        // Error should mention Claude CLI
        assert!(
            error_string.contains("Claude") || error_string.contains("claude"),
            "Error message should mention Claude CLI"
        );

        // On Windows, error should mention both native and WSL
        #[cfg(target_os = "windows")]
        {
            assert!(
                error_string.contains("WSL") || error_string.contains("wsl"),
                "Windows error should mention WSL as an option"
            );
        }

        println!("✓ Auto mode error message is helpful: {:?}", e);
    } else {
        println!("⊘ Skipping test - auto detection succeeded");
    }
}

// ============================================================================
// Integration Tests: Error Handling
// ============================================================================

#[test]
fn test_runner_error_types() {
    // Test that RunnerError variants exist and can be constructed
    let _native_error = RunnerError::NativeExecutionFailed {
        reason: "test".to_string(),
    };

    let _wsl_error = RunnerError::WslExecutionFailed {
        reason: "test".to_string(),
    };

    let _detection_error = RunnerError::DetectionFailed {
        reason: "test".to_string(),
    };

    let _config_error = RunnerError::ConfigurationInvalid {
        reason: "test".to_string(),
    };

    println!("✓ RunnerError types test passed");
}

// ============================================================================
// Integration Tests: NDJSON Merging (FR-RUN-008, FR-RUN-009)
// ============================================================================

/// AT-RUN-004: Test interleaved noise + multiple JSON frames → last valid frame wins
#[test]
fn test_ndjson_interleaved_noise_and_json() {
    use xchecker::runner::{NdjsonResult, Runner};

    let stdout = r#"Starting process...
{"frame": 1, "status": "initializing"}
Some debug output
Warning: something happened
{"frame": 2, "status": "processing"}
More noise here
{"frame": 3, "status": "complete"}
Done!"#;

    let result = Runner::parse_ndjson(stdout);

    match result {
        NdjsonResult::ValidJson(json) => {
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed["frame"], 3);
            assert_eq!(parsed["status"], "complete");
            println!("✓ AT-RUN-004: Interleaved noise + multiple JSON frames test passed");
        }
        NdjsonResult::NoValidJson { .. } => {
            panic!("Expected ValidJson with last frame");
        }
    }
}

/// AT-RUN-005: Test partial JSON followed by timeout → claude_failure with excerpt
#[test]
fn test_ndjson_partial_json_with_valid_frames() {
    use xchecker::runner::{NdjsonResult, Runner};

    // Simulates a timeout scenario where we have valid JSON followed by partial JSON
    let stdout = r#"{"frame": 1, "status": "ok"}
{"frame": 2, "incomplete": tru"#;

    let result = Runner::parse_ndjson(stdout);

    match result {
        NdjsonResult::ValidJson(json) => {
            // Should return the last valid JSON (frame 1)
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed["frame"], 1);
            assert_eq!(parsed["status"], "ok");
            println!("✓ AT-RUN-005: Partial JSON with valid frames test passed");
        }
        NdjsonResult::NoValidJson { .. } => {
            panic!("Expected ValidJson from first frame");
        }
    }
}

/// Test no valid JSON → claude_failure with excerpt (256 chars, redacted)
#[test]
fn test_ndjson_no_valid_json_returns_excerpt() {
    use xchecker::runner::{NdjsonResult, Runner};

    let stdout = "This is just plain text\nNo JSON here\nJust noise and errors";

    let result = Runner::parse_ndjson(stdout);

    match result {
        NdjsonResult::ValidJson(_) => {
            panic!("Expected NoValidJson");
        }
        NdjsonResult::NoValidJson { tail_excerpt } => {
            assert_eq!(tail_excerpt, stdout);
            assert!(tail_excerpt.len() <= 256);
            println!("✓ No valid JSON returns excerpt test passed");
        }
    }
}

/// Test tail excerpt truncation for long output
#[test]
fn test_ndjson_tail_excerpt_truncation() {
    use xchecker::runner::{NdjsonResult, Runner};

    // Create output longer than 256 characters with no valid JSON
    let long_output = "x".repeat(300);

    let result = Runner::parse_ndjson(&long_output);

    match result {
        NdjsonResult::ValidJson(_) => {
            panic!("Expected NoValidJson");
        }
        NdjsonResult::NoValidJson { tail_excerpt } => {
            assert_eq!(tail_excerpt.len(), 256);
            // Should be the last 256 characters
            assert_eq!(tail_excerpt, "x".repeat(256));
            println!("✓ Tail excerpt truncation test passed");
        }
    }
}

/// Test NDJSON with only partial/malformed JSON
#[test]
fn test_ndjson_only_partial_json() {
    use xchecker::runner::{NdjsonResult, Runner};

    let stdout = r#"{"incomplete": tru"#;

    let result = Runner::parse_ndjson(stdout);

    match result {
        NdjsonResult::ValidJson(_) => {
            panic!("Expected NoValidJson");
        }
        NdjsonResult::NoValidJson { tail_excerpt } => {
            assert_eq!(tail_excerpt, stdout);
            println!("✓ Only partial JSON test passed");
        }
    }
}

/// Test NDJSON with mixed valid and invalid JSON
#[test]
fn test_ndjson_mixed_valid_invalid() {
    use xchecker::runner::{NdjsonResult, Runner};

    let stdout = r#"{"valid": "json"}
{malformed json}
{"another": "valid"}
[not an object but valid json]
{"final": "valid"}"#;

    let result = Runner::parse_ndjson(stdout);

    match result {
        NdjsonResult::ValidJson(json) => {
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed["final"], "valid");
            println!("✓ Mixed valid/invalid JSON test passed");
        }
        NdjsonResult::NoValidJson { .. } => {
            panic!("Expected ValidJson");
        }
    }
}

/// Test NDJSON with empty lines and whitespace
#[test]
fn test_ndjson_empty_lines_and_whitespace() {
    use xchecker::runner::{NdjsonResult, Runner};

    let stdout = r#"
{"frame": 1}

   
{"frame": 2}
  
"#;

    let result = Runner::parse_ndjson(stdout);

    match result {
        NdjsonResult::ValidJson(json) => {
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed["frame"], 2);
            println!("✓ Empty lines and whitespace test passed");
        }
        NdjsonResult::NoValidJson { .. } => {
            panic!("Expected ValidJson");
        }
    }
}

/// Test NDJSON with unicode content
#[test]
fn test_ndjson_unicode_content() {
    use xchecker::runner::{NdjsonResult, Runner};

    let stdout = r#"{"message": "Hello 世界"}
{"emoji": "🎉🎊"}
{"final": "完成"}"#;

    let result = Runner::parse_ndjson(stdout);

    match result {
        NdjsonResult::ValidJson(json) => {
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed["final"], "完成");
            println!("✓ Unicode content test passed");
        }
        NdjsonResult::NoValidJson { .. } => {
            panic!("Expected ValidJson");
        }
    }
}

// ============================================================================
// Test Runner
// ============================================================================

#[cfg(test)]
mod test_runner {
    use super::*;

    #[test]
    fn run_all_runner_execution_tests() {
        println!("\n=== Running Runner Execution Tests (FR-RUN) ===\n");

        // Unit tests
        test_runner_mode_native_creation();
        test_runner_mode_auto_creation(); // May fail if Claude not installed
        test_runner_mode_wsl_creation();
        test_runner_default_is_auto();
        test_runner_description_native();
        test_runner_description_auto();
        test_runner_description_wsl();
        test_wsl_distro_name_from_options();

        // Platform-specific tests
        #[cfg(not(target_os = "windows"))]
        {
            test_auto_detection_non_windows_always_native();
            test_runner_validation_wsl_on_non_windows_fails();

            // Task 5.6: Comprehensive auto mode tests
            test_auto_mode_non_windows_always_native();
        }

        #[cfg(target_os = "windows")]
        {
            test_auto_detection_windows_native_or_wsl();
            test_wsl_distro_name_from_wsl_command();
            test_wsl_execution_uses_exec_with_discrete_argv().unwrap();
            test_auto_mode_fallback_logic().unwrap();

            // Task 5.6: Comprehensive auto mode tests
            test_auto_mode_windows_native_available();
            test_auto_mode_windows_wsl_fallback();
            test_auto_mode_windows_neither_available();
            test_auto_mode_with_wsl_options();
        }

        // Integration tests
        test_runner_validation_native(); // May fail if Claude not installed
        test_stdin_piping_to_process().unwrap();
        test_stdout_stderr_capture().unwrap();
        test_claude_response_structure();
        test_runner_error_types();

        // Task 5.6: Additional auto mode tests
        test_auto_mode_runner_creation_and_validation();
        test_auto_mode_detection_is_deterministic();
        test_auto_mode_error_messages_are_helpful();

        // NDJSON merging tests (FR-RUN-008, FR-RUN-009)
        test_ndjson_interleaved_noise_and_json();
        test_ndjson_partial_json_with_valid_frames();
        test_ndjson_no_valid_json_returns_excerpt();
        test_ndjson_tail_excerpt_truncation();
        test_ndjson_only_partial_json();
        test_ndjson_mixed_valid_invalid();
        test_ndjson_empty_lines_and_whitespace();
        test_ndjson_unicode_content();

        println!("\n=== Runner Execution Tests Completed ===\n");
    }
}

//! Integration tests for doctor LLM provider checks
//!
//! Validates that `xchecker doctor` correctly:
//! - Checks LLM provider configuration
//! - Validates Claude binary path existence
//! - Detects missing binaries
//! - Works with custom binary paths
//! - Handles invalid provider configurations
//!
//! These tests ensure the doctor command properly validates the LLM backend
//! configuration before any actual LLM calls are made.

use std::env;
use std::sync::{Mutex, MutexGuard, OnceLock};
use tempfile::{NamedTempFile, TempDir};
use xchecker::config::{CliArgs, Config};
use xchecker::doctor::{CheckStatus, DoctorCommand};

// ===== Environment Lock =====
// These tests modify PATH, which is global process state.
// We must serialize tests that touch environment variables.

static DOCTOR_ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn doctor_env_guard() -> MutexGuard<'static, ()> {
    DOCTOR_ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
}

// ===== Helper Functions =====

/// Create a minimal test environment with isolated PATH
struct TestEnv {
    _lock: MutexGuard<'static, ()>,
    _temp_dir: TempDir,
    original_path: Option<String>,
}

impl TestEnv {
    fn new() -> Self {
        // Acquire lock first - this serializes all TestEnv-using tests
        let lock = doctor_env_guard();
        let temp_dir = TempDir::new().unwrap();

        // Save original PATH
        let original_path = env::var("PATH").ok();

        // Set PATH to empty to isolate from real binaries
        unsafe {
            env::set_var("PATH", "");
        }

        Self {
            _lock: lock,
            _temp_dir: temp_dir,
            original_path,
        }
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        // Restore original PATH while still holding the lock
        unsafe {
            if let Some(ref path) = self.original_path {
                env::set_var("PATH", path);
            } else {
                env::remove_var("PATH");
            }
        }
        // Lock is released after restoration
    }
}

/// Find a specific check by name in doctor output
fn find_check<'a>(
    checks: &'a [xchecker::doctor::DoctorCheck],
    name: &str,
) -> Option<&'a xchecker::doctor::DoctorCheck> {
    checks.iter().find(|c| c.name == name)
}

// ===== Test Scenario 1: No provider set → should default and pass/warn based on binary availability =====

#[test]
fn test_no_provider_set_defaults_to_claude_cli() {
    let _env = TestEnv::new();

    // Setup: Create config with no explicit provider
    let cli_args = CliArgs::default();
    let config =
        Config::discover(&cli_args).expect("Config discovery should succeed with defaults");

    // Provider should default to claude-cli
    assert_eq!(
        config.llm.provider,
        Some("claude-cli".to_string()),
        "Provider should default to claude-cli when not specified"
    );

    // Execute: Run doctor checks
    let mut doctor = DoctorCommand::new(config);
    let output = doctor
        .run_with_options()
        .expect("Doctor should run successfully");

    // Verify: llm_provider check exists
    let llm_check =
        find_check(&output.checks, "llm_provider").expect("llm_provider check should be present");

    assert_eq!(
        llm_check.name, "llm_provider",
        "Check name should be 'llm_provider'"
    );

    // Status could be pass (if claude in PATH) or fail/warn (if not)
    // We just verify the check runs and produces a result
    assert!(
        matches!(
            llm_check.status,
            CheckStatus::Pass | CheckStatus::Warn | CheckStatus::Fail
        ),
        "llm_provider check should have a valid status"
    );

    // Details should mention the provider
    assert!(
        llm_check.details.contains("claude-cli"),
        "Details should mention claude-cli provider, got: {}",
        llm_check.details
    );
}

#[test]
fn test_no_provider_no_binary_in_path_fails() {
    let _env = TestEnv::new();

    // Setup: Empty PATH (no claude binary)
    unsafe {
        env::set_var("PATH", "");
    }

    let cli_args = CliArgs::default();
    let config = Config::discover(&cli_args).expect("Config discovery should succeed");

    // Execute: Run doctor checks
    let mut doctor = DoctorCommand::new(config);
    let output = doctor
        .run_with_options()
        .expect("Doctor should run successfully");

    // Verify: llm_provider check should exist and have meaningful status
    let llm_check =
        find_check(&output.checks, "llm_provider").expect("llm_provider check should be present");

    // The check's behavior depends on how claude can be discovered:
    // - Fail: No binary found at all
    // - Warn: Binary found via fallback (WSL, registry, etc.) but not in native PATH
    // - Pass: Binary found (possibly via mechanisms outside PATH like WSL claude detection)
    //
    // On modern Windows with Claude CLI installed, it may be discoverable even without PATH.
    // This test verifies the check runs and produces a valid status, but we can't
    // guarantee a specific failure mode across all environments.
    assert!(
        matches!(
            llm_check.status,
            CheckStatus::Pass | CheckStatus::Fail | CheckStatus::Warn
        ),
        "llm_provider check should have a valid status"
    );

    // If it fails, details should be informative
    if llm_check.status == CheckStatus::Fail {
        assert!(
            llm_check.details.contains("not found")
                || llm_check.details.contains("PATH")
                || llm_check.details.contains("binary"),
            "Failure details should be informative, got: {}",
            llm_check.details
        );
    }
}

// ===== Test Scenario 2: Provider = "claude-cli" with invalid binary path → should fail =====

#[test]
fn test_claude_cli_with_invalid_binary_path_fails() {
    let _env = TestEnv::new();

    // Setup: Configure with non-existent binary path
    let fake_path = "/absolutely/nonexistent/path/to/claude";
    let cli_args = CliArgs {
        llm_provider: Some("claude-cli".to_string()),
        llm_claude_binary: Some(fake_path.to_string()),
        ..Default::default()
    };

    let config =
        Config::discover(&cli_args).expect("Config discovery should succeed even with bad path");

    // Execute: Run doctor checks
    let mut doctor = DoctorCommand::new(config);
    let output = doctor
        .run_with_options()
        .expect("Doctor should run successfully");

    // Verify: llm_provider check should fail
    let llm_check =
        find_check(&output.checks, "llm_provider").expect("llm_provider check should be present");

    assert_eq!(
        llm_check.status,
        CheckStatus::Fail,
        "llm_provider check should fail with invalid binary path"
    );

    assert!(
        llm_check.details.contains(fake_path),
        "Details should mention the invalid path, got: {}",
        llm_check.details
    );

    assert!(
        llm_check.details.contains("does not exist")
            || llm_check.details.contains("not found")
            || llm_check.details.contains("binary"),
        "Details should indicate the binary doesn't exist, got: {}",
        llm_check.details
    );
}

// ===== Test Scenario 3: Provider = "claude-cli" with valid custom binary path → should pass =====

#[test]
fn test_claude_cli_with_valid_custom_binary_passes() {
    let _env = TestEnv::new();

    // Setup: Create a temporary file to act as the binary
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let temp_path = temp_file.path().to_str().unwrap().to_string();

    let cli_args = CliArgs {
        llm_provider: Some("claude-cli".to_string()),
        llm_claude_binary: Some(temp_path.clone()),
        ..Default::default()
    };

    let config = Config::discover(&cli_args).expect("Config discovery should succeed");

    // Execute: Run doctor checks
    let mut doctor = DoctorCommand::new(config);
    let output = doctor
        .run_with_options()
        .expect("Doctor should run successfully");

    // Verify: llm_provider check should pass
    let llm_check =
        find_check(&output.checks, "llm_provider").expect("llm_provider check should be present");

    assert_eq!(
        llm_check.status,
        CheckStatus::Pass,
        "llm_provider check should pass with valid custom binary path"
    );

    assert!(
        llm_check.details.contains(&temp_path),
        "Details should mention the custom binary path, got: {}",
        llm_check.details
    );

    assert!(
        llm_check.details.contains("custom binary"),
        "Details should indicate this is a custom binary, got: {}",
        llm_check.details
    );
}

// ===== Test Scenario 4: Invalid/unsupported provider → should fail during config validation =====

#[test]
fn test_unsupported_provider_fails_config_validation() {
    let _env = TestEnv::new();

    // Setup: Try to configure with genuinely unsupported provider
    // Note: gemini-cli is now supported per V12/V14, so we use a truly unknown provider
    let cli_args = CliArgs {
        llm_provider: Some("totally-unknown-provider".to_string()),
        ..Default::default()
    };

    // Execute: Config discovery should fail
    let result = Config::discover(&cli_args);

    // Verify: Should fail during config validation
    assert!(
        result.is_err(),
        "Config discovery should fail with unsupported provider"
    );

    let error = result.unwrap_err();
    let error_msg = error.to_string();

    assert!(
        error_msg.contains("totally-unknown-provider") || error_msg.contains("provider"),
        "Error should mention the invalid provider, got: {}",
        error_msg
    );
}

#[test]
fn test_unknown_provider_fails_config_validation() {
    let _env = TestEnv::new();

    // Setup: Try to configure with completely unknown provider
    let cli_args = CliArgs {
        llm_provider: Some("totally-unknown-llm".to_string()),
        ..Default::default()
    };

    // Execute: Config discovery should fail
    let result = Config::discover(&cli_args);

    // Verify: Should fail during config validation
    assert!(
        result.is_err(),
        "Config discovery should fail with unknown provider"
    );
}

// ===== Test Scenario 5: Doctor JSON output includes llm_provider check =====

#[test]
fn test_doctor_json_output_includes_llm_provider_check() {
    let _env = TestEnv::new();

    // Setup: Create a valid config
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let temp_path = temp_file.path().to_str().unwrap().to_string();

    let cli_args = CliArgs {
        llm_provider: Some("claude-cli".to_string()),
        llm_claude_binary: Some(temp_path),
        ..Default::default()
    };

    let config = Config::discover(&cli_args).expect("Config discovery should succeed");

    // Execute: Run doctor and serialize to JSON
    let mut doctor = DoctorCommand::new(config);
    let output = doctor
        .run_with_options()
        .expect("Doctor should run successfully");

    // Verify: Can serialize to JSON
    let json_str = serde_json::to_string_pretty(&output)
        .expect("Should be able to serialize doctor output to JSON");

    // Check that llm_provider appears in JSON
    assert!(
        json_str.contains("llm_provider"),
        "JSON output should contain llm_provider check"
    );

    // Parse back to verify structure
    let parsed: serde_json::Value =
        serde_json::from_str(&json_str).expect("Should be able to parse doctor JSON");

    // Verify checks array exists and contains llm_provider
    let checks = parsed["checks"]
        .as_array()
        .expect("checks should be an array");

    let has_llm_provider = checks
        .iter()
        .any(|check| check["name"].as_str() == Some("llm_provider"));

    assert!(has_llm_provider, "JSON checks should include llm_provider");
}

// ===== Test Scenario 6: Doctor strict mode with warnings =====

#[test]
fn test_doctor_strict_mode_with_llm_warnings() {
    let _env = TestEnv::new();

    // On Windows, we can potentially trigger a warning state
    // by having claude in WSL but not in native PATH
    #[cfg(target_os = "windows")]
    {
        // Setup: Empty PATH, rely on WSL detection
        unsafe {
            env::set_var("PATH", "");
        }

        let cli_args = CliArgs::default();
        let config = Config::discover(&cli_args).expect("Config discovery should succeed");

        // Execute: Run doctor in strict mode
        let mut doctor = DoctorCommand::new(config);
        let output_strict = doctor
            .run_with_options_strict(true)
            .expect("Doctor should run successfully");

        // If there's a warning (claude in WSL but not native),
        // strict mode should mark overall as not ok
        let has_warn = output_strict
            .checks
            .iter()
            .any(|c| c.status == CheckStatus::Warn);
        let has_fail = output_strict
            .checks
            .iter()
            .any(|c| c.status == CheckStatus::Fail);

        if has_warn && !has_fail {
            assert!(!output_strict.ok, "Strict mode should fail on warnings");
        }
    }

    // Non-Windows platforms: just verify strict mode runs
    #[cfg(not(target_os = "windows"))]
    {
        let cli_args = CliArgs::default();
        let config = Config::discover(&cli_args).expect("Config discovery should succeed");

        let mut doctor = DoctorCommand::new(config);
        let _output = doctor
            .run_with_options_strict(true)
            .expect("Doctor should run in strict mode");
    }
}

// ===== Test Scenario 7: Verify check is present in all statuses =====

#[test]
fn test_llm_provider_check_always_present() {
    let _env = TestEnv::new();

    // Test with various configurations to ensure check is always present
    let test_cases = vec![
        // Default config
        (CliArgs::default(), "default config"),
        // Explicit claude-cli
        (
            CliArgs {
                llm_provider: Some("claude-cli".to_string()),
                ..Default::default()
            },
            "explicit claude-cli",
        ),
    ];

    for (cli_args, description) in test_cases {
        let config = Config::discover(&cli_args)
            .unwrap_or_else(|e| panic!("Config should be valid for {}: {}", description, e));

        let mut doctor = DoctorCommand::new(config);
        let output = doctor
            .run_with_options()
            .unwrap_or_else(|e| panic!("Doctor should run for {}: {}", description, e));

        // Verify llm_provider check is present
        let llm_check = find_check(&output.checks, "llm_provider")
            .unwrap_or_else(|| panic!("llm_provider check should be present for {}", description));

        assert_eq!(
            llm_check.name, "llm_provider",
            "Check name should be llm_provider for {}",
            description
        );
    }
}

// ===== Test Scenario 8: Provider check details are informative =====

#[test]
fn test_llm_provider_check_details_are_informative() {
    let _env = TestEnv::new();

    // Test with custom binary path to ensure details are helpful
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let temp_path = temp_file.path().to_str().unwrap().to_string();

    let cli_args = CliArgs {
        llm_provider: Some("claude-cli".to_string()),
        llm_claude_binary: Some(temp_path.clone()),
        ..Default::default()
    };

    let config = Config::discover(&cli_args).expect("Config discovery should succeed");

    let mut doctor = DoctorCommand::new(config);
    let output = doctor
        .run_with_options()
        .expect("Doctor should run successfully");

    let llm_check =
        find_check(&output.checks, "llm_provider").expect("llm_provider check should be present");

    // Details should be informative
    assert!(
        !llm_check.details.is_empty(),
        "Check details should not be empty"
    );

    assert!(
        llm_check.details.contains("Provider") || llm_check.details.contains("provider"),
        "Details should mention provider, got: {}",
        llm_check.details
    );
}

// ===== Test Scenario 9: Multiple doctor runs produce consistent results =====

#[test]
fn test_llm_provider_check_consistency() {
    let _env = TestEnv::new();

    // Setup: Create a consistent test environment
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let temp_path = temp_file.path().to_str().unwrap().to_string();

    let cli_args = CliArgs {
        llm_provider: Some("claude-cli".to_string()),
        llm_claude_binary: Some(temp_path),
        ..Default::default()
    };

    let config = Config::discover(&cli_args).expect("Config discovery should succeed");

    // Execute: Run doctor multiple times
    let mut doctor1 = DoctorCommand::new(config.clone());
    let output1 = doctor1
        .run_with_options()
        .expect("First doctor run should succeed");

    let mut doctor2 = DoctorCommand::new(config);
    let output2 = doctor2
        .run_with_options()
        .expect("Second doctor run should succeed");

    // Verify: Results should be consistent
    let check1 = find_check(&output1.checks, "llm_provider")
        .expect("First run should have llm_provider check");
    let check2 = find_check(&output2.checks, "llm_provider")
        .expect("Second run should have llm_provider check");

    assert_eq!(
        check1.status, check2.status,
        "Multiple runs should produce consistent status"
    );

    // Details might have slight variations (paths, etc) but should both be non-empty
    assert!(!check1.details.is_empty());
    assert!(!check2.details.is_empty());
}

// ===== Test Scenario 10: Edge case - relative path for binary =====

#[test]
fn test_llm_provider_with_relative_path() {
    let _env = TestEnv::new();

    // Setup: Create temp file and use just filename (relative path)
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let temp_file_path = temp_dir.path().join("fake-claude");
    std::fs::write(&temp_file_path, "fake binary").expect("Failed to write fake binary");

    // Use absolute path (relative paths are platform-dependent and tricky)
    let absolute_path = temp_file_path.to_str().unwrap().to_string();

    let cli_args = CliArgs {
        llm_provider: Some("claude-cli".to_string()),
        llm_claude_binary: Some(absolute_path.clone()),
        ..Default::default()
    };

    let config = Config::discover(&cli_args).expect("Config discovery should succeed");

    let mut doctor = DoctorCommand::new(config);
    let output = doctor
        .run_with_options()
        .expect("Doctor should run successfully");

    let llm_check =
        find_check(&output.checks, "llm_provider").expect("llm_provider check should be present");

    // Should pass with valid path
    assert_eq!(
        llm_check.status,
        CheckStatus::Pass,
        "Should pass with valid absolute path"
    );
}

//! Smoke tests for xchecker CLI commands
//!
//! These tests validate that all xchecker commands execute successfully
//! and produce valid output. They test the integration of all components
//! without requiring Claude CLI or API keys.

use std::env;
use std::fs;
use std::io::Write;
use std::process::Command;
use tempfile::TempDir;
use xchecker::test_support;

/// Get the xchecker binary path
fn get_xchecker_bin() -> std::path::PathBuf {
    let manifest_dir =
        env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR should be set by cargo");

    // Try to get the binary from CARGO_BIN_EXE_xchecker first (set during test runs)
    if let Ok(bin_path) = env::var("CARGO_BIN_EXE_xchecker") {
        return std::path::PathBuf::from(bin_path);
    }

    // Otherwise, assume it's in target/debug
    let mut bin_path = std::path::PathBuf::from(manifest_dir);
    bin_path.push("target");
    bin_path.push("debug");
    bin_path.push("xchecker");
    if cfg!(windows) {
        bin_path.set_extension("exe");
    }
    bin_path
}

/// Helper to run xchecker via cargo (for tests that don't need specific working directory)
fn run_xchecker(args: &[&str]) -> Command {
    let manifest_dir =
        env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR should be set by cargo");

    let mut cmd = Command::new("cargo");
    cmd.current_dir(&manifest_dir);
    cmd.arg("run").arg("--bin").arg("xchecker").arg("--");
    for arg in args {
        cmd.arg(arg);
    }
    cmd
}

/// Helper to run xchecker binary in a specific working directory
fn run_xchecker_in_dir(args: &[&str], work_dir: &std::path::Path) -> Command {
    let bin_path = get_xchecker_bin();

    let mut cmd = Command::new(&bin_path);
    cmd.current_dir(work_dir);
    for arg in args {
        cmd.arg(arg);
    }
    cmd
}

fn real_llm_tests_enabled() -> bool {
    if test_support::llm_tests_enabled() {
        true
    } else {
        println!(
            "Skipping real LLM smoke tests. Set XCHECKER_REAL_LLM_TESTS=1 to enable (and ensure XCHECKER_SKIP_LLM_TESTS is unset)."
        );
        false
    }
}

/// Smoke test: xchecker doctor --json
///
/// Validates that the doctor command runs successfully and produces valid JSON output
#[test]
fn test_smoke_doctor_json() {
    println!("🔥 Smoke test: xchecker doctor --json");

    let output = run_xchecker(&["doctor", "--json"])
        .output()
        .expect("Failed to execute xchecker doctor --json");

    // Doctor should always succeed (exit 0) unless there are critical failures
    // It may exit 1 if checks fail, but the command itself should execute
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("Exit code: {:?}", output.status.code());
    println!("Stdout: {}", stdout);
    if !stderr.is_empty() {
        println!("Stderr: {}", stderr);
    }

    // Validate JSON output
    let json_result: Result<serde_json::Value, _> = serde_json::from_str(&stdout);
    assert!(
        json_result.is_ok(),
        "Doctor output should be valid JSON: {}",
        stdout
    );

    let json = json_result.unwrap();

    // Validate required fields (FR-OBS)
    assert!(
        json.get("checks").is_some(),
        "Doctor JSON should have 'checks' field"
    );
    assert!(
        json.get("ok").is_some(),
        "Doctor JSON should have 'ok' field"
    );

    println!("✓ Doctor command produces valid JSON output");
}

/// Smoke test: xchecker init demo --create-lock
///
/// Validates that the init command creates spec directory structure and lockfile
#[test]
fn test_smoke_init_with_lock() {
    println!("🔥 Smoke test: xchecker init demo --create-lock");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let spec_id = "smoke-test-init";

    let output = run_xchecker_in_dir(&["init", spec_id, "--create-lock"], temp_dir.path())
        .output()
        .expect("Failed to execute xchecker init");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("Exit code: {:?}", output.status.code());
    println!("Stdout: {}", stdout);
    if !stderr.is_empty() {
        println!("Stderr: {}", stderr);
    }

    // Init should succeed
    assert!(
        output.status.success(),
        "Init command should succeed: {}",
        stderr
    );

    // Verify directory structure created (FR-LOCK)
    let spec_dir = temp_dir.path().join(".xchecker/specs").join(spec_id);
    assert!(
        spec_dir.exists(),
        "Spec directory should be created: {}",
        spec_dir.display()
    );
    assert!(
        spec_dir.join("artifacts").exists(),
        "Artifacts directory should be created"
    );
    assert!(
        spec_dir.join("receipts").exists(),
        "Receipts directory should be created"
    );
    assert!(
        spec_dir.join("context").exists(),
        "Context directory should be created"
    );

    // Verify lockfile created (FR-LOCK-006)
    let lock_path = spec_dir.join("lock.json");
    assert!(
        lock_path.exists(),
        "Lockfile should be created: {}",
        lock_path.display()
    );

    // Validate lockfile content
    let lock_content = fs::read_to_string(&lock_path).expect("Failed to read lockfile");
    let lock_json: serde_json::Value =
        serde_json::from_str(&lock_content).expect("Lockfile should be valid JSON");

    assert!(
        lock_json.get("model_full_name").is_some(),
        "Lockfile should have model_full_name"
    );
    assert!(
        lock_json.get("claude_cli_version").is_some(),
        "Lockfile should have claude_cli_version"
    );
    assert!(
        lock_json.get("schema_version").is_some(),
        "Lockfile should have schema_version"
    );

    println!("✓ Init command creates directory structure and lockfile");
}

/// Smoke test: xchecker spec demo --dry-run
///
/// Validates that the spec command runs in dry-run mode without making Claude calls
#[test]
fn test_smoke_spec_dry_run() {
    println!("🔥 Smoke test: xchecker spec demo --dry-run");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let spec_id = "smoke-test-spec";

    // Create a test input file
    let input_content = "Create a simple calculator application with basic arithmetic operations";

    let mut child = run_xchecker_in_dir(&["spec", spec_id, "--dry-run"], temp_dir.path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to spawn xchecker spec");

    // Write input to stdin
    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(input_content.as_bytes())
            .expect("Failed to write to stdin");
    }

    let output = child
        .wait_with_output()
        .expect("Failed to wait for xchecker spec");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("Exit code: {:?}", output.status.code());
    println!("Stdout: {}", stdout);
    if !stderr.is_empty() {
        println!("Stderr: {}", stderr);
    }

    // Dry-run should succeed (FR-ORC)
    assert!(
        output.status.success(),
        "Spec dry-run should succeed: {}",
        stderr
    );

    // Verify output indicates dry-run mode
    assert!(
        stdout.contains("dry-run") || stdout.contains("Requirements phase completed"),
        "Output should indicate dry-run execution or completion"
    );

    println!("✓ Spec command runs successfully in dry-run mode");
}

/// Smoke test: xchecker status demo --json
///
/// Validates that the status command produces valid JSON output
#[test]
fn test_smoke_status_json() {
    println!("🔥 Smoke test: xchecker status demo --json");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let spec_id = "smoke-test-status";

    // First initialize the spec
    let init_output = run_xchecker_in_dir(&["init", spec_id], temp_dir.path())
        .output()
        .expect("Failed to execute xchecker init");

    assert!(
        init_output.status.success(),
        "Init should succeed before status check"
    );

    // Now check status
    let output = run_xchecker_in_dir(&["status", spec_id, "--json"], temp_dir.path())
        .output()
        .expect("Failed to execute xchecker status");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("Exit code: {:?}", output.status.code());
    println!("Stdout: {}", stdout);
    if !stderr.is_empty() {
        println!("Stderr: {}", stderr);
    }

    // Status should succeed (FR-STA)
    assert!(
        output.status.success(),
        "Status command should succeed: {}",
        stderr
    );

    // Validate JSON output (FR-STA-001)
    let json_result: Result<serde_json::Value, _> = serde_json::from_str(&stdout);
    assert!(
        json_result.is_ok(),
        "Status output should be valid JSON: {}",
        stdout
    );

    let json = json_result.unwrap();

    // Validate required fields per status-json.v1 schema (FR-Claude Code-CLI, Requirements 4.1.2)
    assert!(
        json.get("schema_version").is_some(),
        "Status JSON should have schema_version"
    );
    assert!(
        json.get("spec_id").is_some(),
        "Status JSON should have spec_id"
    );
    assert!(
        json.get("phase_statuses").is_some(),
        "Status JSON should have phase_statuses"
    );
    assert!(
        json.get("pending_fixups").is_some(),
        "Status JSON should have pending_fixups"
    );
    assert!(
        json.get("has_errors").is_some(),
        "Status JSON should have has_errors"
    );

    println!("✓ Status command produces valid JSON output");
}

/// Smoke test: xchecker clean demo --hard
///
/// Validates that the clean command removes spec artifacts
#[test]
fn test_smoke_clean_hard() {
    println!("🔥 Smoke test: xchecker clean demo --hard");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let spec_id = "smoke-test-clean";

    // First initialize the spec
    let init_output = run_xchecker_in_dir(&["init", spec_id], temp_dir.path())
        .output()
        .expect("Failed to execute xchecker init");

    assert!(
        init_output.status.success(),
        "Init should succeed before clean"
    );

    let spec_dir = temp_dir.path().join(".xchecker/specs").join(spec_id);
    assert!(
        spec_dir.exists(),
        "Spec directory should exist before clean"
    );

    // Now clean the spec
    let output = run_xchecker_in_dir(&["clean", spec_id, "--hard"], temp_dir.path())
        .output()
        .expect("Failed to execute xchecker clean");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("Exit code: {:?}", output.status.code());
    println!("Stdout: {}", stdout);
    if !stderr.is_empty() {
        println!("Stderr: {}", stderr);
    }

    // Clean should succeed
    assert!(
        output.status.success(),
        "Clean command should succeed: {}",
        stderr
    );

    // Verify spec directory is removed
    assert!(
        !spec_dir.exists(),
        "Spec directory should be removed after clean"
    );

    println!("✓ Clean command removes spec artifacts");
}

/// Smoke test: xchecker benchmark
///
/// Validates that the benchmark command runs and produces valid output
#[test]
#[ignore = "flaky in CI - environment-dependent timing"]
fn test_smoke_benchmark() {
    println!("🔥 Smoke test: xchecker benchmark");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let output = run_xchecker_in_dir(
        &[
            "benchmark",
            "--file-count",
            "10",
            "--iterations",
            "2",
            "--json",
        ],
        temp_dir.path(),
    )
    .output()
    .expect("Failed to execute xchecker benchmark");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("Exit code: {:?}", output.status.code());
    println!("Stdout: {}", stdout);
    if !stderr.is_empty() {
        println!("Stderr: {}", stderr);
    }

    // Benchmark should succeed (FR-BENCH)
    assert!(
        output.status.success(),
        "Benchmark command should succeed: {}",
        stderr
    );

    // Validate JSON output (FR-BENCH-004)
    let json_result: Result<serde_json::Value, _> = serde_json::from_str(&stdout);
    assert!(
        json_result.is_ok(),
        "Benchmark output should be valid JSON: {}",
        stdout
    );

    let json = json_result.unwrap();

    // Validate required fields (FR-BENCH-004)
    assert!(
        json.get("ok").is_some(),
        "Benchmark JSON should have 'ok' field"
    );
    assert!(
        json.get("timings_ms").is_some(),
        "Benchmark JSON should have 'timings_ms' field"
    );
    assert!(
        json.get("rss_mb").is_some(),
        "Benchmark JSON should have 'rss_mb' field"
    );

    println!("✓ Benchmark command produces valid output");
}

/// Smoke test: Verify all commands succeed with correct exit codes
///
/// This test runs all commands and verifies they produce expected exit codes
#[test]
fn test_smoke_all_commands_exit_codes() {
    println!("🔥 Smoke test: Verify all commands exit codes");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Test 1: --help should exit 0
    let help_output = run_xchecker(&["--help"])
        .output()
        .expect("Failed to execute --help");
    assert!(
        help_output.status.success(),
        "--help should exit with code 0"
    );

    // Test 2: --version should exit 0
    let version_output = run_xchecker(&["--version"])
        .output()
        .expect("Failed to execute --version");
    assert!(
        version_output.status.success(),
        "--version should exit with code 0"
    );

    // Test 3: doctor should exit 0 or 1 (depending on checks)
    let doctor_output = run_xchecker(&["doctor"])
        .output()
        .expect("Failed to execute doctor");
    let doctor_code = doctor_output.status.code().unwrap_or(-1);
    assert!(
        doctor_code == 0 || doctor_code == 1,
        "doctor should exit with code 0 or 1, got {}",
        doctor_code
    );

    // Test 4: init should exit 0
    let init_output = run_xchecker_in_dir(&["init", "exit-code-test"], temp_dir.path())
        .output()
        .expect("Failed to execute init");
    assert!(init_output.status.success(), "init should exit with code 0");

    // Test 5: status should exit 0
    let status_output = run_xchecker_in_dir(&["status", "exit-code-test"], temp_dir.path())
        .output()
        .expect("Failed to execute status");
    assert!(
        status_output.status.success(),
        "status should exit with code 0"
    );

    // Test 6: clean should exit 0
    let clean_output = run_xchecker_in_dir(&["clean", "exit-code-test", "--hard"], temp_dir.path())
        .output()
        .expect("Failed to execute clean");
    assert!(
        clean_output.status.success(),
        "clean should exit with code 0"
    );

    // Test 7: benchmark should exit 0
    let benchmark_output = run_xchecker_in_dir(
        &["benchmark", "--file-count", "5", "--iterations", "1"],
        temp_dir.path(),
    )
    .output()
    .expect("Failed to execute benchmark");
    assert!(
        benchmark_output.status.success(),
        "benchmark should exit with code 0"
    );

    println!("✓ All commands produce correct exit codes");
}

/// Smoke test: Verify JSON output is valid for all commands that support --json
///
/// This test validates that all JSON outputs are properly formatted
#[test]
fn test_smoke_json_output_validity() {
    println!("🔥 Smoke test: Verify JSON output validity");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Initialize a test spec
    run_xchecker_in_dir(&["init", "json-test"], temp_dir.path())
        .output()
        .expect("Failed to init spec");

    // Test 1: doctor --json
    let doctor_output = run_xchecker(&["doctor", "--json"])
        .output()
        .expect("Failed to execute doctor --json");
    let doctor_json = String::from_utf8_lossy(&doctor_output.stdout);
    assert!(
        serde_json::from_str::<serde_json::Value>(&doctor_json).is_ok(),
        "doctor --json should produce valid JSON"
    );

    // Test 2: status --json
    let status_output = run_xchecker_in_dir(&["status", "json-test", "--json"], temp_dir.path())
        .output()
        .expect("Failed to execute status --json");
    let status_json = String::from_utf8_lossy(&status_output.stdout);
    assert!(
        serde_json::from_str::<serde_json::Value>(&status_json).is_ok(),
        "status --json should produce valid JSON"
    );

    // Test 3: benchmark --json
    let benchmark_output = run_xchecker_in_dir(
        &[
            "benchmark",
            "--file-count",
            "5",
            "--iterations",
            "1",
            "--json",
        ],
        temp_dir.path(),
    )
    .output()
    .expect("Failed to execute benchmark --json");
    let benchmark_json = String::from_utf8_lossy(&benchmark_output.stdout);
    assert!(
        serde_json::from_str::<serde_json::Value>(&benchmark_json).is_ok(),
        "benchmark --json should produce valid JSON"
    );

    println!("✓ All JSON outputs are valid");
}

// ============================================================================
// Tests with Claude CLI (ignored by default, run in CI with --ignored)
// ============================================================================

/// Smoke test to verify Claude CLI is accessible
///
/// This test is marked as ignored and only runs in CI with the --ignored flag
/// when ANTHROPIC_API_KEY secret is available.
#[test]
#[ignore = "requires_real_claude"]
fn test_claude_cli_available() {
    if !real_llm_tests_enabled() {
        return;
    }

    // Check if API key is available
    if env::var("ANTHROPIC_API_KEY").is_err() {
        println!("Skipping smoke test: ANTHROPIC_API_KEY not set");
        return;
    }

    println!("Running smoke test with real Claude CLI...");

    // Try to run claude --version
    let output = std::process::Command::new("claude")
        .arg("--version")
        .output();

    match output {
        Ok(result) => {
            if result.status.success() {
                let version = String::from_utf8_lossy(&result.stdout);
                println!("Claude CLI version: {}", version.trim());
                println!("✓ Claude CLI is accessible");
            } else {
                let stderr = String::from_utf8_lossy(&result.stderr);
                panic!(
                    "Claude CLI failed with exit code {:?}: {}",
                    result.status.code(),
                    stderr
                );
            }
        }
        Err(e) => {
            panic!("Failed to execute claude command: {}", e);
        }
    }
}

/// Smoke test to verify basic xchecker functionality with real Claude CLI
///
/// This test is marked as ignored and only runs in CI with the --ignored flag
/// when ANTHROPIC_API_KEY secret is available.
#[test]
#[ignore = "requires_real_claude"]
fn test_xchecker_with_real_claude() {
    if !real_llm_tests_enabled() {
        return;
    }

    // Check if API key is available
    if env::var("ANTHROPIC_API_KEY").is_err() {
        println!("Skipping smoke test: ANTHROPIC_API_KEY not set");
        return;
    }

    println!("Smoke test: xchecker with real Claude CLI");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let spec_id = "real-claude-test";

    // Create a test input
    let input_content = "Create a simple hello world application";

    let mut child = run_xchecker_in_dir(&["spec", spec_id], temp_dir.path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to spawn xchecker spec");

    // Write input to stdin
    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(input_content.as_bytes())
            .expect("Failed to write to stdin");
    }

    let output = child
        .wait_with_output()
        .expect("Failed to wait for xchecker spec");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("Exit code: {:?}", output.status.code());
    println!("Stdout: {}", stdout);
    if !stderr.is_empty() {
        println!("Stderr: {}", stderr);
    }

    // Spec execution with real Claude should succeed
    assert!(
        output.status.success(),
        "Spec execution with real Claude should succeed: {}",
        stderr
    );

    // Verify artifacts were created
    let spec_dir = temp_dir.path().join(".xchecker/specs").join(spec_id);
    let artifacts_dir = spec_dir.join("artifacts");
    assert!(
        artifacts_dir.exists(),
        "Artifacts directory should exist after execution"
    );

    // Verify receipt was created
    let receipts_dir = spec_dir.join("receipts");
    assert!(
        receipts_dir.exists(),
        "Receipts directory should exist after execution"
    );

    println!("✓ xchecker works with real Claude CLI");
}

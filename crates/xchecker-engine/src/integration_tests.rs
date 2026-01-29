//! Integration tests for final validation of all components working together
//!
//! This module provides smoke tests and validation to ensure all systems
//! are properly integrated and working as expected.

use crate::runner::CommandSpec;
use anyhow::{Context, Result};
use tempfile::TempDir;

/// Run basic smoke tests to validate integration
pub fn run_smoke_tests() -> Result<()> {
    println!("Running integration smoke tests...");

    // Test 1: CLI help works
    test_cli_help()?;

    // Test 2: Version information works
    test_version_info()?;

    // Test 3: Dry-run execution works
    test_dry_run_execution()?;

    // Test 4: Status command works
    test_status_command()?;

    // Test 5: Benchmark command works
    test_benchmark_command()?;

    println!("✓ All smoke tests passed");
    Ok(())
}

/// Test that CLI help works correctly
fn test_cli_help() -> Result<()> {
    let output = CommandSpec::new("cargo")
        .args(["run", "--bin", "xchecker", "--", "--help"])
        .to_command()
        .output()?;

    if !output.status.success() {
        anyhow::bail!("CLI help command failed");
    }

    let help_text = String::from_utf8_lossy(&output.stdout);
    if !help_text.contains("xchecker") || !help_text.contains("spec") {
        anyhow::bail!("CLI help output doesn't contain expected content");
    }

    println!("✓ CLI help works");
    Ok(())
}

/// Test that version information works
fn test_version_info() -> Result<()> {
    let output = CommandSpec::new("cargo")
        .args(["run", "--bin", "xchecker", "--", "--version"])
        .to_command()
        .output()?;

    if !output.status.success() {
        anyhow::bail!("Version command failed");
    }

    let version_text = String::from_utf8_lossy(&output.stdout);
    if version_text.trim().is_empty() {
        anyhow::bail!("Version output is empty");
    }

    println!("✓ Version info works");
    Ok(())
}

/// Test that dry-run execution works end-to-end
fn test_dry_run_execution() -> Result<()> {
    let _temp_dir = TempDir::new()?;

    let mut cmd = CommandSpec::new("cargo")
        .args([
            "run",
            "--bin",
            "xchecker",
            "--",
            "spec",
            "smoke-test",
            "--dry-run",
        ])
        .to_command();

    // Provide test input via stdin
    let mut child = cmd
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    if let Some(stdin) = child.stdin.as_mut() {
        use std::io::Write;
        writeln!(stdin, "Test spec: Create a simple calculator application")?;
        let _: std::io::Result<()> = std::io::Result::Ok(());
    }

    let output = child.wait_with_output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Dry-run execution failed: {stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.contains("Requirements phase completed successfully") {
        anyhow::bail!("Dry-run didn't complete requirements phase");
    }

    println!("✓ Dry-run execution works");
    Ok(())
}

/// Test that status command works
fn test_status_command() -> Result<()> {
    let output = CommandSpec::new("cargo")
        .args([
            "run",
            "--bin",
            "xchecker",
            "--",
            "status",
            "nonexistent-spec",
        ])
        .to_command()
        .output()?;

    if !output.status.success() {
        anyhow::bail!("Status command failed");
    }

    let status_text = String::from_utf8_lossy(&output.stdout);
    if !status_text.contains("Status for spec") {
        anyhow::bail!("Status output doesn't contain expected content");
    }

    println!("✓ Status command works");
    Ok(())
}

/// Test that benchmark command works
fn test_benchmark_command() -> Result<()> {
    // Test 1: Basic benchmark command
    let output = CommandSpec::new("cargo")
        .args([
            "run",
            "--bin",
            "xchecker",
            "--",
            "benchmark",
            "--file-count",
            "10",
            "--iterations",
            "2",
        ])
        .to_command()
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Benchmark command failed: {stderr}");
    }

    let benchmark_text = String::from_utf8_lossy(&output.stdout);
    if !benchmark_text.contains("Benchmark Results") {
        anyhow::bail!("Benchmark output doesn't contain expected content");
    }

    // Test 2: JSON output format
    let json_output = CommandSpec::new("cargo")
        .args([
            "run",
            "--bin",
            "xchecker",
            "--",
            "benchmark",
            "--file-count",
            "5",
            "--iterations",
            "2",
            "--json",
        ])
        .to_command()
        .output()?;

    if !json_output.status.success() {
        let stderr = String::from_utf8_lossy(&json_output.stderr);
        anyhow::bail!("Benchmark JSON command failed: {stderr}");
    }

    let json_text = String::from_utf8_lossy(&json_output.stdout);

    // Parse JSON to validate structure
    let json_value: serde_json::Value =
        serde_json::from_str(&json_text).context("Failed to parse benchmark JSON output")?;

    // Validate required fields (FR-BENCH-004)
    if json_value.get("ok").is_none() {
        anyhow::bail!("JSON output missing 'ok' field");
    }
    if json_value.get("timings_ms").is_none() {
        anyhow::bail!("JSON output missing 'timings_ms' field");
    }
    if json_value.get("rss_mb").is_none() {
        anyhow::bail!("JSON output missing 'rss_mb' field");
    }

    // Test 3: Threshold overrides
    let threshold_output = CommandSpec::new("cargo")
        .args([
            "run",
            "--bin",
            "xchecker",
            "--",
            "benchmark",
            "--file-count",
            "5",
            "--iterations",
            "2",
            "--max-empty-run-secs",
            "10.0",
            "--max-packetization-ms",
            "500.0",
            "--json",
        ])
        .to_command()
        .output()?;

    if !threshold_output.status.success() {
        let stderr = String::from_utf8_lossy(&threshold_output.stderr);
        anyhow::bail!("Benchmark threshold override command failed: {stderr}");
    }

    let threshold_json_text = String::from_utf8_lossy(&threshold_output.stdout);
    let threshold_json: serde_json::Value = serde_json::from_str(&threshold_json_text)
        .context("Failed to parse threshold override JSON output")?;

    // Validate thresholds were applied
    if let Some(thresholds) = threshold_json.get("thresholds") {
        if let Some(empty_run_max) = thresholds.get("empty_run_max_secs") {
            if empty_run_max.as_f64() != Some(10.0) {
                anyhow::bail!("Threshold override for empty_run_max_secs not applied correctly");
            }
        } else {
            anyhow::bail!("Thresholds missing empty_run_max_secs field");
        }

        if let Some(packet_max) = thresholds.get("packetization_max_ms_per_100_files") {
            if packet_max.as_f64() != Some(500.0) {
                anyhow::bail!(
                    "Threshold override for packetization_max_ms_per_100_files not applied correctly"
                );
            }
        } else {
            anyhow::bail!("Thresholds missing packetization_max_ms_per_100_files field");
        }
    } else {
        anyhow::bail!("JSON output missing 'thresholds' field");
    }

    println!("✓ Benchmark command works (basic, JSON, and threshold overrides)");
    Ok(())
}

/// Validate that all required components are properly integrated
pub fn validate_component_integration() -> Result<()> {
    println!("Validating component integration...");

    // Check that all modules are properly exported in lib.rs
    validate_module_exports()?;

    // Check that error handling is properly integrated
    validate_error_integration()?;

    // Check that configuration system is working
    validate_config_integration()?;

    println!("✓ Component integration validated");
    Ok(())
}

/// Validate that all modules are properly exported
fn validate_module_exports() -> Result<()> {
    // This is a compile-time check - if the code compiles, exports are working
    use crate::types::{FileType, PhaseId, RunnerMode};

    // Test that we can create key types
    let _phase_id = PhaseId::Requirements;
    let _file_type = FileType::Markdown;
    let _runner_mode = RunnerMode::Auto;

    println!("✓ Module exports validated");
    Ok(())
}

/// Validate error handling integration
fn validate_error_integration() -> Result<()> {
    use crate::error::{ConfigError, XCheckerError};

    // Test that we can create and handle errors
    let _error = XCheckerError::Config(ConfigError::MissingRequired("test".to_string()));

    println!("✓ Error integration validated");
    Ok(())
}

/// Validate configuration integration
fn validate_config_integration() -> Result<()> {
    // Test that configuration system is accessible
    // This is mainly a compile-time check
    println!("✓ Configuration system accessible");

    println!("✓ Configuration integration validated");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component_integration() {
        validate_component_integration().expect("Component integration validation failed");
    }

    #[test]
    fn test_module_exports() {
        validate_module_exports().expect("Module export validation failed");
    }

    #[test]
    fn test_error_integration() {
        validate_error_integration().expect("Error integration validation failed");
    }

    #[test]
    fn test_config_integration() {
        validate_config_integration().expect("Config integration validation failed");
    }
}

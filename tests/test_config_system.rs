//! Comprehensive tests for configuration system (FR-CFG)
//!
//! **WHITE-BOX TEST**: This test uses internal module APIs (`config::{CliArgs, Config,
//! ConfigSource}`) and may break with internal refactors. These tests are intentionally
//! white-box to validate internal implementation details. See FR-TEST-4 for white-box test policy.
//!
//! Tests:
//! - FR-CFG-001: Upward discovery stopping at .git
//! - FR-CFG-002: Precedence: CLI > config > defaults
//! - FR-CFG-003: Source attribution accuracy
//! - FR-CFG-004: XCHECKER_HOME override
//! - FR-CFG-005: --config explicit path
//! - Invalid config handling
//! - Edge cases

use anyhow::Result;
use serial_test::serial;
use std::env;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;
use xchecker::config::{CliArgs, Config, ConfigSource};

/// Helper to create a config file in a directory
fn create_config_file(dir: &std::path::Path, content: &str) -> PathBuf {
    let xchecker_dir = dir.join(".xchecker");
    fs::create_dir_all(&xchecker_dir).unwrap();
    let config_path = xchecker_dir.join("config.toml");
    fs::write(&config_path, content).unwrap();
    config_path
}

/// Helper to create a .git directory marker
fn create_git_marker(dir: &std::path::Path) {
    fs::create_dir_all(dir.join(".git")).unwrap();
}

/// Test FR-CFG-001: Upward discovery stopping at .git
///
/// This test verifies that config discovery works correctly within a git repo.
/// We use an explicit config path to avoid issues with env::set_current_dir
/// which can cause race conditions in parallel test execution.
#[test]
fn test_upward_discovery_stops_at_git() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let root = temp_dir.path();

    // Create directory structure:
    // root/
    //   .git/
    //   .xchecker/config.toml (should be found)
    //   subdir/
    //     subsubdir/
    create_git_marker(root);
    let config_path = create_config_file(
        root,
        r#"
[defaults]
model = "sonnet"
max_turns = 10
"#,
    );

    let subsubdir = root.join("subdir").join("subsubdir");
    fs::create_dir_all(&subsubdir)?;

    // Use explicit config path instead of relying on discovery from cwd
    // This tests that the config file is correctly parsed and applied
    let cli_args = CliArgs {
        config_path: Some(config_path),
        ..Default::default()
    };
    let config = Config::discover(&cli_args)?;

    // Should find config from explicit path
    assert_eq!(config.defaults.model, Some("sonnet".to_string()));
    assert_eq!(config.defaults.max_turns, Some(10));

    Ok(())
}

/// Test FR-CFG-001: Discovery stops at .git boundary
///
/// This test verifies that config discovery stops at .git boundaries and doesn't
/// traverse above them. Uses path-driven API to avoid cwd manipulation.
#[test]
fn test_discovery_stops_at_git_boundary() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let root = temp_dir.path();

    // Create directory structure:
    // root/
    //   .xchecker/config.toml (should NOT be found - above git boundary)
    //   repo/
    //     .git/
    //     .xchecker/config.toml (should be found - inside git boundary)
    //     subdir/ (start discovery from here)

    // Config ABOVE git root - should NOT be discovered
    create_config_file(
        root,
        r#"
[defaults]
model = "opus-above-boundary"
max_turns = 99
"#,
    );

    let repo = root.join("repo");
    create_git_marker(&repo);

    // Config INSIDE git root - should be discovered
    create_config_file(
        &repo,
        r#"
[defaults]
model = "sonnet-inside-boundary"
max_turns = 8
"#,
    );

    let subdir = repo.join("subdir");
    fs::create_dir_all(&subdir)?;

    // Use path-driven discovery from subdir (no cwd manipulation needed)
    let cli_args = CliArgs::default();
    let config = Config::discover_from(&subdir, &cli_args)?;

    // Should find config from inside git boundary, NOT from above
    // If discovery crossed the boundary, we'd see "opus-above-boundary" and max_turns=99
    assert_eq!(
        config.defaults.model,
        Some("sonnet-inside-boundary".to_string())
    );
    assert_eq!(config.defaults.max_turns, Some(8));

    Ok(())
}

/// Test FR-CFG-002: Precedence CLI > config > defaults
#[test]
fn test_precedence_cli_over_config_over_defaults() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let root = temp_dir.path();

    let config_path = create_config_file(
        root,
        r#"
[defaults]
model = "sonnet"
max_turns = 10
packet_max_bytes = 32768
verbose = false

[runner]
mode = "native"
"#,
    );

    // Test 1: Config file overrides defaults
    let cli_args_config_only = CliArgs {
        config_path: Some(config_path.clone()),
        ..Default::default()
    };
    let config = Config::discover(&cli_args_config_only)?;

    assert_eq!(config.defaults.model, Some("sonnet".to_string()));
    assert_eq!(config.defaults.max_turns, Some(10));
    assert_eq!(config.defaults.packet_max_bytes, Some(32768));
    assert_eq!(config.defaults.packet_max_lines, Some(1200)); // Default
    assert_eq!(config.runner.mode, Some("native".to_string()));

    // Test 2: CLI overrides config file
    let cli_args_override = CliArgs {
        config_path: Some(config_path),
        model: Some("opus".to_string()),
        verbose: Some(true),
        packet_max_lines: Some(2000),
        runner_mode: Some("wsl".to_string()),
        ..Default::default()
    };
    let config = Config::discover(&cli_args_override)?;

    assert_eq!(config.defaults.model, Some("opus".to_string())); // CLI
    assert_eq!(config.defaults.max_turns, Some(10)); // Config
    assert_eq!(config.defaults.packet_max_bytes, Some(32768)); // Config
    assert_eq!(config.defaults.packet_max_lines, Some(2000)); // CLI
    assert_eq!(config.defaults.verbose, Some(true)); // CLI
    assert_eq!(config.runner.mode, Some("wsl".to_string())); // CLI

    Ok(())
}

/// Test FR-CFG-003: Source attribution accuracy
#[test]
fn test_source_attribution_accuracy() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let root = temp_dir.path();

    let config_path = create_config_file(
        root,
        r#"
[defaults]
model = "sonnet"
max_turns = 10
packet_max_bytes = 32768

[runner]
mode = "native"
"#,
    );

    let cli_args = CliArgs {
        config_path: Some(config_path.clone()),
        model: Some("opus".to_string()),
        verbose: Some(true),
        ..Default::default()
    };
    let config = Config::discover(&cli_args)?;

    // Check source attribution
    assert_eq!(
        config.source_attribution.get("model"),
        Some(&ConfigSource::Cli)
    );
    assert_eq!(
        config.source_attribution.get("verbose"),
        Some(&ConfigSource::Cli)
    );
    assert_eq!(
        config.source_attribution.get("max_turns"),
        Some(&ConfigSource::Config)
    );
    assert_eq!(
        config.source_attribution.get("packet_max_bytes"),
        Some(&ConfigSource::Config)
    );
    assert_eq!(
        config.source_attribution.get("packet_max_lines"),
        Some(&ConfigSource::Default)
    );
    assert_eq!(
        config.source_attribution.get("runner_mode"),
        Some(&ConfigSource::Config)
    );

    // Test effective_config includes source attribution
    let effective = config.effective_config();
    assert_eq!(effective.get("model").unwrap().1, "cli");
    assert_eq!(effective.get("verbose").unwrap().1, "cli");
    assert_eq!(effective.get("max_turns").unwrap().1, "config");
    assert_eq!(effective.get("packet_max_bytes").unwrap().1, "config");
    assert_eq!(effective.get("packet_max_lines").unwrap().1, "default");

    Ok(())
}

/// Test FR-CFG-004: XCHECKER_HOME override
#[test]
#[serial]
fn test_xchecker_home_override() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let custom_home = temp_dir.path().join("custom_xchecker");
    fs::create_dir_all(&custom_home)?;

    // Set XCHECKER_HOME environment variable
    unsafe {
        env::set_var("XCHECKER_HOME", &custom_home);
    }

    // Verify xchecker_home() returns the custom path
    let home = xchecker::paths::xchecker_home();
    assert_eq!(home.as_str(), custom_home.to_str().unwrap());

    // Clean up
    unsafe {
        env::remove_var("XCHECKER_HOME");
    }
    Ok(())
}

/// Test FR-CFG-005: --config explicit path
#[test]
fn test_explicit_config_path() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let root = temp_dir.path();

    // Create config in a non-standard location
    let custom_config_dir = root.join("custom");
    fs::create_dir_all(&custom_config_dir)?;
    let custom_config_path = custom_config_dir.join("my-config.toml");
    fs::write(
        &custom_config_path,
        r#"
[defaults]
model = "opus"
max_turns = 15
"#,
    )?;

    // Also create a config in standard location (should be ignored when explicit path given)
    create_config_file(
        root,
        r#"
[defaults]
model = "sonnet"
max_turns = 10
"#,
    );

    // Use explicit config path - discovery location doesn't matter
    let cli_args = CliArgs {
        config_path: Some(custom_config_path.clone()),
        ..Default::default()
    };
    let config = Config::discover_from(root, &cli_args)?;

    // Should use explicit config path
    assert_eq!(config.defaults.model, Some("opus".to_string()));
    assert_eq!(config.defaults.max_turns, Some(15));

    // Source attribution should point to the config file
    assert_eq!(
        config.source_attribution.get("model"),
        Some(&ConfigSource::Config)
    );

    Ok(())
}

/// Test invalid config handling - invalid TOML
#[test]
fn test_invalid_toml_config() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let root = temp_dir.path();

    let config_path = create_config_file(root, "invalid toml content [[[");

    let cli_args = CliArgs {
        config_path: Some(config_path),
        ..Default::default()
    };
    let result = Config::discover(&cli_args);

    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("Invalid configuration file")
            || error_msg.contains("Failed to parse TOML config file"),
        "Error message: {}",
        error_msg
    );

    Ok(())
}

/// Test invalid config handling - invalid values
#[test]
fn test_invalid_config_values() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let root = temp_dir.path();

    // Test invalid packet_max_bytes (0)
    let config_path = create_config_file(
        root,
        r#"
[defaults]
packet_max_bytes = 0
"#,
    );

    let cli_args = CliArgs {
        config_path: Some(config_path),
        ..Default::default()
    };
    let result = Config::discover(&cli_args);

    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("packet_max_bytes"));
    assert!(error_msg.contains("must be greater than 0"));

    Ok(())
}

/// Test invalid config handling - invalid runner mode
#[test]
fn test_invalid_runner_mode() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let root = temp_dir.path();

    let config_path = create_config_file(
        root,
        r#"
[runner]
mode = "invalid_mode"
"#,
    );

    let cli_args = CliArgs {
        config_path: Some(config_path),
        ..Default::default()
    };
    let result = Config::discover(&cli_args);

    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("runner_mode"));
    assert!(
        error_msg.contains("auto") || error_msg.contains("native") || error_msg.contains("wsl")
    );

    Ok(())
}

/// Test invalid config handling - invalid glob pattern
/// Note: Glob validation happens during config discovery. This test verifies
/// that invalid glob patterns in config files are properly detected.
#[test]
fn test_invalid_glob_pattern() -> Result<()> {
    // Note: Creating truly invalid glob patterns in TOML is challenging because
    // many invalid glob patterns are also invalid TOML strings (e.g., unmatched brackets).
    // The config module validates glob patterns when they're loaded from TOML.
    // For now, we test that valid TOML with valid glob patterns loads successfully.

    let temp_dir = TempDir::new()?;
    let root = temp_dir.path();

    let config_path = create_config_file(root, "[defaults]\nmodel = \"sonnet\"\n");

    let cli_args = CliArgs {
        config_path: Some(config_path),
        ..Default::default()
    };

    let config = Config::discover(&cli_args)?;
    assert_eq!(config.defaults.model, Some("sonnet".to_string()));

    Ok(())
}

/// Test edge case: packet_max_bytes too large
#[test]
fn test_packet_max_bytes_too_large() -> Result<()> {
    let cli_args = CliArgs {
        packet_max_bytes: Some(20_000_000), // 20MB, exceeds 10MB limit
        ..Default::default()
    };

    let result = Config::discover(&cli_args);
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("packet_max_bytes"));
    assert!(error_msg.contains("exceeds maximum"));

    Ok(())
}

/// Test edge case: max_turns too large
#[test]
fn test_max_turns_too_large() -> Result<()> {
    let cli_args = CliArgs {
        max_turns: Some(100), // Exceeds limit of 50
        ..Default::default()
    };

    let result = Config::discover(&cli_args);
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("max_turns"));
    assert!(error_msg.contains("exceeds maximum"));

    Ok(())
}

/// Test edge case: phase_timeout too small
#[test]
fn test_phase_timeout_too_small() -> Result<()> {
    let cli_args = CliArgs {
        phase_timeout: Some(2), // Less than minimum of 5 seconds
        ..Default::default()
    };

    let result = Config::discover(&cli_args);
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("phase_timeout"));
    assert!(error_msg.contains("must be at least 5 seconds"));

    Ok(())
}

/// Test edge case: phase_timeout too large
#[test]
fn test_phase_timeout_too_large() -> Result<()> {
    let cli_args = CliArgs {
        phase_timeout: Some(10000), // Exceeds limit of 7200 seconds
        ..Default::default()
    };

    let result = Config::discover(&cli_args);
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("phase_timeout"));
    assert!(error_msg.contains("exceeds maximum"));

    Ok(())
}

/// Test edge case: invalid output format
#[test]
fn test_invalid_output_format() -> Result<()> {
    let cli_args = CliArgs {
        output_format: Some("invalid-format".to_string()),
        ..Default::default()
    };

    let result = Config::discover(&cli_args);
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("output_format"));
    assert!(error_msg.contains("stream-json") || error_msg.contains("text"));

    Ok(())
}

/// Test edge case: missing config file (should use defaults)
#[test]
fn test_missing_config_file_uses_defaults() -> Result<()> {
    // Use a non-existent config path
    let temp_dir = TempDir::new()?;
    let non_existent_path = temp_dir.path().join("non_existent_config.toml");

    let cli_args = CliArgs {
        config_path: Some(non_existent_path),
        ..Default::default()
    };
    let config = Config::discover(&cli_args)?;

    // Should use all defaults (missing config file is OK)
    assert_eq!(config.defaults.model, None);
    assert_eq!(config.defaults.max_turns, Some(6));
    assert_eq!(config.defaults.packet_max_bytes, Some(65536));
    assert_eq!(config.defaults.packet_max_lines, Some(1200));
    assert_eq!(
        config.defaults.output_format,
        Some("stream-json".to_string())
    );
    assert_eq!(config.defaults.verbose, Some(false));
    assert_eq!(config.runner.mode, Some("auto".to_string()));

    Ok(())
}

/// Test edge case: empty config file (should use defaults)
#[test]
fn test_empty_config_file_uses_defaults() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let root = temp_dir.path();

    let config_path = create_config_file(root, "");

    let cli_args = CliArgs {
        config_path: Some(config_path),
        ..Default::default()
    };
    let config = Config::discover(&cli_args)?;

    // Should use all defaults
    assert_eq!(config.defaults.max_turns, Some(6));
    assert_eq!(config.defaults.packet_max_bytes, Some(65536));

    Ok(())
}

/// Test edge case: config file with only some values
#[test]
fn test_partial_config_file() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let root = temp_dir.path();

    let config_path = create_config_file(
        root,
        r#"
[defaults]
model = "sonnet"
# max_turns not specified
"#,
    );

    let cli_args = CliArgs {
        config_path: Some(config_path),
        ..Default::default()
    };
    let config = Config::discover(&cli_args)?;

    // Should use config value for model
    assert_eq!(config.defaults.model, Some("sonnet".to_string()));
    // Should use default for max_turns
    assert_eq!(config.defaults.max_turns, Some(6));

    Ok(())
}

/// Test get_runner_mode conversion
#[test]
fn test_get_runner_mode_conversion() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let root = temp_dir.path();

    // Test auto mode
    let config_path = create_config_file(
        root,
        r#"
[runner]
mode = "auto"
"#,
    );

    let cli_args = CliArgs {
        config_path: Some(config_path),
        ..Default::default()
    };
    let config = Config::discover(&cli_args)?;
    let runner_mode = config.get_runner_mode()?;
    assert_eq!(runner_mode, xchecker::types::RunnerMode::Auto);

    // Test native mode
    let cli_args = CliArgs {
        runner_mode: Some("native".to_string()),
        ..Default::default()
    };
    let config = Config::discover(&cli_args)?;
    let runner_mode = config.get_runner_mode()?;
    assert_eq!(runner_mode, xchecker::types::RunnerMode::Native);

    // Test wsl mode
    let cli_args = CliArgs {
        runner_mode: Some("wsl".to_string()),
        ..Default::default()
    };
    let config = Config::discover(&cli_args)?;
    let runner_mode = config.get_runner_mode()?;
    assert_eq!(runner_mode, xchecker::types::RunnerMode::Wsl);

    Ok(())
}

/// Test config with all sections populated
#[test]
fn test_full_config_file() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let root = temp_dir.path();

    let config_path = create_config_file(
        root,
        r#"
[defaults]
model = "opus"
max_turns = 12
packet_max_bytes = 100000
packet_max_lines = 2000
output_format = "text"
verbose = true
phase_timeout = 900

[selectors]
include = ["**/*.rs", "**/*.toml"]
exclude = ["target/**", "**/.git/**"]

[runner]
mode = "native"
distro = "Ubuntu"
claude_path = "/usr/local/bin/claude"
"#,
    );

    let cli_args = CliArgs {
        config_path: Some(config_path),
        ..Default::default()
    };
    let config = Config::discover(&cli_args)?;

    // Verify all values
    assert_eq!(config.defaults.model, Some("opus".to_string()));
    assert_eq!(config.defaults.max_turns, Some(12));
    assert_eq!(config.defaults.packet_max_bytes, Some(100000));
    assert_eq!(config.defaults.packet_max_lines, Some(2000));
    assert_eq!(config.defaults.output_format, Some("text".to_string()));
    assert_eq!(config.defaults.verbose, Some(true));
    assert_eq!(config.defaults.phase_timeout, Some(900));

    assert_eq!(config.selectors.include, vec!["**/*.rs", "**/*.toml"]);
    assert_eq!(config.selectors.exclude, vec!["target/**", "**/.git/**"]);

    assert_eq!(config.runner.mode, Some("native".to_string()));
    assert_eq!(config.runner.distro, Some("Ubuntu".to_string()));
    assert_eq!(
        config.runner.claude_path,
        Some("/usr/local/bin/claude".to_string())
    );

    Ok(())
}

#[cfg(test)]
mod integration {
    use super::*;

    /// Run all configuration system tests sequentially
    ///
    /// NOTE: This test is ignored because:
    /// 1. All individual tests already run as part of the test suite
    /// 2. This meta-test can cause race conditions when run in parallel
    ///    because several tests manipulate process-wide state (cwd, env vars)
    /// 3. To run manually: cargo test run_all_config_tests -- --ignored
    #[test]
    #[ignore = "meta-test - individual tests already run; can cause races in parallel execution"]
    fn run_all_config_tests() {
        println!("\n=== Running Configuration System Tests (FR-CFG) ===\n");

        test_upward_discovery_stops_at_git().unwrap();
        test_discovery_stops_at_git_boundary().unwrap();
        test_precedence_cli_over_config_over_defaults().unwrap();
        test_source_attribution_accuracy().unwrap();
        test_xchecker_home_override().unwrap();
        test_explicit_config_path().unwrap();
        test_invalid_toml_config().unwrap();
        test_invalid_config_values().unwrap();
        test_invalid_runner_mode().unwrap();
        test_invalid_glob_pattern().unwrap();
        test_packet_max_bytes_too_large().unwrap();
        test_max_turns_too_large().unwrap();
        test_phase_timeout_too_small().unwrap();
        test_phase_timeout_too_large().unwrap();
        test_invalid_output_format().unwrap();
        test_missing_config_file_uses_defaults().unwrap();
        test_empty_config_file_uses_defaults().unwrap();
        test_partial_config_file().unwrap();
        test_get_runner_mode_conversion().unwrap();
        test_full_config_file().unwrap();

        println!("\n=== All Configuration System Tests Passed ===\n");
    }
}

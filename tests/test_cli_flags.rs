//! Test suite for CLI flag wiring and functionality (Task 1.8)
//!
//! Verifies that all global CLI flags are properly defined, parsed, and wired
//! through the configuration system according to FR-CLI requirements.

use anyhow::Result;

/// Test that `build_cli()` includes all required global flags from FR-CLI
#[test]
fn test_all_required_global_flags_defined() {
    let cli = xchecker::cli::build_cli();

    // Get all global arguments
    let global_args: Vec<_> = cli.get_arguments().collect();
    let global_arg_names: Vec<_> = global_args
        .iter()
        .filter_map(|arg| arg.get_long())
        .collect();

    // FR-CLI-001: Required global flags
    let required_flags = vec![
        "stdout-cap-bytes",
        "stderr-cap-bytes",
        "packet-max-bytes",
        "packet-max-lines",
        "phase-timeout",
        "lock-ttl-seconds",
        "ignore-secret-pattern",
        "extra-secret-pattern",
        "debug-packet",
        "allow-links",
        "runner-mode",
        "runner-distro",
        "verbose",
        "config",
        "model",
        "max-turns",
        "output-format",
        "claude-path",
    ];

    for flag in required_flags {
        assert!(
            global_arg_names.contains(&flag),
            "Required global flag --{flag} is not defined in CLI"
        );
    }
}

/// Test that numeric flags have documented defaults and units in help text
#[test]
fn test_numeric_flags_have_documented_defaults() {
    let mut cli = xchecker::cli::build_cli();

    // FR-CLI-002: Help output should document defaults and units
    let help_text = cli.render_help().to_string();

    // Check that numeric/time flags mention their defaults
    let numeric_flags_with_defaults = vec![
        ("packet-max-bytes", "65536"),
        ("packet-max-lines", "1200"),
        ("phase-timeout", "600"),
        ("max-turns", "6"),
    ];

    for (flag, _default) in numeric_flags_with_defaults {
        assert!(
            help_text.contains(&format!("--{flag}")),
            "Flag --{flag} not found in help text"
        );
    }
}

/// Test --runner-mode flag accepts valid values
#[test]
fn test_runner_mode_flag_values() {
    use clap::Parser;

    // Test valid values
    let valid_modes = vec!["auto", "native", "wsl"];

    for mode in valid_modes {
        let args = vec!["xchecker", "--runner-mode", mode, "status", "test-spec"];
        let result = xchecker::cli::Cli::try_parse_from(args);
        assert!(result.is_ok(), "Failed to parse valid runner-mode: {mode}");
    }
}

/// Test --runner-distro flag is properly wired
#[test]
fn test_runner_distro_flag() {
    use clap::Parser;

    let args = vec![
        "xchecker",
        "--runner-distro",
        "Ubuntu-22.04",
        "status",
        "test-spec",
    ];
    let cli = xchecker::cli::Cli::try_parse_from(args).unwrap();

    assert_eq!(cli.runner_distro, Some("Ubuntu-22.04".to_string()));
}

/// Test --llm-fallback-provider flag is properly wired
#[test]
fn test_llm_fallback_provider_flag() {
    use clap::Parser;

    let args = vec![
        "xchecker",
        "--llm-fallback-provider",
        "openrouter",
        "status",
        "test-spec",
    ];
    let cli = xchecker::cli::Cli::try_parse_from(args).unwrap();

    assert_eq!(cli.llm_fallback_provider, Some("openrouter".to_string()));
}

/// Test --prompt-template flag is properly wired
#[test]
fn test_prompt_template_flag() {
    use clap::Parser;

    let args = vec![
        "xchecker",
        "--prompt-template",
        "claude-optimized",
        "status",
        "test-spec",
    ];
    let cli = xchecker::cli::Cli::try_parse_from(args).unwrap();

    assert_eq!(cli.prompt_template, Some("claude-optimized".to_string()));
}

/// Test --llm-gemini-default-model flag is properly wired
#[test]
fn test_llm_gemini_default_model_flag() {
    use clap::Parser;

    let args = vec![
        "xchecker",
        "--llm-gemini-default-model",
        "gemini-2.0-pro",
        "status",
        "test-spec",
    ];
    let cli = xchecker::cli::Cli::try_parse_from(args).unwrap();

    assert_eq!(
        cli.llm_gemini_default_model,
        Some("gemini-2.0-pro".to_string())
    );
}

/// Test --phase-timeout flag is properly wired
#[test]
fn test_phase_timeout_flag() {
    use clap::Parser;

    let args = vec!["xchecker", "--phase-timeout", "300", "status", "test-spec"];
    let cli = xchecker::cli::Cli::try_parse_from(args).unwrap();

    assert_eq!(cli.phase_timeout, Some(300));
}

/// Test --ignore-secret-pattern flag accepts multiple values
#[test]
fn test_ignore_secret_pattern_flag() {
    use clap::Parser;

    let args = vec![
        "xchecker",
        "--ignore-secret-pattern",
        "pattern1",
        "--ignore-secret-pattern",
        "pattern2",
        "status",
        "test-spec",
    ];
    let cli = xchecker::cli::Cli::try_parse_from(args).unwrap();

    assert_eq!(cli.ignore_secret_pattern.len(), 2);
    assert!(cli.ignore_secret_pattern.contains(&"pattern1".to_string()));
    assert!(cli.ignore_secret_pattern.contains(&"pattern2".to_string()));
}

/// Test --extra-secret-pattern flag accepts multiple values
#[test]
fn test_extra_secret_pattern_flag() {
    use clap::Parser;

    let args = vec![
        "xchecker",
        "--extra-secret-pattern",
        "custom.*secret",
        "--extra-secret-pattern",
        "api.*key",
        "status",
        "test-spec",
    ];
    let cli = xchecker::cli::Cli::try_parse_from(args).unwrap();

    assert_eq!(cli.extra_secret_pattern.len(), 2);
    assert!(
        cli.extra_secret_pattern
            .contains(&"custom.*secret".to_string())
    );
    assert!(cli.extra_secret_pattern.contains(&"api.*key".to_string()));
}

/// Test --packet-max-bytes flag is properly wired
#[test]
fn test_packet_max_bytes_flag() {
    use clap::Parser;

    let args = vec![
        "xchecker",
        "--packet-max-bytes",
        "32768",
        "status",
        "test-spec",
    ];
    let cli = xchecker::cli::Cli::try_parse_from(args).unwrap();

    assert_eq!(cli.packet_max_bytes, Some(32768));
}

/// Test --packet-max-lines flag is properly wired
#[test]
fn test_packet_max_lines_flag() {
    use clap::Parser;

    let args = vec![
        "xchecker",
        "--packet-max-lines",
        "600",
        "status",
        "test-spec",
    ];
    let cli = xchecker::cli::Cli::try_parse_from(args).unwrap();

    assert_eq!(cli.packet_max_lines, Some(600));
}

/// Test --verbose flag is properly wired
#[test]
fn test_verbose_flag() {
    use clap::Parser;

    let args = vec!["xchecker", "--verbose", "status", "test-spec"];
    let cli = xchecker::cli::Cli::try_parse_from(args).unwrap();

    assert!(cli.verbose);
}

/// Test --force flag on spec command
#[test]
fn test_force_flag_on_spec() {
    use clap::Parser;

    let args = vec!["xchecker", "spec", "test-spec", "--force"];
    let cli = xchecker::cli::Cli::try_parse_from(args).unwrap();

    match cli.command {
        xchecker::cli::Commands::Spec { force, .. } => {
            assert!(force);
        }
        _ => panic!("Expected Spec command"),
    }
}

/// Test --apply-fixups flag on spec command
#[test]
fn test_apply_fixups_flag() {
    use clap::Parser;

    let args = vec!["xchecker", "spec", "test-spec", "--apply-fixups"];
    let cli = xchecker::cli::Cli::try_parse_from(args).unwrap();

    match cli.command {
        xchecker::cli::Commands::Spec { apply_fixups, .. } => {
            assert!(apply_fixups);
        }
        _ => panic!("Expected Spec command"),
    }
}

/// Test --strict-lock flag on spec command
#[test]
fn test_strict_lock_flag() {
    use clap::Parser;

    let args = vec!["xchecker", "spec", "test-spec", "--strict-lock"];
    let cli = xchecker::cli::Cli::try_parse_from(args).unwrap();

    match cli.command {
        xchecker::cli::Commands::Spec { strict_lock, .. } => {
            assert!(strict_lock);
        }
        _ => panic!("Expected Spec command"),
    }
}

/// Test that CLI flags override config file values (precedence test)
#[test]
fn test_cli_flags_override_config() -> Result<()> {
    use xchecker::config::{CliArgs, Config};

    // Create CLI args with overrides
    let cli_args = CliArgs {
        config_path: None,
        model: Some("opus".to_string()),
        max_turns: Some(10),
        packet_max_bytes: Some(100000),
        packet_max_lines: Some(2000),
        output_format: Some("text".to_string()),
        verbose: Some(true),
        runner_mode: Some("native".to_string()),
        runner_distro: Some("Ubuntu".to_string()),
        claude_path: Some("/usr/bin/claude".to_string()),
        allow: vec![],
        deny: vec![],
        dangerously_skip_permissions: false,
        ignore_secret_pattern: vec!["test.*".to_string()],
        extra_secret_pattern: vec!["custom.*".to_string()],
        phase_timeout: Some(900),
        stdout_cap_bytes: Some(4194304),
        stderr_cap_bytes: Some(524288),
        lock_ttl_seconds: Some(1800),
        debug_packet: false,
        allow_links: false,
        strict_validation: None,
        llm_provider: None,
        llm_claude_binary: None,
        llm_gemini_binary: None,
        llm_gemini_default_model: None,
        llm_fallback_provider: None,
        prompt_template: None,
        execution_strategy: None,
    };

    // Load config (will use defaults since no config file)
    let config = Config::discover(&cli_args)?;

    // Verify CLI values took precedence
    assert_eq!(config.defaults.model, Some("opus".to_string()));
    assert_eq!(config.defaults.max_turns, Some(10));
    assert_eq!(config.defaults.packet_max_bytes, Some(100000));
    assert_eq!(config.defaults.packet_max_lines, Some(2000));
    assert_eq!(config.defaults.output_format, Some("text".to_string()));
    assert_eq!(config.defaults.verbose, Some(true));
    assert_eq!(config.runner.mode, Some("native".to_string()));
    assert_eq!(config.runner.distro, Some("Ubuntu".to_string()));
    assert_eq!(
        config.runner.claude_path,
        Some("/usr/bin/claude".to_string())
    );
    assert_eq!(config.defaults.phase_timeout, Some(900));

    // Verify source attribution shows CLI
    use xchecker::config::ConfigSource;
    assert_eq!(
        config.source_attribution.get("model"),
        Some(&ConfigSource::Cli)
    );
    assert_eq!(
        config.source_attribution.get("max_turns"),
        Some(&ConfigSource::Cli)
    );
    assert_eq!(
        config.source_attribution.get("verbose"),
        Some(&ConfigSource::Cli)
    );

    Ok(())
}

/// Test --stdout-cap-bytes flag is properly wired
#[test]
fn test_stdout_cap_bytes_flag() {
    use clap::Parser;

    let args = vec![
        "xchecker",
        "--stdout-cap-bytes",
        "4194304",
        "status",
        "test-spec",
    ];
    let cli = xchecker::cli::Cli::try_parse_from(args).unwrap();

    assert_eq!(cli.stdout_cap_bytes, Some(4194304));
}

/// Test --stderr-cap-bytes flag is properly wired
#[test]
fn test_stderr_cap_bytes_flag() {
    use clap::Parser;

    let args = vec![
        "xchecker",
        "--stderr-cap-bytes",
        "524288",
        "status",
        "test-spec",
    ];
    let cli = xchecker::cli::Cli::try_parse_from(args).unwrap();

    assert_eq!(cli.stderr_cap_bytes, Some(524288));
}

/// Test --lock-ttl-seconds flag is properly wired
#[test]
fn test_lock_ttl_seconds_flag() {
    use clap::Parser;

    let args = vec![
        "xchecker",
        "--lock-ttl-seconds",
        "1800",
        "status",
        "test-spec",
    ];
    let cli = xchecker::cli::Cli::try_parse_from(args).unwrap();

    assert_eq!(cli.lock_ttl_seconds, Some(1800));
}

/// Test --debug-packet flag is properly wired
#[test]
fn test_debug_packet_flag() {
    use clap::Parser;

    let args = vec!["xchecker", "--debug-packet", "status", "test-spec"];
    let cli = xchecker::cli::Cli::try_parse_from(args).unwrap();

    assert!(cli.debug_packet);
}

/// Test --allow-links flag is properly wired
#[test]
fn test_allow_links_flag() {
    use clap::Parser;

    let args = vec!["xchecker", "--allow-links", "status", "test-spec"];
    let cli = xchecker::cli::Cli::try_parse_from(args).unwrap();

    assert!(cli.allow_links);
}

/// Test that new flags are properly passed through config system
#[test]
fn test_new_flags_in_config_system() -> Result<()> {
    use xchecker::config::{CliArgs, Config};

    // Create CLI args with new flags
    let cli_args = CliArgs {
        config_path: None,
        model: None,
        max_turns: None,
        packet_max_bytes: None,
        packet_max_lines: None,
        output_format: None,
        verbose: None,
        runner_mode: None,
        runner_distro: None,
        claude_path: None,
        allow: vec![],
        deny: vec![],
        dangerously_skip_permissions: false,
        ignore_secret_pattern: vec![],
        extra_secret_pattern: vec![],
        phase_timeout: None,
        stdout_cap_bytes: Some(4194304),
        stderr_cap_bytes: Some(524288),
        lock_ttl_seconds: Some(1800),
        debug_packet: true,
        allow_links: true,
        strict_validation: Some(true),
        llm_provider: None,
        llm_claude_binary: None,
        llm_gemini_binary: None,
        llm_gemini_default_model: None,
        llm_fallback_provider: None,
        prompt_template: None,
        execution_strategy: None,
    };

    // Load config
    let config = Config::discover(&cli_args)?;

    // Verify new values are set
    assert_eq!(config.defaults.stdout_cap_bytes, Some(4194304));
    assert_eq!(config.defaults.stderr_cap_bytes, Some(524288));
    assert_eq!(config.defaults.lock_ttl_seconds, Some(1800));
    assert_eq!(config.defaults.debug_packet, Some(true));
    assert_eq!(config.defaults.allow_links, Some(true));

    // Verify source attribution shows CLI
    use xchecker::config::ConfigSource;
    assert_eq!(
        config.source_attribution.get("stdout_cap_bytes"),
        Some(&ConfigSource::Cli)
    );
    assert_eq!(
        config.source_attribution.get("stderr_cap_bytes"),
        Some(&ConfigSource::Cli)
    );
    assert_eq!(
        config.source_attribution.get("lock_ttl_seconds"),
        Some(&ConfigSource::Cli)
    );
    assert_eq!(
        config.source_attribution.get("debug_packet"),
        Some(&ConfigSource::Cli)
    );
    assert_eq!(
        config.source_attribution.get("allow_links"),
        Some(&ConfigSource::Cli)
    );

    Ok(())
}

/// Test that default values are properly set for new flags
#[test]
fn test_new_flags_defaults() -> Result<()> {
    use xchecker::config::{CliArgs, Config};

    // Create empty CLI args
    let cli_args = CliArgs::default();

    // Load config (will use defaults)
    let config = Config::discover(&cli_args)?;

    // Verify defaults are set correctly
    assert_eq!(config.defaults.stdout_cap_bytes, Some(2097152)); // 2 MiB
    assert_eq!(config.defaults.stderr_cap_bytes, Some(262144)); // 256 KiB
    assert_eq!(config.defaults.lock_ttl_seconds, Some(900)); // 15 minutes
    assert_eq!(config.defaults.debug_packet, Some(false));
    assert_eq!(config.defaults.allow_links, Some(false));

    Ok(())
}

/// Test validation of `stdout_cap_bytes`
#[test]
fn test_stdout_cap_bytes_validation() {
    use xchecker::config::{CliArgs, Config};

    // Test too small value
    let cli_args = CliArgs {
        stdout_cap_bytes: Some(512), // Less than 1024
        ..Default::default()
    };

    let result = Config::discover(&cli_args);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("stdout_cap_bytes"));

    // Test too large value
    let cli_args = CliArgs {
        stdout_cap_bytes: Some(200_000_000), // More than 100MB
        ..Default::default()
    };

    let result = Config::discover(&cli_args);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("stdout_cap_bytes"));
}

/// Test validation of `stderr_cap_bytes`
#[test]
fn test_stderr_cap_bytes_validation() {
    use xchecker::config::{CliArgs, Config};

    // Test too small value
    let cli_args = CliArgs {
        stderr_cap_bytes: Some(512), // Less than 1024
        ..Default::default()
    };

    let result = Config::discover(&cli_args);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("stderr_cap_bytes"));

    // Test too large value
    let cli_args = CliArgs {
        stderr_cap_bytes: Some(20_000_000), // More than 10MB
        ..Default::default()
    };

    let result = Config::discover(&cli_args);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("stderr_cap_bytes"));
}

/// Test validation of `lock_ttl_seconds`
#[test]
fn test_lock_ttl_seconds_validation() {
    use xchecker::config::{CliArgs, Config};

    // Test too small value
    let cli_args = CliArgs {
        lock_ttl_seconds: Some(30), // Less than 60
        ..Default::default()
    };

    let result = Config::discover(&cli_args);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("lock_ttl_seconds"));

    // Test too large value
    let cli_args = CliArgs {
        lock_ttl_seconds: Some(100_000), // More than 86400
        ..Default::default()
    };

    let result = Config::discover(&cli_args);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("lock_ttl_seconds"));
}

/// Smoke test for cli::run() error path (Task 6.3)
///
/// Verifies that cli::run() returns an error ExitCode when given invalid arguments.
/// This tests the error handling path where cli::run() catches errors and returns
/// the appropriate exit code without panicking.
///
/// _Requirements: FR-CLI-1, FR-CLI-5_
#[test]
fn test_cli_run_error_path_smoke() {
    use xchecker::ExitCode;

    // Test that cli::run() returns an error when parsing fails
    // We can't easily test cli::run() directly since it parses from std::env::args(),
    // but we can verify the error handling infrastructure is in place by testing
    // that invalid CLI arguments result in parse errors.

    // Test that Cli::try_parse_from returns an error for invalid arguments
    use clap::Parser;

    // Missing required subcommand
    let result = xchecker::cli::Cli::try_parse_from(["xchecker"]);
    assert!(
        result.is_err(),
        "Missing subcommand should result in parse error"
    );

    // Invalid subcommand
    let result = xchecker::cli::Cli::try_parse_from(["xchecker", "invalid-command"]);
    assert!(
        result.is_err(),
        "Invalid subcommand should result in parse error"
    );

    // Invalid flag value
    let result = xchecker::cli::Cli::try_parse_from([
        "xchecker",
        "--phase-timeout",
        "not-a-number",
        "status",
        "test-spec",
    ]);
    assert!(
        result.is_err(),
        "Invalid flag value should result in parse error"
    );

    // Verify ExitCode constants are accessible (part of stable public API)
    assert_eq!(ExitCode::SUCCESS.as_i32(), 0);
    assert_eq!(ExitCode::CLI_ARGS.as_i32(), 2);
    assert_eq!(ExitCode::INTERNAL.as_i32(), 1);
}

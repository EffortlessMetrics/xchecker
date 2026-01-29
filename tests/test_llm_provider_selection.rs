//! Tests for LLM provider configuration and validation
//!
//! **WHITE-BOX TEST**: This test uses internal module APIs (`config::{CliArgs, Config}`,
//! `error::{ConfigError, XCheckerError}`) and may break with internal refactors. These tests
//! are intentionally white-box to validate internal implementation details. See FR-TEST-4 for
//! white-box test policy.
//!
//! Validates that the configuration system correctly:
//! - Accepts supported providers (claude-cli, gemini-cli, openrouter, anthropic)
//! - Rejects unsupported providers during config validation with ConfigError::InvalidValue
//! - Defaults to claude-cli when no provider specified
//! - Validates execution_strategy to only accept "controlled"
//! - Rejects invalid execution strategies like "externaltool" and "external_tool"
//!
//! These tests ensure the multi-provider configuration works correctly
//! in V14 with claude-cli, gemini-cli, openrouter, and anthropic supported.

use std::path::Path;
use xchecker::config::{CliArgs, Config};
use xchecker::error::{ConfigError, XCheckerError};

// Canonical list of supported LLM providers for xchecker v1.0
// Update this list and corresponding tests when adding new providers
const SUPPORTED_PROVIDERS: &[&str] = &["claude-cli", "gemini-cli", "openrouter", "anthropic"];

/// Creates a temp workspace with .xchecker directory and writes a config file
fn with_temp_config<F, R>(config_content: &str, f: F) -> R
where
    F: FnOnce(&Path) -> R,
{
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let xchecker_dir = temp_dir.path().join(".xchecker");
    std::fs::create_dir_all(&xchecker_dir).expect("Failed to create .xchecker dir");
    let config_path = xchecker_dir.join("config.toml");
    std::fs::write(&config_path, config_content).expect("Failed to write config");
    f(&config_path)
}

// ===== Provider Validation Tests =====

#[test]
fn test_no_provider_set_defaults_to_claude_cli() {
    // Setup: Create minimal config (no explicit provider set)
    let cli_args = CliArgs::default();

    // Execute: Discover config (which applies defaults and validates)
    let result = Config::discover(&cli_args);

    // Verify: Should succeed and default to claude-cli
    assert!(
        result.is_ok(),
        "Config discovery should succeed with default provider, got: {:?}",
        result.unwrap_err()
    );

    let config = result.unwrap();

    // Should have defaulted to "claude-cli"
    assert_eq!(
        config.llm.provider,
        Some("claude-cli".to_string()),
        "Provider should default to 'claude-cli'"
    );
}

#[test]
fn test_provider_claude_cli_accepted() {
    // Setup: Create config with explicit claude-cli provider
    let cli_args = CliArgs {
        llm_provider: Some("claude-cli".to_string()),
        ..Default::default()
    };

    // Execute: Discover config (which validates)
    let result = Config::discover(&cli_args);

    // Verify: Should succeed
    assert!(
        result.is_ok(),
        "claude-cli provider should be accepted during config validation, got: {:?}",
        result.unwrap_err()
    );

    let config = result.unwrap();
    assert_eq!(config.llm.provider, Some("claude-cli".to_string()));
}

#[test]
fn test_provider_gemini_cli_accepted() {
    // Setup: Create config with gemini-cli provider (supported in V12+)
    let cli_args = CliArgs {
        llm_provider: Some("gemini-cli".to_string()),
        ..Default::default()
    };

    // Execute: Try to discover config
    let result = Config::discover(&cli_args);

    // Verify: Should succeed (gemini-cli is now supported)
    assert!(
        result.is_ok(),
        "gemini-cli provider should be accepted in V13.1, got: {:?}",
        result.as_ref().err()
    );

    let config = result.unwrap();
    assert_eq!(
        config.llm.provider,
        Some("gemini-cli".to_string()),
        "Provider should be set to gemini-cli"
    );
}

#[test]
fn test_provider_openrouter_accepted() {
    // Setup: Create config with openrouter provider (supported in V13+)
    // HTTP providers require a model to be configured via config file
    let config_content = r#"
[llm]
provider = "openrouter"

[llm.openrouter]
model = "anthropic/claude-3.5-sonnet"
"#;

    with_temp_config(config_content, |config_path| {
        let cli_args = CliArgs {
            config_path: Some(config_path.to_path_buf()),
            ..Default::default()
        };

        // Execute: Try to discover config
        let result = Config::discover(&cli_args);

        // Verify: Should succeed (openrouter is now supported)
        assert!(
            result.is_ok(),
            "openrouter provider should be accepted in V13.1, got: {:?}",
            result.as_ref().err()
        );

        let config = result.unwrap();
        assert_eq!(
            config.llm.provider,
            Some("openrouter".to_string()),
            "Provider should be set to openrouter"
        );
    });
}

#[test]
fn test_provider_anthropic_accepted() {
    // Setup: Create config with anthropic provider (supported in V14)
    // HTTP providers require a model to be configured via config file
    let config_content = r#"
[llm]
provider = "anthropic"

[llm.anthropic]
model = "claude-3-5-sonnet-20241022"
"#;

    with_temp_config(config_content, |config_path| {
        let cli_args = CliArgs {
            config_path: Some(config_path.to_path_buf()),
            ..Default::default()
        };

        // Execute: Try to discover config
        let result = Config::discover(&cli_args);

        // Verify: Should succeed (anthropic is now supported in V14)
        assert!(
            result.is_ok(),
            "anthropic provider should be accepted in V14, got: {:?}",
            result.as_ref().err()
        );

        let config = result.unwrap();
        assert_eq!(
            config.llm.provider,
            Some("anthropic".to_string()),
            "Provider should be set to anthropic"
        );
    });
}

#[test]
fn test_all_supported_providers_are_accepted() {
    // Iterate over the canonical list of supported providers
    // and verify each one is accepted by config validation
    for provider in SUPPORTED_PROVIDERS {
        // HTTP providers (openrouter, anthropic) require a model via config file
        let config_content = match *provider {
            "openrouter" => Some(
                r#"
[llm]
provider = "openrouter"

[llm.openrouter]
model = "anthropic/claude-3.5-sonnet"
"#
                .to_string(),
            ),
            "anthropic" => Some(
                r#"
[llm]
provider = "anthropic"

[llm.anthropic]
model = "claude-3-5-sonnet-20241022"
"#
                .to_string(),
            ),
            _ => None,
        };

        let result = if let Some(content) = config_content {
            // HTTP providers need config file for model
            with_temp_config(&content, |config_path| {
                let cli_args = CliArgs {
                    config_path: Some(config_path.to_path_buf()),
                    ..Default::default()
                };
                Config::discover(&cli_args)
            })
        } else {
            // CLI providers can use direct CLI args
            let cli_args = CliArgs {
                llm_provider: Some(provider.to_string()),
                ..Default::default()
            };
            Config::discover(&cli_args)
        };

        assert!(
            result.is_ok(),
            "Provider '{}' should be accepted during config validation, got: {:?}",
            provider,
            result.as_ref().err()
        );

        let config = result.unwrap();
        assert_eq!(
            config.llm.provider,
            Some(provider.to_string()),
            "Provider should be set to '{}'",
            provider
        );
    }
}

// ===== Execution Strategy Validation Tests =====

#[test]
fn test_execution_strategy_externaltool_rejected() {
    // Setup: Create config with "externaltool" execution strategy
    let cli_args = CliArgs {
        execution_strategy: Some("externaltool".to_string()),
        ..Default::default()
    };

    // Execute: Try to discover config
    let result = Config::discover(&cli_args);

    // Verify: Should fail with ConfigError::InvalidValue
    assert!(
        result.is_err(),
        "execution_strategy='externaltool' should be rejected during config validation"
    );

    let error = result.unwrap_err();
    match error {
        XCheckerError::Config(ConfigError::InvalidValue { key, value }) => {
            assert_eq!(
                key, "llm.execution_strategy",
                "Error should be for llm.execution_strategy key"
            );
            assert!(
                value.contains("externaltool"),
                "Error should mention externaltool, got: {}",
                value
            );
            assert!(
                value.contains("controlled"),
                "Error should mention 'controlled' as the valid strategy, got: {}",
                value
            );
        }
        other => panic!(
            "Expected ConfigError::InvalidValue for externaltool, got: {:?}",
            other
        ),
    }
}

#[test]
fn test_execution_strategy_external_tool_rejected() {
    // Setup: Create config with "external_tool" execution strategy (with underscore)
    let cli_args = CliArgs {
        execution_strategy: Some("external_tool".to_string()),
        ..Default::default()
    };

    // Execute: Try to discover config
    let result = Config::discover(&cli_args);

    // Verify: Should fail with ConfigError::InvalidValue
    assert!(
        result.is_err(),
        "execution_strategy='external_tool' should be rejected during config validation"
    );

    let error = result.unwrap_err();
    match error {
        XCheckerError::Config(ConfigError::InvalidValue { key, value }) => {
            assert_eq!(
                key, "llm.execution_strategy",
                "Error should be for llm.execution_strategy key"
            );
            assert!(
                value.contains("external_tool"),
                "Error should mention external_tool, got: {}",
                value
            );
            assert!(
                value.contains("controlled"),
                "Error should mention 'controlled' as the valid strategy, got: {}",
                value
            );
        }
        other => panic!(
            "Expected ConfigError::InvalidValue for external_tool, got: {:?}",
            other
        ),
    }
}

#[test]
fn test_execution_strategy_controlled_accepted() {
    // Setup: Create config with "controlled" execution strategy (valid)
    let cli_args = CliArgs {
        execution_strategy: Some("controlled".to_string()),
        ..Default::default()
    };

    // Execute: Discover config
    let result = Config::discover(&cli_args);

    // Verify: Should succeed
    assert!(
        result.is_ok(),
        "execution_strategy='controlled' should be accepted, got: {:?}",
        result.unwrap_err()
    );

    let config = result.unwrap();
    assert_eq!(
        config.llm.execution_strategy,
        Some("controlled".to_string())
    );
}

#[test]
fn test_execution_strategy_defaults_to_controlled() {
    // Setup: Create config without explicit execution strategy
    let cli_args = CliArgs::default();

    // Execute: Discover config (should apply default)
    let result = Config::discover(&cli_args);

    // Verify: Should succeed with default "controlled"
    assert!(
        result.is_ok(),
        "Config should succeed with default execution strategy, got: {:?}",
        result.unwrap_err()
    );

    let config = result.unwrap();
    assert_eq!(
        config.llm.execution_strategy,
        Some("controlled".to_string()),
        "Execution strategy should default to 'controlled'"
    );
}

// ===== Edge Case Tests =====

#[test]
fn test_unknown_provider_rejected() {
    // Setup: Create config with completely unknown provider
    let cli_args = CliArgs {
        llm_provider: Some("totally-unknown-provider".to_string()),
        ..Default::default()
    };

    // Execute: Try to discover config
    let result = Config::discover(&cli_args);

    // Verify: Should fail with ConfigError::InvalidValue
    assert!(
        result.is_err(),
        "Unknown provider should be rejected during config validation"
    );

    let error = result.unwrap_err();
    match error {
        XCheckerError::Config(ConfigError::InvalidValue { key, value }) => {
            assert_eq!(key, "llm.provider");
            assert!(
                value.contains("totally-unknown-provider"),
                "Error should mention the unknown provider, got: {}",
                value
            );
        }
        other => panic!("Expected ConfigError::InvalidValue, got: {:?}", other),
    }
}

#[test]
fn test_empty_provider_gets_default() {
    // Setup: Create config with no provider specified
    let cli_args = CliArgs {
        llm_provider: None,
        ..Default::default()
    };

    // Execute: Discover config
    let result = Config::discover(&cli_args);

    // Verify: Should succeed and get default
    assert!(result.is_ok(), "Should succeed with no provider specified");

    let config = result.unwrap();
    assert!(
        config.llm.provider.is_some(),
        "Provider should be set to default"
    );
    assert_eq!(
        config.llm.provider,
        Some("claude-cli".to_string()),
        "Should default to claude-cli"
    );
}

#[test]
fn test_case_sensitivity_in_provider_names() {
    // Test that provider names are case-sensitive
    let test_cases = vec![
        "Claude-CLI", // Wrong case
        "CLAUDE-CLI", // All caps
        "claude-CLI", // Mixed case
        "ClaudeCli",  // No dash
    ];

    for provider in test_cases {
        let cli_args = CliArgs {
            llm_provider: Some(provider.to_string()),
            ..Default::default()
        };

        let result = Config::discover(&cli_args);

        assert!(
            result.is_err(),
            "Provider '{}' should be rejected (case-sensitive), but was accepted",
            provider
        );

        // Verify it's a ConfigError::InvalidValue
        let error = result.unwrap_err();
        assert!(
            matches!(
                error,
                XCheckerError::Config(ConfigError::InvalidValue { .. })
            ),
            "Should get ConfigError::InvalidValue for provider '{}'",
            provider
        );
    }
}

#[test]
fn test_multiple_invalid_configs_together() {
    // Test both invalid provider AND invalid execution strategy
    let cli_args = CliArgs {
        llm_provider: Some("totally-unknown-provider".to_string()),
        execution_strategy: Some("externaltool".to_string()),
        ..Default::default()
    };

    let result = Config::discover(&cli_args);

    // Should fail (provider check comes first)
    assert!(result.is_err(), "Should reject invalid configuration");

    // Verify it's a config error
    let error = result.unwrap_err();
    assert!(
        matches!(
            error,
            XCheckerError::Config(ConfigError::InvalidValue { .. })
        ),
        "Should get ConfigError::InvalidValue"
    );
}

#[test]
fn test_valid_config_accepted() {
    // Integration test: valid config should be accepted
    let cli_args = CliArgs {
        llm_provider: Some("claude-cli".to_string()),
        execution_strategy: Some("controlled".to_string()),
        ..Default::default()
    };

    // Config discovery should succeed
    let config_result = Config::discover(&cli_args);
    assert!(
        config_result.is_ok(),
        "Valid config should be accepted, got: {:?}",
        config_result.unwrap_err()
    );

    let config = config_result.unwrap();
    assert_eq!(config.llm.provider, Some("claude-cli".to_string()));
    assert_eq!(
        config.llm.execution_strategy,
        Some("controlled".to_string())
    );
}

// ===== Error Message Quality Tests =====

#[test]
fn test_error_messages_are_actionable() {
    // Test that error messages provide helpful information for unknown providers
    let test_cases = vec![
        ("totally-unknown-provider", "totally-unknown-provider"),
        ("fake-llm", "fake-llm"),
    ];

    for (provider, expected_in_msg) in test_cases {
        let cli_args = CliArgs {
            llm_provider: Some(provider.to_string()),
            ..Default::default()
        };

        let result = Config::discover(&cli_args);
        assert!(
            result.is_err(),
            "Provider '{}' should be rejected",
            provider
        );

        let error = result.unwrap_err();
        if let XCheckerError::Config(ConfigError::InvalidValue { key: _, value }) = error {
            // Error should mention the provider
            assert!(
                value.contains(expected_in_msg),
                "Error should mention '{}', got: {}",
                expected_in_msg,
                value
            );

            // Error should mention what IS supported
            assert!(
                value.contains("claude-cli") || value.contains("Supported providers"),
                "Error should mention what is supported, got: {}",
                value
            );
        } else {
            panic!(
                "Expected ConfigError::InvalidValue for provider '{}'",
                provider
            );
        }
    }
}

#[test]
fn test_execution_strategy_error_messages() {
    // Test that execution strategy errors are helpful
    let invalid_strategies = vec!["externaltool", "external_tool", "agent", "manual"];

    for strategy in invalid_strategies {
        let cli_args = CliArgs {
            execution_strategy: Some(strategy.to_string()),
            ..Default::default()
        };

        let result = Config::discover(&cli_args);
        assert!(
            result.is_err(),
            "Strategy '{}' should be rejected",
            strategy
        );

        let error = result.unwrap_err();
        if let XCheckerError::Config(ConfigError::InvalidValue { key: _, value }) = error {
            // Error should mention the strategy
            assert!(
                value.contains(strategy),
                "Error should mention '{}', got: {}",
                strategy,
                value
            );

            // Error should mention what IS valid
            assert!(
                value.contains("controlled"),
                "Error should mention 'controlled' as valid strategy, got: {}",
                value
            );
        } else {
            panic!(
                "Expected ConfigError::InvalidValue for strategy '{}'",
                strategy
            );
        }
    }
}

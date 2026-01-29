//! Configuration management for xchecker
//!
//! This module provides hierarchical configuration with discovery and precedence:
//! CLI > file > defaults. Supports TOML configuration files with `[defaults]`,
//! `[selectors]`, and `[runner]` sections.

mod builder;
mod cli_args;
mod discovery;
mod model;
mod selectors;
mod sources;
mod validation;

pub use builder::ConfigBuilder;
pub use cli_args::CliArgs;
pub use model::*;
pub use selectors::ALWAYS_EXCLUDE_PATTERNS;
pub use xchecker_prompt_template::PromptTemplate;
pub use xchecker_selectors::*;
pub use xchecker_utils::types::ConfigSource;

use crate::error::{ConfigError, XCheckerError};
use xchecker_utils::runner::RunnerMode;

impl Config {
    /// Convert runner mode string to enum
    pub fn get_runner_mode(&self) -> Result<RunnerMode, XCheckerError> {
        let mode_str = self.runner.mode.as_deref().unwrap_or("auto");
        match mode_str {
            "auto" => Ok(RunnerMode::Auto),
            "native" => Ok(RunnerMode::Native),
            "wsl" => Ok(RunnerMode::Wsl),
            _ => Err(XCheckerError::Config(ConfigError::InvalidValue {
                key: "runner_mode".to_string(),
                value: format!("Unknown runner mode: {mode_str}"),
            })),
        }
    }

    /// Get the model to use for a specific phase.
    ///
    /// Precedence (highest to lowest):
    /// 1. Phase-specific override (`[phases.<phase>].model`)
    /// 2. Global default (`[defaults].model`)
    /// 3. Hard default: `"haiku"` (fast, cost-effective for testing/development)
    ///
    /// # Example
    ///
    /// ```toml
    /// [defaults]
    /// model = "haiku"
    ///
    /// [phases.design]
    /// model = "sonnet"
    ///
    /// [phases.tasks]
    /// model = "sonnet"
    /// ```
    ///
    /// With the above config:
    /// - `model_for_phase(Requirements)` -> "haiku"
    /// - `model_for_phase(Design)` -> "sonnet"
    /// - `model_for_phase(Tasks)` -> "sonnet"
    #[must_use]
    pub fn model_for_phase(&self, phase: crate::types::PhaseId) -> String {
        use crate::types::PhaseId;

        // First, check for phase-specific override
        let phase_model = match phase {
            PhaseId::Requirements => self.phases.requirements.as_ref(),
            PhaseId::Design => self.phases.design.as_ref(),
            PhaseId::Tasks => self.phases.tasks.as_ref(),
            PhaseId::Review => self.phases.review.as_ref(),
            PhaseId::Fixup => self.phases.fixup.as_ref(),
            PhaseId::Final => self.phases.final_.as_ref(),
        }
        .and_then(|pc| pc.model.clone());

        // Precedence: phase-specific > global default > "haiku"
        phase_model
            .or_else(|| self.defaults.model.clone())
            .unwrap_or_else(|| "haiku".to_string())
    }

    /// Check if strict validation is enabled.
    ///
    /// When strict validation is enabled, phase output validation failures
    /// (meta-summaries, too-short output, missing required sections) become
    /// hard errors that fail the phase. When disabled, validation issues are
    /// logged as warnings only.
    ///
    /// # Returns
    ///
    /// Returns `true` if strict validation is enabled, `false` otherwise.
    /// Defaults to `false` if not explicitly configured.
    #[must_use]
    pub fn strict_validation(&self) -> bool {
        self.defaults.strict_validation.unwrap_or(false)
    }
}

impl xchecker_redaction::SecretConfigProvider for Config {
    fn extra_secret_patterns(&self) -> &[String] {
        &self.security.extra_secret_patterns
    }

    fn ignore_secret_patterns(&self) -> &[String] {
        &self.security.ignore_secret_patterns
    }
}

#[cfg(any(test, feature = "test-utils"))]
impl Config {
    /// Create a minimal Config for testing purposes
    ///
    /// This creates a Config with default values suitable for unit tests
    /// that don't require full configuration discovery.
    pub fn minimal_for_testing() -> Self {
        Config {
            defaults: Defaults::default(),
            selectors: Selectors::default(),
            runner: RunnerConfig::default(),
            llm: LlmConfig {
                provider: None,
                fallback_provider: None,
                claude: None,
                gemini: None,
                openrouter: None,
                anthropic: None,
                execution_strategy: None,
                prompt_template: None,
            },
            phases: PhasesConfig::default(),
            hooks: HooksConfig::default(),
            security: SecurityConfig::default(),
            source_attribution: std::collections::HashMap::new(),
        }
    }
}

/// Test utilities for config testing (available via test-utils feature)
#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils {
    use std::env;

    /// Clear all xchecker config-related environment variables.
    ///
    /// Clears all environment variables starting with `XCHECKER_` to ensure
    /// test isolation. This approach prevents drift as new env vars are added.
    ///
    /// # Safety
    ///
    /// This function removes environment variables, which is inherently
    /// process-global state. Tests using this should be serialized or use
    /// appropriate synchronization.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use xchecker_config::config::test_utils::clear_config_env_vars;
    ///
    /// #[test]
    /// fn my_config_test() {
    ///     clear_config_env_vars();
    ///     // ... test code that may set XCHECKER_* env vars
    /// }
    /// ```
    pub fn clear_config_env_vars() {
        // SAFETY: We're only removing xchecker-specific env vars in test contexts.
        // The env var operations are thread-unsafe but tests using this function
        // should be serialized via config_env_guard or similar synchronization.

        // Collect keys first to avoid iterator invalidation
        let keys: Vec<String> = env::vars()
            .map(|(k, _)| k)
            .filter(|k| k.starts_with("XCHECKER_"))
            .collect();

        // Remove all XCHECKER_* environment variables
        for key in keys {
            // SAFETY: Called only in test contexts with proper synchronization
            unsafe {
                env::remove_var(&key);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, MutexGuard, OnceLock};
    use tempfile::TempDir;

    // Global lock for tests that mutate process-global state (env vars, cwd).
    // Tests that use `config_env_guard()` will be serialized.
    static CONFIG_ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn config_env_guard() -> MutexGuard<'static, ()> {
        let guard = CONFIG_ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap();
        super::test_utils::clear_config_env_vars();
        guard
    }

    fn create_test_config_file(dir: &Path, content: &str) -> PathBuf {
        let xchecker_dir = dir.join(".xchecker");
        crate::paths::ensure_dir_all(&xchecker_dir).unwrap();

        let config_path = xchecker_dir.join("config.toml");
        fs::write(&config_path, content).unwrap();

        config_path
    }

    #[test]
    fn test_default_config() {
        let defaults = Defaults::default();
        assert_eq!(defaults.max_turns, Some(6));
        assert_eq!(defaults.packet_max_bytes, Some(65536));
        assert_eq!(defaults.packet_max_lines, Some(1200));
        assert_eq!(defaults.output_format, Some("stream-json".to_string()));
        assert_eq!(defaults.verbose, Some(false));

        let selectors = Selectors::default();
        assert!(selectors.include.contains(&"README.md".to_string()));
        assert!(selectors.exclude.contains(&"target/**".to_string()));

        let runner = RunnerConfig::default();
        assert_eq!(runner.mode, Some("auto".to_string()));
    }

    #[test]
    fn test_config_discovery_with_cli_override() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();
        let _config_path = create_test_config_file(
            temp_dir.path(),
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
            config_path: None,
            model: Some("opus".to_string()), // CLI override
            max_turns: None,
            packet_max_bytes: None,
            packet_max_lines: None,
            output_format: None,
            verbose: Some(true), // CLI override
            runner_mode: None,
            runner_distro: None,
            claude_path: None,
            allow: vec![],
            deny: vec![],
            dangerously_skip_permissions: false,
            ignore_secret_pattern: vec![],
            extra_secret_pattern: vec![],
            phase_timeout: None,
            stdout_cap_bytes: None,
            stderr_cap_bytes: None,
            lock_ttl_seconds: None,
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

        let config = Config::discover_from(temp_dir.path(), &cli_args).unwrap();

        // CLI overrides should take precedence
        assert_eq!(config.defaults.model, Some("opus".to_string()));
        assert_eq!(config.defaults.verbose, Some(true));

        // Config file values should be used where no CLI override
        assert_eq!(config.defaults.max_turns, Some(10));
        assert_eq!(config.defaults.packet_max_bytes, Some(32768));
        assert_eq!(config.runner.mode, Some("native".to_string()));

        // Check source attribution
        assert_eq!(
            config.source_attribution.get("model"),
            Some(&ConfigSource::Cli)
        );
        assert_eq!(
            config.source_attribution.get("verbose"),
            Some(&ConfigSource::Cli)
        );
    }

    #[test]
    fn test_config_validation() {
        let cli_args = CliArgs {
            max_turns: Some(0), // Invalid
            ..Default::default()
        };

        let result = Config::discover(&cli_args);
        assert!(result.is_err());

        // Assert on structured error type, not string content
        let error = result.unwrap_err();
        match error {
            XCheckerError::Config(ConfigError::InvalidValue { key, .. }) => {
                assert_eq!(key, "max_turns");
            }
            _ => panic!("Expected Config InvalidValue error for max_turns"),
        }
    }

    #[test]
    fn test_effective_config() {
        let _guard = config_env_guard();
        let temp_dir = TempDir::new().unwrap();
        let _config_path = create_test_config_file(
            temp_dir.path(),
            r#"
[defaults]
model = "sonnet"
max_turns = 8
"#,
        );

        let cli_args = CliArgs {
            verbose: Some(true),
            ..Default::default()
        };

        let config = Config::discover_from(temp_dir.path(), &cli_args).unwrap();
        let effective = config.effective_config();

        // Check that values and sources are correctly reported
        assert_eq!(effective.get("model").unwrap().0, "sonnet");
        assert_eq!(effective.get("model").unwrap().1, "config");

        assert_eq!(effective.get("verbose").unwrap().0, "true");
        assert_eq!(effective.get("verbose").unwrap().1, "cli");

        assert_eq!(effective.get("max_turns").unwrap().0, "8");
        assert_eq!(effective.get("max_turns").unwrap().1, "config");
    }

    #[test]
    fn test_invalid_toml_config() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();
        let xchecker_dir = temp_dir.path().join(".xchecker");
        crate::paths::ensure_dir_all(&xchecker_dir).unwrap();

        let config_path = xchecker_dir.join("config.toml");
        fs::write(&config_path, "invalid toml content [[[").unwrap();

        let cli_args = CliArgs::default();

        let result = Config::discover_from(temp_dir.path(), &cli_args);
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("Invalid configuration file")
                || error_msg.contains("Failed to parse TOML config file")
        );
    }

    // ===== Edge Case Tests (Task 9.7) =====
    #[test]
    fn test_config_with_invalid_toml_syntax() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();

        // Test various invalid TOML syntaxes
        let invalid_toml_cases = [
            "[[[ invalid brackets",
            "[defaults\nkey = value", // Missing closing bracket
            "key = ",                 // Missing value
            "[defaults]\nkey value",  // Missing equals
            "[defaults]\nkey = 'unclosed string",
        ];

        for (i, invalid_toml) in invalid_toml_cases.iter().enumerate() {
            let temp_dir = TempDir::new().unwrap();
            let config_path = create_test_config_file(temp_dir.path(), invalid_toml);

            // Use explicit config path instead of changing directory
            let cli_args = CliArgs {
                config_path: Some(config_path),
                ..Default::default()
            };
            let result = Config::discover_from(temp_dir.path(), &cli_args);

            assert!(
                result.is_err(),
                "Should fail for invalid TOML case {i}: {invalid_toml}"
            );
        }
    }

    #[test]
    fn test_config_with_missing_sections() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();

        // Config with only [defaults] section (missing [selectors] and [runner])
        let config_path = create_test_config_file(
            temp_dir.path(),
            r#"
[defaults]
model = "sonnet"
"#,
        );

        let cli_args = CliArgs {
            config_path: Some(config_path),
            ..Default::default()
        };
        let config = Config::discover(&cli_args).unwrap();

        // Should use defaults for missing sections
        assert_eq!(config.defaults.model, Some("sonnet".to_string()));
        assert!(!config.selectors.include.is_empty()); // Should have default selectors
        assert!(config.runner.mode.is_some()); // Should have default runner mode
    }

    #[test]
    fn test_config_with_empty_file() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();

        // Empty config file
        let config_path = create_test_config_file(temp_dir.path(), "");

        let cli_args = CliArgs {
            config_path: Some(config_path),
            ..Default::default()
        };
        let config = Config::discover(&cli_args).unwrap();

        // Should use all defaults
        assert_eq!(config.defaults.max_turns, Some(6));
        assert_eq!(config.defaults.packet_max_bytes, Some(65536));
    }

    #[test]
    fn test_config_with_only_comments() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();

        // Config with only comments
        let config_path = create_test_config_file(
            temp_dir.path(),
            r#"
# This is a comment
# Another comment
# [defaults]
# model = "sonnet"
"#,
        );

        let cli_args = CliArgs {
            config_path: Some(config_path),
            ..Default::default()
        };
        let config = Config::discover(&cli_args).unwrap();

        // Should use all defaults
        assert_eq!(config.defaults.max_turns, Some(6));
    }

    #[test]
    fn test_config_with_wrong_types() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();

        // Config with wrong types (string instead of number)
        let config_path = create_test_config_file(
            temp_dir.path(),
            r#"
[defaults]
max_turns = "not a number"
"#,
        );

        let cli_args = CliArgs {
            config_path: Some(config_path),
            ..Default::default()
        };
        let result = Config::discover(&cli_args);

        assert!(
            result.is_err(),
            "Should fail when max_turns is a string instead of number"
        );
    }

    #[test]
    fn test_config_with_unknown_fields() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();

        // Config with unknown fields (should be ignored by serde's default behavior)
        let config_path = create_test_config_file(
            temp_dir.path(),
            r#"
[defaults]
model = "sonnet"
unknown_field = "should be ignored"
another_unknown = 123

[unknown_section]
key = "value"
"#,
        );

        let cli_args = CliArgs {
            config_path: Some(config_path),
            ..Default::default()
        };
        let config = Config::discover(&cli_args).unwrap();

        // Should successfully load known fields and ignore unknown ones
        assert_eq!(config.defaults.model, Some("sonnet".to_string()));
    }

    #[test]
    fn test_config_validation_with_zero_values() {
        let cli_args = CliArgs {
            packet_max_bytes: Some(0), // Invalid
            ..Default::default()
        };

        let result = Config::discover(&cli_args);
        assert!(result.is_err());

        // Assert on structured error type
        let error = result.unwrap_err();
        match error {
            XCheckerError::Config(ConfigError::InvalidValue { key, .. }) => {
                assert_eq!(key, "packet_max_bytes");
            }
            _ => panic!("Expected Config InvalidValue error for packet_max_bytes"),
        }
    }

    #[test]
    fn test_config_validation_with_excessive_values() {
        // Test packet_max_bytes exceeding limit
        let cli_args = CliArgs {
            packet_max_bytes: Some(20_000_000), // Exceeds 10MB limit
            ..Default::default()
        };

        let result = Config::discover(&cli_args);
        assert!(result.is_err());

        // Assert on structured error type
        let error = result.unwrap_err();
        match error {
            XCheckerError::Config(ConfigError::InvalidValue { key, value }) => {
                assert_eq!(key, "packet_max_bytes");
                assert!(value.contains("exceeds maximum"));
            }
            _ => panic!("Expected Config InvalidValue error for packet_max_bytes exceeding limit"),
        }
    }

    #[test]
    fn test_config_validation_with_invalid_runner_mode() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();

        let config_path = create_test_config_file(
            temp_dir.path(),
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

        assert!(result.is_err(), "Should fail for invalid runner mode");
        assert!(result.unwrap_err().to_string().contains("runner_mode"));
    }

    #[test]
    fn test_config_validation_with_invalid_glob_patterns() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();

        let config_path = create_test_config_file(
            temp_dir.path(),
            r#"
[selectors]
include = ["[invalid-glob"]
exclude = []
"#,
        );

        let cli_args = CliArgs {
            config_path: Some(config_path),
            ..Default::default()
        };
        let result = Config::discover(&cli_args);

        // The validation should catch the invalid glob pattern
        assert!(result.is_err(), "Should fail for invalid glob pattern");
        // The error chain includes the validation error about the glob
        let err_msg = format!("{:?}", result.unwrap_err());
        assert!(
            err_msg.contains("glob")
                || err_msg.contains("Invalid")
                || err_msg.contains("pattern")
                || err_msg.contains("selectors"),
            "Error should be related to glob/pattern validation, got: {err_msg}"
        );
    }

    #[test]
    fn test_config_with_unicode_values() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();

        let config_path = create_test_config_file(
            temp_dir.path(),
            r#"
[defaults]
model = "claude-测试-🚀"

[selectors]
include = ["文档/**/*.md", "README-日本語.md"]
exclude = []
"#,
        );

        let cli_args = CliArgs {
            config_path: Some(config_path),
            ..Default::default()
        };
        let config = Config::discover(&cli_args).unwrap();

        assert_eq!(config.defaults.model, Some("claude-测试-🚀".to_string()));
        assert!(
            config
                .selectors
                .include
                .contains(&"文档/**/*.md".to_string())
        );
    }

    #[test]
    fn test_config_with_very_long_values() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();

        let long_model = "a".repeat(1000);
        let config_content = format!(
            r#"
[defaults]
model = "{long_model}"
"#
        );

        let config_path = create_test_config_file(temp_dir.path(), &config_content);

        let cli_args = CliArgs {
            config_path: Some(config_path),
            ..Default::default()
        };
        let config = Config::discover(&cli_args).unwrap();

        assert_eq!(config.defaults.model, Some(long_model));
    }

    #[test]
    fn test_config_with_special_characters() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();

        let config_path = create_test_config_file(
            temp_dir.path(),
            r#"
[defaults]
model = "sonnet-@#$%"

[selectors]
include = ["**/*.{rs,toml}", "path/with spaces/*.md"]
exclude = ["**/[test]/**"]
"#,
        );

        let cli_args = CliArgs {
            config_path: Some(config_path),
            ..Default::default()
        };
        let config = Config::discover(&cli_args).unwrap();

        assert_eq!(config.defaults.model, Some("sonnet-@#$%".to_string()));
        assert!(
            config
                .selectors
                .include
                .contains(&"path/with spaces/*.md".to_string())
        );
    }

    #[test]
    fn test_config_with_boundary_values() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();

        // Test minimum valid values
        let config_path = create_test_config_file(
            temp_dir.path(),
            r"
[defaults]
max_turns = 1
packet_max_bytes = 1
packet_max_lines = 1
phase_timeout = 5
stdout_cap_bytes = 1024
stderr_cap_bytes = 1024
lock_ttl_seconds = 60
",
        );

        let cli_args = CliArgs {
            config_path: Some(config_path),
            ..Default::default()
        };
        let config = Config::discover(&cli_args).unwrap();

        assert_eq!(config.defaults.max_turns, Some(1));
        assert_eq!(config.defaults.packet_max_bytes, Some(1));
        assert_eq!(config.defaults.phase_timeout, Some(5));
    }

    #[test]
    fn test_config_source_attribution_accuracy() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();

        let config_path = create_test_config_file(
            temp_dir.path(),
            r#"
[defaults]
model = "sonnet"
max_turns = 10
"#,
        );

        let cli_args = CliArgs {
            config_path: Some(config_path),
            verbose: Some(true), // CLI override
            ..Default::default()
        };

        let config = Config::discover(&cli_args).unwrap();

        // Check source attribution
        assert_eq!(
            config.source_attribution.get("verbose"),
            Some(&ConfigSource::Cli)
        );
        assert!(matches!(
            config.source_attribution.get("model"),
            Some(ConfigSource::Config)
        ));
        assert_eq!(
            config.source_attribution.get("packet_max_bytes"),
            Some(&ConfigSource::Default)
        );
    }

    // ===== Edge Case Tests for Task 9.7 =====

    #[test]
    fn test_config_source_attribution() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();
        let config_path = create_test_config_file(
            temp_dir.path(),
            r#"
[defaults]
model = "sonnet"
max_turns = 10
"#,
        );

        let cli_args = CliArgs {
            config_path: Some(config_path),
            model: Some("opus".to_string()), // CLI override
            packet_max_bytes: Some(32768),   // CLI override
            ..Default::default()
        };

        let config = Config::discover(&cli_args).unwrap();

        // Check source attribution
        assert!(matches!(
            config.source_attribution.get("model"),
            Some(ConfigSource::Cli)
        ));
        assert!(matches!(
            config.source_attribution.get("max_turns"),
            Some(ConfigSource::Config)
        ));
        assert!(matches!(
            config.source_attribution.get("packet_max_bytes"),
            Some(ConfigSource::Cli)
        ));
    }

    // ===== LLM Provider and Execution Strategy Validation Tests (V11-V14 enforcement) =====

    #[test]
    fn test_llm_provider_defaults_to_claude_cli() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let cli_args = CliArgs::default();

        let config = Config::discover(&cli_args).unwrap();

        // Should default to claude-cli
        assert_eq!(config.llm.provider, Some("claude-cli".to_string()));
        assert_eq!(
            config.source_attribution.get("llm_provider"),
            Some(&ConfigSource::Default)
        );
    }

    #[test]
    fn test_execution_strategy_defaults_to_controlled() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let cli_args = CliArgs::default();

        let config = Config::discover(&cli_args).unwrap();

        // Should default to controlled
        assert_eq!(
            config.llm.execution_strategy,
            Some("controlled".to_string())
        );
        assert_eq!(
            config.source_attribution.get("execution_strategy"),
            Some(&ConfigSource::Default)
        );
    }

    #[test]
    fn test_llm_provider_rejects_invalid_providers() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();

        // In V14, claude-cli, gemini-cli, openrouter, and anthropic are valid
        // Only unknown providers should be rejected
        let invalid_providers = vec!["openai", "invalid"];

        for provider in invalid_providers {
            let cli_args = CliArgs {
                llm_provider: Some(provider.to_string()),
                ..Default::default()
            };

            let result = Config::discover(&cli_args);
            assert!(result.is_err(), "Should reject provider: {}", provider);

            let error = result.unwrap_err();
            match error {
                XCheckerError::Config(ConfigError::InvalidValue { key, value }) => {
                    assert_eq!(key, "llm.provider");
                    assert!(
                        value.contains(provider),
                        "Error message should mention the invalid provider: {}",
                        value
                    );
                    // anthropic should mention V14+, others should mention supported providers
                    if provider == "anthropic" {
                        assert!(
                            value.contains("V14+") || value.contains("reserved"),
                            "Error message should mention version restriction for anthropic: {}",
                            value
                        );
                    } else {
                        assert!(
                            value.contains("Supported providers")
                                || value.contains("not supported"),
                            "Error message should mention supported providers: {}",
                            value
                        );
                    }
                }
                _ => panic!("Expected Config InvalidValue error for llm.provider"),
            }
        }
    }

    #[test]
    fn test_llm_fallback_provider_from_config_file() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();
        let config_path = create_test_config_file(
            temp_dir.path(),
            r#"
[llm]
provider = "claude-cli"
fallback_provider = "anthropic"

[llm.anthropic]
model = "claude-sonnet-4-20250514"
"#,
        );

        let cli_args = CliArgs {
            config_path: Some(config_path),
            ..Default::default()
        };

        let config = Config::discover(&cli_args).unwrap();

        assert_eq!(config.llm.fallback_provider, Some("anthropic".to_string()));
        assert_eq!(
            config.source_attribution.get("llm_fallback_provider"),
            Some(&ConfigSource::Config)
        );
    }

    #[test]
    fn test_llm_fallback_provider_rejects_invalid_provider() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();
        let config_path = create_test_config_file(
            temp_dir.path(),
            r#"
[llm]
provider = "claude-cli"
fallback_provider = "invalid"
"#,
        );

        let cli_args = CliArgs {
            config_path: Some(config_path),
            ..Default::default()
        };

        let result = Config::discover(&cli_args);
        assert!(result.is_err(), "Should reject invalid fallback provider");

        let error = result.unwrap_err();
        match error {
            XCheckerError::Config(ConfigError::InvalidValue { key, value }) => {
                assert_eq!(key, "llm.fallback_provider");
                assert!(
                    value.contains("invalid"),
                    "Error message should mention the invalid provider: {}",
                    value
                );
            }
            _ => panic!("Expected Config InvalidValue error for llm.fallback_provider"),
        }
    }

    #[test]
    fn test_llm_fallback_provider_prompt_template_incompatible() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();
        let config_path = create_test_config_file(
            temp_dir.path(),
            r#"
[llm]
provider = "claude-cli"
fallback_provider = "openrouter"
prompt_template = "claude-optimized"

[llm.openrouter]
model = "google/gemini-2.0-flash-lite"
"#,
        );

        let cli_args = CliArgs {
            config_path: Some(config_path),
            ..Default::default()
        };

        let result = Config::discover(&cli_args);
        assert!(
            result.is_err(),
            "Should reject incompatible prompt_template for fallback provider"
        );

        let error = result.unwrap_err();
        match error {
            XCheckerError::Config(ConfigError::InvalidValue { key, value }) => {
                assert_eq!(key, "llm.prompt_template");
                assert!(
                    value.contains("openrouter"),
                    "Error should mention fallback provider, got: {}",
                    value
                );
            }
            _ => panic!("Expected Config InvalidValue error for llm.prompt_template"),
        }
    }

    #[test]
    fn test_execution_strategy_rejects_invalid_strategies() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();

        let invalid_strategies = vec!["externaltool", "external_tool", "agent", "batch", "invalid"];

        for strategy in invalid_strategies {
            let cli_args = CliArgs {
                execution_strategy: Some(strategy.to_string()),
                ..Default::default()
            };

            let result = Config::discover(&cli_args);
            assert!(
                result.is_err(),
                "Should reject execution strategy: {}",
                strategy
            );

            let error = result.unwrap_err();
            match error {
                XCheckerError::Config(ConfigError::InvalidValue { key, value }) => {
                    assert_eq!(key, "llm.execution_strategy");
                    assert!(
                        value.contains(strategy),
                        "Error message should mention the invalid strategy: {}",
                        value
                    );
                    assert!(
                        value.contains("V11-V14"),
                        "Error message should mention version restriction: {}",
                        value
                    );
                }
                _ => panic!("Expected Config InvalidValue error for llm.execution_strategy"),
            }
        }
    }

    #[test]
    fn test_llm_provider_accepts_claude_cli() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();

        let cli_args = CliArgs {
            llm_provider: Some("claude-cli".to_string()),
            ..Default::default()
        };

        let config = Config::discover(&cli_args).unwrap();
        assert_eq!(config.llm.provider, Some("claude-cli".to_string()));
    }

    #[test]
    fn test_execution_strategy_accepts_controlled() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();

        let cli_args = CliArgs {
            execution_strategy: Some("controlled".to_string()),
            ..Default::default()
        };

        let config = Config::discover(&cli_args).unwrap();
        assert_eq!(
            config.llm.execution_strategy,
            Some("controlled".to_string())
        );
    }

    #[test]
    fn test_llm_config_from_config_file_with_invalid_provider() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();

        // Use a truly invalid provider that will never be supported
        let config_path = create_test_config_file(
            temp_dir.path(),
            r#"
[llm]
provider = "invalid-provider-xyz"
"#,
        );

        let cli_args = CliArgs {
            config_path: Some(config_path),
            ..Default::default()
        };

        let result = Config::discover(&cli_args);
        assert!(
            result.is_err(),
            "Should reject invalid provider from config file"
        );

        let error = result.unwrap_err();
        match error {
            XCheckerError::Config(ConfigError::InvalidValue { key, value }) => {
                assert_eq!(key, "llm.provider");
                assert!(value.contains("invalid-provider-xyz"));
            }
            _ => panic!("Expected Config InvalidValue error"),
        }
    }

    #[test]
    fn test_llm_config_from_config_file_with_invalid_strategy() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();

        let config_path = create_test_config_file(
            temp_dir.path(),
            r#"
[llm]
execution_strategy = "externaltool"
"#,
        );

        let cli_args = CliArgs {
            config_path: Some(config_path),
            ..Default::default()
        };

        let result = Config::discover(&cli_args);
        assert!(
            result.is_err(),
            "Should reject invalid execution strategy from config file"
        );

        let error = result.unwrap_err();
        match error {
            XCheckerError::Config(ConfigError::InvalidValue { key, value }) => {
                assert_eq!(key, "llm.execution_strategy");
                assert!(value.contains("externaltool"));
            }
            _ => panic!("Expected Config InvalidValue error"),
        }
    }

    #[test]
    fn test_llm_config_from_config_file_with_valid_values() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();

        let config_path = create_test_config_file(
            temp_dir.path(),
            r#"
[llm]
provider = "claude-cli"
execution_strategy = "controlled"
"#,
        );

        let cli_args = CliArgs {
            config_path: Some(config_path.clone()),
            ..Default::default()
        };

        let config = Config::discover(&cli_args).unwrap();

        assert_eq!(config.llm.provider, Some("claude-cli".to_string()));
        assert_eq!(
            config.llm.execution_strategy,
            Some("controlled".to_string())
        );

        // Verify source attribution
        assert!(matches!(
            config.source_attribution.get("llm_provider"),
            Some(ConfigSource::Config)
        ));
        assert!(matches!(
            config.source_attribution.get("execution_strategy"),
            Some(ConfigSource::Config)
        ));
    }

    #[test]
    fn test_llm_config_cli_overrides_config_file() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();

        let config_path = create_test_config_file(
            temp_dir.path(),
            r#"
[llm]
provider = "claude-cli"
execution_strategy = "controlled"
"#,
        );

        // CLI args explicitly set same values (should override with Cli source)
        let cli_args = CliArgs {
            config_path: Some(config_path),
            llm_provider: Some("claude-cli".to_string()),
            execution_strategy: Some("controlled".to_string()),
            ..Default::default()
        };

        let config = Config::discover(&cli_args).unwrap();

        assert_eq!(config.llm.provider, Some("claude-cli".to_string()));
        assert_eq!(
            config.llm.execution_strategy,
            Some("controlled".to_string())
        );

        // Verify CLI takes precedence in source attribution
        assert_eq!(
            config.source_attribution.get("llm_provider"),
            Some(&ConfigSource::Cli)
        );
        assert_eq!(
            config.source_attribution.get("execution_strategy"),
            Some(&ConfigSource::Cli)
        );
    }

    // ===== Prompt Template Validation Tests (Requirement 3.7.6) =====

    #[test]
    fn test_prompt_template_parsing() {
        // Test valid template names
        assert_eq!(
            PromptTemplate::parse("default").unwrap(),
            PromptTemplate::Default
        );
        assert_eq!(
            PromptTemplate::parse("claude-optimized").unwrap(),
            PromptTemplate::ClaudeOptimized
        );
        assert_eq!(
            PromptTemplate::parse("claude_optimized").unwrap(),
            PromptTemplate::ClaudeOptimized
        );
        assert_eq!(
            PromptTemplate::parse("claude").unwrap(),
            PromptTemplate::ClaudeOptimized
        );
        assert_eq!(
            PromptTemplate::parse("openai-compatible").unwrap(),
            PromptTemplate::OpenAiCompatible
        );
        assert_eq!(
            PromptTemplate::parse("openai_compatible").unwrap(),
            PromptTemplate::OpenAiCompatible
        );
        assert_eq!(
            PromptTemplate::parse("openai").unwrap(),
            PromptTemplate::OpenAiCompatible
        );
        assert_eq!(
            PromptTemplate::parse("openrouter").unwrap(),
            PromptTemplate::OpenAiCompatible
        );

        // Test case insensitivity
        assert_eq!(
            PromptTemplate::parse("DEFAULT").unwrap(),
            PromptTemplate::Default
        );
        assert_eq!(
            PromptTemplate::parse("Claude-Optimized").unwrap(),
            PromptTemplate::ClaudeOptimized
        );

        // Test invalid template names
        assert!(PromptTemplate::parse("invalid").is_err());
        assert!(PromptTemplate::parse("unknown-template").is_err());
    }

    #[test]
    fn test_prompt_template_provider_compatibility() {
        // Default template is compatible with all providers
        assert!(
            PromptTemplate::Default
                .validate_provider_compatibility("claude-cli")
                .is_ok()
        );
        assert!(
            PromptTemplate::Default
                .validate_provider_compatibility("gemini-cli")
                .is_ok()
        );
        assert!(
            PromptTemplate::Default
                .validate_provider_compatibility("openrouter")
                .is_ok()
        );
        assert!(
            PromptTemplate::Default
                .validate_provider_compatibility("anthropic")
                .is_ok()
        );

        // Claude-optimized template is compatible with Claude CLI and Anthropic
        assert!(
            PromptTemplate::ClaudeOptimized
                .validate_provider_compatibility("claude-cli")
                .is_ok()
        );
        assert!(
            PromptTemplate::ClaudeOptimized
                .validate_provider_compatibility("anthropic")
                .is_ok()
        );
        assert!(
            PromptTemplate::ClaudeOptimized
                .validate_provider_compatibility("gemini-cli")
                .is_err()
        );
        assert!(
            PromptTemplate::ClaudeOptimized
                .validate_provider_compatibility("openrouter")
                .is_err()
        );

        // OpenAI-compatible template is compatible with OpenRouter and Gemini
        assert!(
            PromptTemplate::OpenAiCompatible
                .validate_provider_compatibility("openrouter")
                .is_ok()
        );
        assert!(
            PromptTemplate::OpenAiCompatible
                .validate_provider_compatibility("gemini-cli")
                .is_ok()
        );
        assert!(
            PromptTemplate::OpenAiCompatible
                .validate_provider_compatibility("claude-cli")
                .is_err()
        );
        assert!(
            PromptTemplate::OpenAiCompatible
                .validate_provider_compatibility("anthropic")
                .is_err()
        );
    }

    #[test]
    fn test_prompt_template_as_str() {
        assert_eq!(PromptTemplate::Default.as_str(), "default");
        assert_eq!(PromptTemplate::ClaudeOptimized.as_str(), "claude-optimized");
        assert_eq!(
            PromptTemplate::OpenAiCompatible.as_str(),
            "openai-compatible"
        );
    }

    #[test]
    fn test_prompt_template_compatible_providers() {
        assert_eq!(
            PromptTemplate::Default.compatible_providers(),
            &["claude-cli", "gemini-cli", "openrouter", "anthropic"]
        );
        assert_eq!(
            PromptTemplate::ClaudeOptimized.compatible_providers(),
            &["claude-cli", "anthropic"]
        );
        assert_eq!(
            PromptTemplate::OpenAiCompatible.compatible_providers(),
            &["openrouter", "gemini-cli"]
        );
    }

    #[test]
    fn test_config_with_valid_prompt_template() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();

        // Test default template with claude-cli
        let config_path = create_test_config_file(
            temp_dir.path(),
            r#"
[llm]
provider = "claude-cli"
prompt_template = "default"
"#,
        );

        let cli_args = CliArgs {
            config_path: Some(config_path),
            ..Default::default()
        };

        let config = Config::discover(&cli_args).unwrap();
        assert_eq!(config.llm.prompt_template, Some("default".to_string()));
    }

    #[test]
    fn test_config_with_claude_optimized_template_and_claude_provider() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();

        let config_path = create_test_config_file(
            temp_dir.path(),
            r#"
[llm]
provider = "claude-cli"
prompt_template = "claude-optimized"
"#,
        );

        let cli_args = CliArgs {
            config_path: Some(config_path),
            ..Default::default()
        };

        let config = Config::discover(&cli_args).unwrap();
        assert_eq!(
            config.llm.prompt_template,
            Some("claude-optimized".to_string())
        );
    }

    #[test]
    fn test_config_with_openai_compatible_template_and_openrouter_provider() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();

        let config_path = create_test_config_file(
            temp_dir.path(),
            r#"
[llm]
provider = "openrouter"
prompt_template = "openai-compatible"

[llm.openrouter]
model = "google/gemini-2.0-flash-lite"
"#,
        );

        let cli_args = CliArgs {
            config_path: Some(config_path),
            ..Default::default()
        };

        let config = Config::discover(&cli_args).unwrap();
        assert_eq!(
            config.llm.prompt_template,
            Some("openai-compatible".to_string())
        );
    }

    #[test]
    fn test_config_rejects_incompatible_template_and_provider() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();

        // Claude-optimized template with OpenRouter provider should fail
        let config_path = create_test_config_file(
            temp_dir.path(),
            r#"
[llm]
provider = "openrouter"
prompt_template = "claude-optimized"

[llm.openrouter]
model = "google/gemini-2.0-flash-lite"
"#,
        );

        let cli_args = CliArgs {
            config_path: Some(config_path),
            ..Default::default()
        };

        let result = Config::discover(&cli_args);
        assert!(
            result.is_err(),
            "Should reject incompatible template and provider"
        );

        let error = result.unwrap_err();
        match error {
            XCheckerError::Config(ConfigError::InvalidValue { key, value }) => {
                assert_eq!(key, "llm.prompt_template");
                assert!(value.contains("not compatible"));
                assert!(value.contains("openrouter"));
            }
            _ => panic!("Expected Config InvalidValue error for incompatible template"),
        }
    }

    #[test]
    fn test_config_rejects_openai_template_with_claude_provider() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();

        // OpenAI-compatible template with Claude CLI provider should fail
        let config_path = create_test_config_file(
            temp_dir.path(),
            r#"
[llm]
provider = "claude-cli"
prompt_template = "openai-compatible"
"#,
        );

        let cli_args = CliArgs {
            config_path: Some(config_path),
            ..Default::default()
        };

        let result = Config::discover(&cli_args);
        assert!(
            result.is_err(),
            "Should reject incompatible template and provider"
        );

        let error = result.unwrap_err();
        match error {
            XCheckerError::Config(ConfigError::InvalidValue { key, value }) => {
                assert_eq!(key, "llm.prompt_template");
                assert!(value.contains("not compatible"));
                assert!(value.contains("claude-cli"));
            }
            _ => panic!("Expected Config InvalidValue error for incompatible template"),
        }
    }

    #[test]
    fn test_config_rejects_invalid_template_name() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();

        let config_path = create_test_config_file(
            temp_dir.path(),
            r#"
[llm]
provider = "claude-cli"
prompt_template = "invalid-template-name"
"#,
        );

        let cli_args = CliArgs {
            config_path: Some(config_path),
            ..Default::default()
        };

        let result = Config::discover(&cli_args);
        assert!(result.is_err(), "Should reject invalid template name");

        let error = result.unwrap_err();
        match error {
            XCheckerError::Config(ConfigError::InvalidValue { key, value }) => {
                assert_eq!(key, "llm.prompt_template");
                assert!(value.contains("Unknown prompt template"));
                assert!(value.contains("invalid-template-name"));
            }
            _ => panic!("Expected Config InvalidValue error for invalid template name"),
        }
    }

    #[test]
    fn test_config_without_prompt_template_uses_default() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();

        // Config without prompt_template should work (uses implicit default)
        let config_path = create_test_config_file(
            temp_dir.path(),
            r#"
[llm]
provider = "claude-cli"
"#,
        );

        let cli_args = CliArgs {
            config_path: Some(config_path),
            ..Default::default()
        };

        let config = Config::discover(&cli_args).unwrap();
        // prompt_template should be None (implicit default behavior)
        assert_eq!(config.llm.prompt_template, None);
    }

    // ===== Per-Phase Model Configuration Tests (B-series feature) =====

    #[test]
    fn test_model_for_phase_defaults_to_global() {
        use crate::types::PhaseId;

        let mut cfg = Config::minimal_for_testing();
        cfg.defaults.model = Some("haiku".to_string());

        // All phases should use the global default
        assert_eq!(cfg.model_for_phase(PhaseId::Requirements), "haiku");
        assert_eq!(cfg.model_for_phase(PhaseId::Design), "haiku");
        assert_eq!(cfg.model_for_phase(PhaseId::Tasks), "haiku");
        assert_eq!(cfg.model_for_phase(PhaseId::Review), "haiku");
        assert_eq!(cfg.model_for_phase(PhaseId::Fixup), "haiku");
        assert_eq!(cfg.model_for_phase(PhaseId::Final), "haiku");
    }

    #[test]
    fn test_model_for_phase_defaults_to_haiku_when_no_global() {
        use crate::types::PhaseId;

        let cfg = Config::minimal_for_testing();
        // No model set anywhere - should default to "haiku"

        assert_eq!(cfg.model_for_phase(PhaseId::Requirements), "haiku");
        assert_eq!(cfg.model_for_phase(PhaseId::Design), "haiku");
    }

    #[test]
    fn test_model_for_phase_with_overrides() {
        use crate::types::PhaseId;

        let mut cfg = Config::minimal_for_testing();
        cfg.defaults.model = Some("haiku".to_string());

        // Set per-phase overrides for design and tasks
        cfg.phases.design = Some(PhaseConfig {
            model: Some("sonnet".to_string()),
            ..Default::default()
        });
        cfg.phases.tasks = Some(PhaseConfig {
            model: Some("sonnet".to_string()),
            ..Default::default()
        });

        // Requirements should use global default
        assert_eq!(cfg.model_for_phase(PhaseId::Requirements), "haiku");
        // Design and Tasks should use per-phase override
        assert_eq!(cfg.model_for_phase(PhaseId::Design), "sonnet");
        assert_eq!(cfg.model_for_phase(PhaseId::Tasks), "sonnet");
        // Review should use global default
        assert_eq!(cfg.model_for_phase(PhaseId::Review), "haiku");
    }

    #[test]
    fn test_model_for_phase_override_without_global_default() {
        use crate::types::PhaseId;

        let mut cfg = Config::minimal_for_testing();
        // No global model set

        // Set per-phase override only for design
        cfg.phases.design = Some(PhaseConfig {
            model: Some("opus".to_string()),
            ..Default::default()
        });

        // Design should use per-phase override
        assert_eq!(cfg.model_for_phase(PhaseId::Design), "opus");
        // Other phases should fall back to hard default "haiku"
        assert_eq!(cfg.model_for_phase(PhaseId::Requirements), "haiku");
        assert_eq!(cfg.model_for_phase(PhaseId::Tasks), "haiku");
    }

    #[test]
    fn test_model_for_phase_with_all_overrides() {
        use crate::types::PhaseId;

        let mut cfg = Config::minimal_for_testing();
        cfg.defaults.model = Some("haiku".to_string());

        // Set different models for each phase
        cfg.phases.requirements = Some(PhaseConfig {
            model: Some("haiku".to_string()),
            ..Default::default()
        });
        cfg.phases.design = Some(PhaseConfig {
            model: Some("sonnet".to_string()),
            ..Default::default()
        });
        cfg.phases.tasks = Some(PhaseConfig {
            model: Some("sonnet".to_string()),
            ..Default::default()
        });
        cfg.phases.review = Some(PhaseConfig {
            model: Some("opus".to_string()),
            ..Default::default()
        });
        cfg.phases.fixup = Some(PhaseConfig {
            model: Some("haiku".to_string()),
            ..Default::default()
        });
        cfg.phases.final_ = Some(PhaseConfig {
            model: Some("opus".to_string()),
            ..Default::default()
        });

        assert_eq!(cfg.model_for_phase(PhaseId::Requirements), "haiku");
        assert_eq!(cfg.model_for_phase(PhaseId::Design), "sonnet");
        assert_eq!(cfg.model_for_phase(PhaseId::Tasks), "sonnet");
        assert_eq!(cfg.model_for_phase(PhaseId::Review), "opus");
        assert_eq!(cfg.model_for_phase(PhaseId::Fixup), "haiku");
        assert_eq!(cfg.model_for_phase(PhaseId::Final), "opus");
    }

    #[test]
    fn test_phases_config_from_toml_file() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();

        let config_path = create_test_config_file(
            temp_dir.path(),
            r#"
[defaults]
model = "haiku"

[phases.design]
model = "sonnet"

[phases.tasks]
model = "sonnet"
"#,
        );

        let cli_args = CliArgs {
            config_path: Some(config_path),
            ..Default::default()
        };

        let config = Config::discover(&cli_args).unwrap();

        use crate::types::PhaseId;

        // Requirements should use global default
        assert_eq!(config.model_for_phase(PhaseId::Requirements), "haiku");
        // Design and Tasks should use per-phase override
        assert_eq!(config.model_for_phase(PhaseId::Design), "sonnet");
        assert_eq!(config.model_for_phase(PhaseId::Tasks), "sonnet");
        // Review should use global default
        assert_eq!(config.model_for_phase(PhaseId::Review), "haiku");
    }

    #[test]
    fn test_phases_config_with_all_fields() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();

        let config_path = create_test_config_file(
            temp_dir.path(),
            r#"
[defaults]
model = "haiku"
max_turns = 6

[phases.review]
model = "opus"
max_turns = 10
phase_timeout = 1200
"#,
        );

        let cli_args = CliArgs {
            config_path: Some(config_path),
            ..Default::default()
        };

        let config = Config::discover(&cli_args).unwrap();

        // Verify phases config was loaded
        assert!(config.phases.review.is_some());
        let review_config = config.phases.review.as_ref().unwrap();
        assert_eq!(review_config.model, Some("opus".to_string()));
        assert_eq!(review_config.max_turns, Some(10));
        assert_eq!(review_config.phase_timeout, Some(1200));

        // Verify model_for_phase works
        use crate::types::PhaseId;
        assert_eq!(config.model_for_phase(PhaseId::Review), "opus");
    }

    #[test]
    fn test_phases_config_empty_section() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();

        // Empty phases section should not cause errors
        let config_path = create_test_config_file(
            temp_dir.path(),
            r#"
[defaults]
model = "haiku"

[phases]
"#,
        );

        let cli_args = CliArgs {
            config_path: Some(config_path),
            ..Default::default()
        };

        let config = Config::discover(&cli_args).unwrap();

        use crate::types::PhaseId;
        // Should use global defaults since no per-phase overrides
        assert_eq!(config.model_for_phase(PhaseId::Requirements), "haiku");
        assert_eq!(config.model_for_phase(PhaseId::Design), "haiku");
    }

    #[test]
    fn test_phases_final_uses_serde_rename() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();

        // "final" is a reserved keyword in Rust, so we use "final_" internally
        // but TOML uses "final"
        let config_path = create_test_config_file(
            temp_dir.path(),
            r#"
[defaults]
model = "haiku"

[phases.final]
model = "opus"
"#,
        );

        let cli_args = CliArgs {
            config_path: Some(config_path),
            ..Default::default()
        };

        let config = Config::discover(&cli_args).unwrap();

        use crate::types::PhaseId;
        assert_eq!(config.model_for_phase(PhaseId::Final), "opus");
    }

    // ===== Strict Validation Configuration Tests (P1 feature) =====

    #[test]
    fn test_strict_validation_defaults_to_false() {
        let cfg = Config::minimal_for_testing();
        // Default should be false (soft validation)
        assert!(!cfg.strict_validation());
    }

    #[test]
    fn test_strict_validation_when_set_true() {
        let mut cfg = Config::minimal_for_testing();
        cfg.defaults.strict_validation = Some(true);
        assert!(cfg.strict_validation());
    }

    #[test]
    fn test_strict_validation_when_set_false() {
        let mut cfg = Config::minimal_for_testing();
        cfg.defaults.strict_validation = Some(false);
        assert!(!cfg.strict_validation());
    }

    #[test]
    fn test_strict_validation_from_toml_file() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();

        let config_path = create_test_config_file(
            temp_dir.path(),
            r#"
[defaults]
strict_validation = true
"#,
        );

        let cli_args = CliArgs {
            config_path: Some(config_path),
            ..Default::default()
        };

        let config = Config::discover(&cli_args).unwrap();
        assert!(config.strict_validation());
    }

    #[test]
    fn test_strict_validation_from_toml_file_false() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();

        let config_path = create_test_config_file(
            temp_dir.path(),
            r#"
[defaults]
strict_validation = false
"#,
        );

        let cli_args = CliArgs {
            config_path: Some(config_path),
            ..Default::default()
        };

        let config = Config::discover(&cli_args).unwrap();
        assert!(!config.strict_validation());
    }

    // ===== ConfigBuilder Tests (Task 2.1) =====

    #[test]
    fn test_config_builder_default() {
        let config = Config::builder().build().unwrap();

        // Should use all defaults
        assert_eq!(config.defaults.max_turns, Some(6));
        assert_eq!(config.defaults.packet_max_bytes, Some(65536));
        assert_eq!(config.defaults.packet_max_lines, Some(1200));
        assert_eq!(config.defaults.phase_timeout, Some(600));
        assert_eq!(config.runner.mode, Some("auto".to_string()));
        assert_eq!(config.llm.provider, Some("claude-cli".to_string()));
        assert_eq!(
            config.llm.execution_strategy,
            Some("controlled".to_string())
        );
    }

    #[test]
    fn test_config_builder_with_packet_max_bytes() {
        let config = Config::builder().packet_max_bytes(32768).build().unwrap();

        assert_eq!(config.defaults.packet_max_bytes, Some(32768));
        assert_eq!(
            config.source_attribution.get("packet_max_bytes"),
            Some(&ConfigSource::Programmatic)
        );
    }

    #[test]
    fn test_config_builder_with_packet_max_lines() {
        let config = Config::builder().packet_max_lines(600).build().unwrap();

        assert_eq!(config.defaults.packet_max_lines, Some(600));
        assert_eq!(
            config.source_attribution.get("packet_max_lines"),
            Some(&ConfigSource::Programmatic)
        );
    }

    #[test]
    fn test_config_builder_with_phase_timeout() {
        use std::time::Duration;

        let config = Config::builder()
            .phase_timeout(Duration::from_secs(300))
            .build()
            .unwrap();

        assert_eq!(config.defaults.phase_timeout, Some(300));
        assert_eq!(
            config.source_attribution.get("phase_timeout"),
            Some(&ConfigSource::Programmatic)
        );
    }

    #[test]
    fn test_config_builder_with_runner_mode() {
        let config = Config::builder().runner_mode("native").build().unwrap();

        assert_eq!(config.runner.mode, Some("native".to_string()));
        assert_eq!(
            config.source_attribution.get("runner_mode"),
            Some(&ConfigSource::Programmatic)
        );
    }

    #[test]
    fn test_config_builder_with_state_dir() {
        let config = Config::builder().state_dir("/custom/path").build().unwrap();

        // state_dir is tracked in source attribution
        assert_eq!(
            config.source_attribution.get("state_dir"),
            Some(&ConfigSource::Programmatic)
        );
    }

    #[test]
    fn test_config_builder_with_all_options() {
        use std::time::Duration;

        let config = Config::builder()
            .state_dir("/custom/state")
            .packet_max_bytes(32768)
            .packet_max_lines(600)
            .phase_timeout(Duration::from_secs(300))
            .runner_mode("native")
            .model("sonnet")
            .max_turns(10)
            .verbose(true)
            .llm_provider("claude-cli")
            .execution_strategy("controlled")
            .build()
            .unwrap();

        assert_eq!(config.defaults.packet_max_bytes, Some(32768));
        assert_eq!(config.defaults.packet_max_lines, Some(600));
        assert_eq!(config.defaults.phase_timeout, Some(300));
        assert_eq!(config.runner.mode, Some("native".to_string()));
        assert_eq!(config.defaults.model, Some("sonnet".to_string()));
        assert_eq!(config.defaults.max_turns, Some(10));
        assert_eq!(config.defaults.verbose, Some(true));
        assert_eq!(config.llm.provider, Some("claude-cli".to_string()));
        assert_eq!(
            config.llm.execution_strategy,
            Some("controlled".to_string())
        );
    }

    #[test]
    fn test_config_builder_validation_rejects_invalid_packet_max_bytes() {
        let result = Config::builder().packet_max_bytes(0).build();

        assert!(result.is_err());
        let error = result.unwrap_err();
        match error {
            XCheckerError::Config(ConfigError::InvalidValue { key, .. }) => {
                assert_eq!(key, "packet_max_bytes");
            }
            _ => panic!("Expected Config InvalidValue error for packet_max_bytes"),
        }
    }

    #[test]
    fn test_config_builder_validation_rejects_excessive_packet_max_bytes() {
        let result = Config::builder()
            .packet_max_bytes(20_000_000) // Exceeds 10MB limit
            .build();

        assert!(result.is_err());
        let error = result.unwrap_err();
        match error {
            XCheckerError::Config(ConfigError::InvalidValue { key, value }) => {
                assert_eq!(key, "packet_max_bytes");
                assert!(value.contains("exceeds maximum"));
            }
            _ => panic!("Expected Config InvalidValue error for packet_max_bytes"),
        }
    }

    #[test]
    fn test_config_builder_validation_rejects_invalid_runner_mode() {
        let result = Config::builder().runner_mode("invalid_mode").build();

        assert!(result.is_err());
        let error = result.unwrap_err();
        match error {
            XCheckerError::Config(ConfigError::InvalidValue { key, .. }) => {
                assert_eq!(key, "runner_mode");
            }
            _ => panic!("Expected Config InvalidValue error for runner_mode"),
        }
    }

    #[test]
    fn test_config_builder_validation_rejects_invalid_execution_strategy() {
        let result = Config::builder().execution_strategy("externaltool").build();

        assert!(result.is_err());
        let error = result.unwrap_err();
        match error {
            XCheckerError::Config(ConfigError::InvalidValue { key, value }) => {
                assert_eq!(key, "llm.execution_strategy");
                assert!(value.contains("externaltool"));
            }
            _ => panic!("Expected Config InvalidValue error for execution_strategy"),
        }
    }

    #[test]
    fn test_config_builder_chaining() {
        // Test that builder methods can be chained in any order
        let config = Config::builder()
            .runner_mode("native")
            .packet_max_bytes(32768)
            .phase_timeout(std::time::Duration::from_secs(300))
            .packet_max_lines(600)
            .build()
            .unwrap();

        assert_eq!(config.defaults.packet_max_bytes, Some(32768));
        assert_eq!(config.defaults.packet_max_lines, Some(600));
        assert_eq!(config.defaults.phase_timeout, Some(300));
        assert_eq!(config.runner.mode, Some("native".to_string()));
    }

    #[test]
    fn test_config_builder_default_impl() {
        // Test that ConfigBuilder implements Default
        let builder = ConfigBuilder::default();
        let config = builder.build().unwrap();

        // Should use all defaults
        assert_eq!(config.defaults.max_turns, Some(6));
        assert_eq!(config.defaults.packet_max_bytes, Some(65536));
    }

    // ===== discover_from_env_and_fs Tests =====

    #[test]
    fn test_discover_from_env_and_fs_uses_defaults() {
        let _guard = config_env_guard();
        // Use isolated home to avoid picking up real config files
        let _home = crate::paths::with_isolated_home();

        // discover_from_env_and_fs should work with no config file present
        let config = Config::discover_from_env_and_fs().unwrap();

        // Should use all defaults
        assert_eq!(config.defaults.max_turns, Some(6));
        assert_eq!(config.defaults.packet_max_bytes, Some(65536));
        assert_eq!(config.defaults.packet_max_lines, Some(1200));
        assert_eq!(config.llm.provider, Some("claude-cli".to_string()));
        assert_eq!(
            config.llm.execution_strategy,
            Some("controlled".to_string())
        );

        // Source attribution should be defaults
        assert_eq!(
            config.source_attribution.get("max_turns"),
            Some(&ConfigSource::Default)
        );
        assert_eq!(
            config.source_attribution.get("llm_provider"),
            Some(&ConfigSource::Default)
        );
    }

    #[test]
    fn test_discover_from_env_and_fs_reads_config_file() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();

        // Create a config file in a temp directory's .xchecker folder
        // (simulating a project with a config file)
        let _config_path = create_test_config_file(
            temp_dir.path(),
            r#"
[defaults]
model = "sonnet"
max_turns = 10
packet_max_bytes = 32768
"#,
        );

        // Use discover_from with the temp directory to simulate being in that project
        let config = Config::discover_from(temp_dir.path(), &CliArgs::default()).unwrap();

        // Should use values from config file
        assert_eq!(config.defaults.model, Some("sonnet".to_string()));
        assert_eq!(config.defaults.max_turns, Some(10));
        assert_eq!(config.defaults.packet_max_bytes, Some(32768));

        // Values not in config file should use defaults
        assert_eq!(config.defaults.packet_max_lines, Some(1200));

        // Source attribution should reflect config file for overridden values
        assert!(matches!(
            config.source_attribution.get("model"),
            Some(ConfigSource::Config)
        ));
        assert!(matches!(
            config.source_attribution.get("max_turns"),
            Some(ConfigSource::Config)
        ));
        // Default values should have Default source
        assert_eq!(
            config.source_attribution.get("packet_max_lines"),
            Some(&ConfigSource::Default)
        );
    }

    #[test]
    fn test_discover_from_env_and_fs_matches_discover_with_empty_cli_args() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();

        // Both methods should produce equivalent configs when no config file exists
        // (discover_from_env_and_fs is equivalent to discover with empty CliArgs)
        let config_env_fs = Config::discover_from_env_and_fs().unwrap();
        let config_discover = Config::discover(&CliArgs::default()).unwrap();

        // Compare key values - both should use defaults
        assert_eq!(config_env_fs.defaults.model, config_discover.defaults.model);
        assert_eq!(
            config_env_fs.defaults.max_turns,
            config_discover.defaults.max_turns
        );
        assert_eq!(
            config_env_fs.defaults.packet_max_bytes,
            config_discover.defaults.packet_max_bytes
        );
        assert_eq!(config_env_fs.llm.provider, config_discover.llm.provider);
        assert_eq!(
            config_env_fs.llm.execution_strategy,
            config_discover.llm.execution_strategy
        );

        // Source attribution should also match
        assert_eq!(
            config_env_fs.source_attribution.get("max_turns"),
            config_discover.source_attribution.get("max_turns")
        );
        assert_eq!(
            config_env_fs.source_attribution.get("llm_provider"),
            config_discover.source_attribution.get("llm_provider")
        );
    }

    // ===== Security Config Tests (Task 23.2) =====

    #[test]
    fn test_security_config_defaults() {
        let config = Config::builder().build().unwrap();

        // Security config should have empty defaults
        assert!(config.security.extra_secret_patterns.is_empty());
        assert!(config.security.ignore_secret_patterns.is_empty());
    }

    #[test]
    fn test_security_config_from_toml_file() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();

        let config_path = create_test_config_file(
            temp_dir.path(),
            r#"
[security]
extra_secret_patterns = ["CUSTOM_[A-Z0-9]{32}", "MY_SECRET_[A-Za-z0-9]{20}"]
ignore_secret_patterns = ["github_pat", "aws_access_key"]
"#,
        );

        let cli_args = CliArgs {
            config_path: Some(config_path),
            ..Default::default()
        };
        let config = Config::discover(&cli_args).unwrap();

        // Should have extra patterns from file
        assert_eq!(config.security.extra_secret_patterns.len(), 2);
        assert!(
            config
                .security
                .extra_secret_patterns
                .contains(&"CUSTOM_[A-Z0-9]{32}".to_string())
        );
        assert!(
            config
                .security
                .extra_secret_patterns
                .contains(&"MY_SECRET_[A-Za-z0-9]{20}".to_string())
        );

        // Should have ignore patterns from file
        assert_eq!(config.security.ignore_secret_patterns.len(), 2);
        assert!(
            config
                .security
                .ignore_secret_patterns
                .contains(&"github_pat".to_string())
        );
        assert!(
            config
                .security
                .ignore_secret_patterns
                .contains(&"aws_access_key".to_string())
        );

        // Source attribution should be config file
        assert!(matches!(
            config.source_attribution.get("security"),
            Some(ConfigSource::Config)
        ));
    }

    #[test]
    fn test_security_config_empty_section() {
        let _guard = config_env_guard();
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();

        let config_path = create_test_config_file(
            temp_dir.path(),
            r#"
[security]
"#,
        );

        let cli_args = CliArgs {
            config_path: Some(config_path),
            ..Default::default()
        };
        let config = Config::discover(&cli_args).unwrap();

        // Empty security section should use defaults
        assert!(config.security.extra_secret_patterns.is_empty());
        assert!(config.security.ignore_secret_patterns.is_empty());
    }

    #[test]
    fn test_security_config_builder_methods() {
        let config = Config::builder()
            .extra_secret_patterns(vec!["PATTERN_A".to_string()])
            .add_extra_secret_pattern("PATTERN_B")
            .ignore_secret_patterns(vec!["ignore_a".to_string()])
            .add_ignore_secret_pattern("ignore_b")
            .build()
            .unwrap();

        // Should have both extra patterns
        assert_eq!(config.security.extra_secret_patterns.len(), 2);
        assert!(
            config
                .security
                .extra_secret_patterns
                .contains(&"PATTERN_A".to_string())
        );
        assert!(
            config
                .security
                .extra_secret_patterns
                .contains(&"PATTERN_B".to_string())
        );

        // Should have both ignore patterns
        assert_eq!(config.security.ignore_secret_patterns.len(), 2);
        assert!(
            config
                .security
                .ignore_secret_patterns
                .contains(&"ignore_a".to_string())
        );
        assert!(
            config
                .security
                .ignore_secret_patterns
                .contains(&"ignore_b".to_string())
        );
    }
}

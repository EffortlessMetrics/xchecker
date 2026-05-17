use crate::error::{ConfigError, XCheckerError};

use super::{Config, PromptTemplate};

impl Config {
    /// Validate configuration values
    pub(crate) fn validate(&self) -> Result<(), XCheckerError> {
        // Validate packet limits
        if let Some(max_bytes) = self.defaults.packet_max_bytes {
            if max_bytes == 0 {
                return Err(XCheckerError::Config(ConfigError::InvalidValue {
                    key: "packet_max_bytes".to_string(),
                    value: "must be greater than 0".to_string(),
                }));
            }
            if max_bytes > 10_000_000 {
                // 10MB limit
                return Err(XCheckerError::Config(ConfigError::InvalidValue {
                    key: "packet_max_bytes".to_string(),
                    value: "exceeds maximum limit of 10MB".to_string(),
                }));
            }
        }

        if let Some(max_lines) = self.defaults.packet_max_lines {
            if max_lines == 0 {
                return Err(XCheckerError::Config(ConfigError::InvalidValue {
                    key: "packet_max_lines".to_string(),
                    value: "must be greater than 0".to_string(),
                }));
            }
            if max_lines > 100_000 {
                return Err(XCheckerError::Config(ConfigError::InvalidValue {
                    key: "packet_max_lines".to_string(),
                    value: "exceeds maximum limit of 100,000".to_string(),
                }));
            }
        }

        // Validate max_turns
        if let Some(max_turns) = self.defaults.max_turns {
            if max_turns == 0 {
                return Err(XCheckerError::Config(ConfigError::InvalidValue {
                    key: "max_turns".to_string(),
                    value: "must be greater than 0".to_string(),
                }));
            }
            if max_turns > 50 {
                return Err(XCheckerError::Config(ConfigError::InvalidValue {
                    key: "max_turns".to_string(),
                    value: "exceeds maximum limit of 50".to_string(),
                }));
            }
        }

        // Validate phase_timeout
        if let Some(phase_timeout) = self.defaults.phase_timeout {
            if phase_timeout < 5 {
                return Err(XCheckerError::Config(ConfigError::InvalidValue {
                    key: "phase_timeout".to_string(),
                    value: "must be at least 5 seconds".to_string(),
                }));
            }
            if phase_timeout > 7200 {
                return Err(XCheckerError::Config(ConfigError::InvalidValue {
                    key: "phase_timeout".to_string(),
                    value: "exceeds maximum limit of 7200 seconds (2 hours)".to_string(),
                }));
            }
        }

        // Validate stdout_cap_bytes
        if let Some(stdout_cap) = self.defaults.stdout_cap_bytes {
            if stdout_cap < 1024 {
                return Err(XCheckerError::Config(ConfigError::InvalidValue {
                    key: "stdout_cap_bytes".to_string(),
                    value: "must be at least 1024 bytes (1 KiB)".to_string(),
                }));
            }
            if stdout_cap > 100_000_000 {
                return Err(XCheckerError::Config(ConfigError::InvalidValue {
                    key: "stdout_cap_bytes".to_string(),
                    value: "exceeds maximum limit of 100MB".to_string(),
                }));
            }
        }

        // Validate stderr_cap_bytes
        if let Some(stderr_cap) = self.defaults.stderr_cap_bytes {
            if stderr_cap < 1024 {
                return Err(XCheckerError::Config(ConfigError::InvalidValue {
                    key: "stderr_cap_bytes".to_string(),
                    value: "must be at least 1024 bytes (1 KiB)".to_string(),
                }));
            }
            if stderr_cap > 10_000_000 {
                return Err(XCheckerError::Config(ConfigError::InvalidValue {
                    key: "stderr_cap_bytes".to_string(),
                    value: "exceeds maximum limit of 10MB".to_string(),
                }));
            }
        }

        // Validate lock_ttl_seconds
        if let Some(lock_ttl) = self.defaults.lock_ttl_seconds {
            if lock_ttl < 60 {
                return Err(XCheckerError::Config(ConfigError::InvalidValue {
                    key: "lock_ttl_seconds".to_string(),
                    value: "must be at least 60 seconds (1 minute)".to_string(),
                }));
            }
            if lock_ttl > 86400 {
                return Err(XCheckerError::Config(ConfigError::InvalidValue {
                    key: "lock_ttl_seconds".to_string(),
                    value: "exceeds maximum limit of 86400 seconds (24 hours)".to_string(),
                }));
            }
        }

        // Validate output format
        if let Some(format) = &self.defaults.output_format {
            match format.as_str() {
                "stream-json" | "text" => {}
                _ => {
                    return Err(XCheckerError::Config(ConfigError::InvalidValue {
                        key: "output_format".to_string(),
                        value: format!("'{format}' is not valid. Must be 'stream-json' or 'text'"),
                    }));
                }
            }
        }

        // Validate runner mode
        if let Some(mode) = &self.runner.mode {
            match mode.as_str() {
                "auto" | "native" | "wsl" => {}
                _ => {
                    return Err(XCheckerError::Config(ConfigError::InvalidValue {
                        key: "runner_mode".to_string(),
                        value: format!("'{mode}' is not valid. Must be 'auto', 'native', or 'wsl'"),
                    }));
                }
            }
        }

        self.selectors.validate()?;

        // Validate LLM provider - supported providers in V14: claude-cli, gemini-cli, openrouter, anthropic
        let is_supported_provider = |provider: &str| {
            matches!(
                provider,
                "claude-cli" | "gemini-cli" | "openrouter" | "anthropic"
            )
        };

        if let Some(provider) = &self.llm.provider {
            if !is_supported_provider(provider.as_str()) {
                return Err(XCheckerError::Config(ConfigError::InvalidValue {
                    key: "llm.provider".to_string(),
                    value: format!(
                        "'{provider}' is not supported. Supported providers: claude-cli, gemini-cli, openrouter, anthropic"
                    ),
                }));
            }
        } else {
            // This should never happen due to default enforcement, but guard against it
            return Err(XCheckerError::Config(ConfigError::MissingRequired(
                "llm.provider is required (should default to 'claude-cli')".to_string(),
            )));
        }

        if let Some(fallback_provider) = &self.llm.fallback_provider
            && !is_supported_provider(fallback_provider.as_str())
        {
            return Err(XCheckerError::Config(ConfigError::InvalidValue {
                key: "llm.fallback_provider".to_string(),
                value: format!(
                    "'{fallback_provider}' is not supported. Supported providers: claude-cli, gemini-cli, openrouter, anthropic"
                ),
            }));
        }

        // Validate HTTP providers have required model configuration.
        // These providers require a model to be explicitly configured since they
        // don't have a safe default like CLI providers do.
        let provider = self.llm.provider.as_deref().unwrap_or("claude-cli");
        self.validate_http_provider_model(provider, false)?;

        // Also validate fallback provider model requirements
        if let Some(fallback_provider) = &self.llm.fallback_provider {
            self.validate_http_provider_model(fallback_provider, true)?;
        }

        // Validate execution strategy - must be "controlled" (V11-V14 requirement)
        if let Some(strategy) = &self.llm.execution_strategy {
            if strategy != "controlled" {
                return Err(XCheckerError::Config(ConfigError::InvalidValue {
                    key: "llm.execution_strategy".to_string(),
                    value: format!(
                        "'{strategy}' is not supported. V11-V14 only support 'controlled' execution strategy. Other strategies like 'externaltool' or 'external_tool' are reserved for future versions"
                    ),
                }));
            }
        } else {
            // This should never happen due to default enforcement, but guard against it
            return Err(XCheckerError::Config(ConfigError::MissingRequired(
                "llm.execution_strategy is required (should default to 'controlled')".to_string(),
            )));
        }

        // Validate prompt template compatibility with provider (Requirement 3.7.6)
        // If a phase is configured with a prompt template that is incompatible with
        // the selected provider, xchecker fails during configuration validation.
        // No "best effort" adaptation; explicit failure prevents silent misbehavior.
        let template = if let Some(template_name) = &self.llm.prompt_template {
            Some(PromptTemplate::parse(template_name).map_err(|e| {
                XCheckerError::Config(ConfigError::InvalidValue {
                    key: "llm.prompt_template".to_string(),
                    value: e,
                })
            })?)
        } else {
            None
        };

        if let Some(template) = template {
            // Get the provider (should always be set due to earlier validation)
            let provider = self.llm.provider.as_deref().unwrap_or("claude-cli");

            // Validate compatibility for primary provider
            template
                .validate_provider_compatibility(provider)
                .map_err(|e| {
                    XCheckerError::Config(ConfigError::InvalidValue {
                        key: "llm.prompt_template".to_string(),
                        value: e,
                    })
                })?;

            // Validate compatibility for fallback provider when configured
            if let Some(fallback_provider) = &self.llm.fallback_provider {
                template
                    .validate_provider_compatibility(fallback_provider)
                    .map_err(|e| {
                        XCheckerError::Config(ConfigError::InvalidValue {
                            key: "llm.prompt_template".to_string(),
                            value: e,
                        })
                    })?;
            }
        }

        Ok(())
    }

    /// Validate that HTTP providers (openrouter, anthropic) have a model configured.
    ///
    /// HTTP providers don't have safe defaults like CLI providers do, so a model
    /// must be explicitly configured.
    fn validate_http_provider_model(
        &self,
        provider: &str,
        is_fallback: bool,
    ) -> Result<(), XCheckerError> {
        let config_key = match provider {
            "openrouter" => {
                let has_model = self
                    .llm
                    .openrouter
                    .as_ref()
                    .and_then(|or| or.model.as_ref())
                    .is_some_and(|m| !m.is_empty());
                if has_model {
                    return Ok(());
                }
                "llm.openrouter.model"
            }
            "anthropic" => {
                let has_model = self
                    .llm
                    .anthropic
                    .as_ref()
                    .and_then(|a| a.model.as_ref())
                    .is_some_and(|m| !m.is_empty());
                if has_model {
                    return Ok(());
                }
                "llm.anthropic.model"
            }
            // CLI providers don't require explicit model configuration
            _ => return Ok(()),
        };

        let context = if is_fallback {
            "Fallback provider"
        } else {
            "Provider"
        };
        Err(XCheckerError::Config(ConfigError::InvalidValue {
            key: config_key.to_string(),
            value: format!(
                "{context} '{provider}' requires a model to be configured. \
                 Please set [llm.{provider}] model = \"model-name\".",
                provider = provider.to_lowercase()
            ),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AnthropicConfig, Config, OpenRouterConfig};

    fn valid_config() -> Config {
        let mut config = Config::minimal_for_testing();
        config.llm.provider = Some("claude-cli".to_string());
        config.llm.execution_strategy = Some("controlled".to_string());
        config
    }

    fn invalid_key(result: Result<(), XCheckerError>) -> String {
        match result.expect_err("configuration should be invalid") {
            XCheckerError::Config(ConfigError::InvalidValue { key, .. }) => key,
            XCheckerError::Config(ConfigError::MissingRequired(key)) => key,
            other => panic!("expected config validation error, got {other:?}"),
        }
    }

    #[test]
    fn validation_accepts_documented_numeric_boundaries() {
        let mut config = valid_config();
        config.defaults.packet_max_bytes = Some(1);
        config.defaults.packet_max_lines = Some(1);
        config.defaults.max_turns = Some(1);
        config.defaults.phase_timeout = Some(5);
        config.defaults.stdout_cap_bytes = Some(1024);
        config.defaults.stderr_cap_bytes = Some(1024);
        config.defaults.lock_ttl_seconds = Some(60);
        config.validate().unwrap();

        config.defaults.packet_max_bytes = Some(10_000_000);
        config.defaults.packet_max_lines = Some(100_000);
        config.defaults.max_turns = Some(50);
        config.defaults.phase_timeout = Some(7200);
        config.defaults.stdout_cap_bytes = Some(100_000_000);
        config.defaults.stderr_cap_bytes = Some(10_000_000);
        config.defaults.lock_ttl_seconds = Some(86_400);
        config.validate().unwrap();
    }

    #[test]
    fn validation_rejects_numeric_values_outside_documented_boundaries() {
        macro_rules! assert_invalid_default {
            ($expected_key:literal, $field:ident, $value:expr) => {{
                let mut config = valid_config();
                config.defaults.$field = Some($value);
                assert_eq!(invalid_key(config.validate()), $expected_key);
            }};
        }

        assert_invalid_default!("packet_max_bytes", packet_max_bytes, 0);
        assert_invalid_default!("packet_max_bytes", packet_max_bytes, 10_000_001);
        assert_invalid_default!("packet_max_lines", packet_max_lines, 0);
        assert_invalid_default!("packet_max_lines", packet_max_lines, 100_001);
        assert_invalid_default!("max_turns", max_turns, 0);
        assert_invalid_default!("max_turns", max_turns, 51);
        assert_invalid_default!("phase_timeout", phase_timeout, 4);
        assert_invalid_default!("phase_timeout", phase_timeout, 7201);
        assert_invalid_default!("stdout_cap_bytes", stdout_cap_bytes, 1023);
        assert_invalid_default!("stdout_cap_bytes", stdout_cap_bytes, 100_000_001);
        assert_invalid_default!("stderr_cap_bytes", stderr_cap_bytes, 1023);
        assert_invalid_default!("stderr_cap_bytes", stderr_cap_bytes, 10_000_001);
        assert_invalid_default!("lock_ttl_seconds", lock_ttl_seconds, 59);
        assert_invalid_default!("lock_ttl_seconds", lock_ttl_seconds, 86_401);
    }

    #[test]
    fn validation_requires_models_for_http_primary_providers() {
        let mut openrouter = valid_config();
        openrouter.llm.provider = Some("openrouter".to_string());
        assert_eq!(invalid_key(openrouter.validate()), "llm.openrouter.model");

        openrouter.llm.openrouter = Some(OpenRouterConfig {
            api_key_env: None,
            base_url: None,
            model: Some(String::new()),
            max_tokens: None,
            temperature: None,
            budget: None,
        });
        assert_eq!(invalid_key(openrouter.validate()), "llm.openrouter.model");

        openrouter.llm.openrouter.as_mut().unwrap().model = Some("openai/gpt-5.1".to_string());
        openrouter.validate().unwrap();

        let mut anthropic = valid_config();
        anthropic.llm.provider = Some("anthropic".to_string());
        assert_eq!(invalid_key(anthropic.validate()), "llm.anthropic.model");

        anthropic.llm.anthropic = Some(AnthropicConfig {
            api_key_env: None,
            base_url: None,
            model: Some("claude-sonnet-4-5".to_string()),
            max_tokens: None,
            temperature: None,
        });
        anthropic.validate().unwrap();
    }

    #[test]
    fn validation_requires_models_for_http_fallback_providers() {
        let mut config = valid_config();
        config.llm.fallback_provider = Some("anthropic".to_string());
        assert_eq!(invalid_key(config.validate()), "llm.anthropic.model");

        config.llm.anthropic = Some(AnthropicConfig {
            api_key_env: None,
            base_url: None,
            model: Some("claude-haiku-4-5".to_string()),
            max_tokens: None,
            temperature: None,
        });
        config.validate().unwrap();
    }

    #[test]
    fn validation_checks_prompt_template_against_fallback_provider() {
        let mut config = valid_config();
        config.llm.prompt_template = Some("claude-optimized".to_string());
        config.llm.fallback_provider = Some("gemini-cli".to_string());

        assert_eq!(invalid_key(config.validate()), "llm.prompt_template");
    }
}

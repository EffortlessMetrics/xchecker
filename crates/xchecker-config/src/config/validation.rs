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

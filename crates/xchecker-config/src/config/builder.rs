use std::collections::HashMap;
use std::path::PathBuf;

use crate::error::XCheckerError;

use super::{
    Config, ConfigSource, Defaults, HooksConfig, LlmConfig, PhasesConfig, RunnerConfig,
    SecurityConfig, Selectors,
};

impl Config {
    /// Create a builder for programmatic configuration.
    ///
    /// Use this when you need to configure xchecker programmatically without
    /// relying on environment variables or config files. This is the recommended
    /// approach for embedding xchecker in other applications.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use xchecker_config::Config;
    /// use std::time::Duration;
    ///
    /// let config = Config::builder()
    ///     .state_dir("/custom/path")
    ///     .packet_max_bytes(32768)
    ///     .packet_max_lines(600)
    ///     .phase_timeout(Duration::from_secs(300))
    ///     .build()
    ///     .expect("Failed to build config");
    /// ```
    #[must_use]
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder::new()
    }
}

/// Builder for programmatic configuration of xchecker.
///
/// `ConfigBuilder` provides a fluent API for constructing `Config` instances
/// without relying on environment variables or config files. This is useful
/// for embedding xchecker in other applications where deterministic behavior
/// is required.
///
/// # Example
///
/// ```rust,no_run
/// use xchecker_config::Config;
/// use std::time::Duration;
///
/// let config = Config::builder()
///     .state_dir("/custom/state")
///     .packet_max_bytes(65536)
///     .packet_max_lines(1200)
///     .phase_timeout(Duration::from_secs(600))
///     .runner_mode("native")
///     .build()
///     .expect("Failed to build config");
/// ```
///
/// # Source Attribution
///
/// All values set via the builder are attributed to `ConfigSource::Programmatic`
/// in the resulting `Config`'s source attribution map.
#[derive(Debug, Clone)]
pub struct ConfigBuilder {
    state_dir: Option<PathBuf>,
    packet_max_bytes: Option<usize>,
    packet_max_lines: Option<usize>,
    phase_timeout: Option<std::time::Duration>,
    runner_mode: Option<String>,
    model: Option<String>,
    max_turns: Option<u32>,
    verbose: Option<bool>,
    llm_provider: Option<String>,
    execution_strategy: Option<String>,
    extra_secret_patterns: Vec<String>,
    ignore_secret_patterns: Vec<String>,
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigBuilder {
    /// Create a new `ConfigBuilder` with no values set.
    ///
    /// All configuration values will use their defaults unless explicitly set.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state_dir: None,
            packet_max_bytes: None,
            packet_max_lines: None,
            phase_timeout: None,
            runner_mode: None,
            model: None,
            max_turns: None,
            verbose: None,
            llm_provider: None,
            execution_strategy: None,
            extra_secret_patterns: Vec::new(),
            ignore_secret_patterns: Vec::new(),
        }
    }

    /// Set the state directory for xchecker operations.
    ///
    /// This overrides the default state directory discovery (XCHECKER_HOME,
    /// upward search for `.xchecker/`).
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the state directory
    #[must_use]
    pub fn state_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.state_dir = Some(path.into());
        self
    }

    /// Set the maximum packet size in bytes.
    ///
    /// This limits the size of context packets sent to the LLM.
    /// Default: 65536 bytes.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Maximum packet size in bytes (must be > 0 and <= 10MB)
    #[must_use]
    pub fn packet_max_bytes(mut self, bytes: usize) -> Self {
        self.packet_max_bytes = Some(bytes);
        self
    }

    /// Set the maximum packet size in lines.
    ///
    /// This limits the number of lines in context packets sent to the LLM.
    /// Default: 1200 lines.
    ///
    /// # Arguments
    ///
    /// * `lines` - Maximum packet size in lines (must be > 0 and <= 100,000)
    #[must_use]
    pub fn packet_max_lines(mut self, lines: usize) -> Self {
        self.packet_max_lines = Some(lines);
        self
    }

    /// Set the phase execution timeout.
    ///
    /// This limits how long a single phase can run before timing out.
    /// Default: 600 seconds (10 minutes).
    ///
    /// # Arguments
    ///
    /// * `timeout` - Phase timeout duration (must be >= 5 seconds and <= 2 hours)
    #[must_use]
    pub fn phase_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.phase_timeout = Some(timeout);
        self
    }

    /// Set the runner mode for process execution.
    ///
    /// Valid values: "auto", "native", "wsl"
    /// Default: "auto"
    ///
    /// # Arguments
    ///
    /// * `mode` - Runner mode string
    #[must_use]
    pub fn runner_mode(mut self, mode: impl Into<String>) -> Self {
        self.runner_mode = Some(mode.into());
        self
    }

    /// Set the model to use for LLM operations.
    ///
    /// Valid values: "haiku", "sonnet", "opus", or specific model versions.
    /// Default: "haiku" (for testing/development)
    ///
    /// # Arguments
    ///
    /// * `model` - Model name or alias
    #[must_use]
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Set the maximum number of turns for LLM interactions.
    ///
    /// Default: 6 turns.
    ///
    /// # Arguments
    ///
    /// * `turns` - Maximum number of turns (must be > 0 and <= 50)
    #[must_use]
    pub fn max_turns(mut self, turns: u32) -> Self {
        self.max_turns = Some(turns);
        self
    }

    /// Set verbose output mode.
    ///
    /// Default: false
    ///
    /// # Arguments
    ///
    /// * `verbose` - Whether to enable verbose output
    #[must_use]
    pub fn verbose(mut self, verbose: bool) -> Self {
        self.verbose = Some(verbose);
        self
    }

    /// Set the LLM provider.
    ///
    /// Valid values: "claude-cli", "gemini-cli", "openrouter", "anthropic"
    /// Default: "claude-cli"
    ///
    /// # Arguments
    ///
    /// * `provider` - LLM provider name
    #[must_use]
    pub fn llm_provider(mut self, provider: impl Into<String>) -> Self {
        self.llm_provider = Some(provider.into());
        self
    }

    /// Set the execution strategy.
    ///
    /// Currently only "controlled" is supported.
    /// Default: "controlled"
    ///
    /// # Arguments
    ///
    /// * `strategy` - Execution strategy name
    #[must_use]
    pub fn execution_strategy(mut self, strategy: impl Into<String>) -> Self {
        self.execution_strategy = Some(strategy.into());
        self
    }

    /// Add extra secret patterns for detection.
    ///
    /// These patterns are added to the built-in patterns and will cause
    /// secret detection to trigger if matched.
    ///
    /// # Arguments
    ///
    /// * `patterns` - Vector of regex patterns to add
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use xchecker_config::Config;
    ///
    /// let config = Config::builder()
    ///     .extra_secret_patterns(vec![
    ///         "SECRET_[A-Z0-9]{32}".to_string(),
    ///         "API_KEY_[A-Za-z0-9]{40}".to_string(),
    ///     ])
    ///     .build()
    ///     .expect("Failed to build config");
    /// ```
    #[must_use]
    pub fn extra_secret_patterns(mut self, patterns: Vec<String>) -> Self {
        self.extra_secret_patterns = patterns;
        self
    }

    /// Add a single extra secret pattern for detection.
    ///
    /// This pattern is added to the built-in patterns and will cause
    /// secret detection to trigger if matched.
    ///
    /// # Arguments
    ///
    /// * `pattern` - Regex pattern to add
    #[must_use]
    pub fn add_extra_secret_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.extra_secret_patterns.push(pattern.into());
        self
    }

    /// Set patterns to ignore during secret detection.
    ///
    /// Pattern IDs listed here will be ignored during secret scanning.
    /// Use this to suppress false positives for known-safe patterns.
    ///
    /// **Warning:** Suppressing patterns reduces security. Only suppress
    /// patterns if you're certain they won't match real secrets.
    ///
    /// # Arguments
    ///
    /// * `patterns` - Vector of pattern IDs to ignore
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use xchecker_config::Config;
    ///
    /// let config = Config::builder()
    ///     .ignore_secret_patterns(vec!["test_token".to_string()])
    ///     .build()
    ///     .expect("Failed to build config");
    /// ```
    #[must_use]
    pub fn ignore_secret_patterns(mut self, patterns: Vec<String>) -> Self {
        self.ignore_secret_patterns = patterns;
        self
    }

    /// Add a single pattern to ignore during secret detection.
    ///
    /// **Warning:** Suppressing patterns reduces security. Only suppress
    /// patterns if you're certain they won't match real secrets.
    ///
    /// # Arguments
    ///
    /// * `pattern` - Pattern ID to ignore
    #[must_use]
    pub fn add_ignore_secret_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.ignore_secret_patterns.push(pattern.into());
        self
    }

    /// Build the `Config` from the builder values.
    ///
    /// This creates a `Config` using the values set on the builder, with
    /// defaults applied for any unset values. The resulting config is
    /// validated before being returned.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Any configuration value is invalid (e.g., packet_max_bytes = 0)
    /// - Validation fails for the resulting configuration
    ///
    /// # Returns
    ///
    /// A fully configured and validated `Config` instance.
    pub fn build(self) -> Result<Config, XCheckerError> {
        let mut source_attribution = HashMap::new();

        // Start with defaults
        let mut defaults = Defaults::default();
        let selectors = Selectors::default();
        let mut runner = RunnerConfig::default();
        let mut llm = LlmConfig {
            provider: None,
            fallback_provider: None,
            claude: None,
            gemini: None,
            openrouter: None,
            anthropic: None,
            execution_strategy: None,
            prompt_template: None,
        };
        let phases = PhasesConfig::default();
        let hooks = HooksConfig::default();

        // Track default sources before any programmatic overrides.
        for key in [
            "max_turns",
            "packet_max_bytes",
            "packet_max_lines",
            "output_format",
            "verbose",
            "runner_mode",
            "phase_timeout",
            "stdout_cap_bytes",
            "stderr_cap_bytes",
            "lock_ttl_seconds",
            "debug_packet",
            "allow_links",
            "llm_provider",
            "execution_strategy",
        ] {
            source_attribution.insert(key.to_string(), ConfigSource::Default);
        }

        // Seed LLM defaults before applying overrides.
        llm.provider = Some("claude-cli".to_string());
        llm.execution_strategy = Some("controlled".to_string());

        // Apply builder values (all attributed to Programmatic source).
        if let Some(bytes) = self.packet_max_bytes {
            defaults.packet_max_bytes = Some(bytes);
            source_attribution.insert("packet_max_bytes".to_string(), ConfigSource::Programmatic);
        }

        if let Some(lines) = self.packet_max_lines {
            defaults.packet_max_lines = Some(lines);
            source_attribution.insert("packet_max_lines".to_string(), ConfigSource::Programmatic);
        }

        if let Some(timeout) = self.phase_timeout {
            defaults.phase_timeout = Some(timeout.as_secs());
            source_attribution.insert("phase_timeout".to_string(), ConfigSource::Programmatic);
        }

        if let Some(mode) = self.runner_mode {
            runner.mode = Some(mode);
            source_attribution.insert("runner_mode".to_string(), ConfigSource::Programmatic);
        }

        if let Some(model) = self.model {
            defaults.model = Some(model);
            source_attribution.insert("model".to_string(), ConfigSource::Programmatic);
        }

        if let Some(turns) = self.max_turns {
            defaults.max_turns = Some(turns);
            source_attribution.insert("max_turns".to_string(), ConfigSource::Programmatic);
        }

        if let Some(verbose) = self.verbose {
            defaults.verbose = Some(verbose);
            source_attribution.insert("verbose".to_string(), ConfigSource::Programmatic);
        }

        // Apply LLM provider.
        if let Some(provider) = self.llm_provider {
            llm.provider = Some(provider);
            source_attribution.insert("llm_provider".to_string(), ConfigSource::Programmatic);
        }

        // Apply execution strategy.
        if let Some(strategy) = self.execution_strategy {
            llm.execution_strategy = Some(strategy);
            source_attribution.insert("execution_strategy".to_string(), ConfigSource::Programmatic);
        }

        // Note: state_dir is stored but not directly used in Config struct
        // It would be used by OrchestratorHandle when creating the orchestrator
        // For now, we store it in a way that can be retrieved if needed
        if self.state_dir.is_some() {
            source_attribution.insert("state_dir".to_string(), ConfigSource::Programmatic);
        }

        // Build security config from builder values
        let security = SecurityConfig {
            extra_secret_patterns: self.extra_secret_patterns,
            ignore_secret_patterns: self.ignore_secret_patterns,
        };
        if !security.extra_secret_patterns.is_empty() || !security.ignore_secret_patterns.is_empty()
        {
            source_attribution.insert("security".to_string(), ConfigSource::Programmatic);
        }

        let config = Config {
            defaults,
            selectors,
            runner,
            llm,
            phases,
            hooks,
            security,
            source_attribution,
        };

        // Validate the configuration
        config.validate()?;

        Ok(config)
    }
}

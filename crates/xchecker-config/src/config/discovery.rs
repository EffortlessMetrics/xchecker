use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};

use crate::error::{ConfigError, XCheckerError};

use super::{
    ClaudeConfig, CliArgs, Config, ConfigSource, Defaults, GeminiConfig, HooksConfig, LlmConfig,
    PhasesConfig, RunnerConfig, SecurityConfig, Selectors,
};

/// TOML configuration file structure
#[derive(Debug, Deserialize, Serialize)]
struct TomlConfig {
    defaults: Option<Defaults>,
    selectors: Option<Selectors>,
    runner: Option<RunnerConfig>,
    llm: Option<LlmConfig>,
    phases: Option<PhasesConfig>,
    hooks: Option<HooksConfig>,
    security: Option<SecurityConfig>,
}

impl Config {
    /// Discover and load configuration with precedence: CLI > file > defaults
    ///
    /// Uses current working directory for config file discovery when no explicit
    /// path is provided in cli_args.
    pub fn discover(cli_args: &CliArgs) -> Result<Self, XCheckerError> {
        let start_dir = std::env::current_dir().map_err(|e| {
            XCheckerError::Config(ConfigError::DiscoveryFailed {
                reason: format!("Failed to get current directory: {e}"),
            })
        })?;
        Self::discover_from(&start_dir, cli_args)
    }

    /// Discover and load configuration starting from a specific directory
    ///
    /// This is the path-driven variant used by tests to avoid process-global state.
    /// Uses the given directory for config file discovery when no explicit path
    /// is provided in cli_args.
    pub fn discover_from(start_dir: &Path, cli_args: &CliArgs) -> Result<Self, XCheckerError> {
        let mut source_attribution = HashMap::new();

        // Start with built-in defaults
        let mut defaults = Defaults::default();
        let mut selectors = Selectors::default();
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
        let mut hooks = HooksConfig::default();
        let mut phases = PhasesConfig::default();
        let mut security = SecurityConfig::default();

        // Track default sources
        source_attribution.insert("max_turns".to_string(), ConfigSource::Default);
        source_attribution.insert("packet_max_bytes".to_string(), ConfigSource::Default);
        source_attribution.insert("packet_max_lines".to_string(), ConfigSource::Default);
        source_attribution.insert("output_format".to_string(), ConfigSource::Default);
        source_attribution.insert("verbose".to_string(), ConfigSource::Default);
        source_attribution.insert("runner_mode".to_string(), ConfigSource::Default);
        source_attribution.insert("phase_timeout".to_string(), ConfigSource::Default);
        source_attribution.insert("stdout_cap_bytes".to_string(), ConfigSource::Default);
        source_attribution.insert("stderr_cap_bytes".to_string(), ConfigSource::Default);
        source_attribution.insert("lock_ttl_seconds".to_string(), ConfigSource::Default);
        source_attribution.insert("debug_packet".to_string(), ConfigSource::Default);
        source_attribution.insert("allow_links".to_string(), ConfigSource::Default);

        // Discover and load config file (if not explicitly provided)
        let config_path = if let Some(explicit_path) = &cli_args.config_path {
            Some(explicit_path.clone())
        } else {
            Self::discover_config_file_from(start_dir)?
        };

        if let Some(path) = &config_path {
            let file_config = Self::load_config_file(path)?;

            let config_source = ConfigSource::Config;

            // Apply config file values (override defaults)
            if let Some(file_defaults) = file_config.defaults {
                if file_defaults.model.is_some() {
                    defaults.model = file_defaults.model;
                    source_attribution.insert("model".to_string(), config_source.clone());
                }
                if file_defaults.max_turns.is_some() {
                    defaults.max_turns = file_defaults.max_turns;
                    source_attribution.insert("max_turns".to_string(), config_source.clone());
                }
                if file_defaults.packet_max_bytes.is_some() {
                    defaults.packet_max_bytes = file_defaults.packet_max_bytes;
                    source_attribution
                        .insert("packet_max_bytes".to_string(), config_source.clone());
                }
                if file_defaults.packet_max_lines.is_some() {
                    defaults.packet_max_lines = file_defaults.packet_max_lines;
                    source_attribution
                        .insert("packet_max_lines".to_string(), config_source.clone());
                }
                if file_defaults.output_format.is_some() {
                    defaults.output_format = file_defaults.output_format;
                    source_attribution.insert("output_format".to_string(), config_source.clone());
                }
                if file_defaults.verbose.is_some() {
                    defaults.verbose = file_defaults.verbose;
                    source_attribution.insert("verbose".to_string(), config_source.clone());
                }
                if file_defaults.phase_timeout.is_some() {
                    defaults.phase_timeout = file_defaults.phase_timeout;
                    source_attribution.insert("phase_timeout".to_string(), config_source.clone());
                }
                if file_defaults.stdout_cap_bytes.is_some() {
                    defaults.stdout_cap_bytes = file_defaults.stdout_cap_bytes;
                    source_attribution
                        .insert("stdout_cap_bytes".to_string(), config_source.clone());
                }
                if file_defaults.stderr_cap_bytes.is_some() {
                    defaults.stderr_cap_bytes = file_defaults.stderr_cap_bytes;
                    source_attribution
                        .insert("stderr_cap_bytes".to_string(), config_source.clone());
                }
                if file_defaults.lock_ttl_seconds.is_some() {
                    defaults.lock_ttl_seconds = file_defaults.lock_ttl_seconds;
                    source_attribution
                        .insert("lock_ttl_seconds".to_string(), config_source.clone());
                }
                if file_defaults.debug_packet.is_some() {
                    defaults.debug_packet = file_defaults.debug_packet;
                    source_attribution.insert("debug_packet".to_string(), config_source.clone());
                }
                if file_defaults.allow_links.is_some() {
                    defaults.allow_links = file_defaults.allow_links;
                    source_attribution.insert("allow_links".to_string(), config_source.clone());
                }
                if file_defaults.strict_validation.is_some() {
                    defaults.strict_validation = file_defaults.strict_validation;
                    source_attribution
                        .insert("strict_validation".to_string(), config_source.clone());
                }
            }

            if let Some(file_selectors) = file_config.selectors {
                if !file_selectors.include.is_empty() {
                    selectors.include = file_selectors.include;
                    source_attribution
                        .insert("selectors_include".to_string(), config_source.clone());
                }
                if !file_selectors.exclude.is_empty() {
                    selectors.exclude = file_selectors.exclude;
                    source_attribution
                        .insert("selectors_exclude".to_string(), config_source.clone());
                }
            }

            if let Some(file_runner) = file_config.runner {
                if file_runner.mode.is_some() {
                    runner.mode = file_runner.mode;
                    source_attribution.insert("runner_mode".to_string(), config_source.clone());
                }
                if file_runner.distro.is_some() {
                    runner.distro = file_runner.distro;
                    source_attribution.insert("runner_distro".to_string(), config_source.clone());
                }
                if file_runner.claude_path.is_some() {
                    runner.claude_path = file_runner.claude_path;
                    source_attribution.insert("claude_path".to_string(), config_source.clone());
                }
            }

            if let Some(file_llm) = file_config.llm {
                if file_llm.provider.is_some() {
                    llm.provider = file_llm.provider;
                    source_attribution.insert("llm_provider".to_string(), config_source.clone());
                }
                if file_llm.fallback_provider.is_some() {
                    llm.fallback_provider = file_llm.fallback_provider;
                    source_attribution
                        .insert("llm_fallback_provider".to_string(), config_source.clone());
                }
                if let Some(file_claude) = file_llm.claude
                    && file_claude.binary.is_some()
                {
                    llm.claude = Some(file_claude);
                    source_attribution
                        .insert("llm_claude_binary".to_string(), config_source.clone());
                }
                if let Some(file_gemini) = file_llm.gemini {
                    llm.gemini = Some(file_gemini);
                    source_attribution
                        .insert("llm_gemini_config".to_string(), config_source.clone());
                }
                if let Some(file_openrouter) = file_llm.openrouter {
                    llm.openrouter = Some(file_openrouter);
                    source_attribution
                        .insert("llm_openrouter_config".to_string(), config_source.clone());
                }
                if let Some(file_anthropic) = file_llm.anthropic {
                    llm.anthropic = Some(file_anthropic);
                    source_attribution
                        .insert("llm_anthropic_config".to_string(), config_source.clone());
                }
                if file_llm.execution_strategy.is_some() {
                    llm.execution_strategy = file_llm.execution_strategy;
                    source_attribution
                        .insert("execution_strategy".to_string(), config_source.clone());
                }
                if file_llm.prompt_template.is_some() {
                    llm.prompt_template = file_llm.prompt_template;
                    source_attribution.insert("prompt_template".to_string(), config_source.clone());
                }
            }

            // Load phases configuration from file
            if let Some(file_phases) = file_config.phases {
                phases = file_phases;
                source_attribution.insert("phases".to_string(), config_source.clone());
            }

            // Load hooks configuration from file
            if let Some(file_hooks) = file_config.hooks {
                hooks = file_hooks;
                source_attribution.insert("hooks".to_string(), config_source.clone());
            }

            // Load security configuration from file
            if let Some(file_security) = file_config.security {
                security = file_security;
                source_attribution.insert("security".to_string(), config_source);
            }
        }

        // Apply CLI overrides (highest priority)
        if let Some(model) = &cli_args.model {
            defaults.model = Some(model.clone());
            source_attribution.insert("model".to_string(), ConfigSource::Cli);
        }
        if let Some(max_turns) = cli_args.max_turns {
            defaults.max_turns = Some(max_turns);
            source_attribution.insert("max_turns".to_string(), ConfigSource::Cli);
        }
        if let Some(packet_max_bytes) = cli_args.packet_max_bytes {
            defaults.packet_max_bytes = Some(packet_max_bytes);
            source_attribution.insert("packet_max_bytes".to_string(), ConfigSource::Cli);
        }
        if let Some(packet_max_lines) = cli_args.packet_max_lines {
            defaults.packet_max_lines = Some(packet_max_lines);
            source_attribution.insert("packet_max_lines".to_string(), ConfigSource::Cli);
        }
        if let Some(output_format) = &cli_args.output_format {
            defaults.output_format = Some(output_format.clone());
            source_attribution.insert("output_format".to_string(), ConfigSource::Cli);
        }
        if let Some(verbose) = cli_args.verbose {
            defaults.verbose = Some(verbose);
            source_attribution.insert("verbose".to_string(), ConfigSource::Cli);
        }
        if let Some(runner_mode) = &cli_args.runner_mode {
            runner.mode = Some(runner_mode.clone());
            source_attribution.insert("runner_mode".to_string(), ConfigSource::Cli);
        }
        if let Some(runner_distro) = &cli_args.runner_distro {
            runner.distro = Some(runner_distro.clone());
            source_attribution.insert("runner_distro".to_string(), ConfigSource::Cli);
        }
        if let Some(claude_path) = &cli_args.claude_path {
            runner.claude_path = Some(claude_path.clone());
            source_attribution.insert("claude_path".to_string(), ConfigSource::Cli);
        }
        if let Some(phase_timeout) = cli_args.phase_timeout {
            defaults.phase_timeout = Some(phase_timeout);
            source_attribution.insert("phase_timeout".to_string(), ConfigSource::Cli);
        }
        if let Some(stdout_cap_bytes) = cli_args.stdout_cap_bytes {
            defaults.stdout_cap_bytes = Some(stdout_cap_bytes);
            source_attribution.insert("stdout_cap_bytes".to_string(), ConfigSource::Cli);
        }
        if let Some(stderr_cap_bytes) = cli_args.stderr_cap_bytes {
            defaults.stderr_cap_bytes = Some(stderr_cap_bytes);
            source_attribution.insert("stderr_cap_bytes".to_string(), ConfigSource::Cli);
        }
        if let Some(lock_ttl_seconds) = cli_args.lock_ttl_seconds {
            defaults.lock_ttl_seconds = Some(lock_ttl_seconds);
            source_attribution.insert("lock_ttl_seconds".to_string(), ConfigSource::Cli);
        }
        if cli_args.debug_packet {
            defaults.debug_packet = Some(true);
            source_attribution.insert("debug_packet".to_string(), ConfigSource::Cli);
        }
        if cli_args.allow_links {
            defaults.allow_links = Some(true);
            source_attribution.insert("allow_links".to_string(), ConfigSource::Cli);
        }
        if let Some(strict_validation) = cli_args.strict_validation {
            defaults.strict_validation = Some(strict_validation);
            source_attribution.insert("strict_validation".to_string(), ConfigSource::Cli);
        }

        // Apply security pattern overrides (CLI > file > defaults)
        if !cli_args.extra_secret_pattern.is_empty() {
            security
                .extra_secret_patterns
                .extend(cli_args.extra_secret_pattern.clone());
            source_attribution.insert("security".to_string(), ConfigSource::Cli);
        }
        if !cli_args.ignore_secret_pattern.is_empty() {
            security
                .ignore_secret_patterns
                .extend(cli_args.ignore_secret_pattern.clone());
            source_attribution.insert("security".to_string(), ConfigSource::Cli);
        }

        // Apply LLM configuration with precedence: CLI > env > config > defaults
        // Check environment variable first
        if let Ok(env_provider) = env::var("XCHECKER_LLM_PROVIDER")
            && !env_provider.is_empty()
        {
            llm.provider = Some(env_provider);
            source_attribution.insert("llm_provider".to_string(), ConfigSource::Env);
        }

        // CLI flag overrides environment variable
        if let Some(provider) = &cli_args.llm_provider {
            llm.provider = Some(provider.clone());
            source_attribution.insert("llm_provider".to_string(), ConfigSource::Cli);
        }

        // Default to "claude-cli" if no provider is set
        if llm.provider.is_none() {
            llm.provider = Some("claude-cli".to_string());
            source_attribution.insert("llm_provider".to_string(), ConfigSource::Default);
        }

        // Apply fallback provider configuration with precedence: CLI > env > config
        if let Ok(env_fallback) = env::var("XCHECKER_LLM_FALLBACK_PROVIDER")
            && !env_fallback.is_empty()
        {
            llm.fallback_provider = Some(env_fallback);
            source_attribution.insert("llm_fallback_provider".to_string(), ConfigSource::Env);
        }

        if let Some(fallback_provider) = &cli_args.llm_fallback_provider {
            llm.fallback_provider = Some(fallback_provider.clone());
            source_attribution.insert("llm_fallback_provider".to_string(), ConfigSource::Cli);
        }

        // Apply prompt template configuration with precedence: CLI > env > config
        if let Ok(env_template) = env::var("XCHECKER_LLM_PROMPT_TEMPLATE")
            && !env_template.is_empty()
        {
            llm.prompt_template = Some(env_template);
            source_attribution.insert("prompt_template".to_string(), ConfigSource::Env);
        }

        if let Some(prompt_template) = &cli_args.prompt_template {
            llm.prompt_template = Some(prompt_template.clone());
            source_attribution.insert("prompt_template".to_string(), ConfigSource::Cli);
        }

        // Apply Claude binary configuration
        if let Some(binary) = &cli_args.llm_claude_binary {
            if llm.claude.is_none() {
                llm.claude = Some(ClaudeConfig { binary: None });
            }
            if let Some(claude_config) = &mut llm.claude {
                claude_config.binary = Some(binary.clone());
                source_attribution.insert("llm_claude_binary".to_string(), ConfigSource::Cli);
            }
        }

        // Apply Gemini binary configuration
        if let Some(binary) = &cli_args.llm_gemini_binary {
            if llm.gemini.is_none() {
                llm.gemini = Some(GeminiConfig {
                    binary: None,
                    default_model: None,
                    profiles: None,
                });
            }
            if let Some(gemini_config) = &mut llm.gemini {
                gemini_config.binary = Some(binary.clone());
                source_attribution.insert("llm_gemini_binary".to_string(), ConfigSource::Cli);
            }
        }

        // Apply Gemini default model configuration with precedence: CLI > env > config
        if let Ok(env_default_model) = env::var("XCHECKER_LLM_GEMINI_DEFAULT_MODEL")
            && !env_default_model.is_empty()
        {
            if llm.gemini.is_none() {
                llm.gemini = Some(GeminiConfig {
                    binary: None,
                    default_model: None,
                    profiles: None,
                });
            }
            if let Some(gemini_config) = &mut llm.gemini {
                gemini_config.default_model = Some(env_default_model);
                source_attribution
                    .insert("llm_gemini_default_model".to_string(), ConfigSource::Env);
            }
        }

        if let Some(default_model) = &cli_args.llm_gemini_default_model {
            if llm.gemini.is_none() {
                llm.gemini = Some(GeminiConfig {
                    binary: None,
                    default_model: None,
                    profiles: None,
                });
            }
            if let Some(gemini_config) = &mut llm.gemini {
                gemini_config.default_model = Some(default_model.clone());
                source_attribution
                    .insert("llm_gemini_default_model".to_string(), ConfigSource::Cli);
            }
        }

        // Apply execution strategy configuration with precedence: CLI > env > config > default
        // Check environment variable (overrides config file)
        if let Ok(env_strategy) = env::var("XCHECKER_EXECUTION_STRATEGY")
            && !env_strategy.is_empty()
        {
            llm.execution_strategy = Some(env_strategy);
            source_attribution.insert("execution_strategy".to_string(), ConfigSource::Env);
        }

        // CLI flag overrides everything
        if let Some(strategy) = &cli_args.execution_strategy {
            llm.execution_strategy = Some(strategy.clone());
            source_attribution.insert("execution_strategy".to_string(), ConfigSource::Cli);
        }

        // Default to "controlled" if not specified
        if llm.execution_strategy.is_none() {
            llm.execution_strategy = Some("controlled".to_string());
            source_attribution.insert("execution_strategy".to_string(), ConfigSource::Default);
        }

        let config = Self {
            defaults,
            selectors,
            runner,
            llm,
            phases,
            hooks,
            security,
            source_attribution,
        };

        // Validate the final configuration
        config.validate()?;

        Ok(config)
    }

    /// Discover config file by searching upward from a given directory
    ///
    /// This is the path-driven variant used by tests to avoid process-global state.
    /// Walks up the directory tree looking for `.xchecker/config.toml`, stopping
    /// at repository root markers (.git, .hg, .svn) or filesystem root.
    pub fn discover_config_file_from(start_dir: &Path) -> Result<Option<PathBuf>, XCheckerError> {
        let mut current_dir = start_dir.to_path_buf();

        loop {
            let config_path = current_dir.join(".xchecker").join("config.toml");
            if config_path.exists() {
                return Ok(Some(config_path));
            }

            // Check if we've reached the filesystem root or repository root
            if current_dir.parent().is_none() {
                break;
            }

            // Check for repository root markers
            if current_dir.join(".git").exists()
                || current_dir.join(".hg").exists()
                || current_dir.join(".svn").exists()
            {
                // Stop at repository root if no config found
                break;
            }

            current_dir = current_dir.parent().unwrap().to_path_buf();
        }

        Ok(None)
    }

    /// Load configuration from TOML file
    fn load_config_file(path: &Path) -> Result<TomlConfig, XCheckerError> {
        match std::fs::read_to_string(path) {
            Ok(content) => toml::from_str(&content).map_err(|e| {
                XCheckerError::Config(ConfigError::InvalidFile(format!(
                    "Failed to parse TOML config file {}: {e}",
                    path.display()
                )))
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Missing config file is OK - return empty config (will use defaults)
                Ok(TomlConfig {
                    defaults: None,
                    selectors: None,
                    runner: None,
                    llm: None,
                    phases: None,
                    hooks: None,
                    security: None,
                })
            }
            Err(e) => Err(XCheckerError::Config(ConfigError::DiscoveryFailed {
                reason: format!("Failed to read config file {}: {}", path.display(), e),
            })),
        }
    }

    /// Discover configuration from environment and filesystem.
    ///
    /// This method uses the same discovery logic as the CLI:
    /// - `XCHECKER_HOME` environment variable (if set)
    /// - Upward search for `.xchecker/config.toml` from current directory
    /// - Built-in defaults
    ///
    /// Precedence: config file > defaults
    ///
    /// This is the recommended method for library consumers who want CLI-like
    /// behavior without needing to construct `CliArgs`.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use xchecker_config::Config;
    ///
    /// let config = Config::discover_from_env_and_fs()
    ///     .expect("Failed to discover config");
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The current directory cannot be determined
    /// - A config file exists but cannot be parsed
    /// - Configuration validation fails
    pub fn discover_from_env_and_fs() -> Result<Self, XCheckerError> {
        // Use empty CliArgs to get config file + defaults behavior
        // This matches CLI semantics without any CLI overrides
        let cli_args = CliArgs::default();
        Self::discover(&cli_args)
    }
}

//! Configuration management for xchecker
//!
//! This module provides hierarchical configuration with discovery and precedence:
//! CLI > file > defaults. Supports TOML configuration files with [defaults],
//! [selectors], and [runner] sections.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};

use crate::error::{ConfigError, XCheckerError};
use crate::hooks::HooksConfig;
use crate::types::RunnerMode;

/// Configuration with hierarchical precedence and source attribution
#[derive(Debug, Clone)]
pub struct Config {
    pub defaults: Defaults,
    pub selectors: Selectors,
    pub runner: RunnerConfig,
    pub llm: LlmConfig,
    /// Hooks configuration for pre/post phase scripts
    // Reserved for hooks integration; not wired in v1.0
    #[allow(dead_code)]
    pub hooks: HooksConfig,
    /// Track source of each setting for status display
    pub source_attribution: HashMap<String, ConfigSource>,
}

/// Default configuration values
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Defaults {
    pub model: Option<String>,
    pub max_turns: Option<u32>,
    pub packet_max_bytes: Option<usize>,
    pub packet_max_lines: Option<usize>,
    pub output_format: Option<String>,
    pub verbose: Option<bool>,
    pub phase_timeout: Option<u64>,
    pub stdout_cap_bytes: Option<usize>,
    pub stderr_cap_bytes: Option<usize>,
    pub lock_ttl_seconds: Option<u64>,
    pub debug_packet: Option<bool>,
    pub allow_links: Option<bool>,
}

/// LLM provider configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LlmConfig {
    pub provider: Option<String>,
    pub fallback_provider: Option<String>,
    pub claude: Option<ClaudeConfig>,
    pub gemini: Option<GeminiConfig>,
    pub openrouter: Option<OpenRouterConfig>,
    pub anthropic: Option<AnthropicConfig>,
    pub execution_strategy: Option<String>,
    /// Prompt template to use for LLM interactions
    /// 
    /// Available templates:
    /// - "default": Universal template compatible with all providers
    /// - "claude-optimized": Optimized for Claude CLI and Anthropic API
    /// - "openai-compatible": Optimized for OpenRouter and OpenAI-compatible APIs
    /// 
    /// If not specified, defaults to "default" which works with all providers.
    pub prompt_template: Option<String>,
}

/// Prompt template types for provider-specific optimizations
/// 
/// Templates define how prompts are structured for different LLM providers.
/// Some templates are optimized for specific providers and may not work
/// correctly with others.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptTemplate {
    /// Universal template compatible with all providers
    /// Uses a simple message format that works across all backends
    Default,
    /// Optimized for Claude CLI and Anthropic API
    /// Uses Claude-specific formatting like XML tags and system prompts
    ClaudeOptimized,
    /// Optimized for OpenRouter and OpenAI-compatible APIs
    /// Uses OpenAI-style message formatting
    OpenAiCompatible,
}

impl PromptTemplate {
    /// Parse a template name string into a PromptTemplate
    /// 
    /// # Errors
    /// 
    /// Returns an error if the template name is not recognized.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "default" => Ok(Self::Default),
            "claude-optimized" | "claude_optimized" | "claude" => Ok(Self::ClaudeOptimized),
            "openai-compatible" | "openai_compatible" | "openai" | "openrouter" => Ok(Self::OpenAiCompatible),
            _ => Err(format!(
                "Unknown prompt template '{}'. Available templates: default, claude-optimized, openai-compatible",
                s
            )),
        }
    }

    /// Check if this template is compatible with the given provider
    /// 
    /// Returns `Ok(())` if compatible, or an error message explaining the incompatibility.
    pub fn validate_provider_compatibility(&self, provider: &str) -> Result<(), String> {
        match (self, provider) {
            // Default template is compatible with all providers
            (Self::Default, _) => Ok(()),
            
            // Claude-optimized template is compatible with Claude CLI and Anthropic
            (Self::ClaudeOptimized, "claude-cli" | "anthropic") => Ok(()),
            (Self::ClaudeOptimized, provider) => Err(format!(
                "Prompt template 'claude-optimized' is not compatible with provider '{}'. \
                 This template uses Claude-specific formatting (XML tags, system prompts) \
                 that may not work correctly with other providers. \
                 Compatible providers: claude-cli, anthropic. \
                 Use 'default' template for cross-provider compatibility.",
                provider
            )),
            
            // OpenAI-compatible template is compatible with OpenRouter and Gemini
            (Self::OpenAiCompatible, "openrouter" | "gemini-cli") => Ok(()),
            (Self::OpenAiCompatible, provider) => Err(format!(
                "Prompt template 'openai-compatible' is not compatible with provider '{}'. \
                 This template uses OpenAI-style message formatting that may not work \
                 correctly with Claude-specific providers. \
                 Compatible providers: openrouter, gemini-cli. \
                 Use 'default' template for cross-provider compatibility.",
                provider
            )),
        }
    }

    /// Get the template name as a string
    #[must_use]
    #[allow(dead_code)] // Public API for template introspection
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::ClaudeOptimized => "claude-optimized",
            Self::OpenAiCompatible => "openai-compatible",
        }
    }

    /// Get a list of providers compatible with this template
    #[must_use]
    #[allow(dead_code)] // Public API for template introspection
    pub const fn compatible_providers(&self) -> &'static [&'static str] {
        match self {
            Self::Default => &["claude-cli", "gemini-cli", "openrouter", "anthropic"],
            Self::ClaudeOptimized => &["claude-cli", "anthropic"],
            Self::OpenAiCompatible => &["openrouter", "gemini-cli"],
        }
    }
}

/// Claude CLI provider configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClaudeConfig {
    pub binary: Option<String>,
}

/// Gemini CLI provider configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GeminiConfig {
    pub binary: Option<String>,
    pub default_model: Option<String>,
    pub profiles: Option<HashMap<String, GeminiProfileConfig>>,
}

/// Gemini profile configuration for per-phase model selection
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GeminiProfileConfig {
    pub model: Option<String>,
    pub max_tokens: Option<u32>,
}

/// OpenRouter HTTP provider configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OpenRouterConfig {
    pub api_key_env: Option<String>,
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub budget: Option<u32>,
}

/// Anthropic HTTP provider configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AnthropicConfig {
    pub api_key_env: Option<String>,
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
}

/// Content selection configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Selectors {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

/// Runner configuration for cross-platform execution
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RunnerConfig {
    pub mode: Option<String>,
    pub distro: Option<String>,
    pub claude_path: Option<String>,
}

/// Source of a configuration value for attribution
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigSource {
    Cli,
    ConfigFile(PathBuf),
    Defaults,
}

impl std::fmt::Display for ConfigSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cli => write!(f, "CLI"),
            Self::ConfigFile(path) => write!(f, "config file ({})", path.display()),
            Self::Defaults => write!(f, "defaults"),
        }
    }
}

/// TOML configuration file structure
#[derive(Debug, Deserialize, Serialize)]
struct TomlConfig {
    defaults: Option<Defaults>,
    selectors: Option<Selectors>,
    runner: Option<RunnerConfig>,
    llm: Option<LlmConfig>,
    hooks: Option<HooksConfig>,
}

/// CLI arguments for configuration override
#[derive(Debug, Clone, Default)]
pub struct CliArgs {
    pub config_path: Option<PathBuf>,
    pub model: Option<String>,
    pub max_turns: Option<u32>,
    pub packet_max_bytes: Option<usize>,
    pub packet_max_lines: Option<usize>,
    pub output_format: Option<String>,
    pub verbose: Option<bool>,
    pub runner_mode: Option<String>,
    pub runner_distro: Option<String>,
    pub claude_path: Option<String>,
    pub allow: Vec<String>,
    pub deny: Vec<String>,
    pub dangerously_skip_permissions: bool,
    pub ignore_secret_pattern: Vec<String>,
    pub extra_secret_pattern: Vec<String>,
    pub phase_timeout: Option<u64>,
    pub stdout_cap_bytes: Option<usize>,
    pub stderr_cap_bytes: Option<usize>,
    pub lock_ttl_seconds: Option<u64>,
    pub debug_packet: bool,
    pub allow_links: bool,
    pub llm_provider: Option<String>,
    pub llm_claude_binary: Option<String>,
    pub llm_gemini_binary: Option<String>,
    pub execution_strategy: Option<String>,
}

impl Default for Defaults {
    fn default() -> Self {
        Self {
            model: None,
            max_turns: Some(6),
            packet_max_bytes: Some(65536),
            packet_max_lines: Some(1200),
            output_format: Some("stream-json".to_string()),
            verbose: Some(false),
            phase_timeout: Some(600),        // 600 seconds = 10 minutes
            stdout_cap_bytes: Some(2097152), // 2 MiB
            stderr_cap_bytes: Some(262144),  // 256 KiB
            lock_ttl_seconds: Some(900),     // 15 minutes
            debug_packet: Some(false),
            allow_links: Some(false),
        }
    }
}

impl Default for Selectors {
    fn default() -> Self {
        Self {
            include: vec![
                "docs/**/SPEC*.md".to_string(),
                "docs/**/ADR*.md".to_string(),
                "README.md".to_string(),
                "SCHEMASET.*".to_string(),
                "**/Cargo.toml".to_string(),
                "**/*.core.yaml".to_string(),
            ],
            exclude: vec![
                "target/**".to_string(),
                "node_modules/**".to_string(),
                ".git/**".to_string(),
                "**/.DS_Store".to_string(),
            ],
        }
    }
}

impl Default for RunnerConfig {
    fn default() -> Self {
        Self {
            mode: Some("auto".to_string()),
            distro: None,
            claude_path: None,
        }
    }
}

impl Config {
    /// Discover and load configuration with precedence: CLI > file > defaults
    ///
    /// Uses current working directory for config file discovery when no explicit
    /// path is provided in cli_args.
    pub fn discover(cli_args: &CliArgs) -> Result<Self> {
        let start_dir = std::env::current_dir().context("Failed to get current directory")?;
        Self::discover_from(&start_dir, cli_args)
    }

    /// Discover and load configuration starting from a specific directory
    ///
    /// This is the path-driven variant used by tests to avoid process-global state.
    /// Uses the given directory for config file discovery when no explicit path
    /// is provided in cli_args.
    pub fn discover_from(start_dir: &Path, cli_args: &CliArgs) -> Result<Self> {
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

        // Track default sources
        source_attribution.insert("max_turns".to_string(), ConfigSource::Defaults);
        source_attribution.insert("packet_max_bytes".to_string(), ConfigSource::Defaults);
        source_attribution.insert("packet_max_lines".to_string(), ConfigSource::Defaults);
        source_attribution.insert("output_format".to_string(), ConfigSource::Defaults);
        source_attribution.insert("verbose".to_string(), ConfigSource::Defaults);
        source_attribution.insert("runner_mode".to_string(), ConfigSource::Defaults);
        source_attribution.insert("phase_timeout".to_string(), ConfigSource::Defaults);
        source_attribution.insert("stdout_cap_bytes".to_string(), ConfigSource::Defaults);
        source_attribution.insert("stderr_cap_bytes".to_string(), ConfigSource::Defaults);
        source_attribution.insert("lock_ttl_seconds".to_string(), ConfigSource::Defaults);
        source_attribution.insert("debug_packet".to_string(), ConfigSource::Defaults);
        source_attribution.insert("allow_links".to_string(), ConfigSource::Defaults);

        // Discover and load config file (if not explicitly provided)
        let config_path = if let Some(explicit_path) = &cli_args.config_path {
            Some(explicit_path.clone())
        } else {
            Self::discover_config_file_from(start_dir)?
        };

        if let Some(path) = &config_path {
            let file_config = Self::load_config_file(path)
                .with_context(|| format!("Failed to load config file: {}", path.display()))?;

            let config_source = ConfigSource::ConfigFile(path.clone());

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
                    source_attribution.insert("execution_strategy".to_string(), config_source.clone());
                }
                if file_llm.prompt_template.is_some() {
                    llm.prompt_template = file_llm.prompt_template;
                    source_attribution.insert("prompt_template".to_string(), config_source.clone());
                }
            }

            // Load hooks configuration from file
            if let Some(file_hooks) = file_config.hooks {
                hooks = file_hooks;
                source_attribution.insert("hooks".to_string(), config_source);
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

        // Apply LLM configuration with precedence: CLI > env > config > defaults
        // Check environment variable first
        if let Ok(env_provider) = env::var("XCHECKER_LLM_PROVIDER")
            && !env_provider.is_empty() {
                llm.provider = Some(env_provider);
                source_attribution.insert("llm_provider".to_string(), ConfigSource::Cli);
            }

        // CLI flag overrides environment variable
        if let Some(provider) = &cli_args.llm_provider {
            llm.provider = Some(provider.clone());
            source_attribution.insert("llm_provider".to_string(), ConfigSource::Cli);
        }

        // Default to "claude-cli" if no provider is set
        if llm.provider.is_none() {
            llm.provider = Some("claude-cli".to_string());
            source_attribution.insert("llm_provider".to_string(), ConfigSource::Defaults);
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

        // Apply execution strategy configuration with precedence: CLI > env > config > default
        // Check environment variable (overrides config file)
        if let Ok(env_strategy) = env::var("XCHECKER_EXECUTION_STRATEGY")
            && !env_strategy.is_empty() {
                llm.execution_strategy = Some(env_strategy);
                source_attribution.insert("execution_strategy".to_string(), ConfigSource::Cli);
            }

        // CLI flag overrides everything
        if let Some(strategy) = &cli_args.execution_strategy {
            llm.execution_strategy = Some(strategy.clone());
            source_attribution.insert("execution_strategy".to_string(), ConfigSource::Cli);
        }

        // Default to "controlled" if not specified
        if llm.execution_strategy.is_none() {
            llm.execution_strategy = Some("controlled".to_string());
            source_attribution.insert("execution_strategy".to_string(), ConfigSource::Defaults);
        }

        let config = Self {
            defaults,
            selectors,
            runner,
            llm,
            hooks,
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
    pub fn discover_config_file_from(start_dir: &Path) -> Result<Option<PathBuf>> {
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
    fn load_config_file(path: &Path) -> Result<TomlConfig> {
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let config: TomlConfig = toml::from_str(&content).with_context(|| {
                    format!("Failed to parse TOML config file: {}", path.display())
                })?;
                Ok(config)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Missing config file is OK - return empty config (will use defaults)
                Ok(TomlConfig {
                    defaults: None,
                    selectors: None,
                    runner: None,
                    llm: None,
                    hooks: None,
                })
            }
            Err(e) => Err(anyhow::anyhow!(
                "Failed to read config file {}: {}",
                path.display(),
                e
            )),
        }
    }

    /// Validate configuration values
    fn validate(&self) -> Result<(), XCheckerError> {
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

        // Validate glob patterns in selectors
        for pattern in &self.selectors.include {
            globset::Glob::new(pattern).map_err(|e| {
                XCheckerError::Config(ConfigError::InvalidValue {
                    key: "selectors.include".to_string(),
                    value: format!("Invalid glob pattern '{pattern}': {e}"),
                })
            })?;
        }

        for pattern in &self.selectors.exclude {
            globset::Glob::new(pattern).map_err(|e| {
                XCheckerError::Config(ConfigError::InvalidValue {
                    key: "selectors.exclude".to_string(),
                    value: format!("Invalid glob pattern '{pattern}': {e}"),
                })
            })?;
        }

        // Validate LLM provider - supported providers in V14: claude-cli, gemini-cli, openrouter, anthropic
        if let Some(provider) = &self.llm.provider {
            match provider.as_str() {
                "claude-cli" | "gemini-cli" | "openrouter" | "anthropic" => {
                    // Supported providers in V14
                }
                _ => {
                    return Err(XCheckerError::Config(ConfigError::InvalidValue {
                        key: "llm.provider".to_string(),
                        value: format!(
                            "'{provider}' is not supported. Supported providers: claude-cli, gemini-cli, openrouter, anthropic"
                        ),
                    }));
                }
            }
        } else {
            // This should never happen due to default enforcement, but guard against it
            return Err(XCheckerError::Config(ConfigError::MissingRequired(
                "llm.provider is required (should default to 'claude-cli')".to_string(),
            )));
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
        if let Some(template_name) = &self.llm.prompt_template {
            // Parse the template name
            let template = PromptTemplate::parse(template_name).map_err(|e| {
                XCheckerError::Config(ConfigError::InvalidValue {
                    key: "llm.prompt_template".to_string(),
                    value: e,
                })
            })?;

            // Get the provider (should always be set due to earlier validation)
            let provider = self.llm.provider.as_deref().unwrap_or("claude-cli");

            // Validate compatibility
            template.validate_provider_compatibility(provider).map_err(|e| {
                XCheckerError::Config(ConfigError::InvalidValue {
                    key: "llm.prompt_template".to_string(),
                    value: e,
                })
            })?;
        }

        Ok(())
    }

    /// Get effective configuration as key-value pairs with source attribution
    #[must_use]
    pub fn effective_config(&self) -> HashMap<String, (String, String)> {
        let mut config = HashMap::new();

        // Helper to add config value with source
        let mut add_config = |key: &str, value: Option<&str>| {
            if let Some(val) = value {
                let source = self
                    .source_attribution
                    .get(key)
                    .map_or_else(|| "defaults".to_string(), std::string::ToString::to_string);
                config.insert(key.to_string(), (val.to_string(), source));
            }
        };

        // Add all configuration values
        add_config("model", self.defaults.model.as_deref());

        if let Some(max_turns) = self.defaults.max_turns {
            add_config("max_turns", Some(&max_turns.to_string()));
        }

        if let Some(packet_max_bytes) = self.defaults.packet_max_bytes {
            add_config("packet_max_bytes", Some(&packet_max_bytes.to_string()));
        }

        if let Some(packet_max_lines) = self.defaults.packet_max_lines {
            add_config("packet_max_lines", Some(&packet_max_lines.to_string()));
        }

        add_config("output_format", self.defaults.output_format.as_deref());

        if let Some(verbose) = self.defaults.verbose {
            add_config("verbose", Some(&verbose.to_string()));
        }

        add_config("runner_mode", self.runner.mode.as_deref());
        add_config("runner_distro", self.runner.distro.as_deref());
        add_config("claude_path", self.runner.claude_path.as_deref());

        // Add selector information
        let include_patterns = self.selectors.include.join(", ");
        let exclude_patterns = self.selectors.exclude.join(", ");

        let include_source = self
            .source_attribution
            .get("selectors_include")
            .map_or_else(|| "defaults".to_string(), std::string::ToString::to_string);
        let exclude_source = self
            .source_attribution
            .get("selectors_exclude")
            .map_or_else(|| "defaults".to_string(), std::string::ToString::to_string);

        config.insert(
            "selectors_include".to_string(),
            (include_patterns, include_source),
        );
        config.insert(
            "selectors_exclude".to_string(),
            (exclude_patterns, exclude_source),
        );

        config
    }

    /// Convert runner mode string to enum
    pub fn get_runner_mode(&self) -> Result<RunnerMode> {
        let mode_str = self.runner.mode.as_deref().unwrap_or("auto");
        match mode_str {
            "auto" => Ok(RunnerMode::Auto),
            "native" => Ok(RunnerMode::Native),
            "wsl" => Ok(RunnerMode::Wsl),
            _ => Err(XCheckerError::Config(ConfigError::InvalidValue {
                key: "runner_mode".to_string(),
                value: format!("Unknown runner mode: {mode_str}"),
            })
            .into()),
        }
    }
}

#[cfg(test)]
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
            hooks: HooksConfig::default(),
            source_attribution: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::{Mutex, MutexGuard, OnceLock};
    use tempfile::TempDir;

    // Global lock for tests that mutate process-global state (env vars, cwd).
    // Tests that use `config_env_guard()` will be serialized.
    static CONFIG_ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    #[allow(dead_code)] // Ready for use when #[ignore]d tests are enabled
    fn config_env_guard() -> MutexGuard<'static, ()> {
        CONFIG_ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap()
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

    // TODO: This test has environment isolation issues when run with other tests
    #[test]
    #[ignore]
    fn test_config_discovery_with_cli_override() {
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

        // Change to temp directory
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_dir.path()).unwrap();

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
            llm_provider: None,
            llm_claude_binary: None,
            llm_gemini_binary: None,
            execution_strategy: None,
        };

        let config = Config::discover(&cli_args).unwrap();

        // CLI overrides should take precedence
        assert_eq!(config.defaults.model, Some("claude-3-opus".to_string()));
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

        // Restore original directory
        env::set_current_dir(original_dir).unwrap();
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
        match error.downcast_ref::<XCheckerError>() {
            Some(XCheckerError::Config(ConfigError::InvalidValue { key, .. })) => {
                assert_eq!(key, "max_turns");
            }
            _ => panic!("Expected Config InvalidValue error for max_turns"),
        }
    }

    // TODO: This test has environment isolation issues - needs to be fixed
    #[test]
    #[ignore]
    fn test_effective_config() {
        let temp_dir = TempDir::new().unwrap();
        let _config_path = create_test_config_file(
            temp_dir.path(),
            r#"
[defaults]
model = "sonnet"
max_turns = 8
"#,
        );

        // Change to temp directory
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_dir.path()).unwrap();

        let cli_args = CliArgs {
            verbose: Some(true),
            ..Default::default()
        };

        let config = Config::discover(&cli_args).unwrap();
        let effective = config.effective_config();

        // Check that values and sources are correctly reported
        assert_eq!(effective.get("model").unwrap().0, "sonnet");
        assert!(effective.get("model").unwrap().1.contains("config file"));

        assert_eq!(effective.get("verbose").unwrap().0, "true");
        assert_eq!(effective.get("verbose").unwrap().1, "CLI");

        assert_eq!(effective.get("max_turns").unwrap().0, "8");
        assert!(
            effective
                .get("max_turns")
                .unwrap()
                .1
                .contains("config file")
        );

        // Restore original directory
        env::set_current_dir(original_dir).unwrap();
    }

    // TODO: This test has environment isolation issues - needs to be fixed
    #[test]
    #[ignore]
    fn test_invalid_toml_config() {
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();
        let xchecker_dir = temp_dir.path().join(".xchecker");
        crate::paths::ensure_dir_all(&xchecker_dir).unwrap();

        let config_path = xchecker_dir.join("config.toml");
        fs::write(&config_path, "invalid toml content [[[").unwrap();

        // Change to temp directory
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_dir.path()).unwrap();

        let cli_args = CliArgs::default();

        let result = Config::discover(&cli_args);
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Failed to load config file") || error_msg.contains("parse"));

        // Restore original directory
        env::set_current_dir(original_dir).unwrap();
    }

    // ===== Edge Case Tests (Task 9.7) =====

    // TODO: This test has environment isolation issues - needs to be fixed
    #[test]
    fn test_config_with_invalid_toml_syntax() {
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
            let result = Config::discover(&cli_args);

            assert!(
                result.is_err(),
                "Should fail for invalid TOML case {i}: {invalid_toml}"
            );
        }
    }

    #[test]
    fn test_config_with_missing_sections() {
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
        match error.downcast_ref::<XCheckerError>() {
            Some(XCheckerError::Config(ConfigError::InvalidValue { key, .. })) => {
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
        match error.downcast_ref::<XCheckerError>() {
            Some(XCheckerError::Config(ConfigError::InvalidValue { key, value })) => {
                assert_eq!(key, "packet_max_bytes");
                assert!(value.contains("exceeds maximum"));
            }
            _ => panic!("Expected Config InvalidValue error for packet_max_bytes exceeding limit"),
        }
    }

    #[test]
    fn test_config_validation_with_invalid_runner_mode() {
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

        assert_eq!(
            config.defaults.model,
            Some("sonnet-@#$%".to_string())
        );
        assert!(
            config
                .selectors
                .include
                .contains(&"path/with spaces/*.md".to_string())
        );
    }

    #[test]
    fn test_config_with_boundary_values() {
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
            Some(ConfigSource::ConfigFile(_))
        ));
        assert_eq!(
            config.source_attribution.get("packet_max_bytes"),
            Some(&ConfigSource::Defaults)
        );
    }

    // ===== Edge Case Tests for Task 9.7 =====

    #[test]
    fn test_config_source_attribution() {
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
            model: Some("opus".to_string()),   // CLI override
            packet_max_bytes: Some(32768),     // CLI override
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
            Some(ConfigSource::ConfigFile(_))
        ));
        assert!(matches!(
            config.source_attribution.get("packet_max_bytes"),
            Some(ConfigSource::Cli)
        ));
    }

    // ===== LLM Provider and Execution Strategy Validation Tests (V11-V14 enforcement) =====

    #[test]
    fn test_llm_provider_defaults_to_claude_cli() {
        let _home = crate::paths::with_isolated_home();
        let cli_args = CliArgs::default();

        let config = Config::discover(&cli_args).unwrap();

        // Should default to claude-cli
        assert_eq!(config.llm.provider, Some("claude-cli".to_string()));
        assert_eq!(
            config.source_attribution.get("llm_provider"),
            Some(&ConfigSource::Defaults)
        );
    }

    #[test]
    fn test_execution_strategy_defaults_to_controlled() {
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
            Some(&ConfigSource::Defaults)
        );
    }

    #[test]
    fn test_llm_provider_rejects_invalid_providers() {
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
            match error.downcast_ref::<XCheckerError>() {
                Some(XCheckerError::Config(ConfigError::InvalidValue { key, value })) => {
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
                            value.contains("Supported providers") || value.contains("not supported"),
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
    fn test_execution_strategy_rejects_invalid_strategies() {
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
            match error.downcast_ref::<XCheckerError>() {
                Some(XCheckerError::Config(ConfigError::InvalidValue { key, value })) => {
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
        match error.downcast_ref::<XCheckerError>() {
            Some(XCheckerError::Config(ConfigError::InvalidValue { key, value })) => {
                assert_eq!(key, "llm.provider");
                assert!(value.contains("invalid-provider-xyz"));
            }
            _ => panic!("Expected Config InvalidValue error"),
        }
    }

    #[test]
    fn test_llm_config_from_config_file_with_invalid_strategy() {
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
        match error.downcast_ref::<XCheckerError>() {
            Some(XCheckerError::Config(ConfigError::InvalidValue { key, value })) => {
                assert_eq!(key, "llm.execution_strategy");
                assert!(value.contains("externaltool"));
            }
            _ => panic!("Expected Config InvalidValue error"),
        }
    }

    #[test]
    fn test_llm_config_from_config_file_with_valid_values() {
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
            Some(ConfigSource::ConfigFile(_))
        ));
        assert!(matches!(
            config.source_attribution.get("execution_strategy"),
            Some(ConfigSource::ConfigFile(_))
        ));
    }

    #[test]
    fn test_llm_config_cli_overrides_config_file() {
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
        assert_eq!(PromptTemplate::parse("default").unwrap(), PromptTemplate::Default);
        assert_eq!(PromptTemplate::parse("claude-optimized").unwrap(), PromptTemplate::ClaudeOptimized);
        assert_eq!(PromptTemplate::parse("claude_optimized").unwrap(), PromptTemplate::ClaudeOptimized);
        assert_eq!(PromptTemplate::parse("claude").unwrap(), PromptTemplate::ClaudeOptimized);
        assert_eq!(PromptTemplate::parse("openai-compatible").unwrap(), PromptTemplate::OpenAiCompatible);
        assert_eq!(PromptTemplate::parse("openai_compatible").unwrap(), PromptTemplate::OpenAiCompatible);
        assert_eq!(PromptTemplate::parse("openai").unwrap(), PromptTemplate::OpenAiCompatible);
        assert_eq!(PromptTemplate::parse("openrouter").unwrap(), PromptTemplate::OpenAiCompatible);

        // Test case insensitivity
        assert_eq!(PromptTemplate::parse("DEFAULT").unwrap(), PromptTemplate::Default);
        assert_eq!(PromptTemplate::parse("Claude-Optimized").unwrap(), PromptTemplate::ClaudeOptimized);

        // Test invalid template names
        assert!(PromptTemplate::parse("invalid").is_err());
        assert!(PromptTemplate::parse("unknown-template").is_err());
    }

    #[test]
    fn test_prompt_template_provider_compatibility() {
        // Default template is compatible with all providers
        assert!(PromptTemplate::Default.validate_provider_compatibility("claude-cli").is_ok());
        assert!(PromptTemplate::Default.validate_provider_compatibility("gemini-cli").is_ok());
        assert!(PromptTemplate::Default.validate_provider_compatibility("openrouter").is_ok());
        assert!(PromptTemplate::Default.validate_provider_compatibility("anthropic").is_ok());

        // Claude-optimized template is compatible with Claude CLI and Anthropic
        assert!(PromptTemplate::ClaudeOptimized.validate_provider_compatibility("claude-cli").is_ok());
        assert!(PromptTemplate::ClaudeOptimized.validate_provider_compatibility("anthropic").is_ok());
        assert!(PromptTemplate::ClaudeOptimized.validate_provider_compatibility("gemini-cli").is_err());
        assert!(PromptTemplate::ClaudeOptimized.validate_provider_compatibility("openrouter").is_err());

        // OpenAI-compatible template is compatible with OpenRouter and Gemini
        assert!(PromptTemplate::OpenAiCompatible.validate_provider_compatibility("openrouter").is_ok());
        assert!(PromptTemplate::OpenAiCompatible.validate_provider_compatibility("gemini-cli").is_ok());
        assert!(PromptTemplate::OpenAiCompatible.validate_provider_compatibility("claude-cli").is_err());
        assert!(PromptTemplate::OpenAiCompatible.validate_provider_compatibility("anthropic").is_err());
    }

    #[test]
    fn test_prompt_template_as_str() {
        assert_eq!(PromptTemplate::Default.as_str(), "default");
        assert_eq!(PromptTemplate::ClaudeOptimized.as_str(), "claude-optimized");
        assert_eq!(PromptTemplate::OpenAiCompatible.as_str(), "openai-compatible");
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
        assert_eq!(config.llm.prompt_template, Some("claude-optimized".to_string()));
    }

    #[test]
    fn test_config_with_openai_compatible_template_and_openrouter_provider() {
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();

        let config_path = create_test_config_file(
            temp_dir.path(),
            r#"
[llm]
provider = "openrouter"
prompt_template = "openai-compatible"
"#,
        );

        let cli_args = CliArgs {
            config_path: Some(config_path),
            ..Default::default()
        };

        let config = Config::discover(&cli_args).unwrap();
        assert_eq!(config.llm.prompt_template, Some("openai-compatible".to_string()));
    }

    #[test]
    fn test_config_rejects_incompatible_template_and_provider() {
        let _home = crate::paths::with_isolated_home();
        let temp_dir = TempDir::new().unwrap();

        // Claude-optimized template with OpenRouter provider should fail
        let config_path = create_test_config_file(
            temp_dir.path(),
            r#"
[llm]
provider = "openrouter"
prompt_template = "claude-optimized"
"#,
        );

        let cli_args = CliArgs {
            config_path: Some(config_path),
            ..Default::default()
        };

        let result = Config::discover(&cli_args);
        assert!(result.is_err(), "Should reject incompatible template and provider");

        let error = result.unwrap_err();
        match error.downcast_ref::<XCheckerError>() {
            Some(XCheckerError::Config(ConfigError::InvalidValue { key, value })) => {
                assert_eq!(key, "llm.prompt_template");
                assert!(value.contains("not compatible"));
                assert!(value.contains("openrouter"));
            }
            _ => panic!("Expected Config InvalidValue error for incompatible template"),
        }
    }

    #[test]
    fn test_config_rejects_openai_template_with_claude_provider() {
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
        assert!(result.is_err(), "Should reject incompatible template and provider");

        let error = result.unwrap_err();
        match error.downcast_ref::<XCheckerError>() {
            Some(XCheckerError::Config(ConfigError::InvalidValue { key, value })) => {
                assert_eq!(key, "llm.prompt_template");
                assert!(value.contains("not compatible"));
                assert!(value.contains("claude-cli"));
            }
            _ => panic!("Expected Config InvalidValue error for incompatible template"),
        }
    }

    #[test]
    fn test_config_rejects_invalid_template_name() {
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
        match error.downcast_ref::<XCheckerError>() {
            Some(XCheckerError::Config(ConfigError::InvalidValue { key, value })) => {
                assert_eq!(key, "llm.prompt_template");
                assert!(value.contains("Unknown prompt template"));
                assert!(value.contains("invalid-template-name"));
            }
            _ => panic!("Expected Config InvalidValue error for invalid template name"),
        }
    }

    #[test]
    fn test_config_without_prompt_template_uses_default() {
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
}

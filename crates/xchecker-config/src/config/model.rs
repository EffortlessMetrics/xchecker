use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use xchecker_selectors::Selectors;
use xchecker_utils::types::ConfigSource;

/// Default timeout for hook execution in seconds
pub const DEFAULT_HOOK_TIMEOUT_SECS: u64 = 60;

/// Hook failure behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OnFail {
    /// Log warning and continue (default)
    #[default]
    Warn,
    /// Fail the phase on hook failure
    Fail,
}

impl std::fmt::Display for OnFail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Warn => write!(f, "warn"),
            Self::Fail => write!(f, "fail"),
        }
    }
}

/// Hook type indicating when the hook runs
/// Reserved for hooks integration; not wired in v1.0
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookType {
    /// Runs before phase execution
    PrePhase,
    /// Runs after phase execution
    PostPhase,
}

impl std::fmt::Display for HookType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PrePhase => write!(f, "pre_phase"),
            Self::PostPhase => write!(f, "post_phase"),
        }
    }
}

/// Configuration for a single hook
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HookConfig {
    /// Command to execute (can be a script path or shell command)
    pub command: String,
    /// Behavior on hook failure (default: warn)
    #[serde(default)]
    pub on_fail: OnFail,
    /// Timeout in seconds (default: 60)
    #[serde(default = "default_timeout")]
    pub timeout: u64,
}

fn default_timeout() -> u64 {
    DEFAULT_HOOK_TIMEOUT_SECS
}

/// Hooks configuration section from config.toml
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct HooksConfig {
    /// Pre-phase hooks keyed by phase name
    #[serde(default)]
    pub pre_phase: HashMap<String, HookConfig>,
    /// Post-phase hooks keyed by phase name
    #[serde(default)]
    pub post_phase: HashMap<String, HookConfig>,
}

impl HooksConfig {
    /// Get a pre-phase hook for the given phase
    /// Reserved for hooks integration; not wired in v1.0
    #[must_use]
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn get_pre_phase_hook(&self, phase: crate::types::PhaseId) -> Option<&HookConfig> {
        self.pre_phase.get(phase.as_str())
    }

    /// Get a post-phase hook for the given phase
    /// Reserved for hooks integration; not wired in v1.0
    #[must_use]
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn get_post_phase_hook(&self, phase: crate::types::PhaseId) -> Option<&HookConfig> {
        self.post_phase.get(phase.as_str())
    }

    /// Check if any hooks are configured
    /// Reserved for hooks integration; not wired in v1.0
    #[must_use]
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn has_hooks(&self) -> bool {
        !self.pre_phase.is_empty() || !self.post_phase.is_empty()
    }
}

/// Configuration for xchecker operations.
///
/// `Config` provides hierarchical configuration with discovery and precedence:
/// CLI arguments > config file > built-in defaults.
///
/// # Discovery
///
/// Use [`Config::discover()`] for CLI-like behavior that:
/// - Searches for `.xchecker/config.toml` upward from current directory
/// - Respects the `XCHECKER_HOME` environment variable
/// - Applies built-in defaults for unspecified values
///
/// # Programmatic Configuration
///
/// For embedding scenarios where you need deterministic behavior independent
/// of the user's environment, construct a `Config` directly or use
/// [`OrchestratorHandle::from_config()`](crate::OrchestratorHandle::from_config).
///
/// # Source Attribution
///
/// Each configuration value tracks its source (`cli`, `config`, `programmatic`, or `default`)
/// for debugging and status display.
///
/// # Example
///
/// ```rust,no_run
/// use xchecker_config::Config;
/// use xchecker_config::CliArgs;
///
/// // Discover configuration using CLI semantics
/// let config = Config::discover(&CliArgs::default())?;
///
/// // Access configuration values
/// println!("Model: {:?}", config.defaults.model);
/// println!("Max turns: {:?}", config.defaults.max_turns);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Configuration File Format
///
/// Configuration files use TOML format with these sections:
///
/// ```toml
/// [defaults]
/// model = "haiku"
/// max_turns = 6
/// phase_timeout = 600
///
/// [selectors]
/// include = ["**/*.md", "**/*.yaml"]
/// exclude = ["target/**", "node_modules/**"]
///
/// [runner]
/// mode = "auto"
///
/// [llm]
/// provider = "claude-cli"
/// ```
#[derive(Debug, Clone)]
pub struct Config {
    /// Default values for various settings.
    pub defaults: Defaults,
    /// File selection patterns for packet building.
    pub selectors: Selectors,
    /// Runner configuration for cross-platform execution.
    pub runner: RunnerConfig,
    /// LLM provider configuration.
    pub llm: LlmConfig,
    /// Per-phase configuration overrides.
    pub phases: PhasesConfig,
    /// Hooks configuration for pre/post phase scripts.
    // Reserved for hooks integration; not wired in v1.0
    #[allow(dead_code)]
    pub hooks: HooksConfig,
    /// Security configuration for secret detection and redaction.
    pub security: SecurityConfig,
    /// Source attribution for each setting (for status display).
    pub source_attribution: HashMap<String, ConfigSource>,
}

/// Default configuration values
///
/// # Model selection
///
/// - **Testing/Development**: Leave `model` unset to use `haiku` (fast, cost-effective)
/// - **Production**: Set `model = "sonnet"` or `model = "default"` for best results
/// - **Complex tasks**: Set `model = "opus"` for maximum capability
///
/// Specific model versions (e.g., `claude-sonnet-4-5-20250929`) can be used for
/// reproducibility but simple aliases are recommended.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Defaults {
    /// Model to use. Default: haiku (for testing). Use "sonnet" or "default" for production.
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
    /// Enable strict validation for phase outputs.
    ///
    /// When enabled, validation failures (meta-summaries, too-short output,
    /// missing required sections) become hard errors that fail the phase.
    /// When disabled (default), validation issues are logged as warnings only.
    pub strict_validation: Option<bool>,
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

/// Per-phase configuration overrides
///
/// Allows configuring model, timeout, and max_turns on a per-phase basis.
/// Values set here override global defaults for that specific phase.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct PhaseConfig {
    /// Model to use for this phase (overrides defaults.model)
    pub model: Option<String>,
    /// Maximum turns for this phase (overrides defaults.max_turns)
    pub max_turns: Option<u32>,
    /// Phase timeout in seconds (overrides defaults.phase_timeout)
    pub phase_timeout: Option<u64>,
}

/// Phase-specific configuration section
///
/// Contains optional per-phase configuration overrides.
/// If a phase is not specified or None, global defaults are used.
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
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct PhasesConfig {
    pub requirements: Option<PhaseConfig>,
    pub design: Option<PhaseConfig>,
    pub tasks: Option<PhaseConfig>,
    pub review: Option<PhaseConfig>,
    pub fixup: Option<PhaseConfig>,
    #[serde(rename = "final")]
    pub final_: Option<PhaseConfig>,
}

/// Runner configuration for cross-platform execution
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RunnerConfig {
    pub mode: Option<String>,
    pub distro: Option<String>,
    pub claude_path: Option<String>,
}

/// Security configuration for secret detection and redaction
///
/// This section allows customizing secret detection patterns:
/// - Add extra patterns to detect project-specific secrets
/// - Ignore patterns that cause false positives
///
/// # Example
///
/// ```toml
/// [security]
/// extra_secret_patterns = ["SECRET_[A-Z0-9]{32}", "API_KEY_[A-Za-z0-9]{40}"]
/// ignore_secret_patterns = ["github_pat"]
/// ```
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct SecurityConfig {
    /// Additional regex patterns for secret detection.
    ///
    /// These patterns are added to the built-in patterns and will cause
    /// secret detection to trigger if matched.
    #[serde(default)]
    pub extra_secret_patterns: Vec<String>,

    /// Patterns to suppress from secret detection.
    ///
    /// Pattern IDs listed here will be ignored during secret scanning.
    /// Use this to suppress false positives for known-safe patterns.
    ///
    /// **Warning:** Suppressing patterns reduces security. Only suppress
    /// patterns if you're certain they won't match real secrets.
    #[serde(default)]
    pub ignore_secret_patterns: Vec<String>,
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
            strict_validation: None, // Default: soft validation (warnings only)
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

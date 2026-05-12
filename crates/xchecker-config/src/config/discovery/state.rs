use std::collections::HashMap;

use super::super::{
    ClaudeConfig, Config, ConfigSource, Defaults, GeminiConfig, HooksConfig, LlmConfig,
    PhasesConfig, RunnerConfig, SecurityConfig, Selectors,
};

pub(super) struct DiscoveryState {
    pub(super) defaults: Defaults,
    pub(super) selectors: Selectors,
    pub(super) runner: RunnerConfig,
    pub(super) llm: LlmConfig,
    pub(super) phases: PhasesConfig,
    pub(super) hooks: HooksConfig,
    pub(super) security: SecurityConfig,
    pub(super) source_attribution: HashMap<String, ConfigSource>,
}

impl DiscoveryState {
    pub(super) fn with_defaults() -> Self {
        let mut state = Self {
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
            phases: PhasesConfig::default(),
            security: SecurityConfig::default(),
            source_attribution: HashMap::new(),
        };
        state.record_builtin_default_sources();
        state
    }

    pub(super) fn set_source(&mut self, key: &str, source: ConfigSource) {
        self.source_attribution.insert(key.to_string(), source);
    }

    pub(super) fn ensure_claude(&mut self) -> &mut ClaudeConfig {
        self.llm
            .claude
            .get_or_insert_with(|| ClaudeConfig { binary: None })
    }

    pub(super) fn ensure_gemini(&mut self) -> &mut GeminiConfig {
        self.llm.gemini.get_or_insert_with(|| GeminiConfig {
            binary: None,
            default_model: None,
            profiles: None,
        })
    }

    pub(super) fn into_config(self) -> Config {
        Config {
            defaults: self.defaults,
            selectors: self.selectors,
            runner: self.runner,
            llm: self.llm,
            phases: self.phases,
            hooks: self.hooks,
            security: self.security,
            source_attribution: self.source_attribution,
        }
    }

    fn record_builtin_default_sources(&mut self) {
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
        ] {
            self.set_source(key, ConfigSource::Default);
        }
    }
}

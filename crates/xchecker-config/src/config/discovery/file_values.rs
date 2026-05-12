use super::super::ConfigSource;
use super::file_loader::TomlConfig;
use super::state::DiscoveryState;

pub(super) fn apply_file_config(state: &mut DiscoveryState, file_config: TomlConfig) {
    let config_source = ConfigSource::Config;

    if let Some(file_defaults) = file_config.defaults {
        if file_defaults.model.is_some() {
            state.defaults.model = file_defaults.model;
            state.set_source("model", config_source.clone());
        }
        if file_defaults.max_turns.is_some() {
            state.defaults.max_turns = file_defaults.max_turns;
            state.set_source("max_turns", config_source.clone());
        }
        if file_defaults.packet_max_bytes.is_some() {
            state.defaults.packet_max_bytes = file_defaults.packet_max_bytes;
            state.set_source("packet_max_bytes", config_source.clone());
        }
        if file_defaults.packet_max_lines.is_some() {
            state.defaults.packet_max_lines = file_defaults.packet_max_lines;
            state.set_source("packet_max_lines", config_source.clone());
        }
        if file_defaults.output_format.is_some() {
            state.defaults.output_format = file_defaults.output_format;
            state.set_source("output_format", config_source.clone());
        }
        if file_defaults.verbose.is_some() {
            state.defaults.verbose = file_defaults.verbose;
            state.set_source("verbose", config_source.clone());
        }
        if file_defaults.phase_timeout.is_some() {
            state.defaults.phase_timeout = file_defaults.phase_timeout;
            state.set_source("phase_timeout", config_source.clone());
        }
        if file_defaults.stdout_cap_bytes.is_some() {
            state.defaults.stdout_cap_bytes = file_defaults.stdout_cap_bytes;
            state.set_source("stdout_cap_bytes", config_source.clone());
        }
        if file_defaults.stderr_cap_bytes.is_some() {
            state.defaults.stderr_cap_bytes = file_defaults.stderr_cap_bytes;
            state.set_source("stderr_cap_bytes", config_source.clone());
        }
        if file_defaults.lock_ttl_seconds.is_some() {
            state.defaults.lock_ttl_seconds = file_defaults.lock_ttl_seconds;
            state.set_source("lock_ttl_seconds", config_source.clone());
        }
        if file_defaults.debug_packet.is_some() {
            state.defaults.debug_packet = file_defaults.debug_packet;
            state.set_source("debug_packet", config_source.clone());
        }
        if file_defaults.allow_links.is_some() {
            state.defaults.allow_links = file_defaults.allow_links;
            state.set_source("allow_links", config_source.clone());
        }
        if file_defaults.strict_validation.is_some() {
            state.defaults.strict_validation = file_defaults.strict_validation;
            state.set_source("strict_validation", config_source.clone());
        }
    }

    if let Some(file_selectors) = file_config.selectors {
        if !file_selectors.include.is_empty() {
            state.selectors.include = file_selectors.include;
            state.set_source("selectors_include", config_source.clone());
        }
        if !file_selectors.exclude.is_empty() {
            state.selectors.exclude = file_selectors.exclude;
            state.set_source("selectors_exclude", config_source.clone());
        }
    }

    if let Some(file_runner) = file_config.runner {
        if file_runner.mode.is_some() {
            state.runner.mode = file_runner.mode;
            state.set_source("runner_mode", config_source.clone());
        }
        if file_runner.distro.is_some() {
            state.runner.distro = file_runner.distro;
            state.set_source("runner_distro", config_source.clone());
        }
        if file_runner.claude_path.is_some() {
            state.runner.claude_path = file_runner.claude_path;
            state.set_source("claude_path", config_source.clone());
        }
    }

    if let Some(file_llm) = file_config.llm {
        if file_llm.provider.is_some() {
            state.llm.provider = file_llm.provider;
            state.set_source("llm_provider", config_source.clone());
        }
        if file_llm.fallback_provider.is_some() {
            state.llm.fallback_provider = file_llm.fallback_provider;
            state.set_source("llm_fallback_provider", config_source.clone());
        }
        if let Some(file_claude) = file_llm.claude
            && file_claude.binary.is_some()
        {
            state.llm.claude = Some(file_claude);
            state.set_source("llm_claude_binary", config_source.clone());
        }
        if let Some(file_gemini) = file_llm.gemini {
            state.llm.gemini = Some(file_gemini);
            state.set_source("llm_gemini_config", config_source.clone());
        }
        if let Some(file_openrouter) = file_llm.openrouter {
            state.llm.openrouter = Some(file_openrouter);
            state.set_source("llm_openrouter_config", config_source.clone());
        }
        if let Some(file_anthropic) = file_llm.anthropic {
            state.llm.anthropic = Some(file_anthropic);
            state.set_source("llm_anthropic_config", config_source.clone());
        }
        if file_llm.execution_strategy.is_some() {
            state.llm.execution_strategy = file_llm.execution_strategy;
            state.set_source("execution_strategy", config_source.clone());
        }
        if file_llm.prompt_template.is_some() {
            state.llm.prompt_template = file_llm.prompt_template;
            state.set_source("prompt_template", config_source.clone());
        }
    }

    if let Some(file_phases) = file_config.phases {
        state.phases = file_phases;
        state.set_source("phases", config_source.clone());
    }

    if let Some(file_hooks) = file_config.hooks {
        state.hooks = file_hooks;
        state.set_source("hooks", config_source.clone());
    }

    if let Some(file_security) = file_config.security {
        state.security = file_security;
        state.set_source("security", config_source);
    }
}

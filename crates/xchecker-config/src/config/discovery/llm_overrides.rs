use std::env;

use super::super::{CliArgs, ConfigSource};
use super::state::DiscoveryState;

pub(super) fn apply_llm_env_cli_overrides_and_defaults(
    state: &mut DiscoveryState,
    cli_args: &CliArgs,
) {
    apply_provider(state, cli_args);
    apply_fallback_provider(state, cli_args);
    apply_prompt_template(state, cli_args);
    apply_gemini_default_model(state, cli_args);
    apply_execution_strategy(state, cli_args);
}

fn apply_provider(state: &mut DiscoveryState, cli_args: &CliArgs) {
    if let Ok(env_provider) = env::var("XCHECKER_LLM_PROVIDER")
        && !env_provider.is_empty()
    {
        state.llm.provider = Some(env_provider);
        state.set_source("llm_provider", ConfigSource::Env);
    }

    if let Some(provider) = &cli_args.llm_provider {
        state.llm.provider = Some(provider.clone());
        state.set_source("llm_provider", ConfigSource::Cli);
    }

    if state.llm.provider.is_none() {
        state.llm.provider = Some("claude-cli".to_string());
        state.set_source("llm_provider", ConfigSource::Default);
    }
}

fn apply_fallback_provider(state: &mut DiscoveryState, cli_args: &CliArgs) {
    if let Ok(env_fallback) = env::var("XCHECKER_LLM_FALLBACK_PROVIDER")
        && !env_fallback.is_empty()
    {
        state.llm.fallback_provider = Some(env_fallback);
        state.set_source("llm_fallback_provider", ConfigSource::Env);
    }

    if let Some(fallback_provider) = &cli_args.llm_fallback_provider {
        state.llm.fallback_provider = Some(fallback_provider.clone());
        state.set_source("llm_fallback_provider", ConfigSource::Cli);
    }
}

fn apply_prompt_template(state: &mut DiscoveryState, cli_args: &CliArgs) {
    if let Ok(env_template) = env::var("XCHECKER_LLM_PROMPT_TEMPLATE")
        && !env_template.is_empty()
    {
        state.llm.prompt_template = Some(env_template);
        state.set_source("prompt_template", ConfigSource::Env);
    }

    if let Some(prompt_template) = &cli_args.prompt_template {
        state.llm.prompt_template = Some(prompt_template.clone());
        state.set_source("prompt_template", ConfigSource::Cli);
    }
}

fn apply_gemini_default_model(state: &mut DiscoveryState, cli_args: &CliArgs) {
    if let Ok(env_default_model) = env::var("XCHECKER_LLM_GEMINI_DEFAULT_MODEL")
        && !env_default_model.is_empty()
    {
        state.ensure_gemini().default_model = Some(env_default_model);
        state.set_source("llm_gemini_default_model", ConfigSource::Env);
    }

    if let Some(default_model) = &cli_args.llm_gemini_default_model {
        state.ensure_gemini().default_model = Some(default_model.clone());
        state.set_source("llm_gemini_default_model", ConfigSource::Cli);
    }
}

fn apply_execution_strategy(state: &mut DiscoveryState, cli_args: &CliArgs) {
    if let Ok(env_strategy) = env::var("XCHECKER_EXECUTION_STRATEGY")
        && !env_strategy.is_empty()
    {
        state.llm.execution_strategy = Some(env_strategy);
        state.set_source("execution_strategy", ConfigSource::Env);
    }

    if let Some(strategy) = &cli_args.execution_strategy {
        state.llm.execution_strategy = Some(strategy.clone());
        state.set_source("execution_strategy", ConfigSource::Cli);
    }

    if state.llm.execution_strategy.is_none() {
        state.llm.execution_strategy = Some("controlled".to_string());
        state.set_source("execution_strategy", ConfigSource::Default);
    }
}

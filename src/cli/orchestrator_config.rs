use std::collections::HashMap;
use std::sync::Arc;

use crate::redaction::SecretRedactor;
use crate::{CliArgs, Config, OrchestratorConfig};

/// Create default configuration from Config struct and CLI args
pub(super) fn create_default_config(
    verbose: bool,
    config: &Config,
    cli_args: &CliArgs,
) -> HashMap<String, String> {
    let mut config_map = HashMap::new();

    if verbose {
        config_map.insert("verbose".to_string(), "true".to_string());
    }

    // Use values from the configuration system
    if let Some(packet_max_bytes) = config.defaults.packet_max_bytes {
        config_map.insert("packet_max_bytes".to_string(), packet_max_bytes.to_string());
    }

    if let Some(packet_max_lines) = config.defaults.packet_max_lines {
        config_map.insert("packet_max_lines".to_string(), packet_max_lines.to_string());
    }

    if let Some(max_turns) = config.defaults.max_turns {
        config_map.insert("max_turns".to_string(), max_turns.to_string());
    }

    if let Some(model) = &config.defaults.model {
        config_map.insert("model".to_string(), model.clone());
    }

    if let Some(output_format) = &config.defaults.output_format {
        config_map.insert("output_format".to_string(), output_format.clone());
    }

    if let Some(phase_timeout) = config.defaults.phase_timeout {
        config_map.insert("phase_timeout".to_string(), phase_timeout.to_string());
    }

    if let Some(stdout_cap_bytes) = config.defaults.stdout_cap_bytes {
        config_map.insert("stdout_cap_bytes".to_string(), stdout_cap_bytes.to_string());
    }

    if let Some(stderr_cap_bytes) = config.defaults.stderr_cap_bytes {
        config_map.insert("stderr_cap_bytes".to_string(), stderr_cap_bytes.to_string());
    }

    if let Some(lock_ttl_seconds) = config.defaults.lock_ttl_seconds {
        config_map.insert("lock_ttl_seconds".to_string(), lock_ttl_seconds.to_string());
    }

    if let Some(debug_packet) = config.defaults.debug_packet
        && debug_packet
    {
        config_map.insert("debug_packet".to_string(), "true".to_string());
    }

    if let Some(allow_links) = config.defaults.allow_links
        && allow_links
    {
        config_map.insert("allow_links".to_string(), "true".to_string());
    }

    if let Some(runner_mode) = &config.runner.mode {
        config_map.insert("runner_mode".to_string(), runner_mode.clone());
    }

    if let Some(runner_distro) = &config.runner.distro {
        config_map.insert("runner_distro".to_string(), runner_distro.clone());
    }

    if let Some(claude_path) = &config.runner.claude_path {
        config_map.insert("claude_path".to_string(), claude_path.clone());
    }

    if let Some(provider) = &config.llm.provider {
        config_map.insert("llm_provider".to_string(), provider.clone());
    }

    if let Some(fallback_provider) = &config.llm.fallback_provider {
        config_map.insert(
            "llm_fallback_provider".to_string(),
            fallback_provider.clone(),
        );
    }

    if let Some(execution_strategy) = &config.llm.execution_strategy {
        config_map.insert("execution_strategy".to_string(), execution_strategy.clone());
    }

    if let Some(prompt_template) = &config.llm.prompt_template {
        config_map.insert("prompt_template".to_string(), prompt_template.clone());
    }

    if let Some(claude_config) = &config.llm.claude
        && let Some(binary) = &claude_config.binary
    {
        config_map.insert("llm_claude_binary".to_string(), binary.clone());
    }

    if let Some(gemini_config) = &config.llm.gemini {
        if let Some(binary) = &gemini_config.binary {
            config_map.insert("llm_gemini_binary".to_string(), binary.clone());
        }
        if let Some(default_model) = &gemini_config.default_model {
            config_map.insert(
                "llm_gemini_default_model".to_string(),
                default_model.clone(),
            );
        }
    }

    // Add new CLI arguments (R7.2, R7.4, R9.2)
    if !cli_args.allow.is_empty() {
        config_map.insert("allowed_tools".to_string(), cli_args.allow.join(","));
    }

    if !cli_args.deny.is_empty() {
        config_map.insert("disallowed_tools".to_string(), cli_args.deny.join(","));
    }

    if cli_args.dangerously_skip_permissions {
        config_map.insert(
            "dangerously_skip_permissions".to_string(),
            "true".to_string(),
        );
    }

    if !cli_args.ignore_secret_pattern.is_empty() {
        config_map.insert(
            "ignore_secret_patterns".to_string(),
            cli_args.ignore_secret_pattern.join("|"),
        );
    }

    if !cli_args.extra_secret_pattern.is_empty() {
        config_map.insert(
            "extra_secret_patterns".to_string(),
            cli_args.extra_secret_pattern.join("|"),
        );
    }

    // Add debug_packet flag (FR-PKT-006, FR-PKT-007) for CLI-only overrides
    if cli_args.debug_packet {
        config_map.insert("debug_packet".to_string(), "true".to_string());
    }

    config_map
}

/// Build an OrchestratorConfig from CLI parameters.
///
/// This helper reduces duplication between execute_spec_command and execute_resume_command
/// by combining create_default_config with the common additional parameters.
///
/// # Arguments
/// * `dry_run` - Whether to run in simulation mode
/// * `verbose` - Enable verbose logging
/// * `apply_fixups` - Whether to apply fixups (true) or preview (false)
/// * `config` - The loaded xchecker configuration
/// * `cli_args` - CLI arguments passed by the user
/// * `problem_statement` - Optional problem statement to include in phase prompts
pub(super) fn build_orchestrator_config(
    dry_run: bool,
    verbose: bool,
    apply_fixups: bool,
    config: &Config,
    cli_args: &CliArgs,
    problem_statement: Option<&str>,
    redactor: Arc<SecretRedactor>,
) -> OrchestratorConfig {
    let mut config_map = create_default_config(verbose, config, cli_args);
    config_map.insert("logger_enabled".to_string(), verbose.to_string());
    config_map.insert("apply_fixups".to_string(), apply_fixups.to_string());

    // Include problem statement in config for prompt construction (FR-PKT)
    if let Some(ps) = problem_statement {
        config_map.insert("problem_statement".to_string(), ps.to_string());
    }

    OrchestratorConfig {
        dry_run,
        config: config_map,
        full_config: Some(config.clone()),
        selectors: Some(config.selectors.clone()),
        strict_validation: config.strict_validation(),
        redactor,
        hooks: Some(config.hooks.clone()),
    }
}

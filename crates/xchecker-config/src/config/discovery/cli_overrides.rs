use super::super::{CliArgs, ConfigSource};
use super::state::DiscoveryState;

pub(super) fn apply_cli_overrides(state: &mut DiscoveryState, cli_args: &CliArgs) {
    apply_default_overrides(state, cli_args);
    apply_runner_overrides(state, cli_args);
    apply_security_overrides(state, cli_args);
    apply_llm_binary_overrides(state, cli_args);
}

fn apply_default_overrides(state: &mut DiscoveryState, cli_args: &CliArgs) {
    if let Some(model) = &cli_args.model {
        state.defaults.model = Some(model.clone());
        state.set_source("model", ConfigSource::Cli);
    }
    if let Some(max_turns) = cli_args.max_turns {
        state.defaults.max_turns = Some(max_turns);
        state.set_source("max_turns", ConfigSource::Cli);
    }
    if let Some(packet_max_bytes) = cli_args.packet_max_bytes {
        state.defaults.packet_max_bytes = Some(packet_max_bytes);
        state.set_source("packet_max_bytes", ConfigSource::Cli);
    }
    if let Some(packet_max_lines) = cli_args.packet_max_lines {
        state.defaults.packet_max_lines = Some(packet_max_lines);
        state.set_source("packet_max_lines", ConfigSource::Cli);
    }
    if let Some(output_format) = &cli_args.output_format {
        state.defaults.output_format = Some(output_format.clone());
        state.set_source("output_format", ConfigSource::Cli);
    }
    if let Some(verbose) = cli_args.verbose {
        state.defaults.verbose = Some(verbose);
        state.set_source("verbose", ConfigSource::Cli);
    }
    if let Some(phase_timeout) = cli_args.phase_timeout {
        state.defaults.phase_timeout = Some(phase_timeout);
        state.set_source("phase_timeout", ConfigSource::Cli);
    }
    if let Some(stdout_cap_bytes) = cli_args.stdout_cap_bytes {
        state.defaults.stdout_cap_bytes = Some(stdout_cap_bytes);
        state.set_source("stdout_cap_bytes", ConfigSource::Cli);
    }
    if let Some(stderr_cap_bytes) = cli_args.stderr_cap_bytes {
        state.defaults.stderr_cap_bytes = Some(stderr_cap_bytes);
        state.set_source("stderr_cap_bytes", ConfigSource::Cli);
    }
    if let Some(lock_ttl_seconds) = cli_args.lock_ttl_seconds {
        state.defaults.lock_ttl_seconds = Some(lock_ttl_seconds);
        state.set_source("lock_ttl_seconds", ConfigSource::Cli);
    }
    if cli_args.debug_packet {
        state.defaults.debug_packet = Some(true);
        state.set_source("debug_packet", ConfigSource::Cli);
    }
    if cli_args.allow_links {
        state.defaults.allow_links = Some(true);
        state.set_source("allow_links", ConfigSource::Cli);
    }
    if let Some(strict_validation) = cli_args.strict_validation {
        state.defaults.strict_validation = Some(strict_validation);
        state.set_source("strict_validation", ConfigSource::Cli);
    }
}

fn apply_runner_overrides(state: &mut DiscoveryState, cli_args: &CliArgs) {
    if let Some(runner_mode) = &cli_args.runner_mode {
        state.runner.mode = Some(runner_mode.clone());
        state.set_source("runner_mode", ConfigSource::Cli);
    }
    if let Some(runner_distro) = &cli_args.runner_distro {
        state.runner.distro = Some(runner_distro.clone());
        state.set_source("runner_distro", ConfigSource::Cli);
    }
    if let Some(claude_path) = &cli_args.claude_path {
        state.runner.claude_path = Some(claude_path.clone());
        state.set_source("claude_path", ConfigSource::Cli);
    }
}

fn apply_security_overrides(state: &mut DiscoveryState, cli_args: &CliArgs) {
    if !cli_args.extra_secret_pattern.is_empty() {
        state
            .security
            .extra_secret_patterns
            .extend(cli_args.extra_secret_pattern.clone());
        state.set_source("security", ConfigSource::Cli);
    }
    if !cli_args.ignore_secret_pattern.is_empty() {
        state
            .security
            .ignore_secret_patterns
            .extend(cli_args.ignore_secret_pattern.clone());
        state.set_source("security", ConfigSource::Cli);
    }
}

fn apply_llm_binary_overrides(state: &mut DiscoveryState, cli_args: &CliArgs) {
    if let Some(binary) = &cli_args.llm_claude_binary {
        state.ensure_claude().binary = Some(binary.clone());
        state.set_source("llm_claude_binary", ConfigSource::Cli);
    }

    if let Some(binary) = &cli_args.llm_gemini_binary {
        state.ensure_gemini().binary = Some(binary.clone());
        state.set_source("llm_gemini_binary", ConfigSource::Cli);
    }
}

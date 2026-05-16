//! Lockfile version detection and drift checks for CLI execution flows.

use anyhow::{Context, Result};

use crate::Config;

/// Detect Claude CLI version by running `claude --version`
pub(super) fn detect_claude_cli_version() -> Result<String> {
    use crate::runner::CommandSpec;

    let output = CommandSpec::new("claude")
        .arg("--version")
        .to_command()
        .output()
        .context("Failed to execute 'claude --version'")?;

    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "claude --version exited with non-zero status"
        ));
    }

    let version_str = String::from_utf8(output.stdout)
        .context("Failed to parse claude --version output as UTF-8")?;

    // Parse version from output (format: "claude 0.8.1" or similar)
    let version = version_str
        .split_whitespace()
        .last()
        .ok_or_else(|| anyhow::anyhow!("Failed to parse version from output"))?
        .to_string();

    Ok(version)
}

pub(super) fn build_flow_context(config: &Config) -> crate::lock::FlowContext {
    let execution_strategy = config
        .llm
        .execution_strategy
        .as_deref()
        .unwrap_or("controlled");
    let provider = config.llm.provider.as_deref().unwrap_or("unspecified");
    let prompt_template = config.llm.prompt_template.as_deref().unwrap_or("default");
    let model = config.defaults.model.as_deref().unwrap_or("haiku");

    crate::lock::FlowContext {
        flow_version: format!("xchecker-kernel@{}", env!("CARGO_PKG_VERSION")),
        stage_graph_version: "requirements-design-tasks-review-fixup-final.v1".to_string(),
        gate_set_version: "xchecker-default-gates.v1".to_string(),
        prompt_pack_version: format!("builtin:{prompt_template}"),
        adapter_version: format!("strategy={execution_strategy};provider={provider}"),
        provider_policy: format!("provider={provider};model={model}"),
        tool_context_version: "builtin-tool-context.v1".to_string(),
    }
}

/// Check for lockfile drift and warn or fail based on `strict_lock` flag
pub(super) fn check_lockfile_drift(
    spec_id: &str,
    strict_lock: bool,
    model_full_name: &str,
    claude_cli_version: &str,
    config: &Config,
) -> Result<Option<crate::types::LockDrift>> {
    use crate::lock::{FlowLock, RunContext, XCheckerLock};

    // Try to load lockfile
    let lock = match XCheckerLock::load(spec_id) {
        Ok(Some(lock)) => Some(lock),
        Ok(None) => None,
        Err(e) => {
            eprintln!("⚠ Warning: Failed to load lockfile: {e}");
            None
        }
    };

    let legacy_drift = if let Some(lock) = lock {
        let context = RunContext {
            model_full_name: model_full_name.to_string(),
            claude_cli_version: claude_cli_version.to_string(),
            schema_version: "1".to_string(),
        };
        lock.detect_drift(&context)
    } else {
        None
    };

    let flow_drift = match FlowLock::load(spec_id) {
        Ok(Some(lock)) => lock.detect_drift(&build_flow_context(config)),
        Ok(None) => None,
        Err(e) => {
            eprintln!("⚠ Warning: Failed to load flow lock: {e}");
            None
        }
    };

    if let Some(drift) = &legacy_drift {
        eprintln!("\n⚠ Lockfile drift detected for spec '{spec_id}':");

        if let Some(model_drift) = &drift.model_full_name {
            eprintln!("  Model: {} → {}", model_drift.locked, model_drift.current);
        }

        if let Some(cli_drift) = &drift.claude_cli_version {
            eprintln!("  Claude CLI: {} → {}", cli_drift.locked, cli_drift.current);
        }

        if let Some(schema_drift) = &drift.schema_version {
            eprintln!(
                "  Schema: {} → {}",
                schema_drift.locked, schema_drift.current
            );
        }
    }

    if let Some(drift) = &flow_drift {
        eprintln!("\n⚠ Flow lock drift detected for spec '{spec_id}':");

        if let Some(flow_version) = &drift.flow_version {
            eprintln!(
                "  Flow version: {} → {}",
                flow_version.locked, flow_version.current
            );
        }
        if let Some(stage_graph) = &drift.stage_graph_version {
            eprintln!(
                "  Stage graph: {} → {}",
                stage_graph.locked, stage_graph.current
            );
        }
        if let Some(gate_set) = &drift.gate_set_version {
            eprintln!("  Gate set: {} → {}", gate_set.locked, gate_set.current);
        }
        if let Some(prompt_pack) = &drift.prompt_pack_version {
            eprintln!(
                "  Prompt pack: {} → {}",
                prompt_pack.locked, prompt_pack.current
            );
        }
        if let Some(adapter) = &drift.adapter_version {
            eprintln!("  Adapter: {} → {}", adapter.locked, adapter.current);
        }
        if let Some(provider_policy) = &drift.provider_policy {
            eprintln!(
                "  Provider policy: {} → {}",
                provider_policy.locked, provider_policy.current
            );
        }
        if let Some(tool_context) = &drift.tool_context_version {
            eprintln!(
                "  Tool context: {} → {}",
                tool_context.locked, tool_context.current
            );
        }
    }

    if strict_lock && (legacy_drift.is_some() || flow_drift.is_some()) {
        eprintln!("\n✗ Strict lock mode enabled: failing due to drift");
        eprintln!("  To proceed, either:");
        eprintln!(
            "    - Update the lockfiles: rm .xchecker/specs/{spec_id}/lock.json .xchecker/specs/{spec_id}/flow.lock && xchecker init {spec_id} --create-lock"
        );
        eprintln!("    - Remove --strict-lock flag to allow drift with warning");

        return Err(anyhow::anyhow!("Lock drift detected in strict mode"));
    }

    if legacy_drift.is_some() || flow_drift.is_some() {
        eprintln!("\n  Continuing with drift (use --strict-lock to fail on drift)");
    }

    Ok(legacy_drift)
}

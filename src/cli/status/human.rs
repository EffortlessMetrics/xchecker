use std::collections::HashMap;

use anyhow::{Context, Result};
use camino::Utf8Path;

use crate::types::Receipt;
use crate::{Config, OrchestratorHandle, PhaseId};

use super::{ALL_PHASES, fixups};

/// Emit human-readable status output.
pub(super) fn execute_status_human(
    spec_id: &str,
    handle: &OrchestratorHandle,
    config: &Config,
    base_path: &Utf8Path,
) -> Result<()> {
    println!("Status for spec: {spec_id}");
    println!("  Directory: {base_path}");

    let latest_completed = display_latest_completed_phase(handle);
    display_artifacts(handle)?;
    display_last_receipt(handle, base_path)?;
    display_effective_configuration(config);
    display_flow_lock(spec_id, config);
    display_partial_and_completed_phases(handle);
    fixups::check_and_display_fixup_targets(spec_id)?;
    display_resume_suggestions(spec_id, latest_completed);

    Ok(())
}

fn display_latest_completed_phase(handle: &OrchestratorHandle) -> Option<PhaseId> {
    let latest_completed = handle.artifact_manager().get_latest_completed_phase();
    match latest_completed {
        Some(phase) => println!("  Latest completed phase: {}", phase.as_str()),
        None => println!("  Latest completed phase: None"),
    }
    latest_completed
}

fn display_artifacts(handle: &OrchestratorHandle) -> Result<()> {
    let artifacts = handle
        .artifact_manager()
        .list_artifacts()
        .with_context(|| "Failed to list artifacts")?;

    if artifacts.is_empty() {
        println!("  Artifacts: None");
        return Ok(());
    }

    println!("  Artifacts: {} found", artifacts.len());
    let receipts = handle
        .receipt_manager()
        .list_receipts()
        .with_context(|| "Failed to list receipts")?;
    let artifact_hashes = artifact_hashes_by_filename(&receipts);

    for artifact in &artifacts {
        if let Some(hash) = artifact_hashes.get(artifact) {
            println!("    - {artifact} -> {hash}");
        } else {
            println!("    - {artifact} -> <no hash>");
        }
    }

    Ok(())
}

fn artifact_hashes_by_filename(receipts: &[Receipt]) -> HashMap<String, String> {
    let mut artifact_hashes = HashMap::new();
    for receipt in receipts {
        for output in &receipt.outputs {
            if let Some(filename) = output.path.split('/').next_back() {
                artifact_hashes.insert(filename.to_string(), first_8(&output.blake3_canonicalized));
            }
        }
    }
    artifact_hashes
}

fn first_8(value: &str) -> String {
    value.chars().take(8).collect()
}

fn display_last_receipt(handle: &OrchestratorHandle, base_path: &Utf8Path) -> Result<()> {
    let receipts = handle
        .receipt_manager()
        .list_receipts()
        .with_context(|| "Failed to list receipts")?;

    if receipts.is_empty() {
        println!("  Last receipt: None");
        return Ok(());
    }

    let latest_receipt = receipts.last().unwrap();
    let receipt_filename = format!(
        "{}-{}.json",
        latest_receipt.phase,
        latest_receipt.emitted_at.format("%Y%m%d_%H%M%S")
    );
    let receipt_path = base_path.join("receipts").join(receipt_filename);
    println!("  Last receipt: {receipt_path}");
    display_receipt_summary(latest_receipt);

    Ok(())
}

fn display_receipt_summary(receipt: &Receipt) {
    println!("    Phase: {}", receipt.phase);
    println!(
        "    Emitted at: {}",
        receipt.emitted_at.format("%Y-%m-%d %H:%M:%S UTC")
    );
    println!("    Exit code: {}", receipt.exit_code);
    println!("    Model: {}", receipt.model_full_name);
    if let Some(alias) = &receipt.model_alias {
        println!("    Model alias: {alias}");
    }
    println!("    Runner: {}", receipt.runner);
    if let Some(distro) = &receipt.runner_distro {
        println!("    Runner distro: {distro}");
    }
    println!("    Canonicalization: {}", receipt.canonicalization_version);

    if !receipt.warnings.is_empty() {
        println!("    Warnings: {}", receipt.warnings.len());
        for warning in &receipt.warnings {
            println!("      - {warning}");
        }
    }

    if receipt.fallback_used == Some(true) {
        println!("    Output format fallback: Used (stream-json → text)");
    }
}

fn display_effective_configuration(config: &Config) {
    println!("\n  Effective configuration:");
    for (key, (value, source)) in config.effective_config() {
        println!("    {key} = {value} (from {source})");
    }
}

fn display_flow_lock(spec_id: &str, config: &Config) {
    println!("\n  Governed flow:");
    match crate::lock::FlowLock::load(spec_id) {
        Ok(Some(flow_lock)) => {
            println!("    Flow lock: present");
            println!("    Flow version: {}", flow_lock.flow_version);
            println!("    Stage graph: {}", flow_lock.stage_graph_version);
            println!("    Gate set: {}", flow_lock.gate_set_version);
            println!("    Prompt pack: {}", flow_lock.prompt_pack_version);
            println!("    Adapter: {}", flow_lock.adapter_version);
            println!("    Provider policy: {}", flow_lock.provider_policy);
            display_flow_lock_drift(
                flow_lock.detect_drift(&super::super::build_flow_context(config)),
            );
        }
        Ok(None) => println!("    Flow lock: not created"),
        Err(e) => println!("    Flow lock: unreadable ({e})"),
    }
}

fn display_flow_lock_drift(drift: Option<crate::lock::FlowLockDrift>) {
    let Some(drift) = drift else {
        println!("    Drift: none");
        return;
    };

    println!("    Drift detected:");
    if let Some(flow_version) = drift.flow_version {
        println!(
            "      - Flow version: {} -> {}",
            flow_version.locked, flow_version.current
        );
    }
    if let Some(stage_graph) = drift.stage_graph_version {
        println!(
            "      - Stage graph: {} -> {}",
            stage_graph.locked, stage_graph.current
        );
    }
    if let Some(gate_set) = drift.gate_set_version {
        println!(
            "      - Gate set: {} -> {}",
            gate_set.locked, gate_set.current
        );
    }
    if let Some(prompt_pack) = drift.prompt_pack_version {
        println!(
            "      - Prompt pack: {} -> {}",
            prompt_pack.locked, prompt_pack.current
        );
    }
    if let Some(adapter) = drift.adapter_version {
        println!("      - Adapter: {} -> {}", adapter.locked, adapter.current);
    }
    if let Some(provider_policy) = drift.provider_policy {
        println!(
            "      - Provider policy: {} -> {}",
            provider_policy.locked, provider_policy.current
        );
    }
    if let Some(tool_context) = drift.tool_context_version {
        println!(
            "      - Tool context: {} -> {}",
            tool_context.locked, tool_context.current
        );
    }
}

fn display_partial_and_completed_phases(handle: &OrchestratorHandle) {
    let mut partial_phases = Vec::new();
    let mut completed_phases = Vec::new();

    for phase in ALL_PHASES {
        if handle.artifact_manager().has_partial_artifact(phase) {
            partial_phases.push(phase);
        }
        if handle.artifact_manager().phase_completed(phase) {
            completed_phases.push(phase);
        }
    }

    if !partial_phases.is_empty() {
        println!("\n  Partial artifacts found:");
        for phase in partial_phases {
            println!("    - {} (from failed execution)", phase.as_str());
        }
    }

    if !completed_phases.is_empty() {
        println!("\n  Completed phases:");
        for phase in completed_phases {
            println!("    - {}", phase.as_str());
        }
    }
}

fn display_resume_suggestions(spec_id: &str, latest_completed: Option<PhaseId>) {
    match latest_completed {
        Some(PhaseId::Requirements) => {
            println!("\n  Resume options:");
            println!("    - Continue to Design: xchecker resume {spec_id} --phase design");
            println!("    - Re-run Requirements: xchecker resume {spec_id} --phase requirements");
        }
        Some(PhaseId::Design) => {
            println!("\n  Resume options:");
            println!("    - Continue to Tasks: xchecker resume {spec_id} --phase tasks");
            println!("    - Re-run Design: xchecker resume {spec_id} --phase design");
        }
        Some(PhaseId::Tasks) => {
            println!("\n  Resume options:");
            println!("    - Continue to Review: xchecker resume {spec_id} --phase review");
            println!("    - Re-run Tasks: xchecker resume {spec_id} --phase tasks");
        }
        Some(PhaseId::Review) => {
            println!("\n  Resume options:");
            println!("    - Continue to Fixup: xchecker resume {spec_id} --phase fixup");
            println!("    - Re-run Review: xchecker resume {spec_id} --phase review");
        }
        Some(_) => {
            println!("\n  Resume options:");
            println!("    - Re-run any phase: xchecker resume {spec_id} --phase <phase_name>");
        }
        None => {
            println!("\n  Resume options:");
            println!(
                "    - Start from Requirements: xchecker resume {spec_id} --phase requirements"
            );
        }
    }
}

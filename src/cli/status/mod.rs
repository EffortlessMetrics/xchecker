use anyhow::{Context, Result};

use crate::{Config, OrchestratorHandle, PhaseId, emit_jcs};

mod fixups;
mod human;
mod json;

pub(crate) use fixups::count_pending_fixups_for_spec;

/// Emit status output as canonical JSON using JCS (RFC 8785)
/// Per FR-Claude Code-CLI (Requirements 4.1.2): Returns compact status summary
pub(super) fn emit_status_json(output: &crate::types::StatusJsonOutput) -> Result<String> {
    // Use emit_jcs from crate root for JCS canonicalization
    emit_jcs(output).context("Failed to emit status JSON")
}

/// Execute the status command.
pub(super) fn execute_status_command(spec_id: &str, as_json: bool, config: &Config) -> Result<()> {
    // Create read-only handle to access managers (no lock needed for status)
    let handle = OrchestratorHandle::readonly(spec_id)
        .with_context(|| format!("Failed to create orchestrator for spec: {spec_id}"))?;

    // Check if spec directory exists
    let base_path = handle.artifact_manager().base_path();
    if !base_path.exists() {
        print_missing_spec_status(spec_id, base_path.as_str(), as_json);
        return Ok(());
    }

    if as_json {
        json::execute_status_json(spec_id, &handle, config)
    } else {
        human::execute_status_human(spec_id, &handle, config, base_path)
    }
}

fn print_missing_spec_status(spec_id: &str, base_path: &str, as_json: bool) {
    if as_json {
        // Return empty JSON for non-existent spec
        println!("{{}}");
    } else {
        println!("Status for spec: {spec_id}");
        println!("  Status: No spec found");
        println!("  Directory: {base_path} (does not exist)");
    }
}

const ALL_PHASES: [PhaseId; 6] = [
    PhaseId::Requirements,
    PhaseId::Design,
    PhaseId::Tasks,
    PhaseId::Review,
    PhaseId::Fixup,
    PhaseId::Final,
];

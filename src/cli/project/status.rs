//! Workspace status collection and formatting for project commands.

use anyhow::{Context, Result};

use crate::emit_jcs;

/// Derive spec status from the latest receipt
///
/// Returns a human-readable status string based on the latest receipt:
/// - "success" if the latest receipt has exit_code 0
/// - "failed" if the latest receipt has non-zero exit_code
/// - "not_started" if no receipts exist
/// - "unknown" if receipts cannot be read
pub(crate) fn derive_spec_status(spec_id: &str) -> String {
    use crate::receipt::ReceiptManager;

    let base_path = crate::paths::spec_root(spec_id);
    let receipt_manager = ReceiptManager::new(&base_path);

    // Try to list all receipts for this spec
    match receipt_manager.list_receipts() {
        Ok(receipts) => {
            if receipts.is_empty() {
                "not_started".to_string()
            } else {
                // Get the latest receipt (list_receipts returns sorted by emitted_at)
                let latest = receipts.last().unwrap();
                if latest.exit_code == 0 {
                    // Include the phase name for more context
                    format!("{}: success", latest.phase)
                } else {
                    format!("{}: failed", latest.phase)
                }
            }
        }
        Err(_) => {
            // Check if the spec directory exists at all
            if base_path.exists() {
                "unknown".to_string()
            } else {
                "not_started".to_string()
            }
        }
    }
}

/// Execute the project status command
/// Per FR-WORKSPACE (Requirements 4.3.4): Emits aggregated status for all specs
pub(crate) fn execute_project_status_command(
    workspace_override: Option<&std::path::Path>,
    json: bool,
) -> Result<()> {
    use crate::receipt::ReceiptManager;
    use crate::types::{WorkspaceSpecStatus, WorkspaceStatusJsonOutput, WorkspaceStatusSummary};
    use crate::workspace::{self, Workspace};

    // Resolve workspace path
    let workspace_path = workspace::resolve_workspace(workspace_override)?.ok_or_else(|| {
        anyhow::anyhow!("No workspace found. Run 'xchecker project init <name>' first.")
    })?;

    // Load workspace
    let ws = Workspace::load(&workspace_path)?;

    // Collect status for each spec
    let mut spec_statuses = Vec::new();
    let mut summary = WorkspaceStatusSummary {
        total_specs: ws.specs.len() as u32,
        successful_specs: 0,
        failed_specs: 0,
        pending_specs: 0,
        not_started_specs: 0,
        stale_specs: 0,
    };

    // Define stale threshold (7 days)
    let stale_threshold = chrono::Duration::days(7);
    let now = chrono::Utc::now();

    for spec in ws.list_specs() {
        let base_path = crate::paths::spec_root(&spec.id);
        let receipt_manager = ReceiptManager::new(&base_path);

        // Get receipts for this spec
        let receipts = receipt_manager.list_receipts().unwrap_or_default();

        // Determine spec status
        let (status, latest_phase, last_activity, has_errors) = if receipts.is_empty() {
            summary.not_started_specs += 1;
            ("not_started".to_string(), None, None, false)
        } else {
            let latest = receipts.last().unwrap();
            let last_activity_time = latest.emitted_at;
            let is_stale = now.signed_duration_since(last_activity_time) > stale_threshold;

            if is_stale {
                summary.stale_specs += 1;
            }

            if latest.exit_code == 0 {
                // Check if all phases are complete
                let all_phases_complete = receipts
                    .iter()
                    .any(|r| r.phase == "final" && r.exit_code == 0);
                if all_phases_complete {
                    summary.successful_specs += 1;
                    (
                        if is_stale { "stale" } else { "success" }.to_string(),
                        Some(latest.phase.clone()),
                        Some(last_activity_time),
                        false,
                    )
                } else {
                    summary.pending_specs += 1;
                    (
                        if is_stale { "stale" } else { "pending" }.to_string(),
                        Some(latest.phase.clone()),
                        Some(last_activity_time),
                        false,
                    )
                }
            } else {
                summary.failed_specs += 1;
                (
                    "failed".to_string(),
                    Some(latest.phase.clone()),
                    Some(last_activity_time),
                    true,
                )
            }
        };

        // Count pending fixups for this spec
        let pending_fixups = count_pending_fixups_for_spec(&spec.id);

        spec_statuses.push(WorkspaceSpecStatus {
            spec_id: spec.id.clone(),
            tags: spec.tags.clone(),
            status,
            latest_phase,
            last_activity,
            pending_fixups,
            has_errors,
        });
    }

    if json {
        // Emit JSON output
        let output = WorkspaceStatusJsonOutput {
            schema_version: "workspace-status-json.v1".to_string(),
            workspace_name: ws.name.clone(),
            workspace_path: workspace_path.display().to_string(),
            specs: spec_statuses,
            summary,
        };

        let json_output = emit_workspace_status_json(&output)?;
        println!("{json_output}");
    } else {
        // Human-readable output
        println!("Workspace: {}", ws.name);
        println!("Location: {}", workspace_path.display());
        println!();

        // Summary
        println!("Summary:");
        println!("  Total specs: {}", summary.total_specs);
        println!("  Successful: {}", summary.successful_specs);
        println!("  Failed: {}", summary.failed_specs);
        println!("  Pending: {}", summary.pending_specs);
        println!("  Not started: {}", summary.not_started_specs);
        if summary.stale_specs > 0 {
            println!("  Stale (>7 days): {}", summary.stale_specs);
        }
        println!();

        if spec_statuses.is_empty() {
            println!("No specs registered.");
            println!("\nAdd specs with: xchecker project add-spec <spec-id>");
        } else {
            println!("Specs:");
            for spec in &spec_statuses {
                let tags_str = if spec.tags.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", spec.tags.join(", "))
                };

                let phase_str = spec.latest_phase.as_deref().unwrap_or("-");
                let fixups_str = if spec.pending_fixups > 0 {
                    format!(" ({} fixups)", spec.pending_fixups)
                } else {
                    String::new()
                };

                // Format: spec-id (status, phase) [tags] (fixups)
                println!(
                    "  - {} ({}, {}){}{}",
                    spec.spec_id, spec.status, phase_str, tags_str, fixups_str
                );
            }
        }
    }

    Ok(())
}

/// Count pending fixups for a spec
fn count_pending_fixups_for_spec(spec_id: &str) -> u32 {
    crate::fixup::pending_fixups_for_spec(spec_id).targets
}

/// Emit workspace status output as canonical JSON using JCS (RFC 8785)
pub(crate) fn emit_workspace_status_json(
    output: &crate::types::WorkspaceStatusJsonOutput,
) -> Result<String> {
    // Use emit_jcs from crate root for JCS canonicalization
    emit_jcs(output).context("Failed to emit workspace status JSON")
}

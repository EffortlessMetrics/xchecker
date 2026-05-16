//! Project and workspace CLI command handlers.
//!
//! These handlers own workspace-oriented commands so `cli.rs` can focus on
//! argument parsing and top-level dispatch.

use anyhow::{Context, Result};

use super::ProjectCommands;
use super::json::{emit_workspace_history_json, emit_workspace_status_json};
use crate::XCheckerError;
use crate::error::ConfigError;
use crate::spec_id::sanitize_spec_id;

/// Derive spec status from the latest receipt
///
/// Returns a human-readable status string based on the latest receipt:
/// - "success" if the latest receipt has exit_code 0
/// - "failed" if the latest receipt has non-zero exit_code
/// - "not_started" if no receipts exist
/// - "unknown" if receipts cannot be read
pub(super) fn derive_spec_status(spec_id: &str) -> String {
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

/// Execute project/workspace management commands
pub(super) fn execute_project_command(cmd: ProjectCommands) -> Result<()> {
    use crate::workspace::{self, Workspace};

    match cmd {
        ProjectCommands::Init { name } => {
            let cwd = std::env::current_dir().context("Failed to get current directory")?;

            let workspace_path = workspace::init_workspace(&cwd, &name)?;

            println!("✓ Initialized workspace: {}", name);
            println!("  Created: {}", workspace_path.display());
            println!("\nNext steps:");
            println!("  - Add specs with: xchecker project add-spec <spec-id>");
            println!("  - List specs with: xchecker project list");

            Ok(())
        }
        ProjectCommands::AddSpec {
            spec_id,
            tag,
            force,
        } => {
            // Sanitize spec ID
            let sanitized_id = sanitize_spec_id(&spec_id).map_err(|e| {
                XCheckerError::Config(ConfigError::InvalidValue {
                    key: "spec_id".to_string(),
                    value: format!("{e}"),
                })
            })?;

            // Discover workspace
            let workspace_path = workspace::discover_workspace_from_cwd()?.ok_or_else(|| {
                anyhow::anyhow!("No workspace found. Run 'xchecker project init <name>' first.")
            })?;

            // Load workspace
            let mut ws = Workspace::load(&workspace_path)?;

            // Add spec
            ws.add_spec(&sanitized_id, tag.clone(), force)?;

            // Save workspace
            ws.save(&workspace_path)?;

            println!("✓ Added spec '{}' to workspace", sanitized_id);
            if !tag.is_empty() {
                println!("  Tags: {}", tag.join(", "));
            }

            Ok(())
        }
        ProjectCommands::List { workspace } => {
            // Resolve workspace path
            let workspace_path =
                workspace::resolve_workspace(workspace.as_deref())?.ok_or_else(|| {
                    anyhow::anyhow!("No workspace found. Run 'xchecker project init <name>' first.")
                })?;

            // Load workspace
            let ws = Workspace::load(&workspace_path)?;

            println!("Workspace: {}", ws.name);
            println!("Location: {}", workspace_path.display());
            println!();

            if ws.specs.is_empty() {
                println!("No specs registered.");
                println!("\nAdd specs with: xchecker project add-spec <spec-id>");
            } else {
                println!("Specs ({}):", ws.specs.len());
                for spec in ws.list_specs() {
                    // Derive status from latest receipt
                    let status = derive_spec_status(&spec.id);

                    let tags_str = if spec.tags.is_empty() {
                        String::new()
                    } else {
                        format!(" [{}]", spec.tags.join(", "))
                    };

                    // Format: spec-id (status) [tags]
                    println!("  - {} ({}){}", spec.id, status, tags_str);
                }
            }

            Ok(())
        }
        ProjectCommands::Status { workspace, json } => {
            execute_project_status_command(workspace.as_deref(), json)
        }
        ProjectCommands::History { spec_id, json } => {
            // Sanitize spec ID
            let sanitized_id = sanitize_spec_id(&spec_id).map_err(|e| {
                XCheckerError::Config(ConfigError::InvalidValue {
                    key: "spec_id".to_string(),
                    value: format!("{e}"),
                })
            })?;
            execute_project_history_command(&sanitized_id, json)
        }
        ProjectCommands::Tui { workspace } => execute_project_tui_command(workspace.as_deref()),
    }
}

/// Execute the project status command
/// Per FR-WORKSPACE (Requirements 4.3.4): Emits aggregated status for all specs
fn execute_project_status_command(
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
pub(super) fn count_pending_fixups_for_spec(spec_id: &str) -> u32 {
    crate::fixup::pending_fixups_for_spec(spec_id).targets
}

/// Execute the project history command
/// Per FR-WORKSPACE (Requirements 4.3.5): Emits timeline of phase progression
pub(super) fn execute_project_history_command(spec_id: &str, json: bool) -> Result<()> {
    use crate::receipt::ReceiptManager;
    use crate::types::{HistoryEntry, HistoryMetrics, WorkspaceHistoryJsonOutput};

    // Get spec base path
    let base_path = crate::paths::spec_root(spec_id);

    // Check if spec exists
    if !base_path.exists() {
        if json {
            // Return empty history for non-existent spec
            let output = WorkspaceHistoryJsonOutput {
                schema_version: "workspace-history-json.v1".to_string(),
                spec_id: spec_id.to_string(),
                timeline: vec![],
                metrics: HistoryMetrics {
                    total_executions: 0,
                    successful_executions: 0,
                    failed_executions: 0,
                    total_tokens_input: 0,
                    total_tokens_output: 0,
                    total_fixups: 0,
                    first_execution: None,
                    last_execution: None,
                },
            };
            let json_output = emit_workspace_history_json(&output)?;
            println!("{json_output}");
        } else {
            println!("History for spec: {spec_id}");
            println!("  Status: Spec not found");
            println!("  Directory: {} (does not exist)", base_path);
        }
        return Ok(());
    }

    // Load receipts
    let receipt_manager = ReceiptManager::new(&base_path);
    let receipts = receipt_manager.list_receipts().unwrap_or_default();

    // Build timeline from receipts
    let mut timeline: Vec<HistoryEntry> = Vec::new();
    let mut metrics = HistoryMetrics {
        total_executions: 0,
        successful_executions: 0,
        failed_executions: 0,
        total_tokens_input: 0,
        total_tokens_output: 0,
        total_fixups: 0,
        first_execution: None,
        last_execution: None,
    };

    for receipt in &receipts {
        let success = receipt.exit_code == 0;

        // Extract LLM metadata if available
        let (tokens_input, tokens_output, provider, model) = if let Some(ref llm) = receipt.llm {
            (
                llm.tokens_input,
                llm.tokens_output,
                llm.provider.clone(),
                llm.model_used.clone(),
            )
        } else {
            (None, None, None, Some(receipt.model_full_name.clone()))
        };

        // Count fixups for fixup phase
        let fixup_count = if receipt.phase == "fixup" && success {
            Some(receipt.outputs.len() as u32)
        } else {
            None
        };

        let entry = HistoryEntry {
            phase: receipt.phase.clone(),
            timestamp: receipt.emitted_at,
            exit_code: receipt.exit_code,
            success,
            tokens_input,
            tokens_output,
            fixup_count,
            model,
            provider,
        };

        // Update metrics
        metrics.total_executions += 1;
        if success {
            metrics.successful_executions += 1;
        } else {
            metrics.failed_executions += 1;
        }
        if let Some(ti) = tokens_input {
            metrics.total_tokens_input += ti;
        }
        if let Some(to) = tokens_output {
            metrics.total_tokens_output += to;
        }
        if let Some(fc) = fixup_count {
            metrics.total_fixups += fc;
        }

        // Track first and last execution
        if metrics.first_execution.is_none()
            || receipt.emitted_at < metrics.first_execution.unwrap()
        {
            metrics.first_execution = Some(receipt.emitted_at);
        }
        if metrics.last_execution.is_none() || receipt.emitted_at > metrics.last_execution.unwrap()
        {
            metrics.last_execution = Some(receipt.emitted_at);
        }

        timeline.push(entry);
    }

    // Sort timeline by timestamp (oldest first)
    timeline.sort_by_key(|e| e.timestamp);

    if json {
        let output = WorkspaceHistoryJsonOutput {
            schema_version: "workspace-history-json.v1".to_string(),
            spec_id: spec_id.to_string(),
            timeline,
            metrics,
        };
        let json_output = emit_workspace_history_json(&output)?;
        println!("{json_output}");
    } else {
        // Human-readable output
        println!("History for spec: {spec_id}");
        println!("Location: {}", base_path);
        println!();

        // Summary metrics
        println!("Summary:");
        println!("  Total executions: {}", metrics.total_executions);
        println!("  Successful: {}", metrics.successful_executions);
        println!("  Failed: {}", metrics.failed_executions);
        if metrics.total_tokens_input > 0 || metrics.total_tokens_output > 0 {
            println!(
                "  Total tokens: {} input, {} output",
                metrics.total_tokens_input, metrics.total_tokens_output
            );
        }
        if metrics.total_fixups > 0 {
            println!("  Total fixups applied: {}", metrics.total_fixups);
        }
        if let Some(first) = metrics.first_execution {
            println!(
                "  First execution: {}",
                first.format("%Y-%m-%d %H:%M:%S UTC")
            );
        }
        if let Some(last) = metrics.last_execution {
            println!("  Last execution: {}", last.format("%Y-%m-%d %H:%M:%S UTC"));
        }
        println!();

        if timeline.is_empty() {
            println!("No executions recorded.");
        } else {
            println!("Timeline ({} entries):", timeline.len());
            for entry in &timeline {
                let status_icon = if entry.success { "✓" } else { "✗" };
                let tokens_str = match (entry.tokens_input, entry.tokens_output) {
                    (Some(ti), Some(to)) => format!(" [{} in, {} out]", ti, to),
                    (Some(ti), None) => format!(" [{} in]", ti),
                    (None, Some(to)) => format!(" [{} out]", to),
                    (None, None) => String::new(),
                };
                let fixup_str = entry
                    .fixup_count
                    .map(|c| format!(" ({} fixups)", c))
                    .unwrap_or_default();

                println!(
                    "  {} {} {} (exit {}){}{}",
                    entry.timestamp.format("%Y-%m-%d %H:%M:%S"),
                    status_icon,
                    entry.phase,
                    entry.exit_code,
                    tokens_str,
                    fixup_str
                );
            }
        }
    }

    Ok(())
}

/// Execute the project TUI command
/// Per FR-WORKSPACE-TUI (Requirements 4.4.1, 4.4.2, 4.4.3): Interactive terminal UI
fn execute_project_tui_command(workspace_override: Option<&std::path::Path>) -> Result<()> {
    use crate::workspace;

    // Resolve workspace path
    let workspace_path = workspace::resolve_workspace(workspace_override)?.ok_or_else(|| {
        anyhow::anyhow!("No workspace found. Run 'xchecker project init <name>' first.")
    })?;

    // Run the TUI
    crate::tui::run_tui(&workspace_path)
}

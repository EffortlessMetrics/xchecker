//! Workspace history timeline collection and formatting for project commands.

use anyhow::{Context, Result};

use crate::emit_jcs;

/// Execute the project history command
/// Per FR-WORKSPACE (Requirements 4.3.5): Emits timeline of phase progression
pub(crate) fn execute_project_history_command(spec_id: &str, json: bool) -> Result<()> {
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

/// Emit workspace history output as canonical JSON using JCS (RFC 8785)
pub(crate) fn emit_workspace_history_json(
    output: &crate::types::WorkspaceHistoryJsonOutput,
) -> Result<String> {
    // Use emit_jcs from crate root for JCS canonicalization
    emit_jcs(output).context("Failed to emit workspace history JSON")
}

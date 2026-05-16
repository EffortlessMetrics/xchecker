mod history;
mod status;

use anyhow::{Context, Result};

use super::ProjectCommands;
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
fn derive_spec_status(spec_id: &str) -> String {
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
            status::execute_project_status_command(workspace.as_deref(), json)
        }
        ProjectCommands::History { spec_id, json } => {
            // Sanitize spec ID
            let sanitized_id = sanitize_spec_id(&spec_id).map_err(|e| {
                XCheckerError::Config(ConfigError::InvalidValue {
                    key: "spec_id".to_string(),
                    value: format!("{e}"),
                })
            })?;
            history::execute_project_history_command(&sanitized_id, json)
        }
        ProjectCommands::Tui { workspace } => execute_project_tui_command(workspace.as_deref()),
    }
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

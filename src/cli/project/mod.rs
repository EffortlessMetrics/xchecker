//! Project/workspace CLI command definitions and dispatch.

mod history;
mod status;
mod tui;

#[cfg(test)]
pub(crate) use history::{emit_workspace_history_json, execute_project_history_command};
#[cfg(test)]
pub(crate) use status::{derive_spec_status, emit_workspace_status_json};

use anyhow::{Context, Result};
use clap::Subcommand;
use std::path::PathBuf;

use crate::XCheckerError;
use crate::error::ConfigError;
use crate::spec_id::sanitize_spec_id;

/// Project/workspace management subcommands
#[derive(Subcommand)]
pub enum ProjectCommands {
    /// Initialize a new workspace in the current directory
    ///
    /// Creates a `workspace.yaml` file that marks this directory as a project root.
    /// The workspace can then be used to manage multiple specs.
    ///
    /// EXAMPLES:
    ///   xchecker project init my-project
    Init {
        /// Name for the workspace
        name: String,
    },

    /// Add a spec to the workspace
    ///
    /// Registers a spec in the workspace registry with optional tags.
    ///
    /// EXAMPLES:
    ///   xchecker project add-spec feature-auth
    ///   xchecker project add-spec feature-auth --tag backend --tag security
    AddSpec {
        /// Spec ID to add
        spec_id: String,

        /// Tags for categorization (can be specified multiple times)
        #[arg(long, short)]
        tag: Vec<String>,

        /// Force override if spec already exists
        #[arg(long)]
        force: bool,
    },

    /// List all specs in the workspace
    ///
    /// Shows all registered specs with their tags and status.
    ///
    /// EXAMPLES:
    ///   xchecker project list
    List {
        /// Path to workspace file (overrides discovery)
        #[arg(long)]
        workspace: Option<PathBuf>,
    },

    /// Show aggregated status for all specs in the workspace
    ///
    /// Displays status summary for all registered specs including phase summaries,
    /// counts of failed/pending/stale specs, and overall workspace health.
    ///
    /// EXAMPLES:
    ///   xchecker project status
    ///   xchecker project status --json
    Status {
        /// Path to workspace file (overrides discovery)
        #[arg(long)]
        workspace: Option<PathBuf>,

        /// Output status as JSON (workspace-status-json.v1 schema)
        #[arg(long)]
        json: bool,
    },

    /// Show history timeline for a spec
    ///
    /// Displays timeline of phase progression, timestamps, and selected metrics
    /// including LLM token usage and fixup counts.
    ///
    /// EXAMPLES:
    ///   xchecker project history feature-auth
    ///   xchecker project history feature-auth --json
    ///
    /// Per FR-WORKSPACE (Requirements 4.3.5): Emits timeline of phase progression
    History {
        /// Spec ID to show history for
        spec_id: String,

        /// Output history as JSON (workspace-history-json.v1 schema)
        #[arg(long)]
        json: bool,
    },

    /// Launch interactive terminal UI for workspace overview
    ///
    /// Displays an interactive TUI showing specs list with tags and status,
    /// latest receipt summary per selected spec, pending fixups, error counts,
    /// and stale specs. The TUI is read-only (no destructive operations).
    ///
    /// Navigation:
    ///   - Arrow keys or j/k: Move selection up/down
    ///   - Enter: View details for selected spec
    ///   - Esc: Go back / close details
    ///   - q: Quit
    ///
    /// EXAMPLES:
    ///   xchecker project tui
    ///
    /// Per FR-WORKSPACE-TUI (Requirements 4.4.1, 4.4.2, 4.4.3)
    Tui {
        /// Path to workspace file (overrides discovery)
        #[arg(long)]
        workspace: Option<PathBuf>,
    },
}

/// Execute project/workspace management commands
pub fn execute_project_command(cmd: ProjectCommands) -> Result<()> {
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
                    let status = status::derive_spec_status(&spec.id);

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
        ProjectCommands::Tui { workspace } => {
            tui::execute_project_tui_command(workspace.as_deref())
        }
    }
}

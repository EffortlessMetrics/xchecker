//! Project terminal UI command entrypoint.

use anyhow::Result;

/// Execute the project TUI command
/// Per FR-WORKSPACE-TUI (Requirements 4.4.1, 4.4.2, 4.4.3): Interactive terminal UI
pub(super) fn execute_project_tui_command(
    workspace_override: Option<&std::path::Path>,
) -> Result<()> {
    use crate::workspace;

    // Resolve workspace path
    let workspace_path = workspace::resolve_workspace(workspace_override)?.ok_or_else(|| {
        anyhow::anyhow!("No workspace found. Run 'xchecker project init <name>' first.")
    })?;

    // Run the TUI
    crate::tui::run_tui(&workspace_path)
}

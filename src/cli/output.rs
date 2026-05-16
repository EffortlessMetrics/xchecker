use anyhow::{Context, Result};
use crossterm::style::{Color, Stylize};
use std::io::IsTerminal;

/// Check if colored output should be used.
///
/// Returns true only if:
/// - stdout is a terminal (TTY)
/// - NO_COLOR environment variable is not set
fn use_color() -> bool {
    std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

/// Return a styled check mark (✓) if colors are enabled, otherwise plain.
pub(super) fn styled_check() -> String {
    if use_color() {
        format!("{}", "✓".with(Color::Green).bold())
    } else {
        "✓".to_string()
    }
}

/// Return a styled warning mark (⚠) if colors are enabled, otherwise plain.
pub(super) fn styled_warning() -> String {
    if use_color() {
        format!("{}", "⚠".with(Color::Yellow).bold())
    } else {
        "⚠".to_string()
    }
}

/// Return styled success text if colors are enabled, otherwise plain.
pub(super) fn styled_success(text: &str) -> String {
    if use_color() {
        format!("{}", text.with(Color::Green).bold())
    } else {
        text.to_string()
    }
}

/// Return styled info text (cyan) if colors are enabled, otherwise plain.
pub(super) fn styled_info(text: &str) -> String {
    if use_color() {
        format!("{}", text.with(Color::Cyan).bold())
    } else {
        text.to_string()
    }
}

/// Emit spec output as canonical JSON using JCS (RFC 8785)
pub(super) fn emit_spec_json(output: &crate::types::SpecOutput) -> Result<String> {
    crate::emit_jcs(output).context("Failed to emit spec JSON")
}

/// Emit status output as canonical JSON using JCS (RFC 8785)
/// Per FR-Claude Code-CLI (Requirements 4.1.2): Returns compact status summary
pub(super) fn emit_status_json(output: &crate::types::StatusJsonOutput) -> Result<String> {
    crate::emit_jcs(output).context("Failed to emit status JSON")
}

/// Emit resume output as canonical JSON using JCS (RFC 8785)
/// Per FR-Claude Code-CLI (Requirements 4.1.3): Returns resume context without full packet/artifacts
pub(super) fn emit_resume_json(output: &crate::types::ResumeJsonOutput) -> Result<String> {
    crate::emit_jcs(output).context("Failed to emit resume JSON")
}

/// Emit workspace status output as canonical JSON using JCS (RFC 8785)
pub(super) fn emit_workspace_status_json(
    output: &crate::types::WorkspaceStatusJsonOutput,
) -> Result<String> {
    crate::emit_jcs(output).context("Failed to emit workspace status JSON")
}

/// Emit workspace history output as canonical JSON using JCS (RFC 8785)
pub(super) fn emit_workspace_history_json(
    output: &crate::types::WorkspaceHistoryJsonOutput,
) -> Result<String> {
    crate::emit_jcs(output).context("Failed to emit workspace history JSON")
}

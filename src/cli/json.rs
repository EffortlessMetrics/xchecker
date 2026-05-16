//! Canonical JSON emitters for CLI response contracts.

use anyhow::{Context, Result};

use crate::emit_jcs;

/// Emit spec output as canonical JSON using JCS (RFC 8785)
pub(super) fn emit_spec_json(output: &crate::types::SpecOutput) -> Result<String> {
    emit_jcs(output).context("Failed to emit spec JSON")
}

/// Emit status output as canonical JSON using JCS (RFC 8785)
/// Per FR-Claude Code-CLI (Requirements 4.1.2): Returns compact status summary
pub(super) fn emit_status_json(output: &crate::types::StatusJsonOutput) -> Result<String> {
    emit_jcs(output).context("Failed to emit status JSON")
}

/// Emit resume output as canonical JSON using JCS (RFC 8785)
pub(super) fn emit_resume_json(output: &crate::types::ResumeJsonOutput) -> Result<String> {
    emit_jcs(output).context("Failed to emit resume JSON")
}

/// Emit workspace status output as canonical JSON using JCS (RFC 8785)
pub(super) fn emit_workspace_status_json(
    output: &crate::types::WorkspaceStatusJsonOutput,
) -> Result<String> {
    emit_jcs(output).context("Failed to emit workspace status JSON")
}

/// Emit workspace history output as canonical JSON using JCS (RFC 8785)
pub(super) fn emit_workspace_history_json(
    output: &crate::types::WorkspaceHistoryJsonOutput,
) -> Result<String> {
    emit_jcs(output).context("Failed to emit workspace history JSON")
}

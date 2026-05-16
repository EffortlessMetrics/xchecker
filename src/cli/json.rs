//! Canonical JSON emitters for CLI output contracts.
//!
//! Keeping these wrappers in one module makes the command handlers depend on a
//! single SRP boundary for JCS serialization and contextual error messages.

use anyhow::{Context, Result};

/// Emit spec output as canonical JSON using JCS (RFC 8785).
pub(super) fn emit_spec_json(output: &crate::types::SpecOutput) -> Result<String> {
    crate::emit_jcs(output).context("Failed to emit spec JSON")
}

/// Emit status output as canonical JSON using JCS (RFC 8785).
pub(super) fn emit_status_json(output: &crate::types::StatusJsonOutput) -> Result<String> {
    crate::emit_jcs(output).context("Failed to emit status JSON")
}

/// Emit resume output as canonical JSON using JCS (RFC 8785).
pub(super) fn emit_resume_json(output: &crate::types::ResumeJsonOutput) -> Result<String> {
    crate::emit_jcs(output).context("Failed to emit resume JSON")
}

/// Emit workspace status output as canonical JSON using JCS (RFC 8785).
pub(super) fn emit_workspace_status_json(
    output: &crate::types::WorkspaceStatusJsonOutput,
) -> Result<String> {
    crate::emit_jcs(output).context("Failed to emit workspace status JSON")
}

/// Emit workspace history output as canonical JSON using JCS (RFC 8785).
pub(super) fn emit_workspace_history_json(
    output: &crate::types::WorkspaceHistoryJsonOutput,
) -> Result<String> {
    crate::emit_jcs(output).context("Failed to emit workspace history JSON")
}

use anyhow::{Context, Result};

use crate::{Config, PhaseId, emit_jcs};

/// Emit spec output as canonical JSON using JCS (RFC 8785)
pub(crate) fn emit_spec_json(output: &crate::types::SpecOutput) -> Result<String> {
    emit_jcs(output).context("Failed to emit spec JSON")
}

/// Emit status output as canonical JSON using JCS (RFC 8785)
/// Per FR-Claude Code-CLI (Requirements 4.1.2): Returns compact status summary
pub(crate) fn emit_status_json(output: &crate::types::StatusJsonOutput) -> Result<String> {
    emit_jcs(output).context("Failed to emit status JSON")
}

/// Generate next steps hint for resume JSON output
pub(crate) fn generate_next_steps_hint(
    spec_id: &str,
    phase_id: PhaseId,
    current_inputs: &crate::types::CurrentInputs,
    _config: &Config,
) -> String {
    if !current_inputs.spec_exists {
        return format!(
            "Spec '{}' does not exist. Run 'xchecker spec {}' to create it first.",
            spec_id, spec_id
        );
    }

    let has_requirements = current_inputs
        .available_artifacts
        .iter()
        .any(|a| a.contains("requirements"));
    let has_design = current_inputs
        .available_artifacts
        .iter()
        .any(|a| a.contains("design"));
    let has_tasks = current_inputs
        .available_artifacts
        .iter()
        .any(|a| a.contains("tasks"));
    let has_review = current_inputs
        .available_artifacts
        .iter()
        .any(|a| a.contains("review"));

    match phase_id {
        PhaseId::Requirements => {
            "Run requirements phase to generate initial requirements from the problem statement."
                .to_string()
        }
        PhaseId::Design => {
            if has_requirements {
                "Run design phase to generate architecture and design from requirements."
                    .to_string()
            } else {
                format!(
                    "Requirements phase not completed. Run 'xchecker resume {} --phase requirements' first.",
                    spec_id
                )
            }
        }
        PhaseId::Tasks => {
            if has_design {
                "Run tasks phase to generate implementation tasks from design.".to_string()
            } else {
                format!(
                    "Design phase not completed. Run 'xchecker resume {} --phase design' first.",
                    spec_id
                )
            }
        }
        PhaseId::Review => {
            if has_tasks {
                "Run review phase to review and validate the generated spec.".to_string()
            } else {
                format!(
                    "Tasks phase not completed. Run 'xchecker resume {} --phase tasks' first.",
                    spec_id
                )
            }
        }
        PhaseId::Fixup => {
            if has_review {
                "Run fixup phase to apply any suggested changes from review.".to_string()
            } else {
                format!(
                    "Review phase not completed. Run 'xchecker resume {} --phase review' first.",
                    spec_id
                )
            }
        }
        PhaseId::Final => "Run final phase to complete the spec generation workflow.".to_string(),
    }
}

/// Emit resume output as canonical JSON using JCS (RFC 8785)
/// Per FR-Claude Code-CLI (Requirements 4.1.3): Returns resume context without full packet/artifacts
pub(crate) fn emit_resume_json(output: &crate::types::ResumeJsonOutput) -> Result<String> {
    emit_jcs(output).context("Failed to emit resume JSON")
}

/// Emit workspace status output as canonical JSON using JCS (RFC 8785)
pub(crate) fn emit_workspace_status_json(
    output: &crate::types::WorkspaceStatusJsonOutput,
) -> Result<String> {
    emit_jcs(output).context("Failed to emit workspace status JSON")
}

/// Emit workspace history output as canonical JSON using JCS (RFC 8785)
pub(crate) fn emit_workspace_history_json(
    output: &crate::types::WorkspaceHistoryJsonOutput,
) -> Result<String> {
    emit_jcs(output).context("Failed to emit workspace history JSON")
}

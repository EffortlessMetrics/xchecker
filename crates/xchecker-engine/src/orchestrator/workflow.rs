//! Workflow execution and rewind logic for multi-phase orchestration.
//!
//! This module handles sequencing phases, managing rewinds, and tracking
//! workflow-level state. It builds on top of the single-phase engine in
//! `phase_exec.rs`.

use anyhow::{Context, Result};

use crate::error::{PhaseError, XCheckerError};
use crate::fixup::FixupMode;
use crate::phase::{NextStep, Phase};
use crate::phases::{DesignPhase, FixupPhase, RequirementsPhase, ReviewPhase, TasksPhase};
use crate::types::{FileType, PhaseId, PipelineInfo};

use super::{OrchestratorConfig, PhaseOrchestrator};

/// Result of executing a complete workflow with rewind support.
///
/// Internal type for future workflow execution API that handles multi-phase
/// orchestration with rewind capability. Not currently exposed publicly.
#[allow(dead_code)] // Future-facing: used for workflow execution API
#[derive(Debug)]
pub(crate) struct WorkflowResult {
    /// Whether the entire workflow completed successfully
    pub success: bool,
    /// List of all phase executions (including rewinds)
    pub completed_phases: Vec<PhaseExecution>,
    /// Total number of rewinds that occurred
    pub total_rewinds: usize,
    /// Final error if the workflow failed
    pub final_error: Option<String>,
}

/// Information about a single phase execution within a workflow.
///
/// Internal type that tracks execution history for workflow orchestration.
/// Not currently exposed publicly.
#[allow(dead_code)] // Future-facing: used for workflow execution API
#[derive(Debug)]
pub(crate) struct PhaseExecution {
    /// The phase that was executed
    pub phase: PhaseId,
    /// Whether the phase completed successfully
    pub success: bool,
    /// Whether this phase triggered a rewind
    pub rewind_triggered: bool,
    /// The target phase for rewind (if any)
    pub rewind_target: Option<PhaseId>,
    /// Any error that occurred during execution
    pub error: Option<String>,
}

/// Result of executing a single phase with rewind information.
///
/// Internal type that extends `ExecutionResult` with rewind tracking for
/// workflow orchestration. Not currently exposed publicly.
#[allow(dead_code)] // Future-facing: used for workflow execution API
#[derive(Debug)]
pub(crate) struct PhaseExecutionResult {
    /// Whether the phase completed successfully
    pub success: bool,
    /// Whether this phase triggered a rewind
    pub rewind_triggered: bool,
    /// The target phase for rewind (if any)
    pub rewind_target: Option<PhaseId>,
    /// Any error that occurred during execution
    pub error: Option<String>,
}

impl PhaseOrchestrator {
    /// Execute the complete spec generation workflow with rewind support.
    ///
    /// Internal method for future multi-phase orchestration that handles
    /// the full Requirements → Design → Tasks → Review → Fixup → Final flow
    /// with automatic rewind support when phases request it.
    ///
    /// This is not part of the public API; currently phases are executed individually.
    #[allow(dead_code)] // Future-facing: used for complete workflow execution
    pub(crate) async fn execute_complete_workflow(
        &self,
        config: &OrchestratorConfig,
    ) -> Result<WorkflowResult> {
        let mut rewind_count = 0;
        const MAX_REWIND_COUNT: usize = 2;
        let mut execution_history = Vec::new();

        // Define the standard phase order
        let standard_phases = [
            PhaseId::Requirements,
            PhaseId::Design,
            PhaseId::Tasks,
            PhaseId::Review,
            PhaseId::Fixup,
            PhaseId::Final,
        ];

        let mut current_phase_index = 0;

        while current_phase_index < standard_phases.len() {
            let phase_id = standard_phases[current_phase_index];

            // Skip Final phase for now (not implemented)
            if phase_id == PhaseId::Final {
                break;
            }

            println!("Executing phase: {}", phase_id.as_str());

            let result = match self
                .execute_single_phase_with_rewind_support(phase_id, config)
                .await
            {
                Ok(result) => result,
                Err(e) => {
                    execution_history.push(PhaseExecution {
                        phase: phase_id,
                        success: false,
                        rewind_triggered: false,
                        rewind_target: None,
                        error: Some(e.to_string()),
                    });

                    return Ok(WorkflowResult {
                        success: false,
                        completed_phases: execution_history,
                        total_rewinds: rewind_count,
                        final_error: Some(e.to_string()),
                    });
                }
            };

            execution_history.push(PhaseExecution {
                phase: phase_id,
                success: result.success,
                rewind_triggered: result.rewind_triggered,
                rewind_target: result.rewind_target,
                error: result.error.clone(),
            });

            if !result.success {
                return Ok(WorkflowResult {
                    success: false,
                    completed_phases: execution_history,
                    total_rewinds: rewind_count,
                    final_error: result.error,
                });
            }

            // Handle rewind if requested
            if result.rewind_triggered {
                if rewind_count >= MAX_REWIND_COUNT {
                    return Ok(WorkflowResult {
                        success: false,
                        completed_phases: execution_history,
                        total_rewinds: rewind_count,
                        final_error: Some(format!(
                            "Maximum rewind count ({MAX_REWIND_COUNT}) exceeded"
                        )),
                    });
                }

                rewind_count += 1;

                if let Some(target_phase) = result.rewind_target {
                    // Find the target phase index
                    if let Some(target_index) =
                        standard_phases.iter().position(|&p| p == target_phase)
                    {
                        current_phase_index = target_index;
                        println!(
                            "Rewinding to phase: {} (rewind #{}/{})",
                            target_phase.as_str(),
                            rewind_count,
                            MAX_REWIND_COUNT
                        );
                        continue;
                    }
                    return Ok(WorkflowResult {
                        success: false,
                        completed_phases: execution_history,
                        total_rewinds: rewind_count,
                        final_error: Some(format!(
                            "Invalid rewind target: {}",
                            target_phase.as_str()
                        )),
                    });
                }
            }

            // Move to next phase
            current_phase_index += 1;
        }

        Ok(WorkflowResult {
            success: true,
            completed_phases: execution_history,
            total_rewinds: rewind_count,
            final_error: None,
        })
    }

    /// Execute a single phase with rewind support.
    ///
    /// Internal helper for workflow execution that handles phase execution
    /// and detects rewind requests from `NextStep` results.
    ///
    /// This is not part of the public API.
    #[allow(dead_code)] // Future-facing: used for phase execution with rewind
    async fn execute_single_phase_with_rewind_support(
        &self,
        phase_id: PhaseId,
        config: &OrchestratorConfig,
    ) -> Result<PhaseExecutionResult> {
        // Check dependencies first
        if !self.can_resume_from_phase(phase_id)? {
            return Err(XCheckerError::Phase(PhaseError::DependencyNotSatisfied {
                phase: phase_id.as_str().to_string(),
                dependency: "required previous phases".to_string(),
            })
            .into());
        }

        // Create the appropriate phase instance
        let execution_result = match phase_id {
            PhaseId::Requirements => {
                let phase = RequirementsPhase::new();
                self.execute_phase_with_next_step_handling(&phase, config)
                    .await?
            }
            PhaseId::Design => {
                let phase = DesignPhase::new();
                self.execute_phase_with_next_step_handling(&phase, config)
                    .await?
            }
            PhaseId::Tasks => {
                let phase = TasksPhase::new();
                self.execute_phase_with_next_step_handling(&phase, config)
                    .await?
            }
            PhaseId::Review => {
                let phase = ReviewPhase::new();
                self.execute_phase_with_next_step_handling(&phase, config)
                    .await?
            }
            PhaseId::Fixup => {
                // Determine fixup mode from configuration
                let apply_fixups = config
                    .config
                    .get("apply_fixups")
                    .is_some_and(|s| s == "true");

                let fixup_mode = if apply_fixups {
                    FixupMode::Apply
                } else {
                    FixupMode::Preview
                };

                let phase = FixupPhase::new_with_mode(fixup_mode);
                self.execute_phase_with_next_step_handling(&phase, config)
                    .await?
            }
            PhaseId::Final => {
                return Err(XCheckerError::Phase(PhaseError::InvalidTransition {
                    from: "current state".to_string(),
                    to: "Final phase not yet implemented".to_string(),
                })
                .into());
            }
        };

        Ok(execution_result)
    }

    /// Execute a phase and handle `NextStep` results.
    ///
    /// Internal helper that executes a phase and interprets the `NextStep` value
    /// from the phase result to determine if rewind is requested.
    ///
    /// This is not part of the public API.
    #[allow(dead_code)] // Future-facing: used for phase execution with next step handling
    async fn execute_phase_with_next_step_handling<P: Phase>(
        &self,
        phase: &P,
        config: &OrchestratorConfig,
    ) -> Result<PhaseExecutionResult> {
        let phase_id = phase.id();

        // ORC-002: Route through execute_phase_core for unified execution logic
        // Note: execute_phase_core may return an error for secret detection,
        // but workflow needs to return Ok(PhaseExecutionResult) with success=false
        let core = match self.execute_phase_core(phase, config).await {
            Ok(core_output) => core_output,
            Err(e) => {
                // Handle secret detection error specially for workflow
                if let Some(xchecker_err) = e.downcast_ref::<XCheckerError>()
                    && let XCheckerError::Phase(PhaseError::ExecutionFailed { phase: _, code }) =
                        xchecker_err
                    && *code == crate::exit_codes::codes::SECRET_DETECTED
                {
                    return Ok(PhaseExecutionResult {
                        success: false,
                        rewind_triggered: false,
                        rewind_target: None,
                        error: Some("Secret detected in packet".to_string()),
                    });
                }
                // Propagate other errors
                return Err(e);
            }
        };

        // Handle Claude CLI failure
        if core.claude_exit_code != 0 {
            return Ok(PhaseExecutionResult {
                success: false,
                rewind_triggered: false,
                rewind_target: None,
                error: Some(format!(
                    "Claude CLI failed with exit code: {}",
                    core.claude_exit_code
                )),
            });
        }

        // Workflow-specific behavior: Store artifacts directly (not staged)
        // Note: execute_phase_core already stored to .partial/ and promoted.
        // We need to re-store directly for workflow compatibility.
        let mut output_hashes = Vec::new();

        for artifact in &core.phase_result.artifacts {
            let _artifact_result = self
                .artifact_manager()
                .store_artifact(artifact)
                .with_context(|| format!("Failed to store artifact: {}", artifact.name))?;

            // Create file hash for receipt
            let file_type = if let Some(ext) = std::path::Path::new(&artifact.name).extension() {
                FileType::from_extension(ext.to_str().unwrap_or(""))
            } else {
                match artifact.artifact_type {
                    crate::artifact::ArtifactType::Markdown => FileType::Markdown,
                    crate::artifact::ArtifactType::CoreYaml => FileType::Yaml,
                    _ => FileType::Text,
                }
            };

            let file_hash = self
                .receipt_manager()
                .create_file_hash(
                    &format!("artifacts/{}", artifact.name),
                    &artifact.content,
                    file_type,
                    phase_id.as_str(),
                )
                .map_err(|e| {
                    XCheckerError::Phase(PhaseError::OutputValidationFailed {
                        phase: phase_id.as_str().to_string(),
                        reason: e.to_string(),
                    })
                })?;

            output_hashes.push(file_hash);
        }

        // Prepare receipt flags
        let mut flags = std::collections::HashMap::new();
        flags.insert("phase".to_string(), phase_id.as_str().to_string());

        // Extract rewind information from phase_result.next_step (FR-WORKFLOW)
        let (rewind_triggered, rewind_target) = match &core.phase_result.next_step {
            NextStep::Rewind { to } => {
                flags.insert("rewind_triggered".to_string(), "true".to_string());
                flags.insert("rewind_target".to_string(), to.as_str().to_string());
                (true, Some(*to))
            }
            NextStep::Continue => (false, None),
            NextStep::Complete => (false, None),
        };

        // Extract model information from claude_metadata
        let (model_alias, model_full_name) = if let Some(metadata) = &core.claude_metadata {
            (
                metadata.model_alias.clone(),
                metadata.model_full_name.clone(),
            )
        } else {
            (None, "haiku".to_string())
        };

        // Create receipt using core outputs
        let mut warnings = Vec::new();
        if let Some(warning) = &core.llm_fallback_warning {
            warnings.push(warning.clone());
        }

        let mut receipt = self.receipt_manager().create_receipt_with_redactor(
            config.redactor.as_ref(),
            self.spec_id(),
            phase_id,
            core.claude_exit_code,
            output_hashes,
            env!("CARGO_PKG_VERSION"),
            core.claude_metadata
                .as_ref()
                .map_or("0.8.1", |m| m.claude_cli_version.as_str()),
            &model_full_name,
            model_alias,
            flags,
            core.packet_evidence.clone(),
            None, // No stderr_tail for successful execution
            None, // No stderr_redacted for successful execution
            warnings,
            core.claude_metadata.as_ref().map(|m| m.fallback_used),
            core.claude_metadata
                .as_ref()
                .map_or("native", |m| m.runner.as_str()),
            core.claude_metadata
                .as_ref()
                .and_then(|m| m.runner_distro.clone()),
            None, // No error_kind for successful execution
            None, // No error_reason for successful execution
            None, // No diff_context
            Some(PipelineInfo {
                execution_strategy: Some("controlled".to_string()),
            }),
        );

        // Set LLM info from invocation result (V11+ multi-provider support)
        receipt.llm = core.llm_result.map(|r| r.into_llm_info());

        let _receipt_path = self
            .receipt_manager()
            .write_receipt(&receipt)
            .with_context(|| format!("Failed to write receipt for phase: {}", phase_id.as_str()))?;

        Ok(PhaseExecutionResult {
            success: true,
            rewind_triggered,
            rewind_target,
            error: None,
        })
    }
}

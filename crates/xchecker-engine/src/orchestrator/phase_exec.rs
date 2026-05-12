//! Single-phase execution, timeout handling, and receipt emission.
//!
//! This module contains phase execution code extracted from mod.rs.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::error::{PhaseError, XCheckerError};
use crate::exit_codes;
use crate::fixup::{FixupMode, FixupPhase};
use crate::packet::PacketBuilder;
use crate::phase::{Phase, PhaseContext};
use crate::phases::{DesignPhase, RequirementsPhase, ReviewPhase, TasksPhase};
use crate::status::artifact::{Artifact, ArtifactType};
use crate::types::{ErrorKind, PacketEvidence, PhaseId, PipelineInfo};

use super::llm::ClaudeExecutionMetadata;
use super::{OrchestratorConfig, PhaseOrchestrator, PhaseTimeout};

mod artifacts;
mod hooks;
mod llm_step;
mod packet;
mod receipts;

/// Result of executing a phase through the orchestrator.
///
/// Contains all information about the phase execution including
/// success status, artifacts created, and any errors encountered.
///
/// This is the primary result type returned by orchestrator execution methods
/// and is intended for CLI and Kiro callers to inspect execution outcomes.
///
/// # Fields
/// - `phase`: The phase that was executed
/// - `success`: Whether the phase completed successfully (exit_code == 0)
/// - `exit_code`: Exit code from the phase execution (0 = success, see `exit_codes` module)
/// - `artifact_paths`: Paths to artifacts that were created (empty on failure)
/// - `receipt_path`: Path to the receipt file (always present, even on failure)
/// - `error`: Human-readable error message if execution failed
#[derive(Debug)]
pub struct ExecutionResult {
    /// The phase that was executed
    pub phase: PhaseId,
    /// Whether the phase completed successfully
    pub success: bool,
    /// Exit code from the phase execution
    pub exit_code: i32,
    /// Paths to artifacts that were created
    pub artifact_paths: Vec<PathBuf>,
    /// Path to the receipt file
    pub receipt_path: Option<PathBuf>,
    /// Any error that occurred during execution
    pub error: Option<String>,
}

// ============================================================================
// ORC-002: Phase Core Execution Architecture
// ============================================================================
//
// The execute_phase function follows a 10-step execution sequence:
//
// 1. Remove stale .partial/ directories (FR-ORC-003, FR-ORC-007)
// 2. Validate transition and acquire exclusive lock
// 3. Build packet with phase context
// 4. Scan for secrets (blocks on detection)
// 5. Execute LLM invocation (or simulate in dry-run)
// 6. Handle Claude CLI failure (save partial, create failure receipt)
// 7. Postprocess Claude response into artifacts
// 8. Write partial artifacts to .partial/ staging directory (FR-ORC-004)
// 9. Promote staged artifacts to final location (atomic rename)
// 10. Create and write receipt with cryptographic hashes (FR-ORC-005, FR-ORC-006)
//
// Future Refactoring: execute_phase_core
// ---------------------------------------
// To reduce duplication between phase_exec.rs and workflow.rs, a future
// execute_phase_core function will extract steps 3-10 into a reusable helper.
// This function will return PhaseCoreOutput containing all intermediate values
// needed for receipt generation and artifact management.
//
// The separation will look like:
//   execute_phase (current)      → High-level orchestration + timeout handling
//   execute_phase_core (future)  → Core execution logic (packet → receipt)
//
// Benefits:
// - Eliminates duplicate receipt/artifact handling logic
// - Makes testing easier (can test core logic independently)
// - Simplifies workflow.rs implementation
// - Maintains backward compatibility with existing tests
//
// PhaseCoreOutput Structure
// --------------------------
// The PhaseCoreOutput struct captures intermediate values generated during
// phase execution. These values are needed for:
// - Receipt generation (packet_evidence, claude_metadata, llm_result)
// - Error handling (claude_exit_code)
// - Artifact content (phase_result)
//
// Fields:
// - packet_evidence: PacketEvidence - Metadata about files included in packet
// - claude_exit_code: i32 - Exit code from LLM execution (0 = success)
// - claude_metadata: Option<ClaudeExecutionMetadata> - LLM execution details
// - llm_result: Option<LlmResult> - Structured LLM result for receipt
// - phase_result: PhaseResult - Postprocessed artifacts from response
//
// Removed fields (v1.0 cleanup - not consumed by callers):
// - phase_id, prompt, claude_response, artifact_paths, output_hashes, atomic_write_warnings
//
// This struct is returned by execute_phase_core and consumed by workflow.rs
// for receipt generation.

/// Captures intermediate values generated during core phase execution.
///
/// This structure is used by `execute_phase_core` (ORC-002) to return values
/// needed for receipt generation and artifact management in workflow execution.
///
/// # Usage
///
/// ```ignore
/// let core_output = execute_phase_core(phase, config).await?;
///
/// // Create receipt from core output
/// let receipt = receipt_manager.create_receipt(
///     spec_id,
///     phase_id,  // from phase.id()
///     core_output.claude_exit_code,
///     // ... other fields from core_output
/// );
/// ```
///
/// # Fields
///
/// - `packet_evidence`: Metadata about files included in the LLM packet
/// - `claude_exit_code`: Exit code from LLM (0 = success, non-zero = failure)
/// - `claude_metadata`: Optional execution metadata (model, version, runner info)
/// - `llm_result`: Structured LLM result that converts to `LlmInfo` on receipts
/// - `phase_result`: Postprocessed artifacts with parsed content
///
/// # Removed Fields (v1.0 cleanup)
///
/// The following fields were removed as they are not consumed by callers:
/// - `phase_id`: Callers use `phase.id()` directly
/// - `prompt`: Only used internally during execution
/// - `claude_response`: Only used internally for postprocessing
/// - `artifact_paths`: Callers re-store artifacts directly
/// - `output_hashes`: Callers re-compute hashes
/// - `atomic_write_warnings`: Not used by workflow execution
pub(crate) struct PhaseCoreOutput {
    /// Metadata about files included in the LLM packet
    pub packet_evidence: PacketEvidence,
    /// Exit code from LLM execution (0 = success, non-zero = failure)
    pub claude_exit_code: i32,
    /// Optional LLM execution metadata (model, version, runner info, stderr)
    pub claude_metadata: Option<ClaudeExecutionMetadata>,
    /// Structured LLM result that converts to LlmInfo on receipts
    pub llm_result: Option<crate::llm::LlmResult>,
    /// Warning message when a fallback provider was used
    pub llm_fallback_warning: Option<String>,
    /// Postprocessed artifacts with parsed content from LLM response
    pub phase_result: xchecker_phase_api::PhaseResult,
}

/// Execute a phase with timeout enforcement
pub(crate) async fn execute_phase_with_timeout<F, T>(
    fut: F,
    phase_id: PhaseId,
    timeout_config: &PhaseTimeout,
) -> Result<T, XCheckerError>
where
    F: std::future::Future<Output = Result<T>>,
{
    match tokio::time::timeout(timeout_config.duration, fut).await {
        Ok(result) => result.map_err(|e| {
            // Convert anyhow::Error to XCheckerError
            // Try to downcast to XCheckerError first
            match e.downcast::<XCheckerError>() {
                Ok(xchecker_err) => xchecker_err,
                Err(_original_err) => {
                    // If it's not an XCheckerError, wrap it as a generic phase error
                    XCheckerError::Phase(PhaseError::ExecutionFailed {
                        phase: phase_id.as_str().to_string(),
                        code: 1,
                    })
                }
            }
        }),
        Err(_) => {
            // Timeout occurred
            Err(XCheckerError::Phase(PhaseError::Timeout {
                phase: phase_id.as_str().to_string(),
                timeout_seconds: timeout_config.duration.as_secs(),
            }))
        }
    }
}

impl PhaseOrchestrator {
    /// Execute the Requirements phase end-to-end with timeout.
    ///
    /// This is the primary entry point for generating requirements from a spec.
    /// Validates the phase transition, builds a packet, invokes the LLM,
    /// and stores the resulting artifacts and receipt.
    ///
    /// Used by CLI and Kiro flows; respects `dry_run` and LLM configuration.
    ///
    /// **Note**: Public for tests and advanced integrations; prefer `OrchestratorHandle`
    /// for general use. May be narrowed to `pub(crate)` in a future version.
    ///
    /// Reserved for future orchestration API; not currently used by CLI.
    ///
    /// # Errors
    /// Returns error if transition is invalid or execution fails.
    #[cfg_attr(not(test), allow(dead_code))]
    pub async fn execute_requirements_phase(
        &self,
        config: &OrchestratorConfig,
    ) -> Result<ExecutionResult> {
        // Validate transition before execution (FR-ORC-001)
        self.validate_transition(PhaseId::Requirements)?;

        let phase = self.get_phase_impl(PhaseId::Requirements, config)?;
        self.execute_phase_with_timeout_handling(phase.as_ref(), config)
            .await
    }

    /// Execute the Design phase end-to-end with timeout.
    ///
    /// Generates architecture and design documents based on requirements.
    /// Validates the phase transition and dependencies before execution.
    ///
    /// Used by CLI and Kiro flows; respects `dry_run` and LLM configuration.
    ///
    /// **Note**: Public for tests and advanced integrations; prefer `OrchestratorHandle`
    /// for general use. May be narrowed to `pub(crate)` in a future version.
    ///
    /// # Errors
    /// Returns error if transition is invalid or execution fails.
    #[allow(dead_code)] // Phase execution method for CLI/library usage
    pub async fn execute_design_phase(
        &self,
        config: &OrchestratorConfig,
    ) -> Result<ExecutionResult> {
        // Validate transition before execution (FR-ORC-001)
        self.validate_transition(PhaseId::Design)?;

        let phase = self.get_phase_impl(PhaseId::Design, config)?;
        self.execute_phase_with_timeout_handling(phase.as_ref(), config)
            .await
    }

    /// Execute the Tasks phase end-to-end with timeout.
    ///
    /// Generates implementation tasks and milestones based on design.
    /// Validates the phase transition and dependencies before execution.
    ///
    /// Used by CLI and Kiro flows; respects `dry_run` and LLM configuration.
    ///
    /// **Note**: Public for tests and advanced integrations; prefer `OrchestratorHandle`
    /// for general use. May be narrowed to `pub(crate)` in a future version.
    ///
    /// # Errors
    /// Returns error if transition is invalid or execution fails.
    #[allow(dead_code)] // Phase execution method for CLI/library usage
    pub async fn execute_tasks_phase(
        &self,
        config: &OrchestratorConfig,
    ) -> Result<ExecutionResult> {
        // Validate transition before execution (FR-ORC-001)
        self.validate_transition(PhaseId::Tasks)?;

        let phase = self.get_phase_impl(PhaseId::Tasks, config)?;
        self.execute_phase_with_timeout_handling(phase.as_ref(), config)
            .await
    }

    /// Resume execution from a specific phase.
    ///
    /// Validates that all dependencies are satisfied before executing.
    /// Use this to continue a workflow from any valid phase.
    ///
    /// # Arguments
    /// * `phase_id` - The phase to resume from
    /// * `config` - Execution configuration
    ///
    /// # Errors
    /// Returns error if dependencies are not satisfied or execution fails.
    pub async fn resume_from_phase(
        &self,
        phase_id: PhaseId,
        config: &OrchestratorConfig,
    ) -> Result<ExecutionResult> {
        // Validate transition before execution (FR-ORC-001, FR-ORC-002)
        self.validate_transition(phase_id)?;

        // Use phase factory to get the appropriate phase implementation
        let phase = self.get_phase_impl(phase_id, config)?;
        self.execute_phase_with_resume(phase.as_ref(), config).await
    }

    /// Execute a phase with timeout handling
    pub(crate) async fn execute_phase_with_timeout_handling(
        &self,
        phase: &dyn Phase,
        config: &OrchestratorConfig,
    ) -> Result<ExecutionResult> {
        let phase_id = phase.id();

        // Get timeout configuration from config
        let timeout_config = PhaseTimeout::from_config(config);

        // Execute phase with timeout
        match execute_phase_with_timeout(
            self.execute_phase(phase, config),
            phase_id,
            &timeout_config,
        )
        .await
        {
            Ok(result) => Ok(result),
            Err(XCheckerError::Phase(PhaseError::Timeout {
                phase: _,
                timeout_seconds,
            })) => {
                // Handle timeout: write partial artifact and receipt with warning
                self.handle_phase_timeout(phase_id, timeout_seconds, config)
                    .await
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Handle phase timeout by writing partial artifact and receipt
    async fn handle_phase_timeout(
        &self,
        phase_id: PhaseId,
        timeout_seconds: u64,
        config: &OrchestratorConfig,
    ) -> Result<ExecutionResult> {
        // Create minimal partial artifact
        let partial_content = format!(
            "# {} Phase (Partial - Timeout)\n\nThis phase timed out after {} seconds.\n\nNo output was generated before the timeout occurred.\n",
            phase_id.as_str(),
            timeout_seconds
        );

        let partial_filename = format!(
            "{:02}-{}.partial.md",
            self.get_phase_number(phase_id),
            phase_id.as_str().to_lowercase()
        );

        // Store partial artifact
        let partial_artifact = Artifact {
            name: partial_filename,
            content: partial_content.clone(),
            artifact_type: ArtifactType::Partial,
            blake3_hash: blake3::hash(partial_content.as_bytes())
                .to_hex()
                .to_string(),
        };

        let partial_result = self.artifact_manager().store_artifact(&partial_artifact)?;
        let partial_path = partial_result.path;

        // Create receipt with timeout warning
        let packet_evidence = PacketEvidence {
            files: vec![],
            max_bytes: 65536,
            max_lines: 1200,
        };

        let mut flags = HashMap::new();
        flags.insert("phase".to_string(), phase_id.as_str().to_string());

        let warnings = vec![format!("phase_timeout:{}", timeout_seconds)];
        let pipeline_info = Some(PipelineInfo {
            execution_strategy: Some("controlled".to_string()),
        });

        // Use config values for truthful failure receipts (no hard-coded metadata)
        let configured_model = config.config.get("model").map_or("unknown", |s| s.as_str());
        let configured_runner = config
            .config
            .get("runner_mode")
            .map_or("unknown", |s| s.as_str());

        let receipt = self.receipt_manager().create_receipt_with_redactor(
            config.redactor.as_ref(),
            self.spec_id(),
            phase_id,
            exit_codes::codes::PHASE_TIMEOUT, // Exit code 10
            vec![],                           // No successful outputs
            env!("CARGO_PKG_VERSION"),
            "unknown", // Claude may have been running but we don't have metadata
            configured_model,
            None, // No model alias
            flags,
            packet_evidence,
            None, // No stderr_tail
            None, // No stderr_redacted
            warnings,
            None, // No fallback
            configured_runner,
            None, // No runner distro
            Some(ErrorKind::PhaseTimeout),
            Some(format!("Phase timed out after {timeout_seconds} seconds")),
            None, // No diff_context,
            pipeline_info,
        );

        let receipt_path = self.receipt_manager().write_receipt(&receipt)?;

        Ok(ExecutionResult {
            phase: phase_id,
            success: false,
            exit_code: exit_codes::codes::PHASE_TIMEOUT,
            artifact_paths: vec![partial_path.into_std_path_buf()],
            receipt_path: Some(receipt_path.into_std_path_buf()),
            error: Some(format!("Phase timed out after {timeout_seconds} seconds")),
        })
    }

    /// Execute a phase with resume support (handles partial artifacts)
    async fn execute_phase_with_resume(
        &self,
        phase: &dyn Phase,
        config: &OrchestratorConfig,
    ) -> Result<ExecutionResult> {
        let phase_id = phase.id();

        // Check if there's a partial artifact from a previous failed run
        let has_partial = self.artifact_manager().has_partial_artifact(phase_id);

        if has_partial {
            println!(
                "Found partial artifact for {} phase from previous failed run",
                phase_id.as_str()
            );

            if !config.dry_run {
                // In a real implementation, we might want to ask the user if they want to:
                // 1. Continue from partial (not implemented yet)
                // 2. Start fresh (delete partial and re-run)
                // For now, we'll delete the partial and start fresh
                println!("Deleting partial artifact and starting fresh...");
                self.artifact_manager().delete_partial_artifact(phase_id)?;
            }
        }

        // Execute the phase normally
        let result = self.execute_phase(phase, config).await?;

        // If successful and we had a partial, clean up any remaining partials
        if result.success {
            // Delete any partial artifacts on success (R4.5)
            if let Err(e) = self.artifact_manager().delete_partial_artifact(phase_id) {
                // Log warning but don't fail the operation
                eprintln!("Warning: Failed to clean up partial artifact: {e}");
            }
        }

        Ok(result)
    }

    /// Execute core phase logic: packet → LLM → artifacts (steps 1-8 from execute_phase).
    ///
    /// This function extracts the common execution flow shared between execute_phase
    /// and workflow execution. It performs:
    ///
    /// 1. Prompt generation
    /// 2. Packet creation with context
    /// 3. Secret scanning (returns early with error on detection)
    /// 4. Debug packet writing (if enabled)
    /// 5. LLM invocation (dry-run or real)
    /// 6. Postprocessing response into artifacts
    /// 7. Artifact staging to .partial/ + warnings collection
    /// 8. File hashing + artifact promotion to final location
    ///
    /// # Returns
    ///
    /// `PhaseCoreOutput` containing all intermediate values needed for receipt generation.
    ///
    /// # Errors
    ///
    /// Returns error if any core execution step fails. Note that LLM failures
    /// (non-zero exit codes) are NOT errors - they're captured in `claude_exit_code`.
    /// Early returns occur for:
    /// - Secret detection (XCheckerError::Phase with ErrorKind::SecretDetected)
    /// - Packet creation failures
    /// - Postprocessing failures (only if exit_code == 0)
    pub(crate) async fn execute_phase_core(
        &self,
        phase: &dyn Phase,
        config: &OrchestratorConfig,
    ) -> Result<PhaseCoreOutput> {
        let phase_id = phase.id();

        // Create phase context
        let phase_context = self.create_phase_context(phase_id, config)?;

        // Check dependencies
        self.check_phase_dependencies(phase)?;

        // Step 1: Generate prompt
        let prompt = phase.prompt(&phase_context);

        // Step 2: Build packet (FR-ORC-003)
        let packet = phase.make_packet(&phase_context).map_err(|e| {
            XCheckerError::Phase(PhaseError::PacketCreationFailed {
                phase: phase_id.as_str().to_string(),
                reason: e.to_string(),
            })
        })?;

        // Log packet hash and budget usage for visibility
        let budget = packet.budget_usage();
        tracing::info!(
            target: "xchecker::packet",
            spec_id = %self.spec_id(),
            phase = %phase_id.as_str(),
            packet_hash = %packet.hash(),
            bytes_used = budget.bytes_used,
            bytes_limit = budget.max_bytes,
            lines_used = budget.lines_used,
            lines_limit = budget.max_lines,
            "Built packet for phase"
        );

        let packet_evidence = packet.evidence.clone();

        // Step 3: Scan for secrets (FR-ORC-003, FR-SEC)
        let redactor = config.redactor.as_ref();

        // Check for secrets in the packet content - return error immediately if found
        if redactor.has_secrets(&packet.content, "packet")? {
            return Err(XCheckerError::Phase(PhaseError::ExecutionFailed {
                phase: phase_id.as_str().to_string(),
                code: exit_codes::codes::SECRET_DETECTED,
            })
            .into());
        }

        // Store packet for debugging/preview
        let _packet_preview_path = self
            .artifact_manager()
            .store_context_file(&format!("{}-packet", phase_id.as_str()), &packet.content)?;

        // Step 4: Write full debug packet if --debug-packet flag is set (FR-PKT-006, FR-PKT-007)
        // Only write after secret scan passes; file is excluded from receipts
        let debug_packet_enabled = config
            .config
            .get("debug_packet")
            .is_some_and(|s| s == "true");

        if debug_packet_enabled {
            // Get context directory from artifact manager
            let context_dir = self.artifact_manager().context_path();

            // Create a temporary PacketBuilder just to call write_debug_packet
            let temp_builder = PacketBuilder::new().map_err(|e| {
                XCheckerError::Phase(PhaseError::PacketCreationFailed {
                    phase: phase_id.as_str().to_string(),
                    reason: format!("Failed to create PacketBuilder for debug packet: {e}"),
                })
            })?;

            if let Err(e) =
                temp_builder.write_debug_packet(&packet.content, phase_id.as_str(), &context_dir)
            {
                // Log warning but don't fail the operation (debug packet is optional)
                eprintln!("Warning: Failed to write debug packet: {e}");
            }
        }

        // Step 5: Execute LLM (or simulate in dry-run mode)
        let (claude_response, claude_exit_code, claude_metadata, llm_result, llm_fallback_warning) =
            if config.dry_run {
                let simulated_llm = self.simulate_llm_result(phase_id);
                let simulated_metadata = super::llm::ClaudeExecutionMetadata {
                    model_alias: None,
                    model_full_name: "haiku".to_string(),
                    claude_cli_version: "0.8.1".to_string(),
                    fallback_used: false,
                    runner: "simulated".to_string(),
                    runner_distro: None,
                    stderr_tail: None,
                };
                (
                    self.simulate_claude_response(phase_id, &prompt),
                    0,
                    Some(simulated_metadata),
                    Some(simulated_llm),
                    None,
                )
            } else {
                // Use new LLM backend abstraction (V11: Claude CLI only)
                self.run_llm_invocation(&prompt, &packet.content, phase_id, config)
                    .await?
            };

        // Step 6: Postprocess Claude response (only if LLM succeeded)
        let phase_result = if claude_exit_code == 0 {
            phase
                .postprocess(&claude_response, &phase_context)
                .with_context(|| {
                    format!(
                        "Failed to postprocess response for phase: {}",
                        phase_id.as_str()
                    )
                })?
        } else {
            // For failed LLM runs, return empty result - caller will handle partial artifact
            xchecker_phase_api::PhaseResult {
                artifacts: vec![],
                next_step: xchecker_phase_api::NextStep::Continue,
                metadata: xchecker_phase_api::PhaseMetadata::default(),
            }
        };

        // Step 7: Write partial artifacts to .partial/ subdirectory (FR-ORC-004)
        for artifact in &phase_result.artifacts {
            // Store to .partial/ staging directory first
            let _partial_result = self
                .artifact_manager()
                .store_partial_staged_artifact(artifact)
                .with_context(|| format!("Failed to store partial artifact: {}", artifact.name))?;
        }

        // Step 8: Promote to final (atomic rename) (FR-ORC-004)
        for artifact in &phase_result.artifacts {
            let _final_path = self
                .artifact_manager()
                .promote_staged_to_final(&artifact.name)
                .with_context(|| {
                    format!("Failed to promote artifact to final: {}", artifact.name)
                })?;
        }

        // Return intermediate values for receipt generation
        // Note: Callers (workflow.rs) re-store artifacts and re-compute hashes,
        // so we don't return phase_id, artifact_paths, output_hashes, or atomic_write_warnings
        Ok(PhaseCoreOutput {
            packet_evidence,
            claude_exit_code,
            claude_metadata,
            llm_result,
            llm_fallback_warning,
            phase_result,
        })
    }

    /// Execute a single phase with full orchestration
    pub(crate) async fn execute_phase(
        &self,
        phase: &dyn Phase,
        config: &OrchestratorConfig,
    ) -> Result<ExecutionResult> {
        let phase_id = phase.id();

        self.artifact_manager()
            .remove_stale_partial_dir()
            .with_context(|| "Failed to remove stale .partial/ directory")?;

        let hooks = self.run_pre_phase_hook(phase_id, config).await?;
        if let Some(abort) = hooks.abort {
            return Ok(abort);
        }

        let phase_input = self.prepare_phase_packet(phase, config)?;

        if let Some(secret_result) = self.write_secret_detected_result(
            phase_id,
            config,
            phase_input.packet.evidence.clone(),
            &phase_input.packet.content,
        )? {
            return Ok(secret_result);
        }

        self.store_packet_context_files(phase_id, &phase_input.packet.content, config)?;

        let llm = match self
            .execute_llm_step(
                phase_id,
                config,
                &phase_input.prompt,
                &phase_input.packet.content,
                phase_input.packet.evidence.clone(),
            )
            .await?
        {
            llm_step::LlmStepOutcome::Completed(llm) => llm,
            llm_step::LlmStepOutcome::Failed(result) => return Ok(result),
        };

        if llm.exit_code != 0 {
            return self.write_nonzero_llm_result(
                phase_id,
                config,
                phase_input.packet.evidence.clone(),
                llm,
            );
        }

        let phase_result = phase
            .postprocess(&llm.response, &phase_input.context)
            .with_context(|| {
                format!(
                    "Failed to postprocess response for phase: {}",
                    phase_id.as_str()
                )
            })?;

        let stored_artifacts = self.store_success_artifacts(phase_id, &phase_result)?;
        let warnings = stored_artifacts
            .warnings
            .into_iter()
            .chain(hooks.warnings.into_iter())
            .collect();
        let receipt_path = self.write_success_result(
            phase_id,
            config,
            phase_input.packet.evidence,
            stored_artifacts.output_hashes,
            warnings,
            llm,
        )?;

        self.run_post_phase_hook(phase_id, config).await;

        Ok(ExecutionResult {
            phase: phase_id,
            success: true,
            exit_code: 0,
            artifact_paths: stored_artifacts.paths,
            receipt_path: Some(receipt_path),
            error: None,
        })
    }

    /// Create phase context for execution
    pub(crate) fn create_phase_context(
        &self,
        phase_id: PhaseId,
        config: &OrchestratorConfig,
    ) -> Result<PhaseContext> {
        // List available artifacts from previous phases
        let artifacts = self.artifact_manager().list_artifacts().map_err(|e| {
            XCheckerError::Phase(PhaseError::ContextCreationFailed {
                phase: phase_id.as_str().to_string(),
                reason: format!("Failed to list existing artifacts: {e}"),
            })
        })?;

        Ok(PhaseContext {
            spec_id: self.spec_id().to_string(),
            spec_dir: self
                .artifact_manager()
                .base_path()
                .clone()
                .into_std_path_buf(),
            config: config.config.clone(),
            artifacts,
            selectors: config.selectors.clone(),
            strict_validation: config.strict_validation,
            redactor: config.redactor.clone(),
        })
    }

    /// Check that phase dependencies are satisfied
    pub(crate) fn check_phase_dependencies(&self, phase: &dyn Phase) -> Result<()> {
        let deps = phase.deps();

        for dep_phase in deps {
            // Check if we have a successful receipt for the dependency
            if let Some(receipt) = self.receipt_manager().read_latest_receipt(*dep_phase)? {
                if receipt.exit_code != 0 {
                    return Err(XCheckerError::Phase(PhaseError::DependencyNotSatisfied {
                        phase: phase.id().as_str().to_string(),
                        dependency: dep_phase.as_str().to_string(),
                    })
                    .into());
                }
            } else {
                return Err(XCheckerError::Phase(PhaseError::DependencyNotSatisfied {
                    phase: phase.id().as_str().to_string(),
                    dependency: dep_phase.as_str().to_string(),
                })
                .into());
            }
        }

        Ok(())
    }

    /// Create a simulated LlmResult for dry-run mode
    /// This ensures receipts have complete LLM metadata even during testing
    pub(crate) fn simulate_llm_result(&self, _phase_id: PhaseId) -> crate::llm::LlmResult {
        crate::llm::LlmResult::new(
            "simulated response".to_string(),
            "claude-cli-simulated".to_string(),
            "haiku".to_string(),
        )
        .with_tokens(1000, 2000)
        .with_timeout(false)
        .with_timeout_seconds(600) // Default timeout for simulated runs
        .with_extension("dry_run", serde_json::json!(true))
    }

    /// Simulate Claude CLI response for testing/dry-run
    pub(crate) fn simulate_claude_response(&self, _phase_id: PhaseId, _prompt: &str) -> String {
        match _phase_id {
            PhaseId::Requirements => {
                // Generate a realistic requirements document
                r"# Requirements Document

## Introduction

This is a generated requirements document for the current specification. The system will provide core functionality for managing and processing specifications through a structured workflow.

## Requirements

### Requirement 1

**User Story:** As a developer, I want to generate structured requirements from rough ideas, so that I can create comprehensive specifications efficiently.

#### Acceptance Criteria

1. WHEN I provide a problem statement THEN the system SHALL generate structured requirements in EARS format
2. WHEN requirements are generated THEN they SHALL include user stories and acceptance criteria
3. WHEN the process completes THEN the system SHALL produce both markdown and YAML artifacts

### Requirement 2

**User Story:** As a developer, I want deterministic output generation, so that I can reproduce results consistently.

#### Acceptance Criteria

1. WHEN identical inputs are provided THEN the system SHALL produce identical canonicalized outputs
2. WHEN artifacts are created THEN they SHALL include BLAKE3 hashes for verification
3. WHEN the process runs THEN it SHALL create audit receipts for traceability

### Requirement 3

**User Story:** As a developer, I want atomic file operations, so that partial writes don't corrupt the system state.

#### Acceptance Criteria

1. WHEN writing artifacts THEN the system SHALL use atomic write operations
2. WHEN failures occur THEN partial artifacts SHALL be preserved for debugging
3. WHEN operations complete THEN all files SHALL be in a consistent state

## Non-Functional Requirements

**NFR1 Performance:** The system SHALL complete requirements generation within reasonable time limits
**NFR2 Reliability:** All file operations SHALL be atomic to prevent corruption
**NFR3 Auditability:** All operations SHALL be logged with cryptographic verification
".to_string()
            }
            PhaseId::Design => {
                r"# Design Document

## Overview

This is a comprehensive design document for the current specification. The system implements a phase-based architecture for orchestrating spec generation workflows using the Claude CLI.

## Architecture

The system follows a modular architecture with clear separation of concerns:

```mermaid
graph TD
    A[CLI Entry] --> B[Phase Orchestrator]
    B --> C[Requirements Phase]
    C --> D[Design Phase]
    D --> E[Tasks Phase]
    E --> F[Review Phase]
```

## Components and Interfaces

### Phase System
- **Phase trait**: Defines the interface for all workflow phases
- **PhaseOrchestrator**: Manages phase execution and dependencies
- **PhaseContext**: Provides context and configuration to phases

### Artifact Management
- **ArtifactManager**: Handles atomic file operations and storage
- **ReceiptManager**: Creates and manages execution receipts
- **Canonicalizer**: Ensures deterministic output formatting

## Data Models

### Core Types
- `PhaseId`: Enumeration of available phases
- `Artifact`: Represents generated outputs with metadata
- `Receipt`: Audit trail for phase execution

### Configuration
- `OrchestratorConfig`: Runtime configuration parameters
- `PhaseContext`: Execution context for phases

## Error Handling

The system implements comprehensive error handling with:
- Structured error types for different failure modes
- Partial artifact preservation on failures
- Detailed error reporting with context

## Testing Strategy

- Unit tests for individual components
- Integration tests for end-to-end workflows
- Property-based tests for determinism validation
- Mock Claude CLI for testing scenarios
".to_string()
            }
            PhaseId::Tasks => {
                r"# Implementation Plan

## Milestone 1: Core Phase System

- [ ] 1. Set up project structure and core interfaces
  - Create directory structure for phases, artifacts, and receipts
  - Define Phase trait with separated concerns (prompt, make_packet, postprocess)
  - Implement PhaseId enum and basic dependency system
  - _Requirements: R10.1, R10.3_

- [ ] 2. Implement Requirements phase
- [ ] 2.1 Create RequirementsPhase struct
  - Implement Phase trait methods for requirements generation
  - Create prompt template for EARS format requirements
  - Add packet construction with basic context
  - _Requirements: R1.1_

- [ ] 2.2 Add requirements postprocessing
  - Parse Claude response into requirements.md artifact
  - Generate requirements.core.yaml with structured data
  - Implement artifact creation and storage
  - _Requirements: R1.1, R2.1_

- [ ]* 2.3 Write unit tests for Requirements phase
  - Test prompt generation and packet creation
  - Verify postprocessing creates correct artifacts
  - Test error handling scenarios
  - _Requirements: R1.1_

## Milestone 2: Design and Tasks Phases

- [ ] 3. Implement Design phase
- [ ] 3.1 Create DesignPhase struct
  - Implement Phase trait with architecture-focused prompts
  - Add dependency on Requirements phase
  - Include requirements artifacts in packet construction
  - _Requirements: R1.1_

- [ ] 3.2 Add design postprocessing
  - Parse Claude response into design.md artifact
  - Generate design.core.yaml with structured data
  - Implement component and interface extraction
  - _Requirements: R1.1, R2.1_

- [ ] 4. Implement Tasks phase
- [ ] 4.1 Create TasksPhase struct
  - Implement Phase trait with implementation planning prompts
  - Add dependencies on Design and Requirements phases
  - Include all upstream artifacts in packet construction
  - _Requirements: R1.1_

- [ ] 4.2 Add tasks postprocessing
  - Parse Claude response into tasks.md artifact
  - Generate tasks.core.yaml with structured task data
  - Implement task parsing and validation
  - _Requirements: R1.1, R2.1_

- [ ]* 4.3 Write integration tests for phase system
  - Test Requirements → Design → Tasks flow
  - Verify dependency checking works correctly
  - Test artifact propagation between phases
  - _Requirements: R1.1, R4.2_

## Milestone 3: Orchestrator Integration

- [ ] 5. Update PhaseOrchestrator for new phases
- [ ] 5.1 Add execution methods for Design and Tasks phases
  - Implement execute_design_phase method
  - Implement execute_tasks_phase method
  - Update dependency checking logic
  - _Requirements: R1.1, R4.2_

- [ ] 5.2 Enhance Claude response simulation
  - Add realistic responses for Design phase
  - Add realistic responses for Tasks phase
  - Update test scenarios for all phases
  - _Requirements: R4.1_

- [ ]* 5.3 Write end-to-end integration tests
  - Test complete Requirements → Design → Tasks workflow
  - Verify artifact creation and receipt generation
  - Test error handling and partial artifact storage
  - _Requirements: R1.1, R4.3_
".to_string()
            }
            _ => {
                format!("Simulated response for phase: {}", _phase_id.as_str())
            }
        }
    }

    /// Get the phase number for artifact naming
    pub(crate) const fn get_phase_number(&self, phase_id: PhaseId) -> u8 {
        match phase_id {
            PhaseId::Requirements => 0,
            PhaseId::Design => 10,
            PhaseId::Tasks => 20,
            PhaseId::Review => 30,
            PhaseId::Fixup => 40,
            PhaseId::Final => 50,
        }
    }

    /// Get a phase implementation by ID (phase factory)
    /// This method creates the appropriate Phase trait object for the given phase ID
    pub(crate) fn get_phase_impl(
        &self,
        phase_id: PhaseId,
        config: &OrchestratorConfig,
    ) -> Result<Box<dyn Phase>> {
        match phase_id {
            PhaseId::Requirements => Ok(Box::new(RequirementsPhase::new())),
            PhaseId::Design => Ok(Box::new(DesignPhase::new())),
            PhaseId::Tasks => Ok(Box::new(TasksPhase::new())),
            PhaseId::Review => Ok(Box::new(ReviewPhase::new())),
            PhaseId::Fixup => {
                // Determine fixup mode from configuration (FR-FIX-004, FR-FIX-005)
                let apply_fixups = config
                    .config
                    .get("apply_fixups")
                    .is_some_and(|s| s == "true");

                let fixup_mode = if apply_fixups {
                    FixupMode::Apply
                } else {
                    FixupMode::Preview
                };

                Ok(Box::new(FixupPhase::new_with_mode(fixup_mode)))
            }
            PhaseId::Final => Err(anyhow::anyhow!("Final phase not yet implemented")),
        }
    }
}

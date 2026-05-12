//! Pre/post phase hook execution and hook-specific failure receipts.

use std::collections::HashMap;

use anyhow::Result;

use crate::exit_codes;
use crate::hooks::{HookContext, HookExecutor, HookType, execute_and_process_hook};
use crate::types::{ErrorKind, PacketEvidence, PhaseId, PipelineInfo};

use super::ExecutionResult;
use crate::orchestrator::{OrchestratorConfig, PhaseOrchestrator};

pub(crate) enum PrePhaseHookResult {
    Continue { warnings: Vec<String> },
    Abort(ExecutionResult),
}

impl PhaseOrchestrator {
    /// Run the configured pre-phase hook, creating an audit receipt if it aborts the phase.
    pub(crate) async fn run_pre_phase_hook(
        &self,
        phase_id: PhaseId,
        config: &OrchestratorConfig,
        pipeline_info: Option<PipelineInfo>,
    ) -> Result<PrePhaseHookResult> {
        let Some(ref hooks_config) = config.hooks else {
            return Ok(PrePhaseHookResult::Continue { warnings: vec![] });
        };
        let Some(hook_config) = hooks_config.get_pre_phase_hook(phase_id) else {
            return Ok(PrePhaseHookResult::Continue { warnings: vec![] });
        };

        let executor = hook_executor();
        let context = HookContext::new(self.spec_id(), phase_id, HookType::PrePhase);
        let mut hook_warnings = Vec::new();

        match execute_and_process_hook(
            &executor,
            hook_config,
            &context,
            HookType::PrePhase,
            phase_id,
        )
        .await
        {
            Ok(outcome) => {
                if let Some(warning) = outcome.warning() {
                    hook_warnings.push(warning.to_warning_string());
                }
                if outcome.should_continue() {
                    Ok(PrePhaseHookResult::Continue {
                        warnings: hook_warnings,
                    })
                } else {
                    let error_reason = format!(
                        "Pre-phase hook failed: {}",
                        outcome.error().map(|e| e.to_string()).unwrap_or_default()
                    );
                    self.pre_phase_hook_failure_result(
                        phase_id,
                        config,
                        pipeline_info,
                        hook_warnings,
                        "hook_failure",
                        error_reason,
                    )
                }
            }
            Err(e) => {
                let error_reason = format!("Pre-phase hook error: {}", e);
                self.pre_phase_hook_failure_result(
                    phase_id,
                    config,
                    pipeline_info,
                    vec![format!("hook_error:pre_phase:{}", e)],
                    "hook_error",
                    error_reason,
                )
            }
        }
    }

    /// Run post-phase hooks after successful artifact/receipt creation.
    pub(crate) async fn run_post_phase_hook(&self, phase_id: PhaseId, config: &OrchestratorConfig) {
        let Some(ref hooks_config) = config.hooks else {
            return;
        };
        let Some(hook_config) = hooks_config.get_post_phase_hook(phase_id) else {
            return;
        };

        let executor = hook_executor();
        let context = HookContext::new(self.spec_id(), phase_id, HookType::PostPhase);

        match execute_and_process_hook(
            &executor,
            hook_config,
            &context,
            HookType::PostPhase,
            phase_id,
        )
        .await
        {
            Ok(outcome) => {
                if let Some(warning) = outcome.warning() {
                    tracing::warn!(
                        phase = %phase_id.as_str(),
                        "Post-phase hook warning: {}",
                        warning.to_warning_string()
                    );
                }
                if !outcome.should_continue() {
                    tracing::warn!(
                        phase = %phase_id.as_str(),
                        "Post-phase hook had on_fail=fail but phase artifacts already created; treating as warning"
                    );
                }
            }
            Err(e) => {
                tracing::warn!(
                    phase = %phase_id.as_str(),
                    error = %e,
                    "Post-phase hook execution error (treated as warning)"
                );
            }
        }
    }

    fn pre_phase_hook_failure_result(
        &self,
        phase_id: PhaseId,
        config: &OrchestratorConfig,
        pipeline_info: Option<PipelineInfo>,
        hook_warnings: Vec<String>,
        flag_key: &str,
        error_reason: String,
    ) -> Result<PrePhaseHookResult> {
        let packet_evidence = PacketEvidence {
            files: vec![],
            max_bytes: 65536,
            max_lines: 1200,
        };
        let mut flags = HashMap::new();
        flags.insert("phase".to_string(), phase_id.as_str().to_string());
        flags.insert(flag_key.to_string(), "pre_phase".to_string());

        let configured_model = config.config.get("model").map_or("unknown", |s| s.as_str());
        let configured_runner = config
            .config
            .get("runner_mode")
            .map_or("unknown", |s| s.as_str());

        let receipt = self.receipt_manager().create_receipt_with_redactor(
            config.redactor.as_ref(),
            self.spec_id(),
            phase_id,
            exit_codes::codes::CLAUDE_FAILURE,
            vec![],
            env!("CARGO_PKG_VERSION"),
            "unknown",
            configured_model,
            None,
            flags,
            packet_evidence,
            None,
            None,
            hook_warnings,
            None,
            configured_runner,
            None,
            Some(ErrorKind::ClaudeFailure),
            Some(error_reason.clone()),
            None,
            pipeline_info,
        );

        let receipt_path = self.receipt_manager().write_receipt(&receipt)?;
        Ok(PrePhaseHookResult::Abort(ExecutionResult {
            phase: phase_id,
            success: false,
            exit_code: exit_codes::codes::CLAUDE_FAILURE,
            artifact_paths: vec![],
            receipt_path: Some(receipt_path.into_std_path_buf()),
            error: Some(error_reason),
        }))
    }
}

fn hook_executor() -> HookExecutor {
    HookExecutor::new(std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")))
}

//! Hook execution policy for phase execution.

use anyhow::Result;

use crate::hooks::{HookContext, HookExecutor, HookType, execute_and_process_hook};
use crate::types::PhaseId;

use super::super::{OrchestratorConfig, PhaseOrchestrator};
use super::ExecutionResult;

pub(super) struct PrePhaseHookOutcome {
    pub(super) warnings: Vec<String>,
    pub(super) abort: Option<ExecutionResult>,
}

impl PhaseOrchestrator {
    pub(super) async fn run_pre_phase_hook(
        &self,
        phase_id: PhaseId,
        config: &OrchestratorConfig,
    ) -> Result<PrePhaseHookOutcome> {
        let mut warnings = Vec::new();
        let Some(ref hooks_config) = config.hooks else {
            return Ok(PrePhaseHookOutcome {
                warnings,
                abort: None,
            });
        };
        let Some(hook_config) = hooks_config.get_pre_phase_hook(phase_id) else {
            return Ok(PrePhaseHookOutcome {
                warnings,
                abort: None,
            });
        };

        let executor = HookExecutor::new(
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
        );
        let context = HookContext::new(self.spec_id(), phase_id, HookType::PrePhase);

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
                    warnings.push(warning.to_warning_string());
                }
                if outcome.should_continue() {
                    Ok(PrePhaseHookOutcome {
                        warnings,
                        abort: None,
                    })
                } else {
                    let error_reason = format!(
                        "Pre-phase hook failed: {}",
                        outcome.error().map(|e| e.to_string()).unwrap_or_default()
                    );
                    let abort = self.write_pre_hook_failure_result(
                        phase_id,
                        config,
                        warnings.clone(),
                        "hook_failure",
                        error_reason,
                    )?;
                    Ok(PrePhaseHookOutcome {
                        warnings,
                        abort: Some(abort),
                    })
                }
            }
            Err(e) => {
                let error_reason = format!("Pre-phase hook error: {}", e);
                let abort = self.write_pre_hook_failure_result(
                    phase_id,
                    config,
                    vec![format!("hook_error:pre_phase:{}", e)],
                    "hook_error",
                    error_reason,
                )?;
                Ok(PrePhaseHookOutcome {
                    warnings,
                    abort: Some(abort),
                })
            }
        }
    }

    pub(super) async fn run_post_phase_hook(&self, phase_id: PhaseId, config: &OrchestratorConfig) {
        let Some(ref hooks_config) = config.hooks else {
            return;
        };
        let Some(hook_config) = hooks_config.get_post_phase_hook(phase_id) else {
            return;
        };

        let executor = HookExecutor::new(
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
        );
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
}

//! LLM invocation selection and invocation-error classification.

use anyhow::Result;

use super::super::llm::{ClaudeExecutionMetadata, LlmInvocationError};
use super::super::{OrchestratorConfig, PhaseOrchestrator};
use super::ExecutionResult;
use crate::error::XCheckerError;
use crate::types::{PacketEvidence, PhaseId};

pub(super) struct LlmPhaseExecution {
    pub(super) response: String,
    pub(super) exit_code: i32,
    pub(super) metadata: Option<ClaudeExecutionMetadata>,
    pub(super) result: Option<crate::llm::LlmResult>,
    pub(super) fallback_warning: Option<String>,
}

pub(super) enum LlmStepOutcome {
    Completed(LlmPhaseExecution),
    Failed(ExecutionResult),
}

impl PhaseOrchestrator {
    pub(super) async fn execute_llm_step(
        &self,
        phase_id: PhaseId,
        config: &OrchestratorConfig,
        prompt: &str,
        packet_content: &str,
        packet_evidence: PacketEvidence,
    ) -> Result<LlmStepOutcome> {
        if config.dry_run {
            return Ok(LlmStepOutcome::Completed(LlmPhaseExecution {
                response: self.simulate_claude_response(phase_id, prompt),
                exit_code: 0,
                metadata: Some(ClaudeExecutionMetadata {
                    model_alias: None,
                    model_full_name: "haiku".to_string(),
                    claude_cli_version: "0.8.1".to_string(),
                    fallback_used: false,
                    runner: "simulated".to_string(),
                    runner_distro: None,
                    stderr_tail: None,
                }),
                result: Some(self.simulate_llm_result(phase_id)),
                fallback_warning: None,
            }));
        }

        match self
            .run_llm_invocation(prompt, packet_content, phase_id, config)
            .await
        {
            Ok((response, exit_code, metadata, result, fallback_warning)) => {
                Ok(LlmStepOutcome::Completed(LlmPhaseExecution {
                    response,
                    exit_code,
                    metadata,
                    result,
                    fallback_warning,
                }))
            }
            Err(e) => {
                let (xchecker_err, fallback_warning) =
                    if let Some(invocation_err) = e.downcast_ref::<LlmInvocationError>() {
                        (
                            invocation_err.error(),
                            invocation_err.fallback_warning().map(str::to_string),
                        )
                    } else if let Some(xchecker_err) = e.downcast_ref::<XCheckerError>() {
                        (xchecker_err, None)
                    } else {
                        return Err(e);
                    };

                if let XCheckerError::Llm(llm_err) = xchecker_err {
                    let failure = if matches!(llm_err, crate::llm::LlmError::BudgetExceeded { .. })
                    {
                        self.write_budget_exhaustion_result(
                            phase_id,
                            config,
                            packet_evidence,
                            llm_err,
                            fallback_warning.as_deref(),
                        )?
                    } else {
                        self.write_llm_error_result(
                            phase_id,
                            config,
                            prompt,
                            packet_content,
                            packet_evidence,
                            xchecker_err,
                            llm_err,
                            fallback_warning.as_deref(),
                        )?
                    };
                    Ok(LlmStepOutcome::Failed(failure))
                } else {
                    Err(e)
                }
            }
        }
    }
}

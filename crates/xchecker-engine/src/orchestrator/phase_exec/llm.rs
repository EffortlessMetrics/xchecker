//! LLM invocation branching and invocation-error receipt generation.

use std::collections::HashMap;

use anyhow::Result;

use crate::error::XCheckerError;
use crate::exit_codes;
use crate::packet::Packet;
use crate::types::{ErrorKind, LlmInfo, PhaseId, PipelineInfo};

use super::ExecutionResult;
use crate::orchestrator::llm::{ClaudeExecutionMetadata, LlmInvocationError};
use crate::orchestrator::{OrchestratorConfig, PhaseOrchestrator};

pub(crate) enum PhaseLlmOutcome {
    Completed {
        claude_response: String,
        claude_exit_code: i32,
        claude_metadata: Option<ClaudeExecutionMetadata>,
        llm_result: Option<crate::llm::LlmResult>,
        llm_fallback_warning: Option<String>,
    },
    Failed(ExecutionResult),
}

impl PhaseOrchestrator {
    /// Invoke the configured LLM or produce dry-run output, turning provider invocation errors into receipts.
    pub(crate) async fn execute_llm_for_phase(
        &self,
        phase_id: PhaseId,
        prompt: &str,
        packet: &Packet,
        config: &OrchestratorConfig,
        pipeline_info: Option<PipelineInfo>,
    ) -> Result<PhaseLlmOutcome> {
        if config.dry_run {
            return Ok(PhaseLlmOutcome::Completed {
                claude_response: self.simulate_claude_response(phase_id, prompt),
                claude_exit_code: 0,
                claude_metadata: Some(ClaudeExecutionMetadata {
                    model_alias: None,
                    model_full_name: "haiku".to_string(),
                    claude_cli_version: "0.8.1".to_string(),
                    fallback_used: false,
                    runner: "simulated".to_string(),
                    runner_distro: None,
                    stderr_tail: None,
                }),
                llm_result: Some(self.simulate_llm_result(phase_id)),
                llm_fallback_warning: None,
            });
        }

        match self
            .run_llm_invocation(prompt, &packet.content, phase_id, config)
            .await
        {
            Ok((response, exit_code, metadata, result, fallback_warning)) => {
                Ok(PhaseLlmOutcome::Completed {
                    claude_response: response,
                    claude_exit_code: exit_code,
                    claude_metadata: metadata,
                    llm_result: result,
                    llm_fallback_warning: fallback_warning,
                })
            }
            Err(e) => {
                let (xchecker_err, fallback_warning) =
                    if let Some(invocation_err) = e.downcast_ref::<LlmInvocationError>() {
                        (
                            invocation_err.error(),
                            invocation_err.fallback_warning().map(|s| s.to_string()),
                        )
                    } else if let Some(xchecker_err) = e.downcast_ref::<XCheckerError>() {
                        (xchecker_err, None)
                    } else {
                        return Err(e);
                    };

                if let XCheckerError::Llm(llm_err) = xchecker_err {
                    let result = self.llm_error_result(
                        phase_id,
                        prompt,
                        packet,
                        config,
                        pipeline_info,
                        xchecker_err,
                        llm_err,
                        fallback_warning,
                    )?;
                    return Ok(PhaseLlmOutcome::Failed(result));
                }

                Err(e)
            }
        }
    }

    fn llm_error_result(
        &self,
        phase_id: PhaseId,
        prompt: &str,
        packet: &Packet,
        config: &OrchestratorConfig,
        pipeline_info: Option<PipelineInfo>,
        xchecker_err: &XCheckerError,
        llm_err: &crate::llm::LlmError,
        fallback_warning: Option<String>,
    ) -> Result<ExecutionResult> {
        if matches!(llm_err, crate::llm::LlmError::BudgetExceeded { .. }) {
            return self.budget_exhaustion_result(
                phase_id,
                packet,
                config,
                pipeline_info,
                llm_err,
                fallback_warning,
            );
        }

        let packet_evidence = packet.evidence.clone();
        let mut flags = HashMap::new();
        flags.insert("phase".to_string(), phase_id.as_str().to_string());

        let configured_model = config.config.get("model").map_or("unknown", |s| s.as_str());
        let configured_runner = config
            .config
            .get("runner_mode")
            .map_or("unknown", |s| s.as_str());

        let (exit_code, error_kind) = exit_codes::error_to_exit_code_and_kind(xchecker_err);

        let invocation = self.build_llm_invocation(phase_id, prompt, &packet.content, config);
        let provider = self
            .config_from_orchestrator_config(config)
            .llm
            .provider
            .unwrap_or_else(|| "claude-cli".to_string());

        let mut llm_info = LlmInfo {
            provider: Some(provider),
            model_used: if invocation.model.is_empty() {
                None
            } else {
                Some(invocation.model.clone())
            },
            tokens_input: None,
            tokens_output: None,
            timed_out: None,
            timeout_seconds: Some(invocation.timeout.as_secs()),
            budget_exhausted: None,
        };

        let mut warnings = Vec::new();
        match llm_err {
            crate::llm::LlmError::Timeout { duration } => {
                llm_info.timed_out = Some(true);
                llm_info.timeout_seconds = Some(duration.as_secs());
                warnings.push(format!("phase_timeout:{}", duration.as_secs()));
            }
            _ => {
                llm_info.timed_out = Some(false);
                warnings.push(format!("llm_error:{}", llm_err));
            }
        }
        if let Some(ref warning) = fallback_warning {
            warnings.push(warning.clone());
        }

        let mut receipt = self.receipt_manager().create_receipt_with_redactor(
            config.redactor.as_ref(),
            self.spec_id(),
            phase_id,
            exit_code,
            vec![],
            env!("CARGO_PKG_VERSION"),
            "unknown",
            configured_model,
            None,
            flags,
            packet_evidence,
            None,
            None,
            warnings,
            None,
            configured_runner,
            None,
            Some(error_kind),
            Some(llm_err.to_string()),
            None,
            pipeline_info,
        );

        receipt.llm = Some(llm_info);
        let receipt_path = self.receipt_manager().write_receipt(&receipt)?;

        Ok(ExecutionResult {
            phase: phase_id,
            success: false,
            exit_code,
            artifact_paths: vec![],
            receipt_path: Some(receipt_path.into_std_path_buf()),
            error: Some(llm_err.to_string()),
        })
    }

    fn budget_exhaustion_result(
        &self,
        phase_id: PhaseId,
        packet: &Packet,
        config: &OrchestratorConfig,
        pipeline_info: Option<PipelineInfo>,
        llm_err: &crate::llm::LlmError,
        fallback_warning: Option<String>,
    ) -> Result<ExecutionResult> {
        let packet_evidence = packet.evidence.clone();
        let mut flags = HashMap::new();
        flags.insert("phase".to_string(), phase_id.as_str().to_string());

        let configured_model = config.config.get("model").map_or("unknown", |s| s.as_str());
        let configured_runner = config
            .config
            .get("runner_mode")
            .map_or("unknown", |s| s.as_str());

        let mut warnings = vec![format!("LLM budget exhausted: {}", llm_err)];
        if let Some(ref warning) = fallback_warning {
            warnings.push(warning.clone());
        }

        let mut receipt = self.receipt_manager().create_receipt_with_redactor(
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
            warnings,
            None,
            configured_runner,
            None,
            Some(ErrorKind::ClaudeFailure),
            Some(llm_err.to_string()),
            None,
            pipeline_info,
        );

        receipt.llm = Some(LlmInfo::for_budget_exhaustion());
        let receipt_path = self.receipt_manager().write_receipt(&receipt)?;

        Ok(ExecutionResult {
            phase: phase_id,
            success: false,
            exit_code: exit_codes::codes::CLAUDE_FAILURE,
            artifact_paths: vec![],
            receipt_path: Some(receipt_path.into_std_path_buf()),
            error: Some(llm_err.to_string()),
        })
    }
}

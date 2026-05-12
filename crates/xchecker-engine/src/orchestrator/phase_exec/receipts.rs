//! Receipt emission for phase success and failure paths.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::error::{PhaseError, XCheckerError};
use crate::exit_codes;
use crate::status::artifact::{Artifact, ArtifactType};
use crate::types::{ErrorKind, FileHash, LlmInfo, PacketEvidence, PhaseId, PipelineInfo};

use super::super::{OrchestratorConfig, PhaseOrchestrator};
use super::ExecutionResult;
use super::llm_step::LlmPhaseExecution;

impl PhaseOrchestrator {
    pub(super) fn write_pre_hook_failure_result(
        &self,
        phase_id: PhaseId,
        config: &OrchestratorConfig,
        warnings: Vec<String>,
        flag_key: &str,
        error_reason: String,
    ) -> Result<ExecutionResult> {
        let mut flags = phase_flags(phase_id);
        flags.insert(flag_key.to_string(), "pre_phase".to_string());
        let (configured_model, configured_runner) = configured_model_and_runner(config);
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
            empty_packet_evidence(),
            None,
            None,
            warnings,
            None,
            configured_runner,
            None,
            Some(ErrorKind::ClaudeFailure),
            Some(error_reason.clone()),
            None,
            controlled_pipeline_info(),
        );
        let receipt_path = self.receipt_manager().write_receipt(&receipt)?;
        Ok(failure_result(
            phase_id,
            exit_codes::codes::CLAUDE_FAILURE,
            vec![],
            Some(receipt_path.into_std_path_buf()),
            error_reason,
        ))
    }

    pub(super) fn write_secret_detected_result(
        &self,
        phase_id: PhaseId,
        config: &OrchestratorConfig,
        packet_evidence: PacketEvidence,
        packet_content: &str,
    ) -> Result<Option<ExecutionResult>> {
        let redactor = config.redactor.as_ref();
        if !redactor.has_secrets(packet_content, "packet")? {
            return Ok(None);
        }

        let matches = redactor.scan_for_secrets(packet_content, "packet")?;
        let secret_patterns: Vec<String> = matches.iter().map(|m| m.pattern_id.clone()).collect();
        let error_reason = format!(
            "Secret detected in packet. Matched patterns: {}",
            secret_patterns.join(", ")
        );

        let receipt = self.receipt_manager().create_receipt_with_redactor(
            config.redactor.as_ref(),
            self.spec_id(),
            phase_id,
            exit_codes::codes::SECRET_DETECTED,
            vec![],
            env!("CARGO_PKG_VERSION"),
            "0.8.1",
            "haiku",
            None,
            phase_flags(phase_id),
            packet_evidence,
            None,
            None,
            vec!["Secret detection prevented Claude invocation".to_string()],
            None,
            "native",
            None,
            Some(ErrorKind::SecretDetected),
            Some(error_reason.clone()),
            None,
            controlled_pipeline_info(),
        );
        let receipt_path = self.receipt_manager().write_receipt(&receipt)?;
        Ok(Some(failure_result(
            phase_id,
            exit_codes::codes::SECRET_DETECTED,
            vec![],
            Some(receipt_path.into_std_path_buf()),
            error_reason,
        )))
    }

    pub(super) fn write_budget_exhaustion_result(
        &self,
        phase_id: PhaseId,
        config: &OrchestratorConfig,
        packet_evidence: PacketEvidence,
        llm_err: &crate::llm::LlmError,
        llm_fallback_warning: Option<&str>,
    ) -> Result<ExecutionResult> {
        let (configured_model, configured_runner) = configured_model_and_runner(config);
        let mut warnings = vec![format!("LLM budget exhausted: {llm_err}")];
        if let Some(warning) = llm_fallback_warning {
            warnings.push(warning.to_string());
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
            phase_flags(phase_id),
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
            controlled_pipeline_info(),
        );
        receipt.llm = Some(LlmInfo::for_budget_exhaustion());
        let receipt_path = self.receipt_manager().write_receipt(&receipt)?;
        Ok(failure_result(
            phase_id,
            exit_codes::codes::CLAUDE_FAILURE,
            vec![],
            Some(receipt_path.into_std_path_buf()),
            llm_err.to_string(),
        ))
    }

    pub(super) fn write_llm_error_result(
        &self,
        phase_id: PhaseId,
        config: &OrchestratorConfig,
        prompt: &str,
        packet_content: &str,
        packet_evidence: PacketEvidence,
        xchecker_err: &XCheckerError,
        llm_err: &crate::llm::LlmError,
        llm_fallback_warning: Option<&str>,
    ) -> Result<ExecutionResult> {
        let (configured_model, configured_runner) = configured_model_and_runner(config);
        let (exit_code, error_kind) = exit_codes::error_to_exit_code_and_kind(xchecker_err);
        let invocation = self.build_llm_invocation(phase_id, prompt, packet_content, config);
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
                warnings.push(format!("llm_error:{llm_err}"));
            }
        }
        if let Some(warning) = llm_fallback_warning {
            warnings.push(warning.to_string());
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
            phase_flags(phase_id),
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
            controlled_pipeline_info(),
        );
        receipt.llm = Some(llm_info);
        let receipt_path = self.receipt_manager().write_receipt(&receipt)?;
        Ok(failure_result(
            phase_id,
            exit_code,
            vec![],
            Some(receipt_path.into_std_path_buf()),
            llm_err.to_string(),
        ))
    }

    pub(super) fn write_nonzero_llm_result(
        &self,
        phase_id: PhaseId,
        config: &OrchestratorConfig,
        packet_evidence: PacketEvidence,
        llm: LlmPhaseExecution,
    ) -> Result<ExecutionResult> {
        let partial_filename = format!(
            "{:02}-{}.partial.md",
            self.get_phase_number(phase_id),
            phase_id.as_str().to_lowercase()
        );
        let partial_result = self.artifact_manager().store_artifact(&Artifact {
            name: partial_filename.clone(),
            content: llm.response.clone(),
            artifact_type: ArtifactType::Partial,
            blake3_hash: blake3::hash(llm.response.as_bytes()).to_hex().to_string(),
        })?;

        let (model_alias, model_full_name) = model_names(&llm.metadata);
        let mut warnings = vec!["Phase execution failed with non-zero exit code".to_string()];
        if let Some(ref warning) = llm.fallback_warning {
            warnings.push(warning.clone());
        }

        let mut receipt = self.receipt_manager().create_receipt_with_redactor(
            config.redactor.as_ref(),
            self.spec_id(),
            phase_id,
            llm.exit_code,
            vec![],
            env!("CARGO_PKG_VERSION"),
            llm.metadata
                .as_ref()
                .map_or("0.8.1", |m| m.claude_cli_version.as_str()),
            &model_full_name,
            model_alias,
            phase_flags(phase_id),
            packet_evidence,
            Some("Claude CLI execution failed".to_string()),
            None,
            warnings,
            llm.metadata.as_ref().map(|m| m.fallback_used),
            llm.metadata
                .as_ref()
                .map_or("native", |m| m.runner.as_str()),
            llm.metadata.as_ref().and_then(|m| m.runner_distro.clone()),
            Some(ErrorKind::ClaudeFailure),
            Some("Claude CLI execution failed".to_string()),
            None,
            controlled_pipeline_info(),
        );
        receipt.llm = llm.result.map(|result| result.into_llm_info());
        let receipt_path = self.receipt_manager().write_receipt(&receipt)?;

        let stderr_info = llm
            .metadata
            .as_ref()
            .and_then(|m| m.stderr_tail.clone())
            .unwrap_or_else(|| "No stderr captured".to_string());
        let enhanced_error = if !stderr_info.is_empty() && stderr_info != "No stderr captured" {
            XCheckerError::Phase(PhaseError::ExecutionFailedWithStderr {
                phase: phase_id.as_str().to_string(),
                code: llm.exit_code,
                stderr_tail: stderr_info,
            })
        } else {
            XCheckerError::Phase(PhaseError::PartialOutputSaved {
                phase: phase_id.as_str().to_string(),
                partial_path: format!("artifacts/{partial_filename}"),
            })
        };

        Ok(failure_result(
            phase_id,
            llm.exit_code,
            vec![partial_result.path.into_std_path_buf()],
            Some(receipt_path.into_std_path_buf()),
            enhanced_error.to_string(),
        ))
    }

    pub(super) fn write_success_result(
        &self,
        phase_id: PhaseId,
        config: &OrchestratorConfig,
        packet_evidence: PacketEvidence,
        output_hashes: Vec<FileHash>,
        mut warnings: Vec<String>,
        llm: LlmPhaseExecution,
    ) -> Result<PathBuf> {
        let (model_alias, model_full_name) = model_names(&llm.metadata);
        if let Some(warning) = llm.fallback_warning {
            warnings.push(warning);
        }

        let mut receipt = self.receipt_manager().create_receipt_with_redactor(
            config.redactor.as_ref(),
            self.spec_id(),
            phase_id,
            0,
            output_hashes,
            env!("CARGO_PKG_VERSION"),
            llm.metadata
                .as_ref()
                .map_or("0.8.1", |m| m.claude_cli_version.as_str()),
            &model_full_name,
            model_alias,
            phase_flags(phase_id),
            packet_evidence,
            None,
            None,
            warnings,
            llm.metadata.as_ref().map(|m| m.fallback_used),
            llm.metadata
                .as_ref()
                .map_or("native", |m| m.runner.as_str()),
            llm.metadata.as_ref().and_then(|m| m.runner_distro.clone()),
            None,
            None,
            None,
            controlled_pipeline_info(),
        );
        receipt.llm = llm.result.map(|r| r.into_llm_info());

        self.receipt_manager()
            .write_receipt(&receipt)
            .map(|p| p.into_std_path_buf())
            .with_context(|| format!("Failed to write receipt for phase: {}", phase_id.as_str()))
    }
}

fn phase_flags(phase_id: PhaseId) -> HashMap<String, String> {
    let mut flags = HashMap::new();
    flags.insert("phase".to_string(), phase_id.as_str().to_string());
    flags
}

fn configured_model_and_runner(config: &OrchestratorConfig) -> (&str, &str) {
    (
        config.config.get("model").map_or("unknown", |s| s.as_str()),
        config
            .config
            .get("runner_mode")
            .map_or("unknown", |s| s.as_str()),
    )
}

fn controlled_pipeline_info() -> Option<PipelineInfo> {
    Some(PipelineInfo {
        execution_strategy: Some("controlled".to_string()),
    })
}

fn empty_packet_evidence() -> PacketEvidence {
    PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    }
}

fn model_names(
    metadata: &Option<crate::orchestrator::llm::ClaudeExecutionMetadata>,
) -> (Option<String>, String) {
    metadata.as_ref().map_or((None, "haiku".to_string()), |m| {
        (m.model_alias.clone(), m.model_full_name.clone())
    })
}

fn failure_result(
    phase_id: PhaseId,
    exit_code: i32,
    artifact_paths: Vec<PathBuf>,
    receipt_path: Option<PathBuf>,
    error: String,
) -> ExecutionResult {
    ExecutionResult {
        phase: phase_id,
        success: false,
        exit_code,
        artifact_paths,
        receipt_path,
        error: Some(error),
    }
}

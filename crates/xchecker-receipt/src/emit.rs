use anyhow::{Context, Result};
use chrono::Utc;
use std::collections::HashMap;

use xchecker_utils::types::{ErrorKind, FileHash, PacketEvidence, PhaseId, Receipt};

use super::ReceiptManager;

impl ReceiptManager {
    /// Emit receipt JSON using JCS canonicalization (RFC 8785).
    pub(super) fn emit_receipt_jcs(receipt: &Receipt) -> Result<String> {
        let json_value = serde_json::to_value(receipt)
            .with_context(|| "Failed to serialize receipt to JSON value")?;
        let json_bytes = serde_json_canonicalizer::to_vec(&json_value)
            .with_context(|| "Failed to canonicalize receipt JSON")?;
        let json_content = String::from_utf8(json_bytes)
            .with_context(|| "Failed to convert canonical JSON to UTF-8 string")?;

        Ok(json_content)
    }

    /// Create an enhanced receipt for a completed phase
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn create_receipt(
        &self,
        spec_id: &str,
        phase: PhaseId,
        exit_code: i32,
        outputs: Vec<FileHash>,
        xchecker_version: &str,
        claude_cli_version: &str,
        model_full_name: &str,
        model_alias: Option<String>,
        flags: HashMap<String, String>,
        packet: PacketEvidence,
        stderr_tail: Option<String>,
        stderr_redacted: Option<String>,
        warnings: Vec<String>,
        fallback_used: Option<bool>,
        runner: &str,
        runner_distro: Option<String>,
        error_kind: Option<ErrorKind>,
        error_reason: Option<String>,
        diff_context: Option<u32>,
        pipeline: Option<xchecker_utils::types::PipelineInfo>,
    ) -> Receipt {
        self.create_receipt_with_redactor(
            xchecker_redaction::default_redactor(),
            spec_id,
            phase,
            exit_code,
            outputs,
            xchecker_version,
            claude_cli_version,
            model_full_name,
            model_alias,
            flags,
            packet,
            stderr_tail,
            stderr_redacted,
            warnings,
            fallback_used,
            runner,
            runner_distro,
            error_kind,
            error_reason,
            diff_context,
            pipeline,
        )
    }

    /// Create an enhanced receipt for a completed phase, applying a caller-provided redactor (FR-SEC-19).
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn create_receipt_with_redactor(
        &self,
        redactor: &xchecker_redaction::SecretRedactor,
        spec_id: &str,
        phase: PhaseId,
        exit_code: i32,
        outputs: Vec<FileHash>,
        xchecker_version: &str,
        claude_cli_version: &str,
        model_full_name: &str,
        model_alias: Option<String>,
        flags: HashMap<String, String>,
        packet: PacketEvidence,
        stderr_tail: Option<String>,
        stderr_redacted: Option<String>,
        warnings: Vec<String>,
        fallback_used: Option<bool>,
        runner: &str,
        runner_distro: Option<String>,
        error_kind: Option<ErrorKind>,
        error_reason: Option<String>,
        diff_context: Option<u32>,
        pipeline: Option<xchecker_utils::types::PipelineInfo>,
    ) -> Receipt {
        // Sort outputs by path for stable diffs
        let mut sorted_outputs = outputs;
        sorted_outputs.sort_by(|a, b| a.path.cmp(&b.path));

        // Apply redaction to all user-facing strings before persisting
        // This ensures no secrets leak into receipts
        let redacted_stderr_tail = stderr_tail.as_ref().map(|s| redactor.redact_string(s));

        let redacted_stderr_redacted = stderr_redacted.as_ref().map(|s| redactor.redact_string(s));

        let redacted_warnings = warnings.iter().map(|w| redactor.redact_string(w)).collect();

        let redacted_error_reason = error_reason.as_ref().map(|r| redactor.redact_string(r));

        Receipt {
            schema_version: "1".to_string(),
            emitted_at: Utc::now(),
            spec_id: spec_id.to_string(),
            phase: phase.as_str().to_string(),
            xchecker_version: xchecker_version.to_string(),
            claude_cli_version: claude_cli_version.to_string(),
            model_full_name: model_full_name.to_string(),
            model_alias,
            canonicalization_version: self.canonicalizer.version().to_string(),
            canonicalization_backend: self.canonicalizer.backend().to_string(),
            flags,
            runner: runner.to_string(),
            runner_distro,
            packet,
            outputs: sorted_outputs,
            exit_code,
            error_kind,
            error_reason: redacted_error_reason,
            stderr_tail: redacted_stderr_tail,
            stderr_redacted: redacted_stderr_redacted,
            warnings: redacted_warnings,
            fallback_used,
            diff_context,
            llm: None, // Will be set by orchestrator when ClaudeResponse is available
            pipeline,
        }
    }
}

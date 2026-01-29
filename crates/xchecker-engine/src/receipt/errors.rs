use camino::Utf8PathBuf;
use chrono::Utc;
use std::collections::HashMap;

use crate::error::XCheckerError;
use crate::types::{ErrorKind, PacketEvidence, PhaseId, Receipt};

use super::ReceiptManager;

/// Helper function to convert `XCheckerError` to (`exit_code`, `error_kind`) tuple
/// This replicates the logic from `exit_codes` module to avoid trait import issues
pub(super) const fn error_to_exit_code_and_kind(error: &XCheckerError) -> (i32, ErrorKind) {
    use crate::error::PhaseError;

    match error {
        // Configuration errors map to CLI_ARGS
        XCheckerError::Config(_) => (2, ErrorKind::CliArgs),

        // Packet overflow before Claude invocation
        XCheckerError::PacketOverflow { .. } => (7, ErrorKind::PacketOverflow),

        // Secret detection (redaction hard stop)
        XCheckerError::SecretDetected { .. } => (8, ErrorKind::SecretDetected),

        // Concurrent execution / lock held
        XCheckerError::ConcurrentExecution { .. } => (9, ErrorKind::LockHeld),
        XCheckerError::Lock(_) => (9, ErrorKind::LockHeld),

        // Phase errors
        XCheckerError::Phase(phase_err) => match phase_err {
            PhaseError::Timeout { .. } => (10, ErrorKind::PhaseTimeout),
            // Invalid transitions are CLI argument errors (FR-ORC-001, FR-ORC-002)
            PhaseError::InvalidTransition { .. } => (2, ErrorKind::CliArgs),
            PhaseError::DependencyNotSatisfied { .. } => (2, ErrorKind::CliArgs),
            _ => (1, ErrorKind::Unknown),
        },

        // Claude CLI failures
        XCheckerError::Claude(_) => (70, ErrorKind::ClaudeFailure),
        XCheckerError::Runner(_) => (70, ErrorKind::ClaudeFailure),

        // All other errors default to exit code 1 with Unknown kind
        _ => (1, ErrorKind::Unknown),
    }
}

/// Write an error receipt and exit the process with the appropriate exit code
///
/// This function ensures that the receipt's `exit_code` field always matches
/// the process exit code, and that `error_kind` and `error_reason` are properly set.
///
/// This is the canonical way to handle fatal errors in xchecker to prevent
/// silent drift between receipt data and actual process exit codes.
///
/// # Arguments
///
/// * `error` - The `XCheckerError` that caused the failure
/// * `spec_id` - The spec ID being processed
/// * `phase` - The phase that was executing when the error occurred
/// * `spec_base_path` - Path to the spec's base directory
///
/// # Panics
///
/// This function never returns - it always exits the process.
#[allow(dead_code)] // Error handling utility for receipt generation
pub fn write_error_receipt_and_exit(
    error: &XCheckerError,
    spec_id: &str,
    phase: PhaseId,
    spec_base_path: &Utf8PathBuf,
) -> ! {
    // Get exit code and error kind from the error
    let (exit_code, error_kind) = error_to_exit_code_and_kind(error);
    let error_reason = error.to_string();

    // Apply redaction to error reason before persisting
    let redacted_error_reason = crate::redaction::redact_user_string(&error_reason);

    // Create receipt manager
    let receipt_manager = ReceiptManager::new(spec_base_path);

    // Create minimal error receipt with required fields
    let receipt = Receipt {
        schema_version: "1".to_string(),
        emitted_at: Utc::now(),
        spec_id: spec_id.to_string(),
        phase: phase.as_str().to_string(),
        xchecker_version: env!("CARGO_PKG_VERSION").to_string(),
        claude_cli_version: "unknown".to_string(), // May not be available during early errors
        model_full_name: "unknown".to_string(),
        model_alias: None,
        canonicalization_version: "yaml-v1,md-v1".to_string(),
        canonicalization_backend: "jcs-rfc8785".to_string(),
        flags: HashMap::new(),
        runner: "unknown".to_string(),
        runner_distro: None,
        packet: PacketEvidence {
            files: vec![],
            max_bytes: 0,
            max_lines: 0,
        },
        outputs: vec![],
        exit_code,
        error_kind: Some(error_kind),
        error_reason: Some(redacted_error_reason.clone()),
        stderr_tail: None,
        stderr_redacted: None,
        warnings: vec![],
        fallback_used: None,
        diff_context: None,
        llm: None,      // No LLM info for early errors
        pipeline: None, // No pipeline info for early errors
    };

    // Try to write the receipt, but don't fail if we can't
    // (the error might be related to filesystem issues)
    if let Err(write_err) = receipt_manager.write_receipt(&receipt) {
        eprintln!("Warning: Failed to write error receipt: {write_err}");
        eprintln!("Original error: {redacted_error_reason}");
    }

    // Exit with the appropriate code
    std::process::exit(exit_code);
}

impl ReceiptManager {
    /// Create an error receipt and write it to disk
    ///
    /// This is used when an error occurs during phase execution to ensure
    /// the receipt contains `error_kind` and `error_reason` fields that match
    /// the process exit code.
    ///
    /// Note: Redaction is applied automatically by `create_receipt`, so no need
    /// to redact here. This ensures consistent redaction across all receipt types.
    #[must_use]
    #[allow(dead_code)] // Error handling utility for receipt generation
    #[allow(clippy::too_many_arguments)]
    pub fn create_error_receipt(
        &self,
        spec_id: &str,
        phase: PhaseId,
        error: &XCheckerError,
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
        diff_context: Option<u32>,
        pipeline: Option<crate::types::PipelineInfo>,
    ) -> Receipt {
        // Get exit code and error kind from the error
        let (exit_code, error_kind) = error_to_exit_code_and_kind(error);
        let error_reason = error.to_string();

        // create_receipt will apply redaction automatically
        self.create_receipt(
            spec_id,
            phase,
            exit_code,
            vec![], // No outputs for error receipts
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
            Some(error_kind),
            Some(error_reason),
            diff_context,
            pipeline,
        )
    }
}

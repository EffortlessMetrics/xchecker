use super::{LlmError, PhaseError, UserFriendlyError, XCheckerError};

impl XCheckerError {
    /// Get a user-friendly error message with context and actionable suggestions.
    ///
    /// This method combines the error message, context, and suggestions into
    /// a single formatted string suitable for display to end users. The format is:
    ///
    /// ```text
    /// Error: <user message>
    ///
    /// Context: <context if available>
    ///
    /// Suggestions:
    ///   • <suggestion 1>
    ///   • <suggestion 2>
    ///   ...
    /// ```
    ///
    /// # Example
    ///
    /// ```rust
    /// use xchecker_utils::error::XCheckerError;
    ///
    /// let err = XCheckerError::SecretDetected {
    ///     pattern: "ghp_".to_string(),
    ///     location: "test.txt".to_string(),
    /// };
    /// let message = err.display_for_user();
    /// assert!(message.contains("Security issue"));
    /// assert!(message.contains("Suggestions:"));
    /// ```
    #[must_use]
    pub fn display_for_user(&self) -> String {
        self.display_for_user_with_redactor(xchecker_redaction::default_redactor())
    }

    /// Get a user-friendly error message with context and actionable suggestions,
    /// applying a caller-provided redactor as a final safety net (FR-SEC-19).
    #[must_use]
    pub fn display_for_user_with_redactor(
        &self,
        redactor: &xchecker_redaction::SecretRedactor,
    ) -> String {
        let mut output = String::new();

        // Add the main error message
        output.push_str(&format!("Error: {}\n", self.user_message()));

        // Add context if available
        if let Some(ctx) = self.context() {
            output.push_str(&format!("\nContext: {}\n", ctx));
        }

        // Add suggestions if any
        let suggestions = self.suggestions();
        if !suggestions.is_empty() {
            output.push_str("\nSuggestions:\n");
            for suggestion in suggestions {
                output.push_str(&format!("  • {}\n", suggestion));
            }
        }

        // Apply redaction to ensure no secrets leak in user-facing error messages.
        // This is the final safety net before output reaches the user (FR-SEC-19).
        redactor.redact_string(&output)
    }

    /// Map this error to the appropriate CLI exit code.
    ///
    /// This is the single source of truth for both CLI exit codes and receipt
    /// `exit_code` fields. The mapping follows the documented exit code table:
    ///
    /// | Exit Code | Name | Description |
    /// |-----------|------|-------------|
    /// | 0 | SUCCESS | Completed successfully |
    /// | 1 | INTERNAL | General failure |
    /// | 2 | CLI_ARGS | Invalid CLI arguments |
    /// | 7 | PACKET_OVERFLOW | Packet size exceeded |
    /// | 8 | SECRET_DETECTED | Secret found in content |
    /// | 9 | LOCK_HELD | Lock already held |
    /// | 10 | PHASE_TIMEOUT | Phase timed out |
    /// | 70 | CLAUDE_FAILURE | Claude CLI failed |
    ///
    /// # Example
    ///
    /// ```rust
    /// use xchecker_utils::error::XCheckerError;
    /// use xchecker_utils::exit_codes::ExitCode;
    ///
    /// let err = XCheckerError::SecretDetected {
    ///     pattern: "ghp_".to_string(),
    ///     location: "test.txt".to_string(),
    /// };
    /// assert_eq!(err.to_exit_code(), ExitCode::SECRET_DETECTED);
    /// ```
    #[must_use]
    pub fn to_exit_code(&self) -> crate::exit_codes::ExitCode {
        use crate::exit_codes::ExitCode;

        match self {
            // Configuration errors map to CLI_ARGS
            XCheckerError::Config(_) => ExitCode::CLI_ARGS,

            // Packet overflow before Claude invocation
            XCheckerError::PacketOverflow { .. } => ExitCode::PACKET_OVERFLOW,

            // Secret detection (redaction hard stop)
            XCheckerError::SecretDetected { .. } => ExitCode::SECRET_DETECTED,

            // Concurrent execution / lock held
            XCheckerError::ConcurrentExecution { .. } => ExitCode::LOCK_HELD,
            XCheckerError::Lock(_) => ExitCode::LOCK_HELD,

            // Phase errors
            XCheckerError::Phase(phase_err) => {
                match phase_err {
                    PhaseError::Timeout { .. } => ExitCode::PHASE_TIMEOUT,
                    // Invalid transitions are CLI argument errors (FR-ORC-001, FR-ORC-002)
                    PhaseError::InvalidTransition { .. } => ExitCode::CLI_ARGS,
                    PhaseError::DependencyNotSatisfied { .. } => ExitCode::CLI_ARGS,
                    _ => ExitCode::INTERNAL,
                }
            }

            // Claude CLI failures
            XCheckerError::Claude(_) => ExitCode::CLAUDE_FAILURE,
            XCheckerError::Runner(_) => ExitCode::CLAUDE_FAILURE,

            // LLM backend errors
            XCheckerError::Llm(llm_err) => match llm_err {
                LlmError::ProviderAuth(_) => ExitCode::CLAUDE_FAILURE,
                LlmError::ProviderQuota(_) => ExitCode::CLAUDE_FAILURE,
                LlmError::ProviderOutage(_) => ExitCode::CLAUDE_FAILURE,
                LlmError::Timeout { .. } => ExitCode::PHASE_TIMEOUT,
                LlmError::Misconfiguration(_) => ExitCode::CLI_ARGS,
                LlmError::Unsupported(_) => ExitCode::CLI_ARGS,
                LlmError::Transport(_) => ExitCode::CLAUDE_FAILURE,
                LlmError::BudgetExceeded { .. } => ExitCode::CLAUDE_FAILURE,
            },

            // All other errors default to exit code 1 (INTERNAL)
            _ => ExitCode::INTERNAL,
        }
    }
}

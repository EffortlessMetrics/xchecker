//! Exit code constants and error kind mapping for xchecker
//!
//! This module defines standardized exit codes for different failure modes
//! and provides mapping from `XCheckerError` to exit codes and error kinds.

use crate::error::XCheckerError;
use crate::types::ErrorKind;

/// Exit code constants for xchecker
pub mod codes {
    /// Success - operation completed successfully
    #[allow(dead_code)] // Used in tests (line 103)
    pub const SUCCESS: i32 = 0;

    /// CLI arguments error - invalid or missing command-line arguments
    pub const CLI_ARGS: i32 = 2;

    /// Packet overflow - input packet exceeded size limits before Claude invocation
    pub const PACKET_OVERFLOW: i32 = 7;

    /// Secret detected - redaction system detected potential secrets
    pub const SECRET_DETECTED: i32 = 8;

    /// Lock held - another process is already working on the same spec
    pub const LOCK_HELD: i32 = 9;

    /// Phase timeout - phase execution exceeded configured timeout
    pub const PHASE_TIMEOUT: i32 = 10;

    /// Claude failure - underlying Claude CLI invocation failed
    pub const CLAUDE_FAILURE: i32 = 70;
}

/// Convert `XCheckerError` to (`exit_code`, `error_kind`) tuple
#[allow(dead_code)] // Error handling utility for receipt generation
pub fn error_to_exit_code_and_kind(error: &XCheckerError) -> (i32, ErrorKind) {
    match error {
        // Configuration errors map to CLI_ARGS
        XCheckerError::Config(_) => (codes::CLI_ARGS, ErrorKind::CliArgs),

        // Packet overflow before Claude invocation
        XCheckerError::PacketOverflow { .. } => (codes::PACKET_OVERFLOW, ErrorKind::PacketOverflow),

        // Secret detection (redaction hard stop)
        XCheckerError::SecretDetected { .. } => (codes::SECRET_DETECTED, ErrorKind::SecretDetected),

        // Concurrent execution / lock held
        XCheckerError::ConcurrentExecution { .. } => (codes::LOCK_HELD, ErrorKind::LockHeld),
        XCheckerError::Lock(_) => (codes::LOCK_HELD, ErrorKind::LockHeld),

        // Phase errors
        XCheckerError::Phase(phase_err) => {
            use crate::error::PhaseError;
            match phase_err {
                PhaseError::Timeout { .. } => (codes::PHASE_TIMEOUT, ErrorKind::PhaseTimeout),
                // Invalid transitions are CLI argument errors (FR-ORC-001, FR-ORC-002)
                PhaseError::InvalidTransition { .. } => (codes::CLI_ARGS, ErrorKind::CliArgs),
                PhaseError::DependencyNotSatisfied { .. } => (codes::CLI_ARGS, ErrorKind::CliArgs),
                _ => (1, ErrorKind::Unknown),
            }
        }

        // Claude CLI failures
        XCheckerError::Claude(_) => (codes::CLAUDE_FAILURE, ErrorKind::ClaudeFailure),
        XCheckerError::Runner(_) => (codes::CLAUDE_FAILURE, ErrorKind::ClaudeFailure),

        // LLM backend errors
        XCheckerError::Llm(llm_err) => {
            use crate::llm::LlmError;
            match llm_err {
                LlmError::ProviderAuth(_) => (codes::CLAUDE_FAILURE, ErrorKind::ClaudeFailure),
                LlmError::ProviderQuota(_) => (codes::CLAUDE_FAILURE, ErrorKind::ClaudeFailure),
                LlmError::ProviderOutage(_) => (codes::CLAUDE_FAILURE, ErrorKind::ClaudeFailure),
                LlmError::Timeout { .. } => (codes::PHASE_TIMEOUT, ErrorKind::PhaseTimeout),
                LlmError::Misconfiguration(_) => (codes::CLI_ARGS, ErrorKind::CliArgs),
                LlmError::Unsupported(_) => (codes::CLI_ARGS, ErrorKind::CliArgs),
                LlmError::Transport(_) => (codes::CLAUDE_FAILURE, ErrorKind::ClaudeFailure),
                LlmError::BudgetExceeded { .. } => {
                    (codes::CLAUDE_FAILURE, ErrorKind::ClaudeFailure)
                }
            }
        }

        // All other errors default to exit code 1 with Unknown kind
        _ => (1, ErrorKind::Unknown),
    }
}

/// Convert `XCheckerError` to (`exit_code`, `error_kind`) tuple
impl From<&XCheckerError> for (i32, ErrorKind) {
    fn from(err: &XCheckerError) -> (i32, ErrorKind) {
        error_to_exit_code_and_kind(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{ClaudeError, ConfigError, PhaseError, RunnerError};
    use crate::lock::LockError;
    use crate::types::ErrorKind;

    #[test]
    fn test_exit_code_constants() {
        assert_eq!(codes::SUCCESS, 0);
        assert_eq!(codes::CLI_ARGS, 2);
        assert_eq!(codes::PACKET_OVERFLOW, 7);
        assert_eq!(codes::SECRET_DETECTED, 8);
        assert_eq!(codes::LOCK_HELD, 9);
        assert_eq!(codes::PHASE_TIMEOUT, 10);
        assert_eq!(codes::CLAUDE_FAILURE, 70);
    }

    #[test]
    fn test_error_kind_serialization() {
        // Test snake_case serialization
        let json = serde_json::to_string(&ErrorKind::CliArgs).unwrap();
        assert_eq!(json, r#""cli_args""#);

        let json = serde_json::to_string(&ErrorKind::PacketOverflow).unwrap();
        assert_eq!(json, r#""packet_overflow""#);

        let json = serde_json::to_string(&ErrorKind::SecretDetected).unwrap();
        assert_eq!(json, r#""secret_detected""#);

        let json = serde_json::to_string(&ErrorKind::LockHeld).unwrap();
        assert_eq!(json, r#""lock_held""#);

        let json = serde_json::to_string(&ErrorKind::PhaseTimeout).unwrap();
        assert_eq!(json, r#""phase_timeout""#);

        let json = serde_json::to_string(&ErrorKind::ClaudeFailure).unwrap();
        assert_eq!(json, r#""claude_failure""#);

        let json = serde_json::to_string(&ErrorKind::Unknown).unwrap();
        assert_eq!(json, r#""unknown""#);
    }

    #[test]
    fn test_config_error_mapping() {
        let err = XCheckerError::Config(ConfigError::InvalidFile("test".to_string()));
        let (code, kind) = (&err).into();
        assert_eq!(code, codes::CLI_ARGS);
        assert_eq!(kind, ErrorKind::CliArgs);
    }

    #[test]
    fn test_packet_overflow_mapping() {
        let err = XCheckerError::PacketOverflow {
            used_bytes: 100000,
            used_lines: 2000,
            limit_bytes: 65536,
            limit_lines: 1200,
        };
        let (code, kind) = (&err).into();
        assert_eq!(code, codes::PACKET_OVERFLOW);
        assert_eq!(kind, ErrorKind::PacketOverflow);
    }

    #[test]
    fn test_secret_detected_mapping() {
        let err = XCheckerError::SecretDetected {
            pattern: "ghp_".to_string(),
            location: "test.txt".to_string(),
        };
        let (code, kind) = (&err).into();
        assert_eq!(code, codes::SECRET_DETECTED);
        assert_eq!(kind, ErrorKind::SecretDetected);
    }

    #[test]
    fn test_concurrent_execution_mapping() {
        let err = XCheckerError::ConcurrentExecution {
            id: "test-spec".to_string(),
        };
        let (code, kind) = (&err).into();
        assert_eq!(code, codes::LOCK_HELD);
        assert_eq!(kind, ErrorKind::LockHeld);
    }

    #[test]
    fn test_lock_error_mapping() {
        let lock_err = LockError::ConcurrentExecution {
            spec_id: "test-spec".to_string(),
            pid: 12345,
            created_ago: "5m".to_string(),
        };
        let err = XCheckerError::Lock(lock_err);
        let (code, kind) = (&err).into();
        assert_eq!(code, codes::LOCK_HELD);
        assert_eq!(kind, ErrorKind::LockHeld);
    }

    #[test]
    fn test_phase_timeout_mapping() {
        let phase_err = PhaseError::Timeout {
            phase: "REQUIREMENTS".to_string(),
            timeout_seconds: 600,
        };
        let err = XCheckerError::Phase(phase_err);
        let (code, kind) = (&err).into();
        assert_eq!(code, codes::PHASE_TIMEOUT);
        assert_eq!(kind, ErrorKind::PhaseTimeout);
    }

    #[test]
    fn test_phase_non_timeout_mapping() {
        let phase_err = PhaseError::ExecutionFailed {
            phase: "DESIGN".to_string(),
            code: 1,
        };
        let err = XCheckerError::Phase(phase_err);
        let (code, kind) = (&err).into();
        assert_eq!(code, 1);
        assert_eq!(kind, ErrorKind::Unknown);
    }

    #[test]
    fn test_claude_error_mapping() {
        let claude_err = ClaudeError::ExecutionFailed {
            stderr: "API error".to_string(),
        };
        let err = XCheckerError::Claude(claude_err);
        let (code, kind) = (&err).into();
        assert_eq!(code, codes::CLAUDE_FAILURE);
        assert_eq!(kind, ErrorKind::ClaudeFailure);
    }

    #[test]
    fn test_runner_error_mapping() {
        let runner_err = RunnerError::NativeExecutionFailed {
            reason: "command not found".to_string(),
        };
        let err = XCheckerError::Runner(runner_err);
        let (code, kind) = (&err).into();
        assert_eq!(code, codes::CLAUDE_FAILURE);
        assert_eq!(kind, ErrorKind::ClaudeFailure);
    }

    #[test]
    fn test_io_error_mapping() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err = XCheckerError::Io(io_err);
        let (code, kind) = (&err).into();
        assert_eq!(code, 1);
        assert_eq!(kind, ErrorKind::Unknown);
    }

    #[test]
    fn test_invalid_transition_mapping() {
        let phase_err = PhaseError::InvalidTransition {
            from: "none".to_string(),
            to: "design".to_string(),
        };
        let err = XCheckerError::Phase(phase_err);
        let (code, kind) = (&err).into();
        assert_eq!(code, codes::CLI_ARGS);
        assert_eq!(kind, ErrorKind::CliArgs);
    }

    #[test]
    fn test_dependency_not_satisfied_mapping() {
        let phase_err = PhaseError::DependencyNotSatisfied {
            phase: "design".to_string(),
            dependency: "requirements".to_string(),
        };
        let err = XCheckerError::Phase(phase_err);
        let (code, kind) = (&err).into();
        assert_eq!(code, codes::CLI_ARGS);
        assert_eq!(kind, ErrorKind::CliArgs);
    }

    #[test]
    fn test_llm_provider_auth_mapping() {
        use crate::llm::LlmError;
        let llm_err = LlmError::ProviderAuth("Invalid API key".to_string());
        let err = XCheckerError::Llm(llm_err);
        let (code, kind) = (&err).into();
        assert_eq!(code, codes::CLAUDE_FAILURE);
        assert_eq!(kind, ErrorKind::ClaudeFailure);
    }

    #[test]
    fn test_llm_provider_quota_mapping() {
        use crate::llm::LlmError;
        let llm_err = LlmError::ProviderQuota("Rate limit exceeded".to_string());
        let err = XCheckerError::Llm(llm_err);
        let (code, kind) = (&err).into();
        assert_eq!(code, codes::CLAUDE_FAILURE);
        assert_eq!(kind, ErrorKind::ClaudeFailure);
    }

    #[test]
    fn test_llm_provider_outage_mapping() {
        use crate::llm::LlmError;
        let llm_err = LlmError::ProviderOutage("Service unavailable".to_string());
        let err = XCheckerError::Llm(llm_err);
        let (code, kind) = (&err).into();
        assert_eq!(code, codes::CLAUDE_FAILURE);
        assert_eq!(kind, ErrorKind::ClaudeFailure);
    }

    #[test]
    fn test_llm_timeout_mapping() {
        use crate::llm::LlmError;
        use std::time::Duration;
        let llm_err = LlmError::Timeout {
            duration: Duration::from_secs(300),
        };
        let err = XCheckerError::Llm(llm_err);
        let (code, kind) = (&err).into();
        assert_eq!(code, codes::PHASE_TIMEOUT);
        assert_eq!(kind, ErrorKind::PhaseTimeout);
    }

    #[test]
    fn test_llm_misconfiguration_mapping() {
        use crate::llm::LlmError;
        let llm_err = LlmError::Misconfiguration("Missing provider config".to_string());
        let err = XCheckerError::Llm(llm_err);
        let (code, kind) = (&err).into();
        assert_eq!(code, codes::CLI_ARGS);
        assert_eq!(kind, ErrorKind::CliArgs);
    }

    #[test]
    fn test_llm_unsupported_mapping() {
        use crate::llm::LlmError;
        let llm_err = LlmError::Unsupported("ExternalTool not yet supported".to_string());
        let err = XCheckerError::Llm(llm_err);
        let (code, kind) = (&err).into();
        assert_eq!(code, codes::CLI_ARGS);
        assert_eq!(kind, ErrorKind::CliArgs);
    }

    #[test]
    fn test_llm_transport_mapping() {
        use crate::llm::LlmError;
        let llm_err = LlmError::Transport("Connection failed".to_string());
        let err = XCheckerError::Llm(llm_err);
        let (code, kind) = (&err).into();
        assert_eq!(code, codes::CLAUDE_FAILURE);
        assert_eq!(kind, ErrorKind::ClaudeFailure);
    }

    #[test]
    fn test_llm_budget_exceeded_mapping() {
        use crate::llm::LlmError;
        let llm_err = LlmError::BudgetExceeded {
            limit: 20,
            attempted: 21,
        };
        let err = XCheckerError::Llm(llm_err);
        let (code, kind) = (&err).into();
        assert_eq!(code, codes::CLAUDE_FAILURE);
        assert_eq!(kind, ErrorKind::ClaudeFailure);
    }
}

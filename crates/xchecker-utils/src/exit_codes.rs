//! Exit code constants and error kind mapping for xchecker.
//!
//! This module defines standardized exit codes for different failure modes
//! and provides mapping from `XCheckerError` to exit codes and error kinds.
//!
//! # Exit Code Table
//!
//! | Code | Constant | Description |
//! |------|----------|-------------|
//! | 0 | `SUCCESS` | Operation completed successfully |
//! | 1 | `INTERNAL` | General/internal failure |
//! | 2 | `CLI_ARGS` | Invalid CLI arguments or configuration |
//! | 7 | `PACKET_OVERFLOW` | Input packet exceeded size limits |
//! | 8 | `SECRET_DETECTED` | Secret found in content (security) |
//! | 9 | `LOCK_HELD` | Another process holds the lock |
//! | 10 | `PHASE_TIMEOUT` | Phase execution timed out |
//! | 70 | `CLAUDE_FAILURE` | Claude CLI invocation failed |

use crate::error::XCheckerError;
use crate::types::ErrorKind;

/// Exit codes matching the documented exit code table.
///
/// `ExitCode` provides type-safe exit code handling for xchecker operations.
/// Use the named constants for common exit codes, or [`as_i32()`](Self::as_i32)
/// to get the numeric value for `std::process::exit()`.
///
/// This is a stable public type. The numeric values are part of the public API
/// and will not change in 1.x releases.
///
/// # Constants
///
/// | Constant | Value | Description |
/// |----------|-------|-------------|
/// | [`SUCCESS`](Self::SUCCESS) | 0 | Operation completed successfully |
/// | [`INTERNAL`](Self::INTERNAL) | 1 | General/internal failure |
/// | [`CLI_ARGS`](Self::CLI_ARGS) | 2 | Invalid CLI arguments |
/// | [`PACKET_OVERFLOW`](Self::PACKET_OVERFLOW) | 7 | Packet size exceeded |
/// | [`SECRET_DETECTED`](Self::SECRET_DETECTED) | 8 | Secret found in content |
/// | [`LOCK_HELD`](Self::LOCK_HELD) | 9 | Lock already held |
/// | [`PHASE_TIMEOUT`](Self::PHASE_TIMEOUT) | 10 | Phase timed out |
/// | [`CLAUDE_FAILURE`](Self::CLAUDE_FAILURE) | 70 | Claude CLI failed |
///
/// # Example
///
/// ```rust
/// use xchecker_utils::exit_codes::ExitCode;
///
/// // Using named constants
/// let code = ExitCode::SUCCESS;
/// assert_eq!(code.as_i32(), 0);
///
/// let code = ExitCode::PACKET_OVERFLOW;
/// assert_eq!(code.as_i32(), 7);
///
/// // Comparing exit codes
/// assert_eq!(ExitCode::SUCCESS, ExitCode::from_i32(0));
/// ```
///
/// # Integration with XCheckerError
///
/// Use [`XCheckerError::to_exit_code()`](crate::XCheckerError::to_exit_code) to map
/// errors to exit codes:
///
/// ```rust
/// use xchecker_utils::error::ConfigError;
/// use xchecker_utils::error::XCheckerError;
/// use xchecker_utils::exit_codes::ExitCode;
///
/// let err = XCheckerError::Config(ConfigError::InvalidFile("test".to_string()));
/// assert_eq!(err.to_exit_code(), ExitCode::CLI_ARGS);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExitCode(i32);

impl ExitCode {
    /// Success - operation completed successfully
    pub const SUCCESS: ExitCode = ExitCode(0);

    /// CLI arguments error - invalid or missing command-line arguments
    pub const CLI_ARGS: ExitCode = ExitCode(2);

    /// Packet overflow - input packet exceeded size limits before Claude invocation
    pub const PACKET_OVERFLOW: ExitCode = ExitCode(7);

    /// Secret detected - redaction system detected potential secrets
    pub const SECRET_DETECTED: ExitCode = ExitCode(8);

    /// Lock held - another process is already working on the same spec
    pub const LOCK_HELD: ExitCode = ExitCode(9);

    /// Phase timeout - phase execution exceeded configured timeout
    pub const PHASE_TIMEOUT: ExitCode = ExitCode(10);

    /// Claude failure - underlying Claude CLI invocation failed
    pub const CLAUDE_FAILURE: ExitCode = ExitCode(70);

    /// Internal error - general failure
    pub const INTERNAL: ExitCode = ExitCode(1);

    /// Get the numeric exit code value.
    ///
    /// Use this with `std::process::exit()`.
    #[must_use]
    pub const fn as_i32(self) -> i32 {
        self.0
    }

    /// Create an ExitCode from a raw i32 value.
    ///
    /// Prefer using the named constants when possible.
    #[must_use]
    pub const fn from_i32(code: i32) -> Self {
        ExitCode(code)
    }
}

impl From<i32> for ExitCode {
    fn from(code: i32) -> Self {
        ExitCode(code)
    }
}

impl From<ExitCode> for i32 {
    fn from(code: ExitCode) -> Self {
        code.0
    }
}

/// Exit code constants for xchecker (legacy module for backward compatibility)
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
            use crate::error::LlmError;
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
        use crate::error::LlmError;
        let llm_err = LlmError::ProviderAuth("Invalid API key".to_string());
        let err = XCheckerError::Llm(llm_err);
        let (code, kind) = (&err).into();
        assert_eq!(code, codes::CLAUDE_FAILURE);
        assert_eq!(kind, ErrorKind::ClaudeFailure);
    }

    #[test]
    fn test_llm_provider_quota_mapping() {
        use crate::error::LlmError;
        let llm_err = LlmError::ProviderQuota("Rate limit exceeded".to_string());
        let err = XCheckerError::Llm(llm_err);
        let (code, kind) = (&err).into();
        assert_eq!(code, codes::CLAUDE_FAILURE);
        assert_eq!(kind, ErrorKind::ClaudeFailure);
    }

    #[test]
    fn test_llm_provider_outage_mapping() {
        use crate::error::LlmError;
        let llm_err = LlmError::ProviderOutage("Service unavailable".to_string());
        let err = XCheckerError::Llm(llm_err);
        let (code, kind) = (&err).into();
        assert_eq!(code, codes::CLAUDE_FAILURE);
        assert_eq!(kind, ErrorKind::ClaudeFailure);
    }

    #[test]
    fn test_llm_timeout_mapping() {
        use crate::error::LlmError;
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
        use crate::error::LlmError;
        let llm_err = LlmError::Misconfiguration("Missing provider config".to_string());
        let err = XCheckerError::Llm(llm_err);
        let (code, kind) = (&err).into();
        assert_eq!(code, codes::CLI_ARGS);
        assert_eq!(kind, ErrorKind::CliArgs);
    }

    #[test]
    fn test_llm_unsupported_mapping() {
        use crate::error::LlmError;
        let llm_err = LlmError::Unsupported("ExternalTool not yet supported".to_string());
        let err = XCheckerError::Llm(llm_err);
        let (code, kind) = (&err).into();
        assert_eq!(code, codes::CLI_ARGS);
        assert_eq!(kind, ErrorKind::CliArgs);
    }

    #[test]
    fn test_llm_transport_mapping() {
        use crate::error::LlmError;
        let llm_err = LlmError::Transport("Connection failed".to_string());
        let err = XCheckerError::Llm(llm_err);
        let (code, kind) = (&err).into();
        assert_eq!(code, codes::CLAUDE_FAILURE);
        assert_eq!(kind, ErrorKind::ClaudeFailure);
    }

    #[test]
    fn test_llm_budget_exceeded_mapping() {
        use crate::error::LlmError;
        let llm_err = LlmError::BudgetExceeded {
            limit: 20,
            attempted: 21,
        };
        let err = XCheckerError::Llm(llm_err);
        let (code, kind) = (&err).into();
        assert_eq!(code, codes::CLAUDE_FAILURE);
        assert_eq!(kind, ErrorKind::ClaudeFailure);
    }

    #[test]
    fn test_validation_failed_mapping() {
        // ValidationFailed maps to exit code 1 (general error) with Unknown kind
        // This is intentional: validation failures are user-recoverable errors
        // that don't fit into infrastructure-level categories
        use crate::error::ValidationError;
        let err = XCheckerError::ValidationFailed {
            phase: "requirements".to_string(),
            issue_count: 2,
            issues: vec![
                ValidationError::MetaSummaryDetected {
                    pattern: "Here is".to_string(),
                },
                ValidationError::TooShort {
                    actual: 10,
                    minimum: 30,
                },
            ],
        };
        let (code, kind) = (&err).into();
        assert_eq!(
            code, 1,
            "ValidationFailed should use exit code 1 (general error)"
        );
        assert_eq!(kind, ErrorKind::Unknown);
    }

    // ========================================================================
    // Tests for XCheckerError::to_exit_code() method
    // These tests verify the method returns ExitCode struct with correct values
    // ========================================================================

    #[test]
    fn test_to_exit_code_config_error() {
        let err = XCheckerError::Config(ConfigError::InvalidFile("test".to_string()));
        assert_eq!(err.to_exit_code(), ExitCode::CLI_ARGS);
        assert_eq!(err.to_exit_code().as_i32(), codes::CLI_ARGS);
    }

    #[test]
    fn test_to_exit_code_packet_overflow() {
        let err = XCheckerError::PacketOverflow {
            used_bytes: 100000,
            used_lines: 2000,
            limit_bytes: 65536,
            limit_lines: 1200,
        };
        assert_eq!(err.to_exit_code(), ExitCode::PACKET_OVERFLOW);
        assert_eq!(err.to_exit_code().as_i32(), codes::PACKET_OVERFLOW);
    }

    #[test]
    fn test_to_exit_code_secret_detected() {
        let err = XCheckerError::SecretDetected {
            pattern: "ghp_".to_string(),
            location: "test.txt".to_string(),
        };
        assert_eq!(err.to_exit_code(), ExitCode::SECRET_DETECTED);
        assert_eq!(err.to_exit_code().as_i32(), codes::SECRET_DETECTED);
    }

    #[test]
    fn test_to_exit_code_concurrent_execution() {
        let err = XCheckerError::ConcurrentExecution {
            id: "test-spec".to_string(),
        };
        assert_eq!(err.to_exit_code(), ExitCode::LOCK_HELD);
        assert_eq!(err.to_exit_code().as_i32(), codes::LOCK_HELD);
    }

    #[test]
    fn test_to_exit_code_lock_error() {
        let lock_err = LockError::ConcurrentExecution {
            spec_id: "test-spec".to_string(),
            pid: 12345,
            created_ago: "5m".to_string(),
        };
        let err = XCheckerError::Lock(lock_err);
        assert_eq!(err.to_exit_code(), ExitCode::LOCK_HELD);
        assert_eq!(err.to_exit_code().as_i32(), codes::LOCK_HELD);
    }

    #[test]
    fn test_to_exit_code_phase_timeout() {
        let phase_err = PhaseError::Timeout {
            phase: "REQUIREMENTS".to_string(),
            timeout_seconds: 600,
        };
        let err = XCheckerError::Phase(phase_err);
        assert_eq!(err.to_exit_code(), ExitCode::PHASE_TIMEOUT);
        assert_eq!(err.to_exit_code().as_i32(), codes::PHASE_TIMEOUT);
    }

    #[test]
    fn test_to_exit_code_phase_invalid_transition() {
        let phase_err = PhaseError::InvalidTransition {
            from: "none".to_string(),
            to: "design".to_string(),
        };
        let err = XCheckerError::Phase(phase_err);
        assert_eq!(err.to_exit_code(), ExitCode::CLI_ARGS);
        assert_eq!(err.to_exit_code().as_i32(), codes::CLI_ARGS);
    }

    #[test]
    fn test_to_exit_code_phase_dependency_not_satisfied() {
        let phase_err = PhaseError::DependencyNotSatisfied {
            phase: "design".to_string(),
            dependency: "requirements".to_string(),
        };
        let err = XCheckerError::Phase(phase_err);
        assert_eq!(err.to_exit_code(), ExitCode::CLI_ARGS);
        assert_eq!(err.to_exit_code().as_i32(), codes::CLI_ARGS);
    }

    #[test]
    fn test_to_exit_code_phase_execution_failed() {
        let phase_err = PhaseError::ExecutionFailed {
            phase: "DESIGN".to_string(),
            code: 1,
        };
        let err = XCheckerError::Phase(phase_err);
        assert_eq!(err.to_exit_code(), ExitCode::INTERNAL);
        assert_eq!(err.to_exit_code().as_i32(), 1);
    }

    #[test]
    fn test_to_exit_code_claude_error() {
        let claude_err = ClaudeError::ExecutionFailed {
            stderr: "API error".to_string(),
        };
        let err = XCheckerError::Claude(claude_err);
        assert_eq!(err.to_exit_code(), ExitCode::CLAUDE_FAILURE);
        assert_eq!(err.to_exit_code().as_i32(), codes::CLAUDE_FAILURE);
    }

    #[test]
    fn test_to_exit_code_runner_error() {
        let runner_err = RunnerError::NativeExecutionFailed {
            reason: "command not found".to_string(),
        };
        let err = XCheckerError::Runner(runner_err);
        assert_eq!(err.to_exit_code(), ExitCode::CLAUDE_FAILURE);
        assert_eq!(err.to_exit_code().as_i32(), codes::CLAUDE_FAILURE);
    }

    #[test]
    fn test_to_exit_code_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err = XCheckerError::Io(io_err);
        assert_eq!(err.to_exit_code(), ExitCode::INTERNAL);
        assert_eq!(err.to_exit_code().as_i32(), 1);
    }

    #[test]
    fn test_to_exit_code_llm_timeout() {
        use crate::error::LlmError;
        use std::time::Duration;
        let llm_err = LlmError::Timeout {
            duration: Duration::from_secs(300),
        };
        let err = XCheckerError::Llm(llm_err);
        assert_eq!(err.to_exit_code(), ExitCode::PHASE_TIMEOUT);
        assert_eq!(err.to_exit_code().as_i32(), codes::PHASE_TIMEOUT);
    }

    #[test]
    fn test_to_exit_code_llm_misconfiguration() {
        use crate::error::LlmError;
        let llm_err = LlmError::Misconfiguration("Missing provider config".to_string());
        let err = XCheckerError::Llm(llm_err);
        assert_eq!(err.to_exit_code(), ExitCode::CLI_ARGS);
        assert_eq!(err.to_exit_code().as_i32(), codes::CLI_ARGS);
    }

    #[test]
    fn test_to_exit_code_llm_provider_auth() {
        use crate::error::LlmError;
        let llm_err = LlmError::ProviderAuth("Invalid API key".to_string());
        let err = XCheckerError::Llm(llm_err);
        assert_eq!(err.to_exit_code(), ExitCode::CLAUDE_FAILURE);
        assert_eq!(err.to_exit_code().as_i32(), codes::CLAUDE_FAILURE);
    }

    #[test]
    fn test_to_exit_code_consistency_with_error_to_exit_code_and_kind() {
        // Verify that to_exit_code() returns the same exit code as error_to_exit_code_and_kind()
        // This ensures consistency between the two APIs

        let test_cases: Vec<XCheckerError> = vec![
            XCheckerError::Config(ConfigError::InvalidFile("test".to_string())),
            XCheckerError::PacketOverflow {
                used_bytes: 100000,
                used_lines: 2000,
                limit_bytes: 65536,
                limit_lines: 1200,
            },
            XCheckerError::SecretDetected {
                pattern: "ghp_".to_string(),
                location: "test.txt".to_string(),
            },
            XCheckerError::ConcurrentExecution {
                id: "test-spec".to_string(),
            },
            XCheckerError::Claude(ClaudeError::ExecutionFailed {
                stderr: "API error".to_string(),
            }),
            XCheckerError::Runner(RunnerError::NativeExecutionFailed {
                reason: "command not found".to_string(),
            }),
            XCheckerError::Phase(PhaseError::Timeout {
                phase: "REQUIREMENTS".to_string(),
                timeout_seconds: 600,
            }),
            XCheckerError::Phase(PhaseError::InvalidTransition {
                from: "none".to_string(),
                to: "design".to_string(),
            }),
        ];

        for err in test_cases {
            let (legacy_code, _kind) = error_to_exit_code_and_kind(&err);
            let new_code = err.to_exit_code().as_i32();
            assert_eq!(
                legacy_code, new_code,
                "Exit code mismatch for error: {:?}",
                err
            );
        }
    }

    // ========================================================================
    // Comprehensive ErrorKind to ExitCode mapping test
    // Validates: Requirements 2.3, 5.2, 10.5
    // ========================================================================

    /// Test that each ErrorKind maps to the correct ExitCode as documented.
    ///
    /// This test validates the documented exit code table:
    /// | Code | Name           | ErrorKind      | Description                |
    /// |------|----------------|----------------|----------------------------|
    /// | 0    | SUCCESS        | (none)         | Completed successfully     |
    /// | 1    | INTERNAL       | Unknown        | General failure            |
    /// | 2    | CLI_ARGS       | CliArgs        | Invalid CLI arguments      |
    /// | 7    | PACKET_OVERFLOW| PacketOverflow | Packet size exceeded       |
    /// | 8    | SECRET_DETECTED| SecretDetected | Secret found in content    |
    /// | 9    | LOCK_HELD      | LockHeld       | Lock already held          |
    /// | 10   | PHASE_TIMEOUT  | PhaseTimeout   | Phase timed out            |
    /// | 70   | CLAUDE_FAILURE | ClaudeFailure  | Claude CLI failed          |
    ///
    /// **Validates: Requirements 2.3, 5.2, 10.5**
    #[test]
    fn test_error_kind_to_exit_code_mapping() {
        // Test each ErrorKind variant maps to the correct exit code
        // This uses explicit test cases as required by the task

        // ErrorKind::CliArgs -> ExitCode::CLI_ARGS (2)
        let err = XCheckerError::Config(ConfigError::InvalidFile("test".to_string()));
        let (code, kind) = error_to_exit_code_and_kind(&err);
        assert_eq!(
            kind,
            ErrorKind::CliArgs,
            "Config error should produce CliArgs kind"
        );
        assert_eq!(code, codes::CLI_ARGS, "CliArgs should map to exit code 2");
        assert_eq!(code, 2, "CLI_ARGS constant should be 2");

        // ErrorKind::PacketOverflow -> ExitCode::PACKET_OVERFLOW (7)
        let err = XCheckerError::PacketOverflow {
            used_bytes: 100000,
            used_lines: 2000,
            limit_bytes: 65536,
            limit_lines: 1200,
        };
        let (code, kind) = error_to_exit_code_and_kind(&err);
        assert_eq!(
            kind,
            ErrorKind::PacketOverflow,
            "PacketOverflow error should produce PacketOverflow kind"
        );
        assert_eq!(
            code,
            codes::PACKET_OVERFLOW,
            "PacketOverflow should map to exit code 7"
        );
        assert_eq!(code, 7, "PACKET_OVERFLOW constant should be 7");

        // ErrorKind::SecretDetected -> ExitCode::SECRET_DETECTED (8)
        let err = XCheckerError::SecretDetected {
            pattern: "ghp_".to_string(),
            location: "test.txt".to_string(),
        };
        let (code, kind) = error_to_exit_code_and_kind(&err);
        assert_eq!(
            kind,
            ErrorKind::SecretDetected,
            "SecretDetected error should produce SecretDetected kind"
        );
        assert_eq!(
            code,
            codes::SECRET_DETECTED,
            "SecretDetected should map to exit code 8"
        );
        assert_eq!(code, 8, "SECRET_DETECTED constant should be 8");

        // ErrorKind::LockHeld -> ExitCode::LOCK_HELD (9)
        let err = XCheckerError::ConcurrentExecution {
            id: "test-spec".to_string(),
        };
        let (code, kind) = error_to_exit_code_and_kind(&err);
        assert_eq!(
            kind,
            ErrorKind::LockHeld,
            "ConcurrentExecution error should produce LockHeld kind"
        );
        assert_eq!(code, codes::LOCK_HELD, "LockHeld should map to exit code 9");
        assert_eq!(code, 9, "LOCK_HELD constant should be 9");

        // ErrorKind::PhaseTimeout -> ExitCode::PHASE_TIMEOUT (10)
        let err = XCheckerError::Phase(PhaseError::Timeout {
            phase: "REQUIREMENTS".to_string(),
            timeout_seconds: 600,
        });
        let (code, kind) = error_to_exit_code_and_kind(&err);
        assert_eq!(
            kind,
            ErrorKind::PhaseTimeout,
            "Phase timeout error should produce PhaseTimeout kind"
        );
        assert_eq!(
            code,
            codes::PHASE_TIMEOUT,
            "PhaseTimeout should map to exit code 10"
        );
        assert_eq!(code, 10, "PHASE_TIMEOUT constant should be 10");

        // ErrorKind::ClaudeFailure -> ExitCode::CLAUDE_FAILURE (70)
        let err = XCheckerError::Claude(ClaudeError::ExecutionFailed {
            stderr: "API error".to_string(),
        });
        let (code, kind) = error_to_exit_code_and_kind(&err);
        assert_eq!(
            kind,
            ErrorKind::ClaudeFailure,
            "Claude error should produce ClaudeFailure kind"
        );
        assert_eq!(
            code,
            codes::CLAUDE_FAILURE,
            "ClaudeFailure should map to exit code 70"
        );
        assert_eq!(code, 70, "CLAUDE_FAILURE constant should be 70");

        // ErrorKind::Unknown -> ExitCode::INTERNAL (1)
        let err = XCheckerError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ));
        let (code, kind) = error_to_exit_code_and_kind(&err);
        assert_eq!(
            kind,
            ErrorKind::Unknown,
            "IO error should produce Unknown kind"
        );
        assert_eq!(code, 1, "Unknown should map to exit code 1 (INTERNAL)");
    }

    /// Test that ExitCode constants match the documented values.
    ///
    /// This validates the ExitCode struct constants against the documented table.
    /// **Validates: Requirements 2.3, 5.2, 10.5**
    #[test]
    fn test_exit_code_struct_constants_match_documented_values() {
        // Verify ExitCode struct constants match the documented exit code table
        assert_eq!(ExitCode::SUCCESS.as_i32(), 0, "SUCCESS should be 0");
        assert_eq!(ExitCode::INTERNAL.as_i32(), 1, "INTERNAL should be 1");
        assert_eq!(ExitCode::CLI_ARGS.as_i32(), 2, "CLI_ARGS should be 2");
        assert_eq!(
            ExitCode::PACKET_OVERFLOW.as_i32(),
            7,
            "PACKET_OVERFLOW should be 7"
        );
        assert_eq!(
            ExitCode::SECRET_DETECTED.as_i32(),
            8,
            "SECRET_DETECTED should be 8"
        );
        assert_eq!(ExitCode::LOCK_HELD.as_i32(), 9, "LOCK_HELD should be 9");
        assert_eq!(
            ExitCode::PHASE_TIMEOUT.as_i32(),
            10,
            "PHASE_TIMEOUT should be 10"
        );
        assert_eq!(
            ExitCode::CLAUDE_FAILURE.as_i32(),
            70,
            "CLAUDE_FAILURE should be 70"
        );

        // Verify codes module constants match ExitCode struct constants
        assert_eq!(
            codes::SUCCESS,
            ExitCode::SUCCESS.as_i32(),
            "codes::SUCCESS should match ExitCode::SUCCESS"
        );
        assert_eq!(
            codes::CLI_ARGS,
            ExitCode::CLI_ARGS.as_i32(),
            "codes::CLI_ARGS should match ExitCode::CLI_ARGS"
        );
        assert_eq!(
            codes::PACKET_OVERFLOW,
            ExitCode::PACKET_OVERFLOW.as_i32(),
            "codes::PACKET_OVERFLOW should match ExitCode::PACKET_OVERFLOW"
        );
        assert_eq!(
            codes::SECRET_DETECTED,
            ExitCode::SECRET_DETECTED.as_i32(),
            "codes::SECRET_DETECTED should match ExitCode::SECRET_DETECTED"
        );
        assert_eq!(
            codes::LOCK_HELD,
            ExitCode::LOCK_HELD.as_i32(),
            "codes::LOCK_HELD should match ExitCode::LOCK_HELD"
        );
        assert_eq!(
            codes::PHASE_TIMEOUT,
            ExitCode::PHASE_TIMEOUT.as_i32(),
            "codes::PHASE_TIMEOUT should match ExitCode::PHASE_TIMEOUT"
        );
        assert_eq!(
            codes::CLAUDE_FAILURE,
            ExitCode::CLAUDE_FAILURE.as_i32(),
            "codes::CLAUDE_FAILURE should match ExitCode::CLAUDE_FAILURE"
        );
    }

    /// Test that to_exit_code() method returns correct ExitCode for each error type.
    ///
    /// This validates the XCheckerError::to_exit_code() method against the documented table.
    /// **Validates: Requirements 2.3, 5.2, 10.5**
    #[test]
    fn test_to_exit_code_matches_documented_table() {
        // Test each documented error type maps to the correct ExitCode via to_exit_code()

        // Config errors -> CLI_ARGS (2)
        let err = XCheckerError::Config(ConfigError::InvalidFile("test".to_string()));
        assert_eq!(err.to_exit_code(), ExitCode::CLI_ARGS);

        // PacketOverflow -> PACKET_OVERFLOW (7)
        let err = XCheckerError::PacketOverflow {
            used_bytes: 100000,
            used_lines: 2000,
            limit_bytes: 65536,
            limit_lines: 1200,
        };
        assert_eq!(err.to_exit_code(), ExitCode::PACKET_OVERFLOW);

        // SecretDetected -> SECRET_DETECTED (8)
        let err = XCheckerError::SecretDetected {
            pattern: "ghp_".to_string(),
            location: "test.txt".to_string(),
        };
        assert_eq!(err.to_exit_code(), ExitCode::SECRET_DETECTED);

        // ConcurrentExecution -> LOCK_HELD (9)
        let err = XCheckerError::ConcurrentExecution {
            id: "test-spec".to_string(),
        };
        assert_eq!(err.to_exit_code(), ExitCode::LOCK_HELD);

        // Lock errors -> LOCK_HELD (9)
        let lock_err = LockError::ConcurrentExecution {
            spec_id: "test-spec".to_string(),
            pid: 12345,
            created_ago: "5m".to_string(),
        };
        let err = XCheckerError::Lock(lock_err);
        assert_eq!(err.to_exit_code(), ExitCode::LOCK_HELD);

        // Phase timeout -> PHASE_TIMEOUT (10)
        let phase_err = PhaseError::Timeout {
            phase: "REQUIREMENTS".to_string(),
            timeout_seconds: 600,
        };
        let err = XCheckerError::Phase(phase_err);
        assert_eq!(err.to_exit_code(), ExitCode::PHASE_TIMEOUT);

        // Claude errors -> CLAUDE_FAILURE (70)
        let claude_err = ClaudeError::ExecutionFailed {
            stderr: "API error".to_string(),
        };
        let err = XCheckerError::Claude(claude_err);
        assert_eq!(err.to_exit_code(), ExitCode::CLAUDE_FAILURE);

        // Runner errors -> CLAUDE_FAILURE (70)
        let runner_err = RunnerError::NativeExecutionFailed {
            reason: "command not found".to_string(),
        };
        let err = XCheckerError::Runner(runner_err);
        assert_eq!(err.to_exit_code(), ExitCode::CLAUDE_FAILURE);

        // IO errors -> INTERNAL (1)
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err = XCheckerError::Io(io_err);
        assert_eq!(err.to_exit_code(), ExitCode::INTERNAL);
    }
}

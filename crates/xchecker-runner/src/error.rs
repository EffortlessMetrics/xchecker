//! Error types for runner module

use thiserror::Error;

/// Runner execution errors for cross-platform Claude CLI execution
#[derive(Error, Debug)]
pub enum RunnerError {
    #[error("Runner detection failed: {reason}")]
    DetectionFailed { reason: String },

    #[error("WSL not available: {reason}")]
    WslNotAvailable { reason: String },

    #[error("WSL execution failed: {reason}")]
    WslExecutionFailed { reason: String },

    #[error("Native execution failed: {reason}")]
    NativeExecutionFailed { reason: String },

    #[error("Runner configuration invalid: {reason}")]
    ConfigurationInvalid { reason: String },

    #[error("Claude CLI not found in runner environment: {runner}")]
    ClaudeNotFoundInRunner { runner: String },

    #[error("Execution timed out after {timeout_seconds} seconds")]
    Timeout { timeout_seconds: u64 },
}

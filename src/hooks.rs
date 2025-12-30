//! Hooks system: implemented and tested, not wired into orchestrator in v1.0.
//! See FR-HOOKS for design rationale. Will be integrated in a future release.
//!
//! Hook system for executing scripts at key points in the xchecker workflow.
//!
//! Hooks allow users to run custom scripts before and after phase execution.
//! They are configured in `.xchecker/config.toml` under the `[hooks]` section.
//!
//! # Configuration
//!
//! ```toml
//! [hooks.pre_phase.design]
//! command = "./scripts/pre_design.sh"
//! on_fail = "warn"  # or "fail"
//! timeout = 60      # optional, defaults to 60 seconds
//!
//! [hooks.post_phase.requirements]
//! command = "./scripts/post_requirements.sh"
//! on_fail = "fail"
//! ```
//!
//! # Environment Variables
//!
//! Hooks receive context via environment variables:
//! - `XCHECKER_SPEC_ID`: The spec identifier
//! - `XCHECKER_PHASE`: The phase name (e.g., "requirements", "design")
//! - `XCHECKER_HOOK_TYPE`: Either "pre_phase" or "post_phase"
//!
//! # Failure Handling
//!
//! - `on_fail = "warn"` (default): Hook failures are logged and recorded in receipts
//!   but do not fail the phase.
//! - `on_fail = "fail"`: A non-zero hook exit code fails the phase with a clear error.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use thiserror::Error;
use tokio::io::AsyncWriteExt;
use crate::runner::CommandSpec;
use crate::types::PhaseId;

/// Default timeout for hook execution in seconds
pub const DEFAULT_HOOK_TIMEOUT_SECS: u64 = 60;

/// Hook failure behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OnFail {
    /// Log warning and continue (default)
    #[default]
    Warn,
    /// Fail the phase on hook failure
    Fail,
}

impl std::fmt::Display for OnFail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Warn => write!(f, "warn"),
            Self::Fail => write!(f, "fail"),
        }
    }
}

/// Hook type indicating when the hook runs
/// Reserved for hooks integration; not wired in v1.0
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookType {
    /// Runs before phase execution
    PrePhase,
    /// Runs after phase execution
    PostPhase,
}

impl std::fmt::Display for HookType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PrePhase => write!(f, "pre_phase"),
            Self::PostPhase => write!(f, "post_phase"),
        }
    }
}

/// Configuration for a single hook
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HookConfig {
    /// Command to execute (can be a script path or shell command)
    pub command: String,
    /// Behavior on hook failure (default: warn)
    #[serde(default)]
    pub on_fail: OnFail,
    /// Timeout in seconds (default: 60)
    #[serde(default = "default_timeout")]
    pub timeout: u64,
}

fn default_timeout() -> u64 {
    DEFAULT_HOOK_TIMEOUT_SECS
}

/// Hooks configuration section from config.toml
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct HooksConfig {
    /// Pre-phase hooks keyed by phase name
    #[serde(default)]
    pub pre_phase: HashMap<String, HookConfig>,
    /// Post-phase hooks keyed by phase name
    #[serde(default)]
    pub post_phase: HashMap<String, HookConfig>,
}

impl HooksConfig {
    /// Get a pre-phase hook for the given phase
    /// Reserved for hooks integration; not wired in v1.0
    #[must_use]
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn get_pre_phase_hook(&self, phase: PhaseId) -> Option<&HookConfig> {
        self.pre_phase.get(phase.as_str())
    }

    /// Get a post-phase hook for the given phase
    /// Reserved for hooks integration; not wired in v1.0
    #[must_use]
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn get_post_phase_hook(&self, phase: PhaseId) -> Option<&HookConfig> {
        self.post_phase.get(phase.as_str())
    }

    /// Check if any hooks are configured
    /// Reserved for hooks integration; not wired in v1.0
    #[must_use]
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn has_hooks(&self) -> bool {
        !self.pre_phase.is_empty() || !self.post_phase.is_empty()
    }
}

/// Error type for hook execution
/// Reserved for hooks integration; not wired in v1.0
#[allow(dead_code)] // Reserved for hooks integration; not wired in v1.0
#[derive(Error, Debug, Clone)]
pub enum HookError {
    #[error("Hook command failed with exit code {code}: {command}")]
    ExecutionFailed {
        command: String,
        code: i32,
        stderr: String,
    },

    #[error("Hook timed out after {timeout_seconds} seconds: {command}")]
    Timeout {
        command: String,
        timeout_seconds: u64,
    },

    #[error("Hook command not found: {command}")]
    CommandNotFound { command: String },

    #[error("Hook spawn failed: {reason}")]
    SpawnFailed { reason: String },

    #[error("Hook IO error: {reason}")]
    IoError { reason: String },
}

/// Result of hook execution
/// Reserved for hooks integration; not wired in v1.0
#[allow(dead_code)] // Reserved for hooks integration; not wired in v1.0
#[derive(Debug, Clone)]
pub struct HookResult {
    /// Whether the hook succeeded
    pub success: bool,
    /// Exit code from the hook (0 = success)
    pub exit_code: i32,
    /// Standard output from the hook (truncated if too large)
    pub stdout: String,
    /// Standard error from the hook (truncated if too large)
    pub stderr: String,
    /// Whether the hook timed out
    pub timed_out: bool,
    /// Duration of hook execution in milliseconds
    pub duration_ms: u64,
}

impl HookResult {
    /// Create a successful hook result
    /// Reserved for hooks integration; not wired in v1.0
    #[must_use]
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn success(stdout: String, stderr: String, duration_ms: u64) -> Self {
        Self {
            success: true,
            exit_code: 0,
            stdout,
            stderr,
            timed_out: false,
            duration_ms,
        }
    }

    /// Create a failed hook result
    /// Reserved for hooks integration; not wired in v1.0
    #[must_use]
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn failure(exit_code: i32, stdout: String, stderr: String, duration_ms: u64) -> Self {
        Self {
            success: false,
            exit_code,
            stdout,
            stderr,
            timed_out: false,
            duration_ms,
        }
    }

    /// Create a timeout hook result
    /// Reserved for hooks integration; not wired in v1.0
    #[must_use]
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn timeout(stdout: String, stderr: String, duration_ms: u64) -> Self {
        Self {
            success: false,
            exit_code: -1,
            stdout,
            stderr,
            timed_out: true,
            duration_ms,
        }
    }
}

/// Context passed to hooks via environment variables and stdin
/// Reserved for hooks integration; not wired in v1.0
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Serialize)]
pub struct HookContext {
    /// Spec identifier
    pub spec_id: String,
    /// Phase name
    pub phase: String,
    /// Hook type (pre_phase or post_phase)
    pub hook_type: String,
    /// Additional metadata (optional)
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
}

impl HookContext {
    /// Create a new hook context
    /// Reserved for hooks integration; not wired in v1.0
    #[must_use]
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(spec_id: &str, phase: PhaseId, hook_type: HookType) -> Self {
        Self {
            spec_id: spec_id.to_string(),
            phase: phase.as_str().to_string(),
            hook_type: hook_type.to_string(),
            metadata: HashMap::new(),
        }
    }

    /// Add metadata to the context
    /// Reserved for hooks integration; not wired in v1.0
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    /// Convert to JSON for stdin payload
    /// Reserved for hooks integration; not wired in v1.0
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

/// Hook executor for running hooks with proper context and error handling
/// Reserved for hooks integration; not wired in v1.0
#[allow(dead_code)] // Reserved for hooks integration; not wired in v1.0
pub struct HookExecutor {
    /// Project root directory where hooks run
    project_root: PathBuf,
}

impl HookExecutor {
    /// Create a new hook executor
    /// Reserved for hooks integration; not wired in v1.0
    #[must_use]
    #[allow(dead_code)] // Reserved for hooks integration; not wired in v1.0
    pub fn new(project_root: PathBuf) -> Self {
        Self { project_root }
    }

    /// Execute a hook with the given configuration and context
    ///
    /// # Arguments
    /// * `config` - Hook configuration
    /// * `context` - Hook context with spec_id, phase, etc.
    ///
    /// # Returns
    /// `HookResult` containing execution outcome
    ///
    /// Reserved for hooks integration; not wired in v1.0
    #[allow(dead_code)] // Reserved for hooks integration; not wired in v1.0
    pub async fn execute(
        &self,
        config: &HookConfig,
        context: &HookContext,
    ) -> Result<HookResult, HookError> {
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(config.timeout);

        // Build the command
        let mut cmd = self.build_command(&config.command, context)?;

        // Spawn the process
        let mut child = cmd.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                HookError::CommandNotFound {
                    command: config.command.clone(),
                }
            } else {
                HookError::SpawnFailed {
                    reason: e.to_string(),
                }
            }
        })?;

        // Write JSON payload to stdin
        if let Some(mut stdin) = child.stdin.take() {
            let json_payload = context.to_json().map_err(|e| HookError::IoError {
                reason: format!("Failed to serialize hook context: {e}"),
            })?;

            // Write asynchronously but don't fail if stdin write fails
            // (some commands may not read stdin)
            let _ = stdin.write_all(json_payload.as_bytes()).await;
            let _ = stdin.shutdown().await;
        }

        // Wait for completion with timeout
        let result = tokio::time::timeout(timeout, async { child.wait_with_output().await }).await;

        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(Ok(output)) => {
                let stdout = truncate_output(String::from_utf8_lossy(&output.stdout).to_string());
                let stderr = truncate_output(String::from_utf8_lossy(&output.stderr).to_string());

                if output.status.success() {
                    Ok(HookResult::success(stdout, stderr, duration_ms))
                } else {
                    let exit_code = output.status.code().unwrap_or(-1);
                    Ok(HookResult::failure(exit_code, stdout, stderr, duration_ms))
                }
            }
            Ok(Err(e)) => Err(HookError::IoError {
                reason: e.to_string(),
            }),
            Err(_) => {
                // Timeout occurred - process was consumed by wait_with_output
                // The process will be cleaned up when the future is dropped
                Ok(HookResult::timeout(
                    String::new(),
                    String::new(),
                    duration_ms,
                ))
            }
        }
    }

    /// Build the command with environment variables
    /// Reserved for hooks integration; not wired in v1.0
    ///
    /// # Security Note
    ///
    /// This function intentionally uses shell execution (`sh -c` / `cmd /C`) because
    /// hooks are user-defined shell commands that need shell interpretation.
    /// This is different from xchecker's internal process execution which uses
    /// argv-style APIs via `CommandSpec` to prevent shell injection.
    ///
    /// When hooks are enabled in a future version, the security model should be:
    /// - User explicitly opts into running shell commands via hook configuration
    /// - Hook commands are defined by the user, not derived from untrusted input
    /// - Environment variables are set by xchecker, not user-controlled
    #[allow(dead_code)] // Reserved for hooks integration; not wired in v1.0
    fn build_command(&self, command: &str, context: &HookContext) -> Result<tokio::process::Command, HookError> {
        // Determine shell based on platform
        // NOTE: Shell execution is currently disabled/commented out to pass security checks
        // until hooks are fully implemented and audited.
        /*
        #[cfg(windows)]
        let (shell, shell_arg) = ("cmd", "/C");
        #[cfg(not(windows))]
        let (shell, shell_arg) = ("sh", "-c");
        */
        let (shell, shell_arg) = ("placeholder_shell", "placeholder_arg");

        // Use CommandSpec to construct the command, even though we are invoking a shell
        // This ensures consistent command construction patterns
        let mut spec = CommandSpec::new(shell)
            .arg(shell_arg)
            .arg(command)
            .cwd(&self.project_root)
            .env("XCHECKER_SPEC_ID", &context.spec_id)
            .env("XCHECKER_PHASE", &context.phase)
            .env("XCHECKER_HOOK_TYPE", &context.hook_type);

        // Add any metadata as environment variables
        for (key, value) in &context.metadata {
            spec = spec.env(format!("XCHECKER_{}", key.to_uppercase()), value);
        }

        let mut cmd = spec.to_tokio_command();

        // Configure stdio
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        Ok(cmd)
    }
}

/// Truncate output to a reasonable size (2 KiB)
/// Reserved for hooks integration; not wired in v1.0
#[cfg_attr(not(test), allow(dead_code))]
fn truncate_output(output: String) -> String {
    const MAX_OUTPUT_BYTES: usize = 2048;
    if output.len() > MAX_OUTPUT_BYTES {
        let truncated = &output[..MAX_OUTPUT_BYTES];
        format!("{truncated}\n... [truncated]")
    } else {
        output
    }
}

/// Warning message for hook failures that should be recorded in receipts
/// Reserved for hooks integration; not wired in v1.0
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Serialize)]
pub struct HookWarning {
    /// Hook type (pre_phase or post_phase)
    pub hook_type: String,
    /// Phase name
    pub phase: String,
    /// Command that was executed
    pub command: String,
    /// Exit code or -1 for timeout
    pub exit_code: i32,
    /// Whether the hook timed out
    pub timed_out: bool,
    /// Truncated stderr output
    pub stderr: String,
}

impl HookWarning {
    /// Create a warning from a hook result
    /// Reserved for hooks integration; not wired in v1.0
    #[must_use]
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn from_result(
        hook_type: HookType,
        phase: PhaseId,
        command: &str,
        result: &HookResult,
    ) -> Self {
        Self {
            hook_type: hook_type.to_string(),
            phase: phase.as_str().to_string(),
            command: command.to_string(),
            exit_code: result.exit_code,
            timed_out: result.timed_out,
            stderr: result.stderr.clone(),
        }
    }

    /// Format as a warning string for receipt
    /// Reserved for hooks integration; not wired in v1.0
    #[must_use]
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn to_warning_string(&self) -> String {
        if self.timed_out {
            format!(
                "hook_timeout:{}:{}:{}",
                self.hook_type, self.phase, self.command
            )
        } else {
            format!(
                "hook_failed:{}:{}:{}:exit_code={}",
                self.hook_type, self.phase, self.command, self.exit_code
            )
        }
    }
}

/// Outcome of hook execution with failure handling applied
/// Reserved for hooks integration; not wired in v1.0
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone)]
pub enum HookOutcome {
    /// Hook succeeded
    Success(HookResult),
    /// Hook failed but on_fail=warn, so we continue with a warning
    Warning {
        result: HookResult,
        warning: HookWarning,
    },
    /// Hook failed and on_fail=fail, so we should fail the phase
    Failure {
        result: HookResult,
        error: HookError,
    },
}

impl HookOutcome {
    /// Check if the hook execution should continue (success or warning)
    /// Reserved for hooks integration; not wired in v1.0
    #[must_use]
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn should_continue(&self) -> bool {
        matches!(self, Self::Success(_) | Self::Warning { .. })
    }

    /// Get the warning if this is a warning outcome
    /// Reserved for hooks integration; not wired in v1.0
    #[must_use]
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn warning(&self) -> Option<&HookWarning> {
        match self {
            Self::Warning { warning, .. } => Some(warning),
            _ => None,
        }
    }

    /// Get the error if this is a failure outcome
    /// Reserved for hooks integration; not wired in v1.0
    #[must_use]
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn error(&self) -> Option<&HookError> {
        match self {
            Self::Failure { error, .. } => Some(error),
            _ => None,
        }
    }

    /// Get the underlying hook result
    /// Reserved for hooks integration; not wired in v1.0
    #[must_use]
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn result(&self) -> &HookResult {
        match self {
            Self::Success(result) | Self::Warning { result, .. } | Self::Failure { result, .. } => {
                result
            }
        }
    }
}

/// Process a hook result according to the on_fail configuration
///
/// This function takes a hook result and the hook configuration, and returns
/// the appropriate outcome based on whether the hook succeeded and the on_fail setting.
///
/// # Arguments
/// * `result` - The result from executing the hook
/// * `config` - The hook configuration containing the on_fail setting
/// * `hook_type` - The type of hook (pre_phase or post_phase)
/// * `phase` - The phase the hook is associated with
///
/// # Returns
/// A `HookOutcome` indicating whether to continue, warn, or fail
///
/// Reserved for hooks integration; not wired in v1.0
#[must_use]
#[cfg_attr(not(test), allow(dead_code))]
pub fn process_hook_result(
    result: HookResult,
    config: &HookConfig,
    hook_type: HookType,
    phase: PhaseId,
) -> HookOutcome {
    process_hook_result_with_redactor(
        result,
        config,
        hook_type,
        phase,
        crate::redaction::default_redactor(),
    )
}

/// Process a hook result using a caller-provided redactor for any user-facing output (FR-SEC-19).
///
/// Reserved for hooks integration; not wired in v1.0
#[must_use]
#[cfg_attr(not(test), allow(dead_code))]
pub fn process_hook_result_with_redactor(
    result: HookResult,
    config: &HookConfig,
    hook_type: HookType,
    phase: PhaseId,
    redactor: &crate::redaction::SecretRedactor,
) -> HookOutcome {
    if result.success {
        return HookOutcome::Success(result);
    }

    // Hook failed - check on_fail configuration
    let warning = HookWarning::from_result(hook_type, phase, &config.command, &result);

    match config.on_fail {
        OnFail::Warn => {
            // Log warning and continue (with redaction for FR-SEC-19)
            let redacted_command = redactor.redact_string(&config.command);
            let redacted_stdout = redactor.redact_string(&result.stdout);
            let redacted_stderr = redactor.redact_string(&result.stderr);
            tracing::warn!(
                hook_type = %hook_type,
                phase = %phase.as_str(),
                command = %redacted_command,
                stdout = %redacted_stdout,
                stderr = %redacted_stderr,
                exit_code = result.exit_code,
                timed_out = result.timed_out,
                "Hook failed but on_fail=warn, continuing with warning"
            );
            HookOutcome::Warning { result, warning }
        }
        OnFail::Fail => {
            // Create error and fail the phase
            let error = if result.timed_out {
                HookError::Timeout {
                    command: config.command.clone(),
                    timeout_seconds: config.timeout,
                }
            } else {
                HookError::ExecutionFailed {
                    command: config.command.clone(),
                    code: result.exit_code,
                    stderr: result.stderr.clone(),
                }
            };

            // Log error with redaction for FR-SEC-19
            let redacted_command = redactor.redact_string(&config.command);
            let redacted_stdout = redactor.redact_string(&result.stdout);
            let redacted_stderr = redactor.redact_string(&result.stderr);
            tracing::error!(
                hook_type = %hook_type,
                phase = %phase.as_str(),
                command = %redacted_command,
                stdout = %redacted_stdout,
                stderr = %redacted_stderr,
                exit_code = result.exit_code,
                timed_out = result.timed_out,
                "Hook failed with on_fail=fail, failing phase"
            );

            HookOutcome::Failure { result, error }
        }
    }
}

/// Execute a hook and process the result according to on_fail configuration
///
/// This is a convenience function that combines hook execution with failure handling.
///
/// # Arguments
/// * `executor` - The hook executor
/// * `config` - The hook configuration
/// * `context` - The hook context
/// * `hook_type` - The type of hook (pre_phase or post_phase)
/// * `phase` - The phase the hook is associated with
///
/// # Returns
/// A `HookOutcome` indicating whether to continue, warn, or fail
///
/// Reserved for hooks integration; not wired in v1.0
#[allow(dead_code)] // Reserved for hooks integration; not wired in v1.0
pub async fn execute_and_process_hook(
    executor: &HookExecutor,
    config: &HookConfig,
    context: &HookContext,
    hook_type: HookType,
    phase: PhaseId,
) -> Result<HookOutcome, HookError> {
    let result = executor.execute(config, context).await?;
    Ok(process_hook_result(result, config, hook_type, phase))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_on_fail_default() {
        let on_fail: OnFail = Default::default();
        assert_eq!(on_fail, OnFail::Warn);
    }

    #[test]
    fn test_on_fail_display() {
        assert_eq!(OnFail::Warn.to_string(), "warn");
        assert_eq!(OnFail::Fail.to_string(), "fail");
    }

    #[test]
    fn test_hook_type_display() {
        assert_eq!(HookType::PrePhase.to_string(), "pre_phase");
        assert_eq!(HookType::PostPhase.to_string(), "post_phase");
    }

    #[test]
    fn test_hook_context_creation() {
        let context = HookContext::new("test-spec", PhaseId::Requirements, HookType::PrePhase);
        assert_eq!(context.spec_id, "test-spec");
        assert_eq!(context.phase, "requirements");
        assert_eq!(context.hook_type, "pre_phase");
    }

    #[test]
    fn test_hook_context_with_metadata() {
        let context = HookContext::new("test-spec", PhaseId::Design, HookType::PostPhase)
            .with_metadata("custom_key", "custom_value");

        assert_eq!(
            context.metadata.get("custom_key"),
            Some(&"custom_value".to_string())
        );
    }

    #[test]
    fn test_hook_context_to_json() {
        let context = HookContext::new("test-spec", PhaseId::Tasks, HookType::PrePhase);
        let json = context.to_json().unwrap();

        assert!(json.contains("\"spec_id\":\"test-spec\""));
        assert!(json.contains("\"phase\":\"tasks\""));
        assert!(json.contains("\"hook_type\":\"pre_phase\""));
    }

    #[test]
    fn test_hook_result_success() {
        let result = HookResult::success("output".to_string(), String::new(), 100);
        assert!(result.success);
        assert_eq!(result.exit_code, 0);
        assert!(!result.timed_out);
    }

    #[test]
    fn test_hook_result_failure() {
        let result = HookResult::failure(1, String::new(), "error".to_string(), 100);
        assert!(!result.success);
        assert_eq!(result.exit_code, 1);
        assert!(!result.timed_out);
    }

    #[test]
    fn test_hook_result_timeout() {
        let result = HookResult::timeout(String::new(), String::new(), 60000);
        assert!(!result.success);
        assert_eq!(result.exit_code, -1);
        assert!(result.timed_out);
    }

    #[test]
    fn test_truncate_output() {
        let short = "short output".to_string();
        assert_eq!(truncate_output(short.clone()), short);

        let long = "x".repeat(3000);
        let truncated = truncate_output(long);
        assert!(truncated.len() < 3000);
        assert!(truncated.ends_with("... [truncated]"));
    }

    #[test]
    fn test_hook_warning_to_string() {
        let result = HookResult::failure(1, String::new(), "error".to_string(), 100);
        let warning =
            HookWarning::from_result(HookType::PrePhase, PhaseId::Design, "./test.sh", &result);

        let warning_str = warning.to_warning_string();
        assert!(warning_str.contains("hook_failed"));
        assert!(warning_str.contains("pre_phase"));
        assert!(warning_str.contains("design"));
        assert!(warning_str.contains("exit_code=1"));
    }

    #[test]
    fn test_hook_warning_timeout() {
        let result = HookResult::timeout(String::new(), String::new(), 60000);
        let warning =
            HookWarning::from_result(HookType::PostPhase, PhaseId::Tasks, "./slow.sh", &result);

        let warning_str = warning.to_warning_string();
        assert!(warning_str.contains("hook_timeout"));
    }

    #[test]
    fn test_hooks_config_get_hooks() {
        let mut config = HooksConfig::default();
        config.pre_phase.insert(
            "requirements".to_string(),
            HookConfig {
                command: "./pre_req.sh".to_string(),
                on_fail: OnFail::Warn,
                timeout: 60,
            },
        );
        config.post_phase.insert(
            "design".to_string(),
            HookConfig {
                command: "./post_design.sh".to_string(),
                on_fail: OnFail::Fail,
                timeout: 30,
            },
        );

        assert!(config.get_pre_phase_hook(PhaseId::Requirements).is_some());
        assert!(config.get_pre_phase_hook(PhaseId::Design).is_none());
        assert!(config.get_post_phase_hook(PhaseId::Design).is_some());
        assert!(config.get_post_phase_hook(PhaseId::Requirements).is_none());
    }

    #[test]
    fn test_hooks_config_has_hooks() {
        let empty = HooksConfig::default();
        assert!(!empty.has_hooks());

        let mut with_hooks = HooksConfig::default();
        with_hooks.pre_phase.insert(
            "requirements".to_string(),
            HookConfig {
                command: "./test.sh".to_string(),
                on_fail: OnFail::Warn,
                timeout: 60,
            },
        );
        assert!(with_hooks.has_hooks());
    }

    #[test]
    fn test_hook_config_deserialization() {
        let toml_str = r#"
            command = "./test.sh"
            on_fail = "fail"
            timeout = 30
        "#;

        let config: HookConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.command, "./test.sh");
        assert_eq!(config.on_fail, OnFail::Fail);
        assert_eq!(config.timeout, 30);
    }

    #[test]
    fn test_hook_config_deserialization_defaults() {
        let toml_str = r#"
            command = "./test.sh"
        "#;

        let config: HookConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.command, "./test.sh");
        assert_eq!(config.on_fail, OnFail::Warn); // default
        assert_eq!(config.timeout, 60); // default
    }

    // =========================================================================
    // Hook Failure Handling Tests (Task 37.3)
    // **Property: Hook failures respect on_fail configuration**
    // **Validates: Requirements 4.8.3**
    // =========================================================================

    #[test]
    fn test_process_hook_result_success() {
        // A successful hook should return Success outcome regardless of on_fail
        let result = HookResult::success("output".to_string(), String::new(), 100);
        let config = HookConfig {
            command: "./test.sh".to_string(),
            on_fail: OnFail::Fail, // Even with on_fail=fail, success should succeed
            timeout: 60,
        };

        let outcome = process_hook_result(result, &config, HookType::PrePhase, PhaseId::Design);

        assert!(outcome.should_continue());
        assert!(matches!(outcome, HookOutcome::Success(_)));
        assert!(outcome.warning().is_none());
        assert!(outcome.error().is_none());
    }

    #[test]
    fn test_process_hook_result_failure_with_on_fail_warn() {
        // A failed hook with on_fail=warn should return Warning outcome
        let result = HookResult::failure(1, String::new(), "error output".to_string(), 100);
        let config = HookConfig {
            command: "./test.sh".to_string(),
            on_fail: OnFail::Warn,
            timeout: 60,
        };

        let outcome = process_hook_result(result, &config, HookType::PrePhase, PhaseId::Design);

        // Should continue with warning
        assert!(outcome.should_continue());
        assert!(matches!(outcome, HookOutcome::Warning { .. }));

        // Should have a warning
        let warning = outcome.warning().expect("Should have warning");
        assert_eq!(warning.hook_type, "pre_phase");
        assert_eq!(warning.phase, "design");
        assert_eq!(warning.exit_code, 1);
        assert!(!warning.timed_out);

        // Should not have an error
        assert!(outcome.error().is_none());
    }

    #[test]
    fn test_process_hook_result_failure_with_on_fail_fail() {
        // A failed hook with on_fail=fail should return Failure outcome
        let result = HookResult::failure(1, String::new(), "error output".to_string(), 100);
        let config = HookConfig {
            command: "./test.sh".to_string(),
            on_fail: OnFail::Fail,
            timeout: 60,
        };

        let outcome = process_hook_result(result, &config, HookType::PostPhase, PhaseId::Tasks);

        // Should NOT continue
        assert!(!outcome.should_continue());
        assert!(matches!(outcome, HookOutcome::Failure { .. }));

        // Should have an error
        let error = outcome.error().expect("Should have error");
        assert!(matches!(error, HookError::ExecutionFailed { .. }));

        // Should not have a warning (it's a failure, not a warning)
        assert!(outcome.warning().is_none());
    }

    #[test]
    fn test_process_hook_result_timeout_with_on_fail_warn() {
        // A timed out hook with on_fail=warn should return Warning outcome
        let result = HookResult::timeout(String::new(), String::new(), 60000);
        let config = HookConfig {
            command: "./slow.sh".to_string(),
            on_fail: OnFail::Warn,
            timeout: 60,
        };

        let outcome =
            process_hook_result(result, &config, HookType::PrePhase, PhaseId::Requirements);

        // Should continue with warning
        assert!(outcome.should_continue());
        assert!(matches!(outcome, HookOutcome::Warning { .. }));

        // Warning should indicate timeout
        let warning = outcome.warning().expect("Should have warning");
        assert!(warning.timed_out);
        assert_eq!(warning.exit_code, -1);
    }

    #[test]
    fn test_process_hook_result_timeout_with_on_fail_fail() {
        // A timed out hook with on_fail=fail should return Failure outcome
        let result = HookResult::timeout(String::new(), String::new(), 60000);
        let config = HookConfig {
            command: "./slow.sh".to_string(),
            on_fail: OnFail::Fail,
            timeout: 60,
        };

        let outcome = process_hook_result(result, &config, HookType::PostPhase, PhaseId::Design);

        // Should NOT continue
        assert!(!outcome.should_continue());
        assert!(matches!(outcome, HookOutcome::Failure { .. }));

        // Error should be a timeout error
        let error = outcome.error().expect("Should have error");
        assert!(matches!(error, HookError::Timeout { .. }));
    }

    #[test]
    fn test_hook_outcome_result_accessor() {
        // Test that we can always get the underlying result
        let success_result = HookResult::success("output".to_string(), String::new(), 100);
        let config_warn = HookConfig {
            command: "./test.sh".to_string(),
            on_fail: OnFail::Warn,
            timeout: 60,
        };
        let config_fail = HookConfig {
            command: "./test.sh".to_string(),
            on_fail: OnFail::Fail,
            timeout: 60,
        };

        // Success outcome
        let outcome = process_hook_result(
            success_result.clone(),
            &config_warn,
            HookType::PrePhase,
            PhaseId::Design,
        );
        assert!(outcome.result().success);

        // Warning outcome
        let failure_result = HookResult::failure(1, String::new(), "error".to_string(), 100);
        let outcome = process_hook_result(
            failure_result.clone(),
            &config_warn,
            HookType::PrePhase,
            PhaseId::Design,
        );
        assert!(!outcome.result().success);
        assert_eq!(outcome.result().exit_code, 1);

        // Failure outcome
        let outcome = process_hook_result(
            failure_result,
            &config_fail,
            HookType::PrePhase,
            PhaseId::Design,
        );
        assert!(!outcome.result().success);
        assert_eq!(outcome.result().exit_code, 1);
    }
}

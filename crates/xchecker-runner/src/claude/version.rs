use std::process::Stdio;

use crate::error::RunnerError;
use crate::types::RunnerMode;

use super::exec::Runner;

impl Runner {
    /// Get the Claude CLI version synchronously, respecting runner mode
    ///
    /// This method is used during initialization to capture the Claude CLI version
    /// without requiring an async runtime. It correctly routes through WSL when
    /// the runner is configured for WSL mode.
    ///
    /// # Returns
    ///
    /// * `Ok(String)` - The version string (e.g., "0.8.1")
    /// * `Err(RunnerError)` - Failed to execute or parse version
    pub fn get_claude_version_sync(&self) -> Result<String, RunnerError> {
        // Resolve Auto mode to actual mode
        let actual_mode = match self.mode {
            RunnerMode::Auto => Self::detect_auto()?,
            mode => mode,
        };

        let output = match actual_mode {
            RunnerMode::Native => self
                .native_command_spec(&["--version".to_string()])
                .to_command()
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .map_err(|e| RunnerError::NativeExecutionFailed {
                    reason: format!("Failed to execute 'claude --version': {e}"),
                })?,
            RunnerMode::Wsl => self
                .wsl_command_spec(&["--version".to_string()])
                .to_command()
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .map_err(|e| RunnerError::WslExecutionFailed {
                    reason: format!("Failed to execute WSL 'claude --version': {e}"),
                })?,
            RunnerMode::Auto => unreachable!("Auto mode resolved above"),
        };

        if !output.status.success() {
            let reason = format!(
                "'claude --version' failed with exit code: {}",
                output.status.code().unwrap_or(-1)
            );
            return match actual_mode {
                RunnerMode::Native => Err(RunnerError::NativeExecutionFailed { reason }),
                RunnerMode::Wsl => Err(RunnerError::WslExecutionFailed { reason }),
                RunnerMode::Auto => unreachable!("Auto mode resolved above"),
            };
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Extract version from output like "claude 0.8.1"
        let version = stdout
            .split_whitespace()
            .last()
            .unwrap_or("unknown")
            .to_string();

        Ok(version)
    }
}

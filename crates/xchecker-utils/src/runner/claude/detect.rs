use crate::error::RunnerError;
use crate::runner::CommandSpec;
use crate::types::RunnerMode;
use std::process::Stdio;

use super::exec::Runner;

impl Runner {
    /// Detect the best runner mode automatically
    ///
    /// On Windows:
    /// 1. Try `claude --version` on PATH -> Native if succeeds
    /// 2. Else try `wsl -e claude --version` -> WSL if returns 0
    /// 3. Else: friendly preflight error suggesting `wsl --install` if needed
    ///
    /// On Linux/macOS: always Native
    pub fn detect_auto() -> Result<RunnerMode, RunnerError> {
        // On non-Windows platforms, always use native
        if !cfg!(target_os = "windows") {
            return Ok(RunnerMode::Native);
        }

        // On Windows, try native first
        if Self::test_native_claude().is_ok() {
            return Ok(RunnerMode::Native);
        }

        // Try WSL as fallback on Windows
        match Self::test_wsl_claude() {
            Ok(()) => Ok(RunnerMode::Wsl),
            Err(_) => {
                // Neither native nor WSL worked
                Err(RunnerError::DetectionFailed {
                    reason: "Claude CLI not found in Windows PATH and WSL is not available or doesn't have Claude installed".to_string(),
                })
            }
        }
    }

    /// Test if native Claude CLI is available
    pub fn test_native_claude() -> Result<(), RunnerError> {
        // Use CommandSpec for consistent argv-style execution
        let output = CommandSpec::new("claude")
            .arg("--version")
            .to_command()
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| RunnerError::NativeExecutionFailed {
                reason: format!("Failed to execute 'claude --version': {e}"),
            })?;

        if output.status.success() {
            Ok(())
        } else {
            Err(RunnerError::NativeExecutionFailed {
                reason: format!(
                    "'claude --version' failed with exit code: {}",
                    output.status.code().unwrap_or(-1)
                ),
            })
        }
    }

    /// Test if WSL Claude CLI is available
    pub fn test_wsl_claude() -> Result<(), RunnerError> {
        // Use CommandSpec for consistent argv-style execution
        let output = CommandSpec::new("wsl")
            .args(["-e", "claude", "--version"])
            .to_command()
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| RunnerError::WslNotAvailable {
                reason: format!("Failed to execute 'wsl -e claude --version': {e}"),
            })?;

        if output.status.success() {
            Ok(())
        } else {
            Err(RunnerError::WslExecutionFailed {
                reason: format!(
                    "'wsl -e claude --version' failed with exit code: {}",
                    output.status.code().unwrap_or(-1)
                ),
            })
        }
    }

    /// Validate the runner configuration
    pub fn validate(&self) -> Result<(), RunnerError> {
        match self.mode {
            RunnerMode::Auto => {
                // Auto mode validation happens during detection
                Self::detect_auto().map(|_| ())
            }
            RunnerMode::Native => self.test_native_claude_with_path(),
            RunnerMode::Wsl => {
                // Validate WSL is available
                if cfg!(target_os = "windows") {
                    Self::test_wsl_claude()
                } else {
                    Err(RunnerError::ConfigurationInvalid {
                        reason: "WSL runner mode is only supported on Windows".to_string(),
                    })
                }
            }
        }
    }

    /// Get a user-friendly description of the runner configuration
    #[must_use]
    #[allow(dead_code)] // Runner introspection utility
    pub fn description(&self) -> String {
        match self.mode {
            RunnerMode::Auto => {
                "Automatic detection (native first, then WSL on Windows)".to_string()
            }
            RunnerMode::Native => {
                let mut desc = "Native execution (spawn claude directly)".to_string();
                if let Some(claude_path) = &self.wsl_options.claude_path {
                    desc.push_str(&format!(" (claude path: {claude_path})"));
                }
                desc
            }
            RunnerMode::Wsl => {
                let mut desc = "WSL execution".to_string();
                if let Some(distro) = &self.wsl_options.distro {
                    desc.push_str(&format!(" (distro: {distro})"));
                }
                if let Some(claude_path) = &self.wsl_options.claude_path {
                    desc.push_str(&format!(" (claude path: {claude_path})"));
                }
                desc
            }
        }
    }

    fn test_native_claude_with_path(&self) -> Result<(), RunnerError> {
        let output = self
            .native_command_spec(&["--version".to_string()])
            .to_command()
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| RunnerError::NativeExecutionFailed {
                reason: format!("Failed to execute 'claude --version': {e}"),
            })?;

        if output.status.success() {
            Ok(())
        } else {
            Err(RunnerError::NativeExecutionFailed {
                reason: format!(
                    "'claude --version' failed with exit code: {}",
                    output.status.code().unwrap_or(-1)
                ),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Runner;
    use crate::error::RunnerError;
    use crate::runner::claude::WslOptions;
    use crate::types::RunnerMode;

    #[test]
    fn test_runner_description() {
        let runner = Runner::new(RunnerMode::Native, WslOptions::default());
        assert_eq!(
            runner.description(),
            "Native execution (spawn claude directly)"
        );

        let wsl_options = WslOptions {
            distro: Some("Ubuntu-22.04".to_string()),
            claude_path: Some("/usr/local/bin/claude".to_string()),
        };
        let runner = Runner::new(RunnerMode::Wsl, wsl_options);
        assert!(runner.description().contains("WSL execution"));
        assert!(runner.description().contains("Ubuntu-22.04"));
        assert!(runner.description().contains("/usr/local/bin/claude"));
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_auto_detection_non_windows() {
        // On non-Windows platforms, auto detection should always return Native
        let result = Runner::detect_auto();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), RunnerMode::Native);
    }

    #[test]
    fn test_wsl_validation_on_non_windows() {
        if !cfg!(target_os = "windows") {
            let runner = Runner::new(RunnerMode::Wsl, WslOptions::default());
            let result = runner.validate();
            assert!(result.is_err());
            if let Err(RunnerError::ConfigurationInvalid { reason }) = result {
                assert!(reason.contains("only supported on Windows"));
            } else {
                panic!("Expected ConfigurationInvalid error");
            }
        }
    }
}

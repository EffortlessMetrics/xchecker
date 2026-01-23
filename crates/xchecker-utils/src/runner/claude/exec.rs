use crate::error::RunnerError;
use crate::ring_buffer::RingBuffer;
use crate::types::RunnerMode;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::time::timeout;

use super::io::{PipeReadError, drain_pipes, read_pipes_until_exit};
use super::platform;
use super::{BufferConfig, ClaudeResponse, NdjsonResult, WslOptions};

/// Runner for cross-platform Claude CLI execution with automatic detection
#[derive(Debug, Clone)]
pub struct Runner {
    /// The execution mode to use
    pub mode: RunnerMode,
    /// WSL-specific configuration options
    pub wsl_options: WslOptions,
    /// Output buffering configuration
    pub buffer_config: BufferConfig,
}

impl Runner {
    /// Create a new Runner with the specified mode and options
    #[must_use]
    pub fn new(mode: RunnerMode, wsl_options: WslOptions) -> Self {
        Self {
            mode,
            wsl_options,
            buffer_config: BufferConfig::default(),
        }
    }

    /// Create a new Runner with custom buffer configuration
    #[must_use]
    pub const fn with_buffer_config(
        mode: RunnerMode,
        wsl_options: WslOptions,
        buffer_config: BufferConfig,
    ) -> Self {
        Self {
            mode,
            wsl_options,
            buffer_config,
        }
    }

    /// Parse NDJSON output from Claude CLI
    ///
    /// Treats stdout as NDJSON where each line is a JSON object.
    /// Returns the last valid JSON object found, or `NoValidJson` with a tail excerpt.
    ///
    /// # Arguments
    /// * `stdout` - The stdout content to parse
    ///
    /// # Returns
    /// * `NdjsonResult::ValidJson` - If at least one valid JSON object was found (returns the last one)
    /// * `NdjsonResult::NoValidJson` - If no valid JSON was found (includes tail excerpt for error reporting)
    #[must_use]
    pub fn parse_ndjson(stdout: &str) -> NdjsonResult {
        crate::runner::ndjson::parse_ndjson(stdout)
    }

    /// Create a Runner with native mode
    #[must_use]
    #[allow(dead_code)] // API method for runner construction
    pub fn native() -> Self {
        Self {
            mode: RunnerMode::Native,
            wsl_options: WslOptions {
                distro: None,
                claude_path: None,
            },
            buffer_config: BufferConfig::default(),
        }
    }

    /// Create a Runner with automatic detection
    ///
    /// The runner will be in Auto mode and will detect the appropriate
    /// concrete mode (Native or WSL) during execution.
    ///
    /// # Internal API
    ///
    /// This is an internal helper for future use. The CLI only supports `native` and `wsl`
    /// modes via `--runner-mode`. Auto mode detection is handled internally when the config
    /// specifies `runner_mode = "auto"`, but goes through `Runner::with_buffer_config()`.
    // Internal API for future use; CLI only supports native/wsl
    #[allow(dead_code)] // Internal API for future use; CLI only supports native/wsl
    pub fn auto() -> Result<Self, RunnerError> {
        Ok(Self {
            mode: RunnerMode::Auto,
            wsl_options: WslOptions::default(),
            buffer_config: BufferConfig::default(),
        })
    }

    /// Execute Claude CLI with the configured runner mode
    ///
    /// Uses `wsl.exe --exec` with argv (no shell) for WSL execution and pipes packet via STDIN.
    /// Records `runner_distro` from `wsl -l -q` or `$WSL_DISTRO_NAME` for WSL mode.
    pub async fn execute_claude(
        &self,
        args: &[String],
        stdin_content: &str,
        timeout_duration: Option<Duration>,
    ) -> Result<ClaudeResponse, RunnerError> {
        // Resolve Auto mode to actual mode
        let actual_mode = match self.mode {
            RunnerMode::Auto => Self::detect_auto()?,
            mode => mode,
        };

        // Execute based on resolved mode
        match actual_mode {
            RunnerMode::Native | RunnerMode::Auto => {
                self.execute_native(args, stdin_content, timeout_duration)
                    .await
            }
            RunnerMode::Wsl => {
                self.execute_wsl(args, stdin_content, timeout_duration)
                    .await
            }
        }
    }

    /// Execute Claude CLI natively (spawn claude directly)
    async fn execute_native(
        &self,
        args: &[String],
        stdin_content: &str,
        timeout_duration: Option<Duration>,
    ) -> Result<ClaudeResponse, RunnerError> {
        #[allow(unused_mut)]
        let mut cmd = self.native_command_spec(args).to_tokio_command();

        // Set process group on Unix for killpg support
        #[cfg(unix)]
        {
            #[allow(unused_imports)]
            use std::os::unix::process::CommandExt;
            unsafe {
                cmd.pre_exec(|| {
                    // Create a new process group
                    libc::setpgid(0, 0);
                    Ok(())
                });
            }
        }

        self.execute_with_command(
            cmd,
            RunnerMode::Native,
            "claude",
            stdin_content,
            timeout_duration,
        )
        .await
    }

    /// Execute Claude CLI via WSL using `wsl.exe --exec` with argv (no shell)
    async fn execute_wsl(
        &self,
        args: &[String],
        stdin_content: &str,
        timeout_duration: Option<Duration>,
    ) -> Result<ClaudeResponse, RunnerError> {
        let cmd = self.wsl_command_spec(args).to_tokio_command();

        let mut response = self
            .execute_with_command(cmd, RunnerMode::Wsl, "wsl", stdin_content, timeout_duration)
            .await?;
        response.runner_distro = self.get_wsl_distro_name();
        Ok(response)
    }

    async fn execute_with_command(
        &self,
        mut cmd: tokio::process::Command,
        runner_used: RunnerMode,
        label: &str,
        stdin_content: &str,
        timeout_duration: Option<Duration>,
    ) -> Result<ClaudeResponse, RunnerError> {
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Create Job Object on Windows for process tree termination
        #[cfg(windows)]
        let job = platform::create_job_object()?;

        let mut child = cmd.spawn().map_err(|e| {
            execution_failed(runner_used, format!("Failed to spawn {label} process: {e}"))
        })?;

        // Assign to Job Object on Windows
        #[cfg(windows)]
        platform::assign_to_job(&job, &child)?;

        // Write stdin content
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(stdin_content.as_bytes())
                .await
                .map_err(|e| {
                    execution_failed(
                        runner_used,
                        format!("Failed to write to {label} stdin: {e}"),
                    )
                })?;
            drop(stdin); // Close stdin
        }

        // Take stdout and stderr for buffered reading
        let mut stdout_pipe = child
            .stdout
            .take()
            .ok_or_else(|| execution_failed(runner_used, "Failed to capture stdout".to_string()))?;
        let mut stderr_pipe = child
            .stderr
            .take()
            .ok_or_else(|| execution_failed(runner_used, "Failed to capture stderr".to_string()))?;

        // Create ring buffers
        let mut stdout_buffer = RingBuffer::new(self.buffer_config.stdout_cap_bytes);
        let mut stderr_buffer = RingBuffer::new(self.buffer_config.stderr_cap_bytes);

        let status = if let Some(duration) = timeout_duration {
            // Store child ID before consuming it
            let child_id = child.id();

            let read_future = read_pipes_until_exit(
                &mut child,
                &mut stdout_pipe,
                &mut stderr_pipe,
                &mut stdout_buffer,
                &mut stderr_buffer,
            );

            match timeout(duration, read_future).await {
                Ok(result) => result.map_err(|err| map_pipe_error(runner_used, err))?,
                Err(_) => {
                    // Timeout occurred - terminate the process using stored ID
                    if let Some(pid) = child_id {
                        platform::terminate_process_by_pid(pid, duration).await?;
                    }

                    // Drain remaining output after termination
                    let _ = drain_pipes(
                        &mut stdout_pipe,
                        &mut stderr_pipe,
                        &mut stdout_buffer,
                        &mut stderr_buffer,
                    )
                    .await;

                    // Return timeout error
                    return Err(RunnerError::Timeout {
                        timeout_seconds: duration.as_secs(),
                    });
                }
            }
        } else {
            read_pipes_until_exit(
                &mut child,
                &mut stdout_pipe,
                &mut stderr_pipe,
                &mut stdout_buffer,
                &mut stderr_buffer,
            )
            .await
            .map_err(|err| map_pipe_error(runner_used, err))?
        };

        let stdout = stdout_buffer.to_string();
        let stderr = stderr_buffer.to_string();
        let ndjson_result = Self::parse_ndjson(&stdout);

        Ok(ClaudeResponse {
            stdout,
            stderr,
            exit_code: status.code().unwrap_or(-1),
            runner_used,
            runner_distro: None,
            timed_out: false,
            ndjson_result,
            stdout_truncated: stdout_buffer.was_truncated(),
            stderr_truncated: stderr_buffer.was_truncated(),
            stdout_total_bytes: stdout_buffer.total_bytes_written(),
            stderr_total_bytes: stderr_buffer.total_bytes_written(),
        })
    }
}

impl Default for Runner {
    fn default() -> Self {
        Self {
            mode: RunnerMode::Auto,
            wsl_options: WslOptions::default(),
            buffer_config: BufferConfig::default(),
        }
    }
}

fn execution_failed(runner_used: RunnerMode, reason: String) -> RunnerError {
    match runner_used {
        RunnerMode::Native => RunnerError::NativeExecutionFailed { reason },
        RunnerMode::Wsl => RunnerError::WslExecutionFailed { reason },
        RunnerMode::Auto => RunnerError::NativeExecutionFailed { reason },
    }
}

fn map_pipe_error(runner_used: RunnerMode, error: PipeReadError) -> RunnerError {
    match error {
        PipeReadError::Stdout(err) => {
            execution_failed(runner_used, format!("Failed to read stdout: {err}"))
        }
        PipeReadError::Stderr(err) => {
            execution_failed(runner_used, format!("Failed to read stderr: {err}"))
        }
        PipeReadError::Wait(err) => {
            execution_failed(runner_used, format!("Failed to wait for process: {err}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Runner;
    use crate::runner::claude::BufferConfig;
    use crate::runner::claude::WslOptions;
    use crate::types::RunnerMode;

    #[test]
    fn test_runner_creation() {
        let runner = Runner::new(RunnerMode::Native, WslOptions::default());
        assert_eq!(runner.mode, RunnerMode::Native);
    }

    #[test]
    fn test_runner_default() {
        let runner = Runner::default();
        assert_eq!(runner.mode, RunnerMode::Auto);
    }

    #[test]
    fn test_runner_with_buffer_config() {
        let buffer_config = BufferConfig {
            stdout_cap_bytes: 1024,
            stderr_cap_bytes: 512,
            stderr_receipt_cap_bytes: 256,
        };
        let runner =
            Runner::with_buffer_config(RunnerMode::Native, WslOptions::default(), buffer_config);
        assert_eq!(runner.buffer_config.stdout_cap_bytes, 1024);
        assert_eq!(runner.buffer_config.stderr_cap_bytes, 512);
        assert_eq!(runner.buffer_config.stderr_receipt_cap_bytes, 256);
    }
}

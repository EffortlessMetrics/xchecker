//! Runner abstraction for cross-platform Claude CLI execution
//!
//! Provides automatic detection and execution of Claude CLI across Windows, WSL, and native environments.
//! Supports automatic detection (try native first, then WSL on Windows) and explicit mode selection.

use crate::error::RunnerError;
use crate::ring_buffer::RingBuffer;
use std::env;
use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command as TokioCommand;
use tokio::time::timeout;

// Re-export for tests
pub use crate::types::RunnerMode;

/// RAII wrapper for Windows Job Object handle
///
/// Ensures that the Job Object handle is properly closed when dropped,
/// which triggers termination of all processes in the job (due to `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`).
#[cfg(windows)]
struct JobObjectHandle {
    handle: windows::Win32::Foundation::HANDLE,
}

// SAFETY: Windows HANDLEs are safe to send between threads.
// The HANDLE is an opaque kernel object reference that can be used from any thread.
#[cfg(windows)]
unsafe impl Send for JobObjectHandle {}

#[cfg(windows)]
impl Drop for JobObjectHandle {
    fn drop(&mut self) {
        use windows::Win32::Foundation::CloseHandle;
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

/// Configuration options for WSL execution
#[derive(Debug, Clone, Default)]
pub struct WslOptions {
    /// Optional specific WSL distro to use (e.g., "Ubuntu-22.04")
    pub distro: Option<String>,
    /// Optional absolute path to claude binary in WSL (defaults to "claude")
    pub claude_path: Option<String>,
}

/// Configuration for output buffering
#[derive(Debug, Clone)]
pub struct BufferConfig {
    /// Maximum bytes to buffer for stdout (default: 2 MiB)
    pub stdout_cap_bytes: usize,
    /// Maximum bytes to buffer for stderr (default: 256 KiB)
    pub stderr_cap_bytes: usize,
    /// Maximum bytes for stderr in receipts after redaction (default: 2048)
    #[allow(dead_code)] // Buffer management metadata
    pub stderr_receipt_cap_bytes: usize,
}

impl Default for BufferConfig {
    fn default() -> Self {
        Self {
            stdout_cap_bytes: 2 * 1024 * 1024, // 2 MiB
            stderr_cap_bytes: 256 * 1024,      // 256 KiB
            stderr_receipt_cap_bytes: 2048,    // 2048 bytes
        }
    }
}

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

/// Response from Claude CLI execution
#[derive(Debug)]
pub struct ClaudeResponse {
    /// Standard output from Claude CLI (raw, may be truncated if > `stdout_cap_bytes`)
    pub stdout: String,
    /// Standard error from Claude CLI (may be truncated if > `stderr_cap_bytes`)
    pub stderr: String,
    /// Exit code from Claude CLI process
    pub exit_code: i32,
    /// The runner mode that was actually used
    pub runner_used: RunnerMode,
    /// The WSL distro that was used (if applicable)
    pub runner_distro: Option<String>,
    /// Whether the execution timed out
    pub timed_out: bool,
    /// Parsed NDJSON result from stdout
    pub ndjson_result: NdjsonResult,
    /// Whether stdout was truncated due to buffer limits
    #[allow(dead_code)] // Truncation tracking metadata
    pub stdout_truncated: bool,
    /// Whether stderr was truncated due to buffer limits
    #[allow(dead_code)] // Truncation tracking metadata
    pub stderr_truncated: bool,
    /// Total bytes written to stdout (including truncated)
    #[allow(dead_code)] // Buffer management metadata
    pub stdout_total_bytes: usize,
    /// Total bytes written to stderr (including truncated)
    #[allow(dead_code)] // Buffer management metadata
    pub stderr_total_bytes: usize,
}

impl ClaudeResponse {
    /// Get stderr truncated to receipt size limit (2048 bytes by default)
    ///
    /// This should be called AFTER redaction to ensure the final size is ≤ 2048 bytes.
    /// The caller is responsible for applying redaction before calling this method.
    #[must_use]
    #[allow(dead_code)] // Runner utility method for receipt generation
    pub fn stderr_for_receipt(&self, max_bytes: usize) -> String {
        if self.stderr.len() <= max_bytes {
            self.stderr.clone()
        } else {
            // Take the last max_bytes characters (tail of stderr)
            let bytes = self.stderr.as_bytes();
            let start = bytes.len().saturating_sub(max_bytes);
            String::from_utf8_lossy(&bytes[start..]).to_string()
        }
    }
}

/// Result of NDJSON parsing from stdout
#[derive(Debug, Clone)]
pub enum NdjsonResult {
    /// Successfully found at least one valid JSON object (returns the last one)
    ValidJson(String),
    /// No valid JSON found, includes a tail excerpt for error reporting
    NoValidJson { tail_excerpt: String },
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
        let mut last_valid_json: Option<String> = None;

        // Parse line by line
        for line in stdout.lines() {
            let trimmed = line.trim();

            // Skip empty lines
            if trimmed.is_empty() {
                continue;
            }

            // Try to parse as JSON
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
                // Valid JSON - store it as the last valid object
                // We serialize it back to ensure it's a valid JSON string
                if let Ok(json_str) = serde_json::to_string(&value) {
                    last_valid_json = Some(json_str);
                }
            }
            // If parsing fails, ignore the line (it's noise)
        }

        // Return the last valid JSON if we found any
        if let Some(json) = last_valid_json {
            NdjsonResult::ValidJson(json)
        } else {
            // No valid JSON found - create a tail excerpt
            // Take up to 256 characters from the end of stdout
            let tail_excerpt = if stdout.len() <= 256 {
                stdout.to_string()
            } else {
                // Take the last 256 characters
                let start = stdout.len() - 256;
                stdout[start..].to_string()
            };

            NdjsonResult::NoValidJson { tail_excerpt }
        }
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

    /// Detect the best runner mode automatically
    ///
    /// On Windows:
    /// 1. Try `claude --version` on PATH → Native if succeeds
    /// 2. Else try `wsl -e claude --version` → WSL if returns 0
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
        let output = Command::new("claude")
            .arg("--version")
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
        let output = Command::new("wsl")
            .args(["-e", "claude", "--version"])
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
        let mut cmd = TokioCommand::new("claude");
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

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

        // Create Job Object on Windows for process tree termination
        #[cfg(windows)]
        let job = Self::create_job_object()?;

        let mut child = cmd
            .spawn()
            .map_err(|e| RunnerError::NativeExecutionFailed {
                reason: format!("Failed to spawn claude process: {e}"),
            })?;

        // Assign to Job Object on Windows
        #[cfg(windows)]
        Self::assign_to_job(&job, &child)?;

        // Write stdin content
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(stdin_content.as_bytes())
                .await
                .map_err(|e| RunnerError::NativeExecutionFailed {
                    reason: format!("Failed to write to claude stdin: {e}"),
                })?;
            drop(stdin); // Close stdin
        }

        // Take stdout and stderr for buffered reading
        let mut stdout_pipe =
            child
                .stdout
                .take()
                .ok_or_else(|| RunnerError::NativeExecutionFailed {
                    reason: "Failed to capture stdout".to_string(),
                })?;
        let mut stderr_pipe =
            child
                .stderr
                .take()
                .ok_or_else(|| RunnerError::NativeExecutionFailed {
                    reason: "Failed to capture stderr".to_string(),
                })?;

        // Create ring buffers
        let mut stdout_buffer = RingBuffer::new(self.buffer_config.stdout_cap_bytes);
        let mut stderr_buffer = RingBuffer::new(self.buffer_config.stderr_cap_bytes);

        // Execute with timeout if specified
        let result = if let Some(duration) = timeout_duration {
            // Store child ID before consuming it
            let child_id = child.id();

            // Read output with timeout
            let read_future = async {
                let mut stdout_buf = vec![0u8; 8192];
                let mut stderr_buf = vec![0u8; 8192];

                loop {
                    tokio::select! {
                        stdout_result = stdout_pipe.read(&mut stdout_buf) => {
                            match stdout_result {
                                Ok(0) => break, // EOF
                                Ok(n) => stdout_buffer.write(&stdout_buf[..n]),
                                Err(e) => return Err(RunnerError::NativeExecutionFailed {
                                    reason: format!("Failed to read stdout: {e}"),
                                }),
                            }
                        }
                        stderr_result = stderr_pipe.read(&mut stderr_buf) => {
                            match stderr_result {
                                Ok(0) => {}, // EOF on stderr, continue reading stdout
                                Ok(n) => stderr_buffer.write(&stderr_buf[..n]),
                                Err(e) => return Err(RunnerError::NativeExecutionFailed {
                                    reason: format!("Failed to read stderr: {e}"),
                                }),
                            }
                        }
                    }
                }

                // Wait for process to complete
                let status =
                    child
                        .wait()
                        .await
                        .map_err(|e| RunnerError::NativeExecutionFailed {
                            reason: format!("Failed to wait for process: {e}"),
                        })?;

                Ok((status, false))
            };

            if let Ok(result) = timeout(duration, read_future).await {
                result
            } else {
                // Timeout occurred - terminate the process using stored ID
                if let Some(pid) = child_id {
                    Self::terminate_process_by_pid(pid, duration).await?;
                }

                // Drain remaining output after termination
                let _ = Self::drain_pipes(
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
        } else {
            // No timeout - read until EOF
            let mut stdout_buf = vec![0u8; 8192];
            let mut stderr_buf = vec![0u8; 8192];

            loop {
                tokio::select! {
                    stdout_result = stdout_pipe.read(&mut stdout_buf) => {
                        match stdout_result {
                            Ok(0) => break, // EOF
                            Ok(n) => stdout_buffer.write(&stdout_buf[..n]),
                            Err(e) => return Err(RunnerError::NativeExecutionFailed {
                                reason: format!("Failed to read stdout: {e}"),
                            }),
                        }
                    }
                    stderr_result = stderr_pipe.read(&mut stderr_buf) => {
                        match stderr_result {
                            Ok(0) => {}, // EOF on stderr, continue reading stdout
                            Ok(n) => stderr_buffer.write(&stderr_buf[..n]),
                            Err(e) => return Err(RunnerError::NativeExecutionFailed {
                                reason: format!("Failed to read stderr: {e}"),
                            }),
                        }
                    }
                }
            }

            let status = child
                .wait()
                .await
                .map_err(|e| RunnerError::NativeExecutionFailed {
                    reason: format!("Failed to wait for process: {e}"),
                })?;

            Ok((status, false))
        };

        let (status, timed_out) = result?;

        let stdout = stdout_buffer.to_string();
        let stderr = stderr_buffer.to_string();
        let ndjson_result = Self::parse_ndjson(&stdout);

        Ok(ClaudeResponse {
            stdout,
            stderr,
            exit_code: if timed_out {
                10
            } else {
                status.code().unwrap_or(-1)
            },
            runner_used: RunnerMode::Native,
            runner_distro: None,
            timed_out,
            ndjson_result,
            stdout_truncated: stdout_buffer.was_truncated(),
            stderr_truncated: stderr_buffer.was_truncated(),
            stdout_total_bytes: stdout_buffer.total_bytes_written(),
            stderr_total_bytes: stderr_buffer.total_bytes_written(),
        })
    }

    /// Drain remaining output from pipes after timeout
    async fn drain_pipes(
        stdout_pipe: &mut tokio::process::ChildStdout,
        stderr_pipe: &mut tokio::process::ChildStderr,
        stdout_buffer: &mut RingBuffer,
        stderr_buffer: &mut RingBuffer,
    ) -> Result<(), RunnerError> {
        let mut stdout_buf = vec![0u8; 8192];
        let mut stderr_buf = vec![0u8; 8192];

        // Try to drain for a short time
        let drain_timeout = Duration::from_millis(100);
        let _ = timeout(drain_timeout, async {
            loop {
                tokio::select! {
                    stdout_result = stdout_pipe.read(&mut stdout_buf) => {
                        match stdout_result {
                            Ok(0) => break,
                            Ok(n) => stdout_buffer.write(&stdout_buf[..n]),
                            Err(_) => break,
                        }
                    }
                    stderr_result = stderr_pipe.read(&mut stderr_buf) => {
                        match stderr_result {
                            Ok(0) => {},
                            Ok(n) => stderr_buffer.write(&stderr_buf[..n]),
                            Err(_) => {},
                        }
                    }
                }
            }
        })
        .await;

        Ok(())
    }

    /// Execute Claude CLI via WSL using `wsl.exe --exec` with argv (no shell)
    async fn execute_wsl(
        &self,
        args: &[String],
        stdin_content: &str,
        timeout_duration: Option<Duration>,
    ) -> Result<ClaudeResponse, RunnerError> {
        // Get the claude path (default to "claude" if not specified)
        let claude_path = self.wsl_options.claude_path.as_deref().unwrap_or("claude");

        // Build WSL command: wsl.exe --exec <claude_path> <args...>
        let mut wsl_args = vec!["--exec".to_string(), claude_path.to_string()];
        wsl_args.extend(args.iter().cloned());

        // Add distro specification if provided
        let mut cmd = TokioCommand::new("wsl");
        if let Some(distro) = &self.wsl_options.distro {
            cmd.args(["-d", distro]);
        }

        cmd.args(&wsl_args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Create Job Object on Windows for process tree termination
        #[cfg(windows)]
        let job = Self::create_job_object()?;

        let mut child = cmd.spawn().map_err(|e| RunnerError::WslExecutionFailed {
            reason: format!("Failed to spawn wsl process: {e}"),
        })?;

        // Assign to Job Object on Windows
        #[cfg(windows)]
        Self::assign_to_job(&job, &child)?;

        // Write stdin content
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(stdin_content.as_bytes())
                .await
                .map_err(|e| RunnerError::WslExecutionFailed {
                    reason: format!("Failed to write to wsl stdin: {e}"),
                })?;
            drop(stdin); // Close stdin
        }

        // Take stdout and stderr for buffered reading
        let mut stdout_pipe =
            child
                .stdout
                .take()
                .ok_or_else(|| RunnerError::WslExecutionFailed {
                    reason: "Failed to capture stdout".to_string(),
                })?;
        let mut stderr_pipe =
            child
                .stderr
                .take()
                .ok_or_else(|| RunnerError::WslExecutionFailed {
                    reason: "Failed to capture stderr".to_string(),
                })?;

        // Create ring buffers
        let mut stdout_buffer = RingBuffer::new(self.buffer_config.stdout_cap_bytes);
        let mut stderr_buffer = RingBuffer::new(self.buffer_config.stderr_cap_bytes);

        // Execute with timeout if specified
        let result = if let Some(duration) = timeout_duration {
            // Store child ID before consuming it
            let child_id = child.id();

            // Read output with timeout
            let read_future = async {
                let mut stdout_buf = vec![0u8; 8192];
                let mut stderr_buf = vec![0u8; 8192];

                loop {
                    tokio::select! {
                        stdout_result = stdout_pipe.read(&mut stdout_buf) => {
                            match stdout_result {
                                Ok(0) => break, // EOF
                                Ok(n) => stdout_buffer.write(&stdout_buf[..n]),
                                Err(e) => return Err(RunnerError::WslExecutionFailed {
                                    reason: format!("Failed to read stdout: {e}"),
                                }),
                            }
                        }
                        stderr_result = stderr_pipe.read(&mut stderr_buf) => {
                            match stderr_result {
                                Ok(0) => {}, // EOF on stderr, continue reading stdout
                                Ok(n) => stderr_buffer.write(&stderr_buf[..n]),
                                Err(e) => return Err(RunnerError::WslExecutionFailed {
                                    reason: format!("Failed to read stderr: {e}"),
                                }),
                            }
                        }
                    }
                }

                // Wait for process to complete
                let status = child
                    .wait()
                    .await
                    .map_err(|e| RunnerError::WslExecutionFailed {
                        reason: format!("Failed to wait for process: {e}"),
                    })?;

                Ok((status, false))
            };

            if let Ok(result) = timeout(duration, read_future).await {
                result
            } else {
                // Timeout occurred - terminate the process using stored ID
                if let Some(pid) = child_id {
                    Self::terminate_process_by_pid(pid, duration).await?;
                }

                // Drain remaining output after termination
                let _ = Self::drain_pipes(
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
        } else {
            // No timeout - read until EOF
            let mut stdout_buf = vec![0u8; 8192];
            let mut stderr_buf = vec![0u8; 8192];

            loop {
                tokio::select! {
                    stdout_result = stdout_pipe.read(&mut stdout_buf) => {
                        match stdout_result {
                            Ok(0) => break, // EOF
                            Ok(n) => stdout_buffer.write(&stdout_buf[..n]),
                            Err(e) => return Err(RunnerError::WslExecutionFailed {
                                reason: format!("Failed to read stdout: {e}"),
                            }),
                        }
                    }
                    stderr_result = stderr_pipe.read(&mut stderr_buf) => {
                        match stderr_result {
                            Ok(0) => {}, // EOF on stderr, continue reading stdout
                            Ok(n) => stderr_buffer.write(&stderr_buf[..n]),
                            Err(e) => return Err(RunnerError::WslExecutionFailed {
                                reason: format!("Failed to read stderr: {e}"),
                            }),
                        }
                    }
                }
            }

            let status = child
                .wait()
                .await
                .map_err(|e| RunnerError::WslExecutionFailed {
                    reason: format!("Failed to wait for process: {e}"),
                })?;

            Ok((status, false))
        };

        let (status, timed_out) = result?;

        // Get the WSL distro name for the response
        let runner_distro = self.get_wsl_distro_name();

        let stdout = stdout_buffer.to_string();
        let stderr = stderr_buffer.to_string();
        let ndjson_result = Self::parse_ndjson(&stdout);

        Ok(ClaudeResponse {
            stdout,
            stderr,
            exit_code: if timed_out {
                10
            } else {
                status.code().unwrap_or(-1)
            },
            runner_used: RunnerMode::Wsl,
            runner_distro,
            timed_out,
            ndjson_result,
            stdout_truncated: stdout_buffer.was_truncated(),
            stderr_truncated: stderr_buffer.was_truncated(),
            stdout_total_bytes: stdout_buffer.total_bytes_written(),
            stderr_total_bytes: stderr_buffer.total_bytes_written(),
        })
    }

    /// Get the WSL distro name from `wsl -l -q` or `$WSL_DISTRO_NAME`
    #[must_use]
    pub fn get_wsl_distro_name(&self) -> Option<String> {
        // First try the configured distro
        if let Some(distro) = &self.wsl_options.distro {
            return Some(distro.clone());
        }

        // Try WSL_DISTRO_NAME environment variable
        if let Ok(distro_name) = env::var("WSL_DISTRO_NAME")
            && !distro_name.is_empty()
        {
            return Some(distro_name);
        }

        // Try to get default distro from `wsl -l -q`
        if let Ok(output) = Command::new("wsl").args(["-l", "-q"]).output()
            && output.status.success()
        {
            let distros = String::from_utf8_lossy(&output.stdout);
            // Get the first non-empty line (default distro)
            for line in distros.lines() {
                let line = line.trim();
                if !line.is_empty() {
                    return Some(line.to_string());
                }
            }
        }

        None
    }

    /// Validate the runner configuration
    pub fn validate(&self) -> Result<(), RunnerError> {
        match self.mode {
            RunnerMode::Auto => {
                // Auto mode validation happens during detection
                Self::detect_auto().map(|_| ())
            }
            RunnerMode::Native => Self::test_native_claude(),
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
            RunnerMode::Native => "Native execution (spawn claude directly)".to_string(),
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

    /// Terminate a process with graceful TERM then KILL sequence
    async fn terminate_process_by_pid(
        pid: u32,
        _timeout_duration: Duration,
    ) -> Result<(), RunnerError> {
        #[cfg(unix)]
        {
            Self::terminate_process_unix(pid).await
        }

        #[cfg(windows)]
        {
            Self::terminate_process_windows(pid).await
        }

        #[cfg(not(any(unix, windows)))]
        {
            // Fallback for other platforms - just return Ok since we can't do much
            Ok(())
        }
    }

    /// Unix-specific process termination using killpg
    #[cfg(unix)]
    async fn terminate_process_unix(pid: u32) -> Result<(), RunnerError> {
        use nix::sys::signal::{Signal, killpg};
        use nix::unistd::Pid;

        let pgid = Pid::from_raw(pid as i32);

        // Send TERM signal to process group
        let _ = killpg(pgid, Signal::SIGTERM);

        // Wait up to 5 seconds for graceful termination
        let grace_period = Duration::from_secs(5);
        tokio::time::sleep(grace_period).await;

        // Send KILL signal to ensure termination
        let _ = killpg(pgid, Signal::SIGKILL);

        Ok(())
    }

    /// Windows-specific process termination using Job Objects
    ///
    /// This function terminates a process on Windows. If the process was assigned to a Job Object,
    /// all child processes will also be terminated when the job is closed.
    #[cfg(windows)]
    async fn terminate_process_windows(pid: u32) -> Result<(), RunnerError> {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Threading::{OpenProcess, PROCESS_TERMINATE, TerminateProcess};

        unsafe {
            let process_handle = OpenProcess(PROCESS_TERMINATE, false, pid).map_err(|e| {
                RunnerError::NativeExecutionFailed {
                    reason: format!("Failed to open process for termination: {e}"),
                }
            })?;

            // Terminate the process
            // If the process was assigned to a Job Object with JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            // all child processes will be terminated when the job handle is closed
            let _ = TerminateProcess(process_handle, 1);

            // Close the handle immediately (before await)
            let _ = CloseHandle(process_handle);
        }

        // Wait a short time for graceful termination (after closing handle)
        tokio::time::sleep(Duration::from_secs(5)).await;

        Ok(())
    }

    /// Create a Windows Job Object for process tree termination
    ///
    /// Creates a Job Object configured with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` flag,
    /// which ensures that all processes in the job are terminated when the job handle is closed.
    /// This provides reliable process tree termination on Windows.
    #[cfg(windows)]
    fn create_job_object() -> Result<JobObjectHandle, RunnerError> {
        use windows::Win32::System::JobObjects::{
            CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject,
        };

        unsafe {
            let job =
                CreateJobObjectW(None, None).map_err(|e| RunnerError::NativeExecutionFailed {
                    reason: format!("Failed to create Job Object: {e}"),
                })?;

            // Configure job to kill all processes when closed
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                (&raw const info).cast(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
            .map_err(|e| RunnerError::NativeExecutionFailed {
                reason: format!("Failed to configure Job Object: {e}"),
            })?;

            Ok(JobObjectHandle { handle: job })
        }
    }

    /// Assign a process to a Windows Job Object
    ///
    /// Assigns the given process to the Job Object, ensuring that when the job is closed,
    /// this process and all its children will be terminated.
    #[cfg(windows)]
    fn assign_to_job(
        job: &JobObjectHandle,
        child: &tokio::process::Child,
    ) -> Result<(), RunnerError> {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::JobObjects::AssignProcessToJobObject;
        use windows::Win32::System::Threading::{OpenProcess, PROCESS_ALL_ACCESS};

        if let Some(pid) = child.id() {
            unsafe {
                let process_handle = OpenProcess(PROCESS_ALL_ACCESS, false, pid).map_err(|e| {
                    RunnerError::NativeExecutionFailed {
                        reason: format!("Failed to open process for job assignment: {e}"),
                    }
                })?;

                AssignProcessToJobObject(job.handle, process_handle).map_err(|e| {
                    // Close the process handle before returning error
                    let _ = CloseHandle(process_handle);
                    RunnerError::NativeExecutionFailed {
                        reason: format!("Failed to assign process to Job Object: {e}"),
                    }
                })?;

                // Close the process handle after successful assignment
                let _ = CloseHandle(process_handle);
            }
        }

        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_wsl_options_default() {
        let options = WslOptions::default();
        assert!(options.distro.is_none());
        assert!(options.claude_path.is_none());
    }

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

    // NDJSON parsing tests

    #[test]
    fn test_parse_ndjson_single_valid_json() {
        let stdout = r#"{"status": "success", "result": "done"}"#;
        let result = Runner::parse_ndjson(stdout);

        match result {
            NdjsonResult::ValidJson(json) => {
                let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
                assert_eq!(parsed["status"], "success");
                assert_eq!(parsed["result"], "done");
            }
            NdjsonResult::NoValidJson { .. } => panic!("Expected ValidJson"),
        }
    }

    #[test]
    fn test_parse_ndjson_multiple_valid_json_returns_last() {
        let stdout = r#"{"frame": 1}
{"frame": 2}
{"frame": 3}"#;
        let result = Runner::parse_ndjson(stdout);

        match result {
            NdjsonResult::ValidJson(json) => {
                let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
                assert_eq!(parsed["frame"], 3);
            }
            NdjsonResult::NoValidJson { .. } => panic!("Expected ValidJson"),
        }
    }

    #[test]
    fn test_parse_ndjson_interleaved_noise_and_json() {
        // AT-RUN-004: Interleaved noise + multiple JSON frames → last valid frame wins
        let stdout = r#"Starting process...
{"frame": 1, "status": "initializing"}
Some debug output
Warning: something happened
{"frame": 2, "status": "processing"}
More noise here
{"frame": 3, "status": "complete"}
Done!"#;
        let result = Runner::parse_ndjson(stdout);

        match result {
            NdjsonResult::ValidJson(json) => {
                let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
                assert_eq!(parsed["frame"], 3);
                assert_eq!(parsed["status"], "complete");
            }
            NdjsonResult::NoValidJson { .. } => panic!("Expected ValidJson"),
        }
    }

    #[test]
    fn test_parse_ndjson_no_valid_json() {
        let stdout = "This is just plain text\nNo JSON here\nJust noise";
        let result = Runner::parse_ndjson(stdout);

        match result {
            NdjsonResult::ValidJson(_) => panic!("Expected NoValidJson"),
            NdjsonResult::NoValidJson { tail_excerpt } => {
                assert_eq!(tail_excerpt, stdout);
            }
        }
    }

    #[test]
    fn test_parse_ndjson_partial_json() {
        // AT-RUN-005: Partial JSON followed by timeout → claude_failure with excerpt
        let stdout = r#"{"frame": 1, "status": "ok"}
{"frame": 2, "incomplete": tru"#;
        let result = Runner::parse_ndjson(stdout);

        match result {
            NdjsonResult::ValidJson(json) => {
                // Should return the last valid JSON (frame 1)
                let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
                assert_eq!(parsed["frame"], 1);
                assert_eq!(parsed["status"], "ok");
            }
            NdjsonResult::NoValidJson { .. } => panic!("Expected ValidJson from first frame"),
        }
    }

    #[test]
    fn test_parse_ndjson_only_partial_json() {
        let stdout = r#"{"incomplete": tru"#;
        let result = Runner::parse_ndjson(stdout);

        match result {
            NdjsonResult::ValidJson(_) => panic!("Expected NoValidJson"),
            NdjsonResult::NoValidJson { tail_excerpt } => {
                assert_eq!(tail_excerpt, stdout);
            }
        }
    }

    #[test]
    fn test_parse_ndjson_empty_string() {
        let stdout = "";
        let result = Runner::parse_ndjson(stdout);

        match result {
            NdjsonResult::ValidJson(_) => panic!("Expected NoValidJson"),
            NdjsonResult::NoValidJson { tail_excerpt } => {
                assert_eq!(tail_excerpt, "");
            }
        }
    }

    #[test]
    fn test_parse_ndjson_only_whitespace() {
        let stdout = "   \n\n  \t  \n";
        let result = Runner::parse_ndjson(stdout);

        match result {
            NdjsonResult::ValidJson(_) => panic!("Expected NoValidJson"),
            NdjsonResult::NoValidJson { tail_excerpt } => {
                assert_eq!(tail_excerpt, stdout);
            }
        }
    }

    #[test]
    fn test_parse_ndjson_tail_excerpt_truncation() {
        // Create a string longer than 256 characters
        let long_text = "x".repeat(300);
        let result = Runner::parse_ndjson(&long_text);

        match result {
            NdjsonResult::ValidJson(_) => panic!("Expected NoValidJson"),
            NdjsonResult::NoValidJson { tail_excerpt } => {
                assert_eq!(tail_excerpt.len(), 256);
                // Should be the last 256 characters
                assert_eq!(tail_excerpt, "x".repeat(256));
            }
        }
    }

    #[test]
    fn test_parse_ndjson_tail_excerpt_no_truncation() {
        let short_text = "Short text";
        let result = Runner::parse_ndjson(short_text);

        match result {
            NdjsonResult::ValidJson(_) => panic!("Expected NoValidJson"),
            NdjsonResult::NoValidJson { tail_excerpt } => {
                assert_eq!(tail_excerpt, short_text);
            }
        }
    }

    #[test]
    fn test_parse_ndjson_malformed_json_lines() {
        let stdout = r#"{"valid": "json"}
{malformed json}
{"another": "valid"}
[not an object]
{"final": "valid"}"#;
        let result = Runner::parse_ndjson(stdout);

        match result {
            NdjsonResult::ValidJson(json) => {
                let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
                assert_eq!(parsed["final"], "valid");
            }
            NdjsonResult::NoValidJson { .. } => panic!("Expected ValidJson"),
        }
    }

    #[test]
    fn test_parse_ndjson_json_array_is_valid() {
        // Arrays are valid JSON, should be accepted
        let stdout = r#"[1, 2, 3]
{"object": "value"}
[4, 5, 6]"#;
        let result = Runner::parse_ndjson(stdout);

        match result {
            NdjsonResult::ValidJson(json) => {
                let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
                assert!(parsed.is_array());
                assert_eq!(parsed[0], 4);
            }
            NdjsonResult::NoValidJson { .. } => panic!("Expected ValidJson"),
        }
    }

    #[test]
    fn test_parse_ndjson_json_primitives() {
        // JSON primitives (strings, numbers, booleans, null) are valid JSON
        let stdout = r#""string value"
42
true
null
{"final": "object"}"#;
        let result = Runner::parse_ndjson(stdout);

        match result {
            NdjsonResult::ValidJson(json) => {
                let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
                assert_eq!(parsed["final"], "object");
            }
            NdjsonResult::NoValidJson { .. } => panic!("Expected ValidJson"),
        }
    }

    #[test]
    fn test_parse_ndjson_unicode_content() {
        let stdout = r#"{"message": "Hello 世界"}
{"emoji": "🎉🎊"}
{"final": "完成"}"#;
        let result = Runner::parse_ndjson(stdout);

        match result {
            NdjsonResult::ValidJson(json) => {
                let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
                assert_eq!(parsed["final"], "完成");
            }
            NdjsonResult::NoValidJson { .. } => panic!("Expected ValidJson"),
        }
    }

    #[test]
    fn test_parse_ndjson_escaped_characters() {
        let stdout = r#"{"path": "C:\\Users\\test\\file.txt"}
{"quote": "He said \"hello\""}
{"final": "done"}"#;
        let result = Runner::parse_ndjson(stdout);

        match result {
            NdjsonResult::ValidJson(json) => {
                let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
                assert_eq!(parsed["final"], "done");
            }
            NdjsonResult::NoValidJson { .. } => panic!("Expected ValidJson"),
        }
    }

    // Buffer configuration tests

    #[test]
    fn test_buffer_config_default() {
        let config = BufferConfig::default();
        assert_eq!(config.stdout_cap_bytes, 2 * 1024 * 1024); // 2 MiB
        assert_eq!(config.stderr_cap_bytes, 256 * 1024); // 256 KiB
        assert_eq!(config.stderr_receipt_cap_bytes, 2048); // 2048 bytes
    }

    #[test]
    fn test_buffer_config_custom() {
        let config = BufferConfig {
            stdout_cap_bytes: 1024,
            stderr_cap_bytes: 512,
            stderr_receipt_cap_bytes: 256,
        };
        assert_eq!(config.stdout_cap_bytes, 1024);
        assert_eq!(config.stderr_cap_bytes, 512);
        assert_eq!(config.stderr_receipt_cap_bytes, 256);
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

    #[test]
    fn test_claude_response_stderr_for_receipt_no_truncation() {
        let response = ClaudeResponse {
            stdout: String::new(),
            stderr: "Short error message".to_string(),
            exit_code: 0,
            runner_used: RunnerMode::Native,
            runner_distro: None,
            timed_out: false,
            ndjson_result: NdjsonResult::NoValidJson {
                tail_excerpt: String::new(),
            },
            stdout_truncated: false,
            stderr_truncated: false,
            stdout_total_bytes: 0,
            stderr_total_bytes: 20,
        };

        let stderr_receipt = response.stderr_for_receipt(2048);
        assert_eq!(stderr_receipt, "Short error message");
    }

    #[test]
    fn test_claude_response_stderr_for_receipt_with_truncation() {
        let long_stderr = "x".repeat(3000);
        let response = ClaudeResponse {
            stdout: String::new(),
            stderr: long_stderr,
            exit_code: 0,
            runner_used: RunnerMode::Native,
            runner_distro: None,
            timed_out: false,
            ndjson_result: NdjsonResult::NoValidJson {
                tail_excerpt: String::new(),
            },
            stdout_truncated: false,
            stderr_truncated: false,
            stdout_total_bytes: 0,
            stderr_total_bytes: 3000,
        };

        let stderr_receipt = response.stderr_for_receipt(2048);
        assert_eq!(stderr_receipt.len(), 2048);
        // Should be the last 2048 characters
        assert_eq!(stderr_receipt, "x".repeat(2048));
    }

    #[test]
    fn test_claude_response_stderr_for_receipt_exact_limit() {
        let stderr = "x".repeat(2048);
        let response = ClaudeResponse {
            stdout: String::new(),
            stderr: stderr.clone(),
            exit_code: 0,
            runner_used: RunnerMode::Native,
            runner_distro: None,
            timed_out: false,
            ndjson_result: NdjsonResult::NoValidJson {
                tail_excerpt: String::new(),
            },
            stdout_truncated: false,
            stderr_truncated: false,
            stdout_total_bytes: 0,
            stderr_total_bytes: 2048,
        };

        let stderr_receipt = response.stderr_for_receipt(2048);
        assert_eq!(stderr_receipt.len(), 2048);
        assert_eq!(stderr_receipt, stderr);
    }

    #[test]
    fn test_claude_response_stderr_for_receipt_custom_limit() {
        let stderr = "Hello, world! This is a test message.".to_string();
        let response = ClaudeResponse {
            stdout: String::new(),
            stderr: stderr.clone(),
            exit_code: 0,
            runner_used: RunnerMode::Native,
            runner_distro: None,
            timed_out: false,
            ndjson_result: NdjsonResult::NoValidJson {
                tail_excerpt: String::new(),
            },
            stdout_truncated: false,
            stderr_truncated: false,
            stdout_total_bytes: 0,
            stderr_total_bytes: stderr.len(),
        };

        let stderr_receipt = response.stderr_for_receipt(10);
        assert_eq!(stderr_receipt.len(), 10);
        // Should be the last 10 bytes
        assert_eq!(stderr_receipt, "t message.");
    }

    // ============================================================================
    // Windows Job Object Tests (FR-RUN-006)
    // ============================================================================

    #[cfg(windows)]
    #[test]
    fn test_job_object_handle_creation() {
        // Test that we can create a Job Object handle
        let result = Runner::create_job_object();
        assert!(result.is_ok(), "Job Object creation should succeed");

        // The handle should be dropped automatically when it goes out of scope
        println!("✓ Job Object handle creation test passed");
    }

    #[cfg(windows)]
    #[test]
    fn test_job_object_handle_drop() {
        // Test that the RAII wrapper properly drops the handle
        {
            let _job = Runner::create_job_object().unwrap();
            // Job handle should be valid here
        }
        // Job handle should be closed here

        println!("✓ Job Object handle drop test passed");
    }

    #[cfg(windows)]
    #[test]
    fn test_multiple_job_object_handles() {
        // Test that we can create multiple Job Object handles
        let job1 = Runner::create_job_object();
        let job2 = Runner::create_job_object();

        assert!(job1.is_ok(), "First Job Object creation should succeed");
        assert!(job2.is_ok(), "Second Job Object creation should succeed");

        println!("✓ Multiple Job Object handles test passed");
    }

    #[cfg(not(windows))]
    #[test]
    fn test_job_objects_not_available_on_non_windows() {
        // Job Objects are Windows-only, so this test just verifies
        // that the code compiles on non-Windows platforms
        println!("⊘ Job Object tests are Windows-only");
    }
}

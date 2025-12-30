//! Runner abstraction for cross-platform Claude CLI execution
//!
//! Provides automatic detection and execution of Claude CLI across Windows, WSL, and native environments.
//! Supports automatic detection (try native first, then WSL on Windows) and explicit mode selection.
//!
//! # Security Model
//!
//! All process execution goes through [`CommandSpec`] to ensure argv-style invocation.
//! This prevents shell injection attacks by ensuring arguments are passed as discrete
//! elements rather than shell strings.

use crate::error::RunnerError;
use crate::ring_buffer::RingBuffer;
use std::collections::HashMap;
use std::env;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command as TokioCommand;
use tokio::time::timeout;

// Re-export for tests
pub use crate::types::RunnerMode;

// ============================================================================
// CommandSpec - Secure Process Execution Specification
// ============================================================================

/// Specification for a command to execute.
///
/// All process execution goes through this type to ensure argv-style invocation.
/// This prevents shell injection attacks by ensuring arguments are passed as
/// discrete elements rather than shell strings.
///
/// # Security
///
/// `CommandSpec` enforces that:
/// - Arguments are `Vec<OsString>`, NOT shell strings
/// - No shell string evaluation (`sh -c`, `cmd /C`) is used
/// - Arguments cross trust boundaries as discrete elements
///
/// # Example
///
/// ```rust
/// use xchecker::runner::CommandSpec;
/// use std::ffi::OsString;
///
/// let cmd = CommandSpec::new("claude")
///     .arg("--print")
///     .arg("--output-format")
///     .arg("json")
///     .cwd("/path/to/workspace");
///
/// assert_eq!(cmd.program, OsString::from("claude"));
/// assert_eq!(cmd.args.len(), 3);
/// ```
#[derive(Debug, Clone)]
pub struct CommandSpec {
    /// The program to execute
    pub program: OsString,
    /// Arguments as discrete elements (NOT shell strings)
    pub args: Vec<OsString>,
    /// Optional working directory
    pub cwd: Option<PathBuf>,
    /// Optional environment overrides
    pub env: Option<HashMap<OsString, OsString>>,
}

impl CommandSpec {
    /// Create a new `CommandSpec` with the given program.
    ///
    /// # Arguments
    ///
    /// * `program` - The program to execute. Can be any type that converts to `OsString`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use xchecker::runner::CommandSpec;
    ///
    /// let cmd = CommandSpec::new("claude");
    /// ```
    #[must_use]
    pub fn new(program: impl Into<OsString>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            cwd: None,
            env: None,
        }
    }

    /// Add a single argument to the command.
    ///
    /// Arguments are stored as discrete `OsString` elements, ensuring no shell
    /// interpretation occurs.
    ///
    /// # Arguments
    ///
    /// * `arg` - The argument to add. Can be any type that converts to `OsString`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use xchecker::runner::CommandSpec;
    ///
    /// let cmd = CommandSpec::new("claude")
    ///     .arg("--print")
    ///     .arg("--verbose");
    /// ```
    #[must_use]
    pub fn arg(mut self, arg: impl Into<OsString>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Add multiple arguments to the command.
    ///
    /// Arguments are stored as discrete `OsString` elements, ensuring no shell
    /// interpretation occurs.
    ///
    /// # Arguments
    ///
    /// * `args` - An iterator of arguments to add.
    ///
    /// # Example
    ///
    /// ```rust
    /// use xchecker::runner::CommandSpec;
    ///
    /// let cmd = CommandSpec::new("claude")
    ///     .args(["--print", "--output-format", "json"]);
    /// ```
    #[must_use]
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    /// Set the working directory for the command.
    ///
    /// # Arguments
    ///
    /// * `cwd` - The working directory path.
    ///
    /// # Example
    ///
    /// ```rust
    /// use xchecker::runner::CommandSpec;
    ///
    /// let cmd = CommandSpec::new("claude")
    ///     .cwd("/path/to/workspace");
    /// ```
    #[must_use]
    pub fn cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    /// Set an environment variable for the command.
    ///
    /// # Arguments
    ///
    /// * `key` - The environment variable name.
    /// * `value` - The environment variable value.
    ///
    /// # Example
    ///
    /// ```rust
    /// use xchecker::runner::CommandSpec;
    ///
    /// let cmd = CommandSpec::new("claude")
    ///     .env("CLAUDE_API_KEY", "sk-...")
    ///     .env("DEBUG", "1");
    /// ```
    #[must_use]
    pub fn env(mut self, key: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        self.env
            .get_or_insert_with(HashMap::new)
            .insert(key.into(), value.into());
        self
    }

    /// Set multiple environment variables for the command.
    ///
    /// # Arguments
    ///
    /// * `envs` - An iterator of (key, value) pairs.
    ///
    /// # Example
    ///
    /// ```rust
    /// use xchecker::runner::CommandSpec;
    ///
    /// let cmd = CommandSpec::new("claude")
    ///     .envs([("DEBUG", "1"), ("VERBOSE", "true")]);
    /// ```
    #[must_use]
    pub fn envs<I, K, V>(mut self, envs: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<OsString>,
        V: Into<OsString>,
    {
        let env_map = self.env.get_or_insert_with(HashMap::new);
        for (key, value) in envs {
            env_map.insert(key.into(), value.into());
        }
        self
    }

    /// Convert this `CommandSpec` into a `std::process::Command`.
    ///
    /// This is the primary way to execute a `CommandSpec`. The resulting `Command`
    /// uses argv-style argument passing, ensuring no shell injection is possible.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use xchecker::runner::CommandSpec;
    ///
    /// let cmd = CommandSpec::new("echo")
    ///     .arg("hello")
    ///     .arg("world");
    ///
    /// let output = cmd.to_command().output().expect("failed to execute");
    /// ```
    #[must_use]
    pub fn to_command(&self) -> Command {
        let mut cmd = Command::new(&self.program);
        cmd.args(&self.args);

        if let Some(ref cwd) = self.cwd {
            cmd.current_dir(cwd);
        }

        if let Some(ref env) = self.env {
            for (key, value) in env {
                cmd.env(key, value);
            }
        }

        cmd
    }

    /// Convert this `CommandSpec` into a `tokio::process::Command`.
    ///
    /// This is used for async execution with timeout support.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use xchecker::runner::CommandSpec;
    ///
    /// # async fn example() {
    /// let cmd = CommandSpec::new("echo")
    ///     .arg("hello");
    ///
    /// let output = cmd.to_tokio_command().output().await.expect("failed to execute");
    /// # }
    /// ```
    #[must_use]
    pub fn to_tokio_command(&self) -> TokioCommand {
        let mut cmd = TokioCommand::new(&self.program);
        cmd.args(&self.args);

        if let Some(ref cwd) = self.cwd {
            cmd.current_dir(cwd);
        }

        if let Some(ref env) = self.env {
            for (key, value) in env {
                cmd.env(key, value);
            }
        }

        cmd
    }
}

impl Default for CommandSpec {
    fn default() -> Self {
        Self {
            program: OsString::new(),
            args: Vec::new(),
            cwd: None,
            env: None,
        }
    }
}

// ============================================================================
// ProcessRunner Trait - Secure Process Execution Interface
// ============================================================================

/// Output from a process execution.
///
/// This is a simplified output type for the `ProcessRunner` trait,
/// containing the essential information from process execution.
#[derive(Debug, Clone)]
pub struct ProcessOutput {
    /// Standard output from the process
    pub stdout: Vec<u8>,
    /// Standard error from the process
    pub stderr: Vec<u8>,
    /// Exit code from the process (None if terminated by signal)
    pub exit_code: Option<i32>,
    /// Whether the execution timed out
    pub timed_out: bool,
}

impl ProcessOutput {
    /// Create a new `ProcessOutput` with the given values.
    #[must_use]
    pub fn new(stdout: Vec<u8>, stderr: Vec<u8>, exit_code: Option<i32>, timed_out: bool) -> Self {
        Self {
            stdout,
            stderr,
            exit_code,
            timed_out,
        }
    }

    /// Get stdout as a UTF-8 string, lossy conversion.
    #[must_use]
    pub fn stdout_string(&self) -> String {
        String::from_utf8_lossy(&self.stdout).to_string()
    }

    /// Get stderr as a UTF-8 string, lossy conversion.
    #[must_use]
    pub fn stderr_string(&self) -> String {
        String::from_utf8_lossy(&self.stderr).to_string()
    }

    /// Check if the process exited successfully (exit code 0).
    #[must_use]
    pub fn success(&self) -> bool {
        self.exit_code == Some(0) && !self.timed_out
    }
}

/// Trait for process execution.
///
/// Implementations MUST use argv-style APIs only (no shell string evaluation).
/// This trait provides a synchronous interface for process execution.
///
/// # Security
///
/// All implementations MUST:
/// - Use `Command::new().args()` style APIs only
/// - NOT use shell string evaluation (`sh -c`, `cmd /C`)
/// - Pass arguments as discrete elements, not concatenated strings
///
/// # Threading
///
/// `ProcessRunner` is a synchronous interface. Implementations MAY internally
/// drive an async runtime (e.g., Tokio for timeouts) but MUST NOT expose async
/// in the public API. This aligns with NFR-ASYNC.
///
/// # Example
///
/// ```rust,no_run
/// use xchecker::runner::{ProcessRunner, CommandSpec, ProcessOutput};
/// use xchecker::error::RunnerError;
/// use std::time::Duration;
///
/// struct MyRunner;
///
/// impl ProcessRunner for MyRunner {
///     fn run(&self, cmd: &CommandSpec, timeout: Duration) -> Result<ProcessOutput, RunnerError> {
///         // Implementation using argv-style APIs only
///         todo!()
///     }
/// }
/// ```
pub trait ProcessRunner {
    /// Execute a command with the given timeout.
    ///
    /// # Arguments
    ///
    /// * `cmd` - The command specification to execute
    /// * `timeout` - Maximum duration to wait for the process to complete
    ///
    /// # Returns
    ///
    /// * `Ok(ProcessOutput)` - The process completed (possibly with non-zero exit code)
    /// * `Err(RunnerError::Timeout)` - The process timed out
    /// * `Err(RunnerError::*)` - Other execution errors
    ///
    /// # Security
    ///
    /// Implementations MUST use argv-style APIs only. The `CommandSpec` ensures
    /// arguments are passed as discrete elements, preventing shell injection.
    fn run(&self, cmd: &CommandSpec, timeout: Duration) -> Result<ProcessOutput, RunnerError>;
}

// ============================================================================
// NativeRunner - Secure Native Process Execution
// ============================================================================

/// Native process runner using `std::process::Command`.
///
/// `NativeRunner` provides secure process execution using argv-style APIs only.
/// It is the primary implementation of [`ProcessRunner`] for native execution
/// without shell interpretation.
///
/// # Security
///
/// `NativeRunner` enforces the following security properties:
/// - Uses `Command::new().args()` only - NO shell string evaluation
/// - Arguments are passed as discrete `OsString` elements
/// - No `sh -c` or `cmd /C` shell invocation
/// - Shell metacharacters in arguments are NOT interpreted
///
/// # Threading
///
/// `NativeRunner` is a synchronous interface. It internally uses a thread-based
/// approach for timeout handling to avoid exposing async in the public API.
/// This aligns with NFR-ASYNC requirements.
///
/// # Example
///
/// ```rust,no_run
/// use xchecker::runner::{NativeRunner, ProcessRunner, CommandSpec};
/// use std::time::Duration;
///
/// let runner = NativeRunner::new();
/// let cmd = CommandSpec::new("echo")
///     .arg("hello")
///     .arg("world");
///
/// let output = runner.run(&cmd, Duration::from_secs(30)).unwrap();
/// assert!(output.success());
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeRunner;

impl NativeRunner {
    /// Create a new `NativeRunner`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use xchecker::runner::NativeRunner;
    ///
    /// let runner = NativeRunner::new();
    /// ```
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl ProcessRunner for NativeRunner {
    /// Execute a command natively using argv-style APIs.
    ///
    /// This implementation:
    /// - Uses `Command::new().args()` only (no shell)
    /// - Handles timeout via thread-based waiting
    /// - Captures stdout and stderr
    /// - Returns exit code or timeout error
    ///
    /// # Arguments
    ///
    /// * `cmd` - The command specification to execute
    /// * `timeout` - Maximum duration to wait for the process to complete
    ///
    /// # Returns
    ///
    /// * `Ok(ProcessOutput)` - The process completed (possibly with non-zero exit code)
    /// * `Err(RunnerError::Timeout)` - The process timed out
    /// * `Err(RunnerError::NativeExecutionFailed)` - Failed to spawn or wait for process
    ///
    /// # Security
    ///
    /// This method uses `CommandSpec::to_command()` which builds a `Command`
    /// using `Command::new().args()` only. No shell string evaluation occurs.
    fn run(&self, cmd: &CommandSpec, timeout: Duration) -> Result<ProcessOutput, RunnerError> {
        use std::sync::mpsc;
        use std::thread;

        // Build the command using argv-style APIs only
        let mut command = cmd.to_command();
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Spawn the process
        let child = command.spawn().map_err(|e| RunnerError::NativeExecutionFailed {
            reason: format!("Failed to spawn process '{}': {}", cmd.program.to_string_lossy(), e),
        })?;

        // Create a channel for the result
        let (tx, rx) = mpsc::channel();

        // Get the child's PID for potential termination
        let child_id = child.id();

        // Spawn a thread to wait for the process
        let handle = thread::spawn(move || {
            let output = child.wait_with_output();
            let _ = tx.send(output);
        });

        // Wait for the result with timeout
        match rx.recv_timeout(timeout) {
            Ok(output_result) => {
                // Process completed within timeout
                let _ = handle.join();
                
                let output = output_result.map_err(|e| RunnerError::NativeExecutionFailed {
                    reason: format!("Failed to wait for process: {e}"),
                })?;

                Ok(ProcessOutput::new(
                    output.stdout,
                    output.stderr,
                    output.status.code(),
                    false,
                ))
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Timeout occurred - attempt to terminate the process
                Self::terminate_process(child_id);
                
                // Wait for the thread to finish (it should complete after termination)
                let _ = handle.join();

                Err(RunnerError::Timeout {
                    timeout_seconds: timeout.as_secs(),
                })
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                // Thread panicked or channel was closed unexpectedly
                Err(RunnerError::NativeExecutionFailed {
                    reason: "Process monitoring thread terminated unexpectedly".to_string(),
                })
            }
        }
    }
}

impl NativeRunner {
    /// Terminate a process by its PID.
    ///
    /// On Unix, sends SIGKILL to the process.
    /// On Windows, uses TerminateProcess.
    fn terminate_process(pid: u32) {
        #[cfg(unix)]
        {
            // Send SIGKILL to the process
            unsafe {
                libc::kill(pid as i32, libc::SIGKILL);
            }
        }

        #[cfg(windows)]
        {
            use windows::Win32::Foundation::CloseHandle;
            use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};

            unsafe {
                if let Ok(handle) = OpenProcess(PROCESS_TERMINATE, false, pid) {
                    let _ = TerminateProcess(handle, 1);
                    let _ = CloseHandle(handle);
                }
            }
        }

        #[cfg(not(any(unix, windows)))]
        {
            // No-op on other platforms
            let _ = pid;
        }
    }
}

// ============================================================================
// WslRunner - Secure WSL Process Execution
// ============================================================================

/// WSL process runner for Windows.
///
/// `WslRunner` provides secure process execution via WSL using argv-style APIs only.
/// It wraps commands with `wsl.exe --exec` to execute them in a WSL distribution
/// without shell interpretation.
///
/// # Security
///
/// `WslRunner` enforces the following security properties:
/// - Uses `wsl.exe --exec` with argv-style argument passing only
/// - Arguments are passed as discrete `OsString` elements via `CommandSpec`
/// - NO shell string concatenation of user data
/// - NO `sh -c` or shell string evaluation
/// - Arguments are validated/normalized at trust boundaries
///
/// # Platform Support
///
/// `WslRunner` is only functional on Windows. On non-Windows platforms,
/// it will return an error indicating WSL is not available.
///
/// # Example
///
/// ```rust,no_run
/// use xchecker::runner::{WslRunner, ProcessRunner, CommandSpec};
/// use std::time::Duration;
///
/// let runner = WslRunner::new();
/// let cmd = CommandSpec::new("echo")
///     .arg("hello")
///     .arg("world");
///
/// // On Windows with WSL, this executes: wsl.exe --exec echo hello world
/// let output = runner.run(&cmd, Duration::from_secs(30)).unwrap();
/// ```
#[derive(Debug, Clone, Default)]
pub struct WslRunner {
    /// Optional specific WSL distro to use (e.g., "Ubuntu-22.04")
    pub distro: Option<String>,
}

impl WslRunner {
    /// Create a new `WslRunner` using the default WSL distribution.
    ///
    /// # Example
    ///
    /// ```rust
    /// use xchecker::runner::WslRunner;
    ///
    /// let runner = WslRunner::new();
    /// ```
    #[must_use]
    pub const fn new() -> Self {
        Self { distro: None }
    }

    /// Create a new `WslRunner` targeting a specific WSL distribution.
    ///
    /// # Arguments
    ///
    /// * `distro` - The name of the WSL distribution (e.g., "Ubuntu-22.04")
    ///
    /// # Example
    ///
    /// ```rust
    /// use xchecker::runner::WslRunner;
    ///
    /// let runner = WslRunner::with_distro("Ubuntu-22.04");
    /// ```
    #[must_use]
    pub fn with_distro(distro: impl Into<String>) -> Self {
        Self {
            distro: Some(distro.into()),
        }
    }

    /// Validate an argument for WSL execution.
    ///
    /// This function validates arguments at trust boundaries to ensure they
    /// don't contain characters that could cause issues in WSL execution.
    ///
    /// # Security
    ///
    /// While `wsl.exe --exec` uses argv-style execution (no shell), we still
    /// validate arguments to:
    /// - Reject null bytes (which could truncate arguments)
    /// - Ensure arguments are valid UTF-8 or valid OS strings
    ///
    /// # Arguments
    ///
    /// * `arg` - The argument to validate
    ///
    /// # Returns
    ///
    /// * `Ok(())` - The argument is valid
    /// * `Err(RunnerError)` - The argument contains invalid characters
    fn validate_argument(arg: &std::ffi::OsStr) -> Result<(), RunnerError> {
        // Check for null bytes which could truncate arguments
        let arg_bytes = arg.as_encoded_bytes();
        if arg_bytes.contains(&0) {
            return Err(RunnerError::WslExecutionFailed {
                reason: "Argument contains null byte which is not allowed".to_string(),
            });
        }
        Ok(())
    }

    /// Build a `CommandSpec` for WSL execution.
    ///
    /// This method transforms the input `CommandSpec` into a WSL-wrapped command
    /// using `wsl.exe --exec` with argv-style argument passing.
    ///
    /// # Security
    ///
    /// - Uses `--exec` flag which bypasses shell interpretation
    /// - Arguments are passed as discrete elements, not concatenated strings
    /// - All arguments are validated before being added to the command
    ///
    /// # Arguments
    ///
    /// * `cmd` - The original command specification to wrap
    ///
    /// # Returns
    ///
    /// * `Ok(CommandSpec)` - The WSL-wrapped command specification
    /// * `Err(RunnerError)` - An argument failed validation
    fn build_wsl_command(&self, cmd: &CommandSpec) -> Result<CommandSpec, RunnerError> {
        // Validate all arguments at the trust boundary
        Self::validate_argument(&cmd.program)?;
        for arg in &cmd.args {
            Self::validate_argument(arg)?;
        }

        // Build the WSL command using argv-style APIs only
        // Command structure: wsl.exe [-d <distro>] --exec <program> <args...>
        let mut wsl_cmd = CommandSpec::new("wsl");

        // Add distro specification if provided (using discrete args, not string concat)
        if let Some(ref distro) = self.distro {
            wsl_cmd = wsl_cmd.arg("-d").arg(distro);
        }

        // Add --exec flag to bypass shell interpretation
        // This is critical for security - it ensures arguments are passed directly
        // to the target program without shell evaluation
        wsl_cmd = wsl_cmd.arg("--exec");

        // Add the original program as a discrete argument
        wsl_cmd = wsl_cmd.arg(&cmd.program);

        // Add all original arguments as discrete elements
        // NO string concatenation occurs here - each arg is a separate element
        for arg in &cmd.args {
            wsl_cmd = wsl_cmd.arg(arg);
        }

        // Preserve working directory if specified
        if let Some(ref cwd) = cmd.cwd {
            wsl_cmd = wsl_cmd.cwd(cwd);
        }

        // Preserve environment variables if specified
        if let Some(ref env) = cmd.env {
            for (key, value) in env {
                wsl_cmd = wsl_cmd.env(key, value);
            }
        }

        Ok(wsl_cmd)
    }
}

impl ProcessRunner for WslRunner {
    /// Execute a command via WSL using argv-style APIs.
    ///
    /// This implementation:
    /// - Wraps the command with `wsl.exe --exec` (no shell)
    /// - Uses `Command::new().args()` only (no shell string evaluation)
    /// - Validates arguments at trust boundaries
    /// - Handles timeout via thread-based waiting
    /// - Captures stdout and stderr
    ///
    /// # Arguments
    ///
    /// * `cmd` - The command specification to execute
    /// * `timeout` - Maximum duration to wait for the process to complete
    ///
    /// # Returns
    ///
    /// * `Ok(ProcessOutput)` - The process completed (possibly with non-zero exit code)
    /// * `Err(RunnerError::Timeout)` - The process timed out
    /// * `Err(RunnerError::WslExecutionFailed)` - Failed to spawn or wait for process
    /// * `Err(RunnerError::WslNotAvailable)` - WSL is not available (non-Windows platform)
    ///
    /// # Security
    ///
    /// This method builds a WSL command using `build_wsl_command()` which:
    /// - Uses `--exec` to bypass shell interpretation
    /// - Passes arguments as discrete elements via `CommandSpec`
    /// - Validates all arguments at trust boundaries
    /// - NO shell string concatenation of user data occurs
    fn run(&self, cmd: &CommandSpec, timeout: Duration) -> Result<ProcessOutput, RunnerError> {
        // WSL is only available on Windows
        if !cfg!(target_os = "windows") {
            return Err(RunnerError::WslNotAvailable {
                reason: "WSL is only available on Windows".to_string(),
            });
        }

        use std::sync::mpsc;
        use std::thread;

        // Build the WSL-wrapped command using argv-style APIs
        let wsl_cmd = self.build_wsl_command(cmd)?;

        // Convert to std::process::Command using argv-style APIs only
        let mut command = wsl_cmd.to_command();
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Spawn the process
        let child = command.spawn().map_err(|e| RunnerError::WslExecutionFailed {
            reason: format!(
                "Failed to spawn WSL process for '{}': {}",
                cmd.program.to_string_lossy(),
                e
            ),
        })?;

        // Get the child's PID for potential termination
        let child_id = child.id();

        // Create a channel for the result
        let (tx, rx) = mpsc::channel();

        // Spawn a thread to wait for the process
        let handle = thread::spawn(move || {
            let output = child.wait_with_output();
            let _ = tx.send(output);
        });

        // Wait for the result with timeout
        match rx.recv_timeout(timeout) {
            Ok(output_result) => {
                // Process completed within timeout
                let _ = handle.join();

                let output = output_result.map_err(|e| RunnerError::WslExecutionFailed {
                    reason: format!("Failed to wait for WSL process: {e}"),
                })?;

                Ok(ProcessOutput::new(
                    output.stdout,
                    output.stderr,
                    output.status.code(),
                    false,
                ))
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Timeout occurred - attempt to terminate the process
                Self::terminate_wsl_process(child_id);

                // Wait for the thread to finish (it should complete after termination)
                let _ = handle.join();

                Err(RunnerError::Timeout {
                    timeout_seconds: timeout.as_secs(),
                })
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                // Thread panicked or channel was closed unexpectedly
                Err(RunnerError::WslExecutionFailed {
                    reason: "WSL process monitoring thread terminated unexpectedly".to_string(),
                })
            }
        }
    }
}

impl WslRunner {
    /// Terminate a WSL process by its PID.
    ///
    /// On Windows, uses TerminateProcess to kill the wsl.exe process,
    /// which will also terminate the child process in WSL.
    fn terminate_wsl_process(pid: u32) {
        #[cfg(windows)]
        {
            use windows::Win32::Foundation::CloseHandle;
            use windows::Win32::System::Threading::{
                OpenProcess, TerminateProcess, PROCESS_TERMINATE,
            };

            unsafe {
                if let Ok(handle) = OpenProcess(PROCESS_TERMINATE, false, pid) {
                    let _ = TerminateProcess(handle, 1);
                    let _ = CloseHandle(handle);
                }
            }
        }

        #[cfg(not(windows))]
        {
            // No-op on non-Windows platforms
            let _ = pid;
        }
    }
}

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
        let mut cmd = CommandSpec::new("claude")
            .args(args)
            .to_tokio_command();

        cmd.stdin(Stdio::piped())
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
        // Use CommandSpec to ensure secure argument passing
        let mut spec = CommandSpec::new("wsl");

        // Add distro specification if provided
        if let Some(distro) = &self.wsl_options.distro {
            spec = spec.args(["-d", distro]);
        }

        spec = spec.arg("--exec").arg(claude_path).args(args);

        let mut cmd = spec.to_tokio_command();
        cmd.stdin(Stdio::piped())
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

        // Try to get default distro using CommandSpec
        if let Ok(output) = CommandSpec::new("wsl").args(["-l", "-q"]).to_command().output()
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
    use proptest::prelude::*;

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

    // ============================================================================
    // CommandSpec Tests (FR-SEC-15)
    // ============================================================================

    #[test]
    fn test_command_spec_new() {
        let cmd = CommandSpec::new("claude");
        assert_eq!(cmd.program, OsString::from("claude"));
        assert!(cmd.args.is_empty());
        assert!(cmd.cwd.is_none());
        assert!(cmd.env.is_none());
    }

    #[test]
    fn test_command_spec_arg() {
        let cmd = CommandSpec::new("claude")
            .arg("--print")
            .arg("--verbose");
        assert_eq!(cmd.args.len(), 2);
        assert_eq!(cmd.args[0], OsString::from("--print"));
        assert_eq!(cmd.args[1], OsString::from("--verbose"));
    }

    #[test]
    fn test_command_spec_args() {
        let cmd = CommandSpec::new("claude")
            .args(["--print", "--output-format", "json"]);
        assert_eq!(cmd.args.len(), 3);
        assert_eq!(cmd.args[0], OsString::from("--print"));
        assert_eq!(cmd.args[1], OsString::from("--output-format"));
        assert_eq!(cmd.args[2], OsString::from("json"));
    }

    #[test]
    fn test_command_spec_cwd() {
        let cmd = CommandSpec::new("claude")
            .cwd("/path/to/workspace");
        assert_eq!(cmd.cwd, Some(PathBuf::from("/path/to/workspace")));
    }

    #[test]
    fn test_command_spec_env() {
        let cmd = CommandSpec::new("claude")
            .env("DEBUG", "1")
            .env("VERBOSE", "true");
        let env = cmd.env.as_ref().unwrap();
        assert_eq!(env.len(), 2);
        assert_eq!(env.get(&OsString::from("DEBUG")), Some(&OsString::from("1")));
        assert_eq!(env.get(&OsString::from("VERBOSE")), Some(&OsString::from("true")));
    }

    #[test]
    fn test_command_spec_envs() {
        let cmd = CommandSpec::new("claude")
            .envs([("DEBUG", "1"), ("VERBOSE", "true")]);
        let env = cmd.env.as_ref().unwrap();
        assert_eq!(env.len(), 2);
        assert_eq!(env.get(&OsString::from("DEBUG")), Some(&OsString::from("1")));
        assert_eq!(env.get(&OsString::from("VERBOSE")), Some(&OsString::from("true")));
    }

    #[test]
    fn test_command_spec_builder_chain() {
        let cmd = CommandSpec::new("claude")
            .arg("--print")
            .args(["--output-format", "json"])
            .cwd("/workspace")
            .env("DEBUG", "1")
            .envs([("VERBOSE", "true")]);

        assert_eq!(cmd.program, OsString::from("claude"));
        assert_eq!(cmd.args.len(), 3);
        assert_eq!(cmd.cwd, Some(PathBuf::from("/workspace")));
        let env = cmd.env.as_ref().unwrap();
        assert_eq!(env.len(), 2);
    }

    #[test]
    fn test_command_spec_default() {
        let cmd = CommandSpec::default();
        assert_eq!(cmd.program, OsString::new());
        assert!(cmd.args.is_empty());
        assert!(cmd.cwd.is_none());
        assert!(cmd.env.is_none());
    }

    #[test]
    fn test_command_spec_clone() {
        let cmd = CommandSpec::new("claude")
            .arg("--print")
            .cwd("/workspace")
            .env("DEBUG", "1");
        let cloned = cmd.clone();

        assert_eq!(cloned.program, cmd.program);
        assert_eq!(cloned.args, cmd.args);
        assert_eq!(cloned.cwd, cmd.cwd);
        assert_eq!(cloned.env, cmd.env);
    }

    #[test]
    fn test_command_spec_to_command() {
        let cmd = CommandSpec::new("echo")
            .arg("hello")
            .arg("world");

        // Verify we can create a std::process::Command
        let std_cmd = cmd.to_command();
        // We can't easily inspect the Command, but we can verify it doesn't panic
        assert!(std::mem::size_of_val(&std_cmd) > 0);
    }

    #[test]
    fn test_command_spec_to_tokio_command() {
        let cmd = CommandSpec::new("echo")
            .arg("hello");

        // Verify we can create a tokio::process::Command
        let tokio_cmd = cmd.to_tokio_command();
        // We can't easily inspect the Command, but we can verify it doesn't panic
        assert!(std::mem::size_of_val(&tokio_cmd) > 0);
    }

    #[test]
    fn test_command_spec_osstring_args() {
        // Test that we can use OsString directly
        let cmd = CommandSpec::new(OsString::from("claude"))
            .arg(OsString::from("--print"));
        assert_eq!(cmd.program, OsString::from("claude"));
        assert_eq!(cmd.args[0], OsString::from("--print"));
    }

    #[test]
    fn test_command_spec_args_are_vec_osstring() {
        // Verify args are stored as Vec<OsString>, not shell strings
        let cmd = CommandSpec::new("claude")
            .arg("arg with spaces")
            .arg("arg;with;semicolons")
            .arg("arg|with|pipes")
            .arg("arg&with&ampersands");

        // Each argument should be stored as a discrete OsString element
        assert_eq!(cmd.args.len(), 4);
        assert_eq!(cmd.args[0], OsString::from("arg with spaces"));
        assert_eq!(cmd.args[1], OsString::from("arg;with;semicolons"));
        assert_eq!(cmd.args[2], OsString::from("arg|with|pipes"));
        assert_eq!(cmd.args[3], OsString::from("arg&with&ampersands"));
    }

    #[test]
    fn test_command_spec_shell_metacharacters_preserved() {
        // Verify that shell metacharacters are preserved as-is (not interpreted)
        // This is critical for security - we don't want shell injection
        let cmd = CommandSpec::new("echo")
            .arg("$(whoami)")
            .arg("`id`")
            .arg("${HOME}")
            .arg("$PATH");

        // These should be stored literally, not expanded
        assert_eq!(cmd.args[0], OsString::from("$(whoami)"));
        assert_eq!(cmd.args[1], OsString::from("`id`"));
        assert_eq!(cmd.args[2], OsString::from("${HOME}"));
        assert_eq!(cmd.args[3], OsString::from("$PATH"));
    }

    // ============================================================================
    // ProcessOutput Tests
    // ============================================================================

    #[test]
    fn test_process_output_new() {
        let output = ProcessOutput::new(
            b"stdout content".to_vec(),
            b"stderr content".to_vec(),
            Some(0),
            false,
        );
        assert_eq!(output.stdout, b"stdout content");
        assert_eq!(output.stderr, b"stderr content");
        assert_eq!(output.exit_code, Some(0));
        assert!(!output.timed_out);
    }

    #[test]
    fn test_process_output_stdout_string() {
        let output = ProcessOutput::new(
            b"hello world".to_vec(),
            Vec::new(),
            Some(0),
            false,
        );
        assert_eq!(output.stdout_string(), "hello world");
    }

    #[test]
    fn test_process_output_stderr_string() {
        let output = ProcessOutput::new(
            Vec::new(),
            b"error message".to_vec(),
            Some(1),
            false,
        );
        assert_eq!(output.stderr_string(), "error message");
    }

    #[test]
    fn test_process_output_success() {
        // Success case: exit code 0, not timed out
        let success = ProcessOutput::new(Vec::new(), Vec::new(), Some(0), false);
        assert!(success.success());

        // Failure case: non-zero exit code
        let failure = ProcessOutput::new(Vec::new(), Vec::new(), Some(1), false);
        assert!(!failure.success());

        // Failure case: timed out
        let timeout = ProcessOutput::new(Vec::new(), Vec::new(), Some(0), true);
        assert!(!timeout.success());

        // Failure case: no exit code (killed by signal)
        let killed = ProcessOutput::new(Vec::new(), Vec::new(), None, false);
        assert!(!killed.success());
    }

    #[test]
    fn test_process_output_clone() {
        let output = ProcessOutput::new(
            b"stdout".to_vec(),
            b"stderr".to_vec(),
            Some(42),
            true,
        );
        let cloned = output.clone();
        assert_eq!(cloned.stdout, output.stdout);
        assert_eq!(cloned.stderr, output.stderr);
        assert_eq!(cloned.exit_code, output.exit_code);
        assert_eq!(cloned.timed_out, output.timed_out);
    }

    #[test]
    fn test_process_output_lossy_utf8() {
        // Test that invalid UTF-8 is handled gracefully
        let invalid_utf8 = vec![0xff, 0xfe, 0x00, 0x01];
        let output = ProcessOutput::new(invalid_utf8.clone(), invalid_utf8, Some(0), false);
        
        // Should not panic, should produce replacement characters
        let stdout = output.stdout_string();
        let stderr = output.stderr_string();
        assert!(!stdout.is_empty());
        assert!(!stderr.is_empty());
    }

    // ============================================================================
    // ProcessRunner Trait Tests
    // ============================================================================

    /// A mock implementation of ProcessRunner for testing
    struct MockRunner {
        expected_output: ProcessOutput,
    }

    impl ProcessRunner for MockRunner {
        fn run(&self, _cmd: &CommandSpec, _timeout: Duration) -> Result<ProcessOutput, RunnerError> {
            Ok(self.expected_output.clone())
        }
    }

    #[test]
    fn test_process_runner_trait_implementation() {
        // Verify that we can implement the ProcessRunner trait
        let mock = MockRunner {
            expected_output: ProcessOutput::new(
                b"mock stdout".to_vec(),
                b"mock stderr".to_vec(),
                Some(0),
                false,
            ),
        };

        let cmd = CommandSpec::new("test").arg("--flag");
        let result = mock.run(&cmd, Duration::from_secs(30));

        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.stdout_string(), "mock stdout");
        assert_eq!(output.stderr_string(), "mock stderr");
        assert!(output.success());
    }

    #[test]
    fn test_process_runner_with_error() {
        /// A mock runner that always returns an error
        struct ErrorRunner;

        impl ProcessRunner for ErrorRunner {
            fn run(&self, _cmd: &CommandSpec, _timeout: Duration) -> Result<ProcessOutput, RunnerError> {
                Err(RunnerError::NativeExecutionFailed {
                    reason: "mock error".to_string(),
                })
            }
        }

        let runner = ErrorRunner;
        let cmd = CommandSpec::new("test");
        let result = runner.run(&cmd, Duration::from_secs(30));

        assert!(result.is_err());
        match result {
            Err(RunnerError::NativeExecutionFailed { reason }) => {
                assert_eq!(reason, "mock error");
            }
            _ => panic!("Expected NativeExecutionFailed error"),
        }
    }

    #[test]
    fn test_process_runner_with_timeout_error() {
        /// A mock runner that simulates a timeout
        struct TimeoutRunner;

        impl ProcessRunner for TimeoutRunner {
            fn run(&self, _cmd: &CommandSpec, timeout: Duration) -> Result<ProcessOutput, RunnerError> {
                Err(RunnerError::Timeout {
                    timeout_seconds: timeout.as_secs(),
                })
            }
        }

        let runner = TimeoutRunner;
        let cmd = CommandSpec::new("test");
        let result = runner.run(&cmd, Duration::from_secs(60));

        assert!(result.is_err());
        match result {
            Err(RunnerError::Timeout { timeout_seconds }) => {
                assert_eq!(timeout_seconds, 60);
            }
            _ => panic!("Expected Timeout error"),
        }
    }

    // ============================================================================
    // NativeRunner Tests (FR-SEC-15, FR-SEC-16)
    // ============================================================================

    #[test]
    fn test_native_runner_new() {
        let runner = NativeRunner::new();
        // NativeRunner is a zero-sized type, just verify it can be created
        assert!(std::mem::size_of_val(&runner) == 0);
    }

    #[test]
    fn test_native_runner_default() {
        let runner = NativeRunner;
        assert!(std::mem::size_of_val(&runner) == 0);
    }

    #[test]
    fn test_native_runner_clone() {
        let runner = NativeRunner::new();
        let cloned = runner;
        // Both should be valid (Copy type)
        assert!(std::mem::size_of_val(&runner) == 0);
        assert!(std::mem::size_of_val(&cloned) == 0);
    }

    #[test]
    fn test_native_runner_echo_command() {
        // Test that NativeRunner can execute a simple echo command
        // This verifies argv-style execution works correctly
        let runner = NativeRunner::new();
        
        #[cfg(windows)]
        let cmd = CommandSpec::new("cmd")
            .arg("/C")
            .arg("echo")
            .arg("hello world");
        
        #[cfg(not(windows))]
        let cmd = CommandSpec::new("echo")
            .arg("hello world");

        let result = runner.run(&cmd, Duration::from_secs(10));
        
        assert!(result.is_ok(), "Echo command should succeed: {:?}", result);
        let output = result.unwrap();
        assert!(output.success(), "Echo should exit with code 0");
        assert!(output.stdout_string().contains("hello world"), 
            "Output should contain 'hello world', got: {}", output.stdout_string());
    }

    #[test]
    fn test_native_runner_shell_metacharacters_not_interpreted() {
        // Test that shell metacharacters are NOT interpreted
        // This is critical for security - verifies no shell injection
        let runner = NativeRunner::new();
        
        // Use echo with shell metacharacters that should be passed literally
        #[cfg(windows)]
        let cmd = CommandSpec::new("cmd")
            .arg("/C")
            .arg("echo")
            .arg("$PATH");
        
        #[cfg(not(windows))]
        let cmd = CommandSpec::new("echo")
            .arg("$PATH");

        let result = runner.run(&cmd, Duration::from_secs(10));
        
        assert!(result.is_ok(), "Command should succeed");
        let output = result.unwrap();
        // The literal string "$PATH" should appear in output, not the expanded PATH variable
        // Note: On Windows cmd /C echo $PATH will output "$PATH" literally
        // On Unix, echo "$PATH" will also output "$PATH" literally since we use argv
        assert!(output.stdout_string().contains("$PATH") || output.stdout_string().contains("PATH"),
            "Shell metacharacter should be preserved or echoed, got: {}", output.stdout_string());
    }

    #[test]
    fn test_native_runner_nonexistent_command() {
        // Test that running a nonexistent command returns an error
        let runner = NativeRunner::new();
        let cmd = CommandSpec::new("this_command_definitely_does_not_exist_12345");

        let result = runner.run(&cmd, Duration::from_secs(10));
        
        assert!(result.is_err(), "Nonexistent command should fail");
        match result {
            Err(RunnerError::NativeExecutionFailed { reason }) => {
                assert!(reason.contains("this_command_definitely_does_not_exist_12345"),
                    "Error should mention the command name: {}", reason);
            }
            _ => panic!("Expected NativeExecutionFailed error"),
        }
    }

    #[test]
    fn test_native_runner_exit_code_propagation() {
        // Test that non-zero exit codes are properly propagated
        let runner = NativeRunner::new();
        
        #[cfg(windows)]
        let cmd = CommandSpec::new("cmd")
            .arg("/C")
            .arg("exit")
            .arg("42");
        
        #[cfg(not(windows))]
        let cmd = CommandSpec::new("sh")
            .arg("-c")
            .arg("exit 42");

        let result = runner.run(&cmd, Duration::from_secs(10));
        
        assert!(result.is_ok(), "Command should complete (even with non-zero exit)");
        let output = result.unwrap();
        assert!(!output.success(), "Exit code 42 should not be success");
        assert_eq!(output.exit_code, Some(42), "Exit code should be 42");
    }

    #[test]
    fn test_native_runner_stderr_capture() {
        // Test that stderr is properly captured
        let runner = NativeRunner::new();
        
        #[cfg(windows)]
        let cmd = CommandSpec::new("cmd")
            .arg("/C")
            .arg("echo error message 1>&2");
        
        #[cfg(not(windows))]
        let cmd = CommandSpec::new("sh")
            .arg("-c")
            .arg("echo 'error message' >&2");

        let result = runner.run(&cmd, Duration::from_secs(10));
        
        assert!(result.is_ok(), "Command should succeed");
        let output = result.unwrap();
        assert!(output.stderr_string().contains("error message"),
            "Stderr should contain 'error message', got: {}", output.stderr_string());
    }

    #[test]
    fn test_native_runner_implements_process_runner() {
        // Verify NativeRunner implements ProcessRunner trait
        fn assert_process_runner<T: ProcessRunner>(_: &T) {}
        
        let runner = NativeRunner::new();
        assert_process_runner(&runner);
    }

    // ============================================================================
    // WslRunner Tests (FR-SEC-17, FR-SEC-18)
    // ============================================================================

    #[test]
    fn test_wsl_runner_new() {
        let runner = WslRunner::new();
        assert!(runner.distro.is_none());
    }

    #[test]
    fn test_wsl_runner_with_distro() {
        let runner = WslRunner::with_distro("Ubuntu-22.04");
        assert_eq!(runner.distro, Some("Ubuntu-22.04".to_string()));
    }

    #[test]
    fn test_wsl_runner_default() {
        let runner = WslRunner::default();
        assert!(runner.distro.is_none());
    }

    #[test]
    fn test_wsl_runner_clone() {
        let runner = WslRunner::with_distro("Ubuntu");
        let cloned = runner.clone();
        assert_eq!(cloned.distro, runner.distro);
    }

    #[test]
    fn test_wsl_runner_implements_process_runner() {
        // Verify WslRunner implements ProcessRunner trait
        fn assert_process_runner<T: ProcessRunner>(_: &T) {}
        
        let runner = WslRunner::new();
        assert_process_runner(&runner);
    }

    #[test]
    fn test_wsl_runner_build_command_basic() {
        let runner = WslRunner::new();
        let cmd = CommandSpec::new("echo")
            .arg("hello")
            .arg("world");

        let wsl_cmd = runner.build_wsl_command(&cmd).unwrap();

        // Should be: wsl --exec echo hello world
        assert_eq!(wsl_cmd.program, OsString::from("wsl"));
        assert_eq!(wsl_cmd.args.len(), 4);
        assert_eq!(wsl_cmd.args[0], OsString::from("--exec"));
        assert_eq!(wsl_cmd.args[1], OsString::from("echo"));
        assert_eq!(wsl_cmd.args[2], OsString::from("hello"));
        assert_eq!(wsl_cmd.args[3], OsString::from("world"));
    }

    #[test]
    fn test_wsl_runner_build_command_with_distro() {
        let runner = WslRunner::with_distro("Ubuntu-22.04");
        let cmd = CommandSpec::new("echo")
            .arg("test");

        let wsl_cmd = runner.build_wsl_command(&cmd).unwrap();

        // Should be: wsl -d Ubuntu-22.04 --exec echo test
        assert_eq!(wsl_cmd.program, OsString::from("wsl"));
        assert_eq!(wsl_cmd.args.len(), 5);
        assert_eq!(wsl_cmd.args[0], OsString::from("-d"));
        assert_eq!(wsl_cmd.args[1], OsString::from("Ubuntu-22.04"));
        assert_eq!(wsl_cmd.args[2], OsString::from("--exec"));
        assert_eq!(wsl_cmd.args[3], OsString::from("echo"));
        assert_eq!(wsl_cmd.args[4], OsString::from("test"));
    }

    #[test]
    fn test_wsl_runner_build_command_preserves_cwd() {
        let runner = WslRunner::new();
        let cmd = CommandSpec::new("ls")
            .cwd("/home/user");

        let wsl_cmd = runner.build_wsl_command(&cmd).unwrap();

        assert_eq!(wsl_cmd.cwd, Some(PathBuf::from("/home/user")));
    }

    #[test]
    fn test_wsl_runner_build_command_preserves_env() {
        let runner = WslRunner::new();
        let cmd = CommandSpec::new("env")
            .env("MY_VAR", "my_value");

        let wsl_cmd = runner.build_wsl_command(&cmd).unwrap();

        let env = wsl_cmd.env.as_ref().unwrap();
        assert_eq!(env.get(&OsString::from("MY_VAR")), Some(&OsString::from("my_value")));
    }

    #[test]
    fn test_wsl_runner_build_command_shell_metacharacters_preserved() {
        // Verify that shell metacharacters are preserved as discrete arguments
        // This is critical for security - no shell injection
        let runner = WslRunner::new();
        let cmd = CommandSpec::new("echo")
            .arg("$(whoami)")
            .arg("`id`")
            .arg("${HOME}")
            .arg("$PATH")
            .arg("arg;with;semicolons")
            .arg("arg|with|pipes")
            .arg("arg&with&ampersands");

        let wsl_cmd = runner.build_wsl_command(&cmd).unwrap();

        // All arguments should be preserved literally as discrete elements
        // wsl --exec echo $(whoami) `id` ${HOME} $PATH arg;with;semicolons arg|with|pipes arg&with&ampersands
        assert_eq!(wsl_cmd.args[2], OsString::from("$(whoami)"));
        assert_eq!(wsl_cmd.args[3], OsString::from("`id`"));
        assert_eq!(wsl_cmd.args[4], OsString::from("${HOME}"));
        assert_eq!(wsl_cmd.args[5], OsString::from("$PATH"));
        assert_eq!(wsl_cmd.args[6], OsString::from("arg;with;semicolons"));
        assert_eq!(wsl_cmd.args[7], OsString::from("arg|with|pipes"));
        assert_eq!(wsl_cmd.args[8], OsString::from("arg&with&ampersands"));
    }

    #[test]
    fn test_wsl_runner_validate_argument_rejects_null_bytes() {
        // Arguments with null bytes should be rejected
        let arg_with_null = OsString::from("hello\0world");
        let result = WslRunner::validate_argument(&arg_with_null);
        
        assert!(result.is_err());
        match result {
            Err(RunnerError::WslExecutionFailed { reason }) => {
                assert!(reason.contains("null byte"));
            }
            _ => panic!("Expected WslExecutionFailed error"),
        }
    }

    #[test]
    fn test_wsl_runner_validate_argument_accepts_valid_args() {
        // Valid arguments should be accepted
        let valid_args = [
            "simple",
            "with spaces",
            "with-dashes",
            "with_underscores",
            "with.dots",
            "/path/to/file",
            "C:\\Windows\\Path",
            "unicode: 日本語",
            "emoji: 🎉",
            "--flag=value",
            "-v",
            "$(not-expanded)",
            "`backticks`",
            "${variable}",
        ];

        for arg in valid_args {
            let os_arg = OsString::from(arg);
            let result = WslRunner::validate_argument(&os_arg);
            assert!(result.is_ok(), "Argument '{}' should be valid", arg);
        }
    }

    #[test]
    fn test_wsl_runner_build_command_rejects_null_in_program() {
        let runner = WslRunner::new();
        let cmd = CommandSpec::new("echo\0bad");

        let result = runner.build_wsl_command(&cmd);
        
        assert!(result.is_err());
        match result {
            Err(RunnerError::WslExecutionFailed { reason }) => {
                assert!(reason.contains("null byte"));
            }
            _ => panic!("Expected WslExecutionFailed error"),
        }
    }

    #[test]
    fn test_wsl_runner_build_command_rejects_null_in_args() {
        let runner = WslRunner::new();
        let cmd = CommandSpec::new("echo")
            .arg("valid")
            .arg("has\0null")
            .arg("also valid");

        let result = runner.build_wsl_command(&cmd);
        
        assert!(result.is_err());
        match result {
            Err(RunnerError::WslExecutionFailed { reason }) => {
                assert!(reason.contains("null byte"));
            }
            _ => panic!("Expected WslExecutionFailed error"),
        }
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_wsl_runner_returns_error_on_non_windows() {
        // On non-Windows platforms, WslRunner should return an error
        let runner = WslRunner::new();
        let cmd = CommandSpec::new("echo").arg("test");

        let result = runner.run(&cmd, Duration::from_secs(10));
        
        assert!(result.is_err());
        match result {
            Err(RunnerError::WslNotAvailable { reason }) => {
                assert!(reason.contains("only available on Windows"));
            }
            _ => panic!("Expected WslNotAvailable error"),
        }
    }

    #[test]
    fn test_wsl_runner_no_string_concatenation() {
        // This test verifies that arguments are passed as discrete elements
        // and no string concatenation occurs
        let runner = WslRunner::with_distro("TestDistro");
        let cmd = CommandSpec::new("program")
            .arg("arg1")
            .arg("arg2 with spaces")
            .arg("arg3;semicolon");

        let wsl_cmd = runner.build_wsl_command(&cmd).unwrap();

        // Verify each argument is a discrete element
        // The command should be: wsl -d TestDistro --exec program arg1 "arg2 with spaces" "arg3;semicolon"
        // But stored as discrete OsString elements, not concatenated
        
        // Count total args: -d, TestDistro, --exec, program, arg1, arg2 with spaces, arg3;semicolon
        assert_eq!(wsl_cmd.args.len(), 7);
        
        // Each argument should be exactly what we passed, not concatenated
        assert_eq!(wsl_cmd.args[4], OsString::from("arg1"));
        assert_eq!(wsl_cmd.args[5], OsString::from("arg2 with spaces"));
        assert_eq!(wsl_cmd.args[6], OsString::from("arg3;semicolon"));
    }

    #[test]
    fn test_wsl_runner_command_construction() {
        // This test verifies that WslRunner correctly wraps commands with --exec
        // to bypass shell interpretation.
        
        let runner = WslRunner::new();
        let cmd = CommandSpec::new("echo")
            .arg("hello")
            .arg("world");
            
        let wsl_cmd = runner.build_wsl_command(&cmd).expect("Failed to build WSL command");
        
        // Verify program is wsl
        assert_eq!(wsl_cmd.program, OsString::from("wsl"));
        
        // Verify arguments structure: --exec echo hello world
        let args: Vec<String> = wsl_cmd.args.iter()
            .map(|s| s.to_string_lossy().to_string())
            .collect();
            
        assert_eq!(args[0], "--exec");
        assert_eq!(args[1], "echo");
        assert_eq!(args[2], "hello");
        assert_eq!(args[3], "world");
        
        // Verify no shell wrapping (sh -c, etc)
        for arg in &args {
            assert!(!arg.contains("sh -c"));
            assert!(!arg.contains("cmd /C"));
        }
    }

    #[test]
    fn test_wsl_runner_with_distro_command_construction() {
        let runner = WslRunner::with_distro("Ubuntu-22.04");
        let cmd = CommandSpec::new("ls").arg("-la");
        
        let wsl_cmd = runner.build_wsl_command(&cmd).expect("Failed to build WSL command");
        
        let args: Vec<String> = wsl_cmd.args.iter()
            .map(|s| s.to_string_lossy().to_string())
            .collect();
            
        // Verify structure: -d Ubuntu-22.04 --exec ls -la
        assert_eq!(args[0], "-d");
        assert_eq!(args[1], "Ubuntu-22.04");
        assert_eq!(args[2], "--exec");
        assert_eq!(args[3], "ls");
        assert_eq!(args[4], "-la");
    }

    #[test]
    fn test_wsl_runner_argument_validation() {
        // Verify that arguments with null bytes are rejected
        let runner = WslRunner::new();
        
        // Create a string with a null byte
        let cmd = CommandSpec::new("echo").arg("hello\0world");
        
        let result = runner.build_wsl_command(&cmd);
        assert!(result.is_err());
        
        if let Err(RunnerError::WslExecutionFailed { reason }) = result {
            assert!(reason.contains("null byte"));
        } else {
            panic!("Expected WslExecutionFailed error");
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]
        #[test]
        fn test_wsl_runner_safety_property(
            program in any::<String>(),
            args in prop::collection::vec(any::<String>(), 0..10),
            distro in prop::option::of(any::<String>())
        ) {
            // Property 17: WSL runner safety
            // Validates: Requirements FR-SEC-4
            
            let mut runner = WslRunner::new();
            if let Some(ref d) = distro {
                runner = WslRunner::with_distro(d.clone());
            }
            
            let mut cmd = CommandSpec::new(&program);
            for arg in &args {
                cmd = cmd.arg(arg);
            }
            
            let result = runner.build_wsl_command(&cmd);
            
            // Check for null bytes in inputs
            let has_null = program.contains('\0') || args.iter().any(|a| a.contains('\0'));
            
            if has_null {
                // Must fail if null bytes are present
                prop_assert!(result.is_err());
            } else {
                // Must succeed if no null bytes
                prop_assert!(result.is_ok());
                let wsl_cmd = result.unwrap();
                
                // Verify structure
                prop_assert_eq!(wsl_cmd.program, OsString::from("wsl"));
                
                let mut expected_args_len = 1; // --exec
                let mut arg_idx = 0;
                
                // Check distro args
                if let Some(ref d) = runner.distro {
                    prop_assert_eq!(&wsl_cmd.args[arg_idx], &OsString::from("-d"));
                    prop_assert_eq!(&wsl_cmd.args[arg_idx+1], &OsString::from(d));
                    arg_idx += 2;
                    expected_args_len += 2;
                }
                
                // Check --exec
                prop_assert_eq!(&wsl_cmd.args[arg_idx], &OsString::from("--exec"));
                arg_idx += 1;
                
                // Check program
                prop_assert_eq!(&wsl_cmd.args[arg_idx], &OsString::from(&program));
                arg_idx += 1;
                expected_args_len += 1;
                
                // Check args
                for (i, arg) in args.iter().enumerate() {
                    prop_assert_eq!(&wsl_cmd.args[arg_idx + i], &OsString::from(arg));
                }
                expected_args_len += args.len();
                
                prop_assert_eq!(wsl_cmd.args.len(), expected_args_len);
            }
        }
    }
}


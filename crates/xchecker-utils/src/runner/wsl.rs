use crate::error::RunnerError;
use std::ffi::OsStr;
use std::process::Stdio;
use std::time::Duration;

use super::{CommandSpec, ProcessOutput, ProcessRunner};

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
/// use xchecker_utils::runner::{WslRunner, ProcessRunner, CommandSpec};
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
    /// use xchecker_utils::runner::WslRunner;
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
    /// use xchecker_utils::runner::WslRunner;
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
    fn validate_argument(arg: &OsStr) -> Result<(), RunnerError> {
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
        let child = command
            .spawn()
            .map_err(|e| RunnerError::WslExecutionFailed {
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
                OpenProcess, PROCESS_TERMINATE, TerminateProcess,
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

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::ffi::OsString;
    use std::path::PathBuf;

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
        let cmd = CommandSpec::new("echo").arg("hello").arg("world");

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
        let cmd = CommandSpec::new("echo").arg("test");

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
        let cmd = CommandSpec::new("ls").cwd("/home/user");

        let wsl_cmd = runner.build_wsl_command(&cmd).unwrap();

        assert_eq!(wsl_cmd.cwd, Some(PathBuf::from("/home/user")));
    }

    #[test]
    fn test_wsl_runner_build_command_preserves_env() {
        let runner = WslRunner::new();
        let cmd = CommandSpec::new("env").env("MY_VAR", "my_value");

        let wsl_cmd = runner.build_wsl_command(&cmd).unwrap();

        let env = wsl_cmd.env.as_ref().unwrap();
        assert_eq!(
            env.get(&OsString::from("MY_VAR")),
            Some(&OsString::from("my_value"))
        );
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
        let cmd = CommandSpec::new("echo").arg("hello").arg("world");

        let wsl_cmd = runner
            .build_wsl_command(&cmd)
            .expect("Failed to build WSL command");

        // Verify program is wsl
        assert_eq!(wsl_cmd.program, OsString::from("wsl"));

        // Verify arguments structure: --exec echo hello world
        let args: Vec<String> = wsl_cmd
            .args
            .iter()
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

        let wsl_cmd = runner
            .build_wsl_command(&cmd)
            .expect("Failed to build WSL command");

        let args: Vec<String> = wsl_cmd
            .args
            .iter()
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

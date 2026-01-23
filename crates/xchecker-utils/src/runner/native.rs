use crate::error::RunnerError;
use std::process::Stdio;
use std::time::Duration;

use super::{CommandSpec, ProcessOutput, ProcessRunner};

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
/// use xchecker_utils::runner::{NativeRunner, ProcessRunner, CommandSpec};
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
    /// use xchecker_utils::runner::NativeRunner;
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
        let child = command
            .spawn()
            .map_err(|e| RunnerError::NativeExecutionFailed {
                reason: format!(
                    "Failed to spawn process '{}': {}",
                    cmd.program.to_string_lossy(),
                    e
                ),
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

        #[cfg(not(any(unix, windows)))]
        {
            // No-op on other platforms
            let _ = pid;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let cmd = CommandSpec::new("echo").arg("hello world");

        let result = runner.run(&cmd, Duration::from_secs(10));

        assert!(result.is_ok(), "Echo command should succeed: {:?}", result);
        let output = result.unwrap();
        assert!(output.success(), "Echo should exit with code 0");
        assert!(
            output.stdout_string().contains("hello world"),
            "Output should contain 'hello world', got: {}",
            output.stdout_string()
        );
    }

    #[test]
    fn test_native_runner_shell_metacharacters_not_interpreted() {
        // Test that shell metacharacters are NOT interpreted
        // This is critical for security - verifies no shell injection
        let runner = NativeRunner::new();

        // Use echo with shell metacharacters that should be passed literally
        #[cfg(windows)]
        let cmd = CommandSpec::new("cmd").arg("/C").arg("echo").arg("$PATH");

        #[cfg(not(windows))]
        let cmd = CommandSpec::new("echo").arg("$PATH");

        let result = runner.run(&cmd, Duration::from_secs(10));

        assert!(result.is_ok(), "Command should succeed");
        let output = result.unwrap();
        // The literal string "$PATH" should appear in output, not the expanded PATH variable
        // Note: On Windows cmd /C echo $PATH will output "$PATH" literally
        // On Unix, echo "$PATH" will also output "$PATH" literally since we use argv
        assert!(
            output.stdout_string().contains("$PATH") || output.stdout_string().contains("PATH"),
            "Shell metacharacter should be preserved or echoed, got: {}",
            output.stdout_string()
        );
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
                assert!(
                    reason.contains("this_command_definitely_does_not_exist_12345"),
                    "Error should mention the command name: {}",
                    reason
                );
            }
            _ => panic!("Expected NativeExecutionFailed error"),
        }
    }

    #[test]
    fn test_native_runner_exit_code_propagation() {
        // Test that non-zero exit codes are properly propagated
        let runner = NativeRunner::new();

        #[cfg(windows)]
        let cmd = CommandSpec::new("cmd").arg("/C").arg("exit").arg("42");

        #[cfg(not(windows))]
        let cmd = CommandSpec::new("sh").arg("-c").arg("exit 42");

        let result = runner.run(&cmd, Duration::from_secs(10));

        assert!(
            result.is_ok(),
            "Command should complete (even with non-zero exit)"
        );
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
        assert!(
            output.stderr_string().contains("error message"),
            "Stderr should contain 'error message', got: {}",
            output.stderr_string()
        );
    }

    #[test]
    fn test_native_runner_implements_process_runner() {
        // Verify NativeRunner implements ProcessRunner trait
        fn assert_process_runner<T: ProcessRunner>(_: &T) {}

        let runner = NativeRunner::new();
        assert_process_runner(&runner);
    }
}

use crate::error::RunnerError;
use std::time::Duration;

use super::CommandSpec;

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
/// ```rust
/// use xchecker_utils::runner::{ProcessRunner, CommandSpec, ProcessOutput};
/// use xchecker_utils::error::RunnerError;
/// use std::time::Duration;
///
/// struct SimpleRunner;
///
/// impl ProcessRunner for SimpleRunner {
///     fn run(&self, cmd: &CommandSpec, _timeout: Duration) -> Result<ProcessOutput, RunnerError> {
///         // Use argv-style execution via CommandSpec::to_command()
///         let output = cmd.to_command()
///             .output()
///             .map_err(|e| RunnerError::NativeExecutionFailed {
///                 reason: e.to_string(),
///             })?;
///
///         Ok(ProcessOutput::new(
///             output.stdout,
///             output.stderr,
///             output.status.code(),
///             false,
///         ))
///     }
/// }
///
/// // Usage example
/// let runner = SimpleRunner;
/// let cmd = CommandSpec::new("echo").arg("hello");
/// let result = runner.run(&cmd, Duration::from_secs(30));
/// assert!(result.is_ok());
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

#[cfg(test)]
mod tests {
    use super::*;

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
        let output = ProcessOutput::new(b"hello world".to_vec(), Vec::new(), Some(0), false);
        assert_eq!(output.stdout_string(), "hello world");
    }

    #[test]
    fn test_process_output_stderr_string() {
        let output = ProcessOutput::new(Vec::new(), b"error message".to_vec(), Some(1), false);
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
        let output = ProcessOutput::new(b"stdout".to_vec(), b"stderr".to_vec(), Some(42), true);
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
        fn run(
            &self,
            _cmd: &CommandSpec,
            _timeout: Duration,
        ) -> Result<ProcessOutput, RunnerError> {
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
            fn run(
                &self,
                _cmd: &CommandSpec,
                _timeout: Duration,
            ) -> Result<ProcessOutput, RunnerError> {
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
            fn run(
                &self,
                _cmd: &CommandSpec,
                timeout: Duration,
            ) -> Result<ProcessOutput, RunnerError> {
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
}

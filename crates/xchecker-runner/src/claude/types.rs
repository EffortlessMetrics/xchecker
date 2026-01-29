use crate::ndjson::NdjsonResult;
use crate::types::RunnerMode;

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
    /// This should be called AFTER redaction to ensure the final size is <= 2048 bytes.
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

#[cfg(test)]
mod tests {
    use super::{BufferConfig, ClaudeResponse, WslOptions};
    use crate::ndjson::NdjsonResult;
    use crate::types::RunnerMode;

    #[test]
    fn test_wsl_options_default() {
        let options = WslOptions::default();
        assert!(options.distro.is_none());
        assert!(options.claude_path.is_none());
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
}

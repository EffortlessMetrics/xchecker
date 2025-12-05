//! Production LLM backend for Claude CLI.
//!
//! This is the production backend for Claude CLI integration. It wraps the existing Runner
//! infrastructure for process control, timeouts, and output buffering.
//!
//! **NOTE:** `src/claude.rs` is legacy/test-only and will be removed in a future release (V19+).
//! All new code should use this backend via the `LlmBackend` trait.

use crate::llm::{LlmBackend, LlmError, LlmInvocation, LlmResult, Message, Role};
use crate::runner::{BufferConfig, NdjsonResult, Runner, WslOptions};
use crate::types::RunnerMode;
use async_trait::async_trait;
use std::path::PathBuf;
use std::time::Duration;

/// Claude CLI backend implementation
pub(crate) struct ClaudeCliBackend {
    /// Path to the Claude CLI binary
    #[allow(dead_code)] // Will be used for validation and error reporting
    binary_path: PathBuf,
    /// Runner for executing Claude CLI
    runner: Runner,
}

impl ClaudeCliBackend {
    /// Create a new Claude CLI backend
    ///
    /// # Arguments
    /// * `binary_path` - Optional path to Claude CLI binary. If None, searches PATH.
    /// * `runner_mode` - Runner mode to use (Auto, Native, or Wsl)
    /// * `wsl_options` - WSL-specific options if using WSL mode
    ///
    /// # Errors
    /// Returns error if binary cannot be found or validated
    pub fn new(
        binary_path: Option<PathBuf>,
        runner_mode: RunnerMode,
        wsl_options: WslOptions,
    ) -> Result<Self, LlmError> {
        // Discover binary if not provided
        let binary = if let Some(path) = binary_path {
            path
        } else {
            Self::discover_binary()?
        };

        // Create runner with appropriate buffer config
        let buffer_config = BufferConfig::default();
        let runner = Runner::with_buffer_config(runner_mode, wsl_options, buffer_config);

        Ok(Self {
            binary_path: binary,
            runner,
        })
    }

    /// Create a new Claude CLI backend from configuration
    ///
    /// This is a convenience constructor that extracts the necessary configuration
    /// from the Config struct and constructs a ClaudeCliBackend.
    ///
    /// # Errors
    /// Returns error if binary cannot be found or configuration is invalid
    pub fn new_from_config(cfg: &crate::config::Config) -> Result<Self, LlmError> {
        // 1. Derive binary path from config or PATH
        let binary_path = if let Some(claude_config) = &cfg.llm.claude {
            claude_config.binary.as_ref().map(PathBuf::from)
        } else {
            None
        };

        // 2. Get runner mode from config
        let runner_mode = cfg.get_runner_mode().map_err(|e| {
            LlmError::Misconfiguration(format!("Invalid runner mode in config: {e}"))
        })?;

        // 3. Get WSL options from config
        let wsl_options = WslOptions {
            distro: cfg.runner.distro.clone(),
            claude_path: cfg.runner.claude_path.clone(),
        };

        // 4. Construct the backend
        Self::new(binary_path, runner_mode, wsl_options)
    }

    /// Discover Claude CLI binary in PATH
    fn discover_binary() -> Result<PathBuf, LlmError> {
        which::which("claude").map_err(|e| {
            LlmError::Misconfiguration(format!(
                "Claude CLI binary not found in PATH. Please install Claude CLI or specify the binary path with --llm-claude-binary. Error: {e}"
            ))
        })
    }

    /// Convert messages to Claude CLI command format
    fn messages_to_prompt(messages: &[Message]) -> String {
        let mut prompt = String::new();

        for msg in messages {
            match msg.role {
                Role::System => {
                    prompt.push_str("System: ");
                    prompt.push_str(&msg.content);
                    prompt.push_str("\n\n");
                }
                Role::User => {
                    prompt.push_str("User: ");
                    prompt.push_str(&msg.content);
                    prompt.push_str("\n\n");
                }
                Role::Assistant => {
                    prompt.push_str("Assistant: ");
                    prompt.push_str(&msg.content);
                    prompt.push_str("\n\n");
                }
            }
        }

        prompt
    }
}

#[async_trait]
impl LlmBackend for ClaudeCliBackend {
    async fn invoke(&self, inv: LlmInvocation) -> Result<LlmResult, LlmError> {
        // Convert messages to prompt format
        let prompt = Self::messages_to_prompt(&inv.messages);

        // Build Claude CLI arguments
        // Claude Code 2.0+ requires -p for non-interactive output
        let mut args = vec![
            "-p".to_string(),                     // Print mode (non-interactive)
            "--output-format".to_string(),        // Output format
            "json".to_string(),                   // JSON format for structured output
        ];

        // Add model if specified in metadata
        if let Some(model_value) = inv.metadata.get("model") {
            if let Some(model_str) = model_value.as_str() {
                args.push("--model".to_string());
                args.push(model_str.to_string());
            }
        } else if !inv.model.is_empty() {
            args.push("--model".to_string());
            args.push(inv.model.clone());
        }

        // Add max_tokens if specified
        if let Some(max_tokens_value) = inv.metadata.get("max_tokens")
            && let Some(max_tokens) = max_tokens_value.as_u64() {
                args.push("--max-tokens".to_string());
                args.push(max_tokens.to_string());
            }

        // Add temperature if specified
        if let Some(temp_value) = inv.metadata.get("temperature")
            && let Some(temp) = temp_value.as_f64() {
                args.push("--temperature".to_string());
                args.push(temp.to_string());
            }

        // Execute Claude CLI with timeout
        let response = self
            .runner
            .execute_claude(&args, &prompt, Some(inv.timeout))
            .await
            .map_err(|e| match e {
                crate::error::RunnerError::Timeout { timeout_seconds } => LlmError::Timeout {
                    duration: Duration::from_secs(timeout_seconds),
                },
                _ => LlmError::Transport(format!("Failed to execute Claude CLI: {e}")),
            })?;

        // Check for timeout
        if response.timed_out {
            return Err(LlmError::Timeout {
                duration: inv.timeout,
            });
        }

        // Parse NDJSON result
        let raw_json = match response.ndjson_result {
            NdjsonResult::ValidJson(json) => json,
            NdjsonResult::NoValidJson { tail_excerpt } => {
                return Err(LlmError::Transport(format!(
                    "No valid JSON found in Claude CLI output. Tail excerpt: {tail_excerpt}"
                )));
            }
        };

        // Claude Code 2.0+ returns JSON with structure: {"type": "result", "result": "...", ...}
        // Extract the actual result text from the wrapper
        let content = match serde_json::from_str::<serde_json::Value>(&raw_json) {
            Ok(json_value) => {
                // Try to extract the "result" field (Claude Code 2.0+ format)
                if let Some(result_text) = json_value.get("result").and_then(|v| v.as_str()) {
                    result_text.to_string()
                } else {
                    // Fallback: use the raw JSON if no "result" field
                    raw_json
                }
            }
            Err(_) => {
                // Not valid JSON, use raw output
                raw_json
            }
        };

        // Build LlmResult
        let mut result = LlmResult::new(content, "claude-cli", &inv.model);

        // Set timeout status
        result = result.with_timeout(false);

        // Add stderr to extensions if present
        if !response.stderr.is_empty() {
            result = result.with_extension("stderr", serde_json::Value::String(response.stderr));
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_messages_to_prompt() {
        let messages = vec![
            Message::system("You are a helpful assistant."),
            Message::user("Hello!"),
            Message::assistant("Hi there!"),
        ];

        let prompt = ClaudeCliBackend::messages_to_prompt(&messages);

        assert!(prompt.contains("System: You are a helpful assistant."));
        assert!(prompt.contains("User: Hello!"));
        assert!(prompt.contains("Assistant: Hi there!"));
    }
}

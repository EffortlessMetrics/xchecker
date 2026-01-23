//! Production LLM backend for Claude CLI.
//!
//! This is the production backend for Claude CLI integration. It wraps the existing Runner
//! infrastructure for process control, timeouts, and output buffering.
//!
//! **NOTE:** `src/claude.rs` is legacy/test-only and will be removed in a future release (V19+).
//! All new code should use this backend via the `LlmBackend` trait.

use crate::runner::{BufferConfig, Runner, WslOptions};
use crate::{LlmBackend, LlmError, LlmInvocation, LlmResult, Message, Role};
use async_trait::async_trait;
use std::path::PathBuf;
use std::time::Duration;
use xchecker_utils::types::{OutputFormat, RunnerMode};

/// Claude CLI backend implementation
pub(crate) struct ClaudeCliBackend {
    /// Path to the Claude CLI binary
    #[allow(dead_code)] // Will be used for validation and error reporting
    binary_path: PathBuf,
    /// Runner for executing Claude CLI
    runner: Runner,
    /// Output format for Claude CLI
    output_format: OutputFormat,
    /// Max turns for Claude CLI (legacy/test CLI)
    max_turns: Option<u32>,
    /// Cached Claude CLI version string
    claude_cli_version: String,
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
        output_format: OutputFormat,
        max_turns: Option<u32>,
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

        let claude_cli_version = runner
            .get_claude_version_sync()
            .unwrap_or_else(|_| "unknown".to_string());

        Ok(Self {
            binary_path: binary,
            runner,
            output_format,
            max_turns,
            claude_cli_version,
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
        let output_format = match cfg.defaults.output_format.as_deref() {
            Some("text") => OutputFormat::Text,
            _ => OutputFormat::StreamJson,
        };
        let max_turns = cfg.defaults.max_turns;

        // 1. Derive binary path from config or runner overrides
        let claude_binary = cfg
            .llm
            .claude
            .as_ref()
            .and_then(|claude_config| claude_config.binary.clone());
        let claude_path = claude_binary
            .clone()
            .or_else(|| cfg.runner.claude_path.clone());
        let binary_path = claude_path.as_ref().map(PathBuf::from);

        // 2. Get runner mode from config
        let runner_mode = cfg.get_runner_mode().map_err(|e| {
            LlmError::Misconfiguration(format!("Invalid runner mode in config: {e}"))
        })?;

        // 3. Get WSL options from config
        let wsl_options = WslOptions {
            distro: cfg.runner.distro.clone(),
            claude_path,
        };

        // 4. Construct the backend
        Self::new(
            binary_path,
            runner_mode,
            wsl_options,
            output_format,
            max_turns,
        )
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

    fn build_stub_args(
        &self,
        inv: &LlmInvocation,
        output_format: OutputFormat,
        scenario: Option<&str>,
    ) -> Vec<String> {
        let mut args = vec![
            "--output-format".to_string(),
            output_format.as_str().to_string(),
        ];

        if output_format == OutputFormat::StreamJson {
            args.push("--include-partial-messages".to_string());
        }

        let model = inv
            .metadata
            .get("model")
            .and_then(|v| v.as_str())
            .filter(|m| !m.is_empty())
            .unwrap_or(inv.model.as_str());
        if !model.is_empty() {
            args.push("--model".to_string());
            args.push(model.to_string());
        }

        if let Some(max_turns) = self.max_turns {
            args.push("--max-turns".to_string());
            args.push(max_turns.to_string());
        }

        if let Some(scenario) = scenario {
            args.push("--scenario".to_string());
            args.push(scenario.to_string());
        }

        if std::env::var("XCHECKER_STUB_NO_SLEEP").is_ok() {
            args.push("--no-sleep".to_string());
        }

        args
    }

    fn build_cli_args(&self, inv: &LlmInvocation, output_format: OutputFormat) -> Vec<String> {
        let mut args = vec![
            "--output-format".to_string(),
            output_format.as_str().to_string(),
        ];

        if output_format == OutputFormat::StreamJson {
            args.push("--include-partial-messages".to_string());
        }

        let model = inv
            .metadata
            .get("model")
            .and_then(|v| v.as_str())
            .filter(|m| !m.is_empty())
            .unwrap_or(inv.model.as_str());
        if !model.is_empty() {
            args.push("--model".to_string());
            args.push(model.to_string());
        }

        if let Some(max_turns) = self.max_turns {
            args.push("--max-turns".to_string());
            args.push(max_turns.to_string());
        }

        args.push("--no-interactive".to_string());

        if let Some(max_tokens_value) = inv.metadata.get("max_tokens")
            && let Some(max_tokens) = max_tokens_value.as_u64()
        {
            args.push("--max-tokens".to_string());
            args.push(max_tokens.to_string());
        }

        if let Some(temp_value) = inv.metadata.get("temperature")
            && let Some(temp) = temp_value.as_f64()
        {
            args.push("--temperature".to_string());
            args.push(temp.to_string());
        }

        args
    }

    fn parse_stream_json_output(output: &str) -> Result<(String, StreamMetadata), LlmError> {
        use serde_json::Value;

        let mut content = String::new();
        let mut metadata = StreamMetadata::default();

        for line in output.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let event: Value = serde_json::from_str(line).map_err(|e| {
                LlmError::Transport(format!("Failed to parse stream-json line: {e}"))
            })?;

            match event.get("type").and_then(|t| t.as_str()) {
                Some("content_block_delta") => {
                    if let Some(delta) = event
                        .get("delta")
                        .and_then(|d| d.get("text"))
                        .and_then(|t| t.as_str())
                    {
                        content.push_str(delta);
                    }
                }
                Some("message_stop") => {
                    if let Some(message) = event.get("message") {
                        if let Some(usage) = message.get("usage") {
                            metadata.input_tokens = usage
                                .get("input_tokens")
                                .and_then(serde_json::Value::as_u64);
                            metadata.output_tokens = usage
                                .get("output_tokens")
                                .and_then(serde_json::Value::as_u64);
                        }

                        metadata.model = message
                            .get("model")
                            .and_then(|m| m.as_str())
                            .map(ToString::to_string);

                        metadata.stop_reason = message
                            .get("stop_reason")
                            .and_then(|r| r.as_str())
                            .map(ToString::to_string);
                    }
                }
                _ => {}
            }
        }

        Ok((content, metadata))
    }

    fn stderr_tail(stderr: &str, max_bytes: usize) -> String {
        if stderr.len() <= max_bytes {
            stderr.to_string()
        } else {
            let bytes = stderr.as_bytes();
            let start = bytes.len().saturating_sub(max_bytes);
            String::from_utf8_lossy(&bytes[start..]).to_string()
        }
    }

    fn runner_label(mode: RunnerMode) -> &'static str {
        match mode {
            RunnerMode::Native => "native",
            RunnerMode::Wsl => "wsl",
            RunnerMode::Auto => "auto",
        }
    }
}

#[derive(Default)]
struct StreamMetadata {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    model: Option<String>,
    #[allow(dead_code)]
    stop_reason: Option<String>,
}

#[async_trait]
impl LlmBackend for ClaudeCliBackend {
    async fn invoke(&self, inv: LlmInvocation) -> Result<LlmResult, LlmError> {
        // Convert messages to prompt format
        let prompt = Self::messages_to_prompt(&inv.messages);

        let scenario = inv
            .metadata
            .get("claude_scenario")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let use_stub = scenario.is_some();

        let mut output_format = self.output_format;
        if scenario.as_deref() == Some("text") {
            output_format = OutputFormat::Text;
        }

        let args = if use_stub {
            self.build_stub_args(&inv, output_format, scenario.as_deref())
        } else {
            self.build_cli_args(&inv, output_format)
        };

        // Execute Claude CLI with timeout
        let mut response = self
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

        let mut fallback_used = false;
        let mut stream_metadata = StreamMetadata::default();

        let content = match output_format {
            OutputFormat::StreamJson => {
                if response.exit_code == 0 {
                    match Self::parse_stream_json_output(&response.stdout) {
                        Ok((parsed, metadata)) => {
                            stream_metadata = metadata;
                            parsed
                        }
                        Err(_) => {
                            fallback_used = true;
                            let fallback_args = if use_stub {
                                self.build_stub_args(&inv, OutputFormat::Text, scenario.as_deref())
                            } else {
                                self.build_cli_args(&inv, OutputFormat::Text)
                            };
                            response = self
                                .runner
                                .execute_claude(&fallback_args, &prompt, Some(inv.timeout))
                                .await
                                .map_err(|e| match e {
                                    crate::error::RunnerError::Timeout { timeout_seconds } => {
                                        LlmError::Timeout {
                                            duration: Duration::from_secs(timeout_seconds),
                                        }
                                    }
                                    _ => LlmError::Transport(format!(
                                        "Failed to execute Claude CLI fallback: {e}"
                                    )),
                                })?;

                            if response.timed_out {
                                return Err(LlmError::Timeout {
                                    duration: inv.timeout,
                                });
                            }

                            response.stdout.clone()
                        }
                    }
                } else {
                    response.stdout.clone()
                }
            }
            OutputFormat::Text => response.stdout.clone(),
        };

        let model_used = stream_metadata.model.clone().unwrap_or_else(|| {
            inv.metadata
                .get("model")
                .and_then(|v| v.as_str())
                .filter(|m| !m.is_empty())
                .unwrap_or(inv.model.as_str())
                .to_string()
        });

        let mut result = LlmResult::new(content, "claude-cli", model_used);

        if let (Some(input), Some(output)) =
            (stream_metadata.input_tokens, stream_metadata.output_tokens)
        {
            result = result.with_tokens(input, output);
        }

        // Set timeout status
        result = result.with_timeout(false);
        result = result.with_timeout_seconds(inv.timeout.as_secs());

        // Add stderr to extensions if present
        let stderr_tail = Self::stderr_tail(&response.stderr, 2048);
        if !stderr_tail.is_empty() {
            result = result.with_extension("stderr", serde_json::Value::String(stderr_tail));
        }

        result = result.with_extension(
            "exit_code",
            serde_json::Value::Number(serde_json::Number::from(response.exit_code as i64)),
        );
        result = result.with_extension(
            "runner_used",
            serde_json::Value::String(Self::runner_label(response.runner_used).to_string()),
        );
        if let Some(distro) = response.runner_distro.clone() {
            result = result.with_extension("runner_distro", serde_json::Value::String(distro));
        }
        result = result.with_extension(
            "claude_cli_version",
            serde_json::Value::String(self.claude_cli_version.clone()),
        );
        result = result.with_extension("fallback_used", serde_json::Value::Bool(fallback_used));

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

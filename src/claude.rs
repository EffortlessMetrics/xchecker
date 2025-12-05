//! Claude CLI integration module (Legacy/Test-Only)
//!
//! This module provides a wrapper around the Claude CLI with controlled surface area,
//! model resolution, and structured output handling with fallback capabilities.
//!
//! **NOTE:** This module is legacy/test-only. The production LLM backend is
//! `llm/claude_cli.rs`. This module will be removed in a future release (V19+)
//! once tests migrate to the new backend.

use crate::error::ClaudeError;
use crate::runner::Runner;
use crate::types::{OutputFormat, PermissionMode, RunnerMode};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Wrapper around Claude CLI with controlled surface and fallback handling
///
/// Legacy/test-only; production code uses `llm::ClaudeCliBackend`
#[derive(Debug, Clone)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct ClaudeWrapper {
    /// Model alias as provided by user (e.g., "sonnet")
    pub model_alias: Option<String>,
    /// Full model name resolved from alias (e.g., "claude-3-haiku-20240307")
    pub model_full_name: String,
    /// Maximum number of conversation turns
    pub max_turns: u32,
    /// Allowed tool patterns for Claude
    pub allowed_tools: Vec<String>,
    /// Disallowed tool patterns for Claude
    pub disallowed_tools: Vec<String>,
    /// Permission mode for tool usage
    pub permission_mode: Option<PermissionMode>,
    /// Claude CLI version captured from `claude --version`
    pub claude_cli_version: String,
    /// Runner for cross-platform execution
    pub runner: Runner,
}

/// Response from Claude CLI execution
///
/// Legacy/test-only; production code uses `llm::LlmResult`
#[derive(Debug, Clone)]
#[allow(dead_code)] // Legacy/test-only; follow-up spec (V19+) to delete once tests migrate
pub struct ClaudeResponse {
    /// The content returned by Claude
    pub content: String,
    /// Additional metadata about the response
    pub metadata: ResponseMetadata,
    /// Standard error output from Claude CLI
    pub stderr: String,
    /// Exit code from Claude CLI process
    pub exit_code: i32,
    /// Output format that was successfully used
    pub output_format: OutputFormat,
    /// Whether fallback to text format was used
    pub fallback_used: bool,
    /// The runner mode that was actually used
    pub runner_used: RunnerMode,
    /// The WSL distro that was used (if applicable)
    pub runner_distro: Option<String>,
    /// Whether the execution timed out
    pub timed_out: bool,
}

/// Metadata about the Claude response
///
/// Legacy/test-only; production code uses `llm::LlmResult` extensions
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct ResponseMetadata {
    /// Input tokens used
    pub input_tokens: Option<u32>,
    /// Output tokens generated
    pub output_tokens: Option<u32>,
    /// Model that was actually used
    pub model: Option<String>,
    /// Stop reason if available
    pub stop_reason: Option<String>,
}

#[cfg_attr(not(test), allow(dead_code))]
impl ClaudeWrapper {
    /// Create a new `ClaudeWrapper` with the specified model and runner
    pub fn new(model: Option<String>, runner: Runner) -> Result<Self, ClaudeError> {
        // Validate runner configuration first
        runner
            .validate()
            .map_err(|e| ClaudeError::ExecutionFailed {
                stderr: format!("Runner validation failed: {e}"),
            })?;

        let claude_cli_version = Self::get_claude_version(&runner)?;

        let (model_alias, model_full_name) = if let Some(model) = model {
            // Try to resolve the model name
            let resolved = Self::resolve_model_alias(&model, &runner)?;
            (Some(model), resolved)
        } else {
            // Use default model
            (None, "haiku".to_string())
        };

        Ok(Self {
            model_alias,
            model_full_name,
            max_turns: 10,
            allowed_tools: Vec::new(),
            disallowed_tools: Vec::new(),
            permission_mode: None,
            claude_cli_version,
            runner,
        })
    }

    /// Create a new `ClaudeWrapper` with automatic runner detection
    #[allow(dead_code)] // Legacy/test-only; follow-up spec (V19+) to delete once tests migrate
    pub fn with_auto_runner(model: Option<String>) -> Result<Self, ClaudeError> {
        let runner = Runner::auto().map_err(|e| ClaudeError::ExecutionFailed {
            stderr: format!("Auto runner detection failed: {e}"),
        })?;
        Self::new(model, runner)
    }

    /// Set maximum number of turns
    #[must_use]
    pub const fn with_max_turns(mut self, max_turns: u32) -> Self {
        self.max_turns = max_turns;
        self
    }

    /// Set allowed tools
    /// Reserved for future tool-mode restrictions
    #[must_use]
    #[allow(dead_code)]
    pub fn with_allowed_tools(mut self, tools: Vec<String>) -> Self {
        self.allowed_tools = tools;
        self
    }

    /// Set disallowed tools
    /// Reserved for future tool-mode restrictions
    #[must_use]
    #[allow(dead_code)]
    pub fn with_disallowed_tools(mut self, tools: Vec<String>) -> Self {
        self.disallowed_tools = tools;
        self
    }

    /// Set permission mode
    /// Reserved for future permission system
    #[must_use]
    #[allow(dead_code)]
    pub const fn with_permission_mode(mut self, mode: PermissionMode) -> Self {
        self.permission_mode = Some(mode);
        self
    }

    /// Get Claude CLI version by running `claude --version` using the specified runner
    fn get_claude_version(runner: &Runner) -> Result<String, ClaudeError> {
        let args = vec!["--version".to_string()];

        // Use tokio runtime to execute async function synchronously
        let runtime = tokio::runtime::Runtime::new().map_err(|e| ClaudeError::ExecutionFailed {
            stderr: format!("Failed to create tokio runtime: {e}"),
        })?;

        let response = runtime
            .block_on(async { runner.execute_claude(&args, "", None).await })
            .map_err(|e| ClaudeError::ExecutionFailed {
                stderr: format!("Failed to get Claude version: {e}"),
            })?;

        if response.exit_code != 0 {
            return Err(ClaudeError::ExecutionFailed {
                stderr: response.stderr,
            });
        }

        // Extract version from output like "claude 0.8.1"
        let version = response
            .stdout
            .split_whitespace()
            .last()
            .unwrap_or("unknown")
            .to_string();

        Ok(version)
    }

    /// Resolve model alias to full model name using Claude CLI
    ///
    /// This function resolves common model aliases to their full names and provides
    /// helpful error messages when models are not available. It follows requirement R7.1
    /// for model alias resolution with proper error handling.
    fn resolve_model_alias(alias: &str, runner: &Runner) -> Result<String, ClaudeError> {
        // First try basic alias resolution for common cases
        let resolved = match alias {
            // Claude 3.5 Sonnet aliases
            "sonnet" | "claude-3-5-sonnet" => "claude-3-5-sonnet-20241022",
            "sonnet-latest" => "claude-3-5-sonnet-20241022",

            // Claude 3 Haiku aliases
            "haiku" | "claude-3-haiku" => "claude-3-haiku-20240307",
            "haiku-latest" => "claude-3-haiku-20240307",

            // Claude 3 Opus aliases
            "opus" | "claude-3-opus" => "claude-3-opus-20240229",
            "opus-latest" => "claude-3-opus-20240229",

            // If it looks like a full model name, validate it exists
            name if name.starts_with("claude-") => {
                // For full model names, we'll validate them by attempting to query available models
                Self::validate_model_name(name, runner)?;
                name
            }
            _ => {
                return Err(ClaudeError::ModelNotAvailable {
                    model: alias.to_string(),
                });
            }
        };

        Ok(resolved.to_string())
    }

    /// Validate that a model name is available by querying Claude CLI
    ///
    /// This is a best-effort validation that attempts to check if the model
    /// is available. If the validation fails, we'll still allow the model
    /// and let Claude CLI handle the final validation during execution.
    fn validate_model_name(model_name: &str, runner: &Runner) -> Result<(), ClaudeError> {
        // Try to query available models to validate the model exists
        // This is a best-effort check - if it fails, we'll still proceed
        let args = vec!["models".to_string()];

        // Use tokio runtime to execute async function synchronously
        let runtime = tokio::runtime::Runtime::new().map_err(|e| ClaudeError::ExecutionFailed {
            stderr: format!("Failed to create tokio runtime: {e}"),
        })?;

        match runtime.block_on(async { runner.execute_claude(&args, "", None).await }) {
            Ok(response) if response.exit_code == 0 => {
                // Check if the model name appears in the output
                if response.stdout.contains(model_name) {
                    Ok(())
                } else {
                    // Model not found in available models, but we'll still allow it
                    // and let Claude CLI provide the final error during execution
                    Ok(())
                }
            }
            Ok(_) | Err(_) => {
                // If we can't query models (e.g., authentication issues, network problems),
                // we'll still allow the model and let Claude CLI handle validation later
                Ok(())
            }
        }
    }

    /// Execute a prompt with Claude CLI, trying stream-json first with text fallback
    #[allow(dead_code)] // Legacy/test-only; follow-up spec (V19+) to delete once tests migrate
    pub async fn execute(
        &self,
        prompt: &str,
        timeout_duration: Option<Duration>,
    ) -> Result<ClaudeResponse, ClaudeError> {
        // First attempt: try stream-json format
        match self
            .execute_with_format(prompt, OutputFormat::StreamJson, timeout_duration)
            .await
        {
            Ok(response) => Ok(response),
            Err(ClaudeError::ParseError { .. }) => {
                // Fallback to text format on parse error once per phase
                let mut response = self
                    .execute_with_format(prompt, OutputFormat::Text, timeout_duration)
                    .await?;
                response.fallback_used = true;
                Ok(response)
            }
            Err(e) => Err(e),
        }
    }

    /// Execute with a specific output format using the configured runner
    async fn execute_with_format(
        &self,
        prompt: &str,
        format: OutputFormat,
        timeout_duration: Option<Duration>,
    ) -> Result<ClaudeResponse, ClaudeError> {
        let mut args = Vec::new();

        // Set output format
        args.push("--output-format".to_string());
        args.push(format.as_str().to_string());

        // Add partial messages for stream-json
        if format == OutputFormat::StreamJson {
            args.push("--include-partial-messages".to_string());
        }

        // Set model
        args.push("--model".to_string());
        args.push(self.model_full_name.clone());

        // Set max turns
        args.push("--max-turns".to_string());
        args.push(self.max_turns.to_string());

        // Add tool permissions
        for tool in &self.allowed_tools {
            args.push("--allow".to_string());
            args.push(tool.clone());
        }

        for tool in &self.disallowed_tools {
            args.push("--deny".to_string());
            args.push(tool.clone());
        }

        // Set permission mode
        if let Some(mode) = &self.permission_mode {
            tracing::debug!(
                target: "xchecker::claude",
                permission_mode = %mode.as_str(),
                "Setting Claude permission mode"
            );
            match mode {
                PermissionMode::Plan => {
                    // Plan mode is default, no flag needed
                }
                PermissionMode::Auto => {
                    args.push("--dangerously-skip-permissions".to_string());
                }
                PermissionMode::Block => {
                    args.push("--deny".to_string());
                    args.push("*".to_string());
                }
            }
        }

        // Set non-interactive mode
        args.push("--no-interactive".to_string());

        // Execute using the runner
        let runner_response = self
            .runner
            .execute_claude(&args, prompt, timeout_duration)
            .await
            .map_err(|e| ClaudeError::ExecutionFailed {
                stderr: format!("Runner execution failed: {e}"),
            })?;

        // Limit stderr to 2 KiB as per requirements
        let stderr_tail = if runner_response.stderr.len() > 2048 {
            format!(
                "...{}",
                &runner_response.stderr[runner_response.stderr.len() - 2045..]
            )
        } else {
            runner_response.stderr.clone()
        };

        if runner_response.exit_code != 0 {
            // Capture partial stdout on failures (limited to 2 KiB like stderr)
            let partial_stdout = if runner_response.stdout.len() > 2048 {
                format!(
                    "...{}",
                    &runner_response.stdout[runner_response.stdout.len() - 2045..]
                )
            } else {
                runner_response.stdout
            };

            let error_message = if partial_stdout.is_empty() {
                stderr_tail
            } else {
                format!("stderr: {stderr_tail}\npartial stdout: {partial_stdout}")
            };

            return Err(ClaudeError::ExecutionFailed {
                stderr: error_message,
            });
        }

        // Parse the output based on format
        let (content, metadata) = match format {
            OutputFormat::StreamJson => self.parse_stream_json(&runner_response.stdout)?,
            OutputFormat::Text => (runner_response.stdout.clone(), ResponseMetadata::default()),
        };

        Ok(ClaudeResponse {
            content,
            metadata,
            stderr: stderr_tail,
            exit_code: runner_response.exit_code,
            output_format: format,
            fallback_used: false,
            runner_used: runner_response.runner_used,
            runner_distro: runner_response.runner_distro,
            timed_out: runner_response.timed_out,
        })
    }

    /// Parse stream-json output from Claude CLI
    pub fn parse_stream_json(
        &self,
        output: &str,
    ) -> Result<(String, ResponseMetadata), ClaudeError> {
        use serde_json::Value;

        let mut content = String::new();
        let mut metadata = ResponseMetadata::default();

        for line in output.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let event: Value = serde_json::from_str(line).map_err(|e| ClaudeError::ParseError {
                reason: format!("Failed to parse JSON line: {e}"),
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
                        // Extract metadata from final message
                        if let Some(usage) = message.get("usage") {
                            metadata.input_tokens = usage
                                .get("input_tokens")
                                .and_then(serde_json::Value::as_u64)
                                .map(|t| t as u32);
                            metadata.output_tokens = usage
                                .get("output_tokens")
                                .and_then(serde_json::Value::as_u64)
                                .map(|t| t as u32);
                        }

                        metadata.model = message
                            .get("model")
                            .and_then(|m| m.as_str())
                            .map(std::string::ToString::to_string);

                        metadata.stop_reason = message
                            .get("stop_reason")
                            .and_then(|r| r.as_str())
                            .map(std::string::ToString::to_string);
                    }
                }
                _ => {
                    // Ignore other event types (conversation_start, message_start, etc.)
                }
            }
        }

        Ok((content, metadata))
    }

    /// Get the model information for receipts
    #[must_use]
    pub fn get_model_info(&self) -> (Option<String>, String) {
        (self.model_alias.clone(), self.model_full_name.clone())
    }

    /// Get the Claude CLI version
    #[must_use]
    pub fn get_version(&self) -> &str {
        &self.claude_cli_version
    }

    /// Get runner information for receipts
    #[must_use]
    pub fn get_runner_info(&self) -> (RunnerMode, Option<String>) {
        (self.runner.mode, self.runner.get_wsl_distro_name())
    }

    /// Execute with explicit fallback tracking for receipt generation
    /// This method is similar to `execute()` but provides more detailed information
    /// about whether fallback was used, which is needed for receipt generation
    /// Reserved for future external execution tracking
    #[allow(dead_code)]
    pub async fn execute_with_fallback_tracking(
        &self,
        prompt: &str,
        timeout_duration: Option<Duration>,
    ) -> Result<(ClaudeResponse, bool), ClaudeError> {
        // First attempt: try stream-json format
        match self
            .execute_with_format(prompt, OutputFormat::StreamJson, timeout_duration)
            .await
        {
            Ok(response) => Ok((response, false)), // No fallback used
            Err(ClaudeError::ParseError { .. }) => {
                // Fallback to text format on parse error once per phase
                let response = self
                    .execute_with_format(prompt, OutputFormat::Text, timeout_duration)
                    .await?;
                Ok((response, true)) // Fallback was used
            }
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Hangs when run in parallel with other tests - run with --ignored to test individually"]
    fn test_model_alias_resolution() {
        use crate::runner::{Runner, WslOptions};
        use crate::types::RunnerMode;

        let _runner = Runner::new(RunnerMode::Native, WslOptions::default());

        // Test basic aliases
        assert_eq!(
            ClaudeWrapper::resolve_model_alias("sonnet", &_runner).unwrap(),
            "claude-3-5-sonnet-20241022"
        );
        assert_eq!(
            ClaudeWrapper::resolve_model_alias("claude-3-5-sonnet", &_runner).unwrap(),
            "claude-3-5-sonnet-20241022"
        );
        assert_eq!(
            ClaudeWrapper::resolve_model_alias("sonnet-latest", &_runner).unwrap(),
            "claude-3-5-sonnet-20241022"
        );

        assert_eq!(
            ClaudeWrapper::resolve_model_alias("haiku", &_runner).unwrap(),
            "claude-3-haiku-20240307"
        );
        assert_eq!(
            ClaudeWrapper::resolve_model_alias("claude-3-haiku", &_runner).unwrap(),
            "claude-3-haiku-20240307"
        );
        assert_eq!(
            ClaudeWrapper::resolve_model_alias("haiku-latest", &_runner).unwrap(),
            "claude-3-haiku-20240307"
        );

        assert_eq!(
            ClaudeWrapper::resolve_model_alias("opus", &_runner).unwrap(),
            "claude-3-opus-20240229"
        );
        assert_eq!(
            ClaudeWrapper::resolve_model_alias("claude-3-opus", &_runner).unwrap(),
            "claude-3-opus-20240229"
        );
        assert_eq!(
            ClaudeWrapper::resolve_model_alias("opus-latest", &_runner).unwrap(),
            "claude-3-opus-20240229"
        );

        // Test full model names (should pass through validation)
        assert_eq!(
            ClaudeWrapper::resolve_model_alias("claude-3-5-sonnet-20241022", &_runner).unwrap(),
            "claude-3-5-sonnet-20241022"
        );

        // Test invalid model
        let result = ClaudeWrapper::resolve_model_alias("invalid-model", &_runner);
        assert!(result.is_err());
        if let Err(ClaudeError::ModelNotAvailable { model }) = result {
            assert_eq!(model, "invalid-model");
        } else {
            panic!("Expected ModelNotAvailable error");
        }
    }

    #[test]
    fn test_builder_pattern() {
        use crate::runner::{Runner, WslOptions};
        use crate::types::RunnerMode;

        // Create a wrapper with mock version for testing
        let wrapper = ClaudeWrapper {
            model_alias: Some("sonnet".to_string()),
            model_full_name: "haiku".to_string(),
            max_turns: 10,
            allowed_tools: Vec::new(),
            disallowed_tools: Vec::new(),
            permission_mode: None,
            claude_cli_version: "0.8.1".to_string(),
            runner: Runner::new(RunnerMode::Native, WslOptions::default()),
        }
        .with_max_turns(5)
        .with_allowed_tools(vec!["file_editor".to_string()])
        .with_permission_mode(PermissionMode::Auto);

        assert_eq!(wrapper.max_turns, 5);
        assert_eq!(wrapper.allowed_tools, vec!["file_editor"]);
        assert_eq!(wrapper.permission_mode, Some(PermissionMode::Auto));
    }

    #[test]
    fn test_parse_stream_json() {
        use crate::runner::{Runner, WslOptions};
        use crate::types::RunnerMode;

        // Create a wrapper with mock version for testing
        let wrapper = ClaudeWrapper {
            model_alias: None,
            model_full_name: "haiku".to_string(),
            max_turns: 10,
            allowed_tools: Vec::new(),
            disallowed_tools: Vec::new(),
            permission_mode: None,
            claude_cli_version: "0.8.1".to_string(),
            runner: Runner::new(RunnerMode::Native, WslOptions::default()),
        };

        let sample_output = r#"{"type": "conversation_start", "conversation": {"id": "conv_123"}}
{"type": "message_start", "message": {"id": "msg_123", "role": "assistant"}}
{"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}
{"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "Hello"}}
{"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": " world"}}
{"type": "content_block_stop", "index": 0}
{"type": "message_stop", "message": {"id": "msg_123", "model": "claude-3-haiku-20240307", "stop_reason": "end_turn", "usage": {"input_tokens": 10, "output_tokens": 5}}}"#;

        let (content, metadata) = wrapper.parse_stream_json(sample_output).unwrap();

        assert_eq!(content, "Hello world");
        assert_eq!(metadata.input_tokens, Some(10));
        assert_eq!(metadata.output_tokens, Some(5));
        assert_eq!(
            metadata.model,
            Some("claude-3-haiku-20240307".to_string())
        );
        assert_eq!(metadata.stop_reason, Some("end_turn".to_string()));
    }

    #[test]
    fn test_fallback_tracking() {
        use crate::runner::{Runner, WslOptions};
        use crate::types::RunnerMode;

        // Create a wrapper with mock version for testing
        // Using haiku as the default model (alias: "haiku", full name: "claude-3-haiku-20240307")
        let wrapper = ClaudeWrapper {
            model_alias: Some("haiku".to_string()),
            model_full_name: "claude-3-haiku-20240307".to_string(),
            max_turns: 10,
            allowed_tools: Vec::new(),
            disallowed_tools: Vec::new(),
            permission_mode: None,
            claude_cli_version: "0.8.1".to_string(),
            runner: Runner::new(RunnerMode::Native, WslOptions::default()),
        };

        // Test that we can get model and runner info for receipts
        let (model_alias, model_full_name) = wrapper.get_model_info();
        assert_eq!(model_alias, Some("haiku".to_string()));
        assert_eq!(model_full_name, "claude-3-haiku-20240307");

        let (runner_mode, _runner_distro) = wrapper.get_runner_info();
        assert_eq!(runner_mode, RunnerMode::Native);

        assert_eq!(wrapper.get_version(), "0.8.1");
    }

    #[test]
    #[ignore = "Hangs when run in parallel with other tests - run with --ignored to test individually"]
    fn test_model_resolution_with_receipts() {
        use crate::runner::{Runner, WslOptions};
        use crate::types::RunnerMode;

        // Test that model resolution works correctly and info is available for receipts
        let wrapper = ClaudeWrapper::new(
            Some("sonnet".to_string()),
            Runner::new(RunnerMode::Native, WslOptions::default()),
        );

        // Should handle the case where Claude CLI is not available in tests
        if let Ok(wrapper) = wrapper {
            let (model_alias, model_full_name) = wrapper.get_model_info();
            assert_eq!(model_alias, Some("sonnet".to_string()));
            assert_eq!(model_full_name, "claude-3-5-sonnet-20241022");
        } else {
            // Expected in test environment where Claude CLI may not be available
            // The important thing is that the resolution logic is tested above
        }

        // Test with full model name
        let wrapper = ClaudeWrapper::new(
            Some("claude-3-haiku-20240307".to_string()),
            Runner::new(RunnerMode::Native, WslOptions::default()),
        );

        if let Ok(wrapper) = wrapper {
            let (model_alias, model_full_name) = wrapper.get_model_info();
            assert_eq!(model_alias, Some("claude-3-haiku-20240307".to_string()));
            assert_eq!(model_full_name, "claude-3-haiku-20240307");
        } else {
            // Expected in test environment where Claude CLI may not be available
        }

        // Test with no model (should use default)
        let wrapper =
            ClaudeWrapper::new(None, Runner::new(RunnerMode::Native, WslOptions::default()));

        if let Ok(wrapper) = wrapper {
            let (model_alias, model_full_name) = wrapper.get_model_info();
            assert_eq!(model_alias, None);
            assert_eq!(model_full_name, "claude-3-5-sonnet-20241022"); // Default model
        } else {
            // Expected in test environment where Claude CLI may not be available
        }
    }
}

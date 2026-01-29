//! Gemini CLI backend implementation
//!
//! Provides LLM backend implementation for Gemini CLI, wrapping the existing Runner
//! infrastructure for process control, timeouts, and output buffering.

use crate::runner::{BufferConfig, CommandSpec, Runner, WslOptions};
use crate::{LlmBackend, LlmError, LlmInvocation, LlmResult, Message, Role};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::time::timeout;
use xchecker_utils::types::RunnerMode;

/// Gemini profile configuration for per-phase model selection
#[derive(Debug, Clone)]
pub(crate) struct GeminiProfile {
    pub model: String,
    pub max_tokens: Option<u32>,
}

/// Gemini CLI backend implementation
pub(crate) struct GeminiCliBackend {
    /// Path to the Gemini CLI binary
    binary_path: PathBuf,
    /// Runner for executing Gemini CLI (reserved for future use with unified execution)
    #[allow(dead_code)]
    runner: Runner,
    /// Default model to use
    default_model: String,
    /// Named profiles for per-phase configuration
    profiles: HashMap<String, GeminiProfile>,
}

impl GeminiCliBackend {
    /// Create a new Gemini CLI backend
    ///
    /// # Arguments
    /// * `binary_path` - Optional path to Gemini CLI binary. If None, searches PATH.
    /// * `runner_mode` - Runner mode to use (Auto, Native, or Wsl)
    /// * `wsl_options` - WSL-specific options if using WSL mode
    /// * `default_model` - Default model to use if not specified in invocation
    /// * `profiles` - Named profiles for per-phase configuration
    ///
    /// # Errors
    /// Returns error if binary cannot be found or validated
    pub fn new(
        binary_path: Option<PathBuf>,
        runner_mode: RunnerMode,
        wsl_options: WslOptions,
        default_model: String,
        profiles: HashMap<String, GeminiProfile>,
    ) -> Result<Self, LlmError> {
        // Discover binary if not provided
        let binary = if let Some(path) = binary_path {
            path
        } else {
            Self::discover_binary()?
        };

        // Create runner with appropriate buffer config
        // Gemini stderr should be captured and redacted to ≤ 2 KiB
        let buffer_config = BufferConfig {
            stdout_cap_bytes: 2 * 1024 * 1024,  // 2 MiB for stdout
            stderr_cap_bytes: 2 * 1024,         // 2 KiB for stderr (requirement 3.4.3)
            stderr_receipt_cap_bytes: 2 * 1024, // 2 KiB for receipt (same as stderr cap)
        };
        let runner = Runner::with_buffer_config(runner_mode, wsl_options, buffer_config);

        Ok(Self {
            binary_path: binary,
            runner,
            default_model,
            profiles,
        })
    }

    /// Create a new Gemini CLI backend from configuration
    ///
    /// This is a convenience constructor that extracts the necessary configuration
    /// from the Config struct and constructs a GeminiCliBackend.
    ///
    /// # Errors
    /// Returns error if binary cannot be found or configuration is invalid
    pub fn new_from_config(cfg: &crate::config::Config) -> Result<Self, LlmError> {
        // 1. Derive binary path from config or PATH
        let binary_path = if let Some(gemini_config) = &cfg.llm.gemini {
            gemini_config.binary.as_ref().map(PathBuf::from)
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

        // 4. Get default model from config
        let default_model = if let Some(gemini_config) = &cfg.llm.gemini {
            gemini_config
                .default_model
                .clone()
                .unwrap_or_else(|| "gemini-2.0-flash-lite".to_string())
        } else {
            "gemini-2.0-flash-lite".to_string()
        };

        // 5. Load profiles from config
        let mut profiles = HashMap::new();
        if let Some(gemini_config) = &cfg.llm.gemini
            && let Some(config_profiles) = &gemini_config.profiles
        {
            for (name, profile_config) in config_profiles {
                profiles.insert(
                    name.clone(),
                    GeminiProfile {
                        model: profile_config
                            .model
                            .clone()
                            .unwrap_or_else(|| default_model.clone()),
                        max_tokens: profile_config.max_tokens,
                    },
                );
            }
        }

        // 6. Construct the backend
        Self::new(
            binary_path,
            runner_mode,
            wsl_options,
            default_model,
            profiles,
        )
    }

    /// Discover Gemini CLI binary in PATH
    fn discover_binary() -> Result<PathBuf, LlmError> {
        which::which("gemini").map_err(|e| {
            LlmError::Misconfiguration(format!(
                "Gemini CLI binary not found in PATH. Please install Gemini CLI or specify the binary path. Error: {e}"
            ))
        })
    }

    /// Convert messages to Gemini CLI prompt format
    ///
    /// Gemini CLI expects a single prompt string, so we concatenate all messages
    /// with role prefixes for context.
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

    /// Resolve model from invocation, profile, or default
    fn resolve_model(&self, inv: &LlmInvocation) -> String {
        // 1. Check if model is specified in invocation
        if !inv.model.is_empty() {
            return inv.model.clone();
        }

        // 2. Check if profile is specified in metadata
        if let Some(profile_name) = inv.metadata.get("profile")
            && let Some(profile_name_str) = profile_name.as_str()
            && let Some(profile) = self.profiles.get(profile_name_str)
        {
            return profile.model.clone();
        }

        // 3. Fall back to default model
        self.default_model.clone()
    }

    /// Resolve max_tokens from invocation, profile, or None
    fn resolve_max_tokens(&self, inv: &LlmInvocation) -> Option<u32> {
        // 1. Check if max_tokens is specified in invocation metadata
        if let Some(max_tokens_value) = inv.metadata.get("max_tokens")
            && let Some(max_tokens) = max_tokens_value.as_u64()
        {
            return Some(max_tokens as u32);
        }

        // 2. Check if profile is specified and has max_tokens
        if let Some(profile_name) = inv.metadata.get("profile")
            && let Some(profile_name_str) = profile_name.as_str()
            && let Some(profile) = self.profiles.get(profile_name_str)
            && let Some(max_tokens) = profile.max_tokens
        {
            return Some(max_tokens);
        }

        // 3. No max_tokens specified
        None
    }
}

#[async_trait]
impl LlmBackend for GeminiCliBackend {
    async fn invoke(&self, inv: LlmInvocation) -> Result<LlmResult, LlmError> {
        // Convert messages to prompt format
        let prompt = Self::messages_to_prompt(&inv.messages);

        // Resolve model and max_tokens
        let model = self.resolve_model(&inv);
        let max_tokens = self.resolve_max_tokens(&inv);

        // Build Gemini CLI command using CommandSpec for secure argv-style execution
        // Format: gemini -p "<prompt>" --model <model>
        // Note: Gemini takes prompt as command-line argument, not stdin
        let mut cmd_spec = CommandSpec::new(&self.binary_path)
            .arg("-p")
            .arg(&prompt)
            .arg("--model")
            .arg(&model);

        // Add max_tokens if specified
        if let Some(tokens) = max_tokens {
            cmd_spec = cmd_spec.arg("--max-tokens").arg(tokens.to_string());
        }

        // Convert to TokioCommand for async execution
        let mut cmd = cmd_spec.to_tokio_command();
        cmd.stdin(Stdio::null())
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

        // Execute with timeout
        let child = cmd
            .spawn()
            .map_err(|e| LlmError::Transport(format!("Failed to spawn Gemini CLI: {e}")))?;

        let output = if let Some(timeout_duration) = Some(inv.timeout) {
            match timeout(timeout_duration, child.wait_with_output()).await {
                Ok(Ok(output)) => output,
                Ok(Err(e)) => {
                    return Err(LlmError::Transport(format!(
                        "Failed to wait for Gemini CLI: {e}"
                    )));
                }
                Err(_) => {
                    return Err(LlmError::Timeout {
                        duration: inv.timeout,
                    });
                }
            }
        } else {
            child
                .wait_with_output()
                .await
                .map_err(|e| LlmError::Transport(format!("Failed to wait for Gemini CLI: {e}")))?
        };

        // Check exit status
        if !output.status.success() {
            return Err(LlmError::Transport(format!(
                "Gemini CLI exited with status: {}",
                output.status
            )));
        }

        // Treat stdout as opaque text (requirement 3.4.2)
        let raw_response = String::from_utf8_lossy(&output.stdout).to_string();

        // Capture stderr and apply 2 KiB limit (requirement 3.4.3)
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let stderr_redacted = if stderr.len() > 2048 {
            format!("{}... [truncated to 2 KiB]", &stderr[..2048])
        } else {
            stderr
        };

        // Build LlmResult
        let mut result = LlmResult::new(raw_response, "gemini-cli", &model);

        // Set timeout status
        result = result.with_timeout(false);
        result = result.with_timeout_seconds(inv.timeout.as_secs());

        // Add stderr to extensions if present (already redacted to ≤ 2 KiB)
        if !stderr_redacted.is_empty() {
            result = result.with_extension("stderr", serde_json::Value::String(stderr_redacted));
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_messages_to_prompt() {
        let messages = vec![
            Message::system("You are a helpful assistant."),
            Message::user("Hello!"),
            Message::assistant("Hi there!"),
        ];

        let prompt = GeminiCliBackend::messages_to_prompt(&messages);

        assert!(prompt.contains("System: You are a helpful assistant."));
        assert!(prompt.contains("User: Hello!"));
        assert!(prompt.contains("Assistant: Hi there!"));
    }

    #[test]
    fn test_resolve_model_from_invocation() {
        let backend = GeminiCliBackend {
            binary_path: PathBuf::from("gemini"),
            runner: Runner::new(RunnerMode::Native, WslOptions::default()),
            default_model: "gemini-2.0-flash-lite".to_string(),
            profiles: HashMap::new(),
        };

        let inv = LlmInvocation::new(
            "test-spec",
            "requirements",
            "gemini-2.0-pro",
            Duration::from_secs(300),
            vec![Message::user("Test")],
        );

        assert_eq!(backend.resolve_model(&inv), "gemini-2.0-pro");
    }

    #[test]
    fn test_resolve_model_from_profile() {
        let mut profiles = HashMap::new();
        profiles.insert(
            "design".to_string(),
            GeminiProfile {
                model: "gemini-2.0-pro".to_string(),
                max_tokens: Some(2048),
            },
        );

        let backend = GeminiCliBackend {
            binary_path: PathBuf::from("gemini"),
            runner: Runner::new(RunnerMode::Native, WslOptions::default()),
            default_model: "gemini-2.0-flash-lite".to_string(),
            profiles,
        };

        let inv = LlmInvocation::new(
            "test-spec",
            "design",
            "", // Empty model, should use profile
            Duration::from_secs(300),
            vec![Message::user("Test")],
        )
        .with_metadata("profile", serde_json::json!("design"));

        assert_eq!(backend.resolve_model(&inv), "gemini-2.0-pro");
    }

    #[test]
    fn test_resolve_model_default() {
        let backend = GeminiCliBackend {
            binary_path: PathBuf::from("gemini"),
            runner: Runner::new(RunnerMode::Native, WslOptions::default()),
            default_model: "gemini-2.0-flash-lite".to_string(),
            profiles: HashMap::new(),
        };

        let inv = LlmInvocation::new(
            "test-spec",
            "requirements",
            "", // Empty model, should use default
            Duration::from_secs(300),
            vec![Message::user("Test")],
        );

        assert_eq!(backend.resolve_model(&inv), "gemini-2.0-flash-lite");
    }

    #[test]
    fn test_resolve_max_tokens_from_invocation() {
        let backend = GeminiCliBackend {
            binary_path: PathBuf::from("gemini"),
            runner: Runner::new(RunnerMode::Native, WslOptions::default()),
            default_model: "gemini-2.0-flash-lite".to_string(),
            profiles: HashMap::new(),
        };

        let inv = LlmInvocation::new(
            "test-spec",
            "requirements",
            "gemini-2.0-pro",
            Duration::from_secs(300),
            vec![Message::user("Test")],
        )
        .with_metadata("max_tokens", serde_json::json!(1024));

        assert_eq!(backend.resolve_max_tokens(&inv), Some(1024));
    }

    #[test]
    fn test_resolve_max_tokens_from_profile() {
        let mut profiles = HashMap::new();
        profiles.insert(
            "design".to_string(),
            GeminiProfile {
                model: "gemini-2.0-pro".to_string(),
                max_tokens: Some(2048),
            },
        );

        let backend = GeminiCliBackend {
            binary_path: PathBuf::from("gemini"),
            runner: Runner::new(RunnerMode::Native, WslOptions::default()),
            default_model: "gemini-2.0-flash-lite".to_string(),
            profiles,
        };

        let inv = LlmInvocation::new(
            "test-spec",
            "design",
            "",
            Duration::from_secs(300),
            vec![Message::user("Test")],
        )
        .with_metadata("profile", serde_json::json!("design"));

        assert_eq!(backend.resolve_max_tokens(&inv), Some(2048));
    }

    #[test]
    fn test_resolve_max_tokens_none() {
        let backend = GeminiCliBackend {
            binary_path: PathBuf::from("gemini"),
            runner: Runner::new(RunnerMode::Native, WslOptions::default()),
            default_model: "gemini-2.0-flash-lite".to_string(),
            profiles: HashMap::new(),
        };

        let inv = LlmInvocation::new(
            "test-spec",
            "requirements",
            "",
            Duration::from_secs(300),
            vec![Message::user("Test")],
        );

        assert_eq!(backend.resolve_max_tokens(&inv), None);
    }
}

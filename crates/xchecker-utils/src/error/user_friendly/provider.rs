use super::UserFriendlyError;
use crate::error::{ClaudeError, ErrorCategory, LlmError, RunnerError};

impl UserFriendlyError for ClaudeError {
    fn user_message(&self) -> String {
        match self {
            Self::NotFound => "Claude CLI is not installed or not found in PATH".to_string(),
            Self::IncompatibleVersion { version } => {
                format!("Claude CLI version {version} is not compatible with xchecker")
            }
            Self::ExecutionFailed { stderr } => {
                format!("Claude CLI execution failed: {stderr}")
            }
            Self::ParseError { reason } => {
                format!("Could not understand Claude CLI response: {reason}")
            }
            Self::ModelNotAvailable { model } => {
                format!("The model '{model}' is not available or accessible")
            }
            Self::AuthenticationFailed { reason } => {
                format!("Claude authentication failed: {reason}")
            }
        }
    }

    fn context(&self) -> Option<String> {
        match self {
            Self::NotFound => Some(
                "xchecker requires the Claude CLI to be installed and available in your PATH."
                    .to_string(),
            ),
            Self::IncompatibleVersion { version: _ } => Some(
                "xchecker is tested with specific versions of the Claude CLI for compatibility."
                    .to_string(),
            ),
            Self::ExecutionFailed { stderr: _ } => {
                Some("The Claude CLI encountered an error during execution.".to_string())
            }
            Self::ParseError { reason: _ } => Some(
                "xchecker expects Claude CLI output in a specific format (stream-json or text)."
                    .to_string(),
            ),
            Self::ModelNotAvailable { model: _ } => Some(
                "Model availability depends on your Claude subscription and API access."
                    .to_string(),
            ),
            Self::AuthenticationFailed { reason: _ } => {
                Some("Claude CLI requires proper authentication to access the API.".to_string())
            }
        }
    }

    fn suggestions(&self) -> Vec<String> {
        match self {
            Self::NotFound => vec![
                "Install the Claude CLI: https://claude.ai/cli".to_string(),
                "Ensure 'claude' is in your PATH".to_string(),
                "Test with 'claude --version' to verify installation".to_string(),
            ],
            Self::IncompatibleVersion { version: _ } => vec![
                "Update Claude CLI to the latest version".to_string(),
                "Check xchecker documentation for supported Claude CLI versions".to_string(),
                "Use 'claude --version' to check your current version".to_string(),
            ],
            Self::ExecutionFailed { stderr: _ } => vec![
                "Check your internet connection".to_string(),
                "Verify Claude CLI authentication with 'claude auth status'".to_string(),
                "Try running the command manually to debug the issue".to_string(),
                "Check if you've exceeded API rate limits".to_string(),
            ],
            Self::ParseError { reason: _ } => vec![
                "This may be a temporary issue - try running the command again".to_string(),
                "Check if Claude CLI output format has changed".to_string(),
                "Report this issue if it persists".to_string(),
            ],
            Self::ModelNotAvailable { model } => {
                let mut suggestions = vec![
                    "Check available models with 'claude models'".to_string(),
                    "Verify your Claude subscription includes access to this model".to_string(),
                ];

                // Provide specific suggestions based on the model alias
                if model.contains("sonnet") || model == "sonnet" {
                    suggestions.push("Try '--model sonnet' for the Sonnet model".to_string());
                } else if model.contains("haiku") || model == "haiku" {
                    suggestions.push("Try '--model haiku' for the Haiku model".to_string());
                } else if model.contains("opus") || model == "opus" {
                    suggestions.push("Try '--model opus' for the Opus model".to_string());
                } else {
                    suggestions.push(
                        "Try using a common alias like 'sonnet', 'haiku', or 'opus'".to_string(),
                    );
                }

                suggestions.push(
                    "Check the Claude CLI documentation for supported model names".to_string(),
                );
                suggestions
            }
            Self::AuthenticationFailed { reason: _ } => vec![
                "Run 'claude auth login' to authenticate".to_string(),
                "Check your API key is valid and not expired".to_string(),
                "Verify your Claude account has API access".to_string(),
                "Try logging out and back in: 'claude auth logout && claude auth login'"
                    .to_string(),
            ],
        }
    }

    fn category(&self) -> ErrorCategory {
        ErrorCategory::ClaudeIntegration
    }
}

impl UserFriendlyError for RunnerError {
    fn user_message(&self) -> String {
        match self {
            Self::DetectionFailed { reason } => {
                format!("Could not detect the best way to run Claude CLI: {reason}")
            }
            Self::WslNotAvailable { reason } => {
                format!("WSL is not available: {reason}")
            }
            Self::WslExecutionFailed { reason } => {
                format!("Failed to run Claude CLI in WSL: {reason}")
            }
            Self::NativeExecutionFailed { reason } => {
                format!("Failed to run Claude CLI natively: {reason}")
            }
            Self::ConfigurationInvalid { reason } => {
                format!("Runner configuration is invalid: {reason}")
            }
            Self::ClaudeNotFoundInRunner { runner } => {
                format!("Claude CLI not found in {runner} environment")
            }
            Self::Timeout { timeout_seconds } => {
                format!("Claude CLI execution timed out after {timeout_seconds} seconds")
            }
        }
    }

    fn context(&self) -> Option<String> {
        match self {
            Self::DetectionFailed { .. } => {
                Some("xchecker automatically detects the best way to run Claude CLI on your system.".to_string())
            }
            Self::WslNotAvailable { .. } => {
                Some("WSL (Windows Subsystem for Linux) is required for running Claude CLI on Windows when not natively available.".to_string())
            }
            Self::WslExecutionFailed { .. } => {
                Some("WSL execution allows running Claude CLI in a Linux environment on Windows.".to_string())
            }
            Self::NativeExecutionFailed { .. } => {
                Some("Native execution runs Claude CLI directly on the host system.".to_string())
            }
            Self::ConfigurationInvalid { .. } => {
                Some("Runner configuration controls how xchecker executes Claude CLI across different platforms.".to_string())
            }
            Self::ClaudeNotFoundInRunner { .. } => {
                Some("Claude CLI must be installed and accessible in the specified runner environment.".to_string())
            }
            Self::Timeout { .. } => {
                Some("Phase execution has configurable timeouts to prevent hanging operations.".to_string())
            }
        }
    }

    fn suggestions(&self) -> Vec<String> {
        match self {
            Self::DetectionFailed { .. } => vec![
                "Try specifying runner mode explicitly: --runner native or --runner wsl"
                    .to_string(),
                "Ensure Claude CLI is installed and accessible".to_string(),
                "Check that 'claude --version' works in your environment".to_string(),
            ],
            Self::WslNotAvailable { .. } => vec![
                "Install WSL: wsl --install".to_string(),
                "Use native runner mode if Claude CLI is available on Windows".to_string(),
                "Check WSL status: wsl --status".to_string(),
            ],
            Self::WslExecutionFailed { .. } => vec![
                "Check that WSL is running: wsl --status".to_string(),
                "Verify Claude CLI is installed in WSL: wsl -e claude --version".to_string(),
                "Try restarting WSL: wsl --shutdown && wsl".to_string(),
                "Use native runner mode as alternative".to_string(),
            ],
            Self::NativeExecutionFailed { .. } => vec![
                "Install Claude CLI for your platform".to_string(),
                "Ensure 'claude' is in your PATH".to_string(),
                "Try WSL runner mode on Windows: --runner wsl".to_string(),
            ],
            Self::ConfigurationInvalid { .. } => vec![
                "Check runner configuration in .xchecker/config.toml".to_string(),
                "Valid runner modes: auto, native, wsl".to_string(),
                "Remove invalid configuration to use defaults".to_string(),
            ],
            Self::ClaudeNotFoundInRunner { runner } => match runner.as_str() {
                "wsl" => vec![
                    "Install Claude CLI in WSL: wsl -e pip install claude-cli".to_string(),
                    "Check WSL PATH: wsl -e echo $PATH".to_string(),
                    "Specify claude_path in configuration if installed in non-standard location"
                        .to_string(),
                ],
                "native" => vec![
                    "Install Claude CLI for your platform".to_string(),
                    "Add Claude CLI to your PATH".to_string(),
                    "Test with: claude --version".to_string(),
                ],
                _ => vec![
                    "Install Claude CLI in the specified runner environment".to_string(),
                    "Check that Claude CLI is accessible and executable".to_string(),
                ],
            },
            Self::Timeout { timeout_seconds: _ } => vec![
                "Increase timeout in configuration or via --phase-timeout flag".to_string(),
                "Check your internet connection if using Claude API".to_string(),
                "Try running with --verbose to see where it's hanging".to_string(),
                "Consider breaking down complex requests into smaller parts".to_string(),
            ],
        }
    }

    fn category(&self) -> ErrorCategory {
        ErrorCategory::ClaudeIntegration
    }
}

impl UserFriendlyError for LlmError {
    fn user_message(&self) -> String {
        match self {
            Self::Transport(msg) => format!("LLM transport error: {msg}"),
            Self::ProviderAuth(msg) => format!("LLM provider authentication failed: {msg}"),
            Self::ProviderQuota(msg) => format!("LLM provider quota exceeded: {msg}"),
            Self::ProviderOutage(msg) => format!("LLM provider service outage: {msg}"),
            Self::Timeout { duration } => {
                format!("LLM invocation timed out after {:?}", duration)
            }
            Self::BudgetExceeded { limit, attempted } => {
                format!(
                    "LLM budget exceeded: attempted {} calls, limit is {}",
                    attempted, limit
                )
            }
            Self::Misconfiguration(msg) => format!("LLM configuration error: {msg}"),
            Self::Unsupported(msg) => format!("LLM feature not supported: {msg}"),
        }
    }

    fn context(&self) -> Option<String> {
        match self {
            Self::Transport(_) => Some(
                "Transport errors occur when the LLM backend cannot be reached or spawned."
                    .to_string(),
            ),
            Self::ProviderAuth(_) => Some(
                "Authentication errors indicate missing or invalid API keys or credentials."
                    .to_string(),
            ),
            Self::ProviderQuota(_) => Some(
                "Quota errors occur when rate limits or usage limits are exceeded.".to_string(),
            ),
            Self::ProviderOutage(_) => {
                Some("Provider outages are temporary service disruptions.".to_string())
            }
            Self::Timeout { .. } => Some(
                "Timeouts occur when LLM invocations take longer than the configured limit."
                    .to_string(),
            ),
            Self::BudgetExceeded { .. } => {
                Some("Budget limits prevent excessive LLM API calls and costs.".to_string())
            }
            Self::Misconfiguration(_) => Some(
                "Configuration errors indicate missing or invalid LLM provider settings."
                    .to_string(),
            ),
            Self::Unsupported(_) => Some(
                "Some LLM features are not yet supported in this version of xchecker.".to_string(),
            ),
        }
    }

    fn suggestions(&self) -> Vec<String> {
        match self {
            Self::Transport(_) => vec![
                "Check that the LLM provider binary is installed and in PATH".to_string(),
                "Verify network connectivity for HTTP providers".to_string(),
                "Try running with --verbose to see detailed error information".to_string(),
            ],
            Self::ProviderAuth(_) => vec![
                "Check that the required API key environment variable is set".to_string(),
                "Verify the API key is valid and not expired".to_string(),
                "For CLI providers, ensure authentication is configured (e.g., 'claude auth login')".to_string(),
            ],
            Self::ProviderQuota(_) => vec![
                "Wait a few minutes and try again".to_string(),
                "Check your provider's rate limits and usage dashboard".to_string(),
                "Consider using a fallback provider if configured".to_string(),
            ],
            Self::ProviderOutage(_) => vec![
                "Wait a few minutes and try again".to_string(),
                "Check the provider's status page for known issues".to_string(),
                "Consider using a fallback provider if configured".to_string(),
            ],
            Self::Timeout { .. } => vec![
                "Increase the timeout in configuration or via CLI flags".to_string(),
                "Check your internet connection".to_string(),
                "Try breaking down complex requests into smaller parts".to_string(),
            ],
            Self::BudgetExceeded { .. } => vec![
                "Increase the budget limit via environment variable (e.g., XCHECKER_OPENROUTER_BUDGET)".to_string(),
                "Review which phases are consuming budget".to_string(),
                "Consider using a different provider with lower costs".to_string(),
            ],
            Self::Misconfiguration(_) => vec![
                "Check the LLM provider configuration in .xchecker/config.toml".to_string(),
                "Ensure required configuration keys are present".to_string(),
                "Review the documentation for provider-specific configuration".to_string(),
            ],
            Self::Unsupported(_) => vec![
                "Check the documentation for supported features in this version".to_string(),
                "Consider upgrading to a newer version of xchecker".to_string(),
                "Use an alternative approach if available".to_string(),
            ],
        }
    }

    fn category(&self) -> ErrorCategory {
        match self {
            Self::Transport(_) => ErrorCategory::ClaudeIntegration,
            Self::ProviderAuth(_) => ErrorCategory::Configuration,
            Self::ProviderQuota(_) => ErrorCategory::ResourceLimits,
            Self::ProviderOutage(_) => ErrorCategory::ClaudeIntegration,
            Self::Timeout { .. } => ErrorCategory::PhaseExecution,
            Self::BudgetExceeded { .. } => ErrorCategory::ResourceLimits,
            Self::Misconfiguration(_) => ErrorCategory::Configuration,
            Self::Unsupported(_) => ErrorCategory::Configuration,
        }
    }
}

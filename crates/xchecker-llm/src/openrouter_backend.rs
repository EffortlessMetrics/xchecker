//! OpenRouter HTTP backend implementation
//!
//! This module provides an HTTP-based LLM backend for OpenRouter, which offers
//! access to multiple models through a unified OpenAI-compatible API.

use crate::LlmError;
use crate::http_client::HttpClient;
use crate::types::{LlmBackend, LlmInvocation, LlmResult, Message, Role};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::debug;

/// Default OpenRouter API endpoint
const DEFAULT_BASE_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

/// Default HTTP referer header value
const DEFAULT_REFERER: &str = "https://effortlesssteven.com/xchecker";

/// Default X-Title header value
const DEFAULT_TITLE: &str = "xchecker";

/// OpenRouter backend configuration
#[derive(Clone)]
pub(crate) struct OpenRouterBackend {
    client: Arc<HttpClient>,
    base_url: String,
    api_key: String,
    default_model: String,
    default_params: HttpParams,
}

/// HTTP request parameters
#[derive(Debug, Clone)]
pub(crate) struct HttpParams {
    pub max_tokens: u32,
    pub temperature: f32,
}

impl Default for HttpParams {
    fn default() -> Self {
        Self {
            max_tokens: 2048,
            temperature: 0.2,
        }
    }
}

impl OpenRouterBackend {
    /// Create a new OpenRouter backend
    ///
    /// # Arguments
    ///
    /// * `api_key` - OpenRouter API key
    /// * `base_url` - Optional custom base URL (defaults to OpenRouter API)
    /// * `default_model` - Default model to use
    /// * `default_params` - Default HTTP parameters
    ///
    /// # Errors
    ///
    /// Returns `LlmError::Misconfiguration` if the HTTP client cannot be constructed
    pub fn new(
        api_key: String,
        base_url: Option<String>,
        default_model: String,
        default_params: HttpParams,
    ) -> Result<Self, LlmError> {
        let client = HttpClient::new()?;

        Ok(Self {
            client: Arc::new(client),
            base_url: base_url.unwrap_or_else(|| DEFAULT_BASE_URL.to_string()),
            api_key,
            default_model,
            default_params,
        })
    }

    /// Create a new OpenRouter backend from configuration
    ///
    /// # Errors
    ///
    /// Returns `LlmError::Misconfiguration` if:
    /// - The API key environment variable is not set
    /// - The HTTP client cannot be constructed
    pub fn new_from_config(config: &crate::config::Config) -> Result<Self, LlmError> {
        // Load API key from environment variable
        let api_key_env = config
            .llm
            .openrouter
            .as_ref()
            .and_then(|or| or.api_key_env.as_deref())
            .unwrap_or("OPENROUTER_API_KEY");

        let api_key = std::env::var(api_key_env).map_err(|_| {
            LlmError::Misconfiguration(format!(
                "OpenRouter API key not found in environment variable '{}'. \
                 Please set this variable or configure a different api_key_env in [llm.openrouter].",
                api_key_env
            ))
        })?;

        // Get base URL from config or use default
        let base_url = config
            .llm
            .openrouter
            .as_ref()
            .and_then(|or| or.base_url.clone());

        // Get default model from config
        let default_model = config
            .llm
            .openrouter
            .as_ref()
            .and_then(|or| or.model.clone())
            .ok_or_else(|| {
                LlmError::Misconfiguration(
                    "OpenRouter model not specified in configuration. \
                     Please set [llm.openrouter] model = \"model-name\"."
                        .to_string(),
                )
            })?;

        // Get default parameters from config
        let default_params = HttpParams {
            max_tokens: config
                .llm
                .openrouter
                .as_ref()
                .and_then(|or| or.max_tokens)
                .unwrap_or(2048),
            temperature: config
                .llm
                .openrouter
                .as_ref()
                .and_then(|or| or.temperature)
                .unwrap_or(0.2),
        };

        Self::new(api_key, base_url, default_model, default_params)
    }

    /// Resolve parameters for this invocation
    ///
    /// Parameters are resolved with the following precedence:
    /// 1. `inv.model` overrides `default_model`
    /// 2. `inv.metadata["max_tokens"]` overrides `default_params.max_tokens`
    /// 3. `inv.metadata["temperature"]` overrides `default_params.temperature`
    /// 4. Unspecified values fall back to backend defaults
    fn resolve_params(&self, inv: &LlmInvocation) -> (String, HttpParams) {
        let model = if inv.model.is_empty() {
            self.default_model.clone()
        } else {
            inv.model.clone()
        };

        let max_tokens = inv
            .metadata
            .get("max_tokens")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(self.default_params.max_tokens);

        let temperature = inv
            .metadata
            .get("temperature")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .unwrap_or(self.default_params.temperature);

        let params = HttpParams {
            max_tokens,
            temperature,
        };

        (model, params)
    }

    /// Convert messages to OpenAI-compatible format
    fn convert_messages(messages: &[Message]) -> Vec<OpenAiMessage> {
        messages
            .iter()
            .map(|msg| OpenAiMessage {
                role: match msg.role {
                    Role::System => "system".to_string(),
                    Role::User => "user".to_string(),
                    Role::Assistant => "assistant".to_string(),
                },
                content: msg.content.clone(),
            })
            .collect()
    }
}

#[async_trait]
impl LlmBackend for OpenRouterBackend {
    async fn invoke(&self, inv: LlmInvocation) -> Result<LlmResult, LlmError> {
        // Resolve parameters for this invocation
        let (model, params) = self.resolve_params(&inv);

        debug!(
            provider = "openrouter",
            model = %model,
            max_tokens = params.max_tokens,
            temperature = params.temperature,
            timeout_secs = inv.timeout.as_secs(),
            "Invoking OpenRouter backend"
        );

        // Convert messages to OpenAI-compatible format
        let openai_messages = Self::convert_messages(&inv.messages);

        // Build request body
        let request_body = OpenRouterRequest {
            model: model.clone(),
            messages: openai_messages,
            max_tokens: params.max_tokens,
            temperature: params.temperature,
            stream: false,
        };

        // Build HTTP request
        let request = reqwest::Client::new()
            .post(&self.base_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("HTTP-Referer", DEFAULT_REFERER)
            .header("X-Title", DEFAULT_TITLE)
            .header("Content-Type", "application/json")
            .json(&request_body);

        // Execute request with retry policy
        let response = self
            .client
            .execute_with_retry(request, inv.timeout, "openrouter")
            .await?;

        // Parse response
        let response_body: OpenRouterResponse = response.json().await.map_err(|e| {
            LlmError::Transport(format!("Failed to parse OpenRouter response: {}", e))
        })?;

        // Extract content from first choice
        let choice = response_body.choices.first().ok_or_else(|| {
            LlmError::Transport("OpenRouter response missing choices[0]".to_string())
        })?;

        // Validate that the response role is "assistant" (expected from LLM)
        debug!(
            provider = "openrouter",
            response_role = %choice.message.role,
            "Received response from OpenRouter"
        );

        let content = choice.message.content.clone().ok_or_else(|| {
            LlmError::Transport("OpenRouter response missing content in choices[0]".to_string())
        })?;

        // Build result
        let mut result = LlmResult::new(content, "openrouter", model);

        // Add token counts if available
        if let Some(usage) = response_body.usage {
            result.tokens_input = Some(usage.prompt_tokens);
            result.tokens_output = Some(usage.completion_tokens);
        }

        // Set timeout status (false since we got a response)
        result.timed_out = Some(false);
        result.timeout_seconds = Some(inv.timeout.as_secs());

        debug!(
            provider = "openrouter",
            tokens_input = ?result.tokens_input,
            tokens_output = ?result.tokens_output,
            "OpenRouter invocation completed"
        );

        Ok(result)
    }
}

/// OpenAI-compatible message format for requests
#[derive(Debug, Clone, Serialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

/// OpenAI-compatible message format for responses
#[derive(Debug, Clone, Deserialize)]
struct OpenAiResponseMessage {
    role: String,
    content: Option<String>,
}

/// OpenRouter request body (OpenAI-compatible)
#[derive(Debug, Clone, Serialize)]
struct OpenRouterRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    max_tokens: u32,
    temperature: f32,
    stream: bool,
}

/// OpenRouter response body (OpenAI-compatible)
#[derive(Debug, Clone, Deserialize)]
struct OpenRouterResponse {
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

/// Choice in OpenRouter response
#[derive(Debug, Clone, Deserialize)]
struct Choice {
    message: OpenAiResponseMessage,
}

/// Token usage information
#[derive(Debug, Clone, Deserialize)]
struct Usage {
    prompt_tokens: u64,
    completion_tokens: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Message;
    use std::time::Duration;

    #[test]
    fn test_resolve_params_uses_defaults() {
        let backend = OpenRouterBackend::new(
            "test-key".to_string(),
            None,
            "default-model".to_string(),
            HttpParams {
                max_tokens: 1024,
                temperature: 0.5,
            },
        )
        .unwrap();

        let inv = LlmInvocation::new(
            "test-spec",
            "test-phase",
            "", // Empty model should use default
            Duration::from_secs(60),
            vec![],
        );

        let (model, params) = backend.resolve_params(&inv);

        assert_eq!(model, "default-model");
        assert_eq!(params.max_tokens, 1024);
        assert_eq!(params.temperature, 0.5);
    }

    #[test]
    fn test_resolve_params_overrides_model() {
        let backend = OpenRouterBackend::new(
            "test-key".to_string(),
            None,
            "default-model".to_string(),
            HttpParams::default(),
        )
        .unwrap();

        let inv = LlmInvocation::new(
            "test-spec",
            "test-phase",
            "custom-model",
            Duration::from_secs(60),
            vec![],
        );

        let (model, _) = backend.resolve_params(&inv);

        assert_eq!(model, "custom-model");
    }

    #[test]
    fn test_resolve_params_overrides_max_tokens() {
        let backend = OpenRouterBackend::new(
            "test-key".to_string(),
            None,
            "default-model".to_string(),
            HttpParams {
                max_tokens: 1024,
                temperature: 0.5,
            },
        )
        .unwrap();

        let mut inv = LlmInvocation::new(
            "test-spec",
            "test-phase",
            "",
            Duration::from_secs(60),
            vec![],
        );
        inv.metadata
            .insert("max_tokens".to_string(), serde_json::json!(2048));

        let (_, params) = backend.resolve_params(&inv);

        assert_eq!(params.max_tokens, 2048);
        assert_eq!(params.temperature, 0.5); // Should keep default
    }

    #[test]
    fn test_resolve_params_overrides_temperature() {
        let backend = OpenRouterBackend::new(
            "test-key".to_string(),
            None,
            "default-model".to_string(),
            HttpParams {
                max_tokens: 1024,
                temperature: 0.5,
            },
        )
        .unwrap();

        let mut inv = LlmInvocation::new(
            "test-spec",
            "test-phase",
            "",
            Duration::from_secs(60),
            vec![],
        );
        inv.metadata
            .insert("temperature".to_string(), serde_json::json!(0.8));

        let (_, params) = backend.resolve_params(&inv);

        assert_eq!(params.max_tokens, 1024); // Should keep default
        assert_eq!(params.temperature, 0.8);
    }

    #[test]
    fn test_convert_messages() {
        let messages = vec![
            Message::new(Role::System, "You are a helpful assistant"),
            Message::new(Role::User, "Hello"),
            Message::new(Role::Assistant, "Hi there!"),
        ];

        let openai_messages = OpenRouterBackend::convert_messages(&messages);

        assert_eq!(openai_messages.len(), 3);
        assert_eq!(openai_messages[0].role, "system");
        assert_eq!(openai_messages[0].content, "You are a helpful assistant");
        assert_eq!(openai_messages[1].role, "user");
        assert_eq!(openai_messages[1].content, "Hello");
        assert_eq!(openai_messages[2].role, "assistant");
        assert_eq!(openai_messages[2].content, "Hi there!");
    }

    #[test]
    fn test_new_from_config_missing_api_key() {
        // Use a unique env var name to avoid conflicts with other tests
        let test_env_var = "OPENROUTER_API_KEY_TEST_MISSING";

        // Clear the environment variable if it exists
        unsafe {
            std::env::remove_var(test_env_var);
        }

        let mut config = crate::config::Config::minimal_for_testing();
        config.llm.openrouter = Some(crate::config::OpenRouterConfig {
            api_key_env: Some(test_env_var.to_string()),
            base_url: None,
            model: Some("test-model".to_string()),
            max_tokens: None,
            temperature: None,
            budget: None,
        });

        let result = OpenRouterBackend::new_from_config(&config);

        match result {
            Err(LlmError::Misconfiguration(msg)) => {
                assert!(
                    msg.contains(test_env_var),
                    "Expected error to mention env var, got: {}",
                    msg
                );
                assert!(
                    msg.contains("not found"),
                    "Expected error to mention 'not found', got: {}",
                    msg
                );
            }
            _ => panic!("Expected Misconfiguration error for missing API key"),
        }
    }

    #[test]
    fn test_new_from_config_missing_model() {
        // Use a unique env var name to avoid conflicts with other tests
        let test_env_var = "OPENROUTER_API_KEY_TEST_MODEL";

        // Set a dummy API key
        unsafe {
            std::env::set_var(test_env_var, "test-key");
        }

        let mut config = crate::config::Config::minimal_for_testing();
        config.llm.openrouter = Some(crate::config::OpenRouterConfig {
            api_key_env: Some(test_env_var.to_string()),
            base_url: None,
            model: None, // Missing model
            max_tokens: None,
            temperature: None,
            budget: None,
        });

        let result = OpenRouterBackend::new_from_config(&config);

        match result {
            Err(LlmError::Misconfiguration(msg)) => {
                assert!(
                    msg.contains("model") || msg.contains("Model"),
                    "Expected error to mention model, got: {}",
                    msg
                );
                assert!(
                    msg.contains("not specified") || msg.contains("specified"),
                    "Expected error to mention 'specified', got: {}",
                    msg
                );
            }
            _ => panic!("Expected Misconfiguration error for missing model"),
        }

        // Clean up
        unsafe {
            std::env::remove_var(test_env_var);
        }
    }
}

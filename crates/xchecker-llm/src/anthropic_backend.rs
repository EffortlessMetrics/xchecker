//! Anthropic HTTP backend implementation
//!
//! This module provides an HTTP-based LLM backend for Anthropic's Messages API,
//! which offers direct access to Claude models through their native API.

use crate::LlmError;
use crate::http_client::HttpClient;
use crate::types::{LlmBackend, LlmInvocation, LlmResult, Message, Role};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::debug;

/// Default Anthropic API endpoint
const DEFAULT_BASE_URL: &str = "https://api.anthropic.com/v1/messages";

/// Anthropic API version header value
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Anthropic backend configuration
#[derive(Clone)]
pub(crate) struct AnthropicBackend {
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

impl AnthropicBackend {
    /// Create a new Anthropic backend
    ///
    /// # Arguments
    ///
    /// * `api_key` - Anthropic API key
    /// * `base_url` - Optional custom base URL (defaults to Anthropic API)
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

    /// Create a new Anthropic backend from configuration
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
            .anthropic
            .as_ref()
            .and_then(|a| a.api_key_env.as_deref())
            .unwrap_or("ANTHROPIC_API_KEY");

        let api_key = std::env::var(api_key_env).map_err(|_| {
            LlmError::Misconfiguration(format!(
                "Anthropic API key not found in environment variable '{}'. \
                 Please set this variable or configure a different api_key_env in [llm.anthropic].",
                api_key_env
            ))
        })?;

        // Get base URL from config or use default
        let base_url = config
            .llm
            .anthropic
            .as_ref()
            .and_then(|a| a.base_url.clone());

        // Get default model from config
        let default_model = config
            .llm
            .anthropic
            .as_ref()
            .and_then(|a| a.model.clone())
            .ok_or_else(|| {
                LlmError::Misconfiguration(
                    "Anthropic model not specified in configuration. \
                     Please set [llm.anthropic] model = \"model-name\"."
                        .to_string(),
                )
            })?;

        // Get default parameters from config
        let default_params = HttpParams {
            max_tokens: config
                .llm
                .anthropic
                .as_ref()
                .and_then(|a| a.max_tokens)
                .unwrap_or(2048),
            temperature: config
                .llm
                .anthropic
                .as_ref()
                .and_then(|a| a.temperature)
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

    /// Convert messages to Anthropic Messages API format
    ///
    /// Anthropic's API uses a `system` field for system prompts and a `messages` array
    /// for user/assistant messages. This function separates system messages from the
    /// conversation messages.
    fn convert_messages(messages: &[Message]) -> (Option<String>, Vec<AnthropicMessage>) {
        let mut system_prompt: Option<String> = None;
        let mut anthropic_messages = Vec::new();

        for msg in messages {
            match msg.role {
                Role::System => {
                    // Concatenate multiple system messages if present
                    if let Some(existing) = system_prompt.as_mut() {
                        existing.push_str("\n\n");
                        existing.push_str(&msg.content);
                    } else {
                        system_prompt = Some(msg.content.clone());
                    }
                }
                Role::User => {
                    anthropic_messages.push(AnthropicMessage {
                        role: "user".to_string(),
                        content: msg.content.clone(),
                    });
                }
                Role::Assistant => {
                    anthropic_messages.push(AnthropicMessage {
                        role: "assistant".to_string(),
                        content: msg.content.clone(),
                    });
                }
            }
        }

        (system_prompt, anthropic_messages)
    }
}

#[async_trait]
impl LlmBackend for AnthropicBackend {
    async fn invoke(&self, inv: LlmInvocation) -> Result<LlmResult, LlmError> {
        // Resolve parameters for this invocation
        let (model, params) = self.resolve_params(&inv);

        debug!(
            provider = "anthropic",
            model = %model,
            max_tokens = params.max_tokens,
            temperature = params.temperature,
            timeout_secs = inv.timeout.as_secs(),
            "Invoking Anthropic backend"
        );

        // Convert messages to Anthropic format
        let (system_prompt, anthropic_messages) = Self::convert_messages(&inv.messages);

        // Build request body
        let request_body = AnthropicRequest {
            model: model.clone(),
            messages: anthropic_messages,
            max_tokens: params.max_tokens,
            temperature: params.temperature,
            system: system_prompt,
        };

        // Build HTTP request
        let request = reqwest::Client::new()
            .post(&self.base_url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&request_body);

        // Execute request with retry policy
        let response = self
            .client
            .execute_with_retry(request, inv.timeout, "anthropic")
            .await?;

        // Parse response
        let response_body: AnthropicResponse = response.json().await.map_err(|e| {
            LlmError::Transport(format!("Failed to parse Anthropic response: {}", e))
        })?;

        // Extract text content from content blocks
        let mut content_parts = Vec::new();
        for content_block in &response_body.content {
            if content_block.content_type == "text"
                && let Some(text) = &content_block.text
            {
                content_parts.push(text.clone());
            }
        }

        // Concatenate all text segments
        let content = content_parts.join("");

        if content.is_empty() {
            return Err(LlmError::Transport(
                "Anthropic response missing text content".to_string(),
            ));
        }

        // Build result
        let mut result = LlmResult::new(content, "anthropic", model);

        // Add token counts if available
        if let Some(usage) = response_body.usage {
            result.tokens_input = Some(usage.input_tokens);
            result.tokens_output = Some(usage.output_tokens);
        }

        // Set timeout status (false since we got a response)
        result.timed_out = Some(false);
        result.timeout_seconds = Some(inv.timeout.as_secs());

        debug!(
            provider = "anthropic",
            tokens_input = ?result.tokens_input,
            tokens_output = ?result.tokens_output,
            "Anthropic invocation completed"
        );

        Ok(result)
    }
}

/// Anthropic message format for requests
#[derive(Debug, Clone, Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

/// Anthropic request body
#[derive(Debug, Clone, Serialize)]
struct AnthropicRequest {
    model: String,
    messages: Vec<AnthropicMessage>,
    max_tokens: u32,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
}

/// Anthropic response body
#[derive(Debug, Clone, Deserialize)]
struct AnthropicResponse {
    content: Vec<ContentBlock>,
    usage: Option<Usage>,
}

/// Content block in Anthropic response
#[derive(Debug, Clone, Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    content_type: String,
    text: Option<String>,
}

/// Token usage information
#[derive(Debug, Clone, Deserialize)]
struct Usage {
    input_tokens: u64,
    output_tokens: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Message;
    use std::time::Duration;

    #[test]
    fn test_resolve_params_uses_defaults() {
        let backend = AnthropicBackend::new(
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
        let backend = AnthropicBackend::new(
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
        let backend = AnthropicBackend::new(
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
        let backend = AnthropicBackend::new(
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
    fn test_convert_messages_separates_system() {
        let messages = vec![
            Message::new(Role::System, "You are a helpful assistant"),
            Message::new(Role::User, "Hello"),
            Message::new(Role::Assistant, "Hi there!"),
        ];

        let (system, anthropic_messages) = AnthropicBackend::convert_messages(&messages);

        assert_eq!(system, Some("You are a helpful assistant".to_string()));
        assert_eq!(anthropic_messages.len(), 2);
        assert_eq!(anthropic_messages[0].role, "user");
        assert_eq!(anthropic_messages[0].content, "Hello");
        assert_eq!(anthropic_messages[1].role, "assistant");
        assert_eq!(anthropic_messages[1].content, "Hi there!");
    }

    #[test]
    fn test_convert_messages_concatenates_multiple_system() {
        let messages = vec![
            Message::new(Role::System, "First system message"),
            Message::new(Role::System, "Second system message"),
            Message::new(Role::User, "Hello"),
        ];

        let (system, anthropic_messages) = AnthropicBackend::convert_messages(&messages);

        assert_eq!(
            system,
            Some("First system message\n\nSecond system message".to_string())
        );
        assert_eq!(anthropic_messages.len(), 1);
        assert_eq!(anthropic_messages[0].role, "user");
    }

    #[test]
    fn test_convert_messages_no_system() {
        let messages = vec![
            Message::new(Role::User, "Hello"),
            Message::new(Role::Assistant, "Hi there!"),
        ];

        let (system, anthropic_messages) = AnthropicBackend::convert_messages(&messages);

        assert_eq!(system, None);
        assert_eq!(anthropic_messages.len(), 2);
    }

    #[test]
    fn test_new_from_config_missing_api_key() {
        // Use a unique env var name to avoid conflicts with other tests
        let test_env_var = "ANTHROPIC_API_KEY_TEST_MISSING";

        // Clear the environment variable if it exists
        unsafe {
            std::env::remove_var(test_env_var);
        }

        let mut config = crate::config::Config::minimal_for_testing();
        config.llm.anthropic = Some(crate::config::AnthropicConfig {
            api_key_env: Some(test_env_var.to_string()),
            base_url: None,
            model: Some("test-model".to_string()),
            max_tokens: None,
            temperature: None,
        });

        let result = AnthropicBackend::new_from_config(&config);

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
        let test_env_var = "ANTHROPIC_API_KEY_TEST_MODEL";

        // Set a dummy API key
        unsafe {
            std::env::set_var(test_env_var, "test-key");
        }

        let mut config = crate::config::Config::minimal_for_testing();
        config.llm.anthropic = Some(crate::config::AnthropicConfig {
            api_key_env: Some(test_env_var.to_string()),
            base_url: None,
            model: None, // Missing model
            max_tokens: None,
            temperature: None,
        });

        let result = AnthropicBackend::new_from_config(&config);

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

//! Core types for LLM backend abstraction

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

use crate::error::LlmError;
use xchecker_utils::types::LlmInfo;

/// Execution strategy determining how LLMs interact with the system
/// Reserved for future multi-strategy support (V15+); currently only Controlled is used
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(not(test), allow(dead_code))] // Reserved for future LLM execution strategy configuration
pub enum ExecutionStrategy {
    /// LLMs only propose changes; all writes go through xchecker's fixup pipeline
    Controlled,
    /// LLMs can directly write to disk or invoke tools (not supported in V11-V14)
    ExternalTool,
}

impl std::fmt::Display for ExecutionStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionStrategy::Controlled => write!(f, "controlled"),
            ExecutionStrategy::ExternalTool => write!(f, "externaltool"),
        }
    }
}

/// Role of a message in a conversation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// System-level instructions
    System,
    /// User input
    User,
    /// Assistant response
    Assistant,
}

/// A single message in a conversation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    /// Role of the message sender
    pub role: Role,
    /// Content of the message (plain UTF-8 text in V11-V14)
    pub content: String,
}

impl Message {
    /// Create a new message
    #[must_use]
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
        }
    }

    /// Create a system message
    #[must_use]
    #[allow(dead_code)] // Builder pattern method for LLM abstraction
    pub fn system(content: impl Into<String>) -> Self {
        Self::new(Role::System, content)
    }

    /// Create a user message
    #[must_use]
    pub fn user(content: impl Into<String>) -> Self {
        Self::new(Role::User, content)
    }

    /// Create an assistant message
    #[must_use]
    #[allow(dead_code)] // Builder pattern method for LLM abstraction
    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new(Role::Assistant, content)
    }
}

/// Input to an LLM backend invocation
#[derive(Debug, Clone)]
pub struct LlmInvocation {
    /// Spec ID for context
    #[allow(dead_code)] // Metadata field for LLM invocation tracking
    pub spec_id: String,
    /// Phase ID for context
    #[allow(dead_code)] // Metadata field for LLM invocation tracking
    pub phase_id: String,
    /// Model to use for this invocation
    pub model: String,
    /// Timeout for this invocation
    pub timeout: Duration,
    /// Ordered list of messages in the conversation
    pub messages: Vec<Message>,
    /// Provider-specific metadata (e.g., temperature, top_p, max_tokens)
    pub metadata: HashMap<String, serde_json::Value>,
}

impl LlmInvocation {
    /// Create a new LLM invocation
    #[must_use]
    pub fn new(
        spec_id: impl Into<String>,
        phase_id: impl Into<String>,
        model: impl Into<String>,
        timeout: Duration,
        messages: Vec<Message>,
    ) -> Self {
        Self {
            spec_id: spec_id.into(),
            phase_id: phase_id.into(),
            model: model.into(),
            timeout,
            messages,
            metadata: HashMap::new(),
        }
    }

    /// Add metadata to the invocation
    #[must_use]
    #[allow(dead_code)] // Builder pattern method for LLM abstraction
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

/// Result from an LLM backend invocation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResult {
    /// Raw response text from the LLM
    pub raw_response: String,
    /// Provider name (e.g., "claude-cli", "openrouter", "anthropic")
    pub provider: String,
    /// Model that was actually used
    pub model_used: String,
    /// Input tokens consumed (if available)
    pub tokens_input: Option<u64>,
    /// Output tokens generated (if available)
    pub tokens_output: Option<u64>,
    /// Whether the invocation timed out
    pub timed_out: Option<bool>,
    /// Timeout duration in seconds (if timeout occurred or was configured)
    pub timeout_seconds: Option<u64>,
    /// Provider-specific extensions
    pub extensions: HashMap<String, serde_json::Value>,
}

/// Fallback metadata when a secondary provider is used.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmFallbackInfo {
    /// Primary provider that failed to construct.
    pub primary_provider: String,
    /// Fallback provider that was constructed successfully.
    pub fallback_provider: String,
    /// Redacted reason for the primary provider failure.
    pub reason: String,
}

impl LlmFallbackInfo {
    /// Format a warning string suitable for receipts.
    #[must_use]
    pub fn warning_message(&self) -> String {
        format!(
            "llm_fallback: Primary provider '{}' failed: {}. Using fallback '{}'.",
            self.primary_provider, self.reason, self.fallback_provider
        )
    }
}

impl LlmResult {
    /// Create a new LLM result
    #[must_use]
    pub fn new(
        raw_response: impl Into<String>,
        provider: impl Into<String>,
        model_used: impl Into<String>,
    ) -> Self {
        Self {
            raw_response: raw_response.into(),
            provider: provider.into(),
            model_used: model_used.into(),
            tokens_input: None,
            tokens_output: None,
            timed_out: None,
            timeout_seconds: None,
            extensions: HashMap::new(),
        }
    }

    /// Set token counts
    #[must_use]
    #[allow(dead_code)] // Builder pattern method for LLM abstraction
    pub fn with_tokens(mut self, input: u64, output: u64) -> Self {
        self.tokens_input = Some(input);
        self.tokens_output = Some(output);
        self
    }

    /// Set timeout status
    #[must_use]
    pub fn with_timeout(mut self, timed_out: bool) -> Self {
        self.timed_out = Some(timed_out);
        self
    }

    /// Set timeout duration in seconds
    #[must_use]
    pub fn with_timeout_seconds(mut self, seconds: u64) -> Self {
        self.timeout_seconds = Some(seconds);
        self
    }

    /// Add an extension field
    #[must_use]
    pub fn with_extension(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.extensions.insert(key.into(), value);
        self
    }

    /// Convert LlmResult to LlmInfo for receipt generation (V11+ multi-provider support)
    #[must_use]
    pub fn into_llm_info(self) -> LlmInfo {
        // Check if budget_exhausted is in extensions
        let budget_exhausted = self
            .extensions
            .get("budget_exhausted")
            .and_then(|v| v.as_bool());

        LlmInfo {
            provider: Some(self.provider),
            model_used: Some(self.model_used),
            tokens_input: self.tokens_input,
            tokens_output: self.tokens_output,
            timed_out: self.timed_out,
            timeout_seconds: self.timeout_seconds,
            budget_exhausted,
        }
    }
}

/// Trait for LLM backend implementations
///
/// All providers (CLI and HTTP) implement this trait, allowing the orchestrator
/// to work with any provider without knowing implementation details.
#[async_trait]
pub trait LlmBackend: Send + Sync {
    /// Invoke the LLM with the given invocation parameters
    ///
    /// # Errors
    ///
    /// Returns `LlmError` for any failure during invocation, including:
    /// - Transport failures (process spawn, network errors)
    /// - Provider errors (auth, quota, outages)
    /// - Timeouts
    /// - Budget exhaustion
    async fn invoke(&self, inv: LlmInvocation) -> Result<LlmResult, LlmError>;
}

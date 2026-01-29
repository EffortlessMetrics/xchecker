//! Prompt template types for xchecker LLM configuration
//!
//! This module provides `PromptTemplate` enum and related functionality
//! for configuring how LLM prompts are structured across different providers.
//!

/// Prompt template types for provider-specific optimizations
///
/// Templates define how prompts are structured for different LLM providers.
/// Some templates are optimized for specific providers and may not work
/// correctly with others.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptTemplate {
    /// Universal template compatible with all providers
    ///
    /// Uses a simple message format that works across all backends
    Default,
    /// Optimized for Claude CLI and Anthropic API
    ///
    /// Uses Claude-specific formatting like XML tags and system prompts
    ClaudeOptimized,
    /// Optimized for OpenRouter and OpenAI-compatible APIs
    ///
    /// Uses OpenAI-style message formatting
    OpenAiCompatible,
}

impl PromptTemplate {
    /// Parse a template name string into a PromptTemplate
    ///
    /// # Errors
    ///
    /// Returns an error if template name is not recognized.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "default" => Ok(Self::Default),
            "claude-optimized" | "claude_optimized" => Ok(Self::ClaudeOptimized),
            "claude" => Ok(Self::ClaudeOptimized),
            "openai-compatible" | "openai_compatible" | "openai" | "openrouter" => {
                Ok(Self::OpenAiCompatible)
            }
            _ => Err(format!(
                "Unknown prompt template '{}'. Available templates: default, claude-optimized, openai-compatible",
                s
            )),
        }
    }

    /// Check if this template is compatible with given provider
    ///
    /// Returns `Ok(())` if compatible, or an error message explaining incompatibility.
    pub fn validate_provider_compatibility(&self, provider: &str) -> Result<(), String> {
        match (self, provider) {
            // Default template is compatible with all providers
            (Self::Default, _) => Ok(()),

            // Claude-optimized template is compatible with Claude CLI and Anthropic
            (Self::ClaudeOptimized, "claude-cli" | "anthropic") => Ok(()),
            (Self::ClaudeOptimized, _) => Err(format!(
                "Prompt template 'claude-optimized' is not compatible with provider '{}'. \
                 This template uses Claude-specific formatting (XML tags, system prompts) \
                 that may not work correctly with other providers. \
                 Compatible providers: claude-cli, anthropic. \
                 Use 'default' template for cross-provider compatibility.",
                provider
            )),

            // OpenAI-compatible template is compatible with OpenRouter and Gemini
            (Self::OpenAiCompatible, "openrouter" | "gemini-cli") => Ok(()),
            (Self::OpenAiCompatible, _) => Err(format!(
                "Prompt template 'openai-compatible' is not compatible with provider '{}'. \
                 This template uses OpenAI-style message formatting that may not work \
                 correctly with Claude-specific providers. \
                 Compatible providers: openrouter, gemini-cli. \
                 Use 'default' template for cross-provider compatibility.",
                provider
            )),
        }
    }

    /// Get template name as a string
    #[must_use]
    #[allow(dead_code)] // Public API for template introspection
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::ClaudeOptimized => "claude-optimized",
            Self::OpenAiCompatible => "openai-compatible",
        }
    }

    /// Get a list of providers compatible with this template
    #[must_use]
    #[allow(dead_code)] // Public API for template introspection
    pub const fn compatible_providers(&self) -> &'static [&'static str] {
        match self {
            Self::Default => &["claude-cli", "gemini-cli", "openrouter", "anthropic"],
            Self::ClaudeOptimized => &["claude-cli", "anthropic"],
            Self::OpenAiCompatible => &["openrouter", "gemini-cli"],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_templates() {
        // Test valid template names
        assert_eq!(
            PromptTemplate::parse("default").unwrap(),
            PromptTemplate::Default
        );
        assert_eq!(
            PromptTemplate::parse("claude-optimized").unwrap(),
            PromptTemplate::ClaudeOptimized
        );
        assert_eq!(
            PromptTemplate::parse("claude").unwrap(),
            PromptTemplate::ClaudeOptimized
        );
        assert_eq!(
            PromptTemplate::parse("openai-compatible").unwrap(),
            PromptTemplate::OpenAiCompatible
        );
        assert_eq!(
            PromptTemplate::parse("openai").unwrap(),
            PromptTemplate::OpenAiCompatible
        );
        assert_eq!(
            PromptTemplate::parse("openrouter").unwrap(),
            PromptTemplate::OpenAiCompatible
        );
        assert_eq!(
            PromptTemplate::parse("DEFAULT").unwrap(),
            PromptTemplate::Default
        );
        assert_eq!(
            PromptTemplate::parse("Claude-Optimized").unwrap(),
            PromptTemplate::ClaudeOptimized
        );
    }

    #[test]
    fn test_parse_invalid_templates() {
        // Test invalid template names
        assert!(PromptTemplate::parse("invalid").is_err());
        assert!(PromptTemplate::parse("unknown-template").is_err());
    }

    #[test]
    fn test_prompt_template_provider_compatibility() {
        // Default template is compatible with all providers
        assert!(
            PromptTemplate::Default
                .validate_provider_compatibility("claude-cli")
                .is_ok()
        );
        assert!(
            PromptTemplate::Default
                .validate_provider_compatibility("gemini-cli")
                .is_ok()
        );
        assert!(
            PromptTemplate::Default
                .validate_provider_compatibility("openrouter")
                .is_ok()
        );
        assert!(
            PromptTemplate::Default
                .validate_provider_compatibility("anthropic")
                .is_ok()
        );
    }

    #[test]
    fn test_prompt_template_as_str() {
        assert_eq!(PromptTemplate::Default.as_str(), "default");
        assert_eq!(PromptTemplate::ClaudeOptimized.as_str(), "claude-optimized");
        assert_eq!(
            PromptTemplate::OpenAiCompatible.as_str(),
            "openai-compatible"
        );
    }

    #[test]
    fn test_prompt_template_compatible_providers() {
        assert_eq!(
            PromptTemplate::Default.compatible_providers(),
            &["claude-cli", "gemini-cli", "openrouter", "anthropic"]
        );
        assert_eq!(
            PromptTemplate::ClaudeOptimized.compatible_providers(),
            &["claude-cli", "anthropic"]
        );
        assert_eq!(
            PromptTemplate::OpenAiCompatible.compatible_providers(),
            &["openrouter", "gemini-cli"]
        );
    }
}

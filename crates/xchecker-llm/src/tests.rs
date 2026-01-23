//! Unit tests for LLM core types

#[cfg(test)]
mod llm_tests {
    use super::super::*;
    use std::time::Duration;

    #[test]
    fn test_execution_strategy_serialization() {
        // Test Controlled serialization
        let controlled = ExecutionStrategy::Controlled;
        let json = serde_json::to_string(&controlled).unwrap();
        assert_eq!(json, r#""controlled""#);

        // Test ExternalTool serialization
        let external = ExecutionStrategy::ExternalTool;
        let json = serde_json::to_string(&external).unwrap();
        assert_eq!(json, r#""externaltool""#);

        // Test deserialization
        let controlled: ExecutionStrategy = serde_json::from_str(r#""controlled""#).unwrap();
        assert_eq!(controlled, ExecutionStrategy::Controlled);

        let external: ExecutionStrategy = serde_json::from_str(r#""externaltool""#).unwrap();
        assert_eq!(external, ExecutionStrategy::ExternalTool);
    }

    #[test]
    fn test_execution_strategy_display() {
        assert_eq!(ExecutionStrategy::Controlled.to_string(), "controlled");
        assert_eq!(ExecutionStrategy::ExternalTool.to_string(), "externaltool");
    }

    #[test]
    fn test_role_serialization() {
        let system = Role::System;
        let json = serde_json::to_string(&system).unwrap();
        assert_eq!(json, r#""system""#);

        let user = Role::User;
        let json = serde_json::to_string(&user).unwrap();
        assert_eq!(json, r#""user""#);

        let assistant = Role::Assistant;
        let json = serde_json::to_string(&assistant).unwrap();
        assert_eq!(json, r#""assistant""#);
    }

    #[test]
    fn test_message_construction() {
        let msg = Message::new(Role::User, "Hello");
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content, "Hello");

        let system_msg = Message::system("System prompt");
        assert_eq!(system_msg.role, Role::System);
        assert_eq!(system_msg.content, "System prompt");

        let user_msg = Message::user("User input");
        assert_eq!(user_msg.role, Role::User);
        assert_eq!(user_msg.content, "User input");

        let assistant_msg = Message::assistant("Assistant response");
        assert_eq!(assistant_msg.role, Role::Assistant);
        assert_eq!(assistant_msg.content, "Assistant response");
    }

    #[test]
    fn test_message_serialization() {
        let msg = Message::user("Test message");
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.role, Role::User);
        assert_eq!(deserialized.content, "Test message");
    }

    #[test]
    fn test_llm_invocation_construction() {
        let messages = vec![
            Message::system("You are a helpful assistant"),
            Message::user("Hello"),
        ];

        let inv = LlmInvocation::new(
            "test-spec",
            "requirements",
            "sonnet",
            Duration::from_secs(300),
            messages.clone(),
        );

        assert_eq!(inv.spec_id, "test-spec");
        assert_eq!(inv.phase_id, "requirements");
        assert_eq!(inv.model, "sonnet");
        assert_eq!(inv.timeout, Duration::from_secs(300));
        assert_eq!(inv.messages.len(), 2);
        assert!(inv.metadata.is_empty());
    }

    #[test]
    fn test_llm_invocation_with_metadata() {
        let inv = LlmInvocation::new(
            "test-spec",
            "design",
            "gpt-4",
            Duration::from_secs(600),
            vec![Message::user("Test")],
        )
        .with_metadata("temperature", serde_json::json!(0.7))
        .with_metadata("max_tokens", serde_json::json!(2048));

        assert_eq!(inv.metadata.len(), 2);
        assert_eq!(
            inv.metadata.get("temperature").unwrap(),
            &serde_json::json!(0.7)
        );
        assert_eq!(
            inv.metadata.get("max_tokens").unwrap(),
            &serde_json::json!(2048)
        );
    }

    #[test]
    fn test_llm_result_construction() {
        let result = LlmResult::new("This is the response", "claude-cli", "haiku");

        assert_eq!(result.raw_response, "This is the response");
        assert_eq!(result.provider, "claude-cli");
        assert_eq!(result.model_used, "haiku");
        assert!(result.tokens_input.is_none());
        assert!(result.tokens_output.is_none());
        assert!(result.timed_out.is_none());
        assert!(result.extensions.is_empty());
    }

    #[test]
    fn test_llm_result_with_tokens() {
        let result = LlmResult::new("Response", "openrouter", "gpt-4").with_tokens(1024, 512);

        assert_eq!(result.tokens_input, Some(1024));
        assert_eq!(result.tokens_output, Some(512));
    }

    #[test]
    fn test_llm_result_with_timeout() {
        let result = LlmResult::new("Partial response", "anthropic", "opus").with_timeout(true);

        assert_eq!(result.timed_out, Some(true));
    }

    #[test]
    fn test_llm_result_with_timeout_seconds() {
        let result = LlmResult::new("Partial response", "anthropic", "opus")
            .with_timeout(true)
            .with_timeout_seconds(300);

        assert_eq!(result.timed_out, Some(true));
        assert_eq!(result.timeout_seconds, Some(300));
    }

    #[test]
    fn test_llm_result_with_extension() {
        let result = LlmResult::new("Response", "provider", "model")
            .with_extension("custom_field", serde_json::json!({"key": "value"}));

        assert_eq!(result.extensions.len(), 1);
        assert_eq!(
            result.extensions.get("custom_field").unwrap(),
            &serde_json::json!({"key": "value"})
        );
    }

    #[test]
    fn test_llm_result_serialization() {
        let result = LlmResult::new("Test response", "test-provider", "test-model")
            .with_tokens(100, 50)
            .with_timeout(false)
            .with_timeout_seconds(600);

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: LlmResult = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.raw_response, "Test response");
        assert_eq!(deserialized.provider, "test-provider");
        assert_eq!(deserialized.model_used, "test-model");
        assert_eq!(deserialized.tokens_input, Some(100));
        assert_eq!(deserialized.tokens_output, Some(50));
        assert_eq!(deserialized.timed_out, Some(false));
        assert_eq!(deserialized.timeout_seconds, Some(600));
    }

    #[test]
    fn test_llm_error_display() {
        let err = LlmError::Transport("Connection failed".to_string());
        assert_eq!(err.to_string(), "Transport error: Connection failed");

        let err = LlmError::ProviderAuth("Invalid API key".to_string());
        assert_eq!(
            err.to_string(),
            "Provider authentication error: Invalid API key"
        );

        let err = LlmError::ProviderQuota("Rate limit exceeded".to_string());
        assert_eq!(
            err.to_string(),
            "Provider quota exceeded: Rate limit exceeded"
        );

        let err = LlmError::ProviderOutage("Service unavailable".to_string());
        assert_eq!(err.to_string(), "Provider outage: Service unavailable");

        let err = LlmError::Timeout {
            duration: Duration::from_secs(300),
        };
        assert_eq!(err.to_string(), "Timeout after 300s");

        let err = LlmError::BudgetExceeded {
            limit: 20,
            attempted: 21,
        };
        assert_eq!(
            err.to_string(),
            "Budget exceeded: attempted 21 calls, limit is 20"
        );

        let err = LlmError::Misconfiguration("Missing config".to_string());
        assert_eq!(err.to_string(), "Misconfiguration: Missing config");

        let err = LlmError::Unsupported("Feature not available".to_string());
        assert_eq!(err.to_string(), "Unsupported: Feature not available");
    }
}

//! LLM integration helpers for `PhaseOrchestrator`.
//!
//! This module contains LLM-related code extracted from mod.rs.

use anyhow::Result;
use std::collections::HashMap;
use std::fmt;

use crate::config::{
    ClaudeConfig, Config, Defaults, GeminiConfig, LlmConfig, PhaseConfig, PhasesConfig,
    PromptTemplate, RunnerConfig, SecurityConfig, Selectors,
};
use crate::error::XCheckerError;
use crate::hooks::HooksConfig;
use crate::llm::{LlmBackend, LlmFallbackInfo, LlmInvocation, LlmResult, Message};
use crate::types::PhaseId;

use super::{OrchestratorConfig, PhaseOrchestrator, PhaseTimeout};

/// Metadata from Claude CLI execution for receipt generation.
///
/// Internal type used to track LLM execution details that get written to receipts.
/// This type is specific to the Claude CLI backend and will be generalized in future versions.
#[derive(Debug, Clone)]
pub(crate) struct ClaudeExecutionMetadata {
    pub model_alias: Option<String>,
    pub model_full_name: String,
    pub claude_cli_version: String,
    pub fallback_used: bool,
    pub runner: String,
    pub runner_distro: Option<String>,
    pub stderr_tail: Option<String>,
}

/// Error wrapper that preserves fallback warning metadata on invocation failures.
#[derive(Debug)]
pub(crate) struct LlmInvocationError {
    error: XCheckerError,
    fallback_warning: Option<String>,
}

impl LlmInvocationError {
    pub(crate) fn new(error: XCheckerError, fallback_warning: Option<String>) -> Self {
        Self {
            error,
            fallback_warning,
        }
    }

    pub(crate) fn error(&self) -> &XCheckerError {
        &self.error
    }

    pub(crate) fn fallback_warning(&self) -> Option<&str> {
        self.fallback_warning.as_deref()
    }
}

impl fmt::Display for LlmInvocationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.error)
    }
}

impl std::error::Error for LlmInvocationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

fn apply_overrides_from_map(config: &mut Config, overrides: &HashMap<String, String>) {
    if let Some(model) = overrides.get("model") {
        config.defaults.model = Some(model.clone());
    }
    if let Some(max_turns) = overrides.get("max_turns")
        && let Ok(parsed) = max_turns.parse::<u32>()
    {
        config.defaults.max_turns = Some(parsed);
    }
    if let Some(output_format) = overrides.get("output_format") {
        config.defaults.output_format = Some(output_format.clone());
    }
    if let Some(phase_timeout) = overrides.get("phase_timeout")
        && let Ok(parsed) = phase_timeout.parse::<u64>()
    {
        config.defaults.phase_timeout = Some(parsed);
    }
    if let Some(runner_mode) = overrides.get("runner_mode") {
        config.runner.mode = Some(runner_mode.clone());
    }
    if let Some(runner_distro) = overrides.get("runner_distro") {
        config.runner.distro = Some(runner_distro.clone());
    }
    if let Some(claude_path) = overrides.get("claude_path") {
        config.runner.claude_path = Some(claude_path.clone());
    }
    if let Some(provider) = overrides.get("llm_provider") {
        config.llm.provider = Some(provider.clone());
    }
    if let Some(fallback_provider) = overrides.get("llm_fallback_provider") {
        config.llm.fallback_provider = Some(fallback_provider.clone());
    }
    if let Some(execution_strategy) = overrides.get("execution_strategy") {
        config.llm.execution_strategy = Some(execution_strategy.clone());
    }
    if let Some(prompt_template) = overrides.get("prompt_template") {
        config.llm.prompt_template = Some(prompt_template.clone());
    }

    // Claude binary path precedence (explicit, single chain):
    // 1. llm_claude_binary (preferred)
    // 2. claude_path (legacy alias)
    // 3. claude_cli_path (oldest alias)
    if let Some(claude_binary_path) = overrides
        .get("llm_claude_binary")
        .or_else(|| overrides.get("claude_path"))
        .or_else(|| overrides.get("claude_cli_path"))
    {
        if config.llm.claude.is_none() {
            config.llm.claude = Some(ClaudeConfig { binary: None });
        }
        if let Some(claude_config) = config.llm.claude.as_mut() {
            claude_config.binary = Some(claude_binary_path.clone());
        }
        config.runner.claude_path = Some(claude_binary_path.clone());
    }

    if let Some(gemini_binary) = overrides.get("llm_gemini_binary") {
        if config.llm.gemini.is_none() {
            config.llm.gemini = Some(GeminiConfig {
                binary: None,
                default_model: None,
                profiles: None,
            });
        }
        if let Some(gemini_config) = config.llm.gemini.as_mut() {
            gemini_config.binary = Some(gemini_binary.clone());
        }
    }

    if let Some(default_model) = overrides.get("llm_gemini_default_model") {
        if config.llm.gemini.is_none() {
            config.llm.gemini = Some(GeminiConfig {
                binary: None,
                default_model: None,
                profiles: None,
            });
        }
        if let Some(gemini_config) = config.llm.gemini.as_mut() {
            gemini_config.default_model = Some(default_model.clone());
        }
    }

    let apply_phase_override = |phase_name: &str, target: &mut Option<PhaseConfig>| {
        let model = overrides
            .get(&format!("phases.{phase_name}.model"))
            .cloned();
        let max_turns = overrides
            .get(&format!("phases.{phase_name}.max_turns"))
            .and_then(|s| s.parse::<u32>().ok());
        let phase_timeout = overrides
            .get(&format!("phases.{phase_name}.phase_timeout"))
            .and_then(|s| s.parse::<u64>().ok());

        if model.is_some() || max_turns.is_some() || phase_timeout.is_some() {
            *target = Some(PhaseConfig {
                model,
                max_turns,
                phase_timeout,
            });
        }
    };

    apply_phase_override("requirements", &mut config.phases.requirements);
    apply_phase_override("design", &mut config.phases.design);
    apply_phase_override("tasks", &mut config.phases.tasks);
    apply_phase_override("review", &mut config.phases.review);
    apply_phase_override("fixup", &mut config.phases.fixup);
    apply_phase_override("final", &mut config.phases.final_);
}

fn build_messages_from_template(
    template: PromptTemplate,
    prompt: &str,
    packet: &str,
) -> Vec<Message> {
    let trimmed_prompt = prompt.trim();
    let trimmed_packet = packet.trim();
    let has_packet = !trimmed_packet.is_empty();

    match template {
        PromptTemplate::Default => {
            let mut content = String::new();
            content.push_str(trimmed_prompt);
            if has_packet {
                content.push_str("\n\n# Context Packet\n");
                content.push_str(trimmed_packet);
            }
            vec![Message::user(content)]
        }
        PromptTemplate::ClaudeOptimized => {
            let mut user_content = String::new();
            user_content.push_str("<instructions>\n");
            user_content.push_str(trimmed_prompt);
            user_content.push_str("\n</instructions>");
            if has_packet {
                user_content.push_str("\n<context>\n");
                user_content.push_str(trimmed_packet);
                user_content.push_str("\n</context>");
            }
            vec![
                Message::system(
                    "You are xchecker. Follow the <instructions> and use <context> when provided. Output only the requested document.",
                ),
                Message::user(user_content),
            ]
        }
        PromptTemplate::OpenAiCompatible => {
            let mut user_content = String::new();
            user_content.push_str(trimmed_prompt);
            if has_packet {
                user_content.push_str("\n\nContext:\n");
                user_content.push_str(trimmed_packet);
            }
            vec![
                Message::system(
                    "You are xchecker. Follow the instructions and use the provided context.",
                ),
                Message::user(user_content),
            ]
        }
    }
}

impl PhaseOrchestrator {
    /// Build a minimal Config from `OrchestratorConfig` for LLM backend construction.
    ///
    /// Internal helper for V11 that extracts the necessary configuration
    /// from `OrchestratorConfig`. Future versions may pass full `Config` to orchestrator.
    ///
    /// This is not part of the public API.
    pub(crate) fn config_from_orchestrator_config(
        &self,
        orc_config: &OrchestratorConfig,
    ) -> Config {
        let mut config = if let Some(full_config) = &orc_config.full_config {
            full_config.clone()
        } else {
            Config {
                defaults: Defaults::default(),
                selectors: Selectors::default(),
                runner: RunnerConfig::default(),
                llm: LlmConfig {
                    provider: None,
                    fallback_provider: None,
                    claude: None,
                    gemini: None,
                    openrouter: None,
                    anthropic: None,
                    execution_strategy: None,
                    prompt_template: None,
                },
                phases: PhasesConfig::default(),
                hooks: HooksConfig::default(),
                security: SecurityConfig::default(),
                source_attribution: HashMap::new(),
            }
        };

        apply_overrides_from_map(&mut config, &orc_config.config);

        config
    }

    /// Construct LLM backend based on configuration.
    ///
    /// Internal helper that creates the appropriate LLM backend.
    /// In V11, only `ClaudeCliBackend` is supported. Future versions will support
    /// multiple providers based on config.
    ///
    /// This is not part of the public API.
    pub(crate) fn make_llm_backend(
        &self,
        orc_config: &OrchestratorConfig,
    ) -> Result<(Box<dyn LlmBackend>, Option<LlmFallbackInfo>), XCheckerError> {
        // Build a Config from OrchestratorConfig
        let cfg = self.config_from_orchestrator_config(orc_config);

        // Use the factory function to construct the appropriate backend
        crate::llm::from_config_with_fallback(&cfg).map_err(XCheckerError::Llm)
    }

    /// Build `LlmInvocation` from packet and phase context.
    ///
    /// Internal helper that constructs an invocation with model, timeout, and messages.
    /// Uses per-phase model configuration with precedence:
    /// 1. Phase-specific override (`[phases.<phase>].model`)
    /// 2. Global default (`[defaults].model`)
    /// 3. Empty string (backend handles its own default)
    ///
    /// This is not part of the public API.
    pub(crate) fn build_llm_invocation(
        &self,
        phase_id: PhaseId,
        prompt: &str,
        packet: &str,
        orc_config: &OrchestratorConfig,
    ) -> LlmInvocation {
        // Build Config from OrchestratorConfig to use model_for_phase
        let cfg = self.config_from_orchestrator_config(orc_config);

        let provider = cfg.llm.provider.as_deref().unwrap_or("claude-cli");

        // Resolve model with explicit overrides when provided, otherwise defer to provider defaults.
        // Model resolution precedence:
        // 1. Phase-specific override ([phases.<phase>].model)
        // 2. Global default ([defaults].model)
        // 3. Empty string - backend handles its own default (e.g., claude-cli uses "haiku",
        //    HTTP backends use their configured [llm.<provider>].model)
        //
        // Note: We don't force "haiku" for claude-cli here because:
        // - If fallback to a different provider happens, the wrong model would be used
        // - Each backend should handle its own default model selection
        let phase_model = match phase_id {
            PhaseId::Requirements => cfg.phases.requirements.as_ref(),
            PhaseId::Design => cfg.phases.design.as_ref(),
            PhaseId::Tasks => cfg.phases.tasks.as_ref(),
            PhaseId::Review => cfg.phases.review.as_ref(),
            PhaseId::Fixup => cfg.phases.fixup.as_ref(),
            PhaseId::Final => cfg.phases.final_.as_ref(),
        }
        .and_then(|pc| pc.model.clone())
        .filter(|model| !model.is_empty());

        let model = phase_model.unwrap_or_else(|| {
            cfg.defaults
                .model
                .clone()
                .filter(|m| !m.is_empty())
                .unwrap_or_default()
        });

        // Get timeout from config with minimum enforcement
        let timeout = PhaseTimeout::from_config(orc_config).duration;

        // Build messages using the configured prompt template, including packet context.
        let template = cfg
            .llm
            .prompt_template
            .as_deref()
            .and_then(|name| PromptTemplate::parse(name).ok())
            .unwrap_or(PromptTemplate::Default);

        let messages = build_messages_from_template(template, prompt, packet);

        // Create invocation
        let mut invocation =
            LlmInvocation::new(&self.spec_id, phase_id.as_str(), model, timeout, messages);

        if let Some(scenario) = orc_config.config.get("claude_scenario") {
            invocation.metadata.insert(
                "claude_scenario".to_string(),
                serde_json::Value::String(scenario.clone()),
            );
        }

        if provider == "gemini-cli" {
            let has_profile = cfg
                .llm
                .gemini
                .as_ref()
                .and_then(|gemini| gemini.profiles.as_ref())
                .is_some_and(|profiles| profiles.contains_key(phase_id.as_str()));

            if has_profile {
                invocation.metadata.insert(
                    "profile".to_string(),
                    serde_json::Value::String(phase_id.as_str().to_string()),
                );
            }
        }

        invocation
    }

    /// Execute LLM invocation using the backend abstraction.
    ///
    /// Internal helper that invokes the LLM backend and converts results to the format
    /// expected by the orchestrator's execution flow.
    ///
    /// Returns `(response_text, exit_code, metadata, llm_result)` tuple compatible with existing code.
    ///
    /// This is not part of the public API.
    pub(crate) async fn run_llm_invocation(
        &self,
        prompt: &str,
        packet: &str,
        phase_id: PhaseId,
        config: &OrchestratorConfig,
    ) -> Result<(
        String,
        i32,
        Option<ClaudeExecutionMetadata>,
        Option<LlmResult>,
        Option<String>,
    )> {
        // Build LLM invocation
        let invocation = self.build_llm_invocation(phase_id, prompt, packet, config);

        // Get backend
        let (backend, fallback_info) = self.make_llm_backend(config)?;
        let fallback_warning = fallback_info.map(|info| info.warning_message());
        let fallback_warning_for_error = fallback_warning.clone();

        // Invoke LLM
        let llm_result = backend.invoke(invocation).await.map_err(|err| {
            anyhow::Error::new(LlmInvocationError::new(
                XCheckerError::Llm(err),
                fallback_warning_for_error.clone(),
            ))
        })?;
        let llm_result = if let Some(ref warning) = fallback_warning {
            llm_result.with_extension(
                "llm_fallback_warning",
                serde_json::Value::String(warning.clone()),
            )
        } else {
            llm_result
        };

        let exit_code = llm_result
            .extensions
            .get("exit_code")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;

        let claude_cli_version = llm_result
            .extensions
            .get("claude_cli_version")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let fallback_used = llm_result
            .extensions
            .get("fallback_used")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let runner = llm_result
            .extensions
            .get("runner_used")
            .and_then(|v| v.as_str())
            .unwrap_or("native")
            .to_string();

        let runner_distro = llm_result
            .extensions
            .get("runner_distro")
            .and_then(|v| v.as_str())
            .map(str::to_string);

        // For V11, we need to convert LlmResult back to the format expected by existing code
        // This maintains compatibility while using the new abstraction
        let metadata = ClaudeExecutionMetadata {
            model_alias: None, // LlmResult doesn't track alias yet
            model_full_name: llm_result.model_used.clone(),
            claude_cli_version,
            fallback_used,
            runner,
            runner_distro,
            stderr_tail: llm_result
                .extensions
                .get("stderr")
                .and_then(|v| v.as_str().map(String::from)),
        };

        // Exit code is derived from backend extensions (defaults to 0)
        // Errors are handled via XCheckerError::Llm mapping
        Ok((
            llm_result.raw_response.clone(),
            exit_code,
            Some(metadata),
            Some(llm_result),
            fallback_warning,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::build_messages_from_template;
    use crate::config::PromptTemplate;
    use crate::llm::Role;

    #[test]
    fn build_messages_default_includes_packet() {
        let messages =
            build_messages_from_template(PromptTemplate::Default, "Do the thing", "packet info");

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, Role::User);
        assert_eq!(
            messages[0].content,
            "Do the thing\n\n# Context Packet\npacket info"
        );
    }

    #[test]
    fn build_messages_claude_optimized_includes_context() {
        let messages = build_messages_from_template(
            PromptTemplate::ClaudeOptimized,
            "Follow steps",
            "context here",
        );

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, Role::System);
        assert_eq!(
            messages[0].content,
            "You are xchecker. Follow the <instructions> and use <context> when provided. Output only the requested document."
        );
        assert_eq!(messages[1].role, Role::User);
        assert_eq!(
            messages[1].content,
            "<instructions>\nFollow steps\n</instructions>\n<context>\ncontext here\n</context>"
        );
    }

    #[test]
    fn build_messages_openai_compatible_omits_empty_packet() {
        let messages =
            build_messages_from_template(PromptTemplate::OpenAiCompatible, "Write summary", "   ");

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, Role::System);
        assert_eq!(
            messages[0].content,
            "You are xchecker. Follow the instructions and use the provided context."
        );
        assert_eq!(messages[1].role, Role::User);
        assert_eq!(messages[1].content, "Write summary");
    }
}

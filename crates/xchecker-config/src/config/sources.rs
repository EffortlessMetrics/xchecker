use std::collections::HashMap;

use super::Config;
use xchecker_utils::types::ConfigSource;

fn stable_source_label(source: &ConfigSource) -> &'static str {
    match source {
        ConfigSource::Cli => "cli",
        ConfigSource::Env => "env",
        ConfigSource::Config => "config",
        ConfigSource::Programmatic => "programmatic",
        ConfigSource::Default => "default",
    }
}

fn source_label(source: Option<&ConfigSource>) -> String {
    match source {
        Some(src) => stable_source_label(src).to_string(),
        None => stable_source_label(&ConfigSource::Default).to_string(),
    }
}

impl Config {
    /// Get effective configuration as key-value pairs with source attribution
    #[must_use]
    pub fn effective_config(&self) -> HashMap<String, (String, String)> {
        let mut config = HashMap::new();

        // Helper to add config value with source
        let mut add_config = |key: &str, value: Option<&str>| {
            if let Some(val) = value {
                let source = source_label(self.source_attribution.get(key));
                config.insert(key.to_string(), (val.to_string(), source));
            }
        };

        // Add all configuration values
        add_config("model", self.defaults.model.as_deref());

        if let Some(max_turns) = self.defaults.max_turns {
            add_config("max_turns", Some(&max_turns.to_string()));
        }

        if let Some(packet_max_bytes) = self.defaults.packet_max_bytes {
            add_config("packet_max_bytes", Some(&packet_max_bytes.to_string()));
        }

        if let Some(packet_max_lines) = self.defaults.packet_max_lines {
            add_config("packet_max_lines", Some(&packet_max_lines.to_string()));
        }

        add_config("output_format", self.defaults.output_format.as_deref());

        if let Some(verbose) = self.defaults.verbose {
            add_config("verbose", Some(&verbose.to_string()));
        }

        add_config("runner_mode", self.runner.mode.as_deref());
        add_config("runner_distro", self.runner.distro.as_deref());
        add_config("claude_path", self.runner.claude_path.as_deref());
        add_config("llm_provider", self.llm.provider.as_deref());
        add_config(
            "llm_fallback_provider",
            self.llm.fallback_provider.as_deref(),
        );
        add_config("execution_strategy", self.llm.execution_strategy.as_deref());
        add_config("prompt_template", self.llm.prompt_template.as_deref());

        // Add selector information
        let include_patterns = self.selectors.include.join(", ");
        let exclude_patterns = self.selectors.exclude.join(", ");

        let include_source = source_label(self.source_attribution.get("selectors_include"));
        let exclude_source = source_label(self.source_attribution.get("selectors_exclude"));

        config.insert(
            "selectors_include".to_string(),
            (include_patterns, include_source),
        );
        config.insert(
            "selectors_exclude".to_string(),
            (exclude_patterns, exclude_source),
        );

        config
    }
}

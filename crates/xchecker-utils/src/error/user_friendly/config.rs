use super::UserFriendlyError;
use crate::error::{ConfigError, ErrorCategory};

impl UserFriendlyError for ConfigError {
    fn user_message(&self) -> String {
        match self {
            Self::InvalidFile(reason) => {
                format!("Configuration file has invalid format: {reason}")
            }
            Self::MissingRequired(key) => {
                format!("Required configuration '{key}' is missing")
            }
            Self::InvalidValue { key, value } => {
                format!("Configuration '{key}' has invalid value: {value}")
            }
            Self::NotFound { path } => {
                format!("Configuration file not found: {path}")
            }
            Self::DiscoveryFailed { reason } => {
                format!("Failed to discover configuration: {reason}")
            }
            Self::ValidationFailed {
                errors,
                error_count: _,
            } => {
                format!(
                    "Configuration validation failed with {} errors: {}",
                    errors.len(),
                    errors.join(", ")
                )
            }
            Self::UnsupportedVersion { version } => {
                format!(
                    "Configuration version '{version}' is not supported by this version of xchecker"
                )
            }
        }
    }

    fn context(&self) -> Option<String> {
        match self {
            Self::InvalidFile(_) => {
                Some("Configuration files must be valid TOML format with [defaults] and [selectors] sections.".to_string())
            }
            Self::MissingRequired(_) => {
                Some("Some configuration values are required for xchecker to function properly.".to_string())
            }
            Self::InvalidValue { key, value: _ } => {
                Some(format!("The '{key}' configuration option has specific format requirements."))
            }
            Self::NotFound { path: _ } => {
                Some("xchecker searches for .xchecker/config.toml starting from the current directory upward.".to_string())
            }
            Self::DiscoveryFailed { reason: _ } => {
                Some("Configuration discovery involves searching the directory tree for .xchecker/config.toml files.".to_string())
            }
            Self::ValidationFailed { errors: _, error_count: _ } => {
                Some("Configuration validation ensures all required sections and values are properly formatted.".to_string())
            }
            Self::UnsupportedVersion { version: _ } => {
                Some("Configuration file format versions ensure compatibility between xchecker versions.".to_string())
            }
        }
    }

    fn suggestions(&self) -> Vec<String> {
        match self {
            Self::InvalidFile(_) => vec![
                "Check the TOML syntax using a TOML validator".to_string(),
                "Ensure the file has proper [defaults] and [selectors] sections".to_string(),
                "Compare with the example configuration in the documentation".to_string(),
            ],
            Self::MissingRequired(key) => vec![
                format!(
                    "Add '{}' to the [defaults] section of .xchecker/config.toml",
                    key
                ),
                "Check the documentation for required configuration options".to_string(),
                "Use CLI flags as a temporary workaround".to_string(),
            ],
            Self::InvalidValue { key, value: _ } => match key.as_str() {
                "model" => vec![
                    "Use a valid Claude model name (e.g., 'haiku', 'sonnet', 'opus')".to_string(),
                    "Check available models with 'claude models'".to_string(),
                ],
                "packet_max_bytes" | "packet_max_lines" => vec![
                    "Use a positive integer value".to_string(),
                    "Consider reasonable limits (e.g., 65536 bytes, 1200 lines)".to_string(),
                ],
                "source" => vec![
                    "Use 'gh', 'fs', or 'stdin' as the source type".to_string(),
                    "For GitHub: --source gh --gh owner/repo".to_string(),
                    "For filesystem: --source fs --repo /path/to/repo".to_string(),
                    "For stdin: --source stdin (default)".to_string(),
                ],
                "gh" => vec![
                    "Use format 'owner/repo' for GitHub repositories".to_string(),
                    "Example: --gh anthropic/claude-cli".to_string(),
                    "Ensure the repository exists and is accessible".to_string(),
                ],
                "repo" => vec![
                    "Provide a valid filesystem path".to_string(),
                    "Ensure the directory exists and is readable".to_string(),
                    "Use absolute or relative paths".to_string(),
                ],
                _ => vec![
                    "Check the documentation for valid values for this option".to_string(),
                    "Remove the option to use the default value".to_string(),
                ],
            },
            Self::NotFound { path: _ } => vec![
                "Create .xchecker/config.toml in your project root".to_string(),
                "Use CLI flags instead of a configuration file".to_string(),
                "Check that you're running xchecker from the correct directory".to_string(),
            ],
            Self::DiscoveryFailed { reason: _ } => vec![
                "Check file permissions in the current directory and parent directories"
                    .to_string(),
                "Ensure you have read access to the directory tree".to_string(),
                "Try running from a different directory with proper permissions".to_string(),
                "Use --config <path> to specify configuration file explicitly".to_string(),
            ],
            Self::ValidationFailed {
                errors: _,
                error_count: _,
            } => vec![
                "Review the configuration file syntax and structure".to_string(),
                "Check that all required sections ([defaults], [selectors]) are present"
                    .to_string(),
                "Validate TOML syntax using an online TOML validator".to_string(),
                "Compare with the example configuration in documentation".to_string(),
            ],
            Self::UnsupportedVersion { version: _ } => vec![
                "Update xchecker to the latest version".to_string(),
                "Check the documentation for supported configuration versions".to_string(),
                "Migrate configuration to the current format".to_string(),
            ],
        }
    }

    fn category(&self) -> ErrorCategory {
        ErrorCategory::Configuration
    }
}

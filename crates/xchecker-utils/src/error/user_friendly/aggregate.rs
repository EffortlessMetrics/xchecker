use super::UserFriendlyError;
use crate::error::{ErrorCategory, XCheckerError};

impl UserFriendlyError for XCheckerError {
    fn user_message(&self) -> String {
        match self {
            Self::Config(config_err) => config_err.user_message(),
            Self::Phase(phase_err) => phase_err.user_message(),
            Self::Claude(claude_err) => claude_err.user_message(),
            Self::Runner(runner_err) => runner_err.user_message(),
            Self::Llm(llm_err) => llm_err.user_message(),
            Self::Io(io_err) => {
                format!("File system operation failed: {io_err}")
            }
            Self::SecretDetected {
                pattern: _,
                location,
            } => {
                format!("Security issue: Detected potential secret in {location}")
            }
            Self::PacketOverflow {
                used_bytes,
                used_lines,
                limit_bytes,
                limit_lines,
            } => {
                format!(
                    "Packet size exceeded limits: {used_bytes} bytes/{used_lines} lines used, {limit_bytes} bytes/{limit_lines} lines allowed"
                )
            }
            Self::ConcurrentExecution { id } => {
                format!("Another xchecker process is already working on spec '{id}'")
            }
            Self::PacketPreviewTooLarge { size } => {
                format!("Packet preview is too large: {size} bytes")
            }
            Self::CanonicalizationFailed { phase, reason } => {
                format!("Failed to normalize output from {phase} phase: {reason}")
            }
            Self::ReceiptWriteFailed { path, reason } => {
                format!("Failed to save execution record to {path}: {reason}")
            }
            Self::ModelResolutionError {
                alias,
                resolved: _,
                reason,
            } => {
                format!("Could not resolve model '{alias}': {reason}")
            }
            Self::Source(source_err) => source_err.user_message(),
            Self::Fixup(fixup_err) => fixup_err.user_message(),
            Self::SpecId(spec_id_err) => spec_id_err.user_message(),
            Self::Lock(lock_err) => lock_err.user_message(),
            Self::ValidationFailed {
                phase,
                issues,
                issue_count: _,
            } => {
                let issue_list: Vec<String> = issues.iter().map(|i| i.to_string()).collect();
                format!(
                    "Validation failed for {} phase: {}",
                    phase,
                    issue_list.join("; ")
                )
            }
        }
    }

    fn context(&self) -> Option<String> {
        match self {
            Self::Config(config_err) => config_err.context(),
            Self::Phase(phase_err) => phase_err.context(),
            Self::Claude(claude_err) => claude_err.context(),
            Self::Runner(runner_err) => runner_err.context(),
            Self::Llm(llm_err) => llm_err.context(),
            Self::Io(_) => Some("This usually indicates a permissions issue or disk space problem.".to_string()),
            Self::SecretDetected { pattern, location: _ } => {
                Some(format!("The pattern '{pattern}' matches common secret formats. This prevents accidental exposure of sensitive data."))
            }
            Self::PacketOverflow { used_bytes: _, used_lines: _, limit_bytes: _, limit_lines: _ } => {
                Some("Packet size limits prevent excessive token usage and ensure Claude API calls remain efficient.".to_string())
            }
            Self::ConcurrentExecution { id: _ } => {
                Some("xchecker uses file locking to prevent data corruption from simultaneous executions.".to_string())
            }
            Self::PacketPreviewTooLarge { size: _ } => {
                Some("Packet previews are limited to prevent excessive disk usage.".to_string())
            }
            Self::CanonicalizationFailed { phase: _, reason: _ } => {
                Some("Canonicalization ensures deterministic output hashing for reproducible results.".to_string())
            }
            Self::ReceiptWriteFailed { path: _, reason: _ } => {
                Some("Receipts provide audit trails and enable resumption of failed executions.".to_string())
            }
            Self::ModelResolutionError { alias: _, resolved: _, reason: _ } => {
                Some("Model resolution maps short aliases to full model names for Claude API calls.".to_string())
            }
            Self::Source(source_err) => source_err.context(),
            Self::Fixup(fixup_err) => fixup_err.context(),
            Self::SpecId(spec_id_err) => spec_id_err.context(),
            Self::Lock(lock_err) => lock_err.context(),
            Self::ValidationFailed { .. } => {
                Some("Strict validation is enabled. LLM output must meet quality requirements: no meta-summaries, minimum length, and required sections.".to_string())
            }
        }
    }

    fn suggestions(&self) -> Vec<String> {
        match self {
            Self::Config(config_err) => config_err.suggestions(),
            Self::Phase(phase_err) => phase_err.suggestions(),
            Self::Claude(claude_err) => claude_err.suggestions(),
            Self::Runner(runner_err) => runner_err.suggestions(),
            Self::Llm(llm_err) => llm_err.suggestions(),
            Self::Io(_) => vec![
                "Check file permissions in the current directory".to_string(),
                "Ensure sufficient disk space is available".to_string(),
                "Verify the directory is writable".to_string(),
            ],
            Self::SecretDetected {
                pattern: _,
                location: _,
            } => vec![
                "Use --ignore-secret-pattern <regex> to suppress this specific pattern".to_string(),
                "Remove or redact the sensitive data from the file".to_string(),
                "Add the file to .gitignore if it contains test data".to_string(),
            ],
            Self::PacketOverflow {
                used_bytes: _,
                used_lines: _,
                limit_bytes,
                limit_lines,
            } => vec![
                format!(
                    "Increase packet_max_bytes in config (current limit: {})",
                    limit_bytes
                ),
                format!(
                    "Increase packet_max_lines in config (current limit: {})",
                    limit_lines
                ),
                "Use more specific include/exclude patterns to reduce content".to_string(),
                "Split large files into smaller, more focused pieces".to_string(),
            ],
            Self::ConcurrentExecution { id } => vec![
                format!(
                    "Wait for the other process to complete or use 'xchecker status {}' to check progress",
                    id
                ),
                "Use --force flag to override the lock (use with caution)".to_string(),
                "Check if a previous process crashed and left a stale lock".to_string(),
            ],
            Self::PacketPreviewTooLarge { size: _ } => vec![
                "Reduce the packet size limits in configuration".to_string(),
                "Use more restrictive include patterns".to_string(),
            ],
            Self::CanonicalizationFailed {
                phase: _,
                reason: _,
            } => vec![
                "Check that the output format matches expected structure".to_string(),
                "Verify YAML syntax if the error involves YAML canonicalization".to_string(),
                "Review the phase output for formatting issues".to_string(),
            ],
            Self::ReceiptWriteFailed { path: _, reason: _ } => vec![
                "Check write permissions for the .xchecker directory".to_string(),
                "Ensure sufficient disk space is available".to_string(),
                "Verify the parent directory exists and is writable".to_string(),
            ],
            Self::ModelResolutionError {
                alias: _,
                resolved: _,
                reason: _,
            } => vec![
                "Check that the Claude CLI is properly installed and authenticated".to_string(),
                "Verify the model name is correct and available".to_string(),
                "Try using the full model name instead of an alias".to_string(),
            ],
            Self::Source(source_err) => source_err.suggestions(),
            Self::Fixup(fixup_err) => fixup_err.suggestions(),
            Self::SpecId(spec_id_err) => spec_id_err.suggestions(),
            Self::Lock(lock_err) => lock_err.suggestions(),
            Self::ValidationFailed { phase, .. } => vec![
                format!(
                    "Set strict_validation = false in config to log warnings instead of failing"
                ),
                format!(
                    "Review the {} phase prompt to ensure it produces compliant output",
                    phase
                ),
                "Check if the LLM response starts with meta-commentary instead of content"
                    .to_string(),
                "Ensure the response meets minimum length requirements".to_string(),
                "Verify required section headers are present in the output".to_string(),
            ],
        }
    }

    fn category(&self) -> ErrorCategory {
        match self {
            Self::Config(_) => ErrorCategory::Configuration,
            Self::Phase(_) => ErrorCategory::PhaseExecution,
            Self::Claude(_) => ErrorCategory::ClaudeIntegration,
            Self::Runner(_) => ErrorCategory::ClaudeIntegration,
            Self::Llm(llm_err) => llm_err.category(),
            Self::Io(_) => ErrorCategory::FileSystem,
            Self::SecretDetected { .. } => ErrorCategory::Security,
            Self::PacketOverflow { .. } => ErrorCategory::ResourceLimits,
            Self::ConcurrentExecution { .. } => ErrorCategory::Concurrency,
            Self::PacketPreviewTooLarge { .. } => ErrorCategory::ResourceLimits,
            Self::CanonicalizationFailed { .. } => ErrorCategory::Validation,
            Self::ReceiptWriteFailed { .. } => ErrorCategory::FileSystem,
            Self::ModelResolutionError { .. } => ErrorCategory::ClaudeIntegration,
            Self::Source(_) => ErrorCategory::Configuration,
            Self::Fixup(fixup_err) => fixup_err.category(),
            Self::SpecId(_) => ErrorCategory::Validation,
            Self::Lock(lock_err) => lock_err.category(),
            Self::ValidationFailed { .. } => ErrorCategory::Validation,
        }
    }
}

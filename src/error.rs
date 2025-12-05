use std::fmt;
use thiserror::Error;

/// Main error type for xchecker operations with user-friendly reporting
#[derive(Error, Debug)]
pub enum XCheckerError {
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("Phase execution error: {0}")]
    Phase(#[from] PhaseError),

    #[error("Claude CLI error: {0}")]
    Claude(#[from] ClaudeError),

    #[error("Runner error: {0}")]
    Runner(#[from] RunnerError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Secret detected: {pattern} in {location}")]
    SecretDetected { pattern: String, location: String },

    #[error(
        "Packet overflow: {used_bytes} bytes, {used_lines} lines > limits {limit_bytes} bytes, {limit_lines} lines"
    )]
    PacketOverflow {
        used_bytes: usize,
        used_lines: usize,
        limit_bytes: usize,
        limit_lines: usize,
    },

    #[error("Concurrent execution detected for spec {id}")]
    ConcurrentExecution { id: String },

    #[error("Packet preview too large: {size} bytes")]
    PacketPreviewTooLarge { size: usize },

    #[error("Canonicalization failed in {phase}: {reason}")]
    CanonicalizationFailed { phase: String, reason: String },

    #[error("Receipt write failed at {path}: {reason}")]
    ReceiptWriteFailed { path: String, reason: String },

    #[error("Model resolution error: alias '{alias}' -> '{resolved}': {reason}")]
    ModelResolutionError {
        alias: String,
        resolved: String,
        reason: String,
    },

    #[error("Source resolution error: {0}")]
    Source(#[from] SourceError),

    #[error("Fixup error: {0}")]
    Fixup(#[from] crate::fixup::FixupError),

    #[error("Spec ID validation error: {0}")]
    SpecId(#[from] crate::spec_id::SpecIdError),

    #[error("File lock error: {0}")]
    Lock(#[from] crate::lock::LockError),

    #[error("LLM backend error: {0}")]
    Llm(#[from] crate::llm::LlmError),
}

/// Trait for providing user-friendly error reporting with context and suggestions
pub trait UserFriendlyError {
    /// Get a user-friendly error message
    fn user_message(&self) -> String;

    /// Get contextual information about the error
    fn context(&self) -> Option<String>;

    /// Get suggested actions to resolve the error
    fn suggestions(&self) -> Vec<String>;

    /// Get the error category for grouping similar errors
    fn category(&self) -> ErrorCategory;
}

/// Categories of errors for better organization and handling
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorCategory {
    Configuration,
    PhaseExecution,
    ClaudeIntegration,
    FileSystem,
    Security,
    ResourceLimits,
    Concurrency,
    Validation,
}

impl fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration => write!(f, "Configuration"),
            Self::PhaseExecution => write!(f, "Phase Execution"),
            Self::ClaudeIntegration => write!(f, "Claude Integration"),
            Self::FileSystem => write!(f, "File System"),
            Self::Security => write!(f, "Security"),
            Self::ResourceLimits => write!(f, "Resource Limits"),
            Self::Concurrency => write!(f, "Concurrency"),
            Self::Validation => write!(f, "Validation"),
        }
    }
}

/// Configuration-related errors
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Invalid configuration file: {0}")]
    InvalidFile(String),

    #[error("Missing required configuration: {0}")]
    MissingRequired(String),

    #[error("Invalid configuration value for {key}: {value}")]
    InvalidValue { key: String, value: String },

    #[error("Configuration file not found at {path}")]
    NotFound { path: String },

    #[error("Configuration discovery failed: {reason}")]
    DiscoveryFailed { reason: String },

    #[error("Configuration validation failed: {error_count} errors")]
    ValidationFailed {
        errors: Vec<String>,
        error_count: usize,
    },

    #[error("Unsupported configuration version: {version}")]
    UnsupportedVersion { version: String },
}

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

/// Phase execution errors
#[derive(Error, Debug)]
pub enum PhaseError {
    #[error("Phase {phase} failed with exit code {code}")]
    ExecutionFailed { phase: String, code: i32 },

    #[error("Phase {phase} dependency not satisfied: missing {dependency}")]
    DependencyNotSatisfied { phase: String, dependency: String },

    #[error("Invalid phase transition from {from} to {to}")]
    InvalidTransition { from: String, to: String },

    #[error("Phase {phase} output validation failed: {reason}")]
    OutputValidationFailed { phase: String, reason: String },

    #[error("Phase {phase} packet creation failed: {reason}")]
    PacketCreationFailed { phase: String, reason: String },

    #[error("Phase {phase} context creation failed: {reason}")]
    ContextCreationFailed { phase: String, reason: String },

    #[error("Phase {phase} timed out after {timeout_seconds} seconds")]
    Timeout { phase: String, timeout_seconds: u64 },

    #[error("Phase {phase} was interrupted by user")]
    Interrupted { phase: String },

    #[error("Phase {phase} resource limit exceeded: {resource} ({limit})")]
    ResourceLimitExceeded {
        phase: String,
        resource: String,
        limit: String,
    },

    #[error("Phase {phase} failed with stderr: {stderr_tail}")]
    ExecutionFailedWithStderr {
        phase: String,
        code: i32,
        stderr_tail: String,
    },

    #[error("Phase {phase} produced partial output due to failure")]
    PartialOutputSaved { phase: String, partial_path: String },
}

impl UserFriendlyError for PhaseError {
    fn user_message(&self) -> String {
        match self {
            Self::ExecutionFailed { phase, code } => {
                format!("The {phase} phase failed to complete successfully (exit code: {code})")
            }
            Self::DependencyNotSatisfied { phase, dependency } => {
                format!(
                    "Cannot run {phase} phase: {dependency} phase must complete successfully first"
                )
            }
            Self::InvalidTransition { from, to } => {
                format!("Cannot transition from {from} phase to {to} phase")
            }
            Self::OutputValidationFailed { phase, reason } => {
                format!("The {phase} phase produced invalid output: {reason}")
            }
            Self::PacketCreationFailed { phase, reason } => {
                format!("Failed to create input packet for {phase} phase: {reason}")
            }
            Self::ContextCreationFailed { phase, reason } => {
                format!("Failed to create execution context for {phase} phase: {reason}")
            }
            Self::Timeout {
                phase,
                timeout_seconds,
            } => {
                format!("The {phase} phase timed out after {timeout_seconds} seconds")
            }
            Self::Interrupted { phase } => {
                format!("The {phase} phase was interrupted")
            }
            Self::ResourceLimitExceeded {
                phase,
                resource,
                limit,
            } => {
                format!("The {phase} phase exceeded {resource} limit: {limit}")
            }
            Self::ExecutionFailedWithStderr {
                phase,
                code,
                stderr_tail,
            } => {
                format!(
                    "The {phase} phase failed (exit code: {code}) with error output: {stderr_tail}"
                )
            }
            Self::PartialOutputSaved {
                phase,
                partial_path,
            } => {
                format!("The {phase} phase failed and partial output was saved to: {partial_path}")
            }
        }
    }

    fn context(&self) -> Option<String> {
        match self {
            Self::ExecutionFailed { phase, code: _ } => {
                Some(format!("The {phase} phase encountered an error during execution. Partial outputs have been saved for debugging."))
            }
            Self::DependencyNotSatisfied { phase: _, dependency: _ } => {
                Some("xchecker phases have dependencies that must be satisfied before execution.".to_string())
            }
            Self::InvalidTransition { from: _, to: _ } => {
                Some("Phase transitions must follow the defined workflow order.".to_string())
            }
            Self::OutputValidationFailed { phase: _, reason: _ } => {
                Some("Phase outputs are validated to ensure they meet the expected format and structure.".to_string())
            }
            Self::PacketCreationFailed { phase: _, reason: _ } => {
                Some("Input packets contain the context and files needed for Claude to generate phase outputs.".to_string())
            }
            Self::ContextCreationFailed { phase: _, reason: _ } => {
                Some("Execution context includes configuration, artifacts, and environment needed for phase execution.".to_string())
            }
            Self::Timeout { phase: _, timeout_seconds: _ } => {
                Some("Phase execution has configurable timeouts to prevent hanging operations.".to_string())
            }
            Self::Interrupted { phase: _ } => {
                Some("Phase execution can be interrupted by user signals (Ctrl+C) or system events.".to_string())
            }
            Self::ResourceLimitExceeded { phase: _, resource: _, limit: _ } => {
                Some("Resource limits prevent excessive memory, disk, or network usage during phase execution.".to_string())
            }
            Self::ExecutionFailedWithStderr { phase: _, code: _, stderr_tail: _ } => {
                Some("Phase execution failed and stderr output has been captured for debugging.".to_string())
            }
            Self::PartialOutputSaved { phase: _, partial_path: _ } => {
                Some("Partial outputs are saved when phases fail to help with debugging and recovery.".to_string())
            }
        }
    }

    fn suggestions(&self) -> Vec<String> {
        match self {
            Self::ExecutionFailed { phase, code: _ } => {
                let mut suggestions = vec![
                    format!(
                        "Check the partial output in .xchecker/specs/<id>/artifacts/*-{}.partial.md",
                        phase.to_lowercase()
                    ),
                    "Review the receipt file for detailed error information".to_string(),
                    "Check the stderr output in the receipt for Claude CLI errors".to_string(),
                ];

                match phase.as_str() {
                    "REQUIREMENTS" => {
                        suggestions.push(
                            "Ensure your problem statement is clear and well-defined".to_string(),
                        );
                        suggestions.push("Try simplifying the requirements scope".to_string());
                    }
                    "DESIGN" => {
                        suggestions.push("Review the requirements for completeness".to_string());
                        suggestions
                            .push("Check if the design complexity is appropriate".to_string());
                    }
                    "TASKS" => {
                        suggestions.push("Verify the design document is complete".to_string());
                        suggestions.push("Consider breaking down complex tasks".to_string());
                    }
                    _ => {}
                }

                suggestions
            }
            Self::DependencyNotSatisfied {
                phase: _,
                dependency,
            } => vec![
                format!("Run the {} phase first: xchecker spec <id>", dependency),
                format!("Check the status of {}: xchecker status <id>", dependency),
                "Ensure the dependency phase completed successfully (exit code 0)".to_string(),
            ],
            Self::InvalidTransition { from: _, to: _ } => vec![
                "Follow the standard phase order: Requirements → Design → Tasks → Review → Fixup"
                    .to_string(),
                "Use 'xchecker resume <id> --phase <name>' to restart from a specific phase"
                    .to_string(),
                "Check 'xchecker status <id>' to see the current phase state".to_string(),
            ],
            Self::OutputValidationFailed {
                phase: _,
                reason: _,
            } => vec![
                "Check the phase output format matches the expected structure".to_string(),
                "Verify that required sections are present in the output".to_string(),
                "Review the canonicalization requirements for the output type".to_string(),
            ],
            Self::PacketCreationFailed {
                phase: _,
                reason: _,
            } => vec![
                "Check that all required input files are accessible".to_string(),
                "Verify packet size limits are not exceeded".to_string(),
                "Ensure no secrets are detected in the input files".to_string(),
                "Review include/exclude patterns in configuration".to_string(),
            ],
            Self::ContextCreationFailed {
                phase: _,
                reason: _,
            } => vec![
                "Check that the spec directory is accessible and writable".to_string(),
                "Verify configuration values are valid".to_string(),
                "Ensure previous phase artifacts are available if required".to_string(),
                "Check file permissions in the working directory".to_string(),
            ],
            Self::Timeout {
                phase,
                timeout_seconds: _,
            } => vec![
                format!("Increase timeout for {} phase in configuration", phase),
                "Check your internet connection if using Claude API".to_string(),
                "Try running with --verbose to see where it's hanging".to_string(),
                "Consider breaking down complex requests into smaller parts".to_string(),
            ],
            Self::Interrupted { phase: _ } => vec![
                "Resume the phase with: xchecker resume <id> --phase <name>".to_string(),
                "Check partial outputs in .xchecker/specs/<id>/artifacts/".to_string(),
                "Use --dry-run to test without making Claude calls".to_string(),
            ],
            Self::ResourceLimitExceeded {
                phase: _,
                resource,
                limit: _,
            } => match resource.as_str() {
                "memory" => vec![
                    "Reduce packet size limits in configuration".to_string(),
                    "Use more restrictive file include patterns".to_string(),
                    "Process files in smaller batches".to_string(),
                ],
                "disk" => vec![
                    "Free up disk space".to_string(),
                    "Clean old spec artifacts with: xchecker clean <id>".to_string(),
                    "Check available disk space".to_string(),
                ],
                "network" => vec![
                    "Check your internet connection".to_string(),
                    "Verify Claude API rate limits".to_string(),
                    "Try again later if rate limited".to_string(),
                ],
                _ => vec![
                    "Check system resources and limits".to_string(),
                    "Review configuration for resource settings".to_string(),
                ],
            },
            Self::ExecutionFailedWithStderr {
                phase,
                code: _,
                stderr_tail: _,
            } => vec![
                format!(
                    "Check the stderr output captured in the receipt for detailed error information"
                ),
                format!(
                    "Review partial outputs in .xchecker/specs/<id>/artifacts/*-{}.partial.md",
                    phase.to_lowercase()
                ),
                "Try running with --dry-run to test configuration without Claude calls".to_string(),
                "Check Claude CLI authentication and connectivity".to_string(),
            ],
            Self::PartialOutputSaved {
                phase: _,
                partial_path,
            } => vec![
                format!("Review the partial output at: {}", partial_path),
                "Check the receipt file for detailed error information and stderr output"
                    .to_string(),
                "Use the partial output to understand where the phase failed".to_string(),
                "Try resuming the phase after addressing any issues".to_string(),
            ],
        }
    }

    fn category(&self) -> ErrorCategory {
        ErrorCategory::PhaseExecution
    }
}

/// Source resolution errors (R6.4)
#[derive(Error, Debug)]
pub enum SourceError {
    #[error("GitHub repository not found: {owner}/{repo}")]
    GitHubRepoNotFound { owner: String, repo: String },

    #[error("GitHub issue not found: {owner}/{repo}#{issue}")]
    GitHubIssueNotFound {
        owner: String,
        repo: String,
        issue: String,
    },

    #[error("GitHub authentication failed: {reason}")]
    GitHubAuthFailed { reason: String },

    #[error("GitHub API error: {status} - {message}")]
    GitHubApiError { status: u16, message: String },

    #[error("Filesystem path not found: {path}")]
    FileSystemNotFound { path: String },

    #[error("Filesystem access denied: {path}")]
    FileSystemAccessDenied { path: String },

    #[error("Filesystem path is not a directory: {path}")]
    FileSystemNotDirectory { path: String },

    #[error("Stdin read failed: {reason}")]
    StdinReadFailed { reason: String },

    #[error("Empty input provided")]
    EmptyInput,

    #[error("Invalid source format: {reason}")]
    InvalidFormat { reason: String },
}

impl UserFriendlyError for SourceError {
    fn user_message(&self) -> String {
        match self {
            Self::GitHubRepoNotFound { owner, repo } => {
                format!("GitHub repository '{owner}/{repo}' could not be found or accessed")
            }
            Self::GitHubIssueNotFound { owner, repo, issue } => {
                format!("Issue #{issue} not found in repository '{owner}/{repo}'")
            }
            Self::GitHubAuthFailed { reason } => {
                format!("GitHub authentication failed: {reason}")
            }
            Self::GitHubApiError { status, message } => {
                format!("GitHub API returned error {status}: {message}")
            }
            Self::FileSystemNotFound { path } => {
                format!("Path '{path}' does not exist")
            }
            Self::FileSystemAccessDenied { path } => {
                format!("Access denied to path '{path}'")
            }
            Self::FileSystemNotDirectory { path } => {
                format!("Path '{path}' is not a directory")
            }
            Self::StdinReadFailed { reason } => {
                format!("Failed to read from standard input: {reason}")
            }
            Self::EmptyInput => {
                "No input provided - please provide a problem statement".to_string()
            }
            Self::InvalidFormat { reason } => {
                format!("Input format is invalid: {reason}")
            }
        }
    }

    fn context(&self) -> Option<String> {
        match self {
            Self::GitHubRepoNotFound { .. } => {
                Some("GitHub source resolution requires access to public repositories or proper authentication for private ones.".to_string())
            }
            Self::GitHubIssueNotFound { .. } => {
                Some("GitHub issues are resolved by their number within the specified repository.".to_string())
            }
            Self::GitHubAuthFailed { .. } => {
                Some("GitHub authentication is required for private repositories and API rate limiting.".to_string())
            }
            Self::GitHubApiError { .. } => {
                Some("GitHub API errors can be temporary or indicate rate limiting, authentication, or permission issues.".to_string())
            }
            Self::FileSystemNotFound { .. } => {
                Some("Filesystem source resolution requires the specified path to exist and be accessible.".to_string())
            }
            Self::FileSystemAccessDenied { .. } => {
                Some("File system permissions must allow read access to the specified directory.".to_string())
            }
            Self::FileSystemNotDirectory { .. } => {
                Some("Filesystem source resolution expects a directory containing project files.".to_string())
            }
            Self::StdinReadFailed { .. } => {
                Some("Standard input is used when no other source is specified or when --source stdin is used.".to_string())
            }
            Self::EmptyInput => {
                Some("xchecker requires a problem statement to generate specifications from.".to_string())
            }
            Self::InvalidFormat { .. } => {
                Some("Input should be a clear problem statement describing what you want to build.".to_string())
            }
        }
    }

    fn suggestions(&self) -> Vec<String> {
        match self {
            Self::GitHubRepoNotFound { owner, repo } => vec![
                format!(
                    "Verify the repository name: https://github.com/{}/{}",
                    owner, repo
                ),
                "Check that the repository is public or you have access".to_string(),
                "Ensure your GitHub authentication is working".to_string(),
                "Try using the full repository URL format".to_string(),
            ],
            Self::GitHubIssueNotFound { owner, repo, issue } => vec![
                format!(
                    "Check if issue #{} exists: https://github.com/{}/{}/issues/{}",
                    issue, owner, repo, issue
                ),
                "Verify the issue number is correct".to_string(),
                "Ensure the issue is not closed or private".to_string(),
                "Try using a different issue number".to_string(),
            ],
            Self::GitHubAuthFailed { .. } => vec![
                "Set up GitHub authentication: gh auth login".to_string(),
                "Check your GitHub token permissions".to_string(),
                "Verify your GitHub CLI is properly configured".to_string(),
                "Try accessing a public repository first".to_string(),
            ],
            Self::GitHubApiError { status, .. } => match *status {
                401 => vec![
                    "Authentication required - run 'gh auth login'".to_string(),
                    "Check your GitHub token is valid and not expired".to_string(),
                ],
                403 => vec![
                    "Rate limit exceeded - wait a few minutes and try again".to_string(),
                    "Authenticate to get higher rate limits".to_string(),
                ],
                404 => vec![
                    "Repository or resource not found - check the URL".to_string(),
                    "Verify you have access to the repository".to_string(),
                ],
                _ => vec![
                    "This may be a temporary GitHub API issue - try again later".to_string(),
                    "Check GitHub status: https://www.githubstatus.com/".to_string(),
                ],
            },
            Self::FileSystemNotFound { path } => vec![
                format!("Create the directory: mkdir -p '{}'", path),
                "Check the path spelling and case sensitivity".to_string(),
                "Use an absolute path to avoid confusion".to_string(),
                "Verify you're in the correct working directory".to_string(),
            ],
            Self::FileSystemAccessDenied { path } => vec![
                format!("Check permissions: ls -la '{}'", path),
                "Ensure you have read access to the directory".to_string(),
                "Try running from a directory you own".to_string(),
                "Use sudo if appropriate (be careful with permissions)".to_string(),
            ],
            Self::FileSystemNotDirectory { path } => vec![
                format!("Use the parent directory of '{}'", path),
                "Specify a directory path, not a file path".to_string(),
                "Check that the path points to a directory".to_string(),
            ],
            Self::StdinReadFailed { .. } => vec![
                "Provide input via pipe: echo 'problem statement' | xchecker spec <id>".to_string(),
                "Use a different source: --source fs --repo /path/to/project".to_string(),
                "Check that stdin is not closed or redirected incorrectly".to_string(),
            ],
            Self::EmptyInput => vec![
                "Provide a problem statement via stdin".to_string(),
                "Use --source fs --repo <path> to read from filesystem".to_string(),
                "Use --source gh --gh owner/repo to read from GitHub issue".to_string(),
                "Example: echo 'Build a web API for user management' | xchecker spec my-api"
                    .to_string(),
            ],
            Self::InvalidFormat { .. } => vec![
                "Provide a clear, single-line problem statement".to_string(),
                "Describe what you want to build in plain English".to_string(),
                "Example: 'Build a REST API for managing user accounts'".to_string(),
                "Avoid complex formatting or multiple unrelated requests".to_string(),
            ],
        }
    }

    fn category(&self) -> ErrorCategory {
        ErrorCategory::Configuration
    }
}

/// Claude CLI integration errors
#[derive(Error, Debug)]
pub enum ClaudeError {
    #[error("Claude CLI not found or not executable")]
    NotFound,

    #[error("Claude CLI version incompatible: {version}")]
    IncompatibleVersion { version: String },

    #[error("Claude CLI execution failed: {stderr}")]
    ExecutionFailed { stderr: String },

    #[error("Failed to parse Claude CLI output: {reason}")]
    ParseError { reason: String },

    #[error("Model '{model}' not available")]
    ModelNotAvailable { model: String },

    #[error("Authentication failed: {reason}")]
    AuthenticationFailed { reason: String },
}

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

/// Runner execution errors for cross-platform Claude CLI execution
#[derive(Error, Debug)]
pub enum RunnerError {
    #[error("Runner detection failed: {reason}")]
    DetectionFailed { reason: String },

    #[error("WSL not available: {reason}")]
    WslNotAvailable { reason: String },

    #[error("WSL execution failed: {reason}")]
    WslExecutionFailed { reason: String },

    #[error("Native execution failed: {reason}")]
    NativeExecutionFailed { reason: String },

    #[error("Runner configuration invalid: {reason}")]
    ConfigurationInvalid { reason: String },

    #[error("Claude CLI not found in runner environment: {runner}")]
    ClaudeNotFoundInRunner { runner: String },

    #[error("Execution timed out after {timeout_seconds} seconds")]
    Timeout { timeout_seconds: u64 },
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

impl UserFriendlyError for crate::llm::LlmError {
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
        }
    }
}

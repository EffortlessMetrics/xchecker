//! Error reporting utilities for user-friendly error display
//!
//! This module provides structured error reporting that transforms technical
//! errors into user-friendly messages with context and actionable suggestions.

use std::fmt;
use xchecker_redaction::SecretRedactor;
use xchecker_utils::error::{ErrorCategory, UserFriendlyError, XCheckerError};
use xchecker_utils::exit_codes;

/// Structured error report with user-friendly formatting
pub struct ErrorReport<'a> {
    error: &'a dyn UserFriendlyError,
    show_suggestions: bool,
    show_context: bool,
}

impl<'a> ErrorReport<'a> {
    /// Create a new error report
    pub fn new(error: &'a dyn UserFriendlyError) -> Self {
        Self {
            error,
            show_suggestions: true,
            show_context: true,
        }
    }

    /// Create a minimal error report with just the message
    #[allow(dead_code)] // Alternative error display API
    pub fn minimal(error: &'a dyn UserFriendlyError) -> Self {
        Self {
            error,
            show_suggestions: false,
            show_context: false,
        }
    }

    /// Enable or disable suggestion display
    #[must_use]
    pub const fn with_suggestions(mut self, show: bool) -> Self {
        self.show_suggestions = show;
        self
    }

    /// Enable or disable context display
    #[must_use]
    #[allow(dead_code)] // Builder pattern method for error display
    pub const fn with_context(mut self, show: bool) -> Self {
        self.show_context = show;
        self
    }

    /// Format the error for display
    #[must_use]
    pub fn format(&self) -> String {
        xchecker_redaction::default_redactor().redact_string(&self.format_inner())
    }

    /// Format the error for display using a caller-provided redactor (FR-SEC-19).
    #[must_use]
    pub fn format_with_redactor(&self, redactor: &SecretRedactor) -> String {
        redactor.redact_string(&self.format_inner())
    }

    fn format_inner(&self) -> String {
        let mut output = String::new();

        // Error header with category
        let category = self.error.category();
        output.push_str(&format!("✗ {}: {}\n", category, self.error.user_message()));

        // Context information
        if self.show_context
            && let Some(context) = self.error.context()
        {
            output.push_str(&format!("\n  Context: {context}\n"));
        }

        // Suggestions
        if self.show_suggestions {
            let suggestions = self.error.suggestions();
            if !suggestions.is_empty() {
                output.push_str("\n  Suggestions:\n");
                for (i, suggestion) in suggestions.iter().enumerate() {
                    output.push_str(&format!("    {}. {}\n", i + 1, suggestion));
                }
            }
        }

        // Add troubleshooting footer for certain error categories
        match self.error.category() {
            ErrorCategory::Configuration => {
                output.push_str("\n  For more help:\n");
                output.push_str("    - Run 'xchecker --help' for usage information\n");
                output.push_str("    - Check the documentation for configuration examples\n");
            }
            ErrorCategory::ClaudeIntegration => {
                output.push_str("\n  Claude CLI troubleshooting:\n");
                output.push_str("    - Verify installation: claude --version\n");
                output.push_str("    - Check authentication: claude auth status\n");
            }
            ErrorCategory::PhaseExecution => {
                output.push_str("\n  Phase execution help:\n");
                output.push_str("    - Check status: xchecker status <id>\n");
                output.push_str("    - View partial outputs in .xchecker/specs/<id>/artifacts/\n");
            }
            _ => {}
        }

        output
    }

    /// Print the error to stderr
    pub fn print_to_stderr(&self) {
        eprintln!("{}", self.format());
    }

    /// Print the error to stdout
    #[allow(dead_code)] // Alternative output method
    pub fn print_to_stdout(&self) {
        println!("{}", self.format());
    }
}

impl fmt::Display for ErrorReport<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format())
    }
}

/// ErrorReporter with centralized exit handling
pub struct ErrorReporter;

impl ErrorReporter {
    /// Report error and exit with appropriate code
    pub fn report_and_exit(error: &XCheckerError) -> ! {
        let (code, _kind) = exit_codes::error_to_exit_code_and_kind(error);

        let report = ErrorReport::new(error).with_suggestions(true);
        report.print_to_stderr();

        std::process::exit(code);
    }

    /// Report error without exiting
    #[allow(dead_code)] // Alternative error reporting API
    pub fn report_error(error: &XCheckerError) {
        let (code, kind) = exit_codes::error_to_exit_code_and_kind(error);

        let report = ErrorReport::new(error)
            .with_context(true)
            .with_suggestions(true);
        report.print_to_stderr();

        tracing::debug!(%code, ?kind, "reported error without exiting");
    }

    /// Report minimal error
    #[allow(dead_code)] // Alternative error reporting API
    pub fn report_minimal(error: &XCheckerError) {
        let (_code, _kind) = exit_codes::error_to_exit_code_and_kind(error);
        let report = ErrorReport::minimal(error);
        report.print_to_stderr();
    }

    /// Check if error should show details
    #[allow(dead_code)] // Error reporting utility
    pub fn should_show_details(_error: &XCheckerError) -> bool {
        // Show details for most errors
        true
    }
}

/// Utility functions for common error reporting patterns
pub mod utils {
    use super::{ErrorCategory, ErrorReport, UserFriendlyError};
    use xchecker_redaction::SecretRedactor;
    use xchecker_utils::error::XCheckerError;

    /// Report an error and exit with appropriate code
    #[allow(dead_code)] // Alternative error reporting utility
    pub fn report_and_exit(error: &XCheckerError, exit_code: Option<i32>) -> ! {
        let report = ErrorReport::new(error);
        report.print_to_stderr();

        let code = exit_code.unwrap_or_else(|| match error.category() {
            ErrorCategory::Configuration => 2,
            ErrorCategory::PhaseExecution => 3,
            ErrorCategory::ClaudeIntegration => 4,
            ErrorCategory::FileSystem => 5,
            ErrorCategory::Security => 6,
            ErrorCategory::ResourceLimits => 7,
            ErrorCategory::Concurrency => 9,
            ErrorCategory::Validation => 8,
        });

        std::process::exit(code);
    }

    /// Report an error without exiting
    #[allow(dead_code)] // Alternative error reporting utility
    pub fn report_error(error: &XCheckerError) {
        let report = ErrorReport::new(error);
        report.print_to_stderr();
    }

    /// Report an error with minimal formatting
    #[allow(dead_code)] // Alternative error reporting utility
    pub fn report_minimal(error: &XCheckerError) {
        let report = ErrorReport::minimal(error);
        report.print_to_stderr();
    }

    /// Check if an error should be reported with full details
    #[must_use]
    #[allow(dead_code)] // Error reporting utility
    pub fn should_show_details(error: &XCheckerError) -> bool {
        matches!(
            error.category(),
            ErrorCategory::Configuration
                | ErrorCategory::ClaudeIntegration
                | ErrorCategory::PhaseExecution
        )
    }

    /// Provide enhanced error context for common failure scenarios (R1.3, R6.4)
    #[must_use]
    pub fn enhance_error_context(error: &XCheckerError) -> Vec<String> {
        match error {
            XCheckerError::Config(config_err) => match config_err {
                xchecker_utils::error::ConfigError::MissingRequired(key) => {
                    if key.contains("gh") {
                        vec![
                            "GitHub source requires: --source gh --gh owner/repo".to_string(),
                            "Example: xchecker spec my-api --source gh --gh anthropic/claude-cli"
                                .to_string(),
                            "Ensure you have GitHub CLI installed and authenticated".to_string(),
                        ]
                    } else if key.contains("repo") {
                        vec![
                            "Filesystem source requires: --source fs --repo /path/to/directory"
                                .to_string(),
                            "Example: xchecker spec my-api --source fs --repo ./my-project"
                                .to_string(),
                            "Ensure the directory exists and is readable".to_string(),
                        ]
                    } else if key.contains("model") {
                        vec![
                            "Specify a Claude model: --model haiku".to_string(),
                            "Check available models with: claude models".to_string(),
                            "Add model to config file [defaults] section".to_string(),
                        ]
                    } else {
                        vec![
                            "Check the documentation for required configuration options"
                                .to_string(),
                            "Use CLI flags as a temporary workaround".to_string(),
                            "Review .xchecker/config.toml for missing values".to_string(),
                        ]
                    }
                }
                xchecker_utils::error::ConfigError::InvalidValue { key, .. } => {
                    if key == "source" {
                        vec![
                                "Valid source types: 'gh' (GitHub), 'fs' (filesystem), 'stdin' (default)".to_string(),
                                "GitHub: --source gh --gh owner/repo".to_string(),
                                "Filesystem: --source fs --repo /path/to/project".to_string(),
                                "Stdin: --source stdin (or omit --source)".to_string(),
                            ]
                    } else if key == "spec_id" {
                        vec![
                            "Use alphanumeric characters, hyphens, and underscores only"
                                .to_string(),
                            "Keep spec ID under 100 characters".to_string(),
                            "Avoid filesystem-reserved characters: / \\ : * ? \" < > |".to_string(),
                            "Example: my-api-spec, user-auth-v2, payment-system".to_string(),
                        ]
                    } else {
                        vec!["Check the documentation for valid values for this configuration option".to_string()]
                    }
                }
                xchecker_utils::error::ConfigError::DiscoveryFailed { .. } => vec![
                    "Create .xchecker/config.toml in your project root".to_string(),
                    "Check directory permissions for config discovery".to_string(),
                    "Use --config <path> to specify configuration file explicitly".to_string(),
                    "Ensure you're running from within a project directory".to_string(),
                ],
                xchecker_utils::error::ConfigError::ValidationFailed { .. } => vec![
                    "Review TOML syntax in .xchecker/config.toml".to_string(),
                    "Ensure all required sections are present: [defaults], [selectors]".to_string(),
                    "Check for typos in configuration keys".to_string(),
                    "Validate TOML format using an online validator".to_string(),
                ],
                _ => vec![],
            },
            XCheckerError::Source(source_err) => match source_err {
                xchecker_utils::error::SourceError::EmptyInput => vec![
                    "Provide input via: echo 'Build a REST API' | xchecker spec my-api".to_string(),
                    "Or use file source: --source fs --repo /path/to/project".to_string(),
                    "Or use GitHub: --source gh --gh owner/repo".to_string(),
                ],
                xchecker_utils::error::SourceError::GitHubRepoNotFound { .. } => vec![
                    "Verify the repository exists and is accessible".to_string(),
                    "Check your GitHub authentication if it's a private repo".to_string(),
                    "Try using a public repository first to test".to_string(),
                ],
                _ => vec![],
            },
            XCheckerError::Phase(phase_err) => {
                match phase_err {
                    xchecker_utils::error::PhaseError::ExecutionFailed { phase, code } => {
                        let mut suggestions = vec![
                            format!(
                                "Check partial outputs: .xchecker/specs/<id>/artifacts/*-{}.partial.md",
                                phase.to_lowercase()
                            ),
                            "Review the receipt file for detailed error information".to_string(),
                            "Try running with --dry-run to test configuration".to_string(),
                        ];

                        // Add phase-specific guidance
                        match phase.as_str() {
                            "REQUIREMENTS" => {
                                suggestions.extend(vec![
                                    "Ensure your problem statement is clear and specific".to_string(),
                                    "Try simplifying the requirements scope".to_string(),
                                    "Check that input is provided via stdin, --source fs, or --source gh".to_string(),
                                ]);
                            }
                            "DESIGN" => {
                                suggestions.extend(vec![
                                    "Verify the requirements document is complete and well-formed".to_string(),
                                    "Check if the design complexity is appropriate for the requirements".to_string(),
                                    "Ensure requirements.core.yaml exists and is valid".to_string(),
                                ]);
                            }
                            "TASKS" => {
                                suggestions.extend(vec![
                                    "Verify the design document is complete and detailed"
                                        .to_string(),
                                    "Check that design.core.yaml exists and is valid".to_string(),
                                    "Consider breaking down complex design elements".to_string(),
                                ]);
                            }
                            _ => {}
                        }

                        // Add exit code specific guidance
                        match *code {
                            1 => suggestions.push(
                                "General execution failure - check Claude CLI authentication"
                                    .to_string(),
                            ),
                            2 => suggestions.push(
                                "Invalid arguments - check Claude CLI version compatibility"
                                    .to_string(),
                            ),
                            126 => suggestions.push(
                                "Permission denied - check Claude CLI executable permissions"
                                    .to_string(),
                            ),
                            127 => suggestions.push(
                                "Command not found - ensure Claude CLI is installed and in PATH"
                                    .to_string(),
                            ),
                            _ => {}
                        }

                        suggestions
                    }
                    xchecker_utils::error::PhaseError::ExecutionFailedWithStderr {
                        phase,
                        stderr_tail,
                        ..
                    } => {
                        let mut suggestions = vec![
                            format!("Claude CLI stderr: {}", stderr_tail),
                            format!(
                                "Check partial outputs: .xchecker/specs/<id>/artifacts/*-{}.partial.md",
                                phase.to_lowercase()
                            ),
                            "Review the receipt file for complete stderr output (≤2 KiB)"
                                .to_string(),
                        ];

                        // Parse stderr for common issues
                        if stderr_tail.contains("authentication") || stderr_tail.contains("auth") {
                            suggestions.extend(vec![
                                "Authentication issue detected - run: claude auth login"
                                    .to_string(),
                                "Check your Claude API key is valid and not expired".to_string(),
                                "Verify your Claude account has API access".to_string(),
                            ]);
                        } else if stderr_tail.contains("rate limit") || stderr_tail.contains("429")
                        {
                            suggestions.extend(vec![
                                "Rate limit exceeded - wait a few minutes and try again"
                                    .to_string(),
                                "Consider upgrading your Claude subscription for higher limits"
                                    .to_string(),
                            ]);
                        } else if stderr_tail.contains("network")
                            || stderr_tail.contains("connection")
                        {
                            suggestions.extend(vec![
                                "Network connectivity issue - check your internet connection"
                                    .to_string(),
                                "Try again in a few minutes if this is a temporary network issue"
                                    .to_string(),
                            ]);
                        } else if stderr_tail.contains("model") {
                            suggestions.extend(vec![
                                "Model access issue - check available models with: claude models"
                                    .to_string(),
                                "Try using a different model with --model flag".to_string(),
                                "Verify your subscription includes access to the requested model"
                                    .to_string(),
                            ]);
                        } else {
                            suggestions.extend(vec![
                                "Verify Claude CLI authentication and connectivity".to_string(),
                                "Try running with --dry-run to test configuration".to_string(),
                            ]);
                        }

                        suggestions
                    }
                    xchecker_utils::error::PhaseError::PartialOutputSaved {
                        phase,
                        partial_path,
                    } => vec![
                        format!("Partial output saved to: {}", partial_path),
                        format!(
                            "Review the partial content to understand where {} phase failed",
                            phase
                        ),
                        "Check the receipt file for stderr output and warnings".to_string(),
                        "Use partial output to debug and fix the issue before retrying".to_string(),
                        "Look for incomplete sections or truncated content in the partial output"
                            .to_string(),
                        "Check if the failure occurred during output generation or validation"
                            .to_string(),
                    ],
                    xchecker_utils::error::PhaseError::DependencyNotSatisfied { dependency, phase } => vec![
                        format!(
                            "Complete the {} phase first before running {}",
                            dependency, phase
                        ),
                        format!("Check status: xchecker status <id>"),
                        "Ensure previous phases completed successfully (exit code 0)".to_string(),
                        format!(
                            "Run: xchecker spec <id> to execute from {} phase",
                            dependency
                        ),
                    ],
                    xchecker_utils::error::PhaseError::PacketCreationFailed { phase, reason } => {
                        let mut suggestions = vec![
                            format!("Packet creation failed for {} phase: {}", phase, reason),
                            "Check that all required input files are accessible".to_string(),
                        ];

                        if reason.contains("size") || reason.contains("limit") {
                            suggestions.extend(vec![
                                "Packet size exceeded limits - reduce content or increase limits".to_string(),
                                "Configure packet_max_bytes and packet_max_lines in .xchecker/config.toml".to_string(),
                                "Use more restrictive include/exclude patterns".to_string(),
                            ]);
                        } else if reason.contains("secret") {
                            suggestions.extend(vec![
                                "Secret detected in input files - remove or redact sensitive data"
                                    .to_string(),
                                "Use --ignore-secret-pattern <regex> to suppress specific patterns"
                                    .to_string(),
                                "Add sensitive files to .gitignore".to_string(),
                            ]);
                        } else {
                            suggestions.extend(vec![
                                "Verify packet size limits are not exceeded".to_string(),
                                "Ensure no secrets are detected in the input files".to_string(),
                            ]);
                        }

                        suggestions
                    }
                    xchecker_utils::error::PhaseError::ContextCreationFailed { phase, reason } => vec![
                        format!("Context creation failed for {} phase: {}", phase, reason),
                        "Check that the spec directory is accessible and writable".to_string(),
                        "Verify configuration values are valid".to_string(),
                        "Ensure previous phase artifacts are available if required".to_string(),
                        "Check file permissions in .xchecker/specs/<id>/ directory".to_string(),
                        "Verify sufficient disk space is available".to_string(),
                    ],
                    xchecker_utils::error::PhaseError::Timeout {
                        phase,
                        timeout_seconds,
                    } => vec![
                        format!(
                            "The {} phase timed out after {} seconds",
                            phase, timeout_seconds
                        ),
                        "Increase timeout in configuration if needed".to_string(),
                        "Check your internet connection for Claude API calls".to_string(),
                        "Try breaking down complex requests into smaller parts".to_string(),
                        "Run with --verbose to see where execution is hanging".to_string(),
                    ],
                    xchecker_utils::error::PhaseError::ResourceLimitExceeded {
                        phase,
                        resource,
                        limit,
                    } => {
                        let mut suggestions = vec![format!(
                            "The {} phase exceeded {} limit: {}",
                            phase, resource, limit
                        )];

                        match resource.as_str() {
                            "memory" => suggestions.extend(vec![
                                "Reduce packet size limits in configuration".to_string(),
                                "Use more restrictive file include patterns".to_string(),
                                "Process files in smaller batches".to_string(),
                            ]),
                            "disk" => suggestions.extend(vec![
                                "Free up disk space".to_string(),
                                "Clean old spec artifacts with: xchecker clean <id>".to_string(),
                                "Check available disk space with df -h".to_string(),
                            ]),
                            _ => suggestions.push(
                                "Check system resources and configuration limits".to_string(),
                            ),
                        }

                        suggestions
                    }
                    _ => vec![],
                }
            }
            XCheckerError::Claude(claude_err) => match claude_err {
                xchecker_utils::error::ClaudeError::NotFound => vec![
                    "Install Claude CLI from: https://claude.ai/cli".to_string(),
                    "Ensure 'claude' is in your PATH".to_string(),
                    "Test installation with: claude --version".to_string(),
                    "On Windows, try WSL if native installation fails".to_string(),
                    "Restart your terminal after installation".to_string(),
                ],
                xchecker_utils::error::ClaudeError::AuthenticationFailed { reason } => {
                    let mut suggestions = vec![
                        "Authenticate with: claude auth login".to_string(),
                        "Check your API key is valid and not expired".to_string(),
                        "Verify your Claude account has API access".to_string(),
                    ];

                    if reason.contains("token") || reason.contains("key") {
                        suggestions.extend(vec![
                            "Your API token may be invalid or expired".to_string(),
                            "Generate a new API key from your Claude account".to_string(),
                        ]);
                    } else if reason.contains("subscription") || reason.contains("plan") {
                        suggestions.extend(vec![
                            "Your Claude subscription may not include API access".to_string(),
                            "Check your Claude account billing and subscription status".to_string(),
                        ]);
                    }

                    suggestions.push(
                        "Try logging out and back in: claude auth logout && claude auth login"
                            .to_string(),
                    );
                    suggestions
                }
                xchecker_utils::error::ClaudeError::ExecutionFailed { stderr } => {
                    let mut suggestions = vec![format!("Claude CLI error: {}", stderr)];

                    if stderr.contains("rate limit") || stderr.contains("429") {
                        suggestions.extend(vec![
                            "Rate limit exceeded - wait a few minutes and try again".to_string(),
                            "Consider upgrading your Claude subscription for higher limits"
                                .to_string(),
                            "Authenticate to get higher rate limits".to_string(),
                        ]);
                    } else if stderr.contains("network")
                        || stderr.contains("connection")
                        || stderr.contains("timeout")
                    {
                        suggestions.extend(vec![
                            "Check your internet connection".to_string(),
                            "Try again in a few minutes if this is a temporary network issue"
                                .to_string(),
                            "Check if you're behind a firewall or proxy".to_string(),
                        ]);
                    } else if stderr.contains("authentication") || stderr.contains("unauthorized") {
                        suggestions.extend(vec![
                            "Authentication issue - run: claude auth status".to_string(),
                            "Re-authenticate with: claude auth login".to_string(),
                        ]);
                    } else {
                        suggestions.extend(vec![
                            "Check your internet connection".to_string(),
                            "Verify Claude CLI authentication with 'claude auth status'"
                                .to_string(),
                            "Check if you've exceeded API rate limits".to_string(),
                        ]);
                    }

                    suggestions
                }
                xchecker_utils::error::ClaudeError::ModelNotAvailable { model } => vec![
                    format!("Model '{}' is not available or accessible", model),
                    "Check available models with: claude models".to_string(),
                    "Verify your Claude subscription includes access to this model".to_string(),
                    "Try using a different model with --model flag".to_string(),
                    "Use the full model name instead of an alias".to_string(),
                ],
                xchecker_utils::error::ClaudeError::ParseError { reason } => vec![
                    format!("Could not parse Claude CLI response: {}", reason),
                    "This may be a temporary issue - try running the command again".to_string(),
                    "Check if Claude CLI output format has changed".to_string(),
                    "Verify Claude CLI version compatibility".to_string(),
                    "Report this issue if it persists".to_string(),
                ],
                xchecker_utils::error::ClaudeError::IncompatibleVersion { version } => vec![
                    format!("Claude CLI version {} is not compatible", version),
                    "Update Claude CLI to the latest version".to_string(),
                    "Check xchecker documentation for supported Claude CLI versions".to_string(),
                    "Use 'claude --version' to check your current version".to_string(),
                ],
            },
            XCheckerError::Runner(runner_err) => match runner_err {
                xchecker_utils::error::RunnerError::DetectionFailed { reason } => vec![
                    format!("Runner detection failed: {}", reason),
                    "Try specifying runner mode explicitly: --runner native or --runner wsl"
                        .to_string(),
                    "Ensure Claude CLI is installed and accessible".to_string(),
                    "Check that 'claude --version' works in your environment".to_string(),
                ],
                xchecker_utils::error::RunnerError::WslNotAvailable { .. } => vec![
                    "Install WSL: wsl --install".to_string(),
                    "Use native runner mode if Claude CLI is available on Windows".to_string(),
                    "Check WSL status: wsl --status".to_string(),
                    "Restart Windows after WSL installation if needed".to_string(),
                ],
                xchecker_utils::error::RunnerError::WslExecutionFailed { reason } => {
                    let mut suggestions = vec![
                        format!("WSL execution failed: {}", reason),
                        "Check that WSL is running: wsl --status".to_string(),
                    ];

                    if reason.contains("claude") {
                        suggestions.extend(vec![
                            "Install Claude CLI in WSL: wsl -e pip install claude-cli".to_string(),
                            "Verify Claude CLI is accessible: wsl -e claude --version".to_string(),
                        ]);
                    } else {
                        suggestions.extend(vec![
                            "Try restarting WSL: wsl --shutdown && wsl".to_string(),
                            "Use native runner mode as alternative".to_string(),
                        ]);
                    }

                    suggestions
                }
                xchecker_utils::error::RunnerError::NativeExecutionFailed { .. } => vec![
                    "Install Claude CLI for your platform".to_string(),
                    "Ensure 'claude' is in your PATH".to_string(),
                    "Try WSL runner mode on Windows: --runner wsl".to_string(),
                    "Restart your terminal after Claude CLI installation".to_string(),
                ],
                xchecker_utils::error::RunnerError::ClaudeNotFoundInRunner { runner } => {
                    match runner.as_str() {
                            "wsl" => vec![
                                "Install Claude CLI in WSL: wsl -e pip install claude-cli".to_string(),
                                "Check WSL PATH: wsl -e echo $PATH".to_string(),
                                "Specify claude_path in configuration if installed in non-standard location".to_string(),
                            ],
                            "native" => vec![
                                "Install Claude CLI for your platform".to_string(),
                                "Add Claude CLI to your PATH".to_string(),
                                "Test with: claude --version".to_string(),
                            ],
                            _ => vec![
                                "Install Claude CLI in the specified runner environment".to_string(),
                                "Check that Claude CLI is accessible and executable".to_string(),
                            ]
                        }
                }
                _ => vec![],
            },
            XCheckerError::SecretDetected { pattern, location } => vec![
                format!(
                    "Security issue: Detected potential secret pattern '{}' in {}",
                    pattern, location
                ),
                "Remove or redact the sensitive data from the file".to_string(),
                "Use --ignore-secret-pattern <pattern-id> to suppress this specific pattern"
                    .to_string(),
                "Add the file to .gitignore if it contains test data".to_string(),
                "Consider using environment variables for secrets instead".to_string(),
            ],
            XCheckerError::PacketOverflow {
                used_bytes,
                used_lines,
                limit_bytes,
                limit_lines,
            } => vec![
                format!(
                    "Packet size exceeded: {} bytes/{} lines used, {} bytes/{} lines allowed",
                    used_bytes, used_lines, limit_bytes, limit_lines
                ),
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
                "Review file selection patterns in .xchecker/config.toml".to_string(),
            ],
            XCheckerError::ConcurrentExecution { id } => vec![
                format!(
                    "Another xchecker process is already working on spec '{}'",
                    id
                ),
                format!(
                    "Wait for the other process to complete or check: xchecker status {}",
                    id
                ),
                "Use --force flag to override the lock (use with caution)".to_string(),
                "Check if a previous process crashed and left a stale lock".to_string(),
                "Look for .xchecker/specs/{}/lock file and remove if stale".to_string(),
            ],
            XCheckerError::CanonicalizationFailed { phase, reason } => vec![
                format!(
                    "Failed to normalize output from {} phase: {}",
                    phase, reason
                ),
                "Check that the output format matches expected structure".to_string(),
                "Verify YAML syntax if the error involves YAML canonicalization".to_string(),
                "Review the phase output for formatting issues".to_string(),
                "Check for malformed markdown or YAML in the output".to_string(),
            ],
            XCheckerError::ModelResolutionError { alias, reason, .. } => vec![
                format!("Could not resolve model '{}': {}", alias, reason),
                "Check that the Claude CLI is properly installed and authenticated".to_string(),
                "Verify the model name is correct and available".to_string(),
                "Try using the full model name instead of an alias".to_string(),
                "Check available models with: claude models".to_string(),
            ],
            _ => vec![],
        }
    }

    /// Create a comprehensive error report with enhanced context (R1.3, R6.4)
    #[must_use]
    fn create_comprehensive_report_inner(error: &XCheckerError) -> String {
        let mut output = String::new();

        // Main error report
        let report = ErrorReport::new(error);
        output.push_str(&report.format_inner());

        // Enhanced context
        let enhanced_context = enhance_error_context(error);
        if !enhanced_context.is_empty() {
            output.push_str("\n  Enhanced guidance:\n");
            for (i, context) in enhanced_context.iter().enumerate() {
                output.push_str(&format!("    {}. {}\n", i + 1, context));
            }
        }

        // Category-specific troubleshooting
        match error.category() {
            ErrorCategory::Configuration => {
                output.push_str("\n  Configuration troubleshooting:\n");
                output.push_str("    - Check .xchecker/config.toml syntax and structure\n");
                output.push_str("    - Verify all required sections are present\n");
                output.push_str("    - Use CLI flags to override configuration temporarily\n");
                output.push_str("    - Run 'xchecker --help' for usage information\n");
            }
            ErrorCategory::ClaudeIntegration => {
                output.push_str("\n  Claude CLI troubleshooting:\n");
                output.push_str("    - Verify installation: claude --version\n");
                output.push_str("    - Check authentication: claude auth status\n");
                output.push_str("    - Test connectivity: claude models\n");
                output.push_str("    - Check API rate limits and subscription status\n");
            }
            ErrorCategory::PhaseExecution => {
                output.push_str("\n  Phase execution troubleshooting:\n");
                output.push_str("    - Check status: xchecker status <id>\n");
                output.push_str("    - View partial outputs in .xchecker/specs/<id>/artifacts/\n");
                output.push_str("    - Review receipt files for detailed execution logs\n");
                output.push_str("    - Try --dry-run to test configuration without Claude calls\n");
            }
            ErrorCategory::Security => {
                output.push_str("\n  Security troubleshooting:\n");
                output.push_str("    - Review files for potential secrets or sensitive data\n");
                output.push_str("    - Use .gitignore to exclude sensitive files\n");
                output
                    .push_str("    - Consider using --ignore-secret-pattern for false positives\n");
                output.push_str("    - Ensure test data doesn't contain real credentials\n");
            }
            ErrorCategory::FileSystem => {
                output.push_str("\n  File system troubleshooting:\n");
                output.push_str("    - Check file and directory permissions\n");
                output.push_str("    - Verify sufficient disk space is available\n");
                output.push_str("    - Ensure the working directory is writable\n");
                output.push_str("    - Check for filesystem-specific limitations\n");
            }
            _ => {
                output.push_str("\n  General troubleshooting:\n");
                output.push_str("    - Run with --verbose for detailed output\n");
                output.push_str("    - Check xchecker documentation for common issues\n");
                output.push_str("    - Ensure all dependencies are properly installed\n");
            }
        }

        // Add recovery suggestions based on error type
        if matches!(error.category(), ErrorCategory::PhaseExecution) {
            output.push_str("\n  Recovery options:\n");
            output
                .push_str("    - Resume from failed phase: xchecker resume <id> --phase <name>\n");
            output.push_str("    - Start over: xchecker clean <id> && xchecker spec <id>\n");
            output.push_str("    - Check partial outputs for debugging information\n");
        }

        output
    }

    /// Create a comprehensive error report with enhanced context (R1.3, R6.4)
    #[must_use]
    pub fn create_comprehensive_report(error: &XCheckerError) -> String {
        xchecker_redaction::default_redactor()
            .redact_string(&create_comprehensive_report_inner(error))
    }

    /// Create a comprehensive error report with enhanced context, applying a caller-provided redactor
    /// as a final safety net (FR-SEC-19).
    #[must_use]
    pub fn create_comprehensive_report_with_redactor(
        error: &XCheckerError,
        redactor: &SecretRedactor,
    ) -> String {
        redactor.redact_string(&create_comprehensive_report_inner(error))
    }

    /// Provide contextual help and suggestions based on the current operation (R1.3, R6.4)
    #[must_use]
    pub fn provide_contextual_help(operation: &str, error: &XCheckerError) -> Vec<String> {
        let mut help = Vec::new();

        match operation {
            "spec" => {
                help.push("Spec generation workflow:".to_string());
                help.push("  1. Requirements phase - analyzes problem statement".to_string());
                help.push("  2. Design phase - creates architecture from requirements".to_string());
                help.push("  3. Tasks phase - generates implementation plan".to_string());

                match error.category() {
                    ErrorCategory::Configuration => {
                        help.push("\nFor spec generation, ensure:".to_string());
                        help.push(
                            "  - Input is provided via stdin, --source fs, or --source gh"
                                .to_string(),
                        );
                        help.push("  - Claude CLI is installed and authenticated".to_string());
                        help.push(
                            "  - Spec ID is valid (alphanumeric, hyphens, underscores)".to_string(),
                        );
                    }
                    ErrorCategory::PhaseExecution => {
                        help.push("\nPhase execution tips:".to_string());
                        help.push("  - Each phase builds on the previous one".to_string());
                        help.push(
                            "  - Partial outputs are saved on failure for debugging".to_string(),
                        );
                        help.push("  - Use --dry-run to test without Claude API calls".to_string());
                    }
                    _ => {}
                }
            }
            "status" => {
                help.push("Status command shows:".to_string());
                help.push("  - Latest completed phase and exit code".to_string());
                help.push("  - List of artifacts with BLAKE3 hashes".to_string());
                help.push("  - Last receipt path and effective configuration".to_string());

                if matches!(error.category(), ErrorCategory::FileSystem) {
                    help.push("\nFor status command, ensure:".to_string());
                    help.push("  - Spec directory exists: .xchecker/specs/<id>/".to_string());
                    help.push("  - You have read permissions to the spec directory".to_string());
                }
            }
            "resume" => {
                help.push("Resume command allows:".to_string());
                help.push("  - Restarting from a specific phase".to_string());
                help.push("  - Continuing after fixing configuration issues".to_string());
                help.push("  - Recovering from partial failures".to_string());

                if matches!(error.category(), ErrorCategory::PhaseExecution) {
                    help.push("\nFor resume, check:".to_string());
                    help.push("  - Previous phases completed successfully".to_string());
                    help.push("  - Required artifacts exist for the target phase".to_string());
                }
            }
            "clean" => {
                help.push("Clean command removes:".to_string());
                help.push("  - All artifacts for the specified spec".to_string());
                help.push("  - Receipts and execution history".to_string());
                help.push("  - Context files and temporary data".to_string());

                if matches!(error.category(), ErrorCategory::FileSystem) {
                    help.push("\nFor clean command, ensure:".to_string());
                    help.push(
                        "  - You have write permissions to .xchecker/specs/<id>/".to_string(),
                    );
                    help.push("  - No other xchecker process is using the spec".to_string());
                }
            }
            _ => {
                help.push("General xchecker usage:".to_string());
                help.push("  - spec <id>: Generate complete specification".to_string());
                help.push("  - status <id>: Check current state".to_string());
                help.push("  - resume <id> --phase <name>: Continue from phase".to_string());
                help.push("  - clean <id>: Remove all artifacts".to_string());
            }
        }

        help
    }

    /// Create an error report with operation-specific context (R1.3, R6.4)
    #[must_use]
    fn create_contextual_report_inner(error: &XCheckerError, operation: &str) -> String {
        let mut output = create_comprehensive_report_inner(error);

        let contextual_help = provide_contextual_help(operation, error);
        if !contextual_help.is_empty() {
            output.push_str("\n  Operation-specific help:\n");
            for help_line in contextual_help {
                if help_line.starts_with("  ") {
                    output.push_str(&format!("  {help_line}\n"));
                } else {
                    output.push_str(&format!("    {help_line}\n"));
                }
            }
        }

        output
    }

    /// Create an error report with operation-specific context (R1.3, R6.4)
    #[must_use]
    pub fn create_contextual_report(error: &XCheckerError, operation: &str) -> String {
        xchecker_redaction::default_redactor()
            .redact_string(&create_contextual_report_inner(error, operation))
    }

    /// Create an error report with operation-specific context (R1.3, R6.4), applying a caller-provided
    /// redactor as a final safety net (FR-SEC-19).
    #[must_use]
    pub fn create_contextual_report_with_redactor(
        error: &XCheckerError,
        operation: &str,
        redactor: &SecretRedactor,
    ) -> String {
        redactor.redact_string(&create_contextual_report_inner(error, operation))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xchecker_utils::error::{ConfigError, XCheckerError};

    #[test]
    fn test_error_report_formatting() {
        let config_error = ConfigError::MissingRequired("model".to_string());
        let error = XCheckerError::Config(config_error);

        let report = ErrorReport::new(&error);
        let formatted = report.format();

        assert!(formatted.contains("Configuration:"));
        assert!(formatted.contains("Required configuration 'model' is missing"));
        assert!(formatted.contains("Suggestions:"));
        assert!(formatted.contains("Add 'model' to the [defaults] section"));
    }

    #[test]
    fn test_minimal_error_report() {
        let config_error = ConfigError::MissingRequired("model".to_string());
        let error = XCheckerError::Config(config_error);

        let report = ErrorReport::minimal(&error);
        let formatted = report.format();

        assert!(formatted.contains("Configuration:"));
        assert!(formatted.contains("Required configuration 'model' is missing"));
        assert!(!formatted.contains("Suggestions:"));
        assert!(!formatted.contains("Context:"));
    }

    #[test]
    fn test_error_report_with_context_only() {
        let config_error = ConfigError::MissingRequired("model".to_string());
        let error = XCheckerError::Config(config_error);

        let report = ErrorReport::new(&error)
            .with_suggestions(false)
            .with_context(true);
        let formatted = report.format();

        assert!(formatted.contains("Configuration:"));
        assert!(formatted.contains("Context:"));
        assert!(!formatted.contains("Suggestions:"));
    }

    #[test]
    fn test_error_category_exit_codes() {
        use xchecker_utils::error::ErrorCategory;

        let config_error = XCheckerError::Config(ConfigError::MissingRequired("test".to_string()));
        assert_eq!(config_error.category(), ErrorCategory::Configuration);

        // Test that different categories would get different exit codes
        let exit_code = match config_error.category() {
            ErrorCategory::Configuration => 2,
            ErrorCategory::PhaseExecution => 3,
            ErrorCategory::ClaudeIntegration => 4,
            ErrorCategory::FileSystem => 5,
            ErrorCategory::Security => 6,
            ErrorCategory::ResourceLimits => 7,
            ErrorCategory::Concurrency => 9,
            ErrorCategory::Validation => 8,
        };

        assert_eq!(exit_code, 2);
    }

    #[test]
    fn test_enhanced_phase_error_reporting() {
        use xchecker_utils::error::{PhaseError, XCheckerError};

        // Test ExecutionFailedWithStderr error
        let phase_error = PhaseError::ExecutionFailedWithStderr {
            phase: "REQUIREMENTS".to_string(),
            code: 1,
            stderr_tail: "Authentication failed".to_string(),
        };
        let error = XCheckerError::Phase(phase_error);

        let enhanced_context = utils::enhance_error_context(&error);
        assert!(!enhanced_context.is_empty());
        assert!(enhanced_context[0].contains("Claude CLI stderr: Authentication failed"));

        // Test PartialOutputSaved error
        let phase_error = PhaseError::PartialOutputSaved {
            phase: "DESIGN".to_string(),
            partial_path: "artifacts/10-design.partial.md".to_string(),
        };
        let error = XCheckerError::Phase(phase_error);

        let enhanced_context = utils::enhance_error_context(&error);
        assert!(!enhanced_context.is_empty());
        assert!(
            enhanced_context[0].contains("Partial output saved to: artifacts/10-design.partial.md")
        );
    }

    #[test]
    fn test_comprehensive_error_report_with_enhanced_context() {
        use xchecker_utils::error::{PhaseError, XCheckerError};

        let phase_error = PhaseError::PacketCreationFailed {
            phase: "REQUIREMENTS".to_string(),
            reason: "File not found".to_string(),
        };
        let error = XCheckerError::Phase(phase_error);

        let comprehensive_report = utils::create_comprehensive_report(&error);

        assert!(comprehensive_report.contains("Phase Execution:"));
        assert!(comprehensive_report.contains("Enhanced guidance:"));
        assert!(comprehensive_report.contains("Packet creation failed for REQUIREMENTS phase"));
        assert!(comprehensive_report.contains("Phase execution troubleshooting:"));
    }

    #[test]
    fn test_contextual_help_for_spec_operation() {
        use xchecker_utils::error::{ConfigError, XCheckerError};

        let config_error = ConfigError::MissingRequired("model".to_string());
        let error = XCheckerError::Config(config_error);

        let help = utils::provide_contextual_help("spec", &error);

        assert!(!help.is_empty());
        assert!(help.iter().any(|h| h.contains("Spec generation workflow")));
        assert!(help.iter().any(|h| h.contains("Requirements phase")));
        assert!(help.iter().any(|h| h.contains("Claude CLI is installed")));
    }

    #[test]
    fn test_contextual_report_includes_operation_help() {
        use xchecker_utils::error::{PhaseError, XCheckerError};

        let phase_error = PhaseError::ExecutionFailed {
            phase: "DESIGN".to_string(),
            code: 1,
        };
        let error = XCheckerError::Phase(phase_error);

        let contextual_report = utils::create_contextual_report(&error, "spec");

        assert!(contextual_report.contains("Phase Execution:"));
        assert!(contextual_report.contains("Operation-specific help:"));
        assert!(contextual_report.contains("Spec generation workflow:"));
    }

    #[test]
    fn test_enhanced_phase_error_with_exit_code_guidance() {
        use xchecker_utils::error::{PhaseError, XCheckerError};

        let phase_error = PhaseError::ExecutionFailed {
            phase: "REQUIREMENTS".to_string(),
            code: 127, // Command not found
        };
        let error = XCheckerError::Phase(phase_error);

        let enhanced_context = utils::enhance_error_context(&error);

        assert!(!enhanced_context.is_empty());
        assert!(
            enhanced_context
                .iter()
                .any(|c| c.contains("Command not found"))
        );
        assert!(
            enhanced_context
                .iter()
                .any(|c| c.contains("Claude CLI is installed"))
        );
    }

    #[test]
    fn test_enhanced_claude_error_with_stderr_parsing() {
        use xchecker_utils::error::{ClaudeError, XCheckerError};

        let claude_error = ClaudeError::ExecutionFailed {
            stderr: "authentication failed: invalid token".to_string(),
        };
        let error = XCheckerError::Claude(claude_error);

        let enhanced_context = utils::enhance_error_context(&error);

        assert!(!enhanced_context.is_empty());
        assert!(
            enhanced_context
                .iter()
                .any(|c| c.contains("authentication failed"))
        );
        assert!(
            enhanced_context
                .iter()
                .any(|c| c.contains("claude auth login"))
        );
    }

    #[test]
    fn test_secret_detection_error_guidance() {
        use xchecker_utils::error::XCheckerError;

        let error = XCheckerError::SecretDetected {
            pattern: "ghp_".to_string(),
            location: "config.yaml:15".to_string(),
        };

        let enhanced_context = utils::enhance_error_context(&error);

        assert!(!enhanced_context.is_empty());
        assert!(
            enhanced_context
                .iter()
                .any(|c| c.contains("Remove or redact"))
        );
        assert!(
            enhanced_context
                .iter()
                .any(|c| c.contains("--ignore-secret-pattern"))
        );
    }

    #[test]
    fn test_packet_overflow_error_guidance() {
        use xchecker_utils::error::XCheckerError;

        let error = XCheckerError::PacketOverflow {
            used_bytes: 70000,
            used_lines: 1500,
            limit_bytes: 65536,
            limit_lines: 1200,
        };

        let enhanced_context = utils::enhance_error_context(&error);

        assert!(!enhanced_context.is_empty());
        assert!(
            enhanced_context
                .iter()
                .any(|c| c.contains("packet_max_bytes"))
        );
        assert!(
            enhanced_context
                .iter()
                .any(|c| c.contains("include/exclude patterns"))
        );
    }
}

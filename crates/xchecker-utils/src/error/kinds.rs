use std::path::PathBuf;
use std::time::Duration;
use thiserror::Error;

pub use xchecker_lock::LockError;

/// Library-level error type with rich context and user-friendly reporting.
///
/// `XCheckerError` is the primary error type returned by xchecker library operations.
/// It provides:
/// - Detailed error information for programmatic handling
/// - User-friendly messages with context and suggestions
/// - Mapping to CLI exit codes for consistent error reporting
///
/// # Error Categories
///
/// Errors are organized into categories for better handling:
///
/// | Category | Description |
/// |----------|-------------|
/// | `Config` | Configuration file or CLI argument errors |
/// | `Phase` | Phase execution failures |
/// | `Claude` | Claude CLI integration errors |
/// | `Runner` | Process execution errors |
/// | `SecretDetected` | Security: secrets found in content |
/// | `PacketOverflow` | Resource: packet size exceeded |
/// | `Lock` | Concurrency: lock already held |
///
/// # Exit Code Mapping
///
/// Use [`to_exit_code()`](Self::to_exit_code) to map errors to CLI exit codes:
///
/// | Exit Code | Error Type |
/// |-----------|------------|
/// | 2 | Configuration/CLI argument errors |
/// | 7 | Packet overflow |
/// | 8 | Secret detected |
/// | 9 | Lock held |
/// | 10 | Phase timeout |
/// | 70 | Claude CLI failure |
/// | 1 | Other errors |
///
/// # User-Friendly Messages
///
/// Use [`display_for_user()`](Self::display_for_user) to get formatted error messages
/// suitable for end users, including context and actionable suggestions.
///
/// # Example
///
/// ```rust
/// use xchecker_utils::error::XCheckerError;
/// use xchecker_utils::exit_codes::ExitCode;
///
/// fn handle_error(err: XCheckerError) {
///     // Get user-friendly message
///     eprintln!("{}", err.display_for_user());
///     
///     // Map to exit code for CLI
///     let code = err.to_exit_code();
///     std::process::exit(code.as_i32());
/// }
/// ```
///
/// # Library vs CLI Usage
///
/// - **Library consumers**: Handle `XCheckerError` directly, use `to_exit_code()` if needed
/// - **CLI**: Maps errors to exit codes and displays user-friendly messages
///
/// Library code returns `XCheckerError` and does NOT call `std::process::exit()`.
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
    Fixup(#[from] FixupError),

    #[error("Spec ID validation error: {0}")]
    SpecId(#[from] SpecIdError),

    #[error("File lock error: {0}")]
    Lock(#[from] LockError),

    #[error("LLM backend error: {0}")]
    Llm(#[from] LlmError),

    #[error("Validation failed for phase {phase}: {issue_count} issue(s)")]
    ValidationFailed {
        phase: String,
        issues: Vec<ValidationError>,
        issue_count: usize,
    },
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

/// Errors that can occur during fixup detection and parsing
#[derive(Error, Debug)]
pub enum FixupError {
    #[error("No fixup markers found in review output")]
    NoFixupMarkersFound,

    #[error("Invalid diff format in block {block_index}: {reason}")]
    InvalidDiffFormat { block_index: usize, reason: String },

    #[error("Git apply validation failed for {target_file}: {reason}")]
    GitApplyValidationFailed { target_file: String, reason: String },

    #[error("Git apply execution failed for {target_file}: {reason}")]
    GitApplyExecutionFailed { target_file: String, reason: String },

    #[error("Target file not found: {path}")]
    TargetFileNotFound { path: String },

    #[error("Failed to create temporary copy of {file}: {reason}")]
    TempCopyFailed { file: String, reason: String },

    #[error("Diff parsing failed: {reason}")]
    DiffParsingFailed { reason: String },

    #[error("No valid diff blocks found")]
    NoValidDiffBlocks,

    #[error("Absolute path not allowed: {0}")]
    AbsolutePath(PathBuf),

    #[error("Parent directory escape not allowed: {0}")]
    ParentDirEscape(PathBuf),

    #[error("Path resolves outside repo root: {0}")]
    OutsideRepo(PathBuf),

    #[error("Path canonicalization failed: {0}")]
    CanonicalizationError(String),

    #[error("Symlink not allowed (use --allow-links to permit): {0}")]
    SymlinkNotAllowed(PathBuf),

    #[error("Hardlink not allowed (use --allow-links to permit): {0}")]
    HardlinkNotAllowed(PathBuf),

    #[error(
        "Could not find matching context for hunk at line {expected_line} in {file} (searched ±{search_window} lines)"
    )]
    FuzzyMatchFailed {
        file: String,
        expected_line: usize,
        search_window: usize,
    },
}

/// Error type for spec ID validation failures
#[derive(Debug, thiserror::Error)]
pub enum SpecIdError {
    #[error("Spec ID is empty after sanitization")]
    Empty,

    #[error("Spec ID contains only invalid characters")]
    OnlyInvalidCharacters,
}

/// Validation error types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// Response starts with meta-commentary instead of document content
    MetaSummaryDetected { pattern: String },
    /// Response is too short for the phase type
    TooShort { actual: usize, minimum: usize },
    /// Required section header is missing
    MissingSectionHeader { header: String },
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MetaSummaryDetected { pattern } => {
                write!(f, "Response contains meta-summary pattern: '{}'", pattern)
            }
            Self::TooShort { actual, minimum } => {
                write!(
                    f,
                    "Response too short: {} lines (minimum: {} lines)",
                    actual, minimum
                )
            }
            Self::MissingSectionHeader { header } => {
                write!(f, "Missing required section: '{}'", header)
            }
        }
    }
}

impl std::error::Error for ValidationError {}

/// Errors that can occur during LLM backend operations
#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    /// Transport-level failure (process spawn, HTTP connectivity)
    #[error("Transport error: {0}")]
    Transport(String),

    /// Provider authentication failure (401, 403, missing API key)
    #[error("Provider authentication error: {0}")]
    ProviderAuth(String),

    /// Provider quota/rate limit exceeded (429)
    #[error("Provider quota exceeded: {0}")]
    ProviderQuota(String),

    /// Provider service outage (5xx errors)
    #[error("Provider outage: {0}")]
    ProviderOutage(String),

    /// Invocation timed out
    #[error("Timeout after {duration:?}")]
    Timeout { duration: Duration },

    /// Budget limit exceeded
    #[error("Budget exceeded: attempted {attempted} calls, limit is {limit}")]
    BudgetExceeded { limit: u32, attempted: u32 },

    /// Configuration error
    #[error("Misconfiguration: {0}")]
    Misconfiguration(String),

    /// Unsupported feature or provider
    #[error("Unsupported: {0}")]
    Unsupported(String),
}

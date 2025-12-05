use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Identifies the different phases in the spec generation workflow
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PhaseId {
    Requirements,
    Design,
    Tasks,
    Review,
    Fixup,
    Final,
}

impl PhaseId {
    /// Returns the string representation of the phase
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Requirements => "requirements",
            Self::Design => "design",
            Self::Tasks => "tasks",
            Self::Review => "review",
            Self::Fixup => "fixup",
            Self::Final => "final",
        }
    }
}

/// Priority levels for content selection in packet building
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
    /// Upstream *.core.yaml files - never evicted
    Upstream,
    /// High priority files (SPEC/ADR/REPORT)
    High,
    /// Medium priority files (README/SCHEMA)
    Medium,
    /// Low priority files (misc)
    Low,
}

/// File types for canonicalization and processing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileType {
    Yaml,
    Markdown,
    Text,
}

impl FileType {
    /// Determine file type from extension
    #[must_use]
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "yaml" | "yml" => Self::Yaml,
            "md" | "markdown" => Self::Markdown,
            _ => Self::Text,
        }
    }
}

/// Permission modes for Claude CLI tool usage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionMode {
    /// Plan mode - show what would be done
    Plan,
    /// Auto mode - automatically approve tool usage
    Auto,
    /// Block mode - block all tool usage
    Block,
}

impl PermissionMode {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Plan => "plan",
            Self::Auto => "auto",
            Self::Block => "block",
        }
    }
}

/// Output formats supported by Claude CLI
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputFormat {
    /// Structured JSON streaming format (preferred)
    StreamJson,
    /// Plain text format (fallback)
    Text,
}

impl OutputFormat {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::StreamJson => "stream-json",
            Self::Text => "text",
        }
    }
}

/// Runner modes for cross-platform Claude CLI execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunnerMode {
    /// Automatic detection (try native first, then WSL on Windows)
    Auto,
    /// Native execution (spawn claude directly)
    Native,
    /// WSL execution (use wsl.exe --exec on Windows)
    Wsl,
}

impl RunnerMode {
    /// Convert runner mode to string representation.
    /// Reserved for future use; CLI uses Display trait instead.
    #[must_use]
    #[allow(dead_code)] // Reserved for future use; CLI uses Display trait instead
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Native => "native",
            Self::Wsl => "wsl",
        }
    }
}

/// Enhanced receipt structure for multi-file support and full auditability
/// Records comprehensive information about phase execution including Claude CLI details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Receipt {
    /// Schema version for this receipt format
    pub schema_version: String,
    /// RFC3339 UTC timestamp when the receipt was emitted
    pub emitted_at: DateTime<Utc>,
    /// Unique identifier for the spec being processed
    pub spec_id: String,
    /// Phase that was executed
    pub phase: String,
    /// Version of xchecker that generated this receipt
    pub xchecker_version: String,
    /// Version of Claude CLI that was used
    pub claude_cli_version: String,
    /// Full model name that was actually used
    pub model_full_name: String,
    /// Model alias that was requested (if any)
    pub model_alias: Option<String>,
    /// Version of the canonicalization algorithm used
    pub canonicalization_version: String,
    /// Backend used for canonicalization (e.g., "jcs-rfc8785")
    pub canonicalization_backend: String,
    /// CLI flags and configuration used
    pub flags: HashMap<String, String>,
    /// Runner mode used for Claude CLI execution ("native" | "wsl")
    pub runner: String,
    /// WSL distribution name if runner is "wsl"
    pub runner_distro: Option<String>,
    /// Evidence of packet construction
    pub packet: PacketEvidence,
    /// BLAKE3 hashes of canonicalized outputs (sorted by path before emission)
    pub outputs: Vec<FileHash>,
    /// Exit code from the phase execution (0 = success)
    pub exit_code: i32,
    /// Error kind for non-zero exits
    pub error_kind: Option<ErrorKind>,
    /// Brief error reason for non-zero exits
    pub error_reason: Option<String>,
    /// Standard error tail (limited to 2 KiB)
    pub stderr_tail: Option<String>,
    /// Redacted standard error output (limited to 2 KiB)
    pub stderr_redacted: Option<String>,
    /// Warnings encountered during execution
    pub warnings: Vec<String>,
    /// Whether fallback to text format was used
    pub fallback_used: Option<bool>,
    /// Diff context lines (0 when --unidiff-zero is enabled)
    pub diff_context: Option<u32>,
    /// LLM metadata for receipts (V11+ multi-provider support)
    pub llm: Option<crate::receipt::LlmInfo>,
    /// Pipeline configuration metadata (V11+)
    pub pipeline: Option<PipelineInfo>,
}

/// Error kinds for receipt error tracking
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(any(test, feature = "test-utils"), derive(strum::VariantNames))]
pub enum ErrorKind {
    CliArgs,
    PacketOverflow,
    SecretDetected,
    LockHeld,
    PhaseTimeout,
    ClaudeFailure,
    Unknown,
}

/// Evidence of packet construction for auditability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacketEvidence {
    /// List of files included in the packet
    pub files: Vec<FileEvidence>,
    /// Maximum bytes allowed in packet
    pub max_bytes: usize,
    /// Maximum lines allowed in packet
    pub max_lines: usize,
}

/// Evidence of a single file's inclusion in the packet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEvidence {
    /// Path to the file relative to project root
    pub path: String,
    /// Optional range of lines included (e.g., "L1-L80")
    pub range: Option<String>,
    /// BLAKE3 hash of the file content before redaction
    pub blake3_pre_redaction: String,
    /// Priority level of this file
    pub priority: Priority,
}

/// Represents a file hash in the receipt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileHash {
    /// Path to the file relative to the spec directory
    pub path: String,
    /// BLAKE3 hash of the canonicalized content
    pub blake3_canonicalized: String,
}

/// Status output structure for JSON emission (schema v1)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusOutput {
    /// Schema version for this status format
    pub schema_version: String,
    /// RFC3339 UTC timestamp when the status was emitted
    pub emitted_at: DateTime<Utc>,
    /// Runner mode used for Claude CLI execution ("native" | "wsl")
    pub runner: String,
    /// WSL distribution name if runner is "wsl"
    pub runner_distro: Option<String>,
    /// Whether fallback to text format was used
    pub fallback_used: bool,
    /// Version of the canonicalization algorithm used
    pub canonicalization_version: String,
    /// Backend used for canonicalization (e.g., "jcs-rfc8785")
    pub canonicalization_backend: String,
    /// Artifacts with path and `blake3_first8` (sorted by path before emission)
    pub artifacts: Vec<ArtifactInfo>,
    /// Path to the last receipt file
    pub last_receipt_path: String,
    /// Effective configuration with source attribution
    pub effective_config: std::collections::BTreeMap<String, ConfigValue>,
    /// Lock drift information if lockfile exists
    pub lock_drift: Option<LockDrift>,
    /// Pending fixup summary (counts only) - optional field
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_fixups: Option<PendingFixupsSummary>,
}

/// Artifact information for status output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactInfo {
    /// Path to the artifact
    pub path: String,
    /// First 8 characters of BLAKE3 hash
    pub blake3_first8: String,
}

/// Configuration value with source attribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigValue {
    /// The configuration value (arbitrary JSON)
    pub value: serde_json::Value,
    /// Source of this configuration value
    pub source: ConfigSource,
}

/// Source of a configuration value
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(any(test, feature = "test-utils"), derive(strum::VariantNames))]
pub enum ConfigSource {
    Cli,
    Config,
    Default,
}

impl From<crate::config::ConfigSource> for ConfigSource {
    fn from(source: crate::config::ConfigSource) -> Self {
        match source {
            crate::config::ConfigSource::Cli => ConfigSource::Cli,
            crate::config::ConfigSource::ConfigFile(_) => ConfigSource::Config,
            crate::config::ConfigSource::Defaults => ConfigSource::Default,
        }
    }
}

/// Lock drift information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockDrift {
    /// Model full name drift
    pub model_full_name: Option<DriftPair>,
    /// Claude CLI version drift
    pub claude_cli_version: Option<DriftPair>,
    /// Schema version drift
    pub schema_version: Option<DriftPair>,
}

/// Drift pair showing locked vs current value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftPair {
    /// Value from lockfile
    pub locked: String,
    /// Current value
    pub current: String,
}

/// Pending fixups summary (counts only, no file contents or diffs)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingFixupsSummary {
    /// Number of target files with pending fixups
    pub targets: u32,
    /// Estimated number of lines to be added
    pub est_added: u32,
    /// Estimated number of lines to be removed
    pub est_removed: u32,
}

/// Pipeline configuration metadata (V11+)
/// All fields are optional for backward compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineInfo {
    /// Execution strategy used ("controlled" | "external_tool")
    pub execution_strategy: Option<String>,
}

/// Spec output structure for JSON emission (schema spec-json.v1)
/// Used by `xchecker spec --json` command for Claude Code integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecOutput {
    /// Schema version for this spec format (e.g., "spec-json.v1")
    pub schema_version: String,
    /// Unique identifier for the spec
    pub spec_id: String,
    /// List of phases with high-level metadata
    pub phases: Vec<PhaseInfo>,
    /// Configuration summary (paths, execution strategy, provider)
    pub config_summary: SpecConfigSummary,
}

/// Phase information for spec output (high-level metadata only)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseInfo {
    /// Phase identifier
    pub phase_id: String,
    /// Phase status: "completed", "pending", "not_started"
    pub status: String,
    /// RFC3339 UTC timestamp of last run (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run: Option<DateTime<Utc>>,
}

/// Configuration summary for spec output
/// Excludes full artifacts and packet contents per FR-Claude Code-CLI requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecConfigSummary {
    /// Execution strategy used ("controlled" | "external_tool")
    pub execution_strategy: String,
    /// LLM provider configured
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// Spec directory path
    pub spec_path: String,
}

/// Status output structure for JSON emission (schema status-json.v2)
/// Used by `xchecker status --json` command for Claude Code integration
/// Includes artifacts with blake3_first8, effective_config, and lock_drift
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusJsonOutput {
    /// Schema version for this status format (e.g., "status-json.v2")
    pub schema_version: String,
    /// Unique identifier for the spec
    pub spec_id: String,
    /// List of phase statuses with receipt IDs
    pub phase_statuses: Vec<PhaseStatusInfo>,
    /// Number of pending fixups (0 if none)
    pub pending_fixups: u32,
    /// Whether any errors exist in the spec
    pub has_errors: bool,
    /// Artifacts with path and blake3_first8 hash (first 8 chars of BLAKE3)
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub artifacts: Vec<ArtifactInfo>,
    /// Effective configuration with source attribution (cli/config/default)
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty", default)]
    pub effective_config: std::collections::BTreeMap<String, ConfigValue>,
    /// Lock drift information if lockfile exists and drift detected
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lock_drift: Option<LockDrift>,
}

/// Phase status information for compact status output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseStatusInfo {
    /// Phase identifier
    pub phase_id: String,
    /// Phase status: "success", "failed", "not_started"
    pub status: String,
    /// Receipt ID for the latest run (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt_id: Option<String>,
}

/// Resume output structure for JSON emission (schema resume-json.v1)
/// Used by `xchecker resume --json` command for Claude Code integration
/// Per FR-Claude Code-CLI (Requirements 4.1.3): Returns resume context without full packet/artifacts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumeJsonOutput {
    /// Schema version for this resume format (e.g., "resume-json.v1")
    pub schema_version: String,
    /// Unique identifier for the spec
    pub spec_id: String,
    /// Phase to resume from
    pub phase: String,
    /// Current inputs available for the phase (artifact names, not full contents)
    pub current_inputs: CurrentInputs,
    /// Next steps hint for the user/agent
    pub next_steps: String,
}

/// Current inputs available for a phase (high-level metadata only)
/// Excludes full packet and raw artifacts per FR-Claude Code-CLI requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentInputs {
    /// List of available artifact names (not full contents)
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub available_artifacts: Vec<String>,
    /// Whether the spec directory exists
    pub spec_exists: bool,
    /// Latest completed phase (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_completed_phase: Option<String>,
}


/// Workspace status output structure for JSON emission (schema workspace-status-json.v1)
/// Used by `xchecker project status --json` command for aggregated workspace status
/// Per FR-WORKSPACE (Requirements 4.3.4): Emits aggregated status for all specs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceStatusJsonOutput {
    /// Schema version for this workspace status format (e.g., "workspace-status-json.v1")
    pub schema_version: String,
    /// Name of the workspace
    pub workspace_name: String,
    /// Path to the workspace file
    pub workspace_path: String,
    /// Per-spec phase summaries
    pub specs: Vec<WorkspaceSpecStatus>,
    /// Summary counts
    pub summary: WorkspaceStatusSummary,
}

/// Per-spec status information for workspace status output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSpecStatus {
    /// Spec identifier
    pub spec_id: String,
    /// Tags associated with the spec
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tags: Vec<String>,
    /// Overall spec status: "success", "failed", "pending", "not_started", "stale"
    pub status: String,
    /// Latest completed phase (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_phase: Option<String>,
    /// RFC3339 UTC timestamp of last activity (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_activity: Option<DateTime<Utc>>,
    /// Number of pending fixups for this spec
    pub pending_fixups: u32,
    /// Whether this spec has errors
    pub has_errors: bool,
}

/// Summary counts for workspace status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceStatusSummary {
    /// Total number of specs in the workspace
    pub total_specs: u32,
    /// Number of specs with successful latest phase
    pub successful_specs: u32,
    /// Number of specs with failed latest phase
    pub failed_specs: u32,
    /// Number of specs with pending work (not completed all phases)
    pub pending_specs: u32,
    /// Number of specs that haven't been started
    pub not_started_specs: u32,
    /// Number of stale specs (no recent activity)
    pub stale_specs: u32,
}

/// Workspace history output structure for JSON emission (schema workspace-history-json.v1)
/// Used by `xchecker project history <spec-id> --json` command for spec timeline
/// Per FR-WORKSPACE (Requirements 4.3.5): Emits timeline of phase progression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceHistoryJsonOutput {
    /// Schema version for this workspace history format (e.g., "workspace-history-json.v1")
    pub schema_version: String,
    /// Spec identifier
    pub spec_id: String,
    /// Timeline of phase executions
    pub timeline: Vec<HistoryEntry>,
    /// Aggregated metrics across all executions
    pub metrics: HistoryMetrics,
}

/// A single entry in the spec history timeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// Phase that was executed
    pub phase: String,
    /// RFC3339 UTC timestamp of execution
    pub timestamp: DateTime<Utc>,
    /// Exit code of the execution (0 = success)
    pub exit_code: i32,
    /// Whether the execution was successful
    pub success: bool,
    /// LLM token usage for this execution (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_input: Option<u64>,
    /// LLM token output for this execution (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_output: Option<u64>,
    /// Number of fixups applied in this execution (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fixup_count: Option<u32>,
    /// Model used for this execution
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Provider used for this execution
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
}

/// Aggregated metrics for spec history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryMetrics {
    /// Total number of phase executions
    pub total_executions: u32,
    /// Number of successful executions
    pub successful_executions: u32,
    /// Number of failed executions
    pub failed_executions: u32,
    /// Total LLM tokens consumed (input)
    pub total_tokens_input: u64,
    /// Total LLM tokens consumed (output)
    pub total_tokens_output: u64,
    /// Total fixups applied across all executions
    pub total_fixups: u32,
    /// First execution timestamp (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_execution: Option<DateTime<Utc>>,
    /// Last execution timestamp (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_execution: Option<DateTime<Utc>>,
}

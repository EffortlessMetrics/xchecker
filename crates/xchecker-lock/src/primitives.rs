//! Data model for advisory locks and reproducibility trust objects.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::io;

/// Lock information stored in the lock file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockInfo {
    /// Process ID that created the lock
    pub pid: u32,
    /// Process start time (seconds since UNIX epoch)
    pub start_time: u64,
    /// Timestamp when the lock was created (seconds since UNIX epoch)
    pub created_at: u64,
    /// Spec ID being locked
    pub spec_id: String,
    /// xchecker version that created the lock
    pub xchecker_version: String,
}

/// `XChecker` lockfile for reproducibility tracking (schema v1)
/// Pins model, CLI version, and schema version to detect drift
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XCheckerLock {
    /// Schema version for this lockfile format
    pub schema_version: String,
    /// RFC3339 UTC timestamp when the lockfile was created
    pub created_at: DateTime<Utc>,
    /// Full model name that was used (e.g., "haiku")
    pub model_full_name: String,
    /// Claude CLI version that was used
    pub claude_cli_version: String,
}

/// Context for current run to compare against lockfile
#[derive(Debug, Clone)]
pub struct RunContext {
    pub model_full_name: String,
    pub claude_cli_version: String,
    pub schema_version: String,
}

/// Context for a governed flow boundary.
///
/// This intentionally pins promoted execution contracts rather than scratch
/// reasoning. The goal is to make replay / resume deterministic at promotion
/// points without freezing exploratory thought inside a stage.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FlowContext {
    pub flow_version: String,
    pub stage_graph_version: String,
    pub gate_set_version: String,
    pub prompt_pack_version: String,
    pub adapter_version: String,
    pub provider_policy: String,
    pub tool_context_version: String,
}

/// Reproducibility lock for the governed flow boundary (`flow.lock`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FlowLock {
    /// Schema version for this flow lock format
    pub schema_version: String,
    /// RFC3339 UTC timestamp when the lock was created
    pub created_at: DateTime<Utc>,
    /// Version of the flow pack or flow definition
    pub flow_version: String,
    /// Version of the stage graph / phase topology
    pub stage_graph_version: String,
    /// Version of the gate set used at promotion points
    pub gate_set_version: String,
    /// Version of the prompt pack used by the flow
    pub prompt_pack_version: String,
    /// Version of the adapter / harness boundary
    pub adapter_version: String,
    /// Capability or provider selection policy applied to the flow
    pub provider_policy: String,
    /// Version of tool permissions / MCP context configuration
    pub tool_context_version: String,
}

/// Drift information for a `FlowLock`.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct FlowLockDrift {
    pub flow_version: Option<DriftPair>,
    pub stage_graph_version: Option<DriftPair>,
    pub gate_set_version: Option<DriftPair>,
    pub prompt_pack_version: Option<DriftPair>,
    pub adapter_version: Option<DriftPair>,
    pub provider_policy: Option<DriftPair>,
    pub tool_context_version: Option<DriftPair>,
}

/// Artifact frozen by a promotion lock.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PromotionArtifact {
    /// Artifact path relative to the spec directory
    pub path: String,
    /// Canonical BLAKE3 hash of the promoted artifact
    pub blake3_canonicalized: String,
}

/// Trust object for artifacts that crossed a gate and became promoted state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PromotionLock {
    /// Schema version for this promotion lock format
    pub schema_version: String,
    /// RFC3339 UTC timestamp when the promotion happened
    pub promoted_at: DateTime<Utc>,
    /// Spec identifier this promotion belongs to
    pub spec_id: String,
    /// Stage / phase whose artifacts were promoted
    pub phase: String,
    /// Winning candidate identifier, if selection happened among alternatives
    pub selected_candidate_id: Option<String>,
    /// Gate name that approved the promotion
    pub gate_name: String,
    /// Version of the gate logic used to approve the promotion
    pub gate_version: String,
    /// Actor or system identity that approved the promotion
    pub approved_by: Option<String>,
    /// Receipt path that produced the promotable output
    pub parent_receipt_path: Option<String>,
    /// Packet lineage identifiers carried into this promotion
    pub parent_packet_lineage: Vec<String>,
    /// Promoted artifacts and their content hashes
    pub artifacts: Vec<PromotionArtifact>,
    /// Non-fatal warnings carried with the promotion event
    pub warnings: Vec<String>,
}

/// Drift pair showing locked vs current value
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DriftPair {
    /// Value from lockfile
    pub locked: String,
    /// Current value
    pub current: String,
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

/// Lock errors for file locking operations
#[derive(Debug, thiserror::Error)]
pub enum LockError {
    #[error(
        "Concurrent execution detected for spec '{spec_id}' (PID {pid}, created {created_ago} ago)"
    )]
    ConcurrentExecution {
        spec_id: String,
        pid: u32,
        created_ago: String,
    },

    #[error(
        "Stale lock detected for spec '{spec_id}' (PID {pid}, age {age_secs}s). Use --force to override"
    )]
    StaleLock {
        spec_id: String,
        pid: u32,
        age_secs: u64,
    },

    #[error("Lock file is corrupted or invalid: {reason}")]
    CorruptedLock { reason: String },

    #[error("Failed to acquire lock: {reason}")]
    AcquisitionFailed { reason: String },

    #[error("Failed to release lock: {reason}")]
    ReleaseFailed { reason: String },

    #[error("IO error during lock operation: {0}")]
    Io(#[from] io::Error),
}

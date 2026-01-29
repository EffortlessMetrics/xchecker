//! Gate result types for policy evaluation
//!
//! This module provides types for representing gate evaluation results.

use serde::{Deserialize, Serialize};

/// Result of gate evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResult {
    /// Schema version for the result
    pub schema_version: String,

    /// ID of the spec being evaluated
    pub spec_id: String,

    /// Whether spec passed all gate checks
    pub passed: bool,

    /// Human-readable summary of result
    pub summary: String,

    /// Individual conditions evaluated
    pub conditions: Vec<GateCondition>,

    /// Reasons for failure (if any)
    pub failure_reasons: Vec<String>,
}

/// Individual condition evaluated by gate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateCondition {
    /// Name of condition
    pub name: String,

    /// Description of what the condition checks
    pub description: String,

    /// Whether the condition passed
    pub passed: bool,

    /// Actual value observed
    pub actual: Option<String>,

    /// Expected value for passing
    pub expected: Option<String>,
}

/// Summary of pending fixups for a spec
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PendingFixupsStats {
    /// Number of target files with pending changes
    pub targets: u32,
    /// Estimated lines to be added
    pub est_added: u32,
    /// Estimated lines to be removed
    pub est_removed: u32,
}

/// Result of attempting to determine pending fixups state
///
/// This tri-state result allows callers to distinguish between:
/// - No fixups needed (None)
/// - Fixups found (Some(stats))
/// - Unable to determine (Unknown) - e.g., corrupted review artifact
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PendingFixupsResult {
    /// No fixups are pending (review completed, no markers, or review not done yet)
    None,
    /// Fixups are pending with the given statistics
    Some(PendingFixupsStats),
    /// Unable to determine fixup state (e.g., markers present but parse failed)
    Unknown {
        /// Reason why fixups state couldn't be determined
        reason: String,
    },
}

impl PendingFixupsResult {
    /// Check if there are definitely no pending fixups
    #[must_use]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    /// Check if fixups are definitely pending
    #[must_use]
    pub fn is_some(&self) -> bool {
        matches!(self, Self::Some(_))
    }

    /// Check if fixup state is unknown/indeterminate
    #[must_use]
    pub fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown { .. })
    }

    /// Get the stats if fixups are pending
    #[must_use]
    pub fn stats(&self) -> Option<&PendingFixupsStats> {
        match self {
            Self::Some(stats) => Some(stats),
            _ => None,
        }
    }

    /// Get the target count, or 0 if none/unknown
    ///
    /// Note: Use this only for display purposes. For gate checks,
    /// use `is_unknown()` to detect indeterminate states.
    #[must_use]
    pub fn targets_or_zero(&self) -> u32 {
        match self {
            Self::Some(stats) => stats.targets,
            _ => 0,
        }
    }

    /// Convert to legacy PendingFixupsStats (for backward compatibility)
    ///
    /// Returns default stats (zeros) for None and Unknown states.
    #[must_use]
    pub fn into_stats(self) -> PendingFixupsStats {
        match self {
            Self::Some(stats) => stats,
            _ => PendingFixupsStats::default(),
        }
    }
}

/// Trait for types that can provide spec data for gate evaluation
///
/// This trait abstracts data sources needed for gate checks, allowing
/// gate evaluation to work with different data providers (e.g., OrchestratorHandle,
/// direct file system access, etc.).
pub trait SpecDataProvider {
    /// Get the base path for the spec
    fn base_path(&self) -> &std::path::Path;

    /// Get the spec ID
    fn spec_id(&self) -> &str;

    /// Get the receipt manager
    fn receipt_manager(&self) -> &xchecker_receipt::ReceiptManager;

    /// Check if a phase is completed
    fn phase_completed(&self, phase: xchecker_utils::types::PhaseId) -> bool;

    /// Get the pending fixups result
    fn pending_fixups_result(&self) -> PendingFixupsResult;
}

use super::model::FixupMode;
use super::parse::FixupParser;

/// Summary of pending fixups for a spec
#[derive(Debug, Clone, Default)]
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
#[derive(Debug, Clone)]
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

    /// Check if the fixup state is unknown/indeterminate
    #[must_use]
    pub fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown { .. })
    }

    /// Get the stats if fixups are pending
    #[must_use]
    pub fn stats(&self) -> Option<&PendingFixupsStats> {
        match self {
            Self::Some(stats) => Some(stats),
            _ => Option::None,
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

/// Count pending fixups for a spec by ID
///
/// This function reads the review artifact (`30-review.md`) for the given spec,
/// parses any fixup markers, and returns statistics about pending changes.
///
/// # Arguments
///
/// * `spec_id` - The spec identifier to check for pending fixups
///
/// # Returns
///
/// Returns `PendingFixupsStats` with the number of target files and estimated
/// line changes. Returns `Default` (all zeros) if no fixups are pending or
/// if the review artifact doesn't exist.
///
/// Note: For gate checks, use `pending_fixups_result_for_spec` which can
/// distinguish between "no fixups" and "unknown/error" states.
#[must_use]
pub fn pending_fixups_for_spec(spec_id: &str) -> PendingFixupsStats {
    pending_fixups_result_for_spec(spec_id).into_stats()
}

/// Count pending fixups using an `OrchestratorHandle`
///
/// This function uses the handle's artifact manager to locate the review artifact
/// and parse fixup statistics.
///
/// # Arguments
///
/// * `handle` - A reference to an `OrchestratorHandle` for the spec
///
/// # Returns
///
/// Returns `PendingFixupsStats` with the number of target files and estimated
/// line changes. Returns `Default` (all zeros) if no fixups are pending or
/// if the review artifact doesn't exist.
///
/// Note: For gate checks, use `pending_fixups_result_from_handle` which can
/// distinguish between "no fixups" and "unknown/error" states.
#[must_use]
pub fn pending_fixups_from_handle(
    handle: &crate::orchestrator::OrchestratorHandle,
) -> PendingFixupsStats {
    pending_fixups_result_from_handle(handle).into_stats()
}

/// Get pending fixups result for a spec by ID (with error state)
///
/// This function reads the review artifact (`30-review.md`) for the given spec,
/// parses any fixup markers, and returns a result that can distinguish between
/// "no fixups", "fixups found", and "unknown/error" states.
///
/// # Arguments
///
/// * `spec_id` - The spec identifier to check for pending fixups
///
/// # Returns
///
/// Returns `PendingFixupsResult` which is one of:
/// - `None` - No fixups pending (review not done, no markers, or empty)
/// - `Some(stats)` - Fixups are pending with statistics
/// - `Unknown { reason }` - Review has markers but parse failed (possible corruption)
#[must_use]
pub fn pending_fixups_result_for_spec(spec_id: &str) -> PendingFixupsResult {
    let base_path = crate::paths::spec_root(spec_id);
    pending_fixups_result_impl(base_path.as_std_path())
}

/// Get pending fixups result using an `OrchestratorHandle` (with error state)
///
/// This function uses the handle's artifact manager to locate the review artifact
/// and parse fixup statistics, returning a result that can distinguish between
/// "no fixups", "fixups found", and "unknown/error" states.
///
/// # Arguments
///
/// * `handle` - A reference to an `OrchestratorHandle` for the spec
///
/// # Returns
///
/// Returns `PendingFixupsResult` which is one of:
/// - `None` - No fixups pending (review not done, no markers, or empty)
/// - `Some(stats)` - Fixups are pending with statistics
/// - `Unknown { reason }` - Review has markers but parse failed (possible corruption)
#[must_use]
pub fn pending_fixups_result_from_handle(
    handle: &crate::orchestrator::OrchestratorHandle,
) -> PendingFixupsResult {
    let base_path = handle.artifact_manager().base_path();
    pending_fixups_result_impl(base_path.as_std_path())
}

/// Internal implementation for counting pending fixups with result type
fn pending_fixups_result_impl(base_path: &std::path::Path) -> PendingFixupsResult {
    let review_md_path = base_path.join("artifacts").join("30-review.md");

    if !review_md_path.exists() {
        return PendingFixupsResult::None; // No review phase completed yet
    }

    // Read the review content
    let review_content = match std::fs::read_to_string(&review_md_path) {
        Ok(content) => content,
        Err(e) => {
            // File exists but can't be read - this is unexpected
            return PendingFixupsResult::Unknown {
                reason: format!("Failed to read review artifact: {}", e),
            };
        }
    };

    // Create fixup parser in preview mode to check for targets
    let fixup_parser = match FixupParser::new(FixupMode::Preview, base_path.to_path_buf()) {
        Ok(parser) => parser,
        Err(e) => {
            return PendingFixupsResult::Unknown {
                reason: format!("Failed to create fixup parser: {}", e),
            };
        }
    };

    // Check if there are fixup markers
    if !fixup_parser.has_fixup_markers(&review_content) {
        return PendingFixupsResult::None; // No fixups needed
    }

    // Parse diffs to get intended targets and stats
    // CRITICAL: If markers are present but parse fails, this is an error state
    // (could indicate corrupted review artifact)
    match fixup_parser.parse_diffs(&review_content) {
        Ok(diffs) => {
            if diffs.is_empty() {
                return PendingFixupsResult::None;
            }

            let targets = diffs.len() as u32;
            let mut est_added: u32 = 0;
            let mut est_removed: u32 = 0;

            // Count added/removed lines from all hunks
            for diff in &diffs {
                for hunk in &diff.hunks {
                    for line in hunk.content.lines() {
                        if line.starts_with('+') && !line.starts_with("+++") {
                            est_added = est_added.saturating_add(1);
                        } else if line.starts_with('-') && !line.starts_with("---") {
                            est_removed = est_removed.saturating_add(1);
                        }
                    }
                }
            }

            PendingFixupsResult::Some(PendingFixupsStats {
                targets,
                est_added,
                est_removed,
            })
        }
        Err(e) => {
            // Markers present but parse failed - this is an unknown/error state
            // Gate should treat this conservatively (as failure)
            PendingFixupsResult::Unknown {
                reason: format!("Review has fixup markers but diff parse failed: {}", e),
            }
        }
    }
}

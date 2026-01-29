use super::model::FixupMode;
use super::parse::FixupParser;
use crate::gate::{PendingFixupsResult, PendingFixupsStats};
use crate::orchestrator::OrchestratorHandle;
use std::path::PathBuf;

/// Get pending fixups result for a spec by ID
///
/// This function reads the review artifact (`30-review.md`) for a given spec,
/// parses any fixup markers, and returns a detailed result about pending changes.
///
/// # Arguments
///
/// * `spec_id` - The spec identifier to check for pending fixups
///
/// # Returns
///
/// Returns `PendingFixupsResult` which can distinguish between:
/// - `None`: No review phase completed yet or no fixups needed
/// - `Some(stats)`: Fixups are pending with specific statistics
/// - `Unknown`: Error state (e.g., file read failed, parse failed)
#[must_use]
pub fn pending_fixups_result_for_spec(spec_id: &str) -> PendingFixupsResult {
    // Resolve spec directory path: .xchecker/specs/<spec_id>
    let spec_dir = PathBuf::from(".xchecker").join("specs").join(spec_id);
    pending_fixups_result_impl(&spec_dir)
}

/// Count pending fixups for a spec by ID
///
/// This function reads the review artifact (`30-review.md`) for a given spec,
/// parses any fixup markers, and returns statistics about pending changes.
///
/// # Arguments
///
/// * `spec_id` - The spec identifier to check for pending fixups
///
/// # Returns
///
/// Returns `PendingFixupsStats` with a number of target files and estimated
/// line changes. Returns `Default` (all zeros) if no fixups are pending or
/// if the review artifact doesn't exist.
///
/// Note: For gate checks, use `pending_fixups_result_for_spec` which can
/// distinguish between "no fixups" and "unknown/error" states.
#[must_use]
pub fn pending_fixups_for_spec(spec_id: &str) -> PendingFixupsStats {
    pending_fixups_result_for_spec(spec_id).into_stats()
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

/// Get pending fixups result from an OrchestratorHandle
///
/// This function provides a convenient wrapper for getting pending fixups
/// from an `OrchestratorHandle` reference, which implements the
/// `SpecDataProvider` trait.
///
/// # Arguments
///
/// * `handle` - A reference to an `OrchestratorHandle`
///
/// # Returns
///
/// Returns `PendingFixupsResult` which can distinguish between:
/// - `None`: No review phase completed yet or no fixups needed
/// - `Some(stats)`: Fixups are pending with specific statistics
/// - `Unknown`: Error state (e.g., file read failed, parse failed)
#[must_use]
pub fn pending_fixups_result_from_handle(handle: &OrchestratorHandle) -> PendingFixupsResult {
    pending_fixups_result_for_spec(handle.spec_id())
}

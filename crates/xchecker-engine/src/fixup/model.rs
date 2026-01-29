//! Fixup model types for spec generation workflows
//!
//! This module re-exports the core types and models from xchecker-fixup-model
//! for detecting and applying changes to specification artifacts.

// Re-export all types from xchecker-fixup-model for compatibility
pub use xchecker_fixup_model::{
    AppliedFile, ChangeSummary, DiffHunk, FixupMode, FixupPreview, FixupResult, UnifiedDiff,
};

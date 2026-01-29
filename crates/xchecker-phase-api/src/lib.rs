//! Phase trait system for orchestrating spec generation workflows
//!
//! This module defines the core Phase trait and related types that enable
//! the structured execution of spec generation phases with separated concerns.
//!
//! # Purpose
//!
//! This crate provides the shared contract between the orchestrator and phase
//! implementations. It contains minimal types needed for phase execution
//! without introducing circular dependencies.

use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use xchecker_packet::{BudgetUsage, Packet};
use xchecker_redaction::SecretRedactor;
use xchecker_selectors::Selectors;
use xchecker_status::artifact::Artifact;
pub use xchecker_utils::types::PhaseId;

/// Represents the next step to take after a phase completes
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NextStep {
    /// Continue to the next phase in the normal flow
    Continue,
    /// Rewind to a previous phase (used by fixup system)
    Rewind { to: PhaseId },
    /// Complete the entire workflow
    #[allow(dead_code)] // Reserved for workflow completion signaling
    Complete,
}

/// Context information passed to phases during execution
#[derive(Debug, Clone)]
pub struct PhaseContext {
    /// Unique identifier for the spec being processed
    pub spec_id: String,
    /// Base directory for the spec artifacts
    pub spec_dir: PathBuf,
    /// Configuration and runtime parameters
    pub config: HashMap<String, String>,
    /// Available artifacts from previous phases
    #[allow(dead_code)] // Reserved for cross-phase artifact references
    pub artifacts: Vec<String>,
    /// Content selectors for packet building (from config)
    ///
    /// If `Some`, phases should use these selectors when building packets.
    /// If `None`, phases fall back to built-in selector defaults.
    pub selectors: Option<Selectors>,
    /// Enable strict validation for phase outputs.
    ///
    /// When `true`, validation failures (meta-summaries, too-short output,
    /// missing required sections) become hard errors that fail the phase.
    /// When `false`, validation issues are logged as warnings only.
    pub strict_validation: bool,
    /// Secret redactor for any user-facing output emitted during phase execution.
    ///
    /// This is built once from the effective configuration and threaded through to ensure
    /// configured extra/ignore patterns are applied consistently.
    pub redactor: Arc<SecretRedactor>,
}

/// Metadata about phase execution
#[derive(Debug, Clone, Default)]
#[allow(dead_code)] // Metadata fields reserved for receipts and diagnostics
pub struct PhaseMetadata {
    /// BLAKE3 hash of the packet used for this phase
    pub packet_hash: Option<String>,
    /// Budget usage information
    pub budget_used: Option<BudgetUsage>,
    /// Duration of phase execution in milliseconds
    pub duration_ms: Option<u64>,
}

/// Result of executing a phase
#[derive(Debug, Clone)]
pub struct PhaseResult {
    /// Artifacts produced by the phase
    pub artifacts: Vec<Artifact>,
    /// What should happen next in the workflow
    pub next_step: NextStep,
    /// Additional metadata about the phase execution
    #[allow(dead_code)] // Metadata reserved for structured receipts
    pub metadata: PhaseMetadata,
}

/// Core trait that all workflow phases must implement
///
/// This trait separates concerns into three distinct operations:
/// - `prompt()`: Generate the prompt for Claude
/// - `make_packet()`: Prepare context packet for Claude
/// - `postprocess()`: Process Claude's response into artifacts
pub trait Phase {
    /// Returns the unique identifier for this phase
    fn id(&self) -> PhaseId;

    /// Returns the phases that must complete before this phase can run
    fn deps(&self) -> &'static [PhaseId];

    /// Returns whether this phase can be resumed from a partial state
    #[allow(dead_code)] // Trait interface for resumable phases
    fn can_resume(&self) -> bool;

    /// Generate the prompt text for Claude CLI
    ///
    /// This method creates the specific prompt that will be sent to Claude
    /// for this phase, based on the current context and available artifacts.
    fn prompt(&self, ctx: &PhaseContext) -> String;

    /// Create a packet of context information for Claude
    ///
    /// This method builds the context packet that will be included with
    /// the prompt, selecting and organizing relevant files and information.
    fn make_packet(&self, ctx: &PhaseContext) -> Result<Packet>;

    /// Process Claude's raw response into structured artifacts
    ///
    /// This method takes Claude's raw output and converts it into the
    /// appropriate artifacts for this phase, handling any necessary
    /// parsing and validation.
    fn postprocess(&self, raw: &str, ctx: &PhaseContext) -> Result<PhaseResult>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_next_step_variants() {
        let continue_step = NextStep::Continue;
        assert!(matches!(continue_step, NextStep::Continue));

        let rewind_step = NextStep::Rewind {
            to: PhaseId::Requirements,
        };
        assert!(matches!(rewind_step, NextStep::Rewind { .. }));

        let complete_step = NextStep::Complete;
        assert!(matches!(complete_step, NextStep::Complete));
    }

    #[test]
    fn test_phase_context_creation() {
        let ctx = PhaseContext {
            spec_id: "test-spec".to_string(),
            spec_dir: PathBuf::from("/tmp/test"),
            config: HashMap::new(),
            artifacts: Vec::new(),
            selectors: None,
            strict_validation: false,
            redactor: Arc::new(SecretRedactor::default()),
        };

        assert_eq!(ctx.spec_id, "test-spec");
        assert!(!ctx.strict_validation);
    }

    #[test]
    fn test_phase_metadata_default() {
        let metadata = PhaseMetadata::default();
        assert_eq!(metadata.packet_hash, None);
        assert!(metadata.budget_used.is_none());
        assert_eq!(metadata.duration_ms, None);
    }
}

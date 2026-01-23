//! Phase trait system for orchestrating spec generation workflows
//!
//! This module defines the core Phase trait and related types that enable
//! the structured execution of spec generation phases with separated concerns.

use crate::config::Selectors;
use crate::redaction::SecretRedactor;
use crate::types::PhaseId;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;

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
    pub spec_dir: std::path::PathBuf,
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
    /// configured extra/ignore patterns are applied consistently (FR-SEC-19).
    pub redactor: Arc<SecretRedactor>,
}

/// A packet of content prepared for Claude CLI consumption
#[derive(Debug, Clone)]
pub struct Packet {
    /// The actual content to send to Claude
    pub content: String,
    /// BLAKE3 hash of the packet content (after redaction)
    pub blake3_hash: String,
    /// Evidence of what went into the packet for auditability
    pub evidence: crate::types::PacketEvidence,
    /// Information about budget usage
    pub budget_used: BudgetUsage,
}

impl Packet {
    /// Create a new packet
    #[must_use]
    pub const fn new(
        content: String,
        blake3_hash: String,
        evidence: crate::types::PacketEvidence,
        budget_used: BudgetUsage,
    ) -> Self {
        Self {
            content,
            blake3_hash,
            evidence,
            budget_used,
        }
    }

    /// Get the packet content
    #[must_use]
    #[allow(dead_code)] // Public API for packet inspection
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Get the packet hash
    #[must_use]
    pub fn hash(&self) -> &str {
        &self.blake3_hash
    }

    /// Get the packet evidence
    #[must_use]
    #[allow(dead_code)] // Public API for evidence inspection
    pub const fn evidence(&self) -> &crate::types::PacketEvidence {
        &self.evidence
    }

    /// Get budget usage information
    #[must_use]
    pub const fn budget_usage(&self) -> &BudgetUsage {
        &self.budget_used
    }

    /// Check if packet is within budget limits
    #[must_use]
    #[allow(dead_code)] // Public API for budget validation
    pub const fn is_within_budget(&self) -> bool {
        !self.budget_used.is_exceeded()
    }
}

// PacketEvidence and FileEvidence are now defined in types.rs

/// Information about packet budget usage
#[derive(Debug, Clone)]
pub struct BudgetUsage {
    /// Current bytes used
    pub bytes_used: usize,
    /// Current lines used
    pub lines_used: usize,
    /// Maximum bytes allowed
    pub max_bytes: usize,
    /// Maximum lines allowed
    pub max_lines: usize,
}

impl BudgetUsage {
    /// Create a new budget tracker
    #[must_use]
    pub const fn new(max_bytes: usize, max_lines: usize) -> Self {
        Self {
            bytes_used: 0,
            lines_used: 0,
            max_bytes,
            max_lines,
        }
    }

    /// Check if adding content would exceed budget
    #[must_use]
    pub const fn would_exceed(&self, bytes: usize, lines: usize) -> bool {
        self.bytes_used + bytes > self.max_bytes || self.lines_used + lines > self.max_lines
    }

    /// Add content to budget tracking
    pub const fn add_content(&mut self, bytes: usize, lines: usize) {
        self.bytes_used += bytes;
        self.lines_used += lines;
    }

    /// Check if budget is exceeded
    #[must_use]
    pub const fn is_exceeded(&self) -> bool {
        self.bytes_used > self.max_bytes || self.lines_used > self.max_lines
    }
}

// Re-export artifact types from the artifact module
pub use crate::artifact::Artifact;

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
    /// Note: Budget enforcement is not implemented yet (will be added in M2).
    fn make_packet(&self, ctx: &PhaseContext) -> Result<Packet>;

    /// Process Claude's raw response into structured artifacts
    ///
    /// This method takes Claude's raw output and converts it into the
    /// appropriate artifacts for this phase, handling any necessary
    /// parsing and validation.
    fn postprocess(&self, raw: &str, ctx: &PhaseContext) -> Result<PhaseResult>;
}

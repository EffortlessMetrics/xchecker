//! Packet construction system for token-efficient context management.
//!
//! This crate implements the packet building system that selects and organizes
//! content for Claude CLI invocations while respecting budget constraints and
//! maintaining evidence for auditability.

use xchecker_utils::types::PacketEvidence;

mod budget;
mod builder;
mod model;
mod render;
mod selectors;

/// A packet of content prepared for Claude CLI consumption.
#[derive(Debug, Clone)]
pub struct Packet {
    /// The actual content to send to Claude.
    pub content: String,
    /// BLAKE3 hash of the packet content (after redaction).
    pub blake3_hash: String,
    /// Evidence of what went into the packet for auditability.
    pub evidence: PacketEvidence,
    /// Information about budget usage.
    pub budget_used: BudgetUsage,
}

impl Packet {
    /// Create a new packet.
    #[must_use]
    pub const fn new(
        content: String,
        blake3_hash: String,
        evidence: PacketEvidence,
        budget_used: BudgetUsage,
    ) -> Self {
        Self {
            content,
            blake3_hash,
            evidence,
            budget_used,
        }
    }

    /// Get the packet content.
    #[must_use]
    #[allow(dead_code)] // Public API for packet inspection
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Get the packet hash.
    #[must_use]
    pub fn hash(&self) -> &str {
        &self.blake3_hash
    }

    /// Get the packet evidence.
    #[must_use]
    #[allow(dead_code)] // Public API for evidence inspection
    pub const fn evidence(&self) -> &PacketEvidence {
        &self.evidence
    }

    /// Get budget usage information.
    #[must_use]
    pub const fn budget_usage(&self) -> &BudgetUsage {
        &self.budget_used
    }

    /// Check if packet is within budget limits.
    #[must_use]
    #[allow(dead_code)] // Public API for budget validation
    pub const fn is_within_budget(&self) -> bool {
        !self.budget_used.is_exceeded()
    }
}

/// Information about packet budget usage.
#[derive(Debug, Clone)]
pub struct BudgetUsage {
    /// Current bytes used.
    pub bytes_used: usize,
    /// Current lines used.
    pub lines_used: usize,
    /// Maximum bytes allowed.
    pub max_bytes: usize,
    /// Maximum lines allowed.
    pub max_lines: usize,
}

impl BudgetUsage {
    /// Create a new budget tracker.
    #[must_use]
    pub const fn new(max_bytes: usize, max_lines: usize) -> Self {
        Self {
            bytes_used: 0,
            lines_used: 0,
            max_bytes,
            max_lines,
        }
    }

    /// Check if adding content would exceed budget.
    #[must_use]
    pub const fn would_exceed(&self, bytes: usize, lines: usize) -> bool {
        self.bytes_used + bytes > self.max_bytes || self.lines_used + lines > self.max_lines
    }

    /// Add content to budget tracking.
    pub const fn add_content(&mut self, bytes: usize, lines: usize) {
        self.bytes_used += bytes;
        self.lines_used += lines;
    }

    /// Check if budget is exceeded.
    #[must_use]
    pub const fn is_exceeded(&self) -> bool {
        self.bytes_used > self.max_bytes || self.lines_used > self.max_lines
    }
}

pub use builder::{DEFAULT_PACKET_MAX_BYTES, DEFAULT_PACKET_MAX_LINES, PacketBuilder};
pub use model::{PriorityRules, SelectedFile};
pub use selectors::ContentSelector;

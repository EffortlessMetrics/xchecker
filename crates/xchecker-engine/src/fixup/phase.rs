//! Fixup phase implementation
//!
//! This module implements the Fixup phase that applies changes to specification
//! artifacts based on review feedback. The Fixup phase parses unified diffs
//! from review output and applies them in preview or apply mode.

use crate::phase::{NextStep, Phase, PhaseContext, PhaseMetadata, PhaseResult};
use crate::status::artifact::{Artifact, ArtifactType};
use anyhow::Result;

use super::FixupMode;
use super::parse::FixupParser;

/// Implementation of Fixup phase
///
/// This phase applies changes to specification artifacts based on review feedback.
/// It parses unified diff blocks from review output and applies them in
/// preview or apply mode.
#[derive(Debug, Clone)]
pub struct FixupPhase {
    mode: FixupMode,
}

impl FixupPhase {
    /// Create a new Fixup phase instance with the specified mode
    #[must_use]
    pub const fn new_with_mode(mode: FixupMode) -> Self {
        Self { mode }
    }

    /// Create a new Fixup phase instance in preview mode
    #[must_use]
    pub const fn new() -> Self {
        Self {
            mode: FixupMode::Preview,
        }
    }
}

impl Phase for FixupPhase {
    fn id(&self) -> crate::types::PhaseId {
        xchecker_utils::types::PhaseId::Fixup
    }

    fn deps(&self) -> &'static [xchecker_utils::types::PhaseId] {
        // Fixup phase depends on Review phase
        &[xchecker_utils::types::PhaseId::Review]
    }

    fn can_resume(&self) -> bool {
        true
    }

    fn prompt(&self, _ctx: &PhaseContext) -> String {
        // Fixup phase doesn't use LLM - it applies pre-parsed diffs
        // The prompt is not used for fixup phase
        String::new()
    }

    fn make_packet(&self, _ctx: &PhaseContext) -> Result<crate::packet::Packet> {
        // Fixup phase doesn't build a packet - it applies pre-parsed diffs
        // Return an empty packet for now
        let empty_content = String::new();
        let blake3_hash = blake3::hash(empty_content.as_bytes()).to_hex().to_string();

        let evidence = crate::types::PacketEvidence {
            files: vec![],
            max_bytes: 65536,
            max_lines: 1200,
        };

        let budget_used = crate::packet::BudgetUsage::new(65536, 1200);

        Ok(crate::packet::Packet::new(
            empty_content,
            blake3_hash,
            evidence,
            budget_used,
        ))
    }

    fn postprocess(&self, raw: &str, ctx: &PhaseContext) -> Result<PhaseResult> {
        // Parse fixup diffs from review output
        let parser = FixupParser::new(self.mode, ctx.spec_dir.clone())?;

        match parser.parse_diffs(raw) {
            Ok(diffs) => {
                // Create fixup.md artifact with parsed diffs
                let fixup_content = format!(
                    "# Fixup Report\n\nMode: {:?}\n\nParsed {} diff(s) from review output.\n",
                    self.mode,
                    diffs.len()
                );

                let fixup_artifact = Artifact {
                    name: "40-fixup.md".to_string(),
                    content: fixup_content.clone(),
                    artifact_type: ArtifactType::Markdown,
                    blake3_hash: blake3::hash(fixup_content.as_bytes()).to_hex().to_string(),
                };

                let artifacts = vec![fixup_artifact];

                // Metadata will be populated by orchestrator
                let metadata = PhaseMetadata::default();

                Ok(PhaseResult {
                    artifacts,
                    next_step: NextStep::Continue, // Proceed to Final phase
                    metadata,
                })
            }
            Err(e) => Err(e.into()),
        }
    }
}

impl Default for FixupPhase {
    fn default() -> Self {
        Self::new()
    }
}

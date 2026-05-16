//! Phase numbering and factory helpers for the orchestrator.

use anyhow::Result;

use crate::fixup::{FixupMode, FixupPhase};
use crate::phase::Phase;
use crate::phases::{DesignPhase, RequirementsPhase, ReviewPhase, TasksPhase};
use crate::types::PhaseId;

use super::{OrchestratorConfig, PhaseOrchestrator};

impl PhaseOrchestrator {
    /// Get the phase number for artifact naming
    pub(crate) const fn get_phase_number(&self, phase_id: PhaseId) -> u8 {
        match phase_id {
            PhaseId::Requirements => 0,
            PhaseId::Design => 10,
            PhaseId::Tasks => 20,
            PhaseId::Review => 30,
            PhaseId::Fixup => 40,
            PhaseId::Final => 50,
        }
    }

    /// Get a phase implementation by ID (phase factory)
    /// This method creates the appropriate Phase trait object for the given phase ID
    pub(crate) fn get_phase_impl(
        &self,
        phase_id: PhaseId,
        config: &OrchestratorConfig,
    ) -> Result<Box<dyn Phase>> {
        match phase_id {
            PhaseId::Requirements => Ok(Box::new(RequirementsPhase::new())),
            PhaseId::Design => Ok(Box::new(DesignPhase::new())),
            PhaseId::Tasks => Ok(Box::new(TasksPhase::new())),
            PhaseId::Review => Ok(Box::new(ReviewPhase::new())),
            PhaseId::Fixup => {
                // Determine fixup mode from configuration (FR-FIX-004, FR-FIX-005)
                let apply_fixups = config
                    .config
                    .get("apply_fixups")
                    .is_some_and(|s| s == "true");

                let fixup_mode = if apply_fixups {
                    FixupMode::Apply
                } else {
                    FixupMode::Preview
                };

                Ok(Box::new(FixupPhase::new_with_mode(fixup_mode)))
            }
            PhaseId::Final => Err(anyhow::anyhow!("Final phase not yet implemented")),
        }
    }
}

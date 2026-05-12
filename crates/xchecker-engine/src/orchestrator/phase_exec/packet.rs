//! Packet construction, redaction checks, and debug packet persistence.

use anyhow::Result;

use crate::error::{PhaseError, XCheckerError};
use crate::packet::{Packet, PacketBuilder};
use crate::phase::{Phase, PhaseContext};
use crate::types::PhaseId;

use crate::orchestrator::{OrchestratorConfig, PhaseOrchestrator};

impl PhaseOrchestrator {
    /// Build a phase packet and emit visibility telemetry about its hash and budget.
    pub(crate) fn build_phase_packet(
        &self,
        phase: &dyn Phase,
        phase_context: &PhaseContext,
        phase_id: PhaseId,
    ) -> Result<Packet> {
        let packet = phase.make_packet(phase_context).map_err(|e| {
            XCheckerError::Phase(PhaseError::PacketCreationFailed {
                phase: phase_id.as_str().to_string(),
                reason: e.to_string(),
            })
        })?;

        let budget = packet.budget_usage();
        tracing::info!(
            target: "xchecker::packet",
            spec_id = %self.spec_id(),
            phase = %phase_id.as_str(),
            packet_hash = %packet.hash(),
            bytes_used = budget.bytes_used,
            bytes_limit = budget.max_bytes,
            lines_used = budget.lines_used,
            lines_limit = budget.max_lines,
            "Built packet for phase"
        );

        Ok(packet)
    }

    /// Persist packet context artifacts that are only written after secret scanning succeeds.
    pub(crate) fn persist_packet_context(
        &self,
        phase_id: PhaseId,
        packet_content: &str,
        config: &OrchestratorConfig,
    ) -> Result<()> {
        self.artifact_manager()
            .store_context_file(&format!("{}-packet", phase_id.as_str()), packet_content)?;

        let debug_packet_enabled = config
            .config
            .get("debug_packet")
            .is_some_and(|s| s == "true");

        if debug_packet_enabled {
            self.write_debug_packet(phase_id, packet_content)?;
        }

        Ok(())
    }

    fn write_debug_packet(&self, phase_id: PhaseId, packet_content: &str) -> Result<()> {
        let context_dir = self.artifact_manager().context_path();
        let temp_builder = PacketBuilder::new().map_err(|e| {
            XCheckerError::Phase(PhaseError::PacketCreationFailed {
                phase: phase_id.as_str().to_string(),
                reason: format!("Failed to create PacketBuilder for debug packet: {e}"),
            })
        })?;

        if let Err(e) =
            temp_builder.write_debug_packet(packet_content, phase_id.as_str(), &context_dir)
        {
            eprintln!("Warning: Failed to write debug packet: {e}");
        }

        Ok(())
    }
}

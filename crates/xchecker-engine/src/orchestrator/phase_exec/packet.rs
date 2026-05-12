//! Packet preparation and debug-file persistence for phase execution.

use anyhow::Result;

use crate::error::{PhaseError, XCheckerError};
use crate::packet::{Packet, PacketBuilder};
use crate::phase::{Phase, PhaseContext};
use crate::types::PhaseId;

use super::super::{OrchestratorConfig, PhaseOrchestrator};

pub(super) struct PhasePacketInput {
    pub(super) context: PhaseContext,
    pub(super) prompt: String,
    pub(super) packet: Packet,
}

impl PhaseOrchestrator {
    pub(super) fn prepare_phase_packet(
        &self,
        phase: &dyn Phase,
        config: &OrchestratorConfig,
    ) -> Result<PhasePacketInput> {
        let phase_id = phase.id();
        let context = self.create_phase_context(phase_id, config)?;
        self.check_phase_dependencies(phase)?;

        let prompt = phase.prompt(&context);
        let packet = phase.make_packet(&context).map_err(|e| {
            XCheckerError::Phase(PhaseError::PacketCreationFailed {
                phase: phase_id.as_str().to_string(),
                reason: e.to_string(),
            })
        })?;

        self.log_packet_budget(phase_id, &packet);

        Ok(PhasePacketInput {
            context,
            prompt,
            packet,
        })
    }

    pub(super) fn store_packet_context_files(
        &self,
        phase_id: PhaseId,
        packet_content: &str,
        config: &OrchestratorConfig,
    ) -> Result<()> {
        self.artifact_manager()
            .store_context_file(&format!("{}-packet", phase_id.as_str()), packet_content)?;

        if config
            .config
            .get("debug_packet")
            .is_some_and(|s| s == "true")
        {
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
        }

        Ok(())
    }

    fn log_packet_budget(&self, phase_id: PhaseId, packet: &Packet) {
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
    }
}

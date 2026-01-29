use super::builder::PacketBuilder;
use anyhow::{Context, Result};
use camino::Utf8Path;
use xchecker_utils::atomic_write::write_file_atomic;

impl PacketBuilder {
    /// Write packet preview to context directory
    /// Always writes `context/<phase>-packet.txt` for auditability
    pub(super) fn write_packet_preview(
        &self,
        content: &str,
        phase: &str,
        context_dir: &Utf8Path,
    ) -> Result<()> {
        // Ensure context directory exists (ignore benign races)
        xchecker_utils::paths::ensure_dir_all(context_dir)
            .with_context(|| format!("Failed to create context directory: {context_dir}"))?;

        let preview_path = context_dir.join(format!("{}-packet.txt", phase.to_lowercase()));

        // Write packet preview
        write_file_atomic(&preview_path, content)
            .with_context(|| format!("Failed to write packet preview to: {preview_path}"))?;

        Ok(())
    }

    /// Write full debug packet if --debug-packet flag is set (FR-PKT-007)
    /// Only writes if secret scan passes; file is excluded from receipts
    pub fn write_debug_packet(
        &self,
        content: &str,
        phase: &str,
        context_dir: &Utf8Path,
    ) -> Result<()> {
        // Ensure context directory exists
        xchecker_utils::paths::ensure_dir_all(context_dir)
            .with_context(|| format!("Failed to create context directory: {context_dir}"))?;

        let debug_path = context_dir.join(format!("{}-packet-debug.txt", phase.to_lowercase()));

        // Write full packet content (after secret scan has passed)
        write_file_atomic(&debug_path, content)
            .with_context(|| format!("Failed to write debug packet to: {debug_path}"))?;

        Ok(())
    }
}

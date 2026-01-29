use super::builder::PacketBuilder;
use crate::BudgetUsage;
use anyhow::{Context, Result};
use camino::Utf8Path;
use xchecker_utils::atomic_write::write_file_atomic;
use xchecker_utils::types::FileEvidence;

impl PacketBuilder {
    /// Write packet manifest on overflow (FR-PKT-006)
    /// Manifest contains only sizes, counts, and file paths (no payload content)
    pub(super) fn write_packet_manifest(
        &self,
        included_files: &[FileEvidence],
        budget: &BudgetUsage,
        phase: &str,
        context_dir: &Utf8Path,
    ) -> Result<()> {
        use serde_json::json;

        // Ensure context directory exists
        xchecker_utils::paths::ensure_dir_all(context_dir)
            .with_context(|| format!("Failed to create context directory: {context_dir}"))?;

        let manifest_path =
            context_dir.join(format!("{}-packet.manifest.json", phase.to_lowercase()));

        // Build manifest with sanitized information (no content)
        let manifest = json!({
            "phase": phase,
            "overflow": true,
            "budget": {
                "max_bytes": budget.max_bytes,
                "max_lines": budget.max_lines,
                "used_bytes": budget.bytes_used,
                "used_lines": budget.lines_used,
            },
            "files": included_files.iter().map(|f| {
                json!({
                    "path": f.path,
                    "priority": format!("{:?}", f.priority),
                    "blake3_pre_redaction": f.blake3_pre_redaction,
                })
            }).collect::<Vec<_>>(),
        });

        // Write manifest as JSON
        let manifest_json = serde_json::to_string_pretty(&manifest)
            .context("Failed to serialize packet manifest")?;

        write_file_atomic(&manifest_path, &manifest_json)
            .with_context(|| format!("Failed to write packet manifest to: {manifest_path}"))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::BudgetUsage;

    #[test]
    fn test_budget_tracking() {
        let mut budget = BudgetUsage::new(1000, 50);

        assert!(!budget.is_exceeded());
        assert!(!budget.would_exceed(500, 25));

        budget.add_content(500, 25);
        assert!(!budget.is_exceeded());
        assert!(budget.would_exceed(600, 10)); // Would exceed bytes
        assert!(budget.would_exceed(400, 30)); // Would exceed lines

        budget.add_content(600, 30);
        assert!(budget.is_exceeded());
    }
}

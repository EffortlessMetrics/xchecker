//! Status output generation for xchecker
//!
//! This module provides functionality to generate structured JSON status outputs
//! with canonical emission using JCS (RFC 8785) for stable diffs across platforms.
//!
//! Note: The CLI currently uses `StatusJsonOutput` (compact format per FR-Claude Code-CLI)
//! for `xchecker status --json`. This module provides `StatusOutput` (full format) which
//! is reserved for future orchestration API and IDE/TUI integration.
//! See FR-STATUS for design rationale.

use anyhow::{Context, Result};
use chrono::Utc;
use std::collections::BTreeMap;
use tracing::warn;

use crate::artifact::ArtifactManager;
use crate::orchestrator::PhaseOrchestrator;
use crate::receipt::ReceiptManager;
use crate::redaction::SecretRedactor;
use crate::types::{ArtifactInfo, ConfigSource, ConfigValue, LockDrift, StatusOutput};

/// Status manager for generating full status outputs (StatusOutput schema).
///
/// Note: The CLI uses `StatusJsonOutput` (compact format) for `--json` output.
/// This manager produces `StatusOutput` (full format) which is reserved for
/// future orchestration API and IDE/TUI integration.
#[cfg_attr(not(test), allow(dead_code))] // Reserved for future orchestration API; CLI uses StatusJsonOutput
pub struct StatusManager;

impl StatusManager {
    /// Generate full status output from an orchestrator.
    ///
    /// Reserved for future orchestration API; not currently used by CLI.
    /// The CLI uses `StatusJsonOutput` (compact format) via its own implementation.
    #[allow(dead_code)] // Reserved for future orchestration API; not currently used by CLI
    pub fn generate_status_from_orchestrator(
        orchestrator: &PhaseOrchestrator,
        effective_config: BTreeMap<String, (String, String)>,
        lock_drift: Option<LockDrift>,
        pending_fixups: Option<crate::types::PendingFixupsSummary>,
        redactor: Option<&SecretRedactor>,
    ) -> Result<StatusOutput> {
        let artifact_manager = orchestrator.artifact_manager();
        let receipt_manager = orchestrator.receipt_manager();

        Self::generate_status_internal(
            artifact_manager,
            receipt_manager,
            effective_config,
            lock_drift,
            pending_fixups,
            redactor,
        )
    }

    /// Internal method to generate full status output.
    ///
    /// Reserved for future orchestration API; not currently used by CLI.
    /// Made public for testing purposes.
    ///
    /// # Warning
    /// This is an internal API and should not be used outside of tests.
    /// Use `generate_status_from_orchestrator` instead.
    #[doc(hidden)]
    #[cfg_attr(not(test), allow(dead_code))] // Reserved for future orchestration API
    pub fn generate_status_internal(
        artifact_manager: &ArtifactManager,
        receipt_manager: &ReceiptManager,
        effective_config: BTreeMap<String, (String, String)>,
        lock_drift: Option<LockDrift>,
        pending_fixups: Option<crate::types::PendingFixupsSummary>,
        redactor: Option<&SecretRedactor>,
    ) -> Result<StatusOutput> {
        // Get the latest receipt to extract runner and canonicalization info
        let receipts = receipt_manager
            .list_receipts()
            .context("Failed to list receipts")?;

        // FR-STA-004: Handle fresh specs with no prior receipts
        let latest_receipt = receipts.last();

        // Get artifacts with hashes (already sorted by path in collect_artifacts)
        let mut artifacts = if latest_receipt.is_some() {
            Self::collect_artifacts(artifact_manager, receipt_manager)?
        } else {
            Vec::new() // No artifacts for fresh spec
        };

        // Ensure artifacts are sorted by path for stable output
        artifacts.sort_by(|a, b| a.path.cmp(&b.path));

        // Get last receipt path if available
        let last_receipt_path = if let Some(receipt) = latest_receipt {
            let receipt_filename = format!(
                "{}-{}.json",
                receipt.phase,
                receipt.emitted_at.format("%Y%m%d_%H%M%S")
            );
            receipt_manager
                .receipts_path()
                .join(&receipt_filename)
                .to_string()
        } else {
            String::new() // No receipt path for fresh spec
        };

        // Build effective config with source attribution
        let effective_config_map = Self::build_effective_config(effective_config, redactor);

        // Use values from latest receipt if available, otherwise use sensible defaults
        Ok(StatusOutput {
            schema_version: "1".to_string(),
            emitted_at: Utc::now(),
            runner: latest_receipt.map_or_else(|| "auto".to_string(), |r| r.runner.clone()),
            runner_distro: latest_receipt.and_then(|r| r.runner_distro.clone()),
            fallback_used: latest_receipt
                .and_then(|r| r.fallback_used)
                .unwrap_or(false),
            canonicalization_version: latest_receipt.map_or_else(
                || "1.0.0".to_string(),
                |r| r.canonicalization_version.clone(),
            ),
            canonicalization_backend: latest_receipt.map_or_else(
                || "jcs-rfc8785".to_string(),
                |r| r.canonicalization_backend.clone(),
            ),
            artifacts,
            last_receipt_path,
            effective_config: effective_config_map,
            lock_drift,
            pending_fixups,
        })
    }

    /// Collect artifacts with their BLAKE3 hashes (first 8 chars).
    ///
    /// Reserved for future orchestration API; not currently used by CLI.
    #[cfg_attr(not(test), allow(dead_code))] // Reserved for future orchestration API
    fn collect_artifacts(
        artifact_manager: &ArtifactManager,
        receipt_manager: &ReceiptManager,
    ) -> Result<Vec<ArtifactInfo>> {
        let artifact_files = artifact_manager
            .list_artifacts()
            .context("Failed to list artifacts")?;

        // Get receipts to extract hashes
        let receipts = receipt_manager
            .list_receipts()
            .context("Failed to list receipts")?;

        // Create a map of artifact paths to their hashes from receipts
        let mut artifact_hashes: BTreeMap<String, String> = BTreeMap::new();
        for receipt in &receipts {
            for output in &receipt.outputs {
                // Extract just the filename from the path for matching
                if let Some(filename) = output.path.split('/').next_back() {
                    let short_hash = if output.blake3_canonicalized.len() >= 8 {
                        &output.blake3_canonicalized[..8]
                    } else {
                        &output.blake3_canonicalized
                    };
                    artifact_hashes.insert(filename.to_string(), short_hash.to_string());
                }
            }
        }

        // Build artifact info list
        let mut artifacts = Vec::new();
        for artifact_file in artifact_files {
            if let Some(hash) = artifact_hashes.get(&artifact_file) {
                artifacts.push(ArtifactInfo {
                    path: format!("artifacts/{artifact_file}"),
                    blake3_first8: hash.clone(),
                });
            }
        }

        // Sort by path for stable output
        artifacts.sort_by(|a, b| a.path.cmp(&b.path));

        Ok(artifacts)
    }

    /// Build effective configuration with source attribution.
    ///
    /// Reserved for future orchestration API; not currently used by CLI.
    #[cfg_attr(not(test), allow(dead_code))] // Reserved for future orchestration API
    fn build_effective_config(
        config_map: BTreeMap<String, (String, String)>,
        redactor: Option<&SecretRedactor>,
    ) -> BTreeMap<String, ConfigValue> {
        let mut effective_config = BTreeMap::new();

        for (key, (value, source_str)) in config_map {
            // Parse source string to ConfigSource enum, accepting legacy labels.
            let normalized = source_str.trim().to_lowercase();
            let source = match normalized.as_str() {
                "cli" => ConfigSource::Cli,
                "config" => ConfigSource::Config,
                "programmatic" => ConfigSource::Programmatic,
                "default" | "defaults" => ConfigSource::Default,
                _ if normalized.contains("config file") => ConfigSource::Config,
                _ => {
                    warn!(
                        key = %key,
                        source_label = %source_str,
                        "Unknown ConfigSource label, falling back to Default"
                    );
                    ConfigSource::Default
                }
            };

            // Redact value if redactor is available
            let redacted_value = if let Some(r) = redactor {
                r.redact_string(&value)
            } else {
                value.clone()
            };

            // Convert value to JSON value
            let json_value = if let Ok(num) = redacted_value.parse::<i64>() {
                serde_json::Value::Number(serde_json::Number::from(num))
            } else if let Ok(boolean) = redacted_value.parse::<bool>() {
                serde_json::Value::Bool(boolean)
            } else {
                serde_json::Value::String(redacted_value)
            };

            effective_config.insert(
                key,
                ConfigValue {
                    value: json_value,
                    source,
                },
            );
        }

        effective_config
    }

    /// Emit full status as canonical JSON using JCS (RFC 8785).
    ///
    /// Reserved for future orchestration API; not currently used by CLI.
    /// The CLI uses its own `emit_status_json` function for `StatusJsonOutput`.
    #[cfg_attr(not(test), allow(dead_code))] // Reserved for future orchestration API
    pub fn emit_json(status: &StatusOutput) -> Result<String> {
        // Serialize to JSON value (artifacts already sorted in generate_status_internal)
        let json_value =
            serde_json::to_value(status).context("Failed to serialize status to JSON value")?;

        // Apply JCS canonicalization
        let json_bytes = serde_json_canonicalizer::to_vec(&json_value)
            .context("Failed to canonicalize status JSON")?;

        let json_string = String::from_utf8(json_bytes)
            .context("Failed to convert canonical JSON to UTF-8 string")?;

        Ok(json_string)
    }

    /// Emit status as pretty-printed JSON (for human readability)
    /// Alternative JSON formatting (vs compact)
    #[allow(dead_code)] // Alternative formatting option
    pub fn emit_json_pretty(status: &StatusOutput) -> Result<String> {
        serde_json::to_string_pretty(status).context("Failed to serialize status to pretty JSON")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{DriftPair, PacketEvidence, PhaseId};
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn create_test_managers_with_id(spec_id: &str) -> (ArtifactManager, ReceiptManager, TempDir) {
        let temp_dir = crate::paths::with_isolated_home();

        let artifact_manager = ArtifactManager::new(spec_id).unwrap();
        let base_path = crate::paths::spec_root(spec_id);
        let receipt_manager = ReceiptManager::new(&base_path);

        (artifact_manager, receipt_manager, temp_dir)
    }

    #[test]
    fn test_generate_status_basic() {
        let (artifact_manager, receipt_manager, _temp_dir) =
            create_test_managers_with_id("test-spec-status-basic");

        // Create a test receipt
        let packet = PacketEvidence {
            files: vec![],
            max_bytes: 65536,
            max_lines: 1200,
        };

        let receipt = receipt_manager.create_receipt(
            "test-spec-status-basic",
            PhaseId::Requirements,
            0,
            vec![],
            "0.1.0",
            "0.8.1",
            "haiku",
            None,
            HashMap::new(),
            packet,
            None,
            None,
            vec![],
            Some(false),
            "native",
            None,
            None,
            None,
            None, // diff_context
            None, // pipeline
        );

        receipt_manager.write_receipt(&receipt).unwrap();

        // Create a minimal effective config
        let mut effective_config = BTreeMap::new();
        effective_config.insert(
            "model".to_string(),
            ("haiku".to_string(), "default".to_string()),
        );
        effective_config.insert(
            "max_turns".to_string(),
            ("6".to_string(), "default".to_string()),
        );

        // Generate status
        let status = StatusManager::generate_status_internal(
            &artifact_manager,
            &receipt_manager,
            effective_config,
            None,
            None, // pending_fixups
            None, // redactor
        )
        .unwrap();

        assert_eq!(status.schema_version, "1");
        assert_eq!(status.runner, "native");
        assert_eq!(status.runner_distro, None);
        assert!(!status.fallback_used);
        assert_eq!(status.canonicalization_version, "yaml-v1,md-v1");
        assert_eq!(status.canonicalization_backend, "jcs-rfc8785");
        assert!(status.last_receipt_path.contains("requirements-"));
        assert!(!status.effective_config.is_empty());
    }

    #[test]
    fn test_emit_json_canonical() {
        let (artifact_manager, receipt_manager, _temp_dir) =
            create_test_managers_with_id("test-spec-emit-json");

        // Create a test receipt
        let packet = PacketEvidence {
            files: vec![],
            max_bytes: 65536,
            max_lines: 1200,
        };

        let receipt = receipt_manager.create_receipt(
            "test-spec-emit-json",
            PhaseId::Requirements,
            0,
            vec![],
            "0.1.0",
            "0.8.1",
            "haiku",
            None,
            HashMap::new(),
            packet,
            None,
            None,
            vec![],
            Some(false),
            "native",
            None,
            None,
            None,
            None, // diff_context
            None, // pipeline
        );

        receipt_manager.write_receipt(&receipt).unwrap();

        // Create a minimal effective config
        let mut effective_config = BTreeMap::new();
        effective_config.insert(
            "model".to_string(),
            ("haiku".to_string(), "default".to_string()),
        );
        effective_config.insert(
            "max_turns".to_string(),
            ("6".to_string(), "default".to_string()),
        );

        // Generate status
        let status = StatusManager::generate_status_internal(
            &artifact_manager,
            &receipt_manager,
            effective_config,
            None,
            None, // pending_fixups
            None, // redactor
        )
        .unwrap();

        // Emit as canonical JSON
        let json = StatusManager::emit_json(&status).unwrap();

        // Verify it's valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["schema_version"], "1");
        assert_eq!(parsed["runner"], "native");

        // Verify canonical JSON properties (no extra whitespace)
        assert!(!json.contains("  "));
        assert!(!json.contains('\n'));
    }

    #[test]
    fn test_status_with_lock_drift() {
        let (artifact_manager, receipt_manager, _temp_dir) =
            create_test_managers_with_id("test-spec-lock-drift");

        // Create a test receipt
        let packet = PacketEvidence {
            files: vec![],
            max_bytes: 65536,
            max_lines: 1200,
        };

        let receipt = receipt_manager.create_receipt(
            "test-spec-lock-drift",
            PhaseId::Requirements,
            0,
            vec![],
            "0.1.0",
            "0.8.1",
            "haiku",
            None,
            HashMap::new(),
            packet,
            None,
            None,
            vec![],
            Some(false),
            "native",
            None,
            None,
            None,
            None, // diff_context
            None, // pipeline
        );

        receipt_manager.write_receipt(&receipt).unwrap();

        // Create a minimal effective config
        let mut effective_config = BTreeMap::new();
        effective_config.insert(
            "model".to_string(),
            ("haiku".to_string(), "default".to_string()),
        );
        effective_config.insert(
            "max_turns".to_string(),
            ("6".to_string(), "default".to_string()),
        );

        // Create lock drift
        let lock_drift = Some(LockDrift {
            model_full_name: Some(DriftPair {
                locked: "haiku".to_string(),
                current: "sonnet".to_string(),
            }),
            claude_cli_version: None,
            schema_version: None,
        });

        // Generate status with drift
        let status = StatusManager::generate_status_internal(
            &artifact_manager,
            &receipt_manager,
            effective_config,
            lock_drift,
            None, // pending_fixups
            None, // redactor
        )
        .unwrap();

        assert!(status.lock_drift.is_some());
        let drift = status.lock_drift.unwrap();
        assert!(drift.model_full_name.is_some());

        let model_drift = drift.model_full_name.unwrap();
        assert_eq!(model_drift.locked, "haiku");
        assert_eq!(model_drift.current, "sonnet");
    }

    #[test]
    fn test_config_source_serialization() {
        // Test that ConfigSource serializes with lowercase
        let source_cli = ConfigSource::Cli;
        let source_config = ConfigSource::Config;
        let source_programmatic = ConfigSource::Programmatic;
        let source_default = ConfigSource::Default;

        let json_cli = serde_json::to_string(&source_cli).unwrap();
        let json_config = serde_json::to_string(&source_config).unwrap();
        let json_programmatic = serde_json::to_string(&source_programmatic).unwrap();
        let json_default = serde_json::to_string(&source_default).unwrap();

        assert_eq!(json_cli, "\"cli\"");
        assert_eq!(json_config, "\"config\"");
        assert_eq!(json_programmatic, "\"programmatic\"");
        assert_eq!(json_default, "\"default\"");
    }

    // ===== Edge Case Tests (Task 9.7) =====

    #[test]
    fn test_status_with_no_artifacts() {
        let (artifact_manager, receipt_manager, _temp_dir) =
            create_test_managers_with_id("test-spec-no-artifacts");

        // Don't create any receipts or artifacts
        let effective_config = BTreeMap::new();

        let status = StatusManager::generate_status_internal(
            &artifact_manager,
            &receipt_manager,
            effective_config,
            None,
            None,
            None,
        )
        .unwrap();

        // Should handle empty state gracefully
        assert_eq!(status.schema_version, "1");
        assert!(status.artifacts.is_empty());
        assert_eq!(status.last_receipt_path, "");
        assert!(status.effective_config.is_empty());
    }

    #[test]
    fn test_status_with_empty_effective_config() {
        let (artifact_manager, receipt_manager, _temp_dir) =
            create_test_managers_with_id("test-spec-empty-config");

        // Create a test receipt
        let packet = PacketEvidence {
            files: vec![],
            max_bytes: 65536,
            max_lines: 1200,
        };

        let receipt = receipt_manager.create_receipt(
            "test-spec-empty-config",
            PhaseId::Requirements,
            0,
            vec![],
            "0.1.0",
            "0.8.1",
            "haiku",
            None,
            HashMap::new(),
            packet,
            None,
            None,
            vec![],
            Some(false),
            "native",
            None,
            None,
            None,
            None, // diff_context
            None, // pipeline
        );

        receipt_manager.write_receipt(&receipt).unwrap();

        // Empty effective config
        let effective_config = BTreeMap::new();

        let status = StatusManager::generate_status_internal(
            &artifact_manager,
            &receipt_manager,
            effective_config,
            None,
            None,
            None,
        )
        .unwrap();

        assert!(status.effective_config.is_empty());
    }

    #[test]
    fn test_status_with_all_drift_types() {
        let (artifact_manager, receipt_manager, _temp_dir) =
            create_test_managers_with_id("test-spec-all-drift");

        // Create a test receipt
        let packet = PacketEvidence {
            files: vec![],
            max_bytes: 65536,
            max_lines: 1200,
        };

        let receipt = receipt_manager.create_receipt(
            "test-spec-all-drift",
            PhaseId::Requirements,
            0,
            vec![],
            "0.1.0",
            "0.8.1",
            "haiku",
            None,
            HashMap::new(),
            packet,
            None,
            None,
            vec![],
            Some(false),
            "native",
            None,
            None,
            None,
            None, // diff_context
            None, // pipeline
        );

        receipt_manager.write_receipt(&receipt).unwrap();

        let effective_config = BTreeMap::new();

        // Create drift for all fields
        let lock_drift = Some(LockDrift {
            model_full_name: Some(DriftPair {
                locked: "haiku".to_string(),
                current: "sonnet".to_string(),
            }),
            claude_cli_version: Some(DriftPair {
                locked: "0.8.1".to_string(),
                current: "0.9.0".to_string(),
            }),
            schema_version: Some(DriftPair {
                locked: "1".to_string(),
                current: "2".to_string(),
            }),
        });

        let status = StatusManager::generate_status_internal(
            &artifact_manager,
            &receipt_manager,
            effective_config,
            lock_drift,
            None,
            None,
        )
        .unwrap();

        assert!(status.lock_drift.is_some());
        let drift = status.lock_drift.unwrap();
        assert!(drift.model_full_name.is_some());
        assert!(drift.claude_cli_version.is_some());
        assert!(drift.schema_version.is_some());
    }

    #[test]
    fn test_status_with_unicode_config_values() {
        let (artifact_manager, receipt_manager, _temp_dir) =
            create_test_managers_with_id("test-spec-unicode-config");

        // Create a test receipt
        let packet = PacketEvidence {
            files: vec![],
            max_bytes: 65536,
            max_lines: 1200,
        };

        let receipt = receipt_manager.create_receipt(
            "test-spec-unicode-config",
            PhaseId::Requirements,
            0,
            vec![],
            "0.1.0",
            "0.8.1",
            "haiku",
            None,
            HashMap::new(),
            packet,
            None,
            None,
            vec![],
            Some(false),
            "native",
            None,
            None,
            None,
            None, // diff_context
            None, // pipeline
        );

        receipt_manager.write_receipt(&receipt).unwrap();

        let mut effective_config = BTreeMap::new();
        effective_config.insert(
            "model".to_string(),
            ("claude-测试-🚀".to_string(), "default".to_string()),
        );
        effective_config.insert(
            "description".to_string(),
            ("説明-日本語-✨".to_string(), "config".to_string()),
        );

        let status = StatusManager::generate_status_internal(
            &artifact_manager,
            &receipt_manager,
            effective_config,
            None,
            None,
            None,
        )
        .unwrap();

        assert!(status.effective_config.contains_key("model"));
        assert!(status.effective_config.contains_key("description"));
    }

    #[test]
    fn test_status_with_very_long_config_values() {
        let (artifact_manager, receipt_manager, _temp_dir) =
            create_test_managers_with_id("test-spec-long-config");

        // Create a test receipt
        let packet = PacketEvidence {
            files: vec![],
            max_bytes: 65536,
            max_lines: 1200,
        };

        let receipt = receipt_manager.create_receipt(
            "test-spec-long-config",
            PhaseId::Requirements,
            0,
            vec![],
            "0.1.0",
            "0.8.1",
            "haiku",
            None,
            HashMap::new(),
            packet,
            None,
            None,
            vec![],
            Some(false),
            "native",
            None,
            None,
            None,
            None, // diff_context
            None, // pipeline
        );

        receipt_manager.write_receipt(&receipt).unwrap();

        let long_value = "a".repeat(10000);
        let mut effective_config = BTreeMap::new();
        effective_config.insert("long_key".to_string(), (long_value, "default".to_string()));

        let status = StatusManager::generate_status_internal(
            &artifact_manager,
            &receipt_manager,
            effective_config,
            None,
            None,
            None,
        )
        .unwrap();

        assert!(status.effective_config.contains_key("long_key"));
    }

    #[test]
    fn test_status_with_special_characters_in_config() {
        let (artifact_manager, receipt_manager, _temp_dir) =
            create_test_managers_with_id("test-spec-special-config");

        // Create a test receipt
        let packet = PacketEvidence {
            files: vec![],
            max_bytes: 65536,
            max_lines: 1200,
        };

        let receipt = receipt_manager.create_receipt(
            "test-spec-special-config",
            PhaseId::Requirements,
            0,
            vec![],
            "0.1.0",
            "0.8.1",
            "haiku",
            None,
            HashMap::new(),
            packet,
            None,
            None,
            vec![],
            Some(false),
            "native",
            None,
            None,
            None,
            None, // diff_context
            None, // pipeline
        );

        receipt_manager.write_receipt(&receipt).unwrap();

        let mut effective_config = BTreeMap::new();
        effective_config.insert(
            "key_with_@#$%".to_string(),
            ("value_with_<>&\"'".to_string(), "default".to_string()),
        );

        let status = StatusManager::generate_status_internal(
            &artifact_manager,
            &receipt_manager,
            effective_config,
            None,
            None,
            None,
        )
        .unwrap();

        assert!(status.effective_config.contains_key("key_with_@#$%"));
    }

    #[test]
    fn test_status_json_emission_with_empty_status() {
        let (artifact_manager, receipt_manager, _temp_dir) =
            create_test_managers_with_id("test-spec-empty-status");

        let effective_config = BTreeMap::new();

        let status = StatusManager::generate_status_internal(
            &artifact_manager,
            &receipt_manager,
            effective_config,
            None,
            None,
            None,
        )
        .unwrap();

        // Should be able to emit as JSON even when empty
        let json = StatusManager::emit_json(&status).unwrap();
        assert!(!json.is_empty());

        // Verify it's valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["schema_version"], "1");
    }

    #[test]
    fn test_status_with_pending_fixups() {
        let (artifact_manager, receipt_manager, _temp_dir) =
            create_test_managers_with_id("test-spec-pending-fixups");

        // Create a test receipt
        let packet = PacketEvidence {
            files: vec![],
            max_bytes: 65536,
            max_lines: 1200,
        };

        let receipt = receipt_manager.create_receipt(
            "test-spec-pending-fixups",
            PhaseId::Requirements,
            0,
            vec![],
            "0.1.0",
            "0.8.1",
            "haiku",
            None,
            HashMap::new(),
            packet,
            None,
            None,
            vec![],
            Some(false),
            "native",
            None,
            None,
            None,
            None, // diff_context
            None, // pipeline
        );

        receipt_manager.write_receipt(&receipt).unwrap();

        let effective_config = BTreeMap::new();

        let pending_fixups = Some(crate::types::PendingFixupsSummary {
            targets: 5,
            est_added: 100,
            est_removed: 50,
        });

        let status = StatusManager::generate_status_internal(
            &artifact_manager,
            &receipt_manager,
            effective_config,
            None,
            pending_fixups,
            None,
        )
        .unwrap();

        assert!(status.pending_fixups.is_some());
        let fixups = status.pending_fixups.unwrap();
        assert_eq!(fixups.targets, 5);
        assert_eq!(fixups.est_added, 100);
        assert_eq!(fixups.est_removed, 50);
    }

    #[test]
    fn test_status_with_zero_pending_fixups() {
        let (artifact_manager, receipt_manager, _temp_dir) =
            create_test_managers_with_id("test-spec-zero-fixups");

        // Create a test receipt
        let packet = PacketEvidence {
            files: vec![],
            max_bytes: 65536,
            max_lines: 1200,
        };

        let receipt = receipt_manager.create_receipt(
            "test-spec-zero-fixups",
            PhaseId::Requirements,
            0,
            vec![],
            "0.1.0",
            "0.8.1",
            "haiku",
            None,
            HashMap::new(),
            packet,
            None,
            None,
            vec![],
            Some(false),
            "native",
            None,
            None,
            None,
            None,
            None, // pipeline
        );

        receipt_manager.write_receipt(&receipt).unwrap();

        let effective_config = BTreeMap::new();

        let pending_fixups = Some(crate::types::PendingFixupsSummary {
            targets: 0,
            est_added: 0,
            est_removed: 0,
        });

        let status = StatusManager::generate_status_internal(
            &artifact_manager,
            &receipt_manager,
            effective_config,
            None,
            pending_fixups,
            None,
        )
        .unwrap();

        assert!(status.pending_fixups.is_some());
        let fixups = status.pending_fixups.unwrap();
        assert_eq!(fixups.targets, 0);
    }

    #[test]
    fn test_build_effective_config_with_different_types() {
        let mut config_map = BTreeMap::new();

        // Test different value types
        config_map.insert(
            "string_value".to_string(),
            ("test".to_string(), "cli".to_string()),
        );
        config_map.insert(
            "int_value".to_string(),
            ("42".to_string(), "config".to_string()),
        );
        config_map.insert(
            "bool_value".to_string(),
            ("true".to_string(), "default".to_string()),
        );
        config_map.insert(
            "negative_int".to_string(),
            ("-10".to_string(), "cli".to_string()),
        );

        let effective_config = StatusManager::build_effective_config(config_map, None);

        // Verify types are correctly parsed
        assert!(matches!(
            effective_config.get("string_value").unwrap().value,
            serde_json::Value::String(_)
        ));
        assert!(matches!(
            effective_config.get("int_value").unwrap().value,
            serde_json::Value::Number(_)
        ));
        assert!(matches!(
            effective_config.get("bool_value").unwrap().value,
            serde_json::Value::Bool(true)
        ));
        assert!(matches!(
            effective_config.get("negative_int").unwrap().value,
            serde_json::Value::Number(_)
        ));
    }

    #[test]
    fn test_build_effective_config_with_empty_map() {
        let config_map = BTreeMap::new();
        let effective_config = StatusManager::build_effective_config(config_map, None);

        assert!(effective_config.is_empty());
    }

    // ===== Edge Case Tests for Task 9.7 =====
    // (Most tests already exist above, keeping only unique ones)

    #[test]
    fn test_status_json_canonical_format() {
        let (artifact_manager, receipt_manager, _temp_dir) =
            create_test_managers_with_id("test-spec-canonical");

        let packet = PacketEvidence {
            files: vec![],
            max_bytes: 65536,
            max_lines: 1200,
        };

        let receipt = receipt_manager.create_receipt(
            "test-spec-canonical",
            PhaseId::Requirements,
            0,
            vec![],
            "0.1.0",
            "0.8.1",
            "haiku",
            None,
            HashMap::new(),
            packet,
            None,
            None,
            vec![],
            Some(false),
            "native",
            None,
            None,
            None,
            None,
            None, // pipeline
        );

        receipt_manager.write_receipt(&receipt).unwrap();

        let mut effective_config = BTreeMap::new();
        effective_config.insert(
            "model".to_string(),
            ("haiku".to_string(), "default".to_string()),
        );

        let status = StatusManager::generate_status_internal(
            &artifact_manager,
            &receipt_manager,
            effective_config,
            None,
            None,
            None,
        )
        .unwrap();

        // Emit as JSON twice
        let json1 = StatusManager::emit_json(&status).unwrap();
        let json2 = StatusManager::emit_json(&status).unwrap();

        // Should be byte-identical (canonical)
        assert_eq!(json1, json2);

        // Should not have pretty-printing whitespace
        assert!(!json1.contains("  "));
        assert!(!json1.contains('\n'));
    }

    #[test]
    fn test_config_source_enum_serialization() {
        // Test that ConfigSource enum serializes correctly
        let cli = ConfigSource::Cli;
        let config = ConfigSource::Config;
        let programmatic = ConfigSource::Programmatic;
        let default = ConfigSource::Default;

        let json_cli = serde_json::to_string(&cli).unwrap();
        let json_config = serde_json::to_string(&config).unwrap();
        let json_programmatic = serde_json::to_string(&programmatic).unwrap();
        let json_default = serde_json::to_string(&default).unwrap();

        assert_eq!(json_cli, "\"cli\"");
        assert_eq!(json_config, "\"config\"");
        assert_eq!(json_programmatic, "\"programmatic\"");
        assert_eq!(json_default, "\"default\"");
    }
}

//! Status output generation for xchecker.
//!
//! This module provides functionality to generate structured JSON status outputs
//! with canonical emission using JCS (RFC 8785) for stable diffs across platforms.
//!
//! Note: The CLI uses `StatusJsonOutput` (compact format per FR-Claude Code-CLI)
//! for `xchecker status --json`. This module provides `StatusOutput` (full format)
//! which is reserved for orchestration APIs and IDE/TUI integration.

use anyhow::{Context, Result};
use chrono::Utc;
use std::collections::BTreeMap;

use crate::artifact::ArtifactManager;
use crate::receipt::ReceiptManager;
use crate::types::{ArtifactInfo, ConfigSource, ConfigValue, LockDrift, StatusOutput};

/// Status manager for generating full status outputs (StatusOutput schema).
#[cfg_attr(not(test), allow(dead_code))] // Reserved for orchestration APIs; CLI uses StatusJsonOutput
pub struct StatusManager;

impl StatusManager {
    /// Generate full status output from an orchestrator.
    ///
    /// Reserved for orchestration APIs; not currently used by CLI.
    #[allow(dead_code)] // Reserved for orchestration APIs; not currently used by CLI
    pub fn generate_status_from_orchestrator(
        artifact_manager: &ArtifactManager,
        receipt_manager: &ReceiptManager,
        effective_config: BTreeMap<String, (String, String)>,
        lock_drift: Option<LockDrift>,
        pending_fixups: Option<crate::types::PendingFixupsSummary>,
    ) -> Result<StatusOutput> {
        Self::generate_status_internal(
            artifact_manager,
            receipt_manager,
            effective_config,
            lock_drift,
            pending_fixups,
            None,
        )
    }

    /// Internal method to generate full status output.
    ///
    /// Reserved for orchestration APIs; made public for tests.
    ///
    /// # Warning
    /// This is an internal API and should not be used outside of tests.
    #[doc(hidden)]
    #[cfg_attr(not(test), allow(dead_code))] // Reserved for orchestration APIs
    pub fn generate_status_internal(
        artifact_manager: &ArtifactManager,
        receipt_manager: &ReceiptManager,
        effective_config: BTreeMap<String, (String, String)>,
        lock_drift: Option<LockDrift>,
        pending_fixups: Option<crate::types::PendingFixupsSummary>,
        secret_redactor: Option<&crate::redaction::SecretRedactor>,
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
        // Determine runner defaults for fresh specs (no receipts).
        // Schema only allows "native" or "wsl", so map "auto" to "native".
        let (runner, runner_distro) = if let Some(receipt) = latest_receipt {
            (receipt.runner.clone(), receipt.runner_distro.clone())
        } else {
            let runner_mode = effective_config
                .get("runner_mode")
                .map(|(value, _)| value.as_str())
                .unwrap_or("native");
            let is_wsl = runner_mode.eq_ignore_ascii_case("wsl");
            let runner = if is_wsl { "wsl" } else { "native" };
            let runner_distro = if is_wsl {
                effective_config
                    .get("runner_distro")
                    .map(|(value, _)| value.clone())
            } else {
                None
            };
            (runner.to_string(), runner_distro)
        };

        let effective_config_map = Self::build_effective_config(effective_config, secret_redactor);

        // Use values from latest receipt if available, otherwise use sensible defaults
        Ok(StatusOutput {
            schema_version: "1".to_string(),
            emitted_at: Utc::now(),
            runner,
            runner_distro,
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
    #[cfg_attr(not(test), allow(dead_code))] // Reserved for orchestration APIs
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

        // Create a map of artifact filenames to their hashes from receipts
        let mut artifact_hashes: BTreeMap<String, String> = BTreeMap::new();
        for receipt in &receipts {
            for output in &receipt.outputs {
                if let Some(filename) = std::path::Path::new(&output.path)
                    .file_name()
                    .and_then(|s| s.to_str())
                {
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
    #[cfg_attr(not(test), allow(dead_code))] // Reserved for orchestration APIs
    fn build_effective_config(
        config_map: BTreeMap<String, (String, String)>,
        secret_redactor: Option<&crate::redaction::SecretRedactor>,
    ) -> BTreeMap<String, ConfigValue> {
        let mut effective_config = BTreeMap::new();

        for (key, (value, source_str)) in config_map {
            let source = match source_str.to_ascii_lowercase().as_str() {
                "cli" => ConfigSource::Cli,
                "env" => ConfigSource::Env,
                "config" => ConfigSource::Config,
                "programmatic" => ConfigSource::Programmatic,
                "default" => ConfigSource::Default,
                _ => ConfigSource::Default,
            };

            let redacted_value = match secret_redactor {
                Some(redactor) => redactor.redact_string(&value),
                None => value,
            };

            let json_value = if let Ok(num) = redacted_value.parse::<i64>() {
                serde_json::Value::Number(num.into())
            } else if redacted_value.eq_ignore_ascii_case("true")
                || redacted_value.eq_ignore_ascii_case("false")
            {
                serde_json::Value::Bool(redacted_value.eq_ignore_ascii_case("true"))
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
    #[cfg_attr(not(test), allow(dead_code))] // Reserved for orchestration APIs
    pub fn emit_json(status: &StatusOutput) -> Result<String> {
        crate::canonicalization::emit_jcs(status).context("Failed to emit status JSON")
    }

    /// Emit status as pretty-printed JSON (for human readability).
    #[allow(dead_code)] // Alternative formatting option
    pub fn emit_json_pretty(status: &StatusOutput) -> Result<String> {
        serde_json::to_string_pretty(status).context("Failed to serialize status to pretty JSON")
    }
}

/// Generate status output for a spec.
///
/// This is a compatibility wrapper around [`StatusManager`].
#[allow(dead_code)] // Public wrapper retained for compatibility
pub fn generate_status(
    spec_id: &str,
    effective_config: Option<&BTreeMap<String, (String, String)>>,
    lock_drift: Option<LockDrift>,
    pending_fixups: Option<crate::types::PendingFixupsSummary>,
) -> Result<StatusOutput, crate::error::XCheckerError> {
    let artifact_manager = ArtifactManager::new_readonly(spec_id).map_err(|e| {
        crate::error::XCheckerError::Config(crate::error::ConfigError::DiscoveryFailed {
            reason: format!("Failed to create artifact manager: {e}"),
        })
    })?;

    let base_path = crate::paths::spec_root(spec_id);
    let receipt_manager = ReceiptManager::new(&base_path);

    let config_map = effective_config.cloned().unwrap_or_default();
    StatusManager::generate_status_internal(
        &artifact_manager,
        &receipt_manager,
        config_map,
        lock_drift,
        pending_fixups,
        None,
    )
    .map_err(|e| {
        crate::error::XCheckerError::Config(crate::error::ConfigError::DiscoveryFailed {
            reason: format!("Failed to generate status: {e}"),
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_manager_creation() {
        // Test that StatusManager can be instantiated
        let _manager = StatusManager;
    }

    #[test]
    fn test_generate_status_from_orchestrator_fresh_spec() {
        // Test status generation for a fresh spec with no receipts
        let temp_dir = crate::paths::with_isolated_home();
        let spec_id = "test-spec-fresh";

        let artifact_manager = ArtifactManager::new(spec_id).unwrap();
        let base_path = crate::paths::spec_root(spec_id);
        let receipt_manager = ReceiptManager::new(&base_path);

        let effective_config = BTreeMap::new();
        let lock_drift = None;
        let pending_fixups = None;

        let result = StatusManager::generate_status_from_orchestrator(
            &artifact_manager,
            &receipt_manager,
            effective_config,
            lock_drift,
            pending_fixups,
        );

        assert!(result.is_ok());
        let status = result.unwrap();

        assert_eq!(status.schema_version, "1");
        assert_eq!(status.artifacts.len(), 0);
        assert_eq!(status.last_receipt_path, "");
        assert_eq!(status.runner, "native"); // Default for fresh spec
        assert!(status.runner_distro.is_none());
        assert!(!status.fallback_used);
        assert_eq!(status.canonicalization_version, "1.0.0");
        assert_eq!(status.canonicalization_backend, "jcs-rfc8785");

        drop(temp_dir);
    }

    #[test]
    fn test_build_effective_config() {
        let mut config_map = BTreeMap::new();
        config_map.insert(
            "key1".to_string(),
            ("value1".to_string(), "cli".to_string()),
        );
        config_map.insert(
            "key2".to_string(),
            ("value2".to_string(), "config".to_string()),
        );
        config_map.insert("key3".to_string(), ("123".to_string(), "env".to_string()));
        config_map.insert(
            "key4".to_string(),
            ("true".to_string(), "default".to_string()),
        );

        let result = StatusManager::build_effective_config(config_map, None);

        assert_eq!(result.len(), 4);

        let key1 = result.get("key1").unwrap();
        assert_eq!(key1.source, ConfigSource::Cli);
        assert_eq!(key1.value, serde_json::json!("value1"));

        let key2 = result.get("key2").unwrap();
        assert_eq!(key2.source, ConfigSource::Config);
        assert_eq!(key2.value, serde_json::json!("value2"));

        let key3 = result.get("key3").unwrap();
        assert_eq!(key3.source, ConfigSource::Env);
        assert_eq!(key3.value, serde_json::json!(123));

        let key4 = result.get("key4").unwrap();
        assert_eq!(key4.source, ConfigSource::Default);
        assert_eq!(key4.value, serde_json::json!(true));
    }

    #[test]
    fn test_emit_json() {
        let status = StatusOutput {
            schema_version: "1".to_string(),
            emitted_at: Utc::now(),
            runner: "native".to_string(),
            runner_distro: None,
            fallback_used: false,
            canonicalization_version: "1.0.0".to_string(),
            canonicalization_backend: "jcs-rfc8785".to_string(),
            artifacts: Vec::new(),
            last_receipt_path: "receipts/test.json".to_string(),
            effective_config: BTreeMap::new(),
            lock_drift: None,
            pending_fixups: None,
        };

        let result = StatusManager::emit_json(&status);

        assert!(result.is_ok());
        let json_str = result.unwrap();

        // Verify it's valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["schema_version"], "1");
        assert_eq!(parsed["runner"], "native");
    }
}

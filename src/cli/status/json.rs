use std::collections::BTreeMap;

use anyhow::{Context, Result};

use crate::lock::{RunContext, XCheckerLock};
use crate::types::{
    ArtifactInfo, ConfigSource, ConfigValue, PhaseStatusInfo, Receipt, StatusJsonOutput,
};
use crate::{Config, OrchestratorHandle, PhaseId};

use super::{ALL_PHASES, count_pending_fixups_for_spec, emit_status_json};

/// Emit the machine-readable status-json.v2 format with full details.
pub(super) fn execute_status_json(
    spec_id: &str,
    handle: &OrchestratorHandle,
    config: &Config,
) -> Result<()> {
    let receipts = handle.receipt_manager().list_receipts().unwrap_or_default();
    let (phase_statuses, has_errors) = build_phase_statuses(&receipts);
    let artifacts = collect_artifacts(handle, &receipts);

    let output = StatusJsonOutput {
        schema_version: "status-json.v2".to_string(),
        spec_id: spec_id.to_string(),
        phase_statuses,
        pending_fixups: count_pending_fixups_for_spec(spec_id),
        has_errors,
        strict_validation: config.strict_validation(),
        artifacts,
        effective_config: build_effective_config(config),
        lock_drift: detect_lock_drift(spec_id, &receipts, config),
    };

    let json_output = emit_status_json(&output).with_context(|| "Failed to emit status JSON")?;
    println!("{json_output}");
    Ok(())
}

fn build_phase_statuses(receipts: &[Receipt]) -> (Vec<PhaseStatusInfo>, bool) {
    let mut phase_statuses = Vec::new();
    let mut has_errors = false;

    for phase_id in &ALL_PHASES {
        let latest_receipt = latest_receipt_for_phase(receipts, *phase_id);
        let (status, receipt_id) = match latest_receipt {
            Some(receipt) if receipt.exit_code == 0 => {
                ("success".to_string(), Some(receipt_id(receipt)))
            }
            Some(receipt) => {
                has_errors = true;
                ("failed".to_string(), Some(receipt_id(receipt)))
            }
            None => ("not_started".to_string(), None),
        };

        phase_statuses.push(PhaseStatusInfo {
            phase_id: phase_id.as_str().to_string(),
            status,
            receipt_id,
        });
    }

    (phase_statuses, has_errors)
}

fn latest_receipt_for_phase(receipts: &[Receipt], phase_id: PhaseId) -> Option<&Receipt> {
    receipts
        .iter()
        .filter(|receipt| receipt.phase == phase_id.as_str())
        .max_by_key(|receipt| receipt.emitted_at)
}

fn receipt_id(receipt: &Receipt) -> String {
    format!(
        "{}-{}",
        receipt.phase,
        receipt.emitted_at.format("%Y%m%d_%H%M%S")
    )
}

fn collect_artifacts(handle: &OrchestratorHandle, receipts: &[Receipt]) -> Vec<ArtifactInfo> {
    let artifact_hashes = artifact_hashes_by_filename(receipts);
    let artifact_files = handle
        .artifact_manager()
        .list_artifacts()
        .unwrap_or_default();

    let mut artifacts: Vec<ArtifactInfo> = artifact_files
        .iter()
        .filter_map(|filename| {
            artifact_hashes.get(filename).map(|hash| ArtifactInfo {
                path: format!("artifacts/{filename}"),
                blake3_first8: hash.clone(),
            })
        })
        .collect();
    artifacts.sort_by(|a, b| a.path.cmp(&b.path));
    artifacts
}

fn artifact_hashes_by_filename(receipts: &[Receipt]) -> BTreeMap<String, String> {
    let mut artifact_hashes = BTreeMap::new();
    for receipt in receipts {
        for output in &receipt.outputs {
            if let Some(filename) = output.path.split('/').next_back() {
                artifact_hashes.insert(filename.to_string(), first_8(&output.blake3_canonicalized));
            }
        }
    }
    artifact_hashes
}

fn first_8(value: &str) -> String {
    value.chars().take(8).collect()
}

fn build_effective_config(config: &Config) -> BTreeMap<String, ConfigValue> {
    let mut effective_config = BTreeMap::new();

    insert_string_config(
        &mut effective_config,
        config,
        "provider",
        config.llm.provider.as_deref(),
    );
    insert_string_config(
        &mut effective_config,
        config,
        "model",
        config.defaults.model.as_deref(),
    );
    insert_number_config(
        &mut effective_config,
        config,
        "max_turns",
        config.defaults.max_turns,
    );
    insert_number_config(
        &mut effective_config,
        config,
        "phase_timeout",
        config.defaults.phase_timeout,
    );
    insert_string_config(
        &mut effective_config,
        config,
        "execution_strategy",
        config.llm.execution_strategy.as_deref(),
    );

    effective_config
}

fn insert_string_config(
    effective_config: &mut BTreeMap<String, ConfigValue>,
    config: &Config,
    key: &str,
    value: Option<&str>,
) {
    if let Some(value) = value {
        insert_config_value(
            effective_config,
            config,
            key,
            serde_json::Value::String(value.to_string()),
        );
    }
}

fn insert_number_config<T>(
    effective_config: &mut BTreeMap<String, ConfigValue>,
    config: &Config,
    key: &str,
    value: Option<T>,
) where
    T: Into<u64>,
{
    if let Some(value) = value {
        insert_config_value(
            effective_config,
            config,
            key,
            serde_json::Value::Number(value.into().into()),
        );
    }
}

fn insert_config_value(
    effective_config: &mut BTreeMap<String, ConfigValue>,
    config: &Config,
    key: &str,
    value: serde_json::Value,
) {
    let source = config
        .source_attribution
        .get(key)
        .cloned()
        .unwrap_or(ConfigSource::Config);
    effective_config.insert(key.to_string(), ConfigValue { value, source });
}

fn detect_lock_drift(
    spec_id: &str,
    receipts: &[Receipt],
    config: &Config,
) -> Option<crate::lock::LockDrift> {
    let Ok(Some(lock)) = XCheckerLock::load(spec_id) else {
        return None;
    };

    let model_full_name = receipts
        .last()
        .map(|r| r.model_full_name.clone())
        .unwrap_or_else(|| config.defaults.model.clone().unwrap_or_default());

    let claude_cli_version = receipts
        .last()
        .map(|r| r.claude_cli_version.clone())
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());

    let context = RunContext {
        model_full_name,
        claude_cli_version,
        schema_version: "1".to_string(),
    };

    lock.detect_drift(&context)
}

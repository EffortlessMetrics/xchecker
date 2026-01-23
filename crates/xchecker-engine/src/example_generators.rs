//! Example generators for schema validation
//!
//! This module provides constructors for generating minimal and full examples
//! of xchecker's JSON output formats (receipt, status, doctor). These examples
//! are used for schema validation and documentation.
//!
//! All examples use:
//! - Fixed timestamps for deterministic output
//! - `BTreeMap` for deterministic key ordering
//! - Sorted arrays (by path for outputs/artifacts, by name for checks)
//! - Pinned tool versions for byte-identical assertions

use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use std::collections::{BTreeMap, HashMap};

use crate::doctor::{CheckStatus, DoctorCheck, DoctorOutput};
use crate::types::{
    ArtifactInfo, ConfigSource, ConfigValue, DriftPair, FileEvidence, FileHash, LlmInfo, LockDrift,
    PacketEvidence, PipelineInfo, Priority, Receipt, StatusOutput,
};

/// Fixed timestamp for deterministic examples
/// Only available in test builds to avoid accidental use in production
#[must_use]
pub fn fixed_now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2025-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc)
}

/// Generate minimal receipt example (required fields only)
/// Uses fixed timestamp for deterministic output
#[must_use]
pub fn make_example_receipt_minimal() -> Receipt {
    Receipt {
        schema_version: "1".to_string(),
        emitted_at: fixed_now(),
        spec_id: "example-spec".to_string(),
        phase: "requirements".to_string(),
        xchecker_version: "0.1.0".to_string(),
        claude_cli_version: "0.8.1".to_string(),
        model_full_name: "haiku".to_string(),
        model_alias: None,
        canonicalization_version: "yaml-v1,md-v1".to_string(),
        canonicalization_backend: "jcs-rfc8785".to_string(),
        flags: HashMap::new(),
        runner: "native".to_string(),
        runner_distro: None,
        packet: PacketEvidence {
            files: vec![],
            max_bytes: 100000,
            max_lines: 5000,
        },
        outputs: vec![],
        exit_code: 0,
        error_kind: None,
        error_reason: None,
        stderr_tail: None,
        stderr_redacted: None,
        warnings: vec![],
        fallback_used: None,
        diff_context: None,
        llm: None,
        pipeline: None,
    }
}

/// Generate full receipt example (all fields populated)
/// Uses `BTreeMap` for flags and sorts arrays for deterministic output
#[must_use]
pub fn make_example_receipt_full() -> Receipt {
    let mut flags = HashMap::new();
    flags.insert("dry_run".to_string(), "true".to_string());
    flags.insert("strict_lock".to_string(), "false".to_string());

    let mut outputs = vec![
        FileHash {
            path: "artifacts/10-design.md".to_string(),
            blake3_canonicalized:
                "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210".to_string(),
        },
        FileHash {
            path: "artifacts/00-requirements.md".to_string(),
            blake3_canonicalized:
                "abc1234567890abcabc1234567890abcabc1234567890abcabc1234567890abc".to_string(),
        },
    ];
    // Sort by path for deterministic output
    outputs.sort_by(|a, b| a.path.cmp(&b.path));

    let mut packet_files = vec![
        FileEvidence {
            path: "specs/example-spec/requirements.md".to_string(),
            range: Some("L1-L80".to_string()),
            blake3_pre_redaction:
                "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string(),
            priority: Priority::High,
        },
        FileEvidence {
            path: "README.md".to_string(),
            range: None,
            blake3_pre_redaction:
                "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890".to_string(),
            priority: Priority::Medium,
        },
    ];
    // Sort by path for deterministic output
    packet_files.sort_by(|a, b| a.path.cmp(&b.path));

    Receipt {
        schema_version: "1".to_string(),
        emitted_at: fixed_now(),
        spec_id: "example-spec".to_string(),
        phase: "design".to_string(),
        xchecker_version: "0.1.0".to_string(),
        claude_cli_version: "0.8.1".to_string(),
        model_full_name: "haiku".to_string(),
        model_alias: Some("sonnet".to_string()),
        canonicalization_version: "yaml-v1,md-v1".to_string(),
        canonicalization_backend: "jcs-rfc8785".to_string(),
        flags,
        runner: "wsl".to_string(),
        runner_distro: Some("Ubuntu-22.04".to_string()),
        packet: PacketEvidence {
            files: packet_files,
            max_bytes: 100000,
            max_lines: 5000,
        },
        outputs,
        exit_code: 0,
        error_kind: None,
        error_reason: Some("Warning: large packet".to_string()),
        stderr_tail: Some("Warning: packet size approaching limit".to_string()),
        stderr_redacted: Some(
            "Warning: packet size approaching limit (secrets redacted)".to_string(),
        ),
        warnings: vec!["rename_retry_count: 2".to_string()],
        fallback_used: Some(true),
        diff_context: Some(3),
        llm: Some(LlmInfo {
            provider: Some("claude-cli".to_string()),
            model_used: Some("haiku".to_string()),
            tokens_input: Some(1234),
            tokens_output: Some(567),
            timed_out: Some(false),
            timeout_seconds: Some(600),
            budget_exhausted: None,
        }),
        pipeline: Some(PipelineInfo {
            execution_strategy: Some("controlled".to_string()),
        }),
    }
}

/// Generate minimal status example (required fields only)
#[must_use]
pub fn make_example_status_minimal() -> StatusOutput {
    StatusOutput {
        schema_version: "1".to_string(),
        emitted_at: fixed_now(),
        runner: "native".to_string(),
        runner_distro: None,
        fallback_used: false,
        canonicalization_version: "yaml-v1,md-v1".to_string(),
        canonicalization_backend: "jcs-rfc8785".to_string(),
        artifacts: vec![],
        last_receipt_path: ".xchecker/receipts/example-spec/requirements.json".to_string(),
        effective_config: BTreeMap::new(),
        lock_drift: None,
        pending_fixups: None,
    }
}

/// Generate full status example (all fields populated)
#[must_use]
pub fn make_example_status_full() -> StatusOutput {
    let mut artifacts = vec![
        ArtifactInfo {
            path: "artifacts/10-design.md".to_string(),
            blake3_first8: "fedcba98".to_string(),
        },
        ArtifactInfo {
            path: "artifacts/00-requirements.md".to_string(),
            blake3_first8: "abc12345".to_string(),
        },
    ];
    // Sort by path for deterministic output
    artifacts.sort_by(|a, b| a.path.cmp(&b.path));

    let mut effective_config = BTreeMap::new();
    effective_config.insert(
        "max_packet_bytes".to_string(),
        ConfigValue {
            value: JsonValue::Number(100000.into()),
            source: ConfigSource::Default,
        },
    );
    effective_config.insert(
        "model".to_string(),
        ConfigValue {
            value: JsonValue::String("haiku".to_string()),
            source: ConfigSource::Config,
        },
    );
    effective_config.insert(
        "strict_lock".to_string(),
        ConfigValue {
            value: JsonValue::Bool(true),
            source: ConfigSource::Cli,
        },
    );

    StatusOutput {
        schema_version: "1".to_string(),
        emitted_at: fixed_now(),
        runner: "wsl".to_string(),
        runner_distro: Some("Ubuntu-22.04".to_string()),
        fallback_used: true,
        canonicalization_version: "yaml-v1,md-v1".to_string(),
        canonicalization_backend: "jcs-rfc8785".to_string(),
        artifacts,
        last_receipt_path: ".xchecker/receipts/example-spec/design.json".to_string(),
        effective_config,
        lock_drift: Some(LockDrift {
            model_full_name: Some(DriftPair {
                locked: "haiku".to_string(),
                current: "sonnet".to_string(),
            }),
            claude_cli_version: Some(DriftPair {
                locked: "0.8.1".to_string(),
                current: "0.9.0".to_string(),
            }),
            schema_version: None,
        }),
        pending_fixups: Some(crate::types::PendingFixupsSummary {
            targets: 3,
            est_added: 42,
            est_removed: 15,
        }),
    }
}

/// Generate minimal doctor example (basic checks)
#[must_use]
pub fn make_example_doctor_minimal() -> DoctorOutput {
    let mut checks = vec![
        DoctorCheck {
            name: "claude_path".to_string(),
            status: CheckStatus::Pass,
            details: "Found claude at /usr/local/bin/claude".to_string(),
        },
        DoctorCheck {
            name: "config_parse".to_string(),
            status: CheckStatus::Pass,
            details: "Configuration parsed and validated successfully".to_string(),
        },
    ];
    // Sort by name for deterministic output
    checks.sort_by(|a, b| a.name.cmp(&b.name));

    DoctorOutput {
        schema_version: "1".to_string(),
        emitted_at: fixed_now(),
        ok: true,
        checks,
        cache_stats: None,
    }
}

/// Generate full doctor example (all check types)
#[must_use]
pub fn make_example_doctor_full() -> DoctorOutput {
    let mut checks = vec![
        DoctorCheck {
            name: "claude_path".to_string(),
            status: CheckStatus::Pass,
            details: "Found claude at /usr/local/bin/claude".to_string(),
        },
        DoctorCheck {
            name: "claude_version".to_string(),
            status: CheckStatus::Pass,
            details: "0.8.1".to_string(),
        },
        DoctorCheck {
            name: "runner_selection".to_string(),
            status: CheckStatus::Pass,
            details: "Runner mode: native (spawn claude directly)".to_string(),
        },
        DoctorCheck {
            name: "wsl_availability".to_string(),
            status: CheckStatus::Warn,
            details: "WSL not installed or not available".to_string(),
        },
        DoctorCheck {
            name: "wsl_default_distro".to_string(),
            status: CheckStatus::Pass,
            details: "Default WSL distro: Ubuntu-22.04".to_string(),
        },
        DoctorCheck {
            name: "write_permissions".to_string(),
            status: CheckStatus::Pass,
            details: ".xchecker directory is writable".to_string(),
        },
        DoctorCheck {
            name: "atomic_rename".to_string(),
            status: CheckStatus::Pass,
            details: "Atomic rename works on same volume".to_string(),
        },
        DoctorCheck {
            name: "config_parse".to_string(),
            status: CheckStatus::Pass,
            details: "Configuration parsed and validated successfully".to_string(),
        },
    ];
    // Sort by name for deterministic output
    checks.sort_by(|a, b| a.name.cmp(&b.name));

    DoctorOutput {
        schema_version: "1".to_string(),
        emitted_at: fixed_now(),
        ok: true,
        checks,
        cache_stats: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_now_returns_consistent_timestamp() {
        let ts1 = fixed_now();
        let ts2 = fixed_now();
        assert_eq!(ts1, ts2);
        assert_eq!(ts1.to_rfc3339(), "2025-01-01T00:00:00+00:00");
    }

    #[test]
    fn test_receipt_minimal_has_required_fields() {
        let receipt = make_example_receipt_minimal();
        assert_eq!(receipt.schema_version, "1");
        assert_eq!(receipt.spec_id, "example-spec");
        assert_eq!(receipt.phase, "requirements");
        assert_eq!(receipt.xchecker_version, "0.1.0");
        assert_eq!(receipt.claude_cli_version, "0.8.1");
        assert_eq!(receipt.runner, "native");
        assert_eq!(receipt.exit_code, 0);
        assert!(receipt.model_alias.is_none());
        assert!(receipt.runner_distro.is_none());
        assert!(receipt.error_kind.is_none());
        assert!(receipt.error_reason.is_none());
        assert!(receipt.stderr_tail.is_none());
        assert!(receipt.fallback_used.is_none());
    }

    #[test]
    fn test_receipt_full_has_all_fields() {
        let receipt = make_example_receipt_full();
        assert_eq!(receipt.schema_version, "1");
        assert!(receipt.model_alias.is_some());
        assert!(receipt.runner_distro.is_some());
        assert!(receipt.error_reason.is_some());
        assert!(receipt.stderr_tail.is_some());
        assert!(receipt.fallback_used.is_some());
        assert!(!receipt.flags.is_empty());
        assert!(!receipt.outputs.is_empty());
        assert!(!receipt.warnings.is_empty());
        assert!(!receipt.packet.files.is_empty());
    }

    #[test]
    fn test_receipt_outputs_sorted_by_path() {
        let receipt = make_example_receipt_full();
        let paths: Vec<&str> = receipt.outputs.iter().map(|o| o.path.as_str()).collect();
        let mut sorted_paths = paths.clone();
        sorted_paths.sort();
        assert_eq!(paths, sorted_paths, "Outputs should be sorted by path");
    }

    #[test]
    fn test_receipt_packet_files_sorted_by_path() {
        let receipt = make_example_receipt_full();
        let paths: Vec<&str> = receipt
            .packet
            .files
            .iter()
            .map(|f| f.path.as_str())
            .collect();
        let mut sorted_paths = paths.clone();
        sorted_paths.sort();
        assert_eq!(paths, sorted_paths, "Packet files should be sorted by path");
    }

    #[test]
    fn test_receipt_blake3_format() {
        let receipt = make_example_receipt_full();
        // Check that blake3_canonicalized is 64 hex chars
        for output in &receipt.outputs {
            assert_eq!(
                output.blake3_canonicalized.len(),
                64,
                "blake3_canonicalized should be 64 chars"
            );
            assert!(
                output
                    .blake3_canonicalized
                    .chars()
                    .all(|c| c.is_ascii_hexdigit()),
                "blake3_canonicalized should be hex"
            );
        }
    }

    #[test]
    fn test_status_minimal_has_required_fields() {
        let status = make_example_status_minimal();
        assert_eq!(status.schema_version, "1");
        assert_eq!(status.runner, "native");
        assert!(!status.fallback_used);
        assert!(status.runner_distro.is_none());
        assert!(status.lock_drift.is_none());
        assert!(status.artifacts.is_empty());
        assert!(status.effective_config.is_empty());
    }

    #[test]
    fn test_status_full_has_all_fields() {
        let status = make_example_status_full();
        assert_eq!(status.schema_version, "1");
        assert!(status.runner_distro.is_some());
        assert!(status.fallback_used);
        assert!(status.lock_drift.is_some());
        assert!(!status.artifacts.is_empty());
        assert!(!status.effective_config.is_empty());
    }

    #[test]
    fn test_status_artifacts_sorted_by_path() {
        let status = make_example_status_full();
        let paths: Vec<&str> = status.artifacts.iter().map(|a| a.path.as_str()).collect();
        let mut sorted_paths = paths.clone();
        sorted_paths.sort();
        assert_eq!(paths, sorted_paths, "Artifacts should be sorted by path");
    }

    #[test]
    fn test_status_blake3_first8_format() {
        let status = make_example_status_full();
        // Check that blake3_first8 is 8 hex chars
        for artifact in &status.artifacts {
            assert_eq!(
                artifact.blake3_first8.len(),
                8,
                "blake3_first8 should be 8 chars"
            );
            assert!(
                artifact
                    .blake3_first8
                    .chars()
                    .all(|c| c.is_ascii_hexdigit()),
                "blake3_first8 should be hex"
            );
        }
    }

    #[test]
    fn test_status_effective_config_uses_btreemap() {
        let status = make_example_status_full();
        // BTreeMap ensures deterministic key order
        let keys: Vec<&String> = status.effective_config.keys().collect();
        let mut sorted_keys = keys.clone();
        sorted_keys.sort();
        assert_eq!(keys, sorted_keys, "Config keys should be sorted (BTreeMap)");
    }

    #[test]
    fn test_doctor_minimal_has_required_fields() {
        let doctor = make_example_doctor_minimal();
        assert_eq!(doctor.schema_version, "1");
        assert!(doctor.ok);
        assert!(!doctor.checks.is_empty());
    }

    #[test]
    fn test_doctor_full_has_all_check_types() {
        let doctor = make_example_doctor_full();
        assert_eq!(doctor.schema_version, "1");
        assert!(doctor.ok);
        assert!(doctor.checks.len() >= 5, "Should have multiple checks");

        // Verify we have different status types
        let has_pass = doctor.checks.iter().any(|c| c.status == CheckStatus::Pass);
        let has_warn = doctor.checks.iter().any(|c| c.status == CheckStatus::Warn);
        assert!(has_pass, "Should have at least one Pass check");
        assert!(has_warn, "Should have at least one Warn check");
    }

    #[test]
    fn test_doctor_checks_sorted_by_name() {
        let doctor = make_example_doctor_full();
        let names: Vec<&str> = doctor.checks.iter().map(|c| c.name.as_str()).collect();
        let mut sorted_names = names.clone();
        sorted_names.sort();
        assert_eq!(names, sorted_names, "Checks should be sorted by name");
    }

    #[test]
    fn test_all_examples_use_fixed_timestamp() {
        let receipt = make_example_receipt_minimal();
        let status = make_example_status_minimal();
        let doctor = make_example_doctor_minimal();

        assert_eq!(receipt.emitted_at, fixed_now());
        assert_eq!(status.emitted_at, fixed_now());
        assert_eq!(doctor.emitted_at, fixed_now());
    }

    #[test]
    fn test_pinned_tool_versions() {
        let receipt = make_example_receipt_minimal();
        assert_eq!(receipt.xchecker_version, "0.1.0");
        assert_eq!(receipt.claude_cli_version, "0.8.1");
    }
}

//! Test schema validation using generated JSON from constructors
//!
//! This test validates that JSON generated from actual constructors (not static files)
//! conforms to the JSON schemas. It also tests array sorting and stable key ordering.

use chrono::Utc;
use serde_json;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use tempfile::TempDir;

use xchecker::doctor::{CheckStatus, DoctorCheck, DoctorOutput};
use xchecker::receipt::ReceiptManager;
use xchecker::types::{
    ArtifactInfo, ConfigSource, ConfigValue, DriftPair, FileHash, LockDrift, PacketEvidence,
    PhaseId, StatusOutput,
};

/// Test that generated receipts validate against schema
#[test]
fn test_generated_receipt_validates_against_schema() {
    use camino::Utf8PathBuf;

    let temp_dir = TempDir::new().unwrap();
    let base_path = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();
    let manager = ReceiptManager::new(&base_path);

    // Generate a receipt using the actual constructor
    let outputs = vec![
        FileHash {
            path: "artifacts/00-requirements.md".to_string(),
            blake3_canonicalized:
                "abc123def456789012345678901234567890123456789012345678901234abcd".to_string(),
        },
        FileHash {
            path: "artifacts/10-design.md".to_string(),
            blake3_canonicalized:
                "def456abc789012345678901234567890123456789012345678901234567890a".to_string(),
        },
    ];

    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let receipt = manager.create_receipt(
        "test-spec",
        PhaseId::Requirements,
        0,
        outputs,
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        HashMap::new(),
        packet,
        None,
        None,
        vec![],
        None,
        "native",
        None,
        None,
        None,
        None,
        None, // pipeline
    );

    // Serialize to JSON
    let json_value = serde_json::to_value(&receipt).unwrap();

    // Load schema and validate
    let schema_content =
        fs::read_to_string("schemas/receipt.v1.json").expect("Failed to read receipt schema");
    let schema: serde_json::Value =
        serde_json::from_str(&schema_content).expect("Failed to parse receipt schema");

    let validator = jsonschema::validator_for(&schema).expect("Failed to compile receipt schema");

    if let Err(error) = validator.validate(&json_value) {
        panic!(
            "Generated receipt failed validation:\n{}",
            error.to_string()
        );
    }

    println!("✓ Generated receipt validates against schema");
}

/// Test that generated status outputs validate against schema
#[test]
fn test_generated_status_validates_against_schema() {
    // Generate a status output using the actual constructor
    let artifacts = vec![
        ArtifactInfo {
            path: "artifacts/00-requirements.md".to_string(),
            blake3_first8: "abc12345".to_string(),
        },
        ArtifactInfo {
            path: "artifacts/10-design.md".to_string(),
            blake3_first8: "def67890".to_string(),
        },
    ];

    let mut effective_config = BTreeMap::new();
    effective_config.insert(
        "model".to_string(),
        ConfigValue {
            value: serde_json::json!("claude-sonnet-4"),
            source: ConfigSource::Cli,
        },
    );
    effective_config.insert(
        "max_turns".to_string(),
        ConfigValue {
            value: serde_json::json!(6),
            source: ConfigSource::Config,
        },
    );

    let lock_drift = Some(LockDrift {
        model_full_name: Some(DriftPair {
            locked: "claude-sonnet-4-20250101".to_string(),
            current: "claude-sonnet-4-20250201".to_string(),
        }),
        claude_cli_version: None,
        schema_version: None,
    });

    let status = StatusOutput {
        schema_version: "1".to_string(),
        emitted_at: Utc::now(),
        runner: "native".to_string(),
        runner_distro: None,
        fallback_used: false,
        canonicalization_version: "yaml-v1,md-v1".to_string(),
        canonicalization_backend: "jcs-rfc8785".to_string(),
        artifacts,
        last_receipt_path: "receipts/00-requirements.json".to_string(),
        effective_config,
        lock_drift,
        pending_fixups: None,
    };

    // Serialize to JSON
    let json_value = serde_json::to_value(&status).unwrap();

    // Load schema and validate
    let schema_content =
        fs::read_to_string("schemas/status.v1.json").expect("Failed to read status schema");
    let schema: serde_json::Value =
        serde_json::from_str(&schema_content).expect("Failed to parse status schema");

    let validator = jsonschema::validator_for(&schema).expect("Failed to compile status schema");

    if let Err(error) = validator.validate(&json_value) {
        panic!("Generated status failed validation:\n{}", error.to_string());
    }

    println!("✓ Generated status validates against schema");
}

/// Test that generated doctor outputs validate against schema
#[test]
fn test_generated_doctor_validates_against_schema() {
    // Generate a doctor output using the actual constructor
    let checks = vec![
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
            name: "wsl_availability".to_string(),
            status: CheckStatus::Warn,
            details: "WSL not installed (Windows only)".to_string(),
        },
    ];

    let doctor = DoctorOutput {
        schema_version: "1".to_string(),
        emitted_at: Utc::now(),
        ok: true,
        checks,
        cache_stats: None,
    };

    // Serialize to JSON
    let json_value = serde_json::to_value(&doctor).unwrap();

    // Load schema and validate
    let schema_content =
        fs::read_to_string("schemas/doctor.v1.json").expect("Failed to read doctor schema");
    let schema: serde_json::Value =
        serde_json::from_str(&schema_content).expect("Failed to parse doctor schema");

    let validator = jsonschema::validator_for(&schema).expect("Failed to compile doctor schema");

    if let Err(error) = validator.validate(&json_value) {
        panic!(
            "Generated doctor output failed validation:\n{}",
            error.to_string()
        );
    }

    println!("✓ Generated doctor output validates against schema");
}

/// Test that arrays are sorted in generated outputs
#[test]
fn test_generated_outputs_have_sorted_arrays() {
    use camino::Utf8PathBuf;

    let temp_dir = TempDir::new().unwrap();
    let base_path = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();
    let manager = ReceiptManager::new(&base_path);

    // Create outputs in unsorted order
    let outputs = vec![
        FileHash {
            path: "artifacts/20-tasks.md".to_string(),
            blake3_canonicalized:
                "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string(),
        },
        FileHash {
            path: "artifacts/00-requirements.md".to_string(),
            blake3_canonicalized:
                "abc123def456789012345678901234567890123456789012345678901234abcd".to_string(),
        },
        FileHash {
            path: "artifacts/10-design.md".to_string(),
            blake3_canonicalized:
                "def456abc789012345678901234567890123456789012345678901234567890a".to_string(),
        },
    ];

    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let receipt = manager.create_receipt(
        "test-spec",
        PhaseId::Requirements,
        0,
        outputs,
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        HashMap::new(),
        packet,
        None,
        None,
        vec![],
        None,
        "native",
        None,
        None,
        None,
        None,
        None, // pipeline
    );

    // Serialize to JSON
    let json_value = serde_json::to_value(&receipt).unwrap();

    // Verify outputs are sorted by path
    let outputs_array = json_value["outputs"].as_array().unwrap();
    assert_eq!(
        outputs_array[0]["path"], "artifacts/00-requirements.md",
        "First output should be 00-requirements.md"
    );
    assert_eq!(
        outputs_array[1]["path"], "artifacts/10-design.md",
        "Second output should be 10-design.md"
    );
    assert_eq!(
        outputs_array[2]["path"], "artifacts/20-tasks.md",
        "Third output should be 20-tasks.md"
    );

    println!("✓ Receipt outputs are sorted by path");

    // Test status artifacts sorting
    let mut artifacts = vec![
        ArtifactInfo {
            path: "artifacts/20-tasks.md".to_string(),
            blake3_first8: "12345678".to_string(),
        },
        ArtifactInfo {
            path: "artifacts/00-requirements.md".to_string(),
            blake3_first8: "abc12345".to_string(),
        },
        ArtifactInfo {
            path: "artifacts/10-design.md".to_string(),
            blake3_first8: "def67890".to_string(),
        },
    ];

    // Sort artifacts by path before emission (as required by design)
    artifacts.sort_by(|a, b| a.path.cmp(&b.path));

    let status = StatusOutput {
        schema_version: "1".to_string(),
        emitted_at: Utc::now(),
        runner: "native".to_string(),
        runner_distro: None,
        fallback_used: false,
        canonicalization_version: "yaml-v1,md-v1".to_string(),
        canonicalization_backend: "jcs-rfc8785".to_string(),
        artifacts,
        last_receipt_path: "receipts/00-requirements.json".to_string(),
        effective_config: BTreeMap::new(),
        lock_drift: None,
        pending_fixups: None,
    };

    let json_value = serde_json::to_value(&status).unwrap();
    let artifacts_array = json_value["artifacts"].as_array().unwrap();
    assert_eq!(
        artifacts_array[0]["path"], "artifacts/00-requirements.md",
        "First artifact should be 00-requirements.md"
    );
    assert_eq!(
        artifacts_array[1]["path"], "artifacts/10-design.md",
        "Second artifact should be 10-design.md"
    );
    assert_eq!(
        artifacts_array[2]["path"], "artifacts/20-tasks.md",
        "Third artifact should be 20-tasks.md"
    );

    println!("✓ Status artifacts are sorted by path");

    // Test doctor checks sorting
    let mut checks = vec![
        DoctorCheck {
            name: "wsl_availability".to_string(),
            status: CheckStatus::Warn,
            details: "WSL not installed".to_string(),
        },
        DoctorCheck {
            name: "claude_path".to_string(),
            status: CheckStatus::Pass,
            details: "Found claude".to_string(),
        },
        DoctorCheck {
            name: "claude_version".to_string(),
            status: CheckStatus::Pass,
            details: "0.8.1".to_string(),
        },
    ];

    // Sort checks by name before emission (as required by design)
    checks.sort_by(|a, b| a.name.cmp(&b.name));

    let doctor = DoctorOutput {
        schema_version: "1".to_string(),
        emitted_at: Utc::now(),
        ok: true,
        checks,
        cache_stats: None,
    };

    let json_value = serde_json::to_value(&doctor).unwrap();
    let checks_array = json_value["checks"].as_array().unwrap();
    assert_eq!(
        checks_array[0]["name"], "claude_path",
        "First check should be claude_path"
    );
    assert_eq!(
        checks_array[1]["name"], "claude_version",
        "Second check should be claude_version"
    );
    assert_eq!(
        checks_array[2]["name"], "wsl_availability",
        "Third check should be wsl_availability"
    );

    println!("✓ Doctor checks are sorted by name");
}

/// Test that different insertion orders produce byte-identical JSON
#[test]
fn test_different_insertion_orders_produce_identical_json() {
    // Test with BTreeMap for effective_config (should maintain sorted order)
    let mut config1 = BTreeMap::new();
    config1.insert(
        "model".to_string(),
        ConfigValue {
            value: serde_json::json!("claude-sonnet-4"),
            source: ConfigSource::Cli,
        },
    );
    config1.insert(
        "max_turns".to_string(),
        ConfigValue {
            value: serde_json::json!(6),
            source: ConfigSource::Config,
        },
    );
    config1.insert(
        "packet_max_bytes".to_string(),
        ConfigValue {
            value: serde_json::json!(65536),
            source: ConfigSource::Default,
        },
    );

    let mut config2 = BTreeMap::new();
    // Insert in different order
    config2.insert(
        "packet_max_bytes".to_string(),
        ConfigValue {
            value: serde_json::json!(65536),
            source: ConfigSource::Default,
        },
    );
    config2.insert(
        "max_turns".to_string(),
        ConfigValue {
            value: serde_json::json!(6),
            source: ConfigSource::Config,
        },
    );
    config2.insert(
        "model".to_string(),
        ConfigValue {
            value: serde_json::json!("claude-sonnet-4"),
            source: ConfigSource::Cli,
        },
    );

    let status1 = StatusOutput {
        schema_version: "1".to_string(),
        emitted_at: Utc::now(),
        runner: "native".to_string(),
        runner_distro: None,
        fallback_used: false,
        canonicalization_version: "yaml-v1,md-v1".to_string(),
        canonicalization_backend: "jcs-rfc8785".to_string(),
        artifacts: vec![],
        last_receipt_path: "receipts/00-requirements.json".to_string(),
        effective_config: config1,
        lock_drift: None,
        pending_fixups: None,
    };

    let status2 = StatusOutput {
        schema_version: "1".to_string(),
        emitted_at: status1.emitted_at, // Use same timestamp
        runner: "native".to_string(),
        runner_distro: None,
        fallback_used: false,
        canonicalization_version: "yaml-v1,md-v1".to_string(),
        canonicalization_backend: "jcs-rfc8785".to_string(),
        artifacts: vec![],
        last_receipt_path: "receipts/00-requirements.json".to_string(),
        effective_config: config2,
        lock_drift: None,
        pending_fixups: None,
    };

    // Serialize both to JSON strings
    let json1 = serde_json::to_string(&status1).unwrap();
    let json2 = serde_json::to_string(&status2).unwrap();

    // They should be byte-identical
    assert_eq!(
        json1, json2,
        "Different insertion orders should produce identical JSON"
    );

    println!("✓ Different insertion orders produce byte-identical JSON");
}

/// Combined test that runs all schema validation checks
#[test]
fn test_generated_outputs_validate_against_schemas() {
    test_generated_receipt_validates_against_schema();
    test_generated_status_validates_against_schema();
    test_generated_doctor_validates_against_schema();
    test_generated_outputs_have_sorted_arrays();
    test_different_insertion_orders_produce_identical_json();

    println!("\n✅ All schema validation tests passed!");
}

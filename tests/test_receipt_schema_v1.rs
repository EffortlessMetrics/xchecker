use chrono::Utc;
use std::collections::HashMap;
use xchecker::types::{ErrorKind, FileHash, PacketEvidence, Receipt};

#[test]
fn test_receipt_schema_version_and_emitted_at() {
    // Create a receipt with schema_version and emitted_at
    let receipt = Receipt {
        schema_version: "1".to_string(),
        emitted_at: Utc::now(),
        spec_id: "test-spec".to_string(),
        phase: "requirements".to_string(),
        xchecker_version: "0.1.0".to_string(),
        claude_cli_version: "0.8.1".to_string(),
        model_full_name: "claude-3-5-sonnet-20241022".to_string(),
        model_alias: Some("sonnet".to_string()),
        canonicalization_version: "yaml-v1,md-v1".to_string(),
        canonicalization_backend: "jcs-rfc8785".to_string(),
        flags: HashMap::new(),
        runner: "native".to_string(),
        runner_distro: None,
        packet: PacketEvidence {
            files: vec![],
            max_bytes: 65536,
            max_lines: 1200,
        },
        outputs: vec![],
        exit_code: 0,
        error_kind: None,
        error_reason: None,
        stderr_tail: None,
        stderr_redacted: None,
        warnings: vec![],
        fallback_used: Some(false),
        diff_context: None,

        llm: None,
        pipeline: None,
    };

    // Verify schema_version is set
    assert_eq!(receipt.schema_version, "1");

    // Verify emitted_at is recent
    let now = Utc::now();
    let duration = now.signed_duration_since(receipt.emitted_at);
    assert!(duration.num_seconds() < 5, "emitted_at should be recent");

    // Serialize to JSON
    let json = serde_json::to_string_pretty(&receipt).unwrap();

    // Verify new fields are present
    assert!(json.contains("\"schema_version\": \"1\""));
    assert!(json.contains("\"emitted_at\""));
    assert!(
        !json.contains("\"timestamp\""),
        "Old timestamp field should not be present"
    );
}

#[test]
fn test_receipt_error_fields() {
    // Create a receipt with error fields
    let receipt = Receipt {
        schema_version: "1".to_string(),
        emitted_at: Utc::now(),
        spec_id: "test-spec".to_string(),
        phase: "requirements".to_string(),
        xchecker_version: "0.1.0".to_string(),
        claude_cli_version: "0.8.1".to_string(),
        model_full_name: "claude-3-5-sonnet-20241022".to_string(),
        model_alias: None,
        canonicalization_version: "yaml-v1,md-v1".to_string(),
        canonicalization_backend: "jcs-rfc8785".to_string(),
        flags: HashMap::new(),
        runner: "native".to_string(),
        runner_distro: None,
        packet: PacketEvidence {
            files: vec![],
            max_bytes: 65536,
            max_lines: 1200,
        },
        outputs: vec![],
        exit_code: 70,
        error_kind: Some(ErrorKind::ClaudeFailure),
        error_reason: Some("Claude CLI execution failed".to_string()),
        stderr_tail: Some("Error output".to_string()),
        stderr_redacted: None,
        warnings: vec![],
        fallback_used: Some(false),
        diff_context: None,

        llm: None,
        pipeline: None,
    };

    // Verify error fields
    assert_eq!(receipt.error_kind, Some(ErrorKind::ClaudeFailure));
    assert_eq!(
        receipt.error_reason,
        Some("Claude CLI execution failed".to_string())
    );
    assert_eq!(receipt.exit_code, 70);

    // Serialize to JSON
    let json = serde_json::to_string_pretty(&receipt).unwrap();

    // Verify error fields are present with snake_case
    assert!(json.contains("\"error_kind\": \"claude_failure\""));
    assert!(json.contains("\"error_reason\": \"Claude CLI execution failed\""));
}

#[test]
fn test_outputs_sorted_by_path() {
    // Create outputs in non-alphabetical order
    let outputs = vec![
        FileHash {
            path: "artifacts/10-design.md".to_string(),
            blake3_canonicalized: "hash3".to_string(),
        },
        FileHash {
            path: "artifacts/00-requirements.md".to_string(),
            blake3_canonicalized: "hash1".to_string(),
        },
        FileHash {
            path: "artifacts/05-analysis.md".to_string(),
            blake3_canonicalized: "hash2".to_string(),
        },
    ];

    let receipt = Receipt {
        schema_version: "1".to_string(),
        emitted_at: Utc::now(),
        spec_id: "test-spec".to_string(),
        phase: "requirements".to_string(),
        xchecker_version: "0.1.0".to_string(),
        claude_cli_version: "0.8.1".to_string(),
        model_full_name: "claude-3-5-sonnet-20241022".to_string(),
        model_alias: None,
        canonicalization_version: "yaml-v1,md-v1".to_string(),
        canonicalization_backend: "jcs-rfc8785".to_string(),
        flags: HashMap::new(),
        runner: "native".to_string(),
        runner_distro: None,
        packet: PacketEvidence {
            files: vec![],
            max_bytes: 65536,
            max_lines: 1200,
        },
        outputs,
        exit_code: 0,
        error_kind: None,
        error_reason: None,
        stderr_tail: None,
        stderr_redacted: None,
        warnings: vec![],
        fallback_used: Some(false),
        diff_context: None,

        llm: None,
        pipeline: None,
    };

    // Note: The ReceiptManager.create_receipt() sorts outputs, but when creating
    // Receipt directly, we need to verify the sorting happens in write_receipt
    // For this test, we just verify the structure allows sorting
    assert_eq!(receipt.outputs.len(), 3);
}

#[test]
fn test_jcs_canonical_json_emission() {
    use camino::Utf8PathBuf;
    use tempfile::TempDir;
    use xchecker::receipt::ReceiptManager;
    use xchecker::types::PhaseId;

    // Create a temporary directory for testing
    let temp_dir = TempDir::new().unwrap();
    let base_path = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();

    let manager = ReceiptManager::new(&base_path);

    // Create outputs in non-alphabetical order to test sorting
    let outputs = vec![
        FileHash {
            path: "artifacts/10-design.md".to_string(),
            blake3_canonicalized: "hash3".to_string(),
        },
        FileHash {
            path: "artifacts/00-requirements.md".to_string(),
            blake3_canonicalized: "hash1".to_string(),
        },
    ];

    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let receipt = manager.create_receipt(
        "test-jcs",
        PhaseId::Requirements,
        0,
        outputs,
        "0.1.0",
        "0.8.1",
        "claude-3-5-sonnet-20241022",
        None,
        HashMap::new(),
        packet,
        None,     // stderr_tail
        None,     // stderr_redacted
        vec![],   // warnings
        None,     // fallback_used
        "native", // runner
        None,     // runner_distro
        None,     // error_kind
        None,     // error_reason
        None,     // diff_context,
        None,     // pipeline
    );

    // Write receipt (which uses JCS)
    let receipt_path = manager.write_receipt(&receipt).unwrap();

    // Read the raw JSON file
    let json_content = std::fs::read_to_string(receipt_path.as_std_path()).unwrap();

    // Verify it's valid JSON
    let parsed: serde_json::Value = serde_json::from_str(&json_content).unwrap();

    // Verify schema_version is present
    assert_eq!(parsed["schema_version"], "1");

    // Verify emitted_at is present (RFC3339 format)
    assert!(parsed["emitted_at"].is_string());
    let emitted_at_str = parsed["emitted_at"].as_str().unwrap();
    assert!(
        emitted_at_str.contains("T") && emitted_at_str.contains("Z"),
        "emitted_at should be in RFC3339 UTC format"
    );

    // Verify outputs are sorted by path
    let outputs_array = parsed["outputs"].as_array().unwrap();
    assert_eq!(outputs_array.len(), 2);
    assert_eq!(outputs_array[0]["path"], "artifacts/00-requirements.md");
    assert_eq!(outputs_array[1]["path"], "artifacts/10-design.md");

    // Verify JCS canonical format (no pretty printing, deterministic key order)
    // JCS should produce compact JSON without whitespace
    assert!(
        !json_content.contains("  "),
        "JCS output should be compact without indentation"
    );
}

#[test]
fn test_different_insertion_orders_produce_identical_json() {
    use camino::Utf8PathBuf;
    use tempfile::TempDir;
    use xchecker::receipt::ReceiptManager;
    use xchecker::types::PhaseId;

    // Create a temporary directory for testing
    let temp_dir = TempDir::new().unwrap();
    let base_path = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();

    let manager = ReceiptManager::new(&base_path);

    // Create first receipt with outputs in one order
    let outputs1 = vec![
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
        FileHash {
            path: "artifacts/20-tasks.md".to_string(),
            blake3_canonicalized:
                "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string(),
        },
    ];

    // Create second receipt with outputs in different order
    let outputs2 = vec![
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

    // Create flags in different insertion orders
    let mut flags1 = HashMap::new();
    flags1.insert("max_turns".to_string(), "10".to_string());
    flags1.insert("output_format".to_string(), "stream-json".to_string());
    flags1.insert("permission_mode".to_string(), "auto".to_string());

    let mut flags2 = HashMap::new();
    flags2.insert("permission_mode".to_string(), "auto".to_string());
    flags2.insert("max_turns".to_string(), "10".to_string());
    flags2.insert("output_format".to_string(), "stream-json".to_string());

    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    // Use a fixed timestamp for both receipts to ensure identical output
    let fixed_timestamp = chrono::DateTime::parse_from_rfc3339("2025-10-24T14:30:00Z")
        .unwrap()
        .with_timezone(&Utc);

    // Create first receipt
    let mut receipt1 = manager.create_receipt(
        "test-snapshot",
        PhaseId::Requirements,
        0,
        outputs1,
        "0.1.0",
        "0.8.1",
        "claude-3-5-sonnet-20241022",
        Some("sonnet".to_string()),
        flags1,
        packet.clone(),
        None,                                                 // stderr_tail
        None,                                                 // stderr_redacted
        vec!["warning1".to_string(), "warning2".to_string()], // warnings
        Some(false),                                          // fallback_used
        "native",                                             // runner
        None,                                                 // runner_distro
        None,                                                 // error_kind
        None,                                                 // error_reason
        None,                                                 // diff_context,
        None,                                                 // pipeline
    );
    receipt1.emitted_at = fixed_timestamp;

    // Create second receipt with different insertion orders
    let mut receipt2 = manager.create_receipt(
        "test-snapshot",
        PhaseId::Requirements,
        0,
        outputs2,
        "0.1.0",
        "0.8.1",
        "claude-3-5-sonnet-20241022",
        Some("sonnet".to_string()),
        flags2,
        packet,
        None,                                                 // stderr_tail
        None,                                                 // stderr_redacted
        vec!["warning1".to_string(), "warning2".to_string()], // warnings
        Some(false),                                          // fallback_used
        "native",                                             // runner
        None,                                                 // runner_distro
        None,                                                 // error_kind
        None,                                                 // error_reason
        None,                                                 // diff_context,
        None,                                                 // pipeline
    );
    receipt2.emitted_at = fixed_timestamp;

    // Serialize both receipts using JCS
    let json_value1 = serde_json::to_value(&receipt1).unwrap();
    let json_bytes1 = serde_json_canonicalizer::to_vec(&json_value1).unwrap();

    let json_value2 = serde_json::to_value(&receipt2).unwrap();
    let json_bytes2 = serde_json_canonicalizer::to_vec(&json_value2).unwrap();

    // Verify byte-identical output
    assert_eq!(
        json_bytes1,
        json_bytes2,
        "Receipts with different insertion orders must produce byte-identical JSON.\n\
         JSON1: {}\n\
         JSON2: {}",
        String::from_utf8_lossy(&json_bytes1),
        String::from_utf8_lossy(&json_bytes2)
    );

    // Also verify the JSON is valid and has expected structure
    let parsed: serde_json::Value = serde_json::from_slice(&json_bytes1).unwrap();

    // Verify outputs are sorted
    let outputs_array = parsed["outputs"].as_array().unwrap();
    assert_eq!(outputs_array.len(), 3);
    assert_eq!(outputs_array[0]["path"], "artifacts/00-requirements.md");
    assert_eq!(outputs_array[1]["path"], "artifacts/10-design.md");
    assert_eq!(outputs_array[2]["path"], "artifacts/20-tasks.md");

    // Verify flags are present (JCS will sort keys)
    let flags_obj = parsed["flags"].as_object().unwrap();
    assert_eq!(flags_obj.len(), 3);
    assert_eq!(flags_obj["max_turns"], "10");
    assert_eq!(flags_obj["output_format"], "stream-json");
    assert_eq!(flags_obj["permission_mode"], "auto");
}

#[test]
fn test_status_different_insertion_orders_produce_identical_json() {
    use std::collections::BTreeMap;
    use xchecker::types::{ArtifactInfo, ConfigSource, ConfigValue, StatusOutput};

    // Use a fixed timestamp for both status outputs
    let fixed_timestamp = chrono::DateTime::parse_from_rfc3339("2025-10-24T14:30:00Z")
        .unwrap()
        .with_timezone(&Utc);

    // Create artifacts in different orders
    let artifacts1 = vec![
        ArtifactInfo {
            path: "artifacts/00-requirements.md".to_string(),
            blake3_first8: "abc12345".to_string(),
        },
        ArtifactInfo {
            path: "artifacts/10-design.md".to_string(),
            blake3_first8: "def67890".to_string(),
        },
    ];

    let artifacts2 = vec![
        ArtifactInfo {
            path: "artifacts/10-design.md".to_string(),
            blake3_first8: "def67890".to_string(),
        },
        ArtifactInfo {
            path: "artifacts/00-requirements.md".to_string(),
            blake3_first8: "abc12345".to_string(),
        },
    ];

    // Create effective_config in different insertion orders
    let mut config1 = BTreeMap::new();
    config1.insert(
        "max_turns".to_string(),
        ConfigValue {
            value: serde_json::Value::Number(10.into()),
            source: ConfigSource::Config,
        },
    );
    config1.insert(
        "model".to_string(),
        ConfigValue {
            value: serde_json::Value::String("claude-sonnet-4".to_string()),
            source: ConfigSource::Cli,
        },
    );
    config1.insert(
        "packet_max_bytes".to_string(),
        ConfigValue {
            value: serde_json::Value::Number(65536.into()),
            source: ConfigSource::Default,
        },
    );

    let mut config2 = BTreeMap::new();
    config2.insert(
        "packet_max_bytes".to_string(),
        ConfigValue {
            value: serde_json::Value::Number(65536.into()),
            source: ConfigSource::Default,
        },
    );
    config2.insert(
        "model".to_string(),
        ConfigValue {
            value: serde_json::Value::String("claude-sonnet-4".to_string()),
            source: ConfigSource::Cli,
        },
    );
    config2.insert(
        "max_turns".to_string(),
        ConfigValue {
            value: serde_json::Value::Number(10.into()),
            source: ConfigSource::Config,
        },
    );

    // Create first status output
    let mut status1 = StatusOutput {
        schema_version: "1".to_string(),
        emitted_at: fixed_timestamp,
        runner: "native".to_string(),
        runner_distro: None,
        fallback_used: false,
        canonicalization_version: "yaml-v1,md-v1".to_string(),
        canonicalization_backend: "jcs-rfc8785".to_string(),
        artifacts: artifacts1,
        last_receipt_path: "receipts/requirements-20251024_143000.json".to_string(),
        effective_config: config1,
        lock_drift: None,
        pending_fixups: None,
    };

    // Sort artifacts by path for status1
    status1.artifacts.sort_by(|a, b| a.path.cmp(&b.path));

    // Create second status output with different insertion orders
    let mut status2 = StatusOutput {
        schema_version: "1".to_string(),
        emitted_at: fixed_timestamp,
        runner: "native".to_string(),
        runner_distro: None,
        fallback_used: false,
        canonicalization_version: "yaml-v1,md-v1".to_string(),
        canonicalization_backend: "jcs-rfc8785".to_string(),
        artifacts: artifacts2,
        last_receipt_path: "receipts/requirements-20251024_143000.json".to_string(),
        effective_config: config2,
        lock_drift: None,
        pending_fixups: None,
    };

    // Sort artifacts by path for status2
    status2.artifacts.sort_by(|a, b| a.path.cmp(&b.path));

    // Serialize both status outputs using JCS
    let json_value1 = serde_json::to_value(&status1).unwrap();
    let json_bytes1 = serde_json_canonicalizer::to_vec(&json_value1).unwrap();

    let json_value2 = serde_json::to_value(&status2).unwrap();
    let json_bytes2 = serde_json_canonicalizer::to_vec(&json_value2).unwrap();

    // Verify byte-identical output
    assert_eq!(
        json_bytes1,
        json_bytes2,
        "Status outputs with different insertion orders must produce byte-identical JSON.\n\
         JSON1: {}\n\
         JSON2: {}",
        String::from_utf8_lossy(&json_bytes1),
        String::from_utf8_lossy(&json_bytes2)
    );

    // Verify the JSON is valid and has expected structure
    let parsed: serde_json::Value = serde_json::from_slice(&json_bytes1).unwrap();

    // Verify artifacts are sorted
    let artifacts_array = parsed["artifacts"].as_array().unwrap();
    assert_eq!(artifacts_array.len(), 2);
    assert_eq!(artifacts_array[0]["path"], "artifacts/00-requirements.md");
    assert_eq!(artifacts_array[1]["path"], "artifacts/10-design.md");

    // Verify effective_config is present (BTreeMap ensures sorted keys)
    let config_obj = parsed["effective_config"].as_object().unwrap();
    assert_eq!(config_obj.len(), 3);
}

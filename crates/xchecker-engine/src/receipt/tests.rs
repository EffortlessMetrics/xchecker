use super::errors::error_to_exit_code_and_kind;
use super::*;
use crate::error::XCheckerError;
use crate::types::PhaseId;
use crate::types::{ErrorKind, FileHash, FileType, PacketEvidence, Receipt};
use chrono::Utc;
use std::collections::HashMap;
use tempfile::TempDir;

fn create_test_manager() -> (ReceiptManager, TempDir) {
    let temp_dir = crate::paths::with_isolated_home();
    let base_path = crate::paths::xchecker_home()
        .join("specs")
        .join("test-spec");

    let manager = ReceiptManager::new(&base_path);

    (manager, temp_dir)
}

#[test]
fn test_create_receipt() {
    let (manager, _temp_dir) = create_test_manager();

    let outputs = vec![FileHash {
        path: "artifacts/00-requirements.md".to_string(),
        blake3_canonicalized: "abc123".to_string(),
    }];

    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let mut flags = std::collections::HashMap::new();
    flags.insert("max_turns".to_string(), "10".to_string());

    let receipt = manager.create_receipt(
        "test-spec",
        PhaseId::Requirements,
        0,
        outputs,
        "0.1.0",
        "0.8.1",
        "haiku",
        Some("sonnet".to_string()),
        flags,
        packet,
        None,
        None, // stderr_redacted
        vec![],
        Some(false),
        "native",
        None,
        None,
        None,
        None, // diff_context,
        None, // pipeline
    );

    assert_eq!(receipt.schema_version, "1");
    assert_eq!(receipt.spec_id, "test-spec");
    assert_eq!(receipt.phase, "requirements");
    assert_eq!(receipt.exit_code, 0);
    assert_eq!(receipt.canonicalization_version, "yaml-v1,md-v1");
    assert_eq!(receipt.canonicalization_backend, "jcs-rfc8785");
    assert_eq!(receipt.runner, "native");
    assert_eq!(receipt.runner_distro, None);
    assert_eq!(receipt.outputs.len(), 1);
    assert_eq!(receipt.outputs[0].path, "artifacts/00-requirements.md");
    assert_eq!(receipt.xchecker_version, "0.1.0");
    assert_eq!(receipt.claude_cli_version, "0.8.1");
    assert_eq!(receipt.model_full_name, "haiku");
    assert_eq!(receipt.model_alias, Some("sonnet".to_string()));
    assert_eq!(receipt.fallback_used, Some(false));
    assert_eq!(receipt.error_kind, None);
    assert_eq!(receipt.error_reason, None);
}

#[test]
fn test_create_file_hash() {
    let (manager, _temp_dir) = create_test_manager();

    let content = "# Requirements\n\nSome requirements content\n";
    let result = manager.create_file_hash(
        "artifacts/00-requirements.md",
        content,
        FileType::Markdown,
        "requirements",
    );

    assert!(result.is_ok());
    let file_hash = result.unwrap();
    assert_eq!(file_hash.path, "artifacts/00-requirements.md");
    assert!(!file_hash.blake3_canonicalized.is_empty());
    assert_eq!(file_hash.blake3_canonicalized.len(), 64); // BLAKE3 hex string length
}

#[test]
fn test_write_and_read_receipt() {
    let (manager, _temp_dir) = create_test_manager();

    let outputs = vec![FileHash {
        path: "artifacts/00-requirements.md".to_string(),
        blake3_canonicalized: "abc123".to_string(),
    }];

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
        std::collections::HashMap::new(),
        packet,
        None,
        None, // stderr_redacted
        vec![],
        None,
        "native",
        None,
        None,
        None,
        None, // diff_context,
        None, // pipeline
    );

    // Write receipt
    let receipt_path = manager.write_receipt(&receipt).unwrap();
    assert!(receipt_path.exists());
    assert!(receipt_path.to_string().contains("requirements-"));
    assert!(receipt_path.to_string().ends_with(".json"));

    // Read receipt back
    let read_receipt = manager.read_latest_receipt(PhaseId::Requirements).unwrap();
    assert!(read_receipt.is_some());

    let read_receipt = read_receipt.unwrap();
    assert_eq!(read_receipt.spec_id, receipt.spec_id);
    assert_eq!(read_receipt.phase, receipt.phase);
    assert_eq!(read_receipt.exit_code, receipt.exit_code);
    assert_eq!(read_receipt.outputs.len(), receipt.outputs.len());
}

#[test]
fn test_no_receipt_exists() {
    let (manager, _temp_dir) = create_test_manager();

    let result = manager.read_latest_receipt(PhaseId::Requirements).unwrap();
    assert!(result.is_none());
}

#[test]
fn test_list_receipts() {
    let (manager, _temp_dir) = create_test_manager();

    // Initially no receipts
    let receipts = manager.list_receipts().unwrap();
    assert_eq!(receipts.len(), 0);

    // Create and write a receipt
    let outputs = vec![FileHash {
        path: "artifacts/00-requirements.md".to_string(),
        blake3_canonicalized: "abc123".to_string(),
    }];

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
        std::collections::HashMap::new(),
        packet,
        None,
        None, // stderr_redacted
        vec![],
        None,
        "native",
        None,
        None,
        None,
        None, // diff_context,
        None, // pipeline
    );

    manager.write_receipt(&receipt).unwrap();

    // Now should have one receipt
    let receipts = manager.list_receipts().unwrap();
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].spec_id, "test-spec");
}

#[test]
fn test_receipt_json_serialization() {
    let (manager, _temp_dir) = create_test_manager();

    let outputs = vec![
        FileHash {
            path: "artifacts/00-requirements.md".to_string(),
            blake3_canonicalized: "abc123def456".to_string(),
        },
        FileHash {
            path: "artifacts/00-requirements.core.yaml".to_string(),
            blake3_canonicalized: "789xyz012".to_string(),
        },
    ];

    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let mut flags = std::collections::HashMap::new();
    flags.insert("output_format".to_string(), "stream-json".to_string());

    let receipt = manager.create_receipt(
        "test-spec-123",
        PhaseId::Design,
        0,
        outputs,
        "0.1.0",
        "0.8.1",
        "haiku",
        Some("sonnet".to_string()),
        flags,
        packet,
        Some("stderr output".to_string()),
        None, // stderr_redacted
        vec!["warning 1".to_string()],
        Some(false),
        "wsl",
        Some("Ubuntu-22.04".to_string()),
        None,
        None,
        None, // diff_context,
        None, // pipeline
    );

    // Serialize to JSON
    let json = serde_json::to_string_pretty(&receipt).unwrap();

    // Verify JSON contains expected fields
    assert!(json.contains("\"schema_version\": \"1\""));
    assert!(json.contains("\"emitted_at\""));
    assert!(json.contains("\"spec_id\": \"test-spec-123\""));
    assert!(json.contains("\"phase\": \"design\""));
    assert!(json.contains("\"exit_code\": 0"));
    assert!(json.contains("\"canonicalization_version\": \"yaml-v1,md-v1\""));
    assert!(json.contains("\"canonicalization_backend\": \"jcs-rfc8785\""));
    assert!(json.contains("\"runner\": \"wsl\""));
    assert!(json.contains("\"runner_distro\": \"Ubuntu-22.04\""));
    assert!(json.contains("\"outputs\""));
    assert!(json.contains("\"xchecker_version\": \"0.1.0\""));
    assert!(json.contains("\"claude_cli_version\": \"0.8.1\""));
    assert!(json.contains("\"model_full_name\": \"haiku\""));
    assert!(json.contains("\"fallback_used\": false"));
    assert!(json.contains("artifacts/00-requirements.core.yaml"));
    assert!(json.contains("789xyz012"));

    // Verify we can deserialize back
    let deserialized: Receipt = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.schema_version, "1");
    assert_eq!(deserialized.spec_id, receipt.spec_id);
    assert_eq!(deserialized.phase, receipt.phase);
    assert_eq!(deserialized.exit_code, receipt.exit_code);
    assert_eq!(deserialized.canonicalization_backend, "jcs-rfc8785");
    assert_eq!(deserialized.runner, "wsl");
    assert_eq!(deserialized.runner_distro, Some("Ubuntu-22.04".to_string()));
    assert_eq!(deserialized.outputs.len(), 2);
    assert_eq!(deserialized.xchecker_version, "0.1.0");
    assert_eq!(deserialized.claude_cli_version, "0.8.1");
    // Verify outputs are sorted by path
    assert_eq!(
        deserialized.outputs[0].path,
        "artifacts/00-requirements.core.yaml"
    );
    assert_eq!(deserialized.outputs[1].path, "artifacts/00-requirements.md");
}

#[test]
fn test_new_fields_serialization() {
    let (manager, _temp_dir) = create_test_manager();

    let outputs = vec![FileHash {
        path: "artifacts/00-requirements.md".to_string(),
        blake3_canonicalized: "abc123".to_string(),
    }];

    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let mut flags = std::collections::HashMap::new();
    flags.insert("output_format".to_string(), "stream-json".to_string());

    let receipt = manager.create_receipt(
        "test-new-fields",
        PhaseId::Requirements,
        0,
        outputs,
        "0.1.0",
        "0.8.1",
        "haiku",
        Some("sonnet".to_string()),
        flags,
        packet,
        None,
        None, // stderr_redacted
        vec![],
        Some(false),
        "wsl",
        Some("Ubuntu-22.04".to_string()),
        None,
        None,
        None, // diff_context,
        None, // pipeline
    );

    // Verify new fields are set correctly
    assert_eq!(receipt.schema_version, "1");
    assert_eq!(receipt.canonicalization_backend, "jcs-rfc8785");
    assert_eq!(receipt.runner, "wsl");
    assert_eq!(receipt.runner_distro, Some("Ubuntu-22.04".to_string()));
    assert_eq!(receipt.error_kind, None);
    assert_eq!(receipt.error_reason, None);

    // Serialize to JSON and verify fields are present
    let json = serde_json::to_string_pretty(&receipt).unwrap();
    assert!(json.contains("\"schema_version\": \"1\""));
    assert!(json.contains("\"emitted_at\""));
    assert!(json.contains("\"canonicalization_backend\": \"jcs-rfc8785\""));
    assert!(json.contains("\"runner\": \"wsl\""));
    assert!(json.contains("\"runner_distro\": \"Ubuntu-22.04\""));

    // Deserialize and verify
    let deserialized: Receipt = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.schema_version, "1");
    assert_eq!(deserialized.canonicalization_backend, "jcs-rfc8785");
    assert_eq!(deserialized.runner, "wsl");
    assert_eq!(deserialized.runner_distro, Some("Ubuntu-22.04".to_string()));
}

#[test]
fn test_add_rename_retry_warning() {
    // Test with retry count
    let mut warnings = vec![];
    add_rename_retry_warning(&mut warnings, Some(3));
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0], "rename_retry_count: 3");

    // Test with no retry count
    let mut warnings2 = vec![];
    add_rename_retry_warning(&mut warnings2, None);
    assert_eq!(warnings2.len(), 0);

    // Test appending to existing warnings
    let mut warnings3 = vec!["existing_warning".to_string()];
    add_rename_retry_warning(&mut warnings3, Some(5));
    assert_eq!(warnings3.len(), 2);
    assert_eq!(warnings3[0], "existing_warning");
    assert_eq!(warnings3[1], "rename_retry_count: 5");
}

#[test]
fn test_diff_context_in_receipt() {
    let (manager, _temp_dir) = create_test_manager();

    let outputs = vec![];
    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    // Test with diff_context set to 0 (unidiff-zero enabled)
    let receipt_with_zero = manager.create_receipt(
        "test-spec",
        PhaseId::Review,
        0,
        outputs.clone(),
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        std::collections::HashMap::new(),
        packet.clone(),
        None,
        None, // stderr_redacted
        vec![],
        None,
        "native",
        None,
        None,
        None,
        Some(0), // diff_context
        None,    // pipeline
    );

    assert_eq!(receipt_with_zero.diff_context, Some(0));

    // Test with diff_context set to None (default)
    let receipt_default = manager.create_receipt(
        "test-spec",
        PhaseId::Review,
        0,
        outputs,
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        std::collections::HashMap::new(),
        packet,
        None,
        None, // stderr_redacted
        vec![],
        None,
        "native",
        None,
        None,
        None,
        None, // diff_context,
        None, // pipeline
    );

    assert_eq!(receipt_default.diff_context, None);

    // Verify serialization includes diff_context
    let json = serde_json::to_string(&receipt_with_zero).unwrap();
    assert!(json.contains("\"diff_context\":0"));
}

#[test]
fn test_integration_with_canonicalization() {
    let (manager, _temp_dir) = create_test_manager();

    // Test with YAML content
    let yaml_content = r"
name: test-spec
version: 1.0
requirements:
  - R1: Basic functionality
  - R2: Error handling
";

    let yaml_hash = manager
        .create_file_hash(
            "artifacts/00-requirements.core.yaml",
            yaml_content,
            FileType::Yaml,
            "requirements",
        )
        .unwrap();

    // Test with Markdown content
    let md_content = "# Requirements\n\nThis is a test requirements document.\n\n## R1: Basic functionality\n\nThe system shall provide basic functionality.\n";

    let md_hash = manager
        .create_file_hash(
            "artifacts/00-requirements.md",
            md_content,
            FileType::Markdown,
            "requirements",
        )
        .unwrap();

    // Create receipt with both hashes
    let outputs = vec![yaml_hash, md_hash];
    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let receipt = manager.create_receipt(
        "integration-test",
        PhaseId::Requirements,
        0,
        outputs,
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        std::collections::HashMap::new(),
        packet,
        None,
        None, // stderr_redacted
        vec![],
        None,
        "native",
        None,
        None,
        None,
        None, // diff_context,
        None, // pipeline
    );

    // Write and verify receipt
    let receipt_path = manager.write_receipt(&receipt).unwrap();
    assert!(receipt_path.exists());

    // Read back and verify
    let read_receipt = manager.read_latest_receipt(PhaseId::Requirements).unwrap();
    assert!(read_receipt.is_some());

    let read_receipt = read_receipt.unwrap();
    assert_eq!(read_receipt.spec_id, "integration-test");
    assert_eq!(read_receipt.outputs.len(), 2);

    // Verify hashes are proper BLAKE3 hex strings
    for hash in &read_receipt.outputs {
        assert_eq!(hash.blake3_canonicalized.len(), 64);
        assert!(
            hash.blake3_canonicalized
                .chars()
                .all(|c| c.is_ascii_hexdigit())
        );
    }
}

#[test]
fn test_error_receipt_creation() {
    let (manager, _temp_dir) = create_test_manager();

    // Create an error using XCheckerError
    let error = XCheckerError::PacketOverflow {
        used_bytes: 100000,
        limit_bytes: 65536,
        used_lines: 1500,
        limit_lines: 1200,
    };

    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let error_receipt = manager.create_error_receipt(
        "test-error-spec",
        PhaseId::Requirements,
        &error,
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        std::collections::HashMap::new(),
        packet,
        Some("stderr output".to_string()),
        None, // stderr_redacted
        vec!["warning1".to_string()],
        None,
        "native",
        None,
        None, // diff_context,
        None, // pipeline
    );

    // Verify error fields are set correctly
    assert_eq!(error_receipt.exit_code, 7); // PacketOverflow exit code
    assert_eq!(error_receipt.error_kind, Some(ErrorKind::PacketOverflow));
    assert!(error_receipt.error_reason.is_some());
    assert!(
        error_receipt
            .error_reason
            .as_ref()
            .unwrap()
            .contains("100000")
    );
    assert_eq!(error_receipt.outputs.len(), 0); // No outputs for error receipts
    assert_eq!(error_receipt.stderr_tail, Some("stderr output".to_string()));
    assert_eq!(error_receipt.warnings.len(), 1);
}

#[test]
fn test_error_receipt_with_different_error_kinds() {
    let (manager, _temp_dir) = create_test_manager();

    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    // Test SecretDetected error
    let secret_error = XCheckerError::SecretDetected {
        pattern: "github_pat".to_string(),
        location: "test.txt:42".to_string(),
    };

    let secret_receipt = manager.create_error_receipt(
        "test-secret",
        PhaseId::Requirements,
        &secret_error,
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        std::collections::HashMap::new(),
        packet.clone(),
        None,
        None, // stderr_redacted
        vec![],
        None,
        "native",
        None,
        None, // diff_context,
        None, // pipeline
    );

    assert_eq!(secret_receipt.exit_code, 8);
    assert_eq!(secret_receipt.error_kind, Some(ErrorKind::SecretDetected));

    // Test ConcurrentExecution error
    let lock_error = XCheckerError::ConcurrentExecution {
        id: "test-spec".to_string(),
    };

    let lock_receipt = manager.create_error_receipt(
        "test-lock",
        PhaseId::Design,
        &lock_error,
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        std::collections::HashMap::new(),
        packet,
        None,
        None,
        vec![],
        None,
        "native",
        None,
        None, // diff_context
        None, // pipeline
    );

    assert_eq!(lock_receipt.exit_code, 9);
    assert_eq!(lock_receipt.error_kind, Some(ErrorKind::LockHeld));
}

#[test]
fn test_optional_fields_in_receipt() {
    let (manager, _temp_dir) = create_test_manager();

    let outputs = vec![FileHash {
        path: "artifacts/00-requirements.md".to_string(),
        blake3_canonicalized: "abc123".to_string(),
    }];

    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    // Test with all optional fields set
    let receipt_with_optionals = manager.create_receipt(
        "test-optionals",
        PhaseId::Requirements,
        0,
        outputs.clone(),
        "0.1.0",
        "0.8.1",
        "haiku",
        Some("sonnet".to_string()),
        std::collections::HashMap::new(),
        packet.clone(),
        Some("stderr content".to_string()),
        None,
        vec!["warning1".to_string(), "warning2".to_string()],
        Some(true),
        "wsl",
        Some("Ubuntu-22.04".to_string()),
        None,
        None,
        Some(3),
        None, // pipeline
    );

    assert_eq!(
        receipt_with_optionals.stderr_tail,
        Some("stderr content".to_string())
    );
    assert_eq!(
        receipt_with_optionals.runner_distro,
        Some("Ubuntu-22.04".to_string())
    );
    assert_eq!(receipt_with_optionals.warnings.len(), 2);
    assert_eq!(receipt_with_optionals.fallback_used, Some(true));
    assert_eq!(receipt_with_optionals.diff_context, Some(3));

    // Test with optional fields as None
    let receipt_without_optionals = manager.create_receipt(
        "test-no-optionals",
        PhaseId::Design,
        0,
        outputs,
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        std::collections::HashMap::new(),
        packet,
        None,
        None,
        vec![],
        None,
        "native",
        None,
        None,
        None,
        None, // diff_context
        None, // pipeline
    );

    assert_eq!(receipt_without_optionals.stderr_tail, None);
    assert_eq!(receipt_without_optionals.runner_distro, None);
    assert_eq!(receipt_without_optionals.warnings.len(), 0);
    assert_eq!(receipt_without_optionals.fallback_used, None);
    assert_eq!(receipt_without_optionals.diff_context, None);
    assert_eq!(receipt_without_optionals.model_alias, None);
}

#[test]
fn test_atomic_write_creates_file() {
    let (manager, _temp_dir) = create_test_manager();

    let outputs = vec![FileHash {
        path: "artifacts/00-requirements.md".to_string(),
        blake3_canonicalized: "abc123".to_string(),
    }];

    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let receipt = manager.create_receipt(
        "test-atomic",
        PhaseId::Requirements,
        0,
        outputs,
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        std::collections::HashMap::new(),
        packet,
        None,
        None,
        vec![],
        None,
        "native",
        None,
        None,
        None,
        None, // diff_context
        None, // pipeline
    );

    // Write receipt using atomic operation
    let receipt_path = manager.write_receipt(&receipt).unwrap();

    // Verify file exists
    assert!(receipt_path.exists());

    // Verify file is readable and contains valid JSON
    let content = std::fs::read_to_string(receipt_path.as_std_path()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

    // Verify it's the correct receipt
    assert_eq!(parsed["spec_id"], "test-atomic");
    assert_eq!(parsed["phase"], "requirements");
}

#[test]
fn test_jcs_emission_byte_identical() {
    let (manager, _temp_dir) = create_test_manager();

    let outputs = vec![
        FileHash {
            path: "artifacts/00-requirements.md".to_string(),
            blake3_canonicalized: "abc123".to_string(),
        },
        FileHash {
            path: "artifacts/10-design.md".to_string(),
            blake3_canonicalized: "def456".to_string(),
        },
    ];

    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let mut flags = std::collections::HashMap::new();
    flags.insert("key1".to_string(), "value1".to_string());
    flags.insert("key2".to_string(), "value2".to_string());

    // Use a fixed timestamp
    let fixed_timestamp = chrono::DateTime::parse_from_rfc3339("2025-10-24T14:30:00Z")
        .unwrap()
        .with_timezone(&Utc);

    let mut receipt = manager.create_receipt(
        "test-jcs",
        PhaseId::Requirements,
        0,
        outputs,
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        flags,
        packet,
        None,
        None,
        vec![],
        None,
        "native",
        None,
        None,
        None,
        None, // diff_context
        None, // pipeline
    );
    receipt.emitted_at = fixed_timestamp;

    // Serialize twice using JCS
    let json_value1 = serde_json::to_value(&receipt).unwrap();
    let json_bytes1 = serde_json_canonicalizer::to_vec(&json_value1).unwrap();

    let json_value2 = serde_json::to_value(&receipt).unwrap();
    let json_bytes2 = serde_json_canonicalizer::to_vec(&json_value2).unwrap();

    // Verify byte-identical output
    assert_eq!(json_bytes1, json_bytes2);

    // Verify it's compact (no pretty printing)
    let json_str = String::from_utf8(json_bytes1).unwrap();
    assert!(!json_str.contains("  ")); // No indentation
    assert!(!json_str.contains('\n')); // No newlines (except possibly at end)
}

#[test]
fn test_receipt_listing_chronological_order() {
    let (manager, _temp_dir) = create_test_manager();

    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    // Create multiple receipts for different phases with different timestamps
    let phases = [PhaseId::Requirements, PhaseId::Design, PhaseId::Tasks];

    for (i, phase) in phases.iter().enumerate() {
        let outputs = vec![FileHash {
            path: format!("artifacts/0{i}-test.md"),
            blake3_canonicalized: format!("hash{i}"),
        }];

        let receipt = manager.create_receipt(
            "test-chronological",
            *phase,
            0,
            outputs,
            "0.1.0",
            "0.8.1",
            "haiku",
            None,
            std::collections::HashMap::new(),
            packet.clone(),
            None,
            None,
            vec![],
            None,
            "native",
            None,
            None,
            None,
            None, // diff_context
            None, // pipeline
        );

        manager.write_receipt(&receipt).unwrap();

        // Small delay to ensure different timestamps
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    // List all receipts
    let receipts = manager.list_receipts().unwrap();

    // Verify we have 3 receipts
    assert_eq!(receipts.len(), 3);

    // Verify they are in chronological order
    for i in 0..receipts.len() - 1 {
        assert!(receipts[i].emitted_at <= receipts[i + 1].emitted_at);
    }
}

#[test]
fn test_read_latest_receipt_returns_most_recent() {
    let (manager, _temp_dir) = create_test_manager();

    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    // Create multiple receipts for the same phase
    for i in 0..3 {
        let outputs = vec![FileHash {
            path: format!("artifacts/version-{i}.md"),
            blake3_canonicalized: format!("hash{i}"),
        }];

        let receipt = manager.create_receipt(
            "test-latest",
            PhaseId::Requirements,
            0,
            outputs,
            "0.1.0",
            "0.8.1",
            "haiku",
            None,
            std::collections::HashMap::new(),
            packet.clone(),
            None,
            None,
            vec![],
            None,
            "native",
            None,
            None,
            None,
            None, // diff_context
            None, // pipeline
        );

        manager.write_receipt(&receipt).unwrap();

        // Small delay to ensure different timestamps
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    // Read the latest receipt
    let latest = manager.read_latest_receipt(PhaseId::Requirements).unwrap();
    assert!(latest.is_some());

    let latest_receipt = latest.unwrap();

    // Verify it's the most recent one (version-2)
    assert_eq!(latest_receipt.outputs[0].path, "artifacts/version-2.md");
}

#[test]
fn test_receipt_edge_case_empty_outputs() {
    let (manager, _temp_dir) = create_test_manager();

    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    // Create receipt with no outputs (error case)
    let receipt = manager.create_receipt(
        "test-empty",
        PhaseId::Requirements,
        70,
        vec![], // Empty outputs
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        std::collections::HashMap::new(),
        packet,
        Some("Claude failed".to_string()),
        None,
        vec![],
        None,
        "native",
        None,
        Some(ErrorKind::ClaudeFailure),
        Some("Claude CLI execution failed".to_string()),
        None, // diff_context
        None, // pipeline
    );

    // Write and read back
    let receipt_path = manager.write_receipt(&receipt).unwrap();
    assert!(receipt_path.exists());

    let read_receipt = manager.read_latest_receipt(PhaseId::Requirements).unwrap();
    assert!(read_receipt.is_some());

    let read_receipt = read_receipt.unwrap();
    assert_eq!(read_receipt.outputs.len(), 0);
    assert_eq!(read_receipt.exit_code, 70);
    assert_eq!(read_receipt.error_kind, Some(ErrorKind::ClaudeFailure));
}

#[test]
fn test_receipt_edge_case_large_warnings_list() {
    let (manager, _temp_dir) = create_test_manager();

    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    // Create a large warnings list
    let warnings: Vec<String> = (0..100).map(|i| format!("warning_{i}")).collect();

    let receipt = manager.create_receipt(
        "test-warnings",
        PhaseId::Requirements,
        0,
        vec![],
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        std::collections::HashMap::new(),
        packet,
        None,
        None,
        warnings,
        None,
        "native",
        None,
        None,
        None,
        None, // diff_context
        None, // pipeline
    );

    // Write and read back
    manager.write_receipt(&receipt).unwrap();

    let read_receipt = manager.read_latest_receipt(PhaseId::Requirements).unwrap();
    assert!(read_receipt.is_some());

    let read_receipt = read_receipt.unwrap();
    assert_eq!(read_receipt.warnings.len(), 100);
    assert_eq!(read_receipt.warnings[0], "warning_0");
    assert_eq!(read_receipt.warnings[99], "warning_99");
}

#[test]
fn test_receipt_edge_case_special_characters_in_fields() {
    let (manager, _temp_dir) = create_test_manager();

    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    // Create receipt with special characters
    let outputs = vec![FileHash {
        path: "artifacts/test-with-unicode-🚀.md".to_string(),
        blake3_canonicalized: "abc123".to_string(),
    }];

    let mut flags = std::collections::HashMap::new();
    flags.insert(
        "key_with_\"quotes\"".to_string(),
        "value_with_\n_newline".to_string(),
    );

    let receipt = manager.create_receipt(
        "test-special-chars-🎉",
        PhaseId::Requirements,
        0,
        outputs,
        "0.1.0",
        "0.8.1",
        "haiku",
        None,
        flags,
        packet,
        Some("stderr with\nnewlines\tand\ttabs".to_string()),
        None,
        vec!["warning with 'quotes'".to_string()],
        None,
        "native",
        None,
        None,
        None,
        None, // diff_context
        None, // pipeline
    );

    // Write and read back
    manager.write_receipt(&receipt).unwrap();

    let read_receipt = manager.read_latest_receipt(PhaseId::Requirements).unwrap();
    assert!(read_receipt.is_some());

    let read_receipt = read_receipt.unwrap();
    assert_eq!(read_receipt.spec_id, "test-special-chars-🎉");
    assert_eq!(
        read_receipt.outputs[0].path,
        "artifacts/test-with-unicode-🚀.md"
    );
    assert!(read_receipt.stderr_tail.as_ref().unwrap().contains('\n'));
}

// ===== Edge Case Tests (Task 9.7) =====

#[test]
fn test_receipt_with_missing_optional_fields() {
    let (manager, _temp_dir) = create_test_manager();

    // Create receipt with all optional fields as None
    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let receipt = manager.create_receipt(
        "test-spec",
        PhaseId::Requirements,
        0,
        vec![],
        "0.1.0",
        "0.8.1",
        "haiku",
        None, // model_alias
        HashMap::new(),
        packet,
        None,   // stderr_tail
        None,   // stderr_redacted
        vec![], // warnings
        None,   // fallback_used
        "native",
        None, // runner_distro
        None, // error_kind
        None, // error_reason
        None, // diff_context,
        None, // pipeline
    );

    // Verify optional fields are None
    assert_eq!(receipt.model_alias, None);
    assert_eq!(receipt.stderr_tail, None);
    assert_eq!(receipt.stderr_redacted, None);
    assert!(receipt.warnings.is_empty());
    assert_eq!(receipt.fallback_used, None);
    assert_eq!(receipt.runner_distro, None);
    assert_eq!(receipt.error_kind, None);
    assert_eq!(receipt.error_reason, None);
    assert_eq!(receipt.diff_context, None);

    // Verify it can be serialized and deserialized
    let json = serde_json::to_string(&receipt).unwrap();
    let deserialized: Receipt = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.model_alias, None);
}

#[test]
fn test_receipt_with_empty_strings() {
    let (manager, _temp_dir) = create_test_manager();

    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let receipt = manager.create_receipt(
        "", // Empty spec_id
        PhaseId::Requirements,
        0,
        vec![],
        "",                  // Empty version
        "",                  // Empty CLI version
        "",                  // Empty model name
        Some(String::new()), // Empty model alias
        HashMap::new(),
        packet,
        Some(String::new()), // Empty stderr_tail
        Some(String::new()), // Empty stderr_redacted
        vec![],
        None,
        "",                  // Empty runner
        Some(String::new()), // Empty runner_distro
        None,
        Some(String::new()), // Empty error_reason
        None,                // diff_context
        None,                // pipeline
    );

    assert_eq!(receipt.spec_id, "");
    assert_eq!(receipt.xchecker_version, "");
    assert_eq!(receipt.claude_cli_version, "");
    assert_eq!(receipt.model_full_name, "");
    assert_eq!(receipt.model_alias, Some(String::new()));
    assert_eq!(receipt.runner, "");
}

#[test]
fn test_receipt_with_very_long_strings() {
    let (manager, _temp_dir) = create_test_manager();

    let long_string = "a".repeat(10000);
    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let receipt = manager.create_receipt(
        &long_string,
        PhaseId::Requirements,
        0,
        vec![],
        &long_string,
        &long_string,
        &long_string,
        Some(long_string.clone()),
        HashMap::new(),
        packet,
        Some(long_string.clone()),
        Some(long_string.clone()),
        vec![long_string.clone()],
        None,
        &long_string,
        Some(long_string.clone()),
        None,
        Some(long_string.clone()),
        None, // diff_context
        None, // pipeline
    );

    assert_eq!(receipt.spec_id.len(), 10000);
    assert_eq!(receipt.xchecker_version.len(), 10000);
}

#[test]
fn test_receipt_with_unicode_content() {
    let (manager, _temp_dir) = create_test_manager();

    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let receipt = manager.create_receipt(
        "测试-spec-🚀",
        PhaseId::Requirements,
        0,
        vec![],
        "版本-1.0",
        "CLI-版本-2.0",
        "claude-模型-✨",
        Some("别名-🌟".to_string()),
        HashMap::new(),
        packet,
        Some("错误-日志-📝".to_string()),
        None,
        vec!["警告-⚠️".to_string()],
        None,
        "运行器-🏃",
        Some("发行版-🐧".to_string()),
        None,
        None,
        None, // diff_context
        None, // pipeline
    );

    assert_eq!(receipt.spec_id, "测试-spec-🚀");
    assert_eq!(receipt.xchecker_version, "版本-1.0");
    assert_eq!(receipt.model_full_name, "claude-模型-✨");
    assert!(receipt.warnings.contains(&"警告-⚠️".to_string()));
}

#[test]
fn test_receipt_with_special_characters() {
    let (manager, _temp_dir) = create_test_manager();

    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let receipt = manager.create_receipt(
        "spec-with-@#$%",
        PhaseId::Requirements,
        0,
        vec![],
        "v1.0-beta+build.123",
        "0.8.1-rc.1",
        "haiku",
        Some("alias-with-!@#".to_string()),
        HashMap::new(),
        packet,
        Some("Error: <>&\"'".to_string()),
        None,
        vec!["Warning: {}[]".to_string()],
        None,
        "native",
        None,
        None,
        None,
        None, // diff_context
        None, // pipeline
    );

    assert_eq!(receipt.spec_id, "spec-with-@#$%");
    assert_eq!(receipt.xchecker_version, "v1.0-beta+build.123");
}

#[test]
fn test_receipt_with_maximum_outputs() {
    let (manager, _temp_dir) = create_test_manager();

    // Create a large number of outputs
    let mut outputs = Vec::new();
    for i in 0..1000 {
        outputs.push(FileHash {
            path: format!("artifacts/file-{i}.md"),
            blake3_canonicalized: format!("hash{i:064}"),
        });
    }

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
        None, // diff_context
        None, // pipeline
    );

    assert_eq!(receipt.outputs.len(), 1000);
    // Verify outputs are sorted
    for i in 0..999 {
        assert!(receipt.outputs[i].path <= receipt.outputs[i + 1].path);
    }
}

#[test]
fn test_receipt_with_all_error_kinds() {
    let (manager, _temp_dir) = create_test_manager();

    let error_kinds = vec![
        ErrorKind::CliArgs,
        ErrorKind::PacketOverflow,
        ErrorKind::SecretDetected,
        ErrorKind::LockHeld,
        ErrorKind::PhaseTimeout,
        ErrorKind::ClaudeFailure,
        ErrorKind::Unknown,
    ];

    for error_kind in error_kinds {
        let packet = PacketEvidence {
            files: vec![],
            max_bytes: 65536,
            max_lines: 1200,
        };

        let error_kind_clone = error_kind.clone();
        let receipt = manager.create_receipt(
            "test-spec",
            PhaseId::Requirements,
            1,
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
            None,
            "native",
            None,
            Some(error_kind_clone.clone()),
            Some(format!("Error: {error_kind_clone:?}")),
            None, // diff_context
            None, // pipeline
        );

        assert_eq!(receipt.error_kind, Some(error_kind));
        assert!(receipt.error_reason.is_some());
    }
}

#[test]
fn test_receipt_with_negative_exit_code() {
    let (manager, _temp_dir) = create_test_manager();

    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let receipt = manager.create_receipt(
        "test-spec",
        PhaseId::Requirements,
        -1, // Negative exit code
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
        None,
        "native",
        None,
        None,
        None,
        None, // diff_context
        None, // pipeline
    );

    assert_eq!(receipt.exit_code, -1);
}

#[test]
fn test_receipt_with_large_exit_code() {
    let (manager, _temp_dir) = create_test_manager();

    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let receipt = manager.create_receipt(
        "test-spec",
        PhaseId::Requirements,
        255, // Maximum typical exit code
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
        None,
        "native",
        None,
        None,
        None,
        None, // diff_context
        None, // pipeline
    );

    assert_eq!(receipt.exit_code, 255);
}

#[test]
fn test_receipt_serialization_with_null_values() {
    let (manager, _temp_dir) = create_test_manager();

    let packet = PacketEvidence {
        files: vec![],
        max_bytes: 65536,
        max_lines: 1200,
    };

    let receipt = manager.create_receipt(
        "test-spec",
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
        None,
        "native",
        None,
        None,
        None,
        None, // diff_context
        None, // pipeline
    );

    let json = serde_json::to_string(&receipt).unwrap();

    // Verify null values are properly serialized
    assert!(json.contains("\"model_alias\":null") || !json.contains("model_alias"));
    assert!(json.contains("\"stderr_tail\":null") || !json.contains("stderr_tail"));

    // Verify it can be deserialized
    let deserialized: Receipt = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.model_alias, None);
}

#[test]
fn test_create_file_hash_with_empty_content() {
    let (manager, _temp_dir) = create_test_manager();

    let result =
        manager.create_file_hash("artifacts/empty.md", "", FileType::Markdown, "requirements");

    assert!(result.is_ok());
    let file_hash = result.unwrap();
    assert_eq!(file_hash.path, "artifacts/empty.md");
    assert!(!file_hash.blake3_canonicalized.is_empty());
}

#[test]
fn test_create_file_hash_with_large_content() {
    let (manager, _temp_dir) = create_test_manager();

    let large_content = "x".repeat(1_000_000); // 1MB of content
    let result = manager.create_file_hash(
        "artifacts/large.md",
        &large_content,
        FileType::Markdown,
        "requirements",
    );

    assert!(result.is_ok());
    let file_hash = result.unwrap();
    assert!(!file_hash.blake3_canonicalized.is_empty());
}

// ===== Edge Case Tests for Task 9.7 =====
// (Most tests already exist above, keeping only unique ones)

#[test]
fn test_receipt_deserialization_with_missing_fields() {
    // JSON with only required fields
    let minimal_json = r#"{
        "schema_version": "1",
        "emitted_at": "2024-01-01T00:00:00Z",
        "canonicalization_version": "yaml-v1,md-v1",
        "canonicalization_backend": "jcs-rfc8785",
        "spec_id": "test-spec",
        "phase": "requirements",
        "exit_code": 0,
        "outputs": [],
        "xchecker_version": "0.1.0",
        "claude_cli_version": "0.8.1",
        "model_full_name": "haiku",
        "flags": {},
        "packet": {
            "files": [],
            "max_bytes": 65536,
            "max_lines": 1200
        },
        "warnings": [],
        "runner": "native"
    }"#;

    let receipt: Receipt = serde_json::from_str(minimal_json).unwrap();
    assert_eq!(receipt.schema_version, "1");
    assert_eq!(receipt.exit_code, 0);
    assert!(receipt.model_alias.is_none());
    assert!(receipt.stderr_redacted.is_none());
    assert!(receipt.runner_distro.is_none());
}

#[test]
fn test_error_kind_mapping() {
    use crate::error::XCheckerError;

    // Test various error types map to correct exit codes and kinds
    let errors = vec![
        (
            XCheckerError::PacketOverflow {
                used_bytes: 100,
                limit_bytes: 50,
                used_lines: 10,
                limit_lines: 5,
            },
            7,
            ErrorKind::PacketOverflow,
        ),
        (
            XCheckerError::SecretDetected {
                pattern: "test".to_string(),
                location: "test.txt:1:1".to_string(),
            },
            8,
            ErrorKind::SecretDetected,
        ),
        (
            XCheckerError::ConcurrentExecution {
                id: "test-spec".to_string(),
            },
            9,
            ErrorKind::LockHeld,
        ),
    ];

    for (error, expected_code, expected_kind) in errors {
        let (code, kind) = error_to_exit_code_and_kind(&error);
        assert_eq!(code, expected_code);
        assert_eq!(kind, expected_kind);
    }
}

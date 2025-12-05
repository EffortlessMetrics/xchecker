//! Comprehensive schema compliance tests
//!
//! This test module verifies:
//! - All schemas have additionalProperties: true
//! - Optional fields are correctly documented
//! - Generated examples validate against schemas
//! - Schema drift detection

use serde_json::Value;
use std::fs;

#[test]
fn test_all_schemas_have_additional_properties_true() {
    let schemas = vec![
        ("schemas/receipt.v1.json", "Receipt"),
        ("schemas/status.v1.json", "Status"),
        ("schemas/doctor.v1.json", "Doctor"),
    ];

    for (schema_path, schema_name) in schemas {
        let schema_content = fs::read_to_string(schema_path)
            .unwrap_or_else(|_| panic!("Failed to read schema: {schema_path}"));
        let schema: Value = serde_json::from_str(&schema_content)
            .unwrap_or_else(|_| panic!("Failed to parse schema: {schema_path}"));

        // Check top-level additionalProperties
        let additional_props = schema
            .get("additionalProperties")
            .unwrap_or_else(|| panic!("{schema_name} schema missing additionalProperties field"));

        assert_eq!(
            additional_props,
            &Value::Bool(true),
            "{schema_name} schema should have additionalProperties: true at top level"
        );

        println!("✓ {schema_name} schema has additionalProperties: true");
    }
}

#[test]
fn test_receipt_optional_fields_documented() {
    let schema_content =
        fs::read_to_string("schemas/receipt.v1.json").expect("Failed to read receipt schema");
    let schema: Value =
        serde_json::from_str(&schema_content).expect("Failed to parse receipt schema");

    let properties = schema["properties"].as_object().expect("No properties");
    let required = schema["required"]
        .as_array()
        .expect("No required array")
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect::<Vec<_>>();

    // Optional fields that should be documented
    let optional_fields = vec![
        "model_alias",
        "runner_distro",
        "error_kind",
        "error_reason",
        "stderr_tail",
        "fallback_used",
        // Note: diff_context is in the Rust struct but not yet in the schema
        // This will be added in task 8.3
    ];

    for field in optional_fields {
        assert!(
            properties.contains_key(field),
            "Optional field '{field}' should be documented in receipt schema"
        );
        assert!(
            !required.contains(&field),
            "Optional field '{field}' should not be in required array"
        );
        println!("✓ Receipt optional field '{field}' is documented");
    }
}

#[test]
fn test_status_optional_fields_documented() {
    let schema_content =
        fs::read_to_string("schemas/status.v1.json").expect("Failed to read status schema");
    let schema: Value =
        serde_json::from_str(&schema_content).expect("Failed to parse status schema");

    let properties = schema["properties"].as_object().expect("No properties");
    let required = schema["required"]
        .as_array()
        .expect("No required array")
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect::<Vec<_>>();

    // Optional fields that should be documented
    let optional_fields = vec!["runner_distro", "lock_drift"];

    for field in optional_fields {
        assert!(
            properties.contains_key(field),
            "Optional field '{field}' should be documented in status schema"
        );
        assert!(
            !required.contains(&field),
            "Optional field '{field}' should not be in required array"
        );
        println!("✓ Status optional field '{field}' is documented");
    }
}

#[test]
fn test_generated_examples_exist() {
    let examples = vec![
        "docs/schemas/receipt.v1.minimal.json",
        "docs/schemas/receipt.v1.full.json",
        "docs/schemas/status.v1.minimal.json",
        "docs/schemas/status.v1.full.json",
        "docs/schemas/doctor.v1.minimal.json",
        "docs/schemas/doctor.v1.full.json",
    ];

    for example_path in examples {
        assert!(
            std::path::Path::new(example_path).exists(),
            "Example file should exist: {example_path}"
        );

        // Verify it's valid JSON
        let content = fs::read_to_string(example_path)
            .unwrap_or_else(|_| panic!("Failed to read example: {example_path}"));
        let _: Value = serde_json::from_str(&content)
            .unwrap_or_else(|_| panic!("Example is not valid JSON: {example_path}"));

        println!("✓ Example exists and is valid JSON: {example_path}");
    }
}

#[test]
fn test_all_examples_validate_against_schemas() {
    // Receipt examples
    let receipt_schema = load_schema("schemas/receipt.v1.json");
    validate_example(
        &receipt_schema,
        "docs/schemas/receipt.v1.minimal.json",
        "Receipt minimal",
    );
    validate_example(
        &receipt_schema,
        "docs/schemas/receipt.v1.full.json",
        "Receipt full",
    );

    // Status examples
    let status_schema = load_schema("schemas/status.v1.json");
    validate_example(
        &status_schema,
        "docs/schemas/status.v1.minimal.json",
        "Status minimal",
    );
    validate_example(
        &status_schema,
        "docs/schemas/status.v1.full.json",
        "Status full",
    );

    // Doctor examples
    let doctor_schema = load_schema("schemas/doctor.v1.json");
    validate_example(
        &doctor_schema,
        "docs/schemas/doctor.v1.minimal.json",
        "Doctor minimal",
    );
    validate_example(
        &doctor_schema,
        "docs/schemas/doctor.v1.full.json",
        "Doctor full",
    );

    println!("\n✅ All examples validate against their schemas");
}

#[test]
fn test_schema_version_consistency() {
    // All schemas should have schema_version: "1"
    let schemas = vec![
        ("schemas/receipt.v1.json", "Receipt"),
        ("schemas/status.v1.json", "Status"),
        ("schemas/doctor.v1.json", "Doctor"),
    ];

    for (schema_path, schema_name) in schemas {
        let schema_content = fs::read_to_string(schema_path)
            .unwrap_or_else(|_| panic!("Failed to read schema: {schema_path}"));
        let schema: Value = serde_json::from_str(&schema_content)
            .unwrap_or_else(|_| panic!("Failed to parse schema: {schema_path}"));

        let schema_version = schema["properties"]["schema_version"]["const"]
            .as_str()
            .unwrap_or_else(|| panic!("{schema_name} schema missing schema_version const"));

        assert_eq!(
            schema_version, "1",
            "{schema_name} schema should have schema_version const = '1'"
        );

        println!("✓ {schema_name} schema has schema_version: '1'");
    }
}

#[test]
fn test_blake3_hash_patterns() {
    let receipt_schema = load_schema("schemas/receipt.v1.json");

    // Check blake3_pre_redaction pattern (64 hex chars)
    let packet_files_schema =
        &receipt_schema["properties"]["packet"]["properties"]["files"]["items"];
    let blake3_pre_pattern = packet_files_schema["properties"]["blake3_pre_redaction"]["pattern"]
        .as_str()
        .expect("blake3_pre_redaction should have pattern");
    assert_eq!(
        blake3_pre_pattern, "^[0-9a-f]{64}$",
        "blake3_pre_redaction should be 64 hex chars"
    );

    // Check blake3_canonicalized pattern (64 hex chars)
    let outputs_schema = &receipt_schema["properties"]["outputs"]["items"];
    let blake3_canon_pattern = outputs_schema["properties"]["blake3_canonicalized"]["pattern"]
        .as_str()
        .expect("blake3_canonicalized should have pattern");
    assert_eq!(
        blake3_canon_pattern, "^[0-9a-f]{64}$",
        "blake3_canonicalized should be 64 hex chars"
    );

    // Check blake3_first8 pattern in status schema (8 hex chars)
    let status_schema = load_schema("schemas/status.v1.json");
    let artifacts_schema = &status_schema["properties"]["artifacts"]["items"];
    let blake3_first8_pattern = artifacts_schema["properties"]["blake3_first8"]["pattern"]
        .as_str()
        .expect("blake3_first8 should have pattern");
    assert_eq!(
        blake3_first8_pattern, "^[0-9a-f]{8}$",
        "blake3_first8 should be 8 hex chars"
    );

    println!("✓ BLAKE3 hash patterns are correct");
}

#[test]
fn test_runner_enum_values() {
    let schemas = vec![
        ("schemas/receipt.v1.json", "Receipt"),
        ("schemas/status.v1.json", "Status"),
    ];

    for (schema_path, schema_name) in schemas {
        let schema = load_schema(schema_path);
        let runner_enum = schema["properties"]["runner"]["enum"]
            .as_array()
            .unwrap_or_else(|| panic!("{schema_name} schema missing runner enum"));

        let expected_values = vec!["native", "wsl"];
        let actual_values: Vec<&str> = runner_enum.iter().map(|v| v.as_str().unwrap()).collect();

        assert_eq!(
            actual_values, expected_values,
            "{schema_name} schema runner enum should be ['native', 'wsl']"
        );

        println!("✓ {schema_name} schema has correct runner enum values");
    }
}

#[test]
fn test_stderr_max_length() {
    let receipt_schema = load_schema("schemas/receipt.v1.json");
    let stderr_max_length = receipt_schema["properties"]["stderr_tail"]["maxLength"]
        .as_u64()
        .expect("stderr_tail should have maxLength");

    assert_eq!(
        stderr_max_length, 2048,
        "stderr_tail should have maxLength: 2048"
    );

    println!("✓ Receipt schema has stderr_tail maxLength: 2048");
}

// Helper functions

fn load_schema(path: &str) -> Value {
    let content =
        fs::read_to_string(path).unwrap_or_else(|_| panic!("Failed to read schema: {path}"));
    serde_json::from_str(&content).unwrap_or_else(|_| panic!("Failed to parse schema: {path}"))
}

fn validate_example(schema: &Value, example_path: &str, name: &str) {
    let example_content = fs::read_to_string(example_path)
        .unwrap_or_else(|_| panic!("Failed to read example: {example_path}"));
    let example: Value = serde_json::from_str(&example_content)
        .unwrap_or_else(|_| panic!("Failed to parse example: {example_path}"));

    let validator = jsonschema::validator_for(schema)
        .unwrap_or_else(|_| panic!("Failed to compile schema for {name}"));

    if let Err(error) = validator.validate(&example) {
        panic!("{} failed validation:\n{}", name, error);
    }

    println!("✓ {name} validates against schema");
}

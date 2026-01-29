/// Test JSON schema validation for receipt, status, and doctor schemas
///
/// This test validates that the example payloads conform to their respective JSON schemas.
/// It uses the jsonschema crate to validate the examples against the schema definitions.
use std::fs;

fn validate_example(schema_path: &str, example_path: &str) {
    let schema_content = fs::read_to_string(schema_path)
        .unwrap_or_else(|_| panic!("Failed to read schema: {}", schema_path));
    let schema: serde_json::Value =
        serde_json::from_str(&schema_content).expect("Failed to parse schema JSON");

    let example_content = fs::read_to_string(example_path)
        .unwrap_or_else(|_| panic!("Failed to read example: {}", example_path));
    let example: serde_json::Value =
        serde_json::from_str(&example_content).expect("Failed to parse example JSON");

    let validator = jsonschema::validator_for(&schema).expect("Failed to compile schema");

    if let Err(error) = validator.validate(&example) {
        panic!(
            "Validation failed for {} against {}:\n{}",
            example_path, schema_path, error
        );
    }
}

#[test]
fn test_receipt_minimal_validates_against_schema() {
    validate_example(
        "schemas/receipt.v1.json",
        "docs/schemas/receipt.v1.minimal.json",
    );
}

#[test]
fn test_receipt_full_validates_against_schema() {
    validate_example(
        "schemas/receipt.v1.json",
        "docs/schemas/receipt.v1.full.json",
    );
}

#[test]
fn test_status_minimal_validates_against_schema() {
    validate_example(
        "schemas/status.v1.json",
        "docs/schemas/status.v1.minimal.json",
    );
}

#[test]
fn test_status_full_validates_against_schema() {
    validate_example("schemas/status.v1.json", "docs/schemas/status.v1.full.json");
}

#[test]
fn test_doctor_minimal_validates_against_schema() {
    validate_example(
        "schemas/doctor.v1.json",
        "docs/schemas/doctor.v1.minimal.json",
    );
}

#[test]
fn test_doctor_full_validates_against_schema() {
    validate_example("schemas/doctor.v1.json", "docs/schemas/doctor.v1.full.json");
}

#[test]
fn test_receipt_schema_constraints() {
    // Load schema
    let schema_content =
        fs::read_to_string("schemas/receipt.v1.json").expect("Failed to read receipt schema");
    let schema: serde_json::Value =
        serde_json::from_str(&schema_content).expect("Failed to parse receipt schema");

    // Verify key constraints are present
    let properties = schema["properties"]
        .as_object()
        .expect("Schema should have properties");

    // Verify runner enum constraint
    let runner = &properties["runner"];
    assert_eq!(runner["enum"], serde_json::json!(["native", "wsl"]));

    // Verify blake3_first8 pattern in outputs
    let outputs = &properties["outputs"]["items"]["properties"]["blake3_canonicalized"];
    assert_eq!(outputs["pattern"], "^[0-9a-f]{64}$");

    // Verify stderr_tail maxLength
    let stderr_tail = &properties["stderr_tail"];
    assert_eq!(stderr_tail["maxLength"], 2048);

    // Verify additionalProperties is true
    assert_eq!(schema["additionalProperties"], true);
}

#[test]
fn test_status_schema_constraints() {
    // Load schema
    let schema_content =
        fs::read_to_string("schemas/status.v1.json").expect("Failed to read status schema");
    let schema: serde_json::Value =
        serde_json::from_str(&schema_content).expect("Failed to parse status schema");

    // Verify key constraints are present
    let properties = schema["properties"]
        .as_object()
        .expect("Schema should have properties");

    // Verify runner enum constraint
    let runner = &properties["runner"];
    assert_eq!(runner["enum"], serde_json::json!(["native", "wsl"]));

    // Verify blake3_first8 pattern in artifacts
    let artifacts = &properties["artifacts"]["items"]["properties"]["blake3_first8"];
    assert_eq!(artifacts["pattern"], "^[0-9a-f]{8}$");

    // Verify additionalProperties is true
    assert_eq!(schema["additionalProperties"], true);
}

#[test]
fn test_doctor_schema_constraints() {
    // Load schema
    let schema_content =
        fs::read_to_string("schemas/doctor.v1.json").expect("Failed to read doctor schema");
    let schema: serde_json::Value =
        serde_json::from_str(&schema_content).expect("Failed to parse doctor schema");

    // Verify key constraints are present
    let properties = schema["properties"]
        .as_object()
        .expect("Schema should have properties");

    // Verify checks array has status enum
    let checks = &properties["checks"]["items"]["properties"]["status"];
    assert_eq!(checks["enum"], serde_json::json!(["pass", "warn", "fail"]));

    // Verify additionalProperties is true
    assert_eq!(schema["additionalProperties"], true);
}

#[test]
fn test_gate_schema_validates_examples() {
    // Load schema
    let schema_content =
        fs::read_to_string("docs/schemas/gate-json.v1.json").expect("Failed to read gate schema");
    let schema: serde_json::Value =
        serde_json::from_str(&schema_content).expect("Failed to parse gate schema");

    // Validate using jsonschema crate
    let validator = jsonschema::validator_for(&schema).expect("Failed to compile gate schema");

    // Validate the examples embedded in the schema
    if let Some(examples) = schema.get("examples").and_then(|e| e.as_array()) {
        for (i, example) in examples.iter().enumerate() {
            if let Err(error) = validator.validate(example) {
                panic!("Gate example {} failed validation:\n{}", i, error);
            }
        }
    }
}

#[test]
fn test_gate_schema_constraints() {
    // Load schema
    let schema_content =
        fs::read_to_string("docs/schemas/gate-json.v1.json").expect("Failed to read gate schema");
    let schema: serde_json::Value =
        serde_json::from_str(&schema_content).expect("Failed to parse gate schema");

    // Verify key constraints are present
    let properties = schema["properties"]
        .as_object()
        .expect("Schema should have properties");

    // Verify schema_version is const
    let schema_version = &properties["schema_version"];
    assert_eq!(schema_version["const"], "gate-json.v1");

    // Verify passed is boolean
    let passed = &properties["passed"];
    assert_eq!(passed["type"], "boolean");

    // Verify conditions is array
    let conditions = &properties["conditions"];
    assert_eq!(conditions["type"], "array");

    // Verify failure_reasons is array of strings
    let failure_reasons = &properties["failure_reasons"];
    assert_eq!(failure_reasons["type"], "array");
    assert_eq!(failure_reasons["items"]["type"], "string");

    // Verify required fields
    let required = schema["required"]
        .as_array()
        .expect("Schema should have required array");
    let required_fields: Vec<&str> = required.iter().filter_map(|v| v.as_str()).collect();
    assert!(required_fields.contains(&"schema_version"));
    assert!(required_fields.contains(&"spec_id"));
    assert!(required_fields.contains(&"passed"));
    assert!(required_fields.contains(&"conditions"));
    assert!(required_fields.contains(&"failure_reasons"));
    assert!(required_fields.contains(&"summary"));
}

#[test]
fn test_gate_json_output_matches_schema() {
    use xchecker::gate::{GateCondition, GateResult, emit_gate_json};

    // Create a test gate result
    let result = GateResult {
        schema_version: "gate-json.v1".to_string(),
        spec_id: "test-spec".to_string(),
        passed: true,
        conditions: vec![GateCondition {
            name: "min_phase".to_string(),
            passed: true,
            description: "Required phase 'tasks' is completed".to_string(),
            actual: Some("tasks completed".to_string()),
            expected: Some("tasks or later".to_string()),
        }],
        failure_reasons: vec![],
        summary: "Gate PASSED: Spec 'test-spec' meets all policy requirements".to_string(),
    };

    // Emit JSON
    let json_str = emit_gate_json(&result, "test-spec").expect("Failed to emit gate JSON");
    let json_value: serde_json::Value =
        serde_json::from_str(&json_str).expect("Failed to parse emitted JSON");

    // Load schema
    let schema_content =
        fs::read_to_string("docs/schemas/gate-json.v1.json").expect("Failed to read gate schema");
    let schema: serde_json::Value =
        serde_json::from_str(&schema_content).expect("Failed to parse gate schema");

    // Validate
    let validator = jsonschema::validator_for(&schema).expect("Failed to compile gate schema");

    if let Err(error) = validator.validate(&json_value) {
        panic!("Gate JSON output failed schema validation:\n{}", error);
    }

    // Verify schema_version is correct
    assert_eq!(json_value["schema_version"], "gate-json.v1");
}

//! M2 Gate: Validate example generation
//!
//! This test module validates that Milestone 2 is complete by verifying:
//! - All example generation tests pass
//! - Generated JSON files validate against schemas
//! - Arrays are sorted in all examples
//! - Examples use fixed timestamps for determinism
//!
//! Requirements: R2

use serde_json;
use std::fs;
use std::path::Path;

use crate::doc_validation::common::SchemaValidator;
use xchecker::example_generators::{
    make_example_doctor_full, make_example_doctor_minimal, make_example_receipt_full,
    make_example_receipt_minimal, make_example_status_full, make_example_status_minimal,
    fixed_now,
};

#[test]
fn m2_gate_all_example_generation_tests_pass() {
    // This test verifies that all example generation tests pass
    // by running the core validation logic

    let validator = SchemaValidator::new().expect("Should load schemas");

    // Test receipt examples
    let receipt_minimal = make_example_receipt_minimal();
    let receipt_minimal_json = serde_json::to_value(&receipt_minimal)
        .expect("Should serialize minimal receipt");
    validator
        .validate("receipt.v1", &receipt_minimal_json)
        .expect("Minimal receipt should validate");

    let receipt_full = make_example_receipt_full();
    let receipt_full_json = serde_json::to_value(&receipt_full)
        .expect("Should serialize full receipt");
    validator
        .validate("receipt.v1", &receipt_full_json)
        .expect("Full receipt should validate");

    // Test status examples
    let status_minimal = make_example_status_minimal();
    let status_minimal_json = serde_json::to_value(&status_minimal)
        .expect("Should serialize minimal status");
    validator
        .validate("status.v1", &status_minimal_json)
        .expect("Minimal status should validate");

    let status_full = make_example_status_full();
    let status_full_json = serde_json::to_value(&status_full)
        .expect("Should serialize full status");
    validator
        .validate("status.v1", &status_full_json)
        .expect("Full status should validate");

    // Test doctor examples
    let doctor_minimal = make_example_doctor_minimal();
    let doctor_minimal_json = serde_json::to_value(&doctor_minimal)
        .expect("Should serialize minimal doctor");
    validator
        .validate("doctor.v1", &doctor_minimal_json)
        .expect("Minimal doctor should validate");

    let doctor_full = make_example_doctor_full();
    let doctor_full_json = serde_json::to_value(&doctor_full)
        .expect("Should serialize full doctor");
    validator
        .validate("doctor.v1", &doctor_full_json)
        .expect("Full doctor should validate");
}

#[test]
fn m2_gate_generated_json_files_exist_and_validate() {
    // Verify that generated JSON files exist in docs/schemas/ and validate against schemas
    let validator = SchemaValidator::new().expect("Should load schemas");

    let test_cases = vec![
        ("docs/schemas/receipt-minimal.json", "receipt.v1"),
        ("docs/schemas/receipt-full.json", "receipt.v1"),
        ("docs/schemas/status-minimal.json", "status.v1"),
        ("docs/schemas/status-full.json", "status.v1"),
        ("docs/schemas/doctor-minimal.json", "doctor.v1"),
        ("docs/schemas/doctor-full.json", "doctor.v1"),
    ];

    for (file_path, schema_name) in test_cases {
        let path = Path::new(file_path);
        assert!(
            path.exists(),
            "Generated file should exist: {}",
            file_path
        );

        let content = fs::read_to_string(path)
            .expect(&format!("Should read file: {}", file_path));
        let json: serde_json::Value = serde_json::from_str(&content)
            .expect(&format!("Should parse JSON: {}", file_path));

        validator
            .validate(schema_name, &json)
            .expect(&format!(
                "Generated file should validate against schema: {}",
                file_path
            ));
    }
}

#[test]
fn m2_gate_arrays_are_sorted_in_all_examples() {
    // Verify that all arrays in generated examples are sorted

    // Receipt outputs sorted by path
    let receipt_full = make_example_receipt_full();
    let receipt_json = serde_json::to_value(&receipt_full).expect("Should serialize receipt");
    
    if let Some(outputs) = receipt_json.get("outputs").and_then(|v| v.as_array()) {
        let paths: Vec<&str> = outputs
            .iter()
            .filter_map(|o| o.get("path").and_then(|p| p.as_str()))
            .collect();
        let mut sorted_paths = paths.clone();
        sorted_paths.sort();
        assert_eq!(
            paths, sorted_paths,
            "Receipt outputs should be sorted by path"
        );
    }

    // Receipt packet files sorted by path
    if let Some(files) = receipt_json
        .get("packet")
        .and_then(|p| p.get("files"))
        .and_then(|f| f.as_array())
    {
        let paths: Vec<&str> = files
            .iter()
            .filter_map(|f| f.get("path").and_then(|p| p.as_str()))
            .collect();
        let mut sorted_paths = paths.clone();
        sorted_paths.sort();
        assert_eq!(
            paths, sorted_paths,
            "Receipt packet files should be sorted by path"
        );
    }

    // Status artifacts sorted by path
    let status_full = make_example_status_full();
    let status_json = serde_json::to_value(&status_full).expect("Should serialize status");
    
    if let Some(artifacts) = status_json.get("artifacts").and_then(|v| v.as_array()) {
        let paths: Vec<&str> = artifacts
            .iter()
            .filter_map(|a| a.get("path").and_then(|p| p.as_str()))
            .collect();
        let mut sorted_paths = paths.clone();
        sorted_paths.sort();
        assert_eq!(
            paths, sorted_paths,
            "Status artifacts should be sorted by path"
        );
    }

    // Doctor checks sorted by name
    let doctor_full = make_example_doctor_full();
    let doctor_json = serde_json::to_value(&doctor_full).expect("Should serialize doctor");
    
    if let Some(checks) = doctor_json.get("checks").and_then(|v| v.as_array()) {
        let names: Vec<&str> = checks
            .iter()
            .filter_map(|c| c.get("name").and_then(|n| n.as_str()))
            .collect();
        let mut sorted_names = names.clone();
        sorted_names.sort();
        assert_eq!(
            names, sorted_names,
            "Doctor checks should be sorted by name"
        );
    }
}

#[test]
fn m2_gate_examples_use_fixed_timestamps() {
    // Verify that all examples use the fixed timestamp for determinism
    // Note: RFC3339 format can be either "Z" or "+00:00" for UTC
    let expected_timestamp_z = "2025-01-01T00:00:00Z";
    let expected_timestamp_offset = "2025-01-01T00:00:00+00:00";

    // Check receipt examples
    let receipt_minimal = make_example_receipt_minimal();
    assert_eq!(
        receipt_minimal.emitted_at,
        fixed_now(),
        "Receipt minimal should use fixed timestamp"
    );
    let receipt_ts = receipt_minimal.emitted_at.to_rfc3339();
    assert!(
        receipt_ts == expected_timestamp_z || receipt_ts == expected_timestamp_offset,
        "Receipt minimal timestamp should be {} or {}, got {}",
        expected_timestamp_z,
        expected_timestamp_offset,
        receipt_ts
    );

    let receipt_full = make_example_receipt_full();
    assert_eq!(
        receipt_full.emitted_at,
        fixed_now(),
        "Receipt full should use fixed timestamp"
    );

    // Check status examples
    let status_minimal = make_example_status_minimal();
    assert_eq!(
        status_minimal.emitted_at,
        fixed_now(),
        "Status minimal should use fixed timestamp"
    );
    let status_ts = status_minimal.emitted_at.to_rfc3339();
    assert!(
        status_ts == expected_timestamp_z || status_ts == expected_timestamp_offset,
        "Status minimal timestamp should be {} or {}, got {}",
        expected_timestamp_z,
        expected_timestamp_offset,
        status_ts
    );

    let status_full = make_example_status_full();
    assert_eq!(
        status_full.emitted_at,
        fixed_now(),
        "Status full should use fixed timestamp"
    );

    // Check doctor examples
    let doctor_minimal = make_example_doctor_minimal();
    assert_eq!(
        doctor_minimal.emitted_at,
        fixed_now(),
        "Doctor minimal should use fixed timestamp"
    );
    let doctor_ts = doctor_minimal.emitted_at.to_rfc3339();
    assert!(
        doctor_ts == expected_timestamp_z || doctor_ts == expected_timestamp_offset,
        "Doctor minimal timestamp should be {} or {}, got {}",
        expected_timestamp_z,
        expected_timestamp_offset,
        doctor_ts
    );

    let doctor_full = make_example_doctor_full();
    assert_eq!(
        doctor_full.emitted_at,
        fixed_now(),
        "Doctor full should use fixed timestamp"
    );

    // Verify fixed_now() is consistent
    let ts1 = fixed_now();
    let ts2 = fixed_now();
    assert_eq!(ts1, ts2, "fixed_now() should return consistent timestamp");
}

#[test]
fn m2_gate_comprehensive_validation() {
    // This is a comprehensive test that validates all M2 Gate requirements in one place
    
    println!("M2 Gate Validation:");
    println!("===================");
    
    // 1. Run example generation tests and verify all pass
    println!("✓ Example generation tests pass");
    m2_gate_all_example_generation_tests_pass();
    
    // 2. Verify generated JSON files validate against schemas
    println!("✓ Generated JSON files validate against schemas");
    m2_gate_generated_json_files_exist_and_validate();
    
    // 3. Verify arrays are sorted in all examples
    println!("✓ Arrays are sorted in all examples");
    m2_gate_arrays_are_sorted_in_all_examples();
    
    // 4. Check that examples use fixed timestamps for determinism
    println!("✓ Examples use fixed timestamps for determinism");
    m2_gate_examples_use_fixed_timestamps();
    
    println!("\nM2 Gate: PASSED ✓");
    println!("All Milestone 2 requirements validated successfully.");
}

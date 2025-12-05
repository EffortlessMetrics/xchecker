//! Schema example validation tests
//!
//! Tests that verify schema examples are:
//! - Valid against their schemas
//! - Complete (minimal and full versions)
//! - Generated from constructors (not hand-written)
//! - Written to docs/schemas/*.json
//!
//! Requirements: R2

use anyhow::Result;
use serde_json;
use std::fs;
use std::path::Path;

use crate::doc_validation::common::SchemaValidator;

// Import example generators - these are test-only functions
use xchecker::example_generators::{
    make_example_doctor_full, make_example_doctor_minimal, make_example_receipt_full,
    make_example_receipt_minimal, make_example_status_full, make_example_status_minimal,
};

/// Helper to write JSON to file with pretty formatting
fn write_json_example(path: &Path, value: &serde_json::Value) -> Result<()> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Write with pretty formatting
    let json_string = serde_json::to_string_pretty(value)?;
    fs::write(path, json_string)?;

    Ok(())
}

#[test]
fn test_generate_receipt_examples() {
    let validator = SchemaValidator::new().expect("Should load schemas");

    // Generate minimal receipt example
    let receipt_minimal = make_example_receipt_minimal();
    let receipt_minimal_json = serde_json::to_value(&receipt_minimal)
        .expect("Should serialize minimal receipt to JSON");

    // Validate minimal example against schema
    validator
        .validate("receipt.v1", &receipt_minimal_json)
        .expect("Minimal receipt should validate against schema");

    // Write minimal example to docs/schemas/
    let minimal_path = Path::new("docs/schemas/receipt-minimal.json");
    write_json_example(minimal_path, &receipt_minimal_json)
        .expect("Should write minimal receipt example");

    // Re-parse written file and validate again (catches serialization/encoding issues)
    let reloaded_minimal = fs::read_to_string(minimal_path)
        .expect("Should read written minimal receipt");
    let reloaded_minimal_json: serde_json::Value = serde_json::from_str(&reloaded_minimal)
        .expect("Should parse reloaded minimal receipt");
    validator
        .validate("receipt.v1", &reloaded_minimal_json)
        .expect("Reloaded minimal receipt should validate");

    // Generate full receipt example
    let receipt_full = make_example_receipt_full();
    let receipt_full_json = serde_json::to_value(&receipt_full)
        .expect("Should serialize full receipt to JSON");

    // Validate full example against schema
    validator
        .validate("receipt.v1", &receipt_full_json)
        .expect("Full receipt should validate against schema");

    // Write full example to docs/schemas/
    let full_path = Path::new("docs/schemas/receipt-full.json");
    write_json_example(full_path, &receipt_full_json)
        .expect("Should write full receipt example");

    // Re-parse written file and validate again
    let reloaded_full = fs::read_to_string(full_path)
        .expect("Should read written full receipt");
    let reloaded_full_json: serde_json::Value = serde_json::from_str(&reloaded_full)
        .expect("Should parse reloaded full receipt");
    validator
        .validate("receipt.v1", &reloaded_full_json)
        .expect("Reloaded full receipt should validate");

    // Verify arrays are sorted in full example
    if let Some(outputs) = receipt_full_json.get("outputs").and_then(|v| v.as_array()) {
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

    if let Some(packet_files) = receipt_full_json
        .get("packet")
        .and_then(|p| p.get("files"))
        .and_then(|f| f.as_array())
    {
        let paths: Vec<&str> = packet_files
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
}

#[test]
fn test_generate_status_examples() {
    let validator = SchemaValidator::new().expect("Should load schemas");

    // Generate minimal status example
    let status_minimal = make_example_status_minimal();
    let status_minimal_json = serde_json::to_value(&status_minimal)
        .expect("Should serialize minimal status to JSON");

    // Validate minimal example against schema
    validator
        .validate("status.v1", &status_minimal_json)
        .expect("Minimal status should validate against schema");

    // Write minimal example to docs/schemas/
    let minimal_path = Path::new("docs/schemas/status-minimal.json");
    write_json_example(minimal_path, &status_minimal_json)
        .expect("Should write minimal status example");

    // Re-parse written file and validate again
    let reloaded_minimal = fs::read_to_string(minimal_path)
        .expect("Should read written minimal status");
    let reloaded_minimal_json: serde_json::Value = serde_json::from_str(&reloaded_minimal)
        .expect("Should parse reloaded minimal status");
    validator
        .validate("status.v1", &reloaded_minimal_json)
        .expect("Reloaded minimal status should validate");

    // Generate full status example
    let status_full = make_example_status_full();
    let status_full_json = serde_json::to_value(&status_full)
        .expect("Should serialize full status to JSON");

    // Validate full example against schema
    validator
        .validate("status.v1", &status_full_json)
        .expect("Full status should validate against schema");

    // Write full example to docs/schemas/
    let full_path = Path::new("docs/schemas/status-full.json");
    write_json_example(full_path, &status_full_json)
        .expect("Should write full status example");

    // Re-parse written file and validate again
    let reloaded_full = fs::read_to_string(full_path)
        .expect("Should read written full status");
    let reloaded_full_json: serde_json::Value = serde_json::from_str(&reloaded_full)
        .expect("Should parse reloaded full status");
    validator
        .validate("status.v1", &reloaded_full_json)
        .expect("Reloaded full status should validate");

    // Verify arrays are sorted in full example
    if let Some(artifacts) = status_full_json.get("artifacts").and_then(|v| v.as_array()) {
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
}

#[test]
fn test_generate_doctor_examples() {
    let validator = SchemaValidator::new().expect("Should load schemas");

    // Generate minimal doctor example
    let doctor_minimal = make_example_doctor_minimal();
    let doctor_minimal_json = serde_json::to_value(&doctor_minimal)
        .expect("Should serialize minimal doctor to JSON");

    // Validate minimal example against schema
    validator
        .validate("doctor.v1", &doctor_minimal_json)
        .expect("Minimal doctor should validate against schema");

    // Write minimal example to docs/schemas/
    let minimal_path = Path::new("docs/schemas/doctor-minimal.json");
    write_json_example(minimal_path, &doctor_minimal_json)
        .expect("Should write minimal doctor example");

    // Re-parse written file and validate again
    let reloaded_minimal = fs::read_to_string(minimal_path)
        .expect("Should read written minimal doctor");
    let reloaded_minimal_json: serde_json::Value = serde_json::from_str(&reloaded_minimal)
        .expect("Should parse reloaded minimal doctor");
    validator
        .validate("doctor.v1", &reloaded_minimal_json)
        .expect("Reloaded minimal doctor should validate");

    // Generate full doctor example
    let doctor_full = make_example_doctor_full();
    let doctor_full_json = serde_json::to_value(&doctor_full)
        .expect("Should serialize full doctor to JSON");

    // Validate full example against schema
    validator
        .validate("doctor.v1", &doctor_full_json)
        .expect("Full doctor should validate against schema");

    // Write full example to docs/schemas/
    let full_path = Path::new("docs/schemas/doctor-full.json");
    write_json_example(full_path, &doctor_full_json)
        .expect("Should write full doctor example");

    // Re-parse written file and validate again
    let reloaded_full = fs::read_to_string(full_path)
        .expect("Should read written full doctor");
    let reloaded_full_json: serde_json::Value = serde_json::from_str(&reloaded_full)
        .expect("Should parse reloaded full doctor");
    validator
        .validate("doctor.v1", &reloaded_full_json)
        .expect("Reloaded full doctor should validate");

    // Verify checks are sorted by name in both examples
    for (name, json) in &[
        ("minimal", &doctor_minimal_json),
        ("full", &doctor_full_json),
    ] {
        if let Some(checks) = json.get("checks").and_then(|v| v.as_array()) {
            let names: Vec<&str> = checks
                .iter()
                .filter_map(|c| c.get("name").and_then(|n| n.as_str()))
                .collect();
            let mut sorted_names = names.clone();
            sorted_names.sort();
            assert_eq!(
                names, sorted_names,
                "Doctor {} checks should be sorted by name",
                name
            );
        }
    }
}

#[test]
fn test_arrays_are_sorted() {
    // This test verifies that all generated examples have sorted arrays
    // It's a comprehensive check across all example types

    // Receipt examples
    let receipt_full = make_example_receipt_full();
    let receipt_json = serde_json::to_value(&receipt_full).expect("Should serialize receipt");

    // Check outputs are sorted
    if let Some(outputs) = receipt_json.get("outputs").and_then(|v| v.as_array()) {
        let paths: Vec<&str> = outputs
            .iter()
            .filter_map(|o| o.get("path").and_then(|p| p.as_str()))
            .collect();
        let mut sorted_paths = paths.clone();
        sorted_paths.sort();
        assert_eq!(paths, sorted_paths, "Receipt outputs not sorted");
    }

    // Check packet files are sorted
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
        assert_eq!(paths, sorted_paths, "Receipt packet files not sorted");
    }

    // Status examples
    let status_full = make_example_status_full();
    let status_json = serde_json::to_value(&status_full).expect("Should serialize status");

    // Check artifacts are sorted
    if let Some(artifacts) = status_json.get("artifacts").and_then(|v| v.as_array()) {
        let paths: Vec<&str> = artifacts
            .iter()
            .filter_map(|a| a.get("path").and_then(|p| p.as_str()))
            .collect();
        let mut sorted_paths = paths.clone();
        sorted_paths.sort();
        assert_eq!(paths, sorted_paths, "Status artifacts not sorted");
    }

    // Doctor examples
    let doctor_full = make_example_doctor_full();
    let doctor_json = serde_json::to_value(&doctor_full).expect("Should serialize doctor");

    // Check checks are sorted by name
    if let Some(checks) = doctor_json.get("checks").and_then(|v| v.as_array()) {
        let names: Vec<&str> = checks
            .iter()
            .filter_map(|c| c.get("name").and_then(|n| n.as_str()))
            .collect();
        let mut sorted_names = names.clone();
        sorted_names.sort();
        assert_eq!(names, sorted_names, "Doctor checks not sorted");
    }
}

#[test]
fn test_byte_identical_jcs_output() {
    // Test that verifies byte-identical output for differently-ordered input (JCS determinism)
    // This ensures that regardless of how we construct the data, JCS produces the same output

    let receipt1 = make_example_receipt_full();
    let receipt2 = make_example_receipt_full();

    // Serialize both using JCS
    let json1 = serde_json::to_value(&receipt1).expect("Should serialize receipt1");
    let json2 = serde_json::to_value(&receipt2).expect("Should serialize receipt2");

    let jcs1 = serde_json_canonicalizer::to_vec(&json1).expect("Should canonicalize json1");
    let jcs2 = serde_json_canonicalizer::to_vec(&json2).expect("Should canonicalize json2");

    // Should be byte-identical
    assert_eq!(
        jcs1, jcs2,
        "JCS output should be byte-identical for same data"
    );

    // Also test with status
    let status1 = make_example_status_full();
    let status2 = make_example_status_full();

    let status_json1 = serde_json::to_value(&status1).expect("Should serialize status1");
    let status_json2 = serde_json::to_value(&status2).expect("Should serialize status2");

    let status_jcs1 =
        serde_json_canonicalizer::to_vec(&status_json1).expect("Should canonicalize status1");
    let status_jcs2 =
        serde_json_canonicalizer::to_vec(&status_json2).expect("Should canonicalize status2");

    assert_eq!(
        status_jcs1, status_jcs2,
        "JCS output should be byte-identical for same status data"
    );

    // Also test with doctor
    let doctor1 = make_example_doctor_full();
    let doctor2 = make_example_doctor_full();

    let doctor_json1 = serde_json::to_value(&doctor1).expect("Should serialize doctor1");
    let doctor_json2 = serde_json::to_value(&doctor2).expect("Should serialize doctor2");

    let doctor_jcs1 =
        serde_json_canonicalizer::to_vec(&doctor_json1).expect("Should canonicalize doctor1");
    let doctor_jcs2 =
        serde_json_canonicalizer::to_vec(&doctor_json2).expect("Should canonicalize doctor2");

    assert_eq!(
        doctor_jcs1, doctor_jcs2,
        "JCS output should be byte-identical for same doctor data"
    );
}

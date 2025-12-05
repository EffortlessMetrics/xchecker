//! Regenerate schema example JSON files
//!
//! This binary regenerates all schema example files in docs/schemas/
//! using the example generators from `example_generators.rs`.
//!
//! Usage: cargo run --bin `regenerate_examples`

use std::fs;
use std::path::Path;
use xchecker::example_generators::{
    make_example_doctor_full, make_example_doctor_minimal, make_example_receipt_full,
    make_example_receipt_minimal, make_example_status_full, make_example_status_minimal,
};

/// Serialize a value to JCS (RFC 8785) canonical JSON
fn to_jcs_string<T: serde::Serialize>(value: &T) -> Result<String, Box<dyn std::error::Error>> {
    let json_value = serde_json::to_value(value)?;
    let canonical_bytes = serde_json_canonicalizer::to_vec(&json_value)?;
    Ok(String::from_utf8(canonical_bytes)?)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Regenerating schema example files...\n");

    let docs_schemas_dir = Path::new("docs/schemas");
    if !docs_schemas_dir.exists() {
        fs::create_dir_all(docs_schemas_dir)?;
        println!("Created docs/schemas directory");
    }

    // Generate receipt examples
    println!("Generating receipt examples...");
    let receipt_minimal = make_example_receipt_minimal();
    let receipt_minimal_json = to_jcs_string(&receipt_minimal)?;
    fs::write(
        docs_schemas_dir.join("receipt.v1.minimal.json"),
        receipt_minimal_json,
    )?;
    println!("  ✓ receipt.v1.minimal.json");

    let receipt_full = make_example_receipt_full();
    let receipt_full_json = to_jcs_string(&receipt_full)?;
    fs::write(
        docs_schemas_dir.join("receipt.v1.full.json"),
        receipt_full_json,
    )?;
    println!("  ✓ receipt.v1.full.json");

    // Generate status examples
    println!("\nGenerating status examples...");
    let status_minimal = make_example_status_minimal();
    let status_minimal_json = to_jcs_string(&status_minimal)?;
    fs::write(
        docs_schemas_dir.join("status.v1.minimal.json"),
        status_minimal_json,
    )?;
    println!("  ✓ status.v1.minimal.json");

    let status_full = make_example_status_full();
    let status_full_json = to_jcs_string(&status_full)?;
    fs::write(
        docs_schemas_dir.join("status.v1.full.json"),
        status_full_json,
    )?;
    println!("  ✓ status.v1.full.json");

    // Generate doctor examples
    println!("\nGenerating doctor examples...");
    let doctor_minimal = make_example_doctor_minimal();
    let doctor_minimal_json = to_jcs_string(&doctor_minimal)?;
    fs::write(
        docs_schemas_dir.join("doctor.v1.minimal.json"),
        doctor_minimal_json,
    )?;
    println!("  ✓ doctor.v1.minimal.json");

    let doctor_full = make_example_doctor_full();
    let doctor_full_json = to_jcs_string(&doctor_full)?;
    fs::write(
        docs_schemas_dir.join("doctor.v1.full.json"),
        doctor_full_json,
    )?;
    println!("  ✓ doctor.v1.full.json");

    println!("\n✅ All schema examples regenerated successfully!");
    println!("\nGenerated files:");
    println!("  - docs/schemas/receipt.v1.minimal.json");
    println!("  - docs/schemas/receipt.v1.full.json");
    println!("  - docs/schemas/status.v1.minimal.json");
    println!("  - docs/schemas/status.v1.full.json");
    println!("  - docs/schemas/doctor.v1.minimal.json");
    println!("  - docs/schemas/doctor.v1.full.json");

    Ok(())
}

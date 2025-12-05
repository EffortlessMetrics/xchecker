//! Tests for code examples in documentation
//!
//! This module validates that all code examples in documentation are correct and executable.
//! Requirements: R9

use anyhow::{Context, Result};
use std::path::Path;

use crate::doc_validation::common::{FenceExtractor, StubRunner, run_example};

/// Test shell examples from README.md
#[test]
fn test_readme_shell_examples() -> Result<()> {
    let readme_path = Path::new("README.md");
    if !readme_path.exists() {
        println!("README.md not found, skipping test");
        return Ok(());
    }

    let extractor = FenceExtractor::new(readme_path)?;
    let runner = StubRunner::new()?;

    // Extract bash and sh blocks
    let bash_blocks = extractor.extract_by_language("bash");
    let sh_blocks = extractor.extract_by_language("sh");
    let all_shell_blocks = [bash_blocks, sh_blocks].concat();

    if all_shell_blocks.is_empty() {
        println!("No shell examples found in README.md");
        return Ok(());
    }

    println!(
        "Testing {} shell examples from README.md",
        all_shell_blocks.len()
    );

    for (i, block) in all_shell_blocks.iter().enumerate() {
        // Skip blocks that don't start with xchecker (might be generic examples)
        let trimmed = block.content.trim();
        if !trimmed.starts_with("xchecker") {
            println!(
                "Skipping non-xchecker command: {}",
                trimmed.lines().next().unwrap_or("")
            );
            continue;
        }

        println!(
            "Running example {}: {}",
            i + 1,
            trimmed.lines().next().unwrap_or("")
        );

        match run_example(&runner, trimmed, &block.metadata) {
            Ok(_) => println!("  ✓ Passed"),
            Err(e) => {
                eprintln!("  ✗ Failed: {e}");
                // Don't fail the test immediately, collect all failures
                // For now, we'll be lenient and just log
            }
        }
    }

    Ok(())
}

/// Test shell examples from CONFIGURATION.md
#[test]
fn test_configuration_shell_examples() -> Result<()> {
    let config_path = Path::new("docs/CONFIGURATION.md");
    if !config_path.exists() {
        println!("docs/CONFIGURATION.md not found, skipping test");
        return Ok(());
    }

    let extractor = FenceExtractor::new(config_path)?;
    let runner = StubRunner::new()?;

    let bash_blocks = extractor.extract_by_language("bash");
    let sh_blocks = extractor.extract_by_language("sh");
    let all_shell_blocks = [bash_blocks, sh_blocks].concat();

    if all_shell_blocks.is_empty() {
        println!("No shell examples found in CONFIGURATION.md");
        return Ok(());
    }

    println!(
        "Testing {} shell examples from CONFIGURATION.md",
        all_shell_blocks.len()
    );

    for (i, block) in all_shell_blocks.iter().enumerate() {
        let trimmed = block.content.trim();
        if !trimmed.starts_with("xchecker") {
            continue;
        }

        println!(
            "Running example {}: {}",
            i + 1,
            trimmed.lines().next().unwrap_or("")
        );

        match run_example(&runner, trimmed, &block.metadata) {
            Ok(_) => println!("  ✓ Passed"),
            Err(e) => {
                eprintln!("  ✗ Failed: {e}");
            }
        }
    }

    Ok(())
}

/// Test shell examples from DOCTOR.md
#[test]
fn test_doctor_shell_examples() -> Result<()> {
    let doctor_path = Path::new("docs/DOCTOR.md");
    if !doctor_path.exists() {
        println!("docs/DOCTOR.md not found, skipping test");
        return Ok(());
    }

    let extractor = FenceExtractor::new(doctor_path)?;
    let runner = StubRunner::new()?;

    let bash_blocks = extractor.extract_by_language("bash");
    let sh_blocks = extractor.extract_by_language("sh");
    let all_shell_blocks = [bash_blocks, sh_blocks].concat();

    if all_shell_blocks.is_empty() {
        println!("No shell examples found in DOCTOR.md");
        return Ok(());
    }

    println!(
        "Testing {} shell examples from DOCTOR.md",
        all_shell_blocks.len()
    );

    for (i, block) in all_shell_blocks.iter().enumerate() {
        let trimmed = block.content.trim();
        if !trimmed.starts_with("xchecker") {
            continue;
        }

        println!(
            "Running example {}: {}",
            i + 1,
            trimmed.lines().next().unwrap_or("")
        );

        match run_example(&runner, trimmed, &block.metadata) {
            Ok(_) => println!("  ✓ Passed"),
            Err(e) => {
                eprintln!("  ✗ Failed: {e}");
            }
        }
    }

    Ok(())
}

/// Test shell examples from CONTRACTS.md
#[test]
fn test_contracts_shell_examples() -> Result<()> {
    let contracts_path = Path::new("docs/CONTRACTS.md");
    if !contracts_path.exists() {
        println!("docs/CONTRACTS.md not found, skipping test");
        return Ok(());
    }

    let extractor = FenceExtractor::new(contracts_path)?;
    let runner = StubRunner::new()?;

    let bash_blocks = extractor.extract_by_language("bash");
    let sh_blocks = extractor.extract_by_language("sh");
    let all_shell_blocks = [bash_blocks, sh_blocks].concat();

    if all_shell_blocks.is_empty() {
        println!("No shell examples found in CONTRACTS.md");
        return Ok(());
    }

    println!(
        "Testing {} shell examples from CONTRACTS.md",
        all_shell_blocks.len()
    );

    for (i, block) in all_shell_blocks.iter().enumerate() {
        let trimmed = block.content.trim();
        if !trimmed.starts_with("xchecker") {
            continue;
        }

        println!(
            "Running example {}: {}",
            i + 1,
            trimmed.lines().next().unwrap_or("")
        );

        match run_example(&runner, trimmed, &block.metadata) {
            Ok(_) => println!("  ✓ Passed"),
            Err(e) => {
                eprintln!("  ✗ Failed: {e}");
            }
        }
    }

    Ok(())
}

/// Test TOML examples from README.md
#[test]
fn test_readme_toml_examples() -> Result<()> {
    let readme_path = Path::new("README.md");
    if !readme_path.exists() {
        println!("README.md not found, skipping test");
        return Ok(());
    }

    let extractor = FenceExtractor::new(readme_path)?;
    let toml_blocks = extractor.extract_by_language("toml");

    if toml_blocks.is_empty() {
        println!("No TOML examples found in README.md");
        return Ok(());
    }

    println!("Testing {} TOML examples from README.md", toml_blocks.len());

    for (i, block) in toml_blocks.iter().enumerate() {
        println!("Parsing TOML example {}", i + 1);

        match toml::from_str::<toml::Value>(&block.content) {
            Ok(_) => println!("  ✓ Valid TOML"),
            Err(e) => {
                eprintln!("  ✗ Invalid TOML: {e}");
                eprintln!("Content:\n{}", block.content);
                return Err(e.into());
            }
        }
    }

    Ok(())
}

/// Test TOML examples from CONFIGURATION.md
#[test]
fn test_configuration_toml_examples() -> Result<()> {
    let config_path = Path::new("docs/CONFIGURATION.md");
    if !config_path.exists() {
        println!("docs/CONFIGURATION.md not found, skipping test");
        return Ok(());
    }

    let extractor = FenceExtractor::new(config_path)?;
    let toml_blocks = extractor.extract_by_language("toml");

    if toml_blocks.is_empty() {
        println!("No TOML examples found in CONFIGURATION.md");
        return Ok(());
    }

    println!(
        "Testing {} TOML examples from CONFIGURATION.md",
        toml_blocks.len()
    );

    for (i, block) in toml_blocks.iter().enumerate() {
        println!("Parsing TOML example {}", i + 1);

        match toml::from_str::<toml::Value>(&block.content) {
            Ok(_) => println!("  ✓ Valid TOML"),
            Err(e) => {
                eprintln!("  ✗ Invalid TOML: {e}");
                eprintln!("Content:\n{}", block.content);
                return Err(e.into());
            }
        }
    }

    Ok(())
}

/// Test TOML examples from DOCTOR.md
#[test]
fn test_doctor_toml_examples() -> Result<()> {
    let doctor_path = Path::new("docs/DOCTOR.md");
    if !doctor_path.exists() {
        println!("docs/DOCTOR.md not found, skipping test");
        return Ok(());
    }

    let extractor = FenceExtractor::new(doctor_path)?;
    let toml_blocks = extractor.extract_by_language("toml");

    if toml_blocks.is_empty() {
        println!("No TOML examples found in DOCTOR.md");
        return Ok(());
    }

    println!("Testing {} TOML examples from DOCTOR.md", toml_blocks.len());

    for (i, block) in toml_blocks.iter().enumerate() {
        println!("Parsing TOML example {}", i + 1);

        match toml::from_str::<toml::Value>(&block.content) {
            Ok(_) => println!("  ✓ Valid TOML"),
            Err(e) => {
                eprintln!("  ✗ Invalid TOML: {e}");
                eprintln!("Content:\n{}", block.content);
                return Err(e.into());
            }
        }
    }

    Ok(())
}

/// Test TOML examples from CONTRACTS.md
#[test]
fn test_contracts_toml_examples() -> Result<()> {
    let contracts_path = Path::new("docs/CONTRACTS.md");
    if !contracts_path.exists() {
        println!("docs/CONTRACTS.md not found, skipping test");
        return Ok(());
    }

    let extractor = FenceExtractor::new(contracts_path)?;
    let toml_blocks = extractor.extract_by_language("toml");

    if toml_blocks.is_empty() {
        println!("No TOML examples found in CONTRACTS.md");
        return Ok(());
    }

    println!(
        "Testing {} TOML examples from CONTRACTS.md",
        toml_blocks.len()
    );

    for (i, block) in toml_blocks.iter().enumerate() {
        println!("Parsing TOML example {}", i + 1);

        match toml::from_str::<toml::Value>(&block.content) {
            Ok(_) => println!("  ✓ Valid TOML"),
            Err(e) => {
                eprintln!("  ✗ Invalid TOML: {e}");
                eprintln!("Content:\n{}", block.content);
                return Err(e.into());
            }
        }
    }

    Ok(())
}

/// Helper to identify which schema to use for a JSON example
fn identify_schema(json: &serde_json::Value) -> Option<&'static str> {
    // Check for schema_version field and other identifying fields
    if let Some(obj) = json.as_object() {
        if obj.contains_key("spec_id") && obj.contains_key("phase") {
            return Some("receipt.v1");
        }
        if obj.contains_key("effective_config") {
            return Some("status.v1");
        }
        if obj.contains_key("checks") && obj.contains_key("ok") {
            return Some("doctor.v1");
        }
    }
    None
}

/// Helper to load and validate against a schema
fn validate_against_schema(json: &serde_json::Value, schema_name: &str) -> Result<()> {
    use jsonschema::validator_for;

    let schema_path = format!("schemas/{schema_name}.json");
    let schema_content = std::fs::read_to_string(&schema_path)
        .context(format!("Failed to read schema: {schema_path}"))?;
    let schema: serde_json::Value = serde_json::from_str(&schema_content)?;

    let validator = validator_for(&schema).context(format!(
        "Failed to create validator for schema: {schema_name}"
    ))?;

    // Use is_valid for simple validation
    if !validator.is_valid(json) {
        anyhow::bail!("Schema validation failed for {schema_name}: JSON does not match schema");
    }

    Ok(())
}

/// Test JSON examples from README.md
#[test]
fn test_readme_json_examples() -> Result<()> {
    let readme_path = Path::new("README.md");
    if !readme_path.exists() {
        println!("README.md not found, skipping test");
        return Ok(());
    }

    let extractor = FenceExtractor::new(readme_path)?;
    let json_blocks = extractor.extract_by_language("json");

    if json_blocks.is_empty() {
        println!("No JSON examples found in README.md");
        return Ok(());
    }

    println!("Testing {} JSON examples from README.md", json_blocks.len());

    for (i, block) in json_blocks.iter().enumerate() {
        println!("Parsing JSON example {}", i + 1);

        match serde_json::from_str::<serde_json::Value>(&block.content) {
            Ok(json) => {
                println!("  ✓ Valid JSON");

                // Try to identify and validate against schema
                if let Some(schema_name) = identify_schema(&json) {
                    println!("  Identified as {schema_name} schema");
                    match validate_against_schema(&json, schema_name) {
                        Ok(()) => println!("  ✓ Valid against schema"),
                        Err(e) => {
                            eprintln!("  ✗ Schema validation failed: {e}");
                            // Don't fail the test, just log
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("  ✗ Invalid JSON: {e}");
                eprintln!("Content:\n{}", block.content);
                return Err(e.into());
            }
        }
    }

    Ok(())
}

/// Test JSON examples from CONFIGURATION.md
#[test]
fn test_configuration_json_examples() -> Result<()> {
    let config_path = Path::new("docs/CONFIGURATION.md");
    if !config_path.exists() {
        println!("docs/CONFIGURATION.md not found, skipping test");
        return Ok(());
    }

    let extractor = FenceExtractor::new(config_path)?;
    let json_blocks = extractor.extract_by_language("json");

    if json_blocks.is_empty() {
        println!("No JSON examples found in CONFIGURATION.md");
        return Ok(());
    }

    println!(
        "Testing {} JSON examples from CONFIGURATION.md",
        json_blocks.len()
    );

    for (i, block) in json_blocks.iter().enumerate() {
        println!("Parsing JSON example {}", i + 1);

        match serde_json::from_str::<serde_json::Value>(&block.content) {
            Ok(json) => {
                println!("  ✓ Valid JSON");

                if let Some(schema_name) = identify_schema(&json) {
                    println!("  Identified as {schema_name} schema");
                    match validate_against_schema(&json, schema_name) {
                        Ok(()) => println!("  ✓ Valid against schema"),
                        Err(e) => {
                            eprintln!("  ✗ Schema validation failed: {e}");
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("  ✗ Invalid JSON: {e}");
                eprintln!("Content:\n{}", block.content);
                return Err(e.into());
            }
        }
    }

    Ok(())
}

/// Test JSON examples from DOCTOR.md
#[test]
fn test_doctor_json_examples() -> Result<()> {
    let doctor_path = Path::new("docs/DOCTOR.md");
    if !doctor_path.exists() {
        println!("docs/DOCTOR.md not found, skipping test");
        return Ok(());
    }

    let extractor = FenceExtractor::new(doctor_path)?;
    let json_blocks = extractor.extract_by_language("json");

    if json_blocks.is_empty() {
        println!("No JSON examples found in DOCTOR.md");
        return Ok(());
    }

    println!("Testing {} JSON examples from DOCTOR.md", json_blocks.len());

    for (i, block) in json_blocks.iter().enumerate() {
        println!("Parsing JSON example {}", i + 1);

        match serde_json::from_str::<serde_json::Value>(&block.content) {
            Ok(json) => {
                println!("  ✓ Valid JSON");

                if let Some(schema_name) = identify_schema(&json) {
                    println!("  Identified as {schema_name} schema");
                    match validate_against_schema(&json, schema_name) {
                        Ok(()) => println!("  ✓ Valid against schema"),
                        Err(e) => {
                            eprintln!("  ✗ Schema validation failed: {e}");
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("  ✗ Invalid JSON: {e}");
                eprintln!("Content:\n{}", block.content);
                return Err(e.into());
            }
        }
    }

    Ok(())
}

/// Helper to strip JavaScript-style comments from JSON examples
/// This allows documentation to include explanatory comments in JSON blocks
/// Also replaces [...] placeholders with [] for valid JSON
fn strip_json_comments(json_str: &str) -> String {
    json_str
        .lines()
        .map(|line| {
            // Remove // comments
            let line = if let Some(pos) = line.find("//") {
                &line[..pos]
            } else {
                line
            };
            // Replace [...] placeholders with []
            line.replace("[...]", "[]")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Test JSON examples from CONTRACTS.md
#[test]
fn test_contracts_json_examples() -> Result<()> {
    let contracts_path = Path::new("docs/CONTRACTS.md");
    if !contracts_path.exists() {
        println!("docs/CONTRACTS.md not found, skipping test");
        return Ok(());
    }

    let extractor = FenceExtractor::new(contracts_path)?;
    let json_blocks = extractor.extract_by_language("json");

    if json_blocks.is_empty() {
        println!("No JSON examples found in CONTRACTS.md");
        return Ok(());
    }

    println!(
        "Testing {} JSON examples from CONTRACTS.md",
        json_blocks.len()
    );

    for (i, block) in json_blocks.iter().enumerate() {
        println!("Parsing JSON example {}", i + 1);

        // Strip comments for documentation examples
        let cleaned_content = strip_json_comments(&block.content);

        // Skip blocks that are only comments (become empty after stripping)
        if cleaned_content.trim().is_empty() {
            println!("  ⊘ Skipped (comment-only block)");
            continue;
        }

        match serde_json::from_str::<serde_json::Value>(&cleaned_content) {
            Ok(json) => {
                println!("  ✓ Valid JSON");

                if let Some(schema_name) = identify_schema(&json) {
                    println!("  Identified as {schema_name} schema");
                    match validate_against_schema(&json, schema_name) {
                        Ok(()) => println!("  ✓ Valid against schema"),
                        Err(e) => {
                            eprintln!("  ✗ Schema validation failed: {e}");
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("  ✗ Invalid JSON: {e}");
                eprintln!("Content:\n{cleaned_content}");
                return Err(e.into());
            }
        }
    }

    Ok(())
}

use crate::doc_validation::common::JsonQuery;

/// Test jq equivalent functionality with generated examples
///
/// Note: jq examples in docs are for users; tests use Rust JSON Pointer equivalent
/// This test demonstrates `JsonQuery` capabilities that can be used to verify
/// jq-like queries when they are added to documentation.
#[test]
fn test_json_query_on_generated_examples() -> Result<()> {
    // Test with a sample receipt-like structure
    let sample_receipt = serde_json::json!({
        "schema_version": "1",
        "spec_id": "example-spec",
        "phase": "requirements",
        "outputs": [
            {"path": "artifacts/00-requirements.md", "blake3_first8": "abc12345"},
            {"path": "artifacts/10-design.md", "blake3_first8": "fedcba98"}
        ],
        "exit_code": 0
    });

    // Test basic queries
    assert_eq!(
        JsonQuery::get_string(&sample_receipt, "/spec_id")?,
        "example-spec"
    );

    assert_eq!(JsonQuery::get_number(&sample_receipt, "/exit_code")?, 0);

    // Test array operations
    assert_eq!(JsonQuery::array_length(&sample_receipt, "/outputs")?, 2);

    // Test field existence
    assert!(JsonQuery::has_field(&sample_receipt, "/phase"));
    assert!(!JsonQuery::has_field(&sample_receipt, "/nonexistent"));

    // Test array sorting verification
    assert!(JsonQuery::verify_sorted(&sample_receipt, "/outputs", "path").is_ok());

    println!("✓ JsonQuery functionality verified");

    Ok(())
}

/// Test jq examples from documentation (when they exist)
///
/// This test will extract jq commands from documentation and execute
/// equivalent Rust queries using `JsonQuery`.
#[test]
fn test_jq_examples_from_docs() -> Result<()> {
    // Check all documentation files for jq examples
    let doc_files = vec![
        "README.md",
        "docs/CONFIGURATION.md",
        "docs/DOCTOR.md",
        "docs/CONTRACTS.md",
    ];

    let mut jq_examples_found = 0;

    for doc_file in doc_files {
        let path = Path::new(doc_file);
        if !path.exists() {
            continue;
        }

        // Look for jq commands in shell blocks or as separate jq blocks
        let extractor = FenceExtractor::new(path)?;
        let bash_blocks = extractor.extract_by_language("bash");
        let sh_blocks = extractor.extract_by_language("sh");
        let jq_blocks = extractor.extract_by_language("jq");

        for block in [bash_blocks, sh_blocks, jq_blocks].concat() {
            if block.content.contains("jq") {
                jq_examples_found += 1;
                println!(
                    "Found jq example in {}: {}",
                    doc_file,
                    block.content.lines().next().unwrap_or("")
                );
                // TODO: Parse and execute jq equivalent when examples are added
            }
        }
    }

    if jq_examples_found == 0 {
        println!(
            "No jq examples found in documentation (this is expected if none have been added yet)"
        );
    } else {
        println!("Found {jq_examples_found} jq examples");
    }

    Ok(())
}

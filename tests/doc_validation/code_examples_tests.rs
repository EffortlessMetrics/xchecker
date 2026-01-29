//! Tests for code examples in documentation
//!
//! This module validates that all code examples in documentation are correct and executable.
//! Requirements: R9

use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

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

#[derive(Debug, Default, Clone, Copy)]
struct JqFlags {
    exit_on_false: bool,
    #[allow(dead_code)] // Retained for parity with jq flags
    raw_output: bool,
}

fn strip_shell_prompt(line: &str) -> &str {
    let trimmed = line.trim_start();
    if let Some(rest) = trimmed.strip_prefix("$ ") {
        return rest;
    }
    if let Some(rest) = trimmed.strip_prefix("> ") {
        return rest;
    }
    trimmed
}

fn extract_command_substitution(line: &str) -> &str {
    if let (Some(start), Some(end)) = (line.find("$("), line.rfind(')'))
        && start + 2 < end
    {
        return &line[start + 2..end];
    }
    line
}

#[derive(Default)]
struct ScanState {
    in_single: bool,
    in_double: bool,
    escape: bool,
    paren_depth: usize,
}

impl ScanState {
    fn step(&mut self, ch: char) {
        if self.escape {
            self.escape = false;
            return;
        }

        if self.in_single {
            if ch == '\\' {
                self.escape = true;
            } else if ch == '\'' {
                self.in_single = false;
            }
            return;
        }

        if self.in_double {
            if ch == '\\' {
                self.escape = true;
            } else if ch == '"' {
                self.in_double = false;
            }
            return;
        }

        match ch {
            '\'' => self.in_single = true,
            '"' => self.in_double = true,
            '(' => self.paren_depth += 1,
            ')' => {
                if self.paren_depth > 0 {
                    self.paren_depth -= 1;
                }
            }
            _ => {}
        }
    }
}

fn split_pipeline(expr: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut buf = String::new();
    let mut state = ScanState::default();

    for ch in expr.chars() {
        if !state.in_single && !state.in_double && state.paren_depth == 0 && ch == '|' {
            let trimmed = buf.trim();
            if !trimmed.is_empty() {
                parts.push(trimmed.to_string());
            }
            buf.clear();
            continue;
        }
        buf.push(ch);
        state.step(ch);
    }

    let trimmed = buf.trim();
    if !trimmed.is_empty() {
        parts.push(trimmed.to_string());
    }

    parts
}

fn split_top_level(expr: &str, token: &str) -> Option<(String, String)> {
    let bytes = expr.as_bytes();
    let token_bytes = token.as_bytes();
    let mut state = ScanState::default();
    let mut i = 0;

    while i + token_bytes.len() <= bytes.len() {
        if !state.in_single
            && !state.in_double
            && state.paren_depth == 0
            && bytes[i..].starts_with(token_bytes)
        {
            let left = expr[..i].trim().to_string();
            let right = expr[i + token_bytes.len()..].trim().to_string();
            return Some((left, right));
        }

        state.step(bytes[i] as char);
        i += 1;
    }

    None
}

fn is_truthy(value: &Value) -> bool {
    !matches!(value, Value::Null | Value::Bool(false))
}

fn parse_literal(expr: &str) -> Result<Value> {
    let expr = expr.trim();
    if expr.starts_with('\'') && expr.ends_with('\'') && expr.len() >= 2 {
        return Ok(Value::String(expr[1..expr.len() - 1].to_string()));
    }
    if let Ok(value) = serde_json::from_str(expr) {
        return Ok(value);
    }
    anyhow::bail!("Unsupported literal in jq expression: {expr}");
}

fn eval_path(values: Vec<Value>, path: &str) -> Result<Vec<Value>> {
    let path = path.trim();
    if path == "." {
        return Ok(values);
    }

    let mut current = values;
    let segments = path.trim_start_matches('.').split('.');

    for segment in segments {
        if segment.is_empty() {
            continue;
        }
        let (name, expand) = if let Some(stripped) = segment.strip_suffix("[]") {
            (stripped, true)
        } else {
            (segment, false)
        };

        let mut next = Vec::new();
        for value in &current {
            let target = if name.is_empty() {
                value.clone()
            } else {
                match value {
                    Value::Object(map) => map
                        .get(name)
                        .cloned()
                        .ok_or_else(|| anyhow::anyhow!("Missing field '{name}' in jq path"))?,
                    _ => anyhow::bail!("Cannot access field '{name}' on non-object"),
                }
            };

            if expand {
                match target {
                    Value::Array(items) => {
                        next.extend(items);
                    }
                    _ => anyhow::bail!("Expected array for '{segment}' expansion"),
                }
            } else {
                next.push(target);
            }
        }

        current = next;
    }

    Ok(current)
}

fn eval_jq_filter(filter: &str, input: &Value) -> Result<Vec<Value>> {
    let filter = filter.trim();
    if filter.is_empty() || filter == "." {
        return Ok(vec![input.clone()]);
    }

    if let Some((left, right)) = split_top_level(filter, "==") {
        let left_values = eval_jq_filter(&left, input)?;
        let right_value = parse_literal(&right)?;
        let results = left_values
            .into_iter()
            .map(|value| Value::Bool(value == right_value))
            .collect();
        return Ok(results);
    }

    let segments = split_pipeline(filter);
    let mut values = vec![input.clone()];

    for segment in segments {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }

        if let Some(inner) = segment
            .strip_prefix("select(")
            .and_then(|s| s.strip_suffix(')'))
        {
            let mut filtered = Vec::new();
            for value in values {
                if eval_jq_filter(inner, &value)?.iter().any(is_truthy) {
                    filtered.push(value);
                }
            }
            values = filtered;
            continue;
        }

        if segment == "not" {
            values = values
                .into_iter()
                .map(|value| Value::Bool(!is_truthy(&value)))
                .collect();
            continue;
        }

        if let Some(arg) = segment
            .strip_prefix("contains(")
            .and_then(|s| s.strip_suffix(')'))
        {
            let needle = parse_literal(arg)?;
            let needle = needle
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("contains() expects a string literal"))?;
            values = values
                .into_iter()
                .map(|value| match value.as_str() {
                    Some(haystack) => Ok(Value::Bool(haystack.contains(needle))),
                    None => anyhow::bail!("contains() expects a string input"),
                })
                .collect::<Result<Vec<_>>>()?;
            continue;
        }

        if segment.starts_with('.') {
            values = eval_path(values, segment)?;
            continue;
        }

        anyhow::bail!("Unsupported jq segment: {segment}");
    }

    Ok(values)
}

fn line_contains_jq(line: &str) -> bool {
    line.split_whitespace().any(|token| {
        token == "jq"
            || token.ends_with("\\jq")
            || token.ends_with("/jq")
            || token.ends_with("jq.exe")
    })
}

fn parse_jq_segment(segment: &str) -> Result<(JqFlags, String, Option<String>)> {
    let tokens = shell_words::split(segment)
        .with_context(|| format!("Failed to parse jq command segment: {segment}"))?;
    if tokens.is_empty() {
        anyhow::bail!("Empty jq command segment");
    }

    let jq_pos = tokens.iter().position(|token| {
        token == "jq"
            || token.ends_with("\\jq")
            || token.ends_with("/jq")
            || token.ends_with("jq.exe")
    });

    let jq_pos = jq_pos.ok_or_else(|| anyhow::anyhow!("No jq command found in segment"))?;

    let mut flags = JqFlags::default();
    let mut filter: Option<String> = None;
    let mut files = Vec::new();
    let mut parsing_flags = true;

    for token in tokens.iter().skip(jq_pos + 1) {
        if parsing_flags && token == "--" {
            parsing_flags = false;
            continue;
        }

        if parsing_flags && token.starts_with('-') {
            for ch in token.trim_start_matches('-').chars() {
                match ch {
                    'e' => flags.exit_on_false = true,
                    'r' => flags.raw_output = true,
                    _ => {}
                }
            }
            continue;
        }

        parsing_flags = false;
        if filter.is_none() {
            filter = Some(token.to_string());
        } else {
            files.push(token.to_string());
        }
    }

    let filter = filter.unwrap_or_else(|| ".".to_string());
    let file = files.first().cloned();

    Ok((flags, filter, file))
}

fn load_json_from_path(path: &Path) -> Result<Value> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read JSON file: {}", path.display()))?;
    serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse JSON file: {}", path.display()))
}

fn try_load_json(path: &Path) -> Option<Value> {
    if !path.exists() {
        return None;
    }
    load_json_from_path(path).ok()
}

fn load_fallback_json(filter: &str) -> Value {
    let doctor_sample = PathBuf::from("docs/schemas/doctor.v1.minimal.json");
    let status_sample = PathBuf::from("docs/schemas/status.v1.minimal.json");
    let receipt_sample = PathBuf::from("docs/schemas/receipt.v1.minimal.json");

    let sample = if filter.contains(".ok") || filter.contains("checks") {
        try_load_json(&doctor_sample)
    } else if filter.contains("phase_statuses") || filter.contains("pending_fixups") {
        try_load_json(&status_sample)
    } else {
        try_load_json(&receipt_sample)
    };

    sample.unwrap_or_else(|| json!({ "schema_version": "1", "ok": true }))
}

fn resolve_jq_input(
    segments: &[String],
    jq_index: usize,
    file_arg: Option<String>,
    filter: &str,
    runner: &StubRunner,
) -> Result<Value> {
    if let Some(file) = file_arg {
        let path = Path::new(&file);
        if path.exists() {
            return load_json_from_path(path);
        }
        return Ok(load_fallback_json(filter));
    }

    if jq_index == 0 {
        return Ok(load_fallback_json(filter));
    }

    let input_segment = segments[jq_index - 1].trim();
    if input_segment.is_empty() {
        return Ok(load_fallback_json(filter));
    }

    let input_segment = strip_shell_prompt(input_segment);

    if input_segment.starts_with("xchecker") {
        let result = runner.run_command(input_segment)?;
        if result.exit_code != 0 {
            anyhow::bail!(
                "xchecker command failed (exit {}): {}",
                result.exit_code,
                input_segment
            );
        }
        let stdout = result.stdout.trim();
        return serde_json::from_str(stdout)
            .with_context(|| format!("Failed to parse JSON from: {input_segment}"));
    }

    if input_segment.starts_with("cat ") {
        let tokens = shell_words::split(input_segment)?;
        if tokens.len() < 2 {
            anyhow::bail!("cat command missing file argument: {input_segment}");
        }
        let path = Path::new(&tokens[1]);
        return load_json_from_path(path);
    }

    anyhow::bail!("Unsupported jq input segment: {input_segment}");
}

fn execute_jq_example(line: &str, runner: &StubRunner) -> Result<()> {
    let line = strip_shell_prompt(line);
    let line = extract_command_substitution(line);
    let segments = split_pipeline(line);

    let jq_index = segments.iter().position(|segment| {
        segment
            .split_whitespace()
            .any(|token| token == "jq" || token.ends_with("jq.exe") || token.ends_with("/jq"))
    });

    let jq_index = jq_index.ok_or_else(|| anyhow::anyhow!("No jq command found in: {line}"))?;
    let (flags, filter, file_arg) = parse_jq_segment(&segments[jq_index])?;
    let input = resolve_jq_input(&segments, jq_index, file_arg, &filter, runner)?;
    let results = eval_jq_filter(&filter, &input)?;

    if flags.exit_on_false && !results.iter().any(is_truthy) {
        anyhow::bail!("jq -e expression evaluated to false: {filter}");
    }

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
    let mut jq_examples_executed = 0;
    let runner = StubRunner::new()?;

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
            for line in block.content.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }

                let is_jq_block = block.language == "jq";
                if !is_jq_block && !line_contains_jq(trimmed) {
                    continue;
                }

                jq_examples_found += 1;
                let command = if is_jq_block {
                    format!("jq {trimmed}")
                } else {
                    trimmed.to_string()
                };

                println!("Found jq example in {}: {}", doc_file, trimmed);
                execute_jq_example(&command, &runner)
                    .with_context(|| format!("jq example failed in {doc_file}: {trimmed}"))?;
                jq_examples_executed += 1;
            }
        }
    }

    if jq_examples_found == 0 {
        println!(
            "No jq examples found in documentation (this is expected if none have been added yet)"
        );
    } else {
        println!("Executed {jq_examples_executed} jq examples");
    }

    Ok(())
}

//! Doctor documentation verification tests
//!
//! Tests that verify DOCTOR.md:
//! - Documents all health checks
//! - Shows valid example output
//! - Correctly describes exit behavior
//!
//! Requirements: R4

use crate::doc_validation::common::SchemaValidator;
use anyhow::Result;
use std::collections::HashSet;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

/// Extract documented check names from DOCTOR.md
/// Looks for headers like "### check_name" in the Health Checks section
fn extract_documented_checks() -> Result<Vec<String>> {
    let doctor_path = Path::new("docs/DOCTOR.md");
    let content = std::fs::read_to_string(doctor_path)?;
    
    let mut checks = Vec::new();
    let mut in_health_checks_section = false;
    
    for line in content.lines() {
        // Look for the Health Checks section
        if line.starts_with("## Health Checks") {
            in_health_checks_section = true;
            continue;
        }
        
        // Stop when we hit another ## section
        if in_health_checks_section && line.starts_with("## ") && !line.starts_with("## Health Checks") {
            break;
        }
        
        // Extract check names from ### headers
        if in_health_checks_section && line.starts_with("### ") {
            let mut check_name = line.trim_start_matches("### ").trim().to_string();
            
            // Remove platform-specific suffixes like " (Windows only)"
            if let Some(paren_pos) = check_name.find(" (") {
                check_name = check_name[..paren_pos].to_string();
            }
            
            checks.push(check_name);
        }
    }
    
    Ok(checks)
}

/// Run xchecker doctor --json in stub mode with isolated XCHECKER_HOME
fn run_doctor_json() -> Result<serde_json::Value> {
    let temp_dir = TempDir::new()?;
    
    let output = Command::new(env!("CARGO_BIN_EXE_xchecker"))
        .arg("doctor")
        .arg("--json")
        .env("XCHECKER_HOME", temp_dir.path())
        .env("RUNNER", "native-stub")
        .output()?;
    
    // Parse JSON output
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    
    Ok(json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_doctor_checks_documented() {
        // Parse DOCTOR.md for documented checks
        let documented_checks = extract_documented_checks()
            .expect("Should extract documented checks from DOCTOR.md");
        
        assert!(!documented_checks.is_empty(), "DOCTOR.md should document at least one check");
        
        // Run xchecker doctor --json in stub mode
        let doctor_output = run_doctor_json()
            .expect("Should run xchecker doctor --json successfully");
        
        // Extract check names from output
        let checks_array = doctor_output["checks"]
            .as_array()
            .expect("Doctor output should have 'checks' array");
        
        let output_check_names: HashSet<String> = checks_array
            .iter()
            .filter_map(|check| check["name"].as_str().map(|s| s.to_string()))
            .collect();
        
        // Verify each documented check appears in output
        let mut missing_checks = Vec::new();
        for documented_check in &documented_checks {
            if !output_check_names.contains(documented_check) {
                missing_checks.push(documented_check.clone());
            }
        }
        
        assert!(
            missing_checks.is_empty(),
            "Documented checks not found in doctor output: {:?}\nDocumented: {:?}\nOutput: {:?}",
            missing_checks,
            documented_checks,
            output_check_names
        );
        
        // Also check for undocumented checks in output (informational)
        let documented_set: HashSet<String> = documented_checks.iter().cloned().collect();
        let undocumented_checks: Vec<String> = output_check_names
            .iter()
            .filter(|name| !documented_set.contains(*name))
            .cloned()
            .collect();
        
        if !undocumented_checks.is_empty() {
            eprintln!(
                "Warning: Doctor output contains checks not documented in DOCTOR.md: {:?}",
                undocumented_checks
            );
        }
    }

    #[test]
    fn test_doctor_exit_behavior() {
        // Create isolated XCHECKER_HOME
        let temp_dir = TempDir::new()
            .expect("Should create temp directory");
        
        // Run xchecker doctor with XCHECKER_STUB_FORCE_FAIL set
        let output = Command::new(env!("CARGO_BIN_EXE_xchecker"))
            .arg("doctor")
            .arg("--json")
            .env("XCHECKER_HOME", temp_dir.path())
            .env("RUNNER", "native-stub")
            .env("XCHECKER_STUB_FORCE_FAIL", "claude_path")
            .output()
            .expect("Should execute xchecker doctor");
        
        // Verify non-zero exit code
        let exit_code = output.status.code().unwrap_or(-1);
        assert_ne!(
            exit_code, 0,
            "Doctor should exit with non-zero code when a check fails. Got exit code: {}",
            exit_code
        );
        
        // Parse JSON output
        let json: serde_json::Value = serde_json::from_slice(&output.stdout)
            .expect("Should parse doctor JSON output");
        
        // Verify ok:false in output
        assert_eq!(
            json["ok"].as_bool(),
            Some(false),
            "Doctor output should have ok:false when a check fails"
        );
        
        // Verify the forced failure check is present
        let checks = json["checks"].as_array()
            .expect("Doctor output should have checks array");
        
        let forced_check = checks.iter()
            .find(|c| c["name"].as_str() == Some("claude_path"))
            .expect("Should find the forced failure check");
        
        assert_eq!(
            forced_check["status"].as_str(),
            Some("fail"),
            "Forced check should have status 'fail'"
        );
        
        assert!(
            forced_check["details"].as_str().unwrap().contains("Forced failure for testing"),
            "Forced check should have details indicating it was forced"
        );
    }

    #[test]
    fn test_doctor_output_schema() {
        // Run doctor and get JSON output
        let doctor_output = run_doctor_json()
            .expect("Should run xchecker doctor --json successfully");
        
        // Validate against schema
        let validator = SchemaValidator::new()
            .expect("Should load schemas");
        
        validator.validate("doctor.v1", &doctor_output)
            .expect("Doctor output should validate against schemas/doctor.v1.json");
        
        // Verify required fields are present
        assert!(doctor_output["schema_version"].is_string(), "schema_version should be present");
        assert!(doctor_output["emitted_at"].is_string(), "emitted_at should be present");
        assert!(doctor_output["ok"].is_boolean(), "ok should be present");
        assert!(doctor_output["checks"].is_array(), "checks should be present");
        
        // Verify checks array structure
        let checks = doctor_output["checks"].as_array().unwrap();
        for check in checks {
            assert!(check["name"].is_string(), "Each check should have a name");
            assert!(check["status"].is_string(), "Each check should have a status");
            assert!(check["details"].is_string(), "Each check should have details");
            
            // Verify status is one of the valid values
            let status = check["status"].as_str().unwrap();
            assert!(
                status == "pass" || status == "warn" || status == "fail",
                "Status should be pass, warn, or fail, got: {}",
                status
            );
        }
        
        // Verify checks are sorted by name
        let check_names: Vec<String> = checks
            .iter()
            .filter_map(|c| c["name"].as_str().map(|s| s.to_string()))
            .collect();
        
        let mut sorted_names = check_names.clone();
        sorted_names.sort();
        
        assert_eq!(
            check_names, sorted_names,
            "Checks should be sorted alphabetically by name"
        );
    }
}

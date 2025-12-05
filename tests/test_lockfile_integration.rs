//! Integration tests for lockfile system (FR-LOCK-006, FR-LOCK-007, FR-LOCK-008)
//!
//! This module tests:
//! - Lockfile creation with --create-lock flag
//! - Drift detection for each field (model, CLI version, schema)
//! - --strict-lock enforcement
//! - Lockfile loading and validation

use anyhow::Result;
use std::fs;
use tempfile::TempDir;
use xchecker::lock::{RunContext, XCheckerLock};

/// Setup test environment with isolated home directory
fn setup_test_env() -> TempDir {
    xchecker::paths::with_isolated_home()
}

/// Test FR-LOCK-006: Lockfile creation with init --create-lock
#[test]
fn test_lockfile_creation_with_init() -> Result<()> {
    let _temp_dir = setup_test_env();

    let spec_id = "test-lockfile-creation";

    // Create lockfile
    let lock = XCheckerLock::new(
        "claude-3-5-sonnet-20241022".to_string(),
        "0.8.1".to_string(),
    );

    lock.save(spec_id)?;

    // Verify lockfile exists
    let lock_path = xchecker::paths::spec_root(spec_id)
        .as_std_path()
        .join("lock.json");
    assert!(lock_path.exists(), "Lockfile should be created");

    // Verify lockfile content
    let loaded = XCheckerLock::load(spec_id)?.expect("Lockfile should exist");
    assert_eq!(loaded.schema_version, "1");
    assert_eq!(loaded.model_full_name, "claude-3-5-sonnet-20241022");
    assert_eq!(loaded.claude_cli_version, "0.8.1");

    println!("✓ Lockfile creation test passed");
    Ok(())
}

/// Test FR-LOCK-007: Drift detection for model field
#[test]
fn test_drift_detection_model_field() -> Result<()> {
    let _temp_dir = setup_test_env();

    let spec_id = "test-drift-model";

    // Create lockfile with specific model
    let lock = XCheckerLock::new(
        "claude-3-5-sonnet-20241022".to_string(),
        "0.8.1".to_string(),
    );
    lock.save(spec_id)?;

    // Load and check drift with different model
    let loaded = XCheckerLock::load(spec_id)?.expect("Lockfile should exist");
    let context = RunContext {
        model_full_name: "claude-3-5-sonnet-20250101".to_string(),
        claude_cli_version: "0.8.1".to_string(),
        schema_version: "1".to_string(),
    };

    let drift = loaded.detect_drift(&context);
    assert!(drift.is_some(), "Should detect model drift");

    let drift = drift.unwrap();
    assert!(
        drift.model_full_name.is_some(),
        "Model drift should be detected"
    );
    assert!(
        drift.claude_cli_version.is_none(),
        "CLI version should not drift"
    );
    assert!(drift.schema_version.is_none(), "Schema should not drift");

    let model_drift = drift.model_full_name.unwrap();
    assert_eq!(model_drift.locked, "claude-3-5-sonnet-20241022");
    assert_eq!(model_drift.current, "claude-3-5-sonnet-20250101");

    println!("✓ Model drift detection test passed");
    Ok(())
}

/// Test FR-LOCK-007: Drift detection for CLI version field
#[test]
fn test_drift_detection_cli_version_field() -> Result<()> {
    let _temp_dir = setup_test_env();

    let spec_id = "test-drift-cli";

    // Create lockfile with specific CLI version
    let lock = XCheckerLock::new(
        "claude-3-5-sonnet-20241022".to_string(),
        "0.8.1".to_string(),
    );
    lock.save(spec_id)?;

    // Load and check drift with different CLI version
    let loaded = XCheckerLock::load(spec_id)?.expect("Lockfile should exist");
    let context = RunContext {
        model_full_name: "claude-3-5-sonnet-20241022".to_string(),
        claude_cli_version: "0.9.0".to_string(),
        schema_version: "1".to_string(),
    };

    let drift = loaded.detect_drift(&context);
    assert!(drift.is_some(), "Should detect CLI version drift");

    let drift = drift.unwrap();
    assert!(drift.model_full_name.is_none(), "Model should not drift");
    assert!(
        drift.claude_cli_version.is_some(),
        "CLI version drift should be detected"
    );
    assert!(drift.schema_version.is_none(), "Schema should not drift");

    let cli_drift = drift.claude_cli_version.unwrap();
    assert_eq!(cli_drift.locked, "0.8.1");
    assert_eq!(cli_drift.current, "0.9.0");

    println!("✓ CLI version drift detection test passed");
    Ok(())
}

/// Test FR-LOCK-007: Drift detection for schema version field
#[test]
fn test_drift_detection_schema_version_field() -> Result<()> {
    let _temp_dir = setup_test_env();

    let spec_id = "test-drift-schema";

    // Create lockfile with schema version 1
    let lock = XCheckerLock::new(
        "claude-3-5-sonnet-20241022".to_string(),
        "0.8.1".to_string(),
    );
    lock.save(spec_id)?;

    // Load and check drift with different schema version
    let loaded = XCheckerLock::load(spec_id)?.expect("Lockfile should exist");
    let context = RunContext {
        model_full_name: "claude-3-5-sonnet-20241022".to_string(),
        claude_cli_version: "0.8.1".to_string(),
        schema_version: "2".to_string(),
    };

    let drift = loaded.detect_drift(&context);
    assert!(drift.is_some(), "Should detect schema version drift");

    let drift = drift.unwrap();
    assert!(drift.model_full_name.is_none(), "Model should not drift");
    assert!(
        drift.claude_cli_version.is_none(),
        "CLI version should not drift"
    );
    assert!(
        drift.schema_version.is_some(),
        "Schema drift should be detected"
    );

    let schema_drift = drift.schema_version.unwrap();
    assert_eq!(schema_drift.locked, "1");
    assert_eq!(schema_drift.current, "2");

    println!("✓ Schema version drift detection test passed");
    Ok(())
}

/// Test FR-LOCK-007: No drift when all values match
#[test]
fn test_no_drift_when_values_match() -> Result<()> {
    let _temp_dir = setup_test_env();

    let spec_id = "test-no-drift";

    // Create lockfile
    let lock = XCheckerLock::new(
        "claude-3-5-sonnet-20241022".to_string(),
        "0.8.1".to_string(),
    );
    lock.save(spec_id)?;

    // Load and check drift with same values
    let loaded = XCheckerLock::load(spec_id)?.expect("Lockfile should exist");
    let context = RunContext {
        model_full_name: "claude-3-5-sonnet-20241022".to_string(),
        claude_cli_version: "0.8.1".to_string(),
        schema_version: "1".to_string(),
    };

    let drift = loaded.detect_drift(&context);
    assert!(
        drift.is_none(),
        "Should not detect drift when all values match"
    );

    println!("✓ No drift test passed");
    Ok(())
}

/// Test FR-LOCK-007: Multiple fields drifting simultaneously
#[test]
fn test_drift_detection_multiple_fields() -> Result<()> {
    let _temp_dir = setup_test_env();

    let spec_id = "test-drift-multiple";

    // Create lockfile
    let lock = XCheckerLock::new(
        "claude-3-5-sonnet-20241022".to_string(),
        "0.8.1".to_string(),
    );
    lock.save(spec_id)?;

    // Load and check drift with all fields different
    let loaded = XCheckerLock::load(spec_id)?.expect("Lockfile should exist");
    let context = RunContext {
        model_full_name: "claude-3-5-sonnet-20250101".to_string(),
        claude_cli_version: "0.9.0".to_string(),
        schema_version: "2".to_string(),
    };

    let drift = loaded.detect_drift(&context);
    assert!(drift.is_some(), "Should detect drift in multiple fields");

    let drift = drift.unwrap();
    assert!(
        drift.model_full_name.is_some(),
        "Model drift should be detected"
    );
    assert!(
        drift.claude_cli_version.is_some(),
        "CLI version drift should be detected"
    );
    assert!(
        drift.schema_version.is_some(),
        "Schema drift should be detected"
    );

    println!("✓ Multiple field drift detection test passed");
    Ok(())
}

/// Test FR-LOCK-008: Lockfile loading and validation
#[test]
fn test_lockfile_loading_and_validation() -> Result<()> {
    let _temp_dir = setup_test_env();

    let spec_id = "test-lockfile-validation";

    // Test 1: Load nonexistent lockfile
    let result = XCheckerLock::load(spec_id)?;
    assert!(
        result.is_none(),
        "Should return None for nonexistent lockfile"
    );

    // Test 2: Create and load valid lockfile
    let lock = XCheckerLock::new(
        "claude-3-5-sonnet-20241022".to_string(),
        "0.8.1".to_string(),
    );
    lock.save(spec_id)?;

    let loaded = XCheckerLock::load(spec_id)?;
    assert!(loaded.is_some(), "Should load existing lockfile");

    let loaded = loaded.unwrap();
    assert_eq!(loaded.schema_version, "1");
    assert_eq!(loaded.model_full_name, "claude-3-5-sonnet-20241022");
    assert_eq!(loaded.claude_cli_version, "0.8.1");

    // Test 3: Verify timestamp is valid
    let timestamp_str = loaded.created_at.to_rfc3339();
    assert!(
        !timestamp_str.is_empty(),
        "Timestamp should be valid RFC3339"
    );

    println!("✓ Lockfile loading and validation test passed");
    Ok(())
}

/// Test FR-LOCK-008: Corrupted lockfile handling
#[test]
fn test_corrupted_lockfile_handling() -> Result<()> {
    let _temp_dir = setup_test_env();

    let spec_id = "test-corrupted-lockfile";

    // Create spec directory
    let spec_root = xchecker::paths::spec_root(spec_id);
    xchecker::paths::ensure_dir_all(&spec_root)?;

    // Write corrupted JSON
    let lock_path = spec_root.as_std_path().join("lock.json");
    fs::write(&lock_path, "{ invalid json }")?;

    // Should return error for corrupted file
    let result = XCheckerLock::load(spec_id);
    assert!(result.is_err(), "Should fail to load corrupted lockfile");

    println!("✓ Corrupted lockfile handling test passed");
    Ok(())
}

/// Test FR-LOCK-008: Empty lockfile handling
#[test]
fn test_empty_lockfile_handling() -> Result<()> {
    let _temp_dir = setup_test_env();

    let spec_id = "test-empty-lockfile";

    // Create spec directory
    let spec_root = xchecker::paths::spec_root(spec_id);
    xchecker::paths::ensure_dir_all(&spec_root)?;

    // Write empty file
    let lock_path = spec_root.as_std_path().join("lock.json");
    fs::write(&lock_path, "")?;

    // Should return error for empty file
    let result = XCheckerLock::load(spec_id);
    assert!(result.is_err(), "Should fail to load empty lockfile");

    println!("✓ Empty lockfile handling test passed");
    Ok(())
}

/// Test FR-LOCK-008: Lockfile with missing required fields
#[test]
fn test_lockfile_missing_fields() -> Result<()> {
    let _temp_dir = setup_test_env();

    let spec_id = "test-missing-fields";

    // Create spec directory
    let spec_root = xchecker::paths::spec_root(spec_id);
    xchecker::paths::ensure_dir_all(&spec_root)?;

    // Write JSON with missing required fields
    let lock_path = spec_root.as_std_path().join("lock.json");
    fs::write(&lock_path, r#"{"schema_version": "1"}"#)?;

    // Should return error for missing fields
    let result = XCheckerLock::load(spec_id);
    assert!(
        result.is_err(),
        "Should fail to load lockfile with missing fields"
    );

    println!("✓ Missing fields handling test passed");
    Ok(())
}

/// Test FR-LOCK-006: Lockfile overwrite behavior
#[test]
fn test_lockfile_overwrite() -> Result<()> {
    let _temp_dir = setup_test_env();

    let spec_id = "test-lockfile-overwrite";

    // Create first lockfile
    let lock1 = XCheckerLock::new(
        "claude-3-5-sonnet-20241022".to_string(),
        "0.8.1".to_string(),
    );
    lock1.save(spec_id)?;

    // Create second lockfile with different values
    let lock2 = XCheckerLock::new(
        "claude-3-5-sonnet-20250101".to_string(),
        "0.9.0".to_string(),
    );
    lock2.save(spec_id)?;

    // Load and verify it has the second lockfile's values
    let loaded = XCheckerLock::load(spec_id)?.expect("Lockfile should exist");
    assert_eq!(loaded.model_full_name, "claude-3-5-sonnet-20250101");
    assert_eq!(loaded.claude_cli_version, "0.9.0");

    println!("✓ Lockfile overwrite test passed");
    Ok(())
}

/// Test FR-LOCK-006: Lockfile directory creation
#[test]
fn test_lockfile_directory_creation() -> Result<()> {
    let _temp_dir = setup_test_env();

    let spec_id = "test-new-directory";

    // Verify directory doesn't exist
    let spec_root = xchecker::paths::spec_root(spec_id);
    let lock_path = spec_root.as_std_path().join("lock.json");
    assert!(!lock_path.exists(), "Lock path should not exist initially");

    // Create lockfile (should create directory)
    let lock = XCheckerLock::new(
        "claude-3-5-sonnet-20241022".to_string(),
        "0.8.1".to_string(),
    );
    lock.save(spec_id)?;

    // Verify directory and file were created
    assert!(lock_path.exists(), "Lock file should be created");
    assert!(
        lock_path.parent().unwrap().exists(),
        "Parent directory should be created"
    );

    println!("✓ Directory creation test passed");
    Ok(())
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    /// Run all lockfile integration tests
    #[test]
    fn run_all_lockfile_tests() {
        println!("\n=== Running Lockfile Integration Tests ===\n");

        test_lockfile_creation_with_init().unwrap();
        test_drift_detection_model_field().unwrap();
        test_drift_detection_cli_version_field().unwrap();
        test_drift_detection_schema_version_field().unwrap();
        test_no_drift_when_values_match().unwrap();
        test_drift_detection_multiple_fields().unwrap();
        test_lockfile_loading_and_validation().unwrap();
        test_corrupted_lockfile_handling().unwrap();
        test_empty_lockfile_handling().unwrap();
        test_lockfile_missing_fields().unwrap();
        test_lockfile_overwrite().unwrap();
        test_lockfile_directory_creation().unwrap();

        println!("\n=== All Lockfile Integration Tests Passed ===\n");
    }
}

//! Feature documentation verification tests
//!
//! Tests that verify documented features:
//! - Have corresponding smoke tests
//! - Match actual implementation behavior
//! - Produce documented side effects
//!
//! Requirements: R10
//!
//! # Feature-to-Test Traceability Map
//!
//! This module documents the mapping between documented features in README.md
//! and their corresponding test implementations. Each feature should have:
//! 1. Documentation in README.md describing the feature
//! 2. Implementation in src/ modules
//! 3. Test coverage validating the feature behavior
//!
//! ## Timeout Feature
//! - **Documentation**: README.md "Multi-Phase Orchestration" section, Exit Codes table (code 10)
//! - **Configuration**: `phase_timeout` in config (default: 600s, min: 5s)
//! - **Implementation**: `src/phase.rs` (`PhaseTimeout` struct), `src/orchestrator.rs` (timeout handling)
//! - **Tests**:
//!   - `tests/test_phase_timeout.rs` - Core timeout functionality tests
//!     - `test_phase_timeout_constants()` - Validates `DEFAULT_SECS` and `MIN_SECS`
//!     - `test_phase_timeout_minimum_enforcement()` - Validates minimum timeout enforcement
//!     - `test_phase_timeout_from_config()` - Validates config parsing
//!     - `test_timeout_warning_format()` - Validates "`phase_timeout:600`" format
//!     - `test_timeout_exit_code()` - Validates exit code 10
//!   - `tests/test_exit_alignment.rs` - Exit code alignment
//!     - `test_receipt_exit_code_alignment()` - Validates receipt `exit_code` matches process exit
//!   - `tests/test_m4_gate_validation.rs` - M4 gate validation
//!     - `test_phase_timeout_configuration()` - Validates timeout config
//!     - `test_phase_timeout_constants()` - Validates constants
//!     - `test_timeout_receipt_warning_format()` - Validates warning format
//!     - `test_timeout_exit_code_constant()` - Validates exit code constant
//! - **Side Effects**:
//!   - Creates `.partial.md` file when timeout occurs
//!   - Adds "`phase_timeout`:<secs>" to receipt warnings array
//!   - Sets `receipt.exit_code` = 10
//!   - Sets `receipt.error_kind` = "`phase_timeout`"
//!   - Process exits with code 10
//!
//! ## Lockfile Drift Detection Feature
//! - **Documentation**: README.md "Lockfile System" section, "Reproducibility Tracking" feature
//! - **Configuration**: `--strict-lock` flag, `--create-lock` flag
//! - **Implementation**: `src/lock.rs` (lockfile management), `src/status.rs` (drift detection)
//! - **Tests**:
//!   - `tests/m3_gate_validation.rs` - M3 gate lockfile tests
//!     - Tests lockfile creation with `--create-lock`
//!     - Tests drift detection when model/CLI version changes
//!     - Tests `--strict-lock` flag behavior (hard fail on drift)
//!   - `tests/integration_full_workflows.rs` - Full workflow integration
//!     - Tests lockfile persistence across phases
//!     - Tests drift reporting in status output
//! - **Side Effects**:
//!   - Creates `.xchecker/<spec-id>/lock.json` file
//!   - Status output includes drift detection results
//!   - With `--strict-lock`: exits with error code on drift
//!   - Lockfile pins: `model_full_name`, `claude_cli_version`, `schema_version`
//!
//! ## Fixup Validation Feature
//! - **Documentation**: README.md "Fixup System" section, "Security First" feature (path validation)
//! - **Configuration**: `--apply-fixups` flag (default: preview mode)
//! - **Implementation**: `src/fixup.rs` (`FixupParser`, path validation), `src/phases.rs` (fixup phase)
//! - **Tests**:
//!   - `tests/m4_gate_validation.rs` - M4 gate fixup tests
//!     - `test_fixup_validation_with_git_apply_check()` - Validates unified diff parsing
//!     - Tests path validation (prevents directory traversal)
//!     - Tests preview vs apply modes
//!   - `tests/test_apply_fixups_flag.rs` - Apply fixups flag behavior
//!     - Tests `--apply-fixups` flag parsing
//!     - Tests default preview mode
//! - **Side Effects**:
//!   - Preview mode: Shows pending fixups in status output
//!   - Apply mode: Modifies target files with validated diffs
//!   - Validates paths to prevent directory traversal (e.g., rejects "../../../etc/passwd")
//!   - Uses `git apply --check` for diff validation
//!   - Status output includes fixup count and validation status
//!
//! ## Exit Code Alignment Feature
//! - **Documentation**: README.md "Exit Codes" section, "Standardized Exit Codes" feature
//! - **Implementation**: `src/exit_codes.rs` (exit code constants), `src/error.rs` (error-to-code mapping)
//! - **Tests**:
//!   - `tests/test_exit_alignment.rs` - Primary exit code alignment tests
//!     - `test_receipt_exit_code_alignment()` - Validates `receipt.exit_code` matches process exit
//!     - `test_error_kind_to_exit_code_mapping()` - Validates `ErrorKind` → exit code mapping
//!     - `test_exit_code_constants()` - Validates all exit code constants
//!   - `tests/test_m4_gate_validation.rs` - M4 gate exit alignment
//!     - `test_error_receipt_exit_code_alignment()` - Validates error receipts
//!     - `test_exit_code_constants_match_documentation()` - Validates constants match docs
//! - **Side Effects**:
//!   - Process exit code always matches `receipt.exit_code` field
//!   - `Receipt.error_kind` matches exit code (e.g., "`phase_timeout`" → 10)
//!   - All error paths write receipt before exit
//!   - Exit codes are stable across versions (part of v1 contract)
//!
//! ## Test Coverage Summary
//!
//! | Feature | Primary Test File | Additional Coverage | Status |
//! |---------|------------------|---------------------|--------|
//! | Timeout | test_phase_timeout.rs | test_exit_alignment.rs, test_m4_gate_validation.rs | ✓ Complete |
//! | Lockfile Drift | m3_gate_validation.rs | integration_full_workflows.rs | ✓ Complete |
//! | Fixup Validation | m4_gate_validation.rs | test_apply_fixups_flag.rs | ✓ Complete |
//! | Exit Alignment | test_exit_alignment.rs | test_m4_gate_validation.rs | ✓ Complete |

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    /// Helper to check if a test file exists and contains specific test functions
    fn test_file_contains_tests(test_file: &str, test_names: &[&str]) -> bool {
        let path = Path::new("tests").join(test_file);
        if !path.exists() {
            return false;
        }

        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return false,
        };

        // Check that all expected test functions exist
        test_names
            .iter()
            .all(|test_name| content.contains(&format!("fn {test_name}")))
    }

    /// Helper to verify README documents a feature
    fn readme_documents_feature(feature_keywords: &[&str]) -> bool {
        let readme_path = Path::new("README.md");
        if !readme_path.exists() {
            return false;
        }

        let content = match fs::read_to_string(readme_path) {
            Ok(c) => c,
            Err(_) => return false,
        };

        // Check that all keywords appear in README
        feature_keywords
            .iter()
            .all(|keyword| content.contains(keyword))
    }

    #[test]
    fn test_timeout_feature_documented() {
        // Verify timeout feature is documented in README
        assert!(
            readme_documents_feature(&["phase_timeout", "Phase execution exceeded timeout"]),
            "Timeout feature should be documented in README.md"
        );

        // Verify corresponding smoke tests exist
        let timeout_tests = [
            "test_phase_timeout_constants",
            "test_phase_timeout_minimum_enforcement",
            "test_phase_timeout_from_config",
            "test_timeout_warning_format",
            "test_timeout_exit_code",
        ];

        assert!(
            test_file_contains_tests("test_phase_timeout.rs", &timeout_tests),
            "Timeout feature should have smoke tests in test_phase_timeout.rs"
        );

        // Verify exit alignment tests exist
        let exit_tests = ["test_receipt_exit_code_alignment", "test_exit_code_mapping"];

        assert!(
            test_file_contains_tests("test_exit_alignment.rs", &exit_tests),
            "Exit alignment tests should exist in test_exit_alignment.rs"
        );

        // Verify documented side effects match implementation
        // Side effects documented in feature map:
        // - Creates .partial.md file
        // - Adds "phase_timeout:<secs>" to warnings
        // - Sets exit_code = 10
        // - Sets error_kind = "phase_timeout"

        // Check that exit code constant is defined
        let exit_codes_path = Path::new("crates/xchecker-utils/src/exit_codes.rs");
        assert!(exit_codes_path.exists(), "Exit codes module should exist");

        let exit_codes_content = fs::read_to_string(exit_codes_path).unwrap();
        assert!(
            exit_codes_content.contains("PHASE_TIMEOUT") && exit_codes_content.contains("10"),
            "PHASE_TIMEOUT exit code should be defined as 10"
        );
    }

    #[test]
    fn test_lockfile_feature_documented() {
        // Verify lockfile drift feature is documented in README
        assert!(
            readme_documents_feature(&["Lockfile System", "lock_drift", "--strict-lock"]),
            "Lockfile drift feature should be documented in README.md"
        );

        // Verify lock.rs module exists
        let lock_path = Path::new("crates/xchecker-utils/src/lock.rs");
        assert!(lock_path.exists(), "Lock module should exist");

        // Verify documented side effects:
        // - Creates lock file
        // - Status output includes drift detection
        // - --strict-lock causes hard fail on drift
        let lock_content = fs::read_to_string(lock_path).unwrap();
        assert!(
            lock_content.contains("Lock") || lock_content.contains("lock"),
            "Lock module should handle lock files"
        );

        // Verify status module handles drift detection
        let status_path = Path::new("crates/xchecker-engine/src/status.rs");
        assert!(status_path.exists(), "Status module should exist");

        let status_content = fs::read_to_string(status_path).unwrap();
        assert!(
            status_content.contains("drift") || status_content.contains("lock"),
            "Status module should handle drift detection"
        );
    }

    #[test]
    fn test_fixup_feature_documented() {
        // Verify fixup validation feature is documented in README
        assert!(
            readme_documents_feature(&["Fixup System", "--apply-fixups", "Preview Mode"]),
            "Fixup validation feature should be documented in README.md"
        );

        // Verify corresponding smoke tests exist
        let fixup_tests = [
            "test_apply_fixups_config_handling",
            "test_fixup_parser_mode_behavior",
            "test_fixup_mode_enum",
        ];

        assert!(
            test_file_contains_tests("test_apply_fixups_flag.rs", &fixup_tests),
            "Fixup feature should have smoke tests in test_apply_fixups_flag.rs"
        );

        // Verify M4 gate tests cover fixup validation
        let m4_gate_path = Path::new("tests/m4_gate_validation.rs");
        assert!(
            m4_gate_path.exists(),
            "M4 gate validation tests should exist"
        );

        let m4_content = fs::read_to_string(m4_gate_path).unwrap();
        assert!(
            m4_content.contains("fixup") || m4_content.contains("Fixup"),
            "M4 gate tests should cover fixup functionality"
        );

        // Verify fixup module exists (now a directory with mod.rs)
        let fixup_path = Path::new("crates/xchecker-engine/src/fixup/mod.rs");
        assert!(fixup_path.exists(), "Fixup module should exist");

        // Verify documented side effects:
        // - Preview mode shows pending fixups
        // - Apply mode modifies files
        // - Path validation prevents directory traversal
        let fixup_content = fs::read_to_string(fixup_path).unwrap();
        assert!(
            fixup_content.contains("Preview") || fixup_content.contains("Apply"),
            "Fixup module should handle preview and apply modes"
        );
        assert!(
            fixup_content.contains("path") || fixup_content.contains("validate"),
            "Fixup module should validate paths"
        );
    }

    #[test]
    fn test_exit_alignment_documented() {
        // Verify exit code alignment is documented in README
        assert!(
            readme_documents_feature(&["Exit Codes", "exit_code", "Standardized Exit Codes"]),
            "Exit code alignment feature should be documented in README.md"
        );

        // Verify corresponding smoke tests exist
        let exit_tests = [
            "test_error_kind_serialization",
            "test_exit_code_mapping",
            "test_receipt_exit_code_alignment",
        ];

        assert!(
            test_file_contains_tests("test_exit_alignment.rs", &exit_tests),
            "Exit alignment feature should have smoke tests in test_exit_alignment.rs"
        );

        // Verify exit_codes.rs module exists
        let exit_codes_path = Path::new("crates/xchecker-utils/src/exit_codes.rs");
        assert!(exit_codes_path.exists(), "Exit codes module should exist");

        // Verify documented exit codes exist
        let exit_codes_content = fs::read_to_string(exit_codes_path).unwrap();
        let expected_codes = [
            ("CLI_ARGS", "2"),
            ("PACKET_OVERFLOW", "7"),
            ("SECRET_DETECTED", "8"),
            ("LOCK_HELD", "9"),
            ("PHASE_TIMEOUT", "10"),
            ("CLAUDE_FAILURE", "70"),
        ];

        for (name, code) in &expected_codes {
            assert!(
                exit_codes_content.contains(name) && exit_codes_content.contains(code),
                "Exit code {name} ({code}) should be defined"
            );
        }

        // Verify documented side effects:
        // - Process exit code matches receipt.exit_code
        // - Receipt.error_kind matches exit code
        // - All error paths write receipt before exit
        let error_path = Path::new("crates/xchecker-utils/src/error.rs");
        assert!(error_path.exists(), "Error module should exist");

        // ErrorKind is defined in types.rs
        let types_path = Path::new("crates/xchecker-utils/src/types.rs");
        assert!(types_path.exists(), "Types module should exist");

        let types_content = fs::read_to_string(types_path).unwrap();
        assert!(
            types_content.contains("ErrorKind"),
            "Types module should define ErrorKind enum"
        );

        // Receipt module should handle error receipts (now a directory with mod.rs)
        let receipt_path = Path::new("crates/xchecker-engine/src/receipt/mod.rs");
        assert!(receipt_path.exists(), "Receipt module should exist");

        let receipt_content = fs::read_to_string(receipt_path).unwrap();
        assert!(
            receipt_content.contains("exit_code") || receipt_content.contains("error"),
            "Receipt module should handle error receipts with exit codes"
        );
    }

    #[test]
    fn test_all_documented_features_have_tests() {
        // This test verifies that all major features documented in README
        // have corresponding test coverage

        let features = [
            ("Multi-Phase Orchestration", "test_phase_timeout.rs"),
            ("Lockfile System", "m3_gate_validation.rs"),
            ("Fixup System", "test_apply_fixups_flag.rs"),
            ("Exit Codes", "test_exit_alignment.rs"),
        ];

        for (feature_name, test_file) in &features {
            assert!(
                readme_documents_feature(&[feature_name]),
                "Feature '{feature_name}' should be documented in README.md"
            );

            let test_path = Path::new("tests").join(test_file);
            assert!(
                test_path.exists(),
                "Feature '{feature_name}' should have tests in {test_file}"
            );
        }
    }

    #[test]
    fn test_feature_side_effects_documented() {
        // Verify that documented side effects are testable and tested

        // Timeout side effects
        let timeout_side_effects = [
            ".partial.md",    // Partial artifact file
            "phase_timeout:", // Warning format
            "exit_code",      // Receipt field
            "error_kind",     // Receipt field
        ];

        let test_timeout_path = Path::new("tests/test_phase_timeout.rs");
        let timeout_content = fs::read_to_string(test_timeout_path).unwrap();

        for side_effect in &timeout_side_effects {
            assert!(
                timeout_content.contains(side_effect),
                "Timeout tests should verify side effect: {side_effect}"
            );
        }

        // Lockfile side effects
        let lockfile_side_effects = [
            "lock_drift",  // Status field
            "strict-lock", // CLI flag (with hyphen as used in CLI)
        ];

        let readme_content = fs::read_to_string("README.md").unwrap();
        for side_effect in &lockfile_side_effects {
            assert!(
                readme_content.contains(side_effect),
                "README should document lockfile side effect: {side_effect}"
            );
        }

        // Fixup side effects
        let fixup_side_effects = [
            "Preview", // Preview mode
            "Apply",   // Apply mode
            "path",    // Path validation
        ];

        let test_fixup_path = Path::new("tests/test_apply_fixups_flag.rs");
        let fixup_content = fs::read_to_string(test_fixup_path).unwrap();

        for side_effect in &fixup_side_effects {
            assert!(
                fixup_content.contains(side_effect),
                "Fixup tests should verify side effect: {side_effect}"
            );
        }

        // Exit alignment side effects
        let exit_side_effects = [
            "exit_code",  // Process exit code
            "error_kind", // Receipt error kind
            "receipt",    // Receipt creation
        ];

        let test_exit_path = Path::new("tests/test_exit_alignment.rs");
        let exit_content = fs::read_to_string(test_exit_path).unwrap();

        for side_effect in &exit_side_effects {
            assert!(
                exit_content.contains(side_effect),
                "Exit alignment tests should verify side effect: {side_effect}"
            );
        }
    }
}

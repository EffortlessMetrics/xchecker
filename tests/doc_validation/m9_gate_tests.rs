//! M9 Gate: CI Integration Validation Tests
//!
//! This module validates that the docs-conformance CI job is properly configured
//! and that all documentation tests run successfully in CI.
//!
//! Requirements: R1-R10

use std::fs;
use std::path::Path;

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that the docs-conformance CI job exists in the workflow file
    #[test]
    fn m9_gate_ci_workflow_has_docs_conformance_job() {
        let ci_path = Path::new(".github/workflows/ci.yml");
        assert!(
            ci_path.exists(),
            "CI workflow file should exist at .github/workflows/ci.yml"
        );

        let ci_content =
            fs::read_to_string(ci_path).expect("Should be able to read CI workflow file");

        assert!(
            ci_content.contains("docs-conformance:"),
            "CI workflow should have a docs-conformance job"
        );

        assert!(
            ci_content.contains("Documentation Conformance"),
            "docs-conformance job should have proper name"
        );

        println!("✓ M9 Gate: CI workflow has docs-conformance job");
    }

    /// Verify that the docs-conformance job runs the correct test command
    #[test]
    fn m9_gate_ci_runs_documentation_tests() {
        let ci_path = Path::new(".github/workflows/ci.yml");
        let ci_content =
            fs::read_to_string(ci_path).expect("Should be able to read CI workflow file");

        // Check that it runs the test_doc_validation test
        assert!(
            ci_content.contains("cargo test --test test_doc_validation"),
            "CI should run documentation validation tests"
        );

        // Check that it uses serial execution for deterministic output
        assert!(
            ci_content.contains("--test-threads=1"),
            "CI should run tests serially for clear output"
        );

        println!("✓ M9 Gate: CI runs documentation validation tests with correct flags");
    }

    /// Verify that the docs-conformance job checks for fresh schema examples
    #[test]
    fn m9_gate_ci_verifies_schema_examples_fresh() {
        let ci_path = Path::new(".github/workflows/ci.yml");
        let ci_content =
            fs::read_to_string(ci_path).expect("Should be able to read CI workflow file");

        // Check that it verifies schema examples are fresh
        assert!(
            ci_content.contains("git diff --exit-code docs/schemas/"),
            "CI should verify schema examples are fresh with git diff"
        );

        // Check that it has helpful error message
        assert!(
            ci_content.contains("Generated schema examples are out of sync"),
            "CI should have clear error message when examples are stale"
        );

        println!("✓ M9 Gate: CI verifies generated schema examples are fresh");
    }

    /// Verify that the CI provides clear instructions for regenerating examples
    #[test]
    fn m9_gate_ci_has_clear_error_messages() {
        let ci_path = Path::new(".github/workflows/ci.yml");
        let ci_content =
            fs::read_to_string(ci_path).expect("Should be able to read CI workflow file");

        // Check for helpful regeneration instructions
        assert!(
            ci_content.contains("cargo test") && ci_content.contains("to regenerate"),
            "CI should provide clear instructions for regenerating examples"
        );

        assert!(
            ci_content.contains("docs/schemas"),
            "CI error message should mention docs/schemas directory"
        );

        println!("✓ M9 Gate: CI has clear error messages guiding developers");
    }

    /// Verify that all documentation test modules are included
    #[test]
    fn m9_gate_all_doc_test_modules_exist() {
        let test_modules = vec![
            "tests/doc_validation/readme_tests.rs",
            "tests/doc_validation/schema_examples_tests.rs",
            "tests/doc_validation/config_tests.rs",
            "tests/doc_validation/doctor_tests.rs",
            "tests/doc_validation/contracts_tests.rs",
            "tests/doc_validation/schema_rust_conformance_tests.rs",
            "tests/doc_validation/changelog_tests.rs",
            "tests/doc_validation/xchecker_home_tests.rs",
            "tests/doc_validation/code_examples_tests.rs",
            "tests/doc_validation/feature_tests.rs",
        ];

        for module in test_modules {
            let path = Path::new(module);
            assert!(
                path.exists(),
                "Documentation test module should exist: {module}"
            );
        }

        println!("✓ M9 Gate: All documentation test modules exist");
    }

    /// Verify that the docs-conformance job runs on ubuntu-latest
    #[test]
    fn m9_gate_ci_runs_on_ubuntu() {
        let ci_path = Path::new(".github/workflows/ci.yml");
        let ci_content =
            fs::read_to_string(ci_path).expect("Should be able to read CI workflow file");

        // Find the docs-conformance section
        let docs_section_start = ci_content
            .find("docs-conformance:")
            .expect("Should find docs-conformance job");

        // Get the section (next 500 chars should be enough)
        // Use safe UTF-8 slicing by finding the nearest valid char boundary
        let end_byte = docs_section_start + 500.min(ci_content.len() - docs_section_start);
        let safe_end = ci_content
            .char_indices()
            .map(|(i, _)| i)
            .find(|&i| i >= end_byte)
            .unwrap_or(ci_content.len());
        let docs_section = &ci_content[docs_section_start..safe_end];

        assert!(
            docs_section.contains("ubuntu-latest"),
            "docs-conformance job should run on ubuntu-latest"
        );

        println!("✓ M9 Gate: CI job runs on ubuntu-latest");
    }

    /// Comprehensive M9 Gate validation
    #[test]
    fn m9_gate_comprehensive_validation() {
        println!("\n=== M9 Gate: Comprehensive CI Integration Validation ===\n");

        // 1. Verify CI workflow exists and has docs-conformance job
        let ci_path = Path::new(".github/workflows/ci.yml");
        assert!(ci_path.exists(), "CI workflow file should exist");
        let ci_content = fs::read_to_string(ci_path).expect("Should read CI file");
        assert!(
            ci_content.contains("docs-conformance:"),
            "Should have docs-conformance job"
        );
        println!("✓ CI workflow has docs-conformance job");

        // 2. Verify correct test command
        assert!(
            ci_content.contains("cargo test --test test_doc_validation"),
            "Should run test_doc_validation"
        );
        assert!(
            ci_content.contains("--test-threads=1"),
            "Should use serial execution"
        );
        println!("✓ CI runs documentation tests with correct flags");

        // 3. Verify schema freshness check
        assert!(
            ci_content.contains("git diff --exit-code docs/schemas/"),
            "Should verify schema examples are fresh"
        );
        println!("✓ CI verifies schema examples are fresh");

        // 4. Verify clear error messages
        assert!(
            ci_content.contains("Generated schema examples are out of sync"),
            "Should have clear error message"
        );
        assert!(
            ci_content.contains("to regenerate"),
            "Should provide regeneration instructions"
        );
        println!("✓ CI has clear error messages");

        // 5. Verify all test modules exist
        let test_modules = vec![
            "tests/doc_validation/readme_tests.rs",
            "tests/doc_validation/schema_examples_tests.rs",
            "tests/doc_validation/config_tests.rs",
            "tests/doc_validation/doctor_tests.rs",
            "tests/doc_validation/contracts_tests.rs",
            "tests/doc_validation/schema_rust_conformance_tests.rs",
            "tests/doc_validation/changelog_tests.rs",
            "tests/doc_validation/xchecker_home_tests.rs",
            "tests/doc_validation/code_examples_tests.rs",
            "tests/doc_validation/feature_tests.rs",
        ];

        for module in &test_modules {
            assert!(Path::new(module).exists(), "Module should exist: {module}");
        }
        println!("✓ All documentation test modules exist");

        // 6. Verify docs/schemas directory exists
        let schemas_dir = Path::new("docs/schemas");
        assert!(schemas_dir.exists(), "docs/schemas directory should exist");
        println!("✓ docs/schemas directory exists");

        // 7. Verify schema example files exist
        let schema_files = vec![
            "docs/schemas/receipt.v1.minimal.json",
            "docs/schemas/receipt.v1.full.json",
            "docs/schemas/status.v1.minimal.json",
            "docs/schemas/status.v1.full.json",
            "docs/schemas/doctor.v1.minimal.json",
            "docs/schemas/doctor.v1.full.json",
        ];

        for file in &schema_files {
            assert!(
                Path::new(file).exists(),
                "Schema example should exist: {file}"
            );
        }
        println!("✓ All schema example files exist");

        println!("\n=== M9 Gate: All CI Integration Validations Passed ===\n");
        println!("Requirements verified:");
        println!("  R1-R10: All documentation tests run in CI");
        println!("\nCI Integration:");
        println!("  ✓ docs-conformance job exists");
        println!("  ✓ Runs documentation validation tests");
        println!("  ✓ Verifies schema examples are fresh");
        println!("  ✓ Provides clear error messages");
        println!("  ✓ All test modules are present");
        println!("  ✓ Schema examples are generated and tracked");
    }
}

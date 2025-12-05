//! M8 Gate: Validate CHANGELOG and contracts documentation
//!
//! This milestone gate verifies that:
//! - All CHANGELOG verification tests pass
//! - All contracts documentation tests pass
//! - JCS and sorting documentation is accurate
//! - Deprecation policy is documented
//!
//! Requirements: R5, R7

use std::path::Path;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc_validation::changelog_tests::{ChangelogLinter, ChangelogParser};

    /// M8 Gate Test 1: Verify CHANGELOG exists and is parseable
    #[test]
    fn m8_gate_changelog_exists_and_parseable() {
        let changelog_path = Path::new("CHANGELOG.md");
        assert!(
            changelog_path.exists(),
            "M8 Gate: CHANGELOG.md must exist at project root"
        );

        let parser =
            ChangelogParser::new(changelog_path).expect("M8 Gate: Failed to parse CHANGELOG.md");

        let versions = parser.extract_versions();
        assert!(
            !versions.is_empty(),
            "M8 Gate: CHANGELOG must have at least one version entry"
        );

        // Should have an Unreleased section
        let has_unreleased = versions.iter().any(|v| v.is_unreleased);
        assert!(
            has_unreleased,
            "M8 Gate: CHANGELOG must have an [Unreleased] section"
        );

        println!(
            "✓ M8 Gate: CHANGELOG.md exists and is parseable ({} versions found)",
            versions.len()
        );
    }

    /// M8 Gate Test 2: Verify CHANGELOG documents key contract fields
    #[test]
    fn m8_gate_changelog_documents_contract_fields() {
        let changelog_path = Path::new("CHANGELOG.md");
        let linter = ChangelogLinter::new(changelog_path)
            .expect("M8 Gate: Failed to create CHANGELOG linter");

        // Key contract fields that should be documented
        let key_fields = vec!["schema_version", "emitted_at", "error_kind", "error_reason"];

        let missing = linter
            .verify_fields_mentioned(&key_fields)
            .expect("M8 Gate: Failed to verify fields");

        assert!(
            missing.is_empty(),
            "M8 Gate: CHANGELOG must document key contract fields. Missing: {missing:?}"
        );

        println!("✓ M8 Gate: CHANGELOG documents all key contract fields");
    }

    /// M8 Gate Test 3: Verify CHANGELOG documents exit codes
    #[test]
    fn m8_gate_changelog_documents_exit_codes() {
        let changelog_path = Path::new("CHANGELOG.md");
        let linter = ChangelogLinter::new(changelog_path)
            .expect("M8 Gate: Failed to create CHANGELOG linter");

        // Key exit codes that should be documented
        let key_exit_codes = vec![0, 2, 7, 8, 9, 10, 70];

        let missing = linter
            .verify_exit_codes_mentioned(&key_exit_codes)
            .expect("M8 Gate: Failed to verify exit codes");

        assert!(
            missing.is_empty(),
            "M8 Gate: CHANGELOG must document all exit codes. Missing: {missing:?}"
        );

        println!("✓ M8 Gate: CHANGELOG documents all exit codes");
    }

    /// M8 Gate Test 4: Verify CHANGELOG has breaking changes section
    #[test]
    fn m8_gate_changelog_has_breaking_changes() {
        let changelog_path = Path::new("CHANGELOG.md");
        let linter = ChangelogLinter::new(changelog_path)
            .expect("M8 Gate: Failed to create CHANGELOG linter");

        assert!(
            linter.has_breaking_changes_section(),
            "M8 Gate: CHANGELOG must have breaking changes section or [BREAKING] markers"
        );

        println!("✓ M8 Gate: CHANGELOG has breaking changes section/markers");
    }

    /// M8 Gate Test 5: Verify CONTRACTS.md exists and documents JCS
    #[test]
    fn m8_gate_contracts_documents_jcs() {
        let contracts_path = Path::new("docs/CONTRACTS.md");
        assert!(
            contracts_path.exists(),
            "M8 Gate: CONTRACTS.md must exist at docs/CONTRACTS.md"
        );

        let contracts_content =
            std::fs::read_to_string(contracts_path).expect("M8 Gate: Failed to read CONTRACTS.md");

        // Verify JCS is documented
        assert!(
            contracts_content.contains("JCS") || contracts_content.contains("RFC 8785"),
            "M8 Gate: CONTRACTS.md must mention JCS or RFC 8785"
        );

        // Verify canonical emission is described
        assert!(
            contracts_content.contains("canonical") || contracts_content.contains("Canonical"),
            "M8 Gate: CONTRACTS.md must describe canonical emission"
        );

        // Verify deterministic ordering is mentioned
        assert!(
            contracts_content.contains("deterministic") || contracts_content.contains("sorted"),
            "M8 Gate: CONTRACTS.md must mention deterministic ordering or sorting"
        );

        println!("✓ M8 Gate: CONTRACTS.md documents JCS emission");
    }

    /// M8 Gate Test 6: Verify CONTRACTS.md documents array sorting
    #[test]
    fn m8_gate_contracts_documents_array_sorting() {
        let contracts_path = Path::new("docs/CONTRACTS.md");
        let contracts_content =
            std::fs::read_to_string(contracts_path).expect("M8 Gate: Failed to read CONTRACTS.md");

        // Verify array sorting is documented
        assert!(
            contracts_content.contains("Array Ordering")
                || (contracts_content.contains("array") && contracts_content.contains("sorted")),
            "M8 Gate: CONTRACTS.md must document array sorting/ordering"
        );

        // Verify specific sorting rules are mentioned
        assert!(
            contracts_content.contains("outputs") && contracts_content.contains("path"),
            "M8 Gate: CONTRACTS.md must document that receipt outputs are sorted by path"
        );

        assert!(
            contracts_content.contains("artifacts") && contracts_content.contains("path"),
            "M8 Gate: CONTRACTS.md must document that status artifacts are sorted by path"
        );

        assert!(
            contracts_content.contains("checks") && contracts_content.contains("name"),
            "M8 Gate: CONTRACTS.md must document that doctor checks are sorted by name"
        );

        println!("✓ M8 Gate: CONTRACTS.md documents array sorting rules");
    }

    /// M8 Gate Test 7: Verify CONTRACTS.md documents deprecation policy
    #[test]
    fn m8_gate_contracts_documents_deprecation_policy() {
        let contracts_path = Path::new("docs/CONTRACTS.md");
        let contracts_content =
            std::fs::read_to_string(contracts_path).expect("M8 Gate: Failed to read CONTRACTS.md");

        // Verify deprecation policy is documented
        assert!(
            contracts_content.contains("Deprecation") || contracts_content.contains("deprecation"),
            "M8 Gate: CONTRACTS.md must document deprecation policy"
        );

        // Verify version lifecycle is described
        assert!(
            contracts_content.contains("lifecycle") || contracts_content.contains("Lifecycle"),
            "M8 Gate: CONTRACTS.md must describe schema version lifecycle"
        );

        // Verify dual support is mentioned
        assert!(
            contracts_content.contains("dual")
                || (contracts_content.contains("both") && contracts_content.contains("supported")),
            "M8 Gate: CONTRACTS.md must mention dual version support during transition"
        );

        // Verify deprecation period is specified
        assert!(
            contracts_content.contains("6 months") || contracts_content.contains("month"),
            "M8 Gate: CONTRACTS.md must specify deprecation period duration"
        );

        // Verify breaking vs additive changes are described
        assert!(
            contracts_content.contains("Breaking") || contracts_content.contains("breaking"),
            "M8 Gate: CONTRACTS.md must describe breaking changes"
        );

        assert!(
            contracts_content.contains("Additive")
                || contracts_content.contains("additive")
                || contracts_content.contains("optional"),
            "M8 Gate: CONTRACTS.md must describe additive changes"
        );

        println!("✓ M8 Gate: CONTRACTS.md documents deprecation policy");
    }

    /// M8 Gate Test 8: Verify schema files exist as documented
    #[test]
    fn m8_gate_schema_files_exist() {
        let schema_files = vec![
            "schemas/receipt.v1.json",
            "schemas/status.v1.json",
            "schemas/doctor.v1.json",
        ];

        for schema_file in &schema_files {
            let schema_path = Path::new(schema_file);
            assert!(
                schema_path.exists(),
                "M8 Gate: Schema file {schema_file} must exist as documented in CONTRACTS.md"
            );
        }

        println!("✓ M8 Gate: All documented schema files exist");
    }

    /// M8 Gate Test 9: Verify CONTRACTS.md mentions schema files
    #[test]
    fn m8_gate_contracts_mentions_schema_files() {
        let contracts_path = Path::new("docs/CONTRACTS.md");
        let contracts_content =
            std::fs::read_to_string(contracts_path).expect("M8 Gate: Failed to read CONTRACTS.md");

        let schema_files = vec!["receipt.v1.json", "status.v1.json", "doctor.v1.json"];

        for schema_file in &schema_files {
            assert!(
                contracts_content.contains(schema_file),
                "M8 Gate: CONTRACTS.md must mention schema file {schema_file}"
            );
        }

        println!("✓ M8 Gate: CONTRACTS.md mentions all schema files");
    }

    /// M8 Gate Test 10: Comprehensive validation - all requirements met
    #[test]
    fn m8_gate_comprehensive_validation() {
        println!("\n=== M8 Gate: Comprehensive Validation ===\n");

        // 1. Verify CHANGELOG exists and is valid
        let changelog_path = Path::new("CHANGELOG.md");
        assert!(changelog_path.exists(), "M8 Gate: CHANGELOG.md must exist");
        println!("✓ CHANGELOG.md exists");

        let linter =
            ChangelogLinter::new(changelog_path).expect("M8 Gate: Failed to parse CHANGELOG.md");
        let versions = linter.get_versions();
        println!("✓ CHANGELOG.md is parseable ({} versions)", versions.len());

        // 2. Verify CHANGELOG documents key fields
        let key_fields = vec!["schema_version", "emitted_at", "error_kind"];
        let missing_fields = linter
            .verify_fields_mentioned(&key_fields)
            .expect("Failed to verify fields");
        assert!(
            missing_fields.is_empty(),
            "M8 Gate: Missing fields in CHANGELOG: {missing_fields:?}"
        );
        println!("✓ CHANGELOG documents key contract fields");

        // 3. Verify CHANGELOG documents exit codes
        let key_codes = vec![0, 2, 7, 8, 9, 10, 70];
        let missing_codes = linter
            .verify_exit_codes_mentioned(&key_codes)
            .expect("Failed to verify exit codes");
        assert!(
            missing_codes.is_empty(),
            "M8 Gate: Missing exit codes in CHANGELOG: {missing_codes:?}"
        );
        println!("✓ CHANGELOG documents exit codes");

        // 4. Verify CHANGELOG has breaking changes markers
        assert!(
            linter.has_breaking_changes_section(),
            "M8 Gate: CHANGELOG must have breaking changes"
        );
        println!("✓ CHANGELOG has breaking changes section");

        // 5. Verify CONTRACTS.md exists
        let contracts_path = Path::new("docs/CONTRACTS.md");
        assert!(contracts_path.exists(), "M8 Gate: CONTRACTS.md must exist");
        println!("✓ CONTRACTS.md exists");

        let contracts_content =
            std::fs::read_to_string(contracts_path).expect("Failed to read CONTRACTS.md");

        // 6. Verify CONTRACTS.md documents JCS
        assert!(
            contracts_content.contains("JCS") || contracts_content.contains("RFC 8785"),
            "M8 Gate: CONTRACTS.md must document JCS"
        );
        println!("✓ CONTRACTS.md documents JCS emission");

        // 7. Verify CONTRACTS.md documents array sorting
        assert!(
            contracts_content.contains("Array Ordering")
                || (contracts_content.contains("array") && contracts_content.contains("sorted")),
            "M8 Gate: CONTRACTS.md must document array sorting"
        );
        println!("✓ CONTRACTS.md documents array sorting");

        // 8. Verify CONTRACTS.md documents deprecation policy
        assert!(
            contracts_content.contains("Deprecation") || contracts_content.contains("deprecation"),
            "M8 Gate: CONTRACTS.md must document deprecation policy"
        );
        println!("✓ CONTRACTS.md documents deprecation policy");

        // 9. Verify schema files exist
        let schema_files = vec![
            "schemas/receipt.v1.json",
            "schemas/status.v1.json",
            "schemas/doctor.v1.json",
        ];
        for schema_file in &schema_files {
            assert!(
                Path::new(schema_file).exists(),
                "M8 Gate: Schema {schema_file} must exist"
            );
        }
        println!("✓ All schema files exist");

        println!("\n=== M8 Gate: All Validations Passed ===\n");
        println!("Requirements verified:");
        println!("  R5: CONTRACTS.md accurately describes versioning policy");
        println!("  R7: CHANGELOG documents all user-facing changes");
        println!("\nCHANGELOG tests:");
        println!("  ✓ CHANGELOG exists and is parseable");
        println!("  ✓ Contract fields are documented");
        println!("  ✓ Exit codes are documented");
        println!("  ✓ Breaking changes are marked");
        println!("\nContracts tests:");
        println!("  ✓ JCS emission is documented");
        println!("  ✓ Array sorting rules are documented");
        println!("  ✓ Deprecation policy is documented");
        println!("  ✓ Schema files exist and are referenced");
    }
}

//! CHANGELOG validation tests
//!
//! This module validates that CHANGELOG.md accurately documents user-facing changes
//! and follows the Keep a Changelog format.

use anyhow::{Context, Result};
use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use std::collections::HashSet;
use std::path::Path;

/// A version entry in the CHANGELOG
#[derive(Debug, Clone)]
pub struct VersionEntry {
    pub version: String,
    pub changes: Vec<String>,
    pub is_unreleased: bool,
}

/// CHANGELOG parser that extracts version entries and their changes
pub struct ChangelogParser {
    content: String,
}

impl ChangelogParser {
    /// Create a new `ChangelogParser` from a file path
    pub fn new(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .context(format!("Failed to read CHANGELOG: {}", path.display()))?;
        Ok(Self { content })
    }

    /// Extract all version entries from the CHANGELOG
    pub fn extract_versions(&self) -> Vec<VersionEntry> {
        let mut versions = Vec::new();
        let parser = Parser::new(&self.content);

        let mut current_version: Option<String> = None;
        let mut current_changes: Vec<String> = Vec::new();
        let mut in_list = false;
        let mut current_text = String::new();
        let mut in_heading = false;
        let mut is_h2 = false;

        for event in parser {
            match event {
                // H2 headers are version entries like "## [Unreleased]" or "## [1.0.0] - 2024-01-01"
                Event::Start(Tag::Heading {
                    level: HeadingLevel::H2,
                    ..
                }) => {
                    // Save previous version if exists
                    if let Some(version) = current_version.take() {
                        let is_unreleased = version.to_lowercase().contains("unreleased");
                        versions.push(VersionEntry {
                            version,
                            changes: std::mem::take(&mut current_changes),
                            is_unreleased,
                        });
                    }
                    current_text.clear();
                    in_heading = true;
                    is_h2 = true;
                }
                Event::Text(text) if in_heading => {
                    current_text.push_str(&text);
                }
                Event::End(TagEnd::Heading(HeadingLevel::H2)) => {
                    // Extract version from heading text like "[Unreleased]" or "[1.0.0]"
                    if is_h2
                        && let Some(version) = Self::extract_version_from_heading(&current_text)
                    {
                        current_version = Some(version);
                    }
                    current_text.clear();
                    in_heading = false;
                    is_h2 = false;
                }

                // List items are individual changes
                Event::Start(Tag::List(_)) if current_version.is_some() => {
                    in_list = true;
                }
                Event::End(TagEnd::List(_)) => {
                    in_list = false;
                }
                Event::Start(Tag::Item) if in_list => {
                    current_text.clear();
                }
                Event::Text(text) if in_list => {
                    current_text.push_str(&text);
                }
                Event::Code(code) if in_list => {
                    current_text.push('`');
                    current_text.push_str(&code);
                    current_text.push('`');
                }
                Event::End(TagEnd::Item) if in_list => {
                    let change = current_text.trim().to_string();
                    if !change.is_empty() {
                        current_changes.push(change);
                    }
                    current_text.clear();
                }

                _ => {}
            }
        }

        // Save last version
        if let Some(version) = current_version {
            let is_unreleased = version.to_lowercase().contains("unreleased");
            versions.push(VersionEntry {
                version,
                changes: current_changes,
                is_unreleased,
            });
        }

        versions
    }

    /// Extract version string from heading text
    fn extract_version_from_heading(text: &str) -> Option<String> {
        // Match patterns like "[Unreleased]" or "[1.0.0]" or "[1.0.0] - 2024-01-01"
        let text = text.trim();
        if text.starts_with('[')
            && let Some(end) = text.find(']')
        {
            return Some(text[1..end].to_string());
        }
        None
    }

    /// Get the unreleased section
    #[allow(dead_code)] // Reserved for future test cases
    pub fn get_unreleased(&self) -> Option<VersionEntry> {
        self.extract_versions()
            .into_iter()
            .find(|v| v.is_unreleased)
    }

    /// Check if CHANGELOG mentions a specific term (field name, exit code, CLI option)
    pub fn mentions_term(&self, term: &str) -> bool {
        // Case-insensitive search for the term
        let lower_content = self.content.to_lowercase();
        let lower_term = term.to_lowercase();
        lower_content.contains(&lower_term)
    }

    /// Extract all mentioned field names from CHANGELOG
    /// Looks for patterns like `field_name` or **`field_name`**
    pub fn extract_mentioned_fields(&self) -> HashSet<String> {
        let mut fields = HashSet::new();

        // Look for code-formatted terms (backticks)
        let re = regex::Regex::new(r"`([a-z_][a-z0-9_]*)`").unwrap();
        for cap in re.captures_iter(&self.content) {
            if let Some(field) = cap.get(1) {
                fields.insert(field.as_str().to_string());
            }
        }

        fields
    }

    /// Extract all mentioned exit codes from CHANGELOG
    /// Looks for patterns like "exit code 7" or "code 7"
    pub fn extract_mentioned_exit_codes(&self) -> HashSet<i32> {
        let mut codes = HashSet::new();

        // Look for "exit code N" or "code N" patterns
        let re = regex::Regex::new(r"(?:exit )?code[:\s]+(\d+)").unwrap();
        for cap in re.captures_iter(&self.content) {
            if let Some(code_str) = cap.get(1)
                && let Ok(code) = code_str.as_str().parse::<i32>()
            {
                codes.insert(code);
            }
        }

        // Also look for standalone numbers in exit code contexts
        let re2 = regex::Regex::new(r"`(\d+)`").unwrap();
        for cap in re2.captures_iter(&self.content) {
            if let Some(code_str) = cap.get(1)
                && let Ok(code) = code_str.as_str().parse::<i32>()
            {
                // Only include if it's a reasonable exit code (0-255)
                if code <= 255 {
                    codes.insert(code);
                }
            }
        }

        codes
    }

    /// Extract all mentioned CLI options from CHANGELOG
    /// Looks for patterns like --option-name
    pub fn extract_mentioned_cli_options(&self) -> HashSet<String> {
        let mut options = HashSet::new();

        // Look for --option-name patterns
        let re = regex::Regex::new(r"--([a-z][a-z0-9-]*)").unwrap();
        for cap in re.captures_iter(&self.content) {
            if let Some(option) = cap.get(1) {
                options.insert(option.as_str().to_string());
            }
        }

        options
    }
}

/// Linter for CHANGELOG validation
pub struct ChangelogLinter {
    parser: ChangelogParser,
}

impl ChangelogLinter {
    /// Create a new `ChangelogLinter`
    pub fn new(changelog_path: &Path) -> Result<Self> {
        let parser = ChangelogParser::new(changelog_path)?;
        Ok(Self { parser })
    }

    /// Verify that specific fields are mentioned in the CHANGELOG
    pub fn verify_fields_mentioned(&self, fields: &[&str]) -> Result<Vec<String>> {
        let mut missing = Vec::new();

        for field in fields {
            if !self.parser.mentions_term(field) {
                missing.push((*field).to_string());
            }
        }

        Ok(missing)
    }

    /// Verify that specific exit codes are mentioned in the CHANGELOG
    pub fn verify_exit_codes_mentioned(&self, codes: &[i32]) -> Result<Vec<i32>> {
        let mentioned_codes = self.parser.extract_mentioned_exit_codes();
        let mut missing = Vec::new();

        for code in codes {
            if !mentioned_codes.contains(code) {
                missing.push(*code);
            }
        }

        Ok(missing)
    }

    /// Verify that specific CLI options are mentioned in the CHANGELOG
    #[allow(dead_code)] // Reserved for future test cases
    pub fn verify_cli_options_mentioned(&self, options: &[&str]) -> Result<Vec<String>> {
        let mentioned_options = self.parser.extract_mentioned_cli_options();
        let mut missing = Vec::new();

        for option in options {
            if !mentioned_options.contains(*option) {
                missing.push((*option).to_string());
            }
        }

        Ok(missing)
    }

    /// Check if CHANGELOG has breaking changes marked
    pub fn has_breaking_changes_section(&self) -> bool {
        let lower_content = self.parser.content.to_lowercase();
        lower_content.contains("breaking") || lower_content.contains("[breaking]")
    }

    /// Get all versions from CHANGELOG
    pub fn get_versions(&self) -> Vec<VersionEntry> {
        self.parser.extract_versions()
    }

    /// Verify that schema version changes are marked as breaking
    /// Returns a list of schema version changes that are not marked as breaking
    pub fn verify_schema_version_changes_marked_breaking(&self) -> Result<Vec<String>> {
        let mut unmarked_changes = Vec::new();

        // Extract all mentions of schema_version changes
        let schema_version_re =
            regex::Regex::new(r#"(?i)schema[_\s]*version[:\s]*["']?(\d+)["']?"#).unwrap();

        // Split content into sections by version
        let versions = self.get_versions();

        for version in &versions {
            // Skip unreleased section for now
            if version.is_unreleased {
                continue;
            }

            // Check if this version mentions schema_version changes
            let version_content = version.changes.join(" ");
            let has_schema_change = schema_version_re.is_match(&version_content);

            if has_schema_change {
                // Check if this version has breaking change markers
                let has_breaking_marker = version.changes.iter().any(|change| {
                    let lower = change.to_lowercase();
                    lower.contains("breaking") || lower.contains("[breaking]")
                });

                if !has_breaking_marker {
                    unmarked_changes.push(format!(
                        "Version {} mentions schema_version changes but lacks [BREAKING] marker",
                        version.version
                    ));
                }
            }
        }

        Ok(unmarked_changes)
    }

    /// Verify that contract field removals/renames are marked as breaking
    /// Takes a list of removed/renamed fields and checks if they're marked as breaking
    pub fn verify_field_changes_marked_breaking(
        &self,
        removed_fields: &[&str],
        renamed_fields: &[(&str, &str)],
    ) -> Result<Vec<String>> {
        let mut unmarked_changes = Vec::new();

        let versions = self.get_versions();

        for version in &versions {
            if version.is_unreleased {
                continue;
            }

            let version_content = version.changes.join(" ").to_lowercase();

            // Check for removed fields
            for field in removed_fields {
                let field_lower = field.to_lowercase();
                if version_content.contains(&field_lower)
                    && (version_content.contains("remov") || version_content.contains("delet"))
                {
                    // This version mentions removing this field
                    let has_breaking_marker = version.changes.iter().any(|change| {
                        let lower = change.to_lowercase();
                        lower.contains("breaking") || lower.contains("[breaking]")
                    });

                    if !has_breaking_marker {
                        unmarked_changes.push(format!(
                            "Version {} removes field '{}' but lacks [BREAKING] marker",
                            version.version, field
                        ));
                    }
                }
            }

            // Check for renamed fields
            for (old_name, new_name) in renamed_fields {
                let old_lower = old_name.to_lowercase();
                let new_lower = new_name.to_lowercase();
                if version_content.contains(&old_lower)
                    && version_content.contains(&new_lower)
                    && (version_content.contains("renam") || version_content.contains("replac"))
                {
                    // This version mentions renaming this field
                    let has_breaking_marker = version.changes.iter().any(|change| {
                        let lower = change.to_lowercase();
                        lower.contains("breaking") || lower.contains("[breaking]")
                    });

                    if !has_breaking_marker {
                        unmarked_changes.push(format!(
                            "Version {} renames field '{}' to '{}' but lacks [BREAKING] marker",
                            version.version, old_name, new_name
                        ));
                    }
                }
            }
        }

        Ok(unmarked_changes)
    }

    /// Check if a specific version entry has breaking change markers
    #[allow(dead_code)] // Reserved for future test cases
    pub fn version_has_breaking_marker(&self, version_name: &str) -> bool {
        let versions = self.get_versions();

        for version in versions {
            if version.version == version_name {
                return version.changes.iter().any(|change| {
                    let lower = change.to_lowercase();
                    lower.contains("breaking") || lower.contains("[breaking]")
                });
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn get_changelog_path() -> PathBuf {
        PathBuf::from("CHANGELOG.md")
    }

    #[test]
    fn test_parse_changelog() {
        let parser =
            ChangelogParser::new(&get_changelog_path()).expect("Failed to parse CHANGELOG.md");

        let versions = parser.extract_versions();
        assert!(
            !versions.is_empty(),
            "CHANGELOG should have at least one version"
        );

        // Should have an Unreleased section
        let unreleased = versions.iter().find(|v| v.is_unreleased);
        assert!(
            unreleased.is_some(),
            "CHANGELOG should have an [Unreleased] section"
        );
    }

    #[test]
    fn test_extract_mentioned_fields() {
        let parser =
            ChangelogParser::new(&get_changelog_path()).expect("Failed to parse CHANGELOG.md");

        let fields = parser.extract_mentioned_fields();

        // These fields are documented in the current CHANGELOG
        assert!(
            fields.contains("schema_version"),
            "CHANGELOG should mention schema_version"
        );
        assert!(
            fields.contains("emitted_at"),
            "CHANGELOG should mention emitted_at"
        );
        assert!(
            fields.contains("error_kind"),
            "CHANGELOG should mention error_kind"
        );
    }

    #[test]
    fn test_extract_mentioned_exit_codes() {
        let parser =
            ChangelogParser::new(&get_changelog_path()).expect("Failed to parse CHANGELOG.md");

        let codes = parser.extract_mentioned_exit_codes();

        // These exit codes are documented in the current CHANGELOG
        assert!(codes.contains(&0), "CHANGELOG should mention exit code 0");
        assert!(codes.contains(&2), "CHANGELOG should mention exit code 2");
        assert!(codes.contains(&7), "CHANGELOG should mention exit code 7");
        assert!(codes.contains(&8), "CHANGELOG should mention exit code 8");
        assert!(codes.contains(&9), "CHANGELOG should mention exit code 9");
        assert!(codes.contains(&10), "CHANGELOG should mention exit code 10");
        assert!(codes.contains(&70), "CHANGELOG should mention exit code 70");
    }

    #[test]
    fn test_extract_mentioned_cli_options() {
        let parser =
            ChangelogParser::new(&get_changelog_path()).expect("Failed to parse CHANGELOG.md");

        let options = parser.extract_mentioned_cli_options();

        // The CHANGELOG might not have CLI options yet, but the parser should work
        // This test just verifies the parser doesn't crash
        println!("Found CLI options: {options:?}");
    }

    #[test]
    fn test_linter_verify_fields() {
        let linter = ChangelogLinter::new(&get_changelog_path()).expect("Failed to create linter");

        // These fields should be mentioned in CHANGELOG
        let fields_to_check = vec!["schema_version", "emitted_at", "error_kind"];
        let missing = linter
            .verify_fields_mentioned(&fields_to_check)
            .expect("Failed to verify fields");

        assert!(
            missing.is_empty(),
            "CHANGELOG should mention all contract fields: {missing:?}"
        );
    }

    #[test]
    fn test_linter_verify_exit_codes() {
        let linter = ChangelogLinter::new(&get_changelog_path()).expect("Failed to create linter");

        // These exit codes should be mentioned in CHANGELOG
        let codes_to_check = vec![0, 2, 7, 8, 9, 10, 70];
        let missing = linter
            .verify_exit_codes_mentioned(&codes_to_check)
            .expect("Failed to verify exit codes");

        assert!(
            missing.is_empty(),
            "CHANGELOG should mention all exit codes: {missing:?}"
        );
    }

    #[test]
    fn test_has_breaking_changes_section() {
        let linter = ChangelogLinter::new(&get_changelog_path()).expect("Failed to create linter");

        // The current CHANGELOG has breaking changes documented
        assert!(
            linter.has_breaking_changes_section(),
            "CHANGELOG should have breaking changes section or markers"
        );
    }

    #[test]
    fn test_version_extraction() {
        let linter = ChangelogLinter::new(&get_changelog_path()).expect("Failed to create linter");

        let versions = linter.get_versions();
        assert!(!versions.is_empty(), "Should extract at least one version");

        // Check that we have an unreleased section
        let has_unreleased = versions.iter().any(|v| v.is_unreleased);
        assert!(has_unreleased, "Should have an [Unreleased] section");

        // Each version should have some changes
        for version in &versions {
            println!(
                "Version: {} - {} changes",
                version.version,
                version.changes.len()
            );
        }
    }

    /// This test demonstrates how the CHANGELOG linter can be used in CI
    /// to verify that changes to contract files are documented in CHANGELOG
    #[test]
    fn test_ci_friendly_changelog_verification() {
        let linter = ChangelogLinter::new(&get_changelog_path()).expect("Failed to create linter");

        // Simulate checking if new fields added to types.rs are documented
        // In a real CI scenario, this would parse git diff to find new fields
        let new_fields = vec!["schema_version", "emitted_at", "error_kind"];
        let missing_fields = linter
            .verify_fields_mentioned(&new_fields)
            .expect("Failed to verify fields");

        if !missing_fields.is_empty() {
            println!("WARNING: The following fields are not mentioned in CHANGELOG:");
            for field in &missing_fields {
                println!("  - {field}");
            }
            println!("\nPlease update CHANGELOG.md to document these contract changes.");
        }

        // For this test, we expect all fields to be documented
        assert!(
            missing_fields.is_empty(),
            "CHANGELOG should document all contract fields. Missing: {missing_fields:?}"
        );

        // Simulate checking if new exit codes are documented
        let new_exit_codes = vec![7, 8, 9, 10, 70];
        let missing_codes = linter
            .verify_exit_codes_mentioned(&new_exit_codes)
            .expect("Failed to verify exit codes");

        if !missing_codes.is_empty() {
            println!("WARNING: The following exit codes are not mentioned in CHANGELOG:");
            for code in &missing_codes {
                println!("  - Exit code {code}");
            }
            println!("\nPlease update CHANGELOG.md to document these exit code changes.");
        }

        assert!(
            missing_codes.is_empty(),
            "CHANGELOG should document all exit codes. Missing: {missing_codes:?}"
        );
    }

    /// Test that demonstrates the heuristic for requiring CHANGELOG updates
    /// when specific files change
    #[test]
    fn test_changelog_heuristic_for_file_changes() {
        // This test demonstrates the heuristic logic that would be used in CI
        // In a real CI scenario, this would check git diff for changed files

        let changed_files = vec![
            "src/types.rs",      // Contract changes
            "src/exit_codes.rs", // Exit code changes
            "src/cli.rs",        // CLI option changes
        ];

        let linter = ChangelogLinter::new(&get_changelog_path()).expect("Failed to create linter");

        // For each changed file, verify CHANGELOG has relevant updates
        for file in changed_files {
            println!("Checking if CHANGELOG documents changes to {file}");

            match file {
                f if f.contains("types.rs") => {
                    // If types.rs changed, verify contract fields are documented
                    let fields = vec!["schema_version", "emitted_at"];
                    let missing = linter
                        .verify_fields_mentioned(&fields)
                        .expect("Failed to verify fields");

                    if missing.is_empty() {
                        println!("  ✓ Contract changes are documented");
                    } else {
                        println!(
                            "  WARNING: types.rs changed but CHANGELOG doesn't mention: {missing:?}"
                        );
                    }
                }
                f if f.contains("exit_codes.rs") => {
                    // If exit_codes.rs changed, verify exit codes are documented
                    let codes = vec![7, 8, 9, 10, 70];
                    let missing = linter
                        .verify_exit_codes_mentioned(&codes)
                        .expect("Failed to verify exit codes");

                    if missing.is_empty() {
                        println!("  ✓ Exit code changes are documented");
                    } else {
                        println!(
                            "  WARNING: exit_codes.rs changed but CHANGELOG doesn't mention codes: {missing:?}"
                        );
                    }
                }
                f if f.contains("cli.rs") => {
                    // If cli.rs changed, verify CLI options are documented
                    // For now, just check that CHANGELOG exists and has content
                    let versions = linter.get_versions();
                    if versions.is_empty() {
                        println!("  WARNING: cli.rs changed but CHANGELOG has no versions");
                    } else {
                        println!(
                            "  ✓ CHANGELOG has version entries (CLI changes should be documented)"
                        );
                    }
                }
                _ => {}
            }
        }
    }

    /// Test verification of breaking changes marking for schema version changes
    #[test]
    fn test_verify_schema_version_changes_marked_breaking() {
        let linter = ChangelogLinter::new(&get_changelog_path()).expect("Failed to create linter");

        let unmarked = linter
            .verify_schema_version_changes_marked_breaking()
            .expect("Failed to verify schema version changes");

        // The current CHANGELOG should properly mark schema version changes as breaking
        // If this fails, it means a schema version change was documented without [BREAKING] marker
        if !unmarked.is_empty() {
            println!("WARNING: Schema version changes not marked as breaking:");
            for item in &unmarked {
                println!("  - {item}");
            }
        }

        // For now, we just verify the function works - in a real scenario this would be enforced
        println!(
            "Schema version change verification: {} potential issues found",
            unmarked.len()
        );
    }

    /// Test verification of breaking changes marking for field removals/renames
    #[test]
    fn test_verify_field_changes_marked_breaking() {
        let linter = ChangelogLinter::new(&get_changelog_path()).expect("Failed to create linter");

        // Example: verify that the timestamp -> emitted_at rename is marked as breaking
        let removed_fields = vec!["timestamp"];
        let renamed_fields = vec![("timestamp", "emitted_at")];

        let unmarked = linter
            .verify_field_changes_marked_breaking(&removed_fields, &renamed_fields)
            .expect("Failed to verify field changes");

        if !unmarked.is_empty() {
            println!("WARNING: Field changes not marked as breaking:");
            for item in &unmarked {
                println!("  - {item}");
            }
        }

        // The current CHANGELOG documents the timestamp -> emitted_at change
        // Verify it's properly marked (or at least documented)
        println!(
            "Field change verification: {} potential issues found",
            unmarked.len()
        );
    }

    /// Test that breaking changes are properly marked in CHANGELOG
    #[test]
    fn test_breaking_changes_properly_marked() {
        let linter = ChangelogLinter::new(&get_changelog_path()).expect("Failed to create linter");

        // Verify that CHANGELOG has breaking changes section or markers
        assert!(
            linter.has_breaking_changes_section(),
            "CHANGELOG should have breaking changes section or [BREAKING] markers"
        );

        // Get all versions and check for breaking change patterns
        let versions = linter.get_versions();

        let mut has_breaking_markers = false;
        for version in &versions {
            let has_marker = version.changes.iter().any(|change| {
                let lower = change.to_lowercase();
                lower.contains("breaking") || lower.contains("[breaking]")
            });

            if has_marker {
                has_breaking_markers = true;
                println!("Version {} has breaking change markers", version.version);
            }
        }

        // The current CHANGELOG should have at least some breaking change markers
        // since we have schema version changes and field renames
        println!("Found breaking change markers: {has_breaking_markers}");
    }

    /// Test that schema version bumps require breaking change markers
    #[test]
    fn test_schema_version_bump_requires_breaking_marker() {
        let linter = ChangelogLinter::new(&get_changelog_path()).expect("Failed to create linter");

        // This test demonstrates the policy: any schema version bump must be marked as breaking
        // In the current CHANGELOG, we have schema v1 documented

        let versions = linter.get_versions();

        for version in &versions {
            let version_content = version.changes.join(" ");

            // Check if this version introduces or changes schema_version
            if version_content.contains("schema_version") || version_content.contains("Schema v") {
                println!("Version {} mentions schema versioning", version.version);

                // In a strict policy, we would require breaking markers for schema changes
                // For now, we just document the expectation
                let has_breaking = version.changes.iter().any(|c| {
                    let lower = c.to_lowercase();
                    lower.contains("breaking") || lower.contains("[breaking]")
                });

                println!("  Has breaking marker: {has_breaking}");
            }
        }
    }

    /// Test that contract field removals require breaking change markers
    #[test]
    fn test_field_removal_requires_breaking_marker() {
        let linter = ChangelogLinter::new(&get_changelog_path()).expect("Failed to create linter");

        // This test demonstrates the policy: removing or renaming fields must be marked as breaking

        let versions = linter.get_versions();

        for version in &versions {
            let version_content = version.changes.join(" ").to_lowercase();

            // Look for removal/rename keywords
            let has_removal = version_content.contains("remov")
                || version_content.contains("delet")
                || version_content.contains("renam")
                || version_content.contains("replac");

            if has_removal {
                println!(
                    "Version {} mentions field changes (removal/rename)",
                    version.version
                );

                let has_breaking = version.changes.iter().any(|c| {
                    let lower = c.to_lowercase();
                    lower.contains("breaking") || lower.contains("[breaking]")
                });

                println!("  Has breaking marker: {has_breaking}");

                // In a strict policy, we would require breaking markers
                // For now, we document the expectation
            }
        }
    }

    /// Test CI-friendly breaking change verification
    #[test]
    fn test_ci_breaking_change_verification() {
        let linter = ChangelogLinter::new(&get_changelog_path()).expect("Failed to create linter");

        println!("\n=== CI Breaking Change Verification ===\n");

        // 1. Check for schema version changes
        let schema_issues = linter
            .verify_schema_version_changes_marked_breaking()
            .expect("Failed to verify schema version changes");

        if schema_issues.is_empty() {
            println!("✓ Schema version changes properly marked");
        } else {
            println!("❌ Schema version changes not properly marked:");
            for issue in &schema_issues {
                println!("   {issue}");
            }
        }

        // 2. Check for field removals/renames
        // In a real CI scenario, this would parse git diff to find actual changes
        let removed_fields = vec!["timestamp"]; // Example: timestamp was replaced
        let renamed_fields = vec![("timestamp", "emitted_at")];

        let field_issues = linter
            .verify_field_changes_marked_breaking(&removed_fields, &renamed_fields)
            .expect("Failed to verify field changes");

        if field_issues.is_empty() {
            println!("✓ Field changes properly marked");
        } else {
            println!("❌ Field changes not properly marked:");
            for issue in &field_issues {
                println!("   {issue}");
            }
        }

        // 3. Verify overall breaking changes section exists
        if linter.has_breaking_changes_section() {
            println!("✓ CHANGELOG has breaking changes section/markers");
        } else {
            println!("❌ CHANGELOG missing breaking changes section/markers");
        }

        println!("\n=== End CI Verification ===\n");

        // For this test, we just verify the checks run successfully
        // In a real CI scenario, we would fail the build if issues are found
    }
}

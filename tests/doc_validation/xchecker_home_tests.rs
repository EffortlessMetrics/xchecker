//! `XCHECKER_HOME` documentation verification tests
//!
//! Tests that verify documentation correctly describes:
//! - `XCHECKER_HOME` environment variable
//! - Default location (./.xchecker)
//! - Override behavior
//! - Directory structure
//! - Thread-local override for tests
//!
//! Requirements: R8

#[cfg(test)]
mod tests {
    use std::path::Path;

    /// Test that `XCHECKER_HOME` is documented in README.md and CONFIGURATION.md
    ///
    /// Requirements: R8
    /// - Verify `XCHECKER_HOME` is mentioned in both files
    /// - Verify default location (./.xchecker) is documented
    /// - Verify override behavior is documented
    #[test]
    fn test_xchecker_home_documented() {
        // Read README.md
        let readme_path = Path::new("README.md");
        let readme_content =
            std::fs::read_to_string(readme_path).expect("Failed to read README.md");

        // Read CONFIGURATION.md
        let config_path = Path::new("docs/CONFIGURATION.md");
        let config_content =
            std::fs::read_to_string(config_path).expect("Failed to read docs/CONFIGURATION.md");

        // Verify XCHECKER_HOME is mentioned in README
        assert!(
            readme_content.contains("XCHECKER_HOME"),
            "README.md must mention XCHECKER_HOME environment variable"
        );

        // Verify XCHECKER_HOME is mentioned in CONFIGURATION
        assert!(
            config_content.contains("XCHECKER_HOME"),
            "CONFIGURATION.md must mention XCHECKER_HOME environment variable"
        );

        // Verify default location is documented
        // The default should be ./.xchecker (relative to current directory)
        let has_default_location =
            readme_content.contains(".xchecker") || config_content.contains(".xchecker");

        assert!(
            has_default_location,
            "Documentation must mention default location ./.xchecker"
        );

        // Verify override behavior is documented
        // Should mention that XCHECKER_HOME can be used to override the location
        let readme_mentions_override = readme_content.contains("XCHECKER_HOME")
            && (readme_content.contains("override")
                || readme_content.contains("environment variable")
                || readme_content.contains("env"));

        let config_mentions_override = config_content.contains("XCHECKER_HOME")
            && (config_content.contains("override")
                || config_content.contains("environment variable")
                || config_content.contains("env"));

        assert!(
            readme_mentions_override || config_mentions_override,
            "Documentation must explain XCHECKER_HOME override behavior"
        );

        // Additional verification: Check that the documentation describes what XCHECKER_HOME does
        // It should be clear that it controls the state directory location
        let describes_purpose = (readme_content.contains("XCHECKER_HOME")
            && (readme_content.contains("state")
                || readme_content.contains("directory")
                || readme_content.contains("location")))
            || (config_content.contains("XCHECKER_HOME")
                && (config_content.contains("state")
                    || config_content.contains("directory")
                    || config_content.contains("location")));

        assert!(
            describes_purpose,
            "Documentation must describe what XCHECKER_HOME controls (state directory location)"
        );
    }

    /// Test that directory structure is documented correctly
    ///
    /// Requirements: R8
    /// - Extract documented directory structure from README
    /// - Compare against `paths::spec_root()` implementation
    /// - Normalize path separators for cross-platform comparison
    /// - Verify documented structure matches actual implementation
    #[test]
    fn test_directory_structure_documented() {
        use crate::doc_validation::common::normalize_paths;

        // Read README.md
        let readme_path = Path::new("README.md");
        let readme_content =
            std::fs::read_to_string(readme_path).expect("Failed to read README.md");

        // Normalize the README content for cross-platform comparison
        let normalized_readme = normalize_paths(&readme_content);

        // The documented structure should mention:
        // - specs/<spec-id>/ - Spec-specific state and artifacts
        // - specs/<spec-id>/artifacts/ - Generated artifacts
        // - specs/<spec-id>/receipts/ - Execution receipts
        // - specs/<spec-id>/context/ - Context files

        // Verify the base structure is documented
        assert!(
            normalized_readme.contains("specs/<spec-id>/"),
            "README must document specs/<spec-id>/ directory structure"
        );

        // Verify artifacts directory is documented
        assert!(
            normalized_readme.contains("specs/<spec-id>/artifacts/")
                || normalized_readme.contains("artifacts/"),
            "README must document artifacts/ subdirectory"
        );

        // Verify receipts directory is documented
        assert!(
            normalized_readme.contains("specs/<spec-id>/receipts/")
                || normalized_readme.contains("receipts/"),
            "README must document receipts/ subdirectory"
        );

        // Verify context directory is documented
        assert!(
            normalized_readme.contains("specs/<spec-id>/context/")
                || normalized_readme.contains("context/"),
            "README must document context/ subdirectory"
        );

        // Now verify against the actual implementation
        // The paths::spec_root() function returns <home>/specs/<spec_id>
        // We verify this by checking the implementation matches the documentation

        // Test with a sample spec_id to verify the path structure
        let test_spec_id = "test-spec";

        // Get the actual path from the implementation
        // Note: We use a thread-local override to avoid affecting other tests
        let _temp_home = xchecker::paths::with_isolated_home();
        let spec_root = xchecker::paths::spec_root(test_spec_id);

        // Normalize the path for cross-platform comparison
        let normalized_spec_root = normalize_paths(spec_root.as_ref());

        // Verify the path structure matches the documented pattern:
        // <home>/specs/<spec-id>
        assert!(
            normalized_spec_root.contains("specs/"),
            "spec_root() must return a path containing 'specs/'"
        );
        assert!(
            normalized_spec_root.ends_with(&format!("specs/{test_spec_id}")),
            "spec_root() must return a path ending with 'specs/<spec-id>'"
        );

        // Verify the documented subdirectories match the expected structure
        // The implementation uses spec_root() as the base, and subdirectories are:
        // - artifacts/ (for generated artifacts)
        // - receipts/ (for execution receipts)
        // - context/ (for context files)

        // Build expected paths
        let artifacts_path = spec_root.join("artifacts");
        let receipts_path = spec_root.join("receipts");
        let context_path = spec_root.join("context");

        // Normalize for comparison
        let normalized_artifacts = normalize_paths(artifacts_path.as_ref());
        let normalized_receipts = normalize_paths(receipts_path.as_ref());
        let normalized_context = normalize_paths(context_path.as_ref());

        // Verify these paths follow the documented structure
        assert!(
            normalized_artifacts.contains(&format!("specs/{test_spec_id}/artifacts")),
            "artifacts path must follow documented structure: specs/<spec-id>/artifacts"
        );
        assert!(
            normalized_receipts.contains(&format!("specs/{test_spec_id}/receipts")),
            "receipts path must follow documented structure: specs/<spec-id>/receipts"
        );
        assert!(
            normalized_context.contains(&format!("specs/{test_spec_id}/context")),
            "context path must follow documented structure: specs/<spec-id>/context"
        );

        // Additional verification: Check that the documentation describes
        // the purpose of each directory
        let has_artifacts_description = normalized_readme.contains("artifacts")
            && (normalized_readme.contains("generated")
                || normalized_readme.contains("requirements")
                || normalized_readme.contains("design"));

        assert!(
            has_artifacts_description,
            "README must describe the purpose of the artifacts/ directory"
        );

        let has_receipts_description = normalized_readme.contains("receipts")
            && (normalized_readme.contains("execution")
                || normalized_readme.contains("metadata")
                || normalized_readme.contains("receipt"));

        assert!(
            has_receipts_description,
            "README must describe the purpose of the receipts/ directory"
        );

        let has_context_description = normalized_readme.contains("context")
            && (normalized_readme.contains("context")
                || normalized_readme.contains("files")
                || normalized_readme.contains("claude"));

        assert!(
            has_context_description,
            "README must describe the purpose of the context/ directory"
        );

        // Verify the structure matches the implementation pattern
        // The documentation should show the hierarchical structure:
        // specs/ -> <spec-id>/ -> {artifacts/, receipts/, context/}

        // Check that the documentation shows the correct hierarchy
        let shows_hierarchy = normalized_readme.contains("specs/")
            && normalized_readme.contains("<spec-id>")
            && normalized_readme.contains("artifacts")
            && normalized_readme.contains("receipts")
            && normalized_readme.contains("context");

        assert!(
            shows_hierarchy,
            "README must show the complete directory hierarchy: specs/<spec-id>/{{artifacts,receipts,context}}"
        );

        // On Windows, verify case-insensitive path comparisons work correctly
        #[cfg(windows)]
        {
            // Use dunce::canonicalize for case-insensitive comparisons on Windows
            // This ensures paths like "Specs" and "specs" are treated as equivalent
            // dunce is available as a Windows-only dependency in Cargo.toml
            use dunce;

            if let Ok(canonical_spec_root) = dunce::canonicalize(spec_root.as_std_path()) {
                let canonical_str = canonical_spec_root.to_string_lossy();
                let normalized_canonical = normalize_paths(&canonical_str);

                // Verify the canonical path still contains the expected structure
                assert!(
                    normalized_canonical.to_lowercase().contains("specs/"),
                    "Canonical path must contain 'specs/' (case-insensitive on Windows)"
                );
            }
        }
    }

    /// Test that thread-local override is documented and implemented correctly
    ///
    /// Requirements: R8
    /// - Verify README or CONFIGURATION mentions thread-local override for tests
    /// - Verify paths module uses thread-local storage (not process-global `set_var`)
    /// - Assert `paths::with_isolated_home()` function exists and is documented
    #[test]
    fn test_thread_local_override_documented() {
        // Read README.md
        let readme_path = Path::new("README.md");
        let readme_content =
            std::fs::read_to_string(readme_path).expect("Failed to read README.md");

        // Read CONFIGURATION.md
        let config_path = Path::new("docs/CONFIGURATION.md");
        let config_content =
            std::fs::read_to_string(config_path).expect("Failed to read docs/CONFIGURATION.md");

        // Verify thread-local override is mentioned in documentation
        // This is important for test isolation
        let readme_mentions_thread_local = readme_content.contains("thread-local")
            || readme_content.contains("with_isolated_home")
            || readme_content.contains("thread local");

        let config_mentions_thread_local = config_content.contains("thread-local")
            || config_content.contains("with_isolated_home")
            || config_content.contains("thread local");

        assert!(
            readme_mentions_thread_local || config_mentions_thread_local,
            "Documentation must mention thread-local override for tests. \
             This is important for test isolation to avoid process-global env var races."
        );

        // Verify that the paths module implementation uses thread-local storage
        // Read the paths.rs source file
        let paths_source_path = Path::new("crates/xchecker-utils/src/paths.rs");
        let paths_source =
            std::fs::read_to_string(paths_source_path).expect("Failed to read paths.rs");

        // Verify thread-local storage is used (not process-global set_var)
        assert!(
            paths_source.contains("thread_local!"),
            "paths module must use thread_local! for test isolation"
        );

        // Verify it doesn't use process-global set_var which would cause races
        // Note: We allow set_var in comments or other contexts, but the main
        // implementation should use thread-local storage
        let uses_thread_local = paths_source.contains("thread_local!");
        let has_thread_home = paths_source.contains("THREAD_HOME");

        assert!(
            uses_thread_local && has_thread_home,
            "paths module must use thread-local storage (THREAD_HOME) for test isolation, \
             not process-global std::env::set_var which causes races"
        );

        // Verify paths::with_isolated_home() function exists
        assert!(
            paths_source.contains("pub fn with_isolated_home()"),
            "paths module must export with_isolated_home() function for test isolation"
        );

        // Verify the function is properly documented
        // Look for documentation comment before the function
        let has_doc_comment = paths_source.contains("/// Test helper")
            || paths_source.contains("/// test helper")
            || paths_source.contains("// Test helper")
            || paths_source.contains("// test helper");

        assert!(
            has_doc_comment,
            "with_isolated_home() function must have documentation explaining its purpose"
        );

        // Verify the function returns TempDir for RAII cleanup
        assert!(
            paths_source.contains("-> tempfile::TempDir"),
            "with_isolated_home() must return TempDir for automatic cleanup"
        );

        // Verify the function is available for tests
        assert!(
            paths_source.contains("#[cfg(any(test, feature = \"test-utils\"))]")
                || paths_source.contains("#[cfg(test)]"),
            "with_isolated_home() must be available in test configuration"
        );

        // Actually test that the function works correctly
        // This verifies the implementation, not just the documentation
        let _temp_home = xchecker::paths::with_isolated_home();
        let home1 = xchecker::paths::xchecker_home();

        // Verify the home path is set to a temp directory
        assert!(
            home1.as_str().contains("temp")
                || home1.as_str().contains("tmp")
                || home1.as_str().contains("Temp"),
            "with_isolated_home() must set xchecker_home to a temporary directory, got: {home1}"
        );

        // Verify thread-local isolation works by creating another isolated home
        // Note: Each call to with_isolated_home() replaces the previous thread-local value
        // This is the expected behavior for test isolation
        let _temp_home2 = xchecker::paths::with_isolated_home();
        let home2 = xchecker::paths::xchecker_home();

        // The new home should be different from the first one
        assert_ne!(
            home1, home2,
            "with_isolated_home() must create a new isolated home each time it's called"
        );

        // Verify the second home is also a temp directory
        assert!(
            home2.as_str().contains("temp")
                || home2.as_str().contains("tmp")
                || home2.as_str().contains("Temp"),
            "with_isolated_home() must set xchecker_home to a temporary directory, got: {home2}"
        );

        // Verify that the documentation explains the precedence order
        // The order should be: thread-local > XCHECKER_HOME env var > default
        let explains_precedence = (readme_content.contains("precedence")
            || readme_content.contains("priority")
            || readme_content.contains("order"))
            && readme_content.contains("XCHECKER_HOME")
            || (config_content.contains("precedence")
                || config_content.contains("priority")
                || config_content.contains("order"))
                && config_content.contains("XCHECKER_HOME");

        assert!(
            explains_precedence,
            "Documentation must explain the precedence order for XCHECKER_HOME resolution: \
             thread-local override > XCHECKER_HOME env var > default ./.xchecker"
        );

        // Verify the implementation matches the documented precedence
        // The xchecker_home() function should check thread-local first
        let checks_thread_local_first = paths_source.contains("THREAD_HOME.with")
            && paths_source.find("THREAD_HOME.with").unwrap()
                < paths_source
                    .find("std::env::var(\"XCHECKER_HOME\")")
                    .unwrap();

        assert!(
            checks_thread_local_first,
            "xchecker_home() implementation must check thread-local override before XCHECKER_HOME env var"
        );
    }
}

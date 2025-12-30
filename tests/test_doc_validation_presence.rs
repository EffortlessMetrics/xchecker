//! Guard test to ensure documentation validation remains in the test matrix.
//!
//! This test verifies that key modules maintain documentation, preventing
//! documentation regression over time. It runs as part of the standard test
//! suite (not ignored) to catch missing docs early.

use xchecker::runner::CommandSpec;

/// Guard test: Verifies that doctests exist and are discoverable.
///
/// This test ensures that `cargo test --doc` finds documentation tests,
/// which means our modules maintain example code in their documentation.
/// If this test fails, it indicates documentation has been removed or
/// doc examples are missing.
#[test]
fn test_doctests_are_present() {
    // Run cargo test --doc in list mode to count available doctests
    let output = CommandSpec::new("cargo")
        .arg("test")
        .arg("--doc")
        .arg("--")
        .arg("--list")
        .cwd(env!("CARGO_MANIFEST_DIR"))
        .to_command()
        .output()
        .expect("Failed to execute cargo test --doc");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Count the number of doctest entries
    // Each doctest appears as a line in the format: "test <module>::<name> ... "
    let doctest_count = stdout
        .lines()
        .filter(|line| line.contains("test") && !line.contains("test result"))
        .count();

    assert!(
        doctest_count > 0,
        "No doctests found! Expected at least some documentation examples. \
         This guard test ensures documentation with examples exists in the codebase."
    );

    println!("✓ Found {} doctests in the codebase", doctest_count);
}

/// Guard test: Verifies that key modules have module-level documentation.
///
/// This test checks that critical modules (config, orchestrator, llm) have
/// documentation by reading the source files and validating doc comments exist.
#[test]
fn test_key_modules_have_documentation() {
    let key_modules = vec![
        ("src/config.rs", "config"),
        ("src/orchestrator/mod.rs", "orchestrator"),
        ("src/llm/mod.rs", "llm"),
    ];

    for (file_path, module_name) in key_modules {
        let full_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(file_path);

        let content = std::fs::read_to_string(&full_path)
            .unwrap_or_else(|_| panic!("Failed to read {}", file_path));

        // Check for module-level documentation (//! or /*!)
        let has_module_docs = content.contains("//!") || content.contains("/*!");

        assert!(
            has_module_docs,
            "Module '{}' (in {}) lacks module-level documentation (//! or /*!).\n\
             This guard test ensures key modules maintain documentation.",
            module_name, file_path
        );

        println!("✓ Module '{}' has documentation", module_name);
    }
}

/// Guard test: Verifies that public API items in key modules have doc comments.
///
/// This test samples key public items to ensure they maintain documentation.
/// It's a lightweight check that catches major documentation regressions.
#[test]
fn test_public_items_have_docs() {
    // Read the config module
    let config_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/config.rs");
    let config_content = std::fs::read_to_string(&config_path).expect("Failed to read config.rs");

    // Check for documentation on public structs/enums/functions
    // Look for patterns like "/// " or "/** " before "pub "
    let lines: Vec<&str> = config_content.lines().collect();
    let mut pub_items_count = 0;
    let mut documented_items_count = 0;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        // Skip if it's inside a test module or commented out
        if trimmed.starts_with("//") && !trimmed.starts_with("///") {
            continue;
        }

        // Check for public items (struct, enum, fn, trait, type, const)
        if trimmed.starts_with("pub struct")
            || trimmed.starts_with("pub enum")
            || trimmed.starts_with("pub fn")
            || trimmed.starts_with("pub trait")
            || trimmed.starts_with("pub type")
            || trimmed.starts_with("pub const")
            || trimmed.starts_with("pub async fn")
        {
            pub_items_count += 1;

            // Check if previous non-empty lines contain doc comments
            let mut has_doc = false;
            for j in (0..i).rev() {
                let prev_line = lines[j].trim();
                if prev_line.is_empty() {
                    continue;
                }
                if prev_line.starts_with("///") || prev_line.starts_with("/**") {
                    has_doc = true;
                    break;
                }
                // If we hit a non-doc comment or other code, stop looking
                if !prev_line.starts_with("///")
                    && !prev_line.starts_with("/**")
                    && !prev_line.starts_with("#[")
                {
                    break;
                }
                if prev_line.starts_with("#[") {
                    continue; // Skip attributes
                }
            }

            if has_doc {
                documented_items_count += 1;
            }
        }
    }

    // We expect at least some public items in config.rs to be documented
    if pub_items_count > 0 {
        let doc_percentage = (documented_items_count as f64 / pub_items_count as f64) * 100.0;

        assert!(
            documented_items_count > 0,
            "No documented public items found in config.rs! \
             This guard test ensures public APIs maintain documentation."
        );

        println!(
            "✓ config.rs: {}/{} public items documented ({:.0}%)",
            documented_items_count, pub_items_count, doc_percentage
        );
    }
}

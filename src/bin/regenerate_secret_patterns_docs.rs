//! Regenerate secret patterns documentation in docs/SECURITY.md
//!
//! This binary regenerates the "Default Secret Patterns" section in docs/SECURITY.md
//! using the canonical pattern definitions from `DEFAULT_SECRET_PATTERNS`.
//!
//! The generated content is placed between these markers:
//! - `<!-- BEGIN GENERATED:DEFAULT_SECRET_PATTERNS -->`
//! - `<!-- END GENERATED:DEFAULT_SECRET_PATTERNS -->`
//!
//! Usage: cargo run --bin regenerate_secret_patterns_docs --features dev-tools
//!
//! This ensures documentation stays in sync with the actual patterns used at runtime.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use xchecker::redaction::{default_pattern_defs, doc_gen};

const BEGIN_MARKER: &str = "<!-- BEGIN GENERATED:DEFAULT_SECRET_PATTERNS -->";
const END_MARKER: &str = "<!-- END GENERATED:DEFAULT_SECRET_PATTERNS -->";

/// Replace content between markers in a file
fn replace_generated_block(content: &str, body: &str) -> Result<String, String> {
    let begin_pos = content
        .find(BEGIN_MARKER)
        .ok_or_else(|| format!("Missing begin marker: {}", BEGIN_MARKER))?;

    let end_pos = content
        .find(END_MARKER)
        .ok_or_else(|| format!("Missing end marker: {}", END_MARKER))?;

    if end_pos < begin_pos {
        return Err("End marker appears before begin marker".to_string());
    }

    let mut out = String::new();
    out.push_str(&content[..begin_pos + BEGIN_MARKER.len()]);
    out.push('\n');
    out.push_str(body);
    out.push_str(&content[end_pos..]);

    Ok(out)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Regenerating secret patterns documentation...\n");

    let security_doc_path = Path::new("docs/SECURITY.md");
    if !security_doc_path.exists() {
        return Err(format!("File not found: {}", security_doc_path.display()).into());
    }

    // Read current content and normalize line endings for cross-platform consistency
    let current_content = doc_gen::normalize_line_endings(&fs::read_to_string(security_doc_path)?);

    // Generate new content from canonical patterns
    let patterns = default_pattern_defs();
    let generated_markdown = doc_gen::render_patterns_markdown(patterns);

    // Replace the generated block
    let new_content = replace_generated_block(&current_content, &generated_markdown)?;

    // Check if content changed
    if current_content == new_content {
        println!("✓ docs/SECURITY.md is already up-to-date");
        println!("\nPattern summary:");
    } else {
        // Write updated content
        fs::write(security_doc_path, &new_content)?;
        println!("✓ Updated docs/SECURITY.md");
        println!("\nPattern summary:");
    }

    // Print summary
    let mut by_category: BTreeMap<&str, usize> = BTreeMap::new();
    for p in patterns {
        *by_category.entry(p.category).or_default() += 1;
    }

    for (category, count) in &by_category {
        println!("  - {}: {} patterns", category, count);
    }
    println!(
        "  Total: {} patterns across {} categories",
        patterns.len(),
        by_category.len()
    );

    println!("\n✅ Secret patterns documentation is in sync with code!");

    Ok(())
}

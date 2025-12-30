//! Secret patterns documentation validation tests
//!
//! These tests ensure that docs/SECURITY.md stays in sync with the canonical
//! secret pattern definitions in `src/redaction.rs`.
//!
//! If these tests fail, run:
//!   cargo run --features dev-tools --bin regenerate_secret_patterns_docs
//!
//! Then verify with:
//!   cargo test --features dev-tools --test doc_validation -- secret_patterns
//!
//! The pattern documentation is auto-generated from `DEFAULT_SECRET_PATTERNS`
//! to prevent documentation drift.

use std::collections::BTreeMap;
use std::fs;
use xchecker::redaction::{default_pattern_defs, doc_gen};

const BEGIN_MARKER: &str = "<!-- BEGIN GENERATED:DEFAULT_SECRET_PATTERNS -->";
const END_MARKER: &str = "<!-- END GENERATED:DEFAULT_SECRET_PATTERNS -->";

/// Extract content between markers from a file
fn extract_generated_block(content: &str) -> Option<String> {
    let begin_pos = content.find(BEGIN_MARKER)?;
    let end_pos = content.find(END_MARKER)?;
    if end_pos <= begin_pos {
        return None;
    }

    let start = begin_pos + BEGIN_MARKER.len();
    // Normalize line endings for cross-platform consistency
    Some(doc_gen::normalize_line_endings(
        content[start..end_pos].trim(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_md_exists() {
        let path = "docs/SECURITY.md";
        assert!(
            std::path::Path::new(path).exists(),
            "docs/SECURITY.md must exist"
        );
    }

    #[test]
    fn test_security_md_has_generated_markers() {
        let content =
            fs::read_to_string("docs/SECURITY.md").expect("Failed to read docs/SECURITY.md");

        assert!(
            content.contains(BEGIN_MARKER),
            "docs/SECURITY.md must contain begin marker: {}\n\
             Run: cargo run --features dev-tools --bin regenerate_secret_patterns_docs",
            BEGIN_MARKER
        );

        assert!(
            content.contains(END_MARKER),
            "docs/SECURITY.md must contain end marker: {}\n\
             Run: cargo run --features dev-tools --bin regenerate_secret_patterns_docs",
            END_MARKER
        );
    }

    #[test]
    fn test_security_md_patterns_match_code() {
        let content =
            fs::read_to_string("docs/SECURITY.md").expect("Failed to read docs/SECURITY.md");

        let doc_content = extract_generated_block(&content)
            .expect("Failed to extract generated block from docs/SECURITY.md");

        // Generate expected content from canonical definitions
        let patterns = default_pattern_defs();
        let expected_content = doc_gen::render_patterns_markdown(patterns)
            .trim()
            .to_string();

        if doc_content != expected_content {
            // Provide helpful diff info
            let doc_lines: Vec<_> = doc_content.lines().collect();
            let expected_lines: Vec<_> = expected_content.lines().collect();

            let mut differences = Vec::new();
            for (i, (doc, exp)) in doc_lines.iter().zip(expected_lines.iter()).enumerate() {
                if doc != exp {
                    differences.push(format!("Line {}: \n  doc: {}\n  exp: {}", i + 1, doc, exp));
                }
            }

            if doc_lines.len() != expected_lines.len() {
                differences.push(format!(
                    "Line count mismatch: doc has {} lines, expected {}",
                    doc_lines.len(),
                    expected_lines.len()
                ));
            }

            let diff_summary = if differences.len() > 5 {
                format!("First 5 differences:\n{}", differences[..5].join("\n"))
            } else {
                differences.join("\n")
            };

            panic!(
                "docs/SECURITY.md secret patterns are out of sync with code!\n\n\
                 {}\n\n\
                 Run: cargo run --features dev-tools --bin regenerate_secret_patterns_docs\n\n\
                 This ensures documentation matches the canonical pattern definitions in src/redaction.rs",
                diff_summary
            );
        }
    }

    #[test]
    fn test_pattern_count_matches() {
        let patterns = default_pattern_defs();
        let content =
            fs::read_to_string("docs/SECURITY.md").expect("Failed to read docs/SECURITY.md");

        // Check that the documented count matches actual count
        let expected_count_text = format!("**{} default secret patterns**", patterns.len());
        assert!(
            content.contains(&expected_count_text),
            "docs/SECURITY.md should state '{}' but doesn't.\n\
             Run: cargo run --features dev-tools --bin regenerate_secret_patterns_docs",
            expected_count_text
        );
    }

    #[test]
    fn test_all_pattern_ids_documented() {
        let patterns = default_pattern_defs();
        let content =
            fs::read_to_string("docs/SECURITY.md").expect("Failed to read docs/SECURITY.md");

        for pattern in patterns {
            let pattern_marker = format!("| `{}` |", pattern.id);
            assert!(
                content.contains(&pattern_marker),
                "Pattern '{}' is not documented in docs/SECURITY.md.\n\
                 Run: cargo run --features dev-tools --bin regenerate_secret_patterns_docs",
                pattern.id
            );
        }
    }

    #[test]
    fn test_category_counts_are_accurate() {
        let patterns = default_pattern_defs();

        // Count patterns per category
        let mut by_category: BTreeMap<&str, usize> = BTreeMap::new();
        for p in patterns {
            *by_category.entry(p.category).or_default() += 1;
        }

        let content =
            fs::read_to_string("docs/SECURITY.md").expect("Failed to read docs/SECURITY.md");

        for (category, count) in by_category {
            let header = format!("#### {} ({} patterns)", category, count);
            assert!(
                content.contains(&header),
                "docs/SECURITY.md should contain '{}' but doesn't.\n\
                 Run: cargo run --features dev-tools --bin regenerate_secret_patterns_docs",
                header
            );
        }
    }

    #[test]
    fn test_default_pattern_defs_api_available() {
        // Verify the public API is accessible
        let defs = default_pattern_defs();
        assert!(
            !defs.is_empty(),
            "default_pattern_defs() should return patterns"
        );

        // Verify structure
        for def in defs {
            assert!(!def.id.is_empty(), "Pattern ID should not be empty");
            assert!(!def.category.is_empty(), "Category should not be empty");
            assert!(!def.regex.is_empty(), "Regex should not be empty");
            assert!(
                !def.description.is_empty(),
                "Description should not be empty"
            );
        }
    }

    #[test]
    fn test_all_regexes_are_valid() {
        let patterns = default_pattern_defs();

        for def in patterns {
            let result = regex::Regex::new(def.regex);
            assert!(
                result.is_ok(),
                "Pattern '{}' has invalid regex '{}': {:?}",
                def.id,
                def.regex,
                result.err()
            );
        }
    }

    #[test]
    fn test_pattern_ids_are_unique() {
        let patterns = default_pattern_defs();
        let mut seen_ids = std::collections::HashSet::new();

        for def in patterns {
            assert!(
                seen_ids.insert(def.id),
                "Duplicate pattern ID: '{}'",
                def.id
            );
        }
    }
}

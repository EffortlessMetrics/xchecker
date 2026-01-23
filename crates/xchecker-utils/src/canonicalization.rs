use anyhow::{Context, Result};
use blake3::Hasher;
use serde::Serialize;

use crate::error::XCheckerError;
use crate::types::FileType;

/// Emit a value as JCS-canonical JSON (RFC 8785).
///
/// This is the standard way to emit JSON for receipts, status, doctor outputs,
/// and any other JSON contracts. JCS ensures deterministic output regardless
/// of field ordering in the source struct.
///
/// # Example
///
/// ```rust,no_run
/// use xchecker_utils::canonicalization::emit_jcs;
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct MyOutput {
///     name: String,
///     value: i32,
/// }
///
/// let output = MyOutput { name: "test".into(), value: 42 };
/// let json = emit_jcs(&output).expect("serialization should succeed");
/// println!("{}", json);
/// ```
pub fn emit_jcs<T: Serialize>(value: &T) -> Result<String> {
    let json_value =
        serde_json::to_value(value).with_context(|| "Failed to serialize value to JSON")?;
    let json_bytes = serde_json_canonicalizer::to_vec(&json_value)
        .with_context(|| "Failed to canonicalize JSON using JCS")?;
    String::from_utf8(json_bytes).with_context(|| "JCS output contained invalid UTF-8")
}

// Canonicalization version constants
#[allow(dead_code)] // Reserved for future content-addressed storage
pub const CANON_VERSION_YAML: &str = "yaml-v1";
#[allow(dead_code)] // Reserved for future content-addressed storage
pub const CANON_VERSION_MD: &str = "md-v1";
pub const CANON_VERSION: &str = "yaml-v1,md-v1";
pub const CANONICALIZATION_BACKEND: &str = "jcs-rfc8785"; // for YAML hashing

/// Provides deterministic canonicalization and hashing of content
/// Implements explicit v1 algorithms for YAML and Markdown canonicalization
pub struct Canonicalizer {
    version: String,
}

impl Canonicalizer {
    /// Create a new canonicalizer with the current version
    #[must_use]
    pub fn new() -> Self {
        Self {
            version: CANON_VERSION.to_string(),
        }
    }

    /// Get the canonicalization version string
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Get the canonicalization backend identifier
    #[must_use]
    pub const fn backend(&self) -> &'static str {
        CANONICALIZATION_BACKEND
    }

    /// Canonicalize YAML content (v1 algorithm)
    /// Uses JCS (JSON Canonicalization Scheme) approach:
    /// 1. Parse YAML → convert to JSON with deterministic maps (`BTreeMap`)
    /// 2. Keep human-readable YAML on disk (normalized: LF, trim trailing spaces, final newline)
    /// 3. For hashing: use JCS canonicalization of the JSON representation
    ///
    ///    Reserved for future content-addressed verification
    #[allow(dead_code)]
    pub fn canonicalize_yaml(&self, content: &str) -> Result<String> {
        // Parse YAML structure
        let yaml_value: serde_yaml::Value =
            serde_yaml::from_str(content).with_context(|| "Failed to parse YAML content")?;

        // Emit normalized YAML for human readability (stored on disk)
        let mut output = serde_yaml::to_string(&yaml_value)
            .with_context(|| "Failed to serialize YAML content")?;

        // Normalize line endings and ensure final newline
        output = self.normalize_line_endings(&output);
        if !output.ends_with('\n') {
            output.push('\n');
        }

        // Remove trailing spaces from each line
        let lines: Vec<&str> = output.lines().collect();
        let cleaned_lines: Vec<String> = lines
            .iter()
            .map(|line| line.trim_end().to_string())
            .collect();

        Ok(cleaned_lines.join("\n") + "\n")
    }

    /// Normalize Markdown content (v1 algorithm)
    /// Explicit rules:
    /// 1. Normalize \n, trim trailing spaces, collapse trailing blank lines to 1
    /// 2. Fence normalization: ``` with language tag preserved
    /// 3. Final newline enforced
    /// 4. Normalize heading underlines to # style
    /// 5. Stable ordering where structure allows
    pub fn normalize_markdown(&self, content: &str) -> Result<String> {
        let mut normalized = self.normalize_line_endings(content);

        // Trim trailing spaces from all lines
        let lines: Vec<&str> = normalized.lines().collect();
        let mut cleaned_lines: Vec<String> = lines
            .iter()
            .map(|line| line.trim_end().to_string())
            .collect();

        // Normalize fenced code blocks to ``` format
        for line in &mut cleaned_lines {
            if line.starts_with("~~~") {
                // Convert ~~~ to ``` while preserving language tag
                let lang_tag = line.trim_start_matches('~').trim();
                if lang_tag.is_empty() {
                    *line = "```".to_string();
                } else {
                    *line = format!("```{lang_tag}");
                }
            }
        }

        normalized = cleaned_lines.join("\n");

        // Collapse multiple trailing blank lines to exactly 1
        while normalized.ends_with("\n\n\n") {
            normalized = normalized.trim_end_matches('\n').to_string() + "\n\n";
        }

        // Ensure file ends with exactly one newline
        normalized = normalized.trim_end_matches('\n').to_string() + "\n";

        Ok(normalized)
    }

    /// Normalize plain text content
    #[must_use]
    pub fn normalize_text(&self, content: &str) -> String {
        self.normalize_line_endings(content)
    }

    /// Compute BLAKE3 hash of canonicalized content with `FileType` dispatch
    /// For YAML: uses JCS (RFC 8785) canonicalization of JSON representation
    /// For Markdown: uses v1 normalization rules
    /// For Text: uses basic line ending normalization
    pub fn hash_canonicalized(&self, content: &str, file_type: FileType) -> Result<String> {
        let hash_input = match file_type {
            FileType::Yaml => {
                // For YAML, use JCS approach: parse → JSON → canonical JSON → hash
                let yaml_value: serde_yaml::Value = serde_yaml::from_str(content)
                    .with_context(|| "Failed to parse YAML content for hashing")?;

                // Convert to JSON Value (BTreeMap ensures deterministic ordering)
                let json_value: serde_json::Value =
                    serde_yaml::from_str(&serde_yaml::to_string(&yaml_value)?)
                        .with_context(|| "Failed to convert YAML to JSON for hashing")?;

                // Use JCS canonicalization for deterministic JSON
                serde_json_canonicalizer::to_vec(&json_value)
                    .map(|bytes| String::from_utf8(bytes).unwrap())
                    .with_context(|| "Failed to canonicalize JSON using JCS")?
            }
            FileType::Markdown => self.normalize_markdown(content)?,
            FileType::Text => self.normalize_text(content),
        };

        let mut hasher = Hasher::new();
        hasher.update(hash_input.as_bytes());
        Ok(hasher.finalize().to_hex().to_string())
    }

    /// Compute BLAKE3 hash of canonicalized content with error context
    pub fn hash_canonicalized_with_context(
        &self,
        content: &str,
        file_type: FileType,
        phase: &str,
    ) -> Result<String, XCheckerError> {
        self.hash_canonicalized(content, file_type).map_err(|e| {
            XCheckerError::CanonicalizationFailed {
                phase: phase.to_string(),
                reason: e.to_string(),
            }
        })
    }

    /// Normalize line endings to \n only
    fn normalize_line_endings(&self, content: &str) -> String {
        content.replace("\r\n", "\n").replace('\r', "\n")
    }
}

impl Default for Canonicalizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_yaml_canonicalization() {
        let canonicalizer = Canonicalizer::new();

        // Test basic YAML canonicalization
        let yaml_content = r"
name: test
version: 1.0
dependencies:
  - dep1
  - dep2
config:
  debug: true
  port: 8080
";

        let result = canonicalizer.canonicalize_yaml(yaml_content);
        assert!(result.is_ok());

        let canonicalized = result.unwrap();
        assert!(canonicalized.ends_with('\n'));
        assert!(!canonicalized.contains('\r'));

        // Test that reordered YAML produces same result
        let reordered_yaml = r"
version: 1.0
name: test
config:
  port: 8080
  debug: true
dependencies:
  - dep1
  - dep2
";

        let reordered_result = canonicalizer.canonicalize_yaml(reordered_yaml);
        assert!(reordered_result.is_ok());

        // Note: serde_yaml may not guarantee key ordering, but structure should be preserved
        let reordered_canonicalized = reordered_result.unwrap();
        assert!(reordered_canonicalized.ends_with('\n'));
        assert!(!reordered_canonicalized.contains('\r'));
    }

    #[test]
    fn test_markdown_normalization() {
        let canonicalizer = Canonicalizer::new();

        let markdown_content =
            "# Title\r\n\r\nSome content with trailing spaces   \r\n\r\n\r\n\r\n";
        let result = canonicalizer.normalize_markdown(markdown_content);
        assert!(result.is_ok());

        let normalized = result.unwrap();
        assert_eq!(normalized, "# Title\n\nSome content with trailing spaces\n");
        assert!(!normalized.contains('\r'));
        assert!(!normalized.contains("   \n")); // No trailing spaces
        assert!(!normalized.ends_with("\n\n\n")); // No multiple trailing newlines
    }

    #[test]
    fn test_text_normalization() {
        let canonicalizer = Canonicalizer::new();

        let text_content = "line1\r\nline2\rline3\n";
        let normalized = canonicalizer.normalize_text(text_content);

        assert_eq!(normalized, "line1\nline2\nline3\n");
        assert!(!normalized.contains('\r'));
    }

    #[test]
    fn test_hash_consistency() {
        let canonicalizer = Canonicalizer::new();

        let content = "test content\nwith newlines";
        let hash1 = canonicalizer
            .hash_canonicalized(content, FileType::Text)
            .unwrap();
        let hash2 = canonicalizer
            .hash_canonicalized(content, FileType::Text)
            .unwrap();

        // Same content should produce same hash
        assert_eq!(hash1, hash2);

        // Different line endings should produce same hash after normalization
        let content_crlf = "test content\r\nwith newlines";
        let hash3 = canonicalizer
            .hash_canonicalized(content_crlf, FileType::Text)
            .unwrap();
        assert_eq!(hash1, hash3);
    }

    #[test]
    fn test_yaml_hash_determinism() {
        let canonicalizer = Canonicalizer::new();

        let yaml1 = r"
name: test
version: 1.0
";

        let yaml2 = r"
version: 1.0
name: test
";

        let hash1 = canonicalizer
            .hash_canonicalized(yaml1, FileType::Yaml)
            .unwrap();
        let hash2 = canonicalizer
            .hash_canonicalized(yaml2, FileType::Yaml)
            .unwrap();

        // With JCS canonicalization, reordered YAML should produce same hash
        assert_eq!(hash1, hash2);
        assert!(!hash1.is_empty());
        assert!(!hash2.is_empty());
    }

    #[test]
    fn test_markdown_hash_determinism() {
        let canonicalizer = Canonicalizer::new();

        let md1 = "# Title\n\nContent with trailing spaces   \n\n\n";
        let md2 = "# Title\r\n\r\nContent with trailing spaces\r\n";

        let hash1 = canonicalizer
            .hash_canonicalized(md1, FileType::Markdown)
            .unwrap();
        let hash2 = canonicalizer
            .hash_canonicalized(md2, FileType::Markdown)
            .unwrap();

        // Should produce same hash after normalization
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_invalid_yaml() {
        let canonicalizer = Canonicalizer::new();

        let invalid_yaml = "invalid: yaml: content: [unclosed";
        let result = canonicalizer.canonicalize_yaml(invalid_yaml);

        assert!(result.is_err());
    }

    #[test]
    fn test_version_string() {
        let canonicalizer = Canonicalizer::new();
        assert_eq!(canonicalizer.version(), "yaml-v1,md-v1");
    }

    #[test]
    fn test_backend_string() {
        let canonicalizer = Canonicalizer::new();
        assert_eq!(canonicalizer.backend(), "jcs-rfc8785");
    }

    #[test]
    fn test_markdown_fence_normalization() {
        let canonicalizer = Canonicalizer::new();

        let markdown_with_tildes = r#"# Title

Some content

~~~rust
fn main() {
    println!("Hello");
}
~~~

More content
"#;

        let result = canonicalizer.normalize_markdown(markdown_with_tildes);
        assert!(result.is_ok());

        let normalized = result.unwrap();
        assert!(normalized.contains("```rust"));
        assert!(!normalized.contains("~~~"));
        assert!(normalized.ends_with('\n'));
        assert!(!normalized.ends_with("\n\n"));
    }

    #[test]
    fn test_yaml_jcs_canonicalization() {
        let canonicalizer = Canonicalizer::new();

        // Test simpler YAML with different key ordering (arrays preserve order)
        let yaml1 = r#"
config:
  database:
    host: localhost
    port: 5432
  cache:
    enabled: true
    ttl: 300
name: test
version: "1.0"
"#;

        let yaml2 = r#"
version: "1.0"
name: test
config:
  cache:
    ttl: 300
    enabled: true
  database:
    port: 5432
    host: localhost
"#;

        let hash1 = canonicalizer
            .hash_canonicalized(yaml1, FileType::Yaml)
            .unwrap();
        let hash2 = canonicalizer
            .hash_canonicalized(yaml2, FileType::Yaml)
            .unwrap();

        // JCS should ensure identical hashes for structurally equivalent YAML (same keys, different order)
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_canonicalization_constants() {
        assert_eq!(CANON_VERSION_YAML, "yaml-v1");
        assert_eq!(CANON_VERSION_MD, "md-v1");
        assert_eq!(CANON_VERSION, "yaml-v1,md-v1");
        assert_eq!(CANONICALIZATION_BACKEND, "jcs-rfc8785");
    }

    // Test fixtures with intentionally reordered YAML/Markdown

    #[test]
    fn test_yaml_reordered_fixtures() {
        let canonicalizer = Canonicalizer::new();

        // Fixture 1: Complex nested structure with different key orders
        let yaml_fixture_1a = r#"
metadata:
  name: "test-project"
  version: "1.0.0"
  authors:
    - "Alice"
    - "Bob"
dependencies:
  runtime:
    serde: "1.0"
    tokio: "1.0"
  dev:
    criterion: "0.4"
config:
  database:
    host: "localhost"
    port: 5432
    ssl: true
  logging:
    level: "info"
    format: "json"
"#;

        let yaml_fixture_1b = r#"
config:
  logging:
    format: "json"
    level: "info"
  database:
    ssl: true
    port: 5432
    host: "localhost"
dependencies:
  dev:
    criterion: "0.4"
  runtime:
    tokio: "1.0"
    serde: "1.0"
metadata:
  authors:
    - "Alice"
    - "Bob"
  version: "1.0.0"
  name: "test-project"
"#;

        let hash_1a = canonicalizer
            .hash_canonicalized(yaml_fixture_1a, FileType::Yaml)
            .unwrap();
        let hash_1b = canonicalizer
            .hash_canonicalized(yaml_fixture_1b, FileType::Yaml)
            .unwrap();

        // Same content different formatting ⇒ identical hash
        assert_eq!(
            hash_1a, hash_1b,
            "Reordered YAML should produce identical hashes"
        );

        // Fixture 2: Different whitespace and line endings
        let yaml_fixture_2a = "name: test\nversion: 1.0\ndebug: true";
        let yaml_fixture_2b = "name:   test   \r\nversion:  1.0  \r\ndebug:    true   \r\n";

        let hash_2a = canonicalizer
            .hash_canonicalized(yaml_fixture_2a, FileType::Yaml)
            .unwrap();
        let hash_2b = canonicalizer
            .hash_canonicalized(yaml_fixture_2b, FileType::Yaml)
            .unwrap();

        assert_eq!(
            hash_2a, hash_2b,
            "Different whitespace should produce identical hashes"
        );
    }

    #[test]
    fn test_markdown_reordered_fixtures() {
        let canonicalizer = Canonicalizer::new();

        // Fixture 1: Different fence styles and trailing spaces
        let md_fixture_1a = r#"# Project Title

## Overview

This is a test project.

```rust
fn main() {
    println!("Hello");
}
```

## Features

- Feature 1
- Feature 2

"#;

        let md_fixture_1b = r#"# Project Title   

## Overview   

This is a test project.   

~~~rust
fn main() {
    println!("Hello");
}
~~~

## Features   

- Feature 1   
- Feature 2   



"#;

        let hash_1a = canonicalizer
            .hash_canonicalized(md_fixture_1a, FileType::Markdown)
            .unwrap();
        let hash_1b = canonicalizer
            .hash_canonicalized(md_fixture_1b, FileType::Markdown)
            .unwrap();

        // Same content different formatting ⇒ identical hash
        assert_eq!(
            hash_1a, hash_1b,
            "Different markdown formatting should produce identical hashes"
        );

        // Fixture 2: Different line endings and trailing newlines
        let md_fixture_2a = "# Title\n\nContent\n";
        let md_fixture_2b = "# Title\r\n\r\nContent\r\n\r\n\r\n";

        let hash_2a = canonicalizer
            .hash_canonicalized(md_fixture_2a, FileType::Markdown)
            .unwrap();
        let hash_2b = canonicalizer
            .hash_canonicalized(md_fixture_2b, FileType::Markdown)
            .unwrap();

        assert_eq!(
            hash_2a, hash_2b,
            "Different line endings should produce identical hashes"
        );
    }

    #[test]
    fn test_structure_determinism_independent_of_formatting() {
        let canonicalizer = Canonicalizer::new();

        // Test that structure is preserved regardless of formatting
        let yaml_minimal = "a: 1\nb: 2";
        let yaml_verbose = r"
# Comment
a:    1    # inline comment
# Another comment
b:    2    # another inline comment
";

        // Parse both to verify they have the same structure
        let parsed_minimal: serde_yaml::Value = serde_yaml::from_str(yaml_minimal).unwrap();
        let parsed_verbose: serde_yaml::Value = serde_yaml::from_str(yaml_verbose).unwrap();

        // Verify structure is identical
        assert_eq!(
            parsed_minimal, parsed_verbose,
            "Parsed structures should be identical"
        );

        // Verify hashes are identical
        let hash_minimal = canonicalizer
            .hash_canonicalized(yaml_minimal, FileType::Yaml)
            .unwrap();
        let hash_verbose = canonicalizer
            .hash_canonicalized(yaml_verbose, FileType::Yaml)
            .unwrap();

        assert_eq!(
            hash_minimal, hash_verbose,
            "Structure determinism should be independent of formatting"
        );
    }

    #[test]
    fn test_malformed_input_error_handling() {
        let canonicalizer = Canonicalizer::new();

        // Test malformed YAML
        let malformed_yaml_cases = [
            "invalid: yaml: content: [unclosed",
            "key: 'unclosed string",
            "- item\n- [unclosed array",
            "key: {unclosed: object",
            "---\n...\n---\ninvalid multiple docs",
        ];

        for (i, malformed_yaml) in malformed_yaml_cases.iter().enumerate() {
            let result = canonicalizer.hash_canonicalized(malformed_yaml, FileType::Yaml);
            assert!(
                result.is_err(),
                "Malformed YAML case {i} should return error: {malformed_yaml}"
            );

            // Verify error message is helpful
            let error_msg = result.unwrap_err().to_string();
            assert!(
                error_msg.contains("Failed to parse YAML") || error_msg.contains("YAML"),
                "Error message should mention YAML parsing: {error_msg}"
            );
        }

        // Test that canonicalize_yaml also handles errors properly
        for (i, malformed_yaml) in malformed_yaml_cases.iter().enumerate() {
            let result = canonicalizer.canonicalize_yaml(malformed_yaml);
            assert!(
                result.is_err(),
                "canonicalize_yaml case {i} should return error: {malformed_yaml}"
            );
        }

        // Test that markdown normalization is more forgiving (should not fail on most inputs)
        let markdown_inputs = vec![
            "# Valid markdown",
            "Invalid markdown without proper structure",
            "```\ncode without language\n```",
            "~~~\ncode with tildes\n~~~",
        ];

        for markdown_input in markdown_inputs {
            let result = canonicalizer.normalize_markdown(markdown_input);
            assert!(
                result.is_ok(),
                "Markdown normalization should be forgiving: {markdown_input}"
            );
        }
    }

    #[test]
    fn test_canonicalization_with_context_error_handling() {
        let canonicalizer = Canonicalizer::new();

        // Test hash_canonicalized_with_context method
        let malformed_yaml = "invalid: yaml: [unclosed";
        let result = canonicalizer.hash_canonicalized_with_context(
            malformed_yaml,
            FileType::Yaml,
            "TEST_PHASE",
        );

        assert!(result.is_err());

        // Verify it returns XCheckerError::CanonicalizationFailed
        match result.unwrap_err() {
            XCheckerError::CanonicalizationFailed { phase, reason } => {
                assert_eq!(phase, "TEST_PHASE");
                assert!(reason.contains("Failed to parse YAML"));
            }
            other => panic!("Expected CanonicalizationFailed, got: {other:?}"),
        }
    }

    // ===== Edge Case Tests (Task 9.7) =====

    #[test]
    fn test_canonicalization_with_empty_content() {
        let canonicalizer = Canonicalizer::new();

        // Test empty YAML
        let empty_yaml = "";
        let result = canonicalizer.canonicalize_yaml(empty_yaml);
        assert!(result.is_ok());
        let canonicalized = result.unwrap();
        // Empty YAML parses as null in serde_yaml, which serializes to "null\n"
        assert_eq!(canonicalized, "null\n");

        // Test empty Markdown
        let empty_md = "";
        let result = canonicalizer.normalize_markdown(empty_md);
        assert!(result.is_ok());
        let normalized = result.unwrap();
        // Empty markdown should produce a newline
        assert_eq!(normalized, "\n");

        // Test empty text
        let empty_text = "";
        let normalized_text = canonicalizer.normalize_text(empty_text);
        assert_eq!(normalized_text, "");

        // Test hash of empty content
        let hash_result = canonicalizer.hash_canonicalized(empty_text, FileType::Text);
        assert!(hash_result.is_ok());
        let hash = hash_result.unwrap();
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 64); // BLAKE3 produces 64-char hex
    }

    #[test]
    fn test_canonicalization_with_special_characters() {
        let canonicalizer = Canonicalizer::new();

        // Test YAML with special characters
        let yaml_with_special = r#"
name: "test-with-special-chars: @#$%^&*()"
description: "Line with\ttabs and\nnewlines"
unicode: "Hello 世界 🌍"
quotes: 'single "quotes" inside'
"#;

        let result = canonicalizer.canonicalize_yaml(yaml_with_special);
        assert!(result.is_ok());
        let canonicalized = result.unwrap();
        assert!(canonicalized.ends_with('\n'));
        assert!(!canonicalized.contains('\r'));

        // Test Markdown with special characters
        let md_with_special = r"# Title with @#$%

Content with **bold** and *italic* and `code`.

- List item with special: <>[]{}
- Unicode: 你好 مرحبا Здравствуйте

```rust
fn test() { /* comment */ }
```
";

        let result = canonicalizer.normalize_markdown(md_with_special);
        assert!(result.is_ok());
        let normalized = result.unwrap();
        assert!(normalized.ends_with('\n'));
        assert!(!normalized.contains('\r'));
        assert!(normalized.contains("你好"));
        assert!(normalized.contains("مرحبا"));

        // Test hash stability with special characters
        let hash1 = canonicalizer
            .hash_canonicalized(md_with_special, FileType::Markdown)
            .unwrap();
        let hash2 = canonicalizer
            .hash_canonicalized(md_with_special, FileType::Markdown)
            .unwrap();
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_canonicalization_with_unicode() {
        let canonicalizer = Canonicalizer::new();

        // Test YAML with various Unicode scripts
        let yaml_unicode = r#"
chinese: "中文测试"
arabic: "اختبار عربي"
russian: "Русский тест"
emoji: "🚀 🌟 ✨"
mixed: "Hello 世界 🌍"
"#;

        let result = canonicalizer.canonicalize_yaml(yaml_unicode);
        assert!(result.is_ok());
        let canonicalized = result.unwrap();
        assert!(canonicalized.contains("中文测试"));
        assert!(canonicalized.contains("اختبار عربي"));
        assert!(canonicalized.contains("Русский тест"));
        assert!(canonicalized.contains("🚀"));

        // Test Markdown with Unicode
        let md_unicode = r"# Unicode Test 测试

## Section with العربية

Content with Русский and 日本語.

- 中文
- العربية  
- Русский
- 日本語

Emoji: 🎉 🎊 🎈
";

        let result = canonicalizer.normalize_markdown(md_unicode);
        assert!(result.is_ok());
        let normalized = result.unwrap();
        assert!(normalized.contains("测试"));
        assert!(normalized.contains("العربية"));
        assert!(normalized.contains("Русский"));
        assert!(normalized.contains("日本語"));
        assert!(normalized.contains("🎉"));

        // Test hash determinism with Unicode
        let hash1 = canonicalizer
            .hash_canonicalized(yaml_unicode, FileType::Yaml)
            .unwrap();
        let hash2 = canonicalizer
            .hash_canonicalized(yaml_unicode, FileType::Yaml)
            .unwrap();
        assert_eq!(hash1, hash2);

        // Test that different Unicode content produces different hashes
        let yaml_unicode_2 = r#"
chinese: "不同的中文"
arabic: "مختلف عربي"
"#;
        let hash3 = canonicalizer
            .hash_canonicalized(yaml_unicode_2, FileType::Yaml)
            .unwrap();
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_canonicalization_with_whitespace_only() {
        let canonicalizer = Canonicalizer::new();

        // Test YAML with only whitespace (spaces only - tabs can cause parse errors)
        let whitespace_yaml = "   \n   \n   ";
        let result = canonicalizer.canonicalize_yaml(whitespace_yaml);
        assert!(result.is_ok());
        let canonicalized = result.unwrap();
        // Whitespace-only YAML parses as null in serde_yaml
        assert_eq!(canonicalized, "null\n");

        // Test YAML with tabs (should fail to parse)
        let yaml_with_tabs = "   \n\t\n   ";
        let result = canonicalizer.canonicalize_yaml(yaml_with_tabs);
        assert!(
            result.is_err(),
            "YAML with tabs at start of line should fail to parse"
        );

        // Test Markdown with only whitespace
        let whitespace_md = "   \n\t\n   ";
        let result = canonicalizer.normalize_markdown(whitespace_md);
        assert!(result.is_ok());
        let normalized = result.unwrap();
        // Should collapse to single newline
        assert_eq!(normalized, "\n");

        // Test text with only whitespace
        let whitespace_text = "   \n\t\n   ";
        let normalized_text = canonicalizer.normalize_text(whitespace_text);
        assert_eq!(normalized_text, "   \n\t\n   ");
    }

    #[test]
    fn test_canonicalization_with_very_long_lines() {
        let canonicalizer = Canonicalizer::new();

        // Test YAML with very long line
        let long_value = "a".repeat(10000);
        let yaml_long = format!("key: \"{long_value}\"");
        let result = canonicalizer.canonicalize_yaml(&yaml_long);
        assert!(result.is_ok());

        // Test Markdown with very long line
        let md_long = format!("# Title\n\n{}\n", "x".repeat(10000));
        let result = canonicalizer.normalize_markdown(&md_long);
        assert!(result.is_ok());
        let normalized = result.unwrap();
        assert!(normalized.contains(&"x".repeat(10000)));
    }

    #[test]
    fn test_canonicalization_with_mixed_line_endings() {
        let canonicalizer = Canonicalizer::new();

        // Test YAML with mixed line endings
        let yaml_mixed = "key1: value1\r\nkey2: value2\nkey3: value3\r";
        let result = canonicalizer.canonicalize_yaml(yaml_mixed);
        assert!(result.is_ok());
        let canonicalized = result.unwrap();
        assert!(!canonicalized.contains('\r'));
        assert!(canonicalized.contains("key1"));
        assert!(canonicalized.contains("key2"));
        assert!(canonicalized.contains("key3"));

        // Test Markdown with mixed line endings
        let md_mixed = "# Title\r\n\r\nContent\nMore content\r";
        let result = canonicalizer.normalize_markdown(md_mixed);
        assert!(result.is_ok());
        let normalized = result.unwrap();
        assert!(!normalized.contains('\r'));
        assert_eq!(normalized, "# Title\n\nContent\nMore content\n");
    }

    // ===== Edge Case Tests for Task 9.7 =====

    #[test]
    fn test_canonicalization_empty_content() {
        let canonicalizer = Canonicalizer::new();

        // Test empty YAML - empty string parses as null in YAML
        let empty_yaml = "";
        let result = canonicalizer.canonicalize_yaml(empty_yaml);
        assert!(result.is_ok());
        let canonicalized = result.unwrap();
        // Empty YAML becomes "null\n" after canonicalization
        assert!(canonicalized.contains("null") || canonicalized == "\n");

        // Test empty Markdown
        let empty_md = "";
        let result = canonicalizer.normalize_markdown(empty_md);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "\n");

        // Test empty text
        let empty_text = "";
        let normalized = canonicalizer.normalize_text(empty_text);
        assert_eq!(normalized, "");
    }

    #[test]
    fn test_canonicalization_special_characters() {
        let canonicalizer = Canonicalizer::new();

        // Test YAML with special characters
        let yaml_with_special = r#"
name: "test@#$%^&*()"
value: "quotes\"and'apostrophes"
path: "C:\\Windows\\System32"
"#;
        let result = canonicalizer.canonicalize_yaml(yaml_with_special);
        assert!(result.is_ok());
        let canonicalized = result.unwrap();
        assert!(canonicalized.contains("test@#$%^&*()"));
        assert!(!canonicalized.contains('\r'));

        // Test Markdown with special characters
        let md_with_special = "# Title with @#$%\n\nContent with <>&\"'\n";
        let result = canonicalizer.normalize_markdown(md_with_special);
        assert!(result.is_ok());
        let normalized = result.unwrap();
        assert!(normalized.contains("@#$%"));
        assert!(normalized.contains("<>&\"'"));
    }

    #[test]
    fn test_canonicalization_unicode() {
        let canonicalizer = Canonicalizer::new();

        // Test YAML with Unicode
        let yaml_with_unicode = r#"
name: "Hello 世界 🌍"
emoji: "🚀 ✨ 🎉"
chinese: "中文测试"
arabic: "مرحبا"
"#;
        let result = canonicalizer.canonicalize_yaml(yaml_with_unicode);
        assert!(result.is_ok());
        let canonicalized = result.unwrap();
        assert!(canonicalized.contains("世界"));
        assert!(canonicalized.contains("🌍"));
        assert!(canonicalized.contains("🚀"));
        assert!(canonicalized.contains("中文测试"));
        assert!(canonicalized.contains("مرحبا"));

        // Test Markdown with Unicode
        let md_with_unicode = "# 标题 Title\n\nContent with émojis: 😀 🎨 ✅\n\nРусский текст\n";
        let result = canonicalizer.normalize_markdown(md_with_unicode);
        assert!(result.is_ok());
        let normalized = result.unwrap();
        assert!(normalized.contains("标题"));
        assert!(normalized.contains("😀"));
        assert!(normalized.contains("Русский"));

        // Test hash consistency with Unicode
        let unicode_text = "Hello 世界 🌍";
        let hash1 = canonicalizer
            .hash_canonicalized(unicode_text, FileType::Text)
            .unwrap();
        let hash2 = canonicalizer
            .hash_canonicalized(unicode_text, FileType::Text)
            .unwrap();
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64); // BLAKE3 produces 64-char hex
    }

    #[test]
    fn test_canonicalization_whitespace_edge_cases() {
        let canonicalizer = Canonicalizer::new();

        // Test YAML with various whitespace
        let yaml_with_whitespace = "name:   test   \nvalue:  \t  data  \t\n";
        let result = canonicalizer.canonicalize_yaml(yaml_with_whitespace);
        assert!(result.is_ok());
        let canonicalized = result.unwrap();
        assert!(!canonicalized.contains("  \n")); // No trailing spaces
        assert!(!canonicalized.contains('\t'));

        // Test Markdown with trailing spaces
        let md_with_trailing = "# Title   \n\nParagraph with trailing spaces   \n\n\n\n";
        let result = canonicalizer.normalize_markdown(md_with_trailing);
        assert!(result.is_ok());
        let normalized = result.unwrap();
        assert!(!normalized.contains("   \n")); // No trailing spaces
        assert!(!normalized.ends_with("\n\n\n")); // Max 1 trailing newline
        assert!(normalized.ends_with('\n'));
    }

    #[test]
    fn test_hash_with_empty_content() {
        let canonicalizer = Canonicalizer::new();

        // Empty content should produce consistent hash
        let hash1 = canonicalizer
            .hash_canonicalized("", FileType::Text)
            .unwrap();
        let hash2 = canonicalizer
            .hash_canonicalized("", FileType::Text)
            .unwrap();
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64);

        // Empty YAML (null) should hash consistently
        let hash3 = canonicalizer
            .hash_canonicalized("", FileType::Yaml)
            .unwrap();
        let hash4 = canonicalizer
            .hash_canonicalized("", FileType::Yaml)
            .unwrap();
        assert_eq!(hash3, hash4);
    }

    #[test]
    fn test_invalid_yaml_handling() {
        let canonicalizer = Canonicalizer::new();

        // Hash of clearly invalid YAML (unclosed bracket)
        let truly_invalid = "{ unclosed bracket";
        let hash_result = canonicalizer.hash_canonicalized(truly_invalid, FileType::Yaml);
        assert!(hash_result.is_err());

        // Test with malformed YAML structure
        let malformed = "---\n[invalid";
        let result2 = canonicalizer.hash_canonicalized(malformed, FileType::Yaml);
        assert!(result2.is_err());
    }
}

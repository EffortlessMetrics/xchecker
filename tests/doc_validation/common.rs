//! Common utilities for documentation validation tests

use anyhow::{Context, Result};
use assert_cmd::Command;
use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use tempfile::TempDir;

/// A fenced code block extracted from markdown
#[derive(Debug, Clone)]
pub struct FencedBlock {
    pub language: String,
    pub content: String,
    pub metadata: BlockMetadata,
}

/// Extractor for fenced code blocks from markdown files
pub struct FenceExtractor {
    content: String,
}

impl FenceExtractor {
    /// Create a new `FenceExtractor` from a file path
    pub fn new(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .context(format!("Failed to read file: {}", path.display()))?;
        Ok(Self { content })
    }

    /// Extract all fenced code blocks using `pulldown_cmark` AST
    /// Handles multi-line fences, backtick variations, tilde fences, and nested blocks
    pub fn extract_blocks(&self) -> Vec<FencedBlock> {
        let mut blocks = vec![];
        let parser = Parser::new(&self.content);
        let mut current_lang: Option<String> = None;
        let mut current_metadata = BlockMetadata::default();
        let mut buffer = String::new();

        for event in parser {
            match event {
                Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) => {
                    let info_str = info.to_string();
                    let mut parts = info_str.split_whitespace();
                    current_lang = parts.next().map(std::string::ToString::to_string);

                    // Parse metadata from remaining parts
                    let metadata_str = parts.collect::<Vec<_>>().join(" ");
                    current_metadata = BlockMetadata::parse(&metadata_str);
                }
                Event::Text(text) if current_lang.is_some() => {
                    buffer.push_str(&text);
                }
                Event::End(TagEnd::CodeBlock) => {
                    if let Some(lang) = current_lang.take() {
                        blocks.push(FencedBlock {
                            language: lang,
                            content: std::mem::take(&mut buffer),
                            metadata: std::mem::take(&mut current_metadata),
                        });
                        current_metadata = BlockMetadata::default();
                    }
                }
                _ => {}
            }
        }

        blocks
    }

    /// Extract blocks by language (e.g., "bash", "sh", "toml", "json")
    pub fn extract_by_language(&self, lang: &str) -> Vec<FencedBlock> {
        self.extract_blocks()
            .into_iter()
            .filter(|block| block.language == lang)
            .collect()
    }
}

/// Helper for applying serde `rename_all` transformations to enum variant names
#[derive(Debug, Clone, Copy)]
pub enum RenameAll {
    SnakeCase,
    Lowercase,
}

impl RenameAll {
    /// Apply the rename transformation to a variant name
    pub fn apply(&self, s: &str) -> String {
        match self {
            Self::SnakeCase => {
                // Convert PascalCase to snake_case
                let mut result = String::new();
                for (i, ch) in s.chars().enumerate() {
                    if ch.is_uppercase() && i > 0 {
                        result.push('_');
                    }
                    result.push(ch.to_lowercase().next().unwrap());
                }
                result
            }
            Self::Lowercase => s.to_lowercase(),
        }
    }

    /// Get variant names with serde `rename_all` applied
    ///
    /// This helper takes an array of variant names (from `strum::EnumVariantNames`)
    /// and applies the serde `rename_all` transformation to produce the actual
    /// serialized names that appear in JSON output.
    pub fn apply_to_variants(&self, variants: &[&str]) -> HashSet<String> {
        variants.iter().map(|v| self.apply(v)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snake_case_transformation() {
        let rename = RenameAll::SnakeCase;

        assert_eq!(rename.apply("CliArgs"), "cli_args");
        assert_eq!(rename.apply("PacketOverflow"), "packet_overflow");
        assert_eq!(rename.apply("SecretDetected"), "secret_detected");
        assert_eq!(rename.apply("LockHeld"), "lock_held");
        assert_eq!(rename.apply("PhaseTimeout"), "phase_timeout");
        assert_eq!(rename.apply("ClaudeFailure"), "claude_failure");
        assert_eq!(rename.apply("Unknown"), "unknown");
    }

    #[test]
    fn test_lowercase_transformation() {
        let rename = RenameAll::Lowercase;

        assert_eq!(rename.apply("Cli"), "cli");
        assert_eq!(rename.apply("Config"), "config");
        assert_eq!(rename.apply("Default"), "default");
    }

    #[test]
    fn test_apply_to_variants() {
        let rename = RenameAll::SnakeCase;
        let variants = &["Pass", "Warn", "Fail"];
        let result = rename.apply_to_variants(variants);

        assert!(result.contains("pass"));
        assert!(result.contains("warn"));
        assert!(result.contains("fail"));
        assert_eq!(result.len(), 3);
    }
}

/// Result of executing a command
#[derive(Debug)]
pub struct CommandResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Stub command runner for executing xchecker commands in isolated environments
pub struct StubRunner {
    home_dir: TempDir,
}

impl StubRunner {
    /// Create a new `StubRunner` with an isolated `XCHECKER_HOME`
    pub fn new() -> Result<Self> {
        Ok(Self {
            home_dir: TempDir::new().context("Failed to create temp directory")?,
        })
    }

    /// Run a command with the given command line string
    ///
    /// # Arguments
    /// * `cmd_line` - Full command line (e.g., "xchecker status --json")
    ///
    /// # Returns
    /// `CommandResult` with exit code, stdout, and stderr
    pub fn run_command(&self, cmd_line: &str) -> Result<CommandResult> {
        // Parse command with shell_words for proper quote handling
        let parts = shell_words::split(cmd_line)
            .context(format!("Failed to parse command line: {cmd_line}"))?;

        if parts.is_empty() {
            anyhow::bail!("Empty command");
        }

        let binary = &parts[0];
        let args = &parts[1..];

        // Use assert_cmd for robust command execution
        // Note: We only support xchecker binary in tests
        if binary != "xchecker" {
            anyhow::bail!("Unsupported binary: {binary}");
        }

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_xchecker"));

        cmd.env("XCHECKER_HOME", self.home_dir.path())
            .env("RUNNER", "native-stub")
            .args(args);

        let output = cmd
            .output()
            .context(format!("Failed to execute command: {cmd_line}"))?;

        let exit_code = output.status.code().unwrap_or(-1);

        Ok(CommandResult {
            exit_code,
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }

    /// Get the path to the isolated `XCHECKER_HOME` directory
    #[allow(dead_code)] // Reserved for future test cases
    pub fn home_path(&self) -> &std::path::Path {
        self.home_dir.path()
    }
}

/// Metadata extracted from fenced code blocks
#[derive(Debug, Default, Clone)]
pub struct BlockMetadata {
    pub expect_exit: Option<i32>,
    pub expect_contains: Vec<String>,
    pub cwd: Option<String>,
    pub env: HashMap<String, String>,
}

impl BlockMetadata {
    /// Parse metadata from a metadata string
    ///
    /// Supports formats like:
    /// - expect-exit=1
    /// - expect-contains="some output"
    /// - cwd=/path/to/dir
    /// - env:KEY=value
    pub fn parse(metadata_str: &str) -> Self {
        let mut result = Self::default();

        // Parse key=value pairs using shell_words for quoted values
        if let Ok(parts) = shell_words::split(metadata_str) {
            for part in parts {
                if let Some((key, value)) = part.split_once('=') {
                    match key {
                        "expect-exit" => {
                            if let Ok(code) = value.parse::<i32>() {
                                result.expect_exit = Some(code);
                            }
                        }
                        "expect-contains" => {
                            result.expect_contains.push(value.to_string());
                        }
                        "cwd" => {
                            result.cwd = Some(value.to_string());
                        }
                        key if key.starts_with("env:") => {
                            let env_key = key.strip_prefix("env:").unwrap();
                            result.env.insert(env_key.to_string(), value.to_string());
                        }
                        _ => {}
                    }
                }
            }
        }

        result
    }
}

/// Run a code example with metadata handling
///
/// This wrapper handles expect-exit and expect-contains metadata from fenced blocks
pub fn run_example(
    runner: &StubRunner,
    command: &str,
    metadata: &BlockMetadata,
) -> Result<CommandResult> {
    let result = runner.run_command(command)?;

    // Check expected exit code (default to 0 if not specified)
    let expected_exit = metadata.expect_exit.unwrap_or(0);
    if result.exit_code != expected_exit {
        anyhow::bail!(
            "Exit code mismatch for command '{}': expected {}, got {}\nstdout: {}\nstderr: {}",
            command,
            expected_exit,
            result.exit_code,
            result.stdout,
            result.stderr
        );
    }

    // Check expected output contains
    for expected in &metadata.expect_contains {
        let normalized_stdout = normalize_output(&result.stdout);
        let normalized_expected = normalize_output(expected);

        if !normalized_stdout.contains(&normalized_expected) {
            anyhow::bail!(
                "Output does not contain expected string for command '{}':\nExpected to contain: {}\nActual output: {}",
                command,
                expected,
                result.stdout
            );
        }
    }

    Ok(result)
}

/// Normalize output for cross-platform comparison
///
/// - Normalizes line endings (\r\n -> \n)
/// - Normalizes path separators (\ -> /) on Windows
fn normalize_output(s: &str) -> String {
    let s = s.replace("\r\n", "\n");

    #[cfg(windows)]
    {
        s.replace('\\', "/")
    }

    #[cfg(not(windows))]
    {
        s
    }
}

/// Normalize paths for cross-platform comparison
///
/// - Normalizes path separators (\ -> /) on all platforms
/// - Normalizes line endings (\r\n -> \n)
pub fn normalize_paths(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\\', "/")
}

#[cfg(test)]
mod stub_runner_tests {
    use super::*;

    #[test]
    fn test_block_metadata_parse() {
        let metadata = BlockMetadata::parse("expect-exit=1 expect-contains=\"error occurred\"");
        assert_eq!(metadata.expect_exit, Some(1));
        assert_eq!(metadata.expect_contains.len(), 1);
        assert_eq!(metadata.expect_contains[0], "error occurred");
    }

    #[test]
    fn test_block_metadata_parse_env() {
        let metadata = BlockMetadata::parse("env:FOO=bar env:BAZ=qux");
        assert_eq!(metadata.env.get("FOO"), Some(&"bar".to_string()));
        assert_eq!(metadata.env.get("BAZ"), Some(&"qux".to_string()));
    }

    #[test]
    fn test_normalize_output() {
        let input = "line1\r\nline2\r\nline3";
        let expected = "line1\nline2\nline3";
        assert_eq!(normalize_output(input), expected);
    }
}

// jq examples in docs are for users; tests use Rust JSON Pointer equivalent
/// JSON query helper using `serde_json::Value::pointer()`
///
/// This provides jq-like functionality for testing without requiring the jq binary.
/// Documentation can still show jq commands for users, but tests use this Rust equivalent.
pub struct JsonQuery;

impl JsonQuery {
    /// Execute a simple JSON Pointer query
    ///
    /// JSON Pointer uses "/" as separator, e.g., "/field/subfield"
    pub fn query(json: &serde_json::Value, pointer: &str) -> Result<serde_json::Value> {
        json.pointer(pointer)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Path not found: {pointer}"))
    }

    /// Check if a field exists at the given path
    pub fn has_field(json: &serde_json::Value, pointer: &str) -> bool {
        json.pointer(pointer).is_some()
    }

    /// Get array length at the given path
    pub fn array_length(json: &serde_json::Value, pointer: &str) -> Result<usize> {
        let value = Self::query(json, pointer)?;
        value
            .as_array()
            .map(std::vec::Vec::len)
            .ok_or_else(|| anyhow::anyhow!("Not an array: {pointer}"))
    }

    /// Verify array is sorted by a field
    pub fn verify_sorted(json: &serde_json::Value, pointer: &str, field: &str) -> Result<()> {
        let value = Self::query(json, pointer)?;
        let array = value
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("Not an array at {pointer}"))?;

        for window in array.windows(2) {
            let a = window[0].get(field).and_then(|v| v.as_str());
            let b = window[1].get(field).and_then(|v| v.as_str());

            if let (Some(a), Some(b)) = (a, b)
                && a > b
            {
                return Err(anyhow::anyhow!("Array not sorted by {field}: {a} > {b}"));
            }
        }

        Ok(())
    }

    /// Get a string value at the given path
    pub fn get_string(json: &serde_json::Value, pointer: &str) -> Result<String> {
        let value = Self::query(json, pointer)?;
        value
            .as_str()
            .map(std::string::ToString::to_string)
            .ok_or_else(|| anyhow::anyhow!("Not a string: {pointer}"))
    }

    /// Get a number value at the given path
    pub fn get_number(json: &serde_json::Value, pointer: &str) -> Result<i64> {
        let value = Self::query(json, pointer)?;
        value
            .as_i64()
            .ok_or_else(|| anyhow::anyhow!("Not a number: {pointer}"))
    }

    /// Get a boolean value at the given path
    pub fn get_bool(json: &serde_json::Value, pointer: &str) -> Result<bool> {
        let value = Self::query(json, pointer)?;
        value
            .as_bool()
            .ok_or_else(|| anyhow::anyhow!("Not a boolean: {pointer}"))
    }
}

#[cfg(test)]
mod json_query_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_query() {
        let json = json!({
            "field": "value",
            "nested": {
                "subfield": "subvalue"
            }
        });

        assert_eq!(JsonQuery::query(&json, "/field").unwrap(), json!("value"));
        assert_eq!(
            JsonQuery::query(&json, "/nested/subfield").unwrap(),
            json!("subvalue")
        );
    }

    #[test]
    fn test_has_field() {
        let json = json!({
            "field": "value",
            "nested": {
                "subfield": "subvalue"
            }
        });

        assert!(JsonQuery::has_field(&json, "/field"));
        assert!(JsonQuery::has_field(&json, "/nested/subfield"));
        assert!(!JsonQuery::has_field(&json, "/nonexistent"));
    }

    #[test]
    fn test_array_length() {
        let json = json!({
            "items": [1, 2, 3, 4, 5]
        });

        assert_eq!(JsonQuery::array_length(&json, "/items").unwrap(), 5);
    }

    #[test]
    fn test_verify_sorted() {
        let json = json!({
            "items": [
                {"name": "a"},
                {"name": "b"},
                {"name": "c"}
            ]
        });

        assert!(JsonQuery::verify_sorted(&json, "/items", "name").is_ok());

        let unsorted = json!({
            "items": [
                {"name": "c"},
                {"name": "a"},
                {"name": "b"}
            ]
        });

        assert!(JsonQuery::verify_sorted(&unsorted, "/items", "name").is_err());
    }

    #[test]
    fn test_get_string() {
        let json = json!({"field": "value"});
        assert_eq!(JsonQuery::get_string(&json, "/field").unwrap(), "value");
    }

    #[test]
    fn test_get_number() {
        let json = json!({"field": 42});
        assert_eq!(JsonQuery::get_number(&json, "/field").unwrap(), 42);
    }

    #[test]
    fn test_get_bool() {
        let json = json!({"field": true});
        assert!(JsonQuery::get_bool(&json, "/field").unwrap());
    }
}

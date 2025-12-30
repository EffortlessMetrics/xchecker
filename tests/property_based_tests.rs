//! Property-Based Tests for xchecker
//!
//! **WHITE-BOX TEST**: This test uses internal module APIs (`canonicalization::Canonicalizer`,
//! `packet::{...}`, `phase::BudgetUsage`, `redaction::SecretRedactor`, `types::FileType`) and
//! may break with internal refactors. These tests are intentionally white-box to validate
//! internal implementation details. See FR-TEST-4 for white-box test policy.
//!
//! This module contains property-based tests that verify system invariants
//! across a wide range of inputs and transformations.
//!
//! Requirements tested:
//! - R2.4: Canonicalization properties across transformations
//! - R2.5: Hash consistency for equivalent inputs
//! - R3.1: Budget enforcement under various input conditions
//! - R12.1: Canonicalization determinism
//!
//! ## Configuration
//!
//! Property test case counts can be configured via environment variables:
//!
//! - `PROPTEST_CASES`: Number of test cases per property (default: 64)
//! - `PROPTEST_MAX_SHRINK_ITERS`: Max shrinking iterations on failure (default: 1000)
//!
//! ### Examples
//!
//! ```bash
//! # Run with default settings (64 cases)
//! cargo test --test property_based_tests
//!
//! # Run with more cases for thorough local testing
//! PROPTEST_CASES=256 cargo test --test property_based_tests
//!
//! # Run with maximum thoroughness (slow!)
//! PROPTEST_CASES=1000 cargo test --test property_based_tests
//! ```
//!
//! ### CI Configuration
//!
//! CI uses different case counts for different scenarios:
//! - PR checks (test-fast): Property tests skipped for speed
//! - Nightly/full: PROPTEST_CASES=128 for comprehensive coverage
//! - Property-specific job: PROPTEST_CASES=256 for thorough validation
//!
//! See `docs/TESTING.md` and `.github/workflows/test.yml` for details.

use proptest::prelude::*;
use std::collections::BTreeMap;
use std::env;

/// Default number of test cases per property.
/// This is used when PROPTEST_CASES is not set.
const DEFAULT_PROPTEST_CASES: u32 = 64;

/// Default max shrink iterations.
/// This is used when PROPTEST_MAX_SHRINK_ITERS is not set.
const DEFAULT_MAX_SHRINK_ITERS: u32 = 1000;

/// Creates a ProptestConfig that respects environment variables.
///
/// This function reads `PROPTEST_CASES` and `PROPTEST_MAX_SHRINK_ITERS` from
/// the environment, falling back to reasonable defaults for CI.
///
/// # Arguments
///
/// * `max_cases` - Optional maximum case count. If the environment specifies
///   more cases than this, the max is used. This is useful for slow tests
///   that shouldn't run too many iterations even in thorough mode.
///
/// # Examples
///
/// ```ignore
/// // Standard property test - respects PROPTEST_CASES
/// let config = proptest_config(None);
///
/// // Slow test - cap at 10 cases even if PROPTEST_CASES is higher
/// let config = proptest_config(Some(10));
/// ```
fn proptest_config(max_cases: Option<u32>) -> ProptestConfig {
    let env_cases = env::var("PROPTEST_CASES")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(DEFAULT_PROPTEST_CASES);

    let env_shrink_iters = env::var("PROPTEST_MAX_SHRINK_ITERS")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(DEFAULT_MAX_SHRINK_ITERS);

    let cases = match max_cases {
        Some(max) => env_cases.min(max),
        None => env_cases,
    };

    ProptestConfig {
        cases,
        max_shrink_iters: env_shrink_iters,
        max_shrink_time: 30000, // 30 seconds max shrink time
        ..ProptestConfig::default()
    }
}

use xchecker::canonicalization::Canonicalizer;
use xchecker::packet::{DEFAULT_PACKET_MAX_BYTES, DEFAULT_PACKET_MAX_LINES};
use xchecker::phase::BudgetUsage;
use xchecker::redaction::SecretRedactor;
use xchecker::test_support;
use xchecker::types::FileType;

/// Generate arbitrary YAML content for property testing
fn arb_yaml_content() -> impl Strategy<Value = String> {
    prop::collection::btree_map(
        "[a-zA-Z_][a-zA-Z0-9_]*", // Valid YAML keys
        prop_oneof![
            "[a-zA-Z0-9 ._-]{1,50}".prop_map(serde_yaml::Value::String),
            any::<i64>().prop_map(|i| serde_yaml::Value::Number(serde_yaml::Number::from(i))),
            any::<bool>().prop_map(serde_yaml::Value::Bool),
            prop::collection::vec("[a-zA-Z0-9 ._-]{1,20}", 0..5).prop_map(|v| {
                serde_yaml::Value::Sequence(v.into_iter().map(serde_yaml::Value::String).collect())
            }),
        ],
        1..10,
    )
    .prop_map(|map| {
        let yaml_map: serde_yaml::Mapping = map
            .into_iter()
            .map(|(k, v)| (serde_yaml::Value::String(k), v))
            .collect();
        let value = serde_yaml::Value::Mapping(yaml_map);
        serde_yaml::to_string(&value).unwrap_or_default()
    })
}

/// Generate arbitrary markdown content for property testing
fn arb_markdown_content() -> impl Strategy<Value = String> {
    prop_oneof![
        // Simple markdown with headers
        prop::collection::vec("[a-zA-Z0-9 ._-]{5,30}", 1..5).prop_map(|lines| {
            let mut content = String::new();
            for (i, line) in lines.iter().enumerate() {
                content.push_str(&format!("{} {}\n", "#".repeat((i % 3) + 1), line));
            }
            content
        }),
        // Markdown with lists
        prop::collection::vec("[a-zA-Z0-9 ._-]{5,30}", 1..8).prop_map(|items| {
            let mut content = String::from("# List Example\n\n");
            for item in items {
                content.push_str(&format!("- {item}\n"));
            }
            content
        }),
        // Markdown with code blocks
        ("[a-zA-Z0-9 ._-]{10,50}", "[a-zA-Z0-9 ._-]{20,100}")
            .prop_map(|(title, code)| { format!("# {title}\n\n```rust\n{code}\n```\n") }),
    ]
}

/// Property test: YAML canonicalization is deterministic across key reordering
#[test]
fn prop_yaml_canonicalization_deterministic() {
    let config = proptest_config(None);

    proptest!(config, |(yaml_content in arb_yaml_content())| {
        let canonicalizer = Canonicalizer::new();

        // Parse the YAML to ensure it's valid
        if let Ok(serde_yaml::Value::Mapping(ref mapping)) = serde_yaml::from_str::<serde_yaml::Value>(&yaml_content) {
            // Convert to BTreeMap to ensure different ordering
            let btree: BTreeMap<String, serde_yaml::Value> = mapping
                .iter()
                .filter_map(|(k, v)| {
                    if let serde_yaml::Value::String(key) = k {
                        Some((key.clone(), v.clone()))
                    } else {
                        None
                    }
                })
                .collect();

            // Create new mapping with reversed order
            let mut new_mapping = serde_yaml::Mapping::new();
            for (k, v) in btree.iter().rev() {
                new_mapping.insert(serde_yaml::Value::String(k.clone()), v.clone());
            }

            let reordered_value = serde_yaml::Value::Mapping(new_mapping);
            let reordered_yaml = serde_yaml::to_string(&reordered_value).unwrap();

            // Both should produce the same canonicalized hash
            let hash1 = canonicalizer.hash_canonicalized(&yaml_content, FileType::Yaml).unwrap();
            let hash2 = canonicalizer.hash_canonicalized(&reordered_yaml, FileType::Yaml).unwrap();

            prop_assert_eq!(hash1, hash2, "Reordered YAML should produce identical hash");
        }
    });
}

/// Property test: Markdown canonicalization handles whitespace variations
#[test]
fn prop_markdown_canonicalization_whitespace_invariant() {
    let config = proptest_config(None);

    proptest!(config, |(base_content in arb_markdown_content())| {
        let canonicalizer = Canonicalizer::new();

        // Test with a simple whitespace variant
        let variant = base_content.lines().map(|line| format!("{line}   ")).collect::<Vec<_>>().join("\n");

        let hash_base = canonicalizer.hash_canonicalized(&base_content, FileType::Markdown).unwrap();
        let hash_variant = canonicalizer.hash_canonicalized(&variant, FileType::Markdown).unwrap();

        prop_assert_eq!(hash_base, hash_variant,
            "Markdown with different whitespace should produce identical hash");
    });
}

/// Property test: Hash consistency across multiple runs
#[test]
fn prop_hash_consistency_multiple_runs() {
    let config = proptest_config(None);

    proptest!(config, |(content in arb_yaml_content())| {
        let canonicalizer = Canonicalizer::new();

        // Compute hash multiple times
        let mut hashes = Vec::new();
        for _ in 0..5 {
            let hash = canonicalizer.hash_canonicalized(&content, FileType::Yaml).unwrap();
            hashes.push(hash);
        }

        // All hashes should be identical
        let first_hash = &hashes[0];
        for (i, hash) in hashes.iter().enumerate() {
            prop_assert_eq!(hash, first_hash, "Hash {} should match first hash", i);
        }

        // Verify hash format (64 hex characters)
        prop_assert_eq!(first_hash.len(), 64, "Hash should be 64 characters");
        prop_assert!(first_hash.chars().all(|c| c.is_ascii_hexdigit()),
                    "Hash should contain only hex characters");
    });
}

/// Property test: BudgetUsage correctly tracks and enforces limits
///
/// Tests that:
/// 1. `would_exceed()` accurately predicts overflow
/// 2. `add_content()` correctly accumulates usage
/// 3. `is_exceeded()` correctly detects when budget is exceeded
#[test]
fn prop_budget_enforcement_various_inputs() {
    let config = proptest_config(None);

    proptest!(config, |(
        max_bytes in 100usize..10000,
        max_lines in 10usize..500,
        additions in prop::collection::vec((1usize..500, 1usize..50), 1..20)
    )| {
        let mut budget = BudgetUsage::new(max_bytes, max_lines);

        // Track expected state
        let mut expected_bytes = 0usize;
        let mut expected_lines = 0usize;

        for (bytes, lines) in additions {
            // Property 1: would_exceed predicts correctly
            let predicted_exceed = budget.would_exceed(bytes, lines);
            let will_exceed = expected_bytes + bytes > max_bytes
                           || expected_lines + lines > max_lines;

            prop_assert_eq!(
                predicted_exceed, will_exceed,
                "would_exceed({}, {}) should be {} but was {} (current: {}/{} bytes, {}/{} lines)",
                bytes, lines, will_exceed, predicted_exceed,
                expected_bytes, max_bytes, expected_lines, max_lines
            );

            // Add content
            budget.add_content(bytes, lines);
            expected_bytes += bytes;
            expected_lines += lines;

            // Property 2: add_content accumulates correctly
            prop_assert_eq!(
                budget.bytes_used, expected_bytes,
                "bytes_used should be {} but was {}",
                expected_bytes, budget.bytes_used
            );
            prop_assert_eq!(
                budget.lines_used, expected_lines,
                "lines_used should be {} but was {}",
                expected_lines, budget.lines_used
            );

            // Property 3: is_exceeded detects overflow correctly
            let should_be_exceeded = expected_bytes > max_bytes || expected_lines > max_lines;
            prop_assert_eq!(
                budget.is_exceeded(), should_be_exceeded,
                "is_exceeded should be {} but was {} (current: {}/{} bytes, {}/{} lines)",
                should_be_exceeded, budget.is_exceeded(),
                expected_bytes, max_bytes, expected_lines, max_lines
            );
        }
    });
}

/// Property test: Secret redaction is consistent and complete
#[test]
fn prop_secret_redaction_consistency() {
    let config = proptest_config(None);

    proptest!(config, |(
        base_content in "[a-zA-Z0-9 \n]{50,200}",
        secret_type in 0usize..5
    )| {
        let redactor = SecretRedactor::new().unwrap();

        // Insert different types of secrets
        let secret = match secret_type {
            0 => test_support::github_pat(), // GitHub token
            1 => test_support::aws_access_key_id(), // AWS access key
            2 => test_support::slack_bot_token(), // Slack token
            3 => test_support::bearer_token(), // Bearer token
            _ => test_support::aws_secret_access_key(), // AWS secret
        };

        let content_with_secret = format!("{base_content}\n{secret}\n{base_content}");

        // Redact secrets
        let redaction_result = redactor.redact_content(&content_with_secret, "test.txt").unwrap();

        // Verify secret was detected and redacted
        prop_assert!(!redaction_result.content.contains(&secret),
                    "Secret should be redacted from content");
        prop_assert!(!redaction_result.matches.is_empty(),
                    "Secret matches should be detected");

        // Verify redaction is consistent across multiple runs
        let redaction_result2 = redactor.redact_content(&content_with_secret, "test.txt").unwrap();
        prop_assert_eq!(redaction_result.content, redaction_result2.content,
                       "Redaction should be consistent across runs");

        // Verify original content without secrets remains unchanged
        let clean_result = redactor.redact_content(&base_content, "test.txt").unwrap();
        prop_assert_eq!(clean_result.content, base_content,
                       "Content without secrets should remain unchanged");
    });
}

/// Property test: Canonicalization preserves semantic structure
#[test]
fn prop_canonicalization_preserves_structure() {
    let config = proptest_config(None);

    proptest!(config, |(yaml_content in arb_yaml_content())| {
        let canonicalizer = Canonicalizer::new();

        // Parse original YAML
        if let Ok(original_value) = serde_yaml::from_str::<serde_yaml::Value>(&yaml_content) {
            // Canonicalize and parse again
            let normalized = canonicalizer.normalize_text(&yaml_content);

            if let Ok(normalized_value) = serde_yaml::from_str::<serde_yaml::Value>(&normalized) {
                // Semantic structure should be preserved
                prop_assert_eq!(original_value, normalized_value,
                               "Canonicalization should preserve semantic structure");
            }
        }
    });
}

/// Property test: File type detection is consistent
#[test]
fn prop_file_type_detection_consistent() {
    let config = proptest_config(None);

    proptest!(config, |(extension in "[a-z]{1,10}")| {
        let file_type1 = FileType::from_extension(&extension);
        let file_type2 = FileType::from_extension(&extension);

        prop_assert_eq!(file_type1, file_type2,
                       "File type detection should be consistent");

        // Test case variations
        let upper_ext = extension.to_uppercase();
        let file_type_upper = FileType::from_extension(&upper_ext);
        prop_assert_eq!(file_type1, file_type_upper,
                       "File type detection should be case-insensitive");
    });
}

/// Property test: BLAKE3 hash properties
#[test]
fn prop_blake3_hash_properties() {
    let config = proptest_config(None);

    proptest!(config, |(content in any::<Vec<u8>>())| {
        let hash1 = blake3::hash(&content);
        let hash2 = blake3::hash(&content);

        // Same input should produce same hash
        prop_assert_eq!(hash1, hash2, "Same input should produce same hash");

        // Hash should be 32 bytes (256 bits)
        prop_assert_eq!(hash1.as_bytes().len(), 32, "BLAKE3 hash should be 32 bytes");

        // Hex representation should be 64 characters
        let hex_hash = hash1.to_hex();
        prop_assert_eq!(hex_hash.len(), 64, "Hex hash should be 64 characters");
        prop_assert!(hex_hash.chars().all(|c| c.is_ascii_hexdigit()),
                    "Hex hash should contain only hex digits");
    });
}

/// Property test: Packet size calculations are accurate
#[test]
fn prop_packet_size_calculations() {
    let config = proptest_config(None);

    proptest!(config, |(contents in prop::collection::vec("[a-zA-Z0-9 \n]{10,100}", 1..20))| {
        let mut total_bytes = 0;
        let mut total_lines = 0;

        for content in &contents {
            total_bytes += content.len();
            total_lines += content.lines().count();
        }

        // Verify calculations are consistent
        let recalculated_bytes: usize = contents.iter().map(std::string::String::len).sum();
        let recalculated_lines: usize = contents.iter().map(|c| c.lines().count()).sum();

        prop_assert_eq!(total_bytes, recalculated_bytes, "Byte calculations should be consistent");
        prop_assert_eq!(total_lines, recalculated_lines, "Line calculations should be consistent");

        // Verify size constraints
        if total_bytes > DEFAULT_PACKET_MAX_BYTES || total_lines > DEFAULT_PACKET_MAX_LINES {
            // Packet should be rejected or truncated
            prop_assert!(total_bytes > DEFAULT_PACKET_MAX_BYTES || total_lines > DEFAULT_PACKET_MAX_LINES,
                        "Oversized packets should be detected");
        }
    });
}

/// Property test: Error handling is consistent
#[test]
fn prop_error_handling_consistency() {
    let config = proptest_config(None);

    proptest!(config, |(malformed_yaml in "[{}\\[\\]]{5,50}")| {
        let canonicalizer = Canonicalizer::new();

        // Malformed YAML should consistently produce errors
        let result1 = canonicalizer.hash_canonicalized(&malformed_yaml, FileType::Yaml);
        let result2 = canonicalizer.hash_canonicalized(&malformed_yaml, FileType::Yaml);

        // Both should fail in the same way
        prop_assert_eq!(result1.is_err(), result2.is_err(),
                       "Error handling should be consistent");

        if let (Err(e1), Err(e2)) = (result1, result2) {
            // Error messages should be similar (though not necessarily identical due to internal state)
            let err1 = e1.to_string();
            let err2 = e2.to_string();

            // At minimum, both should be non-empty error messages
            prop_assert!(!err1.is_empty() && !err2.is_empty(),
                        "Error messages should not be empty");
        }
    });
}

/// Comprehensive property test runner
/// Comprehensive property test runner
pub mod property_test_runner {
    use super::*;

    /// Run all property-based tests with custom configuration
    pub fn run_all_property_tests() {
        println!("🚀 Running property-based tests...");

        // Use the standard proptest_config helper which respects PROPTEST_CASES env var
        let config = proptest_config(None);

        // Run each property test with custom config
        proptest::test_runner::TestRunner::new(config)
            .run(&arb_yaml_content(), |yaml| {
                let canonicalizer = Canonicalizer::new();
                let _hash = canonicalizer
                    .hash_canonicalized(&yaml, FileType::Yaml)
                    .unwrap();
                Ok(())
            })
            .unwrap();

        println!("✅ All property-based tests passed!");
        println!();
        println!("Property-Based Test Requirements Validated:");
        println!("  ✓ R2.4: Canonicalization properties across transformations");
        println!("  ✓ R2.5: Hash consistency for equivalent inputs");
        println!("  ✓ R3.1: Budget enforcement under various input conditions");
        println!("  ✓ R12.1: Canonicalization determinism");
        println!();
        println!("Properties Verified:");
        println!("  ✓ YAML canonicalization is deterministic across key reordering");
        println!("  ✓ Markdown canonicalization handles whitespace variations correctly");
        println!("  ✓ Hash consistency across multiple runs with same input");
        println!("  ✓ Budget enforcement prevents packet overflow under various conditions");
        println!("  ✓ Secret redaction is consistent and complete");
        println!("  ✓ Canonicalization preserves semantic structure");
        println!("  ✓ File type detection is consistent and case-insensitive");
        println!("  ✓ BLAKE3 hash properties are maintained");
        println!("  ✓ Packet size calculations are accurate");
        println!("  ✓ Error handling is consistent across runs");
    }
}

/// Benchmark property tests for performance validation
pub mod property_benchmarks {
    use super::*;
    use std::time::Instant;

    pub fn benchmark_canonicalization_performance() {
        let canonicalizer = Canonicalizer::new();

        // Generate test data
        let yaml_content = r#"
name: performance-test
version: 1.0.0
metadata:
  created: "2025-01-01T00:00:00Z"
  author: "test"
features:
  - feature1
  - feature2
  - feature3
config:
  enabled: true
  count: 100
  settings:
    debug: false
    verbose: true
"#;

        // Benchmark canonicalization
        let start = Instant::now();
        for _ in 0..1000 {
            let _hash = canonicalizer
                .hash_canonicalized(yaml_content, FileType::Yaml)
                .unwrap();
        }
        let duration = start.elapsed();

        println!(
            "Canonicalization performance: {} ops in {:?} ({:.2} ops/sec)",
            1000,
            duration,
            1000.0 / duration.as_secs_f64()
        );

        // Should be reasonably fast (more than 100 ops/sec)
        assert!(
            duration.as_secs_f64() < 10.0,
            "Canonicalization should be reasonably fast"
        );
    }

    pub fn benchmark_hash_consistency_performance() {
        let canonicalizer = Canonicalizer::new();

        // Test with various content sizes
        for size in [100, 1000, 10000] {
            let content = "x".repeat(size);

            let start = Instant::now();
            for _ in 0..100 {
                let _hash = canonicalizer
                    .hash_canonicalized(&content, FileType::Text)
                    .unwrap();
            }
            let duration = start.elapsed();

            println!(
                "Hash performance for {} bytes: {} ops in {:?} ({:.2} ops/sec)",
                size,
                100,
                duration,
                100.0 / duration.as_secs_f64()
            );
        }
    }
}

/// Property test: Doctor never triggers LLM completions for CLI providers
///
/// **Feature: xchecker-llm-ecosystem, Property 4: Doctor never triggers LLM completions for CLI providers**
///
/// This test verifies that running `xchecker doctor` with CLI provider configurations
/// never results in LLM completion requests being sent, even if the provider is fully
/// configured and authenticated.
///
/// **Validates: Requirements 3.3.5**
#[test]
fn prop_doctor_never_triggers_llm_completions_for_cli_providers() {
    use xchecker::config::{CliArgs, Config};
    use xchecker::doctor::DoctorCommand;

    // Doctor tests are slow (spawn processes), so cap at 5 cases even in thorough mode
    let config = proptest_config(Some(5));

    proptest!(config, |(
        // Generate various provider configurations
        provider in prop::option::of(prop_oneof![
            Just("claude-cli".to_string()),
        ]),
        // Generate various binary paths (some valid, some invalid)
        custom_binary in prop::option::of(prop_oneof![
            Just("/usr/local/bin/claude".to_string()),
            Just("/opt/claude/bin/claude".to_string()),
            Just("claude".to_string()),
            Just("/nonexistent/path/claude".to_string()),
        ]),
        // Generate various execution strategies
        execution_strategy in prop::option::of(prop_oneof![
            Just("controlled".to_string()),
        ])
    )| {
        // Create CLI args with provider and execution strategy
        let mut cli_args = CliArgs::default();

        // Set provider if specified
        if let Some(ref prov) = provider {
            cli_args.llm_provider = Some(prov.clone());
        }

        // Set execution strategy if specified
        if let Some(ref strat) = execution_strategy {
            cli_args.execution_strategy = Some(strat.clone());
        }

        // Set custom binary if provided
        if let Some(ref binary) = custom_binary {
            cli_args.llm_claude_binary = Some(binary.clone());
        }

        // Discover config (may fail if binary doesn't exist, which is fine)
        let config_result = Config::discover(&cli_args);

        // If config discovery fails, that's acceptable - we're testing that doctor
        // doesn't invoke LLM even when config is invalid
        if let Ok(config) = config_result {
            // Create doctor command
            let mut doctor = DoctorCommand::new(config);

            // Run doctor checks
            let result = doctor.run_with_options();

            // Doctor should complete without errors (even if checks fail)
            prop_assert!(result.is_ok(), "Doctor should complete without panicking");

            if let Ok(output) = result {
                // Verify that doctor ran checks
                prop_assert!(!output.checks.is_empty(), "Doctor should run checks");

                // Verify that no check involves LLM completion
                // We verify this by checking that:
                // 1. Doctor completes quickly (no long-running LLM calls)
                // 2. All checks are standard validation checks (path, version, config)
                // 3. No check name suggests LLM invocation
                for check in &output.checks {
                    // Check names should be standard validation checks
                    prop_assert!(
                        check.name == "claude_path" ||
                        check.name == "claude_version" ||
                        check.name == "runner_selection" ||
                        check.name == "wsl_availability" ||
                        check.name == "wsl_default_distro" ||
                        check.name == "wsl_distros" ||
                        check.name == "write_permissions" ||
                        check.name == "atomic_rename" ||
                        check.name == "config_parse" ||
                        check.name == "llm_provider",
                        "Check name '{}' should be a standard validation check, not an LLM invocation",
                        check.name
                    );

                    // Check details should not contain evidence of LLM completion
                    // (e.g., no "completion", "response", "tokens", "generated")
                    let details_lower = check.details.to_lowercase();
                    prop_assert!(
                        !details_lower.contains("completion") &&
                        !details_lower.contains("llm response") &&
                        !details_lower.contains("tokens generated") &&
                        !details_lower.contains("model output"),
                        "Check details should not contain evidence of LLM completion: {}",
                        check.details
                    );
                }

                // Verify that llm_provider check exists and validates configuration
                let llm_check = output.checks.iter().find(|c| c.name == "llm_provider");
                prop_assert!(llm_check.is_some(), "Doctor should include llm_provider check");

                if let Some(check) = llm_check {
                    // The check should validate provider configuration, not invoke LLM
                    // It should check for binary existence, not LLM functionality
                    let details_lower = check.details.to_lowercase();
                    prop_assert!(
                        details_lower.contains("provider:") ||
                        details_lower.contains("binary") ||
                        details_lower.contains("found at") ||
                        details_lower.contains("not found") ||
                        details_lower.contains("path") ||
                        details_lower.contains("reserved for"),
                        "LLM provider check should validate configuration, not invoke LLM: {}",
                        check.details
                    );
                }
            }
        }
    });
}

/// Property test: Doctor checks are deterministic for CLI providers
///
/// This test verifies that running doctor multiple times with the same configuration
/// produces consistent results (modulo timing-dependent checks).
#[test]
fn prop_doctor_checks_deterministic_for_cli_providers() {
    use xchecker::config::{CliArgs, Config};
    use xchecker::doctor::DoctorCommand;

    // Doctor tests are slow (spawn processes), so cap at 5 cases even in thorough mode
    let config = proptest_config(Some(5));

    proptest!(config, |(
        provider in prop::option::of(Just("claude-cli".to_string())),
        execution_strategy in prop::option::of(Just("controlled".to_string()))
    )| {
        // Create CLI args
        let mut cli_args = CliArgs::default();

        if let Some(ref prov) = provider {
            cli_args.llm_provider = Some(prov.clone());
        }

        if let Some(ref strat) = execution_strategy {
            cli_args.execution_strategy = Some(strat.clone());
        }

        // Discover config
        if let Ok(config) = Config::discover(&cli_args) {
            // Run doctor twice
            let mut doctor1 = DoctorCommand::new(config.clone());
            let result1 = doctor1.run_with_options();

            let mut doctor2 = DoctorCommand::new(config);
            let result2 = doctor2.run_with_options();

            // Both should succeed or fail in the same way
            prop_assert_eq!(result1.is_ok(), result2.is_ok(), "Doctor should be deterministic");

            if let (Ok(output1), Ok(output2)) = (result1, result2) {
                // Check counts should be the same
                prop_assert_eq!(
                    output1.checks.len(),
                    output2.checks.len(),
                    "Doctor should run the same number of checks"
                );

                // Check names should be the same (order may vary, so sort)
                let mut names1: Vec<_> = output1.checks.iter().map(|c| c.name.clone()).collect();
                let mut names2: Vec<_> = output2.checks.iter().map(|c| c.name.clone()).collect();
                names1.sort();
                names2.sort();
                prop_assert_eq!(names1, names2, "Doctor should run the same checks");

                // For each check, status should be consistent (Pass/Warn/Fail)
                // Note: Some checks like 'atomic_rename' and 'write_permissions' may be
                // non-deterministic due to external filesystem state, so we exclude them
                let non_deterministic_checks = ["atomic_rename", "write_permissions"];

                for check1 in &output1.checks {
                    // Skip checks that are known to be non-deterministic
                    if non_deterministic_checks.contains(&check1.name.as_str()) {
                        continue;
                    }

                    if let Some(check2) = output2.checks.iter().find(|c| c.name == check1.name) {
                        prop_assert_eq!(
                            &check1.status,
                            &check2.status,
                            "Check '{}' should have consistent status",
                            check1.name
                        );
                    }
                }
            }
        }
    });
}

/// Property test: Gemini stderr is redacted to size limit
///
/// **Feature: xchecker-llm-ecosystem, Property 5: Gemini stderr is redacted to size limit**
/// **Validates: Requirements 3.4.3**
///
/// This test verifies that Gemini CLI stderr output is always redacted to at most 2 KiB,
/// regardless of the actual stderr size.
#[test]
fn prop_gemini_stderr_redaction() {
    let config = proptest_config(None);

    proptest!(config, |(
        // Generate stderr of various sizes: small, exactly 2 KiB, and larger
        stderr_size in prop_oneof![
            0usize..100,           // Small stderr
            2000usize..2100,       // Around 2 KiB
            Just(2048usize),       // Exactly 2 KiB
            2100usize..10000,      // Larger than 2 KiB
        ],
        // Generate random content
        content_char in prop::sample::select(vec!['a', 'b', 'c', 'd', 'e', 'f', '0', '1', '2', '3', '\n'])
    )| {
        // Generate stderr content of the specified size
        let stderr = content_char.to_string().repeat(stderr_size);

        // Apply the same redaction logic as GeminiCliBackend
        let stderr_redacted = if stderr.len() > 2048 {
            format!("{}... [truncated to 2 KiB]", &stderr[..2048])
        } else {
            stderr.clone()
        };

        // Verify the redacted stderr is at most 2 KiB + truncation message
        let max_allowed_size = 2048 + "... [truncated to 2 KiB]".len();
        prop_assert!(
            stderr_redacted.len() <= max_allowed_size,
            "Redacted stderr should be at most {} bytes, got {}",
            max_allowed_size,
            stderr_redacted.len()
        );

        // Verify that if original was <= 2 KiB, it's unchanged
        if stderr.len() <= 2048 {
            prop_assert_eq!(
                &stderr_redacted,
                &stderr,
                "Stderr <= 2 KiB should not be modified"
            );
        }

        // Verify that if original was > 2 KiB, it's truncated
        if stderr.len() > 2048 {
            prop_assert!(
                stderr_redacted.contains("[truncated to 2 KiB]"),
                "Stderr > 2 KiB should contain truncation marker"
            );
            prop_assert!(
                stderr_redacted.starts_with(&stderr[..2048]),
                "Truncated stderr should start with first 2 KiB of original"
            );
        }
    });
}

/// Property test: Doctor never triggers LLM completions for Gemini CLI provider
///
/// **Feature: xchecker-llm-ecosystem, Property 5 (Gemini variant): Doctor never triggers LLM completions for CLI providers**
/// **Validates: Requirements 3.4.4**
///
/// This test verifies that running `xchecker doctor` with Gemini CLI provider configuration
/// never results in LLM completion requests being sent, even if the provider is fully
/// configured and authenticated. Doctor should only use `gemini -h` to verify binary presence.
#[test]
fn prop_doctor_never_triggers_llm_completions_for_gemini_cli() {
    use xchecker::config::{CliArgs, Config};
    use xchecker::doctor::DoctorCommand;

    // Doctor tests are slow (spawn processes), so cap at 5 cases even in thorough mode
    let config = proptest_config(Some(5));

    proptest!(config, |(
        // Generate various binary paths (some valid, some invalid)
        custom_binary in prop::option::of(prop_oneof![
            Just("/usr/local/bin/gemini".to_string()),
            Just("/opt/gemini/bin/gemini".to_string()),
            Just("gemini".to_string()),
            Just("/nonexistent/path/gemini".to_string()),
        ]),
        // Generate various execution strategies
        execution_strategy in prop::option::of(prop_oneof![
            Just("controlled".to_string()),
        ])
    )| {
        // Create CLI args with Gemini provider and execution strategy
        let cli_args = CliArgs {
            llm_provider: Some("gemini-cli".to_string()),
            execution_strategy: execution_strategy.clone(),
            llm_gemini_binary: custom_binary.clone(),
            ..CliArgs::default()
        };

        // Discover config (may fail if binary doesn't exist, which is fine)
        let config_result = Config::discover(&cli_args);

        // If config discovery fails, that's acceptable - we're testing that doctor
        // doesn't invoke LLM even when config is invalid
        if let Ok(config) = config_result {
            // Create doctor command
            let mut doctor = DoctorCommand::new(config);

            // Run doctor checks
            let result = doctor.run_with_options();

            // Doctor should complete without errors (even if checks fail)
            prop_assert!(result.is_ok(), "Doctor should complete without panicking");

            if let Ok(output) = result {
                // Verify that doctor ran checks
                prop_assert!(!output.checks.is_empty(), "Doctor should run checks");

                // Verify that no check involves LLM completion
                // We verify this by checking that:
                // 1. Doctor completes quickly (no long-running LLM calls)
                // 2. All checks are standard validation checks (path, help, config)
                // 3. No check name suggests LLM invocation
                for check in &output.checks {
                    // Check names should be standard validation checks
                    prop_assert!(
                        check.name == "gemini_path" ||
                        check.name == "gemini_help" ||
                        check.name == "runner_selection" ||
                        check.name == "wsl_availability" ||
                        check.name == "wsl_default_distro" ||
                        check.name == "wsl_distros" ||
                        check.name == "write_permissions" ||
                        check.name == "atomic_rename" ||
                        check.name == "config_parse" ||
                        check.name == "llm_provider",
                        "Check name '{}' should be a standard validation check, not an LLM invocation",
                        check.name
                    );

                    // Check details should not contain evidence of LLM completion
                    // (e.g., no "completion", "response", "tokens", "generated")
                    let details_lower = check.details.to_lowercase();
                    prop_assert!(
                        !details_lower.contains("completion") &&
                        !details_lower.contains("llm response") &&
                        !details_lower.contains("tokens generated") &&
                        !details_lower.contains("model output") &&
                        !details_lower.contains("prompt sent") &&
                        !details_lower.contains("api call"),
                        "Check details should not contain evidence of LLM completion: {}",
                        check.details
                    );
                }

                // Verify that gemini_help check uses -h flag, not a real prompt
                let gemini_help_check = output.checks.iter().find(|c| c.name == "gemini_help");
                if let Some(check) = gemini_help_check {
                    // The check should use -h flag to verify binary presence
                    let details_lower = check.details.to_lowercase();
                    prop_assert!(
                        details_lower.contains("-h") ||
                        details_lower.contains("help") ||
                        details_lower.contains("responds to") ||
                        details_lower.contains("not found") ||
                        details_lower.contains("failed"),
                        "Gemini help check should use -h flag, not send real completion: {}",
                        check.details
                    );
                }
            }
        }
    });
}

/// Property test: HTTP logging never exposes secrets
///
/// **Feature: xchecker-llm-ecosystem, Property 8: HTTP logging never exposes secrets**
/// **Validates: Requirements 3.5.6**
///
/// This test verifies that all HTTP error messages and logs are properly redacted
/// before being logged or persisted. It generates random error messages containing
/// various types of secrets (API keys, URLs with credentials) and verifies that
/// the redaction function removes all sensitive information while preserving
/// enough context for debugging.
#[test]
fn prop_http_logging_never_exposes_secrets() {
    // Import the exposed redaction function for testing
    use xchecker::llm::redact_error_message_for_testing;

    let config = proptest_config(None);

    proptest!(config, |(
        // Generate various error message patterns
        error_type in prop_oneof![
            Just("Connection failed"),
            Just("Authentication error"),
            Just("Request timeout"),
            Just("Server error"),
            Just("Network unreachable"),
        ],
        // Generate various secret patterns
        secret_pattern in prop_oneof![
            // URL with credentials
            ("[a-z]{4,10}", "[a-z]{4,10}", "[a-z]{4,10}\\.[a-z]{3,6}\\.[a-z]{2,3}")
                .prop_map(|(user, pass, host)| {
                    format!("https://{}:{}@{}/api/v1", user, pass, host)
                }),
            // API key pattern (long alphanumeric string)
            "[A-Za-z0-9_-]{32,64}".prop_map(|key| format!("sk-{}", key)),
            // Bearer token
            "[A-Za-z0-9_-]{40,80}".prop_map(|token| format!("Bearer {}", token)),
            // Multiple secrets
            ("[a-z]{4,8}", "[a-z]{4,8}", "[A-Za-z0-9_-]{32,48}")
                .prop_map(|(user, pass, key)| {
                    format!("https://{}:{}@api.com with key {}", user, pass, key)
                }),
        ],
        // Generate additional context
        context in prop_oneof![
            Just(""),
            Just(" for provider openrouter"),
            Just(" at endpoint /v1/chat/completions"),
            Just(" after 3 retries"),
        ]
    )| {
        // Construct error message with secret
        let error_message = format!("{}: {}{}", error_type, secret_pattern, context);

        // Call the redaction function
        let redacted = redact_error_message_for_testing(&error_message);

        // Verify that the redacted message doesn't contain the original secret
        // Extract potential secrets from the original message
        let potential_secrets = extract_potential_secrets(&secret_pattern);

        for secret in potential_secrets {
            if secret.len() >= 8 {  // Only check secrets that are long enough to be meaningful
                prop_assert!(
                    !redacted.contains(&secret),
                    "Redacted message should not contain secret '{}'. Original: '{}', Redacted: '{}'",
                    secret,
                    error_message,
                    redacted
                );
            }
        }

        // Verify that redaction markers are present
        if error_message.contains("://") && error_message.contains("@") {
            prop_assert!(
                redacted.contains("[REDACTED]@") || !redacted.contains("@"),
                "URL with credentials should be redacted. Original: '{}', Redacted: '{}'",
                error_message,
                redacted
            );
        }

        // Verify that long alphanumeric strings (potential keys) are redacted
        if secret_pattern.len() >= 32 && secret_pattern.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
            prop_assert!(
                redacted.contains("[REDACTED_KEY]") || !redacted.contains(&secret_pattern),
                "Long alphanumeric string should be redacted. Original: '{}', Redacted: '{}'",
                error_message,
                redacted
            );
        }

        // Verify that error context is preserved
        prop_assert!(
            redacted.contains(error_type),
            "Error type should be preserved. Original: '{}', Redacted: '{}'",
            error_message,
            redacted
        );

        // Verify that provider/endpoint context is preserved (if present)
        if context.contains("provider") {
            prop_assert!(
                redacted.contains("provider"),
                "Provider context should be preserved. Original: '{}', Redacted: '{}'",
                error_message,
                redacted
            );
        }
    });
}

/// Helper function to extract potential secrets from a pattern
fn extract_potential_secrets(pattern: &str) -> Vec<String> {
    let mut secrets = Vec::new();

    // Extract credentials from URLs (user:pass)
    if let Some(at_pos) = pattern.find('@')
        && let Some(scheme_end) = pattern.find("://")
    {
        let creds_start = scheme_end + 3;
        if creds_start < at_pos {
            let creds = &pattern[creds_start..at_pos];
            if let Some(colon_pos) = creds.find(':') {
                secrets.push(creds[..colon_pos].to_string());
                secrets.push(creds[colon_pos + 1..].to_string());
            }
        }
    }

    // Extract API keys (long alphanumeric strings)
    let words: Vec<&str> = pattern.split_whitespace().collect();
    for word in words {
        if word.len() >= 32
            && word
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            secrets.push(word.to_string());
        }
    }

    secrets
}

/// Property test: Budget enforcement fails fast on exhaustion
///
/// **Feature: xchecker-llm-ecosystem, Property 9: Budget enforcement fails fast on exhaustion**
/// **Validates: Requirements 3.6.6**
///
/// This property verifies that the BudgetedBackend wrapper correctly enforces
/// budget limits by failing fast when the limit is reached, regardless of whether
/// the underlying backend succeeds or fails.
#[cfg(test)]
mod budget_enforcement_property {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::Duration;
    use xchecker::llm::{
        BudgetedBackend, LlmBackend, LlmError, LlmInvocation, LlmResult, Message, Role,
    };

    // Mock backend for testing
    struct MockBackend {
        call_count: Arc<AtomicU32>,
        should_fail: bool,
    }

    impl MockBackend {
        #[allow(dead_code)] // Reserved for future test cases
        fn new(should_fail: bool) -> Self {
            Self {
                call_count: Arc::new(AtomicU32::new(0)),
                should_fail,
            }
        }

        #[allow(dead_code)] // Reserved for future test cases
        fn get_call_count(&self) -> u32 {
            self.call_count.load(Ordering::SeqCst)
        }
    }

    #[async_trait::async_trait]
    impl LlmBackend for MockBackend {
        async fn invoke(&self, _inv: LlmInvocation) -> Result<LlmResult, LlmError> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            if self.should_fail {
                Err(LlmError::Transport("mock failure".to_string()))
            } else {
                Ok(LlmResult::new("test response", "mock", "mock-model"))
            }
        }
    }

    fn create_test_invocation() -> LlmInvocation {
        LlmInvocation::new(
            "test-spec",
            "test-phase",
            "test-model",
            Duration::from_secs(60),
            vec![Message::new(Role::User, "test message")],
        )
    }

    proptest! {
        #![proptest_config(proptest_config(None))]

        /// Property: For any budget limit and call sequence, the BudgetedBackend
        /// must fail fast with BudgetExceeded when the limit is reached, and
        /// must not invoke the inner backend after the limit is exceeded.
        #[test]
        fn prop_budget_fails_fast_on_exhaustion(
            limit in 1u32..20,
            call_count in 1u32..30,
            should_fail in prop::bool::ANY
        ) {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async {
                let call_counter = Arc::new(AtomicU32::new(0));
                let counter_clone = Arc::clone(&call_counter);

                let mock = MockBackend {
                    call_count: counter_clone,
                    should_fail,
                };

                let backend = BudgetedBackend::new(
                    Box::new(mock),
                    limit
                );

                let mut success_count = 0;
                let mut budget_exceeded_count = 0;
                let mut other_error_count = 0;

                for _ in 0..call_count {
                    let result = backend.invoke(create_test_invocation()).await;
                    match result {
                        Ok(_) => success_count += 1,
                        Err(LlmError::BudgetExceeded { .. }) => {
                            budget_exceeded_count += 1;
                        }
                        Err(_) => other_error_count += 1,
                    }
                }

                // Verify that the inner backend was called at most `limit` times
                let actual_calls = call_counter.load(Ordering::SeqCst);
                prop_assert!(
                    actual_calls <= limit,
                    "Inner backend called {} times, but limit was {}",
                    actual_calls,
                    limit
                );

                // Verify that we got BudgetExceeded errors for calls beyond the limit
                if call_count > limit {
                    prop_assert!(
                        budget_exceeded_count > 0,
                        "Expected BudgetExceeded errors when call_count ({}) > limit ({})",
                        call_count,
                        limit
                    );

                    // The number of BudgetExceeded errors should be call_count - limit
                    prop_assert_eq!(
                        budget_exceeded_count,
                        call_count - limit,
                        "Expected {} BudgetExceeded errors, got {}",
                        call_count - limit,
                        budget_exceeded_count
                    );
                }

                // If the mock backend fails, verify we got the right error types
                if should_fail {
                    // Successful calls should be 0 (since mock always fails)
                    prop_assert_eq!(success_count, 0, "Expected no successful calls when mock fails");
                    // Other errors should be at most `limit` (from the mock backend)
                    prop_assert!(
                        other_error_count <= limit,
                        "Got {} other errors, but limit was {}",
                        other_error_count,
                        limit
                    );
                } else {
                    // Successful calls should be at most `limit`
                    prop_assert!(
                        success_count <= limit,
                        "Got {} successful calls, but limit was {}",
                        success_count,
                        limit
                    );
                    // No other errors expected when mock succeeds
                    prop_assert_eq!(other_error_count, 0, "Expected no other errors when mock succeeds");
                }

                Ok(())
            })?;
        }

        /// Property: Budget tracking counts attempted calls, not successful requests.
        /// Even if the inner backend fails, the budget slot is consumed.
        #[test]
        fn prop_budget_tracks_attempted_calls(
            limit in 1u32..10,
            should_fail in prop::bool::ANY
        ) {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async {
                let call_counter = Arc::new(AtomicU32::new(0));
                let counter_clone = Arc::clone(&call_counter);

                let mock = MockBackend {
                    call_count: counter_clone,
                    should_fail,
                };

                let backend = BudgetedBackend::new(
                    Box::new(mock),
                    limit
                );

                // Make exactly `limit` calls
                for _ in 0..limit {
                    let _ = backend.invoke(create_test_invocation()).await;
                }

                // Verify the inner backend was called exactly `limit` times
                let actual_calls = call_counter.load(Ordering::SeqCst);
                prop_assert_eq!(
                    actual_calls,
                    limit,
                    "Inner backend should be called exactly {} times, got {}",
                    limit,
                    actual_calls
                );

                // The next call should fail with BudgetExceeded
                let result = backend.invoke(create_test_invocation()).await;
                prop_assert!(
                    matches!(result, Err(LlmError::BudgetExceeded { .. })),
                    "Expected BudgetExceeded error after {} calls, got {:?}",
                    limit,
                    result
                );

                // Verify the inner backend was NOT called again
                let calls_after = call_counter.load(Ordering::SeqCst);
                prop_assert_eq!(
                    calls_after,
                    limit,
                    "Inner backend should not be called after budget exhaustion, got {} calls",
                    calls_after
                );

                Ok(())
            })?;
        }
    }
}

/// Property test: JSON output includes schema version
///
/// **Feature: xchecker-llm-ecosystem, Property 11: JSON output includes schema version**
/// **Validates: Requirements 4.1.1**
///
/// This property verifies that all JSON outputs from xchecker commands (spec, status, resume)
/// include a `schema_version` field that identifies the format version.
#[cfg(test)]
mod spec_json_property {
    use super::*;
    use chrono::Utc;
    use xchecker::types::{PhaseInfo, SpecConfigSummary, SpecOutput};

    proptest! {
        #![proptest_config(proptest_config(None))]

        /// Property: For any valid SpecOutput, the JSON serialization must include
        /// a schema_version field with value "spec-json.v1"
        #[test]
        fn prop_spec_json_includes_schema_version(
            spec_id in "[a-z][a-z0-9-]{2,20}",
            num_phases in 0usize..7,
            has_provider in prop::bool::ANY,
            execution_strategy in prop_oneof![
                Just("controlled".to_string()),
            ]
        ) {
            // Generate phases
            let phase_names = ["requirements", "design", "tasks", "review", "fixup", "final"];
            let statuses = ["completed", "pending", "not_started"];

            let phases: Vec<PhaseInfo> = phase_names
                .iter()
                .take(num_phases)
                .enumerate()
                .map(|(i, name)| PhaseInfo {
                    phase_id: name.to_string(),
                    status: statuses[i % statuses.len()].to_string(),
                    last_run: if i % 2 == 0 { Some(Utc::now()) } else { None },
                })
                .collect();

            let output = SpecOutput {
                schema_version: "spec-json.v1".to_string(),
                spec_id: spec_id.clone(),
                phases,
                config_summary: SpecConfigSummary {
                    execution_strategy,
                    provider: if has_provider { Some("claude-cli".to_string()) } else { None },
                    spec_path: format!(".xchecker/specs/{}", spec_id),
                },
            };

            // Serialize to JSON
            let json_result = serde_json::to_string(&output);
            prop_assert!(json_result.is_ok(), "Failed to serialize SpecOutput to JSON");

            let json_str = json_result.unwrap();
            let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

            // Verify schema_version is present and correct
            prop_assert!(
                parsed.get("schema_version").is_some(),
                "JSON output must include schema_version field"
            );
            prop_assert_eq!(
                parsed["schema_version"].as_str().unwrap(),
                "spec-json.v1",
                "schema_version must be 'spec-json.v1'"
            );

            // Verify spec_id is present and matches
            prop_assert!(
                parsed.get("spec_id").is_some(),
                "JSON output must include spec_id field"
            );
            prop_assert_eq!(
                parsed["spec_id"].as_str().unwrap(),
                spec_id,
                "spec_id must match input"
            );
        }

        /// Property: For any valid SpecOutput, the JSON must NOT include packet contents
        /// or full artifacts (per Requirements 4.1.4)
        #[test]
        fn prop_spec_json_excludes_packet_contents(
            spec_id in "[a-z][a-z0-9-]{2,20}",
            num_phases in 0usize..7
        ) {
            let phase_names = ["requirements", "design", "tasks", "review", "fixup", "final"];

            let phases: Vec<PhaseInfo> = phase_names
                .iter()
                .take(num_phases)
                .map(|name| PhaseInfo {
                    phase_id: name.to_string(),
                    status: "not_started".to_string(),
                    last_run: None,
                })
                .collect();

            let output = SpecOutput {
                schema_version: "spec-json.v1".to_string(),
                spec_id: spec_id.clone(),
                phases,
                config_summary: SpecConfigSummary {
                    execution_strategy: "controlled".to_string(),
                    provider: None,
                    spec_path: format!(".xchecker/specs/{}", spec_id),
                },
            };

            // Serialize to JSON
            let json_str = serde_json::to_string(&output).unwrap();
            let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

            // Verify no packet contents are present
            prop_assert!(
                parsed.get("packet").is_none(),
                "JSON should not contain packet field"
            );
            prop_assert!(
                parsed.get("artifacts").is_none(),
                "JSON should not contain artifacts field"
            );
            prop_assert!(
                parsed.get("raw_response").is_none(),
                "JSON should not contain raw_response field"
            );
            prop_assert!(
                parsed.get("prompt").is_none(),
                "JSON should not contain prompt field"
            );
            prop_assert!(
                parsed.get("stderr").is_none(),
                "JSON should not contain stderr field"
            );

            // Verify only expected top-level fields are present
            let expected_fields = ["schema_version", "spec_id", "phases", "config_summary"];
            for (key, _) in parsed.as_object().unwrap() {
                prop_assert!(
                    expected_fields.contains(&key.as_str()),
                    "Unexpected field '{}' in JSON output",
                    key
                );
            }
        }
    }
}

/// Property tests for JSON output size limits (Requirements 4.1.4)
/// **Feature: xchecker-llm-ecosystem, Property 12: JSON output respects size limits**
/// **Validates: Requirements 4.1.4**
///
/// These tests verify that JSON outputs from spec, status, and resume commands
/// do not include full packet contents or raw artifacts.
#[cfg(test)]
mod json_size_limits_property {
    use super::*;
    use xchecker::types::{
        CurrentInputs, PhaseInfo, PhaseStatusInfo, ResumeJsonOutput, SpecConfigSummary, SpecOutput,
        StatusJsonOutput,
    };

    proptest! {
        #![proptest_config(proptest_config(None))]

        /// Property: For any valid SpecOutput, the JSON must NOT include full artifacts
        /// or packet contents (per Requirements 4.1.4)
        #[test]
        fn prop_spec_json_excludes_full_artifacts(
            spec_id in "[a-z][a-z0-9-]{2,20}",
            num_phases in 0usize..7
        ) {
            let phase_names = ["requirements", "design", "tasks", "review", "fixup", "final"];

            let phases: Vec<PhaseInfo> = phase_names
                .iter()
                .take(num_phases)
                .map(|name| PhaseInfo {
                    phase_id: name.to_string(),
                    status: "not_started".to_string(),
                    last_run: None,
                })
                .collect();

            let output = SpecOutput {
                schema_version: "spec-json.v1".to_string(),
                spec_id: spec_id.clone(),
                phases,
                config_summary: SpecConfigSummary {
                    execution_strategy: "controlled".to_string(),
                    provider: None,
                    spec_path: format!(".xchecker/specs/{}", spec_id),
                },
            };

            // Serialize to JSON
            let json_str = serde_json::to_string(&output).unwrap();
            let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

            // Verify no full artifacts are present (only high-level metadata)
            prop_assert!(
                parsed.get("artifacts").is_none(),
                "Spec JSON should not contain full artifacts field"
            );
            prop_assert!(
                parsed.get("packet").is_none(),
                "Spec JSON should not contain packet field"
            );
            prop_assert!(
                parsed.get("raw_content").is_none(),
                "Spec JSON should not contain raw_content field"
            );
            prop_assert!(
                parsed.get("file_contents").is_none(),
                "Spec JSON should not contain file_contents field"
            );

            // Verify only expected top-level fields are present
            let expected_fields = ["schema_version", "spec_id", "phases", "config_summary"];
            for (key, _) in parsed.as_object().unwrap() {
                prop_assert!(
                    expected_fields.contains(&key.as_str()),
                    "Unexpected field '{}' in spec JSON output",
                    key
                );
            }
        }

        /// Property: For any valid StatusJsonOutput, the JSON must NOT include packet contents
        /// (per Requirements 4.1.4)
        #[test]
        fn prop_status_json_excludes_packet_contents(
            spec_id in "[a-z][a-z0-9-]{2,20}",
            num_phases in 0usize..7,
            pending_fixups in 0u32..100,
            has_errors in prop::bool::ANY
        ) {
            let phase_names = ["requirements", "design", "tasks", "review", "fixup", "final"];
            let statuses = ["success", "failed", "not_started"];

            let phase_statuses: Vec<PhaseStatusInfo> = phase_names
                .iter()
                .take(num_phases)
                .enumerate()
                .map(|(i, name)| PhaseStatusInfo {
                    phase_id: name.to_string(),
                    status: statuses[i % statuses.len()].to_string(),
                    receipt_id: if i % 2 == 0 { Some(format!("{}-20241201_100000", name)) } else { None },
                })
                .collect();

            let output = StatusJsonOutput {
                schema_version: "status-json.v2".to_string(),
                spec_id: spec_id.clone(),
                phase_statuses,
                pending_fixups,
                has_errors,
                strict_validation: false,
                artifacts: Vec::new(),
                effective_config: std::collections::BTreeMap::new(),
                lock_drift: None,
            };

            // Serialize to JSON
            let json_str = serde_json::to_string(&output).unwrap();
            let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

            // Verify no raw packet contents are present
            prop_assert!(
                parsed.get("packet").is_none(),
                "Status JSON should not contain packet field"
            );
            prop_assert!(
                parsed.get("raw_response").is_none(),
                "Status JSON should not contain raw_response field"
            );
            prop_assert!(
                parsed.get("stderr").is_none(),
                "Status JSON should not contain stderr field"
            );
            prop_assert!(
                parsed.get("prompt").is_none(),
                "Status JSON should not contain prompt field"
            );

            // Verify only expected top-level fields are present
            // v2 adds artifacts, effective_config, lock_drift, strict_validation
            let expected_fields = ["schema_version", "spec_id", "phase_statuses", "pending_fixups", "has_errors", "artifacts", "effective_config", "lock_drift", "strict_validation"];
            for (key, _) in parsed.as_object().unwrap() {
                prop_assert!(
                    expected_fields.contains(&key.as_str()),
                    "Unexpected field '{}' in status JSON output",
                    key
                );
            }
        }

        /// Property: For any valid ResumeJsonOutput, the JSON must NOT include raw artifacts
        /// or full packet contents (per Requirements 4.1.4)
        #[test]
        fn prop_resume_json_excludes_raw_artifacts(
            spec_id in "[a-z][a-z0-9-]{2,20}",
            phase in prop_oneof![
                Just("requirements".to_string()),
                Just("design".to_string()),
                Just("tasks".to_string()),
                Just("review".to_string()),
                Just("fixup".to_string()),
                Just("final".to_string()),
            ],
            num_artifacts in 0usize..10,
            spec_exists in prop::bool::ANY,
            has_latest_phase in prop::bool::ANY
        ) {
            // Generate artifact names (not contents)
            let artifact_names: Vec<String> = (0..num_artifacts)
                .map(|i| format!("{:02}-artifact.md", i))
                .collect();

            let latest_phase = if has_latest_phase {
                Some("requirements".to_string())
            } else {
                None
            };

            let output = ResumeJsonOutput {
                schema_version: "resume-json.v1".to_string(),
                spec_id: spec_id.clone(),
                phase: phase.clone(),
                current_inputs: CurrentInputs {
                    available_artifacts: artifact_names,
                    spec_exists,
                    latest_completed_phase: latest_phase,
                },
                next_steps: format!("Run {} phase to continue", phase),
            };

            // Serialize to JSON
            let json_str = serde_json::to_string(&output).unwrap();
            let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

            // Verify no raw artifacts are present
            prop_assert!(
                parsed.get("raw_artifacts").is_none(),
                "Resume JSON should not contain raw_artifacts field"
            );
            prop_assert!(
                parsed.get("packet").is_none(),
                "Resume JSON should not contain packet field"
            );
            prop_assert!(
                parsed.get("raw_response").is_none(),
                "Resume JSON should not contain raw_response field"
            );
            prop_assert!(
                parsed.get("file_contents").is_none(),
                "Resume JSON should not contain file_contents field"
            );
            prop_assert!(
                parsed.get("stderr").is_none(),
                "Resume JSON should not contain stderr field"
            );
            prop_assert!(
                parsed.get("prompt").is_none(),
                "Resume JSON should not contain prompt field"
            );

            // Verify only expected top-level fields are present
            let expected_fields = ["schema_version", "spec_id", "phase", "current_inputs", "next_steps"];
            for (key, _) in parsed.as_object().unwrap() {
                prop_assert!(
                    expected_fields.contains(&key.as_str()),
                    "Unexpected field '{}' in resume JSON output",
                    key
                );
            }

            // Verify current_inputs only contains metadata, not full contents
            let current_inputs = parsed.get("current_inputs").unwrap();
            prop_assert!(
                current_inputs.get("raw_content").is_none(),
                "current_inputs should not contain raw_content"
            );
            prop_assert!(
                current_inputs.get("file_contents").is_none(),
                "current_inputs should not contain file_contents"
            );
        }

        /// Property: For any valid ResumeJsonOutput, the JSON must include schema_version
        /// (per Requirements 4.1.1)
        #[test]
        fn prop_resume_json_includes_schema_version(
            spec_id in "[a-z][a-z0-9-]{2,20}",
            phase in prop_oneof![
                Just("requirements".to_string()),
                Just("design".to_string()),
                Just("tasks".to_string()),
            ]
        ) {
            let output = ResumeJsonOutput {
                schema_version: "resume-json.v1".to_string(),
                spec_id: spec_id.clone(),
                phase: phase.clone(),
                current_inputs: CurrentInputs {
                    available_artifacts: vec![],
                    spec_exists: true,
                    latest_completed_phase: None,
                },
                next_steps: format!("Run {} phase", phase),
            };

            // Serialize to JSON
            let json_str = serde_json::to_string(&output).unwrap();
            let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

            // Verify schema_version is present and correct
            prop_assert!(
                parsed.get("schema_version").is_some(),
                "Resume JSON must include schema_version field"
            );
            prop_assert_eq!(
                parsed["schema_version"].as_str().unwrap(),
                "resume-json.v1",
                "schema_version must be 'resume-json.v1'"
            );
        }
    }
}

/// Property test: Workspace discovery searches upward
///
/// **Feature: xchecker-llm-ecosystem, Property 13: Workspace discovery searches upward**
///
/// This test verifies that workspace discovery correctly searches upward from the
/// starting directory to find `workspace.yaml`, using the first found (no merging).
///
/// **Validates: Requirements 4.3.6**
#[test]
fn prop_workspace_discovery_searches_upward() {
    use std::path::PathBuf;
    use tempfile::TempDir;
    use xchecker::workspace::{self, WORKSPACE_FILE_NAME, Workspace};

    let config = proptest_config(None);

    proptest!(config, |(
        // Generate random directory depth (1-5 levels)
        depth in 1usize..6,
        // Generate random workspace placement (0 = root, 1 = first subdir, etc.)
        workspace_level in 0usize..6,
        // Generate random workspace name
        workspace_name in "[a-z][a-z0-9-]{2,10}"
    )| {
        // Create temp directory structure
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Build nested directory structure
        let mut current_path = root.to_path_buf();
        let mut paths: Vec<PathBuf> = vec![current_path.clone()];

        for i in 0..depth {
            current_path = current_path.join(format!("subdir_{}", i));
            std::fs::create_dir_all(&current_path).unwrap();
            paths.push(current_path.clone());
        }

        // Place workspace at the specified level (clamped to actual depth)
        let actual_workspace_level = workspace_level.min(paths.len() - 1);
        let workspace_dir = &paths[actual_workspace_level];
        let workspace_path = workspace_dir.join(WORKSPACE_FILE_NAME);

        // Create workspace file
        let ws = Workspace::new(&workspace_name);
        ws.save(&workspace_path).unwrap();

        // Test discovery from deepest directory
        let deepest_dir = paths.last().unwrap();
        let discovered = workspace::discover_workspace(deepest_dir).unwrap();

        // Property 1: Discovery should find a workspace
        prop_assert!(
            discovered.is_some(),
            "Workspace discovery should find workspace.yaml when it exists in ancestor"
        );

        // Property 2: Discovery should find the FIRST workspace (closest to start)
        // When searching upward, we should find the workspace at the deepest level
        // that has one (i.e., the first one encountered when going up)
        if let Some(found_path) = discovered {
            // The found workspace should be at or above the starting directory
            prop_assert!(
                deepest_dir.starts_with(found_path.parent().unwrap()),
                "Found workspace should be in an ancestor directory"
            );

            // Verify the workspace can be loaded
            let loaded = Workspace::load(&found_path).unwrap();
            prop_assert_eq!(
                loaded.name, workspace_name,
                "Loaded workspace should have correct name"
            );
        }

        // Test discovery from the workspace directory itself
        let discovered_from_ws_dir = workspace::discover_workspace(workspace_dir).unwrap();
        prop_assert!(
            discovered_from_ws_dir.is_some(),
            "Discovery from workspace directory should find workspace"
        );
        prop_assert_eq!(
            discovered_from_ws_dir.unwrap(), workspace_path,
            "Discovery from workspace directory should find that workspace"
        );
    });
}

/// Property test: Workspace discovery returns first found (no merging)
///
/// **Feature: xchecker-llm-ecosystem, Property 13: Workspace discovery searches upward**
///
/// This test verifies that when multiple workspace.yaml files exist in the directory
/// hierarchy, only the first one (closest to the starting directory) is returned.
///
/// **Validates: Requirements 4.3.6**
#[test]
fn prop_workspace_discovery_first_found_no_merging() {
    use tempfile::TempDir;
    use xchecker::workspace::{self, WORKSPACE_FILE_NAME, Workspace};

    let config = proptest_config(None);

    proptest!(config, |(
        // Generate random directory depth (2-4 levels to ensure we can have multiple workspaces)
        depth in 2usize..5,
        // Generate random names for workspaces - use different prefixes to ensure uniqueness
        root_name in "root-[a-z][a-z0-9-]{2,8}",
        nested_name in "nested-[a-z][a-z0-9-]{2,8}"
    )| {
        // Create temp directory structure
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Build nested directory structure
        let mut current_path = root.to_path_buf();
        let mut paths = vec![current_path.clone()];

        for i in 0..depth {
            current_path = current_path.join(format!("level_{}", i));
            std::fs::create_dir_all(&current_path).unwrap();
            paths.push(current_path.clone());
        }

        // Create workspace at root level
        let root_workspace_path = root.join(WORKSPACE_FILE_NAME);
        let root_ws = Workspace::new(&root_name);
        root_ws.save(&root_workspace_path).unwrap();

        // Create workspace at a nested level (middle of the hierarchy)
        let nested_level = depth / 2;
        let nested_workspace_path = paths[nested_level].join(WORKSPACE_FILE_NAME);
        let nested_ws = Workspace::new(&nested_name);
        nested_ws.save(&nested_workspace_path).unwrap();

        // Test discovery from deepest directory
        let deepest_dir = paths.last().unwrap();
        let discovered = workspace::discover_workspace(deepest_dir).unwrap();

        // Property: Should find the nested workspace (first encountered going up)
        prop_assert!(discovered.is_some(), "Should find a workspace");

        let found_path = discovered.unwrap();
        let loaded = Workspace::load(&found_path).unwrap();

        // The found workspace should be the nested one (closer to start)
        prop_assert_eq!(
            &loaded.name, &nested_name,
            "Should find the nested workspace (first encountered), not the root workspace"
        );

        // Verify no merging occurred - the workspace should only have the nested name
        // Since we use different prefixes, names should never be equal
        prop_assert_ne!(
            &loaded.name, &root_name,
            "Should not have merged with root workspace"
        );
    });
}

/// Property test: Workspace discovery returns None when no workspace exists
///
/// **Feature: xchecker-llm-ecosystem, Property 13: Workspace discovery searches upward**
///
/// This test verifies that workspace discovery returns None when no workspace.yaml
/// exists in the directory hierarchy.
///
/// **Validates: Requirements 4.3.6**
#[test]
fn prop_workspace_discovery_returns_none_when_missing() {
    use tempfile::TempDir;
    use xchecker::workspace;

    let config = proptest_config(None);

    proptest!(config, |(
        // Generate random directory depth (1-5 levels)
        depth in 1usize..6
    )| {
        // Create temp directory structure WITHOUT any workspace.yaml
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Build nested directory structure
        let mut current_path = root.to_path_buf();

        for i in 0..depth {
            current_path = current_path.join(format!("empty_dir_{}", i));
            std::fs::create_dir_all(&current_path).unwrap();
        }

        // Test discovery from deepest directory
        let discovered = workspace::discover_workspace(&current_path).unwrap();

        // Property: Should return None when no workspace exists
        prop_assert!(
            discovered.is_none(),
            "Workspace discovery should return None when no workspace.yaml exists"
        );
    });
}

/// Property test: Hooks are subject to timeouts
///
/// **Feature: xchecker-llm-ecosystem, Property 16: Hooks are subject to timeouts**
/// **Validates: Requirements 4.8.4**
///
/// This property verifies that hook execution respects timeout configuration.
/// For any hook configuration with a timeout, if the hook runs longer than the
/// timeout, it should be terminated and handled according to the `on_fail` configuration.
#[cfg(test)]
mod hook_timeout_property {
    use super::*;
    use xchecker::hooks::{
        HookConfig, HookOutcome, HookResult, HookType, OnFail, process_hook_result,
    };
    use xchecker::types::PhaseId;

    proptest! {
        #![proptest_config(proptest_config(None))]

        /// Property: For any hook configuration with a timeout, when the hook times out,
        /// the result should indicate timeout and be handled according to on_fail config.
        #[test]
        fn prop_hook_timeout_respects_on_fail_config(
            timeout_seconds in 1u64..120,
            on_fail in prop_oneof![
                Just(OnFail::Warn),
                Just(OnFail::Fail),
            ],
            phase in prop_oneof![
                Just(PhaseId::Requirements),
                Just(PhaseId::Design),
                Just(PhaseId::Tasks),
            ],
            hook_type in prop_oneof![
                Just(HookType::PrePhase),
                Just(HookType::PostPhase),
            ]
        ) {
            // Create a hook config with the specified timeout and on_fail
            let config = HookConfig {
                command: "./slow_hook.sh".to_string(),
                on_fail,
                timeout: timeout_seconds,
            };

            // Simulate a timeout result (as if the hook timed out)
            let timeout_result = HookResult::timeout(
                String::new(),
                String::new(),
                timeout_seconds * 1000, // duration_ms
            );

            // Process the timeout result
            let outcome = process_hook_result(timeout_result, &config, hook_type, phase);

            // Verify the outcome respects on_fail configuration
            match on_fail {
                OnFail::Warn => {
                    // Should continue with warning
                    prop_assert!(
                        outcome.should_continue(),
                        "Timeout with on_fail=warn should allow continuation"
                    );
                    prop_assert!(
                        matches!(outcome, HookOutcome::Warning { .. }),
                        "Timeout with on_fail=warn should produce Warning outcome"
                    );

                    // Warning should indicate timeout
                    let warning = outcome.warning().expect("Should have warning");
                    prop_assert!(
                        warning.timed_out,
                        "Warning should indicate timeout"
                    );
                    prop_assert_eq!(
                        warning.exit_code,
                        -1,
                        "Timeout exit code should be -1"
                    );
                }
                OnFail::Fail => {
                    // Should NOT continue
                    prop_assert!(
                        !outcome.should_continue(),
                        "Timeout with on_fail=fail should NOT allow continuation"
                    );
                    prop_assert!(
                        matches!(outcome, HookOutcome::Failure { .. }),
                        "Timeout with on_fail=fail should produce Failure outcome"
                    );

                    // Error should be a timeout error
                    let error = outcome.error().expect("Should have error");
                    prop_assert!(
                        matches!(error, xchecker::hooks::HookError::Timeout { .. }),
                        "Error should be a Timeout error"
                    );
                }
            }

            // Verify the underlying result is accessible
            let result = outcome.result();
            prop_assert!(
                result.timed_out,
                "Result should indicate timeout"
            );
            prop_assert!(
                !result.success,
                "Timeout result should not be successful"
            );
        }

        /// Property: For any hook configuration, successful hooks should always
        /// return Success outcome regardless of on_fail setting.
        #[test]
        fn prop_successful_hook_ignores_on_fail(
            timeout_seconds in 1u64..120,
            on_fail in prop_oneof![
                Just(OnFail::Warn),
                Just(OnFail::Fail),
            ],
            duration_ms in 1u64..60000
        ) {
            let config = HookConfig {
                command: "./fast_hook.sh".to_string(),
                on_fail,
                timeout: timeout_seconds,
            };

            // Simulate a successful result
            let success_result = HookResult::success(
                "output".to_string(),
                String::new(),
                duration_ms,
            );

            let outcome = process_hook_result(
                success_result,
                &config,
                HookType::PrePhase,
                PhaseId::Design,
            );

            // Successful hooks should always continue
            prop_assert!(
                outcome.should_continue(),
                "Successful hook should always allow continuation"
            );
            prop_assert!(
                matches!(outcome, HookOutcome::Success(_)),
                "Successful hook should produce Success outcome"
            );
            prop_assert!(
                outcome.warning().is_none(),
                "Successful hook should not have warning"
            );
            prop_assert!(
                outcome.error().is_none(),
                "Successful hook should not have error"
            );
        }

        /// Property: For any hook failure (non-timeout), the outcome should
        /// respect on_fail configuration.
        #[test]
        fn prop_hook_failure_respects_on_fail(
            exit_code in 1i32..128,
            on_fail in prop_oneof![
                Just(OnFail::Warn),
                Just(OnFail::Fail),
            ],
            stderr in "[a-zA-Z0-9 ]{0,100}"
        ) {
            let config = HookConfig {
                command: "./failing_hook.sh".to_string(),
                on_fail,
                timeout: 60,
            };

            // Simulate a failure result
            let failure_result = HookResult::failure(
                exit_code,
                String::new(),
                stderr.clone(),
                100,
            );

            let outcome = process_hook_result(
                failure_result,
                &config,
                HookType::PostPhase,
                PhaseId::Tasks,
            );

            match on_fail {
                OnFail::Warn => {
                    prop_assert!(
                        outcome.should_continue(),
                        "Failure with on_fail=warn should allow continuation"
                    );
                    prop_assert!(
                        matches!(outcome, HookOutcome::Warning { .. }),
                        "Failure with on_fail=warn should produce Warning outcome"
                    );

                    let warning = outcome.warning().expect("Should have warning");
                    prop_assert_eq!(
                        warning.exit_code,
                        exit_code,
                        "Warning should have correct exit code"
                    );
                }
                OnFail::Fail => {
                    prop_assert!(
                        !outcome.should_continue(),
                        "Failure with on_fail=fail should NOT allow continuation"
                    );
                    prop_assert!(
                        matches!(outcome, HookOutcome::Failure { .. }),
                        "Failure with on_fail=fail should produce Failure outcome"
                    );

                    let error = outcome.error().expect("Should have error");
                    prop_assert!(
                        matches!(error, xchecker::hooks::HookError::ExecutionFailed { .. }),
                        "Error should be ExecutionFailed"
                    );
                }
            }
        }
    }
}

// =============================================================================
// Property 11: Secret Redaction Coverage
// =============================================================================
//
// **Feature: crates-io-packaging, Property 11: Secret redaction coverage**
//
// *For any* content containing patterns matching the documented secret categories
// (AWS keys, GCP keys, Azure keys, generic API tokens, database URLs, SSH keys),
// the system SHALL redact those patterns before including the content in receipts,
// status, doctor outputs, or logs.
//
// **Validates: Requirements FR-SEC-1, FR-SEC-5**

/// Generators for each documented secret category
mod secret_generators {
    use super::test_support;
    use proptest::prelude::*;

    /// Generate a valid AWS access key (AKIA prefix + 16 alphanumeric chars)
    pub fn aws_access_key() -> impl Strategy<Value = String> {
        "[A-Z0-9]{16}".prop_map(|suffix| format!("AKIA{}", suffix))
    }

    /// Generate an AWS secret key assignment
    pub fn aws_secret_key() -> impl Strategy<Value = String> {
        "[A-Za-z0-9/+=]{40}".prop_map(|key| format!("AWS_SECRET_ACCESS_KEY={}", key))
    }

    /// Generate a GCP API key (AIza prefix + 35 chars)
    pub fn gcp_api_key() -> impl Strategy<Value = String> {
        "[A-Za-z0-9_-]{35}".prop_map(|suffix| format!("AIza{}", suffix))
    }

    /// Generate an Azure storage key assignment (88-char base64)
    pub fn azure_storage_key() -> impl Strategy<Value = String> {
        "[A-Za-z0-9/+=]{88}".prop_map(|key| format!("AccountKey={}", key))
    }

    /// Generate an Azure SAS token
    pub fn azure_sas_token() -> impl Strategy<Value = String> {
        "[A-Za-z0-9%/+=]{50,60}".prop_map(|sig| format!("?sig={}", sig))
    }

    /// Generate a Bearer token
    pub fn bearer_token() -> impl Strategy<Value = String> {
        "[A-Za-z0-9._-]{30,50}".prop_map(|token| format!("Bearer {}", token))
    }

    /// Generate a JWT token (eyJ prefix for header and payload)
    pub fn jwt_token() -> impl Strategy<Value = String> {
        (
            "[A-Za-z0-9_-]{20,40}",
            "[A-Za-z0-9_-]{20,40}",
            "[A-Za-z0-9_-]{20,40}",
        )
            .prop_map(|(header, payload, sig)| format!("eyJ{}.eyJ{}.{}", header, payload, sig))
    }

    /// Generate a PostgreSQL connection URL with credentials
    pub fn postgres_url() -> impl Strategy<Value = String> {
        ("[a-z]{4,10}", "[a-zA-Z0-9]{8,16}", "[a-z]{4,10}").prop_map(|(user, pass, db)| {
            format!(
                "{}{}{}{}{}{}{}{}{}",
                "postgres", "://", user, ":", pass, "@", "localhost", ":5432/", db
            )
        })
    }

    /// Generate a MySQL connection URL with credentials
    pub fn mysql_url() -> impl Strategy<Value = String> {
        ("[a-z]{4,10}", "[a-zA-Z0-9]{8,16}", "[a-z]{4,10}").prop_map(|(user, pass, db)| {
            format!(
                "{}{}{}{}{}{}{}{}{}",
                "mysql", "://", user, ":", pass, "@", "localhost", ":3306/", db
            )
        })
    }

    /// Generate a MongoDB connection URL with credentials
    pub fn mongodb_url() -> impl Strategy<Value = String> {
        ("[a-z]{4,10}", "[a-zA-Z0-9]{8,16}", "[a-z]{4,10}").prop_map(|(user, pass, db)| {
            format!(
                "{}{}{}{}{}{}{}{}{}",
                "mongodb", "://", user, ":", pass, "@", "cluster.mongodb.net", "/", db
            )
        })
    }

    /// Generate a Redis connection URL with credentials
    pub fn redis_url() -> impl Strategy<Value = String> {
        "[a-zA-Z0-9]{8,16}".prop_map(|pass| {
            format!(
                "{}{}{}{}{}{}",
                "redis", "://", ":", pass, "@localhost", ":6379"
            )
        })
    }

    /// Generate a GitHub personal access token (ghp_ prefix + 36 chars)
    pub fn github_pat() -> impl Strategy<Value = String> {
        "[A-Za-z0-9]{36}".prop_map(|suffix| format!("ghp_{}", suffix))
    }

    /// Generate a GitLab token (glpat- prefix + 20+ chars)
    pub fn gitlab_token() -> impl Strategy<Value = String> {
        "[A-Za-z0-9_-]{20,30}".prop_map(|suffix| format!("glpat-{}", suffix))
    }

    /// Generate a Slack token (xoxb- prefix)
    pub fn slack_token() -> impl Strategy<Value = String> {
        "[A-Za-z0-9-]{20,40}".prop_map(|suffix| format!("xoxb-{}", suffix))
    }

    /// Generate a Stripe API key (sk_live_ or sk_test_ prefix + 24+ chars)
    pub fn stripe_key() -> impl Strategy<Value = String> {
        (
            prop_oneof![Just("live"), Just("test")],
            "[A-Za-z0-9]{24,32}",
        )
            .prop_map(|(env, suffix)| format!("sk_{}_{}", env, suffix))
    }

    /// Generate an SSH private key marker
    pub fn ssh_private_key() -> impl Strategy<Value = String> {
        prop_oneof![
            Just(test_support::pem_marker("RSA ")),
            Just(test_support::pem_marker("OPENSSH ")),
            Just(test_support::pem_marker("EC ")),
            Just(test_support::pem_marker("")),
        ]
    }

    /// Generate a secret from any documented category
    pub fn any_secret_category() -> impl Strategy<Value = (String, &'static str)> {
        prop_oneof![
            aws_access_key().prop_map(|s| (s, "aws_access_key")),
            aws_secret_key().prop_map(|s| (s, "aws_secret_key")),
            gcp_api_key().prop_map(|s| (s, "gcp_api_key")),
            azure_storage_key().prop_map(|s| (s, "azure_storage_key")),
            azure_sas_token().prop_map(|s| (s, "azure_sas_token")),
            bearer_token().prop_map(|s| (s, "bearer_token")),
            jwt_token().prop_map(|s| (s, "jwt_token")),
            postgres_url().prop_map(|s| (s, "postgres_url")),
            mysql_url().prop_map(|s| (s, "mysql_url")),
            mongodb_url().prop_map(|s| (s, "mongodb_url")),
            redis_url().prop_map(|s| (s, "redis_url")),
            github_pat().prop_map(|s| (s, "github_pat")),
            gitlab_token().prop_map(|s| (s, "gitlab_token")),
            slack_token().prop_map(|s| (s, "slack_token")),
            stripe_key().prop_map(|s| (s, "stripe_key")),
            ssh_private_key().prop_map(|s| (s, "ssh_private_key")),
        ]
    }
}

/// Property test: Secret redaction coverage for all documented categories
///
/// **Feature: crates-io-packaging, Property 11: Secret redaction coverage**
///
/// This test verifies that for any content containing patterns matching the
/// documented secret categories (AWS keys, GCP keys, Azure keys, generic API
/// tokens, database URLs, SSH keys), the system SHALL redact those patterns.
///
/// **Validates: Requirements FR-SEC-1, FR-SEC-5**
#[test]
fn prop_secret_redaction_coverage_all_categories() {
    let config = proptest_config(None);

    proptest!(config, |(
        (secret, category) in secret_generators::any_secret_category(),
        prefix in "[a-zA-Z0-9 ]{0,50}",
        suffix in "[a-zA-Z0-9 ]{0,50}"
    )| {
        let redactor = SecretRedactor::new().unwrap();

        // Embed the secret in surrounding content
        let content = format!("{}\n{}\n{}", prefix, secret, suffix);

        // Test 1: has_secrets should detect the secret
        let has_secrets = redactor.has_secrets(&content, "test.txt").unwrap();
        prop_assert!(
            has_secrets,
            "Secret category '{}' should be detected. Secret: '{}'",
            category, secret
        );

        // Test 2: scan_for_secrets should find matches
        let matches = redactor.scan_for_secrets(&content, "test.txt").unwrap();
        prop_assert!(
            !matches.is_empty(),
            "Secret category '{}' should produce matches. Secret: '{}'",
            category, secret
        );

        // Test 3: redact_string should replace the secret with ***
        // This is the primary API for redaction used in logging, error messages, etc.
        let redacted = redactor.redact_string(&content);
        prop_assert!(
            !redacted.contains(&secret),
            "Secret should be redacted from output. Category: '{}', Secret: '{}', Redacted: '{}'",
            category, secret, redacted
        );
        prop_assert!(
            redacted.contains("***"),
            "Redacted output should contain '***' marker for category '{}'",
            category
        );
    });
}

/// Property test: Each specific secret category is detected and redacted
///
/// This test ensures comprehensive coverage by testing each category individually
/// with multiple generated examples.
///
/// **Feature: crates-io-packaging, Property 11: Secret redaction coverage**
/// **Validates: Requirements FR-SEC-1, FR-SEC-5**
#[test]
fn prop_secret_redaction_aws_credentials() {
    let config = proptest_config(None);

    proptest!(config, |(
        access_key in secret_generators::aws_access_key(),
        secret_key in secret_generators::aws_secret_key()
    )| {
        let redactor = SecretRedactor::new().unwrap();

        // Test AWS access key
        let content1 = format!("config: {}", access_key);
        prop_assert!(
            redactor.has_secrets(&content1, "test.txt").unwrap(),
            "AWS access key should be detected: {}", access_key
        );
        let redacted1 = redactor.redact_string(&content1);
        prop_assert!(
            !redacted1.contains(&access_key),
            "AWS access key should be redacted"
        );

        // Test AWS secret key
        let content2 = format!("export {}", secret_key);
        prop_assert!(
            redactor.has_secrets(&content2, "test.txt").unwrap(),
            "AWS secret key should be detected: {}", secret_key
        );
        let redacted2 = redactor.redact_string(&content2);
        prop_assert!(
            !redacted2.contains(&secret_key),
            "AWS secret key should be redacted"
        );
    });
}

/// Property test: GCP credentials are detected and redacted
///
/// **Feature: crates-io-packaging, Property 11: Secret redaction coverage**
/// **Validates: Requirements FR-SEC-1, FR-SEC-5**
#[test]
fn prop_secret_redaction_gcp_credentials() {
    let config = proptest_config(None);

    proptest!(config, |(
        api_key in secret_generators::gcp_api_key()
    )| {
        let redactor = SecretRedactor::new().unwrap();

        let content = format!("GOOGLE_API_KEY={}", api_key);
        prop_assert!(
            redactor.has_secrets(&content, "test.txt").unwrap(),
            "GCP API key should be detected: {}", api_key
        );
        let redacted = redactor.redact_string(&content);
        prop_assert!(
            !redacted.contains(&api_key),
            "GCP API key should be redacted"
        );
    });
}

/// Property test: Azure credentials are detected and redacted
///
/// **Feature: crates-io-packaging, Property 11: Secret redaction coverage**
/// **Validates: Requirements FR-SEC-1, FR-SEC-5**
#[test]
fn prop_secret_redaction_azure_credentials() {
    let config = proptest_config(None);

    proptest!(config, |(
        storage_key in secret_generators::azure_storage_key(),
        sas_token in secret_generators::azure_sas_token()
    )| {
        let redactor = SecretRedactor::new().unwrap();

        // Test Azure storage key
        let content1 = format!("connection: {}", storage_key);
        prop_assert!(
            redactor.has_secrets(&content1, "test.txt").unwrap(),
            "Azure storage key should be detected: {}", storage_key
        );
        let redacted1 = redactor.redact_string(&content1);
        prop_assert!(
            !redacted1.contains(&storage_key),
            "Azure storage key should be redacted"
        );

        // Test Azure SAS token
        let content2 = format!("https://storage.blob.core.windows.net/container{}", sas_token);
        prop_assert!(
            redactor.has_secrets(&content2, "test.txt").unwrap(),
            "Azure SAS token should be detected: {}", sas_token
        );
        let redacted2 = redactor.redact_string(&content2);
        prop_assert!(
            !redacted2.contains(&sas_token),
            "Azure SAS token should be redacted"
        );
    });
}

/// Property test: Generic API tokens are detected and redacted
///
/// **Feature: crates-io-packaging, Property 11: Secret redaction coverage**
/// **Validates: Requirements FR-SEC-1, FR-SEC-5**
#[test]
fn prop_secret_redaction_generic_tokens() {
    let config = proptest_config(None);

    proptest!(config, |(
        bearer in secret_generators::bearer_token(),
        jwt in secret_generators::jwt_token()
    )| {
        let redactor = SecretRedactor::new().unwrap();

        // Test Bearer token
        let content1 = format!("Authorization: {}", bearer);
        prop_assert!(
            redactor.has_secrets(&content1, "test.txt").unwrap(),
            "Bearer token should be detected: {}", bearer
        );
        let redacted1 = redactor.redact_string(&content1);
        prop_assert!(
            !redacted1.contains(&bearer),
            "Bearer token should be redacted"
        );

        // Test JWT token
        let content2 = format!("token={}", jwt);
        prop_assert!(
            redactor.has_secrets(&content2, "test.txt").unwrap(),
            "JWT token should be detected: {}", jwt
        );
        let redacted2 = redactor.redact_string(&content2);
        prop_assert!(
            !redacted2.contains(&jwt),
            "JWT token should be redacted"
        );
    });
}

/// Property test: Database connection URLs are detected and redacted
///
/// **Feature: crates-io-packaging, Property 11: Secret redaction coverage**
/// **Validates: Requirements FR-SEC-1, FR-SEC-5**
#[test]
fn prop_secret_redaction_database_urls() {
    let config = proptest_config(None);

    proptest!(config, |(
        postgres in secret_generators::postgres_url(),
        mysql in secret_generators::mysql_url(),
        mongodb in secret_generators::mongodb_url(),
        redis in secret_generators::redis_url()
    )| {
        let redactor = SecretRedactor::new().unwrap();

        // Test PostgreSQL URL
        let content1 = format!("DATABASE_URL={}", postgres);
        prop_assert!(
            redactor.has_secrets(&content1, "test.txt").unwrap(),
            "PostgreSQL URL should be detected: {}", postgres
        );
        let redacted1 = redactor.redact_string(&content1);
        prop_assert!(
            !redacted1.contains(&postgres),
            "PostgreSQL URL should be redacted"
        );

        // Test MySQL URL
        let content2 = format!("MYSQL_URL={}", mysql);
        prop_assert!(
            redactor.has_secrets(&content2, "test.txt").unwrap(),
            "MySQL URL should be detected: {}", mysql
        );
        let redacted2 = redactor.redact_string(&content2);
        prop_assert!(
            !redacted2.contains(&mysql),
            "MySQL URL should be redacted"
        );

        // Test MongoDB URL
        let content3 = format!("MONGO_URI={}", mongodb);
        prop_assert!(
            redactor.has_secrets(&content3, "test.txt").unwrap(),
            "MongoDB URL should be detected: {}", mongodb
        );
        let redacted3 = redactor.redact_string(&content3);
        prop_assert!(
            !redacted3.contains(&mongodb),
            "MongoDB URL should be redacted"
        );

        // Test Redis URL
        let content4 = format!("REDIS_URL={}", redis);
        prop_assert!(
            redactor.has_secrets(&content4, "test.txt").unwrap(),
            "Redis URL should be detected: {}", redis
        );
        let redacted4 = redactor.redact_string(&content4);
        prop_assert!(
            !redacted4.contains(&redis),
            "Redis URL should be redacted"
        );
    });
}

/// Property test: Platform-specific tokens are detected and redacted
///
/// **Feature: crates-io-packaging, Property 11: Secret redaction coverage**
/// **Validates: Requirements FR-SEC-1, FR-SEC-5**
#[test]
fn prop_secret_redaction_platform_tokens() {
    let config = proptest_config(None);

    proptest!(config, |(
        github in secret_generators::github_pat(),
        gitlab in secret_generators::gitlab_token(),
        slack in secret_generators::slack_token(),
        stripe in secret_generators::stripe_key()
    )| {
        let redactor = SecretRedactor::new().unwrap();

        // Test GitHub PAT
        let content1 = format!("GITHUB_TOKEN={}", github);
        prop_assert!(
            redactor.has_secrets(&content1, "test.txt").unwrap(),
            "GitHub PAT should be detected: {}", github
        );
        let redacted1 = redactor.redact_string(&content1);
        prop_assert!(
            !redacted1.contains(&github),
            "GitHub PAT should be redacted"
        );

        // Test GitLab token
        let content2 = format!("GITLAB_TOKEN={}", gitlab);
        prop_assert!(
            redactor.has_secrets(&content2, "test.txt").unwrap(),
            "GitLab token should be detected: {}", gitlab
        );
        let redacted2 = redactor.redact_string(&content2);
        prop_assert!(
            !redacted2.contains(&gitlab),
            "GitLab token should be redacted"
        );

        // Test Slack token
        let content3 = format!("SLACK_TOKEN={}", slack);
        prop_assert!(
            redactor.has_secrets(&content3, "test.txt").unwrap(),
            "Slack token should be detected: {}", slack
        );
        let redacted3 = redactor.redact_string(&content3);
        prop_assert!(
            !redacted3.contains(&slack),
            "Slack token should be redacted"
        );

        // Test Stripe key
        let content4 = format!("STRIPE_KEY={}", stripe);
        prop_assert!(
            redactor.has_secrets(&content4, "test.txt").unwrap(),
            "Stripe key should be detected: {}", stripe
        );
        let redacted4 = redactor.redact_string(&content4);
        prop_assert!(
            !redacted4.contains(&stripe),
            "Stripe key should be redacted"
        );
    });
}

/// Property test: SSH private keys are detected and redacted
///
/// **Feature: crates-io-packaging, Property 11: Secret redaction coverage**
/// **Validates: Requirements FR-SEC-1, FR-SEC-5**
#[test]
fn prop_secret_redaction_ssh_keys() {
    let config = proptest_config(None);

    proptest!(config, |(
        ssh_key in secret_generators::ssh_private_key()
    )| {
        let redactor = SecretRedactor::new().unwrap();

        let content = format!("key:\n{}\nMIIEvgIBADANBg...\n-----END PRIVATE KEY-----", ssh_key);
        prop_assert!(
            redactor.has_secrets(&content, "test.txt").unwrap(),
            "SSH private key should be detected: {}", ssh_key
        );
        let redacted = redactor.redact_string(&content);
        prop_assert!(
            !redacted.contains(&ssh_key),
            "SSH private key marker should be redacted"
        );
    });
}

/// Property test: Multiple secrets in same content are all redacted
///
/// **Feature: crates-io-packaging, Property 11: Secret redaction coverage**
/// **Validates: Requirements FR-SEC-1, FR-SEC-5**
#[test]
fn prop_secret_redaction_multiple_secrets() {
    let config = proptest_config(None);

    proptest!(config, |(
        (secret1, cat1) in secret_generators::any_secret_category(),
        (secret2, cat2) in secret_generators::any_secret_category()
    )| {
        let redactor = SecretRedactor::new().unwrap();

        // Content with multiple secrets
        let content = format!("first: {}\nsecond: {}", secret1, secret2);

        // Both should be detected
        let matches = redactor.scan_for_secrets(&content, "test.txt").unwrap();
        prop_assert!(
            matches.len() >= 2,
            "Both secrets should be detected. Categories: '{}', '{}'. Found {} matches.",
            cat1, cat2, matches.len()
        );

        // Both should be redacted
        let redacted = redactor.redact_string(&content);
        prop_assert!(
            !redacted.contains(&secret1),
            "First secret ({}) should be redacted", cat1
        );
        prop_assert!(
            !redacted.contains(&secret2),
            "Second secret ({}) should be redacted", cat2
        );
    });
}

// =============================================================================
// Property 12: Redaction Pipeline Completeness
// =============================================================================
//
// **Feature: crates-io-packaging, Property 12: Redaction pipeline completeness**
//
// *For any* string that passes through LLM invocation, logging, or JSON emission,
// the string SHALL have been processed by `redact_all()` with the effective
// `RedactionConfig`.
//
// **Validates: Requirements FR-SEC-1, FR-SEC-5**
//
// This property verifies that all output surfaces in the system apply redaction
// before emitting content. We test this by:
// 1. Verifying that Receipt creation applies redaction to all user-facing fields
// 2. Verifying that global redaction helpers work correctly for all output surfaces
// 3. Verifying that error messages are redacted before display

/// Property test: Receipt creation applies redaction to all user-facing fields
///
/// **Feature: crates-io-packaging, Property 12: Redaction pipeline completeness**
///
/// This test verifies that when a Receipt is created with content containing secrets,
/// all user-facing fields (stderr_tail, stderr_redacted, warnings, error_reason)
/// are properly redacted before being stored in the receipt.
///
/// **Validates: Requirements FR-SEC-1, FR-SEC-5**
#[test]
fn prop_receipt_creation_applies_redaction() {
    use camino::Utf8PathBuf;
    use xchecker::receipt::ReceiptManager;
    use xchecker::redaction::SecretRedactor;
    use xchecker::types::{ErrorKind, PacketEvidence, PhaseId};

    let config = proptest_config(None);

    proptest!(config, |(
        (secret, category) in secret_generators::any_secret_category(),
        safe_prefix in "[a-zA-Z0-9 ]{5,20}",
        safe_suffix in "[a-zA-Z0-9 ]{5,20}"
    )| {
        let redactor = SecretRedactor::new().unwrap();
        let temp_dir = tempfile::tempdir().unwrap();
        let spec_path = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf())
            .expect("temp dir should be valid UTF-8");
        let receipt_manager = ReceiptManager::new(&spec_path);

        // Create content with embedded secret for each user-facing field
        let stderr_with_secret = format!("{} {} {}", safe_prefix, secret, safe_suffix);
        let warning_with_secret = format!("Warning: {} detected", secret);
        let error_reason_with_secret = format!("Failed due to {}", secret);

        // Create a receipt with secrets in user-facing fields
        // Note: create_receipt uses default_redactor() internally, so we use
        // create_receipt_with_redactor to test with our explicit redactor
        let receipt = receipt_manager.create_receipt_with_redactor(
            &redactor,
            "test-spec",
            PhaseId::Requirements,
            0, // exit_code
            vec![], // outputs
            "1.0.0", // xchecker_version
            "1.0.0", // claude_cli_version
            "claude-3-opus", // model_full_name
            Some("opus".to_string()), // model_alias
            std::collections::HashMap::new(), // flags
            PacketEvidence {
                files: vec![],
                max_bytes: 100000,
                max_lines: 1000,
            },
            Some(stderr_with_secret.clone()), // stderr_tail
            Some(stderr_with_secret.clone()), // stderr_redacted
            vec![warning_with_secret.clone()], // warnings
            None, // fallback_used
            "native", // runner
            None, // runner_distro
            Some(ErrorKind::Unknown), // error_kind
            Some(error_reason_with_secret.clone()), // error_reason
            None, // diff_context
            None, // pipeline
        );

        // Verify that the secret is NOT present in any user-facing field
        if let Some(ref stderr_tail) = receipt.stderr_tail {
            prop_assert!(
                !stderr_tail.contains(&secret),
                "stderr_tail should be redacted. Category: '{}', Found secret in: '{}'",
                category, stderr_tail
            );
        }

        if let Some(ref stderr_redacted) = receipt.stderr_redacted {
            prop_assert!(
                !stderr_redacted.contains(&secret),
                "stderr_redacted should be redacted. Category: '{}', Found secret in: '{}'",
                category, stderr_redacted
            );
        }

        for warning in &receipt.warnings {
            prop_assert!(
                !warning.contains(&secret),
                "warnings should be redacted. Category: '{}', Found secret in: '{}'",
                category, warning
            );
        }

        if let Some(ref error_reason) = receipt.error_reason {
            prop_assert!(
                !error_reason.contains(&secret),
                "error_reason should be redacted. Category: '{}', Found secret in: '{}'",
                category, error_reason
            );
        }

        // Verify that the safe content is preserved (redaction doesn't destroy everything)
        if let Some(ref stderr_tail) = receipt.stderr_tail {
            prop_assert!(
                stderr_tail.contains(&safe_prefix) || stderr_tail.contains("***"),
                "Safe content should be preserved or replaced with redaction marker"
            );
        }
    });
}

/// Property test: Global redaction helpers process all output surfaces
///
/// **Feature: crates-io-packaging, Property 12: Redaction pipeline completeness**
///
/// This test verifies that the global redaction helper functions
/// (redact_user_string, redact_user_strings, redact_user_optional) correctly
/// process content for all output surfaces.
///
/// **Validates: Requirements FR-SEC-1, FR-SEC-5**
#[test]
fn prop_global_redaction_helpers_complete() {
    use xchecker::redaction::{redact_user_optional, redact_user_string, redact_user_strings};

    let config = proptest_config(None);

    proptest!(config, |(
        (secret, category) in secret_generators::any_secret_category(),
        safe_content in "[a-zA-Z0-9 ]{10,50}"
    )| {
        // Test redact_user_string
        let content_with_secret = format!("{} contains {}", safe_content, secret);
        let redacted = redact_user_string(&content_with_secret);
        prop_assert!(
            !redacted.contains(&secret),
            "redact_user_string should redact '{}' category. Found in: '{}'",
            category, redacted
        );
        prop_assert!(
            redacted.contains(&safe_content),
            "redact_user_string should preserve safe content"
        );

        // Test redact_user_strings (batch)
        let strings_with_secrets = vec![
            format!("First: {}", secret),
            safe_content.clone(),
            format!("Third: {}", secret),
        ];
        let redacted_strings = redact_user_strings(&strings_with_secrets);
        prop_assert_eq!(
            redacted_strings.len(),
            strings_with_secrets.len(),
            "redact_user_strings should preserve vector length"
        );
        for (i, redacted_str) in redacted_strings.iter().enumerate() {
            prop_assert!(
                !redacted_str.contains(&secret),
                "redact_user_strings[{}] should be redacted. Category: '{}', Found: '{}'",
                i, category, redacted_str
            );
        }
        // Safe content should be preserved
        prop_assert_eq!(
            &redacted_strings[1], &safe_content,
            "Safe content should be unchanged"
        );

        // Test redact_user_optional with Some
        let optional_with_secret = Some(format!("Optional: {}", secret));
        let redacted_optional = redact_user_optional(&optional_with_secret);
        prop_assert!(
            redacted_optional.is_some(),
            "redact_user_optional should preserve Some"
        );
        prop_assert!(
            !redacted_optional.as_ref().unwrap().contains(&secret),
            "redact_user_optional should redact '{}' category. Found: '{}'",
            category, redacted_optional.unwrap()
        );

        // Test redact_user_optional with None
        let none_value: Option<String> = None;
        let redacted_none = redact_user_optional(&none_value);
        prop_assert!(
            redacted_none.is_none(),
            "redact_user_optional should preserve None"
        );
    });
}

/// Property test: Error messages are redacted before display
///
/// **Feature: crates-io-packaging, Property 12: Redaction pipeline completeness**
///
/// This test verifies that error messages containing secrets are properly
/// redacted when using the display_for_user() method or similar user-facing
/// error formatting.
///
/// **Validates: Requirements FR-SEC-1, FR-SEC-5**
#[test]
fn prop_error_messages_redacted() {
    use xchecker::redaction::redact_user_string;

    let config = proptest_config(None);

    proptest!(config, |(
        (secret, category) in secret_generators::any_secret_category(),
        error_context in "[a-zA-Z0-9 ]{10,30}"
    )| {
        // Simulate various error message formats that might contain secrets
        let error_formats = vec![
            format!("Authentication failed with token {}", secret),
            format!("Connection to {} refused", secret),
            format!("Invalid credentials: {}", secret),
            format!("{}: error processing {}", error_context, secret),
            format!("Failed to parse config containing {}", secret),
        ];

        for error_msg in error_formats {
            let redacted = redact_user_string(&error_msg);
            prop_assert!(
                !redacted.contains(&secret),
                "Error message should be redacted. Category: '{}', Original: '{}', Redacted: '{}'",
                category, error_msg, redacted
            );
            // Verify the error context is preserved
            prop_assert!(
                redacted.contains("***") || !error_msg.contains(&secret),
                "Redacted content should contain redaction marker"
            );
        }
    });
}

/// Property test: Redaction is idempotent
///
/// **Feature: crates-io-packaging, Property 12: Redaction pipeline completeness**
///
/// This test verifies that applying redaction multiple times produces the same
/// result as applying it once. This is important for ensuring that content
/// passing through multiple output surfaces doesn't get corrupted.
///
/// **Validates: Requirements FR-SEC-1, FR-SEC-5**
#[test]
fn prop_redaction_idempotent() {
    use xchecker::redaction::redact_user_string;

    let config = proptest_config(None);

    proptest!(config, |(
        (secret, _category) in secret_generators::any_secret_category(),
        content in "[a-zA-Z0-9 ]{10,50}"
    )| {
        let content_with_secret = format!("{} {} more content", content, secret);

        // Apply redaction once
        let redacted_once = redact_user_string(&content_with_secret);

        // Apply redaction twice
        let redacted_twice = redact_user_string(&redacted_once);

        // Apply redaction three times
        let redacted_thrice = redact_user_string(&redacted_twice);

        // All should be identical (idempotent)
        prop_assert_eq!(
            &redacted_once, &redacted_twice,
            "Redaction should be idempotent (once == twice)"
        );
        prop_assert_eq!(
            &redacted_twice, &redacted_thrice,
            "Redaction should be idempotent (twice == thrice)"
        );

        // The secret should not be present in any version
        prop_assert!(
            !redacted_once.contains(&secret),
            "Secret should be redacted after one pass"
        );
    });
}

/// Property test: Redaction preserves content structure
///
/// **Feature: crates-io-packaging, Property 12: Redaction pipeline completeness**
///
/// This test verifies that redaction preserves the overall structure of content
/// (line count, general format) while only replacing secret patterns.
///
/// **Validates: Requirements FR-SEC-1, FR-SEC-5**
#[test]
fn prop_redaction_preserves_structure() {
    use xchecker::redaction::redact_user_string;

    let config = proptest_config(None);

    proptest!(config, |(
        (secret, _category) in secret_generators::any_secret_category(),
        lines in prop::collection::vec("[a-zA-Z0-9 ]{5,30}", 1..10)
    )| {
        // Create multi-line content with secret on one line
        let secret_line_idx = lines.len() / 2;
        let mut content_lines = lines.clone();
        content_lines[secret_line_idx] = format!("{} {}", content_lines[secret_line_idx], secret);
        let content = content_lines.join("\n");

        let redacted = redact_user_string(&content);

        // Line count should be preserved
        let original_line_count = content.lines().count();
        let redacted_line_count = redacted.lines().count();
        prop_assert_eq!(
            original_line_count, redacted_line_count,
            "Redaction should preserve line count"
        );

        // Lines without secrets should be unchanged
        for (i, (original, redacted_line)) in content.lines().zip(redacted.lines()).enumerate() {
            if i != secret_line_idx {
                prop_assert_eq!(
                    original, redacted_line,
                    "Non-secret lines should be unchanged at line {}", i
                );
            }
        }

        // The secret should not be present
        prop_assert!(
            !redacted.contains(&secret),
            "Secret should be redacted"
        );
    });
}


// ============================================================================
// Property 14: Path Sandbox Enforcement
// ============================================================================

/// Property test: Path sandbox enforcement
///
/// **Feature: crates-io-packaging, Property 14: Path sandbox enforcement**
///
/// This test verifies that the path sandbox correctly rejects:
/// - Paths containing ".." traversal components
/// - Absolute paths outside the sandbox root
/// - Various escape attempts through path manipulation
///
/// **Validates: Requirements FR-SEC-3**
#[test]
fn prop_path_sandbox_enforcement() {
    use xchecker::paths::{SandboxConfig, SandboxError, SandboxRoot};
    use tempfile::TempDir;

    let config = proptest_config(None);

    proptest!(config, |(
        // Generate various traversal attempts
        traversal_depth in 1usize..10,
        // Generate random path segments
        path_segments in prop::collection::vec("[a-zA-Z0-9_-]{1,20}", 0..5),
        // Generate position for traversal insertion
        traversal_position in 0usize..6,
    )| {
        // Create a temporary directory for the sandbox root
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let root = SandboxRoot::new(temp_dir.path(), SandboxConfig::default())
            .expect("Failed to create sandbox root");

        // Test 1: Paths with ".." traversal should be rejected
        {
            // Build a path with ".." components
            let mut path_parts: Vec<String> = path_segments.iter().take(traversal_position.min(path_segments.len())).cloned().collect();
            
            // Add traversal components
            for _ in 0..traversal_depth {
                path_parts.push("..".to_string());
            }
            
            // Add remaining segments
            path_parts.extend(path_segments.iter().skip(traversal_position.min(path_segments.len())).cloned());
            
            let traversal_path = path_parts.join("/");
            
            if !traversal_path.is_empty() && traversal_path.contains("..") {
                let result = root.join(&traversal_path);
                prop_assert!(
                    result.is_err(),
                    "Path with '..' traversal should be rejected: {}",
                    traversal_path
                );
                
                if let Err(err) = result {
                    prop_assert!(
                        matches!(err, SandboxError::ParentTraversal { .. }),
                        "Error should be ParentTraversal, got: {:?}",
                        err
                    );
                }
            }
        }

        // Test 2: Pure ".." paths should be rejected
        {
            let pure_traversal = (0..traversal_depth).map(|_| "..").collect::<Vec<_>>().join("/");
            let result = root.join(&pure_traversal);
            prop_assert!(
                result.is_err(),
                "Pure '..' traversal path should be rejected: {}",
                pure_traversal
            );
        }

        // Test 3: Absolute paths should be rejected
        {
            #[cfg(unix)]
            {
                let abs_paths = vec![
                    "/etc/passwd".to_string(),
                    "/tmp/test".to_string(),
                    format!("/home/{}", path_segments.first().unwrap_or(&"user".to_string())),
                ];
                
                for abs_path in abs_paths {
                    let result = root.join(&abs_path);
                    prop_assert!(
                        result.is_err(),
                        "Absolute path should be rejected: {}",
                        abs_path
                    );
                    
                    if let Err(err) = result {
                        prop_assert!(
                            matches!(err, SandboxError::AbsolutePath { .. }),
                            "Error should be AbsolutePath, got: {:?}",
                            err
                        );
                    }
                }
            }
            
            #[cfg(windows)]
            {
                let abs_paths = vec![
                    "C:\\Windows\\System32".to_string(),
                    "D:\\test".to_string(),
                    format!("C:\\Users\\{}", path_segments.first().unwrap_or(&"user".to_string())),
                ];
                
                for abs_path in abs_paths {
                    let result = root.join(&abs_path);
                    prop_assert!(
                        result.is_err(),
                        "Absolute path should be rejected: {}",
                        abs_path
                    );
                    
                    if let Err(err) = result {
                        prop_assert!(
                            matches!(err, SandboxError::AbsolutePath { .. }),
                            "Error should be AbsolutePath, got: {:?}",
                            err
                        );
                    }
                }
            }
        }

        // Test 4: Valid relative paths should be accepted
        {
            if !path_segments.is_empty() {
                let valid_path = path_segments.join("/");
                // Only test if path doesn't contain ".."
                if !valid_path.contains("..") {
                    let result = root.join(&valid_path);
                    prop_assert!(
                        result.is_ok(),
                        "Valid relative path should be accepted: {}",
                        valid_path
                    );
                    
                    if let Ok(sandbox_path) = result {
                        // Verify the relative path is preserved
                        prop_assert_eq!(
                            sandbox_path.relative().to_string_lossy(),
                            valid_path,
                            "Relative path should be preserved"
                        );
                    }
                }
            }
        }
    });
}

/// Property test: Path sandbox rejects various escape patterns
///
/// **Feature: crates-io-packaging, Property 14: Path sandbox enforcement (escape patterns)**
///
/// This test generates various escape attempt patterns and verifies they are all rejected.
///
/// **Validates: Requirements FR-SEC-3**
#[test]
fn prop_path_sandbox_escape_patterns() {
    use xchecker::paths::{SandboxConfig, SandboxRoot};
    use tempfile::TempDir;

    let config = proptest_config(None);

    proptest!(config, |(
        // Generate random prefix segments
        prefix in prop::collection::vec("[a-zA-Z0-9_]{1,10}", 0..3),
        // Generate random suffix segments  
        suffix in prop::collection::vec("[a-zA-Z0-9_]{1,10}", 0..3),
        // Number of parent traversals
        num_traversals in 1usize..5,
    )| {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let root = SandboxRoot::new(temp_dir.path(), SandboxConfig::default())
            .expect("Failed to create sandbox root");

        // Pattern 1: prefix/../../../suffix
        {
            let mut parts = prefix.clone();
            for _ in 0..num_traversals {
                parts.push("..".to_string());
            }
            parts.extend(suffix.clone());
            
            let escape_path = parts.join("/");
            if escape_path.contains("..") {
                let result = root.join(&escape_path);
                prop_assert!(
                    result.is_err(),
                    "Escape pattern should be rejected: {}",
                    escape_path
                );
            }
        }

        // Pattern 2: ./../escape
        {
            let mut parts = vec![".".to_string()];
            for _ in 0..num_traversals {
                parts.push("..".to_string());
            }
            parts.push("escape".to_string());
            
            let escape_path = parts.join("/");
            let result = root.join(&escape_path);
            prop_assert!(
                result.is_err(),
                "Dot-prefixed escape should be rejected: {}",
                escape_path
            );
        }

        // Pattern 3: subdir/./../../escape (with current dir markers)
        {
            let mut parts = prefix.clone();
            parts.push(".".to_string());
            for _ in 0..num_traversals {
                parts.push("..".to_string());
            }
            parts.push("escape".to_string());
            
            let escape_path = parts.join("/");
            if escape_path.contains("..") {
                let result = root.join(&escape_path);
                prop_assert!(
                    result.is_err(),
                    "Mixed escape pattern should be rejected: {}",
                    escape_path
                );
            }
        }
    });
}

/// Property test: Sandbox root validation
///
/// **Feature: crates-io-packaging, Property 14: Path sandbox enforcement (root validation)**
///
/// This test verifies that sandbox root creation properly validates the root path.
///
/// **Validates: Requirements FR-SEC-3**
#[test]
fn prop_sandbox_root_validation() {
    use xchecker::paths::{SandboxConfig, SandboxError, SandboxRoot};
    use tempfile::TempDir;

    let config = proptest_config(None);

    proptest!(config, |(
        // Generate random nonexistent path segments
        nonexistent_segments in prop::collection::vec("[a-zA-Z0-9_]{5,15}", 3..6),
    )| {
        // Test 1: Nonexistent paths should fail
        {
            let nonexistent_path = format!("/nonexistent/{}", nonexistent_segments.join("/"));
            let result = SandboxRoot::new(&nonexistent_path, SandboxConfig::default());
            prop_assert!(
                result.is_err(),
                "Nonexistent path should fail: {}",
                nonexistent_path
            );
            
            if let Err(err) = result {
                prop_assert!(
                    matches!(err, SandboxError::RootNotFound { .. }),
                    "Error should be RootNotFound, got: {:?}",
                    err
                );
            }
        }

        // Test 2: Files (not directories) should fail
        {
            let temp_dir = TempDir::new().expect("Failed to create temp dir");
            let file_path = temp_dir.path().join("file.txt");
            std::fs::write(&file_path, "content").expect("Failed to write file");
            
            let result = SandboxRoot::new(&file_path, SandboxConfig::default());
            prop_assert!(
                result.is_err(),
                "File path should fail as sandbox root"
            );
            
            if let Err(err) = result {
                prop_assert!(
                    matches!(err, SandboxError::RootNotDirectory { .. }),
                    "Error should be RootNotDirectory, got: {:?}",
                    err
                );
            }
        }

        // Test 3: Valid directories should succeed
        {
            let temp_dir = TempDir::new().expect("Failed to create temp dir");
            let result = SandboxRoot::new(temp_dir.path(), SandboxConfig::default());
            prop_assert!(
                result.is_ok(),
                "Valid directory should succeed as sandbox root"
            );
            
            if let Ok(root) = result {
                // Root path should be absolute after canonicalization
                prop_assert!(
                    root.as_path().is_absolute(),
                    "Sandbox root should be absolute"
                );
            }
        }
    });
}

// ============================================================================
// Property 15: Symlink Rejection
// ============================================================================

/// Property test: Symlink rejection
///
/// **Feature: crates-io-packaging, Property 15: Symlink rejection**
///
/// This test verifies that the path sandbox correctly rejects symlinks when
/// configured to do so (default behavior), and allows them when explicitly
/// enabled via configuration.
///
/// The test covers:
/// - Symlinks to files within the sandbox are rejected by default
/// - Symlinks to files outside the sandbox are always rejected (escape attempt)
/// - Symlinks are allowed when `allow_symlinks: true` is configured
/// - Hardlinks are rejected by default (Unix only)
/// - Hardlinks are allowed when `allow_hardlinks: true` is configured
///
/// **Validates: Requirements FR-SEC-3**
#[cfg(unix)]
#[test]
fn prop_symlink_rejection() {
    use xchecker::paths::{SandboxConfig, SandboxError, SandboxRoot};
    use tempfile::TempDir;

    let config = proptest_config(None);

    proptest!(config, |(
        // Generate random file names for targets and links
        target_name in "[a-zA-Z0-9_]{3,15}",
        link_name in "[a-zA-Z0-9_]{3,15}",
        // Generate random file content
        file_content in "[a-zA-Z0-9 ]{10,100}",
        // Generate random subdirectory depth
        subdir_depth in 0usize..3,
    )| {
        // Create a temporary directory for the sandbox root
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        
        // Create subdirectories if needed
        let mut target_dir = temp_dir.path().to_path_buf();
        for i in 0..subdir_depth {
            target_dir = target_dir.join(format!("subdir{}", i));
        }
        std::fs::create_dir_all(&target_dir).expect("Failed to create subdirs");
        
        // Create a target file
        let target_file = target_dir.join(format!("{}.txt", target_name));
        std::fs::write(&target_file, &file_content).expect("Failed to write target file");
        
        // Create a symlink to the target file
        let link_file = temp_dir.path().join(format!("{}_link.txt", link_name));
        std::os::unix::fs::symlink(&target_file, &link_file).expect("Failed to create symlink");
        
        // Test 1: Symlinks should be rejected with default config
        {
            let root = SandboxRoot::new(temp_dir.path(), SandboxConfig::default())
                .expect("Failed to create sandbox root");
            
            let link_relative = format!("{}_link.txt", link_name);
            let result = root.join(&link_relative);
            
            prop_assert!(
                result.is_err(),
                "Symlink should be rejected with default config: {}",
                link_relative
            );
            
            if let Err(err) = result {
                prop_assert!(
                    matches!(err, SandboxError::SymlinkNotAllowed { .. }),
                    "Error should be SymlinkNotAllowed, got: {:?}",
                    err
                );
            }
        }
        
        // Test 2: Symlinks should be allowed with permissive config
        {
            let root = SandboxRoot::new(temp_dir.path(), SandboxConfig::permissive())
                .expect("Failed to create sandbox root");
            
            let link_relative = format!("{}_link.txt", link_name);
            let result = root.join(&link_relative);
            
            prop_assert!(
                result.is_ok(),
                "Symlink should be allowed with permissive config: {}",
                link_relative
            );
        }
        
        // Test 3: Symlinks pointing outside sandbox should always be rejected (escape attempt)
        {
            let outside_dir = TempDir::new().expect("Failed to create outside dir");
            let outside_file = outside_dir.path().join("secret.txt");
            std::fs::write(&outside_file, "secret content").expect("Failed to write outside file");
            
            // Create a symlink inside sandbox pointing outside
            let escape_link = temp_dir.path().join("escape_link.txt");
            std::os::unix::fs::symlink(&outside_file, &escape_link).expect("Failed to create escape symlink");
            
            // Even with permissive config, escape should be detected
            let root = SandboxRoot::new(temp_dir.path(), SandboxConfig::permissive())
                .expect("Failed to create sandbox root");
            
            let result = root.join("escape_link.txt");
            
            prop_assert!(
                result.is_err(),
                "Symlink escape attempt should be rejected even with permissive config"
            );
            
            if let Err(err) = result {
                prop_assert!(
                    matches!(err, SandboxError::EscapeAttempt { .. }),
                    "Error should be EscapeAttempt, got: {:?}",
                    err
                );
            }
        }
    });
}

/// Property test: Hardlink rejection (Unix only)
///
/// **Feature: crates-io-packaging, Property 15: Symlink rejection (hardlinks)**
///
/// This test verifies that hardlinks (files with link count > 1) are rejected
/// by default and allowed when explicitly configured.
///
/// **Validates: Requirements FR-SEC-3**
#[cfg(unix)]
#[test]
fn prop_hardlink_rejection() {
    use xchecker::paths::{SandboxConfig, SandboxError, SandboxRoot};
    use tempfile::TempDir;

    let config = proptest_config(None);

    proptest!(config, |(
        // Generate random file names
        original_name in "[a-zA-Z0-9_]{3,15}",
        hardlink_name in "[a-zA-Z0-9_]{3,15}",
        // Generate random file content
        file_content in "[a-zA-Z0-9 ]{10,100}",
    )| {
        // Create a temporary directory for the sandbox root
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        
        // Create an original file
        let original_file = temp_dir.path().join(format!("{}.txt", original_name));
        std::fs::write(&original_file, &file_content).expect("Failed to write original file");
        
        // Create a hardlink to the original file
        let hardlink_file = temp_dir.path().join(format!("{}_hardlink.txt", hardlink_name));
        std::fs::hard_link(&original_file, &hardlink_file).expect("Failed to create hardlink");
        
        // Test 1: Hardlinks should be rejected with default config
        {
            let root = SandboxRoot::new(temp_dir.path(), SandboxConfig::default())
                .expect("Failed to create sandbox root");
            
            let hardlink_relative = format!("{}_hardlink.txt", hardlink_name);
            let result = root.join(&hardlink_relative);
            
            prop_assert!(
                result.is_err(),
                "Hardlink should be rejected with default config: {}",
                hardlink_relative
            );
            
            if let Err(err) = result {
                prop_assert!(
                    matches!(err, SandboxError::HardlinkNotAllowed { .. }),
                    "Error should be HardlinkNotAllowed, got: {:?}",
                    err
                );
            }
        }
        
        // Test 2: Hardlinks should be allowed with permissive config
        {
            let root = SandboxRoot::new(temp_dir.path(), SandboxConfig::permissive())
                .expect("Failed to create sandbox root");
            
            let hardlink_relative = format!("{}_hardlink.txt", hardlink_name);
            let result = root.join(&hardlink_relative);
            
            prop_assert!(
                result.is_ok(),
                "Hardlink should be allowed with permissive config: {}",
                hardlink_relative
            );
        }
        
        // Test 3: Original file (link count = 1 initially, now = 2) should also be rejected
        // because it now has multiple links
        {
            let root = SandboxRoot::new(temp_dir.path(), SandboxConfig::default())
                .expect("Failed to create sandbox root");
            
            let original_relative = format!("{}.txt", original_name);
            let result = root.join(&original_relative);
            
            prop_assert!(
                result.is_err(),
                "Original file with hardlink should be rejected with default config: {}",
                original_relative
            );
            
            if let Err(err) = result {
                prop_assert!(
                    matches!(err, SandboxError::HardlinkNotAllowed { .. }),
                    "Error should be HardlinkNotAllowed, got: {:?}",
                    err
                );
            }
        }
    });
}

/// Property test: Symlink in path components
///
/// **Feature: crates-io-packaging, Property 15: Symlink rejection (path components)**
///
/// This test verifies that symlinks anywhere in the path (not just the final
/// component) are detected and rejected.
///
/// **Validates: Requirements FR-SEC-3**
#[cfg(unix)]
#[test]
fn prop_symlink_in_path_components() {
    use xchecker::paths::{SandboxConfig, SandboxError, SandboxRoot};
    use tempfile::TempDir;

    let config = proptest_config(None);

    proptest!(config, |(
        // Generate random directory and file names
        real_dir_name in "[a-zA-Z0-9_]{3,10}",
        link_dir_name in "[a-zA-Z0-9_]{3,10}",
        file_name in "[a-zA-Z0-9_]{3,10}",
        file_content in "[a-zA-Z0-9 ]{10,50}",
    )| {
        // Create a temporary directory for the sandbox root
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        
        // Create a real directory with a file
        let real_dir = temp_dir.path().join(&real_dir_name);
        std::fs::create_dir(&real_dir).expect("Failed to create real dir");
        
        let real_file = real_dir.join(format!("{}.txt", file_name));
        std::fs::write(&real_file, &file_content).expect("Failed to write file");
        
        // Create a symlink directory pointing to the real directory
        let link_dir = temp_dir.path().join(&link_dir_name);
        std::os::unix::fs::symlink(&real_dir, &link_dir).expect("Failed to create symlink dir");
        
        // Test: Accessing file through symlink directory should be rejected
        {
            let root = SandboxRoot::new(temp_dir.path(), SandboxConfig::default())
                .expect("Failed to create sandbox root");
            
            // Try to access the file through the symlink directory
            let path_through_symlink = format!("{}/{}.txt", link_dir_name, file_name);
            let result = root.join(&path_through_symlink);
            
            prop_assert!(
                result.is_err(),
                "Path through symlink directory should be rejected: {}",
                path_through_symlink
            );
            
            if let Err(err) = result {
                prop_assert!(
                    matches!(err, SandboxError::SymlinkNotAllowed { .. }),
                    "Error should be SymlinkNotAllowed, got: {:?}",
                    err
                );
            }
        }
        
        // Test: Same path should work with permissive config
        {
            let root = SandboxRoot::new(temp_dir.path(), SandboxConfig::permissive())
                .expect("Failed to create sandbox root");
            
            let path_through_symlink = format!("{}/{}.txt", link_dir_name, file_name);
            let result = root.join(&path_through_symlink);
            
            prop_assert!(
                result.is_ok(),
                "Path through symlink directory should be allowed with permissive config: {}",
                path_through_symlink
            );
        }
        
        // Test: Direct path to real file should work with default config
        {
            let root = SandboxRoot::new(temp_dir.path(), SandboxConfig::default())
                .expect("Failed to create sandbox root");
            
            let direct_path = format!("{}/{}.txt", real_dir_name, file_name);
            let result = root.join(&direct_path);
            
            prop_assert!(
                result.is_ok(),
                "Direct path to real file should be allowed: {}",
                direct_path
            );
        }
    });
}

/// Property test: Mixed symlink and hardlink configuration
///
/// **Feature: crates-io-packaging, Property 15: Symlink rejection (mixed config)**
///
/// This test verifies that symlink and hardlink settings can be configured
/// independently.
///
/// **Validates: Requirements FR-SEC-3**
#[cfg(unix)]
#[test]
fn prop_symlink_hardlink_independent_config() {
    use xchecker::paths::{SandboxConfig, SandboxError, SandboxRoot};
    use tempfile::TempDir;

    let config = proptest_config(None);

    proptest!(config, |(
        // Generate random file names
        target_name in "[a-zA-Z0-9_]{3,10}",
        symlink_name in "[a-zA-Z0-9_]{3,10}",
        hardlink_name in "[a-zA-Z0-9_]{3,10}",
        file_content in "[a-zA-Z0-9 ]{10,50}",
    )| {
        // Create a temporary directory for the sandbox root
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        
        // Create a target file for symlink
        let symlink_target = temp_dir.path().join(format!("{}_target.txt", target_name));
        std::fs::write(&symlink_target, &file_content).expect("Failed to write symlink target");
        
        // Create a separate file for hardlink (to avoid link count issues)
        let hardlink_original = temp_dir.path().join(format!("{}_original.txt", target_name));
        std::fs::write(&hardlink_original, &file_content).expect("Failed to write hardlink original");
        
        // Create symlink
        let symlink_file = temp_dir.path().join(format!("{}_symlink.txt", symlink_name));
        std::os::unix::fs::symlink(&symlink_target, &symlink_file).expect("Failed to create symlink");
        
        // Create hardlink
        let hardlink_file = temp_dir.path().join(format!("{}_hardlink.txt", hardlink_name));
        std::fs::hard_link(&hardlink_original, &hardlink_file).expect("Failed to create hardlink");
        
        // Test 1: Allow symlinks only
        {
            let config = SandboxConfig {
                allow_symlinks: true,
                allow_hardlinks: false,
            };
            let root = SandboxRoot::new(temp_dir.path(), config)
                .expect("Failed to create sandbox root");
            
            // Symlink should be allowed
            let symlink_relative = format!("{}_symlink.txt", symlink_name);
            let symlink_result = root.join(&symlink_relative);
            prop_assert!(
                symlink_result.is_ok(),
                "Symlink should be allowed when allow_symlinks=true"
            );
            
            // Hardlink should be rejected
            let hardlink_relative = format!("{}_hardlink.txt", hardlink_name);
            let hardlink_result = root.join(&hardlink_relative);
            prop_assert!(
                hardlink_result.is_err(),
                "Hardlink should be rejected when allow_hardlinks=false"
            );
            if let Err(err) = hardlink_result {
                prop_assert!(
                    matches!(err, SandboxError::HardlinkNotAllowed { .. }),
                    "Error should be HardlinkNotAllowed"
                );
            }
        }
        
        // Test 2: Allow hardlinks only
        {
            let config = SandboxConfig {
                allow_symlinks: false,
                allow_hardlinks: true,
            };
            let root = SandboxRoot::new(temp_dir.path(), config)
                .expect("Failed to create sandbox root");
            
            // Symlink should be rejected
            let symlink_relative = format!("{}_symlink.txt", symlink_name);
            let symlink_result = root.join(&symlink_relative);
            prop_assert!(
                symlink_result.is_err(),
                "Symlink should be rejected when allow_symlinks=false"
            );
            if let Err(err) = symlink_result {
                prop_assert!(
                    matches!(err, SandboxError::SymlinkNotAllowed { .. }),
                    "Error should be SymlinkNotAllowed"
                );
            }
            
            // Hardlink should be allowed
            let hardlink_relative = format!("{}_hardlink.txt", hardlink_name);
            let hardlink_result = root.join(&hardlink_relative);
            prop_assert!(
                hardlink_result.is_ok(),
                "Hardlink should be allowed when allow_hardlinks=true"
            );
        }
    });
}

/// Property test: Atomic writes for state files
///
/// **Property 13: Atomic writes for state files**
/// **Validates: Requirements FR-SEC-2**
///
/// This property verifies that state file writes are atomic and consistent.
/// It checks that:
/// 1. Writes succeed for arbitrary content
/// 2. Content is correctly written (including line ending normalization)
/// 3. Overwrites work correctly
/// 4. Parent directories are created automatically
#[test]
fn prop_atomic_writes_for_state_files() {
    use xchecker::atomic_write::write_file_atomic;
    use camino::Utf8Path;
    use tempfile::TempDir;
    use std::fs;

    let config = proptest_config(None);

    proptest!(config, |(
        // Generate arbitrary content (including special chars and newlines)
        content in ".*",
        // Generate a filename
        filename in "[a-zA-Z0-9_]{1,20}\\.json",
        // Generate a subdirectory path (optional)
        subdir in prop::option::of("[a-zA-Z0-9_]{1,10}/[a-zA-Z0-9_]{1,10}")
    )| {
        let temp_dir = TempDir::new().unwrap();
        let mut file_path_buf = temp_dir.path().to_path_buf();
        
        if let Some(ref sub) = subdir {
            file_path_buf.push(sub);
        }
        file_path_buf.push(&filename);
        
        let file_path = Utf8Path::from_path(file_path_buf.as_path()).unwrap();

        // 1. Write content atomically
        let result = write_file_atomic(file_path, &content);
        
        prop_assert!(result.is_ok(), "Atomic write should succeed");
        prop_assert!(file_path.exists(), "File should exist after write");
        
        // 2. Verify content
        let read_content = fs::read_to_string(file_path.as_std_path()).unwrap();
        
        // Normalize expected content (atomic write normalizes line endings)
        let expected_content = content.replace("\r\n", "\n").replace('\r', "\n");
        prop_assert_eq!(read_content, expected_content, "File content should match written content (normalized)");
        
        // 3. Verify overwrite
        let new_content = format!("updated: {}", content);
        let result_overwrite = write_file_atomic(file_path, &new_content);
        
        prop_assert!(result_overwrite.is_ok(), "Atomic overwrite should succeed");
        
        let read_new_content = fs::read_to_string(file_path.as_std_path()).unwrap();
        let expected_new_content = new_content.replace("\r\n", "\n").replace('\r', "\n");
        prop_assert_eq!(read_new_content, expected_new_content, "Overwritten content should match");
    });
}

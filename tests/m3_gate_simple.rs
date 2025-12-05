//! M3 Gate Simple Validation Test
//!
//! This module contains a simplified test for the M3 Gate canonicalization requirement.

use anyhow::Result;
use xchecker::canonicalization::Canonicalizer;
use xchecker::types::FileType;

/// Test *.core.yaml canonicalization yields identical hashes for permuted inputs
/// Validates R12.1 requirements for canonicalization determinism
#[test]
fn test_core_yaml_canonicalization_determinism() -> Result<()> {
    let canonicalizer = Canonicalizer::new();

    // Create complex YAML structures with different key ordering
    let yaml_original = r#"
spec_id: "test-spec-123"
phase: "requirements"
version: "1.0"

metadata:
  total_requirements: 5
  total_user_stories: 3
  total_acceptance_criteria: 15
  has_nfrs: true
  complexity_score: 8.5

requirements:
  - id: "REQ-001"
    title: "User Authentication"
    priority: "high"
    user_story: "As a user, I want to authenticate securely"
    acceptance_criteria:
      - "WHEN user provides valid credentials THEN system SHALL authenticate"
      - "WHEN user provides invalid credentials THEN system SHALL reject"
    dependencies: []
    
  - id: "REQ-002"
    title: "Data Validation"
    priority: "medium"
    user_story: "As a system, I want to validate all inputs"
    acceptance_criteria:
      - "WHEN input is provided THEN system SHALL validate format"
      - "WHEN validation fails THEN system SHALL return error"
    dependencies: ["REQ-001"]

nfrs:
  - category: "performance"
    requirement: "Response time SHALL be less than 200ms"
    measurable: true
  - category: "security"
    requirement: "All data SHALL be encrypted at rest"
    measurable: false

dependencies:
  - from: "REQ-002"
    to: "REQ-001"
    type: "functional"

generated_at: "2025-01-01T12:00:00Z"
"#;

    // Same content with completely reordered keys at all levels (but preserving array order)
    let yaml_reordered = r#"
version: "1.0"
generated_at: "2025-01-01T12:00:00Z"
dependencies:
  - type: "functional"
    to: "REQ-001"
    from: "REQ-002"
nfrs:
  - measurable: true
    requirement: "Response time SHALL be less than 200ms"
    category: "performance"
  - measurable: false
    requirement: "All data SHALL be encrypted at rest"
    category: "security"
requirements:
  - dependencies: []
    acceptance_criteria:
      - "WHEN user provides valid credentials THEN system SHALL authenticate"
      - "WHEN user provides invalid credentials THEN system SHALL reject"
    user_story: "As a user, I want to authenticate securely"
    priority: "high"
    title: "User Authentication"
    id: "REQ-001"
  - dependencies: ["REQ-001"]
    acceptance_criteria:
      - "WHEN input is provided THEN system SHALL validate format"
      - "WHEN validation fails THEN system SHALL return error"
    user_story: "As a system, I want to validate all inputs"
    priority: "medium"
    title: "Data Validation"
    id: "REQ-002"
metadata:
  has_nfrs: true
  complexity_score: 8.5
  total_acceptance_criteria: 15
  total_user_stories: 3
  total_requirements: 5
phase: "requirements"
spec_id: "test-spec-123"
"#;

    // Same content with different whitespace and line endings
    let yaml_whitespace = "spec_id: \"test-spec-123\"   \r\nphase: \"requirements\"   \r\nversion: \"1.0\"   \r\n\r\nmetadata:   \r\n  total_requirements: 5   \r\n  total_user_stories: 3   \r\n  total_acceptance_criteria: 15   \r\n  has_nfrs: true   \r\n  complexity_score: 8.5   \r\n\r\nrequirements:   \r\n  - id: \"REQ-001\"   \r\n    title: \"User Authentication\"   \r\n    priority: \"high\"   \r\n    user_story: \"As a user, I want to authenticate securely\"   \r\n    acceptance_criteria:   \r\n      - \"WHEN user provides valid credentials THEN system SHALL authenticate\"   \r\n      - \"WHEN user provides invalid credentials THEN system SHALL reject\"   \r\n    dependencies: []   \r\n      \r\n  - id: \"REQ-002\"   \r\n    title: \"Data Validation\"   \r\n    priority: \"medium\"   \r\n    user_story: \"As a system, I want to validate all inputs\"   \r\n    acceptance_criteria:   \r\n      - \"WHEN input is provided THEN system SHALL validate format\"   \r\n      - \"WHEN validation fails THEN system SHALL return error\"   \r\n    dependencies: [\"REQ-001\"]   \r\n\r\nnfrs:   \r\n  - category: \"performance\"   \r\n    requirement: \"Response time SHALL be less than 200ms\"   \r\n    measurable: true   \r\n  - category: \"security\"   \r\n    requirement: \"All data SHALL be encrypted at rest\"   \r\n    measurable: false   \r\n\r\ndependencies:   \r\n  - from: \"REQ-002\"   \r\n    to: \"REQ-001\"   \r\n    type: \"functional\"   \r\n\r\ngenerated_at: \"2025-01-01T12:00:00Z\"   \r\n";

    // Compute hashes for all variants
    let hash_original = canonicalizer.hash_canonicalized(yaml_original, FileType::Yaml)?;
    let hash_reordered = canonicalizer.hash_canonicalized(yaml_reordered, FileType::Yaml)?;
    let hash_whitespace = canonicalizer.hash_canonicalized(yaml_whitespace, FileType::Yaml)?;

    // All hashes should be identical (R12.1)
    assert_eq!(
        hash_original, hash_reordered,
        "Reordered *.core.yaml should produce identical hash"
    );
    assert_eq!(
        hash_original, hash_whitespace,
        "*.core.yaml with different whitespace should produce identical hash"
    );

    // Verify hashes are valid BLAKE3 (64 hex characters)
    assert_eq!(hash_original.len(), 64, "Hash should be 64 characters");
    assert!(
        hash_original.chars().all(|c| c.is_ascii_hexdigit()),
        "Hash should contain only hex characters"
    );

    println!("✓ *.core.yaml canonicalization determinism test passed");
    println!("  Original hash: {}", &hash_original[..16]);
    println!("  All variants produce identical hash");

    Ok(())
}

/// Test canonicalization version and backend information
#[test]
fn test_canonicalization_metadata() -> Result<()> {
    let canonicalizer = Canonicalizer::new();

    // Test version string (R2.7)
    let version = canonicalizer.version();
    assert_eq!(
        version, "yaml-v1,md-v1",
        "Version should match expected format"
    );

    // Test backend string
    let backend = canonicalizer.backend();
    assert_eq!(
        backend, "jcs-rfc8785",
        "Backend should match expected identifier"
    );

    println!("✓ Canonicalization metadata test passed");
    println!("  Version: {version}");
    println!("  Backend: {backend}");

    Ok(())
}

/// Test hash consistency across multiple runs
#[test]
fn test_hash_consistency() -> Result<()> {
    let canonicalizer = Canonicalizer::new();

    let test_content = r"
name: consistency-test
version: 2.0.0
features:
  - feature1
  - feature2
config:
  enabled: true
  count: 42
";

    // Compute hash multiple times
    let mut hashes = Vec::new();
    for _ in 0..10 {
        let hash = canonicalizer.hash_canonicalized(test_content, FileType::Yaml)?;
        hashes.push(hash);
    }

    // All hashes should be identical
    let first_hash = &hashes[0];
    for (i, hash) in hashes.iter().enumerate() {
        assert_eq!(hash, first_hash, "Hash {i} should match first hash");
    }

    println!("✓ Hash consistency test passed");
    println!("  {} identical hashes generated", hashes.len());

    Ok(())
}

/// Comprehensive M3 Gate canonicalization validation
#[test]
fn test_m3_gate_canonicalization_comprehensive() -> Result<()> {
    println!("🚀 Running M3 Gate canonicalization validation...");

    // Run all canonicalization tests
    test_core_yaml_canonicalization_determinism()?;
    test_canonicalization_metadata()?;
    test_hash_consistency()?;

    println!("✅ M3 Gate canonicalization validation passed!");
    println!();
    println!("M3 Gate Canonicalization Requirements Validated:");
    println!("  ✓ R12.1: *.core.yaml canonicalization yields identical hashes for permuted inputs");
    println!("  ✓ R2.7: Canonicalization version and backend properly recorded");
    println!();
    println!("Key Features Verified:");
    println!("  ✓ YAML canonicalization with complex nested structures and reordering");
    println!("  ✓ Hash consistency across multiple runs");
    println!("  ✓ Canonicalization metadata reporting");

    Ok(())
}

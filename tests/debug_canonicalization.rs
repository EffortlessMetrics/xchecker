//! Debug canonicalization test

use anyhow::Result;
use xchecker::canonicalization::Canonicalizer;
use xchecker::types::FileType;

#[test]
fn debug_yaml_canonicalization() -> Result<()> {
    let canonicalizer = Canonicalizer::new();

    // Simple YAML test
    let yaml1 = r#"
name: test
version: 1.0
config:
  debug: true
  port: 8080
"#;

    let yaml2 = r#"
version: 1.0
config:
  port: 8080
  debug: true
name: test
"#;

    println!("YAML 1:");
    println!("{}", yaml1);
    println!("YAML 2:");
    println!("{}", yaml2);

    let hash1 = canonicalizer.hash_canonicalized(yaml1, FileType::Yaml)?;
    let hash2 = canonicalizer.hash_canonicalized(yaml2, FileType::Yaml)?;

    println!("Hash 1: {}", hash1);
    println!("Hash 2: {}", hash2);

    // Let's also check what the JCS canonicalization produces
    let yaml_value1: serde_yaml_ng::Value = serde_yaml_ng::from_str(yaml1)?;
    let yaml_value2: serde_yaml_ng::Value = serde_yaml_ng::from_str(yaml2)?;

    println!("Parsed YAML 1: {:?}", yaml_value1);
    println!("Parsed YAML 2: {:?}", yaml_value2);

    // Convert to JSON
    let json_str1 = serde_yaml_ng::to_string(&yaml_value1)?;
    let json_str2 = serde_yaml_ng::to_string(&yaml_value2)?;

    println!("JSON string 1: {}", json_str1);
    println!("JSON string 2: {}", json_str2);

    let json_value1: serde_json::Value = serde_yaml_ng::from_str(&json_str1)?;
    let json_value2: serde_json::Value = serde_yaml_ng::from_str(&json_str2)?;

    println!("JSON value 1: {:?}", json_value1);
    println!("JSON value 2: {:?}", json_value2);

    // JCS canonicalization
    let jcs1 = serde_json_canonicalizer::to_vec(&json_value1)?;
    let jcs2 = serde_json_canonicalizer::to_vec(&json_value2)?;

    let jcs_str1 = String::from_utf8(jcs1)?;
    let jcs_str2 = String::from_utf8(jcs2)?;

    println!("JCS 1: {}", jcs_str1);
    println!("JCS 2: {}", jcs_str2);

    assert_eq!(jcs_str1, jcs_str2, "JCS should be identical");
    assert_eq!(hash1, hash2, "Hashes should be identical");

    Ok(())
}

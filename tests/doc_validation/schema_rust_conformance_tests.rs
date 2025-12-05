//! Schema-Rust conformance tests
//!
//! These tests verify that enum definitions in JSON schemas match the Rust enum variants
//! after applying serde `rename_all` transformations.

#[cfg(test)]
mod tests {
    use serde_json::Value;
    use std::collections::HashSet;
    use std::fs;
    use strum::VariantNames;
    use xchecker::doctor::CheckStatus;
    use xchecker::types::{ConfigSource, ErrorKind};

    use crate::doc_validation::common::RenameAll;

    /// Load a JSON schema file and parse it
    fn load_schema(path: &str) -> Value {
        let content = fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("Failed to read schema file {path}: {e}"));
        serde_json::from_str(&content)
            .unwrap_or_else(|e| panic!("Failed to parse schema file {path}: {e}"))
    }

    /// Extract enum values from a schema at a given JSON pointer path
    fn extract_schema_enum(schema: &Value, pointer: &str) -> HashSet<String> {
        let enum_array = schema
            .pointer(pointer)
            .and_then(|v| v.as_array())
            .unwrap_or_else(|| panic!("Enum not found at pointer: {pointer}"));

        enum_array
            .iter()
            .filter_map(|v| v.as_str())
            .filter(|s| *s != "null") // Filter out null values from nullable enums
            .map(std::string::ToString::to_string)
            .collect()
    }

    #[test]
    fn test_error_kind_enum_conformance() {
        // Load receipt schema
        let schema = load_schema("schemas/receipt.v1.json");

        // Extract ErrorKind enum values from schema
        let schema_variants = extract_schema_enum(&schema, "/properties/error_kind/enum");

        // Get Rust variants and apply snake_case transformation
        let rename = RenameAll::SnakeCase;
        let rust_variants = rename.apply_to_variants(ErrorKind::VARIANTS);

        // Compare
        if rust_variants != schema_variants {
            let missing_in_schema: Vec<_> = rust_variants.difference(&schema_variants).collect();
            let extra_in_schema: Vec<_> = schema_variants.difference(&rust_variants).collect();

            panic!(
                "ErrorKind enum mismatch!\n\
                 Rust variants (after snake_case): {rust_variants:?}\n\
                 Schema variants: {schema_variants:?}\n\
                 Missing in schema: {missing_in_schema:?}\n\
                 Extra in schema: {extra_in_schema:?}"
            );
        }

        assert_eq!(
            rust_variants, schema_variants,
            "ErrorKind enum should match between Rust and schema"
        );
    }

    #[test]
    fn test_check_status_enum_conformance() {
        // Load doctor schema
        let schema = load_schema("schemas/doctor.v1.json");

        // Extract CheckStatus enum values from schema
        let schema_variants =
            extract_schema_enum(&schema, "/properties/checks/items/properties/status/enum");

        // Get Rust variants and apply snake_case transformation
        let rename = RenameAll::SnakeCase;
        let rust_variants = rename.apply_to_variants(CheckStatus::VARIANTS);

        // Compare
        if rust_variants != schema_variants {
            let missing_in_schema: Vec<_> = rust_variants.difference(&schema_variants).collect();
            let extra_in_schema: Vec<_> = schema_variants.difference(&rust_variants).collect();

            panic!(
                "CheckStatus enum mismatch!\n\
                 Rust variants (after snake_case): {rust_variants:?}\n\
                 Schema variants: {schema_variants:?}\n\
                 Missing in schema: {missing_in_schema:?}\n\
                 Extra in schema: {extra_in_schema:?}"
            );
        }

        assert_eq!(
            rust_variants, schema_variants,
            "CheckStatus enum should match between Rust and schema"
        );
    }

    #[test]
    fn test_config_source_enum_conformance() {
        // Load status schema
        let schema = load_schema("schemas/status.v1.json");

        // Extract ConfigSource enum values from schema
        let schema_variants = extract_schema_enum(
            &schema,
            "/properties/effective_config/additionalProperties/properties/source/enum",
        );

        // Get Rust variants and apply lowercase transformation
        let rename = RenameAll::Lowercase;
        let rust_variants = rename.apply_to_variants(ConfigSource::VARIANTS);

        // Compare
        if rust_variants != schema_variants {
            let missing_in_schema: Vec<_> = rust_variants.difference(&schema_variants).collect();
            let extra_in_schema: Vec<_> = schema_variants.difference(&rust_variants).collect();

            panic!(
                "ConfigSource enum mismatch!\n\
                 Rust variants (after lowercase): {rust_variants:?}\n\
                 Schema variants: {schema_variants:?}\n\
                 Missing in schema: {missing_in_schema:?}\n\
                 Extra in schema: {extra_in_schema:?}"
            );
        }

        assert_eq!(
            rust_variants, schema_variants,
            "ConfigSource enum should match between Rust and schema"
        );
    }

    #[test]
    fn test_error_kind_individual_variants() {
        // Test each variant individually for better error messages
        let schema = load_schema("schemas/receipt.v1.json");
        let schema_variants = extract_schema_enum(&schema, "/properties/error_kind/enum");

        let rename = RenameAll::SnakeCase;

        // Test each Rust variant
        for variant in ErrorKind::VARIANTS {
            let transformed = rename.apply(variant);
            assert!(
                schema_variants.contains(&transformed),
                "Schema missing ErrorKind variant: {transformed} (transformed from {variant})"
            );
        }

        // Test that schema doesn't have extra variants
        let rust_variants = rename.apply_to_variants(ErrorKind::VARIANTS);
        for schema_variant in &schema_variants {
            assert!(
                rust_variants.contains(schema_variant),
                "Schema has extra ErrorKind variant not in Rust: {schema_variant}"
            );
        }
    }

    #[test]
    fn test_check_status_individual_variants() {
        // Test each variant individually for better error messages
        let schema = load_schema("schemas/doctor.v1.json");
        let schema_variants =
            extract_schema_enum(&schema, "/properties/checks/items/properties/status/enum");

        let rename = RenameAll::SnakeCase;

        // Test each Rust variant
        for variant in CheckStatus::VARIANTS {
            let transformed = rename.apply(variant);
            assert!(
                schema_variants.contains(&transformed),
                "Schema missing CheckStatus variant: {transformed} (transformed from {variant})"
            );
        }

        // Test that schema doesn't have extra variants
        let rust_variants = rename.apply_to_variants(CheckStatus::VARIANTS);
        for schema_variant in &schema_variants {
            assert!(
                rust_variants.contains(schema_variant),
                "Schema has extra CheckStatus variant not in Rust: {schema_variant}"
            );
        }
    }

    #[test]
    fn test_config_source_individual_variants() {
        // Test each variant individually for better error messages
        let schema = load_schema("schemas/status.v1.json");
        let schema_variants = extract_schema_enum(
            &schema,
            "/properties/effective_config/additionalProperties/properties/source/enum",
        );

        let rename = RenameAll::Lowercase;

        // Test each Rust variant
        for variant in ConfigSource::VARIANTS {
            let transformed = rename.apply(variant);
            assert!(
                schema_variants.contains(&transformed),
                "Schema missing ConfigSource variant: {transformed} (transformed from {variant})"
            );
        }

        // Test that schema doesn't have extra variants
        let rust_variants = rename.apply_to_variants(ConfigSource::VARIANTS);
        for schema_variant in &schema_variants {
            assert!(
                rust_variants.contains(schema_variant),
                "Schema has extra ConfigSource variant not in Rust: {schema_variant}"
            );
        }
    }

    /// Extract required fields from a schema
    fn extract_required_fields(schema: &Value) -> HashSet<String> {
        schema
            .get("required")
            .and_then(|v| v.as_array())
            .unwrap_or_else(|| panic!("Required fields not found in schema"))
            .iter()
            .filter_map(|v| v.as_str())
            .map(std::string::ToString::to_string)
            .collect()
    }

    #[test]
    fn test_receipt_required_fields() {
        // IMPORTANT: Update this list when Receipt struct fields change
        // This list should contain all non-Option<T> fields from the Receipt struct
        let rust_required_fields = vec![
            "schema_version",
            "emitted_at",
            "spec_id",
            "phase",
            "xchecker_version",
            "claude_cli_version",
            "model_full_name",
            "canonicalization_version",
            "canonicalization_backend",
            "flags",
            "runner",
            "packet",
            "outputs",
            "exit_code",
            "warnings",
        ];

        let rust_fields: HashSet<String> = rust_required_fields
            .iter()
            .map(|s| (*s).to_string())
            .collect();

        // Load schema and extract required fields
        let schema = load_schema("schemas/receipt.v1.json");
        let schema_fields = extract_required_fields(&schema);

        // Compare
        if rust_fields != schema_fields {
            let missing_in_schema: Vec<_> = rust_fields.difference(&schema_fields).collect();
            let extra_in_schema: Vec<_> = schema_fields.difference(&rust_fields).collect();

            panic!(
                "Receipt required fields mismatch!\n\
                 Rust non-Option fields: {rust_fields:?}\n\
                 Schema required fields: {schema_fields:?}\n\
                 Missing in schema: {missing_in_schema:?}\n\
                 Extra in schema: {extra_in_schema:?}"
            );
        }

        assert_eq!(
            rust_fields, schema_fields,
            "Receipt required fields should match between Rust and schema"
        );
    }

    #[test]
    fn test_status_required_fields() {
        // IMPORTANT: Update this list when StatusOutput struct fields change
        // This list should contain all non-Option<T> fields from the StatusOutput struct
        let rust_required_fields = [
            "schema_version",
            "emitted_at",
            "runner",
            "fallback_used",
            "canonicalization_version",
            "canonicalization_backend",
            "artifacts",
            "last_receipt_path",
            "effective_config",
        ];

        let rust_fields: HashSet<String> = rust_required_fields
            .iter()
            .map(|s| (*s).to_string())
            .collect();

        // Load schema and extract required fields
        let schema = load_schema("schemas/status.v1.json");
        let schema_fields = extract_required_fields(&schema);

        // Compare
        if rust_fields != schema_fields {
            let missing_in_schema: Vec<_> = rust_fields.difference(&schema_fields).collect();
            let extra_in_schema: Vec<_> = schema_fields.difference(&rust_fields).collect();

            panic!(
                "StatusOutput required fields mismatch!\n\
                 Rust non-Option fields: {rust_fields:?}\n\
                 Schema required fields: {schema_fields:?}\n\
                 Missing in schema: {missing_in_schema:?}\n\
                 Extra in schema: {extra_in_schema:?}"
            );
        }

        assert_eq!(
            rust_fields, schema_fields,
            "StatusOutput required fields should match between Rust and schema"
        );
    }

    #[test]
    fn test_doctor_required_fields() {
        // IMPORTANT: Update this list when DoctorOutput struct fields change
        // This list should contain all non-Option<T> fields from the DoctorOutput struct
        let rust_required_fields = ["schema_version", "emitted_at", "ok", "checks"];

        let rust_fields: HashSet<String> = rust_required_fields
            .iter()
            .map(|s| (*s).to_string())
            .collect();

        // Load schema and extract required fields
        let schema = load_schema("schemas/doctor.v1.json");
        let schema_fields = extract_required_fields(&schema);

        // Compare
        if rust_fields != schema_fields {
            let missing_in_schema: Vec<_> = rust_fields.difference(&schema_fields).collect();
            let extra_in_schema: Vec<_> = schema_fields.difference(&rust_fields).collect();

            panic!(
                "DoctorOutput required fields mismatch!\n\
                 Rust non-Option fields: {rust_fields:?}\n\
                 Schema required fields: {schema_fields:?}\n\
                 Missing in schema: {missing_in_schema:?}\n\
                 Extra in schema: {extra_in_schema:?}"
            );
        }

        assert_eq!(
            rust_fields, schema_fields,
            "DoctorOutput required fields should match between Rust and schema"
        );
    }
}

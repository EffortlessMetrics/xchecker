//! Tests for enum introspection with strum `VariantNames`

#[cfg(test)]
mod tests {
    use strum::VariantNames;
    use xchecker::doctor::CheckStatus;
    use xchecker::types::{ConfigSource, ErrorKind};

    #[test]
    fn test_error_kind_has_variant_names() {
        // Verify ErrorKind has VariantNames trait
        let variants = ErrorKind::VARIANTS;

        assert!(variants.contains(&"CliArgs"));
        assert!(variants.contains(&"PacketOverflow"));
        assert!(variants.contains(&"SecretDetected"));
        assert!(variants.contains(&"LockHeld"));
        assert!(variants.contains(&"PhaseTimeout"));
        assert!(variants.contains(&"ClaudeFailure"));
        assert!(variants.contains(&"Unknown"));
        assert_eq!(variants.len(), 7);
    }

    #[test]
    fn test_check_status_has_variant_names() {
        // Verify CheckStatus has VariantNames trait
        let variants = CheckStatus::VARIANTS;

        assert!(variants.contains(&"Pass"));
        assert!(variants.contains(&"Warn"));
        assert!(variants.contains(&"Fail"));
        assert_eq!(variants.len(), 3);
    }

    #[test]
    fn test_config_source_has_variant_names() {
        // Verify ConfigSource has VariantNames trait
        let variants = ConfigSource::VARIANTS;

        assert!(variants.contains(&"Cli"));
        assert!(variants.contains(&"Config"));
        assert!(variants.contains(&"Programmatic"));
        assert!(variants.contains(&"Default"));
        assert_eq!(variants.len(), 4);
    }

    #[test]
    fn test_error_kind_with_rename_all() {
        use crate::doc_validation::common::RenameAll;

        let rename = RenameAll::SnakeCase;
        let variants = ErrorKind::VARIANTS;
        let transformed = rename.apply_to_variants(variants);

        // Verify snake_case transformation matches serde serialization
        assert!(transformed.contains("cli_args"));
        assert!(transformed.contains("packet_overflow"));
        assert!(transformed.contains("secret_detected"));
        assert!(transformed.contains("lock_held"));
        assert!(transformed.contains("phase_timeout"));
        assert!(transformed.contains("claude_failure"));
        assert!(transformed.contains("unknown"));
    }

    #[test]
    fn test_check_status_with_rename_all() {
        use crate::doc_validation::common::RenameAll;

        let rename = RenameAll::SnakeCase;
        let variants = CheckStatus::VARIANTS;
        let transformed = rename.apply_to_variants(variants);

        // Verify snake_case transformation matches serde serialization
        assert!(transformed.contains("pass"));
        assert!(transformed.contains("warn"));
        assert!(transformed.contains("fail"));
    }

    #[test]
    fn test_config_source_with_rename_all() {
        use crate::doc_validation::common::RenameAll;

        let rename = RenameAll::Lowercase;
        let variants = ConfigSource::VARIANTS;
        let transformed = rename.apply_to_variants(variants);

        // Verify lowercase transformation matches serde serialization
        assert!(transformed.contains("cli"));
        assert!(transformed.contains("config"));
        assert!(transformed.contains("programmatic"));
        assert!(transformed.contains("default"));
    }
}

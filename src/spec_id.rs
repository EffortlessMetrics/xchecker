//! Spec ID sanitization and validation
//!
//! This module provides functionality to sanitize and validate spec IDs
//! to ensure they are safe for use as directory names and identifiers.

use crate::error::{ErrorCategory, UserFriendlyError};
use unicode_normalization::UnicodeNormalization;

/// Error type for spec ID validation failures
#[derive(Debug, thiserror::Error)]
pub enum SpecIdError {
    #[error("Spec ID is empty after sanitization")]
    Empty,

    #[error("Spec ID contains only invalid characters")]
    OnlyInvalidCharacters,
}

impl UserFriendlyError for SpecIdError {
    fn user_message(&self) -> String {
        match self {
            Self::Empty => "The spec ID is empty or contains no valid characters".to_string(),
            Self::OnlyInvalidCharacters => {
                "The spec ID contains only invalid characters (no alphanumeric, dots, or dashes)"
                    .to_string()
            }
        }
    }

    fn context(&self) -> Option<String> {
        Some("Spec IDs are used as directory names and must contain valid filesystem characters. Only ASCII alphanumeric characters, dots (.), dashes (-), and underscores (_) are allowed. Invalid characters are automatically replaced with underscores.".to_string())
    }

    fn suggestions(&self) -> Vec<String> {
        match self {
            Self::Empty => vec![
                "Provide a non-empty spec ID".to_string(),
                "Use alphanumeric characters, dots, dashes, or underscores".to_string(),
                "Example: my-api-spec, user-auth-v2, payment-system".to_string(),
                "Avoid using only special characters or whitespace".to_string(),
            ],
            Self::OnlyInvalidCharacters => vec![
                "Include at least one alphanumeric character, dot, or dash".to_string(),
                "Valid characters: A-Z, a-z, 0-9, . (dot), - (dash), _ (underscore)".to_string(),
                "Example: my-spec, api-v2, user_auth".to_string(),
                "Avoid using only special characters like !@#$%^&*()".to_string(),
                "Unicode characters will be replaced with underscores".to_string(),
            ],
        }
    }

    fn category(&self) -> ErrorCategory {
        ErrorCategory::Validation
    }
}

/// Sanitizes a spec ID to ensure it's safe for filesystem use
///
/// This function:
/// - Normalizes Unicode with NFKC to handle confusables
/// - Accepts only [A-Za-z0-9._-]
/// - Replaces invalid characters with underscore
/// - Rejects control characters and whitespace
/// - Warns user when sanitization occurs
/// - Rejects empty IDs after sanitization
///
/// # Arguments
///
/// * `id` - The raw spec ID to sanitize
///
/// # Returns
///
/// * `Ok(String)` - The sanitized spec ID
/// * `Err(SpecIdError)` - If the ID is empty after sanitization
///
/// # Examples
///
/// ```
/// use xchecker::spec_id::sanitize_spec_id;
///
/// // Valid ID passes through unchanged
/// assert_eq!(sanitize_spec_id("my-spec_123").unwrap(), "my-spec_123");
///
/// // Invalid characters are replaced with underscores
/// assert_eq!(sanitize_spec_id("my spec!").unwrap(), "my_spec_");
///
/// // Unicode confusables are normalized
/// assert_eq!(sanitize_spec_id("ｍｙ－ｓｐｅｃ").unwrap(), "my-spec");
/// ```
pub fn sanitize_spec_id(id: &str) -> Result<String, SpecIdError> {
    // Step 1: Normalize with NFKC (Unicode normalization) to handle confusables
    let normalized: String = id.nfkc().collect();

    // Step 2: Filter and replace invalid characters
    let mut sanitized: String = normalized
        .chars()
        .map(|c| {
            // Accept [A-Za-z0-9._-]
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                // Replace everything else with underscore
                '_'
            }
        })
        .collect();

    // Step 2.5: Replace consecutive dots to prevent path traversal
    while sanitized.contains("..") {
        sanitized = sanitized.replace("..", "__");
    }

    // Step 3: Check if empty after sanitization
    if sanitized.is_empty() {
        return Err(SpecIdError::Empty);
    }

    // Step 4: Check if result contains only underscores (no meaningful content)
    // We allow dots, dashes, and underscores, but if there are ONLY underscores
    // and no alphanumeric, dots, or dashes, it's invalid
    let has_meaningful_content = sanitized
        .chars()
        .any(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
    if !has_meaningful_content {
        return Err(SpecIdError::OnlyInvalidCharacters);
    }

    // Step 5: Warn user if sanitization occurred
    if sanitized != id {
        let redacted_original = crate::redaction::redact_user_string(id);
        let redacted_sanitized = crate::redaction::redact_user_string(&sanitized);
        eprintln!(
            "Warning: spec ID sanitized from '{redacted_original}' to '{redacted_sanitized}'"
        );
    }

    Ok(sanitized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::UserFriendlyError;

    #[test]
    fn test_spec_id_error_user_friendly() {
        // Test Empty error
        let empty_err = SpecIdError::Empty;
        assert!(!empty_err.user_message().is_empty());
        assert!(empty_err.context().is_some());
        assert!(!empty_err.suggestions().is_empty());

        // Test OnlyInvalidCharacters error
        let invalid_err = SpecIdError::OnlyInvalidCharacters;
        assert!(!invalid_err.user_message().is_empty());
        assert!(invalid_err.context().is_some());
        assert!(!invalid_err.suggestions().is_empty());

        // Verify suggestions are actionable
        let suggestions = invalid_err.suggestions();
        assert!(suggestions.iter().any(|s| s.contains("Example:")));
    }

    #[test]
    fn test_valid_spec_id_unchanged() {
        // Valid IDs should pass through unchanged
        assert_eq!(sanitize_spec_id("my-spec").unwrap(), "my-spec");
        assert_eq!(sanitize_spec_id("my_spec").unwrap(), "my_spec");
        assert_eq!(sanitize_spec_id("my.spec").unwrap(), "my.spec");
        assert_eq!(sanitize_spec_id("MySpec123").unwrap(), "MySpec123");
        assert_eq!(sanitize_spec_id("spec-123_v2.0").unwrap(), "spec-123_v2.0");
    }

    #[test]
    fn test_invalid_characters_replaced() {
        // Invalid characters should be replaced with underscores
        assert_eq!(sanitize_spec_id("my spec").unwrap(), "my_spec");
        assert_eq!(sanitize_spec_id("my/spec").unwrap(), "my_spec");
        assert_eq!(sanitize_spec_id("my\\spec").unwrap(), "my_spec");
        assert_eq!(sanitize_spec_id("my:spec").unwrap(), "my_spec");
        assert_eq!(sanitize_spec_id("my*spec").unwrap(), "my_spec");
        assert_eq!(sanitize_spec_id("my?spec").unwrap(), "my_spec");
        assert_eq!(sanitize_spec_id("my\"spec").unwrap(), "my_spec");
        assert_eq!(sanitize_spec_id("my<spec>").unwrap(), "my_spec_");
        assert_eq!(sanitize_spec_id("my|spec").unwrap(), "my_spec");
        assert_eq!(
            sanitize_spec_id("my!@#$%^&*()spec").unwrap(),
            "my__________spec"
        );
    }

    #[test]
    fn test_unicode_confusables_normalized() {
        // Full-width characters should be normalized to ASCII
        assert_eq!(sanitize_spec_id("ｍｙ－ｓｐｅｃ").unwrap(), "my-spec");
        assert_eq!(sanitize_spec_id("ＭｙＳｐｅｃ１２３").unwrap(), "MySpec123");

        // Other Unicode confusables - these normalize to multi-char sequences
        // ⓜ -> (m), ⓨ -> (y), ⒮ -> (s), ⓟ -> (p), ⓔ -> (e), ⓒ -> (c)
        // The parentheses get replaced with underscores
        let result = sanitize_spec_id("ⓜⓨ⒮ⓟⓔⓒ").unwrap();
        // Just verify it contains the letters and underscores
        assert!(result.contains('m'));
        assert!(result.contains('y'));
        assert!(result.contains('s'));
        assert!(result.contains('p'));
        assert!(result.contains('e'));
        assert!(result.contains('c'));
    }

    #[test]
    fn test_control_characters_replaced() {
        // Control characters should be replaced
        assert_eq!(sanitize_spec_id("my\nspec").unwrap(), "my_spec");
        assert_eq!(sanitize_spec_id("my\tspec").unwrap(), "my_spec");
        assert_eq!(sanitize_spec_id("my\rspec").unwrap(), "my_spec");
        assert_eq!(sanitize_spec_id("my\x00spec").unwrap(), "my_spec");
        assert_eq!(sanitize_spec_id("my\x1Fspec").unwrap(), "my_spec");
    }

    #[test]
    fn test_whitespace_replaced() {
        // All whitespace should be replaced
        assert_eq!(sanitize_spec_id("my spec").unwrap(), "my_spec");
        assert_eq!(sanitize_spec_id("my  spec").unwrap(), "my__spec");
        assert_eq!(sanitize_spec_id("  my-spec  ").unwrap(), "__my-spec__");
        assert_eq!(sanitize_spec_id("my\u{00A0}spec").unwrap(), "my_spec"); // non-breaking space
    }

    #[test]
    fn test_empty_id_rejected() {
        // Empty string should be rejected
        assert!(matches!(sanitize_spec_id(""), Err(SpecIdError::Empty)));
    }

    #[test]
    fn test_only_invalid_characters_rejected() {
        // IDs with only invalid characters should be rejected
        assert!(matches!(
            sanitize_spec_id("!!!"),
            Err(SpecIdError::OnlyInvalidCharacters)
        ));
        assert!(matches!(
            sanitize_spec_id("   "),
            Err(SpecIdError::OnlyInvalidCharacters)
        ));
        assert!(matches!(
            sanitize_spec_id("@#$%"),
            Err(SpecIdError::OnlyInvalidCharacters)
        ));
    }

    #[test]
    fn test_mixed_valid_invalid() {
        // Mix of valid and invalid characters
        assert_eq!(sanitize_spec_id("my-spec!@#").unwrap(), "my-spec___");
        assert_eq!(sanitize_spec_id("!!!my-spec").unwrap(), "___my-spec");
        assert_eq!(sanitize_spec_id("my!!!spec").unwrap(), "my___spec");
    }

    #[test]
    fn test_unicode_emoji_replaced() {
        // Emoji should be replaced
        assert_eq!(sanitize_spec_id("my-spec-🚀").unwrap(), "my-spec-_");
        assert_eq!(sanitize_spec_id("🎉party🎊").unwrap(), "_party_");
    }

    #[test]
    fn test_unicode_letters_replaced() {
        // Non-ASCII letters should be replaced (only ASCII alphanumeric allowed)
        assert_eq!(sanitize_spec_id("café").unwrap(), "caf_");
        assert_eq!(sanitize_spec_id("naïve").unwrap(), "na_ve");
        // Japanese characters result in only underscores, which is invalid
        assert!(matches!(
            sanitize_spec_id("日本語"),
            Err(SpecIdError::OnlyInvalidCharacters)
        ));
    }

    #[test]
    fn test_long_spec_id() {
        // Long IDs should be sanitized but not truncated
        let long_id = "a".repeat(200);
        assert_eq!(sanitize_spec_id(&long_id).unwrap(), long_id);

        let long_invalid = "!".repeat(200);
        assert!(matches!(
            sanitize_spec_id(&long_invalid),
            Err(SpecIdError::OnlyInvalidCharacters)
        ));
    }

    #[test]
    fn test_edge_cases() {
        // Single character IDs
        assert_eq!(sanitize_spec_id("a").unwrap(), "a");
        assert_eq!(sanitize_spec_id("1").unwrap(), "1");
        assert_eq!(sanitize_spec_id("-").unwrap(), "-");
        assert_eq!(sanitize_spec_id(".").unwrap(), ".");

        // Dots and dashes are meaningful
        // Note: consecutive dots are replaced with underscores to prevent path traversal
        assert_eq!(sanitize_spec_id("...").unwrap(), "__.");
        assert_eq!(sanitize_spec_id("---").unwrap(), "---");

        // Only underscores is not meaningful (result of replacing invalid chars)
        assert!(matches!(
            sanitize_spec_id("_"),
            Err(SpecIdError::OnlyInvalidCharacters)
        ));
        assert!(matches!(
            sanitize_spec_id("___"),
            Err(SpecIdError::OnlyInvalidCharacters)
        ));
    }

    #[test]
    fn test_nfkc_normalization() {
        // NFKC should normalize compatibility characters
        // ﬁ (U+FB01) -> fi
        assert_eq!(sanitize_spec_id("ﬁle").unwrap(), "file");

        // ² (U+00B2) -> 2
        assert_eq!(sanitize_spec_id("spec²").unwrap(), "spec2");

        // ℃ (U+2103) -> °C -> _C
        assert_eq!(sanitize_spec_id("temp℃").unwrap(), "temp_C");
    }
}

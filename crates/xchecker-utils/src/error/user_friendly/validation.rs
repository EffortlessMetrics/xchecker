use super::UserFriendlyError;
use crate::error::{ErrorCategory, SpecIdError};

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

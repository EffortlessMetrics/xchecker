use crate::error::ErrorCategory;

mod aggregate;
mod config;
mod fixup;
mod lock;
mod phase;
mod provider;
mod source;
mod validation;

/// Trait for providing user-friendly error reporting with context and suggestions
pub trait UserFriendlyError {
    /// Get a user-friendly error message
    fn user_message(&self) -> String;

    /// Get contextual information about the error
    fn context(&self) -> Option<String>;

    /// Get suggested actions to resolve the error
    fn suggestions(&self) -> Vec<String>;

    /// Get the error category for grouping similar errors
    fn category(&self) -> ErrorCategory;
}

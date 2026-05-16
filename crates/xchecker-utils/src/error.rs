//! Error types and user-facing reporting for xchecker.
//!
//! This module is split by responsibility:
//! - `kinds` contains the error enums and lightweight trait impls.
//! - `category` contains user-facing error category labels.
//! - `user_friendly` maps errors to messages, context, and suggestions.
//! - `xchecker` contains top-level formatting and exit-code mapping.

mod category;
mod kinds;
mod user_friendly;
mod xchecker;

pub use category::ErrorCategory;
pub use kinds::{
    ClaudeError, ConfigError, FixupError, LlmError, LockError, PhaseError, RunnerError,
    SourceError, SpecIdError, ValidationError, XCheckerError,
};
pub use user_friendly::UserFriendlyError;

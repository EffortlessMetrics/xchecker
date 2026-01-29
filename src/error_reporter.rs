//! Error reporting utilities for user-friendly error display
//!
//! This module is a thin re-export facade for the `xchecker-error-reporter` crate.
//! The actual implementation has been extracted to a separate crate for better modularity.

// Re-export everything from xchecker-error-reporter
pub use xchecker_error_reporter::*;

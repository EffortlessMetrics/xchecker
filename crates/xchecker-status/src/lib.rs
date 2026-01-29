//! Status output generation for xchecker.
//!
//! This crate provides functionality to generate structured JSON status outputs
//! with canonical emission using JCS (RFC 8785) for stable diffs across platforms.
//!
//! # Modules
//!
//! - [`artifact`] - Artifact management with atomic writes and directory structure
//! - [`status`] - Status output generation

pub use xchecker_receipt as receipt;
pub use xchecker_redaction as redaction;
pub use xchecker_utils::atomic_write;
pub use xchecker_utils::canonicalization;
pub use xchecker_utils::error;
pub use xchecker_utils::lock;
pub use xchecker_utils::paths;
pub use xchecker_utils::types;

pub mod artifact;
pub mod status;

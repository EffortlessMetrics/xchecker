//! xchecker-cli - CLI interface for xchecker
//!
//! This crate provides the command-line interface for the xchecker tool,
//! including command parsing, argument handling, and CLI-specific logic.
//!
//! The main CLI implementation currently lives in `src/cli.rs` of the root crate.
//! This microcrate re-exports stable types for downstream consumers and will
//! absorb the full CLI implementation in a future refactor.

// Re-export types from their new locations after modularization
pub use xchecker_config::{CliArgs, Config};
pub use xchecker_utils::error::XCheckerError;
pub use xchecker_utils::exit_codes::ExitCode;
pub use xchecker_utils::types::PhaseId;

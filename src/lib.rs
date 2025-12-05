//! xchecker - A CLI tool for orchestrating spec generation workflows using the Claude CLI
//!
//! This crate provides a deterministic, token-efficient pipeline that transforms rough ideas
//! into detailed implementation plans through a structured phase-based approach.

/// Returns the xchecker version with embedded git revision
/// Format: "{`CARGO_PKG_VERSION}+{GIT_SHA`}"
#[must_use]
pub fn xchecker_version() -> String {
    format!("{}+{}", env!("CARGO_PKG_VERSION"), env!("GIT_HASH"))
}

pub mod paths;

pub mod artifact;
pub mod atomic_write;
pub mod benchmark;
pub mod cache;
pub mod canonicalization;
// Legacy wrapper; follow-up spec (V19+) to delete once tests migrate
#[cfg(any(test, feature = "legacy_claude"))]
pub mod claude;
pub mod cli;
pub mod config;
pub mod doctor;
pub mod error;
pub mod error_reporter;
pub mod example_generators;
pub mod exit_codes;
pub mod fixup;
pub mod gate;
pub mod hooks;
pub mod integration_tests;
pub mod llm;
pub mod lock;
pub mod logging;
pub mod orchestrator;
pub mod packet;
pub mod phase;
pub mod phases;
pub mod process_memory;
pub mod receipt;
pub mod redaction;
pub mod ring_buffer;
pub mod runner;
pub mod source;
pub mod spec_id;
pub mod status;
pub mod template;
pub mod types;
pub mod tui;
pub mod workspace;
pub mod wsl;

// Core types and errors used by external consumers
pub use config::{CliArgs, Config};
pub use error::{ErrorCategory, UserFriendlyError, XCheckerError};
pub use receipt::write_error_receipt_and_exit;
pub use spec_id::{SpecIdError, sanitize_spec_id};
pub use types::*;

// Re-exports removed for internal-only items that are not used externally.

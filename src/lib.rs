//! xchecker - Spec pipeline with receipts and gateable JSON contracts
//!
//! This crate provides a deterministic, token-efficient pipeline that transforms rough ideas
//! into detailed implementation plans through a structured phase-based approach.
//!
//! xchecker can be used in two ways:
//! - **CLI**: Install via `cargo install xchecker` and run from command line
//! - **Library**: Add as a dependency and use internal APIs to embed in your application
//!
//! # Quick Start (CLI)
//!
//! Install xchecker from crates.io:
//!
//! ```bash
//! cargo install xchecker
//! ```
//!
//! Run a spec generation workflow:
//!
//! ```bash
//! # Initialize a new spec
//! xchecker init my-feature
//!
//! # Run all phases (dry-run mode for testing)
//! xchecker spec my-feature --dry-run
//!
//! # Check spec status
//! xchecker status my-feature --json
//!
//! # Run environment health checks
//! xchecker doctor --json
//! ```
//!
//! # Quick Start (Library)
//!
//! Add xchecker to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! xchecker = "1"
//! tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
//! ```
//!
//! NOTE: The OrchestratorHandle API is temporarily unavailable due to modularization work.
//! For now, use the CLI directly or internal APIs (not covered by semver).
//!
//! # JSON Contracts
//!
//! xchecker emits JSON in JCS (RFC 8785) canonical form for deterministic output:
//!
//! - Receipts: `schemas/receipt.v1.json`
//! - Status: `schemas/status.v1.json`
//! - Doctor: `schemas/doctor.v1.json`
//!
//! Use [`emit_jcs`] to emit JSON in canonical form for your own integrations.
//!
//! # Stable Public API
//!
//! The following types are part of stable public API for 1.x releases:
//!
//! - [`PhaseId`] - Phase identifiers (Requirements, Design, Tasks, etc.)
//! - [`Config`] and [`ConfigBuilder`] - Configuration management
//! - [`XCheckerError`] - Library error type
//! - [`ExitCode`] - CLI exit codes
//! - [`StatusOutput`] - Spec status information
//! - [`emit_jcs`] - JCS canonical JSON emission
//!
//! Internal modules are accessible via module paths but are marked `#[doc(hidden)]`
//! and are not covered by semver stability guarantees.

// ============================================================================
// Stable Public API - covered by semver guarantees for 1.x
// ============================================================================

// Re-export orchestrator types for backward compatibility
pub use xchecker_engine::orchestrator::{OrchestratorConfig, OrchestratorHandle};

/// Phase identifiers for the spec generation workflow.
///
/// `PhaseId` represents different phases in xchecker's spec generation pipeline:
/// Requirements → Design → Tasks → Review → Fixup → Final.
///
/// See [`PhaseId`] documentation for phase dependencies and serialization details.
pub use xchecker_utils::types::PhaseId;

/// Configuration for xchecker operations.
///
/// `Config` provides hierarchical configuration with discovery and precedence:
/// CLI arguments > config file > built-in defaults.
///
/// Use [`Config::discover()`] for CLI-like behavior or [`Config::builder()`]
/// for programmatic configuration in embedding scenarios.
pub use xchecker_config::Config;

/// Builder for programmatic configuration.
///
/// `ConfigBuilder` allows constructing a [`Config`] programmatically without
/// relying on environment variables or config files. This is useful for
/// embedding xchecker where deterministic behavior is required.
///
/// # Example
///
/// ```rust,no_run
/// use xchecker::Config;
/// use std::time::Duration;
///
/// let config = Config::builder()
///     .state_dir("/custom/state")
///     .packet_max_bytes(65536)
///     .phase_timeout(Duration::from_secs(600))
///     .build()
///     .expect("Failed to build config");
/// ```
pub use xchecker_config::ConfigBuilder;

/// Library-level error type with rich context.
///
/// `XCheckerError` provides detailed error information including:
/// - Error kind for programmatic handling
/// - User-friendly messages via [`display_for_user()`](XCheckerError::display_for_user)
/// - Exit code mapping via [`to_exit_code()`](XCheckerError::to_exit_code)
///
/// Library code returns `XCheckerError` and does NOT call `std::process::exit()`.
pub use xchecker_utils::error::XCheckerError;

/// Exit codes matching the documented exit code table.
///
/// `ExitCode` provides type-safe exit code handling for xchecker operations.
/// Use named constants (e.g., [`ExitCode::SUCCESS`], [`ExitCode::PACKET_OVERFLOW`])
/// or [`as_i32()`](ExitCode::as_i32) to get the numeric value.
///
/// This is a stable public type. The numeric values are part of the public API
/// and will not change in 1.x releases.
pub use xchecker_utils::exit_codes::ExitCode;

/// Status output for a spec, matching `schemas/status.v1.json`.
///
/// `StatusOutput` provides comprehensive status information about a spec's current state,
/// including artifacts, configuration, and any detected drift from locked values.
///
/// This is a stable public type. Changes in 1.x releases are additive only.
pub use xchecker_utils::types::StatusOutput;

/// JCS (RFC 8785) canonical JSON emission for JSON contracts.
///
/// Use this function to emit JSON in canonical form for receipts, status, and
/// other JSON contracts. Canonical JSON ensures deterministic output for
/// stable diffs and hash verification.
pub use xchecker_utils::canonicalization::emit_jcs;

// Additional stable re-exports for convenience

/// CLI argument structure for configuration override.
///
/// Used internally by the CLI and for programmatic configuration via
/// [`Config::discover()`].
pub use xchecker_config::CliArgs;

/// Error categories for grouping similar errors.
///
/// Used with [`XCheckerError`] for programmatic error handling.
pub use xchecker_utils::error::ErrorCategory;

/// Trait for providing user-friendly error reporting.
///
/// Implemented by [`XCheckerError`] and its component error types.
pub use xchecker_utils::error::UserFriendlyError;

// ============================================================================
// Internal modules - accessible but not stable
// ============================================================================
// NOTE: cli module is NOT exported here - it's only used by main.rs via `mod cli;`

/// Returns xchecker version with embedded git revision
/// Format: "{`CARGO_PKG_VERSION}+{GIT_SHA`}"
#[must_use]
pub fn xchecker_version() -> String {
    format!("{}+{}", env!("CARGO_PKG_VERSION"), env!("GIT_HASH"))
}

#[cfg(any(test, feature = "test-utils"))]
#[doc(hidden)]
pub use xchecker_utils::test_support;

pub use xchecker_redaction as redaction;
#[doc(hidden)]
pub use xchecker_utils::{
    atomic_write, cache, canonicalization, error, exit_codes, lock, logging, paths, process_memory,
    ring_buffer, source, spec_id, types,
};

#[doc(hidden)]
pub use xchecker_config as config;

#[doc(hidden)]
pub use xchecker_llm as llm;

#[doc(hidden)]
pub use xchecker_engine::{
    benchmark, doctor, example_generators, extraction, fixup, gate, hooks, integration_tests,
    orchestrator, packet, phase, phases, receipt, runner, templates, validation, workspace,
};
pub use xchecker_status as status;

// Re-export artifact module from status crate for tests
#[doc(hidden)]
pub use xchecker_status::artifact;

// Re-export wsl module from doctor crate for tests
#[doc(hidden)]
pub use xchecker_doctor::wsl;

// Legacy wrapper; follow-up spec (V19+) to delete once tests migrate
#[cfg(feature = "legacy_claude")]
#[doc(hidden)]
pub use xchecker_engine::claude;

// CLI module - internal implementation detail, not part of stable public API
// Exported with #[doc(hidden)] to allow white-box testing of CLI flag parsing
// External consumers should use OrchestratorHandle, not CLI internals
#[doc(hidden)]
pub mod cli;
#[doc(hidden)]
pub mod error_reporter;
#[doc(hidden)]
pub mod tui;

// Legacy re-exports for backward compatibility (will be deprecated)
#[doc(hidden)]
pub use receipt::write_error_receipt_and_exit;
#[doc(hidden)]
pub use spec_id::{SpecIdError, sanitize_spec_id};

//! xchecker-cli - CLI interface for xchecker
//!
//! This crate provides the command-line interface for the xchecker tool,
//! including command parsing, argument handling, and CLI-specific logic.

// Re-export types from their new locations after modularization
pub use xchecker_config::{CliArgs, Config};
pub use xchecker_utils::error::XCheckerError;
pub use xchecker_utils::exit_codes::ExitCode;
pub use xchecker_utils::types::PhaseId;

/// Main CLI entry point
///
/// This function parses command-line arguments and executes the appropriate command.
pub async fn run_cli() -> Result<ExitCode, anyhow::Error> {
    // CLI logic will be extracted from src/cli.rs
    // For now, this is a placeholder
    todo!("CLI extraction in progress - see src/cli.rs for current implementation")
}

/// Parse CLI arguments
///
/// This function parses command-line arguments and returns the parsed configuration.
pub fn parse_args() -> Result<CliArgs, anyhow::Error> {
    // CLI argument parsing will be extracted from src/cli.rs
    // For now, this is a placeholder
    todo!("CLI argument parsing extraction in progress - see src/cli.rs for current implementation")
}

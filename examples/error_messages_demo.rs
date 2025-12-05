//! Demonstration of improved error messages and user guidance
//!
//! This example shows how xchecker provides clear, actionable error messages
//! with context and suggestions for common error scenarios.

use std::path::PathBuf;
use xchecker::error::{ConfigError, PhaseError};
use xchecker::error_reporter::ErrorReport;
use xchecker::fixup::FixupError;
use xchecker::spec_id::SpecIdError;

fn main() {
    println!("=== xchecker Error Messages Demo ===\n");

    // Example 1: Configuration Error
    println!("1. Configuration Error - Missing Required Value:");
    println!("{}", "─".repeat(60));
    let config_err = ConfigError::MissingRequired("model".to_string());
    let report = ErrorReport::new(&config_err);
    println!("{}\n", report.format());

    // Example 2: Phase Execution Error
    println!("2. Phase Execution Error - Requirements Phase Failed:");
    println!("{}", "─".repeat(60));
    let phase_err = PhaseError::ExecutionFailed {
        phase: "REQUIREMENTS".to_string(),
        code: 1,
    };
    let report = ErrorReport::new(&phase_err);
    println!("{}\n", report.format());

    // Example 3: Spec ID Validation Error
    println!("3. Spec ID Validation Error - Invalid Characters:");
    println!("{}", "─".repeat(60));
    let spec_id_err = SpecIdError::OnlyInvalidCharacters;
    let report = ErrorReport::new(&spec_id_err);
    println!("{}\n", report.format());

    // Example 4: Fixup Security Error
    println!("4. Fixup Security Error - Symlink Not Allowed:");
    println!("{}", "─".repeat(60));
    let fixup_err = FixupError::SymlinkNotAllowed(PathBuf::from("config/settings.toml"));
    let report = ErrorReport::new(&fixup_err);
    println!("{}\n", report.format());

    // Example 5: Fixup Path Validation Error
    println!("5. Fixup Path Validation Error - Parent Directory Escape:");
    println!("{}", "─".repeat(60));
    let fixup_err = FixupError::ParentDirEscape(PathBuf::from("../../etc/passwd"));
    let report = ErrorReport::new(&fixup_err);
    println!("{}\n", report.format());

    // Example 6: Phase Dependency Error
    println!("6. Phase Dependency Error - Missing Prerequisite:");
    println!("{}", "─".repeat(60));
    let phase_err = PhaseError::DependencyNotSatisfied {
        phase: "DESIGN".to_string(),
        dependency: "REQUIREMENTS".to_string(),
    };
    let report = ErrorReport::new(&phase_err);
    println!("{}\n", report.format());

    // Example 7: Minimal Error Report (no context or suggestions)
    println!("7. Minimal Error Report (for scripting):");
    println!("{}", "─".repeat(60));
    let config_err = ConfigError::InvalidValue {
        key: "packet_max_bytes".to_string(),
        value: "not-a-number".to_string(),
    };
    let report = ErrorReport::minimal(&config_err);
    println!("{}\n", report.format());

    println!("=== Demo Complete ===");
    println!("\nKey Features:");
    println!("  ✓ Clear, user-friendly error messages");
    println!("  ✓ Contextual information explaining the error");
    println!("  ✓ Actionable suggestions for resolution");
    println!("  ✓ Categorized errors for better organization");
    println!("  ✓ Troubleshooting tips for common scenarios");
}

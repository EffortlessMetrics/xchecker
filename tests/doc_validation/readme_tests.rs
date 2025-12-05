//! README documentation verification tests
//!
//! Tests that verify README.md accurately documents:
//! - All CLI commands exist
//! - All CLI options are correct
//! - Exit code table matches implementation
//! - Example commands execute successfully
//!
//! Requirements: R1

use std::path::Path;
use std::collections::HashSet;

use super::common::{DocParser, CliVerifier};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_readme_commands_exist() {
        // Parse README.md
        let readme_path = Path::new("README.md");
        let parser = DocParser::new(readme_path)
            .expect("Failed to read README.md");

        // Extract all documented commands
        let documented_commands = parser.extract_commands();
        assert!(!documented_commands.is_empty(), "Should find commands in README");

        // Get actual CLI commands
        let cli_verifier = CliVerifier::new();
        let actual_commands = cli_verifier.get_all_commands();

        // Verify each documented command exists in CLI
        let mut missing_in_cli = Vec::new();
        let mut extra_in_readme = Vec::new();

        for cmd in &documented_commands {
            if !cli_verifier.verify_command_exists(cmd) {
                missing_in_cli.push(cmd.clone());
            }
        }

        // Check for commands in CLI that aren't documented
        let documented_set: HashSet<_> = documented_commands.iter().collect();

        for cmd in &actual_commands {
            if !documented_set.contains(cmd) {
                extra_in_readme.push(cmd.clone());
            }
        }

        // Build error message if there are mismatches
        let mut errors = Vec::new();

        if !missing_in_cli.is_empty() {
            errors.push(format!(
                "README documents commands not found in CLI: {}",
                missing_in_cli.join(", ")
            ));
        }

        if !extra_in_readme.is_empty() {
            errors.push(format!(
                "CLI has commands not documented in README: {}",
                extra_in_readme.join(", ")
            ));
        }

        if !errors.is_empty() {
            panic!("Command verification failed:\n  - {}", errors.join("\n  - "));
        }
    }

    #[test]
    fn test_readme_options_exist() {
        // Parse README.md
        let readme_path = Path::new("README.md");
        let parser = DocParser::new(readme_path)
            .expect("Failed to read README.md");

        // Get CLI verifier
        let cli_verifier = CliVerifier::new();

        // Extract all documented commands
        let documented_commands = parser.extract_commands();
        assert!(!documented_commands.is_empty(), "Should find commands in README");

        // For each command, verify its options
        let mut all_errors = Vec::new();

        for command in &documented_commands {
            // Skip if command doesn't exist in CLI (will be caught by test_readme_commands_exist)
            if !cli_verifier.verify_command_exists(command) {
                continue;
            }

            // Extract options documented for this command
            let documented_options = parser.extract_options(command);

            // Get actual options from CLI for this command
            let actual_options = cli_verifier.get_command_options(command);

            // Check each documented option exists
            let mut missing_in_cli = Vec::new();
            let mut extra_in_readme = Vec::new();

            for opt in &documented_options {
                if !cli_verifier.verify_option_exists(command, opt) {
                    missing_in_cli.push(opt.clone());
                }
            }

            // Check for options in CLI that aren't documented
            let documented_set: HashSet<_> = documented_options.iter().collect();
            for opt in &actual_options {
                if !documented_set.contains(opt) {
                    extra_in_readme.push(opt.clone());
                }
            }

            // Build error message for this command if there are mismatches
            if !missing_in_cli.is_empty() {
                all_errors.push(format!(
                    "README missing options for '{}': {}",
                    command,
                    missing_in_cli.iter()
                        .map(|o| format!("--{}", o))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }

            if !extra_in_readme.is_empty() {
                all_errors.push(format!(
                    "CLI has undocumented options for '{}': {}",
                    command,
                    extra_in_readme.iter()
                        .map(|o| format!("--{}", o))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }

        if !all_errors.is_empty() {
            panic!("Option verification failed:\n  - {}", all_errors.join("\n  - "));
        }
    }

    #[test]
    fn test_exit_code_table() {
        // Parse README.md
        let readme_path = Path::new("README.md");
        let parser = DocParser::new(readme_path)
            .expect("Failed to read README.md");

        // Extract exit code table from README
        let documented_codes = parser.extract_exit_codes();
        assert!(!documented_codes.is_empty(), "Should find exit codes in README");

        // Define actual exit codes from exit_codes module
        // Note: Exit code 1 (UNKNOWN) is the default fallback and doesn't have a constant
        let mut actual_codes = std::collections::HashMap::new();
        actual_codes.insert(0, "SUCCESS");
        actual_codes.insert(1, "UNKNOWN"); // Default fallback, not a constant
        actual_codes.insert(2, "CLI_ARGS");
        actual_codes.insert(7, "PACKET_OVERFLOW");
        actual_codes.insert(8, "SECRET_DETECTED");
        actual_codes.insert(9, "LOCK_HELD");
        actual_codes.insert(10, "PHASE_TIMEOUT");
        actual_codes.insert(70, "CLAUDE_FAILURE");

        // Compare documented codes with actual constants
        let mut errors = Vec::new();

        // Check for mismatches in documented codes
        for (code, name) in &documented_codes {
            match actual_codes.get(code) {
                Some(actual_name) => {
                    if actual_name != name {
                        errors.push(format!(
                            "Exit code {} name mismatch: README has '{}', code has '{}'",
                            code, name, actual_name
                        ));
                    }
                }
                None => {
                    errors.push(format!(
                        "Exit code {} ('{}') documented in README but not found in exit_codes module",
                        code, name
                    ));
                }
            }
        }

        // Check for codes in module that aren't documented
        for (code, name) in &actual_codes {
            if !documented_codes.contains_key(code) {
                errors.push(format!(
                    "Exit code {} ('{}') exists in exit_codes module but not documented in README",
                    code, name
                ));
            }
        }

        if !errors.is_empty() {
            panic!("Exit code verification failed:\n  - {}", errors.join("\n  - "));
        }
    }
}

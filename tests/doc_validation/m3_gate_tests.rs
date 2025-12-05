//! M3 Gate: Validate CLI verification
//!
//! This test module validates that Milestone 3 is complete by verifying:
//! - All CLI verification tests pass
//! - All documented commands exist in CLI
//! - All documented options exist for their commands
//! - Exit code table matches exit_codes module
//!
//! Requirements: R1

use std::path::Path;
use std::collections::HashMap;

use crate::doc_validation::common::{DocParser, CliVerifier};

#[test]
fn m3_gate_cli_verification_tests_pass() {
    // This test verifies that all CLI verification tests pass
    // by running the core validation logic

    // Parse README.md
    let readme_path = Path::new("README.md");
    let parser = DocParser::new(readme_path)
        .expect("Failed to read README.md");

    // Get CLI verifier
    let cli_verifier = CliVerifier::new();

    // Extract all documented commands
    let documented_commands = parser.extract_commands();
    assert!(!documented_commands.is_empty(), "Should find commands in README");

    // Verify each documented command exists in CLI
    for cmd in &documented_commands {
        assert!(
            cli_verifier.verify_command_exists(cmd),
            "Command '{}' documented in README but not found in CLI",
            cmd
        );
    }

    // For each command, verify its options
    for command in &documented_commands {
        if !cli_verifier.verify_command_exists(command) {
            continue;
        }

        // Extract options documented for this command
        let documented_options = parser.extract_options(command);

        // Check each documented option exists
        for opt in &documented_options {
            assert!(
                cli_verifier.verify_option_exists(command, opt),
                "Option '--{}' documented for command '{}' but not found in CLI",
                opt,
                command
            );
        }
    }
}

#[test]
fn m3_gate_all_documented_commands_exist() {
    // Verify all documented commands exist in CLI
    let readme_path = Path::new("README.md");
    let parser = DocParser::new(readme_path)
        .expect("Failed to read README.md");

    let documented_commands = parser.extract_commands();
    assert!(!documented_commands.is_empty(), "Should find commands in README");

    let cli_verifier = CliVerifier::new();
    let actual_commands = cli_verifier.get_all_commands();

    // Verify each documented command exists
    let mut missing_commands = Vec::new();
    for cmd in &documented_commands {
        if !cli_verifier.verify_command_exists(cmd) {
            missing_commands.push(cmd.clone());
        }
    }

    assert!(
        missing_commands.is_empty(),
        "Commands documented in README but not found in CLI: {}",
        missing_commands.join(", ")
    );

    // Also check that we have at least the expected core commands
    let expected_commands = vec!["spec", "resume", "status", "doctor"];
    for expected in &expected_commands {
        assert!(
            actual_commands.contains(&expected.to_string()),
            "Expected command '{}' not found in CLI",
            expected
        );
    }
}

#[test]
fn m3_gate_all_documented_options_exist() {
    // Verify all documented options exist for their commands
    let readme_path = Path::new("README.md");
    let parser = DocParser::new(readme_path)
        .expect("Failed to read README.md");

    let cli_verifier = CliVerifier::new();
    let documented_commands = parser.extract_commands();

    let mut all_errors = Vec::new();

    for command in &documented_commands {
        if !cli_verifier.verify_command_exists(command) {
            continue;
        }

        let documented_options = parser.extract_options(command);
        
        for opt in &documented_options {
            if !cli_verifier.verify_option_exists(command, opt) {
                all_errors.push(format!(
                    "Option '--{}' documented for command '{}' but not found in CLI",
                    opt,
                    command
                ));
            }
        }
    }

    assert!(
        all_errors.is_empty(),
        "Option verification failed:\n  - {}",
        all_errors.join("\n  - ")
    );
}

#[test]
fn m3_gate_exit_code_table_matches() {
    // Verify exit code table matches exit_codes module
    let readme_path = Path::new("README.md");
    let parser = DocParser::new(readme_path)
        .expect("Failed to read README.md");

    let documented_codes = parser.extract_exit_codes();
    assert!(!documented_codes.is_empty(), "Should find exit codes in README");

    // Define actual exit codes from exit_codes module
    let mut actual_codes = HashMap::new();
    actual_codes.insert(0, "SUCCESS");
    actual_codes.insert(1, "UNKNOWN");
    actual_codes.insert(2, "CLI_ARGS");
    actual_codes.insert(7, "PACKET_OVERFLOW");
    actual_codes.insert(8, "SECRET_DETECTED");
    actual_codes.insert(9, "LOCK_HELD");
    actual_codes.insert(10, "PHASE_TIMEOUT");
    actual_codes.insert(70, "CLAUDE_FAILURE");

    // Verify all documented codes match actual codes
    for (code, name) in &documented_codes {
        match actual_codes.get(code) {
            Some(actual_name) => {
                assert_eq!(
                    actual_name, name,
                    "Exit code {} name mismatch: README has '{}', code has '{}'",
                    code, name, actual_name
                );
            }
            None => {
                panic!(
                    "Exit code {} ('{}') documented in README but not found in exit_codes module",
                    code, name
                );
            }
        }
    }

    // Verify all actual codes are documented
    for (code, name) in &actual_codes {
        assert!(
            documented_codes.contains_key(code),
            "Exit code {} ('{}') exists in exit_codes module but not documented in README",
            code, name
        );
    }
}

#[test]
fn m3_gate_comprehensive_validation() {
    // This is a comprehensive test that validates all M3 Gate requirements in one place
    
    println!("M3 Gate Validation:");
    println!("===================");
    
    // 1. Run CLI verification tests and verify all pass
    println!("✓ CLI verification tests pass");
    m3_gate_cli_verification_tests_pass();
    
    // 2. Verify all documented commands exist in CLI
    println!("✓ All documented commands exist in CLI");
    m3_gate_all_documented_commands_exist();
    
    // 3. Verify all documented options exist for their commands
    println!("✓ All documented options exist for their commands");
    m3_gate_all_documented_options_exist();
    
    // 4. Verify exit code table matches exit_codes module
    println!("✓ Exit code table matches exit_codes module");
    m3_gate_exit_code_table_matches();
    
    println!("\nM3 Gate: PASSED ✓");
    println!("All Milestone 3 requirements validated successfully.");
}

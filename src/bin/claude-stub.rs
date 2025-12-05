//! Stub Claude CLI for development testing
//!
//! This binary mimics the Claude CLI behavior for testing xchecker without
//! making actual API calls. It supports various response scenarios including
//! stream-json output format with realistic responses.

use clap::{Arg, Command};
use serde_json::json;
use std::io::{self, Write};
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = Command::new("claude-stub")
        .version("0.8.1")
        .about("Stub Claude CLI for testing")
        .arg(
            Arg::new("output-format")
                .long("output-format")
                .value_name("FORMAT")
                .help("Output format (stream-json or text)")
                .default_value("text"),
        )
        .arg(
            Arg::new("include-partial-messages")
                .long("include-partial-messages")
                .help("Include partial messages in stream-json output")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("model")
                .long("model")
                .value_name("MODEL")
                .help("Model to use")
                .default_value("haiku"),
        )
        .arg(
            Arg::new("max-turns")
                .long("max-turns")
                .value_name("N")
                .help("Maximum number of turns")
                .default_value("10"),
        )
        .arg(
            Arg::new("scenario")
                .long("scenario")
                .value_name("SCENARIO")
                .help("Test scenario to simulate")
                .default_value("success"),
        )
        .get_matches();

    let output_format = matches.get_one::<String>("output-format").unwrap();
    let scenario = matches.get_one::<String>("scenario").unwrap();

    match scenario.as_str() {
        "success" => handle_success_scenario(output_format)?,
        "partial" => handle_partial_scenario(output_format)?,
        "malformed" => handle_malformed_scenario(output_format)?,
        "text-fallback" => handle_text_fallback_scenario(output_format)?,
        "error" => handle_error_scenario()?,
        _ => handle_success_scenario(output_format)?,
    }

    Ok(())
}

fn handle_success_scenario(output_format: &str) -> Result<(), Box<dyn std::error::Error>> {
    if output_format == "stream-json" {
        emit_stream_json_success()?;
    } else {
        emit_text_success()?;
    }
    Ok(())
}

fn handle_partial_scenario(output_format: &str) -> Result<(), Box<dyn std::error::Error>> {
    if output_format == "stream-json" {
        emit_stream_json_partial()?;
    } else {
        emit_text_partial()?;
    }
    Ok(())
}

fn handle_malformed_scenario(output_format: &str) -> Result<(), Box<dyn std::error::Error>> {
    if output_format == "stream-json" {
        emit_malformed_json()?;
    } else {
        emit_text_success()?;
    }
    Ok(())
}

fn handle_text_fallback_scenario(_output_format: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Always emit malformed JSON first to trigger fallback
    emit_malformed_json()?;
    std::process::exit(1);
}

fn handle_error_scenario() -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("Error: Authentication failed");
    eprintln!("Please check your API key configuration");
    std::process::exit(1);
}

fn emit_stream_json_success() -> Result<(), Box<dyn std::error::Error>> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    // Emit conversation start event
    let start_event = json!({
        "type": "conversation_start",
        "conversation": {
            "id": "conv_123456789",
            "created_at": "2024-01-01T12:00:00Z"
        }
    });
    writeln!(handle, "{start_event}")?;
    handle.flush()?;
    thread::sleep(Duration::from_millis(100));

    // Emit message start event
    let message_start = json!({
        "type": "message_start",
        "message": {
            "id": "msg_123456789",
            "type": "message",
            "role": "assistant",
            "content": [],
            "model": "haiku",
            "stop_reason": null,
            "stop_sequence": null,
            "usage": {
                "input_tokens": 150,
                "output_tokens": 0
            }
        }
    });
    writeln!(handle, "{message_start}")?;
    handle.flush()?;
    thread::sleep(Duration::from_millis(50));

    // Emit content block start
    let content_start = json!({
        "type": "content_block_start",
        "index": 0,
        "content_block": {
            "type": "text",
            "text": ""
        }
    });
    writeln!(handle, "{content_start}")?;
    handle.flush()?;

    // Emit content deltas for requirements phase response
    let requirements_content = generate_requirements_response();
    let chunks: Vec<&str> = requirements_content.split_whitespace().collect();

    for (i, chunk) in chunks.iter().enumerate() {
        let delta = json!({
            "type": "content_block_delta",
            "index": 0,
            "delta": {
                "type": "text_delta",
                "text": if i == 0 { (*chunk).to_string() } else { format!(" {chunk}") }
            }
        });
        writeln!(handle, "{delta}")?;
        handle.flush()?;
        thread::sleep(Duration::from_millis(10));
    }

    // Emit content block stop
    let content_stop = json!({
        "type": "content_block_stop",
        "index": 0
    });
    writeln!(handle, "{content_stop}")?;
    handle.flush()?;

    // Emit message stop
    let message_stop = json!({
        "type": "message_stop",
        "message": {
            "id": "msg_123456789",
            "type": "message",
            "role": "assistant",
            "content": [
                {
                    "type": "text",
                    "text": requirements_content
                }
            ],
            "model": "haiku",
            "stop_reason": "end_turn",
            "stop_sequence": null,
            "usage": {
                "input_tokens": 150,
                "output_tokens": 450
            }
        }
    });
    writeln!(handle, "{message_stop}")?;
    handle.flush()?;

    Ok(())
}

fn emit_stream_json_partial() -> Result<(), Box<dyn std::error::Error>> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    // Start normally
    let start_event = json!({
        "type": "conversation_start",
        "conversation": {
            "id": "conv_123456789",
            "created_at": "2024-01-01T12:00:00Z"
        }
    });
    writeln!(handle, "{start_event}")?;
    handle.flush()?;

    let message_start = json!({
        "type": "message_start",
        "message": {
            "id": "msg_123456789",
            "type": "message",
            "role": "assistant",
            "content": [],
            "model": "haiku",
            "stop_reason": null,
            "stop_sequence": null,
            "usage": {
                "input_tokens": 150,
                "output_tokens": 0
            }
        }
    });
    writeln!(handle, "{message_start}")?;
    handle.flush()?;

    // Emit partial content then stop abruptly
    let content_start = json!({
        "type": "content_block_start",
        "index": 0,
        "content_block": {
            "type": "text",
            "text": ""
        }
    });
    writeln!(handle, "{content_start}")?;
    handle.flush()?;

    let partial_delta = json!({
        "type": "content_block_delta",
        "index": 0,
        "delta": {
            "type": "text_delta",
            "text": "# Requirements Document\n\n## Introduction\n\nThis document outlines the requirements for"
        }
    });
    writeln!(handle, "{partial_delta}")?;
    handle.flush()?;

    // Simulate interruption - no proper message_stop
    eprintln!("Connection interrupted");
    std::process::exit(1);
}

fn emit_malformed_json() -> Result<(), Box<dyn std::error::Error>> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    // Emit valid start then malformed JSON
    let start_event = json!({
        "type": "conversation_start",
        "conversation": {
            "id": "conv_123456789"
        }
    });
    writeln!(handle, "{start_event}")?;
    handle.flush()?;

    // Emit malformed JSON
    writeln!(
        handle,
        "{{\"type\": \"message_start\", \"message\": {{\"id\": \"msg_123"
    )?;
    handle.flush()?;

    eprintln!("JSON parsing error in stream");
    std::process::exit(1);
}

fn emit_text_success() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", generate_requirements_response());
    Ok(())
}

fn emit_text_partial() -> Result<(), Box<dyn std::error::Error>> {
    print!(
        "# Requirements Document\n\n## Introduction\n\nThis document outlines the requirements for"
    );
    io::stdout().flush()?;
    eprintln!("Connection interrupted");
    std::process::exit(1);
}

fn generate_requirements_response() -> String {
    r#"# Requirements Document

## Introduction

This document outlines the requirements for a user authentication system that provides secure login, registration, and session management capabilities for web applications.

## Requirements

### Requirement 1

**User Story:** As a user, I want to create an account with email and password, so that I can access the application securely.

#### Acceptance Criteria

1. WHEN a user provides a valid email and password THEN the system SHALL create a new account
2. WHEN a user provides an invalid email format THEN the system SHALL reject the registration with a clear error message
3. WHEN a user provides a password shorter than 8 characters THEN the system SHALL reject the registration
4. WHEN a user attempts to register with an existing email THEN the system SHALL prevent duplicate account creation

### Requirement 2

**User Story:** As a registered user, I want to log in with my credentials, so that I can access my account and application features.

#### Acceptance Criteria

1. WHEN a user provides correct email and password THEN the system SHALL authenticate the user and create a session
2. WHEN a user provides incorrect credentials THEN the system SHALL reject the login attempt
3. WHEN a user fails login 5 times THEN the system SHALL temporarily lock the account for 15 minutes
4. WHEN a user successfully logs in THEN the system SHALL redirect them to the dashboard

### Requirement 3

**User Story:** As a logged-in user, I want my session to be maintained securely, so that I don't need to re-authenticate frequently while remaining secure.

#### Acceptance Criteria

1. WHEN a user is authenticated THEN the system SHALL maintain the session for 24 hours of inactivity
2. WHEN a session expires THEN the system SHALL require re-authentication
3. WHEN a user logs out THEN the system SHALL immediately invalidate the session
4. WHEN a user closes the browser THEN the system SHALL maintain the session if "remember me" was selected"#.to_string()
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    #[test]
    fn test_version_output() {
        let output = Command::new("cargo")
            .args(["run", "--bin", "claude-stub", "--", "--version"])
            .output()
            .expect("Failed to execute command");

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("0.8.1"));
    }

    #[test]
    fn test_success_scenario_text() {
        let output = Command::new("cargo")
            .args([
                "run",
                "--bin",
                "claude-stub",
                "--",
                "--output-format",
                "text",
                "--scenario",
                "success",
            ])
            .output()
            .expect("Failed to execute command");

        assert_eq!(output.status.code(), Some(0));
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("# Requirements Document"));
        assert!(stdout.contains("## Introduction"));
        assert!(stdout.contains("**User Story:**"));
        assert!(stdout.contains("#### Acceptance Criteria"));
    }

    #[test]
    fn test_success_scenario_stream_json() {
        let output = Command::new("cargo")
            .args([
                "run",
                "--bin",
                "claude-stub",
                "--",
                "--output-format",
                "stream-json",
                "--scenario",
                "success",
            ])
            .output()
            .expect("Failed to execute command");

        assert_eq!(output.status.code(), Some(0));
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("conversation_start"));
        assert!(stdout.contains("message_start"));
        assert!(stdout.contains("content_block_start"));
        assert!(stdout.contains("content_block_delta"));
        assert!(stdout.contains("message_stop"));
    }

    #[test]
    fn test_error_scenario() {
        let output = Command::new("cargo")
            .args(["run", "--bin", "claude-stub", "--", "--scenario", "error"])
            .output()
            .expect("Failed to execute command");

        assert_eq!(output.status.code(), Some(1));
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("Authentication failed"));
    }

    #[test]
    fn test_malformed_scenario() {
        let output = Command::new("cargo")
            .args([
                "run",
                "--bin",
                "claude-stub",
                "--",
                "--output-format",
                "stream-json",
                "--scenario",
                "malformed",
            ])
            .output()
            .expect("Failed to execute command");

        assert_eq!(output.status.code(), Some(1));
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("conversation_start"));
        // Should contain incomplete JSON
        assert!(stdout.contains("msg_123"));
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("JSON parsing error"));
    }

    #[test]
    fn test_partial_scenario() {
        let output = Command::new("cargo")
            .args([
                "run",
                "--bin",
                "claude-stub",
                "--",
                "--output-format",
                "stream-json",
                "--scenario",
                "partial",
            ])
            .output()
            .expect("Failed to execute command");

        assert_eq!(output.status.code(), Some(1));
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("conversation_start"));
        assert!(stdout.contains("message_start"));
        assert!(stdout.contains("Requirements Document"));
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("Connection interrupted"));
    }
}

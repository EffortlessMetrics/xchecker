//! Stub Claude CLI for development testing
//!
//! This binary mimics the Claude CLI behavior for testing xchecker without
//! making actual API calls. It supports various response scenarios including
//! stream-json output format with realistic responses.

use clap::{Arg, Command};
use serde_json::json;
use std::io::{self, IsTerminal, Read, Write};
use std::thread;
use std::time::Duration;

#[derive(Clone, Copy)]
enum PhaseKind {
    Requirements,
    Design,
    Tasks,
    Review,
    Fixup,
    Final,
}

#[derive(Clone, Copy)]
enum ResponseSize {
    Default,
    Small,
    Medium,
    Large,
}

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
        .arg(
            Arg::new("no-sleep")
                .long("no-sleep")
                .help("Disable artificial delays (for fast CI tests)")
                .action(clap::ArgAction::SetTrue),
        )
        .get_matches();

    let output_format = matches.get_one::<String>("output-format").unwrap();
    let scenario = matches.get_one::<String>("scenario").unwrap();
    let model = matches.get_one::<String>("model").unwrap();
    let no_sleep = matches.get_flag("no-sleep");

    let prompt = read_prompt();
    let phase = detect_phase(&prompt);
    let size = size_for_scenario(scenario);
    let response = build_response(phase, scenario, size);

    match scenario.as_str() {
        "success" | "text" => handle_success_scenario(output_format, no_sleep, model, &response)?,
        "partial" | "truncated" => handle_partial_scenario(
            output_format,
            no_sleep,
            model,
            &response,
            "Connection interrupted",
        )?,
        "malformed" | "text-fallback" => {
            handle_malformed_scenario(output_format, model, &response)?
        }
        "error" => handle_error_scenario(
            output_format,
            no_sleep,
            model,
            &response,
            "Error: Authentication failed\nPlease check your API key configuration",
        )?,
        "network" => handle_error_scenario(
            output_format,
            no_sleep,
            model,
            &response,
            "network error: connection failed",
        )?,
        "permission" => handle_error_scenario(
            output_format,
            no_sleep,
            model,
            &response,
            "permission denied: access is restricted",
        )?,
        "timeout" => handle_error_scenario(
            output_format,
            no_sleep,
            model,
            &response,
            "Request timeout: operation timed out",
        )?,
        "slow" => handle_slow_scenario(output_format, no_sleep, model, &response)?,
        "hang" | "block" => handle_hang_scenario()?,
        _ => handle_success_scenario(output_format, no_sleep, model, &response)?,
    }

    Ok(())
}

fn read_prompt() -> String {
    if io::stdin().is_terminal() {
        return String::new();
    }

    let mut prompt = String::new();
    let _ = io::stdin().read_to_string(&mut prompt);
    prompt
}

fn detect_phase(prompt: &str) -> PhaseKind {
    let lower = prompt.to_ascii_lowercase();

    if lower.contains("phase: design") || lower.contains("# design document") {
        PhaseKind::Design
    } else if lower.contains("phase: tasks") || lower.contains("# implementation plan") {
        PhaseKind::Tasks
    } else if lower.contains("phase: review") || lower.contains("# review") {
        PhaseKind::Review
    } else if lower.contains("phase: fixup") || lower.contains("# fixup") {
        PhaseKind::Fixup
    } else if lower.contains("phase: final") || lower.contains("# final") {
        PhaseKind::Final
    } else {
        PhaseKind::Requirements
    }
}

fn size_for_scenario(scenario: &str) -> ResponseSize {
    match scenario {
        "small" => ResponseSize::Small,
        "medium" => ResponseSize::Medium,
        "large" => ResponseSize::Large,
        _ => ResponseSize::Default,
    }
}

fn build_response(phase: PhaseKind, scenario: &str, size: ResponseSize) -> String {
    match (phase, scenario) {
        (PhaseKind::Review, "fixup_needed") => generate_review_with_fixups(),
        (PhaseKind::Requirements, _) => generate_requirements_response(size),
        (PhaseKind::Design, _) => generate_design_response(),
        (PhaseKind::Tasks, _) => generate_tasks_response(),
        (PhaseKind::Review, _) => generate_review_response(),
        (PhaseKind::Fixup, _) => generate_fixup_response(),
        (PhaseKind::Final, _) => "Final phase output.".to_string(),
    }
}

fn handle_success_scenario(
    output_format: &str,
    no_sleep: bool,
    model: &str,
    content: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if output_format == "stream-json" {
        emit_stream_json_success(model, content, no_sleep)?;
    } else {
        emit_text_success(content)?;
    }
    Ok(())
}

fn handle_partial_scenario(
    output_format: &str,
    no_sleep: bool,
    model: &str,
    content: &str,
    stderr_message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    emit_partial_output(output_format, no_sleep, model, content)?;
    eprintln!("{stderr_message}");
    std::process::exit(1);
}

fn handle_malformed_scenario(
    output_format: &str,
    model: &str,
    content: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if output_format == "stream-json" {
        emit_malformed_json(model)?;
    } else {
        emit_text_success(content)?;
    }
    Ok(())
}

fn handle_error_scenario(
    output_format: &str,
    no_sleep: bool,
    model: &str,
    content: &str,
    stderr_message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    emit_partial_output(output_format, no_sleep, model, content)?;
    eprintln!("{stderr_message}");
    std::process::exit(1);
}

fn handle_slow_scenario(
    output_format: &str,
    no_sleep: bool,
    model: &str,
    content: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if !no_sleep {
        thread::sleep(Duration::from_millis(500));
    }
    handle_success_scenario(output_format, no_sleep, model, content)
}

/// Blocks for a configurable duration to test timeout handling.
/// Duration is read from CLAUDE_STUB_HANG_SECS env var (default: 10 seconds).
fn handle_hang_scenario() -> Result<(), Box<dyn std::error::Error>> {
    let hang_secs: u64 = std::env::var("CLAUDE_STUB_HANG_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);

    thread::sleep(Duration::from_secs(hang_secs));

    // After hanging, return success (though the caller should have killed us by now)
    println!("# Hang scenario completed after {} seconds", hang_secs);
    Ok(())
}

fn emit_partial_output(
    output_format: &str,
    no_sleep: bool,
    model: &str,
    content: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if output_format == "stream-json" {
        emit_stream_json_partial(model, content, no_sleep)?;
    } else {
        emit_text_partial(content)?;
    }
    Ok(())
}

fn emit_stream_json_success(
    model: &str,
    content: &str,
    no_sleep: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    let output_tokens = content.split_whitespace().count() as u64;
    let input_tokens = 150;

    let start_event = json!({
        "type": "conversation_start",
        "conversation": {
            "id": "conv_123456789",
            "created_at": "2024-01-01T12:00:00Z"
        }
    });
    writeln!(handle, "{start_event}")?;
    handle.flush()?;
    if !no_sleep {
        thread::sleep(Duration::from_millis(80));
    }

    let message_start = json!({
        "type": "message_start",
        "message": {
            "id": "msg_123456789",
            "type": "message",
            "role": "assistant",
            "content": [],
            "model": model,
            "stop_reason": null,
            "stop_sequence": null,
            "usage": {
                "input_tokens": input_tokens,
                "output_tokens": 0
            }
        }
    });
    writeln!(handle, "{message_start}")?;
    handle.flush()?;
    if !no_sleep {
        thread::sleep(Duration::from_millis(40));
    }

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

    for chunk in chunk_text(content, 64) {
        let delta = json!({
            "type": "content_block_delta",
            "index": 0,
            "delta": {
                "type": "text_delta",
                "text": chunk
            }
        });
        writeln!(handle, "{delta}")?;
        handle.flush()?;
        if !no_sleep {
            thread::sleep(Duration::from_millis(5));
        }
    }

    let content_stop = json!({
        "type": "content_block_stop",
        "index": 0
    });
    writeln!(handle, "{content_stop}")?;
    handle.flush()?;

    let message_stop = json!({
        "type": "message_stop",
        "message": {
            "id": "msg_123456789",
            "type": "message",
            "role": "assistant",
            "content": [
                {
                    "type": "text",
                    "text": content
                }
            ],
            "model": model,
            "stop_reason": "end_turn",
            "stop_sequence": null,
            "usage": {
                "input_tokens": input_tokens,
                "output_tokens": output_tokens
            }
        }
    });
    writeln!(handle, "{message_stop}")?;
    handle.flush()?;

    Ok(())
}

fn emit_stream_json_partial(
    model: &str,
    content: &str,
    no_sleep: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    let start_event = json!({
        "type": "conversation_start",
        "conversation": {
            "id": "conv_123456789",
            "created_at": "2024-01-01T12:00:00Z"
        }
    });
    writeln!(handle, "{start_event}")?;
    handle.flush()?;
    if !no_sleep {
        thread::sleep(Duration::from_millis(30));
    }

    let message_start = json!({
        "type": "message_start",
        "message": {
            "id": "msg_123456789",
            "type": "message",
            "role": "assistant",
            "content": [],
            "model": model,
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

    let partial_text = partial_content(content);
    let partial_delta = json!({
        "type": "content_block_delta",
        "index": 0,
        "delta": {
            "type": "text_delta",
            "text": partial_text
        }
    });
    writeln!(handle, "{partial_delta}")?;
    handle.flush()?;

    Ok(())
}

fn emit_malformed_json(model: &str) -> Result<(), Box<dyn std::error::Error>> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    let start_event = json!({
        "type": "conversation_start",
        "conversation": {
            "id": "conv_123456789",
            "created_at": "2024-01-01T12:00:00Z"
        }
    });
    writeln!(handle, "{start_event}")?;
    handle.flush()?;

    writeln!(
        handle,
        "{{\"type\": \"message_start\", \"message\": {{\"id\": \"msg_123\", \"model\": \"{model}\""
    )?;
    handle.flush()?;

    eprintln!("JSON parsing error in stream");
    Ok(())
}

fn emit_text_success(content: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("{content}");
    Ok(())
}

fn emit_text_partial(content: &str) -> Result<(), Box<dyn std::error::Error>> {
    print!("{}", partial_content(content));
    io::stdout().flush()?;
    Ok(())
}

fn partial_content(content: &str) -> String {
    let mut lines = Vec::new();
    for line in content.lines().take(8) {
        lines.push(line);
    }
    lines.join("\n")
}

fn chunk_text(content: &str, max_chars: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut start = 0;
    let mut count = 0;

    for (idx, _) in content.char_indices() {
        if count >= max_chars {
            chunks.push(content[start..idx].to_string());
            start = idx;
            count = 0;
        }
        count += 1;
    }

    if start < content.len() {
        chunks.push(content[start..].to_string());
    }

    chunks
}

fn generate_requirements_response(size: ResponseSize) -> String {
    let mut content = r#"# Requirements Document

## Introduction

This document describes a user authentication system that provides secure registration,
login, and session management for web applications.

## Requirements

### Requirement 1

**User Story:** As a user, I want to create an account with email and password, so that I can access the application securely.

#### Acceptance Criteria

1. WHEN a user provides a valid email and password THEN the system SHALL create a new account
2. WHEN a user provides an invalid email format THEN the system SHALL reject the registration with a clear error message
3. WHEN a user provides a password shorter than 8 characters THEN the system SHALL reject the registration

### Requirement 2

**User Story:** As a registered user, I want to log in with my credentials, so that I can access my account and application features.

#### Acceptance Criteria

1. WHEN a user provides correct email and password THEN the system SHALL authenticate the user and create a session
2. WHEN a user provides incorrect credentials THEN the system SHALL reject the login attempt
3. WHEN a user fails login 5 times THEN the system SHALL lock the account for 15 minutes

### Requirement 3

**User Story:** As a logged-in user, I want my session to be maintained securely, so that I do not need to re-authenticate frequently.

#### Acceptance Criteria

1. WHEN a user is authenticated THEN the system SHALL maintain the session for 24 hours of inactivity
2. WHEN a session expires THEN the system SHALL require re-authentication
3. WHEN a user logs out THEN the system SHALL immediately invalidate the session

## Non-Functional Requirements

**NFR1 [Performance]:** The system SHALL respond within 200ms for login operations
**NFR2 [Security]:** The system SHALL use HTTPS for all authentication traffic
**NFR3 [Reliability]:** The system SHALL log authentication failures for audit purposes
"#
    .to_string();

    let extra_lines = match size {
        ResponseSize::Small | ResponseSize::Default => 0,
        ResponseSize::Medium => 12,
        ResponseSize::Large => 48,
    };

    if extra_lines > 0 {
        content.push_str("\n\n## Additional Notes\n");
        for i in 0..extra_lines {
            content.push_str(&format!(
                "Note {}: The system SHOULD include clear audit entries for security events.\n",
                i + 1
            ));
        }
    }

    content
}

fn generate_design_response() -> String {
    r#"# Design Document

## Overview

This design describes an authentication service that exposes REST APIs and uses token-based sessions.
The system separates API handling, domain logic, and persistence concerns.

## Architecture

The architecture uses three primary layers with clear ownership:

```mermaid
graph TD
    A[API] --> B[Auth Service]
    B --> C[User Store]
    B --> D[Session Store]
    B --> E[Audit Log]
```

## Components and Interfaces

### API Layer
- Exposes /register, /login, and /logout endpoints
- Validates request payloads and returns typed errors
- Delegates to the Auth Service for all business logic

### Auth Service
- Creates and verifies password hashes
- Issues and validates session tokens
- Enforces lockout and rate limits
- Records audit events for security tracking

### Data Stores
- User store persists account records
- Session store tracks active tokens and expiry
- Audit log records failed and successful logins

## Data Models

- User { id, email, password_hash, created_at }
- Session { token, user_id, expires_at }
- AuditEvent { id, event_type, created_at, metadata }

## Error Handling

- Return 400 for validation errors
- Return 401 for invalid credentials
- Return 429 for rate limits
- Return 500 for unexpected failures
- Log all authentication failures with context

## Testing Strategy

- Unit tests for password hashing and validation
- Integration tests for full register/login/logout flow
- Property tests for token validation edge cases
- Load tests for login throughput and lockout logic
"#
    .to_string()
}

fn generate_tasks_response() -> String {
    r#"# Implementation Plan

## Milestone 1: Project setup

- [ ] 1. Create base module layout
  - Add api, auth, and storage modules
  - Define common error and result types
  - _Requirements: R1, R2_

- [ ] 2. Define core data models
  - Create User, Session, and AuditEvent structs
  - Add serialization and validation helpers
  - _Requirements: R1, R3_

- [ ]* 2.1 Write unit tests for models
  - Validate required fields and parsing
  - _Requirements: R1_

## Milestone 2: Authentication workflows

- [ ] 3. Implement registration flow
  - Add password hashing utility
  - Persist new user records
  - _Requirements: R1_

- [ ] 4. Implement login flow
  - Verify credentials and lockouts
  - Issue session tokens
  - _Requirements: R2_

- [ ] 5. Implement logout flow
  - Revoke session tokens
  - _Requirements: R2_

- [ ]* 5.1 Write integration tests for auth flows
  - Cover register/login/logout happy paths
  - Cover invalid credentials and lockouts
  - _Requirements: R2_

## Milestone 3: Observability and hardening

- [ ] 6. Add audit logging
  - Record failed and successful logins
  - _Requirements: R3_

- [ ] 7. Add rate limiting
  - Enforce request throttling per IP
  - _Requirements: R3_

- [ ]* 7.1 Write performance smoke tests
  - Validate throughput targets
  - _Requirements: R3_
"#
    .to_string()
}

fn generate_review_response() -> String {
    r#"# Review Document

## Review Summary

The specification is mostly complete but needs additional clarity on rate limiting.

**FIXUP PLAN:**

```diff
--- artifacts/00-requirements.md
+++ artifacts/00-requirements.md
@@
-3. WHEN a user fails login 5 times THEN the system SHALL lock the account for 15 minutes
+3. WHEN a user fails login 5 times THEN the system SHALL lock the account for 15 minutes
+4. WHEN a lockout occurs THEN the system SHALL return a clear retry-after hint
```
"#
    .to_string()
}

fn generate_review_with_fixups() -> String {
    r#"# Review Document

## Review Summary

The specification needs small corrections to improve clarity.

**FIXUP PLAN:**

```diff
--- artifacts/10-design.md
+++ artifacts/10-design.md
@@
-Return 429 for rate limits
+Return 429 for rate limits with retry-after information
```
"#
    .to_string()
}

fn generate_fixup_response() -> String {
    r#"# Fixup Report

Applied 1 change from the review plan.
"#
    .to_string()
}

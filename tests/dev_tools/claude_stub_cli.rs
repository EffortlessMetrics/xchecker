//! Integration tests for claude-stub CLI binary
//!
//! These tests execute the compiled claude-stub binary directly using `assert_cmd`.
//! They are gated behind the `dev-tools` feature and only run when that feature is enabled.
//!
//! Run with: `cargo test --features dev-tools --test claude_stub_cli`

use assert_cmd::assert::OutputAssertExt;
use predicates::prelude::*;
use std::process::Command;
use std::process::Stdio;

fn claude_stub_cmd() -> Command {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("claude-stub"));
    cmd.arg("--no-sleep"); // Fast tests
    cmd.stdin(Stdio::null());
    cmd
}

#[test]
fn version_output() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("claude-stub"));
    let version_predicate =
        predicate::str::is_match(r"\b\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?\b").unwrap();
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("claude-stub"))
        .stdout(version_predicate);
}

#[test]
fn success_scenario_text() {
    claude_stub_cmd()
        .args(["--output-format", "text", "--scenario", "success"])
        .assert()
        .success()
        .stdout(predicate::str::contains("# Requirements Document"))
        .stdout(predicate::str::contains("## Introduction"))
        .stdout(predicate::str::contains("**User Story:**"))
        .stdout(predicate::str::contains("#### Acceptance Criteria"));
}

#[test]
fn success_scenario_stream_json() {
    claude_stub_cmd()
        .args(["--output-format", "stream-json", "--scenario", "success"])
        .assert()
        .success()
        .stdout(predicate::str::contains("conversation_start"))
        .stdout(predicate::str::contains("message_start"))
        .stdout(predicate::str::contains("content_block_start"))
        .stdout(predicate::str::contains("content_block_delta"))
        .stdout(predicate::str::contains("message_stop"));
}

#[test]
fn error_scenario() {
    claude_stub_cmd()
        .args(["--scenario", "error"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("Authentication failed"));
}

#[test]
fn malformed_scenario_stream_json() {
    claude_stub_cmd()
        .args(["--output-format", "stream-json", "--scenario", "malformed"])
        .assert()
        .success()
        .stdout(predicate::str::contains("conversation_start"))
        .stdout(predicate::str::contains("msg_123"))
        .stderr(predicate::str::contains("JSON parsing error"));
}

#[test]
fn partial_scenario_stream_json() {
    claude_stub_cmd()
        .args(["--output-format", "stream-json", "--scenario", "partial"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("conversation_start"))
        .stdout(predicate::str::contains("message_start"))
        .stdout(predicate::str::contains("Requirements Document"))
        .stderr(predicate::str::contains("Connection interrupted"));
}

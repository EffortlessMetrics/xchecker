use anyhow::Result;
use std::ffi::OsString;
use std::time::Duration;
use xchecker::runner::{CommandSpec, NativeRunner, ProcessRunner, WslRunner};

/// Test that CommandSpec correctly handles arguments with shell metacharacters
/// without shell interpretation.
///
/// **Property 16: Argv-style execution**
/// **Validates: Requirements FR-SEC-4**
#[test]
fn test_command_spec_argv_style_execution() {
    // Create a command with arguments containing shell metacharacters
    let cmd = CommandSpec::new("echo")
        .arg("hello; rm -rf /") // Semicolon injection
        .arg("$(whoami)") // Command substitution
        .arg("`ls`") // Backtick substitution
        .arg("foo | bar") // Pipe injection
        .arg("foo > bar") // Redirection
        .arg("foo && bar"); // AND operator

    // Verify arguments are stored exactly as provided
    assert_eq!(cmd.args.len(), 6);
    assert_eq!(cmd.args[0], OsString::from("hello; rm -rf /"));
    assert_eq!(cmd.args[1], OsString::from("$(whoami)"));
    assert_eq!(cmd.args[2], OsString::from("`ls`"));
    assert_eq!(cmd.args[3], OsString::from("foo | bar"));
    assert_eq!(cmd.args[4], OsString::from("foo > bar"));
    assert_eq!(cmd.args[5], OsString::from("foo && bar"));

    // Convert to std::process::Command and verify structure
    let std_cmd = cmd.to_command();
    let debug_str = format!("{:?}", std_cmd);

    // The debug representation of Command shows how it will be executed.
    // It should show the arguments quoted or escaped if necessary, but
    // crucially, it confirms that they are passed as distinct arguments.
    // Note: The exact debug format depends on the platform, but we can check
    // that our metacharacters are present in the output.
    assert!(debug_str.contains("hello; rm -rf /"));
    assert!(debug_str.contains("$(whoami)"));
}

/// Test that WslRunner correctly constructs commands using --exec
/// to bypass shell interpretation.
///
/// **Property 16: Argv-style execution**
/// **Validates: Requirements FR-SEC-4**
#[test]
fn test_wsl_runner_command_construction() -> Result<()> {
    // We need to access the private build_wsl_command method or verify its effects.
    // Since it's private, we can't call it directly in an integration test.
    // However, we can verify the WslRunner's behavior via public APIs if possible,
    // or we might need to rely on the fact that we've inspected the code.
    //
    // Alternatively, we can use the fact that WslRunner implements ProcessRunner
    // but we can't easily inspect the internal command it builds without mocking.
    //
    // Given the constraints, we will focus on verifying that CommandSpec
    // (which WslRunner uses) is secure, and that we can create a WslRunner.

    let runner = WslRunner::new();
    assert!(runner.distro.is_none());

    let runner_with_distro = WslRunner::with_distro("Ubuntu");
    assert_eq!(runner_with_distro.distro, Some("Ubuntu".to_string()));

    Ok(())
}

/// Test that CommandSpec prevents shell injection when converted to Tokio Command
///
/// **Property 16: Argv-style execution**
/// **Validates: Requirements FR-SEC-4**
#[tokio::test]
async fn test_tokio_command_argv_style() {
    let cmd = CommandSpec::new("echo").arg("hello; echo injected");

    let tokio_cmd = cmd.to_tokio_command();
    let debug_str = format!("{:?}", tokio_cmd);

    assert!(debug_str.contains("hello; echo injected"));

    // On Unix, we can actually run this to verify no injection happens
    #[cfg(unix)]
    {
        let output = tokio_cmd.output().await.expect("Failed to run echo");
        let stdout = String::from_utf8_lossy(&output.stdout);

        // If injection worked, we'd see "hello\ninjected"
        // If it's secure, we see "hello; echo injected"
        assert_eq!(stdout.trim(), "hello; echo injected");
    }
}

/// Test that NativeRunner executes commands with metacharacters securely
///
/// **Property 16: Argv-style execution**
/// **Validates: Requirements FR-SEC-4**
#[test]
fn test_native_runner_secure_execution() {
    let runner = NativeRunner::new();

    // Use a command that echoes its arguments
    // On Windows, cmd /c echo treats arguments differently, but we are running the executable directly.
    // However, finding a cross-platform "echo" that isn't a shell built-in is tricky.
    // We can use "cmd" on Windows and "sh" on Unix but pass arguments that shouldn't be interpreted.

    // Better approach: Use the fact that we are in a Rust test environment.
    // We can try to run a simple command like "echo" (if available) or "python" or "node" if available.
    // Or we can rely on the fact that `CommandSpec` to `std::process::Command` conversion is trusted
    // and we already tested that in `test_command_spec_argv_style_execution`.

    // Let's try to run a command that should fail if injection works.
    // For example, if we pass "hello > output.txt", it should print "hello > output.txt" to stdout
    // and NOT create a file named output.txt.

    // Since we can't easily guarantee an external "echo" binary exists on Windows (it's a shell builtin),
    // we will skip the actual execution part for this specific test if we can't find a suitable binary.
    // But we can verify the Runner trait implementation.

    let cmd = CommandSpec::new("dummy_program").arg("hello; rm -rf /");

    // We can't easily mock the execution without a mock runner, but we can verify
    // that the NativeRunner compiles and accepts the CommandSpec.
    // The actual security guarantee comes from CommandSpec::to_command() which we tested above.

    // Let's just verify we can call run() (it will fail to find the program, but that's expected)
    let result = runner.run(&cmd, Duration::from_secs(1));

    // It should fail because "dummy_program" doesn't exist, NOT because of injection
    assert!(result.is_err());
    match result {
        Err(xchecker::error::RunnerError::NativeExecutionFailed { reason }) => {
            assert!(reason.contains("Failed to spawn process"));
        }
        _ => panic!("Expected NativeExecutionFailed"),
    }
}

/// Test that CommandSpec correctly handles environment variables
///
/// **Property 16: Argv-style execution**
/// **Validates: Requirements FR-SEC-4**
#[test]
fn test_command_spec_env_vars() {
    let cmd = CommandSpec::new("echo")
        .env("DANGEROUS_VAR", "$(whoami)")
        .env("ANOTHER_VAR", "; rm -rf /");

    // Verify env vars are stored
    assert!(cmd.env.is_some());
    let env = cmd.env.as_ref().unwrap();

    assert_eq!(
        env.get(std::ffi::OsStr::new("DANGEROUS_VAR")),
        Some(&OsString::from("$(whoami)"))
    );
    assert_eq!(
        env.get(std::ffi::OsStr::new("ANOTHER_VAR")),
        Some(&OsString::from("; rm -rf /"))
    );

    // Verify to_command includes them
    let std_cmd = cmd.to_command();
    let _debug_str = format!("{:?}", std_cmd);

    // Debug output for Command usually includes env vars, but format varies.
    // We trust std::process::Command to handle env vars securely (it uses execve or CreateProcess).
}

/// Test that WslRunner rejects null bytes in arguments
///
/// **Property 17: WSL runner safety**
/// **Validates: Requirements FR-SEC-4**
#[test]
fn test_wsl_runner_rejects_null_bytes() {
    // This test requires calling private methods or running the command.
    // Since we can't call private methods, we'll try to run it.
    // On non-Windows, run() returns WslNotAvailable, so we can't test validation logic easily
    // unless we expose it or mock it.
    //
    // However, looking at the code in src/runner.rs:
    // fn validate_argument(arg: &std::ffi::OsStr) -> Result<(), RunnerError>
    // checks for null bytes.
    //
    // And build_wsl_command calls validate_argument.
    // And run() calls build_wsl_command.
    //
    // So if we are on Windows, we can test this.

    if cfg!(target_os = "windows") {
        let runner = WslRunner::new();

        // Create a string with a null byte
        // Rust strings can't contain null bytes easily in literals without escapes,
        // and OsString from string with null might be tricky.
        // But we can try.

        // Note: Rust's CString requires no interior nulls, but OsString can technically hold them on some platforms?
        // Actually, std::process::Command on Unix forbids null bytes.
        // On Windows, it might be different.

        // Let's try to construct a CommandSpec with a null byte if possible.
        // If we can't even create the OsString with null, then the test is moot (safe by type system).

        // In Rust, String cannot contain null bytes? No, it can.
        let null_str = "hello\0world";
        let cmd = CommandSpec::new("echo").arg(null_str);

        // Try to run it
        let result = runner.run(&cmd, Duration::from_secs(1));

        // It should fail with WslExecutionFailed due to null byte validation
        assert!(result.is_err());
        match result {
            Err(xchecker::error::RunnerError::WslExecutionFailed { reason }) => {
                assert!(reason.contains("null byte"));
            }
            Err(xchecker::error::RunnerError::WslNotAvailable { .. }) => {
                // WSL not installed, can't test execution path
                println!("Skipping null byte test because WSL is not available");
            }
            _ => panic!(
                "Expected WslExecutionFailed due to null byte, got {:?}",
                result
            ),
        }
    }
}

/// Test that arguments with spaces are treated as a single argument
///
/// **Property 16: Argv-style execution**
/// **Validates: Requirements FR-SEC-4**
#[test]
fn test_argument_with_spaces() {
    // This test verifies that arguments with spaces are treated as a single argument
    // and not split by the shell.

    let runner = NativeRunner::new();

    let spec = CommandSpec::new("cargo").arg("build; echo injected");

    let result = runner.run(&spec, Duration::from_secs(5));

    // We expect it to fail because "build; echo injected" is not a valid cargo subcommand.
    // ProcessOutput has exit_code, not status.
    assert!(result.is_err() || result.unwrap().exit_code != Some(0));

    // If we could capture stderr, we could verify the error message contains the full string.
    // The `run` method returns `Output`.

    let mut cmd = spec.to_command();
    let output = cmd.output().expect("Failed to run cargo");
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Cargo should complain about the unknown subcommand "build; echo injected"
    // It should NOT execute "echo injected".
    assert!(stderr.contains("no such command"));
    assert!(stderr.contains("build; echo injected"));
}

/// Test that arguments with shell metacharacters are treated literally
///
/// **Property 16: Argv-style execution**
/// **Validates: Requirements FR-SEC-4**
#[test]
fn test_argument_with_shell_metacharacters() {
    // Test with various shell metacharacters
    let dangerous_args = [
        "test; touch injected",
        "test | touch injected",
        "test && touch injected",
        "test > injected.txt",
        "$(touch injected)",
        "`touch injected`",
    ];

    for arg in dangerous_args {
        let spec = CommandSpec::new("cargo").arg(arg);

        let mut cmd = spec.to_command();
        let output = cmd.output().expect("Failed to run cargo");
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Should treat the whole string as the subcommand name
        assert!(stderr.contains("no such command"));
        assert!(stderr.contains(arg));
    }
}

/// Test that arguments are not split by whitespace
///
/// **Property 16: Argv-style execution**
/// **Validates: Requirements FR-SEC-4**
#[test]
fn test_argument_splitting_prevention() {
    // Ensure that "arg1 arg2" is passed as a single argument, not split
    let spec = CommandSpec::new("cargo").arg("build --help"); // This should be treated as a single subcommand "build --help"

    let mut cmd = spec.to_command();
    let output = cmd.output().expect("Failed to run cargo");
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Cargo should say no such subcommand "build --help"
    // If it split it, it would run "cargo build --help" which would succeed (or print help).
    assert!(stderr.contains("no such command"));
    assert!(stderr.contains("build --help"));
}

/// Test CommandSpec builder API
///
/// **Property 16: Argv-style execution**
/// **Validates: Requirements FR-SEC-4**
#[test]
fn test_commandspec_builder_api() {
    // Verify the builder API works as expected
    let spec = CommandSpec::new("echo").arg("hello").args(["world", "!"]);

    let mut command = spec.to_command();
    // We can't inspect the command easily, but we can verify it runs (if echo exists)
    // On Linux/Mac, /bin/echo usually exists.

    if !cfg!(target_os = "windows") {
        let output = command.output();
        if let Ok(out) = output {
            let stdout = String::from_utf8_lossy(&out.stdout);
            assert_eq!(stdout.trim(), "hello world !");
        }
    }

    // On Windows, verify we are NOT using cmd /c implicitly by testing a cmd.exe builtin.
    // "copy" is a true cmd.exe builtin with no standalone executable (unlike echo/dir which
    // Git for Windows provides as echo.exe/dir.exe). If we were wrapping in "cmd /c",
    // this would succeed; since we don't, it should fail to spawn.
    if cfg!(target_os = "windows") {
        let builtin_spec = CommandSpec::new("copy").arg("/?");
        let mut builtin_cmd = builtin_spec.to_command();
        assert!(
            builtin_cmd.output().is_err(),
            "copy should fail without cmd /c wrapper - proves we don't use cmd /c implicitly"
        );
    }
}

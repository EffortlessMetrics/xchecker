//! Integration tests for WSL Runner functionality
//!
//! These tests verify WSL execution, path translation, environment translation,
//! and `runner_distro` capture. Most tests are Windows-only since WSL is a Windows feature.

#[cfg(test)]
mod wsl_runner_tests {
    #[cfg(target_os = "windows")]
    use serial_test::serial;
    #[allow(unused_imports)]
    use std::time::Duration;
    use xchecker::runner::{Runner, RunnerMode, WslOptions};
    #[allow(unused_imports)]
    use xchecker::wsl::{
        is_wsl_available, translate_env_for_wsl, translate_win_to_wsl, validate_claude_in_wsl,
    };

    /// Restores an env var on drop to keep tests isolated when mutating process env.
    #[cfg(target_os = "windows")]
    struct EnvVarGuard {
        key: String,
        original: Option<String>,
    }

    #[cfg(target_os = "windows")]
    impl EnvVarGuard {
        fn set(key: &str, value: &str) -> Self {
            let original = std::env::var(key).ok();
            // SAFETY: Tests serialize access and restore the prior value.
            unsafe {
                std::env::set_var(key, value);
            }

            Self {
                key: key.to_string(),
                original,
            }
        }

        fn cleared(key: &str) -> Self {
            let original = std::env::var(key).ok();
            // SAFETY: Tests serialize access and restore the prior value.
            unsafe {
                std::env::remove_var(key);
            }

            Self {
                key: key.to_string(),
                original,
            }
        }
    }

    #[cfg(target_os = "windows")]
    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            match &self.original {
                Some(value) => unsafe { std::env::set_var(&self.key, value) },
                None => unsafe { std::env::remove_var(&self.key) },
            }
        }
    }

    /// Test that WSL runner can be created with default options
    #[test]
    fn test_wsl_runner_creation() {
        let runner = Runner::new(RunnerMode::Wsl, WslOptions::default());
        assert_eq!(runner.mode, RunnerMode::Wsl);
        assert!(runner.wsl_options.distro.is_none());
        assert!(runner.wsl_options.claude_path.is_none());
    }

    /// Test that WSL runner can be created with specific distro
    #[test]
    fn test_wsl_runner_creation_with_distro() {
        let wsl_options = WslOptions {
            distro: Some("Ubuntu-22.04".to_string()),
            claude_path: None,
        };
        let runner = Runner::new(RunnerMode::Wsl, wsl_options);
        assert_eq!(runner.mode, RunnerMode::Wsl);
        assert_eq!(runner.wsl_options.distro, Some("Ubuntu-22.04".to_string()));
    }

    /// Test that WSL runner can be created with custom claude path
    #[test]
    fn test_wsl_runner_creation_with_claude_path() {
        let wsl_options = WslOptions {
            distro: None,
            claude_path: Some("/usr/local/bin/claude".to_string()),
        };
        let runner = Runner::new(RunnerMode::Wsl, wsl_options);
        assert_eq!(runner.mode, RunnerMode::Wsl);
        assert_eq!(
            runner.wsl_options.claude_path,
            Some("/usr/local/bin/claude".to_string())
        );
    }

    /// Test that WSL runner description includes distro and claude path
    #[test]
    fn test_wsl_runner_description() {
        let wsl_options = WslOptions {
            distro: Some("Ubuntu-22.04".to_string()),
            claude_path: Some("/usr/local/bin/claude".to_string()),
        };
        let runner = Runner::new(RunnerMode::Wsl, wsl_options);
        let description = runner.description();

        assert!(description.contains("WSL execution"));
        assert!(description.contains("Ubuntu-22.04"));
        assert!(description.contains("/usr/local/bin/claude"));
    }

    /// Test that WSL runner validation fails on non-Windows platforms
    #[test]
    #[cfg(not(target_os = "windows"))]
    fn test_wsl_runner_validation_fails_on_non_windows() {
        let runner = Runner::new(RunnerMode::Wsl, WslOptions::default());
        let result = runner.validate();

        assert!(result.is_err());
        match result {
            Err(e) => {
                let error_str = format!("{:?}", e);
                assert!(
                    error_str.contains("only supported on Windows")
                        || error_str.contains("ConfigurationInvalid")
                );
            }
            Ok(_) => panic!("Expected validation to fail on non-Windows"),
        }
    }

    /// Test that `get_wsl_distro_name` returns configured distro
    #[test]
    fn test_get_wsl_distro_name_from_config() {
        let wsl_options = WslOptions {
            distro: Some("Ubuntu-22.04".to_string()),
            claude_path: None,
        };
        let runner = Runner::new(RunnerMode::Wsl, wsl_options);

        let distro = runner.get_wsl_distro_name();
        assert_eq!(distro, Some("Ubuntu-22.04".to_string()));
    }

    /// Test that `get_wsl_distro_name` falls back to environment variable
    #[test]
    #[cfg(target_os = "windows")]
    #[serial]
    fn test_get_wsl_distro_name_from_env() {
        let _env_guard = EnvVarGuard::set("WSL_DISTRO_NAME", "TestDistro");

        let runner = Runner::new(RunnerMode::Wsl, WslOptions::default());
        let distro = runner.get_wsl_distro_name();

        assert_eq!(distro.as_deref(), Some("TestDistro"));
    }

    /// Test that `get_wsl_distro_name` falls back to wsl -l -q
    #[test]
    #[cfg(target_os = "windows")]
    #[serial]
    fn test_get_wsl_distro_name_from_wsl_list() {
        let _env_guard = EnvVarGuard::cleared("WSL_DISTRO_NAME");

        let runner = Runner::new(RunnerMode::Wsl, WslOptions::default());
        let distro = runner.get_wsl_distro_name();

        // If WSL is available, should get a distro name
        if is_wsl_available().unwrap_or(false) {
            assert!(distro.is_some(), "Expected distro name from wsl -l -q");
        }
    }

    // Integration tests for WSL execution (Windows only)

    /// Test WSL execution with a simple command
    #[tokio::test]
    #[cfg(target_os = "windows")]
    async fn test_wsl_execution_simple_command() {
        // Skip if WSL is not available
        if !is_wsl_available().unwrap_or(false) {
            eprintln!("Skipping test: WSL not available");
            return;
        }

        let runner = Runner::new(RunnerMode::Wsl, WslOptions::default());

        // Execute a simple echo command via WSL
        // Note: We can't test Claude directly without it being installed,
        // so we test the WSL execution mechanism with a known command
        let args = vec!["echo".to_string(), "Hello from WSL".to_string()];

        // This will fail because we're trying to run echo as if it were claude,
        // but it tests the WSL invocation mechanism
        let result = runner
            .execute_claude(&args, "", Some(Duration::from_secs(5)))
            .await;

        // We expect this to fail since echo is not claude, but we're testing
        // that the WSL execution path is exercised
        // The important thing is that it doesn't panic and uses the WSL code path
        match result {
            Ok(_) => {
                // Unexpected success, but that's okay for this test
                println!("WSL execution succeeded (unexpected but acceptable)");
            }
            Err(e) => {
                // Expected failure - we're not actually running claude
                println!("WSL execution failed as expected: {e:?}");
            }
        }
    }

    /// Test that WSL execution captures `runner_distro`
    #[tokio::test]
    #[cfg(target_os = "windows")]
    async fn test_wsl_execution_captures_runner_distro() {
        // Skip if WSL is not available
        if !is_wsl_available().unwrap_or(false) {
            eprintln!("Skipping test: WSL not available");
            return;
        }

        // Skip if Claude is not available in WSL
        if !validate_claude_in_wsl(None).unwrap_or(false) {
            eprintln!("Skipping test: Claude not available in WSL");
            return;
        }

        let runner = Runner::new(RunnerMode::Wsl, WslOptions::default());

        // Try to execute claude --version
        let args = vec!["--version".to_string()];
        let result = runner
            .execute_claude(&args, "", Some(Duration::from_secs(10)))
            .await;

        match result {
            Ok(response) => {
                // Verify that runner_used is WSL
                assert_eq!(response.runner_used, RunnerMode::Wsl);

                // Verify that runner_distro is captured
                assert!(
                    response.runner_distro.is_some(),
                    "Expected runner_distro to be captured"
                );

                println!("Runner distro: {:?}", response.runner_distro);
            }
            Err(e) => {
                eprintln!("Claude execution failed: {e:?}");
                // This is acceptable if Claude is not installed in WSL
            }
        }
    }

    /// Test that WSL execution with specific distro uses that distro
    #[tokio::test]
    #[cfg(target_os = "windows")]
    async fn test_wsl_execution_with_specific_distro() {
        // Skip if WSL is not available
        if !is_wsl_available().unwrap_or(false) {
            eprintln!("Skipping test: WSL not available");
            return;
        }

        // Get available distros
        let distros = match xchecker::runner::CommandSpec::new("wsl")
            .args(["-l", "-q"])
            .to_command()
            .output()
        {
            Ok(output) if output.status.success() => {
                xchecker::wsl::parse_distro_list(&output.stdout).unwrap_or_default()
            }
            _ => {
                eprintln!("Skipping test: Could not get distro list");
                return;
            }
        };

        if distros.is_empty() {
            eprintln!("Skipping test: No WSL distros available");
            return;
        }

        let test_distro = &distros[0];

        let wsl_options = WslOptions {
            distro: Some(test_distro.clone()),
            claude_path: None,
        };
        let runner = Runner::new(RunnerMode::Wsl, wsl_options);

        // Verify that get_wsl_distro_name returns the configured distro
        let distro_name = runner.get_wsl_distro_name();
        assert_eq!(distro_name, Some(test_distro.clone()));
    }

    // Unit tests for path translation in invocation

    /// Test that Windows paths are translated for WSL invocation
    #[test]
    #[cfg(target_os = "windows")]
    fn test_path_translation_for_wsl_invocation() {
        let windows_path = std::path::Path::new("C:\\Users\\test\\file.txt");
        let wsl_path = translate_win_to_wsl(windows_path).unwrap();

        // Should be translated to WSL format
        assert!(
            wsl_path.starts_with("/mnt/c"),
            "Expected path to start with /mnt/c, got: {wsl_path}"
        );
        assert!(wsl_path.contains("Users"));
        assert!(wsl_path.contains("test"));
        assert!(wsl_path.contains("file.txt"));
    }

    /// Test that multiple Windows paths are translated correctly
    #[test]
    #[cfg(target_os = "windows")]
    fn test_multiple_path_translation_for_wsl() {
        let paths = vec![
            "C:\\Windows\\System32",
            "D:\\Projects\\xchecker",
            "E:\\Data\\files",
        ];

        for path_str in paths {
            let path = std::path::Path::new(path_str);
            let wsl_path = translate_win_to_wsl(path).unwrap();

            // Should be translated to WSL format
            assert!(
                wsl_path.starts_with("/mnt/"),
                "Expected path to start with /mnt/, got: {wsl_path}"
            );
        }
    }

    /// Test that UNC paths are translated for WSL invocation
    #[test]
    #[cfg(target_os = "windows")]
    fn test_unc_path_translation_for_wsl() {
        let windows_path = std::path::Path::new("\\\\server\\share\\path\\file.txt");
        let wsl_path = translate_win_to_wsl(windows_path).unwrap();

        // Should be translated to WSL format
        assert!(
            wsl_path.starts_with("/mnt/"),
            "Expected UNC path to start with /mnt/, got: {wsl_path}"
        );
        assert!(wsl_path.contains("server"));
        assert!(wsl_path.contains("share"));
    }

    // Unit tests for environment variable translation in invocation

    /// Test that PATH environment variable is translated for WSL
    #[test]
    #[cfg(target_os = "windows")]
    fn test_path_env_translation_for_wsl() {
        let env = vec![(
            "PATH".to_string(),
            "C:\\Windows\\System32;C:\\Program Files".to_string(),
        )];

        let translated = translate_env_for_wsl(&env);

        assert_eq!(translated.len(), 1);
        assert_eq!(translated[0].0, "PATH");

        // PATH should use colon separator and contain /mnt/c
        let path_value = &translated[0].1;
        assert!(path_value.contains(':'), "PATH should use colon separator");
        assert!(path_value.contains("/mnt/c"), "PATH should contain /mnt/c");
    }

    /// Test that TEMP environment variable is translated for WSL
    #[test]
    #[cfg(target_os = "windows")]
    fn test_temp_env_translation_for_wsl() {
        let env = vec![(
            "TEMP".to_string(),
            "C:\\Users\\test\\AppData\\Local\\Temp".to_string(),
        )];

        let translated = translate_env_for_wsl(&env);

        assert_eq!(translated.len(), 1);
        assert_eq!(translated[0].0, "TEMP");

        // TEMP should be translated to WSL format
        let temp_value = &translated[0].1;
        assert!(
            temp_value.starts_with("/mnt/c"),
            "TEMP should start with /mnt/c, got: {temp_value}"
        );
    }

    /// Test that non-path environment variables pass through unchanged
    #[test]
    fn test_non_path_env_passthrough_for_wsl() {
        let env = vec![
            ("USER".to_string(), "testuser".to_string()),
            ("LANG".to_string(), "en_US.UTF-8".to_string()),
            ("TERM".to_string(), "xterm-256color".to_string()),
        ];

        let translated = translate_env_for_wsl(&env);

        // Non-path variables should pass through unchanged
        assert_eq!(translated.len(), 3);
        assert_eq!(translated[0], ("USER".to_string(), "testuser".to_string()));
        assert_eq!(
            translated[1],
            ("LANG".to_string(), "en_US.UTF-8".to_string())
        );
        assert_eq!(
            translated[2],
            ("TERM".to_string(), "xterm-256color".to_string())
        );
    }

    /// Test that mixed environment variables are handled correctly
    #[test]
    #[cfg(target_os = "windows")]
    fn test_mixed_env_translation_for_wsl() {
        let env = vec![
            (
                "PATH".to_string(),
                "C:\\Windows;C:\\Program Files".to_string(),
            ),
            ("USER".to_string(), "testuser".to_string()),
            ("TEMP".to_string(), "C:\\Temp".to_string()),
            ("LANG".to_string(), "en_US.UTF-8".to_string()),
        ];

        let translated = translate_env_for_wsl(&env);

        assert_eq!(translated.len(), 4);

        // PATH should be translated
        let path_entry = translated.iter().find(|(k, _)| k == "PATH").unwrap();
        assert!(path_entry.1.contains("/mnt/c"));

        // USER should pass through
        let user_entry = translated.iter().find(|(k, _)| k == "USER").unwrap();
        assert_eq!(user_entry.1, "testuser");

        // TEMP should be translated
        let temp_entry = translated.iter().find(|(k, _)| k == "TEMP").unwrap();
        assert!(temp_entry.1.starts_with("/mnt/c"));

        // LANG should pass through
        let lang_entry = translated.iter().find(|(k, _)| k == "LANG").unwrap();
        assert_eq!(lang_entry.1, "en_US.UTF-8");
    }

    // Test artifact persistence (artifacts are persisted in Windows spec root by orchestrator)

    /// Test that artifacts are persisted in Windows spec root
    /// Note: This is handled by the orchestrator, not the runner itself.
    /// The runner just executes in WSL, but artifacts are written to the Windows filesystem.
    #[test]
    fn test_artifact_persistence_concept() {
        // This is a conceptual test to document that artifact persistence
        // happens in the Windows spec root, not in the WSL filesystem.
        // The orchestrator handles writing artifacts after the runner completes.

        // The runner's job is to:
        // 1. Execute Claude in WSL
        // 2. Capture stdout/stderr
        // 3. Return the response to the orchestrator

        // The orchestrator's job is to:
        // 1. Write artifacts to .xchecker/specs/<spec-id>/artifacts/
        // 2. This directory is on the Windows filesystem
        // 3. WSL can access it via /mnt/c/... paths

        // No actual test needed here - this is architectural documentation
        // Artifact persistence is handled by orchestrator in Windows spec root
    }

    // Test runner_distro in receipts

    /// Test that `runner_distro` is included in `ClaudeResponse`
    #[test]
    fn test_runner_distro_in_response() {
        // Create a mock response with runner_distro
        use xchecker::runner::{ClaudeResponse, NdjsonResult};

        let response = ClaudeResponse {
            stdout: String::new(),
            stderr: String::new(),
            exit_code: 0,
            runner_used: RunnerMode::Wsl,
            runner_distro: Some("Ubuntu-22.04".to_string()),
            timed_out: false,
            ndjson_result: NdjsonResult::NoValidJson {
                tail_excerpt: String::new(),
            },
            stdout_truncated: false,
            stderr_truncated: false,
            stdout_total_bytes: 0,
            stderr_total_bytes: 0,
        };

        // Verify that runner_distro is captured
        assert_eq!(response.runner_used, RunnerMode::Wsl);
        assert_eq!(response.runner_distro, Some("Ubuntu-22.04".to_string()));
    }

    /// Test that `runner_distro` is None for native execution
    #[test]
    fn test_runner_distro_none_for_native() {
        use xchecker::runner::{ClaudeResponse, NdjsonResult};

        let response = ClaudeResponse {
            stdout: String::new(),
            stderr: String::new(),
            exit_code: 0,
            runner_used: RunnerMode::Native,
            runner_distro: None,
            timed_out: false,
            ndjson_result: NdjsonResult::NoValidJson {
                tail_excerpt: String::new(),
            },
            stdout_truncated: false,
            stderr_truncated: false,
            stdout_total_bytes: 0,
            stderr_total_bytes: 0,
        };

        // Verify that runner_distro is None for native execution
        assert_eq!(response.runner_used, RunnerMode::Native);
        assert_eq!(response.runner_distro, None);
    }

    // Integration test for wsl.exe --exec with discrete argv

    /// Test that wsl.exe --exec is used with discrete argv elements
    /// This is a conceptual test since we can't easily mock the process execution
    #[test]
    fn test_wsl_exec_with_discrete_argv_concept() {
        // The execute_wsl method in runner.rs uses:
        // let mut wsl_args = vec!["--exec".to_string(), claude_path.to_string()];
        // wsl_args.extend(args.iter().cloned());
        // cmd.args(&wsl_args)

        // This ensures that arguments are passed as discrete argv elements,
        // not concatenated into a shell command string.

        // Example: wsl.exe --exec claude --version
        // NOT: wsl.exe -c "claude --version"

        // This prevents shell injection and quoting issues.
        // wsl.exe --exec is used with discrete argv elements
    }

    // Test that WSL execution handles timeout correctly

    /// Test that WSL execution respects timeout
    #[tokio::test]
    #[cfg(target_os = "windows")]
    async fn test_wsl_execution_timeout() {
        // Skip if WSL is not available
        if !is_wsl_available().unwrap_or(false) {
            eprintln!("Skipping test: WSL not available");
            return;
        }

        let runner = Runner::new(RunnerMode::Wsl, WslOptions::default());

        // Try to execute a command that would timeout
        // We use a sleep command to simulate a long-running process
        let args = vec!["sleep".to_string(), "30".to_string()];
        let result = runner
            .execute_claude(&args, "", Some(Duration::from_secs(1)))
            .await;

        // Should timeout
        match result {
            Ok(_) => {
                // Unexpected success
                println!("Command completed before timeout (unexpected)");
            }
            Err(e) => {
                // Should be a timeout error
                let error_str = format!("{e:?}");
                println!("Got error as expected: {error_str}");
                // We expect either a timeout error or execution failure
                assert!(
                    error_str.contains("Timeout")
                        || error_str.contains("timeout")
                        || error_str.contains("WslExecutionFailed"),
                    "Expected timeout or execution error, got: {error_str}"
                );
            }
        }
    }

    // Test that WSL execution handles stdin correctly

    /// Test that WSL execution pipes stdin correctly
    #[tokio::test]
    #[cfg(target_os = "windows")]
    async fn test_wsl_execution_stdin() {
        // Skip if WSL is not available
        if !is_wsl_available().unwrap_or(false) {
            eprintln!("Skipping test: WSL not available");
            return;
        }

        let runner = Runner::new(RunnerMode::Wsl, WslOptions::default());

        // Try to execute a command that reads from stdin
        // We use cat to echo stdin to stdout
        let args = vec!["cat".to_string()];
        let stdin_content = "Hello from stdin";
        let result = runner
            .execute_claude(&args, stdin_content, Some(Duration::from_secs(5)))
            .await;

        match result {
            Ok(response) => {
                // Should have echoed stdin to stdout
                println!("Stdout: {}", response.stdout);
                // Note: This might not work if cat is not available or if we're trying to run it as claude
            }
            Err(e) => {
                // Expected failure since we're not actually running claude
                println!("Execution failed as expected: {e:?}");
            }
        }
    }
}

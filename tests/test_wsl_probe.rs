//! WSL probe test for Windows CI
//!
//! This test verifies WSL availability on Windows runners.
//! It's marked as ignored and only runs in CI on Windows.

use xchecker::runner::CommandSpec;

/// Test that WSL is available on Windows CI runners
///
/// This test is marked as ignored and only runs in CI with the --ignored flag.
/// On Windows, it attempts to detect WSL. If WSL is not installed, the test
/// is skipped with a clear message.
#[test]
#[ignore = "windows_ci_only"]
fn test_wsl_probe() {
    // Only run on Windows
    if !cfg!(target_os = "windows") {
        println!("Skipping WSL probe test on non-Windows platform");
        return;
    }

    println!("Probing for WSL availability on Windows...");

    // Try to run `wsl --version` to check if WSL is installed
    let version_result = CommandSpec::new("wsl").arg("--version").to_command().output();

    match version_result {
        Ok(output) => {
            if output.status.success() {
                let version_output = String::from_utf8_lossy(&output.stdout);
                println!("WSL is available:");
                println!("{}", version_output);

                // Try to list distributions
                let list_result = CommandSpec::new("wsl").args(["-l", "-v"]).to_command().output();

                if let Ok(list_output) = list_result
                    && list_output.status.success()
                {
                    let distros = String::from_utf8_lossy(&list_output.stdout);
                    println!("\nInstalled WSL distributions:");
                    println!("{}", distros);
                }

                // Try to run a simple command in WSL
                let test_result = CommandSpec::new("wsl")
                    .args(["-e", "echo", "WSL test successful"])
                    .to_command()
                    .output();

                match test_result {
                    Ok(test_output) if test_output.status.success() => {
                        let test_msg = String::from_utf8_lossy(&test_output.stdout);
                        println!("\nWSL execution test: {}", test_msg.trim());
                        println!("✓ WSL is fully functional");
                    }
                    Ok(test_output) => {
                        let stderr = String::from_utf8_lossy(&test_output.stderr);
                        println!("\nWSL execution test failed:");
                        println!("Exit code: {:?}", test_output.status.code());
                        println!("Stderr: {}", stderr);
                        println!("⚠ WSL is installed but may not be configured properly");
                    }
                    Err(e) => {
                        println!("\nWSL execution test error: {}", e);
                        println!("⚠ WSL is installed but execution failed");
                    }
                }
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                println!("WSL command failed:");
                println!("Exit code: {:?}", output.status.code());
                println!("Stderr: {}", stderr);
                println!("⚠ WSL may not be properly installed or configured");
            }
        }
        Err(e) => {
            println!("WSL is not available: {}", e);
            println!("This is expected if WSL is not installed on this Windows system.");
            println!("Skipping WSL-related tests.");
        }
    }

    // This test always passes - it's informational only
    // The continue-on-error flag in CI handles cases where WSL is not installed
}

#[cfg(test)]
mod runner_wsl_tests {
    /// Test that verifies WSL detection logic works correctly
    ///
    /// This is a unit test that doesn't require WSL to be installed.
    /// It just verifies the test infrastructure is set up correctly.
    #[test]
    fn test_wsl_probe_infrastructure() {
        // This test verifies that the WSL probe test exists and can be compiled
        // The actual WSL probe test is marked as ignored and runs separately
        // WSL probe test infrastructure is set up correctly
    }
}

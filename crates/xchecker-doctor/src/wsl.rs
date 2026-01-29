//! WSL (Windows Subsystem for Linux) detection and utilities
//!
//! Provides functions for detecting WSL availability, listing distributions,
//! and validating Claude CLI availability within WSL environments.

use xchecker_utils::error::RunnerError;
use xchecker_utils::runner::CommandSpec;

/// Check if WSL is available on the system
///
/// On Windows, this checks if `wsl.exe -l -q` succeeds and returns at least one distribution.
/// On non-Windows platforms, this always returns false.
///
/// # Returns
/// * `Ok(true)` - WSL is available with at least one installed distribution
/// * `Ok(false)` - WSL is not available or no distributions are installed
/// * `Err(RunnerError)` - An error occurred while checking WSL availability
///
/// # Examples
/// ```no_run
/// use xchecker_doctor::wsl::is_wsl_available;
///
/// match is_wsl_available() {
///     Ok(true) => println!("WSL is available"),
///     Ok(false) => println!("WSL is not available"),
///     Err(e) => eprintln!("Error checking WSL: {}", e),
/// }
/// ```
pub fn is_wsl_available() -> Result<bool, RunnerError> {
    // WSL is only available on Windows
    if !cfg!(target_os = "windows") {
        return Ok(false);
    }

    // Try to execute `wsl.exe -l -q` to list distributions using CommandSpec
    match CommandSpec::new("wsl")
        .args(["-l", "-q"])
        .to_command()
        .output()
    {
        Ok(output) => {
            if !output.status.success() {
                return Ok(false);
            }

            // Parse the output to check if at least one distribution is installed
            let distros = parse_distro_list(&output.stdout)?;
            Ok(!distros.is_empty())
        }
        Err(_) => {
            // wsl.exe not found or failed to execute
            Ok(false)
        }
    }
}

/// Parse the output of `wsl -l -q` to extract distribution names
///
/// The output may be UTF-16LE on some Windows locales, so we need to handle both
/// UTF-8 and UTF-16LE encodings.
///
/// # Arguments
/// * `raw` - Raw bytes from `wsl -l -q` stdout
///
/// # Returns
/// * `Ok(Vec<String>)` - List of distribution names
/// * `Err(RunnerError)` - An error occurred while parsing the output
///
/// # Examples
/// ```no_run
/// use xchecker_doctor::wsl::parse_distro_list;
///
/// let output = b"Ubuntu-22.04\nDebian\n";
/// let distros = parse_distro_list(output).unwrap();
/// assert_eq!(distros, vec!["Ubuntu-22.04", "Debian"]);
/// ```
pub fn parse_distro_list(raw: &[u8]) -> Result<Vec<String>, RunnerError> {
    // Normalize the output (may be UTF-16LE on some Windows locales)
    let text = normalize_wsl_output(raw);

    // Parse lines and filter out empty lines
    let distros: Vec<String> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(std::string::ToString::to_string)
        .collect();

    Ok(distros)
}

/// Validate that Claude CLI is available in WSL
///
/// This function checks if Claude CLI is discoverable within a WSL distribution
/// by executing `wsl.exe -d <distro> -- which claude`.
///
/// On Windows, this validates Claude availability in the specified (or default) WSL distribution.
/// On non-Windows platforms, this always returns false since WSL is not available.
///
/// # Arguments
/// * `distro` - Optional specific WSL distribution name (e.g., "Ubuntu-22.04").
///   If None, uses the default WSL distribution.
///
/// # Returns
/// * `Ok(true)` - Claude CLI is discoverable in WSL
/// * `Ok(false)` - Claude CLI is not found in WSL
/// * `Err(RunnerError)` - An error occurred while checking Claude availability
///
/// # Examples
/// ```no_run
/// use xchecker_doctor::wsl::validate_claude_in_wsl;
///
/// // Check Claude in default WSL distribution
/// match validate_claude_in_wsl(None) {
///     Ok(true) => println!("Claude is available in WSL"),
///     Ok(false) => println!("Claude is not found in WSL"),
///     Err(e) => eprintln!("Error checking Claude: {}", e),
/// }
///
/// // Check Claude in specific distribution
/// match validate_claude_in_wsl(Some("Ubuntu-22.04")) {
///     Ok(true) => println!("Claude is available in Ubuntu-22.04"),
///     Ok(false) => println!("Claude is not found in Ubuntu-22.04"),
///     Err(e) => eprintln!("Error checking Claude: {}", e),
/// }
/// ```
pub fn validate_claude_in_wsl(distro: Option<&str>) -> Result<bool, RunnerError> {
    // WSL is only available on Windows
    if !cfg!(target_os = "windows") {
        return Ok(false);
    }

    // Build the command: wsl.exe [-d <distro>] -- which claude using CommandSpec
    let mut cmd = CommandSpec::new("wsl");

    // Add distro specification if provided
    if let Some(distro_name) = distro {
        cmd = cmd.args(["-d", distro_name]);
    }

    // Add the command to check for Claude
    cmd = cmd.args(["--", "which", "claude"]);

    // Execute the command
    match cmd.to_command().output() {
        Ok(output) => {
            // If the command succeeds and returns a path, Claude is available
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let path = stdout.trim();

                // Verify we got a non-empty path
                if path.is_empty() { Ok(false) } else { Ok(true) }
            } else {
                // Command failed - Claude not found
                Ok(false)
            }
        }
        Err(e) => {
            // Failed to execute wsl.exe - WSL might not be available
            Err(RunnerError::WslNotAvailable {
                reason: format!("Failed to execute wsl.exe: {e}"),
            })
        }
    }
}

/// Normalize WSL output which may be UTF-16LE on some Windows locales
///
/// This function detects if the input is UTF-16LE encoded and converts it to UTF-8.
/// If the input is already UTF-8, it returns it as-is.
///
/// # Arguments
/// * `raw` - Raw bytes from WSL command output
///
/// # Returns
/// * `String` - Normalized UTF-8 string
fn normalize_wsl_output(raw: &[u8]) -> String {
    // Check if this looks like UTF-16LE (every other byte is 0x00 for ASCII)
    let looks_like_utf16le = raw.len() >= 4
        && raw.len().is_multiple_of(2)
        && raw
            .iter()
            .skip(1)
            .step_by(2)
            .take(10)
            .filter(|&&b| b == 0x00)
            .count()
            >= 5;

    if looks_like_utf16le {
        // Decode as UTF-16LE
        let mut u16_vec: Vec<u16> = Vec::new();
        let mut i = 0;
        while i + 1 < raw.len() {
            let u = u16::from_le_bytes([raw[i], raw[i + 1]]);
            u16_vec.push(u);
            i += 2;
        }
        return String::from_utf16_lossy(&u16_vec);
    }

    // Try UTF-8
    String::from_utf8_lossy(raw).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_wsl_available_on_non_windows() {
        if !cfg!(target_os = "windows") {
            let result = is_wsl_available();
            assert!(result.is_ok());
            assert!(!result.unwrap());
        }
    }

    #[test]
    fn test_parse_distro_list_utf8() {
        let output = b"Ubuntu-22.04\nDebian\nkali-linux\n";
        let distros = parse_distro_list(output).unwrap();
        assert_eq!(distros.len(), 3);
        assert_eq!(distros[0], "Ubuntu-22.04");
        assert_eq!(distros[1], "Debian");
        assert_eq!(distros[2], "kali-linux");
    }

    #[test]
    fn test_parse_distro_list_empty() {
        let output = b"";
        let distros = parse_distro_list(output).unwrap();
        assert_eq!(distros.len(), 0);
    }

    #[test]
    fn test_parse_distro_list_whitespace_only() {
        let output = b"   \n\n  \t  \n";
        let distros = parse_distro_list(output).unwrap();
        assert_eq!(distros.len(), 0);
    }

    #[test]
    fn test_parse_distro_list_with_extra_whitespace() {
        let output = b"  Ubuntu-22.04  \n  Debian  \n";
        let distros = parse_distro_list(output).unwrap();
        assert_eq!(distros.len(), 2);
        assert_eq!(distros[0], "Ubuntu-22.04");
        assert_eq!(distros[1], "Debian");
    }

    #[test]
    fn test_parse_distro_list_single_distro() {
        let output = b"Ubuntu-22.04\n";
        let distros = parse_distro_list(output).unwrap();
        assert_eq!(distros.len(), 1);
        assert_eq!(distros[0], "Ubuntu-22.04");
    }

    #[test]
    fn test_normalize_wsl_output_utf8() {
        let input = b"Ubuntu-22.04\nDebian\n";
        let output = normalize_wsl_output(input);
        assert_eq!(output, "Ubuntu-22.04\nDebian\n");
    }

    #[test]
    fn test_normalize_wsl_output_utf16le() {
        // "Ubuntu" in UTF-16LE: U=0x0055, b=0x0062, u=0x0075, n=0x006E, t=0x0074, u=0x0075
        let input = vec![
            0x55, 0x00, // U
            0x62, 0x00, // b
            0x75, 0x00, // u
            0x6E, 0x00, // n
            0x74, 0x00, // t
            0x75, 0x00, // u
        ];
        let output = normalize_wsl_output(&input);
        assert_eq!(output, "Ubuntu");
    }

    #[test]
    fn test_normalize_wsl_output_utf16le_with_newline() {
        // "Hello World\n" in UTF-16LE - needs to be long enough to trigger UTF-16LE detection
        // (at least 10 characters to have 5+ null bytes in odd positions)
        let input = vec![
            0x48, 0x00, // H
            0x65, 0x00, // e
            0x6C, 0x00, // l
            0x6C, 0x00, // l
            0x6F, 0x00, // o
            0x20, 0x00, // space
            0x57, 0x00, // W
            0x6F, 0x00, // o
            0x72, 0x00, // r
            0x6C, 0x00, // l
            0x64, 0x00, // d
            0x0A, 0x00, // \n
        ];
        let output = normalize_wsl_output(&input);
        assert_eq!(output, "Hello World\n");
    }

    #[test]
    fn test_normalize_wsl_output_short_input() {
        // Input too short to be UTF-16LE
        let input = b"Hi";
        let output = normalize_wsl_output(input);
        assert_eq!(output, "Hi");
    }

    #[test]
    fn test_normalize_wsl_output_odd_length() {
        // Odd length input cannot be UTF-16LE
        let input = b"Hello";
        let output = normalize_wsl_output(input);
        assert_eq!(output, "Hello");
    }

    // Integration tests (only run on Windows)

    #[test]
    #[cfg(target_os = "windows")]
    fn test_is_wsl_available_integration() {
        // This test will only pass if WSL is actually installed on the Windows system
        // We don't assert the result, just verify it doesn't panic
        let result = is_wsl_available();
        assert!(result.is_ok());

        // If WSL is available, we should get at least one distro
        if matches!(result, Ok(true)) {
            // Try to get the distro list
            let output = CommandSpec::new("wsl")
                .args(["-l", "-q"])
                .to_command()
                .output();
            if let Ok(output) = output {
                let distros = parse_distro_list(&output.stdout);
                assert!(distros.is_ok());
                let distros = distros.unwrap();
                assert!(!distros.is_empty(), "WSL is available but no distros found");
            }
        }
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_parse_distro_list_integration() {
        // Try to get actual WSL distro list
        if let Ok(output) = CommandSpec::new("wsl")
            .args(["-l", "-q"])
            .to_command()
            .output()
            && output.status.success()
        {
            let distros = parse_distro_list(&output.stdout);
            assert!(distros.is_ok());
            // We don't know what distros are installed, but if WSL works,
            // we should be able to parse the output without errors
        }
    }

    // Unit tests for validate_claude_in_wsl

    #[test]
    fn test_validate_claude_in_wsl_on_non_windows() {
        if !cfg!(target_os = "windows") {
            let result = validate_claude_in_wsl(None);
            assert!(result.is_ok());
            assert!(!result.unwrap());
        }
    }

    #[test]
    fn test_validate_claude_in_wsl_with_distro_on_non_windows() {
        if !cfg!(target_os = "windows") {
            let result = validate_claude_in_wsl(Some("Ubuntu-22.04"));
            assert!(result.is_ok());
            assert!(!result.unwrap());
        }
    }

    // Integration tests for validate_claude_in_wsl (Windows only)

    #[test]
    #[cfg(target_os = "windows")]
    fn test_validate_claude_in_wsl_default_distro_integration() {
        // This test checks if Claude is available in the default WSL distribution
        // We don't assert the result, just verify it doesn't panic and returns a valid result
        let result = validate_claude_in_wsl(None);

        // Should either succeed with true/false or fail with WslNotAvailable
        match result {
            Ok(available) => {
                // Valid result - Claude is either available or not
                println!("Claude in default WSL distro: {available}");
            }
            Err(RunnerError::WslNotAvailable { .. }) => {
                // WSL is not available on this system - that's okay
                println!("WSL is not available on this system");
            }
            Err(e) => {
                panic!("Unexpected error: {e:?}");
            }
        }
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_validate_claude_in_wsl_specific_distro_integration() {
        // First, check if WSL is available and get distro list
        if let Ok(output) = CommandSpec::new("wsl")
            .args(["-l", "-q"])
            .to_command()
            .output()
            && output.status.success()
            && let Ok(distros) = parse_distro_list(&output.stdout)
            && !distros.is_empty()
        {
            // Test with the first available distro
            let distro = &distros[0];
            let result = validate_claude_in_wsl(Some(distro));

            // Should return Ok with true or false
            assert!(result.is_ok());
            let available = result.unwrap();
            println!("Claude in {distro} distro: {available}");
        }
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_validate_claude_in_wsl_nonexistent_distro() {
        // Test with a distro that definitely doesn't exist
        let result = validate_claude_in_wsl(Some("NonexistentDistro12345"));

        // Should return Ok(false) since the distro doesn't exist
        // or Err if WSL itself is not available
        match result {
            Ok(false) => {
                // Expected - distro doesn't exist, so Claude can't be found
                println!("Correctly returned false for nonexistent distro");
            }
            Err(RunnerError::WslNotAvailable { .. }) => {
                // WSL is not available - that's also acceptable
                println!("WSL is not available on this system");
            }
            Ok(true) => {
                panic!("Should not find Claude in nonexistent distro");
            }
            Err(e) => {
                panic!("Unexpected error: {e:?}");
            }
        }
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_validate_claude_in_wsl_consistency_with_is_wsl_available() {
        // If WSL is available, validate_claude_in_wsl should not return WslNotAvailable error
        let wsl_available = is_wsl_available();
        let claude_result = validate_claude_in_wsl(None);

        if matches!(wsl_available, Ok(true)) {
            // WSL is available, so validate_claude_in_wsl should return Ok (not WslNotAvailable)
            match claude_result {
                Ok(_) => {
                    // Good - returned a boolean result
                    println!("WSL is available, Claude validation returned Ok");
                }
                Err(RunnerError::WslNotAvailable { .. }) => {
                    panic!("WSL is available but validate_claude_in_wsl returned WslNotAvailable");
                }
                Err(e) => {
                    panic!("Unexpected error: {e:?}");
                }
            }
        }
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_validate_claude_in_wsl_multiple_distros() {
        // Test Claude validation across all available distros
        if let Ok(output) = CommandSpec::new("wsl")
            .args(["-l", "-q"])
            .to_command()
            .output()
            && output.status.success()
            && let Ok(distros) = parse_distro_list(&output.stdout)
        {
            for distro in distros {
                let result = validate_claude_in_wsl(Some(&distro));

                // Each distro should return a valid result
                match result {
                    Ok(available) => {
                        println!("Claude in {distro}: {available}");
                    }
                    Err(e) => {
                        println!("Error checking Claude in {distro}: {e:?}");
                    }
                }
            }
        }
    }
}

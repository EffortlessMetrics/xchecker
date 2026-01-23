//! WSL (Windows Subsystem for Linux) detection and utilities
//!
//! Provides functions for detecting WSL availability, listing distributions,
//! and validating Claude CLI availability within WSL environments.

use crate::error::RunnerError;
use crate::runner::CommandSpec;

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
/// use xchecker_engine::wsl::is_wsl_available;
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
/// use xchecker_engine::wsl::parse_distro_list;
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
/// use xchecker_engine::wsl::validate_claude_in_wsl;
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

/// Translate a Windows path to WSL format
///
/// This function converts Windows paths to their WSL equivalents using `wsl.exe wslpath -a`
/// for correctness. If wslpath fails, it falls back to a heuristic translation.
///
/// # Path Translation Rules
/// - Drive letters: `C:\path\to\file` → `/mnt/c/path/to/file`
/// - UNC paths: `\\server\share\path` → `/mnt/server/share/path` (heuristic only)
/// - Forward slashes are preserved
/// - Backslashes are converted to forward slashes
///
/// # Arguments
/// * `windows_path` - Windows path to translate (e.g., "C:\\Users\\name\\file.txt")
///
/// # Returns
/// * `Ok(String)` - WSL-formatted path
/// * `Err(RunnerError)` - An error occurred during translation
///
/// # Examples
/// ```no_run
/// use xchecker_engine::wsl::translate_win_to_wsl;
/// use std::path::Path;
///
/// # #[cfg(target_os = "windows")]
/// # {
/// let windows_path = Path::new("C:\\Users\\name\\file.txt");
/// let wsl_path = translate_win_to_wsl(windows_path).unwrap();
/// assert_eq!(wsl_path, "/mnt/c/Users/name/file.txt");
/// # }
/// ```
#[allow(dead_code)] // Future-facing: used for WSL path translation when needed
pub fn translate_win_to_wsl(windows_path: &std::path::Path) -> Result<String, RunnerError> {
    // On non-Windows platforms, just return the path as-is
    if !cfg!(target_os = "windows") {
        return Ok(windows_path.display().to_string());
    }

    // Convert path to string
    let path_str = windows_path.to_string_lossy().to_string();

    // Try using wslpath for canonical translation
    match try_wslpath(&path_str) {
        Ok(wsl_path) => Ok(wsl_path),
        Err(_) => {
            // Fallback to heuristic translation
            Ok(translate_path_heuristic(&path_str))
        }
    }
}

/// Try to translate a Windows path using `wsl.exe wslpath -a`
///
/// This is the preferred method as it handles all edge cases correctly,
/// including UNC paths, network drives, and special characters.
///
/// # Arguments
/// * `windows_path` - Windows path string
///
/// # Returns
/// * `Ok(String)` - WSL-formatted path from wslpath
/// * `Err(RunnerError)` - wslpath command failed or is not available
#[allow(dead_code)] // Future-facing: used for WSL path translation when needed
fn try_wslpath(windows_path: &str) -> Result<String, RunnerError> {
    // Execute wsl.exe wslpath -a <path> using CommandSpec for secure argv-style execution
    let output = CommandSpec::new("wsl")
        .args(["wslpath", "-a", windows_path])
        .to_command()
        .output()
        .map_err(|e| RunnerError::WslExecutionFailed {
            reason: format!("Failed to execute wslpath: {e}"),
        })?;

    if !output.status.success() {
        return Err(RunnerError::WslExecutionFailed {
            reason: format!(
                "wslpath command failed with exit code: {}",
                output.status.code().unwrap_or(-1)
            ),
        });
    }

    // Parse the output
    let wsl_path = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if wsl_path.is_empty() {
        return Err(RunnerError::WslExecutionFailed {
            reason: "wslpath returned empty output".to_string(),
        });
    }

    Ok(wsl_path)
}

/// Translate a Windows path to WSL format using heuristic rules
///
/// This is a fallback when wslpath is not available. It handles:
/// - Drive letters: C:\ → /mnt/c/
/// - UNC paths: \\server\share → /mnt/server/share (best effort)
/// - Backslash to forward slash conversion
///
/// # Arguments
/// * `windows_path` - Windows path string
///
/// # Returns
/// * `String` - WSL-formatted path (best effort)
#[allow(dead_code)] // Future-facing: used for WSL path translation when needed
fn translate_path_heuristic(windows_path: &str) -> String {
    let path = windows_path.trim();

    // Handle UNC paths: \\server\share\path → /mnt/server/share/path
    if path.starts_with("\\\\") || path.starts_with("//") {
        let without_prefix = path.trim_start_matches("\\\\").trim_start_matches("//");
        let normalized = without_prefix.replace('\\', "/");
        return format!("/mnt/{normalized}");
    }

    // Handle drive letters: C:\path → /mnt/c/path
    if path.len() >= 2 && path.chars().nth(1) == Some(':') {
        // Safety: We've verified path.len() >= 2, so .next() always succeeds
        let drive_letter = path
            .chars()
            .next()
            .expect("path length already verified >= 2")
            .to_ascii_lowercase();
        let rest = if path.len() > 2 { &path[2..] } else { "" };

        // Remove leading backslash or forward slash from rest
        let rest = rest.trim_start_matches('\\').trim_start_matches('/');

        // Convert backslashes to forward slashes
        let rest_normalized = rest.replace('\\', "/");

        if rest_normalized.is_empty() {
            return format!("/mnt/{drive_letter}");
        }
        return format!("/mnt/{drive_letter}/{rest_normalized}");
    }

    // For paths without drive letters or UNC prefix, just normalize slashes
    path.replace('\\', "/")
}

/// Translate environment variables for WSL execution
///
/// This function adapts environment variables for use in WSL by translating Windows paths
/// to WSL format while preserving necessary context. It handles common path-containing
/// environment variables like PATH, TEMP, TMP, HOME, etc.
///
/// # Path Translation Rules
/// - PATH: Split by `;`, translate each Windows path, rejoin with `:`
/// - TEMP, TMP, HOME, USERPROFILE: Translate single path values
/// - Other variables: Pass through unchanged
///
/// # Arguments
/// * `env` - Slice of (key, value) tuples representing environment variables
///
/// # Returns
/// * `Vec<(String, String)>` - Translated environment variables
///
/// # Examples
/// ```no_run
/// use xchecker_engine::wsl::translate_env_for_wsl;
///
/// # #[cfg(target_os = "windows")]
/// # {
/// let env = vec![
///     ("PATH".to_string(), "C:\\Windows\\System32;C:\\Program Files".to_string()),
///     ("TEMP".to_string(), "C:\\Users\\name\\AppData\\Local\\Temp".to_string()),
///     ("USER".to_string(), "name".to_string()),
/// ];
///
/// let translated = translate_env_for_wsl(&env);
/// // PATH will be translated to WSL format with : separator
/// // TEMP will be translated to WSL path
/// // USER will be passed through unchanged
/// # }
/// ```
#[must_use]
#[allow(dead_code)] // Future-facing: used for WSL environment translation when needed
pub fn translate_env_for_wsl(env: &[(String, String)]) -> Vec<(String, String)> {
    // On non-Windows platforms, return env as-is
    if !cfg!(target_os = "windows") {
        return env.to_vec();
    }

    let mut translated = Vec::new();

    for (key, value) in env {
        let key_upper = key.to_uppercase();

        // Handle PATH specially - it's a list of paths separated by semicolons
        if key_upper == "PATH" {
            let wsl_path = translate_path_env_var(value);
            translated.push((key.clone(), wsl_path));
        }
        // Handle single-path environment variables
        else if is_path_env_var(&key_upper) {
            // Try to translate as a Windows path
            if let Ok(wsl_path) = translate_win_to_wsl(std::path::Path::new(value)) {
                translated.push((key.clone(), wsl_path));
            } else {
                // If translation fails, pass through unchanged
                translated.push((key.clone(), value.clone()));
            }
        }
        // All other variables pass through unchanged
        else {
            translated.push((key.clone(), value.clone()));
        }
    }

    translated
}

/// Check if an environment variable name typically contains a path
///
/// # Arguments
/// * `key` - Environment variable name (should be uppercase)
///
/// # Returns
/// * `bool` - True if the variable typically contains a path
#[allow(dead_code)] // Future-facing: used for WSL environment translation when needed
fn is_path_env_var(key: &str) -> bool {
    matches!(
        key,
        "TEMP"
            | "TMP"
            | "HOME"
            | "USERPROFILE"
            | "APPDATA"
            | "LOCALAPPDATA"
            | "PROGRAMDATA"
            | "PROGRAMFILES"
            | "PROGRAMFILES(X86)"
            | "SYSTEMROOT"
            | "WINDIR"
            | "HOMEDRIVE"
            | "HOMEPATH"
            | "TMPDIR"
    )
}

/// Translate a PATH environment variable from Windows to WSL format
///
/// Splits the PATH by semicolons, translates each Windows path to WSL format,
/// and rejoins with colons.
///
/// # Arguments
/// * `path_value` - The PATH environment variable value (Windows format with ; separators)
///
/// # Returns
/// * `String` - Translated PATH value (WSL format with : separators)
#[allow(dead_code)] // Future-facing: used for WSL environment translation when needed
fn translate_path_env_var(path_value: &str) -> String {
    // Split by semicolon (Windows PATH separator)
    let paths: Vec<&str> = path_value.split(';').collect();

    let mut translated_paths = Vec::new();

    for path in paths {
        let trimmed = path.trim();

        // Skip empty entries
        if trimmed.is_empty() {
            continue;
        }

        // Try to translate the path
        if let Ok(wsl_path) = translate_win_to_wsl(std::path::Path::new(trimmed)) {
            translated_paths.push(wsl_path);
        } else {
            // If translation fails, include the original path
            // This handles cases where the path might already be in Unix format
            translated_paths.push(trimmed.to_string());
        }
    }

    // Join with colon (Unix PATH separator)
    translated_paths.join(":")
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
    // use std::process::Command;

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

    // Unit tests for path translation

    #[test]
    fn test_translate_path_heuristic_drive_letter() {
        let input = "C:\\Users\\name\\file.txt";
        let output = translate_path_heuristic(input);
        assert_eq!(output, "/mnt/c/Users/name/file.txt");
    }

    #[test]
    fn test_translate_path_heuristic_drive_letter_lowercase() {
        let input = "c:\\users\\name\\file.txt";
        let output = translate_path_heuristic(input);
        assert_eq!(output, "/mnt/c/users/name/file.txt");
    }

    #[test]
    fn test_translate_path_heuristic_drive_letter_uppercase() {
        let input = "D:\\Projects\\xchecker\\src\\main.rs";
        let output = translate_path_heuristic(input);
        assert_eq!(output, "/mnt/d/Projects/xchecker/src/main.rs");
    }

    #[test]
    fn test_translate_path_heuristic_drive_letter_root() {
        let input = "C:\\";
        let output = translate_path_heuristic(input);
        assert_eq!(output, "/mnt/c");
    }

    #[test]
    fn test_translate_path_heuristic_drive_letter_no_trailing_slash() {
        let input = "C:";
        let output = translate_path_heuristic(input);
        assert_eq!(output, "/mnt/c");
    }

    #[test]
    fn test_translate_path_heuristic_drive_letter_forward_slashes() {
        let input = "C:/Users/name/file.txt";
        let output = translate_path_heuristic(input);
        assert_eq!(output, "/mnt/c/Users/name/file.txt");
    }

    #[test]
    fn test_translate_path_heuristic_drive_letter_mixed_slashes() {
        let input = "C:\\Users/name\\file.txt";
        let output = translate_path_heuristic(input);
        assert_eq!(output, "/mnt/c/Users/name/file.txt");
    }

    #[test]
    fn test_translate_path_heuristic_unc_path() {
        let input = "\\\\server\\share\\path\\to\\file.txt";
        let output = translate_path_heuristic(input);
        assert_eq!(output, "/mnt/server/share/path/to/file.txt");
    }

    #[test]
    fn test_translate_path_heuristic_unc_path_forward_slashes() {
        let input = "//server/share/path/to/file.txt";
        let output = translate_path_heuristic(input);
        assert_eq!(output, "/mnt/server/share/path/to/file.txt");
    }

    #[test]
    fn test_translate_path_heuristic_unc_path_mixed_slashes() {
        let input = "\\\\server\\share/path\\to/file.txt";
        let output = translate_path_heuristic(input);
        assert_eq!(output, "/mnt/server/share/path/to/file.txt");
    }

    #[test]
    fn test_translate_path_heuristic_unc_path_root() {
        let input = "\\\\server\\share";
        let output = translate_path_heuristic(input);
        assert_eq!(output, "/mnt/server/share");
    }

    #[test]
    fn test_translate_path_heuristic_relative_path() {
        let input = "relative\\path\\to\\file.txt";
        let output = translate_path_heuristic(input);
        assert_eq!(output, "relative/path/to/file.txt");
    }

    #[test]
    fn test_translate_path_heuristic_unix_style_path() {
        let input = "/usr/local/bin/claude";
        let output = translate_path_heuristic(input);
        assert_eq!(output, "/usr/local/bin/claude");
    }

    #[test]
    fn test_translate_path_heuristic_empty_path() {
        let input = "";
        let output = translate_path_heuristic(input);
        assert_eq!(output, "");
    }

    #[test]
    fn test_translate_path_heuristic_whitespace() {
        let input = "  C:\\Users\\name\\file.txt  ";
        let output = translate_path_heuristic(input);
        assert_eq!(output, "/mnt/c/Users/name/file.txt");
    }

    #[test]
    fn test_translate_path_heuristic_special_characters() {
        let input = "C:\\Users\\name with spaces\\file (1).txt";
        let output = translate_path_heuristic(input);
        assert_eq!(output, "/mnt/c/Users/name with spaces/file (1).txt");
    }

    #[test]
    fn test_translate_win_to_wsl_on_non_windows() {
        if !cfg!(target_os = "windows") {
            let path = std::path::Path::new("/usr/local/bin/claude");
            let result = translate_win_to_wsl(path);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), "/usr/local/bin/claude");
        }
    }

    // Integration tests for translate_win_to_wsl (Windows only)

    #[test]
    #[cfg(target_os = "windows")]
    fn test_translate_win_to_wsl_drive_letter_integration() {
        let path = std::path::Path::new("C:\\Windows\\System32");
        let result = translate_win_to_wsl(path);

        assert!(result.is_ok());
        let wsl_path = result.unwrap();

        // Should either use wslpath (preferred) or fallback to heuristic
        // Both should produce a valid WSL path starting with /mnt/c
        assert!(
            wsl_path.starts_with("/mnt/c"),
            "Expected path to start with /mnt/c, got: {wsl_path}"
        );
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_translate_win_to_wsl_current_directory_integration() {
        // Get current directory
        if let Ok(current_dir) = std::env::current_dir() {
            let result = translate_win_to_wsl(&current_dir);

            assert!(result.is_ok());
            let wsl_path = result.unwrap();

            // Should produce a valid WSL path
            assert!(
                wsl_path.starts_with("/mnt/") || wsl_path.starts_with('/'),
                "Expected valid WSL path, got: {wsl_path}"
            );

            println!(
                "Current dir: {} -> WSL: {}",
                current_dir.display(),
                wsl_path
            );
        }
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_translate_win_to_wsl_temp_directory_integration() {
        // Get temp directory
        let temp_dir = std::env::temp_dir();
        let result = translate_win_to_wsl(&temp_dir);

        assert!(result.is_ok());
        let wsl_path = result.unwrap();

        // Should produce a valid WSL path
        assert!(
            wsl_path.starts_with("/mnt/") || wsl_path.starts_with('/'),
            "Expected valid WSL path, got: {wsl_path}"
        );

        println!("Temp dir: {} -> WSL: {}", temp_dir.display(), wsl_path);
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_translate_win_to_wsl_with_spaces_integration() {
        let path = std::path::Path::new("C:\\Program Files\\Common Files");
        let result = translate_win_to_wsl(path);

        assert!(result.is_ok());
        let wsl_path = result.unwrap();

        // Should handle spaces correctly
        assert!(
            wsl_path.contains("Program Files") || wsl_path.contains("Program%20Files"),
            "Expected path to contain 'Program Files', got: {wsl_path}"
        );
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_translate_win_to_wsl_multiple_drives_integration() {
        // Test multiple drive letters
        for drive in ['C', 'D', 'E'] {
            let path_str = format!("{drive}:\\");
            let path = std::path::Path::new(&path_str);
            let result = translate_win_to_wsl(path);

            assert!(result.is_ok());
            let wsl_path = result.unwrap();

            // Should map to /mnt/<drive_lowercase>
            let expected_prefix = format!("/mnt/{}", drive.to_ascii_lowercase());
            assert!(
                wsl_path.starts_with(&expected_prefix),
                "Expected path to start with {expected_prefix}, got: {wsl_path}"
            );
        }
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_translate_win_to_wsl_fallback_when_wslpath_unavailable() {
        // This test verifies that the fallback heuristic works
        // We can't easily disable wslpath, but we can test the heuristic directly
        let path = std::path::Path::new("C:\\Users\\test\\file.txt");
        let result = translate_win_to_wsl(path);

        assert!(result.is_ok());
        let wsl_path = result.unwrap();

        // Should produce a valid path either way
        assert!(
            wsl_path.starts_with("/mnt/c"),
            "Expected path to start with /mnt/c, got: {wsl_path}"
        );
        assert!(
            wsl_path.contains("Users")
                && wsl_path.contains("test")
                && wsl_path.contains("file.txt"),
            "Expected path to contain all components, got: {wsl_path}"
        );
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_try_wslpath_integration() {
        // Test wslpath directly if WSL is available
        let result = try_wslpath("C:\\Windows");

        // Should either succeed or fail gracefully
        match result {
            Ok(wsl_path) => {
                assert!(
                    wsl_path.starts_with("/mnt/c"),
                    "Expected wslpath to return /mnt/c path, got: {wsl_path}"
                );
                println!("wslpath succeeded: C:\\Windows -> {wsl_path}");
            }
            Err(e) => {
                // wslpath might not be available or WSL might not be installed
                println!("wslpath failed (expected if WSL not available): {e:?}");
            }
        }
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_translate_win_to_wsl_consistency() {
        // Test that translating the same path multiple times gives the same result
        let path = std::path::Path::new("C:\\Users\\test");

        let result1 = translate_win_to_wsl(path);
        let result2 = translate_win_to_wsl(path);

        assert!(result1.is_ok());
        assert!(result2.is_ok());

        assert_eq!(
            result1.unwrap(),
            result2.unwrap(),
            "Path translation should be consistent"
        );
    }

    // Unit tests for translate_env_for_wsl

    #[test]
    fn test_translate_env_for_wsl_on_non_windows() {
        if !cfg!(target_os = "windows") {
            let env = vec![
                ("PATH".to_string(), "/usr/bin:/usr/local/bin".to_string()),
                ("HOME".to_string(), "/home/user".to_string()),
                ("USER".to_string(), "testuser".to_string()),
            ];

            let result = translate_env_for_wsl(&env);

            // On non-Windows, should return unchanged
            assert_eq!(result.len(), 3);
            assert_eq!(
                result[0],
                ("PATH".to_string(), "/usr/bin:/usr/local/bin".to_string())
            );
            assert_eq!(result[1], ("HOME".to_string(), "/home/user".to_string()));
            assert_eq!(result[2], ("USER".to_string(), "testuser".to_string()));
        }
    }

    #[test]
    fn test_translate_env_for_wsl_empty() {
        let env: Vec<(String, String)> = vec![];
        let result = translate_env_for_wsl(&env);
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_translate_env_for_wsl_non_path_vars() {
        let env = vec![
            ("USER".to_string(), "testuser".to_string()),
            ("LANG".to_string(), "en_US.UTF-8".to_string()),
            ("TERM".to_string(), "xterm-256color".to_string()),
        ];

        let result = translate_env_for_wsl(&env);

        // Non-path variables should pass through unchanged
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], ("USER".to_string(), "testuser".to_string()));
        assert_eq!(result[1], ("LANG".to_string(), "en_US.UTF-8".to_string()));
        assert_eq!(
            result[2],
            ("TERM".to_string(), "xterm-256color".to_string())
        );
    }

    #[test]
    fn test_is_path_env_var() {
        // Test common path environment variables
        // Note: PATH is handled specially in translate_env_for_wsl, not by is_path_env_var
        assert!(is_path_env_var("TEMP"));
        assert!(is_path_env_var("TMP"));
        assert!(is_path_env_var("HOME"));
        assert!(is_path_env_var("USERPROFILE"));
        assert!(is_path_env_var("APPDATA"));
        assert!(is_path_env_var("LOCALAPPDATA"));
        assert!(is_path_env_var("PROGRAMDATA"));
        assert!(is_path_env_var("PROGRAMFILES"));
        assert!(is_path_env_var("PROGRAMFILES(X86)"));
        assert!(is_path_env_var("SYSTEMROOT"));
        assert!(is_path_env_var("WINDIR"));
        assert!(is_path_env_var("HOMEDRIVE"));
        assert!(is_path_env_var("HOMEPATH"));
        assert!(is_path_env_var("TMPDIR"));

        // Test non-path variables
        assert!(!is_path_env_var("PATH")); // PATH is handled specially
        assert!(!is_path_env_var("USER"));
        assert!(!is_path_env_var("LANG"));
        assert!(!is_path_env_var("TERM"));
        assert!(!is_path_env_var("SHELL"));
        assert!(!is_path_env_var("EDITOR"));
    }

    #[test]
    fn test_translate_path_env_var_empty() {
        let result = translate_path_env_var("");
        assert_eq!(result, "");
    }

    // These tests are Windows-only because they test Windows→WSL path translation
    // which only makes sense when running on Windows host calling into WSL
    #[test]
    #[cfg(target_os = "windows")]
    fn test_translate_path_env_var_single_path() {
        let input = "C:\\Windows\\System32";
        let result = translate_path_env_var(input);

        // Should translate to WSL format
        assert!(result.starts_with("/mnt/c"));
        assert!(result.contains("Windows"));
        assert!(result.contains("System32"));
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_translate_path_env_var_multiple_paths() {
        let input = "C:\\Windows\\System32;C:\\Program Files;D:\\Tools";
        let result = translate_path_env_var(input);

        // Should be separated by colons
        assert!(result.contains(':'));

        // Should contain translated paths
        let parts: Vec<&str> = result.split(':').collect();
        assert!(
            parts.len() >= 3,
            "Expected at least 3 paths, got {}",
            parts.len()
        );

        // First path should be /mnt/c/...
        assert!(
            parts[0].starts_with("/mnt/c"),
            "First path should start with /mnt/c, got: {}",
            parts[0]
        );
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_translate_path_env_var_with_empty_entries() {
        let input = "C:\\Windows;;C:\\Program Files;";
        let result = translate_path_env_var(input);

        // Empty entries should be filtered out
        let parts: Vec<&str> = result.split(':').collect();

        // Should have 2 non-empty paths
        let non_empty: Vec<&str> = parts.iter().filter(|s| !s.is_empty()).copied().collect();
        assert_eq!(
            non_empty.len(),
            2,
            "Expected 2 non-empty paths, got {}",
            non_empty.len()
        );
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_translate_path_env_var_with_whitespace() {
        let input = " C:\\Windows ; C:\\Program Files ";
        let result = translate_path_env_var(input);

        // Whitespace should be trimmed
        let parts: Vec<&str> = result.split(':').collect();
        assert!(parts.len() >= 2);

        // Paths should not have leading/trailing whitespace
        for part in parts {
            assert_eq!(
                part.trim(),
                part,
                "Path should not have whitespace: '{part}'"
            );
        }
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_translate_path_env_var_mixed_formats() {
        // Mix of Windows paths and Unix-style paths
        let input = "C:\\Windows;/usr/local/bin;D:\\Tools";
        let result = translate_path_env_var(input);

        // Should handle both formats
        let parts: Vec<&str> = result.split(':').collect();
        assert!(parts.len() >= 3);
    }

    // Integration tests for translate_env_for_wsl (Windows only)

    #[test]
    #[cfg(target_os = "windows")]
    fn test_translate_env_for_wsl_path_integration() {
        let env = vec![(
            "PATH".to_string(),
            "C:\\Windows\\System32;C:\\Program Files".to_string(),
        )];

        let result = translate_env_for_wsl(&env);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "PATH");

        // PATH should be translated and use colon separator
        let path_value = &result[0].1;
        assert!(path_value.contains(':'), "PATH should use colon separator");
        assert!(path_value.contains("/mnt/c"), "PATH should contain /mnt/c");
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_translate_env_for_wsl_temp_integration() {
        let env = vec![(
            "TEMP".to_string(),
            "C:\\Users\\test\\AppData\\Local\\Temp".to_string(),
        )];

        let result = translate_env_for_wsl(&env);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "TEMP");

        // TEMP should be translated to WSL format
        let temp_value = &result[0].1;
        assert!(
            temp_value.starts_with("/mnt/c"),
            "TEMP should start with /mnt/c, got: {temp_value}"
        );
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_translate_env_for_wsl_home_integration() {
        let env = vec![("HOME".to_string(), "C:\\Users\\testuser".to_string())];

        let result = translate_env_for_wsl(&env);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "HOME");

        // HOME should be translated to WSL format
        let home_value = &result[0].1;
        assert!(
            home_value.starts_with("/mnt/c"),
            "HOME should start with /mnt/c, got: {home_value}"
        );
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_translate_env_for_wsl_mixed_integration() {
        let env = vec![
            (
                "PATH".to_string(),
                "C:\\Windows\\System32;C:\\Program Files".to_string(),
            ),
            (
                "TEMP".to_string(),
                "C:\\Users\\test\\AppData\\Local\\Temp".to_string(),
            ),
            ("USER".to_string(), "testuser".to_string()),
            ("LANG".to_string(), "en_US.UTF-8".to_string()),
        ];

        let result = translate_env_for_wsl(&env);

        assert_eq!(result.len(), 4);

        // PATH should be translated
        let path_entry = result.iter().find(|(k, _)| k == "PATH").unwrap();
        assert!(
            path_entry.1.contains(':'),
            "PATH should use colon separator"
        );
        assert!(
            path_entry.1.contains("/mnt/c"),
            "PATH should contain /mnt/c"
        );

        // TEMP should be translated
        let temp_entry = result.iter().find(|(k, _)| k == "TEMP").unwrap();
        assert!(
            temp_entry.1.starts_with("/mnt/c"),
            "TEMP should start with /mnt/c"
        );

        // USER should pass through unchanged
        let user_entry = result.iter().find(|(k, _)| k == "USER").unwrap();
        assert_eq!(user_entry.1, "testuser");

        // LANG should pass through unchanged
        let lang_entry = result.iter().find(|(k, _)| k == "LANG").unwrap();
        assert_eq!(lang_entry.1, "en_US.UTF-8");
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_translate_env_for_wsl_case_insensitive_integration() {
        // Test that environment variable names are case-insensitive
        let env = vec![
            ("path".to_string(), "C:\\Windows".to_string()),
            ("Path".to_string(), "C:\\Program Files".to_string()),
            ("PATH".to_string(), "C:\\Users".to_string()),
        ];

        let result = translate_env_for_wsl(&env);

        // All should be recognized as PATH and translated
        assert_eq!(result.len(), 3);
        for (key, value) in result {
            assert!(key.to_uppercase() == "PATH");
            assert!(
                value.contains("/mnt/c"),
                "Value should be translated: {value}"
            );
        }
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_translate_env_for_wsl_current_directory_integration() {
        // Get current directory and test translation
        if let Ok(current_dir) = std::env::current_dir() {
            let current_dir_str = current_dir.to_string_lossy().to_string();

            let env = vec![("WORKDIR".to_string(), current_dir_str.clone())];

            let result = translate_env_for_wsl(&env);

            // WORKDIR is not a known path variable, so it should pass through unchanged
            assert_eq!(result.len(), 1);
            assert_eq!(result[0].0, "WORKDIR");
            assert_eq!(result[0].1, current_dir_str);
        }
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_translate_env_for_wsl_special_characters_integration() {
        let env = vec![(
            "TEMP".to_string(),
            "C:\\Users\\test user\\AppData\\Local\\Temp".to_string(),
        )];

        let result = translate_env_for_wsl(&env);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "TEMP");

        // Should handle spaces in paths
        let temp_value = &result[0].1;
        assert!(
            temp_value.contains("test user") || temp_value.contains("test%20user"),
            "TEMP should preserve or encode spaces: {temp_value}"
        );
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_translate_env_for_wsl_multiple_drives_integration() {
        let env = vec![(
            "PATH".to_string(),
            "C:\\Windows;D:\\Tools;E:\\Apps".to_string(),
        )];

        let result = translate_env_for_wsl(&env);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "PATH");

        let path_value = &result[0].1;

        // Should translate all drives
        assert!(path_value.contains("/mnt/c"), "Should contain /mnt/c");
        assert!(path_value.contains("/mnt/d"), "Should contain /mnt/d");
        assert!(path_value.contains("/mnt/e"), "Should contain /mnt/e");
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_translate_env_for_wsl_preserves_order_integration() {
        let env = vec![
            ("VAR1".to_string(), "value1".to_string()),
            ("PATH".to_string(), "C:\\Windows".to_string()),
            ("VAR2".to_string(), "value2".to_string()),
            ("TEMP".to_string(), "C:\\Temp".to_string()),
            ("VAR3".to_string(), "value3".to_string()),
        ];

        let result = translate_env_for_wsl(&env);

        // Order should be preserved
        assert_eq!(result.len(), 5);
        assert_eq!(result[0].0, "VAR1");
        assert_eq!(result[1].0, "PATH");
        assert_eq!(result[2].0, "VAR2");
        assert_eq!(result[3].0, "TEMP");
        assert_eq!(result[4].0, "VAR3");
    }
}

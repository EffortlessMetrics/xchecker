//! Doctor command for environment health checks
//!
//! Provides preflight checks for Claude CLI availability, runner configuration,
//! write permissions, and configuration validity.

// Re-export shared types from xchecker-utils
pub use xchecker_utils::types::{CheckStatus, DoctorCheck, DoctorOutput};

pub mod wsl;

use anyhow::Result;
use chrono::Utc;
use std::path::Path;

use xchecker_config::Config;
use xchecker_utils::cache;
use xchecker_utils::logging;
use xchecker_utils::paths;
use xchecker_utils::runner::{CommandSpec, Runner, RunnerMode, WslOptions};

/// Doctor command implementation
pub struct DoctorCommand {
    config: Config,
    cache: Option<cache::InsightCache>,
}

impl DoctorCommand {
    /// Create a new doctor command with the given configuration
    #[must_use]
    pub fn new(config: Config) -> Self {
        // Try to create cache for stats (non-fatal if it fails)
        let cache_dir = paths::cache_dir();
        let cache = cache::InsightCache::new(cache_dir).ok();

        Self { config, cache }
    }

    /// Create from CLI args (wired from cli module)
    #[allow(dead_code)] // CLI integration point
    pub fn new_from_cli(cfg: &Config, _matches: &clap::ArgMatches) -> Result<DoctorCommand> {
        let cache_dir = paths::cache_dir();
        let cache = cache::InsightCache::new(cache_dir).ok();

        Ok(Self {
            config: cfg.clone(),
            cache,
        })
    }

    /// Run all health checks and return the doctor output
    #[allow(dead_code)] // CLI integration point
    pub fn run_with_options(&mut self) -> Result<DoctorOutput> {
        self.run_with_options_strict(false)
    }

    /// Run all health checks with optional strict mode
    ///
    /// In strict mode, warnings are treated as failures for exit code purposes
    pub fn run_with_options_strict(&mut self, strict_exit: bool) -> Result<DoctorOutput> {
        let mut checks = Vec::new();

        // Check if stub mode should force a specific check to fail
        // This is used for testing doctor exit behavior
        if let Ok(force_fail_check) = std::env::var("XCHECKER_STUB_FORCE_FAIL") {
            checks.push(DoctorCheck {
                name: force_fail_check.clone(),
                status: CheckStatus::Fail,
                details: format!("Forced failure for testing: {force_fail_check}"),
            });

            // Sort checks by name for stable output (required for JCS canonical emission)
            checks.sort_by(|a, b| a.name.cmp(&b.name));

            // Return early with failure
            return Ok(DoctorOutput {
                schema_version: "1".to_string(),
                emitted_at: Utc::now(),
                ok: false,
                checks,
                cache_stats: None,
            });
        }

        // 1. PATH & version checks - check based on configured provider
        let provider = self.config.llm.provider.as_deref().unwrap_or("claude-cli");

        match provider {
            "claude-cli" => {
                checks.push(self.check_claude_path());
                checks.push(self.check_claude_version());
            }
            "gemini-cli" => {
                checks.push(self.check_gemini_path());
                checks.push(self.check_gemini_help());
            }
            "openrouter" | "anthropic" => {
                // HTTP providers - check configuration without making HTTP calls
                // This is handled by check_llm_provider below
            }
            _ => {
                // Unknown provider - will be caught by check_llm_provider
            }
        }

        // 2. Runner selection & WSL
        checks.push(self.check_runner_selection());
        checks.push(self.check_wsl_availability());

        // On Windows, check WSL default distro and list all distros
        if cfg!(target_os = "windows") {
            checks.push(self.check_wsl_default_distro());
            checks.push(self.check_wsl_distros());
        }

        // 3. Write permissions
        checks.push(self.check_write_permissions());

        // 4. Same-volume atomic rename test
        checks.push(self.check_atomic_rename());

        // 5. Config parsing
        checks.push(self.check_config_parse());

        // 6. LLM provider validation
        checks.push(self.check_llm_provider());

        // Sort checks by name for stable output (required for JCS canonical emission)
        checks.sort_by(|a, b| a.name.cmp(&b.name));

        // Determine overall health
        let has_fail = checks.iter().any(|c| c.status == CheckStatus::Fail);
        let has_warn = checks.iter().any(|c| c.status == CheckStatus::Warn);
        let ok = !has_fail && (!strict_exit || !has_warn);

        // Get cache stats if cache is available (wired from InsightCache)
        let cache_stats = self.cache.as_ref().map(|c| *c.stats());

        // Log cache stats if available (wired into logging)
        if let Some(ref stats) = cache_stats {
            logging::log_cache_stats(stats);
        }

        Ok(DoctorOutput {
            schema_version: "1".to_string(),
            emitted_at: Utc::now(),
            ok,
            checks,
            cache_stats,
        })
    }

    /// Check if claude is in PATH
    fn check_claude_path(&self) -> DoctorCheck {
        if let Ok(path) = which::which("claude") {
            DoctorCheck {
                name: "claude_path".to_string(),
                status: CheckStatus::Pass,
                details: format!("Found claude at {}", path.display()),
            }
        } else {
            // On Windows, provide actionable suggestion if WSL is available
            #[cfg(target_os = "windows")]
            {
                if matches!(wsl::is_wsl_available(), Ok(true)) {
                    // Check if Claude is available in WSL
                    if matches!(wsl::validate_claude_in_wsl(None), Ok(true)) {
                        return DoctorCheck {
                            name: "claude_path".to_string(),
                            status: CheckStatus::Warn,
                            details: "Claude CLI not found in native PATH, but is available in WSL. Consider using --runner-mode wsl or --runner-mode auto".to_string(),
                        };
                    }
                }
            }

            DoctorCheck {
                name: "claude_path".to_string(),
                status: CheckStatus::Fail,
                details: "Claude CLI not found in PATH".to_string(),
            }
        }
    }

    /// Check claude version
    fn check_claude_version(&self) -> DoctorCheck {
        // Use CommandSpec for secure argv-style execution
        match CommandSpec::new("claude")
            .arg("--version")
            .to_command()
            .output()
        {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                DoctorCheck {
                    name: "claude_version".to_string(),
                    status: CheckStatus::Pass,
                    details: version,
                }
            }
            Ok(output) => DoctorCheck {
                name: "claude_version".to_string(),
                status: CheckStatus::Fail,
                details: format!(
                    "claude --version failed with exit code: {}",
                    output.status.code().unwrap_or(-1)
                ),
            },
            Err(e) => DoctorCheck {
                name: "claude_version".to_string(),
                status: CheckStatus::Fail,
                details: format!("Failed to execute claude --version: {e}"),
            },
        }
    }

    /// Check if gemini is in PATH (requirement 3.4.4)
    fn check_gemini_path(&self) -> DoctorCheck {
        if let Ok(path) = which::which("gemini") {
            DoctorCheck {
                name: "gemini_path".to_string(),
                status: CheckStatus::Pass,
                details: format!("Found gemini at {}", path.display()),
            }
        } else {
            DoctorCheck {
                name: "gemini_path".to_string(),
                status: CheckStatus::Fail,
                details: "Gemini CLI not found in PATH".to_string(),
            }
        }
    }

    /// Check gemini help (requirement 3.4.4 - use gemini -h to verify binary presence)
    /// Never sends a real completion request
    fn check_gemini_help(&self) -> DoctorCheck {
        // Use CommandSpec for secure argv-style execution
        match CommandSpec::new("gemini").arg("-h").to_command().output() {
            Ok(output) if output.status.success() => DoctorCheck {
                name: "gemini_help".to_string(),
                status: CheckStatus::Pass,
                details: "Gemini CLI responds to -h flag".to_string(),
            },
            Ok(output) => DoctorCheck {
                name: "gemini_help".to_string(),
                status: CheckStatus::Fail,
                details: format!(
                    "gemini -h failed with exit code: {}",
                    output.status.code().unwrap_or(-1)
                ),
            },
            Err(e) => DoctorCheck {
                name: "gemini_help".to_string(),
                status: CheckStatus::Fail,
                details: format!("Failed to execute gemini -h: {e}"),
            },
        }
    }

    /// Check runner selection
    fn check_runner_selection(&self) -> DoctorCheck {
        match self.config.get_runner_mode() {
            Ok(mode) => {
                let mode_str = match mode {
                    RunnerMode::Auto => "auto (will detect at runtime)",
                    RunnerMode::Native => "native (spawn claude directly)",
                    RunnerMode::Wsl => "wsl (use wsl.exe --exec)",
                };

                // Try to validate the runner
                let runner = Runner::new(
                    mode,
                    WslOptions {
                        distro: self.config.runner.distro.clone(),
                        claude_path: self.config.runner.claude_path.clone(),
                    },
                );

                match runner.validate() {
                    Ok(()) => DoctorCheck {
                        name: "runner_selection".to_string(),
                        status: CheckStatus::Pass,
                        details: format!("Runner mode: {mode_str}"),
                    },
                    Err(e) => DoctorCheck {
                        name: "runner_selection".to_string(),
                        status: CheckStatus::Fail,
                        details: format!("Runner validation failed: {e}"),
                    },
                }
            }
            Err(e) => DoctorCheck {
                name: "runner_selection".to_string(),
                status: CheckStatus::Fail,
                details: format!("Invalid runner mode: {e}"),
            },
        }
    }

    /// Check WSL availability (Windows only)
    fn check_wsl_availability(&self) -> DoctorCheck {
        if !cfg!(target_os = "windows") {
            return DoctorCheck {
                name: "wsl_availability".to_string(),
                status: CheckStatus::Pass,
                details: "WSL not applicable (not Windows)".to_string(),
            };
        }

        // Check if WSL is available
        match wsl::is_wsl_available() {
            Ok(true) => {
                // WSL is available, now check if Claude is installed
                match wsl::validate_claude_in_wsl(None) {
                    Ok(true) => DoctorCheck {
                        name: "wsl_availability".to_string(),
                        status: CheckStatus::Pass,
                        details: "WSL is available and Claude CLI is installed".to_string(),
                    },
                    Ok(false) => DoctorCheck {
                        name: "wsl_availability".to_string(),
                        status: CheckStatus::Warn,
                        details: "WSL is available but Claude CLI not found in WSL. Install Claude in WSL to use --runner-mode wsl".to_string(),
                    },
                    Err(e) => DoctorCheck {
                        name: "wsl_availability".to_string(),
                        status: CheckStatus::Warn,
                        details: format!("WSL is available but Claude check failed: {e}"),
                    },
                }
            }
            Ok(false) => DoctorCheck {
                name: "wsl_availability".to_string(),
                status: CheckStatus::Warn,
                details: "WSL not installed or no distributions available".to_string(),
            },
            Err(e) => DoctorCheck {
                name: "wsl_availability".to_string(),
                status: CheckStatus::Warn,
                details: format!("Failed to check WSL availability: {e}"),
            },
        }
    }

    /// Check WSL default distro (Windows only)
    fn check_wsl_default_distro(&self) -> DoctorCheck {
        if !cfg!(target_os = "windows") {
            return DoctorCheck {
                name: "wsl_default_distro".to_string(),
                status: CheckStatus::Pass,
                details: "WSL not applicable (not Windows)".to_string(),
            };
        }

        // Use CommandSpec for secure argv-style execution
        match CommandSpec::new("wsl")
            .args(["-l", "-v"])
            .to_command()
            .output()
        {
            Ok(output) if output.status.success() => {
                // Normalize WSL output (may be UTF-16LE on some Windows locales)
                let distros = Self::normalize_wsl_output(&output.stdout);

                // Parse the output to find the default distro (marked with *)
                let mut default_distro = None;
                for line in distros.lines() {
                    let line = line.trim();
                    if line.contains('*') {
                        // Extract distro name (format: "* Ubuntu-22.04  Running  2")
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 2 {
                            default_distro = Some(parts[1].to_string());
                            break;
                        }
                    }
                }

                match default_distro {
                    Some(distro) => {
                        // Check if Claude is available in this distro
                        match wsl::validate_claude_in_wsl(Some(&distro)) {
                            Ok(true) => DoctorCheck {
                                name: "wsl_default_distro".to_string(),
                                status: CheckStatus::Pass,
                                details: format!("Default WSL distro: {distro} (Claude available)"),
                            },
                            Ok(false) => DoctorCheck {
                                name: "wsl_default_distro".to_string(),
                                status: CheckStatus::Warn,
                                details: format!("Default WSL distro: {distro} (Claude not found)"),
                            },
                            Err(_) => DoctorCheck {
                                name: "wsl_default_distro".to_string(),
                                status: CheckStatus::Pass,
                                details: format!("Default WSL distro: {distro}"),
                            },
                        }
                    }
                    None => DoctorCheck {
                        name: "wsl_default_distro".to_string(),
                        status: CheckStatus::Warn,
                        details: "Could not determine default WSL distro".to_string(),
                    },
                }
            }
            Ok(_) => DoctorCheck {
                name: "wsl_default_distro".to_string(),
                status: CheckStatus::Warn,
                details: "wsl -l -v command failed".to_string(),
            },
            Err(e) => DoctorCheck {
                name: "wsl_default_distro".to_string(),
                status: CheckStatus::Warn,
                details: format!("Failed to execute wsl -l -v: {e}"),
            },
        }
    }

    /// Check all WSL distros and Claude availability (Windows only)
    fn check_wsl_distros(&self) -> DoctorCheck {
        if !cfg!(target_os = "windows") {
            return DoctorCheck {
                name: "wsl_distros".to_string(),
                status: CheckStatus::Pass,
                details: "WSL not applicable (not Windows)".to_string(),
            };
        }

        // Get list of all WSL distros using CommandSpec for secure argv-style execution
        match CommandSpec::new("wsl")
            .args(["-l", "-q"])
            .to_command()
            .output()
        {
            Ok(output) if output.status.success() => {
                match wsl::parse_distro_list(&output.stdout) {
                    Ok(distros) if !distros.is_empty() => {
                        let mut details_parts =
                            vec![format!("Found {} WSL distribution(s):", distros.len())];

                        for distro in &distros {
                            // Check if Claude is available in this distro
                            match wsl::validate_claude_in_wsl(Some(distro)) {
                                Ok(true) => {
                                    details_parts.push(format!("  - {distro} (Claude: ✓)"));
                                }
                                Ok(false) => {
                                    details_parts.push(format!("  - {distro} (Claude: ✗)"));
                                }
                                Err(_) => {
                                    details_parts.push(format!("  - {distro} (Claude: ?)"));
                                }
                            }
                        }

                        DoctorCheck {
                            name: "wsl_distros".to_string(),
                            status: CheckStatus::Pass,
                            details: details_parts.join("\n"),
                        }
                    }
                    Ok(_) => DoctorCheck {
                        name: "wsl_distros".to_string(),
                        status: CheckStatus::Warn,
                        details: "WSL is installed but no distributions found".to_string(),
                    },
                    Err(e) => DoctorCheck {
                        name: "wsl_distros".to_string(),
                        status: CheckStatus::Warn,
                        details: format!("Failed to parse WSL distro list: {e}"),
                    },
                }
            }
            Ok(_) => DoctorCheck {
                name: "wsl_distros".to_string(),
                status: CheckStatus::Warn,
                details: "wsl -l -q command failed".to_string(),
            },
            Err(_) => DoctorCheck {
                name: "wsl_distros".to_string(),
                status: CheckStatus::Warn,
                details: "WSL not installed or not available".to_string(),
            },
        }
    }

    /// Normalize WSL output which may be UTF-16LE on some Windows locales
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

    /// Check write permissions to .xchecker directory
    fn check_write_permissions(&self) -> DoctorCheck {
        let xchecker_dir = Path::new(".xchecker");

        // Try to create the directory if it doesn't exist (ignore benign races)
        if !xchecker_dir.exists() {
            match paths::ensure_dir_all(xchecker_dir) {
                Ok(()) => {
                    return DoctorCheck {
                        name: "write_permissions".to_string(),
                        status: CheckStatus::Pass,
                        details: "Created .xchecker directory successfully".to_string(),
                    };
                }
                Err(e) => {
                    return DoctorCheck {
                        name: "write_permissions".to_string(),
                        status: CheckStatus::Fail,
                        details: format!("Cannot create .xchecker directory: {e}"),
                    };
                }
            }
        }

        // Try to write a test file
        let test_file = xchecker_dir.join(".doctor_test");
        match std::fs::write(&test_file, "test") {
            Ok(()) => {
                // Clean up test file
                let _ = std::fs::remove_file(&test_file);
                DoctorCheck {
                    name: "write_permissions".to_string(),
                    status: CheckStatus::Pass,
                    details: ".xchecker directory is writable".to_string(),
                }
            }
            Err(e) => DoctorCheck {
                name: "write_permissions".to_string(),
                status: CheckStatus::Fail,
                details: format!("Cannot write to .xchecker directory: {e}"),
            },
        }
    }

    /// Check same-volume atomic rename capability
    fn check_atomic_rename(&self) -> DoctorCheck {
        let xchecker_dir = Path::new(".xchecker");

        // Ensure directory exists (ignore benign races)
        if let Err(e) = paths::ensure_dir_all(xchecker_dir) {
            return DoctorCheck {
                name: "atomic_rename".to_string(),
                status: CheckStatus::Fail,
                details: format!("Cannot create .xchecker directory: {e}"),
            };
        }

        // Create a test file
        let test_file = xchecker_dir.join(".doctor_rename_test");
        let test_target = xchecker_dir.join(".doctor_rename_target");

        match std::fs::write(&test_file, "test") {
            Ok(()) => {
                // Try atomic rename
                match std::fs::rename(&test_file, &test_target) {
                    Ok(()) => {
                        // Clean up
                        let _ = std::fs::remove_file(&test_target);
                        DoctorCheck {
                            name: "atomic_rename".to_string(),
                            status: CheckStatus::Pass,
                            details: "Atomic rename works on same volume".to_string(),
                        }
                    }
                    Err(e) => {
                        // Clean up
                        let _ = std::fs::remove_file(&test_file);
                        DoctorCheck {
                            name: "atomic_rename".to_string(),
                            status: CheckStatus::Fail,
                            details: format!("Atomic rename failed: {e}"),
                        }
                    }
                }
            }
            Err(e) => DoctorCheck {
                name: "atomic_rename".to_string(),
                status: CheckStatus::Fail,
                details: format!("Cannot create test file: {e}"),
            },
        }
    }

    /// Validate config parsing
    fn check_config_parse(&self) -> DoctorCheck {
        // Config is already parsed and validated in the constructor
        // If we got here, config parsing succeeded
        DoctorCheck {
            name: "config_parse".to_string(),
            status: CheckStatus::Pass,
            details: "Configuration parsed and validated successfully".to_string(),
        }
    }

    /// Check LLM provider configuration and binary discoverability
    fn check_llm_provider(&self) -> DoctorCheck {
        // 1. Check provider configuration
        let provider = self.config.llm.provider.as_deref().unwrap_or("claude-cli");

        // Validate that only supported providers are configured
        match provider {
            "claude-cli" => {
                // Supported in V11+
            }
            "gemini-cli" => {
                // Supported in V12+
            }
            "openrouter" => {
                // Supported in V13+ - HTTP provider
                return self.check_http_provider_config("openrouter");
            }
            "anthropic" => {
                // Supported in V14+ - HTTP provider
                return self.check_http_provider_config("anthropic");
            }
            unknown => {
                return DoctorCheck {
                    name: "llm_provider".to_string(),
                    status: CheckStatus::Fail,
                    details: format!(
                        "Unknown provider '{}'. Supported providers: claude-cli, gemini-cli, openrouter, anthropic",
                        unknown
                    ),
                };
            }
        }

        // 2. Check if a custom Claude binary path is configured
        let custom_binary = self
            .config
            .llm
            .claude
            .as_ref()
            .and_then(|c| c.binary.as_ref());

        if let Some(binary_path) = custom_binary {
            // Custom binary path specified - check if it exists
            let path = Path::new(binary_path);
            if path.exists() {
                return DoctorCheck {
                    name: "llm_provider".to_string(),
                    status: CheckStatus::Pass,
                    details: format!("Provider: claude-cli (custom binary at {})", binary_path),
                };
            } else {
                return DoctorCheck {
                    name: "llm_provider".to_string(),
                    status: CheckStatus::Fail,
                    details: format!(
                        "Custom Claude binary path '{}' does not exist. Please check [llm.claude] binary configuration",
                        binary_path
                    ),
                };
            }
        }

        // 3. No custom binary - check if Claude is discoverable in PATH
        #[cfg(target_os = "windows")]
        {
            // On Windows, use 'where' command
            match CommandSpec::new("where")
                .arg("claude")
                .to_command()
                .output()
            {
                Ok(output) if output.status.success() => {
                    let path = String::from_utf8_lossy(&output.stdout)
                        .lines()
                        .next()
                        .unwrap_or("unknown")
                        .trim()
                        .to_string();
                    DoctorCheck {
                        name: "llm_provider".to_string(),
                        status: CheckStatus::Pass,
                        details: format!("Provider: claude-cli (found at {})", path),
                    }
                }
                _ => {
                    // Claude not found in Windows PATH - check WSL
                    if matches!(wsl::is_wsl_available(), Ok(true))
                        && matches!(wsl::validate_claude_in_wsl(None), Ok(true))
                    {
                        return DoctorCheck {
                                name: "llm_provider".to_string(),
                                status: CheckStatus::Warn,
                                details: "Provider: claude-cli (not in native PATH, but available in WSL. Consider using --runner-mode wsl)".to_string(),
                            };
                    }

                    DoctorCheck {
                        name: "llm_provider".to_string(),
                        status: CheckStatus::Fail,
                        details: "Provider: claude-cli (binary not found in PATH or WSL. Install Claude CLI or specify path with --llm-claude-binary)".to_string(),
                    }
                }
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            // On Unix-like systems, use 'which' command
            match CommandSpec::new("which").arg("claude").to_command().output() {
                Ok(output) if output.status.success() => {
                    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    DoctorCheck {
                        name: "llm_provider".to_string(),
                        status: CheckStatus::Pass,
                        details: format!("Provider: claude-cli (found at {})", path),
                    }
                }
                _ => DoctorCheck {
                    name: "llm_provider".to_string(),
                    status: CheckStatus::Fail,
                    details:
                        "Provider: claude-cli (binary not found in PATH. Install Claude CLI or specify path with --llm-claude-binary)"
                            .to_string(),
                },
            }
        }
    }

    /// Check HTTP provider configuration (requirement 3.5.3)
    ///
    /// For HTTP providers:
    /// - Check configured env vars are present
    /// - Never make HTTP calls by default
    /// - Report clear status for each HTTP provider
    fn check_http_provider_config(&self, provider: &str) -> DoctorCheck {
        match provider {
            "openrouter" => {
                // Get the API key environment variable name from config
                let api_key_env = self
                    .config
                    .llm
                    .openrouter
                    .as_ref()
                    .and_then(|or| or.api_key_env.as_deref())
                    .unwrap_or("OPENROUTER_API_KEY");

                // Check if the environment variable is present (requirement 3.5.3)
                // Never read the actual value to avoid logging secrets
                match std::env::var(api_key_env) {
                    Ok(_) => {
                        // API key is present - check if model is configured
                        let model = self
                            .config
                            .llm
                            .openrouter
                            .as_ref()
                            .and_then(|or| or.model.as_ref());

                        match model {
                            Some(model_name) => DoctorCheck {
                                name: "llm_provider".to_string(),
                                status: CheckStatus::Pass,
                                details: format!(
                                    "Provider: openrouter (API key present in {}, model: {})",
                                    api_key_env, model_name
                                ),
                            },
                            None => DoctorCheck {
                                name: "llm_provider".to_string(),
                                status: CheckStatus::Fail,
                                details: format!(
                                    "Provider: openrouter (API key present in {}, but model not configured. Set [llm.openrouter] model = \"model-name\")",
                                    api_key_env
                                ),
                            },
                        }
                    }
                    Err(_) => DoctorCheck {
                        name: "llm_provider".to_string(),
                        status: CheckStatus::Fail,
                        details: format!(
                            "Provider: openrouter (API key not found in environment variable '{}'. Set this variable or configure api_key_env in [llm.openrouter])",
                            api_key_env
                        ),
                    },
                }
            }
            "anthropic" => {
                // Get the API key environment variable name from config
                let api_key_env = self
                    .config
                    .llm
                    .anthropic
                    .as_ref()
                    .and_then(|a| a.api_key_env.as_deref())
                    .unwrap_or("ANTHROPIC_API_KEY");

                // Check if the environment variable is present (requirement 3.5.3)
                // Never read the actual value to avoid logging secrets
                match std::env::var(api_key_env) {
                    Ok(_) => {
                        // API key is present - check if model is configured
                        let model = self
                            .config
                            .llm
                            .anthropic
                            .as_ref()
                            .and_then(|a| a.model.as_ref());

                        match model {
                            Some(model_name) => DoctorCheck {
                                name: "llm_provider".to_string(),
                                status: CheckStatus::Pass,
                                details: format!(
                                    "Provider: anthropic (API key present in {}, model: {})",
                                    api_key_env, model_name
                                ),
                            },
                            None => DoctorCheck {
                                name: "llm_provider".to_string(),
                                status: CheckStatus::Fail,
                                details: format!(
                                    "Provider: anthropic (API key present in {}, but model not configured. Set [llm.anthropic] model = \"model-name\")",
                                    api_key_env
                                ),
                            },
                        }
                    }
                    Err(_) => DoctorCheck {
                        name: "llm_provider".to_string(),
                        status: CheckStatus::Fail,
                        details: format!(
                            "Provider: anthropic (API key not found in environment variable '{}'. Set this variable or configure api_key_env in [llm.anthropic])",
                            api_key_env
                        ),
                    },
                }
            }
            _ => DoctorCheck {
                name: "llm_provider".to_string(),
                status: CheckStatus::Fail,
                details: format!("Unknown HTTP provider: {}", provider),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xchecker_config::CliArgs;

    #[test]
    fn test_doctor_output_structure() {
        let cli_args = CliArgs::default();
        let config = Config::discover(&cli_args).unwrap();
        let mut doctor = DoctorCommand::new(config);

        let output = doctor.run_with_options().unwrap();

        assert_eq!(output.schema_version, "1");
        assert!(!output.checks.is_empty());

        // Verify checks are sorted by name
        let names: Vec<String> = output.checks.iter().map(|c| c.name.clone()).collect();
        let mut sorted_names = names.clone();
        sorted_names.sort();
        assert_eq!(names, sorted_names, "Checks should be sorted by name");
    }

    #[test]
    fn test_checks_sorted_lexicographically() {
        // Create unsorted checks
        let mut checks = [
            DoctorCheck {
                name: "zebra".to_string(),
                status: CheckStatus::Pass,
                details: "test".to_string(),
            },
            DoctorCheck {
                name: "alpha".to_string(),
                status: CheckStatus::Pass,
                details: "test".to_string(),
            },
            DoctorCheck {
                name: "middle".to_string(),
                status: CheckStatus::Pass,
                details: "test".to_string(),
            },
        ];

        // Sort as the run() method does
        checks.sort_by(|a, b| a.name.cmp(&b.name));

        // Verify lexicographic order
        assert_eq!(checks[0].name, "alpha");
        assert_eq!(checks[1].name, "middle");
        assert_eq!(checks[2].name, "zebra");
    }

    #[test]
    fn test_check_status_serialization() {
        let check = DoctorCheck {
            name: "test".to_string(),
            status: CheckStatus::Pass,
            details: "test details".to_string(),
        };

        let json = serde_json::to_string(&check).unwrap();
        assert!(json.contains("\"status\":\"pass\""));

        // Test all status variants
        let pass_check = DoctorCheck {
            name: "test".to_string(),
            status: CheckStatus::Pass,
            details: "test".to_string(),
        };
        let warn_check = DoctorCheck {
            name: "test".to_string(),
            status: CheckStatus::Warn,
            details: "test".to_string(),
        };
        let fail_check = DoctorCheck {
            name: "test".to_string(),
            status: CheckStatus::Fail,
            details: "test".to_string(),
        };

        assert!(
            serde_json::to_string(&pass_check)
                .unwrap()
                .contains("\"pass\"")
        );
        assert!(
            serde_json::to_string(&warn_check)
                .unwrap()
                .contains("\"warn\"")
        );
        assert!(
            serde_json::to_string(&fail_check)
                .unwrap()
                .contains("\"fail\"")
        );
    }

    #[test]
    fn test_strict_exit_mode() {
        let cli_args = CliArgs::default();
        let config = Config::discover(&cli_args).unwrap();
        let mut doctor = DoctorCommand::new(config);

        // Run doctor checks
        let output_normal = doctor.run_with_options().unwrap();

        // If there are any warnings but no failures, strict should be false
        let has_warn = output_normal
            .checks
            .iter()
            .any(|c| c.status == CheckStatus::Warn);
        let has_fail = output_normal
            .checks
            .iter()
            .any(|c| c.status == CheckStatus::Fail);

        if has_warn && !has_fail {
            assert!(
                output_normal.ok,
                "Doctor should be ok with only warnings (no failures)"
            );
        }
    }

    #[test]
    fn test_write_permissions_check() {
        let cli_args = CliArgs::default();
        let config = Config::discover(&cli_args).unwrap();
        let doctor = DoctorCommand::new(config);

        let check = doctor.check_write_permissions();
        assert_eq!(check.name, "write_permissions");
        // Status depends on actual permissions, so we just verify the check runs
    }

    #[test]
    fn test_atomic_rename_check() {
        let cli_args = CliArgs::default();
        let config = Config::discover(&cli_args).unwrap();
        let doctor = DoctorCommand::new(config);

        let check = doctor.check_atomic_rename();
        assert_eq!(check.name, "atomic_rename");
        // Status depends on filesystem capabilities
    }

    #[test]
    fn test_config_parse_check() {
        let cli_args = CliArgs::default();
        let config = Config::discover(&cli_args).unwrap();
        let doctor = DoctorCommand::new(config);

        let check = doctor.check_config_parse();
        assert_eq!(check.name, "config_parse");
        assert_eq!(check.status, CheckStatus::Pass);
    }

    #[test]
    fn test_wsl_output_normalization_utf8() {
        let utf8_bytes = b"Ubuntu\n";
        let result = DoctorCommand::normalize_wsl_output(utf8_bytes);
        assert_eq!(result, "Ubuntu\n");
    }

    #[test]
    fn test_wsl_output_normalization_utf16le() {
        // "Ubuntu" in UTF-16LE
        let utf16le_bytes: Vec<u8> = vec![
            0x55, 0x00, // U
            0x62, 0x00, // b
            0x75, 0x00, // u
            0x6E, 0x00, // n
            0x74, 0x00, // t
            0x75, 0x00, // u
        ];
        let result = DoctorCommand::normalize_wsl_output(&utf16le_bytes);
        assert_eq!(result, "Ubuntu");
    }

    #[test]
    fn test_llm_provider_check() {
        let cli_args = CliArgs::default();
        let config = Config::discover(&cli_args).unwrap();
        let doctor = DoctorCommand::new(config);

        let check = doctor.check_llm_provider();
        assert_eq!(check.name, "llm_provider");
        // Status depends on whether Claude is installed, but check should run
    }

    #[test]
    fn test_llm_provider_with_custom_binary() {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let temp_path = temp_file.path().to_str().unwrap().to_string();

        let cli_args = CliArgs {
            llm_claude_binary: Some(temp_path.clone()),
            ..Default::default()
        };

        let config = Config::discover(&cli_args).unwrap();
        let doctor = DoctorCommand::new(config);

        let check = doctor.check_llm_provider();
        assert_eq!(check.name, "llm_provider");
        assert_eq!(check.status, CheckStatus::Pass);
        assert!(check.details.contains(&temp_path));
    }

    #[test]
    fn test_llm_provider_with_invalid_custom_binary() {
        let cli_args = CliArgs {
            llm_claude_binary: Some("/nonexistent/path/to/claude".to_string()),
            ..Default::default()
        };

        let config = Config::discover(&cli_args).unwrap();
        let doctor = DoctorCommand::new(config);

        let check = doctor.check_llm_provider();
        assert_eq!(check.name, "llm_provider");
        assert_eq!(check.status, CheckStatus::Fail);
        assert!(check.details.contains("does not exist"));
    }

    #[test]
    fn test_llm_provider_included_in_doctor_output() {
        let cli_args = CliArgs::default();
        let config = Config::discover(&cli_args).unwrap();
        let mut doctor = DoctorCommand::new(config);

        let output = doctor.run_with_options().unwrap();

        // Verify llm_provider check is included in the output
        let llm_check = output.checks.iter().find(|c| c.name == "llm_provider");

        assert!(
            llm_check.is_some(),
            "llm_provider check should be included in doctor output"
        );
    }

    #[test]
    fn test_json_output_byte_identical_regardless_of_insertion_order() {
        // Create two outputs with checks in different insertion orders
        let checks1 = vec![
            DoctorCheck {
                name: "zebra".to_string(),
                status: CheckStatus::Pass,
                details: "test".to_string(),
            },
            DoctorCheck {
                name: "alpha".to_string(),
                status: CheckStatus::Pass,
                details: "test".to_string(),
            },
        ];

        let checks2 = vec![
            DoctorCheck {
                name: "alpha".to_string(),
                status: CheckStatus::Pass,
                details: "test".to_string(),
            },
            DoctorCheck {
                name: "zebra".to_string(),
                status: CheckStatus::Pass,
                details: "test".to_string(),
            },
        ];

        let mut output1 = DoctorOutput {
            schema_version: "1".to_string(),
            emitted_at: Utc::now(),
            ok: true,
            checks: checks1,
            cache_stats: None,
        };

        let mut output2 = DoctorOutput {
            schema_version: "1".to_string(),
            emitted_at: output1.emitted_at, // Use same timestamp
            ok: true,
            checks: checks2,
            cache_stats: None,
        };

        // Sort both (as run() does)
        output1.checks.sort_by(|a, b| a.name.cmp(&b.name));
        output2.checks.sort_by(|a, b| a.name.cmp(&b.name));

        // Serialize both
        let json1 = serde_json::to_string(&output1).unwrap();
        let json2 = serde_json::to_string(&output2).unwrap();

        // Should be byte-identical
        assert_eq!(
            json1, json2,
            "Different insertion orders should produce identical JSON after sorting"
        );
    }
}

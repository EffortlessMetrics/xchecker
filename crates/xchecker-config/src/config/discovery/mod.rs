mod cli_overrides;
mod file_loader;
mod file_values;
mod llm_overrides;
mod state;

use std::path::{Path, PathBuf};

use crate::error::{ConfigError, XCheckerError};

use super::{CliArgs, Config};

use self::cli_overrides::apply_cli_overrides;
use self::file_loader::{discover_config_file_from, load_config_file};
use self::file_values::apply_file_config;
use self::llm_overrides::apply_llm_env_cli_overrides_and_defaults;
use self::state::DiscoveryState;

impl Config {
    /// Discover and load configuration with precedence: CLI > file > defaults.
    ///
    /// Uses current working directory for config file discovery when no explicit
    /// path is provided in cli_args.
    pub fn discover(cli_args: &CliArgs) -> Result<Self, XCheckerError> {
        let start_dir = std::env::current_dir().map_err(|e| {
            XCheckerError::Config(ConfigError::DiscoveryFailed {
                reason: format!("Failed to get current directory: {e}"),
            })
        })?;
        Self::discover_from(&start_dir, cli_args)
    }

    /// Discover and load configuration starting from a specific directory.
    ///
    /// This path-driven variant is used by tests to avoid process-global state.
    /// Uses the given directory for config file discovery when no explicit path
    /// is provided in cli_args.
    pub fn discover_from(start_dir: &Path, cli_args: &CliArgs) -> Result<Self, XCheckerError> {
        let mut state = DiscoveryState::with_defaults();

        if let Some(path) = Self::config_path_for(start_dir, cli_args)? {
            let file_config = load_config_file(&path)?;
            apply_file_config(&mut state, file_config);
        }

        apply_cli_overrides(&mut state, cli_args);
        apply_llm_env_cli_overrides_and_defaults(&mut state, cli_args);

        let config = state.into_config();
        config.validate()?;

        Ok(config)
    }

    fn config_path_for(
        start_dir: &Path,
        cli_args: &CliArgs,
    ) -> Result<Option<PathBuf>, XCheckerError> {
        if let Some(explicit_path) = &cli_args.config_path {
            Ok(Some(explicit_path.clone()))
        } else {
            discover_config_file_from(start_dir)
        }
    }

    /// Discover config file by searching upward from a given directory.
    ///
    /// This is the path-driven variant used by tests to avoid process-global state.
    /// Walks up the directory tree looking for `.xchecker/config.toml`, stopping
    /// at repository root markers (.git, .hg, .svn) or filesystem root.
    pub fn discover_config_file_from(start_dir: &Path) -> Result<Option<PathBuf>, XCheckerError> {
        discover_config_file_from(start_dir)
    }

    /// Discover configuration from environment and filesystem.
    ///
    /// This method uses the same discovery logic as the CLI:
    /// - `XCHECKER_HOME` environment variable (if set)
    /// - Upward search for `.xchecker/config.toml` from current directory
    /// - Built-in defaults
    ///
    /// Precedence: config file > defaults
    ///
    /// This is the recommended method for library consumers who want CLI-like
    /// behavior without needing to construct `CliArgs`.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use xchecker_config::Config;
    ///
    /// let config = Config::discover_from_env_and_fs()
    ///     .expect("Failed to discover config");
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The current directory cannot be determined
    /// - A config file exists but cannot be parsed
    /// - Configuration validation fails
    pub fn discover_from_env_and_fs() -> Result<Self, XCheckerError> {
        let cli_args = CliArgs::default();
        Self::discover(&cli_args)
    }
}

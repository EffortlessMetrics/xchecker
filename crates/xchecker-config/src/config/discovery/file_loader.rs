use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{ConfigError, XCheckerError};

use super::super::{
    Defaults, HooksConfig, LlmConfig, PhasesConfig, RunnerConfig, SecurityConfig, Selectors,
};

/// TOML configuration file structure.
#[derive(Debug, Deserialize, Serialize)]
pub(super) struct TomlConfig {
    pub(super) defaults: Option<Defaults>,
    pub(super) selectors: Option<Selectors>,
    pub(super) runner: Option<RunnerConfig>,
    pub(super) llm: Option<LlmConfig>,
    pub(super) phases: Option<PhasesConfig>,
    pub(super) hooks: Option<HooksConfig>,
    pub(super) security: Option<SecurityConfig>,
}

impl TomlConfig {
    fn empty() -> Self {
        Self {
            defaults: None,
            selectors: None,
            runner: None,
            llm: None,
            phases: None,
            hooks: None,
            security: None,
        }
    }
}

pub(super) fn discover_config_file_from(
    start_dir: &Path,
) -> Result<Option<PathBuf>, XCheckerError> {
    let mut current_dir = start_dir.to_path_buf();

    loop {
        let config_path = current_dir.join(".xchecker").join("config.toml");
        if config_path.exists() {
            return Ok(Some(config_path));
        }

        if current_dir.parent().is_none() {
            break;
        }

        if current_dir.join(".git").exists()
            || current_dir.join(".hg").exists()
            || current_dir.join(".svn").exists()
        {
            break;
        }

        current_dir = current_dir.parent().unwrap().to_path_buf();
    }

    Ok(None)
}

pub(super) fn load_config_file(path: &Path) -> Result<TomlConfig, XCheckerError> {
    match std::fs::read_to_string(path) {
        Ok(content) => toml::from_str(&content).map_err(|e| {
            XCheckerError::Config(ConfigError::InvalidFile(format!(
                "Failed to parse TOML config file {}: {e}",
                path.display()
            )))
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(TomlConfig::empty()),
        Err(e) => Err(XCheckerError::Config(ConfigError::DiscoveryFailed {
            reason: format!("Failed to read config file {}: {}", path.display(), e),
        })),
    }
}

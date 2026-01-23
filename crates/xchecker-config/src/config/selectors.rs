use globset::Glob;

use crate::error::{ConfigError, XCheckerError};

use super::Selectors;

/// High-confidence secret file patterns that are always excluded from packet building.
///
/// These patterns target files that almost certainly contain secrets (private keys,
/// environment files, certificates). They are enforced at multiple layers:
/// 1. Default config selectors (this module)
/// 2. Engine-level enforcement in `ContentSelector` (defense-in-depth)
///
/// # Patterns
///
/// - `.env` and `.env.*` - Environment variable files
/// - `*.pem`, `*.pfx`, `*.p12` - Certificate/key files
/// - `id_rsa`, `id_ed25519` - SSH private keys
/// - `.ssh/**` - SSH configuration directory
pub const ALWAYS_EXCLUDE_PATTERNS: &[&str] = &[
    "**/.env",
    "**/.env.*",
    "**/*.pem",
    "**/id_rsa",
    "**/id_ed25519",
    "**/.ssh/**",
    "**/*.pfx",
    "**/*.p12",
];

impl Default for Selectors {
    fn default() -> Self {
        let mut exclude = vec![
            "target/**".to_string(),
            "node_modules/**".to_string(),
            ".git/**".to_string(),
            "**/.DS_Store".to_string(),
        ];

        // Add mandatory security exclusions
        exclude.extend(ALWAYS_EXCLUDE_PATTERNS.iter().map(|s| (*s).to_string()));

        Self {
            include: vec![
                "docs/**/SPEC*.md".to_string(),
                "docs/**/ADR*.md".to_string(),
                "README.md".to_string(),
                "SCHEMASET.*".to_string(),
                "**/Cargo.toml".to_string(),
                "**/*.core.yaml".to_string(),
            ],
            exclude,
        }
    }
}

impl Selectors {
    pub(crate) fn validate(&self) -> Result<(), XCheckerError> {
        // Validate glob patterns in selectors
        for pattern in &self.include {
            Glob::new(pattern).map_err(|e| {
                XCheckerError::Config(ConfigError::InvalidValue {
                    key: "selectors.include".to_string(),
                    value: format!("Invalid glob pattern '{pattern}': {e}"),
                })
            })?;
        }

        for pattern in &self.exclude {
            Glob::new(pattern).map_err(|e| {
                XCheckerError::Config(ConfigError::InvalidValue {
                    key: "selectors.exclude".to_string(),
                    value: format!("Invalid glob pattern '{pattern}': {e}"),
                })
            })?;
        }

        Ok(())
    }
}

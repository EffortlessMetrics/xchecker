//! Orchestrator façade for external consumers.
//!
//! This module provides a clean, stable API for external consumers (CLI, Kiro, MCP tools)
//! to interact with the phase orchestrator without needing to know internal details.
//!
//! **Integration rule**: Outside `src/orchestrator/`, use `OrchestratorHandle`.
//! Direct `PhaseOrchestrator` usage is reserved for tests and orchestrator internals.
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use xchecker::{OrchestratorHandle, PhaseId};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Using environment-based config discovery
//!     let mut handle = OrchestratorHandle::new("my-spec")?;
//!     handle.run_phase(PhaseId::Requirements).await?;
//!     Ok(())
//! }
//! ```

use std::path::PathBuf;

use anyhow::Result;

use crate::artifact::ArtifactManager;
use crate::config::{CliArgs, Config};
use crate::error::{ConfigError, XCheckerError};
use crate::receipt::ReceiptManager;
use crate::spec_id::sanitize_spec_id;
use crate::status::StatusManager;
use crate::types::{PhaseId, StatusOutput};

use super::{ExecutionResult, OrchestratorConfig, PhaseOrchestrator};

/// The primary public API for embedding xchecker.
///
/// `OrchestratorHandle` provides a stable interface for creating specs and running
/// phases programmatically. It is the canonical way to use xchecker outside of the CLI.
///
/// # Overview
///
/// Use `OrchestratorHandle` to:
/// - Create and manage specs programmatically
/// - Execute individual phases or the full workflow
/// - Query spec status and artifacts
/// - Configure execution options
///
/// # Construction
///
/// There are two ways to create an `OrchestratorHandle`:
///
/// - [`OrchestratorHandle::new`]: Uses environment-based config discovery (same as CLI)
/// - [`OrchestratorHandle::from_config`]: Uses explicit configuration (deterministic)
///
/// # Threading
///
/// `OrchestratorHandle` is **NOT** guaranteed `Send` or `Sync` in 1.x.
/// Treat as single-threaded; concurrent use is undefined behavior.
/// This may be relaxed in future versions.
///
/// # Mutability
///
/// Methods that execute phases take `&mut self` to encode "sequential use only"
/// semantics. This prevents accidental concurrent use at compile time.
///
/// # Sync vs Async
///
/// Public APIs are synchronous and manage their own async runtime internally.
/// Tokio is an implementation detail not exposed to library consumers.
///
/// # Example
///
/// ```rust,no_run
/// use xchecker::{OrchestratorHandle, PhaseId};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Using environment-based config discovery
///     let mut handle = OrchestratorHandle::new("my-spec")?;
///     
///     // Run a single phase
///     handle.run_phase(PhaseId::Requirements).await?;
///
///     // Check status
///     let status = handle.status()?;
///     println!("Artifacts: {}", status.artifacts.len());
///     
///     // Get the spec ID
///     println!("Spec: {}", handle.spec_id());
///     Ok(())
/// }
/// ```
///
/// # Using Explicit Configuration
///
/// ```rust,no_run
/// use xchecker::{OrchestratorHandle, Config};
///
/// // Create explicit config programmatically
/// let config = Config::discover(&Default::default())?;
/// let handle = OrchestratorHandle::from_config("my-spec", config)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Error Handling
///
/// All methods return `Result` types. Errors are returned as [`XCheckerError`](crate::XCheckerError)
/// which provides:
/// - Rich context about what went wrong
/// - Actionable suggestions for resolution
/// - Mapping to CLI exit codes via [`to_exit_code()`](crate::XCheckerError::to_exit_code)
pub struct OrchestratorHandle {
    orchestrator: PhaseOrchestrator,
    config: OrchestratorConfig,
    spec_id: String,
}

impl OrchestratorHandle {
    /// Create a handle using environment-based config discovery.
    ///
    /// This uses the same discovery logic as the CLI:
    /// - `XCHECKER_HOME` environment variable
    /// - Upward search for `.xchecker/config.toml`
    /// - Built-in defaults
    ///
    /// Acquires an exclusive lock on the spec directory.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Configuration discovery fails
    /// - Orchestrator creation fails
    /// - Lock cannot be acquired
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use xchecker::OrchestratorHandle;
    ///
    /// let handle = OrchestratorHandle::new("my-spec")?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn new(spec_id: &str) -> Result<Self, XCheckerError> {
        // Use environment-based config discovery (same as CLI)
        let config = Config::discover(&CliArgs::default()).map_err(|e| {
            XCheckerError::Config(crate::error::ConfigError::DiscoveryFailed {
                reason: e.to_string(),
            })
        })?;

        Self::from_config_internal(spec_id, config, false)
    }

    /// Create a handle using explicit configuration.
    ///
    /// This does NOT probe the global environment or filesystem for config.
    /// Use this when you need deterministic behavior independent of the
    /// user's environment.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Orchestrator creation fails
    /// - Lock cannot be acquired
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use xchecker::{OrchestratorHandle, Config};
    ///
    /// // Create explicit config programmatically
    /// let config = Config::discover(&Default::default())?;
    /// let handle = OrchestratorHandle::from_config("my-spec", config)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn from_config(spec_id: &str, config: Config) -> Result<Self, XCheckerError> {
        Self::from_config_internal(spec_id, config, false)
    }

    /// Internal constructor that converts Config to OrchestratorConfig
    fn from_config_internal(
        spec_id: &str,
        config: Config,
        force: bool,
    ) -> Result<Self, XCheckerError> {
        // Sanitize spec ID to prevent path traversal and invalid characters
        let sanitized_id = sanitize_spec_id(spec_id).map_err(|e| {
            XCheckerError::Config(ConfigError::InvalidValue {
                key: "spec_id".to_string(),
                value: e.to_string(),
            })
        })?;

        let redactor = crate::redaction::SecretRedactor::from_config(&config).map_err(|e| {
            XCheckerError::Config(ConfigError::InvalidValue {
                key: "security".to_string(),
                value: e.to_string(),
            })
        })?;

        let orchestrator = if force {
            PhaseOrchestrator::new_with_force(&sanitized_id, true)
        } else {
            PhaseOrchestrator::new(&sanitized_id)
        }
        .map_err(|e| {
            XCheckerError::Config(crate::error::ConfigError::DiscoveryFailed {
                reason: e.to_string(),
            })
        })?;

        // Convert Config to OrchestratorConfig
        let mut orch_config = OrchestratorConfig {
            redactor: std::sync::Arc::new(redactor),
            ..Default::default()
        };

        // Apply config values to orchestrator config
        if let Some(model) = &config.defaults.model {
            orch_config
                .config
                .insert("model".to_string(), model.clone());
        }
        if let Some(timeout) = config.defaults.phase_timeout {
            orch_config
                .config
                .insert("phase_timeout".to_string(), timeout.to_string());
        }
        if let Some(strict) = config.defaults.strict_validation {
            orch_config.strict_validation = strict;
        }

        // Copy selectors
        orch_config.selectors = Some(config.selectors.clone());

        Ok(Self {
            orchestrator,
            config: orch_config,
            spec_id: sanitized_id,
        })
    }

    /// Create a handle with force flag for lock override.
    ///
    /// Use with caution: forcing lock override can lead to race conditions if another
    /// process is actively working on the spec.
    ///
    /// # Errors
    ///
    /// Returns error if orchestrator creation fails.
    pub fn with_force(spec_id: &str, force: bool) -> Result<Self, XCheckerError> {
        let config = Config::discover(&CliArgs::default()).map_err(|e| {
            XCheckerError::Config(crate::error::ConfigError::DiscoveryFailed {
                reason: e.to_string(),
            })
        })?;

        Self::from_config_internal(spec_id, config, force)
    }

    /// Create a handle with custom OrchestratorConfig and force flag.
    ///
    /// This is used by the CLI when it needs to pass specific orchestrator
    /// configuration options.
    ///
    /// # Errors
    ///
    /// Returns error if orchestrator creation fails.
    pub fn with_config_and_force(
        spec_id: &str,
        config: OrchestratorConfig,
        force: bool,
    ) -> Result<Self, XCheckerError> {
        // Sanitize spec ID
        let sanitized_id = sanitize_spec_id(spec_id).map_err(|e| {
            XCheckerError::Config(ConfigError::InvalidValue {
                key: "spec_id".to_string(),
                value: e.to_string(),
            })
        })?;

        let orchestrator = if force {
            PhaseOrchestrator::new_with_force(&sanitized_id, true)
        } else {
            PhaseOrchestrator::new(&sanitized_id)
        }
        .map_err(|e| {
            XCheckerError::Config(crate::error::ConfigError::DiscoveryFailed {
                reason: e.to_string(),
            })
        })?;

        Ok(Self {
            orchestrator,
            config,
            spec_id: sanitized_id,
        })
    }

    /// Create a read-only handle for status inspection.
    ///
    /// Does not acquire locks, allowing inspection while another process
    /// is actively working on the spec.
    ///
    /// # Errors
    ///
    /// Returns error if orchestrator creation fails.
    pub fn readonly(spec_id: &str) -> Result<Self, XCheckerError> {
        // Sanitize spec ID
        let sanitized_id = sanitize_spec_id(spec_id).map_err(|e| {
            XCheckerError::Config(ConfigError::InvalidValue {
                key: "spec_id".to_string(),
                value: e.to_string(),
            })
        })?;

        let orchestrator = PhaseOrchestrator::new_readonly(&sanitized_id).map_err(|e| {
            XCheckerError::Config(crate::error::ConfigError::DiscoveryFailed {
                reason: e.to_string(),
            })
        })?;

        let config = OrchestratorConfig::default();

        Ok(Self {
            orchestrator,
            config,
            spec_id: sanitized_id,
        })
    }

    /// Execute a single phase.
    ///
    /// Behavior matches the CLI `xchecker resume --phase <phase>` command.
    /// Takes `&mut self` to enforce sequential use.
    ///
    /// # Errors
    ///
    /// Returns error if transition is invalid or execution fails.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use xchecker::{OrchestratorHandle, PhaseId};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut handle = OrchestratorHandle::new("my-spec")?;
    /// handle.run_phase(PhaseId::Requirements).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn run_phase(&mut self, phase: PhaseId) -> Result<ExecutionResult> {
        self.orchestrator
            .resume_from_phase(phase, &self.config)
            .await
    }

    /// Execute all phases in sequence.
    ///
    /// Stops on first failure. Behavior matches the CLI `xchecker spec` command.
    /// Takes `&mut self` to enforce sequential use.
    ///
    /// # Errors
    ///
    /// Returns error if any phase fails.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use xchecker::OrchestratorHandle;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut handle = OrchestratorHandle::new("my-spec")?;
    /// handle.run_all().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn run_all(&mut self) -> Result<ExecutionResult> {
        // Execute phases in sequence: Requirements -> Design -> Tasks
        // (Review, Fixup, Final are optional/advanced phases)
        let phases = [PhaseId::Requirements, PhaseId::Design, PhaseId::Tasks];

        let mut last_result = None;
        for phase in phases {
            let result = self
                .orchestrator
                .resume_from_phase(phase, &self.config)
                .await?;

            if !result.success {
                return Ok(result);
            }
            last_result = Some(result);
        }

        // Return the last successful result
        last_result.ok_or_else(|| anyhow::anyhow!("No phases executed"))
    }

    /// Get the current spec status.
    ///
    /// Returns `StatusOutput` which is part of the stable public API.
    ///
    /// # Errors
    ///
    /// Returns error if status generation fails.
    pub fn status(&self) -> Result<StatusOutput, XCheckerError> {
        use std::collections::BTreeMap;

        // Build effective config from orchestrator config
        let mut effective_config: BTreeMap<String, (String, String)> = BTreeMap::new();
        for (key, value) in &self.config.config {
            effective_config.insert(key.clone(), (value.clone(), "programmatic".to_string()));
        }

        StatusManager::generate_status_from_orchestrator(
            &self.orchestrator,
            effective_config,
            None, // lock_drift - not tracked in handle
            None, // pending_fixups - not tracked in handle
            Some(&self.config.redactor),
        )
        .map_err(|e| {
            XCheckerError::Config(crate::error::ConfigError::DiscoveryFailed {
                reason: format!("Failed to generate status: {}", e),
            })
        })
    }

    /// Get the path to the most recent receipt.
    ///
    /// Returns `None` if no receipts have been written.
    #[must_use]
    pub fn last_receipt_path(&self) -> Option<PathBuf> {
        // Check each phase in reverse order to find the most recent receipt
        let phases = [
            PhaseId::Final,
            PhaseId::Fixup,
            PhaseId::Review,
            PhaseId::Tasks,
            PhaseId::Design,
            PhaseId::Requirements,
        ];

        for phase in &phases {
            if let Ok(Some(_receipt)) = self
                .orchestrator
                .receipt_manager()
                .read_latest_receipt(*phase)
            {
                // Construct the receipt path from the receipt manager's base path
                let base_path = self.orchestrator.artifact_manager().base_path();
                let receipts_dir = base_path.join("receipts");

                // Find the most recent receipt file for this phase
                if let Ok(entries) = std::fs::read_dir(&receipts_dir) {
                    let phase_prefix = format!("{}-", phase.as_str());
                    let mut receipt_files: Vec<_> = entries
                        .filter_map(|e| e.ok())
                        .filter(|e| e.file_name().to_string_lossy().starts_with(&phase_prefix))
                        .collect();

                    // Sort by name (timestamp-based) to get the most recent
                    receipt_files.sort_by_key(|b| std::cmp::Reverse(b.file_name()));

                    if let Some(entry) = receipt_files.first() {
                        return Some(entry.path());
                    }
                }
            }
        }

        None
    }

    /// Get the spec ID this handle operates on.
    #[must_use]
    pub fn spec_id(&self) -> &str {
        &self.spec_id
    }

    /// Check if a phase can be run.
    ///
    /// Validates that all dependencies are satisfied and have successful receipts.
    ///
    /// # Returns
    ///
    /// `true` if the phase can be executed, `false` otherwise.
    pub fn can_run_phase(&self, phase: PhaseId) -> Result<bool> {
        self.orchestrator.can_resume_from_phase_public(phase)
    }

    /// Get the current phase state.
    ///
    /// Returns the last successfully completed phase, or `None` if no phases
    /// have been completed.
    pub fn current_phase(&self) -> Result<Option<PhaseId>> {
        self.orchestrator.get_current_phase_state()
    }

    /// Get legal next phases from current state.
    ///
    /// Returns the list of phases that can be validly executed based on
    /// the current workflow state.
    pub fn legal_next_phases(&self) -> Result<Vec<PhaseId>> {
        let current = self.current_phase()?;
        Ok(match current {
            None => vec![PhaseId::Requirements],
            Some(PhaseId::Requirements) => vec![PhaseId::Requirements, PhaseId::Design],
            Some(PhaseId::Design) => vec![PhaseId::Design, PhaseId::Tasks],
            Some(PhaseId::Tasks) => vec![PhaseId::Tasks, PhaseId::Review, PhaseId::Final],
            Some(PhaseId::Review) => vec![PhaseId::Review, PhaseId::Fixup, PhaseId::Final],
            Some(PhaseId::Fixup) => vec![PhaseId::Fixup, PhaseId::Final],
            Some(PhaseId::Final) => vec![PhaseId::Final],
        })
    }

    /// Set a configuration option.
    ///
    /// Common keys include:
    /// - `model`: LLM model to use
    /// - `phase_timeout`: Timeout in seconds
    /// - `apply_fixups`: Whether to apply fixups or preview
    pub fn set_config(&mut self, key: &str, value: &str) {
        self.config
            .config
            .insert(key.to_string(), value.to_string());
    }

    /// Get a configuration option.
    ///
    /// Returns `None` if the key is not set.
    #[must_use]
    pub fn get_config(&self, key: &str) -> Option<&String> {
        self.config.config.get(key)
    }

    /// Enable or disable dry-run mode.
    ///
    /// In dry-run mode, phases are simulated without calling the LLM.
    pub fn set_dry_run(&mut self, dry_run: bool) {
        self.config.dry_run = dry_run;
    }

    /// Get the current orchestrator configuration.
    ///
    /// Returns a reference to the configuration used for phase execution.
    #[must_use]
    pub fn orchestrator_config(&self) -> &OrchestratorConfig {
        &self.config
    }

    /// Access the artifact manager for status queries.
    ///
    /// Use this for read-only operations like checking phase completion,
    /// listing artifacts, or getting the base path.
    #[must_use]
    #[doc(hidden)]
    pub fn artifact_manager(&self) -> &ArtifactManager {
        self.orchestrator.artifact_manager()
    }

    /// Access the receipt manager for status queries.
    ///
    /// Use this for read-only operations like listing receipts or
    /// getting receipt metadata.
    #[must_use]
    #[doc(hidden)]
    pub fn receipt_manager(&self) -> &ReceiptManager {
        self.orchestrator.receipt_manager()
    }

    /// Get a reference to the underlying orchestrator.
    ///
    /// This is primarily for interop with APIs that require `&PhaseOrchestrator`,
    /// such as `StatusManager::generate_status_from_orchestrator`.
    ///
    /// Prefer using the high-level methods on `OrchestratorHandle` when possible.
    #[must_use]
    #[doc(hidden)]
    pub fn as_orchestrator(&self) -> &PhaseOrchestrator {
        &self.orchestrator
    }
}

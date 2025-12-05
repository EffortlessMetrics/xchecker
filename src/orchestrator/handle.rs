//! Orchestrator façade for external consumers
//!
//! This module provides a clean, stable API for external consumers (CLI, Kiro, MCP tools)
//! to interact with the phase orchestrator without needing to know internal details.
//!
//! **Integration rule**: Outside `src/orchestrator/`, use `OrchestratorHandle`.
//! Direct `PhaseOrchestrator` usage is reserved for tests and orchestrator internals.
//!
//! **v1.0 Status**: Most methods in this module are reserved for future IDE/TUI integration.
//! The CLI currently uses `PhaseOrchestrator` directly via internal commands. These methods
//! will be wired into external tooling in a future release.

use anyhow::Result;

use crate::artifact::ArtifactManager;
use crate::receipt::ReceiptManager;
use crate::types::PhaseId;

use super::{ExecutionResult, OrchestratorConfig, PhaseOrchestrator};

/// Kiro-friendly orchestrator handle
///
/// Provides a simplified interface for running phases programmatically.
/// Use this when integrating xchecker with external tools or agents.
///
/// **v1.0 Status**: Reserved for future IDE/TUI integration. The CLI currently
/// uses `PhaseOrchestrator` directly. These methods will be wired into external
/// tooling in a future release.
///
/// # Example
/// ```ignore
/// let handle = OrchestratorHandle::new("my-spec")?;
/// let result = handle.run_phase(PhaseId::Requirements).await?;
/// println!("Success: {}", result.success);
/// ```
#[allow(dead_code)] // Reserved for future IDE/TUI integration
pub struct OrchestratorHandle {
    orchestrator: PhaseOrchestrator,
    config: OrchestratorConfig,
}

impl OrchestratorHandle {
    /// Create a new handle for the given spec.
    ///
    /// Acquires an exclusive lock on the spec directory and creates a handle
    /// with default configuration.
    ///
    /// Not currently used by CLI; reserved for IDE/TUI integration.
    ///
    /// # Errors
    /// Returns error if orchestrator creation fails or lock cannot be acquired.
    #[allow(dead_code)] // Reserved for future IDE/TUI integration
    pub fn new(spec_id: &str) -> Result<Self> {
        let orchestrator = PhaseOrchestrator::new(spec_id)?;
        let config = OrchestratorConfig::default();
        Ok(Self {
            orchestrator,
            config,
        })
    }

    /// Create a handle with custom configuration.
    ///
    /// Allows setting custom config parameters like model, timeout, etc.
    /// before executing phases.
    ///
    /// Not currently used by CLI; reserved for IDE/TUI integration.
    ///
    /// # Errors
    /// Returns error if orchestrator creation fails or lock cannot be acquired.
    #[allow(dead_code)] // Reserved for future IDE/TUI integration
    pub fn with_config(spec_id: &str, config: OrchestratorConfig) -> Result<Self> {
        let orchestrator = PhaseOrchestrator::new(spec_id)?;
        Ok(Self {
            orchestrator,
            config,
        })
    }

    /// Create a handle with force flag for lock override.
    ///
    /// Use with caution: forcing lock override can lead to race conditions if another
    /// process is actively working on the spec.
    ///
    /// Not currently used by CLI; reserved for IDE/TUI integration.
    ///
    /// # Arguments
    /// * `spec_id` - The spec identifier
    /// * `force` - Whether to override existing locks
    ///
    /// # Errors
    /// Returns error if orchestrator creation fails.
    #[allow(dead_code)] // Reserved for future IDE/TUI integration
    pub fn with_force(spec_id: &str, force: bool) -> Result<Self> {
        let orchestrator = PhaseOrchestrator::new_with_force(spec_id, force)?;
        let config = OrchestratorConfig::default();
        Ok(Self {
            orchestrator,
            config,
        })
    }

    /// Create a handle with custom configuration and force flag.
    ///
    /// Combines custom config with lock override capability.
    ///
    /// Not currently used by CLI; reserved for IDE/TUI integration.
    ///
    /// # Errors
    /// Returns error if orchestrator creation fails.
    #[allow(dead_code)] // Reserved for future IDE/TUI integration
    pub fn with_config_and_force(
        spec_id: &str,
        config: OrchestratorConfig,
        force: bool,
    ) -> Result<Self> {
        let orchestrator = PhaseOrchestrator::new_with_force(spec_id, force)?;
        Ok(Self {
            orchestrator,
            config,
        })
    }

    /// Create a read-only handle for status inspection.
    ///
    /// Does not acquire locks, allowing inspection while another process
    /// is actively working on the spec.
    ///
    /// Not currently used by CLI; reserved for IDE/TUI integration.
    ///
    /// # Errors
    /// Returns error if orchestrator creation fails.
    #[allow(dead_code)] // Reserved for future IDE/TUI integration
    pub fn readonly(spec_id: &str) -> Result<Self> {
        let orchestrator = PhaseOrchestrator::new_readonly(spec_id)?;
        let config = OrchestratorConfig::default();
        Ok(Self {
            orchestrator,
            config,
        })
    }

    /// Run a specific phase.
    ///
    /// Validates phase transition rules and executes the phase end-to-end,
    /// generating artifacts and receipts.
    ///
    /// Not currently used by CLI; reserved for IDE/TUI integration.
    ///
    /// # Errors
    /// Returns error if transition is invalid or execution fails.
    #[allow(dead_code)] // Reserved for future IDE/TUI integration
    pub async fn run_phase(&self, phase: PhaseId) -> Result<ExecutionResult> {
        self.orchestrator
            .resume_from_phase(phase, &self.config)
            .await
    }

    /// Check if a phase can be run.
    ///
    /// Validates that all dependencies are satisfied and have successful receipts.
    ///
    /// Not currently used by CLI; reserved for IDE/TUI integration.
    ///
    /// # Returns
    /// `true` if the phase can be executed, `false` otherwise.
    #[allow(dead_code)] // Reserved for future IDE/TUI integration
    pub fn can_run_phase(&self, phase: PhaseId) -> Result<bool> {
        self.orchestrator.can_resume_from_phase_public(phase)
    }

    /// Get the current phase state.
    ///
    /// Returns the last successfully completed phase, or `None` if no phases
    /// have been completed.
    ///
    /// Not currently used by CLI; reserved for IDE/TUI integration.
    #[allow(dead_code)] // Reserved for future IDE/TUI integration
    pub fn current_phase(&self) -> Result<Option<PhaseId>> {
        self.orchestrator.get_current_phase_state()
    }

    /// Get legal next phases from current state.
    ///
    /// Returns the list of phases that can be validly executed based on
    /// the current workflow state.
    ///
    /// Not currently used by CLI; reserved for IDE/TUI integration.
    #[allow(dead_code)] // Reserved for future IDE/TUI integration
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
    ///
    /// Not currently used by CLI; reserved for IDE/TUI integration.
    #[allow(dead_code)] // Reserved for future IDE/TUI integration
    pub fn set_config(&mut self, key: &str, value: &str) {
        self.config
            .config
            .insert(key.to_string(), value.to_string());
    }

    /// Get a configuration option.
    ///
    /// Returns `None` if the key is not set.
    ///
    /// Not currently used by CLI; reserved for IDE/TUI integration.
    #[allow(dead_code)] // Reserved for future IDE/TUI integration
    pub fn get_config(&self, key: &str) -> Option<&String> {
        self.config.config.get(key)
    }

    /// Enable or disable dry-run mode.
    ///
    /// In dry-run mode, phases are simulated without calling the LLM.
    ///
    /// Not currently used by CLI; reserved for IDE/TUI integration.
    #[allow(dead_code)] // Reserved for future IDE/TUI integration
    pub fn set_dry_run(&mut self, dry_run: bool) {
        self.config.dry_run = dry_run;
    }

    /// Get the spec ID.
    ///
    /// Returns the identifier for the spec managed by this handle.
    ///
    /// Not currently used by CLI; reserved for IDE/TUI integration.
    #[must_use]
    #[allow(dead_code)] // Reserved for future IDE/TUI integration
    pub fn spec_id(&self) -> &str {
        self.orchestrator.spec_id()
    }

    /// Get the current orchestrator configuration.
    ///
    /// Returns a reference to the configuration used for phase execution.
    ///
    /// Not currently used by CLI; reserved for IDE/TUI integration.
    #[must_use]
    #[allow(dead_code)] // Reserved for future IDE/TUI integration
    pub fn config(&self) -> &OrchestratorConfig {
        &self.config
    }

    /// Access the artifact manager for status queries.
    ///
    /// Use this for read-only operations like checking phase completion,
    /// listing artifacts, or getting the base path.
    ///
    /// Not currently used by CLI; reserved for IDE/TUI integration.
    #[must_use]
    #[allow(dead_code)] // Reserved for future IDE/TUI integration
    pub fn artifact_manager(&self) -> &ArtifactManager {
        self.orchestrator.artifact_manager()
    }

    /// Access the receipt manager for status queries.
    ///
    /// Use this for read-only operations like listing receipts or
    /// getting receipt metadata.
    ///
    /// Not currently used by CLI; reserved for IDE/TUI integration.
    #[must_use]
    #[allow(dead_code)] // Reserved for future IDE/TUI integration
    pub fn receipt_manager(&self) -> &ReceiptManager {
        self.orchestrator.receipt_manager()
    }

    /// Get a reference to the underlying orchestrator.
    ///
    /// This is primarily for interop with APIs that require `&PhaseOrchestrator`,
    /// such as `StatusManager::generate_status_from_orchestrator`.
    ///
    /// Prefer using the high-level methods on `OrchestratorHandle` when possible.
    ///
    /// Not currently used by CLI; reserved for IDE/TUI integration.
    #[must_use]
    #[allow(dead_code)] // Reserved for future IDE/TUI integration
    pub fn as_orchestrator(&self) -> &PhaseOrchestrator {
        &self.orchestrator
    }
}

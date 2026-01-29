//! Phase orchestrator for executing spec generation workflows
//!
//! This module provides the core orchestration logic that wires together
//! the Phase trait, `ArtifactManager`, and Receipt system to execute
//! phases end-to-end with proper error handling and state management.

mod handle;
mod llm;
mod phase_exec;
mod workflow;

#[allow(unused_imports)]
pub use self::handle::OrchestratorHandle;

#[allow(unused_imports)]
pub use self::phase_exec::ExecutionResult;

// Workflow types are internal - used by execute_complete_workflow which is also pub(crate)
#[allow(unused_imports)]
pub(crate) use self::workflow::{PhaseExecution, PhaseExecutionResult, WorkflowResult};

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::time::Duration;

use crate::config::Selectors;
use crate::error::{PhaseError, XCheckerError};
use crate::hooks::HooksConfig;
use crate::receipt::ReceiptManager;
use crate::status::artifact::ArtifactManager;
use crate::types::PhaseId;
use std::sync::Arc;

/// Orchestrates the execution of spec generation phases.
///
/// This is the main entry point for running phases. It manages:
/// - Phase execution with dependency validation
/// - Artifact storage and receipt generation
/// - Lock management for concurrent access
/// - Error handling and partial artifact preservation
///
/// # API Stability and Usage Guidance
///
/// **Production code**: Use [`OrchestratorHandle`] for all production scenarios. It provides
/// a stable, higher-level API designed for CLI commands and external integrations.
///
/// **Test code**: `PhaseOrchestrator` exposes internal helper methods (marked with `#[doc(hidden)]`)
/// for white-box testing of phase orchestration logic. These methods are public to enable testing
/// but are not part of the stable API and may change without notice.
///
/// **Stability notice**: Methods marked with `#[doc(hidden)]` are internal implementation details
/// and may be narrowed to `pub(crate)` visibility in future versions. Do not rely on them in
/// production code.
///
/// # Examples
///
/// Production usage (preferred):
/// ```ignore
/// let handle = OrchestratorHandle::new("my-spec")?;
/// handle.execute_requirements_phase(&config).await?;
/// ```
///
/// Test usage (white-box testing):
/// ```ignore
/// let orchestrator = PhaseOrchestrator::new("test-spec")?;
/// orchestrator.validate_transition(PhaseId::Design)?;  // Test internal logic
/// let config = OrchestratorConfig::default();
/// orchestrator.execute_requirements_phase(&config).await?;
/// ```
pub struct PhaseOrchestrator {
    spec_id: String,
    artifact_manager: ArtifactManager,
    receipt_manager: ReceiptManager,
}

/// Configuration for orchestrator execution.
///
/// Controls how phases are executed, including dry-run mode
/// and various runtime settings passed via the config map.
///
/// # Common Config Keys
/// - `model`: LLM model to use
/// - `phase_timeout`: Timeout in seconds
/// - `apply_fixups`: Whether to apply fixups or preview
#[derive(Debug, Clone, Default)]
pub struct OrchestratorConfig {
    /// Whether to run in dry-run mode (no Claude calls)
    pub dry_run: bool,
    /// Additional configuration parameters
    pub config: HashMap<String, String>,
    /// Full configuration snapshot for LLM backends (when available).
    ///
    /// When set, this allows LLM backend construction to use the complete
    /// configuration model instead of the flattened config map.
    pub full_config: Option<crate::config::Config>,
    /// Content selectors for packet building
    ///
    /// If `Some`, phases use these selectors when building packets.
    /// If `None`, phases fall back to built-in selector defaults.
    pub selectors: Option<Selectors>,
    /// Enable strict validation for phase outputs.
    ///
    /// When `true`, validation failures (meta-summaries, too-short output,
    /// missing required sections) become hard errors that fail the phase.
    /// When `false`, validation issues are logged as warnings only.
    pub strict_validation: bool,
    /// Secret redactor built from the effective configuration.
    ///
    /// Used for both secret scanning and final-pass redaction of user-facing output (FR-SEC-19).
    pub redactor: Arc<crate::redaction::SecretRedactor>,
    /// Hooks configuration for pre/post phase scripts.
    pub hooks: Option<HooksConfig>,
}

/// Phase timeout configuration with sensible defaults.
///
/// Enforces minimum and default timeout values to prevent
/// runaway phase executions.
#[derive(Debug, Clone)]
pub struct PhaseTimeout {
    /// Timeout duration for phase execution
    pub duration: Duration,
}

impl PhaseTimeout {
    /// Default timeout in seconds (10 minutes)
    pub const DEFAULT_SECS: u64 = 600;

    /// Minimum timeout in seconds (5 seconds)
    pub const MIN_SECS: u64 = 5;

    /// Create a `PhaseTimeout` with a specific duration in seconds
    #[must_use]
    pub fn from_secs(secs: u64) -> Self {
        let timeout_secs = secs.max(Self::MIN_SECS);
        Self {
            duration: Duration::from_secs(timeout_secs),
        }
    }

    /// Create `PhaseTimeout` from configuration with validation
    /// Reads from CLI args or config file, with fallback to default
    #[must_use]
    pub fn from_config(config: &OrchestratorConfig) -> Self {
        let timeout_secs = config
            .config
            .get("phase_timeout")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(Self::DEFAULT_SECS);

        Self::from_secs(timeout_secs)
    }
}

impl PhaseOrchestrator {
    /// Create a new orchestrator for the given spec ID.
    ///
    /// Acquires an exclusive lock on the spec directory to prevent concurrent modifications.
    ///
    /// # Errors
    /// Returns error if lock cannot be acquired or artifact manager creation fails.
    pub fn new(spec_id: &str) -> Result<Self> {
        Self::new_with_force(spec_id, false)
    }

    /// Create a new orchestrator with optional force flag for lock override.
    ///
    /// Use with caution: forcing lock override can lead to race conditions if another
    /// process is actively working on the spec.
    ///
    /// # Arguments
    /// * `spec_id` - The spec identifier
    /// * `force` - Whether to override existing locks
    ///
    /// # Errors
    /// Returns error if artifact manager creation fails.
    pub fn new_with_force(spec_id: &str, force: bool) -> Result<Self> {
        let artifact_manager = ArtifactManager::new_with_force(spec_id, force)
            .with_context(|| format!("Failed to create artifact manager for spec: {spec_id}"))?;

        let receipt_manager = ReceiptManager::new(artifact_manager.base_path());

        Ok(Self {
            spec_id: spec_id.to_string(),
            artifact_manager,
            receipt_manager,
        })
    }

    /// Create a read-only orchestrator that doesn't acquire locks.
    ///
    /// Use this for status queries and inspection operations that don't modify state.
    /// This allows reading spec data while another process holds the write lock.
    ///
    /// # Errors
    /// Returns error if artifact manager creation fails.
    pub fn new_readonly(spec_id: &str) -> Result<Self> {
        // For read-only access, we create the managers directly without locks
        let base_path = crate::paths::spec_root(spec_id);

        // Create a dummy artifact manager without lock for read-only operations
        let artifact_manager = ArtifactManager::new_readonly(spec_id)?;
        let receipt_manager = ReceiptManager::new(&base_path);

        Ok(Self {
            spec_id: spec_id.to_string(),
            artifact_manager,
            receipt_manager,
        })
    }

    /// Check if we can resume from a specific phase
    fn can_resume_from_phase(&self, phase_id: PhaseId) -> Result<bool> {
        // Check dependencies are satisfied
        let deps = match phase_id {
            PhaseId::Requirements => &[][..],
            PhaseId::Design => &[PhaseId::Requirements][..],
            PhaseId::Tasks => &[PhaseId::Design][..],
            PhaseId::Review => &[PhaseId::Tasks][..],
            PhaseId::Fixup => &[PhaseId::Review][..],
            PhaseId::Final => &[PhaseId::Tasks][..], // Can skip review/fixup
        };

        for dep_phase in deps {
            if !self.artifact_manager.phase_completed(*dep_phase) {
                return Ok(false);
            }

            // Also check that the dependency has a successful receipt
            if let Some(receipt) = self.receipt_manager.read_latest_receipt(*dep_phase)? {
                if receipt.exit_code != 0 {
                    return Ok(false);
                }
            } else {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Validate phase transition from current state to target phase.
    ///
    /// Ensures that the workflow follows valid phase sequences and all dependencies
    /// are satisfied before allowing execution. This implements FR-ORC-001 and FR-ORC-002.
    ///
    /// # Visibility and Stability
    ///
    /// This method is `pub` to enable white-box testing of internal phase orchestration
    /// logic. It is **not** part of the stable public API. Production code should use
    /// [`OrchestratorHandle`] instead.
    ///
    /// **Test-oriented API**: Exposed for integration tests that need to verify phase
    /// transition logic directly. May be narrowed to `pub(crate)` in future versions.
    ///
    /// **Stability**: Internal implementation detail. Signature and behavior may change
    /// without warning. Do not use in production code.
    ///
    /// # Arguments
    /// * `target_phase` - The phase to validate transition to
    ///
    /// # Errors
    /// Returns `XCheckerError::Phase` with specific guidance if:
    /// - Transition violates phase sequencing rules
    /// - Required dependencies are not satisfied
    #[doc(hidden)]
    pub fn validate_transition(&self, target_phase: PhaseId) -> Result<(), XCheckerError> {
        // Get current phase from last successful receipt
        let current_phase = self.get_current_phase().map_err(|e| {
            XCheckerError::Phase(PhaseError::ContextCreationFailed {
                phase: target_phase.as_str().to_string(),
                reason: format!("Failed to determine current phase: {e}"),
            })
        })?;

        // Define legal transitions
        let legal_next_phases = match current_phase {
            None => vec![PhaseId::Requirements], // Fresh spec can only start with Requirements
            Some(PhaseId::Requirements) => vec![PhaseId::Requirements, PhaseId::Design],
            Some(PhaseId::Design) => vec![PhaseId::Design, PhaseId::Tasks],
            Some(PhaseId::Tasks) => vec![PhaseId::Tasks, PhaseId::Review, PhaseId::Final],
            Some(PhaseId::Review) => vec![PhaseId::Review, PhaseId::Fixup, PhaseId::Final],
            Some(PhaseId::Fixup) => vec![PhaseId::Fixup, PhaseId::Final],
            Some(PhaseId::Final) => vec![PhaseId::Final], // Can re-run final
        };

        // Check if target phase is in the list of legal next phases
        if !legal_next_phases.contains(&target_phase) {
            let current_str = current_phase.map_or_else(
                || "none (fresh spec)".to_string(),
                |p| p.as_str().to_string(),
            );

            return Err(XCheckerError::Phase(PhaseError::InvalidTransition {
                from: current_str,
                to: target_phase.as_str().to_string(),
            }));
        }

        // Check dependencies are satisfied
        self.check_dependencies_satisfied(target_phase)?;

        Ok(())
    }

    /// Get the current phase from the last successful receipt
    fn get_current_phase(&self) -> Result<Option<PhaseId>> {
        // Check each phase in reverse order to find the last completed one
        let phases = [
            PhaseId::Final,
            PhaseId::Fixup,
            PhaseId::Review,
            PhaseId::Tasks,
            PhaseId::Design,
            PhaseId::Requirements,
        ];

        for phase in &phases {
            if let Some(receipt) = self.receipt_manager.read_latest_receipt(*phase)?
                && receipt.exit_code == 0
            {
                return Ok(Some(*phase));
            }
        }

        Ok(None) // No successful receipts found
    }

    /// Check that all dependencies for a phase are satisfied
    fn check_dependencies_satisfied(&self, phase_id: PhaseId) -> Result<(), XCheckerError> {
        let deps = match phase_id {
            PhaseId::Requirements => &[][..],
            PhaseId::Design => &[PhaseId::Requirements][..],
            PhaseId::Tasks => &[PhaseId::Design][..],
            PhaseId::Review => &[PhaseId::Tasks][..],
            PhaseId::Fixup => &[PhaseId::Review][..],
            PhaseId::Final => &[PhaseId::Tasks][..], // Can skip review/fixup
        };

        for dep_phase in deps {
            // Check if we have a successful receipt for the dependency
            let receipt_result = self
                .receipt_manager
                .read_latest_receipt(*dep_phase)
                .map_err(|e| {
                    XCheckerError::Phase(PhaseError::ContextCreationFailed {
                        phase: phase_id.as_str().to_string(),
                        reason: format!(
                            "Failed to read receipt for dependency {}: {}",
                            dep_phase.as_str(),
                            e
                        ),
                    })
                })?;

            if let Some(receipt) = receipt_result {
                if receipt.exit_code != 0 {
                    return Err(XCheckerError::Phase(PhaseError::DependencyNotSatisfied {
                        phase: phase_id.as_str().to_string(),
                        dependency: dep_phase.as_str().to_string(),
                    }));
                }
            } else {
                return Err(XCheckerError::Phase(PhaseError::DependencyNotSatisfied {
                    phase: phase_id.as_str().to_string(),
                    dependency: dep_phase.as_str().to_string(),
                }));
            }
        }

        Ok(())
    }

    /// Get the spec ID.
    ///
    /// Returns the identifier for the spec managed by this orchestrator.
    #[must_use]
    pub(crate) fn spec_id(&self) -> &str {
        &self.spec_id
    }

    /// Returns a reference to the artifact manager. Used by status generation and tests.
    #[must_use]
    pub fn artifact_manager(&self) -> &ArtifactManager {
        &self.artifact_manager
    }

    /// Returns a reference to the receipt manager. Used by status generation and tests.
    #[must_use]
    pub fn receipt_manager(&self) -> &ReceiptManager {
        &self.receipt_manager
    }

    /// Returns the current phase from last successful receipt.
    ///
    /// # Visibility and Stability
    ///
    /// This method is `pub` to enable white-box testing of phase state detection.
    /// It is **not** part of the stable public API. Production code should use
    /// [`OrchestratorHandle`] instead.
    ///
    /// **Test-oriented API**: Exposed for tests that need to verify current phase
    /// state detection logic. May be narrowed to `pub(crate)` in future versions.
    ///
    /// **Stability**: Internal implementation detail. Signature and behavior may change
    /// without warning. Do not use in production code.
    #[doc(hidden)]
    #[allow(dead_code)]
    pub fn get_current_phase_state(&self) -> Result<Option<PhaseId>> {
        self.get_current_phase()
    }

    /// Checks if resume from a given phase is valid.
    ///
    /// # Visibility and Stability
    ///
    /// This method is `pub` to enable white-box testing of resume validation logic.
    /// It is **not** part of the stable public API. Production code should use
    /// [`OrchestratorHandle`] instead.
    ///
    /// **Test-oriented API**: Exposed for tests that need to verify resume capability
    /// checking. May be narrowed to `pub(crate)` in future versions.
    ///
    /// **Stability**: Internal implementation detail. Signature and behavior may change
    /// without warning. Do not use in production code.
    #[doc(hidden)]
    #[allow(dead_code)]
    pub fn can_resume_from_phase_public(&self, phase_id: PhaseId) -> Result<bool> {
        self.can_resume_from_phase(phase_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phase::{NextStep, Phase, PhaseContext};
    use crate::phases::RequirementsPhase;
    use crate::test_support;
    use std::env;
    use std::path::PathBuf;
    use std::sync::{Mutex, MutexGuard, OnceLock};
    use tempfile::TempDir;

    // Global lock for tests that mutate process-global state (env vars, cwd).
    // Any test that uses `TempDirGuard` will be serialized.
    static ORCHESTRATOR_ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn orchestrator_env_guard() -> MutexGuard<'static, ()> {
        ORCHESTRATOR_ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap()
    }

    #[allow(dead_code)] // Test helper: kept for future test expansion
    fn setup_test_environment() -> (PhaseOrchestrator, TempDir) {
        let _lock = orchestrator_env_guard();
        let temp_dir = TempDir::new().unwrap();

        // Change to temp directory for test
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_dir.path()).unwrap();

        let orchestrator = PhaseOrchestrator::new("test-spec-123").unwrap();

        // Restore original directory
        env::set_current_dir(original_dir).unwrap();

        (orchestrator, temp_dir)
    }

    #[allow(dead_code)] // Test helper: kept for future test expansion
    fn setup_test_environment_with_cleanup() -> (PhaseOrchestrator, TempDir) {
        let _lock = orchestrator_env_guard();
        let temp_dir = TempDir::new().unwrap();

        // Change to temp directory for test and keep it there
        env::set_current_dir(temp_dir.path()).unwrap();

        let orchestrator = PhaseOrchestrator::new("test-spec-123").unwrap();

        (orchestrator, temp_dir)
    }

    #[allow(dead_code)] // Test helper: kept for future test expansion
    fn setup_test_with_unique_id(test_name: &str) -> (PhaseOrchestrator, TempDir) {
        let _lock = orchestrator_env_guard();
        let temp_dir = TempDir::new().unwrap();

        // Store original directory and change to temp directory
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_dir.path()).unwrap();

        let spec_id = format!("test-{test_name}");

        // Create the orchestrator in the temp directory context
        let orchestrator = match PhaseOrchestrator::new(&spec_id) {
            Ok(orch) => orch,
            Err(e) => {
                // Restore directory before panicking
                env::set_current_dir(original_dir).unwrap();
                panic!("Failed to create orchestrator: {e}");
            }
        };

        // Keep the temp directory as current for the test duration
        // The test will need to restore it manually or use a guard

        (orchestrator, temp_dir)
    }

    struct TempDirGuard {
        // Hold the lock for the entire lifetime of the guard
        _lock: MutexGuard<'static, ()>,
        _temp_dir: TempDir,
        _home_dir: TempDir,
        original_dir: PathBuf,
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = env::set_current_dir(&self.original_dir);
            // _lock field drops last, releasing the mutex
        }
    }

    fn setup_test_with_guard(test_name: &str) -> (PhaseOrchestrator, TempDirGuard) {
        // Take the global lock first
        let lock = orchestrator_env_guard();

        // Isolate home directory to prevent cross-test contamination
        let home_dir = crate::paths::with_isolated_home();

        let temp_dir = TempDir::new().unwrap();
        let original_dir = env::current_dir().unwrap();

        env::set_current_dir(temp_dir.path()).unwrap();

        // Use unique spec ID per test to avoid conflicts
        let spec_id = format!("test-{}-{}", test_name, std::process::id());
        let orchestrator = PhaseOrchestrator::new(&spec_id).unwrap();

        let guard = TempDirGuard {
            _lock: lock,
            _temp_dir: temp_dir,
            _home_dir: home_dir,
            original_dir,
        };

        (orchestrator, guard)
    }

    #[test]
    fn test_orchestrator_creation() {
        // Test orchestrator creation logic without file system operations
        // This test verifies the basic structure and configuration
        let config = OrchestratorConfig::default();
        assert!(!config.dry_run);
        assert!(config.config.is_empty());
    }

    #[tokio::test]
    async fn test_requirements_phase_execution() {
        let (orchestrator, _guard) = setup_test_with_guard("execution");

        let config = OrchestratorConfig {
            dry_run: true,
            config: HashMap::new(),
            full_config: None,
            selectors: None,
            strict_validation: false,
            redactor: std::sync::Arc::new(crate::redaction::SecretRedactor::default()),
            hooks: None,
        };

        let result = orchestrator.execute_requirements_phase(&config).await;
        if let Err(ref e) = result {
            eprintln!("Test failed with error: {e:?}");
        }
        assert!(result.is_ok());

        let execution_result = result.unwrap();
        assert_eq!(execution_result.phase, PhaseId::Requirements);
        assert!(execution_result.success);
        assert_eq!(execution_result.exit_code, 0);
        assert!(!execution_result.artifact_paths.is_empty());
        assert!(execution_result.receipt_path.is_some());
        assert!(execution_result.error.is_none());
    }

    #[test]
    fn test_phase_context_creation() {
        // Test phase context structure without file system operations
        use std::path::PathBuf;

        let context = PhaseContext {
            spec_id: "test-spec".to_string(),
            spec_dir: PathBuf::from("/tmp/test"),
            config: HashMap::new(),
            artifacts: vec!["test-artifact.md".to_string()],
            selectors: None,
            strict_validation: false,
            redactor: std::sync::Arc::new(crate::redaction::SecretRedactor::default()),
        };

        assert_eq!(context.spec_id, "test-spec");
        assert_eq!(context.artifacts.len(), 1);
        assert_eq!(context.artifacts[0], "test-artifact.md");
    }

    #[test]
    fn test_dependency_checking() {
        // Test dependency checking logic without requiring file system
        let requirements_phase = RequirementsPhase::new();
        let design_phase = crate::phases::DesignPhase::new();

        // Requirements has no dependencies
        assert_eq!(requirements_phase.deps().len(), 0);

        // Design depends on Requirements
        assert_eq!(design_phase.deps().len(), 1);
        assert_eq!(design_phase.deps()[0], PhaseId::Requirements);
    }

    #[test]
    fn test_claude_response_simulation() {
        // Test Claude response simulation logic without file system operations
        // This is a pure unit test that doesn't require orchestrator setup

        // Test Requirements phase simulation
        let spec_id = "test-claude";
        let response = format!(
            r"# Requirements Document

## Introduction

This is a generated requirements document for spec {}. The system will provide core functionality for managing and processing specifications through a structured workflow.

## Requirements

### Requirement 1

**User Story:** As a developer, I want to generate structured requirements from rough ideas, so that I can create comprehensive specifications efficiently.

#### Acceptance Criteria

1. WHEN I provide a problem statement THEN the system SHALL generate structured requirements in EARS format
2. WHEN requirements are generated THEN they SHALL include user stories and acceptance criteria
3. WHEN the process completes THEN the system SHALL produce both markdown and YAML artifacts
",
            spec_id
        );

        // Verify the simulated response has expected structure
        assert!(!response.is_empty());
        assert!(response.contains("Requirements Document"));
        assert!(response.contains("test-claude"));
        assert!(response.contains("User Story:"));
        assert!(response.contains("Acceptance Criteria"));
    }

    #[test]
    fn test_execution_result_structure() {
        // Test ExecutionResult structure without requiring orchestrator
        let result = ExecutionResult {
            phase: PhaseId::Requirements,
            success: true,
            exit_code: 0,
            artifact_paths: vec![],
            receipt_path: None,
            error: None,
        };

        assert_eq!(result.phase, PhaseId::Requirements);
        assert!(result.success);
        assert_eq!(result.exit_code, 0);
        assert!(result.artifact_paths.is_empty());
        assert!(result.receipt_path.is_none());
        assert!(result.error.is_none());
    }

    #[tokio::test]
    async fn test_secret_scanning_before_claude_invocation() {
        // Test that secret scanning happens before Claude invocation
        // and prevents execution with proper error receipt

        let (orchestrator, _guard) = setup_test_with_guard("secret-scan");

        // Create a custom phase that includes a secret in the packet
        struct SecretPhase;

        impl Phase for SecretPhase {
            fn id(&self) -> PhaseId {
                PhaseId::Requirements
            }

            fn deps(&self) -> &'static [PhaseId] {
                &[]
            }

            fn can_resume(&self) -> bool {
                true
            }

            fn prompt(&self, _ctx: &PhaseContext) -> String {
                "Generate requirements".to_string()
            }

            fn make_packet(&self, _ctx: &PhaseContext) -> Result<xchecker_packet::Packet> {
                // Create a packet with a GitHub PAT secret
                let token = test_support::github_pat();
                let content = format!("Here is my GitHub token: {}\nSome other content", token);
                let blake3_hash = blake3::hash(content.as_bytes()).to_hex().to_string();

                let evidence = crate::types::PacketEvidence {
                    files: vec![],
                    max_bytes: 65536,
                    max_lines: 1200,
                };

                let mut budget = xchecker_packet::BudgetUsage::new(65536, 1200);
                budget.add_content(content.len(), content.lines().count());

                Ok(xchecker_packet::Packet::new(
                    content,
                    blake3_hash,
                    evidence,
                    budget,
                ))
            }

            fn postprocess(
                &self,
                _raw: &str,
                _ctx: &PhaseContext,
            ) -> Result<xchecker_phase_api::PhaseResult> {
                unreachable!("Should not reach postprocess when secret is detected");
            }
        }

        let phase = SecretPhase;
        let config = OrchestratorConfig::default();

        // Execute the phase - should fail with secret detection
        let result = orchestrator.execute_phase(&phase, &config).await;

        assert!(result.is_ok(), "Should return Ok with error result");
        let exec_result = result.unwrap();

        // Verify the execution failed due to secret detection
        assert!(!exec_result.success, "Execution should fail");
        assert_eq!(
            exec_result.exit_code,
            crate::exit_codes::codes::SECRET_DETECTED
        );
        assert!(exec_result.error.is_some(), "Should have error message");
        assert!(exec_result.error.unwrap().contains("Secret detected"));

        // Verify receipt was written
        assert!(
            exec_result.receipt_path.is_some(),
            "Receipt should be written"
        );
    }

    #[tokio::test]
    async fn test_packet_evidence_populated_in_receipt() {
        // Test that PacketEvidence in receipts contains actual file list
        // from the packet that was created

        let (orchestrator, _guard) = setup_test_with_guard("packet-evidence");

        // Create a custom phase that includes file evidence in the packet
        struct EvidencePhase;

        impl Phase for EvidencePhase {
            fn id(&self) -> PhaseId {
                PhaseId::Requirements
            }

            fn deps(&self) -> &'static [PhaseId] {
                &[]
            }

            fn can_resume(&self) -> bool {
                true
            }

            fn prompt(&self, _ctx: &PhaseContext) -> String {
                "Generate requirements".to_string()
            }

            fn make_packet(&self, _ctx: &PhaseContext) -> Result<xchecker_packet::Packet> {
                let content = "Test packet content without secrets";
                let blake3_hash = blake3::hash(content.as_bytes()).to_hex().to_string();

                // Create evidence with specific files
                let evidence = crate::types::PacketEvidence {
                    files: vec![
                        crate::types::FileEvidence {
                            path: "src/main.rs".to_string(),
                            range: Some("L1-L100".to_string()),
                            blake3_pre_redaction: "abc123".to_string(),
                            priority: crate::types::Priority::High,
                        },
                        crate::types::FileEvidence {
                            path: "Cargo.toml".to_string(),
                            range: Some("L1-L50".to_string()),
                            blake3_pre_redaction: "def456".to_string(),
                            priority: crate::types::Priority::Medium,
                        },
                    ],
                    max_bytes: 65536,
                    max_lines: 1200,
                };

                let mut budget = xchecker_packet::BudgetUsage::new(65536, 1200);
                budget.add_content(content.len(), content.lines().count());

                Ok(xchecker_packet::Packet::new(
                    content.to_string(),
                    blake3_hash,
                    evidence,
                    budget,
                ))
            }

            fn postprocess(
                &self,
                _raw: &str,
                ctx: &PhaseContext,
            ) -> Result<crate::phase::PhaseResult> {
                // Generate a simple artifact
                let artifact = crate::status::artifact::Artifact {
                    name: "00-requirements.md".to_string(),
                    content: format!("# Requirements for {}\n\nTest requirements.", ctx.spec_id),
                    artifact_type: crate::status::artifact::ArtifactType::Markdown,
                    blake3_hash: String::new(), // Will be computed during storage
                };

                Ok(crate::phase::PhaseResult {
                    artifacts: vec![artifact],
                    next_step: NextStep::Continue,
                    metadata: crate::phase::PhaseMetadata {
                        packet_hash: None,
                        budget_used: None,
                        duration_ms: None,
                    },
                })
            }
        }

        let phase = EvidencePhase;
        let config = OrchestratorConfig {
            dry_run: true,
            config: HashMap::new(),
            full_config: None,
            selectors: None,
            strict_validation: false,
            redactor: std::sync::Arc::new(crate::redaction::SecretRedactor::default()),
            hooks: None,
        };

        // Execute the phase
        let result = orchestrator.execute_phase(&phase, &config).await;

        assert!(result.is_ok(), "Phase execution should succeed");
        let exec_result = result.unwrap();
        assert!(exec_result.success, "Execution should succeed");

        // Read the receipt and verify packet evidence
        let receipt_path = exec_result.receipt_path.expect("Receipt path should exist");
        let receipt_content = std::fs::read_to_string(&receipt_path).expect("Should read receipt");
        let receipt: serde_json::Value =
            serde_json::from_str(&receipt_content).expect("Should parse receipt");

        // Check that packet (packet evidence) contains our files
        let packet = &receipt["packet"];
        assert!(packet.is_object(), "packet field should exist");

        let files = &packet["files"];
        assert!(files.is_array(), "files should be an array");
        assert_eq!(files.as_array().unwrap().len(), 2, "Should have 2 files");

        // Verify first file
        let first_file = &files[0];
        assert_eq!(first_file["path"], "src/main.rs");
        assert_eq!(first_file["range"], "L1-L100");
        assert_eq!(first_file["blake3_pre_redaction"], "abc123");

        // Verify second file
        let second_file = &files[1];
        assert_eq!(second_file["path"], "Cargo.toml");
        assert_eq!(second_file["range"], "L1-L50");
        assert_eq!(second_file["blake3_pre_redaction"], "def456");
    }
}

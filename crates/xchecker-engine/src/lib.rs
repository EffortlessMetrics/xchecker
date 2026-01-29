// Re-export shared crates to preserve existing `crate::` paths in engine modules.
pub use xchecker_config as config;
pub use xchecker_llm as llm;

pub use xchecker_benchmark as benchmark;
pub use xchecker_doctor as doctor;
pub use xchecker_extraction as extraction;
pub use xchecker_fixup_model as fixup_model;
pub use xchecker_gate as gate;
pub use xchecker_hooks as hooks;
pub use xchecker_packet as packet;
pub use xchecker_receipt as receipt;
pub use xchecker_redaction as redaction;
pub use xchecker_runner as runner;
pub use xchecker_status as status;
pub use xchecker_templates as templates;
pub use xchecker_utils::atomic_write;
pub use xchecker_utils::cache;
pub use xchecker_utils::canonicalization;
pub use xchecker_utils::error;
pub use xchecker_utils::exit_codes;
pub use xchecker_utils::lock;
pub use xchecker_utils::logging;
pub use xchecker_utils::paths;
pub use xchecker_utils::process_memory;
pub use xchecker_utils::ring_buffer;
pub use xchecker_utils::source;
pub use xchecker_utils::spec_id;
#[cfg(any(test, feature = "test-utils"))]
pub use xchecker_utils::test_support;
pub use xchecker_utils::types;
pub use xchecker_validation as validation;
pub use xchecker_workspace as workspace;

#[cfg(any(test, feature = "legacy_claude"))]
pub mod claude;
pub mod example_generators;
pub mod fixup;
pub mod integration_tests;
pub mod orchestrator;

// Re-export orchestrator types for backward compatibility
pub use orchestrator::{OrchestratorConfig, OrchestratorHandle};
pub use xchecker_phase_api as phase;
pub use xchecker_phases as phases;

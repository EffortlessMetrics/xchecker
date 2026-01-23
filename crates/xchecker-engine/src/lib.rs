// Re-export shared crates to preserve existing `crate::` paths in engine modules.
pub use xchecker_config as config;
pub use xchecker_llm as llm;

pub use xchecker_utils::atomic_write;
pub use xchecker_utils::cache;
pub use xchecker_utils::canonicalization;
pub use xchecker_utils::error;
pub use xchecker_utils::exit_codes;
pub use xchecker_utils::lock;
pub use xchecker_utils::logging;
pub use xchecker_utils::paths;
pub use xchecker_utils::process_memory;
pub use xchecker_utils::redaction;
pub use xchecker_utils::ring_buffer;
pub use xchecker_utils::runner;
pub use xchecker_utils::source;
pub use xchecker_utils::spec_id;
#[cfg(any(test, feature = "test-utils"))]
pub use xchecker_utils::test_support;
pub use xchecker_utils::types;

pub mod artifact;
pub mod benchmark;
#[cfg(any(test, feature = "legacy_claude"))]
pub mod claude;
pub mod doctor;
pub mod example_generators;
pub mod extraction;
pub mod fixup;
pub mod gate;
pub mod hooks;
pub mod integration_tests;
pub mod orchestrator;
pub mod packet;
pub mod phase;
pub mod phases;
pub mod receipt;
pub mod status;
pub mod template;
pub mod validation;
pub mod workspace;
pub mod wsl;

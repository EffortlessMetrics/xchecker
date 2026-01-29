//! Gate command for policy-based spec validation

pub mod command;
pub mod exit_codes;
pub mod json;
pub mod paths;
pub mod pending_fixups;
pub mod policy;
pub mod types;

// Re-exports for convenience
pub use command::GateCommand;
pub use exit_codes::{POLICY_VIOLATION, SUCCESS};
pub use json::emit_gate_json;
pub use policy::{
    GatePolicy, load_policy_from_path, parse_duration, parse_phase, resolve_policy_path,
};
pub use types::{
    GateCondition, GateResult, PendingFixupsResult, PendingFixupsStats, SpecDataProvider,
};

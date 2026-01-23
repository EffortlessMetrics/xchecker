//! Runner abstraction for cross-platform Claude CLI execution
//!
//! Provides automatic detection and execution of Claude CLI across Windows, WSL, and native environments.
//! Supports automatic detection (try native first, then WSL on Windows) and explicit mode selection.
//!
//! # Security Model
//!
//! All process execution goes through [`CommandSpec`] to ensure argv-style invocation.
//! This prevents shell injection attacks by ensuring arguments are passed as discrete
//! elements rather than shell strings.

mod claude;
mod command_spec;
mod native;
mod ndjson;
mod process;
mod wsl;

pub use crate::types::RunnerMode;

pub use claude::{BufferConfig, ClaudeResponse, NdjsonResult, Runner, WslOptions};
pub use command_spec::CommandSpec;
pub use native::NativeRunner;
pub use process::{ProcessOutput, ProcessRunner};
pub use wsl::WslRunner;

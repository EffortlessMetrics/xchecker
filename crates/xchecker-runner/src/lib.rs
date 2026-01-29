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

// Declare runner submodules
pub mod claude;
pub mod command_spec;
pub mod error;
pub mod native;
pub mod ndjson;
pub mod process;
pub mod ring_buffer;
pub mod types;
pub mod wsl;

// Re-export everything from xchecker-runner submodules
pub use claude::{BufferConfig, ClaudeResponse, NdjsonResult, Runner, WslOptions};
pub use command_spec::CommandSpec;
pub use error::RunnerError;
pub use native::NativeRunner;
pub use process::{ProcessOutput, ProcessRunner};
pub use ring_buffer::RingBuffer;
pub use types::RunnerMode;
pub use wsl::WslRunner;

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

// Re-export everything from xchecker-runner
pub use xchecker_runner::*;

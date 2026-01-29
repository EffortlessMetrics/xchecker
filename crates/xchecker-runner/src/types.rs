//! Types used by the runner module

use serde::{Deserialize, Serialize};

/// Runner modes for cross-platform Claude CLI execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunnerMode {
    /// Automatic detection (try native first, then WSL on Windows)
    Auto,
    /// Native execution (spawn claude directly)
    Native,
    /// WSL execution (use wsl.exe --exec on Windows)
    Wsl,
}

impl RunnerMode {
    /// Convert runner mode to string representation.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Native => "native",
            Self::Wsl => "wsl",
        }
    }
}

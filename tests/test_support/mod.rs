//! Shared test utilities for integration tests.
//!
//! This module is included via `#[path = "test_support/mod.rs"]` in multiple test files.
//! Not all functions are used in every test file, so we allow dead_code globally.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

pub(crate) fn should_run_e2e() -> bool {
    if std::env::var_os("XCHECKER_E2E").is_none() {
        return false;
    }

    claude_stub_path().is_some()
}

/// Guard that restores the current working directory on drop.
/// Use this in tests that call `std::env::set_current_dir` to prevent
/// CWD-related failures in parallel tests.
pub(crate) struct CwdGuard(PathBuf);

impl CwdGuard {
    /// Change to the specified directory and return a guard that restores the original CWD on drop.
    pub fn new(to: &Path) -> std::io::Result<Self> {
        let prev = std::env::current_dir()?;
        std::env::set_current_dir(to)?;
        Ok(Self(prev))
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.0);
    }
}

/// Guard that restores an environment variable on drop.
/// Use this in tests that modify env vars to prevent pollution between tests.
pub(crate) struct EnvVarGuard {
    key: String,
    original: Option<String>,
}

impl EnvVarGuard {
    /// Set an environment variable and return a guard that restores the original value on drop.
    pub fn set(key: &str, value: &str) -> Self {
        let original = std::env::var(key).ok();
        // SAFETY: Tests serialize access via --test-threads=1 and restore the prior value on drop.
        unsafe {
            std::env::set_var(key, value);
        }

        Self {
            key: key.to_string(),
            original,
        }
    }

    /// Clear an environment variable and return a guard that restores the original value on drop.
    pub fn cleared(key: &str) -> Self {
        let original = std::env::var(key).ok();
        // SAFETY: Tests serialize access via --test-threads=1 and restore the prior value on drop.
        unsafe {
            std::env::remove_var(key);
        }

        Self {
            key: key.to_string(),
            original,
        }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        // SAFETY: Restoring env var to prior state; tests run single-threaded.
        match &self.original {
            Some(value) => unsafe { std::env::set_var(&self.key, value) },
            None => unsafe { std::env::remove_var(&self.key) },
        }
    }
}

pub(crate) fn claude_stub_path() -> Option<String> {
    // Cargo converts hyphens to underscores in CARGO_BIN_EXE_* env vars.
    // So `claude-stub` binary becomes `CARGO_BIN_EXE_claude_stub`.
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_claude_stub") {
        return Some(path);
    }

    // Back-compat: check hyphen form in case older scripts set it directly
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_claude-stub") {
        return Some(path);
    }

    // PATH fallback: only check for claude-stub binary
    // Do NOT fall back to real `claude` - tests using the stub rely on stub-specific scenarios
    which::which("claude-stub")
        .ok()
        .map(|path| path.to_string_lossy().to_string())
}

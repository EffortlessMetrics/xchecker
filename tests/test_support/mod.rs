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
///
/// Use this in tests that modify env vars to prevent pollution between tests.
/// Tests using this guard MUST be annotated with `#[serial]` from `serial_test`
/// to ensure no concurrent access to the process environment.
///
/// # Safety Rationale
///
/// In Rust 2024, `std::env::set_var` and `std::env::remove_var` are `unsafe`
/// because the process environment is shared global state that can be accessed
/// from multiple threads. This guard encapsulates the unsafe operations and
/// ensures the original value is restored on drop (including during panics).
///
/// The caller is responsible for ensuring tests run serially via `#[serial]`.
///
/// # Note
///
/// For tests that can access the main crate, prefer `xchecker::test_support::EnvVarGuard`
/// which is the canonical implementation. This local version exists for integration tests
/// that include this module via `#[path = "test_support/mod.rs"]`.
pub(crate) struct EnvVarGuard {
    key: String,
    original: Option<std::ffi::OsString>,
}

impl EnvVarGuard {
    /// Set an environment variable and return a guard that restores the original value on drop.
    pub fn set(key: &str, value: &str) -> Self {
        let original = std::env::var_os(key);
        // SAFETY: Caller ensures test serialization via #[serial]. Value restored on drop.
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
        let original = std::env::var_os(key);
        // SAFETY: Caller ensures test serialization via #[serial]. Value restored on drop.
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
        // SAFETY: Restoring env var to prior state; caller ensured serialization.
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

    // Fallback: Check target directories relative to CARGO_MANIFEST_DIR
    // This is needed when running tests where the binary was built but not as part of the current
    // test package target set (e.g. running integration tests without --features dev-tools).
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let root = PathBuf::from(manifest_dir);
        // Try common target locations
        let candidates = [
            root.join("target/debug/claude-stub"),
            root.join("target/debug/claude-stub.exe"),
            root.join("target/release/claude-stub"),
            root.join("target/release/claude-stub.exe"),
            // Handle workspace layouts where target might be up a level
            root.join("../target/debug/claude-stub"),
            root.join("../target/debug/claude-stub.exe"),
            root.join("../target/release/claude-stub"),
            root.join("../target/release/claude-stub.exe"),
        ];

        for candidate in &candidates {
            if candidate.exists() {
                return Some(candidate.to_string_lossy().to_string());
            }
        }
    }

    // PATH fallback: only check for claude-stub binary
    // Do NOT fall back to real `claude` - tests using the stub rely on stub-specific scenarios
    if let Ok(path) = which::which("claude-stub") {
        return Some(path.to_string_lossy().to_string());
    }

    // Fallback: check target/debug/claude-stub
    // This is useful in CI environments where the binary might not be in PATH
    // We walk up the directory tree to find the project root
    let current_dir = std::env::current_dir().ok()?;
    let mut dir = current_dir.as_path();
    loop {
        let debug_path = dir.join("target/debug/claude-stub");
        if debug_path.exists() {
            return Some(debug_path.to_string_lossy().to_string());
        }
        let release_path = dir.join("target/release/claude-stub");
        if release_path.exists() {
            return Some(release_path.to_string_lossy().to_string());
        }

        match dir.parent() {
            Some(parent) => dir = parent,
            None => break,
        }
    }

    None
}

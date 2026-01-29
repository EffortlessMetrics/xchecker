//! Path utilities for gate operations
//!
//! This module provides path resolution functions for gate operations.

use std::path::PathBuf;

/// Get the root directory for a spec
///
/// Returns the path to the spec directory under .xchecker/specs/.
pub fn spec_root(spec_id: &str) -> PathBuf {
    // Get XCHECKER_HOME or default to .xchecker in current directory
    let xchecker_home = std::env::var("XCHECKER_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(".xchecker")
        });

    xchecker_home.join("specs").join(spec_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use xchecker_utils::test_support::EnvVarGuard;

    #[test]
    #[serial]
    fn test_spec_root_default() {
        let _guard = EnvVarGuard::cleared("XCHECKER_HOME");

        let path = spec_root("test-spec");
        assert!(path.ends_with(".xchecker/specs/test-spec"));
    }

    #[test]
    #[serial]
    fn test_spec_root_with_env() {
        let _guard = EnvVarGuard::set("XCHECKER_HOME", "/custom/home");

        let path = spec_root("test-spec");
        assert!(path.starts_with("/custom/home"));
        assert!(path.ends_with("specs/test-spec"));
    }
}

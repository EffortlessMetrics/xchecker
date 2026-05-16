//! Filesystem helpers shared by lock implementations.

use anyhow::Result;
use camino::Utf8PathBuf;
use std::cell::RefCell;
use std::fs;
use std::io;

// Thread-local override used only in tests to avoid process-global env races.
thread_local! {
    static THREAD_HOME: RefCell<Option<Utf8PathBuf>> = const { RefCell::new(None) };
}

pub(crate) fn write_file_atomic(path: &Utf8PathBuf, content: &str) -> Result<(), io::Error> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "No parent directory"))?;

    // Ensure parent directory exists
    fs::create_dir_all(parent)?;

    // Create temporary file in the same directory
    let temp_path = parent.join(format!(".{}.tmp", path.file_name().unwrap_or("file")));

    // Write content to temporary file
    fs::write(&temp_path, content)?;

    // Atomically rename temporary file to target path
    fs::rename(&temp_path, path)?;

    Ok(())
}

/// Get the spec root directory for a given spec ID
///
/// This is a simplified version of paths::spec_root that doesn't depend on xchecker-utils
pub(crate) fn xchecker_home() -> Utf8PathBuf {
    if let Some(tl) = THREAD_HOME.with(|tl| tl.borrow().clone()) {
        return tl;
    }
    if let Ok(p) = std::env::var("XCHECKER_HOME") {
        return Utf8PathBuf::from(p);
    }
    Utf8PathBuf::from(".xchecker")
}

/// Get the spec root directory for a given spec ID
///
/// This mirrors xchecker-utils path resolution to keep lock paths consistent.
pub(crate) fn spec_root(spec_id: &str) -> Utf8PathBuf {
    xchecker_home().join("specs").join(spec_id)
}

/// Ensure a directory exists, creating it if necessary
///
/// This is a simplified version of paths::ensure_dir_all that doesn't depend on xchecker-utils
pub(crate) fn ensure_dir_all(path: &Utf8PathBuf) -> Result<(), io::Error> {
    if !path.as_std_path().exists() {
        fs::create_dir_all(path.as_std_path())?;
    }
    Ok(())
}

/// Set a thread-local override for XCHECKER_HOME during tests.
#[cfg(any(test, feature = "test-utils"))]
#[allow(dead_code)]
pub fn set_thread_home_for_tests(path: Utf8PathBuf) {
    THREAD_HOME.with(|tl| *tl.borrow_mut() = Some(path));
}

/// Set up an isolated home directory for testing.
///
/// This avoids process-global environment changes by using thread-local state.
#[cfg(test)]
pub fn with_isolated_home() -> tempfile::TempDir {
    let td = tempfile::TempDir::new().expect("Failed to create temp dir");
    let p = Utf8PathBuf::from_path_buf(td.path().to_path_buf()).unwrap();
    set_thread_home_for_tests(p);
    td
}

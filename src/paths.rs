use camino::Utf8PathBuf;
use std::cell::RefCell;

// Thread-local override used only in tests to avoid process-global env races.
thread_local! {
    static THREAD_HOME: RefCell<Option<Utf8PathBuf>> = const { RefCell::new(None) };
}

/// Resolve xchecker home:
/// 1) thread-local override (tests use this)
/// 2) env `XCHECKER_HOME` (opt-in for users/CI)
/// 3) default ".xchecker"
#[must_use]
pub fn xchecker_home() -> Utf8PathBuf {
    if let Some(tl) = THREAD_HOME.with(|tl| tl.borrow().clone()) {
        return tl;
    }
    if let Ok(p) = std::env::var("XCHECKER_HOME") {
        return Utf8PathBuf::from(p);
    }
    Utf8PathBuf::from(".xchecker")
}

/// <home>/specs/<`spec_id`>
#[must_use]
pub fn spec_root(spec_id: &str) -> Utf8PathBuf {
    xchecker_home().join("specs").join(spec_id)
}

/// <home>/cache
#[must_use]
pub fn cache_dir() -> Utf8PathBuf {
    xchecker_home().join("cache")
}

/// mkdir -p; treat `AlreadyExists` as success (removes TOCTTOU races)
pub fn ensure_dir_all<P: AsRef<std::path::Path>>(p: P) -> std::io::Result<()> {
    match std::fs::create_dir_all(&p) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(e),
    }
}

/// Test helper: provides isolated workspace testing; not part of public API stability guarantees.
///
/// Give this test a unique home under the system temp dir.
/// Hold the `TempDir` for the test's duration so the directory stays alive.
#[cfg(any(test, feature = "test-utils"))]
#[cfg_attr(not(test), allow(dead_code))]
#[must_use]
pub fn with_isolated_home() -> tempfile::TempDir {
    let td = tempfile::TempDir::new().expect("create temp home");
    let p = Utf8PathBuf::from_path_buf(td.path().to_path_buf()).unwrap();
    THREAD_HOME.with(|tl| *tl.borrow_mut() = Some(p));
    td
}

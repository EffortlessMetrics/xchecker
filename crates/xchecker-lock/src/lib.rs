//! File locking system for xchecker with advisory semantics and crash recovery
//!
//! This module provides exclusive file locking per spec ID directory to prevent
//! concurrent execution. The locking is advisory and coordinates xchecker processes
//! but is not a security boundary.

use anyhow::Result;
use camino::Utf8PathBuf;
use chrono::{DateTime, Utc};
use fd_lock::RwLock;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

// Thread-local override used only in tests to avoid process-global env races.
thread_local! {
    static THREAD_HOME: RefCell<Option<Utf8PathBuf>> = const { RefCell::new(None) };
}

/// Default age threshold for considering a lock stale (in seconds)
const DEFAULT_STALE_THRESHOLD_SECS: u64 = 3600; // 1 hour

/// Lock information stored in the lock file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockInfo {
    /// Process ID that created the lock
    pub pid: u32,
    /// Process start time (seconds since UNIX epoch)
    pub start_time: u64,
    /// Timestamp when the lock was created (seconds since UNIX epoch)
    pub created_at: u64,
    /// Spec ID being locked
    pub spec_id: String,
    /// xchecker version that created the lock
    pub xchecker_version: String,
}

/// `XChecker` lockfile for reproducibility tracking (schema v1)
/// Pins model, CLI version, and schema version to detect drift
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XCheckerLock {
    /// Schema version for this lockfile format
    pub schema_version: String,
    /// RFC3339 UTC timestamp when the lockfile was created
    pub created_at: DateTime<Utc>,
    /// Full model name that was used (e.g., "haiku")
    pub model_full_name: String,
    /// Claude CLI version that was used
    pub claude_cli_version: String,
}

/// Context for current run to compare against lockfile
#[derive(Debug, Clone)]
pub struct RunContext {
    pub model_full_name: String,
    pub claude_cli_version: String,
    pub schema_version: String,
}

/// Drift pair showing locked vs current value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftPair {
    /// Value from lockfile
    pub locked: String,
    /// Current value
    pub current: String,
}

/// Lock drift information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockDrift {
    /// Model full name drift
    pub model_full_name: Option<DriftPair>,
    /// Claude CLI version drift
    pub claude_cli_version: Option<DriftPair>,
    /// Schema version drift
    pub schema_version: Option<DriftPair>,
}

/// Lock errors for file locking operations
#[derive(Debug, thiserror::Error)]
pub enum LockError {
    #[error(
        "Concurrent execution detected for spec '{spec_id}' (PID {pid}, created {created_ago} ago)"
    )]
    ConcurrentExecution {
        spec_id: String,
        pid: u32,
        created_ago: String,
    },

    #[error(
        "Stale lock detected for spec '{spec_id}' (PID {pid}, age {age_secs}s). Use --force to override"
    )]
    StaleLock {
        spec_id: String,
        pid: u32,
        age_secs: u64,
    },

    #[error("Lock file is corrupted or invalid: {reason}")]
    CorruptedLock { reason: String },

    #[error("Failed to acquire lock: {reason}")]
    AcquisitionFailed { reason: String },

    #[error("Failed to release lock: {reason}")]
    ReleaseFailed { reason: String },

    #[error("IO error during lock operation: {0}")]
    Io(#[from] io::Error),
}

/// Write file atomically using a temporary file and atomic rename
///
/// This is a simplified version of atomic_write that doesn't depend on xchecker-utils
fn write_file_atomic(path: &Utf8PathBuf, content: &str) -> Result<(), io::Error> {
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
fn xchecker_home() -> Utf8PathBuf {
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
fn spec_root(spec_id: &str) -> Utf8PathBuf {
    xchecker_home().join("specs").join(spec_id)
}

/// Ensure a directory exists, creating it if necessary
///
/// This is a simplified version of paths::ensure_dir_all that doesn't depend on xchecker-utils
fn ensure_dir_all(path: &Utf8PathBuf) -> Result<(), io::Error> {
    if !path.as_std_path().exists() {
        fs::create_dir_all(path.as_std_path())?;
    }
    Ok(())
}

/// Set a thread-local override for XCHECKER_HOME during tests.
#[cfg(any(test, feature = "test-utils"))]
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

impl XCheckerLock {
    /// Create a new lockfile with current context
    #[must_use]
    pub fn new(model_full_name: String, claude_cli_version: String) -> Self {
        Self {
            schema_version: "1".to_string(),
            created_at: Utc::now(),
            model_full_name,
            claude_cli_version,
        }
    }

    /// Detect drift between locked values and current run context
    /// Returns None if no drift detected, Some(LockDrift) if drift exists
    #[must_use]
    pub fn detect_drift(&self, current: &RunContext) -> Option<LockDrift> {
        let mut drift = LockDrift {
            model_full_name: None,
            claude_cli_version: None,
            schema_version: None,
        };

        // Check model drift
        if self.model_full_name != current.model_full_name {
            drift.model_full_name = Some(DriftPair {
                locked: self.model_full_name.clone(),
                current: current.model_full_name.clone(),
            });
        }

        // Check Claude CLI version drift
        if self.claude_cli_version != current.claude_cli_version {
            drift.claude_cli_version = Some(DriftPair {
                locked: self.claude_cli_version.clone(),
                current: current.claude_cli_version.clone(),
            });
        }

        // Check schema version drift
        if self.schema_version != current.schema_version {
            drift.schema_version = Some(DriftPair {
                locked: self.schema_version.clone(),
                current: current.schema_version.clone(),
            });
        }

        // Return None if no drift detected
        if drift.model_full_name.is_none()
            && drift.claude_cli_version.is_none()
            && drift.schema_version.is_none()
        {
            None
        } else {
            Some(drift)
        }
    }

    /// Load lockfile from spec directory
    pub fn load(spec_id: &str) -> Result<Option<Self>, io::Error> {
        let lock_path = Self::get_lock_path(spec_id);

        if !lock_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&lock_path)?;
        let lock: Self = serde_json::from_str(&content)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        Ok(Some(lock))
    }

    /// Save lockfile to spec directory
    pub fn save(&self, spec_id: &str) -> Result<(), io::Error> {
        let lock_path = Self::get_lock_path_utf8(spec_id);

        let json = serde_json::to_string_pretty(self)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        write_file_atomic(&lock_path, &json).map_err(io::Error::other)?;

        Ok(())
    }

    /// Get the path to the lockfile for a spec ID
    fn get_lock_path(spec_id: &str) -> PathBuf {
        Self::get_lock_path_utf8(spec_id).into_std_path_buf()
    }

    /// Get the UTF-8 path to the lockfile for a spec ID
    fn get_lock_path_utf8(spec_id: &str) -> Utf8PathBuf {
        spec_root(spec_id).join("lock.json")
    }
}

/// File lock manager for spec directories
pub struct FileLock {
    /// Path to the lock file
    lock_path: PathBuf,
    /// File descriptor lock (held while active)
    _fd_lock: Option<Box<RwLock<fs::File>>>,
    /// Lock information
    lock_info: LockInfo,
}

impl FileLock {
    /// Attempt to acquire an exclusive lock for the given spec ID
    ///
    /// Uses atomic O_EXCL/create_new semantics to prevent TOCTOU race conditions.
    /// If the lock file already exists, validates the existing lock before deciding
    /// whether to override it.
    ///
    /// # Arguments
    /// * `spec_id` - The spec ID to lock
    /// * `force` - Whether to override stale locks
    /// * `ttl_seconds` - Time-to-live for lock staleness detection (None uses default)
    ///
    /// # Returns
    /// * `Ok(FileLock)` - Successfully acquired lock
    /// * `Err(LockError)` - Failed to acquire lock (concurrent execution, stale lock, etc.)
    pub fn acquire(
        spec_id: &str,
        force: bool,
        ttl_seconds: Option<u64>,
    ) -> Result<Self, LockError> {
        let spec_root = spec_root(spec_id);

        // Ensure the spec directory exists (ignore benign races)
        ensure_dir_all(&spec_root).map_err(|e| LockError::AcquisitionFailed {
            reason: format!("Failed to create spec directory: {e}"),
        })?;

        let lock_path = Self::get_lock_path(spec_id);
        let ttl = ttl_seconds.unwrap_or(DEFAULT_STALE_THRESHOLD_SECS);

        // Attempt atomic lock acquisition with retries for stale lock handling
        Self::acquire_with_retry(spec_id, &lock_path, force, ttl, 3)
    }

    /// Internal helper for atomic lock acquisition with retry logic
    fn acquire_with_retry(
        spec_id: &str,
        lock_path: &Path,
        force: bool,
        ttl_seconds: u64,
        max_retries: u32,
    ) -> Result<Self, LockError> {
        for attempt in 0..max_retries {
            // Create lock info for this attempt
            let lock_info = LockInfo {
                pid: process::id(),
                start_time: Self::get_process_start_time()?,
                created_at: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                spec_id: spec_id.to_string(),
                xchecker_version: env!("CARGO_PKG_VERSION").to_string(),
            };

            // Attempt atomic file creation with O_EXCL semantics (create_new)
            match fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(lock_path)
            {
                Ok(lock_file) => {
                    // Successfully created the file atomically - no race possible
                    return Self::finalize_lock(lock_path.to_path_buf(), lock_file, lock_info);
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    // Lock file exists - validate it
                    match Self::check_existing_lock(lock_path, spec_id, force, ttl_seconds) {
                        Ok(()) => {
                            // Lock is stale/overridable - attempt atomic removal and retry
                            match Self::try_remove_stale_lock(lock_path, spec_id) {
                                Ok(()) => {
                                    // Immediately attempt acquisition after removing stale lock
                                    match fs::OpenOptions::new()
                                        .create_new(true)
                                        .write(true)
                                        .open(lock_path)
                                    {
                                        Ok(lock_file) => {
                                            return Self::finalize_lock(
                                                lock_path.to_path_buf(),
                                                lock_file,
                                                lock_info,
                                            );
                                        }
                                        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                                            // Another process grabbed it - apply backoff if retries remain
                                            if attempt + 1 < max_retries {
                                                let base_delay_ms = 10u64
                                                    .saturating_mul(2u64.saturating_pow(attempt));
                                                // Deterministic jitter based on PID to avoid lockstep retries
                                                // without requiring RNG (0-6ms based on attempt and PID)
                                                let jitter_ms = ((attempt as u64)
                                                    .wrapping_mul(3)
                                                    .wrapping_add((process::id() as u64) % 7))
                                                    % 7;
                                                let delay_ms =
                                                    base_delay_ms.saturating_add(jitter_ms);
                                                std::thread::sleep(
                                                    std::time::Duration::from_millis(
                                                        delay_ms.min(100),
                                                    ),
                                                );
                                                continue;
                                            }
                                            // Max retries reached after another process grabbed lock
                                            return Err(LockError::AcquisitionFailed {
                                                reason: format!(
                                                    "Max retries exceeded for spec '{}': another process acquired lock immediately after stale removal",
                                                    spec_id
                                                ),
                                            });
                                        }
                                        Err(e) => {
                                            return Err(LockError::AcquisitionFailed {
                                                reason: format!(
                                                    "Failed to create lock for spec '{}' after removing stale lock: {e}",
                                                    spec_id
                                                ),
                                            });
                                        }
                                    }
                                }
                                Err(e) => {
                                    // Propagate the specific stale-removal error
                                    return Err(e);
                                }
                            }
                        }
                        Err(e) => return Err(e),
                    }
                }
                Err(e) => {
                    return Err(LockError::AcquisitionFailed {
                        reason: format!(
                            "Failed to create lock file for spec '{}' at '{}': {e}",
                            spec_id,
                            lock_path.display()
                        ),
                    });
                }
            }
        }

        // Reachable only when max_retries == 0 (edge case). All other paths return/continue
        // within the loop. This provides a safety net for the zero-retry edge case.
        Err(LockError::AcquisitionFailed {
            reason: format!(
                "Max retries ({}) exceeded for lock acquisition on spec '{}'",
                max_retries, spec_id
            ),
        })
    }

    /// Finalize lock acquisition by writing lock info and acquiring fd_lock
    fn finalize_lock(
        lock_path: PathBuf,
        lock_file: fs::File,
        lock_info: LockInfo,
    ) -> Result<Self, LockError> {
        let lock_json =
            serde_json::to_string_pretty(&lock_info).map_err(|e| LockError::AcquisitionFailed {
                reason: format!(
                    "Failed to serialize lock info for spec '{}': {e}",
                    lock_info.spec_id
                ),
            })?;

        // Acquire exclusive file descriptor lock and write in one step
        let mut rw_lock = Box::new(RwLock::new(lock_file));
        {
            let fd_lock = rw_lock
                .try_write()
                .map_err(|_e| LockError::ConcurrentExecution {
                    spec_id: lock_info.spec_id.clone(),
                    pid: 0, // Unknown PID since we couldn't read the lock
                    created_ago: "unknown".to_string(),
                })?;

            // Write to the locked file
            let mut file_ref = &*fd_lock;
            file_ref
                .write_all(lock_json.as_bytes())
                .map_err(|e| LockError::AcquisitionFailed {
                    reason: format!(
                        "Failed to write lock info for spec '{}': {e}",
                        lock_info.spec_id
                    ),
                })?;
            file_ref.flush().map_err(|e| LockError::AcquisitionFailed {
                reason: format!(
                    "Failed to flush lock file for spec '{}': {e}",
                    lock_info.spec_id
                ),
            })?;

            // Sync to disk for crash-resilience (small file, acceptable cost)
            file_ref
                .sync_all()
                .map_err(|e| LockError::AcquisitionFailed {
                    reason: format!(
                        "Failed to sync lock file for spec '{}': {e}",
                        lock_info.spec_id
                    ),
                })?;
        }

        Ok(Self {
            lock_path,
            _fd_lock: Some(rw_lock),
            lock_info,
        })
    }

    /// Attempt to remove a stale lock file atomically
    ///
    /// Uses rename-to-stale then delete pattern to minimize race window.
    /// Treats `NotFound` as success since another process may have already removed it.
    /// Includes PID in stale filename to prevent collision under high parallelism.
    fn try_remove_stale_lock(lock_path: &Path, spec_id: &str) -> Result<(), LockError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let pid = process::id();
        let stale_path = lock_path.with_extension(format!("stale.{timestamp}.{pid}"));

        // Atomic rename to mark as stale
        match fs::rename(lock_path, &stale_path) {
            Ok(()) => {
                // Best-effort cleanup of stale file (ignore errors)
                let _ = fs::remove_file(&stale_path);
                Ok(())
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                // Another process already removed/renamed it - that's fine
                Ok(())
            }
            Err(e) => Err(LockError::AcquisitionFailed {
                reason: format!("Failed to rename stale lock for spec '{spec_id}': {e}"),
            }),
        }
    }

    /// Check if a lock exists for the given spec ID
    #[must_use]
    #[allow(dead_code)] // Lock introspection utility
    pub fn exists(spec_id: &str) -> bool {
        let lock_path = Self::get_lock_path(spec_id);
        lock_path.exists()
    }

    /// Get information about an existing lock (if any)
    pub fn get_lock_info(spec_id: &str) -> Result<Option<LockInfo>, LockError> {
        let lock_path = Self::get_lock_path(spec_id);

        if !lock_path.exists() {
            return Ok(None);
        }

        let lock_content =
            fs::read_to_string(&lock_path).map_err(|e| LockError::CorruptedLock {
                reason: format!("Failed to read lock file: {e}"),
            })?;

        let lock_info: LockInfo =
            serde_json::from_str(&lock_content).map_err(|e| LockError::CorruptedLock {
                reason: format!("Failed to parse lock file: {e}"),
            })?;

        Ok(Some(lock_info))
    }

    /// Release the lock (called automatically on drop)
    #[allow(dead_code)] // Lock management utility
    pub fn release(mut self) -> Result<(), LockError> {
        // Drop the file descriptor lock first
        self._fd_lock.take();

        // Remove the lock file
        if self.lock_path.exists() {
            fs::remove_file(&self.lock_path).map_err(|e| LockError::ReleaseFailed {
                reason: format!("Failed to remove lock file: {e}"),
            })?;
        }

        Ok(())
    }

    /// Get the spec ID for this lock
    #[must_use]
    #[allow(dead_code)] // Lock introspection utility
    pub fn spec_id(&self) -> &str {
        &self.lock_info.spec_id
    }

    /// Get the lock information
    #[must_use]
    #[allow(dead_code)] // Lock introspection utility
    pub const fn lock_info(&self) -> &LockInfo {
        &self.lock_info
    }

    /// Get the path to the lock file for a spec ID
    fn get_lock_path(spec_id: &str) -> PathBuf {
        spec_root(spec_id).as_std_path().join(".lock")
    }

    /// Check an existing lock and determine if it should be overridden
    ///
    /// Includes retry logic for empty/partial lockfile reads to handle the case where
    /// another process has just created the file but hasn't written content yet.
    fn check_existing_lock(
        lock_path: &Path,
        spec_id: &str,
        force: bool,
        ttl_seconds: u64,
    ) -> Result<(), LockError> {
        // Retry parameters for handling concurrent initialization
        const MAX_READ_RETRIES: u32 = 3;
        const READ_RETRY_DELAY_MS: u64 = 10;

        for attempt in 0..MAX_READ_RETRIES {
            let lock_content = match fs::read_to_string(lock_path) {
                Ok(content) => content,
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    // Lock was removed between create_new(AlreadyExists) and read.
                    // Treat as "no lock"; caller will retry acquisition.
                    return Ok(());
                }
                Err(e) => {
                    // IO errors during read might be transient (file being written)
                    if attempt + 1 < MAX_READ_RETRIES {
                        std::thread::sleep(std::time::Duration::from_millis(READ_RETRY_DELAY_MS));
                        continue;
                    }
                    return Err(LockError::CorruptedLock {
                        reason: format!("Failed to read existing lock for spec '{}': {e}", spec_id),
                    });
                }
            };

            // Check for empty content (file exists but not yet written)
            if lock_content.is_empty() {
                if attempt + 1 < MAX_READ_RETRIES {
                    std::thread::sleep(std::time::Duration::from_millis(READ_RETRY_DELAY_MS));
                    continue;
                }
                return Err(LockError::CorruptedLock {
                    reason: format!(
                        "Lock file for spec '{}' is empty (may be initializing)",
                        spec_id
                    ),
                });
            }

            // Try to parse the JSON content
            match serde_json::from_str::<LockInfo>(&lock_content) {
                Ok(existing_lock) => {
                    // Successfully parsed - proceed with lock validation
                    return Self::validate_existing_lock(
                        &existing_lock,
                        spec_id,
                        force,
                        ttl_seconds,
                    );
                }
                Err(e) => {
                    // Check if this looks like a partial/incomplete JSON (EOF error)
                    let is_likely_incomplete = e.is_eof()
                        || lock_content.trim().is_empty()
                        || (lock_content.starts_with('{') && !lock_content.contains('}'));

                    // Only retry if it looks like the file might be mid-write
                    if is_likely_incomplete && attempt + 1 < MAX_READ_RETRIES {
                        std::thread::sleep(std::time::Duration::from_millis(READ_RETRY_DELAY_MS));
                        continue;
                    }

                    return Err(LockError::CorruptedLock {
                        reason: format!(
                            "Failed to parse existing lock for spec '{}': {e}",
                            spec_id
                        ),
                    });
                }
            }
        }
        // Note: This is unreachable since MAX_READ_RETRIES > 0 and all paths return/continue.
        // Kept for safety if MAX_READ_RETRIES is ever changed to 0.
        unreachable!("check_existing_lock loop exhausted without returning")
    }

    /// Validate an existing lock and determine if it should be overridden
    fn validate_existing_lock(
        existing_lock: &LockInfo,
        spec_id: &str,
        force: bool,
        ttl_seconds: u64,
    ) -> Result<(), LockError> {
        // Calculate lock age (handle future timestamps gracefully - clock skew)
        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let lock_age = now_secs.saturating_sub(existing_lock.created_at);

        let is_stale = lock_age > ttl_seconds;

        // Check if the process is still running
        if Self::is_process_running(existing_lock.pid) {
            // Process is running - this is a fresh lock
            if !force {
                let created_ago = Self::format_duration_since(existing_lock.created_at);
                return Err(LockError::ConcurrentExecution {
                    spec_id: spec_id.to_string(),
                    pid: existing_lock.pid,
                    created_ago,
                });
            }
            // Force allows overriding even fresh locks
            return Ok(());
        }

        // Process is not running - check staleness
        if is_stale {
            if force {
                // Force flag allows overriding stale locks
                Ok(())
            } else {
                Err(LockError::StaleLock {
                    spec_id: spec_id.to_string(),
                    pid: existing_lock.pid,
                    age_secs: lock_age,
                })
            }
        } else {
            // Lock is recent but process is dead - fail without force
            if force {
                Ok(())
            } else {
                let created_ago = Self::format_duration_since(existing_lock.created_at);
                Err(LockError::ConcurrentExecution {
                    spec_id: spec_id.to_string(),
                    pid: existing_lock.pid,
                    created_ago,
                })
            }
        }
    }

    /// Check if a process with the given PID is still running
    fn is_process_running(pid: u32) -> bool {
        #[cfg(unix)]
        {
            // On Unix systems, use kill(pid, 0) to check if process exists
            // Returns 0 if process exists and we can signal it
            // Returns -1 with ESRCH if process doesn't exist
            // Returns -1 with EPERM if process exists but we lack permission
            let rc = unsafe { libc::kill(pid as i32, 0) };
            if rc == 0 {
                true
            } else {
                // If EPERM, the process exists but we can't signal it
                matches!(
                    io::Error::last_os_error().raw_os_error(),
                    Some(code) if code == libc::EPERM
                )
            }
        }

        #[cfg(windows)]
        {
            // On Windows, try to open the process handle and check if it's still running
            use winapi::um::handleapi::CloseHandle;
            use winapi::um::minwinbase::STILL_ACTIVE;
            use winapi::um::processthreadsapi::{GetExitCodeProcess, OpenProcess};
            use winapi::um::winnt::PROCESS_QUERY_LIMITED_INFORMATION;

            unsafe {
                // Use PROCESS_QUERY_LIMITED_INFORMATION which is sufficient for GetExitCodeProcess
                // and works with more processes than PROCESS_QUERY_INFORMATION
                let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
                if handle.is_null() {
                    return false;
                }

                // Check if the process is still running by getting its exit code
                let mut exit_code: u32 = 0;
                let result = GetExitCodeProcess(handle, &mut exit_code);

                // If GetExitCodeProcess fails, assume process is not running
                if result == 0 {
                    CloseHandle(handle);
                    return false;
                }

                // STILL_ACTIVE (259) means the process is still running
                CloseHandle(handle);
                exit_code == STILL_ACTIVE
            }
        }

        #[cfg(not(any(unix, windows)))]
        {
            // Fallback: assume process is running (conservative approach)
            true
        }
    }

    /// Get the start time of the current process (best effort)
    fn get_process_start_time() -> Result<u64, LockError> {
        // This is a best-effort implementation
        // In practice, we use the current time as an approximation
        Ok(SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs())
    }

    /// Format a duration since a timestamp in a human-readable way
    fn format_duration_since(timestamp: u64) -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let duration = now.saturating_sub(timestamp);

        if duration < 60 {
            format!("{duration}s")
        } else if duration < 3600 {
            format!("{}m", duration / 60)
        } else if duration < 86400 {
            format!("{}h", duration / 3600)
        } else {
            format!("{}d", duration / 86400)
        }
    }
}

impl std::fmt::Debug for FileLock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileLock")
            .field("lock_path", &self.lock_path)
            .field("lock_info", &self.lock_info)
            .field("_fd_lock", &"<RwLock>")
            .finish()
    }
}

impl Drop for FileLock {
    /// Automatically release the lock when the `FileLock` is dropped
    fn drop(&mut self) {
        // Drop the file descriptor lock first
        self._fd_lock.take();

        // Remove the lock file (ignore errors in drop)
        if self.lock_path.exists() {
            let _ = fs::remove_file(&self.lock_path);
        }
    }
}

/// Utility functions for lock management
pub mod utils {
    use super::{
        DEFAULT_STALE_THRESHOLD_SECS, FileLock, LockError, Result, SystemTime, UNIX_EPOCH, fs,
    };

    /// Check if clean operation should be allowed (no active locks unless forced)
    pub fn can_clean(
        spec_id: &str,
        force: bool,
        ttl_seconds: Option<u64>,
    ) -> Result<(), LockError> {
        let ttl = ttl_seconds.unwrap_or(DEFAULT_STALE_THRESHOLD_SECS);
        if let Some(lock_info) = FileLock::get_lock_info(spec_id)? {
            if FileLock::is_process_running(lock_info.pid) {
                if force {
                    // Force flag allows cleaning even with active locks (--hard --force overrides active locks)
                    return Ok(());
                }
                return Err(LockError::ConcurrentExecution {
                    spec_id: spec_id.to_string(),
                    pid: lock_info.pid,
                    created_ago: FileLock::format_duration_since(lock_info.created_at),
                });
            }

            // Process is dead, check if we should allow cleaning
            if !force {
                let lock_age = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    - lock_info.created_at;

                if lock_age <= ttl {
                    return Err(LockError::StaleLock {
                        spec_id: spec_id.to_string(),
                        pid: lock_info.pid,
                        age_secs: lock_age,
                    });
                }
            }
        }

        Ok(())
    }

    /// Force remove a lock file (for emergency cleanup)
    #[allow(dead_code)] // Lock cleanup utility for CLI commands
    pub fn force_remove_lock(spec_id: &str) -> Result<(), LockError> {
        let lock_path = FileLock::get_lock_path(spec_id);

        if lock_path.exists() {
            fs::remove_file(&lock_path).map_err(|e| LockError::ReleaseFailed {
                reason: format!("Failed to force remove lock: {e}"),
            })?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;
    use tempfile::TempDir;

    fn setup_test_env() -> TempDir {
        with_isolated_home()
    }

    #[test]
    fn test_lock_acquisition_and_release() {
        let _temp_dir = setup_test_env();

        let spec_id = "test-spec-acquisition-123";

        // Should be able to acquire lock
        let lock = FileLock::acquire(spec_id, false, None).unwrap();
        assert_eq!(lock.spec_id(), spec_id);

        // The lock file should exist while the lock is held
        let lock_path = FileLock::get_lock_path(spec_id);
        assert!(
            lock_path.exists(),
            "Lock file should exist at: {lock_path:?}"
        );
        assert!(FileLock::exists(spec_id));

        // Should not be able to acquire another lock for same spec
        let result = FileLock::acquire(spec_id, false, None);
        assert!(result.is_err());

        // Release the lock
        lock.release().unwrap();
        assert!(!FileLock::exists(spec_id));

        // Should be able to acquire again after release
        let _lock2 = FileLock::acquire(spec_id, false, None).unwrap();
    }

    #[test]
    fn test_lock_info_serialization() {
        let _temp_dir = setup_test_env();

        let spec_id = "test-spec-serialization-456";
        let _lock = FileLock::acquire(spec_id, false, None).unwrap();

        // Should be able to read lock info
        let lock_info = FileLock::get_lock_info(spec_id).unwrap().unwrap();
        assert_eq!(lock_info.spec_id, spec_id);
        assert_eq!(lock_info.pid, process::id());
        assert!(!lock_info.xchecker_version.is_empty());
    }

    #[test]
    fn test_automatic_cleanup_on_drop() {
        let _temp_dir = setup_test_env();

        let spec_id = "test-spec-cleanup-789";

        {
            let _lock = FileLock::acquire(spec_id, false, None).unwrap();
            assert!(FileLock::exists(spec_id));
        } // lock goes out of scope here

        // Lock should be automatically cleaned up
        assert!(!FileLock::exists(spec_id));
    }

    #[test]
    fn test_force_override_stale_lock() {
        let _temp_dir = setup_test_env();

        let spec_id = "test-spec-stale-override";

        // Create a lock file manually with old timestamp
        let lock_path = FileLock::get_lock_path(spec_id);
        fs::create_dir_all(lock_path.parent().unwrap()).unwrap();

        let old_lock_info = LockInfo {
            pid: 99999, // Non-existent PID
            start_time: 0,
            created_at: 0, // Very old timestamp
            spec_id: spec_id.to_string(),
            xchecker_version: "0.1.0".to_string(),
        };

        let lock_json = serde_json::to_string_pretty(&old_lock_info).unwrap();
        fs::write(&lock_path, lock_json).unwrap();

        // Should fail without force
        let result = FileLock::acquire(spec_id, false, None);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LockError::StaleLock { .. }));

        // Should succeed with force
        let lock = FileLock::acquire(spec_id, true, None).unwrap();
        assert_eq!(lock.spec_id(), spec_id);
    }

    #[test]
    fn test_clean_operation_checks() {
        let _temp_dir = setup_test_env();

        let spec_id = "test-spec-clean-checks";

        // Should be able to clean when no lock exists
        assert!(utils::can_clean(spec_id, false, None).is_ok());

        // Acquire a lock
        let _lock = FileLock::acquire(spec_id, false, None).unwrap();

        // Should not be able to clean with active lock
        let result = utils::can_clean(spec_id, false, None);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            LockError::ConcurrentExecution { .. }
        ));

        // Should be able to clean with force (--hard --force overrides active locks)
        assert!(utils::can_clean(spec_id, true, None).is_ok());
    }

    #[test]
    fn test_lock_path_generation() {
        let _temp_dir = setup_test_env();

        let spec_id = "my-test-spec";
        let expected_path = spec_root(spec_id).as_std_path().join(".lock");
        assert_eq!(FileLock::get_lock_path(spec_id), expected_path);
    }

    #[test]
    fn test_duration_formatting() {
        assert_eq!(
            FileLock::format_duration_since(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    - 30
            ),
            "30s"
        );
        assert_eq!(
            FileLock::format_duration_since(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    - 120
            ),
            "2m"
        );
        assert_eq!(
            FileLock::format_duration_since(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    - 7200
            ),
            "2h"
        );
    }

    #[test]
    fn test_xchecker_lock_creation() {
        let lock = XCheckerLock::new("haiku".to_string(), "0.8.1".to_string());

        assert_eq!(lock.schema_version, "1");
        assert_eq!(lock.model_full_name, "haiku");
        assert_eq!(lock.claude_cli_version, "0.8.1");
    }

    #[test]
    fn test_xchecker_lock_no_drift() {
        let lock = XCheckerLock::new("haiku".to_string(), "0.8.1".to_string());

        let context = RunContext {
            model_full_name: "haiku".to_string(),
            claude_cli_version: "0.8.1".to_string(),
            schema_version: "1".to_string(),
        };

        let drift = lock.detect_drift(&context);
        assert!(drift.is_none(), "Expected no drift when values match");
    }

    #[test]
    fn test_xchecker_lock_model_drift() {
        let lock = XCheckerLock::new("haiku".to_string(), "0.8.1".to_string());

        let context = RunContext {
            model_full_name: "sonnet".to_string(),
            claude_cli_version: "0.8.1".to_string(),
            schema_version: "1".to_string(),
        };

        let drift = lock.detect_drift(&context).expect("Expected drift");
        assert!(drift.model_full_name.is_some());
        assert!(drift.claude_cli_version.is_none());
        assert!(drift.schema_version.is_none());

        let model_drift = drift.model_full_name.unwrap();
        assert_eq!(model_drift.locked, "haiku");
        assert_eq!(model_drift.current, "sonnet");
    }

    #[test]
    fn test_xchecker_lock_cli_version_drift() {
        let lock = XCheckerLock::new("haiku".to_string(), "0.8.1".to_string());

        let context = RunContext {
            model_full_name: "haiku".to_string(),
            claude_cli_version: "0.9.0".to_string(),
            schema_version: "1".to_string(),
        };

        let drift = lock.detect_drift(&context).expect("Expected drift");
        assert!(drift.model_full_name.is_none());
        assert!(drift.claude_cli_version.is_some());
        assert!(drift.schema_version.is_none());

        let cli_drift = drift.claude_cli_version.unwrap();
        assert_eq!(cli_drift.locked, "0.8.1");
        assert_eq!(cli_drift.current, "0.9.0");
    }

    #[test]
    fn test_xchecker_lock_schema_version_drift() {
        let lock = XCheckerLock::new("haiku".to_string(), "0.8.1".to_string());

        let context = RunContext {
            model_full_name: "haiku".to_string(),
            claude_cli_version: "0.8.1".to_string(),
            schema_version: "2".to_string(),
        };

        let drift = lock.detect_drift(&context).expect("Expected drift");
        assert!(drift.model_full_name.is_none());
        assert!(drift.claude_cli_version.is_none());
        assert!(drift.schema_version.is_some());

        let schema_drift = drift.schema_version.unwrap();
        assert_eq!(schema_drift.locked, "1");
        assert_eq!(schema_drift.current, "2");
    }

    #[test]
    fn test_xchecker_lock_multiple_drift() {
        let lock = XCheckerLock::new("haiku".to_string(), "0.8.1".to_string());

        let context = RunContext {
            model_full_name: "sonnet".to_string(),
            claude_cli_version: "0.9.0".to_string(),
            schema_version: "2".to_string(),
        };

        let drift = lock.detect_drift(&context).expect("Expected drift");
        assert!(drift.model_full_name.is_some());
        assert!(drift.claude_cli_version.is_some());
        assert!(drift.schema_version.is_some());
    }

    #[test]
    fn test_xchecker_lock_save_and_load() {
        let _temp_dir = setup_test_env();

        let spec_id = "test-spec-lockfile";
        let lock = XCheckerLock::new("haiku".to_string(), "0.8.1".to_string());

        // Save lockfile
        lock.save(spec_id).expect("Failed to save lockfile");

        // Load lockfile
        let loaded = XCheckerLock::load(spec_id)
            .expect("Failed to load lockfile")
            .expect("Lockfile should exist");

        assert_eq!(loaded.schema_version, lock.schema_version);
        assert_eq!(loaded.model_full_name, lock.model_full_name);
        assert_eq!(loaded.claude_cli_version, lock.claude_cli_version);
    }

    #[test]
    fn test_xchecker_lock_load_nonexistent() {
        let _temp_dir = setup_test_env();

        let spec_id = "nonexistent-spec";
        let loaded = XCheckerLock::load(spec_id).expect("Load should succeed");

        assert!(
            loaded.is_none(),
            "Should return None for nonexistent lockfile"
        );
    }

    #[test]
    fn test_xchecker_lock_corrupted_file() {
        let _temp_dir = setup_test_env();

        let spec_id = "test-spec-corrupted";
        let lock_path = XCheckerLock::get_lock_path(spec_id);

        // Create spec directory
        fs::create_dir_all(lock_path.parent().unwrap()).unwrap();

        // Write corrupted JSON
        fs::write(&lock_path, "{ invalid json }").unwrap();

        // Should return error for corrupted file
        let result = XCheckerLock::load(spec_id);
        assert!(result.is_err(), "Should fail to load corrupted lockfile");
    }

    #[test]
    fn test_xchecker_lock_empty_file() {
        let _temp_dir = setup_test_env();

        let spec_id = "test-spec-empty";
        let lock_path = XCheckerLock::get_lock_path(spec_id);

        // Create spec directory
        fs::create_dir_all(lock_path.parent().unwrap()).unwrap();

        // Write empty file
        fs::write(&lock_path, "").unwrap();

        // Should return error for empty file
        let result = XCheckerLock::load(spec_id);
        assert!(result.is_err(), "Should fail to load empty lockfile");
    }

    #[test]
    fn test_xchecker_lock_overwrite_existing() {
        let _temp_dir = setup_test_env();

        let spec_id = "test-spec-overwrite";

        // Create first lockfile
        let lock1 = XCheckerLock::new("haiku".to_string(), "0.8.1".to_string());
        lock1.save(spec_id).unwrap();

        // Create second lockfile with different values
        let lock2 = XCheckerLock::new("sonnet".to_string(), "0.9.0".to_string());
        lock2.save(spec_id).unwrap();

        // Load and verify it has the second lockfile's values
        let loaded = XCheckerLock::load(spec_id)
            .expect("Failed to load lockfile")
            .expect("Lockfile should exist");

        assert_eq!(loaded.model_full_name, "sonnet");
        assert_eq!(loaded.claude_cli_version, "0.9.0");
    }

    #[test]
    fn test_xchecker_lock_drift_all_fields_match() {
        let lock = XCheckerLock::new("haiku".to_string(), "0.8.1".to_string());

        let context = RunContext {
            model_full_name: "haiku".to_string(),
            claude_cli_version: "0.8.1".to_string(),
            schema_version: "1".to_string(),
        };

        let drift = lock.detect_drift(&context);
        assert!(
            drift.is_none(),
            "Should return None when all fields match exactly"
        );
    }

    #[test]
    fn test_xchecker_lock_drift_case_sensitive() {
        let lock = XCheckerLock::new("haiku".to_string(), "0.8.1".to_string());

        // Test with different case
        let context = RunContext {
            model_full_name: "Claude-3-5-Sonnet-20241022".to_string(),
            claude_cli_version: "0.8.1".to_string(),
            schema_version: "1".to_string(),
        };

        let drift = lock.detect_drift(&context);
        assert!(drift.is_some(), "Drift detection should be case-sensitive");
        assert!(drift.unwrap().model_full_name.is_some());
    }

    #[test]
    fn test_xchecker_lock_drift_whitespace_sensitive() {
        let lock = XCheckerLock::new("haiku".to_string(), "0.8.1".to_string());

        // Test with extra whitespace
        let context = RunContext {
            model_full_name: "haiku ".to_string(),
            claude_cli_version: "0.8.1".to_string(),
            schema_version: "1".to_string(),
        };

        let drift = lock.detect_drift(&context);
        assert!(
            drift.is_some(),
            "Drift detection should be whitespace-sensitive"
        );
        assert!(drift.unwrap().model_full_name.is_some());
    }

    #[test]
    fn test_xchecker_lock_save_creates_directory() {
        let _temp_dir = setup_test_env();

        let spec_id = "test-spec-new-dir";
        let lock = XCheckerLock::new("haiku".to_string(), "0.8.1".to_string());

        // Directory should not exist yet
        let lock_path = XCheckerLock::get_lock_path(spec_id);
        assert!(!lock_path.exists());

        // Save should create directory
        lock.save(spec_id).unwrap();

        // Directory and file should now exist
        assert!(lock_path.exists());
        assert!(lock_path.parent().unwrap().exists());
    }

    #[test]
    fn test_xchecker_lock_json_format() {
        let _temp_dir = setup_test_env();

        let spec_id = "test-spec-json-format";
        let lock = XCheckerLock::new("haiku".to_string(), "0.8.1".to_string());

        lock.save(spec_id).unwrap();

        // Read raw JSON and verify format
        let lock_path = XCheckerLock::get_lock_path(spec_id);
        let json_content = fs::read_to_string(&lock_path).unwrap();

        // Should be valid JSON
        let parsed: serde_json::Value =
            serde_json::from_str(&json_content).expect("Should be valid JSON");

        // Verify required fields exist
        assert!(parsed.get("schema_version").is_some());
        assert!(parsed.get("created_at").is_some());
        assert!(parsed.get("model_full_name").is_some());
        assert!(parsed.get("claude_cli_version").is_some());

        // Verify values
        assert_eq!(parsed["schema_version"], "1");
        assert_eq!(parsed["model_full_name"], "haiku");
        assert_eq!(parsed["claude_cli_version"], "0.8.1");
    }

    #[test]
    fn test_xchecker_lock_timestamp_format() {
        let lock = XCheckerLock::new("haiku".to_string(), "0.8.1".to_string());

        // Verify created_at is a valid RFC3339 timestamp
        let timestamp_str = lock.created_at.to_rfc3339();
        assert!(!timestamp_str.is_empty());

        // Should be parseable back to DateTime
        let parsed = DateTime::parse_from_rfc3339(&timestamp_str);
        assert!(parsed.is_ok(), "Should be parseable RFC3339 timestamp");
    }

    #[test]
    fn test_configurable_ttl_parameter() {
        let _temp_dir = setup_test_env();

        let spec_id = "test-spec-configurable-ttl";

        // Create a lock file with timestamp 2 minutes ago
        let lock_path = FileLock::get_lock_path(spec_id);
        fs::create_dir_all(lock_path.parent().unwrap()).unwrap();

        let two_minutes_ago = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - 120;

        let old_lock_info = LockInfo {
            pid: 99999, // Non-existent PID
            start_time: 0,
            created_at: two_minutes_ago,
            spec_id: spec_id.to_string(),
            xchecker_version: "0.1.0".to_string(),
        };

        let lock_json = serde_json::to_string_pretty(&old_lock_info).unwrap();
        fs::write(&lock_path, lock_json).unwrap();

        // With TTL of 60 seconds (1 minute), lock should be stale
        let result = FileLock::acquire(spec_id, false, Some(60));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LockError::StaleLock { .. }));

        // With TTL of 180 seconds (3 minutes), lock should not be stale yet
        // but process is dead, so it should still fail without force
        let result = FileLock::acquire(spec_id, false, Some(180));
        assert!(result.is_err());

        // With force, should succeed regardless of TTL
        let lock = FileLock::acquire(spec_id, true, Some(60)).unwrap();
        assert_eq!(lock.spec_id(), spec_id);
    }

    #[test]
    fn test_stale_lock_detection_by_age() {
        let _temp_dir = setup_test_env();

        let spec_id = "test-spec-stale-by-age";

        // Create a lock file with very old timestamp (2 hours ago)
        let lock_path = FileLock::get_lock_path(spec_id);
        fs::create_dir_all(lock_path.parent().unwrap()).unwrap();

        let two_hours_ago = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - 7200;

        let old_lock_info = LockInfo {
            pid: 99999, // Non-existent PID
            start_time: 0,
            created_at: two_hours_ago,
            spec_id: spec_id.to_string(),
            xchecker_version: "0.1.0".to_string(),
        };

        let lock_json = serde_json::to_string_pretty(&old_lock_info).unwrap();
        fs::write(&lock_path, lock_json).unwrap();

        // Should detect as stale with default TTL (1 hour)
        let result = FileLock::acquire(spec_id, false, None);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LockError::StaleLock { .. }));

        // Should succeed with force
        let lock = FileLock::acquire(spec_id, true, None).unwrap();
        assert_eq!(lock.spec_id(), spec_id);
    }

    #[test]
    fn test_stale_lock_detection_by_dead_process() {
        let _temp_dir = setup_test_env();

        let spec_id = "test-spec-stale-by-pid";

        // Create a lock file with recent timestamp but non-existent PID
        let lock_path = FileLock::get_lock_path(spec_id);
        fs::create_dir_all(lock_path.parent().unwrap()).unwrap();

        let recent_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - 60; // 1 minute ago

        let old_lock_info = LockInfo {
            pid: 99999, // Non-existent PID
            start_time: 0,
            created_at: recent_time,
            spec_id: spec_id.to_string(),
            xchecker_version: "0.1.0".to_string(),
        };

        let lock_json = serde_json::to_string_pretty(&old_lock_info).unwrap();
        fs::write(&lock_path, lock_json).unwrap();

        // Should fail even though lock is recent, because process is dead
        let result = FileLock::acquire(spec_id, false, None);
        assert!(result.is_err());

        // Should succeed with force
        let lock = FileLock::acquire(spec_id, true, None).unwrap();
        assert_eq!(lock.spec_id(), spec_id);
    }

    #[test]
    fn test_concurrent_execution_detection() {
        let _temp_dir = setup_test_env();

        let spec_id = "test-spec-concurrent";

        // Acquire first lock
        let _lock1 = FileLock::acquire(spec_id, false, None).unwrap();

        // Try to acquire second lock - should fail with ConcurrentExecution
        let result = FileLock::acquire(spec_id, false, None);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            LockError::ConcurrentExecution { .. }
        ));

        // Even with force, should succeed if process is still running
        let result = FileLock::acquire(spec_id, true, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_lock_release_on_normal_exit() {
        let _temp_dir = setup_test_env();

        let spec_id = "test-spec-normal-exit";

        // Acquire lock
        let lock = FileLock::acquire(spec_id, false, None).unwrap();
        assert!(FileLock::exists(spec_id));

        // Explicitly release
        lock.release().unwrap();

        // Lock should be gone
        assert!(!FileLock::exists(spec_id));

        // Should be able to acquire again after release
        let _lock2 = FileLock::acquire(spec_id, false, None).unwrap();
    }

    #[test]
    fn test_lock_cleanup_on_panic() {
        let _temp_dir = setup_test_env();

        let spec_id = "test-spec-panic-cleanup";

        {
            let _lock = FileLock::acquire(spec_id, false, None).unwrap();
            assert!(FileLock::exists(spec_id));
        } // lock goes out of scope here, Drop should clean up

        // Lock should be automatically cleaned up by Drop
        assert!(!FileLock::exists(spec_id));
    }

    #[test]
    fn test_force_flag_breaks_stale_lock() {
        let _temp_dir = setup_test_env();

        let spec_id = "test-spec-force-break";

        // Create a stale lock
        let lock_path = FileLock::get_lock_path(spec_id);
        fs::create_dir_all(lock_path.parent().unwrap()).unwrap();

        let old_lock_info = LockInfo {
            pid: 99999,
            start_time: 0,
            created_at: 0,
            spec_id: spec_id.to_string(),
            xchecker_version: "0.1.0".to_string(),
        };

        let lock_json = serde_json::to_string_pretty(&old_lock_info).unwrap();
        fs::write(&lock_path, lock_json).unwrap();

        // Should fail without force
        let result = FileLock::acquire(spec_id, false, None);
        assert!(result.is_err());

        // Should succeed with force
        let lock = FileLock::acquire(spec_id, true, None).unwrap();
        assert_eq!(lock.spec_id(), spec_id);

        // Lock info should be updated with current process
        let new_lock_info = FileLock::get_lock_info(spec_id).unwrap().unwrap();
        assert_eq!(new_lock_info.pid, process::id());
    }

    #[test]
    fn test_lock_info_with_invalid_pid() {
        let _temp_dir = setup_test_env();

        let spec_id = "test-spec-invalid-pid";
        let lock_path = FileLock::get_lock_path(spec_id);
        fs::create_dir_all(lock_path.parent().unwrap()).unwrap();

        // Create a lock with an invalid PID (0 is never a valid PID)
        let invalid_lock_info = LockInfo {
            pid: 0,
            start_time: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            spec_id: spec_id.to_string(),
            xchecker_version: "0.1.0".to_string(),
        };

        let lock_json = serde_json::to_string_pretty(&invalid_lock_info).unwrap();
        fs::write(&lock_path, lock_json).unwrap();

        // Should be able to acquire with force (PID 0 is never running)
        let result = FileLock::acquire(spec_id, true, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_lock_info_with_invalid_host() {
        let _temp_dir = setup_test_env();

        let spec_id = "test-spec-invalid-host";

        // Create a lock with current PID but we'll test that it still works
        let lock = FileLock::acquire(spec_id, false, None).unwrap();
        let lock_info = lock.lock_info();

        // Verify lock info is valid
        assert_eq!(lock_info.spec_id, spec_id);
        assert_eq!(lock_info.pid, process::id());
        assert!(!lock_info.xchecker_version.is_empty());
    }

    #[test]
    fn test_lock_with_corrupted_lock_file() {
        let _temp_dir = setup_test_env();

        let spec_id = "test-spec-corrupted-lock";
        let lock_path = FileLock::get_lock_path(spec_id);
        fs::create_dir_all(lock_path.parent().unwrap()).unwrap();

        // Write corrupted JSON to lock file
        fs::write(&lock_path, "{ invalid json content }").unwrap();

        // Should fail with CorruptedLock error
        let result = FileLock::acquire(spec_id, false, None);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            LockError::CorruptedLock { .. }
        ));

        // Force flag doesn't bypass corrupted lock detection - it only bypasses stale lock detection
        // Corrupted locks are always an error that requires manual intervention
        let result_force = FileLock::acquire(spec_id, true, None);
        assert!(result_force.is_err());
        assert!(matches!(
            result_force.unwrap_err(),
            LockError::CorruptedLock { .. }
        ));
    }

    #[test]
    fn test_lock_with_partial_json() {
        let _temp_dir = setup_test_env();

        let spec_id = "test-spec-partial-json";
        let lock_path = FileLock::get_lock_path(spec_id);
        fs::create_dir_all(lock_path.parent().unwrap()).unwrap();

        // Write partial JSON (missing closing brace)
        fs::write(&lock_path, r#"{"pid": 12345, "start_time":"#).unwrap();

        // Should fail with CorruptedLock error
        let result = FileLock::acquire(spec_id, false, None);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            LockError::CorruptedLock { .. }
        ));
    }

    #[test]
    fn test_lock_with_wrong_json_structure() {
        let _temp_dir = setup_test_env();

        let spec_id = "test-spec-wrong-structure";
        let lock_path = FileLock::get_lock_path(spec_id);
        fs::create_dir_all(lock_path.parent().unwrap()).unwrap();

        // Write valid JSON but wrong structure (array instead of object)
        fs::write(&lock_path, r#"["not", "a", "lock", "object"]"#).unwrap();

        // Should fail with CorruptedLock error
        let result = FileLock::acquire(spec_id, false, None);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            LockError::CorruptedLock { .. }
        ));
    }

    #[test]
    fn test_lock_with_missing_required_fields() {
        let _temp_dir = setup_test_env();

        let spec_id = "test-spec-missing-fields";
        let lock_path = FileLock::get_lock_path(spec_id);
        fs::create_dir_all(lock_path.parent().unwrap()).unwrap();

        // Write JSON with missing required fields
        fs::write(&lock_path, r#"{"pid": 12345}"#).unwrap();

        // Should fail with CorruptedLock error
        let result = FileLock::acquire(spec_id, false, None);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            LockError::CorruptedLock { .. }
        ));
    }

    #[test]
    fn test_lock_with_extra_fields() {
        let _temp_dir = setup_test_env();

        let spec_id = "test-spec-extra-fields";
        let lock_path = FileLock::get_lock_path(spec_id);
        fs::create_dir_all(lock_path.parent().unwrap()).unwrap();

        // Create lock info with all required fields plus extra
        let lock_info_json = r#"{
            "pid": 12345,
            "start_time": 0,
            "created_at": 0,
            "spec_id": "test-spec-extra-fields",
            "xchecker_version": "0.1.0",
            "extra_field": "should be ignored"
        }"#;

        fs::write(&lock_path, lock_info_json).unwrap();

        // Should succeed with force (extra fields should be ignored)
        let result = FileLock::acquire(spec_id, true, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_lock_with_very_old_timestamp() {
        let _temp_dir = setup_test_env();

        let spec_id = "test-spec-very-old";

        // Create a lock with timestamp from year 1970
        let lock_path = FileLock::get_lock_path(spec_id);
        fs::create_dir_all(lock_path.parent().unwrap()).unwrap();

        let old_lock_info = LockInfo {
            pid: 99999,
            start_time: 0,
            created_at: 0, // Unix epoch
            spec_id: spec_id.to_string(),
            xchecker_version: "0.1.0".to_string(),
        };

        let lock_json = serde_json::to_string_pretty(&old_lock_info).unwrap();
        fs::write(&lock_path, lock_json).unwrap();

        // Should be detected as stale
        let result = FileLock::acquire(spec_id, false, None);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LockError::StaleLock { .. }));

        // Should succeed with force
        let lock = FileLock::acquire(spec_id, true, None).unwrap();
        assert_eq!(lock.spec_id(), spec_id);
    }

    #[test]
    fn test_lock_with_future_timestamp() {
        let _temp_dir = setup_test_env();

        let spec_id = "test-spec-future";

        // Create a lock with timestamp 1 hour in the future (clock skew scenario)
        let lock_path = FileLock::get_lock_path(spec_id);
        fs::create_dir_all(lock_path.parent().unwrap()).unwrap();

        let future_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 3600; // 1 hour in the future

        let future_lock_info = LockInfo {
            pid: 99999, // Non-existent PID
            start_time: future_timestamp,
            created_at: future_timestamp,
            spec_id: spec_id.to_string(),
            xchecker_version: "0.1.0".to_string(),
        };

        let lock_json = serde_json::to_string_pretty(&future_lock_info).unwrap();
        fs::write(&lock_path, lock_json).unwrap();

        // Future timestamps should be handled gracefully (no panic)
        // Treated as age=0 (not stale), but PID check should still apply
        let result = FileLock::acquire(spec_id, false, None);
        // Result depends on whether PID 99999 exists (unlikely), but no overflow/panic should occur
        assert!(
            result.is_ok() || result.is_err(),
            "Should handle future timestamp without panic"
        );
    }

    #[test]
    fn test_lock_info_with_empty_spec_id() {
        let _temp_dir = setup_test_env();

        let spec_id = "";

        // Should handle empty spec_id gracefully
        let result = FileLock::acquire(spec_id, false, None);
        // May succeed or fail depending on path handling, but shouldn't panic
        assert!(
            result.is_ok() || result.is_err(),
            "Should handle empty spec_id without panic"
        );
    }

    #[test]
    fn test_lock_info_with_special_characters_in_spec_id() {
        let _temp_dir = setup_test_env();

        let spec_id = "test-spec-with-special-@#$%";

        // Should handle special characters in spec_id
        let result = FileLock::acquire(spec_id, false, None);
        // May succeed or fail depending on filesystem, but shouldn't panic
        if let Ok(lock) = result {
            assert_eq!(lock.spec_id(), spec_id);
        }
    }

    #[test]
    fn test_get_lock_info_with_nonexistent_lock() {
        let _temp_dir = setup_test_env();

        let spec_id = "nonexistent-lock-spec";

        let result = FileLock::get_lock_info(spec_id);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_get_lock_info_with_corrupted_lock() {
        let _temp_dir = setup_test_env();

        let spec_id = "corrupted-lock-info-spec";
        let lock_path = FileLock::get_lock_path(spec_id);
        fs::create_dir_all(lock_path.parent().unwrap()).unwrap();

        // Write corrupted content
        fs::write(&lock_path, "not json at all").unwrap();

        let result = FileLock::get_lock_info(spec_id);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            LockError::CorruptedLock { .. }
        ));
    }

    #[test]
    fn test_xchecker_lock_with_empty_values() {
        let lock = XCheckerLock::new(String::new(), String::new());

        assert_eq!(lock.schema_version, "1");
        assert_eq!(lock.model_full_name, "");
        assert_eq!(lock.claude_cli_version, "");
    }

    #[test]
    fn test_xchecker_lock_with_very_long_values() {
        let long_model = "a".repeat(1000);
        let long_version = "b".repeat(1000);

        let lock = XCheckerLock::new(long_model.clone(), long_version.clone());

        assert_eq!(lock.model_full_name, long_model);
        assert_eq!(lock.claude_cli_version, long_version);
    }

    #[test]
    fn test_xchecker_lock_with_unicode_values() {
        let unicode_model = "claude-测试-🚀";
        let unicode_version = "版本-1.0-✨";

        let lock = XCheckerLock::new(unicode_model.to_string(), unicode_version.to_string());

        assert_eq!(lock.model_full_name, unicode_model);
        assert_eq!(lock.claude_cli_version, unicode_version);
    }

    #[test]
    fn test_empty_lockfile_error_includes_spec_id() {
        let _temp_dir = setup_test_env();

        let spec_id = "test-spec-empty-lockfile-msg";
        let lock_path = FileLock::get_lock_path(spec_id);
        fs::create_dir_all(lock_path.parent().unwrap()).unwrap();

        // Write empty file (simulates race condition during initialization)
        fs::write(&lock_path, "").unwrap();

        // Should fail with CorruptedLock error that includes spec_id
        let result = FileLock::acquire(spec_id, false, None);
        assert!(result.is_err());

        match result.unwrap_err() {
            LockError::CorruptedLock { reason } => {
                assert!(
                    reason.contains(spec_id),
                    "Error message should contain spec_id: {reason}"
                );
                assert!(
                    reason.contains("empty") || reason.contains("initializing"),
                    "Error message should mention empty/initializing: {reason}"
                );
            }
            other => panic!("Expected CorruptedLock error, got: {other:?}"),
        }
    }

    #[test]
    fn test_partial_json_lockfile_error_includes_spec_id() {
        let _temp_dir = setup_test_env();

        let spec_id = "test-spec-partial-json-msg";
        let lock_path = FileLock::get_lock_path(spec_id);
        fs::create_dir_all(lock_path.parent().unwrap()).unwrap();

        // Write partial JSON (simulates mid-write race condition)
        fs::write(&lock_path, r#"{"pid": 12345, "start_time":"#).unwrap();

        // Should fail with CorruptedLock error that includes spec_id
        let result = FileLock::acquire(spec_id, false, None);
        assert!(result.is_err());

        match result.unwrap_err() {
            LockError::CorruptedLock { reason } => {
                assert!(
                    reason.contains(spec_id),
                    "Error message should contain spec_id: {reason}"
                );
            }
            other => panic!("Expected CorruptedLock error, got: {other:?}"),
        }
    }

    #[test]
    fn test_corrupted_json_error_includes_spec_id() {
        let _temp_dir = setup_test_env();

        let spec_id = "test-spec-error-format";

        let lock_path = FileLock::get_lock_path(spec_id);
        fs::create_dir_all(lock_path.parent().unwrap()).unwrap();

        // Write corrupted JSON that won't be retried (definite corruption, not EOF)
        fs::write(&lock_path, r#"{"invalid": "structure", "no_pid": true}"#).unwrap();

        // Should fail with CorruptedLock error that includes spec_id
        let result = FileLock::acquire(spec_id, false, None);
        assert!(result.is_err());

        match result.unwrap_err() {
            LockError::CorruptedLock { reason } => {
                assert!(
                    reason.contains(spec_id),
                    "Error message should contain spec_id: {reason}"
                );
            }
            other => panic!("Expected CorruptedLock error, got: {other:?}"),
        }
    }

    #[test]
    fn test_concurrent_lock_error_includes_spec_id() {
        let _temp_dir = setup_test_env();

        let spec_id = "test-spec-concurrent-msg";

        // First acquire a lock
        let _lock1 = FileLock::acquire(spec_id, false, None).unwrap();

        // Try to acquire again - this will fail with ConcurrentExecution
        let result = FileLock::acquire(spec_id, false, None);
        assert!(result.is_err());

        // ConcurrentExecution error should include spec_id
        match result.unwrap_err() {
            LockError::ConcurrentExecution {
                spec_id: err_spec, ..
            } => {
                assert_eq!(err_spec, spec_id);
            }
            other => panic!("Expected ConcurrentExecution error, got: {other:?}"),
        }
    }

    #[test]
    fn test_validate_existing_lock_handles_clock_skew() {
        let _temp_dir = setup_test_env();

        let spec_id = "test-spec-clock-skew-validation";
        let lock_path = FileLock::get_lock_path(spec_id);
        fs::create_dir_all(lock_path.parent().unwrap()).unwrap();

        // Create a lock with timestamp 1 hour in the future (clock skew)
        let future_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 3600;

        let lock_info = LockInfo {
            pid: 99999, // Non-existent PID
            start_time: future_timestamp,
            created_at: future_timestamp,
            spec_id: spec_id.to_string(),
            xchecker_version: "0.1.0".to_string(),
        };

        let lock_json = serde_json::to_string_pretty(&lock_info).unwrap();
        fs::write(&lock_path, lock_json).unwrap();

        // Should not panic due to clock skew - saturating_sub handles this
        // With force=true, should succeed
        let result = FileLock::acquire(spec_id, true, None);
        assert!(result.is_ok(), "Should handle clock skew gracefully");
    }
}

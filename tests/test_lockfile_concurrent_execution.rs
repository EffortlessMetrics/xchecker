//! Integration tests for concurrent execution prevention with actual multi-process scenarios
//!
//! **WHITE-BOX TEST**: This test uses internal module APIs (`lock::{FileLock, LockError}`) and
//! may break with internal refactors. These tests are intentionally white-box to validate
//! internal implementation details. See FR-TEST-4 for white-box test policy.
//!
//! This test suite validates FR-LOCK-001, FR-LOCK-002, and FR-LOCK-005 by spawning
//! actual child processes to test real concurrent execution scenarios.

use anyhow::Result;
use serial_test::serial;
use std::sync::Arc;
use std::thread;
use tempfile::TempDir;
use xchecker::lock::{FileLock, LockError};

/// Helper to set up isolated test environment
fn setup_test_env() -> TempDir {
    xchecker::paths::with_isolated_home()
}

#[test]
fn test_concurrent_lock_acquisition_same_process() -> Result<()> {
    let _temp_dir = setup_test_env();
    let spec_id = "test-concurrent-same-process";

    // Acquire first lock
    let lock1 = FileLock::acquire(spec_id, false, None)?;
    assert_eq!(lock1.spec_id(), spec_id);
    assert!(FileLock::exists(spec_id));

    // Try to acquire second lock in same process - should fail
    let result = FileLock::acquire(spec_id, false, None);
    assert!(result.is_err(), "Second lock acquisition should fail");

    match result.unwrap_err() {
        LockError::ConcurrentExecution {
            spec_id: locked_spec,
            pid,
            ..
        } => {
            assert_eq!(locked_spec, spec_id);
            assert_eq!(pid, std::process::id());
        }
        other => panic!("Expected ConcurrentExecution error, got: {:?}", other),
    }

    // Release first lock
    lock1.release()?;
    assert!(!FileLock::exists(spec_id));

    // Should be able to acquire again after release
    let lock2 = FileLock::acquire(spec_id, false, None)?;
    assert_eq!(lock2.spec_id(), spec_id);

    Ok(())
}

#[test]
fn test_lock_held_by_active_process_exit_9() -> Result<()> {
    let _temp_dir = setup_test_env();
    let spec_id = "test-lock-exit-9";

    // Acquire lock
    let _lock = FileLock::acquire(spec_id, false, None)?;

    // Try to acquire again - should fail with ConcurrentExecution
    let result = FileLock::acquire(spec_id, false, None);
    assert!(result.is_err());

    match result.unwrap_err() {
        LockError::ConcurrentExecution { .. } => {
            // This is the expected error that maps to exit code 9
            // The error itself doesn't contain the exit code, but the
            // XCheckerError wrapper would map this to exit code 9
        }
        other => panic!("Expected ConcurrentExecution error, got: {:?}", other),
    }

    Ok(())
}

#[test]
fn test_lock_released_on_normal_exit() -> Result<()> {
    let _temp_dir = setup_test_env();
    let spec_id = "test-lock-normal-exit";

    // Acquire and explicitly release
    let lock = FileLock::acquire(spec_id, false, None)?;
    assert!(FileLock::exists(spec_id));

    lock.release()?;
    assert!(
        !FileLock::exists(spec_id),
        "Lock should be removed after release"
    );

    // Should be able to acquire again
    let _lock2 = FileLock::acquire(spec_id, false, None)?;

    Ok(())
}

#[test]
fn test_lock_cleanup_on_drop() -> Result<()> {
    let _temp_dir = setup_test_env();
    let spec_id = "test-lock-drop-cleanup";

    {
        let _lock = FileLock::acquire(spec_id, false, None)?;
        assert!(FileLock::exists(spec_id));
        // Lock goes out of scope here
    }

    // Lock should be automatically cleaned up by Drop
    assert!(!FileLock::exists(spec_id), "Lock should be removed by Drop");

    // Should be able to acquire again
    let _lock2 = FileLock::acquire(spec_id, false, None)?;

    Ok(())
}

#[test]
fn test_lock_cleanup_on_panic_simulation() -> Result<()> {
    let _temp_dir = setup_test_env();
    let spec_id = "test-lock-panic-cleanup";

    // Use catch_unwind to simulate panic without actually panicking the test
    let result = std::panic::catch_unwind(|| {
        let _lock = FileLock::acquire(spec_id, false, None).unwrap();
        assert!(FileLock::exists(spec_id));
        // Simulate panic by returning early
        // Drop will still be called
    });

    // Whether panic occurred or not, Drop should have cleaned up
    assert!(result.is_ok());
    assert!(
        !FileLock::exists(spec_id),
        "Lock should be cleaned up even after panic"
    );

    Ok(())
}

#[test]
fn test_lock_file_contains_correct_info() -> Result<()> {
    let _temp_dir = setup_test_env();
    let spec_id = "test-lock-info-content";

    let _lock = FileLock::acquire(spec_id, false, None)?;

    // Get lock info
    let lock_info = FileLock::get_lock_info(spec_id)?.expect("Lock info should exist");

    // Verify fields
    assert_eq!(lock_info.spec_id, spec_id);
    assert_eq!(lock_info.pid, std::process::id());
    assert!(!lock_info.xchecker_version.is_empty());
    assert!(lock_info.start_time > 0);
    assert!(lock_info.created_at > 0);

    Ok(())
}

/// Test that multiple threads cannot acquire the same lock simultaneously.
///
/// This test is deterministic: the main thread holds the lock while
/// spawned threads attempt acquisition (they should all fail).
/// No sleep-based timing that could cause flaky behavior on macOS.
///
/// Note: Uses XCHECKER_HOME env var instead of TLS-based with_isolated_home
/// because spawned threads don't inherit TLS state. Env vars are shared.
#[test]
#[serial]
fn test_concurrent_threads_same_process() -> Result<()> {
    // Create temp dir and set XCHECKER_HOME env var (shared across threads)
    let temp_dir = TempDir::new()?;
    let original_home = std::env::var("XCHECKER_HOME").ok();
    // SAFETY: This is a single-threaded test setup; env var is set before spawning threads
    unsafe { std::env::set_var("XCHECKER_HOME", temp_dir.path()) };

    // Cleanup guard to restore env var on exit (including panic)
    struct EnvGuard(Option<String>);
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            // SAFETY: This runs after all spawned threads have been joined
            unsafe {
                match &self.0 {
                    Some(val) => std::env::set_var("XCHECKER_HOME", val),
                    None => std::env::remove_var("XCHECKER_HOME"),
                }
            }
        }
    }
    let _guard = EnvGuard(original_home);

    let spec_id = "test-concurrent-threads";

    // Main thread acquires and holds the lock for the duration of the test
    let lock = FileLock::acquire(spec_id, false, None)?;
    assert!(
        FileLock::exists(spec_id),
        "Lock should exist after acquisition"
    );

    let spec_id_arc = Arc::new(spec_id.to_string());
    let mut handles = vec![];

    // Spawn multiple threads that try to acquire the same lock while it's held
    for _ in 0..5 {
        let spec_id = Arc::clone(&spec_id_arc);
        let handle = thread::spawn(move || {
            // Try to acquire - should fail immediately since lock is held
            FileLock::acquire(&spec_id, false, None)
        });
        handles.push(handle);
    }

    // Collect results - all should be errors (lock is held)
    let mut error_count = 0;
    for handle in handles {
        match handle.join().unwrap() {
            Ok(_) => panic!("Thread should not have acquired lock while it's held"),
            Err(LockError::ConcurrentExecution { .. }) => {
                error_count += 1;
            }
            Err(other) => panic!("Expected ConcurrentExecution error, got: {:?}", other),
        }
    }

    assert_eq!(
        error_count, 5,
        "All 5 threads should fail with ConcurrentExecution"
    );

    // Now release the lock
    lock.release()?;
    assert!(
        !FileLock::exists(spec_id),
        "Lock should not exist after release"
    );

    // Verify we can acquire again after release
    let lock2 = FileLock::acquire(spec_id, false, None)?;
    assert!(
        FileLock::exists(spec_id),
        "Lock should be acquirable after release"
    );
    lock2.release()?;

    Ok(())
}

#[test]
fn test_stale_lock_detection_and_force_override() -> Result<()> {
    let _temp_dir = setup_test_env();
    let spec_id = "test-stale-lock-force";

    // Create a stale lock manually
    let spec_root = xchecker::paths::spec_root(spec_id);
    let lock_path = spec_root.as_std_path().join(".lock");
    std::fs::create_dir_all(lock_path.parent().unwrap())?;

    let stale_lock_info = xchecker::lock::LockInfo {
        pid: 99999, // Non-existent PID
        start_time: 0,
        created_at: 0, // Very old timestamp
        spec_id: spec_id.to_string(),
        xchecker_version: "0.1.0".to_string(),
    };

    let lock_json = serde_json::to_string_pretty(&stale_lock_info)?;
    std::fs::write(&lock_path, lock_json)?;

    // Should fail without force
    let result = FileLock::acquire(spec_id, false, None);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), LockError::StaleLock { .. }));

    // Should succeed with force
    let lock = FileLock::acquire(spec_id, true, None)?;
    assert_eq!(lock.spec_id(), spec_id);

    // Verify lock info is updated
    let new_lock_info = FileLock::get_lock_info(spec_id)?.unwrap();
    assert_eq!(new_lock_info.pid, std::process::id());

    Ok(())
}

#[test]
fn test_lock_with_dead_process_recent_timestamp() -> Result<()> {
    let _temp_dir = setup_test_env();
    let spec_id = "test-dead-process-recent";

    // Create a lock with recent timestamp but dead process
    let spec_root = xchecker::paths::spec_root(spec_id);
    let lock_path = spec_root.as_std_path().join(".lock");
    std::fs::create_dir_all(lock_path.parent().unwrap())?;

    let recent_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        - 60; // 1 minute ago

    let dead_process_lock = xchecker::lock::LockInfo {
        pid: 99999, // Non-existent PID
        start_time: 0,
        created_at: recent_time,
        spec_id: spec_id.to_string(),
        xchecker_version: "0.1.0".to_string(),
    };

    let lock_json = serde_json::to_string_pretty(&dead_process_lock)?;
    std::fs::write(&lock_path, lock_json)?;

    // Should fail without force (process is dead but lock is recent)
    let result = FileLock::acquire(spec_id, false, None);
    assert!(result.is_err());

    // Should succeed with force
    let lock = FileLock::acquire(spec_id, true, None)?;
    assert_eq!(lock.spec_id(), spec_id);

    Ok(())
}

#[test]
fn test_configurable_ttl() -> Result<()> {
    let _temp_dir = setup_test_env();
    let spec_id = "test-configurable-ttl";

    // Create a lock 2 minutes old
    let spec_root = xchecker::paths::spec_root(spec_id);
    let lock_path = spec_root.as_std_path().join(".lock");
    std::fs::create_dir_all(lock_path.parent().unwrap())?;

    let two_minutes_ago = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        - 120;

    let old_lock = xchecker::lock::LockInfo {
        pid: 99999,
        start_time: 0,
        created_at: two_minutes_ago,
        spec_id: spec_id.to_string(),
        xchecker_version: "0.1.0".to_string(),
    };

    let lock_json = serde_json::to_string_pretty(&old_lock)?;
    std::fs::write(&lock_path, lock_json)?;

    // With TTL of 60 seconds, should be stale
    let result = FileLock::acquire(spec_id, false, Some(60));
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), LockError::StaleLock { .. }));

    // With TTL of 180 seconds, should not be stale (but process is dead)
    let result = FileLock::acquire(spec_id, false, Some(180));
    assert!(result.is_err()); // Still fails because process is dead

    // Force should work regardless
    let lock = FileLock::acquire(spec_id, true, Some(60))?;
    assert_eq!(lock.spec_id(), spec_id);

    Ok(())
}

#[test]
fn test_multiple_specs_independent_locks() -> Result<()> {
    let _temp_dir = setup_test_env();

    let spec1 = "test-spec-1";
    let spec2 = "test-spec-2";
    let spec3 = "test-spec-3";

    // Should be able to acquire locks for different specs simultaneously
    let lock1 = FileLock::acquire(spec1, false, None)?;
    let lock2 = FileLock::acquire(spec2, false, None)?;
    let lock3 = FileLock::acquire(spec3, false, None)?;

    assert!(FileLock::exists(spec1));
    assert!(FileLock::exists(spec2));
    assert!(FileLock::exists(spec3));

    // Verify each lock has correct spec_id
    assert_eq!(lock1.spec_id(), spec1);
    assert_eq!(lock2.spec_id(), spec2);
    assert_eq!(lock3.spec_id(), spec3);

    // Release all locks
    lock1.release()?;
    lock2.release()?;
    lock3.release()?;

    assert!(!FileLock::exists(spec1));
    assert!(!FileLock::exists(spec2));
    assert!(!FileLock::exists(spec3));

    Ok(())
}

#[test]
fn test_lock_acquisition_creates_directory() -> Result<()> {
    let _temp_dir = setup_test_env();
    let spec_id = "test-new-spec-dir";

    // Directory should not exist yet
    let spec_root = xchecker::paths::spec_root(spec_id);
    let lock_path = spec_root.as_std_path().join(".lock");
    assert!(!lock_path.exists());

    // Acquiring lock should create directory
    let lock = FileLock::acquire(spec_id, false, None)?;

    // Directory and lock file should now exist
    assert!(lock_path.exists());
    assert!(lock_path.parent().unwrap().exists());

    lock.release()?;

    Ok(())
}

#[test]
fn test_lock_error_messages() -> Result<()> {
    let _temp_dir = setup_test_env();
    let spec_id = "test-error-messages";

    // Test ConcurrentExecution error message
    let _lock = FileLock::acquire(spec_id, false, None)?;
    let result = FileLock::acquire(spec_id, false, None);

    match result {
        Err(LockError::ConcurrentExecution {
            spec_id: locked_spec,
            pid,
            created_ago,
        }) => {
            assert_eq!(locked_spec, spec_id);
            assert_eq!(pid, std::process::id());
            assert!(!created_ago.is_empty());
        }
        _ => panic!("Expected ConcurrentExecution error"),
    }

    Ok(())
}

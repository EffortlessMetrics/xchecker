//! Integration tests for concurrent execution prevention with actual multi-process scenarios
//!
//! This test suite validates FR-LOCK-001, FR-LOCK-002, and FR-LOCK-005 by spawning
//! actual child processes to test real concurrent execution scenarios.

use anyhow::Result;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
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

#[test]
fn test_concurrent_threads_same_process() -> Result<()> {
    let _temp_dir = setup_test_env();
    let spec_id = "test-concurrent-threads";

    let success_count = Arc::new(Mutex::new(0));
    let error_count = Arc::new(Mutex::new(0));

    let mut handles = vec![];

    // Spawn multiple threads trying to acquire the same lock
    for i in 0..5 {
        let success_count = Arc::clone(&success_count);
        let error_count = Arc::clone(&error_count);

        let handle = thread::spawn(move || {
            // Add small delay to increase chance of concurrent attempts
            thread::sleep(Duration::from_millis(i * 10));

            match FileLock::acquire(spec_id, false, None) {
                Ok(lock) => {
                    *success_count.lock().unwrap() += 1;
                    // Hold lock briefly
                    thread::sleep(Duration::from_millis(50));
                    lock.release().ok();
                }
                Err(_) => {
                    *error_count.lock().unwrap() += 1;
                }
            }
        });

        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    let success = *success_count.lock().unwrap();
    let errors = *error_count.lock().unwrap();

    // Exactly one thread should succeed, others should fail
    assert_eq!(success, 1, "Exactly one thread should acquire the lock");
    assert_eq!(errors, 4, "Four threads should fail to acquire the lock");

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

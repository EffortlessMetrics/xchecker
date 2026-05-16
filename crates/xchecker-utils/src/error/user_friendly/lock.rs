use std::io;

use super::UserFriendlyError;
use crate::error::{ErrorCategory, LockError};

impl UserFriendlyError for LockError {
    fn user_message(&self) -> String {
        match self {
            Self::ConcurrentExecution {
                spec_id,
                pid,
                created_ago,
            } => {
                format!(
                    "Another xchecker process is already running for spec '{spec_id}' (PID {pid}, started {created_ago})"
                )
            }
            Self::StaleLock {
                spec_id,
                pid,
                age_secs,
            } => {
                format!("Stale lock detected for spec '{spec_id}' (PID {pid}, age {age_secs}s)")
            }
            Self::CorruptedLock { reason } => {
                format!("Lock file is corrupted or invalid: {reason}")
            }
            Self::AcquisitionFailed { reason } => {
                format!("Failed to acquire exclusive lock: {reason}")
            }
            Self::ReleaseFailed { reason } => {
                format!("Failed to release lock: {reason}")
            }
            Self::Io(e) => {
                format!("File system error during lock operation: {e}")
            }
        }
    }

    fn context(&self) -> Option<String> {
        match self {
            Self::ConcurrentExecution { .. } => {
                Some("xchecker uses advisory file locks to prevent concurrent execution on the same spec. This ensures data integrity and prevents conflicts.".to_string())
            }
            Self::StaleLock { .. } => {
                Some("Stale locks can occur when xchecker processes are terminated unexpectedly. The lock system prevents accidental conflicts.".to_string())
            }
            Self::CorruptedLock { .. } => {
                Some("Lock files contain process information in JSON format. Corruption can occur due to disk issues or interrupted writes.".to_string())
            }
            Self::AcquisitionFailed { .. } => {
                Some("Lock acquisition ensures exclusive access to spec directories during operations that modify state.".to_string())
            }
            Self::ReleaseFailed { .. } => {
                Some("Lock release cleans up the lock file when operations complete. Failure to release may leave stale locks.".to_string())
            }
            Self::Io(_) => {
                Some("File system operations are required for lock management. Check permissions and disk space.".to_string())
            }
        }
    }

    fn suggestions(&self) -> Vec<String> {
        match self {
            Self::ConcurrentExecution { spec_id, pid, .. } => vec![
                format!("Wait for the other process (PID {}) to complete", pid),
                "Check if the process is still running with: ps {} (Unix) or tasklist /FI \"PID eq {}\" (Windows)".to_string(),
                "If the process is stuck, terminate it and try again".to_string(),
                format!("Use --force to override if you're certain no other process is running on spec '{}'", spec_id),
            ],
            Self::StaleLock { spec_id, pid, .. } => vec![
                format!("Use --force to override the stale lock for spec '{}'", spec_id),
                format!("Verify that process {} is no longer running", pid),
                "Check system logs for any crashed xchecker processes".to_string(),
                "Consider cleaning up old spec directories if they're no longer needed".to_string(),
            ],
            Self::CorruptedLock { .. } => vec![
                "Remove the corrupted lock file manually: rm .xchecker/specs/<spec_id>/.lock".to_string(),
                "Check disk space and file system integrity".to_string(),
                "Ensure proper shutdown of xchecker processes to prevent corruption".to_string(),
            ],
            Self::AcquisitionFailed { .. } => vec![
                "Check file permissions in the .xchecker directory".to_string(),
                "Ensure sufficient disk space for lock file creation".to_string(),
                "Verify that the parent directory is writable".to_string(),
                "Try running from a different directory with proper permissions".to_string(),
            ],
            Self::ReleaseFailed { .. } => vec![
                "Check file permissions for the lock file".to_string(),
                "Ensure the lock file exists and is writable".to_string(),
                "The lock will be automatically cleaned up when the process exits".to_string(),
            ],
            Self::Io(e) => {
                match e.kind() {
                    io::ErrorKind::PermissionDenied => vec![
                        "Check file and directory permissions".to_string(),
                        "Ensure you have write access to the .xchecker directory".to_string(),
                        "Try running with appropriate privileges".to_string(),
                    ],
                    io::ErrorKind::NotFound => vec![
                        "Ensure the .xchecker directory exists".to_string(),
                        "Check that the spec directory path is correct".to_string(),
                    ],
                    io::ErrorKind::AlreadyExists => vec![
                        "Another process may have created the lock file simultaneously".to_string(),
                        "Wait a moment and try again".to_string(),
                    ],
                    _ => vec![
                        "Check disk space and file system health".to_string(),
                        "Verify file system permissions".to_string(),
                        "Try the operation again".to_string(),
                    ]
                }
            }
        }
    }

    fn category(&self) -> ErrorCategory {
        match self {
            Self::ConcurrentExecution { .. } | Self::StaleLock { .. } => ErrorCategory::Concurrency,
            Self::CorruptedLock { .. } => ErrorCategory::Validation,
            Self::AcquisitionFailed { .. } | Self::ReleaseFailed { .. } => {
                ErrorCategory::FileSystem
            }
            Self::Io(_) => ErrorCategory::FileSystem,
        }
    }
}

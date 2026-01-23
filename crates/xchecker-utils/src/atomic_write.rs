//! Atomic file operations with cross-platform support (FR-FS)
//!
//! This module provides atomic file write operations with:
//! - Temporary file creation with fsync
//! - Atomic rename (same filesystem)
//! - Windows rename retry with exponential backoff (≤ 250ms total)
//! - Cross-filesystem fallback (copy→fsync→replace)
//! - Warning tracking for retries and fallbacks

use anyhow::{Context, Result};
use camino::Utf8Path;
use std::fs;
use std::io::Write;
use std::path::Path;

use tempfile::NamedTempFile;

#[cfg(target_os = "windows")]
use std::{thread, time::Duration};

/// Result of an atomic write operation
#[derive(Debug, Clone, Default)]
pub struct AtomicWriteResult {
    /// Number of rename retries that occurred (Windows only)
    pub rename_retry_count: u32,
    /// Whether cross-filesystem fallback was used
    pub used_cross_filesystem_fallback: bool,
    /// Any warnings generated during the operation
    pub warnings: Vec<String>,
}

/// Atomically write content to a file using temp file + fsync + rename
///
/// This implements FR-FS-001 through FR-FS-005:
/// - FR-FS-001: Write to temporary file first, fsync, then atomically rename
/// - FR-FS-002: Windows rename retry with bounded exponential backoff (≤ 250ms)
/// - FR-FS-003: Track `rename_retry_count` in warnings
/// - FR-FS-004: UTF-8 encoding with LF line endings
/// - FR-FS-005: Cross-filesystem fallback (copy→fsync→replace)
pub fn write_file_atomic(path: &Utf8Path, content: &str) -> Result<AtomicWriteResult> {
    let mut result = AtomicWriteResult::default();

    // Normalize line endings to LF (FR-FS-004)
    let normalized_content = normalize_line_endings(content);

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create parent directory: {parent}"))?;
    }

    // Create temporary file in the same directory as the target
    let temp_dir = path.parent().unwrap_or_else(|| Utf8Path::new("."));
    let mut temp_file = NamedTempFile::new_in(temp_dir)
        .with_context(|| format!("Failed to create temporary file in: {temp_dir}"))?;

    // Write content to temporary file
    temp_file
        .write_all(normalized_content.as_bytes())
        .with_context(|| "Failed to write content to temporary file")?;

    // Ensure data is written to disk (FR-FS-001)
    temp_file
        .as_file()
        .sync_all()
        .with_context(|| "Failed to fsync temporary file")?;

    // Get the temp file path before attempting rename (for cross-filesystem fallback)
    let temp_path = temp_file.path().to_path_buf();

    // Attempt atomic rename with platform-specific retry logic
    let rename_result = atomic_rename(temp_file, path.as_std_path());

    match rename_result {
        Ok(retry_count) => {
            result.rename_retry_count = retry_count;
            if retry_count > 0 {
                result.warnings.push(format!(
                    "Rename required {retry_count} retries due to transient filesystem locks"
                ));
            }
        }
        Err(e) if is_cross_filesystem_error(&e) => {
            // FR-FS-005: Cross-filesystem fallback
            result.used_cross_filesystem_fallback = true;
            result
                .warnings
                .push("Used cross-filesystem fallback (copy→fsync→replace)".to_string());

            // Fallback: copy→fsync→replace
            cross_filesystem_copy_from_path(&temp_path, path)?;
        }
        Err(e) => {
            return Err(e).with_context(|| format!("Failed to atomically write file: {path}"));
        }
    }

    Ok(result)
}

/// Normalize line endings to LF (FR-FS-004)
fn normalize_line_endings(content: &str) -> String {
    content.replace("\r\n", "\n").replace('\r', "\n")
}

/// Attempt atomic rename with platform-specific retry logic
///
/// Returns the number of retries that were needed.
/// On Windows, implements exponential backoff with ≤ 250ms total (FR-FS-002)
#[cfg(target_os = "windows")]
fn atomic_rename(mut temp_file: NamedTempFile, target: &Path) -> Result<u32> {
    use std::io::ErrorKind;

    const MAX_RETRIES: u32 = 5;
    const INITIAL_DELAY_MS: u64 = 10;
    const MAX_TOTAL_DELAY_MS: u64 = 250;

    let mut retry_count = 0;
    let mut total_delay_ms = 0;

    loop {
        // Try to persist
        match temp_file.persist(target) {
            Ok(_) => return Ok(retry_count),
            Err(persist_error) => {
                // Check if we should retry
                if retry_count >= MAX_RETRIES {
                    return Err(anyhow::anyhow!(persist_error.error));
                }

                // Check if this is a retryable error (permission denied, access denied)
                let is_retryable = matches!(
                    persist_error.error.kind(),
                    ErrorKind::PermissionDenied | ErrorKind::Other
                );

                if !is_retryable {
                    return Err(anyhow::anyhow!(persist_error.error));
                }

                // Calculate delay with exponential backoff
                let delay_ms = INITIAL_DELAY_MS * 2_u64.pow(retry_count);

                // Check if we would exceed total delay budget
                if total_delay_ms + delay_ms > MAX_TOTAL_DELAY_MS {
                    // Use remaining budget
                    let remaining = MAX_TOTAL_DELAY_MS.saturating_sub(total_delay_ms);
                    if remaining > 0 {
                        thread::sleep(Duration::from_millis(remaining));
                    }
                    // One final attempt
                    return persist_error
                        .file
                        .persist(target)
                        .map(|_| retry_count + 1)
                        .map_err(|e| anyhow::anyhow!(e.error));
                }

                // Sleep and retry
                thread::sleep(Duration::from_millis(delay_ms));
                total_delay_ms += delay_ms;
                retry_count += 1;

                // Get the temp file back for next iteration
                temp_file = persist_error.file;
            }
        }
    }
}

/// Attempt atomic rename (Unix: no retry needed)
#[cfg(not(target_os = "windows"))]
fn atomic_rename(temp_file: NamedTempFile, target: &Path) -> Result<u32> {
    temp_file
        .persist(target)
        .map(|_| 0) // No retries on Unix
        .map_err(|e| anyhow::anyhow!(e.error))
}

/// Check if an error indicates a cross-filesystem operation
#[cfg(unix)]
fn is_cross_filesystem_error(err: &anyhow::Error) -> bool {
    use std::io::ErrorKind;

    if let Some(io_error) = err.downcast_ref::<std::io::Error>() {
        if io_error.kind() != ErrorKind::Other {
            return false;
        }
        match io_error.raw_os_error() {
            Some(code) => code == 18, // EXDEV on Linux/macOS
            None => false,
        }
    } else {
        false
    }
}

/// Check if an error indicates a cross-filesystem operation
#[cfg(windows)]
fn is_cross_filesystem_error(_err: &anyhow::Error) -> bool {
    false
}

/// Perform cross-filesystem copy: copy→fsync→replace (FR-FS-005)
fn cross_filesystem_copy_from_path(temp_path: &Path, target: &Utf8Path) -> Result<()> {
    // Read content from temp file
    let content = fs::read(temp_path)
        .with_context(|| "Failed to read temporary file for cross-filesystem copy")?;

    // Create a new temp file in the target directory
    let target_dir = target.parent().unwrap_or_else(|| Utf8Path::new("."));
    let mut target_temp = NamedTempFile::new_in(target_dir)
        .with_context(|| format!("Failed to create temp file in target directory: {target_dir}"))?;

    // Write content
    target_temp
        .write_all(&content)
        .with_context(|| "Failed to write content during cross-filesystem copy")?;

    // Fsync
    target_temp
        .as_file()
        .sync_all()
        .with_context(|| "Failed to fsync during cross-filesystem copy")?;

    // Atomic rename (should succeed since we're on the same filesystem now)
    target_temp
        .persist(target.as_std_path())
        .map_err(|e| anyhow::anyhow!(e.error))
        .with_context(|| "Failed to persist during cross-filesystem copy")?;

    // Clean up original temp file
    let _ = fs::remove_file(temp_path);

    Ok(())
}

/// Read a file with CRLF tolerance (FR-FS-005)
///
/// This reads a file and normalizes line endings to LF, making it tolerant
/// of CRLF line endings on Windows.
#[allow(dead_code)] // Test utility for cross-platform testing
pub fn read_file_with_crlf_tolerance(path: &Utf8Path) -> Result<String> {
    let content = fs::read_to_string(path.as_std_path())
        .with_context(|| format!("Failed to read file: {path}"))?;

    Ok(normalize_line_endings(&content))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_temp_dir() -> TempDir {
        TempDir::new().unwrap()
    }

    #[test]
    fn test_normalize_line_endings() {
        assert_eq!(
            normalize_line_endings("line1\r\nline2\r\nline3"),
            "line1\nline2\nline3"
        );
        assert_eq!(
            normalize_line_endings("line1\rline2\rline3"),
            "line1\nline2\nline3"
        );
        assert_eq!(
            normalize_line_endings("line1\nline2\nline3"),
            "line1\nline2\nline3"
        );
        assert_eq!(
            normalize_line_endings("mixed\r\nline\nending\r"),
            "mixed\nline\nending\n"
        );
    }

    #[test]
    fn test_atomic_write_basic() {
        let temp_dir = create_temp_dir();
        let path_buf = temp_dir.path().join("test.txt");
        let file_path = Utf8Path::from_path(path_buf.as_path()).unwrap();

        let content = "test content\nwith multiple lines";
        let result = write_file_atomic(file_path, content);

        assert!(result.is_ok());
        let write_result = result.unwrap();
        assert_eq!(write_result.rename_retry_count, 0);
        assert!(!write_result.used_cross_filesystem_fallback);
        assert!(write_result.warnings.is_empty());

        // Verify file exists and has correct content
        assert!(file_path.exists());
        let read_content = fs::read_to_string(file_path.as_std_path()).unwrap();
        assert_eq!(read_content, content);
    }

    #[test]
    fn test_atomic_write_normalizes_line_endings() {
        let temp_dir = create_temp_dir();
        let path_buf = temp_dir.path().join("test_crlf.txt");
        let file_path = Utf8Path::from_path(path_buf.as_path()).unwrap();

        let content_with_crlf = "line1\r\nline2\r\nline3";
        let result = write_file_atomic(file_path, content_with_crlf);

        assert!(result.is_ok());

        // Verify content has LF line endings
        let read_content = fs::read_to_string(file_path.as_std_path()).unwrap();
        assert_eq!(read_content, "line1\nline2\nline3");
        assert!(!read_content.contains("\r\n"));
    }

    #[test]
    fn test_atomic_write_creates_parent_directory() {
        let temp_dir = create_temp_dir();
        let path_buf = temp_dir.path().join("nested").join("dir").join("test.txt");
        let nested_path = Utf8Path::from_path(path_buf.as_path()).unwrap();

        let content = "test content";
        let result = write_file_atomic(nested_path, content);

        assert!(result.is_ok());
        assert!(nested_path.exists());

        let read_content = fs::read_to_string(nested_path.as_std_path()).unwrap();
        assert_eq!(read_content, content);
    }

    #[test]
    fn test_atomic_write_overwrites_existing() {
        let temp_dir = create_temp_dir();
        let path_buf = temp_dir.path().join("overwrite.txt");
        let file_path = Utf8Path::from_path(path_buf.as_path()).unwrap();

        // Write initial content
        let initial_content = "initial content";
        write_file_atomic(file_path, initial_content).unwrap();

        // Overwrite with new content
        let new_content = "new content";
        let result = write_file_atomic(file_path, new_content);

        assert!(result.is_ok());

        // Verify new content
        let read_content = fs::read_to_string(file_path.as_std_path()).unwrap();
        assert_eq!(read_content, new_content);
    }

    #[test]
    fn test_read_file_with_crlf_tolerance() {
        let temp_dir = create_temp_dir();
        let path_buf = temp_dir.path().join("crlf_test.txt");
        let file_path = Utf8Path::from_path(path_buf.as_path()).unwrap();

        // Write file with CRLF line endings directly (bypassing our atomic write)
        let content_with_crlf = b"line1\r\nline2\r\nline3";
        fs::write(file_path.as_std_path(), content_with_crlf).unwrap();

        // Read with CRLF tolerance
        let result = read_file_with_crlf_tolerance(file_path);

        assert!(result.is_ok());
        let content = result.unwrap();
        assert_eq!(content, "line1\nline2\nline3");
        assert!(!content.contains('\r'));
    }

    #[test]
    fn test_atomic_write_empty_content() {
        let temp_dir = create_temp_dir();
        let path_buf = temp_dir.path().join("empty.txt");
        let file_path = Utf8Path::from_path(path_buf.as_path()).unwrap();

        let result = write_file_atomic(file_path, "");

        assert!(result.is_ok());
        assert!(file_path.exists());

        let read_content = fs::read_to_string(file_path.as_std_path()).unwrap();
        assert_eq!(read_content, "");
    }

    #[test]
    fn test_atomic_write_large_content() {
        let temp_dir = create_temp_dir();
        let path_buf = temp_dir.path().join("large.txt");
        let file_path = Utf8Path::from_path(path_buf.as_path()).unwrap();

        // Create large content (1 MB)
        let large_content = "x".repeat(1024 * 1024);
        let result = write_file_atomic(file_path, &large_content);

        assert!(result.is_ok());
        assert!(file_path.exists());

        let read_content = fs::read_to_string(file_path.as_std_path()).unwrap();
        assert_eq!(read_content.len(), large_content.len());
    }

    #[test]
    fn test_atomic_write_unicode_content() {
        let temp_dir = create_temp_dir();
        let path_buf = temp_dir.path().join("unicode.txt");
        let file_path = Utf8Path::from_path(path_buf.as_path()).unwrap();

        let unicode_content = "Hello 世界 🌍 Привет مرحبا";
        let result = write_file_atomic(file_path, unicode_content);

        assert!(result.is_ok());

        let read_content = fs::read_to_string(file_path.as_std_path()).unwrap();
        assert_eq!(read_content, unicode_content);
    }

    #[test]
    fn test_atomic_write_special_characters() {
        let temp_dir = create_temp_dir();
        let path_buf = temp_dir.path().join("special.txt");
        let file_path = Utf8Path::from_path(path_buf.as_path()).unwrap();

        let special_content = "Special chars: \t\n\"'`$\\{}[]()";
        let result = write_file_atomic(file_path, special_content);

        assert!(result.is_ok());

        let read_content = fs::read_to_string(file_path.as_std_path()).unwrap();
        // Note: \n will be preserved, but \r\n would be normalized
        assert!(read_content.contains("Special chars:"));
    }
}

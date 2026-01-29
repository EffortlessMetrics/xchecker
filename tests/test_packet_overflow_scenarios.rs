//! Comprehensive packet overflow scenario tests (FR-PKT-002, FR-PKT-003, FR-PKT-004, FR-PKT-005)
//!
//! **WHITE-BOX TEST**: This test uses internal module APIs (`packet::PacketBuilder`,
//! `exit_codes::codes`) and may break with internal refactors. These tests are intentionally
//! white-box to validate internal implementation details. See FR-TEST-4 for white-box test policy.
//!
//! This test suite verifies:
//! - Upstream file budget checking
//! - Failure before Claude invocation on overflow
//! - Manifest writing on overflow
//! - Receipt with actual size and limits
//! - Upstream files alone exceed budget
//! - Regular files excluded when budget reached
//! - Exit code 7 on overflow

use anyhow::Result;
use camino::Utf8PathBuf;
use std::fs;
use tempfile::TempDir;
use xchecker::error::XCheckerError;
use xchecker::exit_codes::codes;
use xchecker::packet::PacketBuilder;

/// Test FR-PKT-002: Upstream files alone exceed budget
#[test]
fn test_upstream_files_alone_exceed_budget() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");

    // Create multiple upstream files that together exceed budget
    fs::write(base_path.join("file1.core.yaml"), "x".repeat(60))?;
    fs::write(base_path.join("file2.core.yaml"), "y".repeat(60))?;
    fs::write(base_path.join("file3.core.yaml"), "z".repeat(60))?;

    // Set very small limits
    let mut builder = PacketBuilder::with_limits(100, 10)?;
    let result = builder.build_packet(&base_path, "test", &context_dir, None);

    // Should fail with PacketOverflow
    assert!(result.is_err());

    let err = result.unwrap_err();
    let root_cause = err.root_cause();

    // Verify it's a PacketOverflow error
    if let Some(xchecker_err) = root_cause.downcast_ref::<XCheckerError>() {
        match xchecker_err {
            XCheckerError::PacketOverflow {
                used_bytes,
                used_lines,
                limit_bytes,
                limit_lines,
            } => {
                // Verify the error contains the actual values
                assert!(*used_bytes > *limit_bytes || *used_lines > *limit_lines);
                assert_eq!(*limit_bytes, 100);
                assert_eq!(*limit_lines, 10);
            }
            _ => panic!("Expected PacketOverflow error, got: {xchecker_err:?}"),
        }
    }

    // Verify manifest was written
    let manifest_file = context_dir.join("test-packet.manifest.json");
    assert!(
        manifest_file.exists(),
        "Manifest should be written on overflow"
    );

    // Verify manifest contains overflow information
    let manifest_content = fs::read_to_string(&manifest_file)?;
    assert!(manifest_content.contains("\"overflow\": true"));
    assert!(manifest_content.contains("\"max_bytes\": 100"));
    assert!(manifest_content.contains("\"max_lines\": 10"));
    assert!(manifest_content.contains("used_bytes"));
    assert!(manifest_content.contains("used_lines"));

    Ok(())
}

/// Test FR-PKT-003: Regular files excluded when budget reached
#[test]
fn test_regular_files_excluded_when_budget_reached() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");

    // Create small upstream file and large regular files
    fs::write(base_path.join("small.core.yaml"), "key: value")?;
    fs::write(base_path.join("large1.md"), "x".repeat(500))?;
    fs::write(base_path.join("large2.md"), "y".repeat(500))?;

    // Set limits that allow upstream but not all regular files
    let mut builder = PacketBuilder::with_limits(200, 30)?;
    let packet = builder.build_packet(&base_path, "test", &context_dir, None)?;

    // Upstream file should be included
    assert!(
        packet
            .evidence
            .files
            .iter()
            .any(|f| f.path.contains("small.core.yaml")),
        "Upstream file should always be included"
    );

    // At least one large file should be excluded
    let large_files_included = packet
        .evidence
        .files
        .iter()
        .filter(|f| f.path.contains("large"))
        .count();

    // Not all large files should be included due to budget constraints
    assert!(
        large_files_included < 2,
        "Not all large files should be included when budget is tight"
    );

    Ok(())
}

/// Test FR-PKT-004: Failure before Claude invocation
#[test]
fn test_failure_before_claude_invocation() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");

    // Create large upstream file
    fs::write(base_path.join("large.core.yaml"), "x".repeat(1000))?;

    // Use very small limits
    let mut builder = PacketBuilder::with_limits(50, 5)?;
    let result = builder.build_packet(&base_path, "test", &context_dir, None);

    // Should fail immediately during packet building, before any Claude invocation
    assert!(result.is_err(), "Should fail before Claude invocation");

    // The error should be PacketOverflow
    let err = result.unwrap_err();
    let root_cause = err.root_cause();

    if let Some(xchecker_err) = root_cause.downcast_ref::<XCheckerError>() {
        assert!(
            matches!(xchecker_err, XCheckerError::PacketOverflow { .. }),
            "Should be PacketOverflow error"
        );
    }

    Ok(())
}

/// Test FR-PKT-005: Manifest written to context/<phase>-packet.manifest.json
///
/// Note: Files must fit within max_file_size individually but exceed packet
/// budget together to trigger manifest writing on overflow.
#[test]
fn test_manifest_written_to_correct_location() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");

    // Create upstream files that fit within file limit (100 bytes) individually
    // but together exceed packet budget when combined with formatting
    fs::write(base_path.join("first.core.yaml"), "x".repeat(50))?;
    fs::write(base_path.join("second.core.yaml"), "y".repeat(50))?;

    // Use small limits to trigger overflow
    let mut builder = PacketBuilder::with_limits(100, 10)?;
    let _result = builder.build_packet(&base_path, "requirements", &context_dir, None);

    // Verify manifest file location
    let manifest_file = context_dir.join("requirements-packet.manifest.json");
    assert!(
        manifest_file.exists(),
        "Manifest should be at context/requirements-packet.manifest.json"
    );

    // Verify manifest structure
    let manifest_content = fs::read_to_string(&manifest_file)?;
    let manifest: serde_json::Value = serde_json::from_str(&manifest_content)?;

    assert_eq!(manifest["phase"], "requirements");
    assert_eq!(manifest["overflow"], true);
    assert!(manifest["budget"].is_object());
    assert!(manifest["files"].is_array());

    Ok(())
}

/// Test FR-PKT-005: Receipt includes `used_bytes`, `used_lines`, `limit_bytes`, `limit_lines`
#[test]
fn test_overflow_error_includes_all_fields() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");

    // Create large upstream file
    fs::write(base_path.join("large.core.yaml"), "x".repeat(500))?;

    // Use specific limits
    let max_bytes = 123;
    let max_lines = 45;
    let mut builder = PacketBuilder::with_limits(max_bytes, max_lines)?;
    let result = builder.build_packet(&base_path, "test", &context_dir, None);

    assert!(result.is_err());

    let err = result.unwrap_err();
    let root_cause = err.root_cause();

    if let Some(xchecker_err) = root_cause.downcast_ref::<XCheckerError>() {
        match xchecker_err {
            XCheckerError::PacketOverflow {
                used_bytes,
                used_lines,
                limit_bytes,
                limit_lines,
            } => {
                // Verify all fields are present and correct
                assert!(*used_bytes > 0, "used_bytes should be populated");
                assert!(*used_lines > 0, "used_lines should be populated");
                assert_eq!(
                    *limit_bytes, max_bytes,
                    "limit_bytes should match configured limit"
                );
                assert_eq!(
                    *limit_lines, max_lines,
                    "limit_lines should match configured limit"
                );

                // Verify overflow condition
                assert!(
                    *used_bytes > *limit_bytes || *used_lines > *limit_lines,
                    "At least one limit should be exceeded"
                );
            }
            _ => panic!("Expected PacketOverflow error"),
        }
    }

    Ok(())
}

/// Test FR-PKT-005: Exit code 7 on overflow
#[test]
fn test_exit_code_7_on_overflow() -> Result<()> {
    // Create a PacketOverflow error
    let err = XCheckerError::PacketOverflow {
        used_bytes: 100000,
        used_lines: 2000,
        limit_bytes: 65536,
        limit_lines: 1200,
    };

    // Verify it maps to exit code 7
    let (exit_code, error_kind) = (&err).into();
    assert_eq!(exit_code, codes::PACKET_OVERFLOW);
    assert_eq!(exit_code, 7);
    assert_eq!(error_kind, xchecker::types::ErrorKind::PacketOverflow);

    Ok(())
}

/// Test FR-PKT-006: Manifest contains file metadata but no content
#[test]
fn test_manifest_contains_metadata_not_content() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");

    // Create file with identifiable content
    let secret_content = "SECRET_DATA_12345";
    fs::write(base_path.join("data.core.yaml"), secret_content)?;

    // Trigger overflow
    let mut builder = PacketBuilder::with_limits(50, 5)?;
    let _result = builder.build_packet(&base_path, "test", &context_dir, None);

    // Read manifest
    let manifest_file = context_dir.join("test-packet.manifest.json");
    let manifest_content = fs::read_to_string(&manifest_file)?;

    // Verify file path is present
    assert!(manifest_content.contains("data.core.yaml"));

    // Verify metadata is present
    assert!(manifest_content.contains("blake3_pre_redaction"));
    assert!(manifest_content.contains("priority"));

    // Verify actual content is NOT present
    assert!(
        !manifest_content.contains(secret_content),
        "Manifest should not contain file content"
    );

    Ok(())
}

/// Test FR-PKT-002: Byte limit overflow with upstream files
#[test]
fn test_byte_limit_overflow_upstream() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");

    // Create upstream file that exceeds byte limit
    fs::write(base_path.join("large.core.yaml"), "x".repeat(200))?;

    // Set byte limit very low
    let mut builder = PacketBuilder::with_limits(100, 10000)?;
    let result = builder.build_packet(&base_path, "test", &context_dir, None);

    assert!(result.is_err());

    let err = result.unwrap_err();
    let root_cause = err.root_cause();

    if let Some(xchecker_err) = root_cause.downcast_ref::<XCheckerError>() {
        match xchecker_err {
            XCheckerError::PacketOverflow {
                used_bytes,
                limit_bytes,
                ..
            } => {
                assert!(*used_bytes > *limit_bytes, "Byte limit should be exceeded");
            }
            _ => panic!("Expected PacketOverflow error"),
        }
    }

    Ok(())
}

/// Test FR-PKT-002: Line limit overflow with upstream files
#[test]
fn test_line_limit_overflow_upstream() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");

    // Create upstream file with many lines
    fs::write(base_path.join("lines.core.yaml"), "line\n".repeat(100))?;

    // Set line limit very low
    let mut builder = PacketBuilder::with_limits(100000, 10)?;
    let result = builder.build_packet(&base_path, "test", &context_dir, None);

    assert!(result.is_err());

    let err = result.unwrap_err();
    let root_cause = err.root_cause();

    if let Some(xchecker_err) = root_cause.downcast_ref::<XCheckerError>() {
        match xchecker_err {
            XCheckerError::PacketOverflow {
                used_lines,
                limit_lines,
                ..
            } => {
                assert!(*used_lines > *limit_lines, "Line limit should be exceeded");
            }
            _ => panic!("Expected PacketOverflow error"),
        }
    }

    Ok(())
}

/// Test FR-PKT-003: Multiple upstream files with budget constraints
#[test]
fn test_multiple_upstream_files_budget() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");

    // Create multiple upstream files
    fs::write(base_path.join("file1.core.yaml"), "data1: ".repeat(20))?;
    fs::write(base_path.join("file2.core.yaml"), "data2: ".repeat(20))?;
    fs::write(base_path.join("file3.core.yaml"), "data3: ".repeat(20))?;

    // Set limits that can't accommodate all upstream files
    let mut builder = PacketBuilder::with_limits(150, 20)?;
    let result = builder.build_packet(&base_path, "test", &context_dir, None);

    // Should fail because upstream files are non-evictable
    assert!(result.is_err());

    // Verify manifest includes all upstream files that were processed
    let manifest_file = context_dir.join("test-packet.manifest.json");
    assert!(manifest_file.exists());

    let manifest_content = fs::read_to_string(&manifest_file)?;

    // At least one upstream file should be in the manifest
    assert!(
        manifest_content.contains("file1.core.yaml")
            || manifest_content.contains("file2.core.yaml")
            || manifest_content.contains("file3.core.yaml")
    );

    Ok(())
}

/// Test FR-PKT-004: Packet preview written even on overflow
///
/// Note: Files must fit within max_file_size individually but exceed packet
/// budget together to trigger preview writing on overflow.
#[test]
fn test_packet_preview_on_overflow() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");

    // Create upstream files that fit within file limit (100 bytes) individually
    // but together exceed packet budget when combined with formatting
    fs::write(base_path.join("first.core.yaml"), "x".repeat(50))?;
    fs::write(base_path.join("second.core.yaml"), "y".repeat(50))?;

    // Trigger overflow
    let mut builder = PacketBuilder::with_limits(100, 10)?;
    let _result = builder.build_packet(&base_path, "design", &context_dir, None);

    // Verify packet preview was written
    let preview_file = context_dir.join("design-packet.txt");
    assert!(
        preview_file.exists(),
        "Packet preview should be written even on overflow"
    );

    // Verify preview contains file marker
    let preview_content = fs::read_to_string(&preview_file)?;
    assert!(preview_content.contains("==="));
    assert!(
        preview_content.contains("first.core.yaml") || preview_content.contains("second.core.yaml")
    );

    Ok(())
}

/// Test FR-PKT-005: Manifest includes budget information
///
/// Note: Files must fit within max_file_size individually but exceed packet
/// budget together to trigger manifest writing on overflow.
#[test]
fn test_manifest_includes_budget_info() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");

    // Create upstream files that fit within file limit (200 bytes) individually
    // but together exceed packet budget when combined with formatting
    fs::write(base_path.join("first.core.yaml"), "x".repeat(100))?;
    fs::write(base_path.join("second.core.yaml"), "y".repeat(100))?;

    // Use specific limits - file limit is 200, packet budget is 200
    // Files fit in file limit but together exceed packet budget
    let max_bytes = 200;
    let max_lines = 25;
    let mut builder = PacketBuilder::with_limits(max_bytes, max_lines)?;
    let _result = builder.build_packet(&base_path, "test", &context_dir, None);

    // Read and parse manifest
    let manifest_file = context_dir.join("test-packet.manifest.json");
    let manifest_content = fs::read_to_string(&manifest_file)?;
    let manifest: serde_json::Value = serde_json::from_str(&manifest_content)?;

    // Verify budget section
    assert!(manifest["budget"].is_object());
    assert_eq!(manifest["budget"]["max_bytes"], max_bytes);
    assert_eq!(manifest["budget"]["max_lines"], max_lines);
    assert!(manifest["budget"]["used_bytes"].is_number());
    assert!(manifest["budget"]["used_lines"].is_number());

    // Verify overflow flag
    assert_eq!(manifest["overflow"], true);

    Ok(())
}

/// Test FR-PKT-006: Manifest includes file priorities
///
/// Note: Files must fit within max_file_size individually but exceed packet
/// budget together to trigger manifest writing on overflow.
#[test]
fn test_manifest_includes_file_priorities() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");

    // Create upstream files that fit within file limit (100 bytes) individually
    // but together exceed packet budget when combined with formatting
    fs::write(base_path.join("upstream.core.yaml"), "x".repeat(50))?;
    fs::write(base_path.join("another.core.yaml"), "y".repeat(50))?;

    // Trigger overflow with small budget that will definitely be exceeded
    let mut builder = PacketBuilder::with_limits(100, 5)?;
    let _result = builder.build_packet(&base_path, "test", &context_dir, None);

    // Read manifest
    let manifest_file = context_dir.join("test-packet.manifest.json");
    let manifest_content = fs::read_to_string(&manifest_file)?;
    let manifest: serde_json::Value = serde_json::from_str(&manifest_content)?;

    // Verify files array has priority information
    assert!(manifest["files"].is_array());
    let files = manifest["files"].as_array().unwrap();

    for file in files {
        assert!(
            file["priority"].is_string(),
            "Each file should have a priority"
        );
        assert!(file["path"].is_string(), "Each file should have a path");
        assert!(
            file["blake3_pre_redaction"].is_string(),
            "Each file should have a hash"
        );
    }

    Ok(())
}

/// Test FR-PKT-002: Overflow detection is deterministic
#[test]
fn test_overflow_detection_deterministic() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");

    // Create large upstream file
    fs::write(base_path.join("large.core.yaml"), "x".repeat(500))?;

    // Run multiple times with same limits
    let max_bytes = 100;
    let max_lines = 10;

    for _ in 0..3 {
        let mut builder = PacketBuilder::with_limits(max_bytes, max_lines)?;
        let result = builder.build_packet(&base_path, "test", &context_dir, None);

        // Should always fail
        assert!(result.is_err());

        // Should always be PacketOverflow
        let err = result.unwrap_err();
        let root_cause = err.root_cause();

        if let Some(xchecker_err) = root_cause.downcast_ref::<XCheckerError>() {
            assert!(
                matches!(xchecker_err, XCheckerError::PacketOverflow { .. }),
                "Should consistently be PacketOverflow error"
            );
        }
    }

    Ok(())
}

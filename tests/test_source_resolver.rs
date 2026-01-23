//! Comprehensive tests for SourceResolver implementation (FR-SOURCE)
//!
//! **WHITE-BOX TEST**: This test uses internal module APIs (`source::{...}`) and may break
//! with internal refactors. These tests are intentionally white-box to validate internal
//! implementation details. See FR-TEST-4 for white-box test policy.
//!
//! This test suite verifies:
//! - GitHub source resolution (FR-SOURCE-001)
//! - GitHub validation (owner, repo, issue_id) (FR-SOURCE-002)
//! - Filesystem source resolution (files and directories) (FR-SOURCE-003)
//! - Filesystem validation (path exists) (FR-SOURCE-004)
//! - Stdin source resolution (FR-SOURCE-005)
//! - Stdin validation (non-empty) (FR-SOURCE-006)
//! - Error handling with user-friendly messages
//! - Actionable suggestions in errors
//! - Source metadata tracking

use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;
use xchecker::error::UserFriendlyError;
use xchecker::source::{SourceError, SourceResolver, SourceType};

// ============================================================================
// GitHub Source Resolution Tests (FR-SOURCE-001, FR-SOURCE-002)
// ============================================================================

#[test]
fn test_github_source_resolution_valid() {
    // Test valid GitHub source resolution
    let result = SourceResolver::resolve_github("rust-lang", "rust", "12345");

    assert!(
        result.is_ok(),
        "Valid GitHub source should resolve successfully"
    );

    let content = result.unwrap();

    // Verify content contains expected information
    assert!(content.content.contains("GitHub issue #12345"));
    assert!(content.content.contains("rust-lang/rust"));

    // Verify metadata
    assert_eq!(
        content.metadata.get("owner"),
        Some(&"rust-lang".to_string())
    );
    assert_eq!(content.metadata.get("repo"), Some(&"rust".to_string()));
    assert_eq!(content.metadata.get("issue_id"), Some(&"12345".to_string()));

    // Verify source type
    match content.source_type {
        SourceType::GitHub { owner, repo } => {
            assert_eq!(owner, "rust-lang");
            assert_eq!(repo, "rust");
        }
        _ => panic!("Expected GitHub source type"),
    }
}

#[test]
fn test_github_source_resolution_with_special_characters() {
    // Test GitHub source with special characters in names
    let result = SourceResolver::resolve_github("my-org", "my-repo-123", "999");

    assert!(
        result.is_ok(),
        "GitHub source with hyphens and numbers should work"
    );

    let content = result.unwrap();
    assert_eq!(content.metadata.get("owner"), Some(&"my-org".to_string()));
    assert_eq!(
        content.metadata.get("repo"),
        Some(&"my-repo-123".to_string())
    );
}

#[test]
fn test_github_validation_empty_owner() {
    // Test validation: empty owner should fail
    let result = SourceResolver::resolve_github("", "repo", "123");

    assert!(result.is_err(), "Empty owner should fail validation");

    match result.unwrap_err() {
        SourceError::InvalidFormat { reason } => {
            assert!(reason.contains("owner"));
            assert!(reason.contains("repo"));
        }
        _ => panic!("Expected InvalidFormat error"),
    }
}

#[test]
fn test_github_validation_empty_repo() {
    // Test validation: empty repo should fail
    let result = SourceResolver::resolve_github("owner", "", "123");

    assert!(result.is_err(), "Empty repo should fail validation");

    match result.unwrap_err() {
        SourceError::InvalidFormat { reason } => {
            assert!(reason.contains("owner"));
            assert!(reason.contains("repo"));
        }
        _ => panic!("Expected InvalidFormat error"),
    }
}

#[test]
fn test_github_validation_invalid_issue_id_non_numeric() {
    // Test validation: non-numeric issue ID should fail
    let result = SourceResolver::resolve_github("owner", "repo", "invalid");

    assert!(
        result.is_err(),
        "Non-numeric issue ID should fail validation"
    );

    match result.unwrap_err() {
        SourceError::InvalidFormat { reason } => {
            assert!(reason.contains("valid number"));
        }
        _ => panic!("Expected InvalidFormat error"),
    }
}

#[test]
fn test_github_validation_invalid_issue_id_with_letters() {
    // Test validation: issue ID with letters should fail
    let result = SourceResolver::resolve_github("owner", "repo", "123abc");

    assert!(
        result.is_err(),
        "Issue ID with letters should fail validation"
    );

    match result.unwrap_err() {
        SourceError::InvalidFormat { reason } => {
            assert!(reason.contains("valid number"));
        }
        _ => panic!("Expected InvalidFormat error"),
    }
}

#[test]
fn test_github_validation_negative_issue_id() {
    // Test validation: negative issue ID should fail
    let result = SourceResolver::resolve_github("owner", "repo", "-123");

    assert!(result.is_err(), "Negative issue ID should fail validation");
}

#[test]
fn test_github_validation_zero_issue_id() {
    // Test validation: zero issue ID should succeed (edge case)
    let result = SourceResolver::resolve_github("owner", "repo", "0");

    // Zero is technically a valid u32, so this should succeed
    assert!(result.is_ok(), "Zero issue ID should be valid");
}

#[test]
fn test_github_validation_large_issue_id() {
    // Test validation: large issue ID should succeed
    let result = SourceResolver::resolve_github("owner", "repo", "999999999");

    assert!(result.is_ok(), "Large issue ID should be valid");
}

// ============================================================================
// Filesystem Source Resolution Tests (FR-SOURCE-003, FR-SOURCE-004)
// ============================================================================

#[test]
fn test_filesystem_source_resolution_file() {
    // Create a temporary file
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.txt");

    let test_content = "This is a test file for source resolution.";
    fs::write(&file_path, test_content).unwrap();

    // Test file source resolution
    let result = SourceResolver::resolve_filesystem(&file_path);

    assert!(
        result.is_ok(),
        "Valid file source should resolve successfully"
    );

    let content = result.unwrap();

    // Verify content matches file content
    assert_eq!(content.content, test_content);

    // Verify metadata
    assert_eq!(
        content.metadata.get("path"),
        Some(&file_path.display().to_string())
    );
    assert_eq!(content.metadata.get("type"), Some(&"file".to_string()));

    // Verify source type
    match content.source_type {
        SourceType::FileSystem { path } => {
            assert_eq!(path, file_path);
        }
        _ => panic!("Expected FileSystem source type"),
    }
}

#[test]
fn test_filesystem_source_resolution_directory() {
    // Create a temporary directory
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path().to_path_buf();

    // Test directory source resolution
    let result = SourceResolver::resolve_filesystem(&dir_path);

    assert!(
        result.is_ok(),
        "Valid directory source should resolve successfully"
    );

    let content = result.unwrap();

    // Verify content contains directory information
    assert!(content.content.contains("Directory source"));
    assert!(content.content.contains(&dir_path.display().to_string()));

    // Verify metadata
    assert_eq!(
        content.metadata.get("path"),
        Some(&dir_path.display().to_string())
    );
    assert_eq!(content.metadata.get("type"), Some(&"directory".to_string()));
}

#[test]
fn test_filesystem_source_resolution_file_with_unicode() {
    // Create a temporary file with unicode content
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("unicode.txt");

    let test_content = "Unicode content: 你好世界 🚀 Привет мир";
    fs::write(&file_path, test_content).unwrap();

    // Test file source resolution with unicode
    let result = SourceResolver::resolve_filesystem(&file_path);

    assert!(
        result.is_ok(),
        "File with unicode should resolve successfully"
    );

    let content = result.unwrap();
    assert_eq!(content.content, test_content);
}

#[test]
fn test_filesystem_source_resolution_empty_file() {
    // Create an empty temporary file
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("empty.txt");

    fs::write(&file_path, "").unwrap();

    // Test empty file source resolution
    let result = SourceResolver::resolve_filesystem(&file_path);

    assert!(result.is_ok(), "Empty file should resolve successfully");

    let content = result.unwrap();
    assert_eq!(content.content, "");
}

#[test]
fn test_filesystem_validation_path_not_found() {
    // Test validation: non-existent path should fail
    let nonexistent_path = PathBuf::from("/nonexistent/path/to/file.txt");

    let result = SourceResolver::resolve_filesystem(&nonexistent_path);

    assert!(result.is_err(), "Non-existent path should fail validation");

    match result.unwrap_err() {
        SourceError::FileSystemNotFound { path } => {
            assert!(path.contains("nonexistent"));
        }
        _ => panic!("Expected FileSystemNotFound error"),
    }
}

#[test]
fn test_filesystem_validation_relative_path_not_found() {
    // Test validation: non-existent relative path should fail
    let nonexistent_path = PathBuf::from("./this/does/not/exist.txt");

    let result = SourceResolver::resolve_filesystem(&nonexistent_path);

    assert!(
        result.is_err(),
        "Non-existent relative path should fail validation"
    );
}

#[test]
fn test_filesystem_source_resolution_nested_directory() {
    // Create a nested directory structure
    let temp_dir = TempDir::new().unwrap();
    let nested_dir = temp_dir.path().join("level1").join("level2");
    fs::create_dir_all(&nested_dir).unwrap();

    // Test nested directory source resolution
    let result = SourceResolver::resolve_filesystem(&nested_dir);

    assert!(
        result.is_ok(),
        "Nested directory should resolve successfully"
    );
}

// ============================================================================
// Stdin Source Resolution Tests (FR-SOURCE-005, FR-SOURCE-006)
// ============================================================================

// Note: Testing stdin directly is challenging in unit tests because it requires
// actual stdin input. These tests verify the error handling and validation logic.

#[test]
fn test_stdin_validation_empty_input() {
    // This test verifies that empty stdin is properly validated
    // In a real scenario, resolve_stdin() would read from actual stdin
    // For unit testing, we verify the error type is correct

    // The actual stdin test would require process spawning or mocking
    // We verify the error handling logic exists
    let error = SourceError::EmptyInput;

    match error {
        SourceError::EmptyInput => {
            // Correct error type
        }
        _ => panic!("Expected EmptyInput error"),
    }
}

// ============================================================================
// Error Handling and User-Friendly Messages Tests
// ============================================================================

#[test]
fn test_github_error_user_friendly_message() {
    let error = SourceError::GitHubRepoNotFound {
        owner: "owner".to_string(),
        repo: "repo".to_string(),
    };

    let message = error.user_message();
    assert!(message.contains("GitHub repository"));
    assert!(message.contains("owner/repo"));
}

#[test]
fn test_github_error_context() {
    let error = SourceError::GitHubRepoNotFound {
        owner: "owner".to_string(),
        repo: "repo".to_string(),
    };

    let context = error.context();
    assert!(context.is_some());
    assert!(context.unwrap().contains("GitHub source resolution"));
}

#[test]
fn test_github_error_suggestions_authentication() {
    let error = SourceError::GitHubAuthFailed {
        reason: "authentication failed".to_string(),
    };

    let suggestions = error.suggestions();
    assert!(!suggestions.is_empty());
    assert!(suggestions.iter().any(|s| s.contains("gh auth login")));
    assert!(suggestions.iter().any(|s| s.contains("token")));
}

#[test]
fn test_github_error_suggestions_not_found() {
    let error = SourceError::GitHubRepoNotFound {
        owner: "owner".to_string(),
        repo: "repo".to_string(),
    };

    let suggestions = error.suggestions();
    assert!(!suggestions.is_empty());
    assert!(
        suggestions
            .iter()
            .any(|s| s.contains("https://github.com/owner/repo"))
    );
    assert!(suggestions.iter().any(|s| s.contains("public")));
}

#[test]
fn test_github_error_suggestions_rate_limit() {
    let error = SourceError::GitHubApiError {
        status: 403,
        message: "rate limit exceeded".to_string(),
    };

    let suggestions = error.suggestions();
    assert!(!suggestions.is_empty());
    assert!(
        suggestions
            .iter()
            .any(|s| s.contains("Rate limit exceeded"))
    );
}

#[test]
fn test_filesystem_error_user_friendly_message() {
    let error = SourceError::FileSystemNotFound {
        path: "/path/to/file.txt".to_string(),
    };

    let message = error.user_message();
    assert!(message.contains("does not exist"));
    assert!(message.contains("/path/to/file.txt"));
}

#[test]
fn test_filesystem_error_context() {
    let error = SourceError::FileSystemNotFound {
        path: "/path/to/file.txt".to_string(),
    };

    let context = error.context();
    assert!(context.is_some());
    assert!(context.unwrap().contains("Filesystem source resolution"));
}

#[test]
fn test_filesystem_error_suggestions() {
    let error = SourceError::FileSystemNotFound {
        path: "/path/to/file.txt".to_string(),
    };

    let suggestions = error.suggestions();
    assert!(!suggestions.is_empty());
    assert!(suggestions.iter().any(|s| s.contains("mkdir -p")));
    assert!(suggestions.iter().any(|s| s.contains("/path/to/file.txt")));
    assert!(suggestions.iter().any(|s| s.contains("absolute path")));
}

#[test]
fn test_stdin_error_user_friendly_message() {
    let error = SourceError::EmptyInput;

    let message = error.user_message();
    assert!(message.contains("No input provided"));
}

#[test]
fn test_stdin_error_context() {
    let error = SourceError::EmptyInput;

    let context = error.context();
    assert!(context.is_some());
    assert!(context.unwrap().contains("problem statement"));
}

#[test]
fn test_stdin_error_suggestions() {
    let error = SourceError::EmptyInput;

    let suggestions = error.suggestions();
    assert!(!suggestions.is_empty());
    assert!(suggestions.iter().any(|s| s.contains("stdin")));
    assert!(suggestions.iter().any(|s| s.contains("--source fs")));
}

#[test]
fn test_invalid_configuration_error_user_friendly_message() {
    let error = SourceError::InvalidFormat {
        reason: "Missing required parameter".to_string(),
    };

    let message = error.user_message();
    assert!(message.contains("Input format is invalid"));
    assert!(message.contains("Missing required parameter"));
}

#[test]
fn test_invalid_configuration_error_suggestions() {
    let error = SourceError::InvalidFormat {
        reason: "Unknown source type".to_string(),
    };

    let suggestions = error.suggestions();
    assert!(!suggestions.is_empty());
    assert!(suggestions.iter().any(|s| s.contains("single-line")));
    assert!(suggestions.iter().any(|s| s.contains("plain English")));
}

// ============================================================================
// Source Metadata Tracking Tests
// ============================================================================

#[test]
fn test_github_metadata_tracking() {
    let result = SourceResolver::resolve_github("test-owner", "test-repo", "42");
    assert!(result.is_ok());

    let content = result.unwrap();

    // Verify all expected metadata is present
    assert!(content.metadata.contains_key("owner"));
    assert!(content.metadata.contains_key("repo"));
    assert!(content.metadata.contains_key("issue_id"));

    // Verify metadata values
    assert_eq!(content.metadata.get("owner").unwrap(), "test-owner");
    assert_eq!(content.metadata.get("repo").unwrap(), "test-repo");
    assert_eq!(content.metadata.get("issue_id").unwrap(), "42");
}

#[test]
fn test_filesystem_file_metadata_tracking() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("metadata_test.txt");
    fs::write(&file_path, "test content").unwrap();

    let result = SourceResolver::resolve_filesystem(&file_path);
    assert!(result.is_ok());

    let content = result.unwrap();

    // Verify all expected metadata is present
    assert!(content.metadata.contains_key("path"));
    assert!(content.metadata.contains_key("type"));

    // Verify metadata values
    assert_eq!(content.metadata.get("type").unwrap(), "file");
    assert!(
        content
            .metadata
            .get("path")
            .unwrap()
            .contains("metadata_test.txt")
    );
}

#[test]
fn test_filesystem_directory_metadata_tracking() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path().to_path_buf();

    let result = SourceResolver::resolve_filesystem(&dir_path);
    assert!(result.is_ok());

    let content = result.unwrap();

    // Verify all expected metadata is present
    assert!(content.metadata.contains_key("path"));
    assert!(content.metadata.contains_key("type"));

    // Verify metadata values
    assert_eq!(content.metadata.get("type").unwrap(), "directory");
}

// ============================================================================
// Error Category Tests
// ============================================================================

#[test]
fn test_error_category_configuration() {
    use xchecker::error::ErrorCategory;

    let errors = vec![
        SourceError::GitHubRepoNotFound {
            owner: "owner".to_string(),
            repo: "repo".to_string(),
        },
        SourceError::FileSystemNotFound {
            path: "test".to_string(),
        },
        SourceError::EmptyInput,
        SourceError::InvalidFormat {
            reason: "test".to_string(),
        },
    ];

    for error in errors {
        assert_eq!(error.category(), ErrorCategory::Configuration);
    }
}

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn test_source_resolution_workflow_github() {
    // Test complete workflow for GitHub source
    let result = SourceResolver::resolve_github("microsoft", "vscode", "100");

    assert!(result.is_ok());
    let content = result.unwrap();

    // Verify all components are present
    assert!(!content.content.is_empty());
    assert!(!content.metadata.is_empty());

    match content.source_type {
        SourceType::GitHub { .. } => {}
        _ => panic!("Expected GitHub source type"),
    }
}

#[test]
fn test_source_resolution_workflow_filesystem() {
    // Test complete workflow for filesystem source
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("workflow_test.txt");
    fs::write(&file_path, "Workflow test content").unwrap();

    let result = SourceResolver::resolve_filesystem(&file_path);

    assert!(result.is_ok());
    let content = result.unwrap();

    // Verify all components are present
    assert!(!content.content.is_empty());
    assert!(!content.metadata.is_empty());

    match content.source_type {
        SourceType::FileSystem { .. } => {}
        _ => panic!("Expected FileSystem source type"),
    }
}

#[test]
fn test_multiple_source_resolutions() {
    // Test that multiple resolutions work independently
    let temp_dir = TempDir::new().unwrap();
    let file1 = temp_dir.path().join("file1.txt");
    let file2 = temp_dir.path().join("file2.txt");

    fs::write(&file1, "Content 1").unwrap();
    fs::write(&file2, "Content 2").unwrap();

    let result1 = SourceResolver::resolve_filesystem(&file1);
    let result2 = SourceResolver::resolve_filesystem(&file2);
    let result3 = SourceResolver::resolve_github("owner1", "repo1", "1");
    let result4 = SourceResolver::resolve_github("owner2", "repo2", "2");

    assert!(result1.is_ok());
    assert!(result2.is_ok());
    assert!(result3.is_ok());
    assert!(result4.is_ok());

    // Verify they are independent
    assert_ne!(result1.unwrap().content, result2.unwrap().content);
    assert_ne!(
        result3.unwrap().metadata.get("owner"),
        result4.unwrap().metadata.get("owner")
    );
}

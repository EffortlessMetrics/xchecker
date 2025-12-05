//! Integration tests for improved error messages and user guidance
//!
//! This test suite validates that all error types provide:
//! - Clear, user-friendly messages
//! - Contextual information
//! - Actionable suggestions
//! - Appropriate error categories

use std::path::PathBuf;
use xchecker::error::{
    ClaudeError, ConfigError, ErrorCategory, PhaseError, RunnerError, SourceError,
    UserFriendlyError, XCheckerError,
};
use xchecker::fixup::FixupError;
use xchecker::lock::LockError;
use xchecker::spec_id::SpecIdError;

#[test]
fn test_all_error_types_have_user_friendly_implementations() {
    // Test ConfigError
    let config_err = ConfigError::MissingRequired("model".to_string());
    assert!(!config_err.user_message().is_empty());
    assert!(config_err.context().is_some());
    assert!(!config_err.suggestions().is_empty());
    assert_eq!(config_err.category(), ErrorCategory::Configuration);

    // Test PhaseError
    let phase_err = PhaseError::ExecutionFailed {
        phase: "REQUIREMENTS".to_string(),
        code: 1,
    };
    assert!(!phase_err.user_message().is_empty());
    assert!(phase_err.context().is_some());
    assert!(!phase_err.suggestions().is_empty());
    assert_eq!(phase_err.category(), ErrorCategory::PhaseExecution);

    // Test ClaudeError
    let claude_err = ClaudeError::NotFound;
    assert!(!claude_err.user_message().is_empty());
    assert!(claude_err.context().is_some());
    assert!(!claude_err.suggestions().is_empty());
    assert_eq!(claude_err.category(), ErrorCategory::ClaudeIntegration);

    // Test RunnerError
    let runner_err = RunnerError::DetectionFailed {
        reason: "Claude not found".to_string(),
    };
    assert!(!runner_err.user_message().is_empty());
    assert!(runner_err.context().is_some());
    assert!(!runner_err.suggestions().is_empty());
    assert_eq!(runner_err.category(), ErrorCategory::ClaudeIntegration);

    // Test SourceError
    let source_err = SourceError::EmptyInput;
    assert!(!source_err.user_message().is_empty());
    assert!(source_err.context().is_some());
    assert!(!source_err.suggestions().is_empty());
    assert_eq!(source_err.category(), ErrorCategory::Configuration);

    // Test FixupError
    let fixup_err = FixupError::NoFixupMarkersFound;
    assert!(!fixup_err.user_message().is_empty());
    assert!(fixup_err.context().is_some());
    assert!(!fixup_err.suggestions().is_empty());
    assert_eq!(fixup_err.category(), ErrorCategory::Validation);

    // Test SpecIdError
    let spec_id_err = SpecIdError::Empty;
    assert!(!spec_id_err.user_message().is_empty());
    assert!(spec_id_err.context().is_some());
    assert!(!spec_id_err.suggestions().is_empty());
    assert_eq!(spec_id_err.category(), ErrorCategory::Validation);

    // Test LockError
    let lock_err = LockError::ConcurrentExecution {
        spec_id: "test-spec".to_string(),
        pid: 1234,
        created_ago: "5 minutes".to_string(),
    };
    assert!(!lock_err.user_message().is_empty());
    assert!(lock_err.context().is_some());
    assert!(!lock_err.suggestions().is_empty());
    assert_eq!(lock_err.category(), ErrorCategory::Concurrency);
}

#[test]
fn test_error_messages_are_actionable() {
    // Test that suggestions contain actionable guidance
    let config_err = ConfigError::InvalidValue {
        key: "source".to_string(),
        value: "invalid".to_string(),
    };
    let suggestions = config_err.suggestions();
    assert!(
        suggestions
            .iter()
            .any(|s| s.contains("--source") || s.contains("source"))
    );

    let phase_err = PhaseError::DependencyNotSatisfied {
        phase: "DESIGN".to_string(),
        dependency: "REQUIREMENTS".to_string(),
    };
    let suggestions = phase_err.suggestions();
    assert!(suggestions.iter().any(|s| s.contains("xchecker")));

    let fixup_err = FixupError::SymlinkNotAllowed(PathBuf::from("test.txt"));
    let suggestions = fixup_err.suggestions();
    assert!(suggestions.iter().any(|s| s.contains("--allow-links")));
}

#[test]
fn test_error_context_provides_explanation() {
    // Test that context explains the error category
    let config_err = ConfigError::DiscoveryFailed {
        reason: "permission denied".to_string(),
    };
    let context = config_err.context();
    assert!(context.is_some());
    assert!(context.unwrap().contains("discovery"));

    let fixup_err = FixupError::AbsolutePath(PathBuf::from("/absolute/path"));
    let context = fixup_err.context();
    assert!(context.is_some());
    assert!(context.unwrap().contains("directory traversal"));

    let spec_id_err = SpecIdError::OnlyInvalidCharacters;
    let context = spec_id_err.context();
    assert!(context.is_some());
    assert!(context.unwrap().contains("filesystem"));
}

#[test]
fn test_xchecker_error_delegates_to_inner_errors() {
    // Test that XCheckerError properly delegates to inner error types
    let config_err = ConfigError::MissingRequired("model".to_string());
    let xchecker_err = XCheckerError::Config(config_err);

    assert!(!xchecker_err.user_message().is_empty());
    assert!(xchecker_err.context().is_some());
    assert!(!xchecker_err.suggestions().is_empty());
    assert_eq!(xchecker_err.category(), ErrorCategory::Configuration);

    let fixup_err = FixupError::NoFixupMarkersFound;
    let xchecker_err = XCheckerError::Fixup(fixup_err);

    assert!(!xchecker_err.user_message().is_empty());
    assert!(xchecker_err.context().is_some());
    assert!(!xchecker_err.suggestions().is_empty());
    assert_eq!(xchecker_err.category(), ErrorCategory::Validation);

    let spec_id_err = SpecIdError::Empty;
    let xchecker_err = XCheckerError::SpecId(spec_id_err);

    assert!(!xchecker_err.user_message().is_empty());
    assert!(xchecker_err.context().is_some());
    assert!(!xchecker_err.suggestions().is_empty());
    assert_eq!(xchecker_err.category(), ErrorCategory::Validation);
}

#[test]
fn test_security_errors_have_appropriate_guidance() {
    // Test that security-related errors provide appropriate guidance
    let fixup_err = FixupError::SymlinkNotAllowed(PathBuf::from("test.txt"));
    assert_eq!(fixup_err.category(), ErrorCategory::Security);
    let suggestions = fixup_err.suggestions();
    assert!(suggestions.iter().any(|s| s.contains("security")));

    let fixup_err = FixupError::ParentDirEscape(PathBuf::from("../escape"));
    assert_eq!(fixup_err.category(), ErrorCategory::Security);
    let suggestions = fixup_err.suggestions();
    assert!(suggestions.iter().any(|s| s.contains("..")));

    let xchecker_err = XCheckerError::SecretDetected {
        pattern: "github_pat".to_string(),
        location: "test.txt".to_string(),
    };
    assert_eq!(xchecker_err.category(), ErrorCategory::Security);
    let suggestions = xchecker_err.suggestions();
    assert!(
        suggestions
            .iter()
            .any(|s| s.contains("--ignore-secret-pattern"))
    );
}

#[test]
fn test_phase_errors_include_phase_specific_guidance() {
    // Test that phase errors provide phase-specific suggestions
    let requirements_err = PhaseError::ExecutionFailed {
        phase: "REQUIREMENTS".to_string(),
        code: 1,
    };
    let suggestions = requirements_err.suggestions();
    assert!(
        suggestions
            .iter()
            .any(|s| s.contains("problem statement") || s.contains("requirements"))
    );

    let design_err = PhaseError::ExecutionFailed {
        phase: "DESIGN".to_string(),
        code: 1,
    };
    let suggestions = design_err.suggestions();
    assert!(
        suggestions
            .iter()
            .any(|s| s.contains("requirements") || s.contains("design"))
    );

    let tasks_err = PhaseError::ExecutionFailed {
        phase: "TASKS".to_string(),
        code: 1,
    };
    let suggestions = tasks_err.suggestions();
    assert!(
        suggestions
            .iter()
            .any(|s| s.contains("design") || s.contains("tasks"))
    );
}

#[test]
fn test_file_system_errors_suggest_permission_checks() {
    // Test that file system errors suggest checking permissions
    let fixup_err = FixupError::TargetFileNotFound {
        path: "missing.txt".to_string(),
    };
    assert_eq!(fixup_err.category(), ErrorCategory::FileSystem);
    let suggestions = fixup_err.suggestions();
    assert!(
        suggestions
            .iter()
            .any(|s| s.contains("Verify") || s.contains("Check"))
    );

    let fixup_err = FixupError::TempCopyFailed {
        file: "test.txt".to_string(),
        reason: "permission denied".to_string(),
    };
    assert_eq!(fixup_err.category(), ErrorCategory::FileSystem);
    let suggestions = fixup_err.suggestions();
    assert!(
        suggestions
            .iter()
            .any(|s| s.contains("permission") || s.contains("disk space"))
    );
}

#[test]
fn test_validation_errors_explain_format_requirements() {
    // Test that validation errors explain what's expected
    let fixup_err = FixupError::InvalidDiffFormat {
        block_index: 1,
        reason: "missing header".to_string(),
    };
    assert_eq!(fixup_err.category(), ErrorCategory::Validation);
    let context = fixup_err.context();
    assert!(context.is_some());
    assert!(context.unwrap().contains("unified diff"));

    let spec_id_err = SpecIdError::OnlyInvalidCharacters;
    assert_eq!(spec_id_err.category(), ErrorCategory::Validation);
    let suggestions = spec_id_err.suggestions();
    assert!(suggestions.iter().any(|s| s.contains("alphanumeric")));
}

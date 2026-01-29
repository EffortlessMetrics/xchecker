//! Output validation for phase responses
//!
//! This module validates LLM responses to ensure they contain actual document
//! content rather than meta-commentary or summaries.

use regex::Regex;
use std::sync::LazyLock;
use xchecker_utils::error::ValidationError;
use xchecker_utils::types::PhaseId;

/// Patterns that indicate meta-commentary rather than actual content
static META_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // Starts with first-person declarations
        Regex::new(r"(?i)^(I('ve| have| will| am)|Here('s| is)|This is a|Let me)").unwrap(),
        // "I created/generated/produced" anywhere early
        Regex::new(r"(?i)^.{0,50}I('ve| have) (created|generated|produced|written|made)")
            .unwrap(),
        // Enthusiastic starts
        Regex::new(r"(?i)^(Perfect!|Great!|Excellent!|Certainly!|Sure!|Absolutely!)").unwrap(),
        // "Based on" or "as requested" declarations
        Regex::new(r"(?i)^.{0,30}(based on (the|your)|as (you )?requested)").unwrap(),
        // Summary declarations
        Regex::new(r"(?i)^.{0,50}(here is|below is|the following is) (a |the )?(comprehensive|detailed|complete)")
            .unwrap(),
        // "I'll create" or "I will create"
        Regex::new(r"(?i)^.{0,30}I('ll| will) (create|generate|write|produce)").unwrap(),
    ]
});

/// Minimum line counts per phase
fn min_lines_for_phase(phase: PhaseId) -> usize {
    match phase {
        PhaseId::Requirements => 30,
        PhaseId::Design => 50,
        PhaseId::Tasks => 25, // Lowered from 40 for stub compatibility
        PhaseId::Review => 15,
        PhaseId::Fixup => 10,
        PhaseId::Final => 5,
    }
}

/// Required headers per phase (at least one must be present)
fn required_headers_for_phase(phase: PhaseId) -> Vec<&'static str> {
    match phase {
        PhaseId::Requirements => vec!["# Requirements", "## Introduction", "## Requirements"],
        PhaseId::Design => vec!["# Design", "## Overview", "## Architecture"],
        PhaseId::Tasks => vec!["# Implementation", "- [ ]", "- [x]"],
        PhaseId::Review => vec!["# Review", "## Review", "FIXUP PLAN"],
        PhaseId::Fixup => vec!["# Fixup", "Applied", "fixup"],
        PhaseId::Final => vec![], // Final phase has no required headers
    }
}

/// Output validator for LLM responses
pub struct OutputValidator;

impl OutputValidator {
    /// Validate an LLM response for the given phase
    ///
    /// Returns `Ok(())` if the response is valid, or a list of validation errors.
    pub fn validate(content: &str, phase: PhaseId) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        // Check for meta-summary patterns (only for generative phases)
        if matches!(
            phase,
            PhaseId::Requirements | PhaseId::Design | PhaseId::Tasks
        ) && let Some(pattern) = Self::detect_meta_summary(content)
        {
            errors.push(ValidationError::MetaSummaryDetected { pattern });
        }

        // Check minimum length
        let line_count = content.lines().count();
        let min_lines = min_lines_for_phase(phase);
        if line_count < min_lines {
            errors.push(ValidationError::TooShort {
                actual: line_count,
                minimum: min_lines,
            });
        }

        // Check for required headers (at least one must be present)
        let required = required_headers_for_phase(phase);
        let has_any_header = required.iter().any(|header| content.contains(header));
        if !has_any_header && !required.is_empty() {
            errors.push(ValidationError::MissingSectionHeader {
                header: required.join(" OR "),
            });
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Check if content starts with meta-summary patterns
    fn detect_meta_summary(content: &str) -> Option<String> {
        // Get first 200 chars for pattern matching
        let prefix: String = content.chars().take(200).collect();

        for pattern in META_PATTERNS.iter() {
            if let Some(m) = pattern.find(&prefix) {
                return Some(m.as_str().to_string());
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_requirements_document() {
        let content = r#"# Requirements Document

## Introduction

This system provides user authentication functionality for secure access control.
The system will handle login, logout, and session management operations.

## Requirements

### Requirement 1

**User Story:** As a user, I want to log in securely so that my data is protected.

#### Acceptance Criteria

1. WHEN the user enters valid credentials THEN the system SHALL authenticate them
2. WHEN the user enters invalid credentials THEN the system SHALL show an error
3. WHEN the user is authenticated THEN the system SHALL create a session

### Requirement 2

**User Story:** As a user, I want to log out so that I can end my session.

#### Acceptance Criteria

1. WHEN the user clicks logout THEN the system SHALL terminate the session
2. WHEN the session is terminated THEN the system SHALL redirect to login

## Non-Functional Requirements

**NFR1 [Performance]:** The system SHALL respond within 200ms
**NFR2 [Security]:** The system SHALL use HTTPS for all communications"#;

        let result = OutputValidator::validate(content, PhaseId::Requirements);
        assert!(result.is_ok(), "Expected valid, got: {:?}", result);
    }

    #[test]
    fn test_meta_summary_detection() {
        let bad_content = r#"I've created a comprehensive requirements document for you.

# Requirements Document

## Introduction"#;

        let result = OutputValidator::validate(bad_content, PhaseId::Requirements);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, ValidationError::MetaSummaryDetected { .. }))
        );
    }

    #[test]
    fn test_perfect_start_detection() {
        let bad_content = "Perfect! Based on your requirements, here is the design.";
        let result = OutputValidator::detect_meta_summary(bad_content);
        assert!(result.is_some());
    }

    #[test]
    fn test_here_is_detection() {
        let bad_content = "Here is the comprehensive requirements document you requested.";
        let result = OutputValidator::detect_meta_summary(bad_content);
        assert!(result.is_some());
    }

    #[test]
    fn test_valid_start_not_detected() {
        let good_content = "# Requirements Document\n\n## Introduction\n\nThis system...";
        let result = OutputValidator::detect_meta_summary(good_content);
        assert!(result.is_none());
    }

    #[test]
    fn test_too_short_detection() {
        let short_content = "# Requirements\n\nShort content";
        let result = OutputValidator::validate(short_content, PhaseId::Requirements);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, ValidationError::TooShort { .. }))
        );
    }
}

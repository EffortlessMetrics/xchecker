//! Source resolution for different input types
//!
//! This module handles resolving different source types (GitHub, filesystem, stdin)
//! and provides structured error reporting for resolution failures.

use crate::error::{ErrorCategory, UserFriendlyError};
use std::path::PathBuf;
use thiserror::Error;

/// Source resolution errors (R6.4)
#[derive(Error, Debug)]
pub enum SourceError {
    #[error("GitHub source resolution failed: {reason}")]
    GitHubResolutionFailed { reason: String },

    #[error("Filesystem source not found: {path}")]
    FileSystemNotFound { path: String },

    #[error("Stdin source is empty or invalid")]
    StdinInvalid,

    #[error("Invalid source configuration: {reason}")]
    InvalidConfiguration { reason: String },
}

impl UserFriendlyError for SourceError {
    fn user_message(&self) -> String {
        match self {
            Self::GitHubResolutionFailed { reason } => {
                format!("Could not resolve GitHub source: {reason}")
            }
            Self::FileSystemNotFound { path } => {
                format!("Source file or directory not found: {path}")
            }
            Self::StdinInvalid => "No input provided via stdin or input is invalid".to_string(),
            Self::InvalidConfiguration { reason } => {
                format!("Source configuration is invalid: {reason}")
            }
        }
    }

    fn context(&self) -> Option<String> {
        match self {
            Self::GitHubResolutionFailed { reason: _ } => {
                Some("GitHub source resolution requires valid repository access and issue numbers.".to_string())
            }
            Self::FileSystemNotFound { path: _ } => {
                Some("Filesystem sources must point to existing files or directories within the project.".to_string())
            }
            Self::StdinInvalid => {
                Some("Stdin input should contain a clear problem statement or specification.".to_string())
            }
            Self::InvalidConfiguration { reason: _ } => {
                Some("Source configuration must specify valid source type and required parameters.".to_string())
            }
        }
    }

    fn suggestions(&self) -> Vec<String> {
        match self {
            Self::GitHubResolutionFailed { reason } => {
                let mut suggestions = vec![
                    "Verify the repository owner and name are correct".to_string(),
                    "Check that the issue number exists and is accessible".to_string(),
                    "Ensure you have read access to the repository".to_string(),
                ];

                if reason.contains("authentication") {
                    suggestions.push("Check your GitHub authentication credentials".to_string());
                } else if reason.contains("not found") {
                    suggestions.push("Verify the repository exists and is public or accessible".to_string());
                } else if reason.contains("rate limit") {
                    suggestions.push("Wait for GitHub API rate limit to reset".to_string());
                }

                suggestions
            }
            Self::FileSystemNotFound { path } => vec![
                format!("Check that the path '{}' exists", path),
                "Use an absolute path or path relative to current directory".to_string(),
                "Verify file permissions allow read access".to_string(),
                "Use 'ls' or 'dir' to list available files".to_string(),
            ],
            Self::StdinInvalid => vec![
                "Pipe input to xchecker: echo 'problem statement' | xchecker spec <id> --source stdin".to_string(),
                "Provide a clear problem description in the input".to_string(),
                "Use a different source type if stdin is not appropriate".to_string(),
            ],
            Self::InvalidConfiguration { reason: _ } => vec![
                "Use --source gh --gh owner/repo for GitHub sources".to_string(),
                "Use --source fs --repo <path> for filesystem sources".to_string(),
                "Use --source stdin for stdin input".to_string(),
                "Check the documentation for valid source configuration options".to_string(),
            ],
        }
    }

    fn category(&self) -> ErrorCategory {
        ErrorCategory::Configuration
    }
}

/// Source types supported by xchecker
/// Reserved for future multi-source spec ingestion (GitHub issues, filesystem, stdin)
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum SourceType {
    GitHub { owner: String, repo: String },
    FileSystem { path: PathBuf },
    Stdin,
}

/// Resolved source content
/// Reserved for future multi-source spec ingestion
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SourceContent {
    pub source_type: SourceType,
    pub content: String,
    pub metadata: std::collections::HashMap<String, String>,
}

/// Source resolver for different input types
pub struct SourceResolver;

impl SourceResolver {
    /// Resolve a GitHub source
    pub fn resolve_github(
        owner: &str,
        repo: &str,
        issue_id: &str,
    ) -> Result<SourceContent, SourceError> {
        // Simulate GitHub API resolution
        if owner.is_empty() || repo.is_empty() {
            return Err(SourceError::InvalidConfiguration {
                reason: "GitHub owner and repo must be specified".to_string(),
            });
        }

        if issue_id.parse::<u32>().is_err() {
            return Err(SourceError::GitHubResolutionFailed {
                reason: "Issue ID must be a valid number".to_string(),
            });
        }

        // For now, return a simulated response
        // In a real implementation, this would make GitHub API calls
        let content = format!(
            "GitHub issue #{issue_id} from {owner}/{repo}\n\nThis is a simulated issue description that would be fetched from the GitHub API."
        );

        let mut metadata = std::collections::HashMap::new();
        metadata.insert("owner".to_string(), owner.to_string());
        metadata.insert("repo".to_string(), repo.to_string());
        metadata.insert("issue_id".to_string(), issue_id.to_string());

        Ok(SourceContent {
            source_type: SourceType::GitHub {
                owner: owner.to_string(),
                repo: repo.to_string(),
            },
            content,
            metadata,
        })
    }

    /// Resolve a filesystem source
    pub fn resolve_filesystem(path: &PathBuf) -> Result<SourceContent, SourceError> {
        if !path.exists() {
            return Err(SourceError::FileSystemNotFound {
                path: path.display().to_string(),
            });
        }

        let content = if path.is_file() {
            std::fs::read_to_string(path).map_err(|_| SourceError::FileSystemNotFound {
                path: path.display().to_string(),
            })?
        } else if path.is_dir() {
            format!(
                "Directory source: {}\n\nThis would contain a summary of the directory contents and relevant files.",
                path.display()
            )
        } else {
            return Err(SourceError::FileSystemNotFound {
                path: path.display().to_string(),
            });
        };

        let mut metadata = std::collections::HashMap::new();
        metadata.insert("path".to_string(), path.display().to_string());
        metadata.insert(
            "type".to_string(),
            if path.is_file() { "file" } else { "directory" }.to_string(),
        );

        Ok(SourceContent {
            source_type: SourceType::FileSystem { path: path.clone() },
            content,
            metadata,
        })
    }

    /// Resolve stdin source
    pub fn resolve_stdin() -> Result<SourceContent, SourceError> {
        use std::io::Read;

        let mut buffer = String::new();
        std::io::stdin()
            .read_to_string(&mut buffer)
            .map_err(|_| SourceError::StdinInvalid)?;

        if buffer.trim().is_empty() {
            return Err(SourceError::StdinInvalid);
        }

        let mut metadata = std::collections::HashMap::new();
        metadata.insert("length".to_string(), buffer.len().to_string());

        Ok(SourceContent {
            source_type: SourceType::Stdin,
            content: buffer,
            metadata,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_github_source_resolution() {
        let result = SourceResolver::resolve_github("owner", "repo", "123");
        assert!(result.is_ok());

        let content = result.unwrap();
        assert!(content.content.contains("GitHub issue #123"));
        assert_eq!(content.metadata.get("owner"), Some(&"owner".to_string()));
    }

    #[test]
    fn test_github_source_invalid_issue() {
        let result = SourceResolver::resolve_github("owner", "repo", "invalid");
        assert!(result.is_err());

        if let Err(SourceError::GitHubResolutionFailed { reason }) = result {
            assert!(reason.contains("valid number"));
        } else {
            panic!("Expected GitHubResolutionFailed error");
        }
    }

    #[test]
    fn test_filesystem_source_not_found() {
        let path = PathBuf::from("/nonexistent/path");
        let result = SourceResolver::resolve_filesystem(&path);
        assert!(result.is_err());

        if let Err(SourceError::FileSystemNotFound { path: error_path }) = result {
            assert!(error_path.contains("nonexistent"));
        } else {
            panic!("Expected FileSystemNotFound error");
        }
    }

    #[test]
    fn test_source_error_user_friendly_messages() {
        let error = SourceError::GitHubResolutionFailed {
            reason: "authentication failed".to_string(),
        };

        assert!(
            error
                .user_message()
                .contains("Could not resolve GitHub source")
        );
        assert!(!error.suggestions().is_empty());
        assert!(
            error
                .suggestions()
                .iter()
                .any(|s| s.contains("authentication"))
        );
    }
}

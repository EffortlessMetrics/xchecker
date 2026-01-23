//! Source resolution for different input types
//!
//! This module handles resolving different source types (GitHub, filesystem, stdin)
//! and provides structured error reporting for resolution failures.

pub use crate::error::SourceError;
use std::path::PathBuf;

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
            return Err(SourceError::InvalidFormat {
                reason: "GitHub owner and repo must be specified".to_string(),
            });
        }

        if issue_id.parse::<u32>().is_err() {
            return Err(SourceError::InvalidFormat {
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
            .map_err(|e| SourceError::StdinReadFailed {
                reason: e.to_string(),
            })?;

        if buffer.trim().is_empty() {
            return Err(SourceError::EmptyInput);
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
    use crate::error::UserFriendlyError;

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

        if let Err(SourceError::InvalidFormat { reason }) = result {
            assert!(reason.contains("valid number"));
        } else {
            panic!("Expected InvalidFormat error");
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
        let error = SourceError::GitHubAuthFailed {
            reason: "authentication failed".to_string(),
        };

        assert!(error.user_message().contains("authentication failed"));
        assert!(!error.suggestions().is_empty());
        assert!(error.suggestions().iter().any(|s| s.contains("auth")));
    }
}

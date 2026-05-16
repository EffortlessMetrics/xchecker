use super::UserFriendlyError;
use crate::error::{ErrorCategory, SourceError};

impl UserFriendlyError for SourceError {
    fn user_message(&self) -> String {
        match self {
            Self::GitHubRepoNotFound { owner, repo } => {
                format!("GitHub repository '{owner}/{repo}' could not be found or accessed")
            }
            Self::GitHubIssueNotFound { owner, repo, issue } => {
                format!("Issue #{issue} not found in repository '{owner}/{repo}'")
            }
            Self::GitHubAuthFailed { reason } => {
                format!("GitHub authentication failed: {reason}")
            }
            Self::GitHubApiError { status, message } => {
                format!("GitHub API returned error {status}: {message}")
            }
            Self::FileSystemNotFound { path } => {
                format!("Path '{path}' does not exist")
            }
            Self::FileSystemAccessDenied { path } => {
                format!("Access denied to path '{path}'")
            }
            Self::FileSystemNotDirectory { path } => {
                format!("Path '{path}' is not a directory")
            }
            Self::StdinReadFailed { reason } => {
                format!("Failed to read from standard input: {reason}")
            }
            Self::EmptyInput => {
                "No input provided - please provide a problem statement".to_string()
            }
            Self::InvalidFormat { reason } => {
                format!("Input format is invalid: {reason}")
            }
        }
    }

    fn context(&self) -> Option<String> {
        match self {
            Self::GitHubRepoNotFound { .. } => {
                Some("GitHub source resolution requires access to public repositories or proper authentication for private ones.".to_string())
            }
            Self::GitHubIssueNotFound { .. } => {
                Some("GitHub issues are resolved by their number within the specified repository.".to_string())
            }
            Self::GitHubAuthFailed { .. } => {
                Some("GitHub authentication is required for private repositories and API rate limiting.".to_string())
            }
            Self::GitHubApiError { .. } => {
                Some("GitHub API errors can be temporary or indicate rate limiting, authentication, or permission issues.".to_string())
            }
            Self::FileSystemNotFound { .. } => {
                Some("Filesystem source resolution requires the specified path to exist and be accessible.".to_string())
            }
            Self::FileSystemAccessDenied { .. } => {
                Some("File system permissions must allow read access to the specified directory.".to_string())
            }
            Self::FileSystemNotDirectory { .. } => {
                Some("Filesystem source resolution expects a directory containing project files.".to_string())
            }
            Self::StdinReadFailed { .. } => {
                Some("Standard input is used when no other source is specified or when --source stdin is used.".to_string())
            }
            Self::EmptyInput => {
                Some("xchecker requires a problem statement to generate specifications from.".to_string())
            }
            Self::InvalidFormat { .. } => {
                Some("Input should be a clear problem statement describing what you want to build.".to_string())
            }
        }
    }

    fn suggestions(&self) -> Vec<String> {
        match self {
            Self::GitHubRepoNotFound { owner, repo } => vec![
                format!(
                    "Verify the repository name: https://github.com/{}/{}",
                    owner, repo
                ),
                "Check that the repository is public or you have access".to_string(),
                "Ensure your GitHub authentication is working".to_string(),
                "Try using the full repository URL format".to_string(),
            ],
            Self::GitHubIssueNotFound { owner, repo, issue } => vec![
                format!(
                    "Check if issue #{} exists: https://github.com/{}/{}/issues/{}",
                    issue, owner, repo, issue
                ),
                "Verify the issue number is correct".to_string(),
                "Ensure the issue is not closed or private".to_string(),
                "Try using a different issue number".to_string(),
            ],
            Self::GitHubAuthFailed { .. } => vec![
                "Set up GitHub authentication: gh auth login".to_string(),
                "Check your GitHub token permissions".to_string(),
                "Verify your GitHub CLI is properly configured".to_string(),
                "Try accessing a public repository first".to_string(),
            ],
            Self::GitHubApiError { status, .. } => match *status {
                401 => vec![
                    "Authentication required - run 'gh auth login'".to_string(),
                    "Check your GitHub token is valid and not expired".to_string(),
                ],
                403 => vec![
                    "Rate limit exceeded - wait a few minutes and try again".to_string(),
                    "Authenticate to get higher rate limits".to_string(),
                ],
                404 => vec![
                    "Repository or resource not found - check the URL".to_string(),
                    "Verify you have access to the repository".to_string(),
                ],
                _ => vec![
                    "This may be a temporary GitHub API issue - try again later".to_string(),
                    "Check GitHub status: https://www.githubstatus.com/".to_string(),
                ],
            },
            Self::FileSystemNotFound { path } => vec![
                format!("Create the directory: mkdir -p '{}'", path),
                "Check the path spelling and case sensitivity".to_string(),
                "Use an absolute path to avoid confusion".to_string(),
                "Verify you're in the correct working directory".to_string(),
            ],
            Self::FileSystemAccessDenied { path } => vec![
                format!("Check permissions: ls -la '{}'", path),
                "Ensure you have read access to the directory".to_string(),
                "Try running from a directory you own".to_string(),
                "Use sudo if appropriate (be careful with permissions)".to_string(),
            ],
            Self::FileSystemNotDirectory { path } => vec![
                format!("Use the parent directory of '{}'", path),
                "Specify a directory path, not a file path".to_string(),
                "Check that the path points to a directory".to_string(),
            ],
            Self::StdinReadFailed { .. } => vec![
                "Provide input via pipe: echo 'problem statement' | xchecker spec <id>".to_string(),
                "Use a different source: --source fs --repo /path/to/project".to_string(),
                "Check that stdin is not closed or redirected incorrectly".to_string(),
            ],
            Self::EmptyInput => vec![
                "Provide a problem statement via stdin".to_string(),
                "Use --source fs --repo <path> to read from filesystem".to_string(),
                "Use --source gh --gh owner/repo to read from GitHub issue".to_string(),
                "Example: echo 'Build a web API for user management' | xchecker spec my-api"
                    .to_string(),
            ],
            Self::InvalidFormat { .. } => vec![
                "Provide a clear, single-line problem statement".to_string(),
                "Describe what you want to build in plain English".to_string(),
                "Example: 'Build a REST API for managing user accounts'".to_string(),
                "Avoid complex formatting or multiple unrelated requests".to_string(),
            ],
        }
    }

    fn category(&self) -> ErrorCategory {
        ErrorCategory::Configuration
    }
}

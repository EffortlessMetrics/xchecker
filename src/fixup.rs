//! Fixup detection and parsing system with preview/apply modes
//!
//! This module implements the fixup system that detects "FIXUP PLAN:" markers in review output,
//! parses unified diff blocks, and provides preview/apply modes for safe fixup application.
//!
//! # Security
//!
//! All path operations use the sandboxed path types from `crate::paths` to prevent:
//! - Directory traversal attacks via `..` components
//! - Absolute path escapes
//! - Symlink-based escapes (configurable)
//! - Hardlink-based escapes (configurable)
//!
//! The `FixupParser` uses `SandboxRoot` to validate all target paths before any file operations.
//! This ensures that diff application cannot escape the workspace root.

use anyhow::Result;
use camino::Utf8Path;
use regex::Regex;
use std::collections::HashMap;
use std::path::PathBuf;
use tempfile::TempDir;
use thiserror::Error;

use crate::atomic_write::write_file_atomic;
use crate::error::{ErrorCategory, UserFriendlyError};
use crate::paths::{SandboxConfig, SandboxError, SandboxPath, SandboxRoot};
use crate::runner::CommandSpec;

/// Errors that can occur during fixup detection and parsing
#[derive(Error, Debug)]
pub enum FixupError {
    #[error("No fixup markers found in review output")]
    NoFixupMarkersFound,

    #[error("Invalid diff format in block {block_index}: {reason}")]
    InvalidDiffFormat { block_index: usize, reason: String },

    #[error("Git apply validation failed for {target_file}: {reason}")]
    GitApplyValidationFailed { target_file: String, reason: String },

    #[error("Git apply execution failed for {target_file}: {reason}")]
    GitApplyExecutionFailed { target_file: String, reason: String },

    #[error("Target file not found: {path}")]
    TargetFileNotFound { path: String },

    #[error("Failed to create temporary copy of {file}: {reason}")]
    TempCopyFailed { file: String, reason: String },

    #[error("Diff parsing failed: {reason}")]
    DiffParsingFailed { reason: String },

    #[error("No valid diff blocks found")]
    NoValidDiffBlocks,

    #[error("Absolute path not allowed: {0}")]
    AbsolutePath(PathBuf),

    #[error("Parent directory escape not allowed: {0}")]
    ParentDirEscape(PathBuf),

    #[error("Path resolves outside repo root: {0}")]
    OutsideRepo(PathBuf),

    #[error("Path canonicalization failed: {0}")]
    CanonicalizationError(String),

    #[error("Symlink not allowed (use --allow-links to permit): {0}")]
    SymlinkNotAllowed(PathBuf),

    #[error("Hardlink not allowed (use --allow-links to permit): {0}")]
    HardlinkNotAllowed(PathBuf),

    #[error(
        "Could not find matching context for hunk at line {expected_line} in {file} (searched ±{search_window} lines)"
    )]
    FuzzyMatchFailed {
        file: String,
        expected_line: usize,
        search_window: usize,
    },
}

impl UserFriendlyError for FixupError {
    fn user_message(&self) -> String {
        match self {
            Self::NoFixupMarkersFound => {
                "No fixup changes were found in the review output".to_string()
            }
            Self::InvalidDiffFormat {
                block_index,
                reason,
            } => {
                format!("Diff block {block_index} has invalid format: {reason}")
            }
            Self::GitApplyValidationFailed {
                target_file,
                reason,
            } => {
                format!("Cannot apply changes to '{target_file}': {reason}")
            }
            Self::GitApplyExecutionFailed {
                target_file,
                reason,
            } => {
                format!("Failed to apply changes to '{target_file}': {reason}")
            }
            Self::TargetFileNotFound { path } => {
                format!("Target file '{path}' does not exist")
            }
            Self::TempCopyFailed { file, reason } => {
                format!("Could not create temporary copy of '{file}': {reason}")
            }
            Self::DiffParsingFailed { reason } => {
                format!("Could not parse diff content: {reason}")
            }
            Self::NoValidDiffBlocks => "No valid diff blocks found in the fixup plan".to_string(),
            Self::AbsolutePath(path) => {
                format!("Absolute paths are not allowed: {}", path.display())
            }
            Self::ParentDirEscape(path) => {
                format!(
                    "Path attempts to escape parent directory: {}",
                    path.display()
                )
            }
            Self::OutsideRepo(path) => {
                format!("Path resolves outside repository root: {}", path.display())
            }
            Self::CanonicalizationError(reason) => {
                format!("Could not resolve file path: {reason}")
            }
            Self::SymlinkNotAllowed(path) => {
                format!(
                    "Symlinks are not allowed: {} (use --allow-links to permit)",
                    path.display()
                )
            }
            Self::HardlinkNotAllowed(path) => {
                format!(
                    "Hardlinks are not allowed: {} (use --allow-links to permit)",
                    path.display()
                )
            }
            Self::FuzzyMatchFailed {
                file,
                expected_line,
                search_window,
            } => {
                format!(
                    "Could not find matching context for diff hunk at line {} in '{}' (searched ±{} lines)",
                    expected_line, file, search_window
                )
            }
        }
    }

    fn context(&self) -> Option<String> {
        match self {
            Self::NoFixupMarkersFound => {
                Some("The review phase should produce a 'FIXUP PLAN:' section with unified diffs for files that need changes.".to_string())
            }
            Self::InvalidDiffFormat { .. } => {
                Some("Fixup diffs must follow the unified diff format with proper headers and hunks.".to_string())
            }
            Self::GitApplyValidationFailed { .. } | Self::GitApplyExecutionFailed { .. } => {
                Some("Git apply is used to safely apply diff patches to files with validation.".to_string())
            }
            Self::TargetFileNotFound { .. } => {
                Some("Fixup targets must exist in the repository before changes can be applied.".to_string())
            }
            Self::TempCopyFailed { .. } => {
                Some("Temporary copies are created to safely test changes before applying them.".to_string())
            }
            Self::DiffParsingFailed { .. } | Self::NoValidDiffBlocks => {
                Some("Fixup plans contain unified diff blocks that describe file changes.".to_string())
            }
            Self::AbsolutePath(_) | Self::ParentDirEscape(_) | Self::OutsideRepo(_) => {
                Some("Fixup paths are validated to prevent directory traversal and ensure changes stay within the repository.".to_string())
            }
            Self::CanonicalizationError(_) => {
                Some("Path canonicalization resolves symlinks and relative paths to absolute paths for validation.".to_string())
            }
            Self::SymlinkNotAllowed(_) | Self::HardlinkNotAllowed(_) => {
                Some("Symlinks and hardlinks are blocked by default for security. Use --allow-links to permit them.".to_string())
            }
            Self::FuzzyMatchFailed { .. } => {
                Some("The diff hunk's context lines couldn't be matched to the file, which may indicate the file has changed since the diff was generated.".to_string())
            }
        }
    }

    fn suggestions(&self) -> Vec<String> {
        match self {
            Self::NoFixupMarkersFound => vec![
                "Check the review output in .xchecker/specs/<id>/artifacts/review.md".to_string(),
                "Ensure the review phase completed successfully".to_string(),
                "The review phase may not have identified any changes needed".to_string(),
                "Try running the review phase again if it failed".to_string(),
            ],
            Self::InvalidDiffFormat {
                block_index,
                reason,
            } => vec![
                format!("Review diff block {} in the review output", block_index),
                "Ensure the diff follows unified diff format (--- and +++ headers)".to_string(),
                "Check that hunk headers use @@ format".to_string(),
                format!("Specific issue: {}", reason),
            ],
            Self::GitApplyValidationFailed {
                target_file,
                reason,
            } => vec![
                format!("Check the current state of '{}'", target_file),
                "The file may have been modified since the review phase".to_string(),
                "Try running the review phase again to generate fresh diffs".to_string(),
                format!("Git apply error: {}", reason),
                "Use --dry-run to preview changes without applying them".to_string(),
            ],
            Self::GitApplyExecutionFailed {
                target_file,
                reason,
            } => vec![
                format!("Check file permissions for '{}'", target_file),
                "Ensure the file is writable".to_string(),
                "Check available disk space".to_string(),
                format!("Git apply error: {}", reason),
                "Try running with --verbose for more details".to_string(),
            ],
            Self::TargetFileNotFound { path } => vec![
                format!("Verify that '{}' exists in the repository", path),
                "The file may have been deleted or moved since the review phase".to_string(),
                "Check the file path is correct and relative to the repository root".to_string(),
                "Run the review phase again to generate fresh fixup plans".to_string(),
            ],
            Self::TempCopyFailed { file, reason } => vec![
                "Check available disk space for temporary files".to_string(),
                "Ensure you have write permissions in the temp directory".to_string(),
                format!("File: {}", file),
                format!("Reason: {}", reason),
            ],
            Self::DiffParsingFailed { reason } => vec![
                "Check the review output format".to_string(),
                "Ensure the FIXUP PLAN section contains valid unified diffs".to_string(),
                format!("Parsing error: {}", reason),
                "Try running the review phase again".to_string(),
            ],
            Self::NoValidDiffBlocks => vec![
                "Check the review output for FIXUP PLAN section".to_string(),
                "Ensure diff blocks follow unified diff format".to_string(),
                "The review phase may not have generated any valid changes".to_string(),
                "Try running the review phase again".to_string(),
            ],
            Self::AbsolutePath(path) => vec![
                format!("Use relative paths instead of absolute: {}", path.display()),
                "Fixup paths must be relative to the repository root".to_string(),
                "Remove leading '/' or drive letters from paths".to_string(),
            ],
            Self::ParentDirEscape(path) => vec![
                format!("Remove '..' components from path: {}", path.display()),
                "Fixup paths must not escape the repository directory".to_string(),
                "Use paths relative to the repository root".to_string(),
            ],
            Self::OutsideRepo(path) => vec![
                format!("Path resolves outside repository: {}", path.display()),
                "Ensure all fixup targets are within the repository".to_string(),
                "Check for symlinks that point outside the repository".to_string(),
                "Use --allow-links if you need to modify symlinked files".to_string(),
            ],
            Self::CanonicalizationError(reason) => vec![
                "Check that the file path exists and is accessible".to_string(),
                "Verify file permissions allow reading the path".to_string(),
                format!("Error: {}", reason),
            ],
            Self::SymlinkNotAllowed(path) => vec![
                format!("Symlink detected: {}", path.display()),
                "Use --allow-links flag to permit symlink modifications".to_string(),
                "Consider modifying the symlink target directly instead".to_string(),
                "Symlinks are blocked by default for security".to_string(),
            ],
            Self::HardlinkNotAllowed(path) => vec![
                format!("Hardlink detected: {}", path.display()),
                "Use --allow-links flag to permit hardlink modifications".to_string(),
                "Consider modifying one of the linked files directly".to_string(),
                "Hardlinks are blocked by default for security".to_string(),
            ],
            Self::FuzzyMatchFailed { file, .. } => vec![
                format!(
                    "The file '{}' may have changed since the review phase",
                    file
                ),
                "Run the review phase again to generate fresh diffs".to_string(),
                "Check if the file has been modified by another process".to_string(),
                "Use 'xchecker resume <id> --phase review' to regenerate fixups".to_string(),
            ],
        }
    }

    fn category(&self) -> ErrorCategory {
        match self {
            Self::NoFixupMarkersFound | Self::NoValidDiffBlocks => ErrorCategory::Validation,
            Self::InvalidDiffFormat { .. } | Self::DiffParsingFailed { .. } => {
                ErrorCategory::Validation
            }
            Self::AbsolutePath(_) | Self::ParentDirEscape(_) | Self::OutsideRepo(_) => {
                ErrorCategory::Security
            }
            Self::SymlinkNotAllowed(_) | Self::HardlinkNotAllowed(_) => ErrorCategory::Security,
            Self::TargetFileNotFound { .. } | Self::TempCopyFailed { .. } => {
                ErrorCategory::FileSystem
            }
            Self::CanonicalizationError(_) => ErrorCategory::FileSystem,
            Self::GitApplyValidationFailed { .. } | Self::GitApplyExecutionFailed { .. } => {
                ErrorCategory::PhaseExecution
            }
            Self::FuzzyMatchFailed { .. } => ErrorCategory::PhaseExecution,
        }
    }
}

/// Mode for fixup operations - preview or apply
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixupMode {
    /// Preview mode: parse & validate unified diffs, list targets, no writes
    Preview,
    /// Apply mode: run git apply --check first, then apply changes
    Apply,
}

impl FixupMode {
    #[must_use]
    #[allow(dead_code)] // String conversion for serialization/display
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Preview => "preview",
            Self::Apply => "apply",
        }
    }
}

/// Represents a unified diff for a single file
#[derive(Debug, Clone)]
pub struct UnifiedDiff {
    /// Target file path relative to project root
    pub target_file: String,
    /// The complete diff content including headers
    pub diff_content: String,
    /// Individual hunks in the diff
    pub hunks: Vec<DiffHunk>,
}

/// Represents a single hunk within a unified diff
#[derive(Debug, Clone)]
pub struct DiffHunk {
    /// Original file line range (start, count)
    pub old_range: (usize, usize),
    /// New file line range (start, count)
    #[allow(dead_code)] // Diff analysis metadata
    pub new_range: (usize, usize),
    /// The hunk content including context and changes
    pub content: String,
}

/// Result of fixup preview operation
#[derive(Debug, Clone)]
pub struct FixupPreview {
    /// List of target files that would be modified
    pub target_files: Vec<String>,
    /// Summary of changes per file
    pub change_summary: HashMap<String, ChangeSummary>,
    /// Warnings encountered during parsing/validation
    pub warnings: Vec<String>,
    /// Whether all diffs passed validation
    pub all_valid: bool,
}

/// Summary of changes for a single file
#[derive(Debug, Clone)]
pub struct ChangeSummary {
    /// Number of hunks in the diff
    #[allow(dead_code)] // Diff reporting metadata
    pub hunk_count: usize,
    /// Estimated lines added
    pub lines_added: usize,
    /// Estimated lines removed
    pub lines_removed: usize,
    /// Whether the diff passed git apply --check validation
    pub validation_passed: bool,
    /// Validation warnings or errors
    #[allow(dead_code)] // Future diff reporting
    pub validation_messages: Vec<String>,
}

/// Result of fixup apply operation
#[derive(Debug, Clone)]
pub struct FixupResult {
    /// Files that were successfully modified with their details
    pub applied_files: Vec<AppliedFile>,
    /// Files that failed to apply
    pub failed_files: Vec<String>,
    /// Warnings encountered during application
    pub warnings: Vec<String>,
    /// Whether 3-way merge was used for any files
    pub three_way_used: bool,
}

/// Details of a successfully applied file
#[derive(Debug, Clone)]
pub struct AppliedFile {
    /// Path to the file that was modified
    pub path: String,
    /// BLAKE3 hash (first 8 chars) of the new content
    pub blake3_first8: String,
    /// Whether the file was actually applied (always true for `AppliedFile`)
    pub applied: bool,
    /// Warnings specific to this file (e.g., permission preservation issues)
    pub warnings: Vec<String>,
}

/// Main fixup parser that handles detection and parsing of fixup plans
///
/// # Security
///
/// `FixupParser` uses `SandboxRoot` to validate all target paths before any file operations.
/// This ensures that:
/// - Paths cannot escape the workspace root via `..` traversal
/// - Absolute paths are rejected
/// - Symlinks are rejected by default (configurable via `SandboxConfig`)
/// - Hardlinks are rejected by default (configurable via `SandboxConfig`)
///
/// All path validation happens through `SandboxRoot::join()` which provides
/// comprehensive security checks before any file I/O.
pub struct FixupParser {
    /// Operating mode (preview or apply)
    pub mode: FixupMode,
    /// Sandboxed root directory for resolving and validating relative paths
    sandbox_root: SandboxRoot,
}

impl FixupParser {
    /// Create a new fixup parser with a sandboxed root directory.
    ///
    /// # Arguments
    ///
    /// * `mode` - The operating mode (preview or apply)
    /// * `base_dir` - The base directory to use as the sandbox root
    ///
    /// # Errors
    ///
    /// Returns an error if the base directory cannot be used as a sandbox root
    /// (e.g., doesn't exist, isn't a directory, or can't be canonicalized).
    pub fn new(mode: FixupMode, base_dir: PathBuf) -> Result<Self, FixupError> {
        let sandbox_root = SandboxRoot::new(&base_dir, SandboxConfig::default())
            .map_err(|e| FixupError::CanonicalizationError(format!("Failed to create sandbox root: {e}")))?;
        Ok(Self { mode, sandbox_root })
    }

    /// Create a new fixup parser with custom sandbox configuration.
    ///
    /// # Arguments
    ///
    /// * `mode` - The operating mode (preview or apply)
    /// * `base_dir` - The base directory to use as the sandbox root
    /// * `config` - Custom sandbox configuration (e.g., to allow symlinks)
    ///
    /// # Errors
    ///
    /// Returns an error if the base directory cannot be used as a sandbox root.
    pub fn with_config(mode: FixupMode, base_dir: PathBuf, config: SandboxConfig) -> Result<Self, FixupError> {
        let sandbox_root = SandboxRoot::new(&base_dir, config)
            .map_err(|e| FixupError::CanonicalizationError(format!("Failed to create sandbox root: {e}")))?;
        Ok(Self { mode, sandbox_root })
    }

    /// Get the sandbox root path.
    #[must_use]
    pub fn base_dir(&self) -> &std::path::Path {
        self.sandbox_root.as_path()
    }

    /// Validate and resolve a target path within the sandbox.
    ///
    /// This method uses `SandboxRoot::join()` to validate the path, ensuring:
    /// - The path is relative (not absolute)
    /// - The path doesn't contain `..` traversal components
    /// - The path doesn't escape the sandbox root
    /// - The path isn't a symlink (unless configured to allow)
    /// - The path isn't a hardlink (unless configured to allow)
    ///
    /// # Arguments
    ///
    /// * `target_file` - The relative path to validate
    ///
    /// # Returns
    ///
    /// A `SandboxPath` that is guaranteed to be within the sandbox root.
    fn validate_target_path(&self, target_file: &str) -> Result<SandboxPath, FixupError> {
        self.sandbox_root.join(target_file).map_err(|e| match e {
            SandboxError::AbsolutePath { path } => FixupError::AbsolutePath(PathBuf::from(path)),
            SandboxError::ParentTraversal { path } => FixupError::ParentDirEscape(PathBuf::from(path)),
            SandboxError::EscapeAttempt { path, .. } => FixupError::OutsideRepo(PathBuf::from(path)),
            SandboxError::SymlinkNotAllowed { path } => FixupError::SymlinkNotAllowed(PathBuf::from(path)),
            SandboxError::HardlinkNotAllowed { path } => FixupError::HardlinkNotAllowed(PathBuf::from(path)),
            SandboxError::RootNotFound { path } | SandboxError::RootNotDirectory { path } => {
                FixupError::CanonicalizationError(format!("Invalid sandbox root: {path}"))
            }
            SandboxError::RootCanonicalizationFailed { path, reason } |
            SandboxError::PathCanonicalizationFailed { path, reason } => {
                FixupError::CanonicalizationError(format!("Failed to canonicalize {path}: {reason}"))
            }
        })
    }

    /// Detect if the review output contains fixup markers
    #[must_use]
    pub fn has_fixup_markers(&self, content: &str) -> bool {
        self.detect_fixup_markers(content).is_some()
    }

    /// Detect fixup markers in review output
    /// Returns the content after the first marker if found
    #[must_use]
    pub fn detect_fixup_markers(&self, content: &str) -> Option<String> {
        // Look for "FIXUP PLAN:" or "needs fixups" markers
        let fixup_plan_regex = Regex::new(r"(?i)FIXUP PLAN:").unwrap();
        let needs_fixups_regex = Regex::new(r"(?i)needs fixups").unwrap();

        if let Some(mat) = fixup_plan_regex.find(content) {
            return Some(content[mat.end()..].to_string());
        }

        if let Some(mat) = needs_fixups_regex.find(content) {
            return Some(content[mat.end()..].to_string());
        }

        None
    }

    /// Parse unified diff blocks from fixup content
    pub fn parse_diffs(&self, content: &str) -> Result<Vec<UnifiedDiff>, FixupError> {
        let fixup_content = self
            .detect_fixup_markers(content)
            .ok_or(FixupError::NoFixupMarkersFound)?;

        let diffs = self.extract_diff_blocks(&fixup_content)?;

        if diffs.is_empty() {
            return Err(FixupError::NoValidDiffBlocks);
        }

        Ok(diffs)
    }

    /// Extract diff blocks from fenced code blocks
    fn extract_diff_blocks(&self, content: &str) -> Result<Vec<UnifiedDiff>, FixupError> {
        let mut diffs = Vec::new();

        // Regex to match fenced diff blocks: ```diff ... ```
        // Use (?s) flag to make . match newlines
        let diff_block_regex = Regex::new(r"(?s)```diff\n(.*?)\n```").unwrap();

        for (block_index, captures) in diff_block_regex.captures_iter(content).enumerate() {
            let diff_content = captures
                .get(1)
                .ok_or_else(|| FixupError::InvalidDiffFormat {
                    block_index,
                    reason: "No diff content found in block".to_string(),
                })?
                .as_str();

            match self.parse_unified_diff(diff_content, block_index) {
                Ok(diff) => diffs.push(diff),
                Err(e) => {
                    // Log the error but continue processing other blocks
                    eprintln!("Warning: Failed to parse diff block {block_index}: {e}");
                }
            }
        }

        Ok(diffs)
    }

    /// Parse a single unified diff block
    fn parse_unified_diff(
        &self,
        diff_content: &str,
        block_index: usize,
    ) -> Result<UnifiedDiff, FixupError> {
        let lines: Vec<&str> = diff_content.lines().collect();

        if lines.is_empty() {
            return Err(FixupError::InvalidDiffFormat {
                block_index,
                reason: "Empty diff block".to_string(),
            });
        }

        // Find the --- and +++ headers
        let mut old_file = None;
        let mut new_file = None;
        let mut header_end = 0;

        for (i, line) in lines.iter().enumerate() {
            if let Some(rest) = line.strip_prefix("--- ") {
                old_file = Some(rest.trim());
            } else if let Some(rest) = line.strip_prefix("+++ ") {
                new_file = Some(rest.trim());
                header_end = i + 1;
                break;
            }
        }

        let target_file = new_file
            .or(old_file)
            .ok_or_else(|| FixupError::InvalidDiffFormat {
                block_index,
                reason: "No --- or +++ headers found".to_string(),
            })?;

        // Remove a/ and b/ prefixes if present (common in git diffs)
        let target_file = if target_file.starts_with("a/") || target_file.starts_with("b/") {
            &target_file[2..]
        } else {
            target_file
        };

        // Parse hunks
        let hunks = self.parse_hunks(&lines[header_end..], block_index)?;

        Ok(UnifiedDiff {
            target_file: target_file.to_string(),
            diff_content: diff_content.to_string(),
            hunks,
        })
    }

    /// Parse hunks from diff lines
    fn parse_hunks(&self, lines: &[&str], block_index: usize) -> Result<Vec<DiffHunk>, FixupError> {
        let mut hunks = Vec::new();
        let mut current_hunk_lines = Vec::new();
        let mut current_hunk_header = None;

        // Regex to match hunk headers: @@ -old_start,old_count +new_start,new_count @@
        let hunk_header_regex = Regex::new(r"^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@").unwrap();

        for line in lines {
            if let Some(captures) = hunk_header_regex.captures(line) {
                // Save previous hunk if exists
                if let Some((old_range, new_range)) = current_hunk_header {
                    hunks.push(DiffHunk {
                        old_range,
                        new_range,
                        content: current_hunk_lines.join("\n"),
                    });
                }

                // Parse new hunk header
                let old_start: usize = captures.get(1).unwrap().as_str().parse().map_err(|_| {
                    FixupError::InvalidDiffFormat {
                        block_index,
                        reason: "Invalid old start line number".to_string(),
                    }
                })?;

                let old_count: usize = captures
                    .get(2)
                    .map_or(1, |m| m.as_str().parse().unwrap_or(1));

                let new_start: usize = captures.get(3).unwrap().as_str().parse().map_err(|_| {
                    FixupError::InvalidDiffFormat {
                        block_index,
                        reason: "Invalid new start line number".to_string(),
                    }
                })?;

                let new_count: usize = captures
                    .get(4)
                    .map_or(1, |m| m.as_str().parse().unwrap_or(1));

                current_hunk_header = Some(((old_start, old_count), (new_start, new_count)));
                current_hunk_lines = vec![(*line).to_string()];
            } else {
                // Add line to current hunk
                current_hunk_lines.push((*line).to_string());
            }
        }

        // Save last hunk if exists
        if let Some((old_range, new_range)) = current_hunk_header {
            hunks.push(DiffHunk {
                old_range,
                new_range,
                content: current_hunk_lines.join("\n"),
            });
        }

        Ok(hunks)
    }

    /// Preview changes without applying them
    ///
    /// Line endings are normalized before calculating diff statistics (FR-FIX-010)
    ///
    /// # Security
    ///
    /// All target paths are validated through `SandboxRoot::join()` to ensure:
    /// - Paths cannot escape the workspace root via `..` traversal
    /// - Absolute paths are rejected
    /// - Symlinks are rejected by default (configurable via `SandboxConfig`)
    /// - Hardlinks are rejected by default (configurable via `SandboxConfig`)
    pub fn preview_changes(&self, diffs: &[UnifiedDiff]) -> Result<FixupPreview, FixupError> {
        let mut target_files = Vec::new();
        let mut change_summary = HashMap::new();
        let mut warnings = Vec::new();
        let mut all_valid = true;

        for diff in diffs {
            target_files.push(diff.target_file.clone());

            let mut validation_messages = Vec::new();
            let mut validation_passed = true;

            // 1. Path validation using SandboxRoot::join() via validate_target_path
            // This replaces the legacy validate_fixup_target function with the
            // centralized sandbox validation that provides comprehensive security checks.
            if let Err(e) = self.validate_target_path(&diff.target_file) {
                validation_passed = false;
                all_valid = false;
                let msg = format!("Invalid target path: {e}");
                validation_messages.push(msg.clone());
                warnings.push(format!("{}: {}", diff.target_file, msg));
            }

            // 2. Validate diff against temp copy (only if path validation passed)
            if validation_passed {
                match self.validate_diff_with_git_apply(diff) {
                    Ok(messages) => {
                        validation_messages.extend(messages);
                    }
                    Err(e) => {
                        validation_passed = false;
                        all_valid = false;
                        warnings.push(format!("Validation failed for {}: {}", diff.target_file, e));
                        validation_messages.push(e.to_string());
                    }
                }
            }

            // Calculate change statistics (with line ending normalization - FR-FIX-010)
            let (lines_added, lines_removed) = self.calculate_change_stats(diff);

            change_summary.insert(
                diff.target_file.clone(),
                ChangeSummary {
                    hunk_count: diff.hunks.len(),
                    lines_added,
                    lines_removed,
                    validation_passed,
                    validation_messages,
                },
            );
        }

        Ok(FixupPreview {
            target_files,
            change_summary,
            warnings,
            all_valid,
        })
    }

    /// Apply changes to files using atomic writes (FR-FIX-005, FR-FIX-006, FR-FIX-008)
    ///
    /// This method implements the atomic write pattern:
    /// 1. Validate target path using SandboxPath (security check)
    /// 2. Write to .tmp file with fsync
    /// 3. Create .bak backup if file exists
    /// 4. Atomic rename with Windows retry
    /// 5. Preserve file permissions (Unix) or attributes (Windows)
    /// 6. Record warnings if permission preservation fails
    ///
    /// # Security
    ///
    /// All target paths are validated through `SandboxRoot::join()` to ensure:
    /// - Paths cannot escape the workspace root via `..` traversal
    /// - Absolute paths are rejected
    /// - Symlinks are rejected by default (configurable via `SandboxConfig`)
    /// - Hardlinks are rejected by default (configurable via `SandboxConfig`)
    pub fn apply_changes(&self, diffs: &[UnifiedDiff]) -> Result<FixupResult, FixupError> {
        if self.mode != FixupMode::Apply {
            return Err(FixupError::DiffParsingFailed {
                reason: "Cannot apply changes in preview mode".to_string(),
            });
        }

        let mut applied_files = Vec::new();
        let mut failed_files = Vec::new();
        let mut warnings = Vec::new();

        for diff in diffs {
            // Validate target path before applying using SandboxPath (hard error in apply mode)
            // This replaces the legacy validate_fixup_target function with the
            // centralized sandbox validation that provides comprehensive security checks.
            if let Err(e) = self.validate_target_path(&diff.target_file) {
                failed_files.push(diff.target_file.clone());
                warnings.push(format!(
                    "Path validation failed for {}: {}",
                    diff.target_file, e
                ));
                continue;
            }

            match self.apply_single_diff_atomic(diff) {
                Ok(applied_file) => {
                    // Collect file-specific warnings
                    for warning in &applied_file.warnings {
                        warnings.push(format!("{}: {}", diff.target_file, warning));
                    }
                    applied_files.push(applied_file);
                }
                Err(e) => {
                    failed_files.push(diff.target_file.clone());
                    warnings.push(format!("Failed to apply {}: {}", diff.target_file, e));
                }
            }
        }

        Ok(FixupResult {
            applied_files,
            failed_files,
            warnings,
            three_way_used: false, // Not using git apply anymore
        })
    }

    /// Apply a single diff using atomic writes with backup and permission preservation
    ///
    /// This method implements FR-FIX-005, FR-FIX-006, FR-FIX-007, and FR-FIX-008:
    /// - FR-FIX-005: Atomic write with temp file + fsync + rename
    /// - FR-FIX-006: Create .bak backup and preserve file permissions
    /// - FR-FIX-007: Cross-filesystem fallback (copy→fsync→replace)
    /// - FR-FIX-008: Record warnings for permission preservation failures
    ///
    /// # Security
    ///
    /// The target path is validated through `SandboxRoot::join()` before any file operations.
    fn apply_single_diff_atomic(&self, diff: &UnifiedDiff) -> Result<AppliedFile, FixupError> {
        use std::fs;

        // Validate and get the sandboxed target path
        // This ensures the path is within the sandbox root and passes all security checks
        let sandbox_path = self.validate_target_path(&diff.target_file)?;
        let target_path = sandbox_path.as_path();

        if !target_path.exists() {
            return Err(FixupError::TargetFileNotFound {
                path: diff.target_file.clone(),
            });
        }

        let mut file_warnings = Vec::new();

        // Read original content with CRLF tolerance (FR-FS-005)
        // Line endings will be normalized during diff application
        let original_content =
            fs::read_to_string(target_path).map_err(|e| FixupError::TempCopyFailed {
                file: diff.target_file.clone(),
                reason: format!("Failed to read original file: {e}"),
            })?;

        // Get original file permissions/attributes before modification
        let original_metadata =
            fs::metadata(target_path).map_err(|e| FixupError::TempCopyFailed {
                file: diff.target_file.clone(),
                reason: format!("Failed to get file metadata: {e}"),
            })?;

        #[cfg(unix)]
        let original_permissions = {
            use std::os::unix::fs::PermissionsExt;
            Some(original_metadata.permissions().mode())
        };

        #[cfg(windows)]
        let original_readonly = original_metadata.permissions().readonly();

        // Apply the diff to get new content
        let new_content = self.apply_diff_to_content(&original_content, diff)?;

        // Compute BLAKE3 hash of new content
        let blake3_hash = self.compute_blake3_hash(&new_content);
        let blake3_first8 = blake3_hash[..8].to_string();

        // Create .bak backup (FR-FIX-006)
        let backup_path = target_path.with_extension("bak");
        fs::copy(target_path, &backup_path).map_err(|e| FixupError::TempCopyFailed {
            file: diff.target_file.clone(),
            reason: format!("Failed to create .bak backup: {e}"),
        })?;

        // Convert PathBuf to Utf8Path for atomic_write
        let target_utf8_path =
            Utf8Path::from_path(target_path).ok_or_else(|| FixupError::TempCopyFailed {
                file: diff.target_file.clone(),
                reason: "Path contains invalid UTF-8".to_string(),
            })?;

        // Use centralized atomic write with cross-filesystem fallback (FR-FIX-005, FR-FIX-007)
        let write_result = write_file_atomic(target_utf8_path, &new_content).map_err(|e| {
            FixupError::TempCopyFailed {
                file: diff.target_file.clone(),
                reason: format!("Failed to write file atomically: {e}"),
            }
        })?;

        // Collect warnings from atomic write (FR-FIX-007, FR-FIX-008)
        file_warnings.extend(write_result.warnings);

        // Preserve file permissions (FR-FIX-006, FR-FIX-008)
        #[cfg(unix)]
        {
            if let Some(mode) = original_permissions {
                use std::os::unix::fs::PermissionsExt;
                let permissions = fs::Permissions::from_mode(mode);
                if let Err(e) = fs::set_permissions(target_path, permissions) {
                    file_warnings.push(format!("Failed to preserve file permissions: {}", e));
                }
            }
        }

        #[cfg(windows)]
        {
            if let Ok(metadata) = fs::metadata(target_path) {
                let mut permissions = metadata.permissions();
                permissions.set_readonly(original_readonly);
                if let Err(e) = fs::set_permissions(target_path, permissions) {
                    file_warnings.push(format!("Failed to preserve file attributes: {e}"));
                }
            }
        }

        Ok(AppliedFile {
            path: diff.target_file.clone(),
            blake3_first8,
            applied: true,
            warnings: file_warnings,
        })
    }

    /// Apply a diff to content string with fuzzy matching support
    ///
    /// Line endings are normalized to LF before applying the diff (FR-FIX-010).
    /// If the hunk's expected line position doesn't match, we search within a
    /// window (±FUZZY_SEARCH_WINDOW lines) to find the best matching context.
    fn apply_diff_to_content(
        &self,
        content: &str,
        diff: &UnifiedDiff,
    ) -> Result<String, FixupError> {
        const FUZZY_SEARCH_WINDOW: usize = 50;
        const MIN_CONTEXT_MATCH_RATIO: f64 = 0.7;

        // Normalize line endings before applying diff (FR-FIX-010, FR-FS-005)
        let normalized_content = normalize_line_endings_for_diff(content);
        let mut lines: Vec<String> = normalized_content
            .lines()
            .map(std::string::ToString::to_string)
            .collect();

        // Track cumulative offset from previous hunks' additions/deletions
        let mut cumulative_offset: i64 = 0;

        // Apply each hunk in order
        for hunk in &diff.hunks {
            let (old_start, _old_count) = hunk.old_range;
            let hunk_lines: Vec<&str> = hunk.content.lines().collect();

            // Extract OLD lines from hunk (context lines + removed lines, NOT added lines)
            // These represent what exists in the original file and should be used for matching
            let context_lines: Vec<&str> = hunk_lines
                .iter()
                .skip(1) // Skip @@ header
                .filter(|line| {
                    // Include context lines (starting with ' ') and removed lines (starting with '-')
                    // but NOT added lines (starting with '+') as those don't exist in the original file
                    line.starts_with(' ')
                        || line.starts_with('-')
                        || (!line.starts_with('+') && !line.starts_with("@@"))
                })
                .filter(|line| !line.starts_with("---")) // Exclude --- header
                .map(|line| {
                    // Strip the prefix character (' ' or '-')
                    if line.starts_with(' ') || line.starts_with('-') {
                        &line[1..]
                    } else {
                        *line
                    }
                })
                .collect();

            // Calculate expected position with cumulative offset
            let expected_pos = ((old_start as i64 - 1) + cumulative_offset).max(0) as usize;

            // Try exact match first, then fuzzy match if needed
            let actual_start = if self.context_matches_at(&lines, expected_pos, &context_lines) {
                expected_pos
            } else {
                // Fuzzy search within window
                match self.find_best_context_match(
                    &lines,
                    expected_pos,
                    &context_lines,
                    FUZZY_SEARCH_WINDOW,
                    MIN_CONTEXT_MATCH_RATIO,
                ) {
                    Some((pos, _confidence)) => {
                        tracing::warn!(
                            "Fuzzy match: hunk at line {} shifted to line {} in '{}'",
                            old_start,
                            pos + 1,
                            diff.target_file
                        );
                        pos
                    }
                    None => {
                        return Err(FixupError::FuzzyMatchFailed {
                            file: diff.target_file.clone(),
                            expected_line: old_start,
                            search_window: FUZZY_SEARCH_WINDOW,
                        });
                    }
                }
            };

            // Track additions and deletions for offset calculation
            let mut additions = 0i64;
            let mut deletions = 0i64;

            // Apply hunk at actual_start position
            let mut hunk_idx = 1; // Start after @@ line
            let mut file_idx = actual_start;

            while hunk_idx < hunk_lines.len() {
                let line = hunk_lines[hunk_idx];

                if line.starts_with('+') && !line.starts_with("+++") {
                    // Add line
                    let new_line = line[1..].to_string();
                    if file_idx <= lines.len() {
                        lines.insert(file_idx, new_line);
                    } else {
                        lines.push(new_line);
                    }
                    file_idx += 1;
                    additions += 1;
                } else if line.starts_with('-') && !line.starts_with("---") {
                    // Remove line
                    if file_idx < lines.len() {
                        lines.remove(file_idx);
                        deletions += 1;
                    }
                } else if line.starts_with(' ') {
                    // Context line - just advance
                    file_idx += 1;
                } else if !line.starts_with("@@") {
                    // Context line without leading space
                    file_idx += 1;
                }

                hunk_idx += 1;
            }

            // Update cumulative offset for subsequent hunks
            cumulative_offset += additions - deletions;
        }

        Ok(lines.join("\n") + "\n")
    }

    /// Check if context lines match at a specific position
    fn context_matches_at(&self, lines: &[String], pos: usize, context: &[&str]) -> bool {
        if context.is_empty() {
            return true; // No context to match
        }

        // Check if we have enough lines
        if pos >= lines.len() {
            return false;
        }

        // Try to match context lines starting at pos
        let mut matches = 0;
        for (i, ctx_line) in context.iter().enumerate() {
            let file_pos = pos + i;
            if file_pos >= lines.len() {
                break;
            }
            if self.lines_match(&lines[file_pos], ctx_line) {
                matches += 1;
            }
        }

        // Require all context lines to match for exact match
        matches == context.len()
    }

    /// Find the best matching position for context within a search window
    fn find_best_context_match(
        &self,
        lines: &[String],
        expected_pos: usize,
        context: &[&str],
        window: usize,
        min_ratio: f64,
    ) -> Option<(usize, f64)> {
        if context.is_empty() {
            return Some((expected_pos, 1.0));
        }

        let start = expected_pos.saturating_sub(window);
        let end = (expected_pos + window).min(lines.len());

        let mut best_match: Option<(usize, f64)> = None;

        for candidate in start..end {
            let score = self.context_match_score(lines, candidate, context);
            if score >= min_ratio && best_match.is_none_or(|(_, best_score)| score > best_score) {
                best_match = Some((candidate, score));
            }
        }

        best_match
    }

    /// Calculate match score for context at a position (0.0 to 1.0)
    fn context_match_score(&self, lines: &[String], pos: usize, context: &[&str]) -> f64 {
        if context.is_empty() {
            return 1.0;
        }

        let mut matches = 0;
        for (i, ctx_line) in context.iter().enumerate() {
            let file_pos = pos + i;
            if file_pos >= lines.len() {
                break;
            }
            if self.lines_match(&lines[file_pos], ctx_line) {
                matches += 1;
            }
        }

        (matches as f64) / (context.len() as f64)
    }

    /// Compare two lines with whitespace normalization
    fn lines_match(&self, file_line: &str, context_line: &str) -> bool {
        // Exact match
        if file_line == context_line {
            return true;
        }

        // Whitespace-normalized match (collapse multiple spaces, trim)
        let normalize = |s: &str| -> String { s.split_whitespace().collect::<Vec<_>>().join(" ") };

        normalize(file_line) == normalize(context_line)
    }

    /// Compute BLAKE3 hash of content
    fn compute_blake3_hash(&self, content: &str) -> String {
        use blake3;
        let hash = blake3::hash(content.as_bytes());
        hash.to_hex().to_string()
    }

    /// Apply changes to files (legacy git apply method - kept for compatibility)
    #[allow(dead_code)]
    pub fn apply_changes_with_git(&self, diffs: &[UnifiedDiff]) -> Result<FixupResult, FixupError> {
        if self.mode != FixupMode::Apply {
            return Err(FixupError::DiffParsingFailed {
                reason: "Cannot apply changes in preview mode".to_string(),
            });
        }

        let mut applied_files = Vec::new();
        let mut failed_files = Vec::new();
        let mut warnings = Vec::new();
        let mut three_way_used = false;

        for diff in diffs {
            match self.apply_single_diff(diff) {
                Ok(used_three_way) => {
                    applied_files.push(AppliedFile {
                        path: diff.target_file.clone(),
                        blake3_first8: "00000000".to_string(), // Not computed in git apply mode
                        applied: true,
                        warnings: Vec::new(),
                    });
                    if used_three_way {
                        three_way_used = true;
                        warnings.push(format!("Used 3-way merge for {}", diff.target_file));
                    }
                }
                Err(e) => {
                    failed_files.push(diff.target_file.clone());
                    warnings.push(format!("Failed to apply {}: {}", diff.target_file, e));
                }
            }
        }

        Ok(FixupResult {
            applied_files,
            failed_files,
            warnings,
            three_way_used,
        })
    }

    /// Validate a diff using git apply --check
    ///
    /// # Security
    ///
    /// The target path is validated through `SandboxRoot::join()` before any file operations.
    fn validate_diff_with_git_apply(&self, diff: &UnifiedDiff) -> Result<Vec<String>, FixupError> {
        // Validate and get the sandboxed target path
        let sandbox_path = self.validate_target_path(&diff.target_file)?;
        let target_path = sandbox_path.as_path();

        if !target_path.exists() {
            return Err(FixupError::TargetFileNotFound {
                path: diff.target_file.clone(),
            });
        }

        // Create temporary directory and copy target file
        let temp_dir = TempDir::new().map_err(|e| FixupError::TempCopyFailed {
            file: diff.target_file.clone(),
            reason: e.to_string(),
        })?;

        let temp_file = temp_dir.path().join("target_file");
        std::fs::copy(target_path, &temp_file).map_err(|e| FixupError::TempCopyFailed {
            file: diff.target_file.clone(),
            reason: e.to_string(),
        })?;

        // Write diff to temporary file
        let diff_file = temp_dir.path().join("changes.diff");
        std::fs::write(&diff_file, &diff.diff_content).map_err(|e| FixupError::TempCopyFailed {
            file: "diff".to_string(),
            reason: e.to_string(),
        })?;

        // Run git apply --check using CommandSpec for secure argv-style execution
        let output = CommandSpec::new("git")
            .args(["apply", "--check", "--verbose"])
            .arg(&diff_file)
            .cwd(temp_dir.path())
            .to_command()
            .output()
            .map_err(|e| FixupError::GitApplyValidationFailed {
                target_file: diff.target_file.clone(),
                reason: format!("Failed to run git apply: {e}"),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(FixupError::GitApplyValidationFailed {
                target_file: diff.target_file.clone(),
                reason: stderr.to_string(),
            });
        }

        // Return any warnings from stdout/stderr
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let mut messages = Vec::new();
        if !stdout.is_empty() {
            messages.push(stdout.to_string());
        }
        if !stderr.is_empty() {
            messages.push(stderr.to_string());
        }

        Ok(messages)
    }

    /// Apply a single diff to its target file
    ///
    /// # Security
    ///
    /// The target path is validated through `SandboxRoot::join()` before any file operations.
    fn apply_single_diff(&self, diff: &UnifiedDiff) -> Result<bool, FixupError> {
        // Validate and get the sandboxed target path
        let sandbox_path = self.validate_target_path(&diff.target_file)?;
        let target_path = sandbox_path.as_path();

        if !target_path.exists() {
            return Err(FixupError::TargetFileNotFound {
                path: diff.target_file.clone(),
            });
        }

        // Write diff to temporary file
        let temp_dir = TempDir::new().map_err(|e| FixupError::TempCopyFailed {
            file: diff.target_file.clone(),
            reason: e.to_string(),
        })?;

        let diff_file = temp_dir.path().join("changes.diff");
        std::fs::write(&diff_file, &diff.diff_content).map_err(|e| FixupError::TempCopyFailed {
            file: "diff".to_string(),
            reason: e.to_string(),
        })?;

        // First try git apply --check using CommandSpec for secure argv-style execution
        let check_output = CommandSpec::new("git")
            .args(["apply", "--check"])
            .arg(&diff_file)
            .cwd(self.base_dir())
            .to_command()
            .output()
            .map_err(|e| FixupError::GitApplyValidationFailed {
                target_file: diff.target_file.clone(),
                reason: format!("Failed to run git apply --check: {e}"),
            })?;

        if !check_output.status.success() {
            // Try with 3-way merge as last resort using CommandSpec
            let three_way_output = CommandSpec::new("git")
                .args(["apply", "--3way"])
                .arg(&diff_file)
                .cwd(self.base_dir())
                .to_command()
                .output()
                .map_err(|e| FixupError::GitApplyExecutionFailed {
                    target_file: diff.target_file.clone(),
                    reason: format!("Failed to run git apply --3way: {e}"),
                })?;

            if !three_way_output.status.success() {
                let stderr = String::from_utf8_lossy(&three_way_output.stderr);
                return Err(FixupError::GitApplyExecutionFailed {
                    target_file: diff.target_file.clone(),
                    reason: stderr.to_string(),
                });
            }

            return Ok(true); // Used 3-way merge
        }

        // Apply the diff normally using CommandSpec for secure argv-style execution
        let apply_output = CommandSpec::new("git")
            .args(["apply"])
            .arg(&diff_file)
            .cwd(self.base_dir())
            .to_command()
            .output()
            .map_err(|e| FixupError::GitApplyExecutionFailed {
                target_file: diff.target_file.clone(),
                reason: format!("Failed to run git apply: {e}"),
            })?;

        if !apply_output.status.success() {
            let stderr = String::from_utf8_lossy(&apply_output.stderr);
            return Err(FixupError::GitApplyExecutionFailed {
                target_file: diff.target_file.clone(),
                reason: stderr.to_string(),
            });
        }

        Ok(false) // Did not use 3-way merge
    }

    /// Calculate change statistics from a diff
    fn calculate_change_stats(&self, diff: &UnifiedDiff) -> (usize, usize) {
        let mut lines_added = 0;
        let mut lines_removed = 0;

        for hunk in &diff.hunks {
            for line in hunk.content.lines() {
                if line.starts_with('+') && !line.starts_with("+++") {
                    lines_added += 1;
                } else if line.starts_with('-') && !line.starts_with("---") {
                    lines_removed += 1;
                }
            }
        }

        (lines_added, lines_removed)
    }
}

/// Normalize line endings to LF (FR-FIX-010, FR-FS-004, FR-FS-005)
///
/// This function converts all line ending styles (CRLF, CR, LF) to LF.
/// This ensures consistent diff calculation regardless of the source file's
/// line ending style, which is especially important on Windows where files
/// may have CRLF line endings.
///
/// # Arguments
///
/// * `content` - The content to normalize
///
/// # Returns
///
/// A string with all line endings normalized to LF (\n)
///
/// # Examples
///
/// ```
/// use xchecker::fixup::normalize_line_endings_for_diff;
///
/// let crlf_content = "line1\r\nline2\r\n";
/// let normalized = normalize_line_endings_for_diff(crlf_content);
/// assert_eq!(normalized, "line1\nline2\n");
/// ```
#[must_use]
pub fn normalize_line_endings_for_diff(content: &str) -> String {
    content.replace("\r\n", "\n").replace('\r', "\n")
}

/// Validates that a fixup target path is safe to apply patches to.
///
/// This function ensures that:
/// - The path is not absolute
/// - The path does not contain parent directory (`..`) components
/// - The path is not a symlink (unless `allow_links` is true)
/// - The path is not a hardlink (unless `allow_links` is true)
/// - After symlink resolution, the path resolves within the repository root
///
/// On Windows, this function uses `dunce::canonicalize` for normalized
/// case-insensitive path comparison to handle Windows path semantics correctly.
///
/// # Arguments
///
/// * `path` - The target path to validate (relative to repo root)
/// * `repo_root` - The repository root directory
/// * `allow_links` - Whether to allow symlinks and hardlinks (default: false)
///
/// # Returns
///
/// Returns `Ok(())` if the path is valid, or a `FixupError` describing why it's invalid.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use xchecker::fixup::validate_fixup_target;
///
/// let repo_root = Path::new("/home/user/project");
/// let target = Path::new("src/main.rs");
///
/// // Valid path
/// assert!(validate_fixup_target(target, repo_root, false).is_ok());
///
/// // Invalid: absolute path
/// let absolute = Path::new("/etc/passwd");
/// assert!(validate_fixup_target(absolute, repo_root, false).is_err());
///
/// // Invalid: parent directory escape
/// let escape = Path::new("../../../etc/passwd");
/// assert!(validate_fixup_target(escape, repo_root, false).is_err());
/// ```
pub fn validate_fixup_target(
    path: &std::path::Path,
    repo_root: &std::path::Path,
    allow_links: bool,
) -> Result<(), FixupError> {
    // Reject absolute paths
    if path.is_absolute() {
        return Err(FixupError::AbsolutePath(path.to_path_buf()));
    }

    // Reject paths with parent directory components
    if path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(FixupError::ParentDirEscape(path.to_path_buf()));
    }

    // Construct the full path
    let full_path = repo_root.join(path);

    // Check for symlinks and hardlinks using lstat (unless allow_links is true)
    if !allow_links {
        // Use lstat to get metadata without following symlinks
        let metadata = full_path.symlink_metadata().map_err(|e| {
            FixupError::CanonicalizationError(format!("Failed to get file metadata: {e}"))
        })?;

        // Check if it's a symlink
        if metadata.is_symlink() {
            return Err(FixupError::SymlinkNotAllowed(path.to_path_buf()));
        }

        // Check if it's a hardlink (more than one hard link to the same inode)
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if metadata.nlink() > 1 {
                return Err(FixupError::HardlinkNotAllowed(path.to_path_buf()));
            }
        }

        #[cfg(windows)]
        {
            // On Windows, we use a different approach to detect hardlinks
            // We check if the file has multiple names by comparing the file index
            // This is a best-effort check; hardlinks are less common on Windows
            // For now, we'll use a workaround: try to get file metadata via different APIs
            // If the file exists and is not a symlink, we'll allow it unless we can prove it's a hardlink

            // Note: Windows hardlink detection is complex and requires Win32 API calls
            // For the MVP, we'll document this limitation and rely on the symlink check
            // A full implementation would use GetFileInformationByHandle to check nNumberOfLinks

            // TODO: Implement proper Windows hardlink detection using Win32 API
            // For now, we skip hardlink detection on Windows as it requires unsafe code
            // and Win32 API calls. This is documented as a known limitation.
        }
    }

    // Canonicalize both paths to resolve symlinks and get absolute paths
    let resolved = full_path.canonicalize().map_err(|e| {
        FixupError::CanonicalizationError(format!("Failed to canonicalize target path: {e}"))
    })?;

    let canonical_repo_root = repo_root.canonicalize().map_err(|e| {
        FixupError::CanonicalizationError(format!("Failed to canonicalize repo root: {e}"))
    })?;

    // On Windows, use dunce::canonicalize for normalized case-insensitive comparison
    #[cfg(target_os = "windows")]
    let (resolved, canonical_repo_root) = {
        let resolved = dunce::canonicalize(&resolved).map_err(|e| {
            FixupError::CanonicalizationError(format!("Failed to normalize Windows path: {e}"))
        })?;
        let canonical_repo_root = dunce::canonicalize(&canonical_repo_root).map_err(|e| {
            FixupError::CanonicalizationError(format!("Failed to normalize Windows repo root: {e}"))
        })?;
        (resolved, canonical_repo_root)
    };

    // Ensure the resolved path is within the repo root
    if !resolved.starts_with(&canonical_repo_root) {
        return Err(FixupError::OutsideRepo(resolved));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::UserFriendlyError;
    use std::fs;

    #[test]
    fn test_fixup_error_user_friendly() {
        // Test NoFixupMarkersFound error
        let no_markers_err = FixupError::NoFixupMarkersFound;
        assert!(!no_markers_err.user_message().is_empty());
        assert!(no_markers_err.context().is_some());
        assert!(!no_markers_err.suggestions().is_empty());

        // Test InvalidDiffFormat error
        let invalid_diff_err = FixupError::InvalidDiffFormat {
            block_index: 1,
            reason: "missing hunk header".to_string(),
        };
        assert!(invalid_diff_err.user_message().contains("block 1"));
        assert!(invalid_diff_err.context().is_some());
        assert!(!invalid_diff_err.suggestions().is_empty());

        // Test SymlinkNotAllowed error
        let symlink_err = FixupError::SymlinkNotAllowed(PathBuf::from("test/file.txt"));
        assert!(symlink_err.user_message().contains("--allow-links"));
        assert!(symlink_err.context().is_some());
        let suggestions = symlink_err.suggestions();
        assert!(suggestions.iter().any(|s| s.contains("--allow-links")));

        // Test AbsolutePath error
        let abs_path_err = FixupError::AbsolutePath(PathBuf::from("/absolute/path"));
        assert!(abs_path_err.user_message().contains("Absolute paths"));
        assert!(abs_path_err.context().is_some());
        assert!(!abs_path_err.suggestions().is_empty());
    }

    #[test]
    fn test_detect_fixup_markers() {
        let temp_dir = TempDir::new().unwrap();
        let parser = FixupParser::new(FixupMode::Preview, temp_dir.path().to_path_buf()).unwrap();

        // Test FIXUP PLAN: marker
        let content1 = "Some review content\nFIXUP PLAN:\nHere are the fixes needed...";
        assert!(parser.has_fixup_markers(content1));

        // Test needs fixups marker
        let content2 = "The review shows that this needs fixups in several areas...";
        assert!(parser.has_fixup_markers(content2));

        // Test no markers
        let content3 = "This is a clean review with no issues found.";
        assert!(!parser.has_fixup_markers(content3));
    }

    #[test]
    fn test_parse_simple_diff() {
        let temp_dir = TempDir::new().unwrap();
        let parser = FixupParser::new(FixupMode::Preview, temp_dir.path().to_path_buf()).unwrap();

        let content = r#"
FIXUP PLAN:
The following changes are needed:

```diff
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,4 @@
 fn main() {
+    println!("Hello, world!");
     // TODO: implement
 }
```
"#;

        let diffs = parser.parse_diffs(content).unwrap();
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].target_file, "src/main.rs");
        assert_eq!(diffs[0].hunks.len(), 1);
    }

    #[test]
    fn test_validate_fixup_target_rejects_absolute_paths() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();

        // Create a test file in the repo
        let test_file = repo_root.join("test.txt");
        fs::write(&test_file, "test content").unwrap();

        // Test absolute path rejection - use platform-appropriate absolute path
        #[cfg(unix)]
        let absolute_path = std::path::Path::new("/etc/passwd");

        #[cfg(windows)]
        let absolute_path = std::path::Path::new("C:\\Windows\\System32\\config\\sam");

        let result = validate_fixup_target(absolute_path, repo_root, false);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), FixupError::AbsolutePath(_)));
    }

    #[test]
    fn test_validate_fixup_target_rejects_parent_dir_escapes() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();

        // Create a test file in the repo
        let test_file = repo_root.join("test.txt");
        fs::write(&test_file, "test content").unwrap();

        // Test parent directory escape rejection
        let escape_path = std::path::Path::new("../../../etc/passwd");
        let result = validate_fixup_target(escape_path, repo_root, false);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            FixupError::ParentDirEscape(_)
        ));

        // Test another escape pattern
        let escape_path2 = std::path::Path::new("subdir/../../outside.txt");
        let result2 = validate_fixup_target(escape_path2, repo_root, false);
        assert!(result2.is_err());
        assert!(matches!(
            result2.unwrap_err(),
            FixupError::ParentDirEscape(_)
        ));
    }

    #[test]
    fn test_validate_fixup_target_accepts_valid_relative_paths() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();

        // Create test files in the repo
        let test_file = repo_root.join("test.txt");
        fs::write(&test_file, "test content").unwrap();

        let subdir = repo_root.join("subdir");
        fs::create_dir(&subdir).unwrap();
        let nested_file = subdir.join("nested.txt");
        fs::write(&nested_file, "nested content").unwrap();

        // Test valid relative paths
        let valid_path1 = std::path::Path::new("test.txt");
        assert!(validate_fixup_target(valid_path1, repo_root, false).is_ok());

        let valid_path2 = std::path::Path::new("subdir/nested.txt");
        assert!(validate_fixup_target(valid_path2, repo_root, false).is_ok());
    }

    #[test]
    fn test_validate_fixup_target_rejects_symlinks_by_default() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();

        // Create a regular file in the repo
        let target_file = repo_root.join("target.txt");
        fs::write(&target_file, "target content").unwrap();

        // Create a symlink inside the repo pointing to the target file
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let symlink_path = repo_root.join("link_to_target");
            symlink(&target_file, &symlink_path).unwrap();

            // Test that symlink is rejected by default
            let result =
                validate_fixup_target(std::path::Path::new("link_to_target"), repo_root, false);
            assert!(result.is_err());
            assert!(matches!(
                result.unwrap_err(),
                FixupError::SymlinkNotAllowed(_)
            ));
        }

        #[cfg(windows)]
        {
            use std::os::windows::fs::symlink_file;
            let symlink_path = repo_root.join("link_to_target");
            // Windows symlinks require admin privileges, so we skip if it fails
            if symlink_file(&target_file, &symlink_path).is_ok() {
                let result =
                    validate_fixup_target(std::path::Path::new("link_to_target"), repo_root, false);
                assert!(result.is_err());
                assert!(matches!(
                    result.unwrap_err(),
                    FixupError::SymlinkNotAllowed(_)
                ));
            }
        }
    }

    #[test]
    fn test_validate_fixup_target_allows_symlinks_with_flag() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();

        // Create a regular file in the repo
        let target_file = repo_root.join("target.txt");
        fs::write(&target_file, "target content").unwrap();

        // Create a symlink inside the repo pointing to the target file
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let symlink_path = repo_root.join("link_to_target");
            symlink(&target_file, &symlink_path).unwrap();

            // Test that symlink is allowed with allow_links=true
            let result =
                validate_fixup_target(std::path::Path::new("link_to_target"), repo_root, true);
            assert!(result.is_ok());
        }

        #[cfg(windows)]
        {
            use std::os::windows::fs::symlink_file;
            let symlink_path = repo_root.join("link_to_target");
            // Windows symlinks require admin privileges, so we skip if it fails
            if symlink_file(&target_file, &symlink_path).is_ok() {
                let result =
                    validate_fixup_target(std::path::Path::new("link_to_target"), repo_root, true);
                assert!(result.is_ok());
            }
        }
    }

    #[test]
    fn test_validate_fixup_target_rejects_hardlinks_by_default() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();

        // Create a regular file in the repo
        let target_file = repo_root.join("target.txt");
        fs::write(&target_file, "target content").unwrap();

        // Create a hardlink to the target file
        #[cfg(unix)]
        {
            let hardlink_path = repo_root.join("hardlink_to_target");
            std::fs::hard_link(&target_file, &hardlink_path).unwrap();

            // Test that hardlink is rejected by default
            let result =
                validate_fixup_target(std::path::Path::new("hardlink_to_target"), repo_root, false);
            assert!(result.is_err());
            assert!(matches!(
                result.unwrap_err(),
                FixupError::HardlinkNotAllowed(_)
            ));
        }

        #[cfg(windows)]
        {
            use std::fs::hard_link;
            let hardlink_path = repo_root.join("hardlink_to_target");
            // Try to create hardlink, skip if it fails (requires permissions)
            if hard_link(&target_file, &hardlink_path).is_ok() {
                // Note: Windows hardlink detection is not fully implemented yet
                // This test documents the expected behavior once implemented
                // For now, we skip this test on Windows
                println!("Skipping hardlink rejection test on Windows (not yet implemented)");
            }
        }
    }

    #[test]
    fn test_validate_fixup_target_allows_hardlinks_with_flag() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();

        // Create a regular file in the repo
        let target_file = repo_root.join("target.txt");
        fs::write(&target_file, "target content").unwrap();

        // Create a hardlink to the target file
        #[cfg(unix)]
        {
            let hardlink_path = repo_root.join("hardlink_to_target");
            std::fs::hard_link(&target_file, &hardlink_path).unwrap();

            // Test that hardlink is allowed with allow_links=true
            let result =
                validate_fixup_target(std::path::Path::new("hardlink_to_target"), repo_root, true);
            assert!(result.is_ok());
        }

        #[cfg(windows)]
        {
            use std::fs::hard_link;
            let hardlink_path = repo_root.join("hardlink_to_target");
            // Try to create hardlink, skip if it fails (requires permissions)
            if hard_link(&target_file, &hardlink_path).is_ok() {
                // Note: Windows hardlink detection is not fully implemented yet
                // This test documents the expected behavior once implemented
                // For now, we skip this test on Windows
                println!("Skipping hardlink allow test on Windows (not yet implemented)");
            }
        }
    }

    #[test]
    fn test_validate_fixup_target_symlink_escape() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();

        // Create a directory outside the repo
        let outside_dir = temp_dir.path().parent().unwrap().join("outside");
        fs::create_dir_all(&outside_dir).unwrap();
        let outside_file = outside_dir.join("secret.txt");
        fs::write(&outside_file, "secret content").unwrap();

        // Create a symlink inside the repo pointing outside
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let symlink_path = repo_root.join("escape_link");
            let _ = symlink(&outside_file, &symlink_path);

            // Test that symlink is rejected by default (before checking if it escapes)
            let result =
                validate_fixup_target(std::path::Path::new("escape_link"), repo_root, false);
            assert!(result.is_err());
            // Should fail with SymlinkNotAllowed before checking OutsideRepo
            assert!(matches!(
                result.unwrap_err(),
                FixupError::SymlinkNotAllowed(_)
            ));

            // Test that symlink escape is detected when allow_links=true
            let result_with_links =
                validate_fixup_target(std::path::Path::new("escape_link"), repo_root, true);
            assert!(result_with_links.is_err());
            assert!(matches!(
                result_with_links.unwrap_err(),
                FixupError::OutsideRepo(_)
            ));
        }

        #[cfg(windows)]
        {
            use std::os::windows::fs::symlink_file;
            let symlink_path = repo_root.join("escape_link");
            // Windows symlinks require admin privileges, so we skip if it fails
            if symlink_file(&outside_file, &symlink_path).is_ok() {
                // Test that symlink is rejected by default
                let result =
                    validate_fixup_target(std::path::Path::new("escape_link"), repo_root, false);
                assert!(result.is_err());
                assert!(matches!(
                    result.unwrap_err(),
                    FixupError::SymlinkNotAllowed(_)
                ));

                // Test that symlink escape is detected when allow_links=true
                let result_with_links =
                    validate_fixup_target(std::path::Path::new("escape_link"), repo_root, true);
                assert!(result_with_links.is_err());
                assert!(matches!(
                    result_with_links.unwrap_err(),
                    FixupError::OutsideRepo(_)
                ));
            }
        }
    }

    #[test]
    #[cfg(windows)]
    fn test_validate_fixup_target_windows_case_insensitive() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();

        // Create a test file
        let test_file = repo_root.join("Test.txt");
        fs::write(&test_file, "test content").unwrap();

        // Test that different case variations are accepted (Windows is case-insensitive)
        let lower_case = std::path::Path::new("test.txt");
        let result = validate_fixup_target(lower_case, repo_root, false);
        // This should succeed because Windows paths are case-insensitive
        assert!(result.is_ok());

        let upper_case = std::path::Path::new("TEST.TXT");
        let result2 = validate_fixup_target(upper_case, repo_root, false);
        assert!(result2.is_ok());
    }

    #[test]
    fn test_validate_fixup_target_nonexistent_file() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();

        // Test with a file that doesn't exist
        let nonexistent = std::path::Path::new("does_not_exist.txt");
        let result = validate_fixup_target(nonexistent, repo_root, false);

        // Should fail with canonicalization error since the file doesn't exist
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            FixupError::CanonicalizationError(_)
        ));
    }

    #[test]
    fn test_validate_fixup_target_with_dot_components() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();

        // Create a test file
        let test_file = repo_root.join("test.txt");
        fs::write(&test_file, "test content").unwrap();

        // Test that paths with . components are accepted (they don't escape)
        let dot_path = std::path::Path::new("./test.txt");
        let result = validate_fixup_target(dot_path, repo_root, false);
        assert!(result.is_ok());

        // Test nested . components
        let nested_dot = std::path::Path::new("./subdir/../test.txt");
        // This should fail because it contains .. component
        let result2 = validate_fixup_target(nested_dot, repo_root, false);
        assert!(result2.is_err());
    }

    #[test]
    fn test_parse_multiple_hunks() {
        let temp_dir = TempDir::new().unwrap();
        let parser = FixupParser::new(FixupMode::Preview, temp_dir.path().to_path_buf()).unwrap();

        let content = r#"
FIXUP PLAN:
Multiple changes needed:

```diff
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,3 +1,4 @@
 pub fn foo() {
+    println!("Starting foo");
     // implementation
 }
@@ -10,2 +11,3 @@
 pub fn bar() {
+    println!("Starting bar");
     // implementation
 }
```
"#;

        let diffs = parser.parse_diffs(content).unwrap();
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].target_file, "src/lib.rs");
        assert_eq!(diffs[0].hunks.len(), 2);

        // Verify first hunk
        let hunk1 = &diffs[0].hunks[0];
        assert_eq!(hunk1.old_range, (1, 3));
        assert_eq!(hunk1.new_range, (1, 4));

        // Verify second hunk
        let hunk2 = &diffs[0].hunks[1];
        assert_eq!(hunk2.old_range, (10, 2));
        assert_eq!(hunk2.new_range, (11, 3));
    }

    #[test]
    fn test_parse_multiple_diffs() {
        let temp_dir = TempDir::new().unwrap();
        let parser = FixupParser::new(FixupMode::Preview, temp_dir.path().to_path_buf()).unwrap();

        let content = r#"
FIXUP PLAN:
Changes needed in multiple files:

```diff
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,2 +1,3 @@
 fn main() {
+    println!("Hello");
 }
```

```diff
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,2 +1,3 @@
 pub fn test() {
+    println!("Test");
 }
```
"#;

        let diffs = parser.parse_diffs(content).unwrap();
        assert_eq!(diffs.len(), 2);
        assert_eq!(diffs[0].target_file, "src/main.rs");
        assert_eq!(diffs[1].target_file, "src/lib.rs");
    }

    #[test]
    fn test_calculate_change_stats() {
        let temp_dir = TempDir::new().unwrap();
        let parser = FixupParser::new(FixupMode::Preview, temp_dir.path().to_path_buf()).unwrap();

        let content = r#"
FIXUP PLAN:

```diff
--- a/src/test.rs
+++ b/src/test.rs
@@ -1,5 +1,6 @@
 fn test() {
+    let x = 1;
     let y = 2;
-    let z = 3;
+    let z = 4;
     println!("test");
 }
```
"#;

        let diffs = parser.parse_diffs(content).unwrap();
        assert_eq!(diffs.len(), 1);

        let (added, removed) = parser.calculate_change_stats(&diffs[0]);
        assert_eq!(added, 2); // +let x = 1; and +let z = 4;
        assert_eq!(removed, 1); // -let z = 3;
    }

    #[test]
    fn test_parse_diff_without_git_prefix() {
        let temp_dir = TempDir::new().unwrap();
        let parser = FixupParser::new(FixupMode::Preview, temp_dir.path().to_path_buf()).unwrap();

        let content = r#"
FIXUP PLAN:

```diff
--- src/main.rs
+++ src/main.rs
@@ -1,2 +1,3 @@
 fn main() {
+    println!("Hello");
 }
```
"#;

        let diffs = parser.parse_diffs(content).unwrap();
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].target_file, "src/main.rs");
    }

    #[test]
    fn test_parse_diff_with_git_prefix() {
        let temp_dir = TempDir::new().unwrap();
        let parser = FixupParser::new(FixupMode::Preview, temp_dir.path().to_path_buf()).unwrap();

        let content = r#"
FIXUP PLAN:

```diff
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,2 +1,3 @@
 fn main() {
+    println!("Hello");
 }
```
"#;

        let diffs = parser.parse_diffs(content).unwrap();
        assert_eq!(diffs.len(), 1);
        // Should strip a/ and b/ prefixes
        assert_eq!(diffs[0].target_file, "src/main.rs");
    }

    #[test]
    fn test_hunk_range_parsing() {
        let temp_dir = TempDir::new().unwrap();
        let parser = FixupParser::new(FixupMode::Preview, temp_dir.path().to_path_buf()).unwrap();

        let content = r"
FIXUP PLAN:

```diff
--- a/test.txt
+++ b/test.txt
@@ -5,3 +5,4 @@
 line 5
+new line
 line 6
 line 7
@@ -10 +11,2 @@
 line 10
+another new line
```
";

        let diffs = parser.parse_diffs(content).unwrap();
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].hunks.len(), 2);

        // First hunk: @@ -5,3 +5,4 @@
        let hunk1 = &diffs[0].hunks[0];
        assert_eq!(hunk1.old_range, (5, 3));
        assert_eq!(hunk1.new_range, (5, 4));

        // Second hunk: @@ -10 +11,2 @@ (implicit count of 1)
        let hunk2 = &diffs[0].hunks[1];
        assert_eq!(hunk2.old_range, (10, 1));
        assert_eq!(hunk2.new_range, (11, 2));
    }

    #[test]
    fn test_empty_diff_block() {
        let temp_dir = TempDir::new().unwrap();
        let parser = FixupParser::new(FixupMode::Preview, temp_dir.path().to_path_buf()).unwrap();

        let content = r"
FIXUP PLAN:

```diff
```
";

        let result = parser.parse_diffs(content);
        // Should fail with NoValidDiffBlocks since the diff block is empty
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), FixupError::NoValidDiffBlocks));
    }

    #[test]
    fn test_malformed_hunk_header() {
        let temp_dir = TempDir::new().unwrap();
        let parser = FixupParser::new(FixupMode::Preview, temp_dir.path().to_path_buf()).unwrap();

        let content = r"
FIXUP PLAN:

```diff
--- a/test.txt
+++ b/test.txt
@@ invalid hunk header @@
 some content
```
";

        let diffs = parser.parse_diffs(content).unwrap();
        // Parser should handle malformed hunks gracefully
        // The diff should be parsed but with 0 hunks since the header is invalid
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].hunks.len(), 0);
    }

    #[test]
    fn test_no_fixup_markers() {
        let temp_dir = TempDir::new().unwrap();
        let parser = FixupParser::new(FixupMode::Preview, temp_dir.path().to_path_buf()).unwrap();

        let content = "This is just regular review content without any fixup markers.";

        let result = parser.parse_diffs(content);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            FixupError::NoFixupMarkersFound
        ));
    }

    #[test]
    fn test_fixup_preview_structure() {
        let temp_dir = TempDir::new().unwrap();
        let parser = FixupParser::new(FixupMode::Preview, temp_dir.path().to_path_buf()).unwrap();

        // Create a simple diff
        let diff = UnifiedDiff {
            target_file: "test.txt".to_string(),
            diff_content: "--- a/test.txt\n+++ b/test.txt\n@@ -1,1 +1,2 @@\n line1\n+line2"
                .to_string(),
            hunks: vec![DiffHunk {
                old_range: (1, 1),
                new_range: (1, 2),
                content: "@@ -1,1 +1,2 @@\n line1\n+line2".to_string(),
            }],
        };

        // Calculate stats manually
        let (added, removed) = parser.calculate_change_stats(&diff);
        assert_eq!(added, 1);
        assert_eq!(removed, 0);

        // Verify ChangeSummary structure can be created
        let summary = ChangeSummary {
            hunk_count: diff.hunks.len(),
            lines_added: added,
            lines_removed: removed,
            validation_passed: true,
            validation_messages: vec![],
        };

        assert_eq!(summary.hunk_count, 1);
        assert_eq!(summary.lines_added, 1);
        assert_eq!(summary.lines_removed, 0);
        assert!(summary.validation_passed);
    }

    #[test]
    fn test_fixup_result_structure() {
        // Test that FixupResult can be created with all fields
        let result = FixupResult {
            applied_files: vec![
                AppliedFile {
                    path: "file1.txt".to_string(),
                    blake3_first8: "abcd1234".to_string(),
                    applied: true,
                    warnings: Vec::new(),
                },
                AppliedFile {
                    path: "file2.txt".to_string(),
                    blake3_first8: "efgh5678".to_string(),
                    applied: true,
                    warnings: vec!["Permission warning".to_string()],
                },
            ],
            failed_files: vec!["file3.txt".to_string()],
            warnings: vec!["Warning: file3.txt had conflicts".to_string()],
            three_way_used: false,
        };

        assert_eq!(result.applied_files.len(), 2);
        assert_eq!(result.failed_files.len(), 1);
        assert_eq!(result.warnings.len(), 1);
        assert!(!result.three_way_used);
        assert_eq!(result.applied_files[0].blake3_first8, "abcd1234");
        assert_eq!(result.applied_files[1].warnings.len(), 1);
    }

    #[test]
    fn test_unified_diff_structure() {
        // Test that UnifiedDiff can be created with all required fields
        let hunk = DiffHunk {
            old_range: (1, 3),
            new_range: (1, 4),
            content: "@@ -1,3 +1,4 @@\n line1\n+line2\n line3".to_string(),
        };

        let diff = UnifiedDiff {
            target_file: "src/main.rs".to_string(),
            diff_content:
                "--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,3 +1,4 @@\n line1\n+line2\n line3"
                    .to_string(),
            hunks: vec![hunk],
        };

        assert_eq!(diff.target_file, "src/main.rs");
        assert_eq!(diff.hunks.len(), 1);
        assert_eq!(diff.hunks[0].old_range, (1, 3));
        assert_eq!(diff.hunks[0].new_range, (1, 4));
    }

    #[test]
    fn test_case_insensitive_fixup_markers() {
        let temp_dir = TempDir::new().unwrap();
        let parser = FixupParser::new(FixupMode::Preview, temp_dir.path().to_path_buf()).unwrap();

        // Test various case variations
        let content1 = "fixup plan:\nSome content";
        assert!(parser.has_fixup_markers(content1));

        let content2 = "FIXUP PLAN:\nSome content";
        assert!(parser.has_fixup_markers(content2));

        let content3 = "Fixup Plan:\nSome content";
        assert!(parser.has_fixup_markers(content3));

        let content4 = "needs FIXUPS in several places";
        assert!(parser.has_fixup_markers(content4));
    }
}

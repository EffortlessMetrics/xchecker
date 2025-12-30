//! Packet construction system for token-efficient context management
//!
//! This module implements the packet building system that selects and organizes
//! content for Claude CLI invocations while respecting budget constraints and
//! maintaining evidence for auditability.

use crate::atomic_write::write_file_atomic;
use crate::cache::{InsightCache, calculate_content_hash};
use crate::config::Selectors;
use crate::error::XCheckerError;
use crate::logging::Logger;
use crate::phase::{BudgetUsage, Packet};
use crate::redaction::{SecretRedactor, create_secret_detected_error};
use crate::types::{FileEvidence, PacketEvidence, Priority};
use anyhow::{Context, Result};
use blake3::Hasher;
use camino::{Utf8Path, Utf8PathBuf};
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::fs;

/// Default maximum bytes allowed in a packet
pub const DEFAULT_PACKET_MAX_BYTES: usize = 65536;

/// Default maximum lines allowed in a packet
pub const DEFAULT_PACKET_MAX_LINES: usize = 1200;

/// Content selector that implements priority-based file selection
/// with concrete defaults and LIFO ordering within priority classes
#[derive(Debug, Clone)]
pub struct ContentSelector {
    /// Include patterns for file selection
    include_patterns: GlobSet,
    /// Exclude patterns for file filtering
    exclude_patterns: GlobSet,
    /// Priority rules for content selection
    priority_rules: PriorityRules,
}

/// Priority rules defining the selection order
/// Order: *.core.yaml (non-evictable) → SPEC/ADR/REPORT → README/SCHEMA → misc
/// LIFO within each priority class
#[derive(Debug, Clone)]
pub struct PriorityRules {
    /// High priority patterns (SPEC/ADR/REPORT)
    pub high: GlobSet,
    /// Medium priority patterns (README/SCHEMA)
    pub medium: GlobSet,
    /// Low priority patterns (misc files)
    #[allow(dead_code)] // Reserved for metadata tracking
    pub low: GlobSet,
}

impl Default for PriorityRules {
    fn default() -> Self {
        let mut high_builder = GlobSetBuilder::new();
        high_builder.add(Glob::new("**/SPEC*").unwrap());
        high_builder.add(Glob::new("**/ADR*").unwrap());
        high_builder.add(Glob::new("**/REPORT*").unwrap());
        high_builder.add(Glob::new("**/*SPEC*").unwrap());
        high_builder.add(Glob::new("**/*ADR*").unwrap());
        high_builder.add(Glob::new("**/*REPORT*").unwrap());
        // Problem statement files get high priority - critical context for LLM
        high_builder.add(Glob::new("**/problem-statement*").unwrap());
        high_builder.add(Glob::new("**/*problem-statement*").unwrap());

        let mut medium_builder = GlobSetBuilder::new();
        medium_builder.add(Glob::new("**/README*").unwrap());
        medium_builder.add(Glob::new("**/SCHEMA*").unwrap());
        medium_builder.add(Glob::new("**/*README*").unwrap());
        medium_builder.add(Glob::new("**/*SCHEMA*").unwrap());

        let mut low_builder = GlobSetBuilder::new();
        low_builder.add(Glob::new("**/*").unwrap()); // Catch-all for misc files

        Self {
            high: high_builder.build().unwrap(),
            medium: medium_builder.build().unwrap(),
            low: low_builder.build().unwrap(),
        }
    }
}

impl ContentSelector {
    /// Create a new `ContentSelector` with default patterns
    pub fn new() -> Result<Self> {
        let mut include_builder = GlobSetBuilder::new();
        // Default include patterns based on requirements
        include_builder.add(Glob::new("**/*.md")?);
        include_builder.add(Glob::new("**/*.yaml")?);
        include_builder.add(Glob::new("**/*.yml")?);
        include_builder.add(Glob::new("**/*.toml")?);
        include_builder.add(Glob::new("**/*.txt")?);
        include_builder.add(Glob::new("**/README*")?);
        include_builder.add(Glob::new("**/SPEC*")?);
        include_builder.add(Glob::new("**/ADR*")?);
        include_builder.add(Glob::new("**/REPORT*")?);
        include_builder.add(Glob::new("**/SCHEMA*")?);
        // Spec context directories - problem statements, notes
        include_builder.add(Glob::new("context/**/*.md")?);
        include_builder.add(Glob::new("source/**/*.md")?);

        let mut exclude_builder = GlobSetBuilder::new();
        // Default exclude patterns
        exclude_builder.add(Glob::new("**/target/**")?);
        exclude_builder.add(Glob::new("**/node_modules/**")?);
        exclude_builder.add(Glob::new("**/.git/**")?);
        // Note: .xchecker/** is excluded for repo-level searches,
        // but when building packets from spec_dir, we're already inside .xchecker/specs/<id>

        Ok(Self {
            include_patterns: include_builder.build()?,
            exclude_patterns: exclude_builder.build()?,
            priority_rules: PriorityRules::default(),
        })
    }

    /// Create a `ContentSelector` with custom patterns
    /// Alternative constructor for custom pattern configuration
    #[allow(dead_code)] // Alternative API constructor
    pub fn with_patterns(include_patterns: Vec<&str>, exclude_patterns: Vec<&str>) -> Result<Self> {
        let mut include_builder = GlobSetBuilder::new();
        for pattern in include_patterns {
            include_builder.add(Glob::new(pattern)?);
        }

        let mut exclude_builder = GlobSetBuilder::new();
        for pattern in exclude_patterns {
            exclude_builder.add(Glob::new(pattern)?);
        }

        Ok(Self {
            include_patterns: include_builder.build()?,
            exclude_patterns: exclude_builder.build()?,
            priority_rules: PriorityRules::default(),
        })
    }

    /// Create a `ContentSelector` from optional config selectors.
    ///
    /// # Precedence
    ///
    /// - If `selectors` is `Some`: use those include/exclude patterns.
    /// - If `None`: fall back to built-in defaults.
    ///
    /// # Errors
    ///
    /// Returns an error if any glob pattern is invalid.
    pub fn from_selectors(selectors: Option<&Selectors>) -> Result<Self> {
        match selectors {
            Some(sel) => {
                let mut include_builder = GlobSetBuilder::new();
                for pattern in &sel.include {
                    include_builder.add(Glob::new(pattern)?);
                }

                let mut exclude_builder = GlobSetBuilder::new();
                for pattern in &sel.exclude {
                    exclude_builder.add(Glob::new(pattern)?);
                }

                Ok(Self {
                    include_patterns: include_builder.build()?,
                    exclude_patterns: exclude_builder.build()?,
                    priority_rules: PriorityRules::default(),
                })
            }
            None => Self::new(),
        }
    }

    /// Determine the priority of a file based on its path
    #[must_use]
    pub fn get_priority(&self, path: &Utf8Path) -> Priority {
        let path_str = path.as_str();

        // *.core.yaml files are always Upstream priority (non-evictable)
        if path_str.ends_with(".core.yaml") {
            return Priority::Upstream;
        }

        // Check priority patterns
        if self.priority_rules.high.is_match(path_str) {
            Priority::High
        } else if self.priority_rules.medium.is_match(path_str) {
            Priority::Medium
        } else {
            Priority::Low
        }
    }

    /// Check if a file should be included based on include/exclude patterns
    #[must_use]
    pub fn should_include(&self, path: &Utf8Path) -> bool {
        let path_str = path.as_str();

        // First check if excluded
        if self.exclude_patterns.is_match(path_str) {
            return false;
        }

        // Then check if included
        self.include_patterns.is_match(path_str)
    }

    /// Select files from a directory with priority-based ordering
    /// Returns files grouped by priority, with LIFO ordering within each group
    pub fn select_files(&self, base_path: &Utf8Path) -> Result<Vec<SelectedFile>> {
        let mut files = Vec::new();

        // Walk the directory tree
        self.walk_directory(base_path, &mut files)?;

        // Sort by priority (Upstream first, then High, Medium, Low)
        // Within each priority, maintain LIFO order (reverse chronological)
        files.sort_by(|a, b| {
            match a.priority.cmp(&b.priority) {
                std::cmp::Ordering::Equal => {
                    // Within same priority, use LIFO (reverse order)
                    b.path.cmp(&a.path)
                }
                other => other,
            }
        });

        Ok(files)
    }

    /// Recursively walk directory and collect matching files
    fn walk_directory(&self, dir: &Utf8Path, files: &mut Vec<SelectedFile>) -> Result<()> {
        if !dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = Utf8PathBuf::try_from(entry.path()).context("Invalid UTF-8 path")?;

            if path.is_dir() {
                self.walk_directory(&path, files)?;
            } else if self.should_include(&path) {
                let priority = self.get_priority(&path);
                let content = fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read file: {path}"))?;

                // Calculate pre-redaction hash
                let mut hasher = Hasher::new();
                hasher.update(content.as_bytes());
                let blake3_pre_redaction = hasher.finalize().to_hex().to_string();

                let line_count = content.lines().count();
                let byte_count = content.len();

                files.push(SelectedFile {
                    path: path.clone(),
                    content,
                    priority,
                    blake3_pre_redaction,
                    line_count,
                    byte_count,
                });
            }
        }

        Ok(())
    }
}

/// Represents a file selected for potential inclusion in a packet
#[derive(Debug, Clone)]
pub struct SelectedFile {
    /// Path to the file
    pub path: Utf8PathBuf,
    /// File content
    pub content: String,
    /// Priority level
    pub priority: Priority,
    /// BLAKE3 hash before redaction
    pub blake3_pre_redaction: String,
    /// Number of lines in the file
    #[allow(dead_code)] // Metadata for budget tracking
    pub line_count: usize,
    /// Number of bytes in the file
    #[allow(dead_code)] // Metadata for budget tracking
    pub byte_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_priority_assignment() {
        let selector = ContentSelector::new().unwrap();

        // Test upstream priority for .core.yaml files
        assert_eq!(
            selector.get_priority(Utf8Path::new("test.core.yaml")),
            Priority::Upstream
        );
        assert_eq!(
            selector.get_priority(Utf8Path::new("docs/design.core.yaml")),
            Priority::Upstream
        );

        // Test high priority patterns
        assert_eq!(
            selector.get_priority(Utf8Path::new("SPEC-001.md")),
            Priority::High
        );
        assert_eq!(
            selector.get_priority(Utf8Path::new("docs/ADR-001.md")),
            Priority::High
        );
        assert_eq!(
            selector.get_priority(Utf8Path::new("REPORT-final.md")),
            Priority::High
        );

        // Test medium priority patterns
        assert_eq!(
            selector.get_priority(Utf8Path::new("README.md")),
            Priority::Medium
        );
        assert_eq!(
            selector.get_priority(Utf8Path::new("docs/SCHEMA.yaml")),
            Priority::Medium
        );

        // Test low priority (misc files)
        assert_eq!(
            selector.get_priority(Utf8Path::new("src/main.rs")),
            Priority::Low
        );
        assert_eq!(
            selector.get_priority(Utf8Path::new("config.toml")),
            Priority::Low
        );
    }

    #[test]
    fn test_include_exclude_patterns() {
        let selector = ContentSelector::new().unwrap();

        // Should include
        assert!(selector.should_include(Utf8Path::new("README.md")));
        assert!(selector.should_include(Utf8Path::new("config.yaml")));
        assert!(selector.should_include(Utf8Path::new("docs/spec.md")));

        // Should exclude
        assert!(!selector.should_include(Utf8Path::new("target/debug/main")));
        assert!(!selector.should_include(Utf8Path::new("node_modules/package/index.js")));
        assert!(!selector.should_include(Utf8Path::new(".git/config")));
        assert!(!selector.should_include(Utf8Path::new(".xchecker/specs/test/receipt.json")));
    }

    #[test]
    fn test_budget_tracking() {
        let mut budget = BudgetUsage::new(1000, 50);

        assert!(!budget.is_exceeded());
        assert!(!budget.would_exceed(500, 25));

        budget.add_content(500, 25);
        assert!(!budget.is_exceeded());
        assert!(budget.would_exceed(600, 10)); // Would exceed bytes
        assert!(budget.would_exceed(400, 30)); // Would exceed lines

        budget.add_content(600, 30);
        assert!(budget.is_exceeded());
    }

    #[test]
    fn test_file_selection_ordering() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;

        // Create test files with different priorities
        fs::write(base_path.join("test.core.yaml"), "upstream: content")?;
        fs::write(base_path.join("SPEC-001.md"), "# High priority spec")?;
        fs::write(base_path.join("README.md"), "# Medium priority readme")?;
        fs::write(base_path.join("config.toml"), "# Low priority config")?;

        let selector = ContentSelector::new()?;
        let files = selector.select_files(&base_path)?;

        // Verify ordering: Upstream -> High -> Medium -> Low
        assert_eq!(files.len(), 4);
        assert_eq!(files[0].priority, Priority::Upstream);
        assert_eq!(files[1].priority, Priority::High);
        assert_eq!(files[2].priority, Priority::Medium);
        assert_eq!(files[3].priority, Priority::Low);

        Ok(())
    }
}

/// Packet builder that constructs context packets with evidence tracking
/// and budget enforcement for Claude CLI invocations
#[derive(Debug)]
pub struct PacketBuilder {
    /// Content selector for file prioritization
    selector: ContentSelector,
    /// Secret redactor for protecting sensitive information
    redactor: SecretRedactor,
    /// Insight cache for performance optimization (R3.4, R3.5)
    cache: Option<InsightCache>,
    /// Maximum bytes allowed in packet
    max_bytes: usize,
    /// Maximum lines allowed in packet
    max_lines: usize,
}

impl PacketBuilder {
    /// Create a new `PacketBuilder` with default limits
    pub fn new() -> Result<Self> {
        Ok(Self {
            selector: ContentSelector::new()?,
            redactor: SecretRedactor::new()?,
            cache: None,
            max_bytes: DEFAULT_PACKET_MAX_BYTES,
            max_lines: DEFAULT_PACKET_MAX_LINES,
        })
    }

    /// Create a `PacketBuilder` with default limits and cache
    #[allow(dead_code)] // Builder pattern method for API surface
    pub fn with_cache(cache_dir: Utf8PathBuf) -> Result<Self> {
        Ok(Self {
            selector: ContentSelector::new()?,
            redactor: SecretRedactor::new()?,
            cache: Some(InsightCache::new(cache_dir)?),
            max_bytes: DEFAULT_PACKET_MAX_BYTES,
            max_lines: DEFAULT_PACKET_MAX_LINES,
        })
    }

    /// Create a `PacketBuilder` using selectors from Config, if present.
    ///
    /// If `selectors` is `None`, falls back to built-in selector defaults.
    ///
    /// # Errors
    ///
    /// Returns an error if selector patterns are invalid or redactor creation fails.
    pub fn with_selectors(selectors: Option<&Selectors>) -> Result<Self> {
        Ok(Self {
            selector: ContentSelector::from_selectors(selectors)?,
            redactor: SecretRedactor::new()?,
            cache: None,
            max_bytes: DEFAULT_PACKET_MAX_BYTES,
            max_lines: DEFAULT_PACKET_MAX_LINES,
        })
    }

    /// Create a `PacketBuilder` with selectors and custom limits.
    ///
    /// If `selectors` is `None`, falls back to built-in selector defaults.
    ///
    /// # Errors
    ///
    /// Returns an error if selector patterns are invalid or redactor creation fails.
    #[allow(dead_code)] // Builder pattern method for API surface
    pub fn with_selectors_and_limits(
        selectors: Option<&Selectors>,
        max_bytes: usize,
        max_lines: usize,
    ) -> Result<Self> {
        Ok(Self {
            selector: ContentSelector::from_selectors(selectors)?,
            redactor: SecretRedactor::new()?,
            cache: None,
            max_bytes,
            max_lines,
        })
    }

    /// Create a `PacketBuilder` with custom limits
    #[allow(dead_code)] // Builder pattern method for API surface
    pub fn with_limits(max_bytes: usize, max_lines: usize) -> Result<Self> {
        Ok(Self {
            selector: ContentSelector::new()?,
            redactor: SecretRedactor::new()?,
            cache: None,
            max_bytes,
            max_lines,
        })
    }

    /// Create a `PacketBuilder` with custom limits and cache
    #[allow(dead_code)] // Builder pattern method for API surface
    pub fn with_limits_and_cache(
        max_bytes: usize,
        max_lines: usize,
        cache_dir: Utf8PathBuf,
    ) -> Result<Self> {
        Ok(Self {
            selector: ContentSelector::new()?,
            redactor: SecretRedactor::new()?,
            cache: Some(InsightCache::new(cache_dir)?),
            max_bytes,
            max_lines,
        })
    }

    /// Create a `PacketBuilder` with custom selector and limits
    #[must_use]
    #[allow(dead_code)] // Builder pattern method for API surface
    pub fn with_selector_and_limits(
        selector: ContentSelector,
        max_bytes: usize,
        max_lines: usize,
    ) -> Self {
        Self {
            selector,
            redactor: SecretRedactor::new().expect("Failed to create SecretRedactor"),
            cache: None,
            max_bytes,
            max_lines,
        }
    }

    /// Create a `PacketBuilder` with custom redactor, selector and limits
    #[must_use]
    #[allow(dead_code)] // Builder pattern method for API surface
    pub const fn with_redactor_selector_and_limits(
        redactor: SecretRedactor,
        selector: ContentSelector,
        max_bytes: usize,
        max_lines: usize,
    ) -> Self {
        Self {
            selector,
            redactor,
            cache: None,
            max_bytes,
            max_lines,
        }
    }

    /// Create a `PacketBuilder` with all custom components
    #[must_use]
    #[allow(dead_code)] // Builder pattern method for API surface
    pub const fn with_all_components(
        redactor: SecretRedactor,
        selector: ContentSelector,
        cache: Option<InsightCache>,
        max_bytes: usize,
        max_lines: usize,
    ) -> Self {
        Self {
            selector,
            redactor,
            cache,
            max_bytes,
            max_lines,
        }
    }

    /// Get a mutable reference to the redactor for configuration
    #[allow(dead_code)] // Builder accessor for API surface
    pub const fn redactor_mut(&mut self) -> &mut SecretRedactor {
        &mut self.redactor
    }

    /// Get a reference to the redactor
    #[must_use]
    #[allow(dead_code)] // Builder accessor for API surface
    pub const fn redactor(&self) -> &SecretRedactor {
        &self.redactor
    }

    /// Get a mutable reference to the cache for configuration
    #[allow(dead_code)] // Builder accessor for API surface
    pub const fn cache_mut(&mut self) -> Option<&mut InsightCache> {
        self.cache.as_mut()
    }

    /// Get a reference to the cache
    #[must_use]
    #[allow(dead_code)] // Builder accessor for API surface
    pub const fn cache(&self) -> Option<&InsightCache> {
        self.cache.as_ref()
    }

    /// Set the cache (replaces existing cache if any)
    #[allow(dead_code)] // Builder setter for API surface
    pub fn set_cache(&mut self, cache: InsightCache) {
        self.cache = Some(cache);
    }

    /// Remove the cache
    #[allow(dead_code)] // Builder method for API surface
    pub fn remove_cache(&mut self) {
        self.cache = None;
    }

    /// Build a packet from the given base path and phase context
    /// Returns a Packet with content and evidence, or fails pre-Claude if budget exceeded
    pub fn build_packet(
        &mut self,
        base_path: &Utf8Path,
        phase: &str,
        context_dir: &Utf8Path,
        logger: Option<&Logger>,
    ) -> Result<Packet> {
        // Select files using priority-based selection
        let selected_files = self
            .selector
            .select_files(base_path)
            .with_context(|| format!("Failed to select files from {base_path}"))?;

        // First, scan all files for secrets before processing
        for file in &selected_files {
            if self
                .redactor
                .has_secrets(&file.content, file.path.as_ref())?
            {
                let matches = self
                    .redactor
                    .scan_for_secrets(&file.content, file.path.as_ref())?;
                return Err(create_secret_detected_error(&matches).into());
            }
        }

        // Build packet content with budget enforcement and redaction
        let mut budget = BudgetUsage::new(self.max_bytes, self.max_lines);
        let mut packet_content = String::new();
        let mut included_files = Vec::new();

        // First pass: Add all upstream files (never evicted)
        let mut upstream_files = Vec::new();
        let mut other_files = Vec::new();

        for file in selected_files {
            if file.priority == Priority::Upstream {
                upstream_files.push(file);
            } else {
                other_files.push(file);
            }
        }

        // Add upstream files first (always included)
        for file in upstream_files {
            let file_content = self.process_file_with_cache(&file, phase, logger)?;

            // Add file content to packet
            packet_content.push_str(&format!("=== {} ===\n", file.path));
            packet_content.push_str(&file_content);
            packet_content.push_str("\n\n");

            // Update budget based on content size
            let content_size = file_content.len() + file.path.as_str().len() + 10;
            let line_count = file_content.lines().count() + 3;
            budget.add_content(content_size, line_count);

            // Create file evidence with pre-redaction hash
            let evidence = FileEvidence {
                path: file.path.to_string(),
                range: None, // Full file for now
                blake3_pre_redaction: file.blake3_pre_redaction,
                priority: file.priority,
            };
            included_files.push(evidence);
        }

        // Check if upstream files alone exceed budget
        if budget.is_exceeded() {
            // Write packet preview before failing
            self.write_packet_preview(&packet_content, phase, context_dir)?;

            // Write packet manifest on overflow (FR-PKT-006)
            self.write_packet_manifest(&included_files, &budget, phase, context_dir)?;

            return Err(XCheckerError::PacketOverflow {
                used_bytes: budget.bytes_used,
                used_lines: budget.lines_used,
                limit_bytes: budget.max_bytes,
                limit_lines: budget.max_lines,
            }
            .into());
        }

        // Second pass: Add other files until budget is reached
        for file in other_files {
            let file_content = self.process_file_with_cache(&file, phase, logger)?;

            let content_size = file_content.len() + file.path.as_str().len() + 10;
            let line_count = file_content.lines().count() + 3;

            // Check if this file would exceed budget
            if budget.would_exceed(content_size, line_count) {
                // Skip this file to stay within budget
                continue;
            }

            // Add file content to packet
            packet_content.push_str(&format!("=== {} ===\n", file.path));
            packet_content.push_str(&file_content);
            packet_content.push_str("\n\n");

            // Update budget
            budget.add_content(content_size, line_count);

            // Create file evidence with pre-redaction hash
            let evidence = FileEvidence {
                path: file.path.to_string(),
                range: None, // Full file for now
                blake3_pre_redaction: file.blake3_pre_redaction,
                priority: file.priority,
            };
            included_files.push(evidence);
        }

        // Calculate packet hash (after redaction has been applied)
        let packet_blake3 = self.calculate_packet_hash(&packet_content);

        // Create packet evidence
        let evidence = PacketEvidence {
            files: included_files,
            max_bytes: self.max_bytes,
            max_lines: self.max_lines,
        };

        // Always write packet preview for context (redacted content)
        self.write_packet_preview(&packet_content, phase, context_dir)?;

        Ok(Packet::new(packet_content, packet_blake3, evidence, budget))
    }

    /// Calculate BLAKE3 hash of packet content
    fn calculate_packet_hash(&self, content: &str) -> String {
        let mut hasher = Hasher::new();
        hasher.update(content.as_bytes());
        hasher.finalize().to_hex().to_string()
    }

    /// Process a file with cache integration (R3.4, R3.5)
    /// Returns either cached insights or processes the file and caches the result
    fn process_file_with_cache(
        &mut self,
        file: &SelectedFile,
        phase: &str,
        logger: Option<&Logger>,
    ) -> Result<String> {
        // Calculate content hash for cache key
        let content_hash = calculate_content_hash(&file.content);

        // Try to get cached insights first (R3.4)
        if let Some(cache) = &mut self.cache
            && let Some(cached_insights) =
                cache.get_insights(&file.path, &content_hash, phase, logger)?
        {
            // Cache hit - return formatted insights
            let insights_content = format!(
                "CACHED INSIGHTS:\n{}",
                cached_insights
                    .iter()
                    .map(|insight| format!("• {insight}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
            return Ok(insights_content);
        }

        // Cache miss - process file normally
        // Apply redaction to content
        let redaction_result = self
            .redactor
            .redact_content(&file.content, file.path.as_ref())?;
        let redacted_content = redaction_result.content;

        // Generate insights if cache is available (R3.5)
        if let Some(cache) = &mut self.cache {
            let insights = cache.generate_insights(&file.content, &file.path, phase, file.priority);

            // Store insights in cache for future use
            cache.store_insights(
                &file.path,
                &file.content,
                &content_hash,
                phase,
                file.priority,
                insights.clone(),
                logger,
            )?;

            // Return insights instead of raw content for better token efficiency
            let insights_content = format!(
                "INSIGHTS:\n{}\n\nORIGINAL CONTENT:\n{}",
                insights
                    .iter()
                    .map(|insight| format!("• {insight}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
                redacted_content
            );

            Ok(insights_content)
        } else {
            // No cache - return redacted content only
            Ok(redacted_content)
        }
    }

    /// Write packet preview to context directory
    /// Always writes `context/<phase>-packet.txt` for auditability
    fn write_packet_preview(
        &self,
        content: &str,
        phase: &str,
        context_dir: &Utf8Path,
    ) -> Result<()> {
        // Ensure context directory exists (ignore benign races)
        crate::paths::ensure_dir_all(context_dir)
            .with_context(|| format!("Failed to create context directory: {context_dir}"))?;

        let preview_path = context_dir.join(format!("{}-packet.txt", phase.to_lowercase()));

        // Write packet preview
        write_file_atomic(&preview_path, content)
            .with_context(|| format!("Failed to write packet preview to: {preview_path}"))?;

        Ok(())
    }

    /// Write packet manifest on overflow (FR-PKT-006)
    /// Manifest contains only sizes, counts, and file paths (no payload content)
    fn write_packet_manifest(
        &self,
        included_files: &[FileEvidence],
        budget: &BudgetUsage,
        phase: &str,
        context_dir: &Utf8Path,
    ) -> Result<()> {
        use serde_json::json;

        // Ensure context directory exists
        crate::paths::ensure_dir_all(context_dir)
            .with_context(|| format!("Failed to create context directory: {context_dir}"))?;

        let manifest_path =
            context_dir.join(format!("{}-packet.manifest.json", phase.to_lowercase()));

        // Build manifest with sanitized information (no content)
        let manifest = json!({
            "phase": phase,
            "overflow": true,
            "budget": {
                "max_bytes": budget.max_bytes,
                "max_lines": budget.max_lines,
                "used_bytes": budget.bytes_used,
                "used_lines": budget.lines_used,
            },
            "files": included_files.iter().map(|f| {
                json!({
                    "path": f.path,
                    "priority": format!("{:?}", f.priority),
                    "blake3_pre_redaction": f.blake3_pre_redaction,
                })
            }).collect::<Vec<_>>(),
        });

        // Write manifest as JSON
        let manifest_json = serde_json::to_string_pretty(&manifest)
            .context("Failed to serialize packet manifest")?;

        write_file_atomic(&manifest_path, &manifest_json)
            .with_context(|| format!("Failed to write packet manifest to: {manifest_path}"))?;

        Ok(())
    }

    /// Write full debug packet if --debug-packet flag is set (FR-PKT-007)
    /// Only writes if secret scan passes; file is excluded from receipts
    pub fn write_debug_packet(
        &self,
        content: &str,
        phase: &str,
        context_dir: &Utf8Path,
    ) -> Result<()> {
        // Ensure context directory exists
        crate::paths::ensure_dir_all(context_dir)
            .with_context(|| format!("Failed to create context directory: {context_dir}"))?;

        let debug_path = context_dir.join(format!("{}-packet-debug.txt", phase.to_lowercase()));

        // Write full packet content (after secret scan has passed)
        write_file_atomic(&debug_path, content)
            .with_context(|| format!("Failed to write debug packet to: {debug_path}"))?;

        Ok(())
    }

    /// Log cache statistics if verbose logging is enabled and cache is available
    #[allow(dead_code)] // Diagnostic logging utility
    pub fn log_cache_stats(&self, logger: &Logger) {
        if let Some(cache) = &self.cache {
            cache.log_stats(logger);
        }
    }
}

impl Default for PacketBuilder {
    fn default() -> Self {
        Self::new().expect("Failed to create default PacketBuilder")
    }
}

#[cfg(test)]
mod packet_builder_tests {
    use super::*;
    use crate::test_support;
    use tempfile::TempDir;

    #[test]
    fn test_packet_builder_creation() -> Result<()> {
        let builder = PacketBuilder::new()?;
        assert_eq!(builder.max_bytes, DEFAULT_PACKET_MAX_BYTES);
        assert_eq!(builder.max_lines, DEFAULT_PACKET_MAX_LINES);
        Ok(())
    }

    #[test]
    fn test_packet_builder_with_custom_limits() -> Result<()> {
        let builder = PacketBuilder::with_limits(32768, 600)?;
        assert_eq!(builder.max_bytes, 32768);
        assert_eq!(builder.max_lines, 600);
        Ok(())
    }

    #[test]
    fn test_packet_construction() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
        let context_dir = base_path.join("context");

        // Create test files
        fs::write(
            base_path.join("README.md"),
            "# Test Project\nThis is a test.",
        )?;
        fs::write(base_path.join("config.yaml"), "key: value\nother: data")?;

        let mut builder = PacketBuilder::new()?;
        let packet = builder.build_packet(&base_path, "requirements", &context_dir, None)?;

        // Verify packet properties
        assert!(!packet.content.is_empty());
        assert!(!packet.blake3_hash.is_empty());
        assert_eq!(packet.evidence.files.len(), 2);
        assert!(packet.is_within_budget());

        // Verify context file was written
        let context_file = context_dir.join("requirements-packet.txt");
        assert!(context_file.exists());

        Ok(())
    }

    #[test]
    fn test_budget_overflow_handling() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
        let context_dir = base_path.join("context");

        // Create a large upstream file that exceeds budget (upstream files are always included)
        let large_content = "upstream_data: ".repeat(10000); // Large upstream content
        fs::write(base_path.join("large.core.yaml"), &large_content)?;

        // Use very small limits to trigger overflow
        let mut builder = PacketBuilder::with_limits(1000, 10)?;
        let result = builder.build_packet(&base_path, "test", &context_dir, None);

        // Should fail with PacketOverflow error because upstream file exceeds budget
        assert!(result.is_err());

        // Context file should still be written
        let context_file = context_dir.join("test-packet.txt");
        assert!(context_file.exists());

        Ok(())
    }

    #[test]
    fn test_upstream_priority_non_evictable() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
        let context_dir = base_path.join("context");

        // Create small upstream file and regular file
        fs::write(base_path.join("small.core.yaml"), "key: value")?;
        fs::write(base_path.join("large.md"), "# Large file\n".repeat(100))?; // Large regular file

        // Use limits that would exclude the large regular file but allow upstream
        let mut builder = PacketBuilder::with_limits(200, 20)?;
        let packet = builder.build_packet(&base_path, "test", &context_dir, None)?;

        // Upstream file should be included
        assert!(
            packet
                .evidence
                .files
                .iter()
                .any(|f| f.path.contains("small.core.yaml"))
        );
        // Large regular file should be excluded due to budget
        assert!(
            !packet
                .evidence
                .files
                .iter()
                .any(|f| f.path.contains("large.md"))
        );

        Ok(())
    }

    #[test]
    fn test_upstream_overflow_causes_failure() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
        let context_dir = base_path.join("context");

        // Create large upstream file that exceeds budget
        let large_content = "upstream_data: ".repeat(1000); // Large upstream content
        fs::write(base_path.join("large.core.yaml"), &large_content)?;

        // Use very small limits
        let mut builder = PacketBuilder::with_limits(100, 5)?;
        let result = builder.build_packet(&base_path, "test", &context_dir, None);

        // Should fail because upstream file exceeds budget
        assert!(result.is_err());

        // Context file should still be written
        let context_file = context_dir.join("test-packet.txt");
        assert!(context_file.exists());

        Ok(())
    }

    #[test]
    fn test_packet_hash_calculation() {
        let builder = PacketBuilder::new().unwrap();
        let content1 = "test content";
        let content2 = "test content";
        let content3 = "different content";

        let hash1 = builder.calculate_packet_hash(content1);
        let hash2 = builder.calculate_packet_hash(content2);
        let hash3 = builder.calculate_packet_hash(content3);

        // Same content should produce same hash
        assert_eq!(hash1, hash2);
        // Different content should produce different hash
        assert_ne!(hash1, hash3);

        // Hash should be valid hex string
        assert_eq!(hash1.len(), 64); // BLAKE3 produces 32-byte hash = 64 hex chars
    }

    #[test]
    fn test_secret_detection_prevents_packet_creation() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
        let context_dir = base_path.join("context");
        let token = test_support::github_pat();

        // Create a file with a secret
        fs::write(
            base_path.join("config.yaml"),
            format!("github_token: {}", token),
        )?;

        let mut builder = PacketBuilder::new()?;
        let result = builder.build_packet(&base_path, "test", &context_dir, None);

        // Should fail with SecretDetected error
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("Secret detected"));

        Ok(())
    }

    #[test]
    fn test_redaction_applied_to_packet_content() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
        let context_dir = base_path.join("context");

        // Create a file with content that would be redacted if secrets were ignored
        fs::write(base_path.join("README.md"), "# Test\nThis is safe content.")?;

        let mut builder = PacketBuilder::new()?;

        // Add a pattern that would match but then ignore it to test redaction
        builder
            .redactor_mut()
            .add_extra_pattern("test_pattern".to_string(), r"safe")?;
        builder
            .redactor_mut()
            .add_ignored_pattern("test_pattern".to_string());

        let packet = builder.build_packet(&base_path, "test", &context_dir, None)?;

        // Packet should be created successfully
        assert!(!packet.content.is_empty());
        assert!(packet.content.contains("This is safe content.")); // Should not be redacted since ignored

        Ok(())
    }

    #[test]
    fn test_pre_redaction_hash_preserved_in_evidence() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
        let context_dir = base_path.join("context");

        // Create a file with normal content
        let original_content = "# Test Project\nThis is normal content.";
        fs::write(base_path.join("README.md"), original_content)?;

        // Calculate expected pre-redaction hash
        let mut hasher = Hasher::new();
        hasher.update(original_content.as_bytes());
        let expected_hash = hasher.finalize().to_hex().to_string();

        let mut builder = PacketBuilder::new()?;
        let packet = builder.build_packet(&base_path, "test", &context_dir, None)?;

        // Check that evidence contains the pre-redaction hash
        assert_eq!(packet.evidence.files.len(), 1);
        assert_eq!(packet.evidence.files[0].blake3_pre_redaction, expected_hash);

        Ok(())
    }

    #[test]
    fn test_packet_hash_reflects_redacted_content() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
        let context_dir = base_path.join("context");
        let token = test_support::github_pat();

        // Create a file with content that contains a pattern that will be redacted
        fs::write(
            base_path.join("README.md"),
            format!("# Test\nThis contains a {} token.", token),
        )?;

        // Create a builder with the pattern ignored
        let mut builder = PacketBuilder::new()?;
        builder
            .redactor_mut()
            .add_ignored_pattern("github_pat".to_string());

        let packet = builder.build_packet(&base_path, "test", &context_dir, None)?;

        // Packet should be created successfully since pattern is ignored
        assert!(!packet.content.is_empty());
        // Content should contain the original token since it's ignored
        assert!(packet.content.contains(&token));

        Ok(())
    }

    #[test]
    fn test_redactor_configuration() -> Result<()> {
        let mut builder = PacketBuilder::new()?;

        // Test adding extra pattern
        builder
            .redactor_mut()
            .add_extra_pattern("custom".to_string(), r"CUSTOM_[A-Z0-9]+")?;

        // Test adding ignored pattern
        builder
            .redactor_mut()
            .add_ignored_pattern("github_pat".to_string());

        // Verify patterns are configured
        let pattern_ids = builder.redactor().get_pattern_ids();
        assert!(pattern_ids.contains(&"custom".to_string()));

        let ignored = builder.redactor().get_ignored_patterns();
        assert!(ignored.contains(&"github_pat".to_string()));

        Ok(())
    }

    #[test]
    fn test_cache_integration() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
        let context_dir = base_path.join("context");
        let cache_dir = base_path.join("cache");

        // Create a test file
        fs::write(
            base_path.join("README.md"),
            "# Test Project\nThis is test content for caching.",
        )?;

        // First run - should be cache miss
        let mut builder = PacketBuilder::with_cache(cache_dir.clone())?;
        let packet1 = builder.build_packet(&base_path, "requirements", &context_dir, None)?;

        // Verify cache stats show miss and write (may be more than 1 if multiple files selected)
        let stats = builder.cache().unwrap().stats();
        assert!(stats.misses >= 1);
        assert!(stats.writes >= 1);
        assert_eq!(stats.hits, 0);

        // Second run - should be cache hit
        let mut builder2 = PacketBuilder::with_cache(cache_dir)?;
        let packet2 = builder2.build_packet(&base_path, "requirements", &context_dir, None)?;

        // Verify cache stats show hit (may be more than 1 if multiple files selected)
        let stats2 = builder2.cache().unwrap().stats();
        assert!(stats2.hits >= 1);
        // Note: misses might be > 0 if there are files not in cache from first run

        // Both packets should contain insights
        assert!(packet1.content.contains("INSIGHTS:"));
        assert!(packet2.content.contains("CACHED INSIGHTS:"));

        Ok(())
    }

    #[test]
    fn test_cache_invalidation_on_content_change() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
        let context_dir = base_path.join("context");
        let cache_dir = base_path.join("cache");
        let test_file = base_path.join("README.md");

        // Create initial file
        fs::write(&test_file, "# Test Project\nOriginal content.")?;

        // First run - cache miss
        let mut builder = PacketBuilder::with_cache(cache_dir.clone())?;
        let _packet1 = builder.build_packet(&base_path, "requirements", &context_dir, None)?;
        assert!(builder.cache().unwrap().stats().misses >= 1);

        // Wait a bit to ensure different modification time
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Modify file content
        fs::write(&test_file, "# Test Project\nModified content.")?;

        // Second run - should be cache miss due to file change
        let mut builder2 = PacketBuilder::with_cache(cache_dir)?;
        let _packet2 = builder2.build_packet(&base_path, "requirements", &context_dir, None)?;

        // Should show cache miss (invalidation happens during cache lookup)
        let stats = builder2.cache().unwrap().stats();
        assert!(stats.misses >= 1);
        // Note: invalidations might be 0 if cache files were removed rather than invalidated in memory

        Ok(())
    }

    #[test]
    fn test_insights_generation_requirements_phase() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
        let context_dir = base_path.join("context");
        let cache_dir = base_path.join("cache");

        // Create a requirements document
        let requirements_content = r"
# Requirements Document

## Requirements

### Requirement 1

**User Story:** As a developer, I want to test functionality, so that I can ensure quality.

#### Acceptance Criteria

1. WHEN I run tests THEN the system SHALL pass all tests
2. WHEN errors occur THEN the system SHALL report them clearly

### Requirement 2

**User Story:** As a user, I want reliable features, so that I can be productive.
";
        fs::write(base_path.join("requirements.md"), requirements_content)?;

        let mut builder = PacketBuilder::with_cache(cache_dir)?;
        let packet = builder.build_packet(&base_path, "requirements", &context_dir, None)?;

        // Should contain insights specific to requirements
        assert!(packet.content.contains("INSIGHTS:"));
        assert!(packet.content.contains("user stories") || packet.content.contains("User Story"));
        assert!(
            packet.content.contains("acceptance criteria")
                || packet.content.contains("WHEN")
                || packet.content.contains("THEN")
        );

        Ok(())
    }

    #[test]
    fn test_cache_with_different_phases() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
        let context_dir = base_path.join("context");
        let cache_dir = base_path.join("cache");

        // Create a test file
        fs::write(
            base_path.join("design.md"),
            "# Design Document\nArchitecture and components.",
        )?;

        let mut builder = PacketBuilder::with_cache(cache_dir)?;

        // Build packet for requirements phase
        let _packet1 = builder.build_packet(&base_path, "requirements", &context_dir, None)?;

        // Build packet for design phase (same file, different phase)
        let _packet2 = builder.build_packet(&base_path, "design", &context_dir, None)?;

        // Should have separate cache entries for different phases
        let stats = builder.cache().unwrap().stats();
        assert!(stats.misses >= 2); // At least two different cache keys (may be more if multiple files)
        assert!(stats.writes >= 2); // At least two separate cache entries

        Ok(())
    }

    #[test]
    fn test_packet_builder_without_cache() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
        let context_dir = base_path.join("context");

        // Create a test file
        fs::write(
            base_path.join("README.md"),
            "# Test Project\nNo cache test.",
        )?;

        // Builder without cache
        let mut builder = PacketBuilder::new()?;
        let packet = builder.build_packet(&base_path, "requirements", &context_dir, None)?;

        // Should not contain insights (no cache)
        assert!(!packet.content.contains("INSIGHTS:"));
        assert!(!packet.content.contains("CACHED INSIGHTS:"));

        // Should contain original content
        assert!(packet.content.contains("No cache test."));

        Ok(())
    }

    // ===== Empty Input Handling Tests (Task 7.7) =====

    #[test]
    fn test_empty_packet_no_files_selected() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
        let context_dir = base_path.join("context");

        // Create files that will be excluded by default patterns
        fs::create_dir_all(base_path.join(".git"))?;
        fs::write(base_path.join(".git/config"), "git config")?;
        fs::create_dir_all(base_path.join("target"))?;
        fs::write(base_path.join("target/debug.log"), "debug output")?;

        // Build packet with no matching files
        let mut builder = PacketBuilder::new()?;
        let packet = builder.build_packet(&base_path, "requirements", &context_dir, None)?;

        // Packet should be empty (no files selected)
        assert!(packet.content.is_empty() || packet.content.trim().is_empty());
        assert_eq!(packet.evidence.files.len(), 0);
        assert!(packet.is_within_budget());

        // Context file should still be written (even if empty)
        let context_file = context_dir.join("requirements-packet.txt");
        assert!(context_file.exists());

        Ok(())
    }

    #[test]
    fn test_empty_file_in_packet() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
        let context_dir = base_path.join("context");

        // Create an empty file that matches include patterns
        fs::write(base_path.join("empty.md"), "")?;
        fs::write(base_path.join("README.md"), "# Non-empty content")?;

        let mut builder = PacketBuilder::new()?;
        let packet = builder.build_packet(&base_path, "requirements", &context_dir, None)?;

        // Packet should include both files
        assert_eq!(packet.evidence.files.len(), 2);

        // Verify empty file is included in evidence
        let empty_file_evidence = packet
            .evidence
            .files
            .iter()
            .find(|f| f.path.contains("empty.md"));
        assert!(empty_file_evidence.is_some());

        // Packet content should contain file markers
        assert!(packet.content.contains("=== "));
        assert!(packet.content.contains("empty.md"));

        Ok(())
    }

    #[test]
    fn test_whitespace_only_file_in_packet() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
        let context_dir = base_path.join("context");

        // Create a file with only whitespace
        fs::write(base_path.join("whitespace.md"), "   \n\t\n   ")?;
        fs::write(base_path.join("README.md"), "# Content")?;

        let mut builder = PacketBuilder::new()?;
        let packet = builder.build_packet(&base_path, "requirements", &context_dir, None)?;

        // Both files should be included
        assert_eq!(packet.evidence.files.len(), 2);

        // Whitespace should be preserved in packet
        assert!(packet.content.contains("whitespace.md"));

        Ok(())
    }

    #[test]
    fn test_secret_scanning_on_empty_content() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
        let context_dir = base_path.join("context");

        // Create an empty file
        fs::write(base_path.join("empty.yaml"), "")?;

        let mut builder = PacketBuilder::new()?;

        // Should not fail on secret scanning for empty content
        let result = builder.build_packet(&base_path, "requirements", &context_dir, None);
        assert!(result.is_ok());

        let packet = result.unwrap();
        assert_eq!(packet.evidence.files.len(), 1);

        Ok(())
    }

    #[test]
    fn test_empty_directory_packet_generation() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
        let context_dir = base_path.join("context");

        // Create empty directory structure
        fs::create_dir_all(base_path.join("docs"))?;
        fs::create_dir_all(base_path.join("src"))?;

        let mut builder = PacketBuilder::new()?;
        let packet = builder.build_packet(&base_path, "requirements", &context_dir, None)?;

        // No files should be selected from empty directories
        assert_eq!(packet.evidence.files.len(), 0);
        assert!(packet.content.is_empty() || packet.content.trim().is_empty());

        // Context file should still be written
        let context_file = context_dir.join("requirements-packet.txt");
        assert!(context_file.exists());

        Ok(())
    }

    #[test]
    fn test_empty_packet_budget_tracking() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
        let context_dir = base_path.join("context");

        // No files created - completely empty
        let mut builder = PacketBuilder::new()?;
        let packet = builder.build_packet(&base_path, "requirements", &context_dir, None)?;

        // Budget should show zero usage
        assert_eq!(packet.budget_used.bytes_used, 0);
        assert_eq!(packet.budget_used.lines_used, 0);
        assert!(packet.is_within_budget());

        Ok(())
    }

    #[test]
    fn test_empty_file_with_cache() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
        let context_dir = base_path.join("context");
        let cache_dir = base_path.join("cache");

        // Create an empty file
        fs::write(base_path.join("empty.md"), "")?;

        let mut builder = PacketBuilder::with_cache(cache_dir)?;
        let packet = builder.build_packet(&base_path, "requirements", &context_dir, None)?;

        // Empty file should be processed without errors
        assert_eq!(packet.evidence.files.len(), 1);

        // Cache should handle empty content gracefully
        if let Some(cache) = builder.cache() {
            let stats = cache.stats();
            // Should have attempted to process the file
            assert!(stats.hits + stats.misses >= 1);
        }

        Ok(())
    }

    #[test]
    fn test_nonexistent_directory_packet_generation() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
        let nonexistent_path = base_path.join("nonexistent");
        let context_dir = base_path.join("context");

        let mut builder = PacketBuilder::new()?;

        // Should handle nonexistent directory gracefully
        let packet = builder.build_packet(&nonexistent_path, "requirements", &context_dir, None)?;

        // No files should be selected
        assert_eq!(packet.evidence.files.len(), 0);
        assert!(packet.content.is_empty() || packet.content.trim().is_empty());

        Ok(())
    }

    // ===== Selector Wiring Tests (B2) =====

    #[test]
    fn test_content_selector_from_selectors_uses_defaults_when_none() -> Result<()> {
        let selector = ContentSelector::from_selectors(None)?;

        // Verify that when None is passed, selector uses default patterns
        // We test this by checking it accepts default include patterns and rejects default excludes
        // Default includes: *.md, *.yaml, *.yml, etc.
        assert!(selector.should_include(Utf8Path::new("README.md")));
        assert!(selector.should_include(Utf8Path::new("config.yaml")));

        // Default excludes: .git/*, target/*, node_modules/*, etc.
        assert!(!selector.should_include(Utf8Path::new(".git/config")));
        assert!(!selector.should_include(Utf8Path::new("target/debug/main")));
        assert!(!selector.should_include(Utf8Path::new("node_modules/foo/bar.js")));

        Ok(())
    }

    #[test]
    fn test_content_selector_from_selectors_uses_provided_patterns() -> Result<()> {
        let selectors = Selectors {
            include: vec!["src/**".to_string()],
            exclude: vec!["**/*.log".to_string()],
        };

        let selector = ContentSelector::from_selectors(Some(&selectors))?;

        // Should include files matching custom include patterns
        assert!(selector.should_include(Utf8Path::new("src/main.rs")));
        assert!(selector.should_include(Utf8Path::new("src/lib/utils.rs")));

        // Should NOT include files outside custom include patterns (no default includes)
        assert!(!selector.should_include(Utf8Path::new("README.md")));
        assert!(!selector.should_include(Utf8Path::new("config.yaml")));

        // Should exclude files matching custom exclude patterns
        assert!(!selector.should_include(Utf8Path::new("src/debug.log")));

        Ok(())
    }

    #[test]
    fn test_content_selector_from_selectors_empty_patterns() -> Result<()> {
        let selectors = Selectors {
            include: vec![],
            exclude: vec![],
        };

        let selector = ContentSelector::from_selectors(Some(&selectors))?;

        // With empty patterns, nothing matches include (empty globset never matches)
        // and nothing matches exclude (empty globset never matches)
        assert!(!selector.should_include(Utf8Path::new("README.md")));
        assert!(!selector.should_include(Utf8Path::new("src/main.rs")));

        Ok(())
    }

    #[test]
    fn test_packet_builder_with_selectors_uses_defaults_when_none() -> Result<()> {
        let builder = PacketBuilder::with_selectors(None)?;

        // Verify builder uses default limits
        assert_eq!(builder.max_bytes, DEFAULT_PACKET_MAX_BYTES);
        assert_eq!(builder.max_lines, DEFAULT_PACKET_MAX_LINES);

        Ok(())
    }

    #[test]
    fn test_packet_builder_with_selectors_accepts_custom_patterns() -> Result<()> {
        let selectors = Selectors {
            include: vec!["**/*.rs".to_string()],
            exclude: vec!["**/test_*.rs".to_string()],
        };

        let builder = PacketBuilder::with_selectors(Some(&selectors))?;

        // Verify builder was created successfully with custom selectors
        assert_eq!(builder.max_bytes, DEFAULT_PACKET_MAX_BYTES);
        assert_eq!(builder.max_lines, DEFAULT_PACKET_MAX_LINES);

        Ok(())
    }

    #[test]
    fn test_packet_builder_with_selectors_and_limits() -> Result<()> {
        let selectors = Selectors {
            include: vec!["docs/**".to_string()],
            exclude: vec![],
        };

        let builder = PacketBuilder::with_selectors_and_limits(Some(&selectors), 32768, 600)?;

        // Verify builder was created with custom limits
        assert_eq!(builder.max_bytes, 32768);
        assert_eq!(builder.max_lines, 600);

        Ok(())
    }

    #[test]
    fn test_packet_builder_with_custom_selectors_filters_files() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
        let context_dir = base_path.join("context");

        // Create various test files
        fs::write(base_path.join("README.md"), "# Test Project")?;
        fs::write(base_path.join("config.yaml"), "key: value")?;
        fs::create_dir_all(base_path.join("src"))?;
        fs::write(base_path.join("src/main.rs"), "fn main() {}")?;
        fs::write(base_path.join("src/lib.rs"), "pub fn lib() {}")?;

        // Create builder with custom selectors that only include .rs files
        let selectors = Selectors {
            include: vec!["**/*.rs".to_string()],
            exclude: vec![],
        };
        let mut builder = PacketBuilder::with_selectors(Some(&selectors))?;
        let packet = builder.build_packet(&base_path, "requirements", &context_dir, None)?;

        // Should only include .rs files
        assert_eq!(packet.evidence.files.len(), 2);
        assert!(
            packet
                .evidence
                .files
                .iter()
                .all(|f| f.path.ends_with(".rs"))
        );

        // README.md and config.yaml should NOT be included
        assert!(
            !packet
                .evidence
                .files
                .iter()
                .any(|f| f.path.ends_with(".md"))
        );
        assert!(
            !packet
                .evidence
                .files
                .iter()
                .any(|f| f.path.ends_with(".yaml"))
        );

        Ok(())
    }
}

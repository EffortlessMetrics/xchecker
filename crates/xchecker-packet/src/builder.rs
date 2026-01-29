use super::model::{CandidateFile, SelectedFile};
use super::selectors::ContentSelector;
use crate::{BudgetUsage, Packet};
use anyhow::{Context, Result};
use blake3::Hasher;
use camino::{Utf8Path, Utf8PathBuf};
use std::fs;
use std::sync::{Arc, Mutex};
use std::thread;
use xchecker_config::Selectors;
use xchecker_redaction::SecretRedactor;
use xchecker_utils::cache::InsightCache;
use xchecker_utils::error::XCheckerError;
use xchecker_utils::logging::Logger;
use xchecker_utils::types::{FileEvidence, PacketEvidence, Priority};

/// Default maximum bytes allowed in a packet
pub const DEFAULT_PACKET_MAX_BYTES: usize = 65536;

/// Default maximum lines allowed in a packet
pub const DEFAULT_PACKET_MAX_LINES: usize = 1200;

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
            selector: ContentSelector::new()?.max_file_size(DEFAULT_PACKET_MAX_BYTES as u64),
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
            selector: ContentSelector::new()?.max_file_size(DEFAULT_PACKET_MAX_BYTES as u64),
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
            selector: ContentSelector::from_selectors(selectors)?
                .max_file_size(DEFAULT_PACKET_MAX_BYTES as u64),
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
            selector: ContentSelector::from_selectors(selectors)?.max_file_size(max_bytes as u64),
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
            selector: ContentSelector::new()?.max_file_size(max_bytes as u64),
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
            selector: ContentSelector::new()?.max_file_size(max_bytes as u64),
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
            selector: selector.max_file_size(max_bytes as u64),
            redactor: SecretRedactor::new().expect("Failed to create SecretRedactor"),
            cache: None,
            max_bytes,
            max_lines,
        }
    }

    /// Create a `PacketBuilder` with custom redactor, selector and limits
    #[must_use]
    #[allow(dead_code)] // Builder pattern method for API surface
    pub fn with_redactor_selector_and_limits(
        redactor: SecretRedactor,
        selector: ContentSelector,
        max_bytes: usize,
        max_lines: usize,
    ) -> Self {
        Self {
            selector: selector.max_file_size(max_bytes as u64),
            redactor,
            cache: None,
            max_bytes,
            max_lines,
        }
    }

    /// Create a `PacketBuilder` with all custom components
    #[must_use]
    #[allow(dead_code)] // Builder pattern method for API surface
    pub fn with_all_components(
        redactor: SecretRedactor,
        selector: ContentSelector,
        cache: Option<InsightCache>,
        max_bytes: usize,
        max_lines: usize,
    ) -> Self {
        Self {
            selector: selector.max_file_size(max_bytes as u64),
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

    /// Enable or disable symlink following for content selection.
    ///
    /// When enabled, symlinks are only followed if they resolve to paths
    /// within the base directory being scanned (sandbox validation).
    /// This prevents path traversal attacks.
    ///
    /// Default is `false` (symlinks are skipped for security).
    #[must_use]
    #[allow(dead_code)] // Builder configuration method
    pub fn allow_symlinks(mut self, allow: bool) -> Self {
        self.selector = self.selector.allow_symlinks(allow);
        self
    }

    /// Build a packet from the given base path and phase context
    /// Returns a Packet with content and evidence, or fails pre-Claude if budget exceeded
    pub fn build_packet(
        &mut self,
        base_path: &Utf8Path,
        phase: &str,
        context_dir: &Utf8Path,
        _logger: Option<&Logger>,
    ) -> Result<Packet> {
        // Select candidates using lazy selection (no content reading yet)
        let candidates = self
            .selector
            .select_candidates(base_path)
            .with_context(|| format!("Failed to select files from {base_path}"))?;

        // Prepare for parallel processing
        // Extract cache to wrap in Arc<Mutex>
        let cache_arc = self.cache.take().map(|c| Arc::new(Mutex::new(c)));
        let redactor_ref = &self.redactor;
        let max_file_size = self.selector.get_max_file_size();

        // Process files in parallel
        // We use std::thread::scope to allow sharing references (like redactor_ref)
        let num_threads = thread::available_parallelism().map_or(4, |n| n.get());
        let chunk_size = if candidates.is_empty() {
            1
        } else {
            candidates.len().div_ceil(num_threads)
        };

        // Process candidates in chunks
        let process_results = thread::scope(|s| {
            let mut handles = Vec::new();
            for chunk in candidates.chunks(chunk_size) {
                // Clone the Arc for the cache (cheap)
                let cache_clone = cache_arc.as_ref().map(|arc| arc.clone());

                let handle = s.spawn(move || {
                    let mut results = Vec::with_capacity(chunk.len());
                    for candidate in chunk {
                        let result = process_candidate_file(
                            candidate,
                            max_file_size,
                            phase,
                            redactor_ref,
                            cache_clone.as_ref(),
                        );
                        results.push(result);
                    }
                    results
                });
                handles.push(handle);
            }

            // Collect results preserving order
            let mut all_results = Vec::with_capacity(candidates.len());
            for handle in handles {
                if let Ok(chunk_results) = handle.join() {
                    all_results.extend(chunk_results);
                } else {
                    // One thread panicked
                    return Err(anyhow::anyhow!(
                        "Worker thread panicked during packet assembly"
                    ));
                }
            }
            Ok(all_results)
        })?;

        // Restore cache to self
        if let Some(arc) = cache_arc {
            if let Ok(mutex) = Arc::try_unwrap(arc) {
                self.cache = Some(mutex.into_inner().unwrap());
            } else {
                // This should not happen as all threads have joined and dropped their clones
                return Err(anyhow::anyhow!("Failed to unwrap cache Arc"));
            }
        }

        // Build packet from results
        let mut budget = BudgetUsage::new(self.max_bytes, self.max_lines);
        let mut packet_content = String::new();
        let mut included_files = Vec::new();

        // Separate Upstream and Other results to apply budget logic
        // process_results corresponds 1:1 to candidates
        let mut upstream_results = Vec::new();
        let mut other_results = Vec::new();

        for (candidate, result) in candidates.iter().zip(process_results.into_iter()) {
            if candidate.priority == Priority::Upstream {
                upstream_results.push((candidate, result));
            } else {
                other_results.push((candidate, result));
            }
        }

        // First pass: Add all upstream files
        for (_candidate, result) in upstream_results {
            // Propagate errors from processing
            match result {
                Ok(Some((file, file_content, content_size, line_count))) => {
                    // Add file content to packet
                    let redacted_path = self.redactor.redact_string(file.path.as_str());
                    packet_content.push_str(&format!("=== {} ===\n", redacted_path));
                    packet_content.push_str(&file_content);
                    packet_content.push_str("\n\n");

                    // Update budget
                    budget.add_content(content_size, line_count);

                    // Create file evidence
                    let evidence = FileEvidence {
                        path: file.path.to_string(),
                        range: None, // Full file for now
                        blake3_pre_redaction: file.blake3_pre_redaction,
                        priority: file.priority,
                    };
                    included_files.push(evidence);
                }
                Ok(None) => { /* Skipped file */ }
                Err(e) => return Err(e),
            }
        }

        // Check if upstream files alone exceed budget
        if budget.is_exceeded() {
            self.write_packet_preview(&packet_content, phase, context_dir)?;
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
        for (_candidate, result) in other_results {
            match result {
                Ok(Some((file, file_content, content_size, line_count))) => {
                    // Check if this file would exceed budget
                    if budget.would_exceed(content_size, line_count) {
                        // Skip this file to stay within budget
                        continue;
                    }

                    // Add file content to packet
                    let redacted_path = self.redactor.redact_string(file.path.as_str());
                    packet_content.push_str(&format!("=== {} ===\n", redacted_path));
                    packet_content.push_str(&file_content);
                    packet_content.push_str("\n\n");

                    // Update budget
                    budget.add_content(content_size, line_count);

                    // Create file evidence
                    let evidence = FileEvidence {
                        path: file.path.to_string(),
                        range: None, // Full file for now
                        blake3_pre_redaction: file.blake3_pre_redaction,
                        priority: file.priority,
                    };
                    included_files.push(evidence);
                }
                Ok(None) => { /* Skipped file */ }
                Err(e) => return Err(e),
            }
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

    /// Log cache statistics if verbose logging is enabled and cache is available
    #[allow(dead_code)] // Diagnostic logging utility
    pub fn log_cache_stats(&self, logger: &Logger) {
        if let Some(cache) = &self.cache {
            cache.log_stats(logger);
        }
    }
}

/// Helper function to process a single candidate file in parallel.
/// This encapsulates reading, hashing, redaction, and cache interaction.
fn process_candidate_file(
    candidate: &CandidateFile,
    max_file_size: u64,
    phase: &str,
    redactor: &SecretRedactor,
    cache: Option<&Arc<Mutex<InsightCache>>>,
) -> Result<Option<(SelectedFile, String, usize, usize)>> {
    // DoS protection: check file size before reading
    let metadata = fs::metadata(&candidate.path)
        .with_context(|| format!("Failed to get file metadata: {}", candidate.path))?;

    if !metadata.is_file() {
        return Ok(None);
    }

    if metadata.len() > max_file_size {
        // For upstream files (critical context), fail hard if they exceed the limit
        if candidate.priority == Priority::Upstream {
            return Err(anyhow::anyhow!(
                "Upstream file {} exceeds size limit of {} bytes (size: {}). \
                 Critical context files must fit within the configured limit.",
                candidate.path,
                max_file_size,
                metadata.len()
            ));
        }

        tracing::warn!(
            "Skipping large file: {} ({} bytes > limit {})",
            candidate.path,
            metadata.len(),
            max_file_size
        );
        return Ok(None);
    }

    // Read content
    let content = fs::read_to_string(&candidate.path)
        .with_context(|| format!("Failed to read file: {}", candidate.path))?;

    // Scan for secrets immediately after reading
    if redactor.has_secrets(&content, candidate.path.as_ref())? {
        let matches = redactor.scan_for_secrets(&content, candidate.path.as_ref())?;
        return Err(XCheckerError::SecretDetected {
            pattern: matches
                .first()
                .map(|m| m.pattern_id.clone())
                .unwrap_or_else(|| "unknown".to_string()),
            location: matches
                .first()
                .map(|m| m.file_path.clone())
                .unwrap_or_else(|| "unknown".to_string()),
        }
        .into());
    }

    // Calculate pre-redaction hash
    let mut hasher = Hasher::new();
    hasher.update(content.as_bytes());
    let blake3_pre_redaction = hasher.finalize().to_hex().to_string();

    let line_count_raw = content.lines().count();
    let byte_count_raw = content.len();

    let selected_file = SelectedFile {
        path: candidate.path.clone(),
        content: content.clone(), // Clone needed for SelectedFile
        priority: candidate.priority,
        blake3_pre_redaction: blake3_pre_redaction.clone(),
        line_count: line_count_raw,
        byte_count: byte_count_raw,
    };

    // Cache Logic Inlined
    let file_content = if let Some(cache_mutex) = cache {
        // Try to get cached insights
        let cached_insights = {
            let mut guard = cache_mutex.lock().expect("Cache mutex poisoned");
            // Pass None for logger to avoid Sync issues in threads
            guard.get_insights(&selected_file.path, &blake3_pre_redaction, phase, None)?
        };

        if let Some(insights) = cached_insights {
            format!(
                "CACHED INSIGHTS:\n{}",
                insights
                    .iter()
                    .map(|insight| format!("• {insight}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        } else {
            // Cache miss
            let redaction_result = redactor.redact_content(&content, candidate.path.as_ref())?;
            let redacted_content = redaction_result.content;

            // Generate insights
            // Use a temporary cache instance or lock again?
            // generate_insights is a method on InsightCache, but it is pure logic if we look at it?
            // Wait, generate_insights is `&self`.
            // So we need to lock to call it?

            // InsightCache methods:
            // pub fn generate_insights(&self, ...)

            // We need a reference to cache to call generate_insights.
            // But we dropped the lock.

            let insights = {
                let guard = cache_mutex.lock().expect("Cache mutex poisoned");
                guard.generate_insights(&content, &candidate.path, phase, candidate.priority)
            };

            // Store insights
            {
                let mut guard = cache_mutex.lock().expect("Cache mutex poisoned");
                guard.store_insights(
                    &candidate.path,
                    &content,
                    &blake3_pre_redaction,
                    phase,
                    candidate.priority,
                    insights.clone(),
                    None,
                )?;
            }

            format!(
                "INSIGHTS:\n{}\n\nORIGINAL CONTENT:\n{}",
                insights
                    .iter()
                    .map(|insight| format!("• {insight}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
                redacted_content
            )
        }
    } else {
        // No cache
        let redaction_result = redactor.redact_content(&content, candidate.path.as_ref())?;
        redaction_result.content
    };

    let content_size = file_content.len() + candidate.path.as_str().len() + 10;
    let line_count = file_content.lines().count() + 3;

    Ok(Some((
        selected_file,
        file_content,
        content_size,
        line_count,
    )))
}

impl Default for PacketBuilder {
    fn default() -> Self {
        Self::new().expect("Failed to create default PacketBuilder")
    }
}

#[cfg(test)]
mod packet_builder_tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;
    use xchecker_utils::test_support;

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

        // Should fail because upstream file exceeds size limit (which matches packet limit)
        assert!(result.is_err());
        let err = result.unwrap_err();
        // Use Debug format to check error chain
        let err_msg = format!("{:?}", err);
        assert!(err_msg.contains("Upstream file"));
        assert!(err_msg.contains("exceeds size limit"));

        // Context file is NOT written when file selection fails early
        let context_file = context_dir.join("test-packet.txt");
        assert!(!context_file.exists());

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

        // Should fail because upstream file exceeds size limit
        assert!(result.is_err());
        let err = result.unwrap_err();
        // Use Debug format to check error chain
        let err_msg = format!("{:?}", err);
        assert!(err_msg.contains("Upstream file"));
        assert!(err_msg.contains("exceeds size limit"));

        // Context file is NOT written when file selection fails early
        let context_file = context_dir.join("test-packet.txt");
        assert!(!context_file.exists());

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

    // ===== Filename Redaction Security Tests =====

    #[test]
    fn test_filename_with_secret_is_redacted_in_packet_content() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
        let context_dir = base_path.join("context");

        // Create a file with a GitHub PAT in the filename
        // Note: We use a valid token format that will be detected by the secret patterns
        let secret_token = test_support::github_pat();
        let filename_with_secret = format!("config_{}.yaml", secret_token);
        fs::write(base_path.join(&filename_with_secret), "key: value")?;

        let mut builder = PacketBuilder::new()?;
        let packet = builder.build_packet(&base_path, "requirements", &context_dir, None)?;

        // Packet should have included the file
        assert_eq!(packet.evidence.files.len(), 1);

        // The packet CONTENT should NOT contain the raw secret in the filename header
        // It should be redacted to "***"
        assert!(
            !packet.content.contains(&secret_token),
            "Packet content should NOT contain the raw secret token in filename"
        );

        // The redacted filename should be in the content (with *** replacing the secret)
        assert!(
            packet.content.contains("config_***"),
            "Packet content should contain redacted filename with ***"
        );

        // The file evidence path should still contain the original path
        // (evidence is internal, not sent to LLM)
        assert!(packet.evidence.files[0].path.contains(&secret_token));

        Ok(())
    }

    #[test]
    fn test_filename_with_aws_key_is_redacted_in_packet_content() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
        let context_dir = base_path.join("context");

        // Create a file with an AWS access key in the filename
        let aws_key = test_support::aws_access_key_id();
        let filename_with_secret = format!("backup_{}.md", aws_key);
        fs::write(base_path.join(&filename_with_secret), "# Backup data")?;

        let mut builder = PacketBuilder::new()?;
        let packet = builder.build_packet(&base_path, "requirements", &context_dir, None)?;

        // The packet content should NOT contain the raw AWS key
        assert!(
            !packet.content.contains(&aws_key),
            "Packet content should NOT contain the raw AWS access key in filename"
        );

        // Should contain redacted marker
        assert!(
            packet.content.contains("***"),
            "Packet content should contain redacted marker"
        );

        Ok(())
    }

    #[test]
    fn test_filename_without_secret_unchanged_in_packet_content() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
        let context_dir = base_path.join("context");

        // Create a file with a normal filename (no secrets)
        fs::write(base_path.join("normal_config.yaml"), "key: value")?;

        let mut builder = PacketBuilder::new()?;
        let packet = builder.build_packet(&base_path, "requirements", &context_dir, None)?;

        // The packet content should contain the normal filename unchanged
        assert!(
            packet.content.contains("normal_config.yaml"),
            "Packet content should contain normal filename unchanged"
        );

        // Should NOT contain redaction markers for this file
        assert!(
            !packet.content.contains("normal_config*"),
            "Normal filename should not be redacted"
        );

        Ok(())
    }
}

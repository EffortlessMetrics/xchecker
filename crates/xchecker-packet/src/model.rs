use camino::Utf8PathBuf;
use globset::{Glob, GlobSet, GlobSetBuilder};
use xchecker_utils::types::Priority;

/// Priority rules defining the selection order
/// Order: *.core.yaml (non-evictable) → SPEC/ADR/REPORT → README/SCHEMA → misc
/// LIFO within each priority class
#[derive(Debug, Clone)]
pub struct PriorityRules {
    /// Combined globset for all priorities (High -> Medium -> Low)
    pub combined: GlobSet,
    /// Start index for medium priority patterns
    pub medium_start_index: usize,
    /// Start index for low priority patterns
    pub low_start_index: usize,
}

impl Default for PriorityRules {
    fn default() -> Self {
        // Use const slices to avoid heap allocation for static pattern data
        const HIGH_PATTERNS: &[&str] = &[
            "**/SPEC*",
            "**/ADR*",
            "**/REPORT*",
            "**/*SPEC*",
            "**/*ADR*",
            "**/*REPORT*",
            // Problem statement files get high priority - critical context for LLM
            "**/problem-statement*",
            "**/*problem-statement*",
        ];

        const MEDIUM_PATTERNS: &[&str] =
            &["**/README*", "**/SCHEMA*", "**/*README*", "**/*SCHEMA*"];

        const LOW_PATTERNS: &[&str] = &[
            "**/*", // Catch-all for misc files
        ];

        let mut builder = GlobSetBuilder::new();
        for p in HIGH_PATTERNS
            .iter()
            .chain(MEDIUM_PATTERNS)
            .chain(LOW_PATTERNS)
        {
            builder.add(Glob::new(p).unwrap());
        }

        Self {
            combined: builder.build().unwrap(),
            medium_start_index: HIGH_PATTERNS.len(),
            low_start_index: HIGH_PATTERNS.len() + MEDIUM_PATTERNS.len(),
        }
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

/// Represents a candidate file for selection (lazy loading)
#[derive(Debug, Clone)]
pub struct CandidateFile {
    /// Path to the file
    pub path: Utf8PathBuf,
    /// Priority level
    pub priority: Priority,
}

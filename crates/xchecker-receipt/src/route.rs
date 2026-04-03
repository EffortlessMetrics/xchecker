//! Route receipts record *why* a particular fix approach was chosen.
//!
//! Each [`RouteReceipt`] captures the reasoning behind a routing decision
//! -- for example, serializing tests that mutate global state, downgrading
//! a performance assertion on a non-strict runner, or skipping an
//! unsupported capability.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// RouteKind
// ---------------------------------------------------------------------------

/// The category of routing decision that was made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteKind {
    /// Serialize tests that mutate global state so they cannot race.
    SerializeGlobalStateTests,
    /// Downgrade a performance assertion when running on a non-strict runner.
    AdvisoryPerfOnNonStrictRunner,
    /// Skip an unsupported capability entirely.
    CapabilitySkip,
    /// Pin a dependency to a known-good version.
    DependencyPin,
    /// Apply a correctness fix.
    CorrectnessFix,
    /// Optimize resource usage (memory, CPU, handles, etc.).
    ResourceOptimization,
    /// Improve test isolation (temp dirs, env vars, ports, etc.).
    TestIsolation,
    /// Correct or update documentation.
    DocCorrection,
    /// A project-specific routing reason not covered by the built-in variants.
    Custom(String),
}

impl std::fmt::Display for RouteKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SerializeGlobalStateTests => write!(f, "serialize global-state tests"),
            Self::AdvisoryPerfOnNonStrictRunner => {
                write!(f, "advisory perf on non-strict runner")
            }
            Self::CapabilitySkip => write!(f, "capability skip"),
            Self::DependencyPin => write!(f, "dependency pin"),
            Self::CorrectnessFix => write!(f, "correctness fix"),
            Self::ResourceOptimization => write!(f, "resource optimization"),
            Self::TestIsolation => write!(f, "test isolation"),
            Self::DocCorrection => write!(f, "doc correction"),
            Self::Custom(reason) => write!(f, "custom: {reason}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Alternative
// ---------------------------------------------------------------------------

/// An alternative approach that was considered but rejected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Alternative {
    /// What the alternative approach would have done.
    pub description: String,
    /// Why this alternative was rejected.
    pub reason_rejected: String,
}

// ---------------------------------------------------------------------------
// RouteReceipt
// ---------------------------------------------------------------------------

/// A receipt that records *why* a particular fix approach was chosen.
///
/// Route receipts complement the existing phase receipts by capturing the
/// reasoning behind routing decisions.  They are designed to be serialized
/// as JCS-canonical JSON alongside regular receipts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouteReceipt {
    /// Schema version for forward compatibility (currently `"1"`).
    pub schema_version: String,
    /// When this receipt was created.
    pub emitted_at: DateTime<Utc>,
    /// The category of routing decision.
    pub route_kind: RouteKind,
    /// The file targeted by the fix (relative to spec root).
    pub target_file: String,
    /// A short, human-readable summary of what was done.
    pub description: String,
    /// The reasoning behind the decision.
    pub rationale: String,
    /// Supporting evidence (log lines, test output, etc.).
    pub evidence: Vec<String>,
    /// Alternatives that were considered and why they were rejected.
    pub alternatives_considered: Vec<Alternative>,
    /// Confidence level in the chosen approach (0.0 -- 1.0).
    pub confidence: f64,
    /// Whether this change can be safely reverted later.
    pub reversible: bool,
}

impl RouteReceipt {
    /// Produce a one-line summary suitable for logs or TUI display.
    ///
    /// Format: `[{route_kind}] {target_file}: {description}`
    #[must_use]
    pub fn summary_line(&self) -> String {
        format!(
            "[{}] {}: {}",
            self.route_kind, self.target_file, self.description
        )
    }

    /// Serialize to JCS-canonical JSON (RFC 8785).
    ///
    /// # Errors
    ///
    /// Returns an error if serialization or canonicalization fails.
    pub fn to_json(&self) -> anyhow::Result<String> {
        let value = serde_json::to_value(self)?;
        let bytes = serde_json_canonicalizer::to_vec(&value)?;
        Ok(String::from_utf8(bytes)?)
    }
}

// ---------------------------------------------------------------------------
// RouteReceiptBuilder
// ---------------------------------------------------------------------------

/// Builder for [`RouteReceipt`].
///
/// Only `route_kind`, `target_file`, `description`, and `rationale` are
/// mandatory.  Everything else has sensible defaults.
///
/// # Example
///
/// ```
/// use xchecker_receipt::route::{RouteReceiptBuilder, RouteKind};
///
/// let receipt = RouteReceiptBuilder::new(
///     RouteKind::TestIsolation,
///     "tests/integration/server.rs",
///     "Use per-test temp dirs instead of shared /tmp/out",
///     "Shared temp dir causes flaky failures under parallel execution",
/// )
/// .confidence(0.95)
/// .reversible(true)
/// .build();
/// ```
pub struct RouteReceiptBuilder {
    route_kind: RouteKind,
    target_file: String,
    description: String,
    rationale: String,
    evidence: Vec<String>,
    alternatives_considered: Vec<Alternative>,
    confidence: f64,
    reversible: bool,
}

impl RouteReceiptBuilder {
    /// Start building a new route receipt.
    #[must_use]
    pub fn new(
        route_kind: RouteKind,
        target_file: impl Into<String>,
        description: impl Into<String>,
        rationale: impl Into<String>,
    ) -> Self {
        Self {
            route_kind,
            target_file: target_file.into(),
            description: description.into(),
            rationale: rationale.into(),
            evidence: Vec::new(),
            alternatives_considered: Vec::new(),
            confidence: 1.0,
            reversible: true,
        }
    }

    /// Add a piece of evidence (log line, test output, etc.).
    #[must_use]
    pub fn evidence(mut self, item: impl Into<String>) -> Self {
        self.evidence.push(item.into());
        self
    }

    /// Add multiple evidence items at once.
    #[must_use]
    pub fn evidence_all(mut self, items: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.evidence.extend(items.into_iter().map(Into::into));
        self
    }

    /// Record an alternative that was considered and rejected.
    #[must_use]
    pub fn alternative(
        mut self,
        description: impl Into<String>,
        reason_rejected: impl Into<String>,
    ) -> Self {
        self.alternatives_considered.push(Alternative {
            description: description.into(),
            reason_rejected: reason_rejected.into(),
        });
        self
    }

    /// Set the confidence level (clamped to 0.0 -- 1.0).
    #[must_use]
    pub fn confidence(mut self, value: f64) -> Self {
        self.confidence = value.clamp(0.0, 1.0);
        self
    }

    /// Set whether the change is reversible.
    #[must_use]
    pub fn reversible(mut self, value: bool) -> Self {
        self.reversible = value;
        self
    }

    /// Consume the builder and produce a [`RouteReceipt`].
    #[must_use]
    pub fn build(self) -> RouteReceipt {
        RouteReceipt {
            schema_version: "1".to_string(),
            emitted_at: Utc::now(),
            route_kind: self.route_kind,
            target_file: self.target_file,
            description: self.description,
            rationale: self.rationale,
            evidence: self.evidence,
            alternatives_considered: self.alternatives_considered,
            confidence: self.confidence,
            reversible: self.reversible,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_minimal() {
        let receipt = RouteReceiptBuilder::new(
            RouteKind::CorrectnessFix,
            "src/main.rs",
            "Fix off-by-one in loop bound",
            "Loop iterated one extra time causing index-out-of-bounds",
        )
        .build();

        assert_eq!(receipt.schema_version, "1");
        assert_eq!(receipt.route_kind, RouteKind::CorrectnessFix);
        assert_eq!(receipt.target_file, "src/main.rs");
        assert_eq!(receipt.description, "Fix off-by-one in loop bound");
        assert_eq!(
            receipt.rationale,
            "Loop iterated one extra time causing index-out-of-bounds"
        );
        assert!(receipt.evidence.is_empty());
        assert!(receipt.alternatives_considered.is_empty());
        assert!((receipt.confidence - 1.0).abs() < f64::EPSILON);
        assert!(receipt.reversible);
    }

    #[test]
    fn builder_full() {
        let receipt = RouteReceiptBuilder::new(
            RouteKind::SerializeGlobalStateTests,
            "tests/db_tests.rs",
            "Serialize tests that share the global DB connection",
            "Parallel execution causes deadlocks on the single-connection pool",
        )
        .evidence("FAILED tests::db_tests::create_user -- deadlock detected")
        .evidence("FAILED tests::db_tests::delete_user -- connection pool exhausted")
        .alternative(
            "Use a connection-per-test pool",
            "Would require major refactor of the test harness",
        )
        .alternative(
            "Disable parallelism globally",
            "Penalizes unrelated tests that are safe to run in parallel",
        )
        .confidence(0.85)
        .reversible(false)
        .build();

        assert_eq!(receipt.route_kind, RouteKind::SerializeGlobalStateTests);
        assert_eq!(receipt.evidence.len(), 2);
        assert_eq!(receipt.alternatives_considered.len(), 2);
        assert!((receipt.confidence - 0.85).abs() < f64::EPSILON);
        assert!(!receipt.reversible);
        assert_eq!(
            receipt.alternatives_considered[0].description,
            "Use a connection-per-test pool"
        );
        assert_eq!(
            receipt.alternatives_considered[1].reason_rejected,
            "Penalizes unrelated tests that are safe to run in parallel"
        );
    }

    #[test]
    fn builder_evidence_all() {
        let lines = vec![
            "line 1: error at col 5",
            "line 2: error at col 10",
            "line 3: warning",
        ];
        let receipt = RouteReceiptBuilder::new(
            RouteKind::CapabilitySkip,
            "src/wasm.rs",
            "Skip WASM target",
            "WASM target is not yet supported",
        )
        .evidence_all(lines)
        .build();

        assert_eq!(receipt.evidence.len(), 3);
        assert_eq!(receipt.evidence[0], "line 1: error at col 5");
    }

    #[test]
    fn confidence_clamped() {
        let receipt = RouteReceiptBuilder::new(
            RouteKind::DependencyPin,
            "Cargo.toml",
            "Pin serde to 1.0.200",
            "Later versions break our macro usage",
        )
        .confidence(1.5)
        .build();
        assert!((receipt.confidence - 1.0).abs() < f64::EPSILON);

        let receipt2 = RouteReceiptBuilder::new(
            RouteKind::DependencyPin,
            "Cargo.toml",
            "Pin serde to 1.0.200",
            "Later versions break our macro usage",
        )
        .confidence(-0.5)
        .build();
        assert!(receipt2.confidence.abs() < f64::EPSILON);
    }

    #[test]
    fn summary_line_format() {
        let receipt = RouteReceiptBuilder::new(
            RouteKind::TestIsolation,
            "tests/integration/server.rs",
            "Use per-test temp dirs",
            "Shared dir causes flaky failures",
        )
        .build();

        assert_eq!(
            receipt.summary_line(),
            "[test isolation] tests/integration/server.rs: Use per-test temp dirs"
        );
    }

    #[test]
    fn summary_line_custom_kind() {
        let receipt = RouteReceiptBuilder::new(
            RouteKind::Custom("platform workaround".to_string()),
            "src/sys/windows.rs",
            "Add retry loop for file locks",
            "Windows holds brief locks after close",
        )
        .build();

        assert_eq!(
            receipt.summary_line(),
            "[custom: platform workaround] src/sys/windows.rs: Add retry loop for file locks"
        );
    }

    #[test]
    fn to_json_roundtrip() {
        let receipt = RouteReceiptBuilder::new(
            RouteKind::ResourceOptimization,
            "src/cache.rs",
            "Cap in-memory cache at 64 MB",
            "Unbounded cache caused OOM on CI runners with 2 GB RAM",
        )
        .evidence("OOM kill at 1.8 GB RSS")
        .alternative(
            "Use an LRU eviction policy",
            "Adds complexity for marginal benefit vs a hard cap",
        )
        .confidence(0.9)
        .reversible(true)
        .build();

        let json = receipt.to_json().expect("serialization should succeed");

        // Verify it is valid JSON by deserializing back.
        let parsed: RouteReceipt =
            serde_json::from_str(&json).expect("deserialization should succeed");

        assert_eq!(parsed.route_kind, receipt.route_kind);
        assert_eq!(parsed.target_file, receipt.target_file);
        assert_eq!(parsed.description, receipt.description);
        assert_eq!(parsed.rationale, receipt.rationale);
        assert_eq!(parsed.evidence, receipt.evidence);
        assert_eq!(
            parsed.alternatives_considered,
            receipt.alternatives_considered
        );
        assert!((parsed.confidence - receipt.confidence).abs() < f64::EPSILON);
        assert_eq!(parsed.reversible, receipt.reversible);
    }

    #[test]
    fn to_json_is_canonical() {
        let receipt = RouteReceiptBuilder::new(
            RouteKind::DocCorrection,
            "README.md",
            "Fix outdated install command",
            "Users reported 404 from the old URL",
        )
        .build();

        let json1 = receipt.to_json().expect("first serialization");
        let json2 = receipt.to_json().expect("second serialization");

        // JCS canonicalization means identical inputs produce identical outputs.
        assert_eq!(json1, json2);
    }

    #[test]
    fn display_all_route_kinds() {
        // Ensure Display is implemented for every variant and produces non-empty output.
        let kinds = vec![
            RouteKind::SerializeGlobalStateTests,
            RouteKind::AdvisoryPerfOnNonStrictRunner,
            RouteKind::CapabilitySkip,
            RouteKind::DependencyPin,
            RouteKind::CorrectnessFix,
            RouteKind::ResourceOptimization,
            RouteKind::TestIsolation,
            RouteKind::DocCorrection,
            RouteKind::Custom("example".to_string()),
        ];
        for kind in &kinds {
            let display = format!("{kind}");
            assert!(
                !display.is_empty(),
                "Display for {kind:?} should not be empty"
            );
        }
    }

    #[test]
    fn serde_route_kind_variants() {
        // Ensure all named variants serialize to snake_case and round-trip.
        let kinds = vec![
            (
                RouteKind::SerializeGlobalStateTests,
                "\"serialize_global_state_tests\"",
            ),
            (
                RouteKind::AdvisoryPerfOnNonStrictRunner,
                "\"advisory_perf_on_non_strict_runner\"",
            ),
            (RouteKind::CapabilitySkip, "\"capability_skip\""),
            (RouteKind::DependencyPin, "\"dependency_pin\""),
            (RouteKind::CorrectnessFix, "\"correctness_fix\""),
            (RouteKind::ResourceOptimization, "\"resource_optimization\""),
            (RouteKind::TestIsolation, "\"test_isolation\""),
            (RouteKind::DocCorrection, "\"doc_correction\""),
        ];
        for (kind, expected_json) in &kinds {
            let json = serde_json::to_string(kind).unwrap();
            assert_eq!(&json, *expected_json, "mismatch for {kind:?}");
            let parsed: RouteKind = serde_json::from_str(&json).unwrap();
            assert_eq!(&parsed, kind);
        }

        // Custom variant
        let custom = RouteKind::Custom("my reason".to_string());
        let json = serde_json::to_string(&custom).unwrap();
        let parsed: RouteKind = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, custom);
    }
}

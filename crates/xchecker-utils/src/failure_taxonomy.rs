//! Failure taxonomy for CI/test failure classification.
//!
//! This module provides machine-readable types for classifying CI and test
//! failures into well-known categories. These classifications can appear in
//! receipts and dossiers to support automated triage and trend analysis.
//!
//! # Failure Classes
//!
//! | Class | Description |
//! |-------|-------------|
//! | `CorrectnessDefect` | A genuine bug in production or test code |
//! | `GlobalStateFlake` | Non-deterministic failure from shared mutable state |
//! | `EnvironmentExhaustion` | Resource limits (disk, memory, file descriptors) |
//! | `RunnerPerformanceVariance` | Timing-sensitive failures on slow CI runners |
//! | `PlatformCapabilityMismatch` | OS/platform feature unavailable or behaving differently |
//! | `ToolchainDrift` | Compiler, dependency, or tooling version mismatch |
//! | `Unknown` | Not yet classified |
//!
//! # Example
//!
//! ```rust
//! use xchecker_utils::failure_taxonomy::{
//!     ClassifiedFailure, Confidence, FailureClass, FixClass,
//! };
//!
//! let failure = ClassifiedFailure {
//!     class: FailureClass::GlobalStateFlake,
//!     confidence: Confidence::High,
//!     evidence: vec!["test_a and test_b both write to /tmp/shared".into()],
//!     suggested_fix_class: Some(FixClass::SerializeGlobalStateTests),
//!     platform: Some("ubuntu-latest".into()),
//!     test_name: Some("test_concurrent_write".into()),
//!     file_path: Some("crates/xchecker-utils/tests/integration.rs".into()),
//! };
//!
//! assert_eq!(
//!     failure.summary(),
//!     "global_state_flake (high confidence): test_a and test_b both write to /tmp/shared"
//! );
//! ```

use serde::{Deserialize, Serialize};
use std::fmt;

/// Classification of a CI/test failure.
///
/// Each variant maps to a well-known failure mode observed across real CI runs.
/// The `Unknown` variant is used when a failure has not yet been triaged.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureClass {
    /// A genuine bug in production or test code.
    CorrectnessDefect,
    /// Non-deterministic failure caused by shared mutable global state
    /// (e.g., two tests racing on the same temp directory or environment variable).
    GlobalStateFlake,
    /// Failure caused by resource exhaustion (disk space, memory, file descriptors,
    /// process limits) in the CI environment.
    EnvironmentExhaustion,
    /// Timing-sensitive failure that only manifests on slow or overloaded CI runners
    /// (e.g., a 5-second timeout that passes locally but fails on shared infrastructure).
    RunnerPerformanceVariance,
    /// Failure due to an OS or platform capability that is missing or behaves
    /// differently (e.g., Unix signals on Windows, symlink permissions).
    PlatformCapabilityMismatch,
    /// Failure caused by a change in compiler version, dependency version,
    /// or toolchain configuration (e.g., a new Clippy lint, MSRV bump).
    ToolchainDrift,
    /// Not yet classified.
    Unknown,
}

impl FailureClass {
    /// Returns the canonical snake_case string for this class.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::CorrectnessDefect => "correctness_defect",
            Self::GlobalStateFlake => "global_state_flake",
            Self::EnvironmentExhaustion => "environment_exhaustion",
            Self::RunnerPerformanceVariance => "runner_performance_variance",
            Self::PlatformCapabilityMismatch => "platform_capability_mismatch",
            Self::ToolchainDrift => "toolchain_drift",
            Self::Unknown => "unknown",
        }
    }
}

impl fmt::Display for FailureClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Confidence level for a failure classification.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    /// Strong signal (e.g., deterministic repro, clear root cause).
    High,
    /// Moderate signal (e.g., pattern match but no isolated repro).
    Medium,
    /// Weak signal (e.g., heuristic guess, insufficient data).
    Low,
}

impl Confidence {
    /// Returns the canonical snake_case string for this confidence level.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
        }
    }
}

impl fmt::Display for Confidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Suggested remediation strategy for a classified failure.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixClass {
    /// Serialize tests that share global state (e.g., `#[serial_test]`).
    SerializeGlobalStateTests,
    /// Mark timing-sensitive assertions as advisory on non-strict runners.
    AdvisoryPerfOnNonStrictRunner,
    /// Add a capability-gate skip (e.g., `#[cfg(unix)]`, platform check).
    CapabilitySkip,
    /// Update a dependency or pin a toolchain version.
    DependencyUpdate,
    /// Fix the production or test code directly.
    CodeFix,
    /// Reduce resource consumption or increase resource limits.
    ResourceOptimization,
}

impl FixClass {
    /// Returns the canonical snake_case string for this fix class.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::SerializeGlobalStateTests => "serialize_global_state_tests",
            Self::AdvisoryPerfOnNonStrictRunner => "advisory_perf_on_non_strict_runner",
            Self::CapabilitySkip => "capability_skip",
            Self::DependencyUpdate => "dependency_update",
            Self::CodeFix => "code_fix",
            Self::ResourceOptimization => "resource_optimization",
        }
    }
}

impl fmt::Display for FixClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A classified failure with supporting evidence.
///
/// Combines a [`FailureClass`] with contextual metadata so that receipts,
/// dossiers, and dashboards can render actionable triage information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifiedFailure {
    /// The failure classification.
    pub class: FailureClass,
    /// How confident the classification is.
    pub confidence: Confidence,
    /// Human-readable evidence strings supporting the classification.
    pub evidence: Vec<String>,
    /// Suggested remediation strategy, if one applies.
    pub suggested_fix_class: Option<FixClass>,
    /// CI platform or OS where the failure was observed (e.g., `"ubuntu-latest"`).
    pub platform: Option<String>,
    /// Fully qualified test name, if the failure is test-scoped.
    pub test_name: Option<String>,
    /// Source file path associated with the failure, if known.
    pub file_path: Option<String>,
}

impl ClassifiedFailure {
    /// Returns a one-line summary suitable for logs and receipt fields.
    ///
    /// Format: `"<class> (<confidence> confidence): <first evidence line>"`
    ///
    /// If no evidence is provided, the summary omits the colon-delimited suffix.
    ///
    /// # Example
    ///
    /// ```rust
    /// use xchecker_utils::failure_taxonomy::*;
    ///
    /// let f = ClassifiedFailure {
    ///     class: FailureClass::ToolchainDrift,
    ///     confidence: Confidence::Medium,
    ///     evidence: vec!["clippy 0.1.81 introduced new lint".into()],
    ///     suggested_fix_class: Some(FixClass::DependencyUpdate),
    ///     platform: None,
    ///     test_name: None,
    ///     file_path: None,
    /// };
    /// assert_eq!(
    ///     f.summary(),
    ///     "toolchain_drift (medium confidence): clippy 0.1.81 introduced new lint"
    /// );
    /// ```
    #[must_use]
    pub fn summary(&self) -> String {
        match self.evidence.first() {
            Some(first) => {
                format!("{} ({} confidence): {first}", self.class, self.confidence)
            }
            None => {
                format!("{} ({} confidence)", self.class, self.confidence)
            }
        }
    }
}

impl fmt::Display for ClassifiedFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.summary())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── FailureClass ────────────────────────────────────────────────

    #[test]
    fn failure_class_display_matches_as_str() {
        let variants = [
            FailureClass::CorrectnessDefect,
            FailureClass::GlobalStateFlake,
            FailureClass::EnvironmentExhaustion,
            FailureClass::RunnerPerformanceVariance,
            FailureClass::PlatformCapabilityMismatch,
            FailureClass::ToolchainDrift,
            FailureClass::Unknown,
        ];
        for v in &variants {
            assert_eq!(v.to_string(), v.as_str());
        }
    }

    #[test]
    fn failure_class_serde_roundtrip() {
        let original = FailureClass::PlatformCapabilityMismatch;
        let json = serde_json::to_string(&original).unwrap();
        assert_eq!(json, r#""platform_capability_mismatch""#);
        let restored: FailureClass = serde_json::from_str(&json).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn failure_class_all_variants_serde_roundtrip() {
        let variants = [
            (FailureClass::CorrectnessDefect, "\"correctness_defect\""),
            (FailureClass::GlobalStateFlake, "\"global_state_flake\""),
            (
                FailureClass::EnvironmentExhaustion,
                "\"environment_exhaustion\"",
            ),
            (
                FailureClass::RunnerPerformanceVariance,
                "\"runner_performance_variance\"",
            ),
            (
                FailureClass::PlatformCapabilityMismatch,
                "\"platform_capability_mismatch\"",
            ),
            (FailureClass::ToolchainDrift, "\"toolchain_drift\""),
            (FailureClass::Unknown, "\"unknown\""),
        ];
        for (variant, expected_json) in &variants {
            let json = serde_json::to_string(variant).unwrap();
            assert_eq!(
                &json, expected_json,
                "serialization mismatch for {variant:?}"
            );
            let restored: FailureClass = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, &restored, "roundtrip mismatch for {variant:?}");
        }
    }

    // ── Confidence ──────────────────────────────────────────────────

    #[test]
    fn confidence_display_matches_as_str() {
        let variants = [Confidence::High, Confidence::Medium, Confidence::Low];
        for v in &variants {
            assert_eq!(v.to_string(), v.as_str());
        }
    }

    #[test]
    fn confidence_serde_roundtrip() {
        let original = Confidence::Medium;
        let json = serde_json::to_string(&original).unwrap();
        assert_eq!(json, r#""medium""#);
        let restored: Confidence = serde_json::from_str(&json).unwrap();
        assert_eq!(original, restored);
    }

    // ── FixClass ────────────────────────────────────────────────────

    #[test]
    fn fix_class_display_matches_as_str() {
        let variants = [
            FixClass::SerializeGlobalStateTests,
            FixClass::AdvisoryPerfOnNonStrictRunner,
            FixClass::CapabilitySkip,
            FixClass::DependencyUpdate,
            FixClass::CodeFix,
            FixClass::ResourceOptimization,
        ];
        for v in &variants {
            assert_eq!(v.to_string(), v.as_str());
        }
    }

    #[test]
    fn fix_class_serde_roundtrip() {
        let original = FixClass::SerializeGlobalStateTests;
        let json = serde_json::to_string(&original).unwrap();
        assert_eq!(json, r#""serialize_global_state_tests""#);
        let restored: FixClass = serde_json::from_str(&json).unwrap();
        assert_eq!(original, restored);
    }

    // ── ClassifiedFailure ───────────────────────────────────────────

    #[test]
    fn summary_with_evidence() {
        let f = ClassifiedFailure {
            class: FailureClass::GlobalStateFlake,
            confidence: Confidence::High,
            evidence: vec!["test_a and test_b both write to /tmp/shared".into()],
            suggested_fix_class: Some(FixClass::SerializeGlobalStateTests),
            platform: Some("ubuntu-latest".into()),
            test_name: Some("test_concurrent_write".into()),
            file_path: Some("crates/xchecker-utils/tests/integration.rs".into()),
        };
        assert_eq!(
            f.summary(),
            "global_state_flake (high confidence): test_a and test_b both write to /tmp/shared"
        );
    }

    #[test]
    fn summary_without_evidence() {
        let f = ClassifiedFailure {
            class: FailureClass::Unknown,
            confidence: Confidence::Low,
            evidence: vec![],
            suggested_fix_class: None,
            platform: None,
            test_name: None,
            file_path: None,
        };
        assert_eq!(f.summary(), "unknown (low confidence)");
    }

    #[test]
    fn display_delegates_to_summary() {
        let f = ClassifiedFailure {
            class: FailureClass::ToolchainDrift,
            confidence: Confidence::Medium,
            evidence: vec!["clippy 0.1.81 introduced new lint".into()],
            suggested_fix_class: Some(FixClass::DependencyUpdate),
            platform: None,
            test_name: None,
            file_path: None,
        };
        assert_eq!(f.to_string(), f.summary());
    }

    #[test]
    fn classified_failure_serde_roundtrip() {
        let original = ClassifiedFailure {
            class: FailureClass::EnvironmentExhaustion,
            confidence: Confidence::High,
            evidence: vec!["disk usage at 98%".into(), "/tmp ran out of inodes".into()],
            suggested_fix_class: Some(FixClass::ResourceOptimization),
            platform: Some("windows-latest".into()),
            test_name: Some("test_large_packet".into()),
            file_path: Some("crates/xchecker-engine/tests/packet.rs".into()),
        };

        let json = serde_json::to_string_pretty(&original).unwrap();
        let restored: ClassifiedFailure = serde_json::from_str(&json).unwrap();

        assert_eq!(original.class, restored.class);
        assert_eq!(original.confidence, restored.confidence);
        assert_eq!(original.evidence, restored.evidence);
        assert_eq!(original.suggested_fix_class, restored.suggested_fix_class);
        assert_eq!(original.platform, restored.platform);
        assert_eq!(original.test_name, restored.test_name);
        assert_eq!(original.file_path, restored.file_path);
    }

    #[test]
    fn classified_failure_optional_fields_absent() {
        let json = r#"{
            "class": "correctness_defect",
            "confidence": "high",
            "evidence": ["assertion failed in line 42"],
            "suggested_fix_class": null,
            "platform": null,
            "test_name": null,
            "file_path": null
        }"#;
        let f: ClassifiedFailure = serde_json::from_str(json).unwrap();
        assert_eq!(f.class, FailureClass::CorrectnessDefect);
        assert!(f.suggested_fix_class.is_none());
        assert!(f.platform.is_none());
    }

    #[test]
    fn summary_uses_first_evidence_only() {
        let f = ClassifiedFailure {
            class: FailureClass::RunnerPerformanceVariance,
            confidence: Confidence::Medium,
            evidence: vec![
                "timeout after 5s on shared runner".into(),
                "passes locally in 0.8s".into(),
            ],
            suggested_fix_class: Some(FixClass::AdvisoryPerfOnNonStrictRunner),
            platform: Some("macos-latest".into()),
            test_name: None,
            file_path: None,
        };
        assert_eq!(
            f.summary(),
            "runner_performance_variance (medium confidence): timeout after 5s on shared runner"
        );
    }
}

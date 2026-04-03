//! CI dossier output format.
//!
//! A structured artifact that compresses CI failure investigation into a
//! reviewable document. Captures failing lanes, platform clustering, failure
//! classification, evidence, repro commands, and suggested fix categories.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ── Core types ───────────────────────────────────────────────────────────

/// Top-level CI dossier: a structured summary of a CI run's failures.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CiDossier {
    /// Schema version for forward-compatible evolution.
    pub schema_version: String,

    /// When the dossier was emitted (UTC).
    pub emitted_at: DateTime<Utc>,

    /// CI run identifier (e.g. GitHub Actions run ID).
    pub run_id: Option<String>,

    /// URL to the CI run (e.g. `https://github.com/owner/repo/actions/runs/123`).
    pub run_url: Option<String>,

    /// The shared failing lane/step name that triggered investigation, if any.
    pub shared_failing_lane: Option<String>,

    /// Per-platform failures.
    pub failures: Vec<PlatformFailure>,

    /// Reproduction bundle with commands and environment.
    pub repro_bundle: ReproBundle,

    /// Aggregate summary across all failures.
    pub summary: DossierSummary,
}

/// A single platform's failure entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlatformFailure {
    /// Platform identifier (e.g. `ubuntu-latest`, `windows-latest`, `macos-14`).
    pub platform: String,

    /// CI job name.
    pub job_name: String,

    /// Classification of the failure (e.g. `compile_error`, `test_timeout`,
    /// `flaky_test`, `infra`).
    pub failure_class: String,

    /// Confidence in the classification (0.0 to 1.0).
    pub confidence: f64,

    /// Raw evidence: log excerpts, error messages, etc.
    pub evidence: Vec<String>,

    /// Suggested fix category (e.g. `pin_dependency`, `increase_timeout`,
    /// `fix_code`, `retry`).
    pub suggested_fix: String,
}

/// Reproduction bundle: everything needed to reproduce the failures locally.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReproBundle {
    /// Ordered list of commands to reproduce.
    pub commands: Vec<ReproCommand>,

    /// Environment variables to set.
    pub env_vars: Vec<String>,

    /// Caveats or known limitations of the repro steps.
    pub caveats: Vec<String>,
}

/// A single reproduction command.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReproCommand {
    /// Human-readable description of what this command does.
    pub description: String,

    /// The shell command to run.
    pub command: String,

    /// Platform constraint, if any (e.g. `linux`, `windows`, `macos`, or `all`).
    pub platform: String,
}

/// Aggregate summary across all failures in the dossier.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DossierSummary {
    /// Total number of platform failures.
    pub total_failures: usize,

    /// Distinct platforms affected.
    pub platforms_affected: Vec<String>,

    /// Distinct failure classes observed.
    pub failure_classes: Vec<String>,

    /// Overall confidence (minimum across individual failures).
    pub overall_confidence: f64,

    /// Recommended next action.
    pub recommended_action: RecommendedAction,
}

/// What the investigator should do next.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecommendedAction {
    /// Re-run the pipeline; failures are likely transient.
    Rerun,
    /// A code or config fix is required.
    FixRequired,
    /// Failures are ambiguous; a human should review.
    HumanReview,
    /// Failures indicate a systemic issue; escalate.
    Escalate,
}

// ── Constructors ─────────────────────────────────────────────────────────

impl CiDossier {
    /// Current schema version for CI dossier documents.
    pub const SCHEMA_VERSION: &'static str = "1";

    /// Create an empty dossier, optionally seeded with a run ID.
    #[must_use]
    pub fn new_empty(run_id: Option<String>) -> Self {
        Self {
            schema_version: Self::SCHEMA_VERSION.to_string(),
            emitted_at: Utc::now(),
            run_id,
            run_url: None,
            shared_failing_lane: None,
            failures: Vec::new(),
            repro_bundle: ReproBundle {
                commands: Vec::new(),
                env_vars: Vec::new(),
                caveats: Vec::new(),
            },
            summary: DossierSummary {
                total_failures: 0,
                platforms_affected: Vec::new(),
                failure_classes: Vec::new(),
                overall_confidence: 1.0,
                recommended_action: RecommendedAction::HumanReview,
            },
        }
    }
}

// ── Serialisation ────────────────────────────────────────────────────────

impl CiDossier {
    /// Serialize the dossier to canonical JSON (JCS / RFC 8785).
    pub fn to_json(&self) -> Result<String> {
        let value = serde_json::to_value(self)
            .with_context(|| "Failed to serialize CiDossier to JSON value")?;
        let bytes = serde_json_canonicalizer::to_vec(&value)
            .with_context(|| "Failed to canonicalize CiDossier JSON")?;
        String::from_utf8(bytes).with_context(|| "Canonical JSON was not valid UTF-8")
    }

    /// Render the dossier as a human-readable Markdown report.
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let mut md = String::with_capacity(2048);

        // Title
        md.push_str("# CI Dossier\n\n");

        // Metadata table
        md.push_str("| Field | Value |\n|---|---|\n");
        md.push_str(&format!("| Schema version | {} |\n", self.schema_version));
        md.push_str(&format!(
            "| Emitted at | {} |\n",
            self.emitted_at.format("%Y-%m-%dT%H:%M:%SZ")
        ));
        if let Some(ref id) = self.run_id {
            md.push_str(&format!("| Run ID | {id} |\n"));
        }
        if let Some(ref url) = self.run_url {
            md.push_str(&format!("| Run URL | {url} |\n"));
        }
        if let Some(ref lane) = self.shared_failing_lane {
            md.push_str(&format!("| Shared failing lane | {lane} |\n"));
        }
        md.push('\n');

        // Summary section
        md.push_str("## Summary\n\n");
        md.push_str(&format!(
            "- **Total failures:** {}\n",
            self.summary.total_failures
        ));
        md.push_str(&format!(
            "- **Platforms affected:** {}\n",
            if self.summary.platforms_affected.is_empty() {
                "none".to_string()
            } else {
                self.summary.platforms_affected.join(", ")
            }
        ));
        md.push_str(&format!(
            "- **Failure classes:** {}\n",
            if self.summary.failure_classes.is_empty() {
                "none".to_string()
            } else {
                self.summary.failure_classes.join(", ")
            }
        ));
        md.push_str(&format!(
            "- **Overall confidence:** {:.0}%\n",
            self.summary.overall_confidence * 100.0
        ));
        md.push_str(&format!(
            "- **Recommended action:** {}\n\n",
            recommended_action_display(&self.summary.recommended_action)
        ));

        // Failures section
        if !self.failures.is_empty() {
            md.push_str("## Failures\n\n");
            for (i, f) in self.failures.iter().enumerate() {
                md.push_str(&format!(
                    "### {}. {} ({}) \n\n",
                    i + 1,
                    f.job_name,
                    f.platform
                ));
                md.push_str(&format!("- **Failure class:** {}\n", f.failure_class));
                md.push_str(&format!("- **Confidence:** {:.0}%\n", f.confidence * 100.0));
                md.push_str(&format!("- **Suggested fix:** {}\n", f.suggested_fix));

                if !f.evidence.is_empty() {
                    md.push_str("\n<details><summary>Evidence</summary>\n\n```\n");
                    for line in &f.evidence {
                        md.push_str(line);
                        md.push('\n');
                    }
                    md.push_str("```\n\n</details>\n");
                }
                md.push('\n');
            }
        }

        // Repro bundle
        if !self.repro_bundle.commands.is_empty() {
            md.push_str("## Reproduction\n\n");

            if !self.repro_bundle.env_vars.is_empty() {
                md.push_str("**Environment variables:**\n\n```bash\n");
                for var in &self.repro_bundle.env_vars {
                    md.push_str(&format!("export {var}\n"));
                }
                md.push_str("```\n\n");
            }

            md.push_str("**Commands:**\n\n");
            for cmd in &self.repro_bundle.commands {
                md.push_str(&format!(
                    "- `{}` ({}) -- {}\n",
                    cmd.command, cmd.platform, cmd.description
                ));
            }
            md.push('\n');

            if !self.repro_bundle.caveats.is_empty() {
                md.push_str("**Caveats:**\n\n");
                for caveat in &self.repro_bundle.caveats {
                    md.push_str(&format!("- {caveat}\n"));
                }
                md.push('\n');
            }
        }

        md
    }
}

/// Human-friendly label for [`RecommendedAction`].
fn recommended_action_display(action: &RecommendedAction) -> &'static str {
    match action {
        RecommendedAction::Rerun => "Rerun",
        RecommendedAction::FixRequired => "Fix required",
        RecommendedAction::HumanReview => "Human review",
        RecommendedAction::Escalate => "Escalate",
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a small but representative dossier for testing.
    fn sample_dossier() -> CiDossier {
        let mut dossier = CiDossier::new_empty(Some("12345".to_string()));
        dossier.run_url = Some("https://github.com/owner/repo/actions/runs/12345".to_string());
        dossier.shared_failing_lane = Some("test-full".to_string());

        dossier.failures = vec![
            PlatformFailure {
                platform: "ubuntu-latest".to_string(),
                job_name: "test-full (ubuntu)".to_string(),
                failure_class: "test_timeout".to_string(),
                confidence: 0.85,
                evidence: vec![
                    "thread 'engine::timeout' panicked at 'assertion failed'".to_string(),
                    "note: test timed out after 60s".to_string(),
                ],
                suggested_fix: "increase_timeout".to_string(),
            },
            PlatformFailure {
                platform: "windows-latest".to_string(),
                job_name: "test-full (windows)".to_string(),
                failure_class: "flaky_test".to_string(),
                confidence: 0.60,
                evidence: vec!["intermittent EPERM on rename".to_string()],
                suggested_fix: "retry".to_string(),
            },
        ];

        dossier.repro_bundle = ReproBundle {
            commands: vec![ReproCommand {
                description: "Run the failing test suite".to_string(),
                command: "cargo test --workspace --tests -- engine::timeout".to_string(),
                platform: "all".to_string(),
            }],
            env_vars: vec!["RUST_LOG=debug".to_string()],
            caveats: vec!["Windows flake may not reproduce locally".to_string()],
        };

        dossier.summary = DossierSummary {
            total_failures: 2,
            platforms_affected: vec!["ubuntu-latest".to_string(), "windows-latest".to_string()],
            failure_classes: vec!["test_timeout".to_string(), "flaky_test".to_string()],
            overall_confidence: 0.60,
            recommended_action: RecommendedAction::FixRequired,
        };

        dossier
    }

    // -- new_empty --

    #[test]
    fn new_empty_has_schema_version() {
        let d = CiDossier::new_empty(None);
        assert_eq!(d.schema_version, CiDossier::SCHEMA_VERSION);
        assert!(d.failures.is_empty());
        assert_eq!(d.summary.total_failures, 0);
    }

    #[test]
    fn new_empty_preserves_run_id() {
        let d = CiDossier::new_empty(Some("run-99".to_string()));
        assert_eq!(d.run_id.as_deref(), Some("run-99"));
    }

    // -- to_json --

    #[test]
    fn to_json_roundtrips() {
        let dossier = sample_dossier();
        let json = dossier.to_json().expect("serialization should succeed");

        // Must be valid JSON
        let parsed: CiDossier = serde_json::from_str(&json).expect("should deserialize back");

        assert_eq!(parsed.run_id, dossier.run_id);
        assert_eq!(parsed.failures.len(), 2);
        assert_eq!(
            parsed.summary.recommended_action,
            RecommendedAction::FixRequired
        );
    }

    #[test]
    fn to_json_empty_dossier() {
        let d = CiDossier::new_empty(None);
        let json = d.to_json().expect("empty dossier should serialize");
        assert!(json.contains("\"schema_version\""));
    }

    // -- to_markdown --

    #[test]
    fn to_markdown_contains_title() {
        let md = sample_dossier().to_markdown();
        assert!(md.starts_with("# CI Dossier\n"));
    }

    #[test]
    fn to_markdown_contains_summary_section() {
        let md = sample_dossier().to_markdown();
        assert!(md.contains("## Summary"));
        assert!(md.contains("**Total failures:** 2"));
        assert!(md.contains("Fix required"));
    }

    #[test]
    fn to_markdown_lists_failures() {
        let md = sample_dossier().to_markdown();
        assert!(md.contains("test-full (ubuntu)"));
        assert!(md.contains("test-full (windows)"));
        assert!(md.contains("test_timeout"));
        assert!(md.contains("flaky_test"));
    }

    #[test]
    fn to_markdown_includes_evidence() {
        let md = sample_dossier().to_markdown();
        assert!(md.contains("thread 'engine::timeout' panicked"));
        assert!(md.contains("intermittent EPERM on rename"));
    }

    #[test]
    fn to_markdown_includes_repro_bundle() {
        let md = sample_dossier().to_markdown();
        assert!(md.contains("## Reproduction"));
        assert!(md.contains("cargo test --workspace --tests -- engine::timeout"));
        assert!(md.contains("RUST_LOG=debug"));
        assert!(md.contains("Windows flake may not reproduce locally"));
    }

    #[test]
    fn to_markdown_empty_dossier() {
        let d = CiDossier::new_empty(None);
        let md = d.to_markdown();
        assert!(md.contains("# CI Dossier"));
        assert!(md.contains("**Total failures:** 0"));
        // No failures or repro sections for empty dossier
        assert!(!md.contains("## Failures"));
        assert!(!md.contains("## Reproduction"));
    }

    #[test]
    fn recommended_action_serde_roundtrip() {
        for action in [
            RecommendedAction::Rerun,
            RecommendedAction::FixRequired,
            RecommendedAction::HumanReview,
            RecommendedAction::Escalate,
        ] {
            let json = serde_json::to_string(&action).unwrap();
            let back: RecommendedAction = serde_json::from_str(&json).unwrap();
            assert_eq!(back, action);
        }
    }

    #[test]
    fn recommended_action_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&RecommendedAction::FixRequired).unwrap(),
            "\"fix_required\""
        );
        assert_eq!(
            serde_json::to_string(&RecommendedAction::HumanReview).unwrap(),
            "\"human_review\""
        );
    }
}

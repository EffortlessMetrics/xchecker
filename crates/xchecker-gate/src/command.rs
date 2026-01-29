//! Gate command for policy-based spec validation
//!
//! This module provides gate command implementation for evaluating specs
//! against configurable policies to determine if they
//! meet requirements for CI/CD gates.

use camino::Utf8PathBuf;
use std::time::Duration;
use xchecker_receipt::ReceiptManager;

use crate::policy::GatePolicy;
use crate::types::{GateCondition, GateResult};

/// Gate command for policy-based spec validation
///
/// Evaluates a spec against a configurable policy to determine if it
/// meets requirements for CI/CD gates.
pub struct GateCommand {
    spec_id: String,
    policy: GatePolicy,
}

impl GateCommand {
    /// Create a new gate command
    pub fn new(spec_id: String, policy: GatePolicy) -> Self {
        Self { spec_id, policy }
    }

    /// Execute gate evaluation
    pub fn execute(&self) -> anyhow::Result<GateResult> {
        let base_path = crate::paths::spec_root(&self.spec_id);
        let base_path_utf8 = Utf8PathBuf::from_path_buf(base_path)
            .map_err(|_| anyhow::anyhow!("Invalid UTF-8 path"))?;
        let receipt_manager = ReceiptManager::new(&base_path_utf8);

        // Check if spec exists
        if !base_path_utf8.as_path().exists() {
            return Ok(GateResult {
                schema_version: "gate-json.v1".to_string(),
                spec_id: self.spec_id.clone(),
                passed: false,
                summary: format!("Spec '{}' does not exist", self.spec_id),
                conditions: vec![],
                failure_reasons: vec![format!(
                    "Spec directory not found: {}",
                    base_path_utf8.as_str()
                )],
            });
        }

        let mut conditions = Vec::new();
        let mut failure_reasons = Vec::new();

        // Evaluate minimum phase requirement
        let min_phase_passed =
            self.evaluate_min_phase(&receipt_manager, &mut conditions, &mut failure_reasons);

        // Evaluate pending fixups requirement
        let fixups_passed =
            self.evaluate_pending_fixups(&receipt_manager, &mut conditions, &mut failure_reasons);

        // Evaluate phase age requirement
        let age_passed =
            self.evaluate_phase_age(&receipt_manager, &mut conditions, &mut failure_reasons);

        let passed = min_phase_passed && fixups_passed && age_passed;

        let summary = if passed {
            format!("Spec '{}' passed all gate checks", self.spec_id)
        } else {
            format!("Spec '{}' failed gate checks", self.spec_id)
        };

        Ok(GateResult {
            schema_version: "gate-json.v1".to_string(),
            spec_id: self.spec_id.clone(),
            passed,
            summary,
            conditions,
            failure_reasons,
        })
    }

    fn evaluate_min_phase(
        &self,
        receipt_manager: &ReceiptManager,
        conditions: &mut Vec<GateCondition>,
        failure_reasons: &mut Vec<String>,
    ) -> bool {
        let policy_min_phase = self.policy.min_phase.as_ref();
        let spec_latest_phase = receipt_manager
            .list_receipts()
            .ok()
            .and_then(|receipts| receipts.last().map(|r| r.phase.clone()));

        let passed = match (policy_min_phase, spec_latest_phase.clone()) {
            (None, _) => true, // No minimum phase requirement
            (Some(_policy_phase), None) => {
                // No receipts, spec not started
                false
            }
            (Some(policy_phase), Some(spec_phase)) => {
                // Compare phase IDs
                policy_phase.as_str() <= spec_phase.as_str()
            }
        };

        let condition_name = format!(
            "Minimum phase: {}",
            policy_min_phase.map_or("none", |p| p.as_str())
        );
        let description = format!(
            "Spec has completed at least phase '{}'",
            policy_min_phase.map_or("none", |p| p.as_str())
        );

        let actual = spec_latest_phase.clone();
        let expected = policy_min_phase.cloned();

        conditions.push(GateCondition {
            name: condition_name,
            description,
            passed,
            actual: actual.map(|p| p.as_str().to_string()),
            expected: expected.map(|p| p.as_str().to_string()),
        });

        if !passed {
            failure_reasons.push(format!(
                "Spec has not reached minimum required phase '{}'",
                policy_min_phase.map_or("none", |p| p.as_str())
            ));
        }

        passed
    }

    fn evaluate_pending_fixups(
        &self,
        _receipt_manager: &ReceiptManager,
        conditions: &mut Vec<GateCondition>,
        failure_reasons: &mut Vec<String>,
    ) -> bool {
        if !self.policy.fail_on_pending_fixups {
            // Not configured to check pending fixups
            return true;
        }

        let base_path = crate::paths::spec_root(&self.spec_id);
        let pending_fixups = crate::pending_fixups::pending_fixups_for_spec(&base_path);

        let passed = pending_fixups.targets == 0;

        let condition_name = "Pending fixups".to_string();
        let description = "No pending fixups should exist".to_string();

        let actual = Some(format!(
            "{} targets with pending changes",
            pending_fixups.targets
        ));
        let expected = Some("0 targets".to_string());

        conditions.push(GateCondition {
            name: condition_name,
            description,
            passed,
            actual,
            expected,
        });

        if !passed {
            failure_reasons.push(format!(
                "Spec has {} pending fixups",
                pending_fixups.targets
            ));
        }

        passed
    }

    fn evaluate_phase_age(
        &self,
        receipt_manager: &ReceiptManager,
        conditions: &mut Vec<GateCondition>,
        failure_reasons: &mut Vec<String>,
    ) -> bool {
        let max_age = match self.policy.max_phase_age {
            Some(age) => age,
            None => return true, // No age requirement
        };

        let latest_receipt = receipt_manager
            .list_receipts()
            .ok()
            .and_then(|receipts| receipts.last().cloned());

        let passed = match &latest_receipt {
            Some(receipt) => {
                let age = chrono::Utc::now().signed_duration_since(receipt.emitted_at);
                let age_duration = age.to_std().unwrap_or(Duration::MAX);
                age_duration <= max_age
            }
            None => false, // No receipts, can't evaluate age
        };

        let condition_name = format!("Phase age: {}", format_duration(max_age));
        let description = format!(
            "Latest successful phase should be no older than {}",
            format_duration(max_age)
        );

        let actual = latest_receipt.as_ref().map(|r| {
            let age = chrono::Utc::now().signed_duration_since(r.emitted_at);
            let age_duration = age.to_std().unwrap_or(Duration::MAX);
            format!("{} old", format_duration(age_duration))
        });
        let expected = Some(format!("<= {}", format_duration(max_age)));

        conditions.push(GateCondition {
            name: condition_name,
            description,
            passed,
            actual,
            expected,
        });

        if !passed {
            failure_reasons.push(format!(
                "Latest phase is older than maximum allowed age of {}",
                format_duration(max_age)
            ));
        }

        passed
    }
}

/// Format a duration as a human-readable string
fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    if total_seconds >= 86400 {
        let days = total_seconds / 86400;
        format!("{}d", days)
    } else if total_seconds >= 3600 {
        let hours = total_seconds / 3600;
        format!("{}h", hours)
    } else if total_seconds >= 60 {
        let minutes = total_seconds / 60;
        format!("{}m", minutes)
    } else {
        format!("{}s", total_seconds)
    }
}

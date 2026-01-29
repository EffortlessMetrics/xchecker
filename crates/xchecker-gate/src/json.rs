//! JSON emission for gate results
//!
//! This module provides functions for emitting gate evaluation results as JSON.

use anyhow::Context;
use serde::Serialize;
use xchecker_utils::canonicalization::emit_jcs;

use crate::types::{GateCondition, GateResult};

/// Schema version for gate JSON output
pub const GATE_JSON_SCHEMA_VERSION: &str = "gate-json.v1";

/// JSON output wrapper for gate results
///
/// This struct wraps GateResult with additional fields required by the schema:
/// - schema_version: Version identifier for the output format
/// - spec_id: The spec being evaluated
#[derive(Debug, Clone, Serialize)]
pub struct GateJsonOutput {
    /// Schema version identifier
    pub schema_version: String,

    /// Spec ID being evaluated
    pub spec_id: String,

    /// Whether spec passed all gate checks
    pub passed: bool,

    /// Individual conditions evaluated
    pub conditions: Vec<GateCondition>,

    /// Reasons for failure (if any)
    pub failure_reasons: Vec<String>,

    /// Human-readable summary of result
    pub summary: String,
}

impl GateJsonOutput {
    /// Create a new GateJsonOutput from a GateResult and spec_id
    #[must_use]
    pub fn new(result: &GateResult, spec_id: &str) -> Self {
        Self {
            schema_version: GATE_JSON_SCHEMA_VERSION.to_string(),
            spec_id: spec_id.to_string(),
            passed: result.passed,
            conditions: result.conditions.clone(),
            failure_reasons: result.failure_reasons.clone(),
            summary: result.summary.clone(),
        }
    }
}

/// Emit gate result as canonical JSON using JCS (RFC 8785)
///
/// This function emits gate evaluation result in canonical JSON form
/// for deterministic output and stable diffs.
///
/// The output includes schema_version and spec_id fields as required
/// by the gate-json.v1 schema.
pub fn emit_gate_json(result: &GateResult, spec_id: &str) -> anyhow::Result<String> {
    let output = GateJsonOutput::new(result, spec_id);
    emit_jcs(&output).context("Failed to emit gate JSON")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{GateCondition, GateResult};

    #[test]
    fn test_emit_gate_json() {
        let result = GateResult {
            schema_version: "gate-json.v1".to_string(),
            spec_id: "test-spec".to_string(),
            passed: true,
            summary: "Spec passed all checks".to_string(),
            conditions: vec![GateCondition {
                name: "Test condition".to_string(),
                description: "A test condition".to_string(),
                passed: true,
                actual: Some("actual".to_string()),
                expected: Some("expected".to_string()),
            }],
            failure_reasons: vec![],
        };

        let json = emit_gate_json(&result, "test-spec");
        assert!(json.is_ok());

        let json_str = json.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed["schema_version"], "gate-json.v1");
        assert_eq!(parsed["spec_id"], "test-spec");
        assert_eq!(parsed["passed"], true);
        assert_eq!(parsed["summary"], "Spec passed all checks");
        assert_eq!(parsed["conditions"].as_array().unwrap().len(), 1);
        assert!(parsed["failure_reasons"].as_array().unwrap().is_empty());
    }
}

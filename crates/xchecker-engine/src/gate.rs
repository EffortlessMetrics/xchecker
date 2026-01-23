//! Gate command for policy-based spec validation
//!
//! This module provides the gate command functionality for evaluating
//! spec status against configurable policies. Used for CI/CD integration
//! to gate merges on xchecker receipts/status.
//!
//! Requirements:
//! - 4.5.1: `xchecker gate <spec-id>` reads latest status + relevant receipts
//! - 4.5.2: Exit codes: 0 on policy success, 1 on policy violation
//! - 4.5.3: Policy parameters: --min-phase, --fail-on-pending-fixups, --max-phase-age
//! - 4.5.4: Human-friendly output and optional --json flag

use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::orchestrator::OrchestratorHandle;
use crate::types::{PhaseId, Receipt};

/// Exit codes for gate command
pub mod exit_codes {
    /// Policy passed - all conditions met
    #[allow(dead_code)] // Reserved for policy configuration; used in tests
    pub const POLICY_PASS: i32 = 0;
    /// Policy violation - one or more conditions not met
    pub const POLICY_VIOLATION: i32 = 1;
}

/// Gate policy configuration
#[derive(Debug, Clone)]
pub struct GatePolicy {
    /// Minimum phase that must be completed (default: tasks)
    pub min_phase: PhaseId,
    /// Fail if any pending fixups exist
    pub fail_on_pending_fixups: bool,
    /// Maximum age of the latest successful phase run
    pub max_phase_age: Option<Duration>,
}

impl GatePolicy {
    /// Apply policy overrides (from policy file) to the current policy.
    #[must_use]
    pub fn apply_overrides(mut self, overrides: GatePolicyOverrides) -> Self {
        if let Some(min_phase) = overrides.min_phase {
            self.min_phase = min_phase;
        }
        if let Some(fail_on_pending_fixups) = overrides.fail_on_pending_fixups {
            self.fail_on_pending_fixups = fail_on_pending_fixups;
        }
        if let Some(max_phase_age) = overrides.max_phase_age {
            self.max_phase_age = Some(max_phase_age);
        }
        self
    }
}

impl Default for GatePolicy {
    fn default() -> Self {
        Self {
            min_phase: PhaseId::Tasks,
            fail_on_pending_fixups: false,
            max_phase_age: None,
        }
    }
}

#[derive(Debug, Default)]
pub struct GatePolicyOverrides {
    min_phase: Option<PhaseId>,
    fail_on_pending_fixups: Option<bool>,
    max_phase_age: Option<Duration>,
}

#[derive(Debug, Deserialize)]
struct PolicyFile {
    gate: Option<GatePolicyFile>,
}

#[derive(Debug, Deserialize)]
struct GatePolicyFile {
    require_phase: Option<String>,
    min_phase: Option<String>,
    allow_fixups: Option<bool>,
    fail_on_pending_fixups: Option<bool>,
    max_age_days: Option<i64>,
    max_phase_age: Option<String>,
}

/// Resolve policy file path from CLI override, repo-local policy, or global config.
pub fn resolve_policy_path(explicit: Option<&Path>) -> Result<Option<PathBuf>> {
    if let Some(path) = explicit {
        if !path.exists() {
            anyhow::bail!("Policy file not found: {}", path.display());
        }
        return Ok(Some(path.to_path_buf()));
    }

    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    if let Some(path) = discover_policy_file_from(&cwd)? {
        return Ok(Some(path));
    }

    if let Some(path) = global_policy_path()
        && path.exists()
    {
        return Ok(Some(path));
    }

    Ok(None)
}

/// Load gate policy configuration from a TOML policy file.
pub fn load_policy_from_path(path: &Path) -> Result<GatePolicy> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read policy file: {}", path.display()))?;
    let parsed: PolicyFile = toml::from_str(&content)
        .with_context(|| format!("Failed to parse policy file: {}", path.display()))?;

    let overrides = if let Some(gate) = parsed.gate {
        parse_policy_overrides(&gate)
            .with_context(|| format!("Invalid gate policy in {}", path.display()))?
    } else {
        GatePolicyOverrides::default()
    };

    Ok(GatePolicy::default().apply_overrides(overrides))
}

fn parse_policy_overrides(gate: &GatePolicyFile) -> Result<GatePolicyOverrides> {
    let mut overrides = GatePolicyOverrides::default();

    let phase_value = gate.require_phase.as_ref().or(gate.min_phase.as_ref());
    if let Some(phase_str) = phase_value {
        overrides.min_phase = Some(parse_phase(phase_str)?);
    }

    // Validate that allow_fixups and fail_on_pending_fixups are not both set
    // These are inverse representations of the same policy; specifying both is ambiguous
    if gate.allow_fixups.is_some() && gate.fail_on_pending_fixups.is_some() {
        anyhow::bail!(
            "[gate] cannot specify both allow_fixups and fail_on_pending_fixups; \
             use fail_on_pending_fixups (canonical) or allow_fixups (legacy alias), not both"
        );
    }

    // fail_on_pending_fixups is the canonical field; allow_fixups is a legacy alias
    if let Some(fail_on_pending_fixups) = gate.fail_on_pending_fixups {
        overrides.fail_on_pending_fixups = Some(fail_on_pending_fixups);
    } else if let Some(allow_fixups) = gate.allow_fixups {
        overrides.fail_on_pending_fixups = Some(!allow_fixups);
    }

    if let Some(max_phase_age) = gate.max_phase_age.as_ref() {
        overrides.max_phase_age = Some(parse_duration(max_phase_age)?);
    } else if let Some(max_age_days) = gate.max_age_days {
        if max_age_days < 0 {
            anyhow::bail!("max_age_days must be >= 0");
        }
        overrides.max_phase_age = Some(Duration::days(max_age_days));
    }

    Ok(overrides)
}

fn discover_policy_file_from(start_dir: &Path) -> Result<Option<PathBuf>> {
    let mut current_dir = start_dir.to_path_buf();

    loop {
        let policy_path = current_dir.join(".xchecker").join("policy.toml");
        if policy_path.exists() {
            return Ok(Some(policy_path));
        }

        if current_dir.parent().is_none() {
            break;
        }

        if current_dir.join(".git").exists()
            || current_dir.join(".hg").exists()
            || current_dir.join(".svn").exists()
        {
            break;
        }

        current_dir = current_dir.parent().unwrap().to_path_buf();
    }

    Ok(None)
}

fn global_policy_path() -> Option<PathBuf> {
    if cfg!(windows)
        && let Some(appdata) = std::env::var_os("APPDATA")
    {
        let mut path = PathBuf::from(appdata);
        path.push("xchecker");
        path.push("policy.toml");
        return Some(path);
    }

    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        let mut path = PathBuf::from(xdg);
        path.push("xchecker");
        path.push("policy.toml");
        return Some(path);
    }

    if let Some(home) = std::env::var_os("HOME") {
        let mut path = PathBuf::from(home);
        path.push(".config");
        path.push("xchecker");
        path.push("policy.toml");
        return Some(path);
    }

    None
}

/// Result of gate policy evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResult {
    /// Whether the policy passed
    pub passed: bool,
    /// Spec ID that was evaluated
    pub spec_id: String,
    /// List of evaluated conditions
    pub conditions: Vec<GateCondition>,
    /// List of reasons for failure (empty if passed)
    pub failure_reasons: Vec<String>,
    /// Summary message
    pub summary: String,
}

/// A single evaluated condition in the gate policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateCondition {
    /// Name of the condition
    pub name: String,
    /// Whether this condition passed
    pub passed: bool,
    /// Description of what was checked
    pub description: String,
    /// Actual value found (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual: Option<String>,
    /// Expected value (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
}

/// JSON output structure for gate command (schema gate-json.v1)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateJsonOutput {
    /// Schema version for this gate output format
    pub schema_version: String,
    /// Spec ID that was evaluated
    pub spec_id: String,
    /// Whether the policy passed
    pub passed: bool,
    /// List of evaluated conditions
    pub conditions: Vec<GateCondition>,
    /// List of reasons for failure (empty if passed)
    pub failure_reasons: Vec<String>,
    /// Summary message
    pub summary: String,
}

impl From<GateResult> for GateJsonOutput {
    fn from(result: GateResult) -> Self {
        Self {
            schema_version: "gate-json.v1".to_string(),
            spec_id: result.spec_id,
            passed: result.passed,
            conditions: result.conditions,
            failure_reasons: result.failure_reasons,
            summary: result.summary,
        }
    }
}

/// Gate command executor
pub struct GateCommand {
    spec_id: String,
    policy: GatePolicy,
}

impl GateCommand {
    /// Create a new gate command with the given spec ID and policy
    #[must_use]
    pub fn new(spec_id: String, policy: GatePolicy) -> Self {
        Self { spec_id, policy }
    }

    /// Execute the gate policy evaluation
    pub fn execute(&self) -> Result<GateResult> {
        // Create read-only handle to access managers (no lock needed for gate)
        let handle = OrchestratorHandle::readonly(&self.spec_id)
            .with_context(|| format!("Failed to create orchestrator for spec: {}", self.spec_id))?;

        // Check if spec directory exists
        let base_path = handle.artifact_manager().base_path();
        if !base_path.exists() {
            return Ok(GateResult {
                passed: false,
                spec_id: self.spec_id.clone(),
                conditions: vec![GateCondition {
                    name: "spec_exists".to_string(),
                    passed: false,
                    description: "Spec directory must exist".to_string(),
                    actual: Some("not found".to_string()),
                    expected: Some("exists".to_string()),
                }],
                failure_reasons: vec![format!("Spec '{}' does not exist", self.spec_id)],
                summary: format!("Gate FAILED: Spec '{}' not found", self.spec_id),
            });
        }

        let mut conditions = Vec::new();
        let mut failure_reasons = Vec::new();

        // Get receipts for evaluation
        let receipts = handle.receipt_manager().list_receipts().unwrap_or_default();

        // Evaluate min-phase condition
        let min_phase_result = self.evaluate_min_phase(&handle, &receipts);
        if !min_phase_result.passed {
            failure_reasons.push(min_phase_result.description.clone());
        }
        conditions.push(min_phase_result);

        // Evaluate pending fixups condition (if enabled)
        if self.policy.fail_on_pending_fixups {
            let fixups_result = self.evaluate_pending_fixups(&handle);
            if !fixups_result.passed {
                failure_reasons.push(fixups_result.description.clone());
            }
            conditions.push(fixups_result);
        }

        // Evaluate max-phase-age condition (if configured)
        if let Some(max_age) = self.policy.max_phase_age {
            let age_result = self.evaluate_phase_age(&receipts, max_age);
            if !age_result.passed {
                failure_reasons.push(age_result.description.clone());
            }
            conditions.push(age_result);
        }

        // Determine overall result
        let passed = failure_reasons.is_empty();
        let summary = if passed {
            format!(
                "Gate PASSED: Spec '{}' meets all policy requirements",
                self.spec_id
            )
        } else {
            format!(
                "Gate FAILED: Spec '{}' has {} policy violation(s)",
                self.spec_id,
                failure_reasons.len()
            )
        };

        Ok(GateResult {
            passed,
            spec_id: self.spec_id.clone(),
            conditions,
            failure_reasons,
            summary,
        })
    }

    /// Evaluate the minimum phase requirement
    fn evaluate_min_phase(
        &self,
        handle: &OrchestratorHandle,
        receipts: &[Receipt],
    ) -> GateCondition {
        let required_phase = self.policy.min_phase;

        // Check if the required phase has a successful receipt
        let phase_str = required_phase.as_str();
        let has_successful_receipt = receipts
            .iter()
            .any(|r| r.phase == phase_str && r.exit_code == 0);

        // Also check if artifacts exist for the phase
        let phase_completed = handle.artifact_manager().phase_completed(required_phase);

        let passed = has_successful_receipt || phase_completed;
        let actual = if passed {
            format!("{} completed", phase_str)
        } else {
            // Find the latest completed phase
            let latest = self.find_latest_successful_phase(receipts);
            match latest {
                Some(phase) => format!("latest: {}", phase),
                None => "no phases completed".to_string(),
            }
        };

        GateCondition {
            name: "min_phase".to_string(),
            passed,
            description: if passed {
                format!("Required phase '{}' is completed", phase_str)
            } else {
                format!("Required phase '{}' not completed ({})", phase_str, actual)
            },
            actual: Some(actual),
            expected: Some(format!("{} or later", phase_str)),
        }
    }

    /// Find the latest successful phase from receipts
    fn find_latest_successful_phase(&self, receipts: &[Receipt]) -> Option<String> {
        let phase_order = [
            PhaseId::Requirements,
            PhaseId::Design,
            PhaseId::Tasks,
            PhaseId::Review,
            PhaseId::Fixup,
            PhaseId::Final,
        ];

        // Find the highest phase with a successful receipt
        let mut latest: Option<&str> = None;
        for phase in &phase_order {
            let phase_str = phase.as_str();
            if receipts
                .iter()
                .any(|r| r.phase == phase_str && r.exit_code == 0)
            {
                latest = Some(phase_str);
            }
        }
        latest.map(String::from)
    }

    /// Evaluate the pending fixups condition
    fn evaluate_pending_fixups(&self, handle: &OrchestratorHandle) -> GateCondition {
        use crate::fixup::PendingFixupsResult;

        let result = crate::fixup::pending_fixups_result_from_handle(handle);

        match result {
            PendingFixupsResult::None => GateCondition {
                name: "pending_fixups".to_string(),
                passed: true,
                description: "No pending fixups".to_string(),
                actual: Some("0".to_string()),
                expected: Some("0".to_string()),
            },
            PendingFixupsResult::Some(stats) => GateCondition {
                name: "pending_fixups".to_string(),
                passed: false,
                description: format!("{} pending fixup(s) found", stats.targets),
                actual: Some(stats.targets.to_string()),
                expected: Some("0".to_string()),
            },
            PendingFixupsResult::Unknown { reason } => {
                // Treat unknown/error state conservatively as failure
                // This prevents gates from passing with corrupted review artifacts
                GateCondition {
                    name: "pending_fixups".to_string(),
                    passed: false,
                    description: format!("Unable to determine pending fixups: {}", reason),
                    actual: Some("unknown".to_string()),
                    expected: Some("0".to_string()),
                }
            }
        }
    }

    /// Evaluate the phase age condition
    ///
    /// Phase age is defined as wall-clock time since the latest SUCCESSFUL receipt
    /// for the min_phase. Failed receipts do not count towards age.
    fn evaluate_phase_age(&self, receipts: &[Receipt], max_age: Duration) -> GateCondition {
        let phase_str = self.policy.min_phase.as_str();

        // Find the latest SUCCESSFUL receipt for the required phase
        let latest_successful = receipts
            .iter()
            .filter(|r| r.phase == phase_str && r.exit_code == 0)
            .max_by_key(|r| r.emitted_at);

        match latest_successful {
            Some(receipt) => {
                let age = Utc::now() - receipt.emitted_at;
                let passed = age <= max_age;

                let age_str = format_duration(age);
                let max_age_str = format_duration(max_age);

                GateCondition {
                    name: "max_phase_age".to_string(),
                    passed,
                    description: if passed {
                        format!("Phase '{}' is fresh (age: {})", phase_str, age_str)
                    } else {
                        format!(
                            "Phase '{}' is stale (age: {}, max: {})",
                            phase_str, age_str, max_age_str
                        )
                    },
                    actual: Some(age_str),
                    expected: Some(format!("<= {}", max_age_str)),
                }
            }
            None => GateCondition {
                name: "max_phase_age".to_string(),
                passed: false,
                description: format!("No successful receipt found for phase '{}'", phase_str),
                actual: Some("no successful receipt".to_string()),
                expected: Some(format!("<= {}", format_duration(max_age))),
            },
        }
    }
}

/// Format a duration in a human-readable way
fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.num_seconds();

    if total_seconds < 60 {
        format!("{}s", total_seconds)
    } else if total_seconds < 3600 {
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;
        if seconds == 0 {
            format!("{}m", minutes)
        } else {
            format!("{}m {}s", minutes, seconds)
        }
    } else if total_seconds < 86400 {
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        if minutes == 0 {
            format!("{}h", hours)
        } else {
            format!("{}h {}m", hours, minutes)
        }
    } else {
        let days = total_seconds / 86400;
        let hours = (total_seconds % 86400) / 3600;
        if hours == 0 {
            format!("{}d", days)
        } else {
            format!("{}d {}h", days, hours)
        }
    }
}

/// Parse a duration string (e.g., "7d", "24h", "30m")
pub fn parse_duration(s: &str) -> Result<Duration> {
    let s = s.trim();
    if s.is_empty() {
        anyhow::bail!("Empty duration string");
    }

    // Try to parse as a number with unit suffix
    let (num_str, unit) = if let Some(stripped) = s.strip_suffix('d') {
        (stripped, "d")
    } else if let Some(stripped) = s.strip_suffix('h') {
        (stripped, "h")
    } else if let Some(stripped) = s.strip_suffix('m') {
        (stripped, "m")
    } else if let Some(stripped) = s.strip_suffix('s') {
        (stripped, "s")
    } else {
        // Assume days if no unit
        (s, "d")
    };

    let num: i64 = num_str
        .parse()
        .with_context(|| format!("Invalid duration number: '{}'", num_str))?;

    let duration = match unit {
        "d" => Duration::days(num),
        "h" => Duration::hours(num),
        "m" => Duration::minutes(num),
        "s" => Duration::seconds(num),
        _ => anyhow::bail!("Unknown duration unit: '{}'", unit),
    };

    Ok(duration)
}

/// Parse a phase name string to PhaseId
pub fn parse_phase(s: &str) -> Result<PhaseId> {
    match s.to_lowercase().as_str() {
        "requirements" => Ok(PhaseId::Requirements),
        "design" => Ok(PhaseId::Design),
        "tasks" => Ok(PhaseId::Tasks),
        "review" => Ok(PhaseId::Review),
        "fixup" => Ok(PhaseId::Fixup),
        "final" => Ok(PhaseId::Final),
        _ => anyhow::bail!(
            "Unknown phase '{}'. Valid phases: requirements, design, tasks, review, fixup, final",
            s
        ),
    }
}

/// Emit gate result as canonical JSON using JCS (RFC 8785)
pub fn emit_gate_json(result: &GateResult) -> Result<String> {
    let output: GateJsonOutput = result.clone().into();

    // Serialize to JSON value
    let json_value =
        serde_json::to_value(&output).context("Failed to serialize gate output to JSON value")?;

    // Apply JCS canonicalization for stable output
    let json_bytes = serde_json_canonicalizer::to_vec(&json_value)
        .context("Failed to canonicalize gate JSON")?;

    let json_string = String::from_utf8(json_bytes)
        .context("Failed to convert canonical JSON to UTF-8 string")?;

    Ok(json_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_default_policy() {
        let policy = GatePolicy::default();
        assert_eq!(policy.min_phase, PhaseId::Tasks);
        assert!(!policy.fail_on_pending_fixups);
        assert!(policy.max_phase_age.is_none());
    }

    #[test]
    fn test_parse_duration_days() {
        let duration = parse_duration("7d").unwrap();
        assert_eq!(duration.num_days(), 7);
    }

    #[test]
    fn test_load_policy_from_path_overrides_defaults() {
        let temp_dir = TempDir::new().unwrap();
        let policy_path = temp_dir.path().join("policy.toml");

        let content = r#"
[gate]
require_phase = "design"
allow_fixups = false
max_age_days = 7
"#;

        fs::write(&policy_path, content).unwrap();

        let policy = load_policy_from_path(&policy_path).unwrap();

        assert_eq!(policy.min_phase, PhaseId::Design);
        assert!(policy.fail_on_pending_fixups);
        assert_eq!(policy.max_phase_age, Some(Duration::days(7)));
    }

    #[test]
    fn test_load_policy_rejects_conflicting_fixup_options() {
        let temp_dir = TempDir::new().unwrap();
        let policy_path = temp_dir.path().join("policy.toml");

        // Both allow_fixups and fail_on_pending_fixups set - should error
        let content = r#"
[gate]
allow_fixups = true
fail_on_pending_fixups = true
"#;

        fs::write(&policy_path, content).unwrap();

        let result = load_policy_from_path(&policy_path);
        assert!(result.is_err(), "Should reject conflicting fixup options");

        // Check the full error chain for the expected message
        let err = result.unwrap_err();
        let err_chain = format!("{:#}", err);
        assert!(
            err_chain.contains("cannot specify both"),
            "Error chain should mention conflict: {}",
            err_chain
        );
    }

    #[test]
    fn test_discover_policy_file_from_repo() {
        let temp_dir = TempDir::new().unwrap();
        let policy_dir = temp_dir.path().join(".xchecker");
        let policy_path = policy_dir.join("policy.toml");
        let nested_dir = temp_dir.path().join("nested").join("child");

        fs::create_dir_all(&policy_dir).unwrap();
        fs::create_dir_all(&nested_dir).unwrap();
        fs::write(&policy_path, "[gate]\nrequire_phase = \"tasks\"\n").unwrap();

        let found = discover_policy_file_from(&nested_dir).unwrap();

        assert_eq!(found, Some(policy_path));
    }

    #[test]
    fn test_parse_duration_hours() {
        let duration = parse_duration("24h").unwrap();
        assert_eq!(duration.num_hours(), 24);
    }

    #[test]
    fn test_parse_duration_minutes() {
        let duration = parse_duration("30m").unwrap();
        assert_eq!(duration.num_minutes(), 30);
    }

    #[test]
    fn test_parse_duration_seconds() {
        let duration = parse_duration("60s").unwrap();
        assert_eq!(duration.num_seconds(), 60);
    }

    #[test]
    fn test_parse_duration_no_unit() {
        // Default to days
        let duration = parse_duration("7").unwrap();
        assert_eq!(duration.num_days(), 7);
    }

    #[test]
    fn test_parse_duration_invalid() {
        assert!(parse_duration("").is_err());
        assert!(parse_duration("abc").is_err());
        assert!(parse_duration("7x").is_err());
    }

    #[test]
    fn test_parse_phase() {
        assert_eq!(parse_phase("requirements").unwrap(), PhaseId::Requirements);
        assert_eq!(parse_phase("design").unwrap(), PhaseId::Design);
        assert_eq!(parse_phase("tasks").unwrap(), PhaseId::Tasks);
        assert_eq!(parse_phase("review").unwrap(), PhaseId::Review);
        assert_eq!(parse_phase("fixup").unwrap(), PhaseId::Fixup);
        assert_eq!(parse_phase("final").unwrap(), PhaseId::Final);

        // Case insensitive
        assert_eq!(parse_phase("REQUIREMENTS").unwrap(), PhaseId::Requirements);
        assert_eq!(parse_phase("Design").unwrap(), PhaseId::Design);
    }

    #[test]
    fn test_parse_phase_invalid() {
        assert!(parse_phase("invalid").is_err());
        assert!(parse_phase("").is_err());
    }

    #[test]
    fn test_format_duration_seconds() {
        assert_eq!(format_duration(Duration::seconds(30)), "30s");
        assert_eq!(format_duration(Duration::seconds(59)), "59s");
    }

    #[test]
    fn test_format_duration_minutes() {
        assert_eq!(format_duration(Duration::minutes(5)), "5m");
        assert_eq!(format_duration(Duration::seconds(90)), "1m 30s");
    }

    #[test]
    fn test_format_duration_hours() {
        assert_eq!(format_duration(Duration::hours(2)), "2h");
        assert_eq!(format_duration(Duration::minutes(90)), "1h 30m");
    }

    #[test]
    fn test_format_duration_days() {
        assert_eq!(format_duration(Duration::days(7)), "7d");
        assert_eq!(format_duration(Duration::hours(36)), "1d 12h");
    }

    #[test]
    fn test_gate_result_to_json() {
        let result = GateResult {
            passed: true,
            spec_id: "test-spec".to_string(),
            conditions: vec![GateCondition {
                name: "min_phase".to_string(),
                passed: true,
                description: "Required phase 'tasks' is completed".to_string(),
                actual: Some("tasks completed".to_string()),
                expected: Some("tasks or later".to_string()),
            }],
            failure_reasons: vec![],
            summary: "Gate PASSED: Spec 'test-spec' meets all policy requirements".to_string(),
        };

        let json = emit_gate_json(&result).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["schema_version"], "gate-json.v1");
        assert_eq!(parsed["spec_id"], "test-spec");
        assert_eq!(parsed["passed"], true);
    }

    #[test]
    fn test_gate_nonexistent_spec() {
        let _temp_dir = crate::paths::with_isolated_home();

        let policy = GatePolicy::default();
        let gate = GateCommand::new("nonexistent-spec".to_string(), policy);

        let result = gate.execute().unwrap();

        assert!(!result.passed);
        assert!(
            result
                .failure_reasons
                .iter()
                .any(|r| r.contains("does not exist"))
        );
    }
}

// ===== Gate Exit Code Tests (Task 33.3) =====
// **Property: Gate returns correct exit codes**
// **Validates: Requirements 4.5.2**

#[test]
fn test_gate_exit_code_policy_pass() {
    // Verify POLICY_PASS is 0
    assert_eq!(exit_codes::POLICY_PASS, 0);
}

#[test]
fn test_gate_exit_code_policy_violation() {
    // Verify POLICY_VIOLATION is 1
    assert_eq!(exit_codes::POLICY_VIOLATION, 1);
}

#[test]
fn test_gate_result_passed_has_no_failures() {
    let result = GateResult {
        passed: true,
        spec_id: "test-spec".to_string(),
        conditions: vec![GateCondition {
            name: "min_phase".to_string(),
            passed: true,
            description: "Required phase 'tasks' is completed".to_string(),
            actual: Some("tasks completed".to_string()),
            expected: Some("tasks or later".to_string()),
        }],
        failure_reasons: vec![],
        summary: "Gate PASSED".to_string(),
    };

    assert!(result.passed);
    assert!(result.failure_reasons.is_empty());
}

#[test]
fn test_gate_result_failed_has_failures() {
    let result = GateResult {
        passed: false,
        spec_id: "test-spec".to_string(),
        conditions: vec![GateCondition {
            name: "min_phase".to_string(),
            passed: false,
            description: "Required phase 'tasks' not completed".to_string(),
            actual: Some("latest: requirements".to_string()),
            expected: Some("tasks or later".to_string()),
        }],
        failure_reasons: vec!["Required phase 'tasks' not completed".to_string()],
        summary: "Gate FAILED".to_string(),
    };

    assert!(!result.passed);
    assert!(!result.failure_reasons.is_empty());
}

#[test]
fn test_gate_stale_spec_with_recent_failure() {
    // Test that a spec with success 10 days ago and failure yesterday is still stale
    // per max-phase-age. Failed receipts do not count towards age.
    use crate::types::{PacketEvidence, Receipt};

    let _temp_dir = crate::paths::with_isolated_home();

    // Create a spec with receipts
    let spec_id = "test-stale-spec";
    let base_path = crate::paths::spec_root(spec_id);
    crate::paths::ensure_dir_all(base_path.join("receipts")).unwrap();

    // Create a successful receipt from 10 days ago
    let old_success_time = Utc::now() - Duration::days(10);
    let old_receipt = Receipt {
        schema_version: "1".to_string(),
        emitted_at: old_success_time,
        spec_id: spec_id.to_string(),
        phase: "tasks".to_string(),
        xchecker_version: "1.0.0".to_string(),
        claude_cli_version: "0.8.1".to_string(),
        model_full_name: "haiku".to_string(),
        model_alias: None,
        canonicalization_version: "yaml-v1,md-v1".to_string(),
        canonicalization_backend: "jcs-rfc8785".to_string(),
        flags: std::collections::HashMap::new(),
        runner: "native".to_string(),
        runner_distro: None,
        packet: PacketEvidence {
            files: vec![],
            max_bytes: 65536,
            max_lines: 1200,
        },
        outputs: vec![],
        exit_code: 0, // SUCCESS
        error_kind: None,
        error_reason: None,
        stderr_tail: None,
        stderr_redacted: None,
        warnings: vec![],
        fallback_used: None,
        diff_context: None,
        llm: None,
        pipeline: None,
    };

    // Create a failed receipt from yesterday
    let recent_failure_time = Utc::now() - Duration::days(1);
    let recent_receipt = Receipt {
        schema_version: "1".to_string(),
        emitted_at: recent_failure_time,
        spec_id: spec_id.to_string(),
        phase: "tasks".to_string(),
        xchecker_version: "1.0.0".to_string(),
        claude_cli_version: "0.8.1".to_string(),
        model_full_name: "haiku".to_string(),
        model_alias: None,
        canonicalization_version: "yaml-v1,md-v1".to_string(),
        canonicalization_backend: "jcs-rfc8785".to_string(),
        flags: std::collections::HashMap::new(),
        runner: "native".to_string(),
        runner_distro: None,
        packet: PacketEvidence {
            files: vec![],
            max_bytes: 65536,
            max_lines: 1200,
        },
        outputs: vec![],
        exit_code: 1, // FAILURE - should not count towards age
        error_kind: None,
        error_reason: Some("Test failure".to_string()),
        stderr_tail: None,
        stderr_redacted: None,
        warnings: vec![],
        fallback_used: None,
        diff_context: None,
        llm: None,
        pipeline: None,
    };

    // Write receipts
    let _receipt_manager = crate::receipt::ReceiptManager::new(&base_path);

    // Write old success receipt
    let old_json = serde_json::to_string_pretty(&old_receipt).unwrap();
    let old_filename = format!("tasks-{}.json", old_success_time.format("%Y%m%d_%H%M%S"));
    std::fs::write(base_path.join("receipts").join(&old_filename), &old_json).unwrap();

    // Write recent failure receipt
    let recent_json = serde_json::to_string_pretty(&recent_receipt).unwrap();
    let recent_filename = format!("tasks-{}.json", recent_failure_time.format("%Y%m%d_%H%M%S"));
    std::fs::write(
        base_path.join("receipts").join(&recent_filename),
        &recent_json,
    )
    .unwrap();

    // Create artifacts directory so spec is considered to exist
    crate::paths::ensure_dir_all(base_path.join("artifacts")).unwrap();

    // Evaluate gate with max-phase-age of 7 days
    let policy = GatePolicy {
        min_phase: PhaseId::Tasks,
        fail_on_pending_fixups: false,
        max_phase_age: Some(Duration::days(7)),
    };

    let gate = GateCommand::new(spec_id.to_string(), policy);
    let result = gate.execute().unwrap();

    // The gate should FAIL because:
    // - The latest SUCCESSFUL receipt is 10 days old (exceeds 7 day limit)
    // - The recent FAILED receipt does NOT count towards age
    assert!(!result.passed, "Gate should fail for stale spec");
    assert!(
        result.failure_reasons.iter().any(|r| r.contains("stale")),
        "Failure reason should mention staleness: {:?}",
        result.failure_reasons
    );
}

#[test]
fn test_gate_fresh_spec_passes_age_check() {
    use crate::types::{PacketEvidence, Receipt};

    let _temp_dir = crate::paths::with_isolated_home();

    // Create a spec with a recent successful receipt
    let spec_id = "test-fresh-spec";
    let base_path = crate::paths::spec_root(spec_id);
    crate::paths::ensure_dir_all(base_path.join("receipts")).unwrap();
    crate::paths::ensure_dir_all(base_path.join("artifacts")).unwrap();

    // Create a successful receipt from 2 days ago
    let recent_time = Utc::now() - Duration::days(2);
    let receipt = Receipt {
        schema_version: "1".to_string(),
        emitted_at: recent_time,
        spec_id: spec_id.to_string(),
        phase: "tasks".to_string(),
        xchecker_version: "1.0.0".to_string(),
        claude_cli_version: "0.8.1".to_string(),
        model_full_name: "haiku".to_string(),
        model_alias: None,
        canonicalization_version: "yaml-v1,md-v1".to_string(),
        canonicalization_backend: "jcs-rfc8785".to_string(),
        flags: std::collections::HashMap::new(),
        runner: "native".to_string(),
        runner_distro: None,
        packet: PacketEvidence {
            files: vec![],
            max_bytes: 65536,
            max_lines: 1200,
        },
        outputs: vec![],
        exit_code: 0, // SUCCESS
        error_kind: None,
        error_reason: None,
        stderr_tail: None,
        stderr_redacted: None,
        warnings: vec![],
        fallback_used: None,
        diff_context: None,
        llm: None,
        pipeline: None,
    };

    // Write receipt
    let json = serde_json::to_string_pretty(&receipt).unwrap();
    let filename = format!("tasks-{}.json", recent_time.format("%Y%m%d_%H%M%S"));
    std::fs::write(base_path.join("receipts").join(&filename), &json).unwrap();

    // Evaluate gate with max-phase-age of 7 days
    let policy = GatePolicy {
        min_phase: PhaseId::Tasks,
        fail_on_pending_fixups: false,
        max_phase_age: Some(Duration::days(7)),
    };

    let gate = GateCommand::new(spec_id.to_string(), policy);
    let result = gate.execute().unwrap();

    // The gate should PASS because the successful receipt is only 2 days old
    assert!(
        result.passed,
        "Gate should pass for fresh spec: {:?}",
        result.failure_reasons
    );
}

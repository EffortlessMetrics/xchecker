//! Gate policy configuration and parsing
//!
//! This module provides policy types and parsing functions for the gate command.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;
use xchecker_utils::types::PhaseId;

/// Gate policy for spec validation
///
/// Defines the rules that a spec must meet to pass the gate.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GatePolicy {
    /// Minimum phase that must be completed
    #[serde(default)]
    pub min_phase: Option<PhaseId>,

    /// Fail if any pending fixups exist
    #[serde(default)]
    pub fail_on_pending_fixups: bool,

    /// Maximum age of the latest successful phase
    #[serde(default)]
    pub max_phase_age: Option<Duration>,
}

/// Resolve policy path from CLI argument or default locations
///
/// Searches for policy file in the following order:
/// 1. Explicit path provided via --policy flag
/// 2. `.xchecker/policy.toml` in current directory or repo root
/// 3. `~/.config/xchecker/policy.toml`
pub fn resolve_policy_path(policy_path: Option<&Path>) -> Result<Option<PathBuf>> {
    if let Some(path) = policy_path {
        // Explicit path provided
        if path.exists() {
            return Ok(Some(path.to_path_buf()));
        }
        anyhow::bail!("Policy file not found: {}", path.display());
    }

    // Try .xchecker/policy.toml in current directory
    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    let local_policy = cwd.join(".xchecker").join("policy.toml");
    if local_policy.exists() {
        return Ok(Some(local_policy));
    }

    // Try to find repo root and check for .xchecker/policy.toml
    let repo_root = find_repo_root(&cwd)?;
    let repo_policy = repo_root.join(".xchecker").join("policy.toml");
    if repo_policy.exists() {
        return Ok(Some(repo_policy));
    }

    // Try ~/.config/xchecker/policy.toml
    if let Some(config_dir) = dirs::config_dir() {
        let config_policy = config_dir.join("xchecker").join("policy.toml");
        if config_policy.exists() {
            return Ok(Some(config_policy));
        }
    }

    // No policy file found
    Ok(None)
}

/// Find repository root by looking for .git directory
fn find_repo_root(start: &Path) -> Result<PathBuf> {
    let mut current = start.to_path_buf();

    for _ in 0..10 {
        // Check for .git directory
        if current.join(".git").exists() {
            return Ok(current);
        }

        // Move to parent directory
        if !current.pop() {
            break;
        }
    }

    // No .git found, return start directory
    Ok(start.to_path_buf())
}

/// Load policy from a TOML file
pub fn load_policy_from_path(path: &Path) -> Result<GatePolicy> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read policy file: {}", path.display()))?;

    let policy: GatePolicy = toml::from_str(&content)
        .with_context(|| format!("Failed to parse policy TOML: {}", path.display()))?;

    Ok(policy)
}

/// Parse a phase string into a PhaseId
pub fn parse_phase(phase_str: &str) -> Result<PhaseId> {
    match phase_str.to_lowercase().as_str() {
        "requirements" => Ok(PhaseId::Requirements),
        "design" => Ok(PhaseId::Design),
        "tasks" => Ok(PhaseId::Tasks),
        "review" => Ok(PhaseId::Review),
        "fixup" => Ok(PhaseId::Fixup),
        "final" => Ok(PhaseId::Final),
        _ => anyhow::bail!(
            "Unknown phase '{}'. Valid phases: requirements, design, tasks, review, fixup, final",
            phase_str
        ),
    }
}

/// Parse a duration string (e.g., "7d", "24h", "30m")
pub fn parse_duration(duration_str: &str) -> Result<Duration> {
    let duration_str = duration_str.trim().to_lowercase();

    // Parse the numeric part and the unit
    let mut num_str = String::new();
    let mut unit_str = String::new();

    for c in duration_str.chars() {
        if c.is_ascii_digit() || c == '.' {
            num_str.push(c);
        } else {
            unit_str.push(c);
        }
    }

    let value: f64 = num_str
        .parse()
        .with_context(|| format!("Invalid duration value: {}", num_str))?;

    let duration = match unit_str.as_str() {
        "s" | "sec" | "second" | "seconds" => Duration::from_secs_f64(value),
        "m" | "min" | "minute" | "minutes" => Duration::from_secs_f64(value * 60.0),
        "h" | "hour" | "hours" => Duration::from_secs_f64(value * 3600.0),
        "d" | "day" | "days" => Duration::from_secs_f64(value * 86400.0),
        "w" | "week" | "weeks" => Duration::from_secs_f64(value * 604800.0),
        _ => anyhow::bail!(
            "Unknown duration unit '{}'. Valid units: s/m/h/d/w",
            unit_str
        ),
    };

    Ok(duration)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_phase() {
        assert_eq!(parse_phase("requirements").unwrap(), PhaseId::Requirements);
        assert_eq!(parse_phase("design").unwrap(), PhaseId::Design);
        assert_eq!(parse_phase("tasks").unwrap(), PhaseId::Tasks);
        assert_eq!(parse_phase("REVIEW").unwrap(), PhaseId::Review); // Case insensitive
        assert!(parse_phase("invalid").is_err());
    }

    #[test]
    fn test_parse_duration() {
        assert_eq!(
            parse_duration("7d").unwrap(),
            Duration::from_secs(7 * 86400)
        );
        assert_eq!(
            parse_duration("24h").unwrap(),
            Duration::from_secs(24 * 3600)
        );
        assert_eq!(parse_duration("30m").unwrap(), Duration::from_secs(30 * 60));
        assert_eq!(parse_duration("90s").unwrap(), Duration::from_secs(90));
        assert!(parse_duration("invalid").is_err());
    }

    #[test]
    fn test_gate_policy_default() {
        let policy = GatePolicy::default();
        assert!(policy.min_phase.is_none());
        assert!(!policy.fail_on_pending_fixups);
        assert!(policy.max_phase_age.is_none());
    }
}

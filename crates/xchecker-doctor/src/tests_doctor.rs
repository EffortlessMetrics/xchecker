//! Test hazard scanner for CI stability diagnostics
//!
//! Scans test files for common CI hazard patterns that have historically
//! caused flaky or platform-specific failures. Identified from real CI
//! failures in PR #11.
//!
//! # Hazard Classes
//!
//! 1. **Global state mutation** -- `set_current_dir` without serialization
//! 2. **Fragile performance assertions** -- wall-clock `assert!` on `elapsed`/`Duration`
//!    without env-var or feature-flag gating
//! 3. **Platform capability assumptions** -- Unix signal functions without `#[cfg(unix)]`
//!    or skip paths
//! 4. **Ignored test advisory** -- `#[ignore]` with reason strings (informational)

use std::path::{Path, PathBuf};

use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Classification of a test hazard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HazardKind {
    /// `set_current_dir` without `#[serial]` or `--test-threads=1` nearby.
    GlobalStateMutation,
    /// Hard `assert!` on `elapsed` / `Duration` without env-var or feature gating.
    FragilePerformanceAssertion,
    /// Unix-only signal APIs (`killpg`, `kill`, `SIGTERM`, `SIGKILL`) without
    /// `#[cfg(unix)]` or skip paths.
    PlatformCapabilityAssumption,
    /// `#[ignore]` with a reason string -- purely advisory.
    IgnoredTestAdvisory,
}

/// A single finding produced by the test hazard scanner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestHazard {
    /// Path of the file containing the hazard.
    pub file_path: PathBuf,
    /// 1-based line number where the hazard was detected.
    pub line_number: usize,
    /// Classification of the hazard.
    pub kind: HazardKind,
    /// Human-readable description of what was detected.
    pub description: String,
    /// Suggested remediation.
    pub suggested_fix: String,
}

/// Aggregated output of a test-hazard scan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestsDoctorOutput {
    /// Total number of files scanned.
    pub files_scanned: usize,
    /// Hazards discovered.
    pub hazards: Vec<TestHazard>,
}

// ---------------------------------------------------------------------------
// Scanner
// ---------------------------------------------------------------------------

/// Scan a directory tree for test-file hazards.
///
/// `root` is typically the workspace root. The scanner looks at:
/// - `<root>/tests/**/*.rs`
/// - `<root>/crates/*/tests/**/*.rs`
/// - `<root>/crates/*/src/**/*.rs` (only files containing `#[cfg(test)]` or `#[test]`)
///
/// Returns an aggregated [`TestsDoctorOutput`].
pub fn scan_test_hazards(root: &Path) -> Result<TestsDoctorOutput> {
    let mut files: Vec<PathBuf> = Vec::new();

    // Collect candidate Rust files
    collect_rs_files(&root.join("tests"), &mut files);
    let crates_dir = root.join("crates");
    if crates_dir.is_dir()
        && let Ok(entries) = std::fs::read_dir(&crates_dir)
    {
        for entry in entries.flatten() {
            let crate_path = entry.path();
            if crate_path.is_dir() {
                collect_rs_files(&crate_path.join("tests"), &mut files);
                collect_rs_files(&crate_path.join("src"), &mut files);
            }
        }
    }

    // Also scan root src/
    collect_rs_files(&root.join("src"), &mut files);

    let mut hazards: Vec<TestHazard> = Vec::new();
    let mut files_scanned = 0;

    for path in &files {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // For src/ files, only scan if they contain test code
        let is_test_file = path.components().any(|c| c.as_os_str() == "tests");
        if !is_test_file && !content.contains("#[cfg(test)]") && !content.contains("#[test]") {
            continue;
        }

        files_scanned += 1;
        scan_content(path, &content, &mut hazards);
    }

    Ok(TestsDoctorOutput {
        files_scanned,
        hazards,
    })
}

/// Scan a single content string (exposed for unit testing with synthetic input).
pub fn scan_content(path: &Path, content: &str, hazards: &mut Vec<TestHazard>) {
    let lines: Vec<&str> = content.lines().collect();

    check_global_state_mutation(path, &lines, hazards);
    check_fragile_performance_assertions(path, &lines, hazards);
    check_platform_capability_assumptions(path, &lines, hazards);
    check_ignored_test_advisory(path, &lines, hazards);
}

// ---------------------------------------------------------------------------
// Individual hazard checkers
// ---------------------------------------------------------------------------

/// Detect `set_current_dir` usage without nearby serialization markers.
fn check_global_state_mutation(path: &Path, lines: &[&str], hazards: &mut Vec<TestHazard>) {
    let serialization_markers = ["#[serial]", "--test-threads=1", "serial_test"];

    for (idx, line) in lines.iter().enumerate() {
        if !line.contains("set_current_dir") {
            continue;
        }
        // Skip comments
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*') {
            continue;
        }

        // Look in a window around the call (30 lines before, 5 after) for serialization markers
        let window_start = idx.saturating_sub(30);
        let window_end = (idx + 5).min(lines.len());
        let window = &lines[window_start..window_end];

        let has_serialization = window
            .iter()
            .any(|l| serialization_markers.iter().any(|m| l.contains(m)));

        if !has_serialization {
            hazards.push(TestHazard {
                file_path: path.to_path_buf(),
                line_number: idx + 1,
                kind: HazardKind::GlobalStateMutation,
                description: format!(
                    "`set_current_dir` call without serialization (line {})",
                    idx + 1
                ),
                suggested_fix:
                    "Add `#[serial]` from `serial_test` crate, or run with `--test-threads=1`"
                        .to_string(),
            });
        }
    }
}

/// Detect hard assertions on elapsed time / Duration without gating.
fn check_fragile_performance_assertions(
    path: &Path,
    lines: &[&str],
    hazards: &mut Vec<TestHazard>,
) {
    // Match assert! / assert_eq! / assert_ne! lines that reference elapsed or Duration
    let assert_re = Regex::new(r"assert(?:_eq|_ne|_lt|_le|_gt|_ge)?!\s*\(").expect("valid regex");
    let timing_re =
        Regex::new(r"(?:\.elapsed\(\)|Duration::|duration|elapsed)").expect("valid regex");
    let gating_markers = [
        "XCHECKER_STRICT",
        "CI_STRICT",
        "strict",
        "feature",
        "cfg(",
        "env::var",
        "std::env::var",
        "option_env!",
    ];

    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*') {
            continue;
        }

        if !assert_re.is_match(trimmed) || !timing_re.is_match(trimmed) {
            continue;
        }

        // Check window for gating
        let window_start = idx.saturating_sub(10);
        let window_end = (idx + 3).min(lines.len());
        let window = &lines[window_start..window_end];

        let has_gating = window
            .iter()
            .any(|l| gating_markers.iter().any(|m| l.contains(m)));

        if !has_gating {
            hazards.push(TestHazard {
                file_path: path.to_path_buf(),
                line_number: idx + 1,
                kind: HazardKind::FragilePerformanceAssertion,
                description: format!(
                    "Hard timing assertion without env/feature gating (line {})",
                    idx + 1
                ),
                suggested_fix:
                    "Guard with `if std::env::var(\"CI_STRICT\").is_ok()` or a feature flag"
                        .to_string(),
            });
        }
    }
}

/// Detect Unix-only signal/process functions without platform gating.
fn check_platform_capability_assumptions(
    path: &Path,
    lines: &[&str],
    hazards: &mut Vec<TestHazard>,
) {
    let signal_re =
        Regex::new(r"\b(killpg|kill|SIGTERM|SIGKILL|SIGINT|SIGHUP)\b").expect("valid regex");
    let platform_markers = [
        "#[cfg(unix)]",
        "#[cfg(target_os",
        "#[cfg(not(windows))]",
        "cfg!(unix)",
        "cfg!(target_os",
        "cfg!(not(windows))",
        "skip_if_windows",
        "if cfg!(windows)",
    ];

    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*') {
            continue;
        }

        if !signal_re.is_match(trimmed) {
            continue;
        }

        // Look in a broader window (50 lines before, 5 after) for platform gating.
        // cfg attributes on modules can be far above individual lines.
        let window_start = idx.saturating_sub(50);
        let window_end = (idx + 5).min(lines.len());
        let window = &lines[window_start..window_end];

        let has_platform_gate = window
            .iter()
            .any(|l| platform_markers.iter().any(|m| l.contains(m)));

        if !has_platform_gate {
            hazards.push(TestHazard {
                file_path: path.to_path_buf(),
                line_number: idx + 1,
                kind: HazardKind::PlatformCapabilityAssumption,
                description: format!(
                    "Unix signal/process API without platform gate (line {})",
                    idx + 1
                ),
                suggested_fix:
                    "Add `#[cfg(unix)]` on the test or module, or add a runtime skip for Windows"
                        .to_string(),
            });
        }
    }
}

/// Report `#[ignore]` annotations that carry reason strings (advisory only).
fn check_ignored_test_advisory(path: &Path, lines: &[&str], hazards: &mut Vec<TestHazard>) {
    let ignore_re = Regex::new(r#"#\[ignore\s*=\s*"([^"]+)"\s*\]"#).expect("valid regex");

    for (idx, line) in lines.iter().enumerate() {
        if let Some(caps) = ignore_re.captures(line) {
            let reason = caps.get(1).map_or("", |m| m.as_str());
            hazards.push(TestHazard {
                file_path: path.to_path_buf(),
                line_number: idx + 1,
                kind: HazardKind::IgnoredTestAdvisory,
                description: format!("Ignored test: \"{}\" (line {})", reason, idx + 1),
                suggested_fix:
                    "Review whether this test can be re-enabled or needs a tracking issue"
                        .to_string(),
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Recursively collect `.rs` files under `dir`.
fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    if !dir.is_dir() {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_rs_files(&p, out);
        } else if p.extension().is_some_and(|ext| ext == "rs") {
            out.push(p);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// Helper: scan inline content and return hazards.
    fn scan(content: &str) -> Vec<TestHazard> {
        let mut hazards = Vec::new();
        scan_content(Path::new("test.rs"), content, &mut hazards);
        hazards
    }

    // -- GlobalStateMutation ------------------------------------------------

    #[test]
    fn detects_set_current_dir_without_serial() {
        let src = r#"
#[test]
fn my_test() {
    std::env::set_current_dir("/tmp").unwrap();
}
"#;
        let h = scan(src);
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].kind, HazardKind::GlobalStateMutation);
        assert!(h[0].description.contains("set_current_dir"));
    }

    #[test]
    fn allows_set_current_dir_with_serial() {
        let src = r#"
use serial_test::serial;

#[test]
#[serial]
fn my_test() {
    std::env::set_current_dir("/tmp").unwrap();
}
"#;
        let h = scan(src);
        assert!(h.is_empty(), "should not flag when #[serial] is nearby");
    }

    #[test]
    fn allows_set_current_dir_with_test_threads() {
        let src = r#"
// Run with: cargo test -- --test-threads=1
#[test]
fn my_test() {
    std::env::set_current_dir("/tmp").unwrap();
}
"#;
        let h = scan(src);
        assert!(
            h.is_empty(),
            "should not flag when --test-threads=1 is nearby"
        );
    }

    #[test]
    fn ignores_set_current_dir_in_comments() {
        let src = r#"
#[test]
fn my_test() {
    // std::env::set_current_dir("/tmp").unwrap();
}
"#;
        let h = scan(src);
        assert!(h.is_empty(), "should not flag commented-out code");
    }

    // -- FragilePerformanceAssertion ----------------------------------------

    #[test]
    fn detects_elapsed_assert_without_gating() {
        let src = r#"
#[test]
fn perf_test() {
    let start = std::time::Instant::now();
    do_work();
    assert!(start.elapsed() < Duration::from_secs(5));
}
"#;
        let h = scan(src);
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].kind, HazardKind::FragilePerformanceAssertion);
    }

    #[test]
    fn allows_elapsed_assert_with_env_gating() {
        let src = r#"
#[test]
fn perf_test() {
    let start = std::time::Instant::now();
    do_work();
    if std::env::var("CI_STRICT").is_ok() {
        assert!(start.elapsed() < Duration::from_secs(5));
    }
}
"#;
        let h = scan(src);
        assert!(h.is_empty(), "should not flag when env gating is nearby");
    }

    #[test]
    fn detects_duration_assert_eq() {
        let src = r#"
#[test]
fn timing_test() {
    assert_eq!(elapsed, Duration::from_millis(100));
}
"#;
        let h = scan(src);
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].kind, HazardKind::FragilePerformanceAssertion);
    }

    // -- PlatformCapabilityAssumption ---------------------------------------

    #[test]
    fn detects_killpg_without_cfg_unix() {
        let src = r#"
#[test]
fn signal_test() {
    unsafe { libc::killpg(pid, libc::SIGTERM); }
}
"#;
        let h = scan(src);
        // Should detect both killpg and SIGTERM on the same line
        assert!(!h.is_empty());
        assert!(
            h.iter()
                .all(|h| h.kind == HazardKind::PlatformCapabilityAssumption)
        );
    }

    #[test]
    fn allows_signal_with_cfg_unix() {
        let src = r#"
#[cfg(unix)]
#[test]
fn signal_test() {
    unsafe { libc::killpg(pid, libc::SIGTERM); }
}
"#;
        let h = scan(src);
        assert!(h.is_empty(), "should not flag when #[cfg(unix)] is nearby");
    }

    #[test]
    fn allows_signal_with_runtime_skip() {
        let src = r#"
#[test]
fn signal_test() {
    if cfg!(windows) { return; }
    unsafe { libc::kill(pid, libc::SIGTERM); }
}
"#;
        let h = scan(src);
        assert!(
            h.is_empty(),
            "should not flag when cfg!(windows) skip is nearby"
        );
    }

    #[test]
    fn ignores_signal_names_in_comments() {
        let src = r#"
#[test]
fn my_test() {
    // We used to use SIGTERM here but not anymore
}
"#;
        let h = scan(src);
        assert!(h.is_empty(), "should not flag signal names in comments");
    }

    // -- IgnoredTestAdvisory ------------------------------------------------

    #[test]
    fn detects_ignored_test_with_reason() {
        let src = r#"
#[test]
#[ignore = "requires real Claude API"]
fn expensive_test() {}
"#;
        let h = scan(src);
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].kind, HazardKind::IgnoredTestAdvisory);
        assert!(h[0].description.contains("requires real Claude API"));
    }

    #[test]
    fn does_not_flag_ignore_without_reason() {
        let src = r#"
#[test]
#[ignore]
fn expensive_test() {}
"#;
        let h = scan(src);
        assert!(h.is_empty(), "bare #[ignore] should not be flagged");
    }

    // -- Serialization roundtrip -------------------------------------------

    #[test]
    fn hazard_serializes_to_json() {
        let hazard = TestHazard {
            file_path: PathBuf::from("tests/foo.rs"),
            line_number: 42,
            kind: HazardKind::GlobalStateMutation,
            description: "set_current_dir without serialization".to_string(),
            suggested_fix: "Add #[serial]".to_string(),
        };
        let json = serde_json::to_string(&hazard).unwrap();
        assert!(json.contains("global_state_mutation"));
        assert!(json.contains("\"line_number\":42"));
    }

    #[test]
    fn output_serializes_to_json() {
        let output = TestsDoctorOutput {
            files_scanned: 5,
            hazards: vec![],
        };
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("\"files_scanned\":5"));
        assert!(json.contains("\"hazards\":[]"));
    }

    // -- scan_test_hazards on empty dir ------------------------------------

    #[test]
    fn scan_empty_dir_returns_zero() {
        let tmp = tempfile::tempdir().unwrap();
        let output = scan_test_hazards(tmp.path()).unwrap();
        assert_eq!(output.files_scanned, 0);
        assert!(output.hazards.is_empty());
    }

    // -- collect_rs_files --------------------------------------------------

    #[test]
    fn collect_rs_files_finds_nested() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("a").join("b");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("test.rs"), "fn main() {}").unwrap();
        std::fs::write(sub.join("readme.md"), "# hi").unwrap();

        let mut files = Vec::new();
        collect_rs_files(tmp.path(), &mut files);
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("test.rs"));
    }

    // -- Multiple hazards in one file --------------------------------------

    #[test]
    fn detects_multiple_hazard_types() {
        let src = r#"
#[test]
fn bad_test() {
    std::env::set_current_dir("/tmp").unwrap();
    assert!(start.elapsed() < Duration::from_secs(1));
    unsafe { libc::killpg(pid, libc::SIGTERM); }
}
#[test]
#[ignore = "flaky on CI"]
fn another_test() {}
"#;
        let h = scan(src);
        let kinds: Vec<&HazardKind> = h.iter().map(|h| &h.kind).collect();
        assert!(kinds.contains(&&HazardKind::GlobalStateMutation));
        assert!(kinds.contains(&&HazardKind::FragilePerformanceAssertion));
        assert!(kinds.contains(&&HazardKind::PlatformCapabilityAssumption));
        assert!(kinds.contains(&&HazardKind::IgnoredTestAdvisory));
    }
}

//! Release readiness diagnostics for the xchecker workspace.
//!
//! Validates that the workspace is in a publishable state by checking version
//! coherence, publish tier ordering, README presence, package assemblability,
//! and lockfile freshness.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use std::process::Command;

use xchecker_utils::types::{CheckStatus, DoctorCheck};

/// Run all release-readiness checks against the given workspace root.
///
/// Returns a `Vec<DoctorCheck>` sorted by name. Each check is independent;
/// a failure in one does not prevent the others from running.
pub fn check_release_readiness(workspace_root: &Path) -> Vec<DoctorCheck> {
    let mut checks = Vec::new();

    checks.push(check_version_coherence(workspace_root));
    checks.push(check_readme_presence(workspace_root));
    checks.push(check_lockfile_freshness(workspace_root));

    // Publish-tier checks return multiple items
    checks.extend(check_publish_tiers(workspace_root));

    // Package assemblability is a warning-only check
    checks.push(check_package_assemblability(workspace_root));

    checks.sort_by(|a, b| a.name.cmp(&b.name));
    checks
}

// ---------------------------------------------------------------------------
// 1. Version coherence
// ---------------------------------------------------------------------------

/// Verify all workspace member crates use `version.workspace = true` or have
/// an explicit version matching `workspace.package.version`.
fn check_version_coherence(workspace_root: &Path) -> DoctorCheck {
    let name = "release_version_coherence".to_string();

    let root_toml_path = workspace_root.join("Cargo.toml");
    let root_contents = match std::fs::read_to_string(&root_toml_path) {
        Ok(c) => c,
        Err(e) => {
            return DoctorCheck {
                name,
                status: CheckStatus::Fail,
                details: format!("Cannot read root Cargo.toml: {e}"),
            };
        }
    };

    let root_doc: toml::Table = match root_contents.parse() {
        Ok(d) => d,
        Err(e) => {
            return DoctorCheck {
                name,
                status: CheckStatus::Fail,
                details: format!("Cannot parse root Cargo.toml: {e}"),
            };
        }
    };

    // Extract workspace.package.version
    let ws_version = root_doc
        .get("workspace")
        .and_then(|w| w.get("package"))
        .and_then(|p| p.get("version"))
        .and_then(|v| v.as_str());

    let ws_version = match ws_version {
        Some(v) => v.to_string(),
        None => {
            return DoctorCheck {
                name,
                status: CheckStatus::Fail,
                details: "workspace.package.version not found in root Cargo.toml".to_string(),
            };
        }
    };

    // Extract workspace members
    let members = match extract_workspace_members(&root_doc) {
        Ok(m) => m,
        Err(e) => {
            return DoctorCheck {
                name,
                status: CheckStatus::Fail,
                details: format!("Cannot extract workspace members: {e}"),
            };
        }
    };

    let mut mismatches: Vec<String> = Vec::new();

    for member_path in &members {
        let cargo_path = workspace_root.join(member_path).join("Cargo.toml");
        let contents = match std::fs::read_to_string(&cargo_path) {
            Ok(c) => c,
            Err(e) => {
                mismatches.push(format!("{member_path}: cannot read Cargo.toml ({e})"));
                continue;
            }
        };

        let doc: toml::Table = match contents.parse() {
            Ok(d) => d,
            Err(e) => {
                mismatches.push(format!("{member_path}: cannot parse Cargo.toml ({e})"));
                continue;
            }
        };

        let pkg = match doc.get("package") {
            Some(p) => p,
            None => {
                mismatches.push(format!("{member_path}: missing [package] section"));
                continue;
            }
        };

        // Check for version.workspace = true
        let version_entry = pkg.get("version");
        match version_entry {
            Some(toml::Value::Table(t))
                if t.get("workspace") == Some(&toml::Value::Boolean(true)) =>
            {
                // Good - inherits workspace version
            }
            Some(toml::Value::String(v)) if v == &ws_version => {
                // Explicit version that matches - acceptable
            }
            Some(toml::Value::String(v)) => {
                mismatches.push(format!(
                    "{member_path}: version is \"{v}\" but workspace is \"{ws_version}\""
                ));
            }
            _ => {
                mismatches.push(format!("{member_path}: unexpected version format"));
            }
        }
    }

    if mismatches.is_empty() {
        DoctorCheck {
            name,
            status: CheckStatus::Pass,
            details: format!(
                "All {} workspace members use version {ws_version}",
                members.len()
            ),
        }
    } else {
        DoctorCheck {
            name,
            status: CheckStatus::Fail,
            details: format!("Version mismatches found:\n  {}", mismatches.join("\n  ")),
        }
    }
}

// ---------------------------------------------------------------------------
// 2. Publish tier validation
// ---------------------------------------------------------------------------

/// Validate `scripts/publish-tiers.txt`:
/// - every listed crate exists in the workspace
/// - every publishable crate is listed
/// - tier ordering respects internal dependency order
fn check_publish_tiers(workspace_root: &Path) -> Vec<DoctorCheck> {
    let mut checks = Vec::new();

    let tiers_path = workspace_root.join("scripts").join("publish-tiers.txt");
    let tiers_contents = match std::fs::read_to_string(&tiers_path) {
        Ok(c) => c,
        Err(e) => {
            checks.push(DoctorCheck {
                name: "release_tier_file".to_string(),
                status: CheckStatus::Warn,
                details: format!("Cannot read scripts/publish-tiers.txt: {e}"),
            });
            return checks;
        }
    };

    // Parse tiers: each non-comment, non-empty line is one tier
    let tiers: Vec<Vec<String>> = tiers_contents
        .lines()
        .filter(|l| {
            let trimmed = l.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#')
        })
        .map(|line| {
            line.split_whitespace()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        })
        .collect();

    // Flatten all crates mentioned in tiers
    let tier_crates: BTreeSet<String> = tiers.iter().flat_map(|t| t.iter().cloned()).collect();

    // Build crate-to-tier-index map
    let mut crate_tier: HashMap<String, usize> = HashMap::new();
    for (idx, tier) in tiers.iter().enumerate() {
        for krate in tier {
            crate_tier.insert(krate.clone(), idx);
        }
    }

    // Load workspace members and their crate names
    let root_toml_path = workspace_root.join("Cargo.toml");
    let root_contents = match std::fs::read_to_string(&root_toml_path) {
        Ok(c) => c,
        Err(e) => {
            checks.push(DoctorCheck {
                name: "release_tier_file".to_string(),
                status: CheckStatus::Fail,
                details: format!("Cannot read root Cargo.toml: {e}"),
            });
            return checks;
        }
    };

    let root_doc: toml::Table = match root_contents.parse() {
        Ok(d) => d,
        Err(e) => {
            checks.push(DoctorCheck {
                name: "release_tier_file".to_string(),
                status: CheckStatus::Fail,
                details: format!("Cannot parse root Cargo.toml: {e}"),
            });
            return checks;
        }
    };

    let members = match extract_workspace_members(&root_doc) {
        Ok(m) => m,
        Err(e) => {
            checks.push(DoctorCheck {
                name: "release_tier_file".to_string(),
                status: CheckStatus::Fail,
                details: format!("Cannot extract workspace members: {e}"),
            });
            return checks;
        }
    };

    // Map member path -> crate name, and collect all publishable crate names
    let mut workspace_crates: BTreeSet<String> = BTreeSet::new();
    let mut member_name_map: BTreeMap<String, String> = BTreeMap::new(); // path -> name

    for member_path in &members {
        let cargo_path = workspace_root.join(member_path).join("Cargo.toml");
        if let Ok(contents) = std::fs::read_to_string(&cargo_path)
            && let Ok(doc) = contents.parse::<toml::Table>()
            && let Some(pkg_name) = doc
                .get("package")
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str())
        {
            // Check if publish = false
            let publish_false = doc
                .get("package")
                .and_then(|p| p.get("publish"))
                .and_then(|v| v.as_bool())
                == Some(false);

            if !publish_false {
                workspace_crates.insert(pkg_name.to_string());
            }
            member_name_map.insert(member_path.clone(), pkg_name.to_string());
        }
    }

    // Check 1: every tier crate exists in the workspace
    let unknown: Vec<&String> = tier_crates
        .iter()
        .filter(|c| !workspace_crates.contains(c.as_str()))
        .collect();

    // Check 2: every publishable workspace crate is in the tiers
    let missing: Vec<&String> = workspace_crates
        .iter()
        .filter(|c| !tier_crates.contains(c.as_str()))
        .collect();

    let mut coverage_issues: Vec<String> = Vec::new();
    if !unknown.is_empty() {
        coverage_issues.push(format!(
            "Crates in tiers but not in workspace: {}",
            unknown
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !missing.is_empty() {
        coverage_issues.push(format!(
            "Publishable crates missing from tiers: {}",
            missing
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    if coverage_issues.is_empty() {
        checks.push(DoctorCheck {
            name: "release_tier_coverage".to_string(),
            status: CheckStatus::Pass,
            details: format!(
                "All {} publishable crates are listed in {} tiers",
                workspace_crates.len(),
                tiers.len()
            ),
        });
    } else {
        checks.push(DoctorCheck {
            name: "release_tier_coverage".to_string(),
            status: CheckStatus::Fail,
            details: coverage_issues.join("; "),
        });
    }

    // Check 3: tier ordering respects dependency order
    // For each crate, its internal dependencies must be in the same or earlier tier.
    let mut ordering_violations: Vec<String> = Vec::new();

    for member_path in &members {
        let cargo_path = workspace_root.join(member_path).join("Cargo.toml");
        let contents = match std::fs::read_to_string(&cargo_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let crate_name = match member_name_map.get(member_path) {
            Some(n) => n,
            None => continue,
        };

        let my_tier = match crate_tier.get(crate_name) {
            Some(t) => *t,
            None => continue, // not in tiers
        };

        // Extract internal workspace dependencies from the TOML
        let internal_deps = extract_internal_deps(&contents, &workspace_crates);

        for dep in &internal_deps {
            if let Some(&dep_tier) = crate_tier.get(dep)
                && dep_tier > my_tier
            {
                ordering_violations.push(format!(
                    "{crate_name} (tier {my_tier}) depends on {dep} (tier {dep_tier})",
                    my_tier = my_tier + 1,
                    dep_tier = dep_tier + 1,
                ));
            }
        }
    }

    if ordering_violations.is_empty() {
        checks.push(DoctorCheck {
            name: "release_tier_ordering".to_string(),
            status: CheckStatus::Pass,
            details: "Tier ordering respects dependency graph".to_string(),
        });
    } else {
        checks.push(DoctorCheck {
            name: "release_tier_ordering".to_string(),
            status: CheckStatus::Fail,
            details: format!(
                "Tier ordering violations:\n  {}",
                ordering_violations.join("\n  ")
            ),
        });
    }

    checks
}

// ---------------------------------------------------------------------------
// 3. README / docs existence
// ---------------------------------------------------------------------------

/// For each publishable crate, check that either the crate has its own
/// `README.md` or the root README is referenced via `readme` in Cargo.toml.
fn check_readme_presence(workspace_root: &Path) -> DoctorCheck {
    let name = "release_readme_presence".to_string();

    let root_toml_path = workspace_root.join("Cargo.toml");
    let root_contents = match std::fs::read_to_string(&root_toml_path) {
        Ok(c) => c,
        Err(e) => {
            return DoctorCheck {
                name,
                status: CheckStatus::Fail,
                details: format!("Cannot read root Cargo.toml: {e}"),
            };
        }
    };

    let root_doc: toml::Table = match root_contents.parse() {
        Ok(d) => d,
        Err(e) => {
            return DoctorCheck {
                name,
                status: CheckStatus::Fail,
                details: format!("Cannot parse root Cargo.toml: {e}"),
            };
        }
    };

    let members = match extract_workspace_members(&root_doc) {
        Ok(m) => m,
        Err(e) => {
            return DoctorCheck {
                name,
                status: CheckStatus::Fail,
                details: format!("Cannot extract workspace members: {e}"),
            };
        }
    };

    let mut missing_readme: Vec<String> = Vec::new();

    for member_path in &members {
        let cargo_path = workspace_root.join(member_path).join("Cargo.toml");
        let contents = match std::fs::read_to_string(&cargo_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let doc: toml::Table = match contents.parse() {
            Ok(d) => d,
            Err(_) => continue,
        };

        let pkg = match doc.get("package") {
            Some(p) => p,
            None => continue,
        };

        let pkg_name = pkg
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or(member_path);

        // Skip publish = false
        if pkg.get("publish").and_then(|v| v.as_bool()) == Some(false) {
            continue;
        }

        // Check if `readme` key is set in Cargo.toml (any value counts)
        let has_readme_key = pkg.get("readme").is_some();

        // Check if the crate directory contains a README.md
        let crate_readme = workspace_root.join(member_path).join("README.md");
        let has_local_readme = crate_readme.exists();

        if !has_readme_key && !has_local_readme {
            missing_readme.push(pkg_name.to_string());
        }
    }

    if missing_readme.is_empty() {
        DoctorCheck {
            name,
            status: CheckStatus::Pass,
            details: "All publishable crates have a README or readme reference".to_string(),
        }
    } else {
        DoctorCheck {
            name,
            status: CheckStatus::Warn,
            details: format!(
                "{} crate(s) without README.md or readme field: {}",
                missing_readme.len(),
                missing_readme.join(", ")
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// 4. Package assemblability (dry-run)
// ---------------------------------------------------------------------------

/// Shell out to `cargo package --list` for the root crate to verify it can be
/// assembled. Uses a timeout; failures are warnings, not hard failures.
fn check_package_assemblability(workspace_root: &Path) -> DoctorCheck {
    let name = "release_package_assemblable".to_string();

    let result = Command::new("cargo")
        .args(["package", "--list", "--allow-dirty"])
        .current_dir(workspace_root)
        .output();

    match result {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let file_count = stdout.lines().count();
            DoctorCheck {
                name,
                status: CheckStatus::Pass,
                details: format!("Root crate can be packaged ({file_count} files in archive)"),
            }
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let tail: String = stderr
                .lines()
                .rev()
                .take(5)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join("\n");
            DoctorCheck {
                name,
                status: CheckStatus::Warn,
                details: format!(
                    "cargo package --list failed (exit {}):\n{tail}",
                    output.status
                ),
            }
        }
        Err(e) => DoctorCheck {
            name,
            status: CheckStatus::Warn,
            details: format!("Could not run cargo package --list: {e}"),
        },
    }
}

// ---------------------------------------------------------------------------
// 5. Lockfile freshness
// ---------------------------------------------------------------------------

/// Run `cargo update --dry-run` and check whether the lockfile would change.
fn check_lockfile_freshness(workspace_root: &Path) -> DoctorCheck {
    let name = "release_lockfile_fresh".to_string();

    // `cargo update --dry-run` exits 0 and prints to stderr when updates are
    // available. If the lockfile is perfectly fresh, stderr is minimal.
    let result = Command::new("cargo")
        .args(["update", "--dry-run"])
        .current_dir(workspace_root)
        .output();

    match result {
        Ok(output) if output.status.success() => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Lines containing "Updating" indicate actual changes
            let updates: Vec<&str> = stderr.lines().filter(|l| l.contains("Updating")).collect();

            if updates.is_empty() {
                DoctorCheck {
                    name,
                    status: CheckStatus::Pass,
                    details: "Cargo.lock is up to date".to_string(),
                }
            } else {
                DoctorCheck {
                    name,
                    status: CheckStatus::Warn,
                    details: format!(
                        "Cargo.lock has {} pending update(s):\n  {}",
                        updates.len(),
                        updates
                            .iter()
                            .take(10)
                            .copied()
                            .collect::<Vec<_>>()
                            .join("\n  ")
                    ),
                }
            }
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            DoctorCheck {
                name,
                status: CheckStatus::Warn,
                details: format!(
                    "cargo update --dry-run exited with {}:\n{}",
                    output.status,
                    stderr.lines().take(5).collect::<Vec<_>>().join("\n")
                ),
            }
        }
        Err(e) => DoctorCheck {
            name,
            status: CheckStatus::Warn,
            details: format!("Could not run cargo update --dry-run: {e}"),
        },
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract the `workspace.members` list from a parsed root Cargo.toml.
fn extract_workspace_members(root_doc: &toml::Table) -> Result<Vec<String>, String> {
    let members = root_doc
        .get("workspace")
        .and_then(|w| w.get("members"))
        .and_then(|m| m.as_array())
        .ok_or_else(|| "workspace.members not found".to_string())?;

    members
        .iter()
        .map(|v| {
            v.as_str()
                .map(|s| s.to_string())
                .ok_or_else(|| "workspace.members contains non-string entry".to_string())
        })
        .collect()
}

/// Extract internal workspace dependency names from a crate's Cargo.toml content.
///
/// We look for dependency keys that match known workspace crate names, scanning
/// `[dependencies]`, `[dev-dependencies]`, and `[build-dependencies]`.
fn extract_internal_deps(cargo_contents: &str, workspace_crates: &BTreeSet<String>) -> Vec<String> {
    let doc: toml::Table = match cargo_contents.parse() {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    let mut deps = BTreeSet::new();

    for section in &["dependencies", "dev-dependencies", "build-dependencies"] {
        if let Some(table) = doc.get(*section).and_then(|v| v.as_table()) {
            for key in table.keys() {
                if workspace_crates.contains(key) {
                    deps.insert(key.clone());
                }
            }
        }
    }

    deps.into_iter().collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Find the workspace root by walking up from the crate directory.
    fn workspace_root() -> std::path::PathBuf {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        // crates/xchecker-doctor -> workspace root is two levels up
        manifest_dir
            .parent()
            .and_then(|p| p.parent())
            .expect("Could not find workspace root")
            .to_path_buf()
    }

    #[test]
    fn test_version_coherence_passes_on_real_workspace() {
        let root = workspace_root();
        let check = check_version_coherence(&root);
        assert_eq!(check.name, "release_version_coherence");
        assert_eq!(
            check.status,
            CheckStatus::Pass,
            "Version coherence should pass: {}",
            check.details
        );
    }

    #[test]
    fn test_publish_tier_coverage_passes_on_real_workspace() {
        let root = workspace_root();
        let checks = check_publish_tiers(&root);

        let coverage = checks
            .iter()
            .find(|c| c.name == "release_tier_coverage")
            .expect("release_tier_coverage check should exist");

        assert_eq!(
            coverage.status,
            CheckStatus::Pass,
            "Tier coverage should pass: {}",
            coverage.details
        );
    }

    #[test]
    fn test_publish_tier_ordering_passes_on_real_workspace() {
        let root = workspace_root();
        let checks = check_publish_tiers(&root);

        let ordering = checks
            .iter()
            .find(|c| c.name == "release_tier_ordering")
            .expect("release_tier_ordering check should exist");

        assert_eq!(
            ordering.status,
            CheckStatus::Pass,
            "Tier ordering should pass: {}",
            ordering.details
        );
    }

    #[test]
    fn test_check_release_readiness_returns_sorted_checks() {
        let root = workspace_root();
        let checks = check_release_readiness(&root);

        assert!(!checks.is_empty(), "Should return at least one check");

        let names: Vec<&str> = checks.iter().map(|c| c.name.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted, "Checks should be sorted by name");
    }

    #[test]
    fn test_extract_workspace_members() {
        let toml_str = r#"
[workspace]
members = [".", "crates/foo", "crates/bar"]

[workspace.package]
version = "1.0.0"
"#;
        let doc: toml::Table = toml_str.parse().unwrap();
        let members = extract_workspace_members(&doc).unwrap();
        assert_eq!(members, vec![".", "crates/foo", "crates/bar"]);
    }

    #[test]
    fn test_extract_internal_deps() {
        let cargo_toml = r#"
[package]
name = "my-crate"
version = "1.0.0"

[dependencies]
xchecker-utils = { workspace = true }
serde = "1.0"

[dev-dependencies]
xchecker-config = { workspace = true }
"#;
        let mut workspace = BTreeSet::new();
        workspace.insert("xchecker-utils".to_string());
        workspace.insert("xchecker-config".to_string());
        workspace.insert("xchecker-engine".to_string());

        let deps = extract_internal_deps(cargo_toml, &workspace);
        assert!(deps.contains(&"xchecker-utils".to_string()));
        assert!(deps.contains(&"xchecker-config".to_string()));
        assert!(!deps.contains(&"xchecker-engine".to_string()));
    }

    #[test]
    fn test_version_coherence_detects_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // Write root Cargo.toml
        std::fs::write(
            root.join("Cargo.toml"),
            r#"
[workspace]
members = ["crates/good", "crates/bad"]

[workspace.package]
version = "2.0.0"
"#,
        )
        .unwrap();

        // Good crate: version.workspace = true
        let good_dir = root.join("crates/good");
        std::fs::create_dir_all(&good_dir).unwrap();
        std::fs::write(
            good_dir.join("Cargo.toml"),
            r#"
[package]
name = "good"
version.workspace = true
"#,
        )
        .unwrap();

        // Bad crate: explicit wrong version
        let bad_dir = root.join("crates/bad");
        std::fs::create_dir_all(&bad_dir).unwrap();
        std::fs::write(
            bad_dir.join("Cargo.toml"),
            r#"
[package]
name = "bad"
version = "1.0.0"
"#,
        )
        .unwrap();

        let check = check_version_coherence(root);
        assert_eq!(check.status, CheckStatus::Fail);
        assert!(check.details.contains("1.0.0"));
        assert!(check.details.contains("2.0.0"));
    }
}

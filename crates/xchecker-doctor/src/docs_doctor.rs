//! Documentation health checks for the `doctor docs` diagnostic subcommand.
//!
//! Scans Markdown files in `docs/` and `README.md` for broken internal links,
//! INDEX.md completeness, stale code references, and audience mixing.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;
use xchecker_utils::types::{CheckStatus, DoctorCheck};

/// Run all documentation health checks against a workspace root.
///
/// Returns a `Vec<DoctorCheck>` with results for:
/// - `docs_broken_links` -- broken internal Markdown links
/// - `docs_index_completeness` -- files missing from `docs/INDEX.md`
/// - `docs_readme_links` -- broken links in `README.md`
/// - `docs_stale_code_refs` -- references to Rust identifiers not found in source
/// - `docs_audience_mixing` -- files mixing user-facing and contributor-only markers
pub fn check_docs_health(workspace_root: &Path) -> Vec<DoctorCheck> {
    vec![
        check_broken_links(workspace_root),
        check_index_completeness(workspace_root),
        check_readme_links(workspace_root),
        check_stale_code_refs(workspace_root),
        check_audience_mixing(workspace_root),
    ]
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Collect all `.md` files under a directory, recursively.
fn collect_md_files(dir: &Path) -> Vec<PathBuf> {
    let mut results = Vec::new();
    collect_md_files_recursive(dir, &mut results);
    results
}

fn collect_md_files_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_md_files_recursive(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "md") {
            out.push(path);
        }
    }
}

/// Extract markdown links from content, returning `(line_number, link_text, target_path)`.
///
/// Only considers relative file paths -- ignores URLs (http/https/mailto), anchors-only
/// links (`#section`), and bare fragment references.
fn extract_md_links(content: &str) -> Vec<(usize, String, String)> {
    let link_re = Regex::new(r"\[([^\]]*)\]\(([^)]+)\)").expect("valid regex");
    let mut links = Vec::new();

    for (line_idx, line) in content.lines().enumerate() {
        for cap in link_re.captures_iter(line) {
            let text = cap[1].to_string();
            let target = cap[2].to_string();

            // Skip external URLs
            if target.starts_with("http://")
                || target.starts_with("https://")
                || target.starts_with("mailto:")
            {
                continue;
            }
            // Skip pure anchor links
            if target.starts_with('#') {
                continue;
            }

            // Strip fragment from path (e.g. `CLI.md#embedding` -> `CLI.md`)
            let path_part = target.split('#').next().unwrap_or(&target).to_string();
            if path_part.is_empty() {
                continue;
            }

            links.push((line_idx + 1, text, path_part));
        }
    }
    links
}

/// Resolve a relative link target against the directory containing `source_file`.
fn resolve_link(source_file: &Path, target: &str) -> PathBuf {
    let base = source_file.parent().unwrap_or(source_file);
    base.join(target)
}

// ---------------------------------------------------------------------------
// Check 1: Broken internal links in docs/ and README.md
// ---------------------------------------------------------------------------

fn check_broken_links(root: &Path) -> DoctorCheck {
    let docs_dir = root.join("docs");
    let mut md_files = collect_md_files(&docs_dir);

    let readme = root.join("README.md");
    if readme.is_file() {
        md_files.push(readme);
    }

    let mut broken: Vec<String> = Vec::new();

    for md_file in &md_files {
        let content = match fs::read_to_string(md_file) {
            Ok(c) => c,
            Err(_) => continue,
        };

        for (line_no, _text, target) in extract_md_links(&content) {
            // Skip directory links like `contributor/`
            if target.ends_with('/') {
                let resolved = resolve_link(md_file, &target);
                if !resolved.is_dir() {
                    let rel = md_file
                        .strip_prefix(root)
                        .unwrap_or(md_file)
                        .display()
                        .to_string()
                        .replace('\\', "/");
                    broken.push(format!(
                        "  {rel}:{line_no} -> {target} (directory not found)"
                    ));
                }
                continue;
            }

            let resolved = resolve_link(md_file, &target);
            if !resolved.is_file() {
                let rel = md_file
                    .strip_prefix(root)
                    .unwrap_or(md_file)
                    .display()
                    .to_string()
                    .replace('\\', "/");
                broken.push(format!("  {rel}:{line_no} -> {target}"));
            }
        }
    }

    if broken.is_empty() {
        DoctorCheck {
            name: "docs_broken_links".to_string(),
            status: CheckStatus::Pass,
            details: "All internal doc links resolve to existing files".to_string(),
        }
    } else {
        DoctorCheck {
            name: "docs_broken_links".to_string(),
            status: CheckStatus::Fail,
            details: format!(
                "{} broken link(s) found:\n{}",
                broken.len(),
                broken.join("\n")
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// Check 2: INDEX.md completeness
// ---------------------------------------------------------------------------

fn check_index_completeness(root: &Path) -> DoctorCheck {
    let index_path = root.join("docs").join("INDEX.md");
    let index_content = match fs::read_to_string(&index_path) {
        Ok(c) => c,
        Err(e) => {
            return DoctorCheck {
                name: "docs_index_completeness".to_string(),
                status: CheckStatus::Fail,
                details: format!("Could not read docs/INDEX.md: {e}"),
            };
        }
    };

    // Collect all .md files in docs/ subdirectories (skip docs/*.md at top level
    // since INDEX.md is the index itself and top-level files may be legacy)
    let docs_dir = root.join("docs");
    let all_md = collect_md_files(&docs_dir);

    // Build a set of relative paths mentioned in INDEX.md
    let referenced: HashSet<String> = extract_md_links(&index_content)
        .into_iter()
        .map(|(_, _, target)| {
            // Normalize to forward slashes for comparison
            target.replace('\\', "/")
        })
        .collect();

    let mut missing: Vec<String> = Vec::new();

    for md_path in &all_md {
        // Only check files in subdirectories of docs/
        let rel_to_docs = match md_path.strip_prefix(&docs_dir) {
            Ok(r) => r,
            Err(_) => continue,
        };

        // Skip files directly in docs/ (top-level legacy files and INDEX.md itself)
        if rel_to_docs.parent().is_some_and(|p| p == Path::new("")) {
            continue;
        }

        // Skip non-md files in schemas/ etc.
        if rel_to_docs
            .components()
            .next()
            .is_some_and(|c| c.as_os_str() == "schemas")
        {
            continue;
        }

        let rel_str = rel_to_docs.display().to_string().replace('\\', "/");

        if !referenced.contains(&rel_str) {
            missing.push(format!("  docs/{rel_str}"));
        }
    }

    if missing.is_empty() {
        DoctorCheck {
            name: "docs_index_completeness".to_string(),
            status: CheckStatus::Pass,
            details: "All doc files in subdirectories are referenced in INDEX.md".to_string(),
        }
    } else {
        DoctorCheck {
            name: "docs_index_completeness".to_string(),
            status: CheckStatus::Warn,
            details: format!(
                "{} doc file(s) not referenced in INDEX.md:\n{}",
                missing.len(),
                missing.join("\n")
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// Check 3: README link health
// ---------------------------------------------------------------------------

fn check_readme_links(root: &Path) -> DoctorCheck {
    let readme = root.join("README.md");
    let content = match fs::read_to_string(&readme) {
        Ok(c) => c,
        Err(e) => {
            return DoctorCheck {
                name: "docs_readme_links".to_string(),
                status: CheckStatus::Fail,
                details: format!("Could not read README.md: {e}"),
            };
        }
    };

    let mut broken: Vec<String> = Vec::new();

    for (line_no, _text, target) in extract_md_links(&content) {
        if target.ends_with('/') {
            let resolved = resolve_link(&readme, &target);
            if !resolved.is_dir() {
                broken.push(format!(
                    "  README.md:{line_no} -> {target} (directory not found)"
                ));
            }
            continue;
        }

        let resolved = resolve_link(&readme, &target);
        if !resolved.is_file() {
            broken.push(format!("  README.md:{line_no} -> {target}"));
        }
    }

    if broken.is_empty() {
        DoctorCheck {
            name: "docs_readme_links".to_string(),
            status: CheckStatus::Pass,
            details: "All README.md links resolve to existing files".to_string(),
        }
    } else {
        DoctorCheck {
            name: "docs_readme_links".to_string(),
            status: CheckStatus::Fail,
            details: format!(
                "{} broken README link(s):\n{}",
                broken.len(),
                broken.join("\n")
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// Check 4: Stale code references
// ---------------------------------------------------------------------------

/// Well-known Rust identifiers that docs commonly reference.
/// If a doc file mentions one of these and it no longer exists in `src/` or
/// `crates/`, we flag it.
const CODE_IDENTIFIERS: &[&str] = &[
    "OrchestratorHandle",
    "PhaseOrchestrator",
    "DoctorCommand",
    "DoctorCheck",
    "DoctorOutput",
    "CheckStatus",
    "PhaseId",
    "Config",
    "Runner",
    "RunnerMode",
    "CommandSpec",
    "SandboxRoot",
    "InsightCache",
    "Packet",
    "PacketBuilder",
    "Receipt",
    "StatusOutput",
    "GateResult",
    "LlmProvider",
];

fn check_stale_code_refs(root: &Path) -> DoctorCheck {
    // First, determine which identifiers actually exist in source
    let src_dirs: Vec<PathBuf> = vec![root.join("src"), root.join("crates")];

    let mut existing_idents: HashSet<&str> = HashSet::new();
    for ident in CODE_IDENTIFIERS {
        if ident_exists_in_source(&src_dirs, ident) {
            existing_idents.insert(ident);
        }
    }

    // Now scan doc files for references to identifiers that do NOT exist
    let docs_dir = root.join("docs");
    let mut md_files = collect_md_files(&docs_dir);
    let readme = root.join("README.md");
    if readme.is_file() {
        md_files.push(readme);
    }
    let claude_md = root.join("CLAUDE.md");
    if claude_md.is_file() {
        md_files.push(claude_md);
    }

    let mut stale: Vec<String> = Vec::new();

    for md_file in &md_files {
        let content = match fs::read_to_string(md_file) {
            Ok(c) => c,
            Err(_) => continue,
        };

        for ident in CODE_IDENTIFIERS {
            if !existing_idents.contains(ident) && content.contains(ident) {
                let rel = md_file
                    .strip_prefix(root)
                    .unwrap_or(md_file)
                    .display()
                    .to_string()
                    .replace('\\', "/");
                stale.push(format!(
                    "  {rel} references `{ident}` (not found in source)"
                ));
            }
        }
    }

    // Deduplicate
    stale.sort();
    stale.dedup();

    if stale.is_empty() {
        DoctorCheck {
            name: "docs_stale_code_refs".to_string(),
            status: CheckStatus::Pass,
            details: "All documented Rust identifiers found in source".to_string(),
        }
    } else {
        DoctorCheck {
            name: "docs_stale_code_refs".to_string(),
            status: CheckStatus::Warn,
            details: format!(
                "{} stale code reference(s):\n{}",
                stale.len(),
                stale.join("\n")
            ),
        }
    }
}

/// Check whether `ident` appears in any `.rs` file under the given directories.
fn ident_exists_in_source(dirs: &[PathBuf], ident: &str) -> bool {
    for dir in dirs {
        if !dir.is_dir() {
            continue;
        }
        if ident_found_recursively(dir, ident) {
            return true;
        }
    }
    false
}

fn ident_found_recursively(dir: &Path, ident: &str) -> bool {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return false,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if ident_found_recursively(&path, ident) {
                return true;
            }
        } else if path.extension().is_some_and(|ext| ext == "rs")
            && let Ok(content) = fs::read_to_string(&path)
            && content.contains(ident)
        {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Check 5: Audience mixing
// ---------------------------------------------------------------------------

/// User-facing content markers (heuristic).
const USER_MARKERS: &[&str] = &[
    "cargo install",
    "xchecker doctor",
    "xchecker spec",
    "xchecker status",
    "xchecker resume",
    "you can",
    "You can",
    "## Install",
    "## Quick",
    "getting started",
    "Getting Started",
];

/// Contributor / internal markers (heuristic).
const CONTRIBUTOR_MARKERS: &[&str] = &[
    "pub(crate)",
    "internal API",
    "module structure",
    "implementation detail",
    "white-box test",
    "#[cfg(test)]",
    "cargo test --workspace",
    "PhaseOrchestrator::new",
    "dev-dependency",
];

fn check_audience_mixing(root: &Path) -> DoctorCheck {
    let docs_dir = root.join("docs");
    let md_files = collect_md_files(&docs_dir);

    let mut mixed: Vec<String> = Vec::new();

    for md_file in &md_files {
        let content = match fs::read_to_string(md_file) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let has_user = USER_MARKERS.iter().any(|m| content.contains(m));
        let has_contributor = CONTRIBUTOR_MARKERS.iter().any(|m| content.contains(m));

        if has_user && has_contributor {
            let rel = md_file
                .strip_prefix(root)
                .unwrap_or(md_file)
                .display()
                .to_string()
                .replace('\\', "/");

            // Collect which markers were found for diagnostics
            let found_user: Vec<&str> = USER_MARKERS
                .iter()
                .filter(|m| content.contains(**m))
                .copied()
                .collect();
            let found_contrib: Vec<&str> = CONTRIBUTOR_MARKERS
                .iter()
                .filter(|m| content.contains(**m))
                .copied()
                .collect();

            mixed.push(format!(
                "  {rel}\n    user markers: {}\n    contributor markers: {}",
                found_user.join(", "),
                found_contrib.join(", "),
            ));
        }
    }

    if mixed.is_empty() {
        DoctorCheck {
            name: "docs_audience_mixing".to_string(),
            status: CheckStatus::Pass,
            details: "No audience mixing detected in doc files".to_string(),
        }
    } else {
        DoctorCheck {
            name: "docs_audience_mixing".to_string(),
            status: CheckStatus::Warn,
            details: format!(
                "{} file(s) mix user-facing and contributor content:\n{}",
                mixed.len(),
                mixed.join("\n")
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Helper to create a temp workspace with a docs/ structure.
    fn make_workspace() -> TempDir {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Create docs structure
        fs::create_dir_all(root.join("docs/guides")).unwrap();
        fs::create_dir_all(root.join("docs/reference")).unwrap();
        fs::create_dir_all(root.join("docs/contributor")).unwrap();
        fs::create_dir_all(root.join("src")).unwrap();

        tmp
    }

    #[test]
    fn test_extract_md_links_basic() {
        let content = r#"
# Hello

See [the guide](guides/SETUP.md) for details.
Also [external](https://example.com) and [anchor](#section).
Check [ref](reference/CLI.md#embedding) too.
"#;
        let links = extract_md_links(content);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].2, "guides/SETUP.md");
        assert_eq!(links[1].2, "reference/CLI.md");
    }

    #[test]
    fn test_extract_md_links_ignores_urls() {
        let content = "[link](https://example.com)\n[mail](mailto:a@b.com)\n";
        let links = extract_md_links(content);
        assert!(links.is_empty());
    }

    #[test]
    fn test_broken_links_pass_when_all_exist() {
        let tmp = make_workspace();
        let root = tmp.path();

        fs::write(root.join("README.md"), "# Hello\n").unwrap();
        fs::write(root.join("docs/INDEX.md"), "[guide](guides/SETUP.md)\n").unwrap();
        fs::write(root.join("docs/guides/SETUP.md"), "# Setup\n").unwrap();

        let check = check_broken_links(root);
        assert_eq!(check.status, CheckStatus::Pass);
    }

    #[test]
    fn test_broken_links_fail_when_target_missing() {
        let tmp = make_workspace();
        let root = tmp.path();

        fs::write(root.join("README.md"), "# Hello\n").unwrap();
        fs::write(
            root.join("docs/INDEX.md"),
            "[missing](guides/NONEXISTENT.md)\n",
        )
        .unwrap();

        let check = check_broken_links(root);
        assert_eq!(check.status, CheckStatus::Fail);
        assert!(check.details.contains("NONEXISTENT.md"));
    }

    #[test]
    fn test_index_completeness_pass() {
        let tmp = make_workspace();
        let root = tmp.path();

        fs::write(root.join("docs/guides/SETUP.md"), "# Setup\n").unwrap();
        fs::write(root.join("docs/INDEX.md"), "[setup](guides/SETUP.md)\n").unwrap();

        let check = check_index_completeness(root);
        assert_eq!(check.status, CheckStatus::Pass);
    }

    #[test]
    fn test_index_completeness_warns_on_missing() {
        let tmp = make_workspace();
        let root = tmp.path();

        fs::write(root.join("docs/guides/SETUP.md"), "# Setup\n").unwrap();
        fs::write(root.join("docs/guides/HIDDEN.md"), "# Hidden\n").unwrap();
        fs::write(root.join("docs/INDEX.md"), "[setup](guides/SETUP.md)\n").unwrap();

        let check = check_index_completeness(root);
        assert_eq!(check.status, CheckStatus::Warn);
        assert!(check.details.contains("HIDDEN.md"));
    }

    #[test]
    fn test_readme_links_pass() {
        let tmp = make_workspace();
        let root = tmp.path();

        fs::write(root.join("docs/guides/SETUP.md"), "# Setup\n").unwrap();
        fs::write(root.join("README.md"), "[setup](docs/guides/SETUP.md)\n").unwrap();

        let check = check_readme_links(root);
        assert_eq!(check.status, CheckStatus::Pass);
    }

    #[test]
    fn test_readme_links_fail() {
        let tmp = make_workspace();
        let root = tmp.path();

        fs::write(root.join("README.md"), "[gone](docs/guides/GONE.md)\n").unwrap();

        let check = check_readme_links(root);
        assert_eq!(check.status, CheckStatus::Fail);
        assert!(check.details.contains("GONE.md"));
    }

    #[test]
    fn test_stale_code_refs_pass() {
        let tmp = make_workspace();
        let root = tmp.path();

        fs::write(root.join("README.md"), "# Hello\n").unwrap();
        fs::write(root.join("docs/INDEX.md"), "index\n").unwrap();
        fs::write(
            root.join("src/lib.rs"),
            "pub struct DoctorCheck {}\npub struct OrchestratorHandle {}\n",
        )
        .unwrap();
        fs::write(
            root.join("docs/guides/FOO.md"),
            "Use `DoctorCheck` to check health.\n",
        )
        .unwrap();

        let check = check_stale_code_refs(root);
        assert_eq!(check.status, CheckStatus::Pass);
    }

    #[test]
    fn test_stale_code_refs_warns() {
        let tmp = make_workspace();
        let root = tmp.path();

        fs::write(root.join("README.md"), "# Hello\n").unwrap();
        fs::write(root.join("docs/INDEX.md"), "index\n").unwrap();
        // Source has no identifiers at all
        fs::write(root.join("src/lib.rs"), "fn main() {}\n").unwrap();
        fs::write(
            root.join("docs/guides/FOO.md"),
            "The `PacketBuilder` type handles packet construction.\n",
        )
        .unwrap();

        let check = check_stale_code_refs(root);
        assert_eq!(check.status, CheckStatus::Warn);
        assert!(check.details.contains("PacketBuilder"));
    }

    #[test]
    fn test_audience_mixing_pass_pure_user() {
        let tmp = make_workspace();
        let root = tmp.path();

        fs::write(
            root.join("docs/guides/INSTALL.md"),
            "## Install\n\nYou can install with `cargo install xchecker`.\n",
        )
        .unwrap();

        let check = check_audience_mixing(root);
        assert_eq!(check.status, CheckStatus::Pass);
    }

    #[test]
    fn test_audience_mixing_pass_pure_contributor() {
        let tmp = make_workspace();
        let root = tmp.path();

        fs::write(
            root.join("docs/contributor/INTERNALS.md"),
            "The internal API uses `pub(crate)` for module structure.\n",
        )
        .unwrap();

        let check = check_audience_mixing(root);
        assert_eq!(check.status, CheckStatus::Pass);
    }

    #[test]
    fn test_audience_mixing_warns_on_mixed() {
        let tmp = make_workspace();
        let root = tmp.path();

        fs::write(
            root.join("docs/guides/MIXED.md"),
            "## Install\n\nYou can run `cargo install xchecker`.\n\nThe internal API uses `pub(crate)` visibility.\n",
        )
        .unwrap();

        let check = check_audience_mixing(root);
        assert_eq!(check.status, CheckStatus::Warn);
        assert!(check.details.contains("MIXED.md"));
    }

    #[test]
    fn test_check_docs_health_returns_all_five_checks() {
        let tmp = make_workspace();
        let root = tmp.path();

        fs::write(root.join("README.md"), "# Hello\n").unwrap();
        fs::write(root.join("docs/INDEX.md"), "index\n").unwrap();
        fs::write(root.join("src/lib.rs"), "fn main() {}\n").unwrap();

        let checks = check_docs_health(root);
        assert_eq!(checks.len(), 5);

        let names: Vec<&str> = checks.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"docs_broken_links"));
        assert!(names.contains(&"docs_index_completeness"));
        assert!(names.contains(&"docs_readme_links"));
        assert!(names.contains(&"docs_stale_code_refs"));
        assert!(names.contains(&"docs_audience_mixing"));
    }
}

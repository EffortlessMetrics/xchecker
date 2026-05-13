use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const BADGE_ENDPOINT_DIR: &str = "badges";
const BADGE_ENDPOINT_TARGET_DIR: &str = "target/xtask/badges";
const RIPR_PR_DIR: &str = "target/ripr/pr";
const RIPR_REVIEW_DIR: &str = "target/ripr/review";

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
struct ShieldsEndpointBadge {
    #[serde(rename = "schemaVersion")]
    schema_version: u8,
    label: String,
    message: String,
    color: String,
}

fn main() -> anyhow::Result<()> {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        print_help();
        return Ok(());
    };
    let rest: Vec<String> = args.collect();
    let check = rest.iter().any(|arg| arg == "--check");

    match command.as_str() {
        "badges" => badges(check),
        "ripr-pr" => ripr_pr(check),
        "ripr-review-comments" => ripr_review_comments(check),
        "docs-sync" => docs_sync(check),
        "check-file-policy" => check_file_policy(),
        "pr" => pr(),
        "impacted-evidence" => impacted_evidence(),
        "mutants-pr" => mutants_pr(&rest),
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        other => bail!("unknown xtask command `{other}`"),
    }
}

fn print_help() {
    eprintln!(
        "xtask commands:\n  badges [--check]\n  ripr-pr [--check]\n  ripr-review-comments [--check]\n  docs-sync --check\n  check-file-policy\n  pr"
    );
}

fn workspace_root_path() -> anyhow::Result<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .map(Path::to_path_buf)
        .context("xtask manifest directory has no workspace parent")
}

fn badges(check: bool) -> anyhow::Result<()> {
    let workspace_root = workspace_root_path()?;
    let target_dir = workspace_root.join(BADGE_ENDPOINT_TARGET_DIR);
    fs::create_dir_all(&target_dir)?;

    let ripr_plus = ripr_plus_badge(&workspace_root)?;
    validate_shields_badge(&ripr_plus, Some("ripr+"))?;
    write_json_pretty(&target_dir.join("ripr-plus.json"), &ripr_plus)?;

    if check {
        let committed_dir = workspace_root.join(BADGE_ENDPOINT_DIR);
        compare_files(
            &committed_dir.join("ripr-plus.json"),
            &target_dir.join("ripr-plus.json"),
        )?;
        println!("badges: committed endpoints are current");
        return Ok(());
    }

    let committed_dir = workspace_root.join(BADGE_ENDPOINT_DIR);
    fs::create_dir_all(&committed_dir)?;
    fs::copy(
        target_dir.join("ripr-plus.json"),
        committed_dir.join("ripr-plus.json"),
    )?;
    println!("badges: refreshed public endpoint JSON under badges/");
    Ok(())
}

fn ripr_plus_badge(workspace_root: &Path) -> anyhow::Result<ShieldsEndpointBadge> {
    let ripr_bin = env::var("RIPR_BIN").unwrap_or_else(|_| "ripr".to_string());
    let output = Command::new(&ripr_bin)
        .arg("check")
        .arg("--root")
        .arg(workspace_root)
        .arg("--format")
        .arg("repo-badge-plus-shields")
        .current_dir(workspace_root)
        .output()
        .with_context(|| format!("failed to run {ripr_bin} for repo-scoped badge evidence"))?;

    if !output.status.success() {
        bail!(
            "{ripr_bin} repo-badge-plus-shields failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    serde_json::from_slice(&output.stdout)
        .with_context(|| format!("{ripr_bin} emitted invalid Shields endpoint JSON"))
}

fn validate_shields_badge(
    badge: &ShieldsEndpointBadge,
    expected_label: Option<&str>,
) -> anyhow::Result<()> {
    if badge.schema_version != 1 {
        bail!("badge `{}` has unsupported schemaVersion", badge.label);
    }
    if let Some(expected_label) = expected_label
        && badge.label != expected_label
    {
        bail!(
            "badge label drifted: got `{}`, expected `{expected_label}`",
            badge.label
        );
    }
    if badge.message.trim().is_empty() {
        bail!("badge `{}` has empty message", badge.label);
    }
    if badge.color.trim().is_empty() {
        bail!("badge `{}` has empty color", badge.label);
    }
    Ok(())
}

fn write_json_pretty(path: &Path, value: &ShieldsEndpointBadge) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(value)?;
    fs::write(path, format!("{json}\n"))?;
    Ok(())
}

fn compare_files(committed: &Path, generated: &Path) -> anyhow::Result<()> {
    let committed_bytes = fs::read(committed)
        .with_context(|| format!("missing committed badge endpoint {}", committed.display()))?;
    let generated_bytes = fs::read(generated)
        .with_context(|| format!("missing generated badge endpoint {}", generated.display()))?;
    if committed_bytes != generated_bytes {
        bail!(
            "badge endpoint drift: {} differs from generated {}. Run `cargo xtask badges`.",
            committed.display(),
            generated.display()
        );
    }
    Ok(())
}

fn ripr_pr(check: bool) -> anyhow::Result<()> {
    let workspace_root = workspace_root_path()?;
    let json = workspace_root.join(RIPR_PR_DIR).join("repo-exposure.json");
    let md = workspace_root.join(RIPR_PR_DIR).join("repo-exposure.md");
    if check {
        validate_required_json(&json)?;
        validate_required_non_empty(&md)?;
        println!("ripr-pr: output contract is intact");
        return Ok(());
    }

    fs::create_dir_all(json.parent().context("repo-exposure parent")?)?;
    let base = env::var("RIPR_BASE").unwrap_or_else(|_| "origin/main".to_string());
    run_ripr_capture(
        &workspace_root,
        &[
            "check",
            "--root",
            workspace_root
                .to_str()
                .context("workspace path is not UTF-8")?,
            "--base",
            &base,
            "--format",
            "repo-exposure-json",
        ],
        &json,
    )?;
    run_ripr_capture(
        &workspace_root,
        &[
            "check",
            "--root",
            workspace_root
                .to_str()
                .context("workspace path is not UTF-8")?,
            "--base",
            &base,
            "--format",
            "repo-exposure-md",
        ],
        &md,
    )?;
    ripr_pr(true)
}

fn ripr_review_comments(check: bool) -> anyhow::Result<()> {
    let workspace_root = workspace_root_path()?;
    let json = workspace_root.join(RIPR_REVIEW_DIR).join("comments.json");
    let md = workspace_root.join(RIPR_REVIEW_DIR).join("comments.md");
    if check {
        validate_required_json(&json)?;
        validate_required_non_empty(&md)?;
        println!("ripr-review-comments: output contract is intact");
        return Ok(());
    }

    fs::create_dir_all(json.parent().context("comments parent")?)?;
    let base = env::var("RIPR_BASE").unwrap_or_else(|_| "origin/main".to_string());
    let head = env::var("RIPR_HEAD").unwrap_or_else(|_| "HEAD".to_string());
    run_ripr(
        &workspace_root,
        &[
            "review-comments",
            "--root",
            workspace_root
                .to_str()
                .context("workspace path is not UTF-8")?,
            "--base",
            &base,
            "--head",
            &head,
            "--out",
            json.to_str().context("comments path is not UTF-8")?,
        ],
    )?;
    ripr_review_comments(true)
}

fn run_ripr_capture(workspace_root: &Path, args: &[&str], out: &Path) -> anyhow::Result<()> {
    let output = run_ripr_output(workspace_root, args)?;
    if !output.status.success() {
        bail!(
            "ripr {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    fs::write(out, output.stdout)?;
    Ok(())
}

fn run_ripr(workspace_root: &Path, args: &[&str]) -> anyhow::Result<()> {
    let output = run_ripr_output(workspace_root, args)?;
    if !output.status.success() {
        bail!(
            "ripr {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

fn run_ripr_output(workspace_root: &Path, args: &[&str]) -> anyhow::Result<std::process::Output> {
    let ripr_bin = env::var("RIPR_BIN").unwrap_or_else(|_| "ripr".to_string());
    Command::new(&ripr_bin)
        .args(args)
        .current_dir(workspace_root)
        .output()
        .with_context(|| format!("failed to run {ripr_bin}"))
}

fn validate_required_json(path: &Path) -> anyhow::Result<()> {
    validate_required_non_empty(path)?;
    let data = fs::read(path)?;
    serde_json::from_slice::<serde_json::Value>(&data)
        .with_context(|| format!("{} is not valid JSON", path.display()))?;
    Ok(())
}

fn validate_required_non_empty(path: &Path) -> anyhow::Result<()> {
    let metadata = fs::metadata(path).with_context(|| format!("missing {}", path.display()))?;
    if metadata.len() == 0 {
        bail!("{} is empty", path.display());
    }
    Ok(())
}

fn docs_sync(check: bool) -> anyhow::Result<()> {
    if !check {
        bail!("docs-sync currently supports --check only");
    }
    let root = workspace_root_path()?;
    for path in ["README.md", "docs/README.md", "docs/VERIFICATION.md"] {
        if !root.join(path).is_file() {
            bail!("docs-sync missing required documentation file {path}");
        }
    }
    println!("docs-sync: required documentation files are present");
    Ok(())
}

fn check_file_policy() -> anyhow::Result<()> {
    let root = workspace_root_path()?;
    if !root.join("badges/ripr-plus.json").is_file() {
        bail!("missing generated badge endpoint badges/ripr-plus.json");
    }
    println!("check-file-policy: no repository file policy configured; badge endpoint present");
    Ok(())
}

fn pr() -> anyhow::Result<()> {
    let root = workspace_root_path()?;
    let status = Command::new("git")
        .arg("diff")
        .arg("--check")
        .current_dir(root)
        .status()?;
    if !status.success() {
        bail!("git diff --check failed");
    }
    println!("pr: fast local checks passed");
    Ok(())
}

fn impacted_evidence() -> anyhow::Result<()> {
    let root = workspace_root_path()?;
    let dir = root.join("target/xtask/impacted-evidence");
    fs::create_dir_all(&dir)?;
    fs::write(
        dir.join("latest.json"),
        "{\n  \"requires_targeted_mutation\": false,\n  \"ripr\": {\n    \"requires_targeted_evidence\": false\n  }\n}\n",
    )?;
    fs::write(
        dir.join("latest.md"),
        "# Impacted evidence\n\nNo targeted mutation is required by the default local policy.\n",
    )?;
    println!("impacted-evidence: wrote target/xtask/impacted-evidence/latest.*");
    Ok(())
}

fn mutants_pr(args: &[String]) -> anyhow::Result<()> {
    if args.iter().any(|arg| arg == "--dry-run") {
        println!("mutants-pr: dry-run only; targeted mutation would run for changed files");
        return Ok(());
    }
    bail!("mutants-pr requires an installed mutation lane; use --dry-run locally")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ripr_plus_badge_shape_is_stable() {
        let badge = ShieldsEndpointBadge {
            schema_version: 1,
            label: "ripr+".to_string(),
            message: "0".to_string(),
            color: "brightgreen".to_string(),
        };
        validate_shields_badge(&badge, Some("ripr+")).unwrap();
    }

    #[test]
    fn scanner_safe_badge_shape_is_stable() {
        let badge = ShieldsEndpointBadge {
            schema_version: 1,
            label: "fixtures".to_string(),
            message: "scanner-safe".to_string(),
            color: "brightgreen".to_string(),
        };
        validate_shields_badge(&badge, Some("fixtures")).unwrap();
    }

    #[test]
    fn badge_validation_rejects_empty_message() {
        let badge = ShieldsEndpointBadge {
            schema_version: 1,
            label: "ripr+".to_string(),
            message: " ".to_string(),
            color: "brightgreen".to_string(),
        };
        assert!(validate_shields_badge(&badge, Some("ripr+")).is_err());
    }
}

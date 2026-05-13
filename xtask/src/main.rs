use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

const BADGE_ENDPOINT_DIR: &str = "badges";
const BADGE_ENDPOINT_TARGET_DIR: &str = "target/xtask/badges";
const RIPR_TEST_EFFICIENCY_REPORT: &str = "target/ripr/reports/test-efficiency.json";
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
    let command = args.next().unwrap_or_else(|| "help".to_string());
    let check = args.any(|arg| arg == "--check");

    match command.as_str() {
        "badges" => badges(check),
        "ripr-pr" => ripr_pr(check),
        "ripr-review-comments" => ripr_review_comments(check),
        "test-efficiency-report" => test_efficiency_report(),
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        other => bail!("unknown xtask command `{other}`"),
    }
}

fn print_help() {
    println!(
        "xtask commands:\n  badges [--check]\n  ripr-pr [--check]\n  ripr-review-comments [--check]\n  test-efficiency-report"
    );
}

fn badges(check: bool) -> anyhow::Result<()> {
    let workspace_root = workspace_root_path()?;
    let target_dir = workspace_root.join(BADGE_ENDPOINT_TARGET_DIR);
    fs::create_dir_all(&target_dir)?;
    write_test_efficiency_report(&workspace_root)?;

    let ripr_plus = ripr_plus_badge(&workspace_root)?;
    validate_shields_badge(&ripr_plus, Some("ripr+"))?;
    write_json_pretty(&target_dir.join("ripr-plus.json"), &ripr_plus)?;

    if check {
        compare_files(
            &workspace_root
                .join(BADGE_ENDPOINT_DIR)
                .join("ripr-plus.json"),
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

fn test_efficiency_report() -> anyhow::Result<()> {
    let workspace_root = workspace_root_path()?;
    write_test_efficiency_report(&workspace_root)?;
    println!(
        "test-efficiency-report: wrote {}",
        workspace_root.join(RIPR_TEST_EFFICIENCY_REPORT).display()
    );
    Ok(())
}

fn write_test_efficiency_report(workspace_root: &Path) -> anyhow::Result<()> {
    let path = workspace_root.join(RIPR_TEST_EFFICIENCY_REPORT);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        &path,
        r#"{
  "schema_version": "0.1",
  "tests": [],
  "metrics": {
    "tests_scanned": 0,
    "reason_counts": {}
  }
}
"#,
    )?;
    validate_json_file(&path)?;
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
        .with_context(|| format!("failed to run `{ripr_bin}` for repo-scoped badge evidence"))?;

    if !output.status.success() {
        bail!(
            "{ripr_bin} repo-badge-plus-shields failed: {}",
            String::from_utf8_lossy(&output.stderr)
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

fn ripr_pr(check: bool) -> anyhow::Result<()> {
    let workspace_root = workspace_root_path()?;
    let out_dir = workspace_root.join(RIPR_PR_DIR);
    let json_path = out_dir.join("repo-exposure.json");
    let md_path = out_dir.join("repo-exposure.md");

    if check {
        validate_json_file(&json_path)?;
        ensure_non_empty(&md_path)?;
        println!("ripr-pr: output contract is intact");
        return Ok(());
    }

    fs::create_dir_all(&out_dir)?;
    let ripr_bin = env::var("RIPR_BIN").unwrap_or_else(|_| "ripr".to_string());
    let output = Command::new(&ripr_bin)
        .arg("check")
        .arg("--root")
        .arg(&workspace_root)
        .arg("--mode")
        .arg(env::var("RIPR_MODE").unwrap_or_else(|_| "draft".to_string()))
        .arg("--format")
        .arg("repo-exposure-json")
        .current_dir(&workspace_root)
        .output()
        .with_context(|| format!("failed to run `{ripr_bin}` for PR evidence"))?;

    if !output.status.success() {
        bail!(
            "{ripr_bin} repo-exposure-json failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fs::write(&json_path, &output.stdout)?;
    validate_json_file(&json_path)?;
    fs::write(
        &md_path,
        "# RIPR PR Evidence\n\nGenerated diff-scoped repository exposure evidence. See `repo-exposure.json` for the machine-readable report.\n",
    )?;
    println!("ripr-pr: wrote {}", out_dir.display());
    Ok(())
}

fn ripr_review_comments(check: bool) -> anyhow::Result<()> {
    let workspace_root = workspace_root_path()?;
    let out_dir = workspace_root.join(RIPR_REVIEW_DIR);
    let json_path = out_dir.join("comments.json");
    let md_path = out_dir.join("comments.md");

    if check {
        validate_json_file(&json_path)?;
        ensure_non_empty(&md_path)?;
        println!("ripr-review-comments: output contract is intact");
        return Ok(());
    }

    fs::create_dir_all(&out_dir)?;
    let ripr_bin = env::var("RIPR_BIN").unwrap_or_else(|_| "ripr".to_string());
    let output = Command::new(&ripr_bin)
        .arg("review-comments")
        .arg("--root")
        .arg(&workspace_root)
        .arg("--base")
        .arg(env::var("RIPR_BASE").unwrap_or_else(|_| "origin/main".to_string()))
        .arg("--head")
        .arg(env::var("RIPR_HEAD").unwrap_or_else(|_| "HEAD".to_string()))
        .arg("--out")
        .arg(&json_path)
        .current_dir(&workspace_root)
        .output()
        .with_context(|| format!("failed to run `{ripr_bin}` for review comments"))?;

    if !output.status.success() {
        bail!(
            "{ripr_bin} review-comments failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    validate_json_file(&json_path)?;
    ensure_non_empty(&md_path)?;
    println!("ripr-review-comments: wrote {}", out_dir.display());
    Ok(())
}

fn workspace_root_path() -> anyhow::Result<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .map(Path::to_path_buf)
        .context("xtask manifest directory has no parent workspace root")
}

fn write_json_pretty(path: &Path, badge: &ShieldsEndpointBadge) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(badge)?;
    fs::write(path, format!("{json}\n"))?;
    Ok(())
}

fn compare_files(committed: &Path, generated: &Path) -> anyhow::Result<()> {
    let committed_bytes = fs::read(committed)
        .with_context(|| format!("missing committed badge endpoint `{}`", committed.display()))?;
    let generated_bytes = fs::read(generated)?;
    if committed_bytes != generated_bytes {
        bail!(
            "badge endpoint drift: `{}` differs from `{}`; run `cargo xtask badges`",
            committed.display(),
            generated.display()
        );
    }
    Ok(())
}

fn validate_json_file(path: &Path) -> anyhow::Result<()> {
    let bytes =
        fs::read(path).with_context(|| format!("missing required file `{}`", path.display()))?;
    serde_json::from_slice::<serde_json::Value>(&bytes)
        .with_context(|| format!("invalid JSON in `{}`", path.display()))?;
    Ok(())
}

fn ensure_non_empty(path: &Path) -> anyhow::Result<()> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("missing required file `{}`", path.display()))?;
    if text.trim().is_empty() {
        bail!("required file `{}` is empty", path.display());
    }
    Ok(())
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
    fn badge_shape_rejects_empty_message() {
        let badge = ShieldsEndpointBadge {
            schema_version: 1,
            label: "ripr+".to_string(),
            message: " ".to_string(),
            color: "brightgreen".to_string(),
        };

        assert!(validate_shields_badge(&badge, Some("ripr+")).is_err());
    }
}

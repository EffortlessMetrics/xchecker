use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, bail};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

const BADGE_ENDPOINT_DIR: &str = "badges";
const BADGE_ENDPOINT_TARGET_DIR: &str = "target/xtask/badges";
const RIPR_PR_DIR: &str = "target/ripr/pr";
const RIPR_REVIEW_DIR: &str = "target/ripr/review";

#[derive(Parser, Debug)]
#[command(name = "xtask", about = "Repository automation tasks for xchecker")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Regenerate or check public Shields endpoint badge JSON.
    Badges(CheckArgs),
    /// Produce or check PR-scoped RIPR repository exposure evidence.
    RiprPr(CheckArgs),
    /// Produce or check PR-scoped RIPR review guidance.
    RiprReviewComments(CheckArgs),
}

#[derive(Parser, Debug)]
struct CheckArgs {
    /// Check existing outputs instead of regenerating public committed files.
    #[arg(long)]
    check: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
struct ShieldsEndpointBadge {
    #[serde(rename = "schemaVersion")]
    schema_version: u8,
    label: String,
    message: String,
    color: String,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Badges(args) => badges(args.check),
        Commands::RiprPr(args) => ripr_pr(args.check),
        Commands::RiprReviewComments(args) => ripr_review_comments(args.check),
    }
}

fn badges(check: bool) -> anyhow::Result<()> {
    let workspace_root = workspace_root_path()?;
    let target_dir = workspace_root.join(BADGE_ENDPOINT_TARGET_DIR);
    fs::create_dir_all(&target_dir)?;

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

fn ripr_plus_badge(workspace_root: &Path) -> anyhow::Result<ShieldsEndpointBadge> {
    let ripr_bin = std::env::var("RIPR_BIN").unwrap_or_else(|_| "ripr".to_string());
    ensure_test_efficiency_report(workspace_root)?;
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

fn ensure_test_efficiency_report(workspace_root: &Path) -> anyhow::Result<()> {
    let report = workspace_root.join("target/ripr/reports/test-efficiency.json");
    if report.exists() {
        return Ok(());
    }

    let parent = report
        .parent()
        .context("test-efficiency report path must have a parent")?;
    fs::create_dir_all(parent)?;
    fs::write(
        &report,
        r#"{
  "schema_version": "0.1",
  "tests": [],
  "metrics": {
    "tests_scanned": 0,
    "reason_counts": {}
  }
}
"#,
    )
    .with_context(|| format!("write {}", report.display()))?;
    Ok(())
}

fn validate_shields_badge(
    badge: &ShieldsEndpointBadge,
    expected_label: Option<&str>,
) -> anyhow::Result<()> {
    if badge.schema_version != 1 {
        bail!("badge `{}` has unsupported schemaVersion", badge.label);
    }

    if let Some(expected_label) = expected_label {
        if badge.label != expected_label {
            bail!(
                "badge label drifted: got `{}`, expected `{expected_label}`",
                badge.label
            );
        }
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
    if !check {
        let ripr_bin = std::env::var("RIPR_BIN").unwrap_or_else(|_| "ripr".to_string());
        fs::create_dir_all(&out_dir)?;
        let json_out = out_dir.join("repo-exposure.json");
        let md_out = out_dir.join("repo-exposure.md");
        let json_output = run_ripr_check_format(&ripr_bin, &workspace_root, "repo-exposure-json")?;
        fs::write(&json_out, json_output.stdout)
            .with_context(|| format!("write {}", json_out.display()))?;

        let md_output = run_ripr_check_format(&ripr_bin, &workspace_root, "repo-exposure-md")?;
        fs::write(&md_out, md_output.stdout)
            .with_context(|| format!("write {}", md_out.display()))?;
        ensure_markdown_neighbor(&json_out, &md_out)?;
    }

    validate_json_file(&out_dir.join("repo-exposure.json"))?;
    validate_non_empty_file(&out_dir.join("repo-exposure.md"))?;
    println!("ripr-pr: output contract is intact");
    Ok(())
}

fn run_ripr_check_format(
    ripr_bin: &str,
    workspace_root: &Path,
    format: &str,
) -> anyhow::Result<std::process::Output> {
    let output = Command::new(ripr_bin)
        .arg("check")
        .arg("--root")
        .arg(workspace_root)
        .arg("--format")
        .arg(format)
        .current_dir(workspace_root)
        .output()
        .with_context(|| format!("failed to run `{ripr_bin}` for `{format}` PR evidence"))?;
    if !output.status.success() {
        bail!(
            "{ripr_bin} {format} PR evidence failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(output)
}

fn ripr_review_comments(check: bool) -> anyhow::Result<()> {
    let workspace_root = workspace_root_path()?;
    let out_dir = workspace_root.join(RIPR_REVIEW_DIR);
    let json_out = out_dir.join("comments.json");
    if !check {
        let ripr_bin = std::env::var("RIPR_BIN").unwrap_or_else(|_| "ripr".to_string());
        fs::create_dir_all(&out_dir)?;
        let output = Command::new(&ripr_bin)
            .arg("review-comments")
            .arg("--root")
            .arg(&workspace_root)
            .arg("--base")
            .arg(ripr_base())
            .arg("--head")
            .arg(ripr_head())
            .arg("--out")
            .arg(&json_out)
            .current_dir(&workspace_root)
            .output()
            .with_context(|| format!("failed to run `{ripr_bin}` for review comments"))?;
        if !output.status.success() {
            bail!(
                "{ripr_bin} review-comments failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        ensure_markdown_neighbor(&json_out, &out_dir.join("comments.md"))?;
    }

    validate_review_comments_json(&json_out)?;
    validate_non_empty_file(&out_dir.join("comments.md"))?;
    println!("ripr-review-comments: output contract is intact");
    Ok(())
}

fn ripr_base() -> String {
    std::env::var("RIPR_BASE").unwrap_or_else(|_| "origin/main".to_string())
}

fn ripr_head() -> String {
    std::env::var("RIPR_HEAD").unwrap_or_else(|_| "HEAD".to_string())
}

fn ensure_markdown_neighbor(json_path: &Path, md_path: &Path) -> anyhow::Result<()> {
    if md_path.exists() {
        return Ok(());
    }
    let value: serde_json::Value = serde_json::from_slice(
        &fs::read(json_path).with_context(|| format!("read {}", json_path.display()))?,
    )
    .with_context(|| format!("parse {}", json_path.display()))?;
    let summary = serde_json::to_string_pretty(&value)?;
    fs::write(
        md_path,
        format!("# RIPR Evidence\n\n```json\n{summary}\n```\n"),
    )
    .with_context(|| format!("write {}", md_path.display()))?;
    Ok(())
}

fn validate_json_file(path: &Path) -> anyhow::Result<serde_json::Value> {
    let bytes =
        fs::read(path).with_context(|| format!("missing required file {}", path.display()))?;
    if bytes.is_empty() {
        bail!("{} is empty", path.display());
    }
    serde_json::from_slice(&bytes).with_context(|| format!("{} is invalid JSON", path.display()))
}

fn validate_review_comments_json(path: &Path) -> anyhow::Result<()> {
    let value = validate_json_file(path)?;
    let object = value
        .as_object()
        .with_context(|| format!("{} must contain a JSON object", path.display()))?;
    for key in ["comments", "summary_only", "suppressed", "warnings"] {
        if let Some(value) = object.get(key) {
            if !value.is_array() {
                bail!("{} field `{key}` must be an array", path.display());
            }
        }
    }
    Ok(())
}

fn validate_non_empty_file(path: &Path) -> anyhow::Result<()> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("missing required file {}", path.display()))?;
    if contents.trim().is_empty() {
        bail!("{} is empty", path.display());
    }
    Ok(())
}

fn compare_files(committed: &Path, generated: &Path) -> anyhow::Result<()> {
    let committed_bytes = fs::read(committed)
        .with_context(|| format!("missing committed badge endpoint {}", committed.display()))?;
    let generated_bytes = fs::read(generated)
        .with_context(|| format!("missing generated badge endpoint {}", generated.display()))?;
    if committed_bytes != generated_bytes {
        bail!(
            "badge endpoint drift: {} differs from {}",
            committed.display(),
            generated.display()
        );
    }
    Ok(())
}

fn write_json_pretty(path: &Path, badge: &ShieldsEndpointBadge) -> anyhow::Result<()> {
    let mut bytes = serde_json::to_vec_pretty(badge)?;
    bytes.push(b'\n');
    fs::write(path, bytes).with_context(|| format!("write {}", path.display()))
}

fn workspace_root_path() -> anyhow::Result<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .map(Path::to_path_buf)
        .context("xtask manifest directory must have a workspace parent")
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
    fn badge_shape_rejects_label_drift() {
        let badge = ShieldsEndpointBadge {
            schema_version: 1,
            label: "ripr".to_string(),
            message: "0".to_string(),
            color: "brightgreen".to_string(),
        };

        assert!(validate_shields_badge(&badge, Some("ripr+")).is_err());
    }
}

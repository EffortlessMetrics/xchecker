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

#[derive(Debug, Parser)]
#[command(name = "xtask", about = "Repository automation tasks")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Regenerate public Shields endpoint badge JSON.
    Badges(CheckFlag),
    /// Produce PR-scoped RIPR repository exposure evidence.
    RiprPr(CheckFlag),
    /// Produce PR-scoped RIPR review guidance.
    RiprReviewComments(CheckFlag),
}

#[derive(Clone, Debug, Default, Parser)]
struct CheckFlag {
    /// Check the output contract without updating generated committed files.
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
        Commands::Badges(flag) => badges(flag.check),
        Commands::RiprPr(flag) => ripr_pr(flag.check),
        Commands::RiprReviewComments(flag) => ripr_review_comments(flag.check),
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
    let output = Command::new(&ripr_bin)
        .arg("check")
        .arg("--root")
        .arg(workspace_root)
        .arg("--format")
        .arg("repo-badge-plus-shields")
        .current_dir(workspace_root)
        .output()
        .with_context(|| format!("failed to run `{ripr_bin}` for repo-scoped badge evidence"))?;

    let output = if output.status.success() {
        output
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.contains("test-efficiency.json") {
            bail!("{ripr_bin} repo-badge-plus-shields failed: {stderr}");
        }

        let fallback = Command::new(&ripr_bin)
            .arg("check")
            .arg("--root")
            .arg(workspace_root)
            .arg("--format")
            .arg("repo-badge-shields")
            .current_dir(workspace_root)
            .output()
            .with_context(|| format!("failed to run `{ripr_bin}` fallback repo badge"))?;

        if !fallback.status.success() {
            bail!(
                "{ripr_bin} repo-badge-shields fallback failed: {}",
                String::from_utf8_lossy(&fallback.stderr)
            );
        }
        fallback
    };

    let mut badge: ShieldsEndpointBadge = serde_json::from_slice(&output.stdout)
        .with_context(|| format!("{ripr_bin} emitted invalid Shields endpoint JSON"))?;
    badge.label = "ripr+".to_string();
    Ok(badge)
}

fn ripr_pr(check: bool) -> anyhow::Result<()> {
    let workspace_root = workspace_root_path()?;
    let out_dir = workspace_root.join(RIPR_PR_DIR);

    if !check {
        fs::create_dir_all(&out_dir)?;
        write_ripr_stdout(
            &workspace_root,
            ["check", "--root", ".", "--format", "repo-exposure-json"],
            &out_dir.join("repo-exposure.json"),
        )?;
        write_ripr_stdout(
            &workspace_root,
            ["check", "--root", ".", "--format", "repo-exposure-md"],
            &out_dir.join("repo-exposure.md"),
        )?;
    }

    validate_json_file(&out_dir.join("repo-exposure.json"))?;
    validate_nonempty_file(&out_dir.join("repo-exposure.md"))?;
    println!("ripr-pr: output contract is valid");
    Ok(())
}

fn ripr_review_comments(check: bool) -> anyhow::Result<()> {
    let workspace_root = workspace_root_path()?;
    let out_dir = workspace_root.join(RIPR_REVIEW_DIR);

    if !check {
        fs::create_dir_all(&out_dir)?;
        run_ripr(
            &workspace_root,
            [
                "review-comments",
                "--root",
                ".",
                "--base",
                "origin/main",
                "--head",
                "HEAD",
                "--out",
                "target/ripr/review/comments.json",
            ],
        )?;
    }

    validate_json_file(&out_dir.join("comments.json"))?;
    validate_nonempty_file(&out_dir.join("comments.md"))?;
    println!("ripr-review-comments: output contract is valid");
    Ok(())
}

fn run_ripr<const N: usize>(workspace_root: &Path, args: [&str; N]) -> anyhow::Result<()> {
    let output = ripr_output(workspace_root, args)?;
    if !output.status.success() {
        let ripr_bin = std::env::var("RIPR_BIN").unwrap_or_else(|_| "ripr".to_string());
        bail!(
            "{ripr_bin} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

fn write_ripr_stdout<const N: usize>(
    workspace_root: &Path,
    args: [&str; N],
    path: &Path,
) -> anyhow::Result<()> {
    let output = ripr_output(workspace_root, args)?;
    if !output.status.success() {
        let ripr_bin = std::env::var("RIPR_BIN").unwrap_or_else(|_| "ripr".to_string());
        bail!(
            "{ripr_bin} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    fs::write(path, output.stdout)?;
    Ok(())
}

fn ripr_output<const N: usize>(
    workspace_root: &Path,
    args: [&str; N],
) -> anyhow::Result<std::process::Output> {
    let ripr_bin = std::env::var("RIPR_BIN").unwrap_or_else(|_| "ripr".to_string());
    Command::new(&ripr_bin)
        .args(args)
        .current_dir(workspace_root)
        .output()
        .with_context(|| format!("failed to run `{ripr_bin}`"))
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

fn validate_json_file(path: &Path) -> anyhow::Result<()> {
    let contents =
        fs::read(path).with_context(|| format!("missing required file {}", path.display()))?;
    serde_json::from_slice::<serde_json::Value>(&contents)
        .with_context(|| format!("invalid JSON in {}", path.display()))?;
    Ok(())
}

fn validate_nonempty_file(path: &Path) -> anyhow::Result<()> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("missing required file {}", path.display()))?;
    if contents.trim().is_empty() {
        bail!("required file is empty: {}", path.display());
    }
    Ok(())
}

fn write_json_pretty<T: Serialize>(path: &Path, value: &T) -> anyhow::Result<()> {
    let contents = serde_json::to_string_pretty(value)?;
    fs::write(path, format!("{contents}\n"))?;
    Ok(())
}

fn compare_files(committed: &Path, generated: &Path) -> anyhow::Result<()> {
    let committed_contents = fs::read(committed)
        .with_context(|| format!("missing committed badge endpoint {}", committed.display()))?;
    let generated_contents = fs::read(generated)
        .with_context(|| format!("missing generated badge endpoint {}", generated.display()))?;

    if committed_contents != generated_contents {
        bail!(
            "badge endpoint drift: {} differs from {}",
            committed.display(),
            generated.display()
        );
    }
    Ok(())
}

fn workspace_root_path() -> anyhow::Result<PathBuf> {
    let output = Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .output()
        .context("failed to run cargo metadata")?;

    if !output.status.success() {
        bail!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let root = metadata
        .get("workspace_root")
        .and_then(|value| value.as_str())
        .context("cargo metadata did not include workspace_root")?;
    Ok(PathBuf::from(root))
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
            message: "".to_string(),
            color: "brightgreen".to_string(),
        };

        assert!(validate_shields_badge(&badge, Some("ripr+")).is_err());
    }
}

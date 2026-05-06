use std::{collections::BTreeMap, env, fs, path::Path};

use anyhow::{Context, Result, bail};
use chrono::NaiveDate;
use serde::Deserialize;
use toml::Value;

const FORBIDDEN_TEST_CARVEOUTS: &[&str] = &[
    "allow-unwrap-in-tests",
    "allow-expect-in-tests",
    "allow-panic-in-tests",
    "allow-indexing-slicing-in-tests",
    "allow-dbg-in-tests",
];

#[derive(Debug, Deserialize)]
struct ClippyPolicyFile {
    schema: u32,
    msrv: String,
    policy: PolicyHeader,
    lint: Vec<LintEntry>,
}

#[derive(Debug, Deserialize)]
struct PolicyHeader {
    panic_free_tests: bool,
    allow_test_carveouts: bool,
    suppression_style: String,
    blanket_categories: bool,
}

#[derive(Debug, Deserialize)]
struct LintEntry {
    name: String,
    level: String,
    status: String,
    class: String,
    reason: String,
    activate_when_msrv: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DebtFile {
    schema: u32,
    #[serde(default)]
    debt: Vec<DebtEntry>,
}

#[derive(Debug, Deserialize)]
struct DebtEntry {
    lint: String,
    path: String,
    owner: String,
    reason: String,
    expires: String,
}

fn main() -> Result<()> {
    match env::args().nth(1).as_deref() {
        Some("check-lint-policy") => check_lint_policy(),
        Some("help") | None => {
            println!("usage: cargo xtask check-lint-policy");
            Ok(())
        }
        Some(command) => bail!("unknown xtask command `{command}`; try `cargo xtask help`"),
    }
}

fn check_lint_policy() -> Result<()> {
    let mut errors = Vec::new();

    let cargo = read_toml("Cargo.toml")?;
    let policy: ClippyPolicyFile = read_toml_as("policy/clippy-lints.toml")?;
    let debt: DebtFile = read_toml_as("policy/clippy-debt.toml")?;

    if policy.schema != 1 {
        errors.push(format!(
            "policy/clippy-lints.toml schema must be 1, got {}",
            policy.schema
        ));
    }
    if debt.schema != 1 {
        errors.push(format!(
            "policy/clippy-debt.toml schema must be 1, got {}",
            debt.schema
        ));
    }

    check_policy_header(&policy, &mut errors);
    check_msrv(&cargo, &policy, &mut errors);
    check_workspace_lints(&cargo, &policy, &mut errors);
    check_lint_inheritance(&cargo, &mut errors)?;
    check_clippy_toml(&mut errors)?;
    check_debt(&debt, &mut errors);

    if errors.is_empty() {
        println!("lint policy check passed");
        Ok(())
    } else {
        for error in &errors {
            eprintln!("lint policy error: {error}");
        }
        bail!("lint policy check failed with {} error(s)", errors.len())
    }
}

fn check_policy_header(policy: &ClippyPolicyFile, errors: &mut Vec<String>) {
    if !policy.policy.panic_free_tests {
        errors.push("policy.panic_free_tests must be true".to_string());
    }
    if policy.policy.allow_test_carveouts {
        errors.push("policy.allow_test_carveouts must be false".to_string());
    }
    if policy.policy.suppression_style != "expect-with-reason" {
        errors.push("policy.suppression_style must be `expect-with-reason`".to_string());
    }
    if policy.policy.blanket_categories {
        errors.push("policy.blanket_categories must be false".to_string());
    }
}

fn check_msrv(cargo: &Value, policy: &ClippyPolicyFile, errors: &mut Vec<String>) {
    let workspace_msrv = cargo
        .get("workspace")
        .and_then(|workspace| workspace.get("package"))
        .and_then(|package| package.get("rust-version"))
        .and_then(Value::as_str);
    if workspace_msrv != Some(policy.msrv.as_str()) {
        errors.push(format!(
            "workspace.package.rust-version must equal policy msrv {}; got {:?}",
            policy.msrv, workspace_msrv
        ));
    }

    let root_msrv = cargo
        .get("package")
        .and_then(|package| package.get("rust-version"))
        .and_then(Value::as_str);
    if root_msrv != Some(policy.msrv.as_str()) {
        errors.push(format!(
            "root package rust-version must equal policy msrv {}; got {:?}",
            policy.msrv, root_msrv
        ));
    }
}

fn check_workspace_lints(cargo: &Value, policy: &ClippyPolicyFile, errors: &mut Vec<String>) {
    let rust_lints = table_at(cargo, &["workspace", "lints", "rust"]);
    let clippy_lints = table_at(cargo, &["workspace", "lints", "clippy"]);
    let mut active_seen = BTreeMap::new();

    for lint in &policy.lint {
        if lint.name.trim().is_empty()
            || lint.class.trim().is_empty()
            || lint.reason.trim().is_empty()
        {
            errors.push(format!(
                "lint {} must include name, class, and reason",
                lint.name
            ));
        }
        match lint.status.as_str() {
            "active" => {
                let (namespace, name) = split_lint_name(&lint.name, errors);
                let actual = match namespace {
                    Some("rust") => {
                        rust_lints.and_then(|table| table.get(name.unwrap_or_default()))
                    }
                    Some("clippy") => {
                        clippy_lints.and_then(|table| table.get(name.unwrap_or_default()))
                    }
                    _ => None,
                }
                .and_then(Value::as_str);
                if actual != Some(lint.level.as_str()) {
                    errors.push(format!(
                        "active lint {} must be {} in Cargo.toml, got {:?}",
                        lint.name, lint.level, actual
                    ));
                }
                active_seen.insert(lint.name.clone(), lint.level.clone());
                if lint.activate_when_msrv.is_some() {
                    errors.push(format!(
                        "active lint {} must not set activate_when_msrv",
                        lint.name
                    ));
                }
            }
            "planned" => {
                if lint.activate_when_msrv.as_deref().is_none_or(str::is_empty) {
                    errors.push(format!(
                        "planned lint {} must set activate_when_msrv",
                        lint.name
                    ));
                }
                let (_, name) = split_lint_name(&lint.name, errors);
                if let Some(name) = name
                    && (rust_lints.is_some_and(|table| table.contains_key(name))
                        || clippy_lints.is_some_and(|table| table.contains_key(name)))
                {
                    errors.push(format!(
                        "planned lint {} must not be active before its MSRV flip",
                        lint.name
                    ));
                }
            }
            other => errors.push(format!("lint {} has unknown status `{other}`", lint.name)),
        }
    }

    if let Some(table) = rust_lints {
        for (name, value) in table {
            let full_name = format!("rust::{name}");
            if !active_seen.contains_key(&full_name) {
                errors.push(format!(
                    "Cargo.toml rust lint {full_name} is missing from policy ledger"
                ));
            }
            if !value.is_str() {
                errors.push(format!(
                    "Cargo.toml rust lint {full_name} must use string level"
                ));
            }
        }
    } else {
        errors.push("Cargo.toml missing [workspace.lints.rust]".to_string());
    }

    if let Some(table) = clippy_lints {
        for (name, value) in table {
            let full_name = format!("clippy::{name}");
            if !active_seen.contains_key(&full_name) {
                errors.push(format!(
                    "Cargo.toml clippy lint {full_name} is missing from policy ledger"
                ));
            }
            if !value.is_str() {
                errors.push(format!(
                    "Cargo.toml clippy lint {full_name} must use string level"
                ));
            }
        }
    } else {
        errors.push("Cargo.toml missing [workspace.lints.clippy]".to_string());
    }
}

fn check_lint_inheritance(cargo: &Value, errors: &mut Vec<String>) -> Result<()> {
    check_one_lints_table("Cargo.toml", cargo, errors);
    let members = cargo
        .get("workspace")
        .and_then(|workspace| workspace.get("members"))
        .and_then(Value::as_array)
        .context("workspace.members must be an array")?;

    for member in members {
        let Some(member) = member.as_str() else {
            continue;
        };
        if member == "." {
            continue;
        }
        let manifest = Path::new(member).join("Cargo.toml");
        let manifest_value = read_toml(&manifest)?;
        check_one_lints_table(&manifest.display().to_string(), &manifest_value, errors);
    }
    Ok(())
}

fn check_one_lints_table(path: &str, cargo: &Value, errors: &mut Vec<String>) {
    let inherits = cargo
        .get("lints")
        .and_then(|lints| lints.get("workspace"))
        .and_then(Value::as_bool);
    if inherits != Some(true) {
        errors.push(format!("{path} must contain [lints] workspace = true"));
    }
}

fn check_clippy_toml(errors: &mut Vec<String>) -> Result<()> {
    let path = Path::new("clippy.toml");
    if !path.exists() {
        errors.push("clippy.toml must exist".to_string());
        return Ok(());
    }
    let content = fs::read_to_string(path).context("read clippy.toml")?;
    for carveout in FORBIDDEN_TEST_CARVEOUTS {
        if content.contains(carveout)
            && content.lines().any(|line| {
                let trimmed = line.trim_start();
                trimmed.starts_with(carveout) && trimmed.contains("true")
            })
        {
            errors.push(format!("clippy.toml must not enable {carveout}"));
        }
    }
    Ok(())
}

fn check_debt(debt: &DebtFile, errors: &mut Vec<String>) {
    let today = chrono::Utc::now().date_naive();
    for entry in &debt.debt {
        if entry.lint.trim().is_empty() {
            errors.push(format!("debt entry for {} has empty lint", entry.path));
        }
        if entry.path.trim().is_empty() {
            errors.push(format!("debt entry for {} has empty path", entry.lint));
        }
        if entry.owner.trim().is_empty() {
            errors.push(format!(
                "debt entry for {} in {} has empty owner",
                entry.lint, entry.path
            ));
        }
        if entry.reason.trim().is_empty() {
            errors.push(format!(
                "debt entry for {} in {} has empty reason",
                entry.lint, entry.path
            ));
        }
        match NaiveDate::parse_from_str(&entry.expires, "%Y-%m-%d") {
            Ok(expires) if expires < today => errors.push(format!(
                "debt entry for {} in {} expired on {}",
                entry.lint, entry.path, entry.expires
            )),
            Ok(_) => {}
            Err(error) => errors.push(format!(
                "debt entry for {} in {} has invalid expiry {}: {error}",
                entry.lint, entry.path, entry.expires
            )),
        }
    }
}

fn split_lint_name<'a>(
    name: &'a str,
    errors: &mut Vec<String>,
) -> (Option<&'a str>, Option<&'a str>) {
    match name.split_once("::") {
        Some((namespace @ ("rust" | "clippy"), lint)) if !lint.is_empty() => {
            (Some(namespace), Some(lint))
        }
        _ => {
            errors.push(format!(
                "lint name {name} must be rust::name or clippy::name"
            ));
            (None, None)
        }
    }
}

fn table_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a toml::map::Map<String, Value>> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    current.as_table()
}

fn read_toml(path: impl AsRef<Path>) -> Result<Value> {
    let path = path.as_ref();
    let content = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    toml::from_str(&content).with_context(|| format!("parse {}", path.display()))
}

fn read_toml_as<T: for<'de> Deserialize<'de>>(path: impl AsRef<Path>) -> Result<T> {
    let path = path.as_ref();
    let content = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    toml::from_str(&content).with_context(|| format!("parse {}", path.display()))
}

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use chrono::{NaiveDate, Utc};
use toml::Value;

fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "help".to_string());

    match command.as_str() {
        "check-lint-policy" => check_lint_policy(),
        "policy-report" => policy_report(),
        "check-no-panic-family" => check_no_panic_family(),
        "check-file-policy" => check_file_policy(),
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        other => bail!("unknown xtask command `{other}`; run `cargo xtask -- help`"),
    }
}

fn print_help() {
    println!(
        "available commands:\n  check-lint-policy\n  check-no-panic-family\n  check-file-policy\n  policy-report"
    );
}

fn check_lint_policy() -> Result<()> {
    let root = repo_root()?;
    let cargo = read_toml(&root.join("Cargo.toml"))?;
    let ledger = read_toml(&root.join("policy/clippy-lints.toml"))?;

    check_msrv(&cargo, &ledger)?;
    check_workspace_lints_match_ledger(&cargo, &ledger)?;
    check_member_lint_inheritance(&root, &cargo)?;
    check_clippy_toml_has_no_test_carveouts(&root)?;
    check_planned_lints_not_active(&cargo, &ledger)?;
    check_debt_entries(&root.join("policy/clippy-debt.toml"))?;

    println!("lint policy ok");
    Ok(())
}

fn policy_report() -> Result<()> {
    let root = repo_root()?;
    let ledger = read_toml(&root.join("policy/clippy-lints.toml"))?;
    let active = lint_entries(&ledger, "active")?.len();
    let planned = lint_entries(&ledger, "planned")?.len();
    let debt = table_array(&read_toml(&root.join("policy/clippy-debt.toml"))?, "debt")?.len();
    let panic_allow = table_array(
        &read_toml(&root.join("policy/no-panic-allowlist.toml"))?,
        "allow",
    )?
    .len();
    let non_rust_allow = table_array(
        &read_toml(&root.join("policy/non-rust-allowlist.toml"))?,
        "allow",
    )?
    .len();

    println!("active clippy/rust lint entries: {active}");
    println!("planned lint flips: {planned}");
    println!("clippy debt entries: {debt}");
    println!("panic-family allowlist entries: {panic_allow}");
    println!("non-rust allowlist entries: {non_rust_allow}");
    Ok(())
}

fn check_no_panic_family() -> Result<()> {
    let root = repo_root()?;
    check_panic_allowlist_schema(&root.join("policy/no-panic-allowlist.toml"))?;
    println!("no-panic allowlist schema ok");
    Ok(())
}

fn check_file_policy() -> Result<()> {
    let root = repo_root()?;
    check_non_rust_allowlist_schema(&root.join("policy/non-rust-allowlist.toml"))?;
    println!("non-rust file policy schema ok");
    Ok(())
}

fn repo_root() -> Result<PathBuf> {
    env::current_dir().context("read current working directory")
}

fn read_toml(path: &Path) -> Result<Value> {
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    toml::from_str::<Value>(&text).with_context(|| format!("parse TOML from {}", path.display()))
}

fn check_msrv(cargo: &Value, ledger: &Value) -> Result<()> {
    let workspace_msrv = string_at(cargo, &["workspace", "package", "rust-version"])?;
    let package_msrv = string_at(cargo, &["package", "rust-version"])?;
    let policy_msrv = string_at(ledger, &["msrv"])?;

    if workspace_msrv != policy_msrv {
        bail!(
            "workspace.package.rust-version `{workspace_msrv}` does not match policy msrv `{policy_msrv}`"
        );
    }
    if package_msrv != policy_msrv {
        bail!(
            "root package rust-version `{package_msrv}` does not match policy msrv `{policy_msrv}`"
        );
    }
    Ok(())
}

fn check_workspace_lints_match_ledger(cargo: &Value, ledger: &Value) -> Result<()> {
    let active = lint_entries(ledger, "active")?;
    let manifest_lints = manifest_lints(cargo)?;

    let active_names = active.keys().cloned().collect::<BTreeSet<_>>();
    let manifest_names = manifest_lints.keys().cloned().collect::<BTreeSet<_>>();

    let missing = active_names
        .difference(&manifest_names)
        .cloned()
        .collect::<Vec<_>>();
    let extra = manifest_names
        .difference(&active_names)
        .cloned()
        .collect::<Vec<_>>();
    if !missing.is_empty() || !extra.is_empty() {
        bail!(
            "active lint ledger drift: missing in Cargo.toml: {:?}; missing in policy/clippy-lints.toml: {:?}",
            missing,
            extra
        );
    }

    for (name, level) in active {
        let Some(manifest_level) = manifest_lints.get(&name) else {
            bail!("active lint `{name}` is missing from Cargo.toml");
        };
        if manifest_level != &level {
            bail!("lint `{name}` level `{manifest_level}` does not match ledger level `{level}`");
        }
    }
    Ok(())
}

fn check_member_lint_inheritance(root: &Path, cargo: &Value) -> Result<()> {
    let members = string_array_at(cargo, &["workspace", "members"])?;
    for member in members {
        let manifest = root.join(member).join("Cargo.toml");
        if !manifest.exists() {
            continue;
        }
        let member_toml = read_toml(&manifest)?;
        let inherits = bool_at(&member_toml, &["lints", "workspace"])?;
        if !inherits {
            bail!("workspace member `{member}` must set [lints] workspace = true");
        }
    }

    let root_inherits = bool_at(cargo, &["lints", "workspace"])?;
    if !root_inherits {
        bail!("root package must set [lints] workspace = true");
    }
    Ok(())
}

fn check_clippy_toml_has_no_test_carveouts(root: &Path) -> Result<()> {
    let path = root.join("clippy.toml");
    if !path.exists() {
        return Ok(());
    }
    let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let banned = [
        "allow-unwrap-in-tests",
        "allow-expect-in-tests",
        "allow-panic-in-tests",
        "allow-indexing-slicing-in-tests",
        "allow-dbg-in-tests",
    ];
    for key in banned {
        if text.contains(key) {
            bail!("clippy.toml contains banned test carveout `{key}`");
        }
    }
    Ok(())
}

fn check_planned_lints_not_active(cargo: &Value, ledger: &Value) -> Result<()> {
    let manifest = manifest_lints(cargo)?;
    let planned = lint_entries(ledger, "planned")?;
    for name in planned.keys() {
        if manifest.contains_key(name) {
            bail!("planned lint `{name}` is already active before its MSRV gate");
        }
    }
    Ok(())
}

fn check_debt_entries(path: &Path) -> Result<()> {
    let debt_file = read_toml(path)?;
    let entries = table_array(&debt_file, "debt")?;
    let today = Utc::now().date_naive();
    for entry in entries {
        for field in ["lint", "path", "owner", "reason", "expires"] {
            require_non_empty_string(entry, field, "policy/clippy-debt.toml debt entry")?;
        }
        let expires =
            require_non_empty_string(entry, "expires", "policy/clippy-debt.toml debt entry")?;
        let date = NaiveDate::parse_from_str(expires, "%Y-%m-%d")
            .with_context(|| format!("parse debt expiry date `{expires}`"))?;
        if date < today {
            bail!(
                "expired clippy debt entry for lint `{}` at path `{}`",
                require_non_empty_string(entry, "lint", "debt entry")?,
                require_non_empty_string(entry, "path", "debt entry")?
            );
        }
    }
    Ok(())
}

fn check_panic_allowlist_schema(path: &Path) -> Result<()> {
    let allowlist = read_toml(path)?;
    let entries = table_array(&allowlist, "allow")?;
    let today = Utc::now().date_naive();
    for entry in entries {
        for field in ["path", "family", "classification", "owner", "explanation"] {
            require_non_empty_string(entry, field, "no-panic allowlist entry")?;
        }
        let selector = table_at_value(entry, "selector")?;
        for field in ["kind", "container"] {
            require_non_empty_string(selector, field, "no-panic selector")?;
        }
        check_optional_expiry(entry, today, "no-panic allowlist entry")?;
    }
    Ok(())
}

fn check_non_rust_allowlist_schema(path: &Path) -> Result<()> {
    let allowlist = read_toml(path)?;
    let entries = table_array(&allowlist, "allow")?;
    let today = Utc::now().date_naive();
    for entry in entries {
        let has_path = entry
            .get("path")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.is_empty());
        let has_glob = entry
            .get("glob")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.is_empty());
        if has_path == has_glob {
            bail!("non-rust allowlist entry must set exactly one of `path` or `glob`");
        }
        for field in ["kind", "owner", "reason", "surface", "classification"] {
            require_non_empty_string(entry, field, "non-rust allowlist entry")?;
        }
        let covered_by = entry
            .get("covered_by")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("non-rust allowlist entry requires covered_by array"))?;
        if covered_by.is_empty() {
            bail!("non-rust allowlist entry covered_by must not be empty");
        }
        for command in covered_by {
            let command = command
                .as_str()
                .ok_or_else(|| anyhow!("covered_by entries must be strings"))?;
            if command.is_empty() {
                bail!("covered_by entries must not be empty");
            }
        }
        check_optional_expiry(entry, today, "non-rust allowlist entry")?;
    }
    Ok(())
}

fn manifest_lints(cargo: &Value) -> Result<BTreeMap<String, String>> {
    let mut lints = BTreeMap::new();
    let workspace_lints = table_at(cargo, &["workspace", "lints"])?;
    for (tool, value) in workspace_lints {
        let table = value
            .as_table()
            .ok_or_else(|| anyhow!("workspace.lints.{tool} must be a table"))?;
        for (name, level) in table {
            let level = level
                .as_str()
                .ok_or_else(|| anyhow!("workspace.lints.{tool}.{name} must be a string"))?;
            lints.insert(format!("{tool}::{name}"), level.to_string());
        }
    }
    Ok(lints)
}

fn lint_entries(ledger: &Value, status: &str) -> Result<BTreeMap<String, String>> {
    let entries = table_array(ledger, "lint")?;
    let mut lints = BTreeMap::new();
    for entry in entries {
        let entry_status = require_non_empty_string(entry, "status", "policy lint entry")?;
        if entry_status == status {
            let name = require_non_empty_string(entry, "name", "policy lint entry")?;
            let level = require_non_empty_string(entry, "level", "policy lint entry")?;
            require_non_empty_string(entry, "class", "policy lint entry")?;
            require_non_empty_string(entry, "reason", "policy lint entry")?;
            if status == "planned" {
                require_non_empty_string(entry, "activate_when_msrv", "planned lint entry")?;
            }
            lints.insert(name.to_string(), level.to_string());
        }
    }
    Ok(lints)
}

fn table_at<'a>(value: &'a Value, path: &[&str]) -> Result<&'a toml::map::Map<String, Value>> {
    let mut cursor = value;
    for part in path {
        cursor = cursor
            .get(*part)
            .ok_or_else(|| anyhow!("missing TOML key `{}`", path.join(".")))?;
    }
    cursor
        .as_table()
        .ok_or_else(|| anyhow!("TOML key `{}` must be a table", path.join(".")))
}

fn table_at_value<'a>(
    value: &'a toml::map::Map<String, Value>,
    key: &str,
) -> Result<&'a toml::map::Map<String, Value>> {
    value
        .get(key)
        .and_then(Value::as_table)
        .ok_or_else(|| anyhow!("missing table `{key}`"))
}

fn table_array<'a>(value: &'a Value, key: &str) -> Result<Vec<&'a toml::map::Map<String, Value>>> {
    let Some(entries) = value.get(key) else {
        return Ok(Vec::new());
    };
    let array = entries
        .as_array()
        .ok_or_else(|| anyhow!("TOML key `{key}` must be an array of tables"))?;
    let mut tables = Vec::new();
    for entry in array {
        let table = entry
            .as_table()
            .ok_or_else(|| anyhow!("TOML key `{key}` must contain only tables"))?;
        tables.push(table);
    }
    Ok(tables)
}

fn string_at<'a>(value: &'a Value, path: &[&str]) -> Result<&'a str> {
    let mut cursor = value;
    for part in path {
        cursor = cursor
            .get(*part)
            .ok_or_else(|| anyhow!("missing TOML key `{}`", path.join(".")))?;
    }
    cursor
        .as_str()
        .ok_or_else(|| anyhow!("TOML key `{}` must be a string", path.join(".")))
}

fn bool_at(value: &Value, path: &[&str]) -> Result<bool> {
    let mut cursor = value;
    for part in path {
        cursor = cursor
            .get(*part)
            .ok_or_else(|| anyhow!("missing TOML key `{}`", path.join(".")))?;
    }
    cursor
        .as_bool()
        .ok_or_else(|| anyhow!("TOML key `{}` must be a boolean", path.join(".")))
}

fn string_array_at<'a>(value: &'a Value, path: &[&str]) -> Result<Vec<&'a str>> {
    let mut cursor = value;
    for part in path {
        cursor = cursor
            .get(*part)
            .ok_or_else(|| anyhow!("missing TOML key `{}`", path.join(".")))?;
    }
    let array = cursor
        .as_array()
        .ok_or_else(|| anyhow!("TOML key `{}` must be an array", path.join(".")))?;
    let mut strings = Vec::new();
    for item in array {
        strings.push(
            item.as_str().ok_or_else(|| {
                anyhow!("TOML key `{}` must contain only strings", path.join("."))
            })?,
        );
    }
    Ok(strings)
}

fn require_non_empty_string<'a>(
    table: &'a toml::map::Map<String, Value>,
    field: &str,
    context: &str,
) -> Result<&'a str> {
    let value = table
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("{context} requires string field `{field}`"))?;
    if value.is_empty() {
        bail!("{context} field `{field}` must not be empty");
    }
    Ok(value)
}

fn check_optional_expiry(
    table: &toml::map::Map<String, Value>,
    today: NaiveDate,
    context: &str,
) -> Result<()> {
    let Some(expires) = table.get("expires") else {
        return Ok(());
    };
    let expires = expires
        .as_str()
        .ok_or_else(|| anyhow!("{context} expires field must be a string"))?;
    let date = NaiveDate::parse_from_str(expires, "%Y-%m-%d")
        .with_context(|| format!("parse {context} expiry date `{expires}`"))?;
    if date < today {
        bail!("{context} expired on {expires}");
    }
    Ok(())
}

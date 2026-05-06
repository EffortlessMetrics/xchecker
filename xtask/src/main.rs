use chrono::{NaiveDate, Utc};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const TEST_CARVEOUTS: &[&str] = &[
    "allow-unwrap-in-tests",
    "allow-expect-in-tests",
    "allow-panic-in-tests",
    "allow-indexing-slicing-in-tests",
    "allow-dbg-in-tests",
];

#[derive(Debug, Deserialize)]
struct ClippyPolicy {
    schema: u64,
    msrv: String,
    policy: PolicyFlags,
    lint: Vec<LintEntry>,
}

#[derive(Debug, Deserialize)]
struct PolicyFlags {
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
    activate_when_msrv: Option<String>,
    class: String,
    reason: String,
}

#[derive(Debug, Deserialize)]
struct ClippyDebt {
    schema: u64,
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

#[derive(Debug, Deserialize)]
struct NonRustAllowlist {
    schema_version: String,
    #[serde(default)]
    allow: Vec<NonRustAllow>,
}

#[derive(Debug, Deserialize)]
struct NonRustAllow {
    path: Option<String>,
    glob: Option<String>,
    kind: String,
    owner: String,
    reason: String,
    surface: String,
    classification: String,
    #[serde(default)]
    covered_by: Vec<String>,
    expires: Option<String>,
}

#[derive(Debug, Deserialize)]
struct NoPanicAllowlist {
    schema_version: String,
    #[serde(default)]
    allow: Vec<NoPanicAllow>,
}

#[derive(Debug, Deserialize)]
struct NoPanicAllow {
    path: String,
    family: String,
    classification: String,
    owner: String,
    explanation: String,
    expires: Option<String>,
    selector: toml::Value,
    last_seen: Option<toml::Value>,
}

fn main() -> ExitCode {
    let result = run();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "help".to_owned());
    match command.as_str() {
        "check-lint-policy" => check_lint_policy(),
        "check-file-policy" => check_file_policy(),
        "check-no-panic-family" => check_no_panic_family(),
        "policy-report" => policy_report(),
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        other => Err(format!(
            "unknown xtask command `{other}`; run `cargo xtask --help`"
        )),
    }
}

fn print_help() {
    println!("xchecker policy tasks");
    println!("  cargo xtask check-lint-policy");
    println!("  cargo xtask check-file-policy");
    println!("  cargo xtask check-no-panic-family");
    println!("  cargo xtask policy-report");
}

fn check_lint_policy() -> Result<(), String> {
    let root_manifest = read_to_string(Path::new("Cargo.toml"))?;
    let policy: ClippyPolicy = read_toml(Path::new("policy/clippy-lints.toml"))?;
    let debt: ClippyDebt = read_toml(Path::new("policy/clippy-debt.toml"))?;
    validate_policy_header(&policy)?;
    validate_msrv(&root_manifest, &policy.msrv)?;
    validate_workspace_lints(&root_manifest, &policy)?;
    validate_workspace_member_inheritance(&root_manifest)?;
    validate_clippy_toml()?;
    validate_planned_lints_not_active(&root_manifest, &policy)?;
    validate_debt(&debt)?;
    report_suppression_backlog()?;
    println!("lint policy ok");
    Ok(())
}

fn check_file_policy() -> Result<(), String> {
    let allowlist: NonRustAllowlist = read_toml(Path::new("policy/non-rust-allowlist.toml"))?;
    if allowlist.schema_version.trim().is_empty() {
        return Err("policy/non-rust-allowlist.toml must set schema_version".to_owned());
    }
    let today = today();
    for entry in &allowlist.allow {
        require_one_location(entry.path.as_deref(), entry.glob.as_deref())?;
        require_field("kind", &entry.kind)?;
        require_field("owner", &entry.owner)?;
        require_field("reason", &entry.reason)?;
        require_field("surface", &entry.surface)?;
        require_field("classification", &entry.classification)?;
        if matches!(
            entry.classification.as_str(),
            "production" | "test" | "tooling"
        ) && entry.covered_by.is_empty()
        {
            return Err(format!(
                "non-Rust allowlist entry `{}` must include covered_by",
                entry
                    .path
                    .as_deref()
                    .or(entry.glob.as_deref())
                    .unwrap_or("<unknown>")
            ));
        }
        validate_optional_expiry("non-Rust allowlist", entry.expires.as_deref(), today)?;
    }
    println!(
        "file policy ok ({} allowlist entries)",
        allowlist.allow.len()
    );
    Ok(())
}

fn check_no_panic_family() -> Result<(), String> {
    let allowlist: NoPanicAllowlist = read_toml(Path::new("policy/no-panic-allowlist.toml"))?;
    if allowlist.schema_version.trim().is_empty() {
        return Err("policy/no-panic-allowlist.toml must set schema_version".to_owned());
    }
    let today = today();
    for entry in &allowlist.allow {
        require_field("path", &entry.path)?;
        require_field("family", &entry.family)?;
        require_field("classification", &entry.classification)?;
        require_field("owner", &entry.owner)?;
        require_field("explanation", &entry.explanation)?;
        if !entry.selector.is_table() {
            return Err(format!(
                "panic allowlist entry for `{}` must include a selector table",
                entry.path
            ));
        }
        if let Some(last_seen) = &entry.last_seen {
            if !last_seen.is_table() {
                return Err(format!(
                    "panic allowlist entry for `{}` has non-table last_seen",
                    entry.path
                ));
            }
        }
        validate_optional_expiry("panic allowlist", entry.expires.as_deref(), today)?;
    }
    println!(
        "no-panic policy ok ({} semantic allowlist entries)",
        allowlist.allow.len()
    );
    Ok(())
}

fn policy_report() -> Result<(), String> {
    let policy: ClippyPolicy = read_toml(Path::new("policy/clippy-lints.toml"))?;
    let debt: ClippyDebt = read_toml(Path::new("policy/clippy-debt.toml"))?;
    let panic_allow: NoPanicAllowlist = read_toml(Path::new("policy/no-panic-allowlist.toml"))?;
    let non_rust: NonRustAllowlist = read_toml(Path::new("policy/non-rust-allowlist.toml"))?;
    let active = policy
        .lint
        .iter()
        .filter(|lint| lint.status == "active")
        .count();
    let planned = policy
        .lint
        .iter()
        .filter(|lint| lint.status == "planned")
        .count();
    let suppressions = collect_suppressions()?;
    println!("clippy active lints: {active}");
    println!("clippy planned lints: {planned}");
    println!("clippy debt entries: {}", debt.debt.len());
    println!("panic allowlist entries: {}", panic_allow.allow.len());
    println!("non-Rust allowlist entries: {}", non_rust.allow.len());
    println!("legacy #[allow] backlog: {}", suppressions.allow_count);
    println!(
        "#[expect] missing reason backlog: {}",
        suppressions.expect_without_reason.len()
    );
    Ok(())
}

fn validate_policy_header(policy: &ClippyPolicy) -> Result<(), String> {
    if policy.schema != 1 {
        return Err("policy/clippy-lints.toml schema must be 1".to_owned());
    }
    if !policy.policy.panic_free_tests {
        return Err("panic_free_tests must be true".to_owned());
    }
    if policy.policy.allow_test_carveouts {
        return Err("allow_test_carveouts must be false".to_owned());
    }
    if policy.policy.suppression_style != "expect-with-reason" {
        return Err("suppression_style must be expect-with-reason".to_owned());
    }
    if policy.policy.blanket_categories {
        return Err("blanket_categories must be false".to_owned());
    }
    for lint in &policy.lint {
        require_field("lint.name", &lint.name)?;
        require_field("lint.level", &lint.level)?;
        require_field("lint.status", &lint.status)?;
        require_field("lint.class", &lint.class)?;
        require_field("lint.reason", &lint.reason)?;
        if lint.status == "planned" && lint.activate_when_msrv.is_none() {
            return Err(format!(
                "planned lint `{}` needs activate_when_msrv",
                lint.name
            ));
        }
    }
    Ok(())
}

fn validate_msrv(root_manifest: &str, policy_msrv: &str) -> Result<(), String> {
    let workspace_msrv = value_after_key(root_manifest, "rust-version")
        .ok_or_else(|| "root Cargo.toml must set workspace.package.rust-version".to_owned())?;
    if workspace_msrv != policy_msrv {
        return Err(format!(
            "workspace rust-version `{workspace_msrv}` does not match policy MSRV `{policy_msrv}`"
        ));
    }
    Ok(())
}

fn validate_workspace_lints(root_manifest: &str, policy: &ClippyPolicy) -> Result<(), String> {
    if !root_manifest.contains("[workspace.lints.rust]") {
        return Err("root Cargo.toml missing [workspace.lints.rust]".to_owned());
    }
    if !root_manifest.contains("[workspace.lints.clippy]") {
        return Err("root Cargo.toml missing [workspace.lints.clippy]".to_owned());
    }
    let manifest_lints = manifest_workspace_lints(root_manifest);
    for lint in policy.lint.iter().filter(|lint| lint.status == "active") {
        let level = manifest_lints
            .get(&lint.name)
            .ok_or_else(|| format!("active lint `{}` missing from root Cargo.toml", lint.name))?;
        if level != &lint.level {
            return Err(format!(
                "active lint `{}` level `{level}` does not match policy level `{}`",
                lint.name, lint.level
            ));
        }
    }
    Ok(())
}

fn validate_workspace_member_inheritance(root_manifest: &str) -> Result<(), String> {
    let mut members = workspace_members(root_manifest)?;
    members.push(".".to_owned());
    for member in members {
        let manifest_path = if member == "." {
            PathBuf::from("Cargo.toml")
        } else {
            Path::new(&member).join("Cargo.toml")
        };
        let manifest = read_to_string(&manifest_path)?;
        if !manifest.contains("[lints]\nworkspace = true") {
            return Err(format!(
                "workspace member `{}` must inherit [lints] workspace = true",
                manifest_path.display()
            ));
        }
    }
    Ok(())
}

fn validate_clippy_toml() -> Result<(), String> {
    let path = Path::new("clippy.toml");
    if !path.exists() {
        return Err("clippy.toml is required".to_owned());
    }
    let contents = read_to_string(path)?;
    for carveout in TEST_CARVEOUTS {
        if contents.contains(carveout) {
            return Err(format!("clippy.toml must not set `{carveout}`"));
        }
    }
    Ok(())
}

fn validate_planned_lints_not_active(
    root_manifest: &str,
    policy: &ClippyPolicy,
) -> Result<(), String> {
    let manifest_lints = manifest_workspace_lints(root_manifest);
    for lint in policy.lint.iter().filter(|lint| lint.status == "planned") {
        if manifest_lints.contains_key(&lint.name) {
            return Err(format!(
                "planned lint `{}` is active before MSRV {}",
                lint.name,
                lint.activate_when_msrv.as_deref().unwrap_or("<unknown>")
            ));
        }
    }
    Ok(())
}

fn validate_debt(debt: &ClippyDebt) -> Result<(), String> {
    if debt.schema != 1 {
        return Err("policy/clippy-debt.toml schema must be 1".to_owned());
    }
    let today = today();
    for entry in &debt.debt {
        require_field("debt.lint", &entry.lint)?;
        require_field("debt.path", &entry.path)?;
        require_field("debt.owner", &entry.owner)?;
        require_field("debt.reason", &entry.reason)?;
        require_field("debt.expires", &entry.expires)?;
        validate_optional_expiry("clippy debt", Some(&entry.expires), today)?;
    }
    Ok(())
}

fn report_suppression_backlog() -> Result<(), String> {
    let suppressions = collect_suppressions()?;
    if suppressions.allow_count > 0 {
        eprintln!(
            "warning: found {} legacy #[allow] suppressions; migrate them to #[expect(..., reason = \"...\")] in follow-up PRs",
            suppressions.allow_count
        );
    }
    if !suppressions.expect_without_reason.is_empty() {
        eprintln!(
            "warning: found {} #[expect] suppressions without a reason; migrate them in follow-up PRs",
            suppressions.expect_without_reason.len()
        );
    }
    Ok(())
}

struct SuppressionSummary {
    allow_count: usize,
    expect_without_reason: Vec<String>,
}

fn collect_suppressions() -> Result<SuppressionSummary, String> {
    let mut allow_count = 0usize;
    let mut expect_without_reason = Vec::new();
    for file in rust_files(Path::new("."))? {
        if file.starts_with(Path::new("target")) {
            continue;
        }
        let contents = read_to_string(&file)?;
        let lines: Vec<&str> = contents.lines().collect();
        let mut index = 0usize;
        while index < lines.len() {
            let line = lines[index];
            let trimmed = line.trim_start();
            if trimmed.starts_with("#[allow") {
                allow_count += 1;
            }
            if trimmed.starts_with("#[expect") {
                let start = index;
                let mut attribute = String::from(line);
                while !attribute.contains(']') && index + 1 < lines.len() {
                    index += 1;
                    attribute.push_str(lines[index]);
                }
                if !attribute.contains("reason") {
                    expect_without_reason.push(format!("{}:{}", file.display(), start + 1));
                }
            }
            index += 1;
        }
    }
    Ok(SuppressionSummary {
        allow_count,
        expect_without_reason,
    })
}

fn manifest_workspace_lints(manifest: &str) -> BTreeMap<String, String> {
    let mut current: Option<&str> = None;
    let mut lints = BTreeMap::new();
    for raw in manifest.lines() {
        let line = raw.trim();
        match line {
            "[workspace.lints.rust]" => {
                current = Some("rust");
                continue;
            }
            "[workspace.lints.clippy]" => {
                current = Some("clippy");
                continue;
            }
            _ if line.starts_with('[') => {
                current = None;
                continue;
            }
            _ => {}
        }
        let Some(prefix) = current else {
            continue;
        };
        if line.is_empty() || line.starts_with('#') || !line.contains('=') {
            continue;
        }
        let mut parts = line.splitn(2, '=');
        let Some(name) = parts.next() else {
            continue;
        };
        let Some(level) = parts.next() else {
            continue;
        };
        let name = name.trim();
        let key = if prefix == "clippy" {
            format!("clippy::{name}")
        } else {
            name.to_owned()
        };
        lints.insert(key, trim_quotes(level.trim()).to_owned());
    }
    lints
}

fn workspace_members(manifest: &str) -> Result<Vec<String>, String> {
    let mut in_members = false;
    let mut members = Vec::new();
    for raw in manifest.lines() {
        let line = raw.trim();
        if line == "members = [" {
            in_members = true;
            continue;
        }
        if in_members && line == "]" {
            return Ok(members);
        }
        if in_members {
            let value = line.trim_end_matches(',').trim();
            if value.starts_with('"') && value.ends_with('"') {
                members.push(trim_quotes(value).to_owned());
            }
        }
    }
    Err("could not parse [workspace].members from Cargo.toml".to_owned())
}

fn rust_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    visit_files(root, &mut files, &|path| {
        path.extension() == Some(OsStr::new("rs"))
    })?;
    Ok(files)
}

fn visit_files(
    root: &Path,
    files: &mut Vec<PathBuf>,
    include: &dyn Fn(&Path) -> bool,
) -> Result<(), String> {
    let entries =
        fs::read_dir(root).map_err(|error| format!("read {}: {error}", root.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("read {} entry: {error}", root.display()))?;
        let path = entry.path();
        let file_name = entry.file_name();
        if file_name == OsStr::new(".git") || file_name == OsStr::new("target") {
            continue;
        }
        let metadata = entry
            .metadata()
            .map_err(|error| format!("metadata {}: {error}", path.display()))?;
        if metadata.is_dir() {
            visit_files(&path, files, include)?;
        } else if metadata.is_file() && include(&path) {
            files.push(path);
        }
    }
    Ok(())
}

fn value_after_key(contents: &str, key: &str) -> Option<String> {
    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with(key) && line.contains('=') {
            let value = line.split_once('=')?.1.trim();
            return Some(trim_quotes(value).to_owned());
        }
    }
    None
}

fn require_one_location(path: Option<&str>, glob: Option<&str>) -> Result<(), String> {
    match (path, glob) {
        (Some(value), None) | (None, Some(value)) if !value.trim().is_empty() => Ok(()),
        _ => Err("allowlist entries must set exactly one non-empty path or glob".to_owned()),
    }
}

fn require_field(name: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{name} must not be empty"))
    } else {
        Ok(())
    }
}

fn validate_optional_expiry(
    kind: &str,
    expires: Option<&str>,
    today: NaiveDate,
) -> Result<(), String> {
    let Some(expires) = expires else {
        return Ok(());
    };
    let date = NaiveDate::parse_from_str(expires, "%Y-%m-%d")
        .map_err(|error| format!("{kind} expiry `{expires}` must use YYYY-MM-DD: {error}"))?;
    if date < today {
        return Err(format!("{kind} entry expired on {expires}"));
    }
    Ok(())
}

fn today() -> NaiveDate {
    Utc::now().date_naive()
}

fn trim_quotes(value: &str) -> &str {
    value.trim().trim_matches('"')
}

fn read_to_string(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("read {}: {error}", path.display()))
}

fn read_toml<T>(path: &Path) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    let contents = read_to_string(path)?;
    toml::from_str(&contents).map_err(|error| format!("parse {}: {error}", path.display()))
}

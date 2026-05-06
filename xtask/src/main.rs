use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use chrono::Utc;
use toml::Value;

const ROOT_MANIFEST: &str = "Cargo.toml";
const CLIPPY_LEDGER: &str = "policy/clippy-lints.toml";
const CLIPPY_DEBT: &str = "policy/clippy-debt.toml";
const CLIPPY_CONFIG: &str = "clippy.toml";

const REQUIRED_RUST_LINTS: &[(&str, &str)] = &[
    ("unsafe_code", "warn"),
    ("unsafe_op_in_unsafe_fn", "deny"),
    ("unused_must_use", "deny"),
    ("unexpected_cfgs", "warn"),
    ("const_item_interior_mutations", "deny"),
    ("function_casts_as_integer", "deny"),
];

const REQUIRED_CLIPPY_LINTS: &[(&str, &str)] = &[
    ("dbg_macro", "deny"),
    ("todo", "deny"),
    ("unimplemented", "deny"),
    ("panic", "deny"),
    ("unreachable", "deny"),
    ("unwrap_used", "deny"),
    ("expect_used", "deny"),
    ("get_unwrap", "deny"),
    ("unwrap_in_result", "deny"),
    ("panic_in_result_fn", "deny"),
    ("string_slice", "deny"),
    ("indexing_slicing", "deny"),
    ("out_of_bounds_indexing", "deny"),
    ("unchecked_time_subtraction", "deny"),
    ("char_indices_as_byte_indices", "deny"),
    ("sliced_string_as_bytes", "deny"),
    ("index_refutable_slice", "deny"),
    ("let_underscore_future", "deny"),
    ("let_underscore_must_use", "deny"),
    ("let_underscore_lock", "deny"),
    ("unused_result_ok", "deny"),
    ("map_err_ignore", "deny"),
    ("assertions_on_result_states", "deny"),
    ("lines_filter_map_ok", "deny"),
    ("await_holding_lock", "deny"),
    ("await_holding_refcell_ref", "deny"),
    ("await_holding_invalid_type", "deny"),
    ("future_not_send", "warn"),
    ("large_futures", "warn"),
    ("arc_with_non_send_sync", "deny"),
    ("rc_mutex", "deny"),
    ("mut_mutex_lock", "deny"),
    ("readonly_write_lock", "deny"),
    ("mem_forget", "deny"),
    ("forget_non_drop", "deny"),
    ("drop_non_drop", "deny"),
    ("undocumented_unsafe_blocks", "deny"),
    ("multiple_unsafe_ops_per_block", "deny"),
    ("repr_packed_without_abi", "deny"),
    ("float_cmp", "deny"),
    ("float_cmp_const", "deny"),
    ("float_equality_without_abs", "deny"),
    ("lossy_float_literal", "deny"),
    ("cast_sign_loss", "deny"),
    ("cast_possible_wrap", "warn"),
    ("cast_possible_truncation", "warn"),
    ("cast_precision_loss", "warn"),
    ("invalid_upcast_comparisons", "deny"),
    ("cast_abs_to_unsigned", "deny"),
    ("cast_enum_truncation", "deny"),
    ("cast_nan_to_int", "deny"),
    ("manual_midpoint", "warn"),
    ("manual_is_multiple_of", "warn"),
    ("manual_div_ceil", "warn"),
    ("arithmetic_side_effects", "warn"),
    ("suspicious_open_options", "deny"),
    ("nonsensical_open_options", "deny"),
    ("ineffective_open_options", "deny"),
    ("path_buf_push_overwrite", "deny"),
    ("join_absolute_paths", "deny"),
    ("read_line_without_trim", "warn"),
    ("exit", "deny"),
    ("iter_not_returning_iterator", "deny"),
    ("expl_impl_clone_on_copy", "deny"),
    ("infallible_try_from", "deny"),
    ("fallible_impl_from", "deny"),
    ("error_impl_error", "deny"),
    ("result_unit_err", "warn"),
    ("result_large_err", "warn"),
    ("format_in_format_args", "deny"),
    ("to_string_in_format_args", "deny"),
    ("unused_format_specs", "deny"),
    ("unnecessary_debug_formatting", "warn"),
    ("uninlined_format_args", "warn"),
    ("manual_let_else", "warn"),
    ("manual_ok_or", "warn"),
    ("manual_strip", "warn"),
    ("manual_split_once", "warn"),
    ("manual_is_variant_and", "warn"),
    ("filter_map_next", "warn"),
    ("flat_map_option", "warn"),
    ("match_result_ok", "deny"),
    ("cloned_instead_of_copied", "warn"),
    ("iter_cloned_collect", "warn"),
    ("iter_overeager_cloned", "warn"),
    ("needless_collect", "warn"),
    ("redundant_closure", "warn"),
    ("redundant_closure_for_method_calls", "warn"),
    ("missing_panics_doc", "deny"),
    ("missing_errors_doc", "warn"),
    ("allow_attributes", "deny"),
    ("allow_attributes_without_reason", "deny"),
    ("blanket_clippy_restriction_lints", "deny"),
    ("ignore_without_reason", "deny"),
    ("should_panic_without_expect", "deny"),
];

const PLANNED_LINTS: &[(&str, &str, &str)] = &[
    ("clippy::same_length_and_capacity", "deny", "1.94"),
    ("clippy::manual_ilog2", "warn", "1.94"),
    ("clippy::decimal_bitwise_operands", "warn", "1.94"),
    ("clippy::needless_type_cast", "warn", "1.94"),
    ("clippy::disallowed_fields", "deny", "1.95"),
    ("clippy::manual_checked_ops", "warn", "1.95"),
    ("clippy::manual_take", "warn", "1.95"),
    ("clippy::manual_pop_if", "warn", "1.95"),
    ("clippy::duration_suboptimal_units", "warn", "1.95"),
    ("clippy::unnecessary_trailing_comma", "warn", "1.95"),
];

const TEST_CARVEOUTS: &[&str] = &[
    "allow-unwrap-in-tests",
    "allow-expect-in-tests",
    "allow-panic-in-tests",
    "allow-indexing-slicing-in-tests",
    "allow-dbg-in-tests",
];

fn main() -> ExitCode {
    let result = run();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        print_help();
        return Ok(());
    };

    match command.as_str() {
        "check-lint-policy" => check_lint_policy(),
        "policy-report" => policy_report(),
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        other => Err(format!("unknown xtask command: {other}")),
    }
}

fn print_help() {
    println!("xtask commands:");
    println!("  check-lint-policy  validate Clippy policy ledgers and workspace inheritance");
    println!("  policy-report      print current policy inventory");
}

fn policy_report() -> Result<(), String> {
    let root = repo_root()?;
    let cargo = read_toml(&root.join(ROOT_MANIFEST))?;
    let ledger = read_toml(&root.join(CLIPPY_LEDGER))?;
    let members = workspace_members(&root, &cargo)?;
    let planned = array(&ledger, "planned")?;
    let debt = read_toml(&root.join(CLIPPY_DEBT))?;
    let debt_count = debt
        .get("debt")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);

    println!("workspace members: {}", members.len());
    println!("required rust lints: {}", REQUIRED_RUST_LINTS.len());
    println!("required clippy lints: {}", REQUIRED_CLIPPY_LINTS.len());
    println!("planned upgrade lints: {}", planned.len());
    println!("clippy debt entries: {debt_count}");
    Ok(())
}

fn check_lint_policy() -> Result<(), String> {
    let root = repo_root()?;
    let cargo = read_toml(&root.join(ROOT_MANIFEST))?;
    let ledger = read_toml(&root.join(CLIPPY_LEDGER))?;

    check_msrv(&cargo, &ledger)?;
    check_required_lints(&cargo)?;
    check_lint_inheritance(&root, &cargo)?;
    check_clippy_config(&root.join(CLIPPY_CONFIG))?;
    check_planned_lints(&cargo, &ledger)?;
    check_debt(&root.join(CLIPPY_DEBT))?;
    check_suppressions(&root)?;

    println!("lint policy OK");
    Ok(())
}

fn repo_root() -> Result<PathBuf, String> {
    env::current_dir().map_err(|err| format!("failed to read current directory: {err}"))
}

fn read_toml(path: &Path) -> Result<Value, String> {
    let content = fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    toml::from_str::<Value>(&content)
        .map_err(|err| format!("failed to parse {}: {err}", path.display()))
}

fn check_msrv(cargo: &Value, ledger: &Value) -> Result<(), String> {
    let cargo_msrv = nested_str(cargo, &["workspace", "package", "rust-version"])?;
    let ledger_msrv = required_str(ledger, "msrv")?;
    if cargo_msrv != ledger_msrv {
        return Err(format!(
            "workspace.package.rust-version ({cargo_msrv}) must match {CLIPPY_LEDGER} msrv ({ledger_msrv})"
        ));
    }

    let root_msrv = nested_str(cargo, &["package", "rust-version"])?;
    if root_msrv != ledger_msrv {
        return Err(format!(
            "root package rust-version ({root_msrv}) must match {CLIPPY_LEDGER} msrv ({ledger_msrv})"
        ));
    }

    let policy = table(ledger, "policy")?;
    require_bool(policy, "panic_free_tests", true)?;
    require_bool(policy, "allow_test_carveouts", false)?;
    require_bool(policy, "blanket_categories", false)?;
    let style = required_table_str(policy, "suppression_style")?;
    if style != "expect-with-reason" {
        return Err("policy.suppression_style must be expect-with-reason".to_string());
    }
    Ok(())
}

fn check_required_lints(cargo: &Value) -> Result<(), String> {
    let rust = nested_table(cargo, &["workspace", "lints", "rust"])?;
    for &(name, level) in REQUIRED_RUST_LINTS {
        require_lint_level(rust, name, level, "workspace.lints.rust")?;
    }

    let clippy = nested_table(cargo, &["workspace", "lints", "clippy"])?;
    for &(name, level) in REQUIRED_CLIPPY_LINTS {
        require_lint_level(clippy, name, level, "workspace.lints.clippy")?;
    }
    Ok(())
}

fn check_lint_inheritance(root: &Path, cargo: &Value) -> Result<(), String> {
    for member in workspace_members(root, cargo)? {
        let manifest = member.join("Cargo.toml");
        let parsed = read_toml(&manifest)?;
        let lints = table(&parsed, "lints")?;
        require_bool(lints, "workspace", true).map_err(|err| {
            format!(
                "{} must inherit workspace lints with [lints] workspace = true: {err}",
                manifest.display()
            )
        })?;
    }
    Ok(())
}

fn check_clippy_config(path: &Path) -> Result<(), String> {
    let content = fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    for carveout in TEST_CARVEOUTS {
        if content.contains(carveout) && content.contains("= true") {
            return Err(format!(
                "{} must not enable test carveout {carveout}",
                path.display()
            ));
        }
    }
    Ok(())
}

fn check_planned_lints(cargo: &Value, ledger: &Value) -> Result<(), String> {
    let planned = array(ledger, "planned")?;
    for &(name, level, msrv) in PLANNED_LINTS {
        let found = planned.iter().any(|entry| {
            entry.get("name").and_then(Value::as_str) == Some(name)
                && entry.get("level").and_then(Value::as_str) == Some(level)
                && entry.get("activate_when_msrv").and_then(Value::as_str) == Some(msrv)
                && entry
                    .get("reason")
                    .and_then(Value::as_str)
                    .is_some_and(has_text)
        });
        if !found {
            return Err(format!(
                "{CLIPPY_LEDGER} must track planned lint {name} at {level} for Rust {msrv}"
            ));
        }
    }

    let clippy = nested_table(cargo, &["workspace", "lints", "clippy"])?;
    for &(name, _, _) in PLANNED_LINTS {
        if let Some(short) = name.strip_prefix("clippy::") {
            if clippy.contains_key(short) {
                return Err(format!(
                    "planned lint {name} must not be active before its recorded MSRV bump"
                ));
            }
        }
    }
    Ok(())
}

fn check_debt(path: &Path) -> Result<(), String> {
    let debt = read_toml(path)?;
    let Some(entries) = debt.get("debt").and_then(Value::as_array) else {
        return Ok(());
    };

    let today = Utc::now().date_naive();
    for entry in entries {
        for field in ["lint", "path", "owner", "reason", "expires"] {
            let value = entry.get(field).and_then(Value::as_str);
            if !value.is_some_and(has_text) {
                return Err(format!(
                    "{} debt entries must include non-empty {field}",
                    path.display()
                ));
            }
        }
        let expires = required_value_str(entry, "expires")?;
        let expiry = chrono::NaiveDate::parse_from_str(expires, "%Y-%m-%d")
            .map_err(|err| format!("invalid debt expiry {expires}: {err}"))?;
        if expiry < today {
            return Err(format!("expired lint debt in {}: {expires}", path.display()));
        }
    }
    Ok(())
}

fn check_suppressions(root: &Path) -> Result<(), String> {
    for file in rust_files(root)? {
        let content = fs::read_to_string(&file)
            .map_err(|err| format!("failed to read {}: {err}", file.display()))?;
        for (line_number, line) in content.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("#[allow")
                && (trimmed.contains("clippy::all")
                    || trimmed.contains("clippy::pedantic")
                    || trimmed.contains("clippy::nursery")
                    || trimmed.contains("clippy::restriction"))
            {
                return Err(format!(
                    "{}:{} uses a blanket #[allow]; use a narrow suppression with a reason",
                    file.display(),
                    line_number.saturating_add(1)
                ));
            }
            if line.trim_start().starts_with("#[expect") && !line.contains("reason") {
                return Err(format!(
                    "{}:{} uses #[expect] without a reason",
                    file.display(),
                    line_number.saturating_add(1)
                ));
            }
        }
    }
    Ok(())
}

fn rust_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    collect_rust_files(root, &mut files)?;
    Ok(files)
}

fn collect_rust_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|err| format!("failed to read {}: {err}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|err| format!("failed to read directory entry: {err}"))?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(OsStr::to_str) else {
            continue;
        };
        if matches!(name, ".git" | "target" | ".xchecker") {
            continue;
        }
        if path.is_dir() {
            collect_rust_files(&path, files)?;
        } else if path.extension().and_then(OsStr::to_str) == Some("rs") {
            files.push(path);
        }
    }
    Ok(())
}

fn workspace_members(root: &Path, cargo: &Value) -> Result<Vec<PathBuf>, String> {
    let members = nested_array(cargo, &["workspace", "members"])?;
    let mut paths = Vec::new();
    for member in members {
        let Some(member_path) = member.as_str() else {
            return Err("workspace.members must contain only strings".to_string());
        };
        paths.push(root.join(member_path));
    }
    Ok(paths)
}

fn require_lint_level(
    table: &toml::map::Map<String, Value>,
    name: &str,
    expected: &str,
    section: &str,
) -> Result<(), String> {
    let actual = table
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing {section}.{name}"))?;
    if actual != expected {
        return Err(format!("{section}.{name} must be {expected}, found {actual}"));
    }
    Ok(())
}

fn nested_str<'a>(value: &'a Value, path: &[&str]) -> Result<&'a str, String> {
    nested_value(value, path)?
        .as_str()
        .ok_or_else(|| format!("{} must be a string", path.join(".")))
}

fn nested_array<'a>(value: &'a Value, path: &[&str]) -> Result<&'a Vec<Value>, String> {
    nested_value(value, path)?
        .as_array()
        .ok_or_else(|| format!("{} must be an array", path.join(".")))
}

fn nested_table<'a>(
    value: &'a Value,
    path: &[&str],
) -> Result<&'a toml::map::Map<String, Value>, String> {
    nested_value(value, path)?
        .as_table()
        .ok_or_else(|| format!("{} must be a table", path.join(".")))
}

fn nested_value<'a>(value: &'a Value, path: &[&str]) -> Result<&'a Value, String> {
    let mut current = value;
    for part in path {
        current = current
            .get(*part)
            .ok_or_else(|| format!("missing {}", path.join(".")))?;
    }
    Ok(current)
}

fn table<'a>(value: &'a Value, name: &str) -> Result<&'a toml::map::Map<String, Value>, String> {
    value
        .get(name)
        .and_then(Value::as_table)
        .ok_or_else(|| format!("missing table {name}"))
}

fn array<'a>(value: &'a Value, name: &str) -> Result<&'a Vec<Value>, String> {
    value
        .get(name)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("missing array {name}"))
}

fn required_str<'a>(value: &'a Value, name: &str) -> Result<&'a str, String> {
    value
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing string {name}"))
}

fn required_table_str<'a>(
    table: &'a toml::map::Map<String, Value>,
    name: &str,
) -> Result<&'a str, String> {
    table
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing string {name}"))
}

fn required_value_str<'a>(value: &'a Value, name: &str) -> Result<&'a str, String> {
    value
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing string {name}"))
}

fn require_bool(
    table: &toml::map::Map<String, Value>,
    name: &str,
    expected: bool,
) -> Result<(), String> {
    let actual = table
        .get(name)
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("missing boolean {name}"))?;
    if actual != expected {
        return Err(format!("{name} must be {expected}, found {actual}"));
    }
    Ok(())
}

fn has_text(value: &str) -> bool {
    !value.trim().is_empty()
}

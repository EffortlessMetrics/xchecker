//! Integration tests for hook execution in the orchestrator.

#[path = "test_support/mod.rs"]
mod test_support;

use std::collections::HashMap;
use std::fs;
use std::sync::{Mutex, MutexGuard, OnceLock};

use anyhow::Result;
use tempfile::TempDir;
use xchecker::exit_codes;
use xchecker::hooks::{DEFAULT_HOOK_TIMEOUT_SECS, HookConfig, HooksConfig, OnFail};
use xchecker::orchestrator::{OrchestratorConfig, OrchestratorHandle};
use xchecker::types::PhaseId;

fn hook_env_command(output_file: &str) -> String {
    if cfg!(windows) {
        format!(
            "echo %XCHECKER_SPEC_ID%:%XCHECKER_PHASE%:%XCHECKER_HOOK_TYPE% > {}",
            output_file
        )
    } else {
        format!(
            r#"printf "%s:%s:%s" "$XCHECKER_SPEC_ID" "$XCHECKER_PHASE" "$XCHECKER_HOOK_TYPE" > {}"#,
            output_file
        )
    }
}

static HOOK_ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn hook_env_guard() -> MutexGuard<'static, ()> {
    HOOK_ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
}

fn failing_command() -> String {
    if cfg!(windows) {
        "exit /b 1".to_string()
    } else {
        "exit 1".to_string()
    }
}

fn assert_hook_output(contents: &str, spec_id: &str, phase: &str, hook_type: &str) {
    let parts: Vec<&str> = contents.trim().split(':').collect();
    assert_eq!(parts.len(), 3, "Hook output format mismatch: {}", contents);
    assert_eq!(parts[0], spec_id);
    assert_eq!(parts[1], phase);
    assert_eq!(parts[2], hook_type);
}

fn unique_spec_id(test_name: &str) -> String {
    format!("hooks-integration-{}-{}", test_name, std::process::id())
}

#[tokio::test]
#[allow(clippy::await_holding_lock)] // Lock serializes tests; safe in single-threaded test context
async fn test_pre_and_post_hooks_execute() -> Result<()> {
    let _guard = hook_env_guard();
    let _home = xchecker::paths::with_isolated_home();
    let temp_dir = TempDir::new()?;
    let _cwd = test_support::CwdGuard::new(temp_dir.path())?;

    let spec_id = unique_spec_id("pre-post");

    let pre_file = "pre_hook.txt";
    let post_file = "post_hook.txt";

    let mut pre_phase = HashMap::new();
    pre_phase.insert(
        "requirements".to_string(),
        HookConfig {
            command: hook_env_command(pre_file),
            on_fail: OnFail::Warn,
            timeout: DEFAULT_HOOK_TIMEOUT_SECS,
        },
    );

    let mut post_phase = HashMap::new();
    post_phase.insert(
        "requirements".to_string(),
        HookConfig {
            command: hook_env_command(post_file),
            on_fail: OnFail::Warn,
            timeout: DEFAULT_HOOK_TIMEOUT_SECS,
        },
    );

    let hooks = HooksConfig {
        pre_phase,
        post_phase,
    };

    let config = OrchestratorConfig {
        dry_run: true,
        config: HashMap::new(),
        full_config: None,
        selectors: None,
        strict_validation: false,
        redactor: Default::default(),
        hooks: Some(hooks),
    };

    let mut handle = OrchestratorHandle::with_config_and_force(&spec_id, config, false)?;
    let result = handle.run_phase(PhaseId::Requirements).await?;

    assert!(result.success, "Phase should succeed with hooks enabled");

    let pre_contents = fs::read_to_string(temp_dir.path().join(pre_file))?;
    let post_contents = fs::read_to_string(temp_dir.path().join(post_file))?;

    assert_hook_output(&pre_contents, &spec_id, "requirements", "pre_phase");
    assert_hook_output(&post_contents, &spec_id, "requirements", "post_phase");

    Ok(())
}

#[tokio::test]
#[allow(clippy::await_holding_lock)] // Lock serializes tests; safe in single-threaded test context
async fn test_pre_phase_hook_failure_aborts_phase() -> Result<()> {
    let _guard = hook_env_guard();
    let _home = xchecker::paths::with_isolated_home();
    let temp_dir = TempDir::new()?;
    let _cwd = test_support::CwdGuard::new(temp_dir.path())?;

    let spec_id = unique_spec_id("pre-fail");

    let mut pre_phase = HashMap::new();
    pre_phase.insert(
        "requirements".to_string(),
        HookConfig {
            command: failing_command(),
            on_fail: OnFail::Fail,
            timeout: DEFAULT_HOOK_TIMEOUT_SECS,
        },
    );

    let hooks = HooksConfig {
        pre_phase,
        post_phase: HashMap::new(),
    };

    let config = OrchestratorConfig {
        dry_run: true,
        config: HashMap::new(),
        full_config: None,
        selectors: None,
        strict_validation: false,
        redactor: Default::default(),
        hooks: Some(hooks),
    };

    let mut handle = OrchestratorHandle::with_config_and_force(&spec_id, config, false)?;
    let result = handle.run_phase(PhaseId::Requirements).await?;

    assert!(!result.success, "Phase should fail when pre-hook fails");
    assert_eq!(result.exit_code, exit_codes::codes::CLAUDE_FAILURE);

    let receipt_path = result.receipt_path.expect("Receipt path should exist");
    let receipt_content = fs::read_to_string(receipt_path)?;
    let receipt_json: serde_json::Value = serde_json::from_str(&receipt_content)?;

    assert_eq!(
        receipt_json["flags"]["hook_failure"],
        serde_json::Value::String("pre_phase".to_string())
    );

    Ok(())
}

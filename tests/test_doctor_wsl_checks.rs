//! Integration tests for doctor WSL checks (FR-WSL-006)
//!
//! These tests verify that the doctor command properly checks WSL availability,
//! lists distributions, validates Claude availability, and provides actionable
//! suggestions when native Claude is missing but WSL is ready.

use xchecker::config::{CliArgs, Config};
use xchecker::doctor::{CheckStatus, DoctorCommand};

#[test]
fn test_doctor_includes_wsl_checks_on_windows() {
    let cli_args = CliArgs::default();
    let config = Config::discover(&cli_args).unwrap();
    let mut doctor = DoctorCommand::new(config);

    let output = doctor.run_with_options().unwrap();

    if cfg!(target_os = "windows") {
        // On Windows, should include WSL-related checks
        let check_names: Vec<String> = output.checks.iter().map(|c| c.name.clone()).collect();

        assert!(
            check_names.contains(&"wsl_availability".to_string()),
            "Should include wsl_availability check on Windows"
        );
        assert!(
            check_names.contains(&"wsl_default_distro".to_string()),
            "Should include wsl_default_distro check on Windows"
        );
        assert!(
            check_names.contains(&"wsl_distros".to_string()),
            "Should include wsl_distros check on Windows"
        );
    } else {
        // On non-Windows, WSL checks should still be present but marked as not applicable
        let wsl_check = output.checks.iter().find(|c| c.name == "wsl_availability");

        if let Some(check) = wsl_check {
            assert_eq!(check.status, CheckStatus::Pass);
            assert!(
                check.details.contains("not applicable") || check.details.contains("not Windows")
            );
        }
    }
}

#[test]
fn test_doctor_checks_are_sorted() {
    let cli_args = CliArgs::default();
    let config = Config::discover(&cli_args).unwrap();
    let mut doctor = DoctorCommand::new(config);

    let output = doctor.run_with_options().unwrap();

    // Verify checks are sorted by name (required for JCS canonical emission)
    let names: Vec<String> = output.checks.iter().map(|c| c.name.clone()).collect();
    let mut sorted_names = names.clone();
    sorted_names.sort();

    assert_eq!(
        names, sorted_names,
        "Doctor checks should be sorted by name for JCS canonical emission"
    );
}

#[test]
#[cfg(target_os = "windows")]
fn test_doctor_wsl_availability_check_windows() {
    let cli_args = CliArgs::default();
    let config = Config::discover(&cli_args).unwrap();
    let mut doctor = DoctorCommand::new(config);

    let output = doctor.run_with_options().unwrap();

    let wsl_check = output
        .checks
        .iter()
        .find(|c| c.name == "wsl_availability")
        .expect("Should have wsl_availability check on Windows");

    // Status should be Pass or Warn (not Fail)
    assert!(
        matches!(wsl_check.status, CheckStatus::Pass | CheckStatus::Warn),
        "WSL availability check should be Pass or Warn, got: {:?}",
        wsl_check.status
    );

    // Details should provide useful information
    assert!(
        !wsl_check.details.is_empty(),
        "WSL availability check should have details"
    );

    println!("WSL availability: {}", wsl_check.details);
}

#[test]
#[cfg(target_os = "windows")]
fn test_doctor_wsl_default_distro_check_windows() {
    let cli_args = CliArgs::default();
    let config = Config::discover(&cli_args).unwrap();
    let mut doctor = DoctorCommand::new(config);

    let output = doctor.run_with_options().unwrap();

    let distro_check = output
        .checks
        .iter()
        .find(|c| c.name == "wsl_default_distro")
        .expect("Should have wsl_default_distro check on Windows");

    // Status should be Pass or Warn (not Fail)
    assert!(
        matches!(distro_check.status, CheckStatus::Pass | CheckStatus::Warn),
        "WSL default distro check should be Pass or Warn, got: {:?}",
        distro_check.status
    );

    // Details should provide useful information
    assert!(
        !distro_check.details.is_empty(),
        "WSL default distro check should have details"
    );

    println!("WSL default distro: {}", distro_check.details);
}

#[test]
#[cfg(target_os = "windows")]
fn test_doctor_wsl_distros_list_check_windows() {
    let cli_args = CliArgs::default();
    let config = Config::discover(&cli_args).unwrap();
    let mut doctor = DoctorCommand::new(config);

    let output = doctor.run_with_options().unwrap();

    let distros_check = output
        .checks
        .iter()
        .find(|c| c.name == "wsl_distros")
        .expect("Should have wsl_distros check on Windows");

    // Status should be Pass or Warn (not Fail)
    assert!(
        matches!(distros_check.status, CheckStatus::Pass | CheckStatus::Warn),
        "WSL distros check should be Pass or Warn, got: {:?}",
        distros_check.status
    );

    // Details should provide useful information
    assert!(
        !distros_check.details.is_empty(),
        "WSL distros check should have details"
    );

    println!("WSL distros: {}", distros_check.details);

    // If WSL is available, details should list distributions
    if distros_check.status == CheckStatus::Pass {
        assert!(
            distros_check.details.contains("distribution")
                || distros_check.details.contains("distro"),
            "WSL distros check should mention distributions"
        );
    }
}

#[test]
#[cfg(target_os = "windows")]
fn test_doctor_claude_path_with_wsl_suggestion() {
    let cli_args = CliArgs::default();
    let config = Config::discover(&cli_args).unwrap();
    let mut doctor = DoctorCommand::new(config);

    let output = doctor.run_with_options().unwrap();

    let claude_check = output
        .checks
        .iter()
        .find(|c| c.name == "claude_path")
        .expect("Should have claude_path check");

    // If native Claude is not found but WSL is available with Claude,
    // the check should be Warn with actionable suggestion
    if claude_check.status == CheckStatus::Warn {
        assert!(
            claude_check.details.contains("WSL") || claude_check.details.contains("wsl"),
            "Warning should mention WSL when Claude is available there"
        );
        assert!(
            claude_check.details.contains("--runner-mode"),
            "Warning should suggest --runner-mode flag"
        );
    }

    println!(
        "Claude path check: {:?} - {}",
        claude_check.status, claude_check.details
    );
}

#[test]
#[cfg(target_os = "windows")]
fn test_doctor_wsl_checks_consistency() {
    // Verify that WSL checks are consistent with each other
    let cli_args = CliArgs::default();
    let config = Config::discover(&cli_args).unwrap();
    let mut doctor = DoctorCommand::new(config);

    let output = doctor.run_with_options().unwrap();

    let wsl_availability = output.checks.iter().find(|c| c.name == "wsl_availability");

    let wsl_distros = output.checks.iter().find(|c| c.name == "wsl_distros");

    if let (Some(availability), Some(distros)) = (wsl_availability, wsl_distros) {
        // If WSL is not available, distros check should also reflect that
        if availability.details.contains("not installed")
            || availability.details.contains("not available")
        {
            assert!(
                distros.status == CheckStatus::Warn,
                "If WSL is not available, distros check should be Warn"
            );
        }

        // If WSL is available, distros check should list distributions
        if availability.status == CheckStatus::Pass {
            // Distros check should either Pass (with distros) or Warn (no distros)
            assert!(
                matches!(distros.status, CheckStatus::Pass | CheckStatus::Warn),
                "If WSL is available, distros check should be Pass or Warn"
            );
        }
    }
}

#[test]
#[cfg(target_os = "windows")]
fn test_doctor_wsl_claude_availability_reporting() {
    // Verify that Claude availability is properly reported for each distro
    let cli_args = CliArgs::default();
    let config = Config::discover(&cli_args).unwrap();
    let mut doctor = DoctorCommand::new(config);

    let output = doctor.run_with_options().unwrap();

    let distros_check = output.checks.iter().find(|c| c.name == "wsl_distros");

    if let Some(check) = distros_check
        && check.status == CheckStatus::Pass
        && check.details.contains("distribution")
    {
        // Should indicate Claude availability with ✓ or ✗
        let has_claude_indicator = check.details.contains("✓")
            || check.details.contains("✗")
            || check.details.contains('?');
        assert!(
            has_claude_indicator,
            "Distros check should indicate Claude availability with ✓, ✗, or ?"
        );
    }
}

#[test]
#[cfg(not(target_os = "windows"))]
fn test_doctor_wsl_checks_not_applicable_on_non_windows() {
    let cli_args = CliArgs::default();
    let config = Config::discover(&cli_args).unwrap();
    let mut doctor = DoctorCommand::new(config);

    let output = doctor.run_with_options().unwrap();

    // WSL checks should exist but be marked as not applicable
    let wsl_checks: Vec<_> = output
        .checks
        .iter()
        .filter(|c| c.name.starts_with("wsl_"))
        .collect();

    for check in wsl_checks {
        assert_eq!(
            check.status,
            CheckStatus::Pass,
            "WSL check {} should be Pass on non-Windows",
            check.name
        );
        assert!(
            check.details.contains("not applicable") || check.details.contains("not Windows"),
            "WSL check {} should indicate it's not applicable on non-Windows",
            check.name
        );
    }
}

#[test]
fn test_doctor_json_output_includes_wsl_checks() {
    let cli_args = CliArgs::default();
    let config = Config::discover(&cli_args).unwrap();
    let mut doctor = DoctorCommand::new(config);

    let output = doctor.run_with_options().unwrap();

    // Serialize to JSON
    let json = serde_json::to_string(&output).unwrap();

    // Verify WSL checks are in the JSON output
    assert!(
        json.contains("wsl_availability"),
        "JSON should include wsl_availability check"
    );

    if cfg!(target_os = "windows") {
        assert!(
            json.contains("wsl_default_distro"),
            "JSON should include wsl_default_distro check on Windows"
        );
        assert!(
            json.contains("wsl_distros"),
            "JSON should include wsl_distros check on Windows"
        );
    }
}

#[test]
#[cfg(target_os = "windows")]
fn test_doctor_actionable_suggestions_when_native_missing_wsl_ready() {
    // This test verifies that when native Claude is missing but WSL is ready,
    // the doctor provides actionable suggestions

    let cli_args = CliArgs::default();
    let config = Config::discover(&cli_args).unwrap();
    let mut doctor = DoctorCommand::new(config);

    let output = doctor.run_with_options().unwrap();

    let claude_check = output.checks.iter().find(|c| c.name == "claude_path");

    let wsl_check = output.checks.iter().find(|c| c.name == "wsl_availability");

    if let (Some(claude), Some(wsl)) = (claude_check, wsl_check) {
        // If native Claude is not found (Fail or Warn) and WSL has Claude (Pass)
        if matches!(claude.status, CheckStatus::Fail | CheckStatus::Warn)
            && wsl.status == CheckStatus::Pass
            && wsl.details.contains("Claude CLI is installed")
        {
            // Claude check should provide actionable suggestion
            assert!(
                claude.details.contains("--runner-mode") || claude.details.contains("WSL"),
                "Should provide actionable suggestion about using WSL when native Claude is missing but WSL is ready. Got: {}",
                claude.details
            );
        }
    }
}

#[test]
fn test_doctor_wsl_checks_do_not_cause_failures() {
    // WSL checks should never cause the overall doctor to fail
    // They should only Pass or Warn, never Fail

    let cli_args = CliArgs::default();
    let config = Config::discover(&cli_args).unwrap();
    let mut doctor = DoctorCommand::new(config);

    let output = doctor.run_with_options().unwrap();

    let wsl_checks: Vec<_> = output
        .checks
        .iter()
        .filter(|c| c.name.starts_with("wsl_"))
        .collect();

    for check in wsl_checks {
        assert!(
            matches!(check.status, CheckStatus::Pass | CheckStatus::Warn),
            "WSL check {} should never Fail, got: {:?}",
            check.name,
            check.status
        );
    }
}

#[test]
#[cfg(target_os = "windows")]
fn test_doctor_wsl_default_distro_shows_claude_status() {
    let cli_args = CliArgs::default();
    let config = Config::discover(&cli_args).unwrap();
    let mut doctor = DoctorCommand::new(config);

    let output = doctor.run_with_options().unwrap();

    let distro_check = output
        .checks
        .iter()
        .find(|c| c.name == "wsl_default_distro");

    if let Some(check) = distro_check
        && check.status == CheckStatus::Pass
        && check.details.contains("Default WSL distro:")
    {
        // Should indicate whether Claude is available
        let has_claude_status = check.details.contains("Claude available")
            || check.details.contains("Claude not found");

        assert!(
            has_claude_status,
            "Default distro check should indicate Claude availability status. Got: {}",
            check.details
        );
    }
}

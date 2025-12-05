//! JCS emission performance benchmarks (NFR1)
//!
//! Tests that JCS canonicalization meets the ≤ 50ms target for typical payloads

use anyhow::Result;
use chrono::Utc;
use serde_json::json;
use std::collections::BTreeMap;
use std::time::Instant;
use xchecker::doctor::{CheckStatus, DoctorCheck, DoctorOutput};
use xchecker::types::{ArtifactInfo, ConfigSource, ConfigValue, StatusOutput};

/// Helper to serialize using JCS
fn to_jcs_string<T: serde::Serialize>(value: &T) -> Result<String> {
    let json_value = serde_json::to_value(value)?;
    let canonical_bytes = serde_json_canonicalizer::to_vec(&json_value)?;
    Ok(String::from_utf8(canonical_bytes)?)
}

/// Create a typical receipt-like structure for benchmarking
/// Note: We use a simplified structure since Receipt has many required fields
fn create_typical_receipt_json() -> serde_json::Value {
    json!({
        "schema_version": "1",
        "emitted_at": Utc::now().to_rfc3339(),
        "spec_id": "test-spec",
        "phase": "requirements",
        "xchecker_version": "0.1.0",
        "claude_cli_version": "0.8.5",
        "model_full_name": "haiku",
        "model_alias": null,
        "canonicalization_version": "yaml-v1,md-v1",
        "canonicalization_backend": "jcs-rfc8785",
        "flags": {
            "packet_max_bytes": "65536",
            "packet_max_lines": "1200"
        },
        "runner": "native",
        "runner_distro": null,
        "packet": {
            "files": [],
            "total_bytes": 1024,
            "total_lines": 50
        },
        "outputs": [
            {"path": "00-requirements.md", "blake3_first8": "abcd1234"},
            {"path": "00-requirements.core.yaml", "blake3_first8": "ef567890"}
        ],
        "exit_code": 0,
        "error_kind": null,
        "error_reason": null,
        "stderr_tail": null,
        "stderr_redacted": "Some stderr output",
        "warnings": ["Warning 1", "Warning 2"],
        "fallback_used": false,
        "diff_context": null
    })
}

/// Create a typical status output for benchmarking
fn create_typical_status() -> StatusOutput {
    let mut effective_config = BTreeMap::new();
    effective_config.insert(
        "packet_max_bytes".to_string(),
        ConfigValue {
            value: json!(65536),
            source: ConfigSource::Default,
        },
    );
    effective_config.insert(
        "packet_max_lines".to_string(),
        ConfigValue {
            value: json!(1200),
            source: ConfigSource::Config,
        },
    );
    effective_config.insert(
        "phase_timeout".to_string(),
        ConfigValue {
            value: json!(600),
            source: ConfigSource::Cli,
        },
    );

    let mut artifacts = Vec::new();
    for i in 0..20 {
        artifacts.push(ArtifactInfo {
            path: format!("artifact-{i:03}.md"),
            blake3_first8: format!("{:08x}", i * 12345),
        });
    }

    StatusOutput {
        schema_version: "1".to_string(),
        emitted_at: Utc::now(),
        runner: "native".to_string(),
        runner_distro: None,
        fallback_used: false,
        canonicalization_version: "yaml-v1,md-v1".to_string(),
        canonicalization_backend: "jcs-rfc8785".to_string(),
        artifacts,
        last_receipt_path: "receipts/requirements-20241124_120000.json".to_string(),
        effective_config,
        lock_drift: None,
        pending_fixups: None,
    }
}

/// Create a typical doctor output for benchmarking
fn create_typical_doctor() -> DoctorOutput {
    let checks = vec![
        DoctorCheck {
            name: "atomic_rename".to_string(),
            status: CheckStatus::Pass,
            details: "Atomic rename test passed".to_string(),
        },
        DoctorCheck {
            name: "claude_path".to_string(),
            status: CheckStatus::Pass,
            details: "Found claude at /usr/local/bin/claude".to_string(),
        },
        DoctorCheck {
            name: "claude_version".to_string(),
            status: CheckStatus::Pass,
            details: "Claude CLI version 0.8.5".to_string(),
        },
        DoctorCheck {
            name: "config_parse".to_string(),
            status: CheckStatus::Pass,
            details: "Configuration parsed successfully".to_string(),
        },
        DoctorCheck {
            name: "runner_selection".to_string(),
            status: CheckStatus::Pass,
            details: "Runner mode: native".to_string(),
        },
        DoctorCheck {
            name: "write_permissions".to_string(),
            status: CheckStatus::Pass,
            details: "Write permissions OK".to_string(),
        },
        DoctorCheck {
            name: "wsl_availability".to_string(),
            status: CheckStatus::Warn,
            details: "WSL not available (not on Windows)".to_string(),
        },
    ];

    DoctorOutput {
        schema_version: "1".to_string(),
        emitted_at: Utc::now(),
        ok: true,
        checks,
        cache_stats: None,
    }
}

#[test]
fn test_receipt_jcs_performance() -> Result<()> {
    let receipt = create_typical_receipt_json();

    // Warm-up run
    let _ = to_jcs_string(&receipt)?;

    // Measure 10 runs
    let mut timings = Vec::new();
    for _ in 0..10 {
        let start = Instant::now();
        let _ = to_jcs_string(&receipt)?;
        let elapsed = start.elapsed();
        timings.push(elapsed.as_micros() as f64 / 1000.0); // Convert to ms
    }

    // Calculate median
    timings.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = timings[timings.len() / 2];

    println!("Receipt JCS emission:");
    println!("  Median: {median:.3} ms");
    println!(
        "  Min: {:.3} ms",
        timings.iter().copied().fold(f64::INFINITY, f64::min)
    );
    println!(
        "  Max: {:.3} ms",
        timings.iter().copied().fold(f64::NEG_INFINITY, f64::max)
    );

    // NFR1 target: ≤ 50ms for JCS emission
    // Receipt is the smallest payload, should be well under target
    assert!(
        median < 50.0,
        "Receipt JCS emission took {median:.3} ms, exceeds 50ms target"
    );

    Ok(())
}

#[test]
fn test_status_jcs_performance() -> Result<()> {
    let status = create_typical_status();

    // Warm-up run
    let _ = to_jcs_string(&status)?;

    // Measure 10 runs
    let mut timings = Vec::new();
    for _ in 0..10 {
        let start = Instant::now();
        let _ = to_jcs_string(&status)?;
        let elapsed = start.elapsed();
        timings.push(elapsed.as_micros() as f64 / 1000.0); // Convert to ms
    }

    // Calculate median
    timings.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = timings[timings.len() / 2];

    println!("Status JCS emission:");
    println!("  Median: {median:.3} ms");
    println!(
        "  Min: {:.3} ms",
        timings.iter().copied().fold(f64::INFINITY, f64::min)
    );
    println!(
        "  Max: {:.3} ms",
        timings.iter().copied().fold(f64::NEG_INFINITY, f64::max)
    );

    // NFR1 target: ≤ 50ms for JCS emission
    assert!(
        median < 50.0,
        "Status JCS emission took {median:.3} ms, exceeds 50ms target"
    );

    Ok(())
}

#[test]
fn test_doctor_jcs_performance() -> Result<()> {
    let doctor = create_typical_doctor();

    // Warm-up run
    let _ = to_jcs_string(&doctor)?;

    // Measure 10 runs
    let mut timings = Vec::new();
    for _ in 0..10 {
        let start = Instant::now();
        let _ = to_jcs_string(&doctor)?;
        let elapsed = start.elapsed();
        timings.push(elapsed.as_micros() as f64 / 1000.0); // Convert to ms
    }

    // Calculate median
    timings.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = timings[timings.len() / 2];

    println!("Doctor JCS emission:");
    println!("  Median: {median:.3} ms");
    println!(
        "  Min: {:.3} ms",
        timings.iter().copied().fold(f64::INFINITY, f64::min)
    );
    println!(
        "  Max: {:.3} ms",
        timings.iter().copied().fold(f64::NEG_INFINITY, f64::max)
    );

    // NFR1 target: ≤ 50ms for JCS emission
    assert!(
        median < 50.0,
        "Doctor JCS emission took {median:.3} ms, exceeds 50ms target"
    );

    Ok(())
}

#[test]
fn test_combined_jcs_performance() -> Result<()> {
    // Test all three together to simulate a typical workflow
    let receipt = create_typical_receipt_json();
    let status = create_typical_status();
    let doctor = create_typical_doctor();

    // Warm-up run
    let _ = to_jcs_string(&receipt)?;
    let _ = to_jcs_string(&status)?;
    let _ = to_jcs_string(&doctor)?;

    // Measure 10 runs of all three
    let mut timings = Vec::new();
    for _ in 0..10 {
        let start = Instant::now();
        let _ = to_jcs_string(&receipt)?;
        let _ = to_jcs_string(&status)?;
        let _ = to_jcs_string(&doctor)?;
        let elapsed = start.elapsed();
        timings.push(elapsed.as_micros() as f64 / 1000.0); // Convert to ms
    }

    // Calculate median
    timings.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = timings[timings.len() / 2];

    println!("Combined JCS emission (receipt + status + doctor):");
    println!("  Median: {median:.3} ms");
    println!(
        "  Min: {:.3} ms",
        timings.iter().copied().fold(f64::INFINITY, f64::min)
    );
    println!(
        "  Max: {:.3} ms",
        timings.iter().copied().fold(f64::NEG_INFINITY, f64::max)
    );

    // NFR1 target: ≤ 50ms for JCS emission
    // Combined should still be under target since individual operations are fast
    assert!(
        median < 50.0,
        "Combined JCS emission took {median:.3} ms, exceeds 50ms target"
    );

    Ok(())
}

#[test]
fn test_large_status_jcs_performance() -> Result<()> {
    // Create a larger status output with 100 artifacts
    let mut effective_config = BTreeMap::new();
    for i in 0..20 {
        effective_config.insert(
            format!("config_key_{i}"),
            ConfigValue {
                value: json!(format!("value_{}", i)),
                source: ConfigSource::Default,
            },
        );
    }

    let mut artifacts = Vec::new();
    for i in 0..100 {
        artifacts.push(ArtifactInfo {
            path: format!("artifact-{i:03}.md"),
            blake3_first8: format!("{:08x}", i * 12345),
        });
    }

    let status = StatusOutput {
        schema_version: "1".to_string(),
        emitted_at: Utc::now(),
        runner: "native".to_string(),
        runner_distro: None,
        fallback_used: false,
        canonicalization_version: "yaml-v1,md-v1".to_string(),
        canonicalization_backend: "jcs-rfc8785".to_string(),
        artifacts,
        last_receipt_path: "receipts/requirements-20241124_120000.json".to_string(),
        effective_config,
        lock_drift: None,
        pending_fixups: None,
    };

    // Warm-up run
    let _ = to_jcs_string(&status)?;

    // Measure 10 runs
    let mut timings = Vec::new();
    for _ in 0..10 {
        let start = Instant::now();
        let _ = to_jcs_string(&status)?;
        let elapsed = start.elapsed();
        timings.push(elapsed.as_micros() as f64 / 1000.0); // Convert to ms
    }

    // Calculate median
    timings.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = timings[timings.len() / 2];

    println!("Large Status JCS emission (100 artifacts, 20 config keys):");
    println!("  Median: {median:.3} ms");
    println!(
        "  Min: {:.3} ms",
        timings.iter().copied().fold(f64::INFINITY, f64::min)
    );
    println!(
        "  Max: {:.3} ms",
        timings.iter().copied().fold(f64::NEG_INFINITY, f64::max)
    );

    // NFR1 target: ≤ 50ms for JCS emission
    // Even with larger payload, should be under target
    assert!(
        median < 50.0,
        "Large Status JCS emission took {median:.3} ms, exceeds 50ms target"
    );

    Ok(())
}

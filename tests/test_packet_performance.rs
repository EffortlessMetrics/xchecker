//! Performance tests for packet assembly optimization (Task 9.4)
//!
//! **WHITE-BOX TEST**: This test uses internal module APIs (`packet::{ContentSelector,
//! PacketBuilder}`) and may break with internal refactors. These tests are intentionally
//! white-box to validate internal implementation details. See FR-TEST-4 for white-box test policy.
//!
//! This test suite profiles packet assembly to identify bottlenecks and verify
//! that the ≤ 200ms target for 100 files is met (NFR1).

use anyhow::Result;
use blake3::Hasher;
use camino::Utf8PathBuf;
use std::fs;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use xchecker::packet::{ContentSelector, PacketBuilder};

/// Profile packet assembly with 100 files - detailed breakdown
#[test]
fn profile_packet_assembly_100_files() -> Result<()> {
    // Create temporary directory with 100 test files
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");
    fs::create_dir_all(&context_dir)?;

    println!("\n=== Profiling Packet Assembly (100 files) ===\n");

    // Create 100 test files with different priorities
    create_test_files(&base_path, 100)?;

    // Warm-up run
    {
        let mut builder = PacketBuilder::new()?;
        let _ = builder.build_packet(&base_path, "test", &context_dir, None)?;
    }

    // Profile individual operations
    println!("1. Detailed Operation Breakdown:");

    // File selection
    let selection_time = profile_file_selection(&base_path)?;
    println!("   File Selection: {:.2}ms", selection_time.as_millis());

    // File reading
    let reading_time = profile_file_reading(&base_path)?;
    println!("   File Reading: {:.2}ms", reading_time.as_millis());

    // BLAKE3 hashing
    let hashing_time = profile_blake3_hashing(&base_path)?;
    println!("   BLAKE3 Hashing: {:.2}ms", hashing_time.as_millis());

    // Priority sorting
    let sorting_time = profile_priority_sorting(&base_path)?;
    println!("   Priority Sorting: {:.2}ms", sorting_time.as_millis());

    println!();

    // Profile full packet assembly (5 runs)
    println!("2. Full Packet Assembly (5 runs):");
    let mut timings = Vec::new();
    for i in 1..=5 {
        let start = Instant::now();
        let mut builder = PacketBuilder::new()?;
        let _packet = builder.build_packet(&base_path, "test", &context_dir, None)?;
        let duration = start.elapsed();
        timings.push(duration);
        println!("   Run {}: {:.2}ms", i, duration.as_millis());
    }

    // Calculate statistics
    let median = calculate_median(&timings);
    let avg = timings.iter().sum::<Duration>() / timings.len() as u32;
    let min = timings.iter().min().unwrap();
    let max = timings.iter().max().unwrap();

    println!("\n3. Statistics:");
    println!("   Median: {:.2}ms", median.as_millis());
    println!("   Average: {:.2}ms", avg.as_millis());
    println!("   Min: {:.2}ms", min.as_millis());
    println!("   Max: {:.2}ms", max.as_millis());

    // Check against target (200ms for 100 files)
    let target = Duration::from_millis(200);
    println!("\n4. Target Verification:");
    println!("   Target: {}ms", target.as_millis());
    println!("   Median: {:.2}ms", median.as_millis());
    println!(
        "   Margin: {:.1}% of target",
        (median.as_millis() as f64 / target.as_millis() as f64) * 100.0
    );

    // Only enforce perf assertion if XCHECKER_ENFORCE_PERF is set
    let enforce_perf = std::env::var_os("XCHECKER_ENFORCE_PERF").is_some();

    if median <= target {
        println!("   Status: ✓ PASS (median ≤ target)");
    } else {
        println!("   Status: ✗ FAIL (median > target)");
        println!(
            "\n   Optimization needed! Current: {:.2}ms, Target: {}ms",
            median.as_millis(),
            target.as_millis()
        );
        if !enforce_perf {
            println!("   (non-fatal: set XCHECKER_ENFORCE_PERF=1 to gate on perf)");
        }
    }

    // Only fail if enforcement is enabled
    if enforce_perf {
        assert!(
            median <= target,
            "Packet assembly median {:.2}ms exceeds target {}ms",
            median.as_millis(),
            target.as_millis()
        );
    }

    Ok(())
}

/// Profile file selection operation
fn profile_file_selection(base_path: &Utf8PathBuf) -> Result<Duration> {
    let selector = ContentSelector::new()?;

    let start = Instant::now();
    let _files = selector.select_files(base_path)?;
    let duration = start.elapsed();

    Ok(duration)
}

/// Create test files for benchmarking
fn create_test_files(base_path: &Utf8PathBuf, count: usize) -> Result<()> {
    // Create different types of files to test priority selection
    let file_types = [
        ("test.core.yaml", "upstream: content\n"),
        ("SPEC-001.md", "# High priority spec\n"),
        ("ADR-001.md", "# Architecture decision\n"),
        ("README.md", "# Medium priority readme\n"),
        ("SCHEMA.yaml", "schema: definition\n"),
        ("config.toml", "# Low priority config\n"),
    ];

    let files_per_type = count / file_types.len();
    let mut file_count = 0;

    for (base_name, content) in &file_types {
        for i in 0..files_per_type {
            if file_count >= count {
                break;
            }

            let file_name = if i == 0 {
                base_name.to_string()
            } else {
                format!("{}-{}", base_name, i)
            };

            let file_path = base_path.join(&file_name);

            // Create content with realistic size (1KB per file)
            let mut file_content = content.to_string();
            while file_content.len() < 1024 {
                file_content.push_str("Additional content for realistic file size. ");
            }

            fs::write(&file_path, file_content)?;
            file_count += 1;
        }
    }

    // Fill remaining files if needed
    while file_count < count {
        let file_name = format!("test-{}.md", file_count);
        let file_path = base_path.join(&file_name);

        let mut content = format!("# Test File {}\n\n", file_count);
        while content.len() < 1024 {
            content.push_str("Test content for benchmarking. ");
        }

        fs::write(&file_path, content)?;
        file_count += 1;
    }

    Ok(())
}

/// Calculate median duration
fn calculate_median(durations: &[Duration]) -> Duration {
    let mut sorted = durations.to_vec();
    sorted.sort();

    let mid = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        (sorted[mid - 1] + sorted[mid]) / 2
    } else {
        sorted[mid]
    }
}

/// Profile file reading operation
fn profile_file_reading(base_path: &Utf8PathBuf) -> Result<Duration> {
    let selector = ContentSelector::new()?;
    let files = selector.select_files(base_path)?;

    let start = Instant::now();
    for file in &files {
        let _ = fs::read_to_string(&file.path)?;
    }
    let duration = start.elapsed();

    Ok(duration)
}

/// Profile BLAKE3 hashing operation
fn profile_blake3_hashing(base_path: &Utf8PathBuf) -> Result<Duration> {
    let selector = ContentSelector::new()?;
    let files = selector.select_files(base_path)?;

    let start = Instant::now();
    for file in &files {
        let mut hasher = Hasher::new();
        hasher.update(file.content.as_bytes());
        let _ = hasher.finalize();
    }
    let duration = start.elapsed();

    Ok(duration)
}

/// Profile priority sorting operation
fn profile_priority_sorting(base_path: &Utf8PathBuf) -> Result<Duration> {
    let selector = ContentSelector::new()?;
    let mut files = selector.select_files(base_path)?;

    let start = Instant::now();
    files.sort_by(|a, b| match a.priority.cmp(&b.priority) {
        std::cmp::Ordering::Equal => b.path.cmp(&a.path),
        other => other,
    });
    let duration = start.elapsed();

    Ok(duration)
}

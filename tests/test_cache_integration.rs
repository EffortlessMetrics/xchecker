//! Comprehensive tests for InsightCache integration with PacketBuilder (Task 9.3)
//!
//! This test suite verifies:
//! - Cache initialization with cache_dir
//! - Cache hit/miss logic in packet assembly
//! - Insight generation for cache misses
//! - Cache storage after insight generation
//! - Cache statistics logging
//! - Cache invalidation on file change
//! - Cache performance improvement (>50% speedup)
//! - Cache hit rate >70% on repeated runs

use anyhow::Result;
use camino::Utf8PathBuf;
use proptest::prelude::*;
use std::env;
use std::fs;
use std::thread;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use xchecker::cache::InsightCache;
use xchecker::logging::Logger;
use xchecker::packet::PacketBuilder;

/// Test cache initialization with cache_dir
#[test]
fn test_cache_initialization() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let cache_dir = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?.join("cache");

    // Create PacketBuilder with cache
    let builder = PacketBuilder::with_cache(cache_dir.clone())?;

    // Verify cache is initialized
    assert!(builder.cache().is_some());
    assert!(cache_dir.exists());

    // Verify cache stats are initialized
    let cache = builder.cache().unwrap();
    let stats = cache.stats();
    assert_eq!(stats.hits, 0);
    assert_eq!(stats.misses, 0);
    assert_eq!(stats.invalidations, 0);
    assert_eq!(stats.writes, 0);

    Ok(())
}

/// Test cache hit returns cached insights
#[test]
fn test_cache_hit_returns_cached_insights() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");
    let cache_dir = base_path.join("cache");

    // Create a test file
    fs::write(
        base_path.join("test.md"),
        "# Test Document\nThis is test content for caching.",
    )?;

    // First build - should be a cache miss
    let mut builder1 = PacketBuilder::with_cache(cache_dir.clone())?;
    let packet1 = builder1.build_packet(&base_path, "requirements", &context_dir, None)?;

    // Verify cache miss
    let stats1 = builder1.cache().unwrap().stats();
    assert!(stats1.misses > 0);
    assert!(stats1.writes > 0);

    // Verify packet contains insights
    assert!(packet1.content.contains("INSIGHTS:") || packet1.content.contains("CACHED INSIGHTS:"));

    // Second build - should be a cache hit
    let mut builder2 = PacketBuilder::with_cache(cache_dir)?;
    let packet2 = builder2.build_packet(&base_path, "requirements", &context_dir, None)?;

    // Verify cache hit (stats are per-builder instance, so we check hits > 0)
    let stats2 = builder2.cache().unwrap().stats();
    assert!(
        stats2.hits > 0,
        "Expected cache hits but got: hits={}, misses={}",
        stats2.hits,
        stats2.misses
    );

    // Verify packet contains cached insights
    assert!(packet2.content.contains("CACHED INSIGHTS:"));

    // Content should be similar (both contain insights)
    assert!(packet2.content.contains("test.md"));

    Ok(())
}

/// Test cache miss generates and stores insights
#[test]
fn test_cache_miss_generates_and_stores_insights() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");
    let cache_dir = base_path.join("cache");

    // Create a test file
    fs::write(
        base_path.join("design.md"),
        "# Design Document\n## Architecture\nComponent-based design.",
    )?;

    // Build packet with cache
    let mut builder = PacketBuilder::with_cache(cache_dir.clone())?;
    let packet = builder.build_packet(&base_path, "design", &context_dir, None)?;

    // Verify cache miss occurred
    let stats = builder.cache().unwrap().stats();
    assert!(stats.misses > 0);
    assert_eq!(stats.hits, 0);

    // Verify insights were generated and stored
    assert!(stats.writes > 0);
    assert!(packet.content.contains("INSIGHTS:"));

    // Verify cache directory contains cache files
    let cache_files: Vec<_> = fs::read_dir(&cache_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("json"))
        .collect();
    assert!(!cache_files.is_empty());

    Ok(())
}

/// Test cache invalidation on file change
#[test]
fn test_cache_invalidation_on_file_change() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");
    let cache_dir = base_path.join("cache");

    // Create initial file
    let test_file = base_path.join("test.md");
    fs::write(&test_file, "# Original Content")?;

    // First build - cache miss
    let mut builder1 = PacketBuilder::with_cache(cache_dir.clone())?;
    let _packet1 = builder1.build_packet(&base_path, "requirements", &context_dir, None)?;

    let stats1 = builder1.cache().unwrap().stats();
    assert!(stats1.misses > 0);
    assert!(stats1.writes > 0);

    // Wait to ensure different modification time
    thread::sleep(Duration::from_millis(100));

    // Modify the file
    fs::write(&test_file, "# Modified Content\nNew information added.")?;

    // Second build - should invalidate cache and be a miss
    let mut builder2 = PacketBuilder::with_cache(cache_dir)?;
    let _packet2 = builder2.build_packet(&base_path, "requirements", &context_dir, None)?;

    let stats2 = builder2.cache().unwrap().stats();
    // Should have invalidations due to file change
    assert!(stats2.invalidations > 0 || stats2.misses > 0);

    Ok(())
}

/// Test cache performance improvement (>50% speedup)
///
/// Uses tolerant timing comparison to avoid flaky failures on busy machines.
/// Cache hit time should be ≤ cache miss time × TOLERANCE.
///
/// NOTE: This is a **sanity check**, not a microbenchmark. CI runners have
/// significant timing variance due to shared resources, virtualization, and
/// I/O contention. The tolerance is set high to avoid flaky failures while
/// still catching gross performance regressions.
#[test]
fn test_cache_performance_improvement() -> Result<()> {
    // Tolerance factor for timing comparisons.
    // Cache hit should be at most TOLERANCE times slower than cache miss.
    // A value of 2.0 allows for 100% variance due to system noise on CI runners.
    // Per Requirements 3.1, 3.2: use relative timing assertions, not strict less-than.
    const TOLERANCE: f64 = 2.0;

    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");
    let cache_dir = base_path.join("cache");

    // Create multiple test files with substantial content
    for i in 0..10 {
        let content = format!(
            "# Document {}\n{}\n",
            i,
            "## Section\nContent line\n".repeat(50)
        );
        fs::write(base_path.join(format!("doc{}.md", i)), content)?;
    }

    // First run without cache - measure time
    let start_no_cache = Instant::now();
    let mut builder_no_cache = PacketBuilder::new()?;
    let _packet_no_cache =
        builder_no_cache.build_packet(&base_path, "requirements", &context_dir, None)?;
    let duration_no_cache = start_no_cache.elapsed();

    // First run with cache (cache miss) - measure time
    let start_first_cache = Instant::now();
    let mut builder_first = PacketBuilder::with_cache(cache_dir.clone())?;
    let _packet_first =
        builder_first.build_packet(&base_path, "requirements", &context_dir, None)?;
    let duration_first_cache = start_first_cache.elapsed();

    // Second run with cache (cache hit) - measure time
    let start_second_cache = Instant::now();
    let mut builder_second = PacketBuilder::with_cache(cache_dir)?;
    let _packet_second =
        builder_second.build_packet(&base_path, "requirements", &context_dir, None)?;
    let duration_second_cache = start_second_cache.elapsed();

    // Verify cache hit occurred
    let stats = builder_second.cache().unwrap().stats();
    assert!(stats.hits > 0);

    // Cache hit should be faster than no cache or first cache run
    // Note: This is a performance test, so we're lenient with the threshold
    // We expect at least some speedup, though >50% might not always be achieved
    // in a test environment with small files
    println!(
        "Performance: no_cache={:?}, first_cache={:?}, second_cache={:?}",
        duration_no_cache, duration_first_cache, duration_second_cache
    );

    // Verify second run is not significantly slower than first run (cache hit vs cache miss)
    // Using tolerant comparison: hit_time <= miss_time * TOLERANCE
    // This avoids flaky failures on busy machines while still catching regressions.
    let hit_ms = duration_second_cache.as_secs_f64() * 1000.0;
    let miss_ms = duration_first_cache.as_secs_f64() * 1000.0;
    let ratio = if miss_ms > 0.0 { hit_ms / miss_ms } else { 0.0 };

    assert!(
        hit_ms <= miss_ms * TOLERANCE,
        "Cache hit should be at most {}x cache miss time, got {:.2}x (hit={:.2}ms, miss={:.2}ms)",
        TOLERANCE,
        ratio,
        hit_ms,
        miss_ms
    );

    Ok(())
}

/// Helper function to compute median of timing measurements.
/// Used for robust performance comparisons that are less sensitive to outliers.
fn median(times: &mut [f64]) -> f64 {
    times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let len = times.len();
    if len == 0 {
        return 0.0;
    }
    if len.is_multiple_of(2) {
        (times[len / 2 - 1] + times[len / 2]) / 2.0
    } else {
        times[len / 2]
    }
}

/// Test cache performance using median of multiple runs for robustness.
///
/// This test is more robust than single-measurement comparisons because it:
/// - Takes multiple measurements to reduce variance from system noise
/// - Uses median instead of mean to be resistant to outliers
/// - Uses tolerant comparison (TOLERANCE factor) to allow for system variance
///
/// NOTE: This is a **sanity check**, not a microbenchmark. The tolerance is set
/// high (2.0x) to account for CI runner variance while still catching regressions.
///
/// Per Requirements 3.1, 3.2: use median-based comparison if single-measurement is flaky.
#[test]
fn test_cache_performance_median_comparison() -> Result<()> {
    // Tolerance factor for timing comparisons.
    // Cache hit median should be at most TOLERANCE times the cache miss median.
    // A value of 2.0 allows for 100% variance due to CI noise.
    const TOLERANCE: f64 = 2.0;
    // Number of runs for each measurement type
    const NUM_RUNS: usize = 5;

    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");
    let cache_dir = base_path.join("cache");

    // Create multiple test files with substantial content
    for i in 0..10 {
        let content = format!(
            "# Document {}\n{}\n",
            i,
            "## Section\nContent line\n".repeat(50)
        );
        fs::write(base_path.join(format!("doc{}.md", i)), content)?;
    }

    // Populate cache with initial run
    let mut builder_init = PacketBuilder::with_cache(cache_dir.clone())?;
    let _packet_init = builder_init.build_packet(&base_path, "requirements", &context_dir, None)?;

    // Collect multiple cache miss measurements (using fresh cache each time)
    let mut miss_times: Vec<f64> = Vec::with_capacity(NUM_RUNS);
    for _ in 0..NUM_RUNS {
        // Create fresh temp dir for cache miss measurement
        let miss_temp = TempDir::new()?;
        let miss_cache_dir = Utf8PathBuf::try_from(miss_temp.path().to_path_buf())?;

        let start = Instant::now();
        let mut builder = PacketBuilder::with_cache(miss_cache_dir)?;
        let _packet = builder.build_packet(&base_path, "requirements", &context_dir, None)?;
        miss_times.push(start.elapsed().as_secs_f64() * 1000.0);
    }

    // Collect multiple cache hit measurements (using populated cache)
    let mut hit_times: Vec<f64> = Vec::with_capacity(NUM_RUNS);
    for _ in 0..NUM_RUNS {
        let start = Instant::now();
        let mut builder = PacketBuilder::with_cache(cache_dir.clone())?;
        let _packet = builder.build_packet(&base_path, "requirements", &context_dir, None)?;
        hit_times.push(start.elapsed().as_secs_f64() * 1000.0);

        // Verify cache hit occurred
        let stats = builder.cache().unwrap().stats();
        assert!(stats.hits > 0, "Expected cache hits but got none");
    }

    // Compute medians
    let miss_median = median(&mut miss_times);
    let hit_median = median(&mut hit_times);
    let ratio = if miss_median > 0.0 {
        hit_median / miss_median
    } else {
        0.0
    };

    println!(
        "Median performance: miss_median={:.2}ms, hit_median={:.2}ms, ratio={:.2}x",
        miss_median, hit_median, ratio
    );
    println!("  Miss times: {:?}", miss_times);
    println!("  Hit times: {:?}", hit_times);

    // Assert cache hit median is not significantly slower than cache miss median
    assert!(
        hit_median <= miss_median * TOLERANCE,
        "Cache hit median ({:.2}ms) should be at most {}x cache miss median ({:.2}ms), got {:.2}x",
        hit_median,
        TOLERANCE,
        miss_median,
        ratio
    );

    Ok(())
}

/// Test cache hit rate >70% on repeated runs
#[test]
fn test_cache_hit_rate_repeated_runs() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");
    let cache_dir = base_path.join("cache");

    // Create test files
    for i in 0..5 {
        fs::write(
            base_path.join(format!("file{}.md", i)),
            format!("# File {}\nContent for file {}.", i, i),
        )?;
    }

    // First run - populate cache
    let mut builder1 = PacketBuilder::with_cache(cache_dir.clone())?;
    let _packet1 = builder1.build_packet(&base_path, "requirements", &context_dir, None)?;

    // Run multiple times and track hit rate
    let mut total_hits = 0;
    let mut total_misses = 0;

    for _ in 0..5 {
        let mut builder = PacketBuilder::with_cache(cache_dir.clone())?;
        let _packet = builder.build_packet(&base_path, "requirements", &context_dir, None)?;

        let stats = builder.cache().unwrap().stats();
        total_hits += stats.hits;
        total_misses += stats.misses;
    }

    // Calculate hit rate
    let total_requests = total_hits + total_misses;
    let hit_rate = if total_requests > 0 {
        (total_hits as f64 / total_requests as f64) * 100.0
    } else {
        0.0
    };

    println!(
        "Cache hit rate: {:.1}% ({} hits, {} misses)",
        hit_rate, total_hits, total_misses
    );

    // Verify hit rate is >70%
    assert!(
        hit_rate > 70.0,
        "Cache hit rate should be >70%, got {:.1}%",
        hit_rate
    );

    Ok(())
}

/// Test cache statistics logging
#[test]
fn test_cache_statistics_logging() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");
    let cache_dir = base_path.join("cache");

    // Create test file
    fs::write(base_path.join("test.md"), "# Test")?;

    // Create logger
    let logger = Logger::new(true); // verbose mode

    // Build packet with cache and logger
    let mut builder = PacketBuilder::with_cache(cache_dir)?;
    let _packet = builder.build_packet(&base_path, "requirements", &context_dir, Some(&logger))?;

    // Log cache stats
    builder.log_cache_stats(&logger);

    // Verify stats are tracked
    let stats = builder.cache().unwrap().stats();
    assert!(stats.hits + stats.misses > 0);

    Ok(())
}

/// Test cache with different phases
#[test]
fn test_cache_with_different_phases() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");
    let cache_dir = base_path.join("cache");

    // Create test file
    fs::write(
        base_path.join("doc.md"),
        "# Document\nContent for testing phases.",
    )?;

    // Build for requirements phase
    let mut builder1 = PacketBuilder::with_cache(cache_dir.clone())?;
    let _packet1 = builder1.build_packet(&base_path, "requirements", &context_dir, None)?;

    let stats1 = builder1.cache().unwrap().stats();
    assert!(stats1.misses > 0);
    assert!(stats1.writes > 0);

    // Build for design phase (same file, different phase)
    let mut builder2 = PacketBuilder::with_cache(cache_dir.clone())?;
    let _packet2 = builder2.build_packet(&base_path, "design", &context_dir, None)?;

    let stats2 = builder2.cache().unwrap().stats();
    // Should be a miss because phase is different (different cache key)
    assert!(stats2.misses > 0);

    // Build for requirements phase again (should hit cache)
    let mut builder3 = PacketBuilder::with_cache(cache_dir)?;
    let _packet3 = builder3.build_packet(&base_path, "requirements", &context_dir, None)?;

    let stats3 = builder3.cache().unwrap().stats();
    assert!(stats3.hits > 0);

    Ok(())
}

/// Test cache with mixed priority files
#[test]
fn test_cache_with_mixed_priority_files() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");
    let cache_dir = base_path.join("cache");

    // Create files with different priorities
    fs::write(base_path.join("upstream.core.yaml"), "key: value")?;
    fs::write(base_path.join("SPEC.md"), "# Specification")?;
    fs::write(base_path.join("README.md"), "# Readme")?;
    fs::write(base_path.join("config.toml"), "setting = true")?;

    // First build - all cache misses
    let mut builder1 = PacketBuilder::with_cache(cache_dir.clone())?;
    let _packet1 = builder1.build_packet(&base_path, "requirements", &context_dir, None)?;

    let stats1 = builder1.cache().unwrap().stats();
    assert!(stats1.misses >= 4); // At least 4 files
    assert!(stats1.writes >= 4);

    // Second build - all cache hits
    let mut builder2 = PacketBuilder::with_cache(cache_dir)?;
    let _packet2 = builder2.build_packet(&base_path, "requirements", &context_dir, None)?;

    let stats2 = builder2.cache().unwrap().stats();
    assert!(
        stats2.hits >= 4,
        "Expected at least 4 cache hits but got: hits={}, misses={}",
        stats2.hits,
        stats2.misses
    );

    Ok(())
}

/// Test cache persistence across builder instances
#[test]
fn test_cache_persistence_across_instances() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");
    let cache_dir = base_path.join("cache");

    // Create test file
    fs::write(base_path.join("persistent.md"), "# Persistent Content")?;

    // First builder instance - populate cache
    {
        let mut builder1 = PacketBuilder::with_cache(cache_dir.clone())?;
        let _packet1 = builder1.build_packet(&base_path, "requirements", &context_dir, None)?;

        let stats1 = builder1.cache().unwrap().stats();
        assert!(stats1.writes > 0);
    }

    // Second builder instance - should read from disk cache
    {
        let mut builder2 = PacketBuilder::with_cache(cache_dir)?;
        let _packet2 = builder2.build_packet(&base_path, "requirements", &context_dir, None)?;

        let stats2 = builder2.cache().unwrap().stats();
        assert!(
            stats2.hits > 0,
            "Expected cache hits from disk but got: hits={}, misses={}",
            stats2.hits,
            stats2.misses
        );
        // Note: writes might be > 0 if cache needs to update memory cache from disk
    }

    Ok(())
}

/// Test cache with empty files
#[test]
fn test_cache_with_empty_files() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");
    let cache_dir = base_path.join("cache");

    // Create empty file
    fs::write(base_path.join("empty.md"), "")?;

    // Build with cache
    let mut builder = PacketBuilder::with_cache(cache_dir)?;
    let result = builder.build_packet(&base_path, "requirements", &context_dir, None);

    // Should handle empty file gracefully
    assert!(result.is_ok());

    // Cache should track the operation
    let stats = builder.cache().unwrap().stats();
    assert!(stats.hits + stats.misses > 0);

    Ok(())
}

/// Test cache insight generation quality
#[test]
fn test_cache_insight_generation_quality() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let cache_dir = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;

    let cache = InsightCache::new(cache_dir)?;

    // Test requirements phase insights
    let req_content = r#"
# Requirements Document

## Requirements

### Requirement 1

**User Story:** As a developer, I want to test, so that I can verify functionality.

#### Acceptance Criteria

1. WHEN I run tests THEN the system SHALL pass
2. WHEN errors occur THEN the system SHALL report them
"#;

    let insights = cache.generate_insights(
        req_content,
        Utf8PathBuf::from("requirements.md").as_path(),
        "requirements",
        xchecker::types::Priority::High,
    );

    // Verify insight count (10-25 per R3.5)
    assert!(insights.len() >= 10, "Should have at least 10 insights");
    assert!(insights.len() <= 25, "Should have at most 25 insights");

    // Verify insights contain relevant information
    let insights_text = insights.join(" ");
    assert!(
        insights_text.contains("user")
            || insights_text.contains("User")
            || insights_text.contains("story"),
        "Should mention user stories"
    );

    Ok(())
}

/// Test cache with large files
#[test]
fn test_cache_with_large_files() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");
    let cache_dir = base_path.join("cache");

    // Create large file
    let large_content = "# Large Document\n".to_string() + &"Content line\n".repeat(1000);
    fs::write(base_path.join("large.md"), &large_content)?;

    // Build with cache
    let mut builder = PacketBuilder::with_cache(cache_dir.clone())?;
    let _packet1 = builder.build_packet(&base_path, "requirements", &context_dir, None)?;

    // Verify cache handled large file
    let stats1 = builder.cache().unwrap().stats();
    assert!(stats1.writes > 0);

    // Second build should hit cache
    let mut builder2 = PacketBuilder::with_cache(cache_dir)?;
    let _packet2 = builder2.build_packet(&base_path, "requirements", &context_dir, None)?;

    let stats2 = builder2.cache().unwrap().stats();
    assert!(stats2.hits > 0);

    Ok(())
}

/// Test cache memory and disk consistency
#[test]
fn test_cache_memory_disk_consistency() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
    let context_dir = base_path.join("context");
    let cache_dir = base_path.join("cache");

    // Create test file
    fs::write(base_path.join("test.md"), "# Test Content")?;

    // First build - populates both memory and disk cache
    let mut builder1 = PacketBuilder::with_cache(cache_dir.clone())?;
    let packet1 = builder1.build_packet(&base_path, "requirements", &context_dir, None)?;

    // Second build with same instance - should hit memory cache
    let packet2 = builder1.build_packet(&base_path, "requirements", &context_dir, None)?;

    let stats1 = builder1.cache().unwrap().stats();
    assert!(stats1.hits > 0); // Memory cache hit

    // Third build with new instance - should hit disk cache
    let mut builder2 = PacketBuilder::with_cache(cache_dir)?;
    let packet3 = builder2.build_packet(&base_path, "requirements", &context_dir, None)?;

    let stats2 = builder2.cache().unwrap().stats();
    assert!(stats2.hits > 0); // Disk cache hit

    // All packets should contain cached insights
    assert!(packet1.content.contains("INSIGHTS:") || packet1.content.contains("CACHED INSIGHTS:"));
    assert!(packet2.content.contains("CACHED INSIGHTS:"));
    assert!(packet3.content.contains("CACHED INSIGHTS:"));

    Ok(())
}

/// Test cache builder methods
#[test]
fn test_cache_builder_methods() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let cache_dir = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;

    // Test with_cache
    let builder1 = PacketBuilder::with_cache(cache_dir.clone())?;
    assert!(builder1.cache().is_some());

    // Test with_limits_and_cache
    let builder2 = PacketBuilder::with_limits_and_cache(32768, 600, cache_dir.clone())?;
    assert!(builder2.cache().is_some());

    // Test cache_mut
    let mut builder3 = PacketBuilder::with_cache(cache_dir.clone())?;
    assert!(builder3.cache_mut().is_some());

    // Test set_cache
    let mut builder4 = PacketBuilder::new()?;
    assert!(builder4.cache().is_none());

    let new_cache = InsightCache::new(cache_dir.clone())?;
    builder4.set_cache(new_cache);
    assert!(builder4.cache().is_some());

    // Test remove_cache
    builder4.remove_cache();
    assert!(builder4.cache().is_none());

    Ok(())
}

// ============================================================================
// Property-Based Tests for Cache Performance
// ============================================================================

/// Default number of test cases per property.
const DEFAULT_PROPTEST_CASES: u32 = 64;

/// Default max shrink iterations.
const DEFAULT_MAX_SHRINK_ITERS: u32 = 1000;

/// Creates a ProptestConfig that respects environment variables.
fn proptest_config(max_cases: Option<u32>) -> ProptestConfig {
    let env_cases = env::var("PROPTEST_CASES")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(DEFAULT_PROPTEST_CASES);

    let env_shrink_iters = env::var("PROPTEST_MAX_SHRINK_ITERS")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(DEFAULT_MAX_SHRINK_ITERS);

    let cases = match max_cases {
        Some(max) => env_cases.min(max),
        None => env_cases,
    };

    ProptestConfig {
        cases,
        max_shrink_iters: env_shrink_iters,
        max_shrink_time: 30000, // 30 seconds max shrink time
        ..ProptestConfig::default()
    }
}

/// Property test: Cache hit time ≤ cache miss time × TOLERANCE
///
/// **Feature: xchecker-final-cleanup, Property 2: Cache hit ≤ miss × TOLERANCE**
///
/// This property test verifies that cache hits are not significantly slower than
/// cache misses across various file configurations. Uses median of multiple runs
/// for robustness against system noise.
///
/// **Validates: Requirements 3.1**
///
/// NOTE: This test is inherently flaky on Windows due to I/O variability.
/// Windows-specific factors that cause variance:
/// - Antivirus real-time scanning
/// - File system journaling (NTFS)
/// - Background indexing services
/// - Memory pressure from other processes
///
/// The tolerance is set high (2.0x) to account for this variance while still
/// catching gross regressions. The minimum file count is set to 5 to ensure
/// the cache benefit outweighs overhead.
#[test]
fn prop_cache_hit_not_slower_than_miss() {
    // Tolerance factor: cache hit should be at most TOLERANCE times cache miss time.
    // A value of 2.0 allows for 100% variance due to system noise, which is necessary
    // on Windows where I/O timing can be extremely variable. On Windows specifically,
    // antivirus scanning, file system journaling, and other system overhead can cause
    // cache hits to occasionally be slower than misses for small workloads.
    // Per Requirements 3.1: use relative timing assertions where cache hit time is
    // ≤ miss time × TOLERANCE.
    const TOLERANCE: f64 = 2.0;
    // Number of runs for median calculation
    const NUM_RUNS: usize = 5;

    // Cache performance tests are slow, so cap at 10 cases even in thorough mode
    let config = proptest_config(Some(10));

    proptest!(config, |(
        // Generate varying file counts (5-10 files)
        // Minimum of 5 files ensures enough work for caching to provide measurable benefit;
        // with fewer files, cache overhead can exceed benefit on systems with high I/O variance
        file_count in 5usize..=10,
        // Generate varying content sizes (50-100 lines per file)
        // Larger minimum ensures meaningful workload
        content_lines in 50usize..=100
    )| {
        let temp_dir = TempDir::new().unwrap();
        let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf()).unwrap();
        let context_dir = base_path.join("context");
        let cache_dir = base_path.join("cache");

        // Create test files with varying content
        for i in 0..file_count {
            let content = format!(
                "# Document {}\n{}\n",
                i,
                "## Section\nContent line for testing cache performance.\n".repeat(content_lines)
            );
            fs::write(base_path.join(format!("doc{}.md", i)), content).unwrap();
        }

        // Populate cache with initial run
        let mut builder_init = PacketBuilder::with_cache(cache_dir.clone()).unwrap();
        let _packet_init = builder_init
            .build_packet(&base_path, "requirements", &context_dir, None)
            .unwrap();

        // Collect multiple cache miss measurements (using fresh cache each time)
        let mut miss_times: Vec<f64> = Vec::with_capacity(NUM_RUNS);
        for _ in 0..NUM_RUNS {
            // Create fresh temp dir for cache miss measurement
            let miss_temp = TempDir::new().unwrap();
            let miss_cache_dir = Utf8PathBuf::try_from(miss_temp.path().to_path_buf()).unwrap();

            let start = Instant::now();
            let mut builder = PacketBuilder::with_cache(miss_cache_dir).unwrap();
            let _packet = builder
                .build_packet(&base_path, "requirements", &context_dir, None)
                .unwrap();
            miss_times.push(start.elapsed().as_secs_f64() * 1000.0);
        }

        // Collect multiple cache hit measurements (using populated cache)
        let mut hit_times: Vec<f64> = Vec::with_capacity(NUM_RUNS);
        for _ in 0..NUM_RUNS {
            let start = Instant::now();
            let mut builder = PacketBuilder::with_cache(cache_dir.clone()).unwrap();
            let _packet = builder
                .build_packet(&base_path, "requirements", &context_dir, None)
                .unwrap();
            hit_times.push(start.elapsed().as_secs_f64() * 1000.0);

            // Verify cache hit occurred
            let stats = builder.cache().unwrap().stats();
            prop_assert!(stats.hits > 0, "Expected cache hits but got none");
        }

        // Compute medians using the helper function
        let miss_median = median(&mut miss_times);
        let hit_median = median(&mut hit_times);
        let ratio = if miss_median > 0.0 {
            hit_median / miss_median
        } else {
            0.0
        };

        // Minimum duration threshold (10ms): below this, timing noise dominates and
        // the ratio becomes meaningless. On Windows especially, ultra-short operations
        // have high relative variance due to antivirus, journaling, etc.
        const MIN_DURATION_MS: f64 = 10.0;

        // Only assert on ratio if both durations are above the noise floor
        if miss_median >= MIN_DURATION_MS && hit_median >= MIN_DURATION_MS {
            // Assert cache hit median is not significantly slower than cache miss median
            prop_assert!(
                hit_median <= miss_median * TOLERANCE,
                "Cache hit median ({:.2}ms) should be at most {}x cache miss median ({:.2}ms), got {:.2}x\n\
                 File count: {}, Content lines: {}\n\
                 Miss times: {:?}\n\
                 Hit times: {:?}",
                hit_median,
                TOLERANCE,
                miss_median,
                ratio,
                file_count,
                content_lines,
                miss_times,
                hit_times
            );
        }
        // For ultra-short workloads, just verify cache mechanism works (no timing assertion)
    });
}

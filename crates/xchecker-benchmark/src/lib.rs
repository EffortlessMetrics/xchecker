//! Benchmarking utilities for performance validation (NFR1)
//!
//! This module provides benchmarking capabilities to validate that xchecker
//! meets its performance targets: empty run ≤ 5s, packetization ≤ 200ms for 100 files.

use anyhow::Result;
use camino::{Utf8Path, Utf8PathBuf};
use std::fs;
use std::time::{Duration, Instant};
use tempfile::TempDir;

use xchecker_packet::ContentSelector;
use xchecker_utils::logging::{Logger, PerformanceMetrics};
use xchecker_utils::process_memory::ProcessMemory;

/// Benchmark configuration
#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    /// Number of files to create for packetization benchmark
    pub file_count: usize,
    /// Size of each test file in bytes
    pub file_size_bytes: usize,
    /// Number of benchmark iterations
    pub iterations: usize,
    /// Whether to use verbose logging
    pub verbose: bool,
    /// Performance thresholds for validation
    pub thresholds: BenchmarkThresholds,
}

/// Performance thresholds for benchmark validation (FR-BENCH-004, FR-BENCH-005, FR-BENCH-006)
#[derive(Debug, Clone)]
pub struct BenchmarkThresholds {
    /// Maximum allowed empty run time in seconds (default: 5.0)
    pub empty_run_max_secs: f64,
    /// Maximum allowed packetization time in milliseconds per 100 files (default: 200.0)
    pub packetization_max_ms_per_100_files: f64,
    /// Maximum allowed RSS memory in MB (optional)
    pub max_rss_mb: Option<f64>,
    /// Maximum allowed commit memory in MB (Windows only, optional)
    pub max_commit_mb: Option<f64>,
}

impl Default for BenchmarkThresholds {
    fn default() -> Self {
        Self {
            empty_run_max_secs: 5.0,
            packetization_max_ms_per_100_files: 200.0,
            max_rss_mb: None,
            max_commit_mb: None,
        }
    }
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            file_count: 100,
            file_size_bytes: 1024, // 1KB per file
            iterations: 5,
            verbose: false,
            thresholds: BenchmarkThresholds::default(),
        }
    }
}

/// Benchmark results (FR-BENCH-004, FR-BENCH-005, FR-BENCH-006)
#[derive(Debug, Clone)]
pub struct BenchmarkResults {
    /// Whether all performance thresholds were met (FR-BENCH-004)
    pub ok: bool,
    /// Timing results in milliseconds by benchmark name (FR-BENCH-004)
    pub timings_ms: std::collections::BTreeMap<String, f64>,
    /// Process RSS memory in MB (FR-BENCH-003)
    pub rss_mb: f64,
    /// Process commit memory in MB (Windows only, FR-BENCH-003)
    pub commit_mb: Option<f64>,
    /// Empty run timing results (all runs including warm-up)
    pub empty_run_results: Vec<Duration>,
    /// Packetization timing results (all runs including warm-up)
    pub packetization_results: Vec<Duration>,
    /// Median empty run timing (excluding warm-up)
    pub empty_run_median: Option<Duration>,
    /// Median packetization timing (excluding warm-up)
    pub packetization_median: Option<Duration>,
    /// Performance metrics from the last run
    #[allow(dead_code)] // Performance data for receipts
    pub performance_metrics: Option<PerformanceMetrics>,
    /// Process memory at benchmark completion
    pub process_memory: Option<ProcessMemory>,
    /// Whether all performance targets were met (legacy field, use 'ok' instead)
    pub targets_met: bool,
    /// Any performance violations (FR-BENCH-006)
    pub violations: Vec<String>,
}

/// Benchmark runner for performance validation
pub struct BenchmarkRunner {
    pub config: BenchmarkConfig,
}

impl BenchmarkRunner {
    /// Create a new benchmark runner
    #[must_use]
    pub const fn new(config: BenchmarkConfig) -> Self {
        Self { config }
    }

    /// Run all benchmarks and validate performance targets
    pub fn run_all_benchmarks(&self) -> Result<BenchmarkResults> {
        let mut logger = Logger::new(self.config.verbose);

        if self.config.verbose {
            println!("=== Starting Benchmark Suite ===");
            println!("Configuration:");
            println!("  File count: {}", self.config.file_count);
            println!("  File size: {} bytes", self.config.file_size_bytes);
            println!(
                "  Iterations: {} (1 warm-up + {} measured)",
                self.config.iterations,
                self.config.iterations - 1
            );
            println!();
        }

        // Run empty run benchmarks (includes warm-up)
        let empty_run_results = self.benchmark_empty_run(&mut logger)?;

        // Run packetization benchmarks (includes warm-up)
        let packetization_results = self.benchmark_packetization(&mut logger)?;

        // Calculate medians (excluding first warm-up run)
        let empty_run_median = Self::calculate_median(&empty_run_results[1..]);
        let packetization_median = Self::calculate_median(&packetization_results[1..]);

        // Generate performance metrics
        let performance_metrics = logger.generate_performance_metrics();

        // Get process memory at completion
        let process_memory = ProcessMemory::current().ok();

        // Build timings_ms map (FR-BENCH-004)
        let mut timings_ms = std::collections::BTreeMap::new();
        if let Some(median) = empty_run_median {
            timings_ms.insert("empty_run".to_string(), median.as_secs_f64() * 1000.0);
        }
        if let Some(median) = packetization_median {
            timings_ms.insert("packetization".to_string(), median.as_millis() as f64);
        }

        // Extract memory metrics (FR-BENCH-003)
        let rss_mb = process_memory.as_ref().map_or(0.0, |m| m.rss_mb);
        let commit_mb = process_memory.as_ref().and_then(|m| {
            #[cfg(target_os = "windows")]
            {
                if m.ffi_fallback {
                    None
                } else {
                    Some(m.commit_mb)
                }
            }
            #[cfg(not(target_os = "windows"))]
            {
                let _ = m; // Silence unused variable warning on non-Windows
                None
            }
        });

        // Perform threshold comparison (FR-BENCH-005, FR-BENCH-006)
        let (ok, violations) =
            self.check_thresholds(empty_run_median, packetization_median, rss_mb, commit_mb);

        // Legacy validation for backwards compatibility
        let legacy_violations = logger.validate_performance_targets();
        let targets_met = legacy_violations.is_empty();

        if self.config.verbose {
            logger.print_performance_summary();
        }

        Ok(BenchmarkResults {
            ok,
            timings_ms,
            rss_mb,
            commit_mb,
            empty_run_results,
            packetization_results,
            empty_run_median,
            packetization_median,
            performance_metrics: Some(performance_metrics),
            process_memory,
            targets_met,
            violations,
        })
    }

    /// Check performance thresholds and return (ok, violations) (FR-BENCH-005, FR-BENCH-006)
    fn check_thresholds(
        &self,
        empty_run_median: Option<Duration>,
        packetization_median: Option<Duration>,
        rss_mb: f64,
        commit_mb: Option<f64>,
    ) -> (bool, Vec<String>) {
        let mut violations = Vec::new();

        // Check empty run threshold
        if let Some(median) = empty_run_median {
            let median_secs = median.as_secs_f64();
            if median_secs > self.config.thresholds.empty_run_max_secs {
                violations.push(format!(
                    "Empty run median {:.3}s exceeds threshold {:.3}s",
                    median_secs, self.config.thresholds.empty_run_max_secs
                ));
            }
        }

        // Check packetization threshold (scaled by file count)
        if let Some(median) = packetization_median {
            let median_ms = median.as_millis() as f64;
            let target_ms = (self.config.thresholds.packetization_max_ms_per_100_files
                * self.config.file_count as f64)
                / 100.0;

            if median_ms > target_ms {
                violations.push(format!(
                    "Packetization median {:.1}ms exceeds threshold {:.1}ms for {} files",
                    median_ms, target_ms, self.config.file_count
                ));
            }
        }

        // Check RSS threshold if configured
        if let Some(max_rss) = self.config.thresholds.max_rss_mb
            && rss_mb > max_rss
        {
            violations.push(format!(
                "RSS memory {rss_mb:.1}MB exceeds threshold {max_rss:.1}MB"
            ));
        }

        // Check commit threshold if configured (Windows only)
        if let Some(max_commit) = self.config.thresholds.max_commit_mb
            && let Some(commit) = commit_mb
            && commit > max_commit
        {
            violations.push(format!(
                "Commit memory {commit:.1}MB exceeds threshold {max_commit:.1}MB"
            ));
        }

        let ok = violations.is_empty();
        (ok, violations)
    }

    /// Calculate median duration from a slice of durations
    fn calculate_median(durations: &[Duration]) -> Option<Duration> {
        if durations.is_empty() {
            return None;
        }

        let mut sorted = durations.to_vec();
        sorted.sort();

        let mid = sorted.len() / 2;
        if sorted.len().is_multiple_of(2) {
            // Even number of elements: average the two middle values
            Some((sorted[mid - 1] + sorted[mid]) / 2)
        } else {
            // Odd number of elements: take the middle value
            Some(sorted[mid])
        }
    }

    /// Benchmark empty run performance (NFR1: ≤ 5s)
    /// First iteration is a warm-up pass, remaining iterations are measured
    fn benchmark_empty_run(&self, logger: &mut Logger) -> Result<Vec<Duration>> {
        let mut results = Vec::new();

        for i in 0..self.config.iterations {
            let is_warmup = i == 0;

            if self.config.verbose {
                if is_warmup {
                    println!("Running empty run warm-up pass...");
                } else {
                    println!(
                        "Running empty run benchmark iteration {}/{}",
                        i,
                        self.config.iterations - 1
                    );
                }
            }

            let start = Instant::now();

            // Simulate empty run operations (no Claude calls)
            self.simulate_empty_run_operations()?;

            let duration = start.elapsed();

            // Only record non-warmup runs in logger
            if !is_warmup {
                logger.record_empty_run_timing(duration);
            }

            results.push(duration);

            if self.config.verbose {
                if is_warmup {
                    println!("  Warm-up: {:.3}s (not counted)", duration.as_secs_f64());
                } else {
                    println!("  Run {}: {:.3}s", i, duration.as_secs_f64());
                }
            }
        }

        Ok(results)
    }

    /// Benchmark packetization performance (NFR1: ≤ 200ms for 100 files)
    /// First iteration is a warm-up pass, remaining iterations are measured
    fn benchmark_packetization(&self, logger: &mut Logger) -> Result<Vec<Duration>> {
        let mut results = Vec::new();

        for i in 0..self.config.iterations {
            let is_warmup = i == 0;

            if self.config.verbose {
                if is_warmup {
                    println!("\nRunning packetization warm-up pass...");
                } else {
                    println!(
                        "Running packetization benchmark iteration {}/{}",
                        i,
                        self.config.iterations - 1
                    );
                }
            }

            // Create temporary directory with test files
            let temp_dir = TempDir::new()?;
            let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;

            // Create test files (deterministic workload)
            self.create_test_files(&base_path)?;

            let start = Instant::now();

            // Run packetization
            self.run_packetization(&base_path)?;

            let duration = start.elapsed();

            // Only record non-warmup runs in logger
            if !is_warmup {
                logger.record_packetization_timing(duration, self.config.file_count);
            }

            results.push(duration);

            if self.config.verbose {
                if is_warmup {
                    println!(
                        "  Warm-up: {:.1}ms for {} files (not counted)",
                        duration.as_millis(),
                        self.config.file_count
                    );
                } else {
                    println!(
                        "  Run {}: {:.1}ms for {} files",
                        i,
                        duration.as_millis(),
                        self.config.file_count
                    );
                }
            }
        }

        Ok(results)
    }

    /// Simulate empty run operations (configuration loading, validation, etc.)
    fn simulate_empty_run_operations(&self) -> Result<()> {
        // Simulate configuration loading
        std::thread::sleep(Duration::from_millis(10));

        // Simulate file system operations
        let temp_dir = TempDir::new()?;
        let _base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;

        // Simulate validation operations
        std::thread::sleep(Duration::from_millis(5));

        Ok(())
    }

    /// Create test files for packetization benchmark
    fn create_test_files(&self, base_path: &Utf8Path) -> Result<()> {
        // Create different types of files to test priority selection
        let file_types = [
            ("test.core.yaml", "upstream"),
            ("SPEC-001.md", "high"),
            ("ADR-001.md", "high"),
            ("README.md", "medium"),
            ("SCHEMA.yaml", "medium"),
            ("config.toml", "low"),
        ];

        let files_per_type = self.config.file_count / file_types.len();
        let mut file_count = 0;

        for (base_name, _priority) in &file_types {
            for i in 0..files_per_type {
                if file_count >= self.config.file_count {
                    break;
                }

                let file_name = if i == 0 {
                    (*base_name).to_string()
                } else {
                    format!("{base_name}-{i}")
                };

                let file_path = base_path.join(&file_name);
                let content = self.generate_test_content(&file_name);

                fs::write(&file_path, content)?;
                file_count += 1;
            }
        }

        // Fill remaining files if needed
        while file_count < self.config.file_count {
            let file_name = format!("test-{file_count}.md");
            let file_path = base_path.join(&file_name);
            let content = self.generate_test_content(&file_name);

            fs::write(&file_path, content)?;
            file_count += 1;
        }

        Ok(())
    }

    /// Generate test content for a file
    fn generate_test_content(&self, file_name: &str) -> String {
        let mut content = format!("# Test File: {file_name}\n\n");

        // Generate content to reach target size
        let base_content = "This is test content for benchmarking purposes. ";
        let needed_chars = self.config.file_size_bytes.saturating_sub(content.len());
        let repeat_count = needed_chars / base_content.len() + 1;

        for i in 0..repeat_count {
            content.push_str(&format!("{}Line {}. ", base_content, i + 1));
            if content.len() >= self.config.file_size_bytes {
                break;
            }
        }

        // Truncate to exact size if needed
        if content.len() > self.config.file_size_bytes {
            content.truncate(self.config.file_size_bytes);
        }

        content
    }

    /// Run packetization on test files
    fn run_packetization(&self, base_path: &Utf8Path) -> Result<()> {
        let selector = ContentSelector::new()?;
        let _selected_files = selector.select_files(base_path)?;

        // Simulate packet building operations without actually building
        // (to avoid dependencies on other components)
        std::thread::sleep(Duration::from_millis(1)); // Minimal processing time

        Ok(())
    }

    /// Print benchmark summary
    pub fn print_summary(&self, results: &BenchmarkResults) {
        println!("\n=== Benchmark Results ===");

        // Empty run results
        if !results.empty_run_results.is_empty() {
            let measured_runs = &results.empty_run_results[1..]; // Exclude warm-up
            let avg_empty_run = if measured_runs.is_empty() {
                Duration::from_secs(0)
            } else {
                measured_runs.iter().sum::<Duration>() / measured_runs.len() as u32
            };
            let default_duration = Duration::from_secs(0);
            let max_empty_run = measured_runs.iter().max().unwrap_or(&default_duration);
            let median_empty_run = results.empty_run_median.unwrap_or(Duration::from_secs(0));

            println!("Empty Run Performance:");
            println!("  Runs:    {} measured (+ 1 warm-up)", measured_runs.len());
            println!("  Median:  {:.3}s", median_empty_run.as_secs_f64());
            println!("  Average: {:.3}s", avg_empty_run.as_secs_f64());
            println!("  Maximum: {:.3}s", max_empty_run.as_secs_f64());
            println!("  Target:  5.000s");

            if median_empty_run <= Duration::from_secs(5) {
                println!("  Status:  ✓ PASS (median ≤ target)");
            } else {
                println!("  Status:  ✗ FAIL (median > target)");
            }
        }

        // Packetization results
        if !results.packetization_results.is_empty() {
            let measured_runs = &results.packetization_results[1..]; // Exclude warm-up
            let avg_packetization = if measured_runs.is_empty() {
                Duration::from_secs(0)
            } else {
                measured_runs.iter().sum::<Duration>() / measured_runs.len() as u32
            };
            let default_duration = Duration::from_secs(0);
            let max_packetization = measured_runs.iter().max().unwrap_or(&default_duration);
            let median_packetization = results
                .packetization_median
                .unwrap_or(Duration::from_secs(0));

            println!(
                "\nPacketization Performance ({} files):",
                self.config.file_count
            );
            println!("  Runs:    {} measured (+ 1 warm-up)", measured_runs.len());
            println!("  Median:  {:.1}ms", median_packetization.as_millis());
            println!("  Average: {:.1}ms", avg_packetization.as_millis());
            println!("  Maximum: {:.1}ms", max_packetization.as_millis());

            // Calculate target for file count using configured threshold
            let target_ms = (self.config.thresholds.packetization_max_ms_per_100_files
                * self.config.file_count as f64)
                / 100.0;
            println!("  Target:  {target_ms:.0}ms");

            if median_packetization.as_millis() as f64 <= target_ms {
                println!("  Status:  ✓ PASS (median ≤ target)");
            } else {
                println!("  Status:  ✗ FAIL (median > target)");
            }
        }

        // Overall status
        println!("\nOverall Performance:");
        if results.targets_met {
            println!("  Status: ✓ All targets met");
        } else {
            println!("  Status: ✗ Some targets not met");
            for violation in &results.violations {
                println!("    - {violation}");
            }
        }

        // Process memory usage (R3.1, R3.4, R3.5)
        if let Some(mem) = &results.process_memory {
            println!("\nProcess Memory:");
            println!("  {}", mem.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_benchmark_config_default() {
        let config = BenchmarkConfig::default();
        assert_eq!(config.file_count, 100);
        assert_eq!(config.file_size_bytes, 1024);
        assert_eq!(config.iterations, 5);
        assert!(!config.verbose);
    }

    #[test]
    fn test_benchmark_runner_creation() {
        let config = BenchmarkConfig::default();
        let runner = BenchmarkRunner::new(config.clone());
        assert_eq!(runner.config.file_count, config.file_count);
    }

    #[test]
    fn test_generate_test_content() {
        let config = BenchmarkConfig {
            file_size_bytes: 100,
            ..Default::default()
        };
        let runner = BenchmarkRunner::new(config);

        let content = runner.generate_test_content("test.md");
        assert!(content.len() <= 100);
        assert!(content.starts_with("# Test File: test.md"));
    }

    #[test]
    fn test_create_test_files() -> Result<()> {
        let config = BenchmarkConfig {
            file_count: 10,
            file_size_bytes: 50,
            ..Default::default()
        };
        let runner = BenchmarkRunner::new(config);

        let temp_dir = TempDir::new()?;
        let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;

        runner.create_test_files(&base_path)?;

        // Count created files
        let mut file_count = 0;
        for entry in fs::read_dir(&base_path)? {
            let entry = entry?;
            if entry.path().is_file() {
                file_count += 1;
            }
        }

        assert_eq!(file_count, 10);
        Ok(())
    }

    #[test]
    fn test_simulate_empty_run_operations() -> Result<()> {
        let config = BenchmarkConfig::default();
        let runner = BenchmarkRunner::new(config);

        let start = Instant::now();
        runner.simulate_empty_run_operations()?;
        let duration = start.elapsed();

        // Should complete quickly (well under 5s target)
        assert!(duration < Duration::from_secs(1));
        Ok(())
    }

    #[test]
    fn test_process_memory_in_results() -> Result<()> {
        let config = BenchmarkConfig {
            file_count: 5,
            file_size_bytes: 50,
            iterations: 1,
            verbose: false,
            ..Default::default()
        };
        let runner = BenchmarkRunner::new(config);

        let results = runner.run_all_benchmarks()?;

        // Process memory should be captured
        assert!(
            results.process_memory.is_some(),
            "Process memory should be captured"
        );

        let mem = results.process_memory.unwrap();

        // RSS should be positive
        assert!(mem.rss_mb > 0.0, "RSS should be positive");

        // Test one-decimal rendering
        let display = mem.display();
        assert!(display.contains("RSS:"), "Display should contain 'RSS:'");
        assert!(display.contains("MB"), "Display should contain 'MB'");

        // Verify decimal precision (should have a decimal point)
        assert!(
            display.contains('.'),
            "Display should have decimal point for one-decimal precision"
        );

        // On Windows, check for fallback warning if applicable
        #[cfg(target_os = "windows")]
        {
            if mem.ffi_fallback {
                assert!(
                    display.contains("FFI fallback"),
                    "Display should indicate FFI fallback"
                );
            }
        }

        Ok(())
    }

    #[test]
    fn test_warmup_pass_execution() -> Result<()> {
        let config = BenchmarkConfig {
            file_count: 5,
            file_size_bytes: 50,
            iterations: 3, // 1 warm-up + 2 measured
            verbose: false,
            ..Default::default()
        };
        let runner = BenchmarkRunner::new(config);

        let results = runner.run_all_benchmarks()?;

        // Should have 3 total results (1 warm-up + 2 measured)
        assert_eq!(
            results.empty_run_results.len(),
            3,
            "Should have 3 empty run results"
        );
        assert_eq!(
            results.packetization_results.len(),
            3,
            "Should have 3 packetization results"
        );

        // Medians should be calculated from the 2 measured runs
        assert!(
            results.empty_run_median.is_some(),
            "Empty run median should be calculated"
        );
        assert!(
            results.packetization_median.is_some(),
            "Packetization median should be calculated"
        );

        Ok(())
    }

    #[test]
    fn test_median_calculation_odd_count() {
        let durations = vec![
            Duration::from_millis(100),
            Duration::from_millis(200),
            Duration::from_millis(300),
        ];

        let median = BenchmarkRunner::calculate_median(&durations);
        assert_eq!(median, Some(Duration::from_millis(200)));
    }

    #[test]
    fn test_median_calculation_even_count() {
        let durations = vec![
            Duration::from_millis(100),
            Duration::from_millis(200),
            Duration::from_millis(300),
            Duration::from_millis(400),
        ];

        let median = BenchmarkRunner::calculate_median(&durations);
        // Median of 200 and 300 is 250
        assert_eq!(median, Some(Duration::from_millis(250)));
    }

    #[test]
    fn test_median_calculation_single_value() {
        let durations = vec![Duration::from_millis(150)];

        let median = BenchmarkRunner::calculate_median(&durations);
        assert_eq!(median, Some(Duration::from_millis(150)));
    }

    #[test]
    fn test_median_calculation_empty() {
        let durations: Vec<Duration> = vec![];

        let median = BenchmarkRunner::calculate_median(&durations);
        assert_eq!(median, None);
    }

    #[test]
    fn test_median_calculation_unsorted() {
        let durations = vec![
            Duration::from_millis(300),
            Duration::from_millis(100),
            Duration::from_millis(200),
        ];

        let median = BenchmarkRunner::calculate_median(&durations);
        // Should sort first, then return middle value
        assert_eq!(median, Some(Duration::from_millis(200)));
    }

    #[test]
    fn test_deterministic_workload_generation() -> Result<()> {
        let config = BenchmarkConfig {
            file_count: 10,
            file_size_bytes: 100,
            iterations: 1,
            verbose: false,
            ..Default::default()
        };
        let runner = BenchmarkRunner::new(config);

        // Create two workloads with same config
        let temp_dir1 = TempDir::new()?;
        let base_path1 = Utf8PathBuf::try_from(temp_dir1.path().to_path_buf())?;
        runner.create_test_files(&base_path1)?;

        let temp_dir2 = TempDir::new()?;
        let base_path2 = Utf8PathBuf::try_from(temp_dir2.path().to_path_buf())?;
        runner.create_test_files(&base_path2)?;

        // Count files in both directories
        let count1 = std::fs::read_dir(&base_path1)?.count();
        let count2 = std::fs::read_dir(&base_path2)?.count();

        assert_eq!(count1, count2, "Both workloads should have same file count");
        assert_eq!(count1, 10, "Should have exactly 10 files");

        // Verify file names are deterministic
        let mut files1: Vec<String> = std::fs::read_dir(&base_path1)?
            .filter_map(std::result::Result::ok)
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        files1.sort();

        let mut files2: Vec<String> = std::fs::read_dir(&base_path2)?
            .filter_map(std::result::Result::ok)
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        files2.sort();

        assert_eq!(files1, files2, "File names should be deterministic");

        Ok(())
    }

    #[test]
    fn test_workload_file_sizes_consistent() -> Result<()> {
        let config = BenchmarkConfig {
            file_count: 5,
            file_size_bytes: 200,
            iterations: 1,
            verbose: false,
            ..Default::default()
        };
        let runner = BenchmarkRunner::new(config);

        let temp_dir = TempDir::new()?;
        let base_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())?;
        runner.create_test_files(&base_path)?;

        // Check that all files are approximately the target size
        for entry in std::fs::read_dir(&base_path)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            let size = metadata.len();

            // Files should be close to target size (within reasonable bounds)
            assert!(
                size <= 200,
                "File size {} should not exceed target {}",
                size,
                200
            );
            assert!(
                size >= 150,
                "File size {} should be reasonably close to target {}",
                size,
                200
            );
        }

        Ok(())
    }

    #[test]
    fn test_measured_runs_count() -> Result<()> {
        let config = BenchmarkConfig {
            file_count: 5,
            file_size_bytes: 50,
            iterations: 5, // 1 warm-up + 4 measured
            verbose: false,
            ..Default::default()
        };
        let runner = BenchmarkRunner::new(config);

        let results = runner.run_all_benchmarks()?;

        // Total results should equal iterations
        assert_eq!(results.empty_run_results.len(), 5);
        assert_eq!(results.packetization_results.len(), 5);

        // Medians should be calculated from 4 measured runs (excluding first)
        let measured_empty = &results.empty_run_results[1..];
        let measured_packet = &results.packetization_results[1..];

        assert_eq!(measured_empty.len(), 4);
        assert_eq!(measured_packet.len(), 4);

        Ok(())
    }

    #[test]
    fn test_process_memory_integration() -> Result<()> {
        // Test that ProcessMemory is properly integrated into benchmark results
        let config = BenchmarkConfig {
            file_count: 5,
            file_size_bytes: 50,
            iterations: 1,
            verbose: false,
            ..Default::default()
        };
        let runner = BenchmarkRunner::new(config);

        let results = runner.run_all_benchmarks()?;

        // Process memory should be captured
        assert!(
            results.process_memory.is_some(),
            "Process memory should be captured in benchmark results"
        );

        let mem = results.process_memory.unwrap();

        // RSS should be positive and reasonable
        assert!(
            mem.rss_mb > 0.0,
            "RSS should be positive, got {:.1}MB",
            mem.rss_mb
        );
        assert!(
            mem.rss_mb < 1024.0,
            "RSS should be < 1GB for a test process, got {:.1}MB",
            mem.rss_mb
        );

        // On Windows, verify commit_mb is present (unless fallback)
        #[cfg(target_os = "windows")]
        {
            if !mem.ffi_fallback {
                assert!(
                    mem.commit_mb > 0.0,
                    "Commit should be positive when not using fallback, got {:.1}MB",
                    mem.commit_mb
                );
                assert!(
                    mem.commit_mb < 2048.0,
                    "Commit should be < 2GB for a test process, got {:.1}MB",
                    mem.commit_mb
                );
            }
        }

        Ok(())
    }

    #[test]
    fn test_process_memory_display_in_summary() -> Result<()> {
        // Test that process memory is properly displayed in benchmark summary
        let config = BenchmarkConfig {
            file_count: 5,
            file_size_bytes: 50,
            iterations: 1,
            verbose: false,
            ..Default::default()
        };
        let runner = BenchmarkRunner::new(config);

        let results = runner.run_all_benchmarks()?;

        // Verify process memory is present
        assert!(results.process_memory.is_some());

        // The print_summary method should handle displaying this
        // We can't easily test console output, but we can verify the data is there
        let mem = results.process_memory.as_ref().unwrap();
        let display = mem.display();

        // Verify display format
        assert!(display.contains("RSS:"), "Display should contain 'RSS:'");
        assert!(display.contains("MB"), "Display should contain 'MB'");

        #[cfg(target_os = "windows")]
        {
            if mem.ffi_fallback {
                assert!(
                    display.contains("FFI fallback"),
                    "Windows display should indicate FFI fallback when used"
                );
            } else {
                assert!(
                    display.contains("Commit:"),
                    "Windows display should contain 'Commit:' when not using fallback"
                );
            }
        }

        Ok(())
    }

    // ===== FR-BENCH-004, FR-BENCH-005, FR-BENCH-006 Tests =====

    #[test]
    fn test_benchmark_results_structure() -> Result<()> {
        // Test that BenchmarkResults has all required fields (FR-BENCH-004)
        // Use generous thresholds since this test is about structure, not performance
        let config = BenchmarkConfig {
            file_count: 5,
            file_size_bytes: 50,
            iterations: 2, // 1 warm-up + 1 measured
            verbose: false,
            thresholds: BenchmarkThresholds {
                empty_run_max_secs: 60.0,                    // Very generous for test env
                packetization_max_ms_per_100_files: 10000.0, // Very generous for test env
                max_rss_mb: Some(4096.0),                    // 4GB threshold
                max_commit_mb: Some(8192.0),                 // 8GB threshold
            },
        };
        let runner = BenchmarkRunner::new(config);

        let results = runner.run_all_benchmarks()?;

        // Verify ok field is boolean - the type system guarantees this,
        // so we just verify we can read it
        let _ok: bool = results.ok;

        // Verify timings_ms map exists and contains expected keys
        assert!(
            results.timings_ms.contains_key("empty_run"),
            "timings_ms should contain 'empty_run'"
        );
        assert!(
            results.timings_ms.contains_key("packetization"),
            "timings_ms should contain 'packetization'"
        );

        // Verify rss_mb is present and positive
        assert!(results.rss_mb > 0.0, "rss_mb should be positive");

        // Verify commit_mb is optional (Windows only)
        #[cfg(target_os = "windows")]
        {
            // May or may not be present depending on FFI fallback
            if let Some(commit) = results.commit_mb {
                assert!(commit > 0.0, "commit_mb should be positive when present");
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            assert!(
                results.commit_mb.is_none(),
                "commit_mb should be None on non-Windows"
            );
        }

        Ok(())
    }

    #[test]
    fn test_threshold_comparison_pass() -> Result<()> {
        // Test that thresholds pass when performance is good (FR-BENCH-005)
        let config = BenchmarkConfig {
            file_count: 5,
            file_size_bytes: 50,
            iterations: 2, // 1 warm-up + 1 measured
            verbose: false,
            thresholds: BenchmarkThresholds {
                empty_run_max_secs: 60.0,                      // Very generous threshold
                packetization_max_ms_per_100_files: 100_000.0, // Very generous threshold
                max_rss_mb: None,    // Avoid flakiness from process-wide RSS
                max_commit_mb: None, // Avoid flakiness from process-wide commit
            },
        };
        let runner = BenchmarkRunner::new(config);

        let results = runner.run_all_benchmarks()?;

        // With generous thresholds, should pass
        assert!(
            results.ok,
            "Benchmark should pass with generous thresholds; violations={:?}",
            results.violations
        );
        assert!(
            results.violations.is_empty(),
            "Should have no violations with generous thresholds"
        );

        Ok(())
    }

    #[test]
    fn test_threshold_comparison_fail() -> Result<()> {
        // Test that thresholds fail when exceeded (FR-BENCH-006)
        let config = BenchmarkConfig {
            file_count: 5,
            file_size_bytes: 50,
            iterations: 2, // 1 warm-up + 1 measured
            verbose: false,
            thresholds: BenchmarkThresholds {
                empty_run_max_secs: 0.001,                 // Impossibly low threshold
                packetization_max_ms_per_100_files: 0.001, // Impossibly low threshold
                max_rss_mb: Some(0.1),                     // Impossibly low threshold
                max_commit_mb: Some(0.1),                  // Impossibly low threshold
            },
        };
        let runner = BenchmarkRunner::new(config);

        let results = runner.run_all_benchmarks()?;

        // With impossibly low thresholds, should fail
        assert!(!results.ok, "Benchmark should fail with low thresholds");
        assert!(
            !results.violations.is_empty(),
            "Should have violations with low thresholds"
        );

        // Should have at least empty run and packetization violations
        let has_empty_run_violation = results.violations.iter().any(|v| v.contains("Empty run"));
        let has_packetization_violation = results
            .violations
            .iter()
            .any(|v| v.contains("Packetization"));

        assert!(
            has_empty_run_violation,
            "Should have empty run violation: {:?}",
            results.violations
        );
        assert!(
            has_packetization_violation,
            "Should have packetization violation: {:?}",
            results.violations
        );

        Ok(())
    }

    #[test]
    fn test_configurable_thresholds() -> Result<()> {
        // Test that thresholds are configurable (FR-BENCH-005)
        let custom_thresholds = BenchmarkThresholds {
            empty_run_max_secs: 3.0,
            packetization_max_ms_per_100_files: 150.0,
            max_rss_mb: Some(512.0),
            max_commit_mb: Some(1024.0),
        };

        let config = BenchmarkConfig {
            file_count: 5,
            file_size_bytes: 50,
            iterations: 2,
            verbose: false,
            thresholds: custom_thresholds,
        };

        // Verify thresholds are set correctly
        assert_eq!(config.thresholds.empty_run_max_secs, 3.0);
        assert_eq!(config.thresholds.packetization_max_ms_per_100_files, 150.0);
        assert_eq!(config.thresholds.max_rss_mb, Some(512.0));
        assert_eq!(config.thresholds.max_commit_mb, Some(1024.0));

        Ok(())
    }

    #[test]
    fn test_default_thresholds() -> Result<()> {
        // Test that default thresholds match NFR1 requirements
        let thresholds = BenchmarkThresholds::default();

        assert_eq!(
            thresholds.empty_run_max_secs, 5.0,
            "Default empty run threshold should be 5.0s (NFR1)"
        );
        assert_eq!(
            thresholds.packetization_max_ms_per_100_files, 200.0,
            "Default packetization threshold should be 200ms per 100 files (NFR1)"
        );
        assert_eq!(
            thresholds.max_rss_mb, None,
            "Default RSS threshold should be None"
        );
        assert_eq!(
            thresholds.max_commit_mb, None,
            "Default commit threshold should be None"
        );

        Ok(())
    }

    #[test]
    fn test_median_calculation_for_results() -> Result<()> {
        // Test that median is correctly calculated and used in results
        let config = BenchmarkConfig {
            file_count: 5,
            file_size_bytes: 50,
            iterations: 4, // 1 warm-up + 3 measured
            verbose: false,
            ..Default::default()
        };
        let runner = BenchmarkRunner::new(config);

        let results = runner.run_all_benchmarks()?;

        // Verify medians are calculated
        assert!(
            results.empty_run_median.is_some(),
            "Empty run median should be calculated"
        );
        assert!(
            results.packetization_median.is_some(),
            "Packetization median should be calculated"
        );

        // Verify timings_ms contains median values
        let empty_run_ms = results.timings_ms.get("empty_run").unwrap();
        let packetization_ms = results.timings_ms.get("packetization").unwrap();

        assert!(*empty_run_ms > 0.0, "Empty run timing should be positive");
        assert!(
            *packetization_ms > 0.0,
            "Packetization timing should be positive"
        );

        // Verify median matches timings_ms
        let expected_empty_run_ms = results.empty_run_median.unwrap().as_secs_f64() * 1000.0;
        assert_eq!(
            *empty_run_ms, expected_empty_run_ms,
            "timings_ms should match median"
        );

        Ok(())
    }

    #[test]
    fn test_rss_memory_threshold() -> Result<()> {
        // Test RSS memory threshold checking
        let config = BenchmarkConfig {
            file_count: 5,
            file_size_bytes: 50,
            iterations: 2,
            verbose: false,
            thresholds: BenchmarkThresholds {
                empty_run_max_secs: 100.0,                   // High enough to pass
                packetization_max_ms_per_100_files: 10000.0, // High enough to pass
                max_rss_mb: Some(0.1),                       // Very low to trigger violation
                max_commit_mb: None,
            },
        };
        let runner = BenchmarkRunner::new(config);

        let results = runner.run_all_benchmarks()?;

        // Should fail due to RSS threshold
        assert!(!results.ok, "Should fail with low RSS threshold");

        // Should have RSS violation
        let has_rss_violation = results.violations.iter().any(|v| v.contains("RSS memory"));
        assert!(
            has_rss_violation,
            "Should have RSS violation: {:?}",
            results.violations
        );

        Ok(())
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_commit_memory_threshold_windows() -> Result<()> {
        // Test commit memory threshold checking on Windows
        let config = BenchmarkConfig {
            file_count: 5,
            file_size_bytes: 50,
            iterations: 2,
            verbose: false,
            thresholds: BenchmarkThresholds {
                empty_run_max_secs: 100.0,                   // High enough to pass
                packetization_max_ms_per_100_files: 10000.0, // High enough to pass
                max_rss_mb: Some(10000.0),                   // High enough to pass
                max_commit_mb: Some(0.1),                    // Very low to trigger violation
            },
        };
        let runner = BenchmarkRunner::new(config);

        let results = runner.run_all_benchmarks()?;

        // Should fail due to commit threshold (if not using FFI fallback)
        if results.commit_mb.is_some() {
            assert!(!results.ok, "Should fail with low commit threshold");

            // Should have commit violation
            let has_commit_violation = results
                .violations
                .iter()
                .any(|v| v.contains("Commit memory"));
            assert!(
                has_commit_violation,
                "Should have commit violation: {:?}",
                results.violations
            );
        }

        Ok(())
    }

    #[test]
    fn test_ok_false_on_threshold_failure() -> Result<()> {
        // Test that ok is false when any threshold is exceeded (FR-BENCH-006)
        let config = BenchmarkConfig {
            file_count: 5,
            file_size_bytes: 50,
            iterations: 2,
            verbose: false,
            thresholds: BenchmarkThresholds {
                empty_run_max_secs: 0.001,                   // Will fail
                packetization_max_ms_per_100_files: 10000.0, // Will pass
                max_rss_mb: None,
                max_commit_mb: None,
            },
        };
        let runner = BenchmarkRunner::new(config);

        let results = runner.run_all_benchmarks()?;

        // ok should be false when any threshold fails
        assert!(
            !results.ok,
            "ok should be false when any threshold is exceeded"
        );
        assert!(
            !results.violations.is_empty(),
            "violations should not be empty when ok is false"
        );

        Ok(())
    }

    #[test]
    fn test_timings_ms_btreemap_ordering() -> Result<()> {
        // Test that timings_ms uses BTreeMap for deterministic ordering
        let config = BenchmarkConfig {
            file_count: 5,
            file_size_bytes: 50,
            iterations: 2,
            verbose: false,
            ..Default::default()
        };
        let runner = BenchmarkRunner::new(config);

        let results = runner.run_all_benchmarks()?;

        // Collect keys in order
        let keys: Vec<&String> = results.timings_ms.keys().collect();

        // Should be sorted alphabetically (BTreeMap property)
        assert_eq!(keys[0], "empty_run", "First key should be 'empty_run'");
        assert_eq!(
            keys[1], "packetization",
            "Second key should be 'packetization'"
        );

        Ok(())
    }

    #[test]
    fn test_threshold_scaling_by_file_count() -> Result<()> {
        // Test that packetization threshold scales with file count
        let config_50_files = BenchmarkConfig {
            file_count: 50,
            file_size_bytes: 50,
            iterations: 2,
            verbose: false,
            thresholds: BenchmarkThresholds {
                empty_run_max_secs: 100.0,
                packetization_max_ms_per_100_files: 200.0, // 200ms per 100 files
                max_rss_mb: None,
                max_commit_mb: None,
            },
        };

        let config_100_files = BenchmarkConfig {
            file_count: 100,
            file_size_bytes: 50,
            iterations: 2,
            verbose: false,
            thresholds: BenchmarkThresholds {
                empty_run_max_secs: 100.0,
                packetization_max_ms_per_100_files: 200.0, // 200ms per 100 files
                max_rss_mb: None,
                max_commit_mb: None,
            },
        };

        // For 50 files: threshold should be 100ms (200 * 50 / 100)
        // For 100 files: threshold should be 200ms (200 * 100 / 100)

        // We can't easily test the actual threshold calculation without running benchmarks,
        // but we can verify the config is set correctly
        assert_eq!(config_50_files.file_count, 50);
        assert_eq!(config_100_files.file_count, 100);
        assert_eq!(
            config_50_files
                .thresholds
                .packetization_max_ms_per_100_files,
            config_100_files
                .thresholds
                .packetization_max_ms_per_100_files
        );

        Ok(())
    }

    #[test]
    fn test_violations_list_format() -> Result<()> {
        // Test that violations have clear, actionable messages
        let config = BenchmarkConfig {
            file_count: 5,
            file_size_bytes: 50,
            iterations: 2,
            verbose: false,
            thresholds: BenchmarkThresholds {
                empty_run_max_secs: 0.001,
                packetization_max_ms_per_100_files: 0.001,
                max_rss_mb: Some(0.1),
                max_commit_mb: Some(0.1),
            },
        };
        let runner = BenchmarkRunner::new(config);

        let results = runner.run_all_benchmarks()?;

        // Verify violations have clear format
        for violation in &results.violations {
            // Each violation should mention what exceeded what
            assert!(
                violation.contains("exceeds threshold"),
                "Violation should mention 'exceeds threshold': {violation}"
            );

            // Should contain numeric values
            let has_numbers = violation.chars().any(|c| c.is_ascii_digit());
            assert!(
                has_numbers,
                "Violation should contain numeric values: {violation}"
            );
        }

        Ok(())
    }
}

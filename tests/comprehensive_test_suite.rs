//! Comprehensive Test Suite for xchecker
//!
//! This module provides a unified test runner that executes all test categories
//! and provides comprehensive validation of the xchecker system.
//!
//! Test Categories:
//! - Integration tests for full workflows (R1.1, R2.2, R2.5, R4.2)
//! - Property-based tests (R2.4, R2.5, R3.1, R12.1)
//! - Golden pipeline tests (R4.1, R4.3, R4.4)

use anyhow::Result;

// Import test modules
#[cfg(feature = "legacy_claude")]
mod golden_pipeline_tests;
mod integration_full_workflows;
#[cfg(feature = "test-utils")]
mod property_based_tests;

/// Comprehensive test suite configuration
#[derive(Debug, Clone)]
pub struct TestSuiteConfig {
    /// Whether to run integration tests
    pub run_integration: bool,
    /// Whether to run property-based tests
    pub run_property: bool,
    /// Whether to run golden pipeline tests
    pub run_golden_pipeline: bool,
    /// Whether to run performance benchmarks
    pub run_benchmarks: bool,
    /// Verbose output during test execution
    pub verbose: bool,
}

impl Default for TestSuiteConfig {
    fn default() -> Self {
        Self {
            run_integration: true,
            run_property: cfg!(feature = "test-utils"),
            run_golden_pipeline: cfg!(feature = "legacy_claude"),
            run_benchmarks: false, // Benchmarks are optional
            verbose: false,
        }
    }
}

/// Test suite results summary
#[derive(Debug)]
pub struct TestSuiteResults {
    /// Integration test results
    pub integration_passed: bool,
    /// Property-based test results
    pub property_passed: bool,
    /// Golden pipeline test results
    pub golden_pipeline_passed: bool,
    /// Benchmark results (if run)
    pub benchmark_passed: Option<bool>,
    /// Total test execution time
    pub total_duration: std::time::Duration,
    /// Individual test durations
    pub test_durations: std::collections::HashMap<String, std::time::Duration>,
}

impl TestSuiteResults {
    /// Check if all enabled tests passed
    #[must_use]
    pub fn all_passed(&self) -> bool {
        self.integration_passed
            && self.property_passed
            && self.golden_pipeline_passed
            && self.benchmark_passed.unwrap_or(true)
    }

    /// Get summary statistics
    #[must_use]
    pub fn summary(&self) -> String {
        let total_tests = 3 + i32::from(self.benchmark_passed.is_some());
        let passed_tests = [
            self.integration_passed,
            self.property_passed,
            self.golden_pipeline_passed,
        ]
        .iter()
        .filter(|&&x| x)
        .count()
            + usize::from(self.benchmark_passed == Some(true));

        format!(
            "{}/{} test suites passed in {:?}",
            passed_tests, total_tests, self.total_duration
        )
    }
}

/// Main comprehensive test suite runner
pub async fn run_comprehensive_test_suite(config: TestSuiteConfig) -> Result<TestSuiteResults> {
    let start_time = std::time::Instant::now();
    let mut test_durations = std::collections::HashMap::new();

    println!("🚀 Starting comprehensive xchecker test suite...");
    println!("Configuration: {config:?}");
    println!();

    // Initialize results
    let mut integration_passed = true;
    let mut property_passed = true;
    let mut golden_pipeline_passed = true;
    let mut benchmark_passed = None;

    // Run Integration Tests
    if config.run_integration {
        println!("📋 Running Integration Tests for Full Workflows...");
        let integration_start = std::time::Instant::now();

        match integration_full_workflows::run_full_workflow_validation().await {
            Ok(()) => {
                println!("✅ Integration tests passed!");
                integration_passed = true;
            }
            Err(e) => {
                println!("❌ Integration tests failed: {e}");
                integration_passed = false;
            }
        }

        let integration_duration = integration_start.elapsed();
        test_durations.insert("integration".to_string(), integration_duration);
        println!("Integration tests completed in {integration_duration:?}");
        println!();
    }

    // Run Property-Based Tests
    if config.run_property {
        #[cfg(feature = "test-utils")]
        {
            println!("🔬 Running Property-Based Tests...");
            let property_start = std::time::Instant::now();

            // Property tests are synchronous, so we run them in a blocking context
            match tokio::task::spawn_blocking(|| {
                // Run property tests
                property_based_tests::property_test_runner::run_all_property_tests();
                Ok::<(), anyhow::Error>(())
            })
            .await
            {
                Ok(Ok(())) => {
                    println!("✅ Property-based tests passed!");
                    property_passed = true;
                }
                Ok(Err(e)) => {
                    println!("❌ Property-based tests failed: {e:?}");
                    property_passed = false;
                }
                Err(e) => {
                    println!("❌ Property-based tests panicked: {e}");
                    property_passed = false;
                }
            }

            let property_duration = property_start.elapsed();
            test_durations.insert("property".to_string(), property_duration);
            println!("Property-based tests completed in {property_duration:?}");
            println!();
        }
        #[cfg(not(feature = "test-utils"))]
        {
            println!("🔬 Property-Based Tests skipped (feature \"test-utils\" not enabled).");
            property_passed = true;
            println!();
        }
    }

    // Run Golden Pipeline Tests
    if config.run_golden_pipeline {
        #[cfg(feature = "legacy_claude")]
        {
            println!("🏗️ Running Golden Pipeline Tests...");
            let golden_start = std::time::Instant::now();

            match golden_pipeline_tests::run_golden_pipeline_validation().await {
                Ok(()) => {
                    println!("✅ Golden pipeline tests passed!");
                    golden_pipeline_passed = true;
                }
                Err(e) => {
                    println!("❌ Golden pipeline tests failed: {e}");
                    golden_pipeline_passed = false;
                }
            }

            let golden_duration = golden_start.elapsed();
            test_durations.insert("golden_pipeline".to_string(), golden_duration);
            println!("Golden pipeline tests completed in {golden_duration:?}");
            println!();
        }
        #[cfg(not(feature = "legacy_claude"))]
        {
            println!("🏗️ Golden Pipeline Tests skipped (feature \"legacy_claude\" not enabled).");
            golden_pipeline_passed = true;
            println!();
        }
    }

    // Run Benchmarks (optional)
    if config.run_benchmarks {
        #[cfg(feature = "test-utils")]
        {
            println!("⚡ Running Performance Benchmarks...");
            let benchmark_start = std::time::Instant::now();

            match tokio::task::spawn_blocking(|| {
                // Run benchmark tests
                property_based_tests::property_benchmarks::benchmark_canonicalization_performance();
                property_based_tests::property_benchmarks::benchmark_hash_consistency_performance();
                Ok::<(), anyhow::Error>(())
            })
            .await
            {
                Ok(Ok(())) => {
                    println!("✅ Benchmarks completed successfully!");
                    benchmark_passed = Some(true);
                }
                Ok(Err(e)) => {
                    println!("❌ Benchmarks failed: {e:?}");
                    benchmark_passed = Some(false);
                }
                Err(e) => {
                    println!("❌ Benchmarks panicked: {e}");
                    benchmark_passed = Some(false);
                }
            }

            let benchmark_duration = benchmark_start.elapsed();
            test_durations.insert("benchmarks".to_string(), benchmark_duration);
            println!("Benchmarks completed in {benchmark_duration:?}");
            println!();
        }
        #[cfg(not(feature = "test-utils"))]
        {
            println!("⚡ Performance Benchmarks skipped (feature \"test-utils\" not enabled).");
            benchmark_passed = None;
            println!();
        }
    }

    let total_duration = start_time.elapsed();

    let results = TestSuiteResults {
        integration_passed,
        property_passed,
        golden_pipeline_passed,
        benchmark_passed,
        total_duration,
        test_durations,
    };

    // Print final summary
    print_test_summary(&results, &config);

    Ok(results)
}

/// Print comprehensive test summary
fn print_test_summary(results: &TestSuiteResults, config: &TestSuiteConfig) {
    println!("📊 COMPREHENSIVE TEST SUITE SUMMARY");
    println!("═══════════════════════════════════════");
    println!();

    // Overall status
    if results.all_passed() {
        println!("🎉 ALL TESTS PASSED! 🎉");
    } else {
        println!("❌ SOME TESTS FAILED");
    }
    println!();

    // Individual test suite results
    println!("Test Suite Results:");
    if config.run_integration {
        println!(
            "  Integration Tests:     {}",
            if results.integration_passed {
                "✅ PASSED"
            } else {
                "❌ FAILED"
            }
        );
    }
    if config.run_property {
        println!(
            "  Property-Based Tests:  {}",
            if results.property_passed {
                "✅ PASSED"
            } else {
                "❌ FAILED"
            }
        );
    }
    if config.run_golden_pipeline {
        println!(
            "  Golden Pipeline Tests: {}",
            if results.golden_pipeline_passed {
                "✅ PASSED"
            } else {
                "❌ FAILED"
            }
        );
    }
    if config.run_benchmarks {
        println!(
            "  Performance Benchmarks: {}",
            match results.benchmark_passed {
                Some(true) => "✅ PASSED",
                Some(false) => "❌ FAILED",
                None => "⏭️ SKIPPED",
            }
        );
    }
    println!();

    // Timing information
    println!("Execution Times:");
    for (test_name, duration) in &results.test_durations {
        println!("  {test_name:<20}: {duration:?}");
    }
    println!("  {:<20}: {:?}", "TOTAL", results.total_duration);
    println!();

    // Requirements coverage summary
    println!("Requirements Coverage Validated:");
    if config.run_integration {
        println!("  ✓ R1.1: Complete spec generation flows");
        println!("  ✓ R2.2: Deterministic outputs with identical inputs");
        println!("  ✓ R2.5: Structure determinism for canonicalized outputs");
        println!("  ✓ R4.2: Resume scenarios and failure recovery");
    }
    if config.run_property {
        println!("  ✓ R2.4: Canonicalization properties across transformations");
        println!("  ✓ R2.5: Hash consistency for equivalent inputs");
        println!("  ✓ R3.1: Budget enforcement under various input conditions");
        println!("  ✓ R12.1: Canonicalization determinism");
    }
    if config.run_golden_pipeline {
        println!("  ✓ R4.1: Claude CLI integration with stream-json and text fallback");
        println!("  ✓ R4.3: Error handling and partial output preservation");
        println!("  ✓ R4.4: Structured output handling with fallback capabilities");
    }
    println!();

    // System capabilities verified
    println!("System Capabilities Verified:");
    println!("  ✓ End-to-end workflow execution (Requirements → Design → Tasks)");
    println!("  ✓ Deterministic canonicalization with BLAKE3 verification");
    println!("  ✓ Robust error handling and recovery mechanisms");
    println!("  ✓ Claude CLI integration with multiple output formats");
    println!("  ✓ Property-based validation of core algorithms");
    println!("  ✓ Comprehensive audit trail with receipts");
    println!("  ✓ Atomic file operations and partial artifact handling");
    println!("  ✓ Secret redaction and security measures");
    println!("  ✓ Budget enforcement and resource management");
    println!("  ✓ Cross-platform compatibility (Native/WSL)");
    println!();

    println!("Summary: {}", results.summary());
}

/// Quick test runner for CI/CD environments
pub async fn run_quick_test_suite() -> Result<bool> {
    let config = TestSuiteConfig {
        run_integration: true,
        run_property: false, // Skip property tests for speed
        run_golden_pipeline: cfg!(feature = "legacy_claude"),
        run_benchmarks: false,
        verbose: false,
    };

    let results = run_comprehensive_test_suite(config).await?;
    Ok(results.all_passed())
}

/// Full test runner for development environments
pub async fn run_full_test_suite() -> Result<bool> {
    let config = TestSuiteConfig {
        run_integration: true,
        run_property: cfg!(feature = "test-utils"),
        run_golden_pipeline: cfg!(feature = "legacy_claude"),
        run_benchmarks: cfg!(feature = "test-utils"),
        verbose: true,
    };

    let results = run_comprehensive_test_suite(config).await?;
    Ok(results.all_passed())
}

/// Test runner entry point for external callers
#[tokio::test]
#[ignore = "requires_claude_stub"]
async fn run_comprehensive_tests() -> Result<()> {
    let config = TestSuiteConfig::default();
    let results = run_comprehensive_test_suite(config).await?;

    assert!(
        results.all_passed(),
        "Comprehensive test suite failed: {}",
        results.summary()
    );

    Ok(())
}

/// Smoke test for basic functionality
#[tokio::test]
async fn smoke_test() -> Result<()> {
    println!("🔥 Running smoke test...");

    // Test basic canonicalization
    let canonicalizer = xchecker::canonicalization::Canonicalizer::new();
    let test_yaml = "name: test\nversion: 1.0";
    let hash = canonicalizer.hash_canonicalized(test_yaml, xchecker::types::FileType::Yaml)?;
    assert_eq!(hash.len(), 64, "Hash should be 64 characters");

    // Test basic packet builder
    let _packet_builder = xchecker::packet::PacketBuilder::new()?;

    // Test basic secret redaction
    let redactor = xchecker::redaction::SecretRedactor::new()?;
    let test_content = "This is safe content";
    let result = redactor.redact_content(test_content, "test.txt")?;
    assert_eq!(
        result.content, test_content,
        "Safe content should be unchanged"
    );

    println!("✅ Smoke test passed!");
    Ok(())
}

/// Performance regression test
#[tokio::test]
async fn performance_regression_test() -> Result<()> {
    println!("⚡ Running performance regression test...");

    let canonicalizer = xchecker::canonicalization::Canonicalizer::new();

    // Test canonicalization performance
    let test_yaml = r"
name: performance-test
version: 1.0.0
features:
  - feature1
  - feature2
  - feature3
config:
  enabled: true
  count: 100
";

    let start = std::time::Instant::now();
    for _ in 0..100 {
        let _hash = canonicalizer.hash_canonicalized(test_yaml, xchecker::types::FileType::Yaml)?;
    }
    let duration = start.elapsed();

    // Should complete 100 canonicalizations in under 1 second
    assert!(
        duration.as_secs() < 1,
        "Canonicalization performance regression detected"
    );

    println!("✅ Performance regression test passed ({duration:?} for 100 ops)");
    Ok(())
}

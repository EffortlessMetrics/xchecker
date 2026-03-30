# xchecker justfile - Test lanes and common development tasks
# 
# Usage: just <recipe>
# Install just: https://github.com/casey/just#installation
#
# Test Profiles:
#   test-fast  - Quick feedback loop (lib + bins only, ~30s)
#   test-full  - Complete test suite (all tests, ~3-5min)
#   test-local - Local-green profile (no external deps)
#   test-stub  - Integration tests with claude-stub
#
# See docs/TESTING.md for detailed documentation.

set windows-shell := ["pwsh.exe", "-NoLogo", "-Command"]

# Default recipe - show available commands
default:
    @just --list

# ============================================================================
# TEST LANES
# ============================================================================

# Fast test lane: lib + bins only (~30s)
# Use for quick feedback during development
test-fast:
    cargo test --lib --bins

# Full test lane: all tests including property tests and integration
# Use for comprehensive validation before commits
test-full:
    cargo test --all-features

# Local-green profile: tests that don't require external dependencies
# Safe for CI without network, stub, or binary requirements
test-local:
    cargo test --lib
    cargo test --doc
    cargo test --tests -- \
        --skip requires_claude_stub \
        --skip requires_real_claude \
        --skip requires_xchecker_binary \
        --skip requires_future_phase \
        --skip requires_future_api \
        --skip requires_refactoring \
        --skip windows_ci_only

# Stub suite: integration tests with claude-stub
# Requires building claude-stub first
test-stub: build-stub
    cargo test --tests -- \
        --include-ignored \
        --skip requires_real_claude \
        --skip requires_xchecker_binary \
        --skip requires_future_phase \
        --skip requires_future_api \
        --skip requires_refactoring \
        --skip windows_ci_only

# Property-based tests only
test-pbt:
    cargo test --test property_based_tests

# Property-based tests with increased iterations (for thorough local testing)
test-pbt-thorough:
    PROPTEST_CASES=256 cargo test --test property_based_tests

# ============================================================================
# BUILD TARGETS
# ============================================================================

# Build claude-stub binary (required for stub tests)
build-stub:
    cargo build --bin claude-stub

# Build all binaries
build:
    cargo build --all-targets

# Build release binaries
build-release:
    cargo build --release --all-targets

# ============================================================================
# QUALITY CHECKS
# ============================================================================

# Run clippy linter
lint:
    cargo clippy --all-targets --all-features

# Check code formatting
fmt-check:
    cargo fmt -- --check

# Format code
fmt:
    cargo fmt

# Run all quality checks (lint + format check)
check: lint fmt-check

# Package every workspace crate the way crates.io will see it
package-publish-check:
    cargo package --workspace --allow-dirty --no-verify

# Run modularization verification gate (formatting, clippy, tests, dependency graph)
verify-modularization:
    @{{ if os_family() == "windows" { "pwsh -File scripts/verify-modularization.ps1" } else { "bash scripts/verify-modularization.sh" } }}

# ============================================================================
# DOCUMENTATION
# ============================================================================

# Run doc validation tests
test-docs:
    cargo test --test test_doc_validation -- --test-threads=1

# Validate schema examples
test-schemas:
    cargo test schema_examples_tests

# ============================================================================
# CI SIMULATION
# ============================================================================

# Simulate CI fast lane (what runs on every PR)
ci-fast: check test-fast test-docs

# Simulate CI full lane (what runs nightly)
ci-full: check test-full test-docs

# Show the crates.io publish order without publishing anything
publish-plan:
    @{{ if os_family() == "windows" { "pwsh -File scripts/publish-workspace.ps1" } else { "bash scripts/publish-workspace.sh" } }}

# Run tiered crates.io dry-runs in publish order
publish-dry-run:
    @{{ if os_family() == "windows" { "pwsh -File scripts/publish-workspace.ps1 -DryRun" } else { "bash scripts/publish-workspace.sh --dry-run" } }}

# Publish crates.io tiers in order
publish-execute:
    @{{ if os_family() == "windows" { "pwsh -File scripts/publish-workspace.ps1 -Execute" } else { "bash scripts/publish-workspace.sh --execute" } }}

# ============================================================================
# DEVELOPMENT HELPERS
# ============================================================================

# Watch for changes and run fast tests
watch:
    cargo watch -x "test --lib --bins"

# Clean build artifacts
clean:
    cargo clean

# Show test statistics
test-stats:
    @echo "Test file count:"
    @find tests -name "*.rs" | wc -l
    @echo ""
    @echo "Test function count (approximate):"
    @grep -r "#\[test\]" tests src --include="*.rs" | wc -l

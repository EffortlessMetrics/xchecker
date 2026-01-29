# Testing Guide

This document describes xchecker's test infrastructure, test lanes, and how to run tests effectively.

## Quick Start

```bash
# Optional: enter the Nix dev shell (pins Rust 1.89 + tools)
nix develop

# Install just (if not already installed)
# macOS: brew install just
# Windows: winget install Casey.Just
# Linux: cargo install just

# Run fast tests (recommended for development)
just test-fast

# Run full test suite
just test-full
```

## Test Lanes

xchecker uses a tiered test strategy to balance fast feedback with comprehensive coverage.

### `test-fast` - Quick Feedback (~30s)

```bash
just test-fast
# or: cargo test --lib --bins
```

**What runs:**
- Library unit tests (`src/**/*.rs`)
- Binary tests (`src/bin/**/*.rs`)

**When to use:**
- During active development
- Before committing small changes
- Quick sanity checks

### `test-full` - Complete Suite (~3-5min)

```bash
just test-full
# or: cargo test --all-features
```

**What runs:**
- All library and binary tests
- Property-based tests
- Integration tests
- Doc tests

**When to use:**
- Before pushing to remote
- Before creating PRs
- After significant changes

### `test-local` - Local-Green Profile

```bash
just test-local
```

**What runs:**
- All tests that don't require external dependencies
- Skips tests needing claude-stub, real Claude, or compiled binaries

**When to use:**
- CI environments without external tools
- Quick validation without building stubs

### `test-stub` - Integration with Claude Stub

```bash
just test-stub
```

**What runs:**
- Integration tests using the claude-stub mock
- Builds claude-stub automatically

**When to use:**
- Testing LLM integration paths
- Validating orchestrator behavior

## Test Categories

### By Dependency

| Category | Skip Flag | Description |
|----------|-----------|-------------|
| `requires_claude_stub` | `--skip requires_claude_stub` | Needs claude-stub binary |
| `requires_real_claude` | `--skip requires_real_claude` | Needs real Claude CLI + API |
| `requires_xchecker_binary` | `--skip requires_xchecker_binary` | Needs compiled xchecker |
| `requires_future_phase` | `--skip requires_future_phase` | Blocked on unimplemented phases |
| `requires_future_api` | `--skip requires_future_api` | Blocked on unimplemented APIs |
| `requires_refactoring` | `--skip requires_refactoring` | Needs code refactoring |
| `windows_ci_only` | `--skip windows_ci_only` | Windows-specific CI tests |

### By Type

| Type | Command | Description |
|------|---------|-------------|
| Unit tests | `cargo test --lib` | Pure unit tests, no I/O |
| Doc tests | `cargo test --doc` | Documentation examples |
| Integration | `cargo test --tests` | Full integration tests |
| Property | `cargo test --test property_based_tests` | Property-based tests |

## Property-Based Testing

xchecker uses [proptest](https://github.com/proptest-rs/proptest) for property-based testing.

### Running Property Tests

```bash
# Default (64 cases per property)
just test-pbt

# Thorough (256 cases per property)
just test-pbt-thorough

# Custom case count
PROPTEST_CASES=1000 cargo test --test property_based_tests
```

### Configuration

Property test behavior can be controlled via environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `PROPTEST_CASES` | 64 | Number of test cases per property |
| `PROPTEST_MAX_SHRINK_ITERS` | 1000 | Max shrinking iterations |
| `PROPTEST_DISABLE_FAILURE_PERSISTENCE` | unset | Set to `1` to disable failure persistence |

### Slow Test Caps

Some property tests are inherently slow (e.g., doctor tests that spawn processes). These tests have maximum case count caps that override `PROPTEST_CASES` when the environment variable would exceed the cap:

| Test Category | Max Cases | Reason |
|---------------|-----------|--------|
| Doctor tests | 5-20 | Spawn external processes |
| HTTP provider tests | 20 | Environment setup overhead |
| Standard tests | No cap | Fast in-memory operations |

This ensures CI completes in reasonable time even with high `PROPTEST_CASES` values.

### Regression Files

Failed property tests are recorded in `.proptest-regressions` files next to the test file. These are committed to the repository to ensure regressions are caught.

## LLM Provider Test Gating

Tests that call real LLM providers are gated behind environment variables:

| Variable | Description |
|----------|-------------|
| `XCHECKER_SKIP_LLM_TESTS=1` | Skip all real LLM tests |
| `XCHECKER_REAL_LLM_TESTS=1` | Enable real LLM tests |
| `XCHECKER_ENABLE_REAL_CLAUDE=1` | Enable real Claude API tests |

### Running Real LLM Tests

```bash
# Requires API keys to be set
XCHECKER_REAL_LLM_TESTS=1 cargo test --test smoke -- --ignored
```

**Warning:** Real LLM tests incur API costs. Use sparingly.

## Windows-Specific Testing

Some tests behave differently on Windows or are Windows-only:

```bash
# Run Windows-specific tests
cargo test -- windows

# Skip Windows-specific tests on other platforms
cargo test -- --skip windows_ci_only
```

### WSL Tests

WSL detection and runner tests are available on Windows:

```bash
# Run WSL probe test (Windows only)
cargo test test_wsl_probe -- --ignored --nocapture
```

## Heavy Tests

Some tests are resource-intensive and may be slow:

| Test | Duration | Notes |
|------|----------|-------|
| Property tests | ~60s | Runs many iterations |
| Doctor/WSL integration | ~30s | System probing |
| Large file handling | ~20s | Memory-intensive |
| Concurrent lockfile | ~15s | Parallel execution |

### Running Specific Subsets

```bash
# Skip heavy tests for quick iteration
cargo test --lib --bins

# Run only schema tests
cargo test schema

# Run only doctor tests
cargo test doctor

# Run tests matching a pattern
cargo test packet
```

## CI Configuration

### PR Checks (Fast)

Every PR runs:
- `cargo fmt -- --check`
- `cargo clippy --all-targets --all-features`
- `cargo test --all-features -- --test-threads=1`

### Nightly (Full)

Nightly builds run:
- All PR checks
- Property tests with increased iterations
- Real LLM tests (if credentials available)
- Cross-platform matrix (Linux, macOS, Windows)

## Troubleshooting

### Tests Fail with "requires_claude_stub"

Build the claude-stub binary first:

```bash
cargo build --bin claude-stub
```

Or use `just test-stub` which builds it automatically.

### Property Tests Timeout

Increase the timeout or reduce case count:

```bash
PROPTEST_CASES=32 cargo test --test property_based_tests
```

### Tests Fail on Windows

Some tests use Unix-specific features. Check for `#[cfg(unix)]` attributes or use:

```bash
cargo test -- --skip unix
```

### Flaky Tests

If a test is flaky, run it in isolation:

```bash
cargo test test_name -- --test-threads=1 --nocapture
```

## See Also

- [TEST_MATRIX.md](TEST_MATRIX.md) - Detailed test inventory
- [claude-stub.md](claude-stub.md) - Claude stub documentation
- [CI_PROFILES.md](CI_PROFILES.md) - CI configuration details

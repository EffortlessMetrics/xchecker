# CI Test Profiles

This document describes the different CI test profiles used in the xchecker project. Each profile is designed for specific testing scenarios and environments.

---

## Local-Green Profile

### Overview

The **Local-Green** profile is the primary CI profile for fast, reliable testing across all platforms. It runs the majority of the test suite without requiring any external dependencies, network access, or additional binaries.

### Characteristics

- **Test Coverage**: 791 tests (92.7% of total test suite)
- **Duration**: ~30 seconds
- **Platforms**: All (Linux, macOS, Windows)
- **Dependencies**: None (no external services, APIs, or binaries required)
- **Network**: No network calls
- **Stability**: High - designed to be deterministic and always pass

### Command

```bash
cargo test --tests -- \
  --skip requires_claude_stub \
  --skip requires_real_claude \
  --skip requires_xchecker_binary \
  --skip requires_future_phase \
  --skip requires_future_api \
  --skip requires_refactoring \
  --skip windows_ci_only
```

### Excluded Test Categories

The Local-Green profile skips the following test categories:

1. **requires_claude_stub**: Tests requiring Claude API stub/mock setup
2. **requires_real_claude**: Tests requiring actual Claude API access
3. **requires_xchecker_binary**: Tests requiring a compiled xchecker binary
4. **requires_future_phase**: Tests for features planned for future phases
5. **requires_future_api**: Tests for API features not yet implemented
6. **requires_refactoring**: Tests that need code refactoring before they can pass
7. **windows_ci_only**: Tests that should only run on Windows CI environments

### GitHub Actions Configuration

Here's the recommended GitHub Actions job configuration for the Local-Green profile:

```yaml
name: Local-Green Tests

on:
  push:
    branches: [ main ]
  pull_request:
    branches: [ main ]

jobs:
  tests-local-green:
    name: Local-Green (${{ matrix.os }})
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]

    steps:
      - name: Checkout code
        uses: actions/checkout@v4

      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          override: true

      - name: Run Local-Green tests
        run: |
          cargo test --tests -- \
            --skip requires_claude_stub \
            --skip requires_real_claude \
            --skip requires_xchecker_binary \
            --skip requires_future_phase \
            --skip requires_future_api \
            --skip requires_refactoring \
            --skip windows_ci_only
```

### Expected Behavior

The Local-Green profile is designed to:

- **Always pass** on all platforms (Linux, macOS, Windows)
- **Run quickly** (~30 seconds) to provide fast feedback
- **Require no setup** beyond Rust toolchain installation
- **Work offline** with no network connectivity required
- **Be deterministic** with no flaky tests or race conditions

### When to Use

Use the Local-Green profile when:

- Running pre-commit checks locally
- Setting up CI for pull requests
- Verifying cross-platform compatibility
- Running tests in environments without external service access
- Needing fast feedback during development

### Troubleshooting

If Local-Green tests fail:

1. **Check platform-specific issues**: Ensure the failure isn't due to a platform-specific bug
2. **Verify no external dependencies**: Confirm the test doesn't accidentally depend on external resources
3. **Review test isolation**: Ensure tests are properly isolated and don't interfere with each other
4. **Check for timing issues**: Verify there are no race conditions or timing-dependent assertions

### Maintenance Notes

When adding new tests to the project:

- By default, new tests should be included in the Local-Green profile
- If a test requires external dependencies, mark it with the appropriate skip tag
- Keep the Local-Green profile fast - tests taking >5 seconds individually should be reviewed
- Ensure new tests are deterministic and platform-agnostic unless specifically tagged

---

## Doc Validation Profile

### Overview

The **Doc Validation** profile validates documentation accuracy, code examples, and schema conformance. This profile ensures that all documentation examples compile and work correctly, and that JSON schema examples match their schemas.

### Characteristics

- **Test Coverage**: Doctests + schema validation tests
- **Duration**: Fast (~5 seconds)
- **Platforms**: All (Linux, macOS, Windows)
- **Dependencies**: None (no external services or APIs required)
- **Network**: No network calls
- **Stability**: High - deterministic validation tests

### Commands

```bash
# Run doctests (tests embedded in /// doc comments)
cargo test --doc

# Run schema example validation tests
cargo test schema_examples_tests
```

### What This Validates

1. **Doctests**: Tests embedded in source code documentation comments
   - Config API examples
   - Usage patterns in doc comments
   - Code snippets throughout `src/`
   - Ensures examples compile and execute correctly

2. **Schema Examples**: Tests in `tests/doc_validation/schema_examples_tests.rs`
   - Validates receipt/status/doctor schema examples
   - Ensures examples conform to JSON schemas
   - Verifies example generation functions work correctly
   - Checks array sorting and determinism

### Part of Local-Green Suite

The doc validation profile is part of the **local-green baseline** and must remain green at all times. These tests are critical for:
- **Documentation accuracy**: Ensures examples in docs actually work
- **Schema conformance**: Validates JSON outputs match their schemas
- **API contract validation**: Proves documented APIs behave as specified

### CI Integration

Include in your CI pipeline as part of the standard test suite:

```yaml
- name: Run doc validation
  run: |
    cargo test --doc
    cargo test schema_examples_tests
```

### Expected Behavior

The Doc Validation profile is designed to:

- **Catch outdated examples**: Documentation examples that no longer compile
- **Validate schema compliance**: JSON examples that don't match schemas
- **Ensure API accuracy**: Documented APIs match implementation
- **Be fast**: Complete in ~5 seconds for quick feedback

### When to Use

Use the Doc Validation profile when:

- Making changes to public APIs
- Updating documentation or examples
- Modifying JSON schemas
- Adding new example code to docs
- Running full CI validation

### Maintenance Notes

When updating code that affects documentation:

- Run `cargo test --doc` to verify examples still work
- Run schema validation tests if changing JSON output
- Update examples in doc comments to reflect API changes
- Ensure new public APIs include working examples

---

## Stub Suite Profile

### Overview

The **Stub Suite** profile extends Local-Green by including integration tests that use the `claude-stub` binary to mock Claude API responses. This provides comprehensive integration testing without incurring API costs or requiring network access.

### Characteristics

- **Test Coverage**: 840 tests (98.5% of total test suite)
- **Duration**: ~2 minutes
- **Platforms**: All (Linux, macOS, Windows)
- **Dependencies**: `claude-stub` binary (must be built first)
- **Network**: No network calls (mocked responses)
- **Stability**: High - deterministic mocked responses

### Prerequisites

The Stub Suite requires building the `claude-stub` binary before running tests:

```bash
cargo build --bin claude-stub
```

### Command

```bash
# Build claude-stub first
cargo build --bin claude-stub

# Run all tests except real Claude
cargo test --tests --include-ignored -- \
  --skip requires_real_claude \
  --skip requires_xchecker_binary \
  --skip requires_future_phase \
  --skip requires_future_api \
  --skip requires_refactoring \
  --skip windows_ci_only
```

### What This Adds Over Local-Green

The Stub Suite includes 49 additional tests marked with `#[ignore = "requires_claude_stub"]`:

- M1 gate integration tests (7 tests)
- M1 gate simple validation tests (8 tests)
- M3/M4 gate validation tests
- Golden pipeline tests (7 tests)
- End-to-end workflow tests with mocked LLM (6 tests)

### GitHub Actions Configuration

**Status**: Not currently automated (optional/manual)

Recommended configuration if you want to enable it:

```yaml
stub-suite:
  name: Stub Suite (${{ matrix.os }})
  runs-on: ${{ matrix.os }}
  strategy:
    matrix:
      os: [ubuntu-latest, macos-latest, windows-latest]

  steps:
    - name: Checkout code
      uses: actions/checkout@v4

    - name: Setup Rust
      uses: dtolnay/rust-toolchain@stable

    - name: Build claude-stub
      run: cargo build --bin claude-stub

    - name: Run Stub Suite
      run: |
        cargo test --tests --include-ignored -- \
          --skip requires_real_claude \
          --skip requires_xchecker_binary \
          --skip requires_future_phase \
          --skip requires_future_api \
          --skip requires_refactoring \
          --skip windows_ci_only
```

### When to Use

Use the Stub Suite when:

- Testing LLM interaction logic without API costs
- Validating phase transitions and orchestration flows
- Pre-merge integration validation (more thorough than Local-Green)
- Local development when you need comprehensive coverage
- Debugging integration issues without hitting real APIs

### Excluded Tests

The Stub Suite still skips:

- **requires_real_claude** (4 tests): Real Claude API smoke tests
- **requires_xchecker_binary** (2 tests): Binary integration tests
- **requires_future_phase/api** (4 tests): Unimplemented features
- **requires_refactoring** (2 tests): Tests needing code updates
- **windows_ci_only** (1 test): Platform-specific tests

---

## Firehose Profile (All Tests)

### Overview

The **Firehose** profile runs **all 853 tests** including those that make real Claude API calls. This is the most comprehensive test suite but also the most expensive and time-consuming. It should only be used for pre-release validation or manual investigation of real-world issues.

### Characteristics

- **Test Coverage**: 853 tests (100% of all tests)
- **Duration**: ~5-10 minutes
- **Platforms**: Linux only (for cost control)
- **Dependencies**: Real Claude API access, all binaries, secrets
- **Network**: Required (real API calls)
- **Stability**: Low - network-dependent, can be flaky

### Prerequisites

The Firehose profile requires:

- ✅ Real Claude API access (Anthropic API key)
- ✅ `claude` CLI binary available and configured, OR
- ✅ `ANTHROPIC_API_KEY` environment variable set
- ✅ `xchecker` binary compiled (`cargo build --release`)
- ✅ `claude-stub` binary compiled (`cargo build --bin claude-stub`)
- ✅ Environment variable: `XCHECKER_ENABLE_REAL_CLAUDE=1`

### Command

```bash
XCHECKER_ENABLE_REAL_CLAUDE=1 cargo test --tests --include-ignored
```

### GitHub Actions Specification

**Trigger**: MANUAL or NIGHTLY only (NOT on every PR/push)

**Recommended configuration**:

```yaml
name: Firehose (Real Claude API)

on:
  # Manual trigger only
  workflow_dispatch:

  # OR nightly schedule (choose one)
  # schedule:
  #   - cron: '0 2 * * *'  # 2 AM UTC daily

jobs:
  firehose:
    name: Firehose - All Tests (Real Claude)
    runs-on: ubuntu-latest  # Linux only for cost control

    steps:
      - name: Checkout code
        uses: actions/checkout@v4

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Build xchecker binary
        run: cargo build --release

      - name: Build claude-stub binary
        run: cargo build --bin claude-stub

      - name: Run Firehose test suite
        run: XCHECKER_ENABLE_REAL_CLAUDE=1 cargo test --tests --include-ignored
        env:
          ANTHROPIC_API_KEY: ${{ secrets.ANTHROPIC_API_KEY }}
        continue-on-error: true  # Don't fail build on flaky network issues

      - name: Upload test results
        if: always()
        uses: actions/upload-artifact@v3
        with:
          name: firehose-test-results
          path: |
            target/debug/test-results/
            firehose.log
```

**Required GitHub Secrets**:
- `ANTHROPIC_API_KEY` - Your Anthropic Claude API key

**Platform**: Linux only (for cost control)

### Warnings and Limitations

⚠️ **Costs Money**
- Makes real API calls to Claude
- Each test run incurs API usage charges
- Estimated cost: $0.01 - $0.05 per run (based on 10-15 API calls)
- Rate limits may apply depending on your API tier

⚠️ **Can Be Flaky**
- Network dependency (internet connectivity required)
- API rate limits may cause intermittent failures
- External service availability affects test stability
- Timeouts possible under high load or slow connections

⚠️ **Takes Time**
- Full suite runs ~5-10 minutes (vs. 30s for Local-Green)
- Not suitable for fast feedback loops
- Blocks on network I/O and API response times
- Serial execution may be required to avoid rate limits

⚠️ **Not for Routine Validation**
- Too slow for PR validation
- Too expensive for every commit
- Too unreliable for blocking CI gates
- Use Local-Green or Stub Suite for routine checks

### When to Use

✅ **Appropriate scenarios**:
- Before major releases (v1.0, v2.0, etc.)
- Investigating real-world integration issues
- Validating Claude API compatibility after LLM updates
- Pre-deployment smoke testing in staging environments
- Manual validation of critical bug fixes
- Nightly regression testing (scheduled, non-blocking)

❌ **Inappropriate scenarios**:
- Every pull request (use Local-Green instead)
- Every commit to main (use Stub Suite instead)
- Local development loops (use Local-Green)
- Blocking CI gates (too slow and flaky)

### Real Claude API Tests

The following tests require real Claude API access (marked with `#[ignore = "requires_real_claude"]`):

| Test File | Test Function | Purpose |
|-----------|---------------|---------|
| `tests/smoke.rs` | `test_real_claude_basic_interaction` | Verify real Claude CLI works |
| `tests/smoke.rs` | `test_real_claude_streaming_response` | Verify streaming API works |
| `tests/test_exit_alignment.rs` | `test_xchecker_exit_code_success` | Verify exit codes with real binary |
| `tests/test_exit_alignment.rs` | `test_xchecker_exit_code_failure` | Verify error exit codes |

### Cost Analysis

**API Calls per Run**:
- Real Claude tests: 4 test functions
- Each test may make 1-3 API calls
- Estimated: ~10-15 API calls per Firehose run

**Monthly Cost Scenarios** (rough estimates):

| Frequency | Runs/Month | Est. Cost/Month |
|-----------|------------|-----------------|
| **Manual only** | 5-10 | $0.05 - $0.50 |
| **Nightly** | 30 | $0.30 - $1.50 |
| **Per-commit** | 100+ | $1.00 - $5.00+ |

**Recommendation**: Manual or nightly only, NOT per-commit or per-PR

---

## Profile Comparison Matrix

### Quick Reference

| Profile | Test Count | Duration | Cost | Network | Use Case |
|---------|-----------|----------|------|---------|----------|
| **Local-Green** | 791 (92.7%) | ~30s | Free | No | Default CI, PR validation |
| **Stub Suite** | 840 (98.5%) | ~2min | Free | No | Integration testing |
| **Firehose** | 853 (100%) | ~5-10min | $$ | Yes | Pre-release, real-world validation |

### Detailed Comparison

#### Capabilities

| Feature | Local-Green | Stub Suite | Firehose |
|---------|-------------|------------|----------|
| **Unit tests** | ✅ | ✅ | ✅ |
| **Dry-run integration** | ✅ | ✅ | ✅ |
| **Stub-based integration** | ❌ | ✅ | ✅ |
| **Real Claude API** | ❌ | ❌ | ✅ |
| **Binary integration** | ❌ | ❌ | ✅ |
| **Network required** | ❌ | ❌ | ✅ |
| **API costs** | ❌ | ❌ | ✅ |

#### Performance

| Metric | Local-Green | Stub Suite | Firehose |
|--------|-------------|------------|----------|
| **Test count** | 791 (92.7%) | 840 (98.5%) | 853 (100%) |
| **Duration** | ~30s | ~2min | ~5-10min |
| **Parallelizable** | ✅ | ✅ | ⚠️ (rate limits) |
| **Deterministic** | ✅ | ✅ | ❌ (network) |
| **Cacheable** | ✅ | ✅ | ❌ |

#### CI Strategy Recommendations

| Scenario | Recommended Profile | Rationale |
|----------|---------------------|-----------|
| **PR validation** | Local-Green | Fast feedback, no flakiness |
| **Merge to main** | Local-Green | Sufficient coverage for routine changes |
| **Pre-release** | Firehose | Comprehensive validation before shipping |
| **Nightly** | Firehose | Catch real-world integration issues |
| **Manual testing** | Stub Suite or Firehose | Depends on what you're validating |
| **Local dev** | Local-Green | Fast iteration cycle |

---

## Test Ignore Markers

All tests use standardized `#[ignore = "reason"]` attributes for consistency.

### Standard Ignore Reasons

| Marker | Count | Description | Included In |
|--------|-------|-------------|-------------|
| `requires_claude_stub` | 49 | Needs `claude-stub` binary | Stub Suite, Firehose |
| `requires_real_claude` | 4 | Real Claude CLI + API | Firehose only |
| `requires_xchecker_binary` | 2 | Compiled `xchecker` binary | Firehose only |
| `requires_future_phase` | 2 | Unimplemented phase (Review, Final) | None (will fail) |
| `requires_future_api` | 2 | API not yet wired | None (will fail) |
| `requires_refactoring` | 2 | Needs code refactoring | None (will fail) |
| `windows_ci_only` | 1 | Windows-specific test | Platform-specific |

### Running Specific Markers

```bash
# Run ONLY tests with a specific marker
cargo test --tests -- --ignored --test requires_real_claude

# Skip tests with a specific marker
cargo test --tests -- --skip requires_real_claude

# Run all ignored tests (Firehose mode)
cargo test --tests -- --include-ignored
```

---

## Environment Variables

### `XCHECKER_ENABLE_REAL_CLAUDE`

Controls whether tests should attempt real Claude API calls.

**Values**:
- `1` or `true` - Enable real Claude API calls (Firehose mode)
- Unset or `0` - Disable real Claude API calls (default)

**Usage**:
```bash
# Enable for Firehose
XCHECKER_ENABLE_REAL_CLAUDE=1 cargo test --tests --include-ignored

# Disable (default)
cargo test --tests
```

### `ANTHROPIC_API_KEY`

Required for real Claude API calls (Firehose profile).

**Set in GitHub Actions**:
```yaml
env:
  ANTHROPIC_API_KEY: ${{ secrets.ANTHROPIC_API_KEY }}
```

**Set locally**:
```bash
export ANTHROPIC_API_KEY="sk-ant-..."
XCHECKER_ENABLE_REAL_CLAUDE=1 cargo test --tests --include-ignored
```

---

## Best Practices

### For Developers

**Local development**:
1. Use Local-Green for fast iteration: `cargo test --lib`
2. Use Stub Suite before pushing: Build stub, then run with `--include-ignored --skip requires_real_claude`
3. Only use Firehose when debugging real API issues

**Pull requests**:
1. Ensure Local-Green passes before creating PR
2. Do NOT run Firehose in CI for every PR
3. Add appropriate `#[ignore = "..."]` markers to new tests

**Pre-release**:
1. Run Stub Suite to catch integration issues
2. Run Firehose manually to validate real API compatibility
3. Document any Firehose failures in release notes

### For CI/CD

**Required gates** (fast, reliable):
- ✅ Local-Green on all PRs
- ✅ Lint and format checks
- ✅ Schema validation
- ✅ Secret scanning

**Optional gates** (slower, comprehensive):
- ⚠️ Stub Suite on main branch
- ⚠️ Parallel tests (non-blocking, validate stability)

**Manual/Nightly only** (expensive, slow, flaky):
- ❌ Firehose - NOT for routine CI

---

## CI Jobs Reference

This table describes the current GitHub Actions jobs and their required/optional status:

| Job | Workflow | When it runs | Required for PRs? | Description |
|-----|----------|--------------|-------------------|-------------|
| `lint` | ci.yml | PRs, main | ✅ Yes | Format + clippy checks |
| `test-serial` | ci.yml | PRs, main | ✅ Yes | Serial tests on all 3 OS |
| `test-parallel` | ci.yml | PRs, main | ❌ No | Parallel tests (non-blocking, validating stability) |
| `schema-validation` | ci.yml | PRs, main | ✅ Yes | JSON schema compliance |
| `secret-scanning` | ci.yml | PRs, main | ✅ Yes | Secret detection tests |
| `docs-conformance` | ci.yml | PRs, main | ✅ Yes | Documentation validation |
| `gate-validation` | ci.yml | PRs, main | ✅ Yes | Gate command tests |
| `test-real` | ci.yml | main only | ❌ No | Real Claude API (requires secret) |
| `test-fast` | test.yml | PRs only | ✅ Yes | Quick unit tests (~30s) |
| `test-full` | test.yml | main, nightly | ❌ No | Comprehensive tests |
| `property-tests` | test.yml | main, nightly | ❌ No | Property-based tests with high case count |
| `stub-tests` | test.yml | PRs, main, nightly | ❌ No | Integration tests with claude-stub (non-blocking) |
| `example-validation` | test.yml | All events | ✅ Yes | Validate showcase examples |
| `walkthrough-validation` | test.yml | All events | ✅ Yes | Validate walkthrough snippets |

### Stub Integration Stance

The `stub-tests` job currently runs on PRs but is **not required** in branch protection. This provides:

- **Visibility**: PR authors see stub-dependent test results before merge
- **Non-blocking**: Failures don't block merges while we validate stability
- **Path to required**: After 3 consecutive stable weeks, consider making this required

To promote stub-tests to required:
1. Monitor stability in PR feedback for 3+ weeks
2. If consistently green, add to branch protection required checks
3. Update this documentation when promoting

---

## Troubleshooting

### Firehose Failures

**Symptom**: Firehose tests fail with network errors

**Possible causes**:
1. No internet connectivity
2. API rate limits exceeded
3. `ANTHROPIC_API_KEY` not set or invalid
4. Claude CLI not installed

**Solutions**:
1. Check network: `curl https://api.anthropic.com`
2. Wait for rate limit reset (check API dashboard)
3. Verify API key: `echo $ANTHROPIC_API_KEY`
4. Install Claude CLI or set API key directly

**Symptom**: Firehose tests are slow

**Possible causes**:
1. Network latency
2. API response times
3. Rate limiting backoff

**Solutions**:
1. Run with `--test-threads=1` to avoid rate limits
2. Run subset: `cargo test --test smoke -- --ignored`
3. Use Stub Suite for faster feedback

### Stub Suite Failures

**Symptom**: Tests fail with "claude-stub not found"

**Possible causes**:
1. `claude-stub` binary not built
2. Wrong PATH configuration

**Solutions**:
```bash
# Build claude-stub first
cargo build --bin claude-stub

# Verify it's built
ls target/debug/claude-stub  # Unix
dir target\debug\claude-stub.exe  # Windows

# Run tests
cargo test --tests --include-ignored -- --skip requires_real_claude
```

---

## Gate Workflow Patterns

xchecker provides a `gate` command for enforcing spec completion policies in CI. There are two patterns for integrating the gate into your workflow:

### Smoke Gate Pattern (Default)

The **Smoke Gate** pattern validates that the gate command and JSON output work correctly, but does NOT fail the CI job when the spec doesn't meet requirements. This is useful for:

- Initial integration testing
- Demonstrating gate functionality
- Non-blocking informational checks

**Behavior**: Always exits 0 if the gate command runs successfully, regardless of `passed` status.

```yaml
- name: Run gate check (smoke)
  run: |
    set +e
    ./target/release/xchecker gate my-spec --min-phase tasks --json > gate-result.json
    GATE_STATUS=$?

    # Validate JSON structure
    PASSED=$(cat gate-result.json | jq -r '.passed')

    if [ "$PASSED" = "true" ]; then
      echo "Gate PASSED"
    else
      echo "Gate returned passed=false (informational)"
      echo "Failure reasons:"
      cat gate-result.json | jq -r '.failure_reasons[]'
    fi

    # Always exit 0 for smoke test
    exit 0
```

### Strict Gate Pattern (Production)

The **Strict Gate** pattern enforces spec policies as blocking CI checks. When the gate returns `passed=false`, the CI job fails and blocks the merge. This is the recommended pattern for production use.

**Behavior**: Exits non-zero when `passed=false`, blocking the PR/merge.

```yaml
- name: Run gate check (strict)
  run: |
    # Run gate and capture exit code
    ./target/release/xchecker gate my-spec \
      --min-phase tasks \
      --fail-on-pending-fixups \
      --json > gate-result.json

    GATE_STATUS=$?

    # Display result
    cat gate-result.json | jq .

    # Check if gate passed
    PASSED=$(cat gate-result.json | jq -r '.passed')

    if [ "$PASSED" = "true" ]; then
      echo "✓ Gate PASSED - spec meets all policy requirements"
      exit 0
    else
      echo "✗ Gate FAILED - spec does not meet policy requirements"
      echo ""
      echo "Failure reasons:"
      cat gate-result.json | jq -r '.failure_reasons[]'
      echo ""
      echo "To resolve:"
      echo "  1. Run 'xchecker status my-spec' to see current progress"
      echo "  2. Complete required phases: 'xchecker spec my-spec --phase <phase>'"
      echo "  3. Address any pending fixups"
      exit 1
    fi
```

### Converting Smoke to Strict

To convert from the smoke pattern to strict enforcement:

1. Remove the `set +e` that suppresses errors
2. Remove the `exit 0` at the end
3. Add explicit `exit 1` when `passed=false`
4. Configure the job as a required status check in GitHub settings

**GitHub Repository Settings**:
1. Go to Settings → Branches
2. Add/edit branch protection rule for `main`
3. Enable "Require status checks to pass before merging"
4. Add "Gate Check" as a required status check

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Gate passed - all policy conditions met |
| 1 | Gate failed - one or more policy violations |
| 2+ | Runtime error (config, I/O, etc.) |

### Policy Parameters

```bash
xchecker gate <spec-id> [OPTIONS]

Options:
  --policy <path>            Load gate policy from a TOML file
                             Defaults to .xchecker/policy.toml or ~/.config/xchecker/policy.toml

  --min-phase <phase>         Require at least this phase completed
                              Values: requirements, design, tasks, review, fixup, final

  --fail-on-pending-fixups    Fail if any pending fixups exist

  --max-phase-age <duration>  Fail if latest success is older than threshold
                              Format: 7d (days), 24h (hours), 30m (minutes)

  --json                      Output structured JSON for CI parsing
```

### Example: Tiered Gate Policies

Different policies for different environments:

```yaml
jobs:
  gate-development:
    # Lenient policy for feature branches
    runs-on: ubuntu-latest
    steps:
      - run: xchecker gate $SPEC --min-phase requirements --json

  gate-staging:
    # Moderate policy for staging
    runs-on: ubuntu-latest
    steps:
      - run: xchecker gate $SPEC --min-phase design --max-phase-age 7d --json

  gate-production:
    # Strict policy for production
    runs-on: ubuntu-latest
    steps:
      - run: xchecker gate $SPEC --min-phase tasks --fail-on-pending-fixups --max-phase-age 24h --json
```

---

## See Also

- [TEST_MATRIX.md](TEST_MATRIX.md) - Detailed test inventory and statistics
- [claude-stub.md](claude-stub.md) - Test harness documentation
- [CONFIGURATION.md](CONFIGURATION.md) - Runtime configuration options
- [INDEX.md](INDEX.md) - Documentation index
- `.github/workflows/ci.yml` - Current CI configuration
- `.github/workflows/xchecker-gate.yml` - Gate workflow example

---

## Changelog

**2025-12-06** - CI Jobs Reference and stub stance
- Added CI Jobs Reference table documenting all workflow jobs
- Documented stub-tests as non-blocking on PRs with path to required
- Updated test.yml to run stub-tests on PRs for visibility

**2025-12-02** - Initial comprehensive CI profiles documentation
- Documented Local-Green profile (existing)
- Added Doc Validation profile
- Added Stub Suite profile specification
- Added Firehose profile with detailed warnings and cost analysis
- Added GitHub Actions specifications for manual/nightly triggers
- Added comparison matrix and best practices
- Added troubleshooting guide

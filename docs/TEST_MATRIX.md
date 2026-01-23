# Test Matrix: Claude Usage and Local-Green Readiness

**Generated:** 2025-12-03
**Agent:** 0.4 - V14.1 Test Infrastructure Update
**Purpose:** Ground-truth map of tests vs. external Claude usage

## Quick Reference

For day-to-day development, use the `justfile` commands:

```bash
just test-fast    # Quick feedback (~30s) - lib + bins only
just test-full    # Complete suite (~3-5min) - all tests
just test-local   # Local-green profile - no external deps
just test-stub    # Integration tests with claude-stub
just test-pbt     # Property-based tests only
```

See [TESTING.md](TESTING.md) for detailed usage documentation.

---

## Heavy Tests

Some tests are resource-intensive. Here's what to expect:

| Test Category | Duration | Memory | Notes |
|---------------|----------|--------|-------|
| Property-based tests | ~60s | Low | Runs 64+ iterations per property |
| Doctor/WSL integration | ~30s | Low | System probing, may spawn processes |
| Large file handling | ~20s | High | Tests with 10MB+ files |
| Concurrent lockfile | ~15s | Medium | Spawns multiple threads |
| Schema validation | ~10s | Low | JSON schema parsing |

### Skipping Heavy Tests

```bash
# Skip property tests for quick iteration
cargo test --lib --bins

# Run only fast unit tests
cargo test --lib -- --skip large --skip concurrent
```

---

## Windows-Specific Testing

### Running on Windows

```bash
# Standard test run (works on all platforms)
cargo test --all-features

# Windows-specific tests
cargo test -- windows

# WSL probe test (Windows only, ignored by default)
cargo test test_wsl_probe -- --ignored --nocapture
```

### Skipping Windows Tests on Other Platforms

```bash
cargo test -- --skip windows_ci_only
```

### Known Windows Differences

- Path separators: Tests use `camino::Utf8Path` for cross-platform paths
- Process termination: Uses Job Objects instead of process groups
- Line endings: Tests normalize CRLF to LF

---

## LLM Provider Test Gating

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `XCHECKER_SKIP_LLM_TESTS` | unset | Set to `1` to skip all real LLM tests |
| `XCHECKER_REAL_LLM_TESTS` | unset | Set to `1` to enable real LLM tests |
| `XCHECKER_ENABLE_REAL_CLAUDE` | unset | Set to `1` to enable real Claude API tests |
| `ANTHROPIC_API_KEY` | unset | Required for real Anthropic tests |
| `OPENROUTER_API_KEY` | unset | Required for real OpenRouter tests |
| `GEMINI_API_KEY` | unset | Required for real Gemini tests |
| `XCHECKER_OPENROUTER_BUDGET` | 20 | Max OpenRouter calls per process |

### Running Real LLM Tests

```bash
# Enable real Claude tests (requires API key)
XCHECKER_ENABLE_REAL_CLAUDE=1 ANTHROPIC_API_KEY=sk-... cargo test --test smoke -- --ignored

# Enable all real LLM tests
XCHECKER_REAL_LLM_TESTS=1 cargo test -- --ignored
```

**Warning:** Real LLM tests incur API costs. Budget controls are enforced.

---

## Property-Based Test Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PROPTEST_CASES` | 64 | Number of test cases per property |
| `PROPTEST_MAX_SHRINK_ITERS` | 1000 | Max shrinking iterations on failure |
| `PROPTEST_DISABLE_FAILURE_PERSISTENCE` | unset | Set to `1` to disable `.proptest-regressions` files |

### Slow Test Caps

Some property tests have maximum case count caps to ensure reasonable CI times:

| Test Category | Max Cases | Reason |
|---------------|-----------|--------|
| Doctor CLI tests | 5 | Spawn external processes |
| Doctor HTTP tests | 20 | Environment setup overhead |
| Standard tests | No cap | Fast in-memory operations |

When `PROPTEST_CASES` exceeds a test's cap, the cap is used instead.

### Increasing Test Counts for Local Runs

```bash
# Thorough testing (256 cases)
just test-pbt-thorough

# Custom case count
PROPTEST_CASES=1000 cargo test --test property_based_tests

# Maximum thoroughness (slow!)
PROPTEST_CASES=10000 cargo test --test property_based_tests -- --test-threads=1
```

### Regression Files

Property test failures are recorded in `.proptest-regressions` files:
- `tests/property_based_tests.proptest-regressions`
- `tests/test_doctor_http_provider_checks.proptest-regressions`

These files are committed to ensure regressions are caught in CI.

---

## Executive Summary

This document provides a comprehensive analysis of all test files in `tests/` to determine which tests require external Claude API calls and which are safe for local-green CI profiles (no network needed).

### Standardized Ignore Reasons

Tests use standardized `#[ignore = "reason"]` strings for consistency:

| Reason | Description | Skip Command |
|--------|-------------|--------------|
| `requires_claude_stub` | Needs claude-stub binary for mocking | `--skip requires_claude_stub` |
| `requires_real_claude` | Requires real Claude CLI and API | `--skip requires_real_claude` |
| `requires_xchecker_binary` | Needs compiled xchecker binary | `--skip requires_xchecker_binary` |
| `requires_future_phase` | Blocked on unimplemented phase (Review, Final) | `--skip requires_future_phase` |
| `requires_future_api` | Blocked on API not yet wired | `--skip requires_future_api` |
| `requires_refactoring` | Test needs code refactoring | `--skip requires_refactoring` |
| `windows_ci_only` | Windows-specific CI test | `--skip windows_ci_only` |

**Local-Green Profile Commands:**
```bash
# Library tests (always run)
cargo test --lib

# Doc validation (always run - critical for documentation accuracy)
cargo test --doc
cargo test schema_examples_tests

# Integration tests (local-green)
cargo test --tests -- --skip requires_claude_stub --skip requires_real_claude --skip requires_xchecker_binary --skip requires_future_phase --skip requires_future_api --skip requires_refactoring --skip windows_ci_only
```

### Test Profiles

#### Profile 1: Local-Green (No External Dependencies)

**Purpose**: Fast, reliable CI profile with no network, no stub, no binary requirements.

**Commands**:
```bash
# Library tests (always run)
cargo test --lib

# Doc validation (always run - part of local-green baseline)
cargo test --doc
cargo test schema_examples_tests

# Integration tests (local-green)
cargo test --tests -- \
  --skip requires_claude_stub \
  --skip requires_real_claude \
  --skip requires_xchecker_binary \
  --skip requires_future_phase \
  --skip requires_future_api \
  --skip requires_refactoring \
  --skip windows_ci_only
```

**What Runs**:
- Pure unit tests (595 library tests)
- Doctests (documentation examples)
- Schema validation tests (JSON schema conformance)
- Dry-run integration tests
- In-memory tests

**What Skips**: All tests requiring external binaries, stub, or real Claude API

**Duration**: ~35 seconds total (30s tests + 5s doc validation)

#### Profile 2: Stub Suite (With Claude Stub)

**Purpose**: Integration testing with full LLM mocking via claude-stub.

**Command**:
```bash
# Build claude-stub first
cargo build --bin claude-stub

# Run all tests except real Claude
cargo test --tests -- \
  --include-ignored \
  --skip requires_real_claude \
  --skip requires_xchecker_binary \
  --skip requires_future_phase \
  --skip requires_future_api \
  --skip requires_refactoring \
  --skip windows_ci_only
```

**What Runs**: Integration tests with mocked Claude responses
**What Skips**: Real Claude API tests, future/unimplemented features

#### Profile 3: Full Firehose (Everything)

**Purpose**: Complete test coverage including real Claude API calls (for nightly/on-demand runs).

**Prerequisites**:
- Claude API key set in environment
- `claude` CLI binary available
- Set `XCHECKER_ENABLE_REAL_CLAUDE=1`

**Command**:
```bash
XCHECKER_ENABLE_REAL_CLAUDE=1 cargo test --tests -- --include-ignored
```

**What Runs**: Everything, including real Claude API integration
**Warning**: Slow, requires network, incurs API costs

### Statistics

**Generated:** 2025-12-02
**Agent:** 0.3 - Test Inventory Snapshot

#### Test Count Summary

- **Total test files:** 97
- **Total test functions:** 853 (`#[test]` + `#[tokio::test]`)
- **Ignored tests:** 62
- **Local-green tests (no ignore):** 791 (92.7%)

#### Breakdown by Ignore Reason

| Ignore Reason | Count | Percentage of Ignored |
|---------------|-------|----------------------|
| `requires_claude_stub` | 49 | 79.0% |
| `requires_real_claude` | 4 | 6.5% |
| `requires_xchecker_binary` | 2 | 3.2% |
| `requires_refactoring` | 2 | 3.2% |
| `requires_future_phase` | 2 | 3.2% |
| `requires_future_api` | 2 | 3.2% |
| `windows_ci_only` | 1 | 1.6% |
| **Total Ignored** | **62** | **100%** |

#### Test Suites with Ignore Attributes

**`requires_claude_stub` (49 tests across 11 files):**
- `comprehensive_test_suite.rs` (1/3 tests)
- `golden_pipeline_tests.rs` (7/8 tests)
- `integration_full_workflows.rs` (6/6 tests - fully ignored)
- `integration_m1_gate.rs` (7/7 tests - fully ignored)
- `m1_gate_simple.rs` (3/3 tests - fully ignored)
- `m1_gate_simple_validation.rs` (5/5 tests - fully ignored)
- `m1_gate_validation.rs` (8/8 tests - fully ignored)
- `test_end_to_end_workflows.rs` (6/13 tests)
- `test_m4_gate_validation.rs` (1/14 tests)
- `test_phase_timeout.rs` (1/9 tests)
- `test_phase_timeout_scenarios.rs` (4/17 tests)

**`requires_real_claude` (4 tests across 2 files):**
- `smoke.rs` (2 tests)
- `test_exit_alignment.rs` (2 tests)

**`requires_xchecker_binary` (2 tests across 1 file):**
- `test_exit_alignment.rs` (2 tests)

**`requires_refactoring` (2 tests across 2 files):**
- `m1_gate_unit_tests.rs` (1 test)
- `property_based_tests.rs` (1 test)

**`requires_future_phase` (2 tests across 1 file):**
- `test_end_to_end_workflows.rs` (2 tests)

**`requires_future_api` (2 tests across 2 files):**
- `test_end_to_end_workflows.rs` (1 test)
- `test_packet_phase_integration.rs` (1 test)

**`windows_ci_only` (1 test across 1 file):**
- `test_wsl_probe.rs` (1 test)

### Bucket Distribution

| Bucket | Count | Description |
|--------|-------|-------------|
| **local-green** | 791 | Tests safe for local CI (no network, no external dependencies) |
| **requires-claude-stub** | 49 | Tests requiring claude-stub binary for mocking |
| **requires-real-Claude** | 4 | Tests requiring real Claude API access |
| **requires-binary** | 2 | Tests requiring compiled xchecker binary |
| **requires-implementation** | 6 | Tests blocked on future phases/APIs or refactoring |
| **platform-specific** | 1 | Windows-only CI tests |

---

## Requirements Traceability: FR-VLD (Validation)

This section maps FR-VLD (validation requirements) to specific test coverage.

### FR-VLD-001: Strict Validation Mode

**Requirement**: Support `strict_validation` config flag that treats low-quality LLM output as hard errors.

| Test Location | Test Name | Coverage |
|--------------|-----------|----------|
| `src/phases.rs` (unit) | `test_requirements_postprocess_strict_mode_rejects_meta_summary` | Strict mode rejects meta-summaries |
| `src/phases.rs` (unit) | `test_design_postprocess_strict_mode_rejects_meta_summary` | Strict mode in Design phase |
| `src/phases.rs` (unit) | `test_tasks_postprocess_strict_mode_rejects_meta_summary` | Strict mode in Tasks phase |
| `src/phases.rs` (unit) | `test_requirements_postprocess_soft_mode_allows_invalid_output` | Soft mode logs warnings only |
| `src/exit_codes.rs` (unit) | `test_validation_failed_mapping` | ValidationFailed → exit code 1 |

### FR-VLD-002: Output Validation

**Requirement**: Validate LLM output quality before accepting artifacts.

| Test Location | Test Name | Coverage |
|--------------|-----------|----------|
| `src/validation.rs` (unit) | `test_validate_requirements_detects_meta_summary` | Detects "Since no specific..." |
| `src/validation.rs` (unit) | `test_validate_requirements_accepts_valid_output` | Accepts properly structured output |
| `src/validation.rs` (unit) | `test_validate_design_structure` | Validates design document structure |
| `src/validation.rs` (unit) | `test_validate_tasks_structure` | Validates tasks document structure |

### FR-VLD-003: Configuration Propagation

**Requirement**: `strict_validation` flows from config → CLI → phases.

| Test Location | Test Name | Coverage |
|--------------|-----------|----------|
| `src/config.rs` (unit) | `test_config_strict_validation_default` | Default is false |
| `src/config.rs` (unit) | `test_config_strict_validation_from_toml` | Parses from config file |
| `tests/test_cli_flags.rs` | `test_strict_validation_cli_flag` | `--strict-validation` flag |
| `tests/test_cli_flags.rs` | `test_no_strict_validation_cli_flag` | `--no-strict-validation` flag |

---

## Detailed Test Inventory

### Category 1: LOCAL-GREEN (Safe for CI)

These tests either:
- Use `dry_run: true` (simulated Claude responses)
- Don't call orchestrator at all (pure unit tests)
- Already use claude-stub consistently

#### 1.1 Tests with `dry_run: true`

| Test File | Orchestrator Usage | Safe? | Notes |
|-----------|-------------------|-------|-------|
| `integration_full_workflows.rs` | ✓ | ✅ Yes | Uses `dry_run: true` in determinism test (line 341) |
| `test_workflow_receipt_regression.rs` | ✓ | ✅ Yes | Uses `dry_run: true` consistently |
| `test_resume_functionality.rs` | ✓ | ✅ Yes | Uses `dry_run: true` for resume logic tests |
| `test_phase_transition_validation.rs` | ✓ | ✅ Yes | Uses `dry_run: true` for transition validation |
| `test_phase_orchestration_integration.rs` | ✓ | ✅ Yes | Uses `dry_run: true` for orchestration tests |
| `test_packet_phase_integration.rs` | ✓ | ✅ Yes | Uses `dry_run: true` for packet integration |
| `test_fixup_command_integration.rs` | ✓ | ✅ Yes | Uses `dry_run: true` |
| `test_fixup_cli_integration.rs` | ✓ | ✅ Yes | Uses `dry_run: true` |
| `test_debug_packet_integration.rs` | ✓ | ✅ Yes | Uses `dry_run: true` for packet debugging |

#### 1.2 Pure Unit Tests (No Orchestrator)

These tests don't use `PhaseOrchestrator` or make Claude calls at all:

| Test File | Purpose | Safe? |
|-----------|---------|-------|
| `smoke.rs` | CLI smoke tests (no orchestrator) | ✅ Yes |
| `test_packet_builder.rs` | PacketBuilder unit tests | ✅ Yes |
| `test_schema_compliance.rs` | Schema validation tests | ✅ Yes |
| `m2_gate_canonicalization.rs` | Canonicalization tests | ✅ Yes |
| `test_config_system.rs` | Config discovery tests | ✅ Yes |
| `test_wsl_runner.rs` | WSL runner unit tests | ✅ Yes |
| `test_wsl_probe.rs` | WSL detection tests | ✅ Yes |
| `test_secret_redaction_comprehensive.rs` | Secret redaction tests | ✅ Yes |
| `test_redaction_coverage.rs` | Redaction coverage tests | ✅ Yes |
| `test_redaction_security.rs` | Redaction security tests | ✅ Yes |
| `test_source_resolver.rs` | Source resolver tests | ✅ Yes |
| `test_runner_execution.rs` | Runner execution tests | ✅ Yes |
| `test_runner_buffering.rs` | Runner buffering tests | ✅ Yes |
| `test_lockfile_integration.rs` | Lockfile tests | ✅ Yes |
| `test_lockfile_concurrent_execution.rs` | Concurrent lockfile tests | ✅ Yes |
| `test_json_schema_validation.rs` | JSON schema validation | ✅ Yes |
| `test_line_ending_normalization.rs` | Line ending tests | ✅ Yes |
| `test_cross_platform_line_endings.rs` | Cross-platform line endings | ✅ Yes |
| `test_large_file_handling.rs` | Large file tests | ✅ Yes |
| `test_packet_overflow_scenarios.rs` | Packet overflow tests | ✅ Yes |
| `test_packet_performance.rs` | Packet performance tests | ✅ Yes |
| `test_fixup_apply_mode.rs` | Fixup apply mode tests | ✅ Yes |
| `test_fixup_preview_mode.rs` | Fixup preview mode tests | ✅ Yes |
| `test_fixup_cross_filesystem.rs` | Cross-filesystem fixup tests | ✅ Yes |
| `test_cli_flags.rs` | CLI flag parsing tests | ✅ Yes |
| `test_doctor_wsl_checks.rs` | Doctor WSL check tests | ✅ Yes |
| `test_doctor_llm_checks.rs` | Doctor LLM provider checks (12 tests) | ✅ Yes |
| `test_doc_validation_presence.rs` | Doc validation guard tests (3 tests) | ✅ Yes |
| `test_engine_invariants.rs` | Engine invariant tests (14 tests - B3.7-B3.15) | ✅ Yes |
| `test_error_receipt_metadata.rs` | Error receipt metadata tests (9 tests) | ✅ Yes |
| `test_generated_schema_validation.rs` | Generated schema validation | ✅ Yes |
| `test_jcs_performance.rs` | JCS performance tests | ✅ Yes |
| `test_error_handling_comprehensive.rs` | Error handling tests | ✅ Yes |
| `test_llm_provider_selection.rs` | LLM provider validation tests (16 tests) | ✅ Yes |
| `test_error_messages_improvement.rs` | Error message tests | ✅ Yes |
| `test_phase_trait_system.rs` | Phase trait tests | ✅ Yes |
| `test_structured_logging.rs` | Structured logging tests | ✅ Yes |
| `test_status_reporting.rs` | Status reporting tests | ✅ Yes |
| `test_v1_1_jcs_emission.rs` | JCS emission tests | ✅ Yes |
| `test_v1_2_blake3_hashing.rs` | BLAKE3 hashing tests | ✅ Yes |
| `test_receipt_schema_v1.rs` | Receipt schema tests | ✅ Yes |
| `test_llm_receipt_metadata.rs` | LLM receipt metadata tests | ✅ Yes |
| `test_error_receipt_generation.rs` | Error receipt tests | ✅ Yes |
| `test_secret_redaction_error_paths.rs` | Secret redaction error paths | ✅ Yes |
| `test_unix_process_termination.rs` | Unix process termination | ✅ Yes |
| `test_windows_job_objects.rs` | Windows job objects | ✅ Yes |
| `comprehensive_test_suite.rs` | Comprehensive suite | ✅ Yes |
| `property_based_tests.rs` | Property-based tests | ✅ Yes |
| `m1_gate_unit_tests.rs` | M1 gate unit tests | ✅ Yes |
| `m1_gate_final_validation.rs` | M1 gate final validation | ✅ Yes |
| `m3_gate_simple.rs` | M3 gate simple tests | ✅ Yes |
| `m3_gate_contracts_validation.rs` | M3 gate contract tests | ✅ Yes |
| `m4_gate_simple.rs` | M4 gate simple tests | ✅ Yes |
| `debug_canonicalization.rs` | Debug canonicalization tests | ✅ Yes |
| `test_doc_validation.rs` | Doc validation tests | ✅ Yes |
| `test_cache_integration.rs` | Cache integration tests | ✅ Yes |

#### 1.3 Doc Validation Tests (Always-Run - Critical for Documentation Accuracy)

**Doc validation tests are ALWAYS-RUN tests** that must remain green as part of the local-green baseline.

**Commands**:
```bash
cargo test --doc                    # Doctests in source code
cargo test schema_examples_tests    # Schema validation tests
```

**Why Always-Run**: These tests validate:
- Documentation examples compile and execute correctly
- JSON schema examples conform to their schemas
- Public API documentation matches implementation
- Code examples in docs are accurate and current

**Duration**: ~5 seconds (fast feedback)

All tests in `tests/doc_validation/` are pure validation tests:

| Test File | Purpose | Safe? | Category |
|-----------|---------|-------|----------|
| `schema_examples_tests.rs` | **Schema example validation** | ✅ Yes | **Always-Run** |
| `changelog_tests.rs` | Changelog validation | ✅ Yes | Local-Green |
| `code_examples_tests.rs` | Code example validation | ✅ Yes | Local-Green |
| `common.rs` | Common test utilities | ✅ Yes | Local-Green |
| `config_tests.rs` | Config doc validation | ✅ Yes | Local-Green |
| `contracts_tests.rs` | Contract doc validation | ✅ Yes | Local-Green |
| `doctor_tests.rs` | Doctor doc validation | ✅ Yes | Local-Green |
| `enum_introspection_tests.rs` | Enum introspection | ✅ Yes | Local-Green |
| `feature_tests.rs` | Feature doc validation | ✅ Yes | Local-Green |
| `m2_gate_tests.rs` | M2 gate doc tests | ✅ Yes | Local-Green |
| `m3_gate_tests.rs` | M3 gate doc tests | ✅ Yes | Local-Green |
| `m4_gate_tests.rs` | M4 gate doc tests | ✅ Yes | Local-Green |
| `m8_gate_tests.rs` | M8 gate doc tests | ✅ Yes | Local-Green |
| `m9_gate_tests.rs` | M9 gate doc tests | ✅ Yes | Local-Green |
| `readme_tests.rs` | README validation | ✅ Yes | Local-Green |
| `schema_rust_conformance_tests.rs` | Schema conformance | ✅ Yes | Local-Green |
| `xchecker_home_tests.rs` | XCHECKER_HOME tests | ✅ Yes | Local-Green |

**Note**: `schema_examples_tests.rs` is explicitly called out as **always-run** because it validates the core JSON schema examples that external tools depend on.

---

### Category 2: NEEDS-STUB (Uses Claude, Should Use Stub)

These tests use `dry_run: false` and attempt to call real Claude CLI. They should be converted to use claude-stub consistently.

| Test File | Current Status | Claude Usage | Recommendation |
|-----------|---------------|--------------|----------------|
| `integration_m1_gate.rs` | Uses claude-stub | `dry_run: false` with `claude_cli_path: "cargo run --bin claude-stub --"` | ✅ Already uses stub |
| `m1_gate_validation.rs` | Uses claude-stub | `dry_run: false` with `claude_cli_path: "cargo run --bin claude-stub --"` | ✅ Already uses stub |
| `m1_gate_simple.rs` | Uses claude-stub | `dry_run: false` with `claude_cli_path: "cargo run --bin claude-stub --"` | ✅ Already uses stub |
| `m1_gate_simple_validation.rs` | Uses claude-stub | `dry_run: false` with `claude_cli_path: "cargo run --bin claude-stub --"` | ✅ Already uses stub |
| `golden_pipeline_tests.rs` | Uses claude-stub | `dry_run: false` with `claude_cli_path: "cargo run --bin claude-stub --"` | ✅ Already uses stub |
| `m3_gate_validation.rs` | Uses claude-stub | `dry_run: false` with comment "Use simulated Claude responses" | ✅ Already uses stub |
| `m4_gate_validation.rs` | Uses claude-stub | `dry_run: false` with `claude_cli_path: "cargo run --bin claude-stub --"` | ✅ Already uses stub |
| `m5_gate_validation.rs` | Uses orchestrator | `dry_run: false` patterns found | ⚠️ Verify stub usage |
| `m6_gate_validation.rs` | Uses orchestrator | May use Claude | ⚠️ Verify stub usage |
| `test_apply_fixups_flag.rs` | Uses orchestrator | May use Claude | ⚠️ Verify stub usage |
| `test_phase_timeout.rs` | Uses orchestrator | May use Claude | ⚠️ Verify stub usage |
| `test_phase_timeout_scenarios.rs` | Uses orchestrator | May use Claude | ⚠️ Verify stub usage |
| `test_m4_gate_validation.rs` | Uses orchestrator | May use Claude | ⚠️ Verify stub usage |
| `test_end_to_end_workflows.rs` | Uses orchestrator | May use Claude | ⚠️ Verify stub usage |

**Analysis:**
- Most M1/M3/M4 gate tests **already use claude-stub** properly
- A few tests need verification to ensure they're using stub or dry_run
- None should require real Claude CLI for CI

---

### Category 3: REQUIRES-REAL-CLAUDE (Must Hit Real API)

**Count: 0 tests**

No tests were identified that require real Claude API access. All tests either:
- Use `dry_run: true` for simulation
- Use `claude-stub` for deterministic responses
- Don't call Claude at all

---

## Orchestrator API Analysis

### Key Functions and Their Behavior

Based on analysis of `src/orchestrator/`:

#### 1. `OrchestratorConfig`
```rust
pub struct OrchestratorConfig {
    pub dry_run: bool,  // Controls Claude execution
    pub config: HashMap<String, String>,  // Additional config
}
```

- **`dry_run: true`**: Uses `simulate_claude_response()` - no network calls
- **`dry_run: false`**: Uses `execute_claude_cli()` - requires Claude CLI

#### 2. Claude Integration Path

When `dry_run: false`:
1. Checks for `claude_cli_path` in config
2. If path contains `"claude-stub"`, calls `execute_claude_stub()`
3. Otherwise, calls real `ClaudeWrapper::execute()`

**Key Code (src/orchestrator/mod.rs:346-350):**
```rust
if let Some(claude_cli_path) = config.config.get("claude_cli_path")
    && claude_cli_path.contains("claude-stub")
{
    return self.execute_claude_stub(prompt, config).await;
}
```

#### 3. Phase Execution Functions

All phase execution functions follow this pattern:
- `execute_requirements_phase(config)`
- `execute_design_phase(config)`
- `execute_tasks_phase(config)`
- `execute_review_phase(config)`
- `execute_fixup_phase(config)`

Each respects the `dry_run` flag in config.

---

## Recommendations

### For CI/CD Pipelines

#### Local-Green Profile (Default CI)
```yaml
# Run all tests that don't require network
cargo test --lib
cargo test --test '*' \
  --exclude integration_m1_gate \
  --exclude m1_gate_validation \
  --exclude golden_pipeline_tests
```

**Safe to run:**
- All unit tests (74 files)
- All doc validation tests (17 files)
- Tests with `dry_run: true` (9 files)

#### Integration Test Profile (Optional)
```yaml
# Run tests with claude-stub (requires building the stub)
cargo build --bin claude-stub
cargo test integration_m1_gate
cargo test m1_gate_validation
cargo test golden_pipeline_tests
```

**Requires:**
- Building `claude-stub` binary
- No real Claude CLI needed

#### E2E Profile (Disabled by Default)
```yaml
# Only run if XCHECKER_E2E is set or Claude CLI is available
if [ -n "$XCHECKER_E2E" ] || which claude; then
  cargo test --test integration_full_workflows -- --include-ignored
fi
```

**Requires:**
- Real Claude CLI installed
- API credentials configured
- Should be opt-in only

### For Test File Updates

#### Tests to Convert to Stub

The following tests may need explicit stub configuration:

1. **`m5_gate_validation.rs`** - Verify uses stub or dry_run
2. **`m6_gate_validation.rs`** - Verify uses stub or dry_run
3. **`test_apply_fixups_flag.rs`** - Consider using dry_run
4. **`test_phase_timeout.rs`** - Consider using dry_run
5. **`test_phase_timeout_scenarios.rs`** - Consider using dry_run
6. **`test_m4_gate_validation.rs`** - Verify uses stub
7. **`test_end_to_end_workflows.rs`** - Verify uses stub

#### Recommended Pattern

For tests that need Claude-like behavior:

**Option A: Use dry_run (simplest)**
```rust
let config = OrchestratorConfig {
    dry_run: true,  // Simulated responses
    config: HashMap::new(),
};
```

**Option B: Use claude-stub (more realistic)**
```rust
let config = OrchestratorConfig {
    dry_run: false,
    config: {
        let mut map = HashMap::new();
        map.insert(
            "claude_cli_path".to_string(),
            "cargo run --bin claude-stub --".to_string(),
        );
        map.insert("claude_scenario".to_string(), "success".to_string());
        map
    },
};
```

---

## Test Coverage by Category

### By Orchestrator Usage

| Category | Count | Percentage |
|----------|-------|------------|
| Uses orchestrator | 20 | 21% |
| Pure unit tests | 74 | 79% |

### By Claude Dependency

| Category | Count | Percentage | CI Safe? |
|----------|-------|------------|----------|
| No Claude calls | 74 | 79% | ✅ Yes |
| Uses dry_run | 9 | 10% | ✅ Yes |
| Uses claude-stub | 9 | 10% | ✅ Yes (needs stub build) |
| May use real Claude | 2 | 2% | ⚠️ Needs verification |

### By Test Purpose

| Purpose | Count | Example Files |
|---------|-------|---------------|
| Unit tests | 74 | `test_packet_builder.rs`, `test_schema_compliance.rs` |
| Integration tests (stub) | 9 | `integration_m1_gate.rs`, `golden_pipeline_tests.rs` |
| Integration tests (dry) | 9 | `test_phase_orchestration_integration.rs` |
| Doc validation | 17 | All in `doc_validation/` |
| Gate validation | 11 | `m1_gate_*.rs`, `m3_gate_*.rs`, `m4_gate_*.rs` |

---

## Verification Checklist

For each test marked "⚠️ Verify stub usage":

- [ ] `m5_gate_validation.rs` - Check config, ensure stub or dry_run
- [ ] `m6_gate_validation.rs` - Check config, ensure stub or dry_run
- [ ] `test_apply_fixups_flag.rs` - Check config, consider dry_run
- [ ] `test_phase_timeout.rs` - Check config, consider dry_run
- [ ] `test_phase_timeout_scenarios.rs` - Check config, consider dry_run
- [ ] `test_m4_gate_validation.rs` - Check config, ensure uses stub
- [ ] `test_end_to_end_workflows.rs` - Check config, ensure uses stub

---

## Appendix: File Locations

### Test Files by Directory

```
tests/
├── *.rs (29 main test files)
├── doc_validation/
│   ├── *.rs (17 doc validation tests)
│   └── mod.rs
└── (65 other test files)

Total: 94 test files
```

### Key Source Files Analyzed

- `src/orchestrator/mod.rs` - Main orchestrator with dry_run logic
- `src/orchestrator/llm.rs` - LLM integration helpers
- `src/orchestrator/workflow.rs` - Workflow execution
- `src/orchestrator/phase_exec.rs` - Phase execution engine

---

## Conclusion

**Overall Assessment: 88% Local-Green Ready**

- **83 out of 94 tests** (88%) are already safe for local CI
- **9 tests** use claude-stub (safe if stub is built)
- **2 tests** need verification
- **0 tests** require real Claude API

**Action Items:**
1. Verify the 2 tests marked with ⚠️
2. Ensure CI builds `claude-stub` for integration tests
3. Document the local-green vs integration-test split in CI config
4. Consider adding `#[cfg(feature = "integration-tests")]` markers

**Local-Green Profile is Viable:** The vast majority of tests can run without any external dependencies, making this project well-suited for fast, reliable CI/CD pipelines.

## See Also


- [claude-stub.md](claude-stub.md) - Test harness documentation
- [INDEX.md](INDEX.md) - Documentation index

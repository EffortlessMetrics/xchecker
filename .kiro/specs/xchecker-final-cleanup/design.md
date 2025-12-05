# Design Document

## Overview

This design covers the final cleanup work to bring xchecker from "functionally complete" to "release-ready." The work is primarily:

1. **Test fixes** - Fix failing/flaky tests by aligning expectations with actual behavior
2. **Dead code resolution** - Either wire unused code or annotate it with clear intent
3. **Cross-platform verification** - Ensure full test suite passes on Linux/macOS/Windows

This is a cleanup spec, not new features. The goal is "green, boring, and auditable."

## Architecture

No architectural changes. This spec modifies existing code to:
- Fix test patterns (avoid `set_current_dir`, use explicit workspace paths)
- Add annotations (`#[cfg_attr(not(test), allow(dead_code))]`)
- Update documentation (doc comments, PLATFORM.md, CONFIGURATION.md)
- Remove unused code where appropriate

## Components and Interfaces

### Test Helper Pattern

Replace the problematic CWD-changing pattern:

```rust
// BEFORE (problematic on Windows)
let dir = tempfile::tempdir().unwrap();
env::set_current_dir(&dir).unwrap();
// ... test code assumes CWD is temp dir

// AFTER (explicit workspace)
fn with_temp_workspace<F: FnOnce(&Path)>(f: F) {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    
    // Create .xchecker/config.toml under root
    std::fs::create_dir_all(root.join(".xchecker")).unwrap();
    
    // Pass root explicitly via XCHECKER_HOME env var
    std::env::set_var("XCHECKER_HOME", root);
    
    // Pass root explicitly to test code
    f(root);
    
    // Clean up env var
    std::env::remove_var("XCHECKER_HOME");
    // tmp lives for scope of f
}
```

This pattern:
- Avoids Windows-specific "path not found" races and drive/current-dir weirdness
- Makes tests platform-agnostic (no assumptions about drive letters or UNC paths)
- Keeps `TempDir` alive for the scope of the test closure

### HTTP Client Isolation Pattern

To make "no HTTP calls" testable, factor `HttpClient` behind a trait:

```rust
// In src/http.rs or similar
pub trait HttpClientProvider {
    fn create_client(&self) -> Result<HttpClient, Error>;
}

// Production implementation
pub struct RealHttpClientProvider;
impl HttpClientProvider for RealHttpClientProvider {
    fn create_client(&self) -> Result<HttpClient, Error> {
        // Real reqwest client
    }
}

// Test implementation that panics if called
#[cfg(test)]
pub struct PanickingHttpClientProvider;
#[cfg(test)]
impl HttpClientProvider for PanickingHttpClientProvider {
    fn create_client(&self) -> Result<HttpClient, Error> {
        panic!("Doctor should not construct HTTP clients!");
    }
}
```

Alternative: Use `#[cfg(test)]` to inject a panicking HTTP client in doctor code paths, and assert tests pass (so production path doesn't reach it).

### Annotation Pattern

For code that is implemented but not wired:

```rust
// At module head - single high-signal comment
//! Hooks system: implemented and tested, not wired into orchestrator in v1.0.
//! See FR-HOOKS for design rationale. Will be integrated in a future release.

/// Reserved for hooks integration; not wired in v1.0
/// Test seam; not part of public API stability guarantees
#[cfg_attr(not(test), allow(dead_code))]
pub struct HookExecutor { ... }
```

Key discipline: when adding `#[allow(dead_code)]` anywhere, you MUST add an accompanying comment explaining intent. This prevents slowly re-fuzzing the signal.

### Performance Test Pattern

For timing comparisons, use tolerant assertions with optional multi-run median:

```rust
const TOLERANCE: f64 = 1.2;

// Option A: Single measurement with tolerance
let ratio = cache_hit_ms as f64 / cache_miss_ms as f64;
assert!(
    ratio <= TOLERANCE,
    "Cache hit should be at most {}x miss time, got {}x",
    TOLERANCE, ratio
);

// Option B: Multiple runs with median comparison (more robust)
fn median(times: &mut [u64]) -> u64 {
    times.sort();
    times[times.len() / 2]
}

let miss_times: Vec<u64> = (0..5).map(|_| measure_cache_miss()).collect();
let hit_times: Vec<u64> = (0..5).map(|_| measure_cache_hit()).collect();

let miss_median = median(&mut miss_times.clone());
let hit_median = median(&mut hit_times.clone());

assert!(
    hit_median as f64 <= miss_median as f64 * TOLERANCE,
    "Cache hit median should be at most {}x miss median",
    TOLERANCE
);
```

## Data Models

No new data models. Existing models may have unused fields removed (e.g., `PhaseCoreOutput`).

### PhaseCoreOutput Field Removal

Before removing fields from `PhaseCoreOutput`, verify no traceability violations:
- Check if FR-ORC or other specs require specific fields
- If fields are moved to Receipt/Status instead of removed, update traceability docs
- Fields to evaluate: `phase_id`, `prompt`, `claude_response`, `artifact_paths`, `output_hashes`, `atomic_write_warnings`

## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system-essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

Most requirements in this cleanup spec are example-based (code structure, documentation presence) rather than property-based. The key properties are:

### Property 1: Doctor performs only static checks

*For any* provider configuration (valid or invalid), doctor SHALL perform only static checks (env vars, config files, binary presence) and SHALL NOT construct HTTP clients or make network calls.

**Validates: Requirements 1.3**

Implementation approach:
- Factor `HttpClient::new` behind a trait
- In tests, inject `PanickingHttpClientProvider` 
- Assert tests pass (proving production path doesn't reach HTTP construction)
- Alternative: `#[cfg(test)]` a panicking HTTP client in doctor code

The existing `prop_doctor_no_http_calls_for_http_providers` property test needs these fixes:
- Assert no HTTP client construction (the invariant we care about)
- Allow Pass/Warn/Fail based on config validity (don't over-constrain)
- Drop the assumption that "valid API key + model ⇒ Pass"

### Property 2: Cache hit is not slower than miss (within tolerance)

*For any* cache operation, the cache hit time SHALL be ≤ cache miss time × TOLERANCE (where TOLERANCE = 1.2).

**Validates: Requirements 3.1**

This is a relaxed performance property that allows for system noise while still catching regressions. If CI still flakes with single measurements, use median of N runs.

## Error Handling

No new error types. Existing error handling remains unchanged.

## Testing Strategy

### Dual Testing Approach

This cleanup spec is primarily about fixing existing tests, not adding new ones.

**Property-Based Testing:**
- Use `proptest` (already configured in the project)
- Property 1 (doctor static checks) is already implemented; fix assertions
- Property 2 (cache performance) converts existing flaky test to tolerant property

**Unit/Example Testing:**
- Most requirements are verified by example tests or code inspection
- CI gates (clippy, fmt, test matrix) provide verification

### Test Modifications

| Test File | Change |
|-----------|--------|
| `test_doctor_http_provider_checks.rs` | Replace `set_current_dir` with `with_temp_workspace` helper; relax property assertions to only assert "no HTTP client construction" |
| `test_doctor_llm_checks.rs` | Change unsupported provider from "gemini-cli" to "totally-unknown-provider" |
| `test_cache_integration.rs` | Use TOLERANCE-based ratio comparison; optionally use median of N runs |
| `test_llm_provider_selection.rs` | Add positive assertions for all four providers; mark as canonical list in comments |

### Canonical Provider List

In `test_llm_provider_selection.rs`, add a comment marking the supported providers as canonical:

```rust
// Canonical list of supported LLM providers for xchecker v1.0
// Update this list and corresponding tests when adding new providers
const SUPPORTED_PROVIDERS: &[&str] = &[
    "claude-cli",
    "gemini-cli", 
    "openrouter",
    "anthropic",
];

#[test]
fn test_all_supported_providers_are_accepted() {
    for provider in SUPPORTED_PROVIDERS {
        let result = create_backend(provider);
        assert!(result.is_ok(), "Provider {} should be supported", provider);
    }
}
```

### Annotation Verification

CI will verify annotations via `RUSTFLAGS="-D warnings"` after cleanup:
- All `#[allow(dead_code)]` must have accompanying comment
- All test-only functions must have `#[cfg_attr(not(test), allow(dead_code))]`

### Cross-Platform Verification

| Platform | Verification |
|----------|--------------|
| Linux | `cargo test --all-targets --all-features` |
| macOS | `cargo test --all-targets --all-features` |
| Windows | `cargo test --all-targets --all-features` |
| Windows+WSL | WSL-specific tests pass when WSL available |

## Implementation Decisions (v1.0)

Per requirements, these decisions are locked for v1.0:

| Component | Decision | Annotation | Notes |
|-----------|----------|------------|-------|
| Hooks | Annotated as reserved | `// TODO: wire into orchestrator in future release` | Single high-signal comment at module head |
| `src/claude.rs` | Legacy/test-only | `#[cfg(any(test, feature = "legacy_claude"))]` | Follow-up spec (V19+) to delete once tests migrate to new backend |
| `llm/claude_cli.rs` | Production backend | Doc comment: "Production LLM backend" | Canonical path for Claude CLI integration |
| `Runner::auto` | Internal helper | `// Internal API; CLI supports native/wsl only` | Verify no CLI code path accidentally calls it |
| StatusManager | Wired into CLI | Single codepath for status command | CLI calls `StatusManager::generate_status_from_orchestrator`, not custom logic |
| OrchestratorHandle | Reserved for external API | `// Reserved for IDE/TUI integration` | Unused methods annotated, not removed |
| PhaseCoreOutput unused fields | Removed | N/A | Verify no traceability violations first |

## Detailed Implementation Notes

### Req 1: HTTP Doctor Tests

1. **Refactor `test_doctor_http_provider_checks.rs`:**
   - Replace any `env::set_current_dir` with explicit temp dirs + `XCHECKER_HOME`
   - Adjust `prop_doctor_no_http_calls_for_http_providers` to:
     - Assert that doctor doesn't construct/use `HttpClient` (via test-only shim)
     - Drop the assumption that "valid API key + model ⇒ Pass"
   - Verify it passes on Windows and doesn't panic with `NotFound` path errors

### Req 2: LLM Provider Tests

1. **Update `test_unsupported_provider_fails_config_validation`:**
   - Use "totally-unknown-provider" instead of "gemini-cli"
   
2. **Update `test_llm_provider_selection.rs`:**
   - Assert that `claude-cli`, `gemini-cli`, `openrouter`, `anthropic` all produce `Ok(BackendKind::…)`
   - Keep a single table of supported provider strings as the canonical list
   - Add comment marking this as the source of truth for supported providers

### Req 3: Cache & Perf Tests

1. **Rewrite `test_cache_performance_improvement`:**
   - Measure miss and hit times
   - Assert `hit <= miss * 1.2`
   - Optionally run each variant a few times and compare medians
   - If CI still flakes, add `#[ignore]` and wire into a separate perf lane

### Req 4: Hooks Module

1. **Add annotations to `hooks.rs`:**
   - Add `#[cfg_attr(not(test), allow(dead_code))]` at module or type level
   - Add clear top-level comment: "implemented, not wired into orchestrator in v1.0"
   
2. **Annotate `Config::hooks`:**
   - Add `// Reserved for hooks integration; not wired in v1.0`

### Req 5: Claude Wrapper

1. **Gate `src/claude.rs`:**
   - Add `#[cfg(any(test, feature = "legacy_claude"))]` (or move under `tests/`)
   - Add `cfg_attr(not(test), allow(dead_code))` to unused fields/methods
   
2. **Document production backend:**
   - Add doc comment in `llm/claude_cli.rs`: "Production LLM backend; src/claude.rs is legacy/test-only"
   
3. **Ensure test cfg consistency:**
   - Tests importing `ClaudeWrapper` must be under same cfg gate

### Req 6: Runner::auto

1. **Verify no CLI usage:**
   - Ensure no CLI code path uses `Runner::auto` (only internal/test usage)
   
2. **Update docs:**
   - `CONFIGURATION.md` and `--help`: only `native` and `wsl` are supported; `auto` reserved for future
   
3. **Add annotation:**
   - `// Internal API for future use; CLI only supports native/wsl` above `Runner::auto`

### Req 7: StatusManager

1. **Refactor CLI status command:**
   - Call `StatusManager::generate_status_from_orchestrator` / `generate_status_internal`
   - Remove any duplicate status-building code
   
2. **Annotate remaining unused helpers:**
   - Add `cfg_attr(not(test), allow(dead_code))` + "reserved for future orchestration API"

### Req 8: OrchestratorHandle & PhaseCoreOutput

1. **Remove unused `PhaseCoreOutput` fields:**
   - Evaluate: `phase_id`, `prompt`, `claude_response`, `artifact_paths`, `output_hashes`, `atomic_write_warnings`
   - Wire into `StatusOutput`/`Receipt` if needed, otherwise remove
   - Verify no FR-ORC traceability violations
   
2. **Annotate `OrchestratorHandle` methods:**
   - Add `cfg_attr(not(test), allow(dead_code))` 
   - Add doc comments: "reserved for IDE/TUI/external orchestration"

### Req 9: Test Seams

For `paths::with_isolated_home`, `ReceiptManager::receipts_path`, `Workspace::get_spec`:
- Add `cfg_attr(not(test), allow(dead_code))`
- Add `/// Test seam; not part of public API stability guarantees`

### Req 10: Test Hygiene

Run `cargo test` with `RUSTFLAGS="-D warnings"` and:
- Remove all unused imports
- Either use or underscore-prefix unused helpers
- Where keeping helpers for future tests, add `#[allow(dead_code)] // Reserved for future test cases`

### Req 11: Cross-Platform

Once above is done:
1. Run `RUSTFLAGS="-D warnings" cargo test --all-targets --all-features` on Linux, macOS, Windows
2. Ensure WSL-specific tests behave correctly
3. Document any unavoidable platform quirks in `PLATFORM.md`
4. Lock CI to `RUSTFLAGS="-D warnings"` after tree is clean

## Files to Modify

### Test Files
- `tests/test_doctor_http_provider_checks.rs` - Fix CWD pattern, relax property, add HTTP client isolation
- `tests/test_doctor_llm_checks.rs` - Update unsupported provider string
- `tests/test_cache_integration.rs` - Use tolerant timing comparison with optional median
- `tests/test_llm_provider_selection.rs` - Add positive provider assertions, mark as canonical list
- `tests/test_llm_budget_exhaustion_receipt.rs` - Remove unused imports
- `tests/property_based_tests.rs` - Remove unused imports
- `tests/doc_validation/changelog_tests.rs` - Remove unused functions or use them
- `tests/doc_validation/common.rs` - Annotate `StubRunner::home_path`
- `tests/test_windows_job_objects.rs` - Annotate `count_timeout_processes`

### Source Files
- `src/hooks.rs` - Add module-level comment + `#[cfg_attr(not(test), allow(dead_code))]`
- `src/config.rs` - Annotate `hooks` field
- `src/claude.rs` - Gate with `#[cfg(any(test, feature = "legacy_claude"))]`
- `src/llm/claude_cli.rs` - Add doc comment about production status
- `src/runner.rs` - Annotate `Runner::auto` as internal, verify no CLI usage
- `src/status.rs` - Ensure CLI uses StatusManager helpers (single codepath)
- `src/orchestrator/handle.rs` - Annotate reserved methods
- `src/orchestrator/phase_exec.rs` - Remove unused `PhaseCoreOutput` fields (after traceability check)
- `src/paths.rs` - Annotate `with_isolated_home`
- `src/receipt.rs` - Annotate `receipts_path` if test-only
- `src/workspace.rs` - Annotate `get_spec` if test-only
- `src/template.rs` - Remove unused `tempfile::TempDir` import
- `src/gate.rs` - Remove or use `POLICY_PASS` constant

### Documentation Files
- `docs/CONFIGURATION.md` - Document runner modes (native/wsl supported, auto internal)
- `docs/PLATFORM.md` - Document any platform-specific issues found

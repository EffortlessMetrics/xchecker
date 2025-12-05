# Requirements Document

## Introduction

This spec covers the final cleanup work to bring xchecker from "functionally complete" to "fully implemented and release-ready." The focus is on fixing remaining failing/flaky tests, aligning test expectations with actual implementation behavior, cleaning up dead code warnings, and ensuring cross-platform verification is complete.

This is explicitly a cleanup and release-readiness spec, not new runtime features.

## Glossary

- **xchecker**: The Rust CLI tool for orchestrating spec generation workflows
- **Property-Based Test**: Tests that verify properties hold across randomly generated inputs
- **Dead Code**: Code that exists but is never executed in production paths
- **CWD**: Current Working Directory - the process's current directory
- **WSL**: Windows Subsystem for Linux
- **JCS**: JSON Canonicalization Scheme (RFC 8785)
- **Test Seam**: A library function exposed primarily for testing purposes
- **Full Test Suite**: `cargo test --all-targets --all-features` with `#[ignore]` and `requires_*`-gated tests allowed to skip when documented in CI_PROFILES/PLATFORM

## Requirements

### Requirement 1

**User Story:** As a developer, I want the HTTP doctor tests to pass reliably, so that CI is green and I can trust the test suite.

#### Acceptance Criteria

1. WHEN tests in `test_doctor_http_provider_checks.rs` run THEN the system SHALL NOT use `env::set_current_dir` to change into temp directories
2. WHEN tests need an isolated workspace THEN the system SHALL create temp directories and pass paths explicitly via `XCHECKER_HOME` environment variable or `--workspace` CLI flag
3. WHEN the property test `prop_doctor_no_http_calls_for_http_providers` runs THEN the system SHALL assert that doctor performs only static checks (env/config/binary presence) and SHALL NOT require network access or construct HTTP client code paths
4. WHEN doctor checks a provider with valid API key and model THEN the system SHALL allow Pass, Warn, or Fail based on full config validity (not just key presence); deterministic Pass guarantees are deferred to a future spec
5. WHEN tests run on Windows THEN path handling SHALL be platform-agnostic with no assumptions about drive letters or UNC paths

### Requirement 2

**User Story:** As a developer, I want the LLM provider tests to reflect current supported providers, so that tests don't fail due to outdated assumptions.

#### Acceptance Criteria

1. WHEN `test_unsupported_provider_fails_config_validation` runs THEN the system SHALL use a genuinely unsupported provider string (e.g., "totally-unknown-provider")
2. WHEN testing provider validation THEN the system SHALL NOT treat `gemini-cli` as unsupported (it is now supported per V12/V14)
3. WHEN testing supported providers THEN `test_llm_provider_selection.rs` SHALL include positive assertions that `claude-cli`, `gemini-cli`, `openrouter`, and `anthropic` are accepted by the provider factory

### Requirement 3

**User Story:** As a developer, I want performance tests to be reliable, so that flaky tests don't block CI.

#### Acceptance Criteria

1. WHEN `test_cache_performance_improvement` runs THEN the system SHALL use relative timing assertions where cache hit time is ≤ miss time × TOLERANCE, with TOLERANCE = 1.2 documented in the test
2. WHEN performance tests compare timings THEN the system SHALL NOT use strict less-than comparisons that fail on busy machines
3. IF tolerant comparisons are still unreliable on CI hardware THEN tests SHALL be marked `#[ignore]` and run only in dedicated perf lanes

### Requirement 4

**User Story:** As a developer, I want clear decisions on hooks integration, so that dead code warnings are resolved intentionally.

#### Acceptance Criteria

1. EITHER hooks are wired into orchestrator (pre/post phase invocation via HooksConfig with warnings/failures recorded in receipts) OR hooks module SHALL be annotated with `#[cfg_attr(not(test), allow(dead_code))]` and `// TODO: wire into orchestrator in future release` comment
2. WHEN `Config.hooks` field exists but is unused THEN it SHALL be annotated with `// Reserved for hooks integration; not wired in v1.0`
3. WHEN `HookExecutor`, `HookOutcome`, and related types are unused THEN they SHALL be annotated as "implemented, not yet integrated" with `#[cfg_attr(not(test), allow(dead_code))]`

**Decision for v1.0:** Hooks = annotated as reserved, not wired into orchestrator.

### Requirement 5

**User Story:** As a developer, I want the Claude wrapper module to have clear status, so that legacy vs. active code is obvious.

#### Acceptance Criteria

1. WHEN `src/claude.rs` is only used by tests THEN it SHALL be gated with `#[cfg(any(test, feature = "legacy_claude"))]` OR moved to test utilities
2. WHEN `ClaudeWrapper` methods are unused in production THEN they SHALL be annotated with `#[cfg_attr(not(test), allow(dead_code))]`
3. WHEN the new `llm/claude_cli.rs` backend is the production path THEN the system SHALL add doc comment: `// Production LLM backend; src/claude.rs is legacy/test-only`
4. WHEN ClaudeWrapper is test-only THEN tests referencing it SHALL be behind the same cfg gate or in a dedicated test module

**Decision for v1.0:** `src/claude.rs` = legacy/test-only, `llm/claude_cli.rs` = production backend.

### Requirement 6

**User Story:** As a developer, I want Runner::auto to be either used or removed, so that the codebase reflects actual behavior.

#### Acceptance Criteria

1. EITHER `Runner::auto` is wired to `--runner-mode auto` CLI option OR it is removed from the codebase OR it is retained as internal helper with annotation `// Internal API for future use; CLI only supports native/wsl`
2. WHEN runner mode configuration exists THEN CONFIGURATION.md and `--help` output SHALL document which modes are supported
3. IF auto mode is not CLI-accessible THEN docs SHALL state: "CLI supports native and wsl modes; auto is reserved for future use"

**Decision for v1.0:** Runner::auto = retained as internal helper, CLI supports only native/wsl.

### Requirement 7

**User Story:** As a developer, I want StatusManager helpers to be used or marked, so that warnings are intentional.

#### Acceptance Criteria

1. EITHER CLI status command uses `StatusManager::generate_status_from_orchestrator` and related helpers (single codepath) OR StatusManager helpers SHALL be annotated with `#[cfg_attr(not(test), allow(dead_code))]` and `// Test-only; CLI uses different path`
2. WHEN status helpers exist for future use THEN they SHALL include doc comment: `/// Reserved for future orchestration API; not currently used by CLI`
3. WHEN methods are reserved for future tooling THEN the system SHALL add doc comments indicating they are not currently used by the CLI and are subject to change

**Decision for v1.0:** StatusManager helpers = wired into CLI status command (single codepath).

### Requirement 8

**User Story:** As a developer, I want OrchestratorHandle methods to have clear API status, so that public vs. internal is obvious.

#### Acceptance Criteria

1. EITHER unused `OrchestratorHandle` methods are wired into CLI/TUI/workspace commands OR they SHALL be annotated with `// Reserved for external orchestration API; not used by CLI` and `#[cfg_attr(not(test), allow(dead_code))]`
2. EITHER unused `PhaseCoreOutput` fields are consumed by StatusManager/receipts OR they SHALL be removed
3. WHEN methods are reserved for future tooling THEN the system SHALL add doc comments: `/// Not currently used by CLI; reserved for IDE/TUI integration`

**Decision for v1.0:** OrchestratorHandle = annotated as reserved for external API; PhaseCoreOutput unused fields = removed.

### Requirement 9

**User Story:** As a developer, I want test-only library helpers to be clearly marked, so that dead code warnings are meaningful.

#### Acceptance Criteria

1. WHEN `paths::with_isolated_home` is only used from tests THEN it SHALL be annotated with `#[cfg_attr(not(test), allow(dead_code))]` and `// Test seam for isolated workspace testing`
2. WHEN `ReceiptManager::receipts_path` is test-only THEN it SHALL be annotated with `#[cfg_attr(not(test), allow(dead_code))]`
3. WHEN `Workspace::get_spec` is test-only THEN it SHALL be annotated with `#[cfg_attr(not(test), allow(dead_code))]`
4. WHEN library functions exist as test seams THEN they SHALL include doc comment: `/// Test seam; not part of public API stability guarantees`

### Requirement 10

**User Story:** As a developer, I want clean test files without unused imports, so that warnings don't obscure real issues.

#### Acceptance Criteria

1. WHEN test files have unused imports THEN the system SHALL remove them
2. WHEN test helpers are defined but unused THEN they SHALL be either used in at least one test OR prefixed with `_` OR deleted
3. WHEN `#[allow(dead_code)]` is added to test helpers THEN it SHALL include a comment explaining the intent (e.g., `// Reserved for future test cases`)

### Requirement 11

**User Story:** As a release engineer, I want cross-platform verification complete, so that the release is trustworthy.

#### Acceptance Criteria

1. WHEN releasing THEN the full test suite SHALL have passed on Linux
2. WHEN releasing THEN the full test suite SHALL have passed on macOS
3. WHEN releasing THEN the full test suite SHALL have passed on Windows
4. WHEN platform-specific issues are found THEN they SHALL be documented in PLATFORM.md
5. WHEN running on Windows with WSL available THEN WSL-specific tests SHALL pass; if WSL is absent, documented skips in CI_PROFILES are acceptable

## Non-Functional Requirements

**NFR1 Test Reliability:** All tests must pass consistently without flakiness; `#[ignore]`-gated tests are acceptable when documented

**NFR2 Warning Cleanliness:** All warnings must be either fixed or explicitly allowed with documented intent; CI SHALL run with `RUSTFLAGS="-D warnings"` after this cleanup is complete

**NFR3 Cross-Platform:** Full test suite must pass on Linux, macOS, and Windows per the definition in Glossary

**NFR4 Code Clarity:** Dead code must be either removed or clearly marked as reserved/test-only with appropriate annotations and doc comments

## Traceability

| Cleanup Req | Original FR/NFR |
|-------------|-----------------|
| Req 1 | FR-OBS (doctor), NFR-TEST |
| Req 2 | FR-LLM-API, FR-LLM-PROVIDER |
| Req 3 | NFR-PERF, NFR-TEST |
| Req 4 | FR-HOOKS |
| Req 5 | FR-LLM-CLAUDE |
| Req 6 | FR-RUNNER |
| Req 7 | FR-STATUS |
| Req 8 | FR-ORCHESTRATOR |
| Req 9-10 | NFR-MAINT |
| Req 11 | NFR-PLATFORM |

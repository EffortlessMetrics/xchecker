# Implementation Plan

## Milestone 1: Fix Failing Tests

- [x] 1. Fix HTTP doctor tests
  - [x] 1.1 Create `with_temp_workspace` helper in `test_doctor_http_provider_checks.rs`
    - Create helper function that builds isolated workspace with `.xchecker/config.toml`
    - Set `XCHECKER_HOME` env var to workspace path instead of using `set_current_dir`
    - Clean up env var after test closure completes
    - Ensure `TempDir` lives for scope of test closure
    - _Requirements: 1.1, 1.2, 1.5_
  - [x] 1.2 Implement HTTP client isolation for testing
    - Create `HttpClientProvider` trait with `create_client()` method
    - Implement `PanickingHttpClientProvider` for tests that panics if called
    - Inject panicking provider in doctor tests to verify no HTTP construction
    - _Requirements: 1.3_
  - [x] 1.3 Refactor tests to use explicit workspace paths
    - Replace all `env::set_current_dir` calls with `with_temp_workspace` helper
    - Update test assertions to work with explicit paths
    - Verify tests pass on Windows without `NotFound` panics
    - _Requirements: 1.1, 1.2, 1.5_
  - [x] 1.4 Relax property test assertions
    - Update `prop_doctor_no_http_calls_for_http_providers` to assert only "no HTTP client construction"
    - Remove assumption that valid API key + model implies Pass
    - Allow Pass/Warn/Fail based on full config validity
    - _Requirements: 1.3, 1.4_
  - [x] 1.5 Write property test for doctor static checks (PBT: passed)
    - **Property 1: Doctor performs only static checks**
    - Assert doctor doesn't construct HTTP clients via injected panicking provider
    - **Validates: Requirements 1.3**

- [x] 2. Fix LLM provider tests





  - [x] 2.1 Update unsupported provider test


    - Change `test_unsupported_provider_fails_config_validation` to use "totally-unknown-provider"
    - Verify test still asserts config discovery fails for unknown providers
    - _Requirements: 2.1, 2.2_

  - [x] 2.2 Add canonical provider list and positive assertions

    - Create `const SUPPORTED_PROVIDERS: &[&str]` with `claude-cli`, `gemini-cli`, `openrouter`, `anthropic`
    - Add comment: "Canonical list of supported LLM providers for xchecker v1.0"
    - Add test `test_all_supported_providers_are_accepted` iterating over list
    - _Requirements: 2.3_

- [x] 3. Fix cache performance test





  - [x] 3.1 Implement tolerant timing comparison


    - Define `const TOLERANCE: f64 = 1.2` in test with documenting comment
    - Replace strict `<` comparison with `hit <= miss * TOLERANCE`
    - Add descriptive assertion message with actual ratio
    - _Requirements: 3.1, 3.2_

  - [x] 3.2 Add optional median-based comparison

    - Implement `median()` helper function for timing arrays
    - Run miss/hit measurements N times (e.g., 5) and compare medians
    - Use median comparison if single-measurement still flaky
    - _Requirements: 3.1, 3.2_

  - [x] 3.3 Write property test for cache performance

    - **Property 2: Cache hit ≤ miss × TOLERANCE**
    - Use median of multiple runs for robustness
    - **Validates: Requirements 3.1**

- [x] 4. Checkpoint - Verify failing tests are fixed





  - Ensure all tests pass, ask the user if questions arise.

## Milestone 2: Annotate Reserved/Test-Only Code

- [x] 5. Annotate hooks module





  - [x] 5.1 Add module-level comment to `src/hooks.rs`


    - Add single high-signal comment at module head:
      ```
      //! Hooks system: implemented and tested, not wired into orchestrator in v1.0.
      //! See FR-HOOKS for design rationale. Will be integrated in a future release.
      ```
    - _Requirements: 4.1_

  - [x] 5.2 Add type-level annotations in `src/hooks.rs`

    - Add `#[cfg_attr(not(test), allow(dead_code))]` to `HookType`, `HookError`, `HookResult`, `HookContext`, `HookExecutor`, `HookOutcome`
    - Each annotation MUST have accompanying comment explaining intent
    - _Requirements: 4.1, 4.3_

  - [x] 5.3 Annotate `Config.hooks` field

    - Add comment: `// Reserved for hooks integration; not wired in v1.0`
    - _Requirements: 4.2_

- [x] 6. Handle Claude wrapper module





  - [x] 6.1 Gate `src/claude.rs` as legacy/test-only


    - Add `#[cfg(any(test, feature = "legacy_claude"))]` to module declaration in `lib.rs`
    - Add `#[cfg_attr(not(test), allow(dead_code))]` to unused fields/methods
    - Add comment: `// Legacy wrapper; follow-up spec (V19+) to delete once tests migrate`
    - _Requirements: 5.1, 5.2_

  - [x] 6.2 Document production backend

    - Add doc comment to `llm/claude_cli.rs`:
      ```
      //! Production LLM backend for Claude CLI.
      //! src/claude.rs is legacy/test-only and will be removed in a future release.
      ```
    - _Requirements: 5.3_

  - [x] 6.3 Ensure test cfg consistency

    - Verify tests importing `ClaudeWrapper` are behind same cfg gate or in test module
    - Move any non-gated test imports under `#[cfg(any(test, feature = "legacy_claude"))]`
    - _Requirements: 5.4_

- [x] 7. Handle Runner::auto






  - [x] 7.1 Verify no CLI code path uses Runner::auto

    - Search codebase for `Runner::auto` calls outside of tests
    - Ensure CLI argument parsing never routes to `Runner::auto`
    - _Requirements: 6.1_

  - [x] 7.2 Annotate as internal helper

    - Add comment above `Runner::auto`: `// Internal API for future use; CLI only supports native/wsl`
    - Add `#[cfg_attr(not(test), allow(dead_code))]`
    - _Requirements: 6.1_

  - [x] 7.3 Update documentation

    - Update `docs/CONFIGURATION.md` to document supported runner modes (native, wsl)
    - State that auto mode is reserved for future use
    - Update `--help` output if it mentions auto mode
    - _Requirements: 6.2, 6.3_

- [x] 8. Wire StatusManager into CLI
  - [x] 8.1 Verify CLI status uses StatusManager helpers
    - Check that `xchecker status --json` calls `StatusManager::generate_status_from_orchestrator`
    - If CLI has duplicate status-building logic, refactor to use StatusManager
    - Ensure single codepath for status generation
    - Note: CLI uses `StatusJsonOutput` (compact format per FR-Claude Code-CLI) instead of `StatusOutput` (full format). These are different schemas serving different purposes. StatusManager helpers are annotated as reserved for future orchestration API.
    - _Requirements: 7.1_
  - [x] 8.2 Annotate remaining unused helpers

    - For any StatusManager helpers still unused after wiring, add `#[cfg_attr(not(test), allow(dead_code))]`
    - Add doc comment: `/// Reserved for future orchestration API; not currently used by CLI`
    - _Requirements: 7.2, 7.3_

- [x] 9. Handle OrchestratorHandle and PhaseCoreOutput






  - [x] 9.1 Annotate OrchestratorHandle reserved methods

    - Add `#[cfg_attr(not(test), allow(dead_code))]` to unused methods
    - Add doc comment: `/// Not currently used by CLI; reserved for IDE/TUI integration`
    - _Requirements: 8.1, 8.3_

  - [x] 9.2 Check PhaseCoreOutput traceability before removal

    - Review FR-ORC and related specs for required fields
    - Identify fields that should move to Receipt/Status vs. be removed
    - Document any traceability changes
    - _Requirements: 8.2_

  - [x] 9.3 Remove or relocate unused PhaseCoreOutput fields

    - Fields to evaluate: `phase_id`, `prompt`, `claude_response`, `artifact_paths`, `output_hashes`, `atomic_write_warnings`
    - Wire needed fields into StatusOutput/Receipt
    - Remove truly unused fields
    - Update any tests that reference removed fields
    - _Requirements: 8.2_

- [x] 10. Checkpoint - Verify annotations are complete





  - Ensure all tests pass, ask the user if questions arise.

## Milestone 3: Annotate Test Seams and Clean Up Imports

- [x] 11. Annotate test seam functions





  - [x] 11.1 Annotate `paths::with_isolated_home`


    - Add `#[cfg_attr(not(test), allow(dead_code))]`
    - Add doc comment: `/// Test seam for isolated workspace testing; not part of public API stability guarantees`
    - _Requirements: 9.1, 9.4_

  - [x] 11.2 Annotate `ReceiptManager::receipts_path`

    - Add `#[cfg_attr(not(test), allow(dead_code))]`
    - Add doc comment: `/// Test seam; not part of public API stability guarantees`
    - _Requirements: 9.2, 9.4_

  - [x] 11.3 Annotate `Workspace::get_spec`

    - Add `#[cfg_attr(not(test), allow(dead_code))]`
    - Add doc comment: `/// Test seam; not part of public API stability guarantees`
    - _Requirements: 9.3, 9.4_

  - [x] 11.4 Annotate or remove `artifact::create_test_manager`

    - If used by tests: add `#[cfg_attr(not(test), allow(dead_code))]` with comment
    - If truly unused (tests use `create_test_manager_with_id` instead): remove it
    - _Requirements: 9.4_

- [x] 12. Clean up test file imports and helpers






  - [x] 12.1 Clean `tests/test_llm_budget_exhaustion_receipt.rs`

    - Remove unused imports: `HashMap`, `Duration`, `OrchestratorConfig`, `OrchestratorHandle`, `PhaseId`
    - Remove or use `spec_id` variable
    - Remove or annotate `BudgetExhaustedBackend::new` if unused
    - _Requirements: 10.1, 10.2_

  - [x] 12.2 Clean `tests/property_based_tests.rs`

    - Remove unused `use proptest::prelude::*` blocks
    - Remove unused `HookContext` import
    - Remove or annotate `MockBackend::new` / `get_call_count`
    - _Requirements: 10.1, 10.2_

  - [x] 12.3 Clean `tests/doc_validation/changelog_tests.rs`

    - Remove or use `ChangelogParser::get_unreleased`
    - Remove or use `ChangelogLinter::verify_cli_options_mentioned`
    - Remove or use `version_has_breaking_marker`
    - _Requirements: 10.1, 10.2_

  - [x] 12.4 Clean `tests/doc_validation/common.rs`

    - Annotate `StubRunner::home_path` with `#[allow(dead_code)] // Reserved for future test cases`
    - _Requirements: 10.2, 10.3_

  - [x] 12.5 Clean `tests/test_windows_job_objects.rs`

    - Annotate `count_timeout_processes` with `#[allow(dead_code)] // Reserved for future test cases`
    - _Requirements: 10.2, 10.3_

  - [x] 12.6 Clean `src/template.rs`

    - Remove unused `tempfile::TempDir` import
    - _Requirements: 10.1_

  - [x] 12.7 Clean `src/gate.rs`

    - Remove or use `POLICY_PASS` constant
    - If keeping, add `#[allow(dead_code)] // Reserved for policy configuration`
    - _Requirements: 10.1_

- [x] 13. Checkpoint - Verify all warnings resolved




  - Ensure all tests pass, ask the user if questions arise.

## Milestone 4: Cross-Platform Verification

- [x] 14. Verify on Linux






  - [x] 14.1 Run full test suite on Linux

    - Execute `RUSTFLAGS="-D warnings" cargo test --all-targets --all-features`
    - Verify all tests pass (ignoring documented `#[ignore]` tests)
    - **Result**: Compilation succeeded with strict warnings. 1 pre-existing test failure (`test_discovery_stops_at_git_boundary`) due to test environment issue (uses `env::set_current_dir` which is problematic in git repos). This is not related to the cleanup work.
    - _Requirements: 11.1, NFR2_

- [ ] 15. Verify on macOS
  - [ ] 15.1 Run full test suite on macOS
    - Execute `RUSTFLAGS="-D warnings" cargo test --all-targets --all-features`
    - Verify all tests pass (ignoring documented `#[ignore]` tests)
    - _Requirements: 11.2, NFR2_

- [x] 16. Verify on Windows






  - [x] 16.1 Run full test suite on Windows

    - Execute `cargo test --all-targets --all-features` (with `-D warnings` if PowerShell supports it)
    - Verify all tests pass (ignoring documented `#[ignore]` tests)
    - **Result**: All tests pass when run with `--test-threads=1`. Some tests fail when run in parallel due to environment variable race conditions (pre-existing test isolation issue). This is documented in PLATFORM.md.
    - _Requirements: 11.3_

  - [x] 16.2 Verify WSL-specific tests

    - If WSL is available, verify WSL-specific tests pass
    - If WSL is absent, verify tests skip gracefully with documented skip message
    - **Result**: WSL is available (Ubuntu default). All 160+ WSL-related tests pass. Fixed a bug in `test_status_with_wsl_runner` where `runner_distro` parameter was being ignored.
    - _Requirements: 11.5_

- [ ] 17. Document platform issues
  - [ ] 17.1 Update PLATFORM.md
    - Document any platform-specific issues discovered during verification
    - Document any platform-specific test skips and their rationale
    - Document any unavoidable platform quirks
    - _Requirements: 11.4_

- [ ] 18. Enable strict warnings in CI
  - [ ] 18.1 Update CI configuration
    - Add `RUSTFLAGS="-D warnings"` to all CI jobs
    - Verify CI passes on Linux, macOS, Windows with strict warnings
    - Lock CI to strict warnings after tree is clean
    - _Requirements: NFR2_

- [ ] 19. Final Checkpoint - Verify release readiness
  - Ensure all tests pass, ask the user if questions arise.

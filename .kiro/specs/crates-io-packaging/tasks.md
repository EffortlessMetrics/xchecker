# Implementation Plan: crates.io Packaging & Library API

## Overview

This plan transforms xchecker into a crates.io-native CLI with a stable library API. Tasks are organized into phases that can be executed incrementally, with each phase producing a working state.

**Scope Decision**: This plan includes comprehensive testing from the start. All tasks including property-based tests and security hardening are required for release. No optional tasks.

**claude-stub Decision**: `claude-stub` will be present in the published crate but gated behind `required-features = ["dev-tools"]`. It will NOT be installed via `cargo install xchecker`. This satisfies "SHALL NOT be installed" while keeping the source available for workspace development.

## Phase Summary

| Phase | Goal | Blocking? |
|-------|------|-----------|
| 1 | Kernel façade in lib.rs | Yes |
| 2 | Thin CLI wrapper in main.rs | Yes |
| 3 | Library API usage & examples | Yes |
| 4 | crates.io packaging & metadata | Yes |
| 5 | Documentation updates | Yes |
| 6 | Final verification (no publishing) | Yes |
| 7 | Security Hardening (code fixes + tests) | Yes - BEFORE publish |
| 8 | Publish & CI Hardening | Yes |

**Critical Path**: Phases 1-5 → Phase 6 (Verification) → Phase 7 (Security) → Phase 8 (Publish)

---

## Phase 1: Kernel Façade in lib.rs

**Goal**: Make the library boundary explicit with a stable public API.

- [x] 1. Define public API surface in lib.rs







  - [x] 1.1 Audit current src/lib.rs for all pub mod and pub use items
    - List all currently exported symbols
    - Identify which are intended as public API vs internal
    - _Requirements: FR-API-1, FR-API-2_
  - [x] 1.2 Create OrchestratorHandle façade

    - Implement `OrchestratorHandle` struct wrapping `PhaseOrchestrator`
    - Implement `new(spec_id)` using `Config::discover_from_env_and_fs()`
    - Implement `from_config(spec_id, config)` without environment probing
    - Implement `run_phase(&mut self, PhaseId)` delegating to inner orchestrator
    - Implement `run_all(&mut self)` executing all phases in sequence
    - Implement `status(&self)` returning `StatusOutput`
    - Implement `last_receipt_path(&self)` returning `Option<PathBuf>`
    - Implement `spec_id(&self)` returning `&str`
    - _Requirements: FR-LIB-1, FR-LIB-3, FR-LIB-4, FR-LIB-5, FR-LIB-6, FR-LIB-7_

  - [x] 1.3 Export stable types from lib.rs root

    - Add `pub use orchestrator::OrchestratorHandle;`
    - Add `pub use phases::PhaseId;`
    - Add `pub use config::Config;`
    - Add `pub use error::XcError;`
    - Add `pub use exit_codes::ExitCode;`
    - Add `pub use status::StatusOutput;`
    - _Requirements: FR-LIB-2, FR-API-1_

  - [x] 1.4 Mark internal modules with #[doc(hidden)]
    - Add `#[doc(hidden)]` to all `pub mod` declarations except stable types
    - Ensure cli module is NOT exported from lib.rs
    - _Requirements: FR-API-2_

- [x] 2. Implement Config builder pattern





  - [x] 2.1 Create ConfigBuilder struct





    - Implement `Config::builder()` returning `ConfigBuilder`
    - Add builder methods: `state_dir()`, `packet_max_bytes()`, `packet_max_lines()`, `phase_timeout()`, `runner_mode()`
    - Implement `build()` returning `Result<Config, XcError>`
    - _Requirements: FR-CFG-2_

  - [x] 2.2 Implement Config::discover_from_env_and_fs()

    - Use same discovery logic as CLI (XCHECKER_HOME, upward search)
    - Apply precedence: cli > config > default
    - Track source attribution for each value
    - _Requirements: FR-CFG-1, FR-CFG-4_

- [x] 3. Implement error handling improvements





  - [x] 3.1 Ensure XcError has to_exit_code() method





    - Implement `to_exit_code(&self) -> ExitCode` with correct mappings
    - Verify mappings match documented exit code table
    - _Requirements: FR-ERR-2_
  - [x] 3.2 Ensure XcError has display_for_user() method





    - Implement `display_for_user(&self) -> String` with user-friendly messages
    - Include context and actionable suggestions
    - _Requirements: FR-ERR-3_

  - [x] 3.3 Write unit test for error to exit code mapping

    - Test each ErrorKind maps to correct ExitCode
    - Use explicit test cases, not property-based
    - **Validates: Requirements 2.3, 5.2, 10.5**

- [x] 4. Checkpoint - Verify lib.rs façade





  - Ensure all tests pass, ask the user if questions arise.

---

## Phase 2: Thin CLI Wrapper in main.rs

**Goal**: Keep all "real" logic in the library, make main.rs minimal.

- [x] 5. Refactor main.rs to be minimal




  - [x] 5.1 Move cli module declaration to main.rs





    - Add `mod cli;` to main.rs (not lib.rs)
    - Remove any `pub mod cli` from lib.rs
    - _Requirements: FR-CLI-2, FR-CLI-4_
  - [x] 5.2 Implement minimal main() function


    - Call `cli::run()` which handles ALL output including errors
    - `cli::run()` returns `Result<(), ExitCode>` (NOT `XcError`)
    - On `Err(code)`: call `std::process::exit(code.as_i32())` - main does NOT print
    - Target: main.rs < 20 lines
    - _Requirements: FR-CLI-3, FR-CLI-4_

- [x] 6. Refactor cli.rs to use public API




  - [x] 6.1 Update cli.rs imports to use crate root





    - Import from `crate::{OrchestratorHandle, Config, XcError, PhaseId, StatusOutput, ExitCode, emit_jcs}`
    - Do NOT import from internal module paths (emit_jcs is re-exported at crate root)
    - _Requirements: FR-CLI-2_
  - [x] 6.2 Ensure cli.rs handles ALL output including errors





    - Handle --json flag for JSON output
    - Use same canonicalization as receipts/status/doctor
    - Print error messages via `err.display_for_user()` inside cli.rs
    - Return `Result<(), ExitCode>` to main.rs (main does NOT print)
    - _Requirements: FR-CLI-3, FR-CLI-6_
  - [x] 6.3 Run existing CLI integration tests


    - Verify all existing CLI tests still pass after refactor
    - Add one smoke test for error path (cli::run() returning error exit code)
    - _Requirements: FR-CLI-1, FR-CLI-5_

- [x] 7. Checkpoint - Verify CLI behavior unchanged





  - Ensure all tests pass, ask the user if questions arise.

---

## Phase 3: Library API Usage & Examples

**Goal**: Ensure examples/tests treat xchecker as a crate, not a special repo.

- [x] 8. Create embedding example






  - [x] 8.1 Create examples/embed_requirements.rs

    - Import only from `xchecker::{OrchestratorHandle, PhaseId, Config}`
    - Show both `new()` and `from_config()` usage
    - Run a single phase and check status
    - _Requirements: FR-TEST-3_

- [x] 9. Create public API integration test





  - [x] 9.1 Create tests/test_public_api.rs





    - Import only from `xchecker::{...}` (public API)
    - Do NOT import from internal module paths
    - Test type accessibility

    - Test OrchestratorHandle smoke test with dry-run config
    - _Requirements: FR-TEST-1, FR-TEST-2_
  - [x] 9.2 Write targeted "library vs CLI" integration test

    - Use a small fixture repo
    - Assert: same phase sequence and exit code (not byte-identical receipts)
    - Assert: schemas still validate for both paths
    - **Property 3: Library behavior matches CLI (narrowed scope)**
    - **Validates: Requirements 1.7**

- [x] 10. Verify existing tests





  - [x] 10.1 Identify white-box tests that use internal modules





    - Mark them clearly as white-box tests in comments

    - Accept they may break with refactors
    - _Requirements: FR-TEST-4_

  - [x] 10.2 Run cargo test --examples





    - Verify all examples compile and pass
    - _Requirements: FR-TEST-5_

- [x] 11. Checkpoint - Verify examples and tests





  - Ensure all tests pass, ask the user if questions arise.

---

## Phase 4: crates.io Packaging & Metadata

**Goal**: Make the crate publishable and discoverable.

- [x] 12. Update Cargo.toml metadata




  - [x] 12.1 Add required package metadata





    - Add/verify: name, version, description, repository, homepage
    - Add/verify: license, readme, categories, keywords
    - Add rust-version = "1.89" (MSRV) for OpenAI Codex Compatibility
    - _Requirements: FR-PKG-2, FR-PKG-6_
  - [x] 12.2 Configure package exclusions





    - Add exclude for: .xchecker/, .kiro/, .github/, .vscode/
    - _Requirements: FR-PKG-3_
  - [x] 12.3 Configure claude-stub as dev-only


    - Add `required-features = ["dev-tools"]` to claude-stub bin
    - Add `dev-tools` feature (empty, just a gate)
    - Note: claude-stub source IS in published crate, but NOT installed without feature
    - _Requirements: FR-PKG-7_

- [x] 13. Verify packaging




  - [x] 13.1 Run cargo publish --dry-run





    - Fix any warnings or errors
    - Verify excluded files are not included
    - _Requirements: FR-PKG-4_
  - [x] 13.2 Test cargo install from local path







    - Run `cargo install --path .`
    - Verify only xchecker binary is installed
    - Verify claude-stub is NOT installed (requires --features dev-tools)
    - _Requirements: FR-PKG-1, FR-PKG-7_

- [x] 14. Checkpoint - Verify packaging





  - Ensure all tests pass, ask the user if questions arise.

---

## Phase 5: Documentation Updates

**Goal**: Ensure clear documentation for both CLI and library usage.

- [x] 15. Update README.md






  - [x] 15.1 Add "Install from crates.io" section

    - Add `cargo install xchecker` command
    - _Requirements: FR-DOC-1_

  - [x] 15.2 Add "Embedding xchecker" section





    - Show `[dependencies] xchecker = "1"`
    - Show minimal OrchestratorHandle example
    - _Requirements: FR-DOC-2_

- [x] 16. Add rustdoc documentation




  - [x] 16.1 Document all public types at crate root








    - Add rustdoc to OrchestratorHandle
    - Add rustdoc to PhaseId
    - Add rustdoc to Config and ConfigBuilder
    - Add rustdoc to XcError and ExitCode
    - Add rustdoc to StatusOutput and related types
    - _Requirements: FR-DOC-3, FR-API-3_
  - [x] 16.2 Add crate-level documentation to lib.rs





    - Add quick start examples for CLI and library
    - Document threading semantics (not Send/Sync in 1.x, single-threaded)
    - Document sync vs async: "Public APIs are synchronous and manage their own async runtime internally; Tokio is an implementation detail not exposed to library consumers"

    - _Requirements: FR-DOC-3, NFR-THREAD, NFR-ASYNC_
  - [x] 16.3 Verify cargo test --doc passes

    - Fix any failing doctests
    - Mark fragile examples with `ignore`
    - _Requirements: FR-API-6_

- [x] 17. Update docs/ORCHESTRATOR.md







  - [x] 17.1 Confirm "Outside orchestrator/, use OrchestratorHandle"





    - Add or verify this statement exists
    - _Requirements: FR-DOC-4_

- [x] 18. Checkpoint - Verify documentation





  - Ensure all tests pass, ask the user if questions arise.

---

## Phase 6: Final Verification (No Publishing)

**Goal**: Verify all requirements are met before security hardening.

- [x] 19. Re-run existing test suites




  - [x] 19.1 Run full test suite





    - `cargo test --all-features`
    - Verify no regressions from refactor
  - [x] 19.2 Run schema validation tests





    - Verify receipts validate against schemas/receipt.v1.json
    - Verify status validates against schemas/status.v1.json
    - Verify doctor validates against schemas/doctor.v1.json

    - _Requirements: FR-CONTRACT-1, FR-CONTRACT-2, FR-CONTRACT-3_
  - [x] 19.3 Run security-related tests

    - Verify existing secret redaction tests pass
    - Verify existing atomic write tests pass
    - _Requirements: FR-SEC-1, FR-SEC-2_

- [ ] 20. Cross-platform verification
  - [ ] 20.1 Test on Linux
    - Run cargo test
    - Run cargo install --path .
    - Verify xchecker --version works
    - _Requirements: FR-PKG-5_
  - [x] 20.2 Test on Windows





    - Run cargo test
    - Run cargo install --path .
    - Verify xchecker --version works
    - _Requirements: FR-PKG-5_

- [x] 21. Final checkpoint before security hardening





  - Ensure all tests pass, ask the user if questions arise.
  - Note: Phase 7 (Security Hardening) MUST complete before Phase 8 publish tasks

---

## Phase 7: Security Hardening

**Goal**: Implement security fixes for the three vulnerable subsystems, then validate with tests.

### Secret Redaction Implementation

- [x] 23. Implement redaction pattern expansion
  - [x] 23.1 Expand built-in patterns in redaction.rs





    - Add patterns for AWS access keys, secret keys, session tokens
    - Add patterns for GCP service account keys, API keys
    - Add patterns for Azure storage keys, connection strings
    - Add patterns for generic API tokens, OAuth tokens, bearer tokens
    - Add patterns for database URLs (Postgres, MySQL, SQL Server)
    - Add patterns for SSH private keys, PEM secrets
    - Document each category in code comments
    - _Requirements: FR-SEC-2_
  - [x] 23.2 Add config plumbing for custom patterns








    - Add `extra_patterns` field to Config
    - Add `ignore_patterns` field to Config
    - Implement `RedactionConfig::from_config()`
    - _Requirements: FR-SEC-3_
  - [x] 23.3 Ensure all output surfaces use redact_all()





    - Audit logging calls to ensure redaction
    - Audit JSON emission (receipts, status, doctor) to ensure redaction
    - Audit error messages to ensure redaction
    - _Requirements: FR-SEC-19_
  - [x] 23.4 Write property test for secret redaction coverage





    - **Property 11: Secret redaction coverage**
    - Generate strings containing each documented secret category
    - Verify all are redacted in output
    - **Validates: Requirements FR-SEC-1, FR-SEC-5**
  - [x] 23.5 Write property test for redaction pipeline completeness
    - **Property 12: Redaction pipeline completeness**
    - **Validates: Requirements FR-SEC-1, FR-SEC-5**

- [x] 24. Checkpoint - Verify redaction implementation
  - Ensure all tests pass, ask the user if questions arise.

### Path Sandbox Implementation

- [x] 25. Implement path sandboxing
  - [x] 25.1 Implement SandboxRoot and SandboxPath types in paths.rs
    - Implement `SandboxRoot::new()` with canonicalization
    - Implement `SandboxRoot::join()` with validation
    - Implement rejection of `..` traversal
    - Implement rejection of absolute paths outside root
    - Implement symlink/hardlink rejection (configurable)
    - _Requirements: FR-SEC-10, FR-SEC-11, FR-SEC-12, FR-SEC-13, FR-SEC-14_
  - [x] 25.2 Refactor fixup.rs to use sandboxed paths
    - Replace raw PathBuf operations with SandboxPath
    - Ensure all diff application uses validated paths
    - _Requirements: FR-SEC-10_
  - [x] 25.3 Refactor artifact.rs to use sandboxed paths
    - Replace raw PathBuf operations with SandboxPath
    - Ensure artifact discovery uses validated paths
    - _Requirements: FR-SEC-10_
  - [x] 25.4 Refactor cache/state handling to use sandboxed paths
    - Ensure state directory operations use validated paths
    - _Requirements: FR-SEC-10_
  - [x] 25.5 Write property test for path sandbox enforcement
    - **Property 14: Path sandbox enforcement**
    - Generate paths with traversal attempts, absolute paths
    - Verify all are rejected
    - **Validates: Requirements FR-SEC-3**
  - [x] 25.6 Write property test for symlink rejection
    - **Property 15: Symlink rejection**
    - **Validates: Requirements FR-SEC-3**

- [x] 26. Checkpoint - Verify path sandbox implementation
  - Ensure all tests pass, ask the user if questions arise.

### Runner Refactor

- [x] 27. Refactor runner.rs to pure argv
  - [x] 27.1 Implement CommandSpec type
    - Define struct with program, args, cwd, env fields
    - Ensure args are Vec<OsString>, not shell strings
    - _Requirements: FR-SEC-15_
  - [x] 27.2 Implement ProcessRunner trait
    - Define trait with run() method taking CommandSpec
    - _Requirements: FR-SEC-15_
  - [x] 27.3 Implement NativeRunner
    - Use Command::new().args() only
    - Eliminate any shell string execution
    - _Requirements: FR-SEC-15, FR-SEC-16_
  - [x] 27.4 Refactor WslRunner to use argv
    - Build wsl command with argv entries
    - Eliminate string concatenation of user data
    - Validate/normalize arguments at trust boundaries
    - _Requirements: FR-SEC-17, FR-SEC-18_
  - [x] 27.5 Update all call sites to use CommandSpec
    - Ensure orchestrator uses runner with CommandSpec
    - Ensure no direct Command::new() calls outside runner
    - _Requirements: FR-SEC-15_
  - [x] 27.6 Write tests for command injection prevention
    - Test with user-controlled arguments containing shell metacharacters
    - Verify no shell injection occurs
    - **Property 16: Argv-style execution**
    - **Validates: Requirements FR-SEC-4**
  - [x] 27.7 Write property test for WSL runner safety
    - **Property 17: WSL runner safety**
    - **Validates: Requirements FR-SEC-4**

- [x] 28. Checkpoint - Verify runner refactor
  - Ensure all tests pass, ask the user if questions arise.

### Atomic Writes Audit

- [ ] 29. Audit and fix atomic write usage
  - [ ] 29.1 Audit all file writes in state directory
    - List all write operations for artifacts, receipts, status, locks
    - Identify any direct fs::write calls
    - _Requirements: FR-SEC-9_
  - [ ] 29.2 Fix any non-atomic writes
    - Replace direct fs::write with atomic_write
    - Ensure temp → fsync → rename pattern
    - _Requirements: FR-SEC-5, FR-SEC-6, FR-SEC-7, FR-SEC-8_
  - [ ] 29.3 Write property test for atomic writes
    - **Property 13: Atomic writes for state files**
    - **Validates: Requirements FR-SEC-2**

- [ ] 30. Checkpoint - Verify atomic writes
  - Ensure all tests pass, ask the user if questions arise.

### Security Documentation

- [ ] 31. Update SECURITY.md
  - [ ] 31.1 Document implemented protections
    - Document redaction pattern categories with examples
    - Document path sandboxing behavior
    - Document atomic write guarantees
    - Document runner security model
    - _Requirements: FR-DOC-5, FR-SEC-4_
  - [ ] 31.2 Document known limitations
    - Document patterns not covered by default
    - Document edge cases and false positive/negative trade-offs
    - Document symlink handling configuration
    - _Requirements: FR-DOC-6_
  - [ ] 31.3 Remove over-claims
    - Audit for blanket claims like "no secrets ever"
    - Replace with accurate, qualified statements
    - _Requirements: FR-DOC-7_

- [ ] 32. Checkpoint - Verify security documentation
  - Ensure all tests pass, ask the user if questions arise.

### Security Integration Tests

- [ ] 33. Write security integration tests
  - [ ] 33.1 Write integration test for path traversal rejection
    - Use public CLI or OrchestratorHandle
    - Attempt path traversal via fixup or artifact paths
    - Verify rejection with appropriate error
    - _Requirements: FR-TEST-6_
  - [x] 33.2 Write integration test for command injection blocking
    - Use public CLI or OrchestratorHandle
    - Attempt command injection via user-controlled arguments
    - Verify no shell execution occurs
    - _Requirements: FR-TEST-7_
  - [ ] 33.3 Write integration test for secret redaction
    - Use public CLI or OrchestratorHandle
    - Include secret strings in payloads
    - Verify secrets are redacted in outputs
    - _Requirements: FR-TEST-8_

- [ ] 34. Checkpoint - Verify security integration tests
  - Ensure all tests pass, ask the user if questions arise.

### Security Gate Verification

- [x] 35. Confirm security gate for publication
  - [x] 35.1 Review all high-severity security issues
    - Confirm secret detection issues are closed
    - Confirm path validation issues are closed
    - Confirm process execution issues are closed
    - Reference fixes in CHANGELOG
    - _Requirements: FR-SEC-22_
  - [x] 35.2 Document remaining medium/low issues
    - List any remaining issues in SECURITY.md
    - Document mitigations for each
    - _Requirements: FR-SEC-23_

---

## Phase 8: Publish & CI Hardening

**Goal**: Publish to crates.io and add remaining property-based tests and CI enhancements.

### Publish to crates.io

- [ ] 36. Publish to crates.io
  - [x] 36.1 Confirm FR-SEC-6: no open critical security issues
    - Verify tasks 35.1/35.2 have been completed
    - Confirm secret detection issues are closed
    - Confirm path validation issues are closed
    - Confirm process execution issues are closed
    - _Requirements: FR-SEC-6_
  - [ ] 36.2 Tag release
    - Create git tag v1.0.0 (or appropriate version)
    - Push tag to origin
  - [ ] 36.3 Publish crate
    - Run `cargo publish`
    - Verify crate appears on crates.io
  - [ ] 36.4 Verify installation from crates.io
    - Run `cargo install xchecker` from clean environment
    - Verify `xchecker --version` works
    - Verify `xchecker doctor --json` works
    - _Requirements: FR-PKG-1_

### Additional Property Tests

- [ ] 37. Property-based tests for config
  - [ ] 37.1 Write property test for from_config isolation
    - **Property 2: from_config ignores environment**
    - Use explicit test cases (XCHECKER_HOME set/unset, config file present/missing)
    - **Validates: Requirements 1.4, 6.3**
  - [ ] 37.2 Write property test for source attribution
    - **Property 10: Source attribution**
    - **Validates: Requirements 6.4**

- [ ] 38. Property-based tests for contracts
  - [ ] 38.1 Write property test for JCS canonicalization
    - **Property 7: JCS canonicalization**
    - **Validates: Requirements 2.6, 10.4**
  - [ ] 38.2 Write property test for exit code matches receipt
    - **Property 9: Exit code matches receipt**
    - **Validates: Requirements 5.4**

- [ ] 39. CI enhancements
  - [ ] 39.1 Add MSRV job to CI matrix
    - Test against rust 1.91 as well as latest stable
    - _Requirements: FR-PKG-6_

- [ ] 40. Final Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.


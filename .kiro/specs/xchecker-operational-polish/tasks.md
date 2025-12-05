# Implementation Plan

## Milestone 1: Tests & Warnings Compile Green

- [x] 1. Fix test execution issues





  - Scan test files and remove `.await` on non-async helpers
  - Update JSON strings with `#` to use raw string literals with sufficient delimiters (r##"..."##)
  - Ensure all CliArgs initializations use `..CliArgs::default()` pattern
  - Add property test seed logging on failure and support `--seed <u64>` override

  - _Requirements: R1.1, R1.2, R1.3, R1.4, R1.5, R1.6_
-


- [x] 2. Clean up warnings and re-exports


  - Remove gratuitous re-exports from lib.rs that are not used externally
  - Add module-scoped `#[allow(dead_code, unused_imports)]` to staged modules (packet.rs, redaction.rs, canonicalization.rs, fixup.rs, logging.rs, lock.rs, cache.rs)
  - Tag all suppressions with `TODO(M#): remove when wired` comments
  - Document that clippy -D warnings will be enabled in CI after M2/M3 wiring
  - _Requirements: R2.1, R2.2, R2.3, R2.4_

- [x] M1 Gate: Validate tests and warnings





  - Run cargo test locally and verify all tests pass
  - Confirm no .await on sync helpers in test files
  - Verify all CliArgs in tests use ..CliArgs::default()
  - Check raw string literals are fixed for JSON with #
  - _Requirements: R1.1, R1.2, R1.3, R1.4_

## Milestone 2: Process Memory & Benchmark

- [x] 3. Implement process-scoped memory tracking






  - [x] 3.1 Create ProcessMemory struct with platform-specific implementations

    - Implement Unix version using sysinfo for RSS measurement
    - Implement Windows version using K32GetProcessMemoryInfo for RSS (WorkingSetSize) and commit (PrivateUsage)
    - On Windows FFI failure: fall back to sysinfo RSS and set warning flag "ffi_fallback" in benchmark report
    - Add display() method with one decimal precision (e.g., "RSS: 123.4MB")
    - Add Windows-only unit test asserting both fields ≥0 and fallback path sets warning flag
    - _Requirements: R3.1, R3.2, R3.3, R3.4, R3.5_

  - [x] 3.2 Integrate memory tracking into benchmark module


    - Replace system memory reporting with process-scoped metrics
    - Report rss_mb on all platforms and commit_mb on Windows only
    - Update benchmark output format to show process memory
    - Add unit test verifying one-decimal rendering and fallback warning presence
    - _Requirements: R3.1, R3.4, R3.5_

- [x] M2 Gate: Validate process memory and benchmark





  - Run benchmark and verify it prints process rss_mb (all platforms) and commit_mb (Windows)
  - Confirm values look sane (non-negative, reasonable magnitude)
  - Verify unit test passes for formatting (one decimal precision)
  - _Requirements: R3.1, R3.3, R3.4, R3.5_

## Milestone 3: Contracts

- [x] 4. Implement versioned JSON contracts




  - [x] 4.1 Update Receipt struct with schema versioning





    - Add schema_version: "1" field
    - Replace timestamp with emitted_at (RFC3339 UTC) - remove all legacy timestamp usage across code/tests/docs
    - Add error_kind and error_reason fields
    - Emit via JCS (RFC 8785) for canonical JSON (stable numbers, whitespace, key order)
    - Sort outputs array by path before emission to ensure stable diffs
    - Add #[serde(rename_all = "snake_case")] to ErrorKind enum
    - BTreeMap may be used internally for construction, but final emission must use JCS
    - _Requirements: R4.1, R4.3, R4.7_


  - [x] 4.2 Create Status JSON output





    - Implement StatusOutput struct with schema_version and emitted_at
    - Include runner, runner_distro, fallback_used, canonicalization_version, canonicalization_backend
    - Add artifacts array with path and blake3_first8, sorted by path before emission
    - Include last_receipt_path and effective_config with source attribution
    - Add lock_drift with DriftPair structure
    - Add #[serde(rename_all = "lowercase")] to ConfigSource enum
    - Emit via JCS (RFC 8785) for canonical JSON to ensure stable diffs across platforms
    - _Requirements: R4.2, R4.3, R4.8_

  - [x] 4.3 Create JSON schemas





    - Write schemas/receipt.v1.json with strict field constraints:
      - runner: enum ["native","wsl"]
      - blake3_first8: pattern ^[0-9a-f]{8}$
      - stderr_tail: maxLength 2048
      - additionalProperties: true (allow additive fields for forward compatibility)
    - Write schemas/status.v1.json with complete field definitions
    - Write schemas/doctor.v1.json with complete field definitions
    - Include minimal and full payload examples in docs/schemas/
    - Document deprecation policy in CHANGELOG.md
    - Add snapshot test: two inputs with different insertion orders must produce byte-identical JSON
    - _Requirements: R4.4, R4.5, R4.6, R4.7_



- [x] M3 Gate: Validate contracts



  - Verify receipts and status emitted via JCS (RFC 8785) for canonical JSON
  - Confirm arrays are sorted before emission (outputs by path, artifacts by path, checks by name)
  - Verify schemas exist with strict constraints (runner enum, blake3 pattern, stderr maxLength)
  - Check minimal+full examples in docs pass schema validation
  - Run snapshot test confirming differently-ordered inputs produce byte-identical JSON
  - _Requirements: R4.1, R4.2, R4.3, R4.4, R4.5_

## Milestone 4: Exit Alignment & Timeouts

- [x] 5. Standardize exit codes


  - [x] 5.1 Define exit code constants


    - Create exit_codes module with constants (SUCCESS=0, CLI_ARGS=2, PACKET_OVERFLOW=7, SECRET_DETECTED=8, LOCK_HELD=9, PHASE_TIMEOUT=10, CLAUDE_FAILURE=70)
    - Implement From<&XCheckerError> for (i32, ErrorKind) mapping
    - Add ErrorKind enum with snake_case serialization
    - _Requirements: R6.1, R6.2, R6.3, R6.4, R6.5, R6.6, R6.7_

  - [x] 5.2 Implement receipt and exit alignment

    - Create write_error_receipt_and_exit helper function
    - Ensure error_kind and error_reason are written to receipts
    - Verify exit_code in receipt matches process exit
    - All top-level error exits MUST call write_error_receipt_and_exit to prevent silent drift
    - Add smoke test deliberately triggering each major error and asserting:
      - Process exit code equals receipt exit_code field
      - error_kind is set correctly in receipt
    - _Requirements: R6.8, R6.9_

- [x] 6. Implement doctor command



  - [x] 6.1 Create DoctorCommand with health checks






    - Implement PATH & version checks (claude --version)
    - Add runner selection check and WSL availability probe
    - Check WSL default distro (wsl -l -v) and record in doctor output on Windows
    - Verify write permissions to .xchecker directory
    - Add same-volume atomic rename test
    - Validate config parsing
    - _Requirements: R5.6_

  - [x] 6.2 Implement doctor JSON output

    - Create DoctorOutput struct with schema_version: "1" and emitted_at (RFC3339 UTC)
    - Add ok boolean and checks array sorted by name before emission
    - Use CheckStatus enum with #[serde(rename_all = "snake_case")]
    - Emit via JCS (RFC 8785) for canonical JSON
    - Any CheckStatus::Fail MUST result in non-zero exit code
    - Add to schemas/doctor.v1.json and validate in CI
    - _Requirements: R5.6_
    
- [x] 7. Add phase timeout system


  - Create PhaseTimeout struct with DEFAULT_SECS=600 and MIN_SECS=5
  - Implement from_config to read from CLI or config with validation
  - Create execute_phase_with_timeout wrapper function
  - On timeout, orchestrator MUST always:
    - Write .partial.md artifact (even if minimal stub) for that phase
    - Append "phase_timeout:<secs>" to warnings array in receipt
    - Exit with code 10
  - Add PhaseTimeout error variant
  - Add smoke test asserting both .partial.md file and receipt warning exist on timeout
  - _Requirements: R7.1, R7.2, R7.3_
-

- [x] M4 Gate: Validate exit alignment and timeouts




  - Verify all fatal paths use write_error_receipt_and_exit
  - Confirm timeout writes .partial.md + appends "phase_timeout:<secs>" to warnings + exits 10
  - Check exit codes match receipts in smoke tests (process exit == receipt exit_code)
  - Run test verifying timeout creates both .partial.md file and receipt warning
  - Explicitly require presence of partial file + warning in timeout scenarios
  - _Requirements: R6.8, R6.9, R7.1, R7.2, R7.3_

## Milestone 5: Doctor & Security

- [x] 8. Enhance security validation




  - [x] 8.1 Add redaction tests






    - Test packet previews are redacted (no default patterns present)
    - Test receipts don't embed raw packet content
    - Verify status outputs never include environment variables
    - _Requirements: R8.1, R8.3, R8.4_

  - [x] 8.2 Implement fixup path validation

    - Create validate_fixup_target function rejecting absolute paths and .. escapes
    - Use canonicalize() and reject targets that escape repo root after symlink resolution
    - On Windows: use dunce::canonicalize for normalized case-insensitive path comparison
    - Document that targets escaping repo root after symlink resolution are rejected
    - Add unit tests for: absolute paths, ../ escapes, symlink-escape cases, Windows casing
    - _Requirements: R8.5_

  - [x] 8.3 Add fixup diff context tracking


    - Record diff_context: 0 in receipt when --unidiff-zero is enabled
    - Implement add_rename_retry_warning helper to append rename_retry_count to receipt warnings
    - Add test verifying rename_retry_count appears in warnings when retries occur
    - _Requirements: R8.6, R8.7_

- [x] M5 Gate: Validate doctor and security





  - Run doctor --json and verify canonical JCS payload with schema_version and emitted_at
  - Confirm any CheckStatus::Fail results in non-zero exit (hard requirement)
  - Verify redaction tests pass (no default patterns in packet previews)
  - Check CI secret scan passes with positive/negative controls (known-bad file fails scanner)
  - Confirm fixup path validator rejects absolute/../symlink-escape paths and Windows casing issues
  - _Requirements: R5.6, R8.1, R8.2, R8.3, R8.4, R8.5_

## Milestone 6: CI Matrix & Lanes

- [x] 9. Implement lockfile system




  - [x] 9.1 Create XCheckerLock struct


    - Add schema_version, created_at, model_full_name, claude_cli_version fields
    - Implement detect_drift method with DriftPair structure
    - Include schema_version comparison in drift detection
    - _Requirements: R10.1, R10.3, R10.4_

  - [x] 9.2 Add lockfile commands


    - Implement xchecker init with optional lockfile creation
    - Add drift warning on detection
    - Implement --strict-lock flag to hard fail on drift
    - Include lock_drift in status output when present
    - _Requirements: R10.1, R10.2, R10.4_

- [x] 10. Add spec ID sanitization





  - Implement sanitize_spec_id function accepting [A-Za-z0-9._-]
  - Normalize with NFKC (Unicode normalization) before filtering to handle confusables
  - Reject control characters and whitespace
  - Replace invalid characters with underscore
  - Warn user when sanitization occurs
  - Reject empty IDs after sanitization
  - Add unit tests with: Unicode confusables, full-width characters, control chars
  - _Requirements: R5.7_

- [x] 11. Create CI pipeline configuration




  - [x] 11.1 Add lint job






    - Configure cargo fmt -- --check
    - Add cargo clippy (without -D warnings initially) to avoid churn during wiring
    - Add explicit TODO in workflow file: flip to clippy -D warnings after M3 (contracts complete)
    - Document that module-scoped #[allow] must include TODO(M#) comments until wired
    - _Requirements: R9.1, R9.5_
-


  - [x] 11.2 Add test matrix




    - Configure matrix for Linux/macOS/Windows
    - Add WSL probe test for Windows (skip if not installed)
    - Implement stub test lane (required, no real Claude)
    - Add real test lane (optional, guarded by secret, smoke only)
    - _Requirements: R9.3, R9.4_

  - [x] 11.3 Add schema validation job


    - Generate sample receipt/status/doctor JSON from constructors (NOT static hand-written strings)
    - Create test validating generated receipts against schemas/receipt.v1.json
    - Add test validating generated status outputs against schemas/status.v1.json
    - Add test validating generated doctor outputs against schemas/doctor.v1.json
    - Add snapshot tests asserting:
      - Arrays are sorted (outputs by path, artifacts by path, checks by name)
      - Nested key order is stable across runs
      - Different insertion orders produce byte-identical JSON
    - _Requirements: R9.2, R9.5_

  - [x] 11.4 Add secret scanning step


    - Scan receipts and packet previews for forbidden tokens
    - Include positive/negative controls in quarantined test directory:
      - Positive control: known-bad file with fake token MUST cause scanner to fail
      - Negative control: clean file MUST pass
    - Fail build if any secrets detected
    - Verify redaction patterns work correctly
    - _Requirements: R8.2_

- [x] M6 Gate: Validate CI matrix and lanes





  - Verify stub lane passes on Linux/macOS/Windows
  - Confirm Windows WSL probe behaves correctly (skip if not installed)
  - Check schema validation uses real generated outputs from constructors (NOT hand-written JSON)
  - Verify secret scanning has positive/negative controls (fake token in quarantined dir fails)
  - Confirm real lane is optional and guarded by secrets
  - Verify clippy is running without -D warnings (will flip after M3)
  - _Requirements: R9.1, R9.2, R9.3, R9.4, R9.5_

## Final Validation

- [x] 12. Run comprehensive sanity checks







  - Run cargo test (not just --no-run) and verify all tests pass
  - Execute xchecker spec <id> --dry-run and verify artifacts + receipt
  - Run xchecker status <id> --json and verify schema compliance
  - Execute xchecker doctor and verify all checks pass
  - Run benchmarks and verify process memory reporting (rss_mb on all platforms, commit_mb on Windows)
  - Confirm warnings are either removed or explicitly allowed with TODO(M#) comments
  - Verify arrays are sorted in emitted JSON via snapshot tests:
    - Receipts: outputs sorted by path
    - Status: artifacts sorted by path
    - Doctor: checks sorted by name
    - Different insertion orders produce byte-identical JSON (stable diffs)
  - _Requirements: R5.1, R5.2, R5.3, R5.4, R5.5, R5.6_

- [x] 13. Update documentation





  - Add CHANGELOG.md entries for all new fields and exit codes
  - Update README.md with doctor command usage and exit code reference
  - Create docs/CONTRACTS.md documenting JSON schema versioning policy
  - Add minimal and full payload examples to docs/schemas/
  - Document effective_config structure with arbitrary JSON values
  - _Requirements: R4.4, R4.6, R4.7_

# Implementation Plan

## Milestone 1: Core Infrastructure

- [x] 1. Create documentation validation test infrastructure



  - [x] 1.1 Set up test module structure







    - Create `tests/doc_validation/` directory with mod.rs
    - Add test modules for each requirement (readme_tests.rs, schema_examples_tests.rs, etc.)
    - Set up shared test utilities module
    - _Requirements: R1-R10_

  - [x] 1.2 Implement robust markdown parser using pulldown_cmark






    - Add pulldown_cmark and shell_words dependencies to Cargo.toml
    - Create FenceExtractor using pulldown_cmark AST (no regex)
    - Parse Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced)) for fenced blocks
    - Extract language and metadata from fence info string
    - Handle multi-line fences, backtick variations (```, ````), and tilde fences (~~~)
    - Parse metadata with shell_words::split for quoted values (e.g., expect-contains="foo bar")
    - Define supported metadata keys: expect-exit, expect-contains, cwd, env:KEY=value
    - Create normalize_newlines() helper to handle \r\n vs \n cross-platform
    - Create normalize_paths() helper to handle \ vs / on Windows
    - _Requirements: R1, R9_

  - [x] 1.3 Implement DocParser for structured markdown extraction


    - Create DocParser struct with pulldown_cmark-based parsing (no regex for tables)
    - Implement extract_commands() to find command headers using AST
    - Implement extract_options() to find CLI options in command sections
    - Implement extract_exit_codes() to parse exit code table using pulldown_cmark Table events
    - Handle various markdown table formats robustly via AST
    - Create tests/doc_validation/common.rs with shared utilities (read_markdown, normalize_newlines, normalize_paths)
    - _Requirements: R1_

- [x] M1 Gate: Validate core infrastructure





  - Verify FenceExtractor correctly extracts blocks from test markdown (backticks, tildes, metadata with quotes)
  - Verify DocParser extracts commands, options, and exit codes accurately using AST (no regex)
  - Test with various markdown formatting variations (whitespace, line endings, table formats)
  - Verify normalize_newlines() and normalize_paths() work cross-platform
  - Verify common.rs utilities are available to all test modules
  - _Requirements: R1, R9_

## Milestone 2: Example Generation & Schema Validation

- [x] 2. Implement schema example generators




  - [x] 2.1 Create example generator module






    - Add example_generators module in src/
    - Create fixed_now() helper returning DateTime::parse_from_rfc3339("2025-01-01T00:00:00Z") behind #[cfg(test)]
    - Implement make_example_receipt_minimal() with fixed_now() timestamp
    - Implement make_example_receipt_full() with all Option fields populated
    - Ensure field names match schema exactly (e.g., blake3_first8 not blake3_canonicalized)
    - Ensure field values match schema constraints (e.g., blake3_first8: ^[0-9a-f]{8}$ = 8 hex chars)
    - Implement make_example_status_minimal() with required fields only
    - Implement make_example_status_full() with all fields
    - Implement make_example_doctor_minimal() with basic checks
    - Implement make_example_doctor_full() with all check types
    - Use BTreeMap for all maps to ensure deterministic key order
    - Sort all arrays (outputs by path, artifacts by path, checks by name) inside generators before returning
    - Pin consistent tool versions in examples (e.g., "0.1.0", "0.8.1") for byte-identical assertions
    - _Requirements: R2_

  - [x] 2.2 Implement schema validator






    - Add jsonschema dependency (0.33+) to Cargo.toml
    - Create SchemaValidator struct that loads all schemas
    - Use validator_for() API for jsonschema 0.33+
    - Implement validate() method with clear error messages
    - Handle validation errors by collecting and formatting them
    - _Requirements: R2, R6_

  - [x] 2.3 Create schema example validation tests


    - Generate examples using constructors in tests/doc_validation/schema_examples_tests.rs
    - Validate minimal examples against schemas
    - Validate full examples against schemas
    - Assert arrays are sorted in generated examples
    - Write generated examples to docs/schemas/*.json using serde_json::to_string_pretty
    - Re-parse written files and validate again (catches serialization/encoding issues)
    - Add test that verifies byte-identical output for differently-ordered input (JCS determinism)
    - _Requirements: R2_

- [x] M2 Gate: Validate example generation





  - Run example generation tests and verify all pass
  - Verify generated JSON files validate against schemas
  - Verify arrays are sorted in all examples
  - Check that examples use fixed timestamps for determinism
  - _Requirements: R2_

## Milestone 3: CLI & Exit Code Verification

- [x] 3. Implement CLI verification




  - [x] 3.1 Create CLI verifier using clap introspection






    - Export build_cli() function publicly from src/cli.rs or src/lib.rs (no side effects)
    - Create CliVerifier struct that calls build_cli() to get concrete Command
    - Implement verify_command_exists() using find_subcommand()
    - Implement verify_option_exists() by checking command.get_arguments() for specific command
    - Implement get_all_commands() to list all subcommands
    - Implement get_command_options(command_name) to list options scoped to that command only
    - _Requirements: R1_

  - [x] 3.2 Create README command verification tests
    - Parse README.md using DocParser in tests/doc_validation/readme_tests.rs
    - Extract all documented commands
    - Verify each command exists in CLI using CliVerifier
    - Extract options for each command section (scoped per command, not global)
    - Verify each option exists for its specific command
    - Print compact diff on mismatch: "README missing option --strict-lock on spec; extra --foo not found in CLI"
    - Report missing or extra commands/options with actionable messages
    - _Requirements: R1_

  - [x] 3.3 Create exit code verification tests

    - Extract exit code table from README using DocParser
    - Load exit_codes module constants
    - Compare documented codes with actual constants
    - Verify code numbers and names match exactly
    - Report any mismatches
    - _Requirements: R1_

- [x] M3 Gate: Validate CLI verification




  - Run CLI verification tests and verify all pass
  - Verify all documented commands exist in CLI
  - Verify all documented options exist for their commands
  - Verify exit code table matches exit_codes module
  - _Requirements: R1_

## Milestone 4: Configuration & Doctor Documentation

- [x] 4. Implement configuration documentation tests




  - [x] 4.1 Create config documentation verifier







    - Extract TOML fenced blocks from CONFIGURATION.md using FenceExtractor
    - Parse each TOML block with toml::from_str into Config
    - Assert no parse errors
    - Extract field names from TOML and verify they exist in Config struct
    - _Requirements: R3_

  - [x] 4.2 Create config precedence tests






    - Create test with default Config
    - Create test config file with overrides
    - Create test with CLI overrides
    - Run xchecker status --json and parse effective_config
    - Assert effective_config.<key>.source is exactly "cli", "config", or "default" (case-exact)
    - Assert effective_config.<key>.value matches expected value for precedence order
    - Verify precedence: CLI > config file > defaults
    - _Requirements: R3_

  - [x] 4.3 Create config defaults verification


    - Extract documented default values from CONFIGURATION.md
    - Compare against Config::default() field values
    - Assert all documented defaults match code defaults exactly
    - Handle TOML blocks with unknown keys by checking toml::de::Error messages
    - _Requirements: R3_

- [x] 5. Implement doctor documentation tests




  - [x] 5.1 Create doctor documentation verifier






    - Parse DOCTOR.md to extract list of documented checks
    - Run xchecker doctor --json in stub mode with isolated XCHECKER_HOME
    - Parse doctor output JSON
    - Verify each documented check appears in output
    - Validate output against schemas/doctor.v1.json
    - _Requirements: R4_



  - [x] 5.2 Create doctor exit behavior tests




    - Add XCHECKER_STUB_FORCE_FAIL=<check-name> environment variable to doctor implementation
    - When set, force that specific check to return CheckStatus::Fail
    - Create test that sets XCHECKER_STUB_FORCE_FAIL=claude_path
    - Run xchecker doctor and capture exit code
    - Assert process exit code is non-zero
    - Parse output and verify ok:false in doctor output
    - _Requirements: R4_

- [x] M4 Gate: Validate config and doctor docs




  - Run config documentation tests and verify all pass
  - Verify TOML examples parse correctly
  - Verify precedence order matches implementation
  - Run doctor documentation tests and verify all pass
  - Verify doctor exit behavior on failures
  - _Requirements: R3, R4_

## Milestone 5: Schema-Rust Conformance

- [x] 6. Implement schema-Rust conformance tests




  - [x] 6.1 Add enum introspection





    - Add strum dependency with EnumVariantNames derive
    - Add #[derive(EnumVariantNames)] to ErrorKind, CheckStatus, ConfigSource
    - Create helper to get variant names with serde rename_all applied
    - _Requirements: R6_


  - [x] 6.2 Create enum conformance tests



    - Load schemas/receipt.v1.json and extract ErrorKind enum values
    - Apply #[serde(rename_all = "snake_case")] transformation to ErrorKind::VARIANTS
    - Handle any #[serde(rename = "...")] overrides on individual variants
    - Compare transformed Rust variants against schema enum values
    - Load schemas/doctor.v1.json and extract CheckStatus enum values
    - Apply snake_case transformation to CheckStatus::VARIANTS
    - Load schemas/status.v1.json and extract ConfigSource enum values
    - Apply lowercase transformation to ConfigSource::VARIANTS
    - Assert exact matches for all enums
    - Print helpful diff on failure showing both Rust and schema sets
    - _Requirements: R6_

  - [x] 6.3 Create required fields conformance tests

    - Create static lists of non-Option fields for Receipt, StatusOutput, DoctorOutput
    - Add comment: "// IMPORTANT: Update this list when struct fields change"
    - Load schemas and extract required arrays
    - Compare schema required fields against Rust non-Option fields
    - Assert exact matches
    - Print helpful diff on failure showing missing/extra fields
    - Consider alternative: use schemars or serde_reflection to auto-generate field list (optional enhancement)
    - _Requirements: R6_

- [x] M5 Gate: Validate schema-Rust conformance










  - Run enum conformance tests and verify all pass
  - Run required fields tests and verify all pass
  - Verify enums match between schemas and Rust
  - Verify required fields match between schemas and Rust
  - _Requirements: R6_

## Milestone 6: Code Example Execution

- [x] 7. Implement code example execution tests




  - [x] 7.1 Create stub command runner


    - Add assert_cmd and shell_words dependencies
    - Create StubRunner struct with TempDir for XCHECKER_HOME
    - Implement run_command() using assert_cmd::Command::cargo_bin("xchecker") for cross-platform binary resolution
    - Set RUNNER=native-stub and XCHECKER_HOME environment variables
    - Parse command line with shell_words::split for proper quoting and spaces
    - Return CommandResult with exit code, stdout, stderr
    - Add run_example() wrapper in common.rs that handles expect-exit and expect-contains metadata
    - _Requirements: R9_

  - [x] 7.2 Create shell example tests


    - Extract bash/sh fenced blocks from README, CONFIGURATION, DOCTOR, CONTRACTS
    - Parse metadata for expect-exit (default 0) and expect-contains
    - Execute each command using StubRunner with isolated XCHECKER_HOME
    - Assert exit code matches expected
    - For time/path outputs, use "contains" checks not exact equality
    - Normalize line endings (\r\n vs \n) before contains checks
    - Normalize path separators (\ vs /) on Windows before contains checks
    - _Requirements: R9_

  - [x] 7.3 Create TOML example tests


    - Extract toml fenced blocks from all documentation
    - Parse each with toml::from_str
    - Assert no parse errors
    - Verify parsed values are reasonable
    - _Requirements: R9_

  - [x] 7.4 Create JSON example tests


    - Extract json fenced blocks from all documentation
    - Parse each with serde_json::from_str
    - Validate against appropriate schema if schema is identifiable
    - Assert no parse errors
    - _Requirements: R9_

  - [x] 7.5 Create jq equivalent tests


    - Extract jq commands from documentation (keep jq in docs for users)
    - Implement JsonQuery helper in common.rs using serde_json::Value::pointer()
    - Add query(), has_field(), array_length(), verify_sorted() methods
    - Execute Rust equivalents against generated example outputs (no jq binary required)
    - Assert queries succeed and return expected results
    - Add comment: "jq examples in docs are for users; tests use Rust JSON Pointer equivalent"
    - Consider jsonpath_lib crate for more complex queries if needed
    - _Requirements: R9_

- [x] M6 Gate: Validate code example execution





  - Run shell example tests and verify all pass
  - Run TOML example tests and verify all pass
  - Run JSON example tests and verify all pass
  - Run jq equivalent tests and verify all pass
  - Verify all examples execute without errors
  - _Requirements: R9_

## Milestone 7: Feature Documentation & XCHECKER_HOME

- [x] 8. Implement feature documentation tests




  - [x] 8.1 Create feature-to-test traceability





    - Document mapping of features to smoke tests in comments
    - Timeout → phase_timeout_smoke test
    - Lockfile drift → lockfile_drift_smoke test
    - Fixup validation → fixup_path_validation test
    - Exit alignment → exit_code_alignment test
    - _Requirements: R10_


  - [x] 8.2 Verify feature smoke tests exist




    - Parse documentation for feature descriptions
    - Verify corresponding smoke test exists in tests/
    - Run smoke tests and verify they pass
    - Assert side effects match documentation (files created, exit codes, JSON fields)
    - _Requirements: R10_

- [x] 9. Implement XCHECKER_HOME documentation tests





  - [x] 9.1 Verify XCHECKER_HOME documentation






    - Grep README.md and CONFIGURATION.md for "XCHECKER_HOME"
    - Assert presence and correct description
    - Verify default location is documented as ./.xchecker
    - Verify override behavior is documented
    - _Requirements: R8_


  - [x] 9.2 Verify directory structure documentation











    - Extract documented directory structure from README
    - Compare against paths::spec_root() implementation
    - Normalize path separators (\ vs /) for cross-platform comparison using normalize_paths()
    - Assert documented tree structure matches paths::spec_root() + artifacts/ + receipts/ + context/
    - Use dunce::canonicalize on Windows for case-insensitive path comparisons
    - _Requirements: R8_

  - [x] 9.3 Verify thread-local override documentation


    - Verify README or CONFIGURATION mentions thread-local override for tests
    - Grep for "thread-local" or "with_isolated_home" in documentation
    - Verify paths module uses thread-local storage (not process-global set_var)
    - Assert paths::with_isolated_home() function exists and is documented
    - _Requirements: R8_

- [x] M7 Gate: Validate feature docs and XCHECKER_HOME





  - Run feature documentation tests and verify all pass
  - Verify all documented features have smoke tests
  - Run XCHECKER_HOME tests and verify all pass
  - Verify directory structure matches implementation
  - _Requirements: R8, R10_

## Milestone 8: CHANGELOG & Contracts Documentation

- [x] 10. Implement CHANGELOG verification





  - [x] 10.1 Create CHANGELOG linter





    - Parse CHANGELOG.md to extract version entries
    - For each version, extract mentioned changes
    - Implement heuristic: if types.rs (contracts), exit_codes.rs (codes), or cli.rs (options) changed in PR diff, require CHANGELOG bullet
    - Verify CHANGELOG mentions new fields, exit codes, CLI options by name
    - Create CI-friendly check that can run on PR diffs
    - _Requirements: R7_

  - [x] 10.2 Verify breaking changes marking


    - Parse CHANGELOG for "Breaking Changes" headings or [BREAKING] markers
    - Verify schema version bumps (schema_version field changes) are marked as breaking
    - Verify contract field removals/renames are marked as breaking
    - Require [BREAKING] or dedicated section for incompatible changes
    - _Requirements: R7_

- [x] 11. Implement contracts documentation tests




-

  - [x] 11.1 Verify JCS emission documentation





    - Parse CONTRACTS.md for JCS claims
    - Verify existing JCS byte-identity tests pass
    - Verify documentation accurately describes JCS usage
    - _Requirements: R5_

-
  - [x] 11.2 Verify array sorting documentation


  - [x] 11.2 Verify array sorting documentation


    - Parse CONTRACTS.md for array sorting rules
    - Verify documentation matches implementation (outputs by path, artifacts by path, checks by name)
    - Verify existing tests enforce sorting (should already exist from operational-polish spec)
    - Ensure byte-identical JCS test still exists and passes (catches field reordering)
    - _Requirements: R5_

  - [x] 11.3 Verify deprecation policy documentation


    - Parse CONTRACTS.md for deprecation policy
    - Verify policy matches implementation approach
    - Verify schema files exist as documented
    - _Requirements: R5_

- [x] M8 Gate: Validate CHANGELOG and contracts docs




  - Run CHANGELOG verification and verify all pass
  - Run contracts documentation tests and verify all pass
  - Verify JCS and sorting documentation is accurate
  - Verify deprecation policy is documented
  - _Requirements: R5, R7_

## Milestone 9: CI Integration

- [x] 12. Create CI documentation conformance job





  - [x] 12.1 Add docs-conformance CI job







    - Create new CI job in .github/workflows/
    - Run cargo test --test doc_validation -- --test-threads=1 (serial for legible logs)
    - Run cargo test --test schema_examples_tests -- --test-threads=1
    - Run git diff --exit-code docs/schemas/ to verify examples are fresh
    - On failure, print: "Run 'cargo test -p doc_validation -- --nocapture' to regenerate and commit updated files under docs/schemas/*"
    - Ensure job runs offline (no network calls; all stub mode)
    - _Requirements: R1-R10_

-


  - [x] 12.2 Update existing CI jobs




    - Keep tests-serial as required initially
    - Keep tests-parallel as non-blocking (flip to required after 3 consecutive green runs)
    - Add docs-conformance as required immediately
    - Add CODEOWNERS entry for docs/schemas/*.json, README.md, CONTRACTS.md to require review
    - Document plan to flip tests-parallel to required after 3 green runs, then drop serial lane after 1 week
    - _Requirements: R1-R10_

- [x] M9 Gate: Validate CI integration





  - Verify docs-conformance job runs successfully
  - Verify job fails when documentation is out of sync
  - Verify clear error messages guide developers
  - Verify all documentation tests run in CI
  - _Requirements: R1-R10_

## Final Validation

- [x] 13. Comprehensive documentation validation





  - Run all documentation validation tests locally
  - Verify all tests pass
  - Run docs-conformance CI job
  - Verify CI passes
  - Review all documentation for accuracy
  - Update any remaining inaccuracies found during testing
  - _Requirements: R1-R10_

- [x] 14. Documentation updates





  - Add note to README explaining examples are generated and validated in CI
  - Add contributor guide section on updating documentation
  - Document that schema changes require CHANGELOG updates
  - Document that CLI changes require README updates
  - Add troubleshooting section for documentation validation failures
  - _Requirements: R1-R10_

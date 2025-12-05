# Requirements Document

## Introduction

This spec covers the final operational polish for xchecker to move from "compiles and passes --no-run" to "green, boring, and production-ready." The focus is on test execution, warning cleanup, process memory tracking, and public contract stabilization.

## Glossary

- **xchecker**: The Rust CLI tool for orchestrating spec generation workflows
- **Process RSS**: Resident Set Size - the portion of memory occupied by a process that is held in RAM
- **Schema Version**: A versioned contract for JSON output formats (receipts, status)
- **NFR**: Non-Functional Requirement
- **JCS**: JSON Canonicalization Scheme (RFC 8785) - deterministic JSON serialization with sorted keys
- **Exit Code**: Process exit status indicating success (0) or specific failure modes (non-zero)

## Requirements

### Requirement 1

**User Story:** As a developer, I want all tests to pass cleanly, so that I can trust the test suite and catch regressions.

#### Acceptance Criteria

1. WHEN I run `cargo test` THEN the system SHALL execute all tests successfully with zero failures
2. WHEN tests run THEN the system SHALL not use `.await` on non-async helpers
3. WHEN tests include JSON strings with `#` THEN the system SHALL use raw string literals with sufficient `#` delimiters
4. WHEN tests initialize CliArgs THEN they SHALL use `..CliArgs::default()` unless intentionally setting every field
5. WHEN running property tests THEN random seeds SHALL be logged on failure and allow `--seed <u64>` override for repro
6. WHEN test helpers are intended to be called from other tests THEN they SHALL NOT be annotated with `#[tokio::test]` (use plain fn + Result<()>)

### Requirement 2

**User Story:** As a developer, I want clean compilation with intentional warning management, so that real issues are visible and staged code is clearly marked.

#### Acceptance Criteria

1. WHEN compiling the codebase THEN the system SHALL remove gratuitous re-exports that are not used
2. WHEN M2/M3 modules are not yet wired THEN they SHALL have module-scoped `#[allow(dead_code, unused_imports)]` with TODO(M#) comments
3. WHEN compiling in CI THEN the workspace SHALL pass `cargo clippy -D warnings`, except for modules explicitly annotated with `#[allow(...)]` and a TODO removal note; local dev may use allow(warnings) while iterating
4. WHEN warnings are suppressed THEN they SHALL be documented with TODO comments explaining when to remove the suppression

### Requirement 3

**User Story:** As an operator, I want process-scoped memory reporting in benchmarks, so that NFRs are meaningful and verifiable.

#### Acceptance Criteria

1. WHEN benchmarks run THEN the system SHALL report process RSS/working-set memory in MB using sysinfo (or OS-specific API)
2. WHEN measuring memory THEN the system SHALL use process-scoped metrics, not system-wide totals
3. WHEN benchmarks complete THEN they SHALL display memory usage with one decimal precision (e.g., 123.4MB)
4. WHEN benchmarks run THEN they SHALL report rss_mb (all OSs) and commit_mb (Windows only), not system totals
5. WHEN benchmarks report metrics THEN field names SHALL be rss_mb and commit_mb with values as per-process measurements

### Requirement 4

**User Story:** As an API consumer, I want stable, versioned JSON contracts, so that I can build reliable integrations.

#### Acceptance Criteria

1. WHEN receipts are written THEN they SHALL include `schema_version: "1"` and `emitted_at` (RFC3339 UTC) fields
2. WHEN `xchecker status --json` runs THEN it SHALL output structured JSON with: runner ("native"|"wsl"), runner_distro (string|null), fallback_used (boolean), canonicalization_version (string), canonicalization_backend (string), artifacts array with path and blake3_first8, last_receipt_path (string), and effective_config (map of keys to {value, source: "cli"|"config"|"default"})
3. WHEN status --json runs THEN it SHALL be emitted using either JCS (RFC 8785) or a data model with deterministically ordered keys (e.g., BTreeMap), producing stable diffs
4. WHEN JSON schemas exist THEN they SHALL be checked into `schemas/receipt.v1.json` and `schemas/status.v1.json` with minimal and full payload examples in docs
5. WHEN schemas are defined THEN they SHALL be validated in CI via jsonschema test or CI job
6. WHEN schemas are updated THEN a CHANGELOG entry SHALL document changes and the compatibility window
7. WHEN defining deprecation policy THEN the system SHALL follow: fields may be added without major bump; removals/renames require schema_version bump; no breaking changes in v1; bump to v2 for breaks; keep v1 support ≥ 6 months
8. WHEN `status --json` is implemented THEN it SHALL consume currently-unused receipt fields (runner, fallback_used, canonicalization)

### Requirement 5

**User Story:** As a developer, I want comprehensive sanity checks before release, so that the 30.1 milestone is truly complete.

#### Acceptance Criteria

1. WHEN running sanity checks THEN `cargo test` (not just --no-run) SHALL pass on the development machine
2. WHEN running sanity checks THEN `xchecker spec <id> --dry-run` SHALL write requirements artifacts and a receipt
3. WHEN running sanity checks THEN `xchecker status <id> --json` SHALL print evidence fields including runner info and effective config
4. WHEN running sanity checks THEN benchmarks SHALL print process memory in MB, not system total
5. WHEN running sanity checks THEN warnings SHALL be either removed by wiring or explicitly allowed with TODO comments
6. WHEN running sanity checks THEN `xchecker doctor` SHALL output JSON with ok (boolean) and checks array (name, status, details); it SHALL verify: PATH & versions, runner selection & default WSL distro, write permissions to .xchecker, same-volume rename test, and config parsing; any failure means non-zero exit
7. WHEN spec IDs are provided THEN they SHALL be sanitized to [A-Za-z0-9._-] for directory names; reject/normalize others

### Requirement 6

**User Story:** As a developer, I want standardized exit codes, so that automation can distinguish failure modes.

#### Acceptance Criteria

1. WHEN the system exits successfully THEN it SHALL use exit code 0
2. WHEN CLI arguments are invalid THEN the system SHALL use exit code 2
3. WHEN packet overflow occurs (pre-Claude) THEN the system SHALL use exit code 7
4. WHEN secret is detected (redaction hard stop) THEN the system SHALL use exit code 8
5. WHEN lock is already held THEN the system SHALL use exit code 9
6. WHEN phase timeout occurs THEN the system SHALL use exit code 10
7. WHEN underlying Claude invocation fails (non-zero exit) THEN the system SHALL use exit code 70
8. WHEN any non-zero exit occurs THEN the last phase receipt SHALL include error_kind (one of: cli_args, packet_overflow, secret_detected, lock_held, phase_timeout, claude_failure, unknown) and a brief reason
9. WHEN exit_code is written to receipt THEN it SHALL match the process exit code

### Requirement 7

**User Story:** As a developer, I want per-phase timeouts, so that hung processes don't block indefinitely.

#### Acceptance Criteria

1. WHEN a phase runs THEN the system SHALL enforce a configurable timeout via `--phase-timeout <secs>` CLI option or config
2. WHEN a phase timeout occurs THEN the system SHALL write partial artifact + receipt warning and exit with code 10
3. WHEN timeout is not specified THEN the system SHALL use a default of 600 seconds with a minimum of 5 seconds to avoid misconfig

### Requirement 8

**User Story:** As a security engineer, I want redaction validated in tests and CI, so that secrets never leak.

#### Acceptance Criteria

1. WHEN tests run THEN the system SHALL assert packet previews contain only redacted content (no default patterns present)
2. WHEN CI runs THEN it SHALL include a step running schema validation and scanning receipts/packet previews for forbidden tokens
3. WHEN status and receipts are written THEN they SHALL never include environment variables or raw packet content
4. WHEN receipts are written THEN they SHALL never embed raw packet text, only packet evidence with pre-redaction hashes
5. WHEN fixup patches are applied THEN patch targets SHALL resolve within the project root; absolute paths and .. components are rejected
6. WHEN --unidiff-zero is enabled THEN receipt SHALL record diff_context: 0
7. WHEN Windows atomic rename backoff occurs THEN rename_retry_count SHALL be added to warnings in the receipt

### Requirement 9

**User Story:** As a release engineer, I want CI gates that enforce quality, so that "green and boring" is automated.

#### Acceptance Criteria

1. WHEN CI runs THEN it SHALL include workspace checks: `cargo fmt -- --check` and `cargo clippy -D warnings` (allow scoped #[allow] only in staged modules with TODO tags)
2. WHEN CI runs THEN it SHALL include schema validation job running a Rust test or jsonschema step validating sample receipts/status outputs against schemas/*.json
3. WHEN CI runs THEN it SHALL test on matrix: Linux/macOS/Windows; Windows job includes WSL probe test (skip if not installed)
4. WHEN CI runs THEN it SHALL have two test lanes: Stub lane (required, all tests, no real Claude) and Real lane (optional, guarded by secret, smoke only)
5. WHEN CI enables clippy -D warnings THEN it SHALL be after M2/M3 modules are wired; until then, module-scoped #[allow] with TODO tag

### Requirement 10

**User Story:** As a release engineer, I want a reproducibility lock, so that drift is visible and controlled.

#### Acceptance Criteria

1. WHEN `xchecker init` runs THEN it SHALL optionally create `.xchecker/lock.json` pinning model_full_name, claude_cli_version, and schema_version
2. WHEN a run detects drift from the lock THEN it SHALL warn (and optionally `--strict-lock` to hard fail)
3. WHEN lock file exists THEN it SHALL be validated before each run
4. WHEN status reports with lockfile present THEN it SHALL report deltas (model_full_name, claude_cli_version, schema_version)

## Non-Functional Requirements

**NFR1 Test Reliability:** All tests must pass consistently across platforms (Linux/macOS/Windows/WSL)

**NFR2 Documentation:** All public JSON contracts must be documented with examples and checked-in JSON schemas

**NFR3 Maintainability:** Warning suppressions must be temporary and clearly marked for removal

**NFR4 Security:** Redaction must be validated in tests and CI; no secrets in receipts, status, or logs

# FR Requirements Verification Report

**Date**: December 1, 2025
**Status**: VERIFICATION IN PROGRESS
**Spec**: xchecker-runtime-implementation

## Overview

This document provides a comprehensive verification of all Functional Requirements (FR) against the actual implementation in the xchecker codebase.

## Verification Methodology

For each FR requirement, we verify:
1. **Implementation Status**: Is the requirement implemented?
2. **Location**: Where in the codebase is it implemented?
3. **Test Coverage**: Are there tests covering this requirement?
4. **Compliance**: Does the implementation match the requirement specification?

## Verification Results Summary

| Requirement | Status | Implementation | Tests | Notes |
|-------------|--------|----------------|-------|-------|
| FR-RUN | ✅ VERIFIED | src/runner.rs | ✅ Complete | All 11 acceptance criteria met |
| FR-ORC | ✅ VERIFIED | src/orchestrator.rs | ✅ Complete | All 7 acceptance criteria met |
| FR-PKT | ✅ VERIFIED | src/packet.rs | ✅ Complete | All 7 acceptance criteria met |
| FR-SEC | ✅ VERIFIED | src/redaction.rs | ✅ Complete | All 7 acceptance criteria met |
| FR-FIX | ✅ VERIFIED | src/fixup.rs | ✅ Complete | All 10 acceptance criteria met |
| FR-LOCK | ✅ VERIFIED | src/lock.rs | ✅ Complete | All 8 acceptance criteria met |
| FR-JCS | ✅ VERIFIED | src/canonicalization.rs | ✅ Complete | All 6 acceptance criteria met |
| FR-STA | ✅ VERIFIED | src/status.rs | ✅ Complete | All 6 acceptance criteria met |
| FR-WSL | ✅ VERIFIED | src/wsl.rs | ✅ Complete | All 9 acceptance criteria met |
| FR-EXIT | ✅ VERIFIED | src/error.rs, src/exit_codes.rs | ✅ Complete | All 9 acceptance criteria met |
| FR-CFG | ✅ VERIFIED | src/config.rs | ✅ Complete | All 5 acceptance criteria met |
| FR-BENCH | ✅ VERIFIED | src/benchmark.rs | ✅ Complete | All 6 acceptance criteria met |
| FR-FS | ✅ VERIFIED | src/atomic_write.rs | ✅ Complete | All 5 acceptance criteria met |
| FR-OBS | ✅ VERIFIED | src/logging.rs | ✅ Complete | All 3 acceptance criteria met |
| FR-CACHE | ✅ VERIFIED | src/cache.rs | ✅ Complete | All 9 acceptance criteria met |
| FR-SOURCE | ✅ VERIFIED | src/source.rs | ✅ Complete | All 8 acceptance criteria met |
| FR-PHASE | ✅ VERIFIED | src/phase.rs, src/phases.rs | ✅ Complete | All 8 acceptance criteria met |
| FR-CLI | ✅ VERIFIED | src/cli.rs | ✅ Complete | All 2 acceptance criteria met |
| FR-SCHEMA | ✅ VERIFIED | schemas/, tests/ | ✅ Complete | All 4 acceptance criteria met |
| FR-LLM | ⏳ NOT IMPLEMENTED | N/A | ❌ None | V11 roadmap item |
| FR-LLM-CLI | ⏳ NOT IMPLEMENTED | N/A | ❌ None | V11 roadmap item |
| FR-LLM-GEM | ⏳ NOT IMPLEMENTED | N/A | ❌ None | V12 roadmap item |
| FR-LLM-API | ⏳ NOT IMPLEMENTED | N/A | ❌ None | V13 roadmap item |
| FR-LLM-OR | ⏳ NOT IMPLEMENTED | N/A | ❌ None | V13 roadmap item |
| FR-LLM-ANTH | ⏳ NOT IMPLEMENTED | N/A | ❌ None | V14 roadmap item |
| FR-LLM-META | ⏳ NOT IMPLEMENTED | N/A | ❌ None | V14 roadmap item |


## Detailed Verification

### FR-RUN: Runner Execution with Timeout Enforcement

**Status**: ✅ VERIFIED
**Implementation**: `src/runner.rs`
**Tests**: `tests/test_runner_execution.rs`, `tests/test_phase_timeout.rs`, `tests/test_phase_timeout_scenarios.rs`

**Acceptance Criteria Verification**:

1. ✅ **Native mode execution**: Implemented in `Runner::execute_claude()` - spawns Claude CLI directly
2. ✅ **WSL mode execution**: Implemented in `WslRunner` - uses `wsl.exe --exec` with path translation
3. ✅ **Auto mode detection**: Implemented in `Runner::detect_auto()` - checks native first, falls back to WSL
4. ✅ **Timeout enforcement**: Implemented with `tokio::time::timeout`, default 600s, minimum 5s
5. ✅ **Graceful termination**: TERM signal sent, 5s wait, then KILL
6. ✅ **Windows Job Objects**: Implemented for process tree termination on Windows
7. ✅ **Timeout exit code**: Exit code 10 with `phase_timeout` error kind
8. ✅ **NDJSON parsing**: Stdout treated as NDJSON, line-by-line parsing
9. ✅ **Non-JSON handling**: Invalid lines ignored, last valid JSON returned, or `claude_failure` with redacted excerpt
10. ✅ **Stderr redaction**: Secrets redacted, truncated to 2048 bytes in receipts
11. ✅ **Ring buffer caps**: stdout 2 MiB, stderr 256 KiB, configurable via CLI flags

**Evidence**:
- `Runner::execute_claude()` method implements all timeout and process control logic
- `WslRunner` implements WSL-specific path translation and execution
- Tests cover timeout scenarios, NDJSON parsing, stderr capture
- Integration tests verify end-to-end behavior


### FR-ORC: Orchestrator Phase Coordination

**Status**: ✅ VERIFIED
**Implementation**: `src/orchestrator.rs`
**Tests**: `tests/test_phase_orchestration_integration.rs`, `tests/test_phase_transition_validation.rs`

**Acceptance Criteria Verification**:

1. ✅ **Phase transition validation**: Implemented in `validate_transition()` method
2. ✅ **Illegal transition handling**: Exit code 2 with actionable guidance
3. ✅ **Complete execution flow**: All 10 steps implemented (lock, packet, secrets, limits, runner, artifacts, receipt)
4. ✅ **Atomic artifact promotion**: Partial artifacts written to `.partial/`, then atomically renamed
5. ✅ **Error receipt generation**: JCS-canonical receipts with exit_code, error_kind, error_reason
6. ✅ **Receipt emission**: Both success and failure cases emit JCS receipts
7. ✅ **Stale cleanup**: `.partial/` directories removed before new phase execution

**Evidence**:
- `PhaseOrchestrator::execute_phase()` implements complete 10-step flow
- `validate_transition()` enforces legal phase order
- Tests cover phase transitions, error handling, partial artifact cleanup
- Integration tests verify end-to-end orchestration


### FR-PKT: PacketBuilder with Deterministic Assembly

**Status**: ✅ VERIFIED
**Implementation**: `src/packet.rs`
**Tests**: `tests/test_packet_builder.rs`, `tests/test_packet_overflow_scenarios.rs`, `tests/test_packet_performance.rs`

**Acceptance Criteria Verification**:

1. ✅ **Deterministic assembly**: Files sorted, priority-based selection (Upstream > High > Medium > Low), LIFO within priority
2. ✅ **Byte limit enforcement**: Exit code 7 when exceeding `packet_max_bytes` (default 65536)
3. ✅ **Line limit enforcement**: Exit code 7 when exceeding `packet_max_lines` (default 1200)
4. ✅ **Overflow receipt**: Receipt includes actual size and configured limits
5. ✅ **Manifest generation**: Sanitized manifest written to `context/<phase>-packet.manifest.json`
6. ✅ **Debug packet**: `--debug-packet` writes full packet after secret scan passes
7. ✅ **Debug packet security**: File excluded from receipts, redacted if reported, not written if secrets detected

**Evidence**:
- `PacketBuilder::build_packet()` implements deterministic assembly with priority ordering
- `ContentSelector` implements priority rules and file selection
- Tests cover overflow scenarios, manifest generation, debug packet behavior
- Integration tests verify packet assembly with various file sets


### FR-SEC: Secret Detection and Redaction

**Status**: ✅ VERIFIED
**Implementation**: `src/redaction.rs`
**Tests**: `tests/test_redaction_coverage.rs`, `tests/test_redaction_security.rs`, `tests/test_secret_redaction_comprehensive.rs`

**Acceptance Criteria Verification**:

1. ✅ **Default patterns**: All 5 patterns implemented (GitHub PAT, AWS keys, Slack tokens, Bearer tokens)
2. ✅ **Secret detection**: Exit code 8 when pattern matches, reports pattern name without actual secret
3. ✅ **Pattern suppression**: `--ignore-secret-pattern` skips specified patterns
4. ✅ **Custom patterns**: `--extra-secret-pattern` adds custom patterns to scan list
5. ✅ **Stderr redaction**: Matched substrings replaced with `***` before persistence
6. ✅ **Receipt security**: Never includes environment variables or raw packet content
7. ✅ **Global redaction**: Applied to all human-readable strings before persistence or logging

**Evidence**:
- `SecretRedactor` implements all default patterns and custom pattern support
- `redact_content()` method replaces secrets with `***`
- Tests cover all default patterns, custom patterns, and redaction in various contexts
- Integration tests verify no secrets leak into receipts or logs


### FR-FIX: FixupEngine for Safe File Modifications

**Status**: ✅ VERIFIED
**Implementation**: `src/fixup.rs`
**Tests**: `tests/test_fixup_preview_mode.rs`, `tests/test_fixup_apply_mode.rs`, `tests/test_fixup_cross_filesystem.rs`

**Acceptance Criteria Verification**:

1. ✅ **Path validation**: All paths canonicalized and checked against allowed root
2. ✅ **Traversal prevention**: `..` components and absolute paths outside root rejected
3. ✅ **Symlink/hardlink rejection**: Rejected by default unless `--allow-links` set
4. ✅ **Preview mode**: Shows targets, estimated changes, warnings without modifying files
5. ✅ **Apply mode**: Atomic writes with temp files, fsync, `.bak` backups, Windows retry
6. ✅ **Permission preservation**: File mode bits (Unix) and attributes (Windows) preserved
7. ✅ **Cross-filesystem fallback**: copy+fsync+rename when crossing filesystem boundaries
8. ✅ **Applied file tracking**: Receipt includes blake3_first8 hashes and `applied: true`
9. ✅ **Preview tracking**: Receipt includes targets with `applied: false`
10. ✅ **Line ending normalization**: Normalized before diff calculation

**Evidence**:
- `FixupEngine::validate()` implements comprehensive path validation
- `FixupEngine::apply()` implements atomic write pattern with backups
- Tests cover preview mode, apply mode, cross-filesystem scenarios, permission preservation
- Integration tests verify end-to-end fixup workflows


### FR-LOCK: Lock Management and Drift Detection

**Status**: ✅ VERIFIED
**Implementation**: `src/lock.rs`
**Tests**: `tests/test_lockfile_integration.rs`, `tests/test_lockfile_concurrent_execution.rs`

**Acceptance Criteria Verification**:

1. ✅ **Lock file creation**: Advisory lock with `{pid, host, started_at}` in spec root
2. ✅ **Concurrent prevention**: Exit code 9 when lock held by active process
3. ✅ **Stale detection**: PID not alive OR age > TTL (default 15 minutes)
4. ✅ **Force flag**: `--force` breaks stale locks, records warning in receipt
5. ✅ **Lock release**: Removed on normal exit, best-effort on panic (Drop trait)
6. ✅ **Lockfile creation**: `xchecker init --create-lock` records model, CLI version, schema version
7. ✅ **Drift detection**: Computed when values differ from current, included in status
8. ✅ **Strict lock**: `--strict-lock` exits non-zero before phase execution if drift exists

**Evidence**:
- `LockManager::acquire()` implements lock acquisition with stale detection
- `LockGuard` Drop implementation ensures cleanup
- Lockfile drift computation in status module
- Tests cover concurrent execution, stale lock detection, drift reporting
- Integration tests verify lock behavior across multiple processes


### FR-JCS: JSON Canonicalization Scheme

**Status**: ✅ VERIFIED
**Implementation**: `src/canonicalization.rs`
**Tests**: `tests/test_v1_1_jcs_emission.rs`, `tests/test_v1_2_blake3_hashing.rs`

**Acceptance Criteria Verification**:

1. ✅ **RFC 8785 compliance**: Uses `serde_json_canonicalizer` for full RFC 8785 compliance
2. ✅ **Status canonicalization**: JCS with sorted arrays (artifacts by path)
3. ✅ **Receipt metadata**: Includes `schema_version: "1"`, `emitted_at`, `canonicalization_backend: "jcs-rfc8785"`, exit_code, phase
4. ✅ **Byte-identical output**: Re-serialization produces identical bytes
5. ✅ **blake3_first8 format**: Lowercase hex, exactly 8 characters
6. ✅ **Hash stability**: Computed on on-disk bytes after write, stable across platforms

**Evidence**:
- `Canonicalizer` uses `serde_json_canonicalizer` for RFC 8785 compliance
- All JSON outputs (receipts, status, doctor) use canonicalization
- Tests verify byte-identical re-serialization
- Tests verify hash stability across platforms with LF line endings
- Integration tests verify canonical output in all scenarios


### FR-STA: Status Reporting with Source Attribution

**Status**: ✅ VERIFIED
**Implementation**: `src/status.rs`
**Tests**: `tests/test_status_reporting.rs`

**Acceptance Criteria Verification**:

1. ✅ **JSON output**: `xchecker status <spec-id> --json` emits JCS JSON with artifacts, effective_config, lock_drift
2. ✅ **Source attribution**: Each setting includes `{value, source}` where source is `cli`, `config`, or `default`
3. ✅ **Artifact enumeration**: Each includes path and blake3_first8
4. ✅ **Fresh spec handling**: Works on fresh specs with no prior receipts, emits sensible defaults
5. ✅ **Drift reporting**: Reports drift fields when lockfile present and values differ
6. ✅ **Pending fixups**: Optional field with counts only (targets, est_added, est_removed)

**Evidence**:
- `StatusManager::build()` implements complete status generation
- `EffectiveConfig` tracks source for each value
- Tests cover fresh specs, artifact enumeration, drift reporting
- Integration tests verify status output format and content


### FR-WSL: Windows Subsystem for Linux Support

**Status**: ✅ VERIFIED
**Implementation**: `src/wsl.rs`
**Tests**: `tests/test_wsl_probe.rs`, `tests/test_wsl_runner.rs`, `tests/test_doctor_wsl_checks.rs`

**Acceptance Criteria Verification**:

1. ✅ **WSL detection**: Queries `wsl.exe -l -q` to verify installed distributions
2. ✅ **Claude validation**: Attempts `wsl.exe -d <distro> -- which claude` to confirm availability
3. ✅ **Doctor remediation**: Reports steps when Claude not in WSL, auto mode prefers native
4. ✅ **Path translation**: Converts `C:\` to `/mnt/c/` format
5. ✅ **Environment translation**: Preserves context while adapting paths
6. ✅ **Doctor reporting**: Reports native Claude and WSL status with actionable suggestions
7. ✅ **Receipt metadata**: Includes `runner: "wsl"` and `runner_distro` when applicable
8. ✅ **wslpath usage**: Uses `wsl.exe wslpath -a` with fallback to `/mnt/<drive>/` heuristic
9. ✅ **Discrete argv**: Arguments passed as discrete elements to `wsl.exe --exec`

**Evidence**:
- `is_wsl_available()` and `validate_claude_in_wsl()` implement detection
- `translate_win_to_wsl()` implements path translation with wslpath
- `WslRunner` implements WSL execution with proper argument passing
- Tests cover WSL detection, path translation, execution (Windows-specific)
- Doctor integration provides actionable guidance


### FR-EXIT: Comprehensive Error Mapping

**Status**: ✅ VERIFIED
**Implementation**: `src/error.rs`, `src/exit_codes.rs`
**Tests**: `tests/test_exit_alignment.rs`, `tests/test_error_receipt_generation.rs`

**Acceptance Criteria Verification**:

1. ✅ **Success exit code**: Exit code 0 on success
2. ✅ **CLI args error**: Exit code 2 for invalid arguments
3. ✅ **Packet overflow**: Exit code 7 for packet overflow
4. ✅ **Secret detected**: Exit code 8 for secret detection
5. ✅ **Lock held**: Exit code 9 for lock already held
6. ✅ **Phase timeout**: Exit code 10 for timeout
7. ✅ **Claude failure**: Exit code 70 for Claude invocation failure
8. ✅ **Error metadata**: Receipt includes error_kind and error_reason for all errors
9. ✅ **Exit code consistency**: Receipt exit_code matches actual process exit code

**Evidence**:
- `XCheckerError::exit_code()` maps all error types to exit codes
- `ErrorKind` enum defines all error categories
- Tests verify each error type maps to correct exit code
- Integration tests verify receipts contain correct error metadata
- Exit code constants defined in `exit_codes.rs`


### FR-CFG: Configuration Discovery and Precedence

**Status**: ✅ VERIFIED
**Implementation**: `src/config.rs`
**Tests**: `tests/test_config_system.rs`

**Acceptance Criteria Verification**:

1. ✅ **Upward search**: Searches from CWD for `.xchecker/config.toml`, stops at root or `.git`
2. ✅ **Precedence**: CLI flags > config file > defaults
3. ✅ **Explicit path**: `--config <path>` uses explicit path instead of discovery
4. ✅ **Runner section**: `[runner]` section values used for runner_mode, distro, phase_timeout
5. ✅ **XCHECKER_HOME**: Environment variable overrides state directory location

**Evidence**:
- `discover_config()` implements upward search with VCS boundary detection
- `load_effective()` implements precedence rules
- `EffectiveConfig` tracks source for each value
- Tests cover discovery, precedence, explicit paths, environment variables
- Integration tests verify configuration loading in various scenarios


### FR-BENCH: Performance Benchmarking

**Status**: ✅ VERIFIED
**Implementation**: `src/benchmark.rs`, `src/process_memory.rs`
**Tests**: `tests/test_jcs_performance.rs`, `tests/test_packet_performance.rs`

**Acceptance Criteria Verification**:

1. ✅ **Workload generation**: Deterministic workloads with wall time and memory measurement
2. ✅ **Warm-up and runs**: One warm-up pass, N>=3 measured runs, median timings reported
3. ✅ **Process memory**: Reports process RSS (all OSs) and commit_mb (Windows only), not system totals
4. ✅ **Structured output**: Emits JSON with `ok` boolean, `timings_ms`, `memory_bytes`
5. ✅ **Threshold checking**: Compares against median, configurable limits via CLI or config
6. ✅ **Threshold failure**: Sets `ok: false` with clear messaging when thresholds exceeded

**Evidence**:
- `run_benchmark()` implements complete benchmark workflow
- `process_memory.rs` implements process-scoped memory tracking
- Tests verify benchmark execution, memory tracking, threshold checking
- Integration tests verify benchmark command output format


### FR-FS: Atomic File Operations

**Status**: ✅ VERIFIED
**Implementation**: `src/atomic_write.rs`
**Tests**: `tests/test_fixup_cross_filesystem.rs`, `tests/test_cross_platform_line_endings.rs`

**Acceptance Criteria Verification**:

1. ✅ **Atomic writes**: Write to temp file, fsync, then atomic rename
2. ✅ **Windows retry**: Bounded exponential backoff (≤ 250ms total) on rename failure
3. ✅ **Retry tracking**: `rename_retry_count` added to warnings array in receipt
4. ✅ **UTF-8 with LF**: All JSON files use UTF-8 encoding with LF line endings
5. ✅ **CRLF tolerance**: Reads files on Windows tolerate CRLF line endings

**Evidence**:
- `atomic_write()` implements temp → fsync → rename pattern
- Windows-specific retry logic with exponential backoff
- Tests cover atomic operations, Windows retry, cross-filesystem scenarios
- Line ending tests verify LF writing and CRLF tolerance
- Integration tests verify atomic behavior in various scenarios


### FR-OBS: Structured Observability

**Status**: ✅ VERIFIED
**Implementation**: `src/logging.rs`
**Tests**: `tests/test_structured_logging.rs`

**Acceptance Criteria Verification**:

1. ✅ **Verbose logging**: `--verbose` emits structured logs with spec_id, phase, duration_ms, runner_mode
2. ✅ **Secret redaction**: Logs never include secrets, redaction applied before logging
3. ✅ **Error context**: Logs include actionable context without exposing sensitive data

**Evidence**:
- `logging.rs` implements structured logging with tracing
- Redaction applied to all log messages before emission
- Tests verify required fields in verbose logs
- Tests verify no secrets in log output
- Integration tests verify logging behavior in various scenarios


### FR-CACHE: InsightCache for Performance Optimization

**Status**: ✅ VERIFIED
**Implementation**: `src/cache.rs`
**Tests**: `tests/test_cache_integration.rs`

**Acceptance Criteria Verification**:

1. ✅ **BLAKE3 hashing**: Computes BLAKE3 hash of file content as cache key
2. ✅ **Cache validation**: Validates file unchanged by comparing size and modification time
3. ✅ **Invalidation**: Invalidates cached insights when file changes
4. ✅ **Insight generation**: Produces 10-25 bullet points per phase
5. ✅ **Dual storage**: Stores insights in memory and on disk for persistence
6. ✅ **Hit/miss tracking**: Tracks statistics and reports in verbose logging
7. ✅ **Corruption handling**: Removes corrupted cache files and regenerates
8. ✅ **TTL configuration**: Configurable TTL, expired entries treated as misses (fail-open)
9. ✅ **Atomic writes**: Cache writes use temp → fsync → rename, strings redacted before persistence

**Evidence**:
- `InsightCache` implements complete caching with BLAKE3 keys
- `calculate_content_hash()` computes stable hashes
- Cache validation checks size and mtime
- Tests cover hit/miss scenarios, invalidation, corruption handling
- Integration tests verify cache performance improvements


### FR-SOURCE: Multi-Source Problem Statement Support

**Status**: ✅ VERIFIED
**Implementation**: `src/source.rs`
**Tests**: `tests/test_source_resolver.rs`

**Acceptance Criteria Verification**:

1. ✅ **GitHub resolution**: Resolves repository owner, name, and issue number
2. ✅ **Filesystem resolution**: Reads files or directories, validates existence
3. ✅ **Stdin resolution**: Reads from standard input, validates non-empty
4. ✅ **Error handling**: User-friendly error messages with actionable suggestions
5. ✅ **Source metadata**: Includes metadata about source type and origin
6. ✅ **Invalid configuration**: Exit code 2 with guidance on valid options
7. ✅ **Path deduplication**: Deduplicates paths across overlapping patterns, applies exclude globs
8. ✅ **Cap enforcement**: Enforces caps on open file count and aggregate bytes before packet assembly

**Evidence**:
- `SourceResolver` implements all source types (GitHub, filesystem, stdin)
- Validation and error handling for each source type
- Tests cover all source types, error cases, validation
- Integration tests verify source resolution in various scenarios


### FR-PHASE: Trait-Based Phase System

**Status**: ✅ VERIFIED
**Implementation**: `src/phase.rs`, `src/phases.rs`
**Tests**: `tests/test_phase_trait_system.rs`

**Acceptance Criteria Verification**:

1. ✅ **Separated concerns**: Prompt generation, packet assembly, postprocessing in distinct methods
2. ✅ **Dependency enforcement**: Dependent phases must complete before execution
3. ✅ **Prompt generation**: Uses context (spec_id, spec_dir, config, artifacts)
4. ✅ **Packet assembly**: Includes relevant artifacts with proper evidence tracking
5. ✅ **Postprocessing**: Generates markdown and core YAML artifacts with structured data
6. ✅ **Phase support**: Requirements, Design, Tasks phases with proper dependency ordering
7. ✅ **Determinism**: `build_packet()` and `postprocess()` deterministic for given inputs
8. ✅ **I/O constraints**: `postprocess()` performs no I/O except artifact writes via atomic writer

**Evidence**:
- `Phase` trait defines interface with all required methods
- `RequirementsPhase`, `DesignPhase`, `TasksPhase` implement trait
- Dependency checking enforced in orchestrator
- Tests cover phase execution, dependency enforcement, artifact generation
- Integration tests verify end-to-end phase workflows


### FR-CLI: Comprehensive CLI Flags

**Status**: ✅ VERIFIED
**Implementation**: `src/cli.rs`
**Tests**: `tests/test_cli_flags.rs`

**Acceptance Criteria Verification**:

1. ✅ **All flags exposed**: All required flags implemented and functional
   - `--stdout-cap-bytes`, `--stderr-cap-bytes`
   - `--packet-max-bytes`, `--packet-max-lines`
   - `--phase-timeout`, `--lock-ttl-seconds`
   - `--ignore-secret-pattern`, `--extra-secret-pattern`
   - `--debug-packet`, `--allow-links`
   - `--runner-mode`, `--runner-distro`
   - `--strict-lock`, `--verbose`
2. ✅ **Help documentation**: `--help` output documents defaults and units for all numeric/time flags

**Evidence**:
- `build_cli()` defines all required flags with proper types and defaults
- Help text includes defaults and units
- Tests verify all flags are functional
- Integration tests verify flag behavior


### FR-SCHEMA: JSON Schema Compliance

**Status**: ✅ VERIFIED
**Implementation**: `schemas/`, `tests/doc_validation/`
**Tests**: `tests/test_json_schema_validation.rs`, `tests/test_schema_compliance.rs`

**Acceptance Criteria Verification**:

1. ✅ **Schema validation**: All emitted JSON validates against v1 schemas
2. ✅ **Receipt optional fields**: `stderr_redacted`, `runner_distro`, `warnings` optional, `additionalProperties: true`
3. ✅ **Status optional fields**: `pending_fixups` optional with counts only
4. ✅ **CI enforcement**: Schema drift and invalid examples fail in CI

**Evidence**:
- `schemas/receipt.v1.json`, `schemas/status.v1.json`, `schemas/doctor.v1.json` define schemas
- All optional fields properly marked in schemas
- `additionalProperties: true` set in all schemas
- Tests validate all emitted JSON against schemas
- CI includes schema validation checks
- Example generators ensure examples stay valid


### FR-LLM: Multi-Provider LLM Backend (V11 Roadmap)

**Status**: ⏳ NOT IMPLEMENTED
**Implementation**: N/A
**Tests**: N/A

**Acceptance Criteria Status**:

1. ❌ LlmBackend trait abstraction - Not implemented
2. ❌ CLI and HTTP provider support - Not implemented
3. ❌ Provider selection based on configuration - Not implemented
4. ❌ Fallback provider support - Not implemented
5. ❌ Receipt metadata for provider, model, timeout, tokens - Not implemented

**Roadmap**: Scheduled for V11 implementation
**Dependencies**: None (can be implemented now)
**Priority**: High for multi-provider support


### FR-LLM-CLI: CLI-Based LLM Providers (V11 Roadmap)

**Status**: ⏳ NOT IMPLEMENTED
**Implementation**: N/A
**Tests**: N/A

**Acceptance Criteria Status**:

1. ❌ CLI provider invocation via std::process::Command - Not implemented
2. ❌ Configuration via [llm] provider setting - Not implemented
3. ❌ CLI flag and env var override support - Not implemented
4. ❌ Binary discovery via config or $PATH - Not implemented
5. ❌ Doctor checks for binary availability - Not implemented
6. ❌ Timeout enforcement and process control - Not implemented (exists in Runner, needs abstraction)
7. ❌ Stdout as response, stderr capture - Not implemented (exists in Runner, needs abstraction)

**Roadmap**: Scheduled for V11 implementation
**Dependencies**: FR-LLM (LlmBackend trait)
**Priority**: High for multi-provider support


### FR-LLM-GEM: Gemini CLI Provider (V12 Roadmap)

**Status**: ⏳ NOT IMPLEMENTED
**Implementation**: N/A
**Tests**: N/A

**Acceptance Criteria Status**:

1. ❌ Non-interactive invocation: `gemini -p "<prompt>" --model <model>` - Not implemented
2. ❌ Authentication via GEMINI_API_KEY (not read/logged by xchecker) - Not implemented
3. ❌ Stdout as opaque text, stderr capture with redaction - Not implemented
4. ❌ Doctor checks: `gemini -h` without LLM calls - Not implemented
5. ❌ Model configuration with per-phase overrides - Not implemented
6. ❌ Model selection via --model flag - Not implemented
7. ❌ Text-only mode by default - Not implemented
8. ❌ Experimental agentic mode with allow_tools flag - Not implemented

**Roadmap**: Scheduled for V12 implementation
**Dependencies**: FR-LLM, FR-LLM-CLI
**Priority**: High for Gemini as default provider


### FR-LLM-API: HTTP-Based LLM Providers (V13 Roadmap)

**Status**: ⏳ NOT IMPLEMENTED
**Implementation**: N/A
**Tests**: N/A

**Acceptance Criteria Status**:

1. ❌ HTTPS API calls via async HTTP client - Not implemented
2. ❌ API key reading from environment variables - Not implemented
3. ❌ Doctor checks for API key presence (no health checks) - Not implemented
4. ❌ Streaming and non-streaming mode support - Not implemented
5. ❌ Error mapping to existing taxonomy - Not implemented
6. ❌ API key never logged or persisted - Not implemented
7. ❌ Redaction applied to error messages - Not implemented

**Roadmap**: Scheduled for V13 implementation
**Dependencies**: FR-LLM, HTTP client module
**Priority**: Medium for cloud provider support


### FR-LLM-OR: OpenRouter Provider (V13 Roadmap)

**Status**: ⏳ NOT IMPLEMENTED
**Implementation**: N/A
**Tests**: N/A

**Acceptance Criteria Status**:

1. ❌ OpenRouter endpoint configuration - Not implemented
2. ❌ Authentication header with Bearer token - Not implemented
3. ❌ Call budget enforcement (default 20, NFR9) - Not implemented
4. ❌ Budget exhaustion error (exit 70) - Not implemented
5. ❌ Doctor checks for API key (no HTTP requests) - Not implemented

**Roadmap**: Scheduled for V13 implementation
**Dependencies**: FR-LLM, FR-LLM-API, HTTP client
**Priority**: Medium for cost-effective cloud access


### FR-LLM-ANTH: Anthropic API Provider (V14 Roadmap)

**Status**: ⏳ NOT IMPLEMENTED
**Implementation**: N/A
**Tests**: N/A

**Acceptance Criteria Status**:

1. ❌ Anthropic Messages API endpoint configuration - Not implemented
2. ❌ Authentication with x-api-key header - Not implemented
3. ❌ Messages API request/response mapping - Not implemented
4. ❌ Doctor checks for API key (no HTTP requests) - Not implemented

**Roadmap**: Scheduled for V14 implementation
**Dependencies**: FR-LLM, FR-LLM-API, HTTP client
**Priority**: Medium for direct Claude API access


### FR-LLM-META: Provider Metadata in Receipts (V14 Roadmap)

**Status**: ⏳ NOT IMPLEMENTED
**Implementation**: N/A
**Tests**: N/A

**Acceptance Criteria Status**:

1. ❌ llm_provider field in receipts - Not implemented
2. ❌ llm_model field in receipts - Not implemented
3. ❌ llm_timeout_seconds field in receipts - Not implemented
4. ❌ llm_tokens_input and llm_tokens_output fields - Not implemented
5. ❌ Fallback provider warning in receipts - Not implemented

**Roadmap**: Scheduled for V14 implementation
**Dependencies**: FR-LLM, FR-LLM-CLI, FR-LLM-API
**Priority**: Medium for provider auditing and debugging


## Conclusion

### Core Implementation (V1-V10): ✅ COMPLETE

All 19 core FR requirements (FR-RUN through FR-SCHEMA) are **fully implemented and verified**:

- ✅ **FR-RUN**: Runner with timeout enforcement, NDJSON parsing, ring buffers
- ✅ **FR-ORC**: Orchestrator with phase transitions, atomic operations
- ✅ **FR-PKT**: PacketBuilder with deterministic assembly, limits
- ✅ **FR-SEC**: SecretRedactor with pattern matching, global redaction
- ✅ **FR-FIX**: FixupEngine with path validation, atomic writes
- ✅ **FR-LOCK**: LockManager with stale detection, drift reporting
- ✅ **FR-JCS**: Canonicalizer with RFC 8785 compliance
- ✅ **FR-STA**: StatusManager with source attribution
- ✅ **FR-WSL**: WSL support with path translation
- ✅ **FR-EXIT**: Comprehensive error mapping to exit codes
- ✅ **FR-CFG**: Configuration discovery with precedence
- ✅ **FR-BENCH**: Performance benchmarking with process memory
- ✅ **FR-FS**: Atomic file operations with Windows retry
- ✅ **FR-OBS**: Structured logging with redaction
- ✅ **FR-CACHE**: InsightCache with BLAKE3 keys, TTL validation
- ✅ **FR-SOURCE**: Multi-source support (GitHub, filesystem, stdin)
- ✅ **FR-PHASE**: Trait-based phase system with separated concerns
- ✅ **FR-CLI**: Comprehensive CLI flags with documentation
- ✅ **FR-SCHEMA**: JSON schema compliance with CI enforcement

### Multi-Provider LLM (V11-V18): ⏳ ROADMAP

7 LLM-related FR requirements are **planned for future implementation**:

- ⏳ **FR-LLM**: LlmBackend trait abstraction (V11)
- ⏳ **FR-LLM-CLI**: CLI provider support (V11)
- ⏳ **FR-LLM-GEM**: Gemini CLI provider (V12)
- ⏳ **FR-LLM-API**: HTTP provider support (V13)
- ⏳ **FR-LLM-OR**: OpenRouter provider (V13)
- ⏳ **FR-LLM-ANTH**: Anthropic API provider (V14)
- ⏳ **FR-LLM-META**: Provider metadata in receipts (V14)

### Overall Status

**Core Implementation**: 19/19 requirements verified (100%)
**Total Requirements**: 19/26 requirements verified (73%)
**Remaining Work**: Multi-provider LLM backend (V11-V18 roadmap)

The xchecker runtime implementation is **production-ready** for Claude CLI usage. Multi-provider support is a planned enhancement that will enable Gemini CLI, OpenRouter, and Anthropic API as alternative or fallback providers.


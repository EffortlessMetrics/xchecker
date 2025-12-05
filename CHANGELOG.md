# Changelog

All notable changes to xchecker will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2025-11-30

### Added

#### Runtime Implementation - Complete Spec Generation Workflow

This release represents the completion of the xchecker runtime implementation, transforming xchecker from a validated CLI shell into a fully functional spec generation tool. All core components are implemented, tested, and operational across Linux, macOS, and Windows platforms.

##### Core Features

- **Phase-Based Workflow**: Complete implementation of Requirements → Design → Tasks → Review → Fixup → Final phase progression
  - Each phase produces structured artifacts (Markdown + YAML) with full traceability
  - Phase dependencies enforced by orchestrator (e.g., Design requires Requirements)
  - Resume capability from any completed phase
  - Atomic artifact promotion with `.partial/` staging

- **Runner System (FR-RUN)**: Process execution with timeout enforcement and cross-platform support
  - **Native mode**: Direct CLI binary execution on Linux, macOS, Windows
  - **WSL mode**: Windows Subsystem for Linux integration with path translation
  - **Auto mode**: Intelligent detection (native first, WSL fallback on Windows)
  - **Timeout enforcement**: Configurable phase timeout (default 600s, min 5s) with graceful TERM → KILL sequence
  - **Process tree termination**: Windows Job Objects and Unix process groups (killpg)
  - **NDJSON stream merging**: Parse stdout line-by-line, return last valid JSON frame
  - **Ring buffers**: Stdout 2 MiB, stderr 256 KiB (configurable via `--stdout-cap-bytes`, `--stderr-cap-bytes`)
  - **WSL distro capture**: Automatic detection from `wsl -l -q` or `$WSL_DISTRO_NAME`

- **Packet Builder (FR-PKT)**: Deterministic request payload assembly with size enforcement
  - **Priority-based selection**: Upstream > High > Medium > Low with LIFO ordering within classes
  - **Deterministic ordering**: Sorted file paths for reproducible packets
  - **Budget enforcement**: Configurable limits (default 65536 bytes, 1200 lines) with pre-invocation checks
  - **Overflow handling**: Exit code 7 with manifest written to `context/<phase>-packet.manifest.json`
  - **Packet preview**: Always written to `context/` directory for debugging
  - **Debug mode**: `--debug-packet` flag writes full packet after secret scan passes

- **Secret Redaction (FR-SEC)**: Comprehensive secret detection and redaction before external invocation
  - **Default patterns**: GitHub PAT, AWS keys, Slack tokens, Bearer tokens
  - **Custom patterns**: `--extra-secret-pattern` adds patterns, `--ignore-secret-pattern` suppresses
  - **Hard stop**: Exit code 8 on secret detection before Claude invocation
  - **Global redaction**: Applied to all human-readable strings (stderr, error_reason, warnings, context, logs)
  - **Receipt safety**: Never includes environment variables or raw packet content
  - **Path redaction**: Secrets in file paths redacted in receipts and logs

- **Fixup Engine (FR-FIX)**: Safe file modification with path validation and atomic operations
  - **Path validation**: Canonicalization, root boundary checking, `..` component rejection
  - **Symlink/hardlink protection**: Rejected by default unless `--allow-links` flag set
  - **Preview mode**: Default behavior shows intended changes without modifying files
  - **Apply mode**: `--apply-fixups` flag enables actual file modifications
  - **Atomic writes**: Temp file → fsync → `.bak` backup → atomic rename with Windows retry
  - **Permission preservation**: POSIX mode bits (Unix) and file attributes (Windows)
  - **Cross-filesystem fallback**: Copy → fsync → replace when rename crosses filesystem boundaries

- **Lock Manager (FR-LOCK)**: Concurrent execution prevention with stale lock detection
  - **Advisory locks**: JSON lock file with `{pid, host, started_at}` in spec root
  - **Stale detection**: PID not alive OR age > TTL (configurable, default 15 minutes)
  - **Force flag**: `--force` breaks stale locks with warning recorded in receipt
  - **Exit code 9**: Immediate failure when lock held by active process
  - **Lockfile drift**: Tracks model, CLI version, schema version changes
  - **Strict mode**: `--strict-lock` flag fails on drift before phase execution

- **Canonicalization (FR-JCS)**: RFC 8785-compliant JSON emission for deterministic output
  - **JCS emission**: All JSON outputs (receipts, status, doctor) use canonical serialization
  - **Byte-identical**: Re-serialization produces identical output for stable diffs
  - **Array sorting**: Artifacts by path, checks by name, outputs by path
  - **BLAKE3 hashing**: Full 64-character hex hashes computed on canonicalized content
  - **Cross-platform stability**: LF line endings enforced, CRLF tolerated on read

- **Status Manager (FR-STA)**: Comprehensive spec state reporting with source attribution
  - **Effective config**: Each setting includes `{value, source}` where source is `cli`, `config`, or `default`
  - **Artifact enumeration**: All artifacts with path and `blake3_first8` hash
  - **Lock drift reporting**: Detects changes in model, CLI version, schema version
  - **Pending fixups**: Optional summary with target count and estimated line changes
  - **Fresh spec support**: Works on specs with no prior receipts

- **InsightCache (FR-CACHE)**: BLAKE3-keyed cache for file summaries to avoid reprocessing
  - **Content hashing**: BLAKE3 hash of file content as cache key
  - **Two-tier caching**: Memory cache + disk persistence across runs
  - **File change detection**: Size and modification time validation
  - **Automatic invalidation**: Cache entries invalidated when files change
  - **Phase-specific insights**: 10-25 bullet points per phase (requirements, design, tasks, review)
  - **Cache statistics**: Hit/miss tracking with verbose logging
  - **Performance**: >70% hit rate on repeated runs, >50% speedup for large codebases

- **Source Resolver (FR-SOURCE)**: Multi-source support for problem statements
  - **GitHub sources**: Repository owner, name, and issue number resolution
  - **Filesystem sources**: File and directory reading with validation
  - **Stdin sources**: Standard input reading with non-empty validation
  - **User-friendly errors**: Actionable suggestions when source resolution fails
  - **Source metadata**: Tracks source type and origin for traceability

- **Phase Trait System (FR-PHASE)**: Trait-based phase implementation with separated concerns
  - **Phase trait**: Defines `id()`, `deps()`, `can_resume()`, `prompt()`, `make_packet()`, `postprocess()`
  - **Phase implementations**: RequirementsPhase, DesignPhase, TasksPhase
  - **Dependency enforcement**: Design requires Requirements, Tasks requires Design

## [Unreleased]

### Added

#### Phase Execution and LLM Backend Improvements - 2025-12-02

- **Unified Phase Execution Engine**: Introduced `execute_phase_core()` for consistent execution across all phases
  - Single execution pathway ensures uniform behavior for Requirements, Design, Tasks, Review, Fixup, and Final phases
  - Standardized packet building, LLM invocation, and artifact promotion logic
  - Reduces code duplication and improves maintainability

- **LLM Backend Abstraction**: Abstracted Claude CLI invocation behind config-driven provider selection
  - `llm_provider` configuration field controls which backend to use
  - V11-V14 schemas enforce "claude-cli" as the only supported provider
  - Future-proofing for additional LLM providers (OpenAI, Anthropic API, etc.)
  - Clean separation between orchestration logic and LLM invocation

- **ExecutionStrategy Configuration**: Introduced execution strategy concept for phase behavior control
  - V11-V14 schemas support only "controlled" strategy (legacy behavior)
  - Future versions will support additional strategies (autonomous, interactive, etc.)
  - Config validation ensures only supported strategies are used for a given schema version
  - Prevents runtime errors from unsupported configuration values

- **Doctor Command LLM Provider Validation**: Enhanced health checks for LLM backend verification
  - Validates Claude CLI discoverability on system PATH
  - Checks for `claude` binary availability before spec execution
  - Reports actionable errors when Claude CLI is not installed or inaccessible
  - Ensures environment is correctly configured for LLM invocation

- **Comprehensive Engine Invariant Tests**: Added test suite B3.7-B3.14 for execution engine validation
  - B3.7: Packet builder produces deterministic, correctly formatted packets
  - B3.8: Receipt metadata includes all required fields (phase, exit_code, emitted_at)
  - B3.9: Artifact promotion correctly moves files from .partial/ to final location
  - B3.10: ExecutionStrategy validation rejects unsupported values
  - B3.11: LLM provider validation rejects unsupported providers for schema version
  - B3.12: Phase dependency enforcement (Design requires Requirements, etc.)
  - B3.13: Resume functionality correctly skips completed phases
  - B3.14: Error handling produces correct exit codes and error_kind values

- **Config Validation for LLM Provider and ExecutionStrategy**: Added validation layer for configuration values
  - Schema version determines which LLM providers are supported
  - V11-V14: Only "claude-cli" provider allowed
  - Schema version determines which execution strategies are supported
  - V11-V14: Only "controlled" strategy allowed
  - Clear error messages guide users when invalid configurations are detected
  - Validation occurs at config load time, not runtime, for fail-fast behavior

#### Runtime Implementation - Complete Spec Generation Workflow

This release represents the completion of the xchecker runtime implementation, transforming xchecker from a validated CLI shell into a fully functional spec generation tool. All core components are implemented, tested, and operational across Linux, macOS, and Windows platforms.

##### Core Features

- **Phase-Based Workflow**: Complete implementation of Requirements → Design → Tasks → Review → Fixup → Final phase progression
  - Each phase produces structured artifacts (Markdown + YAML) with full traceability
  - Phase dependencies enforced by orchestrator (e.g., Design requires Requirements)
  - Resume capability from any completed phase
  - Atomic artifact promotion with `.partial/` staging

- **Runner System (FR-RUN)**: Process execution with timeout enforcement and cross-platform support
  - **Native mode**: Direct CLI binary execution on Linux, macOS, Windows
  - **WSL mode**: Windows Subsystem for Linux integration with path translation
  - **Auto mode**: Intelligent detection (native first, WSL fallback on Windows)
  - **Timeout enforcement**: Configurable phase timeout (default 600s, min 5s) with graceful TERM → KILL sequence
  - **Process tree termination**: Windows Job Objects and Unix process groups (killpg)
  - **NDJSON stream merging**: Parse stdout line-by-line, return last valid JSON frame
  - **Ring buffers**: Stdout 2 MiB, stderr 256 KiB (configurable via `--stdout-cap-bytes`, `--stderr-cap-bytes`)
  - **WSL distro capture**: Automatic detection from `wsl -l -q` or `$WSL_DISTRO_NAME`

- **Packet Builder (FR-PKT)**: Deterministic request payload assembly with size enforcement
  - **Priority-based selection**: Upstream > High > Medium > Low with LIFO ordering within classes
  - **Deterministic ordering**: Sorted file paths for reproducible packets
  - **Budget enforcement**: Configurable limits (default 65536 bytes, 1200 lines) with pre-invocation checks
  - **Overflow handling**: Exit code 7 with manifest written to `context/<phase>-packet.manifest.json`
  - **Packet preview**: Always written to `context/` directory for debugging
  - **Debug mode**: `--debug-packet` flag writes full packet after secret scan passes

- **Secret Redaction (FR-SEC)**: Comprehensive secret detection and redaction before external invocation
  - **Default patterns**: GitHub PAT, AWS keys, Slack tokens, Bearer tokens
  - **Custom patterns**: `--extra-secret-pattern` adds patterns, `--ignore-secret-pattern` suppresses
  - **Hard stop**: Exit code 8 on secret detection before Claude invocation
  - **Global redaction**: Applied to all human-readable strings (stderr, error_reason, warnings, context, logs)
  - **Receipt safety**: Never includes environment variables or raw packet content
  - **Path redaction**: Secrets in file paths redacted in receipts and logs

- **Fixup Engine (FR-FIX)**: Safe file modification with path validation and atomic operations
  - **Path validation**: Canonicalization, root boundary checking, `..` component rejection
  - **Symlink/hardlink protection**: Rejected by default unless `--allow-links` flag set
  - **Preview mode**: Default behavior shows intended changes without modifying files
  - **Apply mode**: `--apply-fixups` flag enables actual file modifications
  - **Atomic writes**: Temp file → fsync → `.bak` backup → atomic rename with Windows retry
  - **Permission preservation**: POSIX mode bits (Unix) and file attributes (Windows)
  - **Cross-filesystem fallback**: Copy → fsync → replace when rename crosses filesystem boundaries

- **Lock Manager (FR-LOCK)**: Concurrent execution prevention with stale lock detection
  - **Advisory locks**: JSON lock file with `{pid, host, started_at}` in spec root
  - **Stale detection**: PID not alive OR age > TTL (configurable, default 15 minutes)
  - **Force flag**: `--force` breaks stale locks with warning recorded in receipt
  - **Exit code 9**: Immediate failure when lock held by active process
  - **Lockfile drift**: Tracks model, CLI version, schema version changes
  - **Strict mode**: `--strict-lock` flag fails on drift before phase execution

- **Canonicalization (FR-JCS)**: RFC 8785-compliant JSON emission for deterministic output
  - **JCS emission**: All JSON outputs (receipts, status, doctor) use canonical serialization
  - **Byte-identical**: Re-serialization produces identical output for stable diffs
  - **Array sorting**: Artifacts by path, checks by name, outputs by path
  - **BLAKE3 hashing**: Full 64-character hex hashes computed on canonicalized content
  - **Cross-platform stability**: LF line endings enforced, CRLF tolerated on read

- **Status Manager (FR-STA)**: Comprehensive spec state reporting with source attribution
  - **Effective config**: Each setting includes `{value, source}` where source is `cli`, `config`, or `default`
  - **Artifact enumeration**: All artifacts with path and `blake3_first8` hash
  - **Lock drift reporting**: Detects changes in model, CLI version, schema version
  - **Pending fixups**: Optional summary with target count and estimated line changes
  - **Fresh spec support**: Works on specs with no prior receipts

- **InsightCache (FR-CACHE)**: BLAKE3-keyed cache for file summaries to avoid reprocessing
  - **Content hashing**: BLAKE3 hash of file content as cache key
  - **Two-tier caching**: Memory cache + disk persistence across runs
  - **File change detection**: Size and modification time validation
  - **Automatic invalidation**: Cache entries invalidated when files change
  - **Phase-specific insights**: 10-25 bullet points per phase (requirements, design, tasks, review)
  - **Cache statistics**: Hit/miss tracking with verbose logging
  - **Performance**: >70% hit rate on repeated runs, >50% speedup for large codebases

- **Source Resolver (FR-SOURCE)**: Multi-source support for problem statements
  - **GitHub sources**: Repository owner, name, and issue number resolution
  - **Filesystem sources**: File and directory reading with validation
  - **Stdin sources**: Standard input reading with non-empty validation
  - **User-friendly errors**: Actionable suggestions when source resolution fails
  - **Source metadata**: Tracks source type and origin for traceability

- **Phase Trait System (FR-PHASE)**: Trait-based phase implementation with separated concerns
  - **Phase trait**: Defines `id()`, `deps()`, `can_resume()`, `prompt()`, `make_packet()`, `postprocess()`
  - **Phase implementations**: RequirementsPhase, DesignPhase, TasksPhase
  - **Dependency enforcement**: Design requires Requirements, Tasks requires Design
  - **Artifact generation**: Both Markdown and core YAML artifacts per phase
  - **Deterministic**: `build_packet()` and `postprocess()` are deterministic for given inputs

##### CLI Flags

All CLI flags support configuration file overrides and have documented defaults:

- `--runner-mode <auto|native|wsl>`: Runner execution mode (default: auto)
- `--runner-distro <name>`: WSL distribution name (when runner_mode is wsl)
- `--phase-timeout <seconds>`: Phase timeout in seconds (default: 600, min: 5)
- `--packet-max-bytes <bytes>`: Maximum packet size in bytes (default: 65536)
- `--packet-max-lines <lines>`: Maximum packet lines (default: 1200)
- `--stdout-cap-bytes <bytes>`: Stdout ring buffer size (default: 2097152 = 2 MiB)
- `--stderr-cap-bytes <bytes>`: Stderr ring buffer size (default: 262144 = 256 KiB)
- `--lock-ttl-seconds <seconds>`: Lock TTL in seconds (default: 900 = 15 minutes)
- `--ignore-secret-pattern <regex>`: Suppress specific secret patterns
- `--extra-secret-pattern <regex>`: Add custom secret patterns
- `--debug-packet`: Write full packet to context/ after secret scan passes
- `--allow-links`: Allow symlinks and hardlinks in fixup targets
- `--strict-lock`: Hard fail on lockfile drift
- `--force`: Break stale locks
- `--apply-fixups`: Apply file changes (default is preview mode)
- `--verbose`: Enable structured logging with phase, spec_id, duration_ms, runner_mode

##### Receipt Fields

Receipts now include comprehensive metadata for auditability and debugging:

**Core fields** (always present):
- `schema_version`: Always "1" for v1 receipts
- `emitted_at`: RFC3339 UTC timestamp (replaces legacy `timestamp`)
- `canonicalization_backend`: Backend identifier (e.g., "jcs-rfc8785")
- `phase`: Phase identifier (requirements, design, tasks, review, fixup, final)
- `exit_code`: Process exit code (0 for success, non-zero for errors)

**Optional fields** (present when applicable):
- `stderr_redacted`: Stderr output truncated to 2048 bytes after redaction
- `runner`: Execution mode ("native" or "wsl")
- `runner_distro`: WSL distribution name (when runner is "wsl")
- `error_kind`: Error category (cli_args, packet_overflow, secret_detected, lock_held, phase_timeout, claude_failure, unknown)
- `error_reason`: Human-readable error description (redacted)
- `warnings`: Array of warning messages (e.g., stale lock broken, rename retries, permission preservation failures)
- `fallback_used`: Boolean flag for text format fallback (when NDJSON parsing fails)
- `diff_context`: Diff context for fixup phase
- `packet_evidence`: File list with blake3_pre_redaction hashes and priority levels

##### Status Fields

Status output includes comprehensive spec state information:

**Core fields**:
- `schema_version`: Always "1" for v1 status
- `emitted_at`: RFC3339 UTC timestamp
- `runner`: Execution mode ("native" or "wsl")
- `canonicalization_backend`: Backend identifier (e.g., "jcs-rfc8785")
- `artifacts`: Array of artifacts with path and `blake3_first8` (sorted by path)
- `effective_config`: Configuration with source attribution (cli/config/default)

**Optional fields**:
- `runner_distro`: WSL distribution name (when runner is "wsl")
- `fallback_used`: Boolean flag for text format fallback
- `canonicalization_version`: Canonicalization version string
- `lock_drift`: Drift detection for model, CLI version, schema version
- `pending_fixups`: Summary with target count and estimated line changes (counts only, no file contents)

##### Exit Codes

Standardized exit codes for automation and error handling:

- `0`: Success - operation completed successfully
- `1`: Unknown error - unexpected failure
- `2`: CLI arguments invalid - malformed command or missing required arguments
- `7`: Packet overflow - packet exceeds size limits before Claude invocation
- `8`: Secret detected - secret pattern matched, hard stop before external invocation
- `9`: Lock already held - another process holds the lock
- `10`: Phase timeout - operation exceeded configured timeout
- `70`: Claude CLI failure - Claude invocation failed (network, auth, quota, etc.)

##### Configuration System (FR-CFG)

- **Upward discovery**: Search from CWD for `.xchecker/config.toml`, stop at filesystem root or `.git`
- **Precedence**: CLI flags > config file > defaults
- **Source attribution**: Each config value tracks its source (cli/config/default)
- **Explicit path**: `--config <path>` overrides discovery
- **XCHECKER_HOME**: Environment variable overrides state directory location

##### Cross-Platform Support (NFR3)

- **Linux**: Full support with native runner and Unix process groups
- **macOS**: Full support with native runner and Unix process groups
- **Windows**: Full support with native runner and Job Objects for process tree termination
- **WSL**: Windows Subsystem for Linux integration with path translation and environment adaptation
- **Line endings**: LF enforcement on write, CRLF tolerance on read (Windows)

##### Performance (NFR1)

Benchmark results (Windows, Release build, 2024-11-23):
- **Empty run**: 16.8ms median (Target: ≤ 5000ms) - ✅ 99.7% under target
- **Packetization (100 files)**: 10ms median (Target: ≤ 200ms) - ✅ 95% under target
- **JCS emission**: <50ms (Target: ≤ 50ms) - ✅ Target met
- **Process memory**: RSS 18.9MB, Commit 11.5MB (Windows)

##### Security (NFR2)

- **No secrets on disk**: Except under explicit `--debug-packet` after successful scan
- **Global redaction**: Applied before persistence to all human-readable strings
- **Path validation**: Prevents traversal, rejects symlinks/hardlinks by default
- **API keys**: Never logged or persisted
- **Receipt safety**: No environment variables or raw packet content

##### Observability (NFR4)

- **Structured logging**: `--verbose` provides phase, spec_id, duration_ms, runner_mode
- **Secret-free logs**: Redaction applied before any output
- **Actionable errors**: All errors include suggestions for resolution
- **Traceability**: Full audit trail via receipts and artifacts

#### Performance Benchmarks (NFR1 Validation)

**Benchmark Results** (Windows, Release build, 2024-11-23):
- **Empty Run Performance**: 16.8ms median (Target: ≤ 5000ms) - ✅ **99.7% under target**
- **Packetization (100 files)**: 10ms median (Target: ≤ 200ms) - ✅ **95% under target**
- **Process Memory**: RSS 18.9MB, Commit 11.5MB (Windows)
- **Overall Status**: ✅ All NFR1 performance targets met

The benchmark suite validates that xchecker meets its non-functional requirements:
- Baseline operations complete in milliseconds, not seconds
- File packetization is highly efficient even with 100+ files
- Memory footprint remains minimal during operations
- All measurements use median of 4 runs (excluding 1 warm-up pass)

Run benchmarks yourself: `xchecker benchmark --verbose`

#### JSON Schema Versioning and Contracts (v1)

- **Receipt Schema v1** (`schemas/receipt.v1.json`): Versioned receipt format with comprehensive field definitions
  - `schema_version`: Always "1" for this version
  - `emitted_at`: RFC3339 UTC timestamp replacing legacy `timestamp` field
  - `error_kind` and `error_reason`: Structured error reporting for non-zero exits
  - `runner`: Enum ["native", "wsl"] for execution mode tracking
  - `runner_distro`: Optional WSL distribution name
  - `canonicalization_backend`: Backend identifier (e.g., "jcs-rfc8785")
  - `fallback_used`: Boolean flag for text format fallback
  - All receipts emitted using JCS (RFC 8785) for canonical JSON with stable diffs
  - `outputs` array sorted by path before emission for deterministic ordering
  - `stderr_tail` limited to 2048 characters maximum
  - `blake3_first8` pattern: `^[0-9a-f]{8}$` for hash validation

- **Status Schema v1** (`schemas/status.v1.json`): Versioned status output format
  - `schema_version`: Always "1" for this version
  - `emitted_at`: RFC3339 UTC timestamp
  - `runner` and `runner_distro`: Execution mode information
  - `fallback_used`: Boolean flag for text format fallback
  - `canonicalization_version` and `canonicalization_backend`: Canonicalization tracking
  - `artifacts`: Array of artifacts with path and `blake3_first8` (sorted by path)
  - `last_receipt_path`: Path to most recent receipt
  - `effective_config`: Configuration with source attribution (cli/config/default)
  - `lock_drift`: Optional drift detection for model, CLI version, and schema version
  - All status outputs emitted using JCS (RFC 8785) for canonical JSON

- **Doctor Schema v1** (`schemas/doctor.v1.json`): Versioned health check output format
  - `schema_version`: Always "1" for this version
  - `emitted_at`: RFC3339 UTC timestamp
  - `ok`: Boolean overall health status
  - `checks`: Array of health checks (sorted by name) with status enum ["pass", "warn", "fail"]
  - All doctor outputs emitted using JCS (RFC 8785) for canonical JSON

- **Exit Codes**: Standardized exit codes for automation
  - `0`: Success
  - `2`: CLI arguments invalid
  - `7`: Packet overflow (pre-Claude)
  - `8`: Secret detected (redaction hard stop)
  - `9`: Lock already held
  - `10`: Phase timeout
  - `70`: Claude CLI invocation failure

- **Example Payloads**: Minimal and full examples for all schemas in `docs/schemas/`
  - `receipt.v1.minimal.json` and `receipt.v1.full.json`
  - `status.v1.minimal.json` and `status.v1.full.json`
  - `doctor.v1.minimal.json` and `doctor.v1.full.json`

### Changed

- **[BREAKING] Receipt Format**: Replaced `timestamp` field with `emitted_at` (RFC3339 UTC)
  - **Migration**: Update code to read `emitted_at` instead of `timestamp`
  - Old receipts with `timestamp` will continue to deserialize but should be regenerated
  - Schema v1 maintains backward compatibility via `additionalProperties: true`

- **Runner Architecture**: Refactored to support multiple execution modes (native, WSL, auto)
  - No breaking changes to public API
  - Internal runner implementation now supports cross-platform execution

- **Packet Assembly**: Enhanced with priority-based selection and deterministic ordering
  - No breaking changes to packet format
  - Improved token efficiency with InsightCache integration

- **Error Handling**: Comprehensive error mapping to standardized exit codes
  - All errors now include `error_kind` and `error_reason` in receipts
  - Actionable suggestions provided for all error scenarios

### Deprecated

- **Legacy `timestamp` field**: Use `emitted_at` instead (RFC3339 UTC format)
  - Will be removed in schema v2 (no earlier than 6 months after v2 release)
  - Current receipts include both fields for backward compatibility

### Migration Guide

#### Updating from Pre-Runtime Implementation

If you were using xchecker before the runtime implementation:

1. **Receipt Parsing**: Update code to read `emitted_at` instead of `timestamp`
   ```rust
   // Old
   let timestamp = receipt.timestamp;
   
   // New
   let emitted_at = receipt.emitted_at;
   ```

2. **Exit Code Handling**: Update automation to handle new exit codes
   - Exit code 7: Packet overflow (pre-Claude)
   - Exit code 8: Secret detected
   - Exit code 9: Lock held
   - Exit code 10: Phase timeout
   - Exit code 70: Claude failure

3. **Configuration**: Update config files to use new structure
   ```toml
   # Old (if you had custom config)
   [runner]
   mode = "native"
   
   # New (same structure, more options)
   [runner]
   mode = "auto"  # or "native" or "wsl"
   phase_timeout = 600
   
   [packet]
   max_bytes = 65536
   max_lines = 1200
   
   [secrets]
   extra_patterns = ["CUSTOM_.*_KEY"]
   ignore_patterns = []
   ```

4. **CLI Flags**: Update scripts to use new flag names
   - All flags now have consistent naming (e.g., `--phase-timeout` instead of `--timeout`)
   - Use `--verbose` for structured logging instead of debug output

5. **Receipts**: Regenerate old receipts to get new fields
   ```bash
   # Clean and regenerate spec
   xchecker clean <spec-id>
   xchecker spec <spec-id> --source stdin < problem.txt
   ```

6. **Lockfiles**: Create lockfiles for reproducibility
   ```bash
   xchecker init <spec-id> --create-lock
   ```

#### Breaking Changes Summary

- **Receipt schema**: `timestamp` → `emitted_at` (backward compatible via `additionalProperties: true`)
- **Exit codes**: New standardized exit codes (0, 1, 2, 7, 8, 9, 10, 70)
- **Configuration**: New structure with more options (old configs still work with defaults)

All breaking changes maintain backward compatibility where possible. Old receipts will continue to deserialize, but new features require regeneration.

### Fixed

- **Windows Rename Retry**: Atomic file operations now retry on Windows with bounded exponential backoff (≤ 250ms total) to handle antivirus interference
- **Cross-Filesystem Fallback**: File operations correctly fall back to copy → fsync → replace when rename crosses filesystem boundaries
- **WSL Path Translation**: Windows paths correctly translated to WSL format using `wslpath -a` with fallback to `/mnt/<drive>` heuristic
- **Process Tree Termination**: Timeout handling now correctly terminates child processes via Windows Job Objects and Unix process groups
- **NDJSON Parsing**: Stdout parsing correctly handles interleaved noise and multiple JSON frames, returning last valid frame
- **Secret Redaction**: Secrets in file paths now correctly redacted in receipts and logs
- **Lock Stale Detection**: Lock manager correctly detects stale locks via PID liveness check and age comparison
- **Line Ending Normalization**: Diff estimates correctly normalize line endings before calculation
- **Permission Preservation**: File mode bits (Unix) and attributes (Windows) correctly preserved during fixup apply

### Performance

- **Packet Assembly**: Optimized file reading and priority sorting for 95% improvement (10ms for 100 files)
- **JCS Emission**: Optimized JSON canonicalization for <50ms emission time
- **InsightCache**: Reduces packet assembly time by >50% for large codebases with >70% hit rate
- **Ring Buffers**: Efficient stdout/stderr capture with configurable caps (2 MiB / 256 KiB)
- **BLAKE3 Hashing**: Fast content hashing for cache keys and artifact verification

### Documentation

- **README**: Comprehensive documentation of all commands, flags, and exit codes
- **CONFIGURATION.md**: Detailed configuration guide with examples
- **CONTRACTS.md**: JSON schema versioning policy and compatibility guarantees
- **DOCTOR.md**: Health check documentation with remediation steps
- **TRACEABILITY.md**: Requirements traceability matrix
- **Schema Examples**: Minimal and full examples for all schemas (receipt, status, doctor)
- **Code Examples**: All examples validated in CI for correctness
- **Platform Notes**: Windows, WSL, Linux, macOS specific documentation

### Testing

- **Unit Tests**: Comprehensive unit test coverage for all modules (>90% coverage)
- **Integration Tests**: End-to-end workflow tests for all phases
- **Platform Tests**: Windows-specific (WSL, Job Objects) and Unix-specific (killpg) tests
- **Property-Based Tests**: Randomized testing for canonicalization, hashing, and packet assembly
- **Smoke Tests**: CI smoke tests for all commands (doctor, init, spec, status, clean, benchmark)
- **Schema Validation**: All emitted JSON validated against v1 schemas in CI
- **Documentation Tests**: All code examples and CLI flags validated in CI

### Internal

- **Module Organization**: Canonicalization in `canonicalization.rs`, secret redaction in `redaction.rs`
- **Error Taxonomy**: Comprehensive error types with exit code mapping
- **Atomic Operations**: All writes use temp → fsync → atomic rename pattern
- **Logging Infrastructure**: Structured logging with tracing crate
- **Test Infrastructure**: Integration test framework with golden file support
- **CI Pipeline**: Multi-platform testing (Linux, macOS, Windows) with schema validation

## JSON Schema Deprecation Policy

XChecker follows a strict deprecation policy for JSON schemas to ensure API stability:

### Versioning Rules

1. **Additive Changes** (no version bump required):
   - New optional fields may be added to existing schemas
   - New enum values may be added to existing enums
   - `additionalProperties: true` allows forward compatibility

2. **Breaking Changes** (require schema version bump):
   - Removing fields
   - Renaming fields
   - Changing field types
   - Making optional fields required
   - Removing enum values
   - Changing validation constraints (patterns, min/max, etc.)

3. **Schema Version Lifecycle**:
   - v1 will remain stable with no breaking changes
   - When breaking changes are needed, v2 will be introduced
   - v1 support will be maintained for **at least 6 months** after v2 release
   - Deprecation warnings will be added to v1 outputs during the transition period
   - After the 6-month window, v1 may be removed in a major version bump

### Migration Path

When a new schema version is introduced:

1. **Announcement**: Release notes will document all breaking changes
2. **Dual Support**: Both old and new schema versions will be supported during transition
3. **Deprecation Warnings**: Old schema outputs will include deprecation notices
4. **Migration Guide**: Documentation will provide step-by-step migration instructions
5. **Removal**: Old schema version removed after minimum 6-month support window

### Compatibility Guarantees

- **Forward Compatibility**: Consumers should ignore unknown fields (`additionalProperties: true`)
- **Backward Compatibility**: Producers may emit additional fields without version bump
- **Canonical Emission**: All JSON outputs use JCS (RFC 8785) for stable diffs and deterministic ordering
- **Array Ordering**: Arrays are sorted before emission (outputs by path, artifacts by path, checks by name)

### Schema Validation

All schemas are validated in CI:
- JSON Schema validation against `schemas/*.v1.json`
- Snapshot tests for canonical emission and stable ordering
- Example payloads in `docs/schemas/` must pass validation

For questions about schema compatibility, please open an issue on GitHub.


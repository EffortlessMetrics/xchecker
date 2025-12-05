# Requirements Document

## Introduction

**Status: CORE IMPLEMENTATION COMPLETE → MULTI-PROVIDER LLM ROADMAP (V11–V18)**

This spec originally covered the runtime implementation work to move xchecker from a validated CLI shell to a fully functional tool. **The core implementation is now complete.** This document now serves as the requirements baseline for the next phase: multi-provider LLM backend support and ecosystem expansion.

**Current State (V1–V10):**
- ✅ All core components implemented (Runner, Orchestrator, PacketBuilder, SecretRedactor, FixupEngine, LockManager)
- ✅ End-to-end phase execution working
- ✅ Cross-platform support (Linux, macOS, Windows, WSL)
- ✅ Comprehensive test coverage
- ✅ Verification, edge cases, optimization, and polish complete

**Next Phase (V11–V18): Multi-Provider LLM & Ecosystem**
- **V11**: LLM Core Skeleton & Claude Backend (MVP+) — Controlled-only execution, LlmBackend abstraction
- **V12**: Gemini CLI as First-Class Provider — Gemini as default, Claude optional
- **V13**: HTTP Client & OpenRouter Backend — Cost-gated optional HTTP provider
- **V14**: Anthropic HTTP & Rich Metadata — Full multi-provider matrix with docs
- **V15**: Claude Code (Claude Code) Integration & UX — IDE integration, slash commands
- **V16**: Workspace & Multi-Spec Orchestration — Project-level status, history, TUI
- **V17**: Policy & Enforcement ("Double-Entry SDLC" in CI) — Gates, branch protection integration
- **V18**: Ecosystem & Templates — Batteries-included flows, plugin hooks, showcase examples

**Implementation Notes:**
- JCS emission implemented in `canonicalization.rs` (not separate `jcs.rs`)
- Secret scanning implemented as `SecretRedactor` in `redaction.rs`
- Additional features beyond original plan: InsightCache, source resolution, integration test framework
- LLM backend abstraction will replace direct Runner usage in orchestrator

## Glossary

- **xchecker**: The Rust CLI tool for orchestrating spec generation workflows
- **Runner**: Process launcher for Claude CLI (native or WSL modes)
- **LLM Backend**: Abstraction for invoking language models via CLI or HTTP
- **Transport**: Method of communicating with an LLM (CLI binary or HTTP API)
- **Provider**: Specific LLM service (Gemini CLI, Claude CLI, OpenRouter, Anthropic API)
- **Orchestrator**: Component that enforces phase order and coordinates execution
- **Packet**: Deterministically assembled request payload with enforced size limits
- **Receipt**: Structured per-phase result JSON (v1 schema) with JCS canonicalization
- **Fixup**: Proposed file changes from review phase, applied via unified diffs
- **JCS**: JSON Canonicalization Scheme (RFC 8785) - deterministic JSON serialization
- **Phase**: One step in the workflow (requirements, design, tasks, review, fixup, final)
- **Spec Root**: `XCHECKER_HOME/specs/<spec-id>` directory structure
- **InsightCache**: BLAKE3-keyed cache for file summaries to avoid reprocessing unchanged files
- **SourceResolver**: Component that resolves different input types (GitHub, filesystem, stdin)
- **SecretRedactor**: Component that detects and redacts secrets before persistence or external invocation
- **Canonicalizer**: Single choke point for RFC 8785-compliant JSON canonicalization

## Requirements

### Requirement 1 (FR-RUN)

**User Story:** As a developer, I want the Runner to execute Claude CLI with proper timeout enforcement, so that hung processes don't block indefinitely and I get reliable results.

#### Acceptance Criteria

1. WHEN the Runner executes in native mode THEN the system SHALL spawn the Claude CLI process directly with the configured arguments
2. WHEN the Runner executes in WSL mode on Windows THEN the system SHALL translate paths and environment variables to WSL format and use `wsl.exe --exec`
3. WHEN the Runner executes in auto mode THEN the system SHALL detect Claude in the native PATH first, otherwise fall back to WSL on Windows
4. WHEN a phase timeout is configured THEN the Runner SHALL enforce the wall-clock timeout (default 600s, minimum 5s) and kill the process if exceeded
5. WHEN a timeout occurs THEN the Runner SHALL first send graceful termination (TERM) and wait up to 5 seconds, then force kill (KILL)
6. WHEN a timeout occurs on Windows THEN the Runner SHALL assign the process to a Job Object and terminate the job to ensure child processes are killed
7. WHEN a timeout occurs THEN the system SHALL exit with code 10 and write a receipt with `phase_timeout` error kind and `stderr_redacted` field
8. WHEN the Runner captures stdout THEN the system SHALL treat it as NDJSON where each line is a JSON object
9. WHEN stdout contains non-JSON or malformed lines THEN the system SHALL ignore those lines; IF ≥1 valid JSON object is read THEN the system SHALL return the **last valid JSON object**; OTHERWISE the system SHALL return `claude_failure` with a **redacted** tail excerpt (≤ 256 chars before redaction)
10. WHEN the Runner captures stderr THEN the system SHALL redact secrets and truncate to 2048 bytes maximum in the `stderr_redacted` field before persisting in receipts
11. The Runner SHALL enforce ring-buffer caps with defaults: `stdout_cap_bytes = 2 MiB`, `stderr_cap_bytes = 256 KiB`; both SHALL be configurable via `--stdout-cap-bytes` and `--stderr-cap-bytes`

### Requirement 2 (FR-ORC)

**User Story:** As a developer, I want the Orchestrator to enforce legal phase transitions and coordinate all execution steps, so that the workflow remains consistent and auditable.

#### Acceptance Criteria

1. WHEN the Orchestrator executes a phase THEN the system SHALL validate the transition is legal based on the current state
2. WHEN an illegal transition is requested THEN the system SHALL exit with code 2 and provide actionable guidance on valid next steps
3. WHEN the Orchestrator executes a phase THEN the system SHALL acquire an exclusive lock, build the packet, scan for secrets, enforce limits, invoke the Runner, and write artifacts atomically
4. WHEN a phase completes successfully THEN the system SHALL write partial artifacts to a temporary location first, then promote to final names atomically
5. WHEN a phase fails THEN the system SHALL write an error receipt with JCS canonicalization including exit_code, error_kind, and error_reason
6. WHEN the Orchestrator completes a phase THEN the system SHALL emit a JCS-canonical receipt for both success and failure cases
7. WHEN any phase starts THEN the system SHALL remove stale `.partial/` directories (best effort) before staging new partial artifacts, and the completed phase SHALL be defined by the last **successful** receipt for `<spec-id>`

### Requirement 3 (FR-PKT)

**User Story:** As a developer, I want the PacketBuilder to assemble inputs deterministically with enforced limits, so that I can prevent oversized requests and maintain reproducibility.

#### Acceptance Criteria

1. WHEN the PacketBuilder assembles a packet THEN the system SHALL collect inputs in deterministic order and count bytes and lines
2. WHEN the packet exceeds `packet_max_bytes` (default 65536) THEN the system SHALL exit with code 7 before invoking Claude
3. WHEN the packet exceeds `packet_max_lines` (default 1200) THEN the system SHALL exit with code 7 before invoking Claude
4. WHEN a packet overflow occurs THEN the system SHALL write a receipt including the actual size and configured limits
5. WHEN a packet overflow occurs THEN the system SHALL write a sanitized packet manifest to `context/<phase>-packet.manifest.json` containing only sizes, counts, and file paths (no payload content)
6. WHEN `--debug-packet` is provided AND the SecretScanner passes THEN the system MAY write the full packet to `context/<phase>-packet.txt`
7. WHEN `--debug-packet` writes a full packet THEN this file MUST be excluded from receipts, MUST be redacted if later reported, and MUST NOT be written if any secret rule fires

### Requirement 4 (FR-SEC)

**User Story:** As a security engineer, I want the SecretScanner to detect and block secrets before they reach Claude or get persisted, so that sensitive information never leaks.

#### Acceptance Criteria

1. WHEN the SecretScanner scans a packet THEN the system SHALL check against default patterns: `ghp_[A-Za-z0-9]{36}`, `AKIA[0-9A-Z]{16}`, `AWS_SECRET_ACCESS_KEY=`, `xox[baprs]-`, `Bearer [A-Za-z0-9._-]{20,}`
2. WHEN a secret pattern matches THEN the system SHALL exit with code 8 and report which pattern matched without including the actual secret
3. WHEN `--ignore-secret-pattern <regex>` is provided THEN the system SHALL skip that pattern during scanning
4. WHEN `--extra-secret-pattern <regex>` is provided THEN the system SHALL add that pattern to the scan list
5. WHEN the SecretScanner redacts stderr THEN the system SHALL replace matched substrings with `***` before persisting
6. WHEN receipts are written THEN the system SHALL never include environment variables or raw packet content
7. Global redaction SHALL be applied to all human-readable strings (stderr, error_reason, warnings, context lines, doctor/status text, previews) before persistence or logging

### Requirement 5 (FR-FIX)

**User Story:** As a developer, I want the FixupEngine to safely preview and apply file changes, so that I can review proposed changes before committing them and ensure no path traversal vulnerabilities.

#### Acceptance Criteria

1. WHEN the FixupEngine validates a plan THEN the system SHALL canonicalize all paths and ensure they are under the allowed root
2. WHEN a fixup target contains `..` components or is an absolute path outside the root THEN the system SHALL reject it with an explanatory error
3. WHEN the FixupEngine validates a plan THEN the system SHALL reject targets that are symlinks or hardlinks unless `--allow-links` is explicitly set
4. WHEN fixups run in preview mode (default) THEN the system SHALL show intended targets, estimated added/removed lines, and validation warnings without modifying files
5. WHEN fixups run with `--apply-fixups` THEN the system SHALL write to temporary files, fsync, create `.bak` backups, and atomically rename with retry on Windows
6. WHEN applying fixups THEN file mode bits (Unix) or attributes (Windows) SHALL be preserved
7. WHEN atomic rename must cross filesystem boundaries THEN the system SHALL fall back to copy+fsync+rename on the same volume
8. WHEN fixups are applied THEN the system SHALL record applied files in the receipt with their blake3_first8 hashes and `applied: true`
9. WHEN fixups are in preview mode THEN the receipt SHALL include targets with `applied: false` and no file system modifications
10. WHEN computing diff estimates THEN line endings SHALL be normalized before calculation

### Requirement 6 (FR-LOCK)

**User Story:** As a developer, I want the LockManager to prevent concurrent runs and detect lockfile drift, so that I can avoid race conditions and track reproducibility.

#### Acceptance Criteria

1. WHEN the LockManager acquires a lock THEN the system SHALL create an advisory lock file in the spec root containing `{pid, host, started_at}`
2. WHEN a lock is already held by another process THEN the system SHALL exit with code 9 immediately
3. WHEN evaluating a lock THEN the system SHALL consider it stale if the PID is not alive on the same host OR the lock age exceeds a configurable TTL (default 15 minutes)
4. WHEN `--force` is provided and a stale lock exists THEN the system MAY break the lock and record the action as a warning in the next receipt
5. WHEN a lock is released THEN the system SHALL remove the lock file on normal exit and best-effort on panic
6. WHEN `xchecker init --create-lock` runs THEN the system SHALL create a lockfile recording `model_full_name`, `claude_cli_version`, and `schema_version`
7. WHEN a lockfile exists and values differ from current THEN the system SHALL compute drift and include it in status output
8. WHEN `--strict-lock` is provided and drift exists THEN the system SHALL exit non-zero before executing any phase

### Requirement 7 (FR-JCS)

**User Story:** As a developer, I want JCS emission for all JSON outputs, so that receipts and status are deterministic and produce stable diffs.

#### Acceptance Criteria

1. WHEN the system writes receipts THEN the system SHALL use an RFC 8785-compliant JCS canonicalization where key ordering alone (e.g., BTreeMap) is insufficient and numeric and string normalization MUST follow RFC 8785
2. WHEN the system writes status output THEN the system SHALL use JCS canonicalization with sorted arrays (artifacts by path)
3. WHEN receipts are written THEN they SHALL include `schema_version: "1"`, `emitted_at` (RFC3339 UTC), `canonicalization_backend: "jcs-rfc8785"`, exit_code, and phase metadata
4. WHEN receipts are re-serialized THEN they SHALL produce byte-identical output (ordering stable)
5. WHEN the system computes blake3_first8 THEN the system SHALL use lowercase hex and exactly 8 characters
6. WHEN computing blake3_first8 for artifacts THEN the system SHALL compute on on-disk bytes after write (JSON is LF-terminated) so it is stable across platforms

### Requirement 8 (FR-STA)

**User Story:** As a developer, I want the StatusManager to report effective configuration with source attribution, so that I can understand which settings are active and where they came from.

#### Acceptance Criteria

1. WHEN `xchecker status <spec-id> --json` runs THEN the system SHALL emit JCS JSON with artifacts, effective_config, and lock_drift
2. WHEN status reports effective_config THEN each setting SHALL include `{value, source}` where source is exactly `cli`, `config`, or `default`
3. WHEN status enumerates artifacts THEN each SHALL include path and blake3_first8
4. WHEN status runs on a fresh spec with no prior receipts THEN the system SHALL emit sensible defaults without errors
5. WHEN status runs with a lockfile present THEN the system SHALL report drift fields (model_full_name, claude_cli_version, schema_version) if they differ
6. Status MAY include `"pending_fixups": { "targets": <u32>, "est_added": <u32>, "est_removed": <u32> }`; omit when unavailable

### Requirement 9 (FR-WSL)

**User Story:** As a Windows developer, I want WSL detection and path translation, so that I can use xchecker seamlessly when Claude is only available in WSL.

#### Acceptance Criteria

1. WHEN the system detects WSL availability THEN the system SHALL query `wsl.exe -l -q` to verify at least one installed distribution
2. WHEN validating WSL readiness THEN the system SHALL also attempt `wsl.exe -d <distro> -- which claude` to confirm Claude availability inside WSL
3. WHEN Claude is not discoverable in WSL THEN `xchecker doctor` SHALL report remediation steps and Runner in auto mode SHALL prefer native
4. WHEN the Runner translates Windows paths to WSL THEN the system SHALL convert `C:\` to `/mnt/c/` format
5. WHEN the Runner translates environment variables for WSL THEN the system SHALL preserve necessary context while adapting paths
6. WHEN `xchecker doctor` runs on Windows THEN the system SHALL report native Claude availability and WSL detection status with actionable suggestions
7. WHEN WSL execution is used THEN receipts SHALL include `runner: "wsl"` and `runner_distro` if applicable
8. WHEN translating Windows paths to WSL THEN the system SHOULD use `wsl.exe wslpath -a <winpath>`, falling back to `/mnt/<drive-letter>/<rest>` if unavailable
9. WHEN invoking `wsl.exe --exec` THEN arguments MUST be passed as discrete argv elements (no shell concatenation)

### Requirement 10 (FR-EXIT)

**User Story:** As a developer, I want comprehensive error mapping to standardized exit codes, so that automation can distinguish failure modes reliably.

#### Acceptance Criteria

1. WHEN the system exits successfully THEN the system SHALL use exit code 0
2. WHEN CLI arguments are invalid THEN the system SHALL use exit code 2
3. WHEN packet overflow occurs THEN the system SHALL use exit code 7
4. WHEN a secret is detected THEN the system SHALL use exit code 8
5. WHEN a lock is already held THEN the system SHALL use exit code 9
6. WHEN a phase timeout occurs THEN the system SHALL use exit code 10
7. WHEN the Claude invocation fails THEN the system SHALL use exit code 70
8. WHEN any error occurs THEN the receipt SHALL include error_kind (one of: cli_args, packet_overflow, secret_detected, lock_held, phase_timeout, claude_failure, unknown) and error_reason
9. WHEN the exit_code is written to a receipt THEN the system SHALL ensure it matches the actual process exit code

### Requirement 11 (FR-CFG)

**User Story:** As a developer, I want configuration discovery with precedence rules, so that I can set project defaults without repeating CLI flags.

#### Acceptance Criteria

1. WHEN the system loads configuration THEN the system SHALL search upward from CWD for `.xchecker/config.toml` stopping at filesystem root or VCS boundary (.git)
2. WHEN configuration is loaded THEN the system SHALL apply precedence: CLI flags override config file values which override built-in defaults
3. WHEN `--config <path>` is provided THEN the system SHALL use that explicit path instead of discovery
4. WHEN configuration includes `[runner]` section THEN the system SHALL use those values for runner_mode, distro, and phase_timeout
5. WHEN `XCHECKER_HOME` environment variable is set THEN the system SHALL use that location for state directory

### Requirement 12 (FR-BENCH)

**User Story:** As a developer, I want the benchmark command to measure performance with process-scoped memory, so that I can verify NFRs are met.

#### Acceptance Criteria

1. WHEN `xchecker benchmark` runs THEN the system SHALL generate deterministic workloads and measure wall time and memory
2. WHEN benchmarks execute THEN the system SHALL run one warm-up pass and N>=3 measured runs, reporting median timings
3. WHEN benchmarks measure memory THEN the system SHALL report process RSS (all OSs) and commit_mb (Windows only), not system totals
4. WHEN benchmarks complete THEN the system SHALL emit structured JSON with `ok` boolean, `timings_ms`, and `memory_bytes`
5. WHEN benchmarks check thresholds THEN the system SHALL compare against the median and use configurable limits via CLI or config file
6. WHEN benchmarks fail thresholds THEN the system SHALL set `ok: false` and provide clear messaging

### Requirement 13 (FR-FS)

**User Story:** As a developer, I want atomic file operations with Windows retry logic, so that writes are safe even with antivirus interference.

#### Acceptance Criteria

1. WHEN the system writes artifacts THEN the system SHALL write to a temporary file first, fsync, then atomically rename
2. WHEN atomic rename fails on Windows THEN the system SHALL retry with bounded exponential backoff (≤ 250ms total)
3. WHEN Windows rename retry occurs THEN the system SHALL add `rename_retry_count` to the warnings array in the receipt
4. WHEN all JSON files are written THEN the system SHALL use UTF-8 encoding with LF line endings
5. WHEN the system reads files on Windows THEN the system SHALL tolerate CRLF line endings

### Requirement 14 (FR-OBS)

**User Story:** As a developer, I want structured observability with secret redaction, so that I can debug issues without leaking sensitive information.

#### Acceptance Criteria

1. WHEN `--verbose` is provided THEN the system SHALL emit structured logs including spec_id, phase, duration_ms, and runner_mode
2. WHEN logs are emitted THEN the system SHALL never include secrets and SHALL apply redaction before logging
3. WHEN errors occur THEN logs SHALL include actionable context without exposing sensitive data

### Requirement 15 (FR-CACHE)

**User Story:** As a developer, I want insight caching with TTL and validation, so that unchanged files are not reprocessed and performance is optimized.

#### Acceptance Criteria

1. WHEN the InsightCache processes a file THEN the system SHALL compute a BLAKE3 hash of the file content and use it as a cache key
2. WHEN cached insights exist for a file THEN the system SHALL validate the file has not changed by comparing size and modification time
3. WHEN a file has changed THEN the system SHALL invalidate the cached insights and regenerate them
4. WHEN generating insights THEN the system SHALL produce 10-25 bullet points per phase as per the caching strategy
5. WHEN insights are cached THEN the system SHALL store them both in memory and on disk for persistence across runs
6. WHEN the cache is queried THEN the system SHALL track hit/miss statistics and report them in verbose logging
7. WHEN cache files are corrupted THEN the system SHALL remove them and regenerate insights
8. Cache TTL SHALL be configurable; expired entries SHALL be treated as misses without blocking phase execution (fail-open)
9. Cache writes SHALL follow temp → fsync → atomic rename and SHALL pass strings through redaction before persistence

### Requirement 16 (FR-SOURCE)

**User Story:** As a developer, I want multi-source support for problem statements, so that I can initiate specs from GitHub issues, filesystem files, or stdin.

#### Acceptance Criteria

1. WHEN a GitHub source is specified THEN the system SHALL resolve the repository owner, name, and issue number
2. WHEN a filesystem source is specified THEN the system SHALL read the file or directory and validate it exists
3. WHEN stdin source is specified THEN the system SHALL read from standard input and validate it is not empty
4. WHEN source resolution fails THEN the system SHALL provide user-friendly error messages with actionable suggestions
5. WHEN a source is resolved THEN the system SHALL include metadata about the source type and origin
6. WHEN invalid source configuration is provided THEN the system SHALL exit with code 2 and provide guidance on valid options
7. The resolver SHALL deduplicate paths across overlapping patterns and apply exclude globs before include priority
8. The resolver SHALL enforce caps on **open file count** and **aggregate bytes** prior to packet assembly; exceeding caps SHALL surface as `packet_overflow` before Runner invocation

### Requirement 17 (FR-PHASE)

**User Story:** As a developer, I want a trait-based phase system with separated concerns, so that phases can be implemented independently and tested in isolation.

#### Acceptance Criteria

1. WHEN a phase is implemented THEN the system SHALL separate prompt generation, packet assembly, and postprocessing into distinct methods
2. WHEN a phase declares dependencies THEN the system SHALL enforce that dependent phases complete before execution
3. WHEN a phase generates a prompt THEN the system SHALL use context information including spec_id, spec_dir, config, and available artifacts
4. WHEN a phase creates a packet THEN the system SHALL include relevant artifacts from previous phases with proper evidence tracking
5. WHEN a phase postprocesses Claude's response THEN the system SHALL generate both markdown artifacts and core YAML artifacts with structured data
6. WHEN phases are executed THEN the system SHALL support Requirements, Design, and Tasks phases with proper dependency ordering
7. `build_packet()` and `postprocess()` SHALL be deterministic for a given `{inputs, config, env, cache}`
8. `postprocess()` SHALL perform no I/O except artifact writes via the atomic writer defined by FR-FS

### Requirement 18 (FR-CLI)

**User Story:** As a developer, I want comprehensive CLI flags with documented defaults, so that I can configure all system behaviors without editing config files.

#### Acceptance Criteria

1. The CLI SHALL expose: `--stdout-cap-bytes`, `--stderr-cap-bytes`, `--packet-max-bytes`, `--packet-max-lines`, `--phase-timeout`, `--lock-ttl-seconds`, `--ignore-secret-pattern`, `--extra-secret-pattern`, `--debug-packet`, `--allow-links`, `--runner-mode`, `--runner-distro`, `--strict-lock`, `--verbose`
2. `--help` output SHALL document defaults and units for all numeric/time flags

### Requirement 19 (FR-SCHEMA)

**User Story:** As a developer, I want JSON schema compliance and drift control, so that all emitted JSON is validated and breaking changes are caught in CI.

#### Acceptance Criteria

1. All emitted JSON (receipts, status, doctor, benchmark) SHALL validate against the project's v1 schemas
2. `receipt.v1.json` SHALL allow optional fields `stderr_redacted`, `runner_distro`, and `warnings`, and set `additionalProperties: true`
3. `status.v1.json` SHALL allow optional `pending_fixups` with counts only
4. CI SHALL fail on schema drift or invalid example regeneration

### Requirement 20 (FR-LLM)

**User Story:** As a developer, I want to use multiple LLM providers interchangeably, so that I can choose the best model for my needs and avoid vendor lock-in.

#### Acceptance Criteria

1. WHEN the system invokes an LLM THEN the system SHALL abstract the invocation behind a single `LlmBackend` interface so orchestrator and phases are agnostic to transport method
2. WHEN configuration specifies a provider THEN the system SHALL support both CLI-based providers (Gemini CLI, Claude CLI) and HTTP-based providers (OpenRouter, Anthropic API)
3. WHEN a provider is configured THEN the system SHALL select the appropriate backend implementation based on provider type
4. WHEN multiple providers are available THEN the system SHALL support fallback from primary to secondary provider if primary is unavailable
5. WHEN an LLM invocation completes THEN the receipt SHALL record provider name, model used, timeout, and token counts where available

### Requirement 21 (FR-LLM-CLI)

**User Story:** As a developer, I want to use CLI-based LLM providers, so that I can leverage locally installed tools with their native authentication.

#### Acceptance Criteria

1. WHEN a CLI provider is configured THEN the system SHALL invoke the provider via `std::process::Command` with the existing Runner timeout and process control guarantees
2. WHEN selecting a CLI provider THEN the system SHALL support configuration via `[llm] provider = "gemini-cli" | "claude-cli"` in config file
3. WHEN selecting a CLI provider THEN the system SHALL support override via `--llm-provider` CLI flag or `XCHECKER_LLM_PROVIDER` environment variable
4. WHEN a CLI provider is configured THEN the system SHALL discover the binary via explicit config `[llm.<provider>] binary = "<path>"` or fallback to `$PATH` resolution
5. WHEN `xchecker doctor` runs THEN the system SHALL report whether each configured CLI binary is found, its version (best-effort), and authentication status where detectable
6. WHEN a CLI provider is invoked THEN the system SHALL apply the same timeout enforcement, ring buffer management, and process tree termination as the existing Runner
7. WHEN a CLI provider produces output THEN the system SHALL treat stdout as the response content and capture stderr into the ring buffer with redaction

### Requirement 22 (FR-LLM-GEM)

**User Story:** As a developer, I want to use Gemini CLI as my primary LLM provider, so that I can leverage Google's models with my existing API quota.

#### Acceptance Criteria

1. WHEN Gemini CLI is configured THEN the system SHALL invoke it non-interactively using the exact command: `gemini -p "<prompt>" --model <model>` with no REPL or interactive commands
2. WHEN Gemini CLI is configured THEN the system SHALL assume Gemini CLI handles authentication itself via `GEMINI_API_KEY` environment variable; xchecker SHALL NOT read or log this value
3. WHEN Gemini CLI produces output THEN the system SHALL treat stdout as opaque text (no NDJSON requirement) and capture stderr with redaction and 2 KiB cap
4. WHEN `xchecker doctor` runs with Gemini configured THEN the system SHALL run `gemini -h` to confirm binary functionality but SHALL NOT call the LLM with test prompts
5. WHEN Gemini model is configured THEN the system SHALL support `[llm.gemini] default_model = "<model>"` with optional per-phase overrides for `model_requirements`, `model_design`, `model_tasks`, `model_review`, `model_fixup`
6. WHEN Gemini CLI is invoked THEN the system SHALL pass the model via `--model` flag, choosing per-phase override if present, otherwise default_model
7. WHEN Gemini CLI is used THEN the system SHALL keep it in text-only mode by default, not enabling or relying on filesystem tools that write directly
8. WHEN Gemini CLI configuration includes `[llm.gemini] allow_tools = true` THEN the system MAY enable agentic mode as an experimental feature with appropriate safety controls (deferred to post-1.0)

### Requirement 23 (FR-LLM-API)

**User Story:** As a developer, I want to use HTTP-based LLM APIs, so that I can access cloud models without local CLI installation.

#### Acceptance Criteria

1. WHEN an HTTP provider is configured THEN the system SHALL support calling remote APIs over HTTPS using an async HTTP client
2. WHEN an HTTP provider requires authentication THEN the system SHALL read API keys from environment variables specified in config `[llm.<provider>] api_key_env = "<ENV_VAR_NAME>"`
3. WHEN `xchecker doctor` runs with HTTP provider configured THEN the system SHALL check that the required environment variable is set and optionally perform a health check in opt-in mode only
4. WHEN an HTTP provider is invoked THEN the system SHALL support both streaming and non-streaming modes, starting with non-streaming for simplicity
5. WHEN an HTTP API call fails THEN the system SHALL map errors to existing error taxonomy: 4xx auth/quota → `claude_failure` with exit code 70, 5xx → same with provider outage note, network timeout → `phase_timeout` with exit code 10
6. WHEN logging or persisting HTTP provider data THEN the system SHALL never log API keys, raw HTTP headers, or full request bodies, but MAY include provider name, model, region, and token counts
7. WHEN an HTTP provider response is received THEN the system SHALL apply redaction to any error messages before persistence

### Requirement 24 (FR-LLM-OR)

**User Story:** As a developer, I want to use OpenRouter as an HTTP provider, so that I can access multiple model providers through a single API with potentially free or low-cost options.

#### Acceptance Criteria

1. WHEN OpenRouter is configured THEN the system SHALL use endpoint `https://openrouter.ai/api/v1/chat/completions` by default and support `[llm.openrouter] base_url`, `api_key_env = "OPENROUTER_API_KEY"`, `model = "<provider/model>"`, `max_tokens = <int>`, `temperature = <float>`
2. WHEN OpenRouter is invoked THEN the system SHALL include authentication header `Authorization: Bearer $OPENROUTER_API_KEY` and SHALL never log the API key
3. WHEN OpenRouter is invoked THEN the system SHALL include required headers `HTTP-Referer: https://effortlesssteven.com/xchecker` and `X-Title: xchecker` to identify the tool
4. WHEN OpenRouter is used THEN the system SHALL use OpenAI-compatible request format with `model`, `messages` (system + user roles), and `stream: false`
5. WHEN OpenRouter responds THEN the system SHALL extract `choices[0].message.content` into `raw_response` and token counts from `usage` if available
6. WHEN OpenRouter configuration is documented THEN the system SHALL note that free/low-cost model availability changes and users should check the OpenRouter dashboard; recommended default: `google/gemini-2.0-flash-lite`

### Requirement 25 (FR-LLM-ANTH)

**User Story:** As a developer, I want to use Anthropic API directly, so that I can access Claude models via HTTP without the CLI.

#### Acceptance Criteria

1. WHEN Anthropic API is configured THEN the system SHALL use endpoint `https://api.anthropic.com/v1/messages` by default and support `[llm.anthropic] base_url`, `api_key_env = "ANTHROPIC_API_KEY"`, `model = "<model>"`, `max_tokens = <int>`, `temperature = <float>`
2. WHEN Anthropic API is invoked THEN the system SHALL include required headers: `x-api-key: $ANTHROPIC_API_KEY`, `anthropic-version: 2023-06-01`, `content-type: application/json` and SHALL never log the API key
3. WHEN Anthropic API is invoked THEN the system SHALL use the Messages API format with `model`, `max_tokens`, `temperature`, and `messages` (user role)
4. WHEN Anthropic API responds THEN the system SHALL extract `content[0].text` into `raw_response` and token counts from `usage` if available
5. WHEN Anthropic API is used THEN the system SHALL maintain compatibility with existing Claude CLI prompt templates where possible, or maintain separate templates per provider

### Requirement 26 (FR-LLM-META)

**User Story:** As a developer, I want provider metadata in receipts and observability, so that I can track which models were used and debug provider-specific issues.

#### Acceptance Criteria

1. WHEN any LLM invocation completes THEN the receipt SHALL include `llm_provider` field with provider name (e.g., "gemini-cli", "claude-cli", "openrouter", "anthropic")
2. WHEN any LLM invocation completes THEN the receipt SHALL include `llm_model` field with the model identifier used
3. WHEN any LLM invocation completes THEN the receipt SHALL include `llm_timeout_seconds` field with the timeout value applied
4. WHEN token counts are available from the provider THEN the receipt SHALL include `llm_tokens_input` and `llm_tokens_output` fields, otherwise these SHALL be null
5. WHEN a fallback provider is used THEN the receipt SHALL include a warning indicating primary provider was unavailable and fallback was used

## Non-Functional Requirements

**NFR1 Performance:** `spec --dry-run` baseline completes ≤ 5s; packetization of 100 files ≤ 200ms; JCS emission ≤ 50ms
- **Verification**: Run benchmarks and measure actual performance
- **Status**: ⏳ Needs verification with actual benchmarks
- **Tasks**: V6.5, V9.4, V9.5, V9.6

**NFR2 Security:** No secrets written to disk except under explicit `--debug-packet` after successful scan; redaction applied before persistence; path validation prevents traversal; symlinks/hardlinks rejected by default; API keys never logged or persisted
- **Verification**: Test secret detection, redaction in all output paths, path validation, API key handling
- **Status**: ✅ Implementation complete, needs comprehensive testing
- **Tasks**: V2.2, V4.2, V7.2, V7.6, V7.7

**NFR3 Portability:** Linux, macOS, Windows (native) pass full runtime tests; Windows+WSL passes interop tests
- **Verification**: Run full test suite on all platforms
- **Status**: ✅ Implementation complete, needs platform-specific testing
- **Tasks**: V5 (all), V8.4, V10.6

**NFR4 Observability:** `--verbose` provides structured logs with phase, spec_id, duration_ms, runner_mode, llm_provider; no secrets logged; redaction applied before any output
- **Verification**: Test logging output, verify required fields, test redaction
- **Status**: ✅ Implementation complete, needs comprehensive testing
- **Tasks**: V6.6, V6.7, V6.8

**NFR5 Atomicity:** All writes use temp-file + fsync + rename; Windows retry logic handles transient locks; same-volume constraint enforced
- **Verification**: Test atomic operations, Windows retry, cross-filesystem fallback
- **Status**: ✅ Implementation complete, needs edge case testing
- **Tasks**: V3.4, V4.5, V7.4

**NFR6 Determinism:** RFC 8785-compliant JCS emission produces byte-identical output; arrays sorted; blake3_first8 computed on on-disk bytes; stable across platforms
- **Verification**: Test JCS byte-identity, array sorting, hash stability
- **Status**: ✅ Implementation complete, needs verification testing
- **Tasks**: V1.1, V1.2, V8.2, V8.3

**NFR7 Caching Efficiency:** InsightCache achieves >70% hit rate on repeated runs with unchanged files; cache validation completes in <10ms per file; cached insights reduce packet assembly time by >50% for large codebases
- **Verification**: Test cache hit rates, validation performance, packet assembly speedup
- **Status**: ✅ Implementation complete, needs performance verification
- **Tasks**: V9.3, V9.4

**NFR8 Cost Control:** Automated tests that call real LLM providers SHALL be skippable via `XCHECKER_SKIP_LLM_TESTS=1` environment flag and SHALL use minimal prompts (single short message) with low max_tokens (≤ 256 for tests); `xchecker doctor` and documentation conformance tests SHALL NOT call real LLMs; recommended test models: Gemini CLI `gemini-2.0-flash-lite` (1000 calls/day, 1M tokens per call free preview quota), OpenRouter `google/gemini-2.0-flash-lite` (1000 calls/day, 1M tokens per call for some models on free tier)
- **Verification**: Test environment flag skipping, verify doctor doesn't call LLMs (AT-LLM-008), verify test prompts are minimal
- **Status**: ⏳ Needs implementation
- **Tasks**: V11.14, V11.11

**NFR9 OpenRouter Call Budget:** For OpenRouter, xchecker SHALL treat calls as a limited resource (1000 calls/day on current free tier, with 1M tokens per call for some models); the default per-process OpenRouter call budget SHALL NOT exceed 20 calls in a single test or CLI run to prevent accidental quota exhaustion; all LLM integration tests that hit OpenRouter SHALL be gated behind `XCHECKER_USE_OPENROUTER=1` and respect the per-process call budget; if the per-process call budget is exceeded, the OpenRouter backend SHALL fail fast with a clear "LLM budget exhausted" error; the call budget MAY be increased via `XCHECKER_OPENROUTER_BUDGET` for local runs, but CI SHALL use the default or lower value
- **Verification**: Test budget enforcement, test budget override, verify CI stays under budget
- **Status**: ⏳ Needs implementation
- **Tasks**: V11.5, V11.11

## Verification Requirements Matrix

| Requirement | Implementation Status | Testing Status | Verification Tasks |
|-------------|----------------------|----------------|-------------------|
| FR-RUN | ✅ Complete | ⏳ Needs edge cases | V2.3, V2.4, V2.5, V2.6 |
| FR-ORC | ✅ Complete | ⏳ Needs edge cases | V3.1, V3.2, V3.5, V3.6 |
| FR-PKT | ✅ Complete | ⏳ Needs wiring | V2.1, V7.3, V9.2 |
| FR-SEC | ✅ Complete | ⏳ Needs edge cases | V2.2, V7.2, V7.6 |
| FR-FIX | ✅ Complete | ⏳ Needs testing | V4 (all) |
| FR-LOCK | ✅ Complete | ⏳ Needs edge cases | V3.3, V7.5 |
| FR-JCS | ✅ Complete | ⏳ Needs verification | V1.1, V1.2, V8.2 |
| FR-STA | ✅ Complete | ⏳ Needs testing | V4.8, V4.9 |
| FR-WSL | ✅ Complete | ⏳ Needs platform tests | V5 (all) |
| FR-EXIT | ✅ Complete | ⏳ Needs testing | V1.3, V7.1 |
| FR-CFG | ✅ Complete | ⏳ Needs testing | V1.6 |
| FR-BENCH | ✅ Complete | ⏳ Needs verification | V6 (all) |
| FR-FS | ✅ Complete | ⏳ Needs edge cases | V3.4, V4.5, V5.10 |
| FR-OBS | ✅ Complete | ⏳ Needs testing | V6.6, V6.7, V6.8 |
| FR-CACHE | ✅ Complete | ⏳ Needs wiring | V9.3, V9.4 |
| FR-SOURCE | ✅ Complete | ⏳ Needs wiring | V9.2 |
| FR-PHASE | ✅ Complete | ⏳ Needs wiring | V3.6, V9.2 |
| FR-CLI | ✅ Complete | ⏳ Needs testing | V1.8 |
| FR-SCHEMA | ✅ Complete | ⏳ Needs verification | V8.2, V8.3 |
| FR-LLM | ⏳ Needs implementation | ⏳ Not started | V11.1 |
| FR-LLM-CLI | ⏳ Needs implementation | ⏳ Not started | V11.2 |
| FR-LLM-GEM | ⏳ Needs implementation | ⏳ Not started | V11.3 |
| FR-LLM-API | ⏳ Needs implementation | ⏳ Not started | V11.4 |
| FR-LLM-OR | ⏳ Needs implementation | ⏳ Not started | V11.5 |
| FR-LLM-ANTH | ⏳ Needs implementation | ⏳ Not started | V11.6 |
| FR-LLM-META | ⏳ Needs implementation | ⏳ Not started | V11.7 |

## Verification & Improvement Focus

**Current Implementation Status:**
- ✅ All core modules implemented (canonicalization, redaction, runner, orchestrator, packet, fixup, lock, status, receipt, config)
- ✅ End-to-end phase execution working
- ✅ Cross-platform support (Linux, macOS, Windows, WSL)
- ✅ Security controls operational (secret redaction, path validation)
- ✅ Performance monitoring (benchmarks, logging)
- ✅ Comprehensive error handling with exit codes

**Verification Priorities:**
1. **Testing**: Comprehensive unit and integration tests for all modules
2. **Edge Cases**: Timeout scenarios, packet overflow, secret detection, concurrent execution
3. **Performance**: Verify NFR1 targets met (dry-run ≤ 5s, packetization ≤ 200ms, JCS ≤ 50ms)
4. **Integration**: Wire PacketBuilder into orchestrator, remove TODOs from staged modules
5. **Platform**: Windows-specific testing (WSL, Job Objects), cross-platform verification
6. **Documentation**: Update all docs to match implementation, verify examples work

**Missing Implementation Work:**
- Wire PacketBuilder into orchestrator (replace placeholders)
- Remove `#![allow(dead_code, unused_imports)]` from staged modules
- Complete integration of InsightCache into PacketBuilder
- Implement comprehensive error path testing
- Optimize performance to meet NFR1 targets

## Assumptions & Out-of-Scope

**Assumptions:**
- Claude CLI is installed and licensed by the user
- Users have appropriate permissions to write to XCHECKER_HOME
- On Windows with WSL, at least one WSL distribution is installed if WSL mode is used
- Core implementation is functional and operational
- Focus is on verification, testing, and optimization

**Out-of-Scope:**
- Prompt engineering and LLM prompt optimization
- Cloud callbacks or long-running background agents
- Changing published schemas (v1 is stable)
- Multi-user concurrent access to the same spec directory
- Greenfield implementation (core work is complete)
- Agentic mode with filesystem tools (deferred to future work, experimental flag only)
- Custom LLM provider plugins (only built-in providers supported)


---

# V11–V18 Roadmap: Multi-Provider LLM & Ecosystem Expansion

## V11 – LLM Core Skeleton & Claude Backend (MVP+)

**Goal**: Put the existing Runner behind a clean LlmBackend abstraction, keep Controlled writes, and wire basic LLM metadata into receipts. No new providers yet; just Claude CLI under a better shape.

### Requirement 25 (FR-EXEC)

**User Story:** As a developer, I want to enforce Controlled execution mode where LLMs only propose changes, so that all file writes go through the FixupEngine and atomic write pipeline.

#### Acceptance Criteria

1. WHEN the system executes a phase THEN the system SHALL enforce ExecutionStrategy::Controlled (only valid value in V11)
2. WHEN configuration specifies execution_strategy THEN the system SHALL accept only "controlled" (default)
3. WHEN ExternalTool strategy is somehow selected THEN the system SHALL return XCheckerError::Unsupported("ExternalTool not yet supported")
4. WHEN a phase completes THEN the system SHALL ensure no LLM output directly modifies files; all writes go through FixupEngine + atomic pipeline
5. WHEN receipts are written THEN the system SHALL include execution_strategy field for audit trail

### Requirement 26 (FR-LLM)

**User Story:** As a developer, I want to use multiple LLM providers interchangeably, so that I can choose the best model for my needs and avoid vendor lock-in.

#### Acceptance Criteria

1. WHEN the system invokes an LLM THEN the system SHALL abstract the invocation behind a single `LlmBackend` trait so orchestrator and phases are agnostic to transport method
2. WHEN configuration specifies a provider THEN the system SHALL support both CLI-based providers (Gemini CLI, Claude CLI) and HTTP-based providers (OpenRouter, Anthropic API)
3. WHEN a provider is configured THEN the system SHALL select the appropriate backend implementation based on provider type
4. WHEN multiple providers are available THEN the system SHALL support fallback from primary to secondary provider if primary is unavailable
5. WHEN an LLM invocation completes THEN the receipt SHALL record provider name, model used, timeout, and token counts where available

### Requirement 27 (FR-LLM-CLI)

**User Story:** As a developer, I want to use CLI-based LLM providers, so that I can leverage locally installed tools with their native authentication.

#### Acceptance Criteria

1. WHEN a CLI provider is configured THEN the system SHALL invoke the provider via `std::process::Command` with the existing Runner timeout and process control guarantees
2. WHEN selecting a CLI provider THEN the system SHALL support configuration via `[llm] provider = "gemini-cli" | "claude-cli"` in config file
3. WHEN selecting a CLI provider THEN the system SHALL support override via `--llm-provider` CLI flag or `XCHECKER_LLM_PROVIDER` environment variable
4. WHEN a CLI provider is configured THEN the system SHALL discover the binary via explicit config `[llm.<provider>] binary = "<path>"` or fallback to `$PATH` resolution
5. WHEN `xchecker doctor` runs THEN the system SHALL report whether each configured CLI binary is found, its version (best-effort), and authentication status where detectable
6. WHEN a CLI provider is invoked THEN the system SHALL apply the same timeout enforcement, ring buffer management, and process tree termination as the existing Runner
7. WHEN a CLI provider produces output THEN the system SHALL treat stdout as the response content and capture stderr into the ring buffer with redaction

### Requirement 28 (FR-LLM-GEM)

**User Story:** As a developer, I want to use Gemini CLI as my primary LLM provider, so that I can leverage Google's models with my existing API quota.

#### Acceptance Criteria

1. WHEN Gemini CLI is configured THEN the system SHALL invoke it non-interactively using the exact command: `gemini -p "<prompt>" --model <model>` with no REPL or interactive commands
2. WHEN Gemini CLI is configured THEN the system SHALL assume Gemini CLI handles authentication itself via `GEMINI_API_KEY` environment variable; xchecker SHALL NOT read or log this value
3. WHEN Gemini CLI produces output THEN the system SHALL treat stdout as opaque text (no NDJSON requirement) and capture stderr with redaction and 2 KiB cap
4. WHEN `xchecker doctor` runs with Gemini configured THEN the system SHALL run `gemini -h` to confirm binary functionality but SHALL NOT call the LLM with test prompts
5. WHEN Gemini model is configured THEN the system SHALL support `[llm.gemini] default_model = "<model>"` with optional per-phase overrides for `model_requirements`, `model_design`, `model_tasks`, `model_review`, `model_fixup`
6. WHEN Gemini CLI is invoked THEN the system SHALL pass the model via `--model` flag, choosing per-phase override if present, otherwise default_model
7. WHEN Gemini CLI is used THEN the system SHALL keep it in text-only mode by default, not enabling or relying on filesystem tools that write directly
8. WHEN Gemini CLI configuration includes `[llm.gemini] allow_tools = true` THEN the system MAY enable agentic mode as an experimental feature with appropriate safety controls (deferred to post-1.0)

### Requirement 29 (FR-LLM-API)

**User Story:** As a developer, I want to use HTTP-based LLM APIs, so that I can access cloud models without local CLI installation.

#### Acceptance Criteria

1. WHEN an HTTP provider is configured THEN the system SHALL support calling remote APIs over HTTPS using an async HTTP client
2. WHEN an HTTP provider requires authentication THEN the system SHALL read API keys from environment variables specified in config `[llm.<provider>] api_key_env = "<ENV_VAR_NAME>"`
3. WHEN `xchecker doctor` runs with HTTP provider configured THEN the system SHALL check that the required environment variable is set and optionally perform a health check in opt-in mode only
4. WHEN an HTTP provider is invoked THEN the system SHALL support both streaming and non-streaming modes, starting with non-streaming for simplicity
5. WHEN an HTTP API call fails THEN the system SHALL map errors to existing error taxonomy: 4xx auth/quota → `claude_failure` with exit code 70, 5xx → same with provider outage note, network timeout → `phase_timeout` with exit code 10
6. WHEN logging or persisting HTTP provider data THEN the system SHALL never log API keys, raw HTTP headers, or full request bodies, but MAY include provider name, model, region, and token counts
7. WHEN an HTTP provider response is received THEN the system SHALL apply redaction to any error messages before persistence

### Requirement 30 (FR-LLM-OR)

**User Story:** As a developer, I want to use OpenRouter as an HTTP provider, so that I can access multiple model providers through a single API with potentially free or low-cost options.

#### Acceptance Criteria

1. WHEN OpenRouter is configured THEN the system SHALL use endpoint `https://openrouter.ai/api/v1/chat/completions` by default and support `[llm.openrouter] base_url`, `api_key_env = "OPENROUTER_API_KEY"`, `model = "<provider/model>"`, `max_tokens = <int>`, `temperature = <float>`
2. WHEN OpenRouter is invoked THEN the system SHALL include authentication header `Authorization: Bearer $OPENROUTER_API_KEY` and SHALL never log the key
3. WHEN OpenRouter is invoked THEN the system SHALL enforce a call budget (default 20, overridable via `XCHECKER_OPENROUTER_BUDGET`) to prevent runaway costs
4. WHEN the call budget is exceeded THEN the system SHALL exit with code 70 and report the budget limit
5. WHEN `xchecker doctor` runs with OpenRouter configured THEN the system SHALL check that `OPENROUTER_API_KEY` is set but SHALL NOT send an HTTP request

### Requirement 31 (FR-LLM-ANTH)

**User Story:** As a developer, I want to use Anthropic's API directly, so that I can leverage Claude models with Anthropic's native API.

#### Acceptance Criteria

1. WHEN Anthropic API is configured THEN the system SHALL use endpoint `https://api.anthropic.com/v1/messages` by default and support `[llm.anthropic] base_url`, `api_key_env = "ANTHROPIC_API_KEY"`, `model = "<model>"`, `max_tokens = <int>`, `temperature = <float>`
2. WHEN Anthropic API is invoked THEN the system SHALL include authentication header `x-api-key: $ANTHROPIC_API_KEY` and SHALL never log the key
3. WHEN Anthropic API is invoked THEN the system SHALL use the Messages API with proper request/response mapping
4. WHEN `xchecker doctor` runs with Anthropic configured THEN the system SHALL check that `ANTHROPIC_API_KEY` is set but SHALL NOT send an HTTP request

### Requirement 32 (FR-LLM-META)

**User Story:** As a developer, I want rich LLM metadata in receipts, so that I can audit which provider and model was used for each phase.

#### Acceptance Criteria

1. WHEN an LLM invocation completes THEN the receipt SHALL include `llm.provider` (e.g., "gemini-cli", "claude-cli", "openrouter", "anthropic")
2. WHEN an LLM invocation completes THEN the receipt SHALL include `llm.model_used` (the actual model name/version)
3. WHEN an LLM invocation completes THEN the receipt SHALL include `llm.tokens_input` and `llm.tokens_output` where available from the provider
4. WHEN an LLM invocation times out THEN the receipt SHALL include `llm.timed_out: true`
5. WHEN an LLM invocation fails THEN the receipt SHALL include provider-specific error details without exposing API keys

## V12 – Gemini CLI as First-Class Provider

**Goal**: Add Gemini CLI as the default CLI backend, with Claude as optional/fallback. Still Controlled mode only.

### Requirement 33 (FR-LLM-GEM-CONFIG)

**User Story:** As a developer, I want to configure Gemini CLI as my default provider, so that I can use Google's models by default.

#### Acceptance Criteria

1. WHEN configuration specifies `[llm] provider = "gemini-cli"` THEN the system SHALL use Gemini as the primary backend
2. WHEN configuration specifies `[llm] fallback_provider = "claude-cli"` THEN the system SHALL use Claude only if Gemini is unavailable
3. WHEN both `[llm.gemini]` and `[llm.claude]` sections are present THEN the system SHALL parse both fully
4. WHEN `xchecker doctor` runs THEN the system SHALL report Gemini availability (binary, version) and Claude availability (if configured as fallback)

## V13 – HTTP Client & OpenRouter Backend (Optional)

**Goal**: Add a single HTTP path (OpenRouter) that can be enabled when you want it, with clear budgets.

### Requirement 34 (FR-LLM-HTTP-CLIENT)

**User Story:** As a developer, I want to use HTTP-based LLM APIs, so that I can access cloud models without local CLI installation.

#### Acceptance Criteria

1. WHEN an HTTP provider is configured THEN the system SHALL use an async HTTP client (reqwest) with proper error handling
2. WHEN HTTP requests are made THEN the system SHALL include proper User-Agent and timeout headers
3. WHEN HTTP requests fail THEN the system SHALL map errors to existing error taxonomy with clear messaging

## V14 – Anthropic HTTP, Rich Metadata & Provider Docs

**Goal**: Add Anthropic HTTP backend and finish the metadata + docs story.

### Requirement 35 (FR-LLM-DOCS)

**User Story:** As a developer, I want comprehensive documentation for all LLM providers, so that I can choose and configure the right provider for my needs.

#### Acceptance Criteria

1. WHEN I read docs/LLM_PROVIDERS.md THEN the system SHALL document all supported providers (Gemini CLI, Claude CLI, OpenRouter, Anthropic)
2. WHEN I read the documentation THEN the system SHALL include environment variables, config keys, and test gating for each provider
3. WHEN I read the documentation THEN the system SHALL include cost estimates and budget controls where applicable
4. WHEN I read the documentation THEN the system SHALL include authentication setup instructions for each provider

## V15 – Claude Code (Claude Code) Integration & UX

**Goal**: Make it trivial to trigger xchecker phases from Claude Code, with xchecker compressing context and enforcing invariants.

### Requirement 36 (FR-Claude Code-CLI)

**User Story:** As a Claude Code user, I want to trigger xchecker phases from Claude Code, so that I can orchestrate spec generation as part of my development workflow.

#### Acceptance Criteria

1. WHEN `xchecker spec --json` runs THEN the system SHALL emit a stable JSON shape that Claude Code can parse
2. WHEN `xchecker status --json` runs THEN the system SHALL emit a compact status summary suitable for agent consumption
3. WHEN `xchecker resume --phase <phase> --json` runs THEN the system SHALL emit a compact summary for the next agent step
4. WHEN Claude Code calls xchecker THEN the system SHALL compress state and enforce Controlled edits

### Requirement 37 (FR-Claude Code-FLOWS)

**User Story:** As a Claude Code user, I want example flows that show how to use xchecker, so that I can integrate it into my projects.

#### Acceptance Criteria

1. WHEN I read the documentation THEN the system SHALL provide example Claude Code flows for spec generation
2. WHEN I read the documentation THEN the system SHALL show how to use receipts/status JSON instead of raw repo state
3. WHEN I read the documentation THEN the system SHALL document a canonical `/xchecker` slash command

## V16 – Workspace & Multi-Spec Orchestration

**Goal**: Move from "one spec at a time" to a workspace view where xchecker can track, summarize, and surface dozens of specs across a codebase.

### Requirement 38 (FR-WORKSPACE)

**User Story:** As a developer, I want to manage multiple specs in a workspace, so that I can see the status of all specs at a glance.

#### Acceptance Criteria

1. WHEN `xchecker project init <name>` runs THEN the system SHALL create a workspace registry file
2. WHEN `xchecker project add-spec <spec-id> --tag <tag>` runs THEN the system SHALL register the spec with tags
3. WHEN `xchecker project list` runs THEN the system SHALL list all specs with status, tags, and metadata
4. WHEN `xchecker project status --json` runs THEN the system SHALL emit aggregated status for all specs
5. WHEN `xchecker project history <spec-id> --json` runs THEN the system SHALL emit a timeline of phase progression and metrics

### Requirement 39 (FR-WORKSPACE-TUI)

**User Story:** As a developer, I want a terminal UI to browse specs and receipts, so that I can navigate without leaving the terminal.

#### Acceptance Criteria

1. WHEN `xchecker project tui` runs THEN the system SHALL display a text UI with specs list, receipt summary, and warnings
2. WHEN I navigate the TUI THEN the system SHALL show pending fixups, error counts, and stale specs

## V17 – Policy & Enforcement ("Double-Entry SDLC" in CI)

**Goal**: Turn xchecker's receipts + status into enforceable gates in CI and repo workflows.

### Requirement 40 (FR-GATE)

**User Story:** As a developer, I want to enforce spec completeness in CI, so that PRs cannot merge without proper spec documentation.

#### Acceptance Criteria

1. WHEN `xchecker gate <spec-id>` runs THEN the system SHALL evaluate a policy against the latest receipt
2. WHEN the policy passes THEN the system SHALL exit with code 0
3. WHEN the policy fails THEN the system SHALL exit with code non-zero and report structured errors for CI
4. WHEN policy is configured THEN the system SHALL support: `--min-phase tasks`, `--fail-on-pending-fixups`, `--max-phase-age 7d`

### Requirement 41 (FR-GATE-CI)

**User Story:** As a developer, I want ready-to-use CI templates, so that I can add xchecker gating to my repo in minutes.

#### Acceptance Criteria

1. WHEN I read the documentation THEN the system SHALL provide `.github/workflows/xchecker-gate.yml` template
2. WHEN I read the documentation THEN the system SHALL provide GitLab CI equivalent
3. WHEN I read the documentation THEN the system SHALL show how to make xchecker gate a required status check

## V18 – Ecosystem & Templates (Batteries Included)

**Goal**: Turn xchecker from "power user tool" into something a modern dev/team can adopt in an afternoon.

### Requirement 42 (FR-TEMPLATES)

**User Story:** As a developer, I want spec templates for common stacks, so that I can bootstrap a spec in minutes.

#### Acceptance Criteria

1. WHEN `xchecker template list` runs THEN the system SHALL show available templates (fullstack-nextjs, rust-microservice, python-fastapi, docs-refactor)
2. WHEN `xchecker template init <template> <spec-id>` runs THEN the system SHALL seed a starting problem statement and config
3. WHEN I read the documentation THEN the system SHALL show example spec flows for each template

### Requirement 43 (FR-HOOKS)

**User Story:** As a developer, I want to hook xchecker into my infrastructure, so that I can integrate it with my tools.

#### Acceptance Criteria

1. WHEN `[hooks]` section is configured THEN the system SHALL support pre-phase and post-phase hooks
2. WHEN a hook is triggered THEN the system SHALL execute the configured command with context
3. WHEN a hook fails THEN the system SHALL log the failure but continue execution (non-blocking)
4. WHEN I read the documentation THEN the system SHALL show examples: Slack notifications, dashboard sync, Prometheus metrics

### Requirement 44 (FR-SHOWCASE)

**User Story:** As a new user, I want to see concrete examples of xchecker in action, so that I can understand what it does.

#### Acceptance Criteria

1. WHEN I read examples/ directory THEN the system SHALL include fullstack-nextjs example with scripted workflow
2. WHEN I read examples/ directory THEN the system SHALL include mono-repo example with multiple specs
3. WHEN I read the documentation THEN the system SHALL include walkthroughs: "20 minutes to xchecker", "spec to PR with Claude Code"

# xchecker Runtime Requirements (V1–V10)

## 1. Introduction

**Status:** Runtime core implemented, verified, and released as v1.0.0.

**Scope of this document:** Functional and non-functional requirements for the **runtime** only (V1–V10).

This spec describes the xchecker runtime:
- A Rust CLI that orchestrates a multi-phase SDLC flow:
  `requirements → design → tasks → review → fixup → final`.
- A deterministic, schema-governed runtime that emits receipts/status/doctor
  outputs and enforces constraints on processes, filesystem, secrets, and
  configuration.

The **multi-provider LLM and ecosystem work (V11–V18)** is intentionally out of scope here and lives in a separate document ([LLM_PROVIDERS.md](LLM_PROVIDERS.md)).

**Current Runtime State (V1–V10):**

- ✅ All core components implemented (Runner, Orchestrator, PacketBuilder,
  SecretRedactor, FixupEngine, LockManager, StatusManager, Config system,
  Canonicalizer, Receipt/Doctor/Status, Benchmark, SourceResolver, InsightCache).
- ✅ End-to-end phase execution working.
- ✅ Cross-platform support implemented (Linux, macOS, Windows, WSL).
- ✅ Comprehensive test coverage and smoke tests.
- ✅ NFRs for security, observability, atomicity, determinism, caching met, with
  performance partially verified (benchmarks in place, some tests need refresh).

## 2. Glossary

- **xchecker** – Rust CLI tool for orchestrating spec-generation workflows.
- **Runner** – Process launcher for Claude CLI (native or WSL modes).
- **Orchestrator** – Component that enforces phase order and coordinates execution.
- **Phase** – One step in the workflow: requirements, design, tasks, review, fixup, final.
- **Packet** – Deterministically assembled request payload (text) with enforced size limits.
- **Receipt** – Per-phase JSON result (v1 schema) with JCS canonicalization.
- **StatusOutput** – JSON snapshot of spec state and effective configuration.
- **DoctorOutput** – JSON summary of environment health.
- **JCS** – JSON Canonicalization Scheme (RFC 8785) for deterministic JSON.
- **Fixup** – Proposed file changes from review phase, applied via unified diffs.
- **Spec Root** – `XCHECKER_HOME/specs/<spec-id>`.
- **InsightCache** – BLAKE3-keyed cache for per-file insights to avoid reprocessing.
- **SourceResolver** – Resolves input sources (GitHub, filesystem, stdin).
- **SecretRedactor** – Detects and redacts secrets before persistence and external invocation.
- **Canonicalizer** – Single choke point for RFC 8785 JSON canonicalization.

---

## 3. Functional Requirements (Runtime)

> IDs FR-RUN…FR-SCHEMA correspond to V1–V10 runtime features.

### FR-RUN (Requirement 1): Runner

**User Story:**
As a developer, I want the Runner to execute Claude CLI with proper timeout enforcement, so hung processes don't block indefinitely and I get reliable results.

**Acceptance Criteria**

1. Native mode spawns the Claude CLI process directly with configured arguments.
2. On Windows, WSL mode translates paths and environment variables to WSL format
   and uses `wsl.exe --exec`.
3. Auto mode:
   - Tries native PATH Claude first.
   - Falls back to WSL on Windows if native is unavailable.
4. Phase timeout is enforced (default 600s, minimum 5s) with wall-clock timeout.
5. On timeout, Runner sends TERM, waits up to 5s, then sends KILL.
6. On Windows, Runner assigns processes to a Job Object and terminates the job on timeout.
7. On timeout, xchecker exits with code 10 and writes a receipt with
   `error_kind = "phase_timeout"` and `stderr_redacted`.
8. Stdout is treated as NDJSON (each line a JSON object).
9. Mixed/malformed stdout handling:
   - Non-JSON lines ignored.
   - If ≥1 valid JSON object is read, the **last valid object** is used.
   - If none, exit with `claude_failure` and a redacted tail excerpt (≤256 chars pre-redaction).
10. Stderr is redacted and truncated to ≤2048 bytes in `stderr_redacted` before persistence.
11. Ring buffers are enforced:
    - `stdout_cap_bytes = 2 MiB` (default),
    - `stderr_cap_bytes = 256 KiB` (default),
    configurable via `--stdout-cap-bytes` and `--stderr-cap-bytes`.

---

### FR-ORC (Requirement 2): Orchestrator

**User Story:**
As a developer, I want the Orchestrator to enforce legal phase transitions and coordinate execution so the workflow remains consistent and auditable.

**Acceptance Criteria**

1. Executes only legal transitions based on current state.
2. Illegal transition → exit code 2 with actionable guidance on valid next steps.
3. For each phase, orchestrator:
   - Acquires exclusive lock.
   - Builds packet.
   - Scans for secrets.
   - Enforces limits.
   - Invokes Runner.
   - Writes artifacts atomically.
4. On successful phase:
   - Writes partial artifacts under `.partial/`.
   - Promotes to final names via atomic rename.
5. On failure:
   - Writes error receipt (JCS) with `exit_code`, `error_kind`, `error_reason`.
6. Success and failure receipts are JCS-canonical.
7. At start of any phase:
   - Stale `.partial/` directories are removed (best effort).
   - The current completed phase is defined by the last **successful** receipt.

---

### FR-PKT (Requirement 3): PacketBuilder

**User Story:**
As a developer, I want the PacketBuilder to assemble inputs deterministically with enforced limits, so I can prevent oversized requests and maintain reproducibility.

**Acceptance Criteria**

1. Packet assembly:
   - Deterministic ordering of inputs.
   - Counts bytes and lines.
2. If `packet_max_bytes` (default 65536) exceeded → exit 7 before invoking Claude.
3. If `packet_max_lines` (default 1200) exceeded → exit 7 before invoking Claude.
4. On overflow, receipt includes actual size and configured limits.
5. On overflow, write sanitized manifest to
   `context/<phase>-packet.manifest.json` (sizes/counts/paths only).
6. With `--debug-packet` and no secrets detected:
   - May write full packet to `context/<phase>-packet.txt`.
7. Debug packet behavior:
   - File excluded from receipts.
   - Content must be redacted if later reported.
   - Must not be written if any secret rule fires.

---

### FR-SEC (Requirement 4): Secret Scanning

**User Story:**
As a security engineer, I want the SecretScanner to detect and block secrets before they reach Claude or are persisted.

**Acceptance Criteria**

1. Default patterns include:
   - `ghp_[A-Za-z0-9]{36}` (GitHub PAT),
   - `AKIA[0-9A-Z]{16}` (AWS Access Key),
   - `AWS_SECRET_ACCESS_KEY=`,
   - `xox[baprs]-` (Slack),
   - `Bearer [A-Za-z0-9._-]{20,}`.
2. On match, exit code 8 and report which pattern fired (no actual secret).
3. `--ignore-secret-pattern <regex>` suppresses specific patterns.
4. `--extra-secret-pattern <regex>` adds patterns.
5. Stderr redaction: secrets replaced with `***` before persistence.
6. Receipts never include environment variables or raw packet content.
7. Global redaction applied to all human-readable strings (stderr, error_reason,
   warnings, context, doctor/status text, previews) before logging or persistence.

---

### FR-FIX (Requirement 5): FixupEngine

**User Story:**
As a developer, I want the FixupEngine to preview and apply changes safely with path validation.

**Acceptance Criteria**

1. Validation canonicalizes target paths and ensures they are under the allowed root.
2. Targets with `..` or absolute paths outside root are rejected with clear error.
3. Targets that are symlinks or hardlinks are rejected unless `--allow-links` is set.
4. Preview mode (default):
   - Lists intended targets.
   - Shows estimated added/removed lines.
   - Surfaces validation warnings.
   - Does not modify files.
5. Apply mode (`--apply-fixups`):
   - Writes to temp files, fsync, creates `.bak` if target exists.
   - Uses atomic rename with Windows retry.
6. Applies fixups preserving POSIX mode bits / Windows attributes where possible.
7. Cross-filesystem writes use copy→fsync→replace, with original removed only after success.
8. Receipts record applied files with `blake3_first8` and `applied: true`.
9. Preview receipts record targets with `applied: false`.
10. Line ending normalization is applied before diff calculations (LF on write).

---

### FR-LOCK (Requirement 6): LockManager & Lockfile

**User Story:**
As a developer, I want locks to prevent concurrent runs and lockfiles to track reproducibility and drift.

**Acceptance Criteria**

1. Lock acquisition creates an advisory lock file in spec root with `{pid, host, started_at}`.
2. If lock is held by an active process, exit code 9 immediately.
3. Lock is stale if:
   - PID not alive on same host, OR
   - Age > TTL (default 15 minutes, configurable).
4. `--force` breaks stale locks and records a warning in the next receipt.
5. Lock file is removed on normal exit and best-effort on panic (Drop).
6. `xchecker init --create-lock` creates a lockfile capturing:
   - `model_full_name`,
   - `claude_cli_version`,
   - `schema_version`.
7. Lockfile drift (current vs locked values) is computed and included in status output.
8. `--strict-lock` fails (non-zero exit) if drift exists, before any phase.

---

### FR-JCS (Requirement 7): Canonicalization

**User Story:**
As a developer, I want JCS for all JSON outputs so receipts/status have stable diffs.

**Acceptance Criteria**

1. Receipts use RFC 8785 JCS canonicalization (beyond simple BTreeMap ordering; numeric/string normalization according to JCS).
2. Status output uses JCS canonicalization with sorted arrays (artifacts by path).
3. Receipts include:
   - `schema_version: "1"`,
   - `emitted_at` (RFC3339 UTC),
   - `canonicalization_backend: "jcs-rfc8785"`,
   - `exit_code` and phase metadata.
4. Re-serializing receipts yields **byte-identical** JSON.
5. `blake3_first8` is lowercase hex, exactly 8 characters.
6. `blake3_first8` for artifacts is computed from on-disk bytes (LF line endings) so it is stable across platforms.

---

### FR-STA (Requirement 8): Status

**User Story:**
As a developer, I want status outputs with effective configuration and source attribution.

**Acceptance Criteria**

1. `xchecker status <spec-id> --json` emits JCS JSON including `artifacts`, `effective_config`, and `lock_drift`.
2. Each effective_config entry includes `{ value, source }` with source in `{"cli","env","config","programmatic","default"}`.
3. Each artifact entry includes `path` and `blake3_first8`.
4. On a fresh spec with no receipts, status emits sensible defaults without errors.
5. With a lockfile present, drift in `model_full_name`, `claude_cli_version`, `schema_version` is reported.
6. Status may include `"pending_fixups": { "targets", "est_added", "est_removed" }`; omit when unavailable.

---

### FR-WSL (Requirement 9): WSL Support

**User Story:**
As a Windows developer, I want seamless WSL support when Claude is only available in WSL.

**Acceptance Criteria**

1. WSL availability:
   - Checked via `wsl.exe -l -q`, verifying at least one distro.
2. WSL readiness:
   - Claude availability checked via `wsl.exe -d <distro> -- which claude`.
3. If Claude not discoverable in WSL:
   - `doctor` reports remediation;
   - auto mode prefers native runner.
4. Windows path translation:
   - `C:\` → `/mnt/c/` form (UNC and wslpath handled appropriately).
5. Env translation:
   - Path-like env vars adapted to WSL form.
6. `xchecker doctor` on Windows reports native Claude and WSL status with actionable suggestions.
7. Receipts for WSL runs include `runner = "wsl"` and `runner_distro` if relevant.
8. Path translation should use `wsl.exe wslpath -a` when available; fall back to `/mnt/<drive>/<rest>` otherwise.
9. `wsl.exe --exec` gets discrete argv elements (no single shell string).

---

### FR-EXIT (Requirement 10): Exit Codes

**User Story:**
As a developer, I want standardized exit codes to drive automation and CI.

**Acceptance Criteria**

1. Successful runs use exit code 0.
2. CLI argument/config problems use exit code 2.
3. Packet overflow uses exit code 7.
4. Secret detection uses exit code 8.
5. Lock held uses exit code 9.
6. Phase timeout uses exit code 10.
7. Claude/Runner failures use exit code 70.
8. Every error maps to:
   - `error_kind ∈ {cli_args, packet_overflow, secret_detected, lock_held, phase_timeout, claude_failure, unknown}`.
   - `error_reason` in the receipt.
9. `exit_code` in receipts always matches the process exit code.

---

### FR-CFG (Requirement 11): Configuration

**User Story:**
As a developer, I want discoverable configuration with clear precedence.

**Acceptance Criteria**

1. Config discovery:
   - Search upward from CWD for `.xchecker/config.toml`, stopping at filesystem root or `.git`.
2. Precedence:
   - CLI flags > config file > built-in defaults.
3. `--config <path>` uses explicit file path instead of discovery.
4. Config `[runner]` section controls runner_mode, distro, phase_timeout.
5. `XCHECKER_HOME` env var overrides the state directory location.

---

### FR-BENCH (Requirement 12): Benchmark

**User Story:**
As a developer, I want benchmarks that measure wall-time and process memory.

**Acceptance Criteria**

1. `xchecker benchmark` generates deterministic workloads and measures wall time and memory.
2. Benchmarks:
   - Use one warm-up pass and N≥3 measured runs with median reported.
3. Benchmarks report process RSS on all OSs and commit_mb on Windows only.
4. Results emitted as JSON with `ok`, `timings_ms`, and `memory_bytes`.
5. Threshold comparison uses medians and configurable limits (CLI/config).
6. If thresholds exceeded:
   - `ok: false` with clear messaging.

---

### FR-FS (Requirement 13): Filesystem & Atomicity

**User Story:**
As a developer, I want atomic file writes robust to AV/locking behavior.

**Acceptance Criteria**

1. All artifact writes:
   - Temp file → fsync → atomic rename.
2. On Windows:
   - Atomic rename retries with bounded exponential backoff (≤250ms total).
3. Rename retries on Windows are recorded via `rename_retry_count` in receipt `warnings`.
4. All JSON files are UTF-8 with LF line endings.
5. Reads on Windows tolerate CRLF.

---

### FR-OBS (Requirement 14): Observability

**User Story:**
As a developer, I want structured observability without leaking secrets.

**Acceptance Criteria**

1. `--verbose` emits logs with `spec_id`, `phase`, `duration_ms`, `runner_mode`.
2. Logs never include secrets; redaction applied before logging.
3. Errors logs include actionable context but no sensitive data.

---

### FR-CACHE (Requirement 15): InsightCache

**User Story:**
As a developer, I want caching for insights so unchanged files aren't reprocessed.

**Acceptance Criteria**

1. When processing a file, InsightCache computes a BLAKE3 hash for cache keys.
2. When cached insights exist:
   - Cache validates file unchanged via size + mtime, plus content hash.
3. Changed file → cached insights invalidated and regenerated.
4. Insights generation produces 10–25 bullet points per phase.
5. Insights stored both in memory and on disk for persistence.
6. Cache usage tracked (hits/misses/invalidation) and logged in verbose mode.
7. Corrupted cache files are removed and regenerated.
8. Cache TTL is configurable; expired entries treated as misses (fail-open).
9. Cache writes follow atomic write pattern and apply redaction before persistence.

---

### FR-SOURCE (Requirement 16): SourceResolver

**User Story:**
As a developer, I want to feed specs from GitHub, filesystem, or stdin.

**Acceptance Criteria**

1. GitHub source:
   - Resolves owner, repo, and issue number.
2. Filesystem source:
   - Reads file or directory and validates existence.
3. Stdin source:
   - Reads from stdin and validates non-empty content.
4. Failure cases:
   - User-friendly errors with actionable suggestions.
5. On success:
   - Provides `SourceContent` with content + metadata about type/origin.
6. Invalid source configuration → exit code 2 with guidance.
7. Resolver deduplicates paths, applies excludes before includes.
8. Resolver enforces caps on open file count and aggregate bytes before packet assembly; exceeding caps reported as `packet_overflow` before Runner invocation.

---

### FR-PHASE (Requirement 17): Phase Trait System

**User Story:**
As a developer, I want phases implemented via traits with separated concerns.

**Acceptance Criteria**

1. For each phase, `Phase` trait separates:
   - `prompt()`,
   - `make_packet()`,
   - `postprocess()`.
2. Dependencies declared on `deps()` are enforced before execution.
3. `prompt()` uses context: spec_id, spec_dir, config, previous artifacts.
4. `make_packet()` assembles packets including relevant upstream artifacts with evidence.
5. `postprocess()` generates markdown and core YAML artifacts from LLM output.
6. Requirements, Design, and Tasks phases are supported with correct dependency ordering.
7. `build_packet()` and `postprocess()` are deterministic for a given `{inputs, config, env, cache}`.
8. `postprocess()` performs no I/O except artifact writes via FR-FS atomic writer.

---

### FR-CLI (Requirement 18): CLI Surface

**User Story:**
As a developer, I want a stable CLI surface with documented defaults.

**Acceptance Criteria**

1. CLI exposes at least:
   - `--stdout-cap-bytes`,
   - `--stderr-cap-bytes`,
   - `--packet-max-bytes`,
   - `--packet-max-lines`,
   - `--phase-timeout`,
   - `--lock-ttl-seconds`,
   - `--ignore-secret-pattern`,
   - `--extra-secret-pattern`,
   - `--debug-packet`,
   - `--allow-links`,
   - `--runner-mode`,
   - `--runner-distro`,
   - `--strict-lock`,
   - `--verbose`.
2. `--help` documents defaults and units for numeric/time flags.

---

### FR-SCHEMA (Requirement 19): JSON Schemas & Drift

**User Story:**
As a developer, I want JSON schema compliance with drift detection.

**Acceptance Criteria**

1. All emitted JSON (receipts, status, doctor, benchmark) validate against v1 schemas.
2. `receipt.v1.json`:
   - Optional fields: `stderr_redacted`, `runner_distro`, `warnings`, etc.
   - `additionalProperties: true`.
3. `status.v1.json`:
   - Optional `pending_fixups` (counts only).
4. CI fails if schema drift or stale examples are detected.

---

### FR-VLD (Requirement 20): Output Validation

**User Story:**
As a developer, I want phase outputs to be validated for quality, so I can catch meta-summaries, too-short outputs, and missing sections before they pollute downstream phases.

**Acceptance Criteria**

1. Phase outputs MAY be validated for:
   - Meta-summaries (e.g., "Here is...", "I'll create...", "This document...")
   - Minimum length per phase (Requirements: 30 lines, Design: 50, Tasks: 40, Review: 15, Fixup: 10, Final: 5)
   - Required section headers per phase
2. Validation is performed in `postprocess()` for Requirements, Design, and Tasks phases.
3. When `strict_validation = false` (default):
   - Validation issues are logged as warnings via `eprintln!`.
   - Phase execution continues.
4. When `strict_validation = true`:
   - Validation issues cause phase failure with `XCheckerError::ValidationFailed`.
   - Exit code is 1 (general error).
5. `ValidationFailed` error includes:
   - Phase name
   - List of validation issues
   - Issue count
6. `ValidationFailed` user-friendly error includes:
   - Actionable suggestions (disable strict mode, tune prompts, check output quality)
   - Context explaining strict mode behavior

---

## 4. Non-Functional Requirements (Runtime)

NFRs apply to the **runtime** (V1–V10). LLM-specific NFRs (NFR8–9) are defined in the LLM spec.

### NFR1 – Performance

- `spec --dry-run` baseline ≤ 5 seconds.
- Packetization of 100 files ≤ 200ms.
- JCS emission ≤ 50ms.

**Status:** Partially verified
- JCS emission benchmarked and well within threshold.
- Packet and dry-run benchmarks implemented but some performance tests need maintenance (struct drift) and re-execution.

---

### NFR2 – Security

- No secrets written to disk except under explicit `--debug-packet` after successful scan.
- Redaction applied before persistence.
- Path validation prevents directory traversal; symlinks/hardlinks rejected by default.
- API keys never logged or persisted.

**Status:** Verified (implementation & tests).

---

### NFR3 – Portability

- Full runtime tests pass on Linux, macOS, Windows.
- Platform-specific WSL tests pass on Windows.

**Status:** Implemented; complete CI validation depends on refreshing some tests after struct changes.

---

### NFR4 – Observability

- `--verbose` logs include phase, spec_id, duration_ms, runner_mode, etc.
- No secrets logged; redaction before output.

**Status:** Verified.

---

### NFR5 – Atomicity

- All writes use temp-file → fsync → rename.
- Windows retry logic handles transient locks.
- Same-volume constraint honored.

**Status:** Verified.

---

### NFR6 – Determinism

- JCS canonicalization produces byte-identical output for given input.
- Arrays sorted; `blake3_first8` computed on LF-normalized file content.
- Stable across platforms.

**Status:** Verified.

---

### NFR7 – Caching Efficiency

- InsightCache achieves >70% hit rate on repeated runs with unchanged files.
- Cache validation inexpensive and provides measurable speedup.

**Status:** Verified in tests; further benchmark polish is optional.

---

## 5. Scope Boundaries

**In scope:**
Everything above (FR-RUN through FR-SCHEMA, NFR1–NFR7) is **runtime**.

**Out of scope in this document:**

- FR-LLM, FR-LLM-CLI, FR-LLM-GEM, FR-LLM-API, FR-LLM-OR, FR-LLM-ANTH, FR-LLM-META.
- FR-EXEC, FR-WORKSPACE, FR-GATE, FR-TEMPLATES, FR-HOOKS, FR-SHOWCASE.
- NFR8 (LLM cost control), NFR9 (OpenRouter budgets).

Those live in **[LLM_PROVIDERS.md](LLM_PROVIDERS.md)** and represent the **next phase** of the project.

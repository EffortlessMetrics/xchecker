# Implementation Plan - Verification & Improvement Phase

**Status: CORE IMPLEMENTATION COMPLETE ✅**

This plan focuses on verification, integration, and optimization of the completed runtime implementation. Tasks are organized by functional area and aligned with the traceability matrix in the design document.

## Implementation Status Summary

| Component | Status | Wiring Needed | Key Tests |
|-----------|--------|---------------|-----------|
| Canonicalization | ✅ Complete | None | V1.1, V1.2 |
| SecretRedactor | ✅ Complete | None | V2.2, AT-SEC-003 |
| Runner | ⏳ Partial | Timeout, NDJSON, buffers | AT-RUN-004, AT-RUN-005, AT-RUN-006 |
| Orchestrator | ⏳ Partial | Phase system integration | AT-ORC-003, AT-ORC-004 |
| PacketBuilder | ✅ Complete | Wire to orchestrator | AT-PKT-004, AT-PKT-006 |
| FixupEngine | ✅ Complete | None | AT-FIX-001, AT-FIX-002, AT-FIX-003 |
| LockManager | ✅ Complete | None | V3.3, V7.5 |
| StatusManager | ✅ Complete | None | AT-STA-004 |
| InsightCache | ✅ Complete | Wire to PacketBuilder | AT-CACHE-002, AT-CACHE-003 |
| SourceResolver | ✅ Complete | Wire to CLI | AT-SRC-001, AT-SRC-002 |
| Phase System | ✅ Complete | Wire to orchestrator | AT-PHASE-001, AT-PHASE-002 |

## Quick Reference

Use the traceability matrix in `design.md` to map FR requirements → implementation → tests.

**Key Test Naming Convention**: `AT-{MODULE}-{NUMBER}` (e.g., AT-RUN-004, AT-CACHE-002)

## Critical Path (Minimum Viable Verification)

To get end-to-end functionality working:

1. **V2.4-V2.6**: Complete Runner (timeout, NDJSON, buffers) → enables AT-RUN-* tests
2. **V9.2**: Wire Phase system + PacketBuilder + SourceResolver into orchestrator → enables end-to-end flow
3. **V3.2**: Complete orchestrator execution flow with .partial/ cleanup → enables AT-ORC-* tests
4. **V8.6**: End-to-end workflow tests (Requirements → Design → Tasks) → validates full system
5. **V9.3**: Wire InsightCache into PacketBuilder → enables caching benefits

Everything else can be done in parallel or deferred.

## Release 1.0 Cut Line (Must Be Done)

**Core Wiring (Blocking):**
- [ ] 9.2 Wire PacketBuilder + Phase + SourceResolver into orchestrator
- [ ] 9.3 Wire InsightCache into PacketBuilder (can be stubbed but called)
- [ ] 8.9 Verify Phase trait system implementation

**LLM Backend (Blocking):**
- [ ] 11.0 Introduce ExecutionStrategy layer (Controlled vs ExternalTool)
- [ ] 11.1 LlmBackend abstraction (trait + factory + fallback)
- [ ] 11.2 ClaudeCliBackend (wrap existing runner) - **Skeleton S1**
- [ ] 11.3 GeminiCliBackend (non-interactive, text-only) - **Skeleton S2**
- [ ] 11.8 Config parsing for provider selection - **Skeleton S1/S2**
- [ ] 11.10 Wire LlmBackend into orchestrator - **Skeleton S1**
- [ ] 11.7 LLM metadata in receipts (minimum: provider + model + timeout) - **Skeleton S1**
- [ ] 11.14 Implement XCHECKER_SKIP_LLM_TESTS gating helper

**Post-1.0 / Optional for First Release:**
- [ ] 11.4 HTTP client module - **Skeleton S3**
- [ ] 11.5 OpenRouter backend - **Skeleton S3**
- [ ] 11.6 Anthropic backend - **Skeleton S3**
- [ ] 11.9 Doctor LLM checks (beyond "binary exists")
- [ ] 11.11 Full LLM integration tests hitting real APIs
- [ ] 11.12 Comprehensive LLM documentation

## Task Organization

- **V1-V2**: Core verification (JCS, hashing, errors, runner, packet, secrets)
- **V3-V4**: Orchestration & state (phases, locks, fixups, status)
- **V5**: Platform support (WSL, Windows, Unix)
- **V6**: Performance & observability (benchmarks, logging)
- **V7**: Edge cases & error paths
- **V8**: Integration & end-to-end testing
- **V9**: Wiring & optimization (CRITICAL PATH)
- **V10**: Final verification & release

---

## V1: Core Infrastructure Verification & Completion

**Goal**: Verify and complete core infrastructure components

- [x] **1.1 Verify and test JCS emission (FR-JCS)**


  - Test byte-identical re-serialization with different insertion orders
  - Test sorted arrays (artifacts by path, checks by name)
  - Test numeric and string normalization per RFC 8785
  - Verify receipts, status, and doctor outputs use JCS
  - Write unit tests for edge cases (empty objects, special characters, unicode)
  - _Requirements: FR-JCS-001, FR-JCS-002, FR-JCS-003, FR-JCS-004_


- [x] **1.2 Verify and test BLAKE3 hashing (FR-JCS)**


  - Test hash stability across platforms (LF line endings)
  - Test hash computation on canonicalized content
  - Test full 64-character hex format
  - Verify hashes in receipts match on-disk artifacts
  - Write unit tests for hash edge cases (empty files, large files)
  - _Requirements: FR-JCS-005, FR-JCS-006_

- [x] **1.3 Verify and test error handling (FR-EXIT)**





  - Test each ErrorKind maps to correct exit code
  - Test error_kind and error_reason in receipts
  - Test receipt written on all error paths
  - Verify exit code matches receipt exit_code field
  - Test error context and suggestions for user-friendly errors
  - Write integration tests for each error scenario
  - _Requirements: FR-EXIT-001 through FR-EXIT-009_

- [x] **1.4 Verify and test receipt management (FR-JCS, FR-EXIT)**













  - Test success receipt creation with all fields
  - Test error receipt creation with error_kind/error_reason
  - Test JCS emission for receipts
  - Test atomic write (temp → fsync → rename)
  - Test receipt reading and listing
  - Test optional fields (stderr_redacted, runner_distro, warnings)
  - Write unit tests for receipt edge cases
  - _Requirements: FR-JCS-001, FR-JCS-003, FR-EXIT-008_

- [x] **1.5 Verify and test status reporting (FR-STA)**





  - Test status generation with effective_config
  - Test source attribution (cli/config/default)
  - Test artifact enumeration with blake3 hashes
  - Test fresh spec (no prior receipts)
  - Test lock drift reporting
  - Test JCS emission for status
  - Write unit tests for status edge cases
  - _Requirements: FR-STA-001 through FR-STA-005_

- [x] **1.6 Verify and test configuration system (FR-CFG)**








  - Test upward discovery stopping at .git
  - Test precedence: CLI > config > defaults
  - Test source attribution accuracy
  - Test XCHECKER_HOME override
  - Test --config explicit path
  - Test invalid config handling
  - Write unit tests for config edge cases
  - _Requirements: FR-CFG-001 through FR-CFG-005_

- [x] **1.7 Verify and test lockfile system (FR-LOCK)**





  - Test lockfile creation with --create-lock
  - Test drift detection for each field
  - Test --strict-lock enforcement
  - Test no drift when values match
  - Test lockfile loading and validation
  - Write unit tests for lockfile edge cases
  - _Requirements: FR-LOCK-006, FR-LOCK-007, FR-LOCK-008_

- [x] **1.8 Wire global CLI flags**



  - Verify all flags in build_cli() are functional
  - Test --runner-mode, --runner-distro, --phase-timeout
  - Test --ignore-secret-pattern, --extra-secret-pattern
  - Test --debug-packet, --force, --allow-links
  - Test --packet-max-bytes, --packet-max-lines
  - Test --verbose flag with structured logging
  - _Requirements: All FR-*_

---

## V2: Runner & Packet Implementation & Testing

**Goal**: Complete and test process control and packet assembly

- [x] **2.1 Implement and test packet builder (FR-PKT)**





  - Verify deterministic ordering (sorted file paths)
  - Test priority-based selection (Upstream > High > Medium > Low)
  - Test LIFO ordering within priority classes
  - Test byte and line counting during assembly
  - Test limit enforcement (exit 7 on overflow)
  - Test packet manifest generation on overflow
  - Test --debug-packet flag (writes full packet after secret scan)
  - Verify packet preview always written to context/
  - Write unit tests for ContentSelector priority assignment
  - Write integration tests for packet construction
  - _Requirements: FR-PKT-001 through FR-PKT-007_



- [x] **2.2 Implement and test secret redaction (FR-SEC)**



  - Test all default patterns (GitHub PAT, AWS keys, Slack, Bearer)
  - Test --extra-secret-pattern adds custom patterns
  - Test --ignore-secret-pattern suppresses patterns
  - Test redaction replaces matches with ***
  - Test exit code 8 on secret detection
  - Test secrets in file paths (redacted in receipts/logs)
  - Test secrets in error messages (redacted before persistence)
  - Test secrets in stderr (redacted before truncation)
  - Verify receipts never include env vars or raw packet content
  - Write unit tests for each pattern
  - Write integration tests for secret detection scenarios
  - _Requirements: FR-SEC-001 through FR-SEC-006_

- [x] **2.3 Implement and test runner execution (FR-RUN)**
  - **Note**: Runner will be refactored to support any CLI-based LLM provider (not just Claude)
  - Test native mode spawns CLI binary directly
  - Test WSL mode uses `wsl.exe --exec` with discrete argv
  - Test auto mode: native first, then WSL fallback on Windows
  - Test runner_distro captured from wsl -l -q or $WSL_DISTRO_NAME
  - Test stdin piping to CLI process
  - Test stdout/stderr capture
  - Write unit tests for runner mode detection
  - Write integration tests for each runner mode
  - _Requirements: FR-RUN-001, FR-RUN-002, FR-RUN-003_

- [x] **2.4 Implement and test timeout enforcement (FR-RUN)**





  - Implement tokio::time::timeout wrapper
  - Implement graceful TERM (wait 5s) then KILL sequence
  - Implement Windows Job Objects for process tree termination
  - Implement Unix killpg for process group termination
  - Test timeout triggers after configured duration (default 600s, min 5s)
  - Test exit code 10 on timeout
  - Test receipt written with phase_timeout error kind
  - Test stderr_redacted field populated
  - Test pipes drained even after timeout
  - Write integration tests for timeout scenarios
  - _Requirements: FR-RUN-004, FR-RUN-005, FR-RUN-006, FR-RUN-007_

- [x] **2.5 Implement and test NDJSON merging (FR-RUN)**





  - Implement line-by-line stdout parsing
  - Implement JSON validation per line
  - Implement last valid JSON object selection
  - Implement non-JSON line ignoring
  - Test interleaved noise + multiple JSON frames (AT-RUN-004)
  - Test partial JSON followed by timeout (AT-RUN-005)
  - Test no valid JSON → claude_failure with excerpt (256 chars, redacted)
  - Write unit tests for NDJSON parsing
  - Write integration tests for stream merging scenarios
  - _Requirements: FR-RUN-008, FR-RUN-009_

- [x] **2.6 Implement and test output buffering (FR-RUN)**




  - Implement ring buffer for stdout (cap at 2 MiB)
  - Implement ring buffer for stderr (cap at 256 KiB)
  - Implement stderr redaction before truncation to 2048 bytes
  - Test large stdout stream > 2 MiB (AT-RUN-006)
  - Test stderr truncation and redaction
  - Verify InvocationResult.stderr_redacted ≤ 2048 bytes
  - Write unit tests for buffer management
  - _Requirements: FR-RUN-010_

- [x] **2.7 Wire PacketBuilder into orchestrator (FR-PKT, FR-ORC)**










  - Replace placeholder packet building in orchestrator
  - Integrate secret scanning before Claude invocation
  - Populate PacketEvidence in receipts with file list
  - Test end-to-end packet → Claude → receipt flow
  - Verify packet overflow fails before Claude invocation
  - Verify secret detection fails before Claude invocation
  - _Requirements: FR-PKT, FR-SEC, FR-ORC-003_

---

## V3: Orchestrator & Phase Coordination Implementation & Testing

**Goal**: Complete and test phase execution and state management

- [x] **3.1 Implement and test phase orchestration (FR-ORC)**









  - Implement phase transition validation (Requirements → Design → Tasks → Review → Fixup → Final)
  - Implement illegal transition detection with actionable guidance (exit 2)
  - Implement dependency checking before phase execution
  - Test each legal transition
  - Test illegal transitions return exit 2 with guidance
  - Test dependency not satisfied returns error
  - Write unit tests for transition validation logic
  - Write integration tests for phase execution flow
  - _Requirements: FR-ORC-001, FR-ORC-002_

- [x] **3.2 Implement and test phase execution flow (FR-ORC)**





  - Implement Step 0: Remove stale .partial/ directories
  - Implement Step 1: Validate transition
  - Implement Step 2: Acquire exclusive lock
  - Implement Step 3: Build packet
  - Implement Step 4: Scan for secrets
  - Implement Step 5: Enforce packet limits
  - Implement Step 6: Invoke Runner
  - Implement Step 7: Write partial artifact to .partial/
  - Implement Step 8: Promote to final (atomic rename)
  - Implement Step 9: Write receipt (success or error)
  - Test stale .partial/ cleanup (AT-ORC-003)
  - Test atomic promotion of artifacts
  - Test receipt emission for success and error
  - _Requirements: FR-ORC-003, FR-ORC-004, FR-ORC-005, FR-ORC-006_

- [x] **3.3 Implement and test lock management (FR-LOCK)**





  - Implement advisory lock file creation with {pid, host, started_at}
  - Implement lock acquisition (exit 9 if held by active process)
  - Implement stale detection (PID not alive OR age > TTL)
  - Implement --force flag to break stale locks
  - Implement lock release on normal exit
  - Implement best-effort lock release on panic
  - Test lock acquisition and release
  - Test concurrent execution (exit 9)
  - Test stale lock detection (PID not alive)
  - Test stale lock detection (age > TTL)
  - Test --force breaking stale lock with warning in receipt
  - Test configurable TTL parameter
  - Write unit tests for stale detection logic
  - Write integration tests for lock scenarios
  - _Requirements: FR-LOCK-001 through FR-LOCK-005_
-

- [x] **3.4 Implement and test atomic file operations (FR-FS)**




  - Implement write to temporary file with fsync
  - Implement atomic rename (same filesystem)
  - Implement Windows rename retry with exponential backoff (≤ 250ms total)
  - Implement cross-filesystem fallback (copy→fsync→replace)
  - Implement rename_retry_count tracking in warnings
  - Test temp file creation and fsync
  - Test atomic rename success
  - Test Windows rename retry (platform-specific)
  - Test cross-filesystem fallback (AT-FS-004)
  - Test warning recorded when retries occur
  - Test UTF-8 encoding with LF line endings
  - Test CRLF tolerance on read (Windows)
  - Write unit tests for atomic write logic
  - Write integration tests for file operations
  - _Requirements: FR-FS-001, FR-FS-002, FR-FS-003, FR-FS-004, FR-FS-005_

- [x] **3.5 Implement and test resume functionality (FR-ORC)**





  - Implement resume() method with transition validation
  - Implement current state detection from last receipt
  - Implement partial artifact detection and handling
  - Test resume from each phase
  - Test resume with partial artifacts (delete and restart)
  - Test resume with missing dependencies (error)
  - Test resume with failed dependency receipt (error)
  - Write integration tests for resume scenarios
  - _Requirements: FR-ORC-002, FR-ORC-003_

- [x] **3.6 Implement timeout handling in orchestrator (FR-ORC, FR-RUN)**





  - Implement PhaseTimeout configuration (default 600s, min 5s)
  - Implement execute_phase_with_timeout wrapper
  - Implement handle_phase_timeout for partial artifact creation
  - Test timeout during phase execution
  - Test partial artifact written on timeout
  - Test receipt with phase_timeout error kind
  - Test exit code 10 on timeout
  - Write integration tests for timeout scenarios
  - _Requirements: FR-RUN-004, FR-RUN-007, FR-ORC-005_

---

## V4: Fixup & Status Implementation & Testing

**Goal**: Complete and test file modification and reporting systems

- [x] **4.1 Implement and test fixup plan structures (FR-FIX)**





  - Implement UnifiedDiff struct with target_file, hunks
  - Implement DiffHunk struct with old_range, new_range, lines
  - Implement FixupPreview struct with target_files, change_summaries
  - Implement FixupResult struct with applied_files, warnings
  - Implement ChangeSummary struct with hunk_count, added, removed
  - Test unified diff parsing from review output
  - Test hunk extraction and validation
  - Write unit tests for diff parsing
  - _Requirements: FR-FIX-001_

- [x] **4.2 Implement and test fixup path validation (FR-FIX)**





  - Implement path canonicalization
  - Implement root boundary checking
  - Implement .. component rejection
  - Implement absolute path outside root rejection
  - Implement symlink detection with lstat
  - Implement hardlink detection
  - Implement --allow-links flag support
  - Test path traversal rejection (AT-FIX-003)
  - Test absolute paths outside root rejection
  - Test symlink rejection (default)
  - Test hardlink rejection (default)
  - Test --allow-links flag allows symlinks/hardlinks
  - Write unit tests for path validation logic
  - Write integration tests for path validation scenarios
  - _Requirements: FR-FIX-001, FR-FIX-002, FR-FIX-003_

- [x] **4.3 Implement and test fixup preview mode (FR-FIX)**









  - Implement preview() method that shows intended changes
  - Implement target file listing
  - Implement estimated line change calculation (added/removed)
  - Implement validation warning display
  - Implement no file system modifications
  - Test preview output format
  - Test no file modifications (AT-FIX-001)
  - Test intended targets shown
  - Test estimated line changes shown
  - Test validation warnings displayed
  - Test receipt includes targets with applied: false
  - Write unit tests for preview logic
  - Write integration tests for preview mode
  - _Requirements: FR-FIX-004, FR-FIX-009_



- [x] **4.4 Implement and test fixup apply mode (FR-FIX)**






  - Implement apply() method with atomic writes
  - Implement write to .tmp files with fsync
  - Implement .bak backup creation if file exists
  - Implement atomic rename with Windows retry
  - Implement file mode bit preservation (Unix)
  - Implement file attribute preservation (Windows)
  - Implement warning recording if permission preservation fails
  - Test .bak files created (AT-FIX-002)
  - Test atomic rename success
  - Test Windows rename retry
  - Test permission preservation
  - Test warnings recorded for permission issues
  - Test applied files in receipt with blake3_first8 and applied: true
  - Write unit tests for apply logic
  - Write integration tests for apply mode
  - _Requirements: FR-FIX-005, FR-FIX-006, FR-FIX-008_

- [x] **4.5 Implement and test cross-filesystem fallback (FR-FIX)**





  - Implement same-filesystem detection
  - Implement fallback: copy→fsync→replace
  - Implement original removal only after successful fsync+close
  - Implement warning recording in AppliedChange.warnings
  - Test cross-filesystem fallback (AT-FS-004)
  - Test warning recorded when fallback used
  - Test original file removed only after success
  - Write unit tests for filesystem detection
  - Write integration tests for cross-filesystem scenarios
  - _Requirements: FR-FIX-007_

-

- [x] **4.6 Implement and test line ending normalization (FR-FIX)**



  - Implement line ending normalization before diff calculation
  - Implement LF enforcement for JSON and text artifacts
  - Implement CRLF tolerance on read (Windows)
  - Test diff estimates with mixed line endings
  - Test LF written to disk
  - Test CRLF read correctly
  - Write unit tests for line ending handling
  - _Requirements: FR-FIX-010, FR-FS-004, FR-FS-005_
-

- [x] **4.7 Wire fixup command (FR-FIX)**




  - Implement resume --phase fixup command parsing
  - Implement --apply-fixups flag handling
  - Implement review output loading
  - Implement FixupPlan derivation from review
  - Implement plan validation
  - Implement preview() call when no --apply-fixups
  - Implement apply() call when --apply-fixups set
  - Implement receipt writing with fixup results
  - Test fixup preview (default behavior)
  - Test fixup apply with --apply-fixups
  - Write integration tests for fixup command
  - _Requirements: FR-FIX-001 through FR-FIX-010_

- [x] **4.8 Implement and test status manager (FR-STA)**





  - Implement StatusManager::generate_status_from_orchestrator()
  - Implement artifact collection with blake3 hashes
  - Implement effective_config building with source attribution
  - Implement lock drift detection and reporting
  - Implement pending fixup summary (counts only)
  - Implement JCS emission for status output
  - Test fresh spec (no prior receipts) (AT-STA-002)
  - Test effective_config source attribution (cli/config/default)
  - Test artifact enumeration with blake3_first8
  - Test lock drift reporting
  - Test pending fixup summary
  - Test JCS canonical output
  - Write unit tests for status building
  - Write integration tests for status scenarios
  - _Requirements: FR-STA-001 through FR-STA-005_

- [x] **4.9 Wire status command (FR-STA)**





  - Implement status --json command parsing
  - Implement EffectiveConfig building with source attribution
  - Implement artifact enumeration
  - Implement blake3_first8 computation
  - Implement lockfile loading and drift computation
  - Implement StatusOutput building
  - Implement JCS emission
  - Test status command output
  - Test --json flag
  - Write integration tests for status command
  - _Requirements: FR-STA-001 through FR-STA-005_

---

## V5: Platform Support & WSL Implementation & Testing

**Goal**: Complete and test cross-platform support and WSL integration

- [x] **5.1 Implement and test WSL detection (FR-WSL)**





  - Implement is_wsl_available() checking `wsl.exe -l -q`
  - Implement distro list parsing
  - Implement verification of at least one installed distribution
  - Test WSL detection on Windows
  - Test WSL not available error handling
  - Test distro list parsing
  - Write unit tests for WSL detection logic
  - Write integration tests for WSL availability (Windows only)
  - _Requirements: FR-WSL-001_

- [x] **5.2 Implement and test Claude validation in WSL (FR-WSL)**





  - Implement validate_claude_in_wsl() method
  - Implement `wsl.exe -d <distro> -- which claude` execution
  - Implement return true if Claude discoverable
  - Implement return false if Claude not found
  - Test Claude validation in WSL (Windows only)
  - Test Claude not discoverable handling
  - Test distro-specific validation
  - Write unit tests for validation logic
  - Write integration tests for Claude detection (Windows only)
  - _Requirements: FR-WSL-002, FR-WSL-003_

- [x] **5.3 Implement and test WSL path translation (FR-WSL)**




  - Implement translate_win_to_wsl() method
  - Implement `wsl.exe wslpath -a` for correctness
  - Implement drive letter translation (C:\ → /mnt/c/)
  - Implement UNC path handling
  - Implement fallback heuristic if wslpath fails
  - Test drive letter translation (Windows only)
  - Test UNC path translation (Windows only)
  - Test wslpath usage
  - Test fallback heuristic
  - Write unit tests for path translation logic
  - Write integration tests for path translation (Windows only)
  - _Requirements: FR-WSL-004_


- [x] **5.4 Implement and test WSL environment translation (FR-WSL)**





  - Implement translate_env_for_wsl() method
  - Implement path adaptation in environment variables
  - Implement context preservation
  - Test environment variable translation (Windows only)
  - Test path variables adapted correctly
  - Write unit tests for env translation logic
  - Write integration tests for env translation (Windows only)
  - _Requirements: FR-WSL-005_
-

- [x] **5.5 Implement and test WslRunner (FR-WSL)**













  - Implement WslRunner struct
  - Implement path translation before invocation
  - Implement env var translation before invocation
  - Implement `wsl.exe --exec` with discrete argv elements
  - Implement artifact persistence in Windows spec root
  - Implement runner_distro capture
  - Test WSL execution (Windows only)
  - Test path translation in invocation
  - Test env var translation in invocation
  - Test artifact persistence in Windows root
  - Test runner_distro in receipts
  - Write unit tests for WslRunner
  - Write integration tests for WSL execution (Windows only)
  - _Requirements: FR-WSL-001 through FR-WSL-005, FR-WSL-007_

- [x] **5.6 Implement and test runner factory auto mode (FR-WSL)**









  - Implement auto mode detection logic
  - Implement native Claude detection first
  - Implement WSL fallback on Windows if native missing
  - Implement WSL availability check
  - Implement Claude in WSL check
  - Test auto mode on Windows (native available)
  - Test auto mode on Windows (native missing, WSL available)
  - Test auto mode on Windows (neither available)
  - Test auto mode on Linux/macOS (always native)
  - Write unit tests for auto detection logic
  - Write integration tests for auto mode (Windows only)
  - _Requirements: FR-WSL-003, FR-RUN-003_

- [x] **5.7 Implement and test doctor WSL checks (FR-WSL)**



  - Implement native claude --version check
  - Implement WSL availability reporting (list distros)
  - Implement which claude inside selected distro check
  - Implement actionable suggestions when native missing but WSL ready
  - Test doctor on Windows with native Claude
  - Test doctor on Windows without native Claude
  - Test doctor on Windows with WSL available
  - Test doctor on Windows without WSL
  - Test actionable suggestions displayed
  - Write integration tests for doctor WSL checks (Windows only)
  - _Requirements: FR-WSL-006_

- [x] **5.8 Implement and test Windows Job Objects (FR-RUN)**





  - Implement Job Object creation on Windows
  - Implement process assignment to Job Object
  - Implement job termination on timeout
  - Implement child process termination via job
  - Test Job Object creation (Windows only)
  - Test process tree termination (Windows only)
  - Test timeout with Job Objects (Windows only)
  - Write unit tests for Job Object logic (Windows only)
  - Write integration tests for process termination (Windows only)
  - _Requirements: FR-RUN-006_

- [x] **5.9 Implement and test Unix process group termination (FR-RUN)**





  - Implement killpg for process group termination
  - Implement TERM signal sending
  - Implement KILL signal sending after grace period
  - Test process group termination (Linux/macOS only)
  - Test TERM then KILL sequence (Linux/macOS only)
  - Write integration tests for process termination (Linux/macOS only)
  - _Requirements: FR-RUN-005_

- [x] **5.10 Test cross-platform line ending handling (FR-FS)**





  - Test CRLF tolerance on read (Windows)
  - Test LF enforcement on write (all platforms)
  - Test line ending normalization
  - Test JSON files written with LF
  - Test text files written with LF
  - Write unit tests for line ending handling
  - Write integration tests for cross-platform files
  - _Requirements: FR-FS-004, FR-FS-005_

---

## V6: Performance & Observability Implementation & Testing

**Goal**: Complete and test performance monitoring and logging

- [x] **6.1 Implement and test benchmark workload generation (FR-BENCH)**





  - Implement deterministic workload generation
  - Implement warm-up pass execution
  - Implement N>=3 measured runs
  - Implement timing measurement
  - Test workload generation creates consistent files
  - Test warm-up pass executes
  - Test measured runs execute
  - Write unit tests for workload generation
  - _Requirements: FR-BENCH-001, FR-BENCH-002_


- [x] **6.2 Implement and test process memory tracking (FR-BENCH)**




  - Implement sysinfo crate integration
  - Implement process RSS measurement (all OSs)
  - Implement commit_mb measurement (Windows only)
  - Implement process-scoped metrics (not system totals)
  - Test RSS measurement
  - Test commit_mb measurement (Windows only)
  - Test process-scoped vs system-wide distinction
  - Write unit tests for memory tracking
  - _Requirements: FR-BENCH-003_

- [x] **6.3 Implement and test benchmark results (FR-BENCH)**





  - Implement BenchmarkResults struct with ok, timings_ms, rss_mb, commit_mb
  - Implement median calculation from N runs
  - Implement threshold comparison
  - Implement ok: false when threshold exceeded
  - Implement configurable thresholds via CLI or config
  - Test median calculation
  - Test threshold comparison
  - Test ok: false on threshold failure
  - Test configurable thresholds
  - Write unit tests for results calculation
  - _Requirements: FR-BENCH-004, FR-BENCH-005, FR-BENCH-006_

- [x] **6.4 Wire benchmark command (FR-BENCH)**





  - Implement benchmark command parsing
  - Implement threshold override via CLI
  - Implement benchmark execution
  - Implement structured JSON output
  - Test benchmark command
  - Test threshold overrides
  - Test JSON output format
  - Write integration tests for benchmark command
  - _Requirements: FR-BENCH-001 through FR-BENCH-006_

- [x] **6.5 Run and verify performance benchmarks (NFR1)**





  - Run `spec --dry-run` baseline benchmark
  - Verify ≤ 5s target met
  - Run packetization of 100 files benchmark
  - Verify ≤ 200ms target met
  - Run JCS emission benchmark
  - Verify ≤ 50ms target met
  - Document results in CHANGELOG
  - Optimize if targets not met
  - _Requirements: NFR1_

- [x] **6.6 Implement and test structured logging (FR-OBS)**





  - Implement tracing subscriber setup
  - Implement env filter configuration
  - Implement compact human-readable format (default)
  - Implement --verbose format with spec_id, phase, duration_ms, runner_mode
  - Test default log format
  - Test --verbose includes required fields
  - Test log output structure
  - Write unit tests for logging configuration
  - _Requirements: FR-OBS-001_

- [x] **6.7 Implement and test redaction in logging (FR-OBS)**





  - Implement SecretScanner::redact() application before logging
  - Implement environment variable exclusion from logs
  - Implement secret redaction in error messages
  - Implement secret redaction in context
  - Test no secrets in log output
  - Test no environment variables in logs
  - Test error context without sensitive data
  - Write unit tests for log redaction
  - Write integration tests for logging scenarios
  - _Requirements: FR-OBS-002, FR-OBS-003_

- [x] **6.8 Add final redaction pass to all user-facing strings (FR-SEC, FR-OBS)**





  - Audit all error messages for redaction
  - Audit all context strings for redaction
  - Audit all preview text for redaction
  - Implement redaction before display or persistence
  - Verify receipts never include env vars or raw packet content
  - Test redaction in error messages
  - Test redaction in context strings
  - Test redaction in preview text
  - Write integration tests for redaction coverage
  - _Requirements: FR-SEC-006, FR-OBS-002_

---

## V7: Edge Cases & Error Handling Implementation & Testing

**Goal**: Implement and test edge cases and error scenarios

- [x] **7.1 Implement and test error receipt generation (FR-EXIT)**





  - Implement error_to_exit_code_and_kind() mapping function
  - Implement write_error_receipt_and_exit() function
  - Implement ReceiptManager::create_error_receipt() method
  - Ensure exit_code field matches process exit code
  - Ensure error_kind and error_reason populated
  - Test CliArgs error → exit 2, error_kind: cli_args
  - Test PacketOverflow error → exit 7, error_kind: packet_overflow
  - Test SecretDetected error → exit 8, error_kind: secret_detected
  - Test Lock error → exit 9, error_kind: lock_held
  - Test PhaseTimeout error → exit 10, error_kind: phase_timeout
  - Test Claude/Runner error → exit 70, error_kind: claude_failure
  - Test Unknown error → exit 1, error_kind: unknown
  - Test receipt written on all error paths
  - Verify exit code matches receipt exit_code field
  - Write unit tests for error mapping
  - Write integration tests for each error scenario
  - _Requirements: FR-EXIT-001 through FR-EXIT-009_

- [x] **7.2 Implement and test secret redaction in error paths (FR-SEC)**





  - Implement redaction in error_reason before receipt write
  - Implement redaction in stderr_tail before receipt write
  - Implement redaction in context lines before receipt write
  - Implement redaction in warning messages before receipt write
  - Test secret in error_reason (AT-SEC-003)
  - Test secret in stderr_tail
  - Test secret in context lines
  - Test secret in warning messages
  - Verify all redacted before persistence
  - Verify receipts never include actual secrets
  - Write unit tests for error path redaction
  - Write integration tests for secret in errors
  - _Requirements: FR-SEC-005, FR-SEC-006_

- [x] **7.3 Implement and test packet overflow scenarios (FR-PKT)**





  - Implement upstream file budget checking
  - Implement failure before Claude invocation on overflow
  - Implement manifest writing on overflow
  - Implement receipt with actual size and limits
  - Test upstream files alone exceed budget
  - Test regular files excluded when budget reached
  - Test failure before Claude invocation
  - Test manifest written to context/<phase>-packet.manifest.json
  - Test receipt includes used_bytes, used_lines, limit_bytes, limit_lines
  - Test exit code 7 on overflow
  - Write unit tests for overflow detection
  - Write integration tests for overflow scenarios
  - _Requirements: FR-PKT-002, FR-PKT-003, FR-PKT-004, FR-PKT-005_

- [x] **7.4 Implement and test phase timeout scenarios (FR-RUN, FR-ORC)**





  - Implement timeout detection at different stages
  - Implement partial artifact saving on timeout
  - Implement timeout receipt generation
  - Test timeout during packet building
  - Test timeout during Claude execution
  - Test timeout during artifact writing
  - Test partial artifacts saved with .partial.md extension
  - Test receipt with phase_timeout error kind
  - Test exit code 10 on timeout
  - Test warning includes timeout duration
  - Write integration tests for timeout at each stage
  - _Requirements: FR-RUN-004, FR-RUN-007, FR-ORC-005_

- [x] **7.5 Implement and test concurrent execution prevention (FR-LOCK)**





  - Implement lock acquisition check
  - Implement active process detection (PID alive check)
  - Implement lock release on normal exit
  - Implement best-effort lock release on panic (Drop trait)
  - Test two processes trying to acquire same lock
  - Test lock held by active process (exit 9)
  - Test lock released on normal exit
  - Test lock cleanup on panic (best-effort)
  - Test lock file contains correct pid, host, started_at
  - Write unit tests for lock acquisition logic
  - Write integration tests for concurrent execution
  - _Requirements: FR-LOCK-001, FR-LOCK-002, FR-LOCK-005_

- [x] **7.6 Implement and test --debug-packet behavior (FR-PKT)**





  - Implement --debug-packet flag handling
  - Implement full packet write after secret scan passes
  - Implement packet write to context/<phase>-packet.txt
  - Implement exclusion from receipts
  - Implement no write if any secret rule fires
  - Test --debug-packet writes full packet
  - Test packet not written if secrets detected
  - Test packet file not cross-linked in receipts
  - Test packet content redacted if later reported
  - Write integration tests for --debug-packet scenarios
  - _Requirements: FR-PKT-006, FR-PKT-007_

- [x] **7.7 Implement and test empty input handling (FR-PKT, FR-SEC)**





  - Implement empty packet handling
  - Implement empty file handling
  - Implement no files selected handling
  - Test empty packet generation
  - Test empty file in packet
  - Test no files selected scenario
  - Test secret scanning on empty content
  - Write unit tests for empty input cases
  - _Requirements: FR-PKT, FR-SEC_

- [x] **7.8 Implement and test large file handling (FR-PKT, FR-RUN)**





  - Implement large file detection
  - Implement file size limits
  - Implement ring buffer for large stdout
  - Implement ring buffer for large stderr
  - Test files exceeding packet budget
  - Test stdout > 2 MiB (ring buffer)
  - Test stderr > 256 KiB (ring buffer)
  - Test truncation in receipts (stderr ≤ 2048 bytes)
  - Write integration tests for large file scenarios
  - _Requirements: FR-PKT, FR-RUN-010_

---

## V8: Integration & Smoke Tests Implementation & Testing

**Goal**: Implement and test end-to-end workflows and CI integration

- [x] **8.1 Implement and run smoke tests (All FR-*)**





  - Implement smoke test: xchecker doctor --json
  - Implement smoke test: xchecker init demo --create-lock
  - Implement smoke test: xchecker spec demo --dry-run
  - Implement smoke test: xchecker status demo --json
  - Implement smoke test: xchecker clean demo --hard
  - Implement smoke test: xchecker benchmark
  - Run all smoke tests locally
  - Verify all commands succeed
  - Verify JSON output is valid
  - Verify exit codes are correct
  - Write integration tests for smoke test scenarios
  - Add smoke tests to CI pipeline
  - _Requirements: All FR-*_

- [x] **8.2 Implement and verify schema compliance (FR-JCS)**





  - Implement schema example generators (already in example_generators.rs)
  - Regenerate all schema examples
  - Verify receipt.v1.minimal.json validates against schema
  - Verify receipt.v1.full.json validates against schema
  - Verify status.v1.minimal.json validates against schema
  - Verify status.v1.full.json validates against schema
  - Verify doctor.v1.minimal.json validates against schema
  - Verify doctor.v1.full.json validates against schema
  - Check for schema drift
  - Verify additionalProperties: true in all schemas
  - Verify optional fields documented correctly
  - Write tests for schema validation
  - Add schema validation to CI
  - _Requirements: FR-JCS-001, FR-JCS-003_

- [x] **8.3 Update schema files with all optional fields (FR-JCS)**





  - Add stderr_redacted (optional) to receipt.v1.json
  - Add runner_distro (optional) to receipt.v1.json
  - Add error_kind (optional) to receipt.v1.json
  - Add error_reason (optional) to receipt.v1.json
  - Add warnings (optional) to receipt.v1.json
  - Add fallback_used (optional) to receipt.v1.json
  - Add diff_context (optional) to receipt.v1.json
  - Add pending_fixups (optional) to status.v1.json
  - Add lock_drift (optional) to status.v1.json
  - Confirm additionalProperties: true in all schemas
  - Regenerate examples after schema updates
  - Verify examples still validate
  - _Requirements: FR-JCS-003, FR-STA-001_

- [x] **8.4 Run full test suite on all platforms (NFR3)**








  - Run unit tests on Linux
  - Run unit tests on macOS
  - Run unit tests on Windows
  - Run integration tests on Linux
  - Run integration tests on macOS
  - Run integration tests on Windows
  - Run platform-specific tests on Windows (WSL, Job Objects)
  - Run platform-specific tests on Linux/macOS (killpg)
  - Verify all tests pass on all platforms
  - Verify CI passes on all platforms
  - Document any platform-specific issues
  - _Requirements: NFR3_

- [x] **8.5 Implement and verify documentation accuracy (All FR-*)**





  - Implement doc_validation tests (already exists)
  - Run doc_validation tests
  - Verify CLI flags match README
  - Verify exit codes match README
  - Verify configuration examples parse correctly
  - Verify code examples execute successfully
  - Verify enum values match between schemas and Rust
  - Verify required fields match between schemas and Rust
  - Fix any documentation mismatches
  - Add documentation validation to CI
  - _Requirements: All FR-*_

- [x] **8.6 Implement end-to-end workflow tests (All FR-*)**





  - Implement test: full spec generation (Requirements → Design → Tasks)
  - Implement test: resume from each phase
  - Implement test: fixup preview and apply
  - Implement test: status reporting at each stage
  - Implement test: error recovery and partial artifacts
  - Implement test: lock conflict and resolution
  - Implement test: timeout and recovery
  - Implement test: secret detection and blocking
  - Implement test: packet overflow and manifest
  - Run all end-to-end tests
  - Verify workflows complete successfully
  - Write integration tests for complete workflows
  - _Requirements: All FR-*_

- [x] **8.7 Verify InsightCache implementation (FR-CACHE)**





  - Test cache creation and initialization
  - Test BLAKE3 content hash calculation
  - Test cache key generation ({hash}_{phase})
  - Test memory cache hit/miss
  - Test disk cache persistence
  - Test file change detection (size, mtime)
  - Test cache invalidation on file change
  - Test insight generation (10-25 bullet points)
  - Test phase-specific insights (requirements, design, tasks, review)
  - Test cache statistics tracking
  - Test cache statistics logging
  - Verify all unit tests pass
  - _Requirements: FR-CACHE-001 through FR-CACHE-007_

- [x] **8.8 Verify SourceResolver implementation (FR-SOURCE)**





  - Test GitHub source resolution
  - Test GitHub validation (owner, repo, issue_id)
  - Test filesystem source resolution (files)
  - Test filesystem source resolution (directories)
  - Test filesystem validation (path exists)
  - Test stdin source resolution
  - Test stdin validation (non-empty)
  - Test error handling with user-friendly messages
  - Test actionable suggestions in errors
  - Test source metadata tracking
  - Verify all unit tests pass
  - _Requirements: FR-SOURCE-001 through FR-SOURCE-006_

- [x] **8.9 Verify Phase trait system implementation (FR-PHASE)**





  - Test Phase trait methods (id, deps, can_resume, prompt, make_packet, postprocess)
  - Test PhaseContext building
  - Test Packet assembly with evidence
  - Test BudgetUsage tracking
  - Test PhaseResult with artifacts and next step
  - Test RequirementsPhase implementation
  - Test DesignPhase implementation and dependency on Requirements
  - Test TasksPhase implementation and dependency on Design
  - Test artifact generation (markdown + core YAML)
  - Test prompt generation with context
  - Test packet assembly with previous artifacts
  - Test postprocessing into structured artifacts
  - Verify all unit tests pass
  - _Requirements: FR-PHASE-001 through FR-PHASE-006_

---

## V9: Cleanup & Optimization Implementation

**Goal**: Remove TODOs, wire components, optimize performance, improve code quality

- [x] **9.1 Remove staged module TODOs and wire components**





  - Remove `#![allow(dead_code, unused_imports)]` from packet.rs
  - Remove `#![allow(dead_code, unused_imports)]` from redaction.rs
  - Remove `#![allow(dead_code, unused_imports)]` from canonicalization.rs
  - Remove `#![allow(dead_code, unused_imports)]` from fixup.rs
  - Remove `#![allow(dead_code, unused_imports)]` from logging.rs
  - Remove `#![allow(dead_code, unused_imports)]` from lock.rs
  - Remove `#![allow(dead_code, unused_imports)]` from cache.rs
  - Remove `#![allow(dead_code, unused_imports)]` from source.rs
  - Remove `#![allow(dead_code, unused_imports)]` from phases.rs
  - **Note**: Module-local `#[allow(dead_code)]` on specific items is acceptable when supporting future providers (e.g., extra variants or helper types for HTTP backends)
  - Remove TODO(M2) comments from all modules
  - Verify no compiler warnings after removal
  - Verify all modules are actually used
  - _Requirements: All FR-*_

- [x] **9.2 Wire PacketBuilder and Phase system into orchestrator (FR-PKT, FR-SEC, FR-ORC, FR-PHASE, FR-SOURCE)**





  - Replace placeholder packet building in execute_phase()
  - Integrate Phase trait system into orchestrator
  - Implement phase factory (get_phase_impl() method)
  - Integrate PhaseContext building
  - Integrate phase.prompt() call
  - Integrate phase.make_packet() call
  - Integrate phase.postprocess() call
  - Integrate PacketBuilder::build_packet() call
  - Integrate secret scanning before Claude invocation
  - Integrate SourceResolver for problem statement input
  - Populate PacketEvidence in receipts with actual file list
  - Populate packet.files with FileEvidence structs
  - Include blake3_pre_redaction hashes
  - Include priority levels
  - Test end-to-end source → phase → packet → Claude → receipt flow
  - Test Requirements → Design → Tasks phase progression
  - Test phase dependency enforcement
  - Test packet overflow fails before Claude invocation
  - Test secret detection fails before Claude invocation
  - Verify PacketEvidence in receipts is accurate
  - Verify artifacts generated (markdown + core YAML)
  - _Requirements: FR-PKT, FR-SEC, FR-ORC-003, FR-PHASE, FR-SOURCE_

- [x] **9.3 Wire InsightCache into PacketBuilder (FR-CACHE)**





  - Integrate InsightCache into PacketBuilder
  - Implement cache initialization with cache_dir
  - Implement cache hit/miss logic in packet assembly
  - Implement insight generation for cache misses
  - Implement cache storage after insight generation
  - Implement cache statistics logging
  - Test cache hit returns cached insights
  - Test cache miss generates and stores insights
  - Test cache invalidation on file change
  - Test cache performance improvement (>50% speedup)
  - Test cache hit rate >70% on repeated runs
  - Verify cache statistics in verbose logging
  - _Requirements: FR-CACHE, NFR7_

- [x] **9.4 Optimize packet assembly performance (NFR1)**





  - Profile packetization of 100 files
  - Identify performance bottlenecks
  - Optimize file reading (parallel reads if beneficial)
  - Optimize BLAKE3 hashing (reuse hasher if beneficial)
  - Optimize priority sorting (pre-sort if beneficial)
  - Optimize content selection (early termination)
  - Run benchmark after optimizations
  - Verify ≤ 200ms target met
  - Document optimization results
  - _Requirements: NFR1_

- [x] **9.5 Optimize JCS emission performance (NFR1)**





  - Profile receipt serialization
  - Profile status serialization
  - Profile doctor serialization
  - Identify performance bottlenecks
  - Optimize JSON value construction
  - Optimize canonicalization calls
  - Run benchmark after optimizations
  - Verify ≤ 50ms target met
  - Document optimization results
  - _Requirements: NFR1_

- [x] **9.6 Optimize overall dry-run performance (NFR1)**





  - Profile `spec --dry-run` baseline
  - Identify performance bottlenecks
  - Optimize config loading
  - Optimize artifact enumeration
  - Optimize receipt reading
  - Optimize status generation
  - Run benchmark after optimizations
  - Verify ≤ 5s target met
  - Document optimization results
  - _Requirements: NFR1_

- [x] **9.7 Add missing unit tests for edge cases**













  - Test canonicalization with empty content
  - Test canonicalization with special characters
  - Test canonicalization with unicode
  - Test redaction with overlapping patterns
  - Test redaction with patterns at boundaries
  - Test lock manager with invalid PID
  - Test lock manager with invalid host
  - Test lock manager with corrupted lock file
  - Test config with invalid TOML
  - Test config with missing sections
  - Test receipt with missing fields
  - Test status with no artifacts
  - Achieve >90% code coverage
  - _Requirements: All FR-*_

- [x] **9.8 Improve error messages and user guidance**





  - Review all error messages for clarity
  - Add actionable suggestions to all errors
  - Improve context information in errors
  - Test error messages with users
  - Update error messages based on feedback
  - Verify UserFriendlyError trait implemented for all errors
  - Verify suggestions() method provides actionable guidance
  - _Requirements: All FR-*_

- [x] **9.9 Code quality improvements**





  - Run clippy with all lints
  - Fix all clippy warnings
  - Run rustfmt on all files
  - Review and improve code comments
  - Review and improve function documentation
  - Review and improve module documentation
  - Remove dead code
  - Remove unused imports
  - Simplify complex functions
  - Extract common patterns into helpers
  - _Requirements: All FR-*_

---

## V10: Final Verification & Documentation

**Goal**: Final checks, documentation updates, and release preparation

- [x] **10.1 Update CHANGELOG with all changes**





  - Document all implemented features
  - Document all CLI flags added (--runner-mode, --runner-distro, --phase-timeout, etc.)
  - Document all receipt fields added (stderr_redacted, runner_distro, error_kind, error_reason, warnings, fallback_used, diff_context)
  - Document all status fields added (runner, runner_distro, fallback_used, canonicalization_version, canonicalization_backend, lock_drift, pending_fixups)
  - Document all exit codes (0, 1, 2, 7, 8, 9, 10, 70)
  - Mark any breaking changes with [BREAKING]
  - Add migration guide for breaking changes
  - Group changes by category (Features, Bug Fixes, Performance, Documentation)
  - _Requirements: All FR-*_

- [x] **10.2 Update README with complete documentation**





  - Verify all commands documented (spec, resume, status, clean, doctor, init, benchmark, test)
  - Verify all CLI flags documented
  - Verify all exit codes documented
  - Verify all configuration options documented
  - Verify all examples work and are up-to-date
  - Update troubleshooting section with common issues
  - Add performance benchmarks section
  - Add security section with redaction details
  - Add platform support section with WSL details
  - Verify links to other documentation files work
  - _Requirements: All FR-*_

- [x] **10.3 Update design document to match implementation**





  - Update module sections with actual implementations
  - Document actual module names (canonicalization.rs, redaction.rs)
  - Document additional modules (cache.rs, source.rs, integration_tests.rs)
  - Update data flows with actual implementation
  - Update testing strategy with actual tests
  - Update risks & mitigations with lessons learned
  - Mark all sections as IMPLEMENTED or IN PROGRESS
  - _Requirements: All FR-*_

- [x] **10.4 Verify and update schema files**





  - Confirm stderr_redacted (optional) in receipt.v1.json
  - Confirm runner_distro (optional) in receipt.v1.json
  - Confirm error_kind (optional) in receipt.v1.json
  - Confirm error_reason (optional) in receipt.v1.json
  - Confirm warnings (optional) in receipt.v1.json
  - Confirm fallback_used (optional) in receipt.v1.json
  - Confirm diff_context (optional) in receipt.v1.json
  - Confirm pending_fixups (optional) in status.v1.json
  - Confirm lock_drift (optional) in status.v1.json
  - Confirm additionalProperties: true in all schemas
  - Verify schema descriptions are accurate
  - Verify schema examples are up-to-date
  - _Requirements: FR-JCS-003_

- [x] **10.5 Create or update additional documentation**





  - Update docs/CONFIGURATION.md with all config options
  - Update docs/DOCTOR.md with all health checks
  - Update docs/CONTRACTS.md with schema versioning policy
  - Update docs/TRACEABILITY.md with requirements traceability
  - Create docs/PERFORMANCE.md with benchmark results
  - Create docs/SECURITY.md with redaction details
  - Create docs/PLATFORM.md with platform-specific notes
  - Verify all documentation is consistent
  - _Requirements: All FR-*_

- [x] **10.6 Run final verification suite**



  - Run all unit tests on Linux
  - Run all unit tests on macOS
  - Run all unit tests on Windows
  - Run all integration tests on Linux
  - Run all integration tests on macOS
  - Run all integration tests on Windows
  - Run all platform-specific tests
  - Run runtime-smoke CI job
  - Run docs-conformance CI job
  - Run performance benchmarks
  - Verify all tests pass
  - Verify CI green on all platforms
  - _Requirements: All FR-*_


  - [x] **10.6.1 Run final verification suite Windows**







  - Run all unit tests on Windows
  - Run all integration tests on Windows
  - Run all platform-specific tests
  - Run runtime-smoke CI job
  - Run docs-conformance CI job
  - Run performance benchmarks
  - Verify all tests pass
  - Verify CI green on all platforms
  - _Requirements: All FR-*_

  - [ ] **10.6.2 Run final verification suite Linux**


  - Run all unit tests on Linux
  - Run all integration tests on Linux
  - Run all platform-specific tests
  - Run runtime-smoke CI job
  - Run docs-conformance CI job
  - Run performance benchmarks
  - Verify all tests pass
  - Verify CI green on all platforms
  - _Requirements: All FR-*_

  - [ ] **10.6.3 Run final verification suite MacOS**


  - Run all unit tests on macOS
  - Run all integration tests on macOS
  - Run all platform-specific tests
  - Run runtime-smoke CI job
  - Run docs-conformance CI job
  - Run performance benchmarks
  - Verify all tests pass
  - Verify CI green on all platforms
  - _Requirements: All FR-*_


  - [x] **10.6.4 Run final verification suite WSL**






  - Run all unit tests on WSL
  - Run all integration tests on WSL
  - Run all platform-specific tests
  - Run runtime-smoke CI job
  - Run docs-conformance CI job
  - Run performance benchmarks
  - Verify all tests pass
  - Verify CI green on all platforms
  - _Requirements: All FR-*_


- [-] **10.7 Verify all requirements met**


  - Review each FR requirement (FR-RUN through FR-OBS)
  - Verify implementation matches requirement
  - Verify tests cover requirement
  - Verify documentation covers requirement
  - Mark each requirement as VERIFIED
  - Document any deviations or exceptions
  - _Requirements: All FR-*_

- [x] **10.8 Verify all NFRs met**









  - Verify NFR1 Performance: benchmarks meet targets
  - Verify NFR2 Security: no secrets leaked, redaction working
  - Verify NFR3 Portability: all platforms pass tests
  - Verify NFR4 Observability: logging provides required info
  - Verify NFR5 Atomicity: all writes atomic with retry
  - Verify NFR6 Determinism: JCS produces byte-identical output
  - Document NFR verification results
  - _Requirements: NFR1-NFR6_

- [x] **10.9 Prepare release**



  - Update version number in Cargo.toml
  - Update version number in README
  - Update version number in CHANGELOG
  - Create git tag for release
  - Build release binaries for all platforms
  - Test release binaries on all platforms
  - Prepare release notes
  - _Requirements: All FR-*_

- [x] **10.10 Mark spec complete**





  - All requirements verified ✅
  - All NFRs met ✅
  - All documentation updated ✅
  - All tests passing ✅
  - CI green on all platforms ✅
  - Performance targets met ✅
  - Security controls verified ✅
  - Ready for production use ✅
  - _Requirements: All FR-*_

---

## V11: Verification Checklist

Before marking the spec complete, verify:

- [x] All core modules implemented and functional
  - [x] Canonicalization (FR-JCS)
  - [x] SecretRedactor (FR-SEC)
  - [x] Runner (FR-RUN)
  - [x] Orchestrator (FR-ORC)
  - [x] PacketBuilder (FR-PKT)
  - [x] FixupEngine (FR-FIX)
  - [x] LockManager (FR-LOCK)
  - [x] StatusManager (FR-STA)
  - [x] Config system (FR-CFG)
  - [x] Benchmark (FR-BENCH)
  - [x] InsightCache (FR-CACHE)
  - [x] SourceResolver (FR-SOURCE)
  - [x] Phase trait system (FR-PHASE)
- [x] All FR requirements verified against implementation



  - [x] FR-RUN through FR-OBS (original requirements)
  - [x] FR-CACHE (InsightCache)
  - [x] FR-SOURCE (SourceResolver)
  - [x] FR-PHASE (Phase trait system)
  - [x] FR-LLM (LLM backend abstraction) - Verified as NOT IMPLEMENTED (V11 roadmap)
  - [x] FR-LLM-CLI (CLI provider support) - Verified as NOT IMPLEMENTED (V11 roadmap)
  - [x] FR-LLM-GEM (Gemini CLI) - Verified as NOT IMPLEMENTED (V12 roadmap)
  - [x] FR-LLM-API (HTTP provider support) - Verified as NOT IMPLEMENTED (V13 roadmap)
  - [x] FR-LLM-OR (OpenRouter) - Verified as NOT IMPLEMENTED (V13 roadmap)
  - [x] FR-LLM-ANTH (Anthropic API) - Verified as NOT IMPLEMENTED (V14 roadmap)
  - [x] FR-LLM-META (Provider metadata) - Verified as NOT IMPLEMENTED (V14 roadmap)
- [ ] Unit tests green on Linux/macOS/Windows
- [ ] Integration tests green on Linux/macOS/Windows
- [ ] Platform-specific tests green (WSL on Windows)
- [ ] runtime-smoke CI job passes
- [ ] docs-conformance CI job passes
- [ ] Receipts are JCS-canonical and schema-valid
- [ ] Status output is JCS-canonical and schema-valid
- [ ] Arrays sorted (artifacts by path)
- [ ] blake3 hashes populated for all artifacts
- [ ] No secrets in logs, receipts, or artifacts (except explicit --debug-packet)
- [ ] CHANGELOG updated with all features
  - [ ] InsightCache feature documented
  - [ ] SourceResolver feature documented
  - [ ] Phase trait system documented
  - [ ] Multi-provider LLM backend documented
  - [ ] Gemini CLI support documented
  - [ ] OpenRouter support documented
  - [ ] Anthropic API support documented
- [ ] README updated with all features
- [ ] Design document updated to match implementation
  - [ ] InsightCache module design added
  - [ ] SourceResolver module design added
  - [ ] Phase trait system design added
  - [ ] LLM backend abstraction design added
  - [ ] Gemini CLI backend design added
  - [ ] HTTP client design added
  - [ ] OpenRouter backend design added
  - [ ] Anthropic backend design added
  - [ ] LLM backend factory design added
- [ ] Schema files updated with optional fields
- [ ] Examples regenerated and committed
- [ ] All TODOs removed from staged modules
  - [ ] cache.rs TODOs removed
  - [ ] source.rs TODOs removed
  - [ ] phases.rs TODOs removed
  - [ ] llm/mod.rs TODOs removed
  - [ ] llm/gemini_cli.rs TODOs removed
  - [ ] llm/claude_cli.rs TODOs removed
  - [ ] llm/http_client.rs TODOs removed
  - [ ] llm/openrouter.rs TODOs removed
  - [ ] llm/anthropic.rs TODOs removed
  - [ ] llm/factory.rs TODOs removed
- [ ] Performance benchmarks meet NFR1 and NFR7 targets
  - [ ] NFR1: Packet assembly, JCS emission, dry-run
  - [ ] NFR7: Cache hit rate >70%, validation <10ms, speedup >50%

---

## V11: Multi-Provider LLM Backend Implementation

**Goal**: Implement multi-provider LLM backend support with Gemini CLI, OpenRouter, and Anthropic API

**Implementation Strategy**: Walking skeleton approach
- **Skeleton S1**: Single CLI backend (Claude) behind LlmBackend
- **Skeleton S2**: Swap in Gemini CLI as primary
- **Skeleton S3**: Add HTTP path (OpenRouter, Anthropic)

- [ ] **11.0 Introduce ExecutionStrategy layer (FR-EXEC) - Skeleton S1**
  - Add `ExecutionStrategy` enum: `Controlled`, `ExternalTool`
  - Add configuration surface:
    - Default: `Controlled` for all LLM providers
    - Optional experimental flag: `[llm.gemini] allow_tools = true`
  - Update orchestrator to select a `WriteStrategy` per phase from config
  - Implement `Controlled` path:
    - `phase.postprocess()` returns structured artifacts / fixup plan
    - `FixupEngine` applies changes, receipts/artifacts based on xchecker writes
  - Stub `ExternalTool` path:
    - Mark unsupported or experimental; don't enable by default
  - Test that Gemini CLI and Claude CLI run in `Controlled` mode by default
  - Document: "xchecker owns all file writes; LLM provides text/JSON only"
  - _Requirements: FR-LLM-GEM-006, FR-LLM-GEM-007_

- [ ] **11.1 Implement LLM backend abstraction (FR-LLM) - Skeleton S1**
  - Define `LlmBackend` trait in `src/llm/mod.rs`
  - Define `LlmInvocation` struct with spec_id, phase, prompt, timeout, model
  - Define `LlmResult` struct with raw_response, stderr_tail, timed_out, provider, model_used, tokens
  - Define `BackendKind` enum (ClaudeCli, GeminiCli, OpenRouterApi, AnthropicApi)
  - Define `BackendConfig` struct with kind, binary_path, base_url, api_key_env, model, max_tokens, temperature
  - Implement `LlmBackendFactory` with `create()` and `create_with_fallback()` methods
  - Test factory creation for each provider type
  - Test fallback logic when primary provider unavailable
  - Write unit tests for factory pattern
  - _Requirements: FR-LLM-001, FR-LLM-002, FR-LLM-003, FR-LLM-004, FR-LLM-005_

- [ ] **11.2 Refactor existing Runner to ClaudeCliBackend (FR-LLM-CLI) - Skeleton S1**
  - Create `src/llm/claude_cli.rs` module
  - Implement `ClaudeCliBackend` struct
  - Implement `LlmBackend` trait for `ClaudeCliBackend`
  - Reuse existing Runner infrastructure for process control
  - Maintain existing NDJSON parsing and timeout behavior
  - Test Claude CLI backend invocation
  - Test timeout enforcement
  - Test NDJSON merging
  - Write unit tests for Claude CLI backend
  - _Requirements: FR-LLM-CLI-001, FR-LLM-CLI-002, FR-LLM-CLI-003, FR-LLM-CLI-004, FR-LLM-CLI-005, FR-LLM-CLI-006, FR-LLM-CLI-007_

- [ ] **11.3 Implement Gemini CLI backend (FR-LLM-GEM, NFR8) - Skeleton S2**
  - Create `src/llm/gemini_cli.rs` module
  - Implement `GeminiCliBackend` struct with binary_path, default_model, phase_models, allow_tools
  - Implement `new()` method with config parsing
  - Implement `validate_binary()` method to check `gemini -h`
  - Implement `build_args()` method for non-interactive invocation:
    - **Exact command**: `gemini -p "<prompt>" --model <model>`
    - No REPL, no `/commands`, just one-shot text output
  - Implement `LlmBackend` trait for `GeminiCliBackend`
  - Implement per-phase model selection (check phase override, fallback to default)
  - **Authentication**: Assume `GEMINI_API_KEY` in environment (Gemini CLI reads it)
  - **Output contract**: Treat stdout as opaque text (no NDJSON requirement)
  - **Tools policy**: Text-only mode by default; no filesystem tools enabled
  - **Quota awareness (NFR8)**: 
    - Document: 1000 calls/day with 1M tokens per call free preview quota (extremely generous)
    - No hard budget enforcement needed (CLI tool, primary backend, quota is very generous)
    - Tests use minimal prompts to stay well under quota
  - Reuse existing Runner infrastructure for process control
  - Test Gemini CLI binary discoveryS
  - Test non-interactive invocation with `-p` and `--model`
  - Test per-phase model selection
  - Test timeout enforcement
  - Test stdout capture as raw_response
  - Test stderr redaction and 2 KiB cap
  - Write unit tests for Gemini CLI backend
  - Write integration tests for Gemini CLI invocation (skippable via `XCHECKER_SKIP_LLM_TESTS=1`, small prompts)
  - _Requirements: FR-LLM-GEM-001, FR-LLM-GEM-002, FR-LLM-GEM-003, FR-LLM-GEM-004, FR-LLM-GEM-006, NFR8_

- [ ] **11.4 Implement HTTP client module (FR-LLM-API) - Skeleton S3**
  - Create `src/llm/http_client.rs` module
  - Add `reqwest` dependency to Cargo.toml
  - Implement `HttpClient` struct with reqwest::Client
  - Implement `new()` method
  - Implement `post_json()` method with url, headers, body
  - Implement `HttpError` enum (Auth, Quota, ServerError, NetworkTimeout, InvalidResponse)
  - Implement error mapping: 4xx → Auth/Quota, 5xx → ServerError, timeout → NetworkTimeout
  - Test HTTP client creation
  - Test POST request with headers
  - Test error mapping for each error type
  - Test API key never logged
  - Write unit tests for HTTP client
  - _Requirements: FR-LLM-API-001, FR-LLM-API-002, FR-LLM-API-003, FR-LLM-API-004, FR-LLM-API-005, FR-LLM-API-006, FR-LLM-API-007_

- [ ] **11.5 Implement OpenRouter backend (FR-LLM-OR, NFR9) - Skeleton S3**
  - Create `src/llm/openrouter.rs` module
  - Implement `OpenRouterBackend` struct with client, base_url, api_key, model, max_tokens, temperature
  - Implement `new()` method with config parsing and API key loading from env
  - **Endpoint**: `https://openrouter.ai/api/v1/chat/completions` (default)
  - **Authentication**: `Authorization: Bearer $OPENROUTER_API_KEY`
  - **Required headers**:
    - `HTTP-Referer: https://effortlesssteven.com/xchecker`
    - `X-Title: xchecker`
  - Implement `build_request()` method for OpenAI-compatible format:
    - `model`, `messages` (system + user), `stream: false`
  - Implement `LlmBackend` trait for `OpenRouterBackend`
  - Parse response: extract `choices[0].message.content` into `raw_response`
  - Extract token counts from `usage` if available
  - **Call budget enforcement (NFR9)**:
    - Wrap OpenRouterBackend with `BudgetedBackend<OpenRouterBackend>`
    - Default per-process budget: 20 calls
    - Read `XCHECKER_OPENROUTER_BUDGET` env var to allow local override
    - On budget exhaustion, return `RunnerError::LlmBudgetExceeded` with clear error_reason
    - Track calls with atomic counter (thread-safe)
  - **Error mapping**:
    - 401/403 → `claude_failure` (exit 70)
    - 429 → `claude_failure` with "rate limited" in error_reason
    - 5xx → `claude_failure` with "provider outage" note
    - Network timeout → `phase_timeout` (exit 10)
    - Budget exceeded → `claude_failure` with "LLM budget exhausted" in error_reason
  - Test OpenRouter backend creation
  - Test request building with OpenAI-compatible format
  - Test header injection (HTTP-Referer, X-Title)
  - Test response parsing
  - Test API key loading from `OPENROUTER_API_KEY`
  - Test API key never logged
  - Test budget enforcement (default 20 calls)
  - Test budget override via `XCHECKER_OPENROUTER_BUDGET`
  - Test budget exhaustion error
  - Write unit tests for OpenRouter backend
  - Write integration tests for OpenRouter invocation (skippable via `XCHECKER_USE_OPENROUTER=1`, max_tokens <= 256)
  - _Requirements: FR-LLM-OR-001, FR-LLM-OR-002, FR-LLM-OR-003, FR-LLM-OR-004, FR-LLM-API-005, FR-LLM-API-006, NFR9_

- [ ] **11.6 Implement Anthropic API backend (FR-LLM-ANTH) - Skeleton S3**
  - Create `src/llm/anthropic.rs` module
  - Implement `AnthropicBackend` struct with client, base_url, api_key, model, max_tokens, temperature
  - Implement `new()` method with config parsing and API key loading from env
  - **Endpoint**: `https://api.anthropic.com/v1/messages` (default)
  - **Required headers**:
    - `x-api-key: $ANTHROPIC_API_KEY`
    - `anthropic-version: 2023-06-01`
    - `content-type: application/json`
  - Implement `build_request()` method for Anthropic API format:
    - `model`, `max_tokens`, `temperature`, `messages` (user role)
  - Implement `LlmBackend` trait for `AnthropicBackend`
  - Parse response: extract `content[0].text` into `raw_response`
  - Extract token counts from `usage` if available
  - **Error mapping**: Same as OpenRouter (4xx → auth/quota, 5xx → outage, timeout → phase_timeout)
  - Test Anthropic backend creation
  - Test request building with Messages API format
  - Test response parsing
  - Test API key loading from `ANTHROPIC_API_KEY`
  - Test API key never logged
  - Write unit tests for Anthropic backend
  - Write integration tests for Anthropic invocation (skippable via env flag, max_tokens <= 128)
  - _Requirements: FR-LLM-ANTH-001, FR-LLM-ANTH-002, FR-LLM-ANTH-003, FR-LLM-API-005, FR-LLM-API-006_

- [ ] **11.7 Add LLM metadata to receipts (FR-LLM-META) - Skeleton S1**
  - Update `Receipt` struct in `src/receipt.rs`
  - Add `llm_provider: Option<String>` field
  - Add `llm_model: Option<String>` field
  - Add `llm_timeout_seconds: Option<u64>` field
  - Add `llm_tokens_input: Option<u32>` field
  - Add `llm_tokens_output: Option<u32>` field
  - Update `ReceiptManager::write_success()` to populate LLM fields from `LlmResult`
  - Update `ReceiptManager::write_error()` to include LLM fields if available
  - Test receipt includes LLM metadata
  - Test receipt with missing token counts (null values)
  - Test receipt with fallback provider (warning included)
  - Write unit tests for receipt LLM metadata
  - _Requirements: FR-LLM-META-001, FR-LLM-META-002, FR-LLM-META-003, FR-LLM-META-004, FR-LLM-META-005_

- [ ] **11.8 Update configuration system for LLM providers (FR-LLM, FR-LLM-CLI, FR-LLM-GEM, FR-LLM-API, FR-LLM-OR, FR-LLM-ANTH) - Skeleton S1/S2**
  - Update `src/config.rs` to parse `[llm]` section
  - Add `llm_provider: String` field to `EffectiveConfig`
  - Add `llm_fallback_provider: Option<String>` field
  - Add `llm_config: BackendConfig` field
  - Parse `[llm.gemini]`, `[llm.claude]`, `[llm.openrouter]`, `[llm.anthropic]` sections
  - Support `--llm-provider` CLI flag override
  - Support `XCHECKER_LLM_PROVIDER` environment variable override
  - Test config parsing for each provider
  - Test CLI flag override
  - Test environment variable override
  - Test fallback provider configuration
  - Write unit tests for LLM config parsing
  - _Requirements: FR-LLM-CLI-002, FR-LLM-CLI-003, FR-LLM-GEM-003, FR-LLM-GEM-004, FR-LLM-API-002, FR-LLM-OR-001, FR-LLM-ANTH-001_

- [ ] **11.9 Update doctor command for LLM providers (FR-LLM-CLI, FR-LLM-GEM, FR-LLM-API) - Skeleton S2**
  - Update `src/doctor.rs` to check LLM provider availability
  - For CLI providers:
    - Check binary exists via `which` / `where`
    - Run `gemini -h` or `claude --version` to confirm functionality
    - **Do NOT call LLM** (no test prompts in doctor)
  - For HTTP providers:
    - Check API key environment variable is set
    - **Do NOT make HTTP requests** (no health checks in doctor by default)
  - Report provider name, binary path (CLI), model, authentication status
  - Provide actionable suggestions when provider unavailable
  - Test doctor with Gemini CLI configured
  - Test doctor with OpenRouter configured
  - Test doctor with missing binary
  - Test doctor with missing API key
  - **Test that doctor never calls LLMs** (AT-LLM-008)
  - Write unit tests for doctor LLM checks
  - _Requirements: FR-LLM-CLI-005, FR-LLM-GEM-002, FR-LLM-API-003, NFR8_

- [ ] **11.10 Wire LLM backend into orchestrator (FR-LLM) - Skeleton S1**
  - Update `src/orchestrator.rs` to use `LlmBackend` instead of direct Runner
  - Create LLM backend via factory in `execute_phase()`
  - Build `LlmInvocation` from packet and phase context
  - Invoke `backend.invoke()` instead of `runner.execute_claude()`
  - Handle `LlmResult` and extract raw_response
  - Pass raw_response to `phase.postprocess()`
  - Populate receipt with LLM metadata from `LlmResult`
  - Record fallback usage in warnings if applicable
  - Test orchestrator with Gemini CLI backend
  - Test orchestrator with OpenRouter backend
  - Test orchestrator with fallback provider
  - Test receipt includes LLM metadata
  - Write integration tests for orchestrator with LLM backends
  - _Requirements: FR-LLM-001, FR-LLM-002, FR-LLM-003, FR-LLM-004, FR-LLM-005_

- [ ] **11.11 Add LLM provider smoke tests (NFR8, NFR9) - Skeleton S3**
  - Create integration test for Gemini CLI end-to-end
  - Create integration test for OpenRouter end-to-end
  - Create integration test for Anthropic end-to-end
  - **Test gating**:
    - Gemini CLI: skippable via `XCHECKER_SKIP_LLM_TESTS=1`
    - OpenRouter: requires `XCHECKER_USE_OPENROUTER=1` (explicit opt-in)
    - Anthropic: requires `XCHECKER_USE_ANTHROPIC=1` (explicit opt-in)
  - **Use minimal prompts**: Single short message ("ping" or "hello")
  - **Use low max_tokens**: <= 256 for OpenRouter/Anthropic tests
  - **OpenRouter call budget (NFR9)**:
    - Limit OpenRouter calls in test suite to ≤ 10 per full run
    - Assert `used_calls <= configured_budget` at end of test run
    - Use default 20-call budget in tests
  - **Recommended models**:
    - Gemini CLI: `gemini-2.0-flash-lite` (free preview quota)
    - OpenRouter: `google/gemini-2.0-flash-lite` (1000 calls/day free tier)
    - Anthropic: `claude-3-5-sonnet-20241022` (paid, keep max_tokens very low)
  - **Test coverage strategy**:
    - Most tests use mocks/stubs
    - Real OpenRouter only hit in 1-2 "happy path" flows (Requirements/Design/Tasks)
    - 1 fixup application round-trip
    - 1-2 error mapping tests (e.g., budget exhaustion, auth failure)
  - Test provider selection
  - Test fallback provider
  - Test receipt metadata
  - Test budget enforcement for OpenRouter
  - Add smoke tests to CI with skip flags enabled by default
  - Document how to run LLM tests locally
  - Document quota expectations and cost control (1000/day OpenRouter, 20/run default)
  - _Requirements: NFR8, NFR9_

- [ ] **11.12 Update documentation for LLM providers - Post-1.0**
  - Update README with LLM provider configuration examples
  - Document Gemini CLI setup and authentication
  - Document OpenRouter setup and API key
  - Document Anthropic API setup and API key
  - Document provider selection and fallback
  - Document per-phase model configuration for Gemini
  - Document cost control and test skipping
  - Update CONFIGURATION.md with `[llm]` section details
  - Update DOCTOR.md with LLM provider checks
  - Create docs/LLM_PROVIDERS.md with detailed provider guide
  - _Requirements: All FR-LLM-*_

- [ ] **11.13 Update schema files for LLM metadata (FR-LLM-META) - Skeleton S1**
  - Update `schemas/receipt.v1.json` to include optional LLM fields
  - Add `llm_provider` (optional string)
  - Add `llm_model` (optional string)
  - Add `llm_timeout_seconds` (optional number)
  - Add `llm_tokens_input` (optional number)
  - Add `llm_tokens_output` (optional number)
  - **Ensure backward compatibility**:
    - All new fields MUST be optional
    - Keep `additionalProperties: true`
    - Add doc comment: "Only add fields as optional; do not remove or change types of existing fields in v1. Breaking changes require v2 schemas."
  - Regenerate schema examples
  - Verify examples validate against updated schema
  - Add schema validation to CI
  - _Requirements: FR-LLM-META-001, FR-LLM-META-002, FR-LLM-META-003, FR-LLM-META-004_

- [ ] **11.14 Implement XCHECKER_SKIP_LLM_TESTS gating helper (NFR8) - Skeleton S1**
  - Create helper function `llm_tests_enabled()` in test utilities
  - Check environment variables:
    - `XCHECKER_SKIP_LLM_TESTS=1` → skip all LLM integration tests
    - `XCHECKER_REAL_LLM_TESTS=1` → enable heavy LLM test suite (local only)
  - Default behavior: skip LLM tests in CI
  - Use helper in all LLM integration tests (Gemini, OpenRouter, Anthropic)
  - Add test: `test_doctor_does_not_call_llms` (AT-LLM-008)
    - Run `xchecker doctor --json` with LLM provider configured but no keys/binaries
    - Assert no network calls or CLI invocations attempted
    - Errors are purely static checks
  - Document environment variables in README and test documentation
  - _Requirements: NFR8_

---

## Notes

- **Focus**: This is a verification and improvement spec, not a greenfield implementation
- **Core work complete**: All D1-D6 delivery phases from original plan are implemented
- **Current phase**: Verification, edge case testing, optimization, and polish
- **Additional features implemented beyond original plan**:
  - **InsightCache** (FR-CACHE): BLAKE3-keyed cache with TTL validation and phase-specific insights
  - **SourceResolver** (FR-SOURCE): Multi-source support for GitHub, filesystem, and stdin
  - **Phase Trait System** (FR-PHASE): Trait-based phase implementation with separated concerns
  - **Integration test framework**: End-to-end testing infrastructure
  - **Example generators**: Schema example generation for documentation
- **Module naming**: canonicalization.rs (not jcs.rs), redaction.rs (not secret.rs)
- **Integration**: Some modules have TODOs for wiring into orchestrator (M2/M3 work)
  - PacketBuilder needs integration with orchestrator (V9.2)
  - InsightCache needs integration with PacketBuilder (V9.3)
  - Phase trait system needs integration with orchestrator (V9.2)
  - SourceResolver needs integration with CLI commands (V9.2)


---

# V11–V18: Multi-Provider LLM & Ecosystem Implementation

## V11: LLM Core Skeleton & Claude Backend (MVP+)

**Goal**: Put the existing Runner behind a clean LlmBackend abstraction, keep Controlled writes, and wire basic LLM metadata into receipts.

- [ ] **11.0 Introduce ExecutionStrategy layer (FR-EXEC)**
  - Add `ExecutionStrategy` enum: `Controlled`, `ExternalTool`
  - Add configuration surface: `[llm] execution_strategy = "controlled"` (default, only valid value)
  - Update orchestrator to thread ExecutionStrategy into phase execution
  - Implement Controlled path: LLM proposes text/JSON; all writes go through FixupEngine + atomic pipeline
  - Implement ExternalTool path: return `XCheckerError::Unsupported("ExternalTool not yet supported")`
  - Add execution_strategy field to receipts for audit trail
  - _Requirements: FR-EXEC_

- [ ] **11.1 LlmBackend trait + factory (FR-LLM)**
  - Create `src/llm/mod.rs` with trait definition
  - Define `LlmInvocation` struct with spec_id, phase_id, prompt, timeout, model
  - Define `LlmResult` struct with provider, model_used, raw_response, stderr_tail, timed_out, tokens_input, tokens_output
  - Define `LlmBackend` trait with `invoke()` method
  - Define `BackendKind` enum: `ClaudeCli`, `GeminiCli`, `OpenRouterApi`, `AnthropicApi`
  - Implement `LlmBackendFactory::create()` to read `[llm] provider` and return appropriate backend
  - Support only `ClaudeCli` in V11; return error for other providers
  - _Requirements: FR-LLM_

- [ ] **11.2 ClaudeCliBackend wrapping current Runner (FR-LLM-CLI)**
  - Create `src/llm/claude_cli.rs`
  - Implement `ClaudeCliBackend` struct holding Runner configuration
  - Implement `LlmBackend` for `ClaudeCliBackend`
  - Reuse existing Runner timeout handling, Job Objects, killpg, NDJSON parsing, ring buffers
  - Map timeout → `RunnerError::PhaseTimeout`
  - Map no-valid-JSON → `RunnerError::NoValidJson` with redacted tail excerpt
  - Write unit tests for happy path using claude-stub
  - _Requirements: FR-LLM-CLI_

- [ ] **11.3 Orchestrator uses LlmBackend (FR-LLM)**
  - Update `PhaseOrchestrator` to hold `Box<dyn LlmBackend>`
  - In `execute_phase()`, build `LlmInvocation` from `PhaseContext` + packet
  - Call `backend.invoke(invocation).await?`
  - Pass `result.raw_response` into `phase.postprocess()`
  - Fallback to direct Runner only in tests or behind feature flag
  - _Requirements: FR-LLM_

- [ ] **11.4 LLM metadata → existing LlmInfo in Receipt (FR-LLM-META)**
  - Implement `From<LlmResult> for LlmInfo`
  - Populate `Receipt.llm` with provider, model_used, tokens_input, tokens_output, timed_out
  - Ensure receipts always include llm metadata when backend is invoked
  - On error after LLM invocation, attach whatever metadata available
  - Regenerate schema examples
  - _Requirements: FR-LLM-META_

- [ ] **11.5 Minimal LLM config / CLI overrides (FR-LLM-CLI)**
  - Add `[llm]` section parsing: `provider = "claude-cli"`, `execution_strategy = "controlled"`
  - Add CLI flag: `--llm-provider` (accepts only "claude-cli" in V11)
  - Add env var: `XCHECKER_LLM_PROVIDER` (same restriction)
  - Validate provider value; error if unsupported
  - _Requirements: FR-LLM-CLI_

- [ ] **11.6 Test gating helper + "doctor does not call models" (NFR8)**
  - Implement `llm_tests_enabled()` helper checking `XCHECKER_SKIP_LLM_TESTS`
  - Use in every test that would touch real CLI/model
  - Add `test_doctor_does_not_call_llms`: configure provider, no binary on PATH, run doctor, assert config/availability check (not LLM call)
  - _Requirements: NFR8_

- [ ]* **11.7 Write property tests for LLM backend abstraction**
  - **Property 1**: For any valid LlmInvocation, backend.invoke() returns LlmResult with provider set
  - **Property 2**: For any timeout duration, backend respects timeout and returns timed_out: true
  - **Validates: Requirements FR-LLM, FR-LLM-CLI_

- [ ]* **11.8 Write integration tests for Claude CLI backend**
  - Test happy path: valid prompt → valid response
  - Test timeout: prompt takes too long → timeout error
  - Test invalid JSON: no valid JSON in stdout → claude_failure
  - Test stderr capture: stderr redacted and truncated
  - _Requirements: FR-LLM-CLI_

## V12: Gemini CLI as First-Class Provider

**Goal**: Add Gemini CLI as the default CLI backend, with Claude as optional/fallback.

- [ ] **12.0 GeminiCliBackend (FR-LLM-GEM)**
  - Create `src/llm/gemini_cli.rs`
  - Implement `GeminiCliBackend` struct with binary_path, default_model, phase_models, allow_tools_prompting
  - Implement `LlmBackend` for `GeminiCliBackend`
  - Invoke `gemini -p "<prompt>" --model <model>` via Runner
  - Treat stdout as opaque text (not NDJSON)
  - Capture stderr with redaction and 2 KiB cap
  - _Requirements: FR-LLM-GEM_

- [ ] **12.1 Config + provider selection (FR-LLM-GEM, FR-LLM-CLI)**
  - Update config parsing: `[llm] provider = "gemini-cli"`, `fallback_provider = "claude-cli"`
  - Parse `[llm.gemini]` and `[llm.claude]` sections fully
  - Update `LlmBackendFactory::create()` to support Gemini as primary
  - Implement fallback logic: use Claude only if Gemini unavailable
  - _Requirements: FR-LLM-GEM, FR-LLM-CLI_

- [ ] **12.2 Doctor checks (FR-LLM-GEM, FR-LLM-CLI)**
  - For gemini-cli: `which gemini`, `gemini -h` to confirm availability
  - For claude-cli (if configured/fallback): `which claude`, `claude --version`
  - Report both providers' availability
  - No completions inside doctor (only binary presence/version)
  - _Requirements: FR-LLM-GEM, FR-LLM-CLI_

- [ ] **12.3 Smoke tests + docs (NFR8)**
  - Add `test_gemini_cli_smoke` gated by `XCHECKER_SKIP_LLM_TESTS`
  - Update README: "How to use Gemini CLI as your LLM"
  - Note that xchecker keeps writes Controlled
  - _Requirements: NFR8_

- [ ]* **12.4 Write property tests for Gemini CLI backend**
  - **Property 1**: For any valid prompt, Gemini backend returns response with provider = "gemini-cli"
  - **Property 2**: For any model override, backend uses per-phase model if present, else default
  - **Validates: Requirements FR-LLM-GEM_

## V13: HTTP Client & OpenRouter Backend (Optional)

**Goal**: Add a single HTTP path (OpenRouter) that can be enabled when you want it, with clear budgets.

- [ ] **13.0 HttpClient (FR-LLM-API)**
  - Create `src/llm/http_client.rs`
  - Implement `HttpClient` struct with reqwest::Client
  - Implement error mapping: 4xx → auth/quota, 5xx → provider outage, timeout → phase_timeout
  - Map to existing error taxonomy (exit codes, error_kind)
  - _Requirements: FR-LLM-API_

- [ ] **13.1 OpenRouterBackend (FR-LLM-OR, NFR9)**
  - Create `src/llm/openrouter.rs`
  - Implement `OpenRouterBackend` struct with base_url, api_key, model, max_tokens, temperature, call_budget
  - Implement `LlmBackend` for `OpenRouterBackend`
  - Default base_url: `https://openrouter.ai/api/v1/chat/completions`
  - Default model: `google/gemini-2.0-flash-lite` (for cost sanity)
  - Default call_budget: 20, overridable via `XCHECKER_OPENROUTER_BUDGET`
  - Track calls_made; exit code 70 if budget exceeded
  - _Requirements: FR-LLM-OR, NFR9_

- [ ] **13.2 Config & doctor checks (FR-LLM-OR, FR-LLM-API)**
  - Parse `[llm.openrouter]` section: url, model, api_key_env, max_tokens, temperature
  - Doctor: confirm `OPENROUTER_API_KEY` env var present
  - Doctor: do NOT send HTTP request (opt-in only)
  - _Requirements: FR-LLM-OR, FR-LLM-API_

- [ ] **13.3 Smoke tests & gating (NFR9)**
  - Add OpenRouter integration tests gated by `XCHECKER_USE_OPENROUTER=1`
  - Use very small prompts, tiny max_tokens
  - Validate call budgets (≤ 10 per test run)
  - _Requirements: NFR9_

- [ ]* **13.4 Write property tests for OpenRouter backend**
  - **Property 1**: For any valid prompt, OpenRouter backend returns response with provider = "openrouter"
  - **Property 2**: For any call budget, backend tracks calls and exits with code 70 if exceeded
  - **Validates: Requirements FR-LLM-OR_

## V14: Anthropic HTTP, Rich Metadata & Provider Docs

**Goal**: Add Anthropic HTTP backend and finish the metadata + docs story.

- [ ] **14.0 AnthropicBackend (FR-LLM-ANTH)**
  - Create `src/llm/anthropic.rs`
  - Implement `AnthropicBackend` struct with base_url, api_key, model, max_tokens, temperature
  - Implement `LlmBackend` for `AnthropicBackend`
  - Use Anthropic's Messages API
  - Default base_url: `https://api.anthropic.com/v1/messages`
  - _Requirements: FR-LLM-ANTH_

- [ ] **14.1 Rich metadata & schema (FR-LLM-META)**
  - Extend `LlmResult` with tokens_input, tokens_output from usage
  - Map into `LlmInfo` in receipts
  - Ensure `receipt.v1.json` schema includes optional token counts
  - _Requirements: FR-LLM-META_

- [ ] **14.2 Docs & examples (FR-LLM-DOCS)**
  - Create `docs/LLM_PROVIDERS.md` with:
    - Gemini CLI: setup, config, cost, authentication
    - Claude CLI: setup, config, cost, authentication
    - OpenRouter: setup, config, cost, budget control, supported models
    - Anthropic: setup, config, cost, authentication
    - Comparison table (cost, speed, quality, local vs cloud)
    - Test gating: `XCHECKER_SKIP_LLM_TESTS`, `XCHECKER_USE_OPENROUTER`
  - Update README with LLM provider section
  - Update CONFIGURATION.md with all LLM options
  - Update DOCTOR.md with LLM checks
  - _Requirements: FR-LLM-DOCS_

- [ ]* **14.3 Write property tests for Anthropic backend**
  - **Property 1**: For any valid prompt, Anthropic backend returns response with provider = "anthropic"
  - **Property 2**: For any response, tokens_input and tokens_output are populated
  - **Validates: Requirements FR-LLM-ANTH_

## V15: Claude Code (Claude Code) Integration & UX

**Goal**: Make it trivial to trigger xchecker phases from Claude Code.

- [ ] **15.0 CLI / protocol hooks (FR-Claude Code-CLI)**
  - Add `xchecker spec <spec-id> --json` command
  - Add `xchecker status <spec-id> --json` command
  - Add `xchecker resume <spec-id> --phase <phase> --json` command
  - Emit stable JSON shapes suitable for Claude Code parsing
  - _Requirements: FR-Claude Code-CLI_

- [ ] **15.1 Claude Code flows (FR-Claude Code-FLOWS)**
  - Provide example Claude Code project (or flow definitions)
  - Show how to call `xchecker spec` for Requirements
  - Show how to call `xchecker resume --phase design`, `--phase tasks`, etc.
  - Show how to use receipts/status JSON instead of raw repo
  - _Requirements: FR-Claude Code-FLOWS_

- [ ] **15.2 Slash-command UX (FR-Claude Code-CLI)**
  - Document canonical `/xchecker` command:
    - `/xchecker spec <spec-id> [source...]`
    - `/xchecker status <spec-id>`
    - `/xchecker resume <spec-id> --phase design`
  - Show how to wire to Claude Code's tool or webhooks
  - _Requirements: FR-Claude Code-CLI_

- [ ]* **15.3 Write integration tests for Claude Code flows**
  - Test `xchecker spec --json` output format
  - Test `xchecker status --json` output format
  - Test `xchecker resume --phase <phase> --json` output format
  - Verify JSON is parseable by Claude Code
  - _Requirements: FR-Claude Code-CLI_

## V16: Workspace & Multi-Spec Orchestration

**Goal**: Move from "one spec at a time" to a workspace view.

- [ ] **16.0 Workspace model & registry (FR-WORKSPACE)**
  - Add light "workspace" abstraction (purely file-based)
  - Default workspace = current repo root + `.xchecker/`
  - Add workspace registry file: `workspace.yaml` or `projects.yaml`
  - Implement `xchecker project init <name>` command
  - Implement `xchecker project add-spec <spec-id> --tag <tag>` command
  - Implement `xchecker project list` command
  - Registry fields: spec_id, status, tags, llm_provider
  - _Requirements: FR-WORKSPACE_

- [ ] **16.1 Project-level status & health surfaces (FR-WORKSPACE)**
  - Implement `xchecker project status [--json]` command
  - Aggregate per-spec StatusOutput into single JSON
  - Human mode: KISS table with SPEC, PHASE, COMPLETE, PENDING_FIXUPS
  - Use Level-1 metrics from receipts: exit_code != 0 count, last run time, stale detection
  - _Requirements: FR-WORKSPACE_

- [ ] **16.2 Timeline / history & trend metrics (FR-WORKSPACE)**
  - Implement `xchecker project history <spec-id> [--json]` command
  - Emit compact timeline: phase progression, duration_ms, exit_codes, LLM provider switches
  - Optional project metrics: P95 phase times, fixup rounds, error kind distribution
  - _Requirements: FR-WORKSPACE_

- [ ] **16.3 TUI (optional but nice) (FR-WORKSPACE)**
  - Optional crate feature `tui` with text UI
  - Panels: specs list (left), latest receipt summary (top right), pending fixups/warnings (bottom right)
  - Implement `xchecker project tui` command
  - _Requirements: FR-WORKSPACE_

- [ ]* **16.4 Write integration tests for workspace commands**
  - Test `xchecker project init` creates registry
  - Test `xchecker project add-spec` registers spec
  - Test `xchecker project list` lists all specs
  - Test `xchecker project status --json` aggregates status
  - Test `xchecker project history --json` emits timeline
  - _Requirements: FR-WORKSPACE_

## V17: Policy & Enforcement ("Double-Entry SDLC" in CI)

**Goal**: Turn xchecker's receipts + status into enforceable gates.

- [ ] **17.0 xchecker gate command (FR-GATE)**
  - Implement `xchecker gate <spec-id> [--policy <path>] [--json]` command
  - Read latest StatusOutput + recent receipts
  - Evaluate simple policy:
    - No phases in error state
    - No pending fixups above threshold
    - Latest phase ≥ design or ≥ tasks (configurable)
    - Lockfile drift resolved (or allowed by policy)
  - Return exit 0 if all gates pass
  - Return exit non-zero + structured error for CI if gates fail
  - Policy options: `--min-phase tasks`, `--fail-on-pending-fixups`, `--max-phase-age 7d`
  - _Requirements: FR-GATE_

- [ ] **17.1 GitHub / GitLab CI templates (FR-GATE-CI)**
  - Provide `.github/workflows/xchecker-gate.yml` template
  - Provide GitLab CI equivalent
  - Document: "How to make xchecker gate a required status check"
  - Document: "How to gate merges on spec completeness and fixups"
  - _Requirements: FR-GATE-CI_

- [ ] **17.2 Policy evolution / advanced integration (FR-GATE)**
  - Policy as code (future-friendly): simple DSL or JSON
  - Org/scoped policies: global defaults in `~/.config/xchecker/policy.toml`, repo-local overrides in `.xchecker/policy.toml`
  - _Requirements: FR-GATE_

- [ ]* **17.3 Write integration tests for gate command**
  - Test gate passes when all conditions met
  - Test gate fails when phase in error state
  - Test gate fails when pending fixups exceed threshold
  - Test gate fails when phase too old
  - Test gate with custom policy file
  - _Requirements: FR-GATE_

## V18: Ecosystem & Templates (Batteries Included)

**Goal**: Turn xchecker into something a team can adopt in an afternoon.

- [ ] **18.0 Spec templates & "quickstart" flows (FR-TEMPLATES)**
  - Implement `xchecker template list` command
  - Implement `xchecker template init <template> <spec-id>` command
  - Available templates: fullstack-nextjs, rust-microservice, python-fastapi, docs-refactor
  - Each template seeds: starting problem statement, minimal `.xchecker/config.toml`, example spec flow snippet
  - Templates live as bundled files under `templates/` or small registry format
  - _Requirements: FR-TEMPLATES_

- [ ] **18.1 Plugin hooks (internal extension points) (FR-HOOKS)**
  - Define extension points: pre-phase hook, post-phase hook
  - Implement as `hooks.toml` that can shell out to scripts
  - Example: `[hooks.pre_phase] command = "scripts/xchecker-pre-phase.sh"`
  - Use cases: Slack/Teams notifications, dashboard sync, Prometheus metrics
  - _Requirements: FR-HOOKS_

- [ ] **18.2 "Showcase" examples & narrative (FR-SHOWCASE)**
  - Add `examples/fullstack-nextjs/`: minimal app skeleton, scripted workflow, README with xchecker outputs
  - Add `examples/mono-repo/`: multiple spec IDs, workspace commands in action
  - Add markdown walkthroughs: "Running xchecker on your repo in 20 minutes", "From spec to PR: xchecker + Claude Code flow"
  - _Requirements: FR-SHOWCASE_

- [ ]* **18.3 Write integration tests for templates and hooks**
  - Test `xchecker template list` returns available templates
  - Test `xchecker template init` seeds correct files
  - Test hooks are executed at correct phases
  - Test hook failures don't block execution
  - _Requirements: FR-TEMPLATES, FR-HOOKS_

---

## Release Checklist (V11–V18)

- [ ] All V11–V18 requirements implemented
- [ ] All V11–V18 tests passing
- [ ] All V11–V18 documentation updated
- [ ] Performance benchmarks still meet NFR1 targets
- [ ] Security controls verified (no API keys logged, redaction working)
- [ ] Cross-platform testing complete (Linux, macOS, Windows, WSL)
- [ ] CI green on all platforms
- [ ] CHANGELOG updated with all V11–V18 features
- [ ] README updated with multi-provider and ecosystem features
- [ ] Design document updated to reflect V11–V18 implementation
- [ ] Ready for production release

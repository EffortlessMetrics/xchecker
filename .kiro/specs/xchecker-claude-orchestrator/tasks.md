# Implementation Plan

## Milestone 0: Walking Skeleton (End-to-End Requirements Phase)

- [x] 1. Set up project structure and minimal types










  - Create Cargo.toml with dependencies: clap, serde, tokio, anyhow, blake3 (with rayon feature), tempfile, camino, serde_yaml, toml, globset, ignore, fd-lock, serde_json, serde_json_canonicalizer, regex, chrono, itertools, once_cell
  - Define core enums: PhaseId, Priority, FileType, PermissionMode, OutputFormat, RunnerMode
  - Create basic error hierarchy with enhanced XCheckerError variants including RunnerError, PacketOverflow, ModelResolutionError
  - Add build.rs to embed git rev into xchecker_version: "{CARGO_PKG_VERSION}+{GIT_SHA}"
  - _Requirements: R10.1, R10.2, R12.6_

- [x] 2. Create minimal artifact management





  - [x] 2.1 Implement basic ArtifactManager with atomic writes and Windows retry logic

    - Use tempfile → rename pattern for all file operations
    - Add Windows-only persist/rename retry with bounded exponential backoff (≤ 250 ms total) for AV/indexer locks
    - Create directory structure: artifacts/, receipts/, context/
    - Normalize line endings to \n on all writes and hashes
    - _Requirements: R4.3, R4.5, R8.3, R8.4, NFR2_

- [x] 3. Build minimal receipt system




  - [x] 3.1 Create basic Receipt struct for single-phase validation


    - Include essential fields: spec_id, phase, timestamp, exit_code
    - Add JSON serialization with minimal schema
    - Hash canonicalized outputs and record canonicalization_version
    - _Requirements: R2.1, R2.2, R2.6, R2.7_

- [x] 4. Implement thin Phase trait system






  - [x] 4.1 Create Phase trait with separated concerns

    - Define PhaseId enum and basic dependency system
    - Implement prompt(), make_packet(), postprocess() separation (no execute())
    - Add NextStep enum with rewind capabilities
    - _Requirements: R10.1, R10.3, R4.6_

  - [x] 4.2 Create Requirements phase implementation only


    - Implement basic prompt generation for requirements phase
    - Create minimal packet construction (no budgets yet)
    - Add postprocessing to generate requirements.md artifact
    - _Requirements: R1.1_

- [x] 5. Create Claude CLI stub for testing




  - [x] 5.1 Build stub Claude CLI for development


    - Create test harness that emits valid stream-json responses
    - Support requirements phase prompts with realistic outputs
    - Include various response scenarios: success, partial, malformed
    - _Requirements: R4.1, R4.4_

- [x] 6. Build minimal CLI and orchestrator










  - [x] 6.1 Implement basic CLI with spec command


    - Create spec subcommand with minimal flags
    - Add basic source resolution (stdin only initially)
    - Implement dry-run mode showing planned execution
    - _Requirements: R1.1, R1.4, R6.3_

  - [x] 6.2 Create basic orchestrator for Requirements phase







    - Wire together Phase trait, ArtifactManager, Receipt system
    - Execute single Requirements phase end-to-end
    - Generate artifacts and receipts for validation
    - _Requirements: R1.1, R1.2_

- [x] 7. **M0 Gate**: Validate walking skeleton





  - Run `xchecker spec 42` using stub Claude to produce 00-requirements.md + receipt
  - Verify `xchecker status 42` shows last completed phase
  - Confirm atomic file operations and basic error handling work
  - _Requirements: R1.1, R1.2, R2.1_

## Milestone 1: Full Claude Integration and Multi-File Receipts

- [x] 8. Implement Runner abstraction for Windows/WSL support




  - [x] 8.1 Create Runner enum and detection logic with proper WSL detection


    - Implement RunnerMode::Auto detection: try claude --version on PATH → Native, else try wsl -e claude --version → WSL
    - Add WslOptions for distro and claude_path configuration
    - Use wsl.exe --exec with argv (no shell) for WSL execution; record runner_distro from wsl -l -q or $WSL_DISTRO_NAME
    - Provide friendly preflight error suggesting wsl --install if needed
    - _Requirements: R12.1, R12.2, R12.3, R12.5, R12.6_

- [x] 9. Implement real Claude CLI wrapper











  - [x] 9.1 Create ClaudeWrapper with controlled surface and Runner integration



    - Implement model alias resolution to full name once per run
    - Capture Claude CLI version from `claude --version`
    - Record both model_alias and model_full_name in receipts
    - Integrate with Runner for cross-platform execution
    - _Requirements: R7.1, R12.2_

  - [x] 9.2 Add structured output handling with fallback




    - Always try --output-format stream-json --include-partial-messages first
    - Implement fallback to --output-format text on parse failure once per phase
    - Record fallback_used=true in receipt when fallback occurs
    - Capture stderr_tail (2 KiB max) and partial stdout on failures
    - _Requirements: R4.1, R4.3, R4.4_

- [x] 10. Enhance receipt system for multi-file support









  - [x] 10.1 Implement full Receipt schema with PacketEvidence and Runner info



    - Add PacketEvidence with FileEvidence arrays including pre-redaction hashes
    - Include xchecker_version (from build.rs), claude_cli_version, canonicalization_version
    - Add runner, runner_distro, fallback_used, canonicalization_backend fields
    - Support multiple output files with individual canonicalized hashes
    - _Requirements: R2.1, R2.2, R2.6, R2.7, R12.5_

- [-] 11. Add basic error handling and logging











  - [x] 11.1 Implement structured error reporting










    - Create user-friendly error messages for common failure modes
    - Add context and suggestions for configuration issues
    - Implement proper error propagation through phase system
    - _Requirements: R1.3, R4.3, R6.4_

- [x] 12. **M1 Gate**: Validate real Claude integration and Runner system










  - Run complete Requirements phase with real Claude CLI on both Native and WSL (Windows only)
  - Verify receipt contains all required metadata including runner info and version information
  - Test fallback behavior from stream-json to text format
  - Test RunnerMode::Auto detection on Windows
  - _Requirements: R4.1, R4.4, R2.1, R12.1, R12.2_

## Milestone 2: Packet System and Security

- [x] 13. Build packet construction with concrete budgets and evidence





  - [x] 13.1 Implement ContentSelector with priority-based selection


    - Set concrete defaults: packet_max_bytes=65536, packet_max_lines=1200 (config-overridable)
    - Define priority order: *.core.yaml (non-evictable) → SPEC/ADR/REPORT → README/SCHEMA → misc; LIFO within class
    - Always write context/<phase>-packet.txt (success or fail)
    - _Requirements: R3.1, R3.2_

  - [x] 13.2 Create PacketBuilder with evidence tracking


    - Store packet_blake3 after redaction; store per-file blake3_pre_redaction
    - Implement packet preview generation for context/<phase>-packet.txt
    - On overflow, fail pre-Claude, write packet preview, produce receipt with exit_code!=0
    - _Requirements: R3.1, R3.3, R3.4_

- [x] 14. Implement secret redaction system





  - [x] 14.1 Create SecretRedactor with configurable patterns


    - Implement default patterns: ghp_[A-Za-z0-9]{36}, AKIA[0-9A-Z]{16}, AWS_SECRET_ACCESS_KEY[=:], xox[baprs]-[A-Za-z0-9-]+, Bearer [A-Za-z0-9._-]{20,}
    - Add support for --ignore-secret-pattern and --extra-secret-pattern flags
    - Log pattern IDs and location (filename+range), never matched text
    - _Requirements: R9.1, R9.2_

  - [x] 14.2 Integrate redaction into packet pipeline


    - Scan files before packet construction
    - Apply redaction to packet content before hashing
    - Store {path, range?, blake3_pre_redaction} for each included piece
    - _Requirements: R9.1, R9.2_

- [x] 15. **M2 Gate**: Validate packet system and security





  - Test oversized packet fails pre-Claude with preview written and receipt showing overflow
  - Verify redaction changes packet hash while evidence preserves source integrity
  - Confirm secret detection aborts run unless explicitly ignored
  - _Requirements: R3.3, R9.1, R9.2_

## Milestone 3: Canonicalization and Full Phase System
- [x] 16. Implement canonicalization system with explicit v1 rules





  - [x] 16.1 Create Canonicalizer struct with explicit v1 algorithms


    - Implement YAML canonicalization (v1) using JCS: YAML → JSON (BTreeMap) → RFC 8785 JCS → BLAKE3 hash
    - Keep human-readable YAML on disk (normalized: LF, trim trailing spaces, final newline)
    - Implement Markdown normalization (v1): normalize \n, trim trailing spaces, collapse trailing blank lines to 1, fence normalization, final newline
    - Add hash_canonicalized method with FileType dispatch
    - Record canonicalization_version: "yaml-v1,md-v1" and canonicalization_backend: "jcs-rfc8785" in receipts
    - _Requirements: R2.4, R2.5, R13.1, R13.2_

  - [x] 15.2 Add canonicalization testing utilities




    - Create test fixtures with intentionally reordered YAML/Markdown
    - Test: same content different formatting ⇒ identical hash
    - Verify structure determinism independent of text formatting
    - Test error handling for malformed inputs
    - _Requirements: R12.1, R12.3_

- [x] 16. Complete phase system implementation






  - [x] 16.1 Implement Design and Tasks phases



    - Add Design phase with architecture-focused prompts
    - Add Tasks phase with implementation planning prompts
    - Integrate with packet system and canonicalization
    - _Requirements: R1.1_

  - [x] 16.2 Handle partial artifacts and resume


    - Implement partial artifact storage on phase failures
    - Add resume capability from any completed phase
    - Delete partials on success and promote to final filenames
    - _Requirements: R4.3, R4.5_

- [x] 17. **M3 Gate**: Validate canonicalization and multi-phase flow





  - Test *.core.yaml canonicalization yields identical hashes for permuted inputs
  - Run Requirements → Design → Tasks flow end-to-end
  - Verify resume functionality from intermediate phases
  - _Requirements: R12.1, R1.1, R4.2_

## Milestone 4: Reviews, Fixups, and Status

- [x] 18. Implement review and fixup system with preview/apply modes




  - [x] 18.1 Create fixup detection and parsing with preview/apply split


    - Detect "FIXUP PLAN:" and "needs fixups" markers in review output
    - Parse unified diff blocks with ---/+++ headers using git apply --check validation (no --unidiff-zero by default)
    - Default = preview: parse & validate unified diffs against temp copy, capture diagnostics into warnings, list targets, no writes
    - --apply-fixups gate: run git apply --check first, then apply; consider --3way as last resort and record usage in receipt
    - _Requirements: R5.1, R5.2, R5.4, R5.5, R5.6_
t
  - [x] 18.2 Build phase orchestrator with rewind


    - Implement dependency resolution and execution order
    - Add rewind cap (e.g., 2 loops) with reason recorded in receipt
    - Handle NextStep::Rewind scenarios from fixup phase
    - _Requirements: R1.1, R4.2, R4.6, R5.4_

- [x] 19. Add status and clean commands





  - [x] 19.1 Implement status command


    - Show latest completed phase, artifacts with first-8 BLAKE3 hashes
    - Display last receipt path and effective configuration
    - Add source attribution showing CLI > file > defaults precedence
    - _Requirements: R2.6, R8.1, R8.2, R11.3_

  - [x] 19.2 Add verbose logging and observability


    - Implement --verbose flag with detailed operation logs
    - Log selected files, sizes, hashes (no secrets ever logged)
    - Add timing and resource usage tracking
    - _Requirements: R7.5, NFR5_

- [x] 20. **M4 Gate**: Validate reviews and status





  - Test review detects FIXUP PLAN: and surfaces at least one validated unified diff block
  - Verify status command shows complete phase information
  - Confirm verbose logging provides useful debugging information
  - _Requirements: R5.1, R2.6, R7.5_

## Milestone 5: Configuration and Source Plugins

- [x] 21. Implement configuration system with discovery and precedence





  - [x] 21.1 Create Config struct with TOML support and discovery


    - Implement hierarchical config: CLI > file > defaults
    - Add discovery process searching upward from CWD for .xchecker/config.toml; allow --config override
    - Support [defaults], [selectors], and [runner] sections with precedence tracking
    - _Requirements: R11.1, R11.2, R11.5_

  - [x] 21.2 Add configuration validation and reporting with source attribution


    - Validate config on load with helpful error messages
    - Implement effective config display for status command with source attribution
    - Show CLI > config > defaults precedence and source of each setting
    - _Requirements: R11.3, R11.5_

- [x] 22. Enhance Claude wrapper with model resolution




  - [x] 22.1 Add model alias resolution


    - Resolve model alias to full name once per run
    - Record both model_alias and model_full_name in receipts
    - Handle model resolution errors with helpful messages
    - _Requirements: R7.1_

- [x] 23. Add file locking with crash recovery


  - [x] 23.1 Implement robust file locking with advisory semantics



    - Use advisory lock with PID+start time at .xchecker/specs/<id>/.lock (coordinates xchecker processes, not a security boundary)
    - On stale lock, allow --force after age threshold
    - Document in README and errors that fd-lock is advisory and cannot prevent other tools from writing
    - Ensure clean refuses to run if lock present (unless --hard --force)
    - _Requirements: NFR3_

- [x] 24. **M5 Gate**: Validate configuration and locking





  - Test config precedence = CLI > file > defaults with status showing effective config
  - Verify file locking prevents concurrent execution with proper error codes
  - Confirm model alias resolution works correctly
  - _Requirements: R11.5, NFR3, R7.1_

## Milestone 6: Performance and Advanced Features

- [x] 25. Add fixup application (optional)






  - [x] 25.1 Build fixup application system (gated)





    - Gate with --apply-fixups flag; default to preview mode
    - Apply parsed diffs to upstream artifacts using git apply --check in dry-run
    - Update affected files and continue flow
    - List intended targets before application
    - _Requirements: R5.4, R5.6_

- [x] 26. Performance optimization and caching




  - [x] 26.1 Implement insight cache with BLAKE3 keys







    - Cache file summaries based on content hashes
    - Implement cache invalidation on file changes
    - Add cache hit/miss metrics for verbose mode
    - _Requirements: R3.4, R3.5_

  - [x] 26.2 Add performance monitoring and benchmarking


    - Benchmark on Linux/macOS/WSL; track P50/P95 for empty run and packetization
    - Implement timing for empty runs (target ≤ 5s)
    - Monitor packetization speed (target ≤ 200ms for 100 files)
    - Add memory usage tracking and bounded consumption
    - _Requirements: NFR1_

- [x] 27. Write comprehensive test suite






  - [x] 27.1 Create integration tests for full workflows

    - Test complete spec generation flows end-to-end
    - Verify resume scenarios and failure recovery
    - Test determinism with identical inputs producing same outputs
    - _Requirements: R1.1, R2.2, R2.5, R4.2_

  - [x] 27.2 Add property-based tests


    - Test canonicalization properties across transformations
    - Verify hash consistency for equivalent inputs
    - Test budget enforcement under various input conditions
    - _Requirements: R2.4, R2.5, R3.1, R12.1_



  - [x] 27.3 Create golden pipeline tests

    - Test with stub Claude CLI emitting various response types
    - Verify handling of truncated, malformed, and plain text responses
    - Test all error conditions and recovery scenarios
    - _Requirements: R4.1, R4.3, R4.4_

- [x] 28. **M6 Gate**: Validate performance and testing





  - Confirm empty run ≤ 5s; packetizer ≤ 200ms for 100 files
  - Verify all property tests pass with deterministic behavior
  - Test complete golden pipeline scenarios
  - _Requirements: NFR1, R2.5, R4.1_

## Milestone 7: Documentation and Release

- [x] 29. Complete CLI interface







  - [x] 29.1 Add remaining CLI commands and source resolution




    - Implement clean command with confirmation prompts
    - Add source resolution for GitHub, filesystem, stdin
    - Complete all CLI flags: --model, --source, --dry-run, --verbose, etc.
    - _Requirements: R1.4, R6.1, R6.2, R6.3, R7.1-R7.5, R8.1, R8.2_
-

- [x] 30. Final integration and documentation





  - [x] 30.1 Wire together all components















    - Integrate all systems into main CLI application
    - Ensure proper initialization and cleanup
    - Add final validation and smoke tests
    - _Requirements: All requirements_

  - [x] 30.2 Add CLI help and usage documentation


    - Create comprehensive help text for all commands and flags
    - Add examples for common usage patterns
    - Document configuration file format and options
    - Create traceability matrix in /docs/TRACEABILITY.md
    - _Requirements: R6.4, R11.3_
# Dead-code / warnings backlog

Generated: 2025-12-01
Total warnings: 60

This document categorizes all compiler warnings from `cargo check` into actionable groups.

---

## Future features (keep + #[allow(dead_code)])

These items are reserved for planned features and should be preserved with `#[allow(dead_code)]` annotations.

### LLM Backend Abstractions (V15+)
- **src/llm/mod.rs**
  - `ExecutionStrategy` (unused import)
    - REASON: Reserved for future LLM backend abstraction (V15+). Used in types but not yet wired through main execution path.
    - ACTION: Keep, add `#[allow(unused_imports)]` with comment

### Tool Mode / Permission System
- **src/claude.rs**
  - `with_allowed_tools` (line 119)
  - `with_disallowed_tools` (line 126)
  - `with_permission_mode` (line 133)
  - `execute_with_fallback_tracking` (line 470)
    - REASON: Reserved for tool-mode restrictions and external execution features
    - ACTION: Add `#[allow(dead_code)]` with comment about future tool-mode feature
  - `ClaudeResponse` fields: `metadata`, `output_format`, `runner_used`, `runner_distro`, `timed_out` (lines 39-53)
    - REASON: Metadata fields for future structured output and diagnostics
    - ACTION: Add `#[allow(dead_code)]` to struct

### Orchestrator Handle API
- **src/orchestrator/handle.rs**
  - `OrchestratorHandle` struct and all methods (lines 23-94)
    - REASON: Public API for library usage, not used internally but designed for external consumption
    - ACTION: Add module-level `#[allow(dead_code)]` with comment about library API

### Extended Redaction Features
- **src/redaction.rs**
  - `RedactionResult` fields: `matches`, `has_secrets` (lines 43-45)
    - REASON: Detailed redaction metadata for future structured reporting
    - ACTION: Add `#[allow(dead_code)]` to fields
  - `redact_strings`, `redact_optional`, `add_extra_pattern`, `add_ignored_pattern`, `get_pattern_ids`, `get_ignored_patterns` (lines 128-311)
    - REASON: Extended redaction API for custom patterns and batch operations
    - ACTION: Add `#[allow(dead_code)]` with comment about extended API

### Source Resolution System
- **src/source.rs**
  - `SourceType` variants: `GitHub`, `FileSystem` (lines 106-107)
  - `SourceContent` fields: `source_type`, `content`, `metadata` (lines 114-116)
    - REASON: Planned feature for GitHub issue/PR ingestion and multi-source specs
    - ACTION: Add `#[allow(dead_code)]` with comment about future source resolution

### Canonicalization Algorithms
- **src/canonicalization.rs**
  - `CANON_VERSION_YAML` (line 9)
  - `CANON_VERSION_MD` (line 10)
  - `canonicalize_yaml` method (line 46)
    - REASON: Future content-addressed storage and verification
    - ACTION: Add `#[allow(dead_code)]` with comment about future versioning

---

## Test harness glue / Test utilities

These items appear to be test infrastructure that may or may not be actively used.

### Artifact Testing
- **src/artifact.rs**
  - `Artifact::new` (line 28)
  - `Artifact::blake3_hash` field (line 22)
  - `ArtifactType::Context` variant (line 59)
  - Multiple `ArtifactManager` methods:
    - `new` (line 80)
    - `store_phase_artifact` (line 228)
    - `store_partial_artifact` (line 244)
    - `receipts_path` (line 311)
    - `read_artifact` (line 329)
    - `read_partial_artifact` (line 343)
    - `promote_partial_to_final` (line 362)
  - DECISION: Review if these are used in integration tests or dead; if dead, remove

### Atomic Write Testing
- **src/atomic_write.rs**
  - `read_file_with_crlf_tolerance` (line 242)
    - REASON: Test utility for cross-platform testing
    - DECISION: Check if used in tests; if not, remove

### Cache Testing
- **src/cache.rs**
  - `InsightCache::clear` (line 572)
  - `InsightCache::log_stats` (line 590)
    - DECISION: Review if needed for cache management or debugging

### Doctor CLI Integration
- **src/doctor.rs**
  - `DoctorCommand::new_from_cli` (line 71)
  - `DoctorCommand::run_with_options` (line 85)
    - REASON: CLI integration points that may be wired elsewhere
    - DECISION: Check CLI usage in main.rs / cli.rs

### Lock Management Utilities
- **src/lock.rs**
  - `FileLock::exists`, `release`, `spec_id`, `lock_info` (lines 424-473)
  - `force_remove_lock` (line 686)
    - DECISION: Utility functions for lock introspection/cleanup - may be needed for CLI commands

### Packet Builder Variants
- **src/packet.rs**
  - Multiple `PacketBuilder` constructors:
    - `with_cache` (line 381)
    - `with_limits` (line 392)
    - `with_limits_and_cache` (line 403)
    - `with_selector_and_limits` (line 419)
    - `with_redactor_selector_and_limits` (line 435)
    - `with_all_components` (line 452)
  - `PacketBuilder` accessors:
    - `redactor_mut`, `redactor`, `cache_mut`, `cache`, `set_cache`, `remove_cache` (lines 469-496)
  - `log_cache_stats` (line 798)
  - DECISION: Builder pattern methods - likely planned API surface; consider keeping with `#[allow]`

### Receipt Utilities
- **src/receipt.rs**
  - `error_to_exit_code_and_kind` (line 35)
  - `write_error_receipt_and_exit` (line 88)
  - `ReceiptManager::create_error_receipt` (line 371)
  - `add_rename_retry_warning` (line 474)
    - DECISION: Error handling utilities - check if used in error paths

### Runner Utilities
- **src/runner.rs**
  - `Runner::native` (line 217)
  - `Runner::description` (line 839)
  - `ClaudeResponse::stderr_for_receipt` (line 116)
    - DECISION: API methods for runner introspection

---

## Metadata fields (keep for structured output)

These fields are part of structured types and should be kept even if not actively read, as they support serialization/debugging.

### Phase Execution Metadata
- **src/phase.rs**
  - `PhaseMetadata` fields: `packet_hash`, `budget_used`, `duration_ms` (lines 148-152)
  - `PhaseResult::metadata` field (line 163)
  - `PhaseContext::config` field (line 29)
    - REASON: Structured metadata for receipts and diagnostics
    - ACTION: Add `#[allow(dead_code)]` - these support Debug/Serialize

### Packet Metadata
- **src/packet.rs**
  - `PriorityRules::low` field (line 47)
  - `SelectedFile` fields: `line_count`, `byte_count` (lines 235-237)
    - REASON: Metadata for budget tracking and diagnostics
    - ACTION: Keep for serialization

### Benchmark Metadata
- **src/benchmark.rs**
  - `BenchmarkResults::performance_metrics` (line 86)
    - REASON: Performance data for receipts
    - ACTION: Keep for serialization

### Runner Metadata
- **src/runner.rs**
  - `BufferConfig::stderr_receipt_cap_bytes` (line 59)
  - `ClaudeResponse` fields: `stdout_truncated`, `stderr_truncated`, `stdout_total_bytes`, `stderr_total_bytes` (lines 101-107)
    - REASON: Buffer management and truncation tracking
    - ACTION: Keep for diagnostics

### Fixup Metadata
- **src/fixup.rs**
  - `DiffHunk::new_range` (line 314)
  - `ChangeSummary` fields: `hunk_count`, `validation_messages` (lines 336-344)
    - REASON: Diff analysis metadata
    - ACTION: Keep for future diff reporting

---

## Error reporting (unused but part of API)

Error reporting utilities that aren't actively used but support the error system.

### Error Report API
- **src/error_reporter.rs**
  - `ErrorReport::minimal`, `with_context`, `print_to_stdout` (lines 27-105)
  - `ErrorReporter::report_error`, `report_minimal`, `should_show_details` (lines 131-150)
  - Standalone functions: `report_and_exit`, `report_error`, `report_minimal`, `should_show_details` (lines 162-194)
    - REASON: Alternative error display API not currently wired
    - DECISION: Either wire up or add `#[allow]` for future use

---

## Trait methods (interface completeness)

- **src/phase.rs**
  - `Phase::can_resume` trait method (line 180)
    - REASON: Trait interface completeness for resumable phases
    - ACTION: Keep as part of trait definition (even if implementations don't use it yet)

---

## Exit codes (test-only usage)

- **src/exit_codes.rs**
  - `codes::SUCCESS` constant (line 12)
    - REASON: Used in tests (line 103: `assert_eq!(codes::SUCCESS, 0);`)
    - ACTION: Keep - it's used in test module at bottom of file

---

## LLM types (builder pattern / API surface)

- **src/llm/types.rs**
  - `Message::system`, `Message::assistant` (lines 60-72)
  - `LlmInvocation` fields: `spec_id`, `phase_id` (lines 81-83)
  - `LlmInvocation::with_metadata` (line 116)
  - `LlmResult::with_tokens` (line 162)
    - REASON: Builder pattern methods and metadata fields for LLM abstraction
    - ACTION: Add `#[allow(dead_code)]` - part of public API surface

---

## Logger utilities

- **src/logging.rs**
  - `Logger::log_cache_stats` (line 861)
    - REASON: Diagnostic logging utility
    - DECISION: Wire up or mark `#[allow]`

---

## Status output

- **src/status.rs**
  - `StatusManager::emit_json_pretty` (line 209)
    - REASON: Alternative JSON formatting (vs compact)
    - ACTION: Add `#[allow]` or remove if superseded

---

## Miscellaneous utilities

- **src/fixup.rs**
  - `FixupMode::as_str` (line 289)
    - REASON: String conversion for serialization/display
    - ACTION: Keep or use in Display impl

- **src/packet.rs**
  - `ContentSelector::with_patterns` (line 108)
    - REASON: Alternative constructor
    - ACTION: Keep with `#[allow]` or wire up

- **src/phase.rs**
  - `Packet::content`, `evidence`, `is_within_budget` (lines 66-90)
  - `NextStep::Complete` variant (line 18)
    - DECISION: Review phase execution logic to see if these should be wired

- **src/ring_buffer.rs**
  - `RingBuffer::len`, `is_empty` (lines 52-58)
    - REASON: Standard collection API
    - ACTION: Add `#[allow]` - part of complete API

---

## Candidates for immediate deletion

These appear to have no clear use case:

1. **src/redaction.rs**
   - `redact_user_optional` (line 386)
   - `redact_user_strings` (line 398)
   - REASON: Duplicates of methods in `SecretRedactor`

---

## Summary statistics

- **Future features**: ~25 items (tool mode, LLM backend, source resolution, canonicalization)
- **Test/harness glue**: ~20 items (needs review to determine if used in tests)
- **Metadata fields**: ~15 items (keep for serialization)
- **API surface**: ~15 items (builder patterns, trait methods)
- **Deletion candidates**: ~2-5 items (clear duplicates)

---

## Recommended next steps

1. **Add `#[allow(dead_code)]` annotations** to "Future features" section with explanatory comments
2. **Review test usage** of items in "Test harness glue" section
3. **Wire up or remove** error reporting alternatives
4. **Delete** clear duplication in redaction.rs
5. **Run `cargo check` again** to verify remaining warnings

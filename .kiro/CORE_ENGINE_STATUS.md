# Core Engine Status

## Baseline Status

**Commit Hash:** `07e1eff8d44bf3701f61500046224920c725bb1d`

**Date:** 2025-12-02

### Library Tests (`cargo test --lib`)

**Status:** ✅ PASSED

- **Total Tests:** 598
- **Passed:** 595
- **Failed:** 0
- **Ignored:** 3
- **Duration:** 82.53s

**Warnings:**
- 3 compiler warnings (unused imports and dead code)

### Integration Tests (Local-Green Profile)

**Command:**
```bash
cargo test --tests -- --skip requires_claude_stub --skip requires_real_claude --skip requires_xchecker_binary --skip requires_future_phase --skip requires_future_api --skip requires_refactoring --skip windows_ci_only
```

**Status:** ❌ FAILED

**Failure Details:**

The integration tests failed to compile due to missing field errors in `tests/test_cli_flags.rs`:

1. **Error 1 (Line 261):** Missing field `execution_strategy` in initializer of `xchecker::CliArgs`
2. **Error 2 (Line 405):** Missing field `execution_strategy` in initializer of `xchecker::CliArgs`

**Additional Warnings:**
- 1 warning in `integration_full_workflows` test (unused `mut`)
- 3 warnings in `test_doc_validation` test (dead code)
- 11 warnings in binary compilation (unused functions, dead code)

### Summary

The **library tests are fully passing**, which indicates the core engine logic is functioning correctly. However, the **integration tests fail to compile** due to missing field initialization in test code. This is a test infrastructure issue rather than a core engine functionality issue.

The `execution_strategy` field was likely added to `CliArgs` struct but the test files were not updated accordingly.

### Next Steps

To achieve local-green status:
1. Update `tests/test_cli_flags.rs` at lines 261 and 405 to include the `execution_strategy` field
2. Re-run the integration test suite
3. Address any remaining test failures

---

## Public API Surfaces

As of commit `07e1eff`, the following public surfaces are exposed and used correctly:

### Orchestrator Module
- **`OrchestratorHandle`** ✅ - Primary entry point, correctly used by CLI and tests
- **`OrchestratorConfig`** ✅ - Configuration for handle behavior
- **`ExecutionResult`** ✅ - Return type from phase execution
- **`PhaseOrchestrator`** ⚠️ - Exposed for tests and status integration (will be refined in ORC-003)

### LLM Module
- **`LlmBackend` trait** ✅ - Abstraction layer, used only within orchestrator
- **`llm::from_config()`** ✅ - Factory function for backend creation

### Config Module
- **`Config`, `CliArgs`** ✅ - Main configuration objects
- **`Config::discover()`** ✅ - Factory for building config from sources

### CLI Module
- Only imports `OrchestratorHandle` and `OrchestratorConfig` ✅

**Architectural Rules Validated:**
- External orchestrator access goes through `OrchestratorHandle` ✅
- `LlmBackend` is the only LLM entrypoint from orchestrator ✅
- Config follows clean factory pattern ✅

---

## Test Inventory

**Test Files:** 97
**Test Functions:** 853 total
- **Local-Green:** 791 (92.7%) - No external dependencies
- **Ignored:** 62 (7.3%)

### Breakdown by Ignore Reason
| Reason | Count | % of Total |
|--------|-------|------------|
| `requires_claude_stub` | 49 | 5.7% |
| `requires_real_claude` | 4 | 0.5% |
| `requires_xchecker_binary` | 2 | 0.2% |
| `requires_refactoring` | 2 | 0.2% |
| `requires_future_phase` | 2 | 0.2% |
| `requires_future_api` | 2 | 0.2% |
| `windows_ci_only` | 1 | 0.1% |

**Local-Green CI Readiness:** EXCELLENT (92.7% coverage without external dependencies)

---

## Phase 4 - LLM Skeleton Hardening (2025-12-02)

### Config Enforcement (A.1)
✅ **Completed** - `src/config.rs`
- Provider defaults to `"claude-cli"` if unset
- Only `"claude-cli"` provider accepted (V11-V14 constraint)
- Execution strategy defaults to `"controlled"` if unset
- Only `"controlled"` strategy accepted (V11-V14 constraint)
- Clear error messages for unsupported values
- 10 new config validation tests added

### LLM Backend Factory Guardrails (A.2)
✅ **Completed** - `src/llm/mod.rs`
- `from_config()` validates provider and strategy
- Only accepts `"claude-cli"` + `"controlled"` combination
- Returns appropriate `LlmError::Unsupported` for invalid values
- 5 factory validation tests added
- Defense-in-depth with config-level validation

### Doctor LLM Provider Checks (A.3)
✅ **Completed** - `src/doctor.rs`
- New `check_llm_provider()` validates LLM configuration
- Checks Claude CLI binary discoverability (PATH validation)
- Platform-specific checks (Windows `where`, Unix `which`, WSL fallback)
- No actual LLM invocations (discovery only)
- Clear pass/warn/fail messages

### Doctor LLM Tests (A.4)
✅ **Completed** - `tests/test_doctor_llm_checks.rs`
- 12 comprehensive tests for doctor LLM validation
- Tests default provider behavior, invalid paths, unsupported providers
- Isolated from real LLM calls (PATH isolation, temp files)
- All tests passing

### Provider Selection Tests (A.5)
✅ **Completed** - `tests/test_llm_provider_selection.rs`
- 16 comprehensive provider/strategy validation tests
- Tests defaults, valid values, invalid providers, execution strategies
- Case sensitivity and error message validation
- All tests passing

### LLM Documentation (A.6)
✅ **Completed**
- Updated `docs/ORCHESTRATOR.md` with "LLM Layer (V11 Skeleton)" section
- Updated `docs/CONFIGURATION.md` with `[llm]` section details
- Documented V11-V14 constraints and V15+ roadmap
- Cross-references between all docs established

---

## Engine Invariants Implemented

The following invariants are now enforced by tests in `tests/test_engine_invariants.rs`:

### B3.7: ExternalTool Execution Strategy Rejected
✅ **Test:** `test_externaltool_execution_strategy_rejected`
- Validates that `execution_strategy = "externaltool"` fails config validation
- Clear error message about V11-V14 constraints

### B3.8: Packet Construction Validation
✅ **Test:** `test_packet_construction_in_execute_phase_core`
- `packet_evidence.max_bytes` and `max_lines` match configured limits
- `packet_evidence.files` is non-empty when content exists
- Packet files exist on disk

### B3.9: Prompt/Packet Consistency
✅ **Test:** `test_prompt_packet_consistency`
- Both prompt and packet building paths are executed
- Packet evidence consistently populated across phases
- Cross-phase artifact inclusion validated

### B3.10: Phase Transition Error Specificity
✅ **Tests in:** `tests/test_phase_transition_validation.rs`
- Invalid transitions produce specific `InvalidTransition` errors
- Missing dependencies produce `DependencyNotSatisfied` errors
- Error messages are actionable

### B3.11-B3.15: Packet Evidence & Receipt Validation
✅ **Tests:**
- `test_packet_evidence_round_trip_validation` - Packet limits match config
- `test_pipeline_execution_strategy_consistency` - Always "controlled"
- `test_receipt_required_fields_populated` - All 16 required fields present
- `test_packet_file_count_matches_actual_files` - Evidence matches reality
- `test_receipt_consistency_across_executions` - Consistent structure

### B3.12-B3.13: Error Receipt Metadata
✅ **Tests in:** `tests/test_error_receipt_metadata.rs` (9 tests)
- `error_kind` set appropriately based on error type
- `error_reason` non-empty and redacted
- `pipeline.execution_strategy == "controlled"` even on error
- Timestamps and phase metadata populated on errors

---

## Tech Debt / Upcoming Cleanup

### Warnings to Address
- Unused imports in some test files
- Dead code warnings in dry-run paths
- Some `#[allow]` attributes need TODO tags

### Visibility Refinements (ORC-003)
- Continue tightening `pub` → `pub(crate)` for internal APIs
- Document test-only surfaces more clearly

### Future Work
- ORC-002: Extract `execute_phase_core` as standalone function
- V15+: Support additional LLM providers (gemini-cli, openrouter, anthropic)
- V15+: Support additional execution strategies

---

*This document was established by Agent 0.1 - Baseline Verifier and updated by Agent C.1 - CORE_ENGINE_STATUS*

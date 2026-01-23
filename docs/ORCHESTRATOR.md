# Orchestrator System

The orchestrator system is the core execution engine of xchecker, responsible for managing the lifecycle of spec generation phases. It coordinates LLM invocations, artifact management, receipt generation, and multi-phase workflows with rewind support.

## Overview

### What PhaseOrchestrator Does

`PhaseOrchestrator` is the central component that executes spec generation phases end-to-end. It manages:

- **Phase execution**: Validates dependencies, builds packets, invokes LLMs, and processes responses
- **Artifact lifecycle**: Stores artifacts atomically using staging directories (`.partial/`) before promotion
- **Receipt generation**: Creates audit trails with cryptographic hashes and execution metadata
- **Lock management**: Ensures exclusive access to prevent concurrent modifications
- **Error handling**: Preserves partial artifacts on failure for debugging

### Relationship to CLI and Kiro

```
┌─────────────────────────────────────────────────────────────┐
│  CLI / Kiro Agent                                           │
│  (User-facing interface)                                    │
└───────────────────────────┬─────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│  OrchestratorHandle                                         │
│  (Safe façade for external callers)                         │
│  - run_phase(), can_run_phase(), current_phase()           │
└───────────────────────────┬─────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│  PhaseOrchestrator                                          │
│  (Core orchestration engine)                                │
│  - execute_phase(), validate_transition()                   │
│  - execute_complete_workflow()                              │
└───────────────────────────┬─────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│  Sub-modules                                                │
│  ┌────────────┬────────────┬────────────┬──────────────┐   │
│  │ llm.rs     │ phase_exec │ workflow   │ handle.rs    │   │
│  │            │ .rs        │ .rs        │              │   │
│  └────────────┴────────────┴────────────┴──────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

- **CLI**: Invokes orchestrator via `OrchestratorHandle` for commands like `xchecker run requirements`
- **Kiro**: Uses `OrchestratorHandle` programmatically to execute phases, inspect state, and resume workflows
- **Handle**: Provides a stable, simplified API that abstracts internal orchestrator complexity

### Integration Rule

> **Outside `src/orchestrator/`, use `OrchestratorHandle`.**
> Direct `PhaseOrchestrator` usage is reserved for tests and orchestrator internals.

This rule ensures:
- **Stable API**: Handle methods are designed for external consumption and won't change unexpectedly
- **Proper locking**: Handle constructors manage locks correctly (`with_force`, `readonly`)
- **Future-proofing**: Internal orchestrator changes won't break external callers

**Exceptions**: White-box tests that intentionally probe orchestrator internals may use `PhaseOrchestrator` directly. Such tests should include a comment like:
```rust
// White-box test: intentionally bypass OrchestratorHandle to probe internals.
```

## Modules

### `llm.rs` – LLM Provider Abstraction

Handles LLM backend integration and configuration. In V11+, this module supports multiple LLM providers through the `LlmBackend` trait abstraction.

**Key responsibilities:**
- Construct LLM backend from configuration (currently `ClaudeCliBackend` only)
- Build `LlmInvocation` from prompts and phase context
- Execute invocations and convert `LlmResult` to orchestrator-compatible format
- Extract metadata for receipt generation (`ClaudeExecutionMetadata`)

**Important types:**
- `ClaudeExecutionMetadata`: Captures model, version, runner info, and stderr for receipts
- `LlmResult → LlmInfo`: Conversion from invocation result to receipt metadata

**Dry-run vs real execution:**
- `dry_run: true`: Simulates LLM responses without actual API calls
- `dry_run: false`: Invokes real LLM backend (Claude CLI, Gemini CLI in future)

**Code Location:** `src/orchestrator/llm.rs`

### `phase_exec.rs` – Single Phase Execution; Receipts and Artifacts

Implements the core phase execution logic with comprehensive error handling and atomic artifact storage.

**Execution Engine:**
```
PhaseOrchestrator
  ├─ execute_requirements_phase / resume_from_phase / etc.
  ├─ execute_phase_with_timeout_handling
  ├─ execute_phase
  └─ execute_phase_core   ← shared engine (packet + secrets + LLM + artifacts + hashes)
```

**Execution flow:**
1. **Validate transition**: Check dependencies and legal phase transitions
2. **Build packet**: Collect context and artifacts from previous phases
3. **Secret scanning**: Scan packet content before LLM invocation (blocks on detection)
4. **LLM invocation**: Execute LLM or simulate in dry-run mode
5. **Postprocessing**: Parse LLM response into structured artifacts
6. **Staging**: Write artifacts to `.partial/` subdirectory
7. **Atomic promotion**: Rename from `.partial/` to final location
8. **Receipt generation**: Create audit trail with hashes and metadata

**Key types:**
- `ExecutionResult`: Contains success status, exit code, artifact paths, receipt path, and errors
- `PhaseTimeout`: Configurable timeout with sensible defaults (600s default, 5s minimum)

**Error handling:**
- **Timeout**: Writes partial artifact with timeout warning, exits with code 10
- **Claude failure**: Saves partial output, creates failure receipt with stderr_tail
- **Secret detection**: Prevents LLM invocation, exits with code 8

**Artifact storage:**
- Uses `.partial/` staging directory for atomic writes (FR-ORC-004)
- Promotes artifacts to final location only after all validations pass
- Cleans up partial artifacts on successful execution

**Code Location:** `src/orchestrator/phase_exec.rs`

### `workflow.rs` – Multi-Phase Workflow and Rewind Logic

Orchestrates complete workflows across multiple phases with support for rewinds (e.g., Review → Design).

**Standard phase order:**
```
Requirements → Design → Tasks → Review → Fixup → Final
```

**Rewind logic:**
- Phases can return `NextStep::Rewind { to: PhaseId }` to restart from an earlier phase
- Max rewind count: 2 (prevents infinite loops)
- Rewind information stored in receipts via `flags` field

**Key types:**
- `WorkflowResult`: Captures entire workflow execution including all rewinds
- `PhaseExecution`: Records success, rewind trigger, and target phase for each execution
- `PhaseExecutionResult`: Single phase result with rewind metadata

**Workflow execution:**
```rust
pub async fn execute_complete_workflow(&self, config: &OrchestratorConfig) -> Result<WorkflowResult>
```

Executes phases in sequence, handling dependencies and rewinds automatically. Workflow stops on:
- Phase execution failure
- Maximum rewind count exceeded
- Invalid rewind target

**Code Location:** `src/orchestrator/workflow.rs`

### `handle.rs` – Safe Façade for External Callers

Provides a clean, stable API for Kiro agents and external tools to interact with the orchestrator.

**Key methods:**
- `new(spec_id)`: Create handle with default config
- `readonly(spec_id)`: Create read-only handle (no locks)
- `run_phase(phase_id)`: Execute a specific phase with validation
- `can_run_phase(phase_id)`: Check if dependencies are satisfied
- `current_phase()`: Get the last successfully completed phase
- `legal_next_phases()`: Get allowed transitions from current state

**Configuration helpers:**
- `set_config(key, value)`: Set orchestrator configuration options
- `set_dry_run(bool)`: Enable/disable dry-run mode
- `with_config(spec_id, config)`: Create handle with custom configuration

**Example usage:**
```rust
let mut handle = OrchestratorHandle::new("my-spec")?;
handle.set_dry_run(true);
let result = handle.run_phase(PhaseId::Requirements).await?;
println!("Success: {}", result.success);
```

**Code Location:** `src/orchestrator/handle.rs`

## Phase Execution Engine

The orchestrator follows a unified execution architecture where all phase execution—whether invoked directly via `OrchestratorHandle::run_phase` or as part of a multi-phase workflow—flows through the same core execution pipeline.

### Unified Call Graph

```
┌─────────────────────────────────────────────────────────────────┐
│  OrchestratorHandle::run_phase(phase_id)                        │
│  (Public API - CLI, Kiro, external tools)                       │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│  PhaseOrchestrator::execute_requirements_phase()                │
│  PhaseOrchestrator::execute_design_phase()                      │
│  PhaseOrchestrator::execute_tasks_phase()                       │
│  (Phase-specific entry points)                                  │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│  PhaseOrchestrator::execute_phase()                             │
│  (Timeout wrapper + lock management)                            │
│  - Handles timeout enforcement                                  │
│  - Manages exclusive locks                                      │
│  - Delegates to core execution                                  │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│  execute_phase_core (future - ORC-002)                          │
│  (Shared execution pipeline)                                    │
│  1. Build packet with phase context                             │
│  2. Scan for secrets (blocks on detection)                      │
│  3. Execute LLM invocation via run_llm_invocation               │
│  4. Postprocess response into artifacts                         │
│  5. Stage artifacts to .partial/ directory                      │
│  6. Promote staged artifacts atomically                         │
│  7. Create receipt with hashes and metadata                     │
└─────────────────────────────────────────────────────────────────┘
```

### Workflow Integration

Multi-phase workflows in `workflow.rs` use the same execution pipeline:

```
┌─────────────────────────────────────────────────────────────────┐
│  PhaseOrchestrator::execute_complete_workflow()                 │
│  (Internal workflow orchestration)                              │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            │  For each phase in sequence:
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│  execute_phase_with_next_step_handling()                        │
│  (Workflow-specific wrapper)                                    │
│  - Handles NextStep::Continue / Rewind                          │
│  - Tracks rewind count                                          │
│  - Updates workflow state                                       │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
                    [Same execute_phase() path as above]
```

**Key insight**: Both single-phase and workflow execution converge on the same `execute_phase()` function, ensuring:
- Consistent receipt metadata (`llm.*`, `pipeline.execution_strategy`)
- Identical artifact handling (staging, promotion, hashing)
- Uniform error handling and timeout behavior
- Same secret scanning and LLM invocation path

### Shared Execution Pipeline

All phases follow this 10-step execution sequence (documented in `phase_exec.rs`):

1. **Remove stale .partial/ directories** - Clean up incomplete artifacts from previous runs
2. **Validate transition and acquire lock** - Check dependencies, ensure exclusive access
3. **Build packet with phase context** - Collect artifacts and configuration via `PacketBuilder`
4. **Scan for secrets** - Run `SecretRedactor` scan (blocks execution if secrets found)
5. **Execute LLM invocation** - Call `run_llm_invocation()` with unified backend
6. **Handle failures** - Save partial output, create failure receipt with stderr
7. **Postprocess response** - Parse LLM output into structured artifacts
8. **Stage artifacts** - Write to `.partial/` subdirectory for atomicity
9. **Promote atomically** - Rename from `.partial/` to final location
10. **Generate receipt** - Write audit trail with BLAKE3 hashes and metadata

### LLM Layer Integration

All LLM calls flow through the unified `run_llm_invocation()` function (ORC-001 ✅):

```
execute_phase()
    │
    ├─→ run_llm_invocation(prompt, config)
    │       │
    │       ├─→ make_llm_backend(config)  // Factory: creates ClaudeCliBackend
    │       │       │
    │       │       └─→ from_config(config)  // llm::from_config
    │       │
    │       ├─→ backend.invoke(invocation)  // LlmBackend trait
    │       │
    │       └─→ LlmResult  // Contains provider, model, tokens, timeout info
    │
    └─→ LlmResult.into_llm_info()  // Convert to receipt metadata
```

This unified path ensures:
- All receipts contain `llm.*` metadata (`provider`, `model_used`, `tokens_input/output`, `timed_out`)
- All receipts contain `pipeline.execution_strategy = "controlled"`
- Dry-run and real execution produce consistent receipt structure
- No legacy LLM paths (`execute_claude_cli` is no longer in hot path)

## Engine Invariants

The phase execution engine maintains these invariants, validated by tests in `tests/test_engine_invariants.rs`:

### B3.1: Packet Evidence Always Present
**Test**: `test_core_output_has_packet_evidence`

Every phase execution produces `packet_evidence` in receipts with:
- `max_bytes` (positive integer, default 65536)
- `max_lines` (positive integer, default 1200)
- `files` array with file metadata (path, hash, priority)

### B3.2: Successful Phases Produce Artifacts
**Test**: `test_core_output_success_has_artifacts`

When `exit_code == 0`:
- `artifact_paths` is non-empty
- All artifacts exist on disk
- Receipt `outputs` array matches artifact count

### B3.3: Output Hashes Match Artifacts
**Test**: `test_core_output_has_hashes`

Receipt `outputs` array:
- Length matches number of artifacts produced
- Each output has `blake3_canonicalized` field (64 hex characters)
- All hashes are non-empty and properly formatted

### B3.4: Phase Execution is Deterministic
**Test**: `test_phase_execution_deterministic`

Running the same phase multiple times produces:
- Same exit code
- Same number of artifacts
- Consistent receipt structure

### B3.5: Receipts Have Required Metadata
**Test**: `test_receipts_have_required_metadata`

All receipts contain:
- `schema_version`, `emitted_at`, `spec_id`, `phase`
- `xchecker_version`, `exit_code`, `packet`, `outputs`
- `flags`, `pipeline` (with `execution_strategy`)
- `llm` (object or null), `runner`

### B3.6: Artifacts Follow Naming Convention
**Test**: `test_artifacts_follow_naming_convention`

Artifacts use predictable naming:
- Requirements: `00-requirements.md`, `requirements.core.yaml`
- Design: `01-design.md`, `design.core.yaml`
- Tasks: `02-tasks.md`, `tasks.core.yaml`

### B3.7: ExternalTool Execution Strategy Rejected
**Test**: `test_externaltool_execution_strategy_rejected`

Configuration validation prevents:
- `execution_strategy = "externaltool"` (not supported in V11-V14)
- `execution_strategy = "external_tool"` (not supported in V11-V14)
- Only `"controlled"` strategy is accepted

### B3.8: Packet Construction Validated
**Test**: `test_packet_construction_in_execute_phase_core`

Packet evidence reflects actual packet content:
- Configured limits (`max_bytes`, `max_lines`) match receipt
- File entries have complete metadata (path, hash, priority)
- Design packets include Requirements artifacts

### B3.9: Prompt/Packet Consistency
**Test**: `test_prompt_packet_consistency`

Phase execution always:
- Builds packet (evidence in receipt)
- Creates packet preview files (in `.context/`)
- Includes prior phase artifacts in subsequent packets

### B3.11: Packet Evidence Round-Trip Validation
**Test**: `test_packet_evidence_round_trip_validation`

Packet evidence accurately reflects packet construction:
- File count matches actual files included
- BLAKE3 hashes are properly formatted (64 hex chars)
- Priority values are valid enum members (`high`, `medium`, `low`, `upstream`)
- Limits remain consistent across phases

### B3.12: Pipeline Execution Strategy Consistency
**Test**: `test_pipeline_execution_strategy_consistency`

All receipts have:
- `pipeline.execution_strategy = "controlled"` (enforced in V11-V14)
- Consistent strategy across all phases
- No variation between single-phase and workflow execution

### B3.13: Receipt Required Fields Populated
**Test**: `test_receipt_required_fields_populated`

Comprehensive validation that all receipt fields are:
- Present (not missing)
- Non-null (where required)
- Properly formatted (timestamps, versions, enums)

### B3.14: Packet File Count Matches Actual Files
**Test**: `test_packet_file_count_matches_actual_files`

When phases have dependencies:
- Packet evidence counts match actual artifact inclusion
- Design packets include Requirements artifacts
- All packet files have complete metadata

### B3.15: Receipt Consistency Across Executions
**Test**: `test_receipt_consistency_across_executions`

Multiple executions of the same phase produce:
- Same schema version, exit code, phase identifier
- Same packet limits and execution strategy
- Consistent structural fields

## Execution Flows

### Single-Phase Flow

```
┌──────────────────────┐
│ validate_transition  │  Check dependencies and legal transitions
└──────────┬───────────┘
           │
           ▼
┌──────────────────────┐
│ create_phase_context │  Gather available artifacts and config
└──────────┬───────────┘
           │
           ▼
┌──────────────────────┐
│ phase.make_packet()  │  Build packet with context
└──────────┬───────────┘
           │
           ▼
┌──────────────────────┐
│ SecretRedactor scan  │  Scan for secrets (blocks if found)
└──────────┬───────────┘
           │
           ▼
┌──────────────────────┐
│ LLM invocation       │  Execute Claude CLI or simulate
│ (via llm.rs)         │
└──────────┬───────────┘
           │
           ▼
┌──────────────────────┐
│ phase.postprocess()  │  Parse response into artifacts
└──────────┬───────────┘
           │
           ▼
┌──────────────────────┐
│ store_partial_staged │  Write to .partial/ directory
│ _artifact()          │
└──────────┬───────────┘
           │
           ▼
┌──────────────────────┐
│ promote_staged_to    │  Atomic rename to final location
│ _final()             │
└──────────┬───────────┘
           │
           ▼
┌──────────────────────┐
│ create_receipt()     │  Generate audit trail with hashes
│ write_receipt()      │
└──────────────────────┘
```

### Workflow with Rewind (Requirements → Design → Tasks → Review/Fixup)

```
┌─────────────┐
│ Requirements│  Phase 1: Generate requirements from rough spec
└──────┬──────┘
       │
       ▼
┌─────────────┐
│   Design    │  Phase 2: Create architecture & component design
└──────┬──────┘
       │
       ▼
┌─────────────┐
│    Tasks    │  Phase 3: Break down into implementation tasks
└──────┬──────┘
       │
       ▼
┌─────────────┐
│   Review    │  Phase 4: Validate and suggest fixups
└──────┬──────┘
       │
       │  NextStep::Rewind { to: Design }
       │  (Design needs revision based on review feedback)
       │
       ▼
┌─────────────┐
│   Design    │  Rewind #1: Re-run design with feedback
└──────┬──────┘
       │
       ▼
┌─────────────┐
│    Tasks    │  Re-run tasks with updated design
└──────┬──────┘
       │
       ▼
┌─────────────┐
│   Review    │  Re-review updated tasks
└──────┬──────┘
       │
       │  NextStep::Continue
       │
       ▼
┌─────────────┐
│    Fixup    │  Phase 5: Apply or preview fixups
└──────┬──────┘
       │
       ▼
┌─────────────┐
│    Final    │  Phase 6: Final assembly (future)
└─────────────┘
```

**Rewind constraints:**
- Maximum 2 rewinds per workflow execution
- Only backward rewinds allowed (e.g., Review → Design, not Design → Tasks)
- Rewind information stored in receipt `flags`: `rewind_triggered`, `rewind_target`

## LLM Layer

### Supported Providers

In V14+, xchecker supports multiple LLM providers through a unified `LlmBackend` abstraction.

**Supported Providers:**
- **`claude-cli`** (default): Uses the official Claude CLI tool
- **`gemini-cli`**: Uses Google's Gemini CLI
- **`openrouter`**: Uses OpenRouter HTTP API
- **`anthropic`**: Uses Anthropic HTTP API

**Supported Execution Strategy:**
- **`controlled`**: The only supported execution strategy.
  - LLMs propose changes via structured output (e.g., fixups)
  - All file modifications go through xchecker's fixup pipeline
  - No direct disk writes by LLMs
  - No external tool invocation by LLMs (planned for V15+)

**Configuration Validation:**

Attempting to use `execution_strategy = "externaltool"` will fail validation:

```toml
# ❌ This will fail validation
[llm]
execution_strategy = "externaltool"  # Reserved for V15+
```

**See Also:**
- **Configuration**: [CONFIGURATION.md](CONFIGURATION.md) for `[llm]` section details
- **Provider Details**: [LLM_PROVIDERS.md](LLM_PROVIDERS.md) for full provider guide

## LLM Behavior

### `dry_run` vs Real Claude vs `claude-stub`

#### 1. Dry-Run Mode (`dry_run: true`)
- **Use case**: Testing, validation, CI/CD without LLM costs
- **Behavior**: Simulates LLM responses with realistic content
- **Receipt**: Still generated with simulated `LlmInfo`
- **Example**: Requirements phase generates 3 sample requirements in EARS format

```rust
let config = OrchestratorConfig {
    dry_run: true,
    config: HashMap::new(),
};
let result = orchestrator.execute_requirements_phase(&config).await?;
// No actual Claude invocation, but artifacts and receipts are created
```

#### 2. Real Claude (`dry_run: false`)
- **Use case**: Production spec generation
- **Behavior**: Invokes Claude CLI with timeout and fallback handling
- **LLM backend**: `ClaudeCliBackend` (V11+)
- **Timeout**: Configurable via `phase_timeout` config option (default: 600s)
- **Fallback**: Automatically falls back to text parsing if JSON streaming fails

**Model selection:**
```rust
config.config.insert("model".to_string(), "sonnet".to_string());
```

#### 3. Claude Stub (`claude_cli_path` contains "claude-stub")
- **Use case**: Integration testing with controlled scenarios
- **Behavior**: Uses `claude-stub` binary with scenario support
- **Scenarios**: `success`, `failure`, `timeout`, etc.
- **Configuration**:

```rust
config.config.insert("claude_cli_path".to_string(), "/path/to/claude-stub".to_string());
config.config.insert("claude_scenario".to_string(), "success".to_string());
```

### How `LlmResult` Turns into `LlmInfo` on Receipts

When an LLM invocation completes, the result is converted to receipt metadata:

**LlmResult** (from backend):
```rust
pub struct LlmResult {
    pub raw_response: String,
    pub provider: String,           // "claude-cli"
    pub model_used: String,          // "sonnet"
    pub tokens_input: Option<usize>,
    pub tokens_output: Option<usize>,
    pub timed_out: bool,
    pub extensions: HashMap<String, serde_json::Value>,
}
```

**LlmInfo** (in receipt):
```rust
pub struct LlmInfo {
    pub provider: Option<String>,          // "claude-cli"
    pub model_used: Option<String>,        // "sonnet"
    pub tokens_input: Option<usize>,       // 1024
    pub tokens_output: Option<usize>,      // 512
    pub timed_out: Option<bool>,           // false
    pub timeout_seconds: Option<u64>,      // 600
    pub budget_exhausted: Option<bool>,    // false
}
```

**Conversion happens in `phase_exec.rs`:**
```rust
// Line ~666 in phase_exec.rs
receipt.llm = llm_result.map(|r| r.into_llm_info());
```

This ensures receipts always contain complete LLM metadata for audit and debugging purposes. The conversion:
1. Maps provider and model fields directly
2. Preserves token counts for cost tracking
3. Records timeout status for performance analysis
4. Adds timeout_seconds from configuration

## Examples

### Example 1: Run Requirements in Dry-Run Mode

```rust
use xchecker::orchestrator::{OrchestratorHandle, OrchestratorConfig};
use xchecker::types::PhaseId;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Configure for dry-run mode (no actual Claude invocation)
    let config = OrchestratorConfig {
        dry_run: true,
        config: HashMap::new(),
    };

    // Create handle with config
    let handle = OrchestratorHandle::with_config("my-spec", config)?;

    // Execute requirements phase
    let result = handle.run_phase(PhaseId::Requirements).await?;

    // Check results
    println!("Success: {}", result.success);
    println!("Exit code: {}", result.exit_code);
    println!("Artifacts: {:?}", result.artifact_paths);
    println!("Receipt: {:?}", result.receipt_path);

    // Output:
    // Success: true
    // Exit code: 0
    // Artifacts: ["artifacts/00-requirements.md", "artifacts/requirements.core.yaml"]
    // Receipt: Some("receipts/requirements/20250101T120000_00.receipt.json")

    Ok(())
}
```

### Example 2: Run Multiple Phases with Configuration

```rust
use xchecker::orchestrator::{OrchestratorHandle, OrchestratorConfig};
use xchecker::types::PhaseId;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Configure with custom model and timeout
    let mut config_map = HashMap::new();
    config_map.insert("model".to_string(), "sonnet".to_string());
    config_map.insert("phase_timeout".to_string(), "900".to_string()); // 15 minutes
    config_map.insert("apply_fixups".to_string(), "true".to_string());

    let config = OrchestratorConfig {
        dry_run: false, // Use real Claude
        config: config_map,
    };

    // Create handle with config
    let handle = OrchestratorHandle::with_config("production-spec", config)?;

    // Execute phases sequentially
    let phases = [
        PhaseId::Requirements,
        PhaseId::Design,
        PhaseId::Tasks,
        PhaseId::Review,
    ];

    for phase in phases {
        println!("Running {:?} phase...", phase);
        let result = handle.run_phase(phase).await?;
        if !result.success {
            eprintln!("Phase {:?} failed: {:?}", phase, result.error);
            break;
        }
        println!("  ✓ {:?} completed", phase);
    }

    Ok(())
}
```

### Example 3: Inspect Latest Receipt for a Spec

```rust
use xchecker::orchestrator::OrchestratorHandle;
use xchecker::types::PhaseId;

fn main() -> anyhow::Result<()> {
    // Create read-only handle (no locks acquired)
    let handle = OrchestratorHandle::readonly("my-spec")?;

    // Read the latest receipt for Requirements phase
    let receipt = handle
        .receipt_manager()
        .read_latest_receipt(PhaseId::Requirements)?;

    if let Some(receipt) = receipt {
        println!("Exit code: {}", receipt.exit_code);
        println!("Model: {}", receipt.model_full_name);
        println!("Outputs: {} files", receipt.outputs.len());

        // Check LLM metadata
        if let Some(llm_info) = &receipt.llm {
            println!("Provider: {:?}", llm_info.provider);
            println!("Tokens in: {:?}", llm_info.tokens_input);
            println!("Tokens out: {:?}", llm_info.tokens_output);
        }

        // Check execution strategy
        if let Some(pipeline) = &receipt.pipeline {
            println!("Execution strategy: {:?}", pipeline.execution_strategy);
        }

        // Check for rewinds
        if receipt.flags.contains_key("rewind_triggered") {
            println!("Rewind to: {:?}", receipt.flags.get("rewind_target"));
        }
    } else {
        println!("No receipt found for Requirements phase");
    }

    Ok(())
}
```

## Pointers to Test Files

The following test files illustrate orchestrator behavior and provide additional examples:

### `tests/test_llm_receipt_metadata.rs`

**Tests LLM metadata in receipts** (V11+ multi-provider support)

- Validates `LlmInfo` structure with provider, model, tokens
- Validates `PipelineInfo` with execution_strategy = "controlled"
- Property tests for controlled execution (no direct disk writes)
- Demonstrates backward compatibility with old receipts

**Key tests:**
- `test_execution_strategy_in_receipt_controlled`: Verifies pipeline.execution_strategy appears in receipts
- `test_llm_metadata_in_receipt`: Validates LlmInfo structure and serialization
- `property_controlled_execution_no_disk_writes`: Property test ensuring controlled mode prevents direct writes
- `property_controlled_mode_enforces_fixup_pipeline`: Verifies all writes go through fixup system

**Example:**
```rust
// From test_llm_receipt_metadata.rs
#[test]
fn test_llm_metadata_in_receipt() {
    let (manager, _temp_dir) = create_test_manager();

    let mut receipt = manager.create_receipt(/* ... */);

    // Add LLM metadata
    receipt.llm = Some(LlmInfo {
        provider: Some("claude-cli".to_string()),
        model_used: Some("sonnet".to_string()),
        tokens_input: Some(1024),
        tokens_output: Some(512),
        timed_out: Some(false),
        // ...
    });

    // Verify serialization includes all fields
    let json = serde_json::to_string_pretty(&receipt).unwrap();
    assert!(json.contains("\"provider\": \"claude-cli\""));
}
```

### `tests/test_workflow_receipt_regression.rs`

**Regression tests for workflow receipt consistency**

- Validates all phases generate receipts with consistent metadata
- Verifies LLM info and pipeline info are properly populated
- Tests packet evidence preservation across phases

**Key tests:**
- `test_requirements_receipt_has_llm_info`: Ensures llm field exists in schema
- `test_requirements_receipt_has_pipeline_info`: Validates execution_strategy = "controlled"
- `test_receipt_packet_evidence_preserved`: Verifies packet metadata survives to receipt
- `test_multi_phase_receipts_consistent_metadata`: Checks metadata consistency across phases

**Example:**
```rust
// From test_workflow_receipt_regression.rs
#[tokio::test]
async fn test_requirements_receipt_has_pipeline_info() -> Result<()> {
    let (orchestrator, _temp) = setup_test("pipeline-info");
    let config = dry_run_config();

    let result = orchestrator.execute_requirements_phase(&config).await?;

    // Read and verify receipt
    let receipt_path = result.receipt_path.expect("Should have receipt path");
    let content = std::fs::read_to_string(&receipt_path)?;
    let receipt: serde_json::Value = serde_json::from_str(&content)?;

    // Verify pipeline info
    let pipeline = &receipt["pipeline"];
    assert_eq!(
        pipeline["execution_strategy"].as_str(),
        Some("controlled"),
        "Execution strategy should be controlled"
    );

    Ok(())
}
```

### Other Relevant Test Files

- **`tests/test_phase_timeout.rs`**: Tests timeout handling with partial artifact preservation
- **`tests/integration_full_workflows.rs`**: End-to-end workflow tests with multiple phases
- **`tests/test_secret_scanning_ci.rs`**: Tests secret detection before LLM invocation
- **`src/orchestrator/mod.rs`**: Unit tests within orchestrator module itself

## Configuration Reference

### OrchestratorConfig Options

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `dry_run` | bool | false | Simulate LLM responses without API calls |
| `model` | String | "sonnet" | LLM model to use |
| `phase_timeout` | u64 | 600 | Timeout in seconds (min: 5s) |
| `max_turns` | u32 | N/A | Maximum conversation turns for LLM |
| `apply_fixups` | bool | false | Apply fixups (true) or preview (false) |
| `debug_packet` | bool | false | Write full debug packet to context dir |
| `claude_cli_path` | String | auto-detect | Path to Claude CLI binary |
| `claude_scenario` | String | "success" | Scenario for claude-stub testing |
| `runner_mode` | String | "auto" | Runner mode: native, wsl, docker |
| `runner_distro` | String | N/A | WSL distro name if using WSL runner |
| `strict_validation` | bool | false | Treat validation failures as hard errors |

### Strict Validation Mode

Strict validation mode controls how the orchestrator handles low-quality or invalid LLM output.

**Configuration Flow:**

```
[defaults]                    Config::strict_validation()
strict_validation = true  →   OrchestratorConfig.strict_validation  →  PhaseContext.strict_validation
                                                                              ↓
                                                                    Phase::postprocess() checks
                                                                              ↓
                                                          ValidationFailed error (if strict) or warning (if soft)
```

**Where `strict_validation` is Read:**
- `Config::strict_validation()` in `src/config.rs` - reads from config file
- CLI flags `--strict-validation` / `--no-strict-validation` override config
- Merged into `OrchestratorConfig` during orchestrator initialization

**How It Flows Through Execution:**
1. `OrchestratorConfig.strict_validation` is set from config + CLI flags
2. `PhaseContext.strict_validation` is populated in `create_phase_context()` (`src/orchestrator/phase_exec.rs`)
3. Each phase's `postprocess()` method checks `ctx.strict_validation`
4. `OutputValidator::validate()` runs regardless of mode

**Behavior Difference:**

| Mode | On Validation Failure | Exit Code | Effect |
|------|----------------------|-----------|--------|
| Soft (`false`) | Log warning to stderr | 0 (success) | Artifacts written, execution continues |
| Strict (`true`) | Return `ValidationFailed` error | 1 | Phase fails, no artifacts written |

**Example Configuration:**

```toml
# .xchecker/config.toml
[defaults]
strict_validation = true  # Fail on low-quality LLM output
```

**CLI Override:**

```bash
# Enable strict mode for this run only
xchecker spec my-spec --strict-validation

# Disable strict mode even if config enables it
xchecker spec my-spec --no-strict-validation
```

**What Triggers Validation Failures:**
- Meta-summary responses ("Since no specific problem statement was provided...")
- Missing required sections in phase output
- Malformed markdown structure
- Empty or placeholder content

**See Also:**
- [CONFIGURATION.md](CONFIGURATION.md) - Full config reference including `[defaults].strict_validation`
- [TEST_MATRIX.md](TEST_MATRIX.md) - FR-VLD test coverage for validation

### Exit Codes

| Code | Constant | Meaning |
|------|----------|---------|
| 0 | SUCCESS | Phase completed successfully |
| 8 | SECRET_DETECTED | Secret found in packet, execution blocked |
| 10 | PHASE_TIMEOUT | Phase exceeded configured timeout |
| 1 | GENERAL_ERROR | General execution failure |

See `src/exit_codes/codes.rs` for complete list.

## Best Practices

### 1. Always Use Handles for External Integration
```rust
// ✓ Good: Using handle
let handle = OrchestratorHandle::new("spec")?;
let result = handle.run_phase(PhaseId::Requirements).await?;

// ✗ Bad: Direct orchestrator usage from external tools
let orch = PhaseOrchestrator::new("spec")?;
orch.execute_requirements_phase(&config).await?;
```

### 2. Use Read-Only Mode for Inspection
```rust
// When you only need to read state, use readonly to avoid locks
let handle = OrchestratorHandle::readonly("spec")?;
let current = handle.current_phase()?;
```

### 3. Handle Partial Artifacts
```rust
if !result.success {
    eprintln!("Phase failed: {:?}", result.error);
    eprintln!("Partial artifact: {:?}", result.artifact_paths);
    // Partial artifacts are preserved in .partial/ for debugging
}
```

### 4. Configure Appropriate Timeouts
```rust
// For complex phases or slow models, increase timeout
config.config.insert("phase_timeout".to_string(), "1200".to_string()); // 20 min
```

### 5. Always Check Dependencies
```rust
if handle.can_run_phase(PhaseId::Design)? {
    handle.run_phase(PhaseId::Design).await?;
} else {
    let current = handle.current_phase()?;
    println!("Cannot run Design. Current phase: {:?}", current);
}
```

## Testing Hooks

The following methods are exposed for testing and inspection purposes. They are considered internal API and may change without notice.

### Public Helpers (Safe to Use)

| Method | Visibility | Usage | Narrowing Status |
|--------|------------|-------|------------------|
| `artifact_manager()` | `pub` | Access artifact storage for verification | Keep - heavily used (40+ call sites) |
| `receipt_manager()` | `pub` | Access receipt storage for audit trail | Keep - heavily used (20+ call sites) |
| `get_current_phase_state()` | `pub` | Get the last successfully completed phase | Consider `pub(crate)` - test-only API |
| `can_resume_from_phase_public()` | `pub` | Check if resuming from a specific phase is valid | Consider `pub(crate)` - test-only API |

**Note**: `artifact_manager()` and `receipt_manager()` are exposed via `OrchestratorHandle` and should remain public. The other two methods are primarily used in white-box tests and may be narrowed to `pub(crate)` in a future version once all external callers migrate to `OrchestratorHandle`.

### Internal Methods (Do Not Use)

These methods are marked `pub` for internal module access but should not be called from external code:

- `validate_transition()` - Internal phase transition validation
- `check_dependencies_satisfied()` - Internal dependency checking
- `run_llm_invocation()` - Internal LLM execution (unified in ORC-001)

### Test Configuration

For integration tests, use `OrchestratorConfig` with `dry_run: true` to avoid external dependencies:

```rust
let config = OrchestratorConfig::default();
config.set("dry_run", "true");
```

See `docs/TEST_MATRIX.md` for the complete test inventory and classification.

## Orchestrator vNext Backlog

Future improvements to consider (non-urgent, tracked for deliberate scheduling):

### ORC-001 – Unify LLM Invocation ✅ COMPLETED

**Goal**: Ensure `workflow.rs` uses `run_llm_invocation` instead of `execute_claude_cli`.

**Status**: ✅ COMPLETED. Both `phase_exec.rs` and `workflow.rs` now use `run_llm_invocation` for LLM calls. The legacy `execute_claude_cli` is no longer in the hot path.

**Impact**: All receipts (single-phase + workflow) now populate `llm` consistently via `LlmResult::into_llm_info`. workflow.rs now uses run_llm_invocation; legacy execute_claude_cli is no longer in hot path.

### ORC-002 – Extract `execute_phase_core`

**Goal**: Factor out the shared "packet → secret scan → LLM → staged artifacts → receipt" logic.

**Context**: Both `phase_exec.rs` and `workflow.rs` implement similar receipt/artifact handling. Extracting a common helper would reduce duplication while keeping tests green.

**Impact**: Reduced code duplication, easier maintenance.

### ORC-003 – Narrow Public Helpers

**Goal**: Once `OrchestratorHandle` is widely adopted, reconsider whether `artifact_manager`, `receipt_manager`, `get_current_phase_state`, and `can_resume_from_phase_public` can be:
- Moved behind a dedicated test helper module, or
- Narrowed to `pub(crate)` again

**Context**: These were exposed for interop during migration. With the façade pattern established, they may be narrower than necessary.

**Impact**: Cleaner public API surface.

## See Also

- [CONFIGURATION.md](CONFIGURATION.md) - Full configuration reference and LLM/runner knobs
- [SECURITY.md](SECURITY.md) - Secret detection and redaction
- [TRACEABILITY.md](TRACEABILITY.md) - Requirements traceability
- [TEST_MATRIX.md](TEST_MATRIX.md) - Complete test inventory and classification
- [CORE_ENGINE_STATUS.md](../.kiro/CORE_ENGINE_STATUS.md) - Current engine status and completion tracking
- [INDEX.md](INDEX.md) - Documentation navigation index

### Related Test Files

- `tests/test_engine_invariants.rs` - B3.1-B3.15 invariant validation tests
- `tests/test_orchestrator_handle_smoke.rs` - Canary test for OrchestratorHandle contract
- `tests/test_llm_receipt_metadata.rs` - LLM metadata validation (V11+ multi-provider)
- `tests/test_workflow_receipt_regression.rs` - Workflow receipt consistency tests

---

**Document Version**: 2.3
**Last Updated**: 2025-12-02
**Maintainer**: Agent C.2 - ORCHESTRATOR.md Engine Section

*Changes in 2.3: Added "Phase Execution Engine" section with unified call graph, "Engine Invariants" subsection documenting B3.1-B3.15 tests, integrated LLM Layer section (from A.6), added cross-references to TEST_MATRIX.md, CONFIGURATION.md, CORE_ENGINE_STATUS.md, and related test files.*

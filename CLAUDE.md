# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

xchecker is a Rust CLI tool for orchestrating spec generation workflows using LLMs. It transforms rough feature ideas into structured requirements, designs, and implementation tasks through a multi-phase pipeline.

## Workspace Layout

xchecker is a Cargo workspace with a stable facade in the root crate:

- `crates/xchecker-utils`: foundations (paths/sandbox, atomic write, logging, redaction, runner, types)
- `crates/xchecker-config`: config model + discovery + validation + builder + selectors
- `crates/xchecker-llm`: provider backends (claude-cli, gemini-cli, openrouter, anthropic)
- `crates/xchecker-engine`: orchestration + phases + packet + fixup + receipt + status + workspace + gate + hooks
- `src/lib.rs` + `src/main.rs`: stable API re-exports and CLI entrypoint for `xchecker`

## Build & Test Commands

```bash
# Build
cargo build                    # Debug build
cargo build --release          # Release build

# Run all tests (fast)
cargo test --workspace --lib --bins

# Run local-green tests (no external dependencies)
cargo test --workspace --lib && cargo test --workspace --tests -- --skip requires_claude_stub --skip requires_real_claude --skip requires_xchecker_binary

# Run tests with claude-stub (requires building stub first)
cargo build --bin claude-stub --features dev-tools
cargo test --workspace --tests -- --include-ignored --skip requires_real_claude

# Run a single test
cargo test test_name -- --nocapture

# Linting and formatting
cargo fmt -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Run benchmarks
cargo bench

# Documentation validation
cargo test --features dev-tools --test doc_validation -- --test-threads=1
```

## Architecture

### Phase Pipeline

xchecker executes specs through a sequential phase pipeline:
```
Requirements -> Design -> Tasks -> Review -> Fixup -> Final
```

Each phase:
1. Builds a **packet** (context gathered from artifacts)
2. Scans for secrets (blocks execution if detected)
3. Invokes LLM (Claude CLI, Gemini CLI, HTTP providers, or dry-run simulation)
4. Postprocesses response into artifacts
5. Writes artifacts atomically via `.partial/` staging
6. Generates receipts with BLAKE3 hashes

### Key Modules

| Module | Purpose |
|--------|---------|
| `crates/xchecker-engine/src/orchestrator/` | Core execution engine - phase lifecycle, LLM invocation, workflows |
| `crates/xchecker-engine/src/phases.rs` | Phase implementations (Requirements, Design, Tasks, Review, Fixup) |
| `crates/xchecker-engine/src/packet/` | Packet building with budget enforcement and file selection |
| `crates/xchecker-engine/src/receipt/` | JSON receipt generation with cryptographic hashes |
| `crates/xchecker-engine/src/fixup/` | Diff parsing and application with preview/apply modes; correctly handles implicit count defaults in hunk headers |
| `crates/xchecker-config/src/config/` | Config model, discovery, validation, builder, selectors |
| `crates/xchecker-utils/src/runner/` | Process execution (native/WSL, timeouts, Job Objects) |
| `crates/xchecker-utils/src/redaction.rs` | Secret detection and redaction |
| `crates/xchecker-llm/src/` | LLM backends (claude-cli, gemini-cli, openrouter, anthropic) |

### Integration Rule

**Outside `crates/xchecker-engine/src/orchestrator/`, always use `OrchestratorHandle`** - the stable facade for CLI and external tools. Do not use `PhaseOrchestrator` directly except in white-box tests.

```rust
// Correct
let handle = OrchestratorHandle::new("my-spec")?;
handle.run_phase(PhaseId::Requirements).await?;

// Avoid (internal API)
let orch = PhaseOrchestrator::new("my-spec")?;
```

## Testing Strategy

### Test Profiles

1. **Local-Green** (~30s): No external dependencies - `cargo test --workspace --lib`
2. **Stub Suite** (~3min): Integration with claude-stub mock
3. **Full Firehose** (~10min): Real Claude API (set `XCHECKER_ENABLE_REAL_CLAUDE=1`)

### Test Skip Markers

Tests requiring external resources are marked with skip flags:
- `requires_claude_stub` - Needs compiled claude-stub binary
- `requires_real_claude` - Needs real Claude CLI + API key
- `requires_xchecker_binary` - Needs compiled xchecker binary

### Running Specific Test Suites

```bash
# Schema validation
cargo test --features dev-tools --test doc_validation schema

# Doctor/health checks
cargo test doctor

# Packet tests
cargo test packet

# Orchestrator invariants
cargo test --test test_engine_invariants
```

### Test Utils Feature

`xchecker-utils` helpers live behind the `test-utils` feature. If a crate's tests need them, add a dev-dependency with `features = ["test-utils"]`.

## Key Patterns

### Artifact Naming Convention
- Requirements: `00-requirements.md`, `00-requirements.core.yaml`
- Design: `10-design.md`, `10-design.core.yaml`
- Tasks: `20-tasks.md`, `20-tasks.core.yaml`
- Review: `30-review.md`, `30-review.core.yaml`
- Fixup: `40-fixup.md`, `40-fixup.core.yaml`

### State Directory
xchecker stores state in `.xchecker/` (configurable via `XCHECKER_HOME`):
```
.xchecker/
  specs/<spec-id>/
    artifacts/    # Generated phase outputs
    receipts/     # Execution audit trails
    context/      # Packet previews for debugging
```

### JSON Output Contracts
All JSON outputs use JCS (RFC 8785) for canonical emission. Schema versions:
- `schemas/receipt.v1.json` - Execution receipts
- `schemas/status.v1.json` - Spec status
- `schemas/doctor.v1.json` - Health checks

### Exit Codes
| Code | Name | Description |
|------|------|-------------|
| 0 | SUCCESS | Operation completed |
| 7 | PACKET_OVERFLOW | Packet size exceeded |
| 8 | SECRET_DETECTED | Secret found in packet |
| 9 | LOCK_HELD | Lock already held |
| 10 | PHASE_TIMEOUT | Phase timed out |
| 70 | CLAUDE_FAILURE | Claude CLI failed |

## LLM Configuration (V11-V14)

Only the `controlled` execution strategy is supported. Providers supported in V14: `claude-cli`, `gemini-cli`, `openrouter`, `anthropic`. All file modifications go through the fixup pipeline - LLMs propose changes, xchecker applies them.

```toml
# .xchecker/config.toml
[llm]
provider = "claude-cli"           # or "gemini-cli", "openrouter", "anthropic"
execution_strategy = "controlled"  # Only supported value
```

## Current Sharp Edges / Follow-ups

- ConfigSource attribution: env overrides are currently labeled `Config`; decide if they should be `Programmatic` and update docs/tests consistently.
- Packet builder split is in progress: keep `packet::builder` as glue; future extractions likely cache adapter and file formatting.
- LLM binary path precedence in `crates/xchecker-engine/src/orchestrator/llm.rs` should be explicit (`llm_claude_binary` > `claude_path` > `claude_cli_path`).
- Process-group handling should live in the runner (`xchecker-utils`) rather than provider backends.
- Fixup path validation routes (`SandboxRoot::join()` and `validate_fixup_target()`) should stay policy-aligned to avoid divergence.

## Recent Improvements

### Fixup Parser Hunk Header Parsing (2026-01-28)

Fixed a bug in the fixup parser's hunk header regex pattern where optional count values were being incorrectly matched. The parser now correctly handles implicit count defaults when old_count or new_count is omitted in unified diff hunk headers.

**Before:** The regex pattern `r"^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@"` was using the wrong capture group index for old_count, causing incorrect parsing of hunk headers like `@@ -10 +11,2 @@`.

**After:** Corrected the capture group mapping to properly extract:
- `old_start` from capture group 1
- `old_count` (optional, defaults to 1) from capture group 2
- `new_start` from capture group 3
- `new_count` (optional, defaults to 1) from capture group 4

This fix ensures that the fixup parser correctly handles all valid unified diff formats, including those with implicit count values.

## Documentation

- [docs/ORCHESTRATOR.md](docs/ORCHESTRATOR.md) - Core engine architecture
- [docs/TESTING.md](docs/TESTING.md) - Test lanes and CI integration
- [docs/CONFIGURATION.md](docs/CONFIGURATION.md) - Full config reference
- [docs/CONTRACTS.md](docs/CONTRACTS.md) - JSON schema versioning
- [docs/LLM_PROVIDERS.md](docs/LLM_PROVIDERS.md) - Provider configuration
- [docs/WORKSPACE_GUIDE.md](docs/WORKSPACE_GUIDE.md) - Workspace and TUI usage

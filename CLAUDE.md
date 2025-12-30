# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

xchecker is a Rust CLI tool for orchestrating spec generation workflows using Claude AI. It transforms rough feature ideas into structured requirements, designs, and implementation tasks through a multi-phase pipeline.

## Build & Test Commands

```bash
# Build
cargo build                    # Debug build
cargo build --release          # Release build

# Run all tests (fast)
cargo test --lib --bins        # ~30s - unit tests only

# Run local-green tests (no external dependencies)
cargo test --lib && cargo test --tests -- --skip requires_claude_stub

# Run tests with claude-stub (requires building stub first)
cargo build --bin claude-stub
cargo test --tests -- --include-ignored --skip requires_real_claude

# Run a single test
cargo test test_name -- --nocapture

# Linting and formatting
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings

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
3. Invokes LLM (Claude CLI or dry-run simulation)
4. Postprocesses response into artifacts
5. Writes artifacts atomically via `.partial/` staging
6. Generates receipts with BLAKE3 hashes

### Key Modules

| Module | Purpose |
|--------|---------|
| `src/orchestrator/` | Core execution engine - phase lifecycle, LLM invocation, workflows |
| `src/phases.rs` | Phase implementations (Requirements, Design, Tasks, Review, Fixup) |
| `src/packet.rs` | Packet building with budget enforcement and file selection |
| `src/receipt.rs` | JSON receipt generation with cryptographic hashes |
| `src/fixup.rs` | Diff parsing and application with preview/apply modes |
| `src/redaction.rs` | Secret detection and redaction |
| `src/runner.rs` | Claude CLI execution (native/WSL) |
| `src/llm/` | LLM backend abstraction (currently claude-cli only) |

### Integration Rule

**Outside `src/orchestrator/`, always use `OrchestratorHandle`** - the stable facade for CLI and external tools. Do not use `PhaseOrchestrator` directly except in white-box tests.

```rust
// Correct
let handle = OrchestratorHandle::new("my-spec")?;
handle.run_phase(PhaseId::Requirements).await?;

// Avoid (internal API)
let orch = PhaseOrchestrator::new("my-spec")?;
```

## Testing Strategy

### Test Profiles

1. **Local-Green** (~30s): No external dependencies - `cargo test --lib`
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

Only `claude-cli` provider with `controlled` execution strategy is supported. All file modifications go through the fixup pipeline - LLMs propose changes, xchecker applies them.

```toml
# .xchecker/config.toml
[llm]
provider = "claude-cli"           # Only supported value
execution_strategy = "controlled"  # Only supported value
```

## Documentation

- [docs/ORCHESTRATOR.md](docs/ORCHESTRATOR.md) - Core engine architecture
- [docs/TESTING.md](docs/TESTING.md) - Test lanes and CI integration
- [docs/CONFIGURATION.md](docs/CONFIGURATION.md) - Full config reference
- [docs/CONTRACTS.md](docs/CONTRACTS.md) - JSON schema versioning

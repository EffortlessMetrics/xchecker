# xchecker

[![Crates.io](https://img.shields.io/crates/v/xchecker.svg)](https://crates.io/crates/xchecker)
[![License](https://img.shields.io/crates/l/xchecker.svg)](https://github.com/EffortlessMetrics/xchecker#license)

A Rust CLI tool for orchestrating spec generation workflows with Claude AI. Transform rough feature ideas into structured requirements, designs, and implementation tasks through an automated multi-phase pipeline.

## Features

- **Multi-Phase Orchestration**: Requirements -> Design -> Tasks -> Review -> Fixup -> Final with configurable phase_timeout (default: 600s). Phase execution exceeded timeout results in exit code 10.
- **Lockfile System**: Reproducibility tracking with `--create-lock` and `--strict-lock` flags. Detects lock_drift when model or CLI versions change between executions.
- **Fixup System**: Secure diff application with Preview Mode (default) and Apply Mode (`--apply-fixups`). Path validation prevents directory traversal attacks.
- **Ecosystem Features**:
  - **Workspace Orchestration**: Manage multi-spec projects with `xchecker project` commands.
  - **Templates**: Bootstrap new specs instantly with `xchecker template init` (Next.js, Rust, Python support).
  - **Hooks**: Customize workflows with pre/post-phase shell hooks.
  - **Policy Gates**: Enforce quality standards in CI/CD with `xchecker gate`.
- **Controlled Execution**: Only `controlled` strategy is supported; LLMs propose diffs and xchecker applies them.
- **Standardized Exit Codes**: Process exit codes always match receipt exit_code field for reliable automation and monitoring.
- **Versioned JSON Contracts**: Stable schemas for receipts, status, and health checks
- **Multi-Provider Support**:
  - **CLI**: Claude CLI, Gemini CLI
  - **HTTP**: OpenRouter, Anthropic API
  - **Resilience**: Robust fallback logic and budget control for HTTP providers
- **Security First**: Automatic secret detection and redaction
- **Cross-Platform**: Linux, macOS, Windows with WSL support

## Installation

```bash
# From crates.io
cargo install xchecker

# From source
git clone https://github.com/EffortlessMetrics/xchecker.git
cd xchecker && cargo install --path .
```

**Requirements**: Rust 1.89+, and a configured LLM provider (e.g. [Claude CLI](https://claude.ai/download), Gemini CLI, or API key).

## Embedding xchecker

xchecker can be embedded as a library in your Rust applications:

```toml
# Cargo.toml
[dependencies]
xchecker = "1"
```

```rust
use xchecker::{OrchestratorHandle, PhaseId, Config};

fn main() -> Result<(), xchecker::XcError> {
    // Option 1: Use environment-based discovery (like CLI)
    let mut handle = OrchestratorHandle::new("my-feature")?;
    
    // Option 2: Use explicit configuration
    let config = Config::builder()
        .state_dir(".xchecker")
        .build()?;
    let mut handle = OrchestratorHandle::from_config("my-feature", config)?;
    
    // Run a single phase
    handle.run_phase(PhaseId::Requirements)?;
    
    // Check status
    let status = handle.status()?;
    println!("Artifacts: {:?}", status.artifacts);
    
    Ok(())
}
```

See the [examples/](examples/) directory for more embedding examples.

## Quick Start

```bash
# Check your environment
xchecker doctor

# Create a spec from stdin
echo "Build a REST API for user management" | xchecker spec my-feature

# Check status
xchecker status my-feature

# Resume from a specific phase
xchecker resume my-feature --phase design

# Apply code changes
xchecker resume my-feature --phase fixup --apply-fixups
```

## Commands

| Command | Description |
|---------|-------------|
| `xchecker spec <id>` | Generate a new spec through the requirements phase |
| `xchecker resume <id> --phase <phase>` | Resume execution from a specific phase |
| `xchecker status <id>` | Display spec status and configuration |
| `xchecker clean <id>` | Clean up spec artifacts and receipts |
| `xchecker doctor` | Run environment health checks |
| `xchecker init <id>` | Initialize a new spec with optional lockfile |
| `xchecker benchmark` | Run performance benchmarks |

### Common Options

```bash
--dry-run              # Preview without making LLM calls
--json                 # Output as JSON
--force                # Override stale locks
--apply-fixups         # Apply file changes (default is preview)
--verbose              # Enable structured logging
```

## Configuration

xchecker uses a hierarchical configuration system: CLI flags > config file > defaults.

```toml
# .xchecker/config.toml
[defaults]
model = "haiku"
phase_timeout = 600

[selectors]
include = ["src/**/*.rs", "docs/**/*.md"]
exclude = ["target/**", ".git/**"]
```

Hooks are opt-in and configured under `[hooks]` for pre/post-phase scripts.

See [Configuration Guide](docs/CONFIGURATION.md) for all options.

## State Directory

xchecker stores all state in a directory determined by:

1. **Thread-local override** (used internally for test isolation)
2. **XCHECKER_HOME environment variable** (user/CI override)
3. **Default: `./.xchecker`** (relative to current working directory)

### Using XCHECKER_HOME

Override the default state directory location using the `XCHECKER_HOME` environment variable:

```bash
# Set for your session
export XCHECKER_HOME=/path/to/custom/state
xchecker spec my-feature

# Or inline for a single command
XCHECKER_HOME=/tmp/xchecker-test xchecker status my-feature

# Useful for CI/CD to isolate builds
XCHECKER_HOME=/tmp/build-${BUILD_ID} xchecker spec feature
```

### Directory Structure

The state directory contains specs/<spec-id>/ directories for each specification, with the following structure:

```
.xchecker/                    # State directory (XCHECKER_HOME)
├── config.toml              # Configuration file (optional)
└── specs/                   # All specs
    └── <spec-id>/          # Individual spec directory
        ├── artifacts/      # Generated artifacts (requirements, design, tasks)
        │   ├── 00-requirements.md
        │   ├── 10-design.md
        │   └── 20-tasks.md
        ├── receipts/       # Execution receipts with metadata
        │   └── <phase>-<timestamp>.json
        └── context/        # Context files sent to Claude
            └── packet-<hash>.txt
```

**Directory purposes:**
- **specs/<spec-id>/artifacts/**: Generated phase outputs (requirements, design, tasks, review, fixup)
- **specs/<spec-id>/receipts/**: Execution audit trails with BLAKE3 hashes and metadata
- **specs/<spec-id>/context/**: Packet previews for debugging (enabled with `--debug-packet`)

## Exit Codes

| Code | Name | Description |
|------|------|-------------|
| 0 | SUCCESS | Completed successfully |
| 7 | PACKET_OVERFLOW | Packet size exceeded |
| 8 | SECRET_DETECTED | Secret found in content |
| 9 | LOCK_HELD | Lock already held |
| 10 | PHASE_TIMEOUT | Phase timed out |
| 70 | CLAUDE_FAILURE | LLM Provider failure |

## Known Limitations & Guarantees

### Guarantees

| What | Guarantee |
|------|-----------|
| **v1 JSON schemas** | Additive-only changes; no breaking changes without version bump |
| **Canonical emission** | JCS (RFC 8785) for reproducible, diff-friendly JSON |
| **Atomic writes** | All artifact writes via staging directory; no partial files |
| **Secret scanning** | Always runs before LLM invocation; blocks on detection |
| **Failure modes** | Structured exit codes; no silent failures or "best effort" modes |
| **State directory layout** | `artifacts/`, `receipts/`, `problem_statement.txt` are stable API |

### Limitations

| Area | Limitation |
|------|------------|
| **LLM provider** | Agnostic; supports Claude CLI, Gemini CLI, OpenRouter, Anthropic API |
| **Execution strategy** | Controlled only; LLMs propose diffs, xchecker applies |
| **Fixup engine** | Context-based fuzzy matching works for contiguous context; fails on ambiguous patterns, large shifts, or context split by deletions |
| **Diff complexity** | Best with small, focused changes; large refactors may fail fuzzy matching |
| **Windows** | Requires WSL or native Claude CLI; some path edge cases |

For fixup limitations and workarounds, see the [Debugging Guide](docs/DEBUGGING_GUIDE.md#fuzzy-matching--what-works-what-doesnt).

## Documentation

| Document | Description |
|----------|-------------|
| [Configuration](docs/CONFIGURATION.md) | Full configuration reference |
| [Testing](docs/TESTING.md) | Test lanes and profiles |
| [Orchestrator](docs/ORCHESTRATOR.md) | Core engine architecture |
| [Contracts](docs/CONTRACTS.md) | JSON schema versioning |
| [Doctor](docs/DOCTOR.md) | Health check details |
| [LLM Providers](docs/LLM_PROVIDERS.md) | Provider configuration |
| [Troubleshooting](docs/TROUBLESHOOTING.md) | Common issues and fixes |

### Walkthroughs

- [20-Minute Quickstart](docs/WALKTHROUGH_20_MINUTES.md) - Get running fast
- [Spec to PR](docs/WALKTHROUGH_SPEC_TO_PR.md) - Complete workflow guide

### Operations

- [Debugging Guide](docs/DEBUGGING_GUIDE.md) - Diagnostics and debugging workflows
- [Workspace Guide](docs/WORKSPACE_GUIDE.md) - Workspace setup and TUI usage
- [CI Profiles](docs/CI_PROFILES.md) - CI gate configuration

## Development

```bash
# Run fast tests (~30s)
cargo test --lib --bins

# Run full suite
cargo test

# Check formatting and lints
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
```

See [Testing Guide](docs/TESTING.md) for test profiles and CI configuration.

## License

MIT OR Apache-2.0

## Contributing

1. All tests pass: `cargo test`
2. Code formatted: `cargo fmt`
3. No clippy warnings: `cargo clippy -- -D warnings`
4. Documentation updated for new features

See [CHANGELOG.md](CHANGELOG.md) for version history.

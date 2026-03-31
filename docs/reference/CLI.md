# CLI Reference

Complete reference for all xchecker commands, options, exit codes, and library embedding.

## Commands

| Command | Description |
|---------|-------------|
| `xchecker spec <id>` | Create a new spec and run the requirements phase. Reads the feature idea from stdin. |
| `xchecker resume <id> --phase <phase>` | Resume execution from a specific phase (design, tasks, review, fixup). |
| `xchecker status <id>` | Display spec status: completed phases, artifacts, and current configuration. |
| `xchecker clean <id>` | Remove all artifacts, receipts, and context files for a spec. |
| `xchecker doctor` | Run environment health checks: LLM availability, config validity, permissions. |
| `xchecker init <id>` | Initialize a new spec directory with optional lockfile (`--create-lock`). |
| `xchecker benchmark` | Run performance benchmarks for packet building and phase execution. |
| `xchecker project init <name>` | Initialize a workspace for managing multiple specs. |
| `xchecker project add-spec <id>` | Add a spec to the current workspace. |
| `xchecker project status` | Show status of all specs in the workspace. |
| `xchecker project history` | Show execution history across all workspace specs. |
| `xchecker project list` | List all specs in the workspace. |
| `xchecker template init <id> --template <name>` | Bootstrap a new spec from a template (e.g., `nextjs`, `rust`, `python`). |
| `xchecker template list` | List available templates. |
| `xchecker gate <id>` | Run policy checks for CI/CD enforcement. |

## Global Options

| Option | Description | Default |
|--------|-------------|---------|
| `--dry-run` | Preview the pipeline without making LLM calls. Packets are built but not sent. | off |
| `--json` | Emit output as JSON (JCS-canonical). Works with `status`, `doctor`, `gate`. | off |
| `--force` | Override stale locks. Use when a previous run crashed and left a lock behind. | off |
| `--apply-fixups` | Apply file changes proposed by the LLM. Without this flag, fixups are previewed only. | off (preview) |
| `--verbose` | Enable structured logging to stderr. | off |
| `--llm-provider <name>` | Override the configured LLM provider. One of: `claude-cli`, `gemini-cli`, `openrouter`, `anthropic`. | from config |
| `--model <name>` | Override the model name passed to the LLM provider. | from config |
| `--phase-timeout <secs>` | Maximum seconds to wait for a single phase to complete. | 600 |
| `--debug-packet` | Write the assembled packet to `context/` before sending it to the LLM. Useful for diagnosing prompt issues. | off |

## Exit Codes

Every command produces a numeric exit code. These are stable and safe to use in scripts and CI pipelines.

| Code | Name | Description | What to do |
|------|------|-------------|------------|
| 0 | `SUCCESS` | Operation completed successfully. | Nothing -- you are done. |
| 7 | `PACKET_OVERFLOW` | The assembled packet exceeded the provider's context limit. | Narrow your file selectors or split the spec into smaller units. |
| 8 | `SECRET_DETECTED` | A secret pattern was found in content destined for the LLM. | Remove the secret from the source files or add the file to your exclude list. |
| 9 | `LOCK_HELD` | Another xchecker process holds the lock for this spec. | Wait for the other process, or use `--force` if it crashed. |
| 10 | `PHASE_TIMEOUT` | A phase exceeded the configured `--phase-timeout`. | Increase the timeout or simplify the spec so the LLM responds faster. |
| 70 | `CLAUDE_FAILURE` | The LLM provider process failed (crash, auth error, network). | Check `xchecker doctor` output and verify your provider credentials. |

Exit codes in receipts always match the process exit code. This is a stable contract.

## Embedding as a Library

xchecker exposes a stable Rust API through the `OrchestratorHandle` facade.

Add the dependency:

```toml
# Cargo.toml
[dependencies]
xchecker = "1"
```

Use it:

```rust
use xchecker::{OrchestratorHandle, PhaseId, Config};

fn main() -> Result<(), xchecker::XcError> {
    // Option 1: Use environment-based discovery (like the CLI does)
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

The golden rule for library consumers: always use `OrchestratorHandle`. The internal `PhaseOrchestrator` type is not part of the public API and may change without notice.

## JSON Output

When `--json` is passed, commands emit JCS-canonical JSON (RFC 8785) to stdout. Schemas are versioned and follow an additive-only evolution policy:

| Schema | File | Used by |
|--------|------|---------|
| Receipt v1 | `schemas/receipt.v1.json` | `xchecker spec`, `xchecker resume` |
| Status v1 | `schemas/status.v1.json` | `xchecker status --json` |
| Doctor v1 | `schemas/doctor.v1.json` | `xchecker doctor --json` |

See [CONTRACTS.md](../CONTRACTS.md) for the full versioning policy.

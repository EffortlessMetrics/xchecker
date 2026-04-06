# xchecker

[![Crates.io](https://img.shields.io/crates/v/xchecker.svg)](https://crates.io/crates/xchecker)
[![License](https://img.shields.io/crates/l/xchecker.svg)](https://github.com/EffortlessMetrics/xchecker#license)

Turn rough feature ideas into structured specs -- requirements, designs, and implementation tasks -- using LLM-powered orchestration.

## See It Work

```bash
# Check your environment is ready
$ xchecker doctor
  LLM provider: claude-cli ... ok
  Config: .xchecker/config.toml ... ok
  Permissions: artifacts/ ... ok

# Feed in a feature idea and generate requirements
$ echo "Build a REST API for user management" | xchecker spec my-api
  Phase: requirements ... done (12.4s)
  Artifact: specs/my-api/artifacts/00-requirements.md

# See where you are
$ xchecker status my-api
  Spec: my-api
  Completed: requirements
  Next: design

# Pick up where you left off
$ xchecker resume my-api --phase design
  Phase: design ... done (18.1s)
  Artifact: specs/my-api/artifacts/10-design.md
```

## What xchecker Does for You

- **Structured thinking, not blank-page paralysis.**
  You describe the idea; xchecker walks it through requirements, design, tasks, review, and fixup -- each phase building on the last.

- **Your secrets never leave your machine.**
  45+ secret patterns (API keys, tokens, credentials) are scanned and blocked before any content reaches an LLM. If a secret is detected, execution stops immediately.

- **Your work is never lost.**
  Every artifact is written atomically through a staging directory. Every execution produces an audit receipt with BLAKE3 hashes. You can resume from any phase at any time.

- **Works with your LLM.**
  Claude CLI, Gemini CLI, OpenRouter, or the Anthropic API. Switch providers with a flag; the pipeline stays the same.

- **CI-ready from day one.**
  Deterministic exit codes, JSON output on every command, and policy gates you can wire into any CI pipeline.

## Key Features

### Multi-Phase Orchestration

xchecker walks each spec through a sequential phase pipeline:

```
Requirements → Design → Tasks → Review → Fixup → Final
```

Each phase builds a context packet, invokes the LLM, and writes artifacts atomically. If a phase exceeds its `phase_timeout` (default 600s), execution stops with: "Phase execution exceeded timeout" and exit code 10.

### State Directory

xchecker stores all state in `.xchecker/` by default. Override the location with the `XCHECKER_HOME` environment variable:

```bash
XCHECKER_HOME=/tmp/build xchecker status my-feature
```

Directory structure:

```
.xchecker/
  specs/<spec-id>/
    artifacts/    # Generated requirements, design, and task documents
    receipts/     # Execution audit trails with BLAKE3 hashes
    context/      # Packet previews and debugging files
```

### Lockfile System

Lock reproducibility metadata (model version, CLI version, schema version) with `--create-lock`, then detect drift with `lock_drift` in status output. Use `--strict-lock` to fail hard on any drift.

### Fixup System

The fixup phase proposes file changes as unified diffs. By default, xchecker runs in Preview Mode — showing pending changes without applying them. Pass `--apply-fixups` to apply validated diffs to your working tree.

### Standardized Exit Codes

Every command sets a deterministic `exit_code` that matches the receipt's `exit_code` field:

| Code | Name | Description |
|------|------|-------------|
| 0 | SUCCESS | Operation completed |
| 2 | CLI_ARGS | Invalid arguments |
| 7 | PACKET_OVERFLOW | Packet size exceeded |
| 8 | SECRET_DETECTED | Secret found in packet |
| 9 | LOCK_HELD | Lock already held |
| 10 | PHASE_TIMEOUT | Phase timed out |
| 70 | CLAUDE_FAILURE | LLM provider failure |

## Install

```bash
cargo install xchecker
xchecker doctor
```

**Requirements:** Rust 1.89+ and a configured LLM provider ([Claude CLI](https://claude.ai/download), Gemini CLI, or an API key for OpenRouter/Anthropic).

To build from source:

```bash
git clone https://github.com/EffortlessMetrics/xchecker.git
cd xchecker && cargo install --path .
```

## Next Steps

| I want to...                        | Go to                                                       |
|-------------------------------------|-------------------------------------------------------------|
| Get running in 20 minutes           | [tutorials/QUICKSTART.md](docs/tutorials/QUICKSTART.md)     |
| Understand the full workflow        | [tutorials/SPEC_TO_PR.md](docs/tutorials/SPEC_TO_PR.md)     |
| Configure my LLM provider          | [guides/LLM_PROVIDERS.md](docs/guides/LLM_PROVIDERS.md)     |
| Set up CI gates                     | [guides/CI_SETUP.md](docs/guides/CI_SETUP.md)               |
| Look up a command or exit code      | [reference/CLI.md](docs/reference/CLI.md)                    |
| Understand how it works             | [explanation/ARCHITECTURE.md](docs/explanation/ARCHITECTURE.md) |
| Embed xchecker as a library        | [reference/CLI.md#embedding](docs/reference/CLI.md#embedding-as-a-library) |
| Contribute                          | [contributor/](docs/contributor/)                            |

## License

MIT OR Apache-2.0

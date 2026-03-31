# Architecture

How xchecker works, from a user's and integrator's perspective.

## The Problem xchecker Solves

Feature ideas start as vague sentences: "we need user management" or "add payment processing." Turning that into something a team can actually build -- requirements, a design, broken-down tasks -- is tedious, inconsistent, and easy to skip. The result is either a blank page or a wall of unstructured text that nobody trusts.

xchecker applies a structured pipeline to that problem. You feed in the rough idea; it walks the idea through six phases with an LLM, producing versioned artifacts at each step. Every execution is auditable (receipts with cryptographic hashes), every artifact is written atomically (no partial files), and secrets are scanned before anything reaches the LLM.

## The Pipeline

```
Input --> Requirements --> Design --> Tasks --> Review --> Fixup --> Final
            (00-*)        (10-*)    (20-*)    (30-*)    (40-*)
```

| Phase | What it produces |
|-------|-----------------|
| **Requirements** | A structured requirements document and core YAML extracted from your feature idea. |
| **Design** | A technical design covering architecture, data model, and key decisions. |
| **Tasks** | An ordered breakdown of implementation tasks with acceptance criteria. |
| **Review** | A critical review of the design and tasks, flagging gaps and risks. |
| **Fixup** | Concrete file-level diffs that implement the reviewed design. |
| **Final** | Applies fixups (when `--apply-fixups` is set) and closes out the spec. |

Each phase reads artifacts from earlier phases as input, so the pipeline builds cumulatively. You can resume from any phase without re-running the ones before it.

## Key Concepts

- **Spec**: A named unit of work. One feature idea becomes one spec, identified by a short ID like `user-api` or `payment-v2`.
- **Phase**: One step in the pipeline. Each phase invokes the LLM once and produces one or more artifacts.
- **Artifact**: A file produced by a phase -- markdown for humans, YAML for machines. Named by convention: `00-requirements.md`, `10-design.core.yaml`, etc.
- **Receipt**: A JSON record of a single phase execution: inputs, outputs, timestamps, model used, and BLAKE3 hashes of all artifacts.
- **Packet**: The assembled context sent to the LLM for a phase -- earlier artifacts, file selectors, prompt template, and budget constraints.
- **Lock**: A file lock preventing two xchecker processes from modifying the same spec simultaneously.

## How xchecker Keeps You Safe

### Secret scanning

Before any content is sent to an LLM, xchecker scans the assembled packet for 45+ secret patterns: API keys, tokens, passwords, private keys, and credentials. If a match is found, execution stops immediately with exit code 8. No content leaves your machine.

### Path sandboxing

All file paths in fixup diffs are validated against the spec root directory. Directory traversal attempts (`../`) and absolute paths outside the sandbox are rejected. This prevents a malicious or confused LLM response from writing to arbitrary locations on your filesystem.

### Atomic writes

Artifacts are never written directly to their final location. Instead, xchecker stages them in a `.partial/` directory and moves them into place only after the write succeeds and the BLAKE3 hash is computed. If a phase crashes mid-write, you get either the previous artifact or nothing -- never a half-written file.

### Audit receipts

Every phase execution produces a JSON receipt containing: the phase name, timestamps, the model and provider used, token counts, input/output hashes, and the exit code. Receipts use JCS (RFC 8785) canonical encoding for reproducible diffs. This gives you a complete, tamper-evident audit trail of how every artifact was produced.

## Execution Model

xchecker uses a single execution strategy: **controlled**. The LLM proposes changes as text -- markdown documents and unified diffs -- and xchecker's fixup engine parses and applies them. The LLM never directly modifies files on your filesystem.

This matters for three reasons:

1. **Safety.** Every proposed change goes through path validation and secret scanning before it touches the filesystem.
2. **Auditability.** The receipt captures exactly what the LLM proposed and what xchecker applied, with cryptographic hashes for both.
3. **Reproducibility.** The same packet sent to the same model produces the same artifacts, and you can verify this by comparing receipt hashes.

## Crate Architecture

xchecker is a Cargo workspace organized into four layers. Each crate may only depend on crates in the same layer or a lower one.

**Foundation** -- Types, paths, sandboxing, atomic writes, logging, redaction, locking, and the process runner.
Crates: `xchecker-utils`, `xchecker-runner`, `xchecker-lock`, `xchecker-redaction`, `xchecker-error-redaction`

**Infrastructure** -- Configuration model, discovery, validation, and LLM provider backends (Claude CLI, Gemini CLI, OpenRouter, Anthropic).
Crates: `xchecker-config`, `xchecker-selectors`, `xchecker-llm`, `xchecker-prompt-template`

**Domain** -- The core pipeline: phases, orchestrator, packet builder, receipt writer, fixup engine, status tracker, workspaces, gates, and hooks.
Crates: `xchecker-engine`, `xchecker-phase-api`, `xchecker-packet`, `xchecker-receipt`, `xchecker-status`, `xchecker-gate`

**Application** -- The CLI binary, TUI, and the stable `OrchestratorHandle` facade re-exported from the root crate.
Crates: `xchecker` (root), `xchecker-tui`, `xchecker-error-reporter`

The golden rule: crates may only depend on crates in the same or lower layers. See [contributor/dependency-policy.md](../contributor/dependency-policy.md) for the full policy.

## State Directory

xchecker stores all state in a single directory:

```
.xchecker/
  config.toml                    # Configuration (optional)
  specs/<spec-id>/
    artifacts/                   # Phase outputs (requirements, design, tasks, ...)
      00-requirements.md
      00-requirements.core.yaml
      10-design.md
      20-tasks.md
      ...
    receipts/                    # Execution audit trails
      requirements-<timestamp>.json
      design-<timestamp>.json
      ...
    context/                     # Packet previews (when --debug-packet is used)
      packet-<hash>.txt
```

By default, this is `.xchecker/` in your current working directory. Override it with the `XCHECKER_HOME` environment variable:

```bash
# Use a custom state directory
export XCHECKER_HOME=/path/to/state
xchecker spec my-feature

# Isolate CI builds
XCHECKER_HOME=/tmp/build-${BUILD_ID} xchecker spec my-feature
```

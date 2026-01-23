# xchecker Roadmap

**Last Updated**: January 23, 2026
**Current Version**: 1.1.0
**Next Major**: 2.0.0 (Modularization)

---

## Overview

xchecker is a Rust CLI tool for orchestrating spec generation workflows using LLMs. It implements a 6-phase pipeline (Requirements → Design → Tasks → Review → Fixup → Final) with controlled execution, atomic file operations, secret redaction, and cryptographic audit trails.

---

## Current Status: v1.1.0

All V11–V18 roadmap features are **complete and released**.

| Milestone | Version | Status |
|-----------|---------|--------|
| Core Runtime | v1.0.0 | ✅ Complete |
| LLM Abstraction (V11) | v1.1.0 | ✅ Complete |
| Gemini Support (V12) | v1.1.0 | ✅ Complete |
| HTTP Providers (V13-V14) | v1.1.0 | ✅ Complete |
| Ecosystem (V16-V18) | v1.1.0 | ✅ Complete |

---

## Spec Implementation Status

Each spec in `.kiro/specs/` represents a distinct development effort:

| Spec | Purpose | Status |
|------|---------|--------|
| [xchecker-runtime-implementation](.kiro/specs/xchecker-runtime-implementation/) | Core phase pipeline, runner, packet, fixup, receipts | ✅ Complete |
| [xchecker-claude-orchestrator](.kiro/specs/xchecker-claude-orchestrator/) | Orchestrator, phase system, LLM integration | ✅ Complete |
| [xchecker-llm-ecosystem](.kiro/specs/xchecker-llm-ecosystem/) | V11-V18 multi-provider LLM support | ✅ Complete |
| [xchecker-operational-polish](.kiro/specs/xchecker-operational-polish/) | Test fixes, warnings cleanup, benchmark, contracts | ✅ Complete |
| [xchecker-final-cleanup](.kiro/specs/xchecker-final-cleanup/) | Test stability, code annotations, hooks | ✅ Complete |
| [crates-io-packaging](.kiro/specs/crates-io-packaging/) | Library API, crates.io packaging, security hardening | ✅ Complete |
| [documentation-validation](.kiro/specs/documentation-validation/) | Schema validation, doc tests, example generators | ✅ Complete |

---

## Features by Release

### v1.0.0 - Core Runtime (December 2025)

- Phase pipeline: Requirements → Design → Tasks → Review → Fixup → Final
- Cross-platform runner (Native/WSL/Auto modes)
- Packet builder with priority-based file selection
- Fixup engine with fuzzy matching and atomic writes
- Secret redaction (GitHub PAT, AWS, Slack, Bearer tokens)
- Receipt system with BLAKE3 hashes and JCS canonicalization
- Lock manager with drift detection
- JSON contracts v1 (receipt, status, doctor schemas)

### v1.1.0 - Multi-Provider LLM (January 2026)

**LLM Providers:**
- Claude CLI backend
- Gemini CLI backend
- OpenRouter HTTP backend (with budget control)
- Anthropic HTTP backend
- Fallback provider logic

**Ecosystem:**
- Workspace management (`project init`, `add-spec`, `status`, `history`, `list`)
- Template system (`template init`, `template list`)
- Gate command for CI/CD policy enforcement
- Hooks system (pre/post-phase)
- Rich metadata in receipts (tokens, model, cost)

---

## Active Work

### v1.2.0 - Polish & Documentation

| Task | Priority | Status |
|------|----------|--------|
| Add `--llm-gemini-binary` CLI flag | High | Open |
| Config test environment isolation | Medium | Open |
| Hooks documentation in CONFIGURATION.md | Medium | Open |
| README update with V11-V14 completeness | Low | Open |

See [plans/xchecker-completion-plan.md](plans/xchecker-completion-plan.md) for details.

### v2.0.0 - Modularization

Major architectural refactoring to improve maintainability, testing, and build performance.

**Summary:**
- Split from 4 crates to 19 crates
- Extract CLI layer (5471 lines) into dedicated crate
- Decompose xchecker-engine into domain-specific crates
- Establish clear dependency hierarchy

**Target Crate Structure:**

| Layer | Crates |
|-------|--------|
| Foundation | xchecker-core |
| Infrastructure | xchecker-config, xchecker-llm, xchecker-runner |
| Domain | xchecker-phases, xchecker-orchestrator, xchecker-workspace, xchecker-fixup, xchecker-status, xchecker-gate, xchecker-templates, xchecker-doctor, xchecker-benchmark, xchecker-hooks, xchecker-validation, xchecker-extraction |
| Application | xchecker-cli |

See [plans/modularization-report.md](plans/modularization-report.md) for the full plan.

---

## Architecture

### Phase Pipeline

```
Input → Requirements → Design → Tasks → Review → Fixup → Final
         (00-*)        (10-*)   (20-*)  (30-*)   (40-*)
```

### LLM Providers

| Provider | Type | Authentication |
|----------|------|----------------|
| claude-cli | CLI | Native |
| gemini-cli | CLI | GEMINI_API_KEY |
| openrouter | HTTP | OPENROUTER_API_KEY |
| anthropic | HTTP | ANTHROPIC_API_KEY |

### Execution Model

All LLM interactions use **controlled execution**:
- LLM proposes changes as text/JSON
- All file modifications go through FixupEngine
- Atomic writes via `.partial/` staging
- No direct file modification by LLM

---

## Documentation

| Document | Purpose |
|----------|---------|
| [README.md](README.md) | Quick start and feature overview |
| [CHANGELOG.md](CHANGELOG.md) | Version history and release notes |
| [docs/CONFIGURATION.md](docs/CONFIGURATION.md) | Full configuration reference |
| [docs/LLM_PROVIDERS.md](docs/LLM_PROVIDERS.md) | Provider setup and configuration |
| [docs/ORCHESTRATOR.md](docs/ORCHESTRATOR.md) | Core engine architecture |
| [docs/CONTRACTS.md](docs/CONTRACTS.md) | JSON schema versioning policy |
| [docs/WORKSPACE_GUIDE.md](docs/WORKSPACE_GUIDE.md) | Workspace and TUI usage |
| [CLAUDE.md](CLAUDE.md) | AI agent guidance for development |

---

## Development Resources

| Resource | Purpose |
|----------|---------|
| [.kiro/](.kiro/) | Spec development audit trail |
| [.kiro/steering/](.kiro/steering/) | Product, structure, and tech guidelines |
| [plans/](plans/) | Modularization and completion plans |
| [schemas/](schemas/) | JSON schema definitions |

---

## Quick Commands

```bash
# Build and test
cargo build
cargo test --workspace --lib

# Run with different providers
xchecker spec my-feature --llm-provider claude-cli
xchecker spec my-feature --llm-provider gemini-cli
xchecker spec my-feature --llm-provider openrouter

# Health check
xchecker doctor --json

# Workspace management
xchecker project init my-repo
xchecker project status --json

# Policy gate (CI/CD)
xchecker gate my-spec --min-phase tasks --fail-on-pending-fixups
```

---

## Contributing

1. Check [plans/xchecker-completion-plan.md](plans/xchecker-completion-plan.md) for open tasks
2. Review [CLAUDE.md](CLAUDE.md) for codebase guidance
3. Follow the test profiles in [docs/TESTING.md](docs/TESTING.md)
4. Run `cargo test --workspace --lib` before submitting PRs

---

**Status**: Active development - v1.1.0 released, v2.0.0 modularization in planning

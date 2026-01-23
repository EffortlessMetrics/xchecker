# V11–V18 Implementation Record

> **Note**: This document is the historical record for the V11–V18 roadmap.
> For the current project roadmap, see **[ROADMAP.md](../ROADMAP.md)** at the repo root.

**Last Updated**: January 23, 2026
**Implemented In**: v1.1.0

---

## Summary

The V11–V18 roadmap for multi-provider LLM support and ecosystem expansion is **complete**. All planned features have been implemented and are available in v1.1.0.

## Completed Milestones

### v1.0.0 - Core Runtime (December 2025)

| Feature | Status |
|---------|--------|
| Phase Pipeline (Requirements → Design → Tasks → Review → Fixup) | ✅ |
| JCS Canonicalization (RFC 8785) | ✅ |
| Secret Redaction | ✅ |
| Cross-platform Runner (Native/WSL/Auto) | ✅ |
| Packet Builder with budget enforcement | ✅ |
| Fixup Engine with fuzzy matching | ✅ |
| Lock Manager with drift detection | ✅ |
| Receipt System (BLAKE3 hashes) | ✅ |
| JSON Contracts v1 (receipt, status, doctor schemas) | ✅ |

### v1.1.0 - Multi-Provider LLM Support (January 2026)

| Feature | Status |
|---------|--------|
| **V11: LLM Abstraction** | |
| - LlmBackend trait | ✅ |
| - ExecutionStrategy (controlled mode) | ✅ |
| - Claude CLI backend | ✅ |
| **V12: Gemini Support** | |
| - Gemini CLI backend | ✅ |
| - Provider selection | ✅ |
| - Fallback provider logic | ✅ |
| **V13: HTTP Providers** | |
| - HTTP client | ✅ |
| - OpenRouter backend | ✅ |
| - Budget enforcement | ✅ |
| **V14: Anthropic Direct** | |
| - Anthropic HTTP backend | ✅ |
| - Rich metadata in receipts | ✅ |
| **V16-V18: Ecosystem** | |
| - Workspace management | ✅ |
| - Template system | ✅ |
| - Gate command (CI/CD) | ✅ |
| - Hooks system | ✅ |

---

## Active Work

### v1.2.0 - Polish & Documentation

| Task | Priority | Status |
|------|----------|--------|
| Add `--llm-gemini-binary` CLI flag | High | Open |
| Config test environment isolation | Medium | Open |
| Hooks documentation in CONFIGURATION.md | Medium | Open |
| README update with V11-V14 completeness | Low | Open |

### v2.0.0 - Modularization

A major architectural refactoring to improve maintainability and enable independent crate development.

See **[plans/modularization-report.md](../plans/modularization-report.md)** for full details.

**Summary:**
- Split into 19 crates from current 4
- Extract CLI layer (5471 lines) into dedicated crate
- Decompose xchecker-engine into domain-specific crates
- Establish clear dependency hierarchy

**Proposed Crates:**
| Crate | Purpose |
|-------|---------|
| xchecker-core | Foundation utilities |
| xchecker-runner | Process execution |
| xchecker-phases | Phase execution |
| xchecker-orchestrator | Workflow orchestration |
| xchecker-workspace | Multi-spec management |
| xchecker-fixup | Fixup detection/application |
| xchecker-status | Status queries |
| xchecker-gate | CI/CD gate enforcement |
| xchecker-templates | Template management |
| xchecker-doctor | Health diagnostics |
| xchecker-benchmark | Performance benchmarking |
| xchecker-hooks | Hook system |
| xchecker-validation | Validation logic |
| xchecker-extraction | Content extraction |
| xchecker-cli | CLI, TUI, error reporting |

---

## Design Principles

### Controlled Execution
- All LLM outputs go through FixupEngine + atomic write pipeline
- No direct file modification by LLM
- ExternalTool/agentic mode reserved for future versions

### Provider Abstraction
- Single `LlmBackend` trait hides transport details
- Orchestrator agnostic to CLI vs HTTP
- Easy to add new providers without changing orchestrator

### Cost Control
- OpenRouter: default budget 20 calls, configurable via `budget` or `XCHECKER_OPENROUTER_BUDGET`
- Exit code 70 if budget exceeded
- Doctor checks don't send HTTP requests (opt-in only)

### Compression for AI Agents
- Receipts and status JSON enable agents to work with minimal context
- Compact JSON shapes for spec, status, resume commands
- JCS canonical emission for reproducibility

---

## Reference Documentation

| Document | Purpose |
|----------|---------|
| [CHANGELOG.md](../CHANGELOG.md) | Version history and release notes |
| [plans/modularization-report.md](../plans/modularization-report.md) | v2.0.0 architecture plans |
| [plans/xchecker-completion-plan.md](../plans/xchecker-completion-plan.md) | Outstanding tasks and blockers |
| [docs/LLM_PROVIDERS.md](../docs/LLM_PROVIDERS.md) | Provider configuration guide |
| [docs/CONFIGURATION.md](../docs/CONFIGURATION.md) | Full config reference |
| [docs/ORCHESTRATOR.md](../docs/ORCHESTRATOR.md) | Core engine architecture |

---

## Historical Context

The V11–V18 roadmap was originally planned as an 8-phase, 15–23 week effort spanning multi-provider LLM support and ecosystem features. Implementation was completed ahead of schedule with all planned features shipping in v1.1.0.

**Original Timeline (November 2025):**
- V11-V14: LLM backend abstraction and providers
- V15: Claude Code integration (compressed JSON shapes)
- V16: Workspace orchestration
- V17: Policy gates
- V18: Templates and hooks

**Actual Delivery:**
- v1.0.0 (December 2025): Core runtime
- v1.1.0 (January 2026): All V11-V18 features

---

**Status**: Active roadmap - v1.1.0 released, v2.0.0 modularization in planning

# V11–V18 Roadmap Integration Summary

**Date**: November 30, 2025  
**Status**: ✅ Roadmap integrated into existing spec

## Overview

The comprehensive V11–V18 roadmap for multi-provider LLM support and ecosystem expansion has been successfully integrated into the existing xchecker runtime implementation spec.

## What Was Updated

### 1. Requirements Document (`.kiro/specs/xchecker-runtime-implementation/requirements.md`)

**Added:**
- Updated Introduction to reflect V11–V18 roadmap phases
- 20 new requirements (Requirements 25–44) covering:
  - **V11**: ExecutionStrategy, LlmBackend abstraction, Claude CLI backend
  - **V12**: Gemini CLI as first-class provider
  - **V13**: HTTP client and OpenRouter backend
  - **V14**: Anthropic HTTP backend and rich metadata
  - **V15**: Claude Code (Claude Code) integration
  - **V16**: Workspace and multi-spec orchestration
  - **V17**: Policy and enforcement gates
  - **V18**: Ecosystem, templates, and plugin hooks

**Format:**
- Each requirement follows EARS pattern (WHEN/THEN/SHALL)
- INCOSE quality rules applied (active voice, no vague terms, measurable criteria)
- User stories included for context
- Acceptance criteria clearly defined

### 2. Design Document (`.kiro/specs/xchecker-runtime-implementation/design.md`)

**Added:**
- New section: "V11–V18 Roadmap: Multi-Provider LLM & Ecosystem Expansion"
- Detailed architecture for each phase:
  - Component diagrams (ASCII)
  - Code structure examples
  - Configuration examples
  - CLI command examples
  - JSON output shapes

**Phases Documented:**
- **V11**: LlmBackend trait, ExecutionStrategy, ClaudeCliBackend
- **V12**: GeminiCliBackend, provider selection, fallback logic
- **V13**: HttpClient, OpenRouterBackend, cost control
- **V14**: AnthropicBackend, rich metadata, documentation
- **V15**: Claude Code integration, JSON shapes, slash commands
- **V16**: Workspace registry, project commands, history, TUI
- **V17**: Gate command, CI templates, policy as code
- **V18**: Templates, hooks, showcase examples

**Additional Content:**
- Implementation roadmap summary table (Phase, Goal, Key Deliverables, Timeline)
- Risks & mitigations for each phase
- Total estimated effort: 15–23 weeks (3–5 months)

### 3. Tasks Document (`.kiro/specs/xchecker-runtime-implementation/tasks.md`)

**Added:**
- 60+ implementation tasks organized by phase (V11–V18)
- Each task includes:
  - Clear objective
  - Sub-bullets with specific implementation details
  - Requirements references
  - Optional testing sub-tasks (marked with `*`)

**Task Organization:**
- **V11**: 6 core tasks + 2 optional test tasks
- **V12**: 3 core tasks + 1 optional test task
- **V13**: 3 core tasks + 1 optional test task
- **V14**: 3 core tasks + 1 optional test task
- **V15**: 3 core tasks + 1 optional test task
- **V16**: 4 core tasks + 1 optional test task
- **V17**: 3 core tasks + 1 optional test task
- **V18**: 3 core tasks + 1 optional test task

**Testing Strategy:**
- Property-based tests for each backend (marked with `*`)
- Integration tests for each phase (marked with `*`)
- Optional tests can be skipped for MVP, included for comprehensive coverage

## Key Design Decisions

### 1. Walking Skeleton Approach
Each phase (V11–V18) delivers a complete, working slice:
- V11: Single CLI backend (Claude) behind abstraction
- V12: Swap in Gemini CLI as primary
- V13: Add HTTP path (OpenRouter)
- V14: Add Anthropic HTTP
- V15: Claude Code integration
- V16–V18: Ecosystem expansion

### 2. Controlled Execution Only
- All LLM outputs go through FixupEngine + atomic write pipeline
- No direct file modification by LLM
- ExternalTool strategy stubbed but unsupported in V11–V14

### 3. Provider Abstraction
- Single `LlmBackend` trait hides transport details
- Orchestrator agnostic to CLI vs HTTP
- Easy to add new providers without changing orchestrator

### 4. Cost Control
- OpenRouter: default budget 20 calls, overridable via env var
- Exit code 70 if budget exceeded
- Doctor checks don't send HTTP requests (opt-in only)

### 5. Compression for Claude Code
- Receipts and status JSON enable agents to work with tiny contexts
- Compact JSON shapes for spec, status, resume commands
- No need to parse raw repo state

## Implementation Priorities

### Release 1.0 (V11 MVP)
**Blocking:**
- ExecutionStrategy layer
- LlmBackend abstraction + factory
- ClaudeCliBackend (wrap existing Runner)
- LLM metadata in receipts
- Config parsing for provider selection
- Test gating helper

**Optional:**
- Property-based tests
- Integration tests

### Release 1.1 (V12)
- GeminiCliBackend
- Provider selection with fallback
- Doctor checks for both providers

### Release 1.2 (V13–V14)
- HTTP client
- OpenRouter backend
- Anthropic backend
- Rich metadata
- Comprehensive documentation

### Release 2.0 (V15–V18)
- Claude Code integration
- Workspace orchestration
- Policy gates
- Ecosystem (templates, hooks, examples)

## File Locations

- **Requirements**: `.kiro/specs/xchecker-runtime-implementation/requirements.md` (Requirements 25–44)
- **Design**: `.kiro/specs/xchecker-runtime-implementation/design.md` (V11–V18 Roadmap section)
- **Tasks**: `.kiro/specs/xchecker-runtime-implementation/tasks.md` (V11–V18 Implementation section)

## Next Steps

1. **Review & Approve**: Review the integrated roadmap and approve if it aligns with vision
2. **Prioritize**: Decide which phases to tackle first (recommend V11 for MVP)
3. **Sequence**: Break V11 into GitHub issues or sprint tasks
4. **Execute**: Start with V11 skeleton (2–3 weeks)

## Estimated Timeline

| Phase | Duration | Cumulative |
|-------|----------|-----------|
| V11 | 2–3 weeks | 2–3 weeks |
| V12 | 1–2 weeks | 3–5 weeks |
| V13 | 2–3 weeks | 5–8 weeks |
| V14 | 1–2 weeks | 6–10 weeks |
| V15 | 2–3 weeks | 8–13 weeks |
| V16 | 3–4 weeks | 11–17 weeks |
| V17 | 2–3 weeks | 13–20 weeks |
| V18 | 2–3 weeks | 15–23 weeks |

**Total**: 15–23 weeks (3–5 months)

## Key Metrics

- **Requirements**: 44 total (24 original + 20 new)
- **Tasks**: 60+ implementation tasks
- **Optional Tests**: 16 property-based + integration test tasks
- **Documentation**: 8 phases with detailed architecture
- **Code Modules**: 10+ new modules (llm/*, workspace, gate, templates, hooks)

## Alignment with Original Vision

✅ **Controlled Execution**: All writes through FixupEngine + atomic pipeline  
✅ **Multi-Provider**: Abstraction supports CLI and HTTP backends  
✅ **Cost Control**: Budget enforcement for HTTP providers  
✅ **Compression**: Receipts/status JSON for Claude Code integration  
✅ **Ecosystem**: Templates, hooks, showcase examples  
✅ **Double-Entry SDLC**: Policy gates for CI integration  

---

**Status**: Ready for review and prioritization

# xchecker-dev Route-to-Completion Plan

## Executive Summary

**Project**: xchecker-dev  
**Version**: 1.0.0  
**Purpose**: CLI tool for orchestrating spec generation workflows with Claude AI  
**Current State**: Core implementation complete; V11-V14 LLM backend support implemented; V15-V18 ecosystem features implemented but not fully wired

---

## Project Overview

xchecker is a Rust-based CLI tool that orchestrates spec generation workflows using AI language models. It implements a 6-phase pipeline (Requirements → Design → Tasks → Review → Fixup → Final) with controlled execution, atomic file operations, secret redaction, and cryptographic audit trails.

### Key Architectural Components

1. **LLM Backend Layer** - Trait-based abstraction supporting multiple providers (claude-cli, gemini-cli, openrouter, anthropic)
2. **Phase System** - Trait-based phase implementations with dependency enforcement
3. **Orchestrator** - Manages phase execution, transitions, and state
4. **Runner System** - Cross-platform process execution (Native, WSL, Auto modes)
5. **Packet Builder** - Deterministic context assembly with budget enforcement
6. **Fixup Engine** - Safe file modification with fuzzy matching and atomic writes
7. **Hooks System** - Pre/post-phase hook execution (opt-in, runs from invocation CWD)
8. **Workspace System** - Multi-spec orchestration with registry
9. **Template System** - Built-in templates for quick spec bootstrapping
10. **Gate System** - CI/CD policy enforcement

---

## Current Implementation Status

### ✅ Fully Implemented

| Feature | Status | Evidence |
|---------|--------|----------|
| **Core Runtime (V1-V10)** | | |
| - JCS Canonicalization | ✅ Complete | `src/canonicalization.rs` |
| - Secret Redaction | ✅ Complete | `src/redaction.rs` |
| - Runner System | ✅ Complete | `src/runner.rs` with timeout, NDJSON, buffers |
| - Orchestrator | ✅ Complete | `src/orchestrator/mod.rs`, `phase_exec.rs` |
| - Packet Builder | ✅ Complete | `src/packet.rs` |
| - Fixup Engine | ✅ Complete | `src/fixup.rs` with fuzzy matching |
| - Lock Manager | ✅ Complete | `src/lock.rs` with drift detection |
| - Artifact Manager | ✅ Complete | `src/artifact.rs` |
| - Receipt Manager | ✅ Complete | `src/receipt.rs` with JCS emission |
| - Status Manager | ✅ Complete | `src/status.rs` |
| - Config System | ✅ Complete | `src/config.rs` with discovery |
| - Phase System | ✅ Complete | `src/phase.rs`, `phases.rs` |
| - Exit Codes | ✅ Complete | `src/exit_codes.rs` |
| **LLM Backend (V11-V14)** | | |
| - LlmBackend Trait | ✅ Complete | `src/llm/mod.rs` |
| - Claude CLI Backend | ✅ Complete | `src/llm/claude_cli.rs` |
| - Gemini CLI Backend | ✅ Complete | `src/llm/gemini_cli.rs` |
| - OpenRouter Backend | ✅ Complete | `src/llm/openrouter_backend.rs` |
| - Anthropic Backend | ✅ Complete | `src/llm/anthropic_backend.rs` |
| - HTTP Client | ✅ Complete | `src/llm/http_client.rs` |
| - Budgeted Backend | ✅ Complete | `src/llm/budgeted_backend.rs` |
| - LLM Factory | ✅ Complete | `src/llm/mod.rs` `from_config()` |
| **Ecosystem (V16-V18)** | | |
| - Workspace | ✅ Complete | `src/workspace.rs` |
| - Templates | ✅ Complete | `src/template.rs` |
| - Gate Command | ✅ Complete | `src/gate.rs` |
| - Hooks System | ✅ Complete | `src/hooks.rs` (full implementation) |
| **Documentation** | | |
| - LLM Providers Guide | ✅ Complete | `docs/LLM_PROVIDERS.md` |
| - Configuration Guide | ✅ Complete | `docs/CONFIGURATION.md` |
| - Schema Definitions | ✅ Complete | `schemas/receipt.v1.json`, `status.v1.json`, `doctor.v1.json` |

### ⚠️ Partially Implemented / Not Wired

| Feature | Status | Evidence | Notes |
|---------|--------|----------|-------|
| **ExecutionStrategy** | ⚠️ Controlled only | ExternalTool reserved for V15+ |
| **Fallback Provider** | ⚠️ Config parsing supports it | `src/llm/mod.rs` has `fallback_provider: None` comment |
| **Prompt Templates** | ⚠️ Config parsing supports it | `src/llm/mod.rs` has `prompt_template: None` comment |
| **Gemini CLI Flags** | ⚠️ Config parsing incomplete | `src/cli.rs` has `llm_gemini_binary: None` TODO |
| **TODO Items Found** | | |
| - Claude CLI version extraction | `src/orchestrator/llm.rs:219` - "TODO: Extract from extensions if available" |
| - Runner distro extraction | `src/orchestrator/llm.rs:221` - "TODO: Extract from extensions if available" |
| - Config test isolation issues | `src/config.rs:1864, 1962, 2008, 2036` - Tests marked with environment isolation issues |

### ✅ Recently Completed

| Feature | Status | Notes |
|---------|--------|-------|
| **Hooks Integration** | ✅ Wired | `src/orchestrator/phase_exec.rs` executes pre/post-phase hooks; CWD is invocation directory |
| **Windows Hardlink Detection** | ✅ Complete | `src/paths.rs` `link_count()` uses `GetFileInformationByHandle` |

---

## Critical Blockers and Dependencies

### Blockers

1. **ExecutionStrategy Limited to Controlled**
   - Impact: ExternalTool/agentic mode reserved for V15+
   - Resolution: This is intentional for V11-V14; no action needed

2. **Fallback Provider Not Active**
   - Impact: Config supports `fallback_provider` but factory doesn't use it
   - Resolution: Implement fallback logic in `LlmBackend::from_config()` or document as unsupported

3. **Prompt Templates Not Active**
   - Impact: Config supports `prompt_template` but not used
   - Resolution: Implement template selection in backend factory or document as unsupported

4. **Gemini CLI Binary Flag Missing**
   - Impact: No CLI flag for `--llm-gemini-binary`
   - Resolution: Add flag to `src/cli.rs` and wire through config

5. **Environment Isolation in Config Tests**
   - Impact: Some tests have environment isolation issues
   - Resolution: Fix tests or document as expected behavior

### Dependencies

- **No external dependencies** - All work is internal to the codebase
- **Test framework** - Uses built-in Rust testing tools

---

## Prioritized Completion Plan

### Phase 1: High Priority (Immediate)

1. **Config Test Stability**
   - **Status**: Open
   - **Guidance**: Fix environment isolation issues in `crates/xchecker-config/src/config/mod.rs` (marked with TODOs) to ensure test stability across all platforms.
   - *Note*: Metadata extraction TODOs in `src/orchestrator/llm.rs` are already resolved.

2. **Add Gemini CLI Binary Flag**
   - **Status**: Open / Ready to Implement
   - **Guidance**: Add `--llm-gemini-binary` flag to `Cli` struct in `src/cli.rs` and wire it into `CliArgs` construction. This is required for full V14 feature parity.

3. **Fallback Provider Support**
   - **Status**: ✅ Complete
   - **Notes**: Implemented in `crates/xchecker-llm/src/lib.rs`. The factory correctly handles construction errors and initialization of fallback providers.

### Phase 2: Documentation (Medium Priority)

1. **Update LLM Providers Documentation**
   - **Status**: ✅ Complete
   - **Notes**: `docs/LLM_PROVIDERS.md` is current and accurately reflects V14 support levels for all 4 providers.

2. **Create Hooks Integration Guide**
   - **Status**: Open / Missing Documentation
   - **Guidance**: Update `docs/CONFIGURATION.md` to document the `[hooks]` configuration section. Users currently have no reference for configuring `pre-phase` and `post-phase` hooks despite the feature being implemented.

3. **Update README with Current State**
   - **Status**: Open
   - **Guidance**: Clarify V11-V14 completeness (backend support), document hooks as valid/opt-in, and note that `controlled` is the only supported execution strategy.

### Phase 3: Enhanced LLM Backend (Low Priority)

1. **Implement Prompt Template Support**
   - **Status**: Pending
   - **Guidance**: Add template parsing to config and pass to backend factory. This is a V15+ enabler feature.

2. **Implement Per-Phase Model Selection**
   - **Status**: Pending
   - **Guidance**: Add config support for `model_requirements`, `model_design` overrides and wire into `LlmInvocation` metadata.

### Phase 4: Test Coverage (Low Priority)

1. **Add LLM Integration Tests**
   - Create tests for each LLM backend (claude-cli, gemini-cli, openrouter, anthropic)
   - Add tests for prompt template system

2. **Add Hooks Integration Tests**
   - Create tests for pre/post-phase hook execution
   - Test hook failure behavior (warn vs fail)

### Phase 5: Documentation Polish (Low Priority)

1. **Create Architecture Documentation**
   - Document LLM backend architecture with Mermaid diagrams
   - Document phase execution flow

2. **Create Migration Guide**
   - Document upgrade path from v1.0 to v1.1


---

## Risk Assessment

| Risk | Severity | Mitigation |
|-------|----------|----------|
| Hooks wiring changes orchestrator core | Medium | Comprehensive testing required |
| Windows hardlink detection | Low | Document as known limitation |
| Fallback provider logic | Medium | Requires careful testing of error paths |
| Prompt templates | Low | Breaking changes if not careful |

---

## System Architecture Diagram

```mermaid
graph TD
    A[CLI Entry] --> B[Phase Orchestrator]
    B --> C[LLM Backend Layer]
    B --> D[Phase System]
    B --> E[Runner System]
    B --> F[Fixup Engine]
    B --> G[Hooks System]
    B --> H[Workspace System]
    B --> I[Template System]
    B --> J[Gate System]
    
    C --> C1[Claude CLI Backend]
    C --> C2[Gemini CLI Backend]
    C --> C3[OpenRouter Backend]
    C --> C4[Anthropic Backend]
    
    D --> D1[Requirements Phase]
    D --> D2[Design Phase]
    D --> D3[Tasks Phase]
    D --> D4[Review Phase]
    D --> D5[Fixup Phase]
    
    G -. H1[Hooks Executor]
    G -. H2[Config System]
    
    style C1 fill:#ff6b6b
    style C2 fill:#ff6b6b
    style C3 fill:#ff6b6b
    style C4 fill:#ff6b6b
    
    style G stroke:#ff9999,stroke-width:2px,stroke-dasharray:5 5
    
    class H1 implemented
    class G wired
```

---

## Next Steps

1. Review and approve this plan
2. Switch to Code mode to implement Phase 1 tasks
3. Complete Phase 2-5 tasks as time permits
4. Monitor CI/CD for integration testing

---

## Notes

- The project is in excellent shape with comprehensive core implementation
- Main gaps are integration-related (hooks wiring, fallback, templates)
- All V11-V14 LLM backend features appear to be implemented
- Documentation needs updates to reflect current state
- No critical architectural issues identified
- Test coverage appears comprehensive based on test file structure

# V11–V18 Roadmap Quick Reference

## Phase Overview

```
V11: LLM Core Skeleton & Claude Backend (MVP+)
├─ ExecutionStrategy (Controlled-only)
├─ LlmBackend trait + factory
├─ ClaudeCliBackend (wraps existing Runner)
├─ LLM metadata in receipts
├─ Config parsing
└─ Test gating helper

V12: Gemini CLI as First-Class Provider
├─ GeminiCliBackend
├─ Provider selection with fallback
├─ Doctor checks
└─ Smoke tests + docs

V13: HTTP Client & OpenRouter Backend (Optional)
├─ HttpClient (reqwest)
├─ OpenRouterBackend
├─ Cost control (budget enforcement)
└─ Doctor checks (no HTTP requests)

V14: Anthropic HTTP, Rich Metadata & Provider Docs
├─ AnthropicBackend
├─ Rich metadata (tokens, provider, model)
├─ docs/LLM_PROVIDERS.md
└─ Comprehensive documentation

V15: Claude Code (Claude Code) Integration & UX
├─ xchecker spec --json
├─ xchecker status --json
├─ xchecker resume --phase <phase> --json
└─ Example Claude Code flows

V16: Workspace & Multi-Spec Orchestration
├─ xchecker project init <name>
├─ xchecker project add-spec <spec-id> --tag <tag>
├─ xchecker project list
├─ xchecker project status [--json]
├─ xchecker project history <spec-id> [--json]
└─ xchecker project tui (optional)

V17: Policy & Enforcement ("Double-Entry SDLC" in CI)
├─ xchecker gate <spec-id> [--policy <path>]
├─ Policy options (--min-phase, --fail-on-pending-fixups, --max-phase-age)
├─ GitHub/GitLab CI templates
└─ Policy as code (future)

V18: Ecosystem & Templates (Batteries Included)
├─ xchecker template list
├─ xchecker template init <template> <spec-id>
├─ Plugin hooks (pre-phase, post-phase)
├─ Showcase examples (fullstack-nextjs, mono-repo)
└─ Walkthroughs and documentation
```

## Key Concepts

### ExecutionStrategy
- **Controlled** (V11–V18): LLM proposes text/JSON; all writes through FixupEngine + atomic pipeline
- **ExternalTool** (future): Stubbed; returns "not yet supported"

### LlmBackend Trait
```rust
pub trait LlmBackend: Send + Sync {
    async fn invoke(&self, inv: LlmInvocation<'_>) -> Result<LlmResult, RunnerError>;
}
```

### Supported Providers
| Provider | Type | V | Cost | Auth |
|----------|------|---|------|------|
| Claude CLI | CLI | 11 | Free (licensed) | Native |
| Gemini CLI | CLI | 12 | Free (quota) | GEMINI_API_KEY |
| OpenRouter | HTTP | 13 | Pay-per-use | OPENROUTER_API_KEY |
| Anthropic | HTTP | 14 | Pay-per-use | ANTHROPIC_API_KEY |

### Configuration Hierarchy
```
CLI flags (highest priority)
  ↓
Config file (.xchecker/config.toml)
  ↓
Environment variables
  ↓
Built-in defaults (lowest priority)
```

## Implementation Strategy

### Walking Skeleton Approach
1. **V11**: Single CLI backend (Claude) behind abstraction
2. **V12**: Swap in Gemini CLI as primary
3. **V13**: Add HTTP path (OpenRouter)
4. **V14**: Add Anthropic HTTP
5. **V15**: Claude Code integration
6. **V16–V18**: Ecosystem expansion

### Release Milestones
- **1.0**: V11 (LLM core skeleton)
- **1.1**: V12 (Gemini CLI)
- **1.2**: V13–V14 (HTTP providers)
- **2.0**: V15–V18 (Claude Code + ecosystem)

## File Structure

```
src/
├── llm/
│   ├── mod.rs                 # LlmBackend trait, LlmInvocation, LlmResult
│   ├── factory.rs             # LlmBackendFactory
│   ├── claude_cli.rs          # ClaudeCliBackend (V11)
│   ├── gemini_cli.rs          # GeminiCliBackend (V12)
│   ├── http_client.rs         # HttpClient (V13)
│   ├── openrouter.rs          # OpenRouterBackend (V13)
│   └── anthropic.rs           # AnthropicBackend (V14)
├── workspace.rs               # Workspace registry (V16)
├── gate.rs                    # Gate command (V17)
├── templates.rs               # Template system (V18)
└── hooks.rs                   # Plugin hooks (V18)

docs/
├── LLM_PROVIDERS.md           # Multi-provider documentation (V14)
├── WORKSPACE.md               # Workspace commands (V16)
├── GATE.md                    # Policy gates (V17)
└── ECOSYSTEM.md               # Templates, hooks, examples (V18)

examples/
├── fullstack-nextjs/          # Example project (V18)
└── mono-repo/                 # Multi-spec example (V18)
```

## Configuration Examples

### V11: Claude CLI (Default)
```toml
[llm]
provider = "claude-cli"
execution_strategy = "controlled"

[llm.claude]
binary = "/usr/local/bin/claude"
default_model = "claude-3-5-sonnet"
```

### V12: Gemini CLI (Primary)
```toml
[llm]
provider = "gemini-cli"
fallback_provider = "claude-cli"
execution_strategy = "controlled"

[llm.gemini]
binary = "/usr/local/bin/gemini"
default_model = "gemini-2.0-flash-lite"
model_design = "gemini-2.0-flash"  # per-phase override

[llm.claude]
binary = "/usr/local/bin/claude"
default_model = "claude-3-5-sonnet"
```

### V13: OpenRouter (Optional)
```toml
[llm]
provider = "openrouter"

[llm.openrouter]
base_url = "https://openrouter.ai/api/v1/chat/completions"
api_key_env = "OPENROUTER_API_KEY"
model = "google/gemini-2.0-flash-lite"
max_tokens = 4096
temperature = 0.7
call_budget = 20  # override via XCHECKER_OPENROUTER_BUDGET
```

### V14: Anthropic (Optional)
```toml
[llm]
provider = "anthropic"

[llm.anthropic]
base_url = "https://api.anthropic.com/v1/messages"
api_key_env = "ANTHROPIC_API_KEY"
model = "claude-3-5-sonnet-20241022"
max_tokens = 4096
temperature = 0.7
```

## CLI Commands

### V11–V14: LLM Backend
```bash
xchecker spec <spec-id> --llm-provider gemini-cli --llm-model gemini-2.0-flash
xchecker doctor --json  # Check LLM provider availability
```

### V15: Claude Code Integration
```bash
xchecker spec <spec-id> --json
xchecker status <spec-id> --json
xchecker resume <spec-id> --phase design --json
```

### V16: Workspace
```bash
xchecker project init my-repo
xchecker project add-spec billing-api --tag api --tag critical
xchecker project list
xchecker project status --json
xchecker project history billing-api --json
xchecker project tui
```

### V17: Policy Gates
```bash
xchecker gate billing-api --min-phase tasks --fail-on-pending-fixups
xchecker gate billing-api --policy .xchecker/policy.toml --json
```

### V18: Ecosystem
```bash
xchecker template list
xchecker template init fullstack-nextjs my-feature
```

## Testing Strategy

### Unit Tests
- LlmBackend trait implementations
- Provider-specific logic (CLI invocation, HTTP requests)
- Configuration parsing
- Error mapping

### Property-Based Tests
- For any valid LlmInvocation, backend returns LlmResult with provider set
- For any timeout duration, backend respects timeout
- For any call budget, backend tracks calls and exits if exceeded

### Integration Tests
- End-to-end: source → phase → packet → LLM → receipt
- Provider fallback: primary unavailable → fallback used
- Error scenarios: timeout, invalid JSON, API errors
- Cross-platform: Linux, macOS, Windows, WSL

### Gating
- `XCHECKER_SKIP_LLM_TESTS=1` to skip tests that touch real LLM
- `XCHECKER_USE_OPENROUTER=1` to enable OpenRouter tests (requires API key)
- Doctor never calls LLM (only checks binary presence)

## Estimated Effort

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

## Success Criteria

✅ All LLM outputs go through FixupEngine + atomic pipeline (Controlled execution)  
✅ Single LlmBackend trait abstracts CLI and HTTP providers  
✅ Provider selection via config with fallback support  
✅ Cost control for HTTP providers (budget enforcement)  
✅ Receipts include provider, model, tokens, timeout metadata  
✅ Claude Code can orchestrate xchecker via JSON shapes  
✅ Workspace view shows multi-spec status and history  
✅ Policy gates enforce spec completeness in CI  
✅ Templates and hooks enable ecosystem integration  
✅ Comprehensive documentation for all providers  

---

**Status**: Ready for implementation  
**Next Step**: Start with V11 (2–3 weeks)

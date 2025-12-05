# xchecker LLM & Ecosystem Design (V11–V18)

## Overview

This design document describes the architecture and implementation approach for xchecker's multi-provider LLM support and ecosystem expansion (V11–V18). The design builds on the stable runtime core (V1–V10) and introduces:

- **LLM Backend Abstraction**: A trait-based system for invoking language models via CLI or HTTP
- **Multi-Provider Support**: Claude CLI, Gemini CLI, OpenRouter, and Anthropic backends
- **Execution Strategy**: Controlled mode where LLMs propose changes through xchecker's fixup pipeline
- **Metadata & Telemetry**: Rich LLM usage data in receipts with budget controls
- **Ecosystem Features**: Claude Code integration, workspace management, CI gates, templates, and hooks

The design prioritizes:
- **Safety**: All writes go through the existing fixup/atomic write pipeline
- **Flexibility**: Provider-agnostic orchestration with clean error boundaries
- **Observability**: Comprehensive metadata in receipts for debugging and cost tracking
- **Extensibility**: Clear extension points for new providers and execution modes

## Architecture

### High-Level Structure

```
┌─────────────────────────────────────────────────────────────┐
│                      Orchestrator                            │
│  - Builds LlmInvocation from packet + phase context         │
│  - Receives LlmResult and passes to phase.postprocess()     │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
         ┌─────────────────────────────┐
         │   Box<dyn LlmBackend>       │
         │   async fn invoke(...)      │
         └─────────────┬───────────────┘
                       │
        ┌──────────────┼──────────────┐
        │              │              │
        ▼              ▼              ▼
   ┌─────────┐   ┌─────────┐   ┌──────────┐
   │ CLI     │   │ HTTP    │   │ Budgeted │
   │ Backend │   │ Backend │   │ Wrapper  │
   └─────────┘   └─────────┘   └──────────┘
        │              │
        ▼              ▼
   ┌─────────┐   ┌──────────────┐
   │ Runner  │   │ HttpClient   │
   │ (reuse) │   │ (reqwest)    │
   └─────────┘   └──────────────┘
```

### Key Design Principles

1. **Single Abstraction**: `LlmBackend` trait is the only interface the orchestrator uses
2. **Error Isolation**: `LlmError` type separates LLM concerns from runtime errors
3. **Message-Based**: Canonical `messages` array (role/content) works across all providers
4. **Reuse Infrastructure**: CLI providers leverage existing Runner (timeouts, process control, redaction)
5. **Additive Schema**: Receipt extensions are optional and backward-compatible

## Components and Interfaces

### Core Types

#### LlmBackend Trait

```rust
#[async_trait]
pub trait LlmBackend: Send + Sync {
    async fn invoke(&self, inv: LlmInvocation<'_>) -> Result<LlmResult, LlmError>;
}
```

#### LlmInvocation

```rust
pub struct LlmInvocation {
    pub spec_id: String,
    pub phase_id: String,
    pub model: String,
    pub timeout: Duration,
    pub messages: Vec<Message>,
    pub metadata: HashMap<String, serde_json::Value>,
}

pub struct Message {
    pub role: Role,
    pub content: String,
}

pub enum Role {
    System,
    User,
    Assistant,
}
```

**Design decisions**:
- Uses owned `String` types for simplicity and to allow passing between async tasks without lifetime constraints
- `provider` is NOT in `LlmInvocation`; the backend type itself encodes the provider, and backends populate provider info in `LlmResult` and receipts
- `messages` is the canonical conversation format across all providers
- **V11-V14 constraint**: `Message.content` is plain UTF-8 text only. Tool calls, images, and structured content are out of scope and would require extending this model in future versions
- `metadata` allows provider-specific hints (e.g., `temperature`, `top_p`) that override backend defaults

Each provider backend maps `messages` into its native request format:
- Claude CLI: Converts to prompt text or NDJSON
- Gemini CLI: Converts to `-p` argument
- OpenRouter: Maps to OpenAI-compatible `messages` array
- Anthropic: Maps to Messages API `messages` with explicit `system` field

#### LlmResult

```rust
pub struct LlmResult {
    pub raw_response: String,
    pub provider: String,
    pub model_used: String,
    pub tokens_input: Option<u64>,
    pub tokens_output: Option<u64>,
    pub timed_out: Option<bool>,
    pub extensions: HashMap<String, serde_json::Value>,
}
```

**Design decisions**:
- `provider` and `model_used` are populated by the backend for receipt recording
- Token counts are in provider-native units (tokens, not chars or bytes)
- `timed_out` is `Option<bool>`: `Some(true)` = definitely timed out, `Some(false)` = definitely did not, `None` = unknown/not applicable
- `extensions` is for provider-specific metadata that may be useful for debugging but is NOT automatically surfaced in receipts (requires explicit mapping)

#### LlmError

```rust
pub enum LlmError {
    Transport(String),                              // Process spawn, HTTP connectivity
    ProviderAuth(String),                           // 401, 403, missing API key
    ProviderQuota(String),                          // 429, rate limits
    ProviderOutage(String),                         // 5xx errors
    Timeout { duration: Duration },
    BudgetExceeded { limit: u32, attempted: u32 },
    Misconfiguration(String),
    Unsupported(String),
}
```

**Design decisions**:
- `ProviderAuth` and `ProviderQuota` are separate variants (not combined into `AuthOrQuota`) to preserve diagnostic information, even though they map to the same exit code
- Each variant carries enough context for debugging without exposing secrets

`LlmError` is translated to `XCheckerError` and appropriate exit codes at the orchestration layer:
- `ProviderAuth`, `ProviderQuota`, `ProviderOutage` → `claude_failure` exit code (70)
- `Timeout` → `phase_timeout` exit code (10)
- `Misconfiguration`, `Unsupported` → configuration error exit code (validated on startup, before any run)
- `BudgetExceeded` → exit code 70 with clear "LLM budget exhausted" message

### ExecutionStrategy

```rust
pub enum ExecutionStrategy {
    Controlled,
    ExternalTool,  // Not supported in V11-V14
}
```

**Configuration**:
- Scope: Per xchecker process/run (not per phase)
- Config key: `[llm] execution_strategy = "controlled"`
- CLI flag: `--execution-strategy controlled`
- Env var: `XCHECKER_EXECUTION_STRATEGY=controlled`
- Precedence: CLI flag > env var > config file
- Default: `Controlled` (if not specified)

**V11-V14 behavior**:
- Only `Controlled` is accepted
- `ExternalTool` is allowed in the enum but rejected during configuration parsing/validation
- Rejection happens on startup, before any phases run, with configuration error exit code
- Error message: "ExecutionStrategy::ExternalTool not yet supported in this version"

In `Controlled` mode:
- LLM backend produces text/JSON only
- `phase.postprocess()` interprets `LlmResult.raw_response` into structured artifacts
- `FixupEngine` performs all disk writes via atomic write path
- No LLM can write directly to disk or invoke arbitrary tools
- This guarantee is enforced by never giving backends file handles or write permissions

### Provider Implementations

#### CLI Providers (ClaudeCliBackend, GeminiCliBackend)

CLI providers wrap the existing `Runner` infrastructure. We use distinct types for each provider to encapsulate provider-specific parsing logic:

```rust
pub struct ClaudeCliBackend {
    binary_path: PathBuf,
    runner_config: RunnerConfig,
}

pub struct GeminiCliBackend {
    binary_path: PathBuf,
    runner_config: RunnerConfig,
    profiles: HashMap<String, GeminiProfile>,
}

impl LlmBackend for ClaudeCliBackend {
    async fn invoke(&self, inv: LlmInvocation) -> Result<LlmResult, LlmError> {
        // 1. Build command args from inv.messages
        // 2. Create Runner with timeout from inv.timeout
        // 3. Execute with existing process control (Job Objects, ring buffers)
        // 4. Parse stdout as NDJSON with last_valid_json_wins
        // 5. Apply redaction to stderr
        // 6. Return LlmResult with provider="claude-cli"
    }
}

impl LlmBackend for GeminiCliBackend {
    async fn invoke(&self, inv: LlmInvocation) -> Result<LlmResult, LlmError> {
        // 1. Build command: gemini -p "<prompt>" --model <model>
        // 2. Create Runner with timeout from inv.timeout
        // 3. Execute with existing process control
        // 4. Treat stdout as opaque text → raw_response
        // 5. Capture stderr into ring buffer, redact to ≤ 2 KiB
        // 6. Return LlmResult with provider="gemini-cli"
    }
}
```

**Environment and authentication**:
- CLI backends invoke binaries with the inherited process environment
- xchecker never inspects, logs, or modifies auth-related env vars (e.g., `GEMINI_API_KEY`, `ANTHROPIC_API_KEY`)
- If sandboxing or env var filtering is needed in the future, it must be a visible design change

**Timeout semantics**:
- Effective CLI timeout is `min(inv.timeout, global_max_timeout)`
- `global_max_timeout` is configurable (default: 600s)
- Both timeouts are recorded in error messages for debugging
- Timeout enforcement uses existing Runner infrastructure (Job Objects, process groups)

**Claude CLI specifics**:
- Preserves NDJSON semantics with `last_valid_json_wins` from runtime spec
- stdout parsed as structured JSON
- stderr captured and redacted

**Gemini CLI specifics**:
- Invokes: `gemini -p "<prompt>" --model <model>`
- stdout treated as opaque text → `raw_response`
- stderr captured into ring buffer, redacted to ≤ 2 KiB
- Supports named profiles for per-phase model selection (e.g., `phase.llm_profile = "design"`)

#### HTTP Providers (OpenRouterBackend, AnthropicBackend)

HTTP providers share a single `HttpClient` configured once per process. We use distinct types for each provider to encapsulate provider-specific request/response formats:

```rust
pub struct OpenRouterBackend {
    client: Arc<reqwest::Client>,
    base_url: String,
    api_key: String,
    default_model: String,
    default_params: HttpParams,
}

pub struct AnthropicBackend {
    client: Arc<reqwest::Client>,
    base_url: String,
    api_key: String,
    default_model: String,
    default_params: HttpParams,
}

pub struct HttpParams {
    pub max_tokens: u32,
    pub temperature: f32,
}

impl LlmBackend for OpenRouterBackend {
    async fn invoke(&self, inv: LlmInvocation) -> Result<LlmResult, LlmError> {
        // 1. Build OpenAI-compatible request from inv.messages
        // 2. Add required headers (Authorization, HTTP-Referer, X-Title)
        // 3. POST with timeout and retry policy
        // 4. Map HTTP errors to LlmError variants
        // 5. Parse response: choices[0].message.content and usage
        // 6. Return LlmResult with provider="openrouter"
    }
}
```

**Timeout and retry policy**:
- Per-request timeout: `min(inv.timeout, global_max_http_timeout)` (default global max: 300s)
- Retry policy: Up to 2 retries for 5xx errors and network failures, within `inv.timeout` budget
- Retries use exponential backoff: 1s, 2s
- 4xx errors (auth, quota) are NOT retried
- All retry attempts are logged (redacted)

**Parameter resolution**:
- Values in `LlmInvocation` override backend defaults:
  - `inv.model` overrides `default_model`
  - `inv.metadata["max_tokens"]` overrides `default_params.max_tokens`
  - `inv.metadata["temperature"]` overrides `default_params.temperature`
- Anything unspecified falls back to backend defaults
- This allows per-phase overrides via config

**Prompt template compatibility**:
- If a phase is configured with a prompt template that is incompatible with the selected provider, xchecker fails during configuration validation
- No "best effort" adaptation; explicit failure prevents silent misbehavior

**OpenRouter specifics**:
- Endpoint: `https://openrouter.ai/api/v1/chat/completions`
- Headers: `Authorization: Bearer $OPENROUTER_API_KEY`, `HTTP-Referer: https://effortlesssteven.com/xchecker`, `X-Title: xchecker`
- Request: OpenAI-compatible schema with `model`, `messages`, `stream: false`
- Response: Extract `choices[0].message.content` and `usage`

**Anthropic specifics**:
- Endpoint: `https://api.anthropic.com/v1/messages`
- Headers: `x-api-key: $ANTHROPIC_API_KEY`, `anthropic-version: 2023-06-01`, `content-type: application/json`
- Request: Messages API schema with explicit `system` field for system prompts
- Response: Extract first text segment from `content[...]` and `usage`
- If multiple content blocks exist, concatenate text segments in order

#### BudgetedBackend Wrapper

```rust
pub struct BudgetedBackend {
    inner: Box<dyn LlmBackend>,
    budget: Arc<AtomicU32>,
    limit: u32,
}

impl LlmBackend for BudgetedBackend {
    async fn invoke(&self, inv: LlmInvocation) -> Result<LlmResult, LlmError> {
        let current = self.budget.fetch_add(1, Ordering::SeqCst);
        if current >= self.limit {
            return Err(LlmError::BudgetExceeded {
                limit: self.limit,
                attempted: current + 1,
            });
        }
        self.inner.invoke(inv).await
    }
}
```

**Budget semantics**:
- Tracks attempted calls, not successful HTTP requests
- If the underlying backend errors, the budget slot is still consumed
- This prevents retry loops from bypassing budget limits
- Budget tracking is per xchecker process lifetime
- Each call to `invoke` counts against the limit, regardless of provider success or failure

**Budget enforcement**:
- Default limit: 20 calls per process
- Override via `XCHECKER_OPENROUTER_BUDGET` env var
- On exhaustion: fail fast with `LlmError::BudgetExceeded`, record in receipt with `budget_exhausted: true`, exit with code 70

**Future considerations**:
- If xchecker ever runs as a long-lived daemon, budget tracking will span multiple runs
- Per-run budgets would require a resettable counter scoped by run ID

### Provider Selection and Fallback

Configuration:
```toml
[llm]
provider = "claude-cli"
fallback_provider = "openrouter"  # optional
```

**Provider selection**:
- `provider` is **mandatory**; no implicit default
- Precedence: CLI flag (`--llm-provider`) > env var (`XCHECKER_LLM_PROVIDER`) > config file
- If `provider` is not set anywhere, fail with configuration error: "LLM provider must be specified"
- Supported values: `"claude-cli"`, `"gemini-cli"`, `"openrouter"`, `"anthropic"`
- Unsupported values fail immediately with configuration error

**Backend construction**:
1. Parse `provider` from config/env/CLI flag
2. Attempt to construct primary backend (validate binary path, API key env var, etc.)
3. If construction fails and `fallback_provider` is set:
   - Attempt to construct fallback backend
   - Log warning about fallback usage (redacted)
   - Record in receipt warnings with primary failure reason
4. If both fail: return configuration error before any run starts

**Fallback scope**:
- Fallback is only triggered on construction/validation failure (missing binary, missing API key, invalid config)
- **Runtime errors do NOT trigger fallback**: timeouts, provider outages, quota errors, budget exhaustion all fail the run
- This prevents silent cost/compliance issues (e.g., "OpenRouter is down, silently use Anthropic")

### Doctor Integration

`xchecker doctor` behavior for LLM providers:

**CLI providers**:
- Check binary is resolvable via PATH or configured path
- Optionally print version (e.g., `claude --version`), but don't fail if version detection fails
- Verify binary can be spawned without prompting for input
- **Never trigger LLM completion** (no prompts, no API calls)

**HTTP providers**:
- Check configured env vars are present (e.g., `OPENROUTER_API_KEY`)
- **Never make HTTP calls by default**
- Future: optional `--llm-online` flag for live connectivity checks (not in V11-V14)

**Testing approach**:
- In tests, inject fake dependencies (spawn recorder, HTTP client) to assert "no completion-shape commands / no network calls"
- Avoid OS-level monitoring (syscalls, network sniffing) in favor of dependency injection

Doctor output includes:
- Provider name and type (CLI/HTTP)
- Binary path or endpoint
- Auth status (env var present/missing, never the actual value)
- Version info (if available)
- Clear errors for missing binaries or credentials

## Data Models

### Receipt Extensions

Receipts gain an optional `llm` block under each phase entry:

```json
{
  "receipt_id": "...",
  "spec_id": "...",
  "phases": [
    {
      "phase_id": "requirements",
      "status": "success",
      "llm": {
        "provider": "openrouter",
        "model_used": "google/gemini-2.0-flash-lite",
        "tokens_input": 1024,
        "tokens_output": 512,
        "timed_out": false,
        "timeout_seconds": 300,
        "budget_exhausted": false
      },
      "warnings": [
        {
          "type": "llm_fallback",
          "message": "Primary provider 'claude-cli' failed: binary not found. Using fallback 'openrouter'."
        }
      ]
    }
  ],
  "pipeline": {
    "execution_strategy": "controlled"
  }
}
```

All `llm` fields are optional. Backward compatibility maintained via `additionalProperties: true`.

### Claude Code JSON Surfaces

Commands that support `--json` output for agent consumption:

**`xchecker spec <spec-id> --json`**:
```json
{
  "schema_version": "spec-json.v1",
  "spec_id": "feature-auth",
  "phases": [
    {
      "phase_id": "requirements",
      "status": "completed",
      "last_run": "2024-12-01T10:00:00Z"
    }
  ],
  "config_summary": {
    "execution_strategy": "controlled",
    "provider": "openrouter"
  }
}
```
- No full artifacts or packet contents
- High-level metadata only

**`xchecker status <spec-id> --json`**:
```json
{
  "schema_version": "status-json.v1",
  "spec_id": "feature-auth",
  "phase_statuses": [
    {
      "phase_id": "requirements",
      "status": "success",
      "receipt_id": "req-20241201-100000"
    }
  ],
  "pending_fixups": 0,
  "has_errors": false
}
```

**`xchecker resume <spec-id> --phase <phase> --json`**:
```json
{
  "schema_version": "resume-json.v1",
  "spec_id": "feature-auth",
  "phase": "design",
  "current_inputs": {
    "requirements_artifact": "00-requirements.md"
  },
  "next_steps": "Run design phase to generate architecture"
}
```
- No full packet or raw artifacts
- Hints for next actions

**Schema documentation**:
- JSON schemas are documented in `docs/schemas/` (separate from receipt schemas)
- Backward compatibility: adding optional fields must not break consumers
- Schema versions are independent of receipt schema versions

### Workspace Model

`workspace.yaml` structure:

```yaml
version: "1"
name: "my-project"
specs:
  - id: "feature-auth"
    tags: ["backend", "security"]
    added: "2024-12-01T10:00:00Z"
  - id: "feature-ui"
    tags: ["frontend"]
    added: "2024-12-01T11:00:00Z"
```

**Spec ID resolution**:
- `id` maps to a directory under `.xchecker/specs/<id>/`
- This follows the existing runtime spec convention for spec location
- The runtime spec defines the full spec directory structure

**Discovery**:
- Search upward from CWD for `workspace.yaml`
- The first `workspace.yaml` found is used; multiple workspaces are NOT merged
- `--workspace <path>` flag overrides discovery and specifies exact file path

### Gate Policy

Gate evaluation reads:
- Latest `status.v1.json` for the spec
- Relevant receipts for phase history

**Policy parameters**:
- `--min-phase <phase>`: Require at least this phase succeeded
- `--fail-on-pending-fixups`: Fail if any fixups are pending
- `--max-phase-age <duration>`: Fail if latest success is too old

**Phase age definition**:
- Wall-clock time since the latest **successful** receipt for the specified phase
- Failed runs do NOT reset the age timer
- This prevents flapping phases from appearing "fresh"

**Default policy** (when no flags provided):
- `--min-phase tasks` (require tasks phase completed)
- No age check
- No pending fixups check
- This default is encoded in one place and shared by CLI and library code

**Exit codes**:
- 0: Policy passed
- 1: Policy violation (expected fail, e.g., phase not reached, too old, pending fixups)
- Other: Runtime errors (config error, IO error, missing spec, etc.) use appropriate runtime exit codes

## Correctness Properties


*A property is a characteristic or behavior that should hold true across all valid executions of a system—essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

### Property 1: Controlled execution prevents disk writes

*For any* LLM backend invocation in Controlled mode, the backend must not perform any direct disk writes. All file modifications must go through the FixupEngine and atomic write path.

**Validates: Requirements 3.1.3**

### Property 2: Execution strategy appears in receipts

*For any* receipt generated after a phase execution, the receipt must contain an `execution_strategy` field indicating the mode used (e.g., "controlled").

**Validates: Requirements 3.1.4**

### Property 3: Fallback provider is used on primary failure

*For any* provider configuration with a fallback, when the primary provider fails during construction or validation, the system must attempt to use the fallback provider and record the fallback usage in receipt warnings.

**Validates: Requirements 3.2.6**

### Property 4: Doctor never triggers LLM completions for CLI providers

*For any* CLI provider configuration, running `xchecker doctor` must not result in any LLM completion requests being sent, even if the provider is fully configured and authenticated.

**Validates: Requirements 3.3.5**

### Property 5: Gemini stderr is redacted to size limit

*For any* Gemini CLI invocation, the captured stderr must be redacted to at most 2 KiB before being logged or persisted.

**Validates: Requirements 3.4.3**

### Property 6: Doctor never makes HTTP calls for HTTP providers

*For any* HTTP provider configuration, running `xchecker doctor` (without `--llm-online` flag) must not result in any HTTP requests being made to the provider's API.

**Validates: Requirements 3.5.3**

### Property 7: HTTP errors map to correct LlmError variants

*For any* HTTP backend, when receiving HTTP error responses, the backend must map them correctly: 4xx auth/quota errors to `LlmError::AuthOrQuota`, 5xx errors to `LlmError::ProviderOutage`, and network/request timeouts to `LlmError::Timeout`.

**Validates: Requirements 3.5.5**

### Property 8: HTTP logging never exposes secrets

*For any* HTTP provider invocation, all logged output (including error messages) must not contain API keys, raw auth headers, or full request bodies.

**Validates: Requirements 3.5.6**

### Property 9: Budget enforcement fails fast on exhaustion

*For any* BudgetedBackend with a configured limit, when the number of invocations reaches the limit, subsequent invocation attempts must fail immediately with `LlmError::BudgetExceeded` without calling the underlying provider.

**Validates: Requirements 3.6.6**

### Property 10: Successful invocations record provider metadata

*For any* successful LLM invocation, the resulting receipt must contain an `llm` block with `provider` and `model_used` fields populated.

**Validates: Requirements 3.8.2**

### Property 11: JSON output includes schema version

*For any* command that supports `--json` output (spec, status, resume), the JSON must include a `schema_version` field identifying the format version.

**Validates: Requirements 4.1.1**

### Property 12: JSON output respects size limits

*For any* JSON output from xchecker commands, the output must not include full packet contents or raw artifacts, keeping the response size reasonable for agent consumption.

**Validates: Requirements 4.1.4**

### Property 13: Workspace discovery searches upward

*For any* workspace-aware command, when no `--workspace` flag is provided, the system must search upward from the current working directory to find `workspace.yaml`.

**Validates: Requirements 4.3.6**

### Property 14: Gate returns correct exit codes

*For any* gate evaluation, the command must return exit code 0 on policy success, exit code 1 on policy violation, and other appropriate exit codes for runtime failures (config errors, IO errors, etc.).

**Validates: Requirements 4.5.2**

### Property 15: Hook failures respect on_fail configuration

*For any* configured hook, when the hook exits with a non-zero code, the phase must continue (with warning) if `on_fail = "warn"` (default), or fail if `on_fail = "fail"`.

**Validates: Requirements 4.8.3**

### Property 16: Hooks are subject to timeouts

*For any* hook execution, if the hook runs longer than the configured timeout (default 60s), the system must terminate the hook and handle it according to the `on_fail` configuration.

**Validates: Requirements 4.8.4**

## Error Handling

### Error Translation Strategy

The design maintains clear error boundaries:

1. **LlmError → XCheckerError**: At the orchestration layer, `LlmError` variants are translated to appropriate `XCheckerError` types
2. **XCheckerError → Exit Codes**: The CLI layer maps `XCheckerError` to exit codes defined in the runtime spec
3. **Error Context Preservation**: Error messages include enough context for debugging without exposing secrets

### Error Scenarios and Handling

| Scenario | LlmError Variant | Exit Code | Receipt Recording |
|----------|------------------|-----------|-------------------|
| Missing binary | `Misconfiguration` | Config error | Error in phase status |
| Missing API key | `ProviderAuth` | `claude_failure` (70) | Error in phase status |
| 401/403 HTTP | `ProviderAuth` | `claude_failure` (70) | Error with redacted details |
| 429 rate limit | `ProviderQuota` | `claude_failure` (70) | Error with retry suggestion |
| 5xx server error | `ProviderOutage` | `claude_failure` (70) | Error with provider name |
| Network timeout | `Timeout` | `phase_timeout` (10) | Timeout recorded with duration |
| Budget exhausted | `BudgetExceeded` | 70 | Warning + `budget_exhausted: true` |
| Process spawn fail | `Transport` | Config error | Error with command details |
| Unsupported provider | `Unsupported` | Config error | Error before run starts |

### Redaction Rules

All error messages and logs must be redacted before persistence:

1. **API Keys**: Never log environment variable values or auth headers
2. **Request Bodies**: Log only high-level structure (model, message count), not content
3. **Response Bodies**: Log only status codes and error categories, not full responses
4. **File Paths**: Redact user-specific paths in error messages
5. **Stderr**: Apply ring buffer + redaction (≤ 2 KiB) to all CLI stderr

### Fallback Error Handling

When using fallback providers:

1. Primary provider failure is logged with redacted reason
2. Fallback attempt is logged
3. If fallback succeeds: warning in receipt, run continues
4. If fallback fails: both errors logged (redacted), run fails with configuration error
5. Receipt warnings include: primary provider name, failure category, fallback provider used

## Testing Strategy

### Unit Testing

Unit tests focus on:

1. **Type Construction**: Verify `LlmInvocation`, `LlmResult`, `LlmError` construction and serialization
2. **Error Mapping**: Test HTTP status code → `LlmError` variant mapping
3. **Message Conversion**: Test `messages` array → provider-specific format conversion
4. **Configuration Parsing**: Test provider selection, fallback configuration, profile selection
5. **Budget Tracking**: Test `BudgetedBackend` counter logic
6. **Receipt Extensions**: Test `llm` block serialization and backward compatibility

### Property-Based Testing

We will use **proptest** (Rust's property-based testing library) for this project.

Property-based tests verify universal properties across all inputs. Tests use **injected fakes** rather than OS-level monitoring for testability:

1. **Property 1 (Controlled execution prevents disk writes)**: Inject fake FixupEngine, generate random LLM invocations, verify backends never receive file handles or write permissions
2. **Property 2 (Execution strategy in receipts)**: Generate random phase executions, verify all receipts contain `execution_strategy` field
3. **Property 3 (Fallback provider usage)**: Generate random provider configurations with fallbacks, simulate primary failures, verify fallback is attempted
4. **Property 4 (Doctor no LLM calls for CLI)**: Inject fake spawn recorder, generate random CLI provider configs, run doctor, verify no completion-shape commands spawned
5. **Property 5 (Gemini stderr redaction)**: Generate random stderr outputs of varying sizes (including > 2 KiB), verify all are redacted to ≤ 2 KiB
6. **Property 6 (Doctor no HTTP calls)**: Inject fake HTTP client, generate random HTTP provider configs, run doctor, verify no HTTP requests made
7. **Property 7 (HTTP error mapping)**: Generate random HTTP status codes (4xx, 5xx, timeouts), verify correct `LlmError` variant
8. **Property 8 (No secrets in logs)**: Generate random API keys and requests, capture all logs via test logger, verify no secrets present using pattern matching
9. **Property 9 (Budget enforcement)**: Generate random call sequences (below, at, and above limit), verify budget limit is enforced and correct error returned
10. **Property 10 (Provider metadata in receipts)**: Generate random successful invocations across all providers, verify `provider` and `model_used` are set
11. **Property 11 (Schema version in JSON)**: Generate random commands with `--json` (spec, status, resume), verify `schema_version` present and valid
12. **Property 12 (JSON size limits)**: Generate random specs with large artifacts, verify JSON output excludes full packet/artifacts
13. **Property 13 (Workspace discovery)**: Generate random directory structures with `workspace.yaml` at various levels, verify correct discovery (first found upward)
14. **Property 14 (Gate exit codes)**: Generate random gate policies and spec states (passed, violated, errors), verify correct exit codes (0, 1, other)
15. **Property 15 (Hook failure handling)**: Generate random hooks with various exit codes and `on_fail` configs, verify correct phase behavior (continue vs fail)
16. **Property 16 (Hook timeouts)**: Generate random hooks with various execution times, verify timeout enforcement and correct handling per `on_fail`

Each property test should run at least 100 iterations to ensure coverage across the input space.

**Testing approach**:
- Use dependency injection for all external interactions (file system, process spawning, HTTP, logging)
- Avoid OS-level monitoring (syscalls, network sniffing) in favor of fake implementations
- Fakes should be simple and focused on recording interactions, not simulating full behavior

### Integration Testing

Integration tests verify end-to-end flows:

1. **CLI Provider Flow**: Configure Claude CLI → invoke → verify receipt metadata
2. **HTTP Provider Flow**: Configure OpenRouter → invoke (mocked) → verify request format
3. **Fallback Flow**: Configure primary + fallback → simulate primary failure → verify fallback usage
4. **Budget Exhaustion Flow**: Configure budget limit → make calls → verify exhaustion handling
5. **Doctor Flow**: Configure all providers → run doctor → verify checks without LLM calls
6. **Workspace Flow**: Create workspace → add specs → list → verify status aggregation
7. **Gate Flow**: Create spec with receipts → run gate with various policies → verify exit codes
8. **Hook Flow**: Configure hooks → run phase → verify hook execution and failure handling

### Test Configuration

- **Cost Control**: All tests that could call real LLMs are skippable via `XCHECKER_SKIP_LLM_TESTS=1`
- **Real LLM Tests**: Opt-in via `XCHECKER_REAL_LLM_TESTS=1`, use minimal prompts (≤ 256 tokens)
- **Mocking**: HTTP providers use mocked responses for most tests
- **Test Isolation**: Each test uses isolated temp directories and config files

## Implementation Notes

### Phase Rollout

The implementation follows the V11-V18 roadmap:

**V11 (LLM Core)**:
- Implement `LlmBackend` trait and core types
- Implement `ExecutionStrategy::Controlled` validation
- Implement `ClaudeCliBackend` wrapping existing Runner
- Add `llm` block to receipt schema
- Property tests 1-4

**V12 (Gemini CLI)**:
- Implement `GeminiCliBackend`
- Add profile-based configuration
- Doctor checks for Gemini
- Property tests 5

**V13 (HTTP + OpenRouter)**:
- Implement shared `HttpClient`
- Implement `OpenRouterBackend`
- Implement `BudgetedBackend` wrapper
- Property tests 6-9

**V14 (Anthropic + Metadata)**:
- Implement `AnthropicBackend`
- Complete receipt metadata implementation
- Provider-specific prompt templates
- Property tests 10

**V15 (Claude Code)**:
- Implement `--json` output for spec/status/resume
- Document JSON schemas
- Example flows
- Property tests 11-12

**V16 (Workspace)**:
- Implement workspace registry (`workspace.yaml`)
- Implement `project` subcommands
- Optional TUI
- Property tests 13

**V17 (Gates)**:
- Implement `gate` command
- Policy evaluation logic
- CI templates
- Property tests 14

**V18 (Ecosystem)**:
- Implement templates
- Implement hooks
- Showcase examples
- Property tests 15-16

### Configuration Precedence

For all configuration values:
1. CLI flags (highest priority)
2. Environment variables
3. Config file (`.xchecker/config.toml`)
4. Built-in defaults (lowest priority)

### Backward Compatibility

All schema changes maintain backward compatibility:
- New fields are optional
- `additionalProperties: true` remains in effect
- Old clients can read new receipts (ignoring unknown fields)
- New clients can read old receipts (using defaults for missing fields)

If breaking changes are needed, introduce new schema versions (e.g., `receipt.v2.json`) with explicit migration path.

### Security Considerations

1. **Secret Management**: Never log, persist, or expose API keys or auth tokens
2. **Redaction**: Apply redaction before any logging or persistence
3. **Process Isolation**: CLI providers use existing Job Objects / process groups
4. **Timeout Enforcement**: All LLM calls and hooks have mandatory timeouts
5. **Budget Limits**: Prevent runaway costs with per-process call budgets
6. **Input Validation**: Validate all configuration before constructing backends

### Performance Considerations

1. **Connection Reuse**: HTTP client reuses connections across invocations
2. **Lazy Construction**: Backends are constructed only when needed
3. **Streaming**: Initial implementation is non-streaming; streaming can be added later without API changes
4. **Caching**: Consider caching provider metadata (models, capabilities) in future versions
5. **Parallel Execution**: Multiple specs can run in parallel with independent budgets

### Examples and Walkthroughs

All examples and walkthrough flows are **exercised in CI as smoke tests**:
- `examples/fullstack-nextjs/` has a test script that runs the documented workflow
- `examples/mono-repo/` has a test script that verifies workspace commands
- Walkthrough code snippets are extracted and tested
- This prevents examples from becoming subtly broken over time

### Extensibility Points

Future extensions can add:

1. **New Providers**: Implement `LlmBackend` trait for new providers
2. **Streaming**: Add streaming support to `LlmBackend` without breaking existing code
3. **ExternalTool Mode**: Implement when ready, with explicit safety documentation
4. **Custom Hooks**: Add more hook points (pre/post gate, pre/post doctor, etc.)
5. **Advanced Budgets**: Per-spec budgets, cost-based budgets, time-based budgets
6. **Provider Capabilities**: Query provider for supported features (tools, vision, etc.)


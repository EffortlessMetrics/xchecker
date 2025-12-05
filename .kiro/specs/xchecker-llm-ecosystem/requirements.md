# xchecker LLM & Ecosystem Requirements (V11–V18)

## 1. Introduction

**Status:** Design baseline for multi-provider LLM support and ecosystem expansion.

**Scope:** Functional and non-functional requirements for **V11–V18**, building on the runtime core defined in `docs/REQUIREMENTS_RUNTIME_V1.md`.

Out of scope for this document:

- Changes to core runner, packet, and fixup semantics (covered by the runtime spec).
- New exit codes or error categories beyond those already defined, except where explicitly noted.

V11–V18 introduce:

- Multi-provider LLM backend abstraction.
- Gemini CLI, OpenRouter, Anthropic HTTP support.
- LLM metadata in receipts.
- Claude Code (Claude Code) integration.
- Workspace / multi-spec orchestration.
- CI gates.
- Templates, hooks, and examples.

All requirements here are **additive** to the runtime spec. Backward compatibility rules are defined in §7.

---

## 2. Glossary

- **LLM Backend** – Trait-based abstraction for invoking language models (CLI or HTTP).
- **Transport** – Mechanism used to talk to a provider: CLI process, HTTP API.
- **Provider** – LLM service: Claude CLI, Gemini CLI, OpenRouter, Anthropic API.
- **ExecutionStrategy** – Policy determining whether LLMs only propose changes (`Controlled`) or can directly write (`ExternalTool` – not supported initially).
- **Claude Code (Claude Code)** – Claude IDE environment that can call external tools/CLIs.
- **Workspace** – Logical grouping of multiple specs within a repo or organization.
- **Gate** – Decision step that uses receipts/status/lockfiles to decide whether to allow an action (e.g., PR merge).
- **Template** – Predefined config/spec bundle for a common stack (Next.js, Rust microservice, FastAPI, etc.).
- **Hook** – Script or handler invoked before/after phases or gates.

Common cross-cutting terms:

- **Exit codes** – Reuse existing named exit codes from the runtime spec (e.g. `phase_timeout`, `claude_failure`) unless a new code is explicitly defined.
- **Receipt** – The existing `receipt.v1.json` structure, extended with optional `llm` metadata (§3.8).

---

## 3. Functional Requirements – LLM & Providers (V11–V14)

### 3.1 FR-EXEC (Requirement 25): ExecutionStrategy

**User Story:**
As a developer, I want a **Controlled** execution mode where LLMs only propose changes and all writes go through xchecker's fixup pipeline.

**Acceptance Criteria**

1. An `ExecutionStrategy` enum exists at the core layer (e.g. `core::ExecutionStrategy`) with at least:
   - `Controlled`,
   - `ExternalTool`.
2. In **V11–V14**:
   - Only `ExecutionStrategy::Controlled` is accepted at runtime.
   - Any attempt to configure `ExternalTool` (via config or CLI) fails early with `XCheckerError::Unsupported("ExternalTool not yet supported")` and a non-zero exit code (use the standard "configuration error" exit from the runtime spec).
3. For `Controlled`:
   - The LLM backend produces text/JSON only; it **never** writes to disk or invokes arbitrary tools on its own.
   - `phase.postprocess()` interprets `LlmResult` into structured artifacts/fixup plans.
   - The `FixupEngine` + atomic write path performs all disk writes, reusing existing runtime guarantees.
4. Receipts gain an `execution_strategy` field (e.g., `"controlled"`) in a top-level `pipeline` block, or another location consistent with the runtime spec's schema.
5. Future support for `ExternalTool` must:
   - Explicitly describe any allowed deviation from the `FixupEngine`/atomic semantics.
   - Never silently bypass existing safety controls. Any such change must update the runtime spec and increment schema version if needed.

---

### 3.2 FR-LLM (Requirement 26): LlmBackend Abstraction

**User Story:**
As a developer, I want to switch LLM providers without rewriting orchestration logic.

**Acceptance Criteria**

1. Introduce an `LlmBackend` trait with a single primary entrypoint:
   ```rust
   async fn invoke(&self, inv: LlmInvocation<'_>) -> Result<LlmResult, LlmError>;
   ```
   - `LlmInvocation` includes at minimum:
     - `spec_id` (string),
     - `phase_id` (string),
     - `provider` (enum or identifier),
     - `model` (string),
     - `timeout` (duration),
     - `messages` (ordered list of `{ role, content }` pairs),
     - optional metadata for provider-specific hints (e.g. temperature, top_p).
   - `messages` must be the canonical abstraction for conversation across all backends. Individual providers map this into their own request formats.
   - `LlmResult` includes:
     - `raw_response` (string),
     - optional `tokens_input` and `tokens_output`,
     - optional `timed_out: bool`,
     - provider-specific metadata in an extensions map (string → JSON).
2. A new `LlmError` type exists, distinct from `RunnerError`, with variants for:
   - Transport failures (e.g., process spawn, HTTP connectivity),
   - Provider-level failures (auth, quota, 4xx/5xx),
   - Timeouts,
   - Misconfiguration.
   - `LlmError` is translated into existing `XCheckerError` / exit codes in the orchestration layer.
3. The orchestrator interacts solely with a `Box<dyn LlmBackend>`:
   - It builds `LlmInvocation` from packet + phase context.
   - It receives `LlmResult` and passes `raw_response` into `phase.postprocess()`.
4. Supported provider kinds:
   - `"claude-cli"`,
   - `"gemini-cli"`,
   - `"openrouter"`,
   - `"anthropic"`,
   even if only `"claude-cli"` is implemented in V11. Unsupported providers fail with a configuration error before any run starts.
5. Configuration selects provider:
   ```toml
   [llm]
   provider = "claude-cli" | "gemini-cli" | "openrouter" | "anthropic"
   fallback_provider = "..." # optional
   ```
6. Fallback support:
   - If the primary provider fails creation/validation and a `fallback_provider` is configured, xchecker must attempt to construct the fallback backend.
   - If fallback succeeds, runs continue using the fallback.
   - If fallback fails, the run fails with a configuration error.
   - Any fallback usage must be recorded in receipt warnings (§3.8) and surfaced in human-readable logs.

---

### 3.3 FR-LLM-CLI (Requirement 27): CLI Providers

**User Story:**
As a developer, I want to use local CLIs (Claude, Gemini) as providers with the same process control guarantees as today's Runner.

**Acceptance Criteria**

1. CLI providers reuse the existing Runner infrastructure:
   - Timeouts,
   - Job Objects / process group handling,
   - stdout and stderr ring buffers,
   - redaction.
2. Configuration:
   ```toml
   [llm]
   provider = "claude-cli" | "gemini-cli"

   [llm.claude]
   binary = "/usr/local/bin/claude"  # optional; falls back to PATH

   [llm.gemini]
   binary = "/usr/local/bin/gemini"  # optional; falls back to PATH
   ```
3. Provider selection:
   - `[llm] provider` chooses the active CLI provider.
   - `[llm.<provider>]` subsections contain provider-specific options.
   - `--llm-provider` CLI flag and `XCHECKER_LLM_PROVIDER` env override config, with precedence: CLI flag > env var > config file.
4. Binary discovery:
   - If `binary` is unset, xchecker finds the binary via the current PATH.
   - On failure, `xchecker doctor` and any run must report a clear "binary not found" error, including which locations were checked.
5. `xchecker doctor` behavior for CLI providers:
   - For each configured CLI provider, doctor:
     - Checks that the binary is resolvable.
     - Optionally prints a version string if available (e.g. `claude --version`), but must not fail if version detection fails.
     - Checks that the binary can be spawned without prompting for input.
   - Doctor must not trigger an LLM completion during these checks.
6. When invoked, CLI providers:
   - Apply the same timeout / process tree logic as current Runner.
   - Propagate `LlmInvocation.timeout` into the Runner config.
7. stdout/stderr handling:
   - For Claude CLI: preserve NDJSON semantics from the runtime spec, including `last_valid_json_wins` behavior.
   - For Gemini CLI: treat stdout as opaque text; capture stderr into a ring buffer and apply the same redaction rules as the runtime Runner.

---

### 3.4 FR-LLM-GEM (Requirement 28): Gemini CLI Backend

**User Story:**
As a developer, I want to use Gemini CLI as a first-class backend.

**Acceptance Criteria**

1. Gemini CLI backend invokes a non-interactive command of the form:
   ```bash
   gemini -p "<prompt>" --model <model>
   ```
   or an equivalent invocation that is:
   - non-REPL,
   - non-interactive,
   - suitable for automated use.
2. Authentication:
   - Handled via `GEMINI_API_KEY` (or provider-specific env var per Gemini CLI docs).
   - xchecker never reads, logs, or persists the key or raw auth headers.
3. Output:
   - stdout is treated as opaque text and mapped to `LlmResult.raw_response`.
   - stderr is captured into a ring buffer and redacted to ≤ 2 KiB before logging.
4. `xchecker doctor` with Gemini configured:
   - Uses `gemini -h` (or equivalent help/version command) to verify binary presence.
   - Must not send a real completion request or require a valid API key.
5. Configuration:
   ```toml
   [llm.gemini]
   default_model = "gemini-2.0-flash-lite"

   # Optional named prompt profiles for different phases:
   [llm.gemini.profiles.requirements]
   model = "gemini-2.0-flash-lite"
   max_tokens = 1024

   [llm.gemini.profiles.design]
   model = "gemini-2.0-pro"
   max_tokens = 2048
   ```
   - Per-phase overrides refer to profiles (e.g. `phase.llm_profile = "design"`).
   - If no profile is specified, `default_model` is used with reasonable built-in defaults for `max_tokens` and `temperature`.
6. Tools policy:
   - V11–V14 operate in text-only mode; tools and function calling are disabled.
   - If a future `[llm.gemini] allow_tools = true` flag is added, it must be off by default and documented in an updated spec.

---

### 3.5 FR-LLM-API (Requirement 29): HTTP Providers

**User Story:**
As a developer, I want to use HTTP LLM APIs with proper error mapping and secrecy.

**Acceptance Criteria**

1. HTTP providers share a single `HttpClient` built on `reqwest` (or equivalent), configured once per process with:
   - reasonable connect/read timeouts,
   - standard proxy and TLS support,
   - connection reuse.
2. Credentials:
   - Loaded from env vars defined in config, e.g.:
     ```toml
     [llm.openrouter]
     api_key_env = "OPENROUTER_API_KEY"
     ```
   - If the env var is missing, provider construction fails with a clear error before any run starts.
3. `xchecker doctor` behavior for HTTP providers:
   - Doctor checks for the presence of configured env vars.
   - Doctor must not make HTTP calls by default.
   - An optional `--llm-online` flag may later allow live calls; that would be specified in a future version.
4. HTTP backends initially support non-streaming requests only.
   - Streaming may be added later without changing the `LlmBackend` surface.
5. Error mapping:
   - 4xx auth/quota errors map to an `LlmError::AuthOrQuota` and a `claude_failure` (or equivalent) exit code (as defined in the runtime spec), with a clear reason.
   - 5xx errors map to `LlmError::ProviderOutage` and the same exit class, with a "provider outage" message.
   - Network and request timeouts map to `LlmError::Timeout`, and are translated into `phase_timeout` exit code.
   - All mappings must preserve enough context to debug issues without exposing secrets.
6. Logging:
   - Must never log API keys, raw auth headers, or full request bodies.
   - Allowed to log:
     - provider name,
     - model,
     - region (if applicable),
     - token counts,
     - high-level error code/category and short message.
7. HTTP errors must be redacted before being persisted in receipts or logs.

---

### 3.6 FR-LLM-OR (Requirement 30): OpenRouter Backend

**User Story:**
As a developer, I want to use OpenRouter as a multi-model HTTP provider with budget control.

**Acceptance Criteria**

1. Default endpoint:
   ```
   https://openrouter.ai/api/v1/chat/completions
   ```
2. Configuration:
   ```toml
   [llm]
   provider = "openrouter"

   [llm.openrouter]
   base_url   = "https://openrouter.ai/api/v1/chat/completions"
   api_key_env = "OPENROUTER_API_KEY"
   model       = "google/gemini-2.0-flash-lite"
   max_tokens  = 2048
   temperature = 0.2
   ```
   - `base_url` is optional and defaults to the endpoint above.
   - `model` is required.
3. Required headers:
   - `Authorization: Bearer $OPENROUTER_API_KEY`
   - `HTTP-Referer: https://effortlesssteven.com/xchecker`
   - `X-Title: xchecker`
4. Request format:
   - Uses OpenAI-compatible schema:
     - `model`,
     - `messages[...]` derived from `LlmInvocation.messages`,
     - `stream: false`,
     - optional `max_tokens`, `temperature`, etc.
5. Response parsing:
   - Maps `choices[0].message.content` into `LlmResult.raw_response`.
   - Uses `usage` (if available) to populate `tokens_input` and `tokens_output`.
6. Budget control:
   - Enforced via a `BudgetedBackend` wrapper that implements `LlmBackend`.
   - Wrapper tracks call counts per process and per run, enforcing NFR9 (§5.2).
   - On budget exhaustion, the backend fails fast with `LlmError::BudgetExceeded`, mapped to exit code 70 and a clear "LLM budget exhausted" message.
   - Receipts must include a warning when budget exhaustion occurs (§3.8).

---

### 3.7 FR-LLM-ANTH (Requirement 31): Anthropic Backend

**User Story:**
As a developer, I want to use Anthropic's Messages API directly.

**Acceptance Criteria**

1. Default endpoint:
   ```
   https://api.anthropic.com/v1/messages
   ```
2. Configuration:
   ```toml
   [llm]
   provider = "anthropic"

   [llm.anthropic]
   base_url   = "https://api.anthropic.com/v1/messages"
   api_key_env = "ANTHROPIC_API_KEY"
   model       = "claude-3-5-sonnet-20241022"
   max_tokens  = 2048
   temperature = 0.2
   ```
   - `base_url` is optional; defaults to the endpoint above.
3. Required headers:
   - `x-api-key: $ANTHROPIC_API_KEY`
   - `anthropic-version: 2023-06-01` (or current stable version, pinned in code)
   - `content-type: application/json`
4. Request format:
   - Uses Anthropic's Messages API schema, mapping `LlmInvocation.messages` into a `messages` array with explicit roles.
   - System prompts use the API's `system` field when present.
5. Response parsing:
   - Extracts the first text segment from `content[...]` into `LlmResult.raw_response`.
   - Uses `usage` to populate token counts when available.
   - If multiple content blocks exist, the backend concatenates text segments in a documented order.
6. Prompt templates:
   - Either reuse existing prompt templates (if compatible) or define Anthropic-specific templates.
   - Provider-specific templates must be clearly separated and selected by provider.

---

### 3.8 FR-LLM-META (Requirement 32): LLM Metadata in Receipts

**User Story:**
As a developer, I want receipts to record provider/model/usage metadata.

**Acceptance Criteria**

1. Receipts gain an optional `llm` block under each phase entry, with fields:
   - `provider` (string),
   - `model_used` (string),
   - `tokens_input` (integer, optional),
   - `tokens_output` (integer, optional),
   - `timed_out` (boolean, optional),
   - `timeout_seconds` (integer, optional),
   - `budget_exhausted` (boolean, optional).
2. For any successful invocation:
   - `provider` and `model_used` must be set.
   - Token counts are set when the provider returns usage data.
3. For timeouts:
   - `timed_out: true` is set.
   - `timeout_seconds` reflects the effective timeout.
4. For fallbacks:
   - Receipts must include a warning at the phase level indicating:
     - primary provider name,
     - failure reason (redacted),
     - fallback provider used.
5. For budget exhaustion:
   - `budget_exhausted: true` is set.
   - A phase-level warning explains that calls were blocked due to budget.
6. Backward compatibility:
   - `receipt.v1.json` remains valid:
     - `llm` is optional.
     - All new fields are optional.
     - `additionalProperties: true` remains in effect.

---

## 4. Functional Requirements – Claude Code, Workspace, Gates, Ecosystem (V15–V18)

### 4.1 FR-Claude Code-CLI (Requirement 36): Claude Code CLI Surfaces

**User Story:**
As a Claude Code user, I want xchecker to provide machine-friendly JSON for spec, status, and resume.

**Acceptance Criteria**

1. `xchecker spec <spec-id> --json` returns a stable JSON shape, including:
   - `schema_version` (e.g. `"spec-json.v1"`),
   - `spec_id`,
   - `phases` (with high-level metadata, not full artifacts),
   - config summary (paths, execution strategy).
2. `xchecker status <spec-id> --json` returns a compact status summary:
   - `schema_version` (e.g. `"status-json.v1"`),
   - `spec_id`,
   - `phase_statuses[...]`,
   - latest `receipt_id` per phase,
   - flags for pending fixups, errors.
3. `xchecker resume <spec-id> --phase <phase> --json` returns:
   - `schema_version` (e.g. `"resume-json.v1"`),
   - `spec_id`, `phase`,
   - current inputs and next-step hints (not full packet/artifacts).
4. JSON contracts:
   - Are documented in a separate JSON Schema doc.
   - Avoid large dumps (no full packet or raw artifacts by default).
   - Are backward compatible: adding optional fields must not break consumers.

---

### 4.2 FR-Claude Code-FLOWS (Requirement 37): Example Flows

**User Story:**
As a Claude Code user, I want example flows showing how to integrate xchecker.

**Acceptance Criteria**

1. Documentation includes at least one complete example showing:
   - `xchecker spec --json` → parsed in Claude Code.
   - `xchecker resume --phase design --json` invoked from within Claude Code.
   - How to map JSON into Claude Code's tool invocation model.
2. A canonical slash command scheme is documented, e.g.:
   - `/xchecker spec <spec-id>`
   - `/xchecker resume <spec-id> <phase>`
3. Examples use receipts/status JSON as the source of truth instead of directly introspecting the repo.

---

### 4.3 FR-WORKSPACE (Requirement 38): Workspace Model

**User Story:**
As a developer, I want to manage many specs and see their status in one place.

**Acceptance Criteria**

1. `xchecker project init <name>` creates a workspace registry (e.g. `workspace.yaml`) in the current directory and marks it as the project root.
2. `xchecker project add-spec <spec-id> --tag <tag>`:
   - Registers spec metadata in `workspace.yaml`.
   - Supports multiple tags per spec.
   - Fails if the same spec is added twice without `--force`.
3. `xchecker project list` lists all specs with:
   - spec id,
   - status (derived from latest receipts),
   - tags.
4. `xchecker project status --json` emits aggregated status for all specs:
   - `schema_version` (e.g. `"workspace-status-json.v1"`),
   - per-spec phase summaries,
   - counts of failed / pending / stale specs.
5. `xchecker project history <spec-id> --json` emits a timeline of:
   - phase progression,
   - timestamps,
   - selected metrics (e.g. LLM token usage, fixup counts).
6. Workspace discovery:
   - Commands locate `workspace.yaml` by searching upward from CWD.
   - If multiple workspaces are possible, a `--workspace` flag selects the file.

---

### 4.4 FR-WORKSPACE-TUI (Requirement 39): TUI

**User Story:**
As a developer, I want a terminal UI for workspace overview.

**Acceptance Criteria**

1. `xchecker project tui` launches a TUI that displays:
   - Specs list with tags and last status,
   - Latest receipt summary per selected spec,
   - Pending fixups, error counts, and stale specs.
2. Navigation:
   - Keyboard-only (e.g. arrow keys, j/k, Enter, q).
   - Operates on the same `workspace.yaml` registry as CLI commands.
3. The TUI is read-only in V16 (no destructive operations).

---

### 4.5 FR-GATE (Requirement 40): Gate Command

**User Story:**
As a developer, I want to gate merges on xchecker receipts/status.

**Acceptance Criteria**

1. `xchecker gate <spec-id>`:
   - Reads the latest status + relevant receipts for the spec.
   - Evaluates a policy defined by CLI flags and/or config.
2. Exit codes:
   - 0 on policy success.
   - Non-zero on policy failure, with:
     - 1 for policy violation (expected fail).
     - Existing error codes (e.g. config/IO errors) for runtime failures.
3. Parameters include:
   - `--min-phase <phase_name>` – require that at least this phase has succeeded.
   - `--fail-on-pending-fixups` – fail if any pending fixups exist.
   - `--max-phase-age 7d` – fail if the latest successful run is older than the threshold.
4. Output:
   - Human-friendly text to stdout/stderr explaining pass/fail reasons.
   - Optional `--json` flag emitting structured JSON with:
     - the decision,
     - evaluated conditions,
     - reasons for failure.

---

### 4.6 FR-GATE-CI (Requirement 41): CI Templates

**User Story:**
As a developer, I want ready-made CI templates.

**Acceptance Criteria**

1. The repo ships an example GitHub Actions workflow (e.g. `.github/workflows/xchecker-gate.yml`) that:
   - Runs `xchecker gate <spec-id>` as a PR check.
   - Shows how to configure `--min-phase`, `--fail-on-pending-fixups`, etc.
   - Demonstrates reading workspace/spec configuration from the repo.
2. GitLab CI snippet is documented in `docs/ci/gitlab.md` or similar.
3. Docs explain how to configure required status checks in GitHub / GitLab so that gate failures block merges.

---

### 4.7 FR-TEMPLATES (Requirement 42): Spec Templates

**User Story:**
As a developer, I want to bootstrap a spec quickly from a template.

**Acceptance Criteria**

1. `xchecker template list` lists built-in templates, e.g.:
   - `fullstack-nextjs`,
   - `rust-microservice`,
   - `python-fastapi`,
   - `docs-refactor`.
2. `xchecker template init <template> <spec-id>` seeds:
   - A problem statement,
   - Minimal `.xchecker/config.toml`,
   - An example partial spec flow snippet demonstrating at least one phase.
3. Each template has a short README describing:
   - Intended use,
   - Required prerequisites,
   - How to run the basic flow.

---

### 4.8 FR-HOOKS (Requirement 43): Hooks

**User Story:**
As a developer, I want xchecker to call my scripts at key points.

**Acceptance Criteria**

1. Hooks are defined via config (e.g. `hooks.toml` or a section in `.xchecker/config.toml`) with entries like:
   ```toml
   [hooks.pre_phase.design]
   command = "./scripts/pre_design.sh"
   on_fail = "warn"  # or "fail"
   ```
2. When a hook is configured, xchecker executes it:
   - With relevant context via environment variables (e.g. `XCHECKER_SPEC_ID`, `XCHECKER_PHASE`) and/or a small JSON payload on stdin.
   - In the project root directory.
3. Failure behavior:
   - Default is non-blocking (`on_fail = "warn"`): hook failures are logged and recorded in receipts but do not fail the phase.
   - If `on_fail = "fail"`, a non-zero hook exit code fails the phase with a clear error.
4. Hooks are subject to reasonable timeouts (e.g. 60s by default) to avoid stalling the pipeline.

---

### 4.9 FR-SHOWCASE (Requirement 44): Examples & Walkthroughs

**User Story:**
As a new user, I want concrete examples and walkthroughs.

**Acceptance Criteria**

1. `examples/fullstack-nextjs/`:
   - Shows a working scenario with a scriptable workflow.
   - Includes a README with step-by-step instructions.
2. `examples/mono-repo/`:
   - Demonstrates multiple specs under one workspace.
   - Shows how `xchecker project` commands operate on this setup.
3. Docs include at least two walkthroughs:
   - "Running xchecker on your repo in 20 minutes".
   - "From spec to PR: xchecker + Claude Code flow".
4. Each walkthrough is runnable using code and config present in the repo.

---

## 5. LLM-Specific Non-Functional Requirements

These are in addition to runtime NFRs defined in `REQUIREMENTS_RUNTIME_V1.md`.

### 5.1 NFR8 – Cost Control

- Tests that call real LLMs must be skippable via `XCHECKER_SKIP_LLM_TESTS=1`.
- Default CI configuration skips real LLM calls.
- `xchecker doctor` for LLM providers must not call the LLMs by default.
- Integration tests that use real calls must:
  - Use minimal prompts (short strings).
  - Use low `max_tokens` (≤ 256).
  - Be opt-in via `XCHECKER_REAL_LLM_TESTS=1` or provider-specific envs.
- When real LLM tests are enabled, their use should be visible in test output.

---

### 5.2 NFR9 – OpenRouter Call Budget

- Default per-process OpenRouter call budget is ≤ 20 for a single `xchecker` run.
- A budget override is available via `XCHECKER_OPENROUTER_BUDGET` for local runs.
- CI must respect the default or a stricter budget.
- Tests must assert:
  ```
  used_calls <= configured_budget
  ```
- On budget exhaustion:
  - The backend fails fast with `LlmError::BudgetExceeded`.
  - Exit code 70 (or the existing "provider failure" code) is returned.
  - The event is recorded in receipts (`budget_exhausted: true`) and logs.

---

## 6. Roadmap Summary (V11–V18)

Each version should deliver a walking skeleton that works end-to-end for that slice, then fill in details with tests and docs.

- **V11**: LLM core skeleton:
  - `LlmBackend` abstraction,
  - `ExecutionStrategy::Controlled`,
  - `ClaudeCliBackend` behind `LlmBackend`.
- **V12**: Gemini CLI:
  - `GeminiCliBackend` with config and doctor checks.
- **V13**: HTTP core + OpenRouter:
  - Shared HTTP client,
  - OpenRouter backend,
  - Budgeted backend enforcing NFR9.
- **V14**: Anthropic + LLM metadata:
  - Anthropic Messages API backend,
  - Rich LLM metadata in receipts,
  - Provider-specific prompt templates.
- **V15**: Claude Code CLI surfaces and documented flows.
- **V16**: Workspace model, project status/history, optional TUI.
- **V17**: Gate command + CI templates.
- **V18**: Templates, hooks, showcase examples.

---

## 7. Relationship to Runtime Spec

This LLM & ecosystem spec assumes:

- The runtime requirements in `docs/REQUIREMENTS_RUNTIME_V1.md` are stable and enforced.
- All FR-LLM* requirements here are additive:
  - They may extend schemas but must keep v1 backward compatible by only adding optional fields, or introduce v2 schemas explicitly.
- Changes to runtime behavior or contracts:
  - Must first be reflected in the runtime spec.
  - Must then be referenced or extended here if they affect LLM or ecosystem features.

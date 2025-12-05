# Implementation Plan: xchecker LLM & Ecosystem (V11–V18)

This implementation plan breaks down the design into discrete, incremental coding tasks. Each task builds on previous tasks and references specific requirements from the requirements document.

## V11: LLM Core Skeleton

- [x] 1. Set up LLM core types and abstractions





  - Create `src/llm/` module structure
  - Define `LlmBackend` trait with `async fn invoke()`
  - Define `LlmInvocation`, `LlmResult`, `LlmError` types
  - Define `Message` and `Role` types
  - Define `ExecutionStrategy` enum
  - _Requirements: 3.1.1, 3.2.1_

- [x] 2. Implement ClaudeCliBackend (minimal)




  - Create `ClaudeCliBackend` struct wrapping existing Runner
  - Implement `LlmBackend` trait for `ClaudeCliBackend`
  - Convert `LlmInvocation.messages` to Claude CLI command format
  - Apply timeout from `LlmInvocation.timeout`
  - Parse stdout as NDJSON with `last_valid_json_wins`
  - Capture and redact stderr
  - Populate `LlmResult` with `provider="claude-cli"`
  - No receipts yet (defer to task 4)
  - _Requirements: 3.3.1, 3.3.6, 3.3.7_

- [x] 2.1 Implement provider selection and configuration


  - Add `[llm] provider` config key
  - Add `--llm-provider` CLI flag
  - Add `XCHECKER_LLM_PROVIDER` env var
  - Implement precedence: CLI > env > config
  - Validate provider is mandatory (no default)
  - Support `"claude-cli"` value, reject others with clear error
  - _Requirements: 3.2.4, 3.2.5_

- [x] 2.2 Implement CLI binary discovery


  - Add `[llm.claude] binary` config key
  - Search PATH if binary not configured
  - Report clear error if binary not found
  - _Requirements: 3.3.4_

- [x] 3. Integrate LlmBackend into orchestrator





  - Modify orchestrator to construct `Box<dyn LlmBackend>` based on config
  - Build `LlmInvocation` from packet + phase context
  - Pass `LlmResult.raw_response` to `phase.postprocess()`
  - Translate `LlmError` to `XCheckerError` and exit codes
  - Use hard-coded `ExecutionStrategy::Controlled` for now
  - _Requirements: 3.2.2, 3.2.3_

- [x] 3.1 Write unit tests for LlmError translation


  - Test `ProviderAuth` → `claude_failure` exit code
  - Test `ProviderQuota` → `claude_failure` exit code
  - Test `ProviderOutage` → `claude_failure` exit code
  - Test `Timeout` → `phase_timeout` exit code
  - Test `Misconfiguration` → configuration error exit code
  - _Requirements: 3.2.2_

- [x] 4. Extend receipt schema with LLM metadata





  - Add optional `llm` block to phase entries in receipt schema
  - Add `execution_strategy` field to `pipeline` block
  - Ensure backward compatibility (all fields optional)
  - Update receipt serialization/deserialization
  - Record LLM metadata in receipts from orchestrator
  - _Requirements: 3.1.4, 3.8.1_

- [x] 4.1 Write unit tests for receipt execution_strategy field


  - For Controlled runs, assert `pipeline.execution_strategy == "controlled"`
  - For config pointing to ExternalTool in V11–V14, assert startup failure and no receipt written
  - **Property: Execution strategy appears in receipts**
  - **Validates: Requirements 3.1.4**

- [x] 4.2 Write property test for Controlled execution


  - **Property: Controlled execution prevents disk writes**
  - **Validates: Requirements 3.1.3**

- [x] 5. Implement ExecutionStrategy configuration and validation





  - Add `[llm] execution_strategy` config key
  - Add `--execution-strategy` CLI flag
  - Add `XCHECKER_EXECUTION_STRATEGY` env var
  - Implement precedence: CLI > env > config
  - Validate only `Controlled` is accepted in V11
  - Reject `ExternalTool` with configuration error on startup
  - _Requirements: 3.1.2_

- [x] 6. Implement doctor checks for CLI providers





  - Add doctor check for Claude CLI binary resolution
  - Optionally print version (`claude --version`)
  - Verify binary can be spawned without prompting
  - Never trigger LLM completion
  - _Requirements: 3.3.5_

- [x] 6.1 Write property test for doctor no LLM calls


  - **Property: Doctor never triggers LLM completions for CLI providers**
  - **Validates: Requirements 3.3.5**

- [x] 7. Document LLM provider configuration and usage





  - Add `docs/LLM_PROVIDERS.md` describing gemini-cli, claude-cli, openrouter, anthropic
  - Include env vars, config keys, test gating flags, quota/budget notes
  - Cross-link from README and CONFIGURATION.md
  - _Requirements: FR-LLM-DOCS_

- [x] 8. Checkpoint: V11 walking skeleton complete




  - Ensure all tests pass, ask the user if questions arise.

## V12: Gemini CLI

- [x] 9. Implement GeminiCliBackend





  - Create `GeminiCliBackend` struct wrapping existing Runner
  - Implement `LlmBackend` trait for `GeminiCliBackend`
  - Convert `LlmInvocation.messages` to `gemini -p "<prompt>" --model <model>` format
  - Apply timeout from `LlmInvocation.timeout`
  - Treat stdout as opaque text → `raw_response`
  - Capture stderr into ring buffer, redact to ≤ 2 KiB
  - Populate `LlmResult` with `provider="gemini-cli"`
  - _Requirements: 3.4.1, 3.4.2, 3.4.3_

- [x] 9.1 Write property test for Gemini stderr redaction


  - **Property: Gemini stderr is redacted to size limit**
  - **Validates: Requirements 3.4.3**

- [x] 9.2 Implement Gemini profile-based configuration

  - Add `[llm.gemini] default_model` config key
  - Add `[llm.gemini.profiles.<name>]` sections for per-phase overrides
  - Support `model`, `max_tokens` in profiles
  - Resolve profile from phase config or use default
  - _Requirements: 3.4.5_

- [x] 9.3 Update provider selection to support Gemini

  - Add `"gemini-cli"` to supported provider values
  - Implement binary discovery for Gemini
  - _Requirements: 3.2.4, 3.3.4_


- [x] 9.4 Add doctor checks for Gemini CLI


  - Check Gemini binary resolution
  - Use `gemini -h` to verify binary presence
  - Never send real completion request
  - _Requirements: 3.4.4_


- [x] 9.5 Write property test for Gemini doctor behavior

  - **Property: Doctor never triggers LLM completions for CLI providers** (Gemini variant)
  - **Validates: Requirements 3.4.4**

- [x] 10. Checkpoint - Ensure all tests pass





  - Ensure all tests pass, ask the user if questions arise.

## V13: HTTP Core + OpenRouter

- [x] 11. Implement shared HTTP client infrastructure





  - Create shared `reqwest::Client` configured once per process
  - Configure connect/read timeouts, proxy, TLS
  - Implement connection reuse
  - _Requirements: 3.5.1_

- [x] 11.1 Implement HTTP timeout and retry policy


  - Per-request timeout: `min(inv.timeout, global_max_http_timeout)`
  - Retry policy: up to 2 retries for 5xx and network failures
  - Exponential backoff: 1s, 2s
  - No retries for 4xx errors
  - Log all retry attempts (redacted)
  - _Requirements: 3.5.1_

- [x] 11.2 Implement HTTP error mapping


  - Map 4xx auth/quota errors to `LlmError::ProviderAuth` or `LlmError::ProviderQuota`
  - Map 5xx errors to `LlmError::ProviderOutage`
  - Map network/request timeouts to `LlmError::Timeout`
  - Preserve context for debugging without exposing secrets
  - _Requirements: 3.5.5_

- [x] 11.3 Write unit tests for HTTP error mapping


  - Test 401/403 → `LlmError::ProviderAuth`
  - Test 429 → `LlmError::ProviderQuota`
  - Test 5xx → `LlmError::ProviderOutage`
  - Test network timeout → `LlmError::Timeout`
  - **Property: HTTP errors map to correct LlmError variants**
  - **Validates: Requirements 3.5.5**

- [x] 11.4 Implement HTTP logging with redaction


  - Never log API keys, raw auth headers, or full request bodies
  - Log: provider name, model, region, token counts, high-level error summary
  - Redact HTTP errors before persistence
  - _Requirements: 3.5.6_

- [x] 11.5 Write property test for HTTP logging secrecy


  - **Property: HTTP logging never exposes secrets**
  - **Validates: Requirements 3.5.6**

- [x] 12. Implement OpenRouterBackend





  - Create `OpenRouterBackend` struct using shared HTTP client
  - Implement `LlmBackend` trait for `OpenRouterBackend`
  - Convert `LlmInvocation.messages` to OpenAI-compatible request format
  - Add required headers: `Authorization`, `HTTP-Referer`, `X-Title`
  - POST to `https://openrouter.ai/api/v1/chat/completions`
  - Parse response: extract `choices[0].message.content` and `usage`
  - Populate `LlmResult` with `provider="openrouter"`
  - _Requirements: 3.6.1, 3.6.2, 3.6.3, 3.6.4, 3.6.5_

- [x] 12.1 Implement parameter resolution for HTTP backends

  - `inv.model` overrides `default_model`
  - `inv.metadata["max_tokens"]` overrides `default_params.max_tokens`
  - `inv.metadata["temperature"]` overrides `default_params.temperature`
  - Unspecified values fall back to backend defaults
  - _Requirements: 3.5.1_

- [x] 12.2 Implement credential loading for HTTP providers

  - Load API key from env var specified in config (e.g., `api_key_env = "OPENROUTER_API_KEY"`)
  - Fail with clear error if env var is missing
  - Never log or persist the key
  - _Requirements: 3.5.2_

- [x] 12.3 Update provider selection to support OpenRouter

  - Add `"openrouter"` to supported provider values
  - Implement OpenRouter backend construction
  - _Requirements: 3.2.4_

- [x] 13. Implement BudgetedBackend wrapper





  - Create `BudgetedBackend` struct wrapping `Box<dyn LlmBackend>`
  - Use `Arc<AtomicU32>` for thread-safe budget tracking
  - Increment counter on each `invoke` call (before calling inner backend)
  - Fail fast with `LlmError::BudgetExceeded` if limit reached
  - Track attempted calls, not successful requests
  - _Requirements: 3.6.6_

- [x] 13.1 Write property test for budget enforcement


  - **Property: Budget enforcement fails fast on exhaustion**
  - **Validates: Requirements 3.6.6**

- [x] 13.2 Implement budget configuration


  - Default limit: 20 calls per process
  - Override via `XCHECKER_OPENROUTER_BUDGET` env var
  - Wrap OpenRouter backend with `BudgetedBackend`
  - _Requirements: 5.2_

- [x] 13.3 Record budget exhaustion in receipts


  - Set `budget_exhausted: true` in `llm` block when budget exceeded
  - Add warning to phase entry explaining budget exhaustion
  - _Requirements: 3.6.6, 3.8.5_

- [x] 14. Add doctor checks for HTTP providers




  - Check configured env vars are present
  - Never make HTTP calls by default
  - Report clear status for each HTTP provider
  - _Requirements: 3.5.3_

- [x] 14.1 Write property test for doctor no HTTP calls



  - **Property: Doctor never makes HTTP calls for HTTP providers**
  - **Validates: Requirements 3.5.3**

- [x] 15. Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.
  - **Status: V13 complete - all lib + bin + LLM + doctor + HTTP tests passing**

## V13.1: Budget & Receipt Wiring (Polish)

- [x] 15.1 Wire BudgetedBackend in production





  - Ensure `from_config` wraps OpenRouter in `BudgetedBackend`
  - Add factory test confirming OpenRouter backend is budgeted
  - Test budget exhaustion behavior in production path
  - _Requirements: 3.6.6_

- [x] 15.2 Finalize budget configuration precedence




  - Decide single source of truth: config-only, env-only, or hybrid
  - Implement in `BudgetedBackend` constructor
  - Update `LLM_PROVIDERS.md` and `CONFIGURATION.md` to document precedence
  - Add test for config + env precedence behavior
  - _Requirements: 3.6.6, 5.2_

- [x] 15.3 Wire budget exhaustion into receipts




  - Create `LlmInfo::for_budget_exhaustion()` when `LlmError::BudgetExceeded` occurs
  - Attach to receipt with `budget_exhausted: true`
  - Add warning to phase section explaining exhaustion
  - Add targeted test simulating budget exhaustion and asserting receipt content
  - _Requirements: 3.8.5_

- [x] 15.4 Clean up type visibility





  - Make `LlmError` public to resolve visibility warning with `XCheckerError::Llm`
  - Mark test-only exports (`BudgetedBackend`, `redact_error_message_for_testing`) as `#[doc(hidden)]` or `#[cfg(test)]`
  - Remove or use `HttpClient::last_error` field
  - Remove or assert on `OpenAiResponseMessage::role` field
  - _Requirements: code quality_

- [x] 15.5 Checkpoint: V13.1 polish complete





  - Budget enforcement fully wired in production
  - Receipt integration complete
  - Type surface cleaned up
  - All tests passing

## V14: Anthropic + LLM Metadata

- [x] 16. Implement AnthropicBackend




  - Create `AnthropicBackend` struct using shared HTTP client
  - Implement `LlmBackend` trait for `AnthropicBackend`
  - Convert `LlmInvocation.messages` to Anthropic Messages API format
  - Use `system` field for system prompts
  - Add required headers: `x-api-key`, `anthropic-version`, `content-type`
  - POST to `https://api.anthropic.com/v1/messages`
  - Parse response: extract first text from `content[...]` and `usage`
  - Concatenate multiple text segments if present
  - Populate `LlmResult` with `provider="anthropic"`
  - _Requirements: 3.7.1, 3.7.2, 3.7.3, 3.7.4, 3.7.5_

- [x] 16.1 Update provider selection to support Anthropic


  - Add `"anthropic"` to supported provider values
  - Implement Anthropic backend construction
  - Add `[llm.anthropic]` config struct (base URL, api_key_env, default model)
  - _Requirements: 3.2.4_


- [x] 16.2 Add doctor checks for Anthropic

  - Reuse `check_http_provider_config` pattern
  - Validate API key presence via `api_key_env` or default `ANTHROPIC_API_KEY`
  - Validate model is configured
  - Zero network calls
  - Add 2-3 unit tests in `tests/test_doctor_http_provider_checks.rs`
  - _Requirements: 3.5.3_

- [x] 16.3 Update LLM_PROVIDERS.md for Anthropic


  - Add Anthropic to provider table
  - Document config keys, env vars, doctor behavior
  - Show example configuration
  - _Requirements: FR-LLM-DOCS_

- [x] 17. Implement fallback provider support





  - Add `[llm] fallback_provider` config key
  - Attempt fallback backend construction if primary fails
  - Log warning about fallback usage (redacted)
  - Record fallback usage in receipt warnings
  - Only trigger fallback on construction/validation failure, not runtime errors
  - Ensure runtime errors (timeouts, outages, quota) do not trigger fallback
  - _Requirements: 3.2.6_

- [x] 17.1 Write unit tests for fallback provider usage

  - Test fallback on missing binary
  - Test fallback on missing API key
  - Test no fallback on runtime timeout
  - Test no fallback on provider outage
  - **Property: Fallback provider is used on primary failure**
  - **Validates: Requirements 3.2.6**

- [x] 18. Complete LLM metadata in receipts





  - Ensure all successful invocations populate `provider` and `model_used`
  - Populate token counts when provider returns usage data
  - Set `timed_out` and `timeout_seconds` for timeouts
  - Add warnings for fallbacks with primary failure reason (redacted)
  - _Requirements: 3.8.2, 3.8.3, 3.8.4_


- [x] 18.1 Write unit tests for provider metadata in receipts

  - Test successful invocation records provider and model
  - Test token counts are recorded when available
  - Test timeout metadata is recorded
  - **Property: Successful invocations record provider metadata**
  - **Validates: Requirements 3.8.2**

- [x] 19. Implement provider-specific prompt templates





  - Define template selection mechanism based on provider
  - Fail configuration validation if template incompatible with provider
  - Document template compatibility rules
  - _Requirements: 3.7.6_

- [x] 20. Checkpoint: V11–V14 core LLM backend complete





  - Ensure all tests pass, ask the user if questions arise.
  - **Status: All four providers (claude-cli, gemini-cli, openrouter, anthropic) implemented**
  - **Status: Budget, receipts, doctor checks, and HTTP infrastructure complete**

## V14.1: Test Strategy & CI Foundation

- [x] 20.1 Define test lanes


  - Create `justfile` or `Makefile` with `test-fast` and `test-full` targets
  - `test-fast`: `cargo test --lib --bins`
  - `test-full`: lib + bins + property tests + integration tests
  - Document in `CONTRIBUTING.md` or `docs/TESTING.md`
  - _Requirements: NFR9_

- [x] 20.2 Document test matrix


  - Explain which tests are heavy (property tests, doctor/WSL integrations)
  - Document how to run specific subsets on Windows
  - Document test gating flags for LLM providers
  - Show how to increase property test counts for local runs
  - _Requirements: NFR9_

- [x] 20.3 Add minimal CI workflow


  - Create `.github/workflows/test.yml`
  - Run `test-fast` on every PR
  - Run `test-full` nightly or on protected branch
  - Test on Linux, macOS, Windows
  - _Requirements: 4.6.1_

- [x] 20.4 Checkpoint: Test infrastructure complete


  - Repeatable, low-friction test story established
  - CI provides signal on all platforms
  - Documentation makes system self-explanatory

## V15: Claude Code CLI Surfaces

- [x] 21. Implement `xchecker spec --json` command





  - Add `--json` flag to `spec` command
  - Return JSON with `schema_version`, `spec_id`, `phases`, `config_summary`
  - Exclude full artifacts and packet contents
  - Document JSON schema in `docs/schemas/spec-json.v1.json`
  - _Requirements: 4.1.1_

- [x] 21.1 Write unit tests for spec JSON output


  - Test `schema_version` field is present
  - Test output excludes full packet contents
  - **Property: JSON output includes schema version**
  - **Validates: Requirements 4.1.1**

- [x] 22. Implement `xchecker status --json` command





  - Add `--json` flag to `status` command
  - Return JSON with `schema_version`, `spec_id`, `phase_statuses`, `pending_fixups`, `has_errors`
  - Document JSON schema in `docs/schemas/status-json.v1.json`
  - _Requirements: 4.1.2_

- [x] 23. Implement `xchecker resume --json` command






  - Add `--json` flag to `resume` command
  - Return JSON with `schema_version`, `spec_id`, `phase`, `current_inputs`, `next_steps`
  - Exclude full packet and raw artifacts
  - Document JSON schema in `docs/schemas/resume-json.v1.json`
  - _Requirements: 4.1.3_

- [x] 23.1 Write unit tests for JSON size limits


  - Test spec JSON excludes full artifacts
  - Test status JSON excludes packet contents
  - Test resume JSON excludes raw artifacts
  - **Property: JSON output respects size limits**
  - **Validates: Requirements 4.1.4**

- [x] 24. Document Claude Code integration flows





  - Write example showing `xchecker spec --json` → parse in Claude Code
  - Write example showing `xchecker resume --phase design --json` invocation
  - Document canonical slash command scheme
  - Show how to map JSON into Claude Code's tool invocation model
  - _Requirements: 4.2.1, 4.2.2, 4.2.3_

- [x] 25. Checkpoint - Ensure all tests pass





  - Ensure all tests pass, ask the user if questions arise.

## V16: Workspace Model

- [x] 26. Implement workspace registry





  - Define `workspace.yaml` schema
  - Implement `xchecker project init <name>` command
  - Create `workspace.yaml` in current directory
  - Mark as project root
  - _Requirements: 4.3.1_

- [x] 26.1 Implement workspace discovery


  - Search upward from CWD for `workspace.yaml`
  - Use first found (no merging)
  - Add `--workspace <path>` flag to override discovery
  - _Requirements: 4.3.6_


- [x] 26.2 Write property test for workspace discovery

  - **Property: Workspace discovery searches upward**
  - **Validates: Requirements 4.3.6**

- [x] 27. Implement `xchecker project add-spec` command





  - Add `--tag <tag>` flag (repeatable)
  - Register spec metadata in `workspace.yaml`
  - Fail if same spec added twice without `--force`
  - _Requirements: 4.3.2_

- [x] 28. Implement `xchecker project list` command





  - List all specs with spec id, status (from receipts), tags
  - Derive status from latest receipts
  - _Requirements: 4.3.3_

- [x] 29. Implement `xchecker project status --json` command





  - Emit aggregated status for all specs
  - Include `schema_version`, per-spec phase summaries, counts of failed/pending/stale specs
  - Document JSON schema in `docs/schemas/workspace-status-json.v1.json`
  - _Requirements: 4.3.4_

- [x] 30. Implement `xchecker project history --json` command





  - Emit timeline of phase progression, timestamps, selected metrics
  - Include LLM token usage, fixup counts
  - Document JSON schema in `docs/schemas/workspace-history-json.v1.json`
  - _Requirements: 4.3.5_

- [x] 31. Implement optional TUI for workspace





  - Add `xchecker project tui` command
  - Display specs list with tags and last status
  - Show latest receipt summary per selected spec
  - Show pending fixups, error counts, stale specs
  - Keyboard-only navigation (arrow keys, j/k, Enter, q)
  - Read-only in V16 (no destructive operations)
  - _Requirements: 4.4.1, 4.4.2, 4.4.3_

- [x] 32. Checkpoint - Ensure all tests pass





  - Ensure all tests pass, ask the user if questions arise.

## V17: Gate Command + CI Templates

- [x] 33. Implement gate policy evaluation





  - Create `xchecker gate <spec-id>` command
  - Read latest `status.v1.json` and relevant receipts
  - Implement policy evaluation logic
  - _Requirements: 4.5.1_

- [x] 33.1 Implement gate policy parameters


  - Add `--min-phase <phase>` flag
  - Add `--fail-on-pending-fixups` flag
  - Add `--max-phase-age <duration>` flag
  - Define phase age as wall-clock time since latest successful receipt
  - Failed receipts do not count towards age (prevent flapping phases from appearing fresh)
  - Define default policy: `--min-phase tasks`
  - _Requirements: 4.5.3_


- [x] 33.2 Implement gate exit codes

  - Return 0 on policy success
  - Return 1 on policy violation
  - Return appropriate runtime error codes for config/IO errors
  - _Requirements: 4.5.2_


- [x] 33.3 Write unit tests for gate exit codes

  - Test policy pass returns 0
  - Test policy violation returns 1
  - Test spec with success 10 days ago and failure yesterday is still stale per max-phase-age
  - **Property: Gate returns correct exit codes**
  - **Validates: Requirements 4.5.2**


- [x] 33.4 Implement gate output

  - Human-friendly text to stdout/stderr explaining pass/fail reasons
  - Add `--json` flag for structured output
  - Include decision, evaluated conditions, reasons for failure
  - _Requirements: 4.5.4_

- [x] 34. Create CI templates





  - Create `.github/workflows/xchecker-gate.yml` example
  - Show how to configure `--min-phase`, `--fail-on-pending-fixups`, etc.
  - Demonstrate reading workspace/spec configuration from repo
  - Document GitLab CI snippet in `docs/ci/gitlab.md`
  - Document how to configure required status checks in GitHub/GitLab
  - Wire example smoke tests into existing CI matrix (Linux/macOS/Windows)
  - _Requirements: 4.6.1, 4.6.2, 4.6.3_

- [x] 35. Checkpoint - Ensure all tests pass





  - Ensure all tests pass, ask the user if questions arise.

## V18: Templates, Hooks, Showcase

- [x] 36. Implement spec templates





  - Create `xchecker template list` command
  - List built-in templates: `fullstack-nextjs`, `rust-microservice`, `python-fastapi`, `docs-refactor`
  - _Requirements: 4.7.1_

- [x] 36.1 Implement template initialization


  - Create `xchecker template init <template> <spec-id>` command
  - Seed problem statement, minimal `.xchecker/config.toml`, example partial spec flow
  - Create README for each template describing intended use, prerequisites, basic flow
  - _Requirements: 4.7.2, 4.7.3_

- [x] 37. Implement hooks system








  - Define hooks configuration format in `hooks.toml` or `.xchecker/config.toml`
  - Support `[hooks.pre_phase.<phase>]` and `[hooks.post_phase.<phase>]` sections
  - Support `command` and `on_fail` fields
  - _Requirements: 4.8.1_

- [x] 37.1 Implement hook execution


  - Execute hooks with relevant context via env vars (`XCHECKER_SPEC_ID`, `XCHECKER_PHASE`)
  - Optionally pass small JSON payload on stdin
  - Run in project root directory
  - _Requirements: 4.8.2_


- [x] 37.2 Implement hook failure handling

  - Default: non-blocking (`on_fail = "warn"`) - log and record in receipts
  - If `on_fail = "fail"`, non-zero hook exit code fails the phase
  - _Requirements: 4.8.3_


- [x] 37.3 Write unit tests for hook failure handling

  - Test `on_fail = "warn"` continues with warning
  - Test `on_fail = "fail"` fails the phase
  - **Property: Hook failures respect on_fail configuration**
  - **Validates: Requirements 4.8.3**


- [x] 37.4 Implement hook timeouts

  - Default timeout: 60s
  - Terminate hook if timeout exceeded
  - Handle according to `on_fail` configuration
  - _Requirements: 4.8.4_


- [x] 37.5 Write property test for hook timeouts

  - **Property: Hooks are subject to timeouts**
  - **Validates: Requirements 4.8.4**

- [x] 38. Create showcase examples





  - Create `examples/fullstack-nextjs/` with working scenario and README
  - Create `examples/mono-repo/` demonstrating multiple specs under one workspace
  - Add test scripts to exercise examples in CI
  - _Requirements: 4.9.1, 4.9.2_

- [x] 39. Write walkthroughs





  - Write "Running xchecker on your repo in 20 minutes" walkthrough
  - Write "From spec to PR: xchecker + Claude Code flow" walkthrough
  - Ensure walkthroughs are runnable using code and config in repo
  - Extract and test code snippets in CI
  - _Requirements: 4.9.3, 4.9.4_

- [x] 40. Configure property-based tests for CI





  - Set reasonable max test count (e.g., 64) and shrink limits
  - Ensure PBT suites run in reasonable time on CI hardware
  - Document how to increase test counts for local runs (env flags)
  - _Requirements: NFR9_

- [x] 41. Final checkpoint: V15–V18 ecosystem complete





  - Ensure all tests pass, ask the user if questions arise.

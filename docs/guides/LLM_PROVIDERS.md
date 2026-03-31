# LLM Provider Configuration and Usage

This document describes xchecker's LLM provider system, including configuration, authentication, testing, and cost control for all supported providers.

## Overview

xchecker uses a provider-agnostic LLM backend abstraction that supports multiple language model providers through a unified interface. All LLM invocations go through the `LlmBackend` trait, which handles:

- **Message-based conversations**: Canonical `messages` array (role/content) works across all providers
- **Timeout enforcement**: All invocations have mandatory timeouts with graceful termination
- **Error mapping**: Provider-specific errors mapped to standardized error types
- **Metadata tracking**: Provider, model, and token usage recorded in receipts
- **Security**: Automatic secret redaction and credential protection

## Supported Providers (by Version)

| Provider | Type | Status |
|----------|------|--------|
| **Claude CLI** | CLI | ✅ Supported |
| **Gemini CLI** | CLI | ✅ Supported |
| **OpenRouter** | HTTP | ✅ Supported |
| **Anthropic API** | HTTP | ✅ Supported |

## Execution Strategy

xchecker supports two execution strategies:

| Strategy | Description | Status |
|----------|-------------|--------|
| **Controlled** | LLMs propose changes via structured output; all writes go through xchecker's fixup pipeline | ✅ Supported |
| **ExternalTool** | LLMs can directly write files and invoke tools (agentic workflows) | 🚧 Planned (V15+) |

**Note**: Only `controlled` execution strategy is currently supported. This ensures all file modifications go through xchecker's validated fixup system with atomic writes and security checks.

---

## Prompt Templates

xchecker supports provider-specific prompt templates to optimize LLM interactions for different providers. Templates define how prompts are structured and formatted.

### Available Templates

| Template | Description | Compatible Providers |
|----------|-------------|---------------------|
| **default** | Universal template compatible with all providers | claude-cli, gemini-cli, openrouter, anthropic |
| **claude-optimized** | Optimized for Claude with XML tags and system prompts | claude-cli, anthropic |
| **openai-compatible** | Optimized for OpenAI-style message formatting | openrouter, gemini-cli |

### Configuration

```toml
[llm]
provider = "claude-cli"
prompt_template = "default"  # Optional, defaults to "default"
```

**CLI override:**
```bash
xchecker spec my-feature --prompt-template claude-optimized
```

**Template aliases** (case-insensitive):
- `default`: Universal template
- `claude-optimized`, `claude_optimized`, `claude`: Claude-optimized template
- `openai-compatible`, `openai_compatible`, `openai`, `openrouter`: OpenAI-compatible template

### Compatibility Rules

**⚠️ Important**: If a prompt template is incompatible with the selected provider, xchecker fails during configuration validation. There is no "best effort" adaptation—explicit failure prevents silent misbehavior.

**Compatible combinations**:
- `default` + any provider ✅
- `claude-optimized` + `claude-cli` ✅
- `claude-optimized` + `anthropic` ✅
- `openai-compatible` + `openrouter` ✅
- `openai-compatible` + `gemini-cli` ✅

**Incompatible combinations** (will fail validation):
- `claude-optimized` + `openrouter` ❌
- `claude-optimized` + `gemini-cli` ❌
- `openai-compatible` + `claude-cli` ❌
- `openai-compatible` + `anthropic` ❌

### Example Error

```bash
# Incompatible template and provider
xchecker spec my-feature --llm-provider openrouter

# With config:
# [llm]
# provider = "openrouter"
# prompt_template = "claude-optimized"

# Error: Prompt template 'claude-optimized' is not compatible with provider 'openrouter'.
# This template uses Claude-specific formatting (XML tags, system prompts)
# that may not work correctly with other providers.
# Compatible providers: claude-cli, anthropic.
# Use 'default' template for cross-provider compatibility.
```

### Best Practices

1. **Use `default` for cross-provider compatibility**: If you might switch providers, use the default template
2. **Use provider-specific templates for optimization**: If you're committed to a specific provider, use its optimized template
3. **Test template changes**: After changing templates, verify output quality with a test spec

---

## Provider: Claude CLI

**Type**: CLI (Command-Line Interface)
**Status**: ✅ Fully Supported

### Overview

Claude CLI is the official command-line interface for Anthropic's Claude models. xchecker invokes Claude CLI as a subprocess and parses its output.

### Installation

```bash
# Install Claude CLI (see https://claude.ai/download)
# Verify installation
claude --version

# Authenticate
claude auth login
```

### Configuration

**Minimal (uses defaults):**
```toml
# .xchecker/config.toml
[llm]
provider = "claude-cli"  # Can be omitted (default)
execution_strategy = "controlled"  # Can be omitted (default)
```

**With custom binary path:**
```toml
[llm]
provider = "claude-cli"

[llm.claude]
binary = "/usr/local/bin/claude"  # Optional: custom path
```

**CLI Flags:**
```bash
# Override provider
xchecker spec my-feature --llm-provider claude-cli

# Override execution strategy
xchecker spec my-feature --execution-strategy controlled

# Set fallback provider
xchecker spec my-feature --llm-fallback-provider openrouter

# Set prompt template
xchecker spec my-feature --prompt-template claude-optimized
```

**Environment Variables:**
```bash
# Override provider
export XCHECKER_LLM_PROVIDER=claude-cli

# Override execution strategy
export XCHECKER_EXECUTION_STRATEGY=controlled

# Set fallback provider
export XCHECKER_LLM_FALLBACK_PROVIDER=openrouter

# Set prompt template
export XCHECKER_LLM_PROMPT_TEMPLATE=claude-optimized

xchecker spec my-feature
```

**Precedence**: CLI flags > environment variables > config file > defaults

### Authentication

Claude CLI handles authentication independently. xchecker never reads, logs, or modifies authentication credentials.

**Setup:**
```bash
# Authenticate with Claude CLI
claude auth login

# Verify authentication
claude auth status
```

**Environment**: Claude CLI may use environment variables for authentication (e.g., `ANTHROPIC_API_KEY`). xchecker inherits the process environment but never inspects or logs these values.

### Binary Discovery

xchecker discovers the Claude CLI binary using the following precedence:

1. **Config file**: `[llm.claude] binary = "/path/to/claude"`
2. **PATH search**: Searches system PATH for `claude` executable
3. **Error**: If not found, reports clear error with checked locations

**Verification:**
```bash
# Check Claude CLI availability
xchecker doctor

# Expected output:
# ✓ Claude CLI available (native)
# ✓ Claude CLI version: 0.8.1
```

### Output Format

Claude CLI uses NDJSON (newline-delimited JSON) output format with `last_valid_json_wins` semantics:

- **stdout**: Parsed as NDJSON; last valid JSON object is used as response
- **stderr**: Captured into ring buffer (≤ 256 KiB), redacted, and logged
- **Exit codes**: Non-zero exit codes mapped to `LlmError::Transport`

### Receipt Metadata

Successful Claude CLI invocations record the following in receipts:

```json
{
  "llm": {
    "provider": "claude-cli",
    "model_used": "sonnet",
    "tokens_input": 1024,
    "tokens_output": 512,
    "timed_out": false,
    "timeout_seconds": 600
  }
}
```

---

## Provider: Gemini CLI

**Type**: CLI (Command-Line Interface)
**Status**: ✅ Fully Supported

### Overview

Gemini CLI is Google's command-line interface for Gemini models.

### Configuration

```toml
[llm]
provider = "gemini-cli"

[llm.gemini]
binary = "/usr/local/bin/gemini"  # Optional
default_model = "gemini-2.0-flash-lite"

# Optional: Named profiles for per-phase model selection
[llm.gemini.profiles.requirements]
model = "gemini-2.0-flash-lite"
max_tokens = 1024

[llm.gemini.profiles.design]
model = "gemini-2.0-pro"
max_tokens = 2048
```

**CLI Flags:**
```bash
xchecker spec my-feature --llm-provider gemini-cli
xchecker spec my-feature --llm-gemini-binary /usr/local/bin/gemini
xchecker spec my-feature --llm-gemini-default-model gemini-2.0-pro
```

**Environment Variables:**
```bash
export XCHECKER_LLM_GEMINI_DEFAULT_MODEL=gemini-2.0-pro
```

### Authentication

Gemini CLI uses environment variables for authentication:

```bash
export GEMINI_API_KEY=your_api_key_here
```

xchecker will never read, log, or persist the API key.

### Output Format

- **Invocation**: `gemini -p "<prompt>" --model <model>`
- **stdout**: Treated as opaque text → `raw_response`
- **stderr**: Captured into ring buffer (≤ 2 KiB), redacted, and logged

### Doctor Checks

```bash
xchecker doctor

# Will check:
# - Binary resolution
# - Version detection (using `gemini -h`)
# - No LLM completion requests
```

---

## Provider: OpenRouter

**Type**: HTTP API
**Status**: ✅ Fully Supported

### Overview

OpenRouter is a unified API for accessing multiple LLM providers through a single endpoint.

### Configuration

```toml
[llm]
provider = "openrouter"

[llm.openrouter]
base_url = "https://openrouter.ai/api/v1/chat/completions"  # Optional
api_key_env = "OPENROUTER_API_KEY"  # Required
model = "google/gemini-2.0-flash-lite"  # Required
max_tokens = 2048  # Optional
temperature = 0.2  # Optional
```

### Authentication

OpenRouter uses API keys from environment variables:

```bash
export OPENROUTER_API_KEY=your_api_key_here
```

**Security**:
- xchecker loads the key from the specified env var
- The key will never be logged or persisted
- HTTP requests will include `Authorization: Bearer $OPENROUTER_API_KEY`

### Request Format

OpenRouter uses OpenAI-compatible request format:

```json
{
  "model": "google/gemini-2.0-flash-lite",
  "messages": [
    {"role": "system", "content": "..."},
    {"role": "user", "content": "..."}
  ],
  "stream": false,
  "max_tokens": 2048,
  "temperature": 0.2
}
```

**Required Headers**:
- `Authorization: Bearer $OPENROUTER_API_KEY`
- `HTTP-Referer: https://effortlesssteven.com/xchecker`
- `X-Title: xchecker`

### Response Format

OpenRouter returns OpenAI-compatible responses:

```json
{
  "choices": [
    {
      "message": {
        "content": "..."
      }
    }
  ],
  "usage": {
    "prompt_tokens": 1024,
    "completion_tokens": 512
  }
}
```

xchecker extracts `choices[0].message.content` and `usage` for receipt metadata.

### Budget Control

OpenRouter supports built-in budget limits:

- **Default limit**: 20 calls per xchecker process
- **Configuration precedence** (highest to lowest):
  1. Environment variable: `XCHECKER_OPENROUTER_BUDGET=50`
  2. Config file: `[llm.openrouter] budget = 50`
  3. Default: 20 calls
- **Enforcement**: Fail fast with `LlmError::BudgetExceeded` when limit reached
- **Tracking**: Counts attempted calls, not successful requests (prevents retry loops)

**Configuration examples**:

```toml
# In .xchecker/config.toml
[llm.openrouter]
budget = 50  # Set budget in config file
```

```bash
# Override via environment variable (takes precedence over config)
export XCHECKER_OPENROUTER_BUDGET=100
xchecker spec my-feature

# Or inline
XCHECKER_OPENROUTER_BUDGET=30 xchecker spec my-feature
```

**Budget exhaustion**:
```json
{
  "llm": {
    "provider": "openrouter",
    "budget_exhausted": true
  },
  "warnings": [
    {
      "type": "budget_exhausted",
      "message": "OpenRouter budget exhausted: 20/20 calls used"
    }
  ]
}
```

### Timeout and Retry

- **Timeout**: `min(inv.timeout, global_max_http_timeout)` (default global max: 300s)
- **Retry policy**: Up to 2 retries for 5xx errors and network failures
- **Backoff**: Exponential backoff (1s, 2s)
- **No retry**: 4xx errors (auth, quota) are not retried

### Error Mapping

| HTTP Status | LlmError Variant | Exit Code | Description |
|-------------|------------------|-----------|-------------|
| 401, 403 | `ProviderAuth` | 70 | Authentication failure |
| 429 | `ProviderQuota` | 70 | Rate limit or quota exceeded |
| 5xx | `ProviderOutage` | 70 | Provider server error |
| Timeout | `Timeout` | 10 | Request timeout |
| Network error | `Transport` | 70 | Network connectivity issue |

### Doctor Checks

```bash
xchecker doctor

# Will check:
# - API key env var present (not the value)
# - No HTTP calls by default
# - Optional: `--llm-online` flag for live connectivity check
```

---

## Provider: Anthropic API

**Type**: HTTP API
**Status**: ✅ Fully Supported

### Overview

Anthropic API is the official HTTP API for Claude models, providing direct access to Claude 3.5 Sonnet and other models through Anthropic's Messages API.

### Configuration

**Minimal (required fields):**
```toml
[llm]
provider = "anthropic"

[llm.anthropic]
model = "sonnet"  # Required
```

**Full configuration:**
```toml
[llm]
provider = "anthropic"

[llm.anthropic]
base_url = "https://api.anthropic.com/v1/messages"  # Optional (default shown)
api_key_env = "ANTHROPIC_API_KEY"  # Optional (default shown)
model = "sonnet"  # Required
max_tokens = 2048  # Optional (default: 2048)
temperature = 0.2  # Optional (default: 0.2)
```

**CLI Flags:**
```bash
# Override provider
xchecker spec my-feature --llm-provider anthropic
```

**Environment Variables:**
```bash
# Override provider
export XCHECKER_LLM_PROVIDER=anthropic

xchecker spec my-feature
```

**Precedence**: CLI flags > environment variables > config file > defaults

### Authentication

Anthropic API uses API keys from environment variables:

```bash
export ANTHROPIC_API_KEY=your_api_key_here
```

**Security**:
- xchecker loads the key from the specified env var (default: `ANTHROPIC_API_KEY`)
- The key is never logged or persisted
- HTTP requests include `x-api-key: $ANTHROPIC_API_KEY`
- All error messages are redacted before logging

### Request Format

Anthropic uses its own Messages API format:

```json
{
  "model": "sonnet",
  "system": "You are a helpful assistant",
  "messages": [
    {"role": "user", "content": "..."},
    {"role": "assistant", "content": "..."}
  ],
  "max_tokens": 2048,
  "temperature": 0.2
}
```

**Required Headers**:
- `x-api-key: $ANTHROPIC_API_KEY`
- `anthropic-version: 2023-06-01`
- `content-type: application/json`

**Message Conversion**:
- System messages are extracted and placed in the `system` field
- Multiple system messages are concatenated with `\n\n`
- User and assistant messages are placed in the `messages` array

### Response Format

Anthropic returns Messages API responses:

```json
{
  "content": [
    {
      "type": "text",
      "text": "..."
    }
  ],
  "usage": {
    "input_tokens": 1024,
    "output_tokens": 512
  }
}
```

xchecker extracts the first text segment from `content[...]` and `usage` for receipt metadata. If multiple content blocks exist, text segments are concatenated in order.

### Timeout and Retry

Same as OpenRouter:
- **Timeout**: `min(inv.timeout, global_max_http_timeout)` (default global max: 300s)
- **Retry policy**: Up to 2 retries for 5xx errors and network failures
- **Backoff**: Exponential backoff (1s, 2s)
- **No retry**: 4xx errors (auth, quota) are not retried

### Error Mapping

Same as OpenRouter:

| HTTP Status | LlmError Variant | Exit Code | Description |
|-------------|------------------|-----------|-------------|
| 401, 403 | `ProviderAuth` | 70 | Authentication failure |
| 429 | `ProviderQuota` | 70 | Rate limit or quota exceeded |
| 5xx | `ProviderOutage` | 70 | Provider server error |
| Timeout | `Timeout` | 10 | Request timeout |
| Network error | `Transport` | 70 | Network connectivity issue |

### Doctor Checks

```bash
xchecker doctor

# Checks performed:
# - API key env var present (not the value)
# - Model configured in [llm.anthropic]
# - No HTTP calls by default
# - Optional: `--llm-online` flag for live connectivity check
```

### Receipt Metadata

Successful Anthropic API invocations record the following in receipts:

```json
{
  "llm": {
    "provider": "anthropic",
    "model_used": "sonnet",
    "tokens_input": 1024,
    "tokens_output": 512,
    "timed_out": false
  }
}
```

### Cost Control

- **No automatic budgets**: Anthropic API does not have built-in budget limits in xchecker (unlike OpenRouter)
- **Manual control**: Use Anthropic's own usage tracking and limits
- **Test isolation**: Tests use mocked responses by default

### Example Configuration

**Basic setup:**
```toml
[llm]
provider = "anthropic"

[llm.anthropic]
model = "sonnet"
```

**With custom settings:**
```toml
[llm]
provider = "anthropic"

[llm.anthropic]
model = "sonnet"
max_tokens = 4096
temperature = 0.3
api_key_env = "MY_ANTHROPIC_KEY"  # Custom env var name
```

---

## Provider Fallback

**Status**: ✅ Supported

### Overview

xchecker supports fallback providers for resilience. If the primary provider fails during construction or validation, xchecker will attempt to use a configured fallback.

### Configuration

```toml
[llm]
provider = "claude-cli"
fallback_provider = "openrouter"  # Optional
```

### Fallback Behavior

**Triggers fallback**:
- Missing binary (CLI providers)
- Missing API key (HTTP providers)
- Invalid configuration
- Construction/validation failures

**Does NOT trigger fallback**:
- Runtime timeouts
- Provider outages (5xx errors)
- Quota exhaustion (429 errors)
- Budget exhaustion

**Rationale**: Runtime errors should fail the run to prevent silent cost/compliance issues (e.g., "OpenRouter is down, silently use Anthropic").

### Receipt Recording

Fallback usage will be recorded in receipt warnings:

```json
{
  "warnings": [
    "llm_fallback: Primary provider 'claude-cli' failed: binary not found. Using fallback 'openrouter'."
  ]
}
```

---

## Common Configuration Patterns

### Development (Fast Iteration)

```toml
[llm]
provider = "claude-cli"
execution_strategy = "controlled"

[defaults]
model = "haiku"  # Faster, cheaper model
phase_timeout = 300  # Shorter timeout for quick feedback
```

### Production (Best Quality)

```toml
[llm]
provider = "claude-cli"
execution_strategy = "controlled"

[defaults]
model = "sonnet"  # Best quality model
phase_timeout = 600  # Full timeout for thorough exploration
```

### CI/CD (Reliable, Cost-Controlled)

```toml
[llm]
provider = "claude-cli"
execution_strategy = "controlled"

[defaults]
model = "sonnet"
phase_timeout = 900  # Longer timeout for CI reliability
```

**Environment variables for CI**:
```bash
# Skip real LLM tests in CI
export XCHECKER_SKIP_LLM_TESTS=1

# Use dry-run mode for validation
xchecker spec my-feature --dry-run
```

---

## Testing and Cost Control

### Test Gating Flags

xchecker provides environment variables to control LLM test execution:

| Flag | Default | Description |
|------|---------|-------------|
| `XCHECKER_SKIP_LLM_TESTS` | `0` | Skip all tests that would call real LLMs |
| `XCHECKER_REAL_LLM_TESTS` | `0` | Enable real LLM tests (opt-in, incurs costs) |

**Examples**:
```bash
# Skip real LLM tests (recommended for CI)
XCHECKER_SKIP_LLM_TESTS=1 cargo test

# Enable real LLM tests (local validation only)
XCHECKER_REAL_LLM_TESTS=1 cargo test

# Default: dry-run mode and mocked responses
cargo test
```

### Doctor Behavior

`xchecker doctor` performs health checks without making LLM calls:

**CLI Providers**:
- ✅ Check binary resolution
- ✅ Check version (optional, non-fatal)
- ✅ Verify binary can be spawned
- ❌ Never send LLM completion requests

**HTTP Providers**:
- ✅ Check API key env var present
- ❌ Never make HTTP calls by default
- ⚠️ Optional `--llm-online` flag for live connectivity check

### Cost Control Best Practices

1. **Use dry-run mode**: `xchecker spec my-feature --dry-run`
2. **Skip LLM tests in CI**: `XCHECKER_SKIP_LLM_TESTS=1`
3. **Use budget limits**: `XCHECKER_OPENROUTER_BUDGET=10`
4. **Monitor receipts**: Check token usage in `.xchecker/specs/<spec-id>/receipts/`
5. **Use cheaper models for development**: `haiku`

---

## Troubleshooting

### "Claude CLI not found" Error

```bash
xchecker spec my-feature
# Error: Claude CLI binary not found in PATH
```

**Solution**:
1. Install Claude CLI: https://claude.ai/download
2. Verify installation: `claude --version`
3. Or specify custom path in config:
   ```toml
   [llm.claude]
   binary = "/usr/local/bin/claude"
   ```

### "Authentication failed" Error

```bash
xchecker spec my-feature
# Error: Claude CLI authentication failed
```

**Solution**:
1. Authenticate: `claude auth login`
2. Verify: `claude auth status`

### "Timeout exceeded" Error

```bash
xchecker spec my-feature
# Error: Phase 'requirements' exceeded 600s timeout
```

**Solution**:
1. Increase timeout: `xchecker spec my-feature --phase-timeout 1200`
2. Or configure in `.xchecker/config.toml`:
   ```toml
   [defaults]
   phase_timeout = 1200
   ```

### "Budget exhausted" Error

```bash
xchecker spec my-feature
# Error: OpenRouter budget exhausted: 20/20 calls used
```

**Solution**:
1. Increase budget: `XCHECKER_OPENROUTER_BUDGET=50 xchecker spec my-feature`
2. Or wait for process to complete (budget resets per process)

---

## See Also

- [Configuration Guide](CONFIGURATION.md) - Complete configuration reference
- [Orchestrator Documentation](ORCHESTRATOR.md) - LLM layer architecture
- [Doctor Command](DOCTOR.md) - Health check details
- [Exit Codes](../README.md#exit-codes) - Error code reference
- [Security](SECURITY.md) - Secret detection and redaction

---

## Version History

| Version | Changes |
|---------|---------|
| V11 | Initial LLM provider system with Claude CLI support |
| V12 | Gemini CLI support |
| V13 | OpenRouter HTTP support with budget control |
| V14 | Anthropic API support and fallback providers |
| V15+ | ExternalTool execution strategy (planned) |

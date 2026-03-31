# Configuration Guide

This guide walks you through configuring xchecker for common scenarios. For a
complete reference of every config key, environment variable, and CLI flag, see
[Configuration Reference](../reference/CONFIGURATION.md).

---

## Where xchecker looks for configuration

xchecker resolves settings in this order (highest priority first):

1. **CLI flags** -- `--model sonnet`, `--phase-timeout 1200`, etc.
2. **Environment variables** -- `XCHECKER_LLM_PROVIDER`, `XCHECKER_OPENROUTER_BUDGET`, etc.
3. **Configuration file** -- `.xchecker/config.toml`
4. **Built-in defaults**

### Config file discovery

xchecker searches upward from the current working directory for
`.xchecker/config.toml`. The search stops at the filesystem root or a Git
repository root (`.git` directory). Override with `--config <path>`.

### XCHECKER_HOME

By default, xchecker stores all state in `./.xchecker` relative to your working
directory. Override this with the `XCHECKER_HOME` environment variable:

```bash
# Isolate state per CI build
export XCHECKER_HOME=/tmp/xchecker-build-${BUILD_ID}

# Or inline for a single command
XCHECKER_HOME=/tmp/test xchecker status my-feature
```

The state directory structure:

```
.xchecker/
  config.toml              # Configuration file (optional)
  specs/<spec-id>/
    artifacts/             # Generated phase outputs
    receipts/              # Execution audit trails
    context/               # Packet previews for debugging
```

---

## Basic setup

Create `.xchecker/config.toml` with a provider and model:

```toml
[llm]
provider = "claude-cli"

[defaults]
model = "sonnet"
```

That is enough to run `xchecker spec my-feature`. Everything else has sensible
defaults.

Verify your setup:

```bash
xchecker doctor
```

---

## File selectors

Selectors control which files are gathered into the context packet sent to the
LLM. Use glob patterns:

```toml
[selectors]
include = [
    "src/**/*.rs",
    "docs/**/*.md",
    "Cargo.toml",
]

exclude = [
    "target/**",
    "node_modules/**",
    ".git/**",
    "*.log",
]
```

### Pattern syntax

| Pattern | Meaning |
|---------|---------|
| `*` | Any characters except `/` |
| `**` | Any characters including `/` (recursive) |
| `?` | Any single character |
| `[abc]` | Any character in the set |
| `{a,b}` | Either `a` or `b` |

### Tips

- Prefer specific `include` patterns over broad exclusions.
- If you hit a "packet overflow" error, narrow your includes or increase
  `packet_max_bytes`:

```bash
xchecker spec my-feature --packet-max-bytes 131072
```

---

## LLM provider configuration

xchecker supports four providers. Set the provider in config or via CLI flag:

```toml
[llm]
provider = "claude-cli"   # or "gemini-cli", "openrouter", "anthropic"
```

### Claude CLI (default)

No additional config needed if `claude` is on your PATH:

```toml
[llm]
provider = "claude-cli"

# Optional: custom binary path
[llm.claude]
binary = "/usr/local/bin/claude"
```

### Gemini CLI

```toml
[llm]
provider = "gemini-cli"

[llm.gemini]
default_model = "gemini-2.0-flash-lite"
```

Requires `GEMINI_API_KEY` in your environment.

### OpenRouter

```toml
[llm]
provider = "openrouter"

[llm.openrouter]
model = "google/gemini-2.0-flash-lite"
max_tokens = 2048
temperature = 0.2
budget = 50
```

Requires `OPENROUTER_API_KEY` in your environment.

### Anthropic API

```toml
[llm]
provider = "anthropic"

[llm.anthropic]
model = "sonnet"
```

Requires `ANTHROPIC_API_KEY` in your environment.

For detailed provider documentation (authentication, request formats, error
handling, prompt templates), see the
[LLM Providers guide](LLM_PROVIDERS.md).

---

## Per-phase overrides

Override model, max turns, or timeout for specific phases:

```toml
[phases.requirements]
model = "haiku"            # Cheaper model for requirements

[phases.design]
model = "sonnet"           # Better model for design
max_turns = 8
phase_timeout = 900
```

Phase keys: `requirements`, `design`, `tasks`, `review`, `fixup`, `final`.

---

## Phase timeouts

The default phase timeout is 600 seconds (10 minutes). Increase it for
complex specs or slower providers:

```toml
[defaults]
phase_timeout = 1200       # 20 minutes
```

Or override per run:

```bash
xchecker resume my-feature --phase design --phase-timeout 1200
```

The minimum timeout is 5 seconds.

---

## Hooks

Run scripts before or after any phase. Hooks receive context through environment
variables (`XCHECKER_SPEC_ID`, `XCHECKER_PHASE`, `XCHECKER_HOOK_TYPE`) and a
JSON payload on stdin.

```toml
[hooks.pre_phase.design]
command = "./scripts/pre_design.sh"
on_fail = "warn"          # "warn" (continue) or "fail" (abort phase)
timeout = 60

[hooks.post_phase.requirements]
command = "./scripts/post_requirements.sh"
on_fail = "fail"
```

Hooks run from the invocation working directory via the platform shell (`sh -c`
on Unix, `cmd /C` on Windows).

---

## Security settings

xchecker scans every packet for secrets before sending it to an LLM. The
built-in patterns cover AWS, GCP, Azure, SSH keys, platform tokens, and more.

### Add custom patterns

```toml
[security]
extra_secret_patterns = [
    "INTERNAL_TOKEN_[A-Z0-9]{32}",
    "my-company-secret-prefix-.*",
]
```

Or via CLI:

```bash
xchecker spec my-feature --extra-secret-pattern "SECRET_[A-Z0-9]{32}"
```

### Suppress false positives

```toml
[security]
ignore_secret_patterns = ["ghp_"]
```

---

## Strict validation

When enabled, phase outputs must pass quality checks (minimum length, required
sections, no meta-summaries). Failures abort the phase instead of logging
warnings.

```toml
[defaults]
strict_validation = true
```

Override per run:

```bash
xchecker spec my-feature --strict-validation
xchecker spec my-feature --no-strict-validation
```

Applies to generative phases: Requirements, Design, Tasks.

---

## Fallback providers

Configure a fallback provider that activates if the primary fails to
initialize (missing binary, missing API key, etc.):

```toml
[llm]
provider = "claude-cli"
fallback_provider = "openrouter"
```

Fallback triggers only on construction failures, not on runtime errors like
timeouts or quota exhaustion.

---

## Common recipes

### Use OpenRouter with budget control

```toml
[llm]
provider = "openrouter"

[llm.openrouter]
model = "google/gemini-2.0-flash-lite"
budget = 30
```

Override budget per run:

```bash
XCHECKER_OPENROUTER_BUDGET=100 xchecker spec my-feature
```

### Development configuration (fast iteration)

```toml
[defaults]
model = "haiku"
packet_max_bytes = 32768
max_turns = 3
phase_timeout = 300

[selectors]
include = ["src/**/*.rs", "Cargo.toml", "README.md"]
exclude = ["target/**", "tests/**"]
```

### Production configuration (best quality)

```toml
[defaults]
model = "sonnet"
packet_max_bytes = 65536
max_turns = 6

[selectors]
include = [
    "src/**/*.rs",
    "tests/**/*.rs",
    "docs/**/*.md",
    "Cargo.toml",
    "*.yaml",
]
exclude = ["target/**", ".git/**"]
```

### CI/CD configuration

```toml
[defaults]
runner_mode = "native"
phase_timeout = 900

[selectors]
include = [".github/**/*.yml", "Cargo.toml", "README.md"]
```

```bash
# Skip LLM calls in CI for validation-only runs
xchecker spec my-feature --dry-run
```

---

## Troubleshooting

### Configuration not found

```
Error: Failed to load configuration
```

Create `.xchecker/config.toml` or pass `--config /path/to/config.toml`.

### Invalid TOML syntax

```
Error: Failed to parse TOML config file
Caused by: TOML parse error at line 5, column 12
```

Check quoting, bracket nesting, and string escaping in your TOML file.

### Packet overflow

Your context is too large. Either narrow your file selectors or increase the
limit:

```bash
xchecker spec my-feature --packet-max-bytes 131072
```

### WSL not available

```
Error: WSL runner requested but not available
```

Install WSL with `wsl --install`, or switch to native runner:

```toml
[runner]
mode = "native"
```

### Phase timeout

```
Error: Phase 'design' exceeded 600s timeout
```

Increase the timeout:

```bash
xchecker resume my-feature --phase design --phase-timeout 1200
```

---

## See also

- [Configuration Reference](../reference/CONFIGURATION.md) -- every key, env var, and CLI flag
- [LLM Providers](LLM_PROVIDERS.md) -- detailed provider setup
- [CI Setup](CI_SETUP.md) -- xchecker in CI/CD pipelines
- [Debugging](DEBUGGING.md) -- diagnostics and troubleshooting

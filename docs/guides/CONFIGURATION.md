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

The resolution order (highest priority first):

1. **Thread-local override** -- `with_isolated_home()` for test isolation
2. **`XCHECKER_HOME` environment variable** -- explicit directory override
3. **Default** -- `./.xchecker` relative to working directory

The state directory structure:

```
.xchecker/
  config.toml              # Configuration file (optional)
  specs/<spec-id>/
    artifacts/             # Generated phase outputs
    receipts/              # Execution audit trails
    context/               # Packet previews for debugging
```

For test isolation, `with_isolated_home()` sets a thread-local override that
takes precedence over the environment variable, avoiding process-global
`set_var` races in parallel tests.

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

Hooks let you run custom shell scripts before and after any phase in the
pipeline. Use them for linting, notifications, custom validation, or any
side-effect you need tied to the spec lifecycle.

### Hook points

There are two hook points per phase:

| Hook point | When it runs | Can abort the phase? |
|------------|-------------|---------------------|
| `pre_phase` | Before the LLM is invoked | Yes (when `on_fail = "fail"`) |
| `post_phase` | After artifacts and receipt are written | No (always treated as warning) |

Both hook points are available for every phase: `requirements`, `design`,
`tasks`, `review`, `fixup`, and `final`.

### Configuration syntax

Each hook is defined as a TOML table under `[hooks.pre_phase.<phase>]` or
`[hooks.post_phase.<phase>]`:

```toml
[hooks.pre_phase.design]
command = "./scripts/pre_design.sh"
on_fail = "warn"          # "warn" (default) or "fail"
timeout = 60              # seconds, default 60

[hooks.post_phase.requirements]
command = "./scripts/post_requirements.sh"
on_fail = "fail"
timeout = 30
```

| Key | Required | Default | Description |
|-----|----------|---------|-------------|
| `command` | Yes | -- | Shell command to execute |
| `on_fail` | No | `"warn"` | `"warn"`: log and continue; `"fail"`: abort the phase |
| `timeout` | No | `60` | Maximum execution time in seconds |

You can configure multiple hooks across different phases in the same config
file. Each phase supports at most one `pre_phase` hook and one `post_phase`
hook.

### How hooks receive context

Hooks receive context in two ways:

**Environment variables** -- always set for every hook invocation:

| Variable | Description | Example value |
|----------|-------------|---------------|
| `XCHECKER_SPEC_ID` | The spec identifier | `my-feature` |
| `XCHECKER_PHASE` | The phase name | `requirements`, `design`, `tasks`, etc. |
| `XCHECKER_HOOK_TYPE` | The hook point | `pre_phase` or `post_phase` |

**JSON payload on stdin** -- a JSON object with the same context:

```json
{"spec_id":"my-feature","phase":"design","hook_type":"pre_phase"}
```

If the hook does not read stdin, the payload is silently discarded. Hooks are
free to ignore stdin and rely entirely on environment variables.

### Execution environment

- Hooks run via the platform shell: `sh -c` on Unix, `cmd /C` on Windows.
- The working directory is the directory where `xchecker` was invoked (not the
  spec directory), so relative paths like `./scripts/...` work naturally.
- Stdout and stderr are captured (truncated to 2 KiB each).
- Hooks are subject to their configured `timeout`. A hook that exceeds its
  timeout is terminated and treated as a failure.

### Error handling

**Pre-phase hooks:**

- `on_fail = "warn"` (default): A non-zero exit code or timeout is logged as a
  warning. The warning is recorded in the phase receipt under the `warnings`
  array. The phase proceeds normally.
- `on_fail = "fail"`: A non-zero exit code or timeout aborts the phase
  immediately. A failure receipt is written with a `hook_failure: "pre_phase"`
  flag for audit purposes. The LLM is never invoked.

**Post-phase hooks:**

Post-phase hooks run after the phase has already succeeded -- artifacts have
been written and the receipt has been committed. Because of this, post-phase
hook failures are **always treated as warnings** regardless of the `on_fail`
setting. This ensures that completed work is never discarded due to a
notification script failing.

**Hook execution errors** (e.g., command not found, spawn failure) are handled
the same way as non-zero exit codes: they respect `on_fail` for pre-phase
hooks and are always warnings for post-phase hooks.

### Receipt integration

Hook outcomes are recorded in the phase receipt:

- Pre-phase hook warnings appear in the receipt's `warnings` array as strings
  like `hook_failed:pre_phase:design:./scripts/lint.sh:exit_code=1` or
  `hook_timeout:pre_phase:design:./scripts/lint.sh`.
- Pre-phase hook failures set the receipt flag `hook_failure: "pre_phase"`.
- Pre-phase hook execution errors set the receipt flag
  `hook_error: "pre_phase"`.
- Post-phase hook warnings are logged but not written to the receipt (the
  receipt is already committed when the post-phase hook runs).

### Practical examples

**Lint before fixup:**

```toml
[hooks.pre_phase.fixup]
command = "cargo clippy --workspace --all-targets -- -D warnings"
on_fail = "fail"
timeout = 120
```

If `clippy` finds warnings, the fixup phase is aborted so you can address lint
issues before applying LLM-proposed changes.

**Notify after review:**

```toml
[hooks.post_phase.review]
command = "curl -s -X POST https://hooks.slack.com/services/T.../B.../xxx -d '{\"text\":\"Review phase completed for spec '\"$XCHECKER_SPEC_ID\"'\"}'"
on_fail = "warn"
timeout = 10
```

Sends a Slack notification when the review phase finishes. The short timeout
and `on_fail = "warn"` ensure a network hiccup does not block the workflow.

**Custom validation after requirements:**

```toml
[hooks.post_phase.requirements]
command = "./scripts/validate_requirements.sh"
on_fail = "warn"
timeout = 30
```

Where `validate_requirements.sh` reads stdin for context:

```bash
#!/usr/bin/env bash
# Read the JSON context from stdin
CONTEXT=$(cat)
SPEC_ID=$(echo "$CONTEXT" | jq -r .spec_id)

# Check that the requirements artifact exists and has content
ARTIFACT=".xchecker/specs/${SPEC_ID}/artifacts/00-requirements.md"
if [ ! -s "$ARTIFACT" ]; then
  echo "ERROR: requirements artifact is empty or missing" >&2
  exit 1
fi

echo "Requirements validation passed for ${SPEC_ID}"
```

**Gate design on requirements quality:**

```toml
[hooks.pre_phase.design]
command = "./scripts/check_requirements_quality.sh"
on_fail = "fail"
timeout = 30
```

This blocks the design phase from starting unless the requirements artifact
passes your quality checks.

**All phases -- universal logging:**

```toml
[hooks.pre_phase.requirements]
command = "echo \"Starting $XCHECKER_PHASE for $XCHECKER_SPEC_ID\" >> /tmp/xchecker.log"

[hooks.pre_phase.design]
command = "echo \"Starting $XCHECKER_PHASE for $XCHECKER_SPEC_ID\" >> /tmp/xchecker.log"

[hooks.pre_phase.tasks]
command = "echo \"Starting $XCHECKER_PHASE for $XCHECKER_SPEC_ID\" >> /tmp/xchecker.log"

[hooks.post_phase.requirements]
command = "echo \"Finished $XCHECKER_PHASE for $XCHECKER_SPEC_ID\" >> /tmp/xchecker.log"

[hooks.post_phase.design]
command = "echo \"Finished $XCHECKER_PHASE for $XCHECKER_SPEC_ID\" >> /tmp/xchecker.log"

[hooks.post_phase.tasks]
command = "echo \"Finished $XCHECKER_PHASE for $XCHECKER_SPEC_ID\" >> /tmp/xchecker.log"
```

Each phase must be configured individually -- there is no wildcard hook that
applies to all phases.

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

# Configuration Reference

Complete reference for all xchecker configuration keys, environment variables,
and CLI flags. For a task-oriented guide, see
[Configuration Guide](../guides/CONFIGURATION.md).

---

## Configuration precedence

Settings are resolved in this order (highest priority wins):

| Priority | Source | Example |
|----------|--------|---------|
| 1 (highest) | CLI flags | `--model sonnet` |
| 2 | Environment variables | `XCHECKER_LLM_PROVIDER=openrouter` |
| 3 | Config file | `.xchecker/config.toml` |
| 4 (lowest) | Built-in defaults | `model = "haiku"` |

---

## Config file discovery

xchecker searches upward from the current working directory for
`.xchecker/config.toml`. The search stops at:

- The filesystem root
- A Git repository root (`.git` directory found)

Override with `--config <path>`.

---

## TOML configuration keys

### [defaults]

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `model` | String | `"haiku"` | LLM model name |
| `max_turns` | Integer | `6` | Maximum LLM interaction turns per phase |
| `output_format` | String | `"stream-json"` | LLM output format (`stream-json` or `text`) |
| `packet_max_bytes` | Integer | `65536` | Maximum packet size in bytes |
| `packet_max_lines` | Integer | `1200` | Maximum packet size in lines |
| `runner_mode` | String | `"auto"` | Execution mode (`auto`, `native`, `wsl`) |
| `runner_distro` | String | `null` | WSL distribution name (Windows only) |
| `claude_path` | String | `null` | Custom Claude CLI binary path |
| `phase_timeout` | Integer | `600` | Phase timeout in seconds (minimum 5) |
| `lock_ttl_seconds` | Integer | `900` | Lock TTL in seconds |
| `stdout_cap_bytes` | Integer | `2097152` | Stdout ring buffer cap (2 MiB) |
| `stderr_cap_bytes` | Integer | `262144` | Stderr ring buffer cap (256 KiB) |
| `strict_validation` | Boolean | `false` | Fail phases on validation errors |

### [phases.<phase>]

Per-phase overrides. Phase keys: `requirements`, `design`, `tasks`, `review`,
`fixup`, `final`.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `model` | String | `null` | Override `defaults.model` for this phase |
| `max_turns` | Integer | `null` | Override `defaults.max_turns` for this phase |
| `phase_timeout` | Integer | `null` | Override `defaults.phase_timeout` for this phase |

### [selectors]

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `include` | Array[String] | `["**/*.md", "**/*.yaml", "**/*.yml"]` | Glob patterns for files to include in packets |
| `exclude` | Array[String] | `["target/**", "node_modules/**", ".git/**"]` | Glob patterns for files to exclude from packets |

### [llm]

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `provider` | String | `"claude-cli"` | LLM provider (`claude-cli`, `gemini-cli`, `openrouter`, `anthropic`) |
| `fallback_provider` | String | `null` | Fallback provider if primary fails to initialize |
| `execution_strategy` | String | `"controlled"` | Execution strategy (`controlled` only) |
| `prompt_template` | String | `"default"` | Prompt template (`default`, `claude-optimized`, `openai-compatible`) |

### [llm.claude]

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `binary` | String | `null` | Custom Claude CLI binary path |

### [llm.gemini]

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `binary` | String | `null` | Custom Gemini CLI binary path |
| `default_model` | String | `null` | Default Gemini model |

### [llm.openrouter]

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `api_key_env` | String | `"OPENROUTER_API_KEY"` | Environment variable containing API key |
| `base_url` | String | `"https://openrouter.ai/api/v1/chat/completions"` | API endpoint URL |
| `model` | String | Required | Model identifier (e.g. `"google/gemini-2.0-flash-lite"`) |
| `max_tokens` | Integer | `2048` | Maximum tokens per completion |
| `temperature` | Float | `0.2` | Sampling temperature (0.0--1.0) |
| `budget` | Integer | `20` | Maximum LLM calls per process |

### [llm.anthropic]

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `api_key_env` | String | `"ANTHROPIC_API_KEY"` | Environment variable containing API key |
| `base_url` | String | `"https://api.anthropic.com/v1/messages"` | API endpoint URL |
| `model` | String | Required | Model identifier (e.g. `"sonnet"`) |
| `max_tokens` | Integer | `2048` | Maximum tokens per completion |
| `temperature` | Float | `0.2` | Sampling temperature (0.0--1.0) |

### [runner]

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `mode` | String | `"auto"` | Execution mode (`auto`, `native`, `wsl`) |
| `distro` | String | `null` | WSL distribution (Windows only) |
| `claude_path` | String | `null` | Custom Claude CLI path |
| `phase_timeout` | Integer | `600` | Phase timeout in seconds (minimum 5) |

### [hooks]

The `[hooks]` section configures custom shell scripts that run before or after
phase execution. Hooks are organized into two sub-tables: `pre_phase` (runs
before the LLM is invoked) and `post_phase` (runs after artifacts and receipt
are written).

Each sub-table is keyed by phase name. Phase keys: `requirements`, `design`,
`tasks`, `review`, `fixup`, `final`.

#### [hooks.pre_phase.\<phase\>]

Pre-phase hooks run before the LLM invocation. If `on_fail = "fail"` and the
hook exits non-zero or times out, the phase is aborted and a failure receipt is
written with `hook_failure: "pre_phase"`. If `on_fail = "warn"`, the warning is
recorded in the receipt's `warnings` array and the phase proceeds.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `command` | String | Required | Shell command to execute (via `sh -c` on Unix, `cmd /C` on Windows) |
| `on_fail` | String | `"warn"` | Failure behavior: `"warn"` logs and continues; `"fail"` aborts the phase |
| `timeout` | Integer | `60` | Maximum execution time in seconds; the hook is terminated if exceeded |

#### [hooks.post_phase.\<phase\>]

Post-phase hooks run after the phase succeeds (artifacts written, receipt
committed). Because the phase is already complete, post-phase hook failures are
**always treated as warnings** regardless of the `on_fail` setting.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `command` | String | Required | Shell command to execute (via `sh -c` on Unix, `cmd /C` on Windows) |
| `on_fail` | String | `"warn"` | Failure behavior: effectively always `"warn"` for post-phase hooks |
| `timeout` | Integer | `60` | Maximum execution time in seconds; the hook is terminated if exceeded |

#### Hook environment variables

All hooks receive these environment variables:

| Variable | Description | Example |
|----------|-------------|---------|
| `XCHECKER_SPEC_ID` | Spec identifier | `my-feature` |
| `XCHECKER_PHASE` | Phase name | `design` |
| `XCHECKER_HOOK_TYPE` | Hook point | `pre_phase` or `post_phase` |

Additionally, a JSON payload with the same context is written to the hook's
stdin: `{"spec_id":"...","phase":"...","hook_type":"..."}`.

#### Hook execution details

- Working directory: the directory where `xchecker` was invoked.
- Stdout and stderr are captured and truncated to 2048 bytes each.
- Timeouts terminate the hook process; the result is treated as a failure.

#### Example

```toml
[hooks.pre_phase.fixup]
command = "cargo clippy --workspace -- -D warnings"
on_fail = "fail"
timeout = 120

[hooks.post_phase.review]
command = "./scripts/notify_slack.sh"
on_fail = "warn"
timeout = 10
```

### [security]

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `extra_secret_patterns` | Array[String] | `[]` | Additional regex patterns for secret detection |
| `ignore_secret_patterns` | Array[String] | `[]` | Patterns to suppress from secret detection |

### [debug]

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `debug_packet` | Boolean | `false` | Write full packet to context/ after secret scan |
| `verbose` | Boolean | `false` | Enable verbose structured logging |

---

## Prompt template compatibility

| Template | claude-cli | gemini-cli | openrouter | anthropic |
|----------|-----------|------------|------------|-----------|
| `default` | Yes | Yes | Yes | Yes |
| `claude-optimized` | Yes | No | No | Yes |
| `openai-compatible` | No | Yes | Yes | No |

Template aliases (case-insensitive): `claude` maps to `claude-optimized`;
`openai` and `openrouter` map to `openai-compatible`.

Incompatible combinations are rejected during config validation.

---

## Strict validation checks

When `strict_validation = true`, generative phases (Requirements, Design, Tasks)
must pass these quality checks:

| Check | Description |
|-------|-------------|
| No meta-summaries | Output must not start with "Here is...", "I'll create...", etc. |
| Minimum length | Requirements: 30 lines, Design: 50 lines, Tasks: 40 lines |
| Required sections | Phase-specific headers (e.g. `## Functional Requirements`) |

When `false` (default), violations are logged as warnings but do not abort the phase.

---

## Exit codes

| Code | Name | Description |
|------|------|-------------|
| 0 | SUCCESS | Operation completed |
| 1 | ERROR | General error |
| 2 | CLI_ARGS | Invalid CLI arguments |
| 7 | PACKET_OVERFLOW | Packet size exceeded |
| 8 | SECRET_DETECTED | Secret found in packet |
| 9 | LOCK_HELD | Lock already held |
| 10 | PHASE_TIMEOUT | Phase timed out |
| 70 | CLAUDE_FAILURE | LLM provider failure |

---

## Environment variables

| Variable | Description | Example |
|----------|-------------|---------|
| `XCHECKER_HOME` | Override state directory location | `/tmp/xchecker-build-123` |
| `XCHECKER_LLM_PROVIDER` | Override LLM provider | `openrouter` |
| `XCHECKER_EXECUTION_STRATEGY` | Override execution strategy | `controlled` |
| `XCHECKER_LLM_FALLBACK_PROVIDER` | Set fallback provider | `anthropic` |
| `XCHECKER_LLM_PROMPT_TEMPLATE` | Override prompt template | `claude-optimized` |
| `XCHECKER_LLM_GEMINI_DEFAULT_MODEL` | Override Gemini default model | `gemini-2.0-pro` |
| `XCHECKER_OPENROUTER_BUDGET` | Override OpenRouter call budget | `100` |
| `XCHECKER_SKIP_LLM_TESTS` | Skip all real LLM tests in CI | `1` |
| `XCHECKER_ENABLE_REAL_CLAUDE` | Enable real Claude API tests | `1` |
| `OPENROUTER_API_KEY` | OpenRouter API key | (secret) |
| `ANTHROPIC_API_KEY` | Anthropic API key | (secret) |
| `GEMINI_API_KEY` | Gemini API key | (secret) |

---

## CLI flag to config key mapping

| CLI Flag | Config Key | Description |
|----------|-----------|-------------|
| `--model <name>` | `defaults.model` | LLM model |
| `--max-turns <n>` | `defaults.max_turns` | Max interaction turns |
| `--packet-max-bytes <n>` | `defaults.packet_max_bytes` | Max packet bytes |
| `--packet-max-lines <n>` | `defaults.packet_max_lines` | Max packet lines |
| `--runner-mode <mode>` | `defaults.runner_mode` | Runner mode |
| `--runner-distro <name>` | `defaults.runner_distro` | WSL distro |
| `--claude-path <path>` | `defaults.claude_path` | Claude CLI path |
| `--phase-timeout <secs>` | `defaults.phase_timeout` | Phase timeout |
| `--lock-ttl-seconds <secs>` | `defaults.lock_ttl_seconds` | Lock TTL |
| `--stdout-cap-bytes <n>` | `defaults.stdout_cap_bytes` | Stdout buffer cap |
| `--stderr-cap-bytes <n>` | `defaults.stderr_cap_bytes` | Stderr buffer cap |
| `--strict-validation` | `defaults.strict_validation` | Enable strict validation |
| `--no-strict-validation` | `defaults.strict_validation` | Disable strict validation |
| `--llm-provider <name>` | `llm.provider` | LLM provider |
| `--llm-fallback-provider <name>` | `llm.fallback_provider` | Fallback provider |
| `--execution-strategy <name>` | `llm.execution_strategy` | Execution strategy |
| `--prompt-template <name>` | `llm.prompt_template` | Prompt template |
| `--llm-gemini-default-model <name>` | `llm.gemini.default_model` | Gemini default model |
| `--extra-secret-pattern <regex>` | `security.extra_secret_patterns` | Add secret pattern |
| `--ignore-secret-pattern <regex>` | `security.ignore_secret_patterns` | Suppress secret pattern |
| `--debug-packet` | `debug.debug_packet` | Write debug packet |
| `--verbose` | `debug.verbose` | Verbose logging |
| `--allow-links` | (runtime only) | Allow symlinks/hardlinks in fixups |
| `--strict-lock` | (runtime only) | Strict lock enforcement |
| `--dry-run` | (runtime only) | Simulate without LLM calls |
| `--config <path>` | (discovery override) | Config file path |

---

## See also

- [Configuration Guide](../guides/CONFIGURATION.md) -- task-oriented setup
- [LLM Providers](../guides/LLM_PROVIDERS.md) -- provider details
- [JSON Schemas](SCHEMAS.md) -- schema index
- [Contracts](CONTRACTS.md) -- versioning and compatibility

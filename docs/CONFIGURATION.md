# xchecker Configuration Guide

xchecker uses a hierarchical configuration system with the following precedence:

1. **CLI flags** (highest priority)
2. **Configuration file** 
3. **Built-in defaults** (lowest priority)

## State Directory (XCHECKER_HOME)

xchecker stores all state (specs, artifacts, receipts, and context) in a state directory. The location is determined by:

1. **Thread-local override** (used internally for test isolation)
2. **XCHECKER_HOME environment variable** (user/CI override)
3. **Default: `./.xchecker`** (relative to current working directory)

### Using XCHECKER_HOME

You can override the default state directory location using the `XCHECKER_HOME` environment variable:

```bash
# Set globally for your session
export XCHECKER_HOME=/path/to/custom/state
xchecker spec my-feature

# Or inline for a single command
XCHECKER_HOME=/tmp/xchecker-test xchecker status my-feature

# Useful for CI/CD to isolate builds
XCHECKER_HOME=/tmp/build-${BUILD_ID} xchecker spec feature
```

### Directory Structure

The state directory contains the following structure:

```
.xchecker/                    # State directory (XCHECKER_HOME)
├── config.toml              # Configuration file (optional)
└── specs/                   # All specs
    └── <spec-id>/          # Individual spec directory
        ├── artifacts/      # Generated artifacts
        │   ├── 00-requirements.md
        │   ├── 10-design.md
        │   └── 20-tasks.md
        ├── receipts/       # Execution receipts
        │   └── <phase>-<timestamp>.json
        └── context/        # Context files for Claude
            └── packet-<hash>.txt
```

### Use Cases

**Development**: Use default `./.xchecker` for local development
```bash
cd my-project
xchecker spec my-feature  # Uses ./my-project/.xchecker
```

**CI/CD**: Use isolated directories per build
```bash
export XCHECKER_HOME=/tmp/xchecker-build-${BUILD_ID}
xchecker spec ci-feature
```

**Testing**: Tests use thread-local override for isolation (no environment variable needed)

## Configuration File Discovery

xchecker automatically discovers configuration files by searching upward from the current working directory for `.xchecker/config.toml`. The search stops at:

- The filesystem root
- A Git repository root (if `.git` directory is found)

You can override this behavior with the `--config <path>` flag.

## Configuration File Format

The configuration file uses TOML format with the following sections:

### Example Configuration

```toml
# .xchecker/config.toml

[defaults]
# Model configuration
model = "haiku"
max_turns = 6
output_format = "stream-json"

# Packet limits (token efficiency)
packet_max_bytes = 65536
packet_max_lines = 1200

# Runner configuration
runner_mode = "auto"  # auto, native, wsl
runner_distro = "Ubuntu-22.04"  # WSL distro (optional)
claude_path = "/usr/local/bin/claude"  # Custom Claude path (optional)

# Validation (when true, validation failures fail phases)
strict_validation = false

[llm]
# LLM provider configuration (V11-V14: only claude-cli supported)
provider = "claude-cli"
execution_strategy = "controlled"

[llm.claude]
# Optional: Custom Claude CLI binary path
binary = "/usr/local/bin/claude"

[selectors]
# File inclusion patterns (glob syntax)
include = [
    "docs/**/*.md",
    "*.md",
    "src/**/*.rs",
    "Cargo.toml",
    "*.yaml",
    "*.yml"
]

# File exclusion patterns (glob syntax)
exclude = [
    "target/**",
    "node_modules/**",
    ".git/**",
    "*.log",
    "*.tmp"
]

[runner]
# Runner-specific configuration
mode = "auto"
distro = "Ubuntu-22.04"
claude_path = "/usr/local/bin/claude"
```

## Configuration Sections

### [defaults]

Controls default behavior for all operations.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `model` | String | `"haiku"` | Claude model to use |
| `max_turns` | Integer | `6` | Maximum Claude interaction turns |
| `output_format` | String | `"stream-json"` | Claude output format (`stream-json` or `text`) |
| `packet_max_bytes` | Integer | `65536` | Maximum packet size in bytes |
| `packet_max_lines` | Integer | `1200` | Maximum packet size in lines |
| `runner_mode` | String | `"auto"` | Runner mode (`auto`, `native`, `wsl`) |
| `runner_distro` | String | `null` | WSL distribution name (optional) |
| `claude_path` | String | `null` | Custom Claude CLI path (optional) |
| `phase_timeout` | Integer | `600` | Phase timeout in seconds (minimum 5s) |
| `lock_ttl_seconds` | Integer | `900` | Lock TTL in seconds (default 15 minutes) |
| `stdout_cap_bytes` | Integer | `2097152` | Stdout ring buffer cap in bytes (2 MiB) |
| `stderr_cap_bytes` | Integer | `262144` | Stderr ring buffer cap in bytes (256 KiB) |
| `strict_validation` | Boolean | `false` | Fail phases on validation errors (see below) |

#### Strict Validation Mode

When `strict_validation = true`, phase outputs are validated and must pass quality checks:

1. **No meta-summaries** - Output must not start with phrases like "Here is...", "I'll create...", "This document..."
2. **Minimum length** - Each phase has minimum line requirements (Requirements: 30, Design: 50, Tasks: 40, etc.)
3. **Required sections** - Phase-specific headers must be present (e.g., `## Functional Requirements` for Requirements phase)

**Behavior by mode:**
- `strict_validation = false` (default): Validation issues are logged as warnings, but the phase continues
- `strict_validation = true`: Validation issues cause the phase to fail with exit code 1

**Example configuration:**

```toml
[defaults]
strict_validation = true  # Enforce quality requirements on LLM output
```

**CLI override:**
```bash
# Enable strict validation for a single run
xchecker spec my-feature --strict-validation

# Disable strict validation for a single run
xchecker spec my-feature --no-strict-validation
```

**Applicable phases:** Requirements, Design, Tasks (generative phases only)

### [selectors]

Controls which files are included in context packets.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `include` | Array | `["**/*.md", "**/*.yaml", "**/*.yml"]` | File patterns to include |
| `exclude` | Array | `["target/**", "node_modules/**", ".git/**"]` | File patterns to exclude |

**Pattern Syntax:**
- `*` matches any characters except `/`
- `**` matches any characters including `/` (recursive)
- `?` matches any single character
- `[abc]` matches any character in the set
- `{a,b}` matches either `a` or `b`

### [llm]

LLM provider and execution strategy configuration (V11+ multi-provider support).

**⚠️ V11-V14 Constraints:** Only `claude-cli` provider and `controlled` execution strategy are supported. Other values will cause configuration validation errors.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `provider` | String | `"claude-cli"` | LLM provider to use (**must be `"claude-cli"` in V11-V14**) |
| `execution_strategy` | String | `"controlled"` | Execution strategy (**must be `"controlled"` in V11-V14**) |

**Supported Values (V11-V14):**

- **`provider`**: Only `"claude-cli"` is supported
  - Uses the official Claude CLI tool for invocations
  - Automatically selected if omitted
  - Attempting other values (e.g., `"gemini-cli"`, `"openrouter"`, `"anthropic"`) will fail validation

- **`execution_strategy`**: Only `"controlled"` is supported
  - LLMs propose changes via structured output (e.g., fixups)
  - All file modifications go through xchecker's fixup pipeline
  - No direct disk writes or external tool invocation by LLMs
  - Attempting `"externaltool"` or other values will fail validation

**Valid Configuration Example:**

```toml
# Explicit configuration (can be omitted, uses defaults)
[llm]
provider = "claude-cli"
execution_strategy = "controlled"

# Optional: Claude CLI binary path
[llm.claude]
binary = "/usr/local/bin/claude"
```

**Default Configuration (when omitted):**

```toml
# These defaults are used if [llm] section is omitted
[llm]
provider = "claude-cli"
execution_strategy = "controlled"
```

**Validation Errors:**

Attempting unsupported values will result in clear error messages:

```bash
# ❌ Invalid provider
[llm]
provider = "gemini-cli"
# Error: llm.provider 'gemini-cli' is not supported.
# Currently only 'claude-cli' is supported in V11

# ❌ Invalid execution strategy
[llm]
execution_strategy = "externaltool"
# Error: llm.execution_strategy 'externaltool' is not supported.
# Currently only 'controlled' is supported in V11-V14
```

**Reserved for Future Versions (V15+):**

The following values are reserved for future implementation:

- **Providers**: `gemini-cli`, `openrouter`, `anthropic`
- **Execution Strategies**: `externaltool` (for agentic workflows with direct writes/tool use)

For detailed information on all providers, including authentication, testing, and cost control, see [LLM_PROVIDERS.md](LLM_PROVIDERS.md).

See [ORCHESTRATOR.md](ORCHESTRATOR.md) "LLM Layer (V11 Skeleton)" section for more details on these constraints.

### [llm.openrouter] (Reserved for V13+)

OpenRouter-specific configuration for HTTP API access and budget control.

**⚠️ V11-V14 Note:** OpenRouter is reserved for V13+. This configuration section is documented for future reference.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `api_key_env` | String | `"OPENROUTER_API_KEY"` | Environment variable containing API key |
| `base_url` | String | `"https://openrouter.ai/api/v1/chat/completions"` | API endpoint URL |
| `model` | String | Required | Model identifier (e.g., `"google/gemini-2.0-flash-lite"`) |
| `max_tokens` | Integer | `2048` | Maximum tokens per completion |
| `temperature` | Float | `0.2` | Sampling temperature (0.0-1.0) |
| `budget` | Integer | `20` | Maximum LLM calls per process |

**Budget Configuration Precedence:**

Budget limits are resolved with the following precedence (highest to lowest):

1. **Environment variable**: `XCHECKER_OPENROUTER_BUDGET=50`
2. **Config file**: `[llm.openrouter] budget = 50`
3. **Default**: 20 calls per process

**Example Configuration:**

```toml
[llm.openrouter]
api_key_env = "OPENROUTER_API_KEY"
model = "google/gemini-2.0-flash-lite"
max_tokens = 2048
temperature = 0.2
budget = 50  # Set budget in config file
```

**Environment Variable Override:**

```bash
# Override budget via environment variable (takes precedence over config)
export XCHECKER_OPENROUTER_BUDGET=100
xchecker spec my-feature

# Or inline
XCHECKER_OPENROUTER_BUDGET=30 xchecker spec my-feature
```

**Budget Enforcement:**

- Tracks **attempted calls**, not successful requests
- Fails fast with `LlmError::BudgetExceeded` when limit reached
- Budget resets per xchecker process (not persistent across runs)
- Budget exhaustion is recorded in receipts with `budget_exhausted: true`

For more details on OpenRouter configuration, authentication, and usage, see [LLM_PROVIDERS.md](LLM_PROVIDERS.md#provider-openrouter-reserved-for-v13).

### [runner]

Platform-specific execution configuration.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `mode` | String | `"auto"` | Execution mode |
| `distro` | String | `null` | WSL distribution (Windows only) |
| `claude_path` | String | `null` | Custom Claude CLI path |
| `phase_timeout` | Integer | `600` | Phase timeout in seconds (minimum 5s) |

**Runner Modes:**
- `native`: Use native Claude CLI directly (recommended for most users)
- `wsl`: Force WSL execution (Windows only, requires WSL with Claude CLI installed)
- `auto`: Auto-detect best available option (tries native first, falls back to WSL on Windows)

**Note:** For production use, explicitly specifying `native` or `wsl` is recommended for predictable behavior. The `auto` mode is useful for development environments where the runner may vary.

### [security]

Security and secret detection configuration.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `extra_secret_patterns` | Array | `[]` | Additional regex patterns for secret detection |
| `ignore_secret_patterns` | Array | `[]` | Patterns to suppress from secret detection |

**Default Secret Patterns:**

xchecker includes default secret patterns covering AWS, GCP, Azure, generic API tokens, database connection URLs, SSH/PEM private keys, and platform tokens (GitHub, GitLab, Slack, Stripe, etc.).

For the complete list of patterns, counts, and regex definitions, see [SECURITY.md](SECURITY.md#default-secret-patterns).

### [debug]

Debug and diagnostic configuration.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `debug_packet` | Boolean | `false` | Write full packet to context/ after secret scan |
| `verbose` | Boolean | `false` | Enable verbose structured logging |

## CLI Flag Override Examples

All configuration options can be overridden via CLI flags:

```bash
# Override model
xchecker spec my-feature --model claude-3-opus-20240229

# Override packet limits
xchecker spec my-feature --packet-max-bytes 32768 --packet-max-lines 800

# Override runner mode
xchecker spec my-feature --runner-mode wsl --runner-distro Ubuntu-20.04

# Override Claude path
xchecker spec my-feature --claude-path /custom/path/to/claude

# Override timeout
xchecker spec my-feature --phase-timeout 1200

# Override lock TTL
xchecker spec my-feature --lock-ttl-seconds 1800

# Override buffer sizes
xchecker spec my-feature --stdout-cap-bytes 4194304 --stderr-cap-bytes 524288

# Add custom secret patterns
xchecker spec my-feature --extra-secret-pattern "SECRET_[A-Z0-9]{32}"

# Ignore specific secret patterns
xchecker spec my-feature --ignore-secret-pattern "ghp_"

# Enable debug packet writing
xchecker spec my-feature --debug-packet

# Enable verbose logging
xchecker spec my-feature --verbose

# Allow symlinks and hardlinks in fixups
xchecker resume my-feature --phase fixup --apply-fixups --allow-links

# Strict lock enforcement
xchecker spec my-feature --strict-lock
```

## Configuration Validation

xchecker validates configuration on startup and provides helpful error messages:

```bash
# Check effective configuration
xchecker status my-spec

# This shows:
# - Source of each setting (CLI > config > defaults)
# - Effective values being used
# - Any validation warnings
```

## Environment-Specific Configurations

### Development
```toml
[defaults]
model = "haiku"  # Faster, cheaper model
packet_max_bytes = 32768  # Smaller packets for faster iteration
max_turns = 3  # Fewer turns for quick feedback

[selectors]
include = ["src/**/*.rs", "Cargo.toml", "README.md"]
exclude = ["target/**", "tests/**"]  # Skip tests during development
```

### Production
```toml
[defaults]
model = "sonnet"  # Best quality model
packet_max_bytes = 65536  # Full context
max_turns = 6  # Allow thorough exploration

[selectors]
include = [
    "src/**/*.rs",
    "tests/**/*.rs", 
    "docs/**/*.md",
    "*.md",
    "Cargo.toml",
    "*.yaml"
]
# Minimal exclusions for comprehensive context
exclude = ["target/**", ".git/**"]
```

### CI/CD
```toml
[defaults]
runner_mode = "native"  # Explicit mode for CI
output_format = "text"  # Fallback format for reliability

[selectors]
# Focused context for CI specs
include = [".github/**/*.yml", "Cargo.toml", "README.md"]
exclude = ["target/**", "src/**"]  # Focus on CI configuration
```

## Troubleshooting

### Configuration Not Found
```
Error: Failed to load configuration
Caused by: No configuration file found

Solution: Create .xchecker/config.toml or use --config flag
```

### Invalid TOML Syntax
```
Error: Failed to parse TOML config file
Caused by: TOML parse error at line 5, column 12

Solution: Check TOML syntax, ensure proper quoting and structure
```

### Invalid Values
```
Error: Invalid configuration value
Key: runner_mode, Value: invalid_mode

Solution: Use valid values (auto, native, wsl)
```

### WSL Not Available
```
Error: WSL runner requested but not available
Suggestion: Install WSL with 'wsl --install' or use native runner

Solution: Install WSL or change runner_mode to "native"
```

## Best Practices

1. **Start Simple**: Begin with minimal configuration and add complexity as needed
2. **Use Includes**: Prefer specific include patterns over broad exclusions
3. **Environment-Specific**: Use different configs for dev/prod/CI environments
4. **Version Control**: Commit `.xchecker/config.toml` to share team settings
5. **Document Changes**: Comment configuration choices for team understanding
6. **Test Configurations**: Use `--dry-run` to validate configuration changes
7. **Monitor Performance**: Adjust packet limits based on actual usage patterns

## Security Considerations

- **No Secrets**: Never put API keys or secrets in configuration files
- **Path Validation**: Be careful with custom paths, especially in shared environments
- **File Patterns**: Ensure exclude patterns prevent sensitive file inclusion
- **WSL Security**: Understand WSL security model when using cross-platform execution

For more information, see the [xchecker documentation](https://github.com/your-org/xchecker).

## See Also

- [SECURITY.md](SECURITY.md) - Secret detection and redaction configuration
- [PERFORMANCE.md](PERFORMANCE.md) - Performance-related configuration options
- [DOCTOR.md](DOCTOR.md) - Configuration validation and health checks
- [INDEX.md](INDEX.md) - Documentation index

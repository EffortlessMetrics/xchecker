# xchecker

[![Crates.io](https://img.shields.io/crates/v/xchecker.svg)](https://crates.io/crates/xchecker)
[![License](https://img.shields.io/crates/l/xchecker.svg)](https://github.com/EffortlessMetrics/xchecker#license)

A Rust CLI tool for orchestrating spec generation workflows with Claude AI.

## Features

- **Multi-Phase Orchestration**: Structured workflow through requirements, design, tasks, review, fixup, and final phases
- **Versioned JSON Contracts**: Stable schemas (v1) for receipts, status, and health checks with JCS (RFC 8785) canonical emission
- **Reproducibility Tracking**: Lockfile system pins model, CLI version, and schema version with drift detection
- **Security First**: Automatic secret redaction, pattern detection, and path validation
- **Performance Monitoring**: Process-scoped memory tracking and benchmarking (NFR1 validation)
- **Standardized Exit Codes**: Predictable error codes for CI/CD integration
- **Cross-Platform**: Native execution on Linux, macOS, Windows with WSL fallback support
- **Configuration Hierarchy**: CLI flags > config file > defaults with source attribution
- **Fixup System**: Preview and apply code changes with validation

## Installation

### From Crates.io (Recommended)

```bash
cargo install xchecker
```

### From Source

```bash
git clone https://github.com/EffortlessMetrics/xchecker.git
cd xchecker
cargo install --path .
```

### Requirements

- **Rust 1.70+** (for compilation from source)
- **Claude CLI** installed and authenticated (see [Installation Guide](docs/DOCTOR.md))
- **Linux, macOS, or Windows** (with WSL support on Windows)

## Quick Start

```bash
# Check environment health
xchecker doctor

# Generate a spec
xchecker spec my-feature

# Check status
xchecker status my-feature --json

# Run benchmarks
xchecker benchmark
```

## Commands

### `xchecker spec <spec-id>`

Generate or continue a spec workflow through the requirements phase.

**Options:**
- `--source <type>`: Input source type (`stdin`, `gh`, `fs`)
- `--gh <owner/repo>`: GitHub repository (when source is `gh`)
- `--repo <path>`: Local repository path (when source is `fs`)
- `--dry-run`: Preview execution without making Claude calls
- `--force`: Override stale locks
- `--apply-fixups`: Apply fixups to files (default is preview mode)
- `--strict-lock`: Hard fail on lockfile drift

```bash
# From stdin (default)
echo "Create a REST API" | xchecker spec my-feature

# From GitHub issue
xchecker spec issue-123 --source gh --gh owner/repo

# From filesystem
xchecker spec my-feature --source fs --repo /path/to/project

# Dry run mode
xchecker spec my-feature --dry-run
```

### `xchecker resume <spec-id> --phase <phase>`

Resume execution from a specific phase.

**Options:**
- `--phase <phase>`: Phase to resume from (required)
- `--dry-run`: Preview execution without making Claude calls
- `--force`: Override stale locks
- `--apply-fixups`: Apply fixups to files (default is preview mode)
- `--strict-lock`: Hard fail on lockfile drift

**Phases:** `requirements`, `design`, `tasks`, `review`, `fixup`, `final`

```bash
xchecker resume my-feature --phase design
xchecker resume my-feature --phase fixup --apply-fixups
xchecker resume my-feature --phase design --dry-run
```

### `xchecker status <spec-id>`

Display status and configuration for a spec.

**Options:**
- `--json`: Output status as JSON

```bash
xchecker status my-feature
xchecker status my-feature --json
```

**Shows:**
- Latest completed phase
- Artifacts with BLAKE3 hashes (first 8 chars)
- Last receipt information
- Effective configuration with source attribution
- Lockfile drift detection
- Pending fixups with intended targets

### `xchecker clean <spec-id>`

Clean up spec artifacts and receipts.

**Options:**
- `--hard`: Remove artifacts without confirmation
- `--force`: Force removal even if lock is present

```bash
xchecker clean my-feature
xchecker clean my-feature --hard  # Skip confirmation
xchecker clean my-feature --force  # Force removal even if locked
```

### `xchecker doctor`

Run environment health checks.

**Options:**
- `--json`: Output doctor results as JSON
- `--strict-exit`: Treat warnings as failures (exit non-zero on any warn or fail)

```bash
xchecker doctor
xchecker doctor --json
xchecker doctor --strict-exit  # Treat warnings as failures
```

**Checks:**
- Claude CLI availability and version
- Runner configuration (native/WSL)
- Write permissions
- Configuration parsing
- Atomic rename support
- WSL availability (Windows only)

See [docs/DOCTOR.md](docs/DOCTOR.md) for detailed health check documentation.

### `xchecker init <spec-id>`

Initialize a new spec with optional lockfile creation.

**Options:**
- `--create-lock`: Create a lockfile to pin model and CLI version

```bash
xchecker init my-feature
xchecker init my-feature --create-lock
```

### `xchecker benchmark`

Run performance benchmarks with process memory tracking (NFR1 validation).

**Options:**
- `--file-count <n>`: Number of files to create for packetization benchmark (default: 100)
- `--file-size <bytes>`: Size of each test file in bytes (default: 1024)
- `--iterations <n>`: Number of benchmark iterations (default: 5)

```bash
xchecker benchmark
xchecker benchmark --file-count 100 --iterations 5
xchecker benchmark --file-count 50 --file-size 2048 --iterations 10
```

**Validates:**
- Empty run ≤ 5s
- Packetization ≤ 200ms for 100 files

### `xchecker test`

Run integration smoke tests to validate all components.

**Options:**
- `--components`: Run component validation tests
- `--smoke`: Run smoke tests

```bash
xchecker test
xchecker test --components --smoke
```

## Exit Codes

xchecker uses standardized exit codes for automation and CI integration:

| Code | Name | Description |
|------|------|-------------|
| 0 | `SUCCESS` | Operation completed successfully |
| 1 | `UNKNOWN` | Unknown or unclassified error |
| 2 | `CLI_ARGS` | Invalid CLI arguments or configuration |
| 7 | `PACKET_OVERFLOW` | Packet size exceeded limits (pre-Claude) |
| 8 | `SECRET_DETECTED` | Secret detected by redaction system (hard stop) |
| 9 | `LOCK_HELD` | Lock already held by another process |
| 10 | `PHASE_TIMEOUT` | Phase execution exceeded timeout |
| 70 | `CLAUDE_FAILURE` | Claude CLI invocation failed |

### Exit Code Usage in CI

```bash
# Check exit code in scripts
xchecker spec my-feature
case $? in
  0) echo "Success" ;;
  1) echo "Unknown error" ;;
  2) echo "Configuration error" ;;
  7) echo "Packet too large" ;;
  8) echo "Secret detected" ;;
  9) echo "Lock conflict" ;;
  10) echo "Timeout" ;;
  70) echo "Claude CLI failed" ;;
  *) echo "Unexpected error" ;;
esac
```

### Error Receipts

When xchecker exits with a non-zero code, it writes a receipt with structured error information:

```json
{
  "schema_version": "1",
  "exit_code": 10,
  "error_kind": "phase_timeout",
  "error_reason": "Phase 'requirements' exceeded 600s timeout",
  "warnings": ["phase_timeout:600"]
}
```

**Error Kinds:**
- `cli_args`: Invalid CLI arguments (exit 2)
- `packet_overflow`: Packet size exceeded (exit 7)
- `secret_detected`: Secret detected (exit 8)
- `lock_held`: Lock conflict (exit 9)
- `phase_timeout`: Phase timeout (exit 10)
- `claude_failure`: Claude CLI failure (exit 70)
- `unknown`: Other errors (exit 1)

## LLM Provider

xchecker currently supports **Claude CLI only** as the LLM provider. All spec generation phases use the official Claude CLI tool for LLM invocations.

**V11-V14 Constraints:**
- **Provider**: Only `claude-cli` is supported
- **Execution Strategy**: Only `controlled` execution is supported (LLMs propose changes via structured output; all file modifications go through xchecker's fixup pipeline)
- **V15+**: Future versions will support additional providers (Gemini CLI, OpenRouter, Anthropic API) and execution strategies

**Configuration:**
```toml
# Default LLM configuration (can be omitted)
[llm]
provider = "claude-cli"           # Only supported value in V11-V14
execution_strategy = "controlled"  # Only supported value in V11-V14
```

For detailed LLM provider documentation, including authentication, testing, and cost control, see [docs/LLM_PROVIDERS.md](docs/LLM_PROVIDERS.md).

For LLM configuration options, see [docs/CONFIGURATION.md](docs/CONFIGURATION.md) (LLM section).

For LLM layer architecture and provider abstraction, see [docs/ORCHESTRATOR.md](docs/ORCHESTRATOR.md) (LLM Layer section).

## Configuration

xchecker uses a hierarchical configuration system:

1. **CLI flags** (highest priority)
2. **Configuration file** (`.xchecker/config.toml`)
3. **Built-in defaults** (lowest priority)

Configuration files are automatically discovered by searching upward from the current directory for `.xchecker/config.toml`.

### State Directory (XCHECKER_HOME)

xchecker stores all state (specs, artifacts, receipts) in a state directory. By default, this is `./.xchecker` in the current working directory.

You can override the state directory location using the `XCHECKER_HOME` environment variable:

```bash
# Use custom state directory
export XCHECKER_HOME=/path/to/custom/state
xchecker spec my-feature

# Or inline for a single command
XCHECKER_HOME=/tmp/xchecker-test xchecker status my-feature
```

The state directory contains:
- `specs/<spec-id>/` - Spec-specific state and artifacts
- `specs/<spec-id>/artifacts/` - Generated artifacts (requirements, design, etc.)
- `specs/<spec-id>/receipts/` - Execution receipts with metadata
- `specs/<spec-id>/context/` - Context files for Claude

See [docs/CONFIGURATION.md](docs/CONFIGURATION.md) for detailed configuration documentation.

### Configuration Options

All configuration options can be set via:
1. CLI flags (highest priority)
2. Configuration file (`.xchecker/config.toml`)
3. Built-in defaults (lowest priority)

**Complete Configuration Reference:**

```toml
# .xchecker/config.toml

[defaults]
# Claude model configuration
model = "haiku"  # Model to use for all phases
max_turns = 6                          # Maximum interaction turns
output_format = "stream-json"          # Output format: stream-json or text

# Packet size limits
packet_max_bytes = 65536               # Maximum packet size (64 KB)
packet_max_lines = 1200                # Maximum packet lines

# Timeouts and limits
phase_timeout = 600                    # Phase timeout in seconds (10 minutes)
stdout_cap_bytes = 2097152             # Stdout ring buffer (2 MiB)
stderr_cap_bytes = 262144              # Stderr ring buffer (256 KiB)
lock_ttl_seconds = 900                 # Lock TTL (15 minutes)

# Security
debug_packet = false                   # Write full packet for debugging
allow_links = false                    # Allow symlinks/hardlinks in fixups

[selectors]
# File selection patterns (glob syntax)
include = [
    "docs/**/*.md",
    "src/**/*.rs",
    "*.yaml",
    "*.toml"
]
exclude = [
    "target/**",
    "node_modules/**",
    ".git/**",
    "*.log",
    "*.tmp"
]

[runner]
# Runner configuration
mode = "auto"                          # auto, native, or wsl
distro = "Ubuntu-22.04"                # WSL distro (when mode=wsl)
claude_path = "/usr/local/bin/claude"  # Custom Claude CLI path

[secrets]
# Secret pattern configuration
ignore_patterns = [
    "EXAMPLE_.*",                      # Ignore example patterns
    "TEST_KEY_.*"                      # Ignore test keys
]
extra_patterns = [
    "CUSTOM_KEY_[A-Z0-9]+",           # Add custom patterns
    "SECRET_[A-Za-z0-9]{32}"
]

[claude]
# Claude tool control
allowed_tools = ["read_file", "write_file"]  # Tools to allow
denied_tools = ["execute_command"]           # Tools to deny
dangerously_skip_permissions = false         # Skip permission checks
```

### Minimal Configuration Example

Most users only need a minimal configuration:

```toml
# .xchecker/config.toml

[defaults]
model = "haiku"

[selectors]
include = ["src/**/*.rs", "docs/**/*.md"]
exclude = ["target/**", ".git/**"]
```

### Global CLI Options

Available for all commands:

**Configuration:**
- `--config <path>`: Override config file path (default: auto-discover `.xchecker/config.toml`)
- `--model <name>`: Claude model to use (default: `haiku`)
- `--max-turns <n>`: Maximum Claude interaction turns (default: 6)
- `--output-format <format>`: Claude output format (`stream-json` or `text`, default: `stream-json`)

**Packet Control:**
- `--packet-max-bytes <n>`: Maximum packet size in bytes (default: 65536)
- `--packet-max-lines <n>`: Maximum packet size in lines (default: 1200)
- `--debug-packet`: Write full packet to `context/<phase>-packet.txt` after secret scan passes

**Runner Configuration:**
- `--runner-mode <mode>`: Runner mode (`auto`, `native`, `wsl`, default: `auto`)
- `--runner-distro <name>`: WSL distribution name (when using WSL mode)
- `--claude-path <path>`: Custom Claude CLI path (default: search PATH)

**Timeouts and Limits:**
- `--phase-timeout <seconds>`: Phase timeout in seconds (default: 600, min: 5)
- `--stdout-cap-bytes <n>`: Maximum bytes for stdout ring buffer (default: 2097152 = 2 MiB)
- `--stderr-cap-bytes <n>`: Maximum bytes for stderr ring buffer (default: 262144 = 256 KiB)
- `--lock-ttl-seconds <n>`: Lock TTL in seconds (default: 900 = 15 minutes)

**Security:**
- `--ignore-secret-pattern <regex>`: Ignore specific secret patterns (can be repeated)
- `--extra-secret-pattern <regex>`: Add extra secret patterns to detect (can be repeated)
- `--allow-links`: Allow symlinks and hardlinks in fixup targets (default: reject)

**Claude Tool Control:**
- `--allow <pattern>`: Tool patterns to allow (passed to Claude as allowedTools, can be repeated)
- `--deny <pattern>`: Tool patterns to deny (passed to Claude as disallowedTools, can be repeated)
- `--dangerously-skip-permissions`: Skip permission checks (passed to Claude)

**Output:**
- `--verbose` / `-v`: Enable verbose output with structured logging

## JSON Contracts

xchecker provides versioned JSON schemas for all structured outputs:

- **Receipt Schema v1** (`schemas/receipt.v1.json`): Execution receipts with error tracking
- **Status Schema v1** (`schemas/status.v1.json`): Spec status and configuration
- **Doctor Schema v1** (`schemas/doctor.v1.json`): Health check results

**Key Features:**
- All JSON outputs use **JCS (RFC 8785)** for canonical emission with stable diffs
- Arrays are sorted before emission (outputs by path, checks by name)
- Schema version field enables reliable automation
- Forward compatibility via `additionalProperties: true`
- Strict deprecation policy (v1 supported ≥6 months after v2 release)

See [docs/CONTRACTS.md](docs/CONTRACTS.md) for the complete JSON schema versioning policy.

### Claude Code Integration

xchecker provides machine-friendly JSON output for Claude Code (Claude's IDE environment) integration:

```bash
# Get spec overview (high-level metadata, no full artifacts)
xchecker spec my-feature --json

# Get compact status summary
xchecker status my-feature --json

# Get resume context for a phase
xchecker resume my-feature --phase design --json
```

**Canonical Slash Commands:**
- `/xchecker spec <spec-id>` → `xchecker spec <spec-id> --json`
- `/xchecker status <spec-id>` → `xchecker status <spec-id> --json`
- `/xchecker resume <spec-id> <phase>` → `xchecker resume <spec-id> --phase <phase> --json`

See [docs/CLAUDE_CODE_INTEGRATION.md](docs/CLAUDE_CODE_INTEGRATION.md) for complete integration documentation including tool invocation model and example flows.

### Example Receipt Output

```json
{
  "schema_version": "1",
  "emitted_at": "2025-10-24T14:30:00Z",
  "spec_id": "my-feature",
  "phase": "requirements",
  "exit_code": 0,
  "error_kind": null,
  "error_reason": null,
  "model_full_name": "haiku",
  "runner": "native",
  "canonicalization_backend": "jcs-rfc8785",
  "outputs": [
    {
      "path": "artifacts/00-requirements.md",
      "blake3_canonicalized": "abc123..."
    }
  ],
  "warnings": []
}
```

### Example Status Output

```json
{
  "schema_version": "1",
  "emitted_at": "2025-10-24T14:30:00Z",
  "runner": "native",
  "canonicalization_backend": "jcs-rfc8785",
  "artifacts": [
    {"path": "artifacts/00-requirements.md", "blake3_first8": "abc12345"}
  ],
  "effective_config": {
    "model": {"value": "haiku", "source": "cli"},
    "max_turns": {"value": 6, "source": "config"}
  },
  "lock_drift": null
}
```

## Lockfile System

xchecker supports lockfiles for reproducibility tracking:

```bash
# Create lockfile during init
xchecker init my-feature --create-lock

# Detect drift in status
xchecker status my-feature --json  # Shows lock_drift if present

# Strict mode (fail on drift)
xchecker spec my-feature --strict-lock
xchecker resume my-feature --phase design --strict-lock
```

**Lockfiles pin:**
- Model full name (e.g., `haiku`)
- Claude CLI version (e.g., `0.8.1`)
- Schema version (e.g., `1`)

**Drift Detection:**
When a lockfile exists, xchecker compares current values against locked values and reports drift in status output. Use `--strict-lock` to fail execution on any drift.

## Fixup System

The fixup phase applies code changes from the review phase with validation and preview capabilities.

**Preview Mode (Default):**
```bash
xchecker resume my-feature --phase fixup
```

Shows:
- Intended target files
- Estimated line changes (+/-)
- Validation warnings
- No files are modified

**Apply Mode:**
```bash
xchecker resume my-feature --phase fixup --apply-fixups
```

Applies changes with:
- Path validation (prevents directory traversal)
- Diff validation (ensures changes are well-formed)
- Atomic file operations
- Backup creation before modification

**Status Integration:**
```bash
xchecker status my-feature
```

Shows pending fixups with:
- Number of target files
- Validation status
- Estimated changes
- Command to apply

## Performance Benchmarks

xchecker has been extensively profiled and optimized to meet NFR1 performance targets. All measurements significantly exceed requirements:

### Performance Targets vs Actual

| Metric | Target | Actual | Margin | Status |
|--------|--------|--------|--------|--------|
| Dry-run baseline | ≤ 5000ms | 68ms | 73x faster | ✅ PASS |
| Packetization (100 files) | ≤ 200ms | 13ms | 15x faster | ✅ PASS |
| JCS emission | ≤ 50ms | 0.5ms | 100x faster | ✅ PASS |
| Empty run | ≤ 5000ms | 16ms | 312x faster | ✅ PASS |

### Detailed Performance Characteristics

**Packet Assembly (100 files, ~1KB each):**
- Median: 13ms
- File selection: 17ms (includes directory walking, glob matching)
- File reading: 1ms (efficient sequential I/O)
- BLAKE3 hashing: <1ms (hardware-accelerated)
- Priority sorting: <1ms

**JCS Emission:**
- Receipt serialization: ~0.2ms
- Status serialization: ~0.3ms
- Doctor serialization: ~0.1ms
- Large status (100 artifacts): ~1.1ms

**Process Memory:**
- RSS: ~20MB
- Commit: ~12MB (Windows)

### Running Benchmarks

```bash
# Run all benchmarks
xchecker benchmark

# Custom benchmark parameters
xchecker benchmark --file-count 50 --iterations 10

# With custom thresholds
xchecker benchmark --max-empty-run-secs 3.0 --max-packetization-ms 150.0

# JSON output for automation
xchecker benchmark --json
```

For detailed performance analysis, see:
- [Packet Optimization Results](PACKET_OPTIMIZATION_RESULTS.md)
- [JCS Performance Results](JCS_PERFORMANCE_RESULTS.md)
- [Dry-Run Performance Results](DRY_RUN_PERFORMANCE_RESULTS.md)

## Security

xchecker includes comprehensive security validation with multiple layers of protection:

### Secret Detection and Redaction

**Default Secret Patterns:**
- GitHub Personal Access Tokens: `ghp_[A-Za-z0-9]{36}`
- AWS Access Keys: `AKIA[0-9A-Z]{16}`
- AWS Secret Keys: `AWS_SECRET_ACCESS_KEY=`
- Slack Tokens: `xox[baprs]-`
- Bearer Tokens: `Bearer [A-Za-z0-9._-]{20,}`

**Behavior:**
- Hard stop (exit code 8) when secrets detected in packet content
- Automatic redaction (replaced with `***`) in all output
- Secrets never written to disk except with explicit `--debug-packet` after successful scan
- Global redaction applied to all human-readable strings before persistence

**Configuration:**

```bash
# Ignore false positives
xchecker spec my-feature --ignore-secret-pattern "EXAMPLE_.*"

# Add custom patterns
xchecker spec my-feature --extra-secret-pattern "CUSTOM_KEY_[A-Z0-9]+"
```

### Path Validation

**Fixup System Security:**
- All paths canonicalized and validated to be under project root
- Directory traversal attacks prevented (rejects `..` components)
- Absolute paths outside root rejected
- Symlinks and hardlinks rejected by default (use `--allow-links` to override)
- Atomic file operations with backup creation

### Data Protection

**What's Never Persisted:**
- Environment variables (never included in receipts or logs)
- Raw packet content (only file lists with hashes)
- API keys or authentication tokens
- Unredacted stderr containing secrets

**What's Protected:**
- Stderr limited to 2048 bytes in receipts (after redaction)
- Error messages redacted before persistence
- Context strings redacted before logging
- Preview text redacted before display

### Debug Packet Safety

The `--debug-packet` flag writes full packet content for debugging, but with safeguards:

```bash
# Only writes if secret scan passes
xchecker spec my-feature --debug-packet

# Packet file excluded from receipts
# Packet content redacted if later reported
# No write if any secret rule fires
```

### Security Best Practices

1. **Never commit debug packets** - Add `context/*-packet.txt` to `.gitignore`
2. **Review custom patterns** - Test `--extra-secret-pattern` before production use
3. **Audit fixup targets** - Review preview before using `--apply-fixups`
4. **Use lockfiles** - Pin model versions with `--create-lock` for reproducibility
5. **Monitor receipts** - Check for unexpected warnings or error patterns

## Development

### Testing

xchecker uses a structured test suite organized into three profiles based on external dependencies. For complete test documentation, see [docs/TEST_MATRIX.md](docs/TEST_MATRIX.md).

#### Profile 1: Local-Green (Recommended for CI)

Fast, reliable tests with no external dependencies - no network, no Claude stub, no compiled binaries required.

**What it tests:**
- All library unit tests (595+ tests)
- Integration tests using dry-run mode (simulated LLM responses)
- Pure unit tests with no orchestrator calls
- Configuration, schema validation, and security tests
- LLM provider configuration and validation (see "New LLM Provider Tests" below)

**Command:**
```bash
# Run all local-green tests (791 tests, ~30 seconds)
cargo test --lib && cargo test --tests -- --skip requires_claude_stub
```

**Alternative (explicit skip list):**
```bash
cargo test --lib
cargo test --tests -- \
  --skip requires_claude_stub \
  --skip requires_real_claude \
  --skip requires_xchecker_binary \
  --skip requires_future_phase \
  --skip requires_future_api \
  --skip requires_refactoring \
  --skip windows_ci_only
```

**Expected results:**
- Test count: 791 tests (92.7% of total suite)
- Duration: ~30 seconds
- Requirements: Rust toolchain only

#### Profile 2: Stub Suite (Integration Testing)

Integration tests with full LLM mocking via the claude-stub binary.

**What it tests:**
- End-to-end workflows with mocked Claude responses
- Multi-phase orchestration with deterministic outputs
- Gate validation tests (M1, M3, M4)
- Golden pipeline tests

**Command:**
```bash
# Build claude-stub first
cargo build --bin claude-stub

# Run stub suite (49 additional tests)
cargo test --tests -- --include-ignored --skip requires_real_claude
```

**Expected results:**
- Test count: 840 tests (791 local-green + 49 stub)
- Duration: ~2-3 minutes
- Requirements: claude-stub binary (built from source)

#### Profile 3: Full Firehose (Complete Coverage)

Complete test suite including real Claude API integration tests (for nightly/on-demand runs only).

**What it tests:**
- Everything from Profiles 1 and 2
- Real Claude CLI integration
- API error handling and retry logic
- Network failure scenarios

**Prerequisites:**
- Claude CLI installed and authenticated
- Claude API key in environment
- Set `XCHECKER_ENABLE_REAL_CLAUDE=1`

**Command:**
```bash
XCHECKER_ENABLE_REAL_CLAUDE=1 cargo test --tests -- --include-ignored
```

**Expected results:**
- Test count: 853 tests (all tests)
- Duration: ~5-10 minutes
- Requirements: Real Claude API access, incurs API costs

**Warning:** This profile makes real API calls and incurs costs. Use sparingly for validation before releases.

For detailed instructions on running real LLM tests locally (including cost-effective Haiku configuration), see [tests/LLM_TESTING.md](tests/LLM_TESTING.md).

#### New LLM Provider Tests

The test file `tests/test_llm_provider_selection.rs` validates the new LLM provider configuration system:

**What it tests:**
- Provider validation (accepts `claude-cli`, rejects unsupported providers like `gemini-cli`, `openrouter`, `anthropic`)
- Execution strategy validation (only `controlled` accepted in V11-V14)
- Default provider behavior (`claude-cli` when unspecified)
- Case-sensitive provider names
- Actionable error messages with version constraints

**Examples:**
```bash
# Run provider configuration tests
cargo test --test test_llm_provider_selection

# Run specific provider validation test
cargo test test_provider_claude_cli_accepted
```

These tests ensure the multi-provider configuration stub works correctly in V11-V14 while reserving future providers (gemini-cli, openrouter, anthropic) for V15+.

#### Test Statistics

**Generated:** 2025-12-02 (see [TEST_MATRIX.md](docs/TEST_MATRIX.md) for latest)

- Total test files: 97
- Total test functions: 853
- Local-green tests: 791 (92.7%)
- Stub-dependent tests: 49 (5.7%)
- Real Claude tests: 4 (0.5%)
- Other ignored tests: 9 (1.1%)

#### Other Development Commands

```bash
# Check formatting
cargo fmt -- --check

# Run linter
cargo clippy -- -D warnings

# Build release
cargo build --release

# Run benchmarks
cargo bench

# Run specific test suite
cargo test --test m6_gate_validation

# Run with verbose output
cargo test -- --nocapture
```

### Orchestrator Integration

When integrating with the orchestrator from outside `src/orchestrator/`:

- **Use `OrchestratorHandle`** - the stable façade for CLI, Kiro, and MCP tools
- **Do not use `PhaseOrchestrator` directly** - reserved for internals and white-box tests

See `docs/ORCHESTRATOR.md` for the full API reference.

## Platform Support

xchecker provides comprehensive cross-platform support with automatic detection and fallback mechanisms.

### Supported Platforms

| Platform | Support Level | Runner Mode | Notes |
|----------|---------------|-------------|-------|
| **Linux** | ✅ Primary | Native | Full feature support, primary development platform |
| **macOS** | ✅ Full | Native | Complete feature parity with Linux |
| **Windows** | ✅ Full | Native + WSL | Native execution with automatic WSL fallback |
| **WSL** | ✅ Full | WSL | Automatic detection and path translation |

### Runner Modes

xchecker supports three runner modes for executing Claude CLI:

**Auto Mode (Default):**
```bash
xchecker spec my-feature  # Automatically selects best option
```
- On Linux/macOS: Always uses native Claude CLI
- On Windows: Tries native first, falls back to WSL if native unavailable
- Recommended for most users

**Native Mode:**
```bash
xchecker spec my-feature --runner-mode native
```
- Forces native Claude CLI execution
- Fails if Claude CLI not found in PATH
- Use when you want to ensure native execution

**WSL Mode (Windows only):**
```bash
xchecker spec my-feature --runner-mode wsl --runner-distro Ubuntu-22.04
```
- Forces execution through WSL
- Requires WSL installed with at least one distribution
- Automatically translates Windows paths to WSL format
- Useful when Claude CLI only available in WSL

### WSL Integration (Windows)

xchecker provides seamless WSL integration on Windows with automatic path translation and environment handling.

**Requirements:**
- Windows 10/11 with WSL 2
- At least one WSL distribution installed
- Claude CLI installed inside WSL distribution

**Path Translation:**
- Windows paths automatically converted: `C:\project` → `/mnt/c/project`
- Uses `wsl.exe wslpath -a` for canonical translation
- Fallback to `/mnt/<drive>/<path>` if wslpath unavailable
- UNC paths supported

**Environment Translation:**
- Environment variables adapted for WSL context
- Path variables converted to WSL format
- Context preserved across boundary

**Distribution Selection:**
```bash
# Use specific distribution
xchecker spec my-feature --runner-mode wsl --runner-distro Ubuntu-22.04

# Auto-detect from WSL_DISTRO_NAME environment variable
export WSL_DISTRO_NAME=Ubuntu-22.04
xchecker spec my-feature --runner-mode wsl

# List available distributions
wsl -l -q
```

**Artifacts and Receipts:**
- All artifacts persist in Windows spec root (not inside WSL)
- Receipts include `runner: "wsl"` and `runner_distro` fields
- No file duplication between Windows and WSL

**Validation:**
```bash
# Check WSL availability and Claude CLI
xchecker doctor

# Expected output on Windows with WSL:
# ✓ Claude CLI available (native)
# ✓ WSL available (Ubuntu-22.04, Debian)
# ✓ Claude CLI in WSL (Ubuntu-22.04: /usr/local/bin/claude)
```

### Process Control

**Timeout Enforcement:**
- Default: 600 seconds (10 minutes)
- Configurable: `--phase-timeout <seconds>` (minimum 5 seconds)
- Graceful termination: TERM signal → 5 second wait → KILL signal

**Platform-Specific Process Termination:**

**Windows:**
- Uses Job Objects for process tree termination
- Ensures all child processes terminated on timeout
- Handles Claude CLI spawning subprocesses

**Linux/macOS:**
- Uses process groups with `killpg`
- TERM signal sent to entire process group
- KILL signal after grace period

**WSL (Windows):**
- Uses `wsl.exe --exec` with discrete argv elements
- Job Objects applied to wsl.exe process
- Terminates entire WSL process tree

### Platform-Specific Features

**Windows:**
- Atomic rename with retry logic (handles antivirus interference)
- Exponential backoff up to 250ms total
- Retry count recorded in receipt warnings
- Job Objects for reliable process termination

**Linux/macOS:**
- Process group termination with killpg
- POSIX file mode bit preservation
- Native symlink handling

**All Platforms:**
- UTF-8 encoding with LF line endings
- CRLF tolerance on read (Windows)
- LF enforcement on write (all platforms)
- Cross-filesystem fallback for atomic operations

### Troubleshooting Platform Issues

**Windows: "Claude CLI not found"**
```bash
# Check if Claude CLI installed
where claude

# If not found, check WSL
wsl -l -q
wsl -d Ubuntu-22.04 -- which claude

# Use WSL mode if Claude only in WSL
xchecker spec my-feature --runner-mode wsl
```

**WSL: "Distribution not found"**
```bash
# List installed distributions
wsl -l -q

# Install a distribution
wsl --install -d Ubuntu-22.04

# Set default distribution
wsl --set-default Ubuntu-22.04
```

**WSL: "Claude CLI not found in WSL"**
```bash
# Install Claude CLI in WSL
wsl -d Ubuntu-22.04
# Inside WSL:
curl -fsSL https://claude.ai/install.sh | sh
```

**All Platforms: "Permission denied"**
```bash
# Check write permissions
xchecker doctor

# Verify XCHECKER_HOME is writable
ls -la $XCHECKER_HOME  # Linux/macOS
dir %XCHECKER_HOME%    # Windows
```

### Platform Testing

xchecker includes comprehensive platform-specific tests:

```bash
# Run all tests
cargo test

# Platform-specific tests (Windows)
cargo test --test test_windows_job_objects
cargo test --test test_wsl_runner

# Platform-specific tests (Linux/macOS)
cargo test --test test_unix_process_termination

# Cross-platform tests
cargo test --test test_cross_platform_line_endings
```

### CI/CD Integration

xchecker is tested on all platforms in CI:

- **Linux**: Ubuntu 20.04, 22.04
- **macOS**: macOS 12, 13, 14
- **Windows**: Windows Server 2019, 2022

See `.github/workflows/` for CI configuration examples.

## Documentation

For complete documentation, see [docs/INDEX.md](docs/INDEX.md).

### Walkthroughs

- [Running xchecker in 20 Minutes](docs/WALKTHROUGH_20_MINUTES.md) - Quick start guide for new users
- [From Spec to PR](docs/WALKTHROUGH_SPEC_TO_PR.md) - Complete workflow with Claude Code integration

### Reference

- [Configuration Guide](docs/CONFIGURATION.md) - Hierarchical configuration system
- [Doctor Command](docs/DOCTOR.md) - Environment health checks
- [JSON Contracts](docs/CONTRACTS.md) - Schema versioning and compatibility
- [Claude Code Integration](docs/CLAUDE_CODE_INTEGRATION.md) - Claude Code tool invocation and JSON outputs
- [Traceability](docs/TRACEABILITY.md) - Requirements traceability matrix
- [Test Matrix](docs/TEST_MATRIX.md) - Complete test inventory and classification

### Schema Files

- `schemas/receipt.v1.json` - Receipt schema definition
- `schemas/status.v1.json` - Status schema definition
- `schemas/doctor.v1.json` - Doctor schema definition

### Example Payloads

- `docs/schemas/receipt.v1.minimal.json` - Minimal receipt example
- `docs/schemas/receipt.v1.full.json` - Full receipt example
- `docs/schemas/status.v1.minimal.json` - Minimal status example
- `docs/schemas/status.v1.full.json` - Full status example
- `docs/schemas/doctor.v1.minimal.json` - Minimal doctor example
- `docs/schemas/doctor.v1.full.json` - Full doctor example

**Note:** All example JSON files in `docs/schemas/` are automatically generated from Rust struct constructors and validated against their schemas in CI. These examples are guaranteed to be valid and up-to-date with the current implementation. Do not edit these files manually - they will be regenerated by the test suite.

## Troubleshooting

### Common Issues

**"Claude CLI not found"**

```bash
# Check if Claude CLI is installed
claude --version

# If not installed, install from https://claude.ai/download

# On Windows, check WSL if native not found
xchecker doctor  # Will suggest WSL if available
```

**"Lock already held by another process" (exit code 9)**

```bash
# Check if another xchecker process is running
ps aux | grep xchecker  # Linux/macOS
tasklist | findstr xchecker  # Windows

# If process is stuck, use --force to break stale lock
xchecker spec my-feature --force

# Lock will auto-expire after TTL (default 15 minutes)
```

**"Packet overflow" (exit code 7)**

```bash
# Reduce packet size limits
xchecker spec my-feature --packet-max-bytes 32768 --packet-max-lines 600

# Exclude large files
# Edit .xchecker/config.toml:
[selectors]
exclude = ["target/**", "node_modules/**", "*.log", "*.bin"]

# Check packet manifest for details
cat .xchecker/specs/my-feature/context/requirements-packet.manifest.json
```

**"Secret detected" (exit code 8)**

```bash
# Review which pattern matched (secret itself not shown)
# Check receipt for error_reason

# If false positive, ignore the pattern
xchecker spec my-feature --ignore-secret-pattern "EXAMPLE_.*"

# Or remove the secret from your files before running
```

**"Phase timeout" (exit code 10)**

```bash
# Increase timeout (default 600s)
xchecker spec my-feature --phase-timeout 1200

# Check partial artifacts for progress
ls .xchecker/specs/my-feature/.partial/

# Resume from last successful phase
xchecker resume my-feature --phase design
```

**"Lockfile drift detected"**

```bash
# Check what changed
xchecker status my-feature --json | jq .lock_drift

# Update lockfile to current values
xchecker init my-feature --create-lock

# Or use --strict-lock to fail on drift
xchecker spec my-feature --strict-lock
```

**"Permission denied" errors**

```bash
# Check write permissions
xchecker doctor

# Verify XCHECKER_HOME is writable
ls -la .xchecker  # Linux/macOS
dir .xchecker     # Windows

# Try different location
export XCHECKER_HOME=/tmp/xchecker-test
xchecker spec my-feature
```

**"WSL not available" (Windows)**

```bash
# Check WSL installation
wsl -l -v

# Install WSL if needed
wsl --install

# Install a distribution
wsl --install -d Ubuntu-22.04

# Verify Claude CLI in WSL
wsl -d Ubuntu-22.04 -- which claude
```

**"Invalid configuration"**

```bash
# Validate config file
xchecker doctor

# Check for TOML syntax errors
cat .xchecker/config.toml

# Use explicit config path
xchecker spec my-feature --config /path/to/config.toml

# Reset to defaults (remove config file)
mv .xchecker/config.toml .xchecker/config.toml.bak
```

**"Fixup validation failed"**

```bash
# Preview fixups first
xchecker resume my-feature --phase fixup

# Check for path traversal attempts
# Review fixup targets in preview output

# Allow symlinks if needed
xchecker resume my-feature --phase fixup --apply-fixups --allow-links
```

**Performance issues**

```bash
# Run benchmarks to identify bottlenecks
xchecker benchmark --verbose

# Check if targets met
xchecker benchmark --json | jq .ok

# Reduce file count if packet assembly slow
# Edit .xchecker/config.toml:
[selectors]
include = ["src/**/*.rs", "docs/**/*.md"]  # Be more specific
```

### Documentation Validation Failures

If CI fails with documentation validation errors, follow these steps:

**"Command not found in CLI" or "Option not found":**
- The README documents a command or option that doesn't exist in the code
- Either add the missing implementation or remove the documentation
- Verify with: `cargo run -- <command> --help`

**"Exit code mismatch":**
- The exit codes table in README doesn't match `src/exit_codes.rs`
- Update the table to match the constants in code
- Ensure both the code number and name match exactly

**"Schema validation failed":**
- An example JSON file doesn't validate against its schema
- Run `cargo test --test schema_examples_tests -- --nocapture` to regenerate
- Commit the updated files in `docs/schemas/`

**"Enum mismatch" or "Required fields mismatch":**
- Schema definitions don't match Rust struct definitions
- Update the schema file in `schemas/*.json` to match the Rust struct
- Ensure enum values match with `#[serde(rename_all)]` applied
- Ensure required fields match non-`Option<T>` fields

**"TOML parse error":**
- A TOML example in CONFIGURATION.md has syntax errors or invalid fields
- Fix the TOML syntax or update field names to match `Config` struct
- Test with: `toml::from_str::<Config>("...")`

**"Example execution failed":**
- A shell command example in documentation fails to execute
- Update the example to work with current implementation
- Ensure the command works in stub mode with isolated XCHECKER_HOME

**"Generated examples are stale":**
- The files in `docs/schemas/*.json` are out of date
- Run: `cargo test --test schema_examples_tests -- --nocapture`
- Commit the regenerated files
- Never edit these files manually - they are auto-generated

**General debugging:**
```bash
# Run specific test module
cargo test --test doc_validation readme_tests -- --nocapture

# Run with verbose output
cargo test --test doc_validation -- --nocapture --test-threads=1

# Check what changed
git diff docs/schemas/
```

### Getting Help

If you encounter issues not covered here:

1. **Check the doctor output**: `xchecker doctor --json`
2. **Review receipts**: Check `.xchecker/specs/<spec-id>/receipts/` for error details
3. **Enable verbose logging**: Use `--verbose` flag for detailed output
4. **Check documentation**: See `docs/` directory for detailed guides
5. **Search issues**: Check GitHub issues for similar problems
6. **File a bug report**: Include doctor output, receipt, and verbose logs

## License

See [LICENSE](LICENSE) for details.

## Contributing

Contributions are welcome! Please ensure:

1. All tests pass: `cargo test`
2. Code is formatted: `cargo fmt`
3. No clippy warnings: `cargo clippy -- -D warnings`
4. Documentation is updated for new features
5. Schema changes follow the versioning policy (see [docs/CONTRACTS.md](docs/CONTRACTS.md))
6. Update CHANGELOG.md for user-facing changes

### Documentation Update Guidelines

When making changes to xchecker, ensure documentation stays synchronized:

**CLI Changes (commands, options, flags):**
- Update command documentation in this README
- Update the relevant section under "Commands"
- Add entry to CHANGELOG.md with the option name
- Run `cargo test --test doc_validation` to verify CLI documentation matches implementation

**Schema Changes (adding/removing fields, changing types):**
- Update the schema file in `schemas/*.json`
- Bump schema version if breaking (see [docs/CONTRACTS.md](docs/CONTRACTS.md))
- Add entry to CHANGELOG.md with field names
- Mark breaking changes with `[BREAKING]` in CHANGELOG
- Run `cargo test --test schema_examples_tests` to regenerate example files
- Commit the regenerated files in `docs/schemas/*.json`

**Configuration Changes (new config fields, defaults):**
- Update [docs/CONFIGURATION.md](docs/CONFIGURATION.md)
- Update example TOML blocks with new fields
- Document default values accurately
- Add entry to CHANGELOG.md

**Exit Code Changes:**
- Update the exit codes table in this README
- Update `src/exit_codes.rs` constants
- Add entry to CHANGELOG.md with code number and name

**Feature Changes:**
- Update relevant documentation (README, CONFIGURATION.md, DOCTOR.md, CONTRACTS.md)
- Ensure smoke tests exist for the feature
- Update CHANGELOG.md with user-facing description

**Verifying Documentation:**

All documentation is validated in CI via the `docs-conformance` job. To verify locally:

```bash
# Run all documentation validation tests
cargo test --test doc_validation -- --test-threads=1

# Regenerate schema examples
cargo test --test schema_examples_tests -- --nocapture

# Verify examples are fresh (should show no diff)
git diff --exit-code docs/schemas/
```

The documentation validation suite checks:
- CLI commands and options match clap definitions
- Exit codes match constants in code
- Configuration examples parse correctly
- Schema examples validate against schemas
- Code examples execute successfully
- Enums and required fields match between schemas and Rust structs

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for version history and migration guides.

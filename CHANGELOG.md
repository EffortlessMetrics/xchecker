# Changelog

All notable changes to xchecker will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Workspace and Project Commands**: You can now manage multiple specs in a single project with `project init`, `project add-spec`, `project status`, `project history`, and `project list`. This replaces manual spec juggling with a structured multi-spec workflow.
- **Templates**: `template init` and `template list` let you bootstrap new specs from built-in templates (Next.js, Rust Microservice, Python FastAPI, Docs Refactor), so you start with relevant defaults instead of blank files.
- **CI/CD Gates**: The `gate` command lets your CI pipeline enforce quality and completeness checks on specs before merging, with `--json` output for programmatic consumption.
- **Hooks**: You can now run custom scripts before or after any phase (e.g., linting artifacts, notifying a Slack channel) by configuring pre/post-phase hooks.
- **Gemini CLI Provider**: You can use Google's Gemini models as your LLM backend by setting `provider = "gemini-cli"`.
- **OpenRouter Provider**: You can access a wide range of models through OpenRouter's HTTP API, with budget limits (`budget` config or `XCHECKER_OPENROUTER_BUDGET` env var) to prevent unexpected costs.
- **Anthropic API Provider**: You can call Anthropic models directly over HTTP instead of going through the Claude CLI.
- **Provider Fallback**: If your primary LLM provider fails, xchecker automatically retries with a fallback provider (e.g., Claude -> OpenRouter), configurable via `fallback_provider`.
- **Richer Receipts**: Receipts now include token usage, model version, and cost metrics so you can track exactly what each phase consumed.
- **JSON Output for More Commands**: `--json` flag now works on `spec`, `status`, `resume`, `project`, and `gate` commands, making it easier to integrate xchecker into scripts and dashboards.
- **Budget Enforcement for HTTP Providers**: HTTP-based providers (OpenRouter, Anthropic) enforce strict budget limits so a runaway phase cannot exceed your configured spend.
- **Better Error Messages**: When something goes wrong, xchecker now suggests what to do about it (e.g., "Claude CLI not found -- run `xchecker doctor` to diagnose").
- **LLM Test Cost Control**: Set `XCHECKER_SKIP_LLM_TESTS=1` in CI to skip tests that call real LLM providers, avoiding unexpected API charges.

### Changed

- Reorganized documentation into tutorials, how-to guides, reference, and explanation categories following the Diataxis framework. All docs now live under `docs/tutorials/`, `docs/guides/`, `docs/reference/`, `docs/explanation/`, and `docs/contributor/`.
- Provider configuration now uses dedicated `[llm]` sub-tables per provider, giving you clearer control over provider-specific settings.

### Security

- Fixed a command injection vulnerability in the Gemini CLI backend where unsanitized input could execute arbitrary commands.
- Fixed a bug where running two xchecker instances simultaneously could corrupt lockfiles.
- Fixed a security issue where crafted file paths in fixups could read or write files outside your project directory.
- Fixed a command injection vulnerability where shell metacharacters in arguments could execute arbitrary commands during process execution.

## [1.1.0] - 2026-01-18

### Added

- **Multi-Provider Documentation**: The CLI and documentation now cover how to configure and switch between Gemini, OpenRouter, and Anthropic providers.
- **Smarter File Selection**: The packet builder now picks files by priority and respects configurable size limits, so your LLM context stays focused on the most relevant code.
- **Tamper-Evident Receipts**: Every phase execution produces a receipt with BLAKE3 hashes and canonical JSON (JCS/RFC 8785), letting you verify that artifacts have not been modified after generation. You can also list and read receipts programmatically.
- **Configuration Validation**: xchecker now validates your configuration on startup and reports exactly which setting came from which source (config file, environment variable, or default), making misconfiguration easier to diagnose.
- **Fixup Safety Checks**: Before applying fixups, xchecker validates target paths and reports any pending fixups, so you always know what changes are queued.
- **Developer Guide**: Added `CLAUDE.md` so AI coding agents (like Claude Code) understand the project structure and conventions when working in this repo.
- **Cross-Platform Claude CLI Detection**: The Claude CLI runner now automatically detects your platform (Linux, macOS, Windows, WSL) and configures itself accordingly, including output format and max turns.
- **Streaming Response Support**: xchecker can now parse NDJSON streaming responses from LLM providers, improving responsiveness during long phases.
- **Linux Compatibility**: Improved test workflows and CLI argument parsing for more reliable Linux support.

### Changed

- Reorganized and expanded the README, orchestrator docs, configuration reference, and LLM provider guide to be more comprehensive and easier to navigate.
- Streamlined internal crate structure and consolidated end-to-end test support for faster, more reliable test runs.
- Updated security-critical dependencies (Reqwest, Tokio) and added `libc` for Unix signal handling.

### Fixed

- Fixed integration test failures when running with `claude-stub`, so the stub test suite passes reliably.
- Fixed inconsistent config source labels that could show the wrong origin for a setting in diagnostic output.
- Strengthened artifact path validation to prevent edge cases where invalid paths could slip through.

## [1.0.1] - 2025-12-31

### Added

- **Debugging Guide**: Added a troubleshooting guide ([docs/guides/DEBUGGING.md](docs/guides/DEBUGGING.md)) covering common errors and how to inspect artifacts.
- **Workspace Guide**: Added a guide for managing multi-spec projects ([docs/guides/WORKSPACE.md](docs/guides/WORKSPACE.md)).
- **CI Profiles Guide**: Added documentation for setting up xchecker in CI ([docs/contributor/CI_PROFILES.md](docs/contributor/CI_PROFILES.md)) with cost analysis for each test tier.
- **Installation Scripts**: Added `scripts/install.ps1` (Windows) and `scripts/install.sh` (Linux/macOS) so you can install xchecker with a single command.
- **GitHub Templates**: Added issue and pull request templates so contributors get a consistent starting point.
- **Crates.io Packaging**: xchecker is now published to crates.io, so you can install it with `cargo install xchecker`.
- **Fuzzy Matching Test Coverage**: Added over 800 lines of edge-case tests for the fixup engine's fuzzy matching, catching subtle bugs in how near-miss patches are applied.
- **Documentation Validation**: Automated tests now verify that JSON schema examples in docs stay in sync with the actual schemas, so documentation never silently drifts from reality.
- **CI/CD Gate System**: The gate system now provides robust pass/fail enforcement for CI pipelines, so you can block merges when specs are incomplete.

### Changed

- **Better Configuration Loading**: Configuration loading is more resilient -- missing optional fields no longer cause cryptic errors, and every setting is attributed to its source.
- **Cross-Filesystem Fixups**: Fixups now work reliably when your project spans multiple drives or filesystems (e.g., Docker volumes, network mounts).
- **Improved Fuzzy Matching**: The fixup engine handles more edge cases when matching patches against files that have been edited since the LLM saw them.
- **More Robust Artifact Parsing**: The extraction module now handles malformed LLM output more gracefully instead of failing with unhelpful errors.
- **Minimum Rust Version**: Bumped to Rust 1.89. Older toolchains will get a clear error at build time.
- **WSL Interop**: Improved reliability of Windows/Linux interoperability when running via WSL.

### Fixed

- Fixed a bug where unusual LLM output formatting could cause guardrail checks to incorrectly reject valid responses.
- Updated and pinned security-critical dependencies to address known vulnerabilities.
- Fixed CI workflow issues that could cause intermittent release failures.

## [1.0.0] - 2025-12-05

First stable release of xchecker with complete spec generation workflow support.

### Highlights

- **Complete Spec Pipeline**: Generate structured specs through a multi-phase workflow (Requirements, Design, Tasks, Review, Fixup) with cryptographic audit trails so you can prove what was generated and when.
- **Strict Validation Mode**: Enable `strict_validation` to automatically reject low-quality LLM output instead of silently accepting it.
- **Problem Statement Persistence**: Your original feature description is automatically preserved and injected into every phase, so the LLM never loses sight of what you asked for.
- **Safe File Modifications**: The fixup engine uses fuzzy matching with explicit failure modes -- when a patch cannot be applied, you get actionable suggestions instead of silent corruption.
- **Stable JSON Schemas**: All JSON output follows versioned v1 contracts protected by property tests, so your CI scripts will not break on minor upgrades.
- **CI-Ready Output**: Every command supports `--json` output with documented gate patterns (smoke and strict modes) for pipeline integration.

### Core Features

- **Phase-Based Workflow**: Requirements -> Design -> Tasks -> Review -> Fixup -> Final
  - Each phase produces structured artifacts (Markdown + YAML)
  - Phases must run in order -- xchecker enforces dependencies
  - You can resume from any completed phase without re-running earlier ones
  - Artifacts are staged atomically via `.partial/` so a crash never leaves half-written files

- **Cross-Platform Execution**: Runs natively on Linux, macOS, and Windows
  - WSL mode with automatic path translation for Windows users
  - Auto mode tries native first, falls back to WSL
  - Configurable timeouts with graceful process termination

- **Smart Context Assembly**: The packet builder selects the most relevant files for each phase
  - Priority-based file selection so the LLM sees the most important code first
  - Configurable size limits (default: 64KB, 1200 lines) to stay within context windows
  - Exits with code 7 if your project exceeds limits, so you can adjust selectors

- **Secret Scanning**: Every packet is scanned before being sent to any LLM
  - Detects GitHub PATs, AWS keys, Slack tokens, Bearer tokens, and more
  - Add custom patterns via CLI flags for project-specific secrets
  - Exits with code 8 if secrets are found, blocking the send

- **Safe Fixup Engine**: LLM-proposed file changes are validated before application
  - Path validation prevents writes outside your project directory
  - Preview mode (default) shows what would change before applying
  - Atomic writes with backup so you can always roll back

- **Concurrent Execution Prevention**: A lock manager prevents two xchecker instances from modifying the same spec simultaneously
  - Stale lock detection via PID and TTL
  - Drift tracking for reproducibility audits

- **Versioned JSON Contracts (v1)**: All machine-readable output follows stable schemas
  - Receipt, Status, and Doctor schemas with `schema_version` field
  - JCS (RFC 8785) canonical emission with `emitted_at` timestamps
  - BLAKE3 hashes for artifact integrity verification
  - Structured `error_kind` and `error_reason` fields in receipts for programmatic error handling

### CLI

All commands support `--json` output and `--verbose` logging.

| Command | Description |
|---------|-------------|
| `spec <id>` | Generate spec through requirements |
| `resume <id> --phase <phase>` | Resume from phase |
| `status <id>` | Display spec status |
| `clean <id>` | Remove artifacts |
| `doctor` | Health checks |
| `init <id>` | Initialize spec |
| `benchmark` | Performance tests |

### Exit Codes

xchecker uses standardized exit codes for different failure modes:

| Code | Name | Description |
|------|------|-------------|
| `0` | SUCCESS | Operation completed successfully |
| `2` | CLI_ARGS | Invalid or missing command-line arguments |
| `7` | PACKET_OVERFLOW | Input packet exceeded size limits (default: 64KB, 1200 lines) |
| `8` | SECRET_DETECTED | Redaction system detected potential secrets in packet |
| `9` | LOCK_HELD | Another process is already working on the same spec |
| `10` | PHASE_TIMEOUT | Phase execution exceeded configured timeout (default: 600s) |
| `70` | CLAUDE_FAILURE | Underlying Claude CLI invocation failed |

### Configuration

Hierarchical system: CLI flags > config file > defaults

```toml
[defaults]
model = "haiku"
phase_timeout = 600

[selectors]
include = ["src/**/*.rs"]
exclude = ["target/**"]

[llm]
provider = "claude-cli"
execution_strategy = "controlled"
```

### Platform Support

| Platform | Status |
|----------|--------|
| Linux | Full support |
| macOS | Full support |
| Windows | Native + WSL |

### Performance

- Empty run: 16ms (target: 5000ms)
- Packetization (100 files): 10ms (target: 200ms)
- JCS emission: <50ms

## Schema Versioning Policy

- **v1 stability**: No breaking changes to v1 schemas
- **Additive only**: New optional fields may be added
- **6-month support**: After v2 release, v1 supported for 6+ months
- **JCS emission**: All JSON uses RFC 8785 canonical format

See [docs/reference/CONTRACTS.md](docs/reference/CONTRACTS.md) for details.

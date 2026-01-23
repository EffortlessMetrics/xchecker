# Changelog

All notable changes to xchecker will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Ecosystem Features (V16/V18)**:
  - **Workspace Management**: `project init`, `project add-spec`, `project status`, `project history`, and `project list` commands for multi-spec orchestration.
  - **Template System**: `template init` and `template list` commands with built-in templates (Next.js, Rust Microservice, Python FastAPI, Docs Refactor).
  - **Policy Gates**: `gate` command for CI/CD enforcement with JSON output support.
  - **Hooks System**: Pre/post-phase hooks configuration for custom automation.

- **LLM Provider Support (V11-V14)**:
  - **Gemini CLI**: Full support for Google's Gemini models via `gemini-cli` provider.
  - **OpenRouter**: HTTP-based provider support with budget control (`budget` config, `XCHECKER_OPENROUTER_BUDGET`).
  - **Anthropic API**: Direct HTTP support for Anthropic models.
  - **Fallback Logic**: Robust provider fallback chain (e.g., Claude -> OpenRouter) configurable via `fallback_provider`.
  - **Rich Metadata**: Receipts now include detailed token usage, model versions, and cost metrics.

- **Core Improvements**:
  - **JSON Output Modes**: `--json` flag support for `spec`, `status`, `resume`, `project`, and `gate` commands.
  - **Budget Control**: Strict budget enforcement for HTTP providers to prevent cost overruns.
  - **Error Reporting**: Enhanced `ErrorReporter` with actionable suggestions and context.
  - **Test Infrastructure**: `XCHECKER_SKIP_LLM_TESTS` for CI cost control.

### Security

- **Vulnerability Fixes**:
  - Fixed command injection in Gemini CLI backend.
  - Fixed race conditions in lockfile creation using atomic `create_new`.
  - Fixed path traversal vulnerabilities in `SandboxRoot` with canonicalization checks.
  - Fixed command injection in `ProcessRunner` by enforcing argv-style execution.

### Changed

- **Milestones**: Renamed legacy "V11-V18" milestones to semantic names (Core Runtime, Gemini Support, HTTP Providers, Ecosystem).
- **Configuration**: Expanded `[llm]` configuration section with provider-specific tables.

## [1.1.0] - 2026-01-18

### Added

- **Multi-Provider Support**: Documentation and CLI updates for Gemini, OpenRouter, and Anthropic support (V14 availability).
- **Packet Construction**: New system with priority-based file selection and configurable size limits.
- **Receipt System**: Enhanced JCS (RFC 8785) serialization and BLAKE3 hashing for execution audit trails; added comprehensive reading/listing APIs.
- **Configuration Model**: Comprehensive configuration model with validation, hooks support, and unified `ConfigSource` attribution.
- **Fixup Validation**: Added validation for fixup targets and pending fixups reporting.
- **Unified Phase Execution**: Introduced `execute_phase_core()` for consistent phase execution.
- **Developer Guide**: Added `CLAUDE.md` for AI agent guidance and project overview.
- **Engine Tests**: Added comprehensive engine invariant tests (Suite B3.7-B3.14).
- **Claude CLI Improvements**: Cross-platform runner with automatic detection, output format configuration, and max turns support.
- **NDJSON Support**: Implemented NDJSON parsing functionality for streaming responses.
- **Security Boundaries**: Unified security boundaries and centralized pending fixups handling.
- **Linux Compatibility**: Enhanced test workflows and CLI parsing for robust Linux support.

### Changed

- **Documentation**: Overhauled `README.md`, `ORCHESTRATOR.md`, `CONFIGURATION.md`, and `LLM_PROVIDERS.md`.
- **Refactoring**: Streamlined `xchecker-utils`, consolidated E2E test support, and improved configuration source handling.
- **Dependencies**: Updated security-critical dependencies (Reqwest, Tokio) and added `libc` for Unix support.

### Fixed

- **CI/Tests**: Resolved integration test failures with `claude-stub`.
- **Configuration**: Fixed inconsistency in `ConfigSource` naming (standardized to `Default`).
- **Path Validation**: Enhanced artifact path validation and canonicalization.

## [1.0.1] - 2025-12-31

### Added

- **Documentation Suite**: Added `DEBUGGING_GUIDE.md`, `WORKSPACE_GUIDE.md`, and `CI_PROFILES.md` to provide comprehensive operational guidance.
- **Installation Scripts**: Added `scripts/install.ps1` and `scripts/install.sh` for streamlined cross-platform setup.
- **Community**: Added GitHub issue and Pull Request templates (`.github/ISSUE_TEMPLATE`, `.github/pull_request_template.md`).
- **Packaging**: Included `crates.io` packaging specifications in project metadata.
- **Testing Infrastructure**: 
  - Added comprehensive fuzzy matching tests (`tests/test_fixup_fuzzy_edge_cases.rs`) covering over 800 lines of scenarios.
  - Introduced `tests/doc_validation` for automated schema and documentation verification.
- **Gate System**: Robust implementation of gating logic (`src/gate.rs`) to support CI/CD integration and status checks.

### Changed

- **Configuration System**: Major expansion of configuration loading and validation logic (Dec 6 updates), improving resilience and attribution.
- **Fixup Engine**: 
  - Improved cross-filesystem support for reliable fixes across different drive types.
  - Enhanced fuzzy matching to handle complex edge cases and near-misses.
- **Extraction Module**: Significant refactoring for better error handling and robustness in artifact parsing.
- **Rust Version**: Bumped minimum supported Rust version to 1.89.
- **WSL Runner**: Improved WSL runner test assertions and validation logic for better Windows/Linux interoperability.

### Fixed

- **Guardrails**: Fixed guardrails parsing logic for better resilience against LLM output variations (bot feedback).
- **Dependencies**: Updated and pinned security-critical dependencies (Dec 31).
- **Workflow**: Resolved issues in CI workflows and `release.yml`.

## [1.0.0] - 2025-12-05

First stable release of xchecker with complete spec generation workflow support.

### Highlights

- **Spec Pipeline with Receipts**: Complete multi-phase workflow (Requirements → Design → Tasks → Review → Fixup) with cryptographic audit trails
- **Strict Validation Mode**: Configurable validation for low-quality LLM output with `strict_validation` flag
- **Problem Statement Persistence**: Original input preserved and injected into packets and prompts automatically
- **Safe Fixup Engine**: Fuzzy matching with explicit failure modes (`FuzzyMatchFailed` with actionable suggestions)
- **Schema Versioning**: JSON v1 contracts protected by property tests and versioning guards
- **Gateable JSON Output**: CI-ready `--json` output with documented gate patterns (smoke and strict modes)

### Core Features

- **Phase-Based Workflow**: Requirements -> Design -> Tasks -> Review -> Fixup -> Final
  - Structured artifacts (Markdown + YAML) per phase
  - Phase dependencies enforced by orchestrator
  - Resume from any completed phase
  - Atomic artifact promotion via `.partial/` staging

- **Runner System**: Cross-platform process execution
  - Native mode: Linux, macOS, Windows
  - WSL mode: Automatic path translation
  - Auto mode: Native first, WSL fallback
  - Timeout enforcement with graceful termination

- **Packet Builder**: Deterministic payload assembly
  - Priority-based file selection
  - Configurable size limits (default: 64KB, 1200 lines)
  - Exit code 7 on overflow

- **Secret Redaction**: Pre-invocation security
  - GitHub PAT, AWS keys, Slack tokens, Bearer tokens
  - Custom patterns via CLI flags
  - Exit code 8 on detection

- **Fixup Engine**: Safe file modification
  - Path validation and canonicalization
  - Preview mode by default
  - Atomic writes with backup

- **Lock Manager**: Concurrent execution prevention
  - Stale detection via PID/TTL
  - Drift tracking for reproducibility

- **JSON Contracts (v1)**: Versioned schemas
  - Receipt, Status, Doctor schemas with `schema_version` field
  - JCS (RFC 8785) canonical emission with `emitted_at` timestamps
  - BLAKE3 hashing for artifact verification
  - `error_kind` and `error_reason` fields for structured error reporting in receipts

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

See [CONTRACTS.md](docs/CONTRACTS.md) for details.

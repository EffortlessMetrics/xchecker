# xchecker Documentation Index

This index provides navigation to all xchecker documentation.

## Getting Started

- [README](../README.md) - Main project overview, installation, and quick start

## Walkthroughs

- [WALKTHROUGH_20_MINUTES.md](WALKTHROUGH_20_MINUTES.md) - Running xchecker on your repo in 20 minutes
- [WALKTHROUGH_SPEC_TO_PR.md](WALKTHROUGH_SPEC_TO_PR.md) - From spec to PR: xchecker + Claude Code flow

## Configuration & Setup

- [CONFIGURATION.md](CONFIGURATION.md) - Hierarchical config system, XCHECKER_HOME, all options
- [LLM_PROVIDERS.md](LLM_PROVIDERS.md) - LLM provider configuration, authentication, testing, and cost control
- [DOCTOR.md](DOCTOR.md) - Environment health checks and diagnostics
- [TROUBLESHOOTING.md](TROUBLESHOOTING.md) - Common issues and stable fixes

## Integration

- [CLAUDE_CODE_INTEGRATION.md](CLAUDE_CODE_INTEGRATION.md) - Claude Code integration guide, JSON outputs, tool invocation model

## Architecture & Design

- [ORCHESTRATOR.md](ORCHESTRATOR.md) - Core execution engine, phase flow, workflow orchestration
- [STRUCTURED_LOGGING.md](STRUCTURED_LOGGING.md) - Tracing-based logging implementation
- [CONTRACTS.md](CONTRACTS.md) - JSON schema versioning policy and stability guarantees

## Testing

- [TEST_MATRIX.md](TEST_MATRIX.md) - Complete test inventory, local-green readiness
- [TESTING.md](TESTING.md) - Test lanes, property-based testing, CI integration
- [CI_PROFILES.md](CI_PROFILES.md) - CI test profiles (Local-Green, Stub Suite, Firehose)
- [claude-stub.md](claude-stub.md) - Test harness documentation and scenarios

## CI/CD Integration

- [ci/gitlab.md](ci/gitlab.md) - GitLab CI configuration for xchecker gate
- [GitHub Actions](../.github/workflows/xchecker-gate.yml) - Example GitHub Actions workflow

## Security & Performance

- [SECURITY.md](SECURITY.md) - Secret detection, redaction patterns, path validation
- [PERFORMANCE.md](PERFORMANCE.md) - Benchmarking methodology, NFR targets, optimization

## Platform Support

- [PLATFORM.md](PLATFORM.md) - Cross-platform support (Linux, macOS, Windows, WSL)

## Requirements & Traceability

- [TRACEABILITY.md](TRACEABILITY.md) - Requirements traceability matrix
- [REQUIREMENTS_RUNTIME_V1.md](REQUIREMENTS_RUNTIME_V1.md) - Runtime requirements (V1-V10)

## Development Audit Logs

The repository includes audit trails of xchecker's own development:

- [.xchecker/](../.xchecker/README.md) - xchecker specs, receipts, and artifacts generated during development
- [.kiro/](../.kiro/README.md) - Kiro AI specification files that guided implementation

These directories document the AI-assisted development process and serve as real-world examples of xchecker output formats.

## Schema Examples

The `schemas/` directory contains auto-generated JSON examples:
- `receipt.v1.*.json` - Receipt format examples
- `status.v1.*.json` - Status format examples
- `doctor.v1.*.json` - Doctor format examples

> **Note**: Schema files are auto-generated. Never edit manually.

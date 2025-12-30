# xchecker Documentation

## Getting Started

| Document | Description |
|----------|-------------|
| [README](../README.md) | Overview, installation, and quick start |
| [WALKTHROUGH_20_MINUTES.md](WALKTHROUGH_20_MINUTES.md) | Get running in 20 minutes |
| [WALKTHROUGH_SPEC_TO_PR.md](WALKTHROUGH_SPEC_TO_PR.md) | From spec to PR workflow |

## Configuration

| Document | Description |
|----------|-------------|
| [CONFIGURATION.md](CONFIGURATION.md) | Full configuration reference |
| [LLM_PROVIDERS.md](LLM_PROVIDERS.md) | LLM provider setup and options |
| [DOCTOR.md](DOCTOR.md) | Environment health checks |
| [TROUBLESHOOTING.md](TROUBLESHOOTING.md) | Common issues and fixes |
| [DEBUGGING_GUIDE.md](DEBUGGING_GUIDE.md) | Debugging workflows and diagnostics |
| [WORKSPACE_GUIDE.md](WORKSPACE_GUIDE.md) | Workspace setup and TUI usage |

## Architecture

| Document | Description |
|----------|-------------|
| [ORCHESTRATOR.md](ORCHESTRATOR.md) | Core execution engine |
| [CONTRACTS.md](CONTRACTS.md) | JSON schema versioning |
| [STRUCTURED_LOGGING.md](STRUCTURED_LOGGING.md) | Tracing-based logging |

## Testing

| Document | Description |
|----------|-------------|
| [TESTING.md](TESTING.md) | Test lanes and CI profiles |
| [TEST_MATRIX.md](TEST_MATRIX.md) | Complete test inventory |
| [CI_PROFILES.md](CI_PROFILES.md) | CI test configuration |
| [claude-stub.md](claude-stub.md) | Test harness documentation |

## Security & Performance

| Document | Description |
|----------|-------------|
| [SECURITY.md](SECURITY.md) | Secret detection, redaction, and path validation |
| [PERFORMANCE.md](PERFORMANCE.md) | Benchmarking and optimization |
| [PLATFORM.md](PLATFORM.md) | Cross-platform support |

## CI/CD Integration

| Document | Description |
|----------|-------------|
| [ci/gitlab.md](ci/gitlab.md) | GitLab CI configuration |

## Reference

| Document | Description |
|----------|-------------|
| [TRACEABILITY.md](TRACEABILITY.md) | Requirements traceability |
| [REQUIREMENTS_RUNTIME_V1.md](REQUIREMENTS_RUNTIME_V1.md) | Runtime requirements |
| [CLAUDE_CODE_INTEGRATION.md](CLAUDE_CODE_INTEGRATION.md) | Claude Code integration |

## Schemas

JSON schema examples in `docs/schemas/`:

- `receipt.v1.*.json` - Execution receipt format
- `status.v1.*.json` - Spec status format
- `doctor.v1.*.json` - Health check format

Schema definitions in `schemas/`:

- `receipt.v1.json` - Receipt schema
- `status.v1.json` - Status schema
- `doctor.v1.json` - Doctor schema

> **Note**: Example files are auto-generated. Do not edit manually.

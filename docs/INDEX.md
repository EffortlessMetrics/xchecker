# xchecker Documentation

This documentation follows the [Diataxis](https://diataxis.fr/) framework, organized into four categories based on user needs.

## Tutorials

Learning-oriented guides that take you through a complete workflow.

| Document | Description |
|----------|-------------|
| [README](../README.md) | Project overview and installation |
| [WALKTHROUGH_20_MINUTES.md](WALKTHROUGH_20_MINUTES.md) | Get running in 20 minutes |
| [WALKTHROUGH_SPEC_TO_PR.md](WALKTHROUGH_SPEC_TO_PR.md) | Complete workflow: spec to pull request |

## How-to Guides

Task-oriented guides for specific goals.

| Document | Description |
|----------|-------------|
| [DEBUGGING_GUIDE.md](DEBUGGING_GUIDE.md) | Troubleshoot errors and inspect artifacts |
| [DOCTOR.md](DOCTOR.md) | Run environment health checks |
| [WORKSPACE_GUIDE.md](WORKSPACE_GUIDE.md) | Set up workspaces and use the TUI |
| [PLATFORM.md](PLATFORM.md) | Set up on Windows, macOS, Linux, and WSL |
| [CLAUDE_CODE_INTEGRATION.md](CLAUDE_CODE_INTEGRATION.md) | Integrate with Claude Code editor |
| [ci/gitlab.md](ci/gitlab.md) | Configure GitLab CI pipelines |

## Reference

Technical specifications and configuration options.

| Document | Description |
|----------|-------------|
| [CONFIGURATION.md](CONFIGURATION.md) | Full configuration reference |
| [LLM_PROVIDERS.md](LLM_PROVIDERS.md) | LLM provider setup and options |
| [CONTRACTS.md](CONTRACTS.md) | JSON schema versioning policy |
| [TESTING.md](TESTING.md) | Test strategy and profiles |
| [TEST_MATRIX.md](TEST_MATRIX.md) | Complete test inventory |
| [CI_PROFILES.md](CI_PROFILES.md) | CI test configuration |
| [STRUCTURED_LOGGING.md](STRUCTURED_LOGGING.md) | Tracing-based logging reference |
| [claude-stub.md](claude-stub.md) | Test harness reference |

### JSON Schemas

Schema definitions in `schemas/`:

| Schema | Description |
|--------|-------------|
| `receipt.v1.json` | Execution receipt format |
| `status.v1.json` | Spec status format |
| `doctor.v1.json` | Health check format |

Example files in `docs/schemas/` (auto-generated, do not edit manually).

## Explanation

Background and architectural context.

| Document | Description |
|----------|-------------|
| [ORCHESTRATOR.md](ORCHESTRATOR.md) | Core execution engine architecture |
| [SECURITY.md](SECURITY.md) | Security model, secret detection, and path validation |
| [PERFORMANCE.md](PERFORMANCE.md) | Performance characteristics and benchmarks |
| [TRACEABILITY.md](TRACEABILITY.md) | Requirements traceability matrix |
| [REQUIREMENTS_RUNTIME_V1.md](REQUIREMENTS_RUNTIME_V1.md) | Runtime requirements specification |

## For Contributors

| Document | Description |
|----------|-------------|
| [DEVELOPER_NOTES.md](DEVELOPER_NOTES.md) | Common development issues and fixes |
| [DEPENDENCY_MANAGEMENT.md](DEPENDENCY_MANAGEMENT.md) | Dependency update policies |
| [architecture/dependency-policy.md](architecture/dependency-policy.md) | Crate layering rules |

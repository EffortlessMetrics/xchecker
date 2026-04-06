# xchecker Documentation

## Start Here

Use this path for the first 10 minutes in a real repo:

1. Run `xchecker doctor` to confirm your provider, config, and workspace are ready.
2. Generate your first spec with `xchecker spec my-feature`.
3. Inspect progress and artifacts with `xchecker status my-feature`.
4. Continue the next phase with `xchecker resume my-feature --phase design`.

Canonical docs live under `tutorials/`, `guides/`, `reference/`, `explanation/`, and `contributor/`.
Top-level files such as `docs/CONFIGURATION.md` remain as compatibility redirects.

## Tutorials

Learn xchecker by doing.

| Guide | Time | Description |
|-------|------|-------------|
| [Quickstart](tutorials/QUICKSTART.md) | 20 min | Install, configure, and generate your first spec |
| [Spec to PR](tutorials/SPEC_TO_PR.md) | 45 min | Complete workflow from feature idea to pull request |

## How-to Guides

Solve specific problems.

| Guide | Description |
|-------|-------------|
| [Configuration](guides/CONFIGURATION.md) | Configure xchecker for your project |
| [LLM Providers](guides/LLM_PROVIDERS.md) | Set up Claude, Gemini, OpenRouter, or Anthropic API |
| [Security](guides/SECURITY.md) | Secret scanning, custom patterns, best practices |
| [Debugging](guides/DEBUGGING.md) | Troubleshoot errors and inspect artifacts |
| [CI Setup](guides/CI_SETUP.md) | Set up xchecker in GitHub Actions or GitLab CI |
| [Platform Setup](guides/PLATFORM.md) | Windows, macOS, Linux, and WSL configuration |
| [Workspaces](guides/WORKSPACE.md) | Manage multi-spec projects |
| [Claude Code Integration](guides/CLAUDE_CODE.md) | Use xchecker from Claude Code |
| [Health Checks](guides/DOCTOR.md) | Run and interpret `xchecker doctor` |

## Reference

Look things up.

| Document | Description |
|----------|-------------|
| [CLI Reference](reference/CLI.md) | Commands, options, and exit codes |
| [Configuration Reference](reference/CONFIGURATION.md) | All config keys, env vars, and defaults |
| [JSON Contracts](reference/CONTRACTS.md) | Schema versioning and stability guarantees |
| [Schemas](reference/SCHEMAS.md) | JSON schema file index |
| [Structured Logging](reference/STRUCTURED_LOGGING.md) | Log fields and filtering |

## Explanation

Understand the system.

| Document | Description |
|----------|-------------|
| [Architecture](explanation/ARCHITECTURE.md) | How xchecker works: pipeline, concepts, safety model |
| [Security Model](explanation/SECURITY_MODEL.md) | Defense-in-depth implementation details |
| [Performance](explanation/PERFORMANCE.md) | Benchmarks, targets, and optimization |

## Contributor Docs

For xchecker developers.

| Document | Description |
|----------|-------------|
| [Contributor Index](contributor/INDEX.md) | Entry point for local development and architecture docs |
| [Orchestrator Internals](contributor/ORCHESTRATOR_INTERNALS.md) | Engine module architecture and invariants |
| [Testing](contributor/TESTING.md) | Test lanes, profiles, and infrastructure |
| [Test Matrix](contributor/TEST_MATRIX.md) | Complete test inventory (853 tests) |
| [CI Profiles](contributor/CI_PROFILES.md) | CI configuration and cost analysis |
| [Developer Notes](contributor/DEVELOPER_NOTES.md) | Common dev issues and fixes |
| [Dependency Management](contributor/DEPENDENCY_MANAGEMENT.md) | Dependency update policies |
| [Dependency Policy](contributor/dependency-policy.md) | Crate layering rules |
| [Traceability](contributor/TRACEABILITY.md) | Requirements traceability matrix |
| [Runtime Requirements](contributor/REQUIREMENTS_RUNTIME_V1.md) | V1 runtime specification |
| [Security Gate Review](contributor/SECURITY_GATE_REVIEW.md) | Security audit trail |
| [Claude Stub](contributor/claude-stub.md) | Test harness reference |

## Project

| Document | Description |
|----------|-------------|
| [Roadmap](ROADMAP.md) | Now / Next / Later priorities |

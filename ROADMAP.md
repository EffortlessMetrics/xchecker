# xchecker Roadmap

Last updated: March 2026

This roadmap is organized by commitment level, not by version number. Items move from Later to Next to Now as they are prioritized and staffed. If something here matters to you, open an issue -- it influences priority.

---

## Now (active work)

These are in progress and expected to land in the next release.

- **Documentation rework**: Restructuring all docs into a problem-first Diataxis layout (tutorials, guides, reference, explanation). The goal is to make it possible to find the right answer within two clicks from the README.

- **Hooks documentation completion**: The hooks system (pre/post-phase shell scripts) shipped in v1.1.0 but the configuration reference is incomplete. Filling in the guides with examples for common workflows: linting before fixup, notifications after review, custom validation.

- **Config test environment isolation**: Ensuring config-layer tests do not leak state across test runs. Currently some tests share a global config directory, which causes flaky failures when run in parallel.

- **`--llm-gemini-binary` CLI flag**: Custom Gemini CLI binary path override, matching the existing `--llm-claude-binary` flag. Required for environments where the Gemini CLI is not on `$PATH`.

---

## Next (committed, not yet started)

Design work is done or nearly done. These will move to Now once current work ships.

- **ExternalTool execution strategy**: A second execution mode where LLMs directly write files in agentic workflows, bypassing the fixup pipeline. The controlled strategy remains the default; ExternalTool is opt-in for users who want the LLM to operate more autonomously.

- **Custom prompt templates**: User-defined prompt templates beyond the built-in presets (nextjs, rust, python). Users will be able to define templates in `.xchecker/templates/` with custom system prompts, file selectors, and phase-specific instructions.

- **Workspace-level lockfiles**: Track dependency and model drift across all specs in a multi-spec project. When a model version changes, all specs in the workspace are flagged for re-evaluation.

- **Phase streaming**: Stream LLM output to the TUI during long-running phases instead of waiting for completion. This gives immediate feedback on whether the LLM is heading in the right direction.

---

## Later (under consideration)

These are ideas we think are worth pursuing but have not committed to. Feedback welcome.

- Remote execution mode -- run phases on a build server, collect results locally. Useful for teams that want to centralize LLM costs.
- Plugin system for custom phases beyond the built-in six. Let teams define domain-specific pipeline steps (e.g., threat modeling, compliance review).
- Multi-language template packs (Go, Java, Swift, etc.) with language-specific prompt tuning.
- Parallel phase execution for independent pipeline branches. Some phases (e.g., review and tasks) could run concurrently in certain workflows.
- Web dashboard for workspace-level visibility across multiple specs and team members.

---

## Completed

| Version | Date | Highlights |
|---------|------|------------|
| v1.1.0 | Jan 2026 | Multi-provider LLM support (Claude CLI, Gemini CLI, OpenRouter, Anthropic API), workspaces, templates, gates, hooks, rich receipt metadata |
| v1.0.0 | Dec 2025 | Core 6-phase pipeline, secret redaction, atomic writes, JSON contracts, cross-platform runner, lock manager |

See [CHANGELOG.md](CHANGELOG.md) for detailed release notes.

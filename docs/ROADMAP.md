# Roadmap

Now / Next / Later priorities for xchecker. Updated 2026-03-31.

Items move right-to-left as they are staffed. If something here matters to you, open an issue.

---

## Now (Active)

- **CI stabilization** -- all three platforms green on nightly builds
- **Doctor command family** -- `doctor tests`, `doctor release`, `doctor ci`
- **Failure taxonomy and CI dossier output** -- structured classification of CI failures
- **Route receipts for change-decision auditing** -- track why each change was accepted or rejected
- **Release surface validation** -- verify public API before publish

---

## Next (Planned)

- `xchecker ci diagnose <run-id>` -- compress CI failure logs into actionable summaries
- `xchecker dossier` -- generate pre-PR incident artifacts
- `xchecker receipt diff` -- compare receipts across runs
- `xchecker fixup preflight` -- dry-run fixup validation before apply
- `xchecker packet doctor` -- packet health checks and budget analysis

---

## Later (Exploring)

- **Internal routing** -- overflow, validation, and lock seam extraction
- **Trace graph** -- requirement to task to review to fixup lineage
- `xchecker doctor docs` -- delegation-safety linting for documentation
- **Plugin system** -- custom phases beyond the built-in six
- **Multi-spec dependency tracking** -- cross-spec ordering and invalidation

---

## Completed

| Version | Date | Highlights |
|---------|------|------------|
| v1.1.0 | Jan 2026 | Multi-provider LLM support, workspaces, templates, gates, hooks |
| v1.0.0 | Dec 2025 | Core 6-phase pipeline, secret redaction, atomic writes, JSON contracts |

See [CHANGELOG.md](../CHANGELOG.md) for detailed release notes.

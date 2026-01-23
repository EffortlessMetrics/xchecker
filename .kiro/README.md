# Kiro Development Audit Log

This directory contains Kiro AI specification files used during xchecker's development.

## Contents

- `specs/` - Implementation specs for major development phases (each with requirements.md, design.md, tasks.md)
- `steering/` - Product, structure, and tech guidelines
- `ROADMAP.md` - V11-V18 implementation record (historical)
- `*.md` - Status snapshots and reference docs

## Specs Index

| Spec | Purpose | Status |
|------|---------|--------|
| `xchecker-runtime-implementation/` | Core phase pipeline, runner, packet, fixup, receipts | ✅ Complete |
| `xchecker-claude-orchestrator/` | Orchestrator, phase system, LLM integration | ✅ Complete |
| `xchecker-llm-ecosystem/` | V11-V18 multi-provider LLM support | ✅ Complete |
| `xchecker-operational-polish/` | Test fixes, warnings cleanup, benchmark, contracts | ✅ Complete |
| `xchecker-final-cleanup/` | Test stability, code annotations, hooks | ✅ Complete |
| `crates-io-packaging/` | Library API, crates.io packaging, security hardening | ✅ Complete |
| `documentation-validation/` | Schema validation, doc tests, example generators | ✅ Complete |

## Navigation

- **Project Roadmap**: See [../ROADMAP.md](../ROADMAP.md) at repo root
- **End-User Docs**: See [../docs/](../docs/)
- **Development Guide**: See [../CLAUDE.md](../CLAUDE.md)

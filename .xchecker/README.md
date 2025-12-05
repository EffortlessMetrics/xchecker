# xchecker Audit Log

This directory contains the audit trail of xchecker's own development - specs, receipts, and artifacts generated while building xchecker itself.

## Purpose

These files are intentionally committed to demonstrate:
- Real-world xchecker output formats
- Evolution of the spec generation workflow
- Actual receipts and artifacts from development phases

## Contents

- `config.toml` - Project configuration for xchecker
- `specs/` - Generated specs from development iterations
  - Each spec contains `artifacts/`, `context/`, and `receipts/` subdirectories
  - Receipts include timestamps, model info, and execution metadata
- `cache/` - InsightCache for file summaries (may be gitignored)

## Note

This is **not** test fixture data. Tests create their own temporary directories.
This directory serves as a living audit log of xchecker's development history.

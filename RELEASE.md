# Release Guide

This document describes the process for publishing xchecker crates to crates.io.

## Pre-Release Checklist

Before publishing, ensure all gates pass:

```bash
# 1. Format check
cargo fmt --all -- --check

# 2. Lint check
cargo clippy --workspace --all-targets --all-features -- -D warnings

# 3. Test suite (lib and unit tests)
cargo test --workspace --lib --bins

# 4. Integration tests (skip external dependencies)
cargo test --workspace --tests -- --skip requires_claude_stub --skip requires_real_claude --skip requires_xchecker_binary

# 5. Version alignment check
grep -r "version = \"1.1.0\"" Cargo.toml crates/*/Cargo.toml | head -30
```

## Crate Dependency Tiers

Crates must be published in dependency order. Lower tiers must be published before higher tiers.

### Tier 1: Leaf Crates (No Internal Dependencies)
These have zero internal xchecker dependencies:

| Crate | Description |
|-------|-------------|
| `xchecker-extraction` | Content extraction utilities |
| `xchecker-fixup-model` | Types for fixup model |
| `xchecker-redaction` | Secret detection |
| `xchecker-runner` | Process execution |
| `xchecker-lock` | File locking |

### Tier 2: Foundation Crate
The foundation crate most others depend on:

| Crate | Depends On |
|-------|------------|
| `xchecker-utils` | redaction, lock, runner |

### Tier 3: Low-Tier Crates
Depend only on Tier 1-2:

| Crate | Key Dependencies |
|-------|------------------|
| `xchecker-error-redaction` | utils |
| `xchecker-error-reporter` | utils, redaction |
| `xchecker-prompt-template` | utils |
| `xchecker-selectors` | utils |
| `xchecker-templates` | utils |
| `xchecker-validation` | utils |
| `xchecker-workspace` | utils |

### Tier 4: Mid-Tier Crates
Depend on Tier 1-3:

| Crate | Key Dependencies |
|-------|------------------|
| `xchecker-receipt` | utils, redaction |
| `xchecker-config` | utils, redaction, prompt-template, selectors |
| `xchecker-packet` | utils, config, redaction |
| `xchecker-status` | utils, config, redaction, receipt |
| `xchecker-gate` | utils, receipt |
| `xchecker-doctor` | utils, config |
| `xchecker-llm` | utils, runner, config, error-redaction |
| `xchecker-phase-api` | packet, status, selectors, redaction, utils |
| `xchecker-hooks` | utils, config, runner, redaction |
| `xchecker-benchmark` | utils, packet |

### Tier 5: High-Tier Crates
Depend on most other crates:

| Crate | Key Dependencies |
|-------|------------------|
| `xchecker-phases` | phase-api, packet, extraction, fixup-model, validation, status, utils, config |
| `xchecker-engine` | Almost all crates |

### Tier 6: Top-Level Crates
Final consumer crates:

| Crate | Key Dependencies |
|-------|------------------|
| `xchecker-cli` | utils, config, engine, error-reporter |
| `xchecker-tui` | engine, utils |
| `xchecker` (root) | All public-facing crates |

## Publish Order

Execute these commands in order. Within each tier, crates can be published in parallel.

### Dry-Run Verification

First, verify each crate can package correctly:

```bash
# Tier 1: Leaf crates (can run in parallel)
cargo publish -p xchecker-extraction --dry-run --allow-dirty
cargo publish -p xchecker-fixup-model --dry-run --allow-dirty
cargo publish -p xchecker-redaction --dry-run --allow-dirty
cargo publish -p xchecker-runner --dry-run --allow-dirty
cargo publish -p xchecker-lock --dry-run --allow-dirty

# Tier 2: Foundation (depends on Tier 1 being on crates.io)
cargo publish -p xchecker-utils --dry-run --allow-dirty

# Tier 3: Low-tier (depends on Tier 2 being on crates.io)
cargo publish -p xchecker-error-redaction --dry-run --allow-dirty
cargo publish -p xchecker-error-reporter --dry-run --allow-dirty
cargo publish -p xchecker-prompt-template --dry-run --allow-dirty
cargo publish -p xchecker-selectors --dry-run --allow-dirty
cargo publish -p xchecker-templates --dry-run --allow-dirty
cargo publish -p xchecker-validation --dry-run --allow-dirty
cargo publish -p xchecker-workspace --dry-run --allow-dirty

# Tier 4: Mid-tier
cargo publish -p xchecker-receipt --dry-run --allow-dirty
cargo publish -p xchecker-config --dry-run --allow-dirty
cargo publish -p xchecker-packet --dry-run --allow-dirty
cargo publish -p xchecker-status --dry-run --allow-dirty
cargo publish -p xchecker-gate --dry-run --allow-dirty
cargo publish -p xchecker-doctor --dry-run --allow-dirty
cargo publish -p xchecker-llm --dry-run --allow-dirty
cargo publish -p xchecker-phase-api --dry-run --allow-dirty
cargo publish -p xchecker-hooks --dry-run --allow-dirty
cargo publish -p xchecker-benchmark --dry-run --allow-dirty

# Tier 5: High-tier
cargo publish -p xchecker-phases --dry-run --allow-dirty
cargo publish -p xchecker-engine --dry-run --allow-dirty

# Tier 6: Top-level
cargo publish -p xchecker-cli --dry-run --allow-dirty
cargo publish -p xchecker-tui --dry-run --allow-dirty
cargo publish -p xchecker --dry-run --allow-dirty
```

**Note:** Dry-run for crates depending on unpublished internal crates will fail until those dependencies are on crates.io. This is expected behavior.

### Actual Publish

Once leaf crates verify clean, publish in order:

```bash
# Tier 1: Leaf crates
cargo publish -p xchecker-extraction
cargo publish -p xchecker-fixup-model
cargo publish -p xchecker-redaction
cargo publish -p xchecker-runner
cargo publish -p xchecker-lock

# Wait ~30 seconds for crates.io indexing between tiers

# Tier 2: Foundation
cargo publish -p xchecker-utils

# Tier 3: Low-tier
cargo publish -p xchecker-error-redaction
cargo publish -p xchecker-error-reporter
cargo publish -p xchecker-prompt-template
cargo publish -p xchecker-selectors
cargo publish -p xchecker-templates
cargo publish -p xchecker-validation
cargo publish -p xchecker-workspace

# Tier 4: Mid-tier
cargo publish -p xchecker-receipt
cargo publish -p xchecker-config
cargo publish -p xchecker-packet
cargo publish -p xchecker-status
cargo publish -p xchecker-gate
cargo publish -p xchecker-doctor
cargo publish -p xchecker-llm
cargo publish -p xchecker-phase-api
cargo publish -p xchecker-hooks
cargo publish -p xchecker-benchmark

# Tier 5: High-tier
cargo publish -p xchecker-phases
cargo publish -p xchecker-engine

# Tier 6: Top-level
cargo publish -p xchecker-cli
cargo publish -p xchecker-tui
cargo publish -p xchecker
```

## Automated Release Script

For automated releases, use this script:

```bash
#!/bin/bash
set -euo pipefail

TIERS=(
    "xchecker-extraction xchecker-fixup-model xchecker-redaction xchecker-runner xchecker-lock"
    "xchecker-utils"
    "xchecker-error-redaction xchecker-error-reporter xchecker-prompt-template xchecker-selectors xchecker-templates xchecker-validation xchecker-workspace"
    "xchecker-receipt xchecker-config xchecker-packet xchecker-status xchecker-gate xchecker-doctor xchecker-llm xchecker-phase-api xchecker-hooks xchecker-benchmark"
    "xchecker-phases xchecker-engine"
    "xchecker-cli xchecker-tui xchecker"
)

for tier in "${TIERS[@]}"; do
    echo "Publishing tier: $tier"
    for crate in $tier; do
        echo "  Publishing $crate..."
        cargo publish -p "$crate"
    done
    echo "Waiting for crates.io indexing..."
    sleep 30
done

echo "Release complete!"
```

## Post-Release Verification

After publishing:

```bash
# Verify crates are available
cargo search xchecker

# Test install from crates.io
cargo install xchecker --version 1.1.0

# Verify binary works
xchecker --version
```

## Repository Note

Manifests point to `https://github.com/EffortlessMetrics/xchecker`. Ensure the release tag and source are available there before publishing. If developing in `xchecker-dev`, either:
- Mirror the release commit to `xchecker`, or
- Update `repository`/`homepage` fields to point to `xchecker-dev`

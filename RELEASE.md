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

# 5. Package every crate the way crates.io will see it
cargo package --workspace --allow-dirty --no-verify

# 6. Review the planned publish order
just publish-plan
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

Use packaging verification for the full workspace:

```bash
cargo package --workspace --allow-dirty --no-verify
```

If you want a real crates.io preflight for a leaf crate, dry-run it directly:

```bash
cargo publish --locked -p xchecker-lock --dry-run --allow-dirty
```

For higher tiers, `cargo publish --dry-run` only succeeds after the lower-tier crates for the same version are already indexed on crates.io. Use the checked-in publish scripts to print or execute the release order instead of hand-maintaining command lists.

### Actual Publish

Once the package check is clean, publish in order:

```bash
./scripts/publish-workspace.sh --execute

# Windows PowerShell
pwsh -File scripts/publish-workspace.ps1 -Execute

# Via just
just publish-execute
```

## Automated Release Script

The publish order now lives in version-controlled scripts instead of inline snippets:

```bash
# Print the tier plan
./scripts/publish-workspace.sh
pwsh -File scripts/publish-workspace.ps1
just publish-plan

# Run crates.io dry-runs in tier order
./scripts/publish-workspace.sh --dry-run
pwsh -File scripts/publish-workspace.ps1 -DryRun
just publish-dry-run

# Resume an interrupted release from tier 4
./scripts/publish-workspace.sh --execute --from-tier 4
pwsh -File scripts/publish-workspace.ps1 -Execute -FromTier 4
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

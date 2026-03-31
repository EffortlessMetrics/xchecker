# CI/CD Setup

This guide explains how to integrate xchecker into your CI/CD pipeline to
enforce spec quality gates, catch regressions, and maintain an audit trail of
spec completeness across your team.

---

## Why use xchecker in CI

- **Enforce quality gates** -- block merges until specs reach a required phase
  (e.g. tasks must be complete before merge)
- **Catch regressions** -- detect stale specs with `--max-phase-age`
- **Audit trail** -- receipts with BLAKE3 hashes prove what was generated, when,
  and by which LLM
- **Cost control** -- dry-run mode and budget limits prevent unexpected LLM
  charges in CI

---

## GitHub Actions setup

Create `.github/workflows/xchecker-gate.yml`:

```yaml
name: xchecker Gate

on:
  pull_request:
    paths:
      - '.xchecker/**'
      - 'src/**'

jobs:
  gate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Install xchecker
        run: cargo install xchecker

      - name: Run gate check
        run: |
          SPEC_ID="my-feature"

          xchecker gate "$SPEC_ID" \
            --min-phase tasks \
            --fail-on-pending-fixups \
            --json > gate-result.json

          cat gate-result.json | jq .

          PASSED=$(cat gate-result.json | jq -r '.passed')
          if [ "$PASSED" != "true" ]; then
            echo "Gate FAILED"
            cat gate-result.json | jq -r '.failure_reasons[]'
            exit 1
          fi
```

### Tiered policies

Use different gate strictness for different environments:

```yaml
jobs:
  gate-dev:
    # Lenient: just require requirements
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo install xchecker
      - run: xchecker gate $SPEC --min-phase requirements

  gate-staging:
    # Moderate: design within 7 days
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo install xchecker
      - run: xchecker gate $SPEC --min-phase design --max-phase-age 7d

  gate-prod:
    # Strict: tasks complete, no fixups, fresh
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo install xchecker
      - run: |
          xchecker gate $SPEC \
            --min-phase tasks \
            --fail-on-pending-fixups \
            --max-phase-age 24h
```

---

## GitLab CI setup

Add to `.gitlab-ci.yml`:

```yaml
stages:
  - build
  - gate

variables:
  CARGO_HOME: $CI_PROJECT_DIR/.cargo
  SPEC_ID: "my-feature"

build:
  stage: build
  image: rust:latest
  cache:
    key: ${CI_JOB_NAME}-${CI_COMMIT_REF_SLUG}
    paths:
      - .cargo/
      - target/
  script:
    - cargo build --release
  artifacts:
    paths:
      - target/release/xchecker
    expire_in: 1 hour

xchecker-gate:
  stage: gate
  image: rust:latest
  needs: ["build"]
  script:
    - |
      ./target/release/xchecker gate $SPEC_ID \
        --min-phase tasks \
        --fail-on-pending-fixups \
        --json > gate-result.json

      cat gate-result.json | jq .

      PASSED=$(cat gate-result.json | jq -r '.passed')
      if [ "$PASSED" != "true" ]; then
        echo "Gate FAILED"
        cat gate-result.json | jq -r '.failure_reasons[]'
        exit 1
      fi
  artifacts:
    paths:
      - gate-result.json
    expire_in: 1 week
  rules:
    - if: $CI_PIPELINE_SOURCE == "merge_request_event"
    - if: $CI_COMMIT_BRANCH == $CI_DEFAULT_BRANCH
```

For multi-spec workspaces, use a parallel matrix:

```yaml
xchecker-gate:
  stage: gate
  parallel:
    matrix:
      - SPEC_ID: feature-auth
        MIN_PHASE: tasks
      - SPEC_ID: feature-ui
        MIN_PHASE: design
  script:
    - ./target/release/xchecker gate $SPEC_ID --min-phase $MIN_PHASE
```

---

## The gate command

```bash
xchecker gate <spec-id> [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--min-phase <phase>` | Require at least this phase completed (`requirements`, `design`, `tasks`, `review`, `fixup`, `final`) |
| `--fail-on-pending-fixups` | Fail if any pending fixups exist |
| `--max-phase-age <duration>` | Fail if latest success is older than threshold (`7d`, `24h`, `30m`) |
| `--json` | Output structured JSON (schema: `gate-json.v1`) |
| `--policy <path>` | Load policy from a TOML file |

### Exit codes

| Code | Meaning |
|------|---------|
| 0 | Gate passed -- all conditions met |
| 1 | Gate failed -- one or more policy violations |
| 2+ | Runtime error (config, I/O, etc.) |

---

## Cost control in CI

### Dry-run mode

Run xchecker without making LLM calls. Useful for validating config and packet
construction:

```bash
xchecker spec my-feature --dry-run
```

### Skip LLM tests

When running the xchecker test suite in CI, skip tests that call real LLMs:

```bash
export XCHECKER_SKIP_LLM_TESTS=1
cargo test --workspace
```

### Budget limits for HTTP providers

Cap the number of LLM calls per process when using OpenRouter:

```bash
export XCHECKER_OPENROUTER_BUDGET=10
```

Or in config:

```toml
[llm.openrouter]
budget = 10
```

Budget is enforced per process and resets on each run.

---

## JSON output for automation

All xchecker commands support `--json` for machine-readable output. Parse with
`jq` in CI scripts:

```bash
# Check if gate passed
xchecker gate my-spec --min-phase tasks --json | jq -r '.passed'

# List failure reasons
xchecker gate my-spec --min-phase tasks --json | jq -r '.failure_reasons[]'

# Get completed phases
xchecker status my-spec --json | jq -r '.phase_statuses[] | select(.status == "success") | .phase_id'

# Check overall health
xchecker doctor --json | jq -r '.ok'
```

All JSON output follows versioned schemas with stability guarantees. See
[JSON Schemas](../reference/SCHEMAS.md) and [Contracts](../reference/CONTRACTS.md)
for details.

---

## See also

- [Configuration Guide](CONFIGURATION.md) -- config file setup
- [Quickstart](../tutorials/QUICKSTART.md) -- install and first spec
- [Spec to PR](../tutorials/SPEC_TO_PR.md) -- full workflow tutorial
- [JSON Schemas](../reference/SCHEMAS.md) -- schema index

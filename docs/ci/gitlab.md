# GitLab CI Configuration for xchecker Gate

This document describes how to configure GitLab CI to use xchecker gate as a merge request check.

**Requirements:** FR-GATE-CI (4.6.1, 4.6.2, 4.6.3)

## Quick Start

Add the following to your `.gitlab-ci.yml`:

```yaml
# xchecker Gate - Basic Configuration
xchecker-gate:
  stage: test
  image: rust:latest
  script:
    - cargo build --release
    - ./target/release/xchecker gate my-spec --min-phase tasks
  rules:
    - if: $CI_PIPELINE_SOURCE == "merge_request_event"
    - if: $CI_COMMIT_BRANCH == $CI_DEFAULT_BRANCH
```

## Complete Configuration

### Single Spec Gate

```yaml
stages:
  - build
  - test
  - gate

variables:
  CARGO_HOME: $CI_PROJECT_DIR/.cargo
  SPEC_ID: "my-spec"
  MIN_PHASE: "tasks"

# Cache Cargo dependencies
.cargo-cache: &cargo-cache
  cache:
    key: ${CI_JOB_NAME}-${CI_COMMIT_REF_SLUG}
    paths:
      - .cargo/
      - target/

build:
  stage: build
  image: rust:latest
  <<: *cargo-cache
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
    # Run gate with JSON output
    - |
      ./target/release/xchecker gate $SPEC_ID \
        --min-phase $MIN_PHASE \
        --json > gate-result.json || true
    
    # Display result
    - cat gate-result.json | jq .
    
    # Check if passed
    - |
      PASSED=$(cat gate-result.json | jq -r '.passed')
      if [ "$PASSED" != "true" ]; then
        echo "❌ Gate FAILED"
        cat gate-result.json | jq -r '.failure_reasons[]'
        exit 1
      fi
      echo "✅ Gate PASSED"
  artifacts:
    paths:
      - gate-result.json
    reports:
      dotenv: gate-result.env
    expire_in: 1 week
  rules:
    - if: $CI_PIPELINE_SOURCE == "merge_request_event"
    - if: $CI_COMMIT_BRANCH == $CI_DEFAULT_BRANCH
```

### Multi-Spec Gate (Workspace)

For projects with multiple specs, use a parallel matrix:

```yaml
xchecker-gate:
  stage: gate
  image: rust:latest
  needs: ["build"]
  parallel:
    matrix:
      - SPEC_ID: feature-auth
        MIN_PHASE: tasks
        FAIL_ON_FIXUPS: "false"
      - SPEC_ID: feature-ui
        MIN_PHASE: design
        FAIL_ON_FIXUPS: "false"
      - SPEC_ID: api-v2
        MIN_PHASE: tasks
        FAIL_ON_FIXUPS: "true"
  script:
    - |
      GATE_CMD="./target/release/xchecker gate $SPEC_ID --min-phase $MIN_PHASE"
      
      if [ "$FAIL_ON_FIXUPS" = "true" ]; then
        GATE_CMD="$GATE_CMD --fail-on-pending-fixups"
      fi
      
      echo "Running: $GATE_CMD"
      $GATE_CMD
  rules:
    - if: $CI_PIPELINE_SOURCE == "merge_request_event"
```

### Gate with Age Check

To ensure specs are kept up-to-date:

```yaml
xchecker-gate-fresh:
  stage: gate
  image: rust:latest
  needs: ["build"]
  script:
    - |
      ./target/release/xchecker gate $SPEC_ID \
        --min-phase tasks \
        --max-phase-age 7d \
        --fail-on-pending-fixups
  rules:
    - if: $CI_PIPELINE_SOURCE == "merge_request_event"
```

## Policy Parameters

| Parameter | Description | Example |
|-----------|-------------|---------|
| `--min-phase <phase>` | Require at least this phase completed | `--min-phase tasks` |
| `--fail-on-pending-fixups` | Fail if any pending fixups exist | `--fail-on-pending-fixups` |
| `--max-phase-age <duration>` | Fail if latest success is older than threshold | `--max-phase-age 7d` |
| `--json` | Output structured JSON for CI parsing | `--json` |

### Phase Values

- `requirements` - Requirements phase
- `design` - Design phase
- `tasks` - Tasks phase (default minimum)
- `review` - Review phase
- `fixup` - Fixup phase
- `final` - Final phase

### Duration Format

- `7d` - 7 days
- `24h` - 24 hours
- `30m` - 30 minutes
- `60s` - 60 seconds

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Policy passed - all conditions met |
| 1 | Policy violation - one or more conditions not met |
| Other | Runtime error (config error, IO error, etc.) |

## Setting Up Required Status Checks

### GitLab Merge Request Approvals

1. Go to **Settings > Merge requests**
2. Under "Merge checks", enable **Pipelines must succeed**
3. Optionally enable **All threads must be resolved**

### Protected Branches

1. Go to **Settings > Repository > Protected branches**
2. Select your main branch
3. Set "Allowed to merge" to appropriate role
4. Enable **Require approval from code owners** if desired

### Merge Request Approval Rules

1. Go to **Settings > Merge requests > Approval rules**
2. Create a rule requiring approval from specific users/groups
3. The xchecker-gate job will block merges if it fails

## JSON Output Schema

The `--json` flag outputs structured data following the `gate-json.v1` schema:

```json
{
  "schema_version": "gate-json.v1",
  "spec_id": "feature-auth",
  "passed": true,
  "conditions": [
    {
      "name": "min_phase",
      "passed": true,
      "description": "Required phase 'tasks' is completed",
      "actual": "tasks completed",
      "expected": "tasks or later"
    }
  ],
  "failure_reasons": [],
  "summary": "Gate PASSED: Spec 'feature-auth' meets all policy requirements"
}
```

See `docs/schemas/gate-json.v1.json` for the full schema definition.

## Troubleshooting

### Gate fails with "Spec does not exist"

Ensure the spec directory exists at `.xchecker/specs/<spec-id>/`.

```bash
# Check spec exists
ls -la .xchecker/specs/my-spec/
```

### Gate fails with "No successful receipt found"

The spec needs at least one successful phase run. Run xchecker locally first:

```bash
xchecker run my-spec --phase requirements
```

### Gate fails with stale phase

If using `--max-phase-age`, ensure the spec has been run recently:

```bash
# Check latest receipt timestamp
ls -la .xchecker/specs/my-spec/receipts/
```

### Pipeline is slow

Cache the Cargo build:

```yaml
cache:
  key: ${CI_JOB_NAME}
  paths:
    - .cargo/
    - target/
```

Or use a pre-built Docker image with xchecker installed.

## See Also

- [GitHub Actions Configuration](../.github/workflows/xchecker-gate.yml)
- [Gate JSON Schema](../schemas/gate-json.v1.json)
- [CI Profiles](../CI_PROFILES.md)
- [Testing Documentation](../TESTING.md)

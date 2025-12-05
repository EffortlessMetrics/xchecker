# From Spec to PR: xchecker + Claude Code Flow

This walkthrough demonstrates the complete workflow from creating a spec to submitting a pull request, using xchecker integrated with Claude Code (Claude's IDE environment).

## Overview

This guide covers:

1. Creating a spec with xchecker
2. Using Claude Code to query spec status
3. Implementing features based on generated tasks
4. Validating with gates before PR submission
5. CI integration for automated checks

## Prerequisites

- xchecker installed and configured (see [WALKTHROUGH_20_MINUTES.md](WALKTHROUGH_20_MINUTES.md))
- Claude Code environment set up
- A Git repository with xchecker initialized

## The Complete Flow

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  Create Spec    │────▶│  Claude Code    │────▶│  Implement      │
│  (xchecker)     │     │  Integration    │     │  Features       │
└─────────────────┘     └─────────────────┘     └─────────────────┘
                                                        │
                                                        ▼
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  Submit PR      │◀────│  CI Gate        │◀────│  Validate       │
│                 │     │  Check          │     │  (xchecker)     │
└─────────────────┘     └─────────────────┘     └─────────────────┘
```

## Step 1: Create the Spec

Start by creating a spec for your feature:

```bash
# Initialize the spec
xchecker init user-auth

# Create problem statement
cat > .xchecker/specs/user-auth/context/problem-statement.md << 'EOF'
# User Authentication Feature

## Goal
Implement secure user authentication with session management.

## Requirements
- Email/password login
- OAuth2 support (Google, GitHub)
- JWT-based sessions
- Password reset via email
- Account lockout after failed attempts

## Technical Constraints
- Use existing PostgreSQL database
- Must support both REST and GraphQL APIs
- Session tokens expire after 24 hours
EOF

# Run through all phases
xchecker spec user-auth --source fs
xchecker resume user-auth --phase design
xchecker resume user-auth --phase tasks
```

## Step 2: Query Spec from Claude Code

Claude Code can query xchecker to understand the current spec state.

### Get Spec Overview

In Claude Code, invoke:

```bash
xchecker spec user-auth --json
```

Response:

```json
{
  "schema_version": "spec-json.v1",
  "spec_id": "user-auth",
  "phases": [
    {"phase_id": "requirements", "status": "completed", "last_run": "2024-12-01T10:00:00Z"},
    {"phase_id": "design", "status": "completed", "last_run": "2024-12-01T10:30:00Z"},
    {"phase_id": "tasks", "status": "completed", "last_run": "2024-12-01T11:00:00Z"}
  ],
  "config_summary": {
    "execution_strategy": "controlled",
    "provider": "claude-cli",
    "spec_path": ".xchecker/specs/user-auth"
  }
}
```

### Get Current Status

```bash
xchecker status user-auth --json
```

Response:

```json
{
  "schema_version": "status-json.v1",
  "spec_id": "user-auth",
  "phase_statuses": [
    {"phase_id": "requirements", "status": "success", "receipt_id": "requirements-20241201_100000"},
    {"phase_id": "design", "status": "success", "receipt_id": "design-20241201_103000"},
    {"phase_id": "tasks", "status": "success", "receipt_id": "tasks-20241201_110000"}
  ],
  "pending_fixups": 0,
  "has_errors": false
}
```

### Get Resume Context

When Claude Code needs to understand what to do next:

```bash
xchecker resume user-auth --phase review --json
```

Response:

```json
{
  "schema_version": "resume-json.v1",
  "spec_id": "user-auth",
  "phase": "review",
  "current_inputs": {
    "available_artifacts": [
      "00-requirements.md",
      "00-requirements.core.yaml",
      "01-design.md",
      "02-tasks.md"
    ],
    "spec_exists": true,
    "latest_completed_phase": "tasks"
  },
  "next_steps": "Run review phase to validate implementation against requirements"
}
```

## Step 3: Implement Features

Use the generated tasks as your implementation guide.

### View Tasks

```bash
cat .xchecker/specs/user-auth/artifacts/02-tasks.md
```

Example output:

```markdown
# Implementation Tasks

## Phase 1: Core Authentication
- [ ] Create User model with password hashing
- [ ] Implement login endpoint
- [ ] Implement logout endpoint
- [ ] Add JWT token generation

## Phase 2: OAuth Integration
- [ ] Set up OAuth2 configuration
- [ ] Implement Google OAuth flow
- [ ] Implement GitHub OAuth flow
- [ ] Handle OAuth callback and token exchange

## Phase 3: Session Management
- [ ] Implement session storage
- [ ] Add token refresh mechanism
- [ ] Implement session invalidation
- [ ] Add account lockout logic
```

### Implement with Claude Code Assistance

Claude Code can reference the spec while helping you implement:

```javascript
// Claude Code can query the spec to understand context
const specStatus = await invokeXChecker("status", "user-auth");

// Then provide implementation guidance based on the design
const design = await readFile(".xchecker/specs/user-auth/artifacts/01-design.md");
```

## Step 4: Validate Before PR

Before creating a PR, validate your spec meets quality gates.

### Run Gate Check

```bash
# Basic gate check
xchecker gate user-auth --min-phase tasks

# Strict gate check with age constraint
xchecker gate user-auth \
  --min-phase tasks \
  --max-phase-age 7d \
  --fail-on-pending-fixups
```

### Gate Output

Success:

```
✓ Gate PASSED for spec 'user-auth'
  ✓ Minimum phase 'tasks' reached
  ✓ Phase age: 2 days (within 7d limit)
  ✓ No pending fixups
```

Failure:

```
✗ Gate FAILED for spec 'user-auth'
  ✗ Phase age: 10 days (exceeds 7d limit)
  
Suggestion: Run 'xchecker resume user-auth --phase tasks' to refresh
```

### JSON Gate Output

For automation:

```bash
xchecker gate user-auth --min-phase tasks --json
```

```json
{
  "schema_version": "gate-json.v1",
  "spec_id": "user-auth",
  "decision": "pass",
  "evaluated_conditions": [
    {"condition": "min_phase", "expected": "tasks", "actual": "tasks", "passed": true},
    {"condition": "max_phase_age", "expected": "7d", "actual": "2d", "passed": true},
    {"condition": "pending_fixups", "expected": 0, "actual": 0, "passed": true}
  ]
}
```

## Step 5: CI Integration

Add xchecker gate checks to your CI pipeline.

### GitHub Actions

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
      
      - name: Install Rust
        uses: dtolnay/rust-action@stable
      
      - name: Install xchecker
        run: cargo install xchecker
      
      - name: Run Gate Check
        run: |
          xchecker gate user-auth \
            --min-phase tasks \
            --max-phase-age 14d
```

### GitLab CI

Add to `.gitlab-ci.yml`:

```yaml
xchecker-gate:
  stage: validate
  script:
    - cargo install xchecker
    - xchecker gate user-auth --min-phase tasks --max-phase-age 14d
  rules:
    - changes:
        - .xchecker/**/*
        - src/**/*
```

## Step 6: Submit PR

With gates passing, create your PR:

```bash
# Create feature branch
git checkout -b feature/user-auth

# Add changes
git add .

# Commit with spec reference
git commit -m "feat: implement user authentication

Spec: user-auth
Phases completed: requirements, design, tasks
Gate status: passing"

# Push and create PR
git push -u origin feature/user-auth
```

## Claude Code Tool Integration

### Define xchecker as a Tool

```json
{
  "name": "xchecker",
  "description": "Query and manage xchecker specs",
  "input_schema": {
    "type": "object",
    "properties": {
      "command": {
        "type": "string",
        "enum": ["spec", "status", "resume", "gate"],
        "description": "The xchecker command"
      },
      "spec_id": {
        "type": "string",
        "description": "The spec identifier"
      },
      "phase": {
        "type": "string",
        "description": "Phase for resume command"
      },
      "min_phase": {
        "type": "string",
        "description": "Minimum phase for gate command"
      }
    },
    "required": ["command", "spec_id"]
  }
}
```

### Example Tool Invocations

**Check spec status:**

```json
{
  "name": "xchecker",
  "input": {
    "command": "status",
    "spec_id": "user-auth"
  }
}
```

**Get resume context:**

```json
{
  "name": "xchecker",
  "input": {
    "command": "resume",
    "spec_id": "user-auth",
    "phase": "review"
  }
}
```

**Run gate check:**

```json
{
  "name": "xchecker",
  "input": {
    "command": "gate",
    "spec_id": "user-auth",
    "min_phase": "tasks"
  }
}
```

## Complete Example Script

Here's a script that demonstrates the full flow:

```bash
#!/bin/bash
# full-spec-flow.sh - Complete spec to PR workflow

set -e

SPEC_ID="user-auth"
BRANCH="feature/${SPEC_ID}"

echo "=== Step 1: Initialize Spec ==="
xchecker init $SPEC_ID

echo "=== Step 2: Create Problem Statement ==="
cat > .xchecker/specs/$SPEC_ID/context/problem-statement.md << 'EOF'
# User Authentication
Implement secure authentication with OAuth2 support.
EOF

echo "=== Step 3: Run Phases ==="
xchecker spec $SPEC_ID --source fs
xchecker resume $SPEC_ID --phase design
xchecker resume $SPEC_ID --phase tasks

echo "=== Step 4: Check Status ==="
xchecker status $SPEC_ID --json | jq .

echo "=== Step 5: Validate Gate ==="
xchecker gate $SPEC_ID --min-phase tasks

echo "=== Step 6: Create Branch ==="
git checkout -b $BRANCH

echo "=== Step 7: Commit Spec ==="
git add .xchecker/specs/$SPEC_ID/
git commit -m "spec: add $SPEC_ID specification"

echo "=== Done! ==="
echo "Spec '$SPEC_ID' is ready for implementation."
echo "View tasks: cat .xchecker/specs/$SPEC_ID/artifacts/02-tasks.md"
```

## Best Practices

### 1. Keep Specs Fresh

Run phases periodically to keep specs up to date:

```bash
# Refresh tasks phase weekly
xchecker resume user-auth --phase tasks
```

### 2. Use Gates in CI

Always validate specs before merging:

```bash
xchecker gate $SPEC_ID --min-phase tasks --max-phase-age 14d
```

### 3. Reference Specs in Commits

Include spec references in commit messages:

```
feat: implement OAuth2 login

Spec: user-auth
Task: Phase 2 - OAuth Integration
```

### 4. Review Artifacts Before Implementation

Always review generated artifacts before coding:

```bash
# Review design decisions
cat .xchecker/specs/user-auth/artifacts/01-design.md

# Review task breakdown
cat .xchecker/specs/user-auth/artifacts/02-tasks.md
```

### 5. Use JSON Output for Automation

Prefer JSON output for scripting and automation:

```bash
# Parse with jq
xchecker status user-auth --json | jq '.phase_statuses[] | select(.status == "success")'
```

## Troubleshooting

### Gate Fails with "Phase not reached"

Run the missing phase:

```bash
xchecker resume user-auth --phase tasks
```

### Gate Fails with "Phase too old"

Refresh the phase:

```bash
xchecker resume user-auth --phase tasks
```

### Claude Code Can't Parse JSON

Ensure you're using the `--json` flag:

```bash
xchecker status user-auth --json  # Correct
xchecker status user-auth         # Human-readable, not for parsing
```

## See Also

- [CLAUDE_CODE_INTEGRATION.md](CLAUDE_CODE_INTEGRATION.md) - Detailed Claude Code integration
- [WALKTHROUGH_20_MINUTES.md](WALKTHROUGH_20_MINUTES.md) - Quick start guide
- [CONFIGURATION.md](CONFIGURATION.md) - Configuration reference
- [ci/gitlab.md](ci/gitlab.md) - GitLab CI templates

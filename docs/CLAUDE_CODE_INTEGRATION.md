# Claude Code Integration Guide

This guide documents how to integrate xchecker with Claude Code (Claude's IDE environment) for automated spec-driven development workflows.

## Overview

xchecker provides machine-friendly JSON output for three key commands that enable Claude Code integration:

| Command | Purpose | Schema |
|---------|---------|--------|
| `xchecker spec --json` | Get spec metadata and phase status | `spec-json.v1` |
| `xchecker status --json` | Get compact status summary | `status-json.v1` |
| `xchecker resume --json` | Get resume context for a phase | `resume-json.v1` |

All JSON outputs include a `schema_version` field for reliable parsing and forward compatibility.

## Canonical Slash Command Scheme

Claude Code can invoke xchecker using these canonical slash commands:

```
/xchecker spec <spec-id>           # Get spec overview
/xchecker status <spec-id>         # Get current status
/xchecker resume <spec-id> <phase> # Get resume context for a phase
```

### Mapping to CLI Commands

| Slash Command | CLI Invocation |
|---------------|----------------|
| `/xchecker spec my-feature` | `xchecker spec my-feature --json` |
| `/xchecker status my-feature` | `xchecker status my-feature --json` |
| `/xchecker resume my-feature design` | `xchecker resume my-feature --phase design --json` |

## JSON Output Examples

### Example 1: `xchecker spec --json`

Get high-level spec metadata without exposing full artifacts or packet contents.

**Command:**
```bash
xchecker spec my-feature --json
```

**Output:**
```json
{
  "schema_version": "spec-json.v1",
  "spec_id": "my-feature",
  "phases": [
    {
      "phase_id": "requirements",
      "status": "completed",
      "last_run": "2024-12-01T10:00:00Z"
    },
    {
      "phase_id": "design",
      "status": "completed",
      "last_run": "2024-12-01T11:30:00Z"
    },
    {
      "phase_id": "tasks",
      "status": "pending"
    }
  ],
  "config_summary": {
    "execution_strategy": "controlled",
    "provider": "claude-cli",
    "spec_path": ".xchecker/specs/my-feature"
  }
}
```

**Key Fields:**
- `schema_version`: Always `"spec-json.v1"` for this format
- `phases`: Array of phase metadata (not full artifacts)
- `config_summary`: High-level configuration (no secrets or full paths)

### Example 2: `xchecker status --json`

Get compact status summary with receipt IDs and error flags.

**Command:**
```bash
xchecker status my-feature --json
```

**Output:**
```json
{
  "schema_version": "status-json.v1",
  "spec_id": "my-feature",
  "phase_statuses": [
    {
      "phase_id": "requirements",
      "status": "success",
      "receipt_id": "requirements-20241201_100000"
    },
    {
      "phase_id": "design",
      "status": "success",
      "receipt_id": "design-20241201_113000"
    },
    {
      "phase_id": "tasks",
      "status": "not_started"
    }
  ],
  "pending_fixups": 0,
  "has_errors": false
}
```

**Key Fields:**
- `phase_statuses`: Per-phase status with receipt IDs for traceability
- `pending_fixups`: Count of pending fixup targets (0 if none)
- `has_errors`: Quick check for any failed phases

### Example 3: `xchecker resume --json`

Get resume context for a specific phase without full packet or raw artifacts.

**Command:**
```bash
xchecker resume my-feature --phase design --json
```

**Output:**
```json
{
  "schema_version": "resume-json.v1",
  "spec_id": "my-feature",
  "phase": "design",
  "current_inputs": {
    "available_artifacts": [
      "00-requirements.md",
      "00-requirements.core.yaml"
    ],
    "spec_exists": true,
    "latest_completed_phase": "requirements"
  },
  "next_steps": "Run design phase to generate architecture based on requirements"
}
```

**Key Fields:**
- `current_inputs`: Available artifacts (names only, not contents)
- `next_steps`: Human-readable hint for the next action

## Claude Code Tool Invocation Model

Claude Code can invoke xchecker as an external tool. Here's how to map JSON outputs into Claude Code's tool invocation model.

### Tool Definition

Define xchecker as a tool in Claude Code:

```json
{
  "name": "xchecker",
  "description": "Orchestrate spec-driven development workflows",
  "input_schema": {
    "type": "object",
    "properties": {
      "command": {
        "type": "string",
        "enum": ["spec", "status", "resume"],
        "description": "The xchecker command to run"
      },
      "spec_id": {
        "type": "string",
        "description": "The spec identifier"
      },
      "phase": {
        "type": "string",
        "enum": ["requirements", "design", "tasks", "review", "fixup", "final"],
        "description": "Phase for resume command (required for resume)"
      }
    },
    "required": ["command", "spec_id"]
  }
}
```

### Tool Invocation Examples

**Get spec overview:**
```json
{
  "name": "xchecker",
  "input": {
    "command": "spec",
    "spec_id": "my-feature"
  }
}
```

Maps to: `xchecker spec my-feature --json`

**Get current status:**
```json
{
  "name": "xchecker",
  "input": {
    "command": "status",
    "spec_id": "my-feature"
  }
}
```

Maps to: `xchecker status my-feature --json`

**Get resume context:**
```json
{
  "name": "xchecker",
  "input": {
    "command": "resume",
    "spec_id": "my-feature",
    "phase": "design"
  }
}
```

Maps to: `xchecker resume my-feature --phase design --json`

### Parsing Tool Results

Claude Code should parse the JSON response and use the `schema_version` field to determine the response format:

```javascript
function parseXCheckerResponse(jsonOutput) {
  const response = JSON.parse(jsonOutput);
  
  switch (response.schema_version) {
    case "spec-json.v1":
      return handleSpecResponse(response);
    case "status-json.v1":
      return handleStatusResponse(response);
    case "resume-json.v1":
      return handleResumeResponse(response);
    default:
      throw new Error(`Unknown schema version: ${response.schema_version}`);
  }
}

function handleSpecResponse(spec) {
  // Extract phase information
  const completedPhases = spec.phases
    .filter(p => p.status === "completed")
    .map(p => p.phase_id);
  
  const pendingPhases = spec.phases
    .filter(p => p.status === "pending")
    .map(p => p.phase_id);
  
  return {
    specId: spec.spec_id,
    completedPhases,
    pendingPhases,
    executionStrategy: spec.config_summary.execution_strategy,
    provider: spec.config_summary.provider
  };
}

function handleStatusResponse(status) {
  // Check for errors or pending work
  return {
    specId: status.spec_id,
    hasErrors: status.has_errors,
    pendingFixups: status.pending_fixups,
    latestPhase: status.phase_statuses
      .filter(p => p.status === "success")
      .pop()?.phase_id
  };
}

function handleResumeResponse(resume) {
  // Get context for resuming work
  return {
    specId: resume.spec_id,
    phase: resume.phase,
    availableArtifacts: resume.current_inputs.available_artifacts,
    nextSteps: resume.next_steps
  };
}
```

## Complete Integration Flow

Here's a complete example showing how Claude Code can use xchecker to drive a spec workflow:

### Step 1: Check Spec Status

```bash
# Claude Code invokes:
xchecker status my-feature --json
```

**Response:**
```json
{
  "schema_version": "status-json.v1",
  "spec_id": "my-feature",
  "phase_statuses": [
    {"phase_id": "requirements", "status": "success", "receipt_id": "requirements-20241201_100000"}
  ],
  "pending_fixups": 0,
  "has_errors": false
}
```

**Claude Code interprets:** Requirements phase is complete, no errors.

### Step 2: Get Resume Context for Design Phase

```bash
# Claude Code invokes:
xchecker resume my-feature --phase design --json
```

**Response:**
```json
{
  "schema_version": "resume-json.v1",
  "spec_id": "my-feature",
  "phase": "design",
  "current_inputs": {
    "available_artifacts": ["00-requirements.md", "00-requirements.core.yaml"],
    "spec_exists": true,
    "latest_completed_phase": "requirements"
  },
  "next_steps": "Run design phase to generate architecture based on requirements"
}
```

**Claude Code interprets:** Ready to run design phase, requirements artifacts available.

### Step 3: Execute Design Phase

```bash
# Claude Code invokes (not JSON mode - actual execution):
xchecker resume my-feature --phase design
```

### Step 4: Verify Completion

```bash
# Claude Code invokes:
xchecker spec my-feature --json
```

**Response:**
```json
{
  "schema_version": "spec-json.v1",
  "spec_id": "my-feature",
  "phases": [
    {"phase_id": "requirements", "status": "completed", "last_run": "2024-12-01T10:00:00Z"},
    {"phase_id": "design", "status": "completed", "last_run": "2024-12-01T14:00:00Z"},
    {"phase_id": "tasks", "status": "pending"}
  ],
  "config_summary": {
    "execution_strategy": "controlled",
    "provider": "claude-cli",
    "spec_path": ".xchecker/specs/my-feature"
  }
}
```

**Claude Code interprets:** Design phase complete, tasks phase is next.

## Error Handling

When xchecker encounters errors, it returns non-zero exit codes with structured error information.

### Exit Codes

| Code | Meaning | Action |
|------|---------|--------|
| 0 | Success | Parse JSON response |
| 1 | Unknown error | Check stderr for details |
| 2 | CLI/config error | Fix configuration |
| 7 | Packet overflow | Reduce input size |
| 8 | Secret detected | Remove secrets from input |
| 9 | Lock held | Wait or use `--force` |
| 10 | Phase timeout | Increase timeout or simplify |
| 70 | LLM Provider failure | Check provider CLI/API status |

### Error Response Example

When a command fails, check the exit code and stderr:

```bash
xchecker status nonexistent-spec --json
# Exit code: 2
# Stderr: Error: Spec 'nonexistent-spec' not found
```

Claude Code should handle errors gracefully:

```javascript
async function invokeXChecker(command, specId, phase) {
  const args = buildArgs(command, specId, phase);
  const result = await exec(`xchecker ${args.join(' ')} --json`);
  
  if (result.exitCode !== 0) {
    return {
      success: false,
      exitCode: result.exitCode,
      error: result.stderr
    };
  }
  
  return {
    success: true,
    data: JSON.parse(result.stdout)
  };
}
```

## Best Practices

### 1. Always Use `--json` for Automation

The `--json` flag ensures stable, machine-parseable output:

```bash
# Good: Stable JSON output
xchecker status my-feature --json

# Avoid: Human-readable output may change
xchecker status my-feature
```

### 2. Check Schema Version

Always verify the `schema_version` field before parsing:

```javascript
if (response.schema_version !== "status-json.v1") {
  throw new Error(`Unexpected schema version: ${response.schema_version}`);
}
```

### 3. Use Receipts as Source of Truth

The JSON outputs derive status from receipts, not by introspecting the repository directly. This ensures consistency and auditability.

### 4. Handle Missing Specs Gracefully

Check for spec existence before attempting operations:

```bash
xchecker spec my-feature --json
# If exit code is 2 and error mentions "not found", the spec doesn't exist
```

### 5. Respect Size Limits

JSON outputs intentionally exclude full artifacts and packet contents to keep responses small. If you need artifact contents, read them directly from the spec directory.

## JSON Schema References

Full JSON schemas are available in the `docs/schemas/` directory:

- [`spec-json.v1.json`](schemas/spec-json.v1.json) - Spec output schema
- [`status-json.v1.json`](schemas/status-json.v1.json) - Status output schema
- [`resume-json.v1.json`](schemas/resume-json.v1.json) - Resume output schema

## Related Documentation

- [CONFIGURATION.md](CONFIGURATION.md) - Configuration options
- [LLM_PROVIDERS.md](LLM_PROVIDERS.md) - LLM provider setup
- [CONTRACTS.md](CONTRACTS.md) - JSON schema versioning policy
- [TROUBLESHOOTING.md](TROUBLESHOOTING.md) - Common issues and solutions

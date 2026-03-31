# Workspace Guide

xchecker supports workspace-level management for projects with multiple specs. This guide covers how to use the workspace (project) features.

## Overview

A workspace allows you to:
- Manage multiple specs in a single project
- Tag specs for organization
- View aggregated status across all specs
- Track history and progression
- Use the interactive TUI for monitoring

## Workspace Structure

```
my-project/
├── workspace.yaml           # Workspace configuration
└── .xchecker/
    └── specs/
        ├── auth-feature/    # Individual spec
        │   ├── artifacts/
        │   ├── receipts/
        │   └── lock.json
        ├── api-redesign/
        └── docs-update/
```

## Creating a Workspace

Initialize a new workspace:

```bash
xchecker project init my-workspace
```

This creates a `workspace.yaml` file:

```yaml
name: my-workspace
specs: []
```

## Adding Specs to a Workspace

Add an existing spec:

```bash
xchecker project add-spec auth-feature
```

Add with tags for organization:

```bash
xchecker project add-spec auth-feature --tag backend --tag critical
```

The workspace.yaml updates:

```yaml
name: my-workspace
specs:
  - id: auth-feature
    tags:
      - backend
      - critical
```

## Listing Workspace Specs

View all specs in the workspace:

```bash
xchecker project list
```

Output:
```
Workspace: my-workspace (3 specs)

ID              Tags            Status          Last Activity
auth-feature    backend,critical  tasks          2025-12-06 10:30
api-redesign    backend          design         2025-12-06 09:15
docs-update     docs             requirements   2025-12-05 14:00
```

## Workspace Status

Get aggregated status:

```bash
xchecker project status
```

Output:
```
Workspace Status: my-workspace

Total: 3 specs
  Completed:   0
  In Progress: 2
  Pending:     1
  Stale:       0
  Failed:      0

Recent Activity:
  auth-feature: tasks completed 2 hours ago
  api-redesign: design completed 3 hours ago
```

JSON output for CI:

```bash
xchecker project status --json
```

## Spec History

View the progression of a spec:

```bash
xchecker project history auth-feature
```

Output:
```
Spec: auth-feature

Phase Timeline:
  requirements  2025-12-05 10:00  ✓ completed (exit 0)
  design        2025-12-05 11:30  ✓ completed (exit 0)
  tasks         2025-12-06 10:30  ✓ completed (exit 0)
  review        (not started)
  fixup         (not started)
  final         (not started)

Total Duration: 1d 0h 30m
Model: claude-sonnet-4-20250514
```

JSON output:

```bash
xchecker project history auth-feature --json
```

## Interactive TUI

Launch the terminal UI for real-time monitoring:

```bash
xchecker project tui
```

### TUI Features

- **Navigation**: Arrow keys or j/k to move, Enter to select, Esc to go back
- **Views**:
  - Spec list with tags, status, and last activity
  - Spec detail with receipt summary
  - Phase progression timeline
- **Status Indicators**:
  - Green: Completed successfully
  - Yellow: Pending or stale
  - Red: Failed
- **Keyboard Shortcuts**:
  - `q`: Quit
  - `r`: Refresh
  - `?`: Help

### TUI Limitations (V16 Read-Only)

The TUI is currently read-only:
- Cannot modify specs
- Cannot trigger phase execution
- Cannot delete or clean specs
- State is captured at startup (no real-time sync)

## Organizing with Tags

Tags help organize specs by:
- Component: `backend`, `frontend`, `api`
- Priority: `critical`, `p1`, `p2`
- Type: `feature`, `bugfix`, `refactor`
- Team: `team-a`, `team-b`

Add multiple tags:

```bash
xchecker project add-spec my-spec --tag backend --tag p1 --tag team-a
```

## Workspace Best Practices

### 1. Consistent Tagging

Establish a tagging convention:
```yaml
# workspace.yaml comment
# Tags: component:{backend,frontend,api}, priority:{p0,p1,p2}, type:{feature,bugfix}
```

### 2. Regular Cleanup

Remove completed specs:
```bash
xchecker clean completed-spec --hard
```

### 3. Stale Spec Detection

Specs not updated in 7 days are marked "stale". Review and either:
- Resume the spec
- Clean it up
- Mark as intentionally paused

### 4. CI Integration

Use workspace status in CI:
```yaml
- name: Check Workspace Health
  run: |
    STATUS=$(xchecker project status --json)
    FAILED=$(echo $STATUS | jq '.failed')
    if [ "$FAILED" -gt 0 ]; then
      echo "Failed specs detected"
      exit 1
    fi
```

## Workspace vs Individual Specs

| Feature | Individual Spec | Workspace |
|---------|-----------------|-----------|
| Single spec workflow | `xchecker spec` | `xchecker spec` |
| Multiple specs | Manual management | `xchecker project` |
| Aggregated status | N/A | `xchecker project status` |
| History tracking | Per-receipt | `xchecker project history` |
| Visual monitoring | N/A | `xchecker project tui` |
| Tagging | N/A | `--tag` support |

## Configuration

Workspace-level configuration in `workspace.yaml`:

```yaml
name: my-workspace
default_tags:
  - team-alpha
specs:
  - id: feature-a
    tags:
      - backend
  - id: feature-b
    tags:
      - frontend
```

## Troubleshooting

### Workspace Not Found

```bash
xchecker project init my-workspace
```

### Spec Not in Workspace

Add it:
```bash
xchecker project add-spec <spec-id>
```

### TUI Rendering Issues

Try different terminal:
- Windows: Windows Terminal recommended
- macOS/Linux: iTerm2, Alacritty, or Kitty

### Stale Status

Specs become stale after 7 days of inactivity. To reset:
```bash
xchecker resume <spec-id> --phase <next-phase>
```

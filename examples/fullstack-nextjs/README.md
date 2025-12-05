# Full-Stack Next.js Example

This example demonstrates how to use xchecker to spec out a full-stack Next.js application.

## Overview

This showcase walks through the complete xchecker workflow for building a task management application with:
- Next.js 14 App Router
- TypeScript
- Prisma ORM with PostgreSQL
- NextAuth.js authentication
- Tailwind CSS styling

## Prerequisites

- xchecker installed (`cargo install xchecker`)
- Claude CLI installed and authenticated
- Node.js 18+ (for the actual Next.js development)

## Quick Start

### 1. Initialize the Spec

```bash
cd examples/fullstack-nextjs

# Initialize the spec from the template
xchecker template init fullstack-nextjs task-manager

# Or manually create with a problem statement
echo "Build a task management app with user auth and real-time updates" | xchecker spec task-manager
```

### 2. Run Through Phases

```bash
# Generate requirements
xchecker resume task-manager --phase requirements

# Review and continue to design
xchecker resume task-manager --phase design

# Generate implementation tasks
xchecker resume task-manager --phase tasks

# Check status at any point
xchecker status task-manager --json
```

### 3. Review Outputs

After each phase, review the generated artifacts in `.xchecker/specs/task-manager/artifacts/`:
- `00-requirements.md` - Detailed requirements
- `01-design.md` - Architecture and design
- `02-tasks.md` - Implementation tasks

## Directory Structure

```
examples/fullstack-nextjs/
├── README.md                    # This file
├── workspace.yaml               # Workspace configuration
├── .xchecker/
│   ├── config.toml             # xchecker configuration
│   └── specs/
│       └── task-manager/       # Example spec
│           ├── context/
│           │   └── problem-statement.md
│           └── artifacts/      # Generated artifacts
└── scripts/
    └── validate.sh             # Validation script for CI
```

## Configuration

The `.xchecker/config.toml` is pre-configured for Next.js development:

```toml
[defaults]
model = "haiku"

[selectors]
include = [
    "src/**/*.ts",
    "src/**/*.tsx",
    "app/**/*.ts",
    "app/**/*.tsx",
    "*.json",
    "*.md"
]
exclude = [
    "node_modules/**",
    ".next/**",
    "dist/**"
]
```

## Workflow Tips

### Dry Run Mode

Test the workflow without making Claude API calls:

```bash
xchecker resume task-manager --phase requirements --dry-run
```

### Check Gate Status

Verify the spec meets quality gates:

```bash
xchecker gate task-manager --min-phase design
```

### View Detailed Status

```bash
xchecker status task-manager --json | jq .
```

## CI Integration

This example includes a validation script that can be run in CI:

```bash
./scripts/validate.sh
```

The script verifies:
- Workspace configuration is valid
- Spec structure is correct
- Configuration files are parseable

## Next Steps

After completing the spec workflow:

1. Review the generated tasks in `artifacts/02-tasks.md`
2. Set up your Next.js project based on the design
3. Use the tasks as a development roadmap
4. Run `xchecker gate` in CI to ensure spec compliance

## Related Documentation

- [xchecker Configuration Guide](../../docs/CONFIGURATION.md)
- [Claude Code Integration](../../docs/CLAUDE_CODE_INTEGRATION.md)
- [CI Templates](../../docs/ci/gitlab.md)

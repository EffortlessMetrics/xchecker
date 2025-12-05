# Mono-Repo Example

This example demonstrates how to use xchecker to manage multiple specs within a single workspace, typical of a mono-repo setup.

## Overview

This showcase demonstrates:
- Managing multiple related specs in one workspace
- Using tags to organize specs by team/domain
- Workspace-level status and history tracking
- Gate commands for CI integration

## Scenario

A fictional e-commerce platform with three microservices:
1. **user-service** - User authentication and profile management (Rust)
2. **product-catalog** - Product listing and search (Python/FastAPI)
3. **order-api** - Order processing and fulfillment (Rust)

## Prerequisites

- xchecker installed (`cargo install xchecker`)
- Claude CLI installed and authenticated

## Quick Start

### 1. Explore the Workspace

```bash
cd examples/mono-repo

# List all specs in the workspace
xchecker project list

# View workspace status
xchecker project status --json
```

### 2. Work on Individual Specs

```bash
# Check status of a specific spec
xchecker status user-service

# Resume a spec from a specific phase
xchecker resume user-service --phase requirements

# Run in dry-run mode
xchecker resume product-catalog --phase design --dry-run
```

### 3. Use Tags for Filtering

The workspace uses tags to organize specs:
- `backend` - All backend services
- `rust` - Rust-based services
- `python` - Python-based services
- `auth` - Authentication-related
- `core` - Core business logic

### 4. Gate Commands for CI

```bash
# Check if all specs meet minimum requirements
xchecker gate user-service --min-phase requirements
xchecker gate product-catalog --min-phase requirements
xchecker gate order-api --min-phase requirements

# Check with age constraints
xchecker gate user-service --min-phase design --max-phase-age 7d
```

## Directory Structure

```
examples/mono-repo/
├── README.md                    # This file
├── workspace.yaml               # Workspace with multiple specs
├── .xchecker/
│   ├── config.toml             # Shared configuration
│   └── specs/
│       ├── user-service/       # Rust user service spec
│       │   └── context/
│       │       └── problem-statement.md
│       ├── product-catalog/    # Python product catalog spec
│       │   └── context/
│       │       └── problem-statement.md
│       └── order-api/          # Rust order API spec
│           └── context/
│               └── problem-statement.md
└── scripts/
    ├── validate.sh             # Unix validation script
    └── validate.ps1            # Windows validation script
```

## Workspace Configuration

The `workspace.yaml` defines all specs and their metadata:

```yaml
version: "1"
name: ecommerce-platform
specs:
  - id: user-service
    tags: [backend, rust, auth]
    added: "2024-12-01T00:00:00Z"
  - id: product-catalog
    tags: [backend, python, core]
    added: "2024-12-01T00:00:00Z"
  - id: order-api
    tags: [backend, rust, core]
    added: "2024-12-01T00:00:00Z"
```

## Shared Configuration

All specs share the same `.xchecker/config.toml`:

```toml
[defaults]
model = "haiku"

[selectors]
include = [
    "src/**/*.rs",
    "src/**/*.py",
    "*.toml",
    "*.yaml",
    "*.md"
]
exclude = [
    "target/**",
    "__pycache__/**",
    ".venv/**"
]
```

## CI Integration

### GitHub Actions Example

```yaml
name: Spec Gate

on:
  pull_request:
    paths:
      - '.xchecker/**'

jobs:
  gate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install xchecker
        run: cargo install xchecker
      
      - name: Gate all specs
        run: |
          cd examples/mono-repo
          xchecker gate user-service --min-phase requirements
          xchecker gate product-catalog --min-phase requirements
          xchecker gate order-api --min-phase requirements
```

### Validation Script

Run the validation script to verify the example structure:

```bash
# Unix
./scripts/validate.sh

# Windows
.\scripts\validate.ps1
```

## Workflow Tips

### Batch Operations

Process multiple specs in sequence:

```bash
for spec in user-service product-catalog order-api; do
    echo "Processing $spec..."
    xchecker status $spec
done
```

### Workspace History

View the history of all specs:

```bash
xchecker project history user-service --json
```

### TUI Mode

Launch the terminal UI for workspace overview:

```bash
xchecker project tui
```

## Related Documentation

- [Workspace Model](../../docs/CONFIGURATION.md#workspace)
- [Gate Command](../../docs/CONFIGURATION.md#gate)
- [CI Templates](../../docs/ci/gitlab.md)

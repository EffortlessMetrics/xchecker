# Dependency Policy

## Overview

This document defines the dependency layering rules for the xchecker workspace. These rules ensure the codebase remains maintainable and prevents circular dependencies.

## Dependency Layers

The workspace is organized into the following acyclic layers:

### Foundation Layer
- **Purpose**: Essential shared types and utilities with minimal dependencies
- **Crates**: `xchecker-core` (to be created), essential utilities
- **Rule**: No dependencies on upper layers

### Infrastructure Layer  
- **Purpose**: Cross-cutting infrastructure services
- **Crates**: `xchecker-utils`, `xchecker-config`, `xchecker-llm`, `xchecker-runner`
- **Rule**: May depend on Foundation layer only

### Domain Layer
- **Purpose**: Business logic and domain-specific functionality
- **Crates**: `xchecker-engine-*` (status, gate, workspace, etc.), packet, fixup, orchestrator, phases
- **Rule**: May depend on Foundation and Infrastructure layers

### Application Layer
- **Purpose**: User-facing interfaces
- **Crates**: `xchecker-cli`, `xchecker-tui`, `xchecker-error-reporter`
- **Rule**: May depend on all lower layers

## The Golden Rule

**No crate may depend "up" the stack.**

If two peer crates need to share types, those types must move **down** to a lower layer (typically Foundation or Infrastructure), never sideways.

## Enforcement

1. Use `cargo tree` to verify the dependency graph remains acyclic
2. Any new dependency that violates layering must be rejected
3. Shared traits/interfaces between peers belong in the lowest common ancestor layer

## Examples

### Correct
```
xchecker-cli (App) → xchecker-engine (Domain) → xchecker-config (Infra) → xchecker-core (Foundation)
```

### Incorrect (creates cycle)
```
xchecker-orchestrator ↔ xchecker-phases
```

### Solution
```
xchecker-orchestrator → xchecker-phase-api → xchecker-phases
```

## Modularization Guidelines

When extracting a new crate:

1. Identify the layer it belongs to based on its purpose
2. Ensure all dependencies are from lower layers only
3. If peer dependencies exist, extract shared types to a lower layer first
4. Verify with `cargo tree` after extraction

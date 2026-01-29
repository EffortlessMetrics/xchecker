# JSON Schema Contracts

xchecker provides versioned JSON schemas for all structured outputs to enable reliable automation and integration. This document describes the schema versioning policy, compatibility guarantees, and migration procedures.

## Overview

All JSON outputs from xchecker include a `schema_version` field and follow strict versioning rules to ensure API stability. The current schema version is **v1**.

### Supported Schemas

- **Receipt Schema v1** (`schemas/receipt.v1.json`): Execution receipts with error tracking
- **Status Schema v1** (`schemas/status.v1.json`): Spec status and configuration
- **Doctor Schema v1** (`schemas/doctor.v1.json`): Health check results

## Schema Versioning Policy

### Current Schema Version

**v1** is the current stable schema version for all xchecker JSON outputs. All schemas follow strict versioning rules to ensure API stability and reliable automation.

### Versioning Rules

#### 1. Additive Changes (No Version Bump Required)

The following changes can be made to existing schemas without incrementing the version:

- Adding new optional fields
- Adding new enum values
- Adding new array elements
- Expanding validation constraints (e.g., increasing maxLength)
- Adding new optional nested objects
- Adding new optional array fields

**Rationale**: Consumers should ignore unknown fields (`additionalProperties: true`), making these changes backward compatible.

**Example**:
```json
// v1 schema can add new optional field
{
  "schema_version": "1",
  "new_optional_field": "value",  // Consumers ignore if unknown
  "new_optional_array": [],       // Consumers ignore if unknown
  "new_optional_object": {}       // Consumers ignore if unknown
}
```

**Recent Additive Changes in v1:**
- Added `stderr_redacted` (optional) to receipts
- Added `runner_distro` (optional) to receipts
- Added `error_kind` (optional) to receipts
- Added `error_reason` (optional) to receipts
- Added `warnings` (optional) to receipts
- Added `fallback_used` (optional) to receipts
- Added `diff_context` (optional) to receipts
- Added `llm` (optional) to receipts for provider metadata
- Added `pipeline` (optional) to receipts for execution strategy metadata
- Added `pending_fixups` (optional) to status
- Added `lock_drift` (optional) to status
- Added `canonicalization_backend` to all outputs
- Added `canonicalization_version` to all outputs

#### 2. Breaking Changes (Require Version Bump)

The following changes require incrementing the schema version:

- Removing fields
- Renaming fields
- Changing field types
- Making optional fields required
- Removing enum values
- Tightening validation constraints (e.g., reducing maxLength, adding required patterns)
- Changing array element structure
- Changing object structure
- Changing semantic meaning of existing fields

**Rationale**: These changes break existing consumers and require explicit migration.

**Example**:
```json
// v1 → v2 breaking change
// v1: "timestamp": "2025-10-24T14:30:00Z"
// v2: "emitted_at": "2025-10-24T14:30:00Z"  // Field renamed
```

#### 3. Schema Stability Guarantee

**v1 Stability Promise:**
- No breaking changes will be made to v1 schemas
- Only additive changes allowed
- v1 will remain supported indefinitely
- Deprecation warnings will be added before v2 introduction

**v2 Introduction Criteria:**
- Breaking changes are necessary for significant improvements
- Migration path is well-documented
- Dual support period (v1 + v2) is planned
- Community feedback is incorporated

### Schema Version Lifecycle

1. **v1 Stability**: v1 will remain stable with no breaking changes
2. **v2 Introduction**: When breaking changes are needed, v2 will be introduced
3. **Dual Support**: Both v1 and v2 will be supported during transition
4. **Deprecation Period**: v1 support maintained for **at least 6 months** after v2 release
5. **Removal**: v1 may be removed in a major version bump after the 6-month window

### Migration Timeline Example

```
Month 0: v2 released
  - v1 and v2 both supported
  - v1 outputs include deprecation warnings
  - Documentation updated with migration guide

Month 1-6: Transition period
  - Both versions fully supported
  - Users migrate at their own pace
  - Support available for migration issues

Month 6+: v1 deprecation
  - v1 support may be removed in next major version
  - Advance notice provided in release notes
  - Final migration deadline announced
```

## Compatibility Guarantees

### Forward Compatibility

**Consumers MUST ignore unknown fields** to support forward compatibility:

```rust
// Good: Ignores unknown fields
#[derive(Deserialize)]
struct Receipt {
    schema_version: String,
    emitted_at: String,
    // ... known fields
}

// Bad: Rejects unknown fields
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt { /* ... */ }
```

All schemas specify `additionalProperties: true` to allow producers to emit additional fields without breaking consumers.

### Backward Compatibility

**Producers MAY emit additional fields** without version bump:

```json
{
  "schema_version": "1",
  "emitted_at": "2025-10-24T14:30:00Z",
  "new_optional_field": "value"  // Added without version bump
}
```

Consumers on older versions will ignore the new field, maintaining compatibility.

### Canonical Emission

All JSON outputs use **JCS (RFC 8785)** for canonical emission:

- Deterministic key ordering (sorted alphabetically)
- Stable number formatting
- Consistent whitespace
- Reproducible output across platforms

**Benefits**:
- Stable diffs in version control
- Reliable equality comparisons
- Predictable output for testing

**Example**:
```json
{
  "artifacts": [...],
  "canonicalization_backend": "jcs-rfc8785",
  "emitted_at": "2025-10-24T14:30:00Z",
  "schema_version": "1"
}
```
Keys are always sorted alphabetically, regardless of insertion order.

### Array Ordering

Arrays are sorted before emission to ensure deterministic output:

- **Receipts**: `outputs` sorted by `path`
- **Status**: `artifacts` sorted by `path`
- **Doctor**: `checks` sorted by `name`

**Example**:
```json
{
  "artifacts": [
    {"path": "artifacts/00-requirements.md", "blake3_first8": "abc12345"},
    {"path": "artifacts/10-design.md", "blake3_first8": "def67890"}
  ]
}
```
Always sorted by path, never insertion order.

## Schema Validation

### CI Validation

All schemas are validated in CI:

```bash
# Schema validation test
cargo test schema_validation

# Snapshot tests for canonical emission
cargo test test_canonical_emission
cargo test test_array_ordering
```

### Manual Validation

Validate JSON outputs against schemas:

```bash
# Using jsonschema CLI
jsonschema -i output.json schemas/receipt.v1.json

# Using jq
jq -e '.schema_version == "1"' output.json
```

### Example Payloads

Minimal and full examples are provided in `docs/schemas/`:

- `receipt.v1.minimal.json` - Minimal valid receipt
- `receipt.v1.full.json` - Comprehensive receipt with all optional fields
- `status.v1.minimal.json` - Minimal valid status
- `status.v1.full.json` - Comprehensive status with all optional fields
- `doctor.v1.minimal.json` - Minimal valid doctor output
- `doctor.v1.full.json` - Comprehensive doctor output

## Schema Details

### Receipt Schema v1

**Purpose**: Record execution details, errors, and artifacts for each phase.

**Key Fields**:
- `schema_version`: Always "1"
- `emitted_at`: RFC3339 UTC timestamp
- `exit_code`: Process exit code (0 = success)
- `error_kind`: Structured error type (null on success)
- `error_reason`: Human-readable error message (null on success)
- `outputs`: Array of generated files (sorted by path)
- `warnings`: Array of warning messages

**Exit Code Alignment**: The `exit_code` field MUST match the process exit code.

**Error Kinds**:
- `cli_args`: Invalid CLI arguments (exit code 2)
- `packet_overflow`: Packet size exceeded (exit code 7)
- `secret_detected`: Secret detected (exit code 8)
- `lock_held`: Lock conflict (exit code 9)
- `phase_timeout`: Phase timeout (exit code 10)
- `claude_failure`: LLM Provider failure (e.g. Claude CLI, Gemini CLI) (exit code 70)
- `unknown`: Other errors (exit code 1)

**Example**:
```json
{
  "schema_version": "1",
  "emitted_at": "2025-10-24T14:30:00Z",
  "exit_code": 10,
  "error_kind": "phase_timeout",
  "error_reason": "Phase 'requirements' exceeded 600s timeout",
  "warnings": ["phase_timeout:600"]
}
```

### Status Schema v1

**Purpose**: Report current spec status, configuration, and drift detection.

**Key Fields**:
- `schema_version`: Always "1"
- `emitted_at`: RFC3339 UTC timestamp
- `runner`: Execution mode ("native" or "wsl")
- `artifacts`: Array of generated artifacts (sorted by path)
- `effective_config`: Configuration with source attribution
- `lock_drift`: Optional drift detection results

**Effective Config Structure**:

The `effective_config` field maps configuration keys to objects with:
- `value`: The effective value (arbitrary JSON type)
- `source`: Where the value came from ("cli", "env", "config", "programmatic", or "default")

**Example**:
```json
{
  "schema_version": "1",
  "emitted_at": "2025-01-01T00:00:00Z",
  "runner": "native",
  "runner_distro": null,
  "fallback_used": false,
  "canonicalization_version": "yaml-v1,md-v1",
  "canonicalization_backend": "jcs-rfc8785",
  "artifacts": [],
  "last_receipt_path": ".xchecker/receipts/example-spec/requirements-20250101_000000.json",
  "effective_config": {
    "model": {
      "value": "sonnet",
      "source": "cli"
    },
    "max_turns": {
      "value": 6,
      "source": "config"
    },
    "packet_max_bytes": {
      "value": 65536,
      "source": "default"
    }
  }
}
```

**Value Types**: The `value` field can be any valid JSON type:
- String: `"sonnet"`
- Number: `6`, `65536`, `3.14`
- Boolean: `true`, `false`
- Null: `null`
- Array: `["item1", "item2"]`
- Object: `{"nested": "value"}`

**Lock Drift Structure**:

When a lockfile exists and drift is detected:

```json
{
  "lock_drift": {
    "model_full_name": {
      "locked": "sonnet",
      "current": "sonnet"
    },
    "claude_cli_version": {
      "locked": "0.8.1",
      "current": "0.9.0"
    },
    "schema_version": null
  }
}
```

Each drift field is either:
- `null`: No drift detected
- `{"locked": "...", "current": "..."}`: Drift detected with old and new values

### Doctor Schema v1

**Purpose**: Report environment health check results.

**Key Fields**:
- `schema_version`: Always "1"
- `emitted_at`: RFC3339 UTC timestamp
- `ok`: Overall health status (false if any check fails)
- `checks`: Array of health checks (sorted by name)

**Check Status Values**:
- `pass`: Check succeeded
- `warn`: Non-critical issue detected
- `fail`: Critical issue detected

**Exit Code Behavior**:
- Normal mode: Exit 0 if `ok == true` (warnings allowed)
- Strict mode (`--strict-exit`): Exit 1 if any check is not `pass`

**Example**:
```json
{
  "schema_version": "1",
  "emitted_at": "2025-10-24T12:00:00Z",
  "ok": true,
  "checks": [
    {
      "name": "claude_path",
      "status": "pass",
      "details": "Found claude at /usr/local/bin/claude"
    },
    {
      "name": "wsl_availability",
      "status": "warn",
      "details": "WSL not installed (Windows only)"
    }
  ]
}
```

## Migration Guide

### When v2 is Released

1. **Review Breaking Changes**: Read release notes for all breaking changes
2. **Update Consumers**: Modify code to handle new schema structure
3. **Test with v2**: Validate against v2 schema in test environment
4. **Gradual Rollout**: Deploy to production incrementally
5. **Monitor**: Watch for schema validation errors

### Example Migration (v1 → v2)

Hypothetical example if `timestamp` → `emitted_at` were a v2 change:

```rust
// v1 consumer
#[derive(Deserialize)]
struct Receipt {
    schema_version: String,
    timestamp: String,  // Old field
}

// v2 consumer
#[derive(Deserialize)]
struct Receipt {
    schema_version: String,
    emitted_at: String,  // New field
}

// Dual-version consumer (during transition)
#[derive(Deserialize)]
struct Receipt {
    schema_version: String,
    #[serde(alias = "timestamp")]  // Accept old name
    emitted_at: String,
}
```

### Deprecation Warnings

During the transition period, v1 outputs may include deprecation warnings:

```json
{
  "schema_version": "1",
  "warnings": [
    "schema_v1_deprecated: Please migrate to schema v2 by 2026-04-24"
  ]
}
```

## Best Practices

### For Consumers

1. **Ignore Unknown Fields**: Always allow `additionalProperties`
2. **Check Schema Version**: Validate `schema_version` field on parse
3. **Handle Multiple Versions**: Support both old and new versions during transition
4. **Validate Against Schema**: Use JSON Schema validation in tests
5. **Monitor Deprecation Warnings**: Watch for deprecation notices in outputs

### For Producers

1. **Emit Schema Version**: Always include `schema_version` field
2. **Use Canonical Emission**: Emit via JCS for stable diffs
3. **Sort Arrays**: Sort arrays before emission for deterministic output
4. **Document Changes**: Update CHANGELOG.md for all schema changes
5. **Provide Examples**: Include minimal and full examples for new fields

### For CI/CD

1. **Schema Validation**: Validate all outputs against schemas
2. **Snapshot Tests**: Test canonical emission and array ordering
3. **Version Checks**: Fail if unexpected schema version detected
4. **Deprecation Monitoring**: Alert on deprecation warnings

## Support

### Questions and Issues

For questions about schema compatibility:
- Open an issue on GitHub
- Check CHANGELOG.md for migration guides
- Review example payloads in `docs/schemas/`

### Schema Validation Errors

If you encounter schema validation errors:

1. Check the schema version in the output
2. Validate against the correct schema file
3. Review example payloads for reference
4. Check CHANGELOG.md for recent changes

### Requesting Schema Changes

To request schema changes:

1. Open a GitHub issue describing the use case
2. Specify whether it's additive or breaking
3. Provide example payloads showing desired structure
4. Explain backward compatibility impact

## References

- [JSON Canonicalization Scheme (RFC 8785)](https://datatracker.ietf.org/doc/html/rfc8785)
- [JSON Schema Specification](https://json-schema.org/)
- [Semantic Versioning](https://semver.org/)
- [Keep a Changelog](https://keepachangelog.com/)

## Appendix: Schema Files

All schema files are located in `schemas/`:

- `schemas/receipt.v1.json` - Receipt schema definition
- `schemas/status.v1.json` - Status schema definition
- `schemas/doctor.v1.json` - Doctor schema definition

Example payloads are located in `docs/schemas/`:

- `docs/schemas/receipt.v1.minimal.json`
- `docs/schemas/receipt.v1.full.json`
- `docs/schemas/status.v1.minimal.json`
- `docs/schemas/status.v1.full.json`
- `docs/schemas/doctor.v1.minimal.json`
- `docs/schemas/doctor.v1.full.json`

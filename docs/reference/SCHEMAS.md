# JSON Schemas

xchecker uses versioned JSON schemas for all structured output. All schemas follow
the stability guarantees described in [Contracts](CONTRACTS.md).

## Available Schemas

| Schema | File | Description |
|--------|------|-------------|
| Receipt v1 | [receipt.v1.full.json](../schemas/receipt.v1.full.json) | Phase execution receipt |
| Receipt v1 (minimal) | [receipt.v1.minimal.json](../schemas/receipt.v1.minimal.json) | Minimal receipt variant |
| Status v1 | [status-json.v1.json](../schemas/status-json.v1.json) | Spec status output |
| Status v1 (full) | [status.v1.full.json](../schemas/status.v1.full.json) | Full status example |
| Status v1 (minimal) | [status.v1.minimal.json](../schemas/status.v1.minimal.json) | Minimal status example |
| Doctor v1 | [doctor.v1.full.json](../schemas/doctor.v1.full.json) | Health check output |
| Doctor v1 (minimal) | [doctor.v1.minimal.json](../schemas/doctor.v1.minimal.json) | Minimal doctor variant |
| Gate v1 | [gate-json.v1.json](../schemas/gate-json.v1.json) | Gate command output |
| Spec v1 | [spec-json.v1.json](../schemas/spec-json.v1.json) | Spec generation output |
| Resume v1 | [resume-json.v1.json](../schemas/resume-json.v1.json) | Resume operation output |
| Workspace Status v1 | [workspace-status-json.v1.json](../schemas/workspace-status-json.v1.json) | Workspace status |
| Workspace History v1 | [workspace-history-json.v1.json](../schemas/workspace-history-json.v1.json) | Workspace history |

## Schema Definitions

The canonical schema definitions used for validation live in the top-level
`schemas/` directory:

| Schema | File |
|--------|------|
| Receipt v1 | [receipt.v1.json](../../schemas/receipt.v1.json) |
| Status v1 | [status.v1.json](../../schemas/status.v1.json) |
| Doctor v1 | [doctor.v1.json](../../schemas/doctor.v1.json) |

## Validation

Schemas can be validated using any JSON Schema Draft 2020-12 validator:

```bash
# Using jsonschema CLI
jsonschema -i output.json schemas/receipt.v1.json

# Quick version check with jq
jq -e '.schema_version == "1"' output.json
```

See [Contracts](CONTRACTS.md) for versioning policy and compatibility guarantees.

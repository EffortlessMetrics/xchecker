# Running Real LLM Tests

This document explains how to run the real LLM integration tests locally.

## Prerequisites

1. **Claude CLI installed and authenticated**:
   ```bash
   claude --version
   ```

2. **Claude CLI on PATH** (or specify `--llm-claude-binary` in config)

## Why Tests Are Ignored by Default

Tests that make real LLM calls are marked with `#[ignore]` to:
- Prevent unexpected token spend in CI
- Allow contributors without Claude CLI to run the test suite
- Keep default test runs fast and deterministic

## Running Real LLM Tests

### Quick: Run All Ignored E2E Tests
```bash
cargo test --test test_end_to_end_workflows -- --ignored
```

### With Haiku (Recommended for Cost Efficiency)

Use the test config to force Haiku model:

```bash
# Windows PowerShell
$env:XCHECKER_CONFIG = "tests/config-haiku.toml"
cargo test --test test_end_to_end_workflows -- --ignored

# Unix/WSL
XCHECKER_CONFIG=tests/config-haiku.toml cargo test --test test_end_to_end_workflows -- --ignored
```

Or set the model directly via defaults:
```bash
# Windows PowerShell
$env:XCHECKER_DEFAULTS_MODEL = "haiku"
cargo test --test test_end_to_end_workflows -- --ignored

# Unix/WSL
XCHECKER_DEFAULTS_MODEL=haiku cargo test --test test_end_to_end_workflows -- --ignored
```

### Required Environment Variables

Real LLM tests require explicit opt-in to prevent accidental runs that incur costs:

```bash
# Windows PowerShell
$env:XCHECKER_REAL_LLM_TESTS = "1"
$env:XCHECKER_DEFAULTS_MODEL = "haiku"
cargo test --test test_end_to_end_workflows -- --ignored

# Unix/WSL
XCHECKER_REAL_LLM_TESTS=1 \
XCHECKER_DEFAULTS_MODEL=haiku \
cargo test --test test_end_to_end_workflows -- --ignored
```

To explicitly skip real LLM tests (even if you run with `--ignored`), set:

```bash
XCHECKER_SKIP_LLM_TESTS=1
```

Without `XCHECKER_REAL_LLM_TESTS=1`, the tests will skip with a message directing you here.

## Available Model Aliases

The Claude CLI supports these model aliases:
- `haiku` - Claude 3 Haiku (cheapest, fastest)
- `sonnet` - Claude 3.5 Sonnet (default)
- `opus` - Claude 3 Opus (most capable)

For testing, **always use `haiku`** to minimize cost.

## Test Files That Make Real LLM Calls

| File | Tests | Description |
|------|-------|-------------|
| `test_end_to_end_workflows.rs` | 3 | Full spec generation, resume, complete E2E |

## Cost Expectations

With Haiku:
- Single test run: ~$0.001-0.005
- Full E2E suite: ~$0.01-0.02

With Sonnet:
- Single test run: ~$0.01-0.05
- Full E2E suite: ~$0.10-0.20

## CI Behavior

CI runs do NOT execute `#[ignore]` tests. This is intentional to:
1. Avoid surprise token costs
2. Keep CI deterministic
3. Allow parallel runs without rate limiting issues

## Troubleshooting

### "Claude CLI binary not found"
Ensure `claude` is in your PATH:
```bash
which claude  # Unix
where claude  # Windows
```

### Tests hang
Some Claude CLI tests may hang when run in parallel. Run individually:
```bash
cargo test test_full_spec_generation_workflow -- --ignored
```

### Authentication issues
Re-authenticate Claude CLI:
```bash
claude login
```

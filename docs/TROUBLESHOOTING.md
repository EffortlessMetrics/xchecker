# Troubleshooting Guide

This document captures common issues and their stable fixes.

## Test Helper Visibility Issues

**Problem:** Integration tests fail with `E0432: unresolved import` when trying to use test helpers like `redact_error_message_for_testing`.

**Root Cause:** `#[cfg(test)]` only applies to the crate's own unit tests, not integration tests (which compile as separate crates).

**Solution:**
```rust
// src/llm/http_client.rs
#[doc(hidden)]
pub fn redact_error_message_for_testing(message: &str) -> String {
    redact_error_message(message)
}

// src/llm/mod.rs
#[doc(hidden)]
pub use http_client::redact_error_message_for_testing;
```

Key points:
- No `#[cfg(test)]` on the symbol
- Use `pub` so integration tests can import it
- Use `#[doc(hidden)]` to keep it out of public API docs

## Doctor HTTP Provider Check Failures

**Problem:** Tests fail with "cannot find the file specified" or assertion mismatches on Pass/Fail status.

**Root Cause:** Tests weren't creating realistic config environments before calling `Config::discover()`.

**Solution:**
1. Create a temp directory for each test
2. Set `HOME`/`USERPROFILE` to that temp dir
3. Create `.xchecker/config.yml` with minimal valid config:
   ```yaml
   [llm]
   provider = "openrouter"  # or "anthropic"
   
   [llm.openrouter]  # or [llm.anthropic]
   api_key_env = "OPENROUTER_API_KEY"
   model = "some-model"
   ```
4. Call `Config::discover(&cli_args)` safely

**Expected Behavior:**
- Pass: when env var + model are present
- Fail with "API key" message: when key is missing
- Fail with "model"/"not configured" message: when model is missing

## Config Struct Field Changes

**Problem:** Adding new fields to `LlmConfig` causes `E0063: missing field` errors across the codebase.

**Solution:** When adding optional fields like `fallback_provider`, update all struct initializations:
```rust
LlmConfig {
    provider: "claude-cli".to_string(),
    fallback_provider: None,  // Add this
    // ... existing fields
}
```

Check these locations:
- `config::LlmConfig::minimal_for_testing`
- `Orchestrator` initialization in `llm.rs`
- Any test code constructing `LlmConfig` directly

## Invalid Provider Tests

**Problem:** Tests expecting "invalid provider" errors start passing after implementing new providers.

**Solution:** Use a truly bogus provider name in test configs:
```toml
[llm]
provider = "totally-invalid-provider"
```

Don't use real provider names like "anthropic" or "openrouter" in invalid-provider tests.

## Quick Recovery Checklist

If you hit a regression, verify:

1. ✓ Test helpers are `pub` + `#[doc(hidden)]`, not `#[cfg(test)]`
2. ✓ All `LlmConfig` initializations include new fields
3. ✓ Doctor tests create config files before `Config::discover()`
4. ✓ Invalid-provider tests use bogus provider names
5. ✓ Run `cargo test --lib --bins` to verify unit + integration tests

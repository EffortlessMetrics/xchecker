# Structured Logging (FR-OBS-001)

This document describes the structured logging implementation in xchecker using the `tracing` crate.

## Overview

The logging system provides two modes:
- **Compact mode** (default): Human-readable, minimal output
- **Verbose mode** (`--verbose`): Structured logs with spec_id, phase, duration_ms, and runner_mode fields

## Initialization

Initialize tracing at the start of your application:

```rust
use xchecker::logging::init_tracing;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let verbose = true; // or false for compact mode
    init_tracing(verbose)?;
    
    // Your application code...
    Ok(())
}
```

## Using the Logger

### Basic Usage

```rust
use xchecker::logging::Logger;

let mut logger = Logger::new(verbose);

// Set structured context (FR-OBS-001)
logger.set_spec_id("my-spec".to_string());
logger.set_phase("requirements".to_string());
logger.set_runner_mode("native".to_string());

// Log messages with structured fields
logger.info("Starting phase execution");
logger.warn("Warning during execution");
logger.error("Error occurred");
```

### Verbose Logging

```rust
// Verbose messages (only shown when verbose=true)
logger.verbose("Detailed operation information");
logger.verbose_fmt(format_args!("Processing file: {}", filename));
```

## Standalone Phase Logging Functions

For phase-level logging without a Logger instance:

```rust
use xchecker::logging::{log_phase_start, log_phase_complete, log_phase_error};

// Log phase start
log_phase_start("my-spec", "requirements", "native");

// Log phase completion with duration
log_phase_complete("my-spec", "requirements", 5000); // 5000ms

// Log phase error
log_phase_error("my-spec", "requirements", "timeout occurred", 10000);
```

## Using Tracing Spans

For more advanced structured logging with spans:

```rust
use xchecker::logging::phase_span;

let span = phase_span("my-spec", "design", "wsl");
let _guard = span.enter();

// All logs within this scope will be associated with the span
log_phase_start("my-spec", "design", "wsl");
// ... do work ...
log_phase_complete("my-spec", "design", 2000);
```

## Required Fields (FR-OBS-001)

When verbose mode is enabled, the following structured fields are included in logs:

- **spec_id**: The specification identifier
- **phase**: The current phase (requirements, design, tasks, review, fixup, final)
- **duration_ms**: Elapsed time in milliseconds since logger creation
- **runner_mode**: The runner mode (native, wsl, auto)

## Output Examples

### Compact Mode (Default)

```
2025-11-24T01:20:01.964828Z  INFO Starting phase execution
2025-11-24T01:20:01.965045Z  WARN Warning during execution
2025-11-24T01:20:01.965062Z ERROR Error occurred
```

### Verbose Mode

```
2025-11-24T01:20:01.964828Z  INFO spec_id=my-spec phase=requirements runner_mode=native duration_ms=0 Starting phase execution
2025-11-24T01:20:01.965045Z  WARN spec_id=my-spec phase=requirements runner_mode=native duration_ms=217 Warning during execution
2025-11-24T01:20:01.965062Z ERROR spec_id=my-spec phase=requirements runner_mode=native duration_ms=234 Error occurred
```

## Environment Variable Configuration

You can control log levels using the `RUST_LOG` environment variable:

```bash
# Show all debug logs
RUST_LOG=xchecker=debug xchecker spec my-spec --verbose

# Show only warnings and errors
RUST_LOG=xchecker=warn xchecker spec my-spec

# Show info and above (default)
RUST_LOG=xchecker=info xchecker spec my-spec
```

## Security Considerations (FR-OBS-002, FR-OBS-003)

- Secrets are never logged (redaction is applied before logging)
- Environment variables are not included in logs
- Error messages include actionable context without exposing sensitive data

### Automatic Redaction in Logger Methods

All Logger methods (`info`, `warn`, `error`, `verbose`, `verbose_fmt`) automatically apply redaction before logging. This is implemented through two internal methods in `src/logging.rs`:

1. **`Logger::redact(&str)`**: Applies `SecretRedactor.redact_content()` to all log messages, replacing detected secrets with `[REDACTED:<pattern_id>]` markers.

2. **`Logger::sanitize(&str)`**: First checks for environment variable patterns (KEY=, TOKEN=, SECRET=, PASSWORD=), replacing them with `[ENV_VAR_REDACTED]`, then applies secret redaction.

### SecretRedactor Integration

The Logger struct includes a `SecretRedactor` instance (`Logger::redactor` field):

```rust
/// Secret redactor for sanitizing log output (FR-OBS-002, FR-OBS-003)
redactor: SecretRedactor,
```

This ensures all log output passes through the default secret pattern detectors before emission. See [SECURITY.md](SECURITY.md#default-secret-patterns) for the complete list of patterns.

## Testing

The logging system includes comprehensive unit and integration tests:

```bash
# Run unit tests
cargo test --lib logging

# Run integration tests
cargo test --test test_structured_logging
```

## Performance

The logging system is designed to have minimal performance impact:
- Structured fields are only computed when verbose mode is enabled
- Log statements are evaluated lazily by the tracing framework
- No heap allocations for disabled log levels

## Migration from println!

If you have existing code using `println!` for logging, migrate to structured logging:

```rust
// Before
println!("Starting phase: {}", phase);

// After
logger.info(&format!("Starting phase: {}", phase));

// Or with structured fields
logger.set_phase(phase.to_string());
logger.info("Starting phase");
```

## References

- [tracing crate documentation](https://docs.rs/tracing/)
- [tracing-subscriber documentation](https://docs.rs/tracing-subscriber/)
- [SECURITY.md](SECURITY.md) - Complete list of secret patterns that are automatically redacted
- FR-OBS-001: Structured logging with spec_id, phase, duration_ms, runner_mode
- FR-OBS-002: Secret redaction in logs
- FR-OBS-003: Actionable context in error logs

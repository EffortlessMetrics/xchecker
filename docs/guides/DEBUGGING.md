# Debugging Guide

This guide helps you troubleshoot common issues when using xchecker.

## Understanding xchecker Output

### Receipts

Every phase execution creates a receipt in `.xchecker/specs/<id>/receipts/`:

```
.xchecker/specs/my-spec/receipts/
├── 2025-12-06T10-30-00Z-requirements.json
├── 2025-12-06T10-35-00Z-design.json
└── 2025-12-06T10-40-00Z-tasks.json
```

Each receipt contains:
- `schema_version`: Receipt format version
- `phase`: Phase that was executed
- `exit_code`: 0 for success, non-zero for failures
- `error_kind`: Error type if failed (e.g., `phase_timeout`, `secret_detected`)
- `packet`: Evidence of what was sent to Claude
- `outputs`: Artifacts produced
- `warnings`: Any warnings generated

### Packet Context

Preview what will be sent to Claude before running a phase:

```bash
xchecker spec my-spec --debug-packet
```

This creates a context file at `.xchecker/specs/my-spec/context/` showing the exact packet.

### Verbose Mode

Enable detailed logging:

```bash
RUST_LOG=debug xchecker spec my-spec --verbose
```

Or for just xchecker logs:

```bash
RUST_LOG=xchecker=debug xchecker spec my-spec --verbose
```

## Exit Codes

| Code | Name | Description |
|------|------|-------------|
| 0 | SUCCESS | Operation completed |
| 2 | CLI_ARGS | Invalid arguments or configuration |
| 7 | PACKET_OVERFLOW | Packet exceeds size limits |
| 8 | SECRET_DETECTED | Secret found in packet |
| 9 | LOCK_HELD | Another process is running |
| 10 | PHASE_TIMEOUT | Phase exceeded timeout |
| 70 | CLAUDE_FAILURE | Claude CLI failed |

## Common Issues

### Exit Code 7: PACKET_OVERFLOW

The packet exceeds configured size limits.

**Solutions:**
1. Increase limits in config:
   ```toml
   [defaults]
   packet_max_bytes = 131072
   packet_max_lines = 2400
   ```
2. Use more specific include patterns in selectors
3. Exclude large generated files:
   ```toml
   [selectors]
   exclude = ["*.min.js", "node_modules/**", "target/**"]
   ```
4. Use `--debug-packet` to see what's being included

### Exit Code 8: SECRET_DETECTED

A potential secret pattern was found in the packet content.

**Solutions:**
1. Remove the actual secret from your source files
2. Add false positive patterns to ignore list:
   ```bash
   xchecker spec my-spec --ignore-secret-pattern 'test_api_key_\w+'
   ```
3. Check the receipt for `secret_locations` to see what triggered detection
4. For testing, you can use `--extra-secret-pattern` to add more patterns

### Exit Code 9: LOCK_HELD

Another xchecker process is running for this spec.

**Solutions:**
1. Wait for the other process to complete
2. Check status: `xchecker status my-spec`
3. If the process crashed, force override:
   ```bash
   xchecker spec my-spec --force
   ```
4. Check for stale locks (older than 1 hour) - they're automatically cleared

### Exit Code 10: PHASE_TIMEOUT

The phase exceeded the configured timeout.

**Solutions:**
1. Increase timeout:
   ```bash
   xchecker spec my-spec --phase-timeout 600
   ```
2. Check if Claude is responding slowly
3. Resume from the timed-out phase:
   ```bash
   xchecker resume my-spec --phase <phase-name>
   ```
4. Partial output may be available in artifacts with `.partial.md` suffix

### Exit Code 70: CLAUDE_FAILURE

Claude CLI execution failed.

**Solutions:**
1. Run `xchecker doctor` to check Claude CLI setup
2. Verify Claude CLI works independently: `claude --version`
3. Check API key/authentication
4. On Windows, verify WSL or native Claude installation
5. Check `--runner-mode` setting (native vs wsl)

## Inspecting Phase Artifacts

Artifacts are stored in `.xchecker/specs/<id>/artifacts/`:

```
artifacts/
├── 00-requirements.md      # Requirements document
├── 00-requirements.core.yaml  # Structured metadata
├── 10-design.md            # Design document
├── 20-tasks.md             # Task breakdown
├── 30-review.md            # Review output
└── 40-fixup.md             # Fixup plan (if any)
```

### Partial Outputs

On failure, partial outputs have `.partial.md` suffix:
```
artifacts/
└── 20-tasks.partial.md     # Partial output from failed phase
```

## Health Checks

Run comprehensive health checks:

```bash
xchecker doctor
```

For JSON output:

```bash
xchecker doctor --json
```

Key checks:
- `atomic_rename`: File system supports atomic operations
- `blake3_hashing`: BLAKE3 library works correctly
- `claude_path`: Claude CLI is available
- `config_parse`: Configuration is valid
- `spec_dir_writeable`: Can write to spec directory

## Lockfile Issues

### Missing Lockfile

First run creates a lockfile automatically. If missing:

```bash
xchecker init my-spec --create-lock
```

### Lockfile Drift

When model or Claude CLI version changes, you'll see a drift warning.

**Solutions:**
1. Accept the drift (non-strict mode continues with warning)
2. Run with `--strict-lock` to fail on drift
3. Regenerate lockfile by deleting and re-running init

### Corrupted Lockfile

If lockfile is corrupted, delete it and reinitialize:

```bash
rm .xchecker/specs/my-spec/lock.json
xchecker init my-spec --create-lock
```

## Fuzzy Matching – What Works, What Doesn't

The fixup engine uses fuzzy matching to apply diffs when line numbers have shifted. It searches ±50 lines from the expected position for matching context.

### What Works

| Scenario | Status |
|----------|--------|
| Single hunk diffs with contiguous context | Supported |
| Multi-hunk diffs with contiguous context | Supported |
| Line additions with offset tracking | Supported |
| Preview mode (never mutates files) | Supported |

### Known Limitations

| Scenario | Status | Workaround |
|----------|--------|------------|
| Context split by deletions | Not supported | Keep diffs small, avoid mixed add/delete in same hunk |
| Large line shifts with non-unique context | Not supported | Use more distinctive context lines |
| Ambiguous repeated patterns | Not supported | Ensure context is unique within ±50 lines |
| Replacement hunks with non-contiguous context | Not supported | Regenerate diff with simpler changes |

### FuzzyMatchFailed Error

When fuzzy matching fails, you'll get a structured error with:
- The context lines that couldn't be matched
- The file and expected line range
- Suggestions for remediation

**Common causes:**
1. File modified externally since review phase
2. Large structural changes made the context non-unique
3. LLM generated incorrect context in the diff

**Solutions:**
1. Rerun review: `xchecker resume my-spec --phase review`
2. Keep diffs small and focused
3. Avoid long-range edits that span many functions
4. Check that source files haven't been modified since last phase

### Best Practices for Clean Fixups

- Request small, focused changes rather than large refactors
- One logical change per fixup cycle
- Review diffs before applying: `xchecker resume my-spec --phase fixup` (preview mode)
- Apply only when confident: `xchecker resume my-spec --phase fixup --apply-fixups`

## Debugging Claude Output

### Raw Response

The raw Claude response is captured in receipts under the `raw_response` field (when available).

### Stderr Output

Claude CLI stderr is captured and included in receipts under `claude_stderr`.

## Performance Profiling

Run the benchmark command to verify performance:

```bash
xchecker benchmark
```

Key metrics:
- Empty run should complete in ≤5 seconds
- Packetization of 100 files should complete in ≤200ms

## Getting More Help

1. Check `xchecker --help` for command documentation
2. Review docs in the `docs/` directory
3. Examine receipts for detailed execution history
4. Run `xchecker doctor --json` for machine-readable diagnostics
5. Use `RUST_LOG=debug` for detailed logging

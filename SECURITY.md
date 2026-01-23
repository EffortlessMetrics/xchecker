# Security Model

This document describes the security controls implemented in xchecker.

## File System Security

### Path Sandboxing

xchecker implements strict path validation to prevent directory traversal and path escape attacks:

- **Parent traversal rejection**: Paths containing `..` components are rejected before any filesystem operations
- **Absolute path rejection**: Absolute paths are blocked to prevent arbitrary file access
- **Symlink detection**: Symlinks are rejected by default to prevent escape via symlink targets outside the sandbox
- **Hardlink detection**: Files with link count > 1 are rejected by default (Unix: via `nlink()`, Windows: via `GetFileInformationByHandle`)
- **Canonical path validation**: After joining paths, the resolved path is verified to remain within the sandbox root

The `SandboxRoot` type (in `src/paths.rs`) enforces these constraints. All fixup operations use sandboxed paths to ensure diff application cannot escape the workspace root.

Configuration options:
- `allow_symlinks`: Permits symlinks within the sandbox (default: false)
- `allow_hardlinks`: Permits hardlinks within the sandbox (default: false)

### Atomic Writes

All artifact writes use atomic operations (in `src/atomic_write.rs`):

1. Content is written to a temporary file in the same directory
2. `fsync()` ensures data is durably written to disk
3. Atomic rename replaces the target file
4. On Windows: exponential backoff retry for transient filesystem locks (max 250ms)
5. Cross-filesystem fallback: copy, fsync, replace when rename fails with EXDEV

This prevents partial writes and ensures artifact integrity on crash or interruption.

## Secret Detection

xchecker scans all content before LLM invocation to prevent accidental secret leakage:

- **Pre-invocation scanning**: Packets are scanned for secrets before being sent to Claude
- **Execution blocking**: If secrets are detected, execution fails with exit code 8 (`SECRET_DETECTED`)
- **39 built-in patterns**: Coverage for AWS, GCP, Azure, GitHub, database URLs, private keys, and more
- **Configurable patterns**: Add custom patterns or suppress false positives via configuration
- **User-facing redaction**: Secrets in error messages and logs are replaced with `***`

Pattern categories:
- AWS credentials (access keys, secret keys, session tokens)
- GCP credentials (API keys, service account keys)
- Azure credentials (storage keys, connection strings, SAS tokens)
- Generic API tokens (Bearer, Basic auth, OAuth, JWT)
- Database connection URLs (PostgreSQL, MySQL, MongoDB, Redis)
- SSH/PEM private keys
- Platform tokens (GitHub, GitLab, Slack, Stripe, npm, PyPI)

## Process Execution

### Internal Commands

All internal process execution uses `CommandSpec` (in `src/runner.rs`) which enforces argv-style invocation:

- Arguments are stored as `Vec<OsString>`, not shell strings
- No shell interpretation (`sh -c`, `cmd /C`) is used for internal commands
- Arguments cross trust boundaries as discrete elements, preventing shell injection

Example from the codebase:
```rust
let cmd = CommandSpec::new("claude")
    .arg("--print")
    .arg("--output-format")
    .arg("json");
```

### Hooks

Hooks are **opt-in and wired into the orchestrator**. When configured, they execute before and after each phase:

- Hooks intentionally use shell execution (`sh -c` / `cmd /C`) because they are user-defined shell commands
- Hooks run from the **invocation working directory** (typically the repository root), so relative paths like `./scripts/...` work as expected
- Context is passed via environment variables (`XCHECKER_SPEC_ID`, `XCHECKER_PHASE`, `XCHECKER_HOOK_TYPE`)
- Additional context is passed via stdin as JSON, not shell interpolation
- Users explicitly opt in by adding hook configuration
- Pre-hook failures can be configured to warn or abort the phase (`on_fail = "warn"` or `on_fail = "fail"`)
- Post-hook failures are logged as warnings (non-fatal)

Security recommendations for hooks:
- Only enable hooks in trusted environments
- Disable hooks in CI for untrusted repositories
- Review hook commands before execution
- Use `on_fail = "fail"` for pre-hooks to catch unexpected behavior

## State Isolation

### Per-Spec Directories

Each spec has isolated state under `.xchecker/specs/<spec-id>/`:

```
.xchecker/
  specs/<spec-id>/
    artifacts/    # Generated phase outputs
    receipts/     # Execution audit trails
    context/      # Packet previews for debugging
    .lock         # Execution lock file
    lock.json     # Reproducibility lockfile
```

### Lockfiles

Advisory file locks prevent concurrent modification:

- Exclusive locks are acquired before phase execution
- Lock files contain PID, creation time, and xchecker version
- Stale lock detection with configurable TTL (default: 1 hour)
- `--force` flag available to override stale locks
- Automatic cleanup on normal exit via Drop

### Receipts and Audit Trail

Execution receipts provide cryptographic audit trails:

- BLAKE3 hashes of all input and output artifacts
- JCS (RFC 8785) canonical JSON emission for deterministic hashing
- Phase metadata including duration, model, and timestamps
- Stored in `receipts/` directory with ISO timestamp filenames

## Exit Codes

Security-related exit codes:

| Code | Name | Description |
|------|------|-------------|
| 8 | SECRET_DETECTED | Secret found in packet - execution blocked |
| 9 | LOCK_HELD | Another process holds the lock |

## Reporting Security Issues

If you discover a security vulnerability, please report it by emailing the maintainers directly rather than opening a public issue.

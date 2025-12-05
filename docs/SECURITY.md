# Security Guide

This document describes xchecker's security model, secret detection and redaction system, and security best practices.

## Security Model

xchecker implements a defense-in-depth security model with multiple layers:

1. **Secret Detection**: Scan for secrets before external invocation
2. **Secret Redaction**: Redact secrets before persistence or logging
3. **Path Validation**: Prevent path traversal and unauthorized file access
4. **Sandboxing**: Restrict file operations to project tree
5. **Audit Trail**: Comprehensive receipts for all operations

## Secret Detection and Redaction (FR-SEC)

### Overview

The SecretRedactor component detects and blocks secrets before they reach Claude or get persisted to disk. This is a **hard stop** - if secrets are detected, xchecker exits with code 8 and does not proceed.

### Default Secret Patterns

xchecker detects the following secret patterns by default:

| Pattern | Description | Example |
|---------|-------------|---------|
| `ghp_[A-Za-z0-9]{36}` | GitHub Personal Access Token | `ghp_1234567890abcdefghijklmnopqrstuvwxyz` |
| `AKIA[0-9A-Z]{16}` | AWS Access Key ID | `AKIAIOSFODNN7EXAMPLE` |
| `AWS_SECRET_ACCESS_KEY=` | AWS Secret Access Key | `AWS_SECRET_ACCESS_KEY=wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY` |
| `xox[baprs]-` | Slack tokens | `xoxb-1234567890-1234567890-abcdefghijklmnopqrstuvwx` |
| `Bearer [A-Za-z0-9._-]{20,}` | Bearer tokens | `Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...` |

### Secret Detection Flow

```
┌─────────────────┐
│ Packet Assembly │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Secret Scanning │◄─── Default + Extra Patterns
└────────┬────────┘     - Ignore Patterns
         │
         ├─── Secrets Found ──► Exit Code 8
         │                      Write Error Receipt
         │                      Report Pattern Name (not secret)
         │
         └─── No Secrets ──────► Continue to Claude
                                 Write Debug Packet (if --debug-packet)
```

### Configuration

#### Adding Custom Patterns

```bash
# Add custom pattern via CLI
xchecker spec my-feature --extra-secret-pattern "SECRET_[A-Z0-9]{32}"

# Add multiple patterns
xchecker spec my-feature \
  --extra-secret-pattern "API_KEY_[A-Za-z0-9]{40}" \
  --extra-secret-pattern "TOKEN_[A-Za-z0-9]{64}"
```

```toml
# Add custom patterns via config
[security]
extra_secret_patterns = [
    "SECRET_[A-Z0-9]{32}",
    "API_KEY_[A-Za-z0-9]{40}",
    "TOKEN_[A-Za-z0-9]{64}"
]
```

#### Suppressing Patterns

```bash
# Suppress specific pattern via CLI
xchecker spec my-feature --ignore-secret-pattern "ghp_"

# Suppress multiple patterns
xchecker spec my-feature \
  --ignore-secret-pattern "ghp_" \
  --ignore-secret-pattern "Bearer"
```

```toml
# Suppress patterns via config
[security]
ignore_secret_patterns = [
    "ghp_",
    "Bearer"
]
```

**⚠️ Warning:** Suppressing patterns reduces security. Only suppress patterns if you're certain they won't match real secrets in your codebase.

### Redaction Behavior

When secrets are detected:

1. **Exit Immediately**: xchecker exits with code 8
2. **Write Error Receipt**: Receipt includes error_kind: "secret_detected"
3. **Report Pattern Name**: Receipt shows which pattern matched (not the actual secret)
4. **No Claude Invocation**: Claude is never called
5. **No Packet Writing**: Full packet is never written (even with --debug-packet)

**Example Error Receipt:**
```json
{
  "schema_version": "1",
  "emitted_at": "2025-11-27T12:00:00Z",
  "exit_code": 8,
  "error_kind": "secret_detected",
  "error_reason": "Secret detected matching pattern: ghp_",
  "warnings": []
}
```

### Global Redaction

Even when secrets are not detected during scanning, xchecker applies redaction to all human-readable strings before persistence or logging:

**Redacted Fields:**
- `stderr_redacted` in receipts
- `error_reason` in error receipts
- `warnings` array in receipts
- Context lines in error messages
- Doctor and status output text
- Preview text in fixup mode
- Log messages (when --verbose)

**Never Included:**
- Environment variables
- Raw packet content (except with --debug-packet after successful scan)
- API keys or credentials
- Full file paths with secrets

### Debug Packet Writing

The `--debug-packet` flag writes the full packet to `context/<phase>-packet.txt` for debugging purposes.

**Security Guarantees:**
1. Packet is only written **after** secret scan passes
2. If any secret is detected, packet is **never** written
3. Packet file is **excluded** from receipts
4. Packet content is **redacted** if later reported in errors
5. Packet file should be added to `.gitignore`

**Usage:**
```bash
# Write debug packet (only if no secrets detected)
xchecker spec my-feature --debug-packet

# Packet written to:
# .xchecker/specs/my-feature/context/requirements-packet.txt
```

**⚠️ Warning:** Debug packets may contain sensitive information. Never commit them to version control.

## Path Validation (FR-FIX)

### Path Security Model

xchecker validates all file paths to prevent path traversal and unauthorized access:

1. **Canonicalization**: All paths are canonicalized to absolute paths
2. **Root Boundary**: Paths must be under the allowed root directory
3. **No Traversal**: Paths with `..` components are rejected
4. **No Absolute Escapes**: Absolute paths outside root are rejected
5. **Symlink Detection**: Symlinks are rejected by default
6. **Hardlink Detection**: Hardlinks are rejected by default

### Path Validation Rules

```rust
// Valid paths (under project root)
✅ src/main.rs
✅ docs/README.md
✅ ../sibling-project/file.txt (if within allowed root)

// Invalid paths (rejected)
❌ /etc/passwd (absolute path outside root)
❌ ../../etc/passwd (traversal outside root)
❌ /tmp/symlink (symlink, unless --allow-links)
❌ /tmp/hardlink (hardlink, unless --allow-links)
```

### Symlinks and Hardlinks

By default, xchecker rejects symlinks and hardlinks to prevent:
- Unauthorized file access
- Path traversal attacks
- Confusion about file identity

**Allowing Links:**
```bash
# Allow symlinks and hardlinks in fixups
xchecker resume my-feature --phase fixup --apply-fixups --allow-links
```

**⚠️ Warning:** Only use `--allow-links` if you trust the fixup source and understand the security implications.

### Fixup Path Validation

When applying fixups, xchecker validates all target paths:

```
┌──────────────────┐
│ Parse Fixup Plan │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ Validate Paths   │◄─── Canonicalize
└────────┬─────────┘     Check Root Boundary
         │               Detect Symlinks/Hardlinks
         │
         ├─── Invalid Path ──► Exit with Error
         │                     Show Validation Error
         │
         └─── Valid Paths ────► Preview or Apply
```

## File System Security

### Sandboxing

xchecker restricts file operations to the project tree by default:

**Allowed:**
- Read files under project root
- Write artifacts to `.xchecker/specs/<spec-id>/`
- Write receipts to `.xchecker/specs/<spec-id>/receipts/`
- Write context to `.xchecker/specs/<spec-id>/context/`

**Restricted:**
- Read files outside project root (requires explicit opt-in)
- Write files outside `.xchecker/` directory
- Follow symlinks (requires --allow-links)
- Access system directories

### Atomic File Operations

All file writes use atomic operations to prevent corruption:

1. **Write to Temp**: Write to `.tmp` file first
2. **Fsync**: Flush to disk
3. **Atomic Rename**: Rename to final name (same filesystem)
4. **Fallback**: Copy+fsync+replace for cross-filesystem

**Security Benefits:**
- No partial writes on crash
- No race conditions
- No corruption from concurrent access

### Windows-Specific Security

On Windows, xchecker implements additional security measures:

1. **Job Objects**: Process tree termination on timeout
2. **Retry Logic**: Handle antivirus/indexer locks (≤250ms)
3. **Attribute Preservation**: Maintain file attributes on replace

## Receipt Security

### Receipt Content Policy

Receipts are designed to be safe for version control and sharing:

**Never Included:**
- Environment variables
- Raw packet content
- API keys or credentials
- Secrets (redacted before persistence)
- Full file paths with secrets

**Always Included:**
- Exit codes and error kinds
- Redacted error messages
- File hashes (BLAKE3)
- Timestamps (UTC)
- Configuration sources (cli/config/default)

### Receipt Redaction

Before writing receipts, xchecker applies redaction to:
- `stderr_redacted` field (capped at 2048 bytes after redaction)
- `error_reason` field
- `warnings` array
- Any human-readable strings

**Example:**
```json
{
  "stderr_redacted": "Error: Failed to connect to API\nToken: ***\nRetrying...",
  "error_reason": "Authentication failed with token ***",
  "warnings": ["Deprecated flag --old-flag, use --new-flag instead"]
}
```

## Logging Security

### Verbose Logging

When `--verbose` is enabled, xchecker logs detailed information:

**Logged:**
- File selection and sizes
- Packet assembly details
- Cache hit/miss statistics
- Phase execution timings
- Configuration sources

**Never Logged:**
- Secrets (redacted before logging)
- Environment variables
- Raw packet content
- API keys or credentials

### Log Redaction

All log messages pass through redaction before emission:

```rust
// Before logging
let message = format!("Token: {}", token);

// After redaction
let redacted = redactor.redact(&message);
// "Token: ***"

logger.info(&redacted);
```

## Security Best Practices

### 1. Never Commit Secrets

**Problem:** Secrets in version control are permanent.

**Solution:**
- Use environment variables for secrets
- Add `.xchecker/specs/*/context/*-packet.txt` to `.gitignore`
- Use secret management tools (e.g., 1Password, AWS Secrets Manager)
- Rotate secrets if accidentally committed

### 2. Review Fixups Before Applying

**Problem:** Malicious fixups could modify arbitrary files.

**Solution:**
```bash
# Always preview first (default)
xchecker resume my-feature --phase fixup

# Review intended changes
# Only apply if changes look safe
xchecker resume my-feature --phase fixup --apply-fixups
```

### 3. Use Specific Include Patterns

**Problem:** Broad patterns may include sensitive files.

**Solution:**
```toml
[selectors]
# Be specific about what to include
include = [
    "src/**/*.rs",
    "Cargo.toml",
    "README.md"
]

# Exclude sensitive directories
exclude = [
    ".env",
    ".env.*",
    "secrets/**",
    "credentials/**"
]
```

### 4. Validate Configuration

**Problem:** Invalid configuration may bypass security controls.

**Solution:**
```bash
# Check effective configuration
xchecker status my-feature

# Verify security settings
xchecker doctor --json | jq '.checks[] | select(.name == "secret_redaction")'
```

### 5. Monitor Receipts

**Problem:** Receipts may reveal security issues.

**Solution:**
- Review receipts for unexpected warnings
- Check for secret detection errors (exit code 8)
- Monitor for path validation errors
- Audit fixup applications

### 6. Use Strict Lock Mode

**Problem:** Drift in model or CLI version may affect security.

**Solution:**
```bash
# Create lockfile
xchecker init my-feature --create-lock

# Enforce strict lock
xchecker spec my-feature --strict-lock
```

### 7. Limit File Access

**Problem:** Broad file access increases attack surface.

**Solution:**
```bash
# Use explicit source paths
xchecker spec my-feature --source fs --repo /path/to/safe/directory

# Avoid --allow-links unless necessary
# Only use with trusted fixup sources
```

## Security Incident Response

### If Secrets Are Detected

1. **Verify Detection**: Check receipt for pattern name
2. **Locate Secret**: Search codebase for matching pattern
3. **Remove Secret**: Move to environment variable or secret manager
4. **Rotate Secret**: Assume compromised, rotate immediately
5. **Update Patterns**: Add custom pattern if needed

### If Secrets Are Committed

1. **Rotate Immediately**: Assume secret is compromised
2. **Remove from History**: Use `git filter-branch` or BFG Repo-Cleaner
3. **Force Push**: Update remote repository
4. **Notify Team**: Inform team members to re-clone
5. **Audit Access**: Check for unauthorized access using compromised secret

### If Path Validation Fails

1. **Review Error**: Check receipt for validation error
2. **Verify Intent**: Ensure path is intentional
3. **Check Source**: Verify fixup source is trusted
4. **Report Issue**: If unexpected, report as potential security issue

## Security Auditing

### Audit Checklist

- [ ] No secrets in version control
- [ ] `.gitignore` includes debug packets
- [ ] Configuration uses environment variables for secrets
- [ ] Include patterns are specific
- [ ] Exclude patterns cover sensitive directories
- [ ] Receipts reviewed for warnings
- [ ] Lockfile created and enforced
- [ ] `--allow-links` only used when necessary
- [ ] Fixups reviewed before applying
- [ ] Security patterns updated for project-specific secrets

### Automated Auditing

```bash
# Check for secrets in receipts
find .xchecker/specs/*/receipts/ -name "*.json" -exec jq -e '.error_kind != "secret_detected"' {} \;

# Check for path validation errors
find .xchecker/specs/*/receipts/ -name "*.json" -exec jq -e '.error_reason | contains("path") | not' {} \;

# Verify redaction is working
xchecker doctor --json | jq -e '.checks[] | select(.name == "secret_redaction") | .status == "pass"'
```

## Security Reporting

### Reporting Security Issues

If you discover a security vulnerability in xchecker:

1. **Do Not** open a public GitHub issue
2. **Email** security@xchecker.dev with details
3. **Include** steps to reproduce
4. **Provide** suggested fix if possible
5. **Wait** for response before public disclosure

### Security Updates

Security updates are released as patch versions:
- Critical: Released within 24 hours
- High: Released within 1 week
- Medium: Released within 1 month
- Low: Released in next minor version

## Security Compliance

### Standards Compliance

xchecker follows security best practices from:
- OWASP Top 10
- CWE/SANS Top 25
- NIST Cybersecurity Framework
- Rust Security Guidelines

### Security Features Summary

| Feature | Status | Requirement |
|---------|--------|-------------|
| Secret Detection | ✅ Implemented | FR-SEC-001 |
| Secret Redaction | ✅ Implemented | FR-SEC-005 |
| Path Validation | ✅ Implemented | FR-FIX-002 |
| Symlink Detection | ✅ Implemented | FR-FIX-003 |
| Atomic File Ops | ✅ Implemented | NFR2 |
| Audit Trail | ✅ Implemented | FR-JCS |
| Sandboxing | ✅ Implemented | FR-SEC-004 |
| Log Redaction | ✅ Implemented | FR-OBS-002 |

## References

- [FR-SEC: Secret Detection Requirements](../requirements.md#requirement-4-fr-sec)
- [FR-FIX: Path Validation Requirements](../requirements.md#requirement-5-fr-fix)
- [NFR2: Security Requirements](../requirements.md#nfr2-security)
- [SecretRedactor Implementation](../src/redaction.rs)
- [FixupEngine Implementation](../src/fixup.rs)

## See Also

- [CONFIGURATION.md](CONFIGURATION.md) - Secret pattern configuration options
- [PERFORMANCE.md](PERFORMANCE.md) - Packet size limits that affect security scanning
- [INDEX.md](INDEX.md) - Documentation index

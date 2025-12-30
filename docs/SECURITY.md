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

<!-- BEGIN GENERATED:DEFAULT_SECRET_PATTERNS -->
xchecker includes **39 default secret patterns** across 7 categories.

#### AWS Credentials (5 patterns)

| Pattern ID | Regex | Description |
|------------|-------|-------------|
| `aws_access_key` | `AKIA[0-9A-Z]{16}` | AWS access key IDs |
| `aws_secret_key` | `AWS_SECRET_ACCESS_KEY[=:][A-Za-z0-9/+=]{40}` | Secret access key assignments |
| `aws_secret_key_value` | `(?i)(?:aws_secret\|secret_access_key)[=:][A-Za-z0-9/+=]{40}` | Standalone secret key values |
| `aws_session_token` | `(?i)AWS_SESSION_TOKEN[=:][A-Za-z0-9/+=]{100,}` | Session token assignments |
| `aws_session_token_value` | `(?i)(?:session_token\|security_token)[=:][A-Za-z0-9/+=]{100,}` | Session token values |

#### Azure Credentials (4 patterns)

| Pattern ID | Regex | Description |
|------------|-------|-------------|
| `azure_client_secret` | `(?i)(?:AZURE_CLIENT_SECRET\|client_secret)[=:][A-Za-z0-9~._-]{34,}` | Client secrets |
| `azure_connection_string` | `DefaultEndpointsProtocol=https?;AccountName=[^;]+;AccountKey=[A-Za-z0-9/+=]{86,90}` | Full connection strings |
| `azure_sas_token` | `[?&]sig=[A-Za-z0-9%/+=]{40,}` | Shared Access Signature tokens |
| `azure_storage_key` | `(?i)(?:AccountKey\|storage_key)[=:][A-Za-z0-9/+=]{86,90}` | Storage account keys |

#### Database Connection URLs (5 patterns)

| Pattern ID | Regex | Description |
|------------|-------|-------------|
| `mongodb_url` | `mongodb(\+srv)?://[^:]+:[^@]+@[^\s]+` | MongoDB URLs with credentials |
| `mysql_url` | `mysql://[^:]+:[^@]+@[^\s]+` | MySQL URLs with credentials |
| `postgres_url` | `postgres(?:ql)?://[^:]+:[^@]+@[^\s]+` | PostgreSQL URLs with credentials |
| `redis_url` | `rediss?://[^:]*:[^@]+@[^\s]+` | Redis URLs with credentials |
| `sqlserver_url` | `(?:sqlserver\|mssql)://[^:]+:[^@]+@[^\s]+` | SQL Server URLs with credentials |

#### GCP Credentials (3 patterns)

| Pattern ID | Regex | Description |
|------------|-------|-------------|
| `gcp_api_key` | `AIza[0-9A-Za-z_-]{35}` | Google API keys |
| `gcp_oauth_client_secret` | `(?i)client_secret[=:][A-Za-z0-9_-]{24,}` | OAuth client secrets |
| `gcp_service_account_key` | `-----BEGIN (RSA )?PRIVATE KEY-----` | Service account private key markers |

#### Generic API Tokens (5 patterns)

| Pattern ID | Regex | Description |
|------------|-------|-------------|
| `api_key_header` | `(?i)(?:x-api-key\|api-key\|apikey)[=:][A-Za-z0-9_-]{20,}` | API key headers |
| `authorization_basic` | `Basic [A-Za-z0-9+/=]{20,}` | Basic auth credentials |
| `bearer_token` | `Bearer [A-Za-z0-9._-]{20,}` | Bearer authentication tokens |
| `jwt_token` | `eyJ[A-Za-z0-9_-]*\.eyJ[A-Za-z0-9_-]*\.[A-Za-z0-9_-]*` | JSON Web Tokens |
| `oauth_token` | `(?i)(?:access_token\|refresh_token)[=:][A-Za-z0-9._-]{20,}` | OAuth tokens |

#### Platform-Specific Tokens (12 patterns)

| Pattern ID | Regex | Description |
|------------|-------|-------------|
| `docker_auth` | `"auth":\s*"[A-Za-z0-9+/=]{20,}"` | Docker registry auth tokens |
| `github_app_token` | `gh[us]_[A-Za-z0-9]{36}` | GitHub App tokens |
| `github_oauth` | `gho_[A-Za-z0-9]{36}` | GitHub OAuth tokens |
| `github_pat` | `ghp_[A-Za-z0-9]{36}` | GitHub personal access tokens |
| `gitlab_token` | `glpat-[A-Za-z0-9_-]{20,}` | GitLab personal/project tokens |
| `npm_token` | `npm_[A-Za-z0-9]{36}` | NPM authentication tokens |
| `nuget_key` | `(?i)nuget_?(?:api_?)?key[=:][A-Za-z0-9]{46}` | NuGet API keys |
| `pypi_token` | `pypi-[A-Za-z0-9_-]{50,}` | PyPI API tokens |
| `sendgrid_key` | `SG\.[A-Za-z0-9_-]{22}\.[A-Za-z0-9_-]{43}` | SendGrid API keys |
| `slack_token` | `xox[baprs]-[A-Za-z0-9-]+` | Slack bot/user tokens |
| `stripe_key` | `sk_(?:live\|test)_[A-Za-z0-9]{24,}` | Stripe API keys |
| `twilio_key` | `SK[A-Za-z0-9]{32}` | Twilio API keys |

#### SSH and PEM Private Keys (5 patterns)

| Pattern ID | Regex | Description |
|------------|-------|-------------|
| `ec_private_key` | `-----BEGIN EC PRIVATE KEY-----` | EC private key markers |
| `openssh_private_key` | `-----BEGIN OPENSSH PRIVATE KEY-----` | OpenSSH format markers |
| `pem_private_key` | `-----BEGIN PRIVATE KEY-----` | Generic PEM private key markers |
| `rsa_private_key` | `-----BEGIN RSA PRIVATE KEY-----` | RSA private key markers |
| `ssh_private_key` | `-----BEGIN (?:OPENSSH \|DSA \|EC \|RSA )?PRIVATE KEY-----` | SSH private key markers |
<!-- END GENERATED:DEFAULT_SECRET_PATTERNS -->

### Redaction Examples

The following table demonstrates how xchecker detects and redacts various secret types. Note that the actual redaction replaces the secret with `***` or `[REDACTED:<pattern_id>]` depending on the context.

| Category | Secret Example (Simulated) | Redacted Output |
|----------|----------------------------|-----------------|
| AWS Credentials | `AKIAIOSFODNN7EXAMPLE` | `***` |
| Generic API Tokens | `Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...` | `Bearer ***` |
| Database URLs | `postgres://user:password@localhost:5432/db` | `postgres://user:***@localhost:5432/db` |
| GitHub Tokens | `ghp_1234567890abcdef1234567890abcdef1234` | `***` |
| Private Keys | `-----BEGIN RSA PRIVATE KEY-----` | `***` |

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

xchecker validates file paths to prevent path traversal and restrict access to the workspace:

1. **Canonicalization**: All paths are canonicalized to absolute paths
2. **Root Boundary**: Paths must be under the allowed root directory
3. **No Traversal**: Paths with `..` components are rejected
4. **No Absolute Escapes**: Absolute paths outside root are rejected
5. **Symlink Detection**: Symlinks are rejected by default
6. **Hardlink Detection**: Hardlinks are rejected by default

### Path Validation Rules

The `SandboxRoot` struct in `src/paths.rs` enforces strict path validation rules to ensure all file operations remain within the workspace boundary.

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

### Implementation Details

- **Canonicalization**: `SandboxRoot::new()` canonicalizes the root path, resolving all symlinks.
- **Join Validation**: `SandboxRoot::join()` validates every path component.
- **Symlink Checks**: If `allow_symlinks` is false (default), every component of the path is checked to ensure it's not a symlink.
- **Hardlink Checks**: If `allow_hardlinks` is false (default), file link counts are checked (Unix only).
- **Error Types**: Specific errors like `ParentTraversal`, `AbsolutePath`, and `EscapeAttempt` are returned for different violation types.

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

### Sandboxing and Atomic Writes

xchecker restricts file operations to the project tree by default:

**Allowed:**
- Read files under project root
- Write artifacts to `.xchecker/specs/<spec-id>/`
- Write receipts to `.xchecker/specs/<spec-id>/receipts/`
- Write context to `.xchecker/specs/<spec-id>/context/`, implemented in `src/atomic_write.rs`:

**Atomic Write Guarantees:**
1. **Write to Temp**: Write to a `NamedTempFile` in the same directory (`.tmp` extension).
2. **Fsync**: Call `sync_all()` to flush data to physical disk.
3. **Atomic Rename**: Use `persist()` to atomically rename the temp file to the target.
4. **Fallback**: If cross-filesystem error (EXDEV) occurs, fallback to copy+fsync+replace.

**Security Benefits:**
- Prevents partial writes on crash
- Mitigates race conditions
- Prevents corruption from concurrent access

### Windows-Specific Security

On Windows, xchecker implements additional security measures:

1. **Job Objects**: Process tree termination on timeout using `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`.
2. **Retry Logic**: `atomic_rename` implements exponential backoff (up to 5 retries, max 250ms) to handle transient locks from antivirus or indexers.
3. **Attribute Preservation**: Maintain file attributes on replace.

## Process Execution Security

### Runner Security Model

xchecker executes external commands (e.g., `cargo`, `npm`) using a secure runner architecture designed to prevent command injection and shell exploits:

1. **Pure Argv Execution**: Commands are constructed as a list of arguments (`Vec<OsString>`), never as a single shell string.
2. **No Shell Invocation**: `sh -c`, `cmd /C`, or `PowerShell` are never implicitly invoked.
3. **Argument Isolation**: Each argument is passed directly to the operating system's process spawner, ensuring that shell metacharacters (`;`, `|`, `&&`, `$()`) are treated as literal string data.
4. **WSL Safety**: When running in WSL mode, commands are wrapped in `wsl.exe --exec <prog> <args...>`, which bypasses the default shell behavior of `wsl.exe <command>`.

### Command Injection Prevention

The `CommandSpec` type enforces the separation of program and arguments:

```rust
// Secure by design
let cmd = CommandSpec::new("cargo")
    .arg("build")
    .arg("--message-format=json");

// Prevents injection
// This will look for a subcommand named "build; rm -rf /" and fail safely
let cmd = CommandSpec::new("cargo")
    .arg("build; rm -rf /");
```

### WSL Execution Safety

On Windows, xchecker can execute commands inside WSL distributions. This is hardened against injection:

1. **--exec Flag**: Uses `wsl.exe --exec` instead of the default shell mode.
2. **Null Byte Rejection**: Arguments containing null bytes are rejected before execution to prevent C-string truncation attacks.
3. **Argument Validation**: All arguments are validated for safety before being passed to the WSL bridge.

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

## Known Limitations (FR-DOC-6)

While xchecker implements rigorous security controls, users should be aware of the following limitations:

### Secret Detection Limitations

1. **Line-Based Scanning**: The secret scanner operates on a line-by-line basis. Secrets split across multiple lines (e.g., in a multi-line string literal) may not be detected by patterns that assume single-line content.
2. **Encoding Support**: Scanning is performed on UTF-8 text only. Secrets in binary files, UTF-16 encoded files, or other non-UTF-8 encodings are not detected.
3. **Obfuscation**: Secrets that are Base64 encoded (unless matching a specific token format like JWT), encrypted, or otherwise obfuscated will not be detected.
4. **Custom Secrets**: Proprietary or custom secret formats are not detected unless explicitly added via configuration.

### False Positives and Negatives

- **False Positives**: High-entropy strings (e.g., Git commit hashes, random IDs) may occasionally trigger false positives, particularly with generic patterns like `bearer_token`. Use `ignore_secret_patterns` with caution to suppress these.
- **False Negatives**: Patterns are designed to be conservative to avoid noise. Non-standard variations of keys (e.g., an AWS key that doesn't start with `AKIA`) will be missed.

### Path Validation Edge Cases

- **Race Conditions**: While xchecker uses canonicalization to resolve paths, there is a theoretical Time-of-Check Time-of-Use (TOCTOU) window between validation and file operations. This is mitigated by the atomic write strategy but cannot be eliminated entirely at the OS level.
- **Mount Points**: On Linux/WSL, mount points can behave like directory junctions. xchecker treats them as directories but they may cross filesystem boundaries.

### Symlink Handling

By default, xchecker rejects all symlinks to ensure strict containment. When `--allow-links` is enabled:
- **Target Validation**: xchecker attempts to validate the target of the symlink, but complex chains or circular links may lead to unexpected behavior.
- **Container Escape**: In containerized environments (like Docker), symlinks could potentially reference files outside the intended volume if not carefully managed.

### Runner Limitations

- **Signal Handling**: On Windows, process termination uses `TerminateProcess` (force kill) when timeouts occur, which does not allow the child process to clean up. On Unix, `SIGTERM` is sent first, followed by `SIGKILL`.
- **WSL Dependency**: WSL execution relies on the host's `wsl.exe` configuration. Misconfigured WSL instances may lead to execution failures.

### Windows Hardlink Detection

On Windows, hardlink detection is currently skipped due to the complexity of Win32 API integration. While symlinks are correctly detected and rejected (unless allowed), hardlinks are not explicitly blocked on Windows. This is a known limitation for the v1 release.

### Fixup Engine Limitations

The fuzzy matching algorithm used for applying fixups has known limitations in complex scenarios:
- **Ambiguous Context**: If context lines appear multiple times in the file, the wrong location might be selected.
- **Complex Diffs**: Large cumulative offsets or interleaved additions/deletions may cause patch application to fail.
- **Context Contiguity**: Replacements that break context contiguity may not be matched correctly.

These limitations result in `FuzzyMatchFailed` errors rather than incorrect code application, failing safe.

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

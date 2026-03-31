# Security Model

> This document covers the implementation details of xchecker's security system.
> For a user-facing overview of how to keep your secrets safe, see
> [Security Guide](../guides/SECURITY.md).

## Defense-in-depth overview

xchecker implements a layered security model. No single layer is relied upon
exclusively; if one control fails or is bypassed, the next layer provides
protection.

| Layer | Mechanism | What it prevents |
|-------|-----------|-----------------|
| 1. Secret detection | Regex scanning of all packet content before LLM invocation | Accidental secret exfiltration |
| 2. Secret redaction | Pattern-based replacement in all persisted and logged strings | Secret leakage into receipts and logs |
| 3. Path validation | `SandboxRoot` canonicalization and boundary enforcement | Path traversal, unauthorized file access |
| 4. File system security | Atomic writes via `.partial/` staging, symlink/hardlink rejection | Partial writes, race conditions, link-based escapes |
| 5. Process execution security | Pure argv execution, Job Objects (Windows), process groups (Unix) | Command injection, orphaned processes |
| 6. Receipt security | BLAKE3 hashing, JCS canonicalization | Tamper detection, audit integrity |
| 7. Logging security | Redaction applied to all log output and structured fields | Secret leakage in verbose output |

## Secret detection patterns

### Overview

The `SecretRedactor` component scans all packet content before any LLM
invocation. Detection is a **hard stop**: if any pattern matches, xchecker
exits with code 8 and does not proceed.

### Redaction behavior

When secrets are found in content that will be persisted (receipts, logs, error
messages), xchecker replaces the matched text with `***` or
`[REDACTED:<pattern_id>]` depending on context.

| Category | Secret example (simulated) | Redacted output |
|----------|----------------------------|-----------------|
| AWS Credentials | `AKIAIOSFODNN7EXAMPLE` | `***` |
| Generic API Tokens | `Bearer eyJhbGciOi...` | `Bearer ***` |
| Database URLs | `postgres://user:password@localhost:5432/db` | `postgres://user:***@localhost:5432/db` |
| GitHub Tokens | `ghp_1234567890abcdef1234567890abcdef1234` | `***` |
| Private Keys | `-----BEGIN RSA PRIVATE KEY-----` | `***` |

### Detection flow

```
                    Packet Assembly
                         |
                         v
                   Secret Scanning  <--- Default + Extra Patterns
                         |               - Ignore Patterns
                         |
            +------------+------------+
            |                         |
      Secrets Found             No Secrets
            |                         |
            v                         v
      Exit Code 8              Continue to LLM
      Write Error Receipt      Write Debug Packet (if --debug-packet)
      Report Pattern Name
      (not the secret)
```

### Default pattern table

<!-- BEGIN GENERATED:DEFAULT_SECRET_PATTERNS -->
xchecker includes **45 default secret patterns** across 8 categories.

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

#### LLM Provider Tokens (4 patterns)

| Pattern ID | Regex | Description |
|------------|-------|-------------|
| `anthropic_api_key` | `sk-ant-api03-[A-Za-z0-9_-]{20,}` | Anthropic API keys |
| `huggingface_token` | `hf_[A-Za-z0-9]{34}` | Hugging Face access tokens |
| `openai_api_key` | `sk-(?:proj\|org)-[A-Za-z0-9_-]{20,}` | OpenAI Project/Org API keys |
| `openai_legacy_key` | `sk-[A-Za-z0-9]{48}` | OpenAI Legacy API keys |

#### Platform-Specific Tokens (13 patterns)

| Pattern ID | Regex | Description |
|------------|-------|-------------|
| `docker_auth` | `"auth":\s*"[A-Za-z0-9+/=]{20,}"` | Docker registry auth tokens |
| `github_app_token` | `gh[us]_[A-Za-z0-9]{36}` | GitHub App tokens |
| `github_oauth` | `gho_[A-Za-z0-9]{36}` | GitHub OAuth tokens |
| `github_pat` | `ghp_[A-Za-z0-9]{36}` | GitHub personal access tokens |
| `gitlab_token` | `glpat-[A-Za-z0-9_-]{20,}` | GitLab personal/project tokens |
| `hashicorp_vault_token` | `hv[bs]\.[a-zA-Z0-9_-]{20,}` | HashiCorp Vault tokens |
| `npm_token` | `npm_[A-Za-z0-9]{36}` | NPM authentication tokens |
| `nuget_key` | `(?i)nuget_?(?:api_?)?key[=:][A-Za-z0-9]{46}` | NuGet API keys |
| `pypi_token` | `pypi-[A-Za-z0-9_-]{50,}` | PyPI API tokens |
| `sendgrid_key` | `SG\.[A-Za-z0-9_-]{22}\.[A-Za-z0-9_-]{43}` | SendGrid API keys |
| `slack_token` | `xox[baprs]-[A-Za-z0-9-]+` | Slack bot/user tokens |
| `stripe_key` | `sk_(?:live\|test)_[A-Za-z0-9]{24,}` | Stripe API keys |
| `twilio_key` | `SK[A-Za-z0-9]{32}` | Twilio API keys |

#### SSH and PEM Private Keys (6 patterns)

| Pattern ID | Regex | Description |
|------------|-------|-------------|
| `age_secret_key` | `AGE-SECRET-KEY-1[a-z0-9]{58}` | Age encryption secret keys |
| `ec_private_key` | `-----BEGIN EC PRIVATE KEY-----` | EC private key markers |
| `openssh_private_key` | `-----BEGIN OPENSSH PRIVATE KEY-----` | OpenSSH format markers |
| `pem_private_key` | `-----BEGIN PRIVATE KEY-----` | Generic PEM private key markers |
| `rsa_private_key` | `-----BEGIN RSA PRIVATE KEY-----` | RSA private key markers |
| `ssh_private_key` | `-----BEGIN (?:OPENSSH \|DSA \|EC \|RSA )?PRIVATE KEY-----` | SSH private key markers |
<!-- END GENERATED:DEFAULT_SECRET_PATTERNS -->

### Global redaction

Even when secrets are not detected during the pre-invocation scan, xchecker
applies redaction to all human-readable strings before persistence or logging.

**Redacted fields:**
- `stderr_redacted` in receipts
- `error_reason` in error receipts
- `warnings` array in receipts
- Context lines in error messages
- Doctor and status output text
- Preview text in fixup mode
- Log messages (when `--verbose`)

**Never included in any output:**
- Environment variables
- Raw packet content (except with `--debug-packet` after a successful scan)
- API keys or credentials
- Full file paths containing secrets

### Debug packet writing

The `--debug-packet` flag writes the full packet to
`context/<phase>-packet.txt` for debugging.

**Security guarantees:**

1. The packet is only written **after** the secret scan passes.
2. If any secret is detected, the packet is **never** written.
3. The packet file is excluded from receipts.
4. Packet content is redacted if later reported in errors.
5. The packet file should be added to `.gitignore`.

```bash
xchecker spec my-feature --debug-packet
# Output: .xchecker/specs/my-feature/context/requirements-packet.txt
```

**Warning:** Debug packets may contain sensitive information. Never commit them
to version control.

## Path validation

### SandboxRoot

The `SandboxRoot` struct (in `crates/xchecker-utils/src/paths.rs`) enforces
strict path validation to ensure all file operations remain within the workspace
boundary.

**Validation rules:**

1. **Canonicalization** -- All paths are resolved to absolute paths with symlinks
   resolved at the root level.
2. **Root boundary** -- Every resolved path must be a descendant of the sandbox
   root directory.
3. **No traversal** -- Path components containing `..` that would escape the root
   are rejected.
4. **No absolute escapes** -- Absolute paths outside the root are rejected.
5. **Symlink detection** -- Symlinks are rejected by default.
6. **Hardlink detection** -- Hardlinks are rejected by default.

**Examples:**

```
Valid paths (under project root):
  src/main.rs
  docs/README.md

Invalid paths (rejected):
  /etc/passwd                     -- absolute path outside root
  ../../etc/passwd                -- traversal outside root
  /tmp/symlink -> /etc/shadow     -- symlink (unless --allow-links)
  /tmp/hardlink (nlinks > 1)      -- hardlink (unless --allow-links)
```

### Implementation details

- **`SandboxRoot::new()`** canonicalizes the root path, resolving all symlinks.
- **`SandboxRoot::join()`** validates every path component before joining.
- **Symlink checks:** When `allow_symlinks` is false (default), every component
  of the path is checked to ensure it is not a symlink.
- **Hardlink checks:** When `allow_hardlinks` is false (default), file link
  counts are checked (Unix: `nlink()`, Windows:
  `GetFileInformationByHandle`).
- **Error types:** Specific errors (`ParentTraversal`, `AbsolutePath`,
  `EscapeAttempt`) are returned for each violation type.

### Fixup path validation

When applying fixups, xchecker validates all target paths before any write:

```
      Parse Fixup Plan
            |
            v
      Validate Paths  <--- Canonicalize
            |               Check Root Boundary
            |               Detect Symlinks/Hardlinks
            |
   +--------+---------+
   |                   |
Invalid Path       Valid Paths
   |                   |
   v                   v
Exit with Error   Preview or Apply
Show Validation
Error
```

### Allowing symlinks and hardlinks

By default, xchecker rejects all symlinks and hardlinks to prevent unauthorized
file access, path traversal attacks, and confusion about file identity.

If you trust the fixup source and understand the implications:

```bash
xchecker resume my-feature --phase fixup --apply-fixups --allow-links
```

**Warning:** Only use `--allow-links` if you trust the fixup source and
understand the security implications.

## File system security

### Atomic writes

xchecker restricts file operations to the project tree. All artifact, receipt,
and context writes use an atomic write strategy implemented in
`crates/xchecker-utils/src/atomic_write.rs`:

1. **Write to temp** -- Content is written to a `NamedTempFile` in the same
   directory (`.tmp` extension).
2. **Fsync** -- `sync_all()` flushes data to physical disk.
3. **Atomic rename** -- `persist()` atomically renames the temp file to the
   target path.
4. **Fallback** -- If a cross-filesystem error (`EXDEV`) occurs, xchecker falls
   back to copy + fsync + replace.

**Security benefits:**
- Prevents partial writes on crash.
- Mitigates race conditions.
- Prevents corruption from concurrent access.

### Staging directories

Artifacts are first written to a `.partial/` subdirectory and only promoted to
the final location after all validations pass. This provides an additional layer
of atomicity at the directory level.

### Windows-specific hardening

On Windows, xchecker implements additional measures:

1. **Job Objects** -- Process tree termination on timeout using
   `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`.
2. **Retry logic** -- `atomic_rename` implements exponential backoff (up to 5
   retries, max 250ms) to handle transient locks from antivirus or indexers.
3. **Attribute preservation** -- File attributes are maintained on replace.

### Symlink and hardlink detection

By default, xchecker rejects symlinks and hardlinks at the file system level to
prevent:

- Unauthorized file access via symlink chains.
- Path traversal attacks using directory symlinks.
- Confusion about file identity (hardlinks sharing inodes).

**Windows hardlink detection** uses `GetFileInformationByHandle` (Win32 API)
to read `nNumberOfLinks` for accurate link count detection.

## Process execution security

### Runner architecture

xchecker executes external commands using a secure runner architecture designed
to prevent command injection and shell exploits.

**Core principles:**

1. **Pure argv execution** -- Commands are constructed as `Vec<OsString>`,
   never as a single shell string.
2. **No shell invocation** -- `sh -c`, `cmd /C`, and `PowerShell` are never
   implicitly invoked.
3. **Argument isolation** -- Each argument is passed directly to the OS process
   spawner. Shell metacharacters (`;`, `|`, `&&`, `$()`) are treated as literal
   string data.
4. **WSL safety** -- When running in WSL mode, commands are wrapped as
   `wsl.exe --exec <prog> <args...>`, bypassing the default shell behavior of
   `wsl.exe <command>`.

### CommandSpec

The `CommandSpec` type enforces the separation of program and arguments:

```rust
// Secure by design
let cmd = CommandSpec::new("cargo")
    .arg("build")
    .arg("--message-format=json");

// Injection attempt is harmless -- this looks for a subcommand
// named "build; rm -rf /" and fails safely
let cmd = CommandSpec::new("cargo")
    .arg("build; rm -rf /");
```

### WSL execution safety

On Windows, xchecker can execute commands inside WSL distributions. This path
is hardened:

1. **`--exec` flag** -- Uses `wsl.exe --exec` instead of default shell mode.
2. **Null byte rejection** -- Arguments containing null bytes are rejected before
   execution to prevent C-string truncation attacks.
3. **Argument validation** -- All arguments are validated for safety before
   being passed to the WSL bridge.

### Job Objects and process groups

- **Windows:** When a phase timeout fires, the entire process tree is terminated
  via a Job Object (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`). This prevents
  orphaned child processes.
- **Unix:** `SIGTERM` is sent to the process group first, followed by `SIGKILL`
  after a grace period. The process group is created via `setsid()` or
  `setpgid()` to ensure all descendants are captured.

### Signal handling

| Platform | Normal exit | Timeout |
|----------|------------|---------|
| Unix | Process exits naturally | `SIGTERM` to group, then `SIGKILL` |
| Windows | Process exits naturally | `TerminateProcess` via Job Object (force kill, no cleanup) |

## Receipt security

### Content policy

Receipts are designed to be safe for version control and sharing.

**Never included:**
- Environment variables
- Raw packet content
- API keys or credentials
- Secrets (redacted before persistence)
- Full file paths containing secrets

**Always included:**
- Exit codes and error kinds
- Redacted error messages
- File hashes (BLAKE3)
- Timestamps (UTC)
- Configuration sources (`cli` / `env` / `config` / `programmatic` / `default`)

### Hashing

All artifact hashes use BLAKE3, a cryptographic hash function. Hashes are
recorded in the receipt `outputs` array as 64 hex characters
(`blake3_canonicalized`). This provides:

- **Tamper evidence** -- Any modification to an artifact after receipt generation
  is detectable by recomputing the hash.
- **Deterministic verification** -- The same content always produces the same
  hash.

### JCS canonicalization

Receipts are emitted using JSON Canonicalization Scheme (RFC 8785). This ensures:

- Deterministic key ordering.
- Consistent numeric formatting.
- No ambiguity in whitespace or encoding.
- Hash stability across serialization/deserialization cycles.

### Receipt redaction

Before writing, xchecker applies redaction to:

- `stderr_redacted` field (capped at 2048 bytes after redaction)
- `error_reason` field
- `warnings` array
- Any human-readable strings

```json
{
  "stderr_redacted": "Error: Failed to connect to API\nToken: ***\nRetrying...",
  "error_reason": "Authentication failed with token ***",
  "warnings": ["Deprecated flag --old-flag, use --new-flag instead"]
}
```

## Logging security

### Verbose logging

When `--verbose` is enabled, xchecker logs detailed operational information.

**Logged:**
- File selection and sizes
- Packet assembly details
- Cache hit/miss statistics
- Phase execution timings
- Configuration sources

**Never logged:**
- Secrets (redacted before logging)
- Environment variables
- Raw packet content
- API keys or credentials

### Log redaction

All log messages pass through redaction before emission:

```rust
// Before logging
let message = format!("Token: {}", token);

// After redaction
let redacted = redactor.redact(&message);
// "Token: ***"

logger.info(&redacted);
```

## Known limitations

### Secret detection

1. **Line-based scanning** -- The scanner operates line-by-line. Secrets split
   across multiple lines (e.g., in a multi-line string literal) may not be
   detected by patterns that assume single-line content.
2. **Encoding support** -- Scanning is performed on UTF-8 text only. Secrets in
   binary files, UTF-16 encoded files, or other non-UTF-8 encodings are not
   detected.
3. **Obfuscation** -- Secrets that are Base64 encoded (unless matching a
   specific token format like JWT), encrypted, or otherwise obfuscated will not
   be detected.
4. **Custom secrets** -- Proprietary or custom secret formats are not detected
   unless explicitly added via configuration.

### False positives and negatives

- **False positives:** High-entropy strings (e.g., Git commit hashes, random
  IDs) may occasionally trigger matches, particularly with generic patterns like
  `bearer_token`. Use `ignore_secret_patterns` with caution.
- **False negatives:** Patterns are designed to be conservative to avoid noise.
  Non-standard key variations (e.g., an AWS key that does not start with
  `AKIA`) will be missed.

### Path validation edge cases

- **Race conditions (TOCTOU):** There is a theoretical time-of-check
  time-of-use window between validation and file operations. This is mitigated
  by the atomic write strategy but cannot be eliminated entirely at the OS
  level.
- **Mount points:** On Linux/WSL, mount points can behave like directory
  junctions. xchecker treats them as directories but they may cross filesystem
  boundaries.

### Symlink handling with `--allow-links`

When `--allow-links` is enabled:

- **Target validation:** xchecker attempts to validate the target of the
  symlink, but complex chains or circular links may lead to unexpected behavior.
- **Container escape:** In containerized environments (Docker), symlinks could
  potentially reference files outside the intended volume if not carefully
  managed.

### Runner limitations

- **Signal handling on Windows:** `TerminateProcess` (force kill) does not allow
  the child process to clean up. On Unix, `SIGTERM` is sent first, followed by
  `SIGKILL`.
- **WSL dependency:** WSL execution relies on the host's `wsl.exe`
  configuration. Misconfigured WSL instances may lead to execution failures.

### Windows hardlink detection

Implemented via `GetFileInformationByHandle` (Win32 API), which returns
`nNumberOfLinks` for accurate link count detection. The implementation is in
`crates/xchecker-utils/src/paths.rs` as the `link_count()` function, shared
with Unix.

### Fixup engine

The fuzzy matching algorithm used for applying fixups has known limitations:

- **Ambiguous context:** If context lines appear multiple times in the file, the
  wrong location might be selected.
- **Complex diffs:** Large cumulative offsets or interleaved
  additions/deletions may cause patch application to fail.
- **Context contiguity:** Replacements that break context contiguity may not be
  matched correctly.

These limitations result in `FuzzyMatchFailed` errors rather than incorrect
code application -- the system fails safe.

## Compliance mapping

### OWASP Top 10

| OWASP Category | xchecker Control |
|----------------|-----------------|
| A01:2021 Broken Access Control | Path validation (SandboxRoot), symlink/hardlink rejection |
| A02:2021 Cryptographic Failures | BLAKE3 hashing for artifact integrity |
| A03:2021 Injection | Pure argv execution, no shell invocation, argument isolation |
| A05:2021 Security Misconfiguration | Configuration validation, doctor health checks |
| A07:2021 Identification and Authentication Failures | Secret detection blocks credential leakage |
| A09:2021 Security Logging and Monitoring Failures | Comprehensive receipt audit trail with redaction |

### CWE References

| CWE | Description | xchecker Mitigation |
|-----|-------------|---------------------|
| CWE-22 | Path Traversal | `SandboxRoot` canonicalization and boundary enforcement |
| CWE-59 | Improper Link Resolution | Symlink and hardlink detection and rejection by default |
| CWE-78 | OS Command Injection | Pure argv execution via `CommandSpec`, no shell invocation |
| CWE-200 | Exposure of Sensitive Information | Secret scanning, redaction in all outputs |
| CWE-311 | Missing Encryption of Sensitive Data | Secrets blocked from reaching external services |
| CWE-362 | Race Condition (TOCTOU) | Atomic writes via `.partial/` staging and `persist()` |
| CWE-532 | Insertion of Sensitive Info into Log File | Redaction applied to all log messages |
| CWE-798 | Use of Hard-Coded Credentials | Secret detection across 45+ patterns, 8 categories |

### Security features summary

| Feature | Status | Requirement |
|---------|--------|-------------|
| Secret Detection | Implemented | FR-SEC-001 |
| Secret Redaction | Implemented | FR-SEC-005 |
| Path Validation | Implemented | FR-FIX-002 |
| Symlink Detection | Implemented | FR-FIX-003 |
| Atomic File Ops | Implemented | NFR2 |
| Audit Trail | Implemented | FR-JCS |
| Sandboxing | Implemented | FR-SEC-004 |
| Log Redaction | Implemented | FR-OBS-002 |

## References

- [FR-SEC: Secret Detection Requirements](../requirements.md#requirement-4-fr-sec)
- [FR-FIX: Path Validation Requirements](../requirements.md#requirement-5-fr-fix)
- [NFR2: Security Requirements](../requirements.md#nfr2-security)
- [SecretRedactor Implementation](../../crates/xchecker-utils/src/redaction.rs)
- [SandboxRoot Implementation](../../crates/xchecker-utils/src/paths.rs)

## See also

- [Security Guide](../guides/SECURITY.md) -- User-facing security guide
- [CONFIGURATION.md](../CONFIGURATION.md) -- Secret pattern configuration options
- [PERFORMANCE.md](../PERFORMANCE.md) -- Packet size limits that affect security scanning

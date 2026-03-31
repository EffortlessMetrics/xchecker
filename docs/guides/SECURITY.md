# Security Guide

> xchecker scans all content before it reaches any LLM. If a secret is detected,
> execution stops with exit code 8 and nothing is sent.
>
> For the full implementation details, pattern tables, and compliance mapping,
> see [Security Model](../explanation/SECURITY_MODEL.md).

## How xchecker protects your secrets

Every time xchecker prepares a packet (the bundle of context sent to an LLM), it
runs a secret scanner over the entire payload. The scanner checks against 45+
built-in patterns and any custom patterns you configure. If any pattern matches,
xchecker immediately:

1. Halts execution (exit code 8).
2. Writes an error receipt recording which pattern matched (never the secret itself).
3. Skips the LLM invocation entirely -- nothing leaves your machine.

This happens before any network call, so there is no window where a secret could
be transmitted.

## What gets detected

The built-in patterns cover eight categories:

| Category | Examples | Pattern count |
|----------|----------|---------------|
| AWS Credentials | `AKIA...` access keys, secret keys, session tokens | 5 |
| Azure Credentials | Client secrets, connection strings, SAS tokens, storage keys | 4 |
| Database Connection URLs | PostgreSQL, MySQL, MongoDB, Redis, SQL Server with embedded passwords | 5 |
| GCP Credentials | API keys (`AIza...`), OAuth secrets, service account keys | 3 |
| Generic API Tokens | Bearer tokens, JWTs, Basic auth, OAuth tokens, API key headers | 5 |
| LLM Provider Tokens | Anthropic (`sk-ant-...`), OpenAI (`sk-proj-...`), Hugging Face (`hf_...`) | 4 |
| Platform-Specific Tokens | GitHub PATs/OAuth, GitLab, Slack, Stripe, npm, PyPI, and more | 13 |
| SSH and PEM Private Keys | RSA, EC, OpenSSH, generic PEM, Age encryption keys | 6 |

The full regex table with every pattern ID is in
[Security Model -- Secret Detection Patterns](../explanation/SECURITY_MODEL.md#secret-detection-patterns).

## What happens when a secret is found

When the scanner detects a match, you will see output similar to:

```
Error: Secret detected matching pattern: ghp_
```

xchecker exits with code **8** (`SECRET_DETECTED`). A receipt is written that
records the event without exposing the secret:

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

Key behaviors:

- The LLM is never called.
- The full packet is never written (even with `--debug-packet`).
- The receipt names the **pattern** that matched, not the secret value.

### Finding the offending content

1. Check the receipt's `error_reason` for the pattern name.
2. Search your project files for strings matching that pattern.
3. Move the secret to an environment variable or a secret manager and re-run.

## Custom patterns

### Adding patterns

If your project uses proprietary token formats, add extra patterns so the
scanner catches them:

```bash
# Via CLI flags
xchecker spec my-feature --extra-secret-pattern "SECRET_[A-Z0-9]{32}"
xchecker spec my-feature \
  --extra-secret-pattern "API_KEY_[A-Za-z0-9]{40}" \
  --extra-secret-pattern "TOKEN_[A-Za-z0-9]{64}"
```

```toml
# Via config file (.xchecker/config.toml)
[security]
extra_secret_patterns = [
    "SECRET_[A-Z0-9]{32}",
    "API_KEY_[A-Za-z0-9]{40}",
    "TOKEN_[A-Za-z0-9]{64}",
]
```

### Suppressing patterns

If a built-in pattern produces false positives (for example, a test fixture that
looks like a GitHub PAT but is not one), you can suppress it:

```bash
xchecker spec my-feature --ignore-secret-pattern "ghp_"
```

```toml
[security]
ignore_secret_patterns = [
    "ghp_",
    "Bearer",
]
```

**Warning:** Suppressing patterns reduces security coverage. Only suppress a
pattern when you are certain it will not match real secrets in your codebase.

## Best practices

### Keep secrets out of tracked files

- Store credentials in environment variables, not in source files.
- Use a secret manager (1Password, AWS Secrets Manager, etc.) for production keys.
- Add debug packet paths to `.gitignore`:
  ```
  .xchecker/specs/*/context/*-packet.txt
  ```

### Use specific file selectors

Broad include patterns risk pulling sensitive files into the packet:

```toml
[selectors]
include = ["src/**/*.rs", "Cargo.toml", "README.md"]
exclude = [".env", ".env.*", "secrets/**", "credentials/**"]
```

### Review fixups before applying

Always preview fixups first (the default). Only apply after inspecting the
proposed changes:

```bash
# Preview (default)
xchecker resume my-feature --phase fixup

# Apply only after review
xchecker resume my-feature --phase fixup --apply-fixups
```

### Validate your configuration

```bash
xchecker status my-feature
xchecker doctor --json | jq '.checks[] | select(.name == "secret_redaction")'
```

### Monitor receipts

Review receipts for unexpected warnings, secret detection errors (exit code 8),
and path validation failures. Receipts are your audit trail.

### Use lockfiles

Lockfiles pin the model and CLI version to prevent drift:

```bash
xchecker init my-feature --create-lock
xchecker spec my-feature --strict-lock
```

## Incident response

### A secret was detected before sending

This is the normal, safe path. xchecker stopped execution and nothing was sent.

1. Check the receipt to identify the pattern.
2. Locate and remove the secret from tracked files.
3. Move the secret to an environment variable or secret manager.
4. Re-run.

### A secret was accidentally sent to an LLM

If you believe a secret reached an LLM (for example, by suppressing patterns or
using a version without scanner coverage):

1. **Rotate the secret immediately.** Assume it is compromised.
2. **Audit receipts** for the affected spec to understand what was sent.
3. **Check access logs** for the service the secret protects.
4. **Remove from git history** if the secret was committed:
   ```bash
   # Use BFG Repo-Cleaner or git filter-branch
   # Then force-push and have all team members re-clone
   ```
5. **Add a custom pattern** so the scanner catches this format going forward.

### Path validation failures

1. Review the receipt for the specific validation error.
2. Verify that the file path was intentional (not an LLM hallucination).
3. If the path is unexpected, treat it as a potential security issue and report it.

## Reporting security issues

If you discover a security vulnerability in xchecker:

1. **Do not** open a public GitHub issue.
2. Email **security@xchecker.dev** with details and reproduction steps.
3. Include a suggested fix if possible.
4. Wait for a response before public disclosure.

Security updates follow this timeline:

| Severity | Release target |
|----------|---------------|
| Critical | Within 24 hours |
| High | Within 1 week |
| Medium | Within 1 month |
| Low | Next minor version |

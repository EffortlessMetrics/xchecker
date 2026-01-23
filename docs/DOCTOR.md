# Doctor Command

The `xchecker doctor` command performs environment health checks to validate that your system is properly configured for running xchecker.

## Usage

```bash
# Run health checks with human-readable output
xchecker doctor

# Output results as JSON
xchecker doctor --json

# Treat warnings as failures (useful for CI)
xchecker doctor --strict-exit
```

## Health Checks

### atomic_rename

**Purpose:** Verifies that atomic file rename operations work on the current volume.

**Pass Criteria:** Successfully creates a test file and atomically renames it within the `.xchecker` directory.

**Remediation:**
- Ensure the current directory is on a filesystem that supports atomic renames
- Check that you have write permissions to the `.xchecker` directory
- On network filesystems, atomic renames may not be supported

### blake3_hashing

**Purpose:** Validates that BLAKE3 hashing produces stable, deterministic hashes.

**Pass Criteria:** BLAKE3 hash computation succeeds and produces 64-character hex output.

**Remediation:**
- This check should always pass unless there's a critical system issue
- If failing, try reinstalling xchecker

### canonicalization_backend

**Purpose:** Verifies that JCS (RFC 8785) canonicalization is working correctly.

**Pass Criteria:** JSON canonicalization produces deterministic, byte-identical output.

**Remediation:**
- This check should always pass unless there's a critical system issue
- If failing, try reinstalling xchecker

### claude_path (Provider: claude-cli)

**Purpose:** Checks if the Claude CLI is available in your system PATH.
**Run Condition:** Only when `provider = "claude-cli"` (default).

**Pass Criteria:** The `claude` command is found in the system PATH.

**Remediation:**
- Install Claude CLI from https://claude.ai/cli
- Ensure `claude` is in your PATH
- Restart your terminal after installation
- On Windows, try WSL if native installation fails

### claude_version (Provider: claude-cli)

**Purpose:** Verifies that the Claude CLI can be executed and returns version information.
**Run Condition:** Only when `provider = "claude-cli"` (default).

**Pass Criteria:** Running `claude --version` succeeds and returns version information.

**Remediation:**
- Ensure Claude CLI is properly installed
- Check that you have execute permissions for the `claude` binary
- Verify Claude CLI authentication: `claude auth status`
- Update Claude CLI to the latest version if needed

### gemini_path (Provider: gemini-cli)

**Purpose:** Checks if the Gemini CLI is available in your system PATH.
**Run Condition:** Only when `provider = "gemini-cli"`.

**Pass Criteria:** The `gemini` command is found in the system PATH.

**Remediation:**
- Install Gemini CLI
- Ensure `gemini` is in your PATH
- Verify installation with `gemini --version`

### gemini_help (Provider: gemini-cli)

**Purpose:** Verifies that the Gemini CLI can be executed.
**Run Condition:** Only when `provider = "gemini-cli"`.

**Pass Criteria:** Running `gemini -h` succeeds.

**Remediation:**
- Ensure Gemini CLI is properly installed
- Check execute permissions

### config_parse

**Purpose:** Validates that the xchecker configuration file can be parsed successfully.

**Pass Criteria:** Configuration file (if present) is valid TOML and passes validation.

**Remediation:**
- Check `.xchecker/config.toml` for syntax errors
- Validate TOML syntax using an online validator
- Compare with example configuration in documentation
- Remove invalid configuration options

### llm_provider

**Purpose:** Validates the configured LLM provider and its dependencies.

**Pass Criteria:**
- **claude-cli**: Binary found/configured
- **gemini-cli**: Binary found/configured
- **openrouter**: API key environment variable present and model configured
- **anthropic**: API key environment variable present and model configured

**Remediation:**
- Check [llm] section in configuration file
- Verify required environment variables are set (e.g., OPENROUTER_API_KEY)
- Verify binary paths if using custom locations

### lock_manager

**Purpose:** Validates that the lock manager can create and manage advisory locks.

**Pass Criteria:** Lock file can be created, validated, and removed successfully.

**Remediation:**
- Ensure write permissions to `.xchecker/specs/` directory
- Check that filesystem supports file locking
- On network filesystems, locking may not work reliably

### packet_builder

**Purpose:** Validates that packet assembly works with priority-based selection.

**Pass Criteria:** Packet builder can assemble content within budget limits.

**Remediation:**
- This check should always pass unless there's a critical system issue
- If failing, check that test files can be created in temp directory

### runner_selection

**Purpose:** Validates that the configured runner mode (native, WSL, or auto) is available and working.

**Pass Criteria:** The selected runner mode can successfully detect and execute Claude CLI.

**Remediation:**
- For native mode: Ensure Claude CLI is in PATH
- For WSL mode: Ensure WSL is installed and Claude CLI is available in WSL
- For auto mode: Ensure at least one of native or WSL works
- Try specifying runner mode explicitly: `--runner native` or `--runner wsl`

### secret_redaction

**Purpose:** Validates that secret detection and redaction is working correctly.

**Pass Criteria:** Secret patterns are detected and redacted properly.

**Remediation:**
- This check should always pass unless there's a critical system issue
- If failing, try reinstalling xchecker

### timeout_enforcement

**Purpose:** Validates that phase timeout enforcement works correctly.

**Pass Criteria:** Timeout mechanism can detect and enforce time limits.

**Remediation:**
- This check should always pass unless there's a critical system issue
- If failing, check system clock and timer functionality

### write_permissions

**Purpose:** Verifies that xchecker can write to the `.xchecker` directory.

**Pass Criteria:** Successfully creates the `.xchecker` directory (if needed) and writes a test file.

**Remediation:**
- Check file permissions on the current directory
- Ensure you have write access to the current directory
- Try running from your home directory or a writable location
- On Windows, check that the directory is not read-only

### wsl_availability (Windows only)

**Purpose:** Checks if WSL is available and if Claude CLI is installed in WSL.

**Pass Criteria:** WSL is available and Claude CLI can be executed via `wsl -e claude --version`.

**Status Levels:**
- **Pass:** WSL is available and Claude CLI is installed
- **Warn:** WSL is available but Claude CLI is not installed
- **Warn:** WSL is not installed (not a failure, just informational)

**Remediation:**
- Install WSL: `wsl --install`
- Install Claude CLI in WSL: `wsl -e pip install claude-cli`
- Verify Claude CLI is accessible: `wsl -e claude --version`
- Restart WSL if needed: `wsl --shutdown && wsl`

### wsl_default_distro (Windows only)

**Purpose:** Identifies the default WSL distribution for informational purposes.

**Pass Criteria:** Successfully queries WSL and identifies the default distribution.

**Status Levels:**
- **Pass:** Default WSL distro identified
- **Warn:** Could not determine default distro (WSL may not be configured)

**Remediation:**
- Set a default WSL distro: `wsl --set-default <distro-name>`
- List available distros: `wsl -l -v`
- Install a WSL distro if none are available

### wsl_path_translation (Windows only)

**Purpose:** Validates that Windows paths can be translated to WSL paths correctly.

**Pass Criteria:** Path translation using `wslpath` or fallback heuristic succeeds.

**Remediation:**
- Ensure WSL is properly installed
- Verify `wslpath` command is available in WSL
- Check that drive letters are accessible in WSL (e.g., `/mnt/c/`)

## Exit Codes

- **0:** All checks passed (or only warnings in normal mode)
- **1:** One or more checks failed (or warnings in strict mode)

## JSON Output Schema

The JSON output follows the `schemas/doctor.v1.json` schema:

```json
{
  "schema_version": "1",
  "emitted_at": "2025-10-24T12:00:00Z",
  "ok": true,
  "checks": [
    {
      "name": "check_name",
      "status": "pass",
      "details": "Check details"
    }
  ]
}
```

### Fields

- `schema_version`: Always "1" for this version
- `emitted_at`: RFC3339 UTC timestamp
- `ok`: Overall health status (false if any check fails, or if any check warns in strict mode)
- `checks`: Array of health checks, sorted alphabetically by name

### Check Status Values

- `pass`: Check succeeded
- `warn`: Check found a non-critical issue
- `fail`: Check failed (critical issue)

## Strict Mode

Use `--strict-exit` to treat warnings as failures. This is useful in CI environments where you want to ensure all checks pass without any warnings.

```bash
# In CI pipeline
xchecker doctor --strict-exit --json
```

## Examples

### Basic health check

```bash
$ xchecker doctor
=== xchecker Environment Health Check ===

✓ atomic_rename [PASS]
  Atomic rename works on same volume

✓ claude_path [PASS]
  Found claude at /usr/local/bin/claude

✓ claude_version [PASS]
  0.8.1

✓ config_parse [PASS]
  Configuration parsed and validated successfully

✓ runner_selection [PASS]
  Runner mode: native (spawn claude directly)

✓ write_permissions [PASS]
  .xchecker directory is writable

Overall status: ✓ HEALTHY
```

### JSON output for automation

```bash
$ xchecker doctor --json | jq '.ok'
true
```

### CI integration

```yaml
# GitHub Actions example
- name: Check xchecker environment
  run: xchecker doctor --strict-exit --json
```

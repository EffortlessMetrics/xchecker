## 2026-01-19 - Symlink Traversal in Packet Construction

**Vulnerability:** `ContentSelector::walk_directory` followed symlinks during recursive directory walking, allowing arbitrary file read (path traversal) if a malicious symlink was present in the source tree. This could expose sensitive system files to the LLM context packet.

**Severity:** HIGH

**Root Cause:** The original implementation used `path.is_dir()` which follows symlinks (stat behavior), and `fs::read_to_string()` which also follows symlinks. This meant:
1. A symlink to `/etc/passwd` would be read and included in the packet
2. A symlink to a directory outside the workspace would be recursively traversed

**Learning:** `fs::read_dir` returns entries where `entry.file_type()` reflects the link itself (lstat semantics), but `path.is_dir()` and `path.is_file()` follow links (stat semantics). Recursive walkers must explicitly check `file_type.is_symlink()` to avoid unintended traversal. `fs::read_to_string` also follows links.

**Fix Applied:**
1. Check `file_type.is_symlink()` before processing any entry
2. Default behavior: skip all symlinks (secure-by-default)
3. Optional `allow_symlinks(true)`: validate symlink targets stay within the base directory using `fs::canonicalize()`
4. Fail-closed: broken symlinks or canonicalization errors result in skipping the entry

**Prevention:** Always check `is_symlink()` during directory traversal. Use `SandboxRoot` or equivalent canonicalization checks to ensure symlink targets stay within trust boundaries. Default to rejecting symlinks.

**Files Changed:**
- `crates/xchecker-engine/src/packet/selectors.rs` - Core fix in `walk_directory()`
- `crates/xchecker-engine/src/packet/builder.rs` - API surface for configuration

---

## 2026-01-20 - Unbounded Memory Consumption in File Selection

**Vulnerability:** `ContentSelector::walk_directory` read entire file contents into memory using `fs::read_to_string` before checking if the file fits within the packet budget. This allowed a malicious or misconfigured repository with large files (e.g., 10GB logs) to cause an Out-Of-Memory (OOM) crash or Denial of Service (DoS) by exhausting system resources.

**Severity:** MEDIUM

**Root Cause:** File reading was eager (read-all-then-check) rather than lazy or bounded. The `ContentSelector` did not enforce any size limits during traversal.

**Learning:** Always check file metadata (`fs::metadata(path).len()`) before reading content into memory. Set hard limits on file sizes based on application constraints (e.g., packet budget). Also, avoid reading special files (pipes, devices) which might block indefinitely.

**Fix Applied:**
1. Added `max_file_size` limit to `ContentSelector`.
2. Updated `PacketBuilder` to set `max_file_size` equal to the configured `packet_max_bytes`.
3. In `walk_directory`, check `metadata.len()` and `metadata.is_file()` before reading.
4. Fail hard if critical `Upstream` files exceed the limit, but skip non-critical files with a warning.

**Prevention:** Enforce resource limits early in the processing pipeline (fail-fast). Use metadata checks before I/O.

**Files Changed:**
- `crates/xchecker-engine/src/packet/selectors.rs`
- `crates/xchecker-engine/src/packet/builder.rs`

---

## 2026-01-20 - Symlink Traversal via Non-Existent Paths in SandboxRoot

**Vulnerability:** `SandboxRoot::join()` allowed symlink traversal escape when `allow_symlinks=true` and the target path did not exist. Because non-existent paths bypass `canonicalize()`, a symlinked directory in the path could redirect to outside the sandbox.

**Severity:** HIGH (when `allow_symlinks` is enabled)

**Attack Scenario:**
1. Sandbox root at `/workspace`
2. Attacker creates symlink: `/workspace/escape_dir` → `/tmp/attacker_controlled`
3. Attacker calls `root.join("escape_dir/new_malicious_file.txt")`
4. Old code: path doesn't exist → skip canonicalization → allow the path
5. Result: attacker can write to `/tmp/attacker_controlled/new_malicious_file.txt`

**Root Cause:** The original `SandboxRoot::join()` implementation assumed that for non-existent paths, rejecting `..` components was sufficient. However, symlinks in ancestor directories can redirect the path outside the sandbox without using `..`.

**Learning:** `std::fs::canonicalize()` requires the path to exist. When validating a non-existent path in a sandbox, you must canonicalize and validate the nearest existing ancestor directory, especially when symlinks are allowed.

**Fix Applied:**
1. Added `validate_ancestor_within_sandbox()` method
2. For non-existent paths when `allow_symlinks=true`, find the longest existing prefix
3. Canonicalize that ancestor and verify it stays within the sandbox root
4. Fail with `EscapeAttempt` error if ancestor escapes

**Prevention:** When validating paths that may not exist yet, always validate existing ancestor directories. Don't assume lexical checks (no `..`) are sufficient when symlinks are allowed.

**Files Changed:**
- `crates/xchecker-utils/src/paths.rs` - Added `validate_ancestor_within_sandbox()` and called it for non-existent paths

---

## 2026-01-21 - Information Exposure via Filenames in Packet

**Vulnerability:** Secrets in filenames were not being redacted when building the packet context. While file *content* was scanned and redacted, the `packet_content.push_str(&format!("=== {} ===\n", file.path));` line exposed the raw filename. This could leak sensitive information (e.g., `config_AWS_SECRET=...`) to the LLM.

**Severity:** MEDIUM

**Root Cause:** The `SecretRedactor` was only applied to file content, but filenames are also user-controlled data included in the packet.

**Learning:** All user-controlled data entering a trusted boundary (or leaving via an egress channel like an LLM API) must be sanitized. Filenames are often overlooked but can contain sensitive data.

**Fix Applied:**
1. Refactored `PacketBuilder` to ensure `SecretRedactor` is accessible for filename redaction.
2. Applied `redactor.redact_string(file.path.as_str())` before adding the filename header to the packet.

**Prevention:** Ensure that all components of a data packet (headers, metadata, content) pass through the sanitization layer.

**Files Changed:**
- `crates/xchecker-engine/src/packet/builder.rs`

---

## 2026-01-23 - Enforced Sensitive File Exclusion

**Vulnerability:** Default packet construction could include sensitive files (like `.env`, `private.pem`) if user configuration overrides defaults or is too broad (`**/*`).

**Severity:** MEDIUM

**Root Cause:** The `ContentSelector` relied on default excludes which could be entirely replaced by user configuration. There was no mandatory baseline enforcement.

**Learning:** Security controls (exclusions) should be mandatory and non-overridable for high-risk patterns. Relying on default configuration structs is insufficient when users can replace them entirely.

**Fix Applied:**
1. Added `ALWAYS_EXCLUDE_PATTERNS` constant with high-confidence secret patterns (`.env`, `*.pem`, SSH keys, etc.)
2. Enforced these patterns in ALL constructors: `new()`, `from_selectors()`, and `with_patterns()`
3. Added tests verifying mandatory exclusions override custom user includes
4. Updated `xchecker-config` defaults for defense-in-depth (belt-and-suspenders)

**Prevention:** Define mandatory security baselines separate from configurable defaults. Enforce them in all code paths that construct security-sensitive objects.

**Files Changed:**
- `crates/xchecker-engine/src/packet/selectors.rs` - Core enforcement in all constructors
- `crates/xchecker-config/src/config/selectors.rs` - Mirrored patterns in defaults

---

## 2026-01-24 - Missing LLM Provider API Keys in Secret Detection

**Vulnerability:** The default secret detection patterns missed OpenAI API keys (both legacy and new project/org formats) and Anthropic API keys. Given that `xchecker` is an LLM-orchestration tool, accidental inclusion of these keys is a high-probability risk.

**Severity:** HIGH

**Root Cause:** The `DEFAULT_SECRET_PATTERNS` list was comprehensive for cloud providers (AWS, Azure, GCP) but lacked patterns for the specific LLM providers (Anthropic, OpenAI) that the tool interacts with.

**Learning:** When building tools that integrate with specific 3rd-party services (like LLMs), always prioritize secret detection for those specific services' credentials. Generic patterns often fail to catch provider-specific formats like `sk-ant-...` or `sk-proj-...`.

**Fix Applied:**
1. Added a new category "LLM Provider Tokens" to `redaction.rs`.
2. Added detection for:
   - Anthropic keys: `sk-ant-api03-[A-Za-z0-9_-]{20,}`
   - OpenAI Project/Org keys: `sk-(?:proj|org)-[A-Za-z0-9_-]{20,}`
   - OpenAI Legacy keys: `sk-[A-Za-z0-9]{48}`
3. Verified via regression tests that patterns do not overlap incorrectly.

**Prevention:** Regularly audit secret detection patterns against the specific integrations used by the tool and its users.

**Files Changed:**
- `crates/xchecker-utils/src/redaction.rs`

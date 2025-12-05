# Runtime Implementation Design

## Overview

**Status: IMPLEMENTED - VERIFICATION & IMPROVEMENT PHASE**

This design document originally outlined the architecture for xchecker runtime components. **The implementation is now complete and operational.** This document has been updated to reflect the actual implementation and guide verification, cleanup, and improvement work.

**Implementation Status:**
- ✅ All core modules implemented and functional
- ✅ End-to-end phase execution working
- ✅ Cross-platform support verified
- ✅ Security controls operational
- 🔧 Current focus: verification, edge case handling, optimization

**Actual vs Planned Architecture:**
- `canonicalization.rs` provides JCS emission (not separate `jcs.rs`)
- `redaction.rs` contains `SecretRedactor` (not `SecretScanner`)
- Hash functions integrated into canonicalization module
- Additional modules implemented beyond original plan:
  - `cache.rs` - InsightCache for file insight caching
  - `source.rs` - SourceResolver for multi-source problem statements
  - `phase.rs` / `phases.rs` - Phase trait system for workflow phases
  - `integration_tests.rs` - Integration testing framework
  - `atomic_write.rs` - Atomic file operations
  - `ring_buffer.rs` - Bounded circular buffers
  - `process_memory.rs` - Process memory tracking
  - `example_generators.rs` - Schema example generation
  - `artifact.rs` - Artifact metadata management
  - `paths.rs` - Spec directory path management
  - `spec_id.rs` - Spec identifier validation
  - `types.rs` - Common type definitions
  - `exit_codes.rs` - Exit code constants
  - `error_reporter.rs` - User-friendly error reporting
  - `logging.rs` - Structured logging
  - `doctor.rs` - System health checks
  - `cli.rs` - Command-line interface

## Architecture

### High-Level Component Structure

```
CLI (clap)
└── Commands (spec, resume, status, clean, doctor, init, benchmark)
    ├── Orchestrator (FR-ORC)
    │   ├── Phase validation & state machine
    │   ├── Phase trait system (FR-PHASE)
    │   │   ├── RequirementsPhase
    │   │   ├── DesignPhase
    │   │   └── TasksPhase
    │   └── Coordination of all subsystems
    ├── SourceResolver (FR-SOURCE)
    │   ├── GitHub source resolution
    │   ├── Filesystem source resolution
    │   └── Stdin source resolution
    ├── Runner (FR-RUN, FR-WSL)
    │   ├── NativeRunner
    │   └── WslRunner
    ├── PacketBuilder (FR-PKT)
    │   └── InsightCache (FR-CACHE)
    ├── SecretRedactor (FR-SEC)
    ├── FixupEngine (FR-FIX)
    ├── LockManager (FR-LOCK)
    ├── Canonicalizer (FR-JCS)
    ├── ReceiptManager (FR-JCS, FR-EXIT)
    ├── StatusManager (FR-STA)
    └── Benchmark (FR-BENCH)
```

### Key Design Principles

1. **Atomic Operations**: All writes staged → fsync → atomic rename
2. **Single JCS Choke Point**: All JSON output flows through one emitter
3. **Error Mapping**: Every error maps to `{exit_code, error_kind, error_reason}`
4. **Trait-Based**: Components implement traits for testability
5. **Security First**: Secrets scanned before any external invocation or persistence

## Module Design

### 0. LLM Backend Abstraction (FR-LLM) ⏳ NEEDS IMPLEMENTATION

**Purpose**: Provide a unified interface for invoking language models regardless of transport method (CLI or HTTP).

**Proposed Interface** (`src/llm.rs`):

```rust
pub struct LlmInvocation {
    pub spec_id: String,
    pub phase: PhaseId,
    pub prompt: String,        // already packetized & redacted
    pub timeout: Duration,
    pub model: Option<String>, // provider-specific override
}

pub struct LlmResult {
    pub raw_response: String,       // full text
    pub stderr_tail: Option<String>,
    pub timed_out: bool,
    pub provider: String,           // "gemini-cli", "claude-cli", "openrouter", "anthropic"
    pub model_used: String,
    pub tokens_input: Option<u32>,
    pub tokens_output: Option<u32>,
}

pub trait LlmBackend {
    fn name(&self) -> &'static str;
    fn invoke(&self, invocation: &LlmInvocation) -> Result<LlmResult, RunnerError>;
}

pub enum BackendKind {
    ClaudeCli,
    GeminiCli,
    OpenRouterApi,
    AnthropicApi,
}

pub struct BackendConfig {
    pub kind: BackendKind,
    pub binary_path: Option<PathBuf>,  // for CLI backends
    pub base_url: Option<String>,      // for HTTP backends
    pub api_key_env: Option<String>,   // for HTTP backends
    pub model: String,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
}
```

**Design Notes**:
- Orchestrator only interacts with `Box<dyn LlmBackend>`
- Concrete implementations: `GeminiCliBackend`, `ClaudeCliBackend`, `OpenRouterBackend`, `AnthropicBackend`
- Provider selection via config `[llm] provider = "gemini-cli"` with optional `fallback_provider`
- All backends return `LlmResult` with consistent structure
- Transport-specific details hidden behind trait

**Implementation Status**:
- ⏳ **Needs**: Trait definition and factory pattern
- ⏳ **Needs**: Integration with orchestrator
- ⏳ **Needs**: Provider selection logic with fallback

### 1. Runner Module (FR-RUN, FR-WSL) ✅ IMPLEMENTED

**Purpose**: Execute Claude CLI with timeout enforcement, stream merging, and WSL interop.

**Note**: This module will be refactored to implement the `LlmBackend` trait as `ClaudeCliBackend` in future work.

**Actual Implementation** (`src/runner.rs`):

```rust
pub enum RunnerMode { Auto, Native, Wsl }

pub struct WslOptions {
    pub distro: Option<String>,
    pub claude_path: Option<String>,
}

pub struct Runner {
    pub mode: RunnerMode,
    pub wsl_options: WslOptions,
}

pub struct ClaudeResponse {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub runner_used: RunnerMode,
    pub runner_distro: Option<String>,
}

impl Runner {
    pub fn new(mode: RunnerMode, wsl_options: WslOptions) -> Self;
    pub fn native() -> Self;
    pub fn auto() -> Result<Self, RunnerError>;
    pub fn detect_auto() -> Result<RunnerMode, RunnerError>;
    pub fn execute_claude(&self, args: &[String], stdin_content: &str) -> Result<ClaudeResponse, RunnerError>;
    pub fn get_wsl_distro_name(&self) -> Option<String>;
    pub fn validate(&self) -> Result<(), RunnerError>;
    pub fn description(&self) -> String;
}
```

**Implementation Status**:
- ✅ Native and WSL execution modes implemented
- ✅ Auto mode with detection (native first, WSL fallback on Windows)
- ✅ WSL distro capture from `wsl -l -q` or `$WSL_DISTRO_NAME`
- ✅ Stdin piping and stdout/stderr capture
- ✅ NDJSON framing with last valid JSON frame selection
- ✅ Timeout enforcement with tokio
- ✅ Ring buffers for stdout (2 MiB) and stderr (256 KiB)
- ✅ Process tree termination (Windows Job Objects, Unix killpg)
- ✅ Integration with orchestrator for timeout handling

**Behavioral Contracts**:
- **NDJSON merge**: Parse stdout line-by-line; return the **last valid JSON frame**. If none, return `claude_failure` with a **redacted** tail excerpt (≤256 chars pre-redaction)
- **Buffers**: Ring buffers: stdout 2 MiB, stderr 256 KiB (CLI-configurable via `--stdout-cap-bytes`, `--stderr-cap-bytes`). Receipts cap `stderr_redacted` at 2048 bytes **after** redaction
- **Timeouts**: `tokio::time::timeout` → TERM → 5s → KILL; always drain pipes. Unix: `killpg`. Windows: Job Objects for process-tree termination
- **WSL**: Use `wslpath -a` for canonical path translation (fallback `/mnt/<drive-letter>/<rest>`). Pass argv directly to `wsl.exe --exec` (no shell quoting). Artifacts persist in the Windows spec root

**Verification Status**: ✅ Complete - All verification tasks (V2.3, V2.4, V2.5, V2.6) completed



### 2. Orchestrator Module (FR-ORC) ✅ IMPLEMENTED

**Purpose**: Enforce phase order, coordinate execution, manage state transitions.

**Actual Implementation** (`src/orchestrator.rs`):

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhaseId {
    Requirements,
    Design,
    Tasks,
    Review,
    Fixup,
    Final,
}

pub struct PhaseOrchestrator {
    spec_id: String,
    cfg: EffectiveConfig,
    paths: SpecPaths,
    runner_cfg: RunnerConfig,
}

impl PhaseOrchestrator {
    pub async fn execute_phase(&self, phase: PhaseId) -> crate::Result<()>;
    pub async fn resume(&self, to: PhaseId) -> crate::Result<()>;
    fn validate_transition(&self, target: PhaseId) -> crate::Result<()>;
    fn write_partial_artifact(&self, phase: PhaseId, res: &InvocationResult) 
        -> crate::Result<PathBuf>;
    fn promote_partial_to_final(&self, phase: PhaseId, partial: &Path) 
        -> crate::Result<()>;
}
```

**Implementation Status**:
- ✅ Phase transition validation with legal phase order enforcement
- ✅ Illegal transition detection with actionable guidance (exit 2)
- ✅ Dependency checking before phase execution
- ✅ Complete execution flow with all 10 steps
- ✅ Stale `.partial/` directory cleanup
- ✅ Exclusive lock acquisition
- ✅ Packet building integration
- ✅ Secret scanning integration
- ✅ Packet limit enforcement
- ✅ Runner invocation with timeout
- ✅ Partial artifact writing to `.partial/` subdirectory
- ✅ Atomic promotion to final names
- ✅ Receipt writing for success and error cases
- ✅ Phase trait system integration

**Execution Flow**:
0. Remove stale `.partial/` directories before writing new partial artifacts
1. Validate transition (legal phase order)
2. Acquire exclusive lock
3. Build packet via Phase trait system
4. Scan for secrets
5. Enforce packet limits
6. Invoke Runner with timeout
7. Write partial artifact to `.partial/` subdirectory
8. Promote to final (atomic rename)
9. Write receipt (success or error)
10. On error, write an error receipt (JCS), including any warnings (e.g., stale lock broken, rename retries)

**Phase State Source of Truth**:
- The last **successful** receipt for `<spec-id>` defines the current completed phase
- If no prior receipt exists, current state is "none"; only Requirements is a legal next phase
- On start of any phase, remove stale `.partial/` trees (best-effort)

**Orchestrator Invariants**:
- **Crash cleanup**: Remove stale `.partial/` before staging new partials
- **Pre-invoke gates**: Lock acquired, packet limits enforced, secrets scanned. On failure, write error receipt; **do not** invoke Runner

**Error Handling**: On any error, write error receipt with JCS and exit with mapped code.

**Verification Status**: ✅ Complete - All verification tasks (V3.1, V3.2, V3.5, V3.6) completed



### 3. PacketBuilder Module (FR-PKT) ✅ IMPLEMENTED

**Purpose**: Assemble request payloads deterministically with size enforcement and priority-based selection.

**Actual Implementation** (`src/packet.rs`):

```rust
pub struct ContentSelector {
    include_patterns: GlobSet,
    exclude_patterns: GlobSet,
    priority_rules: PriorityRules,
}

pub struct PacketBuilder {
    selector: ContentSelector,
    redactor: SecretRedactor,
    cache: Option<InsightCache>,
    max_bytes: usize,
    max_lines: usize,
}

pub struct Packet {
    pub content: String,
    pub blake3_hash: String,
    pub evidence: PacketEvidence,
    pub budget_used: BudgetUsage,
}

impl PacketBuilder {
    pub fn new() -> Result<Self>;
    pub fn with_limits(max_bytes: usize, max_lines: usize) -> Result<Self>;
    pub fn with_cache(cache_dir: Utf8PathBuf) -> Result<Self>;
    pub fn build_packet(&mut self, base_path: &Utf8Path, phase: &str, context_dir: &Utf8Path, logger: Option<&Logger>) -> Result<Packet>;
}
```

**Implementation Status**:
- ✅ Priority-based file selection (Upstream > High > Medium > Low)
- ✅ LIFO ordering within priority classes
- ✅ Deterministic ordering (sorted file paths)
- ✅ Byte and line counting during assembly
- ✅ Budget enforcement with overflow detection
- ✅ Secret scanning integration
- ✅ InsightCache integration for token efficiency
- ✅ Packet preview writing to context/
- ✅ Wired into orchestrator via Phase trait system
- ✅ Manifest writing on overflow
- ✅ --debug-packet flag implementation

**Verification Status**: ✅ Complete - All verification tasks (V2.1, V7.3, V7.6, V9.2, V9.3) completed



### 4. Secret Redaction Module (FR-SEC) ✅ IMPLEMENTED

**Purpose**: Detect secrets before external invocation; redact before persistence.

**Actual Implementation** (`src/redaction.rs`):

```rust
pub struct SecretRedactor {
    default_patterns: HashMap<String, Regex>,
    extra_patterns: HashMap<String, Regex>,
    ignored_patterns: Vec<String>,
}

pub struct SecretMatch {
    pub pattern_id: String,
    pub file_path: String,
    pub line_number: usize,
    pub column_range: (usize, usize),
    pub context: String,
}

pub struct RedactionResult {
    pub content: String,
    pub matches: Vec<SecretMatch>,
    pub has_secrets: bool,
}

impl SecretRedactor {
    pub fn new() -> Result<Self>;
    pub fn add_extra_pattern(&mut self, pattern_id: String, pattern: &str) -> Result<()>;
    pub fn ignore_pattern(&mut self, pattern_id: String);
    pub fn has_secrets(&self, content: &str, file_path: &str) -> Result<bool>;
    pub fn scan_for_secrets(&self, content: &str, file_path: &str) -> Result<Vec<SecretMatch>>;
    pub fn redact_content(&self, content: &str, file_path: &str) -> Result<RedactionResult>;
}
```

**Default Patterns**:
- `ghp_[A-Za-z0-9]{36}` (GitHub PAT)
- `AKIA[0-9A-Z]{16}` (AWS Access Key)
- `AWS_SECRET_ACCESS_KEY=` (AWS Secret)
- `xox[baprs]-` (Slack tokens)
- `Bearer [A-Za-z0-9._-]{20,}` (Bearer tokens)

**Implementation Status**:
- ✅ Default pattern matching for common secret types
- ✅ Custom pattern support via `--extra-secret-pattern`
- ✅ Pattern suppression via `--ignore-secret-pattern`
- ✅ Exit code 8 on secret detection
- ✅ Pattern name reporting (not actual secret)
- ✅ Redaction with `***` replacement
- ✅ Global redaction applied to all human-readable fields
- ✅ Receipts never include raw packet content or environment variables
- ✅ File path redaction support
- ✅ Integration with PacketBuilder and orchestrator

**Implementation Notes**:
- Build rules from defaults + `--extra-secret-pattern` - `--ignore-secret-pattern`
- On match: exit code 8, report pattern name (not actual secret)
- Redaction: replace matches with `***`, cap output length
- **Global Redaction**: Apply to all human-readable fields before emission (stderr, error_reason, warnings, contextual strings, doctor/status text, previews)
- Receipts MUST NOT contain raw packet content or environment variables
- Secrets can appear in file paths; treat paths as non-secret but still run redaction on any string copied into receipts/logs

**Verification Status**: ✅ Complete - All verification tasks (V2.2, V7.2, V7.6) completed



### 5. FixupEngine Module (FR-FIX) ✅ IMPLEMENTED

**Purpose**: Validate and apply file changes safely with path validation.

**Actual Implementation** (`src/fixup.rs`):

```rust
pub struct ProposedChange {
    pub path: PathBuf,
    pub old_hash_first8: Option<String>,
    pub new_content: String,
    pub added: usize,
    pub removed: usize,
}

pub struct FixupPlan {
    pub changes: Vec<ProposedChange>,
}

pub struct FixupEngine;

impl FixupEngine {
    pub fn validate(plan: &FixupPlan, root: &Path, allow_links: bool) 
        -> crate::Result<()>;
    
    pub fn preview(plan: &FixupPlan) -> String;
    
    pub fn apply(plan: &FixupPlan, root: &Path) 
        -> crate::Result<Vec<AppliedChange>>;
}

pub struct AppliedChange {
    pub path: PathBuf,
    pub blake3_first8: String,
    pub applied: bool,
    pub warnings: Vec<String>,  // e.g., permission preservation issues
}
```

**Implementation Status**:
- ✅ Path canonicalization and validation
- ✅ Root boundary checking
- ✅ `..` component rejection
- ✅ Absolute path outside root rejection
- ✅ Symlink detection with lstat
- ✅ Hardlink detection
- ✅ `--allow-links` flag support
- ✅ Preview mode (no file modifications)
- ✅ Apply mode with atomic writes
- ✅ `.bak` backup creation
- ✅ Windows rename retry logic
- ✅ Permission preservation (POSIX mode bits, Windows attributes)
- ✅ Cross-filesystem fallback (copy→fsync→replace)
- ✅ Line ending normalization
- ✅ Warning recording for permission issues

**Implementation Notes**:
- **Path Validation**: Canonicalize, ensure under root, reject `..` and absolute paths outside root
- **Symlink/Hardlink**: Reject by default unless `--allow-links`
- **Atomic Write Invariants**: Atomic rename MUST be same-filesystem. If not, fallback to copy→fsync→replace (remove original only after successful fsync+close of the new file)
- **Permission Preservation**: Preserve POSIX mode bits and Windows attributes on replace. Record a warning in `AppliedChange.warnings` if attributes cannot be preserved
- **Backup Creation**: Write to `.tmp`, fsync, create `.bak` if exists, rename with Windows retry
- **Line Ending Normalization**: Compute diff estimates after normalizing line endings

**Verification Status**: ✅ Complete - All verification tasks (V4.1-V4.7) completed




### 6. LockManager Module (FR-LOCK) ✅ IMPLEMENTED

**Purpose**: Prevent concurrent runs; detect and handle stale locks.

**Actual Implementation** (`src/lock.rs`):

```rust
pub struct LockGuard {
    path: PathBuf,
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        // Best-effort removal
    }
}

pub struct LockManager;

impl LockManager {
    pub fn acquire(paths: &SpecPaths, force: bool, ttl: Duration) 
        -> crate::Result<LockGuard>;
    
    fn is_stale(lock_data: &LockData, ttl: Duration) -> bool;
}

#[derive(Serialize, Deserialize)]
struct LockData {
    pid: u32,
    host: String,
    started_at: String,  // RFC3339
}
```

**Implementation Status**:
- ✅ Advisory lock file creation with `{pid, host, started_at}`
- ✅ Lock acquisition with exit code 9 if held by active process
- ✅ Stale detection (PID not alive OR age > TTL)
- ✅ `--force` flag to break stale locks
- ✅ Warning recording in receipt when stale lock broken
- ✅ Lock release on normal exit
- ✅ Best-effort lock release on panic (Drop trait)
- ✅ Configurable TTL parameter (default 15 minutes)

**Implementation Notes**:
- Lock file: `.xchecker/specs/<spec-id>/.lock.json`
- Contains: `{pid, host, started_at}`
- **Stale Detection**: PID not alive on same host OR age > TTL (configurable, default 15 min)
- **Force Flag**: `--force` breaks stale locks, records warning in receipt with `stale_lock_broken: true`
- Exit code 9 if lock held by active process
- TTL parameter read from config or CLI flag

**Verification Status**: ✅ Complete - All verification tasks (V3.3, V7.5) completed

### 7
. Lockfile & Drift Module (FR-LOCK) ✅ IMPLEMENTED

**Purpose**: Track reproducibility; detect configuration drift.

**Actual Implementation** (integrated into `src/lock.rs` and `src/status.rs`):

```rust
#[derive(Serialize, Deserialize)]
pub struct Lockfile {
    pub model_full_name: String,
    pub claude_cli_version: String,
    pub schema_version: String,
}

pub struct Drift {
    pub fields: Vec<String>,
}

pub fn compute_drift(current: &Lockfile, locked: &Lockfile) -> Option<Drift>;
pub fn load_lockfile(paths: &SpecPaths) -> crate::Result<Option<Lockfile>>;
pub fn create_lockfile(paths: &SpecPaths, data: &Lockfile) -> crate::Result<()>;
```

**Implementation Status**:
- ✅ Lockfile creation with `--create-lock`
- ✅ Drift detection for each field (model_full_name, claude_cli_version, schema_version)
- ✅ `--strict-lock` enforcement (exit before phase execution if drift detected)
- ✅ Drift reporting in status output
- ✅ Lockfile loading and validation

**Implementation Notes**:
- Lockfile: `.xchecker/specs/<spec-id>/lock.json`
- Created by `xchecker init --create-lock`
- Drift computed by comparing current vs locked values
- `--strict-lock` enforces: exit before phase execution if drift detected

**Verification Status**: ✅ Complete - All verification tasks (V1.7) completed



### 8. Canonicalization Module (FR-JCS) ✅ IMPLEMENTED

**Purpose**: Single choke point for RFC 8785-compliant JSON canonicalization and content normalization.

**Actual Implementation** (`src/canonicalization.rs`):

```rust
pub struct Canonicalizer {
    version: String,
}

impl Canonicalizer {
    pub fn new() -> Self;
    pub fn version(&self) -> &str;
    pub fn backend(&self) -> &'static str;
    pub fn canonicalize_yaml(&self, content: &str) -> Result<String>;
    pub fn normalize_markdown(&self, content: &str) -> Result<String>;
    pub fn hash_canonicalized_with_context(...) -> Result<String>;
}
```

**Implementation Notes**:
- ✅ Uses `serde_json_canonicalizer` for RFC 8785 compliance
- ✅ Integrated YAML and Markdown normalization
- ✅ BLAKE3 hashing integrated into canonicalization
- ✅ All JSON writes (receipts, status, doctor) use this module
- ✅ Re-serialization produces byte-identical output

### 9. Hash Functions (FR-JCS) ✅ IMPLEMENTED

**Purpose**: Compute stable BLAKE3 hashes for artifacts.

**Actual Implementation** (integrated into `canonicalization.rs` and `artifact.rs`):

```rust
// In canonicalization.rs
impl Canonicalizer {
    pub fn hash_canonicalized_with_context(
        &self,
        content: &str,
        file_type: FileType,
        phase: &str,
    ) -> Result<String, XCheckerError>;
}

// In cache.rs
pub fn calculate_content_hash(content: &str) -> String;
```

**Implementation Notes**:
- ✅ Uses `blake3` crate
- ✅ Full 64-character hex hashes (not truncated to 8)
- ✅ Computes on canonicalized content before hashing
- ✅ Stable across platforms with LF line endings
- ✅ Integrated with canonicalization for deterministic results



### 10. Receipt Manager Module (FR-JCS, FR-EXIT) ✅ IMPLEMENTED

**Purpose**: Write phase receipts with proper error mapping.

**Actual Implementation** (`src/receipt.rs`):

```rust
#[derive(Serialize)]
pub struct Receipt {
    pub schema_version: String,
    pub emitted_at: String,  // RFC3339 UTC
    pub canonicalization_backend: String,
    pub phase: String,
    pub exit_code: i32,
    pub error_kind: Option<String>,
    pub error_reason: Option<String>,
    pub runner: Option<String>,
    pub runner_distro: Option<String>,
    pub duration_ms: Option<u128>,
    pub stderr_redacted: Option<String>,
    pub warnings: Vec<String>,
}

pub struct ReceiptManager;

impl ReceiptManager {
    pub fn write_success(
        phase: PhaseId,
        cfg: &EffectiveConfig,
        inv: &InvocationResult,
        paths: &SpecPaths,
    ) -> crate::Result<()>;
    
    pub fn write_error(
        phase: PhaseId,
        err: &XCheckerError,
        paths: &SpecPaths,
    ) -> crate::Result<()>;
}
```

**Implementation Status**:
- ✅ JCS emission for all receipts
- ✅ Schema version "1" with canonicalization_backend "jcs-rfc8785"
- ✅ Error mapping to exit codes via `XCheckerError::exit_code()`
- ✅ Redaction applied to all human-readable strings before emission
- ✅ Optional fields support (stderr_redacted, runner_distro, error_kind, error_reason, warnings)
- ✅ Success and error receipt writing
- ✅ Atomic write pattern (temp → fsync → rename)

**Implementation Notes**:
- Always use JCS emission
- Include `schema_version: "1"`, `canonicalization_backend: "jcs-rfc8785"`
- Map errors to exit codes via `XCheckerError::exit_code()`
- **Redaction**: Apply to all human-readable strings (stderr, error_reason, context) before emission
- **Optional Fields**: `stderr_redacted`, `runner_distro` (schema v1 allows `additionalProperties: true`)
- Schema alignment: Ensure these fields are documented as optional in `receipt.v1.json`

**Verification Status**: ✅ Complete - All verification tasks (V1.4, V7.1) completed



### 11. Status Manager Module (FR-STA) ✅ IMPLEMENTED

**Purpose**: Report effective configuration and spec state.

**Actual Implementation** (`src/status.rs`):

```rust
#[derive(Serialize)]
pub struct StatusOutput {
    pub schema_version: String,
    pub emitted_at: String,
    pub runner: String,
    pub canonicalization_backend: String,
    pub artifacts: Vec<ArtifactMeta>,
    pub effective_config: BTreeMap<String, ConfigValue>,
    pub lock_drift: Option<DriftInfo>,
}

#[derive(Serialize)]
pub struct ArtifactMeta {
    pub path: String,
    pub blake3_first8: String,
}

#[derive(Serialize)]
pub struct ConfigValue {
    pub value: serde_json::Value,
    pub source: String,  // "cli" | "config" | "default"
}

pub struct StatusManager;

impl StatusManager {
    pub fn build(paths: &SpecPaths, cfg: &EffectiveConfig) 
        -> crate::Result<StatusOutput>;
    
    pub fn emit_json(status: &StatusOutput) -> crate::Result<String>;
}
```

**Implementation Status**:
- ✅ JCS emission for deterministic output
- ✅ Artifact enumeration with blake3_first8 hashes
- ✅ Effective config with source attribution (cli/config/default)
- ✅ Lock drift detection and reporting
- ✅ Works on fresh specs (no prior receipts)
- ✅ Pending fixup summary with counts only
- ✅ Sorted artifacts by path

**Implementation Notes**:
- Use JCS emission for deterministic output
- Sort artifacts by path
- Source attribution must be exact: `cli`, `config`, or `default`
- Works on fresh specs (no prior receipts)
- **Pending Fixup Summary**: Provide counts only: `{ "pending_fixups": { "targets": <u32>, "est_added": <u32>, "est_removed": <u32> } }`. Optional field; omit when unavailable. Do not surface file contents or diffs

**Verification Status**: ✅ Complete - All verification tasks (V1.5, V4.8, V4.9) completed



### 12. Config Module (FR-CFG) ✅ IMPLEMENTED

**Purpose**: Load and merge configuration with precedence.

**Actual Implementation** (`src/config.rs`):

```rust
#[derive(Clone, Debug)]
pub struct EffectiveConfig {
    // Each field tracks value + source
    pub packet_max_bytes: (usize, &'static str),
    pub packet_max_lines: (usize, &'static str),
    pub runner_mode: (RunnerMode, &'static str),
    pub phase_timeout: (Duration, &'static str),
    // ... other fields
}

pub fn load_effective(
    cli: CliOverrides,
    home: Option<PathBuf>,
) -> crate::Result<EffectiveConfig>;

pub fn discover_config() -> crate::Result<Option<PathBuf>>;
```

**Implementation Status**:
- ✅ Upward discovery from CWD for `.xchecker/config.toml`
- ✅ Stop at filesystem root or VCS boundary (`.git`)
- ✅ Precedence: CLI > config file > defaults
- ✅ Source tracking for each value (cli/config/default)
- ✅ `XCHECKER_HOME` env var override
- ✅ `--config` explicit path support
- ✅ Invalid config handling

**Implementation Notes**:
- Search upward from CWD for `.xchecker/config.toml`
- Stop at filesystem root or VCS boundary (`.git`)
- Precedence: CLI > config file > defaults
- Track source for each value: `"cli"`, `"config"`, `"default"`
- `XCHECKER_HOME` env var overrides state location

**Verification Status**: ✅ Complete - All verification tasks (V1.6) completed



### 13. WSL Module (FR-WSL) ✅ IMPLEMENTED

**Purpose**: Detect WSL, translate paths, validate Claude availability.

**Note**: WSL support will be extended to work with any CLI-based LLM backend, not just Claude.

**Actual Implementation** (`src/wsl.rs`):

```rust
pub fn is_wsl_available() -> bool;
pub fn validate_claude_in_wsl(distro: Option<&str>) -> crate::Result<bool>;
pub fn translate_win_to_wsl(p: &Path) -> String;
pub fn translate_env_for_wsl(env: &[(String, String)]) -> Vec<(String, String)>;
```

**Implementation Status**:
- ✅ WSL availability detection via `wsl.exe -l -q`
- ✅ Distro list parsing
- ✅ Claude validation in WSL via `which claude`
- ✅ Path translation using `wslpath -a` with fallback
- ✅ Environment variable translation
- ✅ Discrete argv element passing (no shell quoting)
- ✅ Doctor integration with WSL checks
- ✅ Actionable suggestions when native missing but WSL ready

**Implementation Notes**:
- Check `wsl.exe -l -q` for distro list
- Validate Claude: `wsl.exe -d <distro> -- which claude`
- **Path Translation**: Use `wsl.exe wslpath -a <winpath>` for correctness across drives and UNC paths. If it fails, fallback to `/mnt/<drive-letter>/<rest>` heuristic
- **Argument Passing**: When building `wsl.exe --exec` invocations, pass args as discrete argv elements (not shell-joined) to avoid quoting issues
- **Doctor**: Report native `claude --version` presence, WSL availability (distros), and `which claude` inside selected distro. Suggest `--runner-mode wsl --runner-distro <name>` when native missing but WSL ready

**Verification Status**: ✅ Complete - All verification tasks (V5.1-V5.7) completed

### 14. Benchmark Module (FR-BENCH) ✅ IMPLEMENTED

**Purpose**: Measure performance with process-scoped memory.

**Actual Implementation** (`src/benchmark.rs`):

```rust
#[derive(Serialize)]
pub struct BenchmarkResults {
    pub ok: bool,
    pub timings_ms: BTreeMap<String, f64>,
    pub rss_mb: f64,
    pub commit_mb: Option<f64>,  // Windows only
}

pub fn run_benchmark(opts: BenchmarkOpts) -> crate::Result<BenchmarkResults>;
```

**Implementation Status**:
- ✅ Deterministic workload generation
- ✅ Warm-up pass execution
- ✅ N>=3 measured runs with median calculation
- ✅ Process RSS measurement (all OSs)
- ✅ Commit MB measurement (Windows only)
- ✅ Threshold comparison
- ✅ Structured JSON output
- ✅ Configurable thresholds via CLI or config

**Implementation Notes**:
- One warm-up run + N>=3 measured runs
- Report median timings
- Use `sysinfo` crate for process RSS
- Compare median against thresholds
- Set `ok: false` if any threshold exceeded

**Verification Status**: ✅ Complete - All verification tasks (V6.1-V6.5) completed

### 16. InsightCache Module (FR-CACHE) ✅ IMPLEMENTED

**Purpose**: Cache file insights based on BLAKE3 content hashes to avoid reprocessing unchanged files.

**Actual Implementation** (`src/cache.rs`):

```rust
pub struct InsightCache {
    cache_dir: Utf8PathBuf,
    memory_cache: HashMap<String, CachedInsight>,
    stats: CacheStats,
}

pub struct CachedInsight {
    pub content_hash: String,
    pub file_path: String,
    pub priority: Priority,
    pub insights: Vec<String>,
    pub phase: String,
    pub cached_at: DateTime<Utc>,
    pub file_size: u64,
    pub last_modified: DateTime<Utc>,
}

pub struct CacheStats {
    pub hits: usize,
    pub misses: usize,
    pub invalidations: usize,
    pub writes: usize,
}

impl InsightCache {
    pub fn new(cache_dir: Utf8PathBuf) -> Result<Self>;
    pub fn get_insights(&mut self, file_path: &Utf8Path, content_hash: &str, phase: &str, logger: Option<&Logger>) -> Result<Option<Vec<String>>>;
    pub fn store_insights(&mut self, file_path: &Utf8Path, content: &str, content_hash: &str, phase: &str, priority: Priority, insights: Vec<String>, logger: Option<&Logger>) -> Result<()>;
    pub fn generate_insights(&self, content: &str, file_path: &Utf8Path, phase: &str, priority: Priority) -> Vec<String>;
    pub fn clear(&mut self) -> Result<()>;
    pub fn log_stats(&self, logger: &Logger);
}

pub fn calculate_content_hash(content: &str) -> String;
```

**Implementation Status**:
- ✅ BLAKE3-based content hashing for cache keys
- ✅ Two-tier caching (memory + disk) for performance
- ✅ File change detection via size and modification time
- ✅ Automatic cache invalidation when files change
- ✅ Phase-specific insight generation (10-25 bullet points)
- ✅ Cache statistics tracking (hits, misses, invalidations)
- ✅ Comprehensive test coverage
- ✅ Integration with PacketBuilder (wired)
- ✅ Atomic writes with redaction

**Behavioral Contracts**:
- Writes: temp → fsync → atomic rename; redact user-visible strings pre-write
- TTL expiration is fail-open (treat as miss; do not block phase execution)

**Verification Status**: ✅ Complete - All verification tasks (V8.7, V9.3) completed

### 17. SourceResolver Module (FR-SOURCE) ✅ IMPLEMENTED

**Purpose**: Resolve different source types (GitHub, filesystem, stdin) for problem statements.

**Actual Implementation** (`src/source.rs`):

```rust
pub enum SourceType {
    GitHub { owner: String, repo: String },
    FileSystem { path: PathBuf },
    Stdin,
}

pub struct SourceContent {
    pub source_type: SourceType,
    pub content: String,
    pub metadata: HashMap<String, String>,
}

pub struct SourceResolver;

impl SourceResolver {
    pub fn resolve_github(owner: &str, repo: &str, issue_id: &str) -> Result<SourceContent, SourceError>;
    pub fn resolve_filesystem(path: &PathBuf) -> Result<SourceContent, SourceError>;
    pub fn resolve_stdin() -> Result<SourceContent, SourceError>;
}

pub enum SourceError {
    GitHubResolutionFailed { reason: String },
    FileSystemNotFound { path: String },
    StdinInvalid,
    InvalidConfiguration { reason: String },
}
```

**Implementation Status**:
- ✅ GitHub source resolution with validation
- ✅ Filesystem source resolution (files and directories)
- ✅ Stdin source resolution with validation
- ✅ User-friendly error messages with suggestions
- ✅ Source metadata tracking
- ✅ Comprehensive test coverage
- ✅ Integration with CLI commands (wired)
- ✅ Path deduplication and exclude/include handling
- ✅ File and byte cap enforcement

**Behavioral Contracts**:
- Apply excludes before includes; deduplicate paths; enforce file/byte caps before packet construction
- Cap overflow maps to `packet_overflow` before Runner invocation

**Verification Status**: ✅ Complete - All verification tasks (V8.8, V9.2) completed

### 18. Gemini CLI Backend (FR-LLM-GEM) ⏳ NEEDS IMPLEMENTATION

**Purpose**: Implement LLM backend for Gemini CLI with non-interactive invocation.

**Proposed Implementation** (`src/llm/gemini_cli.rs`):

```rust
pub struct GeminiCliBackend {
    binary_path: PathBuf,
    default_model: String,
    phase_models: HashMap<PhaseId, String>,
    allow_tools: bool,
}

impl GeminiCliBackend {
    pub fn new(config: &BackendConfig) -> Result<Self>;
    pub fn validate_binary(&self) -> Result<(), LlmError>;
    fn build_args(&self, invocation: &LlmInvocation) -> Vec<String>;
}

impl LlmBackend for GeminiCliBackend {
    fn name(&self) -> &'static str { "gemini-cli" }
    
    fn invoke(&self, invocation: &LlmInvocation) -> Result<LlmResult, RunnerError> {
        // Build args: gemini -p "<prompt>" --model <model>
        // Use existing Runner infrastructure for process control
        // Apply timeout, ring buffers, process termination
        // Return LlmResult with stdout as raw_response
    }
}
```

**Configuration**:
```toml
[llm]
provider = "gemini-cli"

[llm.gemini]
binary = "gemini"  # or full path
default_model = "gemini-2.0-flash-lite"
allow_tools = false  # experimental

# Optional per-phase overrides
model_requirements = "gemini-2.0-flash-lite"
model_design = "gemini-2.5-flash"
model_tasks = "gemini-2.0-flash-lite"
```

**Implementation Notes**:
- Reuse existing Runner infrastructure for process control
- Non-interactive mode only: `gemini -p "<prompt>" --model <model>`
- Assume Gemini CLI handles auth via `GEMINI_API_KEY` environment variable
- Text-only mode by default (no filesystem tools)
- Per-phase model selection: check phase-specific override, fallback to default_model

**Verification Tasks**: V11.3

### 19. HTTP Client Module (FR-LLM-API) ⏳ NEEDS IMPLEMENTATION

**Purpose**: Provide HTTP transport for API-based LLM providers.

**Proposed Implementation** (`src/llm/http_client.rs`):

```rust
pub struct HttpClient {
    client: reqwest::Client,
}

impl HttpClient {
    pub fn new() -> Result<Self>;
    
    pub async fn post_json(
        &self,
        url: &str,
        headers: HashMap<String, String>,
        body: serde_json::Value,
    ) -> Result<serde_json::Value, HttpError>;
}

pub enum HttpError {
    Auth { status: u16, message: String },
    Quota { status: u16, message: String },
    ServerError { status: u16, message: String },
    NetworkTimeout { duration: Duration },
    InvalidResponse { reason: String },
}
```

**Implementation Notes**:
- Use `reqwest` crate for HTTP client
- Support both sync and async (start with sync for simplicity)
- Never log API keys, headers, or full request bodies
- Map HTTP errors to existing error taxonomy
- Apply redaction to error messages before persistence

**Verification Tasks**: V11.4

### 19a. Budgeted Backend Wrapper (NFR9) ⏳ NEEDS IMPLEMENTATION

**Purpose**: Enforce per-process call budgets for API-based LLM providers to prevent quota exhaustion.

**Proposed Implementation** (`src/llm/budgeted.rs`):

```rust
pub struct BudgetConfig {
    pub max_calls: u32,
}

pub struct BudgetedBackend<B> {
    inner: B,
    used: std::sync::atomic::AtomicU32,
    budget: BudgetConfig,
}

impl<B: LlmBackend> BudgetedBackend<B> {
    pub fn new(inner: B, budget: BudgetConfig) -> Self {
        Self {
            inner,
            used: AtomicU32::new(0),
            budget,
        }
    }
    
    pub fn calls_used(&self) -> u32 {
        self.used.load(Ordering::Relaxed)
    }
}

impl<B: LlmBackend> LlmBackend for BudgetedBackend<B> {
    fn name(&self) -> &'static str {
        self.inner.name()
    }
    
    fn invoke(&self, invocation: &LlmInvocation) -> Result<LlmResult, RunnerError> {
        let used = self.used.fetch_add(1, Ordering::Relaxed) + 1;
        if used > self.budget.max_calls {
            return Err(RunnerError::LlmBudgetExceeded {
                provider: self.name().to_string(),
                max_calls: self.budget.max_calls,
                used_calls: used,
            });
        }
        self.inner.invoke(invocation)
    }
}

fn read_budget_from_env(provider: &str) -> Option<u32> {
    let var_name = format!("XCHECKER_{}_BUDGET", provider.to_uppercase());
    std::env::var(var_name)
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|v| *v > 0)
}
```

**Usage in Factory**:
```rust
fn make_openrouter_backend(cfg: &Config) -> Box<dyn LlmBackend> {
    let inner = OpenRouterBackend::new(cfg)?;
    let max = read_budget_from_env("openrouter").unwrap_or(20); // NFR9 default
    Box::new(BudgetedBackend::new(inner, BudgetConfig { max_calls: max }))
}
```

**Implementation Notes**:
- Thread-safe atomic counter for call tracking
- Default budget: 20 calls per process (conservative for 1000/day quota)
- Environment override: `XCHECKER_OPENROUTER_BUDGET` for local runs
- Fail fast on budget exhaustion with clear error message
- Orchestrator doesn't need to know about budgets; it's a backend property

**Verification Tasks**: V11.5

### 20. OpenRouter Backend (FR-LLM-OR, NFR9) ⏳ NEEDS IMPLEMENTATION

**Purpose**: Implement LLM backend for OpenRouter API with call budget enforcement.

**Proposed Implementation** (`src/llm/openrouter.rs`):

```rust
pub struct OpenRouterBackend {
    client: HttpClient,
    base_url: String,
    api_key: String,
    model: String,
    max_tokens: u32,
    temperature: f32,
}

impl OpenRouterBackend {
    pub fn new(config: &BackendConfig) -> Result<Self>;
    fn build_request(&self, invocation: &LlmInvocation) -> serde_json::Value;
}

impl LlmBackend for OpenRouterBackend {
    fn name(&self) -> &'static str { "openrouter" }
    
    fn invoke(&self, invocation: &LlmInvocation) -> Result<LlmResult, RunnerError> {
        // Build OpenAI-compatible request
        // Add HTTP-Referer and X-Title headers
        // POST to base_url
        // Parse response and extract content
        // Return LlmResult
    }
}
```

**Configuration**:
```toml
[llm]
provider = "openrouter"

[llm.openrouter]
base_url = "https://openrouter.ai/api/v1/chat/completions"
api_key_env = "OPENROUTER_API_KEY"
model = "google/gemini-2.0-flash-lite"
max_tokens = 2048
temperature = 0.2
```

**Request Format** (OpenAI-compatible):
```json
{
  "model": "google/gemini-2.0-flash-lite",
  "messages": [
    {"role": "system", "content": "You are xchecker's analysis engine."},
    {"role": "user", "content": "<packet JSON here>"}
  ],
  "stream": false
}
```

**Implementation Notes**:
- Use OpenAI-compatible request format
- Include `HTTP-Referer: https://effortlesssteven.com/xchecker` header
- Include `X-Title: xchecker` header
- Read API key from environment variable specified in config
- Document that free model availability changes

**Verification Tasks**: V11.5

### 21. Anthropic API Backend (FR-LLM-ANTH) ⏳ NEEDS IMPLEMENTATION

**Purpose**: Implement LLM backend for Anthropic API.

**Proposed Implementation** (`src/llm/anthropic.rs`):

```rust
pub struct AnthropicBackend {
    client: HttpClient,
    base_url: String,
    api_key: String,
    model: String,
    max_tokens: u32,
    temperature: f32,
}

impl AnthropicBackend {
    pub fn new(config: &BackendConfig) -> Result<Self>;
    fn build_request(&self, invocation: &LlmInvocation) -> serde_json::Value;
}

impl LlmBackend for AnthropicBackend {
    fn name(&self) -> &'static str { "anthropic" }
    
    fn invoke(&self, invocation: &LlmInvocation) -> Result<LlmResult, RunnerError> {
        // Build Anthropic API request
        // POST to base_url
        // Parse response and extract content
        // Return LlmResult
    }
}
```

**Configuration**:
```toml
[llm]
provider = "anthropic"

[llm.anthropic]
base_url = "https://api.anthropic.com/v1/messages"
api_key_env = "ANTHROPIC_API_KEY"
model = "claude-3-5-sonnet-20241022"
max_tokens = 2048
temperature = 0.2
```

**Implementation Notes**:
- Use official Anthropic API schema
- Read API key from environment variable
- May need separate prompt templates for API vs CLI
- Consider compatibility with existing Claude CLI prompts

**Verification Tasks**: V11.6

### 22. LLM Backend Factory (FR-LLM) ⏳ NEEDS IMPLEMENTATION

**Purpose**: Create appropriate LLM backend based on configuration.

**Proposed Implementation** (`src/llm/factory.rs`):

```rust
pub struct LlmBackendFactory;

impl LlmBackendFactory {
    pub fn create(config: &EffectiveConfig) -> Result<Box<dyn LlmBackend>, LlmError> {
        let provider = &config.llm_provider;
        match provider.as_str() {
            "gemini-cli" => Ok(Box::new(GeminiCliBackend::new(&config.llm_config)?)),
            "claude-cli" => Ok(Box::new(ClaudeCliBackend::new(&config.llm_config)?)),
            "openrouter" => Ok(Box::new(OpenRouterBackend::new(&config.llm_config)?)),
            "anthropic" => Ok(Box::new(AnthropicBackend::new(&config.llm_config)?)),
            _ => Err(LlmError::UnknownProvider { name: provider.clone() }),
        }
    }
    
    pub fn create_with_fallback(config: &EffectiveConfig) -> Result<Box<dyn LlmBackend>, LlmError> {
        // Try primary provider
        match Self::create(config) {
            Ok(backend) => Ok(backend),
            Err(e) if config.llm_fallback_provider.is_some() => {
                // Try fallback provider
                let fallback_config = config.with_provider(&config.llm_fallback_provider.unwrap());
                Self::create(&fallback_config)
            }
            Err(e) => Err(e),
        }
    }
}
```

**Implementation Notes**:
- Factory pattern for backend creation
- Support fallback provider if primary unavailable
- Record fallback usage in receipt warnings
- Validate provider configuration before creation

**Verification Tasks**: V11.1

### 23. Additional Supporting Modules ✅ IMPLEMENTED

**Purpose**: Additional modules that support the core functionality.

#### 23a. Atomic Write Module (`src/atomic_write.rs`)

**Purpose**: Provide atomic file write operations with fsync and Windows retry logic.

**Implementation Status**:
- ✅ Temp file creation with fsync
- ✅ Atomic rename (same filesystem)
- ✅ Windows rename retry with exponential backoff
- ✅ Cross-filesystem fallback (copy→fsync→replace)
- ✅ UTF-8 encoding with LF line endings
- ✅ CRLF tolerance on read

#### 23b. Ring Buffer Module (`src/ring_buffer.rs`)

**Purpose**: Bounded circular buffers for stdout/stderr capture.

**Implementation Status**:
- ✅ Ring buffer implementation with configurable capacity
- ✅ Stdout buffer (2 MiB default)
- ✅ Stderr buffer (256 KiB default)
- ✅ Integration with Runner for output capture

#### 23c. Process Memory Module (`src/process_memory.rs`)

**Purpose**: Process-scoped memory tracking for benchmarks.

**Implementation Status**:
- ✅ RSS measurement (all OSs)
- ✅ Commit MB measurement (Windows only)
- ✅ Integration with benchmark module

#### 23d. Integration Tests Module (`src/integration_tests.rs`)

**Purpose**: Framework for integration testing with test helpers.

**Implementation Status**:
- ✅ Test helper functions
- ✅ Fixture management
- ✅ End-to-end workflow testing support

#### 23e. Example Generators Module (`src/example_generators.rs`)

**Purpose**: Generate schema examples for documentation and validation.

**Implementation Status**:
- ✅ Receipt example generation (minimal and full)
- ✅ Status example generation (minimal and full)
- ✅ Doctor example generation (minimal and full)
- ✅ Schema validation support

#### 23f. Artifact Module (`src/artifact.rs`)

**Purpose**: Artifact metadata and management.

**Implementation Status**:
- ✅ Artifact metadata structures
- ✅ BLAKE3 hash computation
- ✅ Artifact enumeration

#### 23g. Paths Module (`src/paths.rs`)

**Purpose**: Spec directory path management.

**Implementation Status**:
- ✅ SpecPaths structure for directory layout
- ✅ Path resolution and validation
- ✅ XCHECKER_HOME support

#### 23h. Spec ID Module (`src/spec_id.rs`)

**Purpose**: Spec identifier validation and normalization.

**Implementation Status**:
- ✅ Spec ID validation
- ✅ Normalization rules
- ✅ Error handling for invalid IDs

#### 23i. Types Module (`src/types.rs`)

**Purpose**: Common type definitions used across modules.

**Implementation Status**:
- ✅ Shared type definitions
- ✅ Serialization support
- ✅ Type conversions

#### 23j. Exit Codes Module (`src/exit_codes.rs`)

**Purpose**: Centralized exit code definitions.

**Implementation Status**:
- ✅ Exit code constants
- ✅ Documentation for each code
- ✅ Integration with error module

#### 23k. Error Reporter Module (`src/error_reporter.rs`)

**Purpose**: User-friendly error reporting with actionable suggestions.

**Implementation Status**:
- ✅ Error formatting
- ✅ Actionable suggestions
- ✅ Context information

#### 23l. Logging Module (`src/logging.rs`)

**Purpose**: Structured logging with tracing.

**Implementation Status**:
- ✅ Tracing subscriber setup
- ✅ Env filter configuration
- ✅ Compact and verbose formats
- ✅ Secret redaction in logs

#### 23m. Doctor Module (`src/doctor.rs`)

**Purpose**: System health checks and diagnostics.

**Implementation Status**:
- ✅ Claude CLI detection
- ✅ WSL availability checks
- ✅ Configuration validation
- ✅ Actionable remediation suggestions
- ✅ JSON output support

#### 23n. CLI Module (`src/cli.rs`)

**Purpose**: Command-line interface definition with clap.

**Implementation Status**:
- ✅ All commands (spec, resume, status, clean, doctor, init, benchmark)
- ✅ All global flags
- ✅ Help text and documentation
- ✅ Default values

### 24. Phase Trait System (FR-PHASE) ✅ IMPLEMENTED

**Purpose**: Provide a trait-based system for implementing workflow phases with separated concerns.

**Actual Implementation** (`src/phase.rs`, `src/phases.rs`):

```rust
// Core trait definition
pub trait Phase {
    fn id(&self) -> PhaseId;
    fn deps(&self) -> &'static [PhaseId];
    fn can_resume(&self) -> bool;
    fn prompt(&self, ctx: &PhaseContext) -> String;
    fn make_packet(&self, ctx: &PhaseContext) -> Result<Packet>;
    fn postprocess(&self, raw: &str, ctx: &PhaseContext) -> Result<PhaseResult>;
}

// Supporting types
pub struct PhaseContext {
    pub spec_id: String,
    pub spec_dir: PathBuf,
    pub config: HashMap<String, String>,
    pub artifacts: Vec<String>,
}

pub struct Packet {
    pub content: String,
    pub blake3_hash: String,
    pub evidence: PacketEvidence,
    pub budget_used: BudgetUsage,
}

pub struct PhaseResult {
    pub artifacts: Vec<Artifact>,
    pub next_step: NextStep,
    pub metadata: HashMap<String, String>,
}

pub enum NextStep {
    Continue,
    Rewind { to: PhaseId },
    Complete,
}

// Concrete phase implementations
pub struct RequirementsPhase;
pub struct DesignPhase;
pub struct TasksPhase;
```

**Implementation Status**:
- ✅ Phase trait with separated concerns (prompt, packet, postprocess)
- ✅ PhaseContext for passing runtime information
- ✅ Packet assembly with evidence tracking
- ✅ PhaseResult with artifacts and next step
- ✅ RequirementsPhase implementation
- ✅ DesignPhase implementation with requirements dependency
- ✅ TasksPhase implementation with design dependency
- ✅ Artifact generation (markdown + core YAML)
- ✅ Comprehensive test coverage
- ✅ Integration with orchestrator (wired)
- ✅ Budget enforcement in packet assembly

**Behavioral Contracts**:
- **Determinism**: Given identical `{inputs, config, env, cache}`, `build_packet()` and `postprocess()` produce identical outputs
- **Side-effects**: `postprocess()` performs no I/O beyond artifact writes through the FR-FS atomic writer

**Verification Status**: ✅ Complete - All verification tasks (V3.6, V8.9, V9.2) completed



### 15. Error Module (FR-EXIT) ✅ IMPLEMENTED

**Purpose**: Unified error type with exit code mapping.

**Actual Implementation** (`src/error.rs`):

```rust
#[derive(Debug, Clone, Serialize)]
pub enum ErrorKind {
    CliArgs,
    PacketOverflow,
    SecretDetected,
    LockHeld,
    PhaseTimeout,
    ClaudeFailure,
    Unknown,
}

pub struct XCheckerError {
    pub kind: ErrorKind,
    pub reason: String,
    pub context: Vec<String>,
}

impl XCheckerError {
    pub fn exit_code(&self) -> i32 {
        match self.kind {
            ErrorKind::CliArgs => 2,
            ErrorKind::PacketOverflow => 7,
            ErrorKind::SecretDetected => 8,
            ErrorKind::LockHeld => 9,
            ErrorKind::PhaseTimeout => 10,
            ErrorKind::ClaudeFailure => 70,
            ErrorKind::Unknown => 1,
        }
    }
}
```

**Implementation Status**:
- ✅ Unified error type for all failures
- ✅ Consistent exit code mapping
- ✅ Error kind and reason in receipts
- ✅ Actionable context in error messages
- ✅ User-friendly error reporting
- ✅ Integration with receipt manager

**Implementation Notes**:
- Single error type for all failures
- Consistent mapping to exit codes
- Receipts always include `{exit_code, error_kind, error_reason}`
- Context provides actionable guidance

**Verification Status**: ✅ Complete - All verification tasks (V1.3, V7.1) completed



## Data Flows

### Flow 1: `xchecker spec my-feature` (with Phase Trait System and Multi-Provider LLM)

1. Parse CLI → build `EffectiveConfig` (includes LLM provider config)
2. Resolve source (GitHub/filesystem/stdin) → get problem statement
3. Create `SpecPaths` for `.xchecker/specs/my-feature/`
4. Acquire lock (exit 9 if held)
5. Create LLM backend via factory:
   a. Try primary provider (e.g., "gemini-cli")
   b. If unavailable, try fallback provider if configured
   c. Record fallback usage in warnings if applicable
6. For each phase (Requirements → Design → Tasks):
   a. Get Phase implementation (RequirementsPhase, DesignPhase, or TasksPhase)
   b. Validate dependencies are satisfied
   c. Build PhaseContext with spec_id, spec_dir, config, artifacts
   d. Generate prompt via `phase.prompt(ctx)`
   e. Create packet via `phase.make_packet(ctx)`:
      - Include relevant artifacts from previous phases
      - Check InsightCache for cached file insights
      - Generate new insights if cache miss
      - Track file evidence and budget usage
   f. Scan packet for secrets (exit 8 if found)
   g. Enforce limits (exit 7 if exceeded)
   h. Build LlmInvocation with packet, phase, timeout
   i. Invoke LLM backend with timeout → get LlmResult
   j. Postprocess response via `phase.postprocess(raw, ctx)`:
      - Generate markdown artifact (e.g., 00-requirements.md)
      - Generate core YAML artifact (e.g., 00-requirements.core.yaml)
      - Determine next step (Continue/Rewind/Complete)
   k. Write partial artifacts to `.partial/`
   l. Promote to final (atomic rename)
   m. Write receipt (JCS) with LLM metadata:
      - llm_provider, llm_model, llm_timeout_seconds
      - llm_tokens_input, llm_tokens_output (if available)
      - warnings (if fallback used)
   n. Store insights in InsightCache for future runs
7. Release lock

### Flow 2: `xchecker resume my-feature --phase fixup --apply-fixups`

1. Parse CLI → build `EffectiveConfig`
2. Validate transition (exit 2 if illegal)
3. Acquire lock
4. Load review outputs → derive `FixupPlan`
5. Validate plan (path traversal, symlinks)
6. Apply changes:
   - Write to `.tmp` files
   - fsync
   - Create `.bak` backups
   - Atomic rename (with Windows retry)
7. Write receipt with applied files
8. Release lock

### Flow 3: `xchecker status my-feature --json`

1. Load `EffectiveConfig` (with source attribution)
2. Enumerate artifacts in `artifacts/`
3. Compute `blake3_first8` for each
4. Load lockfile (if exists) and compute drift
5. Build `StatusOutput` struct
6. Emit via JCS
7. Print to stdout

### Flow 4: InsightCache Operation

1. PacketBuilder requests insights for file
2. Compute BLAKE3 hash of file content
3. Check memory cache with key `{hash}_{phase}`
4. If memory hit:
   - Validate file hasn't changed (size, mtime)
   - Return cached insights if valid
   - Invalidate and continue if changed
5. If memory miss, check disk cache
6. If disk hit:
   - Load cached insight from JSON file
   - Validate content hash and file metadata
   - Load into memory cache
   - Return insights if valid
7. If cache miss:
   - Generate phase-specific insights (10-25 bullet points)
   - Store in memory cache
   - Write to disk cache as JSON
   - Return generated insights
8. Track statistics (hits, misses, invalidations, writes)

### Flow 5: Source Resolution

1. Parse CLI source flags (--source, --gh, --repo, or stdin)
2. Determine source type:
   - GitHub: Extract owner, repo, issue_id
   - Filesystem: Validate path exists
   - Stdin: Read from standard input
3. Resolve source:
   - GitHub: Validate issue_id is numeric, fetch content (simulated)
   - Filesystem: Read file or summarize directory
   - Stdin: Validate non-empty input
4. On error:
   - Generate user-friendly error message
   - Provide actionable suggestions
   - Exit with code 2
5. On success:
   - Return SourceContent with content and metadata
   - Include source type and origin information
6. Use content as problem statement for Requirements phase



## Testing Strategy

### Unit Tests (All Implemented ✅)

- **PacketBuilder**: Deterministic ordering, byte/line counting, limit enforcement
- **SecretRedactor**: Pattern matching, redaction, truncation
- **FixupEngine**: Path validation, traversal rejection, symlink handling
- **LockManager**: Stale detection, PID checking, force flag, TTL parameter
- **Canonicalizer**: Byte-identical re-serialization, sorted arrays, RFC 8785 compliance
- **Hash**: BLAKE3 format, stability across platforms
- **Error**: Exit code mapping consistency
- **Runner**: Mode detection, WSL availability, timeout enforcement
- **Orchestrator**: Phase transition validation, state management
- **Config**: Discovery, precedence, source attribution
- **Receipt**: JCS emission, error mapping, optional fields
- **Status**: Effective config, artifact enumeration, drift reporting
- **Benchmark**: Workload generation, memory tracking, threshold comparison
- **InsightCache**: 
  - Cache creation and initialization
  - Cache miss and store operations
  - Cache hit with valid file
  - Cache invalidation on file change
  - Disk cache persistence across instances
  - Insight generation (10-25 bullet points per phase)
  - Phase-specific insight generation (requirements, design, tasks, review)
  - Cache statistics tracking
- **SourceResolver**:
  - GitHub source resolution with validation
  - Filesystem source resolution (files and directories)
  - Stdin source resolution
  - Error handling with user-friendly messages
  - Invalid source configurations
- **Phase Trait System**:
  - Phase ID and dependency tracking
  - Prompt generation with context
  - Packet assembly with artifacts
  - Postprocessing into artifacts
  - Requirements phase basic properties
  - Design phase dependency on Requirements
  - Tasks phase dependency on Design
- **Atomic Write**: Temp file creation, fsync, atomic rename, Windows retry
- **Ring Buffer**: Bounded capacity, overflow handling
- **Process Memory**: RSS measurement, commit MB (Windows)
- **Doctor**: Health checks, WSL detection, actionable suggestions
- **CLI**: Flag parsing, default values, help text

### Future Unit Tests (For LLM Backend - Not Yet Implemented):
- **LLM Backend Abstraction**:
  - LlmBackend trait implementation for each provider
  - LlmInvocation and LlmResult structures
  - Backend factory creation logic
  - Provider selection and fallback logic
- **Gemini CLI Backend**:
  - Binary discovery and validation
  - Argument building for non-interactive mode
  - Model selection (default and per-phase overrides)
  - Integration with existing Runner infrastructure
- **HTTP Client**:
  - Request building and header management
  - Error mapping (4xx, 5xx, network timeout)
  - API key handling (never logged)
  - Response parsing
- **OpenRouter Backend**:
  - OpenAI-compatible request format
  - Header injection (HTTP-Referer, X-Title)
  - Model selection
- **Anthropic Backend**:
  - Anthropic API request format
  - Compatibility with existing prompts

### Integration Tests (All Implemented ✅)

- **Runner**: 
  - AT-RUN-001: Native execution, timeout enforcement, NDJSON merging, stderr redaction ✅
  - AT-RUN-004: Interleaved noise + multiple JSON frames → last valid frame wins ✅
  - AT-RUN-005: Partial JSON followed by timeout → claude_failure with excerpt ✅
  - AT-RUN-006: Large stdout stream (> 2 MiB) → ring buffer retains last segment; receipt contains truncated redacted tail only ✅
- **Orchestrator**: 
  - AT-ORC-001: Phase order validation, atomic promotion, receipt emission ✅
  - AT-ORC-003: Stale .partial/ present → run succeeds; .partial/ cleaned ✅
  - AT-ORC-004: Phase trait system integration with orchestrator ✅
- **Fixup**: 
  - AT-FIX-001: Preview mode (no writes) ✅
  - AT-FIX-002: Apply mode (with .bak), permission preservation ✅
  - AT-FIX-003: Path traversal rejection ✅
- **Status**: Effective config sources, artifact enumeration, drift reporting ✅
- **Security**:
  - AT-SEC-003: Secret appears in error_reason (simulated) → receipt text is redacted ✅
- **Filesystem**:
  - AT-FS-004: Cross-filesystem destination → fallback copy+fsync+replace; receipt warning present ✅
- **WSL** (Windows only): Path translation, Claude detection, distro selection ✅
- **Cache Integration**:
  - AT-CACHE-001: Cache integration with PacketBuilder ✅
  - AT-CACHE-002: Cache hit rate on repeated runs ✅
  - AT-CACHE-003: Cache invalidation on file changes ✅
  - AT-CACHE-004: Performance improvement with caching ✅
- **Source Integration**:
  - AT-SOURCE-001: GitHub source integration with CLI ✅
  - AT-SOURCE-002: Filesystem source integration with CLI ✅
  - AT-SOURCE-003: Stdin source integration with CLI ✅
- **Phase Integration**:
  - AT-PHASE-001: End-to-end Requirements → Design → Tasks flow ✅
  - AT-PHASE-002: Artifact generation and persistence ✅
  - AT-PHASE-003: Phase dependency enforcement ✅

### Future Integration Tests (For LLM Backend - Not Yet Implemented):
- **LLM Backend Integration**:
  - AT-LLM-001: Gemini CLI backend invocation with timeout
  - AT-LLM-002: OpenRouter backend invocation with API key
  - AT-LLM-003: Anthropic backend invocation with API key
  - AT-LLM-004: Provider fallback when primary unavailable
  - AT-LLM-005: Receipt includes LLM metadata (provider, model, tokens)
  - AT-LLM-006: API keys never logged or persisted
  - AT-LLM-007: Per-phase model selection for Gemini CLI
  - AT-LLM-008: Doctor doesn't call real LLMs

### Platform Tests (All Implemented ✅)

- **Windows**: Rename retry ✅, Job Objects ✅, CRLF tolerance ✅, WSL interop ✅
- **Linux/macOS**: Native execution ✅, permission bits ✅, symlink handling ✅

### Smoke Tests (CI) (All Implemented ✅)

```bash
xchecker doctor --json ✅
xchecker init demo --create-lock ✅
xchecker spec demo --dry-run ✅
xchecker status demo --json ✅
xchecker clean demo --hard ✅
xchecker benchmark ✅
```

### Future Smoke Tests (For LLM Backend - Not Yet Implemented):

```bash
# LLM provider smoke tests (skippable via env flag)
XCHECKER_SKIP_LLM_TESTS=0 xchecker spec demo-gemini --llm-provider gemini-cli
XCHECKER_SKIP_LLM_TESTS=0 xchecker spec demo-openrouter --llm-provider openrouter
```



## Observability (FR-OBS)

### Logging

- Use `tracing` crate with structured fields
- Default: compact human-readable format
- `--verbose`: include `spec_id`, `phase`, `duration_ms`, `runner_mode`
- Never log secrets; apply redaction before any output

### Metrics (Benchmark)

- Process RSS (all OSs)
- Commit MB (Windows only)
- Wall time per phase
- Median of N>=3 runs

## Security Considerations

1. **Secret Scanning**: Hard fail before external invocation
2. **Path Validation**: Canonicalize, reject traversal and escapes
3. **Global Redaction**: Apply to stderr, error_reason, context lines, logs, and any user-facing output before persistence or logging
4. **Debug Mode**: `--debug-packet` requires explicit opt-in and successful secret scan; file not cross-linked in receipts
5. **Receipts**: Never include env vars or raw packet content
6. **Symlinks**: Reject by default unless `--allow-links`
7. **File Paths**: Secrets can appear in file paths; treat paths as non-secret but still run redaction on any string copied into receipts/logs
8. **Environment Variables**: Do not log env vars; when showing configuration, show sources and values only for non-sensitive keys; otherwise elide

## Cross-Platform Considerations

### Windows

- Job Objects for process tree termination
- Rename retry with exponential backoff (≤ 250ms)
- CRLF tolerance on read, LF on write
- WSL path translation: `C:\` → `/mnt/c/`

### Linux/macOS

- Process group termination (killpg)
- File mode bit preservation
- Standard POSIX semantics

### WSL

- Detect via `wsl.exe -l -q`
- Validate Claude: `wsl.exe -d <distro> -- which claude`
- Translate paths and env vars
- Artifacts persist in Windows spec root



## Verification & Improvement Plan

### V1 - Implementation Verification ✅ COMPLETE

**Goal**: Verify core infrastructure matches requirements

- ✅ Canonicalization module with RFC 8785 compliance
- ✅ BLAKE3 hashing integrated into canonicalization
- ✅ Error module with exit code mapping
- ✅ Receipt and Status structs with JCS emission
- ✅ Config discovery and precedence
- ✅ Lockfile and drift detection
- ✅ Global CLI flags in `build_cli()`

**Status**: All core modules implemented and functional

**Remaining Work**: Documentation alignment, edge case testing

### V2 - Runner & Packet Verification ✅ COMPLETE

**Goal**: Verify process control and packet assembly

- ✅ PacketBuilder with priority-based selection and limits
- ✅ SecretRedactor with default patterns and redaction
- ✅ Runner with Native/WSL/Auto modes
- ✅ Timeout enforcement with graceful termination
- ✅ Windows Job Objects (conditional compilation)

**Status**: All components implemented and functional

**Remaining Work**: Edge case testing (timeout scenarios, NDJSON merging, packet overflow)

### V3 - Orchestrator & Phase Coordination Verification ✅ COMPLETE

**Goal**: Verify phase execution and state management

- ✅ PhaseOrchestrator with transition validation
- ✅ LockManager with stale detection
- ✅ Atomic write pattern (temp → fsync → rename)
- ✅ ReceiptManager with JCS emission
- ✅ All phase commands wired and functional

**Status**: End-to-end phase execution working

**Remaining Work**: Edge case testing (illegal transitions, lock conflicts, atomic operations)

### V4 - Fixup & Status Verification ✅ COMPLETE

**Goal**: Verify file modification and reporting systems

- ✅ FixupEngine with path validation
- ✅ Preview and apply modes
- ✅ .bak creation and permission preservation
- ✅ StatusManager with effective config
- ✅ All fixup and status commands wired

**Status**: Fixup and status systems fully functional

**Remaining Work**: Edge case testing (path traversal, symlinks, cross-filesystem operations)

### V5 - Platform Support & WSL Verification ✅ COMPLETE

**Goal**: Verify cross-platform support and WSL integration

- ✅ WSL support integrated into Runner
- ✅ WSL detection and Claude validation
- ✅ Windows rename retry logic
- ✅ WSL checks in doctor command
- ✅ CRLF tolerance and line ending normalization

**Status**: Cross-platform support verified on Linux, macOS, Windows

**Remaining Work**: Windows-specific edge case testing (WSL path translation, Job Objects)

### V6 - Performance & Observability Verification ✅ COMPLETE

**Goal**: Verify performance meets NFRs and logging works

- ✅ Benchmark module with process memory tracking
- ✅ Warm-up and median reporting
- ✅ Structured logging with tracing
- ✅ Secret redaction in all log paths
- ✅ runtime-smoke CI job

**Status**: Performance monitoring and logging operational

**Remaining Work**: Verify NFR1 targets met, test logging edge cases

## Risks & Mitigations (Lessons Learned)

| Risk | Mitigation | Status |
|------|------------|--------|
| Stream merge correctness | Golden tests with interleaved frames; abort on invalid JSON; use last valid object | ✅ Implemented & Tested |
| Memory blow-up from large outputs | Ring buffers: 2 MiB stdout, 256 KiB stderr; drain pipes even after timeout | ✅ Implemented & Tested |
| Windows rename quirks | Retry with jitter; backstop with .bak; cross-filesystem fallback to copy+fsync | ✅ Implemented & Tested |
| Path traversal/symlinks | Canonicalize and prefix check; lstat to reject symlinks | ✅ Implemented & Tested |
| Secret false positives | Expose `--ignore-secret-pattern`; log which rule fired | ✅ Implemented & Tested |
| Stale lock handling | PID/host/timestamp in lock; configurable TTL; `--force` to break | ✅ Implemented & Tested |
| WSL Claude missing | Validate with `which claude`; doctor provides guidance; use `wslpath` for translation | ✅ Implemented & Tested |
| Benchmark flakiness | Warm-up + median of N>=3; document CPU scaling caveats in README | ✅ Implemented & Tested |
| Schema drift | Confirm optional fields in v1 schemas; regenerate examples; CI diff check | ✅ Implemented & Tested |
| Cache invalidation bugs | File size + mtime checks; BLAKE3 content hash; fail-open on corruption | ✅ Implemented & Tested |
| Phase dependency violations | Explicit dependency tracking in Phase trait; validation before execution | ✅ Implemented & Tested |
| Packet overflow edge cases | Upstream file budget checking; manifest writing; fail before Claude invocation | ✅ Implemented & Tested |

## Traceability Matrix

Quick reference for mapping requirements to implementation and tests:

| FR | Repo Surface | Must-Have Tests |
|----|--------------|-----------------|
| FR-RUN | runner.rs | AT-RUN-004 (last_valid_json_wins), AT-RUN-005 (partial_json_timeout), AT-RUN-006 (stdout_ring_buffer), AT-RUN-007 (stderr_redaction_truncation), AT-RUN-010 (timeout_job_objects_unixpg) |
| FR-ORC | orchestrator.rs | AT-ORC-003 (stale_partial_cleanup), AT-ORC-004 (atomic_promotion) |
| FR-PKT | packet.rs | AT-PKT-004 (manifest_written), AT-PKT-006 (debug_packet_rules) |
| FR-SEC | redaction.rs | AT-SEC-003 (redact_in_errors_and_logs) |
| FR-STA | status.rs | AT-STA-004 (pending_fixups_counts) |
| FR-WSL | runner.rs | AT-WSL-001 (wslpath_translation), AT-WSL-002 (argv_passthrough_no_shell) |
| FR-CACHE | cache.rs | AT-CACHE-002 (ttl_expiry_miss), AT-CACHE-003 (atomic_writes_redacted_strings) |
| FR-SOURCE | source.rs | AT-SRC-001 (dedupe_and_excludes), AT-SRC-002 (caps_before_packet) |
| FR-PHASE | phase.rs, phases.rs | AT-PHASE-001 (deterministic_outputs), AT-PHASE-002 (no_side_effects_postprocess) |
| FR-SCHEMA | schemas/* | AT-SCHEMA-001 (receipts_validate), AT-SCHEMA-002 (status_validate), AT-SCHEMA-003 (examples_regenerated) |
| FR-CLI | clap config | AT-CLI-001 (help_has_defaults_units), AT-CLI-002 (flags_wire_to_config) |
| FR-LLM | llm/mod.rs, llm/factory.rs | AT-LLM-004 (fallback_logic), AT-LLM-005 (receipt_metadata) |
| FR-LLM-CLI | llm/gemini_cli.rs, llm/claude_cli.rs | AT-LLM-001 (gemini_invocation), AT-LLM-007 (per_phase_models) |
| FR-LLM-GEM | llm/gemini_cli.rs | AT-LLM-001 (gemini_invocation), AT-LLM-007 (per_phase_models) |
| FR-LLM-API | llm/http_client.rs | AT-LLM-006 (api_keys_never_logged) |
| FR-LLM-OR | llm/openrouter.rs | AT-LLM-002 (openrouter_invocation) |
| FR-LLM-ANTH | llm/anthropic.rs | AT-LLM-003 (anthropic_invocation) |
| FR-LLM-META | receipt.rs | AT-LLM-005 (receipt_metadata) |

## Verification & Improvement Focus

### Current State Assessment

**Implemented ✅:**
- All core modules (canonicalization, redaction, runner, orchestrator, packet, fixup, lock, status, receipt, config)
- End-to-end phase execution (Requirements → Design → Tasks → Review → Fixup → Final)
- Cross-platform support (Linux, macOS, Windows, WSL)
- Security controls (secret redaction, path validation)
- Performance monitoring (benchmarks, logging)
- Comprehensive error handling with standardized exit codes
- JCS canonical emission for all JSON outputs
- Additional features (InsightCache, source resolution, phase trait system, integration test framework, example generators)
- All supporting modules (atomic_write, ring_buffer, process_memory, artifact, paths, spec_id, types, exit_codes, error_reporter, logging, doctor, cli)
- NDJSON framing with last valid JSON frame selection
- Timeout enforcement with tokio and process tree termination
- Ring buffers for stdout/stderr capture
- Windows Job Objects and Unix killpg for process termination
- PacketBuilder wired into orchestrator via Phase trait system
- Manifest writing on packet overflow
- --debug-packet flag implementation
- Windows retry logic and cross-filesystem fallback
- WSL path and environment translation
- InsightCache integration with PacketBuilder
- All cleanup tasks (removed dead code, TODOs, fixed clippy warnings)
- All optimization tasks (packet assembly, JCS emission, dry-run performance)

**Needs Implementation 🔨:**
- **LLM Backend Abstraction**: Multi-provider support (FR-LLM) - V11.0-V11.14
  - ExecutionStrategy layer (Controlled vs ExternalTool)
  - LlmBackend trait and factory
  - ClaudeCliBackend (refactor existing Runner)
  - GeminiCliBackend for Gemini CLI support
  - HTTP client module for API-based providers
  - OpenRouter backend
  - Anthropic API backend
  - Provider metadata in receipts
  - Fallback provider support
  - Call budget enforcement for API providers
  - XCHECKER_SKIP_LLM_TESTS gating

**Completed Verification ✅:**
- Core infrastructure (V1) - All tasks complete
- Runner & Packet (V2) - All tasks complete
- Orchestrator & Phase Coordination (V3) - All tasks complete
- Fixup & Status (V4) - All tasks complete
- Platform Support & WSL (V5) - All tasks complete
- Performance & Observability (V6) - All tasks complete
- Edge Cases & Error Handling (V7) - All tasks complete
- Integration & Smoke Tests (V8) - All tasks complete
- Cleanup & Optimization (V9) - All tasks complete

**Needs Documentation 📚:**
- Update CHANGELOG with all features (V10.1) - ✅ Complete
- Update README to match implementation (V10.2) - ✅ Complete
- Update design document (V10.3) - 🔄 In Progress
- Update schema files with optional fields (V10.4) - Pending
- Create additional docs (PERFORMANCE.md, SECURITY.md, PLATFORM.md) (V10.5) - Pending
- Final verification suite (V10.6) - Pending
- Verify all requirements met (V10.7) - Pending
- Verify all NFRs met (V10.8) - Pending
- Prepare release (V10.9) - Pending
- Mark spec complete (V10.10) - Pending

### Additional Modules Beyond Original Plan

The implementation includes several modules not in the original design. These are now fully documented in the Module Design section above (see sections 16, 17, 23, and 24):

1. **InsightCache** (`src/cache.rs`): BLAKE3-keyed cache for file insights with TTL and validation (FR-CACHE) - See section 16
   - Two-tier caching (memory + disk)
   - File change detection and automatic invalidation
   - Phase-specific insight generation (10-25 bullet points)
   - Cache statistics tracking

2. **SourceResolver** (`src/source.rs`): Multi-source problem statement resolution (FR-SOURCE) - See section 17
   - GitHub source resolution
   - Filesystem source resolution
   - Stdin source resolution
   - User-friendly error messages

3. **Phase Trait System** (`src/phase.rs`, `src/phases.rs`): Trait-based workflow phases (FR-PHASE) - See section 24
   - Separated concerns (prompt, packet, postprocess)
   - Dependency tracking
   - Artifact generation
   - Deterministic outputs

4. **Supporting Modules** - See section 23
   - `atomic_write.rs` - Atomic file operations
   - `ring_buffer.rs` - Bounded circular buffers
   - `process_memory.rs` - Process memory tracking
   - `integration_tests.rs` - Integration testing framework
   - `example_generators.rs` - Schema example generation
   - `artifact.rs` - Artifact metadata
   - `paths.rs` - Spec directory paths
   - `spec_id.rs` - Spec identifier validation
   - `types.rs` - Common type definitions
   - `exit_codes.rs` - Exit code constants
   - `error_reporter.rs` - User-friendly error reporting
   - `logging.rs` - Structured logging
   - `doctor.rs` - System health checks
   - `cli.rs` - Command-line interface

2. **SourceResolver** (`src/source.rs`): Multi-source support for problem statements (FR-SOURCE)
   - GitHub source resolution (owner/repo/issue)
   - Filesystem source resolution (files and directories)
   - Stdin source resolution
   - User-friendly error messages with suggestions

3. **Phase Trait System** (`src/phase.rs`, `src/phases.rs`): Trait-based phase implementation (FR-PHASE)
   - Separated concerns (prompt, packet, postprocess)
   - Dependency tracking between phases
   - Concrete implementations (Requirements, Design, Tasks)
   - Artifact generation (markdown + core YAML)

4. **Integration Tests** (`src/integration_tests.rs`): Framework for end-to-end testing
   - Test harness for full workflow execution
   - Fixture management and cleanup

5. **Example Generators** (`src/example_generators.rs`): Schema example generation for documentation
   - Automated example generation from schemas
   - Documentation consistency validation

## Definition of Done

**Core Implementation ✅:**
- [x] All FR requirements implemented
- [x] All core modules functional (canonicalization, redaction, runner, orchestrator, packet, fixup, lock, status, receipt, config)
- [x] End-to-end execution working (Requirements → Design → Tasks → Review → Fixup → Final)
- [x] Cross-platform support implemented (Linux, macOS, Windows, WSL)
- [x] Security controls operational (secret redaction, path validation)
- [x] Error handling with standardized exit codes
- [x] Additional features (InsightCache, source resolution, integration test framework)

**Verification & Testing 🔍:**
- [ ] All FR requirements verified against implementation (V1-V7)
- [ ] Unit tests green on Linux/macOS/Windows (V8.4)
- [ ] Integration tests green on Linux/macOS/Windows (V8.4)
- [ ] Platform-specific tests green (WSL on Windows, Job Objects, killpg) (V5, V8.4)
- [ ] Edge case tests for all modules (V7)
- [ ] Error path testing complete (V7.1, V7.2)
- [ ] Timeout scenarios tested (V2.4, V7.4)
- [ ] Packet overflow scenarios tested (V7.3)
- [ ] Secret detection scenarios tested (V7.2, V7.6)
- [ ] Concurrent execution scenarios tested (V7.5)
- [ ] End-to-end workflow tests (V8.6)
- [ ] `runtime-smoke` CI job passes (V8.1)
- [ ] `docs-conformance` CI job passes (V8.5)

**Integration & Wiring 🔌:**
- [ ] PacketBuilder wired into orchestrator (V9.2)
- [ ] InsightCache wired into PacketBuilder (V9.3)
- [ ] All TODOs removed from staged modules (V9.1)
- [ ] All `#![allow(dead_code, unused_imports)]` removed (V9.1)
- [ ] PacketEvidence populated in receipts (V9.2)
- [ ] Secret scanning integrated before Claude invocation (V9.2)

**Quality & Performance ⚡:**
- [ ] Performance benchmarks meet NFR1 targets (V6.5, V9.4, V9.5, V9.6)
  - [ ] `spec --dry-run` baseline ≤ 5s
  - [ ] Packetization of 100 files ≤ 200ms
  - [ ] JCS emission ≤ 50ms
- [ ] Receipts and status are JCS-canonical and schema-valid (V8.2)
- [ ] Arrays sorted (artifacts by path, checks by name) (V1.1)
- [ ] blake3 hashes populated for all artifacts (V1.2)
- [ ] No secrets in logs, receipts, or artifacts (except explicit `--debug-packet`) (V7.2, V7.6)
- [ ] Code coverage >90% (V9.7)
- [ ] All clippy warnings fixed (V9.9)
- [ ] Code quality improvements complete (V9.9)

**Documentation 📚:**
- [ ] CHANGELOG updated for all features (V10.1)
- [ ] README updated and accurate (V10.2)
- [ ] Design document reflects actual implementation (V10.3)
- [ ] Schema files updated with optional fields (V10.4)
- [ ] Additional documentation created (PERFORMANCE.md, SECURITY.md, PLATFORM.md) (V10.5)
- [ ] Examples regenerated and committed (V8.2)
- [ ] All documentation validated (V8.5)

**Final Verification ✅:**
- [ ] All requirements verified (V10.7)
- [ ] All NFRs met (V10.8)
- [ ] All documentation updated (V10.1-V10.5)
- [ ] All tests passing (V10.6)
- [ ] CI green on all platforms (V10.6)
- [ ] Performance targets met (V10.8)
- [ ] Security controls verified (V10.8)
- [ ] Release prepared (V10.9)
- [ ] Ready for production use (V10.10)


---

# V11–V18 Roadmap: Multi-Provider LLM & Ecosystem Expansion

## Overview

After completing the core runtime implementation (V1–V10), the next phase focuses on expanding xchecker's capabilities through multi-provider LLM support and ecosystem integration. This roadmap is organized into 8 phases, each building on the previous one.

**Key Principles:**
- **Walking Skeleton Approach**: Each phase delivers a complete, working slice
- **Controlled Execution**: All LLM outputs go through FixupEngine + atomic writes (no direct file modification)
- **Provider Abstraction**: Single `LlmBackend` trait hides transport details (CLI vs HTTP)
- **Cost Control**: Budget enforcement for HTTP providers (OpenRouter, Anthropic)
- **Compression**: Receipts and status JSON enable Claude Code to work with tiny contexts

## V11 – LLM Core Skeleton & Claude Backend (MVP+)

**Goal**: Put the existing Runner behind a clean LlmBackend abstraction, keep Controlled writes, and wire basic LLM metadata into receipts.

### Architecture

```
Orchestrator
├── ExecutionStrategy (Controlled-only in V11)
├── LlmBackend trait
│   ├── ClaudeCliBackend (wraps existing Runner)
│   ├── GeminiCliBackend (V12)
│   ├── OpenRouterBackend (V13)
│   └── AnthropicBackend (V14)
├── LlmBackendFactory
│   └── create(config) → Box<dyn LlmBackend>
└── Receipt.llm (populated with provider, model, tokens, timeout)
```

### Key Components

**ExecutionStrategy Enum:**
```rust
pub enum ExecutionStrategy {
    Controlled,      // LLM proposes; FixupEngine applies
    ExternalTool,    // Stub only; returns Unsupported error
}
```

**LlmBackend Trait:**
```rust
pub struct LlmInvocation<'a> {
    pub spec_id: &'a str,
    pub phase_id: PhaseId,
    pub prompt: String,
    pub timeout: Duration,
    pub model: Option<String>,
}

pub struct LlmResult {
    pub provider: String,
    pub model_used: Option<String>,
    pub raw_response: String,
    pub stderr_tail: Option<String>,
    pub timed_out: bool,
    pub tokens_input: Option<u32>,
    pub tokens_output: Option<u32>,
}

#[async_trait::async_trait]
pub trait LlmBackend: Send + Sync {
    async fn invoke(&self, inv: LlmInvocation<'_>) -> Result<LlmResult, RunnerError>;
}
```

**LlmBackendFactory:**
```rust
pub enum BackendKind {
    ClaudeCli,
    GeminiCli,
    OpenRouterApi,
    AnthropicApi,
}

pub struct LlmBackendFactory;

impl LlmBackendFactory {
    pub fn create(cfg: &BackendConfig) -> Result<Box<dyn LlmBackend>, XCheckerError>;
}
```

### Configuration

```toml
[llm]
provider = "claude-cli"              # default in V11
execution_strategy = "controlled"    # only valid value in V11

[llm.claude]
binary = "/usr/local/bin/claude"     # optional; defaults to $PATH
default_model = "claude-3-5-sonnet"  # optional
```

### CLI Flags

- `--llm-provider <provider>` — Override provider (V11: only "claude-cli")
- `--llm-model <model>` — Override model
- `XCHECKER_LLM_PROVIDER` — Environment variable override

### Doctor Checks

- Verify `[llm] provider` is valid
- Check if binary exists (e.g., `which claude`)
- Report version (best-effort)
- Suggest remediation if binary missing

### Receipt Metadata

```json
{
  "llm": {
    "provider": "claude-cli",
    "model_used": "claude-3-5-sonnet",
    "tokens_input": 1234,
    "tokens_output": 567,
    "timed_out": false
  }
}
```

## V12 – Gemini CLI as First-Class Provider

**Goal**: Add Gemini CLI as the default CLI backend, with Claude as optional/fallback.

### Key Changes

- Gemini CLI becomes default provider
- Claude CLI becomes optional fallback
- Both `[llm.gemini]` and `[llm.claude]` sections fully parsed
- Doctor reports both providers' availability

### Configuration

```toml
[llm]
provider = "gemini-cli"              # new default
fallback_provider = "claude-cli"     # optional fallback
execution_strategy = "controlled"

[llm.gemini]
binary = "/usr/local/bin/gemini"
default_model = "gemini-2.0-flash-lite"
model_requirements = "gemini-2.0-flash"  # per-phase override
model_design = "gemini-2.0-flash"
model_tasks = "gemini-2.0-flash-lite"

[llm.claude]
binary = "/usr/local/bin/claude"
default_model = "claude-3-5-sonnet"
```

### Gemini CLI Invocation

```bash
gemini -p "<prompt>" --model gemini-2.0-flash-lite
```

- Non-interactive mode only
- Text-only output (no tools/filesystem access)
- Stdout treated as opaque text (not NDJSON)
- Stderr captured with redaction and 2 KiB cap

## V13 – HTTP Client & OpenRouter Backend (Optional)

**Goal**: Add a single HTTP path (OpenRouter) that can be enabled when you want it, with clear budgets.

### Key Components

**HttpClient:**
```rust
pub struct HttpClient {
    client: reqwest::Client,
    timeout: Duration,
}

impl HttpClient {
    pub async fn post(&self, url: &str, body: &str, headers: &[(String, String)]) 
        -> Result<String, HttpError>;
}
```

**OpenRouterBackend:**
```rust
pub struct OpenRouterBackend {
    http_client: HttpClient,
    base_url: String,
    api_key: String,
    model: String,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
    call_budget: u32,
    calls_made: AtomicU32,
}
```

### Configuration

```toml
[llm]
provider = "openrouter"

[llm.openrouter]
base_url = "https://openrouter.ai/api/v1/chat/completions"
api_key_env = "OPENROUTER_API_KEY"
model = "google/gemini-2.0-flash-lite"
max_tokens = 4096
temperature = 0.7
call_budget = 20  # default; override via XCHECKER_OPENROUTER_BUDGET
```

### Cost Control

- Default budget: 20 calls per run
- Overridable via `XCHECKER_OPENROUTER_BUDGET` env var
- Exit code 70 if budget exceeded
- Receipt includes call count and budget

### Doctor Checks

- Verify `OPENROUTER_API_KEY` env var is set
- Do NOT send HTTP request (opt-in only)

## V14 – Anthropic HTTP, Rich Metadata & Provider Docs

**Goal**: Add Anthropic HTTP backend and finish the metadata + docs story.

### AnthropicBackend

```rust
pub struct AnthropicBackend {
    http_client: HttpClient,
    base_url: String,
    api_key: String,
    model: String,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
}
```

### Configuration

```toml
[llm]
provider = "anthropic"

[llm.anthropic]
base_url = "https://api.anthropic.com/v1/messages"
api_key_env = "ANTHROPIC_API_KEY"
model = "claude-3-5-sonnet-20241022"
max_tokens = 4096
temperature = 0.7
```

### Rich Metadata

- `llm.tokens_input` and `llm.tokens_output` from provider usage
- `llm.provider` (e.g., "anthropic", "openrouter", "gemini-cli", "claude-cli")
- `llm.model_used` (actual model name/version)
- `llm.timed_out` (boolean)

### Documentation

**docs/LLM_PROVIDERS.md:**
- Gemini CLI: setup, config, cost, authentication
- Claude CLI: setup, config, cost, authentication
- OpenRouter: setup, config, cost, budget control, supported models
- Anthropic: setup, config, cost, authentication
- Comparison table (cost, speed, quality, local vs cloud)
- Test gating: `XCHECKER_SKIP_LLM_TESTS`, `XCHECKER_USE_OPENROUTER`

## V15 – Claude Code (Claude Code) Integration & UX

**Goal**: Make it trivial to trigger xchecker phases from Claude Code.

### CLI Enhancements

**New Commands:**
```bash
xchecker spec <spec-id> --json          # Emit spec metadata
xchecker status <spec-id> --json        # Emit compact status
xchecker resume <spec-id> --phase <phase> --json  # Emit next step summary
```

**JSON Shapes:**

```json
{
  "spec_id": "my-feature",
  "current_phase": "design",
  "is_complete": false,
  "pending_fixups": { "targets": 3, "est_added": 50, "est_removed": 10 },
  "last_run": "2025-11-30T03:12:00Z",
  "llm": { "provider": "gemini-cli", "model": "gemini-2.0-flash-lite" }
}
```

### Claude Code Flows

**Example Flow:**
1. User runs `/xchecker spec my-feature` in Claude Code
2. Claude Code receives compact JSON with current phase and pending fixups
3. Claude Code decides next step (e.g., "run design phase")
4. Claude Code calls `xchecker resume my-feature --phase design --json`
5. xchecker returns compact summary for agent consumption
6. Claude Code processes result and suggests next action

### Slash Command UX

```
/xchecker spec <spec-id> [source...]
/xchecker status <spec-id>
/xchecker resume <spec-id> --phase <phase>
```

## V16 – Workspace & Multi-Spec Orchestration

**Goal**: Move from "one spec at a time" to a workspace view.

### New Commands

```bash
xchecker project init <name>                          # Create workspace registry
xchecker project add-spec <spec-id> --tag <tag>      # Register spec
xchecker project list                                 # List all specs
xchecker project status [--json]                      # Aggregated status
xchecker project history <spec-id> [--json]          # Timeline
xchecker project tui                                  # Terminal UI (optional)
```

### Workspace Registry

**workspace.yaml:**
```yaml
workspace: my-repo
specs:
  - spec_id: billing-api
    tags: [api, critical]
    status: design
    last_run: 2025-11-30T03:12:00Z
  - spec_id: auth-rewrite
    tags: [auth, frontend]
    status: tasks
    last_run: 2025-11-29T10:00:00Z
```

### Project Status Output

```json
{
  "workspace": "my-repo",
  "specs": [
    {
      "spec_id": "billing-api",
      "latest_phase": "design",
      "is_complete": false,
      "last_run": "2025-11-30T03:12:00Z",
      "llm": { "provider": "gemini-cli", "model": "gemini-2.0-flash-lite" },
      "pending_fixups": { "total_targets": 3, "total_hunks": 7 }
    }
  ]
}
```

### Project History

```json
{
  "spec_id": "billing-api",
  "timeline": [
    { "phase": "requirements", "duration_ms": 45000, "exit_code": 0 },
    { "phase": "design", "duration_ms": 120000, "exit_code": 0 },
    { "phase": "tasks", "duration_ms": 90000, "exit_code": 0 }
  ],
  "metrics": {
    "p95_phase_time_ms": 120000,
    "fixup_rounds": 2,
    "error_kinds": []
  }
}
```

## V17 – Policy & Enforcement ("Double-Entry SDLC" in CI)

**Goal**: Turn xchecker's receipts + status into enforceable gates.

### Gate Command

```bash
xchecker gate <spec-id> [--policy <path>] [--json]
```

**Policy Options:**
```bash
--min-phase tasks                    # Require at least tasks phase
--fail-on-pending-fixups             # Fail if any fixups pending
--max-phase-age 7d                   # Fail if phase older than 7 days
```

**Exit Codes:**
- 0: All gates pass
- Non-zero: Gate failed (structured error for CI)

### CI Templates

**.github/workflows/xchecker-gate.yml:**
```yaml
name: xchecker-gate
on: [pull_request]
jobs:
  gate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - run: cargo install xchecker
      - run: xchecker gate my-spec --min-phase tasks
```

### Policy as Code (Future)

```toml
[gate]
require_phase = "tasks"
allow_fixups = false
max_age_days = 7
```

## V18 – Ecosystem & Templates (Batteries Included)

**Goal**: Turn xchecker into something a team can adopt in an afternoon.

### Spec Templates

```bash
xchecker template list
xchecker template init fullstack-nextjs my-feature
```

**Available Templates:**
- `fullstack-nextjs` — Next.js + API + database
- `rust-microservice` — Rust service with tests
- `python-fastapi` — FastAPI + async
- `docs-refactor` — Documentation improvements

### Plugin Hooks

**hooks.toml:**
```toml
[hooks.pre_phase]
command = "scripts/xchecker-pre-phase.sh"

[hooks.post_phase]
command = "scripts/xchecker-post-phase.sh"
```

**Use Cases:**
- Slack notifications on phase completion
- Dashboard sync (internal metrics)
- Prometheus metrics emission
- Git commit on successful fixup

### Showcase Examples

**examples/fullstack-nextjs/:**
- Minimal app skeleton
- Scripted workflow (Requirements → Design → Tasks → Fixup)
- README with xchecker outputs and receipts

**examples/mono-repo/:**
- Multiple spec IDs mapping to sub-services
- Workspace commands in action

**Walkthroughs:**
- "Running xchecker on your repo in 20 minutes"
- "From spec to PR: xchecker + Claude Code flow"

## Implementation Roadmap Summary

| Phase | Goal | Key Deliverables | Timeline |
|-------|------|------------------|----------|
| V11 | LLM Core Skeleton | LlmBackend trait, ClaudeCliBackend, ExecutionStrategy | 2–3 weeks |
| V12 | Gemini CLI | GeminiCliBackend, config parsing, doctor checks | 1–2 weeks |
| V13 | HTTP + OpenRouter | HttpClient, OpenRouterBackend, budget control | 2–3 weeks |
| V14 | Anthropic + Docs | AnthropicBackend, rich metadata, LLM_PROVIDERS.md | 1–2 weeks |
| V15 | Claude Code UX | JSON shapes, slash commands, example flows | 2–3 weeks |
| V16 | Workspace | Project commands, registry, history, TUI | 3–4 weeks |
| V17 | Policy & CI | Gate command, CI templates, policy as code | 2–3 weeks |
| V18 | Ecosystem | Templates, hooks, showcase examples | 2–3 weeks |

**Total Estimated Effort:** 15–23 weeks (3–5 months)

## Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| LLM provider API changes | Abstract behind trait; version-pin dependencies |
| Cost overruns (OpenRouter) | Strict budget enforcement; default low limits |
| Complexity explosion | Walking skeleton approach; one provider at a time |
| CI integration friction | Provide ready-to-copy templates; document thoroughly |
| User adoption | Showcase examples; templates; comprehensive docs |

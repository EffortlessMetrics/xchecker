# Requirements Document

## Introduction

xchecker is a Rust cargo crate that provides a CLI tool for orchestrating spec generation workflows using the Claude CLI. It acts as a subagent router that manages the flow from requirements to design to task lists, with review and fixup passes, while maintaining token efficiency through disk-first context management and deterministic receipts.

## Requirements

### Requirement 1

**User Story:** As a developer, I want to run a single command to generate a complete spec flow, so that I can transform rough ideas into detailed implementation plans without manual orchestration.

#### Acceptance Criteria

1. WHEN I run `xchecker spec <id>` THEN the system SHALL execute the complete flow: Requirements → Design → Tasks → Reviews → Fixups → Final
2. WHEN the flow completes THEN the system SHALL have created all phase artifacts in `.xchecker/specs/<id>/artifacts/`
3. WHEN any phase fails THEN the system SHALL persist partial outputs and provide clear error messages
4. WHEN I specify `--dry-run` THEN the system SHALL show what would be executed without making Claude calls

### Requirement 2

**User Story:** As a developer, I want deterministic and auditable spec generation, so that I can track what inputs produced what outputs and reproduce results.

#### Acceptance Criteria

1. WHEN each phase completes THEN the system SHALL write a receipt with BLAKE3 hashes of inputs and outputs
2. WHEN the packet_blake3, phase prompt, model full name, and CLI version are identical THEN the system SHALL produce identical BLAKE3 hashes for the canonicalized outputs of that phase (YAML keys sorted; markdown normalized). Receipts MUST record canonicalization version
3. WHEN "same inputs" are defined THEN they SHALL mean identical packet hash + identical phase prompt + identical model+version + equal flags
4. WHEN canonicalizing outputs THEN the system SHALL require structure determinism, not verbatim text: canonicalize `*.core.yaml` (sorted keys, trimmed whitespace), and normalize markdown (heading set, fenced blocks)
5. WHEN identical packet hash + same model/version are provided THEN the system SHALL produce identical canonicalized outputs
6. WHEN I run `xchecker status <id>` THEN the system SHALL show the latest completed phase, list of artifacts with first-8 BLAKE3, and last receipt path
7. WHEN receipts are written THEN they SHALL include model full name, flags, timestamps, stderr tails, and canonicalization version

### Requirement 3

**User Story:** As a developer, I want token-efficient context management, so that I can minimize Claude API costs while maintaining necessary context.

#### Acceptance Criteria

1. WHEN building packets THEN the system SHALL enforce packet budgets: packet_max_bytes (default 65536) and packet_max_lines (default 1200)
2. WHEN selecting content THEN upstream `*.core.yaml` SHALL never be evicted; selection order SHALL be highest-priority excerpts → lower-priority until budget; evict tail first
3. WHEN packet size would exceed limits THEN the system SHALL fail fast before invoking Claude, writing context/<phase>-packet.txt preview and a receipt with exit_code!=0
4. WHEN a file hasn't changed THEN the system SHALL reuse cached core insights based on BLAKE3 keys
5. WHEN summarizing content THEN the system SHALL create 10-25 bullet core insights per phase

### Requirement 4

**User Story:** As a developer, I want separate Claude sessions per phase, so that context doesn't accumulate unnecessarily and I can resume specific phases.

#### Acceptance Criteria

1. WHEN executing each phase THEN the system SHALL use separate Claude CLI invocations
2. WHEN I run `xchecker resume <id> --phase <name>` THEN the system SHALL restart from that phase using existing artifacts
3. WHEN a phase fails with non-zero exit THEN the system SHALL save partial stdout as `artifacts/<nn>-<phase>.partial.md`, include stderr_tail (≤2 KiB) and warnings array in receipt, and stop the flow
4. WHEN calling Claude THEN the system SHALL use non-interactive mode with stream-json output format
5. WHEN resuming a phase THEN the system SHALL delete partials on success and promote to final filenames
6. WHEN rerunning `resume --phase DESIGN` THEN the system MUST NOT modify earlier receipts/artifacts unless FIXUPS requires rewind

### Requirement 5

**User Story:** As a developer, I want structured review and fixup capabilities with preview/apply modes, so that gaps in earlier phases can be addressed systematically and safely.

#### Acceptance Criteria

1. WHEN the review phase detects gaps THEN the system SHALL signal the need for fixups with explicit markers ("FIXUP PLAN:" or "needs fixups")
2. WHEN fixups are needed THEN the system SHALL produce unified diffs per file in fenced ```diff blocks with ---/+++ headers; validate with git apply --check (no --unidiff-zero by default)
3. WHEN generating fixups THEN at least one fenced block per target file SHALL be provided with proper diff format
4. WHEN fixups are in preview mode (default) THEN the system SHALL parse & validate unified diffs, list targets, but make no writes
5. WHEN `--apply-fixups` flag is set THEN the system SHALL run git apply --check first, then apply changes and list applied files in receipt
6. WHEN status runs THEN it SHALL list intended targets before apply and record preview in receipt

### Requirement 6

**User Story:** As a developer, I want configurable source resolution, so that I can work with GitHub issues, local files, or stdin input.

#### Acceptance Criteria

1. WHEN I specify `--source gh --gh owner/repo` THEN the system SHALL resolve the ID as a GitHub issue number
2. WHEN I specify `--source fs --repo <path>` THEN the system SHALL use local filesystem context
3. WHEN I specify `--source stdin` THEN the system SHALL read the problem statement from stdin
4. WHEN source resolution fails THEN the system SHALL provide clear error messages with suggested alternatives

### Requirement 7

**User Story:** As a developer, I want comprehensive CLI configuration options, so that I can control model selection, tool permissions, and execution parameters.

#### Acceptance Criteria

1. WHEN I specify `--model <alias|full>` THEN the system SHALL use that model for all Claude calls
2. WHEN I specify `--allow` or `--deny` tool patterns THEN the system SHALL pass these to Claude as allowedTools/disallowedTools
3. WHEN I specify `--max-turns <n>` THEN the system SHALL limit Claude interactions to that number
4. WHEN I specify `--dangerously-skip-permissions` THEN the system SHALL pass this flag to Claude
5. WHEN I specify `--verbose` THEN the system SHALL provide detailed logging of all operations

### Requirement 8

**User Story:** As a developer, I want artifact management capabilities, so that I can clean up old specs and understand the current state.

#### Acceptance Criteria

1. WHEN I run `xchecker clean <id>` THEN the system SHALL prompt for confirmation before removing artifacts
2. WHEN I run `xchecker clean <id> --hard` THEN the system SHALL remove artifacts without confirmation
3. WHEN artifacts are created THEN they SHALL follow the naming convention: `<nn>-<phase>.md` and `<nn>-<phase>.core.yaml`
4. WHEN the directory structure is created THEN it SHALL include `artifacts/`, `receipts/`, and `context/` subdirectories

### Requirement 9

**User Story:** As a developer, I want security and privacy protection, so that sensitive information doesn't leak into packets or receipts.

#### Acceptance Criteria

1. WHEN building packets THEN the redactor SHALL block the following default patterns: `ghp_[A-Za-z0-9]{36}`, `AKIA[0-9A-Z]{16}`, `AWS_SECRET_ACCESS_KEY=`, `xox[baprs]-`, `Bearer [A-Za-z0-9._-]{20,}`
2. WHEN any match is found in packet/receipt THEN the system SHALL abort the run with a clear report unless `--allow-secret-pattern "<regex>"` is provided
3. WHEN writing receipts THEN the system SHALL NOT include environment variables
4. WHEN processing files THEN the system SHALL restrict to project tree by default
5. WHEN external paths are needed THEN the system SHALL require explicit opt-in flags

### Requirement 10

**User Story:** As a developer, I want extensible phase architecture, so that new flows can be added beyond spec generation.

#### Acceptance Criteria

1. WHEN implementing phases THEN they SHALL follow a consistent trait-based interface
2. WHEN adding new flows THEN they SHALL reuse the same orchestration, receipts, and artifact patterns
3. WHEN phases need to rewind THEN they SHALL be able to signal upstream changes through the NextStep enum
4. WHEN new phases are added THEN they SHALL integrate with the existing packet and squeeze systems

### Requirement 11

**User Story:** As a developer, I want configuration management with discovery and precedence, so that I can set defaults and customize behavior without CLI flags.

#### Acceptance Criteria

1. WHEN `.xchecker/config.toml` exists THEN the system SHALL discover it by searching from CWD upward and load `[defaults]`, `[selectors]`, and `[runner]` sections
2. WHEN CLI flags are provided THEN they SHALL override config file values which override built-in defaults
3. WHEN `xchecker status <id>` runs THEN it SHALL show effective configuration with source attribution (CLI > config > defaults)
4. WHEN config includes `[selectors]` THEN the system SHALL use include/exclude globs for context selection
5. WHEN `--config <path>` is specified THEN the system SHALL use that explicit path instead of discovery

### Requirement 12

**User Story:** As a Windows developer, I want automatic WSL detection and Claude execution, so that I can use xchecker seamlessly across Windows and WSL environments.

#### Acceptance Criteria

1. WHEN running on Windows with RunnerMode::Auto THEN the system SHALL detect claude in Windows PATH first, else check WSL availability
2. WHEN WSL is detected via `wsl -e claude --version` returning 0 THEN the system SHALL use `wsl.exe --exec <claude_bin>` with argv (no shell) and pipe packet via STDIN
3. WHEN runner mode is Native THEN the system SHALL spawn claude directly regardless of platform
4. WHEN runner mode is WSL THEN the system SHALL use WSL execution with optional distro and claude_path configuration
5. WHEN receipts are written THEN they SHALL include runner: "native"|"wsl" and runner_distro if applicable
6. WHEN WSL detection fails THEN the system SHALL provide friendly preflight error with install hints

### Requirement 13

**User Story:** As a developer, I want reliable canonicalization testing, so that I can verify deterministic behavior works correctly.

#### Acceptance Criteria

1. WHEN given an intentionally reordered YAML output THEN the canonicalizer MUST produce the same `*.core.yaml` hash
2. WHEN testing canonicalization THEN the system SHALL verify that structure determinism works independent of text formatting  
3. WHEN canonicalization fails THEN the system SHALL provide clear error messages about what couldn't be normalized

## Non-Functional Requirements

**NFR1 Performance:** Empty run ≤ 5s; packetization ≤ 200ms for ≤ 100 files

**NFR2 Atomicity:** Artifacts and receipts written via tempfile+rename to prevent corruption; on Windows, persist/rename is retried with bounded exponential backoff (≤ 250 ms total) to mitigate transient antivirus/indexer locks

**NFR3 Concurrency:** Exclusive filesystem lock per spec id directory; second writer exits with code 9

**NFR4 Portability:** Linux/macOS/WSL2; no bash-specific features required

**NFR5 Observability:** `--verbose` logs selected files, sizes, and hashes; no secrets ever logged
# xchecker-runner Extraction Fix Plan

## Problem Analysis

The xchecker-runner extraction created a broken crate structure with circular dependencies:

1. **Original Structure** (`xchecker-utils/src/runner/`):
   - `runner/mod.rs` - main module file
   - `claude/` - subdirectory with submodules
   - `native.rs` - submodule file
   - `ndjson.rs` - submodule file
   - `process.rs` - submodule file
   - `wsl.rs` - submodule file
   - `command_spec.rs` - submodule file

2. **Extraction Created** (`xchecker-runner/`):
   - Copied all files from `xchecker-utils/src/runner/` to `xchecker-runner/src/`
   - Created duplicate `claude/`, `native/`, `ndjson/`, `process/`, `wsl/`, `command_spec/` directories
   - Removed duplicate `runner/` directory
   - Copied files still have old import paths (`crate::error`, `crate::runner`, etc.)

3. **Current Broken State**:
   - `xchecker-runner/src/lib.rs` tries to declare modules that don't exist
   - Copied files reference removed modules with `crate::` prefix
   - xchecker-llm tries to import `xchecker_utils::runner` (which no longer exists)
   - xchecker-utils no longer re-exports runner types

## Root Cause

The extraction process created a circular dependency:
- `xchecker-runner` needs types from `xchecker-utils` (error, types, ring_buffer)
- `xchecker-utils` needs to re-export runner types for `xchecker-runner`
- But `xchecker-runner` was extracted as a separate crate, breaking this cycle

## Fix Strategy

### Phase 1: Fix xchecker-runner Crate Structure

1. **Update import paths in copied files**:
   - Replace `crate::error` with `xchecker_utils::error`
   - Replace `crate::ring_buffer` with `xchecker_utils::ring_buffer`
   - Replace `crate::types` with `xchecker_utils::types`
   - Replace `crate::ndjson` with `xchecker_utils::runner::ndjson` (if needed)
   - Replace `crate::runner` with `xchecker_utils::runner` (if needed)

2. **Fix xchecker-runner/src/lib.rs**:
   - Remove broken module declarations
   - Re-export types from xchecker-utils
   - Keep the crate structure simple - don't try to reorganize

3. **Update xchecker-utils to re-export runner types**:
   - Add `pub use crate::runner::*` to re-export runner types
   - This allows xchecker-runner to access types from xchecker-utils

### Phase 2: Fix xchecker-llm Imports

1. **Update xchecker-llm/src/lib.rs**:
   - Change `pub use xchecker_utils::runner` to `pub use xchecker_runner as runner`
   - This fixes the import to use the new xchecker-runner crate

### Phase 3: Fix xchecker-error-reporter Imports

1. **Already fixed**:
   - Changed `xchecker_utils::redaction` to `xchecker_redaction`
   - Added xchecker-redaction dependency

### Phase 4: Fix xchecker-config Imports

1. **Already fixed**:
   - Changed `xchecker_utils::redaction` to `xchecker_redaction`
   - Added xchecker-redaction dependency

## Implementation Steps

1. Update all import paths in `xchecker-runner/src/claude/detect.rs`
2. Update all import paths in `xchecker-runner/src/claude/exec.rs`
3. Update all import paths in `xchecker-runner/src/claude/io.rs`
4. Update all import paths in `xchecker-runner/src/claude/native_cmd.rs`
5. Update all import paths in `xchecker-runner/src/claude/platform/mod.rs`
6. Update all import paths in `xchecker-runner/src/claude/platform/unix.rs`
7. Update all import paths in `xchecker-runner/src/claude/platform/windows_job.rs`
8. Update all import paths in `xchecker-runner/src/claude/platform/windows.rs`
9. Update all import paths in `xchecker-runner/src/claude/types.rs`
10. Update all import paths in `xchecker-runner/src/claude/version.rs`
11. Update all import paths in `xchecker-runner/src/claude/wsl.rs`
12. Update all import paths in `xchecker-runner/src/native.rs`
13. Update all import paths in `xchecker-runner/src/ndjson.rs`
14. Update all import paths in `xchecker-runner/src/process.rs`
15. Update all import paths in `xchecker-runner/src/wsl.rs`
16. Update all import paths in `xchecker-runner/src/command_spec.rs`

2. Fix `xchecker-runner/src/lib.rs` module declarations

3. Add runner type re-exports to `xchecker-utils/src/lib.rs`

4. Update `xchecker-llm/src/lib.rs` to import from `xchecker_runner`

5. Run cargo check to verify build

6. Run cargo test to verify functionality

## Dependencies

- xchecker-utils needs to re-export runner types
- xchecker-runner needs to import from xchecker-utils
- xchecker-llm needs to import from xchecker-runner
- xchecker-error-reporter needs xchecker-redaction
- xchecker-config needs xchecker-redaction

## Verification

After implementing all fixes:
- `cargo check --workspace` should pass
- `cargo test --workspace` should pass
- All imports should resolve correctly
- No circular dependencies should exist

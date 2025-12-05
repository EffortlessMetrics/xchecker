# Platform Support Guide

This document describes xchecker's cross-platform support, platform-specific features, and troubleshooting guidance.

## Supported Platforms

xchecker supports the following platforms:

| Platform | Support Level | Runner Modes | Notes |
|----------|--------------|--------------|-------|
| **Linux** | ✅ Full | Native | Best performance, native execution |
| **macOS** | ✅ Full | Native | Full support, native execution |
| **Windows** | ✅ Full | Native, WSL, Auto | Native or WSL execution |
| **WSL2** | ✅ Full | WSL | Recommended over WSL1 |
| **WSL1** | ⚠️ Limited | WSL | Slower, use WSL2 if possible |

## Platform-Specific Features

### Linux

**Advantages:**
- Best overall performance
- Native process control with `killpg`
- Excellent file system performance
- SIMD optimizations for BLAKE3
- No translation overhead

**Process Termination:**
- Uses `killpg` for process group termination
- TERM signal → 5s grace period → KILL signal
- Reliable child process cleanup

**File System:**
- Atomic rename on same filesystem
- Cross-filesystem fallback (copy+fsync+replace)
- POSIX mode bit preservation

**Example:**
```bash
# Native execution (default)
xchecker spec my-feature

# Verify runner mode
xchecker doctor --json | jq '.checks[] | select(.name == "runner_selection")'
```

### macOS

**Advantages:**
- Full native support
- Good file system performance
- SIMD optimizations for BLAKE3
- Native process control

**Process Termination:**
- Uses `killpg` for process group termination
- TERM signal → 5s grace period → KILL signal
- Reliable child process cleanup

**File System:**
- Atomic rename on same filesystem
- Cross-filesystem fallback (copy+fsync+replace)
- POSIX mode bit preservation

**Known Issues:**
- Case-insensitive file system by default (APFS)
- May affect file path matching in some cases

**Example:**
```bash
# Native execution (default)
xchecker spec my-feature

# Check file system case sensitivity
diskutil info / | grep "Case-sensitive"
```

### Windows (Native)

**Advantages:**
- Native execution without WSL
- Windows-specific optimizations
- Job Objects for process tree termination
- Retry logic for antivirus/indexer locks

**Process Termination:**
- Uses Job Objects for process tree termination
- Ensures all child processes are terminated
- Handles nested process hierarchies

**File System:**
- Atomic rename with retry logic (≤250ms)
- Handles antivirus/indexer transient locks
- File attribute preservation
- CRLF tolerance on read, LF enforcement on write

**Retry Logic:**
```
Attempt 1: Immediate rename
Attempt 2: Wait 10ms, retry
Attempt 3: Wait 20ms, retry
Attempt 4: Wait 40ms, retry
Attempt 5: Wait 80ms, retry
Total: ≤250ms
```

**Known Issues:**
- Antivirus scanning can slow file operations
- Windows Defender may scan new files
- Path length limit (260 characters by default)

**Solutions:**
- Exclude `.xchecker/` from antivirus scanning
- Enable long path support: `HKLM\SYSTEM\CurrentControlSet\Control\FileSystem\LongPathsEnabled`
- Use WSL if native performance is insufficient

**Example:**
```bash
# Native execution (default)
xchecker spec my-feature

# Verify Job Objects are working
xchecker doctor --json | jq '.checks[] | select(.name == "timeout_enforcement")'
```

### Windows (WSL)

**Advantages:**
- Linux environment on Windows
- Access to Linux tools and Claude CLI
- Better compatibility with Linux-based workflows
- Fallback when native Claude unavailable

**WSL Modes:**
- **WSL2**: Recommended, better performance, full Linux kernel
- **WSL1**: Legacy, slower, translation layer

**Path Translation:**
- Windows paths → WSL paths: `C:\path\to\file` → `/mnt/c/path/to/file`
- Uses `wslpath -a` for canonical translation
- Fallback heuristic if `wslpath` unavailable

**Process Execution:**
- Uses `wsl.exe --exec` with discrete argv
- No shell quoting issues
- Proper argument passing

**File System:**
- Artifacts persist in Windows spec root
- Cross-filesystem performance impact
- Path translation adds latency

**Runner Distro:**
- Captured from `wsl -l -q` or `$WSL_DISTRO_NAME`
- Included in receipts as `runner_distro`
- Configurable via `--runner-distro`

**Example:**
```bash
# WSL execution (explicit)
xchecker spec my-feature --runner-mode wsl

# WSL with specific distro
xchecker spec my-feature --runner-mode wsl --runner-distro Ubuntu-22.04

# Auto mode (native first, WSL fallback)
xchecker spec my-feature --runner-mode auto
```

## Runner Mode Selection

### Auto Mode (Recommended)

Auto mode automatically selects the best available runner:

1. **Try Native**: Check if Claude CLI is in Windows PATH
2. **Fallback to WSL**: If native unavailable, try WSL
3. **Error**: If neither available, exit with helpful error

**Example:**
```bash
# Auto mode (default)
xchecker spec my-feature

# Explicit auto mode
xchecker spec my-feature --runner-mode auto
```

### Native Mode

Force native execution, fail if Claude CLI not available:

**Example:**
```bash
# Native mode (explicit)
xchecker spec my-feature --runner-mode native

# Verify native Claude
where claude  # Windows
which claude  # Linux/macOS
```

### WSL Mode

Force WSL execution, fail if WSL not available:

**Example:**
```bash
# WSL mode (explicit)
xchecker spec my-feature --runner-mode wsl

# WSL with specific distro
xchecker spec my-feature --runner-mode wsl --runner-distro Ubuntu-22.04

# List available distros
wsl -l -v
```

## Platform-Specific Configuration

### Linux Configuration

```toml
# .xchecker/config.toml
[runner]
mode = "native"
claude_path = "/usr/local/bin/claude"

[selectors]
# Linux-specific patterns
include = [
    "src/**/*.rs",
    "Cargo.toml",
    "*.sh"
]
exclude = [
    "target/**",
    ".git/**"
]
```

### macOS Configuration

```toml
# .xchecker/config.toml
[runner]
mode = "native"
claude_path = "/usr/local/bin/claude"

[selectors]
# macOS-specific patterns
include = [
    "src/**/*.rs",
    "Cargo.toml",
    "*.sh"
]
exclude = [
    "target/**",
    ".git/**",
    ".DS_Store"
]
```

### Windows Native Configuration

```toml
# .xchecker/config.toml
[runner]
mode = "native"
claude_path = "C:\\Program Files\\Claude\\claude.exe"

[selectors]
# Windows-specific patterns
include = [
    "src/**/*.rs",
    "Cargo.toml",
    "*.ps1",
    "*.bat"
]
exclude = [
    "target/**",
    ".git/**",
    "*.tmp"
]
```

### Windows WSL Configuration

```toml
# .xchecker/config.toml
[runner]
mode = "wsl"
distro = "Ubuntu-22.04"
claude_path = "/usr/local/bin/claude"

[selectors]
# WSL-specific patterns
include = [
    "src/**/*.rs",
    "Cargo.toml",
    "*.sh"
]
exclude = [
    "target/**",
    ".git/**"
]
```

## Platform-Specific Troubleshooting

### Linux Issues

#### Permission Denied

**Symptom:** `Permission denied` when executing Claude CLI

**Solution:**
```bash
# Check Claude CLI permissions
ls -l $(which claude)

# Make executable if needed
chmod +x $(which claude)

# Verify execution
claude --version
```

#### File System Errors

**Symptom:** `EXDEV` error during atomic rename

**Solution:**
- Ensure `.xchecker/` is on same filesystem as project
- Check for cross-filesystem mounts
- Verify disk space available

### macOS Issues

#### Gatekeeper Blocking

**Symptom:** "Claude cannot be opened because the developer cannot be verified"

**Solution:**
```bash
# Allow Claude CLI
xattr -d com.apple.quarantine $(which claude)

# Or via System Preferences
# System Preferences → Security & Privacy → Allow
```

#### Case Sensitivity

**Symptom:** File path mismatches due to case-insensitive file system

**Solution:**
- Use consistent casing in file paths
- Consider case-sensitive APFS volume for development
- Check file system: `diskutil info / | grep "Case-sensitive"`

### Windows Native Issues

#### Antivirus Interference

**Symptom:** Slow file operations, timeout errors

**Solution:**
```powershell
# Exclude .xchecker from Windows Defender
Add-MpPreference -ExclusionPath "C:\path\to\project\.xchecker"

# Verify exclusions
Get-MpPreference | Select-Object -ExpandProperty ExclusionPath
```

#### Path Length Limit

**Symptom:** `ERROR_PATH_TOO_LONG` or similar errors

**Solution:**
```powershell
# Enable long path support (requires admin)
New-ItemProperty -Path "HKLM:\SYSTEM\CurrentControlSet\Control\FileSystem" `
  -Name "LongPathsEnabled" -Value 1 -PropertyType DWORD -Force

# Restart required
Restart-Computer
```

#### Claude Not Found

**Symptom:** `'claude' is not recognized as an internal or external command`

**Solution:**
```powershell
# Check if Claude is in PATH
where claude

# Add Claude to PATH if needed
$env:PATH += ";C:\Program Files\Claude"

# Verify
claude --version
```

### Windows WSL Issues

#### WSL Not Installed

**Symptom:** `WSL is not available` error

**Solution:**
```powershell
# Install WSL
wsl --install

# Restart required
Restart-Computer

# Verify installation
wsl -l -v
```

#### Claude Not in WSL

**Symptom:** `Claude CLI not found in WSL` error

**Solution:**
```bash
# Install Claude in WSL
wsl -e bash -c "curl -fsSL https://claude.ai/install.sh | sh"

# Verify installation
wsl -e claude --version
```

#### WSL Path Translation

**Symptom:** `Invalid path` errors when using WSL

**Solution:**
```bash
# Verify wslpath is available
wsl -e wslpath -a "C:\path\to\file"

# Check WSL distro
wsl -l -v

# Set default distro if needed
wsl --set-default Ubuntu-22.04
```

#### WSL Performance

**Symptom:** Slow execution in WSL mode

**Solution:**
```powershell
# Upgrade to WSL2
wsl --set-version Ubuntu-22.04 2

# Verify WSL2
wsl -l -v

# Keep files in WSL filesystem for better performance
wsl -e bash -c "cd ~ && git clone <repo>"
```

## Performance Comparison

### Benchmark Results

Typical performance across platforms (100 files, 5 iterations):

| Platform | Dry Run | Packetization | JCS Emission |
|----------|---------|---------------|--------------|
| **Linux** | 1.8s | 95ms | 18ms |
| **macOS** | 2.3s | 125ms | 24ms |
| **Windows Native** | 2.9s | 145ms | 28ms |
| **Windows WSL2** | 3.7s | 175ms | 30ms |
| **Windows WSL1** | 5.2s | 245ms | 35ms |

### Performance Recommendations

1. **Linux**: Best performance, use for CI/CD
2. **macOS**: Good performance, native development
3. **Windows Native**: Good performance, use if Claude available
4. **Windows WSL2**: Moderate performance, use if native unavailable
5. **Windows WSL1**: Upgrade to WSL2 for better performance

## Platform-Specific Best Practices

### Linux

1. Use native execution (best performance)
2. Ensure Claude CLI in PATH
3. Use SSD storage for `.xchecker/`
4. Monitor disk space

### macOS

1. Use native execution
2. Handle Gatekeeper prompts
3. Be aware of case-insensitive file system
4. Use SSD storage for `.xchecker/`

### Windows Native

1. Exclude `.xchecker/` from antivirus
2. Enable long path support
3. Use SSD storage for `.xchecker/`
4. Monitor disk space
5. Consider WSL if performance insufficient

### Windows WSL

1. Use WSL2 (not WSL1)
2. Keep files in WSL filesystem when possible
3. Use `wslpath` for path translation
4. Set default distro explicitly
5. Monitor cross-filesystem performance

## CI/CD Platform Support

### GitHub Actions

```yaml
name: xchecker CI

on: [push, pull_request]

jobs:
  test:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    runs-on: ${{ matrix.os }}
    
    steps:
      - uses: actions/checkout@v3
      
      - name: Install xchecker
        run: cargo install xchecker
      
      - name: Run doctor
        run: xchecker doctor --strict-exit
      
      - name: Run benchmarks
        run: xchecker benchmark --json
```

### GitLab CI

```yaml
test:linux:
  image: rust:latest
  script:
    - cargo install xchecker
    - xchecker doctor --strict-exit
    - xchecker benchmark --json

test:macos:
  tags: [macos]
  script:
    - cargo install xchecker
    - xchecker doctor --strict-exit
    - xchecker benchmark --json

test:windows:
  tags: [windows]
  script:
    - cargo install xchecker
    - xchecker doctor --strict-exit
    - xchecker benchmark --json
```

## Platform Testing

### Testing Checklist

- [ ] Native execution works
- [ ] WSL execution works (Windows only)
- [ ] Auto mode selects correct runner
- [ ] File operations are atomic
- [ ] Process termination works correctly
- [ ] Path translation works (WSL only)
- [ ] Performance meets targets
- [ ] Doctor checks pass

### Platform-Specific Tests

```bash
# Linux/macOS
cargo test --all-features

# Windows Native (parallel execution may cause flaky tests)
cargo test --all-features

# Windows Native (recommended for CI - avoids env var race conditions)
cargo test --all-features -- --test-threads=1

# Windows WSL
cargo test --all-features -- --test-threads=1
```

### Known Test Isolation Issues

Some tests modify environment variables (e.g., `OPENROUTER_API_KEY`, `XCHECKER_OPENROUTER_BUDGET`) which are not thread-safe in Rust. When tests run in parallel, they can interfere with each other's environment variables, causing intermittent failures.

**Affected tests:**
- `llm::factory_tests::test_openrouter_backend_is_budgeted`
- `llm::factory_tests::test_openrouter_budget_exhaustion_in_production_path`
- `llm::budgeted_backend::tests::test_budget_precedence_env_over_config`

**Workaround:** Run tests with `--test-threads=1` to ensure serial execution:
```bash
cargo test --all-features -- --test-threads=1
```

**Note:** This is a known limitation of Rust's test framework when tests modify global state (environment variables). The tests pass reliably when run serially.

## Platform Support Roadmap

### Current Support (v1.0)
- ✅ Linux native execution
- ✅ macOS native execution
- ✅ Windows native execution
- ✅ Windows WSL2 execution
- ✅ Windows WSL1 execution (limited)
- ✅ Auto runner mode selection

### Future Support
- ⏳ FreeBSD native execution
- ⏳ ARM64 Linux support
- ⏳ ARM64 macOS support (Apple Silicon)
- ⏳ ARM64 Windows support
- ⏳ Docker container execution

## References

- [FR-WSL: WSL Support Requirements](../requirements.md#requirement-12-fr-wsl)
- [NFR3: Portability Requirements](../requirements.md#nfr3-portability)
- [NFR4: Platform Support](../requirements.md#nfr4-portability)
- [Runner Implementation](../src/runner.rs)
- [WSL Implementation](../src/wsl.rs)

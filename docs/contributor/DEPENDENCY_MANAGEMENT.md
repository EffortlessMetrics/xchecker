# Dependency Management Guardrails

This document describes the CI guardrails implemented to maintain the hybrid dependency policy for xchecker.

## Overview

The project uses a hybrid dependency policy:
- **Security-critical dependencies**: Exact patch versions for maximum security
- **Core/platform dependencies**: Coarse minima for flexibility
- **Other dependencies**: Coarse minima with controlled updates

## CI Guardrails Implementation

### 1. Dependency Management Workflow

Location: `.github/workflows/dependency-management.yml`

This comprehensive workflow implements the following guardrails:

#### Dual Testing Strategy
- **Locked Testing**: `cargo test --all-features --locked`
  - Ensures maintainer reproducibility
  - Tests against exact versions in Cargo.lock
  - Runs on all platforms (Linux, macOS, Windows)

- **Fresh Resolve Testing**: `cargo update && cargo test --all-features`
  - Simulates user reality with fresh dependency resolution
  - Catches dependency drift early
  - Runs on all platforms (Linux, macOS, Windows)

#### Dependency Duplication Checks
- Uses `cargo tree -d` to monitor duplicate dependencies
- Alerts for concerning patterns:
  - Windows ecosystem duplicates (>3 instances)
  - Serde ecosystem duplicates (>5 instances)
- Uploads dependency tree as artifact for analysis

#### Security Scanning
- **cargo-audit**: Scans for known vulnerabilities
- **cargo-deny**: Comprehensive license and security checks
- Special attention to security-critical dependencies:
  - reqwest, tokio, serde, serde_json, blake3
- Fails if vulnerabilities found in security-critical deps

#### MSRV Enforcement
- Tests with Rust 1.89 (exact MSRV version)
- Ensures project builds with minimum supported Rust version
- Prevents dependency version increases that break MSRV

#### Dependency Drift Monitoring
- Nightly checks for available updates
- Monitors Cargo.lock freshness
- Reports security-critical dependency updates

#### Policy Validation
- Validates hybrid dependency policy compliance
- Checks exact versions for security-critical deps
- Verifies coarse minima for core deps

### 2. Integration with Existing Workflows

#### Main CI Workflow (`.github/workflows/ci.yml`)
- Added `dependency-policy` job that runs first
- All test jobs depend on dependency policy passing
- Ensures policy compliance before running tests

#### Test Workflow (`.github/workflows/test.yml`)
- Updated test-fast to use `--locked` flag
- Added both locked and fresh resolve testing to test-full
- Maintains backward compatibility while adding guardrails

### 3. Security-Critical Dependencies

The following dependencies are locked to exact patch versions:

```toml
# Security-Critical Dependencies (keep exact patch versions)
reqwest = { version = "0.12.26", features = ["json", "rustls-tls"], default-features = false }
tokio = { version = "1.48.0", features = ["full"] }
serde = { version = "1.0.228", features = ["derive"] }
serde_json = "1.0.145"
blake3 = { version = "1.8.2", features = ["rayon"] }
```

### 4. Core Infrastructure Dependencies

The following dependencies use coarse minima:

```toml
# Core Infrastructure Dependencies (coarse minima)
clap = { version = "4.5", features = ["derive"] }
anyhow = "1.0"
thiserror = "2.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt", "json"] }
tempfile = "3.23"
```

## Usage

### Running Guardrails Locally

Use the provided test script to validate guardrails locally:

```bash
# On Unix systems
./scripts/test-dependency-guardrails.sh

# On Windows (Git Bash or WSL)
bash scripts/test-dependency-guardrails.sh
```

### Running in CI

The dependency management workflow runs automatically on:
- Push to main branch
- Pull requests to main branch
- Nightly schedule (3:00 AM UTC)
- Manual dispatch with options

### Manual Workflow Execution

```bash
# Run full dependency management
gh workflow run dependency-management

# Run security-only scan
gh workflow run dependency-management -f scan_mode=security-only

# Run duplicates-only check
gh workflow run dependency-management -f scan_mode=duplicates-only
```

## Policy Updates

### Adding Security-Critical Dependencies

1. Add to Cargo.toml with exact version
2. Update SECURITY_CRITICAL array in dependency-management.yml
3. Update validation logic in CI workflows
4. Update this documentation

### Updating Security-Critical Dependencies

1. Evaluate security implications
2. Update exact version in Cargo.toml
3. Update CI validation arrays
4. Test thoroughly with both locked and fresh resolve
5. Update documentation

### Adding Core Dependencies

1. Add to Cargo.toml with coarse minimum version
2. Update CORE_DEPS array if needed for validation
3. Test with both locked and fresh resolve

## Monitoring and Alerts

### Artifacts Generated

- `dependency-tree.txt`: Full dependency tree analysis
- `audit-results.json`: Security audit results
- `security-scan-results.json`: Comprehensive security scan results
- `dependency-drift-results.txt`: Available updates analysis

### Failure Scenarios

1. **Policy Violation**: Dependency version doesn't match policy
2. **Locked Test Failure**: Issue with exact dependency versions
3. **Fresh Test Failure**: Dependency drift or compatibility issues
4. **Duplicate Alert**: Excessive duplication detected
5. **Security Vulnerability**: Vulnerability in security-critical dependency
6. **MSRV Failure**: Dependency breaks minimum Rust version

### Response Procedures

1. **Policy Violation**: Review and correct dependency version
2. **Test Failures**: Investigate compatibility issues
3. **Duplicate Alert**: Consider dependency consolidation
4. **Security Issues**: Immediate update of vulnerable dependencies
5. **MSRV Failure**: Downgrade dependency or update MSRV

## Best Practices

1. **Regular Updates**: Review dependency updates monthly
2. **Security First**: Prioritize security-critical dependency updates
3. **Test Both Ways**: Always test with both locked and fresh resolve
4. **Monitor Drift**: Watch for dependency ecosystem changes
5. **Document Changes**: Keep this documentation updated

## Troubleshooting

### Common Issues

1. **Cargo.lock Out of Sync**: Run `cargo generate-lockfile`
2. **MSRV Failure**: Check which dependency increased minimum Rust version
3. **Duplicate Warnings**: Review dependency tree for consolidation opportunities
4. **Security Alerts**: Prioritize updates for security-critical dependencies

### Debug Commands

```bash
# Check dependency tree
cargo tree -d

# Check for updates
cargo update --dry-run

# Run security audit
cargo audit

# Check MSRV compliance
rustup default 1.89.0 && cargo check --all-features
```

## Future Enhancements

1. **Automated Dependency Updates**: PR-based automated updates
2. **Dependency Dashboard**: Visual monitoring of dependency health
3. **Policy Exceptions**: Process for temporary policy bypasses
4. **Integration with Security Tools**: Enhanced vulnerability scanning
5. **Performance Impact Monitoring**: Track dependency size and performance
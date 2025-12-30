#!/bin/bash

# Test script to validate dependency guardrails locally
# This script simulates the CI checks to ensure they work before committing

set -e

echo "=== Testing Dependency Management Guardrails ==="
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print status
print_status() {
    if [ $1 -eq 0 ]; then
        echo -e "${GREEN}✅ $2${NC}"
    else
        echo -e "${RED}❌ $2${NC}"
        exit 1
    fi
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

echo "1. Testing hybrid dependency policy validation..."
# This simulates the dependency-policy job
PYTHON_BIN="${PYTHON_BIN:-python3}"
if ! command -v "$PYTHON_BIN" >/dev/null 2>&1; then
    PYTHON_BIN=python
fi

if ! command -v "$PYTHON_BIN" >/dev/null 2>&1; then
    print_status 1 "Python 3.11+ is required for policy validation"
fi

if "$PYTHON_BIN" - <<'PY'
from pathlib import Path
import re
import sys

try:
    import tomllib
except ModuleNotFoundError:  # Python < 3.11
    try:
        import tomli as tomllib
    except ModuleNotFoundError:
        print("tomllib not available (need Python 3.11+ or tomli).", file=sys.stderr)
        sys.exit(2)

SECURITY_CRITICAL = {
    "reqwest": "0.12.26",
    "tokio": "1.48.0",
    "serde": "1.0.228",
    "serde_json": "1.0.145",
    "blake3": "1.8.2",
}

CORE_DEPS = {
    "clap": "4.5",
    "anyhow": "1.0",
    "thiserror": "2.0",
    "tracing": "0.1",
}

def read_version(deps, name):
    spec = deps.get(name)
    if isinstance(spec, str):
        return spec.strip()
    if isinstance(spec, dict):
        version = spec.get("version")
        if isinstance(version, str):
            return version.strip()
    return None

def extract_version(req):
    if not req:
        return None
    match = re.search(r"\d+\.\d+(?:\.\d+)?", req)
    return match.group(0) if match else None

data = tomllib.loads(Path("Cargo.toml").read_text())
deps = data.get("dependencies", {})

errors = []

print("  Checking security-critical dependency versions...")
for name, expected in SECURITY_CRITICAL.items():
    version = read_version(deps, name)
    if version is None:
        errors.append(f"{name}: missing from Cargo.toml")
        continue
    if version != expected and version != f"={expected}":
        errors.append(f"{name}: expected exact {expected}, found {version}")
    else:
        print(f"  ✅ {name}: exact version {expected}")

print("  Checking core dependency coarse minima...")
for name, minimum in CORE_DEPS.items():
    version = read_version(deps, name)
    if version is None:
        errors.append(f"{name}: missing from Cargo.toml")
        continue
    found = extract_version(version)
    if found is None or not found.startswith(minimum):
        errors.append(f"{name}: expected minimum {minimum}, found {version}")
    else:
        print(f"  ✅ {name}: coarse minimum {minimum} (found {version})")

if errors:
    print("  Policy validation failed:")
    for error in errors:
        print(f"  ❌ {error}")
    sys.exit(1)

print("  ✅ Hybrid dependency policy validation passed")
PY
then
    print_status 0 "Hybrid dependency policy validation"
else
    print_status 1 "Hybrid dependency policy validation"
fi

echo ""
echo "2. Testing locked dependency resolution..."
if cargo test --lib --bins --locked; then
    print_status 0 "Locked dependency testing"
else
    print_status 1 "Locked dependency testing"
fi

echo ""
echo "3. Testing fresh dependency resolution..."
if cargo update && cargo test --lib --bins; then
    print_status 0 "Fresh resolve testing"
else
    print_status 1 "Fresh resolve testing"
fi

echo ""
echo "4. Testing dependency duplication checks..."
if cargo tree -d > /tmp/dep-tree.txt; then
    DUPLICATE_COUNT=$(grep -c "^[a-zA-Z]" /tmp/dep-tree.txt || echo "0")
    echo "  Found $DUPLICATE_COUNT duplicate dependency groups"
    
    # Check for concerning patterns
    WINDOWS_DUPES=$(grep -i "winapi\|windows\|kernel32\|user32" /tmp/dep-tree.txt | wc -l || echo "0")
    SERDE_DUPES=$(grep -i "serde" /tmp/dep-tree.txt | wc -l || echo "0")
    
    if [ "$WINDOWS_DUPES" -gt 3 ]; then
        print_warning "High Windows ecosystem duplication detected ($WINDOWS_DUPES instances)"
    fi
    
    if [ "$SERDE_DUPES" -gt 5 ]; then
        print_warning "High serde ecosystem duplication detected ($SERDE_DUPES instances)"
    fi
    
    print_status 0 "Dependency duplication checks"
else
    print_status 1 "Dependency duplication checks"
fi

echo ""
echo "5. Testing MSRV compliance (if Rust 1.89 is available)..."
if rustup toolchain list | grep -q "1.89.0"; then
    if rustup default 1.89.0 && cargo check --all-features; then
        print_status 0 "MSRV compliance check"
    else
        print_status 1 "MSRV compliance check"
    fi
    # Restore default toolchain
    rustup default stable
else
    print_warning "Rust 1.89.0 not available locally - skipping MSRV check"
fi

echo ""
echo "6. Testing security audit (if cargo-audit is available)..."
if command -v cargo-audit &> /dev/null; then
    if cargo audit; then
        print_status 0 "Security audit"
    else
        print_warning "Security audit found issues (review output)"
    fi
else
    print_warning "cargo-audit not available - skipping security audit"
fi

echo ""
echo "=== All dependency guardrails tests completed ==="
echo ""
echo "To run the full CI dependency management workflow:"
echo "  gh workflow run dependency-management"
echo ""
echo "To run specific jobs:"
echo "  gh workflow run dependency-management -f scan_mode=security-only"
echo "  gh workflow run dependency-management -f scan_mode=duplicates-only"

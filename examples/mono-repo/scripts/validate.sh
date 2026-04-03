#!/usr/bin/env bash
# Validation script for mono-repo example
# This script validates the example structure and configuration.
# Designed to run in CI (GitHub Actions) on ubuntu, macos, and windows (Git Bash).

set -euo pipefail

echo "=== Validating mono-repo example ==="

# Get script directory (works on all platforms)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
EXAMPLE_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "Example directory: $EXAMPLE_DIR"
cd "$EXAMPLE_DIR"

ERRORS=0

check_file() {
    local path="$1"
    local label="$2"
    if [ -f "$path" ]; then
        echo "  [PASS] $label exists: $path"
    else
        echo "  [FAIL] $label not found: $path"
        ERRORS=$((ERRORS + 1))
    fi
}

check_dir() {
    local path="$1"
    local label="$2"
    if [ -d "$path" ]; then
        echo "  [PASS] $label exists: $path"
    else
        echo "  [FAIL] $label not found: $path"
        ERRORS=$((ERRORS + 1))
    fi
}

# Check workspace.yaml exists
echo ""
echo "Checking workspace.yaml..."
check_file "workspace.yaml" "workspace.yaml"

# Validate workspace.yaml has expected specs
echo ""
echo "Validating workspace specs..."
for spec in user-service product-catalog order-api; do
    if grep -q "id: $spec" workspace.yaml 2>/dev/null; then
        echo "  [PASS] Spec '$spec' registered in workspace"
    else
        echo "  [FAIL] Spec '$spec' not found in workspace.yaml"
        ERRORS=$((ERRORS + 1))
    fi
done

# Check .xchecker/config.toml exists
echo ""
echo "Checking .xchecker/config.toml..."
check_file ".xchecker/config.toml" ".xchecker/config.toml"

# Check all spec directories
echo ""
echo "Checking spec directories..."
for spec in user-service product-catalog order-api; do
    SPEC_DIR=".xchecker/specs/$spec"
    check_dir "$SPEC_DIR" "$spec directory"

    # Check context directory
    CONTEXT_DIR="$SPEC_DIR/context"
    check_dir "$CONTEXT_DIR" "$spec context directory"

    # Check problem statement
    check_file "$CONTEXT_DIR/problem-statement.md" "$spec problem-statement.md"
done

# Check README exists
echo ""
echo "Checking README.md..."
check_file "README.md" "README.md"

# Summary
echo ""
if [ "$ERRORS" -eq 0 ]; then
    echo "=== All validations passed ==="
    exit 0
else
    echo "=== FAILED: $ERRORS error(s) found ==="
    exit 1
fi

#!/bin/bash
# Validation script for fullstack-nextjs example
# This script validates the example structure and configuration

set -e

echo "=== Validating fullstack-nextjs example ==="

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
EXAMPLE_DIR="$(dirname "$SCRIPT_DIR")"

cd "$EXAMPLE_DIR"

# Check workspace.yaml exists
echo ""
echo "Checking workspace.yaml..."
if [ ! -f "workspace.yaml" ]; then
    echo "ERROR: workspace.yaml not found"
    exit 1
fi
echo "  ✓ workspace.yaml exists"

# Check .xchecker/config.toml exists
echo ""
echo "Checking .xchecker/config.toml..."
if [ ! -f ".xchecker/config.toml" ]; then
    echo "ERROR: .xchecker/config.toml not found"
    exit 1
fi
echo "  ✓ .xchecker/config.toml exists"

# Check spec directory structure
echo ""
echo "Checking spec directory structure..."
SPEC_DIR=".xchecker/specs/task-manager"
if [ ! -d "$SPEC_DIR" ]; then
    echo "ERROR: Spec directory not found: $SPEC_DIR"
    exit 1
fi
echo "  ✓ Spec directory exists"

# Check context directory
CONTEXT_DIR="$SPEC_DIR/context"
if [ ! -d "$CONTEXT_DIR" ]; then
    echo "ERROR: Context directory not found: $CONTEXT_DIR"
    exit 1
fi
echo "  ✓ Context directory exists"

# Check problem statement
PROBLEM_STATEMENT="$CONTEXT_DIR/problem-statement.md"
if [ ! -f "$PROBLEM_STATEMENT" ]; then
    echo "ERROR: Problem statement not found: $PROBLEM_STATEMENT"
    exit 1
fi
echo "  ✓ Problem statement exists"

# Check README exists
echo ""
echo "Checking README.md..."
if [ ! -f "README.md" ]; then
    echo "ERROR: README.md not found"
    exit 1
fi
echo "  ✓ README.md exists"

echo ""
echo "=== All validations passed ==="
exit 0

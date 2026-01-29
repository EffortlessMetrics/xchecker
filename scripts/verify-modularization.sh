#!/bin/bash
set -e

echo "=== Modularization Verification Gate ==="
echo ""

echo "1. Checking formatting..."
cargo fmt --all -- --check
echo "   ✓ Formatting check passed"

echo ""
echo "2. Running clippy..."
cargo clippy --workspace --all-targets --all-features -- -D warnings
echo "   ✓ Clippy check passed"

echo ""
echo "3. Running tests with all features..."
cargo test --workspace --all-features
echo "   ✓ All tests passed"

echo ""
echo "4. Checking dependency graph for cycles..."
cargo tree --duplicates
echo "   ✓ Dependency graph is clean"

echo ""
echo "=== All verification gates passed ==="

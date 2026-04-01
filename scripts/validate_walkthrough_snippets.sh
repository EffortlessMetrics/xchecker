#!/usr/bin/env bash
# validate_walkthrough_snippets.sh - Extract and validate code snippets from walkthroughs
#
# This script extracts bash code snippets from walkthrough documentation and
# validates that the commands referenced are valid xchecker commands.
#
# Usage: ./scripts/validate_walkthrough_snippets.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "=== Validating Walkthrough Code Snippets ==="
echo "Project root: $PROJECT_ROOT"

ERRORS=0

# Function to extract bash code blocks from markdown
# Returns empty string (not error) when no blocks found.
extract_bash_blocks() {
    local file="$1"
    # Extract content between ```bash and ``` markers, strip the markers.
    # Use "|| true" to avoid exit-code 1 from grep when there are no matches.
    awk '/^```bash$/,/^```$/' "$file" | grep -v '^```' || true
}

# Function to validate xchecker commands
validate_xchecker_commands() {
    local file="$1"
    echo ""
    echo "Checking commands: $file"

    # Extract bash blocks
    local blocks
    blocks="$(extract_bash_blocks "$file")"

    if [ -z "$blocks" ]; then
        echo "  [SKIP] No bash code blocks found"
        return
    fi

    # Check for xchecker commands
    local xchecker_cmds
    xchecker_cmds="$(echo "$blocks" | grep -E '^\s*xchecker\s+' || true)"

    if [ -z "$xchecker_cmds" ]; then
        echo "  [SKIP] No xchecker commands found"
        return
    fi

    # List of valid xchecker subcommands
    local valid_cmds="spec resume status clean doctor init benchmark test gate project template"

    # List of valid global flags
    local valid_flags="--version --help -h -V"

    # Validate each xchecker command
    while IFS= read -r cmd; do
        # Skip empty lines and comments
        [[ -z "$cmd" || "$cmd" =~ ^[[:space:]]*# ]] && continue

        # Extract the subcommand
        local subcmd
        subcmd="$(echo "$cmd" | sed 's/^[[:space:]]*//' | awk '{print $2}')"

        if echo " $valid_cmds " | grep -qF " $subcmd "; then
            echo "  [PASS] Valid command: xchecker $subcmd"
        elif echo " $valid_flags " | grep -qF " $subcmd "; then
            echo "  [PASS] Valid flag: xchecker $subcmd"
        else
            echo "  [FAIL] Unknown command: xchecker $subcmd (in $file)"
            ERRORS=$((ERRORS + 1))
        fi
    done <<< "$xchecker_cmds"
}

# Function to check for broken internal links
check_internal_links() {
    local file="$1"
    echo ""
    echo "Checking internal links: $file"

    # Extract markdown links to .md files
    local links
    links="$(grep -oE '\[.*\]\([^)]+\.md\)' "$file" | sed 's/.*(\([^)]*\))/\1/' || true)"

    if [ -z "$links" ]; then
        echo "  [SKIP] No internal links found"
        return
    fi

    local dir
    dir="$(dirname "$file")"

    while IFS= read -r link; do
        # Skip empty lines
        [[ -z "$link" ]] && continue

        # Strip any anchor fragment (e.g. "FILE.md#section")
        local link_path="${link%%#*}"

        # Resolve relative path
        local target
        if [[ "$link_path" == /* ]]; then
            target="$PROJECT_ROOT$link_path"
        else
            target="$dir/$link_path"
        fi

        # Normalize path (resolve ..)
        if [ -f "$target" ]; then
            echo "  [PASS] Link exists: $link"
        else
            echo "  [FAIL] Broken link: $link -> $target"
            ERRORS=$((ERRORS + 1))
        fi
    done <<< "$links"
}

# Function to check JSON examples are valid
check_json_examples() {
    local file="$1"
    echo ""
    echo "Checking JSON examples: $file"

    # Count JSON blocks
    local block_count
    block_count="$(awk '/^```json$/{count++} END{print count+0}' "$file")"

    if [ "$block_count" -eq 0 ]; then
        echo "  [SKIP] No JSON code blocks found"
        return
    fi

    echo "  [PASS] Found $block_count JSON block(s)"
}

# Main validation
echo ""
echo "=== Walkthrough Files ==="

# Discover walkthrough files: check both legacy locations and new locations.
WALKTHROUGH_FILES=()

# Legacy locations
for f in \
    "$PROJECT_ROOT/docs/WALKTHROUGH_20_MINUTES.md" \
    "$PROJECT_ROOT/docs/WALKTHROUGH_SPEC_TO_PR.md" \
; do
    [ -f "$f" ] && WALKTHROUGH_FILES+=("$f")
done

# New Diataxis tutorial locations
for f in \
    "$PROJECT_ROOT/docs/tutorials/QUICKSTART.md" \
    "$PROJECT_ROOT/docs/tutorials/SPEC_TO_PR.md" \
; do
    [ -f "$f" ] && WALKTHROUGH_FILES+=("$f")
done

if [ ${#WALKTHROUGH_FILES[@]} -eq 0 ]; then
    echo "[FAIL] No walkthrough files found in docs/ or docs/tutorials/"
    exit 1
fi

echo "Found ${#WALKTHROUGH_FILES[@]} walkthrough file(s)"

for file in "${WALKTHROUGH_FILES[@]}"; do
    validate_xchecker_commands "$file"
    check_internal_links "$file"
    check_json_examples "$file"
done

# Summary
echo ""
echo "=== Summary ==="
if [ "$ERRORS" -eq 0 ]; then
    echo "All walkthrough validations passed!"
    exit 0
else
    echo "FAILED: Found $ERRORS error(s)"
    exit 1
fi

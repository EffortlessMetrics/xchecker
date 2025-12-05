#!/bin/bash
# validate_walkthrough_snippets.sh - Extract and validate code snippets from walkthroughs
#
# This script extracts bash code snippets from walkthrough documentation and
# validates that the commands referenced are valid xchecker commands.
#
# Usage: ./scripts/validate_walkthrough_snippets.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "=== Validating Walkthrough Code Snippets ==="
echo "Project root: $PROJECT_ROOT"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

ERRORS=0
WARNINGS=0

# Function to extract bash code blocks from markdown
extract_bash_blocks() {
    local file="$1"
    # Extract content between ```bash and ``` markers
    awk '/^```bash$/,/^```$/' "$file" | grep -v '^```'
}

# Function to validate xchecker commands
validate_xchecker_commands() {
    local file="$1"
    echo ""
    echo "Checking: $file"
    
    # Extract bash blocks
    local blocks
    blocks=$(extract_bash_blocks "$file")
    
    if [ -z "$blocks" ]; then
        echo -e "  ${YELLOW}No bash code blocks found${NC}"
        return
    fi
    
    # Check for xchecker commands
    local xchecker_cmds
    xchecker_cmds=$(echo "$blocks" | grep -E '^\s*xchecker\s+' || true)
    
    if [ -z "$xchecker_cmds" ]; then
        echo -e "  ${YELLOW}No xchecker commands found${NC}"
        return
    fi
    
    # Validate each xchecker command
    while IFS= read -r cmd; do
        # Skip empty lines and comments
        [[ -z "$cmd" || "$cmd" =~ ^[[:space:]]*# ]] && continue
        
        # Extract the subcommand
        local subcmd
        subcmd=$(echo "$cmd" | sed 's/^[[:space:]]*//' | awk '{print $2}')
        
        # List of valid xchecker subcommands
        local valid_cmds="spec resume status clean doctor init benchmark test gate project template"
        
        # List of valid global flags
        local valid_flags="--version --help -h -V"
        
        if echo "$valid_cmds" | grep -qw "$subcmd"; then
            echo -e "  ${GREEN}✓${NC} Valid command: xchecker $subcmd"
        elif echo "$valid_flags" | grep -qw "$subcmd"; then
            echo -e "  ${GREEN}✓${NC} Valid flag: xchecker $subcmd"
        else
            echo -e "  ${RED}✗${NC} Unknown command: xchecker $subcmd"
            ((ERRORS++))
        fi
    done <<< "$xchecker_cmds"
}

# Function to check for broken internal links
check_internal_links() {
    local file="$1"
    echo ""
    echo "Checking internal links in: $file"
    
    # Extract markdown links to .md files
    local links
    links=$(grep -oE '\[.*\]\([^)]+\.md\)' "$file" | sed 's/.*(\([^)]*\))/\1/' || true)
    
    if [ -z "$links" ]; then
        echo -e "  ${YELLOW}No internal links found${NC}"
        return
    fi
    
    local dir
    dir=$(dirname "$file")
    
    while IFS= read -r link; do
        # Skip empty lines
        [[ -z "$link" ]] && continue
        
        # Resolve relative path
        local target
        if [[ "$link" == /* ]]; then
            target="$PROJECT_ROOT$link"
        else
            target="$dir/$link"
        fi
        
        # Normalize path
        target=$(cd "$(dirname "$target")" 2>/dev/null && pwd)/$(basename "$target") 2>/dev/null || target="$link"
        
        if [ -f "$target" ]; then
            echo -e "  ${GREEN}✓${NC} Link exists: $link"
        else
            echo -e "  ${RED}✗${NC} Broken link: $link"
            ((ERRORS++))
        fi
    done <<< "$links"
}

# Function to check JSON examples are valid
check_json_examples() {
    local file="$1"
    echo ""
    echo "Checking JSON examples in: $file"
    
    # Extract JSON blocks
    local json_blocks
    json_blocks=$(awk '/^```json$/,/^```$/' "$file" | grep -v '^```' || true)
    
    if [ -z "$json_blocks" ]; then
        echo -e "  ${YELLOW}No JSON code blocks found${NC}"
        return
    fi
    
    # Count JSON blocks
    local block_count
    block_count=$(awk '/^```json$/{count++} END{print count}' "$file")
    
    echo -e "  Found $block_count JSON block(s)"
    
    # Note: Full JSON validation would require jq or similar
    # For now, just count and report
    echo -e "  ${GREEN}✓${NC} JSON blocks present (manual validation recommended)"
}

# Main validation
echo ""
echo "=== Walkthrough Files ==="

WALKTHROUGH_FILES=(
    "$PROJECT_ROOT/docs/WALKTHROUGH_20_MINUTES.md"
    "$PROJECT_ROOT/docs/WALKTHROUGH_SPEC_TO_PR.md"
)

for file in "${WALKTHROUGH_FILES[@]}"; do
    if [ -f "$file" ]; then
        validate_xchecker_commands "$file"
        check_internal_links "$file"
        check_json_examples "$file"
    else
        echo -e "${RED}File not found: $file${NC}"
        ((ERRORS++))
    fi
done

# Summary
echo ""
echo "=== Summary ==="
if [ $ERRORS -eq 0 ]; then
    echo -e "${GREEN}All walkthrough validations passed!${NC}"
    exit 0
else
    echo -e "${RED}Found $ERRORS error(s)${NC}"
    exit 1
fi

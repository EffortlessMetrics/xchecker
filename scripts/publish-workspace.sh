#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
workspace_root="$(cd -- "$script_dir/.." && pwd)"
tiers_file="$script_dir/publish-tiers.txt"

mode="plan"
allow_dirty=0
from_tier=1
index_wait_seconds=30

usage() {
    cat <<'EOF'
Usage: scripts/publish-workspace.sh [--dry-run | --execute] [--allow-dirty] [--from-tier N] [--index-wait-seconds N]

Modes:
  default     Print the publish order without invoking cargo
  --dry-run   Run `cargo publish --dry-run` for each crate in tier order
  --execute   Publish each crate in tier order, waiting between tiers

Notes:
  - `--dry-run` only succeeds for higher tiers after lower-tier crates for the
    same version are already indexed on crates.io.
  - Publishes use `--locked` by default so the release uses the checked-in
    lockfile state.
  - For local packaging verification that does not require prior publication,
    run `cargo package --workspace --allow-dirty --no-verify`.
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run)
            mode="dry-run"
            ;;
        --execute)
            mode="execute"
            ;;
        --allow-dirty)
            allow_dirty=1
            ;;
        --from-tier)
            shift
            from_tier="${1:?missing value for --from-tier}"
            ;;
        --index-wait-seconds)
            shift
            index_wait_seconds="${1:?missing value for --index-wait-seconds}"
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        *)
            echo "Unknown argument: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
    shift
done

if ! [[ "$from_tier" =~ ^[0-9]+$ ]] || (( from_tier < 1 )); then
    echo "--from-tier must be a positive integer" >&2
    exit 2
fi

if ! [[ "$index_wait_seconds" =~ ^[0-9]+$ ]] || (( index_wait_seconds < 0 )); then
    echo "--index-wait-seconds must be a non-negative integer" >&2
    exit 2
fi

mapfile -t tiers < <(grep -Ev '^[[:space:]]*($|#)' "$tiers_file")

if (( from_tier > ${#tiers[@]} )); then
    echo "--from-tier exceeds the number of publish tiers (${#tiers[@]})" >&2
    exit 2
fi

if [[ "$mode" == "dry-run" ]]; then
    echo "Running cargo publish --dry-run in tier order."
    echo "Higher tiers will fail until lower tiers for this version are already indexed."
fi

for ((tier_index = from_tier - 1; tier_index < ${#tiers[@]}; tier_index++)); do
    tier_number=$((tier_index + 1))
    tier_crates="${tiers[tier_index]}"
    echo "Tier ${tier_number}: ${tier_crates}"

    for crate in $tier_crates; do
        cmd=(cargo publish --locked -p "$crate")
        if [[ "$mode" == "dry-run" ]]; then
            cmd+=(--dry-run)
        fi
        if (( allow_dirty )); then
            cmd+=(--allow-dirty)
        fi

        echo "+ ${cmd[*]}"
        if [[ "$mode" != "plan" ]]; then
            (
                cd "$workspace_root"
                "${cmd[@]}"
            )
        fi
    done

    if [[ "$mode" == "execute" && "$tier_index" -lt $((${#tiers[@]} - 1)) && "$index_wait_seconds" -gt 0 ]]; then
        echo "Waiting ${index_wait_seconds}s for crates.io indexing..."
        sleep "$index_wait_seconds"
    fi
done

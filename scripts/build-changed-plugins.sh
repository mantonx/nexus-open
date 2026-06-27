#!/usr/bin/env bash
# Build only the plugins whose source files are newer than their installed binary.
# Called by air as a pre_cmd before each daemon restart.
# Outputs to NEXUS_PLUGINS_DIR (default: ./tmp/plugins) using atomic rename so
# the daemon never sees a partially-written binary.

set -euo pipefail

PLUGINS_DIR="${NEXUS_PLUGINS_DIR:-./tmp/plugins}"
PLUGINS_SRC="./plugins"
SCRATCH=$(mktemp -d)
trap 'rm -rf "$SCRATCH"' EXIT

mkdir -p "$PLUGINS_DIR"

built=0
for src_dir in "$PLUGINS_SRC"/*/; do
    name=$(basename "$src_dir")
    # Skip directories with no Go files (e.g. assets-only dirs).
    go_files=("$src_dir"*.go)
    [[ -f "${go_files[0]}" ]] || continue

    dest="$PLUGINS_DIR/nexus-$name"
    # Rebuild if dest doesn't exist or any .go file in the plugin is newer.
    needs_build=false
    if [[ ! -f "$dest" ]]; then
        needs_build=true
    else
        while IFS= read -r -d '' f; do
            if [[ "$f" -nt "$dest" ]]; then
                needs_build=true
                break
            fi
        done < <(find "$src_dir" -name '*.go' -print0)
    fi

    if [[ "$needs_build" == true ]]; then
        echo "  → building plugin: $name"
        tmp="$SCRATCH/nexus-$name"
        (cd "$src_dir" && go build -o "$tmp" .) || { echo "  ✗ $name build failed"; exit 1; }
        mv "$tmp" "$dest"
        built=$((built + 1))
    fi
done

if [[ $built -eq 0 ]]; then
    echo "  plugins up to date"
else
    echo "  built $built plugin(s)"
fi

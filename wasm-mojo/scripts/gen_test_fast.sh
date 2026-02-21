#!/usr/bin/env bash
# gen_test_fast.sh — Generate test/fast/test_grp_*.mojo binary runners.
#
# Scans test/test_*.mojo for all `fn test_*` and `def test_*` functions,
# then generates one test/fast/test_grp_<stem>.mojo per source module.
# Each generated file has a `fn main() raises` that creates a single
# WasmInstance and calls every test function from that module.
#
# These files can be compiled with `mojo build` into standalone binaries
# for fast iterative testing (~70ms per binary vs ~11s compilation).
#
# Usage:
#   bash scripts/gen_test_fast.sh
#   mojo build -I ../wasmtime-mojo/src -I test -o build/test-bin/test_grp_scheduler test/fast/test_grp_scheduler.mojo
#
# Or via justfile:
#   just test-build   (calls build_test_binaries.sh which calls this first)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
TEST_DIR="$PROJECT_DIR/test"
FAST_DIR="$TEST_DIR/fast"

mkdir -p "$FAST_DIR"

# ---------------------------------------------------------------------------
# Collect test functions grouped by file
# ---------------------------------------------------------------------------

declare -A FILE_FUNCS   # file_stem -> newline-separated function names
declare -a FILE_ORDER   # preserve discovery order

for test_file in "$TEST_DIR"/test_*.mojo; do
    [ -f "$test_file" ] || continue
    stem="$(basename "$test_file" .mojo)"

    # Skip generated files and non-test modules
    case "$stem" in
        test_all|test_all_small|test_all_probe) continue ;;
    esac

    # Extract function names: lines starting with `fn test_` or `def test_`
    funcs="$(grep -Po '^(?:fn|def) \Ktest_\w+' "$test_file" || true)"
    if [ -z "$funcs" ]; then
        continue
    fi

    FILE_FUNCS["$stem"]="$funcs"
    FILE_ORDER+=("$stem")
done

# ---------------------------------------------------------------------------
# Count totals
# ---------------------------------------------------------------------------

total_tests=0
for stem in "${FILE_ORDER[@]}"; do
    count=$(echo "${FILE_FUNCS[$stem]}" | wc -w)
    total_tests=$((total_tests + count))
done

# ---------------------------------------------------------------------------
# Track what we generate so we can remove stale files
# ---------------------------------------------------------------------------

generated_files=()

# ---------------------------------------------------------------------------
# Generate one runner per module
# ---------------------------------------------------------------------------

for stem in "${FILE_ORDER[@]}"; do
    funcs="${FILE_FUNCS[$stem]}"
    count=$(echo "$funcs" | wc -w)

    # Strip leading "test_" to form the group name
    # e.g. test_scheduler -> scheduler
    short="${stem#test_}"
    out_file="$FAST_DIR/test_grp_${short}.mojo"
    generated_files+=("$out_file")

    tmp_file="$out_file.tmp"

    cat > "$tmp_file" << HEADER
from memory import UnsafePointer
from wasm_harness import WasmInstance, get_instance
import $stem

fn main() raises:
    var w = get_instance()
HEADER

    for fn_name in $funcs; do
        echo "    ${stem}.${fn_name}(w)" >> "$tmp_file"
    done

    echo "    print(\"${short}: ${count}/${count} passed\")" >> "$tmp_file"

    # Only overwrite if content changed (preserves timestamp for incremental builds)
    if [ -f "$out_file" ] && cmp -s "$tmp_file" "$out_file"; then
        rm "$tmp_file"
    else
        mv "$tmp_file" "$out_file"
    fi
done

# ---------------------------------------------------------------------------
# Remove stale generated files (modules that no longer exist)
# ---------------------------------------------------------------------------

removed=0
for existing in "$FAST_DIR"/test_grp_*.mojo; do
    [ -f "$existing" ] || continue
    found=0
    for gen in "${generated_files[@]}"; do
        if [ "$existing" = "$gen" ]; then
            found=1
            break
        fi
    done
    if [ $found -eq 0 ]; then
        rm "$existing"
        removed=$((removed + 1))
    fi
done

echo "Generated ${#FILE_ORDER[@]} fast runners (${total_tests} tests) in test/fast/"
if [ $removed -gt 0 ]; then
    echo "Removed $removed stale runner(s)."
fi

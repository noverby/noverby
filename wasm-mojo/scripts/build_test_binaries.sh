#!/usr/bin/env bash
# build_test_binaries.sh — Compile test/fast/*.mojo into standalone binaries.
#
# Each test group file has a main() that creates one WasmInstance and
# calls all tests in that module sequentially.  Precompiling avoids
# the ~11s Mojo compilation overhead on every run.
#
# Binaries are placed in build/test-bin/.  Compilation runs in parallel
# (up to $JOBS processes, default: nproc).
#
# Usage:
#   bash scripts/build_test_binaries.sh          # build all
#   bash scripts/build_test_binaries.sh -j 4     # limit to 4 jobs
#   bash scripts/build_test_binaries.sh -f        # force rebuild (ignore timestamps)
#
# Or via justfile:
#   just test-build

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
FAST_DIR="$PROJECT_DIR/test/fast"

# Regenerate test/fast/ runners so they stay in sync with test sources.
bash "$SCRIPT_DIR/gen_test_fast.sh"
OUT_DIR="$PROJECT_DIR/build/test-bin"
WASMTIME_MOJO="$PROJECT_DIR/../wasmtime-mojo/src"
TEST_DIR="$PROJECT_DIR/test"

JOBS=$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 8)
FORCE=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        -j|--jobs)  JOBS="$2"; shift 2 ;;
        -f|--force) FORCE=1; shift ;;
        -h|--help)
            echo "Usage: $0 [-j JOBS] [-f|--force]"
            echo "  -j JOBS   Max parallel compilations (default: nproc=$JOBS)"
            echo "  -f        Force rebuild even if binary is up-to-date"
            exit 0
            ;;
        *) echo "Unknown option: $1" >&2; exit 1 ;;
    esac
done

mkdir -p "$OUT_DIR"

# Collect source files
sources=("$FAST_DIR"/test_grp_*.mojo)
total=${#sources[@]}

if [[ $total -eq 0 ]]; then
    echo "No test group files found in $FAST_DIR" >&2
    exit 1
fi

echo "Building $total test binaries (jobs=$JOBS)..."

# Track results
built=0
skipped=0
failed=0
failed_names=()
pids=()
names=()
running=0

# Wait for one slot to free up, collecting its result
wait_for_slot() {
    if [[ $running -ge $JOBS ]]; then
        # Wait for any one child
        local done_pid
        for i in "${!pids[@]}"; do
            if ! kill -0 "${pids[$i]}" 2>/dev/null; then
                done_pid=$i
                break
            fi
        done
        # If all still running, wait for any
        if [[ -z "${done_pid:-}" ]]; then
            wait -n -p WAITED_PID 2>/dev/null || true
            for i in "${!pids[@]}"; do
                if ! kill -0 "${pids[$i]}" 2>/dev/null; then
                    done_pid=$i
                    break
                fi
            done
        fi
        if [[ -n "${done_pid:-}" ]]; then
            wait "${pids[$done_pid]}" 2>/dev/null && {
                built=$((built + 1))
            } || {
                failed=$((failed + 1))
                failed_names+=("${names[$done_pid]}")
                echo "  FAIL: ${names[$done_pid]}" >&2
            }
            unset 'pids[done_pid]'
            unset 'names[done_pid]'
            running=$((running - 1))
        fi
    fi
}

for src in "${sources[@]}"; do
    name=$(basename "$src" .mojo)
    bin="$OUT_DIR/$name"

    # Skip if binary is newer than all relevant sources (unless --force)
    if [[ $FORCE -eq 0 && -f "$bin" ]]; then
        # Check against: the source file itself, wasm_harness, and the test module it imports
        stem="${name#test_grp_}"
        test_module="$TEST_DIR/test_${stem}.mojo"
        harness="$TEST_DIR/wasm_harness.mojo"

        needs_rebuild=0
        for dep in "$src" "$harness" "$test_module"; do
            if [[ -f "$dep" && "$dep" -nt "$bin" ]]; then
                needs_rebuild=1
                break
            fi
        done

        if [[ $needs_rebuild -eq 0 ]]; then
            skipped=$((skipped + 1))
            continue
        fi
    fi

    wait_for_slot

    (
        mojo build -I "$WASMTIME_MOJO" -I "$TEST_DIR" -o "$bin" "$src" 2>&1 \
            | sed "s/^/  [$name] /"
    ) &
    pids+=($!)
    names+=("$name")
    running=$((running + 1))
done

# Wait for all remaining jobs
for i in "${!pids[@]}"; do
    wait "${pids[$i]}" 2>/dev/null && {
        built=$((built + 1))
    } || {
        failed=$((failed + 1))
        failed_names+=("${names[$i]}")
        echo "  FAIL: ${names[$i]}" >&2
    }
done

echo ""
echo "Done: $built built, $skipped skipped, $failed failed (of $total total)"

if [[ $failed -gt 0 ]]; then
    echo ""
    echo "Failed binaries:" >&2
    for n in "${failed_names[@]}"; do
        echo "  - $n" >&2
    done
    exit 1
fi

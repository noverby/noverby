#!/usr/bin/env bash
# build_test_binaries.sh — Compile test/test_*.mojo into standalone binaries.
#
# Each test module has a fn main() that creates one WasmInstance and
# calls all tests in that module sequentially.  Precompiling avoids
# the ~11s Mojo compilation overhead on every run.
#
# Binaries are placed in build/test-bin/.  Compilation runs in parallel
# (up to $JOBS processes, default: nproc).
#
# Usage:
#   bash scripts/build_test_binaries.sh              # build all
#   bash scripts/build_test_binaries.sh -j 4         # limit to 4 jobs
#   bash scripts/build_test_binaries.sh -f            # force rebuild (ignore timestamps)
#   bash scripts/build_test_binaries.sh signals       # build only test_signals
#   bash scripts/build_test_binaries.sh signals mut   # build test_signals + test_mutations
#   bash scripts/build_test_binaries.sh -f dsl        # force rebuild test_dsl only
#
# Filter arguments are matched as substrings against source file names.
# "signals" matches "test_signals.mojo", "mut" matches "test_mutations.mojo", etc.
#
# Or via justfile:
#   just test-build
#   just test-build signals

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
TEST_DIR="$PROJECT_DIR/test"
SRC_DIR="$PROJECT_DIR/src"
OUT_DIR="$PROJECT_DIR/build/test-bin"
WASMTIME_MOJO="$PROJECT_DIR/../wasmtime-mojo/src"

JOBS=$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 8)
FORCE=0
FILTERS=()

while [[ $# -gt 0 ]]; do
    case "$1" in
        -j|--jobs)  JOBS="$2"; shift 2 ;;
        -f|--force) FORCE=1; shift ;;
        -h|--help)
            echo "Usage: $0 [-j JOBS] [-f|--force] [FILTER...]"
            echo "  -j JOBS   Max parallel compilations (default: nproc=$JOBS)"
            echo "  -f        Force rebuild even if binary is up-to-date"
            echo "  FILTER    Substring match against test module names"
            echo ""
            echo "Examples:"
            echo "  $0                  # build all test binaries"
            echo "  $0 signals          # build test_signals only"
            echo "  $0 signals mut      # build test_signals and test_mutations"
            echo "  $0 -f dsl           # force rebuild test_dsl"
            exit 0
            ;;
        -*) echo "Unknown option: $1" >&2; exit 1 ;;
        *) FILTERS+=("$1"); shift ;;
    esac
done

mkdir -p "$OUT_DIR"

# Collect source files — every test/test_*.mojo that contains fn main()
all_sources=()
for f in "$TEST_DIR"/test_*.mojo; do
    [ -f "$f" ] || continue
    grep -q '^fn main' "$f" && all_sources+=("$f")
done

# Apply filter if provided
sources=()
if [[ ${#FILTERS[@]} -gt 0 ]]; then
    for f in "${all_sources[@]}"; do
        name=$(basename "$f")
        for filter in "${FILTERS[@]}"; do
            if [[ "$name" == *"$filter"* ]]; then
                sources+=("$f")
                break
            fi
        done
    done
else
    sources=("${all_sources[@]}")
fi

total=${#sources[@]}

if [[ $total -eq 0 ]]; then
    if [[ ${#FILTERS[@]} -gt 0 ]]; then
        echo "No test files matching filter(s): ${FILTERS[*]}" >&2
        echo "Available test modules:" >&2
        for f in "${all_sources[@]}"; do
            echo "  $(basename "$f" .mojo)" >&2
        done
    else
        echo "No test files with fn main() found in $TEST_DIR" >&2
    fi
    exit 1
fi

if [[ ${#FILTERS[@]} -gt 0 ]]; then
    echo "Building $total test binaries (filter: ${FILTERS[*]}, jobs=$JOBS)..."
else
    echo "Building $total test binaries (jobs=$JOBS)..."
fi

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
        harness="$TEST_DIR/wasm_harness.mojo"

        needs_rebuild=0
        for dep in "$src" "$harness"; do
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
        mojo build -I "$WASMTIME_MOJO" -I "$SRC_DIR" -I "$TEST_DIR" -o "$bin" "$src" 2>&1 \
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

#!/usr/bin/env bash
# run_test_binaries.sh — Run precompiled test binaries in parallel.
#
# Executes all binaries in build/test-bin/ concurrently, waits for each
# in order, and reports pass/fail.  Returns non-zero if any binary fails.
#
# Usage:
#   bash scripts/run_test_binaries.sh          # run all
#   bash scripts/run_test_binaries.sh -v        # verbose (show all output)
#
# Or via justfile:
#   just test-run

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BIN_DIR="$PROJECT_DIR/build/test-bin"

VERBOSE=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        -v|--verbose) VERBOSE=1; shift ;;
        -h|--help)
            echo "Usage: $0 [-v|--verbose]"
            echo "  -v   Show full output from each binary"
            exit 0
            ;;
        *) echo "Unknown option: $1" >&2; exit 1 ;;
    esac
done

if [[ ! -d "$BIN_DIR" ]]; then
    echo "No test binaries found.  Run 'just test-build' first." >&2
    exit 1
fi

# Collect binaries
binaries=()
for bin in "$BIN_DIR"/test_*; do
    [[ -x "$bin" ]] && binaries+=("$bin")
done

total=${#binaries[@]}

if [[ $total -eq 0 ]]; then
    echo "No executable test binaries found in $BIN_DIR" >&2
    echo "Run 'just test-build' first." >&2
    exit 1
fi

echo "Running $total test binaries..."
echo ""

# Create a temp directory for per-binary output
tmp_dir=$(mktemp -d)
trap 'rm -rf "$tmp_dir"' EXIT

start_ns=$(date +%s%N 2>/dev/null || echo 0)

# Launch all binaries in parallel
pids=()
names=()
outfiles=()

for bin in "${binaries[@]}"; do
    name=$(basename "$bin")
    outfile="$tmp_dir/$name.out"

    # Run from the project directory so "build/out.wasm" paths resolve
    (cd "$PROJECT_DIR" && "$bin" > "$outfile" 2>&1) &
    pids+=($!)
    names+=("$name")
    outfiles+=("$outfile")
done

# Wait for each in order and collect results
passed=0
failed=0
failed_names=()

for i in "${!pids[@]}"; do
    pid=${pids[$i]}
    name=${names[$i]}
    outfile=${outfiles[$i]}

    if wait "$pid" 2>/dev/null; then
        passed=$((passed + 1))
        if [[ $VERBOSE -eq 1 ]]; then
            echo "  ✓ $name"
            sed 's/^/    /' "$outfile"
        else
            summary=$(grep -v '^$' "$outfile" | tail -1 || echo "ok")
            echo "  ✓ $name — $summary"
        fi
    else
        failed=$((failed + 1))
        failed_names+=("$name")
        echo "  ✗ $name — FAILED"
        sed 's/^/    /' "$outfile"
    fi
done

end_ns=$(date +%s%N 2>/dev/null || echo 0)

echo ""

# Calculate and display elapsed time
if [[ "$start_ns" != "0" && "$end_ns" != "0" ]]; then
    elapsed_ms=$(( (end_ns - start_ns) / 1000000 ))
    if [[ $elapsed_ms -lt 1000 ]]; then
        echo "Completed in ${elapsed_ms}ms: $passed passed, $failed failed (of $total total)"
    else
        elapsed_s=$(echo "scale=1; $elapsed_ms / 1000" | bc 2>/dev/null || echo "$((elapsed_ms / 1000))")
        echo "Completed in ${elapsed_s}s: $passed passed, $failed failed (of $total total)"
    fi
else
    echo "Completed: $passed passed, $failed failed (of $total total)"
fi

if [[ $failed -gt 0 ]]; then
    echo ""
    echo "Failed:" >&2
    for n in "${failed_names[@]}"; do
        echo "  - $n" >&2
    done
    exit 1
fi

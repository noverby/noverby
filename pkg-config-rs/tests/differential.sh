#!/usr/bin/env bash
#
# Differential testing script for pkg-config-rs.
#
# This script compares the output of our `pkgconf` binary against the system
# `pkg-config` (or `pkgconf`) for a variety of queries. It reports any
# differences in stdout, stderr, or exit code.
#
# Usage:
#   ./tests/differential.sh [--test-data-only] [--system-packages] [--verbose]
#
# Options:
#   --test-data-only    Only test against .pc files in tests/data/
#   --system-packages   Also test against system-installed .pc files
#   --verbose           Print each test as it runs
#   --stop-on-fail      Stop on first failure
#
# Prerequisites:
#   - cargo build must have been run
#   - pkg-config or pkgconf must be installed on the system

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TEST_DATA_DIR="$WORKSPACE_ROOT/tests/data"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Options
TEST_DATA_ONLY=false
SYSTEM_PACKAGES=false
VERBOSE=false
STOP_ON_FAIL=false

for arg in "$@"; do
    case "$arg" in
        --test-data-only)  TEST_DATA_ONLY=true ;;
        --system-packages) SYSTEM_PACKAGES=true ;;
        --verbose)         VERBOSE=true ;;
        --stop-on-fail)    STOP_ON_FAIL=true ;;
        --help|-h)
            echo "Usage: $0 [--test-data-only] [--system-packages] [--verbose] [--stop-on-fail]"
            exit 0
            ;;
        *)
            echo "Unknown option: $arg"
            exit 1
            ;;
    esac
done

# Find our binary
OUR_BIN="$WORKSPACE_ROOT/target/debug/pkgconf"
if [[ ! -x "$OUR_BIN" ]]; then
    echo "Building pkgconf..."
    cargo build --manifest-path "$WORKSPACE_ROOT/Cargo.toml" 2>/dev/null
fi

if [[ ! -x "$OUR_BIN" ]]; then
    echo "Error: Could not find or build $OUR_BIN"
    exit 1
fi

# Find system pkg-config
SYS_BIN=""
for candidate in pkgconf pkg-config; do
    if command -v "$candidate" &>/dev/null; then
        SYS_BIN="$(command -v "$candidate")"
        break
    fi
done

if [[ -z "$SYS_BIN" ]]; then
    echo "Error: Neither pkgconf nor pkg-config found on the system."
    echo "Install one of them to run differential tests."
    exit 1
fi

echo -e "${CYAN}Differential Testing${NC}"
echo "  Our binary:    $OUR_BIN"
echo "  System binary: $SYS_BIN"
echo "  Test data:     $TEST_DATA_DIR"
echo ""

# Counters
PASS=0
FAIL=0
SKIP=0
TOTAL=0

# Temporary files for capturing output
OUR_STDOUT=$(mktemp)
OUR_STDERR=$(mktemp)
SYS_STDOUT=$(mktemp)
SYS_STDERR=$(mktemp)
trap "rm -f $OUR_STDOUT $OUR_STDERR $SYS_STDOUT $SYS_STDERR" EXIT

# Compare a single invocation
# Args: description env_vars... -- args...
compare() {
    local desc="$1"
    shift

    # Collect environment variables until we hit "--"
    local -a env_args=()
    while [[ $# -gt 0 && "$1" != "--" ]]; do
        env_args+=("$1")
        shift
    done
    # Skip the "--" separator
    if [[ $# -gt 0 ]]; then
        shift
    fi

    local -a cmd_args=("$@")

    TOTAL=$((TOTAL + 1))

    if $VERBOSE; then
        echo -n "  Testing: $desc ... "
    fi

    # Build env command prefix
    local -a env_prefix=("env")
    for ev in "${env_args[@]}"; do
        env_prefix+=("$ev")
    done

    # Run our binary
    local our_exit=0
    "${env_prefix[@]}" "$OUR_BIN" "${cmd_args[@]}" >"$OUR_STDOUT" 2>"$OUR_STDERR" || our_exit=$?

    # Run system binary
    local sys_exit=0
    "${env_prefix[@]}" "$SYS_BIN" "${cmd_args[@]}" >"$SYS_STDOUT" 2>"$SYS_STDERR" || sys_exit=$?

    # Compare exit codes
    if [[ $our_exit -ne $sys_exit ]]; then
        FAIL=$((FAIL + 1))
        if $VERBOSE; then
            echo -e "${RED}FAIL${NC} (exit code: ours=$our_exit, theirs=$sys_exit)"
        else
            echo -e "${RED}FAIL${NC}: $desc (exit code: ours=$our_exit, theirs=$sys_exit)"
        fi
        if $VERBOSE; then
            echo "    Our stderr: $(cat "$OUR_STDERR" | head -3)"
            echo "    Sys stderr: $(cat "$SYS_STDERR" | head -3)"
        fi
        if $STOP_ON_FAIL; then
            echo ""
            echo "Stopping on first failure."
            echo "  Our stdout: $(cat "$OUR_STDOUT")"
            echo "  Sys stdout: $(cat "$SYS_STDOUT")"
            echo "  Our stderr: $(cat "$OUR_STDERR")"
            echo "  Sys stderr: $(cat "$SYS_STDERR")"
            exit 1
        fi
        return
    fi

    # Compare stdout (normalize whitespace)
    local our_out sys_out
    our_out="$(cat "$OUR_STDOUT" | sed 's/[[:space:]]*$//' | sort)"
    sys_out="$(cat "$SYS_STDOUT" | sed 's/[[:space:]]*$//' | sort)"

    if [[ "$our_out" != "$sys_out" ]]; then
        # Try comparing without sorting (order might differ but content same)
        our_out="$(cat "$OUR_STDOUT" | sed 's/[[:space:]]*$//')"
        sys_out="$(cat "$SYS_STDOUT" | sed 's/[[:space:]]*$//')"

        # Normalize: split flags by space, sort, rejoin
        local our_norm sys_norm
        our_norm="$(echo "$our_out" | tr ' ' '\n' | sort | tr '\n' ' ' | sed 's/[[:space:]]*$//')"
        sys_norm="$(echo "$sys_out" | tr ' ' '\n' | sort | tr '\n' ' ' | sed 's/[[:space:]]*$//')"

        if [[ "$our_norm" != "$sys_norm" ]]; then
            FAIL=$((FAIL + 1))
            if $VERBOSE; then
                echo -e "${RED}FAIL${NC} (output differs)"
                echo "    Ours:   $(cat "$OUR_STDOUT" | head -3)"
                echo "    Theirs: $(cat "$SYS_STDOUT" | head -3)"
            else
                echo -e "${RED}FAIL${NC}: $desc (output differs)"
                echo "    Ours:   $(cat "$OUR_STDOUT" | head -1)"
                echo "    Theirs: $(cat "$SYS_STDOUT" | head -1)"
            fi
            if $STOP_ON_FAIL; then
                echo ""
                echo "Stopping on first failure."
                echo "  Our full stdout: $(cat "$OUR_STDOUT")"
                echo "  Sys full stdout: $(cat "$SYS_STDOUT")"
                exit 1
            fi
            return
        fi
    fi

    PASS=$((PASS + 1))
    if $VERBOSE; then
        echo -e "${GREEN}PASS${NC}"
    fi
}

# ============================================================================
# Test data packages
# ============================================================================

echo -e "${CYAN}=== Testing against test data packages ===${NC}"

PKG_ENV="PKG_CONFIG_PATH=$TEST_DATA_DIR"
LIB_ENV="PKG_CONFIG_LIBDIR=$TEST_DATA_DIR"
# Common env that isolates from system
COMMON_ENV=("$PKG_ENV" "$LIB_ENV"
    "PKG_CONFIG_SYSROOT_DIR="
    "PKG_CONFIG_ALLOW_SYSTEM_CFLAGS="
    "PKG_CONFIG_ALLOW_SYSTEM_LIBS="
    "PKG_CONFIG_DISABLE_UNINSTALLED=1"
    "PKG_CONFIG_MSVC_SYNTAX="
    "PKG_CONFIG_FDO_SYSROOT_RULES="
    "PKG_CONFIG_LOG="
    "PKG_CONFIG_PRELOADED_FILES="
    "PKG_CONFIG_PURE_DEPGRAPH="
    "PKG_CONFIG_IGNORE_CONFLICTS="
    "PKG_CONFIG_DEBUG_SPEW="
)

# Basic queries
echo -e "\n${YELLOW}--- Basic Queries ---${NC}"

compare "version flag" -- --version

compare "modversion simple" \
    "${COMMON_ENV[@]}" -- --modversion simple

compare "modversion zlib" \
    "${COMMON_ENV[@]}" -- --modversion zlib

compare "modversion depender" \
    "${COMMON_ENV[@]}" -- --modversion depender

compare "modversion libbar" \
    "${COMMON_ENV[@]}" -- --modversion libbar

# Cflags queries
echo -e "\n${YELLOW}--- Cflags Queries ---${NC}"

compare "cflags simple" \
    "${COMMON_ENV[@]}" -- --cflags simple

compare "cflags zlib" \
    "${COMMON_ENV[@]}" -- --cflags zlib

compare "cflags depender" \
    "${COMMON_ENV[@]}" -- --cflags depender

compare "cflags-only-I libbar" \
    "${COMMON_ENV[@]}" -- --cflags-only-I libbar

compare "cflags-only-other libbar" \
    "${COMMON_ENV[@]}" -- --cflags-only-other libbar

compare "cflags nocflags (empty)" \
    "${COMMON_ENV[@]}" -- --cflags nocflags

# Libs queries
echo -e "\n${YELLOW}--- Libs Queries ---${NC}"

compare "libs simple" \
    "${COMMON_ENV[@]}" -- --libs simple

compare "libs zlib" \
    "${COMMON_ENV[@]}" -- --libs zlib

compare "libs depender" \
    "${COMMON_ENV[@]}" -- --libs depender

compare "libs-only-l simple" \
    "${COMMON_ENV[@]}" -- --libs-only-l simple

compare "libs-only-L simple (keep-system)" \
    "${COMMON_ENV[@]}" "PKG_CONFIG_ALLOW_SYSTEM_LIBS=1" -- --libs-only-L simple

compare "libs nolibs (empty)" \
    "${COMMON_ENV[@]}" -- --libs nolibs

# Combined cflags and libs
echo -e "\n${YELLOW}--- Combined Cflags+Libs ---${NC}"

compare "cflags+libs simple" \
    "${COMMON_ENV[@]}" -- --cflags --libs simple

compare "cflags+libs depender" \
    "${COMMON_ENV[@]}" -- --cflags --libs depender

# Variable queries
echo -e "\n${YELLOW}--- Variable Queries ---${NC}"

compare "variable prefix simple" \
    "${COMMON_ENV[@]}" -- --variable=prefix simple

compare "variable libdir simple" \
    "${COMMON_ENV[@]}" -- --variable=libdir simple

compare "variable includedir simple" \
    "${COMMON_ENV[@]}" -- --variable=includedir simple

compare "variable prefix zlib" \
    "${COMMON_ENV[@]}" -- --variable=prefix zlib

# Exists queries
echo -e "\n${YELLOW}--- Exists Queries ---${NC}"

compare "exists simple" \
    "${COMMON_ENV[@]}" -- --exists simple

compare "exists nonexistent" \
    "${COMMON_ENV[@]}" -- --exists nonexistent-package-xyz

compare "exists simple >= 1.0" \
    "${COMMON_ENV[@]}" -- --exists "simple >= 1.0"

compare "exists simple >= 99.0 (fail)" \
    "${COMMON_ENV[@]}" -- --exists "simple >= 99.0"

compare "exists simple = 1.0.0" \
    "${COMMON_ENV[@]}" -- --exists "simple = 1.0.0"

compare "exists simple = 2.0.0 (fail)" \
    "${COMMON_ENV[@]}" -- --exists "simple = 2.0.0"

compare "exists simple < 2.0.0" \
    "${COMMON_ENV[@]}" -- --exists "simple < 2.0.0"

compare "exists simple > 0.5.0" \
    "${COMMON_ENV[@]}" -- --exists "simple > 0.5.0"

compare "exists simple != 2.0.0" \
    "${COMMON_ENV[@]}" -- --exists "simple != 2.0.0"

# Version constraint flags
echo -e "\n${YELLOW}--- Version Constraint Flags ---${NC}"

compare "atleast-version satisfied" \
    "${COMMON_ENV[@]}" -- --atleast-version=1.0.0 simple

compare "atleast-version not satisfied" \
    "${COMMON_ENV[@]}" -- --atleast-version=99.0.0 simple

compare "exact-version match" \
    "${COMMON_ENV[@]}" -- --exact-version=1.0.0 simple

compare "exact-version mismatch" \
    "${COMMON_ENV[@]}" -- --exact-version=2.0.0 simple

compare "max-version satisfied" \
    "${COMMON_ENV[@]}" -- --max-version=2.0.0 simple

compare "max-version not satisfied" \
    "${COMMON_ENV[@]}" -- --max-version=0.5.0 simple

# Dependency resolution
echo -e "\n${YELLOW}--- Dependency Resolution ---${NC}"

compare "libs diamond-a (dedup)" \
    "${COMMON_ENV[@]}" -- --libs diamond-a

compare "cflags diamond-a" \
    "${COMMON_ENV[@]}" -- --cflags diamond-a

compare "libs deep-depender (transitive)" \
    "${COMMON_ENV[@]}" -- --libs deep-depender

compare "cflags deep-depender (transitive)" \
    "${COMMON_ENV[@]}" -- --cflags deep-depender

compare "libs metapackage" \
    "${COMMON_ENV[@]}" -- --libs metapackage

# Static linking
echo -e "\n${YELLOW}--- Static Linking ---${NC}"

compare "static libs private-deps" \
    "${COMMON_ENV[@]}" -- --static --libs private-deps

compare "static cflags private-deps" \
    "${COMMON_ENV[@]}" -- --static --cflags private-deps

compare "static libs static-libs" \
    "${COMMON_ENV[@]}" -- --static --libs static-libs

# Print metadata
echo -e "\n${YELLOW}--- Print Metadata ---${NC}"

compare "print-requires depender" \
    "${COMMON_ENV[@]}" -- --print-requires depender

compare "print-requires-private private-deps" \
    "${COMMON_ENV[@]}" -- --print-requires-private private-deps

compare "print-provides provider" \
    "${COMMON_ENV[@]}" -- --print-provides provider

# Define variable
echo -e "\n${YELLOW}--- Define Variable ---${NC}"

compare "define-variable prefix" \
    "${COMMON_ENV[@]}" -- --define-variable=prefix=/custom --variable=prefix simple

compare "define-variable affects cflags" \
    "${COMMON_ENV[@]}" -- --define-variable=prefix=/custom --cflags simple

# System dirs with keep flags
echo -e "\n${YELLOW}--- System Directory Filtering ---${NC}"

compare "keep-system-cflags zlib" \
    "${COMMON_ENV[@]}" -- --keep-system-cflags --cflags zlib

compare "keep-system-libs simple" \
    "${COMMON_ENV[@]}" "PKG_CONFIG_ALLOW_SYSTEM_LIBS=1" -- --keep-system-libs --libs simple

# Parser edge cases
echo -e "\n${YELLOW}--- Parser Edge Cases ---${NC}"

compare "modversion comments" \
    "${COMMON_ENV[@]}" -- --modversion comments

compare "libs comments" \
    "${COMMON_ENV[@]}" -- --libs comments

compare "modversion multiline" \
    "${COMMON_ENV[@]}" -- --modversion multiline

compare "libs multiline" \
    "${COMMON_ENV[@]}" -- --libs multiline

compare "cflags multiline" \
    "${COMMON_ENV[@]}" -- --cflags multiline

compare "modversion dos-lineendings" \
    "${COMMON_ENV[@]}" -- --modversion dos-lineendings

compare "modversion no-trailing-newline" \
    "${COMMON_ENV[@]}" -- --modversion no-trailing-newline

compare "modversion unicode" \
    "${COMMON_ENV[@]}" -- --modversion unicode

# Tilde version
echo -e "\n${YELLOW}--- Tilde Version ---${NC}"

compare "exists tilde-version" \
    "${COMMON_ENV[@]}" -- --exists tilde-version

compare "modversion tilde-version" \
    "${COMMON_ENV[@]}" -- --modversion tilde-version

compare "tilde < release" \
    "${COMMON_ENV[@]}" -- --exists "tilde-version < 1.0.0"

# ============================================================================
# System-installed packages (optional)
# ============================================================================

if $SYSTEM_PACKAGES; then
    echo ""
    echo -e "${CYAN}=== Testing against system-installed packages ===${NC}"

    # Get list of available packages from system pkg-config
    AVAILABLE_PACKAGES=$("$SYS_BIN" --list-all 2>/dev/null | awk '{print $1}' | head -50)

    if [[ -z "$AVAILABLE_PACKAGES" ]]; then
        echo -e "${YELLOW}No system packages found, skipping.${NC}"
    else
        # Reset env to use system paths
        SYS_PKG_ENV=()

        for pkg in $AVAILABLE_PACKAGES; do
            echo -e "\n${YELLOW}--- Package: $pkg ---${NC}"

            compare "modversion $pkg" \
                "${SYS_PKG_ENV[@]}" -- --modversion "$pkg"

            compare "cflags $pkg" \
                "${SYS_PKG_ENV[@]}" -- --cflags "$pkg"

            compare "libs $pkg" \
                "${SYS_PKG_ENV[@]}" -- --libs "$pkg"

            compare "exists $pkg" \
                "${SYS_PKG_ENV[@]}" -- --exists "$pkg"

            compare "variable prefix $pkg" \
                "${SYS_PKG_ENV[@]}" -- --variable=prefix "$pkg"
        done
    fi
fi

# ============================================================================
# Summary
# ============================================================================

echo ""
echo -e "${CYAN}=== Summary ===${NC}"
echo -e "  Total:   $TOTAL"
echo -e "  ${GREEN}Passed:  $PASS${NC}"
echo -e "  ${RED}Failed:  $FAIL${NC}"
echo -e "  ${YELLOW}Skipped: $SKIP${NC}"
echo ""

if [[ $FAIL -gt 0 ]]; then
    echo -e "${RED}Some tests failed!${NC}"
    exit 1
else
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
fi

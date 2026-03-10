#!/usr/bin/env bash
# build_examples.sh — Build all shared examples for the web (WASM) target.
#
# This script discovers example apps in mojo-gui/examples/ and the existing
# web/examples/ directory, compiles the WASM module (which bundles all apps
# via main.mojo), and copies web-specific assets to the output directory.
#
# Architecture:
#
#   Currently, all example apps are compiled into a single WASM binary
#   (build/out.wasm) via web/src/main.mojo, which imports and re-exports
#   each app's lifecycle functions as @export WASM wrappers. The per-example
#   web/ assets (index.html, main.js) load this shared WASM binary and
#   use convention-based export discovery (e.g. "counter_init") to boot
#   the correct app.
#
#   Future: When apps are refactored to use launch(), each example will
#   compile to its own WASM binary, and this script will build them
#   individually.
#
# Usage:
#
#   cd mojo-gui/web
#   bash scripts/build_examples.sh
#
#   # Or build a specific example:
#   bash scripts/build_examples.sh counter
#
#   # Or build multiple:
#   bash scripts/build_examples.sh counter todo
#
# Prerequisites:
#
#   - mojo (Mojo compiler)
#   - llc (LLVM static compiler, wasm64 target)
#   - wasm-ld (WebAssembly linker)
#   - The mojo-gui/core source tree at ../core/
#
# Output:
#
#   build/out.wasm              — compiled WASM binary (all apps)
#   build/examples/<name>/      — per-example output directories
#     ├── index.html            — HTML shell (copied from examples/<name>/web/ or web/examples/<name>/)
#     └── main.js               — JS entry point (copied)
#
# Environment variables:
#
#   MOJO_FLAGS    — extra flags to pass to `mojo build` (default: -Werror)
#   BUILD_DIR     — output directory (default: build)
#   INITIAL_MEM   — WASM initial memory in bytes (default: 268435456 = 256 MiB)

set -euo pipefail

# ── Configuration ─────────────────────────────────────────────────────────

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WEB_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
ROOT_DIR="$(cd "$WEB_DIR/.." && pwd)"
CORE_DIR="$ROOT_DIR/core"
EXAMPLES_DIR="$ROOT_DIR/examples"
WEB_EXAMPLES_DIR="$WEB_DIR/examples"

BUILD_DIR="${BUILD_DIR:-$WEB_DIR/build}"
MOJO_FLAGS="${MOJO_FLAGS:--Werror}"
INITIAL_MEM="${INITIAL_MEM:-268435456}"

# All known examples (from web/examples/ directory)
ALL_EXAMPLES=(counter todo bench app)

# ── Argument parsing ──────────────────────────────────────────────────────

if [ $# -gt 0 ]; then
    EXAMPLES=("$@")
else
    EXAMPLES=("${ALL_EXAMPLES[@]}")
fi

# ── Helpers ───────────────────────────────────────────────────────────────

log() {
    echo "==> $*"
}

err() {
    echo "ERROR: $*" >&2
    exit 1
}

check_tool() {
    command -v "$1" >/dev/null 2>&1 || err "'$1' not found in PATH. Please install it."
}

# ── Preflight checks ─────────────────────────────────────────────────────

check_tool mojo
check_tool llc
check_tool wasm-ld

[ -d "$CORE_DIR/src" ] || err "Core source not found at $CORE_DIR/src — run from mojo-gui/web/"
[ -f "$WEB_DIR/src/main.mojo" ] || err "main.mojo not found at $WEB_DIR/src/main.mojo"

# ── Step 1: Build the shared WASM binary ──────────────────────────────────
#
# All apps are bundled into a single WASM binary via main.mojo.
# This matches the existing `just build` workflow.

log "Building WASM binary (all apps)..."

mkdir -p "$BUILD_DIR"
rm -f "$BUILD_DIR/out.cwasm"

# Compile Mojo → LLVM IR
mojo build $MOJO_FLAGS \
    --emit llvm \
    -I "$CORE_DIR/src" \
    -I "$CORE_DIR" \
    -I "$WEB_EXAMPLES_DIR" \
    -o "$BUILD_DIR/out.ll" \
    "$WEB_DIR/src/main.mojo"

# Patch LLVM IR for WASM compatibility
# - Remove lifetime intrinsics (not supported by wasm backend)
# - Remove nocreateundeforpoison (not recognized by older LLVM)
# - Strip target-cpu/target-features (native CPU attrs break WASM)
# - Remove empty attribute groups
sed -i 's/ nocreateundeforpoison//g' "$BUILD_DIR/out.ll"
sed -i '/call void @llvm\.lifetime\.\(start\|end\)/d' "$BUILD_DIR/out.ll"
sed -i 's/ "target-cpu"="[^"]*"//g; s/ "target-features"="[^"]*"//g' "$BUILD_DIR/out.ll"
sed -i '/^attributes #[0-9]* = { }$/d' "$BUILD_DIR/out.ll"

# LLVM IR → WASM object
llc --mtriple=wasm64-wasi -filetype=obj "$BUILD_DIR/out.ll"

# Link → WASM binary
wasm-ld \
    --no-entry \
    --export-all \
    --allow-undefined \
    -mwasm64 \
    --initial-memory="$INITIAL_MEM" \
    -o "$BUILD_DIR/out.wasm" \
    "$BUILD_DIR/out.o"

log "WASM binary built: $BUILD_DIR/out.wasm"

# ── Step 2: Copy per-example web assets ───────────────────────────────────
#
# Each example needs its HTML shell and JS entry point. We look in two
# locations (shared examples first, then web-specific examples):
#
#   1. mojo-gui/examples/<name>/web/  (shared examples — target structure)
#   2. mojo-gui/web/examples/<name>/  (current web examples)

log "Copying web assets for examples: ${EXAMPLES[*]}"

for name in "${EXAMPLES[@]}"; do
    OUT="$BUILD_DIR/examples/$name"
    mkdir -p "$OUT"

    # Find the HTML and JS assets
    HTML_SRC=""
    JS_SRC=""

    # Check shared examples directory first
    if [ -f "$EXAMPLES_DIR/$name/web/index.html" ]; then
        HTML_SRC="$EXAMPLES_DIR/$name/web/index.html"
    elif [ -f "$WEB_EXAMPLES_DIR/$name/index.html" ]; then
        HTML_SRC="$WEB_EXAMPLES_DIR/$name/index.html"
    fi

    if [ -f "$EXAMPLES_DIR/$name/web/main.js" ]; then
        JS_SRC="$EXAMPLES_DIR/$name/web/main.js"
    elif [ -f "$WEB_EXAMPLES_DIR/$name/main.js" ]; then
        JS_SRC="$WEB_EXAMPLES_DIR/$name/main.js"
    fi

    if [ -z "$HTML_SRC" ]; then
        echo "  SKIP $name — no index.html found"
        continue
    fi

    cp "$HTML_SRC" "$OUT/index.html"
    [ -n "$JS_SRC" ] && cp "$JS_SRC" "$OUT/main.js"

    echo "  OK   $name → $OUT/"
done

# ── Step 3: Copy shared JS library ───────────────────────────────────────
#
# The examples/lib/ directory contains shared JS modules used by all
# example entry points (app.js, env.js, events.js, interpreter.js, etc.)

if [ -d "$WEB_EXAMPLES_DIR/lib" ]; then
    log "Copying shared JS library..."
    mkdir -p "$BUILD_DIR/examples/lib"
    cp "$WEB_EXAMPLES_DIR/lib/"*.js "$BUILD_DIR/examples/lib/"
    echo "  OK   lib/ → $BUILD_DIR/examples/lib/"
fi

# ── Summary ───────────────────────────────────────────────────────────────

WASM_SIZE=$(wc -c < "$BUILD_DIR/out.wasm" | tr -d ' ')
log "Build complete!"
echo "  WASM binary: $BUILD_DIR/out.wasm ($WASM_SIZE bytes)"
echo "  Examples:    ${EXAMPLES[*]}"
echo ""
echo "Serve locally with:"
echo "  cd $WEB_DIR && deno run --allow-net --allow-read jsr:@std/http/file-server"
echo ""
echo "Then open:"
for name in "${EXAMPLES[@]}"; do
    if [ -f "$BUILD_DIR/examples/$name/index.html" ]; then
        echo "  http://localhost:4507/examples/$name/"
    fi
done

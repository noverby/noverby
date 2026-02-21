# 🔥 wasm-mojo

A reactive UI framework for the browser, written in [Mojo](https://www.modular.com/mojo) and compiled to WebAssembly.

Built from the ground up — signals, virtual DOM, diffing, event handling, and a binary mutation protocol — all running as WASM with a thin TypeScript runtime.

## Features

- **Reactive signals** — fine-grained reactivity with automatic dependency tracking
- **Virtual DOM** — template-based VNodes with keyed diffing
- **Binary mutation protocol** — efficient Mojo → JS communication via shared memory
- **Event system** — DOM events delegated through WASM with action-based handlers
- **Scoped components** — hierarchical scopes with hooks, context, error boundaries, and suspense
- **Ergonomic DSL** — `el_div`, `el_button`, `dyn_text` tag helpers with `to_template()` conversion
- **AppShell abstraction** — single struct bundling runtime, store, allocator, and scheduler
- **Three working apps** — counter, todo list, and js-framework-benchmark
- **1,536 tests** — 676 Mojo (via wasmtime) + 860 JS (via Deno), all passing

## How it works

The build pipeline compiles Mojo source code to WASM through LLVM:

```txt
Mojo → LLVM IR → WASM Object → WASM Binary
```

1. `mojo build` emits LLVM IR as a shared library
2. `llc` compiles the IR to a wasm64-wasi object file
3. `wasm-ld` links the object into a `.wasm` binary
4. `wasmtime` pre-compiles to `.cwasm` for fast instantiation (~70ms)

At runtime, the TypeScript side (`runtime/`) instantiates the WASM module and provides:

- **Memory management** — a bump allocator for `KGEN_CompilerRT_AlignedAlloc`/`AlignedFree`
- **I/O** — `write` routed to `console.log`/`console.error` for stdout/stderr
- **Math builtins** — `fma`, `fmin`, `fmax` and their float variants
- **Libc stubs** — `dup`, `fdopen`, `fflush`, `fclose`, `__cxa_atexit`
- **String ABI** — helpers for reading/writing Mojo `String` structs (including SSO)
- **DOM interpreter** — a stack machine that applies binary mutations to the real DOM
- **Event bridge** — captures DOM events and dispatches them to WASM handlers

## Architecture

```txt
┌─────────────────────────────────────────────────────────┐
│  Browser                                                │
│                                                         │
│  ┌──────────────┐    mutations    ┌──────────────────┐  │
│  │  DOM          │◄──────────────│  JS Interpreter    │  │
│  │  (real nodes) │               │  (stack machine)   │  │
│  └──────┬───────┘               └────────┬───────────┘  │
│         │ events                         ▲ binary buf    │
│         ▼                                │               │
│  ┌──────────────┐               ┌────────┴───────────┐  │
│  │  Event Bridge │──dispatch───►│  WASM Module        │  │
│  │  (JS)         │              │  (Mojo)             │  │
│  └──────────────┘               │                     │  │
│                                 │  ┌─ Signals ──────┐ │  │
│                                 │  │  Scopes         │ │  │
│                                 │  │  VNode Store    │ │  │
│                                 │  │  Diff Engine    │ │  │
│                                 │  │  Mutation Writer│ │  │
│                                 │  └────────────────┘ │  │
│                                 └─────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

## Project structure

```txt
wasm-mojo/
├── src/
│   ├── main.mojo                 # @export wrappers (WASM entry point, 3,335 lines, 397 exports)
│   ├── apps/                     # Application modules
│   │   ├── counter.mojo          # Counter app (Phase 7)
│   │   ├── todo.mojo             # Todo list app (Phase 8)
│   │   └── bench.mojo            # js-framework-benchmark (Phase 9)
│   ├── arena/
│   │   └── element_id.mojo       # ElementId type and allocator
│   ├── bridge/
│   │   └── protocol.mojo         # Opcode constants, MutationWriter
│   ├── component/                # Reusable app infrastructure
│   │   ├── app_shell.mojo        # AppShell struct (runtime + store + allocator + scheduler)
│   │   └── lifecycle.mojo        # mount, diff, finalize helpers; FragmentSlot + flush_fragment
│   ├── events/
│   │   └── registry.mojo         # Handler registry and dispatch
│   ├── mutations/
│   │   ├── create.mojo           # CreateEngine (initial mount)
│   │   └── diff.mojo             # DiffEngine (keyed reconciliation)
│   ├── poc/                      # Original proof-of-concept exports
│   │   ├── arithmetic.mojo       # add, sub, mul, div, mod, pow, neg, abs, min, max, clamp
│   │   ├── bitwise.mojo          # and, or, xor, not, shl, shr
│   │   ├── comparison.mojo       # eq, ne, lt, le, gt, ge, bool ops
│   │   ├── algorithms.mojo       # fibonacci, factorial, gcd
│   │   └── strings.mojo          # identity, print, return, length, concat, repeat, eq
│   ├── scheduler/
│   │   └── scheduler.mojo        # Height-ordered dirty scope queue with deduplication
│   ├── scope/
│   │   ├── scope.mojo            # ScopeState, hooks, context, error/suspense
│   │   └── arena.mojo            # ScopeArena (slab allocator)
│   ├── signals/
│   │   └── runtime.mojo          # Reactive runtime, signal store, context tracking
│   └── vdom/
│       ├── builder.mojo          # TemplateBuilder API (manual template construction)
│       ├── dsl.mojo              # Ergonomic DSL: Node union, el_* helpers, to_template()
│       ├── dsl_tests.mojo        # Self-contained DSL test functions (19 tests, extracted from main.mojo)
│       ├── registry.mojo         # Template storage and lookup
│       ├── tags.mojo             # HTML tag constants (TAG_DIV, TAG_SPAN, ...)
│       ├── template.mojo         # Template, TemplateNode (static structure)
│       └── vnode.mojo            # VNode, DynamicNode, AttributeValue, VNodeBuilder
├── runtime/                      # TypeScript runtime (browser)
│   ├── mod.ts                    # Entry point — instantiate WASM
│   ├── types.ts                  # WasmExports interface
│   ├── memory.ts                 # Bump allocator, WASM memory
│   ├── env.ts                    # Environment imports (I/O, math, libc)
│   ├── strings.ts                # Mojo String ABI helpers (SSO)
│   ├── protocol.ts               # Mutation opcodes (shared with Mojo)
│   ├── interpreter.ts            # DOM stack machine
│   ├── templates.ts              # Template cache (DocumentFragment pool)
│   ├── events.ts                 # Event delegation bridge
│   ├── tags.ts                   # HTML tag name mapping
│   └── app.ts                    # App lifecycle helpers
├── examples/
│   ├── lib/                      # Shared JS runtime for examples
│   │   ├── boot.js               # Re-exports + convenience helpers
│   │   ├── env.js                # WASM memory management, loadWasm()
│   │   ├── interpreter.js        # DOM Interpreter class
│   │   ├── protocol.js           # Op constants + MutationReader
│   │   └── strings.js            # Mojo String ABI writeStringStruct()
│   ├── counter/                  # Counter app (browser)
│   ├── todo/                     # Todo list app (browser)
│   └── bench/                    # js-framework-benchmark (browser)
├── test/                         # Mojo tests (26 modules, 676 tests via wasmtime)
│   ├── wasm_harness.mojo         # WasmInstance harness using wasmtime-mojo FFI
│   ├── test_signals.mojo         # Reactive signals
│   ├── test_scopes.mojo          # Scope arena and hooks
│   ├── test_templates.mojo       # Template builder, registry, VNode store
│   ├── test_mutations.mojo       # Create/diff engines
│   ├── test_events.mojo          # Event handler registry
│   ├── test_protocol.mojo        # Binary mutation encoding
│   ├── test_dsl.mojo             # Ergonomic DSL builder
│   ├── test_component.mojo       # AppShell and lifecycle
│   ├── test_scheduler.mojo       # Scheduler ordering and dedup
│   └── ...                       # + arithmetic, strings, boundaries, etc.
├── test-js/                      # JS runtime integration tests (860 tests via Deno)
│   ├── harness.ts                # Shared WASM loading and test helpers
│   ├── counter.test.ts           # Full counter app lifecycle with DOM
│   ├── todo.test.ts              # Todo app: add, remove, toggle, clear
│   ├── bench.test.ts             # Benchmark operations + timing
│   ├── dsl.test.ts               # DSL builder + VNodeBuilder round-trip
│   ├── interpreter.test.ts       # DOM interpreter + template cache
│   ├── mutations.test.ts         # JS-side MutationReader + memory
│   ├── phase8.test.ts            # Context, error boundaries, suspense
│   └── protocol.test.ts          # Binary protocol parsing
├── scripts/
│   ├── build_test_binaries.sh    # Parallel incremental mojo build for test modules
│   ├── run_test_binaries.sh      # Parallel test binary execution with reporting
│   └── precompile.mojo           # .wasm → .cwasm via wasmtime AOT
├── justfile                      # Build and test commands
├── default.nix                   # Nix dev shell
└── PLAN.md                       # Full development plan (Phases 0–10)
```

## Reactive model

The framework follows the same reactive model as [Dioxus](https://dioxuslabs.com/):

1. **Signals** hold state. Reading a signal inside a scope subscribes that scope.
2. **Writing** to a signal marks all subscribing scopes as dirty.
3. **Dirty scopes** are collected into the **Scheduler** (height-ordered, deduplicated).
4. Scopes are re-rendered in parent-before-child order, producing new VNode trees.
5. The **diff engine** compares old and new VNode trees (with keyed reconciliation).
6. Mutations are written to a **binary buffer** in shared WASM memory.
7. The JS **interpreter** reads the buffer and applies DOM operations.

```txt
Signal write → scope dirty → scheduler → re-render → diff → mutations → DOM update
```

## Binary mutation protocol

Mojo and JS communicate through a binary protocol in shared memory. Each mutation is a compact byte sequence:

| Opcode | Name | Payload |
|--------|------|---------|
| `0x00` | End | — |
| `0x01` | AppendChildren | id: u32, count: u32 |
| `0x02` | AssignId | path: u8[], id: u32 |
| `0x03` | CreatePlaceholder | id: u32 |
| `0x04` | CreateTextNode | id: u32, text: str |
| `0x05` | LoadTemplate | tmpl: u32, index: u32, id: u32 |
| `0x06` | ReplaceWith | id: u32, count: u32 |
| `0x07` | ReplacePlaceholder | path: u8[], count: u32 |
| `0x08` | InsertAfter | id: u32, count: u32 |
| `0x09` | InsertBefore | id: u32, count: u32 |
| `0x0a` | SetAttribute | id: u32, ns: u8, name: str, value: str |
| `0x0b` | SetText | id: u32, text: str |
| `0x0c` | NewEventListener | id: u32, name: str |
| `0x0d` | RemoveEventListener | id: u32, name: str |
| `0x0e` | Remove | id: u32 |
| `0x0f` | PushRoot | id: u32 |

## Prerequisites

Enter the dev shell (requires [Nix](https://nixos.org/)):

```sh
nix develop .#wasm-mojo
```

This provides `just`, `mojo`, `deno`, `llc`, `wasm-ld`, and `wasmtime`.

## Usage

Build the WASM binary:

```sh
just build
```

Run the Mojo tests (precompiled binaries, ~10s):

```sh
just test
```

Run the JS runtime integration tests:

```sh
just test-js
```

Run all tests (Mojo + JS):

```sh
just test-all
```

Serve the examples locally:

```sh
just serve
```

Then open:

- <http://localhost:4507/examples/counter/> — Counter app
- <http://localhost:4507/examples/todo/> — Todo list
- <http://localhost:4507/examples/bench/> — Benchmark

## Test infrastructure

Test execution uses **precompiled binaries** for fast iteration (~10s vs ~5–6 minutes with `mojo test`):

1. Each `test/test_*.mojo` file has an inline `fn main()` that creates one shared `WasmInstance` and calls all test functions sequentially.
2. `scripts/build_test_binaries.sh` compiles each module into a standalone binary in `build/test-bin/` with incremental timestamp checks (parallel, up to `nproc` jobs).
3. `scripts/run_test_binaries.sh` launches all binaries concurrently and reports pass/fail with timing.

| Scenario | Time |
|---|---|
| Cold build (all 26 binaries) | ~92s |
| Incremental build (nothing changed) | <0.1s |
| Run precompiled binaries | ~10s |
| Run single module (`just test-run signals`) | ~100ms |
| Full cycle (`just test`, no code change) | ~11s |
| Full cycle + JS tests (`just test-all`) | ~22s |

Filter by module name (substring match) to target specific tests:

```sh
just test signals             # build + run only test_signals (~100ms)
just test signals mut         # build + run test_signals + test_mutations
just test-run -v dsl          # verbose output for test_dsl only
```

Adding a new test:

1. Write `def test_foo(w: UnsafePointer[WasmInstance])` in the appropriate `test/test_*.mojo` file.
2. Add `test_foo(w)` to the `fn main()` at the bottom of the same file.
3. Run `just test`.

## Test results

1,536 tests across 26 Mojo modules and 8 JS test suites:

- **Signals & reactivity** — create, read, write, subscribe, dirty tracking, context
- **Scopes** — lifecycle, hooks, context propagation, error boundaries, suspense
- **Scheduler** — height-ordered processing, deduplication, multi-scope ordering
- **Templates** — builder, DSL, registry, node queries
- **VNodes** — template refs, text, placeholders, fragments, keyed children
- **Mutations** — create engine, diff engine, binary protocol round-trip
- **Events** — handler registry, dispatch, signal actions
- **DSL** — Node union, tag helpers, to_template conversion, VNodeBuilder
- **Component** — AppShell lifecycle, mount/diff/finalize helpers, FragmentSlot
- **Counter app** — init, mount, click, flush, DOM verification
- **Todo app** — add, remove, toggle, clear, keyed list transitions
- **Benchmark** — create/append/update/swap/select/remove/clear 1000 rows
- **Memory** — allocation cycles, bounded growth, rapid write stability
- **Arithmetic/strings** — original PoC interop regression suite

## Ergonomic DSL

The framework provides an ergonomic builder DSL for declaring UI templates:

```txt
# Tag helpers: el_div, el_span, el_button, el_h1, ... (40 tags)
# Content:    text("Hello"), dyn_text(), dyn_node()
# Attributes: attr("class", "active"), dyn_attr("onclick")

var view = el_div(List(
    el_h1(List(text("Counter"))),
    el_span(List(dyn_text())),
    el_button(List(text("+"), dyn_attr("onclick"))),
))

var template = to_template(view, String("counter"))
```

The `VNodeBuilder` provides ergonomic VNode construction with typed dynamic slots:

```txt
var vb = VNodeBuilder(store, template_id, scope_id)
vb.add_dyn_text(String("0"))
vb.add_dyn_event(handler_id)
var vnode_id = vb.build()
```

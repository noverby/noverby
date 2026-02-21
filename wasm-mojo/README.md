# 🔥 wasm-mojo

A reactive UI framework for the browser, written in [Mojo](https://www.modular.com/mojo) and compiled to WebAssembly.

Built from the ground up — signals, virtual DOM, diffing, event handling, and a binary mutation protocol — all running as WASM with a thin TypeScript runtime.

## Features

- **Reactive signals** — fine-grained reactivity with automatic dependency tracking
- **Virtual DOM** — template-based VNodes with keyed diffing
- **Binary mutation protocol** — efficient Mojo → JS communication via shared memory
- **Event system** — DOM events delegated through WASM with action-based handlers
- **Scoped components** — hierarchical scopes with hooks, context, error boundaries, and suspense
- **Three working apps** — counter, todo list, and js-framework-benchmark

## How it works

The build pipeline compiles Mojo source code to WASM through LLVM:

```txt
Mojo → LLVM IR → WASM Object → WASM Binary
```

1. `mojo build` emits LLVM IR as a shared library
2. `llc` compiles the IR to a wasm64-wasi object file
3. `wasm-ld` links the object into a `.wasm` binary

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
│   ├── main.mojo                 # @export wrappers (WASM entry point)
│   ├── apps/                     # Application modules
│   │   ├── counter.mojo          # Counter app (Phase 7)
│   │   ├── todo.mojo             # Todo list app (Phase 8)
│   │   └── bench.mojo            # js-framework-benchmark (Phase 9)
│   ├── arena/
│   │   └── element_id.mojo       # ElementId type and allocator
│   ├── signals/
│   │   └── runtime.mojo          # Reactive runtime, signal store, context
│   ├── scope/
│   │   ├── scope.mojo            # ScopeState, hooks, context, error/suspense
│   │   └── arena.mojo            # ScopeArena (slab allocator)
│   ├── vdom/
│   │   ├── template.mojo         # Template, TemplateNode (static structure)
│   │   ├── registry.mojo         # Template storage and lookup
│   │   ├── vnode.mojo            # VNode, DynamicNode, AttributeValue
│   │   ├── builder.mojo          # TemplateBuilder API
│   │   └── tags.mojo             # HTML tag constants
│   ├── mutations/
│   │   ├── create.mojo           # CreateEngine (initial mount)
│   │   └── diff.mojo             # DiffEngine (keyed reconciliation)
│   ├── events/
│   │   └── registry.mojo         # Handler registry and dispatch
│   └── bridge/
│       └── protocol.mojo         # Opcode constants, MutationWriter
├── runtime/                      # TypeScript runtime
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
│   ├── counter/                  # Counter app (browser)
│   ├── todo/                     # Todo list app (browser)
│   └── bench/                    # js-framework-benchmark (browser)
├── test/                         # Mojo-side tests (via wasmtime)
│   ├── test_signals.mojo         # Reactive signals
│   ├── test_scopes.mojo          # Scope arena and hooks
│   ├── test_templates.mojo       # Template builder, registry, VNode store
│   ├── test_mutations.mojo       # Create/diff engines
│   ├── test_events.mojo          # Event handler registry
│   ├── test_protocol.mojo        # Binary mutation encoding
│   └── ...                       # + arithmetic, strings, boundaries, etc.
├── test-js/                      # JS runtime integration tests (Deno)
│   ├── counter.test.ts           # Full counter app lifecycle with DOM
│   ├── todo.test.ts              # Todo app: add, remove, toggle, clear
│   ├── bench.test.ts             # Benchmark operations + timing
│   ├── interpreter.test.ts       # DOM interpreter + template cache
│   ├── mutations.test.ts         # JS-side MutationReader + memory
│   ├── phase8.test.ts            # Context, error boundaries, suspense
│   └── protocol.test.ts          # Binary protocol parsing
├── justfile                      # Build and test commands
├── default.nix                   # Nix dev shell
└── PLAN.md                       # Full development plan (Phases 0–9)
```

## Reactive model

The framework follows the same reactive model as [Dioxus](https://dioxuslabs.com/):

1. **Signals** hold state. Reading a signal inside a scope subscribes that scope.
2. **Writing** to a signal marks all subscribing scopes as dirty.
3. **Dirty scopes** are drained and re-rendered, producing new VNode trees.
4. The **diff engine** compares old and new VNode trees (with keyed reconciliation).
5. Mutations are written to a **binary buffer** in shared WASM memory.
6. The JS **interpreter** reads the buffer and applies DOM operations.

```txt
Signal write → scope dirty → re-render → diff → mutations → DOM update
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

This provides `just`, `mojo`, `deno`, `llc`, and `wasm-ld`.

## Usage

Build the WASM binary:

```sh
just build
```

Run the Mojo-side tests (via wasmtime):

```sh
just test
```

Run the JS runtime integration tests (DOM interpreter, apps):

```sh
just test-js
```

Run all tests:

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

## Test results

790 tests across Mojo (wasmtime) and JS (Deno) test suites:

- **Signals & reactivity** — create, read, write, subscribe, dirty tracking
- **Scopes** — lifecycle, hooks, context, error boundaries, suspense
- **Templates** — builder, registry, node queries
- **VNodes** — template refs, text, placeholders, fragments, keyed children
- **Mutations** — create engine, diff engine, binary protocol round-trip
- **Events** — handler registry, dispatch, signal actions
- **Counter app** — init, mount, click, flush, DOM verification
- **Todo app** — add, remove, toggle, clear, keyed list transitions
- **Benchmark** — create/append/update/swap/select/remove/clear 1000 rows
- **Memory** — allocation cycles, bounded growth, rapid write stability
- **Arithmetic/strings** — original PoC interop regression suite
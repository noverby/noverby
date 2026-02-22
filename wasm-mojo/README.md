# 🔥 wasm-mojo

A reactive UI framework for the browser, written in [Mojo](https://www.modular.com/mojo) and compiled to WebAssembly.

Built from the ground up — signals, virtual DOM, diffing, event handling, and a binary mutation protocol — all running as WASM with a thin TypeScript runtime.

## Features

- **Reactive signals** — fine-grained reactivity with automatic dependency tracking
- **Memo (derived signals)** — cached computed values with automatic dependency re-tracking
- **Virtual DOM** — template-based VNodes with keyed diffing
- **Binary mutation protocol** — efficient Mojo → JS communication via shared memory
- **Automatic template wiring** — templates defined once in Mojo, auto-registered in JS via `RegisterTemplate` mutations
- **Automatic event wiring** — handler IDs flow through the mutation protocol; `EventBridge` dispatches events without manual mapping
- **Event system** — DOM events delegated through WASM with action-based handlers
- **Scoped components** — hierarchical scopes with hooks, context, error boundaries, and suspense
- **Ergonomic DSL** — `el_div`, `el_button`, `dyn_text` tag helpers with `to_template()` conversion
- **AppShell abstraction** — single struct bundling runtime, store, allocator, and scheduler
- **ComponentContext** — ergonomic Dioxus-style API with `use_signal()`, `setup_view()`, inline events, auto-numbered `dyn_text()`
- **Three working apps** — counter, todo list, and js-framework-benchmark (all using ComponentContext)
- **ItemBuilder + HandlerAction** — ergonomic per-item building and event dispatch for keyed lists (`begin_item()`, `add_custom_event()`, `get_action()`)
- **String event dispatch** — `ACTION_SIGNAL_SET_STRING` handlers pipe string values from DOM events directly into `SignalString` signals; JS EventBridge extracts `event.target.value` → `writeStringStruct()` → WASM `dispatch_event_with_string` with automatic fallback to numeric/default dispatch
- **2,200 tests** — 987 Mojo (via wasmtime) + 1,213 JS (via Deno), all passing

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
│                                 │  │  Memos          │ │  │
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
│   ├── main.mojo                 # @export wrappers (WASM entry point, 3,476 lines, 419 exports)
│   ├── arena/
│   │   └── element_id.mojo       # ElementId type and allocator
│   ├── bridge/
│   │   └── protocol.mojo         # Opcode constants, MutationWriter
│   ├── component/                # Reusable app infrastructure
│   │   ├── app_shell.mojo        # AppShell struct (runtime + store + allocator + scheduler)
│   │   ├── context.mojo          # ComponentContext — ergonomic API, RenderBuilder, view tree processing
│   │   └── lifecycle.mojo        # mount, diff, finalize helpers; FragmentSlot + flush_fragment
│   ├── events/
│   │   └── registry.mojo         # Handler registry and dispatch
│   ├── mutations/
│   │   ├── create.mojo           # CreateEngine (initial mount)
│   │   └── diff.mojo             # DiffEngine (keyed reconciliation)
│   ├── scheduler/
│   │   └── scheduler.mojo        # Height-ordered dirty scope queue with deduplication
│   ├── scope/
│   │   ├── scope.mojo            # ScopeState, hooks, context, error/suspense
│   │   └── arena.mojo            # ScopeArena (slab allocator)
│   ├── signals/
│   │   ├── memo.mojo             # MemoEntry, MemoStore (slab allocator for derived signals)
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
│   ├── counter/                  # Counter app — simplest example
│   │   ├── counter.mojo          # Mojo app (inline events via setup_view)
│   │   ├── index.html            # Browser entry point
│   │   └── main.js               # JS harness
│   ├── todo/                     # Todo list app
│   │   ├── todo.mojo             # Mojo app (keyed lists, multi-template, custom handlers, SignalString)
│   │   ├── index.html            # Browser entry point
│   │   └── main.js               # JS harness
│   ├── bench/                    # js-framework-benchmark
│   │   ├── bench.mojo            # Mojo app (keyed lists, 7 operations)
│   │   ├── index.html            # Browser entry point
│   │   └── main.js               # JS harness
│   └── lib/                      # Shared JS runtime for examples
│       ├── boot.js               # Re-exports + convenience helpers
│       ├── env.js                # WASM memory management, loadWasm()
│       ├── interpreter.js        # DOM Interpreter class
│       ├── protocol.js           # Op constants + MutationReader
│       └── strings.js            # Mojo String ABI writeStringStruct()
├── test/                         # Mojo tests (29 modules, 903 tests via wasmtime)
│   ├── wasm_harness.mojo         # WasmInstance harness using wasmtime-mojo FFI
│   ├── test_signals.mojo         # Reactive signals
│   ├── test_scopes.mojo          # Scope arena and hooks
│   ├── test_templates.mojo       # Template builder, registry, VNode store
│   ├── test_mutations.mojo       # Create/diff engines
│   ├── test_events.mojo          # Event handler registry
│   ├── test_protocol.mojo        # Binary mutation encoding
│   ├── test_dsl.mojo             # Ergonomic DSL builder
│   ├── test_component.mojo       # AppShell and lifecycle
│   ├── test_memo.mojo            # Memo store, runtime API, hooks, propagation
│   ├── test_scheduler.mojo       # Scheduler ordering and dedup
│   └── ...                       # + arithmetic, strings, boundaries, etc.
├── test-js/                      # JS runtime integration tests (1,152 tests via Deno)
│   ├── harness.ts                # Shared WASM loading and test helpers
│   ├── counter.test.ts           # Full counter app lifecycle with DOM
│   ├── todo.test.ts              # Todo app: add, remove, toggle, clear
│   ├── bench.test.ts             # Benchmark operations + timing
│   ├── dsl.test.ts               # DSL builder + VNodeBuilder round-trip
│   ├── interpreter.test.ts       # DOM interpreter + template cache
│   ├── memo.test.ts              # Memo lifecycle, dirty tracking, propagation
│   ├── mutations.test.ts         # JS-side MutationReader + memory
│   ├── phase8.test.ts            # Context, error boundaries, suspense
│   └── protocol.test.ts          # Binary protocol parsing
├── scripts/
│   ├── build_test_binaries.sh    # Parallel incremental mojo build for test modules
│   ├── run_test_binaries.sh      # Parallel test binary execution with reporting
│   └── precompile.mojo           # .wasm → .cwasm via wasmtime AOT
├── justfile                      # Build and test commands
├── default.nix                   # Nix dev shell
└── CHANGELOG.md                  # Development history (Phases 0–14)
```

## Known limitations

**`@export` only works in the main module.** Mojo's compiler aggressively eliminates dead code before LLVM IR generation. An `@export` decorator on a function in a submodule (e.g., `poc/arithmetic.mojo`) does **not** prevent it from being removed — the function must be called from `main.mojo` to survive. Importing a submodule function without calling it is also insufficient as a DCE anchor. This is why `main.mojo` contains ~419 thin `@export` wrappers that forward to submodule implementations: it is the only reliable way to guarantee WASM export visibility with the current Mojo toolchain. See [CHANGELOG.md § M10.22](CHANGELOG.md#phase-10--modularization--next-steps-) for the full investigation.

**Handler lifecycle is scope-scoped.** Event handlers registered via `runtime.register_handler()` are automatically cleaned up when their owning scope is destroyed. For dynamic lists (todo items, benchmark rows), each item gets its own child scope. Rebuilding a list destroys old child scopes — which triggers `remove_for_scope` cleanup in the `HandlerRegistry` — before creating new ones. Without this pattern, handler IDs leak: after 100 add/remove cycles on a 10-item list, the registry would accumulate ~2,000 stale entries. The child-scope-per-item pattern ensures handler count stays proportional to visible items.

## Reactive model

The framework follows the same reactive model as [Dioxus](https://dioxuslabs.com/):

1. **Signals** hold state. Reading a signal inside a scope subscribes that scope.
2. **Memos** (derived signals) cache computed values. A memo has its own reactive context: it auto-tracks which signals it reads during computation, caches the result, and marks subscribing scopes dirty when its inputs change. Memos are lazy — they only recompute when read while dirty. Dependency re-tracking on recompute means memos automatically adapt to conditional reads.
3. **Writing** to a signal marks all subscribing scopes *and* all subscribing memos as dirty. Dirty memos propagate dirtiness to their own subscribers.
4. **Dirty scopes** are collected into the **Scheduler** (height-ordered, deduplicated).
5. Scopes are re-rendered in parent-before-child order, producing new VNode trees.
6. The **diff engine** compares old and new VNode trees (with keyed reconciliation).
7. Mutations are written to a **binary buffer** in shared WASM memory.
8. The JS **interpreter** reads the buffer and applies DOM operations.

```txt
Signal write → memo dirty → scope dirty → scheduler → re-render → diff → mutations → DOM update
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
| `0x0c` | NewEventListener | id: u32, handler_id: u32, name: str |
| `0x0d` | RemoveEventListener | id: u32, name: str |
| `0x0e` | Remove | id: u32 |
| `0x0f` | PushRoot | id: u32 |
| `0x10` | RegisterTemplate | tmpl_id: u32, name: str, nodes[], attrs[], roots[] |

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

2,200 tests across 29 Mojo modules and 10 JS test suites:

- **Signals & reactivity** — create, read, write, subscribe, dirty tracking, context
- **Scopes** — lifecycle, hooks, context propagation, error boundaries, suspense
- **Scheduler** — height-ordered processing, deduplication, multi-scope ordering
- **Templates** — builder, DSL, registry, node queries
- **VNodes** — template refs, text, placeholders, fragments, keyed children
- **Mutations** — create engine, diff engine, binary protocol round-trip
- **Events** — handler registry, dispatch, signal actions, string dispatch (Phase 20), EventBridge string extraction, dispatch fallback chain, WASM integration
- **DSL** — Node union, tag helpers, to_template conversion, VNodeBuilder
- **Memo** — create/destroy, dirty tracking, auto-track, propagation chain, diamond dependency, dependency re-tracking, cache hit, version bumps, cleanup, hooks
- **Component** — AppShell lifecycle, mount/diff/finalize helpers, FragmentSlot, shell memo helpers, ItemBuilder handler map
- **Counter app** — init, mount, click, flush, DOM verification, memo (doubled count) demo
- **Todo app** — add, remove, toggle, clear, keyed list transitions
- **Benchmark** — create/append/update/swap/select/remove/clear 1000 rows, full DOM integration
- **Memory** — allocation cycles, bounded growth, rapid write stability
- **Arithmetic/strings** — original PoC interop regression suite

## Ergonomic API

All apps use `ComponentContext` for Dioxus-style ergonomics — constructor-based setup,
`use_signal()` with operator overloading, inline event handlers, auto-numbered
dynamic text slots, and multi-arg `el_*` overloads that eliminate `List[Node]()` wrappers:

```mojo
# Dioxus (Rust):
#     fn App() -> Element {
#         let mut count = use_signal(|| 0);
#         rsx! {
#             h1 { "High-Five counter: {count}" }
#             button { onclick: move |_| count += 1, "Up high!" }
#             button { onclick: move |_| count -= 1, "Down low!" }
#         }
#     }

# Mojo equivalent:
struct CounterApp:
    var ctx: ComponentContext
    var count: SignalI32

    fn __init__(out self):
        self.ctx = ComponentContext.create()
        self.count = self.ctx.use_signal(0)
        self.ctx.setup_view(
            el_div(
                el_h1(dyn_text()),
                el_button(text("Up high!"), onclick_add(self.count, 1)),
                el_button(text("Down low!"), onclick_sub(self.count, 1)),
            ),
            String("counter"),
        )

    fn render(mut self) -> UInt32:
        var vb = self.ctx.render_builder()
        vb.add_dyn_text("High-Five counter: " + String(self.count.peek()))
        return vb.build()
```

Multi-template apps (todo, bench) use `KeyedList` with `ItemBuilder` for ergonomic
per-item building, `HandlerAction` for event dispatch, and Phase 18 conditional
helpers (`add_class_if`, `text_when`) to eliminate if/else boilerplate:

```mojo
# Keyed list pattern (todo, bench) — Phase 17 + 18 ergonomics:
alias TODO_ACTION_TOGGLE: UInt8 = 1
alias TODO_ACTION_REMOVE: UInt8 = 2

struct TodoApp:
    var ctx: ComponentContext
    var list_version: SignalI32
    var items: KeyedList  # bundles template_id + FragmentSlot + scope_ids + handler_map

    fn __init__(out self):
        self.ctx = ComponentContext.create()
        self.list_version = self.ctx.use_signal(0)
        self.ctx.end_setup()
        self.ctx.register_template(
            el_div(
                el_input(attr("type", "text"), attr("placeholder", "...")),
                el_button(text("Add"), dyn_attr(0)),
                el_ul(dyn_node(0)),
            ),
            String("todo-app"),
        )
        self.items = KeyedList(self.ctx.register_extra_template(
            el_li(
                dyn_attr(2),
                el_span(dyn_text(0)),
                el_button(text("✓"), dyn_attr(0)),
                el_button(text("✕"), dyn_attr(1)),
            ),
            String("todo-item"),
        ))

    fn build_item(mut self, item: TodoItem) -> UInt32:
        var ib = self.items.begin_item(String(item.id), self.ctx)
        # text_when() replaces 4-line if/else for conditional text
        ib.add_dyn_text(text_when(item.completed, "✓ " + item.text, item.text))
        ib.add_custom_event(String("click"), TODO_ACTION_TOGGLE, item.id)
        ib.add_custom_event(String("click"), TODO_ACTION_REMOVE, item.id)
        # add_class_if() replaces 4-line if/else for conditional class
        ib.add_class_if(item.completed, String("completed"))
        return ib.index()

    fn build_items(mut self) -> UInt32:
        var frag = self.items.begin_rebuild(self.ctx)
        for i in range(len(self.data)):
            var idx = self.build_item(self.data[i])
            self.items.push_child(self.ctx, frag, idx)
        return frag

    fn handle_event(mut self, handler_id: UInt32) -> Bool:
        var action = self.items.get_action(handler_id)
        if action.found:
            if action.tag == TODO_ACTION_TOGGLE:
                self.toggle_item(action.data)
            elif action.tag == TODO_ACTION_REMOVE:
                self.remove_item(action.data)
            return True
        return False
```

Phase 18 also adds `SignalBool` for ergonomic boolean signals and standalone
conditional helpers (`class_if`, `class_when`, `text_when`) usable anywhere:

```mojo
# SignalBool — proper boolean API over Int32 signals:
var visible = ctx.use_signal_bool(True)
visible.toggle()            # True ↔ False
if visible.get(): ...       # read without subscribing
visible.set(False)          # write (marks subscribers dirty)

# Conditional helpers — eliminate if/else boilerplate:
var cls = class_if(is_active, String("active"))           # "active" or ""
var cls = class_when(is_open, String("open"), String("closed"))  # either/or
var txt = text_when(done, String("✓ Done"), item.text)    # conditional text
```

Phase 19 adds `SignalString` for reactive string signals. Unlike `SignalI32`
and `SignalBool` which use the type-erased `SignalStore` (memcpy-based,
safe only for fixed-size value types), `SignalString` stores strings in a
separate `StringStore` (safe for heap types) and uses a companion Int32
"version signal" for subscriber tracking:

```mojo
# SignalString — reactive string signal with proper String API:
var name = ctx.use_signal_string(String("hello"))
var v = name.get()              # read without subscribing
var v = name.read()             # read and subscribe context
name.set(String("world"))       # write (marks subscribers dirty)
if name.is_empty(): ...         # convenience check
var display = String("Hi, ") + String(name) + String("!")  # interpolation

# Use with RenderBuilder or ItemBuilder:
var vb = ctx.render_builder()
vb.add_dyn_text_signal(name)    # reads name.get() and adds as dyn text
var idx = vb.build()

# Multiple signal types in one component:
var count = ctx.use_signal(0)
var label = ctx.use_signal_string(String("Count: 0"))
count += 1
label.set(String("Count: ") + String(count.peek()))
```

## Deferred abstractions

Some Dioxus features cannot be idiomatically expressed in Mojo today due to
language limitations tracked on the [Mojo roadmap](https://docs.modular.com/mojo/roadmap/).
They are documented here so they can be revisited as Mojo evolves:

| Dioxus feature | Mojo blocker | Roadmap item | Status |
|---|---|---|---|
| **Closure event handlers** (`onclick: move \|_\| count += 1`) | No closures/function pointers in WASM; handlers use action-based structs | Lambda syntax (Phase 1), Closure refinement (Phase 1) | 🚧 In progress |
| **`rsx!` macro** (compile-time DSL) | No hygienic macros | Hygienic importable macros (Phase 2) | ⏰ Not started |
| **`for` loops in views** (`for item in items { ... }`) | Views are static templates; iteration happens in build functions | Hygienic macros (Phase 2) | ⏰ Not started |
| **Generic `Signal[T]`** (`use_signal(\|\| vec![])`) | Runtime stores fixed `Int32` signals; parametric stores need conditional conformance. Phase 18 added `SignalBool`, Phase 19 added `SignalString` as manual workarounds | Conditional conformance (Phase 1) | 🚧 In progress |
| **Dynamic component dispatch** (trait objects for components) | No existentials/dynamic traits | Existentials / dynamic traits (Phase 2) | ⏰ Not started |
| **Pattern matching on actions** | `if/elif` chains instead of `match` | Algebraic data types & pattern matching (Phase 2) | ⏰ Not started |
| **Async data loading / suspense** | No `async`/`await` | First-class async support (Phase 2) | ⏰ Not started |
| **Untyped Python-style code** | Explicit types required everywhere | Phase 3: Dynamic OOP | ⏰ Not started |

When these Mojo features land, the corresponding Dioxus patterns can be
adopted — closures would eliminate `ItemBuilder.add_custom_event()` + `get_action()`,
macros would enable an `rsx!`-like DSL, and generic signals would replace the
current `SignalI32` / `SignalBool` / `SignalString` / `MemoI32` handles with
`Signal[Int32]`, `Signal[Bool]`, `Signal[String]`, etc.

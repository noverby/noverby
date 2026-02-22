# wasm-mojo — AI Agent Context

> Read this file first before exploring the project. It replaces ~20 file reads.

## What is this?

A reactive UI framework for the browser, written in Mojo → compiled to WASM64.
Signals, virtual DOM, template diffing, event handling, binary mutation protocol.
Thin TypeScript runtime applies DOM mutations from a shared-memory byte buffer.
Inspired by [Dioxus](https://dioxuslabs.com/) (Rust reactive framework).

## Build & Test

```sh
just build          # Mojo → LLVM IR → wasm64 object → out.wasm
just test           # 903 Mojo tests (precompiled binaries, ~10s)
just test-js        # 1,152 JS tests (Deno)
just test-all       # Both
just serve          # http://localhost:4507/examples/{counter,todo,bench}/
```

Test modules: `test/test_*.mojo` each have `fn main()` that creates a shared
`WasmInstance` (via wasmtime-mojo) and calls all test functions sequentially.
Filter: `just test signals` builds+runs only matching modules.
Adding a test: write `def test_foo(w)` → add `test_foo(w)` to `fn main()`.
Build script needs `-I src/` for native tests importing project packages.

## Mojo Constraints

- **No closures/function pointers in WASM** — event handlers are action-based structs.
- **`@export` only works in main.mojo** — submodule exports get DCE'd. All ~420 WASM exports are thin wrappers in `src/main.mojo` forwarding to submodule implementations.
- **Single-threaded** — no sync needed.
- **Operator overloading** works (SignalI32 has `+=`, `-=`, `peek()`, `set()`).
- **Format**: `mojo format <file>` — pre-commit hooks run this automatically.
- **Commit messages**: `feat(wasm-mojo): Uppercase description` (commitlint enforced).

## Key Abstractions (dependency order)

### Signals & Reactivity (`src/signals/`)

- `Runtime` — reactive runtime: signal store, scope tracking, context management.
- `SignalI32` (`signals/handle.mojo`) — ergonomic handle with `peek()`, `set()`, `+=`, `-=`. Holds key + runtime pointer.
- `MemoI32` — derived signal with lazy recomputation and auto dependency tracking.
- `EffectHandle` — reactive side effects.

### Scopes (`src/scope/`)

- `ScopeState` — lifecycle unit with hooks, context, error boundaries.
- `ScopeArena` — slab allocator for scopes. Parent→child hierarchy.

### Virtual DOM (`src/vdom/`)

- `Node` (DSL union) — `text()`, `dyn_text()`, `dyn_node()`, `attr()`, `dyn_attr()`, `el_div()`, `el_button()`, etc.
- Inline event constructors: `onclick_add(signal, delta)`, `onclick_sub()`, `onclick_set()`, `onclick_toggle()`, `on_event()`.
- `dyn_text()` with no args → auto-numbered (sentinel `DYN_TEXT_AUTO`).
- `to_template(node, name)` → `Template` (static structure for DOM cloning).
- `VNode` — runtime instance of a template with dynamic slots.
- `VNodeBuilder` — fills dynamic text/attr/event slots on a VNode.
- `VNodeStore` — arena for VNode storage.

### Mutations (`src/mutations/`)

- `CreateEngine` — walks VNode tree, emits create mutations (initial mount).
- `DiffEngine` — compares old/new VNode trees, emits minimal update mutations (keyed reconciliation).
- `MutationWriter` (`src/bridge/protocol.mojo`) — writes binary opcodes to shared buffer.

### Events (`src/events/`)

- `HandlerEntry` — action-based handler (signal_add, signal_sub, signal_set, signal_toggle, custom).
- `HandlerRegistry` — maps handler IDs → entries. Scope-scoped cleanup.

### Component Layer (`src/component/`)

- **`AppShell`** — bundles Runtime + VNodeStore + ElementIdAllocator + Scheduler. Low-level API.
- **`ComponentContext`** — ergonomic wrapper over AppShell. High-level API for apps:
  - `ComponentContext.create()` → allocates shell, root scope, begins render bracket.
  - `ctx.use_signal(initial)` → `SignalI32` (auto-subscribes scope).
  - `ctx.use_memo(initial)` → `MemoI32`.
  - `ctx.use_effect()` → `EffectHandle`.
  - `ctx.end_setup()` — closes render bracket.
  - `ctx.register_template(view, name)` — sets `ctx.template_id`.
  - `ctx.register_extra_template(view, name) -> UInt32` — for multi-template apps.
  - `ctx.setup_view(view, name)` — combines `end_setup()` + `register_view()` (with inline event extraction + auto-numbered dyn_text).
  - `ctx.register_view(view, name)` — processes inline events (`onclick_add` etc.), auto-numbers `dyn_text()`, registers handlers.
  - `ctx.render_builder()` → `RenderBuilder` (auto-adds registered event attrs on `build()`).
  - `ctx.mount(writer, vnode_idx)` — emit templates + create + append to root.
  - `ctx.flush(writer, new_idx)` — diff + finalize (convenience).
  - `ctx.dispatch_event(handler_id, event_type)` → Bool.
  - `ctx.consume_dirty()` → Bool.
  - `ctx.on_click_add()`, `on_click_sub()`, `on_click_set()`, `on_click_toggle()` — manual handler registration.
  - `ctx.register_handler(entry)` — raw handler registration.
  - `ctx.create_child_scope()` / `ctx.destroy_child_scopes(ids)` — for keyed list items.
  - `ctx.flush_fragment(writer, slot, frag_idx)` / `ctx.build_empty_fragment()` / `ctx.push_fragment_child()` — fragment lifecycle.
  - `ctx.vnode_builder()` / `ctx.vnode_builder_for(tmpl_id)` — VNode construction.
- **`FragmentSlot`** — tracks empty↔populated transitions for dynamic keyed lists.
- **Lifecycle helpers**: `mount_vnode()`, `diff_and_finalize()`, `flush_fragment()`.

## Apps (`src/apps/`)

All three apps use `ComponentContext` with constructor-based setup.

### CounterApp (`counter.mojo`) — simplest example

```txt
struct CounterApp:
    var ctx: ComponentContext
    var count: SignalI32
    fn __init__: ctx.create() → use_signal → setup_view(inline events) 
    fn render: ctx.render_builder() → add_dyn_text → build()
```

Lifecycle: `counter_app_init()` → `counter_app_rebuild()` → `counter_app_handle_event()` → `counter_app_flush()`.

### TodoApp (`todo.mojo`) — keyed lists, multiple templates, custom handlers

```txt
struct TodoApp:
    var ctx: ComponentContext
    var list_version: SignalI32
    var item_template_id: UInt32
    var items: List[TodoItem]
    var item_slot: FragmentSlot
    var handler_map: List[HandlerItemMapping]
    var item_scope_ids: List[UInt32]
    fn __init__: register_template("todo-app") + register_extra_template("todo-item")
    fn build_item_vnode: per-item child scope + VNodeBuilder + handler registration
    fn build_items_fragment: destroy old scopes → empty fragment → build each item
    fn handle_event: lookup handler_map → toggle/remove item
```

### BenchmarkApp (`bench.mojo`) — js-framework-benchmark, same pattern as todo

Two signals: `version` (list changes), `selected` (highlight row).
Operations: create_rows, append_rows, update_every_10th, select_row, swap_rows, remove_row, clear_rows.

## WASM Export Pattern (`src/main.mojo`)

All exports follow this pattern — thin wrappers forwarding to app modules:

```txt
@export fn counter_init() -> Int64:     return _to_i64(counter_app_init())
@export fn counter_flush(...) -> Int32: ...alloc writer...forward...free writer
@export fn counter_count_value(app_ptr: Int64) -> Int32:
    return _get[CounterApp](app_ptr)[0].count.peek()
```

Helpers: `_to_i64(ptr)`, `_get[T](i64) -> UnsafePointer[T]`, `_b2i(Bool) -> Int32`, `_alloc_writer()`, `_free_writer()`.

## TypeScript Runtime (`runtime/`)

- `mod.ts` — WASM instantiation entry point.
- `interpreter.ts` — DOM stack machine reading binary mutations.
- `events.ts` — `EventBridge` captures DOM events, dispatches handler IDs to WASM.
- `templates.ts` — `TemplateCache` registers templates from `RegisterTemplate` mutations.
- `strings.ts` — Mojo `String` ABI (SSO layout: inline ≤23 bytes, heap pointer otherwise).
- `memory.ts` — bump allocator for WASM linear memory.

## File Size Reference

| File | Lines | Role |
|------|-------|------|
| `src/main.mojo` | ~2,500 | All @export wrappers |
| `src/component/context.mojo` | ~950 | ComponentContext + RenderBuilder + tree processing |
| `src/component/lifecycle.mojo` | ~350 | FragmentSlot + mount/diff helpers |
| `src/component/app_shell.mojo` | ~350 | AppShell (low-level) |
| `src/apps/counter.mojo` | ~120 | Counter app |
| `src/apps/todo.mojo` | ~475 | Todo app |
| `src/apps/bench.mojo` | ~475 | Benchmark app |
| `src/vdom/dsl.mojo` | ~800 | Node DSL + el_* helpers + to_template |
| `src/vdom/vnode.mojo` | ~600 | VNode + VNodeStore + VNodeBuilder |
| `src/signals/runtime.mojo` | ~500 | Reactive runtime |
| `src/mutations/diff.mojo` | ~500 | DiffEngine (keyed reconciliation) |
| `PLAN.md` | ~4,000 | Full development plan (Phases 0–14) |

## Common Patterns

**Adding a signal to a component**: `var foo = self.ctx.use_signal(0)` in setup, `foo.peek()` to read, `foo += 1` or `foo.set(v)` to write.

**Bump version signal**: `self.version += 1` (triggers re-render via scope subscription).

**Inline events in DSL**: `el_button(List[Node](text("Up!"), onclick_add(count, 1)))` — extracted by `register_view()` / `setup_view()`.

**Manual events**: `var hid = ctx.register_handler(HandlerEntry.custom(scope_id, "click"))`, then `vb.add_dyn_event("click", hid)`.

**Keyed list rebuild**: destroy_child_scopes → clear handler_map → build_empty_fragment → for each item: create_child_scope → VNodeBuilder → register handlers → push_fragment_child.

**Flush lifecycle**: `if not ctx.consume_dirty(): return 0` → rebuild → `ctx.flush(writer, new_idx)` or `ctx.flush_fragment(writer, slot, frag_idx)` + `writer.finalize()`.
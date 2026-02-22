# wasm-mojo — AI Agent Context

> Compact quick-reference for AI agents. For project overview, architecture,
> build commands, and test infrastructure see [README.md](README.md).
> For development history see [CHANGELOG.md](CHANGELOG.md).

## Mojo Constraints

- **No closures/function pointers in WASM** — event handlers are action-based structs.
- **`@export` only works in main.mojo** — submodule exports get DCE'd. All ~430 WASM exports are thin wrappers in `src/main.mojo` forwarding to submodule implementations.
- **Single-threaded** — no sync needed.
- **Operator overloading** works (SignalI32 has `+=`, `-=`, `peek()`, `set()`).
- **Format**: `mojo format <file>` — pre-commit hooks run this automatically.
- **Commit messages**: `feat(wasm-mojo): Uppercase description` — commitlint enforced, allowed types: `feat`, `fix`, `chore`, `doc`.

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
- **Multi-arg `el_*` overloads** — 1–5 `Node` argument overloads for all 38 element helpers, eliminating `List[Node](...)` wrappers. Uses `var` ownership + `^` transfer. Example: `el_div(el_h1(dyn_text()), el_button(text("Up!"), onclick_add(count, 1)))`.
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
- **`KeyedList`** (`src/component/keyed_list.mojo`) — bundles `FragmentSlot` + child scope IDs + item template ID + handler map for keyed-list components. Methods: `begin_rebuild(ctx)` (destroy old scopes + clear handler map, return empty fragment), `begin_item(key, ctx)` → `ItemBuilder` (Phase 17 — create scope + keyed VNodeBuilder in one call), `get_action(handler_id)` → `HandlerAction` (Phase 17 — dispatch lookup), `create_scope(ctx)` (create + track child scope), `item_builder(key, ctx)` (keyed VNodeBuilder), `push_child(ctx, frag, child)`, `flush(ctx, writer, frag)` (fragment transitions), `init_slot(anchor, frag)`, `handler_count()`.
- **`ItemBuilder`** — Phase 17 ergonomic per-item builder wrapping VNodeBuilder + child scope + handler map pointer. Methods: `add_dyn_text(value)`, `add_dyn_text_attr(name, value)`, `add_dyn_bool_attr(name, value)`, `add_dyn_event(event, handler_id)`, `add_custom_event(event, action_tag, data)` (registers handler + maps action + adds event attr in one call), `add_dyn_placeholder()`, `index()`.
- **`HandlerAction`** — Phase 17 result of `KeyedList.get_action(handler_id)`. Fields: `tag: UInt8` (app-defined action), `data: Int32` (e.g. item ID), `found: Bool`.
- **Lifecycle helpers**: `mount_vnode()`, `diff_and_finalize()`, `flush_fragment()`.

## App Architectures (`src/apps/`)

All three apps use `ComponentContext` with constructor-based setup and multi-arg `el_*` overloads. TodoApp and BenchmarkApp use Phase 17 `ItemBuilder` + `HandlerAction` for ergonomic per-item building and dispatch.

### CounterApp (`counter.mojo`) — simplest example

```txt
struct CounterApp:
    var ctx: ComponentContext
    var count: SignalI32
    fn __init__: ctx.create() → use_signal → setup_view(inline events, multi-arg el_*)
    fn render: ctx.render_builder() → add_dyn_text → build()
```

Lifecycle: `counter_app_init()` → `counter_app_rebuild()` → `counter_app_handle_event()` → `counter_app_flush()`.

### TodoApp (`todo.mojo`) — keyed lists, multiple templates, custom handlers

```txt
struct TodoApp:
    var ctx: ComponentContext
    var list_version: SignalI32
    var items: KeyedList          # bundles template_id + FragmentSlot + scope_ids + handler_map
    var data: List[TodoItem]
    var add_handler: UInt32
    fn __init__: register_template("todo-app") + KeyedList(register_extra_template("todo-item"))
    fn build_item_vnode: items.begin_item(key, ctx) → ib.add_custom_event() (Phase 17)
    fn build_items_fragment: items.begin_rebuild → build each item → items.push_child
    fn handle_event: items.get_action(handler_id) → toggle/remove item (Phase 17)
```

### BenchmarkApp (`bench.mojo`) — js-framework-benchmark, same pattern as todo

```txt
struct BenchmarkApp:
    var ctx: ComponentContext
    var version: SignalI32
    var selected: SignalI32
    var rows_list: KeyedList      # bundles template_id + FragmentSlot + scope_ids + handler_map
    var rows: List[BenchRow]
```

Two signals: `version` (list changes), `selected` (highlight row).
Operations: create_rows, append_rows, update_every_10th, select_row, swap_rows, remove_row, clear_rows.
Per-row build uses `begin_item()` + `add_custom_event()` (Phase 17).

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
| `src/apps/counter.mojo` | ~115 | Counter app |
| `src/apps/todo.mojo` | ~450 | Todo app (uses KeyedList + ItemBuilder) |
| `src/apps/bench.mojo` | ~430 | Benchmark app (uses KeyedList + ItemBuilder) |
| `src/component/keyed_list.mojo` | ~595 | KeyedList + ItemBuilder + HandlerAction |
| `src/vdom/dsl.mojo` | ~2,775 | Node DSL + el_* helpers + multi-arg overloads + to_template |
| `src/vdom/vnode.mojo` | ~600 | VNode + VNodeStore + VNodeBuilder |
| `src/signals/runtime.mojo` | ~500 | Reactive runtime |
| `src/mutations/diff.mojo` | ~500 | DiffEngine (keyed reconciliation) |
| `CHANGELOG.md` | ~170 | Development history (Phases 0–17) |

## Common Patterns

**Adding a signal to a component**: `var foo = self.ctx.use_signal(0)` in setup, `foo.peek()` to read, `foo += 1` or `foo.set(v)` to write.

**Bump version signal**: `self.version += 1` (triggers re-render via scope subscription).

**Inline events in DSL**: `el_button(text("Up!"), onclick_add(count, 1))` — multi-arg overloads, extracted by `register_view()` / `setup_view()`.

**Manual events**: `var hid = ctx.register_handler(HandlerEntry.custom(scope_id, "click"))`, then `vb.add_dyn_event("click", hid)`.

**Keyed list rebuild (Phase 17 — via ItemBuilder)**: `var frag = self.items.begin_rebuild(ctx)` → for each item: `var ib = items.begin_item(key, ctx)` → `ib.add_dyn_text(...)` → `ib.add_custom_event("click", ACTION_TAG, item_id)` → `items.push_child(ctx, frag, ib.index())`.

**Keyed list dispatch (Phase 17 — via HandlerAction)**: `var action = self.items.get_action(handler_id)` → `if action.found: match action.tag`.

**Keyed list rebuild (Phase 16 — manual)**: `var frag = self.items.begin_rebuild(ctx)` → for each item: `items.create_scope(ctx)` → `items.item_builder(key, ctx)` → register handlers → `items.push_child(ctx, frag, idx)`.

**Keyed list flush (via KeyedList)**: `items.flush(ctx, writer, frag_idx)` + `writer.finalize()`.

**Flush lifecycle**: `if not ctx.consume_dirty(): return 0` → rebuild → `ctx.flush(writer, new_idx)` or `items.flush(ctx, writer, frag_idx)` + `writer.finalize()`.

## Deferred Abstractions (Blocked on Mojo Roadmap)

- **Closure event handlers** → blocked on Lambda syntax + Closure refinement (Phase 1, 🚧). Would eliminate `ItemBuilder.add_custom_event()` + `get_action()`.
- **`rsx!` macro** → blocked on Hygienic importable macros (Phase 2, ⏰). Would enable compile-time DSL like Dioxus.
- **`for` loops in views** → blocked on macros (Phase 2, ⏰). Currently iteration happens in build functions.
- **Generic `Signal[T]`** → blocked on Conditional conformance (Phase 1, 🚧). Currently only `SignalI32` / `MemoI32`.
- **Dynamic component dispatch** → blocked on Existentials / dynamic traits (Phase 2, ⏰).
- **Pattern matching on actions** → blocked on ADTs & pattern matching (Phase 2, ⏰). Currently `if/elif` chains.
- **Async data loading / suspense** → blocked on First-class async (Phase 2, ⏰).
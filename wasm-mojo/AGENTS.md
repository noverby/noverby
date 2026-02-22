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

- `Runtime` — reactive runtime: signal store, string store, scope tracking, context management.
- `SignalStore` — type-erased storage for fixed-size value signals (Int32). Uses raw-byte memcpy — safe for value types only.
- `StringStore` (`signals/runtime.mojo`) — Phase 19 safe heap-string storage with slab-style free-list slot reuse. Methods: `create(initial) -> UInt32`, `read(key) -> String`, `write(key, value)`, `destroy(key)`, `count()`, `contains(key)`. Lives as `Runtime.strings` field. Solves the problem that `SignalStore` (memcpy-based) is unsafe for heap types like String.
- `SignalI32` (`signals/handle.mojo`) — ergonomic handle with `peek()`, `set()`, `+=`, `-=`. Holds key + runtime pointer.
- `SignalBool` (`signals/handle.mojo`) — Phase 18 ergonomic boolean signal wrapping Int32 (0/1). `get() -> Bool`, `read() -> Bool` (with context subscription), `set(Bool)`, `toggle()`, `peek_i32() -> Int32`, `version()`, `__str__()` ("true"/"false"). Created via `ctx.use_signal_bool(initial)` or `ctx.create_signal_bool(initial)`.
- `SignalString` (`signals/handle.mojo`) — Phase 19 ergonomic reactive string signal. Wraps a `string_key` (index in StringStore) + `version_key` (companion Int32 signal in SignalStore for subscriber tracking). `get() -> String` / `peek() -> String` (read without subscribing), `read() -> String` (subscribe context via version signal), `set(String)` (write + bump version → marks subscribers dirty), `version() -> UInt32`, `is_empty() -> Bool`, `__str__() -> String`. Created via `ctx.use_signal_string(initial)` or `ctx.create_signal_string(initial)`.
- `MemoI32` — derived signal with lazy recomputation and auto dependency tracking.
- `EffectHandle` — reactive side effects.

### Scopes (`src/scope/`)

- `ScopeState` — lifecycle unit with hooks, context, error boundaries.
- `ScopeArena` — slab allocator for scopes. Parent→child hierarchy.

### Virtual DOM (`src/vdom/`)

- `Node` (DSL union) — `text()`, `dyn_text()`, `dyn_node()`, `attr()`, `dyn_attr()`, `el_div()`, `el_button()`, etc.
- **Multi-arg `el_*` overloads** — 1–5 `Node` argument overloads for all 38 element helpers, eliminating `List[Node](...)` wrappers. Uses `var` ownership + `^` transfer. Example: `el_div(el_h1(dyn_text()), el_button(text("Up!"), onclick_add(count, 1)))`.
- Inline event constructors: `onclick_add(signal, delta)`, `onclick_sub()`, `onclick_set()`, `onclick_toggle()`, `on_event()`.
- **Conditional helpers** (Phase 18): `class_if(condition, name) -> String` (returns name or ""), `class_when(condition, true_class, false_class) -> String`, `text_when(condition, true_text, false_text) -> String`. Eliminate if/else boilerplate for dynamic attributes and text.
- `dyn_text()` with no args → auto-numbered (sentinel `DYN_TEXT_AUTO`).
- `to_template(node, name)` → `Template` (static structure for DOM cloning).
- `VNode` — runtime instance of a template with dynamic slots.
- `VNodeBuilder` — fills dynamic text/attr/event slots on a VNode. `add_dyn_text(value)`, `add_dyn_text_attr(name, value)`, `add_dyn_bool_attr(name, value)`, `add_dyn_event(event, handler_id)`, `add_dyn_placeholder()`.
- `VNodeStore` — arena for VNode storage.

### Mutations (`src/mutations/`)

- `CreateEngine` — walks VNode tree, emits create mutations (initial mount).
- `DiffEngine` — compares old/new VNode trees, emits minimal update mutations (keyed reconciliation).
- `MutationWriter` (`src/bridge/protocol.mojo`) — writes binary opcodes to shared buffer.

### Events (`src/events/`)

- `HandlerEntry` — action-based handler (signal_add, signal_sub, signal_set, signal_toggle, signal_set_string, custom).
  - Phase 20: `HandlerEntry.signal_set_string(scope_id, string_key, version_key, event_name)` creates a handler that writes a string event value to a `SignalString`. Stores `string_key` in the `signal_key` field and `version_key` in the `operand` field (cast to Int32).
- `HandlerRegistry` — maps handler IDs → entries. Scope-scoped cleanup.
- Action tags: `ACTION_NONE` (0), `ACTION_SIGNAL_SET_I32` (1), `ACTION_SIGNAL_ADD_I32` (2), `ACTION_SIGNAL_SUB_I32` (3), `ACTION_SIGNAL_TOGGLE` (4), `ACTION_SIGNAL_SET_INPUT` (5), `ACTION_SIGNAL_SET_STRING` (6, Phase 20), `ACTION_CUSTOM` (255).

### Component Layer (`src/component/`)

- **`AppShell`** — bundles Runtime + VNodeStore + ElementIdAllocator + Scheduler. Low-level API.
- **`ComponentContext`** — ergonomic wrapper over AppShell. High-level API for apps:
  - `ComponentContext.create()` → allocates shell, root scope, begins render bracket.
  - `ctx.use_signal(initial)` → `SignalI32` (auto-subscribes scope).
  - `ctx.use_signal_bool(initial)` → `SignalBool` (auto-subscribes scope).
  - `ctx.use_signal_string(initial)` → `SignalString` (auto-subscribes scope). Phase 19.
  - `ctx.use_memo(initial)` → `MemoI32`.
  - `ctx.use_effect()` → `EffectHandle`.
  - `ctx.end_setup()` — closes render bracket.
  - `ctx.create_signal_string(initial)` → `SignalString` (no hooks, no subscription). Phase 19.
  - `ctx.register_template(view, name)` — sets `ctx.template_id`.
  - `ctx.register_extra_template(view, name) -> UInt32` — for multi-template apps.
  - `ctx.setup_view(view, name)` — combines `end_setup()` + `register_view()` (with inline event extraction + auto-numbered dyn_text).
  - `ctx.register_view(view, name)` — processes inline events (`onclick_add` etc.), auto-numbers `dyn_text()`, registers handlers.
  - `ctx.render_builder()` → `RenderBuilder` (auto-adds registered event attrs on `build()`). Phase 19 adds `add_dyn_text_signal(SignalString)`.
  - `ctx.mount(writer, vnode_idx)` — emit templates + create + append to root.
  - `ctx.flush(writer, new_idx)` — diff + finalize (convenience).
  - `ctx.dispatch_event(handler_id, event_type)` → Bool.
  - `ctx.dispatch_event_with_string(handler_id, event_type, value: String)` → Bool. Phase 20: dispatches string payloads — for `ACTION_SIGNAL_SET_STRING` handlers, writes value to the target `SignalString`; falls back to normal dispatch for other action types.
  - `ctx.consume_dirty()` → Bool.
  - `ctx.on_click_add()`, `on_click_sub()`, `on_click_set()`, `on_click_toggle()` — manual handler registration.
  - `ctx.register_handler(entry)` — raw handler registration.
  - `ctx.create_child_scope()` / `ctx.destroy_child_scopes(ids)` — for keyed list items.
  - `ctx.flush_fragment(writer, slot, frag_idx)` / `ctx.build_empty_fragment()` / `ctx.push_fragment_child()` — fragment lifecycle.
  - `ctx.vnode_builder()` / `ctx.vnode_builder_for(tmpl_id)` — VNode construction.
- **`FragmentSlot`** — tracks empty↔populated transitions for dynamic keyed lists.
- **`KeyedList`** (`src/component/keyed_list.mojo`) — bundles `FragmentSlot` + child scope IDs + item template ID + handler map for keyed-list components. Methods: `begin_rebuild(ctx)` (destroy old scopes + clear handler map, return empty fragment), `begin_item(key, ctx)` → `ItemBuilder` (Phase 17 — create scope + keyed VNodeBuilder in one call), `get_action(handler_id)` → `HandlerAction` (Phase 17 — dispatch lookup), `create_scope(ctx)` (create + track child scope), `item_builder(key, ctx)` (keyed VNodeBuilder), `push_child(ctx, frag, child)`, `flush(ctx, writer, frag)` (fragment transitions), `init_slot(anchor, frag)`, `handler_count()`.
- **`ItemBuilder`** — Phase 17 ergonomic per-item builder wrapping VNodeBuilder + child scope + handler map pointer. Methods: `add_dyn_text(value)`, `add_dyn_text_signal(SignalString)` (Phase 19 — read signal + add as dyn text), `add_dyn_text_attr(name, value)`, `add_dyn_bool_attr(name, value)`, `add_dyn_event(event, handler_id)`, `add_custom_event(event, action_tag, data)` (registers handler + maps action + adds event attr in one call), `add_class_if(condition, class_name)` (Phase 18 — conditional CSS class in one call), `add_class_when(condition, true_class, false_class)` (Phase 18 — binary class switching), `add_dyn_placeholder()`, `index()`.
- **`HandlerAction`** — Phase 17 result of `KeyedList.get_action(handler_id)`. Fields: `tag: UInt8` (app-defined action), `data: Int32` (e.g. item ID), `found: Bool`.
- **Lifecycle helpers**: `mount_vnode()`, `diff_and_finalize()`, `flush_fragment()`.

## App Architectures (`examples/`)

All three apps use `ComponentContext` with constructor-based setup and multi-arg `el_*` overloads. TodoApp and BenchmarkApp use Phase 17 `ItemBuilder` + `HandlerAction` for ergonomic per-item building and dispatch, with Phase 18 conditional helpers (`add_class_if`, `text_when`) to eliminate if/else boilerplate. Phase 19 adds `SignalString` for reactive string state — TodoApp's `input_text` field was migrated from plain `String` to `SignalString` via `create_signal_string()` (M19.7). Phase 20 adds string event dispatch infrastructure (`ACTION_SIGNAL_SET_STRING`, `dispatch_event_with_string`) enabling JS → WASM string value flow for input events. Phase 20.5 migrates the TodoApp to fully WASM-driven input binding using `bind_value()`, `oninput_set_string()`, and `onclick_custom()` — JS has no special-casing for any handler. Phase 21 introduces `launch()` (`examples/lib/app.js`) — a convention-based app launcher that eliminates per-app boot boilerplate. Phase 22 adds WASM-driven Enter key handling; counter and todo now use identical zero-config launch() calls. Phase 23 converges bench to launch() with `onBoot` for toolbar wiring and event delegation — all three apps now use the shared boot infrastructure.

### CounterApp (`counter.mojo`) — simplest example

```txt
struct CounterApp:
    var ctx: ComponentContext
    var count: SignalI32
    fn __init__: ctx.create() → use_signal → setup_view(inline events, multi-arg el_*)
    fn render: ctx.render_builder() → add_dyn_text → build()
```

Lifecycle: `counter_app_init()` → `counter_app_rebuild()` → `counter_app_handle_event()` → `counter_app_flush()`.

### TodoApp (`todo.mojo`) — keyed lists, two-way input binding, Enter key, custom handlers, SignalString

```txt
struct TodoApp:
    var ctx: ComponentContext
    var list_version: SignalI32
    var input_text: SignalString   # create_signal_string (no subscription, write-buffer)
    var items: KeyedList          # bundles template_id + FragmentSlot + scope_ids + handler_map
    var data: List[TodoItem]
    var add_handler: UInt32       # auto-registered by register_view() via onclick_custom()
    fn __init__: create_signal_string → register_view("todo-app" with bind_value + oninput_set_string
                 + onclick_custom) → view_event_handler_id(1) for add_handler
                 + KeyedList(register_extra_template("todo-item"))
    fn render: ctx.render_builder() → add_dyn_placeholder → build() (auto-populates bindings)
    fn build_item_vnode: items.begin_item(key, ctx) → ib.add_custom_event() (Phase 17)
    fn build_items_fragment: items.begin_rebuild → build each item → items.push_child
    fn handle_event: if add_handler → read input_text.peek(), add_item, clear signal
                     else → items.get_action(handler_id) → toggle/remove item
```

Phase 20.5 migration: TodoApp now uses `register_view()` with inline event/binding helpers for the app template. The input element has `bind_value(input_text)` + `oninput_set_string(input_text)` for Dioxus-style two-way binding. The Add button uses `onclick_custom()` — an inline custom handler auto-registered by `register_view()`. `handle_event()` handles the Add action entirely in WASM: reads `input_text.peek()`, calls `add_item()`, clears via `input_text.set("")`. `render()` uses `render_builder()` which auto-populates bind_value (reads signal → "value" attr) and event listeners. `todo_app_flush()` re-renders the app shell via `ctx.diff()` (to catch bind_value changes) before flushing items via KeyedList. JS dispatches events uniformly — `input`/`change` events go through `todo_dispatch_string()`, all others through `todo_handle_event()` — no special-casing. WASM exports: `todo_dispatch_string` for string events, `todo_add_handler_id` for the Add handler ID, plus existing `todo_set_input`, `todo_input_version`, `todo_input_is_empty`.

### BenchmarkApp (`bench.mojo`) — js-framework-benchmark, WASM-rendered toolbar + keyed rows

```txt
struct BenchmarkApp:
    var ctx: ComponentContext
    var version: SignalI32
    var selected: SignalI32
    var rows_list: KeyedList      # bundles template_id + FragmentSlot + scope_ids + handler_map
    var rows: List[BenchRow]
    var op_name: String           # P24.4: dyn_text[0] — "Ready" or operation name
    var timing_text: String       # P24.4: dyn_text[1] — "" or " — X.Yms"
    var row_count_text: String    # P24.4: dyn_text[2] — "" or " · N rows"
    var create1k_handler: UInt32  # auto-registered by register_view() via onclick_custom()
    var create10k_handler: UInt32
    var append_handler: UInt32
    var update_handler: UInt32
    var swap_handler: UInt32
    var clear_handler: UInt32
    fn __init__: ctx.create() → use_signal(version, selected) → setup_view("bench-app"
                 with 6 onclick_custom buttons, 3 dyn_text status, dyn_node(3) for rows)
                 → view_event_handler_id(0..5) for toolbar handlers
                 + KeyedList(register_extra_template("bench-row"))
                 + op_name = "Ready", timing_text = "", row_count_text = ""
    fn render: ctx.render_builder() → add_dyn_text(op_name) → add_dyn_text(timing_text)
               → add_dyn_text(row_count_text) → add_dyn_placeholder → build()
    fn build_row_vnode: rows_list.begin_item(key, ctx) → ib.add_custom_event() (Phase 17)
    fn build_rows_fragment: rows_list.begin_rebuild → build each row → rows_list.push_child
    fn handle_event: t0 = performance_now()
                     if create1k_handler → create_rows(1000) → op_name/timing_text/row_count_text
                     elif create10k_handler → create_rows(10000) → op_name/timing_text/row_count_text
                     elif append_handler → append_rows(1000) → op_name/timing_text/row_count_text
                     elif update_handler → update_every_10th() → op_name/timing_text/row_count_text
                     elif swap_handler → swap_rows(1, 998) → op_name/timing_text/row_count_text
                     elif clear_handler → clear_rows() → op_name/timing_text/row_count_text
                     else → rows_list.get_action(handler_id) → select/remove row
```

Two signals: `version` (list changes), `selected` (highlight row).
Operations: create_rows, append_rows, update_every_10th, select_row, swap_rows, remove_row, clear_rows.
Per-row build uses `begin_item()` + `add_custom_event()` (Phase 17) + `add_class_if()` (Phase 18).

Phase 24.4: The status bar uses 3 separate `dyn_text` nodes: `dyn_text[0]` = `op_name` ("Ready" or operation name), `dyn_text[1]` = `timing_text` ("" or " — X.Yms"), `dyn_text[2]` = `row_count_text` ("" or " · N rows"). The row list placeholder is at `dyn_node(3)` (indices 0-2 occupied by dyn_text). `format_timing_ms(ms) -> String` returns timing with leading em-dash separator. `format_row_count(count) -> String` returns row count with leading middle-dot separator and comma-formatted number via `_format_number()`. Only changed text nodes receive `SetText` mutations on flush — e.g. update-every-10th changes timing but not row count. New exports: `bench_op_name(app_ptr) -> String`, `bench_timing_text(app_ptr) -> String`, `bench_row_count_text(app_ptr) -> String`. `bench_status_text` returns the concatenation of all three fields for backward compatibility.

Phase 24.3: `performance_now() -> Float64` WASM import via `external_call["performance_now", Float64]()`. Each toolbar operation in `handle_event()` is wrapped with before/after `performance_now()` calls; the elapsed time is formatted to 1 decimal place via `format_timing_ms(ms) -> String` and stored in `timing_text`. Zero JS-side timing code.

Phase 24.2: Uses `setup_view()` for the app shell template ("bench-app") with inline `onclick_custom()` for 6 toolbar buttons. The entire UI — heading, buttons, status bar, table with thead + tbody — is rendered from WASM. Root is `#root` (not `#tbody`). `handle_event()` routes both toolbar button clicks (via handler ID comparison) and row clicks (via `get_action()`). `render()` uses `render_builder()` which auto-populates event handlers; provides 3 `dyn_text()` nodes for status (dynamic_nodes[0-2]) and `dyn_placeholder()` for the keyed row list (dynamic_nodes[3]). **Important:** `dyn_text` and `dyn_node` share the same `dynamic_nodes` index space — three auto-numbered `dyn_text()` get indices 0-2, so `dyn_node` must use index 3. `bench_app_rebuild()` follows the todo pattern: emit templates → render shell → CreateEngine → extract `dyn_node[3]` anchor → init KeyedList slot. `bench_app_flush()` diffs app shell + flushes KeyedList. `bench/main.js` is now a 7-line `launch()` call (only `bufferCapacity` is bench-specific).

**Phase 24 — Bench zero app-specific JS convergence** (see also `examples/bench/main.js` header):

- **P24.1** ✅ — `bench_handle_event` with handler_map dispatch. `BenchmarkApp.handle_event(handler_id)` calls `rows_list.get_action(handler_id)` and routes to `select_row`/`remove_row` (same pattern as `TodoApp.handle_event`). `bench_handle_event` WASM export in `main.mojo`. EventBridge now dispatches row clicks directly — tbody event delegation JS eliminated.
- **P24.2** ✅ — WASM-rendered toolbar with `onclick_custom` handlers. Entire app shell (h1, 6 buttons, status 3 `dyn_text` at dynamic_nodes[0-2], table with thead + tbody > `dyn_node(3)` at dynamic_nodes[3]) rendered from WASM via `setup_view()`. Root changed from `#tbody` to `#root`. 6 handler IDs extracted via `view_event_handler_id()`. `handle_event()` routes toolbar button clicks to benchmark operations + existing row click dispatch. `bench/index.html` simplified to `<div id="root">` + styles. `bench/main.js` reduced to 7-line `launch()` call. Tests updated: `createDOM()` creates root div, DOM tests query rendered tbody, handler lifecycle tests account for 6 toolbar base handlers. **Gotcha:** `dyn_text` and `dyn_node` share the `dynamic_nodes` index space — three auto-numbered `dyn_text()` get indices 0-2, so `dyn_node` must use 3.
- **P24.3** ✅ — `performance.now()` WASM import for timing. `performance_now() -> Float64` via `external_call` — Mojo compiler emits unresolved symbol, `wasm-ld --allow-undefined` turns it into WASM import from `env` module, JS host provides `performance_now: () => performance.now()`. `format_timing_ms(ms) -> String` formats elapsed time to 1 decimal place with em-dash separator. `handle_event()` wraps each toolbar op with before/after `performance_now()`, stores formatted result in `timing_text`. `render()` emits `timing_text` as `dyn_text[1]` — diff detects change on flush, emits `SetText`. Added to `env.js` (browser), `env.ts` (Deno runtime), and `wasm_harness.mojo` (func[16]: deterministic mock clock, increments by 1.0 per call). WASM import count: 16 → 17. Exports: `bench_status_text(app_ptr) -> String`, `bench_handler_id_at(app_ptr, index) -> i32`.
- **P24.4** ✅ — Fine-grained status bar with 3 `dyn_text` nodes. Split single `status_text` into `op_name` (dyn_text[0]), `timing_text` (dyn_text[1]), `row_count_text` (dyn_text[2]). Row list placeholder moved from `dyn_node(1)` to `dyn_node(3)`. Added `format_timing_ms(ms) -> String` (timing only with separator), `format_row_count(count) -> String` (comma-formatted with separator), `_format_number(n) -> String` (comma thousands). New exports: `bench_op_name`, `bench_timing_text`, `bench_row_count_text`. `bench_status_text` returns concatenation for backward compat. `bench/main.js` structurally identical to counter/todo (only `bufferCapacity` override remains).

## WASM Export Pattern (`src/main.mojo`)

All exports follow this pattern — thin wrappers forwarding to app modules:

```txt
@export fn counter_init() -> Int64:     return _to_i64(counter_app_init())
@export fn counter_flush(...) -> Int32: ...alloc writer...forward...free writer
@export fn counter_count_value(app_ptr: Int64) -> Int32:
    return _get[CounterApp](app_ptr)[0].count.peek()
```

Helpers: `_to_i64(ptr)`, `_get[T](i64) -> UnsafePointer[T]`, `_b2i(Bool) -> Int32`, `_alloc_writer()`, `_free_writer()`.

**Naming convention for `launch()`**: The JS `launch({ app: "NAME" })` function discovers WASM exports by prefix — `{NAME}_init`, `{NAME}_rebuild`, `{NAME}_flush` (required), `{NAME}_handle_event` (optional — enables EventBridge dispatch; when missing, EventBridge is a no-op), and `{NAME}_dispatch_string` (optional, enables auto string dispatch for input/change/keydown events). New apps MUST follow this naming convention to be compatible with `launch()`.

## Browser Runtime (`examples/lib/`)

- `app.js` — **`launch(options)`**: Convention-based app launcher (Phase 21, updated Phase 22–24). Given `app: "counter"`, auto-discovers WASM exports by naming convention, sets up interpreter + EventBridge with smart dispatch (auto string dispatch when `{app}_dispatch_string` exists), runs initial mount, and calls optional `onBoot(handle)` for app-specific post-boot wiring. Returns `AppHandle` with `{ fns, appPtr, interp, bufPtr, rootEl, flush }`. Options: `app` (required), `wasm` (required URL), `root` (CSS selector, default `"#root"`), `bufferCapacity` (default 65536), `clearRoot` (default true), `onBoot` (optional callback). **Phase 22**: EventBridge smart dispatch extended to route `keydown` events through `dispatch_string`. **Phase 23**: `{app}_handle_event` made optional — when missing, EventBridge dispatch is a no-op (DOM listeners still attached). **Phase 24.2**: All three apps now use near-zero-config `launch()` (bench only needs `bufferCapacity`; no `onBoot`, no custom root).
- `boot.js` — Re-exports from `app.js`, `env.js`, `events.js`, `interpreter.js`, `protocol.js`, `strings.js`. Low-level API for advanced use cases that need direct control over the boot sequence.
- `env.js` — WASM memory management + import object + `loadWasm()` loader.
- `events.js` — `EventBridge` wires interpreter event mutations to a WASM dispatch callback.
- `interpreter.js` — DOM stack machine applying binary mutation buffers (shared with `runtime/interpreter.ts`).
- `protocol.js` — Op constants + `MutationReader` for binary mutation decoding.
- `strings.js` — `writeStringStruct()` writes JS strings into WASM linear memory as Mojo String structs.

**Example main.js files (Phase 23)**:

Counter — zero app-specific JS:

```txt
import { launch } from "../lib/app.js";
launch({ app: "counter", wasm: new URL("../../build/out.wasm", import.meta.url) });
```

Todo — zero app-specific JS (Enter key handled in WASM via `onkeydown_enter_custom`):

```txt
import { launch } from "../lib/app.js";
launch({ app: "todo", wasm: new URL("../../build/out.wasm", import.meta.url) });
```

Bench — launch() with `onBoot` for toolbar wiring & timing (row clicks handled by `bench_handle_event`):

```txt
import { launch } from "../lib/app.js";
launch({
  app: "bench",
  wasm: new URL("../../build/out.wasm", import.meta.url),
  root: "#tbody",
  bufferCapacity: 8 * 1024 * 1024,
  clearRoot: false,
  onBoot: ({ fns, appPtr, rootEl, flush }) => { /* toolbar buttons + timing */ },
});
```

## TypeScript Runtime (`runtime/`)

- `mod.ts` — WASM instantiation entry point.
- `interpreter.ts` — DOM stack machine reading binary mutations.
- `events.ts` — `EventBridge` captures DOM events, dispatches handler IDs to WASM. For `input`/`change` events, extracts `event.target.value` as a string and dispatches via `DispatchWithStringFn` → `writeStringStruct()` → WASM `dispatch_event_with_string` (Phase 20, M20.2). Falls back to numeric then default dispatch. Note: the browser `app.js` EventBridge additionally routes `keydown` events through string dispatch (Phase 22), but the TypeScript runtime does not yet implement this path.
- `templates.ts` — `TemplateCache` registers templates from `RegisterTemplate` mutations.
- `strings.ts` — Mojo `String` ABI (SSO layout: inline ≤23 bytes, heap pointer otherwise).
- `memory.ts` — bump allocator for WASM linear memory.

## String Event Dispatch (Phase 20)

Phase 20 adds the infrastructure for passing string values from DOM events to WASM `SignalString` signals, culminating in Dioxus-style two-way input binding. Phase 20.5 completes the story by migrating TodoApp to a fully WASM-driven Add flow.

**Dispatch path (M20.1 Mojo + M20.2 JS)**: JS EventBridge `handleEvent()` → for `input`/`change` events: extract `event.target.value` → `writeStringStruct(value)` → `dispatchWithStringFn(hid, eventType, stringPtr)` → WASM `dispatch_event_with_string(rt, handler_id, event_type, string_ptr)` → Runtime looks up handler → for `ACTION_SIGNAL_SET_STRING`: `write_signal_string(string_key, version_key, value)` → bumps version signal → marks subscriber scopes dirty. If string dispatch returns 0: try numeric fallback (`parseInt` + `dispatchWithValueFn`), then default no-payload dispatch. Non-input events (click, keydown, etc.) bypass string dispatch entirely.

**JS wiring (M20.2)**: `EventBridge.setDispatch(dispatch, dispatchWithValue?, dispatchWithString?)` — third parameter enables string dispatch. `AppConfig.handleEventWithString` optional callback; `createApp()` wires it to `EventBridge.dispatchWithStringFn` when provided. `DispatchWithStringFn` type: `(handlerId, eventType, stringPtr) => number`.

**Handler encoding**: `HandlerEntry.signal_set_string(scope_id, string_key, version_key, event_name)` repurposes existing fields — `signal_key` holds the `string_key` (StringStore index), `operand` holds the `version_key` (cast to Int32).

**WASM exports**: `handler_register_signal_set_string`, `dispatch_event_with_string`, `shell_dispatch_event_with_string`, `signal_create_string` (returns packed i64), `signal_string_key`, `signal_version_key`, `signal_peek_string`, `signal_write_string`, `signal_string_count`.

**DSL helpers (M20.3)**: `oninput_set_string(signal: SignalString) -> Node` creates a `NODE_EVENT` for `"input"` with `ACTION_SIGNAL_SET_STRING`. `onchange_set_string(signal: SignalString) -> Node` does the same for `"change"`. Both store `string_key` in `dynamic_index` and `Int32(version_key)` in `operand`, matching `HandlerEntry.signal_set_string()` encoding. Processed by `register_view()` / `setup_view()` which auto-assigns dyn_attr indices and registers handlers.

**Value binding (M20.4)**: `NODE_BIND_VALUE` node kind (tag 7) carries a SignalString reference (attr_name in `text`, string_key in `dynamic_index`, version_key in `operand`). `bind_value(signal: SignalString) -> Node` creates one with `attr_name="value"`; `bind_attr(attr_name, signal) -> Node` supports arbitrary attribute names. `_process_view_tree()` handles `NODE_BIND_VALUE` like `NODE_EVENT` — collects `_ValueBindingInfo` and replaces with `NODE_DYN_ATTR`. New `AutoBinding` tagged union (`AUTO_BIND_EVENT` / `AUTO_BIND_VALUE`) stores both events and value bindings in tree-walk order. `register_view()` interleaves them by `attr_idx`. `RenderBuilder.build()` auto-populates: events via `add_dyn_event()`, value bindings via `peek_signal_string()` + `add_dyn_text_attr()`. Falls back to legacy `EventBinding` path when no auto-bindings present.

**Custom inline handler (M20.5)**: `onclick_custom() -> Node` creates a `NODE_EVENT` for `"click"` with `ACTION_CUSTOM` (value 255), `signal_key=0`, `operand=0`. When dispatched, the runtime marks the scope dirty and returns False — the app's event handler then performs custom routing based on the handler ID. Use `ctx.view_event_handler_id(index)` after `register_view()` to retrieve the auto-registered handler ID.

**Two-way binding + custom action pattern (M20.3 + M20.4 + M20.5)**:

```mojo
el_input(
    attr("type", "text"),
    bind_value(input_text),          # M20.4: value attr ← signal
    oninput_set_string(input_text),   # M20.3: signal ← input event
),
el_button(text("Add"), onclick_custom()),  # M20.5: custom action in WASM
```

Equivalent Dioxus: `input { value: "{text}", oninput: move |e| text.set(e.value()) }` + `button { onclick: move |_| { add(&text); text.set(""); }, "Add" }`

**view_event_handler_id (M20.5)**: `ctx.view_event_handler_id(index: Int) -> UInt32` returns the handler ID for the Nth event registered by `register_view()` in tree-walk order. Example: after `register_view(el_div(el_input(bind_value(sig), oninput_set_string(sig)), el_button(text("Add"), onclick_custom()), ...))`, `view_event_handler_id(0)` = oninput handler, `view_event_handler_id(1)` = Add button handler.

**Keydown Enter handler (Phase 22)**: `onkeydown_enter_custom() -> Node` creates a `NODE_EVENT` for `"keydown"` with `ACTION_KEY_ENTER_CUSTOM` (value 7), `signal_key=0`, `operand=0`. When dispatched via `dispatch_event_with_string()`, the runtime checks the string payload (the key name from `event.key`) — only `"Enter"` triggers the action (marks scope dirty, returns True); all other keys are silently ignored (returns False). The app's `handle_event()` then performs custom routing based on the handler ID, same as `ACTION_CUSTOM`. Use `ctx.view_event_handler_id(index)` after `register_view()` to retrieve the auto-registered handler ID.

**JS keydown dispatch (Phase 22)**: The `launch()` EventBridge in `app.js` routes `keydown` events through `dispatch_string` when `{app}_dispatch_string` exists. It sends `event.key` as the string payload. If the WASM handler accepts the key (returns 1 — e.g. `ACTION_KEY_ENTER_CUSTOM` matched "Enter"), the bridge also calls `handle_event` for app-level routing. If rejected (returns 0), no further dispatch occurs. This two-step dispatch (string filter → app routing) enables WASM-driven keyboard shortcuts with zero app-specific JS.

**Phase 22 TodoApp pattern (Enter key + Add button)**: `view_event_handler_id(0)` = oninput handler, `view_event_handler_id(1)` = Enter key handler, `view_event_handler_id(2)` = Add button handler. Both Enter key and Add button handler IDs are checked in `handle_event()` to trigger the same Add logic. The template uses:

```mojo
el_input(
    attr("type", "text"),
    bind_value(input_text),
    oninput_set_string(input_text),
    onkeydown_enter_custom(),
),
el_button(text("Add"), onclick_custom()),
```

## Node Kind Tags (`src/vdom/dsl.mojo`)

| Tag | Value | Description |
|-----|-------|-------------|
| `NODE_TEXT` | 0 | Static text content |
| `NODE_ELEMENT` | 1 | HTML element with tag, children, attrs |
| `NODE_DYN_TEXT` | 2 | Dynamic text placeholder (slot index) |
| `NODE_DYN_NODE` | 3 | Dynamic node placeholder (slot index) |
| `NODE_STATIC_ATTR` | 4 | Static attribute (name + value) |
| `NODE_DYN_ATTR` | 5 | Dynamic attribute placeholder (slot index) |
| `NODE_EVENT` | 6 | Inline event handler (action + signal + operand) |
| `NODE_BIND_VALUE` | 7 | Value binding (SignalString → dynamic attr) (Phase 20, M20.4) |

## Handler Action Tags (`src/events/registry.mojo`)

| Tag | Value | Description |
|-----|-------|-------------|
| `ACTION_NONE` | 0 | No-op (marks scope dirty) |
| `ACTION_SIGNAL_SET_I32` | 1 | `signal.set(operand)` |
| `ACTION_SIGNAL_ADD_I32` | 2 | `signal += operand` |
| `ACTION_SIGNAL_SUB_I32` | 3 | `signal -= operand` |
| `ACTION_SIGNAL_TOGGLE` | 4 | `signal.set(!signal.get())` |
| `ACTION_SIGNAL_SET_INPUT` | 5 | `signal.set(input_value)` |
| `ACTION_SIGNAL_SET_STRING` | 6 | `string_signal.set(string_value)` (Phase 20) |
| `ACTION_KEY_ENTER_CUSTOM` | 7 | Fires only when key == "Enter" (Phase 22) |
| `ACTION_CUSTOM` | 255 | No Mojo-side action; marks scope dirty for app routing |

## File Size Reference

| File | Lines | Role |
|------|-------|------|
| `src/main.mojo` | ~2,600 | All @export wrappers |
| `src/signals/handle.mojo` | ~670 | SignalI32 + SignalBool + SignalString + MemoI32 + EffectHandle |
| `src/signals/runtime.mojo` | ~630 | Reactive runtime + SignalStore + StringStore |
| `src/component/context.mojo` | ~1,040 | ComponentContext + RenderBuilder + tree processing + view_event_handler_id |
| `src/component/lifecycle.mojo` | ~350 | FragmentSlot + mount/diff helpers |
| `src/component/app_shell.mojo` | ~350 | AppShell (low-level) |
| `examples/counter/counter.mojo` | ~115 | Counter app |
| `examples/todo/todo.mojo` | ~520 | Todo app (M20.5: WASM-driven Add, bind_value, oninput_set_string, onclick_custom) |
| `examples/bench/bench.mojo` | ~985 | Benchmark app (uses KeyedList + ItemBuilder + performance_now timing + 3 dyn_text status bar) |
| `src/component/keyed_list.mojo` | ~595 | KeyedList + ItemBuilder + HandlerAction |
| `src/vdom/dsl.mojo` | ~2,900 | Node DSL + el_* helpers + multi-arg overloads + conditional helpers + onclick_custom + to_template |
| `src/vdom/vnode.mojo` | ~600 | VNode + VNodeStore + VNodeBuilder |
| `src/mutations/diff.mojo` | ~500 | DiffEngine (keyed reconciliation) |
| `runtime/events.ts` | ~375 | EventBridge + DispatchWithStringFn (M20.2) |
| `runtime/app.ts` | ~370 | createApp + createCounterApp + AppConfig with handleEventWithString |
| `runtime/types.ts` | ~690 | WasmExports interface (Phase 20 string dispatch exports) |
| `test-js/events.test.ts` | ~650 | EventBridge string dispatch tests (unit + WASM integration) |
| `test-js/dsl.test.ts` | ~620 | DSL tests incl. M20.3/M20.4/M20.5 binding + onclick_custom tests |
| `test-js/todo.test.ts` | ~1,060 | Todo app tests incl. M20.5 WASM-driven Add flow tests |
| `CHANGELOG.md` | ~230 | Development history (Phases 0–20) |

## Common Patterns

**String event dispatch (Phase 20 — manual)**: Register a handler with `HandlerEntry.signal_set_string(scope_id, signal.string_key, signal.version_key, String("input"))`, then dispatch from JS via `dispatch_event_with_string(rt, handler_id, event_type, string_value)`. The runtime writes the string to the `SignalString` and bumps the version signal.

**Inline string event binding (Phase 20 — M20.3)**: `oninput_set_string(signal)` / `onchange_set_string(signal)` create `NODE_EVENT` nodes with `ACTION_SIGNAL_SET_STRING`. Used with `register_view()` / `setup_view()` for automatic handler registration: `el_input(oninput_set_string(name))`.

**Two-way input binding (Phase 20 — M20.3 + M20.4)**: Combine `bind_value(signal)` (auto-populates `value` attribute at render time) with `oninput_set_string(signal)` (writes input value back to signal): `el_input(attr("type", "text"), bind_value(text), oninput_set_string(text))`. The `RenderBuilder.build()` reads the signal and emits the `value` attr automatically. For custom attribute names, use `bind_attr("placeholder", signal)`.

**Adding a signal to a component**: `var foo = self.ctx.use_signal(0)` in setup, `foo.peek()` to read, `foo += 1` or `foo.set(v)` to write.

**Adding a bool signal**: `var flag = self.ctx.use_signal_bool(False)` in setup, `flag.get()` to read, `flag.set(True)` or `flag.toggle()` to write.

**Adding a string signal**: `var name = self.ctx.use_signal_string(String("hello"))` in setup, `name.get()` / `name.peek()` to read, `name.set(String("world"))` to write, `name.read()` to read with subscription, `name.is_empty()` to check, `String(name)` for interpolation. For non-reactive string state (write-buffer), use `ctx.create_signal_string(initial)` instead — no hook registration, no scope subscription (see TodoApp `input_text`).

**Bump version signal**: `self.version += 1` (triggers re-render via scope subscription).

**Inline events in DSL**: `el_button(text("Up!"), onclick_add(count, 1))` — multi-arg overloads, extracted by `register_view()` / `setup_view()`.

**Inline custom events (M20.5)**: `el_button(text("Add"), onclick_custom())` — creates NODE_EVENT with ACTION_CUSTOM, auto-registered by `register_view()`. Retrieve handler ID via `ctx.view_event_handler_id(index)` for app-specific routing.

**Manual events**: `var hid = ctx.register_handler(HandlerEntry.custom(scope_id, "click"))`, then `vb.add_dyn_event("click", hid)`.

**Keyed list rebuild (Phase 17+18 — via ItemBuilder)**: `var frag = self.items.begin_rebuild(ctx)` → for each item: `var ib = items.begin_item(key, ctx)` → `ib.add_dyn_text(...)` → `ib.add_custom_event("click", ACTION_TAG, item_id)` → `ib.add_class_if(condition, "class")` → `items.push_child(ctx, frag, ib.index())`.

**Conditional helpers (Phase 18)**: `class_if(cond, "name")` → `"name"` or `""`. `class_when(cond, "a", "b")` → `"a"` or `"b"`. `text_when(cond, "yes", "no")` → conditional text. `ib.add_class_if(cond, "name")` → one-call shortcut on ItemBuilder/RenderBuilder.

**String signal in render (Phase 19)**: `vb.add_dyn_text_signal(name)` → reads `name.get()` and adds as dynamic text. Works on both `RenderBuilder` and `ItemBuilder`.

**Keyed list dispatch (Phase 17 — via HandlerAction)**: `var action = self.items.get_action(handler_id)` → `if action.found: match action.tag`.

**Keyed list rebuild (Phase 16 — manual)**: `var frag = self.items.begin_rebuild(ctx)` → for each item: `items.create_scope(ctx)` → `items.item_builder(key, ctx)` → register handlers → `items.push_child(ctx, frag, idx)`.

**Keyed list flush (via KeyedList)**: `items.flush(ctx, writer, frag_idx)` + `writer.finalize()`.

**Flush lifecycle**: `if not ctx.consume_dirty(): return 0` → rebuild → `ctx.flush(writer, new_idx)` or `items.flush(ctx, writer, frag_idx)` + `writer.finalize()`.

**Combined flush (M20.5 TodoApp)**: When the app shell has dynamic bindings (bind_value) AND a KeyedList: `ctx.consume_dirty()` → `render()` → `ctx.diff(writer, new_app_idx)` (catches bind_value changes) → `items.flush(ctx, writer, new_frag_idx)` → `writer.finalize()`. The diff emits SetAttribute for changed value bindings; dyn_node(0) stays as placeholder (diff no-ops, KeyedList manages content separately).

## Deferred Abstractions (Blocked on Mojo Roadmap)

- **Closure event handlers** → blocked on Lambda syntax + Closure refinement (Phase 1, 🚧). Would eliminate `ItemBuilder.add_custom_event()` + `get_action()`. Phase 20 string dispatch + inline DSL helpers (`oninput_set_string`, `bind_value`) address this for input events.
- **`rsx!` macro** → blocked on Hygienic importable macros (Phase 2, ⏰). Would enable compile-time DSL like Dioxus.
- **`for` loops in views** → blocked on macros (Phase 2, ⏰). Currently iteration happens in build functions.
- **Generic `Signal[T]`** → blocked on Conditional conformance (Phase 1, 🚧). Currently `SignalI32` / `SignalBool` / `SignalString` / `MemoI32` (Phase 18 added `SignalBool`, Phase 19 added `SignalString`).
- **Dynamic component dispatch** → blocked on Existentials / dynamic traits (Phase 2, ⏰).
- **Pattern matching on actions** → blocked on ADTs & pattern matching (Phase 2, ⏰). Currently `if/elif` chains.
- **Async data loading / suspense** → blocked on First-class async (Phase 2, ⏰).
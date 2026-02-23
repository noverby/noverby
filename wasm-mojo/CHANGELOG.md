# Changelog

All notable changes to wasm-mojo are documented here, organized by development phase.

## Phase 28 — Conditional Rendering

Added `ConditionalSlot` — a reusable state tracker for conditional DOM content in dynamic node slots. Similar to `FragmentSlot` (which manages keyed lists), `ConditionalSlot` manages a single conditional VNode that can be shown, hidden, or swapped. The diff engine already handles all the underlying transitions (placeholder↔VNode, VNode↔VNode); `ConditionalSlot` provides the component-level API to track what's currently in the DOM.

- **P28.1** — ConditionalSlot struct. Added `ConditionalSlot` to `src/component/lifecycle.mojo` — tracks `anchor_id` (placeholder ElementId when empty), `current_vnode` (VNode index of mounted branch, -1 when empty), and `mounted` (whether a branch is in the DOM). Added `flush_conditional()` function handling two transitions: empty→branch (CreateEngine + ReplaceWith anchor) and branch→branch (DiffEngine handles same/different templates). Added `flush_conditional_empty()` function handling branch→empty (CreatePlaceholder + InsertBefore + remove old VNode roots). Both functions return an updated `ConditionalSlot` and do NOT finalize the mutation buffer (allowing batching). Exported `ConditionalSlot`, `flush_conditional`, and `flush_conditional_empty` from `component/__init__.mojo`.

- **P28.2** — ComponentContext helpers. Added `conditional_slot()` convenience constructor returning an uninitialized `ConditionalSlot`. Added `flush_conditional_slot(writer, slot, vnode_idx)` method delegating to `flush_conditional()` with the context's internal pointers. Added `flush_conditional_slot_empty(writer, slot)` method delegating to `flush_conditional_empty()`. Imported `ConditionalSlot` and flush helpers into `context.mojo`. Added `onclick_toggle(signal: SignalBool)` overload to the DSL (`src/vdom/dsl.mojo`) accepting `SignalBool` directly (internally uses the same `ACTION_SIGNAL_TOGGLE` on the underlying Int32 key).

- **P28.3** — Counter app: show/hide detail. Extended `CounterApp` with `show_detail: SignalBool`, `detail_tmpl: UInt32` (registered via `register_extra_template`), and `cond_slot: ConditionalSlot`. Template updated to 5 children: h1, button(up), button(down), button(toggle detail), dyn_node[1] (conditional slot). Detail template "counter-detail": `div > [ p > dyn_text[0], p > dyn_text[1] ]` showing "Count is even/odd" and "Doubled: N". `render()` always emits `add_dyn_placeholder()` for dyn_node[1]. `build_detail()` builds the detail VNode from current count. `counter_app_rebuild()` extracts anchor from dyn_node_ids[1] after mount. `counter_app_flush()` splits into: diff app shell, then `flush_conditional_slot` (show detail) or `flush_conditional_slot_empty` (hide detail) based on `show_detail.get()`, then finalize. Added WASM exports: `counter_toggle_handler`, `counter_show_detail`, `counter_detail_tmpl_id`, `counter_cond_mounted`. Updated `CounterAppHandle` in `runtime/app.ts` with `toggleHandler`, `getShowDetail()`, `isDetailMounted()`, `toggleDetail()`. Updated existing counter DOM tests from 3→5 children. 10 new JS test suites: toggle handler valid, show_detail starts false, toggle on → detail appears (verifies div/p/text), toggle off → detail removed, on→off→on cycle, detail updates on increment (even/odd + doubled), hidden increment → correct content on show, detail preserved across 5 increments, h1/buttons unaffected by toggle, decrement with detail visible. 13 new Mojo tests in `test/test_conditional.mojo`: toggle handler valid, show_detail starts false, toggle on/off state, on→off→on cycle, increment with detail visible, increment hidden then show, decrement with detail, 10 rapid toggle cycles, detail template registered, mixed increment+toggle sequence, destroy with detail mounted, destroy→recreate with conditional.

- **P28.4** — Todo app: empty state. Extended `TodoApp` with `empty_msg_tmpl: UInt32` (template "todo-empty": `p > "No items yet -- add one above!"`) and `empty_msg_slot: ConditionalSlot`. Template updated to 4 children: input, button, ul > dyn_node[0], dyn_node[1] (empty message slot). `render()` emits two `add_dyn_placeholder()` calls (dyn_node[0] for items, dyn_node[1] for message). `todo_app_rebuild()` extracts message anchor from dyn_node_ids[1] and immediately mounts the message (list starts empty). `todo_app_flush()` shows message when `len(data) == 0` and hides it when items are present. Added WASM export `todo_empty_msg_mounted`. Updated existing todo DOM test from 3→4 children. 5 new JS test suites: empty message visible on initial mount, hidden after adding item, returns after removing all items, add→remove→add cycle, message does not affect item rendering. 5 new Mojo tests: empty msg on initial mount, hidden after add, returns after remove all, add→remove→add cycle, destroy with msg mounted.

**Test count after P28.4:** 978 Mojo (31 modules) + 1,547 JS = 2,525 tests.

---

## Phase 27 — RemoveAttribute Mutation

Added a proper `RemoveAttribute` opcode (`0x11`) to the mutation protocol, replacing the previous workaround of setting attributes to empty strings. This is semantically correct for HTML boolean attributes (`disabled`, `checked`, `hidden`, `selected`, `open`, etc.) where presence means "on" and absence means "off" — setting `disabled=""` still means disabled, only removing the attribute truly disables it.

- **P27.1** — Protocol + MutationWriter. Added `OP_REMOVE_ATTRIBUTE = 0x11` opcode to `src/bridge/protocol.mojo` with wire format `| op (u8) | id (u32) | ns (u8) | name_len (u16) | name ([u8]) |` (same as `SetAttribute` but without the value payload). Added `remove_attribute(id, ns, name)` method to `MutationWriter`. Added `write_op_remove_attribute` WASM export for test access. Added `Op.RemoveAttribute = 0x11` to both `runtime/protocol.ts` and `examples/lib/protocol.js`. Added `MutationRemoveAttribute` interface and parser case to `MutationReader.next()` in both TS and JS. Added `removeAttribute()` method to `MutationBuilder` in `runtime/interpreter.ts`. Protocol round-trip tests: 3 new suites in `test-js/protocol.test.ts` (basic RemoveAttribute, with namespace, Set→Remove sequence) plus updated "all opcodes" test (now 16 opcodes). 2 new Mojo protocol tests (`test_remove_attribute`, `test_remove_attribute_with_namespace`) plus updated all-opcodes test. Protocol test count: 39/39.

- **P27.2** — Interpreter. Added `Op.RemoveAttribute` case to `handleMutation()` in TS `Interpreter` class with `opRemoveAttribute()` private method — calls `element.removeAttribute(name)` or `element.removeAttributeNS(ns, name)` for namespaced attributes. Added `Op.RemoveAttribute` case to browser JS `Interpreter.handle()`. 7 new interpreter test suites in `test-js/interpreter.test.ts`: basic RemoveAttribute, boolean attribute (disabled set→remove), full cycle (Set→Remove→Set re-add), non-existent attribute is no-op, interleaved Set/Remove on multiple attrs, MutationBuilder round-trip for RemoveAttribute.

- **P27.3** — DiffEngine integration. Updated `_diff_dynamic_attrs()` in `src/mutations/diff.mojo`: when new attr is `AVAL_NONE` and old was a real attribute value (not event), emit `remove_attribute()` instead of `set_attribute(name, "")`. This means the diff engine now produces semantically correct DOM mutations for attribute removal. Updated existing `test_diff_attr_removed_text_to_none` Mojo test to expect `OP_REMOVE_ATTRIBUTE`. Added 2 new diff tests: `test_diff_attr_none_to_text` (attribute appearing: AVAL_NONE→AVAL_TEXT emits SetAttribute, no RemoveAttribute) and `test_diff_bool_attr_true_to_false_remove` (HTML boolean pattern: AVAL_TEXT("")→AVAL_NONE emits RemoveAttribute, no SetAttribute). Updated JS diff test "Attribute removed (text → none)" to expect `Op.RemoveAttribute`. Mutation test count: 36/36.

- **P27.4** — DSL helpers for boolean attributes. Updated `VNodeBuilder.add_dyn_bool_attr()` in `src/vdom/dsl.mojo`: when `value` is True, stores `AVAL_TEXT("")` (HTML boolean presence convention); when False, stores `AVAL_NONE` (triggers RemoveAttribute during diff). `ItemBuilder.add_dyn_bool_attr()` and `RenderContext.add_dyn_bool_attr()` delegate to VNodeBuilder so they inherit the new behavior automatically. Added `attr_if(condition, value)` and `attr_when(condition, true_value, false_value)` runtime string helpers to DSL, analogous to `class_if` / `class_when` but for arbitrary attributes. Exported from `vdom/__init__.mojo`.

**Test count after P27.4:** 960 Mojo + 1,477 JS = 2,437 tests.

---

## Phase 26 — App Lifecycle (Destroy / Recreate)

Proved the full app destroy→recreate loop works end-to-end across all three apps (counter, todo, bench). Added `destroy()` to both the browser `launch()` AppHandle and the TS test `createApp()` AppHandle, with proper resource cleanup and double-destroy safety.

- **P26.1** — Wired `destroy()` into `launch()` AppHandle (`examples/lib/app.js`). Discovers `{app}_destroy` WASM export alongside `_init`, `_rebuild`, `_flush`. `destroy()` method: frees mutation buffer via `alignedFree(bufPtr)`, calls `{app}_destroy(appPtr)` to free WASM-side state, clears root DOM via `rootEl.replaceChildren()`, nulls out `appPtr`/`bufPtr`/`interp` fields to prevent use-after-destroy. Idempotent — `destroyed` flag guards against double-destroy. Extended TS `createApp().destroy()` (`runtime/app.ts`) to also free the mutation buffer, clear the root element, null out pointer fields, and set a `destroyed` flag. `CounterAppHandle` now properly proxies `destroyed`, `appPtr`, and `bufPtr` via getters/setters to the inner `AppHandle`.

- **P26.2** — Multi-app lifecycle JS tests (`test-js/lifecycle.test.ts`). 56 new assertions across 14 test suites: counter create→click→destroy→verify root empty; counter destroy→recreate→click→verify DOM correct; counter 10 create/destroy cycles with `heapStats()` — heap growth bounded, free list populated; double-destroy is a safe no-op; destroy with dirty (unflushed) state doesn't crash; todo add items→destroy→recreate→clean slate (0 items, version 0); todo 5 create/add/destroy cycles — heap bounded; bench create rows→destroy→recreate→correct row count; bench warmup pattern (create→1k→destroy→create→1k→measure) validates js-framework-benchmark warmup requirement; bench 5 create/destroy cycles — heap bounded; cross-app lifecycle (counter→destroy→todo→destroy→counter on same root); simultaneous counter instances with independent destroy; `AppHandle.destroyed` flag tracking; pointer fields nulled after destroy.

- **P26.3** — Multi-app lifecycle Mojo tests (`test/test_lifecycle.mojo`). 10 new tests: counter create→use→destroy; counter destroy→recreate cycle with state verification; 10 counter create/destroy cycles with heap stats checks (growth < 1 MB, free blocks > 0); counter destroy with dirty state; todo create→add→destroy→create cycle (clean slate); 5 todo cycles with heap bounded; bench create→rows→destroy→create cycle; bench warmup pattern (create→1k→destroy→create→1k, growth < 50 MB); free list integrity across destroys (reuse still works); interleaved counter→todo→counter on same WASM instance. Added `heap_stats()` method to `WasmInstance` (delegates to `SharedState.heap_stats()`). Fixed dict iteration in `SharedState.heap_stats()` for Mojo 26.1 compatibility.

- **P26.4** — Bench warmup pattern validated in both JS and Mojo test suites. Create bench app → create 1k rows → destroy → create bench app → create 1k rows → verify heap stays bounded and row count is correct. This proves the js-framework-benchmark warmup requirement (create→destroy→create→measure) works end-to-end.

**Test count after P26.4:** 956 Mojo + 1,441 JS = 2,397 tests.

---

## Phase 25 — Freeing Allocator

Replaced the bump allocator (which never reclaimed memory) with a size-class free-list allocator across all three runtimes (TypeScript, JavaScript browser, Mojo test harness), enabling safe memory reuse.

- **P25.1** — Size-class map allocator in TypeScript (`runtime/memory.ts`). JS-side `ptrSize` map (pointer → size) and `freeMap` (size → LIFO stack of pointers). `alignedAlloc` pops matching blocks from the free map (O(1)) with bump fallback. `alignedFree` pushes freed blocks onto size-class buckets. `heapStats()` reports free blocks/bytes. `setAllocatorReuse(on)` toggle. `saveAllocator()` / `restoreAllocator()` / `initTestAllocator()` for test isolation. Design choice: JS-side maps instead of WASM-side headers — avoids pre-init allocation issues, alignment overhead, and slow `DataView` reads. 60 new allocator tests in `test-js/allocator.test.ts`.

- **P25.2** — Size-class map allocator in JavaScript (`examples/lib/env.js`). Ported P25.1 to plain JS for the browser examples runtime. `KGEN_CompilerRT_AlignedFree` wired to `alignedFree` (was no-op `() => 1`). `initMemory()` resets free-list state on WASM reload.

- **P25.3** — Size-class map allocator in Mojo (`test/wasm_harness.mojo`). `SharedState.aligned_alloc` / `aligned_free` with `Dict`-based size lookup and free-list push. `_cb_aligned_free` wired to `state[].aligned_free(ptr)` (was no-op).

- **P25.4** — Scratch arena for transient `writeStringStruct` allocations. `scratchAlloc(align, size)` wraps `alignedAlloc` and records the pointer. `scratchFreeAll()` bulk-frees all recorded scratch pointers. `writeStringStruct()` now uses `scratchAlloc` (both `runtime/strings.ts` and `examples/lib/strings.js`). TS runtime: `scratchFreeAll()` called in `EventBridge.handleEvent()` after string dispatch. JS examples: `scratchFreeAll()` called in `launch()` flush helper after mutations are applied. 19 new scratch arena tests.

- **P25.5** — Fixed double-free bug, enabled safe memory reuse by default. Root cause: compiled WASM emits double-free calls (same pointer freed twice) due to Mojo destructor mechanics. The allocator did not remove pointers from `ptrSize` on free, so double-frees stacked duplicate entries in the free list — two allocations could pop the same pointer, corrupting each other's data. Fix: `alignedFree` deletes pointer from `ptrSize` on first free (subsequent frees silently ignored); `alignedAlloc` re-registers reused pointers in `ptrSize`. `mutation_buf_alloc` (Mojo) now zero-initializes buffers with `memset_zero` so reused blocks don't contain stale protocol data. Applied to all three runtimes. Reuse enabled by default. 28 new WASM-integrated reuse tests covering text/attr/fragment/placeholder/template diffs with reuse enabled.

**Test count after P25.5:** 946 Mojo + 1,385 JS = 2,331 tests.

---

## Phase 24 — Bench Zero App-Specific JS

- **P24.4** — Fine-grained status bar with 3 `dyn_text` nodes. Split single `status_text: String` field on `BenchmarkApp` into three separate fields: `op_name: String` (dyn_text[0] — "Ready" or operation name), `timing_text: String` (dyn_text[1] — "" or " — X.Yms"), `row_count_text: String` (dyn_text[2] — "" or " · N rows"). The status bar `div.status` in the WASM template now contains 3 auto-numbered `dyn_text()` nodes (indices 0, 1, 2) instead of 1. The keyed row list placeholder moved from `dyn_node(1)` to `dyn_node(3)` (since dyn_text occupies indices 0–2). `bench_app_rebuild()` updated to extract `dyn_node_id(3)` for the KeyedList anchor. `render()` now calls `add_dyn_text()` three times (op_name, timing_text, row_count_text) before `add_dyn_placeholder()`. Refactored `format_timing(op_name, ms) -> String` into `format_timing_ms(ms) -> String` which returns only the timing portion with leading em-dash separator (e.g. `" — 12.3ms"`). Added `format_row_count(count) -> String` helper that formats a row count with leading middle-dot separator and comma-formatted number (e.g. `" · 1,000 rows"`). Added `_format_number(n) -> String` helper for comma thousands separators (handles up to 999,999). `handle_event()` now sets all three fields independently after each toolbar operation — only changed text nodes receive `SetText` mutations on flush (e.g. update-every-10th changes timing but not row count). Added 3 new WASM exports in `src/main.mojo`: `bench_op_name(app_ptr) -> String`, `bench_timing_text(app_ptr) -> String`, `bench_row_count_text(app_ptr) -> String`. Updated `bench_status_text` to return the concatenation of all three fields for backward compatibility with P24.3 tests. Added `testBenchStatusTextParts` JS test function in `test-js/bench.test.ts` (single app instance; verifies initial state — op_name="Ready", timing_text="", row_count_text=""; then dispatches create-1k and verifies op_name="Create 1,000 rows", timing_text starts with em-dash and ends with "ms", row_count_text=" · 1,000 rows", full status = concatenation; then dispatches clear and verifies op_name="Clear", row_count_text=" · 0 rows"; 10 assertions total). Updated `examples/bench/main.js` header: P24.4 marked DONE. Updated `AGENTS.md`: P24.4 marked complete, struct fields/render/handle_event descriptions updated, Phase 24 summary updated. `bench/main.js` is now structurally identical to `counter/main.js` and `todo/main.js` — only `bufferCapacity` override remains as bench-specific config.

**Test count after P24.4:** 1,002 Mojo + 1,278 JS = 2,280 tests (+10 new status-part assertions).

- **P24.3** — `performance.now()` WASM import for timing. Added `performance_now() -> Float64` WASM import via `external_call["performance_now", Float64]()` in `examples/bench/bench.mojo` — the Mojo compiler emits an unresolved symbol, `wasm-ld --allow-undefined` turns it into a WASM import from the `env` module, and the JS host provides `performance_now: () => performance.now()`. Added `format_timing(op_name, ms) -> String` helper that formats elapsed time to 1 decimal place (e.g. `"Create 1,000 rows — 12.3ms"`). Added `status_text: String` field to `BenchmarkApp`, initialized to `"Ready"`. Each toolbar operation in `handle_event()` is now wrapped with before/after `performance_now()` calls; the elapsed time is formatted and stored in `status_text`. `render()` emits `status_text` as `dyn_text[0]` instead of a hardcoded string — on flush, the diff engine detects the changed text and emits a `SetText` mutation, updating the status bar automatically. Added `performance_now` to `examples/lib/env.js` (browser) and `runtime/env.ts` (Deno/test runtime). Added `_cb_performance_now` callback to `test/wasm_harness.mojo` (func[16]: deterministic mock clock, increments by 1.0 per call, `mock_time` field on `SharedState`). Import count updated from 16 to 17. Added `bench_status_text(app_ptr) -> String` WASM export for test verification. Added `bench_handler_id_at(app_ptr, index) -> i32` WASM export (returns toolbar handler ID by tree-walk index 0–5). Added 2 new JS test functions in `test-js/bench.test.ts`: `testBenchStatusTextInit` (verifies initial "Ready" status text) and `testBenchStatusTextAfterOps` (single app instance; dispatches create-1k, swap, and clear via `bench_handle_event`, reads `bench_status_text` after each — verifies operation name prefix, "ms" suffix, and em-dash separator; 7 assertions total). DOM timing test deferred: the bump allocator (which never frees) is near capacity by the time P24.3 tests run, so a separate `createBenchApp` allocation would OOM; the existing bench DOM tests (`testBenchDomCreate`, etc.) already verify the full flush → SetText → DOM update pipeline. Updated `examples/bench/main.js` header: P24.3 TODO removed. Updated `AGENTS.md`: P24.3 marked complete. No new runtime or infrastructure abstractions needed — uses existing `external_call`, `render_builder()`, and diff pipeline.

**Test count after P24.3:** 1,002 Mojo + 1,268 JS = 2,270 tests (+8 new timing assertions).

- **P24.2** — WASM-rendered toolbar with `onclick_custom` handlers. Restructured `BenchmarkApp` (`examples/bench/bench.mojo`) to render the entire app shell from WASM via `setup_view()`: heading, 6 toolbar buttons with `onclick_custom()` handlers, status `dyn_text()` (dynamic_nodes[0]), and table structure with `dyn_node(1)` (dynamic_nodes[1]) for the keyed row list inside `<tbody>`. Note: `dyn_text` and `dyn_node` share the same `dynamic_nodes` index space — auto-numbered `dyn_text()` gets index 0, so `dyn_node` must use index 1. Root changed from `#tbody` to `#root`. Added 6 handler ID fields (`create1k_handler`, `create10k_handler`, `append_handler`, `update_handler`, `swap_handler`, `clear_handler`) extracted via `view_event_handler_id()`. Extended `handle_event()` to route toolbar button clicks to the corresponding benchmark operations (create 1k/10k, append, update, swap, clear) in addition to existing row click dispatch (select/remove). Added `render()` method using `render_builder()` with auto-populated event handlers. Updated `bench_app_rebuild()` to follow the todo pattern: emit templates → render app shell → CreateEngine → extract `dyn_node[1]` anchor for KeyedList → append to root. Updated `bench_app_flush()` to diff app shell + flush keyed list. Simplified `examples/bench/index.html` from 35-line static toolbar+table to a single `<div id="root">` (styles retained for CSS classes rendered by WASM). Simplified `examples/bench/main.js` from 114 lines with `onBoot` callback to 7-line zero-config `launch()` call (only `bufferCapacity` remains as bench-specific config). Updated `examples/lib/app.js` comments: bench example now shows near-zero-config launch. Updated `test-js/bench.test.ts`: `createDOM()` creates `<div id="root">` instead of `<table><tbody>`, `createBenchApp()` passes root div (WASM renders tbody inside app shell), DOM tests query `app.tbody` (derived from rendered DOM), handler lifecycle tests account for 6 toolbar handlers as base count, DOM mount test verifies 6 toolbar buttons rendered. Updated `AGENTS.md`: P24.2 marked complete, bench architecture updated. No runtime or infrastructure changes needed — uses existing `register_view()`, `onclick_custom()`, `view_event_handler_id()`, and `render_builder()` APIs.

**Test count after P24.2:** 1,002 Mojo + 1,260 JS = 2,262 tests (+3 new DOM mount assertions).

- **P24.1** — `bench_handle_event` with handler_map dispatch. Added `handle_event(handler_id) -> Bool` method to `BenchmarkApp` (`examples/bench/bench.mojo`) that calls `rows_list.get_action(handler_id)` and routes to `select_row` (for `BENCH_ACTION_SELECT`) or `remove_row` (for `BENCH_ACTION_REMOVE`) — same pattern as `TodoApp.handle_event`. Added `bench_handle_event(app_ptr, handler_id, event_type) -> i32` WASM export in `src/main.mojo`. EventBridge now dispatches row clicks directly through the shared `launch()` dispatch path — the 25-line tbody event delegation block in `examples/bench/main.js` is eliminated. Updated test helper in `test-js/bench.test.ts` to wire `bench_handle_event` instead of no-op. Updated `AGENTS.md`: P24.1 marked complete, bench example updated. No runtime or infrastructure changes needed — the KeyedList handler_map was already populated by `add_custom_event()` calls in `build_row_vnode()`.

**Test count after P24.1:** 1,002 Mojo + 1,257 JS = 2,259 tests (no test changes — refactor only).

---

## Phase 23 — Bench Convergence to `launch()`

- **M23.1** — Bench app converged to shared `launch()` abstraction. `examples/bench/main.js` rewritten from 138 lines of direct `boot.js` imports to 114 lines using `launch()` with `onBoot` callback — same boot infrastructure as counter and todo. `{app}_handle_event` made optional in `launch()` (`examples/lib/app.js`): when missing, EventBridge is still created (DOM listeners attached for NewEventListener mutations) but dispatch callback is a no-op. Apps that use custom event delegation (e.g. bench) wire their own handlers via `onBoot` while benefiting from the shared WASM loading, buffer allocation, interpreter creation, and initial mount sequence. Bench `main.js` uses `launch({ app: "bench", root: "#tbody", bufferCapacity: 8 * 1024 * 1024, clearRoot: false, onBoot: ... })` — toolbar button wiring, event delegation, and timing display remain in `onBoot`. Error handling consolidated into `launch()` (no more manual try/catch in bench). All three example apps (counter, todo, bench) now use the shared `launch()` boot sequence. Updated `app.js` header comments with bench usage example. Updated `AGENTS.md`: naming convention shows `handle_event` as optional, browser runtime section updated for Phase 23, bench example main.js pattern added. Updated `CHANGELOG.md` with Phase 23 entry.

**Test count after M23.1:** 1,002 Mojo + 1,257 JS = 2,259 tests (no test changes — refactor only).

---

## Phase 22 — WASM-Driven Enter Key & Todo/Counter Convergence

- **M22.1** — `ACTION_KEY_ENTER_CUSTOM` action type and `onkeydown_enter_custom()` DSL helper. New action tag (value 7) in `src/events/registry.mojo` for handlers that fire only when the dispatched key string equals `"Enter"`. `Runtime.dispatch_event_with_string()` extended to handle `ACTION_KEY_ENTER_CUSTOM` — checks the string payload against `"Enter"`, marks the owning scope dirty on match (same as `ACTION_CUSTOM`), and returns True; non-matching keys return False with no side effects. New `HandlerEntry.key_enter_custom(scope_id)` convenience constructor. New `onkeydown_enter_custom() -> Node` DSL function in `src/vdom/dsl.mojo` creates a `NODE_EVENT` for `"keydown"` with `ACTION_KEY_ENTER_CUSTOM`, `signal_key=0`, `operand=0`. Processed by `register_view()` / `setup_view()` like other inline event handlers — auto-assigns dynamic attribute index and registers handler. Exported from `vdom` and `events` packages. **JS keydown dispatch**: `launch()` EventBridge in `examples/lib/app.js` extended to route `keydown` events through `dispatch_string` when `{app}_dispatch_string` exists — sends `event.key` as the string payload via `writeStringStruct()`. If the WASM handler accepts the key (returns 1), the bridge also calls `handle_event` for app-level routing. If rejected (returns 0), no further dispatch occurs. This two-step dispatch (string filter → app routing) enables WASM-driven keyboard shortcuts with zero app-specific JS. **TodoApp migration**: `examples/todo/todo.mojo` updated to add `onkeydown_enter_custom()` to the input element alongside `bind_value` and `oninput_set_string`. New `enter_handler` field stores the auto-registered handler ID via `ctx.view_event_handler_id(1)` (2nd event in tree-walk order; Add button moved to index 2). `handle_event()` now checks both `add_handler` and `enter_handler` to trigger the same Add logic. **Todo main.js converged**: `examples/todo/main.js` reduced from 34 lines (with `onBoot` Enter key hook) to 15 lines — zero app-specific JS, identical in structure to `counter/main.js`. The `onBoot` callback is completely eliminated. New WASM export `todo_enter_handler_id(app_ptr) -> i32` returns the Enter key handler ID for tests. **Test updates**: Handler count assertions updated from 2 to 3 app-level handlers (oninput + keydown_enter + onclick_custom). Oninput handler offset corrected from `addHandler - 1` to `addHandler - 2`. 3 new Mojo-side DSL test functions: `test_onkeydown_enter_custom_node` (node kind, event name, action tag), `test_onkeydown_enter_custom_in_element` (counts as dynamic attr), `test_onkeydown_enter_custom_with_binding` (Phase 22 TodoApp pattern with bind_value + oninput + keydown_enter + onclick_custom). 3 new WASM-level tests in `test/test_dsl.mojo`. 3 new JS DSL tests in `test-js/dsl.test.ts`. 10 new JS todo tests in `test-js/todo.test.ts`: enter handler ID validation, dispatch_string with Enter key marks scope dirty, non-Enter key is ignored, Enter triggers Add (dispatch_string + handle_event), Enter with empty input is no-op, Enter Add with DOM rendering, Shift key does not trigger Add. Updated `AGENTS.md` with Phase 22 documentation: keydown Enter handler pattern, JS keydown dispatch, TodoApp handler index layout, Handler Action Tags reference table. Updated `CHANGELOG.md` with Phase 22 entry.

**Test count after M22.1:** 1,002 Mojo + 1,257 JS = 2,259 tests.

---

## Phase 21 — App Launcher Abstraction (`launch()`)

- **M21.1** — Convention-based `launch()` function in `examples/lib/app.js`. New high-level app launcher that eliminates per-app boot boilerplate by discovering WASM exports via naming convention. Given `app: "counter"`, auto-discovers `counter_init`, `counter_rebuild`, `counter_flush`, `counter_handle_event` (required), and optionally `counter_dispatch_string` (enables automatic string dispatch for `input`/`change` events — Dioxus-style two-way binding with zero app-specific JS). The launcher handles the full boot sequence: load WASM → init app → clear root element → create interpreter + mutation buffer → wire EventBridge with smart dispatch → initial mount → optional `onBoot(handle)` callback for app-specific post-boot wiring. Returns an `AppHandle` with `{ fns, appPtr, interp, bufPtr, bufferCapacity, rootEl, flush }`. Options: `app` (required — WASM export prefix), `wasm` (required — URL to .wasm file), `root` (CSS selector, default `"#root"`), `bufferCapacity` (default 65536), `clearRoot` (default true), `onBoot` (optional callback). **Counter main.js** reduced from 60 lines to 5 lines — zero app-specific JS, just `launch({ app: "counter", wasm: ... })`. **Todo main.js** reduced from 105 lines to 34 lines — only app-specific code is the Enter key shortcut wired via `onBoot` (disappears when keydown event handling moves into WASM). **Bench main.js** unchanged — uses direct `boot.js` imports because it relies on manual event delegation and direct WASM calls for each operation; will converge to `launch()` as those features move into WASM. Updated `boot.js` to re-export `launch` from `app.js` and updated header comment to describe it as the low-level API for advanced use cases. Updated `AGENTS.md` with new Browser Runtime section documenting `app.js` and all `examples/lib/` modules, example main.js patterns, and WASM export naming convention for `launch()` compatibility. **Convergence target**: all standard wasm-mojo apps should eventually use identical `launch()` calls with no `onBoot` hook — Dioxus-style `dioxus::launch(App)` equivalent for Mojo WASM.

---

## Phase 20 — String Event Dispatch & Input Binding

- **M20.1** — String event dispatch infrastructure. New `ACTION_SIGNAL_SET_STRING` action tag (value 6) in `src/events/registry.mojo` for handlers that write a string value to a `SignalString`. `HandlerEntry.signal_set_string(scope_id, string_key, version_key, event_name)` convenience constructor stores `string_key` in the `signal_key` field and `version_key` in the `operand` field (cast to Int32). `Runtime.dispatch_event_with_string(handler_id, event_type, value: String)` dispatches string payloads — for `ACTION_SIGNAL_SET_STRING` handlers, calls `write_signal_string(string_key, version_key, value)` which updates the StringStore entry and bumps the version signal (marking subscribers dirty); falls back to normal `dispatch_event` for other action types. Forwarding methods added to `AppShell` and `ComponentContext`. New WASM exports: `handler_register_signal_set_string(rt, scope, string_key, version_key, event_name) -> handler_id`, `dispatch_event_with_string(rt, handler_id, event_type, value) -> i32`, `shell_dispatch_event_with_string(shell, handler_id, event_type, value) -> i32`. Also added string signal WASM exports needed for testing: `signal_create_string(rt, initial) -> packed_i64` (low 32 bits = string_key, high 32 bits = version_key), `signal_string_key(packed) -> i32`, `signal_version_key(packed) -> i32`, `signal_peek_string(rt, string_key) -> String`, `signal_write_string(rt, string_key, version_key, value)`, `signal_string_count(rt) -> i32`. 6 new Mojo tests in `test/test_events.mojo`: handler field verification (action=6, signal_key=string_key, operand=version_key), basic dispatch (writes string to signal), empty string dispatch, overwrite with version tracking, scope dirty via subscriber notification, fallback to normal dispatch for non-string actions.

- **M20.2** — JS EventBridge string event dispatch. Extended `EventBridge` (`runtime/events.ts`) to extract `event.target.value` as a string for `input`/`change` events and dispatch via a new `DispatchWithStringFn` callback. The string value is written to WASM linear memory via `writeStringStruct()` and passed as a Mojo String struct pointer. Dispatch priority for input/change events: (1) try string dispatch → if handled, done; (2) fall back to numeric dispatch (`parseInt`) → if handled, done; (3) fall back to default no-payload dispatch. Non-input events (click, keydown, etc.) bypass string dispatch entirely. Added `DispatchWithStringFn` type and `dispatchWithStringFn` field to `EventBridge`; updated `setDispatch()` to accept optional third parameter. Extended `AppConfig` (`runtime/app.ts`) with optional `handleEventWithString` callback; `createApp()` wires it to the EventBridge when provided. Updated `WasmExports` (`runtime/types.ts`) with Phase 20 exports: `handler_register_signal_set_string`, `dispatch_event_with_string`, `shell_dispatch_event_with_string`, `signal_create_string`, `signal_string_key`, `signal_version_key`, `signal_peek_string`, `signal_write_string`, `signal_string_count`. New `test-js/events.test.ts` with 49 tests in two sections: (1) EventBridge unit tests with mock dispatch functions — input calls string dispatch, change calls string dispatch, string dispatch returns 0 falls back to numeric, non-numeric falls to default, click bypasses string path, empty string dispatches via string path, no string fn falls back to numeric, onAfterDispatch fires, multiple sequential inputs; (2) WASM integration tests — string dispatch writes to SignalString, empty string writes correctly, version signal bumps on dispatch, subscriber scope marked dirty, non-string handler falls back correctly, writeStringStruct round-trip for various strings (empty, ASCII, spaces, emoji, CJK, 100-char).

- **M20.3** — `oninput_set_string(signal)` / `onchange_set_string(signal)` DSL helpers for inline event binding. New functions in `src/vdom/dsl.mojo` create `NODE_EVENT` nodes with `ACTION_SIGNAL_SET_STRING` action, storing `string_key` in `dynamic_index` and `Int32(version_key)` in `operand` — exactly matching `HandlerEntry.signal_set_string()` field encoding. `oninput_set_string(signal: SignalString) -> Node` binds to the `"input"` event; `onchange_set_string(signal: SignalString) -> Node` binds to the `"change"` event. Both are processed by `ComponentContext.register_view()` / `setup_view()` which auto-assigns dynamic attribute indices and registers handlers with `ACTION_SIGNAL_SET_STRING`. Exported from `vdom` package. Enables Dioxus-style inline input binding: `el_input(oninput_set_string(name))`. 3 new Mojo-side test functions in `src/vdom/dsl_tests.mojo`: node field verification (kind, event_name, action tag, string_key, version_key), onchange variant, and element integration (counts as dynamic attr). 3 new WASM-level tests in `test/test_dsl.mojo`. 3 new JS tests in `test-js/dsl.test.ts`.

- **M20.4** — Dynamic `value` attribute binding for two-way input control. New `NODE_BIND_VALUE` node kind tag (value 7) in `src/vdom/dsl.mojo` for value binding nodes that carry a SignalString reference (attr_name in `text`, string_key in `dynamic_index`, version_key in `operand`). New DSL functions: `bind_value(signal: SignalString) -> Node` creates a `NODE_BIND_VALUE` with `attr_name="value"`; `bind_attr(attr_name, signal) -> Node` creates one with a custom attribute name. `_process_view_tree()` in `src/component/context.mojo` extended to handle `NODE_BIND_VALUE` — collects `_ValueBindingInfo` and replaces with `NODE_DYN_ATTR`, preserving tree-walk attr index ordering. New `AutoBinding` tagged union (`AUTO_BIND_EVENT` / `AUTO_BIND_VALUE`) in `src/component/context.mojo` replaces the event-only auto-population with a unified list of auto-populated dynamic attributes stored in tree-walk order. `register_view()` interleaves events and value bindings by comparing their assigned `attr_idx` values. `RenderBuilder` extended with a second constructor accepting `List[AutoBinding]` + `UnsafePointer[Runtime]`; `build()` iterates auto-bindings in order — for events: `add_dyn_event()`; for value bindings: reads `peek_signal_string(string_key)` from the Runtime and calls `add_dyn_text_attr(attr_name, value)`. Falls back to legacy `EventBinding` path for backward compatibility. `render_builder()` uses the auto-binding path when bindings are present. `Node.is_bind_value()`, `Node.bind_value_count()` query methods added. `is_attr()` and `dynamic_attr_count()` updated to include `NODE_BIND_VALUE`. `_build_node()`, `count_dynamic_attr_slots()`, and template Pass 2 updated to treat `NODE_BIND_VALUE` as a dynamic attribute. Exported from `vdom` and `component` packages. Enables Dioxus-style two-way binding: `el_input(bind_value(text), oninput_set_string(text))`. 6 new Mojo-side test functions in `src/vdom/dsl_tests.mojo`: bind_value node fields, bind_attr custom name, bind_value in element (counts as dynamic attr), two-way binding element (2 dynamic attrs), bind_value to_template (TATTR_DYNAMIC), two-way to_template (2 TATTR_DYNAMICs). 6 new WASM-level tests in `test/test_dsl.mojo`. 6 new JS tests in `test-js/dsl.test.ts`.

- **M20.5** — TodoApp WASM-driven Add flow. Migrated the TodoApp example (`examples/todo/todo.mojo`) to use fully WASM-driven input handling, eliminating all JS special-casing for the Add button. New `onclick_custom() -> Node` DSL helper in `src/vdom/dsl.mojo` creates a `NODE_EVENT` with `ACTION_CUSTOM` (value 255), `signal_key=0`, `operand=0` — processed by `register_view()` / `setup_view()` like other inline event handlers. Exported from `vdom` package. New `ComponentContext.view_event_handler_id(index: Int) -> UInt32` method in `src/component/context.mojo` returns the handler ID for the Nth event registered by `register_view()`, enabling apps to retrieve auto-registered custom handler IDs for app-specific routing. **TodoApp changes**: (1) `__init__` switched from `register_template()` to `register_view()` with inline bindings — `el_input(attr("type","text"), attr("placeholder","..."), bind_value(input_text), oninput_set_string(input_text))` for two-way input binding and `el_button(text("Add"), onclick_custom())` for the Add button; (2) `input_text = create_signal_string("")` moved before `register_view()` since `bind_value`/`oninput_set_string` reference the signal's keys at Node construction time; (3) `add_handler` extracted via `ctx.view_event_handler_id(1)` (2nd event in tree-walk order: oninput is 1st, onclick is 2nd); (4) `handle_event()` now handles the Add action entirely in WASM — reads `input_text.peek()`, calls `add_item(text)`, clears via `input_text.set("")`, returns True; (5) `build_app_vnode()` renamed to `render()` using `render_builder()` which auto-populates `bind_value` (reads signal → "value" attr), `oninput_set_string` event listener, and `onclick_custom` event listener; (6) `todo_app_flush()` now re-renders the app shell via `ctx.diff()` before flushing items — the diff detects `bind_value` changes (e.g. input cleared after Add) and emits `SetAttribute` mutations, while `dyn_node(0)` stays as placeholder (diff sees placeholder vs placeholder = no-op, KeyedList manages content separately). New WASM export `todo_dispatch_string(app_ptr, handler_id, event_type, value: String) -> i32` dispatches string events to the todo app's runtime. New WASM export `todo_add_handler_id(app_ptr) -> i32` returns the Add button handler ID. **JS changes** (`examples/todo/main.js`): Simplified to uniform event dispatch — `input`/`change` events extract `event.target.value` via `writeStringStruct()` and call `todo_dispatch_string()`; all other events call `todo_handle_event()` directly; Enter key dispatches the Add handler directly (signal already has current text from `oninput_set_string`); no special-casing for any handler ID. **Two-way binding pattern (complete)**: `el_input(attr("type","text"), bind_value(input_text), oninput_set_string(input_text))` + `el_button(text("Add"), onclick_custom())`. Equivalent Dioxus: `input { value: "{text}", oninput: move |e| text.set(e.value()) }` + `button { onclick: move |_| { add(&text); text.set(""); }, "Add" }`. 3 new Mojo-side test functions in `src/vdom/dsl_tests.mojo`: onclick_custom node fields (kind, event_name, action=ACTION_CUSTOM, signal_key=0, operand=0), onclick_custom in button element (counts as dynamic attr), onclick_custom with bind_value+oninput_set_string in sibling elements (TodoApp pattern). 3 new WASM-level tests in `test/test_dsl.mojo`. 3 new JS DSL tests in `test-js/dsl.test.ts`. 6 new JS todo tests in `test-js/todo.test.ts`: string dispatch updates SignalString, handle_event Add reads signal and adds item, Add with empty input is a no-op, WASM-driven Add with DOM rendering, multiple WASM-driven Adds, todo_dispatch_string export works. Handler count tests updated (base count 1→2 for oninput+onclick_custom app-level handlers).

**Test count after M20.5:** 999 Mojo + 1,240 JS = 2,239 tests.

---

## Phase 19 — SignalString (Reactive String Signals) ✅

- **M19.1** — `StringStore` (`src/signals/runtime.mojo`). Safe heap-string storage with slab-style free-list slot reuse. Methods: `create(initial) -> UInt32`, `read(key) -> String`, `write(key, value)`, `destroy(key)`, `count()`, `contains(key)`. Added as `Runtime.strings` field. Solves the problem that the type-erased `SignalStore` (memcpy-based) is unsafe for heap types like String.
- **M19.2** — `SignalString` handle type (`src/signals/handle.mojo`). Ergonomic reactive string signal wrapping a `string_key` (index in StringStore) + `version_key` (companion Int32 signal in SignalStore for subscriber tracking). API: `get() -> String` (peek without subscribing), `peek() -> String` (alias), `read() -> String` (subscribe context via version signal), `set(String)` (write + bump version → marks subscribers dirty), `version() -> UInt32`, `is_empty() -> Bool`, `__str__() -> String`. Exported from signals package.
- **M19.3** — Runtime string signal methods (`src/signals/runtime.mojo`). `create_signal_string(initial) -> (UInt32, UInt32)` creates string + version signal pair. `peek_signal_string(string_key) -> String`, `read_signal_string(string_key, version_key) -> String` (with context subscription), `write_signal_string(string_key, version_key, value)` (write + bump version), `destroy_signal_string(string_key, version_key)`, `string_signal_count() -> Int`. Hook-based `use_signal_string(initial) -> (UInt32, UInt32)` stores both keys in scope hooks (two HOOK_SIGNAL entries).
- **M19.4** — `use_signal_string` / `create_signal_string` on `ComponentContext` (`src/component/context.mojo`). `ctx.use_signal_string(initial: String) -> SignalString` creates a string signal with hook registration and scope subscription. `ctx.create_signal_string(initial: String) -> SignalString` creates without hooks or subscription.
- **M19.5** — `add_dyn_text_signal(SignalString)` convenience on `RenderBuilder` (`src/component/context.mojo`) and `ItemBuilder` (`src/component/keyed_list.mojo`). Reads the signal's current value (via peek) and adds it as the next dynamic text slot — replaces the common `add_dyn_text(signal.get())` pattern.
- **M19.6** — 38 new Mojo tests: 9 `StringStore` unit tests (create/read, write, count, contains, destroy, reuse slot, multiple entries, empty string, overwrite), 16 `SignalString` unit tests (get, peek, set, set empty, read subscribes, read returns value, version increments, is_empty true/false/after set, str, str empty, copy, multiple writes, concatenation pattern), 3 Runtime string signal tests (count, destroy, use_signal_string hook), 10 `ComponentContext` SignalString integration tests (use_signal_string, empty, subscribes scope, create_signal_string, no subscribe, set/get, version lifecycle, str interpolation, render builder, multiple signals, mixed with SignalI32).

- **M19.7** — TodoApp `input_text` migrated from plain `String` to `SignalString` (`examples/todo/todo.mojo`). Uses `ctx.create_signal_string(String(""))` (no scope subscription — the input value is a write-buffer, not rendered reactively). Updated `todo_set_input` export to use `input_text.set(text)` instead of direct assignment. Added `todo_input_version` and `todo_input_is_empty` WASM exports demonstrating `SignalString.version()` and `SignalString.is_empty()`. Added 12 new JS tests: version tracking (initial 0, increments on each set, list_version decoupled, scope not dirty), `is_empty` state transitions (empty on init, not empty after set, empty after clear).

**Test count after M19.7:** 981 Mojo + 1,164 JS = 2,145 tests.

---

## Phase 18 — Conditional Helpers & SignalBool ✅

- **M18.1** — `SignalBool` handle type (`src/signals/handle.mojo`). Ergonomic boolean signal wrapping Int32 (0/1) with proper Bool API: `get() -> Bool`, `read() -> Bool` (with context subscription), `set(Bool)`, `toggle()`, `peek_i32() -> Int32`, `version()`, `__str__()` ("true"/"false"). Exported from signals package.
- **M18.2** — `use_signal_bool` / `create_signal_bool` on `ComponentContext` (`src/component/context.mojo`). `ctx.use_signal_bool(initial: Bool) -> SignalBool` creates a Bool signal with hook registration and scope subscription. `ctx.create_signal_bool(initial: Bool) -> SignalBool` creates without hooks. Stores Bool as Int32 internally.
- **M18.3** — Conditional helper functions (`src/vdom/dsl.mojo`). `class_if(condition, name) -> String` returns the class name or empty string. `class_when(condition, true_class, false_class) -> String` for binary class switching. `text_when(condition, true_text, false_text) -> String` for general conditional text. Exported from vdom package.
- **M18.4** — `add_class_if` / `add_class_when` convenience methods on `ItemBuilder` (`src/component/keyed_list.mojo`) and `RenderBuilder` (`src/component/context.mojo`). `add_class_if(condition, class_name)` replaces the common 4–5 line if/else class pattern with a single call. `add_class_when(condition, true_class, false_class)` for binary class switching.
- **M18.5** — App migrations. TodoApp: `build_item_vnode()` uses `text_when()` for conditional completion indicator (4 lines → 1) and `add_class_if()` for conditional "completed" class (4 lines → 1). BenchmarkApp: `build_row_vnode()` uses `add_class_if()` for conditional "danger" class (5 lines → 1). Header comments updated to reference Phase 18.
- **M18.6** — 27 new Mojo tests: 13 `SignalBool` unit tests (get, set, toggle, round-trip, read subscription, peek_i32, version, str, copy), 8 conditional helper tests (class_if true/false, class_when true/false, text_when true/false, edge cases), 6 `ComponentContext` SignalBool integration tests (use_signal_bool true/false, scope subscription, create_signal_bool true/false, toggle lifecycle).

**Test count after M18.6:** 943 Mojo + 1,152 JS = 2,095 tests.

---

## Phase 17 — ItemBuilder & HandlerAction (Keyed List Ergonomics) ✅

- **M17.1** — `ItemBuilder` + `HandlerAction` on `KeyedList` (`src/component/keyed_list.mojo`). `ItemBuilder` wraps VNodeBuilder + child scope + handler map pointer, providing `add_dyn_text()`, `add_dyn_text_attr()`, `add_dyn_bool_attr()`, `add_dyn_event()`, `add_custom_event()`, and `index()`. `add_custom_event(event, action_tag, data)` performs three operations in one call: registers a custom handler in the Runtime, stores the handler_id → (action_tag, data) mapping, and adds the dynamic event attribute to the VNode. `HandlerAction` struct returned by `KeyedList.get_action(handler_id)` for WASM-side dispatch (`tag`, `data`, `found` fields). `_HandlerMapping` internal storage type. `handler_map: List[_HandlerMapping]` field added to `KeyedList`. `begin_rebuild()` now also clears the handler map. `begin_item(key, ctx) -> ItemBuilder` creates child scope + keyed VNodeBuilder in one call. `get_action(handler_id) -> HandlerAction` for dispatch lookup. `handler_count()` query method. Phase 16 methods (`create_scope`, `item_builder`, `push_child`) remain available for manual pattern. Exported `ItemBuilder` and `HandlerAction` from component package.
- **M17.2** — TodoApp migration. Removed `HandlerItemMapping` struct and `handler_map` field (replaced by `KeyedList.handler_map`). `build_item_vnode()` rewritten: `begin_item()` replaces `create_scope()` + `item_builder()`; `add_custom_event()` replaces `register_handler()` + `add_dyn_event()` + `handler_map.append()` (3 lines → 1 per handler). `handle_event()` rewritten: `get_action()` replaces manual loop over handler_map. Net reduction: ~40 lines removed.
- **M17.3** — BenchmarkApp migration. `build_row_vnode()` rewritten with `begin_item()` + `add_custom_event()`. Removed `HandlerEntry` import (no longer needed). Added `BENCH_ACTION_SELECT` and `BENCH_ACTION_REMOVE` action tags for consistency. Net reduction: ~20 lines removed.
- **M17.4** — WASM exports for testing. `todo_handler_map_count`, `todo_handler_action`, `todo_handler_action_data` for querying the todo KeyedList's handler map. `bench_handler_map_count` for bench. 7 new Mojo tests validating handler map population, clearing on rebuild, and 2×row_count invariant.
- **M17.5** — Documentation. README updated with Phase 17 `ItemBuilder`/`HandlerAction` examples, updated keyed list pattern, test counts. New "Deferred abstractions" section documenting Dioxus features blocked on Mojo roadmap items (closures, macros, generic signals, async, pattern matching, existentials). AGENTS.md and CHANGELOG.md updated.

**Test count after M17.5:** 916 Mojo + 1,152 JS = 2,068 tests.

---

## Phase 16 — Dioxus-style DSL & KeyedList Abstractions ✅

- **M16.1** — Multi-arg `el_*` overloads. 1–5 `Node` argument overloads for all 38 element helpers (`el_div`, `el_span`, `el_button`, etc.), eliminating `List[Node](...)` wrappers. 190 new function overloads using `var` ownership + `^` transfer for zero-copy ergonomics. DSL now mirrors Dioxus `rsx!` nesting: `el_div(el_h1(dyn_text()), el_button(text("Up!"), onclick_add(count, 1)))`.
- **M16.2** — `KeyedList` abstraction (`src/component/keyed_list.mojo`). Bundles `FragmentSlot` + child scope IDs + item template ID into a single struct. Helper methods: `begin_rebuild()` (destroy old scopes, return empty fragment), `create_scope()` (create + track child scope), `item_builder()` (keyed VNodeBuilder), `push_child()`, `flush()` (fragment transitions), `init_slot()`. Exported from component package.
- **M16.3** — App migrations. CounterApp, TodoApp, BenchmarkApp rewritten with multi-arg `el_*` overloads and `KeyedList`. TodoApp: 3 fields (`item_template_id`, `item_slot`, `item_scope_ids`) → 1 (`items: KeyedList`), `items` list renamed to `data` to avoid collision. BenchmarkApp: 3 fields (`row_template_id`, `row_slot`, `row_scope_ids`) → 1 (`rows_list: KeyedList`). WASM exports in `main.mojo` updated for new field paths. All 2,061 tests pass.

**Test count after M16.3:** 909 Mojo + 1,152 JS = 2,061 tests.

---

## Phase 15 — Ergonomic Component API (Dioxus-style Abstractions) ✅

- **M15.1** — Reactive handles & `ComponentContext`. `SignalI32` with operator overloading (`+=`, `-=`, `peek()`, `set()`), `MemoI32`, `EffectHandle` wrappers. `ComponentContext` high-level API bundling AppShell lifecycle, hook creation (`use_signal`, `use_memo`, `use_effect`), template registration, handler registration. Counter app rewritten from ~50 lines to ~15. 60 new Mojo tests. 2,061 tests.
- **M15.2** — Inline event handlers. `NODE_EVENT` DSL node with inline constructors (`onclick_add`, `onclick_sub`, `onclick_set`, `onclick_toggle`, `on_event`). `register_view()` processes event nodes, auto-assigns dynamic attr indices, registers handlers. `RenderBuilder` auto-populates event handler attributes on `build()`. 2,050 tests.
- **M15.3** — Dioxus-style view setup. Auto-numbered `dyn_text()` (no args, sentinel `DYN_TEXT_AUTO`). `setup_view()` combines `end_setup()` + `register_view()`. `flush()` combines diff + finalize. CounterApp init reduced from 35 lines to 3. 5 new tests. 2,055 tests.
- **M15.4** — Todo & bench migration. `register_extra_template()` for multi-template apps. `create_child_scope()`/`destroy_child_scopes()` for keyed lists. Fragment lifecycle helpers (`flush_fragment`, `build_empty_fragment`, `push_fragment_child`). TodoApp init 71 → 3 lines. BenchmarkApp init 44 → 3 lines. 2,055 tests.
- **M15.5** — Documentation. `AGENTS.md` project context for AI agents. README updated with ergonomic API examples, test counts, and Dioxus vs Mojo comparison.
- **M15.6** — PoC cleanup. Inline poc functions into `@export` wrappers, delete `src/poc/`.

**Test count after M15.6:** 909 Mojo + 1,152 JS = 2,061 tests.

---

## Phase 14 — Effects (Reactive Side Effects) ✅

- **M14.1** — `EffectEntry` & `EffectStore` slab allocator in `src/signals/effect.mojo`. Create, destroy, pending/running flags, slot reuse. Unit tests in `test/test_effect.mojo`.
- **M14.2** — Effect runtime API & WASM exports. `Runtime.create_effect`, `effect_begin_run`, `effect_end_run`, `effect_is_pending`, `effect_mark_pending`, `destroy_effect`. Dependency tracking via reactive contexts. Signal write → effect pending propagation (parallel to memo dirty chain). 9 WASM exports. 50 new Mojo + 52 new JS assertions.
- **M14.3** — `use_effect` hook. `HOOK_EFFECT` tag functional. First render creates effect + pushes hook; re-render returns existing ID. WASM export + TS types. 4 new Mojo tests + 3 new JS suites.
- **M14.4** — AppShell effect helpers. 6 convenience methods mirroring signal/memo pattern. 6 shell WASM exports. TS types. 8 new Mojo tests + 6 new JS suites.
- ~~**M14.5**~~ — Superseded by Phase 15 ergonomic API.
- ~~**M14.6**~~ — Superseded by Phase 15 documentation.

**Test count after M14.4:** 838 Mojo + 1,163 JS = 2,001 tests.

---

## Phase 13 — Handler Lifecycle & Derived Signals (Memo) ✅

- **M13.1** — Scope-scoped handler cleanup. Child scopes per item/row in todo and bench apps. `AppShell.destroy_child_scopes()`. Handler leak verified fixed. 11 new JS assertions. 1,655 tests.
- **M13.2–13.3** — Memo store, runtime API & WASM exports. `MemoEntry` + `MemoStore` slab allocator. `Runtime.memos` field. Signal write → memo dirty → scope dirty chain. Dependency re-tracking on recompute. 9 WASM exports. 50 new Mojo + 52 new JS assertions. 1,757 tests.
- **M13.4** — `use_memo_i32` hook. First render creates memo + pushes `HOOK_MEMO` tag; re-render returns existing ID. 33 Mojo + 23 JS assertions. 1,813 tests.
- **M13.5** — AppShell memo helpers. 6 convenience methods + 6 shell WASM exports. 8 new Mojo + 6 new JS suites. 1,845 tests.
- **M13.6** — Counter app memo demo. `doubled_memo` field, second dynamic text span. Full signal write → memo dirty → recompute → DOM update chain. 13 Mojo + 18 JS assertions. 1,868 tests.
- **M13.7** — Documentation update. README updated with memo section, handler lifecycle, architecture diagram.

---

## Phase 12 — TS Runtime Modernization ✅

- **M12.1** — Simplified `createCounterApp`. Manual template DOM construction removed from `runtime/app.ts`. `onNewListener` uses `handlerId` directly. −42 lines.
- **M12.2** — Generic `createApp` helper. `AppConfig`/`AppHandle` interfaces. Common lifecycle (buffer alloc, interpreter, EventBridge, mount, flush) extracted to reusable factory.
- **M12.3** — Todo app modernization. `createTodoApp()` rewritten to use `createApp()`. ~50 lines of manual template DOM removed.
- **M12.4** — Bench app factory & DOM tests. `createBenchApp()` via `createApp()`. 10 new DOM integration suites (31 assertions). 1,644 tests.
- **M12.5** — Documentation & test count update.

---

## Phase 11 — Automatic Template & Event Wiring ✅

- **M11.1** — Template serialization protocol. `OP_REGISTER_TEMPLATE (0x10)` opcode. Full template structure serialized to binary buffer. JS `MutationReader` decodes new opcode. 3 Mojo + 39 JS assertions.
- **M11.2** — JS template deserializer. `TemplateCache.registerFromMutation()` builds DOM from decoded mutations. `buildTemplateNode()` with inline tag-name lookup. 25 new JS assertions.
- **M11.3** — Handler-aware event mutations. `NewEventListener` wire format extended with `handler_id (u32)`. CreateEngine and DiffEngine pass handler IDs through.
- **M11.4** — EventBridge auto-dispatch. `EventBridge` class hooks `interpreter.onNewListener`. Counter JS simplified from manual handler wiring to 5-line constructor. Todo JS reduced ~70 lines.
- **M11.5** — AppShell template emission. `emit_templates()` + `mount_with_templates()`. All three apps emit templates in mount buffer.
- **M11.6** — Example simplification. Counter 65→52, todo 108→91, bench 152→138 lines. All `templateRoots` maps empty — templates come from WASM. 934 JS tests.

---

## Phase 10 — Modularization & Next Steps ✅

- **M10.1** — App modules extracted (`apps/counter.mojo`, `apps/todo.mojo`, `apps/bench.mojo`). `main.mojo` 4,249 → 2,930 lines.
- **M10.2** — PoC exports extracted to `poc/` package. `main.mojo` is now pure `@export` wrappers.
- **M10.3** — Shared JS runtime extracted to `examples/lib/`. Examples deduplicated: counter 81, todo 194, bench 160 lines.
- **M10.4** — `AppShell` struct + lifecycle helpers + height-ordered scheduler. 37 new tests.
- **M10.5** — Ergonomic builder API. `Node` tagged union, 40 tag helpers (`el_div`, `el_h1`, …), `to_template()`, `VNodeBuilder`. 33 Mojo + 69 JS tests.
- **M10.6** — DSL-based app rewrite. Counter, todo, bench converted from manual builder to `el_*`/`to_template`/`VNodeBuilder` DSL.
- **M10.7** — AppShell integration. All apps refactored from manual subsystem management to `AppShell`.
- **M10.8** — Fragment lifecycle helpers. `FragmentSlot` + `flush_fragment()`. Todo/bench reduced by −192 lines total.
- **M10.9** — AppShell flush methods & scheduler integration. `consume_dirty()` routes through Scheduler. −15 lines across apps.
- **M10.10** — Precompiled test binary infrastructure. Per-module `fn main()`, parallel incremental build. Test suite 5–6 min → ~11s.
- **M10.11** — README & documentation update. Test counts 790 → 1,533.
- **M10.12** — Test filter support. `just test signals`, single-module runs ~100ms vs ~10s.
- **M10.13** — Extract DSL test logic. 19 functions moved to `vdom/dsl_tests.mojo`. `main.mojo` −546 lines.
- **M10.14** — Consolidate WASM ABI helpers. 16 type-specific functions → 2 generic (`_as_ptr[T]`, `_to_i64[T]`). −135 lines.
- **M10.15** — Clean unused imports & writer boilerplate. 140 unused symbols removed. −176 lines.
- **M10.16** — `_b2i(Bool)` helper & `_alloc_node`/`_free_node`. 32 patterns replaced. −47 lines.
- **M10.17** — Typed pointer accessors (`_get_*`). 73 call sites updated. −43 lines.
- **M10.18** — Complete `_as_ptr` migration & writer dedup. `_get_writer` added. −3 lines.
- **M10.19** — Generic `_heap_new[T]`/`_heap_del[T]`. 9 inline patterns replaced. −6 lines.
- **M10.20** — Generic `_get[T]` accessor. 12 type-specific helpers → 1 generic. 270+ call sites. −44 lines.
- **M10.21** — Inline single-use pointer bindings. 157 `var` declarations inlined.
- **M10.22** — Documented `@export` submodule limitation. Mojo DCE eliminates submodule exports before LLVM IR. Wrapper pattern is required.

---

## Phase 9 — Performance & Polish ✅

- **M9** — js-framework-benchmark competitive. Memory bounded. Tier 2 compile-time templates deferred (runtime `TemplateBuilder` + DSL sufficient). Developer tools functional.

---

## Phase 8 — Advanced Features ✅

- **M8** — Todo list works. Conditional rendering, keyed lists, context, error boundaries, suspense.

---

## Phase 7 — First App (End-to-End) ✅

- **M7** — Counter app works in browser. Click increment, see number change. 🎉

---

## Phase 6 — Events ✅

- **M6** — Full event flow: click in DOM → JS → WASM → signal write → re-render → mutations → DOM update.

---

## Phase 5 — JS Interpreter ✅

- **M5** — JS interpreter applies mutations to real DOM. Hand-crafted mutation buffers produce correct DOM trees.

---

## Phase 4 — Mutations & Diffing ✅

- **M4** — Diff algorithm produces correct mutations. Full round-trip: Mojo diff → binary buffer → JS decode → verified.

---

## Phase 3 — Templates & VNodes ✅

- **M3** — Templates registered, Tier 1 VNode builder produces correct structures, tag helpers work.

---

## Phase 2 — Scopes & Components ✅

- **M2** — Scopes created, components render VNodes, hooks work (`use_signal` returns stable signal across re-renders).

---

## Phase 1 — Signals & Reactivity ✅

- **M1** — `Signal[Int32]` works end-to-end: create, read, write, subscribe, notify. Tested via WASM exports.

---

## Phase 0 — Foundation Hardening ✅

- **M0** — Arena allocator + collections + ElementId allocator + binary mutation protocol defined. All existing tests pass.
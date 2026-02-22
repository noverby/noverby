# Changelog

All notable changes to wasm-mojo are documented here, organized by development phase.

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
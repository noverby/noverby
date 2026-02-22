# Roadmap — Phases 26–30

Future work for wasm-mojo, building on the completed allocator (Phase 25).

---

## Phase 26 — App Lifecycle (Destroy / Recreate)

Phase 25 solved the allocator side: freed memory is now safely reused. But
nobody has proven the full app destroy→recreate loop works end-to-end. The
`launch()` function in `examples/lib/app.js` does not expose `destroy()` at
all, and the TS-side `createApp()` destroy path (`runtime/app.ts`) has never
been exercised in a create→use→destroy→create cycle.

### Problem

- `launch()` returns an `AppHandle` with `{ fns, appPtr, interp, bufPtr,
  bufferCapacity, rootEl, flush }` — no `destroy`.
- The `{app}_destroy` WASM exports exist for all three apps but are only
  called from the TS `createApp().destroy()` path, which is never used in
  production or tests.
- DOM cleanup is unverified: stale event listeners, orphan nodes, template
  cache entries from old app instances may leak.
- The js-framework-benchmark requires a "warmup" pattern:
  create→destroy→create→measure. Without lifecycle support this is impossible.

### Design

**JS browser runtime** (`examples/lib/app.js`):

- Add `{app}_destroy` to the convention-discovery list in `launch()`.
- Add `destroy()` to the returned `AppHandle`:
  1. Call `mutation_buf_free(bufPtr)` to free the WASM-side buffer.
  2. Call `{app}_destroy(appPtr)` to free WASM-side app state.
  3. Call `eventBridge.uninstall()` to remove DOM listeners.
  4. Clear `rootEl.innerHTML` (or `rootEl.replaceChildren()`) to remove
     rendered DOM.
  5. Null out handle fields to prevent use-after-destroy.

**TS test runtime** (`runtime/app.ts`):

- `createApp().destroy()` already calls `events.uninstall()` +
  `destroyApp(fns, appPtr)`. Extend to also free the mutation buffer and
  provide a `destroyed` flag.

**Allocator reset considerations**:

- The allocator state (`ptrSize`, `freeMap`, heap pointer) is per-WASM-instance,
  not per-app. Destroying one app should not reset the allocator — freed
  blocks simply go back to the free list for the next app to reuse.
- `heapStats()` should show free blocks increasing after destroy and
  heap pointer staying stable (or even decreasing in effective usage)
  after recreate.

### Steps

#### P26.1 — Wire `destroy()` into `launch()` AppHandle

- Discover `{app}_destroy` in `launch()` (alongside `_init`, `_rebuild`,
  `_flush`).
- Add `destroy()` method to the returned handle: free buffer, destroy app,
  uninstall events, clear DOM.
- Guard against double-destroy (idempotent).

#### P26.2 — Multi-app lifecycle JS tests

New test file `test-js/lifecycle.test.ts`:

- Create counter app → click a few times → destroy → verify root is empty.
- Create counter app → destroy → create counter app → click → flush →
  verify DOM is correct and heap pointer is bounded.
- Create→destroy × 10 loop with `heapStats()` assertions: free bytes
  should grow (or stay stable), heap pointer should not grow linearly.
- Create todo app → add items → destroy → create todo app → verify
  clean slate (0 items, fresh handler IDs).
- Destroy with no prior flush (dirty state) — should not crash.
- Double-destroy — should be a safe no-op.

#### P26.3 — Multi-app lifecycle Mojo tests

Extend `test/wasm_harness.mojo` tests:

- `counter_app_init → use → counter_app_destroy → counter_app_init → use`
  cycle with heap stats checks.
- Todo app create→add→destroy→create cycle.
- Verify `aligned_free` calls during destroy don't corrupt the free list.

#### P26.4 — Bench warmup pattern

- Create bench app → create 1k rows → destroy → create bench app →
  create 1k rows → measure timing.
- Verify heap stays bounded across the warmup cycle.
- This validates the js-framework-benchmark warmup requirement.

### Estimated size

| Step | Scope | ~Lines |
|------|-------|--------|
| P26.1 | Wire destroy into launch() | ~40 |
| P26.2 | JS lifecycle tests | ~300 |
| P26.3 | Mojo lifecycle tests | ~100 |
| P26.4 | Bench warmup pattern | ~80 |
| **Total** | | **~520** |

---

## Phase 27 — RemoveAttribute Mutation

The protocol has `SetAttribute` (opcode `0x0A`) but no `RemoveAttribute`.
Currently, "removing" an attribute means setting it to an empty string, which
is semantically wrong for boolean attributes like `disabled`, `checked`,
`hidden`, `selected`, and `open`.

### Problem

- `class_if(false, "active")` returns `""`, which sets `class=""` rather
  than removing the `class` attribute entirely. For CSS selectors like
  `[class]` or boolean attributes, this is incorrect.
- The diff engine has no way to express "this attribute was present before
  and should now be absent."
- Dioxus has `RemoveAttribute` in its mutation protocol — we should match.

### Design

**New opcode**: `OP_REMOVE_ATTRIBUTE = 0x11`

Wire format:

```txt
| op (u8) | id (u32) | ns (u8) | name_len (u16) | name ([u8]) |
```

Same as `SetAttribute` but without the value payload.

**MutationWriter** (`src/bridge/protocol.mojo`):

- Add `remove_attribute(id, ns, name)` method.

**DiffEngine** (`src/mutations/diff.mojo`):

- When diffing dynamic attributes: if old has a value and new has
  `AVAL_NONE` (or a new sentinel `AVAL_REMOVED`), emit `RemoveAttribute`
  instead of `SetAttribute` with empty string.

**Protocol reader** (`runtime/protocol.ts`, `examples/lib/protocol.js`):

- Add `Op.RemoveAttribute = 0x11` opcode.
- Add `MutationRemoveAttribute` interface and parser case.

**Interpreter** (`runtime/interpreter.ts`, `examples/lib/interpreter.js`):

- Handle `RemoveAttribute`: call `element.removeAttribute(name)` (or
  `element.removeAttributeNS(ns, name)` when ns > 0).

**DSL**:

- Add `AVAL_NONE` sentinel to `AttributeValue` for "no value / remove".
- `class_if(false, name)` could return `AVAL_NONE` instead of `""` —
  or introduce `remove_attr(name)` node type.
- `bind_value` should emit `RemoveAttribute` when the signal is empty
  (instead of `value=""`), depending on the attribute semantics.

### Steps

#### P27.1 — Protocol + MutationWriter

- Add `OP_REMOVE_ATTRIBUTE` opcode to `protocol.mojo`.
- Add `remove_attribute(id, ns, name)` to `MutationWriter`.
- Add `Op.RemoveAttribute` to `runtime/protocol.ts` and
  `examples/lib/protocol.js`.
- Add `MutationRemoveAttribute` type and parser case.
- Protocol round-trip tests (write in WASM, read in JS).

#### P27.2 — Interpreter

- Handle `Op.RemoveAttribute` in both interpreters.
- Test: `SetAttribute("class", "foo")` → `RemoveAttribute("class")` →
  verify `element.getAttribute("class")` is `null`.

#### P27.3 — DiffEngine integration

- Add `AVAL_NONE` sentinel to `AttributeValue`.
- DiffEngine: when new attr is `AVAL_NONE` and old was a real value,
  emit `RemoveAttribute`.
- DiffEngine: when old attr is `AVAL_NONE` and new has a value,
  emit `SetAttribute` (attribute appears for the first time).
- Test: template with dynamic bool attr, diff true→false emits
  `RemoveAttribute`.

#### P27.4 — DSL helpers for boolean attributes

- `bool_attr(name)` → `Node` that produces `SetAttribute(name, "")`
  when true and `RemoveAttribute(name)` when false (for `disabled`,
  `checked`, `hidden`, etc.).
- `dyn_bool_attr(name, condition)` → dynamic attribute that diffs
  correctly with `RemoveAttribute`.
- Update `ItemBuilder.add_dyn_bool_attr()` to use `AVAL_NONE` when
  the condition is false.
- Todo app: add a `disabled` attribute to the Add button when
  input is empty (exercises the full path).

### Estimated size

| Step | Scope | ~Lines |
|------|-------|--------|
| P27.1 | Protocol + writer | ~80 |
| P27.2 | Interpreter | ~40 |
| P27.3 | DiffEngine + AVAL_NONE | ~100 |
| P27.4 | DSL bool attr helpers | ~120 |
| **Total** | | **~340** |

---

## Phase 28 — Conditional Rendering

Currently, conditional UI is handled with text-level workarounds:
`text_when(cond, "yes", "no")`, `class_if(cond, "hidden")`. There is no
way to render entirely different VNode subtrees based on a signal value.
The diff engine already handles "different template or different VNode kind
→ full replacement" (strategy 2), so the infrastructure exists — it just
needs a clean API surface.

### Problem

- Showing/hiding a section requires CSS tricks (`class_if(show, "hidden")`),
  which keeps the DOM nodes alive and accessible to screen readers.
- Rendering different content based on state (e.g. loading spinner vs
  data, login form vs dashboard) requires full subtree replacement.
- `FragmentSlot` handles empty↔populated transitions for keyed lists, but
  there is no equivalent for arbitrary conditional branches.

### Design

A **`ConditionalSlot`** that manages an if/else (or multi-branch) VNode
in a dynamic node position.

```txt
# In the component's view:
el_div(
    dyn_text(),           # dynamic_nodes[0] — title
    dyn_node(1),          # dynamic_nodes[1] — conditional slot
)

# In render():
if show_detail.read():
    builder.add_dyn_node(detail_vnode_idx)
else:
    builder.add_dyn_placeholder()
```

The diff engine already handles the transitions:

- **Placeholder → VNode**: `ReplacePlaceholder` + create new node.
- **VNode → Placeholder**: `ReplaceWith` placeholder.
- **VNode A → VNode B** (different templates): full replacement.
- **Same template**: efficient dynamic-only diff.

What's missing is a **component-level helper** to track the current
branch and emit the right VNode on each render.

### Steps

#### P28.1 — ConditionalSlot struct

New struct in `src/component/lifecycle.mojo`:

- `ConditionalSlot` — tracks current branch (tag + VNode index).
- `set_branch(tag, vnode_idx)` — set the current branch.
- `set_empty()` — set to placeholder (no content).
- Integrates with `VNodeBuilder.add_dyn_node()` /
  `add_dyn_placeholder()`.

#### P28.2 — ComponentContext helpers

- `ctx.conditional_slot()` → `ConditionalSlot` (convenience constructor).
- Document the pattern: `if cond: slot.set_branch(1, build_detail())`
  `else: slot.set_empty()`.
- The diff engine handles the rest — no new mutation opcodes needed.

#### P28.3 — Counter app: show/hide detail

Extend the counter app with a conditional section:

- Add a `show_detail: SignalBool` toggle.
- When true, render a `<div>` with additional info (e.g. "Count is
  even/odd", doubled value from memo).
- When false, render a placeholder.
- Button to toggle `show_detail`.
- Tests: toggle on → verify detail DOM exists. Toggle off → verify
  detail DOM removed. Toggle on→off→on → verify correct content.

#### P28.4 — Todo app: empty state

- When the todo list is empty, show a placeholder message ("No items
  yet — add one above!") instead of an empty `<ul>`.
- Uses `ConditionalSlot`: empty list → message VNode, non-empty →
  `<ul>` with keyed items.
- Tests: start with 0 items → message visible. Add item → message
  gone, list visible. Remove all items → message returns.

### Estimated size

| Step | Scope | ~Lines |
|------|-------|--------|
| P28.1 | ConditionalSlot struct | ~60 |
| P28.2 | ComponentContext helpers | ~30 |
| P28.3 | Counter show/hide detail | ~120 |
| P28.4 | Todo empty state | ~80 |
| **Total** | | **~290** |

---

## Phase 29 — Component Composition

All three apps are monolithic: a single `ComponentContext` owns the entire
view tree. Dioxus uses nested component functions that each own their own
reactive scope. wasm-mojo should support rendering child components within
a parent's template, each with independent reactivity.

### Problem

- A parent component cannot embed a child component in its template.
  The `dyn_node()` slot expects a VNode index, but there is no way for
  a child component to produce a VNode that plugs into the parent's
  dynamic node slot.
- Child components need their own `ScopeState` for independent
  dirty tracking — a change in a child signal should only re-render
  that child, not the entire parent.
- Lifecycle: parent destroy must cascade to child scopes.

### Design

**`ChildComponent`** — a handle that wraps a child scope + template +
VNode builder.

```txt
# Parent setup:
var child = ctx.create_child_component()

# Parent render:
builder.add_dyn_node(child.render(ctx))

# Child setup (in a builder callback or init function):
child.use_signal(...)
child.setup_view(child_view, "child-name")

# Parent flush:
# The diff engine diffs the child's VNode like any other dynamic node.
# If only the child's signals changed, only the child's dynamic
# slots produce mutations.
```

**Scope hierarchy**: Child components get their own `ScopeState` via
`ctx.create_child_scope()`. The scheduler processes child scopes at
their correct height (children after parents that depend on them).
Destroying the parent destroys all child scopes automatically.

**Incremental flush**: When only a child's scope is dirty, the parent
diff should skip unchanged dynamic slots (it already does — same
template + same dynamic values = 0 mutations). The child's
dynamic nodes will diff and produce targeted SetText/SetAttribute
mutations.

### Steps

#### P29.1 — ChildComponent struct

New struct in `src/component/`:

- Wraps a child scope ID + template ID + VNode index.
- `render(ctx) -> UInt32` — builds the child VNode via its
  `RenderBuilder`, returns the VNode index for the parent's
  `add_dyn_node()`.
- `is_dirty(ctx) -> Bool` — check if the child's scope is dirty.
- `destroy(ctx)` — destroy child scope + handlers.

#### P29.2 — ComponentContext child component API

- `ctx.create_child_component(view, name) -> ChildComponent`.
- Registers the child's template via `register_extra_template()`.
- Creates a child scope via `create_child_scope()`.
- Processes the child's view tree for inline events.

#### P29.3 — Flush integration

- When flushing, the parent diff walks dynamic nodes. If a dynamic
  node is a child component's VNode, the diff engine handles it
  normally (same template = diff dynamics, different = replace).
- Test: parent with two children, change one child's signal →
  only that child's SetText emitted.

#### P29.4 — Counter with child component

Extract the counter's display into a child component:

- Parent: toolbar buttons + `dyn_node()` slot for the display child.
- Child: `<p>Count: {n}</p>` with its own signals fed from parent.
- Verify: clicking increment only emits SetText for the child's
  dynamic text, not a full parent re-render.

### Estimated size

| Step | Scope | ~Lines |
|------|-------|--------|
| P29.1 | ChildComponent struct | ~120 |
| P29.2 | ComponentContext API | ~80 |
| P29.3 | Flush integration + tests | ~150 |
| P29.4 | Counter with child | ~100 |
| **Total** | | **~450** |

---

## Phase 30 — Client-Side Routing

With app lifecycle (Phase 26) and component composition (Phase 29),
the pieces exist for a single-page app with URL-based view switching.

### Problem

- Each example app (`counter`, `todo`, `bench`) runs as a standalone
  page with its own HTML file and `launch()` call.
- There is no way to switch between views within a single WASM instance.
- Browser back/forward navigation is not handled.

### Design

**`Router`** — a WASM-side struct that maps URL paths to component
constructors (or branch tags).

```txt
Router:
    routes: Dict[String, UInt8]    # path → branch tag
    current: UInt8                 # active branch
    slot: ConditionalSlot          # Phase 28

    fn navigate(path: String):
        current = routes[path]
        slot.set_branch(current, build_for_branch(current))
        mark_dirty()
```

**JS side**:

- `popstate` listener → call `router_navigate(app, path)` WASM export.
- `launch()` option: `routes: { "/": "counter", "/todo": "todo" }`.
- `pushState` / `replaceState` wrappers called from WASM via imports.

**WASM imports needed**:

- `push_state(path_ptr)` — calls `history.pushState(null, "", path)`.
- `replace_state(path_ptr)` — calls `history.replaceState(...)`.

### Steps

#### P30.1 — Router struct + navigate export

- `Router` struct with route table + `ConditionalSlot`.
- `router_navigate(app, path: String)` WASM export.
- `router_current_path(app) -> String` query export.

#### P30.2 — JS history integration

- Add `push_state` and `replace_state` WASM imports to `env.js`
  and `env.ts`.
- Add `popstate` listener in `launch()` that calls
  `router_navigate`.
- `<a>` click interception (prevent default, push state, navigate).

#### P30.3 — Demo: multi-view app

- New example: `examples/app/` — a single WASM app that hosts
  counter + todo behind route switches.
- `/` → counter view, `/todo` → todo view.
- Nav bar with links rendered from WASM (uses `onclick_custom` +
  `push_state` import).
- Browser back/forward works via `popstate` → `router_navigate`.

#### P30.4 — Route transition tests

- Navigate `/` → `/todo` → `/` → verify correct DOM at each step.
- Browser back (simulated `popstate`) → verify previous view restored.
- Direct navigation to `/todo` → verify todo view rendered.
- `heapStats()` bounded across route transitions (Phase 25 allocator
  reclaims destroyed view memory).

### Estimated size

| Step | Scope | ~Lines |
|------|-------|--------|
| P30.1 | Router struct + exports | ~120 |
| P30.2 | JS history integration | ~80 |
| P30.3 | Multi-view demo app | ~200 |
| P30.4 | Route transition tests | ~150 |
| **Total** | | **~550** |

---

## Dependency graph

```txt
Phase 25 (Allocator) ✅
    │
    ▼
Phase 26 (App Lifecycle) ──────────────────────┐
    │                                           │
    ▼                                           │
Phase 27 (RemoveAttribute) ─── independent ─────┤
    │                                           │
    ▼                                           │
Phase 28 (Conditional Rendering) ───────────────┤
    │                                           │
    ▼                                           │
Phase 29 (Component Composition) ───────────────┤
    │                                           │
    ▼                                           │
Phase 30 (Routing) ◄────────────────────────────┘
```

Phase 26 is the natural first step — it validates Phase 25 and unblocks
everything downstream. Phases 27 and 28 are independent of each other
and can be done in either order. Phase 29 builds on 28 (ConditionalSlot
for slot management). Phase 30 depends on 26 (lifecycle), 28
(conditional rendering), and 29 (composition).

## Summary

| Phase | Feature | ~Lines | Depends on |
|-------|---------|--------|------------|
| 26 | App Lifecycle | ~520 | 25 ✅ |
| 27 | RemoveAttribute | ~340 | — |
| 28 | Conditional Rendering | ~290 | — |
| 29 | Component Composition | ~450 | 28 |
| 30 | Client-Side Routing | ~550 | 26, 28, 29 |
| **Total** | | **~2,150** | |
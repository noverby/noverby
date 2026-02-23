# Phase 33 — Suspense

## Problem

Phase 8.5 added low-level suspense infrastructure to the scope
system — `ScopeState` has `is_suspense_boundary`, `is_pending`
fields with setters/getters, `ScopeArena` has
`find_suspense_boundary()`, `has_pending_descendant()`, and
`resolve_pending()` parent-chain walk-up, and there are WASM exports
(`suspense_set_boundary`, `suspense_set_pending`, `suspense_resolve`,
etc.) with unit tests in `phase8.test.ts`. However:

1. **ComponentContext has no suspense API.** The scope plumbing
   exists but is not surfaced on `ComponentContext` or
   `ChildComponentContext`. No component code uses suspense.

2. **No integration with the render/flush cycle.** When a child
   scope is pending, nothing happens in the DOM — there is no
   mechanism to swap between a loading skeleton and actual content
   based on pending state.

3. **No fallback rendering pattern for loading states.** Suspense in
   React/Dioxus shows a fallback (spinner, skeleton) while async
   children resolve. We have `ConditionalSlot` for show/hide and
   the Phase 32 error boundary pattern for content switching, but
   no established pattern for pending-driven switching.

4. **No resolve mechanism from JS.** The async work (fetch, timer,
   IntersectionObserver) happens in JS. There is no demonstrated
   pattern for JS calling back into WASM to mark a scope as
   resolved and trigger a re-flush.

5. **No demonstration app.** Without a working suspense demo, the
   feature is theoretical — never validated end-to-end with DOM
   rendering, event handling, and resolve transitions.

6. **AGENTS.md lists suspense as "blocked on async."** True
   Mojo-native `async`/`await` is blocked, but suspense at the
   WASM boundary only needs synchronous state management — the
   async happens in JS, and WASM manages pending→resolved→flush
   transitions. This is how all WASM frameworks handle it.

### Current state (Phase 32)

Suspense scope fields exist but are dead code at the component level:

```mojo
# scope/scope.mojo — fields exist but component layer ignores them
var is_suspense_boundary: Bool
var is_pending: Bool

# scope/arena.mojo — walk-up exists but ComponentContext doesn't call it
fn find_suspense_boundary(self, scope_id: UInt32) -> Int
fn has_pending_descendant(self, scope_id: UInt32) -> Bool
fn resolve_pending(mut self, scope_id: UInt32) -> Int
fn set_pending(mut self, scope_id: UInt32, pending: Bool)
fn set_suspense_boundary(mut self, scope_id: UInt32, enabled: Bool)
```

### Target pattern (Phase 33)

```mojo
struct DataLoaderApp:
    var ctx: ComponentContext
    var content: ChildComponentContext      # actual content
    var skeleton: ChildComponentContext     # loading fallback
    var data_text: String                  # loaded data (set on resolve)
    var load_handler: UInt32               # button to trigger load

    fn __init__(out self):
        self.ctx = ComponentContext.create()
        self.ctx.use_suspense_boundary()   # mark root as suspense boundary
        self.data_text = String("")
        self.ctx.setup_view(
            el_div(
                el_h1(dsl_text(String("Data Loader"))),
                el_button(dsl_text(String("Load")), onclick_custom()),
                dyn_node(0),   # content or skeleton
                dyn_node(1),
            ),
            String("data-loader"),
        )
        # Content child: displays loaded data
        var content_ctx = self.ctx.create_child_context(
            el_p(dyn_text()), String("content"),
        )
        self.content = content_ctx^
        # Skeleton child: loading placeholder
        var skel_ctx = self.ctx.create_child_context(
            el_p(dsl_text(String("Loading..."))), String("skeleton"),
        )
        self.skeleton = skel_ctx^

    fn flush(mut self, writer: ...) -> Int32:
        if self.ctx.has_pending():
            # Pending: hide content, show skeleton
            self.content.flush_empty(writer)
            var skel_idx = self.render_skeleton()
            self.skeleton.flush(writer, skel_idx)
        else:
            # Resolved: show content, hide skeleton
            self.skeleton.flush_empty(writer)
            var content_idx = self.render_content()
            self.content.flush(writer, content_idx)
        return self.ctx.finalize(writer)

    fn handle_event(mut self, handler_id: UInt32, ...) -> Bool:
        if handler_id == self.load_handler:
            self.ctx.set_pending(True)     # show skeleton
            return True                     # JS will call resolve later
        return self.ctx.dispatch_event(handler_id, event_type)

    fn resolve(mut self, data: String):
        self.data_text = data
        self.ctx.set_pending(False)        # next flush shows content
```

---

## Design

### Suspense lifecycle

The suspense lifecycle mirrors the Phase 32 error boundary pattern
with the same `ConditionalSlot`-based flush alternation:

```text
             ┌──────────┐
             │  Initial  │ ← content shown (no pending children)
             └─────┬─────┘
                   │ set_pending(True)  — triggered by user action
                   ▼
           ┌───────────────┐
           │   Pending      │ ← boundary.has_pending = true
           │   (skeleton)   │   content.flush_empty() + skeleton.flush()
           └───────┬────────┘
                   │ resolve(data)  — triggered by JS callback
                   ▼
             ┌──────────┐
             │ Resolved  │ ← content re-renders with data, skeleton hidden
             └───────────┘
```

### ComponentContext surface

New methods on `ComponentContext`:

| Method | Description |
|--------|-------------|
| `use_suspense_boundary()` | Mark root scope as suspense boundary |
| `set_pending(pending)` | Set pending state on root scope |
| `has_pending() -> Bool` | Check if any descendant is pending |
| `is_pending() -> Bool` | Check if root scope itself is pending |

New methods on `ChildComponentContext`:

| Method | Description |
|--------|-------------|
| `use_suspense_boundary()` | Mark child scope as suspense boundary |
| `set_pending(pending)` | Set pending state on child scope |
| `has_pending() -> Bool` | Check if any descendant of child is pending |
| `is_pending() -> Bool` | Check if child scope itself is pending |

### Pending state mechanics

Unlike error boundaries where `report_error()` walks up the parent
chain, suspense pending state is set directly on a scope. The
boundary checks `has_pending_descendant()` (which does an O(n) scan
of all live scopes) to decide whether to show the fallback.

```text
Root (suspense boundary) ← checks has_pending_descendant()
  └─ Child A (pending=true) ← set_pending(True) here
```

`set_pending(True)` marks the scope and marks the nearest suspense
boundary dirty so the next flush picks up the state change.
`set_pending(False)` clears pending and marks the boundary dirty.

### Flush integration

The suspense boundary owner checks `has_pending()` during flush:

- **No pending children:** flush content children, hide skeleton
- **Pending children:** hide content children, flush skeleton
- **Resolved:** re-flush content (creates from scratch if hidden),
  hide skeleton

Same `flush` / `flush_empty` alternation as error boundaries.

### Dirty tracking

`set_pending(True)` must find the nearest suspense boundary and mark
it dirty so the flush cycle processes the state change. Similarly,
`set_pending(False)` (resolve) marks the boundary dirty.

The `ComponentContext.set_pending()` method handles this:

```mojo
fn set_pending(mut self, pending: Bool):
    self.shell.runtime[0].scopes.set_pending(self.scope_id, pending)
    # Find and dirty the nearest suspense boundary
    var boundary_id = self.shell.runtime[0].scopes.find_suspense_boundary(
        self.scope_id
    )
    if boundary_id != -1:
        self.shell.runtime[0].mark_scope_dirty(UInt32(boundary_id))
    else:
        # Self is the boundary — mark self dirty
        self.shell.runtime[0].mark_scope_dirty(self.scope_id)
```

### JS resolve callback

JS triggers resolution by calling a WASM export. The pattern:

1. User clicks "Load" → `handle_event` calls `set_pending(True)`
2. JS receives handled=true, performs async work (fetch, setTimeout)
3. JS calls `app_resolve(app_ptr, data_string_ptr)` WASM export
4. WASM sets `pending=false`, stores data, marks boundary dirty
5. Next `app_flush()` call shows content with the loaded data

No new JS runtime infrastructure needed. The resolve export is
app-specific (like `sc_handle_event` or `en_handle_event`). The
`launch()` infrastructure doesn't need changes — resolve is called
from test code or app-specific JS.

### JS runtime

No new JS runtime infrastructure is needed. Suspense is entirely
WASM-side — the JS runtime just applies mutations as usual. The
skeleton and content UIs are rendered through the same mutation
protocol. Resolve callbacks are app-specific WASM exports.

---

## Steps

### P33.1 — ComponentContext suspense surface

**Goal:** Surface the existing scope suspense infrastructure on
`ComponentContext` and `ChildComponentContext` with ergonomic methods.

#### Mojo changes

**`src/component/context.mojo`** — Add to `ComponentContext`:

```mojo
# ── Suspense ─────────────────────────────────────────────────────

fn use_suspense_boundary(mut self):
    """Mark the root scope as a suspense boundary.

    Call during setup (before end_setup / setup_view).  When a
    descendant scope is pending, this boundary should show fallback
    UI.  Check ``has_pending()`` during flush to switch between
    content and skeleton.
    """
    self.shell.runtime[0].scopes.set_suspense_boundary(
        self.scope_id, True
    )

fn set_pending(mut self, pending: Bool):
    """Set the pending (loading) state on the root scope.

    When pending is True, the nearest suspense boundary ancestor
    (or self if self is a boundary) should show fallback UI.
    Marks the boundary scope dirty so the next flush picks up
    the change.

    Args:
        pending: True to enter pending state, False to resolve.
    """
    self.shell.runtime[0].scopes.set_pending(self.scope_id, pending)
    var boundary_id = self.shell.runtime[0].scopes.find_suspense_boundary(
        self.scope_id
    )
    if boundary_id != -1:
        self.shell.runtime[0].mark_scope_dirty(UInt32(boundary_id))
    elif self.shell.runtime[0].scopes.is_suspense_boundary(self.scope_id):
        self.shell.runtime[0].mark_scope_dirty(self.scope_id)

fn has_pending(self) -> Bool:
    """Check whether any descendant of this scope is pending.

    Scans all live scopes for pending descendants. Used by
    suspense boundaries to decide whether to show fallback.

    Returns:
        True if any descendant scope is in pending state.
    """
    return self.shell.runtime[0].scopes.has_pending_descendant(
        self.scope_id
    )

fn is_pending(self) -> Bool:
    """Check whether this scope itself is in pending state.

    Returns:
        True if this scope is pending.
    """
    return self.shell.runtime[0].scopes.is_pending(self.scope_id)
```

**`src/component/child_context.mojo`** — Add to `ChildComponentContext`:

```mojo
# ── Suspense ─────────────────────────────────────────────────────

fn use_suspense_boundary(mut self):
    """Mark this child scope as a suspense boundary."""
    self.runtime[0].scopes.set_suspense_boundary(
        self.scope_id, True
    )

fn set_pending(self, pending: Bool):
    """Set the pending (loading) state on this child scope.

    Marks the nearest suspense boundary ancestor dirty.

    Args:
        pending: True to enter pending state, False to resolve.
    """
    self.runtime[0].scopes.set_pending(self.scope_id, pending)
    var boundary_id = self.runtime[0].scopes.find_suspense_boundary(
        self.scope_id
    )
    if boundary_id != -1:
        self.runtime[0].mark_scope_dirty(UInt32(boundary_id))
    elif self.runtime[0].scopes.is_suspense_boundary(self.scope_id):
        self.runtime[0].mark_scope_dirty(self.scope_id)

fn has_pending(self) -> Bool:
    """Check whether any descendant of this child scope is pending."""
    return self.runtime[0].scopes.has_pending_descendant(
        self.scope_id
    )

fn is_pending(self) -> Bool:
    """Check whether this child scope itself is pending."""
    return self.runtime[0].scopes.is_pending(self.scope_id)
```

#### WASM exports (in `src/main.mojo`)

No standalone exports needed — the existing `suspense_*` exports from
Phase 8.5 cover low-level testing. The new surface is tested through
the demo app exports in P33.2/P33.3.

#### Test: `test/test_suspense.mojo`

New test module with ~15 tests:

1. `ctx_use_suspense_boundary_marks_scope` — after
   `use_suspense_boundary()`, scope is a boundary
2. `ctx_is_pending_initially_false` — starts not pending
3. `ctx_set_pending_true_marks_pending` — `is_pending()` returns True
4. `ctx_set_pending_false_clears_pending` — `is_pending()` returns
   False after clearing
5. `ctx_has_pending_initially_false` — no pending descendants
6. `ctx_set_pending_marks_boundary_dirty` — boundary scope is dirty
   after `set_pending(True)`
7. `ctx_clear_pending_marks_boundary_dirty` — dirty after
   `set_pending(False)`
8. `ctx_has_pending_detects_child` — boundary detects pending child
9. `ctx_has_pending_clears_after_resolve` — `has_pending()` false
   after child resolved
10. `ctx_child_set_pending_marks_parent_boundary_dirty` — child
    pending dirtys parent boundary
11. `ctx_multiple_pending_children` — two pending, resolve one, still
    pending; resolve both, not pending
12. `ctx_nested_boundaries_innermost_catches` — inner boundary
    detects inner child pending, outer doesn't (unless outer scans)
13. `ctx_set_pending_no_boundary_still_works` — pending state set
    even without a boundary (no crash)
14. `ctx_pending_cycle` — pending → resolve → pending → resolve
15. `ctx_boundary_is_not_own_pending` — has_pending checks
    descendants, not self

---

### P33.2 — DataLoaderApp demo

**Goal:** A working suspense app where a "Load" button triggers
pending state, a skeleton UI is shown, and a JS-triggered resolve
shows the loaded content.

#### App structure: DataLoader

```text
DataLoaderApp (root scope = suspense boundary)
├── h1 "Data Loader"
├── button "Load"  (onclick_custom → set_pending)
├── dyn_node[0]   ← content OR skeleton
└── dyn_node[1]   ← the other slot

Content child (DLContentChild):
    p > dyn_text("Data: ...")

Skeleton child (DLSkeletonChild):
    p > dyn_text("Loading...")
```

**Lifecycle:**

1. **Init:** Parent creates suspense boundary, two child contexts
   (content + skeleton). Content is shown initially (no pending),
   skeleton is hidden. Content shows "Data: (none)".
2. **Load:** Load button dispatched → parent calls
   `set_pending(True)` → parent scope marked dirty.
3. **Flush (pending):** `has_pending()` returns True → content hidden
   (`flush_empty`), skeleton shown.
4. **Resolve:** JS calls `dl_resolve(app_ptr, data_string_ptr)` →
   WASM stores data, calls `set_pending(False)` → scope marked dirty.
5. **Flush (resolved):** `has_pending()` returns False → skeleton
   hidden, content re-renders with "Data: {loaded_text}".
6. **Re-load:** Another Load → back to skeleton → another resolve →
   content with new data.

#### Mojo implementation (`src/main.mojo`)

```mojo
# ══════════════════════════════════════════════════════════════════════════════
# Phase 33.2 — DataLoaderApp (suspense demo)
# ══════════════════════════════════════════════════════════════════════════════


struct DLContentChild(Movable):
    """Content child: displays loaded data.

    Template: p > dyn_text("Data: ...")
    """
    var child_ctx: ChildComponentContext

    fn __init__(out self, var child_ctx: ChildComponentContext):
        self.child_ctx = child_ctx^

    fn __moveinit__(out self, deinit other: Self):
        self.child_ctx = other.child_ctx^

    fn render(mut self, data: String) -> UInt32:
        var vb = self.child_ctx.render_builder()
        vb.add_dyn_text(String("Data: ") + data)
        return vb.build()


struct DLSkeletonChild(Movable):
    """Skeleton child: loading placeholder.

    Template: p > dyn_text("Loading...")
    """
    var child_ctx: ChildComponentContext

    fn __init__(out self, var child_ctx: ChildComponentContext):
        self.child_ctx = child_ctx^

    fn __moveinit__(out self, deinit other: Self):
        self.child_ctx = other.child_ctx^

    fn render(mut self) -> UInt32:
        var vb = self.child_ctx.render_builder()
        vb.add_dyn_text(String("Loading..."))
        return vb.build()


struct DataLoaderApp(Movable):
    """Suspense demo app with load/resolve lifecycle.

    Parent: div > h1("Data Loader") + button("Load") + dyn_node[0] + dyn_node[1]
    Content: p > dyn_text("Data: ...")
    Skeleton: p > dyn_text("Loading...")
    """
    var ctx: ComponentContext
    var content: DLContentChild
    var skeleton: DLSkeletonChild
    var data_text: String
    var load_handler: UInt32

    fn __init__(out self):
        self.ctx = ComponentContext.create()
        self.ctx.use_suspense_boundary()
        self.data_text = String("(none)")
        self.ctx.setup_view(
            el_div(
                el_h1(dsl_text(String("Data Loader"))),
                el_button(dsl_text(String("Load")), onclick_custom()),
                dyn_node(0),
                dyn_node(1),
            ),
            String("data-loader"),
        )
        self.load_handler = self.ctx.view_event_handler_id(0)
        # Content child
        var content_ctx = self.ctx.create_child_context(
            el_p(dyn_text()), String("dl-content"),
        )
        self.content = DLContentChild(content_ctx^)
        # Skeleton child
        var skel_ctx = self.ctx.create_child_context(
            el_p(dyn_text()), String("dl-skeleton"),
        )
        self.skeleton = DLSkeletonChild(skel_ctx^)

    fn __moveinit__(out self, deinit other: Self):
        self.ctx = other.ctx^
        self.content = other.content^
        self.skeleton = other.skeleton^
        self.data_text = other.data_text^
        self.load_handler = other.load_handler
```

**Lifecycle functions:**

- `_dl_init() -> UnsafePointer[DataLoaderApp]` — allocate + create
- `_dl_destroy(app_ptr)` — destroy children, context, free
- `_dl_rebuild(app, writer) -> Int32` — mount parent, extract anchors,
  init both child slots, flush content child (initial state), finalize
- `_dl_handle_event(app, handler_id, event_type) -> Bool` — route
  load handler → `ctx.set_pending(True)`
- `_dl_resolve(app, data_string)` — store data, call
  `ctx.set_pending(False)`
- `_dl_flush(app, writer) -> Int32` — check `ctx.has_pending()`:
  - If pending: `content.flush_empty()` + `skeleton.flush()`
  - If not pending: `skeleton.flush_empty()` + `content.flush(data)`

**WASM exports (~18):**

```mojo
@export fn dl_init() -> Int64
@export fn dl_destroy(app_ptr: Int64)
@export fn dl_rebuild(app_ptr: Int64, buf_ptr: Int64, cap: Int32) -> Int32
@export fn dl_handle_event(app_ptr: Int64, hid: Int32, evt: Int32) -> Int32
@export fn dl_flush(app_ptr: Int64, buf_ptr: Int64, cap: Int32) -> Int32
@export fn dl_resolve(app_ptr: Int64, data_ptr: Int64)
@export fn dl_is_pending(app_ptr: Int64) -> Int32
@export fn dl_data_text(app_ptr: Int64) -> String
@export fn dl_load_handler(app_ptr: Int64) -> Int32
@export fn dl_content_mounted(app_ptr: Int64) -> Int32
@export fn dl_skeleton_mounted(app_ptr: Int64) -> Int32
@export fn dl_has_dirty(app_ptr: Int64) -> Int32
@export fn dl_scope_count(app_ptr: Int64) -> Int32
@export fn dl_parent_scope_id(app_ptr: Int64) -> Int32
@export fn dl_content_scope_id(app_ptr: Int64) -> Int32
@export fn dl_skeleton_scope_id(app_ptr: Int64) -> Int32
```

#### TypeScript handle

**`runtime/app.ts`** — Add `DataLoaderAppHandle` and
`createDataLoaderApp()`:

```typescript
interface DataLoaderAppHandle extends AppHandle {
  isPending(): boolean;
  getDataText(): string;
  isContentMounted(): boolean;
  isSkeletonMounted(): boolean;
  hasDirty(): boolean;
  scopeCount(): number;
  load(): void;
  resolve(data: string): void;
}
```

#### Test: `test/test_data_loader.mojo` (~20 tests)

1. `dl_init_creates_app` — pointer is valid
2. `dl_not_pending_initially` — `is_pending` is false
3. `dl_data_text_initially_none` — shows "(none)"
4. `dl_content_mounted_after_rebuild` — content child is in DOM
5. `dl_skeleton_not_mounted_initially` — skeleton hidden
6. `dl_load_sets_pending` — `is_pending` true after load
7. `dl_flush_after_load_hides_content` — content unmounted
8. `dl_flush_after_load_shows_skeleton` — skeleton mounted
9. `dl_resolve_clears_pending` — `is_pending` false after resolve
10. `dl_resolve_stores_data` — data_text matches resolved string
11. `dl_flush_after_resolve_shows_content` — content remounted
12. `dl_flush_after_resolve_hides_skeleton` — skeleton unmounted
13. `dl_content_shows_resolved_data` — text is "Data: {resolved}"
14. `dl_reload_cycle` — load → resolve → load → resolve works
15. `dl_multiple_load_resolve_cycles` — 5 cycles
16. `dl_resolve_with_different_data` — each resolve shows new data
17. `dl_flush_returns_0_when_clean` — no mutations when clean
18. `dl_destroy_does_not_crash` — clean shutdown
19. `dl_destroy_while_pending` — destroy during pending state
20. `dl_scope_ids_distinct` — all scope IDs different

#### Test: `test-js/data_loader.test.ts` (~22 suites)

1. `dl_init state validation` — not pending, data "(none)", handlers
   valid
2. `dl_rebuild produces mutations` — RegisterTemplate, LoadTemplate,
   AppendChildren, SetText "Data: (none)"
3. `dl_DOM structure initial` — h1 + button + p("Data: (none)")
4. `dl_load sets pending` — isPending true
5. `dl_flush after load shows skeleton` — DOM shows "Loading..."
6. `dl_content hidden after load` — content child unmounted
7. `dl_skeleton visible after load` — skeleton child mounted
8. `dl_resolve clears pending` — isPending false
9. `dl_flush after resolve shows content` — DOM shows "Data: Hello"
10. `dl_skeleton hidden after resolve` — skeleton unmounted
11. `dl_content visible after resolve` — content mounted
12. `dl_DOM structure after resolve` — h1 + button + p("Data: Hello")
13. `dl_reload cycle` — load → resolve → load → resolve
14. `dl_resolve with different data` — "First" then "Second"
15. `dl_5 load/resolve cycles` — DOM correct each time
16. `dl_flush returns 0 when clean` — no mutations
17. `dl_destroy does not crash` — clean shutdown
18. `dl_double destroy safe` — no crash
19. `dl_destroy while pending` — no crash
20. `dl_multiple independent instances` — isolated
21. `dl_rapid load/resolve cycles` — 10 cycles
22. `dl_heapStats bounded across load/resolve` — memory stable

Register in `test-js/run.ts`.

---

### P33.3 — SuspenseNestApp demo (nested suspense boundaries)

**Goal:** Demonstrate nested suspense boundaries where inner and
outer boundaries independently show/hide skeletons based on their
descendants' pending states.

#### App structure: SuspenseNest

```text
SuspenseNestApp (outer boundary)
├── h1 "Nested Suspense"
├── button "Outer Load"  (sets outer child pending)
├── dyn_node[0]  ← outer content / outer skeleton
│
├── OuterContentChild (inner boundary)
│   ├── p > dyn_text("Outer: ready")
│   ├── button "Inner Load"  (sets inner child pending)
│   └── dyn_node[0]  ← inner content / inner skeleton
│   │
│   ├── InnerContentChild
│   │   └── p > dyn_text("Inner: {data}")
│   │
│   └── InnerSkeletonChild
│       └── p > dyn_text("Inner loading...")
│
├── OuterSkeletonChild
│   └── p > dyn_text("Outer loading...")
```

**Key scenarios:**

1. **Inner load:** Inner child goes pending → inner boundary shows
   inner skeleton, outer content unaffected.
2. **Inner resolve:** Inner child resolved → inner content shown.
3. **Outer load:** Outer child goes pending → outer boundary shows
   outer skeleton (hides entire inner boundary + children).
4. **Outer resolve:** Outer child resolved → inner boundary visible
   again (may still be pending from inner load).
5. **Both pending:** Inner load then outer load → outer skeleton
   shown. Outer resolve → inner skeleton visible (inner still
   pending). Inner resolve → fully resolved.

#### Mojo implementation (`src/main.mojo`)

Structs:

- `SNInnerContentChild` — displays "Inner: {data}"
- `SNInnerSkeletonChild` — displays "Inner loading..."
- `SNOuterContentChild` — inner boundary managing InnerContent +
  InnerSkeleton, with "Inner Load" button
- `SNOuterSkeletonChild` — displays "Outer loading..."
- `SuspenseNestApp` — outer boundary managing OuterContent +
  OuterSkeleton, with "Outer Load" button

Lifecycle functions: `_sn_init`, `_sn_destroy`, `_sn_rebuild`,
`_sn_handle_event`, `_sn_flush`, `_sn_outer_resolve`,
`_sn_inner_resolve`.

**WASM exports (~25):**

```mojo
@export fn sn_init() -> Int64
@export fn sn_destroy(app_ptr: Int64)
@export fn sn_rebuild(app_ptr: Int64, buf_ptr: Int64, cap: Int32) -> Int32
@export fn sn_handle_event(app_ptr: Int64, hid: Int32, evt: Int32) -> Int32
@export fn sn_flush(app_ptr: Int64, buf_ptr: Int64, cap: Int32) -> Int32
@export fn sn_outer_resolve(app_ptr: Int64, data_ptr: Int64)
@export fn sn_inner_resolve(app_ptr: Int64, data_ptr: Int64)
@export fn sn_is_outer_pending(app_ptr: Int64) -> Int32
@export fn sn_is_inner_pending(app_ptr: Int64) -> Int32
@export fn sn_outer_data(app_ptr: Int64) -> String
@export fn sn_inner_data(app_ptr: Int64) -> String
@export fn sn_outer_load_handler(app_ptr: Int64) -> Int32
@export fn sn_inner_load_handler(app_ptr: Int64) -> Int32
@export fn sn_outer_content_mounted(app_ptr: Int64) -> Int32
@export fn sn_outer_skeleton_mounted(app_ptr: Int64) -> Int32
@export fn sn_inner_content_mounted(app_ptr: Int64) -> Int32
@export fn sn_inner_skeleton_mounted(app_ptr: Int64) -> Int32
@export fn sn_has_dirty(app_ptr: Int64) -> Int32
@export fn sn_scope_count(app_ptr: Int64) -> Int32
@export fn sn_outer_scope_id(app_ptr: Int64) -> Int32
@export fn sn_inner_boundary_scope_id(app_ptr: Int64) -> Int32
@export fn sn_inner_content_scope_id(app_ptr: Int64) -> Int32
@export fn sn_inner_skeleton_scope_id(app_ptr: Int64) -> Int32
@export fn sn_outer_skeleton_scope_id(app_ptr: Int64) -> Int32
```

#### TypeScript handle

**`runtime/app.ts`** — Add `SuspenseNestAppHandle` and
`createSuspenseNestApp()`:

```typescript
interface SuspenseNestAppHandle extends AppHandle {
  isOuterPending(): boolean;
  isInnerPending(): boolean;
  getOuterData(): string;
  getInnerData(): string;
  outerContentMounted(): boolean;
  outerSkeletonMounted(): boolean;
  innerContentMounted(): boolean;
  innerSkeletonMounted(): boolean;
  hasDirty(): boolean;
  scopeCount(): number;
  outerLoad(): void;
  innerLoad(): void;
  outerResolve(data: string): void;
  innerResolve(data: string): void;
}
```

#### Test: `test/test_suspense_nest.mojo` (~22 tests)

1. `sn_init_creates_app` — pointer valid
2. `sn_no_pending_initially` — both not pending
3. `sn_all_content_mounted_after_rebuild` — outer + inner content
   visible
4. `sn_no_skeletons_initially` — both skeletons hidden
5. `sn_inner_load_sets_inner_pending` — inner pending true
6. `sn_inner_load_preserves_outer` — outer not pending
7. `sn_flush_after_inner_load` — inner skeleton shown, inner content
   hidden, outer content still mounted
8. `sn_inner_resolve_clears_inner_pending` — inner clean
9. `sn_flush_after_inner_resolve` — inner content restored with data
10. `sn_outer_load_sets_outer_pending` — outer pending true
11. `sn_flush_after_outer_load` — outer skeleton shown, outer content
    hidden (inner boundary + children also hidden)
12. `sn_outer_resolve_restores_outer_content` — outer content + inner
    boundary visible again
13. `sn_inner_load_then_outer_load` — outer skeleton takes visual
    precedence
14. `sn_outer_resolve_reveals_inner_pending` — after outer resolve,
    inner still pending (inner skeleton shown)
15. `sn_inner_resolve_after_outer_resolve` — full resolution
16. `sn_multiple_inner_load_resolve_cycles` — 5 inner cycles
17. `sn_multiple_outer_load_resolve_cycles` — 5 outer cycles
18. `sn_mixed_load_resolve_sequence` — inner→outer→outer_resolve→
    inner_resolve
19. `sn_resolve_with_different_data` — each resolve shows new data
20. `sn_destroy_does_not_crash` — clean shutdown
21. `sn_destroy_while_pending` — destroy during pending
22. `sn_scope_ids_all_distinct` — no overlap

#### Test: `test-js/suspense_nest.test.ts` (~25 suites)

1. `sn_init state validation` — no pending, handlers valid, distinct
2. `sn_rebuild produces mutations` — templates, mount, initial text
3. `sn_DOM structure initial` — h1 + button + outer p + inner button
   + inner p
4. `sn_inner load — DOM shows inner skeleton` — "Inner loading..."
5. `sn_inner load — outer content unaffected` — outer p still shows
6. `sn_inner resolve — DOM shows inner data` — "Inner: {data}"
7. `sn_outer load — DOM shows outer skeleton` — "Outer loading..."
8. `sn_outer resolve — DOM restored with inner` — all content back
9. `sn_inner then outer load` — outer skeleton shown
10. `sn_outer resolve reveals inner skeleton` — inner skeleton
    visible after outer resolve
11. `sn_inner resolve after outer resolve — full recovery` — all
    content
12. `sn_data text correct` — inner vs outer data strings
13. `sn_scope IDs all distinct` — no overlap
14. `sn_handler IDs all distinct` — 2 unique handlers
15. `sn_flush returns 0 when clean` — no mutations
16. `sn_inner load flush produces minimal mutations` — only inner
    slot changes
17. `sn_outer load flush produces minimal mutations` — only outer
    slot changes
18. `sn_5 inner load/resolve cycles` — DOM correct each time
19. `sn_5 outer load/resolve cycles` — DOM correct each time
20. `sn_destroy does not crash` — clean shutdown
21. `sn_double destroy safe` — no crash
22. `sn_multiple independent instances` — isolated
23. `sn_rapid alternating loads` — 10 inner/outer alternations
24. `sn_heapStats bounded across load cycles` — memory stable
25. `sn_destroy with active pending` — no crash

Register in `test-js/run.ts`.

---

### P33.4 — Documentation & AGENTS.md update

**Goal:** Update project documentation to reflect the new suspense
APIs and patterns.

#### Changes

**`AGENTS.md`** — Update Component Layer section:

- Add `use_suspense_boundary()`, `set_pending()`, `has_pending()`,
  `is_pending()` to ComponentContext API list
- Add same methods to ChildComponentContext API list
- Add "Suspense Pattern" to Common Patterns section:

  ```text
  **Suspense flush pattern:** Check `ctx.has_pending()` in flush
  to switch between content and skeleton children:
      if ctx.has_pending():
          content_child.flush_empty(writer)
          skeleton_child.flush(writer, skeleton_vnode)
      else:
          skeleton_child.flush_empty(writer)
          content_child.flush(writer, content_vnode)

  JS triggers resolution via a WASM export that calls
  ctx.set_pending(False) and stores the loaded data.
  ```

- Add DataLoaderApp and SuspenseNestApp to App Architectures section
- Update File Size Reference with new file sizes
- Update Deferred Abstractions to note that suspense (simulated) is
  now implemented

**`CHANGELOG.md`** — Add Phase 33 entry at the top.

**`README.md`** — Update:

- Features list: add "Suspense — pending state with skeleton fallback
  and JS-triggered resolve"
- Test count in Features section
- Test results section: add Suspense test descriptions
- Ergonomic API section: add suspense code example

---

## Dependency graph

```text
P33.1 (ComponentContext suspense surface)
    │
    ├──────────────────────┐
    ▼                      ▼
P33.2 (DataLoader)    P33.3 (SuspenseNest)
    │                      │
    └──────────┬───────────┘
               ▼
        P33.4 (Documentation)
```

P33.1 is the foundation — it surfaces the existing scope infrastructure
on ComponentContext/ChildComponentContext. P33.2 and P33.3 are
independent demos that validate the APIs from P33.1. P33.4 updates
documentation after the demos are validated.

---

## Estimated size

| Step | Description | ~New Lines | Tests |
|------|-------------|-----------|-------|
| P33.1 | Context suspense surface | ~80 Mojo | 15 Mojo |
| P33.2 | DataLoaderApp demo | ~350 Mojo, ~120 TS | 20 Mojo + 22 JS |
| P33.3 | SuspenseNestApp demo | ~450 Mojo, ~140 TS | 22 Mojo + 25 JS |
| P33.4 | Documentation update | ~0 Mojo, ~50 prose | 0 |
| **Total** | | **~880 Mojo, ~310 TS** | **57 Mojo + 47 JS = 104 tests** |

---

## Phase 34 — Effects in Apps

### P34 Problem

Phase 14 added reactive effects to the signal system — `EffectStore`
has `EffectEntry` with context signals for auto-tracking,
`EffectHandle` provides `is_pending()` / `begin_run()` / `end_run()`
lifecycle, and there are WASM exports (`effect_create`,
`effect_begin_run`, `effect_end_run`, `effect_is_pending`, etc.) with
32 Mojo tests and 20 JS test suites. `ComponentContext` has
`use_effect()` and `create_effect()`. However:

1. **No app uses effects.** The effect infrastructure is tested at the
   unit level (direct runtime/store calls) but never exercised in a
   real component lifecycle with signals, rendering, and DOM output.

2. **No demonstrated pattern for effects in flush.** Effects run
   *after* signal writes (event handling) and *before* the next flush.
   But there is no established pattern showing how to drain pending
   effects, re-run them, and then flush the resulting state changes.

3. **No cascading effect demo.** An effect that reads signal A and
   writes signal B should trigger a re-render when B's subscribers
   are dirty. This cascading pattern is fundamental but untested at
   the app level.

4. **No effect + memo chain demo.** An effect reading a memo output
   should re-run when the memo's input changes. This chain
   (signal → memo → effect → signal → render) is untested.

5. **EffectHandle API is manual.** The `begin_run()` / `end_run()`
   bracket is error-prone. The flush pattern should document the
   standard drain-and-run loop.

### Current state

Effects infrastructure exists but is dead code at the app level:

```mojo
# ComponentContext — hooks exist
fn use_effect(mut self) -> EffectHandle     # during setup
fn create_effect(mut self) -> EffectHandle  # any time

# EffectHandle — lifecycle management
fn is_pending(self) -> Bool
fn begin_run(self)
fn end_run(self)

# Runtime — drain pending
fn pending_effect_count(self) -> Int
fn pending_effect_at(self, index: Int) -> UInt32
```

### Target pattern (Phase 34)

```mojo
struct EffectDemoApp:
    var ctx: ComponentContext
    var count: SignalI32
    var doubled: SignalI32          # written by effect
    var parity: SignalString        # written by effect
    var count_effect: EffectHandle  # reacts to count, writes doubled + parity

    fn __init__(out self):
        self.ctx = ComponentContext.create()
        self.count = self.ctx.use_signal(0)
        self.doubled = self.ctx.use_signal(0)
        self.parity = self.ctx.use_signal_string(String("even"))
        self.count_effect = self.ctx.use_effect()
        self.ctx.setup_view(
            el_div(
                el_h1(dsl_text(String("Effect Demo"))),
                el_button(dsl_text(String("+ 1")), onclick_add(self.count, 1)),
                el_p(dyn_text()),   # "Count: N"
                el_p(dyn_text()),   # "Doubled: N"
                el_p(dyn_text()),   # "Parity: even/odd"
            ),
            String("effect-demo"),
        )

    fn run_effects(mut self):
        """Drain and execute pending effects."""
        if self.count_effect.is_pending():
            self.count_effect.begin_run()
            var c = self.count.read()   # re-subscribe
            self.doubled.set(c * 2)
            if c % 2 == 0:
                self.parity.set(String("even"))
            else:
                self.parity.set(String("odd"))
            self.count_effect.end_run()

    fn render(mut self) -> UInt32:
        var vb = self.ctx.render_builder()
        vb.add_dyn_text(String("Count: ") + String(self.count.peek()))
        vb.add_dyn_text(String("Doubled: ") + String(self.doubled.peek()))
        vb.add_dyn_text(String("Parity: ") + String(self.parity.peek()))
        return vb.build()

    fn flush(mut self, writer: ...) -> Int32:
        if not self.ctx.consume_dirty():
            return 0
        self.run_effects()   # effects may write signals → more dirty
        var idx = self.render()
        return self.ctx.flush(writer, idx)
```

---

### P34 Design

#### Effect execution model

Effects are reactive side effects that run when their subscribed
signals change. Unlike memos (which cache a derived value), effects
perform arbitrary work — writing to other signals, updating derived
state, logging, etc.

```text
Event → signal write → scope dirty + effect pending
                              │              │
                              ▼              ▼
                           flush()     run_effects()
                              │              │
                              │         reads signals (re-subscribe)
                              │         writes derived signals
                              │              │
                              ▼              ▼
                           render()     more scopes dirty
                              │              │
                              └──────┬───────┘
                                     ▼
                              diff + mutations
```

#### Drain-and-run pattern

The standard pattern for effects in the flush cycle:

```mojo
fn flush(mut self, writer: ...) -> Int32:
    if not self.ctx.consume_dirty():
        return 0
    # Run pending effects — they may write signals
    self.run_effects()
    # Now render with all state settled
    var idx = self.render()
    return self.ctx.flush(writer, idx)
```

Effects MUST run before `render()` because they may write to signals
that are read during rendering. The effect's `begin_run()` /
`end_run()` bracket establishes a reactive context so signal reads
during the effect body are tracked as dependencies.

#### Effect + signal chain

```text
count signal     ──write──→  scope dirty + count_effect pending
                                              │
count_effect.begin_run()                      │
  count.read()  ← re-subscribe to count      │
  doubled.set(count * 2)  → scope dirty       │
  parity.set(...)         → scope dirty        │
count_effect.end_run()                         │
                                              ▼
render()  ← reads count, doubled, parity (peek)
```

#### Memo + effect chain

A signal → memo → effect → signal chain demonstrates full reactive
propagation:

```text
input signal → memo (derived = input * 3) → effect reads memo output
                                              → effect writes to
                                                output signal
                                              → output signal
                                                triggers render
```

The EffectMemoApp demo validates this chain.

#### P34 ComponentContext surface

No new methods needed — `use_effect()` and `create_effect()` already
exist. The phase demonstrates the *pattern* of using effects in real
components, not new API surface.

#### P34 JS runtime

No new JS runtime infrastructure needed. Effects are entirely
WASM-side — the JS runtime just applies mutations as usual.

---

### P34 Steps

#### P34.1 — EffectDemoApp

**Goal:** A working app with a count signal and an effect that
computes derived state (doubled, parity) — demonstrating the
effect-in-flush pattern.

##### App structure: EffectDemo

```text
EffectDemoApp (root scope)
├── h1 "Effect Demo"
├── button "+ 1"  (onclick_add count)
├── p > dyn_text("Count: N")
├── p > dyn_text("Doubled: N")
└── p > dyn_text("Parity: even/odd")
```

**Lifecycle:**

1. **Init:** Create count, doubled, parity signals + one effect.
   Effect starts pending (initial run needed).
2. **First flush:** `consume_dirty()` → run_effects (sets doubled=0,
   parity="even") → render → mount.
3. **Increment:** count += 1 → scope dirty + effect pending.
4. **Flush:** run_effects (doubled=2, parity="odd") → render → diff
   → SetText mutations for all three texts.
5. **Multiple increments:** Each increment triggers effect → correct
   derived state.

**WASM exports (~15):**

```mojo
@export fn ed_init() -> Int64
@export fn ed_destroy(app_ptr: Int64)
@export fn ed_rebuild(app_ptr: Int64, buf_ptr: Int64, cap: Int32) -> Int32
@export fn ed_handle_event(app_ptr: Int64, hid: Int32, evt: Int32) -> Int32
@export fn ed_flush(app_ptr: Int64, buf_ptr: Int64, cap: Int32) -> Int32
@export fn ed_count_value(app_ptr: Int64) -> Int32
@export fn ed_doubled_value(app_ptr: Int64) -> Int32
@export fn ed_parity_text(app_ptr: Int64) -> String
@export fn ed_effect_is_pending(app_ptr: Int64) -> Int32
@export fn ed_incr_handler(app_ptr: Int64) -> Int32
@export fn ed_has_dirty(app_ptr: Int64) -> Int32
@export fn ed_scope_count(app_ptr: Int64) -> Int32
```

##### TypeScript handle

```typescript
interface EffectDemoAppHandle extends AppHandle {
  getCount(): number;
  getDoubled(): number;
  getParity(): string;
  isEffectPending(): boolean;
  hasDirty(): boolean;
  increment(): void;
}
```

##### Test: `test/test_effect_demo.mojo` (~18 tests)

1. `ed_init_creates_app` — pointer valid
2. `ed_count_starts_at_0` — initial count
3. `ed_doubled_starts_at_0` — initial doubled
4. `ed_parity_starts_at_even` — initial parity
5. `ed_effect_starts_pending` — initial run needed
6. `ed_rebuild_runs_effect` — after rebuild, doubled=0, parity="even"
7. `ed_increment_updates_count` — count = 1
8. `ed_increment_marks_effect_pending` — effect pending after increment
9. `ed_flush_after_increment_doubled` — doubled = 2
10. `ed_flush_after_increment_parity` — parity = "odd"
11. `ed_effect_not_pending_after_flush` — cleared after run
12. `ed_two_increments_doubled_4` — count=2, doubled=4
13. `ed_two_increments_parity_even` — count=2, parity="even"
14. `ed_10_increments` — count=10, doubled=20, parity="even"
15. `ed_effect_resubscribes_each_run` — dependency tracking works
16. `ed_destroy_does_not_crash` — clean shutdown
17. `ed_flush_returns_0_when_clean` — no mutations when clean
18. `ed_rapid_20_increments` — 20 increments, all correct

##### Test: `test-js/effect_demo.test.ts` (~20 suites)

1. `ed_init state validation` — count=0, doubled=0, parity="even"
2. `ed_rebuild produces mutations` — templates, text nodes
3. `ed_DOM structure initial` — h1 + button + 3 paragraphs
4. `ed_DOM text initial` — "Count: 0", "Doubled: 0", "Parity: even"
5. `ed_increment and flush` — "Count: 1", "Doubled: 2", "Parity: odd"
6. `ed_two increments` — "Count: 2", "Doubled: 4", "Parity: even"
7. `ed_10 increments` — all correct
8. `ed_effect pending after increment` — pending before flush
9. `ed_effect cleared after flush` — not pending after flush
10. `ed_flush returns 0 when clean` — no mutations
11. `ed_derived state always consistent` — doubled = count * 2
12. `ed_parity alternates` — odd/even sequence correct for 5
    increments
13. `ed_destroy does not crash` — clean shutdown
14. `ed_double destroy safe` — no crash
15. `ed_multiple independent instances` — isolated
16. `ed_rapid 20 increments` — all correct
17. `ed_heapStats bounded across increments` — memory stable
18. `ed_DOM updates minimal` — only changed text nodes get SetText
19. `ed_rebuild + immediate flush` — effect runs on first flush
20. `ed_increment without flush` — state stale until flushed

Register in `test-js/run.ts`.

---

#### P34.2 — EffectMemoApp (effect + memo chain)

**Goal:** Demonstrate the signal → memo → effect → signal reactive
chain, where a memo derives a value and an effect reads it to produce
further derived state.

##### App structure: EffectMemo

```text
EffectMemoApp (root scope)
├── h1 "Effect + Memo"
├── button "+ 1"  (onclick_add input)
├── p > dyn_text("Input: N")
├── p > dyn_text("Tripled: N")     ← memo output (input * 3)
├── p > dyn_text("Label: ...")     ← effect reads tripled, writes label
```

**Chain:**

```text
input signal → tripled memo (input * 3) → label effect
                                            reads tripled.read()
                                            writes label signal
                                            ("small" if <10, "big" if ≥10)
```

**WASM exports (~15):**

```mojo
@export fn em_init() -> Int64
@export fn em_destroy(app_ptr: Int64)
@export fn em_rebuild(app_ptr: Int64, buf_ptr: Int64, cap: Int32) -> Int32
@export fn em_handle_event(app_ptr: Int64, hid: Int32, evt: Int32) -> Int32
@export fn em_flush(app_ptr: Int64, buf_ptr: Int64, cap: Int32) -> Int32
@export fn em_input_value(app_ptr: Int64) -> Int32
@export fn em_tripled_value(app_ptr: Int64) -> Int32
@export fn em_label_text(app_ptr: Int64) -> String
@export fn em_effect_is_pending(app_ptr: Int64) -> Int32
@export fn em_memo_value(app_ptr: Int64) -> Int32
@export fn em_incr_handler(app_ptr: Int64) -> Int32
@export fn em_has_dirty(app_ptr: Int64) -> Int32
```

##### TypeScript handle

```typescript
interface EffectMemoAppHandle extends AppHandle {
  getInput(): number;
  getTripled(): number;
  getLabel(): string;
  isEffectPending(): boolean;
  getMemoValue(): number;
  hasDirty(): boolean;
  increment(): void;
}
```

##### Test: `test/test_effect_memo.mojo` (~16 tests)

1. `em_init_creates_app` — pointer valid
2. `em_input_starts_at_0` — initial input
3. `em_tripled_starts_at_0` — memo starts at 0
4. `em_label_starts_at_small` — "small" (0 < 10)
5. `em_increment_updates_input` — input = 1
6. `em_flush_updates_tripled` — tripled = 3
7. `em_flush_updates_label` — "small" (3 < 10)
8. `em_3_increments_tripled_9` — input=3, tripled=9, label="small"
9. `em_4_increments_tripled_12` — input=4, tripled=12, label="big"
10. `em_threshold_boundary` — input=3 → "small", input=4 → "big"
11. `em_memo_and_effect_both_run` — memo recalculates, effect re-runs
12. `em_effect_reads_memo_not_input` — effect depends on tripled,
    not input directly
13. `em_10_increments` — input=10, tripled=30, label="big"
14. `em_destroy_does_not_crash` — clean shutdown
15. `em_flush_returns_0_when_clean` — no mutations
16. `em_rapid_20_increments` — all correct

##### Test: `test-js/effect_memo.test.ts` (~18 suites)

1. `em_init state validation` — input=0, tripled=0, label="small"
2. `em_rebuild produces mutations` — templates, text nodes
3. `em_DOM structure initial` — h1 + button + 3 paragraphs
4. `em_DOM text initial` — "Input: 0", "Tripled: 0", "Label: small"
5. `em_increment and flush` — "Input: 1", "Tripled: 3", "Label: small"
6. `em_4 increments crosses threshold` — label changes to "big"
7. `em_10 increments` — all correct
8. `em_memo + effect both update on same flush` — consistent state
9. `em_flush returns 0 when clean` — no mutations
10. `em_destroy does not crash` — clean shutdown
11. `em_double destroy safe` — no crash
12. `em_multiple independent instances` — isolated
13. `em_rapid 20 increments` — all correct
14. `em_heapStats bounded` — memory stable
15. `em_DOM updates minimal` — only changed text nodes
16. `em_threshold transition exact` — 3→4 is small→big
17. `em_derived state chain consistent` — tripled always input*3,
    label always correct for tripled
18. `em_memo value matches tripled` — memo output accessible

Register in `test-js/run.ts`.

---

#### P34.3 — Documentation & AGENTS.md update

**Goal:** Update project documentation to reflect the effect patterns
and demos.

##### Changes

**`AGENTS.md`** — Update:

- Common Patterns: Add "Effect drain-and-run pattern" documenting the
  standard `run_effects()` → `render()` → `flush()` sequence
- Common Patterns: Add "Effect + memo chain" documenting the
  signal → memo → effect → signal pattern
- App Architectures: Add EffectDemoApp and EffectMemoApp descriptions
- File Size Reference: Update file sizes

**`CHANGELOG.md`** — Add Phase 34 entry.

**`README.md`** — Update:

- Features list: add "Effects in apps — reactive side effects with
  derived state, effect + memo chains"
- Test count
- Test results section: add Effect demo test descriptions
- Ergonomic API section: add effect drain-and-run code example

---

### P34 Dependency graph

```text
P34.1 (EffectDemo — basic effect-in-flush)
    │
    ▼
P34.2 (EffectMemo — signal → memo → effect → signal chain)
    │
    ▼
P34.3 (Documentation)
```

P34.1 establishes the effect-in-flush pattern. P34.2 builds on it
with a memo chain. P34.3 updates documentation.

---

### P34 Estimated size

| Step | Description | ~New Lines | Tests |
|------|-------------|-----------|-------|
| P34.1 | EffectDemoApp | ~250 Mojo, ~100 TS | 18 Mojo + 20 JS |
| P34.2 | EffectMemoApp | ~280 Mojo, ~100 TS | 16 Mojo + 18 JS |
| P34.3 | Documentation update | ~0 Mojo, ~50 prose | 0 |
| **Total** | | **~530 Mojo, ~250 TS** | **34 Mojo + 38 JS = 72 tests** |

---

## Combined dependency graph (Phase 33 + 34)

```text
P33.1 (Suspense surface)             P34.1 (EffectDemo)
    │                                     │
    ├──────────┐                          ▼
    ▼          ▼                     P34.2 (EffectMemo)
P33.2       P33.3                         │
(DataLoader) (SuspenseNest)               ▼
    │          │                     P34.3 (Effect docs)
    └────┬─────┘
         ▼
    P33.4 (Suspense docs)
```

Phase 33 and Phase 34 are independent — they can be executed in
either order or in parallel. Phase 33 surfaces the last remaining
dead scope infrastructure (suspense). Phase 34 validates the existing
effect system in real component lifecycles.

## Combined estimated size

| Phase | ~New Lines | Tests |
|-------|-----------|-------|
| Phase 33 (Suspense) | ~880 Mojo, ~310 TS | 57 Mojo + 47 JS = 104 |
| Phase 34 (Effects) | ~530 Mojo, ~250 TS | 34 Mojo + 38 JS = 72 |
| **Total** | **~1,410 Mojo, ~560 TS** | **91 Mojo + 85 JS = 176 tests** |

---

## Phase 35 — Memo Type Expansion (MemoBool + MemoString)

### P35 Problem

Phase 13 added `MemoI32` — a cached derived value with automatic
dependency tracking via reactive contexts. Phase 18 and 19 expanded
the signal system with `SignalBool` and `SignalString`, giving all
three value types first-class reactive signals. However:

1. **Only `MemoI32` exists.** The memo system only caches `Int32`
   derived values. There is no way to create a cached derived `Bool`
   or `String` without the effect-signal workaround.

2. **Effect-signal workaround is suboptimal.** Phase 34's
   EffectDemoApp derives `parity: SignalString` via an effect that
   reads `count` and writes `"even"/"odd"`. This works but is
   heavier than a memo — effects always mark dependents dirty even if
   the output value didn't change, while a proper memo can skip
   notification when the recomputed value equals the cached one.

3. **Type coverage gap.** Signals have three types (I32, Bool,
   String), but memos have only one (I32). This asymmetry forces
   developers to use effects for derived booleans and strings, mixing
   concerns (effects are for side effects, memos are for derived
   values).

4. **No mixed-type memo chains.** A chain like
   `SignalI32 → MemoI32 → MemoBool → MemoString` (numeric input →
   computed value → threshold check → label) would validate that
   memos of different output types propagate dirtiness correctly
   through the reactive graph.

5. **`ChildComponentContext` has the same gap.** It exposes
   `use_memo(initial: Int32) -> MemoI32` but nothing for Bool or
   String derived values.

### P35 Current state

Memo infrastructure exists for Int32 only:

```mojo
# Runtime — create/read/write
fn create_memo_i32(mut self, scope_id: UInt32, initial: Int32) -> UInt32
fn memo_begin_compute(mut self, memo_id: UInt32)       # type-agnostic
fn memo_end_compute_i32(mut self, memo_id: UInt32, value: Int32)
fn memo_read_i32(mut self, memo_id: UInt32) -> Int32
fn use_memo_i32(mut self, initial: Int32) -> UInt32

# MemoEntry — stores context_id + output_key (generic UInt32 keys)
# MemoStore — slab allocator, dirty tracking, scope cleanup

# ComponentContext
fn use_memo(mut self, initial: Int32) -> MemoI32
fn create_memo(mut self, initial: Int32) -> MemoI32

# ChildComponentContext
fn use_memo(mut self, initial: Int32) -> MemoI32

# Handle
struct MemoI32 — read(), peek(), is_dirty(), begin_compute(), end_compute(Int32)
```

String signals use a separate `StringStore` + version signal pattern:

```mojo
# Runtime — string signal
fn create_signal_string(mut self, initial: String) -> Tuple[UInt32, UInt32]
#   returns (string_key, version_key)
fn read_signal_string(mut self, string_key: UInt32, version_key: UInt32) -> String
fn write_signal_string(mut self, string_key: UInt32, version_key: UInt32, value: String)
fn peek_signal_string(self, string_key: UInt32) -> String
```

### P35 Target pattern

```mojo
# ── MemoBool ──────────────────────────────────────────────────────

struct MemoFormApp:
    var ctx: ComponentContext
    var input: SignalString
    var is_valid: MemoBool          # derived: len(input) > 0
    var status: MemoString          # derived: "✓ Valid: {input}" / "✗ Empty"

    fn __init__(out self):
        self.ctx = ComponentContext.create()
        self.input = self.ctx.use_signal_string(String(""))
        self.is_valid = self.ctx.use_memo_bool(False)
        self.status = self.ctx.use_memo_string(String("✗ Empty"))
        self.ctx.setup_view(
            el_div(
                el_h1(dsl_text(String("Form Validation"))),
                el_input(
                    attr("type", "text"),
                    bind_value(self.input),
                    oninput_set_string(self.input),
                ),
                el_p(dyn_text()),   # "Valid: true/false"
                el_p(dyn_text()),   # "Status: ..."
            ),
            String("memo-form"),
        )

    fn run_memos(mut self):
        """Recompute dirty memos in dependency order."""
        # Step 1: MemoBool (depends on input signal only)
        if self.is_valid.is_dirty():
            self.is_valid.begin_compute()
            var txt = self.input.read()       # re-subscribe
            self.is_valid.end_compute(len(txt) > 0)
        # Step 2: MemoString (depends on input + is_valid)
        if self.status.is_dirty():
            self.status.begin_compute()
            var txt = self.input.read()       # re-subscribe
            var valid = self.is_valid.read()  # re-subscribe
            if valid:
                self.status.end_compute(String("✓ Valid: ") + txt)
            else:
                self.status.end_compute(String("✗ Empty"))

    fn render(mut self) -> UInt32:
        var vb = self.ctx.render_builder()
        vb.add_dyn_text(
            String("Valid: ") + String(self.is_valid.peek())
        )
        vb.add_dyn_text(
            String("Status: ") + String(self.status.peek())
        )
        return vb.build()

    fn flush(mut self, writer: ...) -> Int32:
        if not self.ctx.consume_dirty():
            return 0
        self.run_memos()
        var idx = self.render()
        return self.ctx.flush(writer, idx)
```

---

### P35 Design

#### MemoEntry reuse

`MemoEntry` already stores `context_id: UInt32` and
`output_key: UInt32` — these are opaque signal keys. For `MemoBool`,
the output_key points to a `Bool` signal (`signals.create[Bool]`).
No changes to `MemoEntry` or `MemoStore` are needed for `MemoBool`.

For `MemoString`, the output is stored in `StringStore` (same pattern
as `SignalString`), which requires two keys: `string_key` (into
`StringStore`) and `version_key` (an `Int32` signal for change
tracking / subscriptions). The existing `output_key` field stores
the `version_key`, and a new `string_key` field is added to
`MemoEntry` (default 0 for non-string memos).

```mojo
struct MemoEntry:
    var context_id: UInt32
    var output_key: UInt32      # Int32/Bool signal, or version signal for String
    var string_key: UInt32      # StringStore key (0 for non-string memos)
    var scope_id: UInt32
    var dirty: Bool
    var computing: Bool
```

#### Runtime surface

New methods mirror the existing Int32 pattern:

```mojo
# ── MemoBool ─────────────────────────────────────────────────────
fn create_memo_bool(mut self, scope_id: UInt32, initial: Bool) -> UInt32
fn memo_end_compute_bool(mut self, memo_id: UInt32, value: Bool)
fn memo_read_bool(mut self, memo_id: UInt32) -> Bool
fn use_memo_bool(mut self, initial: Bool) -> UInt32

# ── MemoString ───────────────────────────────────────────────────
fn create_memo_string(mut self, scope_id: UInt32, initial: String) -> UInt32
fn memo_end_compute_string(mut self, memo_id: UInt32, value: String)
fn memo_read_string(mut self, memo_id: UInt32) -> String
fn memo_peek_string(self, memo_id: UInt32) -> String
fn use_memo_string(mut self, initial: String) -> UInt32
fn destroy_memo_string(mut self, memo_id: UInt32)
```

`memo_begin_compute()` is type-agnostic — it sets the reactive
context and clears old subscriptions regardless of the output type.
No change needed.

#### Handle types

```mojo
struct MemoBool(Copyable, Stringable):
    var id: UInt32
    var runtime: UnsafePointer[Runtime, MutExternalOrigin]

    fn read(self) -> Bool          # with context tracking
    fn peek(self) -> Bool          # without subscribing
    fn is_dirty(self) -> Bool
    fn begin_compute(self)
    fn end_compute(self, value: Bool)
    fn recompute_from(self, value: Bool)

struct MemoString(Copyable, Stringable):
    var id: UInt32
    var runtime: UnsafePointer[Runtime, MutExternalOrigin]

    fn read(self) -> String        # with context tracking (via version signal)
    fn peek(self) -> String        # without subscribing
    fn get(self) -> String         # alias for read (matches SignalString)
    fn is_dirty(self) -> Bool
    fn begin_compute(self)
    fn end_compute(self, value: String)
    fn recompute_from(self, value: String)
    fn is_empty(self) -> Bool      # convenience: peek().is_empty()
```

#### ComponentContext surface

```mojo
# ComponentContext
fn use_memo_bool(mut self, initial: Bool) -> MemoBool
fn create_memo_bool(mut self, initial: Bool) -> MemoBool
fn use_memo_string(mut self, initial: String) -> MemoString
fn create_memo_string(mut self, initial: String) -> MemoString

# ChildComponentContext
fn use_memo_bool(mut self, initial: Bool) -> MemoBool
fn use_memo_string(mut self, initial: String) -> MemoString
```

#### AppShell surface

```mojo
fn create_memo_bool(mut self, scope_id: UInt32, initial: Bool) -> UInt32
fn memo_end_compute_bool(mut self, memo_id: UInt32, value: Bool)
fn memo_read_bool(mut self, memo_id: UInt32) -> Bool
fn use_memo_bool(mut self, initial: Bool) -> UInt32

fn create_memo_string(mut self, scope_id: UInt32, initial: String) -> UInt32
fn memo_end_compute_string(mut self, memo_id: UInt32, value: String)
fn memo_read_string(mut self, memo_id: UInt32) -> String
fn memo_peek_string(self, memo_id: UInt32) -> String
fn use_memo_string(mut self, initial: String) -> UInt32
```

#### Memo recomputation order

Multiple memos must be recomputed in dependency order. The standard
pattern (already established in Phase 34's EffectMemoApp) is:

```text
fn run_memos():
    # Recompute in dependency order (earlier memos first)
    if memo_a.is_dirty():
        memo_a.begin_compute()
        ... read inputs ...
        memo_a.end_compute(result_a)
    if memo_b.is_dirty():
        memo_b.begin_compute()
        ... read inputs + memo_a.read() ...
        memo_b.end_compute(result_b)
```

Since Mojo WASM cannot store closures, the recomputation logic lives
in the component — the memo handle provides only lifecycle
management. The developer is responsible for ordering recomputations
correctly. This is the same pattern as `MemoI32`.

#### MemoString lifecycle & cleanup

When a memo string is destroyed (scope cleanup), the Runtime must
destroy both the version signal (output_key) AND the StringStore
entry (string_key). The existing `destroy_memo` path destroys
`context_id` and `output_key` signals — the new `string_key` field
adds one additional `strings.destroy(string_key)` call for string
memos (only when `string_key != 0`).

#### JS runtime

No new JS runtime infrastructure. MemoBool and MemoString are
entirely WASM-side — derived values flow through the normal mutation
protocol (`SetText`, `SetAttribute`) during render/diff.

---

### P35 Steps

#### P35.1 — MemoBool + MemoString infrastructure

**Goal:** Add `MemoBool` and `MemoString` handle types, Runtime
methods, AppShell wrappers, and ComponentContext / ChildComponentContext
hooks. Add unit-level Mojo tests for the new types.

##### Mojo changes

###### `src/signals/memo.mojo` — MemoEntry extension

Add `string_key: UInt32` field to `MemoEntry` (default 0). Update
constructors, `__copyinit__`, `__moveinit__`. Add a string-aware
constructor:

```mojo
fn __init__(
    out self,
    context_id: UInt32,
    output_key: UInt32,
    string_key: UInt32,
    scope_id: UInt32,
):
    self.context_id = context_id
    self.output_key = output_key
    self.string_key = string_key
    self.scope_id = scope_id
    self.dirty = True
    self.computing = False
```

Add `string_key()` accessor to `MemoStore`.

###### `src/signals/handle.mojo` — MemoBool + MemoString structs

Add `MemoBool` and `MemoString` handle structs following the same
pattern as `MemoI32`. Both hold `id: UInt32` + non-owning
`runtime: UnsafePointer[Runtime, MutExternalOrigin]`.

`MemoBool` methods: `read() -> Bool`, `peek() -> Bool`,
`is_dirty() -> Bool`, `begin_compute()`, `end_compute(Bool)`,
`recompute_from(Bool)`, `__str__() -> String`.

`MemoString` methods: `read() -> String`, `peek() -> String`,
`get() -> String`, `is_dirty() -> Bool`, `begin_compute()`,
`end_compute(String)`, `recompute_from(String)`,
`is_empty() -> Bool`, `__str__() -> String`.

###### `src/signals/runtime.mojo` — Runtime methods

Add:

- `create_memo_bool(scope_id, initial) -> UInt32` — creates context
  signal + `Bool` output signal, stores in MemoStore.
- `memo_end_compute_bool(memo_id, value)` — writes `Bool` to output
  signal, clears dirty, restores context.
- `memo_read_bool(memo_id) -> Bool` — reads output signal with
  context tracking.
- `use_memo_bool(initial) -> UInt32` — hook version (first render
  creates, re-render retrieves).
- `create_memo_string(scope_id, initial) -> UInt32` — creates context
  signal + StringStore entry + version signal, stores in MemoStore
  with `string_key`.
- `memo_end_compute_string(memo_id, value)` — writes to StringStore,
  bumps version signal, clears dirty, restores context.
- `memo_read_string(memo_id) -> String` — reads StringStore, subscribes
  via version signal.
- `memo_peek_string(memo_id) -> String` — reads StringStore without
  subscribing.
- `use_memo_string(initial) -> UInt32` — hook version.
- Update `destroy_memo()` to also destroy `string_key` when non-zero.

###### `src/component/app_shell.mojo` — AppShell wrappers

Add forwarding methods for all new Runtime methods (same pattern as
existing `memo_end_compute_i32` / `memo_read_i32` wrappers).

###### `src/component/context.mojo` — ComponentContext hooks

Add `use_memo_bool(Bool) -> MemoBool`,
`create_memo_bool(Bool) -> MemoBool`,
`use_memo_string(String) -> MemoString`,
`create_memo_string(String) -> MemoString`.

###### `src/component/child_context.mojo` — ChildComponentContext hooks

Add `use_memo_bool(Bool) -> MemoBool`,
`use_memo_string(String) -> MemoString`.

###### `src/signals/__init__.mojo` — Exports

Export `MemoBool`, `MemoString` from the signals package.

##### WASM exports (in `src/main.mojo`)

Test-support exports for direct memo manipulation from JS/wasmtime:

```mojo
@export fn memo_bool_create(rt_ptr: Int64, scope_id: Int32, initial: Int32) -> Int32
@export fn memo_bool_begin_compute(rt_ptr: Int64, memo_id: Int32)
@export fn memo_bool_end_compute(rt_ptr: Int64, memo_id: Int32, value: Int32)
@export fn memo_bool_read(rt_ptr: Int64, memo_id: Int32) -> Int32
@export fn memo_bool_is_dirty(rt_ptr: Int64, memo_id: Int32) -> Int32

@export fn memo_string_create(rt_ptr: Int64, scope_id: Int32) -> Int32
@export fn memo_string_begin_compute(rt_ptr: Int64, memo_id: Int32)
@export fn memo_string_end_compute(rt_ptr: Int64, memo_id: Int32, buf_ptr: Int64, len: Int32)
@export fn memo_string_read(rt_ptr: Int64, memo_id: Int32, buf_ptr: Int64, cap: Int32) -> Int32
@export fn memo_string_peek(rt_ptr: Int64, memo_id: Int32, buf_ptr: Int64, cap: Int32) -> Int32
@export fn memo_string_is_dirty(rt_ptr: Int64, memo_id: Int32) -> Int32
```

##### Test: `test/test_memo_bool.mojo` (~14 tests)

1. `mb_create_returns_valid_id` — memo ID is valid
2. `mb_starts_dirty` — initial dirty flag True
3. `mb_initial_value` — peek returns initial value
4. `mb_compute_stores_value` — begin/end compute stores True
5. `mb_compute_clears_dirty` — dirty cleared after compute
6. `mb_signal_write_marks_dirty` — writing subscribed signal dirties memo
7. `mb_read_subscribes_context` — reading in context subscribes
8. `mb_recompute_from_convenience` — single-call recompute
9. `mb_peek_does_not_subscribe` — peek has no side effects
10. `mb_destroy_cleans_up` — memo count decremented
11. `mb_scope_cleanup_destroys_memo` — scope destroy removes memo
12. `mb_multiple_memos_independent` — two memos don't interfere
13. `mb_dirty_propagates_through_chain` — signal → memo_bool chain
14. `mb_str_conversion` — **str** returns "True"/"False"

##### Test: `test/test_memo_string.mojo` (~16 tests)

1. `ms_create_returns_valid_id` — memo ID is valid
2. `ms_starts_dirty` — initial dirty flag True
3. `ms_initial_value` — peek returns initial string
4. `ms_compute_stores_value` — begin/end compute stores string
5. `ms_compute_clears_dirty` — dirty cleared after compute
6. `ms_signal_write_marks_dirty` — writing subscribed signal dirties memo
7. `ms_read_subscribes_context` — reading in context subscribes via version
8. `ms_recompute_from_convenience` — single-call recompute
9. `ms_peek_does_not_subscribe` — peek has no side effects
10. `ms_is_empty_when_empty` — is_empty returns True for ""
11. `ms_is_empty_when_not_empty` — is_empty returns False for "hello"
12. `ms_destroy_cleans_up` — memo count decremented, string freed
13. `ms_scope_cleanup_destroys_memo` — scope destroy removes memo + string
14. `ms_multiple_memos_independent` — two string memos don't interfere
15. `ms_dirty_propagates_through_chain` — signal → memo_string chain
16. `ms_str_conversion` — **str** returns the cached string

---

#### P35.2 — MemoFormApp (MemoBool + MemoString in a form)

**Goal:** A working app with a string input, a `MemoBool` derived
value (validation), and a `MemoString` derived value (status label) —
demonstrating memo type expansion in a practical form-validation
scenario.

##### App structure: MemoForm

```text
MemoFormApp (root scope)
├── h1 "Form Validation"
├── input  (type="text", bind_value + oninput_set_string → input signal)
├── p > dyn_text("Valid: true/false")         ← MemoBool output
├── p > dyn_text("Status: ✓ Valid: .../✗ Empty")  ← MemoString output
```

**Lifecycle:**

1. **Init:** Create `input` (SignalString, ""), `is_valid` (MemoBool,
   False), `status` (MemoString, "✗ Empty"). Both memos start dirty.
2. **Rebuild:** `run_memos()` → is_valid recomputes (reads input →
   len("") == 0 → False), status recomputes (reads input + is_valid →
   "✗ Empty") → render → mount.
3. **Type "hi":** input signal = "hi" → scope dirty + both memos
   dirty. Flush → is_valid recomputes (True), status recomputes
   ("✓ Valid: hi") → render → diff → SetText.
4. **Clear input:** input = "" → is_valid = False, status = "✗ Empty".

**WASM exports (~17):**

```mojo
@export fn mf_init() -> Int64
@export fn mf_destroy(app_ptr: Int64)
@export fn mf_rebuild(app_ptr: Int64, buf_ptr: Int64, cap: Int32) -> Int32
@export fn mf_handle_event(app_ptr: Int64, hid: Int32, evt: Int32) -> Int32
@export fn mf_handle_event_string(app_ptr: Int64, hid: Int32, evt: Int32, buf_ptr: Int64, len: Int32) -> Int32
@export fn mf_flush(app_ptr: Int64, buf_ptr: Int64, cap: Int32) -> Int32
@export fn mf_input_text(app_ptr: Int64, buf_ptr: Int64, cap: Int32) -> Int32
@export fn mf_is_valid(app_ptr: Int64) -> Int32
@export fn mf_status_text(app_ptr: Int64, buf_ptr: Int64, cap: Int32) -> Int32
@export fn mf_is_valid_dirty(app_ptr: Int64) -> Int32
@export fn mf_status_dirty(app_ptr: Int64) -> Int32
@export fn mf_set_input(app_ptr: Int64, buf_ptr: Int64, len: Int32)
@export fn mf_input_handler(app_ptr: Int64) -> Int32
@export fn mf_has_dirty(app_ptr: Int64) -> Int32
@export fn mf_scope_count(app_ptr: Int64) -> Int32
@export fn mf_memo_count(app_ptr: Int64) -> Int32
```

`mf_set_input` is a test helper that writes a string directly to the
input signal (simulates user typing without going through the event
system).

##### TypeScript handle

```typescript
interface MemoFormAppHandle extends AppHandle {
  getInput(): string;
  isValid(): boolean;
  getStatus(): string;
  isValidDirty(): boolean;
  isStatusDirty(): boolean;
  setInput(value: string): void;
  hasDirty(): boolean;
  getMemoCount(): number;
}
```

##### Test: `test/test_memo_form.mojo` (~18 tests)

1. `mf_init_creates_app` — pointer valid
2. `mf_input_starts_empty` — initial input ""
3. `mf_is_valid_starts_false` — initial validation False
4. `mf_status_starts_empty_marker` — initial status "✗ Empty"
5. `mf_memos_start_dirty` — both memos dirty before first flush
6. `mf_rebuild_settles_memos` — after rebuild, both clean
7. `mf_rebuild_is_valid_false` — is_valid = False after rebuild
8. `mf_rebuild_status_empty` — status = "✗ Empty" after rebuild
9. `mf_set_input_marks_dirty` — setting input dirties both memos
10. `mf_flush_after_set_input_valid` — is_valid = True for "hello"
11. `mf_flush_after_set_input_status` — status = "✓ Valid: hello"
12. `mf_clear_input_reverts` — setting "" → is_valid=False, status="✗ Empty"
13. `mf_memo_recomputation_order` — is_valid recomputed before status
14. `mf_multiple_inputs_correct` — "a" → "ab" → "abc" all correct
15. `mf_flush_returns_0_when_clean` — no mutations when clean
16. `mf_memo_count_is_2` — two live memos
17. `mf_destroy_does_not_crash` — clean shutdown
18. `mf_scope_count_is_1` — single root scope

##### Test: `test-js/memo_form.test.ts` (~20 suites)

1. `mf_init state validation` — input="", valid=false, status="✗ Empty"
2. `mf_rebuild produces mutations` — templates, text nodes
3. `mf_DOM structure initial` — h1 + input + 2 paragraphs
4. `mf_DOM text initial` — "Valid: false", "Status: ✗ Empty"
5. `mf_setInput and flush` — "hi" → "Valid: true", "Status: ✓ Valid: hi"
6. `mf_clear input reverts DOM` — "" → "Valid: false", "Status: ✗ Empty"
7. `mf_multiple inputs` — "a" → "ab" → "abc", all DOM texts correct
8. `mf_memos dirty after setInput` — dirty before flush
9. `mf_memos clean after flush` — clean after flush
10. `mf_flush returns 0 when clean` — no mutations
11. `mf_derived state consistent` — valid iff input non-empty
12. `mf_status matches validation` — "✓" when valid, "✗" when invalid
13. `mf_destroy does not crash` — clean shutdown
14. `mf_double destroy safe` — no crash
15. `mf_multiple independent instances` — isolated
16. `mf_rapid 20 inputs` — all correct
17. `mf_heapStats bounded across inputs` — memory stable
18. `mf_DOM updates minimal` — only changed text nodes get SetText
19. `mf_input element has value attribute` — bind_value works
20. `mf_memo count is 2` — two live memos

Register in `test-js/run.ts`.

---

#### P35.3 — MemoChainApp (mixed-type memo chain)

**Goal:** Demonstrate a multi-level mixed-type memo chain:
`SignalI32 → MemoI32 → MemoBool → MemoString`, validating that
dirtiness propagates correctly across memo types and that
recomputation order is deterministic.

##### App structure: MemoChain

```text
MemoChainApp (root scope)
├── h1 "Memo Chain"
├── button "+ 1"  (onclick_add input)
├── p > dyn_text("Input: N")
├── p > dyn_text("Doubled: N")          ← MemoI32 (input * 2)
├── p > dyn_text("Is Big: true/false")  ← MemoBool (doubled >= 10)
├── p > dyn_text("Label: small/BIG")    ← MemoString (is_big ? "BIG" : "small")
```

**Lifecycle:**

1. **Init:** `input` signal (0), `doubled` MemoI32 (0), `is_big`
   MemoBool (False), `label` MemoString ("small"). All memos start
   dirty.
2. **Rebuild:** Recompute chain: doubled=0 → is_big=False →
   label="small" → render → mount.
3. **Increment to 5:** input=5 → doubled=10 → is_big=True →
   label="BIG" → diff → SetText for all four texts.
4. **Increment to 6:** input=6 → doubled=12 → is_big=True (no
   change) → label="BIG" (no change) → diff → SetText only for
   Input and Doubled texts.

Step 4 validates that MemoBool can detect "no change" when the
recomputed value equals the cached value. Whether the framework
currently optimizes this is documented — even without the
optimization, the chain must produce correct final values.

**WASM exports (~18):**

```mojo
@export fn mc_init() -> Int64
@export fn mc_destroy(app_ptr: Int64)
@export fn mc_rebuild(app_ptr: Int64, buf_ptr: Int64, cap: Int32) -> Int32
@export fn mc_handle_event(app_ptr: Int64, hid: Int32, evt: Int32) -> Int32
@export fn mc_flush(app_ptr: Int64, buf_ptr: Int64, cap: Int32) -> Int32
@export fn mc_input_value(app_ptr: Int64) -> Int32
@export fn mc_doubled_value(app_ptr: Int64) -> Int32
@export fn mc_is_big(app_ptr: Int64) -> Int32
@export fn mc_label_text(app_ptr: Int64, buf_ptr: Int64, cap: Int32) -> Int32
@export fn mc_doubled_dirty(app_ptr: Int64) -> Int32
@export fn mc_is_big_dirty(app_ptr: Int64) -> Int32
@export fn mc_label_dirty(app_ptr: Int64) -> Int32
@export fn mc_incr_handler(app_ptr: Int64) -> Int32
@export fn mc_has_dirty(app_ptr: Int64) -> Int32
@export fn mc_scope_count(app_ptr: Int64) -> Int32
@export fn mc_memo_count(app_ptr: Int64) -> Int32
```

##### TypeScript handle

```typescript
interface MemoChainAppHandle extends AppHandle {
  getInput(): number;
  getDoubled(): number;
  isBig(): boolean;
  getLabel(): string;
  isDoubledDirty(): boolean;
  isBigDirty(): boolean;
  isLabelDirty(): boolean;
  increment(): void;
  hasDirty(): boolean;
  getMemoCount(): number;
}
```

##### Test: `test/test_memo_chain.mojo` (~20 tests)

1. `mc_init_creates_app` — pointer valid
2. `mc_input_starts_at_0` — initial input
3. `mc_doubled_starts_at_0` — initial doubled
4. `mc_is_big_starts_false` — initial is_big
5. `mc_label_starts_small` — initial label "small"
6. `mc_all_memos_start_dirty` — all three dirty
7. `mc_rebuild_settles_all` — all three clean after rebuild
8. `mc_rebuild_values_correct` — doubled=0, is_big=false, label="small"
9. `mc_increment_to_1` — doubled=2, is_big=false, label="small"
10. `mc_increment_to_4` — doubled=8, is_big=false, label="small"
11. `mc_increment_to_5_crosses_threshold` — doubled=10, is_big=true, label="BIG"
12. `mc_increment_to_6_stays_big` — doubled=12, is_big=true, label="BIG"
13. `mc_chain_propagation_order` — doubled recomputed before is_big before label
14. `mc_10_increments_all_correct` — cumulative validation
15. `mc_flush_returns_0_when_clean` — no mutations when clean
16. `mc_memo_count_is_3` — three live memos
17. `mc_scope_count_is_1` — single root scope
18. `mc_destroy_does_not_crash` — clean shutdown
19. `mc_rapid_20_increments` — 20 increments, all correct
20. `mc_threshold_boundary_exact` — input=5 (doubled=10) is the exact boundary

##### Test: `test-js/memo_chain.test.ts` (~22 suites)

1. `mc_init state validation` — input=0, doubled=0, is_big=false, label="small"
2. `mc_rebuild produces mutations` — templates, text nodes
3. `mc_DOM structure initial` — h1 + button + 4 paragraphs
4. `mc_DOM text initial` — all four texts correct
5. `mc_increment and flush` — input=1, doubled=2, is_big=false, label="small"
6. `mc_5 increments crosses threshold` — all four texts updated
7. `mc_6 increments stays big` — doubled=12, is_big=true, label="BIG"
8. `mc_10 increments` — all correct
9. `mc_all memos dirty after increment` — dirty before flush
10. `mc_all memos clean after flush` — clean after flush
11. `mc_flush returns 0 when clean` — no mutations
12. `mc_chain produces correct derived state` — for each increment
13. `mc_threshold boundary exact` — input=5 → is_big flips to true
14. `mc_threshold stable above` — input 6,7,8 all is_big=true
15. `mc_destroy does not crash` — clean shutdown
16. `mc_double destroy safe` — no crash
17. `mc_multiple independent instances` — isolated
18. `mc_rapid 20 increments` — all correct
19. `mc_heapStats bounded across increments` — memory stable
20. `mc_DOM updates minimal` — SetText only for changed values
21. `mc_memo count is 3` — three live memos
22. `mc_rebuild + immediate flush` — all memos settle on first flush

Register in `test-js/run.ts`.

---

#### P35.4 — Documentation & AGENTS.md update

##### Changes

**AGENTS.md:**

- **Key Abstractions → Signals & Reactivity:** Add `MemoBool` and
  `MemoString` to the memo handle list.
- **App Architectures:** Add `MemoFormApp` and `MemoChainApp` entries
  with structure diagrams, field lists, lifecycle summaries, and
  WASM export lists.
- **Common Patterns:** Add "Memo type expansion pattern" documenting
  the recomputation order for mixed-type memo chains, and the
  `MemoString` lifecycle (StringStore + version signal).
- **Deferred Abstractions:** Note that `MemoBool` and `MemoString`
  partially address the "Generic `Signal[T]`" gap — three memo types
  now match three signal types, reducing the urgency for
  parametric `Memo[T]`.
- **File Size Reference:** Update file sizes for changed files.

**CHANGELOG.md:**

- Add Phase 35 entry summarizing P35.1 (infra), P35.2 (MemoFormApp),
  P35.3 (MemoChainApp), P35.4 (docs). Include test count delta.

**README.md:**

- Update features list to mention MemoBool + MemoString.
- Update test count.
- Add memo chain code example in the Ergonomic API section.

---

### P35 Dependency graph

```text
P35.1 (MemoBool + MemoString infra)
    │
    ├──────────┐
    ▼          ▼
P35.2       P35.3
(MemoForm)  (MemoChain)
    │          │
    └────┬─────┘
         ▼
    P35.4 (Docs)
```

P35.1 is the foundation — both apps depend on it. P35.2 and P35.3
are independent and can be built in parallel. P35.4 depends on both
apps being complete.

### P35 Estimated size

| Step | ~New Mojo Lines | ~New TS Lines | Tests |
|------|----------------|---------------|-------|
| P35.1 (infra + unit tests) | ~450 | ~60 | 30 Mojo |
| P35.2 (MemoFormApp) | ~280 | ~180 | 18 Mojo + 20 JS = 38 |
| P35.3 (MemoChainApp) | ~300 | ~200 | 20 Mojo + 22 JS = 42 |
| P35.4 (docs) | ~80 | ~0 | 0 |
| **Total** | **~1,110** | **~440** | **68 Mojo + 42 JS = 110 tests** |

---

## Combined dependency graph (Phase 33 + 34 + 35)

```text
P33.1 (Suspense surface)     P34.1 (EffectDemo)     P35.1 (Memo infra)
    │                             │                       │
    ├──────────┐                  ▼                  ├──────────┐
    ▼          ▼             P34.2 (EffectMemo)      ▼          ▼
P33.2       P33.3                 │              P35.2       P35.3
(DataLoader) (SuspenseNest)       ▼              (MemoForm)  (MemoChain)
    │          │             P34.3 (Effect docs)     │          │
    └────┬─────┘                                     └────┬─────┘
         ▼                                                ▼
    P33.4 (Suspense docs)                           P35.4 (Memo docs)
```

Phase 35 is independent of Phases 33 and 34 — it extends the memo
system, not the effect or suspense systems. However, Phase 35
subsumes Phase 34's workaround pattern: apps that previously used
effects to derive Bool/String values can now use MemoBool/MemoString
instead.

## Combined estimated size (Phase 33 + 34 + 35)

| Phase | ~New Lines | Tests |
|-------|-----------|-------|
| Phase 33 (Suspense) | ~880 Mojo, ~310 TS | 57 Mojo + 47 JS = 104 |
| Phase 34 (Effects) | ~530 Mojo, ~250 TS | 34 Mojo + 38 JS = 72 |
| Phase 35 (Memo Expansion) | ~1,110 Mojo, ~440 TS | 68 Mojo + 42 JS = 110 |
| **Total** | **~2,520 Mojo, ~1,000 TS** | **159 Mojo + 127 JS = 286 tests** |

---

## Phase 36 — Recursive Memo Propagation

### P36 Problem

When a signal is written, `Runtime.write_signal` marks direct memo
subscribers dirty and then scans the memo's output signal subscribers —
but only checks for **effects** and **scopes**. It does NOT check
whether an output subscriber is another memo. This means memo → memo
chains are broken at the propagation level:

```text
Current behaviour (Phase 35):

    SignalI32 ──write──► MemoI32 (marked dirty ✓)
                              │
                              ▼ output subscribers scanned...
                         MemoBool (subscriber is memo → NOT checked → NOT dirty ✗)
                              │
                              ▼ never reached
                         MemoString (NOT dirty ✗)
```

As a result, apps with memo chains must use a manual workaround:
check whether the head memo is dirty, and if so eagerly recompute
**all** downstream memos in dependency order — even though the
runtime should be able to mark them dirty automatically.

From `MemoChainApp.run_memos()` (Phase 35):

```mojo
# Workaround: if head dirty, recompute entire chain
if not self.doubled.is_dirty():
    return
# Must recompute all three in order — runtime doesn't mark
# downstream memos dirty for memo → memo chains.
self.doubled.begin_compute()
...
self.is_big.begin_compute()    # not marked dirty by runtime
...
self.label.begin_compute()     # not marked dirty by runtime
...
```

### P36 Current state

The propagation logic in `write_signal` (lines ~570–635 of
`src/signals/runtime.mojo`) has three subscriber categories:

1. **Memo subscriber** — mark dirty, then scan memo's output signal
   subscribers for effects and scopes (ONE level only).
2. **Effect subscriber** — mark pending.
3. **Scope subscriber** — add to `dirty_scopes`.

The inner loop after marking a memo dirty (step 1) mirrors the
top-level loop's effect/scope checks but **omits the memo check**.
This is the root cause: the inner loop is not recursive.

Additionally, all three `memo_end_compute_*` methods write the
computed value directly to the `SignalStore` (bypassing
`write_signal`) to avoid re-entrant propagation during computation.
This is correct — by the time a memo recomputes, all downstream
memos should already be marked dirty from the original signal write.
The fix is therefore entirely in `write_signal`'s propagation, not
in end_compute.

### P36 Target pattern

After the fix, `write_signal` recursively marks all downstream
memos dirty through memo → memo chains:

```text
Target behaviour (Phase 36):

    SignalI32 ──write──► MemoI32 (marked dirty ✓)
                              │
                              ▼ output subscribers scanned...
                         MemoBool (memo subscriber → marked dirty ✓)
                              │
                              ▼ output subscribers scanned...
                         MemoString (memo subscriber → marked dirty ✓)
                              │
                              ▼ output subscribers scanned...
                         Scope (added to dirty_scopes ✓)
```

Apps can then check each memo independently:

```mojo
# After fix: each memo checks is_dirty() independently
fn run_memos(mut self):
    if self.doubled.is_dirty():
        self.doubled.begin_compute()
        var i = self.input.read()
        self.doubled.end_compute(i * 2)
    if self.is_big.is_dirty():
        self.is_big.begin_compute()
        var d = self.doubled.read()
        self.is_big.end_compute(d >= 10)
    if self.label.is_dirty():
        self.label.begin_compute()
        var big = self.is_big.read()
        self.label.end_compute(String("BIG") if big else String("small"))
```

Recomputation order still matters (upstream before downstream), but
the app no longer needs to know the chain structure or gate on the
head memo.

### P36 Design

#### Worklist-based propagation

Replace the flat inner loop in `write_signal` with an iterative
worklist that processes memo output subscribers to arbitrary depth.

When a memo subscriber is found and marked dirty, the memo's ID is
added to a worklist. After the top-level subscriber scan completes,
the worklist is drained: for each memo, its output signal subscribers
are scanned for memos (→ mark dirty, add to worklist), effects
(→ mark pending), and scopes (→ add to dirty_scopes).

```text
fn write_signal(key, value):
    signals.write(key, value)
    var memo_worklist = List[UInt32]()

    # Phase 1: scan direct subscribers of the written signal
    for ctx in signals.get_subscribers(key):
        if is_memo(ctx):
            mark_dirty(memo_id)
            memo_worklist.append(memo_id)
        elif is_effect(ctx):
            mark_pending(effect_id)
        else:
            add_dirty_scope(ctx)

    # Phase 2: drain worklist — propagate through memo chains
    while len(memo_worklist) > 0:
        var mid = memo_worklist.pop()
        var out_key = memos.output_key(mid)
        for sub_ctx in signals.get_subscribers(out_key):
            if is_memo(sub_ctx):
                if not memos.is_dirty(downstream_id):
                    memos.mark_dirty(downstream_id)
                    memo_worklist.append(downstream_id)
            elif is_effect(sub_ctx):
                mark_pending(effect_id)
            else:
                add_dirty_scope(sub_ctx)
```

#### Cycle guard

The `is_dirty()` check before adding to the worklist serves as a
cycle guard: a memo that is already dirty is not re-processed.
True cycles (memo A depends on memo B which depends on memo A) are
impossible in practice — `begin_compute` clears old subscriptions
and re-subscribes fresh, and a memo cannot read its own output
during computation (the `computing` flag guards this). The dirty
check prevents infinite loops in degenerate cases.

#### Diamond dependency handling

Diamond patterns (signal → memo A, signal → memo B, A+B → memo C)
work correctly: memo C subscribes to both A's and B's output signals.
When the signal is written:

1. A is marked dirty (direct subscriber) → added to worklist.
2. B is marked dirty (direct subscriber) → added to worklist.
3. Worklist processes A → scans A's output subscribers → finds C
   → C not dirty → marks C dirty → adds C to worklist.
4. Worklist processes B → scans B's output subscribers → finds C
   → C already dirty → skips (cycle guard).
5. Worklist processes C → scans C's output subscribers → effects/scopes.

C is correctly marked dirty exactly once.

#### Scope of change

- **`src/signals/runtime.mojo`**: Refactor `write_signal` inner loop.
  Extract `_notify_subscriber` helper for the memo/effect/scope check
  (used in both Phase 1 and Phase 2 of the propagation). Net change
  is small: the existing inner loop becomes a worklist drain.

- **`src/main.mojo` (MemoChainApp)**: Simplify `run_memos()` to check
  each memo independently.

- **Tests**: New unit tests for chain propagation at the runtime level,
  plus updated MemoChainApp tests.

- **No JS changes**: The TypeScript runtime is unaffected — propagation
  is entirely within the Mojo WASM module.

### P36 Steps

#### P36.1 — Runtime worklist propagation

Refactor `write_signal` in `src/signals/runtime.mojo` to use a
worklist for recursive memo → memo dirty propagation.

##### Mojo changes

###### `src/signals/runtime.mojo` — `write_signal` refactor

Replace the current flat propagation logic with a two-phase approach:

**Phase 1** — Scan direct subscribers of the written signal. For each
subscriber:

- If memo → mark dirty, append to `memo_worklist`.
- If effect → mark pending.
- If scope → add to `dirty_scopes` (dedup).

**Phase 2** — Drain `memo_worklist`. For each memo ID popped:

- Get the memo's output signal key.
- Scan the output signal's subscribers.
- For each subscriber:
  - If memo → check `is_dirty()`. If not dirty, mark dirty and
    append to worklist. If already dirty, skip (cycle guard).
  - If effect → mark pending.
  - If scope → add to `dirty_scopes` (dedup).

The helper logic for classifying a subscriber context ID
(memo vs effect vs scope) is used in both phases. Extract into a
local pattern or inline — Mojo does not support nested functions
in structs, so the classification remains inline but structured
identically in both phases.

```mojo
fn write_signal[
    T: Copyable & ImplicitlyDestructible & AnyType
](mut self, key: UInt32, value: T):
    self.signals.write[T](key, value)

    var memo_worklist = List[UInt32]()
    var subs = self.signals.get_subscribers(key)

    # Phase 1: direct subscribers of the written signal
    for i in range(len(subs)):
        var ctx = subs[i]
        if self._mark_memo_if_subscriber(ctx, memo_worklist):
            continue
        if self._mark_effect_if_subscriber(ctx):
            continue
        self._add_dirty_scope(ctx)

    # Phase 2: drain worklist — propagate through memo chains
    while len(memo_worklist) > 0:
        var mid = memo_worklist.pop()
        if not self.memos.contains(mid):
            continue
        var out_key = self.memos.output_key(mid)
        var out_subs = self.signals.get_subscribers(out_key)
        for k in range(len(out_subs)):
            var sub_ctx = out_subs[k]
            if self._mark_memo_if_subscriber(sub_ctx, memo_worklist):
                continue
            if self._mark_effect_if_subscriber(sub_ctx):
                continue
            self._add_dirty_scope(sub_ctx)
```

Three small private helpers keep the logic DRY:

```mojo
fn _mark_memo_if_subscriber(
    mut self,
    ctx: UInt32,
    mut worklist: List[UInt32],
) -> Bool:
    """If ctx is a memo's reactive context, mark it dirty and
    add to worklist (if not already dirty).  Returns True if
    ctx was a memo subscriber."""
    for m in range(len(self._memo_ctx_ids)):
        if self._memo_ctx_ids[m] == ctx:
            var memo_id = self._memo_ids[m]
            if self.memos.contains(memo_id):
                if not self.memos.is_dirty(memo_id):
                    self.memos.mark_dirty(memo_id)
                    worklist.append(memo_id)
            return True
    return False

fn _mark_effect_if_subscriber(mut self, ctx: UInt32) -> Bool:
    """If ctx is an effect's reactive context, mark it pending.
    Returns True if ctx was an effect subscriber."""
    for e in range(len(self._effect_ctx_ids)):
        if self._effect_ctx_ids[e] == ctx:
            var effect_id = self._effect_ids[e]
            if self.effects.contains(effect_id):
                self.effects.mark_pending(effect_id)
            return True
    return False

fn _add_dirty_scope(mut self, ctx: UInt32):
    """Add ctx to dirty_scopes if not already present."""
    for j in range(len(self.dirty_scopes)):
        if self.dirty_scopes[j] == ctx:
            return
    self.dirty_scopes.append(ctx)
```

**Note on `_mark_memo_if_subscriber`:** The cycle guard is
`not self.memos.is_dirty(memo_id)`. A memo that was ALREADY dirty
(e.g., from a prior write in the same flush, or from Phase 1 in a
diamond) is not re-added to the worklist. This guarantees
termination: each memo is processed at most once per `write_signal`
call.

**Edge case — memo already dirty from Phase 1:** In a diamond
(signal → memo A, signal → memo B, A+B → memo C), Phase 1 does NOT
directly mark C dirty (C subscribes to A's and B's outputs, not to
the signal). Phase 2 processes A → marks C dirty → adds to worklist.
Phase 2 processes B → C already dirty → skips. Correct.

**Edge case — memo marked dirty but memos.contains() is False:**
The memo may have been destroyed between the subscriber registration
and this write. The `contains()` check handles this safely.

##### WASM exports

No new exports needed. The change is internal to `write_signal`.
Existing exports (`mc_doubled_dirty`, `mc_is_big_dirty`,
`mc_label_dirty`, etc.) will now return correct values after a
signal write without requiring manual recomputation.

##### Test: `test/test_memo_propagation.mojo` (~20 tests)

New test module dedicated to recursive propagation scenarios.
Uses the same `WasmInstance` + WASM harness pattern as existing
memo tests.

Tests:

1. **test_chain_2_level** — signal → memo A → memo B. Write signal,
   assert both A and B are dirty.
2. **test_chain_3_level** — signal → memo A → memo B → memo C. Write
   signal, assert all three dirty.
3. **test_chain_4_level** — signal → A → B → C → D. Write signal,
   assert all four dirty.
4. **test_chain_scope_at_end** — signal → memo A → memo B, scope
   subscribes to B's output. Write signal, assert A dirty, B dirty,
   scope in dirty_scopes.
5. **test_chain_effect_at_end** — signal → memo A → memo B, effect
   subscribes to B's output. Write signal, assert A dirty, B dirty,
   effect pending.
6. **test_diamond_2_inputs** — signal → memo A, signal → memo B,
   memo C subscribes to both A and B outputs. Write signal, assert
   all three dirty. C added to worklist only once.
7. **test_diamond_deep** — signal → A → B, signal → C → B (B has two
   parents). Write signal, assert A, B, C all dirty.
8. **test_chain_already_dirty_skip** — signal → A → B. Manually mark
   B dirty before writing signal. Write signal. Assert A dirty,
   B still dirty (no double processing).
9. **test_chain_recompute_clears_dirty** — signal → A → B. Write
   signal (both dirty). Recompute A, recompute B. Assert both clean.
10. **test_chain_recompute_order_matters** — signal → A → B. Write
    signal. Recompute A (writes new output), recompute B (reads A's
    output). Assert B's value reflects A's new output.
11. **test_chain_independent_write** — signal1 → A, signal2 → B,
    A → C. Write signal1, assert A and C dirty but B clean. Write
    signal2, assert B dirty.
12. **test_chain_propagation_after_resubscribe** — signal → A → B.
    Compute both. Recompute A reading a DIFFERENT signal (re-tracking).
    Write original signal, assert A NOT dirty (unsubscribed). Write
    new signal, assert A dirty, B dirty.
13. **test_chain_with_destroyed_memo** — signal → A → B. Destroy B.
    Write signal. Assert A dirty, no crash (B's slot is vacant).
14. **test_chain_mixed_types** — signal (Int32) → MemoI32 → MemoBool
    → MemoString. Write signal, assert all three dirty. Mirrors
    MemoChainApp topology.
15. **test_chain_string_memo_at_end** — signal → MemoI32 → MemoString.
    Write signal, assert both dirty. Recompute both, assert
    MemoString has correct value.
16. **test_chain_bool_memo_in_middle** — signal → MemoBool → MemoI32.
    Write signal, assert both dirty.
17. **test_chain_no_subscribers** — signal → memo A (no subscribers on
    A's output). Write signal, assert A dirty, no crash.
18. **test_chain_memo_to_memo_and_scope** — signal → A, scope and
    memo B both subscribe to A's output. Write signal, assert A
    dirty, B dirty, scope dirty.
19. **test_chain_memo_to_memo_and_effect** — signal → A, effect and
    memo B both subscribe to A's output. Write signal, assert A
    dirty, B dirty, effect pending.
20. **test_regression_single_memo** — signal → memo (no chain).
    Write signal, assert memo dirty, scope dirty. Verifies the
    refactored code doesn't break the existing single-level case.

#### P36.2 — Simplify MemoChainApp

With recursive propagation, `MemoChainApp.run_memos()` no longer
needs to gate on the head memo. Each memo checks `is_dirty()`
independently.

##### Mojo changes

###### `src/main.mojo` — `MemoChainApp.run_memos()`

Replace the current "if head dirty, recompute all" pattern:

```mojo
# Before (Phase 35):
fn run_memos(mut self):
    if not self.doubled.is_dirty():
        return
    self.doubled.begin_compute()
    var i = self.input.read()
    self.doubled.end_compute(i * 2)
    self.is_big.begin_compute()
    var d = self.doubled.read()
    self.is_big.end_compute(d >= 10)
    self.label.begin_compute()
    var big = self.is_big.read()
    if big:
        self.label.end_compute(String("BIG"))
    else:
        self.label.end_compute(String("small"))
```

With independent checks:

```mojo
# After (Phase 36):
fn run_memos(mut self):
    if self.doubled.is_dirty():
        self.doubled.begin_compute()
        var i = self.input.read()
        self.doubled.end_compute(i * 2)
    if self.is_big.is_dirty():
        self.is_big.begin_compute()
        var d = self.doubled.read()
        self.is_big.end_compute(d >= 10)
    if self.label.is_dirty():
        self.label.begin_compute()
        var big = self.is_big.read()
        if big:
            self.label.end_compute(String("BIG"))
        else:
            self.label.end_compute(String("small"))
```

The recomputation order (doubled → is_big → label) is still
maintained by code order, but the gating logic is per-memo rather
than all-or-nothing from the head.

Remove the doc comment paragraph about the runtime limitation and
the "eagerly recompute all three" rationale — that limitation is
now fixed.

##### Test updates

Update `test/test_memo_chain.mojo` and `test-js/memo_chain.test.ts`
to add tests verifying that individual memos in the chain are
independently dirty after a signal write:

- **test_chain_all_memos_dirty_after_increment** — After mc_handle_event
  (increment), assert mc_doubled_dirty, mc_is_big_dirty, AND
  mc_label_dirty are all 1. (In Phase 35, only mc_doubled_dirty was
  guaranteed; the others relied on the app's explicit recomputation.)
- **test_chain_partial_recompute** — After increment, recompute only
  doubled (begin/end compute). Assert doubled clean, is_big still
  dirty, label still dirty. Then recompute is_big. Assert is_big
  clean, label still dirty. Then recompute label. Assert all clean.

Existing tests continue to pass unchanged — the observable behaviour
(final derived state after flush) is identical.

#### P36.3 — Documentation & AGENTS.md update

##### Changes

**AGENTS.md:**

- **Common Patterns:** Update "Memo type expansion pattern" to remove
  the "runtime does not recursively propagate" caveat. Replace with
  "the runtime automatically propagates dirtiness through memo → memo
  chains (Phase 36); apps still recompute in dependency order but no
  longer need to gate on the head memo."
- **MemoChainApp architecture:** Update the `run_memos` pseudocode
  to show independent `is_dirty()` checks. Remove the note about
  runtime propagation limitation.
- **Common Patterns:** Add "Worklist-based memo propagation" pattern
  describing the two-phase approach.
- **File Size Reference:** Update line counts for `runtime.mojo` and
  `main.mojo`.

**CHANGELOG.md:**

- Add Phase 36 entry summarizing P36.1 (runtime), P36.2 (app
  simplification), P36.3 (docs). Include test count delta.

**README.md:**

- Update test count.
- Update the MemoChainApp code example in Ergonomic API to show
  independent `is_dirty()` checks.
- Add note to "Memo type expansion" section that recursive
  propagation is now automatic.

### P36 Dependency graph

```text
P36.1 (Runtime propagation)
    │
    ▼
P36.2 (Simplify MemoChainApp)
    │
    ▼
P36.3 (Docs)
```

Strictly sequential: the runtime fix must land before the app can
be simplified, and both must be complete before documentation.

### P36 Estimated size

| Step | ~Changed Mojo Lines | ~New Mojo Lines | ~New TS Lines | Tests |
|------|--------------------|-----------------| --------------|-------|
| P36.1 (runtime + unit tests) | ~60 (refactor) | ~550 | ~0 | 20 Mojo |
| P36.2 (MemoChainApp simplification) | ~30 (simplify) | ~40 | ~20 | 2 Mojo + 2 JS |
| P36.3 (docs) | ~0 | ~60 | ~0 | 0 |
| **Total** | **~90** | **~650** | **~20** | **22 Mojo + 2 JS = 24 tests** |

## Phase 37 — Equality-Gated Memo Propagation

### P37 Problem

Phase 36 made memo → memo dirty propagation correct by eagerly
marking all transitive downstream memos dirty whenever a source
signal is written. This is correct but **over-notifying**: every
downstream memo, effect, and scope in the chain is marked dirty
even when an intermediate memo recomputes to the **same value**.

```text
Current behaviour (Phase 36):

    signal count = 5
    memo is_positive = count > 0        // true
    memo label = is_positive ? "+" : "-" // "+"

    count.set(10)
      → is_positive marked dirty  ✓ (correct)
      → label marked dirty        ✗ (wasteful — is_positive is still true)
      → scope marked dirty        ✗ (wasteful — label will be "+" again)

    flush:
      is_positive recomputes → true (unchanged)
      label recomputes → "+" (unchanged)
      scope re-renders → identical VNode → diff finds no mutations
```

The scope re-renders and diffs for nothing. In a real app with N
downstream dependents of a stable intermediate memo, this wastes
O(N) recomputation and diffing work every flush cycle.

This is the classic "glitch" problem in reactive systems. SolidJS,
Vue, and Dioxus all solve it with equality checking at the memo
boundary: if a memo recomputes to the same value, don't propagate
further.

### P37 Current state

Phase 36's `write_signal` uses a two-phase worklist to propagate
dirtiness eagerly at signal-write time:

1. **Phase 1** — Scan direct subscribers of the written signal:
   memos → dirty + worklist; effects → pending; scopes → dirty.
2. **Phase 2** — Drain worklist: for each memo, scan its output
   signal's subscribers with the same classification.

This is a **push-based eager** model: all transitive dirtiness is
resolved immediately when the signal is written.

The `memo_end_compute_*` methods write computed values directly to
the `SignalStore` (bypassing `write_signal` to avoid re-entrant
propagation). They clear the dirty flag and restore the reactive
context — but they do NOT check whether the new value differs from
the old one.

The key insight is that `write_signal`'s eager propagation is a
**pessimistic upper bound**: it marks everything that *could* be
dirty. The equality check at recomputation time provides the
**optimistic refinement**: it cancels dirtiness for subgraphs where
the value didn't actually change.

### P37 Target pattern

After the fix, `memo_end_compute_*` checks whether the new value
equals the old value. If unchanged, it clears dirty but does NOT
propagate — and it **un-dirties** downstream subscribers that were
eagerly marked by `write_signal`:

```text
Target behaviour (Phase 37):

    signal count = 5
    memo is_positive = count > 0        // true
    memo label = is_positive ? "+" : "-" // "+"

    count.set(10)
      → is_positive marked dirty  ✓ (eager — may need recompute)
      → label marked dirty        ✓ (eager — may need recompute)
      → scope marked dirty        ✓ (eager — may need re-render)

    flush (recompute in dependency order):
      is_positive recomputes → true == true (unchanged!)
        → label un-dirtied (equality gate cancels downstream)
        → scope un-dirtied (equality gate cancels downstream)
      label skipped (no longer dirty)
      scope skipped (no longer dirty)
      → zero mutations emitted
```

For apps like MemoChainApp, this means:

```mojo
# Phase 37: equality-gated — if doubled doesn't change,
# is_big and label skip recomputation entirely.
fn run_memos(mut self):
    if self.doubled.is_dirty():
        self.doubled.begin_compute()
        var i = self.input.read()
        self.doubled.end_compute(i * 2)
        # If doubled computed the same value → is_big and label
        # are automatically un-dirtied by the runtime.

    if self.is_big.is_dirty():       # skipped if doubled unchanged
        self.is_big.begin_compute()
        var d = self.doubled.read()
        self.is_big.end_compute(d >= 10)

    if self.label.is_dirty():        # skipped if is_big unchanged
        self.label.begin_compute()
        var big = self.is_big.read()
        self.label.end_compute(String("BIG") if big else String("small"))
```

No app-level code changes required — the equality gate is entirely
inside `memo_end_compute_*`.

### P37 Design

#### Equality checking in `end_compute`

Each `memo_end_compute_*` variant reads the old cached value from
the output signal before writing the new one. If old == new, the
memo is **value-stable**: its output didn't change despite its
inputs being dirty.

For the three memo types:

| Type | Comparison | Storage |
|------|-----------|---------|
| `MemoI32` | `old_i32 == new_i32` | `signals.read[Int32](output_key)` |
| `MemoBool` | `old_i32 == new_i32` (both stored as Int32 0/1) | `signals.read[Int32](output_key)` |
| `MemoString` | `old_string == new_string` | `strings.read(string_key)` |

The comparison is cheap: Int32 equality is a single instruction,
and String equality short-circuits on length before comparing bytes.

#### Un-dirtying downstream subscribers

When a memo is value-stable, its output signal's subscribers were
eagerly marked dirty by `write_signal` (Phase 36 worklist) but
should NOT remain dirty. The runtime must **cancel** their dirtiness.

The cancellation is a mirror of Phase 36's propagation: scan the
stable memo's output signal subscribers and for each:

- **Memo subscriber** → if dirty AND all of its input signals'
  source memos are clean or value-stable, clear dirty. Add to an
  **un-dirty worklist** to propagate cancellation further.
- **Effect subscriber** → if pending AND all triggering memos are
  stable, clear pending.
- **Scope subscriber** → remove from `dirty_scopes`.

The "all inputs stable" check is critical for diamond dependencies:

```text
Diamond: signal → memo A, signal → memo B, A+B → memo C

Case 1: A stable, B changed → C stays dirty (correct)
Case 2: A stable, B stable  → C un-dirtied (correct)
Case 3: A changed, B stable → C stays dirty (correct)
```

#### Conservative un-dirtying (no "all inputs" check)

The "all inputs stable" check is complex: it requires knowing all
of a memo's input memos and whether each has been recomputed yet in
this flush cycle. This is problematic because:

1. Memos don't explicitly track their input memos — dependencies
   are recorded as subscriber relationships on signals, not as
   memo → memo edges.
2. During flush, memos are recomputed in app-defined order, so some
   upstream memos may not have been recomputed yet when a downstream
   memo's un-dirtying is attempted.

A simpler and fully correct approach: **do NOT un-dirty downstream
memos eagerly. Instead, let the recomputation order handle it.**

The key observation: apps already recompute memos in dependency
order (upstream before downstream). If an upstream memo is
value-stable, we skip writing its output signal, so when the
downstream memo recomputes, it reads the **same old value** and
will itself be value-stable (if its computation is pure). The
equality check cascades naturally through the recomputation order.

But this only saves the work of computing the downstream memo's
body — it doesn't prevent the downstream memo from being dirty in
the first place. The real win is skipping the **scope re-render and
diff** when no memo output changed.

#### Hybrid approach: selective un-dirtying

The practical design:

1. **`memo_end_compute_*` checks equality.** If value-stable:
   - Clear dirty (as before).
   - Call `_cancel_memo_dependents(memo_id)` to un-dirty
     **direct downstream memos and effects only** (one level).
   - Remove subscribed scopes from `dirty_scopes`.

2. **`_cancel_memo_dependents` does NOT recurse.** It only processes
   direct subscribers of the stable memo's output signal. This is
   safe because:
   - If a downstream memo is un-dirtied here but has OTHER dirty
     sources, it will be re-dirtied when those sources' memos
     recompute (they call `end_compute` which writes the output
     signal, triggering subscriber notification).
   - Wait — `end_compute` writes directly to `SignalStore`, not via
     `write_signal`, so it does NOT re-notify subscribers. This is
     by design (Phase 36).

   Therefore, one-level un-dirtying is NOT safe for diamonds where
   one parent changed and one didn't: the changed parent's
   `end_compute` writes a new value but doesn't re-dirty the
   downstream memo (because `end_compute` bypasses `write_signal`).

3. **Resolution: recomputation-time re-dirtying.** When a memo
   `end_compute` writes a **changed** value, it must re-dirty the
   downstream memos that may have been un-dirtied by a stable
   sibling. This means `end_compute` must:
   - If value **changed**: propagate dirtiness to output subscribers
     (mirrors `write_signal` Phase 2 logic, but only one level).
   - If value **unchanged**: cancel dirtiness for output subscribers
     (un-dirty one level).

This is the correct and complete design:

```text
memo_end_compute(memo_id, new_value):
    old_value = read output signal
    write new_value to output signal
    clear_dirty(memo_id)

    if old_value == new_value:
        # Value-stable: cancel downstream dirtiness
        for sub in output_signal.subscribers:
            if is_memo(sub) and is_dirty(sub):
                clear_dirty(sub)       # un-dirty
            if is_effect(sub) and is_pending(sub):
                clear_pending(sub)     # un-pending
            if is_scope(sub):
                remove from dirty_scopes
    else:
        # Value changed: re-dirty downstream (in case a sibling
        # stable memo un-dirtied them)
        for sub in output_signal.subscribers:
            if is_memo(sub) and not is_dirty(sub):
                mark_dirty(sub)        # re-dirty
            if is_effect(sub) and not is_pending(sub):
                mark_pending(sub)      # re-pending
            if is_scope(sub) and not in dirty_scopes:
                add to dirty_scopes
```

Wait — Phase 36's `write_signal` already dirtied everything eagerly.
If A is stable and un-dirties C, then B (which changed) must
re-dirty C. But B's `end_compute` runs AFTER A's `end_compute` only
if the app recomputes in the right order. Since memos are
recomputed in dependency order (upstream first), and A and B are
siblings (both direct children of the signal), their order is
defined by code order in `run_memos()`. If A runs first and
un-dirties C, then B runs and re-dirties C, this is correct. If B
runs first (writes changed value, C was already dirty from Phase 36,
no-op), then A runs and un-dirties C — WRONG! C should still be
dirty because B changed.

This shows that **one-level un-dirtying is order-dependent for
diamonds**. The fix: A's un-dirty must check whether C has any OTHER
dirty source memos before un-dirtying. But this requires the
"all inputs stable" check we wanted to avoid.

#### Final design: lazy equality (pull-based)

The simplest correct design avoids un-dirtying entirely and instead
uses **lazy equality** at the consumer side:

1. `write_signal` eagerly marks all transitive memos dirty (Phase 36,
   unchanged).

2. `memo_end_compute_*` checks equality. Stores a `value_changed`
   flag on the memo entry.
   - If value **unchanged**: set `value_changed = False`.
   - If value **changed**: set `value_changed = True`.
   In both cases, clear `dirty` and write the value as before.

3. In the flush loop, after recomputing all memos, add a
   **settle pass**: walk the memo chain again and check which
   scopes/effects actually need updating based on whether their
   triggering memos had `value_changed = True`.

Actually, this is still complex. Let's use the simplest correct
approach:

#### Simplest correct design: skip-if-unchanged in `end_compute`

The minimal and fully correct design:

1. `write_signal` remains unchanged (Phase 36 eager propagation).

2. Each `memo_end_compute_*` variant compares old vs new value:
   - If **unchanged**: clear dirty, restore context, but **do NOT
     write to the output signal**. The output signal retains its
     old value (which is the same). Since the output signal is not
     written, and `end_compute` bypasses `write_signal`, no
     subscribers are notified. The downstream memos remain dirty
     from the Phase 36 eager propagation, but when they recompute,
     they will read the same old value from this memo's output
     signal and will themselves be value-stable (if pure). The
     equality check cascades through the chain.
   - If **changed**: write to the output signal as before. Clear
     dirty, restore context. No additional propagation needed — the
     eager Phase 36 propagation already marked everything dirty.

3. **Scope un-dirtying**: After all memos are recomputed, if every
   memo that a scope subscribes to was value-stable, the scope's
   re-render will produce an identical VNode, and the diff will
   emit zero mutations. This is wasted work but is a no-op in
   practice (the diff is fast on identical VNodes).

   For the full optimization (skip scope re-render entirely), add
   a **settle check** after `run_memos()`: for each dirty scope,
   check whether any of its triggering signals actually changed
   version. If not, remove the scope from the dirty set. This is
   Phase 37.2.

4. **Effect un-pending**: Same principle — if the memo that an
   effect reads was value-stable, the effect reads the same value
   and its body is idempotent (presumably). But effects always
   run when pending, so for correctness we let them run. A future
   optimization could skip effects whose inputs are all stable.

This design is:

- **Correct for diamonds**: All downstream memos are dirty (from
  Phase 36). They all recompute. If one parent changed and one
  didn't, the downstream memo reads both outputs and computes a
  potentially new value — equality check determines propagation.
- **Correct for chains**: Each memo in order checks equality. If
  stable, doesn't write output. Next memo reads old value, is also
  stable. Cascades to the end.
- **No un-dirtying logic**: Avoids the complex cancellation problem.
  The "optimization" is simply skipping the output signal write,
  which means downstream memos see the same value when they
  recompute.
- **Scope/effect savings**: The real CPU savings come from the
  scope settle check (P37.2) which removes scopes from the dirty
  set if their memo inputs were all value-stable.

#### Scope of change

- **`src/signals/memo.mojo`**: Add `value_changed: Bool` field to
  `MemoEntry`. Default `True` (first compute always "changed").
  Add `set_value_changed()` and `did_value_change()` accessors.

- **`src/signals/runtime.mojo`**: Modify `memo_end_compute_i32`,
  `memo_end_compute_bool`, `memo_end_compute_string` to compare
  old vs new value before writing. Set `value_changed` accordingly.
  If unchanged, skip the output signal write.

  Add `settle_scopes()` method that walks `dirty_scopes` and removes
  entries where all subscribing memos are value-stable (i.e.,
  `did_value_change() == False`). This requires a reverse lookup:
  for each scope, find which memos' output signals the scope
  subscribes to, and check `value_changed` on each.

  Simpler approach for `settle_scopes()`: walk dirty_scopes, for
  each scope check if the scope's subscribed signals have changed
  version since the last flush. Actually, the simplest correct
  approach: for each dirty scope, check whether ANY signal it
  subscribes to has a bumped version. If the scope subscribes to a
  memo output signal that was NOT written (value-stable), that
  signal's version is unchanged. If ALL subscribed signals have
  unchanged versions, the scope can be un-dirtied.

  Even simpler: track a `_changed_signals` set in the runtime during
  the flush cycle. Each `memo_end_compute_*` that writes a changed
  value adds the output_key to `_changed_signals`. Each
  `write_signal` adds the written key. `settle_scopes()` checks
  whether any of the scope's subscribed signals are in
  `_changed_signals`.

  Simplest of all: add a `flush_generation: UInt32` to the runtime,
  bumped each flush. Add `last_change_gen: UInt32` to `SignalEntry`.
  `write_signal` and value-changed `end_compute` set
  `last_change_gen = flush_generation`. `settle_scopes()` checks
  the scope's subscribed signals' `last_change_gen`.

  For Phase 37, we'll use the `_changed_signals: List[UInt32]`
  approach as it's the most straightforward.

- **`src/main.mojo`**: Apps call `settle_scopes()` after
  `run_memos()` in their flush functions. Add WASM export
  `runtime_settle_scopes()`.

  Alternatively, `settle_scopes()` is called automatically inside
  `consume_dirty()` on the next flush, but that's too late — dirty
  scopes are already consumed. Better: apps call it explicitly
  between `run_memos()` and `render()`.

  Best integration point: add it to `ComponentContext.consume_dirty()`
  as an optional step, or provide a separate
  `ctx.settle_after_memos()` method. For minimum disruption, expose
  it as a runtime method and let apps opt in.

  Actually, the cleanest integration: modify the existing
  `_mc_flush` / `_em_flush` pattern to call settle after memos:

  ```text
  fn flush():
      if not ctx.consume_dirty(): return 0
      run_memos()
      ctx.settle_scopes()    # NEW: remove scopes with no actual changes
      run_effects()          # effects still run if pending
      var new_idx = render()
      ctx.diff(writer, new_idx)
      return ctx.finalize(writer)
  ```

- **`src/component/context.mojo`**: Add `settle_scopes()` that
  forwards to `runtime.settle_scopes()`.

- **Tests**: New unit tests for equality checking at each memo type,
  chain cascading, diamond correctness, scope settle, and effect
  interaction. New EqualityDemoApp to demonstrate the optimization.

- **No JS changes**: The TypeScript runtime is unaffected.

### P37 Steps

#### P37.1 — MemoEntry `value_changed` flag and equality checking

Add the `value_changed` flag to `MemoEntry` and implement equality
comparison in all three `memo_end_compute_*` variants.

##### Mojo changes

###### `src/signals/memo.mojo` — `MemoEntry` extension

Add a `value_changed` field to `MemoEntry`:

```mojo
struct MemoEntry(Copyable, Equatable, Writable):
    var context_id: UInt32
    var output_key: UInt32
    var string_key: UInt32
    var scope_id: UInt32
    var dirty: Bool
    var computing: Bool
    var value_changed: Bool     # NEW: True if last end_compute wrote a different value
```

Default to `True` in all constructors (first computation is always
treated as a change — the initial value is "new" relative to the
uninitialized state).

Add accessors to `MemoStore`:

```mojo
fn set_value_changed(mut self, id: UInt32, changed: Bool):
    """Set the value_changed flag after end_compute."""
    ...

fn did_value_change(self, id: UInt32) -> Bool:
    """Check whether the last end_compute changed the value."""
    ...
```

Update all `__init__`, `__copyinit__`, `__moveinit__` to include
`value_changed`.

###### `src/signals/runtime.mojo` — equality-gated `end_compute`

Modify `memo_end_compute_i32`:

```mojo
fn memo_end_compute_i32(mut self, memo_id: UInt32, value: Int32):
    if not self.memos.contains(memo_id):
        return
    var entry = self.memos.get(memo_id)
    # Equality check: compare old vs new
    var old_value = self.signals.read[Int32](entry.output_key)
    var changed = (old_value != value)
    if changed:
        self.signals.write[Int32](entry.output_key, value)
    self.memos.set_value_changed(memo_id, changed)
    self.memos.clear_dirty(memo_id)
    self.memos.set_computing(memo_id, False)
    # Restore previous context
    var prev = Int(self.signals.read[Int32](entry.context_id))
    self.current_context = prev
```

Modify `memo_end_compute_bool`:

```mojo
fn memo_end_compute_bool(mut self, memo_id: UInt32, value: Bool):
    if not self.memos.contains(memo_id):
        return
    var entry = self.memos.get(memo_id)
    var val_i32: Int32
    if value:
        val_i32 = Int32(1)
    else:
        val_i32 = Int32(0)
    # Equality check
    var old_value = self.signals.read[Int32](entry.output_key)
    var changed = (old_value != val_i32)
    if changed:
        self.signals.write[Int32](entry.output_key, val_i32)
    self.memos.set_value_changed(memo_id, changed)
    self.memos.clear_dirty(memo_id)
    self.memos.set_computing(memo_id, False)
    var prev = Int(self.signals.read[Int32](entry.context_id))
    self.current_context = prev
```

Modify `memo_end_compute_string`:

```mojo
fn memo_end_compute_string(mut self, memo_id: UInt32, value: String):
    if not self.memos.contains(memo_id):
        return
    var entry = self.memos.get(memo_id)
    # Equality check: compare old string vs new string
    var old_value = self.strings.read(entry.string_key)
    var changed = (old_value != value)
    if changed:
        self.strings.write(entry.string_key, value)
        # Bump version signal only when string actually changed
        var ver = self.peek_signal[Int32](entry.output_key)
        self.signals.write[Int32](entry.output_key, ver + 1)
    self.memos.set_value_changed(memo_id, changed)
    self.memos.clear_dirty(memo_id)
    self.memos.set_computing(memo_id, False)
    var prev = Int(self.signals.read[Int32](entry.context_id))
    self.current_context = prev
```

Key detail for `MemoString`: the version signal is only bumped when
the string actually changes. This is important because downstream
subscribers track the version signal, not the string itself. If the
version doesn't bump, downstream memos that `read()` the string
memo will see the same version and won't be re-subscribed — but
they were already eagerly marked dirty by Phase 36, so this doesn't
affect correctness. The equality cascade works because the downstream
memo reads the same string value and will itself be value-stable.

Add `_changed_signals` tracking list to `Runtime`:

```mojo
var _changed_signals: List[UInt32]    # signal keys changed this flush cycle
```

In `write_signal`, after writing the value, add the key:

```mojo
fn write_signal[T: ...](mut self, key: UInt32, value: T):
    self.signals.write[T](key, value)
    self._changed_signals.append(key)
    # ... existing propagation ...
```

In each `memo_end_compute_*`, when `changed` is True, add the
output key:

```mojo
if changed:
    self.signals.write[Int32](entry.output_key, value)
    self._changed_signals.append(entry.output_key)
```

Add `clear_changed_signals()` to reset at the start of each flush:

```mojo
fn clear_changed_signals(mut self):
    """Reset the changed-signals set.  Call at the start of each flush."""
    self._changed_signals = List[UInt32]()
```

Add a WASM export `runtime_clear_changed_signals(rt_ptr)` (or
integrate into `consume_dirty`). Best approach: call it
automatically at the start of `drain_dirty()`:

```mojo
fn drain_dirty(mut self) -> List[UInt32]:
    self._changed_signals = List[UInt32]()  # reset for this cycle
    var result = self.dirty_scopes^
    self.dirty_scopes = List[UInt32]()
    return result^
```

Add a query method:

```mojo
fn signal_changed_this_cycle(self, key: UInt32) -> Bool:
    """Check whether a signal was written with a new value this flush."""
    for i in range(len(self._changed_signals)):
        if self._changed_signals[i] == key:
            return True
    return False
```

Add `memo_did_value_change(memo_id) -> Bool` WASM export forwarding
to `memos.did_value_change(memo_id)`.

##### WASM exports (in `src/main.mojo`)

```mojo
@export
fn memo_did_value_change(rt_ptr: Int64, memo_id: Int32) -> Int32:
    """Check whether the last end_compute changed the memo's value."""
    var rt = _get[Runtime](rt_ptr)
    if rt[0].memos.did_value_change(UInt32(memo_id)):
        return Int32(1)
    return Int32(0)
```

##### Test: `test/test_memo_equality.mojo` (~22 tests)

New test module for equality-gated memo behaviour. Uses the same
`WasmInstance` + WASM harness pattern.

Tests:

1. **test_i32_same_value_no_change** — Create memo, compute with
   value 42, compute again with 42. Assert `did_value_change` is
   False on second compute.
2. **test_i32_different_value_changed** — Compute with 42, then 43.
   Assert `did_value_change` is True.
3. **test_i32_initial_compute_changed** — First compute (value 0,
   initial is 0). Assert `did_value_change` is True (first compute
   always treated as change).
4. **test_bool_same_value_no_change** — MemoBool compute True, then
   True again. Assert not changed.
5. **test_bool_different_value_changed** — MemoBool compute True,
   then False. Assert changed.
6. **test_bool_false_to_false_no_change** — MemoBool False → False.
   Assert not changed.
7. **test_string_same_value_no_change** — MemoString compute "hello",
   then "hello" again. Assert not changed.
8. **test_string_different_value_changed** — MemoString "hello" then
   "world". Assert changed.
9. **test_string_empty_to_empty_no_change** — MemoString "" → "".
   Assert not changed.
10. **test_string_version_not_bumped_when_stable** — MemoString
    compute "hello" then "hello". Read the version signal before and
    after. Assert version unchanged.
11. **test_string_version_bumped_when_changed** — MemoString "hello"
    then "world". Assert version bumped.
12. **test_chain_cascade_stable** — signal(5) → memo_a(×2=10) →
    memo_b(>0 = true). Write signal(5) (same value via
    `write_signal`). Recompute memo_a → 10 == 10, stable. Recompute
    memo_b → true == true, stable. Assert both `did_value_change`
    False.
13. **test_chain_cascade_changed** — signal(5) → memo_a(×2) →
    memo_b(>10). Initial: a=10, b=false. Write signal(6). Recompute
    a → 12 (changed). Recompute b → true (changed). Assert both
    `did_value_change` True.
14. **test_chain_partial_cascade** — signal(5) → memo_a(×2=10) →
    memo_b(≥10 = true). Write signal(6). Recompute a → 12 (changed).
    Recompute b → true (still true, stable). Assert a changed, b not
    changed.
15. **test_diamond_one_parent_changed** — signal → memo_a(×2),
    signal → memo_b(+0, always same). Both feed memo_c(a + b).
    Write signal(new value). Recompute a (changed), b (stable), c.
    Assert c is dirty and recomputes correctly.
16. **test_diamond_both_parents_stable** — signal → memo_a, memo_b.
    Write same value to signal. Recompute a (stable), b (stable).
    Assert c's inputs are unchanged, c recomputes to same value,
    c is stable.
17. **test_diamond_both_parents_changed** — Write different value.
    Recompute a (changed), b (changed), c (changed). Assert all
    `did_value_change` True.
18. **test_changed_signals_tracking** — Write signal, assert it
    appears in changed_signals. Memo end_compute with changed value,
    assert output_key in changed_signals. Memo end_compute with
    same value, assert output_key NOT in changed_signals.
19. **test_changed_signals_reset_on_drain** — Write signal, drain
    dirty, assert changed_signals is empty (reset happened).
20. **test_mixed_type_chain_cascade** — signal(I32) → MemoI32 →
    MemoBool → MemoString. Write same value. Recompute chain.
    Assert all three stable.
21. **test_mixed_type_chain_partial** — signal(I32=5) → doubled(10)
    → is_big(true) → label("BIG"). Write signal(6) → doubled(12,
    changed) → is_big(true, stable) → label("BIG", stable). Assert
    doubled changed, is_big stable, label stable.
22. **test_regression_changed_flag_reset** — Compute memo (changed),
    then recompute with same value (stable), then recompute with
    different value (changed). Assert flag toggles correctly across
    multiple recomputations.

#### P37.2 — Scope settle pass

Add `settle_scopes()` to the runtime, which removes dirty scopes
whose subscribed signals were all value-stable (not in
`_changed_signals`).

##### Mojo changes

###### `src/signals/runtime.mojo` — `settle_scopes()`

```mojo
fn settle_scopes(mut self):
    """Remove dirty scopes whose subscribed signals all have unchanged values.

    After memo recomputation, some scopes may have been eagerly marked
    dirty (Phase 36) but none of their subscribed signals actually
    changed value.  This method checks each dirty scope and removes
    those where no subscribed signal is in _changed_signals.

    Call after run_memos() and before render() to skip unnecessary
    re-renders.
    """
    var settled = List[UInt32]()
    for i in range(len(self.dirty_scopes)):
        var scope_id = self.dirty_scopes[i]
        var any_changed = False
        # Check all signals — if this scope subscribes to any signal
        # that changed this cycle, keep it dirty.
        for s in range(len(self.signals._entries)):
            if s < len(self.signals._states) and self.signals._states[s].occupied:
                var subs = self.signals._entries[s].subscribers
                var tagged = scope_id | SCOPE_CONTEXT_TAG
                for k in range(len(subs)):
                    if subs[k] == tagged:
                        # This scope subscribes to signal s.
                        # Check if signal s changed this cycle.
                        if self.signal_changed_this_cycle(UInt32(s)):
                            any_changed = True
                            break
                if any_changed:
                    break
        if not any_changed:
            settled.append(scope_id)
    # Remove settled scopes from dirty_scopes
    var new_dirty = List[UInt32]()
    for i in range(len(self.dirty_scopes)):
        var sid = self.dirty_scopes[i]
        var is_settled = False
        for j in range(len(settled)):
            if settled[j] == sid:
                is_settled = True
                break
        if not is_settled:
            new_dirty.append(sid)
    self.dirty_scopes = new_dirty^
```

Note: This scans all signals × dirty scopes, which is O(S × D)
where S = total signals, D = dirty scopes. Both are small in
practice (S < 100, D < 10 for typical apps). For production use,
a more efficient reverse index (scope → subscribed signals) could
be maintained, but for Phase 37 the linear scan is acceptable and
avoids adding new data structures.

Actually, a more efficient approach: for each dirty scope, check
only the signals that the scope actually subscribes to. We can find
those by scanning signal subscribers for the tagged scope ID. But
this is still O(S) per scope.

Better: maintain a `_scope_subscriptions: Dict[UInt32, List[UInt32]]`
mapping scope_id → list of signal keys it subscribes to. But Mojo
doesn't have Dict, and adding a parallel data structure is complex.

For Phase 37, use the O(S × D) scan. It runs once per flush and
is bounded by the (small) number of signals and dirty scopes in
typical applications. Profile in Phase 38 if needed.

Alternative simpler approach: instead of scanning all signals,
scan only `_changed_signals` and check if any dirty scope subscribes
to a changed signal:

```mojo
fn settle_scopes(mut self):
    """Remove dirty scopes that don't subscribe to any changed signal."""
    if len(self._changed_signals) == 0:
        # No signals changed → all scopes are settled
        self.dirty_scopes = List[UInt32]()
        return
    # Build set of scopes that subscribe to changed signals
    var keep = List[UInt32]()
    for i in range(len(self._changed_signals)):
        var sig_key = self._changed_signals[i]
        if Int(sig_key) >= len(self.signals._entries):
            continue
        if not self.signals._states[Int(sig_key)].occupied:
            continue
        var subs = self.signals._entries[Int(sig_key)].subscribers
        for k in range(len(subs)):
            var ctx = subs[k]
            if (ctx & SCOPE_CONTEXT_TAG) != 0:
                var scope_id = ctx & ~SCOPE_CONTEXT_TAG
                # Check if this scope is in dirty_scopes
                for d in range(len(self.dirty_scopes)):
                    if self.dirty_scopes[d] == scope_id:
                        # Keep this scope dirty
                        var already = False
                        for j in range(len(keep)):
                            if keep[j] == scope_id:
                                already = True
                                break
                        if not already:
                            keep.append(scope_id)
                        break
    self.dirty_scopes = keep^
```

This is O(C × avg_subscribers × D) where C = changed signals (small),
avg_subscribers is typically 1–3, and D = dirty scopes (small).
Much more efficient than scanning all signals.

Use this approach.

###### `src/component/context.mojo` — `settle_scopes()` wrapper

```mojo
fn settle_scopes(mut self):
    """Remove dirty scopes with no actual signal changes.

    Call after run_memos() to skip re-renders for scopes whose
    memo inputs were value-stable.
    """
    self.runtime()[0].settle_scopes()
```

Where `runtime()` returns the runtime pointer (already available
via the AppShell or however ComponentContext accesses the runtime).

###### `src/component/app_shell.mojo` — `settle_scopes()` wrapper

```mojo
fn settle_scopes(mut self):
    """Remove dirty scopes with no actual signal changes."""
    self.rt[0].settle_scopes()
```

##### WASM exports (in `src/main.mojo`)

```mojo
@export
fn runtime_settle_scopes(rt_ptr: Int64):
    """Remove dirty scopes whose signals didn't actually change."""
    _get[Runtime](rt_ptr)[0].settle_scopes()
```

For app-level exports, each app that has memos can expose a settle
step. But since `settle_scopes` operates on the runtime directly
and apps already share the runtime through ComponentContext, apps
just call `ctx.settle_scopes()` in their flush functions. No
per-app WASM export needed.

##### Test: `test/test_scope_settle.mojo` (~16 tests)

New test module for scope settling behaviour.

Tests:

1. **test_settle_removes_scope_when_no_change** — signal → memo,
   scope subscribes to memo output. Write same value to signal.
   Recompute memo (stable). Call settle_scopes. Assert dirty_scopes
   is empty.
2. **test_settle_keeps_scope_when_changed** — Write different value.
   Recompute memo (changed). Settle. Assert scope still dirty.
3. **test_settle_mixed_scopes** — scope_a subscribes to stable memo,
   scope_b subscribes to changed memo. Settle. Assert scope_a
   removed, scope_b kept.
4. **test_settle_scope_subscribes_to_signal** — scope subscribes
   directly to a signal (no memo). Write signal (changed). Settle.
   Assert scope kept (signal is in _changed_signals).
5. **test_settle_scope_subscribes_to_both** — scope subscribes to
   stable memo AND changed signal. Settle. Assert scope kept
   (changed signal keeps it dirty).
6. **test_settle_no_dirty_scopes** — No dirty scopes. Settle.
   Assert no crash, dirty_scopes still empty.
7. **test_settle_all_stable** — Multiple scopes, all subscribe to
   stable memos. Settle. Assert all removed.
8. **test_settle_no_changed_signals** — Dirty scopes exist but
   _changed_signals is empty (possible after drain_dirty reset +
   no writes). Settle. Assert all scopes removed.
9. **test_settle_chain_cascade** — signal → A → B → C, scope at end.
   Write same value. Recompute A (stable), B (stable), C (stable).
   Settle. Assert scope removed.
10. **test_settle_chain_partial** — signal → A → B, scope subscribes
    to B. Write new value. Recompute A (changed), B (stable).
    Settle. Assert scope removed (B's output didn't change).
11. **test_settle_chain_changed** — signal → A → B, scope subscribes
    to B. Write value that changes through chain. Recompute A
    (changed), B (changed). Settle. Assert scope kept.
12. **test_settle_diamond_one_stable** — signal → A, B. C reads
    A + B. Scope subscribes to C. A stable, B changed, C changed.
    Settle. Assert scope kept.
13. **test_settle_with_direct_signal_sub** — scope subscribes to
    raw signal (no memo). Signal is written. Settle. Assert scope
    kept (signal write is in _changed_signals).
14. **test_settle_effect_not_affected** — Verify settle_scopes does
    not affect effect pending state. Effect is pending, memo was
    stable. After settle, effect still pending (settle only affects
    scopes).
15. **test_settle_idempotent** — Call settle twice. Assert same
    result (no crash, no double-removal).
16. **test_regression_settle_after_no_memos** — App has signals and
    scopes but no memos. Write signal. Settle. Assert scope kept
    (signal change is tracked).

#### P37.3 — EqualityDemoApp

A demo app that demonstrates the equality gate in action. Uses a
chain where an intermediate memo frequently stabilizes, showing
that downstream memos and scopes skip unnecessary work.

##### App structure: EqualityDemo

```text
  signal input (Int32, starts at 0)
  memo clamped = clamp(input, 0, 10)     // MemoI32
  memo label = clamped > 5 ? "high" : "low"  // MemoString

  UI:
    <div>
      <h1>Equality Gate</h1>
      <button onclick="+1">+ 1</button>
      <button onclick="-1">- 1</button>
      <p>Input: {input}</p>
      <p>Clamped: {clamped}</p>
      <p>Label: {label}</p>
    </div>
```

Interesting behaviour:

- Input 0→1: clamped 0→1 (changed), label "low"→"low" (stable!)
- Input 5→6: clamped 5→6 (changed), label "low"→"high" (changed)
- Input 10→11: clamped 10→10 (stable!), label "high"→"high" (stable!)
- Input 11→12: clamped 10→10 (stable!), label "high"→"high" (stable!)

When input exceeds 10, the entire downstream chain is value-stable —
zero wasted recomputation.

##### Mojo implementation (`src/main.mojo`)

```mojo
struct EqualityDemoApp(Movable):
    var ctx: ComponentContext
    var input: _SignalI32
    var clamped: MemoI32
    var label: MemoString
    var incr_handler: UInt32
    var decr_handler: UInt32

    fn __init__(out self):
        self.ctx = ComponentContext.create()
        self.input = self.ctx.use_signal(0)
        self.clamped = self.ctx.use_memo(0)
        self.label = self.ctx.use_memo_string(String("low"))
        # ... setup_view with el_div, buttons, paragraphs ...
        self.incr_handler = self.ctx.view_event_handler_id(0)
        self.decr_handler = self.ctx.view_event_handler_id(1)

    fn run_memos(mut self):
        if self.clamped.is_dirty():
            self.clamped.begin_compute()
            var i = self.input.read()
            var c = i
            if c < 0: c = 0
            if c > 10: c = 10
            self.clamped.end_compute(c)

        if self.label.is_dirty():
            self.label.begin_compute()
            var c = self.clamped.read()
            if c > 5:
                self.label.end_compute(String("high"))
            else:
                self.label.end_compute(String("low"))

    fn render(mut self) -> UInt32:
        var vb = self.ctx.render_builder()
        vb.add_dyn_text(String("Input: ") + String(self.input.peek()))
        vb.add_dyn_text(String("Clamped: ") + String(self.clamped.peek()))
        vb.add_dyn_text(String("Label: ") + self.label.peek())
        return vb.build()
```

WASM exports follow the established `eq_` prefix pattern:
`eq_init`, `eq_destroy`, `eq_rebuild`, `eq_handle_event`, `eq_flush`,
`eq_input_value`, `eq_clamped_value`, `eq_label_text`,
`eq_clamped_dirty`, `eq_label_dirty`, `eq_clamped_changed`,
`eq_label_changed`, `eq_incr_handler`, `eq_decr_handler`,
`eq_has_dirty`, `eq_scope_count`, `eq_memo_count`.

##### TypeScript handle

```typescript
interface EqualityDemoAppHandle {
  init(): bigint;
  destroy(app: bigint): void;
  rebuild(app: bigint, writer: bigint): number;
  handleEvent(app: bigint, handlerId: number, eventType: number): boolean;
  flush(app: bigint, writer: bigint): number;
  inputValue(app: bigint): number;
  clampedValue(app: bigint): number;
  labelText(app: bigint): string;
  clampedDirty(app: bigint): boolean;
  labelDirty(app: bigint): boolean;
  clampedChanged(app: bigint): boolean;
  labelChanged(app: bigint): boolean;
  incrHandler(app: bigint): number;
  decrHandler(app: bigint): number;
  hasDirty(app: bigint): boolean;
  scopeCount(app: bigint): number;
  memoCount(app: bigint): number;
}
```

##### Test: `test/test_equality_demo.mojo` (~20 tests)

1. **test_eq_initial_state** — input=0, clamped=0, label="low".
2. **test_eq_incr_within_range** — input 0→1, clamped 0→1 (changed),
   label "low"→"low" (stable). Assert clamped_changed=True,
   label_changed=False.
3. **test_eq_incr_across_threshold** — input 5→6, clamped 5→6
   (changed), label "low"→"high" (changed).
4. **test_eq_incr_at_max** — input 10→11, clamped 10→10 (stable),
   label "high"→"high" (stable). Assert clamped_changed=False,
   label_changed=False.
5. **test_eq_incr_above_max** — input 15→16, clamped 10→10 (stable).
6. **test_eq_decr_within_range** — input 5→4, clamped 5→4 (changed),
   label "low"→"low" (stable).
7. **test_eq_decr_across_threshold** — input 6→5, clamped 6→5
   (changed), label "high"→"low" (changed).
8. **test_eq_decr_at_min** — input 0→-1, clamped 0→0 (stable).
9. **test_eq_decr_below_min** — input -5→-6, clamped 0→0 (stable).
10. **test_eq_full_cycle** — increment from 0 to 12, then decrement
    back to 0. Verify clamped stabilizes outside [0,10], label
    stabilizes within each side of threshold.
11. **test_eq_label_dirty_after_clamped_stable** — After clamp
    stabilizes, assert label is NOT dirty (equality cascade).
12. **test_eq_scope_settled_when_all_stable** — After incr above
    max, run_memos + settle_scopes. Assert dirty_scopes empty.
13. **test_eq_scope_dirty_when_label_changed** — After incr across
    threshold, run_memos + settle_scopes. Assert scope still dirty.
14. **test_eq_flush_returns_zero_when_stable** — Flush after incr
    above max. Assert 0 bytes returned (no mutations).
15. **test_eq_flush_returns_nonzero_when_changed** — Flush after
    incr across threshold. Assert > 0 bytes.
16. **test_eq_handle_event_marks_dirty** — dispatch incr event.
    Assert has_dirty True.
17. **test_eq_memo_count** — Assert memo_count == 2.
18. **test_eq_destroy_clean** — Destroy app, no crash.
19. **test_eq_initial_compute_all_changed** — After first rebuild,
    both memos report value_changed True (initial compute).
20. **test_eq_consecutive_stable_flushes** — Multiple increments
    above max, each flush returns 0 (stable chain, no mutations).

##### Test: `test-js/equality_demo.test.ts` (~22 suites)

JS-side integration tests mirroring the Mojo tests, exercising the
full WASM → JS → DOM pipeline:

1. **init and destroy** — lifecycle smoke test.
2. **initial render** — correct DOM content after rebuild.
3. **increment within range** — DOM updates for input and clamped,
   label unchanged.
4. **increment across threshold** — DOM updates for all three.
5. **increment at max (clamped stable)** — only input DOM updates.
6. **increment above max (chain stable)** — only input DOM updates.
7. **decrement within range** — DOM updates for input and clamped.
8. **decrement across threshold** — all three update.
9. **decrement at min (clamped stable)** — only input updates.
10. **clamped_changed after stable** — assert eq_clamped_changed
    returns 0.
11. **label_changed after stable** — assert eq_label_changed
    returns 0.
12. **clamped_changed after value change** — returns 1.
13. **label_changed after value change** — returns 1.
14. **flush returns 0 when stable** — verify zero-byte flush.
15. **flush returns nonzero when changed** — verify non-zero flush.
16. **multiple stable flushes** — 5 increments above max, each
    flush is zero-byte.
17. **full cycle round-trip** — increment 0→12, decrement 12→0,
    verify all intermediate states.
18. **scope count** — 1.
19. **memo count** — 2.
20. **dirty state after event** — has_dirty returns true.
21. **dirty state after stable flush** — has_dirty returns false.
22. **destroy is clean** — no errors after destroy.

#### P37.4 — Update existing apps to use `settle_scopes()`

Update all apps with memo chains to call `ctx.settle_scopes()`
after `run_memos()` in their flush functions. This is a one-line
addition per app.

##### Mojo changes

###### `src/main.mojo` — flush functions

Update `_mc_flush` (MemoChainApp):

```mojo
fn _mc_flush(...) -> Int32:
    if not app[0].ctx.consume_dirty():
        return 0
    app[0].run_memos()
    app[0].ctx.settle_scopes()    # NEW
    var new_idx = app[0].render()
    app[0].ctx.diff(writer_ptr, new_idx)
    return app[0].ctx.finalize(writer_ptr)
```

Update `_em_flush` (EffectMemoApp):

```mojo
fn _em_flush(...) -> Int32:
    if not app[0].ctx.consume_dirty():
        return 0
    app[0].run_memos_and_effects()
    app[0].ctx.settle_scopes()    # NEW
    var new_idx = app[0].render()
    app[0].ctx.diff(writer_ptr, new_idx)
    return app[0].ctx.finalize(writer_ptr)
```

Update `_mf_flush` (MemoFormApp) similarly.

Note: `settle_scopes()` is called AFTER `run_memos()` (and
`run_effects()` if applicable) because memos must be recomputed
first to determine which output signals changed. Effects that write
signals also add to `_changed_signals`. Only after all reactive
computations are done can we safely determine which scopes actually
need re-rendering.

For apps without memos (CounterApp, TodoApp, BenchmarkApp), no
change is needed — they don't have equality-gated chains and their
scopes are dirty because signals changed directly.

##### Test updates

Add tests to existing app test suites verifying the settle
optimization:

**test/test_memo_chain.mojo** — 2 new tests:

- **test_mc_stable_chain_no_rerender** — Set input to value that
  produces the same doubled result (e.g., if doubled is clamped or
  if input didn't change). Flush. Assert 0 bytes (settled).
- **test_mc_settle_after_memos** — After run_memos where all memos
  stable, assert has_dirty is False (scopes settled).

**test-js/memo_chain.test.ts** — 2 new suites:

- **stable chain produces zero-byte flush** — same pattern in JS.
- **settled scope count** — after stable flush, scope count
  unchanged, no DOM mutations.

#### P37.5 — Documentation & AGENTS.md update

##### Changes

**AGENTS.md:**

- **Common Patterns:** Add "Equality-gated memo propagation" pattern
  describing the skip-if-unchanged optimization in `end_compute`
  and the `settle_scopes()` flush integration.
- **Common Patterns:** Update "Effect + memo chain pattern" to
  mention that `settle_scopes()` should be called after memo
  recomputation and effect execution.
- **EqualityDemoApp architecture:** Add to App Architectures section
  with the clamped + threshold memo chain example.
- **File Size Reference:** Update line counts for `memo.mojo`,
  `runtime.mojo`, `context.mojo`, `app_shell.mojo`, `main.mojo`.

**CHANGELOG.md:**

- Add Phase 37 entry summarizing P37.1 (equality checking),
  P37.2 (scope settle), P37.3 (demo app), P37.4 (existing app
  updates), P37.5 (docs). Include test count delta.

**README.md:**

- Update test count.
- Add "Equality-gated memo propagation" to Features list.
- Update MemoChainApp code example to mention that equality checking
  cascades through the chain automatically.
- Add EqualityDemoApp to the app list with a brief description.

### P37 Dependency graph

```text
P37.1 (MemoEntry flag + equality in end_compute + changed_signals tracking)
    │
    ├──► P37.2 (settle_scopes — depends on _changed_signals)
    │        │
    │        ├──► P37.4 (Update existing apps — depends on settle_scopes)
    │        │
    │        └──► P37.3 (EqualityDemoApp — depends on equality + settle)
    │                 │
    └─────────────────┘
                      │
                      ▼
                  P37.5 (Docs — depends on all above)
```

P37.1 must land first (equality checking + changed_signals).
P37.2 depends on P37.1 (uses `_changed_signals`).
P37.3 and P37.4 both depend on P37.2 (use `settle_scopes`).
P37.3 and P37.4 are independent of each other.
P37.5 depends on all above.

### P37 Estimated size

| Step | ~Changed Mojo Lines | ~New Mojo Lines | ~New TS Lines | Tests |
|------|--------------------|-----------------| --------------|-------|
| P37.1 (MemoEntry + equality + tests) | ~80 (end_compute refactor) | ~650 | ~0 | 22 Mojo |
| P37.2 (settle_scopes + tests) | ~20 (runtime + context) | ~500 | ~0 | 16 Mojo |
| P37.3 (EqualityDemoApp + tests) | ~0 | ~400 | ~300 | 20 Mojo + 22 JS |
| P37.4 (existing app updates + tests) | ~15 (flush functions) | ~50 | ~30 | 2 Mojo + 2 JS |
| P37.5 (docs) | ~0 | ~80 | ~0 | 0 |
| **Total** | **~115** | **~1,680** | **~330** | **60 Mojo + 24 JS = 84 tests** |
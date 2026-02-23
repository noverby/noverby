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
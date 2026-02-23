# Phase 32 — Error Boundaries

## Problem

Phase 8.4 added low-level error boundary infrastructure to the scope
system — `ScopeState` has `is_error_boundary`, `has_error`,
`error_message` fields with setters/getters, `ScopeArena` has
`find_error_boundary()` and `propagate_error()` parent-chain walk-up,
and there are WASM exports (`err_set_boundary`, `err_propagate`, etc.)
with unit tests in `phase8.test.ts`. However:

1. **ComponentContext has no error boundary API.** The scope plumbing
   exists but is not surfaced on `ComponentContext` or
   `ChildComponentContext`. No component code uses error boundaries.

2. **No integration with the render/flush cycle.** When an error
   boundary captures an error, nothing happens in the DOM — children
   continue rendering normally. There is no mechanism to swap between
   normal content and a fallback UI based on error state.

3. **No fallback rendering pattern.** Error boundaries in React/Dioxus
   catch errors in their child tree and render fallback UI. We have
   `ConditionalSlot` for show/hide transitions, but no established
   pattern for error-driven content switching.

4. **No recovery mechanism.** Clearing an error should re-render the
   normal child tree, but there is no demo or test showing this
   lifecycle.

5. **No demonstration app.** Without a working error boundary demo,
   the feature is theoretical — never validated end-to-end with DOM
   rendering, event handling, and recovery.

### Current state (Phase 31)

Error boundary scope fields exist but are dead code at the component
level:

```mojo
# scope/scope.mojo — fields exist but component layer ignores them
var is_error_boundary: Bool
var has_error: Bool
var error_message: String

# scope/arena.mojo — walk-up exists but ComponentContext doesn't call it
fn find_error_boundary(self, scope_id: UInt32) -> Int
fn propagate_error(mut self, scope_id: UInt32, message: String) -> Int
```

### Target pattern (Phase 32)

```mojo
comptime CTX_THEME: UInt32 = 1

struct SafeCounterApp:
    var ctx: ComponentContext
    var count: SignalI32
    var child: ChildComponentContext          # normal content
    var fallback: ChildComponentContext        # fallback UI
    var retry_handler: UInt32

    fn __init__(out self):
        self.ctx = ComponentContext.create()
        self.count = self.ctx.use_signal(0)
        # Mark root scope as error boundary
        self.ctx.use_error_boundary()
        self.ctx.setup_view(
            el_div(
                el_h1(text("Safe Counter")),
                el_button(text("+ 1"), onclick_add(self.count, 1)),
                dyn_node(0),   # normal content OR fallback
                dyn_node(1),   # second slot for the other state
            ),
            String("safe-counter"),
        )
        # Normal child: displays count
        var child_ctx = self.ctx.create_child_context(
            el_p(dyn_text()), String("display"),
        )
        self.child = child_ctx^
        # Fallback child: shows error + retry button
        self.retry_handler = self.ctx.register_custom_handler(String("click"))
        var fb_ctx = self.ctx.create_child_context(
            el_div(
                el_p(dyn_text()),
                el_button(text("Retry"), dyn_attr(0)),
            ),
            String("fallback"),
        )
        self.fallback = fb_ctx^

    fn flush(mut self, writer: ...) -> Int32:
        if self.ctx.has_error():
            # Error state: hide normal child, show fallback
            self.child.flush_empty(writer)
            var fb_idx = self.render_fallback()
            self.fallback.flush(writer, fb_idx)
        else:
            # Normal state: show child, hide fallback
            self.fallback.flush_empty(writer)
            var child_idx = self.render_child()
            self.child.flush(writer, child_idx)
        return self.ctx.finalize(writer)

    fn handle_event(mut self, handler_id: UInt32, ...) -> Bool:
        if handler_id == self.retry_handler:
            self.ctx.clear_error()    # clears error → next flush shows child
            return True
        return self.ctx.dispatch_event(handler_id, event_type)
```

---

## Design

### Error boundary lifecycle

The error boundary lifecycle integrates with the existing
`ConditionalSlot`-based flush pattern:

```text
             ┌─────────┐
             │  Normal  │ ← initial state: child rendered, fallback hidden
             └────┬─────┘
                  │ report_error("boom")
                  ▼
           ┌──────────────┐
           │  Error State  │ ← boundary.has_error = true
           │  (fallback)   │   child.flush_empty() + fallback.flush(vnode)
           └──────┬────────┘
                  │ clear_error()
                  ▼
             ┌─────────┐
             │  Normal  │ ← child re-renders, fallback hidden
             └──────────┘
```

### ComponentContext surface

New methods on `ComponentContext`:

| Method | Description |
|--------|-------------|
| `use_error_boundary()` | Mark root scope as error boundary |
| `report_error(msg)` | Propagate error from root scope upward |
| `has_error() -> Bool` | Check if boundary has captured an error |
| `error_message() -> String` | Get the captured error message |
| `clear_error()` | Clear error state, allow re-render |

New methods on `ChildComponentContext`:

| Method | Description |
|--------|-------------|
| `report_error(msg)` | Propagate error from child scope upward |

### Propagation mechanics

Error propagation reuses the existing `ScopeArena.propagate_error()`
which walks the parent chain from the reporting scope to the nearest
ancestor with `is_error_boundary = true` and sets the error there.

```text
Root (boundary) ← error lands here
  └─ Child A
       └─ Child B  ← report_error("crash") starts here
```

When `propagate_error()` returns -1 (no boundary found), the error is
unhandled. The `report_error()` method on ComponentContext returns the
boundary scope ID (or -1) so the caller can detect unhandled errors.

### Flush integration

The error boundary owner checks `has_error()` during flush:

- **No error:** flush normal children, hide fallback children
- **Error present:** hide normal children, flush fallback with error
  message text
- **Error cleared:** re-flush normal children (creates from scratch
  if previously hidden), hide fallback

This is the same `flush` / `flush_empty` alternation that
`ConditionalSlot` already supports — error boundaries don't need a
new slot type.

### Dirty tracking

When `report_error()` sets the error on a boundary scope, it must
mark that scope dirty so the next flush picks up the state change.
`propagate_error()` already sets `has_error` on the boundary scope;
we add a `mark_scope_dirty()` call after successful propagation.

Similarly, `clear_error()` must mark the boundary scope dirty.

### JS runtime

No new JS runtime infrastructure is needed. Error boundaries are
entirely WASM-side — the JS runtime just applies mutations as usual.
The fallback UI is rendered through the same mutation protocol.

---

## Steps

### P32.1 — ComponentContext error boundary surface

**Goal:** Surface the existing scope error boundary infrastructure on
`ComponentContext` and `ChildComponentContext` with ergonomic methods.

#### Mojo changes

**`src/component/context.mojo`** — Add to `ComponentContext`:

```mojo
# ── Error Boundary ───────────────────────────────────────────────

fn use_error_boundary(mut self):
    """Mark the root scope as an error boundary.

    Call during setup (before end_setup). When a descendant scope
    reports an error via `report_error()`, this scope captures it.
    Check `has_error()` during flush to switch to fallback rendering.
    """
    self.shell.runtime[0].scopes.set_error_boundary(
        self.scope_id, True
    )

fn report_error(mut self, message: String) -> Int:
    """Propagate an error from this scope to the nearest boundary.

    Walks up the parent chain from the root scope. If a boundary
    is found, sets the error on it and marks it dirty.

    Args:
        message: Description of the error.

    Returns:
        The boundary scope ID as Int, or -1 if no boundary found.
    """
    var boundary_id = self.shell.runtime[0].scopes.propagate_error(
        self.scope_id, message
    )
    if boundary_id != -1:
        self.shell.runtime[0].mark_scope_dirty(UInt32(boundary_id))
    return boundary_id

fn has_error(self) -> Bool:
    """Check whether this scope (as a boundary) has captured an error.

    Returns:
        True if an error has been propagated to this boundary.
    """
    return self.shell.runtime[0].scopes.has_error(self.scope_id)

fn error_message(self) -> String:
    """Get the error message captured by this boundary.

    Returns:
        The error message string, or empty if no error.
    """
    return self.shell.runtime[0].scopes.get_error_message(
        self.scope_id
    )

fn clear_error(mut self):
    """Clear the error state on this boundary scope.

    After clearing, the next flush should render normal children
    instead of the fallback UI. Marks the scope dirty.
    """
    self.shell.runtime[0].scopes.clear_error(self.scope_id)
    self.shell.runtime[0].mark_scope_dirty(self.scope_id)
```

**`src/component/child_context.mojo`** — Add to `ChildComponentContext`:

```mojo
# ── Error reporting ──────────────────────────────────────────────

fn report_error(self, message: String) -> Int:
    """Propagate an error from this child scope to the nearest boundary.

    Walks up the parent chain from the child scope. If a boundary
    is found, sets the error on it and marks it dirty.

    Args:
        message: Description of the error.

    Returns:
        The boundary scope ID as Int, or -1 if no boundary found.
    """
    var boundary_id = self.runtime[0].scopes.propagate_error(
        self.scope_id, message
    )
    if boundary_id != -1:
        self.runtime[0].mark_scope_dirty(UInt32(boundary_id))
    return boundary_id
```

**`src/component/__init__.mojo`** — No changes needed (methods are on
existing exported structs).

#### WASM exports (in `src/main.mojo`)

Thin wrappers for testing the new ComponentContext surface:

```mojo
# ── ErrorBoundary test helpers ───────────────────────────────────

# Tested through ErrorBoundaryApp and ErrorNestApp exports (P32.2/P32.3).
# No standalone exports needed — the existing err_* exports from
# Phase 8.4 cover low-level testing.
```

#### Test: `test/test_error_boundary.mojo`

New test module with ~15 tests:

1. `ctx_use_error_boundary_marks_scope` — after `use_error_boundary()`,
   scope is a boundary
2. `ctx_has_error_initially_false` — boundary starts with no error
3. `ctx_error_message_initially_empty` — no message before error
4. `ctx_report_error_finds_boundary` — propagation returns boundary ID
5. `ctx_report_error_sets_has_error` — `has_error()` returns True
6. `ctx_report_error_stores_message` — `error_message()` matches
7. `ctx_clear_error_resets_state` — `has_error()` False after clear
8. `ctx_clear_error_empty_message` — message empty after clear
9. `ctx_report_error_no_boundary_returns_neg1` — no boundary → -1
10. `ctx_child_report_error_reaches_parent` — child scope error
    propagates to parent boundary
11. `ctx_report_error_marks_dirty` — boundary scope is dirty after
    propagation
12. `ctx_clear_error_marks_dirty` — scope dirty after clear
13. `ctx_multiple_errors_last_wins` — second error overwrites first
14. `ctx_error_after_clear_works` — error → clear → error cycle
15. `ctx_boundary_is_not_own_boundary` — reporting from the boundary
    scope itself walks to its parent (if any)

#### Test: `test-js/error_boundary.test.ts`

JS tests exercising the low-level error boundary API via existing
`err_*` exports, plus new app-level tests in P32.2:

1. `boundary_flag_survives_scope_lifecycle` — create scope, set
   boundary, verify across dirty cycles
2. `propagate_marks_boundary_dirty` — verify dirty_scopes contains
   boundary after propagation (via runtime query exports)
3. `clear_marks_boundary_dirty` — verify dirty_scopes after clear

---

### P32.2 — ErrorBoundaryApp demo

**Goal:** A working error boundary app where a child can "crash," the
parent catches the error and shows fallback UI, and a Retry button
recovers.

#### App structure: SafeCounter

```text
SafeCounterApp (root scope = error boundary)
├── h1 "Safe Counter"
├── button "+ 1"  (onclick_add count)
├── button "Crash"  (onclick_custom → report_error)
├── dyn_node[0]   ← normal child OR fallback child
└── (second dyn_node[1] for the hidden slot)

Normal child (CounterDisplayChild):
    p > dyn_text("Count: N")

Fallback child (ErrorFallbackChild):
    div > p(dyn_text("Error: ...")) + button("Retry")
```

**Lifecycle:**

1. **Init:** Parent creates error boundary, two child contexts (normal
   + fallback), and a custom "Crash" handler. Normal child is shown,
   fallback is hidden.
2. **Increment:** Parent's count signal updates → normal child re-renders
   with new count.
3. **Crash:** Crash button dispatched → parent calls
   `report_error("Simulated crash")` → parent scope marked dirty.
4. **Flush (error state):** `has_error()` returns True → normal child
   hidden (`flush_empty`), fallback shown with error message.
5. **Retry:** Retry button dispatched → parent calls `clear_error()` →
   scope marked dirty.
6. **Flush (recovered):** `has_error()` returns False → fallback hidden,
   normal child re-renders (creates from scratch since it was hidden).
7. **Count preserved:** The count signal value persists across
   crash/recovery cycles because the signal lives on the parent scope.

#### Mojo implementation (`src/main.mojo`)

```mojo
# ══════════════════════════════════════════════════════════════════════════════
# Phase 32.2 — SafeCounterApp (error boundary demo)
# ══════════════════════════════════════════════════════════════════════════════

comptime _SC_PROP_COUNT: UInt32 = 20


struct SCNormalChild(Movable):
    """Normal content child: displays count.

    Template: p > dyn_text("Count: N")
    Consumes count signal from parent context.
    """
    var child_ctx: ChildComponentContext
    var count: SignalI32

    fn __init__(
        out self,
        var child_ctx: ChildComponentContext,
        var count: SignalI32,
    ):
        self.child_ctx = child_ctx^
        self.count = count^

    fn __moveinit__(out self, deinit other: Self):
        self.child_ctx = other.child_ctx^
        self.count = other.count^

    fn render(mut self) -> UInt32:
        var vb = self.child_ctx.render_builder()
        vb.add_dyn_text(
            String("Count: ") + String(self.count.peek())
        )
        return vb.build()


struct SCFallbackChild(Movable):
    """Fallback child: shows error message + retry button.

    Template: div > p(dyn_text) + button("Retry", dyn_attr[0])
    """
    var child_ctx: ChildComponentContext

    fn __init__(out self, var child_ctx: ChildComponentContext):
        self.child_ctx = child_ctx^

    fn __moveinit__(out self, deinit other: Self):
        self.child_ctx = other.child_ctx^

    fn render(mut self, error_msg: String) -> UInt32:
        var vb = self.child_ctx.render_builder()
        vb.add_dyn_text(String("Error: ") + error_msg)
        return vb.build()


struct SafeCounterApp(Movable):
    """Counter app with error boundary.

    Parent: div > h1("Safe Counter") + button("+1") + button("Crash")
            + dyn_node[0] + dyn_node[1]
    Normal: p > dyn_text("Count: N")
    Fallback: div > p(dyn_text("Error: ...")) + button("Retry")

    The Crash button triggers report_error(). The parent catches it
    and swaps to fallback UI. Retry clears the error and restores
    normal rendering.
    """
    var ctx: ComponentContext
    var count: SignalI32
    var normal: SCNormalChild
    var fallback: SCFallbackChild
    var crash_handler: UInt32
    var retry_handler: UInt32

    fn __init__(out self):
        self.ctx = ComponentContext.create()
        self.count = self.ctx.use_signal(0)
        self.ctx.use_error_boundary()
        self.ctx.provide_signal_i32(_SC_PROP_COUNT, self.count)
        # ... setup_view with buttons and dyn_node slots ...
        # ... create normal + fallback child contexts ...
        # ... register crash + retry custom handlers ...
        ...

    fn __moveinit__(out self, deinit other: Self):
        self.ctx = other.ctx^
        self.count = other.count^
        self.normal = other.normal^
        self.fallback = other.fallback^
        self.crash_handler = other.crash_handler
        self.retry_handler = other.retry_handler

    fn render_parent(mut self) -> UInt32:
        var pvb = self.ctx.render_builder()
        pvb.add_dyn_placeholder()  # dyn_node[0]
        pvb.add_dyn_placeholder()  # dyn_node[1]
        return pvb.build()
```

**Lifecycle functions:**

- `_sc_init() -> UnsafePointer[SafeCounterApp]` — allocate + create
- `_sc_destroy(app_ptr)` — destroy children, context, free
- `_sc_rebuild(app, writer) -> Int32` — mount parent, extract anchors,
  init both child slots, flush normal child, finalize
- `_sc_handle_event(app, handler_id, event_type) -> Bool` — route
  crash handler → `report_error()`, retry handler → `clear_error()`,
  else → `dispatch_event()`
- `_sc_flush(app, writer) -> Int32` — check `has_error()`:
  - If error: `normal.flush_empty()` + `fallback.flush(error_vnode)`
  - If no error: `fallback.flush_empty()` + `normal.flush(count_vnode)`

**WASM exports (~20):**

```mojo
@export fn sc_init() -> Int64
@export fn sc_destroy(app_ptr: Int64)
@export fn sc_rebuild(app_ptr: Int64, buf_ptr: Int64, cap: Int32) -> Int32
@export fn sc_handle_event(app_ptr: Int64, hid: Int32, evt: Int32) -> Int32
@export fn sc_flush(app_ptr: Int64, buf_ptr: Int64, cap: Int32) -> Int32
@export fn sc_count_value(app_ptr: Int64) -> Int32
@export fn sc_has_error(app_ptr: Int64) -> Int32
@export fn sc_error_message(app_ptr: Int64) -> String
@export fn sc_crash_handler(app_ptr: Int64) -> Int32
@export fn sc_retry_handler(app_ptr: Int64) -> Int32
@export fn sc_incr_handler(app_ptr: Int64) -> Int32
@export fn sc_normal_mounted(app_ptr: Int64) -> Int32
@export fn sc_fallback_mounted(app_ptr: Int64) -> Int32
@export fn sc_normal_has_rendered(app_ptr: Int64) -> Int32
@export fn sc_fallback_has_rendered(app_ptr: Int64) -> Int32
@export fn sc_has_dirty(app_ptr: Int64) -> Int32
@export fn sc_handler_count(app_ptr: Int64) -> Int32
@export fn sc_scope_count(app_ptr: Int64) -> Int32
@export fn sc_parent_scope_id(app_ptr: Int64) -> Int32
@export fn sc_normal_scope_id(app_ptr: Int64) -> Int32
@export fn sc_fallback_scope_id(app_ptr: Int64) -> Int32
```

#### TypeScript handle

**`runtime/app.ts`** — Add `SafeCounterAppHandle` and
`createSafeCounterApp()`:

```typescript
interface SafeCounterAppHandle extends AppHandle {
  getCount(): number;
  hasError(): boolean;
  getErrorMessage(): string;
  isNormalMounted(): boolean;
  isFallbackMounted(): boolean;
  normalHasRendered(): boolean;
  fallbackHasRendered(): boolean;
  hasDirty(): boolean;
  handlerCount(): number;
  scopeCount(): number;
  increment(): void;
  crash(): void;
  retry(): void;
}
```

#### Test: `test/test_safe_counter.mojo` (~18 tests)

1. `sc_init_creates_app` — pointer is valid
2. `sc_count_starts_at_0` — initial count is 0
3. `sc_has_error_initially_false` — no error at start
4. `sc_error_message_initially_empty` — empty string
5. `sc_normal_mounted_after_rebuild` — normal child is in DOM
6. `sc_fallback_not_mounted_initially` — fallback hidden
7. `sc_increment_updates_count` — count changes
8. `sc_crash_sets_error` — `has_error()` true after crash
9. `sc_crash_stores_message` — error message matches
10. `sc_flush_after_crash_hides_normal` — normal unmounted
11. `sc_flush_after_crash_shows_fallback` — fallback mounted
12. `sc_retry_clears_error` — `has_error()` false after retry
13. `sc_flush_after_retry_shows_normal` — normal remounted
14. `sc_flush_after_retry_hides_fallback` — fallback unmounted
15. `sc_count_preserved_after_crash_recovery` — signal persists
16. `sc_multiple_crash_retry_cycles` — 5 cycles work
17. `sc_destroy_does_not_crash` — clean shutdown
18. `sc_rapid_increments_after_recovery` — 20 increments post-recovery

#### Test: `test-js/safe_counter.test.ts` (~22 suites)

1. `sc_init state validation` — count=0, no error, handlers valid
2. `sc_rebuild produces mutations` — RegisterTemplate, LoadTemplate,
   AppendChildren, SetText "Count: 0"
3. `sc_increment updates count` — count changes to 1
4. `sc_flush after increment` — SetText "Count: 1"
5. `sc_crash sets error state` — hasError true, message matches
6. `sc_flush after crash swaps to fallback` — DOM shows "Error: ..."
7. `sc_normal hidden after crash` — normal child unmounted
8. `sc_fallback visible after crash` — fallback child mounted
9. `sc_retry clears error` — hasError false
10. `sc_flush after retry restores normal` — DOM shows "Count: N"
11. `sc_fallback hidden after retry` — fallback unmounted
12. `sc_count preserved across crash/retry` — value unchanged
13. `sc_increment after recovery works` — count continues
14. `sc_DOM structure initial` — h1 + buttons + p("Count: 0")
15. `sc_DOM structure error state` — h1 + buttons + div(p("Error:...") +
    button("Retry"))
16. `sc_DOM structure recovered` — back to h1 + buttons + p("Count: N")
17. `sc_multiple crash/retry cycles` — 3 full cycles
18. `sc_crash without increment` — error at count=0
19. `sc_rapid increments then crash` — 10 increments then crash
20. `sc_destroy does not crash` — clean shutdown
21. `sc_double destroy safe` — no crash on double destroy
22. `sc_multiple independent instances` — two instances isolated

Register in `test-js/run.ts`.

---

### P32.3 — ErrorNestApp demo (nested error boundaries)

**Goal:** Demonstrate nested error boundaries where inner boundaries
catch inner errors and outer boundaries catch outer errors.

#### App structure: ErrorNest

```text
ErrorNestApp (outer boundary)
├── h1 "Nested Boundaries"
├── button "Outer Crash"   (crashes to outer boundary)
├── dyn_node[0]  ← outer normal content / outer fallback
│
├── OuterNormalChild (inner boundary)
│   ├── p > dyn_text("Status: OK")
│   ├── button "Inner Crash"   (crashes to inner boundary)
│   └── dyn_node[0]  ← inner normal content / inner fallback
│   │
│   ├── InnerNormalChild
│   │   └── p > dyn_text("Inner: working")
│   │
│   └── InnerFallbackChild
│       └── p > dyn_text("Inner error: ...") + button("Inner Retry")
│
├── OuterFallbackChild
│   └── p > dyn_text("Outer error: ...") + button("Outer Retry")
```

**Key scenarios:**

1. **Inner crash:** Inner child reports error → caught by inner
   boundary (OuterNormalChild) → inner content swaps to inner fallback
   while outer content remains unaffected.
2. **Inner retry:** Clears inner error → inner content restored.
3. **Outer crash:** Button on outer scope reports error → caught by
   outer boundary (ErrorNestApp root) → entire inner boundary + its
   children swapped to outer fallback.
4. **Outer retry:** Clears outer error → inner boundary + children
   restored.
5. **Both errors:** Inner crash then outer crash → outer fallback
   shown (overrides inner state visually). Outer retry → inner
   boundary visible again, still in error state (inner fallback
   shown). Inner retry → fully recovered.

#### Mojo implementation (`src/main.mojo`)

Structs:

- `InnerNormalChild` — displays "Inner: working"
- `InnerFallbackChild` — displays "Inner error: {msg}" + Inner Retry
  button
- `ENOuterNormal` — inner boundary managing InnerNormal +
  InnerFallback, with "Inner Crash" button
- `ENOuterFallback` — displays "Outer error: {msg}" + Outer Retry
  button
- `ErrorNestApp` — outer boundary managing OuterNormal +
  OuterFallback, with "Outer Crash" button

Lifecycle functions: `_en_init`, `_en_destroy`, `_en_rebuild`,
`_en_handle_event`, `_en_flush`.

**WASM exports (~25):**

```mojo
@export fn en_init() -> Int64
@export fn en_destroy(app_ptr: Int64)
@export fn en_rebuild(app_ptr: Int64, buf_ptr: Int64, cap: Int32) -> Int32
@export fn en_handle_event(app_ptr: Int64, hid: Int32, evt: Int32) -> Int32
@export fn en_flush(app_ptr: Int64, buf_ptr: Int64, cap: Int32) -> Int32
@export fn en_has_outer_error(app_ptr: Int64) -> Int32
@export fn en_has_inner_error(app_ptr: Int64) -> Int32
@export fn en_outer_error_message(app_ptr: Int64) -> String
@export fn en_inner_error_message(app_ptr: Int64) -> String
@export fn en_outer_crash_handler(app_ptr: Int64) -> Int32
@export fn en_inner_crash_handler(app_ptr: Int64) -> Int32
@export fn en_outer_retry_handler(app_ptr: Int64) -> Int32
@export fn en_inner_retry_handler(app_ptr: Int64) -> Int32
@export fn en_outer_normal_mounted(app_ptr: Int64) -> Int32
@export fn en_outer_fallback_mounted(app_ptr: Int64) -> Int32
@export fn en_inner_normal_mounted(app_ptr: Int64) -> Int32
@export fn en_inner_fallback_mounted(app_ptr: Int64) -> Int32
@export fn en_has_dirty(app_ptr: Int64) -> Int32
@export fn en_handler_count(app_ptr: Int64) -> Int32
@export fn en_scope_count(app_ptr: Int64) -> Int32
@export fn en_outer_scope_id(app_ptr: Int64) -> Int32
@export fn en_inner_boundary_scope_id(app_ptr: Int64) -> Int32
@export fn en_inner_normal_scope_id(app_ptr: Int64) -> Int32
@export fn en_inner_fallback_scope_id(app_ptr: Int64) -> Int32
@export fn en_outer_fallback_scope_id(app_ptr: Int64) -> Int32
```

#### TypeScript handle

**`runtime/app.ts`** — Add `ErrorNestAppHandle` and
`createErrorNestApp()`:

```typescript
interface ErrorNestAppHandle extends AppHandle {
  hasOuterError(): boolean;
  hasInnerError(): boolean;
  getOuterErrorMessage(): string;
  getInnerErrorMessage(): string;
  outerNormalMounted(): boolean;
  outerFallbackMounted(): boolean;
  innerNormalMounted(): boolean;
  innerFallbackMounted(): boolean;
  hasDirty(): boolean;
  handlerCount(): number;
  scopeCount(): number;
  outerCrash(): void;
  innerCrash(): void;
  outerRetry(): void;
  innerRetry(): void;
}
```

#### Test: `test/test_error_nest.mojo` (~20 tests)

1. `en_init_creates_app` — pointer valid
2. `en_no_errors_initially` — both boundaries clean
3. `en_all_normal_mounted_after_rebuild` — outer + inner normal visible
4. `en_no_fallbacks_initially` — both fallbacks hidden
5. `en_inner_crash_sets_inner_error` — inner `has_error` true
6. `en_inner_crash_preserves_outer` — outer still clean
7. `en_flush_after_inner_crash` — inner fallback shown, inner normal
   hidden, outer normal still mounted
8. `en_inner_retry_clears_inner_error` — inner clean again
9. `en_flush_after_inner_retry` — inner normal restored
10. `en_outer_crash_sets_outer_error` — outer `has_error` true
11. `en_flush_after_outer_crash` — outer fallback shown, outer normal
    hidden (inner boundary + children also hidden)
12. `en_outer_retry_restores_outer_normal` — outer normal + inner
    boundary visible again
13. `en_inner_crash_then_outer_crash` — both errors set, outer
    fallback takes precedence visually
14. `en_outer_retry_reveals_inner_error` — after outer retry, inner
    still in error (inner fallback shown)
15. `en_inner_retry_after_outer_retry` — full recovery
16. `en_multiple_inner_crash_retry_cycles` — 5 inner cycles
17. `en_multiple_outer_crash_retry_cycles` — 5 outer cycles
18. `en_mixed_crash_retry_sequence` — inner→outer→outer_retry→
    inner_retry
19. `en_destroy_does_not_crash` — clean shutdown
20. `en_destroy_with_active_error` — destroy while error is set

#### Test: `test-js/error_nest.test.ts` (~25 suites)

1. `en_init state validation` — no errors, handlers valid and distinct
2. `en_rebuild produces mutations` — RegisterTemplate ×N, LoadTemplate,
   AppendChildren, SetText for initial content
3. `en_DOM structure initial` — h1 + outer button + status p + inner
   button + inner p
4. `en_inner crash — DOM shows inner fallback` — inner fallback text
   visible, inner normal text gone
5. `en_inner crash — outer content unaffected` — outer status p still
   shows "Status: OK"
6. `en_inner retry — DOM restored` — inner normal text visible again
7. `en_outer crash — DOM shows outer fallback` — outer fallback text,
   all inner content gone
8. `en_outer retry — DOM restored with inner` — all content back
9. `en_inner then outer crash` — outer fallback shown
10. `en_outer retry reveals inner fallback` — inner fallback visible
    after outer retry
11. `en_inner retry after outer retry — full recovery` — everything
    normal
12. `en_error messages correct` — inner vs outer message strings
13. `en_scope IDs all distinct` — no overlap
14. `en_handler IDs all distinct` — 4 unique handlers
15. `en_flush returns 0 when clean` — no mutations when no changes
16. `en_inner crash flush produces minimal mutations` — only inner
    slot changes
17. `en_outer crash flush produces minimal mutations` — only outer
    slot changes
18. `en_5 inner crash/retry cycles` — DOM correct each time
19. `en_5 outer crash/retry cycles` — DOM correct each time
20. `en_destroy does not crash` — clean shutdown
21. `en_double destroy safe` — no crash
22. `en_multiple independent instances` — two instances isolated
23. `en_rapid alternating crashes` — 10 inner/outer alternations
24. `en_heapStats bounded across error cycles` — memory stable
25. `en_destroy with active errors` — no crash

Register in `test-js/run.ts`.

---

### P32.4 — Documentation & AGENTS.md update

**Goal:** Update project documentation to reflect the new error
boundary APIs and patterns.

#### Changes

**`AGENTS.md`** — Update Component Layer section:

- Add `use_error_boundary()`, `report_error()`, `has_error()`,
  `error_message()`, `clear_error()` to ComponentContext API list
- Add `report_error()` to ChildComponentContext API list
- Add "Error Boundary Pattern" to Common Patterns section:

  ```text
  **Error boundary flush pattern:** Check `ctx.has_error()` in flush
  to switch between normal and fallback children:
      if ctx.has_error():
          normal_child.flush_empty(writer)
          fallback_child.flush(writer, fallback_vnode)
      else:
          fallback_child.flush_empty(writer)
          normal_child.flush(writer, normal_vnode)
  ```

- Add SafeCounterApp and ErrorNestApp to App Architectures section
- Update File Size Reference with new file sizes
- Update Deferred Abstractions to note that error boundaries are now
  implemented

**`CHANGELOG.md`** — Add Phase 32 entry at the top:

```markdown
## Phase 32 — Error Boundaries

Wired the existing scope-level error boundary infrastructure (Phase
8.4) into the component layer — `ComponentContext` and
`ChildComponentContext` now have ergonomic error boundary methods.
Demonstrated with two apps: SafeCounterApp (single boundary with
crash/retry) and ErrorNestApp (nested boundaries with independent
error/recovery).

- **P32.1** — ComponentContext error boundary surface. Added
  `use_error_boundary()`, `report_error()`, `has_error()`,
  `error_message()`, `clear_error()` to ComponentContext. Added
  `report_error()` to ChildComponentContext. Error propagation
  walks the scope parent chain to the nearest boundary, sets the
  error, and marks the boundary dirty for the next flush cycle.

- **P32.2** — SafeCounterApp demo. Parent with error boundary,
  count signal, Crash button, and two child components (normal
  display + error fallback). Crash triggers `report_error()` →
  fallback shown with error message. Retry calls `clear_error()`
  → normal child re-renders. Count signal persists across crash/
  recovery cycles.

- **P32.3** — ErrorNestApp demo. Nested error boundaries: outer
  boundary on root, inner boundary on a child component. Inner
  crash caught by inner boundary (only inner slot swaps). Outer
  crash caught by outer boundary (entire inner tree replaced).
  Recovery at each level is independent. Mixed crash/retry
  sequences validated.

- **P32.4** — Documentation update. AGENTS.md, CHANGELOG.md, and
  README.md updated with error boundary API, patterns, and test
  counts.

**Test count after P32.4:** ~X Mojo (Y modules) + ~Z JS = ~W tests.
```

**`README.md`** — Update:

- Features list: add "Error Boundaries — scope-level error catching
  with fallback UI and recovery"
- Test count in Features section
- Test results section: add Error Boundary test descriptions
- Ergonomic API section: add error boundary code example

---

## Dependency graph

```text
P32.1 (ComponentContext error boundary surface)
    │
    ├──────────────────────┐
    ▼                      ▼
P32.2 (SafeCounter)    P32.3 (ErrorNest)
    │                      │
    └──────────┬───────────┘
               ▼
        P32.4 (Documentation)
```

P32.1 is the foundation — it surfaces the existing scope infrastructure
on ComponentContext/ChildComponentContext. P32.2 and P32.3 are
independent demos that validate the APIs from P32.1. P32.4 updates
documentation after the demos are validated.

---

## Estimated size

| Step | Description | ~New Lines | Tests |
|------|-------------|-----------|-------|
| P32.1 | Context error boundary surface | ~80 Mojo, ~30 TS | 15 Mojo + 3 JS |
| P32.2 | SafeCounterApp demo | ~350 Mojo, ~120 TS | 18 Mojo + 22 JS |
| P32.3 | ErrorNestApp demo | ~450 Mojo, ~140 TS | 20 Mojo + 25 JS |
| P32.4 | Documentation update | ~0 Mojo, ~50 prose | 0 |
| **Total** | | **~880 Mojo, ~340 TS** | **53 Mojo + 50 JS = 103 tests** |

**Projected test count after P32.4:** ~36+ Mojo modules + ~2,094 JS ≈ 2,094+ tests.
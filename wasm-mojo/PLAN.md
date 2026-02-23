# Phase 31 — Component Props & Context

## Problem

Phase 29 introduced `ChildComponent` for component composition, but the
current pattern has significant limitations:

1. **Children are display-only.** The parent builds the child's VNode
   directly (`child.render_builder()` → `add_dyn_text()` → `build()`),
   which means the parent must know the child's template structure. The
   child has no way to manage its own rendering.

2. **No child-owned state.** `ChildComponent` has its own scope, but
   there is no API to create signals, memos, or effects under that
   scope. All reactive state lives in the parent.

3. **No props mechanism.** Data flows from parent to child only through
   the parent manually building the child's VNode with hardcoded values.
   There is no ergonomic way to pass reactive signal handles as "props"
   that the child can subscribe to.

4. **Context DI is buried.** `ScopeState.provide_context()` and
   `ScopeArena.consume_context()` exist (Phase 8.3) but are not surfaced
   on `ComponentContext`. No app code uses them. The parent-chain walk-up
   is fully implemented but untested at the component level.

5. **No upward communication pattern.** Children cannot notify parents
   of events beyond mutating shared signals (which requires the parent
   to know the child's internal signal keys).

### Current ChildCounterApp pattern (Phase 29)

```mojo
struct ChildCounterApp:
    var ctx: ComponentContext       # parent owns everything
    var count: SignalI32            # parent-owned signal
    var child: ChildComponent       # display-only child

    fn build_child_vnode(mut self) -> UInt32:
        # Parent knows child's template structure — tight coupling
        var cvb = self.child.render_builder(ctx.store_ptr(), ctx.runtime_ptr())
        cvb.add_dyn_text("Count: " + str(self.count.peek()))
        return cvb.build()
```

### Target pattern (Phase 31)

```mojo
struct CounterDisplay:
    var child_ctx: ChildComponentContext
    var count: SignalI32            # received from parent via context
    var show_hex: SignalBool        # child-owned local state

    fn render(mut self) -> UInt32:
        var vb = self.child_ctx.render_builder()
        if self.show_hex.get():
            vb.add_dyn_text("Count: 0x" + hex(self.count.peek()))
        else:
            vb.add_dyn_text("Count: " + str(self.count.peek()))
        return vb.build()

struct ParentApp:
    var ctx: ComponentContext
    var count: SignalI32
    var display: CounterDisplay     # self-rendering child

    fn __init__(out self):
        self.ctx = ComponentContext.create()
        self.count = self.ctx.use_signal(0)
        # Provide count signal to descendants via context
        self.ctx.provide_signal_i32(PROP_COUNT, self.count)
        ...
        # Child creates its own context, consumes the prop
        var child_ctx = self.ctx.create_child_context(
            el_p(dyn_text()), String("display"),
        )
        var received_count = child_ctx.consume_signal_i32(PROP_COUNT)
        var local_toggle = child_ctx.use_signal_bool(False)
        self.display = CounterDisplay(child_ctx^, received_count, local_toggle)
```

---

## Design

### Context keys

Context keys are `UInt32` identifiers chosen by the application. To avoid
collisions, the convention is `comptime` constants:

```mojo
comptime PROP_COUNT: UInt32 = 1
comptime PROP_THEME: UInt32 = 2
comptime CTX_SELECTED_ID: UInt32 = 100
```

Context values are `Int32` — sufficient for signal keys, enum values,
boolean flags, and small integers. Signal handles are reconstructed from
the key + shared runtime pointer.

### ChildComponentContext

A new struct in `src/component/child_context.mojo` that wraps:

- A `ChildComponent` (scope, template, ConditionalSlot, bindings)
- Pointers to the shared `Runtime`, `VNodeStore`, `StringStore`
- The parent's `scope_id` (for context walk-up)
- The child's `scope_id` (for signal/memo creation and dirty tracking)

It provides a subset of `ComponentContext`'s API:

- `use_signal(initial) -> SignalI32` — creates signal under child scope
- `use_signal_bool(initial) -> SignalBool`
- `use_signal_string(initial) -> SignalString`
- `use_memo(initial) -> MemoI32`
- `consume_signal_i32(key) -> SignalI32` — walks up scope chain
- `consume_signal_bool(key) -> SignalBool`
- `consume_signal_string(key) -> SignalString`
- `consume_context(key) -> Int32` — raw context lookup
- `render_builder() -> ChildRenderBuilder`
- `is_dirty() -> Bool`

It does NOT own the `AppShell` — the parent `ComponentContext` does. The
child context holds non-owning pointers to the shared stores, just like
signal handles do.

### Signal sharing via context

Parent provides a signal by storing its key in scope context:

```text
provide_signal_i32(PROP_COUNT, count)
  → self.shell.runtime[0].scopes.provide_context(
        self.scope_id, PROP_COUNT, Int32(count.key))
```

Child consumes it by looking up the key and reconstructing a handle:

```text
consume_signal_i32(PROP_COUNT)
  → var (found, raw_key) = runtime[0].scopes.consume_context(
        self.child_scope_id, PROP_COUNT)
  → return SignalI32(UInt32(raw_key), self.runtime)
```

Since the child scope's `parent_id` is the parent scope's ID (set by
`create_child_scope`), the walk-up finds the parent's context entry
automatically.

### Upward communication

No new primitive is needed. The parent provides a "callback signal" —
a `SignalI32` whose value the child writes to signal an action:

```mojo
# Parent
var on_action = ctx.use_signal(0)  # 0 = no action
ctx.provide_signal_i32(CB_ACTION, on_action)

# Child
var on_action = child_ctx.consume_signal_i32(CB_ACTION)
# In event handler:
on_action.set(ACTION_DELETE)  # marks parent scope dirty
```

The parent checks `on_action.peek()` during flush and resets it. This
is the same pattern as Elm/Redux command signals and works within the
existing reactive system.

### Dirty tracking

Child-owned signals are created under the child scope, so writing them
marks the child scope dirty — not the parent. The parent must check
`child_ctx.is_dirty()` during flush and re-render the child if needed.

Prop signals (parent-owned, consumed by child) mark the *parent* scope
dirty when written (since the parent scope subscribes during its render).
The child reads them via `peek()` during its own render, so the parent
must flush the child whenever the parent itself is dirty. This matches
the existing ChildCounterApp pattern.

---

## Steps

### P31.1 — ComponentContext provide/consume + signal helpers

Surface the scope context DI mechanism on `ComponentContext` and add
typed signal-sharing helpers.

**`src/component/context.mojo` additions:**

```mojo
# ── Context (Dependency Injection) ───────────────────────────────

fn provide_context(mut self, key: UInt32, value: Int32):
    """Provide a context value at the root scope."""
    self.shell.runtime[0].scopes.provide_context(
        self.scope_id, key, value
    )

fn consume_context(self, key: UInt32) -> Tuple[Bool, Int32]:
    """Look up a context value walking up the scope tree."""
    return self.shell.runtime[0].scopes.consume_context(
        self.scope_id, key
    )

fn has_context(self, key: UInt32) -> Bool:
    """Check whether a context value is reachable."""
    return self.consume_context(key)[0]

# ── Signal sharing via context ───────────────────────────────────

fn provide_signal_i32(mut self, key: UInt32, signal: SignalI32):
    """Provide a signal handle to descendants via context."""
    self.provide_context(key, Int32(signal.key))

fn provide_signal_bool(mut self, key: UInt32, signal: SignalBool):
    """Provide a bool signal handle to descendants via context."""
    self.provide_context(key, signal.peek_i32())
    # Store the actual signal key, not the value
    self.shell.runtime[0].scopes.provide_context(
        self.scope_id, key, Int32(signal.key)
    )

fn provide_signal_string(mut self, key: UInt32, signal: SignalString):
    """Provide a string signal handle to descendants via context."""
    self.provide_context(key, Int32(signal.key))

fn consume_signal_i32(self, key: UInt32) -> SignalI32:
    """Look up a SignalI32 from an ancestor's context."""
    var result = self.consume_context(key)
    return SignalI32(UInt32(result[1]), self.shell.runtime)

fn consume_signal_bool(self, key: UInt32) -> SignalBool:
    """Look up a SignalBool from an ancestor's context."""
    var result = self.consume_context(key)
    return SignalBool(UInt32(result[1]), self.shell.runtime)

fn consume_signal_string(self, key: UInt32) -> SignalString:
    """Look up a SignalString from an ancestor's context."""
    var result = self.consume_context(key)
    return SignalString(
        UInt32(result[1]),
        self.shell.runtime,
        self.shell.string_store,
    )
```

**`src/main.mojo` WASM exports (thin wrappers):**

- `ctx_provide_context(app_ptr, key, value)`
- `ctx_consume_context(app_ptr, key) -> Int32`
- `ctx_has_context(app_ptr, key) -> Int32`
- `ctx_provide_signal_i32(app_ptr, key, signal_key)`
- `ctx_consume_signal_i32(app_ptr, key) -> Int32` (returns signal key)

**Tests:**

Mojo (`test/test_context.mojo`, new module, ~15 tests):

- `provide_context` stores value at root scope
- `consume_context` retrieves value from same scope
- `consume_context` walks up parent chain (provide at root, consume at child)
- `consume_context` returns (False, 0) for missing key
- `has_context` returns True/False correctly
- `provide_context` overwrites existing key
- `provide_signal_i32` round-trips through consume
- `provide_signal_bool` round-trips through consume
- `provide_signal_string` round-trips through consume
- consumed signal handle reads correct value
- consumed signal handle writes propagate to parent
- writing consumed signal marks parent scope dirty
- multiple context keys coexist
- context survives across flush cycles
- context cleaned up on destroy

JS (`test-js/context.test.ts`, new file, ~10 tests):

- provide + consume round-trip via WASM exports
- missing key returns 0
- signal sharing via context (provide, consume, read value)
- signal write via consumed handle marks dirty
- overwrite context key
- multiple keys
- child scope consumes parent context
- context across rebuild cycles
- destroy cleanup
- independent instances don't share context

### P31.2 — ChildComponentContext

New struct that gives child components their own reactive state and
access to parent-provided context.

**New file: `src/component/child_context.mojo`**

```mojo
struct ChildComponentContext(Movable):
    """Context for a self-rendering child component.

    Wraps a ChildComponent and provides signal/memo creation under
    the child's scope, context consumption from ancestors, and a
    render_builder() for self-rendering.

    Does NOT own the AppShell — the parent ComponentContext does.
    Holds non-owning pointers to the shared Runtime, VNodeStore,
    StringStore, and ElementIdAllocator.

    Usage:
        var child_ctx = parent_ctx.create_child_context(
            el_p(dyn_text()), String("display"),
        )
        var local_state = child_ctx.use_signal(0)
        var prop = child_ctx.consume_signal_i32(PROP_COUNT)
    """

    var child: ChildComponent
    var scope_id: UInt32
    var runtime: UnsafePointer[Runtime, MutExternalOrigin]
    var store: UnsafePointer[VNodeStore, MutExternalOrigin]
    var string_store: UnsafePointer[StringStore, MutExternalOrigin]
    var eid_alloc: UnsafePointer[ElementIdAllocator, MutExternalOrigin]

    # ── Signal creation (under child scope) ──────────────────────

    fn use_signal(mut self, initial: Int32) -> SignalI32:
        """Create an Int32 signal under the child scope."""

    fn use_signal_bool(mut self, initial: Bool) -> SignalBool:
        """Create a Bool signal under the child scope."""

    fn use_signal_string(mut self, initial: String) -> SignalString:
        """Create a String signal under the child scope."""

    fn use_memo(mut self, initial: Int32) -> MemoI32:
        """Create a memo under the child scope."""

    # ── Context consumption ──────────────────────────────────────

    fn consume_context(self, key: UInt32) -> Tuple[Bool, Int32]:
        """Look up a context value walking up from child scope."""

    fn consume_signal_i32(self, key: UInt32) -> SignalI32:
        """Look up a SignalI32 from an ancestor's context."""

    fn consume_signal_bool(self, key: UInt32) -> SignalBool:
        """Look up a SignalBool from an ancestor's context."""

    fn consume_signal_string(self, key: UInt32) -> SignalString:
        """Look up a SignalString from an ancestor's context."""

    # ── Context provision (at child scope) ───────────────────────

    fn provide_context(mut self, key: UInt32, value: Int32):
        """Provide a context value at the child scope."""

    fn provide_signal_i32(mut self, key: UInt32, signal: SignalI32):
        """Provide a signal to descendants via child scope context."""

    # ── Rendering ────────────────────────────────────────────────

    fn render_builder(self) -> ChildRenderBuilder:
        """Create a ChildRenderBuilder for this child's template."""

    # ── Slot / flush delegation ──────────────────────────────────

    fn init_slot(mut self, anchor_id: UInt32):
        """Initialize the ConditionalSlot after parent mount."""

    fn flush(mut self, writer_ptr, new_vnode_idx: UInt32):
        """Flush the child: create or diff its VNode in the DOM."""

    fn flush_empty(mut self, writer_ptr):
        """Hide the child: remove its DOM content."""

    # ── State queries ────────────────────────────────────────────

    fn is_dirty(self) -> Bool:
        """Check if the child scope or any of its signals changed."""

    fn is_mounted(self) -> Bool:
        """Check whether the child is in the DOM."""

    fn has_rendered(self) -> Bool:
        """Check whether this child has rendered at least once."""

    # ── Destroy ──────────────────────────────────────────────────

    fn destroy(self):
        """Destroy the child scope, its signals, and handlers."""
```

**`src/component/context.mojo` addition:**

```mojo
fn create_child_context(
    mut self, view: Node, name: String,
) -> ChildComponentContext:
    """Create a ChildComponentContext for a self-rendering child.

    Same as create_child_component() but returns a richer context
    that supports signal creation, context consumption, and
    self-rendering.
    """
    var child = self.create_child_component(view, name)
    return ChildComponentContext(
        child^,
        child.scope_id,
        self.shell.runtime,
        self.shell.store,
        self.shell.string_store,
        self.shell.eid_alloc,
    )

fn destroy_child_context(mut self, child_ctx: ChildComponentContext):
    """Destroy a ChildComponentContext and its resources."""
    child_ctx.destroy()
```

**Signal creation under child scope:**

The child scope already exists (via `create_child_scope`). To create
signals under it, `ChildComponentContext.use_signal()` directly calls
`runtime[0].create_signal(initial)` and pushes a hook onto the child
scope. The signal key is stored in the child scope's hook storage,
matching the pattern in `ComponentContext.use_signal()` but targeting
the child scope ID instead of the root scope.

Implementation note: `ComponentContext.use_signal()` calls
`self.shell.use_signal_i32(initial)` which creates the signal in the
store and pushes a hook at the current scope (tracked via
`begin_render`/`end_render`). For `ChildComponentContext`, we bypass
AppShell and call `Runtime` methods directly, explicitly targeting the
child scope ID. No render bracket is needed — signals are always
created (never re-retrieved via cursor) because child components
don't re-run their setup.

**Exports and updates:**

- Export `ChildComponentContext` from `component/__init__.mojo`
- Add WASM exports for child context signal creation and context
  consumption (used by test harness)

**Tests:**

Mojo (`test/test_child_context.mojo`, new module, ~20 tests):

- create child context returns valid scope/template
- child scope is distinct from parent scope
- `use_signal` creates signal under child scope
- `use_signal_bool` under child scope
- `use_signal_string` under child scope
- `use_memo` under child scope
- child signal write marks child scope dirty (not parent)
- child signal write does not mark parent dirty
- parent signal write marks parent dirty (not child)
- `consume_signal_i32` retrieves parent-provided signal
- `consume_signal_bool` retrieves parent-provided signal
- consumed signal reads correct value
- consumed signal write propagates to parent scope
- `render_builder` produces valid VNode
- `is_dirty` reflects child scope state
- `init_slot` + `flush` produces mutations
- `flush` on clean child returns 0
- child context provides context to grandchild
- destroy cleans up child scope and signals
- destroy then recreate cycle

JS (`test-js/child_context.test.ts`, new file, ~15 tests):

- create child context, verify scope/template IDs
- child `use_signal` independent from parent signals
- child signal write → child dirty, parent clean
- parent signal write → parent dirty, child clean
- context prop round-trip (provide at parent, consume at child)
- child self-render via render_builder produces correct VNode
- DOM mount with child context (parent + child visible)
- child local state update → only child SetText mutation
- parent prop update → child re-renders with new value
- mixed local + prop updates in single flush
- destroy does not crash
- destroy + recreate cycle
- multiple independent child contexts
- rapid signal writes bounded memory
- child provides context to sibling (negative — siblings can't see
  each other's context)

### P31.3 — PropsCounterApp demo (self-rendering child with props)

A new demo app that replaces the Phase 29 `ChildCounterApp` pattern
with the new `ChildComponentContext` approach. The child component:

- Receives the `count` signal from the parent via context (prop)
- Owns local state: `show_hex: SignalBool` (display format toggle)
- Renders itself via `child_ctx.render_builder()`
- Has its own event handler (toggle button) under the child scope

**`src/main.mojo` additions:**

```mojo
comptime PROP_COUNT: UInt32 = 1

struct CounterDisplay:
    """Self-rendering child: displays count with format toggle."""
    var child_ctx: ChildComponentContext
    var count: SignalI32           # consumed from parent context
    var show_hex: SignalBool       # child-owned local state

    fn __init__(
        out self,
        var child_ctx: ChildComponentContext,
        count: SignalI32,
        show_hex: SignalBool,
    ):
        self.child_ctx = child_ctx^
        self.count = count
        self.show_hex = show_hex

    fn render(mut self) -> UInt32:
        var vb = self.child_ctx.render_builder()
        var val = self.count.peek()
        if self.show_hex.get():
            vb.add_dyn_text("Count: 0x" + hex(val))
        else:
            vb.add_dyn_text("Count: " + str(val))
        return vb.build()

struct PropsCounterApp:
    """Counter app demonstrating props & child-owned state."""
    var ctx: ComponentContext
    var count: SignalI32
    var display: CounterDisplay

    fn __init__(out self):
        self.ctx = ComponentContext.create()
        self.count = self.ctx.use_signal(0)
        # Provide count to descendants
        self.ctx.provide_signal_i32(PROP_COUNT, self.count)
        self.ctx.setup_view(
            el_div(
                el_h1(text("Props Counter")),
                el_button(text("+ 1"), onclick_add(self.count, 1)),
                el_button(text("- 1"), onclick_sub(self.count, 1)),
                dyn_node(0),     # child slot
            ),
            String("props-counter"),
        )
        # Create self-rendering child with format toggle
        var child_ctx = self.ctx.create_child_context(
            el_div(
                el_p(dyn_text()),
                el_button(text("Toggle hex"), onclick_toggle(...)),
            ),
            String("counter-display"),
        )
        var prop_count = child_ctx.consume_signal_i32(PROP_COUNT)
        var show_hex = child_ctx.use_signal_bool(False)
        self.display = CounterDisplay(child_ctx^, prop_count, show_hex)
```

**Template structure:**

Parent ("props-counter"):

```text
div
  h1 > "Props Counter"
  button > "+" + dynamic_attr[0]  ← onclick_add
  button > "-" + dynamic_attr[1]  ← onclick_sub
  dyn_node[0]                     ← child slot
```

Child ("counter-display"):

```text
div
  p > dynamic_text[0]             ← "Count: N" or "Count: 0xN"
  button > "Toggle hex"
    dynamic_attr[0]               ← onclick_toggle(show_hex)
```

**Lifecycle functions:**

- `props_counter_init` → allocate + construct `PropsCounterApp`
- `props_counter_destroy` → destroy child context + parent context + free
- `props_counter_rebuild` → emit templates, mount parent, extract anchor,
  init child slot, flush child (initial render), finalize
- `props_counter_handle_event` → try child toggle handler, fall back to
  parent signal dispatch
- `props_counter_flush` → check parent dirty OR child dirty; re-render
  parent shell (diff), re-render child (self-render + flush), finalize

**WASM exports (~16):**

- `pc_init`, `pc_destroy`, `pc_rebuild`, `pc_handle_event`, `pc_flush`
- `pc_count_value`, `pc_show_hex`, `pc_toggle_handler`
- `pc_child_scope_id`, `pc_child_tmpl_id`, `pc_child_is_mounted`
- `pc_child_is_dirty`, `pc_parent_scope_id`, `pc_parent_tmpl_id`
- `pc_has_dirty`, `pc_handler_count`

**`runtime/app.ts` additions:**

```typescript
interface PropsCounterAppHandle extends AppHandle {
    getCountValue(): number;
    getShowHex(): boolean;
    toggleHex(): void;
    isChildMounted(): boolean;
    isChildDirty(): boolean;
    getChildScopeId(): number;
}

function createPropsCounterApp(): PropsCounterAppHandle;
```

**Tests:**

Mojo (`test/test_props_counter.mojo`, new module, ~15 tests):

- init creates app with distinct parent/child scopes
- count starts at 0, show_hex starts false
- increment updates count signal
- child receives count via context prop
- child show_hex toggle changes child state
- child show_hex toggle marks child dirty (not parent)
- parent increment marks parent dirty
- child self-render produces correct text ("Count: 0")
- child self-render with hex ("Count: 0x0")
- flush after increment emits child SetText
- flush after toggle emits child SetText (format change)
- mixed increment + toggle in sequence
- destroy cleans up both scopes
- destroy + recreate cycle
- 10 rapid increment cycles

JS (`test-js/props_counter.test.ts`, new file, ~20 tests):

- pc_init state validation (scope IDs, handler IDs, initial values)
- pc_rebuild produces mutations (RegisterTemplate ×2, mount, child create)
- increment updates parent signal, child re-renders with new count
- toggle hex changes display format without affecting count
- toggle hex marks only child dirty
- increment marks only parent dirty
- DOM mount: parent div with h1 + 2 buttons + child div with p + button
- increment → child text updates ("Count: 1")
- toggle → child text updates ("Count: 0x0")
- increment after toggle → hex format preserved ("Count: 0x1")
- 10 increments → correct count
- toggle on → toggle off → decimal restored
- flush returns 0 when clean
- minimal mutations: only SetText for unchanged parent shell
- destroy does not crash
- double destroy safe
- destroy + recreate cycle
- multiple independent instances
- rapid 100 increments bounded memory
- child toggle does not affect parent DOM

### P31.4 — SharedContext app + cross-component tests

A more complex demo showing multiple children sharing parent context
and communicating upward via callback signals. Validates the full
props & context system end-to-end.

#### App structure: ThemeCounter

A parent app with a theme toggle (dark/light) and two child components
that both consume the theme context:

- **CounterChild**: displays count with theme-dependent label
- **SummaryChild**: displays summary text ("N clicks so far") with
  theme-dependent styling class

The parent also receives upward communication: CounterChild has a
"Reset" button that writes to a callback signal consumed by the parent.

```mojo
comptime CTX_THEME: UInt32 = 10       # 0 = light, 1 = dark
comptime CTX_COUNT: UInt32 = 11       # count signal key
comptime CTX_ON_RESET: UInt32 = 12    # callback signal key

struct ThemeCounterApp:
    var ctx: ComponentContext
    var count: SignalI32
    var theme: SignalBool           # False = light, True = dark
    var on_reset: SignalI32         # callback: child writes 1 to request reset

    var counter_child: CounterChild
    var summary_child: SummaryChild
```

**Parent template ("theme-counter"):**

```text
div
  button > "Toggle theme" + dynamic_attr[0]  ← onclick_toggle(theme)
  button > "Increment"    + dynamic_attr[1]  ← onclick_add(count, 1)
  dyn_node[0]             ← counter child slot
  dyn_node[1]             ← summary child slot
```

**CounterChild template ("counter-display"):**

```text
div
  p > dynamic_text[0]                        ← "Count: N" or "Theme: dark, Count: N"
  button > "Reset" + dynamic_attr[0]         ← onclick: set on_reset to 1
```

**SummaryChild template ("summary-display"):**

```text
p
  dynamic_text[0]                            ← "N clicks so far"
  dynamic_attr[0]                            ← class = "light" or "dark"
```

**Lifecycle:**

- Parent provides `CTX_THEME`, `CTX_COUNT`, `CTX_ON_RESET` via context
- Each child consumes what it needs
- CounterChild consumes `CTX_COUNT` + `CTX_THEME` + `CTX_ON_RESET`
- SummaryChild consumes `CTX_COUNT` + `CTX_THEME`
- Parent flush checks `on_reset.peek()`: if 1, resets count to 0 and
  clears the callback signal
- Each child self-renders; parent flushes both children

**WASM exports (~20):**

- `tc_init`, `tc_destroy`, `tc_rebuild`, `tc_handle_event`, `tc_flush`
- `tc_count_value`, `tc_theme_is_dark`, `tc_on_reset_value`
- `tc_counter_scope_id`, `tc_summary_scope_id`
- `tc_counter_is_mounted`, `tc_summary_is_mounted`
- `tc_counter_is_dirty`, `tc_summary_is_dirty`
- `tc_toggle_theme_handler`, `tc_increment_handler`, `tc_reset_handler`
- `tc_counter_child_text`, `tc_summary_child_text`
- `tc_has_dirty`, `tc_handler_count`

**`runtime/app.ts` additions:**

```typescript
interface ThemeCounterAppHandle extends AppHandle {
    getCountValue(): number;
    isDarkTheme(): boolean;
    toggleTheme(): void;
    increment(): void;
    resetViaChild(): void;
    getCounterText(): string;
    getSummaryText(): string;
    isCounterMounted(): boolean;
    isSummaryMounted(): boolean;
}

function createThemeCounterApp(): ThemeCounterAppHandle;
```

**Tests:**

Mojo (`test/test_theme_counter.mojo`, new module, ~20 tests):

- init creates 3 distinct scopes (parent, counter child, summary child)
- count starts at 0, theme starts light
- increment updates count
- both children consume same count signal
- theme toggle updates theme signal
- counter child reads theme from context
- summary child reads theme from context
- callback signal starts at 0
- child reset writes to callback signal
- parent flush detects reset callback and clears count
- counter child self-render with light theme
- counter child self-render with dark theme
- summary child self-render with light theme
- summary child self-render with dark theme
- mixed: increment + toggle theme + flush
- reset callback: count returns to 0
- multiple increment → reset → increment cycle
- destroy cleans up all 3 scopes
- 10 create/destroy cycles bounded memory
- rapid 50 increments + 10 theme toggles

JS (`test-js/theme_counter.test.ts`, new file, ~25 tests):

- tc_init state validation (3 scopes, handler IDs, initial values)
- tc_rebuild mounts parent + both children
- increment → both children update
- theme toggle → both children update format/class
- DOM: parent div with 2 buttons + counter child div + summary p
- increment → counter shows "Count: 1", summary shows "1 clicks"
- theme toggle → counter shows "Theme: dark, Count: 0"
- summary gets "dark" class after theme toggle
- reset button → count returns to 0, both children update
- increment → reset → increment cycle
- theme toggle does not affect count
- both children re-render on count change
- only counter child has reset button handler
- children have independent scope IDs
- flush returns 0 when clean
- minimal mutations per interaction
- destroy does not crash
- double destroy safe
- destroy + recreate cycle
- multiple independent instances
- rapid increments bounded memory
- theme toggle + increment in same flush
- reset + increment in same flush (reset wins — count goes to 0, then +1 = 1)
- 5 create/destroy cycles with state verification
- children not dirty when parent-only change already flushed

---

## Dependency graph

```text
P31.1 (Context surface)
    │
    ▼
P31.2 (ChildComponentContext)
    │
    ├──────────────────────┐
    ▼                      ▼
P31.3 (PropsCounter)    P31.4 (ThemeCounter)
```

P31.1 is the foundation — it surfaces the existing DI mechanism. P31.2
builds on it with child-scope signal creation. P31.3 and P31.4 are
independent demos that validate the APIs from P31.1 and P31.2.

---

## Estimated size

| Step | Description | ~New Lines | Tests |
|------|-------------|-----------|-------|
| P31.1 | Context surface + signal helpers | ~120 Mojo, ~80 TS | 15 Mojo + 10 JS |
| P31.2 | ChildComponentContext struct | ~350 Mojo, ~60 TS | 20 Mojo + 15 JS |
| P31.3 | PropsCounterApp demo | ~280 Mojo, ~100 TS | 15 Mojo + 20 JS |
| P31.4 | ThemeCounterApp demo | ~350 Mojo, ~120 TS | 20 Mojo + 25 JS |
| **Total** | | **~1,100 Mojo, ~360 TS** | **70 Mojo + 70 JS = 140 tests** |

**Projected test count after P31.4:** ~1,070 Mojo + ~1,854 JS = ~2,924 tests.
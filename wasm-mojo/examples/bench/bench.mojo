# BenchmarkApp — js-framework-benchmark implementation.
#
# Phase 24.2: WASM-rendered toolbar with onclick_custom handlers.
#
# The entire app shell — heading, toolbar buttons, status bar, table
# structure — is now rendered from WASM via register_view().  The 6
# toolbar buttons use onclick_custom() handlers auto-registered by
# register_view(), with handler IDs extracted via view_event_handler_id().
# handle_event() routes each button's handler ID to the corresponding
# benchmark operation (create 1k/10k, append, update, swap, clear).
#
# This eliminates all toolbar button wiring JS from bench/main.js.
# The only remaining JS is the launch() call with bufferCapacity.
#
# Migrated to Phase 18 Dioxus-style ergonomics:
#   - `begin_item()` replaces manual `create_scope()` + `item_builder()`
#   - `add_custom_event()` replaces manual `register_handler()` + `add_dyn_event()`
#   - `add_class_if()` replaces 5-line if/else class pattern (Phase 18)
#   - Multi-arg el_* overloads (no List[Node]() wrappers)
#   - KeyedList abstraction (bundles FragmentSlot + scope IDs + template ID + handler map)
#   - Constructor-based setup (all init in __init__)
#   - ctx.use_signal() for automatic scope subscription
#   - SignalI32 handles with operator overloading
#
# Phase 24.2: register_view() with inline onclick_custom() for toolbar
#   - App shell template "bench-app" includes h1, 6 buttons, status, table
#   - 6 onclick_custom() handlers auto-registered by register_view()
#   - handle_event() routes toolbar buttons AND row clicks
#   - Root changed from #tbody to #root — WASM renders entire container
#   - render() uses render_builder() with dyn_text (status) + dyn_placeholder (rows)
#   - rebuild/flush follow the todo pattern (mount shell + init keyed list)
#
# Implements the standard benchmark operations:
#   - Create N rows
#   - Append N rows
#   - Update every 10th row
#   - Select row (highlight)
#   - Swap rows (indices 1 and 998)
#   - Remove row
#   - Clear all rows
#
# Each row has: id (int), label (string).
# The selected row id is tracked separately.
#
# Template structure (Phase 24.2 — WASM-rendered shell):
#
#   "bench-app" (via register_view with auto dyn_attr):
#     div.container > [
#       h1 > text("🔥 Mojo WASM — js-framework-benchmark"),
#       div.controls > [
#         button.btn-create("Create 1,000 rows", onclick_custom()),    -- dyn_attr[0]
#         button.btn-create10k("Create 10,000 rows", onclick_custom()),-- dyn_attr[1]
#         button.btn-append("Append 1,000 rows", onclick_custom()),    -- dyn_attr[2]
#         button.btn-update("Update every 10th row", onclick_custom()),-- dyn_attr[3]
#         button.btn-swap("Swap rows", onclick_custom()),              -- dyn_attr[4]
#         button.btn-clear("Clear", onclick_custom()),                 -- dyn_attr[5]
#       ],
#       div.status > dyn_text[0],         -- status message (dynamic_nodes[0])
#       div.table-wrap > table > [
#         thead > tr > [th("#"), th("Label"), th("Action")],
#         tbody > dyn_node[1],            -- keyed row list (dynamic_nodes[1])
#       ],
#     ]
#
#   "bench-row" (via register_extra_template):
#     tr + dynamic_attr[0](class) > [
#       td > dynamic_text[0] (id),
#       td > a > dynamic_text[1] (label),
#       td > a > text("×")
#     ]
#     dynamic_attr[1] = click on label (select)
#     dynamic_attr[2] = click on delete button (remove)
#
# We use a simple linear congruential generator for pseudo-random labels
# to match the benchmark's adjective + colour + noun pattern.
#
# Compare with Dioxus (Rust):
#
#     fn App() -> Element {
#         let mut rows = use_signal(|| vec![]);
#         let mut selected = use_signal(|| 0usize);
#         rsx! {
#             div { class: "container",
#                 h1 { "🔥 Mojo WASM — js-framework-benchmark" }
#                 div { class: "controls",
#                     button { class: "btn-create",
#                              onclick: move |_| create(1000), "Create 1,000 rows" }
#                     button { class: "btn-create10k",
#                              onclick: move |_| create(10000), "Create 10,000 rows" }
#                     // ... etc
#                 }
#                 div { class: "status", "{status}" }
#                 div { class: "table-wrap",
#                     table {
#                         thead { tr { th { "#" } th { "Label" } th { "Action" } } }
#                         tbody {
#                             for row in rows.read().iter() {
#                                 tr { class: if selected() == row.id { "danger" },
#                                     td { "{row.id}" }
#                                     td { a { onclick: move |_| selected.set(row.id),
#                                              "{row.label}" } }
#                                     td { a { onclick: move |_| remove(row.id), "×" } }
#                                 }
#                             }
#                         }
#                     }
#                 }
#             }
#         }
#     }
#
# Mojo equivalent (with Phase 24.2 register_view):
#
#     struct BenchmarkApp:
#         var ctx: ComponentContext
#         var version: SignalI32
#         var selected: SignalI32
#         var rows_list: KeyedList
#         var create1k_handler: UInt32
#         # ... 5 more handler IDs
#
#         fn __init__(out self):
#             self.ctx = ComponentContext.create()
#             self.version = self.ctx.use_signal(0)
#             self.selected = self.ctx.use_signal(0)
#             self.ctx.setup_view(
#                 el_div(
#                     attr("class", "container"),
#                     el_h1(text("🔥 ...")),
#                     controls_div,  # List[Node] with 6 buttons
#                     el_div(attr("class", "status"), dyn_text()),
#                     el_div(attr("class", "table-wrap"), table),
#                 ),
#                 String("bench-app"),
#             )
#             self.create1k_handler = self.ctx.view_event_handler_id(0)
#             # ...
#             self.rows_list = KeyedList(ctx.register_extra_template(...))
#
#         fn handle_event(mut self, handler_id: UInt32) -> Bool:
#             if handler_id == self.create1k_handler:
#                 self.create_rows(1000)
#                 return True
#             # ... other buttons ...
#             var action = self.rows_list.get_action(handler_id)
#             if action.found:
#                 # select / remove
#             return False

from memory import UnsafePointer
from bridge import MutationWriter
from mutations import CreateEngine

from component import ComponentContext, KeyedList
from signals import SignalI32
from vdom import (
    VNode,
    VNodeStore,
    Node,
    el_div,
    el_h1,
    el_button,
    el_table,
    el_thead,
    el_tbody,
    el_tr,
    el_td,
    el_th,
    el_a,
    text,
    dyn_text,
    dyn_node,
    dyn_attr,
    attr,
    onclick_custom,
    to_template,
    VNodeBuilder,
)


struct BenchRow(Copyable, Movable):
    """A single benchmark table row."""

    var id: Int32
    var label: String

    fn __init__(out self, id: Int32, label: String):
        self.id = id
        self.label = label

    fn __copyinit__(out self, other: Self):
        self.id = other.id
        self.label = other.label

    fn __moveinit__(out self, deinit other: Self):
        self.id = other.id
        self.label = other.label^


# ── Label generation ─────────────────────────────────────────────────────────

alias _ADJ_COUNT: Int = 12
alias _COL_COUNT: Int = 11
alias _NOUN_COUNT: Int = 12


fn _adjective(idx: Int) -> String:
    if idx == 0:
        return "pretty"
    elif idx == 1:
        return "large"
    elif idx == 2:
        return "big"
    elif idx == 3:
        return "small"
    elif idx == 4:
        return "tall"
    elif idx == 5:
        return "short"
    elif idx == 6:
        return "long"
    elif idx == 7:
        return "handsome"
    elif idx == 8:
        return "plain"
    elif idx == 9:
        return "quaint"
    elif idx == 10:
        return "clean"
    else:
        return "elegant"


fn _colour(idx: Int) -> String:
    if idx == 0:
        return "red"
    elif idx == 1:
        return "yellow"
    elif idx == 2:
        return "blue"
    elif idx == 3:
        return "green"
    elif idx == 4:
        return "pink"
    elif idx == 5:
        return "brown"
    elif idx == 6:
        return "purple"
    elif idx == 7:
        return "orange"
    elif idx == 8:
        return "white"
    elif idx == 9:
        return "black"
    else:
        return "grey"


fn _noun(idx: Int) -> String:
    if idx == 0:
        return "table"
    elif idx == 1:
        return "chair"
    elif idx == 2:
        return "house"
    elif idx == 3:
        return "bbq"
    elif idx == 4:
        return "desk"
    elif idx == 5:
        return "car"
    elif idx == 6:
        return "pony"
    elif idx == 7:
        return "cookie"
    elif idx == 8:
        return "sandwich"
    elif idx == 9:
        return "burger"
    elif idx == 10:
        return "pizza"
    else:
        return "mouse"


# App-defined action tags for ItemBuilder.add_custom_event() dispatch.
# These are stored in the KeyedList's handler_map and retrievable via
# get_action().  Used by handle_event() for WASM-side row event dispatch.
alias BENCH_ACTION_SELECT: UInt8 = 1
alias BENCH_ACTION_REMOVE: UInt8 = 2


struct BenchmarkApp(Movable):
    """Js-framework-benchmark app state.

    Phase 24.2: Full WASM-rendered app shell with toolbar buttons.

    All setup — context creation, signal creation, template registration,
    and toolbar handler binding — happens in __init__.  The lifecycle
    functions are thin delegations to ComponentContext.

    Uses register_view() for the app shell template with 6 onclick_custom()
    handlers auto-registered for toolbar buttons.  Uses KeyedList with
    Phase 17 ItemBuilder for ergonomic per-item row building.

    handle_event() routes both toolbar button clicks (create, append,
    update, swap, clear) and row clicks (select, remove) in WASM.

    Manages a list of rows, selection state, and all rendering
    infrastructure via ComponentContext (runtime, templates, vnode
    store, etc.).
    """

    var ctx: ComponentContext
    var version: SignalI32  # bumped on list changes
    var selected: SignalI32  # currently selected row id (0 = none)
    var rows_list: KeyedList  # bundles template_id + FragmentSlot + scope_ids + handler_map
    var rows: List[BenchRow]
    var next_id: Int32
    var rng_state: UInt32  # simple LCG state
    # Toolbar handler IDs (auto-registered by register_view)
    var create1k_handler: UInt32
    var create10k_handler: UInt32
    var append_handler: UInt32
    var update_handler: UInt32
    var swap_handler: UInt32
    var clear_handler: UInt32

    fn __init__(out self):
        """Initialize the benchmark app with all reactive state, templates,
        and toolbar handlers.

        Creates: ComponentContext (runtime, VNode store, element ID
        allocator, scheduler), root scope, version and selected signals,
        the app shell template (with 6 onclick_custom toolbar buttons),
        and the row template via KeyedList.

        Phase 24.2: Uses setup_view() for the app shell template.
        The app shell includes the heading, 6 toolbar buttons with
        onclick_custom() handlers, a status dyn_text, and the table
        structure with dyn_node(0) for the keyed row list.

        Template "bench-app" (via setup_view with auto dyn_attr):
            div.container > [
                h1 > text(...),
                div.controls > [6 buttons with onclick_custom()],
                div.status > dyn_text[0],              (dynamic_nodes[0])
                div.table-wrap > table > [thead, tbody > dyn_node[1]],  (dynamic_nodes[1])
            ]
            auto dyn_attr[0] = onclick_custom (Create 1k)
            auto dyn_attr[1] = onclick_custom (Create 10k)
            auto dyn_attr[2] = onclick_custom (Append)
            auto dyn_attr[3] = onclick_custom (Update)
            auto dyn_attr[4] = onclick_custom (Swap)
            auto dyn_attr[5] = onclick_custom (Clear)

        Template "bench-row" (via register_extra_template):
            tr + dyn_attr[0](class) > [
                td > dyn_text[0],
                td > a + dyn_attr[1] > dyn_text[1],
                td > a + dyn_attr[2] > text("×")
            ]

        Uses multi-arg el_* overloads — no List[Node]() wrappers needed
        (except for the controls div which has 7 children: attr + 6 buttons).
        """
        # 1. Create context and signals
        self.ctx = ComponentContext.create()
        self.version = self.ctx.use_signal(0)
        self.selected = self.ctx.use_signal(0)

        # 2. Build controls div with List[Node] (attr + 6 buttons = 7 items)
        var controls = List[Node]()
        controls.append(attr(String("class"), String("controls")))
        controls.append(
            el_button(
                attr(String("class"), String("btn-create")),
                text(String("Create 1,000 rows")),
                onclick_custom(),
            )
        )
        controls.append(
            el_button(
                attr(String("class"), String("btn-create10k")),
                text(String("Create 10,000 rows")),
                onclick_custom(),
            )
        )
        controls.append(
            el_button(
                attr(String("class"), String("btn-append")),
                text(String("Append 1,000 rows")),
                onclick_custom(),
            )
        )
        controls.append(
            el_button(
                attr(String("class"), String("btn-update")),
                text(String("Update every 10th row")),
                onclick_custom(),
            )
        )
        controls.append(
            el_button(
                attr(String("class"), String("btn-swap")),
                text(String("Swap rows")),
                onclick_custom(),
            )
        )
        controls.append(
            el_button(
                attr(String("class"), String("btn-clear")),
                text(String("Clear")),
                onclick_custom(),
            )
        )
        var controls_div = el_div(controls^)

        # 3. Register the "bench-app" shell template via setup_view()
        #    dyn_text() and dyn_node() share the dynamic_nodes index space.
        #    Auto-numbered dyn_text() gets index 0, so dyn_node must be 1.
        #    setup_view() calls end_setup() + register_view() internally.
        self.ctx.setup_view(
            el_div(
                attr(String("class"), String("container")),
                el_h1(text(String("🔥 Mojo WASM — js-framework-benchmark"))),
                controls_div^,
                el_div(
                    attr(String("class"), String("status")),
                    dyn_text(),
                ),
                el_div(
                    attr(String("class"), String("table-wrap")),
                    el_table(
                        el_thead(
                            el_tr(
                                el_th(text(String("#"))),
                                el_th(text(String("Label"))),
                                el_th(text(String("Action"))),
                            )
                        ),
                        el_tbody(dyn_node(1)),
                    ),
                ),
            ),
            String("bench-app"),
        )

        # 4. Extract handler IDs for the 6 toolbar buttons (tree-walk order)
        self.create1k_handler = self.ctx.view_event_handler_id(0)
        self.create10k_handler = self.ctx.view_event_handler_id(1)
        self.append_handler = self.ctx.view_event_handler_id(2)
        self.update_handler = self.ctx.view_event_handler_id(3)
        self.swap_handler = self.ctx.view_event_handler_id(4)
        self.clear_handler = self.ctx.view_event_handler_id(5)

        # 5. Register the "bench-row" template via KeyedList
        self.rows_list = KeyedList(
            self.ctx.register_extra_template(
                el_tr(
                    dyn_attr(0),  # class on <tr>
                    el_td(dyn_text(0)),
                    el_td(el_a(dyn_attr(1), dyn_text(1))),
                    el_td(el_a(dyn_attr(2), text(String("×")))),
                ),
                String("bench-row"),
            )
        )

        # 6. Initialize remaining state
        self.rows = List[BenchRow]()
        self.next_id = 1
        self.rng_state = 42

    fn __moveinit__(out self, deinit other: Self):
        self.ctx = other.ctx^
        self.version = other.version^
        self.selected = other.selected^
        self.rows_list = other.rows_list^
        self.rows = other.rows^
        self.next_id = other.next_id
        self.rng_state = other.rng_state
        self.create1k_handler = other.create1k_handler
        self.create10k_handler = other.create10k_handler
        self.append_handler = other.append_handler
        self.update_handler = other.update_handler
        self.swap_handler = other.swap_handler
        self.clear_handler = other.clear_handler

    fn _next_random(mut self) -> UInt32:
        """Simple LCG: state = state * 1664525 + 1013904223."""
        self.rng_state = self.rng_state * 1664525 + 1013904223
        return self.rng_state

    fn _generate_label(mut self) -> String:
        """Generate a random "adjective colour noun" label."""
        var a = Int(self._next_random() % _ADJ_COUNT)
        var c = Int(self._next_random() % _COL_COUNT)
        var n = Int(self._next_random() % _NOUN_COUNT)
        return _adjective(a) + " " + _colour(c) + " " + _noun(n)

    fn _bump_version(mut self):
        """Increment the version signal to trigger re-render."""
        self.version += 1

    fn create_rows(mut self, count: Int):
        """Replace all rows with `count` newly generated rows."""
        self.rows = List[BenchRow]()
        for _ in range(count):
            var label = self._generate_label()
            self.rows.append(BenchRow(self.next_id, label))
            self.next_id += 1
        self._bump_version()

    fn append_rows(mut self, count: Int):
        """Append `count` newly generated rows to the list."""
        for _ in range(count):
            var label = self._generate_label()
            self.rows.append(BenchRow(self.next_id, label))
            self.next_id += 1
        self._bump_version()

    fn update_every_10th(mut self):
        """Append " !!!" to every 10th row's label."""
        var i = 0
        while i < len(self.rows):
            self.rows[i].label = self.rows[i].label + " !!!"
            i += 10
        self._bump_version()

    fn select_row(mut self, id: Int32):
        """Select the row with the given id."""
        self.selected.set(id)

    fn swap_rows(mut self, a: Int, b: Int):
        """Swap two rows by their list indices."""
        if a < 0 or b < 0 or a >= len(self.rows) or b >= len(self.rows):
            return
        if a == b:
            return
        var tmp = self.rows[a].copy()
        self.rows[a] = self.rows[b].copy()
        self.rows[b] = tmp.copy()
        self._bump_version()

    fn remove_row(mut self, id: Int32):
        """Remove a row by id."""
        for i in range(len(self.rows)):
            if self.rows[i].id == id:
                var last = len(self.rows) - 1
                if i != last:
                    self.rows[i] = self.rows[last].copy()
                _ = self.rows.pop()
                self._bump_version()
                return

    fn clear_rows(mut self):
        """Remove all rows."""
        self.rows = List[BenchRow]()
        self._bump_version()

    fn handle_event(mut self, handler_id: UInt32) -> Bool:
        """Dispatch an event by handler ID.

        Phase 24.2: Routes both toolbar button clicks and row clicks.

        Toolbar buttons (from register_view onclick_custom handlers):
          - create1k_handler  → create_rows(1000)
          - create10k_handler → create_rows(10000)
          - append_handler    → append_rows(1000)
          - update_handler    → update_every_10th()
          - swap_handler      → swap_rows(1, 998)
          - clear_handler     → clear_rows()

        Row clicks (from KeyedList handler_map, Phase 24.1):
          - BENCH_ACTION_SELECT → select_row(id)
          - BENCH_ACTION_REMOVE → remove_row(id)

        Returns True if the handler was found and the action executed,
        False otherwise.
        """
        # Toolbar button routing
        if handler_id == self.create1k_handler:
            self.create_rows(1000)
            return True
        elif handler_id == self.create10k_handler:
            self.create_rows(10000)
            return True
        elif handler_id == self.append_handler:
            self.append_rows(1000)
            return True
        elif handler_id == self.update_handler:
            self.update_every_10th()
            return True
        elif handler_id == self.swap_handler:
            self.swap_rows(1, 998)
            return True
        elif handler_id == self.clear_handler:
            self.clear_rows()
            return True

        # Row event routing (select/remove via KeyedList handler_map)
        var action = self.rows_list.get_action(handler_id)
        if not action.found:
            return False
        if action.tag == BENCH_ACTION_SELECT:
            self.select_row(action.data)
            return True
        elif action.tag == BENCH_ACTION_REMOVE:
            self.remove_row(action.data)
            return True
        return False

    fn build_row_vnode(mut self, row: BenchRow) -> UInt32:
        """Build a keyed VNode for a single benchmark row.

        Uses Phase 17 ItemBuilder + Phase 18 conditional helpers:
          - `begin_item()` creates child scope + keyed VNodeBuilder
          - `add_custom_event()` registers handler + maps action + adds event attr
          - `add_class_if()` replaces 5-line if/else class pattern

        Template "bench-row": tr > [ td(id), td > a(label), td > a("×") ]
          dynamic_text[0] = row id
          dynamic_text[1] = row label
          dynamic_attr[0] = class on <tr> ("danger" if selected)
          dynamic_attr[1] = click on label <a> (select) → BENCH_ACTION_SELECT
          dynamic_attr[2] = click on delete <a> (remove) → BENCH_ACTION_REMOVE
        """
        var ib = self.rows_list.begin_item(String(row.id), self.ctx)

        # Dynamic text 0: row id
        ib.add_dyn_text(String(row.id))

        # Dynamic text 1: row label
        ib.add_dyn_text(row.label)

        # Dynamic attr 0: class on <tr> ("danger" if selected)
        ib.add_class_if(self.selected.peek() == row.id, String("danger"))

        # Dynamic attr 1: click on label <a> (select — custom handler)
        ib.add_custom_event(String("click"), BENCH_ACTION_SELECT, row.id)

        # Dynamic attr 2: click on delete <a> (remove — custom handler)
        ib.add_custom_event(String("click"), BENCH_ACTION_REMOVE, row.id)

        return ib.index()

    fn build_rows_fragment(mut self) -> UInt32:
        """Build a Fragment VNode containing all row VNodes.

        Uses KeyedList.begin_rebuild() to destroy old child scopes,
        clear the handler map, and create a new empty fragment, then
        builds each row VNode and pushes it as a fragment child.
        """
        var frag_idx = self.rows_list.begin_rebuild(self.ctx)
        for i in range(len(self.rows)):
            var row_idx = self.build_row_vnode(self.rows[i].copy())
            self.rows_list.push_child(self.ctx, frag_idx, row_idx)
        return frag_idx

    fn render(mut self) -> UInt32:
        """Build the app shell VNode using render_builder (auto-populates).

        Phase 24.2: Uses render_builder() which auto-populates all
        event handlers registered by setup_view() in tree-walk order:
          auto dyn_attr[0..5] = onclick_custom handlers (6 toolbar buttons)
          dyn_text[0]         = status message          (dynamic_nodes[0])
          dyn_node[1]         = placeholder (row list)  (dynamic_nodes[1])

        Note: dyn_text and dyn_node share the dynamic_nodes index space.
        Auto-numbered dyn_text() gets index 0, so dyn_node uses index 1.
        """
        var vb = self.ctx.render_builder()

        # Dynamic text 0: status message
        vb.add_dyn_text(String("Ready — click a button to start benchmarking"))

        # Dynamic node 1: placeholder in the <tbody>
        # (index 1 because dyn_text occupies index 0 in the shared space)
        vb.add_dyn_placeholder()

        return vb.build()


fn bench_app_init() -> UnsafePointer[BenchmarkApp]:
    """Initialize the benchmark app.  Returns a pointer to the app state.

    All setup happens in BenchmarkApp.__init__() — this function just
    allocates the heap slot and moves the app into it.
    """
    var app_ptr = UnsafePointer[BenchmarkApp].alloc(1)
    app_ptr.init_pointee_move(BenchmarkApp())
    return app_ptr


fn bench_app_destroy(app_ptr: UnsafePointer[BenchmarkApp]):
    """Destroy the benchmark app and free all resources."""
    app_ptr[0].ctx.destroy()
    app_ptr.destroy_pointee()
    app_ptr.free()


fn bench_app_rebuild(
    app: UnsafePointer[BenchmarkApp],
    writer_ptr: UnsafePointer[MutationWriter],
) -> Int32:
    """Initial render (mount) of the benchmark app.

    Phase 24.2: Builds the full app shell VNode (heading, toolbar,
    status, table) and mounts it.  The <tbody> starts with a placeholder
    comment node whose ElementId we save for later use by the KeyedList.

    Follows the same pattern as todo_app_rebuild:
    1. Emit templates (RegisterTemplate mutations)
    2. Render app shell VNode
    3. Build empty rows fragment
    4. Create DOM via CreateEngine
    5. Extract dyn_node[0] anchor ID for KeyedList
    6. Append to root

    Returns byte offset (length) of mutation data.
    """
    # Emit all registered templates so JS can build DOM from mutations
    app[0].ctx.shell.emit_templates(writer_ptr)

    # Build the app shell VNode (no rows yet — just the template)
    var app_vnode_idx = app[0].render()
    app[0].ctx.current_vnode = Int(app_vnode_idx)

    # Build an empty rows fragment and store it
    var frag_idx = app[0].build_rows_fragment()

    # Create the app template via CreateEngine.
    # This emits LoadTemplate, AssignId, NewEventListener, and
    # CreatePlaceholder + ReplacePlaceholder for dynamic[0].
    var engine = CreateEngine(
        writer_ptr,
        app[0].ctx.shell.eid_alloc,
        app[0].ctx.shell.runtime,
        app[0].ctx.shell.store,
    )
    var num_roots = engine.create_node(app_vnode_idx)

    # After CreateEngine, dynamic[1]'s placeholder has an ElementId.
    # (dyn_node uses index 1 because dyn_text occupies index 0)
    # Initialize the KeyedList's slot with the anchor and empty fragment.
    var anchor_id: UInt32 = 0
    var app_vnode_ptr = app[0].ctx.store_ptr()[0].get_ptr(app_vnode_idx)
    if app_vnode_ptr[0].dyn_node_id_count() > 1:
        anchor_id = app_vnode_ptr[0].get_dyn_node_id(1)
    app[0].rows_list.init_slot(anchor_id, frag_idx)

    # Append the app shell to root element (id 0)
    writer_ptr[0].append_children(0, num_roots)

    writer_ptr[0].finalize()
    return Int32(writer_ptr[0].offset)


fn bench_app_flush(
    app: UnsafePointer[BenchmarkApp],
    writer_ptr: UnsafePointer[MutationWriter],
) -> Int32:
    """Flush pending updates after a benchmark operation.

    Phase 24.2: Re-renders the app shell VNode (to catch any status
    text changes via diff) and flushes the row list via KeyedList.
    Follows the same pattern as todo_app_flush.

    Uses KeyedList.flush() which delegates fragment transitions
    (empty↔populated) to the reusable flush_fragment lifecycle helper.

    Returns byte offset (length) of mutation data, or 0 if nothing dirty.
    """
    # Collect and consume dirty scopes via the scheduler
    if not app[0].ctx.consume_dirty():
        return 0

    # Re-render app shell to pick up any changes.
    # dyn_node(0) stays as placeholder — diff sees placeholder vs
    # placeholder and does nothing (KeyedList manages it separately).
    var new_app_idx = app[0].render()
    app[0].ctx.diff(writer_ptr, new_app_idx)

    # Build a new rows fragment from the current row list
    var new_frag_idx = app[0].build_rows_fragment()

    # Flush via KeyedList (handles all three transitions)
    app[0].rows_list.flush(app[0].ctx, writer_ptr, new_frag_idx)

    writer_ptr[0].finalize()
    return Int32(writer_ptr[0].offset)

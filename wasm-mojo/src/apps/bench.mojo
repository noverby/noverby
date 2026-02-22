# BenchmarkApp — js-framework-benchmark implementation.
#
# Migrated to Dioxus-style ergonomics with Phase 16 abstractions:
#   - Multi-arg el_* overloads (no List[Node]() wrappers)
#   - KeyedList abstraction (bundles FragmentSlot + scope IDs + template ID)
#   - Constructor-based setup (all init in __init__)
#   - ctx.use_signal() for automatic scope subscription
#   - SignalI32 handles with operator overloading
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
# Template structure (built via DSL with multi-arg overloads):
#   "bench-row": tr + dynamic_attr[0](class) > [
#       td > dynamic_text[0] (id),
#       td > a > dynamic_text[1] (label),
#       td > a > text("×")
#   ]
#   dynamic_attr[1] = click on label (select)
#   dynamic_attr[2] = click on delete button (remove)
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
#             for row in rows.read().iter() {
#                 tr { class: if selected() == row.id { "danger" } else { "" },
#                     td { "{row.id}" }
#                     td { a { onclick: move |_| selected.set(row.id), "{row.label}" } }
#                     td { a { onclick: move |_| remove(row.id), "×" } }
#                 }
#             }
#         }
#     }
#
# Mojo equivalent (with Phase 16 abstractions):
#
#     struct BenchmarkApp:
#         var ctx: ComponentContext
#         var version: SignalI32
#         var selected: SignalI32
#         var rows_list: KeyedList   # bundles template_id + FragmentSlot + scope_ids
#
#         fn __init__(out self):
#             self.ctx = ComponentContext.create()
#             self.version = self.ctx.use_signal(0)
#             self.selected = self.ctx.use_signal(0)
#             self.ctx.end_setup()
#             self.rows_list = KeyedList(self.ctx.register_extra_template(
#                 el_tr(
#                     dyn_attr(0),
#                     el_td(dyn_text(0)),
#                     el_td(el_a(dyn_attr(1), dyn_text(1))),
#                     el_td(el_a(dyn_attr(2), text("×"))),
#                 ),
#                 String("bench-row"),
#             ))

from memory import UnsafePointer
from bridge import MutationWriter

from events import HandlerEntry
from component import ComponentContext, KeyedList
from signals import SignalI32
from vdom import (
    VNode,
    VNodeStore,
    Node,
    el_tr,
    el_td,
    el_a,
    text,
    dyn_text,
    dyn_attr,
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


struct BenchmarkApp(Movable):
    """Js-framework-benchmark app state.

    All setup — context creation, signal creation, and template
    registration — happens in __init__.  The lifecycle functions are
    thin delegations to ComponentContext.

    Uses KeyedList to bundle the row template, FragmentSlot, and child
    scope tracking into a single abstraction.

    Manages a list of rows, selection state, and all rendering
    infrastructure via ComponentContext (runtime, templates, vnode
    store, etc.).
    """

    var ctx: ComponentContext
    var version: SignalI32  # bumped on list changes
    var selected: SignalI32  # currently selected row id (0 = none)
    var rows_list: KeyedList  # bundles template_id + FragmentSlot + scope_ids
    var rows: List[BenchRow]
    var next_id: Int32
    var rng_state: UInt32  # simple LCG state

    fn __init__(out self):
        """Initialize the benchmark app with all reactive state and templates.

        Creates: ComponentContext (runtime, VNode store, element ID
        allocator, scheduler), root scope, version and selected signals,
        and the row template via KeyedList.

        Template "bench-row": tr + dyn_attr[0](class) > [
            td > dyn_text[0],          <- id
            td > a + dyn_attr[1] > dyn_text[1],  <- label + select click
            td > a + dyn_attr[2] > text("×")      <- delete click
        ]

        Uses multi-arg el_* overloads — no List[Node]() wrappers needed.
        """
        # 1. Create context and signals
        self.ctx = ComponentContext.create()
        self.version = self.ctx.use_signal(0)
        self.selected = self.ctx.use_signal(0)
        self.ctx.end_setup()

        # 2. Register the "bench-row" template via KeyedList
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

        # 3. Initialize remaining state
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

    fn build_row_vnode(mut self, row: BenchRow) -> UInt32:
        """Build a keyed VNode for a single benchmark row.

        Template "bench-row": tr > [ td(id), td > a(label), td > a("×") ]
          dynamic_attr[0] = class on <tr> ("danger" if selected)
          dynamic_text[0] = row id
          dynamic_text[1] = row label
          dynamic_attr[1] = click on label <a> (select)
          dynamic_attr[2] = click on delete <a> (remove)

        Uses KeyedList.create_scope() and KeyedList.item_builder() for
        ergonomic child scope and VNode construction.
        """
        # Create a child scope for this row's handlers (tracked by KeyedList)
        var child_scope = self.rows_list.create_scope(self.ctx)

        var vb = self.rows_list.item_builder(String(row.id), self.ctx)

        # Dynamic text 0: row id
        vb.add_dyn_text(String(row.id))

        # Dynamic text 1: row label
        vb.add_dyn_text(row.label)

        # Dynamic attr 0: class on <tr> ("danger" if selected)
        var selected = self.selected.peek()
        var tr_class: String
        if selected == row.id:
            tr_class = String("danger")
        else:
            tr_class = String("")
        vb.add_dyn_text_attr(String("class"), tr_class)

        # Dynamic attr 1: click on label <a> (select — custom handler)
        var select_handler = self.ctx.register_handler(
            HandlerEntry.custom(child_scope, String("click"))
        )
        vb.add_dyn_event(String("click"), select_handler)

        # Dynamic attr 2: click on delete <a> (remove — custom handler)
        var remove_handler = self.ctx.register_handler(
            HandlerEntry.custom(child_scope, String("click"))
        )
        vb.add_dyn_event(String("click"), remove_handler)

        return vb.index()

    fn build_rows_fragment(mut self) -> UInt32:
        """Build a Fragment VNode containing all row VNodes.

        Uses KeyedList.begin_rebuild() to destroy old child scopes and
        create a new empty fragment, then builds each row VNode and
        pushes it as a fragment child.
        """
        var frag_idx = self.rows_list.begin_rebuild(self.ctx)
        for i in range(len(self.rows)):
            var row_idx = self.build_row_vnode(self.rows[i].copy())
            self.rows_list.push_child(self.ctx, frag_idx, row_idx)
        return frag_idx


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
    """Initial render of the benchmark table body.

    Creates an anchor placeholder in the DOM (will be replaced on first
    populate).  Emits mutations for the initial empty state.

    Returns byte offset (length) of mutation data.
    """
    # Emit all registered templates so JS can build DOM from mutations
    app[0].ctx.shell.emit_templates(writer_ptr)

    # Create an anchor placeholder
    var anchor_eid = app[0].ctx.shell.eid_alloc[0].alloc()
    writer_ptr[0].create_placeholder(anchor_eid.as_u32())
    writer_ptr[0].append_children(0, 1)

    # Build initial empty fragment and initialize the KeyedList's slot
    var frag_idx = app[0].build_rows_fragment()
    app[0].rows_list.init_slot(anchor_eid.as_u32(), frag_idx)

    writer_ptr[0].finalize()
    return Int32(writer_ptr[0].offset)


fn bench_app_flush(
    app: UnsafePointer[BenchmarkApp],
    writer_ptr: UnsafePointer[MutationWriter],
) -> Int32:
    """Flush pending updates after a benchmark operation.

    Uses KeyedList.flush() which delegates fragment transitions
    (empty↔populated) to the reusable flush_fragment lifecycle helper.

    Returns byte offset (length) of mutation data, or 0 if nothing dirty.
    """
    # Collect and consume dirty scopes via the scheduler
    if not app[0].ctx.consume_dirty():
        return 0

    var new_frag_idx = app[0].build_rows_fragment()

    # Flush via KeyedList (handles all three transitions)
    app[0].rows_list.flush(app[0].ctx, writer_ptr, new_frag_idx)

    writer_ptr[0].finalize()
    return Int32(writer_ptr[0].offset)

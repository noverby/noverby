# BenchmarkApp — js-framework-benchmark implementation.
#
# Phase 9 — Implements the standard benchmark operations:
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
# Template structure (built via DSL):
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

from memory import UnsafePointer
from bridge import MutationWriter

from events import HandlerEntry
from component import AppShell, app_shell_create, FragmentSlot, flush_fragment
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

    Manages a list of rows, selection state, and all rendering
    infrastructure via AppShell (runtime, templates, vnode store, etc.).
    """

    var shell: AppShell
    var scope_id: UInt32
    var version_signal: UInt32  # bumped on list changes
    var selected_signal: UInt32  # currently selected row id (0 = none)
    var row_template_id: UInt32
    var rows: List[BenchRow]
    var next_id: Int32
    var rng_state: UInt32  # simple LCG state
    var row_slot: FragmentSlot  # tracks row list fragment lifecycle

    fn __init__(out self):
        self.shell = AppShell()
        self.scope_id = 0
        self.version_signal = 0
        self.selected_signal = 0
        self.row_template_id = 0
        self.rows = List[BenchRow]()
        self.next_id = 1
        self.rng_state = 42
        self.row_slot = FragmentSlot()

    fn __moveinit__(out self, deinit other: Self):
        self.shell = other.shell^
        self.scope_id = other.scope_id
        self.version_signal = other.version_signal
        self.selected_signal = other.selected_signal
        self.row_template_id = other.row_template_id
        self.rows = other.rows^
        self.next_id = other.next_id
        self.rng_state = other.rng_state
        self.row_slot = other.row_slot^

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
        var current = self.shell.peek_signal_i32(self.version_signal)
        self.shell.write_signal_i32(self.version_signal, current + 1)

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
        self.shell.write_signal_i32(self.selected_signal, id)

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
        """
        var vb = VNodeBuilder(
            self.row_template_id, String(row.id), self.shell.store
        )

        # Dynamic text 0: row id
        vb.add_dyn_text(String(row.id))

        # Dynamic text 1: row label
        vb.add_dyn_text(row.label)

        # Dynamic attr 0: class on <tr> ("danger" if selected)
        var selected = self.shell.peek_signal_i32(self.selected_signal)
        var tr_class: String
        if selected == row.id:
            tr_class = String("danger")
        else:
            tr_class = String("")
        vb.add_dyn_text_attr(String("class"), tr_class)

        # Dynamic attr 1: click on label <a> (select — custom handler)
        var select_handler = self.shell.runtime[0].register_handler(
            HandlerEntry.custom(self.scope_id, String("click"))
        )
        vb.add_dyn_event(String("click"), select_handler)

        # Dynamic attr 2: click on delete <a> (remove — custom handler)
        var remove_handler = self.shell.runtime[0].register_handler(
            HandlerEntry.custom(self.scope_id, String("click"))
        )
        vb.add_dyn_event(String("click"), remove_handler)

        return vb.index()

    fn build_rows_fragment(mut self) -> UInt32:
        """Build a Fragment VNode containing all row VNodes."""
        var frag_idx = self.shell.store[0].push(VNode.fragment())
        for i in range(len(self.rows)):
            var row_idx = self.build_row_vnode(self.rows[i].copy())
            self.shell.store[0].push_fragment_child(frag_idx, row_idx)
        return frag_idx


fn bench_app_init() -> UnsafePointer[BenchmarkApp]:
    """Initialize the benchmark app.  Returns a pointer to the app state.

    Creates: AppShell (runtime, VNode store, element ID allocator,
    scheduler), scope, signals, and the row template.
    """
    var app_ptr = UnsafePointer[BenchmarkApp].alloc(1)
    app_ptr.init_pointee_move(BenchmarkApp())

    # 1. Create subsystem instances via AppShell
    app_ptr[0].shell = app_shell_create()

    # 2. Create root scope and signals
    app_ptr[0].scope_id = app_ptr[0].shell.create_root_scope()
    _ = app_ptr[0].shell.begin_render(app_ptr[0].scope_id)
    app_ptr[0].version_signal = app_ptr[0].shell.use_signal_i32(0)
    app_ptr[0].selected_signal = app_ptr[0].shell.use_signal_i32(0)
    # Read signals to subscribe scope
    _ = app_ptr[0].shell.read_signal_i32(app_ptr[0].version_signal)
    _ = app_ptr[0].shell.read_signal_i32(app_ptr[0].selected_signal)
    app_ptr[0].shell.end_render(-1)

    # 3. Build and register the "bench-row" template via DSL:
    #    tr + dynamic_attr[0](class) > [
    #        td > dynamic_text[0],          ← id
    #        td > a + dynamic_attr[1] > dynamic_text[1],  ← label + select click
    #        td > a + dynamic_attr[2] > text("×")         ← delete click
    #    ]
    var row_view = el_tr(
        List[Node](
            dyn_attr(0),  # class on <tr>
            el_td(List[Node](dyn_text(0))),
            el_td(List[Node](el_a(List[Node](dyn_attr(1), dyn_text(1))))),
            el_td(List[Node](el_a(List[Node](dyn_attr(2), text(String("×")))))),
        )
    )
    var row_template = to_template(row_view, String("bench-row"))
    app_ptr[0].row_template_id = UInt32(
        app_ptr[0].shell.runtime[0].templates.register(row_template^)
    )

    return app_ptr


fn bench_app_destroy(app_ptr: UnsafePointer[BenchmarkApp]):
    """Destroy the benchmark app and free all resources."""
    app_ptr[0].shell.destroy()
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
    # Create an anchor placeholder
    var anchor_eid = app[0].shell.eid_alloc[0].alloc()
    writer_ptr[0].create_placeholder(anchor_eid.as_u32())
    writer_ptr[0].append_children(0, 1)

    # Build initial empty fragment and initialize the FragmentSlot
    var frag_idx = app[0].build_rows_fragment()
    app[0].row_slot = FragmentSlot(anchor_eid.as_u32(), Int(frag_idx))

    writer_ptr[0].finalize()
    return Int32(writer_ptr[0].offset)


fn bench_app_flush(
    app: UnsafePointer[BenchmarkApp],
    writer_ptr: UnsafePointer[MutationWriter],
) -> Int32:
    """Flush pending updates after a benchmark operation.

    Delegates fragment transitions (empty↔populated) to the reusable
    `flush_fragment` lifecycle helper via the app's `FragmentSlot`.

    Returns byte offset (length) of mutation data, or 0 if nothing dirty.
    """
    if not app[0].shell.has_dirty():
        return 0

    var _dirty = app[0].shell.runtime[0].drain_dirty()

    var new_frag_idx = app[0].build_rows_fragment()

    # Flush via lifecycle helper (handles all three transitions)
    app[0].row_slot = flush_fragment(
        writer_ptr,
        app[0].shell.eid_alloc,
        app[0].shell.runtime,
        app[0].shell.store,
        app[0].row_slot,
        new_frag_idx,
    )

    writer_ptr[0].finalize()
    return Int32(writer_ptr[0].offset)

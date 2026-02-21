# TodoApp — Self-contained todo list application.
#
# Phase 8 — Demonstrates:
#   - Dynamic keyed lists (add, remove, toggle items)
#   - Conditional rendering (show/hide completed indicator)
#   - Fragment VNodes with keyed children
#   - String data flow (input text from JS → WASM)
#
# Architecture:
#   - TodoApp struct holds all state: items list, input text, signals, handlers
#   - Items are stored as a flat list of TodoItem structs (not signals)
#   - A "list_version" signal is bumped on every list mutation to trigger re-render
#   - JS calls specific exports (todo_add_item, todo_remove_item, etc.)
#     then calls todo_flush() to get mutation bytes
#
# Templates (built via DSL):
#   - "todo-app": The app shell with input field + item list container
#       div > [ input + button("Add") + ul > dynamic[0] ]
#   - "todo-item": A single list item
#       li > [ span > dynamic_text[0], button("✓") + button("✕") ]
#       dynamic_attr[0] = click handler for toggle
#       dynamic_attr[1] = click handler for remove
#       dynamic_attr[2] = class on the li (for completed styling)

from memory import UnsafePointer
from bridge import MutationWriter
from arena import ElementIdAllocator
from signals import Runtime, create_runtime, destroy_runtime
from mutations import CreateEngine, DiffEngine
from events import HandlerEntry
from vdom import (
    VNode,
    VNodeStore,
    Node,
    el_div,
    el_span,
    el_button,
    el_input,
    el_ul,
    el_li,
    text,
    dyn_text,
    dyn_node,
    dyn_attr,
    attr,
    to_template,
    VNodeBuilder,
)


struct TodoItem(Copyable, Movable):
    """A single todo list item."""

    var id: Int32
    var text: String
    var completed: Bool

    fn __init__(out self, id: Int32, text: String, completed: Bool):
        self.id = id
        self.text = text
        self.completed = completed

    fn __copyinit__(out self, other: Self):
        self.id = other.id
        self.text = other.text
        self.completed = other.completed

    fn __moveinit__(out self, deinit other: Self):
        self.id = other.id
        self.text = other.text^
        self.completed = other.completed


struct TodoApp(Movable):
    """Self-contained todo list application state.

    The item list lives inside the <ul> element of the app template.
    On initial mount, a placeholder comment node occupies the <ul>.
    We track that placeholder's ElementId so we can replace it with
    item nodes, and later manage item-to-item diffs via a Fragment
    VNode that mirrors the <ul>'s children.

    State tracking:
      - ul_placeholder_id: ElementId of the placeholder inside <ul>.
        Non-zero when the list is empty (placeholder is in the DOM).
        Zero when items are present (placeholder was replaced).
      - current_frag: VNode index of the current items Fragment.
        -1 before first render.
      - items_mounted: True once items have replaced the placeholder.
    """

    var runtime: UnsafePointer[Runtime]
    var store: UnsafePointer[VNodeStore]
    var eid_alloc: UnsafePointer[ElementIdAllocator]
    var scope_id: UInt32
    var list_version_signal: UInt32  # bumped on every list mutation
    var app_template_id: UInt32  # "todo-app" template
    var item_template_id: UInt32  # "todo-item" template
    var items: List[TodoItem]
    var next_id: Int32
    var input_text: String
    var current_vnode: Int  # index in store, or -1 if not yet rendered
    var current_frag: Int  # Fragment VNode index, or -1
    var ul_placeholder_id: UInt32  # ElementId of placeholder in <ul>
    var items_mounted: Bool  # True when items are in DOM (placeholder removed)
    # Handler IDs for the app-level controls
    var add_handler: UInt32

    fn __init__(out self):
        self.runtime = UnsafePointer[Runtime]()
        self.store = UnsafePointer[VNodeStore]()
        self.eid_alloc = UnsafePointer[ElementIdAllocator]()
        self.scope_id = 0
        self.list_version_signal = 0
        self.app_template_id = 0
        self.item_template_id = 0
        self.items = List[TodoItem]()
        self.next_id = 1
        self.input_text = String("")
        self.current_vnode = -1
        self.current_frag = -1
        self.ul_placeholder_id = 0
        self.items_mounted = False
        self.add_handler = 0

    fn __moveinit__(out self, deinit other: Self):
        self.runtime = other.runtime
        self.store = other.store
        self.eid_alloc = other.eid_alloc
        self.scope_id = other.scope_id
        self.list_version_signal = other.list_version_signal
        self.app_template_id = other.app_template_id
        self.item_template_id = other.item_template_id
        self.items = other.items^
        self.next_id = other.next_id
        self.input_text = other.input_text^
        self.current_vnode = other.current_vnode
        self.current_frag = other.current_frag
        self.ul_placeholder_id = other.ul_placeholder_id
        self.items_mounted = other.items_mounted
        self.add_handler = other.add_handler

    fn add_item(mut self, text: String):
        """Add a new item and bump the list version signal."""
        if len(text) == 0:
            return
        self.items.append(TodoItem(self.next_id, text, False))
        self.next_id += 1
        self._bump_version()

    fn remove_item(mut self, item_id: Int32):
        """Remove an item by ID and bump the list version signal."""
        for i in range(len(self.items)):
            if self.items[i].id == item_id:
                # Swap-remove for O(1)
                var last = len(self.items) - 1
                if i != last:
                    self.items[i] = self.items[last].copy()
                _ = self.items.pop()
                self._bump_version()
                return

    fn toggle_item(mut self, item_id: Int32):
        """Toggle an item's completed status and bump the list version signal.
        """
        for i in range(len(self.items)):
            if self.items[i].id == item_id:
                self.items[i].completed = not self.items[i].completed
                self._bump_version()
                return

    fn _bump_version(mut self):
        """Increment the list version signal to trigger re-render."""
        var current = self.runtime[0].peek_signal[Int32](
            self.list_version_signal
        )
        self.runtime[0].write_signal[Int32](
            self.list_version_signal, current + 1
        )

    fn build_item_vnode(mut self, item: TodoItem) -> UInt32:
        """Build a keyed VNode for a single todo item.

        Template "todo-item": li > [ span > dynamic_text[0], button("✓"), button("✕") ]
          dynamic_text[0] = item text (possibly with strikethrough indicator)
          dynamic_attr[0] = click on toggle button
          dynamic_attr[1] = click on remove button
          dynamic_attr[2] = class on the li element
        """
        var vb = VNodeBuilder(
            self.item_template_id, String(item.id), self.store
        )

        # Dynamic text: item text with completion indicator
        var display_text: String
        if item.completed:
            display_text = String("✓ ") + item.text
        else:
            display_text = item.text
        vb.add_dyn_text(display_text)

        # Dynamic attr 0: toggle handler (click on ✓ button)
        var toggle_handler = self.runtime[0].register_handler(
            HandlerEntry.custom(self.scope_id, String("click"))
        )
        vb.add_dyn_event(String("click"), toggle_handler)

        # Dynamic attr 1: remove handler (click on ✕ button)
        var remove_handler = self.runtime[0].register_handler(
            HandlerEntry.custom(self.scope_id, String("click"))
        )
        vb.add_dyn_event(String("click"), remove_handler)

        # Dynamic attr 2: class on the li element
        var li_class: String
        if item.completed:
            li_class = String("completed")
        else:
            li_class = String("")
        vb.add_dyn_text_attr(String("class"), li_class)

        return vb.index()

    fn build_items_fragment(mut self) -> UInt32:
        """Build a Fragment VNode containing keyed item children."""
        var frag_idx = self.store[0].push(VNode.fragment())
        for i in range(len(self.items)):
            var item_idx = self.build_item_vnode(self.items[i].copy())
            self.store[0].push_fragment_child(frag_idx, item_idx)
        return frag_idx

    fn build_app_vnode(mut self) -> UInt32:
        """Build the app shell VNode (TemplateRef for todo-app).

        Template "todo-app": div > [ input, button("Add") + dynamic_attr[0], ul > dynamic[0] ]
          dynamic_attr[0] = click on Add button
          dynamic[0] = placeholder (item list managed separately)
        """
        var vb = VNodeBuilder(self.app_template_id, self.store)

        # Dynamic node 0: placeholder in the <ul>
        vb.add_dyn_placeholder()

        # Dynamic attr 0: click on Add button
        vb.add_dyn_event(String("click"), self.add_handler)

        return vb.index()


fn todo_app_init() -> UnsafePointer[TodoApp]:
    """Initialize the todo app.  Returns a pointer to the app state.

    Creates: runtime, VNode store, element ID allocator, scope, signals,
    templates, and event handlers.
    """
    var app_ptr = UnsafePointer[TodoApp].alloc(1)
    app_ptr.init_pointee_move(TodoApp())

    # 1. Create subsystem instances
    app_ptr[0].runtime = create_runtime()
    app_ptr[0].store = UnsafePointer[VNodeStore].alloc(1)
    app_ptr[0].store.init_pointee_move(VNodeStore())
    app_ptr[0].eid_alloc = UnsafePointer[ElementIdAllocator].alloc(1)
    app_ptr[0].eid_alloc.init_pointee_move(ElementIdAllocator())

    # 2. Create root scope and list_version signal
    app_ptr[0].scope_id = app_ptr[0].runtime[0].create_scope(0, -1)
    _ = app_ptr[0].runtime[0].begin_scope_render(app_ptr[0].scope_id)
    app_ptr[0].list_version_signal = app_ptr[0].runtime[0].use_signal_i32(0)
    # Read the signal to subscribe the scope
    _ = app_ptr[0].runtime[0].read_signal[Int32](app_ptr[0].list_version_signal)
    app_ptr[0].runtime[0].end_scope_render(-1)

    # 3. Build and register the "todo-app" template via DSL:
    #    div > [ input (placeholder), button("Add") + dynamic_attr[0], ul > dynamic[0] ]
    var app_view = el_div(
        List[Node](
            el_input(
                List[Node](
                    attr(String("type"), String("text")),
                    attr(
                        String("placeholder"),
                        String("What needs to be done?"),
                    ),
                )
            ),
            el_button(List[Node](text(String("Add")), dyn_attr(0))),
            el_ul(List[Node](dyn_node(0))),
        )
    )
    var app_template = to_template(app_view, String("todo-app"))
    app_ptr[0].app_template_id = UInt32(
        app_ptr[0].runtime[0].templates.register(app_template^)
    )

    # 4. Build and register the "todo-item" template via DSL:
    #    li + dynamic_attr[2] > [ span > dynamic_text[0],
    #                             button("✓") + dynamic_attr[0],
    #                             button("✕") + dynamic_attr[1] ]
    var item_view = el_li(
        List[Node](
            dyn_attr(2),  # class attr on li
            el_span(List[Node](dyn_text(0))),
            el_button(List[Node](text(String("✓")), dyn_attr(0))),
            el_button(List[Node](text(String("✕")), dyn_attr(1))),
        )
    )
    var item_template = to_template(item_view, String("todo-item"))
    app_ptr[0].item_template_id = UInt32(
        app_ptr[0].runtime[0].templates.register(item_template^)
    )

    # 5. Register the Add button handler (custom — JS calls todo_add_item)
    app_ptr[0].add_handler = (
        app_ptr[0]
        .runtime[0]
        .register_handler(
            HandlerEntry.custom(app_ptr[0].scope_id, String("click"))
        )
    )

    return app_ptr


fn todo_app_destroy(app_ptr: UnsafePointer[TodoApp]):
    """Destroy the todo app and free all resources."""
    if app_ptr[0].store:
        app_ptr[0].store.destroy_pointee()
        app_ptr[0].store.free()
    if app_ptr[0].eid_alloc:
        app_ptr[0].eid_alloc.destroy_pointee()
        app_ptr[0].eid_alloc.free()
    if app_ptr[0].runtime:
        destroy_runtime(app_ptr[0].runtime)

    app_ptr.destroy_pointee()
    app_ptr.free()


fn todo_app_rebuild(
    app: UnsafePointer[TodoApp],
    writer_ptr: UnsafePointer[MutationWriter],
) -> Int32:
    """Initial render (mount) of the todo app.

    Builds the app shell VNode and mounts it.  The <ul> starts with a
    placeholder comment node whose ElementId we save for later use.

    Returns the byte offset (length) of the mutation data written.
    """
    # Build the app shell VNode (no items yet — just the template)
    var app_vnode_idx = app[0].build_app_vnode()
    app[0].current_vnode = Int(app_vnode_idx)

    # Build an empty items fragment and store it
    var frag_idx = app[0].build_items_fragment()
    app[0].current_frag = Int(frag_idx)

    # Create the app template via CreateEngine.
    # This emits LoadTemplate, AssignId, NewEventListener, and
    # CreatePlaceholder + ReplacePlaceholder for dynamic[0].
    var engine = CreateEngine(
        writer_ptr, app[0].eid_alloc, app[0].runtime, app[0].store
    )
    var num_roots = engine.create_node(app_vnode_idx)

    # After CreateEngine, dynamic[0]'s placeholder has an ElementId.
    # Save it so we can replace it with items later.
    var app_vnode_ptr = app[0].store[0].get_ptr(app_vnode_idx)
    if app_vnode_ptr[0].dyn_node_id_count() > 0:
        app[0].ul_placeholder_id = app_vnode_ptr[0].get_dyn_node_id(0)
    app[0].items_mounted = False

    # Append the app shell to root element (id 0)
    writer_ptr[0].append_children(0, num_roots)

    writer_ptr[0].finalize()
    return Int32(writer_ptr[0].offset)


fn todo_app_flush(
    app: UnsafePointer[TodoApp],
    writer_ptr: UnsafePointer[MutationWriter],
) -> Int32:
    """Flush pending updates after a list mutation.

    Handles three transitions for the item list inside the <ul>:
      1. empty → populated: create items, ReplaceWith placeholder
      2. populated → populated: diff old fragment vs new fragment (keyed)
      3. populated → empty: remove all items, CreatePlaceholder to restore anchor

    Returns the byte offset (length) of mutation data, or 0 if nothing dirty.
    """
    if not app[0].runtime[0].has_dirty():
        return 0

    var _dirty = app[0].runtime[0].drain_dirty()

    # Build a new items fragment from the current item list
    var new_frag_idx = app[0].build_items_fragment()
    var old_frag_idx = UInt32(app[0].current_frag)

    var old_frag_ptr = app[0].store[0].get_ptr(old_frag_idx)
    var new_frag_ptr = app[0].store[0].get_ptr(new_frag_idx)
    var old_count = old_frag_ptr[0].fragment_child_count()
    var new_count = new_frag_ptr[0].fragment_child_count()

    if not app[0].items_mounted and new_count > 0:
        # ── Transition: empty → populated ─────────────────────────────
        # The <ul> currently has a placeholder comment node.  Create item
        # VNodes, push them on the stack, and ReplaceWith the placeholder.
        var create_eng = CreateEngine(
            writer_ptr, app[0].eid_alloc, app[0].runtime, app[0].store
        )
        var total_roots: UInt32 = 0
        for i in range(new_count):
            var child_idx = (
                app[0].store[0].get_ptr(new_frag_idx)[0].get_fragment_child(i)
            )
            total_roots += create_eng.create_node(child_idx)

        if app[0].ul_placeholder_id != 0 and total_roots > 0:
            writer_ptr[0].replace_with(app[0].ul_placeholder_id, total_roots)
        app[0].items_mounted = True

    elif app[0].items_mounted and new_count == 0:
        # ── Transition: populated → empty ─────────────────────────────
        # Handled after this if-elif chain (needs careful ordering:
        # create placeholder, insert before first item, then remove items).
        pass  # fall through — handled below

    elif app[0].items_mounted and new_count > 0:
        # ── Transition: populated → populated ─────────────────────────
        # Both old and new have items.  Use the keyed diff engine.
        var diff_eng = DiffEngine(
            writer_ptr, app[0].eid_alloc, app[0].runtime, app[0].store
        )
        diff_eng.diff_node(old_frag_idx, new_frag_idx)

    # else: both empty → no-op

    # Handle the populated → empty case properly (we skipped above).
    if app[0].items_mounted and new_count == 0:
        # Reset writer (it has nothing from the pass above)
        # We need to:
        #   1. Find the first old item's root ElementId
        #   2. Create a new placeholder
        #   3. InsertBefore the first old item
        #   4. Remove all old items
        var first_old_root_id: UInt32 = 0
        if old_count > 0:
            var first_child = (
                app[0].store[0].get_ptr(old_frag_idx)[0].get_fragment_child(0)
            )
            var fc_ptr = app[0].store[0].get_ptr(first_child)
            if fc_ptr[0].root_id_count() > 0:
                first_old_root_id = fc_ptr[0].get_root_id(0)
            elif fc_ptr[0].element_id != 0:
                first_old_root_id = fc_ptr[0].element_id

        # Create a new placeholder
        var new_ph_eid = app[0].eid_alloc[0].alloc()
        writer_ptr[0].create_placeholder(new_ph_eid.as_u32())

        # Insert before the first item (which is still in DOM at this point)
        if first_old_root_id != 0:
            writer_ptr[0].insert_before(first_old_root_id, 1)

        # Now remove all old items
        var diff_eng2 = DiffEngine(
            writer_ptr, app[0].eid_alloc, app[0].runtime, app[0].store
        )
        for i in range(old_count):
            var old_child = (
                app[0].store[0].get_ptr(old_frag_idx)[0].get_fragment_child(i)
            )
            diff_eng2._remove_node(old_child)

        app[0].ul_placeholder_id = new_ph_eid.as_u32()
        app[0].items_mounted = False

    # Update current fragment
    app[0].current_frag = Int(new_frag_idx)

    writer_ptr[0].finalize()
    return Int32(writer_ptr[0].offset)

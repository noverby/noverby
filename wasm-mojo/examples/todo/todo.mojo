# TodoApp — Self-contained todo list application.
#
# Migrated to Phase 19 Dioxus-style ergonomics:
#   - `SignalString` for reactive input text (Phase 19 — replaces plain String field)
#   - `begin_item()` replaces manual `create_scope()` + `item_builder()`
#   - `add_custom_event()` replaces manual `register_handler()` + `add_dyn_event()` + `handler_map.append()`
#   - `get_action()` replaces manual handler_map lookup loop
#   - `add_class_if()` replaces 4-line if/else class pattern (Phase 18)
#   - `text_when()` replaces 4-line if/else text pattern (Phase 18)
#   - Multi-arg el_* overloads (no List[Node]() wrappers)
#   - KeyedList abstraction (bundles FragmentSlot + scope IDs + template ID + handler map)
#   - Constructor-based setup (all init in __init__)
#   - ctx.use_signal() for automatic scope subscription
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
# Templates (built via DSL with multi-arg overloads):
#   - "todo-app": The app shell with input field + item list container
#       div > [ input + button("Add") + ul > dynamic[0] ]
#   - "todo-item": A single list item
#       li > [ span > dynamic_text[0], button("✓") + button("✕") ]
#       dynamic_attr[0] = click handler for toggle
#       dynamic_attr[1] = click handler for remove
#       dynamic_attr[2] = class on the li (for completed styling)
#
# Compare with Dioxus (Rust):
#
#     fn TodoApp() -> Element {
#         let mut items = use_signal(|| vec![]);
#         rsx! {
#             div {
#                 input { type: "text", placeholder: "What needs to be done?" }
#                 button { onclick: move |_| add_item(), "Add" }
#                 ul { for item in items.read().iter() {
#                     li { class: if item.completed { "completed" } else { "" },
#                         span { "{item.text}" }
#                         button { onclick: move |_| toggle(item.id), "✓" }
#                         button { onclick: move |_| remove(item.id), "✕" }
#                     }
#                 }}
#             }
#         }
#     }
#
# Mojo equivalent (with Phase 17 abstractions):
#
#     struct TodoApp:
#         var ctx: ComponentContext
#         var list_version: SignalI32
#         var items: KeyedList
#
#         fn __init__(out self):
#             self.ctx = ComponentContext.create()
#             self.list_version = self.ctx.use_signal(0)
#             self.ctx.end_setup()
#             self.ctx.register_template(
#                 el_div(
#                     el_input(attr("type", "text"), attr("placeholder", "What needs to be done?")),
#                     el_button(text("Add"), dyn_attr(0)),
#                     el_ul(dyn_node(0)),
#                 ),
#                 String("todo-app"),
#             )
#             self.items = KeyedList(self.ctx.register_extra_template(
#                 el_li(
#                     dyn_attr(2),
#                     el_span(dyn_text(0)),
#                     el_button(text("✓"), dyn_attr(0)),
#                     el_button(text("✕"), dyn_attr(1)),
#                 ),
#                 String("todo-item"),
#             ))
#
#         fn build_item_vnode(mut self, item: TodoItem) -> UInt32:
#             var ib = self.items.begin_item(String(item.id), self.ctx)
#             ib.add_dyn_text(text_when(item.completed, "✓ " + item.text, item.text))
#             ib.add_custom_event(String("click"), TODO_ACTION_TOGGLE, item.id)
#             ib.add_custom_event(String("click"), TODO_ACTION_REMOVE, item.id)
#             ib.add_class_if(item.completed, String("completed"))
#             return ib.index()
#
#         fn handle_event(mut self, handler_id: UInt32) -> Bool:
#             var action = self.items.get_action(handler_id)
#             if action.found:
#                 if action.tag == TODO_ACTION_TOGGLE:
#                     self.toggle_item(action.data)
#                 elif action.tag == TODO_ACTION_REMOVE:
#                     self.remove_item(action.data)
#                 return True
#             return False

from memory import UnsafePointer
from bridge import MutationWriter
from mutations import CreateEngine
from events import HandlerEntry
from component import ComponentContext, KeyedList
from signals import SignalI32, SignalString
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
    text_when,
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


# App-defined action tags for ItemBuilder.add_custom_event() dispatch.
# These are retrieved via KeyedList.get_action().
alias TODO_ACTION_TOGGLE: UInt8 = 1
alias TODO_ACTION_REMOVE: UInt8 = 2


struct TodoApp(Movable):
    """Self-contained todo list application state.

    All setup — context creation, signal creation, template registration,
    and event handler binding — happens in __init__.  The lifecycle
    functions are thin delegations to ComponentContext.

    Uses KeyedList with Phase 17 ItemBuilder for ergonomic per-item
    building and HandlerAction for dispatch.

    Phase 19: `input_text` is a `SignalString` created via
    `create_signal_string()` (no scope subscription — the input value
    is not rendered reactively, it's a write-buffer for the Add flow).

    The item list lives inside the <ul> element of the app template.
    On initial mount, a placeholder comment node occupies the <ul>.
    KeyedList tracks the placeholder/anchor, current fragment,
    and mounted state for the item list transitions.
    """

    var ctx: ComponentContext
    var list_version: SignalI32
    var items: KeyedList  # bundles template_id + FragmentSlot + scope_ids + handler_map
    var data: List[TodoItem]
    var next_id: Int32
    var input_text: SignalString  # Phase 19: reactive string signal (no subscription)
    # Handler ID for the app-level Add button
    var add_handler: UInt32

    fn __init__(out self):
        """Initialize the todo app with all reactive state, templates, and handlers.

        Creates: ComponentContext (runtime, VNode store, element ID
        allocator, scheduler), root scope, list_version signal, the
        app shell and item templates, and the Add button handler.

        Template "todo-app": div > [ input, button("Add") + dyn_attr[0], ul > dyn_node[0] ]
        Template "todo-item": li + dyn_attr[2] > [ span > dyn_text[0],
                                                    button("✓") + dyn_attr[0],
                                                    button("✕") + dyn_attr[1] ]

        Uses multi-arg el_* overloads — no List[Node]() wrappers needed.
        """
        # 1. Create context and signal
        self.ctx = ComponentContext.create()
        self.list_version = self.ctx.use_signal(0)
        self.ctx.end_setup()

        # 2. Register the "todo-app" template (sets ctx.template_id)
        self.ctx.register_template(
            el_div(
                el_input(
                    attr(String("type"), String("text")),
                    attr(
                        String("placeholder"),
                        String("What needs to be done?"),
                    ),
                ),
                el_button(text(String("Add")), dyn_attr(0)),
                el_ul(dyn_node(0)),
            ),
            String("todo-app"),
        )

        # 3. Register the "todo-item" template via KeyedList
        self.items = KeyedList(
            self.ctx.register_extra_template(
                el_li(
                    dyn_attr(2),  # class attr on li
                    el_span(dyn_text(0)),
                    el_button(text(String("✓")), dyn_attr(0)),
                    el_button(text(String("✕")), dyn_attr(1)),
                ),
                String("todo-item"),
            )
        )

        # 4. Register the Add button handler
        self.add_handler = self.ctx.register_handler(
            HandlerEntry.custom(self.ctx.scope_id, String("click"))
        )

        # 5. Initialize remaining state
        self.data = List[TodoItem]()
        self.next_id = 1
        # Phase 19: input_text as SignalString — created (not used) since
        # the input value doesn't drive renders.  Demonstrates the
        # create_signal_string() path (no hook registration, no subscription).
        self.input_text = self.ctx.create_signal_string(String(""))

    fn __moveinit__(out self, deinit other: Self):
        self.ctx = other.ctx^
        self.list_version = other.list_version^
        self.items = other.items^
        self.data = other.data^
        self.next_id = other.next_id
        self.input_text = (
            other.input_text.copy()
        )  # SignalString is Copyable (not ImplicitlyCopyable)
        self.add_handler = other.add_handler

    fn add_item(mut self, text: String):
        """Add a new item and bump the list version signal."""
        if len(text) == 0:
            return
        self.data.append(TodoItem(self.next_id, text, False))
        self.next_id += 1
        self._bump_version()

    fn remove_item(mut self, item_id: Int32):
        """Remove an item by ID and bump the list version signal."""
        for i in range(len(self.data)):
            if self.data[i].id == item_id:
                # Swap-remove for O(1)
                var last = len(self.data) - 1
                if i != last:
                    self.data[i] = self.data[last].copy()
                _ = self.data.pop()
                self._bump_version()
                return

    fn toggle_item(mut self, item_id: Int32):
        """Toggle an item's completed status and bump the list version signal.
        """
        for i in range(len(self.data)):
            if self.data[i].id == item_id:
                self.data[i].completed = not self.data[i].completed
                self._bump_version()
                return

    fn _bump_version(mut self):
        """Increment the list version signal to trigger re-render."""
        self.list_version += 1

    fn build_item_vnode(mut self, item: TodoItem) -> UInt32:
        """Build a keyed VNode for a single todo item.

        Uses Phase 17 ItemBuilder + Phase 18 conditional helpers:
          - `begin_item()` creates child scope + keyed VNodeBuilder
          - `add_custom_event()` registers handler + maps action + adds event attr
          - `add_class_if()` replaces 4-line if/else class pattern
          - `text_when()` replaces 4-line if/else text pattern

        Template "todo-item": li > [ span > dynamic_text[0], button("✓"), button("✕") ]
          dynamic_text[0] = item text (possibly with completion indicator)
          dynamic_attr[0] = click on toggle button → TODO_ACTION_TOGGLE
          dynamic_attr[1] = click on remove button → TODO_ACTION_REMOVE
          dynamic_attr[2] = class on the li element
        """
        var ib = self.items.begin_item(String(item.id), self.ctx)

        # Dynamic text: item text with completion indicator
        ib.add_dyn_text(
            text_when(
                item.completed,
                String("✓ ") + item.text,
                item.text,
            )
        )

        # Dynamic attr 0: toggle handler (click on ✓ button)
        ib.add_custom_event(String("click"), TODO_ACTION_TOGGLE, item.id)

        # Dynamic attr 1: remove handler (click on ✕ button)
        ib.add_custom_event(String("click"), TODO_ACTION_REMOVE, item.id)

        # Dynamic attr 2: class on the li element
        ib.add_class_if(item.completed, String("completed"))

        return ib.index()

    fn build_items_fragment(mut self) -> UInt32:
        """Build a Fragment VNode containing keyed item children.

        Uses KeyedList.begin_rebuild() to destroy old child scopes,
        clear the handler map, and create a new empty fragment, then
        builds each item VNode and pushes it as a fragment child.
        """
        var frag_idx = self.items.begin_rebuild(self.ctx)
        for i in range(len(self.data)):
            var item_idx = self.build_item_vnode(self.data[i].copy())
            self.items.push_child(self.ctx, frag_idx, item_idx)
        return frag_idx

    fn handle_event(mut self, handler_id: UInt32) -> Bool:
        """Dispatch a click event by handler ID.

        Uses Phase 17 `get_action()` to look up the handler in the
        KeyedList's handler map and determine the action (toggle/remove)
        and target item ID.

        Returns True if the handler was found and the action executed,
        False otherwise (e.g. the add_handler which JS handles specially).
        """
        if handler_id == self.add_handler:
            # Add handler — JS must read the input value and call add_item
            return False

        var action = self.items.get_action(handler_id)
        if action.found:
            if action.tag == TODO_ACTION_TOGGLE:
                self.toggle_item(action.data)
                return True
            elif action.tag == TODO_ACTION_REMOVE:
                self.remove_item(action.data)
                return True
        return False

    fn build_app_vnode(mut self) -> UInt32:
        """Build the app shell VNode (TemplateRef for todo-app).

        Template "todo-app": div > [ input, button("Add") + dynamic_attr[0], ul > dynamic[0] ]
          dynamic_attr[0] = click on Add button
          dynamic[0] = placeholder (item list managed separately)
        """
        var vb = self.ctx.vnode_builder()

        # Dynamic node 0: placeholder in the <ul>
        vb.add_dyn_placeholder()

        # Dynamic attr 0: click on Add button
        vb.add_dyn_event(String("click"), self.add_handler)

        return vb.index()


fn todo_app_init() -> UnsafePointer[TodoApp]:
    """Initialize the todo app.  Returns a pointer to the app state.

    All setup happens in TodoApp.__init__() — this function just
    allocates the heap slot and moves the app into it.
    """
    var app_ptr = UnsafePointer[TodoApp].alloc(1)
    app_ptr.init_pointee_move(TodoApp())
    return app_ptr


fn todo_app_destroy(app_ptr: UnsafePointer[TodoApp]):
    """Destroy the todo app and free all resources."""
    app_ptr[0].ctx.destroy()
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
    # Emit all registered templates so JS can build DOM from mutations
    app[0].ctx.shell.emit_templates(writer_ptr)

    # Build the app shell VNode (no items yet — just the template)
    var app_vnode_idx = app[0].build_app_vnode()
    app[0].ctx.current_vnode = Int(app_vnode_idx)

    # Build an empty items fragment and store it
    var frag_idx = app[0].build_items_fragment()

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

    # After CreateEngine, dynamic[0]'s placeholder has an ElementId.
    # Initialize the KeyedList's slot with the anchor and empty fragment.
    var anchor_id: UInt32 = 0
    var app_vnode_ptr = app[0].ctx.store_ptr()[0].get_ptr(app_vnode_idx)
    if app_vnode_ptr[0].dyn_node_id_count() > 0:
        anchor_id = app_vnode_ptr[0].get_dyn_node_id(0)
    app[0].items.init_slot(anchor_id, frag_idx)

    # Append the app shell to root element (id 0)
    writer_ptr[0].append_children(0, num_roots)

    writer_ptr[0].finalize()
    return Int32(writer_ptr[0].offset)


fn todo_app_flush(
    app: UnsafePointer[TodoApp],
    writer_ptr: UnsafePointer[MutationWriter],
) -> Int32:
    """Flush pending updates after a list mutation.

    Uses KeyedList.flush() which delegates fragment transitions
    (empty↔populated) to the reusable flush_fragment lifecycle helper.

    Returns the byte offset (length) of mutation data, or 0 if nothing dirty.
    """
    # Collect and consume dirty scopes via the scheduler
    if not app[0].ctx.consume_dirty():
        return 0

    # Build a new items fragment from the current item list
    var new_frag_idx = app[0].build_items_fragment()

    # Flush via KeyedList (handles all three transitions)
    app[0].items.flush(app[0].ctx, writer_ptr, new_frag_idx)

    writer_ptr[0].finalize()
    return Int32(writer_ptr[0].offset)

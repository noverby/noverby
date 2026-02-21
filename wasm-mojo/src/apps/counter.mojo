# CounterApp — Self-contained counter application.
#
# Orchestrates all subsystems via AppShell:
#   AppShell (Runtime, VNodeStore, ElementIdAllocator, Scheduler)
#   + Templates + VNodes + Create/Diff
#
# Template structure (built via DSL):
#   div
#     span
#       dynamic_text[0]      ← "Count: N"
#     span
#       dynamic_text[1]      ← "Doubled: 2N"
#     button  (text: "+")
#       dynamic_attr[0]      ← onclick → increment handler
#     button  (text: "−")
#       dynamic_attr[1]      ← onclick → decrement handler

from memory import UnsafePointer
from bridge import MutationWriter
from events import HandlerEntry
from component import AppShell, app_shell_create
from vdom import (
    VNode,
    VNodeStore,
    Node,
    el_div,
    el_span,
    el_button,
    text,
    dyn_text,
    dyn_attr,
    to_template,
    VNodeBuilder,
)


struct CounterApp(Movable):
    """Self-contained counter application state."""

    var shell: AppShell
    var scope_id: UInt32
    var count_signal: UInt32
    var doubled_memo: UInt32
    var template_id: UInt32
    var incr_handler: UInt32
    var decr_handler: UInt32
    var current_vnode: Int  # index in store, or -1 if not yet rendered

    fn __init__(out self):
        self.shell = AppShell()
        self.scope_id = 0
        self.count_signal = 0
        self.doubled_memo = 0
        self.template_id = 0
        self.incr_handler = 0
        self.decr_handler = 0
        self.current_vnode = -1

    fn __moveinit__(out self, deinit other: Self):
        self.shell = other.shell^
        self.scope_id = other.scope_id
        self.count_signal = other.count_signal
        self.doubled_memo = other.doubled_memo
        self.template_id = other.template_id
        self.incr_handler = other.incr_handler
        self.decr_handler = other.decr_handler
        self.current_vnode = other.current_vnode

    fn build_count_text(self) -> String:
        """Build the display string "Count: N" from the current signal value."""
        var val = self.shell.peek_signal_i32(self.count_signal)
        return String("Count: ") + String(val)

    fn build_doubled_text(mut self) -> String:
        """Build the display string "Doubled: 2N" from the memo value.

        Recomputes the memo if dirty (signal changed since last compute),
        then reads the cached value.
        """
        if self.shell.memo_is_dirty(self.doubled_memo):
            self.shell.memo_begin_compute(self.doubled_memo)
            var count = self.shell.read_signal_i32(self.count_signal)
            self.shell.memo_end_compute_i32(self.doubled_memo, count * 2)
        var doubled = self.shell.memo_read_i32(self.doubled_memo)
        return String("Doubled: ") + String(doubled)

    fn build_vnode(mut self) -> UInt32:
        """Build a fresh VNode for the counter component.

        Creates a TemplateRef VNode with:
          - dynamic_text[0] = "Count: N"
          - dynamic_text[1] = "Doubled: 2N"
          - dynamic_attr[0] = onclick → incr_handler
          - dynamic_attr[1] = onclick → decr_handler

        Returns the VNode index in the store.
        """
        var vb = VNodeBuilder(self.template_id, self.shell.store)
        vb.add_dyn_text(self.build_count_text())
        vb.add_dyn_text(self.build_doubled_text())
        vb.add_dyn_event(String("click"), self.incr_handler)
        vb.add_dyn_event(String("click"), self.decr_handler)
        return vb.index()


fn counter_app_init() -> UnsafePointer[CounterApp]:
    """Initialize the counter app.  Returns a pointer to the app state.

    Creates: AppShell (runtime, VNode store, element ID allocator,
    scheduler), scope, signal, template, and event handlers.
    """
    var app_ptr = UnsafePointer[CounterApp].alloc(1)
    app_ptr.init_pointee_move(CounterApp())

    # 1. Create subsystem instances via AppShell
    app_ptr[0].shell = app_shell_create()

    # 2. Create root scope, signal, and memo via hooks
    app_ptr[0].scope_id = app_ptr[0].shell.create_root_scope()
    _ = app_ptr[0].shell.begin_render(app_ptr[0].scope_id)
    app_ptr[0].count_signal = app_ptr[0].shell.use_signal_i32(0)
    # Read the signal during render to subscribe the scope to changes
    _ = app_ptr[0].shell.read_signal_i32(app_ptr[0].count_signal)
    # Create memo for "count * 2" (starts dirty, will compute on first render)
    app_ptr[0].doubled_memo = app_ptr[0].shell.use_memo_i32(0)
    # Read the memo's output to subscribe the scope to memo changes
    _ = app_ptr[0].shell.memo_read_i32(app_ptr[0].doubled_memo)
    app_ptr[0].shell.end_render(-1)

    # 3. Build and register the counter template via DSL:
    #    div > [ span > dynamic_text[0],
    #            span > dynamic_text[1],
    #            button > text("+") + dynamic_attr[0],
    #            button > text("−") + dynamic_attr[1] ]
    var view = el_div(
        List[Node](
            el_span(List[Node](dyn_text(0))),
            el_span(List[Node](dyn_text(1))),
            el_button(List[Node](text(String("+")), dyn_attr(0))),
            el_button(List[Node](text(String("-")), dyn_attr(1))),
        )
    )
    var template = to_template(view, String("counter"))
    app_ptr[0].template_id = UInt32(
        app_ptr[0].shell.runtime[0].templates.register(template^)
    )

    # 4. Register event handlers
    app_ptr[0].incr_handler = UInt32(
        app_ptr[0]
        .shell.runtime[0]
        .register_handler(
            HandlerEntry.signal_add(
                app_ptr[0].scope_id,
                app_ptr[0].count_signal,
                1,
                String("click"),
            )
        )
    )
    app_ptr[0].decr_handler = UInt32(
        app_ptr[0]
        .shell.runtime[0]
        .register_handler(
            HandlerEntry.signal_sub(
                app_ptr[0].scope_id,
                app_ptr[0].count_signal,
                1,
                String("click"),
            )
        )
    )

    return app_ptr


fn counter_app_destroy(app_ptr: UnsafePointer[CounterApp]):
    """Destroy the counter app and free all resources."""
    app_ptr[0].shell.destroy()
    app_ptr.destroy_pointee()
    app_ptr.free()


fn counter_app_rebuild(
    app: UnsafePointer[CounterApp],
    writer_ptr: UnsafePointer[MutationWriter],
) -> Int32:
    """Initial render (mount) of the counter app.

    Emits RegisterTemplate mutations for all templates, then builds the
    VNode tree, runs CreateEngine, emits AppendChildren to mount to
    root (id 0), and finalizes the mutation buffer.

    Returns the byte offset (length) of the mutation data written.
    """
    # Build the initial VNode
    var vnode_idx = app[0].build_vnode()
    app[0].current_vnode = Int(vnode_idx)

    # Mount with templates prepended (RegisterTemplate + CreateEngine → append → finalize)
    return app[0].shell.mount_with_templates(writer_ptr, vnode_idx)


fn counter_app_handle_event(
    app: UnsafePointer[CounterApp], handler_id: UInt32, event_type: UInt8
) -> Bool:
    """Dispatch an event to the counter app.

    Returns True if an action was executed, False otherwise.
    """
    return app[0].shell.dispatch_event(handler_id, event_type)


fn counter_app_flush(
    app: UnsafePointer[CounterApp],
    writer_ptr: UnsafePointer[MutationWriter],
) -> Int32:
    """Flush pending updates after event dispatch.

    If dirty scopes exist, re-renders the counter component, diffs the
    old and new VNode trees, and writes mutations to the buffer.

    Returns the byte offset (length) of the mutation data written,
    or 0 if there was nothing to update.
    """
    # Collect and consume dirty scopes via the scheduler
    if not app[0].shell.consume_dirty():
        return 0

    # Build a new VNode with updated state
    var new_idx = app[0].build_vnode()
    var old_idx = UInt32(app[0].current_vnode)

    # Diff old → new via AppShell
    app[0].shell.diff(writer_ptr, old_idx, UInt32(new_idx))

    # Update current vnode
    app[0].current_vnode = Int(new_idx)

    # Finalize
    return app[0].shell.finalize(writer_ptr)

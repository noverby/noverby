# CounterApp — Self-contained counter application.
#
# Orchestrates all subsystems:
#   Runtime (signals, scopes, handlers) + Templates + VNodes + Create/Diff
#
# Template structure:
#   div
#     span
#       dynamic_text[0]      ← "Count: N"
#     button  (text: "+")
#       dynamic_attr[0]      ← onclick → increment handler
#     button  (text: "−")
#       dynamic_attr[1]      ← onclick → decrement handler

from memory import UnsafePointer
from bridge import MutationWriter
from arena import ElementIdAllocator
from signals import Runtime, create_runtime, destroy_runtime
from mutations import CreateEngine, DiffEngine
from events import HandlerEntry
from vdom import (
    TemplateBuilder,
    create_builder,
    destroy_builder,
    VNode,
    VNodeStore,
    DynamicNode,
    DynamicAttr,
    AttributeValue,
    TAG_DIV,
    TAG_SPAN,
    TAG_BUTTON,
)


struct CounterApp(Movable):
    """Self-contained counter application state."""

    var runtime: UnsafePointer[Runtime]
    var store: UnsafePointer[VNodeStore]
    var eid_alloc: UnsafePointer[ElementIdAllocator]
    var scope_id: UInt32
    var count_signal: UInt32
    var template_id: UInt32
    var incr_handler: UInt32
    var decr_handler: UInt32
    var current_vnode: Int  # index in store, or -1 if not yet rendered

    fn __init__(out self):
        self.runtime = UnsafePointer[Runtime]()
        self.store = UnsafePointer[VNodeStore]()
        self.eid_alloc = UnsafePointer[ElementIdAllocator]()
        self.scope_id = 0
        self.count_signal = 0
        self.template_id = 0
        self.incr_handler = 0
        self.decr_handler = 0
        self.current_vnode = -1

    fn __moveinit__(out self, deinit other: Self):
        self.runtime = other.runtime
        self.store = other.store
        self.eid_alloc = other.eid_alloc
        self.scope_id = other.scope_id
        self.count_signal = other.count_signal
        self.template_id = other.template_id
        self.incr_handler = other.incr_handler
        self.decr_handler = other.decr_handler
        self.current_vnode = other.current_vnode

    fn build_count_text(self) -> String:
        """Build the display string "Count: N" from the current signal value."""
        var val = self.runtime[0].peek_signal[Int32](self.count_signal)
        return String("Count: ") + String(val)

    fn build_vnode(mut self) -> UInt32:
        """Build a fresh VNode for the counter component.

        Creates a TemplateRef VNode with:
          - dynamic_text[0] = "Count: N"
          - dynamic_attr[0] = onclick → incr_handler
          - dynamic_attr[1] = onclick → decr_handler

        Returns the VNode index in the store.
        """
        var idx = self.store[0].push(VNode.template_ref(self.template_id))
        # Dynamic text node: "Count: N"
        self.store[0].push_dynamic_node(
            idx, DynamicNode.text_node(self.build_count_text())
        )
        # Dynamic attr 0: onclick on the "+" button
        self.store[0].push_dynamic_attr(
            idx,
            DynamicAttr(
                String("click"),
                AttributeValue.event(self.incr_handler),
                UInt32(0),
            ),
        )
        # Dynamic attr 1: onclick on the "−" button
        self.store[0].push_dynamic_attr(
            idx,
            DynamicAttr(
                String("click"),
                AttributeValue.event(self.decr_handler),
                UInt32(0),
            ),
        )
        return idx


fn counter_app_init() -> UnsafePointer[CounterApp]:
    """Initialize the counter app.  Returns a pointer to the app state.

    Creates: runtime, VNode store, element ID allocator, scope, signal,
    template, and event handlers.
    """
    var app_ptr = UnsafePointer[CounterApp].alloc(1)
    app_ptr.init_pointee_move(CounterApp())

    # 1. Create subsystem instances
    app_ptr[0].runtime = create_runtime()
    app_ptr[0].store = UnsafePointer[VNodeStore].alloc(1)
    app_ptr[0].store.init_pointee_move(VNodeStore())
    app_ptr[0].eid_alloc = UnsafePointer[ElementIdAllocator].alloc(1)
    app_ptr[0].eid_alloc.init_pointee_move(ElementIdAllocator())

    # 2. Create root scope and signal via hooks
    app_ptr[0].scope_id = app_ptr[0].runtime[0].create_scope(0, -1)
    _ = app_ptr[0].runtime[0].begin_scope_render(app_ptr[0].scope_id)
    app_ptr[0].count_signal = app_ptr[0].runtime[0].use_signal_i32(0)
    # Read the signal during render to subscribe the scope to changes
    _ = app_ptr[0].runtime[0].read_signal[Int32](app_ptr[0].count_signal)
    app_ptr[0].runtime[0].end_scope_render(-1)

    # 3. Build and register the counter template:
    #    div > [ span > dynamic_text[0],
    #            button > text("+") + dynamic_attr[0],
    #            button > text("−") + dynamic_attr[1] ]
    var builder_ptr = create_builder(String("counter"))
    var div_idx = builder_ptr[0].push_element(TAG_DIV, -1)
    var span_idx = builder_ptr[0].push_element(TAG_SPAN, Int(div_idx))
    var _dyn_text = builder_ptr[0].push_dynamic_text(0, Int(span_idx))

    var btn_incr = builder_ptr[0].push_element(TAG_BUTTON, Int(div_idx))
    var _text_plus = builder_ptr[0].push_text(String("+"), Int(btn_incr))
    builder_ptr[0].push_dynamic_attr(Int(btn_incr), 0)

    var btn_decr = builder_ptr[0].push_element(TAG_BUTTON, Int(div_idx))
    var _text_minus = builder_ptr[0].push_text(String("-"), Int(btn_decr))
    builder_ptr[0].push_dynamic_attr(Int(btn_decr), 1)

    var template = builder_ptr[0].build()
    app_ptr[0].template_id = UInt32(
        app_ptr[0].runtime[0].templates.register(template^)
    )
    destroy_builder(builder_ptr)

    # 4. Register event handlers
    app_ptr[0].incr_handler = UInt32(
        app_ptr[0]
        .runtime[0]
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
        .runtime[0]
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


fn counter_app_rebuild(
    app: UnsafePointer[CounterApp],
    writer_ptr: UnsafePointer[MutationWriter],
) -> Int32:
    """Initial render (mount) of the counter app.

    Builds the VNode tree, runs CreateEngine, emits AppendChildren to
    mount to root (id 0), and finalizes the mutation buffer.

    Returns the byte offset (length) of the mutation data written.
    """
    # Build the initial VNode
    var vnode_idx = app[0].build_vnode()
    app[0].current_vnode = Int(vnode_idx)

    # Run CreateEngine to emit mount mutations
    var engine = CreateEngine(
        writer_ptr, app[0].eid_alloc, app[0].runtime, app[0].store
    )
    var num_roots = engine.create_node(vnode_idx)

    # Append to root element (id 0)
    writer_ptr[0].append_children(0, num_roots)

    # Finalize
    writer_ptr[0].finalize()
    return Int32(writer_ptr[0].offset)


fn counter_app_handle_event(
    app: UnsafePointer[CounterApp], handler_id: UInt32, event_type: UInt8
) -> Bool:
    """Dispatch an event to the counter app.

    Returns True if an action was executed, False otherwise.
    """
    return app[0].runtime[0].dispatch_event(handler_id, event_type)


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
    # Check for dirty scopes
    if not app[0].runtime[0].has_dirty():
        return 0

    # Drain dirty scopes (we only have one scope, so just drain)
    var _dirty = app[0].runtime[0].drain_dirty()

    # Build a new VNode with updated state
    var new_idx = app[0].build_vnode()
    var old_idx = UInt32(app[0].current_vnode)

    # Diff old → new
    var engine = DiffEngine(
        writer_ptr, app[0].eid_alloc, app[0].runtime, app[0].store
    )
    engine.diff_node(old_idx, UInt32(new_idx))

    # Update current vnode
    app[0].current_vnode = Int(new_idx)

    # Finalize
    writer_ptr[0].finalize()
    return Int32(writer_ptr[0].offset)

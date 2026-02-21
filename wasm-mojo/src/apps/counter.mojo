# CounterApp — Self-contained counter application.
#
# Orchestrates all subsystems via ComponentContext:
#   ComponentContext (AppShell, Runtime, VNodeStore, ElementIdAllocator, Scheduler)
#   + Templates + VNodes + Create/Diff
#
# This version uses the ergonomic reactive handles (SignalI32, MemoI32)
# and ComponentContext to dramatically reduce boilerplate compared to
# the raw AppShell API.
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
from component import ComponentContext
from signals import SignalI32, MemoI32
from vdom import (
    Node,
    el_div,
    el_span,
    el_button,
    text,
    dyn_text,
    dyn_attr,
    VNodeBuilder,
)
from signals.runtime import Runtime


struct CounterApp(Movable):
    """Self-contained counter application state.

    Uses ComponentContext + reactive handles for concise component authoring.
    Compare with the Dioxus equivalent:

        fn App() -> Element {
            let mut count = use_signal(|| 0);
            rsx! {
                h1 { "High-Five counter: {count}" }
                button { onclick: move |_| count += 1, "Up high!" }
                button { onclick: move |_| count -= 1, "Down low!" }
            }
        }
    """

    var ctx: ComponentContext
    var count: SignalI32
    var doubled: MemoI32
    var incr_handler: UInt32
    var decr_handler: UInt32

    fn __init__(out self):
        self.ctx = ComponentContext()
        self.count = SignalI32(0, UnsafePointer[Runtime]())
        self.doubled = MemoI32(0, UnsafePointer[Runtime]())
        self.incr_handler = 0
        self.decr_handler = 0

    fn __moveinit__(out self, deinit other: Self):
        self.ctx = other.ctx^
        self.count = other.count.copy()
        self.doubled = other.doubled.copy()
        self.incr_handler = other.incr_handler
        self.decr_handler = other.decr_handler

    fn build_count_text(self) -> String:
        """Build the display string "Count: N" from the current signal value."""
        return String("Count: ") + String(self.count.peek())

    fn build_doubled_text(mut self) -> String:
        """Build the display string "Doubled: 2N" from the memo value.

        Recomputes the memo if dirty (signal changed since last compute),
        then reads the cached value.
        """
        if self.doubled.is_dirty():
            self.doubled.begin_compute()
            var val = self.count.read()
            self.doubled.end_compute(val * 2)
        return String("Doubled: ") + String(self.doubled.peek())

    fn build_vnode(mut self) -> UInt32:
        """Build a fresh VNode for the counter component.

        Creates a TemplateRef VNode with:
          - dynamic_text[0] = "Count: N"
          - dynamic_text[1] = "Doubled: 2N"
          - dynamic_attr[0] = onclick → incr_handler
          - dynamic_attr[1] = onclick → decr_handler

        Returns the VNode index in the store.
        """
        var vb = self.ctx.vnode_builder()
        vb.add_dyn_text(self.build_count_text())
        vb.add_dyn_text(self.build_doubled_text())
        vb.add_dyn_event(String("click"), self.incr_handler)
        vb.add_dyn_event(String("click"), self.decr_handler)
        return vb.index()


fn counter_app_init() -> UnsafePointer[CounterApp]:
    """Initialize the counter app.  Returns a pointer to the app state.

    Creates: ComponentContext (runtime, VNode store, element ID allocator,
    scheduler), scope, signals, memo, template, and event handlers.
    """
    var app_ptr = UnsafePointer[CounterApp].alloc(1)
    app_ptr.init_pointee_move(CounterApp())

    # 1. Create context with root scope (begins render bracket)
    app_ptr[0].ctx = ComponentContext.create()

    # 2. Create reactive state via hooks (auto-subscribes scope)
    app_ptr[0].count = app_ptr[0].ctx.use_signal(0)
    app_ptr[0].doubled = app_ptr[0].ctx.use_memo(0)

    # 3. End setup (closes render bracket)
    app_ptr[0].ctx.end_setup()

    # 4. Build and register the counter template via DSL:
    #    div > [ span > dynamic_text[0],
    #            span > dynamic_text[1],
    #            button > text("+") + dynamic_attr[0],
    #            button > text("−") + dynamic_attr[1] ]
    app_ptr[0].ctx.register_template(
        el_div(
            List[Node](
                el_span(List[Node](dyn_text(0))),
                el_span(List[Node](dyn_text(1))),
                el_button(List[Node](text(String("+")), dyn_attr(0))),
                el_button(List[Node](text(String("-")), dyn_attr(1))),
            )
        ),
        String("counter"),
    )

    # 5. Register event handlers via ergonomic helpers
    app_ptr[0].incr_handler = app_ptr[0].ctx.on_click_add(app_ptr[0].count, 1)
    app_ptr[0].decr_handler = app_ptr[0].ctx.on_click_sub(app_ptr[0].count, 1)

    return app_ptr


fn counter_app_destroy(app_ptr: UnsafePointer[CounterApp]):
    """Destroy the counter app and free all resources."""
    app_ptr[0].ctx.destroy()
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
    var vnode_idx = app[0].build_vnode()
    return app[0].ctx.mount(writer_ptr, vnode_idx)


fn counter_app_handle_event(
    app: UnsafePointer[CounterApp], handler_id: UInt32, event_type: UInt8
) -> Bool:
    """Dispatch an event to the counter app.

    Returns True if an action was executed, False otherwise.
    """
    return app[0].ctx.dispatch_event(handler_id, event_type)


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
    if not app[0].ctx.consume_dirty():
        return 0

    # Build a new VNode with updated state
    var new_idx = app[0].build_vnode()

    # Diff old → new and update current vnode
    app[0].ctx.diff(writer_ptr, new_idx)

    # Finalize
    return app[0].ctx.finalize(writer_ptr)

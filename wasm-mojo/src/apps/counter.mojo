# CounterApp — Self-contained counter application.
#
# This version uses the ergonomic register_view() + render_builder() API
# with inline event handlers, achieving a Dioxus-like declarative style.
#
# Compare with the Dioxus equivalent:
#
#     fn App() -> Element {
#         let mut count = use_signal(|| 0);
#         rsx! {
#             h1 { "High-Five counter: {count}" }
#             button { onclick: move |_| count += 1, "Up high!" }
#             button { onclick: move |_| count -= 1, "Down low!" }
#         }
#     }
#
# Template structure (built via register_view with inline events):
#   div
#     h1
#       dynamic_text[0]      ← "High-Five counter: N"
#     button  (text: "Up high!")
#       dynamic_attr[0]      ← onclick → increment handler (auto-registered)
#     button  (text: "Down low!")
#       dynamic_attr[1]      ← onclick → decrement handler (auto-registered)

from memory import UnsafePointer
from bridge import MutationWriter
from component import ComponentContext
from signals import SignalI32
from signals.runtime import Runtime
from vdom import (
    Node,
    el_div,
    el_h1,
    el_button,
    text,
    dyn_text,
    onclick_add,
    onclick_sub,
)


struct CounterApp(Movable):
    """Self-contained counter application state.

    Uses ComponentContext + register_view() + render_builder() for
    Dioxus-like concise component authoring.  Inline event handlers
    (onclick_add, onclick_sub) are co-located with the view definition
    and auto-registered — no manual handler ID management needed.
    """

    var ctx: ComponentContext
    var count: SignalI32

    fn __init__(out self):
        self.ctx = ComponentContext()
        self.count = SignalI32(0, UnsafePointer[Runtime]())

    fn __moveinit__(out self, deinit other: Self):
        self.ctx = other.ctx^
        self.count = other.count^

    fn render(mut self) -> UInt32:
        """Build a fresh VNode for the counter component.

        Uses render_builder() which auto-populates the event handler
        attributes registered by register_view().  The component only
        needs to provide dynamic text values.

        Returns the VNode index in the store.
        """
        var vb = self.ctx.render_builder()
        vb.add_dyn_text(
            String("High-Five counter: ") + String(self.count.peek())
        )
        return vb.build()


fn counter_app_init() -> UnsafePointer[CounterApp]:
    """Initialize the counter app.  Returns a pointer to the app state.

    Creates: ComponentContext (runtime, VNode store, element ID allocator,
    scheduler), scope, signal, template with inline event handlers.
    """
    var app_ptr = UnsafePointer[CounterApp].alloc(1)
    app_ptr.init_pointee_move(CounterApp())

    # 1. Create context with root scope (begins render bracket)
    app_ptr[0].ctx = ComponentContext.create()

    # 2. Create reactive state via hooks (auto-subscribes scope)
    app_ptr[0].count = app_ptr[0].ctx.use_signal(0)

    # 3. End setup (closes render bracket)
    app_ptr[0].ctx.end_setup()

    # 4. Register view with inline event handlers — Dioxus-like:
    #    div > [ h1 > dynamic_text[0],
    #            button > text("Up high!")  + onclick(count += 1),
    #            button > text("Down low!") + onclick(count -= 1) ]
    app_ptr[0].ctx.register_view(
        el_div(
            List[Node](
                el_h1(List[Node](dyn_text(0))),
                el_button(
                    List[Node](
                        text(String("Up high!")),
                        onclick_add(app_ptr[0].count, 1),
                    )
                ),
                el_button(
                    List[Node](
                        text(String("Down low!")),
                        onclick_sub(app_ptr[0].count, 1),
                    )
                ),
            )
        ),
        String("counter"),
    )

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
    var vnode_idx = app[0].render()
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
    var new_idx = app[0].render()

    # Diff old → new and update current vnode
    app[0].ctx.diff(writer_ptr, new_idx)

    # Finalize
    return app[0].ctx.finalize(writer_ptr)

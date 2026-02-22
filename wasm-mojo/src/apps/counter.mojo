# CounterApp — Self-contained counter application.
#
# This version achieves maximum Dioxus-like ergonomics by using:
#   - setup_view() — combines end_setup + register_view in one call
#   - dyn_text()   — auto-numbered dynamic text (no manual index tracking)
#   - flush()      — combines diff + finalize in one call
#   - __init__     — all setup happens in the constructor
#   - Multi-arg el_* overloads — no List[Node]() wrappers needed
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
# Mojo equivalent:
#
#     struct CounterApp:
#         var ctx: ComponentContext
#         var count: SignalI32
#
#         fn __init__(out self):
#             self.ctx = ComponentContext.create()
#             self.count = self.ctx.use_signal(0)
#             self.ctx.setup_view(
#                 el_div(
#                     el_h1(dyn_text()),
#                     el_button(text("Up high!"), onclick_add(self.count, 1)),
#                     el_button(text("Down low!"), onclick_sub(self.count, 1)),
#                 ),
#                 String("counter"),
#             )
#
#         fn render(mut self) -> UInt32:
#             var vb = self.ctx.render_builder()
#             vb.add_dyn_text("High-Five counter: " + String(self.count.peek()))
#             return vb.build()
#
# Template structure (built via setup_view with inline events):
#   div
#     h1
#       dynamic_text[0]      ← "High-Five counter: N"  (auto-numbered)
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

    All setup — context creation, signal creation, view registration,
    and event handler binding — happens in __init__.  The lifecycle
    functions are thin one-liners delegating to ComponentContext.
    """

    var ctx: ComponentContext
    var count: SignalI32

    fn __init__(out self):
        """Initialize the counter app with all reactive state and view.

        Creates: ComponentContext (runtime, VNode store, element ID
        allocator, scheduler), root scope, count signal, and the
        template with inline event handlers.

        setup_view() combines end_setup() + register_view():
          - Closes the render bracket (hook registration)
          - Processes the Node tree: auto-numbers dyn_text() slots,
            collects inline event handlers, builds the template,
            and registers handlers

        dyn_text() uses auto-numbering — no manual index needed.

        Multi-arg el_* overloads eliminate List[Node]() wrappers,
        bringing the DSL closer to Dioxus's rsx! macro.
        """
        self.ctx = ComponentContext.create()
        self.count = self.ctx.use_signal(0)
        self.ctx.setup_view(
            el_div(
                el_h1(dyn_text()),
                el_button(
                    text(String("Up high!")),
                    onclick_add(self.count, 1),
                ),
                el_button(
                    text(String("Down low!")),
                    onclick_sub(self.count, 1),
                ),
            ),
            String("counter"),
        )

    fn __moveinit__(out self, deinit other: Self):
        self.ctx = other.ctx^
        self.count = other.count^

    fn render(mut self) -> UInt32:
        """Build a fresh VNode for the counter component.

        Uses render_builder() which auto-populates the event handler
        attributes registered by setup_view().  The component only
        needs to provide dynamic text values (in tree-walk order).

        Returns the VNode index in the store.
        """
        var vb = self.ctx.render_builder()
        vb.add_dyn_text(
            String("High-Five counter: ") + String(self.count.peek())
        )
        return vb.build()


fn counter_app_init() -> UnsafePointer[CounterApp]:
    """Initialize the counter app.  Returns a pointer to the app state.

    All setup happens in CounterApp.__init__() — this function just
    allocates the heap slot and moves the app into it.
    """
    var app_ptr = UnsafePointer[CounterApp].alloc(1)
    app_ptr.init_pointee_move(CounterApp())
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

    var new_idx = app[0].render()
    return app[0].ctx.flush(writer_ptr, new_idx)

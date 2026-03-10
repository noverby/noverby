# Desktop Counter Example — mojo-gui desktop renderer demo.
#
# This example demonstrates running a mojo-gui counter app as a native
# desktop application using GTK4 + WebKitGTK via the desktop renderer.
#
# It reuses the same core framework (signals, vdom, diff, mutations) as
# the web renderer, but sends mutations to an embedded webview instead
# of WASM linear memory.
#
# Architecture:
#
#     ┌─ Native Mojo Process ──────────────────────────────────────────┐
#     │                                                                 │
#     │  CounterApp (mojo-gui/core)                                     │
#     │    ├── ComponentContext (signals, vdom, diff)                    │
#     │    ├── MutationWriter → heap buffer                             │
#     │    └── HandlerRegistry (event dispatch)                         │
#     │         │                                                       │
#     │         ▼                                                       │
#     │  DesktopApp                                                     │
#     │    ├── Webview (GTK4 + WebKitGTK)                               │
#     │    │     ├── desktop-runtime.js (Interpreter)                    │
#     │    │     └── shell.html (#root)                                  │
#     │    └── DesktopBridge                                            │
#     │          ├── flush_mutations() → base64 → JS interpreter        │
#     │          └── poll_event() ← JSON ← window.mojo_post()          │
#     │                                                                 │
#     └─────────────────────────────────────────────────────────────────┘
#
# Build & Run:
#
#     cd mojo-gui/desktop
#     export MOJO_WEBVIEW_LIB=/path/to/libmojo_webview.so
#     export MOJO_GUI_DESKTOP_RUNTIME=runtime/desktop-runtime.js
#     mojo run -I ../core/src -I ../core -I src examples/counter.mojo
#
# Compare with the web version (examples/counter/counter.mojo):
#   - Same CounterApp struct and reactive logic
#   - Different entry point: DesktopApp instead of @export WASM wrappers
#   - Heap buffer instead of WASM linear memory
#   - Event loop polls webview instead of waiting for JS callbacks

from memory import UnsafePointer, memset_zero, alloc
from bridge.protocol import MutationWriter
from component import ComponentContext, ConditionalSlot
from signals.handle import SignalI32, SignalBool
from html import (
    Node,
    VNodeBuilder,
    el_div,
    el_h1,
    el_p,
    el_button,
    text,
    dyn_text,
    dyn_node,
    onclick_add,
    onclick_sub,
    onclick_toggle,
)

from desktop.app import DesktopApp
from desktop.bridge import DesktopEvent


# ── CounterApp ────────────────────────────────────────────────────────────
#
# This is structurally identical to the web counter (examples/counter/counter.mojo).
# The only difference is that we don't use @export WASM wrappers — instead,
# the DesktopApp event loop drives the lifecycle directly.


struct CounterApp(Movable):
    """Self-contained counter application with conditional detail section.

    Same reactive logic as the web version:
      - count: SignalI32 (incremented/decremented by button clicks)
      - show_detail: SignalBool (toggled to show/hide detail section)
      - ConditionalSlot manages the detail DOM lifecycle
    """

    var ctx: ComponentContext
    var count: SignalI32
    var show_detail: SignalBool
    var detail_tmpl: UInt32
    var cond_slot: ConditionalSlot

    fn __init__(out self):
        self.ctx = ComponentContext.create()
        self.count = self.ctx.use_signal(0)
        self.show_detail = self.ctx.use_signal_bool(False)
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
                el_button(
                    text(String("Toggle detail")),
                    onclick_toggle(self.show_detail),
                ),
                dyn_node(1),
            ),
            String("counter"),
        )
        self.detail_tmpl = self.ctx.register_extra_template(
            el_div(
                el_p(dyn_text(0)),
                el_p(dyn_text(1)),
            ),
            String("counter-detail"),
        )
        self.cond_slot = ConditionalSlot()

    fn __moveinit__(out self, deinit other: Self):
        self.ctx = other.ctx^
        self.count = other.count^
        self.show_detail = other.show_detail^
        self.detail_tmpl = other.detail_tmpl
        self.cond_slot = other.cond_slot.copy()

    fn render(mut self) -> UInt32:
        """Build a fresh VNode for the counter component."""
        var vb = self.ctx.render_builder()
        vb.add_dyn_text(
            String("High-Five counter: ") + String(self.count.peek())
        )
        # dyn_node[1] — placeholder for conditional detail
        vb.add_dyn_placeholder()
        return vb.build()

    fn build_detail(mut self) -> UInt32:
        """Build the detail VNode (even/odd + doubled value)."""
        var count_val = self.count.peek()
        var vb = VNodeBuilder(self.detail_tmpl, self.ctx.store_ptr())
        if count_val % 2 == 0:
            vb.add_dyn_text(String("Count is even"))
        else:
            vb.add_dyn_text(String("Count is odd"))
        vb.add_dyn_text(String("Doubled: ") + String(count_val * 2))
        return vb.index()


# ── Helper: allocate a MutationWriter on the heap ─────────────────────────


fn _alloc_writer(
    buf_ptr: UnsafePointer[UInt8, MutExternalOrigin], capacity: Int
) -> UnsafePointer[MutationWriter, MutExternalOrigin]:
    """Allocate a MutationWriter on the heap, pointing at the given buffer."""
    var writer_ptr = alloc[MutationWriter](1)
    writer_ptr.init_pointee_move(MutationWriter(buf_ptr, capacity))
    return writer_ptr


fn _reset_writer(
    writer_ptr: UnsafePointer[MutationWriter, MutExternalOrigin],
    buf_ptr: UnsafePointer[UInt8, MutExternalOrigin],
    capacity: Int,
):
    """Reset the writer for the next render cycle."""
    # Zero out the buffer.
    for i in range(capacity):
        buf_ptr[i] = 0
    writer_ptr.destroy_pointee()
    writer_ptr.init_pointee_move(MutationWriter(buf_ptr, capacity))


fn _free_writer(writer_ptr: UnsafePointer[MutationWriter, MutExternalOrigin]):
    """Free a heap-allocated MutationWriter."""
    writer_ptr.destroy_pointee()
    writer_ptr.free()


# ── Main ──────────────────────────────────────────────────────────────────


fn main() raises:
    """Desktop counter application entry point.

    The event loop follows this pattern:

        1. Initialize the app and webview
        2. Mount the initial DOM (RegisterTemplate + LoadTemplate + events)
        3. Loop:
           a. Process GTK events (non-blocking)
           b. Poll JS events from the webview ring buffer
           c. Dispatch events to the core framework's HandlerRegistry
           d. If any scopes are dirty, re-render and flush mutations
           e. If idle, block briefly to avoid busy-waiting
    """

    # ── 1. Create the desktop app and counter ─────────────────────────

    var desktop = DesktopApp(
        title="mojo-gui Counter",
        width=400,
        height=350,
        debug=True,
    )

    var counter = CounterApp()

    # ── 2. Initialize webview + JS runtime ────────────────────────────

    desktop.init()

    # ── 3. Initial mount ──────────────────────────────────────────────
    #
    # The core framework renders the initial VNode tree into the mutation
    # buffer. We use the bridge's heap buffer instead of WASM linear memory.
    #
    # alloc[] returns UnsafePointer[T, MutExternalOrigin], which is what
    # MutationWriter and ctx.mount() expect.

    var ext_buf = desktop.buf_ptr()
    var cap = desktop.buf_capacity()
    var writer_ptr = _alloc_writer(ext_buf, cap)

    # Render the initial VNode tree and mount it.
    var vnode_idx = counter.render()
    var mount_len = counter.ctx.mount(writer_ptr, vnode_idx)

    # Extract the ConditionalSlot anchor from dyn_node_ids[1].
    var app_vnode_ptr = counter.ctx.store_ptr()[0].get_ptr(vnode_idx)
    if app_vnode_ptr[0].dyn_node_id_count() > 1:
        var anchor_id = app_vnode_ptr[0].get_dyn_node_id(1)
        counter.cond_slot = ConditionalSlot(anchor_id)

    # Send the initial mutations to the webview.
    if mount_len > 0:
        desktop.flush_mutations(Int(mount_len))

    # Pump a few GTK iterations to let the webview render.
    for _ in range(10):
        _ = desktop.step(blocking=False)

    # ── 4. Interactive event loop ─────────────────────────────────────

    while desktop.is_alive():
        # 4a. Process GTK events (non-blocking).
        var closed = desktop.step(blocking=False)
        if closed:
            break

        # 4b. Poll and dispatch JS events.
        var had_event = False
        while True:
            var event = desktop.poll_event()
            if not event.is_valid():
                break
            had_event = True

            # Dispatch to the core framework's HandlerRegistry.
            _ = counter.ctx.dispatch_event(
                UInt32(event.handler_id), UInt8(event.event_type)
            )

        # 4c. If any scopes are dirty, re-render and flush.
        if counter.ctx.consume_dirty():
            # Reset the mutation buffer for the new render cycle.
            _reset_writer(writer_ptr, ext_buf, cap)

            # Re-render and diff the app shell.
            var new_idx = counter.render()
            counter.ctx.diff(writer_ptr, new_idx)

            # Handle the conditional detail section.
            var should_show = counter.show_detail.get()
            if should_show:
                # Show or update the detail section.
                var detail_idx = counter.build_detail()
                counter.cond_slot = counter.ctx.flush_conditional_slot(
                    writer_ptr, counter.cond_slot, detail_idx
                )
            else:
                # Hide the detail section (back to placeholder).
                counter.cond_slot = counter.ctx.flush_conditional_slot_empty(
                    writer_ptr, counter.cond_slot
                )

            # Finalize and flush.
            var flush_len = counter.ctx.finalize(writer_ptr)
            if flush_len > 0:
                desktop.flush_mutations(Int(flush_len))

        # 4d. If idle, block briefly to avoid busy-waiting.
        if not had_event:
            _ = desktop.step(blocking=True)

    # ── 5. Cleanup ────────────────────────────────────────────────────

    _free_writer(writer_ptr)
    counter.ctx.destroy()
    desktop.destroy()

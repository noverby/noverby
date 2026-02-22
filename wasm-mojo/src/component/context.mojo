# ComponentContext — Streamlined component authoring API.
#
# ComponentContext wraps an AppShell and provides a high-level API for
# component authors, dramatically reducing boilerplate compared to raw
# AppShell + Runtime manipulation.
#
# Instead of:
#
#     var shell = app_shell_create()
#     var scope_id = shell.create_root_scope()
#     _ = shell.begin_render(scope_id)
#     var count_key = shell.use_signal_i32(0)
#     _ = shell.read_signal_i32(count_key)    # subscribe scope
#     var memo_id = shell.use_memo_i32(0)
#     _ = shell.memo_read_i32(memo_id)        # subscribe scope
#     shell.end_render(-1)
#     var tmpl_id = UInt32(shell.runtime[0].templates.register(template^))
#     var incr = UInt32(shell.runtime[0].register_handler(
#         HandlerEntry.signal_add(scope_id, count_key, 1, String("click"))
#     ))
#
# Developers write:
#
#     var ctx = ComponentContext.create()
#     var count = ctx.use_signal(0)           # → SignalI32
#     var doubled = ctx.use_memo(0)           # → MemoI32
#     ctx.end_setup()
#     ctx.register_template(view, "counter")
#     var incr = ctx.on_click_add(count, 1)   # → UInt32 handler ID
#
# Multi-template apps (e.g. todo, bench) use register_extra_template():
#
#     var ctx = ComponentContext.create()
#     var version = ctx.use_signal(0)
#     ctx.end_setup()
#     ctx.register_template(app_view, "todo-app")
#     var item_tmpl = ctx.register_extra_template(item_view, "todo-item")
#
# ComponentContext manages:
#   - AppShell creation and lifecycle
#   - Root scope creation and render bracket
#   - Signal/memo/effect creation with automatic scope subscription
#   - Template registration (single primary + extra templates)
#   - Handler registration with short convenience methods
#   - VNode building via the store
#   - Event dispatch, flush, mount, diff lifecycle
#   - Child scope creation and destruction (for keyed list items)
#   - Fragment lifecycle (for dynamic keyed lists)
#
# Ownership: ComponentContext OWNS the AppShell and is responsible for
# destroying it.  The reactive handles (SignalI32, MemoI32, EffectHandle)
# hold non-owning pointers back to the Runtime inside the shell.

from memory import UnsafePointer
from signals import Runtime
from signals.handle import SignalI32, MemoI32, EffectHandle
from events import HandlerEntry
from bridge import MutationWriter
from .app_shell import AppShell, app_shell_create
from .lifecycle import FragmentSlot
from vdom import (
    Node,
    NODE_EVENT,
    NODE_ELEMENT,
    NODE_DYN_TEXT,
    NODE_DYN_ATTR,
    DYN_TEXT_AUTO,
    VNode,
    to_template,
    VNodeBuilder,
    VNodeStore,
)


# ══════════════════════════════════════════════════════════════════════════════
# EventBinding — Stored handler info for auto-populating VNode event attrs
# ══════════════════════════════════════════════════════════════════════════════


struct EventBinding(Copyable, Movable):
    """A registered event handler binding for use by RenderBuilder.

    Stores the event name and handler ID so that `RenderBuilder.build()`
    can automatically add dynamic event attributes to the VNode without
    the component author needing to track handler IDs manually.
    """

    var event_name: String
    var handler_id: UInt32

    fn __init__(out self, event_name: String, handler_id: UInt32):
        self.event_name = event_name
        self.handler_id = handler_id

    fn __copyinit__(out self, other: Self):
        self.event_name = other.event_name
        self.handler_id = other.handler_id

    fn __moveinit__(out self, deinit other: Self):
        self.event_name = other.event_name^
        self.handler_id = other.handler_id


# ══════════════════════════════════════════════════════════════════════════════
# RenderBuilder — VNodeBuilder wrapper that auto-adds registered events
# ══════════════════════════════════════════════════════════════════════════════


struct RenderBuilder(Movable):
    """Ergonomic VNode builder that auto-populates event handler attributes.

    Created by `ComponentContext.render_builder()`.  The component author
    only needs to call `add_dyn_text()` for each dynamic text slot — the
    event handlers registered via `register_view()` are added automatically
    when `build()` is called.

    Usage (in a component's render method):

        fn render(self) -> UInt32:
            var vb = self.ctx.render_builder()
            vb.add_dyn_text("Count: " + str(self.count.peek()))
            return vb.build()

    Compare with manual VNodeBuilder:

        fn build_vnode(self) -> UInt32:
            var vb = self.ctx.vnode_builder()
            vb.add_dyn_text("Count: " + str(self.count.peek()))
            vb.add_dyn_event("click", self.incr_handler)
            vb.add_dyn_event("click", self.decr_handler)
            return vb.index()
    """

    var _vb: VNodeBuilder
    var _events: List[EventBinding]

    fn __init__(
        out self,
        var vb: VNodeBuilder,
        var events: List[EventBinding],
    ):
        self._vb = vb^
        self._events = events^

    fn __moveinit__(out self, deinit other: Self):
        self._vb = other._vb^
        self._events = other._events^

    # ── Dynamic text ─────────────────────────────────────────────────

    fn add_dyn_text(mut self, value: String):
        """Add a dynamic text node (fills the next DynamicText slot).

        Call in order corresponding to `dyn_text(0)`, `dyn_text(1)`, ...
        placeholders in the template.
        """
        self._vb.add_dyn_text(value)

    fn add_dyn_placeholder(mut self):
        """Add a dynamic placeholder node."""
        self._vb.add_dyn_placeholder()

    # ── Dynamic attributes (manual) ─────────────────────────────────

    fn add_dyn_text_attr(mut self, name: String, value: String):
        """Add a dynamic text attribute (e.g. class, id, href)."""
        self._vb.add_dyn_text_attr(name, value)

    fn add_dyn_bool_attr(mut self, name: String, value: Bool):
        """Add a dynamic boolean attribute (e.g. disabled, checked)."""
        self._vb.add_dyn_bool_attr(name, value)

    # ── Build ────────────────────────────────────────────────────────

    fn build(mut self) -> UInt32:
        """Finalize the VNode by auto-adding all registered event handlers.

        Adds dynamic event attributes for each EventBinding registered
        via `register_view()`, then returns the VNode index.

        Returns:
            The VNode's index in the VNodeStore.
        """
        # Auto-add all registered event handler attributes
        for i in range(len(self._events)):
            self._vb.add_dyn_event(
                self._events[i].event_name,
                self._events[i].handler_id,
            )
        return self._vb.index()


struct ComponentContext(Movable):
    """High-level component authoring context.

    Wraps an AppShell and provides ergonomic methods for creating
    signals, memos, effects, handlers, and managing the component
    lifecycle.

    Basic usage (manual handlers):
        var ctx = ComponentContext.create()
        var count = ctx.use_signal(0)
        ctx.end_setup()

        ctx.register_template(view, "counter")
        var incr = ctx.on_click_add(count, 1)
        var decr = ctx.on_click_sub(count, 1)

    Ergonomic usage (inline events via register_view):
        var ctx = ComponentContext.create()
        var count = ctx.use_signal(0)
        ctx.end_setup()

        ctx.register_view(
            el_div(List[Node](
                el_h1(List[Node](dyn_text(0))),
                el_button(List[Node](text("Up high!"), onclick_add(count, 1))),
                el_button(List[Node](text("Down low!"), onclick_sub(count, 1))),
            )),
            "counter",
        )

        # In render:
        var vb = ctx.render_builder()
        vb.add_dyn_text("High-Five counter: " + str(count.peek()))
        var idx = vb.build()  # auto-adds event handlers
    """

    var shell: AppShell
    var scope_id: UInt32
    var template_id: UInt32
    var current_vnode: Int
    var _setup_done: Bool
    var _view_events: List[EventBinding]

    # ── Construction ─────────────────────────────────────────────────

    fn __init__(out self):
        """Create an uninitialized context.  Call create() instead."""
        self.shell = AppShell()
        self.scope_id = 0
        self.template_id = 0
        self.current_vnode = -1
        self._setup_done = False
        self._view_events = List[EventBinding]()

    fn __moveinit__(out self, deinit other: Self):
        self.shell = other.shell^
        self.scope_id = other.scope_id
        self.template_id = other.template_id
        self.current_vnode = other.current_vnode
        self._setup_done = other._setup_done
        self._view_events = other._view_events^

    @staticmethod
    fn create() -> Self:
        """Create a fully initialized ComponentContext.

        Allocates the AppShell, creates a root scope, and begins the
        first render bracket.  Call use_signal/use_memo/use_effect
        to create hooks, then call end_setup() before registering
        templates and handlers.

        Returns:
            A ready-to-use ComponentContext in setup mode.
        """
        var ctx = ComponentContext()
        ctx.shell = app_shell_create()
        ctx.scope_id = ctx.shell.create_root_scope()
        _ = ctx.shell.begin_render(ctx.scope_id)
        ctx._setup_done = False
        return ctx^

    fn end_setup(mut self):
        """End the initial setup (render bracket).

        Must be called after all use_signal/use_memo/use_effect calls
        and before registering templates and handlers.
        """
        self.shell.end_render(-1)
        self._setup_done = True

    fn destroy(mut self):
        """Free all resources.  Safe to call multiple times."""
        self.shell.destroy()

    # ── Signal hooks ─────────────────────────────────────────────────

    fn use_signal(mut self, initial: Int32) -> SignalI32:
        """Create an Int32 signal and subscribe the root scope to it.

        Must be called during setup (before end_setup).

        This is the ergonomic equivalent of:
            var key = shell.use_signal_i32(initial)
            _ = shell.read_signal_i32(key)  # subscribe scope

        Args:
            initial: The initial Int32 value.

        Returns:
            A SignalI32 handle with operator overloading.
        """
        var key = self.shell.use_signal_i32(initial)
        # Read during render to subscribe the scope
        _ = self.shell.read_signal_i32(key)
        return SignalI32(key, self.shell.runtime)

    fn create_signal(mut self, initial: Int32) -> SignalI32:
        """Create an Int32 signal without the hook system.

        Can be called at any time (not just during setup).
        Does NOT auto-subscribe the scope.

        Args:
            initial: The initial Int32 value.

        Returns:
            A SignalI32 handle.
        """
        var key = self.shell.create_signal_i32(initial)
        return SignalI32(key, self.shell.runtime)

    # ── Memo hooks ───────────────────────────────────────────────────

    fn use_memo(mut self, initial: Int32) -> MemoI32:
        """Create an Int32 memo and subscribe the root scope to it.

        Must be called during setup (before end_setup).

        This is the ergonomic equivalent of:
            var memo_id = shell.use_memo_i32(initial)
            _ = shell.memo_read_i32(memo_id)  # subscribe scope

        Args:
            initial: The initial cached Int32 value.

        Returns:
            A MemoI32 handle with read/recompute methods.
        """
        var memo_id = self.shell.use_memo_i32(initial)
        # Read during render to subscribe the scope to memo output
        _ = self.shell.memo_read_i32(memo_id)
        return MemoI32(memo_id, self.shell.runtime)

    fn create_memo(mut self, initial: Int32) -> MemoI32:
        """Create an Int32 memo without the hook system.

        Can be called at any time.  Does NOT auto-subscribe.

        Args:
            initial: The initial cached Int32 value.

        Returns:
            A MemoI32 handle.
        """
        var memo_id = self.shell.create_memo_i32(self.scope_id, initial)
        return MemoI32(memo_id, self.shell.runtime)

    # ── Effect hooks ─────────────────────────────────────────────────

    fn use_effect(mut self) -> EffectHandle:
        """Create an effect and register it with the hook system.

        Must be called during setup (before end_setup).

        Returns:
            An EffectHandle for lifecycle management.
        """
        var effect_id = self.shell.use_effect()
        return EffectHandle(effect_id, self.shell.runtime)

    fn create_effect(mut self) -> EffectHandle:
        """Create an effect without the hook system.

        Can be called at any time.

        Returns:
            An EffectHandle.
        """
        var effect_id = self.shell.create_effect(self.scope_id)
        return EffectHandle(effect_id, self.shell.runtime)

    # ── Template registration ────────────────────────────────────────

    fn register_template(mut self, view: Node, name: String):
        """Build a template from a Node tree and register it.

        Stores the template ID in `self.template_id` for later use
        in VNode building.

        This is the low-level method — use `register_view()` for the
        ergonomic API that also handles inline event handlers.

        Args:
            view: The root Node of the template tree (from DSL helpers).
            name: The template name (for deduplication).
        """
        var template = to_template(view, name)
        self.template_id = UInt32(
            self.shell.runtime[0].templates.register(template^)
        )

    fn register_extra_template(mut self, view: Node, name: String) -> UInt32:
        """Register an additional template without setting self.template_id.

        Use this when a component needs multiple templates — for example,
        an app shell template (registered via `register_template` or
        `setup_view`) plus a list item template for keyed children.

        Example (todo app):
            ctx.register_template(app_view, "todo-app")
            var item_tmpl = ctx.register_extra_template(item_view, "todo-item")

        Args:
            view: The root Node of the template tree (from DSL helpers).
            name: The template name (for deduplication).

        Returns:
            The registered template ID for use with `vnode_builder_for()`.
        """
        var template = to_template(view, name)
        return UInt32(self.shell.runtime[0].templates.register(template^))

    fn setup_view(mut self, view: Node, name: String):
        """End setup and register a view in one call.

        Combines `end_setup()` + `register_view()` for maximum
        conciseness.  Call after all `use_signal()` / `use_memo()` /
        `use_effect()` calls.

        Supports auto-numbered `dyn_text()` nodes (no explicit index)
        — indices are assigned in tree-walk order (0, 1, 2, ...).

        Equivalent Dioxus pattern:
            rsx! {
                h1 { "High-Five counter: {count}" }
                button { onclick: move |_| count += 1, "Up high!" }
                button { onclick: move |_| count -= 1, "Down low!" }
            }

        Mojo equivalent:
            ctx.setup_view(
                el_div(List[Node](
                    el_h1(List[Node](dyn_text())),
                    el_button(List[Node](text("Up high!"), onclick_add(count, 1))),
                    el_button(List[Node](text("Down low!"), onclick_sub(count, 1))),
                )),
                "counter",
            )

        Args:
            view: The root Node of the template tree (may contain
                  NODE_EVENT and auto-numbered dyn_text nodes).
            name: The template name (for deduplication).
        """
        self.end_setup()
        self.register_view(view, name)

    fn register_view(mut self, view: Node, name: String):
        """Build a template from a Node tree with inline event handlers.

        Processes the Node tree to:
        1. Auto-number any `dyn_text()` nodes with sentinel indices
        2. Find all NODE_EVENT nodes and assign dynamic attr indices
        3. Register event handlers for each NODE_EVENT
        4. Build and register the template

        The registered event handlers are stored internally and
        auto-populated by `render_builder().build()`.

        This is the ergonomic alternative to `register_template()` +
        manual `on_click_add()`/`on_click_sub()` calls.

        Supports both explicit `dyn_text(0)` and auto-numbered
        `dyn_text()` (sentinel DYN_TEXT_AUTO) — they can even be mixed,
        though mixing is not recommended.

        Equivalent Dioxus pattern:
            rsx! {
                h1 { "High-Five counter: {count}" }
                button { onclick: move |_| count += 1, "Up high!" }
                button { onclick: move |_| count -= 1, "Down low!" }
            }

        Mojo equivalent:
            ctx.register_view(
                el_div(List[Node](
                    el_h1(List[Node](dyn_text())),
                    el_button(List[Node](text("Up high!"), onclick_add(count, 1))),
                    el_button(List[Node](text("Down low!"), onclick_sub(count, 1))),
                )),
                "counter",
            )

        Args:
            view: The root Node of the template tree (may contain
                  NODE_EVENT nodes from onclick_add, onclick_sub, etc.,
                  and auto-numbered dyn_text() nodes).
            name: The template name (for deduplication).
        """
        # 1. Process tree: auto-number dyn_text, replace NODE_EVENT with
        #    NODE_DYN_ATTR(auto_index), collect event info.
        var events = List[_EventInfo]()
        var attr_idx = UInt32(0)
        var text_idx = UInt32(0)
        var processed = _process_view_tree(view, events, attr_idx, text_idx)

        # 2. Build and register template from processed tree
        var template = to_template(processed, name)
        self.template_id = UInt32(
            self.shell.runtime[0].templates.register(template^)
        )

        # 3. Register handlers and store bindings
        self._view_events = List[EventBinding]()
        for i in range(len(events)):
            var handler_id = self.shell.runtime[0].register_handler(
                HandlerEntry(
                    self.scope_id,
                    events[i].action,
                    events[i].signal_key,
                    events[i].operand,
                    events[i].event_name,
                )
            )
            self._view_events.append(
                EventBinding(events[i].event_name, handler_id)
            )

    # ── Flush convenience ────────────────────────────────────────────

    fn flush(
        mut self,
        writer_ptr: UnsafePointer[MutationWriter],
        new_vnode_idx: UInt32,
    ) -> Int32:
        """Diff old → new VNode, write End sentinel, return byte length.

        Convenience method combining `diff()` + `finalize()`.
        Typical usage in a flush function:

            if not app[0].ctx.consume_dirty():
                return 0
            var new_idx = app[0].render()
            return app[0].ctx.flush(writer_ptr, new_idx)

        Args:
            writer_ptr: Mutation buffer.
            new_vnode_idx: Index of the new VNode from render().

        Returns:
            Byte length of mutation data.
        """
        self.diff(writer_ptr, new_vnode_idx)
        return self.finalize(writer_ptr)

    # ── Handler registration — click events ──────────────────────────

    fn on_click_add(self, signal: SignalI32, delta: Int32) -> UInt32:
        """Register a click handler that adds `delta` to a signal.

        Equivalent to: `onclick: move |_| signal += delta`

        Args:
            signal: The signal to modify.
            delta: The amount to add on each click.

        Returns:
            The handler ID for use in VNode event attributes.
        """
        return self.shell.runtime[0].register_handler(
            HandlerEntry.signal_add(
                self.scope_id, signal.key, delta, String("click")
            )
        )

    fn on_click_sub(self, signal: SignalI32, delta: Int32) -> UInt32:
        """Register a click handler that subtracts `delta` from a signal.

        Equivalent to: `onclick: move |_| signal -= delta`

        Args:
            signal: The signal to modify.
            delta: The amount to subtract on each click.

        Returns:
            The handler ID for use in VNode event attributes.
        """
        return self.shell.runtime[0].register_handler(
            HandlerEntry.signal_sub(
                self.scope_id, signal.key, delta, String("click")
            )
        )

    fn on_click_set(self, signal: SignalI32, value: Int32) -> UInt32:
        """Register a click handler that sets a signal to a fixed value.

        Equivalent to: `onclick: move |_| signal.set(value)`

        Args:
            signal: The signal to modify.
            value: The value to set on each click.

        Returns:
            The handler ID.
        """
        return self.shell.runtime[0].register_handler(
            HandlerEntry.signal_set(
                self.scope_id, signal.key, value, String("click")
            )
        )

    fn on_click_toggle(self, signal: SignalI32) -> UInt32:
        """Register a click handler that toggles a boolean signal (0 ↔ 1).

        Equivalent to: `onclick: move |_| signal.toggle()`

        Args:
            signal: The boolean signal to toggle.

        Returns:
            The handler ID.
        """
        return self.shell.runtime[0].register_handler(
            HandlerEntry.signal_toggle(
                self.scope_id, signal.key, String("click")
            )
        )

    # ── Handler registration — generic events ────────────────────────

    fn on_event_add(
        self, event_name: String, signal: SignalI32, delta: Int32
    ) -> UInt32:
        """Register a handler for any event that adds `delta` to a signal.

        Args:
            event_name: The DOM event name (e.g. "click", "input").
            signal: The signal to modify.
            delta: The amount to add.

        Returns:
            The handler ID.
        """
        return self.shell.runtime[0].register_handler(
            HandlerEntry.signal_add(
                self.scope_id, signal.key, delta, event_name
            )
        )

    fn on_event_sub(
        self, event_name: String, signal: SignalI32, delta: Int32
    ) -> UInt32:
        """Register a handler for any event that subtracts `delta`.

        Args:
            event_name: The DOM event name.
            signal: The signal to modify.
            delta: The amount to subtract.

        Returns:
            The handler ID.
        """
        return self.shell.runtime[0].register_handler(
            HandlerEntry.signal_sub(
                self.scope_id, signal.key, delta, event_name
            )
        )

    fn on_event_set(
        self, event_name: String, signal: SignalI32, value: Int32
    ) -> UInt32:
        """Register a handler for any event that sets a signal.

        Args:
            event_name: The DOM event name.
            signal: The signal to modify.
            value: The value to set.

        Returns:
            The handler ID.
        """
        return self.shell.runtime[0].register_handler(
            HandlerEntry.signal_set(
                self.scope_id, signal.key, value, event_name
            )
        )

    fn on_input_set(self, signal: SignalI32) -> UInt32:
        """Register an input handler that sets a signal from event data.

        Equivalent to: `oninput: move |e| signal.set(e.value)`

        Args:
            signal: The signal to update with the input value.

        Returns:
            The handler ID.
        """
        return self.shell.runtime[0].register_handler(
            HandlerEntry.signal_set_input(
                self.scope_id, signal.key, String("input")
            )
        )

    fn register_handler(self, entry: HandlerEntry) -> UInt32:
        """Register a raw HandlerEntry for full control.

        Use this when the convenience methods don't cover your use case.

        Args:
            entry: The fully configured handler entry.

        Returns:
            The handler ID.
        """
        return self.shell.runtime[0].register_handler(entry)

    # ── VNode building ───────────────────────────────────────────────

    fn vnode_builder(self) -> VNodeBuilder:
        """Create a VNodeBuilder for the context's registered template.

        Returns:
            A VNodeBuilder ready for add_dyn_text/add_dyn_event calls.
        """
        return VNodeBuilder(self.template_id, self.shell.store)

    fn render_builder(mut self) -> RenderBuilder:
        """Create a RenderBuilder that auto-adds registered event handlers.

        Use this with `register_view()` for the ergonomic rendering API.
        Call `add_dyn_text()` for each dynamic text slot, then `build()`
        to finalize (events are added automatically).

        Usage:
            var vb = ctx.render_builder()
            vb.add_dyn_text("High-Five counter: " + str(count.peek()))
            var idx = vb.build()

        Returns:
            A RenderBuilder wrapping a VNodeBuilder with auto-event support.
        """
        var vb = VNodeBuilder(self.template_id, self.shell.store)
        return RenderBuilder(vb^, self._view_events.copy())

    fn vnode_builder_keyed(self, key: String) -> VNodeBuilder:
        """Create a keyed VNodeBuilder for the context's template.

        Args:
            key: The key string for keyed diffing.

        Returns:
            A keyed VNodeBuilder.
        """
        return VNodeBuilder(self.template_id, key, self.shell.store)

    fn vnode_builder_for(self, template_id: UInt32) -> VNodeBuilder:
        """Create a VNodeBuilder for a specific template ID.

        Use this when the component has multiple templates.

        Args:
            template_id: The template to instantiate.

        Returns:
            A VNodeBuilder.
        """
        return VNodeBuilder(template_id, self.shell.store)

    # ── Mount / Rebuild lifecycle ────────────────────────────────────

    fn mount(
        mut self,
        writer_ptr: UnsafePointer[MutationWriter],
        vnode_idx: UInt32,
    ) -> Int32:
        """Initial mount: emit templates + create VNode + append to root.

        Call this for the first render of the component.

        Args:
            writer_ptr: Mutation buffer.
            vnode_idx: Index of the VNode to mount.

        Returns:
            Byte length of mutation data.
        """
        self.current_vnode = Int(vnode_idx)
        return self.shell.mount_with_templates(writer_ptr, vnode_idx)

    # ── Event dispatch ───────────────────────────────────────────────

    fn dispatch_event(mut self, handler_id: UInt32, event_type: UInt8) -> Bool:
        """Dispatch an event to a handler.

        Args:
            handler_id: The handler to invoke.
            event_type: The event type tag (EVT_CLICK, etc.).

        Returns:
            True if an action was executed.
        """
        return self.shell.dispatch_event(handler_id, event_type)

    # ── Flush lifecycle ──────────────────────────────────────────────

    fn has_dirty(self) -> Bool:
        """Check if any scopes need re-rendering."""
        return self.shell.has_dirty()

    fn consume_dirty(mut self) -> Bool:
        """Collect and consume all dirty scopes.

        Returns:
            True if any scopes were dirty.
        """
        return self.shell.consume_dirty()

    fn diff(
        mut self,
        writer_ptr: UnsafePointer[MutationWriter],
        new_vnode_idx: UInt32,
    ):
        """Diff the current VNode against a new one.

        Updates current_vnode to the new index.

        Args:
            writer_ptr: Mutation buffer.
            new_vnode_idx: Index of the new VNode.
        """
        var old_idx = UInt32(self.current_vnode)
        self.shell.diff(writer_ptr, old_idx, new_vnode_idx)
        self.current_vnode = Int(new_vnode_idx)

    fn finalize(self, writer_ptr: UnsafePointer[MutationWriter]) -> Int32:
        """Write the End sentinel and return byte length.

        Args:
            writer_ptr: Mutation buffer.

        Returns:
            Byte length of mutation data.
        """
        return self.shell.finalize(writer_ptr)

    # ── Child scopes (for keyed list items) ──────────────────────────

    fn create_child_scope(mut self) -> UInt32:
        """Create a child scope under the root scope.

        Each keyed list item typically gets its own child scope so that
        its event handlers can be cleaned up independently when the item
        is removed or the list is rebuilt.

        Returns:
            The new child scope ID.
        """
        return self.shell.create_child_scope(self.scope_id)

    fn destroy_child_scopes(mut self, scope_ids: List[UInt32]):
        """Destroy a list of child scopes and clean up their handlers.

        Call this before rebuilding a keyed list to release the old
        per-item scopes and their registered event handlers.

        Args:
            scope_ids: The child scope IDs to destroy.
        """
        self.shell.destroy_child_scopes(scope_ids)

    # ── Fragment lifecycle (for dynamic keyed lists) ─────────────────

    fn flush_fragment(
        mut self,
        writer_ptr: UnsafePointer[MutationWriter],
        slot: FragmentSlot,
        new_frag_idx: UInt32,
    ) -> FragmentSlot:
        """Flush a fragment slot: diff old vs new fragment and emit mutations.

        Delegates to `AppShell.flush_fragment()` which handles all three
        transitions: empty→populated, populated→populated, populated→empty.

        Does NOT call `finalize()` — the caller must finalize the
        mutation buffer after this returns.

        Args:
            writer_ptr: Pointer to the MutationWriter for output.
            slot: The FragmentSlot tracking the current state.
            new_frag_idx: Index of the new Fragment VNode in the store.

        Returns:
            Updated FragmentSlot with new state.
        """
        return self.shell.flush_fragment(writer_ptr, slot, new_frag_idx)

    fn build_empty_fragment(self) -> UInt32:
        """Create an empty Fragment VNode in the store.

        Convenience for initializing a FragmentSlot before the first
        list render.

        Returns:
            The VNode index of the empty fragment.
        """
        return self.shell.store[0].push(VNode.fragment())

    fn push_fragment_child(self, frag_idx: UInt32, child_idx: UInt32):
        """Append a child VNode to an existing Fragment VNode.

        Args:
            frag_idx: Index of the Fragment VNode.
            child_idx: Index of the child VNode to append.
        """
        self.shell.store[0].push_fragment_child(frag_idx, child_idx)

    # ── Accessors for WASM exports ───────────────────────────────────

    fn runtime_ptr(self) -> UnsafePointer[Runtime]:
        """Return the runtime pointer (for WASM export helpers)."""
        return self.shell.runtime

    fn store_ptr(self) -> UnsafePointer[VNodeStore]:
        """Return the VNode store pointer."""
        return self.shell.store

    fn handler_count(self) -> UInt32:
        """Return the number of live event handlers in the runtime.

        Useful for testing and introspection.
        """
        return self.shell.runtime[0].handler_count()

    fn view_events(self) -> List[EventBinding]:
        """Return a copy of the registered view event bindings.

        Useful for testing and introspection.
        """
        return self._view_events.copy()


# ══════════════════════════════════════════════════════════════════════════════
# Private helpers — Event collection and tree reindexing
# ══════════════════════════════════════════════════════════════════════════════


struct _EventInfo(Copyable, Movable):
    """Internal: collected event handler info from a NODE_EVENT node."""

    var event_name: String
    var action: UInt8
    var signal_key: UInt32
    var operand: Int32

    fn __init__(
        out self,
        event_name: String,
        action: UInt8,
        signal_key: UInt32,
        operand: Int32,
    ):
        self.event_name = event_name
        self.action = action
        self.signal_key = signal_key
        self.operand = operand

    fn __copyinit__(out self, other: Self):
        self.event_name = other.event_name
        self.action = other.action
        self.signal_key = other.signal_key
        self.operand = other.operand

    fn __moveinit__(out self, deinit other: Self):
        self.event_name = other.event_name^
        self.action = other.action
        self.signal_key = other.signal_key
        self.operand = other.operand


fn _process_view_tree(
    node: Node,
    mut events: List[_EventInfo],
    mut attr_idx: UInt32,
    mut text_idx: UInt32,
) -> Node:
    """Recursively clone a Node tree, processing events and auto-numbered text.

    Performs two transformations in a single tree walk:

    1. **NODE_EVENT → NODE_DYN_ATTR**: Each NODE_EVENT is collected into
       `events` and replaced with a NODE_DYN_ATTR with an auto-assigned
       dynamic attribute index.

    2. **Auto-numbered dyn_text**: Any NODE_DYN_TEXT node with
       `dynamic_index == DYN_TEXT_AUTO` (the sentinel from `dyn_text()`)
       is replaced with a properly numbered NODE_DYN_TEXT node using the
       next sequential text index.

    All other nodes are cloned unchanged.  ELEMENT nodes have their
    items recursively processed.

    Args:
        node: The current node to process.
        events: Accumulator for collected event info (mutated in place).
        attr_idx: Running counter for dynamic attr indices (mutated).
        text_idx: Running counter for auto-numbered dyn_text indices (mutated).

    Returns:
        A new Node tree with events replaced and dyn_text auto-numbered.
    """
    if node.kind == NODE_EVENT:
        # Collect the event info
        events.append(
            _EventInfo(
                event_name=node.text,
                action=node.tag,
                signal_key=node.dynamic_index,
                operand=node.operand,
            )
        )
        # Replace with a dyn_attr at the current index
        var idx = attr_idx
        attr_idx += 1
        return Node.dynamic_attr_node(idx)

    elif node.kind == NODE_DYN_TEXT:
        if node.dynamic_index == DYN_TEXT_AUTO:
            # Auto-assign the next sequential text index
            var idx = text_idx
            text_idx += 1
            return Node.dynamic_text_node(idx)
        else:
            # Explicit index — pass through but still advance counter
            # past it so auto-numbering doesn't collide
            if node.dynamic_index >= text_idx:
                text_idx = node.dynamic_index + 1
            return node.copy()

    elif node.kind == NODE_ELEMENT:
        # Recursively process items (children + attrs)
        var new_items = List[Node]()
        for i in range(len(node.items)):
            new_items.append(
                _process_view_tree(node.items[i], events, attr_idx, text_idx)
            )
        return Node(
            kind=NODE_ELEMENT,
            tag=node.tag,
            text=String(""),
            attr_value=String(""),
            dynamic_index=0,
            operand=0,
            items=new_items^,
        )

    else:
        # Pass through unchanged (TEXT, DYN_NODE, STATIC_ATTR, DYN_ATTR)
        return node.copy()

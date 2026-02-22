# KeyedList — Abstraction for dynamic keyed list state management.
#
# KeyedList bundles the three pieces of state that every keyed-list
# component needs:
#
#   1. FragmentSlot — tracks empty↔populated DOM transitions
#   2. scope_ids   — child scope IDs for per-item handler cleanup
#   3. template_id — the item template for building VNodes
#
# It provides helper methods that reduce the common 5-line rebuild
# prologue and 3-line flush epilogue to single calls, bringing
# keyed-list apps closer to Dioxus ergonomics.
#
# Usage (in a todo/bench-style app):
#
#     struct TodoApp:
#         var ctx: ComponentContext
#         var items: KeyedList        # replaces 3 separate fields
#
#         fn __init__(out self):
#             self.ctx = ComponentContext.create()
#             ...
#             self.items = KeyedList(item_template_id)
#
#         fn build_items(mut self) -> UInt32:
#             var frag = self.items.begin_rebuild(self.ctx)
#             for i in range(len(self.data)):
#                 var scope = self.items.create_scope(self.ctx)
#                 var vb = self.items.item_builder(key, self.ctx)
#                 # ... fill vb ...
#                 self.items.push_child(self.ctx, frag, vb.index())
#             return frag
#
#         fn flush(mut self, writer):
#             var frag = self.build_items()
#             self.items.flush(self.ctx, writer, frag)
#
# Compare with the old pattern (3 separate fields + manual orchestration):
#
#     var item_template_id: UInt32
#     var item_slot: FragmentSlot
#     var item_scope_ids: List[UInt32]
#
#     fn build_items(mut self) -> UInt32:
#         self.ctx.destroy_child_scopes(self.item_scope_ids)
#         self.item_scope_ids.clear()
#         var frag = self.ctx.build_empty_fragment()
#         for i in range(len(self.data)):
#             var scope = self.ctx.create_child_scope()
#             self.item_scope_ids.append(scope)
#             var vb = VNodeBuilder(self.item_template_id, key, self.ctx.store_ptr())
#             ...

from memory import UnsafePointer
from bridge import MutationWriter
from .lifecycle import FragmentSlot
from .context import ComponentContext
from vdom import VNodeBuilder


struct KeyedList(Movable):
    """Manages the state for a dynamic keyed list within a component.

    Bundles the FragmentSlot (DOM lifecycle), child scope IDs (handler
    cleanup), and item template ID (VNode construction) that every
    keyed-list component needs.

    Helper methods reduce boilerplate for the common rebuild/flush
    cycle.  The component author still writes the per-item build
    logic (which is app-specific), but the surrounding orchestration
    is handled by KeyedList.
    """

    var template_id: UInt32
    var slot: FragmentSlot
    var scope_ids: List[UInt32]

    # ── Construction ─────────────────────────────────────────────────

    fn __init__(out self):
        """Create an uninitialized KeyedList (no template).

        Call `init_slot()` after the initial mount to set the anchor.
        """
        self.template_id = 0
        self.slot = FragmentSlot()
        self.scope_ids = List[UInt32]()

    fn __init__(out self, template_id: UInt32):
        """Create a KeyedList for the given item template.

        Args:
            template_id: The registered template ID for list items.
        """
        self.template_id = template_id
        self.slot = FragmentSlot()
        self.scope_ids = List[UInt32]()

    fn __moveinit__(out self, deinit other: Self):
        self.template_id = other.template_id
        self.slot = other.slot^
        self.scope_ids = other.scope_ids^

    # ── Slot initialization ──────────────────────────────────────────

    fn init_slot(mut self, anchor_id: UInt32, frag_idx: UInt32):
        """Initialize the fragment slot after the initial mount.

        Call this after CreateEngine has assigned an ElementId to the
        list's anchor placeholder node.

        Args:
            anchor_id: ElementId of the placeholder/anchor in the DOM.
            frag_idx: VNode index of the initial (typically empty) fragment.
        """
        self.slot = FragmentSlot(anchor_id, Int(frag_idx))

    # ── Rebuild lifecycle ────────────────────────────────────────────

    fn begin_rebuild(mut self, mut ctx: ComponentContext) -> UInt32:
        """Start a keyed list rebuild: destroy old scopes, return empty fragment.

        Destroys all tracked child scopes (cleaning up their handlers),
        clears the scope list, and creates a new empty Fragment VNode.

        Call this at the start of your build_items method, then iterate
        over your data calling `create_scope()` + `item_builder()` for
        each item, and `push_child()` to add each built VNode.

        Args:
            ctx: The owning component's context (mutated to destroy scopes).

        Returns:
            VNode index of the new empty Fragment.
        """
        ctx.destroy_child_scopes(self.scope_ids)
        self.scope_ids.clear()
        return ctx.build_empty_fragment()

    fn create_scope(mut self, mut ctx: ComponentContext) -> UInt32:
        """Create a child scope for a list item and track it.

        The scope is automatically cleaned up on the next `begin_rebuild()`
        or when the component is destroyed.

        Args:
            ctx: The owning component's context.

        Returns:
            The new child scope ID for handler registration.
        """
        var scope_id = ctx.create_child_scope()
        self.scope_ids.append(scope_id)
        return scope_id

    fn item_builder(self, key: String, ctx: ComponentContext) -> VNodeBuilder:
        """Create a keyed VNodeBuilder for a list item.

        Uses this KeyedList's template_id and the component's store.

        Args:
            key: The unique key for this item (for keyed diffing).
            ctx: The owning component's context.

        Returns:
            A VNodeBuilder ready for add_dyn_text/add_dyn_event calls.
        """
        return VNodeBuilder(self.template_id, key, ctx.store_ptr())

    fn push_child(
        self, ctx: ComponentContext, frag_idx: UInt32, child_idx: UInt32
    ):
        """Append a built item VNode to the fragment.

        Args:
            ctx: The owning component's context.
            frag_idx: The Fragment VNode index (from begin_rebuild).
            child_idx: The item VNode index (from item_builder().index()).
        """
        ctx.push_fragment_child(frag_idx, child_idx)

    # ── Flush lifecycle ──────────────────────────────────────────────

    fn flush(
        mut self,
        mut ctx: ComponentContext,
        writer_ptr: UnsafePointer[MutationWriter],
        new_frag_idx: UInt32,
    ):
        """Flush the keyed list: diff old vs new fragment, emit mutations.

        Delegates to ComponentContext.flush_fragment() which handles all
        three transitions (empty→populated, populated→populated,
        populated→empty).

        Does NOT call finalize() — the caller must finalize the mutation
        buffer after this returns.

        Args:
            ctx: The owning component's context.
            writer_ptr: Pointer to the MutationWriter for output.
            new_frag_idx: Index of the new Fragment VNode from rebuild.
        """
        self.slot = ctx.flush_fragment(writer_ptr, self.slot, new_frag_idx)

    # ── Queries ──────────────────────────────────────────────────────

    fn scope_count(self) -> Int:
        """Return the number of tracked child scopes."""
        return len(self.scope_ids)

    fn is_mounted(self) -> Bool:
        """Check whether the list has items in the DOM."""
        return self.slot.mounted

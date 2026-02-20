# DiffEngine — Compares old and new VNode trees to emit minimal mutations.
#
# The diff engine is the core of the virtual DOM reconciliation algorithm.
# Given an old VNode tree (with mount state from a previous create/diff)
# and a new VNode tree (from a fresh render), it computes the minimal set
# of DOM mutations needed to transform the old DOM into the new DOM.
#
# Strategy (following Dioxus):
#   1. Same template (TemplateRef with same template_id):
#      → Diff only the dynamic nodes and dynamic attributes.
#      → Transfer mount state from old to new.
#   2. Different template or different VNode kind:
#      → Remove old node, create new node (full replacement).
#   3. Text → Text:
#      → If text changed, emit SetText.
#   4. Placeholder → Placeholder:
#      → No-op (nothing to update).
#   5. Fragment → Fragment:
#      → Reconcile children (unkeyed: pairwise diff with boundary adjustments).
#
# The diff engine reads mount state (root_ids, dyn_node_ids, dyn_attr_ids)
# from the old VNode to know which DOM elements to target, and writes
# updated mount state onto the new VNode.

from memory import UnsafePointer
from bridge import MutationWriter
from arena import ElementId, ElementIdAllocator
from signals import Runtime
from vdom import (
    Template,
    TemplateNode,
    TemplateRegistry,
    VNode,
    VNodeStore,
    DynamicNode,
    DynamicAttr,
    AttributeValue,
    TNODE_ELEMENT,
    TNODE_TEXT,
    TNODE_DYNAMIC,
    TNODE_DYNAMIC_TEXT,
    TATTR_STATIC,
    TATTR_DYNAMIC,
    VNODE_TEMPLATE_REF,
    VNODE_TEXT,
    VNODE_PLACEHOLDER,
    VNODE_FRAGMENT,
    AVAL_TEXT,
    AVAL_INT,
    AVAL_FLOAT,
    AVAL_BOOL,
    AVAL_EVENT,
    AVAL_NONE,
    DNODE_TEXT,
    DNODE_PLACEHOLDER,
)
from .create import CreateEngine


# ── Attribute value comparison helpers ────────────────────────────────────────


fn _attr_values_equal(a: AttributeValue, b: AttributeValue) -> Bool:
    """Check whether two AttributeValues are semantically equal."""
    if a.kind != b.kind:
        return False
    if a.kind == AVAL_TEXT:
        return a.text_value == b.text_value
    elif a.kind == AVAL_INT:
        return a.int_value == b.int_value
    elif a.kind == AVAL_FLOAT:
        return a.float_value == b.float_value
    elif a.kind == AVAL_BOOL:
        return a.bool_value == b.bool_value
    elif a.kind == AVAL_EVENT:
        return a.handler_id == b.handler_id
    elif a.kind == AVAL_NONE:
        return True
    return False


fn _attr_value_to_string(value: AttributeValue) -> String:
    """Convert an AttributeValue to its string representation."""
    if value.kind == AVAL_TEXT:
        return value.text_value
    elif value.kind == AVAL_INT:
        return String(value.int_value)
    elif value.kind == AVAL_FLOAT:
        return String(value.float_value)
    elif value.kind == AVAL_BOOL:
        if value.bool_value:
            return String("true")
        else:
            return String("false")
    elif value.kind == AVAL_NONE:
        return String("")
    else:
        return String("")


# ── DiffEngine ───────────────────────────────────────────────────────────────


struct DiffEngine:
    """Compares old and new VNode trees and emits minimal DOM mutations.

    The engine holds references to the shared resources needed during
    diffing: the mutation writer, element ID allocator, runtime (for
    template registry access), and VNode stores (old and new may be in
    the same or different stores).

    Usage:
        var engine = DiffEngine(writer_ptr, eid_ptr, rt_ptr, store_ptr)
        engine.diff_node(old_vnode_index, new_vnode_index)
        # Mutations have been written to the writer
        # The new VNode now has mount state from the old VNode (transferred/updated)
    """

    var writer: UnsafePointer[MutationWriter]
    var eid_alloc: UnsafePointer[ElementIdAllocator]
    var runtime: UnsafePointer[Runtime]
    var store: UnsafePointer[VNodeStore]

    fn __init__(
        out self,
        writer: UnsafePointer[MutationWriter],
        eid_alloc: UnsafePointer[ElementIdAllocator],
        runtime: UnsafePointer[Runtime],
        store: UnsafePointer[VNodeStore],
    ):
        self.writer = writer
        self.eid_alloc = eid_alloc
        self.runtime = runtime
        self.store = store

    fn diff_node(mut self, old_index: UInt32, new_index: UInt32):
        """Diff two VNodes and emit mutations to transform old → new.

        The old VNode must have mount state populated (from a previous
        create or diff).  The new VNode's mount state will be populated
        as a side effect.
        """
        var old_ptr = self.store[0].get_ptr(old_index)
        var new_ptr = self.store[0].get_ptr(new_index)

        var old_kind = old_ptr[0].kind
        var new_kind = new_ptr[0].kind

        # Same kind — try incremental diff
        if old_kind == new_kind:
            if old_kind == VNODE_TEMPLATE_REF:
                # Same template → diff dynamic content only
                if old_ptr[0].template_id == new_ptr[0].template_id:
                    self._diff_template_ref(old_index, new_index)
                    return
                else:
                    # Different template → full replacement
                    self._replace_node(old_index, new_index)
                    return
            elif old_kind == VNODE_TEXT:
                self._diff_text(old_index, new_index)
                return
            elif old_kind == VNODE_PLACEHOLDER:
                self._diff_placeholder(old_index, new_index)
                return
            elif old_kind == VNODE_FRAGMENT:
                self._diff_fragment(old_index, new_index)
                return

        # Different kinds → full replacement
        self._replace_node(old_index, new_index)

    fn _diff_template_ref(mut self, old_index: UInt32, new_index: UInt32):
        """Diff two TemplateRef VNodes with the same template_id.

        Only dynamic nodes and dynamic attributes are compared.
        Mount state is transferred from old to new, then updated
        where dynamic content has changed.
        """
        var old_ptr = self.store[0].get_ptr(old_index)
        var new_ptr = self.store[0].get_ptr(new_index)

        # Transfer mount state: the DOM elements are the same,
        # we just update their content/attributes
        old_ptr[0].transfer_mount_state_to(new_ptr[0])

        # Diff dynamic attributes
        self._diff_dynamic_attrs(old_index, new_index)

        # Diff dynamic nodes
        self._diff_dynamic_nodes(old_index, new_index)

    fn _diff_dynamic_attrs(mut self, old_index: UInt32, new_index: UInt32):
        """Diff dynamic attributes between old and new TemplateRef VNodes.

        For each dynamic attribute that changed, emit SetAttribute or
        NewEventListener / RemoveEventListener mutations.
        """
        var old_ptr = self.store[0].get_ptr(old_index)
        var new_ptr = self.store[0].get_ptr(new_index)

        var old_count = old_ptr[0].dynamic_attr_count()
        var new_count = new_ptr[0].dynamic_attr_count()
        var min_count = old_count
        if new_count < min_count:
            min_count = new_count

        for i in range(min_count):
            # Re-read pointers each iteration (safety)
            var old_p = self.store[0].get_ptr(old_index)
            var new_p = self.store[0].get_ptr(new_index)

            var old_attr = old_p[0].dynamic_attrs[i].copy()
            var new_attr = new_p[0].dynamic_attrs[i].copy()

            # Get the ElementId for this attribute's target element
            var elem_id: UInt32 = 0
            if i < new_p[0].dyn_attr_id_count():
                elem_id = new_p[0].get_dyn_attr_id(i)

            # Check if the value changed
            if not _attr_values_equal(old_attr.value, new_attr.value):
                # Value changed — emit mutation
                if new_attr.value.kind == AVAL_EVENT:
                    if old_attr.value.kind == AVAL_EVENT:
                        # Event handler changed — remove old, add new
                        if (
                            old_attr.value.handler_id
                            != new_attr.value.handler_id
                        ):
                            self.writer[0].remove_event_listener(
                                elem_id, old_attr.name
                            )
                            self.writer[0].new_event_listener(
                                elem_id, new_attr.name
                            )
                    else:
                        # Was not an event, now is
                        self.writer[0].new_event_listener(
                            elem_id, new_attr.name
                        )
                elif new_attr.value.kind == AVAL_NONE:
                    if old_attr.value.kind == AVAL_EVENT:
                        # Remove event listener
                        self.writer[0].remove_event_listener(
                            elem_id, old_attr.name
                        )
                    else:
                        # Remove attribute by setting to empty
                        var ns_byte: UInt8 = 0
                        if new_attr.has_namespace():
                            ns_byte = 1
                        self.writer[0].set_attribute(
                            elem_id, ns_byte, new_attr.name, String("")
                        )
                else:
                    if old_attr.value.kind == AVAL_EVENT:
                        # Was event, now attribute — remove listener first
                        self.writer[0].remove_event_listener(
                            elem_id, old_attr.name
                        )
                    var ns_byte: UInt8 = 0
                    if new_attr.has_namespace():
                        ns_byte = 1
                    var val_str = _attr_value_to_string(new_attr.value)
                    self.writer[0].set_attribute(
                        elem_id, ns_byte, new_attr.name, val_str
                    )

    fn _diff_dynamic_nodes(mut self, old_index: UInt32, new_index: UInt32):
        """Diff dynamic nodes between old and new TemplateRef VNodes.

        For each dynamic node that changed, emit SetText, ReplaceWith, etc.
        """
        var old_ptr = self.store[0].get_ptr(old_index)
        var new_ptr = self.store[0].get_ptr(new_index)

        var old_count = len(old_ptr[0].dynamic_nodes)
        var new_count = len(new_ptr[0].dynamic_nodes)
        var min_count = old_count
        if new_count < min_count:
            min_count = new_count

        for i in range(min_count):
            # Re-read pointers each iteration (safety)
            var old_p = self.store[0].get_ptr(old_index)
            var new_p = self.store[0].get_ptr(new_index)

            var old_node = old_p[0].dynamic_nodes[i].copy()
            var new_node = new_p[0].dynamic_nodes[i].copy()

            # Get the ElementId for this dynamic node
            var node_id: UInt32 = 0
            if i < new_p[0].dyn_node_id_count():
                node_id = new_p[0].get_dyn_node_id(i)

            if old_node.kind == new_node.kind:
                if old_node.kind == DNODE_TEXT:
                    # Both text — check if content changed
                    if old_node.text != new_node.text:
                        self.writer[0].set_text(node_id, new_node.text)
                # Both placeholder — no change needed
            else:
                # Kind changed — need to replace
                if new_node.kind == DNODE_TEXT:
                    # Placeholder → Text: create text, replace old
                    var new_eid = self.eid_alloc[0].alloc()
                    self.writer[0].create_text_node(
                        new_eid.as_u32(), new_node.text
                    )
                    self.writer[0].replace_with(node_id, 1)
                    # Update the dyn_node_id
                    if (
                        i
                        < self.store[0]
                        .get_ptr(new_index)[0]
                        .dyn_node_id_count()
                    ):
                        self.store[0].get_ptr(new_index)[0].dyn_node_ids[
                            i
                        ] = new_eid.as_u32()
                else:
                    # Text → Placeholder: create placeholder, replace old
                    var new_eid = self.eid_alloc[0].alloc()
                    self.writer[0].create_placeholder(new_eid.as_u32())
                    self.writer[0].replace_with(node_id, 1)
                    if (
                        i
                        < self.store[0]
                        .get_ptr(new_index)[0]
                        .dyn_node_id_count()
                    ):
                        self.store[0].get_ptr(new_index)[0].dyn_node_ids[
                            i
                        ] = new_eid.as_u32()

    fn _diff_text(mut self, old_index: UInt32, new_index: UInt32):
        """Diff two Text VNodes.  Emits SetText if content changed."""
        var old_ptr = self.store[0].get_ptr(old_index)
        var new_ptr = self.store[0].get_ptr(new_index)

        # Transfer mount state
        new_ptr[0].element_id = old_ptr[0].element_id
        new_ptr[0].root_ids = old_ptr[0].root_ids.copy()

        if old_ptr[0].text != new_ptr[0].text:
            self.writer[0].set_text(old_ptr[0].element_id, new_ptr[0].text)

    fn _diff_placeholder(mut self, old_index: UInt32, new_index: UInt32):
        """Diff two Placeholder VNodes.  No mutations needed."""
        var old_ptr = self.store[0].get_ptr(old_index)
        var new_ptr = self.store[0].get_ptr(new_index)

        # Transfer mount state
        new_ptr[0].element_id = old_ptr[0].element_id
        new_ptr[0].root_ids = old_ptr[0].root_ids.copy()

    fn _diff_fragment(mut self, old_index: UInt32, new_index: UInt32):
        """Diff two Fragment VNodes.  Reconciles children (unkeyed)."""
        var old_ptr = self.store[0].get_ptr(old_index)
        var new_ptr = self.store[0].get_ptr(new_index)

        var old_child_count = old_ptr[0].fragment_child_count()
        var new_child_count = new_ptr[0].fragment_child_count()

        var min_count = old_child_count
        if new_child_count < min_count:
            min_count = new_child_count

        # Diff common prefix (pairwise)
        for i in range(min_count):
            var old_child = (
                self.store[0].get_ptr(old_index)[0].get_fragment_child(i)
            )
            var new_child = (
                self.store[0].get_ptr(new_index)[0].get_fragment_child(i)
            )
            self.diff_node(old_child, new_child)

        if new_child_count > old_child_count:
            # New children added — create them and append
            # We need a reference point to insert after.
            # Use the last old child's last root element.
            var ref_id: UInt32 = 0
            if old_child_count > 0:
                var last_old_child = (
                    self.store[0]
                    .get_ptr(old_index)[0]
                    .get_fragment_child(old_child_count - 1)
                )
                var last_child_ptr = self.store[0].get_ptr(last_old_child)
                if last_child_ptr[0].root_id_count() > 0:
                    ref_id = last_child_ptr[0].get_root_id(
                        last_child_ptr[0].root_id_count() - 1
                    )
                elif last_child_ptr[0].element_id != 0:
                    ref_id = last_child_ptr[0].element_id

            # Create new children
            var create_engine = CreateEngine(
                self.writer, self.eid_alloc, self.runtime, self.store
            )
            var total_new_roots: UInt32 = 0
            for i in range(old_child_count, new_child_count):
                var new_child = (
                    self.store[0].get_ptr(new_index)[0].get_fragment_child(i)
                )
                total_new_roots += create_engine.create_node(new_child)

            # Insert after the reference point
            if ref_id != 0 and total_new_roots > 0:
                self.writer[0].insert_after(ref_id, total_new_roots)

        elif old_child_count > new_child_count:
            # Old children removed — remove excess
            for i in range(new_child_count, old_child_count):
                var old_child = (
                    self.store[0].get_ptr(old_index)[0].get_fragment_child(i)
                )
                self._remove_node(old_child)

    fn _replace_node(mut self, old_index: UInt32, new_index: UInt32):
        """Replace the old VNode entirely with the new one.

        Removes old DOM nodes and creates new ones in their place.
        """
        var old_ptr = self.store[0].get_ptr(old_index)

        # Find the first root element ID of the old node (the replacement target)
        var old_root_id: UInt32 = 0
        if old_ptr[0].root_id_count() > 0:
            old_root_id = old_ptr[0].get_root_id(0)
        elif old_ptr[0].element_id != 0:
            old_root_id = old_ptr[0].element_id

        if old_root_id == 0:
            # Old node was never mounted — just create the new one
            var create_engine = CreateEngine(
                self.writer, self.eid_alloc, self.runtime, self.store
            )
            _ = create_engine.create_node(new_index)
            return

        # Create new node(s) — they'll be pushed onto the stack
        var create_engine = CreateEngine(
            self.writer, self.eid_alloc, self.runtime, self.store
        )
        var num_new_roots = create_engine.create_node(new_index)

        # Replace old node with new nodes from the stack
        if num_new_roots > 0:
            self.writer[0].replace_with(old_root_id, num_new_roots)

        # Remove any additional old roots (if old had multiple roots)
        var old_ptr2 = self.store[0].get_ptr(old_index)
        for i in range(1, old_ptr2[0].root_id_count()):
            self.writer[0].remove(old_ptr2[0].get_root_id(i))

        # Free old ElementIds
        self._free_mount_ids(old_index)

    fn _remove_node(mut self, vnode_index: UInt32):
        """Remove a VNode's DOM nodes entirely."""
        var node_ptr = self.store[0].get_ptr(vnode_index)

        if node_ptr[0].kind == VNODE_FRAGMENT:
            # Remove all fragment children recursively
            var child_count = node_ptr[0].fragment_child_count()
            for i in range(child_count):
                var child = (
                    self.store[0].get_ptr(vnode_index)[0].get_fragment_child(i)
                )
                self._remove_node(child)
            return

        # Remove each root element
        var root_count = node_ptr[0].root_id_count()
        for i in range(root_count):
            var rid = self.store[0].get_ptr(vnode_index)[0].get_root_id(i)
            self.writer[0].remove(rid)

        # If it's a non-fragment with element_id but no root_ids
        if root_count == 0 and node_ptr[0].element_id != 0:
            self.writer[0].remove(node_ptr[0].element_id)

        # Free the ElementIds
        self._free_mount_ids(vnode_index)

    fn _free_mount_ids(mut self, vnode_index: UInt32):
        """Free all ElementIds associated with a VNode's mount state."""
        var node_ptr = self.store[0].get_ptr(vnode_index)

        # Free root IDs
        for i in range(node_ptr[0].root_id_count()):
            var rid = self.store[0].get_ptr(vnode_index)[0].get_root_id(i)
            self.eid_alloc[0].free(ElementId(rid))

        # Free dynamic node IDs
        for i in range(node_ptr[0].dyn_node_id_count()):
            var nid = self.store[0].get_ptr(vnode_index)[0].get_dyn_node_id(i)
            self.eid_alloc[0].free(ElementId(nid))

        # Free dynamic attr element IDs
        for i in range(node_ptr[0].dyn_attr_id_count()):
            var aid = self.store[0].get_ptr(vnode_index)[0].get_dyn_attr_id(i)
            self.eid_alloc[0].free(ElementId(aid))

        # Clear mount state
        self.store[0].get_ptr(vnode_index)[0].clear_mount_state()

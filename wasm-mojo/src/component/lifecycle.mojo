# Lifecycle — Reusable mount / update / flush orchestration helpers.
#
# These functions encapsulate the common patterns that every app repeats:
#
#   mount_vnode()        — CreateEngine → append to root → finalize
#   diff_and_finalize()  — DiffEngine → finalize
#   flush_one_scope()    — drain dirty → rebuild → diff → finalize
#
# They operate on raw pointers so they can be called from any app struct
# without requiring trait conformance.  The app is responsible for:
#
#   1. Building the VNode (app-specific logic)
#   2. Tracking the "current vnode" index
#   3. Calling these helpers with the right pointers
#
# Example (counter-style app):
#
#     # Initial mount
#     var vnode_idx = self.build_vnode()
#     var byte_len = mount_vnode(writer, eid, rt, store, vnode_idx)
#
#     # Subsequent flush
#     var new_idx = self.build_vnode()
#     var byte_len = diff_and_finalize(writer, eid, rt, store, old_idx, new_idx)

from memory import UnsafePointer
from bridge import MutationWriter
from arena import ElementIdAllocator
from signals import Runtime
from mutations import CreateEngine, DiffEngine
from vdom import VNodeStore


fn mount_vnode(
    writer_ptr: UnsafePointer[MutationWriter],
    eid_ptr: UnsafePointer[ElementIdAllocator],
    rt_ptr: UnsafePointer[Runtime],
    store_ptr: UnsafePointer[VNodeStore],
    vnode_idx: UInt32,
) -> Int32:
    """Initial render: create mutations for a VNode and append to root.

    Runs CreateEngine on the given VNode, emits AppendChildren to the
    root element (id 0), writes the End sentinel, and returns the byte
    length of the mutation data.

    Args:
        writer_ptr: Pointer to a MutationWriter positioned at the start.
        eid_ptr: Pointer to the ElementIdAllocator for assigning IDs.
        rt_ptr: Pointer to the reactive Runtime (for template lookups).
        store_ptr: Pointer to the VNodeStore containing the VNode.
        vnode_idx: Index of the VNode to mount in the store.

    Returns:
        Byte length of the mutation data written to the buffer.
    """
    var engine = CreateEngine(writer_ptr, eid_ptr, rt_ptr, store_ptr)
    var num_roots = engine.create_node(vnode_idx)

    # Append created nodes to the DOM root (element id 0)
    writer_ptr[0].append_children(0, num_roots)

    # Write End sentinel
    writer_ptr[0].finalize()
    return Int32(writer_ptr[0].offset)


fn mount_vnode_to(
    writer_ptr: UnsafePointer[MutationWriter],
    eid_ptr: UnsafePointer[ElementIdAllocator],
    rt_ptr: UnsafePointer[Runtime],
    store_ptr: UnsafePointer[VNodeStore],
    vnode_idx: UInt32,
    parent_id: UInt32,
) -> Int32:
    """Initial render: create mutations for a VNode and append to a
    specific parent element.

    Same as `mount_vnode` but appends to `parent_id` instead of root (0).

    Args:
        writer_ptr: Pointer to a MutationWriter positioned at the start.
        eid_ptr: Pointer to the ElementIdAllocator for assigning IDs.
        rt_ptr: Pointer to the reactive Runtime (for template lookups).
        store_ptr: Pointer to the VNodeStore containing the VNode.
        vnode_idx: Index of the VNode to mount in the store.
        parent_id: ElementId of the parent to append children to.

    Returns:
        Byte length of the mutation data written to the buffer.
    """
    var engine = CreateEngine(writer_ptr, eid_ptr, rt_ptr, store_ptr)
    var num_roots = engine.create_node(vnode_idx)

    writer_ptr[0].append_children(parent_id, num_roots)

    writer_ptr[0].finalize()
    return Int32(writer_ptr[0].offset)


fn diff_and_finalize(
    writer_ptr: UnsafePointer[MutationWriter],
    eid_ptr: UnsafePointer[ElementIdAllocator],
    rt_ptr: UnsafePointer[Runtime],
    store_ptr: UnsafePointer[VNodeStore],
    old_idx: UInt32,
    new_idx: UInt32,
) -> Int32:
    """Diff two VNodes, emit mutations, and finalize the buffer.

    Runs DiffEngine on the old and new VNodes, writes the End sentinel,
    and returns the byte length of the mutation data.

    Args:
        writer_ptr: Pointer to a MutationWriter positioned at the start.
        eid_ptr: Pointer to the ElementIdAllocator.
        rt_ptr: Pointer to the reactive Runtime.
        store_ptr: Pointer to the VNodeStore containing both VNodes.
        old_idx: Index of the old (current) VNode in the store.
        new_idx: Index of the new (updated) VNode in the store.

    Returns:
        Byte length of the mutation data written to the buffer.
    """
    var engine = DiffEngine(writer_ptr, eid_ptr, rt_ptr, store_ptr)
    engine.diff_node(old_idx, new_idx)

    writer_ptr[0].finalize()
    return Int32(writer_ptr[0].offset)


fn diff_no_finalize(
    writer_ptr: UnsafePointer[MutationWriter],
    eid_ptr: UnsafePointer[ElementIdAllocator],
    rt_ptr: UnsafePointer[Runtime],
    store_ptr: UnsafePointer[VNodeStore],
    old_idx: UInt32,
    new_idx: UInt32,
):
    """Diff two VNodes and emit mutations WITHOUT finalizing.

    Useful when multiple diffs need to be batched into a single mutation
    buffer before the End sentinel is written.  The caller must call
    `writer_ptr[0].finalize()` when all diffs are complete.

    Args:
        writer_ptr: Pointer to a MutationWriter.
        eid_ptr: Pointer to the ElementIdAllocator.
        rt_ptr: Pointer to the reactive Runtime.
        store_ptr: Pointer to the VNodeStore containing both VNodes.
        old_idx: Index of the old (current) VNode in the store.
        new_idx: Index of the new (updated) VNode in the store.
    """
    var engine = DiffEngine(writer_ptr, eid_ptr, rt_ptr, store_ptr)
    engine.diff_node(old_idx, new_idx)


fn create_no_finalize(
    writer_ptr: UnsafePointer[MutationWriter],
    eid_ptr: UnsafePointer[ElementIdAllocator],
    rt_ptr: UnsafePointer[Runtime],
    store_ptr: UnsafePointer[VNodeStore],
    vnode_idx: UInt32,
) -> UInt32:
    """Create mutations for a VNode WITHOUT finalizing or appending.

    Returns the number of root elements created and pushed onto the
    mutation stack.  The caller is responsible for emitting
    AppendChildren / InsertAfter / etc. and calling finalize().

    Useful for composing multiple creates into a single mutation buffer.

    Args:
        writer_ptr: Pointer to a MutationWriter.
        eid_ptr: Pointer to the ElementIdAllocator.
        rt_ptr: Pointer to the reactive Runtime.
        store_ptr: Pointer to the VNodeStore containing the VNode.
        vnode_idx: Index of the VNode to create in the store.

    Returns:
        Number of root nodes created (pushed onto the mutation stack).
    """
    var engine = CreateEngine(writer_ptr, eid_ptr, rt_ptr, store_ptr)
    return engine.create_node(vnode_idx)

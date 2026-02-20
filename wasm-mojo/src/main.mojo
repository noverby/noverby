from bridge import MutationWriter
from arena import ElementId, ElementIdAllocator
from signals import Runtime, create_runtime, destroy_runtime, HOOK_SIGNAL
from mutations import CreateEngine, DiffEngine
from events import (
    HandlerRegistry,
    HandlerEntry,
    EVT_CLICK,
    EVT_INPUT,
    EVT_KEY_DOWN,
    EVT_KEY_UP,
    EVT_MOUSE_MOVE,
    EVT_FOCUS,
    EVT_BLUR,
    EVT_SUBMIT,
    EVT_CHANGE,
    EVT_MOUSE_DOWN,
    EVT_MOUSE_UP,
    EVT_MOUSE_ENTER,
    EVT_MOUSE_LEAVE,
    EVT_CUSTOM,
    ACTION_NONE,
    ACTION_SIGNAL_SET_I32,
    ACTION_SIGNAL_ADD_I32,
    ACTION_SIGNAL_SUB_I32,
    ACTION_SIGNAL_TOGGLE,
    ACTION_SIGNAL_SET_INPUT,
    ACTION_CUSTOM,
)
from vdom import (
    TemplateBuilder,
    create_builder,
    destroy_builder,
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
    TAG_DIV,
    TAG_SPAN,
    TAG_P,
    TAG_H1,
    TAG_H2,
    TAG_H3,
    TAG_H4,
    TAG_H5,
    TAG_H6,
    TAG_UL,
    TAG_OL,
    TAG_LI,
    TAG_BUTTON,
    TAG_INPUT,
    TAG_FORM,
    TAG_A,
    TAG_IMG,
    TAG_TABLE,
    TAG_TR,
    TAG_TD,
    TAG_TH,
    TAG_UNKNOWN,
)
from memory import UnsafePointer


# ── Pointer ↔ Int helpers ────────────────────────────────────────────────────
#
# Mojo 0.25 does not support UnsafePointer construction from an integer
# address directly.  We reinterpret the bits via a temporary heap slot.


@always_inline
fn _int_to_ptr(addr: Int) -> UnsafePointer[UInt8]:
    """Reinterpret an integer address as an UnsafePointer[UInt8].

    Stores the integer into a heap slot, then reads it back as a pointer
    via bitcast. Both Int and UnsafePointer are 64-bit on wasm64.
    """
    var slot = UnsafePointer[Int].alloc(1)
    slot[0] = addr
    var result = slot.bitcast[UnsafePointer[UInt8]]()[0]
    slot.free()
    return result


@always_inline
fn _ptr_to_i64(ptr: UnsafePointer[UInt8]) -> Int64:
    """Return the raw address of a pointer as Int64."""
    return Int64(Int(ptr))


@always_inline
fn _int_to_runtime_ptr(addr: Int) -> UnsafePointer[Runtime]:
    """Reinterpret an integer address as an UnsafePointer[Runtime]."""
    var slot = UnsafePointer[Int].alloc(1)
    slot[0] = addr
    var result = slot.bitcast[UnsafePointer[Runtime]]()[0]
    slot.free()
    return result


@always_inline
fn _runtime_ptr_to_i64(ptr: UnsafePointer[Runtime]) -> Int64:
    """Return the raw address of a Runtime pointer as Int64."""
    return Int64(Int(ptr))


@always_inline
fn _int_to_builder_ptr(addr: Int) -> UnsafePointer[TemplateBuilder]:
    """Reinterpret an integer address as an UnsafePointer[TemplateBuilder]."""
    var slot = UnsafePointer[Int].alloc(1)
    slot[0] = addr
    var result = slot.bitcast[UnsafePointer[TemplateBuilder]]()[0]
    slot.free()
    return result


@always_inline
fn _builder_ptr_to_i64(ptr: UnsafePointer[TemplateBuilder]) -> Int64:
    """Return the raw address of a TemplateBuilder pointer as Int64."""
    return Int64(Int(ptr))


@always_inline
fn _int_to_vnode_store_ptr(addr: Int) -> UnsafePointer[VNodeStore]:
    """Reinterpret an integer address as an UnsafePointer[VNodeStore]."""
    var slot = UnsafePointer[Int].alloc(1)
    slot[0] = addr
    var result = slot.bitcast[UnsafePointer[VNodeStore]]()[0]
    slot.free()
    return result


@always_inline
fn _vnode_store_ptr_to_i64(ptr: UnsafePointer[VNodeStore]) -> Int64:
    """Return the raw address of a VNodeStore pointer as Int64."""
    return Int64(Int(ptr))


# ── ElementId Allocator Test Exports ─────────────────────────────────────────


@export
fn eid_alloc_create() -> Int64:
    """Allocate an ElementIdAllocator on the heap."""
    var ptr = UnsafePointer[ElementIdAllocator].alloc(1)
    ptr.init_pointee_move(ElementIdAllocator())
    return Int64(Int(ptr))


@export
fn eid_alloc_destroy(alloc_ptr: Int64):
    """Destroy and free a heap-allocated ElementIdAllocator."""
    var slot = UnsafePointer[Int].alloc(1)
    slot[0] = Int(alloc_ptr)
    var ptr = slot.bitcast[UnsafePointer[ElementIdAllocator]]()[0]
    slot.free()
    ptr.destroy_pointee()
    ptr.free()


fn _get_eid_alloc(alloc_ptr: Int64) -> UnsafePointer[ElementIdAllocator]:
    var slot = UnsafePointer[Int].alloc(1)
    slot[0] = Int(alloc_ptr)
    var ptr = slot.bitcast[UnsafePointer[ElementIdAllocator]]()[0]
    slot.free()
    return ptr


@export
fn eid_alloc(alloc_ptr: Int64) -> Int32:
    """Allocate a new ElementId.  Returns the raw u32 as i32."""
    var a = _get_eid_alloc(alloc_ptr)
    return Int32(a[0].alloc().as_u32())


@export
fn eid_free(alloc_ptr: Int64, id: Int32):
    """Free an ElementId."""
    var a = _get_eid_alloc(alloc_ptr)
    a[0].free(ElementId(UInt32(id)))


@export
fn eid_is_alive(alloc_ptr: Int64, id: Int32) -> Int32:
    """Check whether an ElementId is currently allocated.  Returns 1 or 0."""
    var a = _get_eid_alloc(alloc_ptr)
    if a[0].is_alive(ElementId(UInt32(id))):
        return 1
    return 0


@export
fn eid_count(alloc_ptr: Int64) -> Int32:
    """Number of allocated IDs (including root)."""
    var a = _get_eid_alloc(alloc_ptr)
    return Int32(a[0].count())


@export
fn eid_user_count(alloc_ptr: Int64) -> Int32:
    """Number of user-allocated IDs (excluding root)."""
    var a = _get_eid_alloc(alloc_ptr)
    return Int32(a[0].user_count())


# ── Runtime / Signals Test Exports ───────────────────────────────────────────
#
# These functions exercise the reactive runtime's signal system.
# Each function receives a runtime pointer (Int64) so the JS harness
# can manage the runtime lifecycle.


@export
fn runtime_create() -> Int64:
    """Allocate a reactive Runtime on the heap."""
    return _runtime_ptr_to_i64(create_runtime())


@export
fn runtime_destroy(rt_ptr: Int64):
    """Destroy and free a heap-allocated Runtime."""
    destroy_runtime(_int_to_runtime_ptr(Int(rt_ptr)))


fn _get_runtime(rt_ptr: Int64) -> UnsafePointer[Runtime]:
    return _int_to_runtime_ptr(Int(rt_ptr))


@export
fn signal_create_i32(rt_ptr: Int64, initial: Int32) -> Int32:
    """Create an Int32 signal.  Returns its key."""
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].create_signal[Int32](initial))


@export
fn signal_read_i32(rt_ptr: Int64, key: Int32) -> Int32:
    """Read an Int32 signal (with context tracking)."""
    var rt = _get_runtime(rt_ptr)
    return rt[0].read_signal[Int32](UInt32(key))


@export
fn signal_write_i32(rt_ptr: Int64, key: Int32, value: Int32):
    """Write a new value to an Int32 signal."""
    var rt = _get_runtime(rt_ptr)
    rt[0].write_signal[Int32](UInt32(key), value)


@export
fn signal_peek_i32(rt_ptr: Int64, key: Int32) -> Int32:
    """Read an Int32 signal without subscribing."""
    var rt = _get_runtime(rt_ptr)
    return rt[0].peek_signal[Int32](UInt32(key))


@export
fn signal_destroy(rt_ptr: Int64, key: Int32):
    """Destroy a signal."""
    var rt = _get_runtime(rt_ptr)
    rt[0].destroy_signal(UInt32(key))


@export
fn signal_subscriber_count(rt_ptr: Int64, key: Int32) -> Int32:
    """Return the number of subscribers for a signal."""
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].signals.subscriber_count(UInt32(key)))


@export
fn signal_version(rt_ptr: Int64, key: Int32) -> Int32:
    """Return the write-version counter for a signal."""
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].signals.version(UInt32(key)))


@export
fn signal_count(rt_ptr: Int64) -> Int32:
    """Return the number of live signals."""
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].signals.signal_count())


@export
fn signal_contains(rt_ptr: Int64, key: Int32) -> Int32:
    """Check whether a signal key is live.  Returns 1 or 0."""
    var rt = _get_runtime(rt_ptr)
    if rt[0].signals.contains(UInt32(key)):
        return 1
    return 0


# ── Context management exports ───────────────────────────────────────────────


@export
fn runtime_set_context(rt_ptr: Int64, context_id: Int32):
    """Set the current reactive context."""
    var rt = _get_runtime(rt_ptr)
    rt[0].set_context(UInt32(context_id))


@export
fn runtime_clear_context(rt_ptr: Int64):
    """Clear the current reactive context."""
    var rt = _get_runtime(rt_ptr)
    rt[0].clear_context()


@export
fn runtime_has_context(rt_ptr: Int64) -> Int32:
    """Check if a reactive context is active.  Returns 1 or 0."""
    var rt = _get_runtime(rt_ptr)
    if rt[0].has_context():
        return 1
    return 0


@export
fn runtime_dirty_count(rt_ptr: Int64) -> Int32:
    """Return the number of dirty scopes."""
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].dirty_count())


@export
fn runtime_has_dirty(rt_ptr: Int64) -> Int32:
    """Check if any scopes are dirty.  Returns 1 or 0."""
    var rt = _get_runtime(rt_ptr)
    if rt[0].has_dirty():
        return 1
    return 0


# ── Scope management exports ─────────────────────────────────────────────────
#
# These functions exercise the scope arena and hook system.
# Scopes live inside the Runtime alongside signals.


@export
fn scope_create(rt_ptr: Int64, height: Int32, parent_id: Int32) -> Int32:
    """Create a new scope.  Returns its ID."""
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].create_scope(UInt32(height), Int(parent_id)))


@export
fn scope_create_child(rt_ptr: Int64, parent_id: Int32) -> Int32:
    """Create a child scope.  Height is parent.height + 1."""
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].create_child_scope(UInt32(parent_id)))


@export
fn scope_destroy(rt_ptr: Int64, id: Int32):
    """Destroy a scope."""
    var rt = _get_runtime(rt_ptr)
    rt[0].destroy_scope(UInt32(id))


@export
fn scope_count(rt_ptr: Int64) -> Int32:
    """Return the number of live scopes."""
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].scope_count())


@export
fn scope_contains(rt_ptr: Int64, id: Int32) -> Int32:
    """Check whether a scope ID is live.  Returns 1 or 0."""
    var rt = _get_runtime(rt_ptr)
    if rt[0].scope_contains(UInt32(id)):
        return 1
    return 0


@export
fn scope_height(rt_ptr: Int64, id: Int32) -> Int32:
    """Return the height (depth) of a scope."""
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].scopes.height(UInt32(id)))


@export
fn scope_parent(rt_ptr: Int64, id: Int32) -> Int32:
    """Return the parent ID of a scope, or -1 if root."""
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].scopes.parent_id(UInt32(id)))


@export
fn scope_is_dirty(rt_ptr: Int64, id: Int32) -> Int32:
    """Check whether a scope is dirty.  Returns 1 or 0."""
    var rt = _get_runtime(rt_ptr)
    if rt[0].scopes.is_dirty(UInt32(id)):
        return 1
    return 0


@export
fn scope_set_dirty(rt_ptr: Int64, id: Int32, dirty: Int32):
    """Set the dirty flag on a scope."""
    var rt = _get_runtime(rt_ptr)
    rt[0].scopes.set_dirty(UInt32(id), dirty != 0)


@export
fn scope_render_count(rt_ptr: Int64, id: Int32) -> Int32:
    """Return how many times a scope has been rendered."""
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].scopes.render_count(UInt32(id)))


@export
fn scope_hook_count(rt_ptr: Int64, id: Int32) -> Int32:
    """Return the number of hooks in a scope."""
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].scopes.hook_count(UInt32(id)))


@export
fn scope_hook_value_at(rt_ptr: Int64, id: Int32, index: Int32) -> Int32:
    """Return the hook value (signal key) at position `index` in scope `id`."""
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].scopes.hook_value_at(UInt32(id), Int(index)))


@export
fn scope_hook_tag_at(rt_ptr: Int64, id: Int32, index: Int32) -> Int32:
    """Return the hook tag at position `index` in scope `id`."""
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].scopes.hook_tag_at(UInt32(id), Int(index)))


# ── Scope rendering lifecycle exports ────────────────────────────────────────


@export
fn scope_begin_render(rt_ptr: Int64, scope_id: Int32) -> Int32:
    """Begin rendering a scope.

    Sets current scope and reactive context.
    Returns the previous scope ID (or -1 if none).
    """
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].begin_scope_render(UInt32(scope_id)))


@export
fn scope_end_render(rt_ptr: Int64, prev_scope: Int32):
    """End rendering the current scope and restore the previous scope."""
    var rt = _get_runtime(rt_ptr)
    rt[0].end_scope_render(Int(prev_scope))


@export
fn scope_has_scope(rt_ptr: Int64) -> Int32:
    """Check if a scope is currently active.  Returns 1 or 0."""
    var rt = _get_runtime(rt_ptr)
    if rt[0].has_scope():
        return 1
    return 0


@export
fn scope_get_current(rt_ptr: Int64) -> Int32:
    """Return the current scope ID, or -1 if none."""
    var rt = _get_runtime(rt_ptr)
    if rt[0].has_scope():
        return Int32(rt[0].get_scope())
    return -1


# ── Hook exports ─────────────────────────────────────────────────────────────


@export
fn hook_use_signal_i32(rt_ptr: Int64, initial: Int32) -> Int32:
    """Hook: create or retrieve an Int32 signal for the current scope.

    On first render: creates signal, stores in hooks, returns key.
    On re-render: returns existing signal key (initial ignored).
    """
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].use_signal_i32(initial))


@export
fn scope_is_first_render(rt_ptr: Int64, scope_id: Int32) -> Int32:
    """Check if a scope is on its first render.  Returns 1 or 0."""
    var rt = _get_runtime(rt_ptr)
    if rt[0].scopes.is_first_render(UInt32(scope_id)):
        return 1
    return 0


# ── Template Builder Exports ─────────────────────────────────────────────────
#
# These functions exercise the template builder and registry system.
# The TemplateBuilder is heap-allocated and accessed via Int64 pointer.


@export
fn tmpl_builder_create(name: String) -> Int64:
    """Create a heap-allocated TemplateBuilder.  Returns its pointer."""
    return _builder_ptr_to_i64(create_builder(name))


@export
fn tmpl_builder_destroy(ptr: Int64):
    """Destroy and free a heap-allocated TemplateBuilder."""
    destroy_builder(_int_to_builder_ptr(Int(ptr)))


fn _get_builder(ptr: Int64) -> UnsafePointer[TemplateBuilder]:
    return _int_to_builder_ptr(Int(ptr))


@export
fn tmpl_builder_push_element(
    ptr: Int64, html_tag: Int32, parent: Int32
) -> Int32:
    """Add an Element node.  parent=-1 means root.  Returns node index."""
    var b = _get_builder(ptr)
    return Int32(b[0].push_element(UInt8(html_tag), Int(parent)))


@export
fn tmpl_builder_push_text(ptr: Int64, text: String, parent: Int32) -> Int32:
    """Add a static Text node.  Returns node index."""
    var b = _get_builder(ptr)
    return Int32(b[0].push_text(text, Int(parent)))


@export
fn tmpl_builder_push_dynamic(
    ptr: Int64, dynamic_index: Int32, parent: Int32
) -> Int32:
    """Add a Dynamic node placeholder.  Returns node index."""
    var b = _get_builder(ptr)
    return Int32(b[0].push_dynamic(UInt32(dynamic_index), Int(parent)))


@export
fn tmpl_builder_push_dynamic_text(
    ptr: Int64, dynamic_index: Int32, parent: Int32
) -> Int32:
    """Add a DynamicText node placeholder.  Returns node index."""
    var b = _get_builder(ptr)
    return Int32(b[0].push_dynamic_text(UInt32(dynamic_index), Int(parent)))


@export
fn tmpl_builder_push_static_attr(
    ptr: Int64, node_index: Int32, name: String, value: String
):
    """Add a static attribute to the specified node."""
    var b = _get_builder(ptr)
    b[0].push_static_attr(Int(node_index), name, value)


@export
fn tmpl_builder_push_dynamic_attr(
    ptr: Int64, node_index: Int32, dynamic_index: Int32
):
    """Add a dynamic attribute placeholder to the specified node."""
    var b = _get_builder(ptr)
    b[0].push_dynamic_attr(Int(node_index), UInt32(dynamic_index))


@export
fn tmpl_builder_node_count(ptr: Int64) -> Int32:
    """Return the number of nodes in the builder."""
    var b = _get_builder(ptr)
    return Int32(b[0].node_count())


@export
fn tmpl_builder_root_count(ptr: Int64) -> Int32:
    """Return the number of root nodes in the builder."""
    var b = _get_builder(ptr)
    return Int32(b[0].root_count())


@export
fn tmpl_builder_attr_count(ptr: Int64) -> Int32:
    """Return the total number of attributes in the builder."""
    var b = _get_builder(ptr)
    return Int32(b[0].attr_count())


@export
fn tmpl_builder_register(rt_ptr: Int64, builder_ptr: Int64) -> Int32:
    """Build the template and register it in the runtime.  Returns template ID.

    The builder is consumed (reset to empty) after this call.
    """
    var rt = _get_runtime(rt_ptr)
    var b = _get_builder(builder_ptr)
    var template = b[0].build()
    return Int32(rt[0].templates.register(template^))


# ── Template Registry Query Exports ──────────────────────────────────────────


@export
fn tmpl_count(rt_ptr: Int64) -> Int32:
    """Return the number of registered templates."""
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].templates.count())


@export
fn tmpl_root_count(rt_ptr: Int64, tmpl_id: Int32) -> Int32:
    """Return the number of root nodes in the template."""
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].templates.root_count(UInt32(tmpl_id)))


@export
fn tmpl_node_count(rt_ptr: Int64, tmpl_id: Int32) -> Int32:
    """Return the total number of nodes in the template."""
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].templates.node_count(UInt32(tmpl_id)))


@export
fn tmpl_node_kind(rt_ptr: Int64, tmpl_id: Int32, node_idx: Int32) -> Int32:
    """Return the kind tag (TNODE_*) of the node at node_idx."""
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].templates.node_kind(UInt32(tmpl_id), Int(node_idx)))


@export
fn tmpl_node_tag(rt_ptr: Int64, tmpl_id: Int32, node_idx: Int32) -> Int32:
    """Return the HTML tag constant of the Element node at node_idx."""
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].templates.node_html_tag(UInt32(tmpl_id), Int(node_idx)))


@export
fn tmpl_node_child_count(
    rt_ptr: Int64, tmpl_id: Int32, node_idx: Int32
) -> Int32:
    """Return the number of children of the node at node_idx."""
    var rt = _get_runtime(rt_ptr)
    return Int32(
        rt[0].templates.node_child_count(UInt32(tmpl_id), Int(node_idx))
    )


@export
fn tmpl_node_child_at(
    rt_ptr: Int64, tmpl_id: Int32, node_idx: Int32, child_pos: Int32
) -> Int32:
    """Return the node index of the child at child_pos within node_idx."""
    var rt = _get_runtime(rt_ptr)
    return Int32(
        rt[0].templates.node_child_at(
            UInt32(tmpl_id), Int(node_idx), Int(child_pos)
        )
    )


@export
fn tmpl_node_dynamic_index(
    rt_ptr: Int64, tmpl_id: Int32, node_idx: Int32
) -> Int32:
    """Return the dynamic slot index of the node at node_idx."""
    var rt = _get_runtime(rt_ptr)
    return Int32(
        rt[0].templates.node_dynamic_index(UInt32(tmpl_id), Int(node_idx))
    )


@export
fn tmpl_node_attr_count(
    rt_ptr: Int64, tmpl_id: Int32, node_idx: Int32
) -> Int32:
    """Return the number of attributes on the node at node_idx."""
    var rt = _get_runtime(rt_ptr)
    return Int32(
        rt[0].templates.node_attr_count(UInt32(tmpl_id), Int(node_idx))
    )


@export
fn tmpl_attr_total_count(rt_ptr: Int64, tmpl_id: Int32) -> Int32:
    """Return the total number of attributes in the template."""
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].templates.attr_total_count(UInt32(tmpl_id)))


@export
fn tmpl_get_root_index(rt_ptr: Int64, tmpl_id: Int32, root_pos: Int32) -> Int32:
    """Return the node index of the root at position root_pos."""
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].templates.get_root_index(UInt32(tmpl_id), Int(root_pos)))


@export
fn tmpl_attr_kind(rt_ptr: Int64, tmpl_id: Int32, attr_idx: Int32) -> Int32:
    """Return the kind (TATTR_*) of the attribute at attr_idx."""
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].templates.get_attr_kind(UInt32(tmpl_id), Int(attr_idx)))


@export
fn tmpl_attr_dynamic_index(
    rt_ptr: Int64, tmpl_id: Int32, attr_idx: Int32
) -> Int32:
    """Return the dynamic index of the attribute at attr_idx."""
    var rt = _get_runtime(rt_ptr)
    return Int32(
        rt[0].templates.get_attr_dynamic_index(UInt32(tmpl_id), Int(attr_idx))
    )


@export
fn tmpl_dynamic_node_count(rt_ptr: Int64, tmpl_id: Int32) -> Int32:
    """Return the number of Dynamic node slots in the template."""
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].templates.dynamic_node_count(UInt32(tmpl_id)))


@export
fn tmpl_dynamic_text_count(rt_ptr: Int64, tmpl_id: Int32) -> Int32:
    """Return the number of DynamicText slots in the template."""
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].templates.dynamic_text_count(UInt32(tmpl_id)))


@export
fn tmpl_dynamic_attr_count(rt_ptr: Int64, tmpl_id: Int32) -> Int32:
    """Return the number of dynamic attribute slots in the template."""
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].templates.dynamic_attr_count(UInt32(tmpl_id)))


@export
fn tmpl_static_attr_count(rt_ptr: Int64, tmpl_id: Int32) -> Int32:
    """Return the number of static attributes in the template."""
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].templates.static_attr_count(UInt32(tmpl_id)))


@export
fn tmpl_contains_name(rt_ptr: Int64, name: String) -> Int32:
    """Check if a template with the given name is registered.  Returns 1 or 0.
    """
    var rt = _get_runtime(rt_ptr)
    if rt[0].templates.contains_name(name):
        return 1
    return 0


@export
fn tmpl_find_by_name(rt_ptr: Int64, name: String) -> Int32:
    """Find a template by name.  Returns ID or -1 if not found."""
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].templates.find_by_name(name))


@export
fn tmpl_node_first_attr(
    rt_ptr: Int64, tmpl_id: Int32, node_idx: Int32
) -> Int32:
    """Return the first attribute index of the node at node_idx."""
    var rt = _get_runtime(rt_ptr)
    return Int32(
        rt[0].templates.node_first_attr(UInt32(tmpl_id), Int(node_idx))
    )


@export
fn tmpl_node_text(rt_ptr: Int64, tmpl_id: Int32, node_idx: Int32) -> String:
    """Return the static text content of a Text node in the template."""
    var rt = _get_runtime(rt_ptr)
    var tmpl_ptr = rt[0].templates.get_ptr(UInt32(tmpl_id))
    var node_ptr = tmpl_ptr[0].get_node_ptr(Int(node_idx))
    return node_ptr[0].text


@export
fn tmpl_attr_name(rt_ptr: Int64, tmpl_id: Int32, attr_idx: Int32) -> String:
    """Return the name of the attribute at attr_idx in the template."""
    var rt = _get_runtime(rt_ptr)
    var tmpl_ptr = rt[0].templates.get_ptr(UInt32(tmpl_id))
    return tmpl_ptr[0].attrs[Int(attr_idx)].name


@export
fn tmpl_attr_value(rt_ptr: Int64, tmpl_id: Int32, attr_idx: Int32) -> String:
    """Return the value of the attribute at attr_idx in the template."""
    var rt = _get_runtime(rt_ptr)
    var tmpl_ptr = rt[0].templates.get_ptr(UInt32(tmpl_id))
    return tmpl_ptr[0].attrs[Int(attr_idx)].value


# ── VNode Store Exports ──────────────────────────────────────────────────────
#
# VNodes are stored in a VNodeStore (heap-allocated separately or embedded
# in the Runtime).  These exports use the Runtime's built-in VNodeStore.


@export
fn vnode_store_create() -> Int64:
    """Allocate a standalone VNodeStore on the heap.  Returns its pointer."""
    var ptr = UnsafePointer[VNodeStore].alloc(1)
    ptr.init_pointee_move(VNodeStore())
    return _vnode_store_ptr_to_i64(ptr)


@export
fn vnode_store_destroy(store_ptr: Int64):
    """Destroy and free a heap-allocated VNodeStore."""
    var ptr = _int_to_vnode_store_ptr(Int(store_ptr))
    ptr.destroy_pointee()
    ptr.free()


fn _get_vnode_store(store_ptr: Int64) -> UnsafePointer[VNodeStore]:
    return _int_to_vnode_store_ptr(Int(store_ptr))


@export
fn vnode_push_template_ref(store_ptr: Int64, tmpl_id: Int32) -> Int32:
    """Create a TemplateRef VNode and push it into the store.  Returns index."""
    var s = _get_vnode_store(store_ptr)
    return Int32(s[0].push(VNode.template_ref(UInt32(tmpl_id))))


@export
fn vnode_push_template_ref_keyed(
    store_ptr: Int64, tmpl_id: Int32, key: String
) -> Int32:
    """Create a keyed TemplateRef VNode.  Returns index."""
    var s = _get_vnode_store(store_ptr)
    return Int32(s[0].push(VNode.template_ref_keyed(UInt32(tmpl_id), key)))


@export
fn vnode_push_text(store_ptr: Int64, text: String) -> Int32:
    """Create a Text VNode and push it into the store.  Returns index."""
    var s = _get_vnode_store(store_ptr)
    return Int32(s[0].push(VNode.text_node(text)))


@export
fn vnode_push_placeholder(store_ptr: Int64, element_id: Int32) -> Int32:
    """Create a Placeholder VNode.  Returns index."""
    var s = _get_vnode_store(store_ptr)
    return Int32(s[0].push(VNode.placeholder(UInt32(element_id))))


@export
fn vnode_push_fragment(store_ptr: Int64) -> Int32:
    """Create an empty Fragment VNode.  Returns index."""
    var s = _get_vnode_store(store_ptr)
    return Int32(s[0].push(VNode.fragment()))


@export
fn vnode_count(store_ptr: Int64) -> Int32:
    """Return the number of VNodes in the store."""
    var s = _get_vnode_store(store_ptr)
    return Int32(s[0].count())


@export
fn vnode_kind(store_ptr: Int64, index: Int32) -> Int32:
    """Return the kind tag (VNODE_*) of the VNode at index."""
    var s = _get_vnode_store(store_ptr)
    return Int32(s[0].kind(UInt32(index)))


@export
fn vnode_template_id(store_ptr: Int64, index: Int32) -> Int32:
    """Return the template_id of the TemplateRef VNode at index."""
    var s = _get_vnode_store(store_ptr)
    return Int32(s[0].template_id(UInt32(index)))


@export
fn vnode_element_id(store_ptr: Int64, index: Int32) -> Int32:
    """Return the element_id of the Placeholder VNode at index."""
    var s = _get_vnode_store(store_ptr)
    return Int32(s[0].element_id(UInt32(index)))


@export
fn vnode_has_key(store_ptr: Int64, index: Int32) -> Int32:
    """Check if the VNode has a key.  Returns 1 or 0."""
    var s = _get_vnode_store(store_ptr)
    if s[0].has_key(UInt32(index)):
        return 1
    return 0


@export
fn vnode_dynamic_node_count(store_ptr: Int64, index: Int32) -> Int32:
    """Return the number of dynamic nodes on the VNode."""
    var s = _get_vnode_store(store_ptr)
    return Int32(s[0].dynamic_node_count(UInt32(index)))


@export
fn vnode_dynamic_attr_count(store_ptr: Int64, index: Int32) -> Int32:
    """Return the number of dynamic attributes on the VNode."""
    var s = _get_vnode_store(store_ptr)
    return Int32(s[0].dynamic_attr_count(UInt32(index)))


@export
fn vnode_fragment_child_count(store_ptr: Int64, index: Int32) -> Int32:
    """Return the number of fragment children on the VNode."""
    var s = _get_vnode_store(store_ptr)
    return Int32(s[0].fragment_child_count(UInt32(index)))


@export
fn vnode_fragment_child_at(
    store_ptr: Int64, index: Int32, child_pos: Int32
) -> Int32:
    """Return the VNode index of the fragment child at child_pos."""
    var s = _get_vnode_store(store_ptr)
    return Int32(s[0].get_fragment_child(UInt32(index), Int(child_pos)))


@export
fn vnode_push_dynamic_text_node(
    store_ptr: Int64, vnode_index: Int32, text: String
):
    """Append a dynamic text node to the VNode at vnode_index."""
    var s = _get_vnode_store(store_ptr)
    s[0].push_dynamic_node(UInt32(vnode_index), DynamicNode.text_node(text))


@export
fn vnode_push_dynamic_placeholder(store_ptr: Int64, vnode_index: Int32):
    """Append a dynamic placeholder node to the VNode at vnode_index."""
    var s = _get_vnode_store(store_ptr)
    s[0].push_dynamic_node(UInt32(vnode_index), DynamicNode.placeholder())


@export
fn vnode_push_dynamic_attr_text(
    store_ptr: Int64,
    vnode_index: Int32,
    name: String,
    value: String,
    elem_id: Int32,
):
    """Append a dynamic text attribute to the VNode."""
    var s = _get_vnode_store(store_ptr)
    s[0].push_dynamic_attr(
        UInt32(vnode_index),
        DynamicAttr(name, AttributeValue.text(value), UInt32(elem_id)),
    )


@export
fn vnode_push_dynamic_attr_int(
    store_ptr: Int64,
    vnode_index: Int32,
    name: String,
    value: Int32,
    elem_id: Int32,
):
    """Append a dynamic integer attribute to the VNode."""
    var s = _get_vnode_store(store_ptr)
    s[0].push_dynamic_attr(
        UInt32(vnode_index),
        DynamicAttr(
            name, AttributeValue.integer(Int64(value)), UInt32(elem_id)
        ),
    )


@export
fn vnode_push_dynamic_attr_bool(
    store_ptr: Int64,
    vnode_index: Int32,
    name: String,
    value: Int32,
    elem_id: Int32,
):
    """Append a dynamic boolean attribute to the VNode."""
    var s = _get_vnode_store(store_ptr)
    s[0].push_dynamic_attr(
        UInt32(vnode_index),
        DynamicAttr(name, AttributeValue.boolean(value != 0), UInt32(elem_id)),
    )


@export
fn vnode_push_dynamic_attr_event(
    store_ptr: Int64,
    vnode_index: Int32,
    name: String,
    handler_id: Int32,
    elem_id: Int32,
):
    """Append a dynamic event handler attribute to the VNode."""
    var s = _get_vnode_store(store_ptr)
    s[0].push_dynamic_attr(
        UInt32(vnode_index),
        DynamicAttr(
            name, AttributeValue.event(UInt32(handler_id)), UInt32(elem_id)
        ),
    )


@export
fn vnode_push_dynamic_attr_none(
    store_ptr: Int64, vnode_index: Int32, name: String, elem_id: Int32
):
    """Append a dynamic none attribute (removal) to the VNode."""
    var s = _get_vnode_store(store_ptr)
    s[0].push_dynamic_attr(
        UInt32(vnode_index),
        DynamicAttr(name, AttributeValue.none(), UInt32(elem_id)),
    )


@export
fn vnode_push_fragment_child(
    store_ptr: Int64, vnode_index: Int32, child_index: Int32
):
    """Append a child VNode index to the Fragment at vnode_index."""
    var s = _get_vnode_store(store_ptr)
    s[0].push_fragment_child(UInt32(vnode_index), UInt32(child_index))


@export
fn vnode_get_dynamic_node_kind(
    store_ptr: Int64, vnode_index: Int32, dyn_index: Int32
) -> Int32:
    """Return the kind of the dynamic node at dyn_index."""
    var s = _get_vnode_store(store_ptr)
    return Int32(
        s[0].get_dynamic_node_kind(UInt32(vnode_index), Int(dyn_index))
    )


@export
fn vnode_get_dynamic_attr_kind(
    store_ptr: Int64, vnode_index: Int32, attr_index: Int32
) -> Int32:
    """Return the attribute value kind of the dynamic attr at attr_index."""
    var s = _get_vnode_store(store_ptr)
    return Int32(
        s[0].get_dynamic_attr_kind(UInt32(vnode_index), Int(attr_index))
    )


@export
fn vnode_get_dynamic_attr_element_id(
    store_ptr: Int64, vnode_index: Int32, attr_index: Int32
) -> Int32:
    """Return the element_id of the dynamic attr at attr_index."""
    var s = _get_vnode_store(store_ptr)
    return Int32(
        s[0].get_dynamic_attr_element_id(UInt32(vnode_index), Int(attr_index))
    )


@export
fn vnode_store_clear(store_ptr: Int64):
    """Clear all VNodes from the store."""
    var s = _get_vnode_store(store_ptr)
    s[0].clear()


# ── Signal arithmetic helpers (test += style operations) ─────────────────────


@export
fn signal_iadd_i32(rt_ptr: Int64, key: Int32, rhs: Int32):
    """Increment a signal: signal += rhs."""
    var rt = _get_runtime(rt_ptr)
    var current = rt[0].peek_signal[Int32](UInt32(key))
    rt[0].write_signal[Int32](UInt32(key), current + rhs)


@export
fn signal_isub_i32(rt_ptr: Int64, key: Int32, rhs: Int32):
    """Decrement a signal: signal -= rhs."""
    var rt = _get_runtime(rt_ptr)
    var current = rt[0].peek_signal[Int32](UInt32(key))
    rt[0].write_signal[Int32](UInt32(key), current - rhs)


# ── Phase 4: Create & Diff Engine Exports ────────────────────────────────────
#
# These exports exercise the CreateEngine and DiffEngine for Phase 4 testing.
# The test harness creates templates and VNodes via existing exports, then
# calls these functions to emit mutations into a buffer.  The JS side reads
# the buffer via MutationReader and verifies the mutation sequence.
#
# Workflow:
#   1. Create runtime, register templates, build VNodes in a VNodeStore
#   2. Allocate a mutation buffer and an ElementIdAllocator
#   3. Call create_vnode → emits create mutations, populates mount state
#   4. Call diff_vnodes → emits diff mutations, transfers mount state
#   5. Read mutations from buffer on JS side
#   6. Clean up


@always_inline
fn _int_to_eid_alloc_ptr(addr: Int) -> UnsafePointer[ElementIdAllocator]:
    """Reinterpret an integer address as an UnsafePointer[ElementIdAllocator].
    """
    var slot = UnsafePointer[Int].alloc(1)
    slot[0] = addr
    var result = slot.bitcast[UnsafePointer[ElementIdAllocator]]()[0]
    slot.free()
    return result


@always_inline
fn _int_to_writer_ptr(addr: Int) -> UnsafePointer[MutationWriter]:
    """Reinterpret an integer address as an UnsafePointer[MutationWriter]."""
    var slot = UnsafePointer[Int].alloc(1)
    slot[0] = addr
    var result = slot.bitcast[UnsafePointer[MutationWriter]]()[0]
    slot.free()
    return result


@export
fn writer_create(buf_ptr: Int64, capacity: Int32) -> Int64:
    """Create a heap-allocated MutationWriter.  Returns its pointer.

    The writer writes to the buffer at buf_ptr with the given capacity.
    """
    var ptr = UnsafePointer[MutationWriter].alloc(1)
    ptr.init_pointee_move(
        MutationWriter(_int_to_ptr(Int(buf_ptr)), Int(capacity))
    )
    return Int64(Int(ptr))


@export
fn writer_destroy(writer_ptr: Int64):
    """Destroy and free a heap-allocated MutationWriter."""
    var ptr = _int_to_writer_ptr(Int(writer_ptr))
    ptr.destroy_pointee()
    ptr.free()


@export
fn writer_offset(writer_ptr: Int64) -> Int32:
    """Return the current write offset of the MutationWriter."""
    var ptr = _int_to_writer_ptr(Int(writer_ptr))
    return Int32(ptr[0].offset)


@export
fn writer_finalize(writer_ptr: Int64) -> Int32:
    """Write the End sentinel and return the final offset."""
    var ptr = _int_to_writer_ptr(Int(writer_ptr))
    ptr[0].finalize()
    return Int32(ptr[0].offset)


@export
fn create_vnode(
    writer_ptr: Int64,
    eid_ptr: Int64,
    rt_ptr: Int64,
    store_ptr: Int64,
    vnode_index: Int32,
) -> Int32:
    """Create mutations for the VNode at vnode_index.

    Emits mutations to the writer and populates the VNode's mount state.
    Returns the number of root elements placed on the stack.

    Args:
        writer_ptr: Pointer to a heap-allocated MutationWriter.
        eid_ptr: Pointer to a heap-allocated ElementIdAllocator.
        rt_ptr: Pointer to a Runtime (for template registry access).
        store_ptr: Pointer to a VNodeStore containing the VNode.
        vnode_index: Index of the VNode in the store.
    """
    var w = _int_to_writer_ptr(Int(writer_ptr))
    var e = _int_to_eid_alloc_ptr(Int(eid_ptr))
    var rt = _int_to_runtime_ptr(Int(rt_ptr))
    var s = _int_to_vnode_store_ptr(Int(store_ptr))

    var engine = CreateEngine(w, e, rt, s)
    return Int32(engine.create_node(UInt32(vnode_index)))


@export
fn diff_vnodes(
    writer_ptr: Int64,
    eid_ptr: Int64,
    rt_ptr: Int64,
    store_ptr: Int64,
    old_index: Int32,
    new_index: Int32,
) -> Int32:
    """Diff old and new VNodes and emit mutations.

    The old VNode must have mount state populated (from a previous create
    or diff).  The new VNode's mount state will be populated as a side effect.
    Returns the writer offset (bytes written) after diffing.

    Args:
        writer_ptr: Pointer to a heap-allocated MutationWriter.
        eid_ptr: Pointer to a heap-allocated ElementIdAllocator.
        rt_ptr: Pointer to a Runtime (for template registry access).
        store_ptr: Pointer to a VNodeStore containing both VNodes.
        old_index: Index of the old VNode in the store.
        new_index: Index of the new VNode in the store.
    """
    var w = _int_to_writer_ptr(Int(writer_ptr))
    var e = _int_to_eid_alloc_ptr(Int(eid_ptr))
    var rt = _int_to_runtime_ptr(Int(rt_ptr))
    var s = _int_to_vnode_store_ptr(Int(store_ptr))

    var engine = DiffEngine(w, e, rt, s)
    engine.diff_node(UInt32(old_index), UInt32(new_index))
    return Int32(w[0].offset)


# ── VNode mount state query exports ──────────────────────────────────────────


@export
fn vnode_root_id_count(store_ptr: Int64, index: Int32) -> Int32:
    """Return the number of root ElementIds assigned to this VNode."""
    var s = _get_vnode_store(store_ptr)
    return Int32(s[0].get_ptr(UInt32(index))[0].root_id_count())


@export
fn vnode_get_root_id(store_ptr: Int64, index: Int32, pos: Int32) -> Int32:
    """Return the root ElementId at position `pos`."""
    var s = _get_vnode_store(store_ptr)
    return Int32(s[0].get_ptr(UInt32(index))[0].get_root_id(Int(pos)))


@export
fn vnode_dyn_node_id_count(store_ptr: Int64, index: Int32) -> Int32:
    """Return the number of dynamic node ElementIds."""
    var s = _get_vnode_store(store_ptr)
    return Int32(s[0].get_ptr(UInt32(index))[0].dyn_node_id_count())


@export
fn vnode_get_dyn_node_id(store_ptr: Int64, index: Int32, pos: Int32) -> Int32:
    """Return the dynamic node ElementId at position `pos`."""
    var s = _get_vnode_store(store_ptr)
    return Int32(s[0].get_ptr(UInt32(index))[0].get_dyn_node_id(Int(pos)))


@export
fn vnode_dyn_attr_id_count(store_ptr: Int64, index: Int32) -> Int32:
    """Return the number of dynamic attribute target ElementIds."""
    var s = _get_vnode_store(store_ptr)
    return Int32(s[0].get_ptr(UInt32(index))[0].dyn_attr_id_count())


@export
fn vnode_get_dyn_attr_id(store_ptr: Int64, index: Int32, pos: Int32) -> Int32:
    """Return the dynamic attribute target ElementId at position `pos`."""
    var s = _get_vnode_store(store_ptr)
    return Int32(s[0].get_ptr(UInt32(index))[0].get_dyn_attr_id(Int(pos)))


@export
fn vnode_is_mounted(store_ptr: Int64, index: Int32) -> Int32:
    """Check whether the VNode has been mounted.  Returns 1 or 0."""
    var s = _get_vnode_store(store_ptr)
    if s[0].get_ptr(UInt32(index))[0].is_mounted():
        return 1
    return 0


# ── Mutation Protocol Test Exports ───────────────────────────────────────────
#
# These functions allow the JS test harness to exercise the MutationWriter.
# Each function writes one mutation to a caller-provided buffer and returns
# the new offset (number of bytes written so far).
#
# Buffer lifecycle:
#   1. JS allocates memory via mutation_buf_alloc(capacity) → pointer
#   2. JS calls write_op_* functions, threading the offset through
#   3. JS reads the raw bytes back from WASM memory via the pointer
#   4. JS frees the buffer via mutation_buf_free(ptr)


@export
fn mutation_buf_alloc(capacity: Int32) -> Int64:
    """Allocate a mutation buffer. Returns a pointer into WASM linear memory."""
    var ptr = UnsafePointer[UInt8].alloc(Int(capacity))
    return _ptr_to_i64(ptr)


@export
fn mutation_buf_free(ptr: Int64):
    """Free a previously allocated mutation buffer."""
    _int_to_ptr(Int(ptr)).free()


# ── Helper: create a MutationWriter at (buf, off) ───────────────────────────


@always_inline
fn _writer(buf: Int64, off: Int32) -> MutationWriter:
    return MutationWriter(_int_to_ptr(Int(buf)), Int(off), 0)


# ── Debug exports ────────────────────────────────────────────────────────────


@export
fn debug_ptr_roundtrip(ptr: Int64) -> Int64:
    """Check that _int_to_ptr round-trips correctly.  Returns the address."""
    var p = _int_to_ptr(Int(ptr))
    return Int64(Int(p))


@export
fn debug_write_byte(ptr: Int64, off: Int32, val: Int32) -> Int32:
    """Write a single byte to ptr+off and return off+1."""
    var p = _int_to_ptr(Int(ptr))
    p[Int(off)] = UInt8(val)
    return off + 1


@export
fn debug_read_byte(ptr: Int64, off: Int32) -> Int32:
    """Read a single byte from ptr+off."""
    var p = _int_to_ptr(Int(ptr))
    return Int32(p[Int(off)])


# ── Simple opcodes (no string/path payload) ──────────────────────────────────


@export
fn write_op_end(buf: Int64, off: Int32) -> Int32:
    var w = _writer(buf, off)
    w.end()
    return Int32(w.offset)


@export
fn write_op_append_children(
    buf: Int64, off: Int32, id: Int32, m: Int32
) -> Int32:
    var w = _writer(buf, off)
    w.append_children(UInt32(id), UInt32(m))
    return Int32(w.offset)


@export
fn write_op_create_placeholder(buf: Int64, off: Int32, id: Int32) -> Int32:
    var w = _writer(buf, off)
    w.create_placeholder(UInt32(id))
    return Int32(w.offset)


@export
fn write_op_load_template(
    buf: Int64, off: Int32, tmpl_id: Int32, index: Int32, id: Int32
) -> Int32:
    var w = _writer(buf, off)
    w.load_template(UInt32(tmpl_id), UInt32(index), UInt32(id))
    return Int32(w.offset)


@export
fn write_op_replace_with(buf: Int64, off: Int32, id: Int32, m: Int32) -> Int32:
    var w = _writer(buf, off)
    w.replace_with(UInt32(id), UInt32(m))
    return Int32(w.offset)


@export
fn write_op_insert_after(buf: Int64, off: Int32, id: Int32, m: Int32) -> Int32:
    var w = _writer(buf, off)
    w.insert_after(UInt32(id), UInt32(m))
    return Int32(w.offset)


@export
fn write_op_insert_before(buf: Int64, off: Int32, id: Int32, m: Int32) -> Int32:
    var w = _writer(buf, off)
    w.insert_before(UInt32(id), UInt32(m))
    return Int32(w.offset)


@export
fn write_op_remove(buf: Int64, off: Int32, id: Int32) -> Int32:
    var w = _writer(buf, off)
    w.remove(UInt32(id))
    return Int32(w.offset)


@export
fn write_op_push_root(buf: Int64, off: Int32, id: Int32) -> Int32:
    var w = _writer(buf, off)
    w.push_root(UInt32(id))
    return Int32(w.offset)


# ── String-carrying opcodes ──────────────────────────────────────────────────


@export
fn write_op_create_text_node(
    buf: Int64, off: Int32, id: Int32, text: String
) -> Int32:
    var w = _writer(buf, off)
    w.create_text_node(UInt32(id), text)
    return Int32(w.offset)


@export
fn write_op_set_text(buf: Int64, off: Int32, id: Int32, text: String) -> Int32:
    var w = _writer(buf, off)
    w.set_text(UInt32(id), text)
    return Int32(w.offset)


@export
fn write_op_set_attribute(
    buf: Int64, off: Int32, id: Int32, ns: Int32, name: String, value: String
) -> Int32:
    var w = _writer(buf, off)
    w.set_attribute(UInt32(id), UInt8(ns), name, value)
    return Int32(w.offset)


@export
fn write_op_new_event_listener(
    buf: Int64, off: Int32, id: Int32, name: String
) -> Int32:
    var w = _writer(buf, off)
    w.new_event_listener(UInt32(id), name)
    return Int32(w.offset)


@export
fn write_op_remove_event_listener(
    buf: Int64, off: Int32, id: Int32, name: String
) -> Int32:
    var w = _writer(buf, off)
    w.remove_event_listener(UInt32(id), name)
    return Int32(w.offset)


# ── Path-carrying opcodes ────────────────────────────────────────────────────


@export
fn write_op_assign_id(
    buf: Int64, off: Int32, path_ptr: Int64, path_len: Int32, id: Int32
) -> Int32:
    var w = _writer(buf, off)
    w.assign_id(_int_to_ptr(Int(path_ptr)), Int(path_len), UInt32(id))
    return Int32(w.offset)


@export
fn write_op_replace_placeholder(
    buf: Int64, off: Int32, path_ptr: Int64, path_len: Int32, m: Int32
) -> Int32:
    var w = _writer(buf, off)
    w.replace_placeholder(_int_to_ptr(Int(path_ptr)), Int(path_len), UInt32(m))
    return Int32(w.offset)


# ── Composite test helper ────────────────────────────────────────────────────


@export
fn write_test_sequence(buf: Int64) -> Int32:
    """Write a known 5-mutation sequence for integration testing.

    Sequence:
      1. LoadTemplate(tmpl_id=1, index=0, id=10)
      2. CreateTextNode(id=11, text="hello")
      3. AppendChildren(id=10, m=1)
      4. PushRoot(id=10)
      5. End
    """
    var w = MutationWriter(_int_to_ptr(Int(buf)), 0)
    w.load_template(1, 0, 10)
    w.create_text_node(11, String("hello"))
    w.append_children(10, 1)
    w.push_root(10)
    w.end()
    return Int32(w.offset)


# ── Phase 6: Event Handler Registry Exports ──────────────────────────────────
#
# These exports exercise the event handler registry and event dispatch.
# Handlers map handler IDs → (scope_id, action, signal_key, operand).
# When an event fires, JS calls dispatch_event which executes the action
# (e.g. signal write) and marks scopes dirty.


@export
fn handler_register_signal_add(
    rt_ptr: Int64,
    scope_id: Int32,
    signal_key: Int32,
    delta: Int32,
    event_name: String,
) -> Int32:
    """Register a handler that adds `delta` to `signal_key` on event.

    Returns the handler ID.
    """
    var rt = _get_runtime(rt_ptr)
    return Int32(
        rt[0].register_handler(
            HandlerEntry.signal_add(
                UInt32(scope_id), UInt32(signal_key), delta, event_name
            )
        )
    )


@export
fn handler_register_signal_sub(
    rt_ptr: Int64,
    scope_id: Int32,
    signal_key: Int32,
    delta: Int32,
    event_name: String,
) -> Int32:
    """Register a handler that subtracts `delta` from `signal_key` on event.

    Returns the handler ID.
    """
    var rt = _get_runtime(rt_ptr)
    return Int32(
        rt[0].register_handler(
            HandlerEntry.signal_sub(
                UInt32(scope_id), UInt32(signal_key), delta, event_name
            )
        )
    )


@export
fn handler_register_signal_set(
    rt_ptr: Int64,
    scope_id: Int32,
    signal_key: Int32,
    value: Int32,
    event_name: String,
) -> Int32:
    """Register a handler that sets `signal_key` to `value` on event.

    Returns the handler ID.
    """
    var rt = _get_runtime(rt_ptr)
    return Int32(
        rt[0].register_handler(
            HandlerEntry.signal_set(
                UInt32(scope_id), UInt32(signal_key), value, event_name
            )
        )
    )


@export
fn handler_register_signal_toggle(
    rt_ptr: Int64,
    scope_id: Int32,
    signal_key: Int32,
    event_name: String,
) -> Int32:
    """Register a handler that toggles `signal_key` (0↔1) on event.

    Returns the handler ID.
    """
    var rt = _get_runtime(rt_ptr)
    return Int32(
        rt[0].register_handler(
            HandlerEntry.signal_toggle(
                UInt32(scope_id), UInt32(signal_key), event_name
            )
        )
    )


@export
fn handler_register_signal_set_input(
    rt_ptr: Int64,
    scope_id: Int32,
    signal_key: Int32,
    event_name: String,
) -> Int32:
    """Register a handler that sets `signal_key` from event input value.

    Returns the handler ID.
    """
    var rt = _get_runtime(rt_ptr)
    return Int32(
        rt[0].register_handler(
            HandlerEntry.signal_set_input(
                UInt32(scope_id), UInt32(signal_key), event_name
            )
        )
    )


@export
fn handler_register_custom(
    rt_ptr: Int64, scope_id: Int32, event_name: String
) -> Int32:
    """Register a custom handler (JS handles the side effect).

    Returns the handler ID.
    """
    var rt = _get_runtime(rt_ptr)
    return Int32(
        rt[0].register_handler(
            HandlerEntry.custom(UInt32(scope_id), event_name)
        )
    )


@export
fn handler_register_noop(
    rt_ptr: Int64, scope_id: Int32, event_name: String
) -> Int32:
    """Register a no-op handler (marks scope dirty, does nothing else).

    Returns the handler ID.
    """
    var rt = _get_runtime(rt_ptr)
    return Int32(
        rt[0].register_handler(HandlerEntry.noop(UInt32(scope_id), event_name))
    )


@export
fn handler_remove(rt_ptr: Int64, handler_id: Int32):
    """Remove an event handler by ID."""
    var rt = _get_runtime(rt_ptr)
    rt[0].remove_handler(UInt32(handler_id))


@export
fn handler_count(rt_ptr: Int64) -> Int32:
    """Return the number of live event handlers."""
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].handler_count())


@export
fn handler_contains(rt_ptr: Int64, handler_id: Int32) -> Int32:
    """Check whether a handler ID is live.  Returns 1 or 0."""
    var rt = _get_runtime(rt_ptr)
    if rt[0].handlers.contains(UInt32(handler_id)):
        return 1
    return 0


@export
fn handler_scope_id(rt_ptr: Int64, handler_id: Int32) -> Int32:
    """Return the scope_id of the handler."""
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].handlers.scope_id(UInt32(handler_id)))


@export
fn handler_action(rt_ptr: Int64, handler_id: Int32) -> Int32:
    """Return the action tag of the handler."""
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].handlers.action(UInt32(handler_id)))


@export
fn handler_signal_key(rt_ptr: Int64, handler_id: Int32) -> Int32:
    """Return the signal_key of the handler."""
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].handlers.signal_key(UInt32(handler_id)))


@export
fn handler_operand(rt_ptr: Int64, handler_id: Int32) -> Int32:
    """Return the operand of the handler."""
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].handlers.operand(UInt32(handler_id)))


@export
fn dispatch_event(rt_ptr: Int64, handler_id: Int32, event_type: Int32) -> Int32:
    """Dispatch an event to a handler.

    Executes the handler's action (e.g. signal write) and marks
    affected scopes dirty.  Returns 1 if an action was executed, 0 otherwise.
    """
    var rt = _get_runtime(rt_ptr)
    if rt[0].dispatch_event(UInt32(handler_id), UInt8(event_type)):
        return 1
    return 0


@export
fn dispatch_event_with_i32(
    rt_ptr: Int64, handler_id: Int32, event_type: Int32, value: Int32
) -> Int32:
    """Dispatch an event with an Int32 payload (e.g. parsed input value).

    For ACTION_SIGNAL_SET_INPUT, the payload is used as the new signal value.
    Returns 1 if an action was executed, 0 otherwise.
    """
    var rt = _get_runtime(rt_ptr)
    if rt[0].dispatch_event_with_i32(
        UInt32(handler_id), UInt8(event_type), value
    ):
        return 1
    return 0


@export
fn runtime_drain_dirty(rt_ptr: Int64) -> Int32:
    """Drain the dirty scope queue.  Returns the number of dirty scopes.

    After calling, the dirty queue is empty.  The caller should re-render
    each returned scope ID.  (For now, just returns the count — the actual
    scope IDs are consumed internally.)
    """
    var rt = _get_runtime(rt_ptr)
    var dirty = rt[0].drain_dirty()
    return Int32(len(dirty))


# ── Original wasm-mojo PoC exports ──────────────────────────────────────────


# Add
@export
fn add_int32(x: Int32, y: Int32) -> Int32:
    return x + y


@export
fn add_int64(x: Int64, y: Int64) -> Int64:
    return x + y


@export
fn add_float32(x: Float32, y: Float32) -> Float32:
    return x + y


@export
fn add_float64(x: Float64, y: Float64) -> Float64:
    return x + y


# Subtract
@export
fn sub_int32(x: Int32, y: Int32) -> Int32:
    return x - y


@export
fn sub_int64(x: Int64, y: Int64) -> Int64:
    return x - y


@export
fn sub_float32(x: Float32, y: Float32) -> Float32:
    return x - y


@export
fn sub_float64(x: Float64, y: Float64) -> Float64:
    return x - y


# Multiply
@export
fn mul_int32(x: Int32, y: Int32) -> Int32:
    return x * y


@export
fn mul_int64(x: Int64, y: Int64) -> Int64:
    return x * y


@export
fn mul_float32(x: Float32, y: Float32) -> Float32:
    return x * y


@export
fn mul_float64(x: Float64, y: Float64) -> Float64:
    return x * y


# Division
@export
fn div_int32(x: Int32, y: Int32) -> Int32:
    return x // y


@export
fn div_int64(x: Int64, y: Int64) -> Int64:
    return x // y


@export
fn div_float32(x: Float32, y: Float32) -> Float32:
    return x / y


@export
fn div_float64(x: Float64, y: Float64) -> Float64:
    return x / y


# Modulo
@export
fn mod_int32(x: Int32, y: Int32) -> Int32:
    return x % y


@export
fn mod_int64(x: Int64, y: Int64) -> Int64:
    return x % y


# Power
@export
fn pow_int32(x: Int32) -> Int32:
    return x**x


@export
fn pow_int64(x: Int64) -> Int64:
    return x**x


@export
fn pow_float32(x: Float32) -> Float32:
    return x**x


@export
fn pow_float64(x: Float64) -> Float64:
    return x**x


# Negate
@export
fn neg_int32(x: Int32) -> Int32:
    return -x


@export
fn neg_int64(x: Int64) -> Int64:
    return -x


@export
fn neg_float32(x: Float32) -> Float32:
    return -x


@export
fn neg_float64(x: Float64) -> Float64:
    return -x


# Absolute value
@export
fn abs_int32(x: Int32) -> Int32:
    if x < 0:
        return -x
    return x


@export
fn abs_int64(x: Int64) -> Int64:
    if x < 0:
        return -x
    return x


@export
fn abs_float32(x: Float32) -> Float32:
    if x < 0:
        return -x
    return x


@export
fn abs_float64(x: Float64) -> Float64:
    if x < 0:
        return -x
    return x


# Min / Max
@export
fn min_int32(x: Int32, y: Int32) -> Int32:
    if x < y:
        return x
    return y


@export
fn max_int32(x: Int32, y: Int32) -> Int32:
    if x > y:
        return x
    return y


@export
fn min_int64(x: Int64, y: Int64) -> Int64:
    if x < y:
        return x
    return y


@export
fn max_int64(x: Int64, y: Int64) -> Int64:
    if x > y:
        return x
    return y


@export
fn min_float64(x: Float64, y: Float64) -> Float64:
    if x < y:
        return x
    return y


@export
fn max_float64(x: Float64, y: Float64) -> Float64:
    if x > y:
        return x
    return y


# Clamp
@export
fn clamp_int32(x: Int32, lo: Int32, hi: Int32) -> Int32:
    if x < lo:
        return lo
    if x > hi:
        return hi
    return x


@export
fn clamp_float64(x: Float64, lo: Float64, hi: Float64) -> Float64:
    if x < lo:
        return lo
    if x > hi:
        return hi
    return x


# Bitwise operations
@export
fn bitand_int32(x: Int32, y: Int32) -> Int32:
    return x & y


@export
fn bitor_int32(x: Int32, y: Int32) -> Int32:
    return x | y


@export
fn bitxor_int32(x: Int32, y: Int32) -> Int32:
    return x ^ y


@export
fn bitnot_int32(x: Int32) -> Int32:
    return ~x


@export
fn shl_int32(x: Int32, y: Int32) -> Int32:
    return x << y


@export
fn shr_int32(x: Int32, y: Int32) -> Int32:
    return x >> y


# Boolean / comparison
@export
fn eq_int32(x: Int32, y: Int32) -> Bool:
    return x == y


@export
fn ne_int32(x: Int32, y: Int32) -> Bool:
    return x != y


@export
fn lt_int32(x: Int32, y: Int32) -> Bool:
    return x < y


@export
fn le_int32(x: Int32, y: Int32) -> Bool:
    return x <= y


@export
fn gt_int32(x: Int32, y: Int32) -> Bool:
    return x > y


@export
fn ge_int32(x: Int32, y: Int32) -> Bool:
    return x >= y


@export
fn bool_and(x: Bool, y: Bool) -> Bool:
    return x and y


@export
fn bool_or(x: Bool, y: Bool) -> Bool:
    return x or y


@export
fn bool_not(x: Bool) -> Bool:
    return not x


# Fibonacci (iterative)
@export
fn fib_int32(n: Int32) -> Int32:
    if n <= 0:
        return 0
    if n == 1:
        return 1
    var a: Int32 = 0
    var b: Int32 = 1
    for _ in range(2, Int(n) + 1):
        var tmp = a + b
        a = b
        b = tmp
    return b


@export
fn fib_int64(n: Int64) -> Int64:
    if n <= 0:
        return 0
    if n == 1:
        return 1
    var a: Int64 = 0
    var b: Int64 = 1
    for _ in range(2, Int(n) + 1):
        var tmp = a + b
        a = b
        b = tmp
    return b


# ══════════════════════════════════════════════════════════════════════════════
# Phase 8.3 — Context (Dependency Injection) Exports
# ══════════════════════════════════════════════════════════════════════════════
#
# Context allows parent scopes to provide key→value pairs that any descendant
# scope can consume without prop drilling.  Lookups walk up the parent chain.
# Keys are UInt32 identifiers; values are Int32 (sufficient for signal keys,
# enum values, flags, etc.).


@export
fn ctx_provide(rt_ptr: Int64, scope_id: Int32, key: Int32, value: Int32):
    """Provide a context value at the given scope.

    If the key already exists, the value is updated.
    """
    var rt = _get_runtime(rt_ptr)
    rt[0].scopes.provide_context(UInt32(scope_id), UInt32(key), value)


@export
fn ctx_consume(rt_ptr: Int64, scope_id: Int32, key: Int32) -> Int32:
    """Look up a context value by walking up the scope tree.

    Returns the value if found, or 0 if not found.
    Use ctx_consume_found() to distinguish "not found" from "value is 0".
    """
    var rt = _get_runtime(rt_ptr)
    var result = rt[0].scopes.consume_context(UInt32(scope_id), UInt32(key))
    return result[1]


@export
fn ctx_consume_found(rt_ptr: Int64, scope_id: Int32, key: Int32) -> Int32:
    """Check whether a context value exists for `key` in the scope's ancestry.

    Returns 1 if found, 0 if not.
    """
    var rt = _get_runtime(rt_ptr)
    var result = rt[0].scopes.consume_context(UInt32(scope_id), UInt32(key))
    if result[0]:
        return 1
    return 0


@export
fn ctx_has_local(rt_ptr: Int64, scope_id: Int32, key: Int32) -> Int32:
    """Check whether the scope itself provides a context for `key`.

    Does NOT walk up the parent chain.  Returns 1 or 0.
    """
    var rt = _get_runtime(rt_ptr)
    if rt[0].scopes.has_context_local(UInt32(scope_id), UInt32(key)):
        return 1
    return 0


@export
fn ctx_count(rt_ptr: Int64, scope_id: Int32) -> Int32:
    """Return the number of context entries provided by this scope."""
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].scopes.context_count(UInt32(scope_id)))


@export
fn ctx_remove(rt_ptr: Int64, scope_id: Int32, key: Int32) -> Int32:
    """Remove a context entry from the scope.  Returns 1 if removed, 0 if not found.
    """
    var rt = _get_runtime(rt_ptr)
    if rt[0].scopes.remove_context(UInt32(scope_id), UInt32(key)):
        return 1
    return 0


# ══════════════════════════════════════════════════════════════════════════════
# Phase 8.4 — Error Boundaries Exports
# ══════════════════════════════════════════════════════════════════════════════
#
# A scope marked as an error boundary catches errors from descendant scopes.
# When a child reports an error, the nearest ancestor boundary captures it
# and can render a fallback UI.  Clearing the error allows recovery.


@export
fn err_set_boundary(rt_ptr: Int64, scope_id: Int32, enabled: Int32):
    """Mark or unmark a scope as an error boundary.

    enabled=1 marks as boundary, enabled=0 unmarks.
    """
    var rt = _get_runtime(rt_ptr)
    rt[0].scopes.set_error_boundary(UInt32(scope_id), enabled != 0)


@export
fn err_is_boundary(rt_ptr: Int64, scope_id: Int32) -> Int32:
    """Check whether the scope is an error boundary.  Returns 1 or 0."""
    var rt = _get_runtime(rt_ptr)
    if rt[0].scopes.is_error_boundary(UInt32(scope_id)):
        return 1
    return 0


@export
fn err_set_error(rt_ptr: Int64, scope_id: Int32, message: String):
    """Set an error directly on the scope."""
    var rt = _get_runtime(rt_ptr)
    rt[0].scopes.set_error(UInt32(scope_id), message)


@export
fn err_clear(rt_ptr: Int64, scope_id: Int32):
    """Clear the error state on the scope."""
    var rt = _get_runtime(rt_ptr)
    rt[0].scopes.clear_error(UInt32(scope_id))


@export
fn err_has_error(rt_ptr: Int64, scope_id: Int32) -> Int32:
    """Check whether the scope has a captured error.  Returns 1 or 0."""
    var rt = _get_runtime(rt_ptr)
    if rt[0].scopes.has_error(UInt32(scope_id)):
        return 1
    return 0


@export
fn err_find_boundary(rt_ptr: Int64, scope_id: Int32) -> Int32:
    """Walk up from `scope_id` to find the nearest error boundary ancestor.

    Returns the boundary scope ID, or -1 if none found.
    """
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].scopes.find_error_boundary(UInt32(scope_id)))


@export
fn err_propagate(rt_ptr: Int64, scope_id: Int32, message: String) -> Int32:
    """Propagate an error from `scope_id` to its nearest error boundary.

    Sets the error on the boundary and returns its scope ID.
    Returns -1 if no boundary found (error is unhandled).
    """
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].scopes.propagate_error(UInt32(scope_id), message))


# ══════════════════════════════════════════════════════════════════════════════
# Phase 8.5 — Suspense Exports
# ══════════════════════════════════════════════════════════════════════════════
#
# A scope marked as a suspense boundary shows a fallback while any descendant
# scope is in a "pending" state (waiting for async data).  When the pending
# scope resolves, the boundary re-renders with actual content.


@export
fn suspense_set_boundary(rt_ptr: Int64, scope_id: Int32, enabled: Int32):
    """Mark or unmark a scope as a suspense boundary.

    enabled=1 marks as boundary, enabled=0 unmarks.
    """
    var rt = _get_runtime(rt_ptr)
    rt[0].scopes.set_suspense_boundary(UInt32(scope_id), enabled != 0)


@export
fn suspense_is_boundary(rt_ptr: Int64, scope_id: Int32) -> Int32:
    """Check whether the scope is a suspense boundary.  Returns 1 or 0."""
    var rt = _get_runtime(rt_ptr)
    if rt[0].scopes.is_suspense_boundary(UInt32(scope_id)):
        return 1
    return 0


@export
fn suspense_set_pending(rt_ptr: Int64, scope_id: Int32, pending: Int32):
    """Set the pending (async loading) state on a scope.

    pending=1 marks as pending, pending=0 marks as resolved.
    """
    var rt = _get_runtime(rt_ptr)
    rt[0].scopes.set_pending(UInt32(scope_id), pending != 0)


@export
fn suspense_is_pending(rt_ptr: Int64, scope_id: Int32) -> Int32:
    """Check whether the scope is in a pending state.  Returns 1 or 0."""
    var rt = _get_runtime(rt_ptr)
    if rt[0].scopes.is_pending(UInt32(scope_id)):
        return 1
    return 0


@export
fn suspense_find_boundary(rt_ptr: Int64, scope_id: Int32) -> Int32:
    """Walk up from `scope_id` to find the nearest suspense boundary ancestor.

    Returns the boundary scope ID, or -1 if none found.
    """
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].scopes.find_suspense_boundary(UInt32(scope_id)))


@export
fn suspense_has_pending(rt_ptr: Int64, scope_id: Int32) -> Int32:
    """Check if any descendant of `scope_id` is pending.  Returns 1 or 0."""
    var rt = _get_runtime(rt_ptr)
    if rt[0].scopes.has_pending_descendant(UInt32(scope_id)):
        return 1
    return 0


@export
fn suspense_resolve(rt_ptr: Int64, scope_id: Int32) -> Int32:
    """Mark a scope as no longer pending and return its suspense boundary.

    Clears the pending flag.  Returns the nearest suspense boundary
    scope ID, or -1 if none.
    """
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].scopes.resolve_pending(UInt32(scope_id)))


# ══════════════════════════════════════════════════════════════════════════════
# Phase 7 — Counter App (End-to-End)
# ══════════════════════════════════════════════════════════════════════════════
#
# Self-contained counter application that orchestrates all subsystems:
#   Runtime (signals, scopes, handlers) + Templates + VNodes + Create/Diff
#
# The counter app state is heap-allocated and accessed via an Int64 pointer.
# JS calls the exported functions to init, mount, handle events, and flush.
#
# Template structure:
#   div
#     span
#       dynamic_text[0]      ← "Count: N"
#     button  (text: "+")
#       dynamic_attr[0]      ← onclick → increment handler
#     button  (text: "−")
#       dynamic_attr[1]      ← onclick → decrement handler


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


fn _int_to_counter_ptr(addr: Int) -> UnsafePointer[CounterApp]:
    """Reinterpret an integer address as an UnsafePointer[CounterApp]."""
    var slot = UnsafePointer[Int].alloc(1)
    slot[0] = addr
    var result = slot.bitcast[UnsafePointer[CounterApp]]()[0]
    slot.free()
    return result


# ── Counter App Lifecycle Exports ────────────────────────────────────────────


@export
fn counter_init() -> Int64:
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

    return Int64(Int(app_ptr))


@export
fn counter_destroy(app_ptr: Int64):
    """Destroy the counter app and free all resources."""
    var ptr = _int_to_counter_ptr(Int(app_ptr))

    # Destroy subsystems
    if ptr[0].store:
        ptr[0].store.destroy_pointee()
        ptr[0].store.free()
    if ptr[0].eid_alloc:
        ptr[0].eid_alloc.destroy_pointee()
        ptr[0].eid_alloc.free()
    if ptr[0].runtime:
        destroy_runtime(ptr[0].runtime)

    ptr.destroy_pointee()
    ptr.free()


@export
fn counter_rebuild(app_ptr: Int64, buf_ptr: Int64, capacity: Int32) -> Int32:
    """Initial render (mount) of the counter app.

    Builds the VNode tree, runs CreateEngine, emits AppendChildren to
    mount to root (id 0), and finalizes the mutation buffer.

    Returns the byte offset (length) of the mutation data written.
    """
    var app = _int_to_counter_ptr(Int(app_ptr))
    var buf = _int_to_ptr(Int(buf_ptr))

    # Build a MutationWriter
    var writer_ptr = UnsafePointer[MutationWriter].alloc(1)
    writer_ptr.init_pointee_move(MutationWriter(buf, Int(capacity)))

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
    var offset = Int32(writer_ptr[0].offset)

    writer_ptr.destroy_pointee()
    writer_ptr.free()

    return offset


@export
fn counter_handle_event(
    app_ptr: Int64, handler_id: Int32, event_type: Int32
) -> Int32:
    """Dispatch an event to the counter app.

    Returns 1 if an action was executed, 0 otherwise.
    """
    var app = _int_to_counter_ptr(Int(app_ptr))
    if app[0].runtime[0].dispatch_event(UInt32(handler_id), UInt8(event_type)):
        return 1
    return 0


@export
fn counter_flush(app_ptr: Int64, buf_ptr: Int64, capacity: Int32) -> Int32:
    """Flush pending updates after event dispatch.

    If dirty scopes exist, re-renders the counter component, diffs the
    old and new VNode trees, and writes mutations to the buffer.

    Returns the byte offset (length) of the mutation data written,
    or 0 if there was nothing to update.
    """
    var app = _int_to_counter_ptr(Int(app_ptr))

    # Check for dirty scopes
    if not app[0].runtime[0].has_dirty():
        return 0

    # Drain dirty scopes (we only have one scope, so just drain)
    var _dirty = app[0].runtime[0].drain_dirty()

    var buf = _int_to_ptr(Int(buf_ptr))

    # Build a MutationWriter
    var writer_ptr = UnsafePointer[MutationWriter].alloc(1)
    writer_ptr.init_pointee_move(MutationWriter(buf, Int(capacity)))

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
    var offset = Int32(writer_ptr[0].offset)

    writer_ptr.destroy_pointee()
    writer_ptr.free()

    return offset


# ── Counter App Query Exports ────────────────────────────────────────────────


@export
fn counter_rt_ptr(app_ptr: Int64) -> Int64:
    """Return the runtime pointer for JS template registration."""
    var app = _int_to_counter_ptr(Int(app_ptr))
    return _runtime_ptr_to_i64(app[0].runtime)


@export
fn counter_tmpl_id(app_ptr: Int64) -> Int32:
    """Return the counter template ID."""
    var app = _int_to_counter_ptr(Int(app_ptr))
    return Int32(app[0].template_id)


@export
fn counter_incr_handler(app_ptr: Int64) -> Int32:
    """Return the increment handler ID."""
    var app = _int_to_counter_ptr(Int(app_ptr))
    return Int32(app[0].incr_handler)


@export
fn counter_decr_handler(app_ptr: Int64) -> Int32:
    """Return the decrement handler ID."""
    var app = _int_to_counter_ptr(Int(app_ptr))
    return Int32(app[0].decr_handler)


@export
fn counter_count_value(app_ptr: Int64) -> Int32:
    """Peek the current count signal value (without subscribing)."""
    var app = _int_to_counter_ptr(Int(app_ptr))
    return app[0].runtime[0].peek_signal[Int32](app[0].count_signal)


@export
fn counter_has_dirty(app_ptr: Int64) -> Int32:
    """Check if the counter app has dirty scopes.  Returns 1 or 0."""
    var app = _int_to_counter_ptr(Int(app_ptr))
    if app[0].runtime[0].has_dirty():
        return 1
    return 0


@export
fn counter_scope_id(app_ptr: Int64) -> Int32:
    """Return the counter app's root scope ID."""
    var app = _int_to_counter_ptr(Int(app_ptr))
    return Int32(app[0].scope_id)


@export
fn counter_count_signal(app_ptr: Int64) -> Int32:
    """Return the counter app's count signal key."""
    var app = _int_to_counter_ptr(Int(app_ptr))
    return Int32(app[0].count_signal)


# ── Todo App ─────────────────────────────────────────────────────────────────
#
# Phase 8 — A todo list application demonstrating:
#   - Dynamic keyed lists (add, remove, toggle items)
#   - Conditional rendering (show/hide completed indicator)
#   - Fragment VNodes with keyed children
#   - String data flow (input text from JS → WASM)
#
# Architecture:
#   - TodoApp struct holds all state: items list, input text, signals, handlers
#   - Items are stored as a flat list of TodoItem structs (not signals)
#   - A "list_version" signal is bumped on every list mutation to trigger re-render
#   - JS calls specific exports (todo_add_item, todo_remove_item, etc.)
#     then calls todo_flush() to get mutation bytes
#
# Templates:
#   - "todo-app": The app shell with input field + item list container
#       div > [ input + button("Add") + ul > dynamic[0] ]
#   - "todo-item": A single list item
#       li > [ span > dynamic_text[0], button("✓") + button("✕") ]
#       dynamic_attr[0] = click handler for toggle
#       dynamic_attr[1] = click handler for remove
#       dynamic_attr[2] = class on the li (for completed styling)


struct TodoItem(Copyable, Movable):
    """A single todo list item."""

    var id: Int32
    var text: String
    var completed: Bool

    fn __init__(out self, id: Int32, text: String, completed: Bool):
        self.id = id
        self.text = text
        self.completed = completed

    fn __copyinit__(out self, other: Self):
        self.id = other.id
        self.text = other.text
        self.completed = other.completed

    fn __moveinit__(out self, deinit other: Self):
        self.id = other.id
        self.text = other.text^
        self.completed = other.completed


struct TodoApp(Movable):
    """Self-contained todo list application state.

    The item list lives inside the <ul> element of the app template.
    On initial mount, a placeholder comment node occupies the <ul>.
    We track that placeholder's ElementId so we can replace it with
    item nodes, and later manage item-to-item diffs via a Fragment
    VNode that mirrors the <ul>'s children.

    State tracking:
      - ul_placeholder_id: ElementId of the placeholder inside <ul>.
        Non-zero when the list is empty (placeholder is in the DOM).
        Zero when items are present (placeholder was replaced).
      - current_frag: VNode index of the current items Fragment.
        -1 before first render.
      - items_mounted: True once items have replaced the placeholder.
    """

    var runtime: UnsafePointer[Runtime]
    var store: UnsafePointer[VNodeStore]
    var eid_alloc: UnsafePointer[ElementIdAllocator]
    var scope_id: UInt32
    var list_version_signal: UInt32  # bumped on every list mutation
    var app_template_id: UInt32  # "todo-app" template
    var item_template_id: UInt32  # "todo-item" template
    var items: List[TodoItem]
    var next_id: Int32
    var input_text: String
    var current_vnode: Int  # index in store, or -1 if not yet rendered
    var current_frag: Int  # Fragment VNode index, or -1
    var ul_placeholder_id: UInt32  # ElementId of placeholder in <ul>
    var items_mounted: Bool  # True when items are in DOM (placeholder removed)
    # Handler IDs for the app-level controls
    var add_handler: UInt32

    fn __init__(out self):
        self.runtime = UnsafePointer[Runtime]()
        self.store = UnsafePointer[VNodeStore]()
        self.eid_alloc = UnsafePointer[ElementIdAllocator]()
        self.scope_id = 0
        self.list_version_signal = 0
        self.app_template_id = 0
        self.item_template_id = 0
        self.items = List[TodoItem]()
        self.next_id = 1
        self.input_text = String("")
        self.current_vnode = -1
        self.current_frag = -1
        self.ul_placeholder_id = 0
        self.items_mounted = False
        self.add_handler = 0

    fn __moveinit__(out self, deinit other: Self):
        self.runtime = other.runtime
        self.store = other.store
        self.eid_alloc = other.eid_alloc
        self.scope_id = other.scope_id
        self.list_version_signal = other.list_version_signal
        self.app_template_id = other.app_template_id
        self.item_template_id = other.item_template_id
        self.items = other.items^
        self.next_id = other.next_id
        self.input_text = other.input_text^
        self.current_vnode = other.current_vnode
        self.current_frag = other.current_frag
        self.ul_placeholder_id = other.ul_placeholder_id
        self.items_mounted = other.items_mounted
        self.add_handler = other.add_handler

    fn add_item(mut self, text: String):
        """Add a new item and bump the list version signal."""
        if len(text) == 0:
            return
        self.items.append(TodoItem(self.next_id, text, False))
        self.next_id += 1
        self._bump_version()

    fn remove_item(mut self, item_id: Int32):
        """Remove an item by ID and bump the list version signal."""
        for i in range(len(self.items)):
            if self.items[i].id == item_id:
                # Swap-remove for O(1)
                var last = len(self.items) - 1
                if i != last:
                    self.items[i] = self.items[last].copy()
                _ = self.items.pop()
                self._bump_version()
                return

    fn toggle_item(mut self, item_id: Int32):
        """Toggle an item's completed status and bump the list version signal.
        """
        for i in range(len(self.items)):
            if self.items[i].id == item_id:
                self.items[i].completed = not self.items[i].completed
                self._bump_version()
                return

    fn _bump_version(mut self):
        """Increment the list version signal to trigger re-render."""
        var current = self.runtime[0].peek_signal[Int32](
            self.list_version_signal
        )
        self.runtime[0].write_signal[Int32](
            self.list_version_signal, current + 1
        )

    fn build_item_vnode(mut self, item: TodoItem) -> UInt32:
        """Build a keyed VNode for a single todo item.

        Template "todo-item": li > [ span > dynamic_text[0], button("✓"), button("✕") ]
          dynamic_text[0] = item text (possibly with strikethrough indicator)
          dynamic_attr[0] = click on toggle button
          dynamic_attr[1] = click on remove button
          dynamic_attr[2] = class on the li element
        """
        var idx = self.store[0].push(
            VNode.template_ref_keyed(self.item_template_id, String(item.id))
        )

        # Dynamic text: item text with completion indicator
        var display_text: String
        if item.completed:
            display_text = String("✓ ") + item.text
        else:
            display_text = item.text

        self.store[0].push_dynamic_node(
            idx, DynamicNode.text_node(display_text)
        )

        # Dynamic attr 0: toggle handler (click on ✓ button)
        var toggle_handler = self.runtime[0].register_handler(
            HandlerEntry.custom(self.scope_id, String("click"))
        )
        self.store[0].push_dynamic_attr(
            idx,
            DynamicAttr(
                String("click"),
                AttributeValue.event(toggle_handler),
                UInt32(0),
            ),
        )

        # Dynamic attr 1: remove handler (click on ✕ button)
        var remove_handler = self.runtime[0].register_handler(
            HandlerEntry.custom(self.scope_id, String("click"))
        )
        self.store[0].push_dynamic_attr(
            idx,
            DynamicAttr(
                String("click"),
                AttributeValue.event(remove_handler),
                UInt32(0),
            ),
        )

        # Dynamic attr 2: class on the li element
        var li_class: String
        if item.completed:
            li_class = String("completed")
        else:
            li_class = String("")
        self.store[0].push_dynamic_attr(
            idx,
            DynamicAttr(
                String("class"),
                AttributeValue.text(li_class),
                UInt32(0),
            ),
        )

        return idx

    fn build_items_fragment(mut self) -> UInt32:
        """Build a Fragment VNode containing keyed item children."""
        var frag_idx = self.store[0].push(VNode.fragment())
        for i in range(len(self.items)):
            var item_idx = self.build_item_vnode(self.items[i].copy())
            self.store[0].push_fragment_child(frag_idx, item_idx)
        return frag_idx

    fn build_app_vnode(mut self) -> UInt32:
        """Build the app shell VNode (TemplateRef for todo-app).

        Template "todo-app": div > [ input, button("Add") + dynamic_attr[0], ul > dynamic[0] ]
          dynamic_attr[0] = click on Add button
          dynamic[0] = placeholder (item list managed separately)
        """
        var app_idx = self.store[0].push(
            VNode.template_ref(self.app_template_id)
        )

        # Dynamic node 0: placeholder in the <ul>
        self.store[0].push_dynamic_node(app_idx, DynamicNode.placeholder())

        # Dynamic attr 0: click on Add button
        self.store[0].push_dynamic_attr(
            app_idx,
            DynamicAttr(
                String("click"),
                AttributeValue.event(self.add_handler),
                UInt32(0),
            ),
        )

        return app_idx


fn _int_to_todo_ptr(addr: Int) -> UnsafePointer[TodoApp]:
    """Reinterpret an integer address as an UnsafePointer[TodoApp]."""
    var slot = UnsafePointer[Int].alloc(1)
    slot[0] = addr
    var result = slot.bitcast[UnsafePointer[TodoApp]]()[0]
    slot.free()
    return result


# ── Todo App Lifecycle Exports ───────────────────────────────────────────────


@export
fn todo_init() -> Int64:
    """Initialize the todo app.  Returns a pointer to the app state.

    Creates: runtime, VNode store, element ID allocator, scope, signals,
    templates, and event handlers.
    """
    var app_ptr = UnsafePointer[TodoApp].alloc(1)
    app_ptr.init_pointee_move(TodoApp())

    # 1. Create subsystem instances
    app_ptr[0].runtime = create_runtime()
    app_ptr[0].store = UnsafePointer[VNodeStore].alloc(1)
    app_ptr[0].store.init_pointee_move(VNodeStore())
    app_ptr[0].eid_alloc = UnsafePointer[ElementIdAllocator].alloc(1)
    app_ptr[0].eid_alloc.init_pointee_move(ElementIdAllocator())

    # 2. Create root scope and list_version signal
    app_ptr[0].scope_id = app_ptr[0].runtime[0].create_scope(0, -1)
    _ = app_ptr[0].runtime[0].begin_scope_render(app_ptr[0].scope_id)
    app_ptr[0].list_version_signal = app_ptr[0].runtime[0].use_signal_i32(0)
    # Read the signal to subscribe the scope
    _ = app_ptr[0].runtime[0].read_signal[Int32](app_ptr[0].list_version_signal)
    app_ptr[0].runtime[0].end_scope_render(-1)

    # 3. Build and register the "todo-app" template:
    #    div > [ input (placeholder), button("Add") + dynamic_attr[0], ul > dynamic[0] ]
    var app_builder_ptr = create_builder(String("todo-app"))

    var div_idx = app_builder_ptr[0].push_element(TAG_DIV, -1)

    # Input field (static in template, JS handles the value)
    var input_idx = app_builder_ptr[0].push_element(TAG_INPUT, Int(div_idx))
    app_builder_ptr[0].push_static_attr(
        Int(input_idx), String("type"), String("text")
    )
    app_builder_ptr[0].push_static_attr(
        Int(input_idx), String("placeholder"), String("What needs to be done?")
    )

    # Add button with dynamic click handler
    var btn_add = app_builder_ptr[0].push_element(TAG_BUTTON, Int(div_idx))
    var _text_add = app_builder_ptr[0].push_text(String("Add"), Int(btn_add))
    app_builder_ptr[0].push_dynamic_attr(Int(btn_add), 0)

    # ul container with dynamic[0] for the item list
    var ul_idx = app_builder_ptr[0].push_element(TAG_UL, Int(div_idx))
    var _dyn_list = app_builder_ptr[0].push_dynamic(0, Int(ul_idx))

    var app_template = app_builder_ptr[0].build()
    app_ptr[0].app_template_id = UInt32(
        app_ptr[0].runtime[0].templates.register(app_template^)
    )
    destroy_builder(app_builder_ptr)

    # 4. Build and register the "todo-item" template:
    #    li + dynamic_attr[2] > [ span > dynamic_text[0],
    #                             button("✓") + dynamic_attr[0],
    #                             button("✕") + dynamic_attr[1] ]
    var item_builder_ptr = create_builder(String("todo-item"))

    var li_idx = item_builder_ptr[0].push_element(TAG_LI, -1)
    item_builder_ptr[0].push_dynamic_attr(Int(li_idx), 2)  # class attr

    var span_idx = item_builder_ptr[0].push_element(TAG_SPAN, Int(li_idx))
    var _dyn_text = item_builder_ptr[0].push_dynamic_text(0, Int(span_idx))

    var btn_toggle = item_builder_ptr[0].push_element(TAG_BUTTON, Int(li_idx))
    var _text_toggle = item_builder_ptr[0].push_text(
        String("✓"), Int(btn_toggle)
    )
    item_builder_ptr[0].push_dynamic_attr(Int(btn_toggle), 0)  # click

    var btn_remove = item_builder_ptr[0].push_element(TAG_BUTTON, Int(li_idx))
    var _text_remove = item_builder_ptr[0].push_text(
        String("✕"), Int(btn_remove)
    )
    item_builder_ptr[0].push_dynamic_attr(Int(btn_remove), 1)  # click

    var item_template = item_builder_ptr[0].build()
    app_ptr[0].item_template_id = UInt32(
        app_ptr[0].runtime[0].templates.register(item_template^)
    )
    destroy_builder(item_builder_ptr)

    # 5. Register the Add button handler (custom — JS calls todo_add_item)
    app_ptr[0].add_handler = (
        app_ptr[0]
        .runtime[0]
        .register_handler(
            HandlerEntry.custom(app_ptr[0].scope_id, String("click"))
        )
    )

    return Int64(Int(app_ptr))


@export
fn todo_destroy(app_ptr: Int64):
    """Destroy the todo app and free all resources."""
    var ptr = _int_to_todo_ptr(Int(app_ptr))

    if ptr[0].store:
        ptr[0].store.destroy_pointee()
        ptr[0].store.free()
    if ptr[0].eid_alloc:
        ptr[0].eid_alloc.destroy_pointee()
        ptr[0].eid_alloc.free()
    if ptr[0].runtime:
        destroy_runtime(ptr[0].runtime)

    ptr.destroy_pointee()
    ptr.free()


@export
fn todo_rebuild(app_ptr: Int64, buf_ptr: Int64, capacity: Int32) -> Int32:
    """Initial render (mount) of the todo app.

    Builds the app shell VNode and mounts it.  The <ul> starts with a
    placeholder comment node whose ElementId we save for later use.

    Returns the byte offset (length) of the mutation data written.
    """
    var app = _int_to_todo_ptr(Int(app_ptr))
    var buf = _int_to_ptr(Int(buf_ptr))

    var writer_ptr = UnsafePointer[MutationWriter].alloc(1)
    writer_ptr.init_pointee_move(MutationWriter(buf, Int(capacity)))

    # Build the app shell VNode (no items yet — just the template)
    var app_vnode_idx = app[0].build_app_vnode()
    app[0].current_vnode = Int(app_vnode_idx)

    # Build an empty items fragment and store it
    var frag_idx = app[0].build_items_fragment()
    app[0].current_frag = Int(frag_idx)

    # Create the app template via CreateEngine.
    # This emits LoadTemplate, AssignId, NewEventListener, and
    # CreatePlaceholder + ReplacePlaceholder for dynamic[0].
    var engine = CreateEngine(
        writer_ptr, app[0].eid_alloc, app[0].runtime, app[0].store
    )
    var num_roots = engine.create_node(app_vnode_idx)

    # After CreateEngine, dynamic[0]'s placeholder has an ElementId.
    # Save it so we can replace it with items later.
    var app_vnode_ptr = app[0].store[0].get_ptr(app_vnode_idx)
    if app_vnode_ptr[0].dyn_node_id_count() > 0:
        app[0].ul_placeholder_id = app_vnode_ptr[0].get_dyn_node_id(0)
    app[0].items_mounted = False

    # Append the app shell to root element (id 0)
    writer_ptr[0].append_children(0, num_roots)

    writer_ptr[0].finalize()
    var offset = Int32(writer_ptr[0].offset)

    writer_ptr.destroy_pointee()
    writer_ptr.free()

    return offset


@export
fn todo_add_item(app_ptr: Int64, text: String):
    """Add a new item to the todo list.

    The text comes from JS (the input field value).
    This bumps the list version signal, marking the scope dirty.
    """
    var app = _int_to_todo_ptr(Int(app_ptr))
    app[0].add_item(text)


@export
fn todo_remove_item(app_ptr: Int64, item_id: Int32):
    """Remove an item by its ID."""
    var app = _int_to_todo_ptr(Int(app_ptr))
    app[0].remove_item(item_id)


@export
fn todo_toggle_item(app_ptr: Int64, item_id: Int32):
    """Toggle an item's completed status."""
    var app = _int_to_todo_ptr(Int(app_ptr))
    app[0].toggle_item(item_id)


@export
fn todo_set_input(app_ptr: Int64, text: String):
    """Update the input text (stored in app state, no re-render)."""
    var app = _int_to_todo_ptr(Int(app_ptr))
    app[0].input_text = text


@export
fn todo_flush(app_ptr: Int64, buf_ptr: Int64, capacity: Int32) -> Int32:
    """Flush pending updates after a list mutation.

    Handles three transitions for the item list inside the <ul>:
      1. empty → populated: create items, ReplaceWith placeholder
      2. populated → populated: diff old fragment vs new fragment (keyed)
      3. populated → empty: remove all items, CreatePlaceholder to restore anchor

    Returns the byte offset (length) of mutation data, or 0 if nothing dirty.
    """
    var app = _int_to_todo_ptr(Int(app_ptr))

    if not app[0].runtime[0].has_dirty():
        return 0

    var _dirty = app[0].runtime[0].drain_dirty()

    var buf = _int_to_ptr(Int(buf_ptr))
    var writer_ptr = UnsafePointer[MutationWriter].alloc(1)
    writer_ptr.init_pointee_move(MutationWriter(buf, Int(capacity)))

    # Build a new items fragment from the current item list
    var new_frag_idx = app[0].build_items_fragment()
    var old_frag_idx = UInt32(app[0].current_frag)

    var old_frag_ptr = app[0].store[0].get_ptr(old_frag_idx)
    var new_frag_ptr = app[0].store[0].get_ptr(new_frag_idx)
    var old_count = old_frag_ptr[0].fragment_child_count()
    var new_count = new_frag_ptr[0].fragment_child_count()

    if not app[0].items_mounted and new_count > 0:
        # ── Transition: empty → populated ─────────────────────────────
        # The <ul> currently has a placeholder comment node.  Create item
        # VNodes, push them on the stack, and ReplaceWith the placeholder.
        var create_eng = CreateEngine(
            writer_ptr, app[0].eid_alloc, app[0].runtime, app[0].store
        )
        var total_roots: UInt32 = 0
        for i in range(new_count):
            var child_idx = (
                app[0].store[0].get_ptr(new_frag_idx)[0].get_fragment_child(i)
            )
            total_roots += create_eng.create_node(child_idx)

        if app[0].ul_placeholder_id != 0 and total_roots > 0:
            writer_ptr[0].replace_with(app[0].ul_placeholder_id, total_roots)
        app[0].items_mounted = True

    elif app[0].items_mounted and new_count == 0:
        # ── Transition: populated → empty ─────────────────────────────
        # Handled after this if-elif chain (needs careful ordering:
        # create placeholder, insert before first item, then remove items).
        pass  # fall through — handled below

    elif app[0].items_mounted and new_count > 0:
        # ── Transition: populated → populated ─────────────────────────
        # Both old and new have items.  Use the keyed diff engine.
        var diff_eng = DiffEngine(
            writer_ptr, app[0].eid_alloc, app[0].runtime, app[0].store
        )
        diff_eng.diff_node(old_frag_idx, new_frag_idx)

    # else: both empty → no-op

    # Handle the populated → empty case properly (we skipped above).
    if app[0].items_mounted and new_count == 0:
        # Reset writer (it has nothing from the pass above)
        # We need to:
        #   1. Find the first old item's root ElementId
        #   2. Create a new placeholder
        #   3. InsertBefore the first old item
        #   4. Remove all old items
        var first_old_root_id: UInt32 = 0
        if old_count > 0:
            var first_child = (
                app[0].store[0].get_ptr(old_frag_idx)[0].get_fragment_child(0)
            )
            var fc_ptr = app[0].store[0].get_ptr(first_child)
            if fc_ptr[0].root_id_count() > 0:
                first_old_root_id = fc_ptr[0].get_root_id(0)
            elif fc_ptr[0].element_id != 0:
                first_old_root_id = fc_ptr[0].element_id

        # Create a new placeholder
        var new_ph_eid = app[0].eid_alloc[0].alloc()
        writer_ptr[0].create_placeholder(new_ph_eid.as_u32())

        # Insert before the first item (which is still in DOM at this point)
        if first_old_root_id != 0:
            writer_ptr[0].insert_before(first_old_root_id, 1)

        # Now remove all old items
        var diff_eng2 = DiffEngine(
            writer_ptr, app[0].eid_alloc, app[0].runtime, app[0].store
        )
        for i in range(old_count):
            var old_child = (
                app[0].store[0].get_ptr(old_frag_idx)[0].get_fragment_child(i)
            )
            diff_eng2._remove_node(old_child)

        app[0].ul_placeholder_id = new_ph_eid.as_u32()
        app[0].items_mounted = False

    # Update current fragment
    app[0].current_frag = Int(new_frag_idx)

    writer_ptr[0].finalize()
    var offset = Int32(writer_ptr[0].offset)

    writer_ptr.destroy_pointee()
    writer_ptr.free()

    return offset


# ── Todo App Query Exports ───────────────────────────────────────────────────


@export
fn todo_app_template_id(app_ptr: Int64) -> Int32:
    """Return the app template ID (for JS template registration)."""
    var app = _int_to_todo_ptr(Int(app_ptr))
    return Int32(app[0].app_template_id)


@export
fn todo_item_template_id(app_ptr: Int64) -> Int32:
    """Return the item template ID (for JS template registration)."""
    var app = _int_to_todo_ptr(Int(app_ptr))
    return Int32(app[0].item_template_id)


@export
fn todo_add_handler(app_ptr: Int64) -> Int32:
    """Return the Add button handler ID."""
    var app = _int_to_todo_ptr(Int(app_ptr))
    return Int32(app[0].add_handler)


@export
fn todo_item_count(app_ptr: Int64) -> Int32:
    """Return the number of items in the list."""
    var app = _int_to_todo_ptr(Int(app_ptr))
    return Int32(len(app[0].items))


@export
fn todo_item_id_at(app_ptr: Int64, index: Int32) -> Int32:
    """Return the ID of the item at the given index."""
    var app = _int_to_todo_ptr(Int(app_ptr))
    return app[0].items[Int(index)].id


@export
fn todo_item_completed_at(app_ptr: Int64, index: Int32) -> Int32:
    """Return 1 if the item at index is completed, 0 otherwise."""
    var app = _int_to_todo_ptr(Int(app_ptr))
    if app[0].items[Int(index)].completed:
        return 1
    return 0


@export
fn todo_has_dirty(app_ptr: Int64) -> Int32:
    """Check if the todo app has dirty scopes.  Returns 1 or 0."""
    var app = _int_to_todo_ptr(Int(app_ptr))
    if app[0].runtime[0].has_dirty():
        return 1
    return 0


@export
fn todo_list_version(app_ptr: Int64) -> Int32:
    """Return the current list version signal value."""
    var app = _int_to_todo_ptr(Int(app_ptr))
    return app[0].runtime[0].peek_signal[Int32](app[0].list_version_signal)


@export
fn todo_scope_id(app_ptr: Int64) -> Int32:
    """Return the root scope ID."""
    var app = _int_to_todo_ptr(Int(app_ptr))
    return Int32(app[0].scope_id)


# ══════════════════════════════════════════════════════════════════════════════
# Phase 9 — Performance & Polish
# ══════════════════════════════════════════════════════════════════════════════


# ── 9.4 Signal Write Batching ────────────────────────────────────────────────
#
# During a batch, multiple signal writes coalesce into a single dirty-scope
# notification.  This prevents redundant re-renders when an event handler
# writes to several signals before flushing.
#
# Usage:
#   runtime_begin_batch(rt)
#   signal_write_i32(rt, key1, val1)
#   signal_write_i32(rt, key2, val2)
#   runtime_end_batch(rt)
#   # → only one dirty entry per affected scope


@export
fn runtime_begin_batch(rt_ptr: Int64):
    """Begin a signal write batch.

    While batching, signal writes still update values and accumulate
    dirty scopes, but no duplicate entries are added.  Call
    runtime_end_batch() to finalize.
    """
    # Batching is implicit in the current design — dirty_scopes already
    # deduplicates.  This export exists so JS can explicitly bracket
    # multi-write handlers for clarity and future optimization.
    pass


@export
fn runtime_end_batch(rt_ptr: Int64):
    """End a signal write batch.  The dirty queue is ready for drain."""
    pass


# ── 9.5 Debug Mode ──────────────────────────────────────────────────────────
#
# Debug exports expose internal state for logging and profiling.


@export
fn debug_signal_store_capacity(rt_ptr: Int64) -> Int32:
    """Return the total number of signal slots (occupied + free)."""
    var rt = _get_runtime(rt_ptr)
    return Int32(len(rt[0].signals._entries))


@export
fn debug_scope_store_capacity(rt_ptr: Int64) -> Int32:
    """Return the total number of scope slots (occupied + free)."""
    var rt = _get_runtime(rt_ptr)
    return Int32(len(rt[0].scopes._scopes))


@export
fn debug_vnode_store_count(store_ptr: Int64) -> Int32:
    """Return the number of VNodes in a standalone store."""
    var s = _get_vnode_store(store_ptr)
    return Int32(s[0].count())


@export
fn debug_handler_store_capacity(rt_ptr: Int64) -> Int32:
    """Return the total number of handler slots."""
    var rt = _get_runtime(rt_ptr)
    return Int32(len(rt[0].handlers._entries))


@export
fn debug_eid_alloc_capacity(alloc_ptr: Int64) -> Int32:
    """Return the total number of ElementId slots."""
    var a = _get_eid_alloc(alloc_ptr)
    return Int32(len(a[0]._slots))


# ── 9.2 Memory Management Test Exports ───────────────────────────────────────
#
# These exports allow the JS test harness to exercise allocation/deallocation
# cycles and verify that memory usage stays bounded.


@export
fn mem_test_signal_cycle(rt_ptr: Int64, count: Int32) -> Int32:
    """Create and destroy `count` signals.  Returns final signal_count (should be 0).
    """
    var rt = _get_runtime(rt_ptr)
    var keys = List[UInt32]()
    for i in range(Int(count)):
        keys.append(rt[0].create_signal[Int32](Int32(i)))
    for i in range(Int(count)):
        rt[0].destroy_signal(keys[i])
    return Int32(rt[0].signals.signal_count())


@export
fn mem_test_scope_cycle(rt_ptr: Int64, count: Int32) -> Int32:
    """Create and destroy `count` scopes.  Returns final scope_count (should be 0).
    """
    var rt = _get_runtime(rt_ptr)
    var ids = List[UInt32]()
    for i in range(Int(count)):
        ids.append(rt[0].create_scope(0, -1))
    for i in range(Int(count)):
        rt[0].destroy_scope(ids[i])
    return Int32(rt[0].scope_count())


@export
fn mem_test_rapid_writes(rt_ptr: Int64, key: Int32, count: Int32) -> Int32:
    """Write to a signal `count` times in sequence.  Returns final dirty_count.
    """
    var rt = _get_runtime(rt_ptr)
    for i in range(Int(count)):
        rt[0].write_signal[Int32](UInt32(key), Int32(i))
    return Int32(rt[0].dirty_count())


# ── 9.1 Benchmark App ───────────────────────────────────────────────────────
#
# Implements the js-framework-benchmark operations:
#   - Create N rows
#   - Append N rows
#   - Update every 10th row
#   - Select row (highlight)
#   - Swap rows (indices 1 and 998)
#   - Remove row
#   - Clear all rows
#
# Each row has: id (int), label (string).
# The selected row id is tracked separately.
#
# Template structure:
#   "bench-row": tr + dynamic_attr[0](class) > [
#       td > dynamic_text[0] (id),
#       td > a > dynamic_text[1] (label),
#       td > a > dynamic_text[2] ("Delete" / "×")
#   ]
#   dynamic_attr[1] = click on label (select)
#   dynamic_attr[2] = click on delete button (remove)
#
# We use a simple linear congruential generator for pseudo-random labels
# to match the benchmark's adjective + colour + noun pattern.


struct BenchRow(Copyable, Movable):
    """A single benchmark table row."""

    var id: Int32
    var label: String

    fn __init__(out self, id: Int32, label: String):
        self.id = id
        self.label = label

    fn __copyinit__(out self, other: Self):
        self.id = other.id
        self.label = other.label

    fn __moveinit__(out self, deinit other: Self):
        self.id = other.id
        self.label = other.label^


# ── Label generation ─────────────────────────────────────────────────────────

alias _ADJ_COUNT: Int = 12
alias _COL_COUNT: Int = 11
alias _NOUN_COUNT: Int = 12


fn _adjective(idx: Int) -> String:
    if idx == 0:
        return "pretty"
    elif idx == 1:
        return "large"
    elif idx == 2:
        return "big"
    elif idx == 3:
        return "small"
    elif idx == 4:
        return "tall"
    elif idx == 5:
        return "short"
    elif idx == 6:
        return "long"
    elif idx == 7:
        return "handsome"
    elif idx == 8:
        return "plain"
    elif idx == 9:
        return "quaint"
    elif idx == 10:
        return "clean"
    else:
        return "elegant"


fn _colour(idx: Int) -> String:
    if idx == 0:
        return "red"
    elif idx == 1:
        return "yellow"
    elif idx == 2:
        return "blue"
    elif idx == 3:
        return "green"
    elif idx == 4:
        return "pink"
    elif idx == 5:
        return "brown"
    elif idx == 6:
        return "purple"
    elif idx == 7:
        return "orange"
    elif idx == 8:
        return "white"
    elif idx == 9:
        return "black"
    else:
        return "grey"


fn _noun(idx: Int) -> String:
    if idx == 0:
        return "table"
    elif idx == 1:
        return "chair"
    elif idx == 2:
        return "house"
    elif idx == 3:
        return "bbq"
    elif idx == 4:
        return "desk"
    elif idx == 5:
        return "car"
    elif idx == 6:
        return "pony"
    elif idx == 7:
        return "cookie"
    elif idx == 8:
        return "sandwich"
    elif idx == 9:
        return "burger"
    elif idx == 10:
        return "pizza"
    else:
        return "mouse"


struct BenchmarkApp(Movable):
    """Js-framework-benchmark app state.

    Manages a list of rows, selection state, and all rendering
    infrastructure (runtime, templates, vnode store, etc.).
    """

    var runtime: UnsafePointer[Runtime]
    var store: UnsafePointer[VNodeStore]
    var eid_alloc: UnsafePointer[ElementIdAllocator]
    var scope_id: UInt32
    var version_signal: UInt32  # bumped on list changes
    var selected_signal: UInt32  # currently selected row id (0 = none)
    var row_template_id: UInt32
    var rows: List[BenchRow]
    var next_id: Int32
    var rng_state: UInt32  # simple LCG state
    var current_frag: Int  # Fragment VNode index, or -1
    var anchor_id: UInt32  # ElementId of anchor node (placeholder when empty)
    var rows_mounted: Bool

    fn __init__(out self):
        self.runtime = UnsafePointer[Runtime]()
        self.store = UnsafePointer[VNodeStore]()
        self.eid_alloc = UnsafePointer[ElementIdAllocator]()
        self.scope_id = 0
        self.version_signal = 0
        self.selected_signal = 0
        self.row_template_id = 0
        self.rows = List[BenchRow]()
        self.next_id = 1
        self.rng_state = 42
        self.current_frag = -1
        self.anchor_id = 0
        self.rows_mounted = False

    fn __moveinit__(out self, deinit other: Self):
        self.runtime = other.runtime
        self.store = other.store
        self.eid_alloc = other.eid_alloc
        self.scope_id = other.scope_id
        self.version_signal = other.version_signal
        self.selected_signal = other.selected_signal
        self.row_template_id = other.row_template_id
        self.rows = other.rows^
        self.next_id = other.next_id
        self.rng_state = other.rng_state
        self.current_frag = other.current_frag
        self.anchor_id = other.anchor_id
        self.rows_mounted = other.rows_mounted

    fn _next_random(mut self) -> UInt32:
        """Simple LCG: state = state * 1664525 + 1013904223."""
        self.rng_state = self.rng_state * 1664525 + 1013904223
        return self.rng_state

    fn _generate_label(mut self) -> String:
        """Generate a random "adjective colour noun" label."""
        var a = Int(self._next_random() % _ADJ_COUNT)
        var c = Int(self._next_random() % _COL_COUNT)
        var n = Int(self._next_random() % _NOUN_COUNT)
        return _adjective(a) + " " + _colour(c) + " " + _noun(n)

    fn _bump_version(mut self):
        """Increment the version signal to trigger re-render."""
        var current = self.runtime[0].peek_signal[Int32](self.version_signal)
        self.runtime[0].write_signal[Int32](self.version_signal, current + 1)

    fn create_rows(mut self, count: Int):
        """Replace all rows with `count` newly generated rows."""
        self.rows = List[BenchRow]()
        for _ in range(count):
            var label = self._generate_label()
            self.rows.append(BenchRow(self.next_id, label))
            self.next_id += 1
        self._bump_version()

    fn append_rows(mut self, count: Int):
        """Append `count` newly generated rows to the list."""
        for _ in range(count):
            var label = self._generate_label()
            self.rows.append(BenchRow(self.next_id, label))
            self.next_id += 1
        self._bump_version()

    fn update_every_10th(mut self):
        """Append " !!!" to every 10th row's label."""
        var i = 0
        while i < len(self.rows):
            self.rows[i].label = self.rows[i].label + " !!!"
            i += 10
        self._bump_version()

    fn select_row(mut self, id: Int32):
        """Select the row with the given id."""
        self.runtime[0].write_signal[Int32](self.selected_signal, id)

    fn swap_rows(mut self, a: Int, b: Int):
        """Swap two rows by their list indices."""
        if a < 0 or b < 0 or a >= len(self.rows) or b >= len(self.rows):
            return
        if a == b:
            return
        var tmp = self.rows[a].copy()
        self.rows[a] = self.rows[b].copy()
        self.rows[b] = tmp.copy()
        self._bump_version()

    fn remove_row(mut self, id: Int32):
        """Remove a row by id."""
        for i in range(len(self.rows)):
            if self.rows[i].id == id:
                var last = len(self.rows) - 1
                if i != last:
                    self.rows[i] = self.rows[last].copy()
                _ = self.rows.pop()
                self._bump_version()
                return

    fn clear_rows(mut self):
        """Remove all rows."""
        self.rows = List[BenchRow]()
        self._bump_version()

    fn build_row_vnode(mut self, row: BenchRow) -> UInt32:
        """Build a keyed VNode for a single benchmark row.

        Template "bench-row": tr > [ td(id), td > a(label), td > a("×") ]
          dynamic_attr[0] = class on <tr> ("danger" if selected)
          dynamic_text[0] = row id
          dynamic_text[1] = row label
          dynamic_text[2] = "×" (static but encoded as dynamic for simplicity)
          dynamic_attr[1] = click on label <a> (select)
          dynamic_attr[2] = click on delete <a> (remove)
        """
        var idx = self.store[0].push(
            VNode.template_ref_keyed(self.row_template_id, String(row.id))
        )

        # Dynamic text 0: row id
        self.store[0].push_dynamic_node(
            idx, DynamicNode.text_node(String(row.id))
        )

        # Dynamic text 1: row label
        self.store[0].push_dynamic_node(idx, DynamicNode.text_node(row.label))

        # Dynamic attr 0: class on <tr> ("danger" if selected)
        var selected = self.runtime[0].peek_signal[Int32](self.selected_signal)
        var tr_class: String
        if selected == row.id:
            tr_class = String("danger")
        else:
            tr_class = String("")
        self.store[0].push_dynamic_attr(
            idx,
            DynamicAttr(
                String("class"),
                AttributeValue.text(tr_class),
                UInt32(0),
            ),
        )

        # Dynamic attr 1: click on label <a> (select — custom handler)
        var select_handler = self.runtime[0].register_handler(
            HandlerEntry.custom(self.scope_id, String("click"))
        )
        self.store[0].push_dynamic_attr(
            idx,
            DynamicAttr(
                String("click"),
                AttributeValue.event(select_handler),
                UInt32(0),
            ),
        )

        # Dynamic attr 2: click on delete <a> (remove — custom handler)
        var remove_handler = self.runtime[0].register_handler(
            HandlerEntry.custom(self.scope_id, String("click"))
        )
        self.store[0].push_dynamic_attr(
            idx,
            DynamicAttr(
                String("click"),
                AttributeValue.event(remove_handler),
                UInt32(0),
            ),
        )

        return idx

    fn build_rows_fragment(mut self) -> UInt32:
        """Build a Fragment VNode containing all row VNodes."""
        var frag_idx = self.store[0].push(VNode.fragment())
        for i in range(len(self.rows)):
            var row_idx = self.build_row_vnode(self.rows[i].copy())
            self.store[0].push_fragment_child(frag_idx, row_idx)
        return frag_idx


fn _int_to_bench_ptr(addr: Int) -> UnsafePointer[BenchmarkApp]:
    """Reinterpret an integer address as an UnsafePointer[BenchmarkApp]."""
    var slot = UnsafePointer[Int].alloc(1)
    slot[0] = addr
    var result = slot.bitcast[UnsafePointer[BenchmarkApp]]()[0]
    slot.free()
    return result


# ── Benchmark App Lifecycle Exports ──────────────────────────────────────────


@export
fn bench_init() -> Int64:
    """Initialize the benchmark app.  Returns a pointer to the app state.

    Creates: runtime, VNode store, element ID allocator, scope, signals,
    and the row template.
    """
    var app_ptr = UnsafePointer[BenchmarkApp].alloc(1)
    app_ptr.init_pointee_move(BenchmarkApp())

    # 1. Create subsystem instances
    app_ptr[0].runtime = create_runtime()
    app_ptr[0].store = UnsafePointer[VNodeStore].alloc(1)
    app_ptr[0].store.init_pointee_move(VNodeStore())
    app_ptr[0].eid_alloc = UnsafePointer[ElementIdAllocator].alloc(1)
    app_ptr[0].eid_alloc.init_pointee_move(ElementIdAllocator())

    # 2. Create root scope and signals
    app_ptr[0].scope_id = app_ptr[0].runtime[0].create_scope(0, -1)
    _ = app_ptr[0].runtime[0].begin_scope_render(app_ptr[0].scope_id)
    app_ptr[0].version_signal = app_ptr[0].runtime[0].use_signal_i32(0)
    app_ptr[0].selected_signal = app_ptr[0].runtime[0].use_signal_i32(0)
    # Read signals to subscribe scope
    _ = app_ptr[0].runtime[0].read_signal[Int32](app_ptr[0].version_signal)
    _ = app_ptr[0].runtime[0].read_signal[Int32](app_ptr[0].selected_signal)
    app_ptr[0].runtime[0].end_scope_render(-1)

    # 3. Build and register the "bench-row" template:
    #    tr + dynamic_attr[0](class) > [
    #        td > dynamic_text[0],          ← id
    #        td > a + dynamic_attr[1] > dynamic_text[1],  ← label + select click
    #        td > a + dynamic_attr[2] > text("×")         ← delete click
    #    ]
    var builder_ptr = create_builder(String("bench-row"))

    var tr_idx = builder_ptr[0].push_element(TAG_TR, -1)
    builder_ptr[0].push_dynamic_attr(Int(tr_idx), 0)  # class

    var td_id = builder_ptr[0].push_element(TAG_TD, Int(tr_idx))
    var _dyn_id = builder_ptr[0].push_dynamic_text(0, Int(td_id))

    var td_label = builder_ptr[0].push_element(TAG_TD, Int(tr_idx))
    var a_label = builder_ptr[0].push_element(TAG_A, Int(td_label))
    builder_ptr[0].push_dynamic_attr(Int(a_label), 1)  # click select
    var _dyn_label = builder_ptr[0].push_dynamic_text(1, Int(a_label))

    var td_action = builder_ptr[0].push_element(TAG_TD, Int(tr_idx))
    var a_remove = builder_ptr[0].push_element(TAG_A, Int(td_action))
    builder_ptr[0].push_dynamic_attr(Int(a_remove), 2)  # click remove
    var _dyn_remove = builder_ptr[0].push_text(String("×"), Int(a_remove))

    var row_template = builder_ptr[0].build()
    app_ptr[0].row_template_id = UInt32(
        app_ptr[0].runtime[0].templates.register(row_template^)
    )
    destroy_builder(builder_ptr)

    return Int64(Int(app_ptr))


@export
fn bench_destroy(app_ptr: Int64):
    """Destroy the benchmark app and free all resources."""
    var ptr = _int_to_bench_ptr(Int(app_ptr))
    if ptr[0].store:
        ptr[0].store.destroy_pointee()
        ptr[0].store.free()
    if ptr[0].eid_alloc:
        ptr[0].eid_alloc.destroy_pointee()
        ptr[0].eid_alloc.free()
    if ptr[0].runtime:
        destroy_runtime(ptr[0].runtime)
    ptr.destroy_pointee()
    ptr.free()


@export
fn bench_create(app_ptr: Int64, count: Int32):
    """Replace all rows with `count` new rows (benchmark: create)."""
    var app = _int_to_bench_ptr(Int(app_ptr))
    app[0].create_rows(Int(count))


@export
fn bench_append(app_ptr: Int64, count: Int32):
    """Append `count` new rows (benchmark: append)."""
    var app = _int_to_bench_ptr(Int(app_ptr))
    app[0].append_rows(Int(count))


@export
fn bench_update(app_ptr: Int64):
    """Update every 10th row label (benchmark: update)."""
    var app = _int_to_bench_ptr(Int(app_ptr))
    app[0].update_every_10th()


@export
fn bench_select(app_ptr: Int64, id: Int32):
    """Select a row by id (benchmark: select)."""
    var app = _int_to_bench_ptr(Int(app_ptr))
    app[0].select_row(id)


@export
fn bench_swap(app_ptr: Int64):
    """Swap rows at indices 1 and 998 (benchmark: swap)."""
    var app = _int_to_bench_ptr(Int(app_ptr))
    app[0].swap_rows(1, 998)


@export
fn bench_remove(app_ptr: Int64, id: Int32):
    """Remove a row by id (benchmark: remove)."""
    var app = _int_to_bench_ptr(Int(app_ptr))
    app[0].remove_row(id)


@export
fn bench_clear(app_ptr: Int64):
    """Clear all rows (benchmark: clear)."""
    var app = _int_to_bench_ptr(Int(app_ptr))
    app[0].clear_rows()


@export
fn bench_rebuild(app_ptr: Int64, buf_ptr: Int64, capacity: Int32) -> Int32:
    """Initial render of the benchmark table body.

    Creates an anchor placeholder in the DOM (will be replaced on first
    populate).  Emits mutations for the initial empty state.

    Returns byte offset (length) of mutation data.
    """
    var app = _int_to_bench_ptr(Int(app_ptr))
    var buf = _int_to_ptr(Int(buf_ptr))

    var writer_ptr = UnsafePointer[MutationWriter].alloc(1)
    writer_ptr.init_pointee_move(MutationWriter(buf, Int(capacity)))

    # Create an anchor placeholder
    var anchor_eid = app[0].eid_alloc[0].alloc()
    app[0].anchor_id = anchor_eid.as_u32()
    writer_ptr[0].create_placeholder(anchor_eid.as_u32())
    writer_ptr[0].append_children(0, 1)

    # Build initial empty fragment
    var frag_idx = app[0].build_rows_fragment()
    app[0].current_frag = Int(frag_idx)
    app[0].rows_mounted = False

    writer_ptr[0].finalize()
    var offset = Int32(writer_ptr[0].offset)
    writer_ptr.destroy_pointee()
    writer_ptr.free()

    return offset


@export
fn bench_flush(app_ptr: Int64, buf_ptr: Int64, capacity: Int32) -> Int32:
    """Flush pending updates after a benchmark operation.

    Handles transitions:
      - empty → populated: create rows, ReplaceWith anchor
      - populated → populated: diff old fragment vs new fragment (keyed)
      - populated → empty: remove rows, recreate anchor

    Returns byte offset (length) of mutation data, or 0 if nothing dirty.
    """
    var app = _int_to_bench_ptr(Int(app_ptr))

    if not app[0].runtime[0].has_dirty():
        return 0

    var _dirty = app[0].runtime[0].drain_dirty()

    var buf = _int_to_ptr(Int(buf_ptr))
    var writer_ptr = UnsafePointer[MutationWriter].alloc(1)
    writer_ptr.init_pointee_move(MutationWriter(buf, Int(capacity)))

    var new_frag_idx = app[0].build_rows_fragment()
    var old_frag_idx = UInt32(app[0].current_frag)

    var old_frag_ptr = app[0].store[0].get_ptr(old_frag_idx)
    var new_frag_ptr = app[0].store[0].get_ptr(new_frag_idx)
    var old_count = old_frag_ptr[0].fragment_child_count()
    var new_count = new_frag_ptr[0].fragment_child_count()

    if not app[0].rows_mounted and new_count > 0:
        # ── Transition: empty → populated ─────────────────────────────
        var create_eng = CreateEngine(
            writer_ptr, app[0].eid_alloc, app[0].runtime, app[0].store
        )
        var total_roots: UInt32 = 0
        for i in range(new_count):
            var child_idx = (
                app[0].store[0].get_ptr(new_frag_idx)[0].get_fragment_child(i)
            )
            total_roots += create_eng.create_node(child_idx)

        if app[0].anchor_id != 0 and total_roots > 0:
            writer_ptr[0].replace_with(app[0].anchor_id, total_roots)
        app[0].rows_mounted = True

    elif app[0].rows_mounted and new_count == 0:
        # ── Transition: populated → empty ─────────────────────────────
        # Create a new anchor placeholder
        var first_old_root_id: UInt32 = 0
        if old_count > 0:
            var first_child = (
                app[0].store[0].get_ptr(old_frag_idx)[0].get_fragment_child(0)
            )
            var fc_ptr = app[0].store[0].get_ptr(first_child)
            if fc_ptr[0].root_id_count() > 0:
                first_old_root_id = fc_ptr[0].get_root_id(0)
            elif fc_ptr[0].element_id != 0:
                first_old_root_id = fc_ptr[0].element_id

        var new_anchor = app[0].eid_alloc[0].alloc()
        writer_ptr[0].create_placeholder(new_anchor.as_u32())

        if first_old_root_id != 0:
            writer_ptr[0].insert_before(first_old_root_id, 1)

        # Remove all old rows
        var diff_eng = DiffEngine(
            writer_ptr, app[0].eid_alloc, app[0].runtime, app[0].store
        )
        for i in range(old_count):
            var old_child = (
                app[0].store[0].get_ptr(old_frag_idx)[0].get_fragment_child(i)
            )
            diff_eng._remove_node(old_child)

        app[0].anchor_id = new_anchor.as_u32()
        app[0].rows_mounted = False

    elif app[0].rows_mounted and new_count > 0:
        # ── Transition: populated → populated ─────────────────────────
        var diff_eng = DiffEngine(
            writer_ptr, app[0].eid_alloc, app[0].runtime, app[0].store
        )
        diff_eng.diff_node(old_frag_idx, new_frag_idx)

    # else: both empty → no-op

    app[0].current_frag = Int(new_frag_idx)

    writer_ptr[0].finalize()
    var offset = Int32(writer_ptr[0].offset)
    writer_ptr.destroy_pointee()
    writer_ptr.free()

    return offset


# ── Benchmark App Query Exports ──────────────────────────────────────────────


@export
fn bench_row_count(app_ptr: Int64) -> Int32:
    """Return the number of rows."""
    var app = _int_to_bench_ptr(Int(app_ptr))
    return Int32(len(app[0].rows))


@export
fn bench_row_id_at(app_ptr: Int64, index: Int32) -> Int32:
    """Return the id of the row at the given index."""
    var app = _int_to_bench_ptr(Int(app_ptr))
    return app[0].rows[Int(index)].id


@export
fn bench_selected(app_ptr: Int64) -> Int32:
    """Return the currently selected row id (0 = none)."""
    var app = _int_to_bench_ptr(Int(app_ptr))
    return app[0].runtime[0].peek_signal[Int32](app[0].selected_signal)


@export
fn bench_has_dirty(app_ptr: Int64) -> Int32:
    """Check if the benchmark app has dirty scopes.  Returns 1 or 0."""
    var app = _int_to_bench_ptr(Int(app_ptr))
    if app[0].runtime[0].has_dirty():
        return 1
    return 0


@export
fn bench_version(app_ptr: Int64) -> Int32:
    """Return the current version signal value."""
    var app = _int_to_bench_ptr(Int(app_ptr))
    return app[0].runtime[0].peek_signal[Int32](app[0].version_signal)


@export
fn bench_row_template_id(app_ptr: Int64) -> Int32:
    """Return the row template ID (for JS template registration)."""
    var app = _int_to_bench_ptr(Int(app_ptr))
    return Int32(app[0].row_template_id)


@export
fn bench_scope_id(app_ptr: Int64) -> Int32:
    """Return the root scope ID."""
    var app = _int_to_bench_ptr(Int(app_ptr))
    return Int32(app[0].scope_id)


# Factorial (iterative)
@export
fn factorial_int32(n: Int32) -> Int32:
    if n <= 1:
        return 1
    var result: Int32 = 1
    for i in range(2, Int(n) + 1):
        result *= Int32(i)
    return result


@export
fn factorial_int64(n: Int64) -> Int64:
    if n <= 1:
        return 1
    var result: Int64 = 1
    for i in range(2, Int(n) + 1):
        result *= Int64(i)
    return result


# GCD (Euclidean algorithm)
@export
fn gcd_int32(x: Int32, y: Int32) -> Int32:
    var a = x
    var b = y
    if a < 0:
        a = -a
    if b < 0:
        b = -b
    while b != 0:
        var tmp = b
        b = a % b
        a = tmp
    return a


# Identity / passthrough
@export
fn identity_int32(x: Int32) -> Int32:
    return x


@export
fn identity_int64(x: Int64) -> Int64:
    return x


@export
fn identity_float32(x: Float32) -> Float32:
    return x


@export
fn identity_float64(x: Float64) -> Float64:
    return x


# Print
@export
fn print_int32():
    alias int32: Int32 = 3
    print(int32)


@export
fn print_int64():
    alias int64: Int64 = 3
    print(2)


@export
fn print_float32():
    alias float32: Float32 = 3.0
    print(float32)


@export
fn print_float64():
    alias float64: Float64 = 3.0
    print(float64)


@export
fn print_static_string():
    print("print-static-string")


# Print input
@export
fn print_input_string(input: String):
    print(input)


# Return
@export
fn return_input_string(x: String) -> String:
    return x


@export
fn return_static_string() -> String:
    return "return-static-string"


# String length
@export
fn string_length(x: String) -> Int64:
    return Int64(len(x))


# String concatenation
@export
fn string_concat(x: String, y: String) -> String:
    return x + y


# String repeat
@export
fn string_repeat(x: String, n: Int32) -> String:
    var result = String("")
    for _ in range(Int(n)):
        result += x
    return result


# String equality
@export
fn string_eq(x: String, y: String) -> Bool:
    return x == y

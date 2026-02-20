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

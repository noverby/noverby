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
    TAG_SECTION,
    TAG_HEADER,
    TAG_FOOTER,
    TAG_NAV,
    TAG_MAIN,
    TAG_ARTICLE,
    TAG_ASIDE,
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
    TAG_TEXTAREA,
    TAG_SELECT,
    TAG_OPTION,
    TAG_LABEL,
    TAG_A,
    TAG_IMG,
    TAG_TABLE,
    TAG_THEAD,
    TAG_TBODY,
    TAG_TR,
    TAG_TD,
    TAG_TH,
    TAG_STRONG,
    TAG_EM,
    TAG_BR,
    TAG_HR,
    TAG_PRE,
    TAG_CODE,
    TAG_UNKNOWN,
    # DSL — Ergonomic builder API (M10.5)
    Node,
    NODE_TEXT,
    NODE_ELEMENT,
    NODE_DYN_TEXT,
    NODE_DYN_NODE,
    NODE_STATIC_ATTR,
    NODE_DYN_ATTR,
    text,
    dyn_text,
    dyn_node,
    attr,
    dyn_attr,
    el,
    el_empty,
    el_div,
    el_span,
    el_p,
    el_section,
    el_header,
    el_footer,
    el_nav,
    el_main,
    el_article,
    el_aside,
    el_h1,
    el_h2,
    el_h3,
    el_h4,
    el_h5,
    el_h6,
    el_ul,
    el_ol,
    el_li,
    el_button,
    el_input,
    el_form,
    el_textarea,
    el_select,
    el_option,
    el_label,
    el_a,
    el_img,
    el_table,
    el_thead,
    el_tbody,
    el_tr,
    el_td,
    el_th,
    el_strong,
    el_em,
    el_br,
    el_hr,
    el_pre,
    el_code,
    to_template,
    to_template_multi,
    VNodeBuilder,
    count_nodes,
    count_all_items,
    count_dynamic_text_slots,
    count_dynamic_node_slots,
    count_dynamic_attr_slots,
    count_static_attr_nodes,
)
from scheduler import Scheduler, SchedulerEntry
from component import (
    AppShell,
    app_shell_create,
    mount_vnode,
    mount_vnode_to,
    diff_and_finalize,
    diff_no_finalize,
    create_no_finalize,
)
from poc import (
    poc_add_int32,
    poc_add_int64,
    poc_add_float32,
    poc_add_float64,
    poc_sub_int32,
    poc_sub_int64,
    poc_sub_float32,
    poc_sub_float64,
    poc_mul_int32,
    poc_mul_int64,
    poc_mul_float32,
    poc_mul_float64,
    poc_div_int32,
    poc_div_int64,
    poc_div_float32,
    poc_div_float64,
    poc_mod_int32,
    poc_mod_int64,
    poc_pow_int32,
    poc_pow_int64,
    poc_pow_float32,
    poc_pow_float64,
    poc_neg_int32,
    poc_neg_int64,
    poc_neg_float32,
    poc_neg_float64,
    poc_abs_int32,
    poc_abs_int64,
    poc_abs_float32,
    poc_abs_float64,
    poc_min_int32,
    poc_max_int32,
    poc_min_int64,
    poc_max_int64,
    poc_min_float64,
    poc_max_float64,
    poc_clamp_int32,
    poc_clamp_float64,
    poc_bitand_int32,
    poc_bitor_int32,
    poc_bitxor_int32,
    poc_bitnot_int32,
    poc_shl_int32,
    poc_shr_int32,
    poc_eq_int32,
    poc_ne_int32,
    poc_lt_int32,
    poc_le_int32,
    poc_gt_int32,
    poc_ge_int32,
    poc_bool_and,
    poc_bool_or,
    poc_bool_not,
    poc_fib_int32,
    poc_fib_int64,
    poc_factorial_int32,
    poc_factorial_int64,
    poc_gcd_int32,
    poc_identity_int32,
    poc_identity_int64,
    poc_identity_float32,
    poc_identity_float64,
    poc_print_int32,
    poc_print_int64,
    poc_print_float32,
    poc_print_float64,
    poc_print_static_string,
    poc_print_input_string,
    poc_return_input_string,
    poc_return_static_string,
    poc_string_length,
    poc_string_concat,
    poc_string_repeat,
    poc_string_eq,
)
from apps import (
    CounterApp,
    counter_app_init,
    counter_app_destroy,
    counter_app_rebuild,
    counter_app_handle_event,
    counter_app_flush,
    TodoApp,
    TodoItem,
    todo_app_init,
    todo_app_destroy,
    todo_app_rebuild,
    todo_app_flush,
    BenchmarkApp,
    BenchRow,
    bench_app_init,
    bench_app_destroy,
    bench_app_rebuild,
    bench_app_flush,
)
from memory import UnsafePointer


# ══════════════════════════════════════════════════════════════════════════════
# Pointer ↔ Int helpers
# ══════════════════════════════════════════════════════════════════════════════
#
# Mojo 0.25 does not support UnsafePointer construction from an integer
# address directly.  We reinterpret the bits via a temporary heap slot.


@always_inline
fn _int_to_ptr(addr: Int) -> UnsafePointer[UInt8]:
    """Reinterpret an integer address as an UnsafePointer[UInt8]."""
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


@always_inline
fn _int_to_counter_ptr(addr: Int) -> UnsafePointer[CounterApp]:
    """Reinterpret an integer address as an UnsafePointer[CounterApp]."""
    var slot = UnsafePointer[Int].alloc(1)
    slot[0] = addr
    var result = slot.bitcast[UnsafePointer[CounterApp]]()[0]
    slot.free()
    return result


@always_inline
fn _int_to_todo_ptr(addr: Int) -> UnsafePointer[TodoApp]:
    """Reinterpret an integer address as an UnsafePointer[TodoApp]."""
    var slot = UnsafePointer[Int].alloc(1)
    slot[0] = addr
    var result = slot.bitcast[UnsafePointer[TodoApp]]()[0]
    slot.free()
    return result


@always_inline
fn _int_to_bench_ptr(addr: Int) -> UnsafePointer[BenchmarkApp]:
    """Reinterpret an integer address as an UnsafePointer[BenchmarkApp]."""
    var slot = UnsafePointer[Int].alloc(1)
    slot[0] = addr
    var result = slot.bitcast[UnsafePointer[BenchmarkApp]]()[0]
    slot.free()
    return result


# ── Helper: quick get-pointer wrappers ───────────────────────────────────────


fn _get_eid_alloc(alloc_ptr: Int64) -> UnsafePointer[ElementIdAllocator]:
    return _int_to_eid_alloc_ptr(Int(alloc_ptr))


fn _get_runtime(rt_ptr: Int64) -> UnsafePointer[Runtime]:
    return _int_to_runtime_ptr(Int(rt_ptr))


fn _get_builder(ptr: Int64) -> UnsafePointer[TemplateBuilder]:
    return _int_to_builder_ptr(Int(ptr))


fn _get_vnode_store(store_ptr: Int64) -> UnsafePointer[VNodeStore]:
    return _int_to_vnode_store_ptr(Int(store_ptr))


# ── Helper: writer at (buf, off) ────────────────────────────────────────────


@always_inline
fn _writer(buf: Int64, off: Int32) -> MutationWriter:
    return MutationWriter(_int_to_ptr(Int(buf)), Int(off), 0)


# ══════════════════════════════════════════════════════════════════════════════
# ElementId Allocator Test Exports
# ══════════════════════════════════════════════════════════════════════════════


@export
fn eid_alloc_create() -> Int64:
    """Allocate an ElementIdAllocator on the heap."""
    var ptr = UnsafePointer[ElementIdAllocator].alloc(1)
    ptr.init_pointee_move(ElementIdAllocator())
    return Int64(Int(ptr))


@export
fn eid_alloc_destroy(alloc_ptr: Int64):
    """Destroy and free a heap-allocated ElementIdAllocator."""
    var ptr = _get_eid_alloc(alloc_ptr)
    ptr.destroy_pointee()
    ptr.free()


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


# ══════════════════════════════════════════════════════════════════════════════
# Runtime / Signals Test Exports
# ══════════════════════════════════════════════════════════════════════════════


@export
fn runtime_create() -> Int64:
    """Allocate a reactive Runtime on the heap."""
    return _runtime_ptr_to_i64(create_runtime())


@export
fn runtime_destroy(rt_ptr: Int64):
    """Destroy and free a heap-allocated Runtime."""
    destroy_runtime(_int_to_runtime_ptr(Int(rt_ptr)))


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


# ══════════════════════════════════════════════════════════════════════════════
# Scope Management Exports
# ══════════════════════════════════════════════════════════════════════════════


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
    """Begin rendering a scope.  Returns the previous scope ID (or -1)."""
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


# ══════════════════════════════════════════════════════════════════════════════
# Template Builder Exports
# ══════════════════════════════════════════════════════════════════════════════


@export
fn tmpl_builder_create(name: String) -> Int64:
    """Create a heap-allocated TemplateBuilder.  Returns its pointer."""
    return _builder_ptr_to_i64(create_builder(name))


@export
fn tmpl_builder_destroy(ptr: Int64):
    """Destroy and free a heap-allocated TemplateBuilder."""
    destroy_builder(_int_to_builder_ptr(Int(ptr)))


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


# ══════════════════════════════════════════════════════════════════════════════
# VNode Store Exports
# ══════════════════════════════════════════════════════════════════════════════


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


# ── Signal arithmetic helpers ────────────────────────────────────────────────


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


# ══════════════════════════════════════════════════════════════════════════════
# Create & Diff Engine Exports
# ══════════════════════════════════════════════════════════════════════════════


@export
fn writer_create(buf_ptr: Int64, capacity: Int32) -> Int64:
    """Create a heap-allocated MutationWriter.  Returns its pointer."""
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
    """Create mutations for the VNode at vnode_index.  Returns root count."""
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
    """Diff old and new VNodes and emit mutations.  Returns writer offset."""
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


# ══════════════════════════════════════════════════════════════════════════════
# Mutation Protocol Test Exports
# ══════════════════════════════════════════════════════════════════════════════


@export
fn mutation_buf_alloc(capacity: Int32) -> Int64:
    """Allocate a mutation buffer. Returns a pointer into WASM linear memory."""
    var ptr = UnsafePointer[UInt8].alloc(Int(capacity))
    return _ptr_to_i64(ptr)


@export
fn mutation_buf_free(ptr: Int64):
    """Free a previously allocated mutation buffer."""
    _int_to_ptr(Int(ptr)).free()


# ── Debug exports ────────────────────────────────────────────────────────────


@export
fn debug_ptr_roundtrip(ptr: Int64) -> Int64:
    """Check that _int_to_ptr round-trips correctly."""
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


# ── Simple opcodes ───────────────────────────────────────────────────────────


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
    """Write a known 5-mutation sequence for integration testing."""
    var w = MutationWriter(_int_to_ptr(Int(buf)), 0)
    w.load_template(1, 0, 10)
    w.create_text_node(11, String("hello"))
    w.append_children(10, 1)
    w.push_root(10)
    w.end()
    return Int32(w.offset)


# ══════════════════════════════════════════════════════════════════════════════
# Event Handler Registry Exports
# ══════════════════════════════════════════════════════════════════════════════


@export
fn handler_register_signal_add(
    rt_ptr: Int64,
    scope_id: Int32,
    signal_key: Int32,
    delta: Int32,
    event_name: String,
) -> Int32:
    """Register a handler that adds `delta` to `signal_key`.  Returns handler ID.
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
    """Register a handler that subtracts `delta` from `signal_key`.  Returns handler ID.
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
    """Register a handler that sets `signal_key` to `value`.  Returns handler ID.
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
    """Register a handler that toggles `signal_key` (0↔1).  Returns handler ID.
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
    """Register a handler that sets `signal_key` from event input.  Returns handler ID.
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
    """Register a custom handler.  Returns handler ID."""
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
    """Register a no-op handler.  Returns handler ID."""
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
    """Dispatch an event to a handler.  Returns 1 if action executed, 0 otherwise.
    """
    var rt = _get_runtime(rt_ptr)
    if rt[0].dispatch_event(UInt32(handler_id), UInt8(event_type)):
        return 1
    return 0


@export
fn dispatch_event_with_i32(
    rt_ptr: Int64, handler_id: Int32, event_type: Int32, value: Int32
) -> Int32:
    """Dispatch an event with an Int32 payload.  Returns 1 if action executed.
    """
    var rt = _get_runtime(rt_ptr)
    if rt[0].dispatch_event_with_i32(
        UInt32(handler_id), UInt8(event_type), value
    ):
        return 1
    return 0


@export
fn runtime_drain_dirty(rt_ptr: Int64) -> Int32:
    """Drain the dirty scope queue.  Returns the number of dirty scopes."""
    var rt = _get_runtime(rt_ptr)
    var dirty = rt[0].drain_dirty()
    return Int32(len(dirty))


# ══════════════════════════════════════════════════════════════════════════════
# Phase 8.3 — Context (Dependency Injection) Exports
# ══════════════════════════════════════════════════════════════════════════════


@export
fn ctx_provide(rt_ptr: Int64, scope_id: Int32, key: Int32, value: Int32):
    """Provide a context value at the given scope."""
    var rt = _get_runtime(rt_ptr)
    rt[0].scopes.provide_context(UInt32(scope_id), UInt32(key), value)


@export
fn ctx_consume(rt_ptr: Int64, scope_id: Int32, key: Int32) -> Int32:
    """Look up a context value by walking up the scope tree.  Returns 0 if not found.
    """
    var rt = _get_runtime(rt_ptr)
    var result = rt[0].scopes.consume_context(UInt32(scope_id), UInt32(key))
    return result[1]


@export
fn ctx_consume_found(rt_ptr: Int64, scope_id: Int32, key: Int32) -> Int32:
    """Check whether a context value exists.  Returns 1 if found, 0 if not."""
    var rt = _get_runtime(rt_ptr)
    var result = rt[0].scopes.consume_context(UInt32(scope_id), UInt32(key))
    if result[0]:
        return 1
    return 0


@export
fn ctx_has_local(rt_ptr: Int64, scope_id: Int32, key: Int32) -> Int32:
    """Check whether the scope itself provides a context for `key`.  Returns 1 or 0.
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
    """Remove a context entry.  Returns 1 if removed, 0 if not found."""
    var rt = _get_runtime(rt_ptr)
    if rt[0].scopes.remove_context(UInt32(scope_id), UInt32(key)):
        return 1
    return 0


# ══════════════════════════════════════════════════════════════════════════════
# Phase 8.4 — Error Boundaries Exports
# ══════════════════════════════════════════════════════════════════════════════


@export
fn err_set_boundary(rt_ptr: Int64, scope_id: Int32, enabled: Int32):
    """Mark or unmark a scope as an error boundary."""
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
    """Find the nearest error boundary ancestor.  Returns scope ID or -1."""
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].scopes.find_error_boundary(UInt32(scope_id)))


@export
fn err_propagate(rt_ptr: Int64, scope_id: Int32, message: String) -> Int32:
    """Propagate an error to its nearest error boundary.  Returns boundary ID or -1.
    """
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].scopes.propagate_error(UInt32(scope_id), message))


# ══════════════════════════════════════════════════════════════════════════════
# Phase 8.5 — Suspense Exports
# ══════════════════════════════════════════════════════════════════════════════


@export
fn suspense_set_boundary(rt_ptr: Int64, scope_id: Int32, enabled: Int32):
    """Mark or unmark a scope as a suspense boundary."""
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
    """Set the pending (async loading) state on a scope."""
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
    """Find the nearest suspense boundary ancestor.  Returns scope ID or -1."""
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
    """Mark a scope as no longer pending.  Returns suspense boundary ID or -1.
    """
    var rt = _get_runtime(rt_ptr)
    return Int32(rt[0].scopes.resolve_pending(UInt32(scope_id)))


# ══════════════════════════════════════════════════════════════════════════════
# Phase 7 — Counter App (End-to-End)
# ══════════════════════════════════════════════════════════════════════════════
#
# Thin @export wrappers calling into apps.counter module.


@export
fn counter_init() -> Int64:
    """Initialize the counter app.  Returns a pointer to the app state."""
    return Int64(Int(counter_app_init()))


@export
fn counter_destroy(app_ptr: Int64):
    """Destroy the counter app and free all resources."""
    counter_app_destroy(_int_to_counter_ptr(Int(app_ptr)))


@export
fn counter_rebuild(app_ptr: Int64, buf_ptr: Int64, capacity: Int32) -> Int32:
    """Initial render (mount) of the counter app.  Returns mutation byte length.
    """
    var app = _int_to_counter_ptr(Int(app_ptr))
    var buf = _int_to_ptr(Int(buf_ptr))

    var writer_ptr = UnsafePointer[MutationWriter].alloc(1)
    writer_ptr.init_pointee_move(MutationWriter(buf, Int(capacity)))

    var offset = counter_app_rebuild(app, writer_ptr)

    writer_ptr.destroy_pointee()
    writer_ptr.free()

    return offset


@export
fn counter_handle_event(
    app_ptr: Int64, handler_id: Int32, event_type: Int32
) -> Int32:
    """Dispatch an event to the counter app.  Returns 1 if action executed."""
    var app = _int_to_counter_ptr(Int(app_ptr))
    if counter_app_handle_event(app, UInt32(handler_id), UInt8(event_type)):
        return 1
    return 0


@export
fn counter_flush(app_ptr: Int64, buf_ptr: Int64, capacity: Int32) -> Int32:
    """Flush pending updates.  Returns mutation byte length, or 0 if nothing dirty.
    """
    var app = _int_to_counter_ptr(Int(app_ptr))
    var buf = _int_to_ptr(Int(buf_ptr))

    var writer_ptr = UnsafePointer[MutationWriter].alloc(1)
    writer_ptr.init_pointee_move(MutationWriter(buf, Int(capacity)))

    var offset = counter_app_flush(app, writer_ptr)

    writer_ptr.destroy_pointee()
    writer_ptr.free()

    return offset


# ── Counter App Query Exports ────────────────────────────────────────────────


@export
fn counter_rt_ptr(app_ptr: Int64) -> Int64:
    """Return the runtime pointer for JS template registration."""
    var app = _int_to_counter_ptr(Int(app_ptr))
    return _runtime_ptr_to_i64(app[0].shell.runtime)


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
    return app[0].shell.peek_signal_i32(app[0].count_signal)


@export
fn counter_has_dirty(app_ptr: Int64) -> Int32:
    """Check if the counter app has dirty scopes.  Returns 1 or 0."""
    var app = _int_to_counter_ptr(Int(app_ptr))
    if app[0].shell.has_dirty():
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


# ══════════════════════════════════════════════════════════════════════════════
# Phase 8 — Todo App
# ══════════════════════════════════════════════════════════════════════════════
#
# Thin @export wrappers calling into apps.todo module.


@export
fn todo_init() -> Int64:
    """Initialize the todo app.  Returns a pointer to the app state."""
    return Int64(Int(todo_app_init()))


@export
fn todo_destroy(app_ptr: Int64):
    """Destroy the todo app and free all resources."""
    todo_app_destroy(_int_to_todo_ptr(Int(app_ptr)))


@export
fn todo_rebuild(app_ptr: Int64, buf_ptr: Int64, capacity: Int32) -> Int32:
    """Initial render (mount) of the todo app.  Returns mutation byte length."""
    var app = _int_to_todo_ptr(Int(app_ptr))
    var buf = _int_to_ptr(Int(buf_ptr))

    var writer_ptr = UnsafePointer[MutationWriter].alloc(1)
    writer_ptr.init_pointee_move(MutationWriter(buf, Int(capacity)))

    var offset = todo_app_rebuild(app, writer_ptr)

    writer_ptr.destroy_pointee()
    writer_ptr.free()

    return offset


@export
fn todo_add_item(app_ptr: Int64, text: String):
    """Add a new item to the todo list."""
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
    """Flush pending updates.  Returns mutation byte length, or 0 if nothing dirty.
    """
    var app = _int_to_todo_ptr(Int(app_ptr))
    var buf = _int_to_ptr(Int(buf_ptr))

    var writer_ptr = UnsafePointer[MutationWriter].alloc(1)
    writer_ptr.init_pointee_move(MutationWriter(buf, Int(capacity)))

    var offset = todo_app_flush(app, writer_ptr)

    writer_ptr.destroy_pointee()
    writer_ptr.free()

    return offset


# ── Todo App Query Exports ───────────────────────────────────────────────────


@export
fn todo_app_template_id(app_ptr: Int64) -> Int32:
    """Return the app template ID."""
    var app = _int_to_todo_ptr(Int(app_ptr))
    return Int32(app[0].app_template_id)


@export
fn todo_item_template_id(app_ptr: Int64) -> Int32:
    """Return the item template ID."""
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
    if app[0].shell.has_dirty():
        return 1
    return 0


@export
fn todo_list_version(app_ptr: Int64) -> Int32:
    """Return the current list version signal value."""
    var app = _int_to_todo_ptr(Int(app_ptr))
    return app[0].shell.peek_signal_i32(app[0].list_version_signal)


@export
fn todo_scope_id(app_ptr: Int64) -> Int32:
    """Return the root scope ID."""
    var app = _int_to_todo_ptr(Int(app_ptr))
    return Int32(app[0].scope_id)


# ══════════════════════════════════════════════════════════════════════════════
# Phase 9 — Benchmark App
# ══════════════════════════════════════════════════════════════════════════════
#
# Thin @export wrappers calling into apps.bench module.


@export
fn bench_init() -> Int64:
    """Initialize the benchmark app.  Returns a pointer to the app state."""
    return Int64(Int(bench_app_init()))


@export
fn bench_destroy(app_ptr: Int64):
    """Destroy the benchmark app and free all resources."""
    bench_app_destroy(_int_to_bench_ptr(Int(app_ptr)))


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
    """Initial render of the benchmark table body.  Returns mutation byte length.
    """
    var app = _int_to_bench_ptr(Int(app_ptr))
    var buf = _int_to_ptr(Int(buf_ptr))

    var writer_ptr = UnsafePointer[MutationWriter].alloc(1)
    writer_ptr.init_pointee_move(MutationWriter(buf, Int(capacity)))

    var offset = bench_app_rebuild(app, writer_ptr)

    writer_ptr.destroy_pointee()
    writer_ptr.free()

    return offset


@export
fn bench_flush(app_ptr: Int64, buf_ptr: Int64, capacity: Int32) -> Int32:
    """Flush pending updates.  Returns mutation byte length, or 0 if nothing dirty.
    """
    var app = _int_to_bench_ptr(Int(app_ptr))
    var buf = _int_to_ptr(Int(buf_ptr))

    var writer_ptr = UnsafePointer[MutationWriter].alloc(1)
    writer_ptr.init_pointee_move(MutationWriter(buf, Int(capacity)))

    var offset = bench_app_flush(app, writer_ptr)

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
    return app[0].shell.peek_signal_i32(app[0].selected_signal)


@export
fn bench_has_dirty(app_ptr: Int64) -> Int32:
    """Check if the benchmark app has dirty scopes.  Returns 1 or 0."""
    var app = _int_to_bench_ptr(Int(app_ptr))
    if app[0].shell.has_dirty():
        return 1
    return 0


@export
fn bench_version(app_ptr: Int64) -> Int32:
    """Return the current version signal value."""
    var app = _int_to_bench_ptr(Int(app_ptr))
    return app[0].shell.peek_signal_i32(app[0].version_signal)


@export
fn bench_row_template_id(app_ptr: Int64) -> Int32:
    """Return the row template ID."""
    var app = _int_to_bench_ptr(Int(app_ptr))
    return Int32(app[0].row_template_id)


@export
fn bench_scope_id(app_ptr: Int64) -> Int32:
    """Return the root scope ID."""
    var app = _int_to_bench_ptr(Int(app_ptr))
    return Int32(app[0].scope_id)


# ══════════════════════════════════════════════════════════════════════════════
# Phase 9.4 — Signal Write Batching
# ══════════════════════════════════════════════════════════════════════════════


@export
fn runtime_begin_batch(rt_ptr: Int64):
    """Begin a signal write batch.

    While batching, signal writes still update values and accumulate
    dirty scopes, but no duplicate entries are added.  Call
    runtime_end_batch() to finalize.
    """
    # Batching is implicit — dirty_scopes already deduplicates.
    pass


@export
fn runtime_end_batch(rt_ptr: Int64):
    """End a signal write batch.  The dirty queue is ready for drain."""
    pass


# ══════════════════════════════════════════════════════════════════════════════
# Phase 9.5 — Debug Mode
# ══════════════════════════════════════════════════════════════════════════════


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


# ── Memory Management Test Exports ───────────────────────────────────────────


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
    """Write to a signal `count` times.  Returns final dirty_count."""
    var rt = _get_runtime(rt_ptr)
    for i in range(Int(count)):
        rt[0].write_signal[Int32](UInt32(key), Int32(i))
    return Int32(rt[0].dirty_count())


# ══════════════════════════════════════════════════════════════════════════════
# Phase 10.4 — Scheduler Exports
# ══════════════════════════════════════════════════════════════════════════════


@always_inline
fn _int_to_scheduler_ptr(addr: Int) -> UnsafePointer[Scheduler]:
    """Reinterpret an integer address as an UnsafePointer[Scheduler]."""
    var slot = UnsafePointer[Int].alloc(1)
    slot[0] = addr
    var result = slot.bitcast[UnsafePointer[Scheduler]]()[0]
    slot.free()
    return result


@export
fn scheduler_create() -> Int64:
    """Allocate a Scheduler on the heap.  Returns its pointer."""
    var ptr = UnsafePointer[Scheduler].alloc(1)
    ptr.init_pointee_move(Scheduler())
    return Int64(Int(ptr))


@export
fn scheduler_destroy(sched_ptr: Int64):
    """Destroy and free a heap-allocated Scheduler."""
    var ptr = _int_to_scheduler_ptr(Int(sched_ptr))
    ptr.destroy_pointee()
    ptr.free()


@export
fn scheduler_collect(sched_ptr: Int64, rt_ptr: Int64):
    """Drain the runtime's dirty queue into the scheduler."""
    var sched = _int_to_scheduler_ptr(Int(sched_ptr))
    var rt = _int_to_runtime_ptr(Int(rt_ptr))
    sched[0].collect(rt)


@export
fn scheduler_collect_one(sched_ptr: Int64, rt_ptr: Int64, scope_id: Int32):
    """Add a single scope to the scheduler queue."""
    var sched = _int_to_scheduler_ptr(Int(sched_ptr))
    var rt = _int_to_runtime_ptr(Int(rt_ptr))
    sched[0].collect_one(rt, UInt32(scope_id))


@export
fn scheduler_next(sched_ptr: Int64) -> Int32:
    """Return and remove the next scope to render (lowest height first)."""
    var sched = _int_to_scheduler_ptr(Int(sched_ptr))
    return Int32(sched[0].next())


@export
fn scheduler_is_empty(sched_ptr: Int64) -> Int32:
    """Check if the scheduler has no pending dirty scopes.  Returns 1 or 0."""
    var sched = _int_to_scheduler_ptr(Int(sched_ptr))
    if sched[0].is_empty():
        return 1
    return 0


@export
fn scheduler_count(sched_ptr: Int64) -> Int32:
    """Return the number of pending dirty scopes."""
    var sched = _int_to_scheduler_ptr(Int(sched_ptr))
    return Int32(sched[0].count())


@export
fn scheduler_has_scope(sched_ptr: Int64, scope_id: Int32) -> Int32:
    """Check if a scope is already in the scheduler queue.  Returns 1 or 0."""
    var sched = _int_to_scheduler_ptr(Int(sched_ptr))
    if sched[0].has_scope(UInt32(scope_id)):
        return 1
    return 0


@export
fn scheduler_clear(sched_ptr: Int64):
    """Discard all pending dirty scopes."""
    var sched = _int_to_scheduler_ptr(Int(sched_ptr))
    sched[0].clear()


# ══════════════════════════════════════════════════════════════════════════════
# Phase 10.4 — AppShell Exports
# ══════════════════════════════════════════════════════════════════════════════


@always_inline
fn _int_to_shell_ptr(addr: Int) -> UnsafePointer[AppShell]:
    """Reinterpret an integer address as an UnsafePointer[AppShell]."""
    var slot = UnsafePointer[Int].alloc(1)
    slot[0] = addr
    var result = slot.bitcast[UnsafePointer[AppShell]]()[0]
    slot.free()
    return result


@export
fn shell_create() -> Int64:
    """Create an AppShell with all subsystems allocated.  Returns its pointer.
    """
    var ptr = UnsafePointer[AppShell].alloc(1)
    ptr.init_pointee_move(app_shell_create())
    return Int64(Int(ptr))


@export
fn shell_destroy(shell_ptr: Int64):
    """Destroy an AppShell and free all resources."""
    var ptr = _int_to_shell_ptr(Int(shell_ptr))
    ptr[0].destroy()
    ptr.destroy_pointee()
    ptr.free()


@export
fn shell_is_alive(shell_ptr: Int64) -> Int32:
    """Check if the shell is alive.  Returns 1 or 0."""
    var ptr = _int_to_shell_ptr(Int(shell_ptr))
    if ptr[0].is_alive():
        return 1
    return 0


@export
fn shell_create_root_scope(shell_ptr: Int64) -> Int32:
    """Create a root scope via the AppShell.  Returns scope ID."""
    var ptr = _int_to_shell_ptr(Int(shell_ptr))
    return Int32(ptr[0].create_root_scope())


@export
fn shell_create_child_scope(shell_ptr: Int64, parent_id: Int32) -> Int32:
    """Create a child scope via the AppShell.  Returns scope ID."""
    var ptr = _int_to_shell_ptr(Int(shell_ptr))
    return Int32(ptr[0].create_child_scope(UInt32(parent_id)))


@export
fn shell_create_signal_i32(shell_ptr: Int64, initial: Int32) -> Int32:
    """Create an Int32 signal via the AppShell.  Returns signal key."""
    var ptr = _int_to_shell_ptr(Int(shell_ptr))
    return Int32(ptr[0].create_signal_i32(initial))


@export
fn shell_read_signal_i32(shell_ptr: Int64, key: Int32) -> Int32:
    """Read an Int32 signal via the AppShell (with context tracking)."""
    var ptr = _int_to_shell_ptr(Int(shell_ptr))
    return ptr[0].read_signal_i32(UInt32(key))


@export
fn shell_peek_signal_i32(shell_ptr: Int64, key: Int32) -> Int32:
    """Peek an Int32 signal via the AppShell (without subscribing)."""
    var ptr = _int_to_shell_ptr(Int(shell_ptr))
    return ptr[0].peek_signal_i32(UInt32(key))


@export
fn shell_write_signal_i32(shell_ptr: Int64, key: Int32, value: Int32):
    """Write to an Int32 signal via the AppShell."""
    var ptr = _int_to_shell_ptr(Int(shell_ptr))
    ptr[0].write_signal_i32(UInt32(key), value)


@export
fn shell_begin_render(shell_ptr: Int64, scope_id: Int32) -> Int32:
    """Begin rendering a scope.  Returns previous scope ID (or -1)."""
    var ptr = _int_to_shell_ptr(Int(shell_ptr))
    return Int32(ptr[0].begin_render(UInt32(scope_id)))


@export
fn shell_end_render(shell_ptr: Int64, prev_scope: Int32):
    """End rendering and restore the previous scope."""
    var ptr = _int_to_shell_ptr(Int(shell_ptr))
    ptr[0].end_render(Int(prev_scope))


@export
fn shell_has_dirty(shell_ptr: Int64) -> Int32:
    """Check if the shell has dirty scopes.  Returns 1 or 0."""
    var ptr = _int_to_shell_ptr(Int(shell_ptr))
    if ptr[0].has_dirty():
        return 1
    return 0


@export
fn shell_collect_dirty(shell_ptr: Int64):
    """Drain dirty scopes into the shell's scheduler."""
    var ptr = _int_to_shell_ptr(Int(shell_ptr))
    ptr[0].collect_dirty()


@export
fn shell_next_dirty(shell_ptr: Int64) -> Int32:
    """Return next dirty scope from the shell's scheduler."""
    var ptr = _int_to_shell_ptr(Int(shell_ptr))
    return Int32(ptr[0].next_dirty())


@export
fn shell_scheduler_empty(shell_ptr: Int64) -> Int32:
    """Check if the shell's scheduler is empty.  Returns 1 or 0."""
    var ptr = _int_to_shell_ptr(Int(shell_ptr))
    if ptr[0].scheduler_empty():
        return 1
    return 0


@export
fn shell_dispatch_event(
    shell_ptr: Int64, handler_id: Int32, event_type: Int32
) -> Int32:
    """Dispatch an event via the AppShell.  Returns 1 if executed."""
    var ptr = _int_to_shell_ptr(Int(shell_ptr))
    if ptr[0].dispatch_event(UInt32(handler_id), UInt8(event_type)):
        return 1
    return 0


@export
fn shell_rt_ptr(shell_ptr: Int64) -> Int64:
    """Return the runtime pointer from an AppShell (for template registration etc.).
    """
    var ptr = _int_to_shell_ptr(Int(shell_ptr))
    return _runtime_ptr_to_i64(ptr[0].runtime)


@export
fn shell_store_ptr(shell_ptr: Int64) -> Int64:
    """Return the VNodeStore pointer from an AppShell."""
    var ptr = _int_to_shell_ptr(Int(shell_ptr))
    return _vnode_store_ptr_to_i64(ptr[0].store)


@export
fn shell_eid_ptr(shell_ptr: Int64) -> Int64:
    """Return the ElementIdAllocator pointer from an AppShell."""
    var ptr = _int_to_shell_ptr(Int(shell_ptr))
    return Int64(Int(ptr[0].eid_alloc))


@export
fn shell_mount(
    shell_ptr: Int64, buf_ptr: Int64, capacity: Int32, vnode_index: Int32
) -> Int32:
    """Mount a VNode via the AppShell.  Returns mutation byte length."""
    var ptr = _int_to_shell_ptr(Int(shell_ptr))

    var writer_ptr = UnsafePointer[MutationWriter].alloc(1)
    writer_ptr.init_pointee_move(
        MutationWriter(_int_to_ptr(Int(buf_ptr)), Int(capacity))
    )

    var result = ptr[0].mount(writer_ptr, UInt32(vnode_index))

    writer_ptr.destroy_pointee()
    writer_ptr.free()

    return result


@export
fn shell_diff(
    shell_ptr: Int64,
    buf_ptr: Int64,
    capacity: Int32,
    old_index: Int32,
    new_index: Int32,
) -> Int32:
    """Diff two VNodes via the AppShell.  Returns mutation byte length."""
    var ptr = _int_to_shell_ptr(Int(shell_ptr))

    var writer_ptr = UnsafePointer[MutationWriter].alloc(1)
    writer_ptr.init_pointee_move(
        MutationWriter(_int_to_ptr(Int(buf_ptr)), Int(capacity))
    )

    ptr[0].diff(writer_ptr, UInt32(old_index), UInt32(new_index))
    var result = ptr[0].finalize(writer_ptr)

    writer_ptr.destroy_pointee()
    writer_ptr.free()

    return result


# ══════════════════════════════════════════════════════════════════════════════
# DSL Ergonomic Builder Exports (M10.5)
# ══════════════════════════════════════════════════════════════════════════════
#
# WASM-exported test functions for the declarative builder DSL.
# Each function exercises a specific aspect of the DSL and returns 1 for
# pass, 0 for fail.  The JS test harness calls these and asserts the result.
#
# Additionally, "dsl_node_*" and "dsl_vb_*" exports provide low-level
# building blocks that the Mojo test harness can orchestrate directly.


# ── Heap-allocated Node handle ───────────────────────────────────────────────


@always_inline
fn _int_to_node_ptr(addr: Int) -> UnsafePointer[Node]:
    """Reinterpret an integer address as an UnsafePointer[Node]."""
    var slot = UnsafePointer[Int].alloc(1)
    slot[0] = addr
    var result = slot.bitcast[UnsafePointer[Node]]()[0]
    slot.free()
    return result


@always_inline
fn _get_node(ptr: Int64) -> UnsafePointer[Node]:
    return _int_to_node_ptr(Int(ptr))


@export
fn dsl_node_text(s: String) -> Int64:
    """Create a text Node on the heap.  Returns a pointer handle."""
    var ptr = UnsafePointer[Node].alloc(1)
    ptr.init_pointee_move(text(s))
    return Int64(Int(ptr))


@export
fn dsl_node_dyn_text(index: Int32) -> Int64:
    """Create a dynamic text Node on the heap."""
    var ptr = UnsafePointer[Node].alloc(1)
    ptr.init_pointee_move(dyn_text(Int(index)))
    return Int64(Int(ptr))


@export
fn dsl_node_dyn_node(index: Int32) -> Int64:
    """Create a dynamic node placeholder on the heap."""
    var ptr = UnsafePointer[Node].alloc(1)
    ptr.init_pointee_move(dyn_node(Int(index)))
    return Int64(Int(ptr))


@export
fn dsl_node_attr(name: String, value: String) -> Int64:
    """Create a static attribute Node on the heap."""
    var ptr = UnsafePointer[Node].alloc(1)
    ptr.init_pointee_move(attr(name, value))
    return Int64(Int(ptr))


@export
fn dsl_node_dyn_attr(index: Int32) -> Int64:
    """Create a dynamic attribute Node on the heap."""
    var ptr = UnsafePointer[Node].alloc(1)
    ptr.init_pointee_move(dyn_attr(Int(index)))
    return Int64(Int(ptr))


@export
fn dsl_node_element(html_tag: Int32) -> Int64:
    """Create an empty element Node on the heap."""
    var ptr = UnsafePointer[Node].alloc(1)
    ptr.init_pointee_move(el_empty(UInt8(html_tag)))
    return Int64(Int(ptr))


@export
fn dsl_node_add_item(parent_ptr: Int64, child_ptr: Int64):
    """Add a child/attr Node to an element Node.

    The child Node is moved out of its heap slot (the child pointer
    becomes invalid after this call).
    """
    var parent = _get_node(parent_ptr)
    var child = _get_node(child_ptr)
    var child_val = child[0].copy()
    parent[0].add_item(child_val^)
    child.destroy_pointee()
    child.free()


@export
fn dsl_node_destroy(ptr: Int64):
    """Destroy and free a heap-allocated Node."""
    var node_ptr = _get_node(ptr)
    node_ptr.destroy_pointee()
    node_ptr.free()


@export
fn dsl_node_kind(ptr: Int64) -> Int32:
    """Return the kind tag of a Node."""
    return Int32(_get_node(ptr)[0].kind)


@export
fn dsl_node_tag(ptr: Int64) -> Int32:
    """Return the HTML tag of an element Node."""
    return Int32(_get_node(ptr)[0].tag)


@export
fn dsl_node_item_count(ptr: Int64) -> Int32:
    """Return the total item count (children + attrs) of an element Node."""
    return Int32(_get_node(ptr)[0].item_count())


@export
fn dsl_node_child_count(ptr: Int64) -> Int32:
    """Return the child count (excluding attrs) of an element Node."""
    return Int32(_get_node(ptr)[0].child_count())


@export
fn dsl_node_attr_count(ptr: Int64) -> Int32:
    """Return the attribute count (excluding children) of an element Node."""
    return Int32(_get_node(ptr)[0].attr_count())


@export
fn dsl_node_dynamic_index(ptr: Int64) -> Int32:
    """Return the dynamic_index of a DYN_TEXT/DYN_NODE/DYN_ATTR Node."""
    return Int32(_get_node(ptr)[0].dynamic_index)


@export
fn dsl_node_count_nodes(ptr: Int64) -> Int32:
    """Recursively count tree nodes (excluding attrs)."""
    return Int32(count_nodes(_get_node(ptr)[0]))


@export
fn dsl_node_count_all(ptr: Int64) -> Int32:
    """Recursively count all items (including attrs)."""
    return Int32(count_all_items(_get_node(ptr)[0]))


@export
fn dsl_node_count_dyn_text(ptr: Int64) -> Int32:
    """Count DYN_TEXT slots in the tree."""
    return Int32(count_dynamic_text_slots(_get_node(ptr)[0]))


@export
fn dsl_node_count_dyn_node(ptr: Int64) -> Int32:
    """Count DYN_NODE slots in the tree."""
    return Int32(count_dynamic_node_slots(_get_node(ptr)[0]))


@export
fn dsl_node_count_dyn_attr(ptr: Int64) -> Int32:
    """Count DYN_ATTR slots in the tree."""
    return Int32(count_dynamic_attr_slots(_get_node(ptr)[0]))


@export
fn dsl_node_count_static_attr(ptr: Int64) -> Int32:
    """Count STATIC_ATTR nodes in the tree."""
    return Int32(count_static_attr_nodes(_get_node(ptr)[0]))


# ── to_template via DSL ──────────────────────────────────────────────────────


@export
fn dsl_to_template(node_ptr: Int64, name: String, rt_ptr: Int64) -> Int32:
    """Convert a Node tree to a Template and register it.

    Args:
        node_ptr: Pointer to the root Node (consumed — freed after use).
        name: Template name for registration.
        rt_ptr: Pointer to the Runtime (owns TemplateRegistry).

    Returns:
        The template ID (UInt32 as Int32).
    """
    var node = _get_node(node_ptr)
    var rt = _get_runtime(rt_ptr)
    var template = to_template(node[0], name)
    var tmpl_id = rt[0].templates.register(template^)

    node.destroy_pointee()
    node.free()

    return Int32(tmpl_id)


# ── VNodeBuilder WASM exports ────────────────────────────────────────────────


@export
fn dsl_vb_create(tmpl_id: Int32, store_ptr: Int64) -> Int64:
    """Create a VNodeBuilder on the heap.

    Returns a pointer handle to the VNodeBuilder.
    """
    var store = _get_vnode_store(store_ptr)
    var ptr = UnsafePointer[VNodeBuilder].alloc(1)
    ptr.init_pointee_move(VNodeBuilder(UInt32(tmpl_id), store))
    return Int64(Int(ptr))


@export
fn dsl_vb_create_keyed(tmpl_id: Int32, key: String, store_ptr: Int64) -> Int64:
    """Create a keyed VNodeBuilder on the heap."""
    var store = _get_vnode_store(store_ptr)
    var ptr = UnsafePointer[VNodeBuilder].alloc(1)
    ptr.init_pointee_move(VNodeBuilder(UInt32(tmpl_id), key, store))
    return Int64(Int(ptr))


@always_inline
fn _int_to_vb_ptr(addr: Int) -> UnsafePointer[VNodeBuilder]:
    """Reinterpret an integer address as an UnsafePointer[VNodeBuilder]."""
    var slot = UnsafePointer[Int].alloc(1)
    slot[0] = addr
    var result = slot.bitcast[UnsafePointer[VNodeBuilder]]()[0]
    slot.free()
    return result


@always_inline
fn _get_vb(ptr: Int64) -> UnsafePointer[VNodeBuilder]:
    return _int_to_vb_ptr(Int(ptr))


@export
fn dsl_vb_destroy(ptr: Int64):
    """Destroy and free a heap-allocated VNodeBuilder."""
    var vb = _get_vb(ptr)
    vb.destroy_pointee()
    vb.free()


@export
fn dsl_vb_add_dyn_text(ptr: Int64, value: String):
    """Add a dynamic text node via the VNodeBuilder."""
    _get_vb(ptr)[0].add_dyn_text(value)


@export
fn dsl_vb_add_dyn_placeholder(ptr: Int64):
    """Add a dynamic placeholder via the VNodeBuilder."""
    _get_vb(ptr)[0].add_dyn_placeholder()


@export
fn dsl_vb_add_dyn_event(ptr: Int64, event_name: String, handler_id: Int32):
    """Add a dynamic event handler via the VNodeBuilder."""
    _get_vb(ptr)[0].add_dyn_event(event_name, UInt32(handler_id))


@export
fn dsl_vb_add_dyn_text_attr(ptr: Int64, name: String, value: String):
    """Add a dynamic text attribute via the VNodeBuilder."""
    _get_vb(ptr)[0].add_dyn_text_attr(name, value)


@export
fn dsl_vb_add_dyn_int_attr(ptr: Int64, name: String, value: Int64):
    """Add a dynamic integer attribute via the VNodeBuilder."""
    _get_vb(ptr)[0].add_dyn_int_attr(name, value)


@export
fn dsl_vb_add_dyn_bool_attr(ptr: Int64, name: String, value: Int32):
    """Add a dynamic boolean attribute via the VNodeBuilder."""
    _get_vb(ptr)[0].add_dyn_bool_attr(name, value != 0)


@export
fn dsl_vb_add_dyn_none_attr(ptr: Int64, name: String):
    """Add a dynamic none/removal attribute via the VNodeBuilder."""
    _get_vb(ptr)[0].add_dyn_none_attr(name)


@export
fn dsl_vb_index(ptr: Int64) -> Int32:
    """Return the VNode index from the VNodeBuilder."""
    return Int32(_get_vb(ptr)[0].index())


# ── Self-contained DSL tests ─────────────────────────────────────────────────
#
# Each returns 1 (pass) or 0 (fail).  These exercise the DSL end-to-end
# without requiring the test harness to assemble Node trees manually.


@export
fn dsl_test_text_node() -> Int32:
    """Test: text() creates a NODE_TEXT with correct content."""
    var n = text(String("hello"))
    if n.kind != NODE_TEXT:
        return 0
    if n.text != String("hello"):
        return 0
    if n.is_element():
        return 0
    if not n.is_text():
        return 0
    if not n.is_child():
        return 0
    if n.is_attr():
        return 0
    return 1


@export
fn dsl_test_dyn_text_node() -> Int32:
    """Test: dyn_text() creates a NODE_DYN_TEXT with correct index."""
    var n = dyn_text(3)
    if n.kind != NODE_DYN_TEXT:
        return 0
    if n.dynamic_index != 3:
        return 0
    if not n.is_dyn_text():
        return 0
    if not n.is_child():
        return 0
    return 1


@export
fn dsl_test_dyn_node_slot() -> Int32:
    """Test: dyn_node() creates a NODE_DYN_NODE with correct index."""
    var n = dyn_node(5)
    if n.kind != NODE_DYN_NODE:
        return 0
    if n.dynamic_index != 5:
        return 0
    if not n.is_dyn_node():
        return 0
    return 1


@export
fn dsl_test_static_attr() -> Int32:
    """Test: attr() creates a NODE_STATIC_ATTR with name and value."""
    var n = attr(String("class"), String("container"))
    if n.kind != NODE_STATIC_ATTR:
        return 0
    if n.text != String("class"):
        return 0
    if n.attr_value != String("container"):
        return 0
    if not n.is_attr():
        return 0
    if not n.is_static_attr():
        return 0
    if n.is_child():
        return 0
    return 1


@export
fn dsl_test_dyn_attr() -> Int32:
    """Test: dyn_attr() creates a NODE_DYN_ATTR with correct index."""
    var n = dyn_attr(2)
    if n.kind != NODE_DYN_ATTR:
        return 0
    if n.dynamic_index != 2:
        return 0
    if not n.is_dyn_attr():
        return 0
    if not n.is_attr():
        return 0
    return 1


@export
fn dsl_test_empty_element() -> Int32:
    """Test: el_div() with no args creates an empty element."""
    var n = el_div()
    if n.kind != NODE_ELEMENT:
        return 0
    if n.tag != TAG_DIV:
        return 0
    if n.item_count() != 0:
        return 0
    if n.child_count() != 0:
        return 0
    if n.attr_count() != 0:
        return 0
    return 1


@export
fn dsl_test_element_with_children() -> Int32:
    """Test: el_div with text children."""
    var n = el_div(List[Node](text(String("hello")), text(String("world"))))
    if n.kind != NODE_ELEMENT:
        return 0
    if n.tag != TAG_DIV:
        return 0
    if n.item_count() != 2:
        return 0
    if n.child_count() != 2:
        return 0
    if n.attr_count() != 0:
        return 0
    return 1


@export
fn dsl_test_element_with_attrs() -> Int32:
    """Test: el_div with attributes only."""
    var n = el_div(
        List[Node](
            attr(String("class"), String("box")),
            attr(String("id"), String("main")),
        )
    )
    if n.item_count() != 2:
        return 0
    if n.child_count() != 0:
        return 0
    if n.attr_count() != 2:
        return 0
    if n.static_attr_count() != 2:
        return 0
    return 1


@export
fn dsl_test_element_mixed() -> Int32:
    """Test: element with a mix of attrs, children, and dynamic slots."""
    var n = el_div(
        List[Node](
            attr(String("class"), String("counter")),
            dyn_attr(0),
            text(String("hello")),
            dyn_text(0),
            el_span(List[Node](text(String("inner")))),
        )
    )
    if n.item_count() != 5:
        return 0
    if n.child_count() != 3:
        return 0
    if n.attr_count() != 2:
        return 0
    if n.static_attr_count() != 1:
        return 0
    if n.dynamic_attr_count() != 1:
        return 0
    return 1


@export
fn dsl_test_nested_elements() -> Int32:
    """Test: deeply nested element tree."""
    var n = el_div(
        List[Node](
            el_h1(List[Node](text(String("Title")))),
            el_ul(
                List[Node](
                    el_li(List[Node](text(String("A")))),
                    el_li(List[Node](text(String("B")))),
                    el_li(List[Node](text(String("C")))),
                )
            ),
        )
    )
    if n.child_count() != 2:
        return 0
    # Total tree nodes: div + h1 + "Title" + ul + li*3 + "A" + "B" + "C" = 10
    if count_nodes(n) != 10:
        return 0
    return 1


@export
fn dsl_test_counter_template() -> Int32:
    """Test: build counter template via DSL and verify structure.

    Builds the same template as CounterApp does manually:
        div > [ span > dynamic_text[0],
                button > text("+") + dynamic_attr[0],
                button > text("-") + dynamic_attr[1] ]
    Then registers it and verifies template properties match.
    """
    # Build using DSL
    var view = el_div(
        List[Node](
            el_span(List[Node](dyn_text(0))),
            el_button(List[Node](text(String("+")), dyn_attr(0))),
            el_button(List[Node](text(String("-")), dyn_attr(1))),
        )
    )

    # Verify Node tree structure before template conversion
    # div(1) + span(1) + dyn_text(1) + button(1) + text("+")(1) + button(1) + text("-")(1) = 7
    # dyn_attr items are attrs, not children — count_nodes skips attrs.
    if count_nodes(view) != 7:
        return 0

    if count_dynamic_text_slots(view) != 1:
        return 0
    if count_dynamic_attr_slots(view) != 2:
        return 0

    # Convert to Template
    var rt_ptr = create_runtime()
    var template = to_template(view, String("dsl-counter"))
    var tmpl_id = rt_ptr[0].templates.register(template^)

    # Verify template properties
    # 1 root (the div)
    if rt_ptr[0].templates.root_count(tmpl_id) != 1:
        destroy_runtime(rt_ptr)
        return 0

    # 7 nodes total: div, span, dyn_text, btn1, text("+"), btn2, text("-")
    if rt_ptr[0].templates.node_count(tmpl_id) != 7:
        destroy_runtime(rt_ptr)
        return 0

    # Root node is an element (div)
    if rt_ptr[0].templates.node_kind(tmpl_id, 0) != TNODE_ELEMENT:
        destroy_runtime(rt_ptr)
        return 0

    # Root node tag is TAG_DIV
    if rt_ptr[0].templates.node_html_tag(tmpl_id, 0) != TAG_DIV:
        destroy_runtime(rt_ptr)
        return 0

    # Div has 3 children: span, button, button
    if rt_ptr[0].templates.node_child_count(tmpl_id, 0) != 3:
        destroy_runtime(rt_ptr)
        return 0

    # 1 dynamic text slot
    if rt_ptr[0].templates.dynamic_text_count(tmpl_id) != 1:
        destroy_runtime(rt_ptr)
        return 0

    # 2 dynamic attr slots
    if rt_ptr[0].templates.dynamic_attr_count(tmpl_id) != 2:
        destroy_runtime(rt_ptr)
        return 0

    destroy_runtime(rt_ptr)
    return 1


@export
fn dsl_test_to_template_simple() -> Int32:
    """Test: simple div with static text converts to valid template."""
    var view = el_div(List[Node](text(String("hello"))))
    var rt_ptr = create_runtime()
    var template = to_template(view, String("dsl-simple"))
    var tmpl_id = rt_ptr[0].templates.register(template^)

    # 2 nodes: div + text
    if rt_ptr[0].templates.node_count(tmpl_id) != 2:
        destroy_runtime(rt_ptr)
        return 0

    # 1 root
    if rt_ptr[0].templates.root_count(tmpl_id) != 1:
        destroy_runtime(rt_ptr)
        return 0

    # Root is element
    if rt_ptr[0].templates.node_kind(tmpl_id, 0) != TNODE_ELEMENT:
        destroy_runtime(rt_ptr)
        return 0

    # Child is text
    if rt_ptr[0].templates.node_kind(tmpl_id, 1) != TNODE_TEXT:
        destroy_runtime(rt_ptr)
        return 0

    destroy_runtime(rt_ptr)
    return 1


@export
fn dsl_test_to_template_attrs() -> Int32:
    """Test: element with static and dynamic attrs converts correctly."""
    var view = el_div(
        List[Node](
            attr(String("class"), String("box")),
            dyn_attr(0),
            text(String("content")),
        )
    )
    var rt_ptr = create_runtime()
    var template = to_template(view, String("dsl-attrs"))
    var tmpl_id = rt_ptr[0].templates.register(template^)

    # 2 nodes: div + text("content")
    if rt_ptr[0].templates.node_count(tmpl_id) != 2:
        destroy_runtime(rt_ptr)
        return 0

    # 1 static attr + 1 dynamic attr = 2 total attrs
    if rt_ptr[0].templates.attr_total_count(tmpl_id) != 2:
        destroy_runtime(rt_ptr)
        return 0

    if rt_ptr[0].templates.static_attr_count(tmpl_id) != 1:
        destroy_runtime(rt_ptr)
        return 0

    if rt_ptr[0].templates.dynamic_attr_count(tmpl_id) != 1:
        destroy_runtime(rt_ptr)
        return 0

    destroy_runtime(rt_ptr)
    return 1


@export
fn dsl_test_to_template_multi_root() -> Int32:
    """Test: multiple root nodes via to_template_multi."""
    var roots = List[Node](
        el_h1(List[Node](text(String("Title")))),
        el_p(List[Node](text(String("Body")))),
    )
    var rt_ptr = create_runtime()
    var template = to_template_multi(roots, String("dsl-multi"))
    var tmpl_id = rt_ptr[0].templates.register(template^)

    # 2 roots
    if rt_ptr[0].templates.root_count(tmpl_id) != 2:
        destroy_runtime(rt_ptr)
        return 0

    # 4 nodes: h1 + "Title" + p + "Body"
    if rt_ptr[0].templates.node_count(tmpl_id) != 4:
        destroy_runtime(rt_ptr)
        return 0

    destroy_runtime(rt_ptr)
    return 1


@export
fn dsl_test_vnode_builder() -> Int32:
    """Test: VNodeBuilder creates a VNode with correct dynamic content."""
    var rt_ptr = create_runtime()
    var store_ptr = UnsafePointer[VNodeStore].alloc(1)
    store_ptr.init_pointee_move(VNodeStore())

    # Register a template (we just need an ID)
    var view = el_div(List[Node](dyn_text(0), dyn_attr(0), dyn_attr(1)))
    var template = to_template(view, String("dsl-vb-test"))
    var tmpl_id = rt_ptr[0].templates.register(template^)

    # Build VNode using VNodeBuilder
    var vb = VNodeBuilder(tmpl_id, store_ptr)
    vb.add_dyn_text(String("Count: 42"))
    vb.add_dyn_event(String("click"), UInt32(10))
    vb.add_dyn_text_attr(String("class"), String("active"))
    var idx = vb.index()

    # Verify VNode
    if store_ptr[0].kind(idx) != VNODE_TEMPLATE_REF:
        store_ptr.destroy_pointee()
        store_ptr.free()
        destroy_runtime(rt_ptr)
        return 0

    if store_ptr[0].template_id(idx) != tmpl_id:
        store_ptr.destroy_pointee()
        store_ptr.free()
        destroy_runtime(rt_ptr)
        return 0

    # 1 dynamic text node
    if store_ptr[0].dynamic_node_count(idx) != 1:
        store_ptr.destroy_pointee()
        store_ptr.free()
        destroy_runtime(rt_ptr)
        return 0

    # 2 dynamic attrs (event + text attr)
    if store_ptr[0].dynamic_attr_count(idx) != 2:
        store_ptr.destroy_pointee()
        store_ptr.free()
        destroy_runtime(rt_ptr)
        return 0

    store_ptr.destroy_pointee()
    store_ptr.free()
    destroy_runtime(rt_ptr)
    return 1


@export
fn dsl_test_vnode_builder_keyed() -> Int32:
    """Test: keyed VNodeBuilder creates a keyed VNode."""
    var rt_ptr = create_runtime()
    var store_ptr = UnsafePointer[VNodeStore].alloc(1)
    store_ptr.init_pointee_move(VNodeStore())

    var view = el_div(List[Node](text(String("item"))))
    var template = to_template(view, String("dsl-keyed"))
    var tmpl_id = rt_ptr[0].templates.register(template^)

    var vb = VNodeBuilder(tmpl_id, String("item-42"), store_ptr)
    var idx = vb.index()

    if not store_ptr[0].has_key(idx):
        store_ptr.destroy_pointee()
        store_ptr.free()
        destroy_runtime(rt_ptr)
        return 0

    store_ptr.destroy_pointee()
    store_ptr.free()
    destroy_runtime(rt_ptr)
    return 1


@export
fn dsl_test_all_tag_helpers() -> Int32:
    """Test: every tag helper produces the correct tag constant."""
    # Layout / Sectioning
    if el_div().tag != TAG_DIV:
        return 0
    if el_span().tag != TAG_SPAN:
        return 0
    if el_p().tag != TAG_P:
        return 0
    if el_section().tag != TAG_SECTION:
        return 0
    if el_header().tag != TAG_HEADER:
        return 0
    if el_footer().tag != TAG_FOOTER:
        return 0
    if el_nav().tag != TAG_NAV:
        return 0
    if el_main().tag != TAG_MAIN:
        return 0
    if el_article().tag != TAG_ARTICLE:
        return 0
    if el_aside().tag != TAG_ASIDE:
        return 0
    # Headings
    if el_h1().tag != TAG_H1:
        return 0
    if el_h2().tag != TAG_H2:
        return 0
    if el_h3().tag != TAG_H3:
        return 0
    if el_h4().tag != TAG_H4:
        return 0
    if el_h5().tag != TAG_H5:
        return 0
    if el_h6().tag != TAG_H6:
        return 0
    # Lists
    if el_ul().tag != TAG_UL:
        return 0
    if el_ol().tag != TAG_OL:
        return 0
    if el_li().tag != TAG_LI:
        return 0
    # Interactive
    if el_button().tag != TAG_BUTTON:
        return 0
    if el_input().tag != TAG_INPUT:
        return 0
    if el_form().tag != TAG_FORM:
        return 0
    if el_textarea().tag != TAG_TEXTAREA:
        return 0
    if el_select().tag != TAG_SELECT:
        return 0
    if el_option().tag != TAG_OPTION:
        return 0
    if el_label().tag != TAG_LABEL:
        return 0
    # Links / Media
    if el_a().tag != TAG_A:
        return 0
    if el_img().tag != TAG_IMG:
        return 0
    # Table
    if el_table().tag != TAG_TABLE:
        return 0
    if el_thead().tag != TAG_THEAD:
        return 0
    if el_tbody().tag != TAG_TBODY:
        return 0
    if el_tr().tag != TAG_TR:
        return 0
    if el_td().tag != TAG_TD:
        return 0
    if el_th().tag != TAG_TH:
        return 0
    # Inline / Formatting
    if el_strong().tag != TAG_STRONG:
        return 0
    if el_em().tag != TAG_EM:
        return 0
    if el_br().tag != TAG_BR:
        return 0
    if el_hr().tag != TAG_HR:
        return 0
    if el_pre().tag != TAG_PRE:
        return 0
    if el_code().tag != TAG_CODE:
        return 0
    return 1


@export
fn dsl_test_count_utilities() -> Int32:
    """Test: count_* utility functions on a non-trivial tree."""
    var tree = el_div(
        List[Node](
            attr(String("class"), String("app")),
            dyn_attr(0),
            el_h1(List[Node](dyn_text(0))),
            el_ul(
                List[Node](
                    el_li(List[Node](text(String("A")), dyn_attr(1))),
                    el_li(List[Node](dyn_text(1), dyn_node(0))),
                )
            ),
        )
    )

    # Tree nodes (excluding attrs): div + h1 + dyn_text(0) + ul + li + "A" + li + dyn_text(1) + dyn_node(0) = 9
    if count_nodes(tree) != 9:
        return 0

    # DYN_TEXT slots: 2 (index 0 inside h1, index 1 inside second li)
    if count_dynamic_text_slots(tree) != 2:
        return 0

    # DYN_NODE slots: 1 (index 0 inside second li)
    if count_dynamic_node_slots(tree) != 1:
        return 0

    # DYN_ATTR slots: 2 (index 0 on div, index 1 on first li)
    if count_dynamic_attr_slots(tree) != 2:
        return 0

    # STATIC_ATTR: 1 (class on div)
    if count_static_attr_nodes(tree) != 1:
        return 0

    return 1


@export
fn dsl_test_template_equivalence() -> Int32:
    """Test: DSL-built template matches manually-built template.

    Builds the counter template both ways and verifies they have
    identical structure (node counts, kinds, tags, child counts,
    dynamic slot counts, attribute counts).
    """
    # ── Method 1: Manual builder (same as CounterApp) ────────────────
    var rt1 = create_runtime()
    var b = TemplateBuilder(String("manual-counter"))
    var div_idx = b.push_element(TAG_DIV, -1)
    var span_idx = b.push_element(TAG_SPAN, Int(div_idx))
    _ = b.push_dynamic_text(0, Int(span_idx))
    var btn1_idx = b.push_element(TAG_BUTTON, Int(div_idx))
    _ = b.push_text(String("+"), Int(btn1_idx))
    b.push_dynamic_attr(Int(btn1_idx), 0)
    var btn2_idx = b.push_element(TAG_BUTTON, Int(div_idx))
    _ = b.push_text(String("-"), Int(btn2_idx))
    b.push_dynamic_attr(Int(btn2_idx), 1)
    var manual_tmpl = b.build()
    var m_id = rt1[0].templates.register(manual_tmpl^)

    # ── Method 2: DSL builder ────────────────────────────────────────
    var rt2 = create_runtime()
    var view = el_div(
        List[Node](
            el_span(List[Node](dyn_text(0))),
            el_button(List[Node](text(String("+")), dyn_attr(0))),
            el_button(List[Node](text(String("-")), dyn_attr(1))),
        )
    )
    var dsl_tmpl = to_template(view, String("dsl-counter"))
    var d_id = rt2[0].templates.register(dsl_tmpl^)

    # ── Compare ──────────────────────────────────────────────────────

    # Node counts must match
    if rt1[0].templates.node_count(m_id) != rt2[0].templates.node_count(d_id):
        destroy_runtime(rt1)
        destroy_runtime(rt2)
        return 0

    # Root counts must match
    if rt1[0].templates.root_count(m_id) != rt2[0].templates.root_count(d_id):
        destroy_runtime(rt1)
        destroy_runtime(rt2)
        return 0

    # Dynamic text slot counts must match
    if rt1[0].templates.dynamic_text_count(m_id) != rt2[
        0
    ].templates.dynamic_text_count(d_id):
        destroy_runtime(rt1)
        destroy_runtime(rt2)
        return 0

    # Dynamic attr slot counts must match
    if rt1[0].templates.dynamic_attr_count(m_id) != rt2[
        0
    ].templates.dynamic_attr_count(d_id):
        destroy_runtime(rt1)
        destroy_runtime(rt2)
        return 0

    # Attr total counts must match
    if rt1[0].templates.attr_total_count(m_id) != rt2[
        0
    ].templates.attr_total_count(d_id):
        destroy_runtime(rt1)
        destroy_runtime(rt2)
        return 0

    # Compare each node kind and tag
    var node_count = rt1[0].templates.node_count(m_id)
    for i in range(node_count):
        if rt1[0].templates.node_kind(m_id, i) != rt2[0].templates.node_kind(
            d_id, i
        ):
            destroy_runtime(rt1)
            destroy_runtime(rt2)
            return 0
        if rt1[0].templates.node_html_tag(m_id, i) != rt2[
            0
        ].templates.node_html_tag(d_id, i):
            destroy_runtime(rt1)
            destroy_runtime(rt2)
            return 0
        if rt1[0].templates.node_child_count(m_id, i) != rt2[
            0
        ].templates.node_child_count(d_id, i):
            destroy_runtime(rt1)
            destroy_runtime(rt2)
            return 0

    destroy_runtime(rt1)
    destroy_runtime(rt2)
    return 1


# ══════════════════════════════════════════════════════════════════════════════
# Original wasm-mojo PoC Exports — Arithmetic, String, Algorithm Functions
# ══════════════════════════════════════════════════════════════════════════════
#
# Thin @export wrappers calling into poc/ package modules.


# ── Add ──────────────────────────────────────────────────────────────────────


@export
fn add_int32(x: Int32, y: Int32) -> Int32:
    return poc_add_int32(x, y)


@export
fn add_int64(x: Int64, y: Int64) -> Int64:
    return poc_add_int64(x, y)


@export
fn add_float32(x: Float32, y: Float32) -> Float32:
    return poc_add_float32(x, y)


@export
fn add_float64(x: Float64, y: Float64) -> Float64:
    return poc_add_float64(x, y)


# ── Subtract ─────────────────────────────────────────────────────────────────


@export
fn sub_int32(x: Int32, y: Int32) -> Int32:
    return poc_sub_int32(x, y)


@export
fn sub_int64(x: Int64, y: Int64) -> Int64:
    return poc_sub_int64(x, y)


@export
fn sub_float32(x: Float32, y: Float32) -> Float32:
    return poc_sub_float32(x, y)


@export
fn sub_float64(x: Float64, y: Float64) -> Float64:
    return poc_sub_float64(x, y)


# ── Multiply ─────────────────────────────────────────────────────────────────


@export
fn mul_int32(x: Int32, y: Int32) -> Int32:
    return poc_mul_int32(x, y)


@export
fn mul_int64(x: Int64, y: Int64) -> Int64:
    return poc_mul_int64(x, y)


@export
fn mul_float32(x: Float32, y: Float32) -> Float32:
    return poc_mul_float32(x, y)


@export
fn mul_float64(x: Float64, y: Float64) -> Float64:
    return poc_mul_float64(x, y)


# ── Division ─────────────────────────────────────────────────────────────────


@export
fn div_int32(x: Int32, y: Int32) -> Int32:
    return poc_div_int32(x, y)


@export
fn div_int64(x: Int64, y: Int64) -> Int64:
    return poc_div_int64(x, y)


@export
fn div_float32(x: Float32, y: Float32) -> Float32:
    return poc_div_float32(x, y)


@export
fn div_float64(x: Float64, y: Float64) -> Float64:
    return poc_div_float64(x, y)


# ── Modulo ───────────────────────────────────────────────────────────────────


@export
fn mod_int32(x: Int32, y: Int32) -> Int32:
    return poc_mod_int32(x, y)


@export
fn mod_int64(x: Int64, y: Int64) -> Int64:
    return poc_mod_int64(x, y)


# ── Power ────────────────────────────────────────────────────────────────────


@export
fn pow_int32(x: Int32) -> Int32:
    return poc_pow_int32(x)


@export
fn pow_int64(x: Int64) -> Int64:
    return poc_pow_int64(x)


@export
fn pow_float32(x: Float32) -> Float32:
    return poc_pow_float32(x)


@export
fn pow_float64(x: Float64) -> Float64:
    return poc_pow_float64(x)


# ── Negate ───────────────────────────────────────────────────────────────────


@export
fn neg_int32(x: Int32) -> Int32:
    return poc_neg_int32(x)


@export
fn neg_int64(x: Int64) -> Int64:
    return poc_neg_int64(x)


@export
fn neg_float32(x: Float32) -> Float32:
    return poc_neg_float32(x)


@export
fn neg_float64(x: Float64) -> Float64:
    return poc_neg_float64(x)


# ── Absolute value ───────────────────────────────────────────────────────────


@export
fn abs_int32(x: Int32) -> Int32:
    return poc_abs_int32(x)


@export
fn abs_int64(x: Int64) -> Int64:
    return poc_abs_int64(x)


@export
fn abs_float32(x: Float32) -> Float32:
    return poc_abs_float32(x)


@export
fn abs_float64(x: Float64) -> Float64:
    return poc_abs_float64(x)


# ── Min / Max ────────────────────────────────────────────────────────────────


@export
fn min_int32(x: Int32, y: Int32) -> Int32:
    return poc_min_int32(x, y)


@export
fn max_int32(x: Int32, y: Int32) -> Int32:
    return poc_max_int32(x, y)


@export
fn min_int64(x: Int64, y: Int64) -> Int64:
    return poc_min_int64(x, y)


@export
fn max_int64(x: Int64, y: Int64) -> Int64:
    return poc_max_int64(x, y)


@export
fn min_float64(x: Float64, y: Float64) -> Float64:
    return poc_min_float64(x, y)


@export
fn max_float64(x: Float64, y: Float64) -> Float64:
    return poc_max_float64(x, y)


# ── Clamp ────────────────────────────────────────────────────────────────────


@export
fn clamp_int32(x: Int32, lo: Int32, hi: Int32) -> Int32:
    return poc_clamp_int32(x, lo, hi)


@export
fn clamp_float64(x: Float64, lo: Float64, hi: Float64) -> Float64:
    return poc_clamp_float64(x, lo, hi)


# ── Bitwise operations ──────────────────────────────────────────────────────


@export
fn bitand_int32(x: Int32, y: Int32) -> Int32:
    return poc_bitand_int32(x, y)


@export
fn bitor_int32(x: Int32, y: Int32) -> Int32:
    return poc_bitor_int32(x, y)


@export
fn bitxor_int32(x: Int32, y: Int32) -> Int32:
    return poc_bitxor_int32(x, y)


@export
fn bitnot_int32(x: Int32) -> Int32:
    return poc_bitnot_int32(x)


@export
fn shl_int32(x: Int32, y: Int32) -> Int32:
    return poc_shl_int32(x, y)


@export
fn shr_int32(x: Int32, y: Int32) -> Int32:
    return poc_shr_int32(x, y)


# ── Boolean / comparison ─────────────────────────────────────────────────────


@export
fn eq_int32(x: Int32, y: Int32) -> Bool:
    return poc_eq_int32(x, y)


@export
fn ne_int32(x: Int32, y: Int32) -> Bool:
    return poc_ne_int32(x, y)


@export
fn lt_int32(x: Int32, y: Int32) -> Bool:
    return poc_lt_int32(x, y)


@export
fn le_int32(x: Int32, y: Int32) -> Bool:
    return poc_le_int32(x, y)


@export
fn gt_int32(x: Int32, y: Int32) -> Bool:
    return poc_gt_int32(x, y)


@export
fn ge_int32(x: Int32, y: Int32) -> Bool:
    return poc_ge_int32(x, y)


@export
fn bool_and(x: Bool, y: Bool) -> Bool:
    return poc_bool_and(x, y)


@export
fn bool_or(x: Bool, y: Bool) -> Bool:
    return poc_bool_or(x, y)


@export
fn bool_not(x: Bool) -> Bool:
    return poc_bool_not(x)


# ── Fibonacci (iterative) ───────────────────────────────────────────────────


@export
fn fib_int32(n: Int32) -> Int32:
    return poc_fib_int32(n)


@export
fn fib_int64(n: Int64) -> Int64:
    return poc_fib_int64(n)


# ── Factorial (iterative) ───────────────────────────────────────────────────


@export
fn factorial_int32(n: Int32) -> Int32:
    return poc_factorial_int32(n)


@export
fn factorial_int64(n: Int64) -> Int64:
    return poc_factorial_int64(n)


# ── GCD (Euclidean algorithm) ────────────────────────────────────────────────


@export
fn gcd_int32(x: Int32, y: Int32) -> Int32:
    return poc_gcd_int32(x, y)


# ── Identity / passthrough ──────────────────────────────────────────────────


@export
fn identity_int32(x: Int32) -> Int32:
    return poc_identity_int32(x)


@export
fn identity_int64(x: Int64) -> Int64:
    return poc_identity_int64(x)


@export
fn identity_float32(x: Float32) -> Float32:
    return poc_identity_float32(x)


@export
fn identity_float64(x: Float64) -> Float64:
    return poc_identity_float64(x)


# ── Print ────────────────────────────────────────────────────────────────────


@export
fn print_int32():
    poc_print_int32()


@export
fn print_int64():
    poc_print_int64()


@export
fn print_float32():
    poc_print_float32()


@export
fn print_float64():
    poc_print_float64()


@export
fn print_static_string():
    poc_print_static_string()


@export
fn print_input_string(input: String):
    poc_print_input_string(input)


# ── String I/O ───────────────────────────────────────────────────────────────


@export
fn return_input_string(x: String) -> String:
    return poc_return_input_string(x)


@export
fn return_static_string() -> String:
    return poc_return_static_string()


@export
fn string_length(x: String) -> Int64:
    return poc_string_length(x)


@export
fn string_concat(x: String, y: String) -> String:
    return poc_string_concat(x, y)


@export
fn string_repeat(x: String, n: Int32) -> String:
    return poc_string_repeat(x, n)


@export
fn string_eq(x: String, y: String) -> Bool:
    return poc_string_eq(x, y)

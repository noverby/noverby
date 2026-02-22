from bridge import MutationWriter
from arena import ElementId, ElementIdAllocator
from signals import Runtime, create_runtime, destroy_runtime
from mutations import CreateEngine, DiffEngine
from events import HandlerEntry
from vdom import (
    TemplateBuilder,
    create_builder,
    destroy_builder,
    VNode,
    VNodeStore,
    DynamicNode,
    DynamicAttr,
    AttributeValue,
    # DSL — Ergonomic builder API (M10.5)
    Node,
    text,
    dyn_text,
    dyn_node,
    attr,
    dyn_attr,
    el_empty,
    to_template,
    VNodeBuilder,
    count_nodes,
    count_all_items,
    count_dynamic_text_slots,
    count_dynamic_node_slots,
    count_dynamic_attr_slots,
    count_static_attr_nodes,
)
from vdom.dsl_tests import (
    test_text_node as _dsl_test_text_node,
    test_dyn_text_node as _dsl_test_dyn_text_node,
    test_dyn_node_slot as _dsl_test_dyn_node_slot,
    test_static_attr as _dsl_test_static_attr,
    test_dyn_attr as _dsl_test_dyn_attr,
    test_empty_element as _dsl_test_empty_element,
    test_element_with_children as _dsl_test_element_with_children,
    test_element_with_attrs as _dsl_test_element_with_attrs,
    test_element_mixed as _dsl_test_element_mixed,
    test_nested_elements as _dsl_test_nested_elements,
    test_counter_template as _dsl_test_counter_template,
    test_to_template_simple as _dsl_test_to_template_simple,
    test_to_template_attrs as _dsl_test_to_template_attrs,
    test_to_template_multi_root as _dsl_test_to_template_multi_root,
    test_vnode_builder as _dsl_test_vnode_builder,
    test_vnode_builder_keyed as _dsl_test_vnode_builder_keyed,
    test_all_tag_helpers as _dsl_test_all_tag_helpers,
    test_count_utilities as _dsl_test_count_utilities,
    test_template_equivalence as _dsl_test_template_equivalence,
)
from scheduler import Scheduler
from component import AppShell, app_shell_create
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
    todo_app_init,
    todo_app_destroy,
    todo_app_rebuild,
    todo_app_flush,
    BenchmarkApp,
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
#
# Generic helpers — one function replaces all type-specific variants.


@always_inline
fn _as_ptr[T: AnyType](addr: Int) -> UnsafePointer[T]:
    """Reinterpret an integer address as an UnsafePointer[T]."""
    var slot = UnsafePointer[Int].alloc(1)
    slot[0] = addr
    var result = slot.bitcast[UnsafePointer[T]]()[0]
    slot.free()
    return result


@always_inline
fn _to_i64[T: AnyType](ptr: UnsafePointer[T]) -> Int64:
    """Return the raw address of a typed pointer as Int64."""
    return Int64(Int(ptr))


# ── Helper: generic heap alloc/free ─────────────────────────────────────────


@always_inline
fn _heap_new[T: Movable](var val: T) -> UnsafePointer[T]:
    """Allocate a single T on the heap and move val into it."""
    var ptr = UnsafePointer[T].alloc(1)
    ptr.init_pointee_move(val^)
    return ptr


@always_inline
fn _heap_del[T: Movable](ptr: UnsafePointer[T]):
    """Destroy and free a single heap-allocated T."""
    ptr.destroy_pointee()
    ptr.free()


# ── Helper: generic get-pointer wrapper ──────────────────────────────────────


@always_inline
fn _get[T: AnyType](ptr: Int64) -> UnsafePointer[T]:
    """Reinterpret an Int64 WASM handle as an UnsafePointer[T]."""
    return _as_ptr[T](Int(ptr))


# ── Helper: Bool → Int32 for WASM ABI ───────────────────────────────────────


@always_inline
fn _b2i(val: Bool) -> Int32:
    """Convert a Bool to Int32 (1 or 0) for WASM export returns."""
    if val:
        return 1
    return 0


# ── Helper: writer at (buf, off) ────────────────────────────────────────────


@always_inline
fn _writer(buf: Int64, off: Int32) -> MutationWriter:
    return MutationWriter(_get[UInt8](buf), Int(off), 0)


# ── Helper: heap-allocated MutationWriter for app rebuild/flush ──────────────


@always_inline
fn _alloc_writer(
    buf_ptr: Int64, capacity: Int32
) -> UnsafePointer[MutationWriter]:
    """Allocate a MutationWriter on the heap with the given buffer and capacity.
    """
    return _heap_new(MutationWriter(_get[UInt8](buf_ptr), Int(capacity)))


@always_inline
fn _free_writer(ptr: UnsafePointer[MutationWriter]):
    """Destroy and free a heap-allocated MutationWriter."""
    _heap_del(ptr)


# ══════════════════════════════════════════════════════════════════════════════
# ElementId Allocator Test Exports
# ══════════════════════════════════════════════════════════════════════════════


@export
fn eid_alloc_create() -> Int64:
    """Allocate an ElementIdAllocator on the heap."""
    return _to_i64(_heap_new(ElementIdAllocator()))


@export
fn eid_alloc_destroy(alloc_ptr: Int64):
    """Destroy and free a heap-allocated ElementIdAllocator."""
    _heap_del(_get[ElementIdAllocator](alloc_ptr))


@export
fn eid_alloc(alloc_ptr: Int64) -> Int32:
    """Allocate a new ElementId.  Returns the raw u32 as i32."""
    return Int32(_get[ElementIdAllocator](alloc_ptr)[0].alloc().as_u32())


@export
fn eid_free(alloc_ptr: Int64, id: Int32):
    """Free an ElementId."""
    _get[ElementIdAllocator](alloc_ptr)[0].free(ElementId(UInt32(id)))


@export
fn eid_is_alive(alloc_ptr: Int64, id: Int32) -> Int32:
    """Check whether an ElementId is currently allocated.  Returns 1 or 0."""
    return _b2i(
        _get[ElementIdAllocator](alloc_ptr)[0].is_alive(ElementId(UInt32(id)))
    )


@export
fn eid_count(alloc_ptr: Int64) -> Int32:
    """Number of allocated IDs (including root)."""
    return Int32(_get[ElementIdAllocator](alloc_ptr)[0].count())


@export
fn eid_user_count(alloc_ptr: Int64) -> Int32:
    """Number of user-allocated IDs (excluding root)."""
    return Int32(_get[ElementIdAllocator](alloc_ptr)[0].user_count())


# ══════════════════════════════════════════════════════════════════════════════
# Runtime / Signals Test Exports
# ══════════════════════════════════════════════════════════════════════════════


@export
fn runtime_create() -> Int64:
    """Allocate a reactive Runtime on the heap."""
    return _to_i64(create_runtime())


@export
fn runtime_destroy(rt_ptr: Int64):
    """Destroy and free a heap-allocated Runtime."""
    destroy_runtime(_get[Runtime](rt_ptr))


@export
fn signal_create_i32(rt_ptr: Int64, initial: Int32) -> Int32:
    """Create an Int32 signal.  Returns its key."""
    return Int32(_get[Runtime](rt_ptr)[0].create_signal[Int32](initial))


@export
fn signal_read_i32(rt_ptr: Int64, key: Int32) -> Int32:
    """Read an Int32 signal (with context tracking)."""
    return _get[Runtime](rt_ptr)[0].read_signal[Int32](UInt32(key))


@export
fn signal_write_i32(rt_ptr: Int64, key: Int32, value: Int32):
    """Write a new value to an Int32 signal."""
    _get[Runtime](rt_ptr)[0].write_signal[Int32](UInt32(key), value)


@export
fn signal_peek_i32(rt_ptr: Int64, key: Int32) -> Int32:
    """Read an Int32 signal without subscribing."""
    return _get[Runtime](rt_ptr)[0].peek_signal[Int32](UInt32(key))


@export
fn signal_destroy(rt_ptr: Int64, key: Int32):
    """Destroy a signal."""
    _get[Runtime](rt_ptr)[0].destroy_signal(UInt32(key))


@export
fn signal_subscriber_count(rt_ptr: Int64, key: Int32) -> Int32:
    """Return the number of subscribers for a signal."""
    return Int32(_get[Runtime](rt_ptr)[0].signals.subscriber_count(UInt32(key)))


@export
fn signal_version(rt_ptr: Int64, key: Int32) -> Int32:
    """Return the write-version counter for a signal."""
    return Int32(_get[Runtime](rt_ptr)[0].signals.version(UInt32(key)))


@export
fn signal_count(rt_ptr: Int64) -> Int32:
    """Return the number of live signals."""
    return Int32(_get[Runtime](rt_ptr)[0].signals.signal_count())


@export
fn signal_contains(rt_ptr: Int64, key: Int32) -> Int32:
    """Check whether a signal key is live.  Returns 1 or 0."""
    return _b2i(_get[Runtime](rt_ptr)[0].signals.contains(UInt32(key)))


# ── Context management exports ───────────────────────────────────────────────


@export
fn runtime_set_context(rt_ptr: Int64, context_id: Int32):
    """Set the current reactive context."""
    _get[Runtime](rt_ptr)[0].set_context(UInt32(context_id))


@export
fn runtime_clear_context(rt_ptr: Int64):
    """Clear the current reactive context."""
    _get[Runtime](rt_ptr)[0].clear_context()


@export
fn runtime_has_context(rt_ptr: Int64) -> Int32:
    """Check if a reactive context is active.  Returns 1 or 0."""
    return _b2i(_get[Runtime](rt_ptr)[0].has_context())


@export
fn runtime_dirty_count(rt_ptr: Int64) -> Int32:
    """Return the number of dirty scopes."""
    return Int32(_get[Runtime](rt_ptr)[0].dirty_count())


@export
fn runtime_has_dirty(rt_ptr: Int64) -> Int32:
    """Check if any scopes are dirty.  Returns 1 or 0."""
    return _b2i(_get[Runtime](rt_ptr)[0].has_dirty())


# ══════════════════════════════════════════════════════════════════════════════
# Scope Management Exports
# ══════════════════════════════════════════════════════════════════════════════


@export
fn scope_create(rt_ptr: Int64, height: Int32, parent_id: Int32) -> Int32:
    """Create a new scope.  Returns its ID."""
    return Int32(
        _get[Runtime](rt_ptr)[0].create_scope(UInt32(height), Int(parent_id))
    )


@export
fn scope_create_child(rt_ptr: Int64, parent_id: Int32) -> Int32:
    """Create a child scope.  Height is parent.height + 1."""
    return Int32(_get[Runtime](rt_ptr)[0].create_child_scope(UInt32(parent_id)))


@export
fn scope_destroy(rt_ptr: Int64, id: Int32):
    """Destroy a scope."""
    _get[Runtime](rt_ptr)[0].destroy_scope(UInt32(id))


@export
fn scope_count(rt_ptr: Int64) -> Int32:
    """Return the number of live scopes."""
    return Int32(_get[Runtime](rt_ptr)[0].scope_count())


@export
fn scope_contains(rt_ptr: Int64, id: Int32) -> Int32:
    """Check whether a scope ID is live.  Returns 1 or 0."""
    return _b2i(_get[Runtime](rt_ptr)[0].scope_contains(UInt32(id)))


@export
fn scope_height(rt_ptr: Int64, id: Int32) -> Int32:
    """Return the height (depth) of a scope."""
    return Int32(_get[Runtime](rt_ptr)[0].scopes.height(UInt32(id)))


@export
fn scope_parent(rt_ptr: Int64, id: Int32) -> Int32:
    """Return the parent ID of a scope, or -1 if root."""
    return Int32(_get[Runtime](rt_ptr)[0].scopes.parent_id(UInt32(id)))


@export
fn scope_is_dirty(rt_ptr: Int64, id: Int32) -> Int32:
    """Check whether a scope is dirty.  Returns 1 or 0."""
    return _b2i(_get[Runtime](rt_ptr)[0].scopes.is_dirty(UInt32(id)))


@export
fn scope_set_dirty(rt_ptr: Int64, id: Int32, dirty: Int32):
    """Set the dirty flag on a scope."""
    _get[Runtime](rt_ptr)[0].scopes.set_dirty(UInt32(id), dirty != 0)


@export
fn scope_render_count(rt_ptr: Int64, id: Int32) -> Int32:
    """Return how many times a scope has been rendered."""
    return Int32(_get[Runtime](rt_ptr)[0].scopes.render_count(UInt32(id)))


@export
fn scope_hook_count(rt_ptr: Int64, id: Int32) -> Int32:
    """Return the number of hooks in a scope."""
    return Int32(_get[Runtime](rt_ptr)[0].scopes.hook_count(UInt32(id)))


@export
fn scope_hook_value_at(rt_ptr: Int64, id: Int32, index: Int32) -> Int32:
    """Return the hook value (signal key) at position `index` in scope `id`."""
    return Int32(
        _get[Runtime](rt_ptr)[0].scopes.hook_value_at(UInt32(id), Int(index))
    )


@export
fn scope_hook_tag_at(rt_ptr: Int64, id: Int32, index: Int32) -> Int32:
    """Return the hook tag at position `index` in scope `id`."""
    return Int32(
        _get[Runtime](rt_ptr)[0].scopes.hook_tag_at(UInt32(id), Int(index))
    )


# ── Scope rendering lifecycle exports ────────────────────────────────────────


@export
fn scope_begin_render(rt_ptr: Int64, scope_id: Int32) -> Int32:
    """Begin rendering a scope.  Returns the previous scope ID (or -1)."""
    return Int32(_get[Runtime](rt_ptr)[0].begin_scope_render(UInt32(scope_id)))


@export
fn scope_end_render(rt_ptr: Int64, prev_scope: Int32):
    """End rendering the current scope and restore the previous scope."""
    _get[Runtime](rt_ptr)[0].end_scope_render(Int(prev_scope))


@export
fn scope_has_scope(rt_ptr: Int64) -> Int32:
    """Check if a scope is currently active.  Returns 1 or 0."""
    return _b2i(_get[Runtime](rt_ptr)[0].has_scope())


@export
fn scope_get_current(rt_ptr: Int64) -> Int32:
    """Return the current scope ID, or -1 if none."""
    var rt = _get[Runtime](rt_ptr)
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
    return Int32(_get[Runtime](rt_ptr)[0].use_signal_i32(initial))


@export
fn hook_use_memo_i32(rt_ptr: Int64, initial: Int32) -> Int32:
    """Hook: create or retrieve an Int32 memo for the current scope.

    On first render: creates memo, stores in hooks (HOOK_MEMO tag), returns ID.
    On re-render: returns existing memo ID (initial ignored).
    """
    return Int32(_get[Runtime](rt_ptr)[0].use_memo_i32(initial))


@export
fn hook_use_effect(rt_ptr: Int64) -> Int32:
    """Hook: create or retrieve an effect for the current scope.

    On first render: creates effect, stores in hooks (HOOK_EFFECT tag), returns ID.
    On re-render: returns existing effect ID.
    """
    return Int32(_get[Runtime](rt_ptr)[0].use_effect())


@export
fn scope_is_first_render(rt_ptr: Int64, scope_id: Int32) -> Int32:
    """Check if a scope is on its first render.  Returns 1 or 0."""
    return _b2i(
        _get[Runtime](rt_ptr)[0].scopes.is_first_render(UInt32(scope_id))
    )


# ══════════════════════════════════════════════════════════════════════════════
# Template Builder Exports
# ══════════════════════════════════════════════════════════════════════════════


@export
fn tmpl_builder_create(name: String) -> Int64:
    """Create a heap-allocated TemplateBuilder.  Returns its pointer."""
    return _to_i64(create_builder(name))


@export
fn tmpl_builder_destroy(ptr: Int64):
    """Destroy and free a heap-allocated TemplateBuilder."""
    destroy_builder(_get[TemplateBuilder](ptr))


@export
fn tmpl_builder_push_element(
    ptr: Int64, html_tag: Int32, parent: Int32
) -> Int32:
    """Add an Element node.  parent=-1 means root.  Returns node index."""
    return Int32(
        _get[TemplateBuilder](ptr)[0].push_element(UInt8(html_tag), Int(parent))
    )


@export
fn tmpl_builder_push_text(ptr: Int64, text: String, parent: Int32) -> Int32:
    """Add a static Text node.  Returns node index."""
    return Int32(_get[TemplateBuilder](ptr)[0].push_text(text, Int(parent)))


@export
fn tmpl_builder_push_dynamic(
    ptr: Int64, dynamic_index: Int32, parent: Int32
) -> Int32:
    """Add a Dynamic node placeholder.  Returns node index."""
    return Int32(
        _get[TemplateBuilder](ptr)[0].push_dynamic(
            UInt32(dynamic_index), Int(parent)
        )
    )


@export
fn tmpl_builder_push_dynamic_text(
    ptr: Int64, dynamic_index: Int32, parent: Int32
) -> Int32:
    """Add a DynamicText node placeholder.  Returns node index."""
    return Int32(
        _get[TemplateBuilder](ptr)[0].push_dynamic_text(
            UInt32(dynamic_index), Int(parent)
        )
    )


@export
fn tmpl_builder_push_static_attr(
    ptr: Int64, node_index: Int32, name: String, value: String
):
    """Add a static attribute to the specified node."""
    _get[TemplateBuilder](ptr)[0].push_static_attr(Int(node_index), name, value)


@export
fn tmpl_builder_push_dynamic_attr(
    ptr: Int64, node_index: Int32, dynamic_index: Int32
):
    """Add a dynamic attribute placeholder to the specified node."""
    _get[TemplateBuilder](ptr)[0].push_dynamic_attr(
        Int(node_index), UInt32(dynamic_index)
    )


@export
fn tmpl_builder_node_count(ptr: Int64) -> Int32:
    """Return the number of nodes in the builder."""
    return Int32(_get[TemplateBuilder](ptr)[0].node_count())


@export
fn tmpl_builder_root_count(ptr: Int64) -> Int32:
    """Return the number of root nodes in the builder."""
    return Int32(_get[TemplateBuilder](ptr)[0].root_count())


@export
fn tmpl_builder_attr_count(ptr: Int64) -> Int32:
    """Return the total number of attributes in the builder."""
    return Int32(_get[TemplateBuilder](ptr)[0].attr_count())


@export
fn tmpl_builder_register(rt_ptr: Int64, builder_ptr: Int64) -> Int32:
    """Build the template and register it in the runtime.  Returns template ID.
    """
    var template = _get[TemplateBuilder](builder_ptr)[0].build()
    return Int32(_get[Runtime](rt_ptr)[0].templates.register(template^))


# ── Template Registry Query Exports ──────────────────────────────────────────


@export
fn tmpl_count(rt_ptr: Int64) -> Int32:
    """Return the number of registered templates."""
    return Int32(_get[Runtime](rt_ptr)[0].templates.count())


@export
fn tmpl_root_count(rt_ptr: Int64, tmpl_id: Int32) -> Int32:
    """Return the number of root nodes in the template."""
    return Int32(_get[Runtime](rt_ptr)[0].templates.root_count(UInt32(tmpl_id)))


@export
fn tmpl_node_count(rt_ptr: Int64, tmpl_id: Int32) -> Int32:
    """Return the total number of nodes in the template."""
    return Int32(_get[Runtime](rt_ptr)[0].templates.node_count(UInt32(tmpl_id)))


@export
fn tmpl_node_kind(rt_ptr: Int64, tmpl_id: Int32, node_idx: Int32) -> Int32:
    """Return the kind tag (TNODE_*) of the node at node_idx."""
    return Int32(
        _get[Runtime](rt_ptr)[0].templates.node_kind(
            UInt32(tmpl_id), Int(node_idx)
        )
    )


@export
fn tmpl_node_tag(rt_ptr: Int64, tmpl_id: Int32, node_idx: Int32) -> Int32:
    """Return the HTML tag constant of the Element node at node_idx."""
    return Int32(
        _get[Runtime](rt_ptr)[0].templates.node_html_tag(
            UInt32(tmpl_id), Int(node_idx)
        )
    )


@export
fn tmpl_node_child_count(
    rt_ptr: Int64, tmpl_id: Int32, node_idx: Int32
) -> Int32:
    """Return the number of children of the node at node_idx."""
    return Int32(
        _get[Runtime](rt_ptr)[0].templates.node_child_count(
            UInt32(tmpl_id), Int(node_idx)
        )
    )


@export
fn tmpl_node_child_at(
    rt_ptr: Int64, tmpl_id: Int32, node_idx: Int32, child_pos: Int32
) -> Int32:
    """Return the node index of the child at child_pos within node_idx."""
    return Int32(
        _get[Runtime](rt_ptr)[0].templates.node_child_at(
            UInt32(tmpl_id), Int(node_idx), Int(child_pos)
        )
    )


@export
fn tmpl_node_dynamic_index(
    rt_ptr: Int64, tmpl_id: Int32, node_idx: Int32
) -> Int32:
    """Return the dynamic slot index of the node at node_idx."""
    return Int32(
        _get[Runtime](rt_ptr)[0].templates.node_dynamic_index(
            UInt32(tmpl_id), Int(node_idx)
        )
    )


@export
fn tmpl_node_attr_count(
    rt_ptr: Int64, tmpl_id: Int32, node_idx: Int32
) -> Int32:
    """Return the number of attributes on the node at node_idx."""
    return Int32(
        _get[Runtime](rt_ptr)[0].templates.node_attr_count(
            UInt32(tmpl_id), Int(node_idx)
        )
    )


@export
fn tmpl_attr_total_count(rt_ptr: Int64, tmpl_id: Int32) -> Int32:
    """Return the total number of attributes in the template."""
    return Int32(
        _get[Runtime](rt_ptr)[0].templates.attr_total_count(UInt32(tmpl_id))
    )


@export
fn tmpl_get_root_index(rt_ptr: Int64, tmpl_id: Int32, root_pos: Int32) -> Int32:
    """Return the node index of the root at position root_pos."""
    return Int32(
        _get[Runtime](rt_ptr)[0].templates.get_root_index(
            UInt32(tmpl_id), Int(root_pos)
        )
    )


@export
fn tmpl_attr_kind(rt_ptr: Int64, tmpl_id: Int32, attr_idx: Int32) -> Int32:
    """Return the kind (TATTR_*) of the attribute at attr_idx."""
    return Int32(
        _get[Runtime](rt_ptr)[0].templates.get_attr_kind(
            UInt32(tmpl_id), Int(attr_idx)
        )
    )


@export
fn tmpl_attr_dynamic_index(
    rt_ptr: Int64, tmpl_id: Int32, attr_idx: Int32
) -> Int32:
    """Return the dynamic index of the attribute at attr_idx."""
    return Int32(
        _get[Runtime](rt_ptr)[0].templates.get_attr_dynamic_index(
            UInt32(tmpl_id), Int(attr_idx)
        )
    )


@export
fn tmpl_dynamic_node_count(rt_ptr: Int64, tmpl_id: Int32) -> Int32:
    """Return the number of Dynamic node slots in the template."""
    return Int32(
        _get[Runtime](rt_ptr)[0].templates.dynamic_node_count(UInt32(tmpl_id))
    )


@export
fn tmpl_dynamic_text_count(rt_ptr: Int64, tmpl_id: Int32) -> Int32:
    """Return the number of DynamicText slots in the template."""
    return Int32(
        _get[Runtime](rt_ptr)[0].templates.dynamic_text_count(UInt32(tmpl_id))
    )


@export
fn tmpl_dynamic_attr_count(rt_ptr: Int64, tmpl_id: Int32) -> Int32:
    """Return the number of dynamic attribute slots in the template."""
    return Int32(
        _get[Runtime](rt_ptr)[0].templates.dynamic_attr_count(UInt32(tmpl_id))
    )


@export
fn tmpl_static_attr_count(rt_ptr: Int64, tmpl_id: Int32) -> Int32:
    """Return the number of static attributes in the template."""
    return Int32(
        _get[Runtime](rt_ptr)[0].templates.static_attr_count(UInt32(tmpl_id))
    )


@export
fn tmpl_contains_name(rt_ptr: Int64, name: String) -> Int32:
    """Check if a template with the given name is registered.  Returns 1 or 0.
    """
    return _b2i(_get[Runtime](rt_ptr)[0].templates.contains_name(name))


@export
fn tmpl_find_by_name(rt_ptr: Int64, name: String) -> Int32:
    """Find a template by name.  Returns ID or -1 if not found."""
    return Int32(_get[Runtime](rt_ptr)[0].templates.find_by_name(name))


@export
fn tmpl_node_first_attr(
    rt_ptr: Int64, tmpl_id: Int32, node_idx: Int32
) -> Int32:
    """Return the first attribute index of the node at node_idx."""
    return Int32(
        _get[Runtime](rt_ptr)[0].templates.node_first_attr(
            UInt32(tmpl_id), Int(node_idx)
        )
    )


@export
fn tmpl_node_text(rt_ptr: Int64, tmpl_id: Int32, node_idx: Int32) -> String:
    """Return the static text content of a Text node in the template."""
    return (
        _get[Runtime](rt_ptr)[0]
        .templates.get_ptr(UInt32(tmpl_id))[0]
        .get_node_ptr(Int(node_idx))[0]
        .text
    )


@export
fn tmpl_attr_name(rt_ptr: Int64, tmpl_id: Int32, attr_idx: Int32) -> String:
    """Return the name of the attribute at attr_idx in the template."""
    return (
        _get[Runtime](rt_ptr)[0]
        .templates.get_ptr(UInt32(tmpl_id))[0]
        .attrs[Int(attr_idx)]
        .name
    )


@export
fn tmpl_attr_value(rt_ptr: Int64, tmpl_id: Int32, attr_idx: Int32) -> String:
    """Return the value of the attribute at attr_idx in the template."""
    return (
        _get[Runtime](rt_ptr)[0]
        .templates.get_ptr(UInt32(tmpl_id))[0]
        .attrs[Int(attr_idx)]
        .value
    )


# ══════════════════════════════════════════════════════════════════════════════
# VNode Store Exports
# ══════════════════════════════════════════════════════════════════════════════


@export
fn vnode_store_create() -> Int64:
    """Allocate a standalone VNodeStore on the heap.  Returns its pointer."""
    return _to_i64(_heap_new(VNodeStore()))


@export
fn vnode_store_destroy(store_ptr: Int64):
    """Destroy and free a heap-allocated VNodeStore."""
    _heap_del(_get[VNodeStore](store_ptr))


@export
fn vnode_push_template_ref(store_ptr: Int64, tmpl_id: Int32) -> Int32:
    """Create a TemplateRef VNode and push it into the store.  Returns index."""
    return Int32(
        _get[VNodeStore](store_ptr)[0].push(VNode.template_ref(UInt32(tmpl_id)))
    )


@export
fn vnode_push_template_ref_keyed(
    store_ptr: Int64, tmpl_id: Int32, key: String
) -> Int32:
    """Create a keyed TemplateRef VNode.  Returns index."""
    return Int32(
        _get[VNodeStore](store_ptr)[0].push(
            VNode.template_ref_keyed(UInt32(tmpl_id), key)
        )
    )


@export
fn vnode_push_text(store_ptr: Int64, text: String) -> Int32:
    """Create a Text VNode and push it into the store.  Returns index."""
    return Int32(_get[VNodeStore](store_ptr)[0].push(VNode.text_node(text)))


@export
fn vnode_push_placeholder(store_ptr: Int64, element_id: Int32) -> Int32:
    """Create a Placeholder VNode.  Returns index."""
    return Int32(
        _get[VNodeStore](store_ptr)[0].push(
            VNode.placeholder(UInt32(element_id))
        )
    )


@export
fn vnode_push_fragment(store_ptr: Int64) -> Int32:
    """Create an empty Fragment VNode.  Returns index."""
    return Int32(_get[VNodeStore](store_ptr)[0].push(VNode.fragment()))


@export
fn vnode_count(store_ptr: Int64) -> Int32:
    """Return the number of VNodes in the store."""
    return Int32(_get[VNodeStore](store_ptr)[0].count())


@export
fn vnode_kind(store_ptr: Int64, index: Int32) -> Int32:
    """Return the kind tag (VNODE_*) of the VNode at index."""
    return Int32(_get[VNodeStore](store_ptr)[0].kind(UInt32(index)))


@export
fn vnode_template_id(store_ptr: Int64, index: Int32) -> Int32:
    """Return the template_id of the TemplateRef VNode at index."""
    return Int32(_get[VNodeStore](store_ptr)[0].template_id(UInt32(index)))


@export
fn vnode_element_id(store_ptr: Int64, index: Int32) -> Int32:
    """Return the element_id of the Placeholder VNode at index."""
    return Int32(_get[VNodeStore](store_ptr)[0].element_id(UInt32(index)))


@export
fn vnode_has_key(store_ptr: Int64, index: Int32) -> Int32:
    """Check if the VNode has a key.  Returns 1 or 0."""
    return _b2i(_get[VNodeStore](store_ptr)[0].has_key(UInt32(index)))


@export
fn vnode_dynamic_node_count(store_ptr: Int64, index: Int32) -> Int32:
    """Return the number of dynamic nodes on the VNode."""
    return Int32(
        _get[VNodeStore](store_ptr)[0].dynamic_node_count(UInt32(index))
    )


@export
fn vnode_dynamic_attr_count(store_ptr: Int64, index: Int32) -> Int32:
    """Return the number of dynamic attributes on the VNode."""
    return Int32(
        _get[VNodeStore](store_ptr)[0].dynamic_attr_count(UInt32(index))
    )


@export
fn vnode_fragment_child_count(store_ptr: Int64, index: Int32) -> Int32:
    """Return the number of fragment children on the VNode."""
    return Int32(
        _get[VNodeStore](store_ptr)[0].fragment_child_count(UInt32(index))
    )


@export
fn vnode_fragment_child_at(
    store_ptr: Int64, index: Int32, child_pos: Int32
) -> Int32:
    """Return the VNode index of the fragment child at child_pos."""
    return Int32(
        _get[VNodeStore](store_ptr)[0].get_fragment_child(
            UInt32(index), Int(child_pos)
        )
    )


@export
fn vnode_push_dynamic_text_node(
    store_ptr: Int64, vnode_index: Int32, text: String
):
    """Append a dynamic text node to the VNode at vnode_index."""
    _get[VNodeStore](store_ptr)[0].push_dynamic_node(
        UInt32(vnode_index), DynamicNode.text_node(text)
    )


@export
fn vnode_push_dynamic_placeholder(store_ptr: Int64, vnode_index: Int32):
    """Append a dynamic placeholder node to the VNode at vnode_index."""
    _get[VNodeStore](store_ptr)[0].push_dynamic_node(
        UInt32(vnode_index), DynamicNode.placeholder()
    )


@export
fn vnode_push_dynamic_attr_text(
    store_ptr: Int64,
    vnode_index: Int32,
    name: String,
    value: String,
    elem_id: Int32,
):
    """Append a dynamic text attribute to the VNode."""
    _get[VNodeStore](store_ptr)[0].push_dynamic_attr(
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
    _get[VNodeStore](store_ptr)[0].push_dynamic_attr(
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
    _get[VNodeStore](store_ptr)[0].push_dynamic_attr(
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
    _get[VNodeStore](store_ptr)[0].push_dynamic_attr(
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
    _get[VNodeStore](store_ptr)[0].push_dynamic_attr(
        UInt32(vnode_index),
        DynamicAttr(name, AttributeValue.none(), UInt32(elem_id)),
    )


@export
fn vnode_push_fragment_child(
    store_ptr: Int64, vnode_index: Int32, child_index: Int32
):
    """Append a child VNode index to the Fragment at vnode_index."""
    _get[VNodeStore](store_ptr)[0].push_fragment_child(
        UInt32(vnode_index), UInt32(child_index)
    )


@export
fn vnode_get_dynamic_node_kind(
    store_ptr: Int64, vnode_index: Int32, dyn_index: Int32
) -> Int32:
    """Return the kind of the dynamic node at dyn_index."""
    return Int32(
        _get[VNodeStore](store_ptr)[0].get_dynamic_node_kind(
            UInt32(vnode_index), Int(dyn_index)
        )
    )


@export
fn vnode_get_dynamic_attr_kind(
    store_ptr: Int64, vnode_index: Int32, attr_index: Int32
) -> Int32:
    """Return the attribute value kind of the dynamic attr at attr_index."""
    return Int32(
        _get[VNodeStore](store_ptr)[0].get_dynamic_attr_kind(
            UInt32(vnode_index), Int(attr_index)
        )
    )


@export
fn vnode_get_dynamic_attr_element_id(
    store_ptr: Int64, vnode_index: Int32, attr_index: Int32
) -> Int32:
    """Return the element_id of the dynamic attr at attr_index."""
    return Int32(
        _get[VNodeStore](store_ptr)[0].get_dynamic_attr_element_id(
            UInt32(vnode_index), Int(attr_index)
        )
    )


@export
fn vnode_store_clear(store_ptr: Int64):
    """Clear all VNodes from the store."""
    _get[VNodeStore](store_ptr)[0].clear()


# ── Signal arithmetic helpers ────────────────────────────────────────────────


@export
fn signal_iadd_i32(rt_ptr: Int64, key: Int32, rhs: Int32):
    """Increment a signal: signal += rhs."""
    var rt = _get[Runtime](rt_ptr)
    var current = rt[0].peek_signal[Int32](UInt32(key))
    rt[0].write_signal[Int32](UInt32(key), current + rhs)


@export
fn signal_isub_i32(rt_ptr: Int64, key: Int32, rhs: Int32):
    """Decrement a signal: signal -= rhs."""
    var rt = _get[Runtime](rt_ptr)
    var current = rt[0].peek_signal[Int32](UInt32(key))
    rt[0].write_signal[Int32](UInt32(key), current - rhs)


# ══════════════════════════════════════════════════════════════════════════════
# Create & Diff Engine Exports
# ══════════════════════════════════════════════════════════════════════════════


@export
fn writer_create(buf_ptr: Int64, capacity: Int32) -> Int64:
    """Create a heap-allocated MutationWriter.  Returns its pointer."""
    return _to_i64(_alloc_writer(buf_ptr, capacity))


@export
fn writer_destroy(writer_ptr: Int64):
    """Destroy and free a heap-allocated MutationWriter."""
    _free_writer(_get[MutationWriter](writer_ptr))


@export
fn writer_offset(writer_ptr: Int64) -> Int32:
    """Return the current write offset of the MutationWriter."""
    return Int32(_get[MutationWriter](writer_ptr)[0].offset)


@export
fn writer_finalize(writer_ptr: Int64) -> Int32:
    """Write the End sentinel and return the final offset."""
    var ptr = _get[MutationWriter](writer_ptr)
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
    var engine = CreateEngine(
        _get[MutationWriter](writer_ptr),
        _get[ElementIdAllocator](eid_ptr),
        _get[Runtime](rt_ptr),
        _get[VNodeStore](store_ptr),
    )
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
    var w = _get[MutationWriter](writer_ptr)
    var engine = DiffEngine(
        w,
        _get[ElementIdAllocator](eid_ptr),
        _get[Runtime](rt_ptr),
        _get[VNodeStore](store_ptr),
    )
    engine.diff_node(UInt32(old_index), UInt32(new_index))
    return Int32(w[0].offset)


# ── VNode mount state query exports ──────────────────────────────────────────


@export
fn vnode_root_id_count(store_ptr: Int64, index: Int32) -> Int32:
    """Return the number of root ElementIds assigned to this VNode."""
    return Int32(
        _get[VNodeStore](store_ptr)[0].get_ptr(UInt32(index))[0].root_id_count()
    )


@export
fn vnode_get_root_id(store_ptr: Int64, index: Int32, pos: Int32) -> Int32:
    """Return the root ElementId at position `pos`."""
    return Int32(
        _get[VNodeStore](store_ptr)[0]
        .get_ptr(UInt32(index))[0]
        .get_root_id(Int(pos))
    )


@export
fn vnode_dyn_node_id_count(store_ptr: Int64, index: Int32) -> Int32:
    """Return the number of dynamic node ElementIds."""
    return Int32(
        _get[VNodeStore](store_ptr)[0]
        .get_ptr(UInt32(index))[0]
        .dyn_node_id_count()
    )


@export
fn vnode_get_dyn_node_id(store_ptr: Int64, index: Int32, pos: Int32) -> Int32:
    """Return the dynamic node ElementId at position `pos`."""
    return Int32(
        _get[VNodeStore](store_ptr)[0]
        .get_ptr(UInt32(index))[0]
        .get_dyn_node_id(Int(pos))
    )


@export
fn vnode_dyn_attr_id_count(store_ptr: Int64, index: Int32) -> Int32:
    """Return the number of dynamic attribute target ElementIds."""
    return Int32(
        _get[VNodeStore](store_ptr)[0]
        .get_ptr(UInt32(index))[0]
        .dyn_attr_id_count()
    )


@export
fn vnode_get_dyn_attr_id(store_ptr: Int64, index: Int32, pos: Int32) -> Int32:
    """Return the dynamic attribute target ElementId at position `pos`."""
    return Int32(
        _get[VNodeStore](store_ptr)[0]
        .get_ptr(UInt32(index))[0]
        .get_dyn_attr_id(Int(pos))
    )


@export
fn vnode_is_mounted(store_ptr: Int64, index: Int32) -> Int32:
    """Check whether the VNode has been mounted.  Returns 1 or 0."""
    return _b2i(
        _get[VNodeStore](store_ptr)[0].get_ptr(UInt32(index))[0].is_mounted()
    )


# ══════════════════════════════════════════════════════════════════════════════
# Mutation Protocol Test Exports
# ══════════════════════════════════════════════════════════════════════════════


@export
fn mutation_buf_alloc(capacity: Int32) -> Int64:
    """Allocate a mutation buffer. Returns a pointer into WASM linear memory."""
    var ptr = UnsafePointer[UInt8].alloc(Int(capacity))
    return _to_i64(ptr)


@export
fn mutation_buf_free(ptr: Int64):
    """Free a previously allocated mutation buffer."""
    _get[UInt8](ptr).free()


# ── Debug exports ────────────────────────────────────────────────────────────


@export
fn debug_ptr_roundtrip(ptr: Int64) -> Int64:
    """Check that _as_ptr round-trips correctly."""
    return _to_i64(_get[UInt8](ptr))


@export
fn debug_write_byte(ptr: Int64, off: Int32, val: Int32) -> Int32:
    """Write a single byte to ptr+off and return off+1."""
    _get[UInt8](ptr)[Int(off)] = UInt8(val)
    return off + 1


@export
fn debug_read_byte(ptr: Int64, off: Int32) -> Int32:
    """Read a single byte from ptr+off."""
    return Int32(_get[UInt8](ptr)[Int(off)])


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
    buf: Int64, off: Int32, id: Int32, handler_id: Int32, name: String
) -> Int32:
    var w = _writer(buf, off)
    w.new_event_listener(UInt32(id), UInt32(handler_id), name)
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
    w.assign_id(_get[UInt8](path_ptr), Int(path_len), UInt32(id))
    return Int32(w.offset)


@export
fn write_op_replace_placeholder(
    buf: Int64, off: Int32, path_ptr: Int64, path_len: Int32, m: Int32
) -> Int32:
    var w = _writer(buf, off)
    w.replace_placeholder(_get[UInt8](path_ptr), Int(path_len), UInt32(m))
    return Int32(w.offset)


@export
fn write_op_register_template(
    buf: Int64, off: Int32, rt_ptr: Int64, tmpl_id: Int32
) -> Int32:
    """Serialize a registered template into the mutation buffer.

    Looks up the template by ID in the Runtime's template registry and
    writes the full OP_REGISTER_TEMPLATE record.

    Args:
        buf: Pointer to the mutation buffer.
        off: Current write offset in the buffer.
        rt_ptr: Pointer to the Runtime (owns the template registry).
        tmpl_id: Template ID to serialize.

    Returns:
        New write offset after the template record.
    """
    var w = _writer(buf, off)
    var tmpl_ptr = _get[Runtime](rt_ptr)[0].templates.get_ptr(UInt32(tmpl_id))
    w.register_template(tmpl_ptr[0].copy())
    return Int32(w.offset)


# ── Composite test helper ────────────────────────────────────────────────────


@export
fn write_test_sequence(buf: Int64) -> Int32:
    """Write a known 5-mutation sequence for integration testing."""
    var w = MutationWriter(_get[UInt8](buf), 0)
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
    return Int32(
        _get[Runtime](rt_ptr)[0].register_handler(
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
    return Int32(
        _get[Runtime](rt_ptr)[0].register_handler(
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
    return Int32(
        _get[Runtime](rt_ptr)[0].register_handler(
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
    return Int32(
        _get[Runtime](rt_ptr)[0].register_handler(
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
    return Int32(
        _get[Runtime](rt_ptr)[0].register_handler(
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
    return Int32(
        _get[Runtime](rt_ptr)[0].register_handler(
            HandlerEntry.custom(UInt32(scope_id), event_name)
        )
    )


@export
fn handler_register_noop(
    rt_ptr: Int64, scope_id: Int32, event_name: String
) -> Int32:
    """Register a no-op handler.  Returns handler ID."""
    return Int32(
        _get[Runtime](rt_ptr)[0].register_handler(
            HandlerEntry.noop(UInt32(scope_id), event_name)
        )
    )


@export
fn handler_remove(rt_ptr: Int64, handler_id: Int32):
    """Remove an event handler by ID."""
    _get[Runtime](rt_ptr)[0].remove_handler(UInt32(handler_id))


@export
fn handler_count(rt_ptr: Int64) -> Int32:
    """Return the number of live event handlers."""
    return Int32(_get[Runtime](rt_ptr)[0].handler_count())


@export
fn handler_contains(rt_ptr: Int64, handler_id: Int32) -> Int32:
    """Check whether a handler ID is live.  Returns 1 or 0."""
    return _b2i(_get[Runtime](rt_ptr)[0].handlers.contains(UInt32(handler_id)))


@export
fn handler_scope_id(rt_ptr: Int64, handler_id: Int32) -> Int32:
    """Return the scope_id of the handler."""
    return Int32(_get[Runtime](rt_ptr)[0].handlers.scope_id(UInt32(handler_id)))


@export
fn handler_action(rt_ptr: Int64, handler_id: Int32) -> Int32:
    """Return the action tag of the handler."""
    return Int32(_get[Runtime](rt_ptr)[0].handlers.action(UInt32(handler_id)))


@export
fn handler_signal_key(rt_ptr: Int64, handler_id: Int32) -> Int32:
    """Return the signal_key of the handler."""
    return Int32(
        _get[Runtime](rt_ptr)[0].handlers.signal_key(UInt32(handler_id))
    )


@export
fn handler_operand(rt_ptr: Int64, handler_id: Int32) -> Int32:
    """Return the operand of the handler."""
    return Int32(_get[Runtime](rt_ptr)[0].handlers.operand(UInt32(handler_id)))


@export
fn dispatch_event(rt_ptr: Int64, handler_id: Int32, event_type: Int32) -> Int32:
    """Dispatch an event to a handler.  Returns 1 if action executed, 0 otherwise.
    """
    return _b2i(
        _get[Runtime](rt_ptr)[0].dispatch_event(
            UInt32(handler_id), UInt8(event_type)
        )
    )


@export
fn dispatch_event_with_i32(
    rt_ptr: Int64, handler_id: Int32, event_type: Int32, value: Int32
) -> Int32:
    """Dispatch an event with an Int32 payload.  Returns 1 if action executed.
    """
    return _b2i(
        _get[Runtime](rt_ptr)[0].dispatch_event_with_i32(
            UInt32(handler_id), UInt8(event_type), value
        )
    )


@export
fn runtime_drain_dirty(rt_ptr: Int64) -> Int32:
    """Drain the dirty scope queue.  Returns the number of dirty scopes."""
    return Int32(len(_get[Runtime](rt_ptr)[0].drain_dirty()))


# ══════════════════════════════════════════════════════════════════════════════
# Phase 8.3 — Context (Dependency Injection) Exports
# ══════════════════════════════════════════════════════════════════════════════


@export
fn ctx_provide(rt_ptr: Int64, scope_id: Int32, key: Int32, value: Int32):
    """Provide a context value at the given scope."""
    _get[Runtime](rt_ptr)[0].scopes.provide_context(
        UInt32(scope_id), UInt32(key), value
    )


@export
fn ctx_consume(rt_ptr: Int64, scope_id: Int32, key: Int32) -> Int32:
    """Look up a context value by walking up the scope tree.  Returns 0 if not found.
    """
    return _get[Runtime](rt_ptr)[0].scopes.consume_context(
        UInt32(scope_id), UInt32(key)
    )[1]


@export
fn ctx_consume_found(rt_ptr: Int64, scope_id: Int32, key: Int32) -> Int32:
    """Check whether a context value exists.  Returns 1 if found, 0 if not."""
    return _b2i(
        _get[Runtime](rt_ptr)[0].scopes.consume_context(
            UInt32(scope_id), UInt32(key)
        )[0]
    )


@export
fn ctx_has_local(rt_ptr: Int64, scope_id: Int32, key: Int32) -> Int32:
    """Check whether the scope itself provides a context for `key`.  Returns 1 or 0.
    """
    return _b2i(
        _get[Runtime](rt_ptr)[0].scopes.has_context_local(
            UInt32(scope_id), UInt32(key)
        )
    )


@export
fn ctx_count(rt_ptr: Int64, scope_id: Int32) -> Int32:
    """Return the number of context entries provided by this scope."""
    return Int32(
        _get[Runtime](rt_ptr)[0].scopes.context_count(UInt32(scope_id))
    )


@export
fn ctx_remove(rt_ptr: Int64, scope_id: Int32, key: Int32) -> Int32:
    """Remove a context entry.  Returns 1 if removed, 0 if not found."""
    return _b2i(
        _get[Runtime](rt_ptr)[0].scopes.remove_context(
            UInt32(scope_id), UInt32(key)
        )
    )


# ══════════════════════════════════════════════════════════════════════════════
# Phase 8.4 — Error Boundaries Exports
# ══════════════════════════════════════════════════════════════════════════════


@export
fn err_set_boundary(rt_ptr: Int64, scope_id: Int32, enabled: Int32):
    """Mark or unmark a scope as an error boundary."""
    _get[Runtime](rt_ptr)[0].scopes.set_error_boundary(
        UInt32(scope_id), enabled != 0
    )


@export
fn err_is_boundary(rt_ptr: Int64, scope_id: Int32) -> Int32:
    """Check whether the scope is an error boundary.  Returns 1 or 0."""
    return _b2i(
        _get[Runtime](rt_ptr)[0].scopes.is_error_boundary(UInt32(scope_id))
    )


@export
fn err_set_error(rt_ptr: Int64, scope_id: Int32, message: String):
    """Set an error directly on the scope."""
    _get[Runtime](rt_ptr)[0].scopes.set_error(UInt32(scope_id), message)


@export
fn err_clear(rt_ptr: Int64, scope_id: Int32):
    """Clear the error state on the scope."""
    _get[Runtime](rt_ptr)[0].scopes.clear_error(UInt32(scope_id))


@export
fn err_has_error(rt_ptr: Int64, scope_id: Int32) -> Int32:
    """Check whether the scope has a captured error.  Returns 1 or 0."""
    return _b2i(_get[Runtime](rt_ptr)[0].scopes.has_error(UInt32(scope_id)))


@export
fn err_find_boundary(rt_ptr: Int64, scope_id: Int32) -> Int32:
    """Find the nearest error boundary ancestor.  Returns scope ID or -1."""
    return Int32(
        _get[Runtime](rt_ptr)[0].scopes.find_error_boundary(UInt32(scope_id))
    )


@export
fn err_propagate(rt_ptr: Int64, scope_id: Int32, message: String) -> Int32:
    """Propagate an error to its nearest error boundary.  Returns boundary ID or -1.
    """
    return Int32(
        _get[Runtime](rt_ptr)[0].scopes.propagate_error(
            UInt32(scope_id), message
        )
    )


# ══════════════════════════════════════════════════════════════════════════════
# Phase 8.5 — Suspense Exports
# ══════════════════════════════════════════════════════════════════════════════


@export
fn suspense_set_boundary(rt_ptr: Int64, scope_id: Int32, enabled: Int32):
    """Mark or unmark a scope as a suspense boundary."""
    _get[Runtime](rt_ptr)[0].scopes.set_suspense_boundary(
        UInt32(scope_id), enabled != 0
    )


@export
fn suspense_is_boundary(rt_ptr: Int64, scope_id: Int32) -> Int32:
    """Check whether the scope is a suspense boundary.  Returns 1 or 0."""
    return _b2i(
        _get[Runtime](rt_ptr)[0].scopes.is_suspense_boundary(UInt32(scope_id))
    )


@export
fn suspense_set_pending(rt_ptr: Int64, scope_id: Int32, pending: Int32):
    """Set the pending (async loading) state on a scope."""
    _get[Runtime](rt_ptr)[0].scopes.set_pending(UInt32(scope_id), pending != 0)


@export
fn suspense_is_pending(rt_ptr: Int64, scope_id: Int32) -> Int32:
    """Check whether the scope is in a pending state.  Returns 1 or 0."""
    return _b2i(_get[Runtime](rt_ptr)[0].scopes.is_pending(UInt32(scope_id)))


@export
fn suspense_find_boundary(rt_ptr: Int64, scope_id: Int32) -> Int32:
    """Find the nearest suspense boundary ancestor.  Returns scope ID or -1."""
    return Int32(
        _get[Runtime](rt_ptr)[0].scopes.find_suspense_boundary(UInt32(scope_id))
    )


@export
fn suspense_has_pending(rt_ptr: Int64, scope_id: Int32) -> Int32:
    """Check if any descendant of `scope_id` is pending.  Returns 1 or 0."""
    return _b2i(
        _get[Runtime](rt_ptr)[0].scopes.has_pending_descendant(UInt32(scope_id))
    )


@export
fn suspense_resolve(rt_ptr: Int64, scope_id: Int32) -> Int32:
    """Mark a scope as no longer pending.  Returns suspense boundary ID or -1.
    """
    return Int32(
        _get[Runtime](rt_ptr)[0].scopes.resolve_pending(UInt32(scope_id))
    )


# ══════════════════════════════════════════════════════════════════════════════
# Phase 13.2 — Memo (Computed/Derived Signal) Exports
# ══════════════════════════════════════════════════════════════════════════════


@export
fn memo_create_i32(rt_ptr: Int64, scope_id: Int32, initial: Int32) -> Int32:
    """Create a memo with an initial cached value.  Returns memo ID."""
    return Int32(
        _get[Runtime](rt_ptr)[0].create_memo_i32(UInt32(scope_id), initial)
    )


@export
fn memo_begin_compute(rt_ptr: Int64, memo_id: Int32):
    """Begin memo computation — sets memo's context as current."""
    _get[Runtime](rt_ptr)[0].memo_begin_compute(UInt32(memo_id))


@export
fn memo_end_compute_i32(rt_ptr: Int64, memo_id: Int32, value: Int32):
    """End memo computation and store the result."""
    _get[Runtime](rt_ptr)[0].memo_end_compute_i32(UInt32(memo_id), value)


@export
fn memo_read_i32(rt_ptr: Int64, memo_id: Int32) -> Int32:
    """Read the memo's cached value."""
    return _get[Runtime](rt_ptr)[0].memo_read_i32(UInt32(memo_id))


@export
fn memo_is_dirty(rt_ptr: Int64, memo_id: Int32) -> Int32:
    """Check whether the memo needs recomputation.  Returns 1 or 0."""
    return _b2i(_get[Runtime](rt_ptr)[0].memo_is_dirty(UInt32(memo_id)))


@export
fn memo_destroy(rt_ptr: Int64, memo_id: Int32):
    """Destroy a memo, cleaning up its context and output signal."""
    _get[Runtime](rt_ptr)[0].destroy_memo(UInt32(memo_id))


@export
fn memo_count(rt_ptr: Int64) -> Int32:
    """Return the number of live memos."""
    return Int32(_get[Runtime](rt_ptr)[0].memo_count())


@export
fn memo_output_key(rt_ptr: Int64, memo_id: Int32) -> Int32:
    """Return the output signal key of the memo (for testing)."""
    return Int32(_get[Runtime](rt_ptr)[0].memo_output_key(UInt32(memo_id)))


@export
fn memo_context_id(rt_ptr: Int64, memo_id: Int32) -> Int32:
    """Return the reactive context ID of the memo (for testing)."""
    return Int32(_get[Runtime](rt_ptr)[0].memo_context_id(UInt32(memo_id)))


# ══════════════════════════════════════════════════════════════════════════════
# Phase 14 — Effect (Reactive Side Effect) Exports
# ══════════════════════════════════════════════════════════════════════════════


@export
fn effect_create(rt_ptr: Int64, scope_id: Int32) -> Int32:
    """Create an effect with a reactive context.  Returns effect ID."""
    return Int32(_get[Runtime](rt_ptr)[0].create_effect(UInt32(scope_id)))


@export
fn effect_begin_run(rt_ptr: Int64, effect_id: Int32):
    """Begin effect execution — sets effect's context as current."""
    _get[Runtime](rt_ptr)[0].effect_begin_run(UInt32(effect_id))


@export
fn effect_end_run(rt_ptr: Int64, effect_id: Int32):
    """End effect execution — clears pending, restores context."""
    _get[Runtime](rt_ptr)[0].effect_end_run(UInt32(effect_id))


@export
fn effect_is_pending(rt_ptr: Int64, effect_id: Int32) -> Int32:
    """Check whether the effect needs re-execution.  Returns 1 or 0."""
    return _b2i(_get[Runtime](rt_ptr)[0].effect_is_pending(UInt32(effect_id)))


@export
fn effect_destroy(rt_ptr: Int64, effect_id: Int32):
    """Destroy an effect, cleaning up its context signal."""
    _get[Runtime](rt_ptr)[0].destroy_effect(UInt32(effect_id))


@export
fn effect_count(rt_ptr: Int64) -> Int32:
    """Return the number of live effects."""
    return Int32(_get[Runtime](rt_ptr)[0].effect_count())


@export
fn effect_context_id(rt_ptr: Int64, effect_id: Int32) -> Int32:
    """Return the reactive context ID of the effect (for testing)."""
    return Int32(_get[Runtime](rt_ptr)[0].effect_context_id(UInt32(effect_id)))


@export
fn effect_drain_pending(rt_ptr: Int64) -> Int32:
    """Return the number of currently pending effects."""
    return Int32(_get[Runtime](rt_ptr)[0].pending_effect_count())


@export
fn effect_pending_at(rt_ptr: Int64, index: Int32) -> Int32:
    """Return the effect ID at the given index in the pending list."""
    return Int32(_get[Runtime](rt_ptr)[0].pending_effect_at(Int(index)))


# ══════════════════════════════════════════════════════════════════════════════
# Phase 7 — Counter App (End-to-End)
# ══════════════════════════════════════════════════════════════════════════════
#
# Thin @export wrappers calling into apps.counter module.


@export
fn counter_init() -> Int64:
    """Initialize the counter app.  Returns a pointer to the app state."""
    return _to_i64(counter_app_init())


@export
fn counter_destroy(app_ptr: Int64):
    """Destroy the counter app and free all resources."""
    counter_app_destroy(_get[CounterApp](app_ptr))


@export
fn counter_rebuild(app_ptr: Int64, buf_ptr: Int64, capacity: Int32) -> Int32:
    """Initial render (mount) of the counter app.  Returns mutation byte length.
    """
    var writer_ptr = _alloc_writer(buf_ptr, capacity)
    var offset = counter_app_rebuild(_get[CounterApp](app_ptr), writer_ptr)
    _free_writer(writer_ptr)
    return offset


@export
fn counter_handle_event(
    app_ptr: Int64, handler_id: Int32, event_type: Int32
) -> Int32:
    """Dispatch an event to the counter app.  Returns 1 if action executed."""
    return _b2i(
        counter_app_handle_event(
            _get[CounterApp](app_ptr), UInt32(handler_id), UInt8(event_type)
        )
    )


@export
fn counter_flush(app_ptr: Int64, buf_ptr: Int64, capacity: Int32) -> Int32:
    """Flush pending updates.  Returns mutation byte length, or 0 if nothing dirty.
    """
    var writer_ptr = _alloc_writer(buf_ptr, capacity)
    var offset = counter_app_flush(_get[CounterApp](app_ptr), writer_ptr)
    _free_writer(writer_ptr)
    return offset


# ── Counter App Query Exports ────────────────────────────────────────────────


@export
fn counter_rt_ptr(app_ptr: Int64) -> Int64:
    """Return the runtime pointer for JS template registration."""
    return _to_i64(_get[CounterApp](app_ptr)[0].ctx.shell.runtime)


@export
fn counter_tmpl_id(app_ptr: Int64) -> Int32:
    """Return the counter template ID."""
    return Int32(_get[CounterApp](app_ptr)[0].ctx.template_id)


@export
fn counter_incr_handler(app_ptr: Int64) -> Int32:
    """Return the increment handler ID (from view_events[0])."""
    var events = _get[CounterApp](app_ptr)[0].ctx.view_events()
    if len(events) > 0:
        return Int32(events[0].handler_id)
    return -1


@export
fn counter_decr_handler(app_ptr: Int64) -> Int32:
    """Return the decrement handler ID (from view_events[1])."""
    var events = _get[CounterApp](app_ptr)[0].ctx.view_events()
    if len(events) > 1:
        return Int32(events[1].handler_id)
    return -1


@export
fn counter_count_value(app_ptr: Int64) -> Int32:
    """Peek the current count signal value (without subscribing)."""
    return _get[CounterApp](app_ptr)[0].count.peek()


@export
fn counter_has_dirty(app_ptr: Int64) -> Int32:
    """Check if the counter app has dirty scopes.  Returns 1 or 0."""
    return _b2i(_get[CounterApp](app_ptr)[0].ctx.has_dirty())


@export
fn counter_scope_id(app_ptr: Int64) -> Int32:
    """Return the counter app's root scope ID."""
    return Int32(_get[CounterApp](app_ptr)[0].ctx.scope_id)


@export
fn counter_count_signal(app_ptr: Int64) -> Int32:
    """Return the counter app's count signal key."""
    return Int32(_get[CounterApp](app_ptr)[0].count.key)


@export
fn counter_doubled_value(app_ptr: Int64) -> Int32:
    """Return doubled count value (computed inline, no memo)."""
    return _get[CounterApp](app_ptr)[0].count.peek() * 2


@export
fn counter_doubled_memo(app_ptr: Int64) -> Int32:
    """Return -1 (doubled memo removed in ergonomic counter rewrite)."""
    return -1


# ══════════════════════════════════════════════════════════════════════════════
# Phase 8 — Todo App
# ══════════════════════════════════════════════════════════════════════════════
#
# Thin @export wrappers calling into apps.todo module.


@export
fn todo_init() -> Int64:
    """Initialize the todo app.  Returns a pointer to the app state."""
    return _to_i64(todo_app_init())


@export
fn todo_destroy(app_ptr: Int64):
    """Destroy the todo app and free all resources."""
    todo_app_destroy(_get[TodoApp](app_ptr))


@export
fn todo_rebuild(app_ptr: Int64, buf_ptr: Int64, capacity: Int32) -> Int32:
    """Initial render (mount) of the todo app.  Returns mutation byte length."""
    var writer_ptr = _alloc_writer(buf_ptr, capacity)
    var offset = todo_app_rebuild(_get[TodoApp](app_ptr), writer_ptr)
    _free_writer(writer_ptr)
    return offset


@export
fn todo_handle_event(
    app_ptr: Int64, handler_id: Int32, event_type: Int32
) -> Int32:
    """Dispatch a click event by handler ID.

    Returns 1 if the handler was found and action executed (toggle/remove),
    0 if the handler is the add handler (JS must read input) or unknown.
    """
    return _b2i(_get[TodoApp](app_ptr)[0].handle_event(UInt32(handler_id)))


@export
fn todo_add_item(app_ptr: Int64, text: String):
    """Add a new item to the todo list."""
    _get[TodoApp](app_ptr)[0].add_item(text)


@export
fn todo_remove_item(app_ptr: Int64, item_id: Int32):
    """Remove an item by its ID."""
    _get[TodoApp](app_ptr)[0].remove_item(item_id)


@export
fn todo_toggle_item(app_ptr: Int64, item_id: Int32):
    """Toggle an item's completed status."""
    _get[TodoApp](app_ptr)[0].toggle_item(item_id)


@export
fn todo_set_input(app_ptr: Int64, text: String):
    """Update the input text (stored in app state, no re-render)."""
    _get[TodoApp](app_ptr)[0].input_text = text


@export
fn todo_flush(app_ptr: Int64, buf_ptr: Int64, capacity: Int32) -> Int32:
    """Flush pending updates.  Returns mutation byte length, or 0 if nothing dirty.
    """
    var writer_ptr = _alloc_writer(buf_ptr, capacity)
    var offset = todo_app_flush(_get[TodoApp](app_ptr), writer_ptr)
    _free_writer(writer_ptr)
    return offset


# ── Todo App Query Exports ───────────────────────────────────────────────────


@export
fn todo_app_template_id(app_ptr: Int64) -> Int32:
    """Return the app template ID."""
    return Int32(_get[TodoApp](app_ptr)[0].app_template_id)


@export
fn todo_item_template_id(app_ptr: Int64) -> Int32:
    """Return the item template ID."""
    return Int32(_get[TodoApp](app_ptr)[0].item_template_id)


@export
fn todo_add_handler(app_ptr: Int64) -> Int32:
    """Return the Add button handler ID."""
    return Int32(_get[TodoApp](app_ptr)[0].add_handler)


@export
fn todo_item_count(app_ptr: Int64) -> Int32:
    """Return the number of items in the list."""
    return Int32(len(_get[TodoApp](app_ptr)[0].items))


@export
fn todo_item_id_at(app_ptr: Int64, index: Int32) -> Int32:
    """Return the ID of the item at the given index."""
    return _get[TodoApp](app_ptr)[0].items[Int(index)].id


@export
fn todo_item_completed_at(app_ptr: Int64, index: Int32) -> Int32:
    """Return 1 if the item at index is completed, 0 otherwise."""
    return _b2i(_get[TodoApp](app_ptr)[0].items[Int(index)].completed)


@export
fn todo_has_dirty(app_ptr: Int64) -> Int32:
    """Check if the todo app has dirty scopes.  Returns 1 or 0."""
    return _b2i(_get[TodoApp](app_ptr)[0].shell.has_dirty())


@export
fn todo_list_version(app_ptr: Int64) -> Int32:
    """Return the current list version signal value."""
    var app = _get[TodoApp](app_ptr)
    return app[0].shell.peek_signal_i32(app[0].list_version_signal)


@export
fn todo_scope_id(app_ptr: Int64) -> Int32:
    """Return the root scope ID."""
    return Int32(_get[TodoApp](app_ptr)[0].scope_id)


@export
fn todo_handler_count(app_ptr: Int64) -> Int32:
    """Return the number of live event handlers in the todo app's runtime."""
    return Int32(_get[TodoApp](app_ptr)[0].shell.runtime[0].handler_count())


# ══════════════════════════════════════════════════════════════════════════════
# Phase 9 — Benchmark App
# ══════════════════════════════════════════════════════════════════════════════
#
# Thin @export wrappers calling into apps.bench module.


@export
fn bench_init() -> Int64:
    """Initialize the benchmark app.  Returns a pointer to the app state."""
    return _to_i64(bench_app_init())


@export
fn bench_destroy(app_ptr: Int64):
    """Destroy the benchmark app and free all resources."""
    bench_app_destroy(_get[BenchmarkApp](app_ptr))


@export
fn bench_create(app_ptr: Int64, count: Int32):
    """Replace all rows with `count` new rows (benchmark: create)."""
    _get[BenchmarkApp](app_ptr)[0].create_rows(Int(count))


@export
fn bench_append(app_ptr: Int64, count: Int32):
    """Append `count` new rows (benchmark: append)."""
    _get[BenchmarkApp](app_ptr)[0].append_rows(Int(count))


@export
fn bench_update(app_ptr: Int64):
    """Update every 10th row label (benchmark: update)."""
    _get[BenchmarkApp](app_ptr)[0].update_every_10th()


@export
fn bench_select(app_ptr: Int64, id: Int32):
    """Select a row by id (benchmark: select)."""
    _get[BenchmarkApp](app_ptr)[0].select_row(id)


@export
fn bench_swap(app_ptr: Int64):
    """Swap rows at indices 1 and 998 (benchmark: swap)."""
    _get[BenchmarkApp](app_ptr)[0].swap_rows(1, 998)


@export
fn bench_remove(app_ptr: Int64, id: Int32):
    """Remove a row by id (benchmark: remove)."""
    _get[BenchmarkApp](app_ptr)[0].remove_row(id)


@export
fn bench_clear(app_ptr: Int64):
    """Clear all rows (benchmark: clear)."""
    _get[BenchmarkApp](app_ptr)[0].clear_rows()


@export
fn bench_rebuild(app_ptr: Int64, buf_ptr: Int64, capacity: Int32) -> Int32:
    """Initial render of the benchmark table body.  Returns mutation byte length.
    """
    var writer_ptr = _alloc_writer(buf_ptr, capacity)
    var offset = bench_app_rebuild(_get[BenchmarkApp](app_ptr), writer_ptr)
    _free_writer(writer_ptr)
    return offset


@export
fn bench_flush(app_ptr: Int64, buf_ptr: Int64, capacity: Int32) -> Int32:
    """Flush pending updates.  Returns mutation byte length, or 0 if nothing dirty.
    """
    var writer_ptr = _alloc_writer(buf_ptr, capacity)
    var offset = bench_app_flush(_get[BenchmarkApp](app_ptr), writer_ptr)
    _free_writer(writer_ptr)
    return offset


# ── Benchmark App Query Exports ──────────────────────────────────────────────


@export
fn bench_row_count(app_ptr: Int64) -> Int32:
    """Return the number of rows."""
    return Int32(len(_get[BenchmarkApp](app_ptr)[0].rows))


@export
fn bench_row_id_at(app_ptr: Int64, index: Int32) -> Int32:
    """Return the id of the row at the given index."""
    return _get[BenchmarkApp](app_ptr)[0].rows[Int(index)].id


@export
fn bench_selected(app_ptr: Int64) -> Int32:
    """Return the currently selected row id (0 = none)."""
    var app = _get[BenchmarkApp](app_ptr)
    return app[0].shell.peek_signal_i32(app[0].selected_signal)


@export
fn bench_has_dirty(app_ptr: Int64) -> Int32:
    """Check if the benchmark app has dirty scopes.  Returns 1 or 0."""
    return _b2i(_get[BenchmarkApp](app_ptr)[0].shell.has_dirty())


@export
fn bench_version(app_ptr: Int64) -> Int32:
    """Return the current version signal value."""
    var app = _get[BenchmarkApp](app_ptr)
    return app[0].shell.peek_signal_i32(app[0].version_signal)


@export
fn bench_row_template_id(app_ptr: Int64) -> Int32:
    """Return the row template ID."""
    return Int32(_get[BenchmarkApp](app_ptr)[0].row_template_id)


@export
fn bench_scope_id(app_ptr: Int64) -> Int32:
    """Return the root scope ID."""
    return Int32(_get[BenchmarkApp](app_ptr)[0].scope_id)


@export
fn bench_handler_count(app_ptr: Int64) -> Int32:
    """Return the number of live event handlers in the bench app's runtime."""
    return Int32(
        _get[BenchmarkApp](app_ptr)[0].shell.runtime[0].handler_count()
    )


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
    return Int32(len(_get[Runtime](rt_ptr)[0].signals._entries))


@export
fn debug_scope_store_capacity(rt_ptr: Int64) -> Int32:
    """Return the total number of scope slots (occupied + free)."""
    return Int32(len(_get[Runtime](rt_ptr)[0].scopes._scopes))


@export
fn debug_vnode_store_count(store_ptr: Int64) -> Int32:
    """Return the number of VNodes in a standalone store."""
    return Int32(_get[VNodeStore](store_ptr)[0].count())


@export
fn debug_handler_store_capacity(rt_ptr: Int64) -> Int32:
    """Return the total number of handler slots."""
    return Int32(len(_get[Runtime](rt_ptr)[0].handlers._entries))


@export
fn debug_eid_alloc_capacity(alloc_ptr: Int64) -> Int32:
    """Return the total number of ElementId slots."""
    return Int32(len(_get[ElementIdAllocator](alloc_ptr)[0]._slots))


# ── Memory Management Test Exports ───────────────────────────────────────────


@export
fn mem_test_signal_cycle(rt_ptr: Int64, count: Int32) -> Int32:
    """Create and destroy `count` signals.  Returns final signal_count (should be 0).
    """
    var rt = _get[Runtime](rt_ptr)
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
    var rt = _get[Runtime](rt_ptr)
    var ids = List[UInt32]()
    for i in range(Int(count)):
        ids.append(rt[0].create_scope(0, -1))
    for i in range(Int(count)):
        rt[0].destroy_scope(ids[i])
    return Int32(rt[0].scope_count())


@export
fn mem_test_rapid_writes(rt_ptr: Int64, key: Int32, count: Int32) -> Int32:
    """Write to a signal `count` times.  Returns final dirty_count."""
    var rt = _get[Runtime](rt_ptr)
    for i in range(Int(count)):
        rt[0].write_signal[Int32](UInt32(key), Int32(i))
    return Int32(rt[0].dirty_count())


# ══════════════════════════════════════════════════════════════════════════════
# Phase 10.4 — Scheduler Exports
# ══════════════════════════════════════════════════════════════════════════════


@export
fn scheduler_create() -> Int64:
    """Allocate a Scheduler on the heap.  Returns its pointer."""
    return _to_i64(_heap_new(Scheduler()))


@export
fn scheduler_destroy(sched_ptr: Int64):
    """Destroy and free a heap-allocated Scheduler."""
    _heap_del(_get[Scheduler](sched_ptr))


@export
fn scheduler_collect(sched_ptr: Int64, rt_ptr: Int64):
    """Drain the runtime's dirty queue into the scheduler."""
    _get[Scheduler](sched_ptr)[0].collect(_get[Runtime](rt_ptr))


@export
fn scheduler_collect_one(sched_ptr: Int64, rt_ptr: Int64, scope_id: Int32):
    """Add a single scope to the scheduler queue."""
    _get[Scheduler](sched_ptr)[0].collect_one(
        _get[Runtime](rt_ptr), UInt32(scope_id)
    )


@export
fn scheduler_next(sched_ptr: Int64) -> Int32:
    """Return and remove the next scope to render (lowest height first)."""
    return Int32(_get[Scheduler](sched_ptr)[0].next())


@export
fn scheduler_is_empty(sched_ptr: Int64) -> Int32:
    """Check if the scheduler has no pending dirty scopes.  Returns 1 or 0."""
    return _b2i(_get[Scheduler](sched_ptr)[0].is_empty())


@export
fn scheduler_count(sched_ptr: Int64) -> Int32:
    """Return the number of pending dirty scopes."""
    return Int32(_get[Scheduler](sched_ptr)[0].count())


@export
fn scheduler_has_scope(sched_ptr: Int64, scope_id: Int32) -> Int32:
    """Check if a scope is already in the scheduler queue.  Returns 1 or 0."""
    return _b2i(_get[Scheduler](sched_ptr)[0].has_scope(UInt32(scope_id)))


@export
fn scheduler_clear(sched_ptr: Int64):
    """Discard all pending dirty scopes."""
    _get[Scheduler](sched_ptr)[0].clear()


# ══════════════════════════════════════════════════════════════════════════════
# Phase 10.4 — AppShell Exports
# ══════════════════════════════════════════════════════════════════════════════


@export
fn shell_create() -> Int64:
    """Create an AppShell with all subsystems allocated.  Returns its pointer.
    """
    return _to_i64(_heap_new(app_shell_create()))


@export
fn shell_destroy(shell_ptr: Int64):
    """Destroy an AppShell and free all resources."""
    var ptr = _get[AppShell](shell_ptr)
    ptr[0].destroy()
    _heap_del(ptr)


@export
fn shell_is_alive(shell_ptr: Int64) -> Int32:
    """Check if the shell is alive.  Returns 1 or 0."""
    return _b2i(_get[AppShell](shell_ptr)[0].is_alive())


@export
fn shell_create_root_scope(shell_ptr: Int64) -> Int32:
    """Create a root scope via the AppShell.  Returns scope ID."""
    return Int32(_get[AppShell](shell_ptr)[0].create_root_scope())


@export
fn shell_create_child_scope(shell_ptr: Int64, parent_id: Int32) -> Int32:
    """Create a child scope via the AppShell.  Returns scope ID."""
    return Int32(
        _get[AppShell](shell_ptr)[0].create_child_scope(UInt32(parent_id))
    )


@export
fn shell_create_signal_i32(shell_ptr: Int64, initial: Int32) -> Int32:
    """Create an Int32 signal via the AppShell.  Returns signal key."""
    return Int32(_get[AppShell](shell_ptr)[0].create_signal_i32(initial))


@export
fn shell_read_signal_i32(shell_ptr: Int64, key: Int32) -> Int32:
    """Read an Int32 signal via the AppShell (with context tracking)."""
    return _get[AppShell](shell_ptr)[0].read_signal_i32(UInt32(key))


@export
fn shell_peek_signal_i32(shell_ptr: Int64, key: Int32) -> Int32:
    """Peek an Int32 signal via the AppShell (without subscribing)."""
    return _get[AppShell](shell_ptr)[0].peek_signal_i32(UInt32(key))


@export
fn shell_write_signal_i32(shell_ptr: Int64, key: Int32, value: Int32):
    """Write to an Int32 signal via the AppShell."""
    _get[AppShell](shell_ptr)[0].write_signal_i32(UInt32(key), value)


@export
fn shell_begin_render(shell_ptr: Int64, scope_id: Int32) -> Int32:
    """Begin rendering a scope.  Returns previous scope ID (or -1)."""
    return Int32(_get[AppShell](shell_ptr)[0].begin_render(UInt32(scope_id)))


@export
fn shell_end_render(shell_ptr: Int64, prev_scope: Int32):
    """End rendering and restore the previous scope."""
    _get[AppShell](shell_ptr)[0].end_render(Int(prev_scope))


@export
fn shell_has_dirty(shell_ptr: Int64) -> Int32:
    """Check if the shell has dirty scopes.  Returns 1 or 0."""
    return _b2i(_get[AppShell](shell_ptr)[0].has_dirty())


@export
fn shell_collect_dirty(shell_ptr: Int64):
    """Drain dirty scopes into the shell's scheduler."""
    _get[AppShell](shell_ptr)[0].collect_dirty()


@export
fn shell_next_dirty(shell_ptr: Int64) -> Int32:
    """Return next dirty scope from the shell's scheduler."""
    return Int32(_get[AppShell](shell_ptr)[0].next_dirty())


@export
fn shell_scheduler_empty(shell_ptr: Int64) -> Int32:
    """Check if the shell's scheduler is empty.  Returns 1 or 0."""
    return _b2i(_get[AppShell](shell_ptr)[0].scheduler_empty())


@export
fn shell_dispatch_event(
    shell_ptr: Int64, handler_id: Int32, event_type: Int32
) -> Int32:
    """Dispatch an event via the AppShell.  Returns 1 if executed."""
    return _b2i(
        _get[AppShell](shell_ptr)[0].dispatch_event(
            UInt32(handler_id), UInt8(event_type)
        )
    )


# ── Shell memo helpers (M13.5) ──────────────────────────────────────────────


@export
fn shell_memo_create_i32(
    shell_ptr: Int64, scope_id: Int32, initial: Int32
) -> Int32:
    """Create an Int32 memo via the AppShell.  Returns memo ID."""
    return Int32(
        _get[AppShell](shell_ptr)[0].create_memo_i32(UInt32(scope_id), initial)
    )


@export
fn shell_memo_begin_compute(shell_ptr: Int64, memo_id: Int32):
    """Begin memo computation via the AppShell."""
    _get[AppShell](shell_ptr)[0].memo_begin_compute(UInt32(memo_id))


@export
fn shell_memo_end_compute_i32(shell_ptr: Int64, memo_id: Int32, value: Int32):
    """End memo computation and cache the result via the AppShell."""
    _get[AppShell](shell_ptr)[0].memo_end_compute_i32(UInt32(memo_id), value)


@export
fn shell_memo_read_i32(shell_ptr: Int64, memo_id: Int32) -> Int32:
    """Read a memo's cached value via the AppShell."""
    return _get[AppShell](shell_ptr)[0].memo_read_i32(UInt32(memo_id))


@export
fn shell_memo_is_dirty(shell_ptr: Int64, memo_id: Int32) -> Int32:
    """Check whether the memo needs recomputation.  Returns 1 or 0."""
    return _b2i(_get[AppShell](shell_ptr)[0].memo_is_dirty(UInt32(memo_id)))


@export
fn shell_use_memo_i32(shell_ptr: Int64, initial: Int32) -> Int32:
    """Hook: create or retrieve an Int32 memo via the AppShell."""
    return Int32(_get[AppShell](shell_ptr)[0].use_memo_i32(initial))


# ══════════════════════════════════════════════════════════════════════════════
# Phase 14.4 — AppShell Effect Helpers
# ══════════════════════════════════════════════════════════════════════════════


@export
fn shell_effect_create(shell_ptr: Int64, scope_id: Int32) -> Int32:
    """Create an effect via the AppShell.  Returns effect ID."""
    return Int32(_get[AppShell](shell_ptr)[0].create_effect(UInt32(scope_id)))


@export
fn shell_effect_begin_run(shell_ptr: Int64, effect_id: Int32):
    """Begin effect execution via the AppShell."""
    _get[AppShell](shell_ptr)[0].effect_begin_run(UInt32(effect_id))


@export
fn shell_effect_end_run(shell_ptr: Int64, effect_id: Int32):
    """End effect execution via the AppShell."""
    _get[AppShell](shell_ptr)[0].effect_end_run(UInt32(effect_id))


@export
fn shell_effect_is_pending(shell_ptr: Int64, effect_id: Int32) -> Int32:
    """Check whether effect needs re-execution via the AppShell.  Returns 1 or 0.
    """
    return _b2i(
        _get[AppShell](shell_ptr)[0].effect_is_pending(UInt32(effect_id))
    )


@export
fn shell_use_effect(shell_ptr: Int64) -> Int32:
    """Hook: create or retrieve an effect via the AppShell."""
    return Int32(_get[AppShell](shell_ptr)[0].use_effect())


@export
fn shell_effect_drain_pending(shell_ptr: Int64) -> Int32:
    """Return the number of pending effects via the AppShell."""
    return Int32(_get[AppShell](shell_ptr)[0].pending_effect_count())


@export
fn shell_effect_pending_at(shell_ptr: Int64, index: Int32) -> Int32:
    """Return the pending effect ID at the given index via the AppShell."""
    return Int32(_get[AppShell](shell_ptr)[0].pending_effect_at(Int(index)))


@export
fn shell_rt_ptr(shell_ptr: Int64) -> Int64:
    """Return the runtime pointer from an AppShell (for template registration etc.).
    """
    return _to_i64(_get[AppShell](shell_ptr)[0].runtime)


@export
fn shell_store_ptr(shell_ptr: Int64) -> Int64:
    """Return the VNodeStore pointer from an AppShell."""
    return _to_i64(_get[AppShell](shell_ptr)[0].store)


@export
fn shell_eid_ptr(shell_ptr: Int64) -> Int64:
    """Return the ElementIdAllocator pointer from an AppShell."""
    return _to_i64(_get[AppShell](shell_ptr)[0].eid_alloc)


@export
fn shell_mount(
    shell_ptr: Int64, buf_ptr: Int64, capacity: Int32, vnode_index: Int32
) -> Int32:
    """Mount a VNode via the AppShell.  Returns mutation byte length."""
    var writer_ptr = _alloc_writer(buf_ptr, capacity)
    var result = _get[AppShell](shell_ptr)[0].mount(
        writer_ptr, UInt32(vnode_index)
    )
    _free_writer(writer_ptr)
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
    var ptr = _get[AppShell](shell_ptr)
    var writer_ptr = _alloc_writer(buf_ptr, capacity)
    ptr[0].diff(writer_ptr, UInt32(old_index), UInt32(new_index))
    var result = ptr[0].finalize(writer_ptr)
    _free_writer(writer_ptr)
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
fn _alloc_node(var val: Node) -> Int64:
    """Heap-allocate a Node and return its address as Int64."""
    return _to_i64(_heap_new(val^))


@always_inline
fn _free_node(addr: Int64):
    """Destroy and free a heap-allocated Node from its Int64 address."""
    _heap_del(_get[Node](addr))


@export
fn dsl_node_text(s: String) -> Int64:
    """Create a text Node on the heap.  Returns a pointer handle."""
    return _alloc_node(text(s))


@export
fn dsl_node_dyn_text(index: Int32) -> Int64:
    """Create a dynamic text Node on the heap."""
    return _alloc_node(dyn_text(Int(index)))


@export
fn dsl_node_dyn_node(index: Int32) -> Int64:
    """Create a dynamic node placeholder on the heap."""
    return _alloc_node(dyn_node(Int(index)))


@export
fn dsl_node_attr(name: String, value: String) -> Int64:
    """Create a static attribute Node on the heap."""
    return _alloc_node(attr(name, value))


@export
fn dsl_node_dyn_attr(index: Int32) -> Int64:
    """Create a dynamic attribute Node on the heap."""
    return _alloc_node(dyn_attr(Int(index)))


@export
fn dsl_node_element(html_tag: Int32) -> Int64:
    """Create an empty element Node on the heap."""
    return _alloc_node(el_empty(UInt8(html_tag)))


@export
fn dsl_node_add_item(parent_ptr: Int64, child_ptr: Int64):
    """Add a child/attr Node to an element Node.

    The child Node is moved out of its heap slot (the child pointer
    becomes invalid after this call).
    """
    var child_val = _get[Node](child_ptr)[0].copy()
    _get[Node](parent_ptr)[0].add_item(child_val^)
    _free_node(child_ptr)


@export
fn dsl_node_destroy(ptr: Int64):
    """Destroy and free a heap-allocated Node."""
    _free_node(ptr)


@export
fn dsl_node_kind(ptr: Int64) -> Int32:
    """Return the kind tag of a Node."""
    return Int32(_get[Node](ptr)[0].kind)


@export
fn dsl_node_tag(ptr: Int64) -> Int32:
    """Return the HTML tag of an element Node."""
    return Int32(_get[Node](ptr)[0].tag)


@export
fn dsl_node_item_count(ptr: Int64) -> Int32:
    """Return the total item count (children + attrs) of an element Node."""
    return Int32(_get[Node](ptr)[0].item_count())


@export
fn dsl_node_child_count(ptr: Int64) -> Int32:
    """Return the child count (excluding attrs) of an element Node."""
    return Int32(_get[Node](ptr)[0].child_count())


@export
fn dsl_node_attr_count(ptr: Int64) -> Int32:
    """Return the attribute count (excluding children) of an element Node."""
    return Int32(_get[Node](ptr)[0].attr_count())


@export
fn dsl_node_dynamic_index(ptr: Int64) -> Int32:
    """Return the dynamic_index of a DYN_TEXT/DYN_NODE/DYN_ATTR Node."""
    return Int32(_get[Node](ptr)[0].dynamic_index)


@export
fn dsl_node_count_nodes(ptr: Int64) -> Int32:
    """Recursively count tree nodes (excluding attrs)."""
    return Int32(count_nodes(_get[Node](ptr)[0]))


@export
fn dsl_node_count_all(ptr: Int64) -> Int32:
    """Recursively count all items (including attrs)."""
    return Int32(count_all_items(_get[Node](ptr)[0]))


@export
fn dsl_node_count_dyn_text(ptr: Int64) -> Int32:
    """Count DYN_TEXT slots in the tree."""
    return Int32(count_dynamic_text_slots(_get[Node](ptr)[0]))


@export
fn dsl_node_count_dyn_node(ptr: Int64) -> Int32:
    """Count DYN_NODE slots in the tree."""
    return Int32(count_dynamic_node_slots(_get[Node](ptr)[0]))


@export
fn dsl_node_count_dyn_attr(ptr: Int64) -> Int32:
    """Count DYN_ATTR slots in the tree."""
    return Int32(count_dynamic_attr_slots(_get[Node](ptr)[0]))


@export
fn dsl_node_count_static_attr(ptr: Int64) -> Int32:
    """Count STATIC_ATTR nodes in the tree."""
    return Int32(count_static_attr_nodes(_get[Node](ptr)[0]))


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
    var template = to_template(_get[Node](node_ptr)[0], name)
    var tmpl_id = _get[Runtime](rt_ptr)[0].templates.register(template^)
    _free_node(node_ptr)
    return Int32(tmpl_id)


# ── VNodeBuilder WASM exports ────────────────────────────────────────────────


@export
fn dsl_vb_create(tmpl_id: Int32, store_ptr: Int64) -> Int64:
    """Create a VNodeBuilder on the heap.

    Returns a pointer handle to the VNodeBuilder.
    """
    return _to_i64(
        _heap_new(VNodeBuilder(UInt32(tmpl_id), _get[VNodeStore](store_ptr)))
    )


@export
fn dsl_vb_create_keyed(tmpl_id: Int32, key: String, store_ptr: Int64) -> Int64:
    """Create a keyed VNodeBuilder on the heap."""
    return _to_i64(
        _heap_new(
            VNodeBuilder(UInt32(tmpl_id), key, _get[VNodeStore](store_ptr))
        )
    )


@export
fn dsl_vb_destroy(ptr: Int64):
    """Destroy and free a heap-allocated VNodeBuilder."""
    _heap_del(_get[VNodeBuilder](ptr))


@export
fn dsl_vb_add_dyn_text(ptr: Int64, value: String):
    """Add a dynamic text node via the VNodeBuilder."""
    _get[VNodeBuilder](ptr)[0].add_dyn_text(value)


@export
fn dsl_vb_add_dyn_placeholder(ptr: Int64):
    """Add a dynamic placeholder via the VNodeBuilder."""
    _get[VNodeBuilder](ptr)[0].add_dyn_placeholder()


@export
fn dsl_vb_add_dyn_event(ptr: Int64, event_name: String, handler_id: Int32):
    """Add a dynamic event handler via the VNodeBuilder."""
    _get[VNodeBuilder](ptr)[0].add_dyn_event(event_name, UInt32(handler_id))


@export
fn dsl_vb_add_dyn_text_attr(ptr: Int64, name: String, value: String):
    """Add a dynamic text attribute via the VNodeBuilder."""
    _get[VNodeBuilder](ptr)[0].add_dyn_text_attr(name, value)


@export
fn dsl_vb_add_dyn_int_attr(ptr: Int64, name: String, value: Int64):
    """Add a dynamic integer attribute via the VNodeBuilder."""
    _get[VNodeBuilder](ptr)[0].add_dyn_int_attr(name, value)


@export
fn dsl_vb_add_dyn_bool_attr(ptr: Int64, name: String, value: Int32):
    """Add a dynamic boolean attribute via the VNodeBuilder."""
    _get[VNodeBuilder](ptr)[0].add_dyn_bool_attr(name, value != 0)


@export
fn dsl_vb_add_dyn_none_attr(ptr: Int64, name: String):
    """Add a dynamic none/removal attribute via the VNodeBuilder."""
    _get[VNodeBuilder](ptr)[0].add_dyn_none_attr(name)


@export
fn dsl_vb_index(ptr: Int64) -> Int32:
    """Return the VNode index from the VNodeBuilder."""
    return Int32(_get[VNodeBuilder](ptr)[0].index())


# ── Self-contained DSL tests ─────────────────────────────────────────────────
#
# Thin @export wrappers delegating to vdom.dsl_tests (M10.13).
# Each returns 1 (pass) or 0 (fail).


@export
fn dsl_test_text_node() -> Int32:
    return _dsl_test_text_node()


@export
fn dsl_test_dyn_text_node() -> Int32:
    return _dsl_test_dyn_text_node()


@export
fn dsl_test_dyn_node_slot() -> Int32:
    return _dsl_test_dyn_node_slot()


@export
fn dsl_test_static_attr() -> Int32:
    return _dsl_test_static_attr()


@export
fn dsl_test_dyn_attr() -> Int32:
    return _dsl_test_dyn_attr()


@export
fn dsl_test_empty_element() -> Int32:
    return _dsl_test_empty_element()


@export
fn dsl_test_element_with_children() -> Int32:
    return _dsl_test_element_with_children()


@export
fn dsl_test_element_with_attrs() -> Int32:
    return _dsl_test_element_with_attrs()


@export
fn dsl_test_element_mixed() -> Int32:
    return _dsl_test_element_mixed()


@export
fn dsl_test_nested_elements() -> Int32:
    return _dsl_test_nested_elements()


@export
fn dsl_test_counter_template() -> Int32:
    return _dsl_test_counter_template()


@export
fn dsl_test_to_template_simple() -> Int32:
    return _dsl_test_to_template_simple()


@export
fn dsl_test_to_template_attrs() -> Int32:
    return _dsl_test_to_template_attrs()


@export
fn dsl_test_to_template_multi_root() -> Int32:
    return _dsl_test_to_template_multi_root()


@export
fn dsl_test_vnode_builder() -> Int32:
    return _dsl_test_vnode_builder()


@export
fn dsl_test_vnode_builder_keyed() -> Int32:
    return _dsl_test_vnode_builder_keyed()


@export
fn dsl_test_all_tag_helpers() -> Int32:
    return _dsl_test_all_tag_helpers()


@export
fn dsl_test_count_utilities() -> Int32:
    return _dsl_test_count_utilities()


@export
fn dsl_test_template_equivalence() -> Int32:
    return _dsl_test_template_equivalence()


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

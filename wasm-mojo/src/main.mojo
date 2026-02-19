from bridge import MutationWriter
from arena import ElementId, ElementIdAllocator
from signals import Runtime, create_runtime, destroy_runtime, HOOK_SIGNAL
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

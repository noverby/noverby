# HandlerRegistry and event dispatch exercised through the real WASM binary
# via wasmtime-py (called from Mojo via Python interop).
#
# These tests verify that the event handler registry and dispatch system work
# correctly when compiled to WASM and executed via the Wasmtime runtime.
#
# Note: Tests for handlers_for_scope, event_name, get (copy), HandlerEntry
# default/copy constructors, and event/action type constants are not covered
# here since those specific APIs lack WASM exports.
#
# Run with:
#   mojo test test/test_events.mojo

from python import Python, PythonObject
from testing import assert_equal, assert_true, assert_false


fn _get_wasm() raises -> PythonObject:
    Python.add_to_path("test")
    var harness = Python.import_module("wasm_harness")
    return harness.get_instance()


# ── Helpers ──────────────────────────────────────────────────────────────────


fn _create_runtime(w: PythonObject) raises -> PythonObject:
    """Create a heap-allocated Runtime via WASM."""
    return w.runtime_create()


fn _destroy_runtime(w: PythonObject, rt: PythonObject) raises:
    """Destroy a heap-allocated Runtime via WASM."""
    w.runtime_destroy(rt)


# ── Registry — initial state ─────────────────────────────────────────────────


fn test_registry_initial_state() raises:
    var w = _get_wasm()
    var rt = _create_runtime(w)

    assert_equal(Int(w.handler_count(rt)), 0, "new registry has 0 handlers")

    _destroy_runtime(w, rt)


# ── Registry — register and query ────────────────────────────────────────────


fn test_register_single_handler() raises:
    var w = _get_wasm()
    var rt = _create_runtime(w)

    var s = Int(w.scope_create(rt, 0, -1))
    var sig = Int(w.signal_create_i32(rt, 0))
    var id = Int(
        w.handler_register_signal_add(
            rt, s, sig, 1, w.write_string_struct("click")
        )
    )

    assert_equal(id, 0, "first handler gets id 0")
    assert_equal(Int(w.handler_count(rt)), 1, "count is 1 after register")
    assert_equal(
        Int(w.handler_contains(rt, id)), 1, "registry contains the handler"
    )

    _destroy_runtime(w, rt)


fn test_register_multiple_handlers() raises:
    var w = _get_wasm()
    var rt = _create_runtime(w)

    var s0 = Int(w.scope_create(rt, 0, -1))
    var s1 = Int(w.scope_create(rt, 0, -1))
    var sig0 = Int(w.signal_create_i32(rt, 0))
    var sig1 = Int(w.signal_create_i32(rt, 0))
    var sig2 = Int(w.signal_create_i32(rt, 0))

    var id0 = Int(
        w.handler_register_signal_set(
            rt, s0, sig0, 42, w.write_string_struct("click")
        )
    )
    var id1 = Int(
        w.handler_register_signal_add(
            rt, s0, sig1, 1, w.write_string_struct("click")
        )
    )
    var id2 = Int(
        w.handler_register_signal_sub(
            rt, s1, sig2, 5, w.write_string_struct("input")
        )
    )

    assert_equal(Int(w.handler_count(rt)), 3, "count is 3 after 3 registers")
    assert_true(id0 != id1, "id0 != id1")
    assert_true(id1 != id2, "id1 != id2")
    assert_true(id0 != id2, "id0 != id2")
    assert_equal(Int(w.handler_contains(rt, id0)), 1, "contains id0")
    assert_equal(Int(w.handler_contains(rt, id1)), 1, "contains id1")
    assert_equal(Int(w.handler_contains(rt, id2)), 1, "contains id2")

    _destroy_runtime(w, rt)


# ── Registry — query fields ──────────────────────────────────────────────────


fn test_query_signal_add_fields() raises:
    var w = _get_wasm()
    var rt = _create_runtime(w)

    var s = Int(w.scope_create(rt, 0, -1))
    var sig = Int(w.signal_create_i32(rt, 0))

    # ACTION_SIGNAL_ADD_I32 = 2
    var id = Int(
        w.handler_register_signal_add(
            rt, s, sig, 10, w.write_string_struct("click")
        )
    )

    assert_equal(Int(w.handler_scope_id(rt, id)), s, "scope_id matches")
    assert_equal(Int(w.handler_action(rt, id)), 2, "action is ADD (2)")
    assert_equal(Int(w.handler_signal_key(rt, id)), sig, "signal_key matches")
    assert_equal(Int(w.handler_operand(rt, id)), 10, "operand is 10")

    _destroy_runtime(w, rt)


fn test_query_signal_set_fields() raises:
    var w = _get_wasm()
    var rt = _create_runtime(w)

    var s = Int(w.scope_create(rt, 0, -1))
    var sig = Int(w.signal_create_i32(rt, 0))

    # ACTION_SIGNAL_SET_I32 = 1
    var id = Int(
        w.handler_register_signal_set(
            rt, s, sig, 99, w.write_string_struct("submit")
        )
    )

    assert_equal(Int(w.handler_action(rt, id)), 1, "action is SET (1)")
    assert_equal(Int(w.handler_signal_key(rt, id)), sig, "signal_key matches")
    assert_equal(Int(w.handler_operand(rt, id)), 99, "operand is 99")

    _destroy_runtime(w, rt)


fn test_query_signal_sub_fields() raises:
    var w = _get_wasm()
    var rt = _create_runtime(w)

    var s = Int(w.scope_create(rt, 0, -1))
    var sig = Int(w.signal_create_i32(rt, 0))

    # ACTION_SIGNAL_SUB_I32 = 3
    var id = Int(
        w.handler_register_signal_sub(
            rt, s, sig, 7, w.write_string_struct("click")
        )
    )

    assert_equal(Int(w.handler_action(rt, id)), 3, "action is SUB (3)")
    assert_equal(Int(w.handler_operand(rt, id)), 7, "operand (delta) is 7")

    _destroy_runtime(w, rt)


fn test_query_signal_toggle_fields() raises:
    var w = _get_wasm()
    var rt = _create_runtime(w)

    var s = Int(w.scope_create(rt, 0, -1))
    var sig = Int(w.signal_create_i32(rt, 0))

    # ACTION_SIGNAL_TOGGLE = 4
    var id = Int(
        w.handler_register_signal_toggle(
            rt, s, sig, w.write_string_struct("click")
        )
    )

    assert_equal(Int(w.handler_action(rt, id)), 4, "action is TOGGLE (4)")
    assert_equal(Int(w.handler_signal_key(rt, id)), sig, "signal_key matches")
    assert_equal(Int(w.handler_operand(rt, id)), 0, "operand is 0 for toggle")

    _destroy_runtime(w, rt)


fn test_query_signal_set_input_fields() raises:
    var w = _get_wasm()
    var rt = _create_runtime(w)

    var s = Int(w.scope_create(rt, 0, -1))
    var sig = Int(w.signal_create_i32(rt, 0))

    # ACTION_SIGNAL_SET_INPUT = 5
    var id = Int(
        w.handler_register_signal_set_input(
            rt, s, sig, w.write_string_struct("input")
        )
    )

    assert_equal(Int(w.handler_action(rt, id)), 5, "action is SET_INPUT (5)")
    assert_equal(Int(w.handler_signal_key(rt, id)), sig, "signal_key matches")

    _destroy_runtime(w, rt)


fn test_query_custom_handler_fields() raises:
    var w = _get_wasm()
    var rt = _create_runtime(w)

    var s = Int(w.scope_create(rt, 0, -1))

    # ACTION_CUSTOM = 255
    var id = Int(
        w.handler_register_custom(rt, s, w.write_string_struct("custom-event"))
    )

    assert_equal(Int(w.handler_action(rt, id)), 255, "action is CUSTOM (255)")
    assert_equal(
        Int(w.handler_signal_key(rt, id)), 0, "signal_key is 0 for custom"
    )
    assert_equal(Int(w.handler_operand(rt, id)), 0, "operand is 0 for custom")

    _destroy_runtime(w, rt)


fn test_query_noop_handler_fields() raises:
    var w = _get_wasm()
    var rt = _create_runtime(w)

    var s = Int(w.scope_create(rt, 0, -1))

    # ACTION_NONE = 0
    var id = Int(w.handler_register_noop(rt, s, w.write_string_struct("blur")))

    assert_equal(Int(w.handler_action(rt, id)), 0, "action is NONE (0)")
    assert_equal(Int(w.handler_scope_id(rt, id)), s, "scope_id matches")

    _destroy_runtime(w, rt)


# ── Registry — remove ────────────────────────────────────────────────────────


fn test_remove_handler() raises:
    var w = _get_wasm()
    var rt = _create_runtime(w)

    var s = Int(w.scope_create(rt, 0, -1))
    var sig0 = Int(w.signal_create_i32(rt, 0))
    var sig1 = Int(w.signal_create_i32(rt, 0))

    var id0 = Int(
        w.handler_register_signal_add(
            rt, s, sig0, 1, w.write_string_struct("click")
        )
    )
    var id1 = Int(
        w.handler_register_signal_set(
            rt, s, sig1, 42, w.write_string_struct("click")
        )
    )

    assert_equal(Int(w.handler_count(rt)), 2, "2 handlers before remove")

    w.handler_remove(rt, id0)

    assert_equal(Int(w.handler_count(rt)), 1, "1 handler after remove")
    assert_equal(
        Int(w.handler_contains(rt, id0)), 0, "removed handler not found"
    )
    assert_equal(
        Int(w.handler_contains(rt, id1)), 1, "other handler still exists"
    )

    _destroy_runtime(w, rt)


fn test_remove_nonexistent_is_noop() raises:
    var w = _get_wasm()
    var rt = _create_runtime(w)

    var s = Int(w.scope_create(rt, 0, -1))
    _ = w.handler_register_noop(rt, s, w.write_string_struct("click"))
    assert_equal(Int(w.handler_count(rt)), 1)

    # Remove an ID that was never registered
    w.handler_remove(rt, 99)
    assert_equal(
        Int(w.handler_count(rt)),
        1,
        "count unchanged after removing nonexistent",
    )

    _destroy_runtime(w, rt)


fn test_double_remove_is_noop() raises:
    var w = _get_wasm()
    var rt = _create_runtime(w)

    var s = Int(w.scope_create(rt, 0, -1))
    var id = Int(w.handler_register_noop(rt, s, w.write_string_struct("click")))
    w.handler_remove(rt, id)
    assert_equal(Int(w.handler_count(rt)), 0, "count is 0 after remove")

    w.handler_remove(rt, id)  # double remove
    assert_equal(
        Int(w.handler_count(rt)), 0, "count still 0 after double remove"
    )

    _destroy_runtime(w, rt)


# ── Registry — slot reuse after remove ───────────────────────────────────────


fn test_slot_reuse_after_remove() raises:
    var w = _get_wasm()
    var rt = _create_runtime(w)

    var s0 = Int(w.scope_create(rt, 0, -1))
    var s1 = Int(w.scope_create(rt, 0, -1))
    var sig0 = Int(w.signal_create_i32(rt, 0))
    var sig1 = Int(w.signal_create_i32(rt, 0))
    var sig3 = Int(w.signal_create_i32(rt, 0))

    var id0 = Int(
        w.handler_register_signal_set(
            rt, s0, sig0, 1, w.write_string_struct("click")
        )
    )
    var id1 = Int(
        w.handler_register_signal_set(
            rt, s0, sig1, 2, w.write_string_struct("click")
        )
    )

    w.handler_remove(rt, id0)

    # New registration should reuse the freed slot
    var id2 = Int(
        w.handler_register_signal_add(
            rt, s1, sig3, 99, w.write_string_struct("input")
        )
    )

    assert_equal(id2, id0, "new handler reuses freed slot")
    assert_equal(Int(w.handler_count(rt)), 2, "count is 2")
    assert_equal(Int(w.handler_contains(rt, id2)), 1, "reused slot is alive")

    # Verify the new handler's data
    assert_equal(Int(w.handler_action(rt, id2)), 2, "action is ADD (2)")
    assert_equal(Int(w.handler_signal_key(rt, id2)), sig3)
    assert_equal(Int(w.handler_operand(rt, id2)), 99)

    # Original id1 should be unchanged
    assert_equal(Int(w.handler_contains(rt, id1)), 1, "id1 still exists")
    assert_equal(Int(w.handler_operand(rt, id1)), 2, "id1 operand unchanged")

    _destroy_runtime(w, rt)


fn test_multiple_slot_reuse() raises:
    var w = _get_wasm()
    var rt = _create_runtime(w)

    var scopes = List[Int]()
    for i in range(5):
        scopes.append(Int(w.scope_create(rt, 0, -1)))

    var ids = List[Int]()
    for i in range(5):
        ids.append(
            Int(
                w.handler_register_noop(
                    rt, scopes[i], w.write_string_struct("click")
                )
            )
        )
    assert_equal(Int(w.handler_count(rt)), 5)

    # Remove all even-indexed
    w.handler_remove(rt, ids[0])
    w.handler_remove(rt, ids[2])
    w.handler_remove(rt, ids[4])
    assert_equal(Int(w.handler_count(rt)), 2)

    # Re-register 3 more — should reuse freed slots
    var new_ids = List[Int]()
    for i in range(3):
        var sig = Int(w.signal_create_i32(rt, 0))
        new_ids.append(
            Int(
                w.handler_register_signal_set(
                    rt,
                    scopes[0],
                    sig,
                    i * 10,
                    w.write_string_struct("input"),
                )
            )
        )
    assert_equal(Int(w.handler_count(rt)), 5, "back to 5 after reuse")

    # All new IDs should be from the set {ids[0], ids[2], ids[4]}
    for i in range(3):
        var nid = new_ids[i]
        assert_true(
            nid == ids[0] or nid == ids[2] or nid == ids[4],
            "new id " + String(nid) + " should be a reused slot",
        )

    _destroy_runtime(w, rt)


# ── Registry — contains with out-of-bounds ID ────────────────────────────────


fn test_contains_out_of_bounds() raises:
    var w = _get_wasm()
    var rt = _create_runtime(w)

    assert_equal(
        Int(w.handler_contains(rt, 0)),
        0,
        "empty registry: contains(0) is false",
    )
    assert_equal(
        Int(w.handler_contains(rt, 100)),
        0,
        "empty registry: contains(100) is false",
    )

    _destroy_runtime(w, rt)


# ── Dispatch — signal_add ────────────────────────────────────────────────────


fn test_dispatch_signal_add() raises:
    var w = _get_wasm()
    var rt = _create_runtime(w)

    var s = Int(w.scope_create(rt, 0, -1))
    var sig = Int(w.signal_create_i32(rt, 10))

    var id = Int(
        w.handler_register_signal_add(
            rt, s, sig, 5, w.write_string_struct("click")
        )
    )

    # EVT_CLICK = 0
    var result = Int(w.dispatch_event(rt, id, 0))
    assert_equal(result, 1, "dispatch returns 1 (action executed)")

    # Signal should have been incremented by 5
    assert_equal(
        Int(w.signal_peek_i32(rt, sig)),
        15,
        "signal is 15 after adding 5 to 10",
    )

    _destroy_runtime(w, rt)


# ── Dispatch — signal_sub ────────────────────────────────────────────────────


fn test_dispatch_signal_sub() raises:
    var w = _get_wasm()
    var rt = _create_runtime(w)

    var s = Int(w.scope_create(rt, 0, -1))
    var sig = Int(w.signal_create_i32(rt, 100))

    var id = Int(
        w.handler_register_signal_sub(
            rt, s, sig, 30, w.write_string_struct("click")
        )
    )

    _ = w.dispatch_event(rt, id, 0)
    assert_equal(
        Int(w.signal_peek_i32(rt, sig)),
        70,
        "signal is 70 after subtracting 30 from 100",
    )

    _destroy_runtime(w, rt)


# ── Dispatch — signal_set ────────────────────────────────────────────────────


fn test_dispatch_signal_set() raises:
    var w = _get_wasm()
    var rt = _create_runtime(w)

    var s = Int(w.scope_create(rt, 0, -1))
    var sig = Int(w.signal_create_i32(rt, 0))

    var id = Int(
        w.handler_register_signal_set(
            rt, s, sig, 42, w.write_string_struct("click")
        )
    )

    _ = w.dispatch_event(rt, id, 0)
    assert_equal(Int(w.signal_peek_i32(rt, sig)), 42, "signal is 42 after set")

    _destroy_runtime(w, rt)


# ── Dispatch — signal_toggle ─────────────────────────────────────────────────


fn test_dispatch_signal_toggle() raises:
    var w = _get_wasm()
    var rt = _create_runtime(w)

    var s = Int(w.scope_create(rt, 0, -1))
    var sig = Int(w.signal_create_i32(rt, 0))

    var id = Int(
        w.handler_register_signal_toggle(
            rt, s, sig, w.write_string_struct("click")
        )
    )

    # Toggle 0 → 1
    _ = w.dispatch_event(rt, id, 0)
    assert_equal(
        Int(w.signal_peek_i32(rt, sig)), 1, "signal toggled from 0 to 1"
    )

    # Toggle 1 → 0
    _ = w.dispatch_event(rt, id, 0)
    assert_equal(
        Int(w.signal_peek_i32(rt, sig)), 0, "signal toggled from 1 to 0"
    )

    _destroy_runtime(w, rt)


# ── Dispatch — signal_set_input (with i32 payload) ───────────────────────────


fn test_dispatch_signal_set_input() raises:
    var w = _get_wasm()
    var rt = _create_runtime(w)

    var s = Int(w.scope_create(rt, 0, -1))
    var sig = Int(w.signal_create_i32(rt, 0))

    var id = Int(
        w.handler_register_signal_set_input(
            rt, s, sig, w.write_string_struct("input")
        )
    )

    # EVT_INPUT = 1, payload = 77
    var result = Int(w.dispatch_event_with_i32(rt, id, 1, 77))
    assert_equal(result, 1, "dispatch returns 1")
    assert_equal(
        Int(w.signal_peek_i32(rt, sig)), 77, "signal set to input value 77"
    )

    _destroy_runtime(w, rt)


# ── Dispatch — marks scope dirty ─────────────────────────────────────────────


fn test_dispatch_marks_scope_dirty() raises:
    var w = _get_wasm()
    var rt = _create_runtime(w)

    var s = Int(w.scope_create(rt, 0, -1))
    var sig = Int(w.signal_create_i32(rt, 0))

    # Subscribe the scope to the signal (via render + read)
    var prev = Int(w.scope_begin_render(rt, s))
    _ = w.signal_read_i32(rt, sig)
    w.scope_end_render(rt, prev)

    assert_equal(Int(w.runtime_has_dirty(rt)), 0, "no dirty scopes initially")

    var id = Int(
        w.handler_register_signal_add(
            rt, s, sig, 1, w.write_string_struct("click")
        )
    )

    _ = w.dispatch_event(rt, id, 0)
    assert_equal(
        Int(w.runtime_has_dirty(rt)), 1, "scope is dirty after dispatch"
    )

    _destroy_runtime(w, rt)


# ── Dispatch — multiple dispatches accumulate ────────────────────────────────


fn test_dispatch_multiple_accumulate() raises:
    var w = _get_wasm()
    var rt = _create_runtime(w)

    var s = Int(w.scope_create(rt, 0, -1))
    var sig = Int(w.signal_create_i32(rt, 0))

    var id = Int(
        w.handler_register_signal_add(
            rt, s, sig, 1, w.write_string_struct("click")
        )
    )

    # Dispatch 10 times
    for _ in range(10):
        _ = w.dispatch_event(rt, id, 0)

    assert_equal(
        Int(w.signal_peek_i32(rt, sig)),
        10,
        "signal is 10 after 10 dispatches adding 1",
    )

    _destroy_runtime(w, rt)


# ── Dispatch — drain dirty ───────────────────────────────────────────────────


fn test_dispatch_and_drain_dirty() raises:
    var w = _get_wasm()
    var rt = _create_runtime(w)

    var s = Int(w.scope_create(rt, 0, -1))
    var sig = Int(w.signal_create_i32(rt, 0))

    # Subscribe
    var prev = Int(w.scope_begin_render(rt, s))
    _ = w.signal_read_i32(rt, sig)
    w.scope_end_render(rt, prev)

    var id = Int(
        w.handler_register_signal_set(
            rt, s, sig, 42, w.write_string_struct("click")
        )
    )

    _ = w.dispatch_event(rt, id, 0)
    assert_equal(Int(w.runtime_dirty_count(rt)), 1, "1 dirty scope")

    var drained = Int(w.runtime_drain_dirty(rt))
    assert_equal(drained, 1, "drained 1 dirty scope")
    assert_equal(Int(w.runtime_has_dirty(rt)), 0, "no dirty scopes after drain")

    _destroy_runtime(w, rt)


# ── Edge case — negative operand ─────────────────────────────────────────────


fn test_negative_operand() raises:
    var w = _get_wasm()
    var rt = _create_runtime(w)

    var s = Int(w.scope_create(rt, 0, -1))
    var sig = Int(w.signal_create_i32(rt, 0))

    var id = Int(
        w.handler_register_signal_add(
            rt, s, sig, -100, w.write_string_struct("click")
        )
    )

    assert_equal(
        Int(w.handler_operand(rt, id)), -100, "negative operand preserved"
    )

    # Dispatch — should add -100
    _ = w.dispatch_event(rt, id, 0)
    assert_equal(
        Int(w.signal_peek_i32(rt, sig)),
        -100,
        "signal is -100 after adding -100 to 0",
    )

    _destroy_runtime(w, rt)


fn test_int32_min_max_operand() raises:
    var w = _get_wasm()
    var rt = _create_runtime(w)

    var s = Int(w.scope_create(rt, 0, -1))
    var sig0 = Int(w.signal_create_i32(rt, 0))
    var sig1 = Int(w.signal_create_i32(rt, 0))

    var id_min = Int(
        w.handler_register_signal_set(
            rt, s, sig0, -2147483648, w.write_string_struct("a")
        )
    )
    var id_max = Int(
        w.handler_register_signal_set(
            rt, s, sig1, 2147483647, w.write_string_struct("b")
        )
    )

    assert_equal(
        Int(w.handler_operand(rt, id_min)), -2147483648, "INT32_MIN operand"
    )
    assert_equal(
        Int(w.handler_operand(rt, id_max)), 2147483647, "INT32_MAX operand"
    )

    _destroy_runtime(w, rt)


# ── Stress — many handlers ───────────────────────────────────────────────────


fn test_stress_100_handlers() raises:
    var w = _get_wasm()
    var rt = _create_runtime(w)

    var scopes = List[Int]()
    for i in range(10):
        scopes.append(Int(w.scope_create(rt, 0, -1)))

    var ids = List[Int]()
    for i in range(100):
        var sig = Int(w.signal_create_i32(rt, 0))
        ids.append(
            Int(
                w.handler_register_signal_set(
                    rt,
                    scopes[i % 10],
                    sig,
                    i * 10,
                    w.write_string_struct("click"),
                )
            )
        )

    assert_equal(Int(w.handler_count(rt)), 100, "100 handlers registered")

    # Verify all are alive and have correct data
    for i in range(100):
        assert_equal(
            Int(w.handler_contains(rt, ids[i])),
            1,
            "handler " + String(i) + " is alive",
        )
        assert_equal(
            Int(w.handler_operand(rt, ids[i])),
            i * 10,
            "handler " + String(i) + " operand correct",
        )

    _destroy_runtime(w, rt)


fn test_stress_register_remove_cycle() raises:
    """Register and remove handlers in a tight loop to exercise free list."""
    var w = _get_wasm()
    var rt = _create_runtime(w)

    var s = Int(w.scope_create(rt, 0, -1))

    for _ in range(500):
        var id = Int(
            w.handler_register_noop(rt, s, w.write_string_struct("click"))
        )
        assert_equal(Int(w.handler_contains(rt, id)), 1)
        w.handler_remove(rt, id)

    assert_equal(
        Int(w.handler_count(rt)),
        0,
        "count is 0 after 500 register/remove cycles",
    )

    # The next alloc should reuse slot 0
    var id = Int(w.handler_register_noop(rt, s, w.write_string_struct("click")))
    assert_equal(id, 0, "first slot reused after cycle")
    assert_equal(Int(w.handler_count(rt)), 1)

    _destroy_runtime(w, rt)


# ── Dispatch — full counter scenario ─────────────────────────────────────────


fn test_dispatch_counter_scenario() raises:
    """Simulate a counter component: scope with signal + click handler."""
    var w = _get_wasm()
    var rt = _create_runtime(w)

    # Create scope and signal
    var s = Int(w.scope_create(rt, 0, -1))
    var count = Int(w.signal_create_i32(rt, 0))

    # Render — subscribe scope to signal
    var prev = Int(w.scope_begin_render(rt, s))
    var key = Int(w.hook_use_signal_i32(rt, 0))
    _ = w.signal_read_i32(rt, key)
    w.scope_end_render(rt, prev)

    # Register increment handler
    var inc_handler = Int(
        w.handler_register_signal_add(
            rt, s, key, 1, w.write_string_struct("click")
        )
    )
    # Register decrement handler
    var dec_handler = Int(
        w.handler_register_signal_sub(
            rt, s, key, 1, w.write_string_struct("click")
        )
    )

    # Click + 3 times
    for _ in range(3):
        _ = w.dispatch_event(rt, inc_handler, 0)

    assert_equal(
        Int(w.signal_peek_i32(rt, key)),
        3,
        "count is 3 after 3 increments",
    )

    # Click - 1 time
    _ = w.dispatch_event(rt, dec_handler, 0)
    assert_equal(
        Int(w.signal_peek_i32(rt, key)),
        2,
        "count is 2 after decrement",
    )

    # Verify scope was dirtied
    assert_equal(Int(w.runtime_has_dirty(rt)), 1, "scope is dirty")

    # Drain dirty and re-render
    _ = w.runtime_drain_dirty(rt)
    prev = Int(w.scope_begin_render(rt, s))
    var key2 = Int(w.hook_use_signal_i32(rt, 0))
    assert_equal(key2, key, "same signal key on re-render")
    assert_equal(Int(w.signal_read_i32(rt, key2)), 2, "count is 2 on re-render")
    w.scope_end_render(rt, prev)

    _destroy_runtime(w, rt)

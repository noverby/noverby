# Tests for HandlerRegistry — native Mojo tests run with `mojo test`.
#
# These tests are a direct port of test/events.test.ts, exercising
# the same scenarios without the WASM/JS round-trip.
#
# Run with:
#   mojo test -I src test-mojo/test_events.mojo

from testing import assert_equal, assert_true, assert_false

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


# ── Registry — initial state ─────────────────────────────────────────────────


fn test_registry_initial_state() raises:
    var reg = HandlerRegistry()
    assert_equal(reg.count(), 0, "new registry has 0 handlers")


# ── Registry — register and query ────────────────────────────────────────────


fn test_register_single_handler() raises:
    var reg = HandlerRegistry()

    var entry = HandlerEntry.signal_add(
        scope_id=UInt32(1),
        signal_key=UInt32(0),
        delta=Int32(1),
        event_name=String("click"),
    )
    var id = reg.register(entry)

    assert_equal(Int(id), 0, "first handler gets id 0")
    assert_equal(reg.count(), 1, "count is 1 after register")
    assert_true(reg.contains(id), "registry contains the handler")


fn test_register_multiple_handlers() raises:
    var reg = HandlerRegistry()

    var id0 = reg.register(
        HandlerEntry.signal_set(
            UInt32(0), UInt32(0), Int32(42), String("click")
        )
    )
    var id1 = reg.register(
        HandlerEntry.signal_add(UInt32(0), UInt32(1), Int32(1), String("click"))
    )
    var id2 = reg.register(
        HandlerEntry.signal_sub(UInt32(1), UInt32(2), Int32(5), String("input"))
    )

    assert_equal(reg.count(), 3, "count is 3 after 3 registers")
    assert_true(Int(id0) != Int(id1), "id0 != id1")
    assert_true(Int(id1) != Int(id2), "id1 != id2")
    assert_true(Int(id0) != Int(id2), "id0 != id2")
    assert_true(reg.contains(id0), "contains id0")
    assert_true(reg.contains(id1), "contains id1")
    assert_true(reg.contains(id2), "contains id2")


# ── Registry — query fields ──────────────────────────────────────────────────


fn test_query_handler_fields() raises:
    var reg = HandlerRegistry()

    var entry = HandlerEntry.signal_add(
        scope_id=UInt32(7),
        signal_key=UInt32(3),
        delta=Int32(10),
        event_name=String("click"),
    )
    var id = reg.register(entry)

    assert_equal(Int(reg.scope_id(id)), 7, "scope_id is 7")
    assert_equal(
        Int(reg.action(id)), Int(ACTION_SIGNAL_ADD_I32), "action is ADD"
    )
    assert_equal(Int(reg.signal_key(id)), 3, "signal_key is 3")
    assert_equal(Int(reg.operand(id)), 10, "operand is 10")
    assert_equal(reg.event_name(id), "click", "event_name is 'click'")


fn test_query_signal_set_fields() raises:
    var reg = HandlerRegistry()

    var entry = HandlerEntry.signal_set(
        scope_id=UInt32(2),
        signal_key=UInt32(5),
        value=Int32(99),
        event_name=String("submit"),
    )
    var id = reg.register(entry)

    assert_equal(
        Int(reg.action(id)), Int(ACTION_SIGNAL_SET_I32), "action is SET"
    )
    assert_equal(Int(reg.signal_key(id)), 5, "signal_key is 5")
    assert_equal(Int(reg.operand(id)), 99, "operand is 99")
    assert_equal(reg.event_name(id), "submit", "event_name is 'submit'")


fn test_query_signal_sub_fields() raises:
    var reg = HandlerRegistry()

    var entry = HandlerEntry.signal_sub(
        scope_id=UInt32(3),
        signal_key=UInt32(1),
        delta=Int32(7),
        event_name=String("click"),
    )
    var id = reg.register(entry)

    assert_equal(
        Int(reg.action(id)), Int(ACTION_SIGNAL_SUB_I32), "action is SUB"
    )
    assert_equal(Int(reg.operand(id)), 7, "operand (delta) is 7")


fn test_query_signal_toggle_fields() raises:
    var reg = HandlerRegistry()

    var entry = HandlerEntry.signal_toggle(
        scope_id=UInt32(1),
        signal_key=UInt32(0),
        event_name=String("click"),
    )
    var id = reg.register(entry)

    assert_equal(
        Int(reg.action(id)), Int(ACTION_SIGNAL_TOGGLE), "action is TOGGLE"
    )
    assert_equal(Int(reg.signal_key(id)), 0, "signal_key is 0")
    assert_equal(Int(reg.operand(id)), 0, "operand is 0 for toggle")


fn test_query_signal_set_input_fields() raises:
    var reg = HandlerRegistry()

    var entry = HandlerEntry.signal_set_input(
        scope_id=UInt32(4),
        signal_key=UInt32(2),
        event_name=String("input"),
    )
    var id = reg.register(entry)

    assert_equal(
        Int(reg.action(id)),
        Int(ACTION_SIGNAL_SET_INPUT),
        "action is SET_INPUT",
    )
    assert_equal(Int(reg.signal_key(id)), 2, "signal_key is 2")
    assert_equal(reg.event_name(id), "input", "event_name is 'input'")


fn test_query_custom_handler_fields() raises:
    var reg = HandlerRegistry()

    var entry = HandlerEntry.custom(
        scope_id=UInt32(9),
        event_name=String("custom-event"),
    )
    var id = reg.register(entry)

    assert_equal(Int(reg.action(id)), Int(ACTION_CUSTOM), "action is CUSTOM")
    assert_equal(Int(reg.signal_key(id)), 0, "signal_key is 0 for custom")
    assert_equal(Int(reg.operand(id)), 0, "operand is 0 for custom")
    assert_equal(
        reg.event_name(id), "custom-event", "event_name is 'custom-event'"
    )


fn test_query_noop_handler_fields() raises:
    var reg = HandlerRegistry()

    var entry = HandlerEntry.noop(
        scope_id=UInt32(5),
        event_name=String("blur"),
    )
    var id = reg.register(entry)

    assert_equal(Int(reg.action(id)), Int(ACTION_NONE), "action is NONE")
    assert_equal(Int(reg.scope_id(id)), 5, "scope_id is 5")
    assert_equal(reg.event_name(id), "blur", "event_name is 'blur'")


# ── Registry — get (copy) ───────────────────────────────────────────────────


fn test_get_returns_copy() raises:
    var reg = HandlerRegistry()

    var entry = HandlerEntry.signal_add(
        UInt32(1), UInt32(0), Int32(1), String("click")
    )
    var id = reg.register(entry)

    var got = reg.get(id)
    assert_equal(Int(got.scope_id), 1, "got.scope_id is 1")
    assert_equal(
        Int(got.action), Int(ACTION_SIGNAL_ADD_I32), "got.action is ADD"
    )
    assert_equal(Int(got.signal_key), 0, "got.signal_key is 0")
    assert_equal(Int(got.operand), 1, "got.operand is 1")
    assert_equal(got.event_name, "click", "got.event_name is 'click'")


# ── Registry — remove ────────────────────────────────────────────────────────


fn test_remove_handler() raises:
    var reg = HandlerRegistry()

    var id0 = reg.register(
        HandlerEntry.signal_add(UInt32(0), UInt32(0), Int32(1), String("click"))
    )
    var id1 = reg.register(
        HandlerEntry.signal_set(
            UInt32(0), UInt32(1), Int32(42), String("click")
        )
    )

    assert_equal(reg.count(), 2, "2 handlers before remove")

    reg.remove(id0)

    assert_equal(reg.count(), 1, "1 handler after remove")
    assert_false(reg.contains(id0), "removed handler not found")
    assert_true(reg.contains(id1), "other handler still exists")


fn test_remove_nonexistent_is_noop() raises:
    var reg = HandlerRegistry()

    _ = reg.register(HandlerEntry.noop(UInt32(0), String("click")))
    assert_equal(reg.count(), 1)

    # Remove an ID that was never registered
    reg.remove(UInt32(99))
    assert_equal(reg.count(), 1, "count unchanged after removing nonexistent")


fn test_double_remove_is_noop() raises:
    var reg = HandlerRegistry()

    var id = reg.register(HandlerEntry.noop(UInt32(0), String("click")))
    reg.remove(id)
    assert_equal(reg.count(), 0, "count is 0 after remove")

    reg.remove(id)  # double remove
    assert_equal(reg.count(), 0, "count still 0 after double remove")


# ── Registry — slot reuse after remove ───────────────────────────────────────


fn test_slot_reuse_after_remove() raises:
    var reg = HandlerRegistry()

    var id0 = reg.register(
        HandlerEntry.signal_set(UInt32(0), UInt32(0), Int32(1), String("click"))
    )
    var id1 = reg.register(
        HandlerEntry.signal_set(UInt32(0), UInt32(1), Int32(2), String("click"))
    )

    reg.remove(id0)

    # New registration should reuse the freed slot
    var id2 = reg.register(
        HandlerEntry.signal_add(
            UInt32(1), UInt32(3), Int32(99), String("input")
        )
    )

    assert_equal(Int(id2), Int(id0), "new handler reuses freed slot")
    assert_equal(reg.count(), 2, "count is 2")
    assert_true(reg.contains(id2), "reused slot is alive")

    # Verify the new handler's data
    assert_equal(Int(reg.action(id2)), Int(ACTION_SIGNAL_ADD_I32))
    assert_equal(Int(reg.signal_key(id2)), 3)
    assert_equal(Int(reg.operand(id2)), 99)
    assert_equal(reg.event_name(id2), "input")

    # Original id1 should be unchanged
    assert_true(reg.contains(id1), "id1 still exists")
    assert_equal(Int(reg.operand(id1)), 2, "id1 operand unchanged")


fn test_multiple_slot_reuse() raises:
    var reg = HandlerRegistry()

    var ids = List[UInt32]()
    for i in range(5):
        ids.append(reg.register(HandlerEntry.noop(UInt32(i), String("click"))))
    assert_equal(reg.count(), 5)

    # Remove all even-indexed
    reg.remove(ids[0])
    reg.remove(ids[2])
    reg.remove(ids[4])
    assert_equal(reg.count(), 2)

    # Re-register 3 more — should reuse freed slots
    var new_ids = List[UInt32]()
    for i in range(3):
        new_ids.append(
            reg.register(
                HandlerEntry.signal_set(
                    UInt32(10 + i), UInt32(0), Int32(i), String("input")
                )
            )
        )
    assert_equal(reg.count(), 5, "back to 5 after reuse")

    # All new IDs should be from the set {ids[0], ids[2], ids[4]}
    for i in range(3):
        var nid = new_ids[i]
        assert_true(
            Int(nid) == Int(ids[0])
            or Int(nid) == Int(ids[2])
            or Int(nid) == Int(ids[4]),
            "new id " + String(Int(nid)) + " should be a reused slot",
        )


# ── Registry — remove_for_scope ──────────────────────────────────────────────


fn test_remove_for_scope() raises:
    var reg = HandlerRegistry()

    # Register handlers across two scopes
    var s1_id0 = reg.register(
        HandlerEntry.signal_add(UInt32(1), UInt32(0), Int32(1), String("click"))
    )
    var s1_id1 = reg.register(
        HandlerEntry.signal_set(
            UInt32(1), UInt32(1), Int32(42), String("input")
        )
    )
    var s2_id0 = reg.register(
        HandlerEntry.signal_sub(UInt32(2), UInt32(2), Int32(5), String("click"))
    )

    assert_equal(reg.count(), 3)

    # Remove all handlers for scope 1
    reg.remove_for_scope(UInt32(1))

    assert_equal(reg.count(), 1, "only scope 2 handler remains")
    assert_false(reg.contains(s1_id0), "scope 1 handler 0 removed")
    assert_false(reg.contains(s1_id1), "scope 1 handler 1 removed")
    assert_true(reg.contains(s2_id0), "scope 2 handler still exists")


fn test_remove_for_scope_no_match() raises:
    var reg = HandlerRegistry()

    _ = reg.register(HandlerEntry.noop(UInt32(1), String("click")))
    _ = reg.register(HandlerEntry.noop(UInt32(2), String("click")))
    assert_equal(reg.count(), 2)

    # Remove for a scope that has no handlers
    reg.remove_for_scope(UInt32(99))
    assert_equal(reg.count(), 2, "count unchanged — no handlers for scope 99")


fn test_remove_for_scope_all() raises:
    var reg = HandlerRegistry()

    _ = reg.register(HandlerEntry.noop(UInt32(1), String("a")))
    _ = reg.register(HandlerEntry.noop(UInt32(1), String("b")))
    _ = reg.register(HandlerEntry.noop(UInt32(1), String("c")))
    assert_equal(reg.count(), 3)

    reg.remove_for_scope(UInt32(1))
    assert_equal(reg.count(), 0, "all handlers removed for scope 1")


# ── Registry — handlers_for_scope ────────────────────────────────────────────


fn test_handlers_for_scope() raises:
    var reg = HandlerRegistry()

    var id0 = reg.register(HandlerEntry.noop(UInt32(1), String("click")))
    _ = reg.register(HandlerEntry.noop(UInt32(2), String("input")))
    var id2 = reg.register(HandlerEntry.noop(UInt32(1), String("blur")))

    var scope1_handlers = reg.handlers_for_scope(UInt32(1))
    assert_equal(len(scope1_handlers), 2, "scope 1 has 2 handlers")

    # The returned IDs should be id0 and id2
    assert_true(
        (
            Int(scope1_handlers[0]) == Int(id0)
            and Int(scope1_handlers[1]) == Int(id2)
        )
        or (
            Int(scope1_handlers[0]) == Int(id2)
            and Int(scope1_handlers[1]) == Int(id0)
        ),
        "scope 1 handler IDs match",
    )


fn test_handlers_for_scope_empty() raises:
    var reg = HandlerRegistry()

    _ = reg.register(HandlerEntry.noop(UInt32(1), String("click")))

    var scope2_handlers = reg.handlers_for_scope(UInt32(2))
    assert_equal(len(scope2_handlers), 0, "scope 2 has 0 handlers")


# ── Registry — clear ─────────────────────────────────────────────────────────


fn test_clear() raises:
    var reg = HandlerRegistry()

    _ = reg.register(HandlerEntry.noop(UInt32(0), String("a")))
    _ = reg.register(HandlerEntry.noop(UInt32(0), String("b")))
    _ = reg.register(HandlerEntry.noop(UInt32(0), String("c")))
    assert_equal(reg.count(), 3)

    reg.clear()
    assert_equal(reg.count(), 0, "count is 0 after clear")
    assert_false(reg.contains(UInt32(0)), "slot 0 not alive after clear")
    assert_false(reg.contains(UInt32(1)), "slot 1 not alive after clear")
    assert_false(reg.contains(UInt32(2)), "slot 2 not alive after clear")


fn test_register_after_clear() raises:
    var reg = HandlerRegistry()

    _ = reg.register(HandlerEntry.noop(UInt32(0), String("a")))
    _ = reg.register(HandlerEntry.noop(UInt32(0), String("b")))
    reg.clear()

    var id = reg.register(
        HandlerEntry.signal_set(
            UInt32(5), UInt32(0), Int32(77), String("click")
        )
    )
    assert_equal(reg.count(), 1, "1 handler after clear + register")
    assert_equal(Int(id), 0, "first ID after clear is 0")
    assert_equal(Int(reg.scope_id(id)), 5, "new handler has correct scope_id")
    assert_equal(Int(reg.operand(id)), 77, "new handler has correct operand")


# ── Registry — contains with out-of-bounds ID ────────────────────────────────


fn test_contains_out_of_bounds() raises:
    var reg = HandlerRegistry()
    assert_false(
        reg.contains(UInt32(0)), "empty registry: contains(0) is false"
    )
    assert_false(
        reg.contains(UInt32(100)), "empty registry: contains(100) is false"
    )


# ── HandlerEntry — default constructor ───────────────────────────────────────


fn test_handler_entry_default() raises:
    var entry = HandlerEntry()
    assert_equal(Int(entry.scope_id), 0, "default scope_id is 0")
    assert_equal(Int(entry.action), Int(ACTION_NONE), "default action is NONE")
    assert_equal(Int(entry.signal_key), 0, "default signal_key is 0")
    assert_equal(Int(entry.operand), 0, "default operand is 0")
    assert_equal(entry.event_name, "", "default event_name is empty")


# ── HandlerEntry — copy ─────────────────────────────────────────────────────


fn test_handler_entry_copy() raises:
    var entry = HandlerEntry.signal_add(
        UInt32(3), UInt32(7), Int32(42), String("click")
    )
    var copy = entry.copy()

    assert_equal(Int(copy.scope_id), 3)
    assert_equal(Int(copy.action), Int(ACTION_SIGNAL_ADD_I32))
    assert_equal(Int(copy.signal_key), 7)
    assert_equal(Int(copy.operand), 42)
    assert_equal(copy.event_name, "click")


# ── Event type tag constants ─────────────────────────────────────────────────


fn test_event_type_constants() raises:
    """Verify event type tags have expected values matching the JS side."""
    assert_equal(Int(EVT_CLICK), 0)
    assert_equal(Int(EVT_INPUT), 1)
    assert_equal(Int(EVT_KEY_DOWN), 2)
    assert_equal(Int(EVT_KEY_UP), 3)
    assert_equal(Int(EVT_MOUSE_MOVE), 4)
    assert_equal(Int(EVT_FOCUS), 5)
    assert_equal(Int(EVT_BLUR), 6)
    assert_equal(Int(EVT_SUBMIT), 7)
    assert_equal(Int(EVT_CHANGE), 8)
    assert_equal(Int(EVT_MOUSE_DOWN), 9)
    assert_equal(Int(EVT_MOUSE_UP), 10)
    assert_equal(Int(EVT_MOUSE_ENTER), 11)
    assert_equal(Int(EVT_MOUSE_LEAVE), 12)
    assert_equal(Int(EVT_CUSTOM), 255)


# ── Action tag constants ─────────────────────────────────────────────────────


fn test_action_tag_constants() raises:
    """Verify action tags have expected values matching the JS side."""
    assert_equal(Int(ACTION_NONE), 0)
    assert_equal(Int(ACTION_SIGNAL_SET_I32), 1)
    assert_equal(Int(ACTION_SIGNAL_ADD_I32), 2)
    assert_equal(Int(ACTION_SIGNAL_SUB_I32), 3)
    assert_equal(Int(ACTION_SIGNAL_TOGGLE), 4)
    assert_equal(Int(ACTION_SIGNAL_SET_INPUT), 5)
    assert_equal(Int(ACTION_CUSTOM), 255)


# ── Stress — many handlers ───────────────────────────────────────────────────


fn test_stress_100_handlers() raises:
    var reg = HandlerRegistry()

    var ids = List[UInt32]()
    for i in range(100):
        ids.append(
            reg.register(
                HandlerEntry.signal_set(
                    UInt32(i % 10),
                    UInt32(i),
                    Int32(i * 10),
                    String("click"),
                )
            )
        )

    assert_equal(reg.count(), 100, "100 handlers registered")

    # Verify all are alive and have correct data
    for i in range(100):
        assert_true(reg.contains(ids[i]), "handler " + String(i) + " is alive")
        assert_equal(
            Int(reg.signal_key(ids[i])),
            i,
            "handler " + String(i) + " signal_key correct",
        )
        assert_equal(
            Int(reg.operand(ids[i])),
            i * 10,
            "handler " + String(i) + " operand correct",
        )


fn test_stress_register_remove_cycle() raises:
    """Register and remove handlers in a tight loop to exercise free list."""
    var reg = HandlerRegistry()

    for _ in range(500):
        var id = reg.register(HandlerEntry.noop(UInt32(0), String("click")))
        assert_true(reg.contains(id))
        reg.remove(id)

    assert_equal(reg.count(), 0, "count is 0 after 500 register/remove cycles")

    # The next alloc should reuse slot 0
    var id = reg.register(HandlerEntry.noop(UInt32(0), String("click")))
    assert_equal(Int(id), 0, "first slot reused after cycle")
    assert_equal(reg.count(), 1)


fn test_stress_remove_for_scope_selective() raises:
    """Register handlers across many scopes, then remove one scope at a time."""
    var reg = HandlerRegistry()

    # 10 handlers per scope, 5 scopes = 50 handlers
    for scope in range(5):
        for _ in range(10):
            _ = reg.register(HandlerEntry.noop(UInt32(scope), String("click")))

    assert_equal(reg.count(), 50)

    # Remove scope 2
    reg.remove_for_scope(UInt32(2))
    assert_equal(reg.count(), 40, "40 after removing scope 2")

    # Remove scope 0
    reg.remove_for_scope(UInt32(0))
    assert_equal(reg.count(), 30, "30 after removing scope 0")

    # Remaining should be scopes 1, 3, 4
    var s1 = reg.handlers_for_scope(UInt32(1))
    var s3 = reg.handlers_for_scope(UInt32(3))
    var s4 = reg.handlers_for_scope(UInt32(4))
    assert_equal(len(s1), 10, "scope 1 has 10 handlers")
    assert_equal(len(s3), 10, "scope 3 has 10 handlers")
    assert_equal(len(s4), 10, "scope 4 has 10 handlers")

    # Scopes 0 and 2 should have none
    var s0 = reg.handlers_for_scope(UInt32(0))
    var s2 = reg.handlers_for_scope(UInt32(2))
    assert_equal(len(s0), 0, "scope 0 has 0 handlers")
    assert_equal(len(s2), 0, "scope 2 has 0 handlers")


# ── Edge case — different event names on same scope ──────────────────────────


fn test_different_event_names_same_scope() raises:
    var reg = HandlerRegistry()

    var id_click = reg.register(
        HandlerEntry.signal_add(UInt32(1), UInt32(0), Int32(1), String("click"))
    )
    var id_input = reg.register(
        HandlerEntry.signal_set_input(UInt32(1), UInt32(1), String("input"))
    )
    var id_blur = reg.register(HandlerEntry.noop(UInt32(1), String("blur")))

    assert_equal(reg.event_name(id_click), "click")
    assert_equal(reg.event_name(id_input), "input")
    assert_equal(reg.event_name(id_blur), "blur")

    var scope_handlers = reg.handlers_for_scope(UInt32(1))
    assert_equal(len(scope_handlers), 3, "scope 1 has 3 handlers")


# ── Edge case — negative operand ─────────────────────────────────────────────


fn test_negative_operand() raises:
    var reg = HandlerRegistry()

    var entry = HandlerEntry.signal_add(
        UInt32(0), UInt32(0), Int32(-100), String("click")
    )
    var id = reg.register(entry)

    assert_equal(Int(reg.operand(id)), -100, "negative operand preserved")


fn test_int32_min_max_operand() raises:
    var reg = HandlerRegistry()

    var id_min = reg.register(
        HandlerEntry.signal_set(
            UInt32(0), UInt32(0), Int32(-2147483648), String("a")
        )
    )
    var id_max = reg.register(
        HandlerEntry.signal_set(
            UInt32(0), UInt32(0), Int32(2147483647), String("b")
        )
    )

    assert_equal(Int(reg.operand(id_min)), -2147483648, "INT32_MIN operand")
    assert_equal(Int(reg.operand(id_max)), 2147483647, "INT32_MAX operand")

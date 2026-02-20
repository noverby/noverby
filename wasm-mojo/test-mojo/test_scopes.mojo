# Tests for ScopeArena and scope/hook lifecycle — native Mojo tests run with `mojo test`.
#
# These tests are a direct port of test/scopes.test.ts, exercising
# the same scenarios without the WASM/JS round-trip.
#
# Run with:
#   mojo test -I src test-mojo/test_scopes.mojo

from testing import assert_equal, assert_true, assert_false

from signals import Runtime, create_runtime, destroy_runtime
from scope import HOOK_SIGNAL


# ── Helpers ──────────────────────────────────────────────────────────────────


fn _make_runtime() -> UnsafePointer[Runtime]:
    return create_runtime()


fn _teardown(rt: UnsafePointer[Runtime]):
    destroy_runtime(rt)


# ── Scope lifecycle ──────────────────────────────────────────────────────────


fn test_scope_create_and_destroy() raises:
    var rt = _make_runtime()

    assert_equal(rt[].scope_count(), 0, "new runtime has 0 scopes")

    var s0 = rt[].create_scope(UInt32(0), -1)
    assert_equal(rt[].scope_count(), 1, "1 scope after create")
    assert_true(rt[].scope_contains(s0), "scope exists")

    rt[].destroy_scope(s0)
    assert_equal(rt[].scope_count(), 0, "0 scopes after destroy")
    assert_false(rt[].scope_contains(s0), "scope no longer exists")

    _teardown(rt)


fn test_scope_sequential_ids() raises:
    var rt = _make_runtime()

    var s0 = rt[].create_scope(UInt32(0), -1)
    var s1 = rt[].create_scope(UInt32(0), -1)
    var s2 = rt[].create_scope(UInt32(0), -1)

    assert_equal(Int(s0), 0, "first scope gets ID 0")
    assert_equal(Int(s1), 1, "second scope gets ID 1")
    assert_equal(Int(s2), 2, "third scope gets ID 2")
    assert_equal(rt[].scope_count(), 3, "3 scopes created")

    _teardown(rt)


fn test_scope_slot_reuse_after_destroy() raises:
    var rt = _make_runtime()

    var s0 = rt[].create_scope(UInt32(0), -1)
    var _s1 = rt[].create_scope(UInt32(0), -1)
    rt[].destroy_scope(s0)

    var s2 = rt[].create_scope(UInt32(0), -1)
    assert_equal(Int(s2), Int(s0), "new scope reuses destroyed slot")
    assert_equal(rt[].scope_count(), 2, "2 scopes after reuse")

    _teardown(rt)


fn test_scope_double_destroy_is_noop() raises:
    var rt = _make_runtime()

    var s0 = rt[].create_scope(UInt32(0), -1)
    rt[].destroy_scope(s0)
    rt[].destroy_scope(s0)  # should not crash
    assert_equal(rt[].scope_count(), 0, "still 0 scopes after double destroy")

    _teardown(rt)


# ── Height and parent tracking ───────────────────────────────────────────────


fn test_scope_height_and_parent_tracking() raises:
    var rt = _make_runtime()

    var root = rt[].create_scope(UInt32(0), -1)
    assert_equal(Int(rt[].scopes.height(root)), 0, "root height is 0")
    assert_equal(rt[].scopes.parent_id(root), -1, "root has no parent (-1)")

    var child = rt[].create_scope(UInt32(1), Int(root))
    assert_equal(Int(rt[].scopes.height(child)), 1, "child height is 1")
    assert_equal(
        rt[].scopes.parent_id(child), Int(root), "child parent is root"
    )

    var grandchild = rt[].create_scope(UInt32(2), Int(child))
    assert_equal(
        Int(rt[].scopes.height(grandchild)), 2, "grandchild height is 2"
    )
    assert_equal(
        rt[].scopes.parent_id(grandchild),
        Int(child),
        "grandchild parent is child",
    )

    _teardown(rt)


fn test_scope_create_child_auto_computes_height() raises:
    var rt = _make_runtime()

    var root = rt[].create_scope(UInt32(0), -1)
    var child = rt[].create_child_scope(root)
    var grandchild = rt[].create_child_scope(child)

    assert_equal(
        Int(rt[].scopes.height(child)), 1, "child height auto-computed to 1"
    )
    assert_equal(
        rt[].scopes.parent_id(child), Int(root), "child parent is root"
    )
    assert_equal(
        Int(rt[].scopes.height(grandchild)),
        2,
        "grandchild height auto-computed to 2",
    )
    assert_equal(
        rt[].scopes.parent_id(grandchild),
        Int(child),
        "grandchild parent is child",
    )

    _teardown(rt)


# ── Dirty flag ───────────────────────────────────────────────────────────────


fn test_scope_dirty_flag() raises:
    var rt = _make_runtime()

    var s = rt[].create_scope(UInt32(0), -1)
    assert_false(rt[].scopes.is_dirty(s), "not dirty initially")

    rt[].scopes.set_dirty(s, True)
    assert_true(rt[].scopes.is_dirty(s), "dirty after set_dirty(True)")

    rt[].scopes.set_dirty(s, False)
    assert_false(rt[].scopes.is_dirty(s), "clean after set_dirty(False)")

    _teardown(rt)


# ── Render count ─────────────────────────────────────────────────────────────


fn test_scope_render_count() raises:
    var rt = _make_runtime()

    var s = rt[].create_scope(UInt32(0), -1)
    assert_equal(
        Int(rt[].scopes.render_count(s)), 0, "render_count starts at 0"
    )

    var prev = rt[].begin_scope_render(s)
    assert_equal(
        Int(rt[].scopes.render_count(s)),
        1,
        "render_count is 1 after first begin_render",
    )
    rt[].end_scope_render(prev)

    var prev2 = rt[].begin_scope_render(s)
    assert_equal(
        Int(rt[].scopes.render_count(s)),
        2,
        "render_count is 2 after second begin_render",
    )
    rt[].end_scope_render(prev2)

    _teardown(rt)


# ── Begin render clears dirty ────────────────────────────────────────────────


fn test_scope_begin_render_clears_dirty() raises:
    var rt = _make_runtime()

    var s = rt[].create_scope(UInt32(0), -1)
    rt[].scopes.set_dirty(s, True)
    assert_true(rt[].scopes.is_dirty(s), "dirty before render")

    var prev = rt[].begin_scope_render(s)
    assert_false(rt[].scopes.is_dirty(s), "clean after begin_render")
    rt[].end_scope_render(prev)

    _teardown(rt)


# ── Begin/end render manages current scope ───────────────────────────────────


fn test_scope_begin_end_render_manages_current() raises:
    var rt = _make_runtime()

    assert_false(rt[].has_scope(), "no scope initially")
    assert_equal(rt[].current_scope, -1, "current scope is -1 initially")

    var s = rt[].create_scope(UInt32(0), -1)
    var prev = rt[].begin_scope_render(s)
    assert_equal(prev, -1, "previous scope is -1 (was no scope)")
    assert_true(rt[].has_scope(), "scope active during render")
    assert_equal(
        Int(rt[].get_scope()), Int(s), "current scope is the rendering scope"
    )

    rt[].end_scope_render(prev)
    assert_false(rt[].has_scope(), "no scope after end_render")
    assert_equal(rt[].current_scope, -1, "current scope is -1 after end_render")

    _teardown(rt)


# ── Begin render sets reactive context ───────────────────────────────────────


fn test_scope_begin_render_sets_reactive_context() raises:
    var rt = _make_runtime()

    var s = rt[].create_scope(UInt32(0), -1)
    assert_false(rt[].has_context(), "no context initially")

    var prev = rt[].begin_scope_render(s)
    assert_true(rt[].has_context(), "context active during render")

    rt[].end_scope_render(prev)
    assert_false(rt[].has_context(), "context cleared after end_render")

    _teardown(rt)


# ── Nested scope rendering ───────────────────────────────────────────────────


fn test_scope_nested_rendering() raises:
    var rt = _make_runtime()

    var root = rt[].create_scope(UInt32(0), -1)
    var child = rt[].create_child_scope(root)

    # Begin rendering root
    var prev1 = rt[].begin_scope_render(root)
    assert_equal(Int(rt[].get_scope()), Int(root), "current scope is root")

    # Nest: begin rendering child
    var prev2 = rt[].begin_scope_render(child)
    assert_equal(prev2, Int(root), "previous scope was root")
    assert_equal(Int(rt[].get_scope()), Int(child), "current scope is child")

    # End child rendering
    rt[].end_scope_render(prev2)
    assert_equal(
        Int(rt[].get_scope()), Int(root), "current scope restored to root"
    )

    # End root rendering
    rt[].end_scope_render(prev1)
    assert_equal(rt[].current_scope, -1, "current scope cleared")

    _teardown(rt)


# ── is_first_render ──────────────────────────────────────────────────────────


fn test_scope_is_first_render() raises:
    var rt = _make_runtime()

    var s = rt[].create_scope(UInt32(0), -1)
    assert_true(
        rt[].scopes.is_first_render(s), "first render before any rendering"
    )

    var prev = rt[].begin_scope_render(s)
    assert_true(
        rt[].scopes.is_first_render(s), "first render during first render pass"
    )
    rt[].end_scope_render(prev)

    var prev2 = rt[].begin_scope_render(s)
    assert_false(
        rt[].scopes.is_first_render(s), "not first render on second pass"
    )
    rt[].end_scope_render(prev2)

    _teardown(rt)


# ── Hooks start empty ────────────────────────────────────────────────────────


fn test_scope_hooks_start_empty() raises:
    var rt = _make_runtime()

    var s = rt[].create_scope(UInt32(0), -1)
    assert_equal(rt[].scopes.hook_count(s), 0, "no hooks initially")

    _teardown(rt)


# ── Hook: use_signal creates signal on first render ──────────────────────────


fn test_hook_use_signal_creates_on_first_render() raises:
    var rt = _make_runtime()

    var s = rt[].create_scope(UInt32(0), -1)
    var prev = rt[].begin_scope_render(s)

    var key = rt[].use_signal_i32(Int32(42))
    assert_equal(
        Int(rt[].read_signal[Int32](key)),
        42,
        "signal created with initial value 42",
    )
    assert_equal(rt[].scopes.hook_count(s), 1, "1 hook after use_signal")
    assert_equal(
        Int(rt[].scopes.hook_value_at(s, 0)),
        Int(key),
        "hook[0] stores the signal key",
    )
    assert_equal(
        Int(rt[].scopes.hook_tag_at(s, 0)),
        Int(HOOK_SIGNAL),
        "hook[0] tag is HOOK_SIGNAL (0)",
    )

    rt[].end_scope_render(prev)
    _teardown(rt)


# ── Hook: use_signal returns same signal on re-render ────────────────────────


fn test_hook_use_signal_same_on_rerender() raises:
    var rt = _make_runtime()

    var s = rt[].create_scope(UInt32(0), -1)

    # First render: create signal
    var prev1 = rt[].begin_scope_render(s)
    var key1 = rt[].use_signal_i32(Int32(100))
    assert_equal(
        Int(rt[].read_signal[Int32](key1)),
        100,
        "first render: signal value is 100",
    )
    rt[].end_scope_render(prev1)

    # Modify signal between renders
    rt[].write_signal[Int32](key1, Int32(200))

    # Second render: retrieve same signal (initial value ignored)
    var prev2 = rt[].begin_scope_render(s)
    var key2 = rt[].use_signal_i32(Int32(999))
    assert_equal(Int(key2), Int(key1), "re-render returns same signal key")
    assert_equal(
        Int(rt[].read_signal[Int32](key2)),
        200,
        "signal retains modified value, not initial",
    )
    assert_equal(
        rt[].scopes.hook_count(s), 1, "still 1 hook (no new hook created)"
    )
    rt[].end_scope_render(prev2)

    _teardown(rt)


# ── Hook: multiple signals in same scope ─────────────────────────────────────


fn test_hook_multiple_signals_same_scope() raises:
    var rt = _make_runtime()

    var s = rt[].create_scope(UInt32(0), -1)

    # First render: create 3 signals
    var prev1 = rt[].begin_scope_render(s)
    var k1 = rt[].use_signal_i32(Int32(10))
    var k2 = rt[].use_signal_i32(Int32(20))
    var k3 = rt[].use_signal_i32(Int32(30))
    assert_equal(rt[].scopes.hook_count(s), 3, "3 hooks after first render")
    assert_true(
        Int(k1) != Int(k2) and Int(k2) != Int(k3), "all signal keys distinct"
    )
    rt[].end_scope_render(prev1)

    # Second render: same order returns same keys
    var prev2 = rt[].begin_scope_render(s)
    var k1b = rt[].use_signal_i32(Int32(0))
    var k2b = rt[].use_signal_i32(Int32(0))
    var k3b = rt[].use_signal_i32(Int32(0))
    assert_equal(Int(k1b), Int(k1), "re-render hook 0 returns same key")
    assert_equal(Int(k2b), Int(k2), "re-render hook 1 returns same key")
    assert_equal(Int(k3b), Int(k3), "re-render hook 2 returns same key")
    assert_equal(rt[].scopes.hook_count(s), 3, "still 3 hooks")
    rt[].end_scope_render(prev2)

    # Values are independent
    assert_equal(Int(rt[].peek_signal[Int32](k1)), 10, "signal 1 has value 10")
    assert_equal(Int(rt[].peek_signal[Int32](k2)), 20, "signal 2 has value 20")
    assert_equal(Int(rt[].peek_signal[Int32](k3)), 30, "signal 3 has value 30")

    _teardown(rt)


# ── Hook: signals in different scopes are independent ────────────────────────


fn test_hook_signals_in_different_scopes_independent() raises:
    var rt = _make_runtime()

    var s1 = rt[].create_scope(UInt32(0), -1)
    var s2 = rt[].create_scope(UInt32(0), -1)

    # Render scope 1
    var prev1 = rt[].begin_scope_render(s1)
    var k1 = rt[].use_signal_i32(Int32(100))
    rt[].end_scope_render(prev1)

    # Render scope 2
    var prev2 = rt[].begin_scope_render(s2)
    var k2 = rt[].use_signal_i32(Int32(200))
    rt[].end_scope_render(prev2)

    assert_true(
        Int(k1) != Int(k2), "different scopes get different signal keys"
    )
    assert_equal(Int(rt[].peek_signal[Int32](k1)), 100, "scope 1 signal is 100")
    assert_equal(Int(rt[].peek_signal[Int32](k2)), 200, "scope 2 signal is 200")

    # Modify one, other unchanged
    rt[].write_signal[Int32](k1, Int32(999))
    assert_equal(
        Int(rt[].peek_signal[Int32](k1)), 999, "scope 1 signal updated"
    )
    assert_equal(
        Int(rt[].peek_signal[Int32](k2)), 200, "scope 2 signal unchanged"
    )

    _teardown(rt)


# ── Hook: signal read during render subscribes scope ─────────────────────────


fn test_hook_signal_read_subscribes_scope() raises:
    var rt = _make_runtime()

    var s = rt[].create_scope(UInt32(0), -1)

    # First render
    var prev = rt[].begin_scope_render(s)
    var key = rt[].use_signal_i32(Int32(0))

    # Read the signal during render — should subscribe this scope
    _ = rt[].read_signal[Int32](key)
    assert_equal(
        rt[].signals.subscriber_count(key),
        1,
        "scope subscribed after read during render",
    )

    rt[].end_scope_render(prev)

    # Write should mark scope dirty
    rt[].write_signal[Int32](key, Int32(42))
    assert_true(rt[].has_dirty(), "dirty after signal write")
    assert_equal(rt[].dirty_count(), 1, "1 dirty scope")

    _teardown(rt)


# ── Hook: peek during render does NOT subscribe ──────────────────────────────


fn test_hook_peek_does_not_subscribe() raises:
    var rt = _make_runtime()

    var s = rt[].create_scope(UInt32(0), -1)

    var prev = rt[].begin_scope_render(s)
    var key = rt[].use_signal_i32(Int32(0))

    # Peek should NOT subscribe
    _ = rt[].peek_signal[Int32](key)
    assert_equal(
        rt[].signals.subscriber_count(key), 0, "peek does not subscribe"
    )

    rt[].end_scope_render(prev)

    _teardown(rt)


# ── Nested rendering: child signals subscribe child scope ────────────────────


fn test_hook_nested_rendering_subscribes_correct_scope() raises:
    var rt = _make_runtime()

    var root = rt[].create_scope(UInt32(0), -1)
    var child = rt[].create_child_scope(root)

    # Begin root render
    var prev_root = rt[].begin_scope_render(root)
    var root_signal = rt[].use_signal_i32(Int32(10))
    _ = rt[].read_signal[Int32](root_signal)

    # Begin child render (nested)
    var prev_child = rt[].begin_scope_render(child)
    var child_signal = rt[].use_signal_i32(Int32(20))
    _ = rt[].read_signal[Int32](child_signal)

    # Child signal should have child as subscriber, not root
    assert_equal(
        rt[].signals.subscriber_count(child_signal),
        1,
        "child signal has 1 subscriber",
    )

    # End child render
    rt[].end_scope_render(prev_child)

    # Root signal should still have root subscribed
    assert_equal(
        rt[].signals.subscriber_count(root_signal),
        1,
        "root signal has 1 subscriber",
    )

    # End root render
    rt[].end_scope_render(prev_root)

    # Write to child signal should only mark child dirty
    rt[].write_signal[Int32](child_signal, Int32(99))
    assert_equal(
        rt[].dirty_count(), 1, "only 1 dirty scope from child signal write"
    )

    _teardown(rt)


# ── Stress: 100 scopes ──────────────────────────────────────────────────────


fn test_scope_stress_100_scopes() raises:
    var rt = _make_runtime()

    var ids = List[UInt32]()
    for i in range(100):
        ids.append(rt[].create_scope(UInt32(0), -1))
    assert_equal(rt[].scope_count(), 100, "100 scopes created")

    # Destroy half (even indices)
    for i in range(0, 100, 2):
        rt[].destroy_scope(ids[i])
    assert_equal(rt[].scope_count(), 50, "50 scopes after destroying half")

    # Create 50 more (reuse freed slots)
    var new_ids = List[UInt32]()
    for i in range(50):
        new_ids.append(rt[].create_scope(UInt32(0), -1))
    assert_equal(rt[].scope_count(), 100, "100 scopes after refill")

    # Verify all odd-indexed original scopes still exist
    var all_exist = True
    for i in range(1, 100, 2):
        if not rt[].scope_contains(ids[i]):
            all_exist = False
            break
    assert_true(all_exist, "all odd-indexed original scopes still exist")

    _teardown(rt)


# ── Hook: signal stable across many re-renders ──────────────────────────────


fn test_hook_signal_stable_across_many_rerenders() raises:
    var rt = _make_runtime()

    var s = rt[].create_scope(UInt32(0), -1)

    # First render
    var prev = rt[].begin_scope_render(s)
    var key = rt[].use_signal_i32(Int32(0))
    rt[].end_scope_render(prev)

    # Increment signal and re-render 50 times
    for i in range(1, 51):
        rt[].write_signal[Int32](key, Int32(i))

        prev = rt[].begin_scope_render(s)
        var k = rt[].use_signal_i32(Int32(999))
        assert_equal(Int(k), Int(key), "re-render: same key")
        rt[].end_scope_render(prev)

    assert_equal(
        Int(rt[].peek_signal[Int32](key)),
        50,
        "signal holds value 50 after 50 writes",
    )
    assert_equal(
        Int(rt[].scopes.render_count(s)),
        51,
        "render_count is 51 after 1 + 50 re-renders",
    )
    assert_equal(rt[].scopes.hook_count(s), 1, "still just 1 hook")

    _teardown(rt)


# ── Simulated counter component ──────────────────────────────────────────────


fn test_hook_simulated_counter_component() raises:
    var rt = _make_runtime()
    var s = rt[].create_scope(UInt32(0), -1)

    # First render
    var prev = rt[].begin_scope_render(s)
    var count_key = rt[].use_signal_i32(Int32(0))
    var count_val = rt[].read_signal[Int32](count_key)
    assert_equal(Int(count_val), 0, "initial count is 0")
    rt[].end_scope_render(prev)

    # Simulate click: count += 1
    var current = rt[].peek_signal[Int32](count_key)
    rt[].write_signal[Int32](count_key, current + Int32(1))
    assert_equal(
        Int(rt[].peek_signal[Int32](count_key)), 1, "count is 1 after increment"
    )
    assert_true(rt[].has_dirty(), "scope marked dirty after signal write")

    # Re-render (triggered by dirty)
    prev = rt[].begin_scope_render(s)
    var count_key2 = rt[].use_signal_i32(Int32(0))
    assert_equal(
        Int(count_key2), Int(count_key), "same signal key on re-render"
    )
    var count_val2 = rt[].read_signal[Int32](count_key2)
    assert_equal(Int(count_val2), 1, "count reads 1 on re-render")
    rt[].end_scope_render(prev)

    # Another click
    current = rt[].peek_signal[Int32](count_key)
    rt[].write_signal[Int32](count_key, current + Int32(1))
    assert_equal(
        Int(rt[].peek_signal[Int32](count_key)),
        2,
        "count is 2 after second increment",
    )

    _teardown(rt)


# ── Simulated multi-state component ──────────────────────────────────────────


fn test_hook_simulated_multi_state_component() raises:
    var rt = _make_runtime()
    var s = rt[].create_scope(UInt32(0), -1)

    # First render: 3 signals (name as i32=0, age=0, submitted=0)
    var prev = rt[].begin_scope_render(s)
    var name_key = rt[].use_signal_i32(Int32(0))
    var age_key = rt[].use_signal_i32(Int32(0))
    var submitted_key = rt[].use_signal_i32(Int32(0))
    assert_equal(rt[].scopes.hook_count(s), 3, "3 hooks for 3 signals")
    rt[].end_scope_render(prev)

    # Simulate user interaction
    rt[].write_signal[Int32](name_key, Int32(42))
    rt[].write_signal[Int32](age_key, Int32(25))

    # Re-render
    prev = rt[].begin_scope_render(s)
    var name_key2 = rt[].use_signal_i32(Int32(0))
    var age_key2 = rt[].use_signal_i32(Int32(0))
    var submitted_key2 = rt[].use_signal_i32(Int32(0))
    assert_equal(Int(name_key2), Int(name_key), "name signal stable")
    assert_equal(Int(age_key2), Int(age_key), "age signal stable")
    assert_equal(
        Int(submitted_key2), Int(submitted_key), "submitted signal stable"
    )
    assert_equal(
        Int(rt[].peek_signal[Int32](name_key2)), 42, "name retains value"
    )
    assert_equal(
        Int(rt[].peek_signal[Int32](age_key2)), 25, "age retains value"
    )
    assert_equal(
        Int(rt[].peek_signal[Int32](submitted_key2)),
        0,
        "submitted still false",
    )
    rt[].end_scope_render(prev)

    _teardown(rt)


# ── Simulated parent-child component tree ────────────────────────────────────


fn test_hook_simulated_parent_child_tree() raises:
    var rt = _make_runtime()

    var parent = rt[].create_scope(UInt32(0), -1)
    var child1 = rt[].create_child_scope(parent)
    var child2 = rt[].create_child_scope(parent)

    # Render parent
    var prev_p = rt[].begin_scope_render(parent)
    var parent_count = rt[].use_signal_i32(Int32(0))
    _ = rt[].read_signal[Int32](parent_count)  # subscribe parent

    # Render child1 (nested)
    var prev_c1 = rt[].begin_scope_render(child1)
    var child1_local = rt[].use_signal_i32(Int32(10))
    _ = rt[].read_signal[Int32](child1_local)  # subscribe child1
    # Also read parent's signal from child1
    _ = rt[].read_signal[Int32](
        parent_count
    )  # child1 subscribes to parent signal
    rt[].end_scope_render(prev_c1)

    # Render child2 (nested)
    var prev_c2 = rt[].begin_scope_render(child2)
    var child2_local = rt[].use_signal_i32(Int32(20))
    _ = rt[].read_signal[Int32](child2_local)  # subscribe child2
    rt[].end_scope_render(prev_c2)

    rt[].end_scope_render(prev_p)

    # parentCount has 2 subscribers: parent + child1
    assert_equal(
        rt[].signals.subscriber_count(parent_count),
        2,
        "parent signal has 2 subscribers (parent + child1)",
    )
    assert_equal(
        rt[].signals.subscriber_count(child1_local),
        1,
        "child1 signal has 1 subscriber",
    )
    assert_equal(
        rt[].signals.subscriber_count(child2_local),
        1,
        "child2 signal has 1 subscriber",
    )

    # Write to parent signal → parent and child1 dirty
    rt[].write_signal[Int32](parent_count, Int32(5))
    assert_equal(
        rt[].dirty_count(), 2, "2 dirty scopes from parent signal write"
    )

    _teardown(rt)


# ── Separate runtimes are isolated ───────────────────────────────────────────


fn test_scope_separate_runtimes_isolated() raises:
    var rt1 = _make_runtime()
    var rt2 = _make_runtime()

    var s1 = rt1[].create_scope(UInt32(0), -1)
    var s2 = rt2[].create_scope(UInt32(0), -1)

    var prev1 = rt1[].begin_scope_render(s1)
    var k1 = rt1[].use_signal_i32(Int32(111))
    rt1[].end_scope_render(prev1)

    var prev2 = rt2[].begin_scope_render(s2)
    var k2 = rt2[].use_signal_i32(Int32(222))
    rt2[].end_scope_render(prev2)

    assert_equal(
        Int(rt1[].peek_signal[Int32](k1)), 111, "runtime 1 signal is 111"
    )
    assert_equal(
        Int(rt2[].peek_signal[Int32](k2)), 222, "runtime 2 signal is 222"
    )

    _teardown(rt1)
    _teardown(rt2)


# ── Render scope with no hooks ───────────────────────────────────────────────


fn test_scope_render_with_no_hooks() raises:
    var rt = _make_runtime()

    var s = rt[].create_scope(UInt32(0), -1)

    # Render with no hook calls (static component)
    var prev = rt[].begin_scope_render(s)
    # No hook calls — just render static content
    rt[].end_scope_render(prev)

    assert_equal(Int(rt[].scopes.render_count(s)), 1, "render_count is 1")
    assert_equal(rt[].scopes.hook_count(s), 0, "0 hooks for static component")

    # Re-render
    var prev2 = rt[].begin_scope_render(s)
    rt[].end_scope_render(prev2)

    assert_equal(Int(rt[].scopes.render_count(s)), 2, "render_count is 2")

    _teardown(rt)

# Tests for Runtime / Signals — native Mojo tests run with `mojo test`.
#
# These tests are a direct port of test/signals.test.ts, exercising
# the same scenarios without the WASM/JS round-trip.
#
# Run with:
#   mojo test -I src test-mojo/test_signals.mojo

from testing import assert_equal, assert_true, assert_false

from signals import Runtime, SignalStore, create_runtime, destroy_runtime


# ── Helpers ──────────────────────────────────────────────────────────────────


fn _make_runtime() -> UnsafePointer[Runtime]:
    """Create a heap-allocated Runtime for testing."""
    return create_runtime()


fn _teardown(rt: UnsafePointer[Runtime]):
    """Destroy a heap-allocated Runtime."""
    destroy_runtime(rt)


# ── Runtime lifecycle ────────────────────────────────────────────────────────


fn test_runtime_create_returns_non_null() raises:
    var rt = _make_runtime()
    assert_true(Int(rt) != 0, "runtime_create returns non-null pointer")
    assert_equal(rt[].signals.signal_count(), 0, "new runtime has 0 signals")
    _teardown(rt)


# ── Signal create and read ───────────────────────────────────────────────────


fn test_signal_create_and_read() raises:
    var rt = _make_runtime()

    var key = rt[].create_signal[Int32](Int32(42))
    assert_equal(Int(key), 0, "first signal gets key 0")
    assert_equal(rt[].signals.signal_count(), 1, "signal_count is 1")
    assert_true(rt[].signals.contains(key), "signal exists")

    var val = rt[].read_signal[Int32](key)
    assert_equal(Int(val), 42, "read returns initial value 42")

    _teardown(rt)


# ── Signal write and read back ───────────────────────────────────────────────


fn test_signal_write_and_read_back() raises:
    var rt = _make_runtime()

    var key = rt[].create_signal[Int32](Int32(0))
    assert_equal(Int(rt[].read_signal[Int32](key)), 0, "initial value is 0")

    rt[].write_signal[Int32](key, Int32(99))
    assert_equal(
        Int(rt[].read_signal[Int32](key)), 99, "read after write returns 99"
    )

    rt[].write_signal[Int32](key, Int32(-42))
    assert_equal(
        Int(rt[].read_signal[Int32](key)), -42, "read after write returns -42"
    )

    _teardown(rt)


# ── Signal peek (no subscription) ────────────────────────────────────────────


fn test_signal_peek() raises:
    var rt = _make_runtime()

    var key = rt[].create_signal[Int32](Int32(77))
    assert_equal(Int(rt[].peek_signal[Int32](key)), 77, "peek returns 77")

    rt[].write_signal[Int32](key, Int32(88))
    assert_equal(
        Int(rt[].peek_signal[Int32](key)), 88, "peek after write returns 88"
    )

    _teardown(rt)


# ── Signal version tracking ──────────────────────────────────────────────────


fn test_signal_version_tracking() raises:
    var rt = _make_runtime()

    var key = rt[].create_signal[Int32](Int32(0))
    assert_equal(Int(rt[].signals.version(key)), 0, "initial version is 0")

    rt[].write_signal[Int32](key, Int32(1))
    assert_equal(
        Int(rt[].signals.version(key)), 1, "version after 1 write is 1"
    )

    rt[].write_signal[Int32](key, Int32(2))
    assert_equal(
        Int(rt[].signals.version(key)), 2, "version after 2 writes is 2"
    )

    # Peek and read don't change version
    _ = rt[].peek_signal[Int32](key)
    _ = rt[].read_signal[Int32](key)
    assert_equal(
        Int(rt[].signals.version(key)), 2, "read/peek don't bump version"
    )

    _teardown(rt)


# ── Multiple independent signals ─────────────────────────────────────────────


fn test_multiple_independent_signals() raises:
    var rt = _make_runtime()

    var k1 = rt[].create_signal[Int32](Int32(10))
    var k2 = rt[].create_signal[Int32](Int32(20))
    var k3 = rt[].create_signal[Int32](Int32(30))

    assert_equal(rt[].signals.signal_count(), 3, "3 signals created")
    assert_true(k1 != k2 and k2 != k3 and k1 != k3, "all keys distinct")

    assert_equal(Int(rt[].read_signal[Int32](k1)), 10, "signal 1 reads 10")
    assert_equal(Int(rt[].read_signal[Int32](k2)), 20, "signal 2 reads 20")
    assert_equal(Int(rt[].read_signal[Int32](k3)), 30, "signal 3 reads 30")

    # Write to one doesn't affect others
    rt[].write_signal[Int32](k2, Int32(200))
    assert_equal(Int(rt[].read_signal[Int32](k1)), 10, "signal 1 unchanged")
    assert_equal(
        Int(rt[].read_signal[Int32](k2)), 200, "signal 2 updated to 200"
    )
    assert_equal(Int(rt[].read_signal[Int32](k3)), 30, "signal 3 unchanged")

    _teardown(rt)


# ── Signal destroy ───────────────────────────────────────────────────────────


fn test_signal_destroy() raises:
    var rt = _make_runtime()

    var k1 = rt[].create_signal[Int32](Int32(10))
    var k2 = rt[].create_signal[Int32](Int32(20))
    assert_equal(rt[].signals.signal_count(), 2, "2 signals before destroy")

    rt[].destroy_signal(k1)
    assert_equal(rt[].signals.signal_count(), 1, "1 signal after destroy")
    assert_false(rt[].signals.contains(k1), "destroyed signal not found")
    assert_true(rt[].signals.contains(k2), "other signal still exists")

    _teardown(rt)


# ── Signal slot reuse after destroy ──────────────────────────────────────────


fn test_signal_slot_reuse_after_destroy() raises:
    var rt = _make_runtime()

    var k1 = rt[].create_signal[Int32](Int32(10))
    rt[].destroy_signal(k1)

    var k2 = rt[].create_signal[Int32](Int32(99))
    assert_equal(Int(k2), Int(k1), "new signal reuses destroyed slot")
    assert_equal(
        Int(rt[].read_signal[Int32](k2)), 99, "reused slot has new value"
    )

    _teardown(rt)


# ── Signal iadd (+=) via read+write ──────────────────────────────────────────


fn _iadd(mut rt: Runtime, key: UInt32, delta: Int32):
    """Increment a signal by delta (read, add, write)."""
    var val = rt.read_signal[Int32](key)
    rt.write_signal[Int32](key, val + delta)


fn _isub(mut rt: Runtime, key: UInt32, delta: Int32):
    """Decrement a signal by delta (read, sub, write)."""
    var val = rt.read_signal[Int32](key)
    rt.write_signal[Int32](key, val - delta)


fn test_signal_iadd() raises:
    var rt = _make_runtime()

    var key = rt[].create_signal[Int32](Int32(10))
    _iadd(rt[], key, Int32(5))
    assert_equal(Int(rt[].read_signal[Int32](key)), 15, "10 += 5 => 15")

    _iadd(rt[], key, Int32(-3))
    assert_equal(Int(rt[].read_signal[Int32](key)), 12, "15 += (-3) => 12")

    _iadd(rt[], key, Int32(0))
    assert_equal(Int(rt[].read_signal[Int32](key)), 12, "12 += 0 => 12")

    _teardown(rt)


# ── Signal isub (-=) via read+write ──────────────────────────────────────────


fn test_signal_isub() raises:
    var rt = _make_runtime()

    var key = rt[].create_signal[Int32](Int32(100))
    _isub(rt[], key, Int32(30))
    assert_equal(Int(rt[].read_signal[Int32](key)), 70, "100 -= 30 => 70")

    _isub(rt[], key, Int32(-10))
    assert_equal(Int(rt[].read_signal[Int32](key)), 80, "70 -= (-10) => 80")

    _teardown(rt)


# ── Context: no context by default ───────────────────────────────────────────


fn test_no_context_by_default() raises:
    var rt = _make_runtime()
    assert_false(rt[].has_context(), "no context initially")
    _teardown(rt)


# ── Context: set and clear ───────────────────────────────────────────────────


fn test_context_set_and_clear() raises:
    var rt = _make_runtime()

    rt[].set_context(UInt32(42))
    assert_true(rt[].has_context(), "context active after set")

    rt[].clear_context()
    assert_false(rt[].has_context(), "no context after clear")

    _teardown(rt)


# ── Subscription: read with context subscribes ───────────────────────────────


fn test_read_with_context_subscribes() raises:
    var rt = _make_runtime()

    var key = rt[].create_signal[Int32](Int32(0))
    assert_equal(
        rt[].signals.subscriber_count(key), 0, "0 subscribers initially"
    )

    # Read without context — no subscription
    _ = rt[].read_signal[Int32](key)
    assert_equal(
        rt[].signals.subscriber_count(key),
        0,
        "still 0 subscribers after read without context",
    )

    # Read with context — subscribes
    rt[].set_context(UInt32(100))
    _ = rt[].read_signal[Int32](key)
    assert_equal(
        rt[].signals.subscriber_count(key),
        1,
        "1 subscriber after read with context",
    )

    # Reading again with same context is idempotent
    _ = rt[].read_signal[Int32](key)
    assert_equal(
        rt[].signals.subscriber_count(key),
        1,
        "still 1 subscriber (idempotent)",
    )

    rt[].clear_context()
    _teardown(rt)


# ── Subscription: peek does NOT subscribe ────────────────────────────────────


fn test_peek_does_not_subscribe() raises:
    var rt = _make_runtime()

    var key = rt[].create_signal[Int32](Int32(0))
    rt[].set_context(UInt32(200))

    _ = rt[].peek_signal[Int32](key)
    assert_equal(
        rt[].signals.subscriber_count(key), 0, "peek does not subscribe"
    )

    rt[].clear_context()
    _teardown(rt)


# ── Subscription: multiple contexts subscribe ────────────────────────────────


fn test_multiple_contexts_subscribe() raises:
    var rt = _make_runtime()

    var key = rt[].create_signal[Int32](Int32(0))

    rt[].set_context(UInt32(10))
    _ = rt[].read_signal[Int32](key)

    rt[].set_context(UInt32(20))
    _ = rt[].read_signal[Int32](key)

    rt[].set_context(UInt32(30))
    _ = rt[].read_signal[Int32](key)

    assert_equal(
        rt[].signals.subscriber_count(key),
        3,
        "3 different contexts subscribed",
    )

    rt[].clear_context()
    _teardown(rt)


# ── Dirty scopes: write with subscribers produces dirty ──────────────────────


fn test_write_marks_subscribers_dirty() raises:
    var rt = _make_runtime()

    var key = rt[].create_signal[Int32](Int32(0))
    assert_false(rt[].has_dirty(), "no dirty scopes initially")

    # Subscribe context 1
    rt[].set_context(UInt32(1))
    _ = rt[].read_signal[Int32](key)

    # Subscribe context 2
    rt[].set_context(UInt32(2))
    _ = rt[].read_signal[Int32](key)

    rt[].clear_context()

    # Write — should mark both contexts dirty
    rt[].write_signal[Int32](key, Int32(42))
    assert_true(rt[].has_dirty(), "has dirty after write")
    assert_equal(rt[].dirty_count(), 2, "2 dirty scopes")

    _teardown(rt)


# ── Dirty scopes: write without subscribers ──────────────────────────────────


fn test_write_without_subscribers_is_clean() raises:
    var rt = _make_runtime()

    var key = rt[].create_signal[Int32](Int32(0))
    rt[].write_signal[Int32](key, Int32(99))

    assert_false(rt[].has_dirty(), "no dirty without subscribers")
    assert_equal(rt[].dirty_count(), 0, "dirty count is 0")

    _teardown(rt)


# ── Dirty scopes: iadd marks dirty ───────────────────────────────────────────


fn test_iadd_marks_subscribers_dirty() raises:
    var rt = _make_runtime()

    var key = rt[].create_signal[Int32](Int32(0))

    rt[].set_context(UInt32(50))
    _ = rt[].read_signal[Int32](key)
    rt[].clear_context()

    # iadd via read + write
    var val = rt[].read_signal[Int32](key)
    rt[].write_signal[Int32](key, val + Int32(1))
    assert_true(rt[].has_dirty(), "iadd marks subscriber dirty")
    assert_equal(rt[].dirty_count(), 1, "1 dirty scope from iadd")

    _teardown(rt)


# ── Multiple writes deduplicate dirty scopes ─────────────────────────────────


fn test_multiple_writes_deduplicate_dirty_scopes() raises:
    var rt = _make_runtime()

    var key = rt[].create_signal[Int32](Int32(0))

    rt[].set_context(UInt32(1))
    _ = rt[].read_signal[Int32](key)
    rt[].clear_context()

    # Write twice — same subscriber should not be double-queued
    rt[].write_signal[Int32](key, Int32(10))
    rt[].write_signal[Int32](key, Int32(20))

    # The dirty count should still be 1 since it's the same context
    assert_equal(rt[].dirty_count(), 1, "same subscriber not double-queued")

    _teardown(rt)


# ── Read after write in same turn returns new value ──────────────────────────


fn test_read_after_write_returns_new_value() raises:
    var rt = _make_runtime()

    var key = rt[].create_signal[Int32](Int32(0))
    rt[].write_signal[Int32](key, Int32(123))
    assert_equal(
        Int(rt[].read_signal[Int32](key)), 123, "immediate read gets 123"
    )

    # iadd
    var val = rt[].read_signal[Int32](key)
    rt[].write_signal[Int32](key, val + Int32(1))
    assert_equal(
        Int(rt[].read_signal[Int32](key)), 124, "iadd then read gets 124"
    )

    _teardown(rt)


# ── Stress: create 100 signals, verify independence ──────────────────────────


fn test_stress_100_independent_signals() raises:
    var rt = _make_runtime()

    var keys = List[UInt32]()
    for i in range(100):
        keys.append(rt[].create_signal[Int32](Int32(i * 10)))

    assert_equal(rt[].signals.signal_count(), 100, "100 signals created")

    # Verify each holds its own value
    for i in range(100):
        assert_equal(
            Int(rt[].read_signal[Int32](keys[i])),
            i * 10,
            "signal " + String(i) + " holds correct value",
        )

    # Write to every other one
    for i in range(0, 100, 2):
        rt[].write_signal[Int32](keys[i], Int32(999))

    # Verify written ones changed and others didn't
    for i in range(100):
        var expected = 999 if i % 2 == 0 else i * 10
        assert_equal(
            Int(rt[].read_signal[Int32](keys[i])),
            expected,
            "signal " + String(i) + " has expected value after selective write",
        )

    _teardown(rt)


# ── Stress: create/destroy cycle reuses slots ────────────────────────────────


fn test_stress_create_destroy_reuse_cycle() raises:
    var rt = _make_runtime()

    # Create 50 signals
    var keys = List[UInt32]()
    for i in range(50):
        keys.append(rt[].create_signal[Int32](Int32(i)))

    # Destroy all even-indexed
    for i in range(0, 50, 2):
        rt[].destroy_signal(keys[i])

    assert_equal(
        rt[].signals.signal_count(), 25, "25 signals after destroying 25"
    )

    # Create 25 more — should reuse freed slots
    var new_keys = List[UInt32]()
    for i in range(25):
        new_keys.append(rt[].create_signal[Int32](Int32(1000 + i)))

    assert_equal(rt[].signals.signal_count(), 50, "back to 50 signals")

    # Verify the new signals have correct values
    for i in range(25):
        assert_equal(
            Int(rt[].read_signal[Int32](new_keys[i])),
            1000 + i,
            "reused slot " + String(i) + " has correct new value",
        )

    # Verify odd-indexed originals still intact
    for i in range(1, 50, 2):
        assert_equal(
            Int(rt[].read_signal[Int32](keys[i])),
            i,
            "original odd-indexed signal " + String(i) + " still intact",
        )

    _teardown(rt)


# ── Edge case: negative values ───────────────────────────────────────────────


fn test_signal_negative_values() raises:
    var rt = _make_runtime()

    var key = rt[].create_signal[Int32](Int32(-2147483648))  # INT32_MIN
    assert_equal(
        Int(rt[].read_signal[Int32](key)), -2147483648, "can store INT32_MIN"
    )

    rt[].write_signal[Int32](key, Int32(2147483647))  # INT32_MAX
    assert_equal(
        Int(rt[].read_signal[Int32](key)), 2147483647, "can store INT32_MAX"
    )

    _teardown(rt)


# ── Edge case: zero initial value ────────────────────────────────────────────


fn test_signal_zero_initial_value() raises:
    var rt = _make_runtime()

    var key = rt[].create_signal[Int32](Int32(0))
    assert_equal(Int(rt[].read_signal[Int32](key)), 0, "zero initial value")
    assert_equal(Int(rt[].peek_signal[Int32](key)), 0, "zero peek value")

    _teardown(rt)


# ── SignalStore direct tests ─────────────────────────────────────────────────


fn test_signal_store_create_and_count() raises:
    var store = SignalStore()
    assert_equal(store.signal_count(), 0, "empty store has 0 signals")

    var k0 = store.create[Int32](Int32(10))
    assert_equal(store.signal_count(), 1, "1 signal after create")
    assert_true(store.contains(k0), "store contains the signal")

    var k1 = store.create[Int32](Int32(20))
    assert_equal(store.signal_count(), 2, "2 signals after second create")
    assert_true(k0 != k1, "keys are distinct")


fn test_signal_store_read_write() raises:
    var store = SignalStore()

    var key = store.create[Int32](Int32(42))
    assert_equal(Int(store.read[Int32](key)), 42, "read returns initial value")

    store.write[Int32](key, Int32(99))
    assert_equal(Int(store.read[Int32](key)), 99, "read returns written value")


fn test_signal_store_peek() raises:
    var store = SignalStore()

    var key = store.create[Int32](Int32(55))
    assert_equal(Int(store.peek[Int32](key)), 55, "peek returns value")


fn test_signal_store_destroy_and_reuse() raises:
    var store = SignalStore()

    var k0 = store.create[Int32](Int32(1))
    var k1 = store.create[Int32](Int32(2))
    assert_equal(store.signal_count(), 2)

    store.destroy(k0)
    assert_equal(store.signal_count(), 1)
    assert_false(store.contains(k0), "destroyed signal gone")
    assert_true(store.contains(k1), "other signal still alive")

    # Re-create should reuse slot
    var k2 = store.create[Int32](Int32(99))
    assert_equal(Int(k2), Int(k0), "reuses freed slot")
    assert_equal(Int(store.read[Int32](k2)), 99, "new value in reused slot")


fn test_signal_store_version() raises:
    var store = SignalStore()

    var key = store.create[Int32](Int32(0))
    assert_equal(Int(store.version(key)), 0, "initial version 0")

    store.write[Int32](key, Int32(1))
    assert_equal(Int(store.version(key)), 1, "version 1 after write")

    store.write[Int32](key, Int32(2))
    assert_equal(Int(store.version(key)), 2, "version 2 after second write")

    # Read does not bump version
    _ = store.read[Int32](key)
    assert_equal(Int(store.version(key)), 2, "version still 2 after read")


fn test_signal_store_subscribe_and_count() raises:
    var store = SignalStore()

    var key = store.create[Int32](Int32(0))
    assert_equal(store.subscriber_count(key), 0, "0 subscribers initially")

    store.subscribe(key, UInt32(1))
    assert_equal(store.subscriber_count(key), 1, "1 subscriber")

    # Idempotent
    store.subscribe(key, UInt32(1))
    assert_equal(store.subscriber_count(key), 1, "still 1 (idempotent)")

    store.subscribe(key, UInt32(2))
    assert_equal(store.subscriber_count(key), 2, "2 subscribers")


fn test_signal_store_unsubscribe() raises:
    var store = SignalStore()

    var key = store.create[Int32](Int32(0))
    store.subscribe(key, UInt32(1))
    store.subscribe(key, UInt32(2))
    assert_equal(store.subscriber_count(key), 2)

    store.unsubscribe(key, UInt32(1))
    assert_equal(store.subscriber_count(key), 1, "1 after unsubscribe")

    store.unsubscribe(key, UInt32(2))
    assert_equal(store.subscriber_count(key), 0, "0 after both unsubscribed")


fn test_signal_store_get_subscribers() raises:
    var store = SignalStore()

    var key = store.create[Int32](Int32(0))
    store.subscribe(key, UInt32(10))
    store.subscribe(key, UInt32(20))
    store.subscribe(key, UInt32(30))

    var subs = store.get_subscribers(key)
    assert_equal(len(subs), 3, "3 subscribers returned")


fn test_signal_store_read_tracked() raises:
    var store = SignalStore()

    var key = store.create[Int32](Int32(42))
    assert_equal(store.subscriber_count(key), 0)

    var val = store.read_tracked[Int32](key, UInt32(99))
    assert_equal(Int(val), 42, "read_tracked returns value")
    assert_equal(
        store.subscriber_count(key), 1, "read_tracked subscribes context"
    )

    # Second read_tracked with same context is idempotent
    _ = store.read_tracked[Int32](key, UInt32(99))
    assert_equal(store.subscriber_count(key), 1, "still 1 (idempotent)")

    # Different context
    _ = store.read_tracked[Int32](key, UInt32(100))
    assert_equal(store.subscriber_count(key), 2, "2 after different context")


fn test_signal_store_contains_out_of_bounds() raises:
    var store = SignalStore()
    assert_false(store.contains(UInt32(0)), "empty store: contains(0) false")
    assert_false(
        store.contains(UInt32(999)), "empty store: contains(999) false"
    )

# Reactive Handles — Ergonomic wrappers around raw signal/memo keys.
#
# These handle types bundle a raw UInt32 key with a Runtime pointer,
# providing a safe, ergonomic API for reading and writing reactive
# values.  They are the foundation for the Dioxus-like concise API
# described in the plan's "Ergonomics-First API Design" section.
#
# Instead of:
#
#     var key = shell.use_signal_i32(0)
#     var val = shell.peek_signal_i32(key)
#     shell.write_signal_i32(key, val + 1)
#
# Developers write:
#
#     var count = SignalI32(key, runtime)
#     count += 1
#
# SignalI32 supports:
#   - peek()       — read without subscribing the current reactive context
#   - read()       — read and subscribe the current context
#   - set(value)   — write a new value (marks subscribers dirty)
#   - += -= *= //= — read-modify-write operators via peek + set
#   - __str__()    — for easy interpolation in text ("Count: " + str(count))
#
# MemoI32 supports:
#   - read()       — read the cached value (with context tracking)
#   - peek()       — read without subscribing
#   - is_dirty()   — check if recomputation is needed
#   - recompute()  — manual recompute when dirty (reads deps, writes cache)
#   - __str__()    — for easy interpolation
#
# Both types are lightweight value types (Copyable + Movable) that hold
# a non-owning pointer to the Runtime.  They do NOT manage the Runtime's
# lifetime — the ComponentContext or AppShell owns that.
#
# Thread safety: WASM is single-threaded, so no synchronisation needed.

from memory import UnsafePointer
from .runtime import Runtime


# ══════════════════════════════════════════════════════════════════════════════
# SignalI32 — Ergonomic handle for an Int32 signal
# ══════════════════════════════════════════════════════════════════════════════


struct SignalI32(Copyable, Movable, Stringable):
    """Ergonomic handle wrapping a raw signal key + runtime pointer.

    Provides operator overloading for concise reactive state management:

        var count = SignalI32(key, runtime_ptr)
        count += 1          # read-modify-write
        count -= 1
        count.set(42)       # direct write
        var v = count.peek() # read without subscribing
        var v = count.read() # read and subscribe context

    The handle does NOT own the Runtime — it holds a non-owning pointer.
    """

    var key: UInt32
    var runtime: UnsafePointer[Runtime]

    # ── Construction ─────────────────────────────────────────────────

    fn __init__(out self, key: UInt32, runtime: UnsafePointer[Runtime]):
        """Create a signal handle from a raw key and runtime pointer.

        Args:
            key: The signal's key in the Runtime's SignalStore.
            runtime: Non-owning pointer to the Runtime.
        """
        self.key = key
        self.runtime = runtime

    fn __copyinit__(out self, other: Self):
        self.key = other.key
        self.runtime = other.runtime

    fn __moveinit__(out self, deinit other: Self):
        self.key = other.key
        self.runtime = other.runtime

    # ── Read ─────────────────────────────────────────────────────────

    fn peek(self) -> Int32:
        """Read the signal value WITHOUT subscribing the current context.

        Use this for one-off reads (e.g. in event handlers) where you
        don't want the calling scope/memo/effect to re-run when the
        signal changes.

        Returns:
            The current Int32 value.
        """
        return self.runtime[0].peek_signal[Int32](self.key)

    fn read(self) -> Int32:
        """Read the signal value AND subscribe the current reactive context.

        If a scope, memo, or effect is currently rendering/computing/running,
        it will be added to this signal's subscriber set and marked dirty
        when the signal changes.

        Returns:
            The current Int32 value.
        """
        return self.runtime[0].read_signal[Int32](self.key)

    # ── Write ────────────────────────────────────────────────────────

    fn set(self, value: Int32):
        """Write a new value to the signal.

        All subscribers (scopes, memos, effects) will be marked dirty.

        Args:
            value: The new Int32 value.
        """
        self.runtime[0].write_signal[Int32](self.key, value)

    # ── Operator overloading — read-modify-write ─────────────────────

    fn __iadd__(mut self, rhs: Int32):
        """Add `rhs` to the signal value.  `count += 1`"""
        self.set(self.peek() + rhs)

    fn __isub__(mut self, rhs: Int32):
        """Subtract `rhs` from the signal value.  `count -= 1`"""
        self.set(self.peek() - rhs)

    fn __imul__(mut self, rhs: Int32):
        """Multiply the signal value by `rhs`.  `count *= 2`"""
        self.set(self.peek() * rhs)

    fn __ifloordiv__(mut self, rhs: Int32):
        """Floor-divide the signal value by `rhs`.  `count //= 2`"""
        self.set(self.peek() // rhs)

    fn __imod__(mut self, rhs: Int32):
        """Modulo the signal value by `rhs`.  `count %= 3`"""
        self.set(self.peek() % rhs)

    # ── Toggle (Bool-as-Int32) ───────────────────────────────────────

    fn toggle(self):
        """Toggle a boolean signal stored as Int32 (0 ↔ 1).

        Reads the current value and writes 1 if it was 0, or 0 otherwise.
        Useful for checkbox/switch state.
        """
        var current = self.peek()
        if current == 0:
            self.set(1)
        else:
            self.set(0)

    # ── Queries ──────────────────────────────────────────────────────

    fn version(self) -> UInt32:
        """Return the signal's write version (monotonically increasing).

        Useful for staleness checks — if the version hasn't changed,
        the value hasn't changed.
        """
        return self.runtime[0].signals.version(self.key)

    # ── Stringable ───────────────────────────────────────────────────

    fn __str__(self) -> String:
        """Return the signal value as a String for display/interpolation.

        Uses peek() so it does NOT subscribe the calling context.
        For reactive display, use read() explicitly and convert.
        """
        return String(self.peek())


# ══════════════════════════════════════════════════════════════════════════════
# MemoI32 — Ergonomic handle for an Int32 memo (computed/derived signal)
# ══════════════════════════════════════════════════════════════════════════════


struct MemoI32(Copyable, Movable, Stringable):
    """Ergonomic handle wrapping a raw memo ID + runtime pointer.

    Memos are derived values that cache their result and recompute only
    when their dependencies change.  Since Mojo WASM cannot store
    closures, the recomputation logic lives in the component — the
    memo handle provides lifecycle management and cached value access.

    Typical usage:

        var doubled = MemoI32(memo_id, runtime_ptr)

        # In render / effect:
        if doubled.is_dirty():
            doubled.begin_compute()
            var count = count_signal.read()  # subscribes memo to signal
            doubled.end_compute(count * 2)

        var text = "Doubled: " + str(doubled)

    The handle does NOT own the Runtime — it holds a non-owning pointer.
    """

    var id: UInt32
    var runtime: UnsafePointer[Runtime]

    # ── Construction ─────────────────────────────────────────────────

    fn __init__(out self, id: UInt32, runtime: UnsafePointer[Runtime]):
        """Create a memo handle from a raw ID and runtime pointer.

        Args:
            id: The memo's ID in the Runtime's MemoStore.
            runtime: Non-owning pointer to the Runtime.
        """
        self.id = id
        self.runtime = runtime

    fn __copyinit__(out self, other: Self):
        self.id = other.id
        self.runtime = other.runtime

    fn __moveinit__(out self, deinit other: Self):
        self.id = other.id
        self.runtime = other.runtime

    # ── Read ─────────────────────────────────────────────────────────

    fn read(self) -> Int32:
        """Read the memo's cached value (with context tracking).

        If a scope or effect is currently active, it will be subscribed
        to this memo's output signal and marked dirty when the memo
        recomputes to a new value.

        Returns:
            The cached Int32 value.
        """
        return self.runtime[0].memo_read_i32(self.id)

    fn peek(self) -> Int32:
        """Read the memo's cached value WITHOUT subscribing.

        Returns:
            The cached Int32 value.
        """
        # memo_read_i32 does context tracking; we need to read without it.
        # The MemoStore stores the value in its output signal (output_key),
        # which we can peek directly.
        return self.runtime[0].peek_signal[Int32](
            self.runtime[0].memos.output_key(self.id)
        )

    # ── Dirty / Recompute lifecycle ──────────────────────────────────

    fn is_dirty(self) -> Bool:
        """Check whether the memo needs recomputation.

        A memo becomes dirty when any of its input signals are written.

        Returns:
            True if the memo should be recomputed before reading.
        """
        return self.runtime[0].memo_is_dirty(self.id)

    fn begin_compute(self):
        """Begin memo recomputation.

        Sets the memo's reactive context as current, so any signals
        read during computation will be tracked as dependencies.
        Must be paired with end_compute().
        """
        self.runtime[0].memo_begin_compute(self.id)

    fn end_compute(self, value: Int32):
        """End memo recomputation and cache the result.

        Writes the computed value to the memo's output signal and
        restores the previous reactive context.

        Args:
            value: The newly computed Int32 value to cache.
        """
        self.runtime[0].memo_end_compute_i32(self.id, value)

    fn recompute_from(self, value: Int32):
        """Convenience: begin_compute + end_compute in one call.

        Use this when the computation doesn't need to read any signals
        inside the compute bracket (e.g. when the component already has
        the value).  Note: this does NOT set up dependency tracking for
        signals read outside the bracket.

        For proper dependency tracking, use begin_compute/end_compute
        and read signals between them.

        Args:
            value: The newly computed value.
        """
        self.begin_compute()
        self.end_compute(value)

    # ── Stringable ───────────────────────────────────────────────────

    fn __str__(self) -> String:
        """Return the memo's cached value as a String.

        Uses peek() so it does NOT subscribe the calling context.
        """
        return String(self.peek())


# ══════════════════════════════════════════════════════════════════════════════
# EffectI32 — Ergonomic handle for an effect (reactive side effect)
# ══════════════════════════════════════════════════════════════════════════════


struct EffectHandle(Copyable, Movable):
    """Ergonomic handle wrapping a raw effect ID + runtime pointer.

    Effects are reactive side effects that run when their dependencies
    change.  Since Mojo WASM cannot store closures, the effect logic
    lives in the component — the handle provides lifecycle management.

    Typical usage:

        var fx = EffectHandle(effect_id, runtime_ptr)

        # After event dispatch:
        if fx.is_pending():
            fx.begin_run()
            var count = count_signal.read()  # re-subscribes
            # ... perform side effect ...
            fx.end_run()

    The handle does NOT own the Runtime — it holds a non-owning pointer.
    """

    var id: UInt32
    var runtime: UnsafePointer[Runtime]

    # ── Construction ─────────────────────────────────────────────────

    fn __init__(out self, id: UInt32, runtime: UnsafePointer[Runtime]):
        """Create an effect handle from a raw ID and runtime pointer.

        Args:
            id: The effect's ID in the Runtime's EffectStore.
            runtime: Non-owning pointer to the Runtime.
        """
        self.id = id
        self.runtime = runtime

    fn __copyinit__(out self, other: Self):
        self.id = other.id
        self.runtime = other.runtime

    fn __moveinit__(out self, deinit other: Self):
        self.id = other.id
        self.runtime = other.runtime

    # ── Lifecycle ────────────────────────────────────────────────────

    fn is_pending(self) -> Bool:
        """Check whether this effect needs to run.

        An effect becomes pending when any of its subscribed signals
        are written.

        Returns:
            True if the effect should be executed.
        """
        return self.runtime[0].effect_is_pending(self.id)

    fn begin_run(self):
        """Begin effect execution.

        Sets the effect's reactive context as current, so any signals
        read during execution will be tracked as dependencies.
        Must be paired with end_run().
        """
        self.runtime[0].effect_begin_run(self.id)

    fn end_run(self):
        """End effect execution.

        Clears the pending flag and restores the previous reactive
        context.
        """
        self.runtime[0].effect_end_run(self.id)

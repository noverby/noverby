# Reactive Runtime — Global signal storage and context tracking.
#
# This module provides the core reactive infrastructure:
#
#   - `SignalStore`  — Type-erased storage for all signal values, backed
#     by raw memory. Each signal is identified by a `UInt32` key.
#
#   - `Runtime`      — Top-level runtime state: the signal store, the
#     "current reactive context" pointer, subscriber bookkeeping, and
#     the dirty-scope queue.
#
# WASM is single-threaded, so there are no synchronisation concerns.
# However, Mojo does not support module-level mutable variables, so
# we heap-allocate a single `Runtime` instance and pass its pointer
# through exported functions.
#
# Subscriber model (mirrors Dioxus):
#   - A **reactive context** is any entity (scope, memo, effect) that
#     reads signals and wants to be notified when they change.
#   - When a signal is read while a context is active, the context's ID
#     is recorded in the signal's subscriber set.
#   - When a signal is written, all subscribers are marked dirty.
#
# Memory layout per signal slot:
#   - value_ptr    : UnsafePointer[UInt8]  — type-erased value storage
#   - value_size   : Int                   — byte size of the stored value
#   - subscribers  : List[UInt32]          — context IDs subscribed to this signal
#   - version      : UInt32               — monotonic write counter (for staleness checks)

from sys import size_of
from memory import UnsafePointer, memcpy


# ── SignalEntry ──────────────────────────────────────────────────────────────


struct SignalEntry(Copyable, Movable):
    """Type-erased storage for a single signal's value + subscribers."""

    var value_ptr: UnsafePointer[UInt8]
    var value_size: Int
    var subscribers: List[UInt32]
    var version: UInt32

    # ── Construction ─────────────────────────────────────────────────

    fn __init__(out self):
        """Create an empty (uninitialised) entry."""
        self.value_ptr = UnsafePointer[UInt8]()
        self.value_size = 0
        self.subscribers = List[UInt32]()
        self.version = 0

    fn __init__(out self, ptr: UnsafePointer[UInt8], size: Int):
        """Create an entry that owns `size` bytes at `ptr`."""
        self.value_ptr = ptr
        self.value_size = size
        self.subscribers = List[UInt32]()
        self.version = 0

    fn __copyinit__(out self, other: Self):
        self.value_size = other.value_size
        self.subscribers = other.subscribers.copy()
        self.version = other.version
        if other.value_ptr and other.value_size > 0:
            self.value_ptr = UnsafePointer[UInt8].alloc(other.value_size)
            memcpy(self.value_ptr, other.value_ptr, other.value_size)
        else:
            self.value_ptr = UnsafePointer[UInt8]()

    fn __moveinit__(out self, deinit other: Self):
        self.value_ptr = other.value_ptr
        self.value_size = other.value_size
        self.subscribers = other.subscribers^
        self.version = other.version

    fn __del__(deinit self):
        """Destroy the entry, freeing value storage."""
        if self.value_ptr:
            self.value_ptr.free()

    # ── Value access ─────────────────────────────────────────────────

    @always_inline
    fn read_value[T: Copyable & Movable & AnyType](self) -> T:
        """Reinterpret the raw bytes as T and return a copy."""
        return self.value_ptr.bitcast[T]()[0].copy()

    @always_inline
    fn write_value[T: Copyable & Movable & AnyType](mut self, value: T):
        """Overwrite the stored bytes with `value` and bump version."""
        var tmp = UnsafePointer[T].alloc(1)
        tmp.init_pointee_copy(value)
        memcpy(self.value_ptr, tmp.bitcast[UInt8](), self.value_size)
        tmp.destroy_pointee()
        tmp.free()
        self.version += 1

    # ── Subscriber management ────────────────────────────────────────

    fn subscribe(mut self, context_id: UInt32):
        """Add `context_id` to the subscriber set (idempotent)."""
        for i in range(len(self.subscribers)):
            if self.subscribers[i] == context_id:
                return  # already subscribed
        self.subscribers.append(context_id)

    fn unsubscribe(mut self, context_id: UInt32):
        """Remove `context_id` from the subscriber set."""
        for i in range(len(self.subscribers)):
            if self.subscribers[i] == context_id:
                # Swap-remove for O(1)
                var last_idx = len(self.subscribers) - 1
                if i != last_idx:
                    self.subscribers[i] = self.subscribers[last_idx]
                _ = self.subscribers.pop()
                return

    fn subscriber_count(self) -> Int:
        """Return the number of subscribed contexts."""
        return len(self.subscribers)


# ── Slot state for the signal store ──────────────────────────────────────────


@fieldwise_init
struct SignalSlotState(Copyable, Movable):
    """Tracks whether a signal slot is occupied or vacant."""

    var occupied: Bool
    var next_free: Int  # Only valid when not occupied; -1 = end of free list.


# ── SignalStore ──────────────────────────────────────────────────────────────


struct SignalStore(Movable):
    """Type-erased storage for all signal values.

    Each signal is identified by a `UInt32` key.  Values are stored as
    raw byte blobs so the store does not need to be parameterised on T.
    """

    var _entries: List[SignalEntry]
    var _states: List[SignalSlotState]
    var _free_head: Int
    var _count: Int

    # ── Construction ─────────────────────────────────────────────────

    fn __init__(out self):
        self._entries = List[SignalEntry]()
        self._states = List[SignalSlotState]()
        self._free_head = -1
        self._count = 0

    fn __moveinit__(out self, deinit other: Self):
        self._entries = other._entries^
        self._states = other._states^
        self._free_head = other._free_head
        self._count = other._count

    # ── Create / Destroy ─────────────────────────────────────────────

    fn create[T: Copyable & Movable & AnyType](mut self, initial: T) -> UInt32:
        """Create a new signal with `initial` value.  Returns its key."""
        var sz = size_of[T]()

        # Allocate value storage and copy initial value into it
        var ptr = UnsafePointer[UInt8].alloc(sz)
        var tmp = UnsafePointer[T].alloc(1)
        tmp.init_pointee_copy(initial)
        memcpy(ptr, tmp.bitcast[UInt8](), sz)
        tmp.destroy_pointee()
        tmp.free()

        var entry = SignalEntry(ptr, sz)

        if self._free_head != -1:
            var idx = self._free_head
            self._free_head = self._states[idx].next_free
            self._entries[idx] = entry^
            self._states[idx] = SignalSlotState(occupied=True, next_free=-1)
            self._count += 1
            return UInt32(idx)
        else:
            var idx = len(self._entries)
            self._entries.append(entry^)
            self._states.append(SignalSlotState(occupied=True, next_free=-1))
            self._count += 1
            return UInt32(idx)

    fn destroy(mut self, key: UInt32):
        """Remove the signal at `key`, freeing its storage."""
        var idx = Int(key)
        if idx < 0 or idx >= len(self._entries):
            return
        if not self._states[idx].occupied:
            return
        # Replace with an empty entry — the old entry's __del__ frees storage
        self._entries[idx] = SignalEntry()
        self._states[idx] = SignalSlotState(
            occupied=False, next_free=self._free_head
        )
        self._free_head = idx
        self._count -= 1

    # ── Read / Write ─────────────────────────────────────────────────

    @always_inline
    fn read[T: Copyable & Movable & AnyType](self, key: UInt32) -> T:
        """Read the signal value at `key` as type T.

        This does NOT perform subscriber tracking — call `read_tracked`
        if you want the current context to be subscribed.
        """
        return self._entries[Int(key)].read_value[T]()

    fn read_tracked[
        T: Copyable & Movable & AnyType
    ](mut self, key: UInt32, context_id: UInt32) -> T:
        """Read the signal value and subscribe `context_id`.

        This is the "normal" read path during component rendering:
        the reading context is automatically subscribed so it will be
        notified when the signal changes.
        """
        self._entries[Int(key)].subscribe(context_id)
        return self._entries[Int(key)].read_value[T]()

    fn write[T: Copyable & Movable & AnyType](mut self, key: UInt32, value: T):
        """Write a new value to the signal at `key`.

        Returns without notifying subscribers — call `get_subscribers`
        afterwards to retrieve the dirty context list.
        """
        self._entries[Int(key)].write_value[T](value)

    fn peek[T: Copyable & Movable & AnyType](self, key: UInt32) -> T:
        """Read without subscribing.  Alias for `read`."""
        return self.read[T](key)

    # ── Subscriber queries ───────────────────────────────────────────

    fn subscribe(mut self, key: UInt32, context_id: UInt32):
        """Manually subscribe `context_id` to the signal at `key`."""
        self._entries[Int(key)].subscribe(context_id)

    fn unsubscribe(mut self, key: UInt32, context_id: UInt32):
        """Remove `context_id` from the signal's subscriber set."""
        self._entries[Int(key)].unsubscribe(context_id)

    fn subscriber_count(self, key: UInt32) -> Int:
        """Return how many contexts are subscribed to signal `key`."""
        return self._entries[Int(key)].subscriber_count()

    fn get_subscribers(self, key: UInt32) -> List[UInt32]:
        """Return a copy of the subscriber list for signal `key`.

        The caller typically iterates this to mark scopes dirty after
        a write.
        """
        return self._entries[Int(key)].subscribers.copy()

    fn version(self, key: UInt32) -> UInt32:
        """Return the write-version counter for signal `key`."""
        return self._entries[Int(key)].version

    # ── Queries ──────────────────────────────────────────────────────

    fn signal_count(self) -> Int:
        """Number of live signals."""
        return self._count

    fn contains(self, key: UInt32) -> Bool:
        """Check whether `key` is a live signal."""
        var idx = Int(key)
        if idx < 0 or idx >= len(self._states):
            return False
        return self._states[idx].occupied


# ── Runtime ──────────────────────────────────────────────────────────────────


struct Runtime(Movable):
    """Top-level reactive runtime state.

    Owns the signal store, the current reactive context pointer, and
    the dirty-scope queue.  A single instance is heap-allocated at
    framework init and its pointer is threaded through all exports.
    """

    var signals: SignalStore
    var current_context: Int  # -1 = no active context
    var dirty_scopes: List[UInt32]

    # ── Construction ─────────────────────────────────────────────────

    fn __init__(out self):
        self.signals = SignalStore()
        self.current_context = -1
        self.dirty_scopes = List[UInt32]()

    fn __moveinit__(out self, deinit other: Self):
        self.signals = other.signals^
        self.current_context = other.current_context
        self.dirty_scopes = other.dirty_scopes^

    # ── Context management ───────────────────────────────────────────

    @always_inline
    fn has_context(self) -> Bool:
        """Check whether a reactive context is currently active."""
        return self.current_context != -1

    @always_inline
    fn get_context(self) -> UInt32:
        """Return the current reactive context ID.

        Precondition: `has_context()` is True.
        """
        return UInt32(self.current_context)

    @always_inline
    fn set_context(mut self, context_id: UInt32):
        """Set the current reactive context."""
        self.current_context = Int(context_id)

    @always_inline
    fn clear_context(mut self):
        """Clear the current reactive context."""
        self.current_context = -1

    fn push_context(mut self, context_id: UInt32) -> Int:
        """Push a new context, returning the previous one (for restore).

        Usage:
            var prev = runtime.push_context(my_ctx)
            # ... reads subscribe to my_ctx ...
            runtime.restore_context(prev)
        """
        var prev = self.current_context
        self.current_context = Int(context_id)
        return prev

    fn restore_context(mut self, prev: Int):
        """Restore a previously saved context."""
        self.current_context = prev

    # ── Signal operations (convenience wrappers) ─────────────────────

    fn create_signal[
        T: Copyable & Movable & AnyType
    ](mut self, initial: T) -> UInt32:
        """Create a signal and return its key."""
        return self.signals.create[T](initial)

    fn read_signal[T: Copyable & Movable & AnyType](mut self, key: UInt32) -> T:
        """Read a signal, auto-subscribing the current context if any."""
        if self.has_context():
            return self.signals.read_tracked[T](key, self.get_context())
        return self.signals.read[T](key)

    fn write_signal[
        T: Copyable & Movable & AnyType
    ](mut self, key: UInt32, value: T):
        """Write a signal and collect dirty scopes.

        After writing, the signal's subscribers are appended to
        `dirty_scopes`.  The scheduler should drain `dirty_scopes`
        to re-render affected components.
        """
        self.signals.write[T](key, value)
        var subs = self.signals.get_subscribers(key)
        for i in range(len(subs)):
            # Append only if not already queued (simple linear scan for now)
            var ctx = subs[i]
            var found = False
            for j in range(len(self.dirty_scopes)):
                if self.dirty_scopes[j] == ctx:
                    found = True
                    break
            if not found:
                self.dirty_scopes.append(ctx)

    fn peek_signal[T: Copyable & Movable & AnyType](self, key: UInt32) -> T:
        """Read a signal without subscribing."""
        return self.signals.peek[T](key)

    fn destroy_signal(mut self, key: UInt32):
        """Destroy a signal, cleaning up subscribers."""
        self.signals.destroy(key)

    # ── Dirty queue ──────────────────────────────────────────────────

    fn drain_dirty(mut self) -> List[UInt32]:
        """Return and clear the dirty-scope queue."""
        var result = self.dirty_scopes^
        self.dirty_scopes = List[UInt32]()
        return result^

    fn has_dirty(self) -> Bool:
        """Check whether any scopes need re-rendering."""
        return len(self.dirty_scopes) > 0

    fn dirty_count(self) -> Int:
        """Number of scopes in the dirty queue."""
        return len(self.dirty_scopes)


# ── Heap-allocated Runtime handle ────────────────────────────────────────────
#
# Since Mojo doesn't support module-level `var`, we heap-allocate the
# Runtime and pass its pointer (as Int64) through exported WASM functions.
# These helpers create and access the heap-allocated instance.


fn create_runtime() -> UnsafePointer[Runtime]:
    """Allocate a Runtime on the heap and return a pointer to it."""
    var ptr = UnsafePointer[Runtime].alloc(1)
    ptr.init_pointee_move(Runtime())
    return ptr


fn destroy_runtime(ptr: UnsafePointer[Runtime]):
    """Destroy and free a heap-allocated Runtime."""
    ptr.destroy_pointee()
    ptr.free()

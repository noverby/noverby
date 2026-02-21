# Reactive Runtime — Global signal storage and context tracking.
#
# This module provides the core reactive infrastructure, including scope
# management for components:
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
from scope import ScopeArena, ScopeState, HOOK_SIGNAL, HOOK_MEMO, HOOK_EFFECT
from vdom import TemplateRegistry, VNodeStore
from .memo import MemoStore, MemoEntry
from .effect import EffectStore, EffectEntry
from events import (
    HandlerRegistry,
    HandlerEntry,
    ACTION_NONE,
    ACTION_SIGNAL_SET_I32,
    ACTION_SIGNAL_ADD_I32,
    ACTION_SIGNAL_SUB_I32,
    ACTION_SIGNAL_TOGGLE,
    ACTION_SIGNAL_SET_INPUT,
    ACTION_CUSTOM,
)


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
    var scopes: ScopeArena
    var templates: TemplateRegistry
    var vnodes: VNodeStore
    var handlers: HandlerRegistry
    var memos: MemoStore
    var effects: EffectStore
    var current_context: Int  # -1 = no active context
    var current_scope: Int  # -1 = no active scope (for hooks)
    var dirty_scopes: List[UInt32]
    # Context-to-memo mapping: parallel arrays.
    # When a reactive context is notified (i.e. appears in a signal's
    # subscriber list after a write), the runtime checks whether that
    # context belongs to a memo.  If so, the memo is marked dirty and
    # the memo's output signal's subscribers are also notified.
    var _memo_ctx_ids: List[UInt32]  # context IDs that belong to memos
    var _memo_ids: List[UInt32]  # corresponding memo IDs
    # Context-to-effect mapping: parallel arrays.
    # When a reactive context is notified after a signal write, the
    # runtime checks whether that context belongs to an effect.  If so,
    # the effect is marked pending (but NOT added to dirty_scopes —
    # effects run after rendering, not during).
    var _effect_ctx_ids: List[UInt32]  # context IDs that belong to effects
    var _effect_ids: List[UInt32]  # corresponding effect IDs

    # ── Construction ─────────────────────────────────────────────────

    fn __init__(out self):
        self.signals = SignalStore()
        self.scopes = ScopeArena()
        self.templates = TemplateRegistry()
        self.vnodes = VNodeStore()
        self.handlers = HandlerRegistry()
        self.memos = MemoStore()
        self.effects = EffectStore()
        self.current_context = -1
        self.current_scope = -1
        self.dirty_scopes = List[UInt32]()
        self._memo_ctx_ids = List[UInt32]()
        self._memo_ids = List[UInt32]()
        self._effect_ctx_ids = List[UInt32]()
        self._effect_ids = List[UInt32]()

    fn __moveinit__(out self, deinit other: Self):
        self.signals = other.signals^
        self.scopes = other.scopes^
        self.templates = other.templates^
        self.vnodes = other.vnodes^
        self.handlers = other.handlers^
        self.memos = other.memos^
        self.effects = other.effects^
        self.current_context = other.current_context
        self.current_scope = other.current_scope
        self.dirty_scopes = other.dirty_scopes^
        self._memo_ctx_ids = other._memo_ctx_ids^
        self._memo_ids = other._memo_ids^
        self._effect_ctx_ids = other._effect_ctx_ids^
        self._effect_ids = other._effect_ids^

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
            var ctx = subs[i]
            # 1. Check if this subscriber is a memo's reactive context
            var is_memo = False
            for m in range(len(self._memo_ctx_ids)):
                if self._memo_ctx_ids[m] == ctx:
                    # Mark the memo dirty
                    var memo_id = self._memo_ids[m]
                    if self.memos.contains(memo_id):
                        self.memos.mark_dirty(memo_id)
                        # Propagate: notify the memo's output signal subscribers
                        var out_key = self.memos.output_key(memo_id)
                        var out_subs = self.signals.get_subscribers(out_key)
                        for k in range(len(out_subs)):
                            var scope_ctx = out_subs[k]
                            # Check if this subscriber is an effect context
                            var is_eff = False
                            for e2 in range(len(self._effect_ctx_ids)):
                                if self._effect_ctx_ids[e2] == scope_ctx:
                                    var eff_id = self._effect_ids[e2]
                                    if self.effects.contains(eff_id):
                                        self.effects.mark_pending(eff_id)
                                    is_eff = True
                                    break
                            if not is_eff:
                                # Normal scope subscriber
                                var found2 = False
                                for j2 in range(len(self.dirty_scopes)):
                                    if self.dirty_scopes[j2] == scope_ctx:
                                        found2 = True
                                        break
                                if not found2:
                                    self.dirty_scopes.append(scope_ctx)
                    is_memo = True
                    break
            if is_memo:
                continue
            # 2. Check if this subscriber is an effect's reactive context
            var is_effect = False
            for e in range(len(self._effect_ctx_ids)):
                if self._effect_ctx_ids[e] == ctx:
                    # Mark the effect pending (but do NOT add to dirty_scopes —
                    # effects run after rendering, not during)
                    var effect_id = self._effect_ids[e]
                    if self.effects.contains(effect_id):
                        self.effects.mark_pending(effect_id)
                    is_effect = True
                    break
            if is_effect:
                continue
            # 3. Normal scope subscriber — append if not already queued
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

    # ── Scope management ─────────────────────────────────────────────

    fn create_scope(mut self, height: UInt32, parent_id: Int) -> UInt32:
        """Create a new scope and return its ID."""
        return self.scopes.create(height, parent_id)

    fn create_child_scope(mut self, parent_id: UInt32) -> UInt32:
        """Create a child scope whose height is parent.height + 1."""
        return self.scopes.create_child(parent_id)

    fn destroy_scope(mut self, id: UInt32):
        """Destroy a scope, freeing its slot for reuse."""
        self.scopes.destroy(id)

    fn scope_count(self) -> Int:
        """Return the number of live scopes."""
        return self.scopes.count()

    fn scope_contains(self, id: UInt32) -> Bool:
        """Check whether `id` is a live scope."""
        return self.scopes.contains(id)

    # ── Scope rendering ──────────────────────────────────────────────

    @always_inline
    fn has_scope(self) -> Bool:
        """Check whether a scope is currently active (being rendered)."""
        return self.current_scope != -1

    @always_inline
    fn get_scope(self) -> UInt32:
        """Return the current scope ID.

        Precondition: `has_scope()` is True.
        """
        return UInt32(self.current_scope)

    fn begin_scope_render(mut self, scope_id: UInt32) -> Int:
        """Begin rendering a scope.

        Sets the current scope, begins the render pass on the scope state,
        and sets the reactive context to the scope ID (so signal reads
        during rendering are tracked).

        Returns the previous scope ID (as Int, -1 if none) for restoration.
        """
        var prev_scope = self.current_scope
        self.current_scope = Int(scope_id)
        self.scopes.begin_render(scope_id)
        # Also set the reactive context to this scope so signal reads
        # during rendering auto-subscribe this scope.
        self.set_context(scope_id)
        return prev_scope

    fn end_scope_render(mut self, prev_scope: Int):
        """End rendering the current scope and restore the previous scope.

        Clears the reactive context and restores the previous scope/context.
        """
        self.current_scope = prev_scope
        if prev_scope == -1:
            self.clear_context()
        else:
            self.set_context(UInt32(prev_scope))

    # ── Hook-based signal creation ───────────────────────────────────

    fn use_signal_i32(mut self, initial: Int32) -> UInt32:
        """Hook: create or retrieve an Int32 signal for the current scope.

        On first render: creates a new signal with `initial`, stores its
        key in the scope's hook array, and returns the key.

        On re-render: retrieves the existing signal key from the hook array
        (initial value is ignored) and returns it.

        Precondition: `has_scope()` is True.
        """
        var scope_id = self.get_scope()
        if self.scopes.is_first_render(scope_id):
            # First render — create signal and store in hooks
            var key = self.signals.create[Int32](initial)
            self.scopes.push_hook(scope_id, HOOK_SIGNAL, key)
            return key
        else:
            # Re-render — return existing signal key
            return self.scopes.next_hook(scope_id)

    fn use_effect(mut self) -> UInt32:
        """Hook: create or retrieve an effect for the current scope.

        Follows the same pattern as use_signal_i32 and use_memo_i32:
          - First render: creates an effect via create_effect, pushes
            HOOK_EFFECT tag + effect ID onto the scope's hook list.
          - Re-render: advances the hook cursor and returns the existing
            effect ID.

        Precondition: current_scope is set (inside a begin/end render).
        """
        var scope_id = self.get_scope()
        if self.scopes.is_first_render(scope_id):
            # First render — create new effect
            var effect_id = self.create_effect(scope_id)
            self.scopes.push_hook(scope_id, HOOK_EFFECT, UInt32(effect_id))
            return UInt32(effect_id)
        else:
            # Re-render — return existing effect ID
            return self.scopes.next_hook(scope_id)

    fn use_memo_i32(mut self, initial: Int32) -> UInt32:
        """Hook: create or retrieve an Int32 memo for the current scope.

        On first render: creates a new memo, stores its ID in the scope's
        hook array (with HOOK_MEMO tag), and returns the memo ID.

        On re-render: retrieves the existing memo ID from the hook array
        and returns it (initial value is ignored).

        Precondition: has_scope() is True.
        """
        var scope_id = self.get_scope()
        if self.scopes.is_first_render(scope_id):
            # First render — create memo and store in hooks
            var memo_id = self.create_memo_i32(scope_id, initial)
            self.scopes.push_hook(scope_id, HOOK_MEMO, memo_id)
            return memo_id
        else:
            # Re-render — return existing memo ID
            return self.scopes.next_hook(scope_id)

    # ── Event handler management ─────────────────────────────────────

    fn register_handler(mut self, entry: HandlerEntry) -> UInt32:
        """Register an event handler and return its stable ID.

        The handler ID is used in AVAL_EVENT attribute values and by
        the JS EventBridge to dispatch events back to WASM.
        """
        return self.handlers.register(entry)

    fn remove_handler(mut self, id: UInt32):
        """Remove an event handler by ID."""
        self.handlers.remove(id)

    fn dispatch_event(mut self, handler_id: UInt32, event_type: UInt8) -> Bool:
        """Dispatch an event to the handler at `handler_id`.

        Executes the handler's action (e.g. signal write) and returns
        True if the action was executed, False if the handler was not
        found or is a no-op.

        After dispatching, affected scopes will be in the dirty queue
        (via signal write → subscriber notification).

        Args:
            handler_id: The handler to invoke.
            event_type: The DOM event type tag (EVT_CLICK, etc.).

        Returns:
            True if an action was executed, False otherwise.
        """
        if not self.handlers.contains(handler_id):
            return False

        var entry = self.handlers.get(handler_id)
        var action = entry.action

        if action == ACTION_NONE:
            # No-op handler — just mark the scope dirty directly
            var found = False
            for j in range(len(self.dirty_scopes)):
                if self.dirty_scopes[j] == entry.scope_id:
                    found = True
                    break
            if not found:
                self.dirty_scopes.append(entry.scope_id)
            return False

        elif action == ACTION_SIGNAL_SET_I32:
            self.write_signal[Int32](entry.signal_key, entry.operand)
            return True

        elif action == ACTION_SIGNAL_ADD_I32:
            var current = self.peek_signal[Int32](entry.signal_key)
            self.write_signal[Int32](entry.signal_key, current + entry.operand)
            return True

        elif action == ACTION_SIGNAL_SUB_I32:
            var current = self.peek_signal[Int32](entry.signal_key)
            self.write_signal[Int32](entry.signal_key, current - entry.operand)
            return True

        elif action == ACTION_SIGNAL_TOGGLE:
            var current = self.peek_signal[Int32](entry.signal_key)
            if current == 0:
                self.write_signal[Int32](entry.signal_key, Int32(1))
            else:
                self.write_signal[Int32](entry.signal_key, Int32(0))
            return True

        elif action == ACTION_CUSTOM:
            # Custom handlers are handled by JS — just mark scope dirty
            var found = False
            for j in range(len(self.dirty_scopes)):
                if self.dirty_scopes[j] == entry.scope_id:
                    found = True
                    break
            if not found:
                self.dirty_scopes.append(entry.scope_id)
            return False

        return False

    fn dispatch_event_with_i32(
        mut self, handler_id: UInt32, event_type: UInt8, value: Int32
    ) -> Bool:
        """Dispatch an event with an Int32 payload (e.g. from input).

        For ACTION_SIGNAL_SET_INPUT, the payload is used as the new signal
        value instead of the handler's operand.  For other actions, this
        falls back to the normal dispatch.

        Args:
            handler_id: The handler to invoke.
            event_type: The DOM event type tag.
            value: The Int32 payload from the event.

        Returns:
            True if an action was executed, False otherwise.
        """
        if not self.handlers.contains(handler_id):
            return False

        var entry = self.handlers.get(handler_id)

        if entry.action == ACTION_SIGNAL_SET_INPUT:
            self.write_signal[Int32](entry.signal_key, value)
            return True

        # Fall back to normal dispatch for other action types
        return self.dispatch_event(handler_id, event_type)

    fn handler_count(self) -> Int:
        """Return the number of live event handlers."""
        return self.handlers.count()

    # ── Memo operations ──────────────────────────────────────────────

    fn create_memo_i32(mut self, scope_id: UInt32, initial: Int32) -> UInt32:
        """Create a memo with an initial cached value.

        Allocates a reactive context (a scope-level context ID via a
        dedicated signal used only for tracking) and an output signal.
        The memo starts dirty so the first read triggers a computation.

        Returns the memo ID (not the output signal key).
        """
        # Allocate a "context signal" whose sole purpose is to act as a
        # reactive-context identifier.  We use a dummy Int32 signal whose
        # key doubles as the context_id.
        var context_id = self.signals.create[Int32](Int32(0))
        # Allocate the output signal that stores the cached result.
        var output_key = self.signals.create[Int32](initial)
        var memo_id = self.memos.create(context_id, output_key, scope_id)
        # Register the context→memo mapping for dirty propagation.
        self._memo_ctx_ids.append(context_id)
        self._memo_ids.append(memo_id)
        return memo_id

    fn memo_begin_compute(mut self, memo_id: UInt32):
        """Begin memo computation.

        Sets the memo's reactive context as the current context so that
        signal reads during computation are tracked as dependencies.
        Clears old subscriptions (re-subscribes fresh on each compute).
        """
        if not self.memos.contains(memo_id):
            return
        var entry = self.memos.get(memo_id)
        self.memos.set_computing(memo_id, True)
        # Clear old subscriptions: unsubscribe this context from all signals
        # it was previously subscribed to.  We do a full scan of signals —
        # acceptable for now since memo count is small.
        for i in range(len(self.signals._entries)):
            if (
                i < len(self.signals._states)
                and self.signals._states[i].occupied
            ):
                self.signals.unsubscribe(UInt32(i), entry.context_id)
        # Push the memo's context as the current reactive context
        # (saves the previous context for restore in end_compute).
        # We store the previous context in a simple way: use the
        # context signal's value as storage for the previous context.
        var prev = self.current_context
        self.signals.write[Int32](entry.context_id, Int32(prev))
        self.current_context = Int(entry.context_id)

    fn memo_end_compute_i32(mut self, memo_id: UInt32, value: Int32):
        """End memo computation and store the result.

        Writes the computed value to the memo's output signal and clears
        the dirty flag.  Restores the previous reactive context.
        """
        if not self.memos.contains(memo_id):
            return
        var entry = self.memos.get(memo_id)
        # Store the result in the output signal (does NOT trigger dirty
        # propagation through write_signal — we write directly to avoid
        # recursive memo updates during computation).
        self.signals.write[Int32](entry.output_key, value)
        self.memos.clear_dirty(memo_id)
        self.memos.set_computing(memo_id, False)
        # Restore previous context
        var prev = Int(self.signals.read[Int32](entry.context_id))
        self.current_context = prev

    fn memo_read_i32(mut self, memo_id: UInt32) -> Int32:
        """Read the memo's cached value.

        Subscribes the current reactive context (if any) to the memo's
        output signal, so the reader is notified when the memo recomputes.

        Does NOT trigger recomputation — the caller must check
        memo_is_dirty() and call begin/end_compute if needed.
        """
        if not self.memos.contains(memo_id):
            return Int32(0)
        var entry = self.memos.get(memo_id)
        # Subscribe the current context to the memo's output signal
        if self.has_context():
            self.signals._entries[Int(entry.output_key)].subscribe(
                self.get_context()
            )
        return self.signals.read[Int32](entry.output_key)

    fn memo_is_dirty(self, memo_id: UInt32) -> Bool:
        """Check whether the memo needs recomputation."""
        if not self.memos.contains(memo_id):
            return False
        return self.memos.is_dirty(memo_id)

    fn destroy_memo(mut self, memo_id: UInt32):
        """Destroy a memo, cleaning up its context and output signal.

        Removes the context→memo mapping, destroys the context signal
        and output signal, and frees the memo slot.
        """
        if not self.memos.contains(memo_id):
            return
        var entry = self.memos.get(memo_id)
        # Remove context→memo mapping
        for i in range(len(self._memo_ctx_ids)):
            if self._memo_ctx_ids[i] == entry.context_id:
                # Swap-remove for O(1)
                var last = len(self._memo_ctx_ids) - 1
                if i != last:
                    self._memo_ctx_ids[i] = self._memo_ctx_ids[last]
                    self._memo_ids[i] = self._memo_ids[last]
                _ = self._memo_ctx_ids.pop()
                _ = self._memo_ids.pop()
                break
        # Destroy the context signal and output signal
        self.signals.destroy(entry.context_id)
        self.signals.destroy(entry.output_key)
        # Destroy the memo entry
        self.memos.destroy(memo_id)

    fn memo_count(self) -> Int:
        """Return the number of live memos."""
        return self.memos.count()

    fn memo_output_key(self, memo_id: UInt32) -> UInt32:
        """Return the output signal key of the memo (for testing)."""
        return self.memos.output_key(memo_id)

    fn memo_context_id(self, memo_id: UInt32) -> UInt32:
        """Return the reactive context ID of the memo (for testing)."""
        return self.memos.context_id(memo_id)

    # ── Effect operations ────────────────────────────────────────────

    fn create_effect(mut self, scope_id: UInt32) -> UInt32:
        """Create an effect with a reactive context.

        Allocates a "context signal" (a dummy Int32 signal whose key
        doubles as the context_id) for dependency tracking.
        The effect starts pending so the first run is triggered.

        Returns the effect ID.
        """
        # Allocate a context signal for dependency tracking
        var context_id = self.signals.create[Int32](Int32(0))
        var effect_id = self.effects.create(context_id, scope_id)
        # Register the context→effect mapping for pending propagation
        self._effect_ctx_ids.append(context_id)
        self._effect_ids.append(effect_id)
        return effect_id

    fn effect_begin_run(mut self, effect_id: UInt32):
        """Begin effect execution.

        Sets the effect's reactive context as the current context so that
        signal reads during execution are tracked as dependencies.
        Clears old subscriptions (re-subscribes fresh on each run).
        """
        if not self.effects.contains(effect_id):
            return
        var entry = self.effects.get(effect_id)
        self.effects.set_running(effect_id, True)
        # Clear old subscriptions: unsubscribe this context from all signals
        # it was previously subscribed to.  Full scan — acceptable for now
        # since effect count is small.
        for i in range(len(self.signals._entries)):
            if (
                i < len(self.signals._states)
                and self.signals._states[i].occupied
            ):
                self.signals.unsubscribe(UInt32(i), entry.context_id)
        # Save previous context in the context signal's value
        var prev = self.current_context
        self.signals.write[Int32](entry.context_id, Int32(prev))
        self.current_context = Int(entry.context_id)

    fn effect_end_run(mut self, effect_id: UInt32):
        """End effect execution.

        Clears the pending flag and running flag.
        Restores the previous reactive context.
        """
        if not self.effects.contains(effect_id):
            return
        var entry = self.effects.get(effect_id)
        self.effects.clear_pending(effect_id)
        self.effects.set_running(effect_id, False)
        # Restore previous context
        var prev = Int(self.signals.read[Int32](entry.context_id))
        self.current_context = prev

    fn effect_is_pending(self, effect_id: UInt32) -> Bool:
        """Check whether the effect needs re-execution."""
        if not self.effects.contains(effect_id):
            return False
        return self.effects.is_pending(effect_id)

    fn destroy_effect(mut self, effect_id: UInt32):
        """Destroy an effect, cleaning up its context signal.

        Removes the context→effect mapping, destroys the context signal,
        and frees the effect slot.
        """
        if not self.effects.contains(effect_id):
            return
        var entry = self.effects.get(effect_id)
        # Remove context→effect mapping
        for i in range(len(self._effect_ctx_ids)):
            if self._effect_ctx_ids[i] == entry.context_id:
                # Swap-remove for O(1)
                var last = len(self._effect_ctx_ids) - 1
                if i != last:
                    self._effect_ctx_ids[i] = self._effect_ctx_ids[last]
                    self._effect_ids[i] = self._effect_ids[last]
                _ = self._effect_ctx_ids.pop()
                _ = self._effect_ids.pop()
                break
        # Destroy the context signal
        self.signals.destroy(entry.context_id)
        # Destroy the effect entry
        self.effects.destroy(effect_id)

    fn effect_count(self) -> Int:
        """Return the number of live effects."""
        return self.effects.count()

    fn effect_context_id(self, effect_id: UInt32) -> UInt32:
        """Return the reactive context ID of the effect (for testing)."""
        return self.effects.context_id(effect_id)

    fn drain_pending_effects(self) -> List[UInt32]:
        """Return a list of effect IDs that are currently pending.

        Does NOT clear pending flags — the caller must begin_run/end_run
        each effect to clear them.
        """
        return self.effects.pending_effects()

    fn pending_effect_count(self) -> Int:
        """Return the number of pending effects."""
        return len(self.effects.pending_effects())

    fn pending_effect_at(self, index: Int) -> UInt32:
        """Return the effect ID at the given index in the pending list.

        This is a convenience for WASM export (avoids returning a List).
        Precondition: index < pending_effect_count().
        """
        var pending = self.effects.pending_effects()
        return pending[index]


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

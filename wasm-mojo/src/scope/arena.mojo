# ScopeArena — Slab-based scope storage.
#
# Manages a pool of ScopeState instances identified by UInt32 IDs.
# Uses the same slab pattern as SignalStore and ElementIdAllocator:
# an intrusive free list provides O(1) alloc/free with automatic
# reuse of freed IDs.
#
# Scope ID 0 is valid (unlike ElementId where 0 is reserved for root).
# The arena does not impose any hierarchy — parent/child relationships
# are tracked by ScopeState.parent_id and ScopeState.height.

from memory import UnsafePointer
from .scope import ScopeState


# ── Slot state ───────────────────────────────────────────────────────────────


@fieldwise_init
struct _ScopeSlotState(Copyable, Movable):
    """Tracks whether a scope slot is occupied or vacant."""

    var occupied: Bool
    var next_free: Int  # Only meaningful when not occupied; -1 = end of free list.


# ── ScopeArena ───────────────────────────────────────────────────────────────


struct ScopeArena(Movable):
    """Slab-based storage for ScopeState instances.

    Each scope is identified by a UInt32 key returned from `create`.
    Freed keys are recycled for future allocations.

    Usage:
        var arena = ScopeArena()
        var id = arena.create(height=0, parent_id=-1)  # root scope
        arena.get(id).begin_render()
        arena.destroy(id)
    """

    var _scopes: List[ScopeState]
    var _states: List[_ScopeSlotState]
    var _free_head: Int
    var _count: Int

    # ── Construction ─────────────────────────────────────────────────

    fn __init__(out self):
        self._scopes = List[ScopeState]()
        self._states = List[_ScopeSlotState]()
        self._free_head = -1
        self._count = 0

    fn __init__(out self, *, capacity: Int):
        """Create an arena with pre-allocated capacity."""
        self._scopes = List[ScopeState](capacity=capacity)
        self._states = List[_ScopeSlotState](capacity=capacity)
        self._free_head = -1
        self._count = 0

    fn __moveinit__(out self, deinit other: Self):
        self._scopes = other._scopes^
        self._states = other._states^
        self._free_head = other._free_head
        self._count = other._count

    # ── Create / Destroy ─────────────────────────────────────────────

    fn create(mut self, height: UInt32, parent_id: Int) -> UInt32:
        """Create a new scope and return its ID.

        Args:
            height: Depth in the component tree (root = 0).
            parent_id: Parent scope ID as Int, or -1 for root scopes.

        Returns:
            The UInt32 scope ID.
        """
        if self._free_head != -1:
            # Reuse a freed slot
            var idx = self._free_head
            self._free_head = self._states[idx].next_free
            self._scopes[idx] = ScopeState(UInt32(idx), height, parent_id)
            self._states[idx] = _ScopeSlotState(occupied=True, next_free=-1)
            self._count += 1
            return UInt32(idx)
        else:
            # Append a new slot
            var idx = len(self._scopes)
            self._scopes.append(ScopeState(UInt32(idx), height, parent_id))
            self._states.append(_ScopeSlotState(occupied=True, next_free=-1))
            self._count += 1
            return UInt32(idx)

    fn create_child(mut self, parent_id: UInt32) -> UInt32:
        """Create a child scope whose height is parent.height + 1.

        Convenience method that reads the parent's height automatically.

        Args:
            parent_id: The parent scope's ID.

        Returns:
            The UInt32 scope ID of the new child.
        """
        var parent_height = self._scopes[Int(parent_id)].height
        return self.create(parent_height + 1, Int(parent_id))

    fn destroy(mut self, id: UInt32):
        """Destroy the scope at `id`, freeing its slot for reuse.

        Destroying a non-existent or already-freed scope is a no-op.
        """
        var idx = Int(id)
        if idx < 0 or idx >= len(self._scopes):
            return
        if not self._states[idx].occupied:
            return
        # Replace with a dummy scope — the old scope's storage is released
        self._scopes[idx] = ScopeState(UInt32(0), UInt32(0), -1)
        self._states[idx] = _ScopeSlotState(
            occupied=False, next_free=self._free_head
        )
        self._free_head = idx
        self._count -= 1

    # ── Access ───────────────────────────────────────────────────────

    fn get_ptr(self, id: UInt32) -> UnsafePointer[ScopeState]:
        """Return a pointer to the ScopeState at `id`.

        The caller must ensure `id` refers to a live scope.
        The pointer is valid until the next mutation of the arena.
        """
        return self._scopes.unsafe_ptr() + Int(id)

    # ── Queries ──────────────────────────────────────────────────────

    fn count(self) -> Int:
        """Return the number of live scopes."""
        return self._count

    fn contains(self, id: UInt32) -> Bool:
        """Check whether `id` is a live scope."""
        var idx = Int(id)
        if idx < 0 or idx >= len(self._states):
            return False
        return self._states[idx].occupied

    fn height(self, id: UInt32) -> UInt32:
        """Return the height (depth) of the scope at `id`."""
        return self._scopes[Int(id)].height

    fn parent_id(self, id: UInt32) -> Int:
        """Return the parent ID of the scope at `id`, or -1 if root."""
        return self._scopes[Int(id)].parent_id

    fn is_dirty(self, id: UInt32) -> Bool:
        """Check whether the scope at `id` is dirty (needs re-render)."""
        return self._scopes[Int(id)].dirty

    fn render_count(self, id: UInt32) -> UInt32:
        """Return how many times the scope at `id` has been rendered."""
        return self._scopes[Int(id)].render_count

    fn hook_count(self, id: UInt32) -> Int:
        """Return the number of hooks in the scope at `id`."""
        return self._scopes[Int(id)].hook_count()

    # ── Mutators (delegating to ScopeState) ──────────────────────────

    fn set_dirty(mut self, id: UInt32, dirty: Bool):
        """Set the dirty flag on the scope at `id`."""
        if dirty:
            self._scopes[Int(id)].mark_dirty()
        else:
            self._scopes[Int(id)].clear_dirty()

    fn begin_render(mut self, id: UInt32):
        """Begin a render pass on the scope at `id`.

        Resets hook cursor, increments render count, clears dirty flag.
        """
        self._scopes[Int(id)].begin_render()

    fn push_hook(mut self, id: UInt32, tag: UInt8, value: UInt32):
        """Push a new hook onto the scope at `id`."""
        self._scopes[Int(id)].push_hook(tag, value)

    fn next_hook(mut self, id: UInt32) -> UInt32:
        """Advance the hook cursor on scope `id` and return the current value.
        """
        return self._scopes[Int(id)].next_hook()

    fn has_more_hooks(self, id: UInt32) -> Bool:
        """Check if scope `id` has more hooks to process."""
        return self._scopes[Int(id)].has_more_hooks()

    fn is_first_render(self, id: UInt32) -> Bool:
        """Check if scope `id` is on its first render."""
        return self._scopes[Int(id)].is_first_render()

    fn hook_value_at(self, id: UInt32, index: Int) -> UInt32:
        """Return the hook value at position `index` in scope `id`."""
        return self._scopes[Int(id)].hook_value_at(index)

    fn hook_tag_at(self, id: UInt32, index: Int) -> UInt8:
        """Return the hook tag at position `index` in scope `id`."""
        return self._scopes[Int(id)].hook_tag_at(index)

    fn hook_cursor(self, id: UInt32) -> Int:
        """Return the current hook cursor position for scope `id`."""
        return self._scopes[Int(id)].hook_cursor

    # ── Bulk operations ──────────────────────────────────────────────

    fn clear(mut self):
        """Destroy all scopes."""
        self._scopes.clear()
        self._states.clear()
        self._free_head = -1
        self._count = 0

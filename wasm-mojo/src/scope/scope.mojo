# ScopeState — Per-component instance state.
#
# Each mounted component gets a scope that tracks:
#   - Its position in the component tree (height, parent)
#   - Whether it needs re-rendering (dirty flag)
#   - Hook storage (signal keys, memo IDs, etc.)
#   - Render lifecycle (render count, hook cursor)
#
# Hooks use positional indexing: the first `signal()` call in a component
# always maps to hook slot 0, the second to slot 1, etc.  On first render,
# hooks are created and stored.  On subsequent renders, the hook cursor
# advances through the existing slots, returning stable handles.
#
# The hook_values list stores UInt32 keys that refer to entries in the
# runtime's SignalStore (or future MemoStore, EffectStore).  A tag byte
# in hook_tags distinguishes the hook type.

from memory import UnsafePointer


# ── Hook type tags ───────────────────────────────────────────────────────────

alias HOOK_SIGNAL: UInt8 = 0
alias HOOK_MEMO: UInt8 = 1
alias HOOK_EFFECT: UInt8 = 2


# ── ScopeState ───────────────────────────────────────────────────────────────


struct ScopeState(Copyable, Movable):
    """Per-component instance state.

    Tracks the component's position in the tree, its dirty status,
    and its hook storage for reactive primitives (signals, memos, effects).
    """

    var id: UInt32
    var height: UInt32
    var parent_id: Int  # -1 if root scope (no parent)
    var dirty: Bool
    var render_count: UInt32
    var hook_cursor: Int  # current position during render (reset each render)

    # Hook storage: parallel arrays of (tag, value)
    # tag = HOOK_SIGNAL | HOOK_MEMO | HOOK_EFFECT
    # value = key into the appropriate runtime store
    var hook_tags: List[UInt8]
    var hook_values: List[UInt32]

    # ── Construction ─────────────────────────────────────────────────

    fn __init__(out self, id: UInt32, height: UInt32, parent_id: Int):
        """Create a new scope state.

        Args:
            id: The scope's unique identifier.
            height: Depth in the component tree (root = 0).
            parent_id: Parent scope ID, or -1 for root scopes.
        """
        self.id = id
        self.height = height
        self.parent_id = parent_id
        self.dirty = False
        self.render_count = 0
        self.hook_cursor = 0
        self.hook_tags = List[UInt8]()
        self.hook_values = List[UInt32]()

    fn __copyinit__(out self, other: Self):
        self.id = other.id
        self.height = other.height
        self.parent_id = other.parent_id
        self.dirty = other.dirty
        self.render_count = other.render_count
        self.hook_cursor = other.hook_cursor
        self.hook_tags = other.hook_tags.copy()
        self.hook_values = other.hook_values.copy()

    fn __moveinit__(out self, deinit other: Self):
        self.id = other.id
        self.height = other.height
        self.parent_id = other.parent_id
        self.dirty = other.dirty
        self.render_count = other.render_count
        self.hook_cursor = other.hook_cursor
        self.hook_tags = other.hook_tags^
        self.hook_values = other.hook_values^

    # ── Render lifecycle ─────────────────────────────────────────────

    fn begin_render(mut self):
        """Begin a render pass.

        Resets the hook cursor to 0 so hooks are accessed in order.
        Increments the render count.  Clears the dirty flag.
        """
        self.hook_cursor = 0
        self.render_count += 1
        self.dirty = False

    fn is_first_render(self) -> Bool:
        """Check if this scope has never been rendered.

        Returns True if render_count is 0 (before first begin_render)
        or 1 (during first render pass).
        """
        return self.render_count <= 1

    # ── Hook management ──────────────────────────────────────────────

    fn hook_count(self) -> Int:
        """Return the number of hooks registered in this scope."""
        return len(self.hook_values)

    fn push_hook(mut self, tag: UInt8, value: UInt32):
        """Register a new hook at the current cursor position.

        Called during first render to create a new hook slot.
        Advances the hook cursor.
        """
        self.hook_tags.append(tag)
        self.hook_values.append(value)
        self.hook_cursor += 1

    fn next_hook(mut self) -> UInt32:
        """Advance the hook cursor and return the value at the current position.

        Called during re-render to retrieve an existing hook's stored value.
        """
        var idx = self.hook_cursor
        self.hook_cursor += 1
        return self.hook_values[idx]

    fn next_hook_tag(self) -> UInt8:
        """Return the tag of the hook at the current cursor position.

        Does NOT advance the cursor — call next_hook() to advance.
        """
        return self.hook_tags[self.hook_cursor]

    fn has_more_hooks(self) -> Bool:
        """Check if there are more hooks to process in this render pass."""
        return self.hook_cursor < len(self.hook_values)

    fn hook_value_at(self, index: Int) -> UInt32:
        """Return the hook value at the given index (for introspection)."""
        return self.hook_values[index]

    fn hook_tag_at(self, index: Int) -> UInt8:
        """Return the hook tag at the given index (for introspection)."""
        return self.hook_tags[index]

    # ── Queries ──────────────────────────────────────────────────────

    fn is_root(self) -> Bool:
        """Check whether this is a root scope (no parent)."""
        return self.parent_id == -1

    fn mark_dirty(mut self):
        """Mark this scope as needing re-render."""
        self.dirty = True

    fn clear_dirty(mut self):
        """Clear the dirty flag (called after re-render)."""
        self.dirty = False

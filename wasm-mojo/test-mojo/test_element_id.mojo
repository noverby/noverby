# Tests for ElementIdAllocator — native Mojo tests run with `mojo test`.
#
# These tests are a direct port of test/element_id.test.ts, exercising
# the same scenarios without the WASM/JS round-trip.
#
# Run with:
#   mojo test -I src test-mojo/test_element_id.mojo

from testing import assert_equal, assert_true, assert_false

from arena import ElementId, ElementIdAllocator, ROOT_ELEMENT_ID


# ── ElementId basics ─────────────────────────────────────────────────────────


fn test_element_id_root() raises:
    var root = ElementId(UInt32(0))
    assert_true(root.is_root(), "ElementId(0) should be root")
    assert_false(
        root.is_valid(), "ElementId(0) should not be valid (it is root)"
    )
    assert_equal(root.as_u32(), UInt32(0))


fn test_element_id_non_root() raises:
    var id = ElementId(UInt32(1))
    assert_false(id.is_root(), "ElementId(1) should not be root")
    assert_true(id.is_valid(), "ElementId(1) should be valid")
    assert_equal(id.as_u32(), UInt32(1))


fn test_element_id_equality() raises:
    var a = ElementId(UInt32(5))
    var b = ElementId(UInt32(5))
    var c = ElementId(UInt32(6))
    assert_true(a == b, "same IDs should be equal")
    assert_true(a != c, "different IDs should not be equal")


fn test_element_id_from_int() raises:
    var id = ElementId(42)
    assert_equal(id.as_u32(), UInt32(42))
    assert_equal(id.as_int(), 42)


fn test_root_element_id_alias() raises:
    var root = materialize[ROOT_ELEMENT_ID]()
    assert_true(root.is_root(), "ROOT_ELEMENT_ID should be root")
    assert_equal(root.as_u32(), UInt32(0))


# ── Allocator — basic allocation ─────────────────────────────────────────────


fn test_allocator_initial_state() raises:
    var alloc = ElementIdAllocator()
    # Root is pre-reserved, so count=1, user_count=0
    assert_equal(alloc.count(), 1, "initial count should be 1 (root)")
    assert_equal(alloc.user_count(), 0, "initial user_count should be 0")


fn test_allocator_first_alloc_returns_1() raises:
    var alloc = ElementIdAllocator()
    var id = alloc.alloc()
    assert_equal(id.as_u32(), UInt32(1), "first alloc should return ID 1")
    assert_equal(alloc.count(), 2)
    assert_equal(alloc.user_count(), 1)


fn test_allocator_sequential_alloc() raises:
    var alloc = ElementIdAllocator()
    var id1 = alloc.alloc()
    var id2 = alloc.alloc()
    var id3 = alloc.alloc()
    assert_equal(id1.as_u32(), UInt32(1))
    assert_equal(id2.as_u32(), UInt32(2))
    assert_equal(id3.as_u32(), UInt32(3))
    assert_equal(alloc.count(), 4)
    assert_equal(alloc.user_count(), 3)


fn test_allocator_is_alive() raises:
    var alloc = ElementIdAllocator()
    var id1 = alloc.alloc()
    var id2 = alloc.alloc()
    assert_true(alloc.is_alive(id1), "id1 should be alive")
    assert_true(alloc.is_alive(id2), "id2 should be alive")
    assert_true(alloc.is_alive(ElementId(UInt32(0))), "root should be alive")
    assert_false(
        alloc.is_alive(ElementId(UInt32(99))),
        "unallocated ID should not be alive",
    )


# ── Allocator — free and reuse ───────────────────────────────────────────────


fn test_allocator_free_decrements_count() raises:
    var alloc = ElementIdAllocator()
    var id1 = alloc.alloc()
    var id2 = alloc.alloc()
    assert_equal(alloc.user_count(), 2)

    alloc.free(id1)
    assert_equal(alloc.user_count(), 1)
    assert_false(alloc.is_alive(id1), "freed ID should not be alive")
    assert_true(alloc.is_alive(id2), "other ID should still be alive")


fn test_allocator_free_root_is_noop() raises:
    var alloc = ElementIdAllocator()
    _ = alloc.alloc()
    var count_before = alloc.count()

    alloc.free(ElementId(UInt32(0)))
    assert_equal(
        alloc.count(), count_before, "freeing root should not change count"
    )
    assert_true(
        alloc.is_alive(ElementId(UInt32(0))),
        "root should still be alive after free attempt",
    )


fn test_allocator_double_free_is_noop() raises:
    var alloc = ElementIdAllocator()
    var id = alloc.alloc()
    alloc.free(id)
    var count_after_first_free = alloc.count()

    alloc.free(id)  # double free
    assert_equal(
        alloc.count(),
        count_after_first_free,
        "double free should not change count",
    )


fn test_allocator_reuse_freed_slot() raises:
    var alloc = ElementIdAllocator()
    var id1 = alloc.alloc()  # gets 1
    var id2 = alloc.alloc()  # gets 2

    alloc.free(id1)  # free slot 1

    var id3 = alloc.alloc()  # should reuse slot 1
    assert_equal(
        id3.as_u32(), id1.as_u32(), "new alloc should reuse freed slot"
    )
    assert_true(alloc.is_alive(id3), "reused ID should be alive")
    assert_true(alloc.is_alive(id2), "other ID should still be alive")


fn test_allocator_reuse_multiple_freed_slots() raises:
    var alloc = ElementIdAllocator()
    var id1 = alloc.alloc()  # 1
    var id2 = alloc.alloc()  # 2
    var id3 = alloc.alloc()  # 3

    alloc.free(id1)
    alloc.free(id3)
    assert_equal(alloc.user_count(), 1, "only id2 should remain")

    # Allocate two more — should reuse freed slots (LIFO order from free list)
    var id4 = alloc.alloc()
    var id5 = alloc.alloc()
    assert_true(alloc.is_alive(id4))
    assert_true(alloc.is_alive(id5))
    assert_equal(alloc.user_count(), 3)

    # The reused IDs should be from {1, 3}
    var reused_a = id4.as_u32()
    var reused_b = id5.as_u32()
    assert_true(
        (reused_a == 1 and reused_b == 3) or (reused_a == 3 and reused_b == 1),
        "should reuse freed slots 1 and 3",
    )


# ── Allocator — next_id ─────────────────────────────────────────────────────


fn test_allocator_next_id_without_free_list() raises:
    var alloc = ElementIdAllocator()
    # No free list, so next_id is len(slots) which is 1 (slot 0 = root)
    assert_equal(
        alloc.next_id().as_u32(),
        UInt32(1),
        "next_id should be 1 initially",
    )

    _ = alloc.alloc()  # allocates 1
    assert_equal(
        alloc.next_id().as_u32(),
        UInt32(2),
        "next_id should be 2 after one alloc",
    )


fn test_allocator_next_id_with_free_list() raises:
    var alloc = ElementIdAllocator()
    var id1 = alloc.alloc()  # 1
    _ = alloc.alloc()  # 2

    alloc.free(id1)
    assert_equal(
        alloc.next_id().as_u32(),
        id1.as_u32(),
        "next_id should be the freed slot",
    )


# ── Allocator — clear ────────────────────────────────────────────────────────


fn test_allocator_clear() raises:
    var alloc = ElementIdAllocator()
    _ = alloc.alloc()
    _ = alloc.alloc()
    _ = alloc.alloc()
    assert_equal(alloc.user_count(), 3)

    alloc.clear()
    assert_equal(alloc.count(), 1, "after clear, only root remains")
    assert_equal(alloc.user_count(), 0, "after clear, no user IDs")

    # Should be able to allocate again starting from 1
    var id = alloc.alloc()
    assert_equal(id.as_u32(), UInt32(1), "first alloc after clear should be 1")


# ── Allocator — capacity constructor ─────────────────────────────────────────


fn test_allocator_with_capacity() raises:
    var alloc = ElementIdAllocator(capacity=100)
    assert_equal(alloc.count(), 1, "capacity ctor still reserves root")
    assert_equal(alloc.user_count(), 0)

    var id = alloc.alloc()
    assert_equal(id.as_u32(), UInt32(1))


# ── Stress — many allocations ────────────────────────────────────────────────


fn test_allocator_stress_100() raises:
    var alloc = ElementIdAllocator()
    var ids = List[ElementId]()

    for i in range(100):
        ids.append(alloc.alloc())

    assert_equal(alloc.user_count(), 100)

    # All IDs should be unique and alive
    for i in range(100):
        assert_true(alloc.is_alive(ids[i]))
        assert_equal(ids[i].as_u32(), UInt32(i + 1))


fn test_allocator_stress_free_even_realloc() raises:
    var alloc = ElementIdAllocator()
    var ids = List[ElementId]()

    # Allocate 50
    for i in range(50):
        ids.append(alloc.alloc())

    # Free all even-indexed (0, 2, 4, ...)
    for i in range(0, 50, 2):
        alloc.free(ids[i])

    assert_equal(alloc.user_count(), 25, "25 should remain after freeing 25")

    # Allocate 25 more — should reuse freed slots
    var new_ids = List[ElementId]()
    for i in range(25):
        new_ids.append(alloc.alloc())

    assert_equal(alloc.user_count(), 50, "back to 50 after reallocating")

    # All new IDs should be alive
    for i in range(25):
        assert_true(alloc.is_alive(new_ids[i]))

    # Original odd-indexed should still be alive with correct values
    for i in range(1, 50, 2):
        assert_true(alloc.is_alive(ids[i]))
        assert_equal(ids[i].as_u32(), UInt32(i + 1))


fn test_allocator_stress_alloc_free_cycle() raises:
    """Allocate and free in a tight loop to exercise slot reuse."""
    var alloc = ElementIdAllocator()

    for _ in range(1000):
        var id = alloc.alloc()
        assert_true(alloc.is_alive(id))
        alloc.free(id)

    # After all cycles, should be back to just root
    assert_equal(alloc.user_count(), 0)
    # But one slot was created and is on the free list
    assert_equal(alloc.count(), 1)

    # Next alloc should reuse that slot
    var id = alloc.alloc()
    assert_equal(id.as_u32(), UInt32(1))

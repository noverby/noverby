# Tests for MutationWriter — native Mojo tests run with `mojo test`.
#
# These tests verify the binary encoding of DOM mutations without
# the WASM/JS round-trip.  Each test allocates a buffer, writes
# mutations via MutationWriter, then reads back raw bytes to verify
# the encoding matches the expected binary layout.
#
# Run with:
#   mojo test -I src test-mojo/test_protocol.mojo

from testing import assert_equal, assert_true, assert_false

from memory import UnsafePointer

from bridge.protocol import (
    MutationWriter,
    OP_END,
    OP_APPEND_CHILDREN,
    OP_ASSIGN_ID,
    OP_CREATE_PLACEHOLDER,
    OP_CREATE_TEXT_NODE,
    OP_LOAD_TEMPLATE,
    OP_REPLACE_WITH,
    OP_REPLACE_PLACEHOLDER,
    OP_INSERT_AFTER,
    OP_INSERT_BEFORE,
    OP_SET_ATTRIBUTE,
    OP_SET_TEXT,
    OP_NEW_EVENT_LISTENER,
    OP_REMOVE_EVENT_LISTENER,
    OP_REMOVE,
    OP_PUSH_ROOT,
)


# ── Buffer helpers ───────────────────────────────────────────────────────────

alias BUF_SIZE = 4096


fn _alloc_buf() -> UnsafePointer[UInt8]:
    """Allocate a zeroed buffer for mutation writing."""
    var buf = UnsafePointer[UInt8].alloc(BUF_SIZE)
    for i in range(BUF_SIZE):
        buf[i] = 0
    return buf


fn _free_buf(buf: UnsafePointer[UInt8]):
    buf.free()


fn _read_u8(buf: UnsafePointer[UInt8], offset: Int) -> UInt8:
    return buf[offset]


fn _read_u16_le(buf: UnsafePointer[UInt8], offset: Int) -> UInt16:
    return UInt16(buf[offset]) | (UInt16(buf[offset + 1]) << 8)


fn _read_u32_le(buf: UnsafePointer[UInt8], offset: Int) -> UInt32:
    return (
        UInt32(buf[offset])
        | (UInt32(buf[offset + 1]) << 8)
        | (UInt32(buf[offset + 2]) << 16)
        | (UInt32(buf[offset + 3]) << 24)
    )


fn _read_str(buf: UnsafePointer[UInt8], offset: Int) -> (String, Int):
    """Read a u32-length-prefixed string. Returns (string, bytes_consumed)."""
    var length = Int(_read_u32_le(buf, offset))
    var chars = List[UInt8](capacity=length + 1)
    for i in range(length):
        chars.append(buf[offset + 4 + i])
    chars.append(0)  # null terminator
    return (String(chars^), 4 + length)


fn _read_short_str(buf: UnsafePointer[UInt8], offset: Int) -> (String, Int):
    """Read a u16-length-prefixed string. Returns (string, bytes_consumed)."""
    var length = Int(_read_u16_le(buf, offset))
    var chars = List[UInt8](capacity=length + 1)
    for i in range(length):
        chars.append(buf[offset + 2 + i])
    chars.append(0)  # null terminator
    return (String(chars^), 2 + length)


# ── End sentinel ─────────────────────────────────────────────────────────────


fn test_end_sentinel() raises:
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    w.end()

    assert_equal(Int(_read_u8(buf, 0)), Int(OP_END), "end writes 0x00")
    assert_equal(w.offset, 1, "offset advances by 1")

    _free_buf(buf)


fn test_finalize_is_end_alias() raises:
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    w.finalize()

    assert_equal(Int(_read_u8(buf, 0)), Int(OP_END), "finalize writes 0x00")
    assert_equal(w.offset, 1, "offset advances by 1")

    _free_buf(buf)


fn test_empty_buffer_starts_at_zero() raises:
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    assert_equal(w.offset, 0, "initial offset is 0")

    _free_buf(buf)


fn test_writer_with_initial_offset() raises:
    var buf = _alloc_buf()
    var w = MutationWriter(buf, 10, BUF_SIZE)

    assert_equal(w.offset, 10, "initial offset is 10")
    w.end()
    assert_equal(w.offset, 11, "after end, offset is 11")
    assert_equal(Int(_read_u8(buf, 10)), Int(OP_END))

    _free_buf(buf)


# ── AppendChildren ───────────────────────────────────────────────────────────


fn test_append_children() raises:
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    w.append_children(UInt32(7), UInt32(3))
    w.end()

    assert_equal(
        Int(_read_u8(buf, 0)),
        Int(OP_APPEND_CHILDREN),
        "opcode is APPEND_CHILDREN",
    )
    assert_equal(Int(_read_u32_le(buf, 1)), 7, "id is 7")
    assert_equal(Int(_read_u32_le(buf, 5)), 3, "m is 3")
    assert_equal(Int(_read_u8(buf, 9)), Int(OP_END), "terminated with END")
    assert_equal(w.offset, 10, "offset is 1 + 4 + 4 + 1 = 10")

    _free_buf(buf)


fn test_append_children_zero() raises:
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    w.append_children(UInt32(1), UInt32(0))
    w.end()

    assert_equal(Int(_read_u8(buf, 0)), Int(OP_APPEND_CHILDREN))
    assert_equal(Int(_read_u32_le(buf, 1)), 1, "id is 1")
    assert_equal(Int(_read_u32_le(buf, 5)), 0, "m is 0 (zero children)")

    _free_buf(buf)


# ── CreatePlaceholder ────────────────────────────────────────────────────────


fn test_create_placeholder() raises:
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    w.create_placeholder(UInt32(42))
    w.end()

    assert_equal(
        Int(_read_u8(buf, 0)),
        Int(OP_CREATE_PLACEHOLDER),
        "opcode is CREATE_PLACEHOLDER",
    )
    assert_equal(Int(_read_u32_le(buf, 1)), 42, "id is 42")
    assert_equal(Int(_read_u8(buf, 5)), Int(OP_END))
    assert_equal(w.offset, 6, "offset is 1 + 4 + 1 = 6")

    _free_buf(buf)


# ── CreateTextNode ───────────────────────────────────────────────────────────


fn test_create_text_node() raises:
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    w.create_text_node(UInt32(5), String("hello"))
    w.end()

    assert_equal(
        Int(_read_u8(buf, 0)),
        Int(OP_CREATE_TEXT_NODE),
        "opcode is CREATE_TEXT_NODE",
    )
    assert_equal(Int(_read_u32_le(buf, 1)), 5, "id is 5")
    # u32 length prefix
    assert_equal(Int(_read_u32_le(buf, 5)), 5, "text length is 5")
    # text bytes
    assert_equal(Int(_read_u8(buf, 9)), Int(ord("h")))
    assert_equal(Int(_read_u8(buf, 10)), Int(ord("e")))
    assert_equal(Int(_read_u8(buf, 11)), Int(ord("l")))
    assert_equal(Int(_read_u8(buf, 12)), Int(ord("l")))
    assert_equal(Int(_read_u8(buf, 13)), Int(ord("o")))
    assert_equal(Int(_read_u8(buf, 14)), Int(OP_END))

    _free_buf(buf)


fn test_create_text_node_empty_string() raises:
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    w.create_text_node(UInt32(1), String(""))
    w.end()

    assert_equal(Int(_read_u8(buf, 0)), Int(OP_CREATE_TEXT_NODE))
    assert_equal(Int(_read_u32_le(buf, 1)), 1, "id is 1")
    assert_equal(Int(_read_u32_le(buf, 5)), 0, "text length is 0")
    assert_equal(Int(_read_u8(buf, 9)), Int(OP_END))

    _free_buf(buf)


fn test_create_text_node_unicode() raises:
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    var text = String("héllo")
    var text_len = len(text)
    w.create_text_node(UInt32(2), text)
    w.end()

    assert_equal(Int(_read_u8(buf, 0)), Int(OP_CREATE_TEXT_NODE))
    assert_equal(Int(_read_u32_le(buf, 1)), 2, "id is 2")
    assert_equal(
        Int(_read_u32_le(buf, 5)), text_len, "text length matches UTF-8 bytes"
    )

    _free_buf(buf)


# ── LoadTemplate ─────────────────────────────────────────────────────────────


fn test_load_template() raises:
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    w.load_template(UInt32(10), UInt32(0), UInt32(100))
    w.end()

    assert_equal(
        Int(_read_u8(buf, 0)),
        Int(OP_LOAD_TEMPLATE),
        "opcode is LOAD_TEMPLATE",
    )
    assert_equal(Int(_read_u32_le(buf, 1)), 10, "tmpl_id is 10")
    assert_equal(Int(_read_u32_le(buf, 5)), 0, "index is 0")
    assert_equal(Int(_read_u32_le(buf, 9)), 100, "id is 100")
    assert_equal(Int(_read_u8(buf, 13)), Int(OP_END))
    assert_equal(w.offset, 14, "offset is 1 + 4 + 4 + 4 + 1 = 14")

    _free_buf(buf)


# ── ReplaceWith ──────────────────────────────────────────────────────────────


fn test_replace_with() raises:
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    w.replace_with(UInt32(5), UInt32(2))
    w.end()

    assert_equal(
        Int(_read_u8(buf, 0)), Int(OP_REPLACE_WITH), "opcode is REPLACE_WITH"
    )
    assert_equal(Int(_read_u32_le(buf, 1)), 5, "id is 5")
    assert_equal(Int(_read_u32_le(buf, 5)), 2, "m is 2")
    assert_equal(Int(_read_u8(buf, 9)), Int(OP_END))

    _free_buf(buf)


# ── InsertAfter ──────────────────────────────────────────────────────────────


fn test_insert_after() raises:
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    w.insert_after(UInt32(8), UInt32(1))
    w.end()

    assert_equal(
        Int(_read_u8(buf, 0)), Int(OP_INSERT_AFTER), "opcode is INSERT_AFTER"
    )
    assert_equal(Int(_read_u32_le(buf, 1)), 8, "id is 8")
    assert_equal(Int(_read_u32_le(buf, 5)), 1, "m is 1")
    assert_equal(Int(_read_u8(buf, 9)), Int(OP_END))

    _free_buf(buf)


# ── InsertBefore ─────────────────────────────────────────────────────────────


fn test_insert_before() raises:
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    w.insert_before(UInt32(9), UInt32(4))
    w.end()

    assert_equal(
        Int(_read_u8(buf, 0)), Int(OP_INSERT_BEFORE), "opcode is INSERT_BEFORE"
    )
    assert_equal(Int(_read_u32_le(buf, 1)), 9, "id is 9")
    assert_equal(Int(_read_u32_le(buf, 5)), 4, "m is 4")
    assert_equal(Int(_read_u8(buf, 9)), Int(OP_END))

    _free_buf(buf)


# ── Remove ───────────────────────────────────────────────────────────────────


fn test_remove() raises:
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    w.remove(UInt32(15))
    w.end()

    assert_equal(Int(_read_u8(buf, 0)), Int(OP_REMOVE), "opcode is REMOVE")
    assert_equal(Int(_read_u32_le(buf, 1)), 15, "id is 15")
    assert_equal(Int(_read_u8(buf, 5)), Int(OP_END))
    assert_equal(w.offset, 6, "offset is 1 + 4 + 1 = 6")

    _free_buf(buf)


# ── PushRoot ─────────────────────────────────────────────────────────────────


fn test_push_root() raises:
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    w.push_root(UInt32(20))
    w.end()

    assert_equal(
        Int(_read_u8(buf, 0)), Int(OP_PUSH_ROOT), "opcode is PUSH_ROOT"
    )
    assert_equal(Int(_read_u32_le(buf, 1)), 20, "id is 20")
    assert_equal(Int(_read_u8(buf, 5)), Int(OP_END))

    _free_buf(buf)


# ── SetText ──────────────────────────────────────────────────────────────────


fn test_set_text() raises:
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    w.set_text(UInt32(3), String("world"))
    w.end()

    assert_equal(Int(_read_u8(buf, 0)), Int(OP_SET_TEXT), "opcode is SET_TEXT")
    assert_equal(Int(_read_u32_le(buf, 1)), 3, "id is 3")
    assert_equal(Int(_read_u32_le(buf, 5)), 5, "text length is 5")
    assert_equal(Int(_read_u8(buf, 9)), Int(ord("w")))
    assert_equal(Int(_read_u8(buf, 10)), Int(ord("o")))
    assert_equal(Int(_read_u8(buf, 11)), Int(ord("r")))
    assert_equal(Int(_read_u8(buf, 12)), Int(ord("l")))
    assert_equal(Int(_read_u8(buf, 13)), Int(ord("d")))
    assert_equal(Int(_read_u8(buf, 14)), Int(OP_END))

    _free_buf(buf)


# ── SetAttribute ─────────────────────────────────────────────────────────────


fn test_set_attribute() raises:
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    w.set_attribute(UInt32(7), UInt8(0), String("class"), String("active"))
    w.end()

    var off = 0
    assert_equal(
        Int(_read_u8(buf, off)),
        Int(OP_SET_ATTRIBUTE),
        "opcode is SET_ATTRIBUTE",
    )
    off += 1

    assert_equal(Int(_read_u32_le(buf, off)), 7, "id is 7")
    off += 4

    assert_equal(Int(_read_u8(buf, off)), 0, "ns is 0 (no namespace)")
    off += 1

    # name is u16-length-prefixed
    assert_equal(Int(_read_u16_le(buf, off)), 5, "name length is 5")
    off += 2
    assert_equal(Int(_read_u8(buf, off)), Int(ord("c")))
    assert_equal(Int(_read_u8(buf, off + 1)), Int(ord("l")))
    assert_equal(Int(_read_u8(buf, off + 2)), Int(ord("a")))
    assert_equal(Int(_read_u8(buf, off + 3)), Int(ord("s")))
    assert_equal(Int(_read_u8(buf, off + 4)), Int(ord("s")))
    off += 5

    # value is u32-length-prefixed
    assert_equal(Int(_read_u32_le(buf, off)), 6, "value length is 6")
    off += 4
    assert_equal(Int(_read_u8(buf, off)), Int(ord("a")))
    assert_equal(Int(_read_u8(buf, off + 1)), Int(ord("c")))
    assert_equal(Int(_read_u8(buf, off + 2)), Int(ord("t")))
    assert_equal(Int(_read_u8(buf, off + 3)), Int(ord("i")))
    assert_equal(Int(_read_u8(buf, off + 4)), Int(ord("v")))
    assert_equal(Int(_read_u8(buf, off + 5)), Int(ord("e")))
    off += 6

    assert_equal(Int(_read_u8(buf, off)), Int(OP_END))

    _free_buf(buf)


fn test_set_attribute_with_namespace() raises:
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    w.set_attribute(UInt32(1), UInt8(1), String("href"), String("url"))
    w.end()

    assert_equal(Int(_read_u8(buf, 0)), Int(OP_SET_ATTRIBUTE))
    assert_equal(Int(_read_u32_le(buf, 1)), 1, "id is 1")
    assert_equal(Int(_read_u8(buf, 5)), 1, "ns is 1 (xlink)")

    _free_buf(buf)


fn test_set_attribute_empty_value() raises:
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    w.set_attribute(UInt32(1), UInt8(0), String("disabled"), String(""))
    w.end()

    var off = 0
    assert_equal(Int(_read_u8(buf, off)), Int(OP_SET_ATTRIBUTE))
    off += 1 + 4 + 1  # op + id + ns

    # name: "disabled" = 8 chars
    assert_equal(Int(_read_u16_le(buf, off)), 8, "name length is 8")
    off += 2 + 8

    # value: empty
    assert_equal(Int(_read_u32_le(buf, off)), 0, "value length is 0")
    off += 4

    assert_equal(Int(_read_u8(buf, off)), Int(OP_END))

    _free_buf(buf)


# ── NewEventListener ─────────────────────────────────────────────────────────


fn test_new_event_listener() raises:
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    w.new_event_listener(UInt32(11), String("click"))
    w.end()

    var off = 0
    assert_equal(
        Int(_read_u8(buf, off)),
        Int(OP_NEW_EVENT_LISTENER),
        "opcode is NEW_EVENT_LISTENER",
    )
    off += 1

    assert_equal(Int(_read_u32_le(buf, off)), 11, "id is 11")
    off += 4

    # name is u16-length-prefixed
    assert_equal(Int(_read_u16_le(buf, off)), 5, "name length is 5")
    off += 2
    assert_equal(Int(_read_u8(buf, off)), Int(ord("c")))
    assert_equal(Int(_read_u8(buf, off + 1)), Int(ord("l")))
    assert_equal(Int(_read_u8(buf, off + 2)), Int(ord("i")))
    assert_equal(Int(_read_u8(buf, off + 3)), Int(ord("c")))
    assert_equal(Int(_read_u8(buf, off + 4)), Int(ord("k")))
    off += 5

    assert_equal(Int(_read_u8(buf, off)), Int(OP_END))

    _free_buf(buf)


# ── RemoveEventListener ─────────────────────────────────────────────────────


fn test_remove_event_listener() raises:
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    w.remove_event_listener(UInt32(11), String("click"))
    w.end()

    var off = 0
    assert_equal(
        Int(_read_u8(buf, off)),
        Int(OP_REMOVE_EVENT_LISTENER),
        "opcode is REMOVE_EVENT_LISTENER",
    )
    off += 1

    assert_equal(Int(_read_u32_le(buf, off)), 11, "id is 11")
    off += 4

    assert_equal(Int(_read_u16_le(buf, off)), 5, "name length is 5")
    off += 2 + 5

    assert_equal(Int(_read_u8(buf, off)), Int(OP_END))

    _free_buf(buf)


# ── AssignId ─────────────────────────────────────────────────────────────────


fn test_assign_id() raises:
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    # Build a path: [0, 1, 2]
    var path = UnsafePointer[UInt8].alloc(3)
    path[0] = 0
    path[1] = 1
    path[2] = 2

    w.assign_id(path, 3, UInt32(50))
    w.end()

    var off = 0
    assert_equal(
        Int(_read_u8(buf, off)), Int(OP_ASSIGN_ID), "opcode is ASSIGN_ID"
    )
    off += 1

    # path_len (u8)
    assert_equal(Int(_read_u8(buf, off)), 3, "path_len is 3")
    off += 1

    # path bytes
    assert_equal(Int(_read_u8(buf, off)), 0, "path[0] is 0")
    assert_equal(Int(_read_u8(buf, off + 1)), 1, "path[1] is 1")
    assert_equal(Int(_read_u8(buf, off + 2)), 2, "path[2] is 2")
    off += 3

    # id
    assert_equal(Int(_read_u32_le(buf, off)), 50, "id is 50")
    off += 4

    assert_equal(Int(_read_u8(buf, off)), Int(OP_END))

    path.free()
    _free_buf(buf)


fn test_assign_id_empty_path() raises:
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    var path = UnsafePointer[UInt8].alloc(1)  # dummy, won't be read
    w.assign_id(path, 0, UInt32(1))
    w.end()

    assert_equal(Int(_read_u8(buf, 0)), Int(OP_ASSIGN_ID))
    assert_equal(Int(_read_u8(buf, 1)), 0, "path_len is 0")
    assert_equal(Int(_read_u32_le(buf, 2)), 1, "id is 1")
    assert_equal(Int(_read_u8(buf, 6)), Int(OP_END))

    path.free()
    _free_buf(buf)


fn test_assign_id_single_element_path() raises:
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    var path = UnsafePointer[UInt8].alloc(1)
    path[0] = 5

    w.assign_id(path, 1, UInt32(99))
    w.end()

    assert_equal(Int(_read_u8(buf, 0)), Int(OP_ASSIGN_ID))
    assert_equal(Int(_read_u8(buf, 1)), 1, "path_len is 1")
    assert_equal(Int(_read_u8(buf, 2)), 5, "path[0] is 5")
    assert_equal(Int(_read_u32_le(buf, 3)), 99, "id is 99")
    assert_equal(Int(_read_u8(buf, 7)), Int(OP_END))

    path.free()
    _free_buf(buf)


# ── ReplacePlaceholder ───────────────────────────────────────────────────────


fn test_replace_placeholder() raises:
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    var path = UnsafePointer[UInt8].alloc(2)
    path[0] = 0
    path[1] = 3

    w.replace_placeholder(path, 2, UInt32(1))
    w.end()

    var off = 0
    assert_equal(
        Int(_read_u8(buf, off)),
        Int(OP_REPLACE_PLACEHOLDER),
        "opcode is REPLACE_PLACEHOLDER",
    )
    off += 1

    assert_equal(Int(_read_u8(buf, off)), 2, "path_len is 2")
    off += 1

    assert_equal(Int(_read_u8(buf, off)), 0, "path[0] is 0")
    assert_equal(Int(_read_u8(buf, off + 1)), 3, "path[1] is 3")
    off += 2

    assert_equal(Int(_read_u32_le(buf, off)), 1, "m is 1")
    off += 4

    assert_equal(Int(_read_u8(buf, off)), Int(OP_END))

    path.free()
    _free_buf(buf)


# ── Multiple mutations in sequence ───────────────────────────────────────────


fn test_multiple_mutations_in_sequence() raises:
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    w.create_placeholder(UInt32(1))
    w.push_root(UInt32(2))
    w.append_children(UInt32(0), UInt32(2))
    w.end()

    var off = 0

    # First: create_placeholder
    assert_equal(Int(_read_u8(buf, off)), Int(OP_CREATE_PLACEHOLDER))
    off += 1
    assert_equal(Int(_read_u32_le(buf, off)), 1)
    off += 4

    # Second: push_root
    assert_equal(Int(_read_u8(buf, off)), Int(OP_PUSH_ROOT))
    off += 1
    assert_equal(Int(_read_u32_le(buf, off)), 2)
    off += 4

    # Third: append_children
    assert_equal(Int(_read_u8(buf, off)), Int(OP_APPEND_CHILDREN))
    off += 1
    assert_equal(Int(_read_u32_le(buf, off)), 0)
    off += 4
    assert_equal(Int(_read_u32_le(buf, off)), 2)
    off += 4

    # End
    assert_equal(Int(_read_u8(buf, off)), Int(OP_END))

    _free_buf(buf)


# ── Mixed mutations with strings ─────────────────────────────────────────────


fn test_mixed_mutations_with_strings() raises:
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    w.load_template(UInt32(0), UInt32(0), UInt32(1))
    w.create_text_node(UInt32(2), String("hi"))
    w.append_children(UInt32(1), UInt32(1))
    w.new_event_listener(UInt32(1), String("click"))
    w.end()

    var off = 0

    # LoadTemplate
    assert_equal(Int(_read_u8(buf, off)), Int(OP_LOAD_TEMPLATE))
    off += 1 + 4 + 4 + 4  # op + tmpl_id + index + id = 13

    # CreateTextNode
    assert_equal(Int(_read_u8(buf, off)), Int(OP_CREATE_TEXT_NODE))
    off += 1
    assert_equal(Int(_read_u32_le(buf, off)), 2, "text node id is 2")
    off += 4
    assert_equal(Int(_read_u32_le(buf, off)), 2, "text length is 2")
    off += 4
    assert_equal(Int(_read_u8(buf, off)), Int(ord("h")))
    assert_equal(Int(_read_u8(buf, off + 1)), Int(ord("i")))
    off += 2

    # AppendChildren
    assert_equal(Int(_read_u8(buf, off)), Int(OP_APPEND_CHILDREN))
    off += 1 + 4 + 4

    # NewEventListener
    assert_equal(Int(_read_u8(buf, off)), Int(OP_NEW_EVENT_LISTENER))
    off += 1
    assert_equal(Int(_read_u32_le(buf, off)), 1)
    off += 4
    assert_equal(Int(_read_u16_le(buf, off)), 5, "event name length is 5")
    off += 2 + 5

    # End
    assert_equal(Int(_read_u8(buf, off)), Int(OP_END))

    _free_buf(buf)


# ── Max u32 values ───────────────────────────────────────────────────────────


fn test_max_u32_values() raises:
    """Ensure the writer correctly encodes the maximum u32 value."""
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    var max_u32 = UInt32(0xFFFFFFFF)
    w.push_root(max_u32)
    w.end()

    assert_equal(Int(_read_u8(buf, 0)), Int(OP_PUSH_ROOT))
    assert_equal(_read_u32_le(buf, 1), max_u32, "max u32 encodes correctly")

    _free_buf(buf)


fn test_zero_ids() raises:
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    w.push_root(UInt32(0))
    w.end()

    assert_equal(Int(_read_u8(buf, 0)), Int(OP_PUSH_ROOT))
    assert_equal(Int(_read_u32_le(buf, 1)), 0, "zero id encodes correctly")

    _free_buf(buf)


# ── Long string payload ─────────────────────────────────────────────────────


fn test_long_string_payload() raises:
    """Test encoding a 1KB string in a text node."""
    var big_size = 8192
    var buf = UnsafePointer[UInt8].alloc(big_size)
    for i in range(big_size):
        buf[i] = 0

    var w = MutationWriter(buf, big_size)

    # Build a 1024-char string
    var long_str = String("")
    for _ in range(1024):
        long_str += "x"

    w.create_text_node(UInt32(1), long_str)
    w.end()

    assert_equal(Int(_read_u8(buf, 0)), Int(OP_CREATE_TEXT_NODE))
    assert_equal(Int(_read_u32_le(buf, 1)), 1, "id is 1")
    assert_equal(Int(_read_u32_le(buf, 5)), 1024, "text length is 1024")

    # Verify all bytes are 'x'
    var all_x = True
    for i in range(1024):
        if buf[9 + i] != UInt8(ord("x")):
            all_x = False
            break

    assert_true(all_x, "all 1024 bytes are 'x'")

    # End sentinel
    assert_equal(Int(_read_u8(buf, 9 + 1024)), Int(OP_END))

    buf.free()


# ── All opcodes in one buffer ────────────────────────────────────────────────


fn test_all_opcodes_in_one_buffer() raises:
    """Write one of each opcode into a single buffer and verify opcodes appear
    in the correct order."""
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    var path = UnsafePointer[UInt8].alloc(1)
    path[0] = 0

    # Write one of each mutation type
    w.append_children(UInt32(1), UInt32(1))  # OP_APPEND_CHILDREN
    w.assign_id(path, 1, UInt32(2))  # OP_ASSIGN_ID
    w.create_placeholder(UInt32(3))  # OP_CREATE_PLACEHOLDER
    w.create_text_node(UInt32(4), String("t"))  # OP_CREATE_TEXT_NODE
    w.load_template(UInt32(5), UInt32(0), UInt32(6))  # OP_LOAD_TEMPLATE
    w.replace_with(UInt32(7), UInt32(1))  # OP_REPLACE_WITH
    w.replace_placeholder(path, 1, UInt32(1))  # OP_REPLACE_PLACEHOLDER
    w.insert_after(UInt32(8), UInt32(1))  # OP_INSERT_AFTER
    w.insert_before(UInt32(9), UInt32(1))  # OP_INSERT_BEFORE
    w.set_attribute(
        UInt32(10), UInt8(0), String("a"), String("b")
    )  # OP_SET_ATTRIBUTE
    w.set_text(UInt32(11), String("x"))  # OP_SET_TEXT
    w.new_event_listener(UInt32(12), String("e"))  # OP_NEW_EVENT_LISTENER
    w.remove_event_listener(UInt32(13), String("e"))  # OP_REMOVE_EVENT_LISTENER
    w.remove(UInt32(14))  # OP_REMOVE
    w.push_root(UInt32(15))  # OP_PUSH_ROOT
    w.end()  # OP_END

    # Walk through and extract just the opcodes
    var opcodes = List[UInt8]()
    var off = 0

    # APPEND_CHILDREN: op(1) + id(4) + m(4) = 9
    opcodes.append(_read_u8(buf, off))
    off += 9

    # ASSIGN_ID: op(1) + path_len(1) + path(1) + id(4) = 7
    opcodes.append(_read_u8(buf, off))
    off += 7

    # CREATE_PLACEHOLDER: op(1) + id(4) = 5
    opcodes.append(_read_u8(buf, off))
    off += 5

    # CREATE_TEXT_NODE: op(1) + id(4) + len(4) + "t"(1) = 10
    opcodes.append(_read_u8(buf, off))
    off += 10

    # LOAD_TEMPLATE: op(1) + tmpl_id(4) + index(4) + id(4) = 13
    opcodes.append(_read_u8(buf, off))
    off += 13

    # REPLACE_WITH: op(1) + id(4) + m(4) = 9
    opcodes.append(_read_u8(buf, off))
    off += 9

    # REPLACE_PLACEHOLDER: op(1) + path_len(1) + path(1) + m(4) = 7
    opcodes.append(_read_u8(buf, off))
    off += 7

    # INSERT_AFTER: op(1) + id(4) + m(4) = 9
    opcodes.append(_read_u8(buf, off))
    off += 9

    # INSERT_BEFORE: op(1) + id(4) + m(4) = 9
    opcodes.append(_read_u8(buf, off))
    off += 9

    # SET_ATTRIBUTE: op(1) + id(4) + ns(1) + name_len(2) + "a"(1) + val_len(4) + "b"(1) = 14
    opcodes.append(_read_u8(buf, off))
    off += 14

    # SET_TEXT: op(1) + id(4) + len(4) + "x"(1) = 10
    opcodes.append(_read_u8(buf, off))
    off += 10

    # NEW_EVENT_LISTENER: op(1) + id(4) + name_len(2) + "e"(1) = 8
    opcodes.append(_read_u8(buf, off))
    off += 8

    # REMOVE_EVENT_LISTENER: op(1) + id(4) + name_len(2) + "e"(1) = 8
    opcodes.append(_read_u8(buf, off))
    off += 8

    # REMOVE: op(1) + id(4) = 5
    opcodes.append(_read_u8(buf, off))
    off += 5

    # PUSH_ROOT: op(1) + id(4) = 5
    opcodes.append(_read_u8(buf, off))
    off += 5

    # END: op(1) = 1
    opcodes.append(_read_u8(buf, off))

    assert_equal(len(opcodes), 16, "should have 16 opcodes (including END)")

    assert_equal(Int(opcodes[0]), Int(OP_APPEND_CHILDREN))
    assert_equal(Int(opcodes[1]), Int(OP_ASSIGN_ID))
    assert_equal(Int(opcodes[2]), Int(OP_CREATE_PLACEHOLDER))
    assert_equal(Int(opcodes[3]), Int(OP_CREATE_TEXT_NODE))
    assert_equal(Int(opcodes[4]), Int(OP_LOAD_TEMPLATE))
    assert_equal(Int(opcodes[5]), Int(OP_REPLACE_WITH))
    assert_equal(Int(opcodes[6]), Int(OP_REPLACE_PLACEHOLDER))
    assert_equal(Int(opcodes[7]), Int(OP_INSERT_AFTER))
    assert_equal(Int(opcodes[8]), Int(OP_INSERT_BEFORE))
    assert_equal(Int(opcodes[9]), Int(OP_SET_ATTRIBUTE))
    assert_equal(Int(opcodes[10]), Int(OP_SET_TEXT))
    assert_equal(Int(opcodes[11]), Int(OP_NEW_EVENT_LISTENER))
    assert_equal(Int(opcodes[12]), Int(OP_REMOVE_EVENT_LISTENER))
    assert_equal(Int(opcodes[13]), Int(OP_REMOVE))
    assert_equal(Int(opcodes[14]), Int(OP_PUSH_ROOT))
    assert_equal(Int(opcodes[15]), Int(OP_END))

    path.free()
    _free_buf(buf)


# ── Offset tracking: total bytes written ─────────────────────────────────────


fn test_offset_tracking() raises:
    """Verify that the writer's offset accurately tracks total bytes written."""
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    assert_equal(w.offset, 0)

    # push_root: 1 + 4 = 5
    w.push_root(UInt32(1))
    assert_equal(w.offset, 5)

    # remove: 1 + 4 = 5  => total 10
    w.remove(UInt32(2))
    assert_equal(w.offset, 10)

    # create_text_node("ab"): 1 + 4 + 4 + 2 = 11  => total 21
    w.create_text_node(UInt32(3), String("ab"))
    assert_equal(w.offset, 21)

    # end: 1  => total 22
    w.end()
    assert_equal(w.offset, 22)

    _free_buf(buf)


# ── Offset threading: non-zero start offset ──────────────────────────────────


fn test_offset_threading_nonzero_start() raises:
    """Start writing at a non-zero offset and verify data lands correctly."""
    var buf = _alloc_buf()

    # First writer at offset 0
    var w1 = MutationWriter(buf, BUF_SIZE)
    w1.push_root(UInt32(1))
    w1.end()
    var first_end = w1.offset

    # Second writer starting where the first left off
    var w2 = MutationWriter(buf, first_end, BUF_SIZE)
    w2.push_root(UInt32(2))
    w2.end()

    # Verify first segment
    assert_equal(Int(_read_u8(buf, 0)), Int(OP_PUSH_ROOT))
    assert_equal(Int(_read_u32_le(buf, 1)), 1)
    assert_equal(Int(_read_u8(buf, 5)), Int(OP_END))

    # Verify second segment
    assert_equal(Int(_read_u8(buf, first_end)), Int(OP_PUSH_ROOT))
    assert_equal(Int(_read_u32_le(buf, first_end + 1)), 2)
    assert_equal(Int(_read_u8(buf, first_end + 5)), Int(OP_END))

    _free_buf(buf)


# ── Readback of trailing data after End ──────────────────────────────────────


fn test_trailing_data_after_end_is_zero() raises:
    """Bytes beyond the End sentinel should still be zero (from our zeroed alloc).
    """
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    w.push_root(UInt32(1))
    w.end()

    var end_offset = w.offset
    # Check a few bytes after
    for i in range(10):
        assert_equal(
            Int(_read_u8(buf, end_offset + i)),
            0,
            "byte at offset "
            + String(end_offset + i)
            + " should be 0 (trailing)",
        )

    _free_buf(buf)


# ── Opcode values match expected constants ───────────────────────────────────


fn test_opcode_values() raises:
    """Verify opcode constants have the expected numeric values matching the JS side.
    """
    assert_equal(Int(OP_END), 0x00)
    assert_equal(Int(OP_APPEND_CHILDREN), 0x01)
    assert_equal(Int(OP_ASSIGN_ID), 0x02)
    assert_equal(Int(OP_CREATE_PLACEHOLDER), 0x03)
    assert_equal(Int(OP_CREATE_TEXT_NODE), 0x04)
    assert_equal(Int(OP_LOAD_TEMPLATE), 0x05)
    assert_equal(Int(OP_REPLACE_WITH), 0x06)
    assert_equal(Int(OP_REPLACE_PLACEHOLDER), 0x07)
    assert_equal(Int(OP_INSERT_AFTER), 0x08)
    assert_equal(Int(OP_INSERT_BEFORE), 0x09)
    assert_equal(Int(OP_SET_ATTRIBUTE), 0x0A)
    assert_equal(Int(OP_SET_TEXT), 0x0B)
    assert_equal(Int(OP_NEW_EVENT_LISTENER), 0x0C)
    assert_equal(Int(OP_REMOVE_EVENT_LISTENER), 0x0D)
    assert_equal(Int(OP_REMOVE), 0x0E)
    assert_equal(Int(OP_PUSH_ROOT), 0x0F)


# ── Little-endian encoding correctness ───────────────────────────────────────


fn test_little_endian_u32_encoding() raises:
    """Verify that multi-byte integers are encoded in little-endian order."""
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    # 0x04030201 in little-endian should be: 01 02 03 04
    w.push_root(UInt32(0x04030201))
    w.end()

    # Skip the opcode byte
    assert_equal(Int(buf[1]), 0x01, "LE byte 0")
    assert_equal(Int(buf[2]), 0x02, "LE byte 1")
    assert_equal(Int(buf[3]), 0x03, "LE byte 2")
    assert_equal(Int(buf[4]), 0x04, "LE byte 3")

    _free_buf(buf)


fn test_little_endian_u16_encoding() raises:
    """Verify that u16 values in short strings are little-endian."""
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    # NewEventListener uses u16 for the name length
    # A 300-char string (0x012C) should encode as: 2C 01
    var name = String("")
    for _ in range(300):
        name += "a"
    w.new_event_listener(UInt32(1), name)
    w.end()

    # op(1) + id(4) = offset 5 for the u16 name length
    assert_equal(Int(buf[5]), 0x2C, "LE u16 low byte")
    assert_equal(Int(buf[6]), 0x01, "LE u16 high byte")

    _free_buf(buf)


# ── String content verification ──────────────────────────────────────────────


fn test_set_text_string_content() raises:
    """Verify each byte of a known string in SET_TEXT."""
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    w.set_text(UInt32(1), String("ABCD"))
    w.end()

    # op(1) + id(4) + len(4) = offset 9 for the string data
    assert_equal(Int(buf[9]), 65, "A = 65")
    assert_equal(Int(buf[10]), 66, "B = 66")
    assert_equal(Int(buf[11]), 67, "C = 67")
    assert_equal(Int(buf[12]), 68, "D = 68")

    _free_buf(buf)


fn test_set_attribute_name_content() raises:
    """Verify the attribute name bytes in SET_ATTRIBUTE."""
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    w.set_attribute(UInt32(1), UInt8(0), String("id"), String("x"))
    w.end()

    # op(1) + id(4) + ns(1) + name_len(2) = offset 8 for name bytes
    assert_equal(Int(buf[8]), Int(ord("i")), "name[0] is 'i'")
    assert_equal(Int(buf[9]), Int(ord("d")), "name[1] is 'd'")

    # After name: val_len(4) at offset 10, then val at offset 14
    assert_equal(Int(_read_u32_le(buf, 10)), 1, "value length is 1")
    assert_equal(Int(buf[14]), Int(ord("x")), "value[0] is 'x'")

    _free_buf(buf)


# ── Multiple text nodes back to back ─────────────────────────────────────────


fn test_multiple_text_nodes() raises:
    """Write two text nodes back to back and verify both are readable."""
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    w.create_text_node(UInt32(1), String("AB"))
    w.create_text_node(UInt32(2), String("CD"))
    w.end()

    var off = 0

    # First text node
    assert_equal(Int(_read_u8(buf, off)), Int(OP_CREATE_TEXT_NODE))
    off += 1
    assert_equal(Int(_read_u32_le(buf, off)), 1, "first node id is 1")
    off += 4
    assert_equal(Int(_read_u32_le(buf, off)), 2, "first text length is 2")
    off += 4
    assert_equal(Int(_read_u8(buf, off)), Int(ord("A")))
    assert_equal(Int(_read_u8(buf, off + 1)), Int(ord("B")))
    off += 2

    # Second text node
    assert_equal(Int(_read_u8(buf, off)), Int(OP_CREATE_TEXT_NODE))
    off += 1
    assert_equal(Int(_read_u32_le(buf, off)), 2, "second node id is 2")
    off += 4
    assert_equal(Int(_read_u32_le(buf, off)), 2, "second text length is 2")
    off += 4
    assert_equal(Int(_read_u8(buf, off)), Int(ord("C")))
    assert_equal(Int(_read_u8(buf, off + 1)), Int(ord("D")))
    off += 2

    assert_equal(Int(_read_u8(buf, off)), Int(OP_END))

    _free_buf(buf)


# ── Composite test sequence ──────────────────────────────────────────────────


fn test_composite_sequence() raises:
    """Write a realistic mutation sequence: load template, assign ID,
    create text node, replace placeholder, append children."""
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    var path = UnsafePointer[UInt8].alloc(2)
    path[0] = 0
    path[1] = 1

    # Realistic render sequence
    w.load_template(UInt32(0), UInt32(0), UInt32(10))
    w.assign_id(path, 2, UInt32(11))
    w.create_text_node(UInt32(12), String("Hello"))
    w.replace_placeholder(path, 2, UInt32(1))
    w.append_children(UInt32(0), UInt32(1))
    w.end()

    var off = 0

    # LoadTemplate
    assert_equal(Int(_read_u8(buf, off)), Int(OP_LOAD_TEMPLATE))
    assert_equal(Int(_read_u32_le(buf, off + 1)), 0, "tmpl_id 0")
    assert_equal(Int(_read_u32_le(buf, off + 5)), 0, "index 0")
    assert_equal(Int(_read_u32_le(buf, off + 9)), 10, "id 10")
    off += 13

    # AssignId
    assert_equal(Int(_read_u8(buf, off)), Int(OP_ASSIGN_ID))
    assert_equal(Int(_read_u8(buf, off + 1)), 2, "path_len 2")
    assert_equal(Int(_read_u8(buf, off + 2)), 0, "path[0] 0")
    assert_equal(Int(_read_u8(buf, off + 3)), 1, "path[1] 1")
    assert_equal(Int(_read_u32_le(buf, off + 4)), 11, "id 11")
    off += 8

    # CreateTextNode
    assert_equal(Int(_read_u8(buf, off)), Int(OP_CREATE_TEXT_NODE))
    assert_equal(Int(_read_u32_le(buf, off + 1)), 12, "text node id 12")
    assert_equal(Int(_read_u32_le(buf, off + 5)), 5, "text len 5 (Hello)")
    off += 1 + 4 + 4 + 5  # 14

    # ReplacePlaceholder
    assert_equal(Int(_read_u8(buf, off)), Int(OP_REPLACE_PLACEHOLDER))
    assert_equal(Int(_read_u8(buf, off + 1)), 2, "path_len 2")
    assert_equal(Int(_read_u32_le(buf, off + 4)), 1, "m 1")
    off += 1 + 1 + 2 + 4  # 8

    # AppendChildren
    assert_equal(Int(_read_u8(buf, off)), Int(OP_APPEND_CHILDREN))
    assert_equal(Int(_read_u32_le(buf, off + 1)), 0, "parent id 0")
    assert_equal(Int(_read_u32_le(buf, off + 5)), 1, "m 1")
    off += 9

    # End
    assert_equal(Int(_read_u8(buf, off)), Int(OP_END))

    path.free()
    _free_buf(buf)


# ── Only End sentinel in buffer ──────────────────────────────────────────────


fn test_only_end_sentinel() raises:
    """A buffer with just an End sentinel should have offset 1."""
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    w.end()

    assert_equal(w.offset, 1)
    assert_equal(Int(_read_u8(buf, 0)), Int(OP_END))
    # Everything after should be zero
    assert_equal(Int(_read_u8(buf, 1)), 0)
    assert_equal(Int(_read_u8(buf, 2)), 0)

    _free_buf(buf)


# ── Set text with empty string ───────────────────────────────────────────────


fn test_set_text_empty() raises:
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    w.set_text(UInt32(7), String(""))
    w.end()

    assert_equal(Int(_read_u8(buf, 0)), Int(OP_SET_TEXT))
    assert_equal(Int(_read_u32_le(buf, 1)), 7, "id is 7")
    assert_equal(Int(_read_u32_le(buf, 5)), 0, "text length is 0")
    assert_equal(
        Int(_read_u8(buf, 9)), Int(OP_END), "immediately followed by END"
    )

    _free_buf(buf)


# ── New event listener with longer name ──────────────────────────────────────


fn test_new_event_listener_long_name() raises:
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    w.new_event_listener(UInt32(5), String("mouseenter"))
    w.end()

    var off = 0
    assert_equal(Int(_read_u8(buf, off)), Int(OP_NEW_EVENT_LISTENER))
    off += 1

    assert_equal(Int(_read_u32_le(buf, off)), 5, "id is 5")
    off += 4

    assert_equal(Int(_read_u16_le(buf, off)), 10, "name length is 10")
    off += 2

    # Verify "mouseenter"
    assert_equal(Int(_read_u8(buf, off + 0)), Int(ord("m")))
    assert_equal(Int(_read_u8(buf, off + 1)), Int(ord("o")))
    assert_equal(Int(_read_u8(buf, off + 2)), Int(ord("u")))
    assert_equal(Int(_read_u8(buf, off + 3)), Int(ord("s")))
    assert_equal(Int(_read_u8(buf, off + 4)), Int(ord("e")))
    assert_equal(Int(_read_u8(buf, off + 5)), Int(ord("e")))
    assert_equal(Int(_read_u8(buf, off + 6)), Int(ord("n")))
    assert_equal(Int(_read_u8(buf, off + 7)), Int(ord("t")))
    assert_equal(Int(_read_u8(buf, off + 8)), Int(ord("e")))
    assert_equal(Int(_read_u8(buf, off + 9)), Int(ord("r")))
    off += 10

    assert_equal(Int(_read_u8(buf, off)), Int(OP_END))

    _free_buf(buf)


# ── Replace with multiple nodes ──────────────────────────────────────────────


fn test_replace_with_many() raises:
    """Test ReplaceWith with a large m value."""
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    w.replace_with(UInt32(100), UInt32(255))
    w.end()

    assert_equal(Int(_read_u8(buf, 0)), Int(OP_REPLACE_WITH))
    assert_equal(Int(_read_u32_le(buf, 1)), 100, "id is 100")
    assert_equal(Int(_read_u32_le(buf, 5)), 255, "m is 255")
    assert_equal(Int(_read_u8(buf, 9)), Int(OP_END))

    _free_buf(buf)


# ── Stress: many mutations in one buffer ─────────────────────────────────────


fn test_stress_many_push_roots() raises:
    """Write 100 push_root mutations and verify they all decode correctly."""
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    for i in range(100):
        w.push_root(UInt32(i))
    w.end()

    # Each push_root is 5 bytes, so total is 100 * 5 + 1 (end) = 501
    assert_equal(w.offset, 501, "100 push_roots + end = 501 bytes")

    # Verify each one
    var off = 0
    for i in range(100):
        assert_equal(
            Int(_read_u8(buf, off)),
            Int(OP_PUSH_ROOT),
            "opcode at mutation " + String(i),
        )
        assert_equal(
            Int(_read_u32_le(buf, off + 1)),
            i,
            "id at mutation " + String(i),
        )
        off += 5

    assert_equal(Int(_read_u8(buf, off)), Int(OP_END), "terminated with END")

    _free_buf(buf)


fn test_stress_many_removes() raises:
    """Write 50 remove mutations and verify they decode correctly."""
    var buf = _alloc_buf()
    var w = MutationWriter(buf, BUF_SIZE)

    for i in range(50):
        w.remove(UInt32(i + 1))
    w.end()

    # Each remove is 5 bytes, so total is 50 * 5 + 1 = 251
    assert_equal(w.offset, 251, "50 removes + end = 251 bytes")

    var off = 0
    for i in range(50):
        assert_equal(Int(_read_u8(buf, off)), Int(OP_REMOVE))
        assert_equal(Int(_read_u32_le(buf, off + 1)), i + 1)
        off += 5

    assert_equal(Int(_read_u8(buf, off)), Int(OP_END))

    _free_buf(buf)

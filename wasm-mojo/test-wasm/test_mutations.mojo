# CreateEngine and DiffEngine exercised through the real WASM binary via
# wasmtime-py (called from Mojo via Python interop).
#
# These tests verify that the create and diff engines work correctly when
# compiled to WASM and executed via the Wasmtime runtime.  Each test creates
# templates and VNodes via WASM exports, allocates a mutation buffer, runs
# the create/diff engines, then reads back mutation bytes to verify correctness.
#
# Run with:
#   mojo test test-wasm/test_mutations.mojo

from python import Python, PythonObject
from testing import assert_equal, assert_true, assert_false


fn _get_wasm() raises -> PythonObject:
    Python.add_to_path("test-wasm")
    var harness = Python.import_module("wasm_harness")
    return harness.get_instance()


# ── Constants ────────────────────────────────────────────────────────────────

alias OP_END = 0x00
alias OP_APPEND_CHILDREN = 0x01
alias OP_ASSIGN_ID = 0x02
alias OP_CREATE_PLACEHOLDER = 0x03
alias OP_CREATE_TEXT_NODE = 0x04
alias OP_LOAD_TEMPLATE = 0x05
alias OP_REPLACE_WITH = 0x06
alias OP_REPLACE_PLACEHOLDER = 0x07
alias OP_INSERT_AFTER = 0x08
alias OP_INSERT_BEFORE = 0x09
alias OP_SET_ATTRIBUTE = 0x0A
alias OP_SET_TEXT = 0x0B
alias OP_NEW_EVENT_LISTENER = 0x0C
alias OP_REMOVE_EVENT_LISTENER = 0x0D
alias OP_REMOVE = 0x0E
alias OP_PUSH_ROOT = 0x0F

alias TAG_DIV = 0
alias TAG_SPAN = 1
alias TAG_P = 2
alias TAG_H1 = 3
alias TAG_BUTTON = 12
alias TAG_LI = 11

alias BUF_CAP = 8192


# ── Helpers ──────────────────────────────────────────────────────────────────


fn _read_u8(w: PythonObject, buf: PythonObject, offset: Int) raises -> Int:
    return Int(w.debug_read_byte(buf, offset))


fn _read_u32_le(w: PythonObject, buf: PythonObject, offset: Int) raises -> Int:
    var b0 = _read_u8(w, buf, offset)
    var b1 = _read_u8(w, buf, offset + 1)
    var b2 = _read_u8(w, buf, offset + 2)
    var b3 = _read_u8(w, buf, offset + 3)
    return b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)


fn _read_u16_le(w: PythonObject, buf: PythonObject, offset: Int) raises -> Int:
    var lo = _read_u8(w, buf, offset)
    var hi = _read_u8(w, buf, offset + 1)
    return lo | (hi << 8)


struct MutationInfo(Copyable, Movable):
    var op: Int
    var id: Int
    var id2: Int
    var id3: Int
    var text_len: Int
    var name_len: Int
    var ns: Int
    var path_len: Int

    fn __init__(out self, op: Int):
        self.op = op
        self.id = 0
        self.id2 = 0
        self.id3 = 0
        self.text_len = 0
        self.name_len = 0
        self.ns = 0
        self.path_len = 0

    fn __copyinit__(out self, other: Self):
        self.op = other.op
        self.id = other.id
        self.id2 = other.id2
        self.id3 = other.id3
        self.text_len = other.text_len
        self.name_len = other.name_len
        self.ns = other.ns
        self.path_len = other.path_len

    fn __moveinit__(out self, owned other: Self):
        self.op = other.op
        self.id = other.id
        self.id2 = other.id2
        self.id3 = other.id3
        self.text_len = other.text_len
        self.name_len = other.name_len
        self.ns = other.ns
        self.path_len = other.path_len


fn _read_mutations(
    w: PythonObject, buf: PythonObject, length: Int
) raises -> List[MutationInfo]:
    """Decode all mutations from the WASM buffer up to the End sentinel."""
    var result = List[MutationInfo]()
    var pos = 0
    while pos < length:
        var op = _read_u8(w, buf, pos)
        pos += 1
        if op == OP_END:
            break
        var m = MutationInfo(op)

        if op == OP_CREATE_TEXT_NODE:
            m.id = _read_u32_le(w, buf, pos)
            pos += 4
            m.text_len = _read_u32_le(w, buf, pos)
            pos += 4
            pos += m.text_len

        elif op == OP_CREATE_PLACEHOLDER:
            m.id = _read_u32_le(w, buf, pos)
            pos += 4

        elif op == OP_LOAD_TEMPLATE:
            m.id = _read_u32_le(w, buf, pos)
            pos += 4
            m.id2 = _read_u32_le(w, buf, pos)
            pos += 4
            m.id3 = _read_u32_le(w, buf, pos)
            pos += 4

        elif op == OP_APPEND_CHILDREN:
            m.id = _read_u32_le(w, buf, pos)
            pos += 4
            m.id2 = _read_u32_le(w, buf, pos)
            pos += 4

        elif op == OP_ASSIGN_ID:
            m.path_len = _read_u8(w, buf, pos)
            pos += 1
            pos += m.path_len
            m.id = _read_u32_le(w, buf, pos)
            pos += 4

        elif op == OP_SET_TEXT:
            m.id = _read_u32_le(w, buf, pos)
            pos += 4
            m.text_len = _read_u32_le(w, buf, pos)
            pos += 4
            pos += m.text_len

        elif op == OP_SET_ATTRIBUTE:
            m.id = _read_u32_le(w, buf, pos)
            pos += 4
            m.ns = _read_u8(w, buf, pos)
            pos += 1
            m.name_len = _read_u16_le(w, buf, pos)
            pos += 2
            pos += m.name_len
            m.text_len = _read_u32_le(w, buf, pos)
            pos += 4
            pos += m.text_len

        elif op == OP_NEW_EVENT_LISTENER:
            m.id = _read_u32_le(w, buf, pos)
            pos += 4
            m.name_len = _read_u16_le(w, buf, pos)
            pos += 2
            pos += m.name_len

        elif op == OP_REMOVE_EVENT_LISTENER:
            m.id = _read_u32_le(w, buf, pos)
            pos += 4
            m.name_len = _read_u16_le(w, buf, pos)
            pos += 2
            pos += m.name_len

        elif op == OP_REMOVE:
            m.id = _read_u32_le(w, buf, pos)
            pos += 4

        elif op == OP_PUSH_ROOT:
            m.id = _read_u32_le(w, buf, pos)
            pos += 4

        elif op == OP_REPLACE_WITH:
            m.id = _read_u32_le(w, buf, pos)
            pos += 4
            m.id2 = _read_u32_le(w, buf, pos)
            pos += 4

        elif op == OP_REPLACE_PLACEHOLDER:
            m.path_len = _read_u8(w, buf, pos)
            pos += 1
            pos += m.path_len
            m.id = _read_u32_le(w, buf, pos)
            pos += 4

        elif op == OP_INSERT_AFTER:
            m.id = _read_u32_le(w, buf, pos)
            pos += 4
            m.id2 = _read_u32_le(w, buf, pos)
            pos += 4

        elif op == OP_INSERT_BEFORE:
            m.id = _read_u32_le(w, buf, pos)
            pos += 4
            m.id2 = _read_u32_le(w, buf, pos)
            pos += 4

        result.append(m^)
    return result^


fn _count_op(mutations: List[MutationInfo], op: Int) -> Int:
    var count = 0
    for i in range(len(mutations)):
        if mutations[i].op == op:
            count += 1
    return count


fn _find_first(mutations: List[MutationInfo], op: Int) -> Int:
    for i in range(len(mutations)):
        if mutations[i].op == op:
            return i
    return -1


# ── Test context: manages runtime, eid alloc, vnode store, writer, buffer ────


struct WasmTestContext(Movable):
    """Manages WASM resources for a create/diff engine test."""

    var w: PythonObject
    var rt: PythonObject
    var eid: PythonObject
    var store: PythonObject
    var buf: PythonObject
    var writer: PythonObject

    fn __init__(out self, w: PythonObject) raises:
        self.w = w
        self.rt = w.runtime_create()
        self.eid = w.eid_alloc_create()
        self.store = w.vnode_store_create()
        self.buf = w.mutation_buf_alloc(BUF_CAP)
        self.writer = w.writer_create(self.buf, BUF_CAP)

    fn __moveinit__(out self, owned other: Self):
        self.w = other.w
        self.rt = other.rt
        self.eid = other.eid
        self.store = other.store
        self.buf = other.buf
        self.writer = other.writer

    fn finalize_and_read(mut self) raises -> List[MutationInfo]:
        """Finalize the writer and read back all mutations."""
        var offset = Int(self.w.writer_finalize(self.writer))
        return _read_mutations(self.w, self.buf, offset)

    fn reset_writer(mut self) raises:
        """Reset the writer for a new mutation sequence."""
        self.w.writer_destroy(self.writer)
        # Zero out the buffer
        for i in range(BUF_CAP):
            _ = self.w.debug_write_byte(self.buf, i, 0)
        self.writer = self.w.writer_create(self.buf, BUF_CAP)

    fn destroy(mut self) raises:
        self.w.writer_destroy(self.writer)
        self.w.mutation_buf_free(self.buf)
        self.w.vnode_store_destroy(self.store)
        self.w.eid_alloc_destroy(self.eid)
        self.w.runtime_destroy(self.rt)


# ── Template registration helpers ────────────────────────────────────────────


fn _register_div_template(mut ctx: WasmTestContext, name: String) raises -> Int:
    """Register a simple <div></div> template, return ID."""
    var b = ctx.w.tmpl_builder_create(ctx.w.write_string_struct(name))
    _ = ctx.w.tmpl_builder_push_element(b, TAG_DIV, -1)
    var tmpl_id = Int(ctx.w.tmpl_builder_register(ctx.rt, b))
    ctx.w.tmpl_builder_destroy(b)
    return tmpl_id


fn _register_div_with_dyn_text(
    mut ctx: WasmTestContext, name: String
) raises -> Int:
    """Register <div>{dyntext_0}</div>, return ID."""
    var b = ctx.w.tmpl_builder_create(ctx.w.write_string_struct(name))
    var div_idx = Int(ctx.w.tmpl_builder_push_element(b, TAG_DIV, -1))
    _ = ctx.w.tmpl_builder_push_dynamic_text(b, 0, div_idx)
    var tmpl_id = Int(ctx.w.tmpl_builder_register(ctx.rt, b))
    ctx.w.tmpl_builder_destroy(b)
    return tmpl_id


fn _register_div_with_dyn_attr(
    mut ctx: WasmTestContext, name: String
) raises -> Int:
    """Register <div {dynattr_0}></div>, return ID."""
    var b = ctx.w.tmpl_builder_create(ctx.w.write_string_struct(name))
    var div_idx = Int(ctx.w.tmpl_builder_push_element(b, TAG_DIV, -1))
    ctx.w.tmpl_builder_push_dynamic_attr(b, div_idx, 0)
    var tmpl_id = Int(ctx.w.tmpl_builder_register(ctx.rt, b))
    ctx.w.tmpl_builder_destroy(b)
    return tmpl_id


fn _register_div_with_dyn_node(
    mut ctx: WasmTestContext, name: String
) raises -> Int:
    """Register <div>{dyn_node_0}</div>, return ID."""
    var b = ctx.w.tmpl_builder_create(ctx.w.write_string_struct(name))
    var div_idx = Int(ctx.w.tmpl_builder_push_element(b, TAG_DIV, -1))
    _ = ctx.w.tmpl_builder_push_dynamic(b, 0, div_idx)
    var tmpl_id = Int(ctx.w.tmpl_builder_register(ctx.rt, b))
    ctx.w.tmpl_builder_destroy(b)
    return tmpl_id


fn _register_complex_template(
    mut ctx: WasmTestContext, name: String
) raises -> Int:
    """Register a complex template:
    <div class="container">
      <h1>"Title"</h1>
      <p>{dyntext_0}</p>
      <button {dynattr_0}>{dyntext_1}</button>
    </div>
    """
    var b = ctx.w.tmpl_builder_create(ctx.w.write_string_struct(name))
    var div_idx = Int(ctx.w.tmpl_builder_push_element(b, TAG_DIV, -1))
    ctx.w.tmpl_builder_push_static_attr(
        b,
        div_idx,
        ctx.w.write_string_struct("class"),
        ctx.w.write_string_struct("container"),
    )

    var h1_idx = Int(ctx.w.tmpl_builder_push_element(b, TAG_H1, div_idx))
    _ = ctx.w.tmpl_builder_push_text(
        b, ctx.w.write_string_struct("Title"), h1_idx
    )

    var p_idx = Int(ctx.w.tmpl_builder_push_element(b, TAG_P, div_idx))
    _ = ctx.w.tmpl_builder_push_dynamic_text(b, 0, p_idx)

    var btn_idx = Int(ctx.w.tmpl_builder_push_element(b, TAG_BUTTON, div_idx))
    ctx.w.tmpl_builder_push_dynamic_attr(b, btn_idx, 0)
    _ = ctx.w.tmpl_builder_push_dynamic_text(b, 1, btn_idx)

    var tmpl_id = Int(ctx.w.tmpl_builder_register(ctx.rt, b))
    ctx.w.tmpl_builder_destroy(b)
    return tmpl_id


# ══════════════════════════════════════════════════════════════════════════════
# CREATE ENGINE TESTS
# ══════════════════════════════════════════════════════════════════════════════


fn test_create_text_vnode() raises:
    var w = _get_wasm()
    var ctx = WasmTestContext(w)

    var vn_idx = Int(
        w.vnode_push_text(ctx.store, w.write_string_struct("hello world"))
    )

    var num_roots = Int(
        w.create_vnode(ctx.writer, ctx.eid, ctx.rt, ctx.store, vn_idx)
    )
    assert_equal(num_roots, 1, "text vnode creates 1 root")

    var mutations = ctx.finalize_and_read()

    assert_equal(len(mutations), 1, "1 mutation emitted for text vnode")
    assert_equal(
        mutations[0].op,
        OP_CREATE_TEXT_NODE,
        "mutation is CreateTextNode",
    )
    assert_true(mutations[0].id > 0, "element id is non-zero")

    # Check mount state
    assert_equal(
        Int(w.vnode_is_mounted(ctx.store, vn_idx)), 1, "text vnode is mounted"
    )
    assert_equal(
        Int(w.vnode_root_id_count(ctx.store, vn_idx)),
        1,
        "text vnode has 1 root id",
    )
    assert_true(
        Int(w.vnode_get_root_id(ctx.store, vn_idx, 0)) > 0,
        "root id is non-zero",
    )

    ctx.destroy()


fn test_create_placeholder_vnode() raises:
    var w = _get_wasm()
    var ctx = WasmTestContext(w)

    var vn_idx = Int(w.vnode_push_placeholder(ctx.store, 0))

    var num_roots = Int(
        w.create_vnode(ctx.writer, ctx.eid, ctx.rt, ctx.store, vn_idx)
    )
    assert_equal(num_roots, 1, "placeholder creates 1 root")

    var mutations = ctx.finalize_and_read()

    assert_equal(len(mutations), 1, "1 mutation for placeholder")
    assert_equal(
        mutations[0].op,
        OP_CREATE_PLACEHOLDER,
        "mutation is CreatePlaceholder",
    )

    assert_equal(
        Int(w.vnode_is_mounted(ctx.store, vn_idx)),
        1,
        "placeholder is mounted",
    )
    assert_equal(
        Int(w.vnode_root_id_count(ctx.store, vn_idx)),
        1,
        "placeholder has 1 root id",
    )

    ctx.destroy()


fn test_create_simple_template_ref() raises:
    var w = _get_wasm()
    var ctx = WasmTestContext(w)

    var tmpl_id = _register_div_template(ctx, "simple-div-mut")

    var vn_idx = Int(w.vnode_push_template_ref(ctx.store, tmpl_id))

    var num_roots = Int(
        w.create_vnode(ctx.writer, ctx.eid, ctx.rt, ctx.store, vn_idx)
    )
    assert_equal(num_roots, 1, "template ref creates 1 root (div)")

    var mutations = ctx.finalize_and_read()

    # Should have at least a LoadTemplate mutation
    var load_count = _count_op(mutations, OP_LOAD_TEMPLATE)
    assert_equal(load_count, 1, "1 LoadTemplate mutation")

    var load_idx = _find_first(mutations, OP_LOAD_TEMPLATE)
    assert_true(load_idx >= 0, "LoadTemplate found")
    assert_equal(
        mutations[load_idx].id,
        tmpl_id,
        "LoadTemplate uses correct template ID",
    )

    # Check mount state
    assert_equal(
        Int(w.vnode_is_mounted(ctx.store, vn_idx)),
        1,
        "template ref is mounted",
    )
    assert_equal(
        Int(w.vnode_root_id_count(ctx.store, vn_idx)),
        1,
        "template ref has 1 root id",
    )

    ctx.destroy()


fn test_create_template_ref_with_dyn_text() raises:
    var w = _get_wasm()
    var ctx = WasmTestContext(w)

    var tmpl_id = _register_div_with_dyn_text(ctx, "dyn-text-div-mut")

    var vn_idx = Int(w.vnode_push_template_ref(ctx.store, tmpl_id))
    w.vnode_push_dynamic_text_node(
        ctx.store, vn_idx, w.write_string_struct("Count: 42")
    )

    var num_roots = Int(
        w.create_vnode(ctx.writer, ctx.eid, ctx.rt, ctx.store, vn_idx)
    )
    assert_equal(num_roots, 1, "template with dyntext creates 1 root")

    var mutations = ctx.finalize_and_read()

    # Should have LoadTemplate
    var load_count = _count_op(mutations, OP_LOAD_TEMPLATE)
    assert_equal(load_count, 1, "1 LoadTemplate")

    # Should have AssignId and/or SetText for the dynamic text
    var assign_count = _count_op(mutations, OP_ASSIGN_ID)
    var set_text_count = _count_op(mutations, OP_SET_TEXT)
    assert_true(
        assign_count > 0 or set_text_count > 0,
        "has AssignId or SetText for dynamic text",
    )

    # Check mount state: should have dynamic node IDs
    assert_true(
        Int(w.vnode_dyn_node_id_count(ctx.store, vn_idx)) > 0,
        "has dynamic node IDs",
    )

    ctx.destroy()


fn test_create_template_ref_with_dyn_attr() raises:
    var w = _get_wasm()
    var ctx = WasmTestContext(w)

    var tmpl_id = _register_div_with_dyn_attr(ctx, "dyn-attr-div-mut")

    var vn_idx = Int(w.vnode_push_template_ref(ctx.store, tmpl_id))
    w.vnode_push_dynamic_attr_text(
        ctx.store,
        vn_idx,
        w.write_string_struct("class"),
        w.write_string_struct("active"),
        0,
    )

    var num_roots = Int(
        w.create_vnode(ctx.writer, ctx.eid, ctx.rt, ctx.store, vn_idx)
    )
    assert_equal(num_roots, 1, "template with dynattr creates 1 root")

    var mutations = ctx.finalize_and_read()

    var load_count = _count_op(mutations, OP_LOAD_TEMPLATE)
    assert_equal(load_count, 1, "1 LoadTemplate")

    # Should have a SetAttribute mutation
    var set_attr_count = _count_op(mutations, OP_SET_ATTRIBUTE)
    assert_true(set_attr_count > 0, "has SetAttribute for dynamic attr")

    # Check mount state: should have dynamic attr IDs
    assert_true(
        Int(w.vnode_dyn_attr_id_count(ctx.store, vn_idx)) > 0,
        "has dynamic attr IDs",
    )

    ctx.destroy()


fn test_create_template_ref_with_event() raises:
    var w = _get_wasm()
    var ctx = WasmTestContext(w)

    var tmpl_id = _register_div_with_dyn_attr(ctx, "event-div-mut")

    var vn_idx = Int(w.vnode_push_template_ref(ctx.store, tmpl_id))
    w.vnode_push_dynamic_attr_event(
        ctx.store, vn_idx, w.write_string_struct("onclick"), 1, 0
    )

    _ = w.create_vnode(ctx.writer, ctx.eid, ctx.rt, ctx.store, vn_idx)

    var mutations = ctx.finalize_and_read()

    # Should have a NewEventListener mutation
    var has_listener = False
    for i in range(len(mutations)):
        if mutations[i].op == OP_NEW_EVENT_LISTENER:
            has_listener = True
    assert_true(has_listener, "has NewEventListener for event attr")

    ctx.destroy()


fn test_create_template_ref_with_dyn_text_node() raises:
    """Create a template ref with a Dynamic (full) node slot filled with text.
    """
    var w = _get_wasm()
    var ctx = WasmTestContext(w)

    var tmpl_id = _register_div_with_dyn_node(ctx, "dyn-node-div-mut")

    var vn_idx = Int(w.vnode_push_template_ref(ctx.store, tmpl_id))
    w.vnode_push_dynamic_text_node(
        ctx.store, vn_idx, w.write_string_struct("dynamic text")
    )

    var num_roots = Int(
        w.create_vnode(ctx.writer, ctx.eid, ctx.rt, ctx.store, vn_idx)
    )
    assert_equal(num_roots, 1, "creates 1 root")

    var mutations = ctx.finalize_and_read()

    # Should have CreateTextNode for the dynamic node
    var has_create_text = False
    for i in range(len(mutations)):
        if mutations[i].op == OP_CREATE_TEXT_NODE:
            has_create_text = True
    assert_true(has_create_text, "has CreateTextNode for dynamic node")

    # Should have ReplacePlaceholder
    var has_replace = False
    for i in range(len(mutations)):
        if mutations[i].op == OP_REPLACE_PLACEHOLDER:
            has_replace = True
    assert_true(has_replace, "has ReplacePlaceholder for dynamic node")

    ctx.destroy()


fn test_create_template_ref_with_dyn_placeholder() raises:
    """Create a template ref with a Dynamic node slot filled with placeholder.
    """
    var w = _get_wasm()
    var ctx = WasmTestContext(w)

    var tmpl_id = _register_div_with_dyn_node(ctx, "dyn-ph-div-mut")

    var vn_idx = Int(w.vnode_push_template_ref(ctx.store, tmpl_id))
    w.vnode_push_dynamic_placeholder(ctx.store, vn_idx)

    _ = w.create_vnode(ctx.writer, ctx.eid, ctx.rt, ctx.store, vn_idx)

    var mutations = ctx.finalize_and_read()

    # Should have CreatePlaceholder for the dynamic node
    var has_create_ph = False
    for i in range(len(mutations)):
        if mutations[i].op == OP_CREATE_PLACEHOLDER:
            has_create_ph = True
    assert_true(has_create_ph, "has CreatePlaceholder for dynamic placeholder")

    # Should have ReplacePlaceholder
    var has_replace = False
    for i in range(len(mutations)):
        if mutations[i].op == OP_REPLACE_PLACEHOLDER:
            has_replace = True
    assert_true(has_replace, "has ReplacePlaceholder")

    ctx.destroy()


fn test_create_fragment_vnode() raises:
    var w = _get_wasm()
    var ctx = WasmTestContext(w)

    # Create 3 text children
    var c1 = Int(w.vnode_push_text(ctx.store, w.write_string_struct("A")))
    var c2 = Int(w.vnode_push_text(ctx.store, w.write_string_struct("B")))
    var c3 = Int(w.vnode_push_text(ctx.store, w.write_string_struct("C")))

    # Create fragment
    var frag_idx = Int(w.vnode_push_fragment(ctx.store))
    w.vnode_push_fragment_child(ctx.store, frag_idx, c1)
    w.vnode_push_fragment_child(ctx.store, frag_idx, c2)
    w.vnode_push_fragment_child(ctx.store, frag_idx, c3)

    var num_roots = Int(
        w.create_vnode(ctx.writer, ctx.eid, ctx.rt, ctx.store, frag_idx)
    )
    assert_equal(num_roots, 3, "fragment creates 3 roots (one per child)")

    var mutations = ctx.finalize_and_read()

    # Should have 3 CreateTextNode mutations
    var create_text_count = _count_op(mutations, OP_CREATE_TEXT_NODE)
    assert_equal(create_text_count, 3, "3 CreateTextNode mutations")

    ctx.destroy()


fn test_create_element_id_uniqueness() raises:
    """Multiple create calls produce unique ElementIds."""
    var w = _get_wasm()
    var ctx = WasmTestContext(w)

    var tmpl_id = _register_div_template(ctx, "unique-div-mut")

    var vn1 = Int(w.vnode_push_template_ref(ctx.store, tmpl_id))
    var vn2 = Int(w.vnode_push_template_ref(ctx.store, tmpl_id))
    var vn3 = Int(w.vnode_push_template_ref(ctx.store, tmpl_id))

    _ = w.create_vnode(ctx.writer, ctx.eid, ctx.rt, ctx.store, vn1)
    _ = w.create_vnode(ctx.writer, ctx.eid, ctx.rt, ctx.store, vn2)
    _ = w.create_vnode(ctx.writer, ctx.eid, ctx.rt, ctx.store, vn3)

    var id1 = Int(w.vnode_get_root_id(ctx.store, vn1, 0))
    var id2 = Int(w.vnode_get_root_id(ctx.store, vn2, 0))
    var id3 = Int(w.vnode_get_root_id(ctx.store, vn3, 0))

    assert_true(id1 != id2, "id1 != id2")
    assert_true(id2 != id3, "id2 != id3")
    assert_true(id1 != id3, "id1 != id3")

    ctx.destroy()


fn test_create_empty_fragment() raises:
    """Empty fragment creates 0 roots."""
    var w = _get_wasm()
    var ctx = WasmTestContext(w)

    var frag_idx = Int(w.vnode_push_fragment(ctx.store))

    var num_roots = Int(
        w.create_vnode(ctx.writer, ctx.eid, ctx.rt, ctx.store, frag_idx)
    )
    assert_equal(num_roots, 0, "empty fragment creates 0 roots")

    var mutations = ctx.finalize_and_read()
    assert_equal(len(mutations), 0, "empty fragment produces 0 mutations")

    ctx.destroy()


fn test_create_complex_template_multi_slots() raises:
    """Create a complex template with multiple dynamic slots."""
    var w = _get_wasm()
    var ctx = WasmTestContext(w)

    var tmpl_id = _register_complex_template(ctx, "complex-mut")

    var vn_idx = Int(w.vnode_push_template_ref(ctx.store, tmpl_id))
    # dyntext_0 -> "Description"
    w.vnode_push_dynamic_text_node(
        ctx.store, vn_idx, w.write_string_struct("Description")
    )
    # dyntext_1 -> "Click me"
    w.vnode_push_dynamic_text_node(
        ctx.store, vn_idx, w.write_string_struct("Click me")
    )
    # dynattr_0 -> onclick event
    w.vnode_push_dynamic_attr_event(
        ctx.store, vn_idx, w.write_string_struct("onclick"), 42, 0
    )

    var num_roots = Int(
        w.create_vnode(ctx.writer, ctx.eid, ctx.rt, ctx.store, vn_idx)
    )
    assert_equal(num_roots, 1, "complex template creates 1 root")

    var mutations = ctx.finalize_and_read()

    # Should have LoadTemplate
    var load_count = _count_op(mutations, OP_LOAD_TEMPLATE)
    assert_equal(load_count, 1, "1 LoadTemplate")

    # Should have AssignId mutations for dynamic slots
    var assign_count = _count_op(mutations, OP_ASSIGN_ID)
    assert_true(assign_count > 0, "has AssignId for dynamic slots")

    # Should have SetText mutations for dynamic text
    var set_text_count = _count_op(mutations, OP_SET_TEXT)
    assert_true(set_text_count > 0, "has SetText for dynamic text")

    # Should have NewEventListener for the onclick
    var listener_count = _count_op(mutations, OP_NEW_EVENT_LISTENER)
    assert_true(listener_count > 0, "has NewEventListener for onclick")

    ctx.destroy()


# ══════════════════════════════════════════════════════════════════════════════
# DIFF ENGINE TESTS
# ══════════════════════════════════════════════════════════════════════════════


fn test_diff_same_text_zero_mutations() raises:
    """Diffing two Text VNodes with the same text -> 0 mutations."""
    var w = _get_wasm()
    var ctx = WasmTestContext(w)

    # Create old text vnode and mount it
    var old_idx = Int(
        w.vnode_push_text(ctx.store, w.write_string_struct("hello"))
    )
    _ = w.create_vnode(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_idx)

    # Reset writer for diff
    ctx.reset_writer()

    # Create new text vnode with same text
    var new_idx = Int(
        w.vnode_push_text(ctx.store, w.write_string_struct("hello"))
    )

    _ = w.diff_vnodes(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_idx, new_idx)

    var mutations = ctx.finalize_and_read()
    assert_equal(len(mutations), 0, "same text produces 0 mutations")

    ctx.destroy()


fn test_diff_text_changed_produces_set_text() raises:
    """Diffing two Text VNodes with different text -> SetText."""
    var w = _get_wasm()
    var ctx = WasmTestContext(w)

    # Create old text vnode and mount it
    var old_idx = Int(
        w.vnode_push_text(ctx.store, w.write_string_struct("hello"))
    )
    _ = w.create_vnode(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_idx)

    # Remember the old root id
    var old_root_id = Int(w.vnode_get_root_id(ctx.store, old_idx, 0))

    # Reset writer for diff
    ctx.reset_writer()

    # Create new text vnode with different text
    var new_idx = Int(
        w.vnode_push_text(ctx.store, w.write_string_struct("world"))
    )

    _ = w.diff_vnodes(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_idx, new_idx)

    var mutations = ctx.finalize_and_read()
    assert_true(len(mutations) > 0, "text change produces mutations")

    # Should have a SetText mutation targeting the old root id
    var has_set_text = False
    for i in range(len(mutations)):
        if mutations[i].op == OP_SET_TEXT:
            has_set_text = True
            assert_equal(
                mutations[i].id,
                old_root_id,
                "SetText targets old root id",
            )
    assert_true(has_set_text, "SetText emitted")

    ctx.destroy()


fn test_diff_text_empty_to_content() raises:
    """Diffing '' -> 'hello' produces SetText."""
    var w = _get_wasm()
    var ctx = WasmTestContext(w)

    var old_idx = Int(w.vnode_push_text(ctx.store, w.write_string_struct("")))
    _ = w.create_vnode(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_idx)
    ctx.reset_writer()

    var new_idx = Int(
        w.vnode_push_text(ctx.store, w.write_string_struct("hello"))
    )
    _ = w.diff_vnodes(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_idx, new_idx)

    var mutations = ctx.finalize_and_read()

    var set_text_count = _count_op(mutations, OP_SET_TEXT)
    assert_equal(set_text_count, 1, "1 SetText for '' -> 'hello'")

    ctx.destroy()


fn test_diff_placeholder_to_placeholder_zero_mutations() raises:
    """Diffing two Placeholders -> 0 mutations."""
    var w = _get_wasm()
    var ctx = WasmTestContext(w)

    var old_idx = Int(w.vnode_push_placeholder(ctx.store, 0))
    _ = w.create_vnode(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_idx)
    ctx.reset_writer()

    var new_idx = Int(w.vnode_push_placeholder(ctx.store, 0))
    _ = w.diff_vnodes(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_idx, new_idx)

    var mutations = ctx.finalize_and_read()
    assert_equal(
        len(mutations), 0, "placeholder -> placeholder produces 0 mutations"
    )

    ctx.destroy()


fn test_diff_same_template_same_dyn_values_zero_mutations() raises:
    """Same template, same dynamic text -> 0 mutations."""
    var w = _get_wasm()
    var ctx = WasmTestContext(w)

    var tmpl_id = _register_div_with_dyn_text(ctx, "same-dyn-mut")

    # Old VNode
    var old_idx = Int(w.vnode_push_template_ref(ctx.store, tmpl_id))
    w.vnode_push_dynamic_text_node(
        ctx.store, old_idx, w.write_string_struct("Count: 5")
    )

    _ = w.create_vnode(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_idx)
    ctx.reset_writer()

    # New VNode with same dynamic text
    var new_idx = Int(w.vnode_push_template_ref(ctx.store, tmpl_id))
    w.vnode_push_dynamic_text_node(
        ctx.store, new_idx, w.write_string_struct("Count: 5")
    )

    _ = w.diff_vnodes(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_idx, new_idx)

    var mutations = ctx.finalize_and_read()
    assert_equal(
        len(mutations),
        0,
        "same template + same dyntext produces 0 mutations",
    )

    ctx.destroy()


fn test_diff_same_template_dyn_text_changed() raises:
    """Same template, dynamic text changed -> SetText."""
    var w = _get_wasm()
    var ctx = WasmTestContext(w)

    var tmpl_id = _register_div_with_dyn_text(ctx, "changed-dyn-mut")

    # Old VNode
    var old_idx = Int(w.vnode_push_template_ref(ctx.store, tmpl_id))
    w.vnode_push_dynamic_text_node(
        ctx.store, old_idx, w.write_string_struct("Count: 5")
    )

    _ = w.create_vnode(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_idx)

    # Get the dynamic node ID assigned during create
    var old_dyn_id = Int(w.vnode_get_dyn_node_id(ctx.store, old_idx, 0))

    ctx.reset_writer()

    # New VNode with different dynamic text
    var new_idx = Int(w.vnode_push_template_ref(ctx.store, tmpl_id))
    w.vnode_push_dynamic_text_node(
        ctx.store, new_idx, w.write_string_struct("Count: 10")
    )

    _ = w.diff_vnodes(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_idx, new_idx)

    var mutations = ctx.finalize_and_read()

    # Should have a SetText targeting the dynamic node's element ID
    var has_set_text = False
    for i in range(len(mutations)):
        if mutations[i].op == OP_SET_TEXT:
            has_set_text = True
            assert_equal(
                mutations[i].id,
                old_dyn_id,
                "SetText targets dynamic node element",
            )
    assert_true(has_set_text, "SetText emitted for changed dynamic text")

    ctx.destroy()


fn test_diff_same_template_attr_changed() raises:
    """Same template, dynamic attribute changed -> SetAttribute."""
    var w = _get_wasm()
    var ctx = WasmTestContext(w)

    var tmpl_id = _register_div_with_dyn_attr(ctx, "attr-changed-mut")

    # Old VNode
    var old_idx = Int(w.vnode_push_template_ref(ctx.store, tmpl_id))
    w.vnode_push_dynamic_attr_text(
        ctx.store,
        old_idx,
        w.write_string_struct("class"),
        w.write_string_struct("old-class"),
        0,
    )

    _ = w.create_vnode(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_idx)
    ctx.reset_writer()

    # New VNode with different attr value
    var new_idx = Int(w.vnode_push_template_ref(ctx.store, tmpl_id))
    w.vnode_push_dynamic_attr_text(
        ctx.store,
        new_idx,
        w.write_string_struct("class"),
        w.write_string_struct("new-class"),
        0,
    )

    _ = w.diff_vnodes(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_idx, new_idx)

    var mutations = ctx.finalize_and_read()

    var set_attr_count = _count_op(mutations, OP_SET_ATTRIBUTE)
    assert_true(set_attr_count > 0, "SetAttribute emitted for changed attr")

    ctx.destroy()


fn test_diff_same_template_attr_unchanged_zero_mutations() raises:
    """Same template, same attribute value -> 0 mutations."""
    var w = _get_wasm()
    var ctx = WasmTestContext(w)

    var tmpl_id = _register_div_with_dyn_attr(ctx, "attr-same-mut")

    # Old VNode
    var old_idx = Int(w.vnode_push_template_ref(ctx.store, tmpl_id))
    w.vnode_push_dynamic_attr_text(
        ctx.store,
        old_idx,
        w.write_string_struct("class"),
        w.write_string_struct("same"),
        0,
    )

    _ = w.create_vnode(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_idx)
    ctx.reset_writer()

    # New VNode with same attr value
    var new_idx = Int(w.vnode_push_template_ref(ctx.store, tmpl_id))
    w.vnode_push_dynamic_attr_text(
        ctx.store,
        new_idx,
        w.write_string_struct("class"),
        w.write_string_struct("same"),
        0,
    )

    _ = w.diff_vnodes(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_idx, new_idx)

    var mutations = ctx.finalize_and_read()
    assert_equal(len(mutations), 0, "same attr value produces 0 mutations")

    ctx.destroy()


fn test_diff_bool_attr_changed() raises:
    """Bool attribute changed -> SetAttribute."""
    var w = _get_wasm()
    var ctx = WasmTestContext(w)

    var tmpl_id = _register_div_with_dyn_attr(ctx, "bool-attr-mut")

    var old_idx = Int(w.vnode_push_template_ref(ctx.store, tmpl_id))
    w.vnode_push_dynamic_attr_bool(
        ctx.store, old_idx, w.write_string_struct("disabled"), 0, 0
    )

    _ = w.create_vnode(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_idx)
    ctx.reset_writer()

    var new_idx = Int(w.vnode_push_template_ref(ctx.store, tmpl_id))
    w.vnode_push_dynamic_attr_bool(
        ctx.store, new_idx, w.write_string_struct("disabled"), 1, 0
    )

    _ = w.diff_vnodes(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_idx, new_idx)

    var mutations = ctx.finalize_and_read()

    var set_attr_count = _count_op(mutations, OP_SET_ATTRIBUTE)
    assert_true(set_attr_count > 0, "SetAttribute emitted for bool change")

    ctx.destroy()


fn test_diff_text_to_placeholder_replacement() raises:
    """Text -> Placeholder (different kind) -> replacement."""
    var w = _get_wasm()
    var ctx = WasmTestContext(w)

    var old_idx = Int(
        w.vnode_push_text(ctx.store, w.write_string_struct("hello"))
    )
    _ = w.create_vnode(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_idx)
    ctx.reset_writer()

    var new_idx = Int(w.vnode_push_placeholder(ctx.store, 0))
    _ = w.diff_vnodes(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_idx, new_idx)

    var mutations = ctx.finalize_and_read()

    # Should have a create and a replace
    var has_create = (
        _count_op(mutations, OP_CREATE_PLACEHOLDER) > 0
        or _count_op(mutations, OP_CREATE_TEXT_NODE) > 0
    )
    var has_replace = _count_op(mutations, OP_REPLACE_WITH) > 0
    assert_true(has_create, "has create for replacement")
    assert_true(has_replace, "has ReplaceWith for kind change")

    ctx.destroy()


fn test_diff_different_templates_replacement() raises:
    """Different templates -> full replacement."""
    var w = _get_wasm()
    var ctx = WasmTestContext(w)

    var tmpl_a = _register_div_template(ctx, "tmpl-a-mut")
    var tmpl_b = _register_div_template(ctx, "tmpl-b-mut")

    var old_idx = Int(w.vnode_push_template_ref(ctx.store, tmpl_a))
    _ = w.create_vnode(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_idx)
    ctx.reset_writer()

    var new_idx = Int(w.vnode_push_template_ref(ctx.store, tmpl_b))

    _ = w.diff_vnodes(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_idx, new_idx)

    var mutations = ctx.finalize_and_read()

    # Should have LoadTemplate for the new template + ReplaceWith
    var has_load = _count_op(mutations, OP_LOAD_TEMPLATE) > 0
    var has_replace = _count_op(mutations, OP_REPLACE_WITH) > 0
    assert_true(has_load, "LoadTemplate for new template in replacement")
    assert_true(has_replace, "ReplaceWith for different templates")

    ctx.destroy()


fn test_diff_fragment_children_text_changed() raises:
    """Fragment diff: child text changed."""
    var w = _get_wasm()
    var ctx = WasmTestContext(w)

    # Old fragment: [A, B]
    var oa = Int(w.vnode_push_text(ctx.store, w.write_string_struct("A")))
    var ob = Int(w.vnode_push_text(ctx.store, w.write_string_struct("B")))
    var old_frag_idx = Int(w.vnode_push_fragment(ctx.store))
    w.vnode_push_fragment_child(ctx.store, old_frag_idx, oa)
    w.vnode_push_fragment_child(ctx.store, old_frag_idx, ob)

    _ = w.create_vnode(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_frag_idx)
    ctx.reset_writer()

    # New fragment: [A, C] (B -> C)
    var na = Int(w.vnode_push_text(ctx.store, w.write_string_struct("A")))
    var nc = Int(w.vnode_push_text(ctx.store, w.write_string_struct("C")))
    var new_frag_idx = Int(w.vnode_push_fragment(ctx.store))
    w.vnode_push_fragment_child(ctx.store, new_frag_idx, na)
    w.vnode_push_fragment_child(ctx.store, new_frag_idx, nc)

    _ = w.diff_vnodes(
        ctx.writer, ctx.eid, ctx.rt, ctx.store, old_frag_idx, new_frag_idx
    )

    var mutations = ctx.finalize_and_read()

    # "A" same -> no mutation, "B" -> "C" -> SetText
    var set_text_count = _count_op(mutations, OP_SET_TEXT)
    assert_equal(set_text_count, 1, "1 SetText for B -> C")

    ctx.destroy()


fn test_diff_fragment_children_removed() raises:
    """Fragment diff: children removed."""
    var w = _get_wasm()
    var ctx = WasmTestContext(w)

    # Old fragment: [A, B, C]
    var oa = Int(w.vnode_push_text(ctx.store, w.write_string_struct("A")))
    var ob = Int(w.vnode_push_text(ctx.store, w.write_string_struct("B")))
    var oc = Int(w.vnode_push_text(ctx.store, w.write_string_struct("C")))
    var old_frag_idx = Int(w.vnode_push_fragment(ctx.store))
    w.vnode_push_fragment_child(ctx.store, old_frag_idx, oa)
    w.vnode_push_fragment_child(ctx.store, old_frag_idx, ob)
    w.vnode_push_fragment_child(ctx.store, old_frag_idx, oc)

    _ = w.create_vnode(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_frag_idx)
    ctx.reset_writer()

    # New fragment: [A] (B, C removed)
    var na = Int(w.vnode_push_text(ctx.store, w.write_string_struct("A")))
    var new_frag_idx = Int(w.vnode_push_fragment(ctx.store))
    w.vnode_push_fragment_child(ctx.store, new_frag_idx, na)

    _ = w.diff_vnodes(
        ctx.writer, ctx.eid, ctx.rt, ctx.store, old_frag_idx, new_frag_idx
    )

    var mutations = ctx.finalize_and_read()

    # Should have Remove mutations for the extra children
    var remove_count = _count_op(mutations, OP_REMOVE)
    assert_true(remove_count >= 2, "at least 2 Remove mutations for B and C")

    ctx.destroy()


fn test_diff_fragment_children_added() raises:
    """Fragment diff: children added."""
    var w = _get_wasm()
    var ctx = WasmTestContext(w)

    # Old fragment: [A]
    var oa = Int(w.vnode_push_text(ctx.store, w.write_string_struct("A")))
    var old_frag_idx = Int(w.vnode_push_fragment(ctx.store))
    w.vnode_push_fragment_child(ctx.store, old_frag_idx, oa)

    _ = w.create_vnode(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_frag_idx)
    ctx.reset_writer()

    # New fragment: [A, B, C]
    var na = Int(w.vnode_push_text(ctx.store, w.write_string_struct("A")))
    var nb = Int(w.vnode_push_text(ctx.store, w.write_string_struct("B")))
    var nc = Int(w.vnode_push_text(ctx.store, w.write_string_struct("C")))
    var new_frag_idx = Int(w.vnode_push_fragment(ctx.store))
    w.vnode_push_fragment_child(ctx.store, new_frag_idx, na)
    w.vnode_push_fragment_child(ctx.store, new_frag_idx, nb)
    w.vnode_push_fragment_child(ctx.store, new_frag_idx, nc)

    _ = w.diff_vnodes(
        ctx.writer, ctx.eid, ctx.rt, ctx.store, old_frag_idx, new_frag_idx
    )

    var mutations = ctx.finalize_and_read()

    # Should have CreateTextNode for B and C, and InsertAfter
    var create_count = _count_op(mutations, OP_CREATE_TEXT_NODE)
    assert_true(create_count >= 2, "at least 2 CreateTextNode for B and C")

    var has_insert = _count_op(mutations, OP_INSERT_AFTER) > 0
    assert_true(has_insert, "has InsertAfter for added children")

    ctx.destroy()


fn test_diff_event_listener_changed() raises:
    """Event listener handler changed -> RemoveEventListener + NewEventListener.
    """
    var w = _get_wasm()
    var ctx = WasmTestContext(w)

    var tmpl_id = _register_div_with_dyn_attr(ctx, "event-change-mut")

    var old_idx = Int(w.vnode_push_template_ref(ctx.store, tmpl_id))
    w.vnode_push_dynamic_attr_event(
        ctx.store, old_idx, w.write_string_struct("onclick"), 1, 0
    )

    _ = w.create_vnode(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_idx)
    ctx.reset_writer()

    var new_idx = Int(w.vnode_push_template_ref(ctx.store, tmpl_id))
    w.vnode_push_dynamic_attr_event(
        ctx.store, new_idx, w.write_string_struct("onclick"), 2, 0
    )

    _ = w.diff_vnodes(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_idx, new_idx)

    var mutations = ctx.finalize_and_read()

    var has_remove = _count_op(mutations, OP_REMOVE_EVENT_LISTENER) > 0
    var has_new = _count_op(mutations, OP_NEW_EVENT_LISTENER) > 0
    assert_true(has_remove, "RemoveEventListener for old handler")
    assert_true(has_new, "NewEventListener for new handler")

    ctx.destroy()


fn test_diff_same_event_listener_zero_mutations() raises:
    """Same event listener -> 0 mutations."""
    var w = _get_wasm()
    var ctx = WasmTestContext(w)

    var tmpl_id = _register_div_with_dyn_attr(ctx, "event-same-mut")

    var old_idx = Int(w.vnode_push_template_ref(ctx.store, tmpl_id))
    w.vnode_push_dynamic_attr_event(
        ctx.store, old_idx, w.write_string_struct("onclick"), 1, 0
    )

    _ = w.create_vnode(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_idx)
    ctx.reset_writer()

    var new_idx = Int(w.vnode_push_template_ref(ctx.store, tmpl_id))
    w.vnode_push_dynamic_attr_event(
        ctx.store, new_idx, w.write_string_struct("onclick"), 1, 0
    )

    _ = w.diff_vnodes(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_idx, new_idx)

    var mutations = ctx.finalize_and_read()
    assert_equal(len(mutations), 0, "same event listener produces 0 mutations")

    ctx.destroy()


fn test_diff_attr_type_changed_text_to_bool() raises:
    """Attribute type changed (text -> bool) -> SetAttribute."""
    var w = _get_wasm()
    var ctx = WasmTestContext(w)

    var tmpl_id = _register_div_with_dyn_attr(ctx, "type-change-mut")

    var old_idx = Int(w.vnode_push_template_ref(ctx.store, tmpl_id))
    w.vnode_push_dynamic_attr_text(
        ctx.store,
        old_idx,
        w.write_string_struct("disabled"),
        w.write_string_struct("yes"),
        0,
    )

    _ = w.create_vnode(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_idx)
    ctx.reset_writer()

    var new_idx = Int(w.vnode_push_template_ref(ctx.store, tmpl_id))
    w.vnode_push_dynamic_attr_bool(
        ctx.store, new_idx, w.write_string_struct("disabled"), 1, 0
    )

    _ = w.diff_vnodes(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_idx, new_idx)

    var mutations = ctx.finalize_and_read()

    var set_attr_count = _count_op(mutations, OP_SET_ATTRIBUTE)
    assert_true(set_attr_count > 0, "SetAttribute for type change")

    ctx.destroy()


fn test_diff_attr_removed_text_to_none() raises:
    """Attribute removed (text -> none) -> SetAttribute with empty value."""
    var w = _get_wasm()
    var ctx = WasmTestContext(w)

    var tmpl_id = _register_div_with_dyn_attr(ctx, "attr-remove-mut")

    var old_idx = Int(w.vnode_push_template_ref(ctx.store, tmpl_id))
    w.vnode_push_dynamic_attr_text(
        ctx.store,
        old_idx,
        w.write_string_struct("class"),
        w.write_string_struct("active"),
        0,
    )

    _ = w.create_vnode(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_idx)
    ctx.reset_writer()

    var new_idx = Int(w.vnode_push_template_ref(ctx.store, tmpl_id))
    w.vnode_push_dynamic_attr_none(
        ctx.store, new_idx, w.write_string_struct("class"), 0
    )

    _ = w.diff_vnodes(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_idx, new_idx)

    var mutations = ctx.finalize_and_read()

    var set_attr_count = _count_op(mutations, OP_SET_ATTRIBUTE)
    assert_true(set_attr_count > 0, "SetAttribute for attr removal")

    ctx.destroy()


fn test_diff_int_attr_changed() raises:
    """Integer attribute value changed -> SetAttribute."""
    var w = _get_wasm()
    var ctx = WasmTestContext(w)

    var tmpl_id = _register_div_with_dyn_attr(ctx, "int-attr-mut")

    var old_idx = Int(w.vnode_push_template_ref(ctx.store, tmpl_id))
    w.vnode_push_dynamic_attr_int(
        ctx.store, old_idx, w.write_string_struct("tabindex"), 1, 0
    )

    _ = w.create_vnode(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_idx)
    ctx.reset_writer()

    var new_idx = Int(w.vnode_push_template_ref(ctx.store, tmpl_id))
    w.vnode_push_dynamic_attr_int(
        ctx.store, new_idx, w.write_string_struct("tabindex"), 5, 0
    )

    _ = w.diff_vnodes(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_idx, new_idx)

    var mutations = ctx.finalize_and_read()

    var set_attr_count = _count_op(mutations, OP_SET_ATTRIBUTE)
    assert_true(set_attr_count > 0, "SetAttribute for int change")

    ctx.destroy()


fn test_diff_mount_state_transfer_preserves_ids() raises:
    """Diff transfers mount state: ElementIds on new VNode match old."""
    var w = _get_wasm()
    var ctx = WasmTestContext(w)

    var tmpl_id = _register_div_with_dyn_text(ctx, "transfer-test-mut")

    # Old VNode
    var old_idx = Int(w.vnode_push_template_ref(ctx.store, tmpl_id))
    w.vnode_push_dynamic_text_node(
        ctx.store, old_idx, w.write_string_struct("old")
    )

    _ = w.create_vnode(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_idx)

    var old_root_id = Int(w.vnode_get_root_id(ctx.store, old_idx, 0))
    var old_dyn_id = Int(w.vnode_get_dyn_node_id(ctx.store, old_idx, 0))

    ctx.reset_writer()

    # New VNode
    var new_idx = Int(w.vnode_push_template_ref(ctx.store, tmpl_id))
    w.vnode_push_dynamic_text_node(
        ctx.store, new_idx, w.write_string_struct("new")
    )

    _ = w.diff_vnodes(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_idx, new_idx)

    # Check that new VNode got the same ElementIds
    var new_root_id = Int(w.vnode_get_root_id(ctx.store, new_idx, 0))
    var new_dyn_id = Int(w.vnode_get_dyn_node_id(ctx.store, new_idx, 0))

    assert_equal(new_root_id, old_root_id, "root ID transferred to new VNode")
    assert_equal(
        new_dyn_id, old_dyn_id, "dynamic node ID transferred to new VNode"
    )

    ctx.destroy()


fn test_diff_sequential_diffs_state_chain() raises:
    """Sequential diffs (state chain): v0 -> v1 -> v2 -> v3."""
    var w = _get_wasm()
    var ctx = WasmTestContext(w)

    var tmpl_id = _register_div_with_dyn_text(ctx, "chain-test-mut")

    # v0: initial
    var v0_idx = Int(w.vnode_push_template_ref(ctx.store, tmpl_id))
    w.vnode_push_dynamic_text_node(
        ctx.store, v0_idx, w.write_string_struct("state-0")
    )

    _ = w.create_vnode(ctx.writer, ctx.eid, ctx.rt, ctx.store, v0_idx)
    ctx.reset_writer()

    # v0 -> v1
    var v1_idx = Int(w.vnode_push_template_ref(ctx.store, tmpl_id))
    w.vnode_push_dynamic_text_node(
        ctx.store, v1_idx, w.write_string_struct("state-1")
    )

    _ = w.diff_vnodes(ctx.writer, ctx.eid, ctx.rt, ctx.store, v0_idx, v1_idx)

    var muts1 = ctx.finalize_and_read()
    var st1 = _count_op(muts1, OP_SET_TEXT)
    assert_equal(st1, 1, "v0 -> v1: 1 SetText")

    ctx.reset_writer()

    # v1 -> v2
    var v2_idx = Int(w.vnode_push_template_ref(ctx.store, tmpl_id))
    w.vnode_push_dynamic_text_node(
        ctx.store, v2_idx, w.write_string_struct("state-2")
    )

    _ = w.diff_vnodes(ctx.writer, ctx.eid, ctx.rt, ctx.store, v1_idx, v2_idx)

    var muts2 = ctx.finalize_and_read()
    var st2 = _count_op(muts2, OP_SET_TEXT)
    assert_equal(st2, 1, "v1 -> v2: 1 SetText")

    ctx.reset_writer()

    # v2 -> v3 (same text -> 0)
    var v3_idx = Int(w.vnode_push_template_ref(ctx.store, tmpl_id))
    w.vnode_push_dynamic_text_node(
        ctx.store, v3_idx, w.write_string_struct("state-2")
    )

    _ = w.diff_vnodes(ctx.writer, ctx.eid, ctx.rt, ctx.store, v2_idx, v3_idx)

    var muts3 = ctx.finalize_and_read()
    assert_equal(len(muts3), 0, "v2 -> v3 same text: 0 mutations")

    ctx.destroy()


fn test_diff_dyn_node_text_to_placeholder() raises:
    """Dynamic node: text -> placeholder -> replacement."""
    var w = _get_wasm()
    var ctx = WasmTestContext(w)

    var tmpl_id = _register_div_with_dyn_node(ctx, "dyn-text-to-ph-mut")

    var old_idx = Int(w.vnode_push_template_ref(ctx.store, tmpl_id))
    w.vnode_push_dynamic_text_node(
        ctx.store, old_idx, w.write_string_struct("some text")
    )

    _ = w.create_vnode(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_idx)
    ctx.reset_writer()

    var new_idx = Int(w.vnode_push_template_ref(ctx.store, tmpl_id))
    w.vnode_push_dynamic_placeholder(ctx.store, new_idx)

    _ = w.diff_vnodes(ctx.writer, ctx.eid, ctx.rt, ctx.store, old_idx, new_idx)

    var mutations = ctx.finalize_and_read()

    # Should have CreatePlaceholder and ReplaceWith
    var has_create_ph = _count_op(mutations, OP_CREATE_PLACEHOLDER) > 0
    var has_replace = _count_op(mutations, OP_REPLACE_WITH) > 0
    assert_true(has_create_ph, "CreatePlaceholder for text -> placeholder")
    assert_true(has_replace, "ReplaceWith for text -> placeholder")

    ctx.destroy()

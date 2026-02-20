# Tests for CreateEngine and DiffEngine — native Mojo tests run with `mojo test`.
#
# These tests are a direct port of test/mutations.test.ts, exercising
# the same scenarios without the WASM/JS round-trip.  Each test sets up
# a Runtime, ElementIdAllocator, VNodeStore, and MutationWriter, then
# runs the create/diff engines and reads back the mutation buffer to
# verify correctness.
#
# Run with:
#   mojo test -I src test-mojo/test_mutations.mojo

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
from arena import ElementId, ElementIdAllocator
from signals import Runtime, create_runtime, destroy_runtime
from mutations import CreateEngine, DiffEngine
from vdom import (
    TemplateBuilder,
    TemplateRegistry,
    VNode,
    VNodeStore,
    DynamicNode,
    DynamicAttr,
    AttributeValue,
    TNODE_ELEMENT,
    TNODE_TEXT,
    TNODE_DYNAMIC,
    TNODE_DYNAMIC_TEXT,
    TATTR_STATIC,
    TATTR_DYNAMIC,
    VNODE_TEMPLATE_REF,
    VNODE_TEXT,
    VNODE_PLACEHOLDER,
    VNODE_FRAGMENT,
    AVAL_TEXT,
    AVAL_INT,
    AVAL_FLOAT,
    AVAL_BOOL,
    AVAL_EVENT,
    AVAL_NONE,
    DNODE_TEXT,
    DNODE_PLACEHOLDER,
    TAG_DIV,
    TAG_SPAN,
    TAG_P,
    TAG_H1,
    TAG_H2,
    TAG_BUTTON,
    TAG_LI,
)


# ── Buffer helpers ───────────────────────────────────────────────────────────

alias BUF_SIZE = 8192


fn _alloc_buf() -> UnsafePointer[UInt8]:
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
    chars.append(0)
    return (String(bytes=chars^), 4 + length)


fn _read_short_str(buf: UnsafePointer[UInt8], offset: Int) -> (String, Int):
    """Read a u16-length-prefixed string. Returns (string, bytes_consumed)."""
    var length = Int(_read_u16_le(buf, offset))
    var chars = List[UInt8](capacity=length + 1)
    for i in range(length):
        chars.append(buf[offset + 2 + i])
    chars.append(0)
    return (String(bytes=chars^), 2 + length)


# ── Mutation record types ────────────────────────────────────────────────────
# A simple tagged struct for decoded mutations.


struct Mutation(Copyable, Movable):
    var op: UInt8
    var id: UInt32
    var id2: UInt32  # second u32 (e.g. m in AppendChildren, index in LoadTemplate)
    var id3: UInt32  # third u32 (e.g. id in LoadTemplate)
    var text: String
    var name: String
    var ns: UInt8
    var path_len: UInt8

    fn __init__(out self, op: UInt8):
        self.op = op
        self.id = 0
        self.id2 = 0
        self.id3 = 0
        self.text = String("")
        self.name = String("")
        self.ns = 0
        self.path_len = 0

    fn __copyinit__(out self, other: Self):
        self.op = other.op
        self.id = other.id
        self.id2 = other.id2
        self.id3 = other.id3
        self.text = other.text
        self.name = other.name
        self.ns = other.ns
        self.path_len = other.path_len

    fn __moveinit__(out self, deinit other: Self):
        self.op = other.op
        self.id = other.id
        self.id2 = other.id2
        self.id3 = other.id3
        self.text = other.text^
        self.name = other.name^
        self.ns = other.ns
        self.path_len = other.path_len


fn _read_mutations(buf: UnsafePointer[UInt8], length: Int) -> List[Mutation]:
    """Decode all mutations from the buffer up to the End sentinel."""
    var result = List[Mutation]()
    var pos = 0
    while pos < length:
        var op = _read_u8(buf, pos)
        pos += 1
        if op == OP_END:
            break
        var m = Mutation(op)

        if op == OP_CREATE_TEXT_NODE:
            # id (u32) | len (u32) | text
            m.id = _read_u32_le(buf, pos)
            pos += 4
            var text_and_len = _read_str(buf, pos)
            m.text = text_and_len[0]
            pos += text_and_len[1]

        elif op == OP_CREATE_PLACEHOLDER:
            # id (u32)
            m.id = _read_u32_le(buf, pos)
            pos += 4

        elif op == OP_LOAD_TEMPLATE:
            # tmpl_id (u32) | index (u32) | id (u32)
            m.id = _read_u32_le(buf, pos)
            pos += 4
            m.id2 = _read_u32_le(buf, pos)
            pos += 4
            m.id3 = _read_u32_le(buf, pos)
            pos += 4

        elif op == OP_APPEND_CHILDREN:
            # id (u32) | m (u32)
            m.id = _read_u32_le(buf, pos)
            pos += 4
            m.id2 = _read_u32_le(buf, pos)
            pos += 4

        elif op == OP_ASSIGN_ID:
            # path_len (u8) | path | id (u32)
            m.path_len = _read_u8(buf, pos)
            pos += 1
            pos += Int(m.path_len)  # skip path bytes
            m.id = _read_u32_le(buf, pos)
            pos += 4

        elif op == OP_SET_TEXT:
            # id (u32) | len (u32) | text
            m.id = _read_u32_le(buf, pos)
            pos += 4
            var st = _read_str(buf, pos)
            m.text = st[0]
            pos += st[1]

        elif op == OP_SET_ATTRIBUTE:
            # id (u32) | ns (u8) | name_len (u16) | name | val_len (u32) | val
            m.id = _read_u32_le(buf, pos)
            pos += 4
            m.ns = _read_u8(buf, pos)
            pos += 1
            var name_pair = _read_short_str(buf, pos)
            m.name = name_pair[0]
            pos += name_pair[1]
            var val_pair = _read_str(buf, pos)
            m.text = val_pair[0]
            pos += val_pair[1]

        elif op == OP_NEW_EVENT_LISTENER:
            # id (u32) | name_len (u16) | name
            m.id = _read_u32_le(buf, pos)
            pos += 4
            var ev_name = _read_short_str(buf, pos)
            m.name = ev_name[0]
            pos += ev_name[1]

        elif op == OP_REMOVE_EVENT_LISTENER:
            # id (u32) | name_len (u16) | name
            m.id = _read_u32_le(buf, pos)
            pos += 4
            var ev_name2 = _read_short_str(buf, pos)
            m.name = ev_name2[0]
            pos += ev_name2[1]

        elif op == OP_REMOVE:
            # id (u32)
            m.id = _read_u32_le(buf, pos)
            pos += 4

        elif op == OP_PUSH_ROOT:
            # id (u32)
            m.id = _read_u32_le(buf, pos)
            pos += 4

        elif op == OP_REPLACE_WITH:
            # id (u32) | m (u32)
            m.id = _read_u32_le(buf, pos)
            pos += 4
            m.id2 = _read_u32_le(buf, pos)
            pos += 4

        elif op == OP_REPLACE_PLACEHOLDER:
            # path_len (u8) | path | m (u32)
            m.path_len = _read_u8(buf, pos)
            pos += 1
            pos += Int(m.path_len)
            m.id = _read_u32_le(buf, pos)  # m count stored in id
            pos += 4

        elif op == OP_INSERT_AFTER:
            # id (u32) | m (u32)
            m.id = _read_u32_le(buf, pos)
            pos += 4
            m.id2 = _read_u32_le(buf, pos)
            pos += 4

        elif op == OP_INSERT_BEFORE:
            # id (u32) | m (u32)
            m.id = _read_u32_le(buf, pos)
            pos += 4
            m.id2 = _read_u32_le(buf, pos)
            pos += 4

        result.append(m^)
    return result^


# ── Test context helpers ─────────────────────────────────────────────────────


struct TestContext(Movable):
    var rt: UnsafePointer[Runtime]
    var eid: UnsafePointer[ElementIdAllocator]
    var store: UnsafePointer[VNodeStore]
    var buf: UnsafePointer[UInt8]
    var writer: UnsafePointer[MutationWriter]

    fn __init__(out self):
        self.rt = UnsafePointer[Runtime].alloc(1)
        self.rt.init_pointee_move(Runtime())

        self.eid = UnsafePointer[ElementIdAllocator].alloc(1)
        self.eid.init_pointee_move(ElementIdAllocator())

        self.store = UnsafePointer[VNodeStore].alloc(1)
        self.store.init_pointee_move(VNodeStore())

        self.buf = _alloc_buf()

        self.writer = UnsafePointer[MutationWriter].alloc(1)
        self.writer.init_pointee_move(MutationWriter(self.buf, BUF_SIZE))

    fn __moveinit__(out self, deinit other: Self):
        self.rt = other.rt
        self.eid = other.eid
        self.store = other.store
        self.buf = other.buf
        self.writer = other.writer

    fn finalize_and_read(mut self) -> (List[Mutation], Int):
        """Finalize the writer and read back all mutations."""
        self.writer[].finalize()
        var offset = self.writer[].offset
        var mutations = _read_mutations(self.buf, offset)
        return (mutations^, offset)

    fn reset_writer(mut self):
        """Reset the writer for a new mutation sequence."""
        self.writer.destroy_pointee()
        for i in range(BUF_SIZE):
            self.buf[i] = 0
        self.writer.init_pointee_move(MutationWriter(self.buf, BUF_SIZE))

    fn destroy(mut self):
        self.writer.destroy_pointee()
        self.writer.free()
        _free_buf(self.buf)
        self.store.destroy_pointee()
        self.store.free()
        self.eid.destroy_pointee()
        self.eid.free()
        self.rt.destroy_pointee()
        self.rt.free()


# ── Template registration helpers ────────────────────────────────────────────


fn _register_div_template(mut ctx: TestContext, name: String) -> UInt32:
    """Register a simple <div></div> template, return ID."""
    var b = TemplateBuilder(name)
    _ = b.push_element(TAG_DIV, -1)
    var tmpl = b.build()
    return ctx.rt[].templates.register(tmpl^)


fn _register_div_with_dyn_text(mut ctx: TestContext, name: String) -> UInt32:
    """Register <div>{dyntext_0}</div>, return ID."""
    var b = TemplateBuilder(name)
    var div_idx = b.push_element(TAG_DIV, -1)
    _ = b.push_dynamic_text(UInt32(0), div_idx)
    var tmpl = b.build()
    return ctx.rt[].templates.register(tmpl^)


fn _register_div_with_dyn_attr(mut ctx: TestContext, name: String) -> UInt32:
    """Register <div {dynattr_0}></div>, return ID."""
    var b = TemplateBuilder(name)
    var div_idx = b.push_element(TAG_DIV, -1)
    b.push_dynamic_attr(div_idx, UInt32(0))
    var tmpl = b.build()
    return ctx.rt[].templates.register(tmpl^)


fn _register_div_with_dyn_node(mut ctx: TestContext, name: String) -> UInt32:
    """Register <div>{dyn_node_0}</div>, return ID."""
    var b = TemplateBuilder(name)
    var div_idx = b.push_element(TAG_DIV, -1)
    _ = b.push_dynamic(UInt32(0), div_idx)
    var tmpl = b.build()
    return ctx.rt[].templates.register(tmpl^)


fn _register_div_with_static_text(mut ctx: TestContext, name: String) -> UInt32:
    """Register <div>"hello"</div>, return ID."""
    var b = TemplateBuilder(name)
    var div_idx = b.push_element(TAG_DIV, -1)
    _ = b.push_text("hello", div_idx)
    var tmpl = b.build()
    return ctx.rt[].templates.register(tmpl^)


fn _register_complex_template(mut ctx: TestContext, name: String) -> UInt32:
    """Register a complex template:
    <div class="container">
      <h1>"Title"</h1>
      <p>{dyntext_0}</p>
      <button {dynattr_0}>{dyntext_1}</button>
    </div>
    """
    var b = TemplateBuilder(name)
    var div_idx = b.push_element(TAG_DIV, -1)
    b.push_static_attr(div_idx, "class", "container")

    var h1_idx = b.push_element(TAG_H1, div_idx)
    _ = b.push_text("Title", h1_idx)

    var p_idx = b.push_element(TAG_P, div_idx)
    _ = b.push_dynamic_text(UInt32(0), p_idx)

    var btn_idx = b.push_element(TAG_BUTTON, div_idx)
    b.push_dynamic_attr(btn_idx, UInt32(0))
    _ = b.push_dynamic_text(UInt32(1), btn_idx)

    var tmpl = b.build()
    return ctx.rt[].templates.register(tmpl^)


# ── Helper to count mutations by opcode ──────────────────────────────────────


fn _count_op(mutations: List[Mutation], op: UInt8) -> Int:
    var count = 0
    for i in range(len(mutations)):
        if mutations[i].op == op:
            count += 1
    return count


fn _find_first(mutations: List[Mutation], op: UInt8) -> Int:
    """Return index of first mutation with given op, or -1."""
    for i in range(len(mutations)):
        if mutations[i].op == op:
            return i
    return -1


# ══════════════════════════════════════════════════════════════════════════════
# CREATE ENGINE TESTS
# ══════════════════════════════════════════════════════════════════════════════


fn test_create_text_vnode() raises:
    var ctx = TestContext()

    var vn_idx = ctx.store[].push(VNode.text_node("hello world"))

    var engine = CreateEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    var num_roots = engine.create_node(vn_idx)

    assert_equal(Int(num_roots), 1, "text vnode creates 1 root")

    var result = ctx.finalize_and_read()
    var mutations = result[0]

    assert_equal(len(mutations), 1, "1 mutation emitted for text vnode")
    assert_equal(
        Int(mutations[0].op),
        Int(OP_CREATE_TEXT_NODE),
        "mutation is CreateTextNode",
    )
    assert_equal(mutations[0].text, "hello world", "text content is correct")
    assert_true(Int(mutations[0].id) > 0, "element id is non-zero")

    # Check mount state
    var vn_ptr = ctx.store[].get_ptr(vn_idx)
    assert_true(vn_ptr[].is_mounted(), "text vnode is mounted")
    assert_equal(vn_ptr[].root_id_count(), 1, "text vnode has 1 root id")
    assert_true(Int(vn_ptr[].get_root_id(0)) > 0, "root id is non-zero")

    ctx.destroy()


fn test_create_placeholder_vnode() raises:
    var ctx = TestContext()

    var vn_idx = ctx.store[].push(VNode.placeholder(UInt32(0)))

    var engine = CreateEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    var num_roots = engine.create_node(vn_idx)

    assert_equal(Int(num_roots), 1, "placeholder creates 1 root")

    var result = ctx.finalize_and_read()
    var mutations = result[0]

    assert_equal(len(mutations), 1, "1 mutation for placeholder")
    assert_equal(
        Int(mutations[0].op),
        Int(OP_CREATE_PLACEHOLDER),
        "mutation is CreatePlaceholder",
    )

    var vn_ptr = ctx.store[].get_ptr(vn_idx)
    assert_true(vn_ptr[].is_mounted(), "placeholder is mounted")
    assert_equal(vn_ptr[].root_id_count(), 1, "placeholder has 1 root id")

    ctx.destroy()


fn test_create_simple_template_ref() raises:
    var ctx = TestContext()

    var tmpl_id = _register_div_template(ctx, "simple-div")

    var vn_idx = ctx.store[].push(VNode.template_ref(tmpl_id))

    var engine = CreateEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    var num_roots = engine.create_node(vn_idx)

    assert_equal(Int(num_roots), 1, "template ref creates 1 root (div)")

    var result = ctx.finalize_and_read()
    var mutations = result[0]

    # Should have at least a LoadTemplate mutation
    var load_count = _count_op(mutations, OP_LOAD_TEMPLATE)
    assert_equal(load_count, 1, "1 LoadTemplate mutation")

    var load_idx = _find_first(mutations, OP_LOAD_TEMPLATE)
    assert_true(load_idx >= 0, "LoadTemplate found")
    assert_equal(
        Int(mutations[load_idx].id),
        Int(tmpl_id),
        "LoadTemplate uses correct template ID",
    )

    # Check mount state
    var vn_ptr = ctx.store[].get_ptr(vn_idx)
    assert_true(vn_ptr[].is_mounted(), "template ref is mounted")
    assert_equal(vn_ptr[].root_id_count(), 1, "template ref has 1 root id")

    ctx.destroy()


fn test_create_template_ref_with_dyn_text() raises:
    var ctx = TestContext()

    var tmpl_id = _register_div_with_dyn_text(ctx, "dyn-text-div")

    var vn = VNode.template_ref(tmpl_id)
    vn.push_dynamic_node(DynamicNode.text_node("Count: 42"))
    var vn_idx = ctx.store[].push(vn^)

    var engine = CreateEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    var num_roots = engine.create_node(vn_idx)

    assert_equal(Int(num_roots), 1, "template with dyntext creates 1 root")

    var result = ctx.finalize_and_read()
    var mutations = result[0]

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

    # Check that at least one SetText has our text
    var has_set_text = False
    for i in range(len(mutations)):
        if mutations[i].op == OP_SET_TEXT and mutations[i].text == "Count: 42":
            has_set_text = True
    assert_true(has_set_text, "SetText with 'Count: 42' emitted")

    # Check mount state: should have dynamic node IDs
    var vn_ptr = ctx.store[].get_ptr(vn_idx)
    assert_true(vn_ptr[].dyn_node_id_count() > 0, "has dynamic node IDs")

    ctx.destroy()


fn test_create_template_ref_with_dyn_attr() raises:
    var ctx = TestContext()

    var tmpl_id = _register_div_with_dyn_attr(ctx, "dyn-attr-div")

    var vn = VNode.template_ref(tmpl_id)
    vn.push_dynamic_attr(
        DynamicAttr("class", AttributeValue.text("active"), UInt32(0))
    )
    var vn_idx = ctx.store[].push(vn^)

    var engine = CreateEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    var num_roots = engine.create_node(vn_idx)

    assert_equal(Int(num_roots), 1, "template with dynattr creates 1 root")

    var result = ctx.finalize_and_read()
    var mutations = result[0]

    var load_count = _count_op(mutations, OP_LOAD_TEMPLATE)
    assert_equal(load_count, 1, "1 LoadTemplate")

    # Should have a SetAttribute mutation
    var set_attr_count = _count_op(mutations, OP_SET_ATTRIBUTE)
    assert_true(set_attr_count > 0, "has SetAttribute for dynamic attr")

    # Check mount state: should have dynamic attr IDs
    var vn_ptr = ctx.store[].get_ptr(vn_idx)
    assert_true(vn_ptr[].dyn_attr_id_count() > 0, "has dynamic attr IDs")

    ctx.destroy()


fn test_create_template_ref_with_event() raises:
    var ctx = TestContext()

    var tmpl_id = _register_div_with_dyn_attr(ctx, "event-div")

    var vn = VNode.template_ref(tmpl_id)
    vn.push_dynamic_attr(
        DynamicAttr("onclick", AttributeValue.event(UInt32(1)), UInt32(0))
    )
    var vn_idx = ctx.store[].push(vn^)

    var engine = CreateEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    _ = engine.create_node(vn_idx)

    var result = ctx.finalize_and_read()
    var mutations = result[0]

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
    var ctx = TestContext()

    var tmpl_id = _register_div_with_dyn_node(ctx, "dyn-node-div")

    var vn = VNode.template_ref(tmpl_id)
    vn.push_dynamic_node(DynamicNode.text_node("dynamic text"))
    var vn_idx = ctx.store[].push(vn^)

    var engine = CreateEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    var num_roots = engine.create_node(vn_idx)

    assert_equal(Int(num_roots), 1, "creates 1 root")

    var result = ctx.finalize_and_read()
    var mutations = result[0]

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
    var ctx = TestContext()

    var tmpl_id = _register_div_with_dyn_node(ctx, "dyn-ph-div")

    var vn = VNode.template_ref(tmpl_id)
    vn.push_dynamic_node(DynamicNode.placeholder())
    var vn_idx = ctx.store[].push(vn^)

    var engine = CreateEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    _ = engine.create_node(vn_idx)

    var result = ctx.finalize_and_read()
    var mutations = result[0]

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
    var ctx = TestContext()

    # Create 3 text children
    var c1 = ctx.store[].push(VNode.text_node("A"))
    var c2 = ctx.store[].push(VNode.text_node("B"))
    var c3 = ctx.store[].push(VNode.text_node("C"))

    # Create fragment
    var frag = VNode.fragment()
    frag.push_fragment_child(c1)
    frag.push_fragment_child(c2)
    frag.push_fragment_child(c3)
    var frag_idx = ctx.store[].push(frag^)

    var engine = CreateEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    var num_roots = engine.create_node(frag_idx)

    assert_equal(Int(num_roots), 3, "fragment creates 3 roots (one per child)")

    var result = ctx.finalize_and_read()
    var mutations = result[0]

    # Should have 3 CreateTextNode mutations
    var create_text_count = _count_op(mutations, OP_CREATE_TEXT_NODE)
    assert_equal(create_text_count, 3, "3 CreateTextNode mutations")

    ctx.destroy()


fn test_create_element_id_uniqueness() raises:
    """Multiple create calls produce unique ElementIds."""
    var ctx = TestContext()

    var tmpl_id = _register_div_template(ctx, "unique-div")

    var vn1 = ctx.store[].push(VNode.template_ref(tmpl_id))
    var vn2 = ctx.store[].push(VNode.template_ref(tmpl_id))
    var vn3 = ctx.store[].push(VNode.template_ref(tmpl_id))

    var engine = CreateEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    _ = engine.create_node(vn1)
    _ = engine.create_node(vn2)
    _ = engine.create_node(vn3)

    var id1 = ctx.store[].get_ptr(vn1)[].get_root_id(0)
    var id2 = ctx.store[].get_ptr(vn2)[].get_root_id(0)
    var id3 = ctx.store[].get_ptr(vn3)[].get_root_id(0)

    assert_true(Int(id1) != Int(id2), "id1 != id2")
    assert_true(Int(id2) != Int(id3), "id2 != id3")
    assert_true(Int(id1) != Int(id3), "id1 != id3")

    ctx.destroy()


# ══════════════════════════════════════════════════════════════════════════════
# DIFF ENGINE TESTS
# ══════════════════════════════════════════════════════════════════════════════


fn test_diff_same_text_zero_mutations() raises:
    """Diffing two Text VNodes with the same text → 0 mutations."""
    var ctx = TestContext()

    # Create old text vnode and mount it
    var old_idx = ctx.store[].push(VNode.text_node("hello"))
    var engine = CreateEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    _ = engine.create_node(old_idx)

    # Reset writer for diff
    ctx.reset_writer()

    # Create new text vnode with same text
    var new_idx = ctx.store[].push(VNode.text_node("hello"))

    var diff = DiffEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    diff.diff_node(old_idx, new_idx)

    var result = ctx.finalize_and_read()
    var mutations = result[0]

    assert_equal(len(mutations), 0, "same text produces 0 mutations")

    ctx.destroy()


fn test_diff_text_changed_produces_set_text() raises:
    """Diffing two Text VNodes with different text → SetText."""
    var ctx = TestContext()

    # Create old text vnode and mount it
    var old_idx = ctx.store[].push(VNode.text_node("hello"))
    var engine = CreateEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    _ = engine.create_node(old_idx)

    # Remember the old root id
    var old_root_id = ctx.store[].get_ptr(old_idx)[].get_root_id(0)

    # Reset writer for diff
    ctx.reset_writer()

    # Create new text vnode with different text
    var new_idx = ctx.store[].push(VNode.text_node("world"))

    var diff = DiffEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    diff.diff_node(old_idx, new_idx)

    var result = ctx.finalize_and_read()
    var mutations = result[0]

    assert_true(len(mutations) > 0, "text change produces mutations")

    # Should have a SetText mutation targeting the old root id
    var has_set_text = False
    for i in range(len(mutations)):
        if mutations[i].op == OP_SET_TEXT:
            has_set_text = True
            assert_equal(
                Int(mutations[i].id),
                Int(old_root_id),
                "SetText targets old root id",
            )
            assert_equal(mutations[i].text, "world", "SetText has new text")
    assert_true(has_set_text, "SetText emitted")

    ctx.destroy()


fn test_diff_text_empty_to_content() raises:
    """Diffing '' → 'hello' produces SetText."""
    var ctx = TestContext()

    var old_idx = ctx.store[].push(VNode.text_node(""))
    var engine = CreateEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    _ = engine.create_node(old_idx)
    ctx.reset_writer()

    var new_idx = ctx.store[].push(VNode.text_node("hello"))
    var diff = DiffEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    diff.diff_node(old_idx, new_idx)

    var result = ctx.finalize_and_read()
    var mutations = result[0]

    var set_text_count = _count_op(mutations, OP_SET_TEXT)
    assert_equal(set_text_count, 1, "1 SetText for '' -> 'hello'")

    ctx.destroy()


fn test_diff_placeholder_to_placeholder_zero_mutations() raises:
    """Diffing two Placeholders → 0 mutations."""
    var ctx = TestContext()

    var old_idx = ctx.store[].push(VNode.placeholder(UInt32(0)))
    var engine = CreateEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    _ = engine.create_node(old_idx)
    ctx.reset_writer()

    var new_idx = ctx.store[].push(VNode.placeholder(UInt32(0)))
    var diff = DiffEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    diff.diff_node(old_idx, new_idx)

    var result = ctx.finalize_and_read()
    var mutations = result[0]

    assert_equal(
        len(mutations), 0, "placeholder -> placeholder produces 0 mutations"
    )

    ctx.destroy()


fn test_diff_same_template_same_dyn_values_zero_mutations() raises:
    """Same template, same dynamic text → 0 mutations."""
    var ctx = TestContext()

    var tmpl_id = _register_div_with_dyn_text(ctx, "same-dyn")

    # Old VNode
    var old_vn = VNode.template_ref(tmpl_id)
    old_vn.push_dynamic_node(DynamicNode.text_node("Count: 5"))
    var old_idx = ctx.store[].push(old_vn^)

    var engine = CreateEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    _ = engine.create_node(old_idx)
    ctx.reset_writer()

    # New VNode with same dynamic text
    var new_vn = VNode.template_ref(tmpl_id)
    new_vn.push_dynamic_node(DynamicNode.text_node("Count: 5"))
    var new_idx = ctx.store[].push(new_vn^)

    var diff = DiffEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    diff.diff_node(old_idx, new_idx)

    var result = ctx.finalize_and_read()
    var mutations = result[0]

    assert_equal(
        len(mutations), 0, "same template + same dyntext produces 0 mutations"
    )

    ctx.destroy()


fn test_diff_same_template_dyn_text_changed() raises:
    """Same template, dynamic text changed → SetText."""
    var ctx = TestContext()

    var tmpl_id = _register_div_with_dyn_text(ctx, "changed-dyn")

    # Old VNode
    var old_vn = VNode.template_ref(tmpl_id)
    old_vn.push_dynamic_node(DynamicNode.text_node("Count: 5"))
    var old_idx = ctx.store[].push(old_vn^)

    var engine = CreateEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    _ = engine.create_node(old_idx)

    # Get the dynamic node ID assigned during create
    var old_dyn_id = ctx.store[].get_ptr(old_idx)[].get_dyn_node_id(0)

    ctx.reset_writer()

    # New VNode with different dynamic text
    var new_vn = VNode.template_ref(tmpl_id)
    new_vn.push_dynamic_node(DynamicNode.text_node("Count: 10"))
    var new_idx = ctx.store[].push(new_vn^)

    var diff = DiffEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    diff.diff_node(old_idx, new_idx)

    var result = ctx.finalize_and_read()
    var mutations = result[0]

    # Should have a SetText targeting the dynamic node's element ID
    var has_set_text = False
    for i in range(len(mutations)):
        if mutations[i].op == OP_SET_TEXT:
            has_set_text = True
            assert_equal(
                Int(mutations[i].id),
                Int(old_dyn_id),
                "SetText targets dynamic node element",
            )
            assert_equal(mutations[i].text, "Count: 10", "SetText has new text")
    assert_true(has_set_text, "SetText emitted for changed dynamic text")

    ctx.destroy()


fn test_diff_same_template_attr_changed() raises:
    """Same template, dynamic attribute changed → SetAttribute."""
    var ctx = TestContext()

    var tmpl_id = _register_div_with_dyn_attr(ctx, "attr-changed")

    # Old VNode
    var old_vn = VNode.template_ref(tmpl_id)
    old_vn.push_dynamic_attr(
        DynamicAttr("class", AttributeValue.text("old-class"), UInt32(0))
    )
    var old_idx = ctx.store[].push(old_vn^)

    var engine = CreateEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    _ = engine.create_node(old_idx)
    ctx.reset_writer()

    # New VNode with different attr value
    var new_vn = VNode.template_ref(tmpl_id)
    new_vn.push_dynamic_attr(
        DynamicAttr("class", AttributeValue.text("new-class"), UInt32(0))
    )
    var new_idx = ctx.store[].push(new_vn^)

    var diff = DiffEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    diff.diff_node(old_idx, new_idx)

    var result = ctx.finalize_and_read()
    var mutations = result[0]

    var set_attr_count = _count_op(mutations, OP_SET_ATTRIBUTE)
    assert_true(set_attr_count > 0, "SetAttribute emitted for changed attr")

    ctx.destroy()


fn test_diff_same_template_attr_unchanged_zero_mutations() raises:
    """Same template, same attribute value → 0 mutations."""
    var ctx = TestContext()

    var tmpl_id = _register_div_with_dyn_attr(ctx, "attr-same")

    # Old VNode
    var old_vn = VNode.template_ref(tmpl_id)
    old_vn.push_dynamic_attr(
        DynamicAttr("class", AttributeValue.text("same"), UInt32(0))
    )
    var old_idx = ctx.store[].push(old_vn^)

    var engine = CreateEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    _ = engine.create_node(old_idx)
    ctx.reset_writer()

    # New VNode with same attr value
    var new_vn = VNode.template_ref(tmpl_id)
    new_vn.push_dynamic_attr(
        DynamicAttr("class", AttributeValue.text("same"), UInt32(0))
    )
    var new_idx = ctx.store[].push(new_vn^)

    var diff = DiffEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    diff.diff_node(old_idx, new_idx)

    var result = ctx.finalize_and_read()
    var mutations = result[0]

    assert_equal(len(mutations), 0, "same attr value produces 0 mutations")

    ctx.destroy()


fn test_diff_bool_attr_changed() raises:
    """Bool attribute changed → SetAttribute."""
    var ctx = TestContext()

    var tmpl_id = _register_div_with_dyn_attr(ctx, "bool-attr")

    var old_vn = VNode.template_ref(tmpl_id)
    old_vn.push_dynamic_attr(
        DynamicAttr("disabled", AttributeValue.boolean(False), UInt32(0))
    )
    var old_idx = ctx.store[].push(old_vn^)

    var engine = CreateEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    _ = engine.create_node(old_idx)
    ctx.reset_writer()

    var new_vn = VNode.template_ref(tmpl_id)
    new_vn.push_dynamic_attr(
        DynamicAttr("disabled", AttributeValue.boolean(True), UInt32(0))
    )
    var new_idx = ctx.store[].push(new_vn^)

    var diff = DiffEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    diff.diff_node(old_idx, new_idx)

    var result = ctx.finalize_and_read()
    var mutations = result[0]

    var set_attr_count = _count_op(mutations, OP_SET_ATTRIBUTE)
    assert_true(set_attr_count > 0, "SetAttribute emitted for bool change")

    ctx.destroy()


fn test_diff_text_to_placeholder_replacement() raises:
    """Text → Placeholder (different kind) → replacement."""
    var ctx = TestContext()

    var old_idx = ctx.store[].push(VNode.text_node("hello"))
    var engine = CreateEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    _ = engine.create_node(old_idx)
    ctx.reset_writer()

    var new_idx = ctx.store[].push(VNode.placeholder(UInt32(0)))
    var diff = DiffEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    diff.diff_node(old_idx, new_idx)

    var result = ctx.finalize_and_read()
    var mutations = result[0]

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
    """Different templates → full replacement."""
    var ctx = TestContext()

    var tmpl_a = _register_div_template(ctx, "tmpl-a")
    var tmpl_b = _register_div_template(ctx, "tmpl-b")

    var old_vn = VNode.template_ref(tmpl_a)
    var old_idx = ctx.store[].push(old_vn^)
    var engine = CreateEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    _ = engine.create_node(old_idx)
    ctx.reset_writer()

    var new_vn = VNode.template_ref(tmpl_b)
    var new_idx = ctx.store[].push(new_vn^)

    var diff = DiffEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    diff.diff_node(old_idx, new_idx)

    var result = ctx.finalize_and_read()
    var mutations = result[0]

    # Should have LoadTemplate for the new template + ReplaceWith
    var has_load = _count_op(mutations, OP_LOAD_TEMPLATE) > 0
    var has_replace = _count_op(mutations, OP_REPLACE_WITH) > 0
    assert_true(has_load, "LoadTemplate for new template in replacement")
    assert_true(has_replace, "ReplaceWith for different templates")

    ctx.destroy()


fn test_diff_fragment_children_text_changed() raises:
    """Fragment diff: child text changed."""
    var ctx = TestContext()

    # Old fragment: [A, B]
    var oa = ctx.store[].push(VNode.text_node("A"))
    var ob = ctx.store[].push(VNode.text_node("B"))
    var old_frag = VNode.fragment()
    old_frag.push_fragment_child(oa)
    old_frag.push_fragment_child(ob)
    var old_frag_idx = ctx.store[].push(old_frag^)

    var engine = CreateEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    _ = engine.create_node(old_frag_idx)
    ctx.reset_writer()

    # New fragment: [A, C] (B -> C)
    var na = ctx.store[].push(VNode.text_node("A"))
    var nc = ctx.store[].push(VNode.text_node("C"))
    var new_frag = VNode.fragment()
    new_frag.push_fragment_child(na)
    new_frag.push_fragment_child(nc)
    var new_frag_idx = ctx.store[].push(new_frag^)

    var diff = DiffEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    diff.diff_node(old_frag_idx, new_frag_idx)

    var result = ctx.finalize_and_read()
    var mutations = result[0]

    # "A" same → no mutation, "B" → "C" → SetText
    var set_text_count = _count_op(mutations, OP_SET_TEXT)
    assert_equal(set_text_count, 1, "1 SetText for B -> C")

    ctx.destroy()


fn test_diff_fragment_children_removed() raises:
    """Fragment diff: children removed."""
    var ctx = TestContext()

    # Old fragment: [A, B, C]
    var oa = ctx.store[].push(VNode.text_node("A"))
    var ob = ctx.store[].push(VNode.text_node("B"))
    var oc = ctx.store[].push(VNode.text_node("C"))
    var old_frag = VNode.fragment()
    old_frag.push_fragment_child(oa)
    old_frag.push_fragment_child(ob)
    old_frag.push_fragment_child(oc)
    var old_frag_idx = ctx.store[].push(old_frag^)

    var engine = CreateEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    _ = engine.create_node(old_frag_idx)
    ctx.reset_writer()

    # New fragment: [A] (B, C removed)
    var na = ctx.store[].push(VNode.text_node("A"))
    var new_frag = VNode.fragment()
    new_frag.push_fragment_child(na)
    var new_frag_idx = ctx.store[].push(new_frag^)

    var diff = DiffEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    diff.diff_node(old_frag_idx, new_frag_idx)

    var result = ctx.finalize_and_read()
    var mutations = result[0]

    # Should have Remove mutations for the extra children
    var remove_count = _count_op(mutations, OP_REMOVE)
    assert_true(remove_count >= 2, "at least 2 Remove mutations for B and C")

    ctx.destroy()


fn test_diff_fragment_children_added() raises:
    """Fragment diff: children added."""
    var ctx = TestContext()

    # Old fragment: [A]
    var oa = ctx.store[].push(VNode.text_node("A"))
    var old_frag = VNode.fragment()
    old_frag.push_fragment_child(oa)
    var old_frag_idx = ctx.store[].push(old_frag^)

    var engine = CreateEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    _ = engine.create_node(old_frag_idx)
    ctx.reset_writer()

    # New fragment: [A, B, C]
    var na = ctx.store[].push(VNode.text_node("A"))
    var nb = ctx.store[].push(VNode.text_node("B"))
    var nc = ctx.store[].push(VNode.text_node("C"))
    var new_frag = VNode.fragment()
    new_frag.push_fragment_child(na)
    new_frag.push_fragment_child(nb)
    new_frag.push_fragment_child(nc)
    var new_frag_idx = ctx.store[].push(new_frag^)

    var diff = DiffEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    diff.diff_node(old_frag_idx, new_frag_idx)

    var result = ctx.finalize_and_read()
    var mutations = result[0]

    # Should have CreateTextNode for B and C, and InsertAfter
    var create_count = _count_op(mutations, OP_CREATE_TEXT_NODE)
    assert_true(create_count >= 2, "at least 2 CreateTextNode for B and C")

    var has_insert = _count_op(mutations, OP_INSERT_AFTER) > 0
    assert_true(has_insert, "has InsertAfter for added children")

    ctx.destroy()


fn test_diff_event_listener_changed() raises:
    """Event listener handler changed → RemoveEventListener + NewEventListener.
    """
    var ctx = TestContext()

    var tmpl_id = _register_div_with_dyn_attr(ctx, "event-change")

    var old_vn = VNode.template_ref(tmpl_id)
    old_vn.push_dynamic_attr(
        DynamicAttr("onclick", AttributeValue.event(UInt32(1)), UInt32(0))
    )
    var old_idx = ctx.store[].push(old_vn^)

    var engine = CreateEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    _ = engine.create_node(old_idx)
    ctx.reset_writer()

    var new_vn = VNode.template_ref(tmpl_id)
    new_vn.push_dynamic_attr(
        DynamicAttr("onclick", AttributeValue.event(UInt32(2)), UInt32(0))
    )
    var new_idx = ctx.store[].push(new_vn^)

    var diff = DiffEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    diff.diff_node(old_idx, new_idx)

    var result = ctx.finalize_and_read()
    var mutations = result[0]

    var has_remove = _count_op(mutations, OP_REMOVE_EVENT_LISTENER) > 0
    var has_new = _count_op(mutations, OP_NEW_EVENT_LISTENER) > 0
    assert_true(has_remove, "RemoveEventListener for old handler")
    assert_true(has_new, "NewEventListener for new handler")

    ctx.destroy()


fn test_diff_same_event_listener_zero_mutations() raises:
    """Same event listener → 0 mutations."""
    var ctx = TestContext()

    var tmpl_id = _register_div_with_dyn_attr(ctx, "event-same")

    var old_vn = VNode.template_ref(tmpl_id)
    old_vn.push_dynamic_attr(
        DynamicAttr("onclick", AttributeValue.event(UInt32(1)), UInt32(0))
    )
    var old_idx = ctx.store[].push(old_vn^)

    var engine = CreateEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    _ = engine.create_node(old_idx)
    ctx.reset_writer()

    var new_vn = VNode.template_ref(tmpl_id)
    new_vn.push_dynamic_attr(
        DynamicAttr("onclick", AttributeValue.event(UInt32(1)), UInt32(0))
    )
    var new_idx = ctx.store[].push(new_vn^)

    var diff = DiffEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    diff.diff_node(old_idx, new_idx)

    var result = ctx.finalize_and_read()
    var mutations = result[0]

    assert_equal(len(mutations), 0, "same event listener produces 0 mutations")

    ctx.destroy()


fn test_diff_attr_type_changed_text_to_bool() raises:
    """Attribute type changed (text → bool) → SetAttribute."""
    var ctx = TestContext()

    var tmpl_id = _register_div_with_dyn_attr(ctx, "type-change")

    var old_vn = VNode.template_ref(tmpl_id)
    old_vn.push_dynamic_attr(
        DynamicAttr("disabled", AttributeValue.text("yes"), UInt32(0))
    )
    var old_idx = ctx.store[].push(old_vn^)

    var engine = CreateEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    _ = engine.create_node(old_idx)
    ctx.reset_writer()

    var new_vn = VNode.template_ref(tmpl_id)
    new_vn.push_dynamic_attr(
        DynamicAttr("disabled", AttributeValue.boolean(True), UInt32(0))
    )
    var new_idx = ctx.store[].push(new_vn^)

    var diff = DiffEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    diff.diff_node(old_idx, new_idx)

    var result = ctx.finalize_and_read()
    var mutations = result[0]

    var set_attr_count = _count_op(mutations, OP_SET_ATTRIBUTE)
    assert_true(set_attr_count > 0, "SetAttribute for type change")

    ctx.destroy()


fn test_diff_attr_removed_text_to_none() raises:
    """Attribute removed (text → none) → SetAttribute with empty value."""
    var ctx = TestContext()

    var tmpl_id = _register_div_with_dyn_attr(ctx, "attr-remove")

    var old_vn = VNode.template_ref(tmpl_id)
    old_vn.push_dynamic_attr(
        DynamicAttr("class", AttributeValue.text("active"), UInt32(0))
    )
    var old_idx = ctx.store[].push(old_vn^)

    var engine = CreateEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    _ = engine.create_node(old_idx)
    ctx.reset_writer()

    var new_vn = VNode.template_ref(tmpl_id)
    new_vn.push_dynamic_attr(
        DynamicAttr("class", AttributeValue.none(), UInt32(0))
    )
    var new_idx = ctx.store[].push(new_vn^)

    var diff = DiffEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    diff.diff_node(old_idx, new_idx)

    var result = ctx.finalize_and_read()
    var mutations = result[0]

    # Should emit a SetAttribute with empty value (attribute removal)
    var set_attr_count = _count_op(mutations, OP_SET_ATTRIBUTE)
    assert_true(set_attr_count > 0, "SetAttribute for attr removal")

    ctx.destroy()


fn test_diff_int_attr_changed() raises:
    """Integer attribute value changed → SetAttribute."""
    var ctx = TestContext()

    var tmpl_id = _register_div_with_dyn_attr(ctx, "int-attr")

    var old_vn = VNode.template_ref(tmpl_id)
    old_vn.push_dynamic_attr(
        DynamicAttr("tabindex", AttributeValue.integer(Int64(1)), UInt32(0))
    )
    var old_idx = ctx.store[].push(old_vn^)

    var engine = CreateEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    _ = engine.create_node(old_idx)
    ctx.reset_writer()

    var new_vn = VNode.template_ref(tmpl_id)
    new_vn.push_dynamic_attr(
        DynamicAttr("tabindex", AttributeValue.integer(Int64(5)), UInt32(0))
    )
    var new_idx = ctx.store[].push(new_vn^)

    var diff = DiffEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    diff.diff_node(old_idx, new_idx)

    var result = ctx.finalize_and_read()
    var mutations = result[0]

    var set_attr_count = _count_op(mutations, OP_SET_ATTRIBUTE)
    assert_true(set_attr_count > 0, "SetAttribute for int change")

    ctx.destroy()


fn test_diff_mount_state_transfer_preserves_ids() raises:
    """Diff transfers mount state: ElementIds on new VNode match old."""
    var ctx = TestContext()

    var tmpl_id = _register_div_with_dyn_text(ctx, "transfer-test")

    # Old VNode
    var old_vn = VNode.template_ref(tmpl_id)
    old_vn.push_dynamic_node(DynamicNode.text_node("old"))
    var old_idx = ctx.store[].push(old_vn^)

    var engine = CreateEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    _ = engine.create_node(old_idx)

    var old_root_id = ctx.store[].get_ptr(old_idx)[].get_root_id(0)
    var old_dyn_id = ctx.store[].get_ptr(old_idx)[].get_dyn_node_id(0)

    ctx.reset_writer()

    # New VNode
    var new_vn = VNode.template_ref(tmpl_id)
    new_vn.push_dynamic_node(DynamicNode.text_node("new"))
    var new_idx = ctx.store[].push(new_vn^)

    var diff = DiffEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    diff.diff_node(old_idx, new_idx)

    # Check that new VNode got the same ElementIds
    var new_root_id = ctx.store[].get_ptr(new_idx)[].get_root_id(0)
    var new_dyn_id = ctx.store[].get_ptr(new_idx)[].get_dyn_node_id(0)

    assert_equal(
        Int(new_root_id), Int(old_root_id), "root ID transferred to new VNode"
    )
    assert_equal(
        Int(new_dyn_id),
        Int(old_dyn_id),
        "dynamic node ID transferred to new VNode",
    )

    ctx.destroy()


fn test_diff_sequential_diffs_state_chain() raises:
    """Sequential diffs (state chain): v0 → v1 → v2 → v3."""
    var ctx = TestContext()

    var tmpl_id = _register_div_with_dyn_text(ctx, "chain-test")

    # v0: initial
    var v0 = VNode.template_ref(tmpl_id)
    v0.push_dynamic_node(DynamicNode.text_node("state-0"))
    var v0_idx = ctx.store[].push(v0^)

    var engine = CreateEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    _ = engine.create_node(v0_idx)
    ctx.reset_writer()

    # v0 → v1
    var v1 = VNode.template_ref(tmpl_id)
    v1.push_dynamic_node(DynamicNode.text_node("state-1"))
    var v1_idx = ctx.store[].push(v1^)

    var diff1 = DiffEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    diff1.diff_node(v0_idx, v1_idx)

    var r1 = ctx.finalize_and_read()
    var muts1 = r1[0]
    var st1 = _count_op(muts1, OP_SET_TEXT)
    assert_equal(st1, 1, "v0 -> v1: 1 SetText")

    ctx.reset_writer()

    # v1 → v2
    var v2 = VNode.template_ref(tmpl_id)
    v2.push_dynamic_node(DynamicNode.text_node("state-2"))
    var v2_idx = ctx.store[].push(v2^)

    var diff2 = DiffEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    diff2.diff_node(v1_idx, v2_idx)

    var r2 = ctx.finalize_and_read()
    var muts2 = r2[0]
    var st2 = _count_op(muts2, OP_SET_TEXT)
    assert_equal(st2, 1, "v1 -> v2: 1 SetText")

    ctx.reset_writer()

    # v2 → v3 (same text → 0)
    var v3 = VNode.template_ref(tmpl_id)
    v3.push_dynamic_node(DynamicNode.text_node("state-2"))
    var v3_idx = ctx.store[].push(v3^)

    var diff3 = DiffEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    diff3.diff_node(v2_idx, v3_idx)

    var r3 = ctx.finalize_and_read()
    var muts3 = r3[0]
    assert_equal(len(muts3), 0, "v2 -> v3 same text: 0 mutations")

    ctx.destroy()


fn test_create_complex_template_multi_slots() raises:
    """Create a complex template with multiple dynamic slots."""
    var ctx = TestContext()

    var tmpl_id = _register_complex_template(ctx, "complex")

    var vn = VNode.template_ref(tmpl_id)
    # dyntext_0 → "Description"
    vn.push_dynamic_node(DynamicNode.text_node("Description"))
    # dyntext_1 → "Click me"
    vn.push_dynamic_node(DynamicNode.text_node("Click me"))
    # dynattr_0 → onclick event
    vn.push_dynamic_attr(
        DynamicAttr("onclick", AttributeValue.event(UInt32(42)), UInt32(0))
    )

    var vn_idx = ctx.store[].push(vn^)

    var engine = CreateEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    var num_roots = engine.create_node(vn_idx)

    assert_equal(Int(num_roots), 1, "complex template creates 1 root")

    var result = ctx.finalize_and_read()
    var mutations = result[0]

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


fn test_create_empty_fragment() raises:
    """Empty fragment creates 0 roots."""
    var ctx = TestContext()

    var frag_idx = ctx.store[].push(VNode.fragment())

    var engine = CreateEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    var num_roots = engine.create_node(frag_idx)

    assert_equal(Int(num_roots), 0, "empty fragment creates 0 roots")

    var result = ctx.finalize_and_read()
    var mutations = result[0]

    assert_equal(len(mutations), 0, "empty fragment produces 0 mutations")

    ctx.destroy()


fn test_diff_dyn_node_text_to_placeholder() raises:
    """Dynamic node: text → placeholder → replacement."""
    var ctx = TestContext()

    var tmpl_id = _register_div_with_dyn_node(ctx, "dyn-text-to-ph")

    var old_vn = VNode.template_ref(tmpl_id)
    old_vn.push_dynamic_node(DynamicNode.text_node("some text"))
    var old_idx = ctx.store[].push(old_vn^)

    var engine = CreateEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    _ = engine.create_node(old_idx)
    ctx.reset_writer()

    var new_vn = VNode.template_ref(tmpl_id)
    new_vn.push_dynamic_node(DynamicNode.placeholder())
    var new_idx = ctx.store[].push(new_vn^)

    var diff = DiffEngine(ctx.writer, ctx.eid, ctx.rt, ctx.store)
    diff.diff_node(old_idx, new_idx)

    var result = ctx.finalize_and_read()
    var mutations = result[0]

    # Should have CreatePlaceholder and ReplaceWith
    var has_create_ph = _count_op(mutations, OP_CREATE_PLACEHOLDER) > 0
    var has_replace = _count_op(mutations, OP_REPLACE_WITH) > 0
    assert_true(has_create_ph, "CreatePlaceholder for text -> placeholder")
    assert_true(has_replace, "ReplaceWith for text -> placeholder")

    ctx.destroy()

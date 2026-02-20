# Tests for TemplateBuilder, TemplateRegistry, and VNodeStore — native Mojo tests.
#
# These tests are a direct port of test/templates.test.ts, exercising
# the same scenarios without the WASM/JS round-trip.
#
# Run with:
#   mojo test -I src test-mojo/test_templates.mojo

from testing import assert_equal, assert_true, assert_false

from signals import Runtime, create_runtime, destroy_runtime
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
    TAG_H3,
    TAG_UL,
    TAG_OL,
    TAG_LI,
    TAG_BUTTON,
    TAG_INPUT,
    TAG_FORM,
    TAG_A,
    TAG_IMG,
    TAG_TABLE,
    TAG_TR,
    TAG_TD,
    TAG_TH,
    TAG_UNKNOWN,
    create_builder,
    destroy_builder,
)


# ── Helpers ──────────────────────────────────────────────────────────────────


fn _make_runtime() -> UnsafePointer[Runtime]:
    return create_runtime()


fn _teardown(rt: UnsafePointer[Runtime]):
    destroy_runtime(rt)


# ══════════════════════════════════════════════════════════════════════════════
# Template Builder — basic lifecycle
# ══════════════════════════════════════════════════════════════════════════════


fn test_builder_basic_lifecycle() raises:
    var rt = _make_runtime()
    var b = TemplateBuilder("test-basic")

    # Push a single div root
    var div_idx = b.push_element(TAG_DIV, -1)
    assert_equal(div_idx, 0, "first element is at index 0")

    # Push a child span inside the div
    var span_idx = b.push_element(TAG_SPAN, div_idx)
    assert_equal(span_idx, 1, "second element is at index 1")

    # Push a text node inside the span
    var text_idx = b.push_text("hello", span_idx)
    assert_equal(text_idx, 2, "text node is at index 2")

    # Check builder counts
    assert_equal(b.node_count(), 3, "builder has 3 nodes")
    assert_equal(b.root_count(), 1, "builder has 1 root")

    # Register
    var tmpl = b.build()
    var tmpl_id = rt[].templates.register(tmpl^)
    assert_equal(Int(tmpl_id), 0, "first template gets ID 0")
    assert_equal(rt[].templates.count(), 1, "1 template registered")

    _teardown(rt)


# ══════════════════════════════════════════════════════════════════════════════
# Template Registry — register and query
# ══════════════════════════════════════════════════════════════════════════════


fn test_registry_register_and_query() raises:
    var rt = _make_runtime()

    # Register first template
    var b1 = TemplateBuilder("alpha")
    _ = b1.push_element(TAG_DIV, -1)
    var t1 = b1.build()
    var id1 = rt[].templates.register(t1^)
    assert_equal(Int(id1), 0, "alpha gets ID 0")

    # Register second template
    var b2 = TemplateBuilder("beta")
    _ = b2.push_element(TAG_SPAN, -1)
    var t2 = b2.build()
    var id2 = rt[].templates.register(t2^)
    assert_equal(Int(id2), 1, "beta gets ID 1")

    assert_equal(rt[].templates.count(), 2, "2 templates registered")

    # Look up by name
    assert_true(rt[].templates.contains_name("alpha"), "contains 'alpha'")
    assert_true(rt[].templates.contains_name("beta"), "contains 'beta'")
    assert_false(
        rt[].templates.contains_name("gamma"), "does not contain 'gamma'"
    )

    assert_equal(rt[].templates.find_by_name("alpha"), 0, "find 'alpha' -> 0")
    assert_equal(rt[].templates.find_by_name("beta"), 1, "find 'beta' -> 1")
    assert_equal(rt[].templates.find_by_name("gamma"), -1, "find 'gamma' -> -1")

    _teardown(rt)


# ══════════════════════════════════════════════════════════════════════════════
# Template Structure — node queries
# ══════════════════════════════════════════════════════════════════════════════


fn test_template_structure_node_queries() raises:
    var rt = _make_runtime()

    # Build: div > (h1 > "Title", p > "Body")
    var b = TemplateBuilder("structure")
    var div_idx = b.push_element(TAG_DIV, -1)
    var h1_idx = b.push_element(TAG_H1, div_idx)
    _ = b.push_text("Title", h1_idx)
    var p_idx = b.push_element(TAG_P, div_idx)
    _ = b.push_text("Body", p_idx)

    var tmpl = b.build()
    var tmpl_id = rt[].templates.register(tmpl^)

    # Roots
    assert_equal(rt[].templates.root_count(tmpl_id), 1, "1 root")
    assert_equal(
        Int(rt[].templates.get_root_index(tmpl_id, 0)), 0, "root is node 0"
    )

    # Total nodes: div + h1 + "Title" + p + "Body" = 5
    assert_equal(rt[].templates.node_count(tmpl_id), 5, "5 nodes total")

    # div (node 0) is Element with TAG_DIV, 2 children
    assert_equal(
        Int(rt[].templates.node_kind(tmpl_id, 0)),
        Int(TNODE_ELEMENT),
        "node 0 is Element",
    )
    assert_equal(
        Int(rt[].templates.node_html_tag(tmpl_id, 0)),
        Int(TAG_DIV),
        "node 0 tag is DIV",
    )
    assert_equal(
        rt[].templates.node_child_count(tmpl_id, 0), 2, "div has 2 children"
    )
    assert_equal(
        Int(rt[].templates.node_child_at(tmpl_id, 0, 0)),
        1,
        "div child 0 is node 1 (h1)",
    )
    assert_equal(
        Int(rt[].templates.node_child_at(tmpl_id, 0, 1)),
        3,
        "div child 1 is node 3 (p)",
    )

    # h1 (node 1)
    assert_equal(
        Int(rt[].templates.node_kind(tmpl_id, 1)),
        Int(TNODE_ELEMENT),
        "node 1 is Element",
    )
    assert_equal(
        Int(rt[].templates.node_html_tag(tmpl_id, 1)),
        Int(TAG_H1),
        "node 1 tag is H1",
    )
    assert_equal(
        rt[].templates.node_child_count(tmpl_id, 1), 1, "h1 has 1 child"
    )

    # "Title" (node 2) is Text
    assert_equal(
        Int(rt[].templates.node_kind(tmpl_id, 2)),
        Int(TNODE_TEXT),
        "node 2 is Text",
    )
    assert_equal(
        rt[].templates.node_child_count(tmpl_id, 2),
        0,
        "text node has 0 children",
    )

    # p (node 3)
    assert_equal(
        Int(rt[].templates.node_kind(tmpl_id, 3)),
        Int(TNODE_ELEMENT),
        "node 3 is Element",
    )
    assert_equal(
        Int(rt[].templates.node_html_tag(tmpl_id, 3)),
        Int(TAG_P),
        "node 3 tag is P",
    )

    # "Body" (node 4) is Text
    assert_equal(
        Int(rt[].templates.node_kind(tmpl_id, 4)),
        Int(TNODE_TEXT),
        "node 4 is Text",
    )

    _teardown(rt)


# ══════════════════════════════════════════════════════════════════════════════
# Template Dynamic Slots — Dynamic and DynamicText nodes
# ══════════════════════════════════════════════════════════════════════════════


fn test_template_dynamic_slots() raises:
    var rt = _make_runtime()

    # Build: div > (dyntext[0], "static", dyn[0], dyntext[1])
    var b = TemplateBuilder("dynamic-slots")
    var div_idx = b.push_element(TAG_DIV, -1)
    _ = b.push_dynamic_text(UInt32(0), div_idx)
    _ = b.push_text("static", div_idx)
    _ = b.push_dynamic(UInt32(0), div_idx)
    _ = b.push_dynamic_text(UInt32(1), div_idx)

    var tmpl = b.build()
    var tmpl_id = rt[].templates.register(tmpl^)

    # Node count: div + dyntext0 + "static" + dyn0 + dyntext1 = 5
    assert_equal(rt[].templates.node_count(tmpl_id), 5, "5 nodes")

    # DynamicText node at index 1
    assert_equal(
        Int(rt[].templates.node_kind(tmpl_id, 1)),
        Int(TNODE_DYNAMIC_TEXT),
        "node 1 is DynamicText",
    )
    assert_equal(
        Int(rt[].templates.node_dynamic_index(tmpl_id, 1)),
        0,
        "dyntext 0 has index 0",
    )

    # Static text at index 2
    assert_equal(
        Int(rt[].templates.node_kind(tmpl_id, 2)),
        Int(TNODE_TEXT),
        "node 2 is Text",
    )

    # Dynamic node at index 3
    assert_equal(
        Int(rt[].templates.node_kind(tmpl_id, 3)),
        Int(TNODE_DYNAMIC),
        "node 3 is Dynamic",
    )
    assert_equal(
        Int(rt[].templates.node_dynamic_index(tmpl_id, 3)),
        0,
        "dynamic 0 has index 0",
    )

    # DynamicText at index 4
    assert_equal(
        Int(rt[].templates.node_kind(tmpl_id, 4)),
        Int(TNODE_DYNAMIC_TEXT),
        "node 4 is DynamicText",
    )
    assert_equal(
        Int(rt[].templates.node_dynamic_index(tmpl_id, 4)),
        1,
        "dyntext 1 has index 1",
    )

    # Slot counts
    assert_equal(
        rt[].templates.dynamic_node_count(tmpl_id), 1, "1 Dynamic node slot"
    )
    assert_equal(
        rt[].templates.dynamic_text_count(tmpl_id), 2, "2 DynamicText slots"
    )

    # div has 4 children
    assert_equal(
        rt[].templates.node_child_count(tmpl_id, 0), 4, "div has 4 children"
    )

    _teardown(rt)


# ══════════════════════════════════════════════════════════════════════════════
# Template Attributes — static and dynamic
# ══════════════════════════════════════════════════════════════════════════════


fn test_template_attributes() raises:
    var rt = _make_runtime()

    # Build: button with class="btn", id="submit", and one dynamic attr
    var b = TemplateBuilder("attrs")
    var btn_idx = b.push_element(TAG_BUTTON, -1)
    _ = b.push_text("Click me", btn_idx)

    b.push_static_attr(btn_idx, "class", "btn")
    b.push_static_attr(btn_idx, "id", "submit")
    b.push_dynamic_attr(btn_idx, UInt32(0))

    assert_equal(b.attr_count(), 3, "builder has 3 attrs")

    var tmpl = b.build()
    var tmpl_id = rt[].templates.register(tmpl^)

    # Total attributes
    assert_equal(
        rt[].templates.attr_total_count(tmpl_id),
        3,
        "template has 3 attrs total",
    )
    assert_equal(rt[].templates.static_attr_count(tmpl_id), 2, "2 static attrs")
    assert_equal(
        rt[].templates.dynamic_attr_count(tmpl_id), 1, "1 dynamic attr"
    )

    # Node-level attr count
    assert_equal(
        rt[].templates.node_attr_count(tmpl_id, 0), 3, "button has 3 attrs"
    )

    # Attr kinds
    assert_equal(
        Int(rt[].templates.get_attr_kind(tmpl_id, 0)),
        Int(TATTR_STATIC),
        "attr 0 is static",
    )
    assert_equal(
        Int(rt[].templates.get_attr_kind(tmpl_id, 1)),
        Int(TATTR_STATIC),
        "attr 1 is static",
    )
    assert_equal(
        Int(rt[].templates.get_attr_kind(tmpl_id, 2)),
        Int(TATTR_DYNAMIC),
        "attr 2 is dynamic",
    )
    assert_equal(
        Int(rt[].templates.get_attr_dynamic_index(tmpl_id, 2)),
        0,
        "dynamic attr index is 0",
    )

    # Node first attr
    assert_equal(
        Int(rt[].templates.node_first_attr(tmpl_id, 0)),
        0,
        "button first attr at index 0",
    )

    _teardown(rt)


# ══════════════════════════════════════════════════════════════════════════════
# Template Deduplication — same name returns same ID
# ══════════════════════════════════════════════════════════════════════════════


fn test_template_deduplication() raises:
    var rt = _make_runtime()

    # Register "counter" template
    var b1 = TemplateBuilder("counter")
    _ = b1.push_element(TAG_DIV, -1)
    var t1 = b1.build()
    var id1 = rt[].templates.register(t1^)

    # Register another template with the SAME name
    var b2 = TemplateBuilder("counter")
    _ = b2.push_element(TAG_SPAN, -1)  # different structure
    var t2 = b2.build()
    var id2 = rt[].templates.register(t2^)

    assert_equal(Int(id1), Int(id2), "same name -> same ID (deduplicated)")
    assert_equal(rt[].templates.count(), 1, "still only 1 template registered")

    # The original template's structure is preserved (div, not span)
    assert_equal(
        Int(rt[].templates.node_html_tag(id1, 0)),
        Int(TAG_DIV),
        "original template structure preserved",
    )

    # Register a different name
    var b3 = TemplateBuilder("todo-item")
    _ = b3.push_element(TAG_LI, -1)
    var t3 = b3.build()
    var id3 = rt[].templates.register(t3^)

    assert_equal(Int(id3), 1, "different name gets new ID")
    assert_equal(rt[].templates.count(), 2, "now 2 templates registered")

    _teardown(rt)


# ══════════════════════════════════════════════════════════════════════════════
# VNode Creation — basic kinds
# ══════════════════════════════════════════════════════════════════════════════


fn test_vnode_creation_basic_kinds() raises:
    var store = VNodeStore()

    # TemplateRef
    var tr = store.push(VNode.template_ref(UInt32(42)))
    assert_equal(Int(tr), 0, "first vnode at index 0")
    assert_equal(
        Int(store.kind(tr)), Int(VNODE_TEMPLATE_REF), "kind is TemplateRef"
    )
    assert_equal(Int(store.template_id(tr)), 42, "template_id is 42")
    assert_false(store.has_key(tr), "no key")
    assert_equal(store.dynamic_node_count(tr), 0, "0 dynamic nodes")
    assert_equal(store.dynamic_attr_count(tr), 0, "0 dynamic attrs")

    # Text
    var txt = store.push(VNode.text_node("hello world"))
    assert_equal(Int(txt), 1, "second vnode at index 1")
    assert_equal(Int(store.kind(txt)), Int(VNODE_TEXT), "kind is Text")

    # Placeholder
    var ph = store.push(VNode.placeholder(UInt32(99)))
    assert_equal(Int(ph), 2, "third vnode at index 2")
    assert_equal(
        Int(store.kind(ph)), Int(VNODE_PLACEHOLDER), "kind is Placeholder"
    )
    assert_equal(Int(store.element_id(ph)), 99, "element_id is 99")

    # Fragment
    var frag = store.push(VNode.fragment())
    assert_equal(Int(frag), 3, "fourth vnode at index 3")
    assert_equal(Int(store.kind(frag)), Int(VNODE_FRAGMENT), "kind is Fragment")
    assert_equal(store.fragment_child_count(frag), 0, "empty fragment")

    # Total count
    assert_equal(store.count(), 4, "4 vnodes in store")


# ══════════════════════════════════════════════════════════════════════════════
# VNode Dynamic Content — nodes and attrs on TemplateRef
# ══════════════════════════════════════════════════════════════════════════════


fn test_vnode_dynamic_content() raises:
    var store = VNodeStore()

    # Create a TemplateRef VNode
    var vn = store.push(VNode.template_ref(UInt32(0)))

    # Add dynamic text nodes
    store.push_dynamic_node(vn, DynamicNode.text_node("Count: 5"))
    store.push_dynamic_node(vn, DynamicNode.text_node("Total: 10"))

    assert_equal(store.dynamic_node_count(vn), 2, "2 dynamic nodes")
    assert_equal(
        Int(store.get_dynamic_node_kind(vn, 0)),
        Int(DNODE_TEXT),
        "dyn node 0 is Text",
    )
    assert_equal(
        Int(store.get_dynamic_node_kind(vn, 1)),
        Int(DNODE_TEXT),
        "dyn node 1 is Text",
    )

    # Add a dynamic placeholder
    store.push_dynamic_node(vn, DynamicNode.placeholder())
    assert_equal(store.dynamic_node_count(vn), 3, "3 dynamic nodes")
    assert_equal(
        Int(store.get_dynamic_node_kind(vn, 2)),
        Int(DNODE_PLACEHOLDER),
        "dyn node 2 is Placeholder",
    )

    # Add dynamic text attribute
    store.push_dynamic_attr(
        vn,
        DynamicAttr("class", AttributeValue.text("active"), UInt32(5)),
    )
    assert_equal(store.dynamic_attr_count(vn), 1, "1 dynamic attr")
    assert_equal(
        Int(store.get_dynamic_attr_kind(vn, 0)),
        Int(AVAL_TEXT),
        "attr 0 is text",
    )
    assert_equal(
        Int(store.get_dynamic_attr_element_id(vn, 0)),
        5,
        "attr 0 elem_id is 5",
    )


# ══════════════════════════════════════════════════════════════════════════════
# VNode Fragments — children
# ══════════════════════════════════════════════════════════════════════════════


fn test_vnode_fragments() raises:
    var store = VNodeStore()

    # Create child vnodes
    var txt1 = store.push(VNode.text_node("A"))
    var txt2 = store.push(VNode.text_node("B"))
    var txt3 = store.push(VNode.text_node("C"))

    # Create fragment and add children
    var frag = store.push(VNode.fragment())
    store.push_fragment_child(frag, txt1)
    store.push_fragment_child(frag, txt2)
    store.push_fragment_child(frag, txt3)

    assert_equal(store.fragment_child_count(frag), 3, "fragment has 3 children")
    assert_equal(
        Int(store.get_fragment_child(frag, 0)), 0, "child 0 is vnode 0"
    )
    assert_equal(
        Int(store.get_fragment_child(frag, 1)), 1, "child 1 is vnode 1"
    )
    assert_equal(
        Int(store.get_fragment_child(frag, 2)), 2, "child 2 is vnode 2"
    )

    # Verify children are text nodes
    assert_equal(Int(store.kind(txt1)), Int(VNODE_TEXT), "child 0 is Text")
    assert_equal(Int(store.kind(txt2)), Int(VNODE_TEXT), "child 1 is Text")
    assert_equal(Int(store.kind(txt3)), Int(VNODE_TEXT), "child 2 is Text")


# ══════════════════════════════════════════════════════════════════════════════
# VNode Keys — keyed TemplateRef
# ══════════════════════════════════════════════════════════════════════════════


fn test_vnode_keys() raises:
    var store = VNodeStore()

    # Unkeyed
    var vn1 = store.push(VNode.template_ref(UInt32(0)))
    assert_false(store.has_key(vn1), "unkeyed vnode has no key")

    # Keyed
    var vn2 = store.push(VNode.template_ref_keyed(UInt32(0), "item-42"))
    assert_true(store.has_key(vn2), "keyed vnode has key")

    # Both are TemplateRef
    assert_equal(
        Int(store.kind(vn1)), Int(VNODE_TEMPLATE_REF), "vn1 is TemplateRef"
    )
    assert_equal(
        Int(store.kind(vn2)), Int(VNODE_TEMPLATE_REF), "vn2 is TemplateRef"
    )


# ══════════════════════════════════════════════════════════════════════════════
# VNode Mixed Attributes — text, int, bool, event, none
# ══════════════════════════════════════════════════════════════════════════════


fn test_vnode_mixed_attributes() raises:
    var store = VNodeStore()
    var vn = store.push(VNode.template_ref(UInt32(0)))

    # Text attribute
    store.push_dynamic_attr(
        vn,
        DynamicAttr("class", AttributeValue.text("btn-primary"), UInt32(1)),
    )
    assert_equal(
        Int(store.get_dynamic_attr_kind(vn, 0)),
        Int(AVAL_TEXT),
        "attr 0 is text",
    )
    assert_equal(
        Int(store.get_dynamic_attr_element_id(vn, 0)), 1, "attr 0 elem_id"
    )

    # Int attribute
    store.push_dynamic_attr(
        vn,
        DynamicAttr("tabindex", AttributeValue.integer(Int64(3)), UInt32(2)),
    )
    assert_equal(
        Int(store.get_dynamic_attr_kind(vn, 1)),
        Int(AVAL_INT),
        "attr 1 is int",
    )
    assert_equal(
        Int(store.get_dynamic_attr_element_id(vn, 1)), 2, "attr 1 elem_id"
    )

    # Bool attribute
    store.push_dynamic_attr(
        vn,
        DynamicAttr("disabled", AttributeValue.boolean(True), UInt32(3)),
    )
    assert_equal(
        Int(store.get_dynamic_attr_kind(vn, 2)),
        Int(AVAL_BOOL),
        "attr 2 is bool",
    )
    assert_equal(
        Int(store.get_dynamic_attr_element_id(vn, 2)), 3, "attr 2 elem_id"
    )

    # Event handler
    store.push_dynamic_attr(
        vn,
        DynamicAttr("onclick", AttributeValue.event(UInt32(77)), UInt32(4)),
    )
    assert_equal(
        Int(store.get_dynamic_attr_kind(vn, 3)),
        Int(AVAL_EVENT),
        "attr 3 is event",
    )
    assert_equal(
        Int(store.get_dynamic_attr_element_id(vn, 3)), 4, "attr 3 elem_id"
    )

    # None (removal)
    store.push_dynamic_attr(
        vn,
        DynamicAttr("hidden", AttributeValue.none(), UInt32(5)),
    )
    assert_equal(
        Int(store.get_dynamic_attr_kind(vn, 4)),
        Int(AVAL_NONE),
        "attr 4 is none",
    )
    assert_equal(
        Int(store.get_dynamic_attr_element_id(vn, 4)), 5, "attr 4 elem_id"
    )

    assert_equal(store.dynamic_attr_count(vn), 5, "5 dynamic attrs total")


# ══════════════════════════════════════════════════════════════════════════════
# VNode Store Lifecycle — create, populate, clear, repopulate
# ══════════════════════════════════════════════════════════════════════════════


fn test_vnode_store_lifecycle() raises:
    var store = VNodeStore()

    # Add some nodes
    _ = store.push(VNode.text_node("A"))
    _ = store.push(VNode.text_node("B"))
    _ = store.push(VNode.placeholder(UInt32(1)))
    assert_equal(store.count(), 3, "3 vnodes before clear")

    # Clear
    store.clear()
    assert_equal(store.count(), 0, "0 vnodes after clear")

    # Repopulate
    var idx = store.push(VNode.template_ref(UInt32(5)))
    assert_equal(Int(idx), 0, "indices restart at 0 after clear")
    assert_equal(store.count(), 1, "1 vnode after repopulate")
    assert_equal(
        Int(store.kind(idx)),
        Int(VNODE_TEMPLATE_REF),
        "repopulated vnode is TemplateRef",
    )


# ══════════════════════════════════════════════════════════════════════════════
# Template Builder — pre-build queries
# ══════════════════════════════════════════════════════════════════════════════


fn test_builder_pre_build_queries() raises:
    var b = TemplateBuilder("query-test")

    # Empty builder
    assert_equal(b.node_count(), 0, "empty builder has 0 nodes")
    assert_equal(b.root_count(), 0, "empty builder has 0 roots")
    assert_equal(b.attr_count(), 0, "empty builder has 0 attrs")

    # Add nodes and check counts incrementally
    var r1 = b.push_element(TAG_UL, -1)
    assert_equal(b.node_count(), 1, "1 node after push_element")
    assert_equal(b.root_count(), 1, "1 root after root push")

    _ = b.push_element(TAG_LI, r1)
    assert_equal(b.node_count(), 2, "2 nodes")
    assert_equal(b.root_count(), 1, "still 1 root (child added)")

    _ = b.push_text("item", 1)
    assert_equal(b.node_count(), 3, "3 nodes")

    # Add another root
    _ = b.push_element(TAG_P, -1)
    assert_equal(b.root_count(), 2, "2 roots now")

    # Add an attribute
    b.push_static_attr(r1, "class", "list")
    assert_equal(b.attr_count(), 1, "1 attr")

    b.push_dynamic_attr(r1, UInt32(0))
    assert_equal(b.attr_count(), 2, "2 attrs")


# ══════════════════════════════════════════════════════════════════════════════
# Complex Template — counter-like structure
# ══════════════════════════════════════════════════════════════════════════════


fn test_complex_template_counter() raises:
    var rt = _make_runtime()

    # Build a counter template:
    # div.counter
    #   h1 > dyntext[0]  ("Count: N")
    #   button > "+"      (onclick = dynamic attr 0)
    #   button > "-"      (onclick = dynamic attr 1)
    var b = TemplateBuilder("counter")
    var div = b.push_element(TAG_DIV, -1)
    b.push_static_attr(div, "class", "counter")

    var h1 = b.push_element(TAG_H1, div)
    _ = b.push_dynamic_text(UInt32(0), h1)

    var btn1 = b.push_element(TAG_BUTTON, div)
    _ = b.push_text("+", btn1)
    b.push_dynamic_attr(btn1, UInt32(0))

    var btn2 = b.push_element(TAG_BUTTON, div)
    _ = b.push_text("-", btn2)
    b.push_dynamic_attr(btn2, UInt32(1))

    var tmpl = b.build()
    var tmpl_id = rt[].templates.register(tmpl^)

    # Verify structure
    # Nodes: div(0), h1(1), dyntext(2), btn1(3), "+"(4), btn2(5), "-"(6) = 7
    assert_equal(
        rt[].templates.node_count(tmpl_id), 7, "7 nodes in counter template"
    )
    assert_equal(rt[].templates.root_count(tmpl_id), 1, "1 root (div)")

    # div children: h1, btn1, btn2
    assert_equal(
        rt[].templates.node_child_count(tmpl_id, 0), 3, "div has 3 children"
    )
    assert_equal(
        Int(rt[].templates.node_child_at(tmpl_id, 0, 0)),
        1,
        "div child 0 = h1",
    )
    assert_equal(
        Int(rt[].templates.node_child_at(tmpl_id, 0, 1)),
        3,
        "div child 1 = btn1",
    )
    assert_equal(
        Int(rt[].templates.node_child_at(tmpl_id, 0, 2)),
        5,
        "div child 2 = btn2",
    )

    # h1 has 1 child: dyntext[0]
    assert_equal(
        rt[].templates.node_child_count(tmpl_id, 1), 1, "h1 has 1 child"
    )
    assert_equal(
        Int(rt[].templates.node_kind(tmpl_id, 2)),
        Int(TNODE_DYNAMIC_TEXT),
        "h1 child is DynamicText",
    )
    assert_equal(
        Int(rt[].templates.node_dynamic_index(tmpl_id, 2)),
        0,
        "dyntext index 0",
    )

    # btn1 has 1 child: "+"
    assert_equal(
        Int(rt[].templates.node_html_tag(tmpl_id, 3)),
        Int(TAG_BUTTON),
        "btn1 is BUTTON",
    )
    assert_equal(
        rt[].templates.node_child_count(tmpl_id, 3), 1, "btn1 has 1 child"
    )
    assert_equal(
        Int(rt[].templates.node_kind(tmpl_id, 4)),
        Int(TNODE_TEXT),
        "btn1 child is Text",
    )

    # btn2 has 1 child: "-"
    assert_equal(
        Int(rt[].templates.node_html_tag(tmpl_id, 5)),
        Int(TAG_BUTTON),
        "btn2 is BUTTON",
    )
    assert_equal(
        rt[].templates.node_child_count(tmpl_id, 5), 1, "btn2 has 1 child"
    )
    assert_equal(
        Int(rt[].templates.node_kind(tmpl_id, 6)),
        Int(TNODE_TEXT),
        "btn2 child is Text",
    )

    # Attribute counts
    assert_equal(
        rt[].templates.static_attr_count(tmpl_id), 1, "1 static attr (class)"
    )
    assert_equal(
        rt[].templates.dynamic_attr_count(tmpl_id),
        2,
        "2 dynamic attrs (onclick x2)",
    )
    assert_equal(rt[].templates.attr_total_count(tmpl_id), 3, "3 attrs total")

    # Dynamic slot counts
    assert_equal(
        rt[].templates.dynamic_text_count(tmpl_id), 1, "1 dynamic text slot"
    )
    assert_equal(
        rt[].templates.dynamic_node_count(tmpl_id), 0, "0 dynamic node slots"
    )

    # Now create a VNode that instantiates this template
    var store = VNodeStore()
    var vn = store.push(VNode.template_ref(tmpl_id))

    # Fill dynamic text: "Count: 5"
    store.push_dynamic_node(vn, DynamicNode.text_node("Count: 5"))

    # Fill dynamic attrs: onclick handlers
    store.push_dynamic_attr(
        vn,
        DynamicAttr("onclick", AttributeValue.event(UInt32(1)), UInt32(3)),
    )
    store.push_dynamic_attr(
        vn,
        DynamicAttr("onclick", AttributeValue.event(UInt32(2)), UInt32(5)),
    )

    assert_equal(store.dynamic_node_count(vn), 1, "vnode has 1 dynamic node")
    assert_equal(store.dynamic_attr_count(vn), 2, "vnode has 2 dynamic attrs")
    assert_equal(
        Int(store.template_id(vn)),
        Int(tmpl_id),
        "vnode references counter template",
    )

    _teardown(rt)


# ══════════════════════════════════════════════════════════════════════════════
# Multiple Templates — different structures in one runtime
# ══════════════════════════════════════════════════════════════════════════════


fn test_multiple_templates_in_one_runtime() raises:
    var rt = _make_runtime()

    # Template 1: simple div > "Hello"
    var b1 = TemplateBuilder("hello")
    var div1 = b1.push_element(TAG_DIV, -1)
    _ = b1.push_text("Hello", div1)
    var t1 = b1.build()
    var id1 = rt[].templates.register(t1^)

    # Template 2: ul > li > "Item 1", li > "Item 2"
    var b2 = TemplateBuilder("list")
    var ul = b2.push_element(TAG_UL, -1)
    var li1 = b2.push_element(TAG_LI, ul)
    _ = b2.push_text("Item 1", li1)
    var li2 = b2.push_element(TAG_LI, ul)
    _ = b2.push_text("Item 2", li2)
    var t2 = b2.build()
    var id2 = rt[].templates.register(t2^)

    # Template 3: form > input + button
    var b3 = TemplateBuilder("form")
    var form = b3.push_element(TAG_FORM, -1)
    _ = b3.push_element(TAG_INPUT, form)
    var submit_btn = b3.push_element(TAG_BUTTON, form)
    _ = b3.push_text("Submit", submit_btn)
    var t3 = b3.build()
    var id3 = rt[].templates.register(t3^)

    # Verify IDs
    assert_equal(Int(id1), 0, "hello gets ID 0")
    assert_equal(Int(id2), 1, "list gets ID 1")
    assert_equal(Int(id3), 2, "form gets ID 2")
    assert_equal(rt[].templates.count(), 3, "3 templates registered")

    # Cross-template queries
    assert_equal(rt[].templates.node_count(id1), 2, "hello has 2 nodes")
    assert_equal(rt[].templates.node_count(id2), 5, "list has 5 nodes")
    assert_equal(rt[].templates.node_count(id3), 4, "form has 4 nodes")

    assert_equal(
        Int(rt[].templates.node_html_tag(id1, 0)),
        Int(TAG_DIV),
        "hello root is div",
    )
    assert_equal(
        Int(rt[].templates.node_html_tag(id2, 0)),
        Int(TAG_UL),
        "list root is ul",
    )
    assert_equal(
        Int(rt[].templates.node_html_tag(id3, 0)),
        Int(TAG_FORM),
        "form root is form",
    )

    # Verify list children
    assert_equal(
        rt[].templates.node_child_count(id2, 0), 2, "ul has 2 children"
    )
    assert_equal(
        Int(rt[].templates.node_html_tag(id2, 1)),
        Int(TAG_LI),
        "first child is li",
    )
    assert_equal(
        Int(rt[].templates.node_html_tag(id2, 3)),
        Int(TAG_LI),
        "second child is li",
    )

    # Verify form children
    assert_equal(
        rt[].templates.node_child_count(id3, 0), 2, "form has 2 children"
    )
    assert_equal(
        Int(rt[].templates.node_html_tag(id3, 1)),
        Int(TAG_INPUT),
        "first child is input",
    )
    assert_equal(
        Int(rt[].templates.node_html_tag(id3, 2)),
        Int(TAG_BUTTON),
        "second child is button",
    )

    _teardown(rt)


# ══════════════════════════════════════════════════════════════════════════════
# Builder Reset — builder is empty after build()
# ══════════════════════════════════════════════════════════════════════════════


fn test_builder_reset_after_build() raises:
    var rt = _make_runtime()

    var b = TemplateBuilder("reset-test")
    _ = b.push_element(TAG_DIV, -1)
    _ = b.push_text("hello", 0)
    b.push_static_attr(0, "class", "x")

    assert_equal(b.node_count(), 2, "2 nodes before build")
    assert_equal(b.root_count(), 1, "1 root before build")
    assert_equal(b.attr_count(), 1, "1 attr before build")

    var tmpl = b.build()
    _ = rt[].templates.register(tmpl^)

    # After build, builder should be reset/empty
    assert_equal(b.node_count(), 0, "0 nodes after build")
    assert_equal(b.root_count(), 0, "0 roots after build")
    assert_equal(b.attr_count(), 0, "0 attrs after build")

    # Can reuse the builder for a new template
    _ = b.push_element(TAG_SPAN, -1)
    assert_equal(b.node_count(), 1, "1 node after reuse")

    var tmpl2 = b.build()
    var id2 = rt[].templates.register(tmpl2^)
    # The name "reset-test" is already registered, so this deduplicates
    assert_equal(Int(id2), 0, "same name -> same ID (deduped after reuse)")

    _teardown(rt)

import { writeStringStruct } from "../runtime/strings.ts";
import type { WasmExports } from "../runtime/types.ts";
import { assert, suite } from "./harness.ts";

// ── Constants (must match Mojo side) ────────────────────────────────────────

// TemplateNode kinds
const TNODE_ELEMENT = 0;
const TNODE_TEXT = 1;
const TNODE_DYNAMIC = 2;
const TNODE_DYNAMIC_TEXT = 3;

// TemplateAttribute kinds
const TATTR_STATIC = 0;
const TATTR_DYNAMIC = 1;

// VNode kinds
const VNODE_TEMPLATE_REF = 0;
const VNODE_TEXT = 1;
const VNODE_PLACEHOLDER = 2;
const VNODE_FRAGMENT = 3;

// AttributeValue kinds
const AVAL_TEXT = 0;
const AVAL_INT = 1;
const AVAL_FLOAT = 2;
const AVAL_BOOL = 3;
const AVAL_EVENT = 4;
const AVAL_NONE = 5;

// DynamicNode kinds
const DNODE_TEXT = 0;
const DNODE_PLACEHOLDER = 1;

// HTML tag constants
const TAG_DIV = 0;
const TAG_SPAN = 1;
const TAG_P = 2;
const TAG_H1 = 10;
const TAG_H2 = 11;
const TAG_H3 = 12;
const TAG_UL = 16;
const TAG_OL = 17;
const TAG_LI = 18;
const TAG_BUTTON = 19;
const TAG_INPUT = 20;
const TAG_FORM = 21;
const TAG_A = 26;
const TAG_IMG = 27;
const TAG_TABLE = 28;
const TAG_TR = 31;
const TAG_TD = 32;
const TAG_TH = 33;
const TAG_UNKNOWN = 255;

// ── Helpers ─────────────────────────────────────────────────────────────────

/**
 * Create a builder, returning its pointer. Caller must destroy.
 */
function createBuilder(fns: WasmExports, name: string): bigint {
	const namePtr = writeStringStruct(name);
	return fns.tmpl_builder_create(namePtr);
}

/**
 * Build a template from a builder and register it in the runtime.
 * Returns the template ID.
 */
function registerBuilder(
	fns: WasmExports,
	rtPtr: bigint,
	builderPtr: bigint,
): number {
	return fns.tmpl_builder_register(rtPtr, builderPtr);
}

/**
 * Push a text node into a builder with a JS string.
 */
function builderPushText(
	fns: WasmExports,
	builderPtr: bigint,
	text: string,
	parent: number,
): number {
	const textPtr = writeStringStruct(text);
	return fns.tmpl_builder_push_text(builderPtr, textPtr, parent);
}

/**
 * Push a static attribute onto a builder node.
 */
function builderPushStaticAttr(
	fns: WasmExports,
	builderPtr: bigint,
	nodeIndex: number,
	name: string,
	value: string,
): void {
	const namePtr = writeStringStruct(name);
	const valuePtr = writeStringStruct(value);
	fns.tmpl_builder_push_static_attr(builderPtr, nodeIndex, namePtr, valuePtr);
}

/**
 * Check if a template name is registered.
 */
function tmplContainsName(
	fns: WasmExports,
	rtPtr: bigint,
	name: string,
): boolean {
	const namePtr = writeStringStruct(name);
	return fns.tmpl_contains_name(rtPtr, namePtr) === 1;
}

/**
 * Find a template by name.
 */
function tmplFindByName(fns: WasmExports, rtPtr: bigint, name: string): number {
	const namePtr = writeStringStruct(name);
	return fns.tmpl_find_by_name(rtPtr, namePtr);
}

/**
 * Push a text VNode into a store.
 */
function vnodePushText(
	fns: WasmExports,
	storePtr: bigint,
	text: string,
): number {
	const textPtr = writeStringStruct(text);
	return fns.vnode_push_text(storePtr, textPtr);
}

/**
 * Push a keyed TemplateRef VNode into a store.
 */
function vnodePushTemplateRefKeyed(
	fns: WasmExports,
	storePtr: bigint,
	tmplId: number,
	key: string,
): number {
	const keyPtr = writeStringStruct(key);
	return fns.vnode_push_template_ref_keyed(storePtr, tmplId, keyPtr);
}

/**
 * Push a dynamic text node onto a VNode.
 */
function vnodePushDynamicTextNode(
	fns: WasmExports,
	storePtr: bigint,
	vnodeIndex: number,
	text: string,
): void {
	const textPtr = writeStringStruct(text);
	fns.vnode_push_dynamic_text_node(storePtr, vnodeIndex, textPtr);
}

/**
 * Push a dynamic text attribute onto a VNode.
 */
function vnodePushDynamicAttrText(
	fns: WasmExports,
	storePtr: bigint,
	vnodeIndex: number,
	name: string,
	value: string,
	elemId: number,
): void {
	const namePtr = writeStringStruct(name);
	const valuePtr = writeStringStruct(value);
	fns.vnode_push_dynamic_attr_text(
		storePtr,
		vnodeIndex,
		namePtr,
		valuePtr,
		elemId,
	);
}

/**
 * Push a dynamic int attribute onto a VNode.
 */
function vnodePushDynamicAttrInt(
	fns: WasmExports,
	storePtr: bigint,
	vnodeIndex: number,
	name: string,
	value: number,
	elemId: number,
): void {
	const namePtr = writeStringStruct(name);
	fns.vnode_push_dynamic_attr_int(storePtr, vnodeIndex, namePtr, value, elemId);
}

/**
 * Push a dynamic bool attribute onto a VNode.
 */
function vnodePushDynamicAttrBool(
	fns: WasmExports,
	storePtr: bigint,
	vnodeIndex: number,
	name: string,
	value: boolean,
	elemId: number,
): void {
	const namePtr = writeStringStruct(name);
	fns.vnode_push_dynamic_attr_bool(
		storePtr,
		vnodeIndex,
		namePtr,
		value ? 1 : 0,
		elemId,
	);
}

/**
 * Push a dynamic event attribute onto a VNode.
 */
function vnodePushDynamicAttrEvent(
	fns: WasmExports,
	storePtr: bigint,
	vnodeIndex: number,
	name: string,
	handlerId: number,
	elemId: number,
): void {
	const namePtr = writeStringStruct(name);
	fns.vnode_push_dynamic_attr_event(
		storePtr,
		vnodeIndex,
		namePtr,
		handlerId,
		elemId,
	);
}

/**
 * Push a dynamic none attribute onto a VNode.
 */
function vnodePushDynamicAttrNone(
	fns: WasmExports,
	storePtr: bigint,
	vnodeIndex: number,
	name: string,
	elemId: number,
): void {
	const namePtr = writeStringStruct(name);
	fns.vnode_push_dynamic_attr_none(storePtr, vnodeIndex, namePtr, elemId);
}

// ── Test Suites ─────────────────────────────────────────────────────────────

export function testTemplates(fns: WasmExports): void {
	testTemplateBuilder(fns);
	testTemplateRegistry(fns);
	testTemplateStructure(fns);
	testTemplateDynamicSlots(fns);
	testTemplateAttributes(fns);
	testTemplateDeduplication(fns);
	testVNodeCreation(fns);
	testVNodeDynamicContent(fns);
	testVNodeFragments(fns);
	testVNodeKeys(fns);
	testVNodeMixedAttributes(fns);
	testVNodeStoreLifecycle(fns);
	testBuilderQueryPreBuild(fns);
	testComplexTemplate(fns);
	testMultipleTemplatesInOneRuntime(fns);
	testBuilderResetAfterBuild(fns);
}

// ── Template Builder Basic ──────────────────────────────────────────────────

function testTemplateBuilder(fns: WasmExports): void {
	suite("Template Builder — basic lifecycle");

	const rtPtr = fns.runtime_create();

	// Create builder
	const b = createBuilder(fns, "test-basic");

	// Push a single div root
	const divIdx = fns.tmpl_builder_push_element(b, TAG_DIV, -1);
	assert(divIdx, 0, "first element is at index 0");

	// Push a child span inside the div
	const spanIdx = fns.tmpl_builder_push_element(b, TAG_SPAN, divIdx);
	assert(spanIdx, 1, "second element is at index 1");

	// Push a text node inside the span
	const textIdx = builderPushText(fns, b, "hello", spanIdx);
	assert(textIdx, 2, "text node is at index 2");

	// Check builder counts
	assert(fns.tmpl_builder_node_count(b), 3, "builder has 3 nodes");
	assert(fns.tmpl_builder_root_count(b), 1, "builder has 1 root");

	// Register
	const tmplId = registerBuilder(fns, rtPtr, b);
	assert(tmplId, 0, "first template gets ID 0");
	assert(fns.tmpl_count(rtPtr), 1, "1 template registered");

	fns.tmpl_builder_destroy(b);
	fns.runtime_destroy(rtPtr);
}

// ── Template Registry ───────────────────────────────────────────────────────

function testTemplateRegistry(fns: WasmExports): void {
	suite("Template Registry — register and query");

	const rtPtr = fns.runtime_create();

	// Register first template
	const b1 = createBuilder(fns, "alpha");
	fns.tmpl_builder_push_element(b1, TAG_DIV, -1);
	const id1 = registerBuilder(fns, rtPtr, b1);
	assert(id1, 0, "alpha gets ID 0");

	// Register second template
	const b2 = createBuilder(fns, "beta");
	fns.tmpl_builder_push_element(b2, TAG_SPAN, -1);
	const id2 = registerBuilder(fns, rtPtr, b2);
	assert(id2, 1, "beta gets ID 1");

	assert(fns.tmpl_count(rtPtr), 2, "2 templates registered");

	// Look up by name
	assert(tmplContainsName(fns, rtPtr, "alpha"), true, "contains 'alpha'");
	assert(tmplContainsName(fns, rtPtr, "beta"), true, "contains 'beta'");
	assert(
		tmplContainsName(fns, rtPtr, "gamma"),
		false,
		"does not contain 'gamma'",
	);

	assert(tmplFindByName(fns, rtPtr, "alpha"), 0, "find 'alpha' → 0");
	assert(tmplFindByName(fns, rtPtr, "beta"), 1, "find 'beta' → 1");
	assert(tmplFindByName(fns, rtPtr, "gamma"), -1, "find 'gamma' → -1");

	fns.tmpl_builder_destroy(b1);
	fns.tmpl_builder_destroy(b2);
	fns.runtime_destroy(rtPtr);
}

// ── Template Structure Queries ──────────────────────────────────────────────

function testTemplateStructure(fns: WasmExports): void {
	suite("Template Structure — node queries");

	const rtPtr = fns.runtime_create();

	// Build: div > (h1 > "Title", p > "Body")
	const b = createBuilder(fns, "structure");
	const divIdx = fns.tmpl_builder_push_element(b, TAG_DIV, -1);
	const h1Idx = fns.tmpl_builder_push_element(b, TAG_H1, divIdx);
	builderPushText(fns, b, "Title", h1Idx);
	const pIdx = fns.tmpl_builder_push_element(b, TAG_P, divIdx);
	builderPushText(fns, b, "Body", pIdx);

	const tmplId = registerBuilder(fns, rtPtr, b);

	// Roots
	assert(fns.tmpl_root_count(rtPtr, tmplId), 1, "1 root");
	assert(fns.tmpl_get_root_index(rtPtr, tmplId, 0), 0, "root is node 0");

	// Total nodes: div + h1 + "Title" + p + "Body" = 5
	assert(fns.tmpl_node_count(rtPtr, tmplId), 5, "5 nodes total");

	// div (node 0) is Element with TAG_DIV, 2 children
	assert(
		fns.tmpl_node_kind(rtPtr, tmplId, 0),
		TNODE_ELEMENT,
		"node 0 is Element",
	);
	assert(fns.tmpl_node_tag(rtPtr, tmplId, 0), TAG_DIV, "node 0 tag is DIV");
	assert(fns.tmpl_node_child_count(rtPtr, tmplId, 0), 2, "div has 2 children");
	assert(
		fns.tmpl_node_child_at(rtPtr, tmplId, 0, 0),
		1,
		"div child 0 is node 1 (h1)",
	);
	assert(
		fns.tmpl_node_child_at(rtPtr, tmplId, 0, 1),
		3,
		"div child 1 is node 3 (p)",
	);

	// h1 (node 1)
	assert(
		fns.tmpl_node_kind(rtPtr, tmplId, 1),
		TNODE_ELEMENT,
		"node 1 is Element",
	);
	assert(fns.tmpl_node_tag(rtPtr, tmplId, 1), TAG_H1, "node 1 tag is H1");
	assert(fns.tmpl_node_child_count(rtPtr, tmplId, 1), 1, "h1 has 1 child");

	// "Title" (node 2) is Text
	assert(fns.tmpl_node_kind(rtPtr, tmplId, 2), TNODE_TEXT, "node 2 is Text");
	assert(
		fns.tmpl_node_child_count(rtPtr, tmplId, 2),
		0,
		"text node has 0 children",
	);

	// p (node 3)
	assert(
		fns.tmpl_node_kind(rtPtr, tmplId, 3),
		TNODE_ELEMENT,
		"node 3 is Element",
	);
	assert(fns.tmpl_node_tag(rtPtr, tmplId, 3), TAG_P, "node 3 tag is P");

	// "Body" (node 4) is Text
	assert(fns.tmpl_node_kind(rtPtr, tmplId, 4), TNODE_TEXT, "node 4 is Text");

	fns.tmpl_builder_destroy(b);
	fns.runtime_destroy(rtPtr);
}

// ── Dynamic Slots ───────────────────────────────────────────────────────────

function testTemplateDynamicSlots(fns: WasmExports): void {
	suite("Template Dynamic Slots — Dynamic and DynamicText nodes");

	const rtPtr = fns.runtime_create();

	// Build: div > (dyntext[0], "static", dyn[0], dyntext[1])
	const b = createBuilder(fns, "dynamic-slots");
	const divIdx = fns.tmpl_builder_push_element(b, TAG_DIV, -1);
	const dt0 = fns.tmpl_builder_push_dynamic_text(b, 0, divIdx);
	const txt = builderPushText(fns, b, "static", divIdx);
	const dn0 = fns.tmpl_builder_push_dynamic(b, 0, divIdx);
	const dt1 = fns.tmpl_builder_push_dynamic_text(b, 1, divIdx);

	const tmplId = registerBuilder(fns, rtPtr, b);

	// Node count: div + dyntext0 + "static" + dyn0 + dyntext1 = 5
	assert(fns.tmpl_node_count(rtPtr, tmplId), 5, "5 nodes");

	// DynamicText node at index 1
	assert(
		fns.tmpl_node_kind(rtPtr, tmplId, 1),
		TNODE_DYNAMIC_TEXT,
		"node 1 is DynamicText",
	);
	assert(
		fns.tmpl_node_dynamic_index(rtPtr, tmplId, 1),
		0,
		"dyntext 0 has index 0",
	);

	// Static text at index 2
	assert(fns.tmpl_node_kind(rtPtr, tmplId, 2), TNODE_TEXT, "node 2 is Text");

	// Dynamic node at index 3
	assert(
		fns.tmpl_node_kind(rtPtr, tmplId, 3),
		TNODE_DYNAMIC,
		"node 3 is Dynamic",
	);
	assert(
		fns.tmpl_node_dynamic_index(rtPtr, tmplId, 3),
		0,
		"dynamic 0 has index 0",
	);

	// DynamicText at index 4
	assert(
		fns.tmpl_node_kind(rtPtr, tmplId, 4),
		TNODE_DYNAMIC_TEXT,
		"node 4 is DynamicText",
	);
	assert(
		fns.tmpl_node_dynamic_index(rtPtr, tmplId, 4),
		1,
		"dyntext 1 has index 1",
	);

	// Slot counts
	assert(fns.tmpl_dynamic_node_count(rtPtr, tmplId), 1, "1 Dynamic node slot");
	assert(fns.tmpl_dynamic_text_count(rtPtr, tmplId), 2, "2 DynamicText slots");

	// div has 4 children
	assert(fns.tmpl_node_child_count(rtPtr, tmplId, 0), 4, "div has 4 children");

	fns.tmpl_builder_destroy(b);
	fns.runtime_destroy(rtPtr);
}

// ── Template Attributes ─────────────────────────────────────────────────────

function testTemplateAttributes(fns: WasmExports): void {
	suite("Template Attributes — static and dynamic");

	const rtPtr = fns.runtime_create();

	// Build: button with class="btn", id="submit", and one dynamic attr
	const b = createBuilder(fns, "attrs");
	const btnIdx = fns.tmpl_builder_push_element(b, TAG_BUTTON, -1);
	builderPushText(fns, b, "Click me", btnIdx);

	builderPushStaticAttr(fns, b, btnIdx, "class", "btn");
	builderPushStaticAttr(fns, b, btnIdx, "id", "submit");
	fns.tmpl_builder_push_dynamic_attr(b, btnIdx, 0); // dynamic onclick

	assert(fns.tmpl_builder_attr_count(b), 3, "builder has 3 attrs");

	const tmplId = registerBuilder(fns, rtPtr, b);

	// Total attributes
	assert(
		fns.tmpl_attr_total_count(rtPtr, tmplId),
		3,
		"template has 3 attrs total",
	);
	assert(fns.tmpl_static_attr_count(rtPtr, tmplId), 2, "2 static attrs");
	assert(fns.tmpl_dynamic_attr_count(rtPtr, tmplId), 1, "1 dynamic attr");

	// Node-level attr count
	assert(fns.tmpl_node_attr_count(rtPtr, tmplId, 0), 3, "button has 3 attrs");

	// Attr kinds
	assert(
		fns.tmpl_attr_kind(rtPtr, tmplId, 0),
		TATTR_STATIC,
		"attr 0 is static",
	);
	assert(
		fns.tmpl_attr_kind(rtPtr, tmplId, 1),
		TATTR_STATIC,
		"attr 1 is static",
	);
	assert(
		fns.tmpl_attr_kind(rtPtr, tmplId, 2),
		TATTR_DYNAMIC,
		"attr 2 is dynamic",
	);
	assert(
		fns.tmpl_attr_dynamic_index(rtPtr, tmplId, 2),
		0,
		"dynamic attr index is 0",
	);

	// Node first attr
	assert(
		fns.tmpl_node_first_attr(rtPtr, tmplId, 0),
		0,
		"button first attr at index 0",
	);

	fns.tmpl_builder_destroy(b);
	fns.runtime_destroy(rtPtr);
}

// ── Template Deduplication ──────────────────────────────────────────────────

function testTemplateDeduplication(fns: WasmExports): void {
	suite("Template Deduplication — same name returns same ID");

	const rtPtr = fns.runtime_create();

	// Register "counter" template
	const b1 = createBuilder(fns, "counter");
	fns.tmpl_builder_push_element(b1, TAG_DIV, -1);
	const id1 = registerBuilder(fns, rtPtr, b1);

	// Register another template with the SAME name
	const b2 = createBuilder(fns, "counter");
	fns.tmpl_builder_push_element(b2, TAG_SPAN, -1); // different structure
	const id2 = registerBuilder(fns, rtPtr, b2);

	assert(id1, id2, "same name → same ID (deduplicated)");
	assert(fns.tmpl_count(rtPtr), 1, "still only 1 template registered");

	// The original template's structure is preserved (div, not span)
	assert(
		fns.tmpl_node_tag(rtPtr, id1, 0),
		TAG_DIV,
		"original template structure preserved",
	);

	// Register a different name
	const b3 = createBuilder(fns, "todo-item");
	fns.tmpl_builder_push_element(b3, TAG_LI, -1);
	const id3 = registerBuilder(fns, rtPtr, b3);

	assert(id3, 1, "different name gets new ID");
	assert(fns.tmpl_count(rtPtr), 2, "now 2 templates registered");

	fns.tmpl_builder_destroy(b1);
	fns.tmpl_builder_destroy(b2);
	fns.tmpl_builder_destroy(b3);
	fns.runtime_destroy(rtPtr);
}

// ── VNode Creation ──────────────────────────────────────────────────────────

function testVNodeCreation(fns: WasmExports): void {
	suite("VNode Creation — basic kinds");

	const storePtr = fns.vnode_store_create();

	// TemplateRef
	const tr = fns.vnode_push_template_ref(storePtr, 42);
	assert(tr, 0, "first vnode at index 0");
	assert(
		fns.vnode_kind(storePtr, tr),
		VNODE_TEMPLATE_REF,
		"kind is TemplateRef",
	);
	assert(fns.vnode_template_id(storePtr, tr), 42, "template_id is 42");
	assert(fns.vnode_has_key(storePtr, tr), 0, "no key");
	assert(fns.vnode_dynamic_node_count(storePtr, tr), 0, "0 dynamic nodes");
	assert(fns.vnode_dynamic_attr_count(storePtr, tr), 0, "0 dynamic attrs");

	// Text
	const txt = vnodePushText(fns, storePtr, "hello world");
	assert(txt, 1, "second vnode at index 1");
	assert(fns.vnode_kind(storePtr, txt), VNODE_TEXT, "kind is Text");

	// Placeholder
	const ph = fns.vnode_push_placeholder(storePtr, 99);
	assert(ph, 2, "third vnode at index 2");
	assert(
		fns.vnode_kind(storePtr, ph),
		VNODE_PLACEHOLDER,
		"kind is Placeholder",
	);
	assert(fns.vnode_element_id(storePtr, ph), 99, "element_id is 99");

	// Fragment
	const frag = fns.vnode_push_fragment(storePtr);
	assert(frag, 3, "fourth vnode at index 3");
	assert(fns.vnode_kind(storePtr, frag), VNODE_FRAGMENT, "kind is Fragment");
	assert(fns.vnode_fragment_child_count(storePtr, frag), 0, "empty fragment");

	// Total count
	assert(fns.vnode_count(storePtr), 4, "4 vnodes in store");

	fns.vnode_store_destroy(storePtr);
}

// ── VNode Dynamic Content ───────────────────────────────────────────────────

function testVNodeDynamicContent(fns: WasmExports): void {
	suite("VNode Dynamic Content — nodes and attrs on TemplateRef");

	const storePtr = fns.vnode_store_create();

	// Create a TemplateRef VNode
	const vn = fns.vnode_push_template_ref(storePtr, 0);

	// Add dynamic text nodes
	vnodePushDynamicTextNode(fns, storePtr, vn, "Count: 5");
	vnodePushDynamicTextNode(fns, storePtr, vn, "Total: 10");

	assert(fns.vnode_dynamic_node_count(storePtr, vn), 2, "2 dynamic nodes");
	assert(
		fns.vnode_get_dynamic_node_kind(storePtr, vn, 0),
		DNODE_TEXT,
		"dyn node 0 is Text",
	);
	assert(
		fns.vnode_get_dynamic_node_kind(storePtr, vn, 1),
		DNODE_TEXT,
		"dyn node 1 is Text",
	);

	// Add a dynamic placeholder
	fns.vnode_push_dynamic_placeholder(storePtr, vn);
	assert(fns.vnode_dynamic_node_count(storePtr, vn), 3, "3 dynamic nodes");
	assert(
		fns.vnode_get_dynamic_node_kind(storePtr, vn, 2),
		DNODE_PLACEHOLDER,
		"dyn node 2 is Placeholder",
	);

	// Add dynamic text attribute
	vnodePushDynamicAttrText(fns, storePtr, vn, "class", "active", 5);
	assert(fns.vnode_dynamic_attr_count(storePtr, vn), 1, "1 dynamic attr");
	assert(
		fns.vnode_get_dynamic_attr_kind(storePtr, vn, 0),
		AVAL_TEXT,
		"attr 0 is text",
	);
	assert(
		fns.vnode_get_dynamic_attr_element_id(storePtr, vn, 0),
		5,
		"attr 0 elem_id is 5",
	);

	fns.vnode_store_destroy(storePtr);
}

// ── VNode Fragments ─────────────────────────────────────────────────────────

function testVNodeFragments(fns: WasmExports): void {
	suite("VNode Fragments — children");

	const storePtr = fns.vnode_store_create();

	// Create child vnodes
	const txt1 = vnodePushText(fns, storePtr, "A");
	const txt2 = vnodePushText(fns, storePtr, "B");
	const txt3 = vnodePushText(fns, storePtr, "C");

	// Create fragment and add children
	const frag = fns.vnode_push_fragment(storePtr);
	fns.vnode_push_fragment_child(storePtr, frag, txt1);
	fns.vnode_push_fragment_child(storePtr, frag, txt2);
	fns.vnode_push_fragment_child(storePtr, frag, txt3);

	assert(
		fns.vnode_fragment_child_count(storePtr, frag),
		3,
		"fragment has 3 children",
	);
	assert(
		fns.vnode_fragment_child_at(storePtr, frag, 0),
		0,
		"child 0 is vnode 0",
	);
	assert(
		fns.vnode_fragment_child_at(storePtr, frag, 1),
		1,
		"child 1 is vnode 1",
	);
	assert(
		fns.vnode_fragment_child_at(storePtr, frag, 2),
		2,
		"child 2 is vnode 2",
	);

	// Verify children are text nodes
	assert(fns.vnode_kind(storePtr, txt1), VNODE_TEXT, "child 0 is Text");
	assert(fns.vnode_kind(storePtr, txt2), VNODE_TEXT, "child 1 is Text");
	assert(fns.vnode_kind(storePtr, txt3), VNODE_TEXT, "child 2 is Text");

	fns.vnode_store_destroy(storePtr);
}

// ── VNode Keys ──────────────────────────────────────────────────────────────

function testVNodeKeys(fns: WasmExports): void {
	suite("VNode Keys — keyed TemplateRef");

	const storePtr = fns.vnode_store_create();

	// Unkeyed
	const vn1 = fns.vnode_push_template_ref(storePtr, 0);
	assert(fns.vnode_has_key(storePtr, vn1), 0, "unkeyed vnode has no key");

	// Keyed
	const vn2 = vnodePushTemplateRefKeyed(fns, storePtr, 0, "item-42");
	assert(fns.vnode_has_key(storePtr, vn2), 1, "keyed vnode has key");

	// Both are TemplateRef
	assert(
		fns.vnode_kind(storePtr, vn1),
		VNODE_TEMPLATE_REF,
		"vn1 is TemplateRef",
	);
	assert(
		fns.vnode_kind(storePtr, vn2),
		VNODE_TEMPLATE_REF,
		"vn2 is TemplateRef",
	);

	fns.vnode_store_destroy(storePtr);
}

// ── VNode Mixed Attributes ──────────────────────────────────────────────────

function testVNodeMixedAttributes(fns: WasmExports): void {
	suite("VNode Mixed Attributes — text, int, bool, event, none");

	const storePtr = fns.vnode_store_create();
	const vn = fns.vnode_push_template_ref(storePtr, 0);

	// Text attribute
	vnodePushDynamicAttrText(fns, storePtr, vn, "class", "btn-primary", 1);
	assert(
		fns.vnode_get_dynamic_attr_kind(storePtr, vn, 0),
		AVAL_TEXT,
		"attr 0 is text",
	);
	assert(
		fns.vnode_get_dynamic_attr_element_id(storePtr, vn, 0),
		1,
		"attr 0 elem_id",
	);

	// Int attribute
	vnodePushDynamicAttrInt(fns, storePtr, vn, "tabindex", 3, 2);
	assert(
		fns.vnode_get_dynamic_attr_kind(storePtr, vn, 1),
		AVAL_INT,
		"attr 1 is int",
	);
	assert(
		fns.vnode_get_dynamic_attr_element_id(storePtr, vn, 1),
		2,
		"attr 1 elem_id",
	);

	// Bool attribute
	vnodePushDynamicAttrBool(fns, storePtr, vn, "disabled", true, 3);
	assert(
		fns.vnode_get_dynamic_attr_kind(storePtr, vn, 2),
		AVAL_BOOL,
		"attr 2 is bool",
	);
	assert(
		fns.vnode_get_dynamic_attr_element_id(storePtr, vn, 2),
		3,
		"attr 2 elem_id",
	);

	// Event handler
	vnodePushDynamicAttrEvent(fns, storePtr, vn, "onclick", 77, 4);
	assert(
		fns.vnode_get_dynamic_attr_kind(storePtr, vn, 3),
		AVAL_EVENT,
		"attr 3 is event",
	);
	assert(
		fns.vnode_get_dynamic_attr_element_id(storePtr, vn, 3),
		4,
		"attr 3 elem_id",
	);

	// None (removal)
	vnodePushDynamicAttrNone(fns, storePtr, vn, "hidden", 5);
	assert(
		fns.vnode_get_dynamic_attr_kind(storePtr, vn, 4),
		AVAL_NONE,
		"attr 4 is none",
	);
	assert(
		fns.vnode_get_dynamic_attr_element_id(storePtr, vn, 4),
		5,
		"attr 4 elem_id",
	);

	assert(
		fns.vnode_dynamic_attr_count(storePtr, vn),
		5,
		"5 dynamic attrs total",
	);

	fns.vnode_store_destroy(storePtr);
}

// ── VNode Store Lifecycle ───────────────────────────────────────────────────

function testVNodeStoreLifecycle(fns: WasmExports): void {
	suite("VNode Store Lifecycle — create, populate, clear, repopulate");

	const storePtr = fns.vnode_store_create();

	// Add some nodes
	vnodePushText(fns, storePtr, "A");
	vnodePushText(fns, storePtr, "B");
	fns.vnode_push_placeholder(storePtr, 1);
	assert(fns.vnode_count(storePtr), 3, "3 vnodes before clear");

	// Clear
	fns.vnode_store_clear(storePtr);
	assert(fns.vnode_count(storePtr), 0, "0 vnodes after clear");

	// Repopulate
	const idx = fns.vnode_push_template_ref(storePtr, 5);
	assert(idx, 0, "indices restart at 0 after clear");
	assert(fns.vnode_count(storePtr), 1, "1 vnode after repopulate");
	assert(
		fns.vnode_kind(storePtr, idx),
		VNODE_TEMPLATE_REF,
		"repopulated vnode is TemplateRef",
	);

	fns.vnode_store_destroy(storePtr);
}

// ── Builder Query Pre-Build ─────────────────────────────────────────────────

function testBuilderQueryPreBuild(fns: WasmExports): void {
	suite("Template Builder — pre-build queries");

	const b = createBuilder(fns, "query-test");

	// Empty builder
	assert(fns.tmpl_builder_node_count(b), 0, "empty builder has 0 nodes");
	assert(fns.tmpl_builder_root_count(b), 0, "empty builder has 0 roots");
	assert(fns.tmpl_builder_attr_count(b), 0, "empty builder has 0 attrs");

	// Add nodes and check counts incrementally
	const r1 = fns.tmpl_builder_push_element(b, TAG_UL, -1);
	assert(fns.tmpl_builder_node_count(b), 1, "1 node after push_element");
	assert(fns.tmpl_builder_root_count(b), 1, "1 root after root push");

	fns.tmpl_builder_push_element(b, TAG_LI, r1);
	assert(fns.tmpl_builder_node_count(b), 2, "2 nodes");
	assert(fns.tmpl_builder_root_count(b), 1, "still 1 root (child added)");

	builderPushText(fns, b, "item", 1);
	assert(fns.tmpl_builder_node_count(b), 3, "3 nodes");

	// Add another root
	fns.tmpl_builder_push_element(b, TAG_P, -1);
	assert(fns.tmpl_builder_root_count(b), 2, "2 roots now");

	// Add an attribute
	builderPushStaticAttr(fns, b, r1, "class", "list");
	assert(fns.tmpl_builder_attr_count(b), 1, "1 attr");

	fns.tmpl_builder_push_dynamic_attr(b, r1, 0);
	assert(fns.tmpl_builder_attr_count(b), 2, "2 attrs");

	fns.tmpl_builder_destroy(b);
}

// ── Complex Template (Counter-like) ─────────────────────────────────────────

function testComplexTemplate(fns: WasmExports): void {
	suite("Complex Template — counter-like structure");

	const rtPtr = fns.runtime_create();

	// Build a counter template:
	// div.counter
	//   h1 > dyntext[0]  ("Count: N")
	//   button > "+"      (onclick = dynamic attr 0)
	//   button > "-"      (onclick = dynamic attr 1)
	const b = createBuilder(fns, "counter");
	const div = fns.tmpl_builder_push_element(b, TAG_DIV, -1);
	builderPushStaticAttr(fns, b, div, "class", "counter");

	const h1 = fns.tmpl_builder_push_element(b, TAG_H1, div);
	fns.tmpl_builder_push_dynamic_text(b, 0, h1);

	const btn1 = fns.tmpl_builder_push_element(b, TAG_BUTTON, div);
	builderPushText(fns, b, "+", btn1);
	fns.tmpl_builder_push_dynamic_attr(b, btn1, 0);

	const btn2 = fns.tmpl_builder_push_element(b, TAG_BUTTON, div);
	builderPushText(fns, b, "-", btn2);
	fns.tmpl_builder_push_dynamic_attr(b, btn2, 1);

	const tmplId = registerBuilder(fns, rtPtr, b);

	// Verify structure
	// Nodes: div(0), h1(1), dyntext(2), btn1(3), "+"(4), btn2(5), "-"(6) = 7
	assert(fns.tmpl_node_count(rtPtr, tmplId), 7, "7 nodes in counter template");
	assert(fns.tmpl_root_count(rtPtr, tmplId), 1, "1 root (div)");

	// div children: h1, btn1, btn2
	assert(fns.tmpl_node_child_count(rtPtr, tmplId, 0), 3, "div has 3 children");
	assert(fns.tmpl_node_child_at(rtPtr, tmplId, 0, 0), 1, "div child 0 = h1");
	assert(fns.tmpl_node_child_at(rtPtr, tmplId, 0, 1), 3, "div child 1 = btn1");
	assert(fns.tmpl_node_child_at(rtPtr, tmplId, 0, 2), 5, "div child 2 = btn2");

	// h1 has 1 child: dyntext[0]
	assert(fns.tmpl_node_child_count(rtPtr, tmplId, 1), 1, "h1 has 1 child");
	assert(
		fns.tmpl_node_kind(rtPtr, tmplId, 2),
		TNODE_DYNAMIC_TEXT,
		"h1 child is DynamicText",
	);
	assert(fns.tmpl_node_dynamic_index(rtPtr, tmplId, 2), 0, "dyntext index 0");

	// btn1 has 1 child: "+"
	assert(fns.tmpl_node_tag(rtPtr, tmplId, 3), TAG_BUTTON, "btn1 is BUTTON");
	assert(fns.tmpl_node_child_count(rtPtr, tmplId, 3), 1, "btn1 has 1 child");
	assert(
		fns.tmpl_node_kind(rtPtr, tmplId, 4),
		TNODE_TEXT,
		"btn1 child is Text",
	);

	// btn2 has 1 child: "-"
	assert(fns.tmpl_node_tag(rtPtr, tmplId, 5), TAG_BUTTON, "btn2 is BUTTON");
	assert(fns.tmpl_node_child_count(rtPtr, tmplId, 5), 1, "btn2 has 1 child");
	assert(
		fns.tmpl_node_kind(rtPtr, tmplId, 6),
		TNODE_TEXT,
		"btn2 child is Text",
	);

	// Attribute counts
	assert(fns.tmpl_static_attr_count(rtPtr, tmplId), 1, "1 static attr (class)");
	assert(
		fns.tmpl_dynamic_attr_count(rtPtr, tmplId),
		2,
		"2 dynamic attrs (onclick x2)",
	);
	assert(fns.tmpl_attr_total_count(rtPtr, tmplId), 3, "3 attrs total");

	// Dynamic slot counts
	assert(fns.tmpl_dynamic_text_count(rtPtr, tmplId), 1, "1 dynamic text slot");
	assert(fns.tmpl_dynamic_node_count(rtPtr, tmplId), 0, "0 dynamic node slots");

	// Now create a VNode that instantiates this template
	const storePtr = fns.vnode_store_create();
	const vn = fns.vnode_push_template_ref(storePtr, tmplId);

	// Fill dynamic text: "Count: 5"
	vnodePushDynamicTextNode(fns, storePtr, vn, "Count: 5");

	// Fill dynamic attrs: onclick handlers
	vnodePushDynamicAttrEvent(fns, storePtr, vn, "onclick", 1, 3);
	vnodePushDynamicAttrEvent(fns, storePtr, vn, "onclick", 2, 5);

	assert(
		fns.vnode_dynamic_node_count(storePtr, vn),
		1,
		"vnode has 1 dynamic node",
	);
	assert(
		fns.vnode_dynamic_attr_count(storePtr, vn),
		2,
		"vnode has 2 dynamic attrs",
	);
	assert(
		fns.vnode_template_id(storePtr, vn),
		tmplId,
		"vnode references counter template",
	);

	fns.vnode_store_destroy(storePtr);
	fns.tmpl_builder_destroy(b);
	fns.runtime_destroy(rtPtr);
}

// ── Multiple Templates in One Runtime ───────────────────────────────────────

function testMultipleTemplatesInOneRuntime(fns: WasmExports): void {
	suite("Multiple Templates — different structures in one runtime");

	const rtPtr = fns.runtime_create();

	// Template 1: simple div > "Hello"
	const b1 = createBuilder(fns, "hello");
	const div1 = fns.tmpl_builder_push_element(b1, TAG_DIV, -1);
	builderPushText(fns, b1, "Hello", div1);
	const id1 = registerBuilder(fns, rtPtr, b1);

	// Template 2: ul > li > "Item 1", li > "Item 2"
	const b2 = createBuilder(fns, "list");
	const ul = fns.tmpl_builder_push_element(b2, TAG_UL, -1);
	const li1 = fns.tmpl_builder_push_element(b2, TAG_LI, ul);
	builderPushText(fns, b2, "Item 1", li1);
	const li2 = fns.tmpl_builder_push_element(b2, TAG_LI, ul);
	builderPushText(fns, b2, "Item 2", li2);
	const id2 = registerBuilder(fns, rtPtr, b2);

	// Template 3: form > input + button
	const b3 = createBuilder(fns, "form");
	const form = fns.tmpl_builder_push_element(b3, TAG_FORM, -1);
	fns.tmpl_builder_push_element(b3, TAG_INPUT, form);
	const submitBtn = fns.tmpl_builder_push_element(b3, TAG_BUTTON, form);
	builderPushText(fns, b3, "Submit", submitBtn);
	const id3 = registerBuilder(fns, rtPtr, b3);

	// Verify IDs
	assert(id1, 0, "hello gets ID 0");
	assert(id2, 1, "list gets ID 1");
	assert(id3, 2, "form gets ID 2");
	assert(fns.tmpl_count(rtPtr), 3, "3 templates registered");

	// Cross-template queries
	assert(fns.tmpl_node_count(rtPtr, id1), 2, "hello has 2 nodes");
	assert(fns.tmpl_node_count(rtPtr, id2), 5, "list has 5 nodes");
	assert(fns.tmpl_node_count(rtPtr, id3), 4, "form has 4 nodes");

	assert(fns.tmpl_node_tag(rtPtr, id1, 0), TAG_DIV, "hello root is div");
	assert(fns.tmpl_node_tag(rtPtr, id2, 0), TAG_UL, "list root is ul");
	assert(fns.tmpl_node_tag(rtPtr, id3, 0), TAG_FORM, "form root is form");

	// Verify list children
	assert(fns.tmpl_node_child_count(rtPtr, id2, 0), 2, "ul has 2 children");
	assert(fns.tmpl_node_tag(rtPtr, id2, 1), TAG_LI, "first child is li");
	assert(fns.tmpl_node_tag(rtPtr, id2, 3), TAG_LI, "second child is li");

	// Verify form children
	assert(fns.tmpl_node_child_count(rtPtr, id3, 0), 2, "form has 2 children");
	assert(fns.tmpl_node_tag(rtPtr, id3, 1), TAG_INPUT, "first child is input");
	assert(
		fns.tmpl_node_tag(rtPtr, id3, 2),
		TAG_BUTTON,
		"second child is button",
	);

	fns.tmpl_builder_destroy(b1);
	fns.tmpl_builder_destroy(b2);
	fns.tmpl_builder_destroy(b3);
	fns.runtime_destroy(rtPtr);
}

// ── Builder Reset After Build ───────────────────────────────────────────────

function testBuilderResetAfterBuild(fns: WasmExports): void {
	suite("Builder Reset — builder is empty after build()");

	const rtPtr = fns.runtime_create();

	const b = createBuilder(fns, "reset-test");
	fns.tmpl_builder_push_element(b, TAG_DIV, -1);
	builderPushText(fns, b, "hello", 0);
	builderPushStaticAttr(fns, b, 0, "class", "x");

	assert(fns.tmpl_builder_node_count(b), 2, "2 nodes before build");
	assert(fns.tmpl_builder_root_count(b), 1, "1 root before build");
	assert(fns.tmpl_builder_attr_count(b), 1, "1 attr before build");

	registerBuilder(fns, rtPtr, b);

	// After build, builder should be reset/empty
	assert(fns.tmpl_builder_node_count(b), 0, "0 nodes after build");
	assert(fns.tmpl_builder_root_count(b), 0, "0 roots after build");
	assert(fns.tmpl_builder_attr_count(b), 0, "0 attrs after build");

	// Can reuse the builder for a new template
	fns.tmpl_builder_push_element(b, TAG_SPAN, -1);
	assert(fns.tmpl_builder_node_count(b), 1, "1 node after reuse");

	const id2 = registerBuilder(fns, rtPtr, b);
	// The name "reset-test" is already registered, so this deduplicates
	assert(id2, 0, "same name → same ID (deduped after reuse)");

	fns.tmpl_builder_destroy(b);
	fns.runtime_destroy(rtPtr);
}

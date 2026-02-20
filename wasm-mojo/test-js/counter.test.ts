// Counter App End-to-End Tests — Phase 7 (M7)
//
// Tests the full counter app lifecycle:
//   init → rebuild → mount → click → flush → DOM update
//
// Uses linkedom for headless DOM and the WASM counter_* exports
// orchestrated through createCounterApp().

import { parseHTML } from "npm:linkedom";
import { createCounterApp } from "../runtime/app.ts";
import { alignedAlloc, getMemory } from "../runtime/memory.ts";
import { MutationReader } from "../runtime/protocol.ts";
import type { WasmExports } from "../runtime/types.ts";
import { assert, pass, suite } from "./harness.ts";

type Fns = WasmExports & Record<string, unknown>;

// ── DOM helper ──────────────────────────────────────────────────────────────

function createDOM() {
	const { document, window } = parseHTML(
		"<!DOCTYPE html><html><body><div id='root'></div></body></html>",
	);
	const root = document.getElementById("root")!;
	return { document, window, root };
}

// ── Constants ───────────────────────────────────────────────────────────────

const EVT_CLICK = 0;
const BUF_CAPACITY = 16384;

// ══════════════════════════════════════════════════════════════════════════════

export function testCounter(fns: Fns): void {
	// ═════════════════════════════════════════════════════════════════════
	// Section 1: Low-level counter app exports
	// ═════════════════════════════════════════════════════════════════════

	suite("Counter — counter_init creates app with correct state");
	{
		const app = fns.counter_init();

		// Query exports should return valid values
		const rtPtr = fns.counter_rt_ptr(app);
		assert(rtPtr !== 0n, true, "runtime pointer is non-zero");

		const tmplId = fns.counter_tmpl_id(app);
		assert(tmplId >= 0, true, "template ID is non-negative");

		const incrH = fns.counter_incr_handler(app);
		assert(incrH >= 0, true, "incr handler ID is non-negative");

		const decrH = fns.counter_decr_handler(app);
		assert(decrH >= 0, true, "decr handler ID is non-negative");
		assert(incrH !== decrH, true, "incr and decr handlers are different");

		const scopeId = fns.counter_scope_id(app);
		assert(scopeId >= 0, true, "scope ID is non-negative");

		const sigKey = fns.counter_count_signal(app);
		assert(sigKey >= 0, true, "signal key is non-negative");

		// Initial count is 0
		assert(fns.counter_count_value(app), 0, "initial count is 0");

		// No dirty scopes yet
		assert(fns.counter_has_dirty(app), 0, "no dirty scopes initially");

		fns.counter_destroy(app);
	}

	// ═════════════════════════════════════════════════════════════════════
	// Section 2: counter_rebuild emits mutations
	// ═════════════════════════════════════════════════════════════════════

	suite("Counter — counter_rebuild produces mutation buffer");
	{
		const app = fns.counter_init();
		const bufPtr = alignedAlloc(8n, BigInt(BUF_CAPACITY));

		const offset = fns.counter_rebuild(app, bufPtr, BUF_CAPACITY);
		assert(offset > 0, true, "rebuild wrote mutations (offset > 0)");

		// Decode the mutations
		const mem = getMemory();
		const mutations = new MutationReader(
			mem.buffer,
			Number(bufPtr),
			offset,
		).readAll();

		assert(mutations.length > 0, true, "at least one mutation decoded");

		// Verify we have a LoadTemplate mutation
		const loadTemplates = mutations.filter((m) => m.op === 0x05);
		assert(loadTemplates.length > 0, true, "contains LoadTemplate mutation");

		// Verify we have an AppendChildren mutation (mounting to root)
		const appendChildren = mutations.filter((m) => m.op === 0x01);
		assert(appendChildren.length > 0, true, "contains AppendChildren mutation");

		// Verify we have a SetText mutation for "Count: 0"
		const setTexts = mutations.filter(
			(m) => m.op === 0x0b && "text" in m,
		) as Array<{ op: number; id: number; text: string }>;
		const countText = setTexts.find((m) => m.text === "Count: 0");
		assert(countText !== undefined, true, 'SetText with "Count: 0" found');

		// Verify we have NewEventListener mutations for click handlers
		const newListeners = mutations.filter((m) => m.op === 0x0c);
		assert(
			newListeners.length >= 2,
			true,
			"at least 2 NewEventListener mutations (+ and - buttons)",
		);

		fns.counter_destroy(app);
	}

	// ═════════════════════════════════════════════════════════════════════
	// Section 3: counter_handle_event modifies signal
	// ═════════════════════════════════════════════════════════════════════

	suite("Counter — counter_handle_event increments signal");
	{
		const app = fns.counter_init();

		const incrH = fns.counter_incr_handler(app);
		const decrH = fns.counter_decr_handler(app);

		// Increment
		const r1 = fns.counter_handle_event(app, incrH, EVT_CLICK);
		assert(r1, 1, "increment dispatch returned 1 (action executed)");
		assert(fns.counter_count_value(app), 1, "count is 1 after increment");
		assert(fns.counter_has_dirty(app), 1, "scope is dirty after event");

		// Increment again
		fns.counter_handle_event(app, incrH, EVT_CLICK);
		assert(fns.counter_count_value(app), 2, "count is 2 after 2nd increment");

		// Decrement
		fns.counter_handle_event(app, decrH, EVT_CLICK);
		assert(fns.counter_count_value(app), 1, "count is 1 after decrement");

		fns.counter_destroy(app);
	}

	// ═════════════════════════════════════════════════════════════════════
	// Section 4: counter_flush produces diff mutations
	// ═════════════════════════════════════════════════════════════════════

	suite("Counter — counter_flush emits SetText after increment");
	{
		const app = fns.counter_init();
		const bufPtr = alignedAlloc(8n, BigInt(BUF_CAPACITY));

		// Initial rebuild
		fns.counter_rebuild(app, bufPtr, BUF_CAPACITY);

		// Dispatch increment
		const incrH = fns.counter_incr_handler(app);
		fns.counter_handle_event(app, incrH, EVT_CLICK);

		// Flush
		const flushLen = fns.counter_flush(app, bufPtr, BUF_CAPACITY);
		assert(flushLen > 0, true, "flush wrote mutations (len > 0)");

		// Decode flush mutations
		const mem = getMemory();
		const mutations = new MutationReader(
			mem.buffer,
			Number(bufPtr),
			flushLen,
		).readAll();

		// Should contain a SetText mutation for "Count: 1"
		const setTexts = mutations.filter(
			(m) => m.op === 0x0b && "text" in m,
		) as Array<{ op: number; id: number; text: string }>;
		const countText = setTexts.find((m) => m.text === "Count: 1");
		assert(countText !== undefined, true, 'flush contains SetText "Count: 1"');

		// After flush, no more dirty scopes
		assert(fns.counter_has_dirty(app), 0, "clean after flush");

		fns.counter_destroy(app);
	}

	// ═════════════════════════════════════════════════════════════════════
	// Section 5: counter_flush with no changes returns 0
	// ═════════════════════════════════════════════════════════════════════

	suite("Counter — counter_flush returns 0 when nothing dirty");
	{
		const app = fns.counter_init();
		const bufPtr = alignedAlloc(8n, BigInt(BUF_CAPACITY));

		// Rebuild (no events dispatched)
		fns.counter_rebuild(app, bufPtr, BUF_CAPACITY);

		// Flush without any event → should return 0
		const len = fns.counter_flush(app, bufPtr, BUF_CAPACITY);
		assert(len, 0, "flush returns 0 when nothing dirty");

		fns.counter_destroy(app);
	}

	// ═════════════════════════════════════════════════════════════════════
	// Section 6: Multiple flush cycles produce correct text
	// ═════════════════════════════════════════════════════════════════════

	suite("Counter — multiple increment/flush cycles update text correctly");
	{
		const app = fns.counter_init();
		const bufPtr = alignedAlloc(8n, BigInt(BUF_CAPACITY));

		fns.counter_rebuild(app, bufPtr, BUF_CAPACITY);
		const incrH = fns.counter_incr_handler(app);
		const mem = getMemory();

		for (let i = 1; i <= 5; i++) {
			fns.counter_handle_event(app, incrH, EVT_CLICK);
			const len = fns.counter_flush(app, bufPtr, BUF_CAPACITY);
			assert(len > 0, true, `flush ${i} produced mutations`);

			const mutations = new MutationReader(
				mem.buffer,
				Number(bufPtr),
				len,
			).readAll();

			const setTexts = mutations.filter(
				(m) => m.op === 0x0b && "text" in m,
			) as Array<{ op: number; id: number; text: string }>;
			const expected = `Count: ${i}`;
			const found = setTexts.find((m) => m.text === expected);
			assert(
				found !== undefined,
				true,
				`flush ${i} contains SetText "${expected}"`,
			);
		}

		assert(fns.counter_count_value(app), 5, "count is 5 after 5 increments");

		fns.counter_destroy(app);
	}

	// ═════════════════════════════════════════════════════════════════════
	// Section 7: Mixed increment/decrement
	// ═════════════════════════════════════════════════════════════════════

	suite("Counter — mixed increment/decrement produces correct count");
	{
		const app = fns.counter_init();
		const bufPtr = alignedAlloc(8n, BigInt(BUF_CAPACITY));

		fns.counter_rebuild(app, bufPtr, BUF_CAPACITY);
		const incrH = fns.counter_incr_handler(app);
		const decrH = fns.counter_decr_handler(app);
		const mem = getMemory();

		// +1, +1, +1, -1, -1 → count = 1
		fns.counter_handle_event(app, incrH, EVT_CLICK);
		fns.counter_flush(app, bufPtr, BUF_CAPACITY);

		fns.counter_handle_event(app, incrH, EVT_CLICK);
		fns.counter_flush(app, bufPtr, BUF_CAPACITY);

		fns.counter_handle_event(app, incrH, EVT_CLICK);
		fns.counter_flush(app, bufPtr, BUF_CAPACITY);

		fns.counter_handle_event(app, decrH, EVT_CLICK);
		fns.counter_flush(app, bufPtr, BUF_CAPACITY);

		fns.counter_handle_event(app, decrH, EVT_CLICK);
		const len = fns.counter_flush(app, bufPtr, BUF_CAPACITY);

		assert(fns.counter_count_value(app), 1, "count is 1 after +3 -2");

		const mutations = new MutationReader(
			mem.buffer,
			Number(bufPtr),
			len,
		).readAll();

		const setTexts = mutations.filter(
			(m) => m.op === 0x0b && "text" in m,
		) as Array<{ op: number; id: number; text: string }>;
		const found = setTexts.find((m) => m.text === "Count: 1");
		assert(found !== undefined, true, 'final flush has SetText "Count: 1"');

		fns.counter_destroy(app);
	}

	// ═════════════════════════════════════════════════════════════════════
	// Section 8: Template registered in runtime via counter_init
	// ═════════════════════════════════════════════════════════════════════

	suite("Counter — template is queryable from runtime");
	{
		const app = fns.counter_init();
		const rtPtr = fns.counter_rt_ptr(app);
		const tmplId = fns.counter_tmpl_id(app);

		// Template should be registered
		assert(fns.tmpl_count(rtPtr) >= 1, true, "at least 1 template");

		// Root count: 1 (single div root)
		assert(fns.tmpl_root_count(rtPtr, tmplId), 1, "template has 1 root");

		// Root is an element node
		const rootIdx = fns.tmpl_get_root_index(rtPtr, tmplId, 0);
		assert(
			fns.tmpl_node_kind(rtPtr, tmplId, rootIdx),
			0,
			"root is TNODE_ELEMENT (0)",
		);

		// Root is a div (TAG_DIV = 0)
		assert(fns.tmpl_node_tag(rtPtr, tmplId, rootIdx), 0, "root tag is div");

		// Root has 3 children: span, button, button
		const childCount = fns.tmpl_node_child_count(rtPtr, tmplId, rootIdx);
		assert(childCount, 3, "root div has 3 children");

		// First child is span (TAG_SPAN = 1)
		const spanIdx = fns.tmpl_node_child_at(rtPtr, tmplId, rootIdx, 0);
		assert(fns.tmpl_node_tag(rtPtr, tmplId, spanIdx), 1, "first child is span");

		// Second child is button (TAG_BUTTON = 19)
		const btn1Idx = fns.tmpl_node_child_at(rtPtr, tmplId, rootIdx, 1);
		assert(
			fns.tmpl_node_tag(rtPtr, tmplId, btn1Idx),
			19,
			"second child is button",
		);

		// Third child is button
		const btn2Idx = fns.tmpl_node_child_at(rtPtr, tmplId, rootIdx, 2);
		assert(
			fns.tmpl_node_tag(rtPtr, tmplId, btn2Idx),
			19,
			"third child is button",
		);

		// Template has dynamic text nodes
		assert(
			fns.tmpl_dynamic_text_count(rtPtr, tmplId) >= 1,
			true,
			"template has at least 1 dynamic text slot",
		);

		// Template has dynamic attrs
		assert(
			fns.tmpl_dynamic_attr_count(rtPtr, tmplId) >= 2,
			true,
			"template has at least 2 dynamic attr slots",
		);

		fns.counter_destroy(app);
	}

	// ═════════════════════════════════════════════════════════════════════
	// Section 9: Full DOM integration — createCounterApp
	// ═════════════════════════════════════════════════════════════════════

	suite("Counter — createCounterApp mounts to DOM");
	{
		const dom = createDOM();
		const handle = createCounterApp(fns, dom.root, dom.document);

		// Root should have children after mount
		assert(
			dom.root.childNodes.length > 0,
			true,
			"root has children after mount",
		);

		// Should have a div as the first child
		const divEl = dom.root.childNodes[0] as Element;
		assert(divEl.nodeName.toLowerCase(), "div", "first child is a div");

		// Div should have 3 children: span, button, button
		assert(divEl.childNodes.length, 3, "div has 3 children");

		const spanEl = divEl.childNodes[0] as Element;
		assert(spanEl.nodeName.toLowerCase(), "span", "first div child is span");

		const btn1 = divEl.childNodes[1] as Element;
		assert(btn1.nodeName.toLowerCase(), "button", "second div child is button");

		const btn2 = divEl.childNodes[2] as Element;
		assert(btn2.nodeName.toLowerCase(), "button", "third div child is button");

		// Button text content
		assert(btn1.textContent, "+", 'first button text is "+"');
		assert(btn2.textContent, "-", 'second button text is "-"');

		// Span should display "Count: 0"
		assert(spanEl.textContent, "Count: 0", 'span displays "Count: 0"');

		// Initial count
		assert(handle.getCount(), 0, "getCount() returns 0");

		handle.destroy();
	}

	// ═════════════════════════════════════════════════════════════════════
	// Section 10: Full DOM integration — increment updates display
	// ═════════════════════════════════════════════════════════════════════

	suite("Counter — increment updates DOM text");
	{
		const dom = createDOM();
		const handle = createCounterApp(fns, dom.root, dom.document);

		const divEl = dom.root.childNodes[0] as Element;
		const spanEl = divEl.childNodes[0] as Element;

		// Click increment
		handle.increment();

		assert(handle.getCount(), 1, "count is 1 after increment");
		assert(
			spanEl.textContent,
			"Count: 1",
			'span displays "Count: 1" after increment',
		);

		handle.destroy();
	}

	// ═════════════════════════════════════════════════════════════════════
	// Section 11: Full DOM integration — multiple increments
	// ═════════════════════════════════════════════════════════════════════

	suite("Counter — 10 increments updates DOM correctly");
	{
		const dom = createDOM();
		const handle = createCounterApp(fns, dom.root, dom.document);

		const divEl = dom.root.childNodes[0] as Element;
		const spanEl = divEl.childNodes[0] as Element;

		for (let i = 0; i < 10; i++) {
			handle.increment();
		}

		assert(handle.getCount(), 10, "count is 10 after 10 increments");
		assert(spanEl.textContent, "Count: 10", 'span displays "Count: 10"');

		handle.destroy();
	}

	// ═════════════════════════════════════════════════════════════════════
	// Section 12: Full DOM integration — decrement
	// ═════════════════════════════════════════════════════════════════════

	suite("Counter — decrement updates DOM text");
	{
		const dom = createDOM();
		const handle = createCounterApp(fns, dom.root, dom.document);

		const divEl = dom.root.childNodes[0] as Element;
		const spanEl = divEl.childNodes[0] as Element;

		// Increment 3 times, decrement once → 2
		handle.increment();
		handle.increment();
		handle.increment();
		handle.decrement();

		assert(handle.getCount(), 2, "count is 2 after +3 -1");
		assert(spanEl.textContent, "Count: 2", 'span displays "Count: 2"');

		handle.destroy();
	}

	// ═════════════════════════════════════════════════════════════════════
	// Section 13: Full DOM integration — decrement below zero
	// ═════════════════════════════════════════════════════════════════════

	suite("Counter — decrement below zero works");
	{
		const dom = createDOM();
		const handle = createCounterApp(fns, dom.root, dom.document);

		const divEl = dom.root.childNodes[0] as Element;
		const spanEl = divEl.childNodes[0] as Element;

		handle.decrement();
		handle.decrement();

		assert(handle.getCount(), -2, "count is -2 after 2 decrements");
		assert(spanEl.textContent, "Count: -2", 'span displays "Count: -2"');

		handle.destroy();
	}

	// ═════════════════════════════════════════════════════════════════════
	// Section 14: Minimal mutations per click
	// ═════════════════════════════════════════════════════════════════════

	suite("Counter — minimal mutations per click (only SetText)");
	{
		const app = fns.counter_init();
		const bufPtr = alignedAlloc(8n, BigInt(BUF_CAPACITY));

		fns.counter_rebuild(app, bufPtr, BUF_CAPACITY);

		const incrH = fns.counter_incr_handler(app);
		fns.counter_handle_event(app, incrH, EVT_CLICK);

		const len = fns.counter_flush(app, bufPtr, BUF_CAPACITY);
		const mem = getMemory();
		const mutations = new MutationReader(
			mem.buffer,
			Number(bufPtr),
			len,
		).readAll();

		// The diff should produce only a SetText mutation (0x0b)
		// No LoadTemplate, no AppendChildren, etc.
		const nonSetText = mutations.filter((m) => m.op !== 0x0b);
		assert(
			nonSetText.length,
			0,
			"flush contains only SetText mutations (minimal diff)",
		);

		assert(mutations.length, 1, "exactly 1 mutation (SetText)");

		fns.counter_destroy(app);
	}

	// ═════════════════════════════════════════════════════════════════════
	// Section 15: DOM structure preserved across updates
	// ═════════════════════════════════════════════════════════════════════

	suite("Counter — DOM structure preserved across updates");
	{
		const dom = createDOM();
		const handle = createCounterApp(fns, dom.root, dom.document);

		const divEl = dom.root.childNodes[0] as Element;
		const spanEl = divEl.childNodes[0] as Element;
		// Update 5 times
		for (let i = 0; i < 5; i++) {
			handle.increment();
		}

		// Structure should be the same objects (no re-creation)
		const newDiv = dom.root.childNodes[0] as Element;
		assert(newDiv.childNodes.length, 3, "div still has 3 children");

		// Buttons should still have their text
		assert(
			(newDiv.childNodes[1] as Element).textContent,
			"+",
			'button 1 still says "+"',
		);
		assert(
			(newDiv.childNodes[2] as Element).textContent,
			"-",
			'button 2 still says "-"',
		);

		// Span should show updated count
		assert(
			(newDiv.childNodes[0] as Element).textContent,
			"Count: 5",
			'span says "Count: 5"',
		);

		handle.destroy();
	}

	// ═════════════════════════════════════════════════════════════════════
	// Section 16: Multiple independent app instances
	// ═════════════════════════════════════════════════════════════════════

	suite("Counter — multiple independent app instances");
	{
		const dom1 = createDOM();
		const dom2 = createDOM();

		const h1 = createCounterApp(fns, dom1.root, dom1.document);
		const h2 = createCounterApp(fns, dom2.root, dom2.document);

		// Increment h1 three times
		h1.increment();
		h1.increment();
		h1.increment();

		// Increment h2 once
		h2.increment();

		assert(h1.getCount(), 3, "app1 count is 3");
		assert(h2.getCount(), 1, "app2 count is 1");

		const span1 = (dom1.root.childNodes[0] as Element).childNodes[0] as Element;
		const span2 = (dom2.root.childNodes[0] as Element).childNodes[0] as Element;

		assert(span1.textContent, "Count: 3", 'app1 span shows "Count: 3"');
		assert(span2.textContent, "Count: 1", 'app2 span shows "Count: 1"');

		h1.destroy();
		h2.destroy();
	}

	// ═════════════════════════════════════════════════════════════════════
	// Section 17: Rapid increment stress test
	// ═════════════════════════════════════════════════════════════════════

	suite("Counter — rapid 100 increments");
	{
		const dom = createDOM();
		const handle = createCounterApp(fns, dom.root, dom.document);

		for (let i = 0; i < 100; i++) {
			handle.increment();
		}

		assert(handle.getCount(), 100, "count is 100 after 100 increments");

		const spanEl = (dom.root.childNodes[0] as Element).childNodes[0] as Element;
		assert(spanEl.textContent, "Count: 100", 'span displays "Count: 100"');

		handle.destroy();
	}

	// ═════════════════════════════════════════════════════════════════════
	// Section 18: Destroy cleans up
	// ═════════════════════════════════════════════════════════════════════

	suite("Counter — destroy does not crash");
	{
		const dom = createDOM();
		const handle = createCounterApp(fns, dom.root, dom.document);

		handle.increment();
		handle.increment();

		// Destroy should not throw
		handle.destroy();
		pass(1);
	}
}

// Todo App End-to-End Tests — Phase 8 (M8)
//
// Tests the full todo app lifecycle:
//   init → rebuild → mount → add/toggle/remove → flush → DOM update
//
// Uses linkedom for headless DOM and the WASM todo_* exports.
// Exercises:
//   - Dynamic keyed list reconciliation (add, remove, toggle)
//   - Conditional rendering (completed class styling)
//   - Fragment VNodes with keyed children
//   - String data flow (input text from JS → WASM)

import { parseHTML } from "npm:linkedom";
import { createApp } from "../runtime/app.ts";
import type { Interpreter } from "../runtime/interpreter.ts";
import { alignedAlloc, getMemory } from "../runtime/memory.ts";
import { MutationReader, Op } from "../runtime/protocol.ts";

import { writeStringStruct } from "../runtime/strings.ts";
import type { TemplateCache } from "../runtime/templates.ts";
import type { WasmExports } from "../runtime/types.ts";
import { assert, pass, suite } from "./harness.ts";

type Fns = WasmExports & Record<string, CallableFunction>;

// ── DOM helper ──────────────────────────────────────────────────────────────

function createDOM() {
	const { document, window } = parseHTML(
		"<!DOCTYPE html><html><body><div id='root'></div></body></html>",
	);
	const root = document.getElementById("root")!;
	return { document, window, root };
}

// ── Constants ───────────────────────────────────────────────────────────────

const BUF_CAPACITY = 65536;

// ── Todo App handle ─────────────────────────────────────────────────────────

interface TodoAppHandle {
	fns: Fns;
	interpreter: Interpreter;
	templates: TemplateCache;
	root: Element;
	appPtr: bigint;
	bufPtr: bigint;

	addItem(text: string): void;
	removeItem(itemId: number): void;
	toggleItem(itemId: number): void;
	itemCount(): number;
	itemIdAt(index: number): number;
	itemCompletedAt(index: number): boolean;
	listVersion(): number;
	hasDirty(): boolean;
	flush(): number;
	destroy(): void;
}

function createTodoApp(fns: Fns, root: Element, doc: Document): TodoAppHandle {
	// Use the generic createApp factory — templates come from WASM via
	// RegisterTemplate mutations, no manual DOM template construction needed.
	const handle = createApp({
		fns,
		root,
		doc,
		bufCapacity: BUF_CAPACITY,
		init: (f) => f.todo_init(),
		rebuild: (f, app, buf, cap) => f.todo_rebuild(app, buf, cap),
		flush: (f, app, buf, cap) => f.todo_flush(app, buf, cap),
		handleEvent: (f, app, hid, evt) => f.todo_handle_event(app, hid, evt),
		destroy: (f, app) => f.todo_destroy(app),
	});

	return {
		fns,
		interpreter: handle.interpreter,
		templates: handle.templates,
		root,
		appPtr: handle.appPtr,
		bufPtr: handle.bufPtr,

		addItem(text: string): void {
			const strPtr = writeStringStruct(text);
			fns.todo_add_item(handle.appPtr, strPtr);
			handle.flushAndApply();
		},

		removeItem(itemId: number): void {
			fns.todo_remove_item(handle.appPtr, itemId);
			handle.flushAndApply();
		},

		toggleItem(itemId: number): void {
			fns.todo_toggle_item(handle.appPtr, itemId);
			handle.flushAndApply();
		},

		itemCount(): number {
			return fns.todo_item_count(handle.appPtr);
		},

		itemIdAt(index: number): number {
			return fns.todo_item_id_at(handle.appPtr, index);
		},

		itemCompletedAt(index: number): boolean {
			return fns.todo_item_completed_at(handle.appPtr, index) === 1;
		},

		listVersion(): number {
			return fns.todo_list_version(handle.appPtr);
		},

		hasDirty(): boolean {
			return fns.todo_has_dirty(handle.appPtr) === 1;
		},

		flush(): number {
			const len = fns.todo_flush(handle.appPtr, handle.bufPtr, BUF_CAPACITY);
			if (len > 0) {
				const mem = getMemory();
				handle.interpreter.applyMutations(
					mem.buffer,
					Number(handle.bufPtr),
					len,
				);
			}
			return len;
		},

		destroy(): void {
			handle.destroy();
		},
	};
}

// ══════════════════════════════════════════════════════════════════════════════

export function testTodo(fns: Fns): void {
	// ═════════════════════════════════════════════════════════════════════
	// Section 1: Low-level todo app exports
	// ═════════════════════════════════════════════════════════════════════

	suite("Todo — todo_init creates app with correct state");
	{
		const appPtr = fns.todo_init();

		const appTmplId = fns.todo_app_template_id(appPtr);
		assert(appTmplId >= 0, true, "app template ID is non-negative");

		const itemTmplId = fns.todo_item_template_id(appPtr);
		assert(itemTmplId >= 0, true, "item template ID is non-negative");
		assert(
			appTmplId !== itemTmplId,
			true,
			"app and item templates are different",
		);

		const addHandler = fns.todo_add_handler(appPtr);
		assert(addHandler >= 0, true, "add handler ID is non-negative");

		const scopeId = fns.todo_scope_id(appPtr);
		assert(scopeId >= 0, true, "scope ID is non-negative");

		const count = fns.todo_item_count(appPtr);
		assert(count, 0, "initial item count is 0");

		const version = fns.todo_list_version(appPtr);
		assert(version, 0, "initial list version is 0");

		fns.todo_destroy(appPtr);
		pass();
		console.log("    ✓ destroy does not crash");
	}

	suite("Todo — add items bumps version and count");
	{
		const appPtr = fns.todo_init();

		fns.todo_add_item(appPtr, writeStringStruct("Buy milk"));
		assert(fns.todo_item_count(appPtr), 1, "count is 1 after adding");
		assert(fns.todo_list_version(appPtr), 1, "version is 1 after adding");

		fns.todo_add_item(appPtr, writeStringStruct("Walk dog"));
		assert(fns.todo_item_count(appPtr), 2, "count is 2 after second add");
		assert(fns.todo_list_version(appPtr), 2, "version is 2 after second add");

		const id1 = fns.todo_item_id_at(appPtr, 0);
		const id2 = fns.todo_item_id_at(appPtr, 1);
		assert(id1 !== id2, true, "item IDs are unique");
		assert(id1 > 0, true, "first item ID is positive");

		fns.todo_destroy(appPtr);
	}

	suite("Todo — toggle item changes completion");
	{
		const appPtr = fns.todo_init();
		fns.todo_add_item(appPtr, writeStringStruct("Test toggle"));

		const id = fns.todo_item_id_at(appPtr, 0);
		assert(fns.todo_item_completed_at(appPtr, 0), 0, "not completed initially");

		fns.todo_toggle_item(appPtr, id);
		assert(fns.todo_item_completed_at(appPtr, 0), 1, "completed after toggle");

		fns.todo_toggle_item(appPtr, id);
		assert(
			fns.todo_item_completed_at(appPtr, 0),
			0,
			"uncompleted after second toggle",
		);

		fns.todo_destroy(appPtr);
	}

	suite("Todo — remove item decreases count");
	{
		const appPtr = fns.todo_init();
		fns.todo_add_item(appPtr, writeStringStruct("A"));
		fns.todo_add_item(appPtr, writeStringStruct("B"));
		fns.todo_add_item(appPtr, writeStringStruct("C"));
		assert(fns.todo_item_count(appPtr), 3, "3 items");

		const idB = fns.todo_item_id_at(appPtr, 1);
		fns.todo_remove_item(appPtr, idB);
		assert(fns.todo_item_count(appPtr), 2, "2 items after remove");

		fns.todo_destroy(appPtr);
	}

	suite("Todo — empty text is not added");
	{
		const appPtr = fns.todo_init();
		fns.todo_add_item(appPtr, writeStringStruct(""));
		assert(fns.todo_item_count(appPtr), 0, "empty text not added");
		assert(fns.todo_list_version(appPtr), 0, "version unchanged for empty add");

		fns.todo_destroy(appPtr);
	}

	suite("Todo — dirty state after item mutation");
	{
		const appPtr = fns.todo_init();

		assert(fns.todo_has_dirty(appPtr), 0, "not dirty initially");
		fns.todo_add_item(appPtr, writeStringStruct("Dirty test"));
		assert(fns.todo_has_dirty(appPtr), 1, "dirty after add_item");

		fns.todo_destroy(appPtr);
	}

	// ═════════════════════════════════════════════════════════════════════
	// Section 2: DOM-level todo app tests
	// ═════════════════════════════════════════════════════════════════════

	suite("Todo — rebuild emits RegisterTemplate before LoadTemplate");
	{
		const appPtr = fns.todo_init();
		const bufPtr = alignedAlloc(8n, BigInt(BUF_CAPACITY));

		const offset = fns.todo_rebuild(appPtr, bufPtr, BUF_CAPACITY);
		assert(offset > 0, true, "todo rebuild wrote mutations");

		const mem = getMemory();
		const mutations = new MutationReader(
			mem.buffer,
			Number(bufPtr),
			offset,
		).readAll();

		const regIndices = mutations
			.map((m, i) => (m.op === Op.RegisterTemplate ? i : -1))
			.filter((i) => i >= 0);
		const loadIdx = mutations.findIndex((m) => m.op === Op.LoadTemplate);

		assert(
			regIndices.length >= 2,
			true,
			"at least 2 RegisterTemplate mutations (app + item templates)",
		);
		assert(loadIdx >= 0, true, "contains LoadTemplate mutation");
		assert(
			regIndices[regIndices.length - 1] < loadIdx,
			true,
			"all RegisterTemplate precede LoadTemplate",
		);

		fns.todo_destroy(appPtr);
	}

	suite("Todo — initial mount renders empty app shell");
	{
		const { document, root } = createDOM();
		const app = createTodoApp(fns, root, document);

		// The app template: div > [ input, button("Add"), ul ]
		const div = root.firstChild;
		assert(div !== null, true, "root has a child");
		assert((div as Element).tagName, "DIV", "first child is div");

		const children = (div as Element).childNodes;
		assert(children.length, 3, "div has 3 children (input, button, ul)");
		assert(children[0].tagName, "INPUT", "first child is input");
		assert(children[1].tagName, "BUTTON", "second child is button");
		assert(children[2].tagName, "UL", "third child is ul");

		assert(children[1].textContent, "Add", 'button text is "Add"');

		// UL should be empty (placeholder was replaced or is a comment)
		const _ulChildren = children[2].childNodes;
		// May have a placeholder comment or be empty
		const liCount = children[2].querySelectorAll("li").length;
		assert(liCount, 0, "no li items in empty list");

		app.destroy();
	}

	suite("Todo — add single item renders li in ul");
	{
		const { document, root } = createDOM();
		const app = createTodoApp(fns, root, document);

		app.addItem("Buy groceries");

		assert(app.itemCount(), 1, "WASM has 1 item");

		const ul = root.querySelector("ul");
		assert(ul !== null, true, "ul exists");

		const lis = ul!.querySelectorAll("li");
		assert(lis.length, 1, "1 li in ul");

		// Check item text (span > text)
		const span = lis[0].querySelector("span");
		assert(span !== null, true, "li has a span");
		assert(
			span!.textContent!.includes("Buy groceries"),
			true,
			'span contains "Buy groceries"',
		);

		// Check buttons exist
		const buttons = lis[0].querySelectorAll("button");
		assert(buttons.length, 2, "li has 2 buttons (toggle, remove)");

		app.destroy();
	}

	suite("Todo — add multiple items renders all in order");
	{
		const { document, root } = createDOM();
		const app = createTodoApp(fns, root, document);

		app.addItem("Item Alpha");
		app.addItem("Item Beta");
		app.addItem("Item Gamma");

		assert(app.itemCount(), 3, "WASM has 3 items");

		const lis = root.querySelectorAll("li");
		assert(lis.length, 3, "3 li elements in DOM");

		const texts = Array.from(lis).map((li: Element) => {
			const span = li.querySelector("span");
			return span ? span.textContent : "";
		});

		assert(texts[0].includes("Item Alpha"), true, "first item is Alpha");
		assert(texts[1].includes("Item Beta"), true, "second item is Beta");
		assert(texts[2].includes("Item Gamma"), true, "third item is Gamma");

		app.destroy();
	}

	suite("Todo — toggle item updates text/class in DOM");
	{
		const { document, root } = createDOM();
		const app = createTodoApp(fns, root, document);

		app.addItem("Toggle me");

		const itemId = app.itemIdAt(0);
		assert(app.itemCompletedAt(0), false, "item not completed initially");

		// Toggle to completed
		app.toggleItem(itemId);

		assert(app.itemCompletedAt(0), true, "item completed after toggle");

		const li = root.querySelector("li");
		assert(li !== null, true, "li exists after toggle");

		// Check that the span text has the ✓ indicator
		const span = li!.querySelector("span");
		const spanText = span!.textContent || "";
		assert(spanText.includes("✓"), true, 'completed item text includes "✓"');

		// Check that the li has the "completed" class
		const liClass = li!.getAttribute("class") || "";
		assert(liClass.includes("completed"), true, 'li has "completed" class');

		// Toggle back
		app.toggleItem(itemId);
		assert(
			app.itemCompletedAt(0),
			false,
			"item uncompleted after second toggle",
		);

		const li2 = root.querySelector("li");
		const span2 = li2!.querySelector("span");
		const spanText2 = span2!.textContent || "";
		assert(
			spanText2.includes("✓"),
			false,
			'uncompleted item text does not include "✓"',
		);

		app.destroy();
	}

	suite("Todo — remove item removes li from DOM");
	{
		const { document, root } = createDOM();
		const app = createTodoApp(fns, root, document);

		app.addItem("Keep");
		app.addItem("Remove me");
		app.addItem("Also keep");

		assert(root.querySelectorAll("li").length, 3, "3 items before remove");

		const removeId = app.itemIdAt(1);
		app.removeItem(removeId);

		assert(app.itemCount(), 2, "WASM has 2 items after remove");

		const lis = root.querySelectorAll("li");
		assert(lis.length, 2, "2 li elements after remove");

		app.destroy();
	}

	suite("Todo — remove all items leaves empty ul");
	{
		const { document, root } = createDOM();
		const app = createTodoApp(fns, root, document);

		app.addItem("A");
		app.addItem("B");

		const id1 = app.itemIdAt(0);
		const id2 = app.itemIdAt(1);

		app.removeItem(id1);
		app.removeItem(id2);

		assert(app.itemCount(), 0, "0 items after removing all");

		const lis = root.querySelectorAll("li");
		assert(lis.length, 0, "0 li elements in DOM");

		app.destroy();
	}

	suite("Todo — add after remove works correctly");
	{
		const { document, root } = createDOM();
		const app = createTodoApp(fns, root, document);

		app.addItem("First");
		const firstId = app.itemIdAt(0);
		app.removeItem(firstId);

		assert(app.itemCount(), 0, "0 items after remove");
		assert(root.querySelectorAll("li").length, 0, "0 li after remove");

		app.addItem("Second");
		assert(app.itemCount(), 1, "1 item after re-add");
		assert(root.querySelectorAll("li").length, 1, "1 li after re-add");

		const span = root.querySelector("li span");
		assert(
			span!.textContent!.includes("Second"),
			true,
			'new item text is "Second"',
		);

		app.destroy();
	}

	suite("Todo — list version increments on each mutation");
	{
		const { document, root } = createDOM();
		const app = createTodoApp(fns, root, document);

		assert(app.listVersion(), 0, "version starts at 0");

		app.addItem("V1");
		assert(app.listVersion(), 1, "version is 1 after add");

		app.addItem("V2");
		assert(app.listVersion(), 2, "version is 2 after second add");

		const id = app.itemIdAt(0);
		app.toggleItem(id);
		assert(app.listVersion(), 3, "version is 3 after toggle");

		app.removeItem(id);
		assert(app.listVersion(), 4, "version is 4 after remove");

		app.destroy();
	}

	suite("Todo — 10 items added and all rendered");
	{
		const { document, root } = createDOM();
		const app = createTodoApp(fns, root, document);

		for (let i = 0; i < 10; i++) {
			app.addItem(`Item ${i}`);
		}

		assert(app.itemCount(), 10, "WASM has 10 items");

		const lis = root.querySelectorAll("li");
		assert(lis.length, 10, "10 li elements in DOM");

		// Check first and last
		const firstSpan = lis[0].querySelector("span");
		assert(
			firstSpan!.textContent!.includes("Item 0"),
			true,
			"first item text correct",
		);

		const lastSpan = lis[9].querySelector("span");
		assert(
			lastSpan!.textContent!.includes("Item 9"),
			true,
			"last item text correct",
		);

		app.destroy();
	}

	suite("Todo — toggle multiple items independently");
	{
		const { document, root } = createDOM();
		const app = createTodoApp(fns, root, document);

		app.addItem("Task A");
		app.addItem("Task B");
		app.addItem("Task C");

		const idA = app.itemIdAt(0);
		const idC = app.itemIdAt(2);

		// Toggle A and C, leave B
		app.toggleItem(idA);
		app.toggleItem(idC);

		assert(app.itemCompletedAt(0), true, "A is completed");
		assert(app.itemCompletedAt(1), false, "B is not completed");
		assert(app.itemCompletedAt(2), true, "C is completed");

		const lis = root.querySelectorAll("li");
		assert(lis.length, 3, "still 3 items");

		// Check classes
		const classA = lis[0].getAttribute("class") || "";
		const classB = lis[1].getAttribute("class") || "";
		const classC = lis[2].getAttribute("class") || "";

		assert(classA.includes("completed"), true, "A has completed class");
		assert(
			classB.includes("completed"),
			false,
			"B does not have completed class",
		);
		assert(classC.includes("completed"), true, "C has completed class");

		app.destroy();
	}

	suite("Todo — remove from middle preserves other items");
	{
		const { document, root } = createDOM();
		const app = createTodoApp(fns, root, document);

		app.addItem("First");
		app.addItem("Middle");
		app.addItem("Last");

		const middleId = app.itemIdAt(1);
		app.removeItem(middleId);

		assert(app.itemCount(), 2, "2 items after removing middle");

		const lis = root.querySelectorAll("li");
		assert(lis.length, 2, "2 li elements");

		// The remaining items should be First and Last
		// Note: remove uses swap-remove, so order may change
		// Just verify we have 2 items and neither is "Middle"
		const texts = Array.from(lis).map((li: Element) => {
			const span = li.querySelector("span");
			return span ? span.textContent : "";
		});
		const hasMiddle = texts.some((t: string) => t.includes("Middle"));
		assert(hasMiddle, false, '"Middle" is not in the list');

		app.destroy();
	}

	suite("Todo — multiple independent app instances");
	{
		const dom1 = createDOM();
		const dom2 = createDOM();
		const app1 = createTodoApp(fns, dom1.root, dom1.document);
		const app2 = createTodoApp(fns, dom2.root, dom2.document);

		app1.addItem("App1 item");
		app2.addItem("App2 item A");
		app2.addItem("App2 item B");

		assert(app1.itemCount(), 1, "app1 has 1 item");
		assert(app2.itemCount(), 2, "app2 has 2 items");

		assert(dom1.root.querySelectorAll("li").length, 1, "app1 DOM has 1 li");
		assert(dom2.root.querySelectorAll("li").length, 2, "app2 DOM has 2 li");

		app1.destroy();
		app2.destroy();
	}

	suite("Todo — rapid 50 adds");
	{
		const { document, root } = createDOM();
		const app = createTodoApp(fns, root, document);

		for (let i = 0; i < 50; i++) {
			app.addItem(`Rapid ${i}`);
		}

		assert(app.itemCount(), 50, "50 items after rapid adds");
		assert(root.querySelectorAll("li").length, 50, "50 li elements in DOM");

		app.destroy();
	}

	suite("Todo — add, toggle, remove interleaved");
	{
		const { document, root } = createDOM();
		const app = createTodoApp(fns, root, document);

		app.addItem("One");
		app.addItem("Two");
		app.addItem("Three");

		// Toggle "Two"
		const twoId = app.itemIdAt(1);
		app.toggleItem(twoId);
		assert(app.itemCompletedAt(1), true, '"Two" is completed');

		// Remove "One"
		const oneId = app.itemIdAt(0);
		app.removeItem(oneId);
		assert(app.itemCount(), 2, "2 items after removing One");

		// Add a new item
		app.addItem("Four");
		assert(app.itemCount(), 3, "3 items after adding Four");

		const lis = root.querySelectorAll("li");
		assert(lis.length, 3, "3 li elements in DOM");

		app.destroy();
	}

	suite("Todo — set_input stores text (no re-render)");
	{
		const appPtr = fns.todo_init();
		const versionBefore = fns.todo_list_version(appPtr);
		fns.todo_set_input(appPtr, writeStringStruct("hello"));
		const versionAfter = fns.todo_list_version(appPtr);
		assert(versionAfter, versionBefore, "set_input does not change version");

		const dirty = fns.todo_has_dirty(appPtr);
		assert(dirty, 0, "set_input does not make scope dirty");

		fns.todo_destroy(appPtr);
	}

	suite("Todo — remove nonexistent item is a no-op");
	{
		const { document, root } = createDOM();
		const app = createTodoApp(fns, root, document);

		app.addItem("Only item");
		app.removeItem(9999); // nonexistent ID

		assert(app.itemCount(), 1, "item count unchanged");
		assert(root.querySelectorAll("li").length, 1, "DOM unchanged");

		app.destroy();
	}

	suite("Todo — toggle nonexistent item is a no-op");
	{
		const { document, root } = createDOM();
		const app = createTodoApp(fns, root, document);

		app.addItem("Only item");
		const vBefore = app.listVersion();
		app.toggleItem(9999); // nonexistent ID

		assert(app.itemCompletedAt(0), false, "item unchanged");
		// Version should not change for nonexistent toggle
		// (the toggle function only bumps if it finds the item)
		const vAfter = app.listVersion();
		assert(vAfter, vBefore, "version unchanged for nonexistent toggle");

		app.destroy();
	}

	suite("Todo — flush with no dirty returns 0");
	{
		const { document, root } = createDOM();
		const app = createTodoApp(fns, root, document);

		// Nothing dirty after initial mount
		const len = app.flush();
		assert(len, 0, "flush returns 0 when not dirty");

		app.destroy();
	}

	suite("Todo — DOM structure: each li has span + 2 buttons");
	{
		const { document, root } = createDOM();
		const app = createTodoApp(fns, root, document);

		app.addItem("Structure test");

		const li = root.querySelector("li")!;
		const children = li.childNodes;

		// li > span + button + button
		let spanCount = 0;
		let buttonCount = 0;
		for (let i = 0; i < children.length; i++) {
			const child = children[i] as Element;
			if (child.tagName === "SPAN") spanCount++;
			if (child.tagName === "BUTTON") buttonCount++;
		}

		assert(spanCount, 1, "li has 1 span");
		assert(buttonCount, 2, "li has 2 buttons");

		app.destroy();
	}
}

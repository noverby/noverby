// App Shell — Orchestrates WASM runtime, DOM interpreter, and event bridge.
//
// Provides `createCounterApp()` for headless (test) usage and
// `mountCounterApp()` for browser usage.  Both wire together:
//
//   1. WASM counter app exports (init, rebuild, handle_event, flush)
//   2. TemplateCache (registers templates from WASM structure queries)
//   3. Interpreter (applies binary mutation buffers to DOM)
//   4. EventBridge (delegates DOM events → WASM dispatch → flush → apply)
//
// The mutation buffer is allocated once in WASM linear memory and reused
// for every rebuild/flush cycle.
//
// Note: We build the template DOM manually rather than using
// TemplateCache.registerFromWasm() because Mojo's String return ABI
// (sret pointer) is not directly callable from JS.  The counter
// template structure is known at compile time anyway.

import { EventBridge, EventType } from "./events.ts";
import { Interpreter } from "./interpreter.ts";
import { alignedAlloc, getMemory } from "./memory.ts";
import { TemplateCache } from "./templates.ts";
import type { WasmExports } from "./types.ts";

// ── Constants ───────────────────────────────────────────────────────────────

/** Default mutation buffer size (16 KiB). */
const BUF_CAPACITY = 16384;

// ── CounterApp handle ───────────────────────────────────────────────────────

/**
 * A fully wired counter app instance.
 *
 * Returned by `createCounterApp`.  Exposes methods for programmatic
 * interaction (useful in tests) and the underlying subsystems for
 * inspection.
 */
export interface CounterAppHandle {
	/** The WASM exports object. */
	fns: WasmExports;

	/** The DOM interpreter. */
	interpreter: Interpreter;

	/** The event bridge (if installed). */
	events: EventBridge;

	/** The template cache. */
	templates: TemplateCache;

	/** The root DOM element. */
	root: Element;

	/** The WASM-side app pointer. */
	appPtr: bigint;

	/** Pointer to the mutation buffer in WASM memory. */
	bufPtr: bigint;

	/** Increment handler ID (for programmatic dispatch). */
	incrHandler: number;

	/** Decrement handler ID (for programmatic dispatch). */
	decrHandler: number;

	/** Read the current count value from WASM. */
	getCount(): number;

	/** Simulate an increment click (dispatch + flush + apply). */
	increment(): void;

	/** Simulate a decrement click (dispatch + flush + apply). */
	decrement(): void;

	/** Dispatch an event by handler ID and flush. */
	dispatchAndFlush(handlerId: number, eventType?: number): void;

	/** Destroy the app and free WASM resources. */
	destroy(): void;
}

// ── App factory ─────────────────────────────────────────────────────────────

/**
 * Create a counter app wired to a DOM root element.
 *
 * This is the headless-compatible version (no browser required).
 * Pass a `Document` from linkedom/deno-dom for testing.
 *
 * @param fns     - Instantiated WASM exports.
 * @param root    - The mount-point DOM element.
 * @param doc     - The Document to use for DOM operations.
 * @param install - Whether to install DOM event delegation (default: false).
 */
export function createCounterApp(
	fns: WasmExports,
	root: Element,
	doc?: Document,
	install = false,
): CounterAppHandle {
	const document = doc ?? root.ownerDocument!;

	// 1. Initialize WASM-side app
	const appPtr = fns.counter_init();
	const tmplId = fns.counter_tmpl_id(appPtr);
	const incrHandler = fns.counter_incr_handler(appPtr);
	const decrHandler = fns.counter_decr_handler(appPtr);

	// 2. Create template cache and register the counter template manually.
	//
	//    The counter template structure (from Mojo):
	//      div
	//        span
	//          <empty text node>         ← dynamic_text[0] slot
	//        button
	//          "+"
	//          (dynamic_attr[0] slot)    ← onclick
	//        button
	//          "-"
	//          (dynamic_attr[1] slot)    ← onclick
	//
	//    We build this DOM tree directly instead of calling
	//    registerFromWasm() to avoid the Mojo String sret ABI issue.
	const templates = new TemplateCache(document);
	{
		const div = document.createElement("div");

		const span = document.createElement("span");
		span.appendChild(document.createTextNode("")); // dynamic text placeholder
		div.appendChild(span);

		const btnIncr = document.createElement("button");
		btnIncr.appendChild(document.createTextNode("+"));
		div.appendChild(btnIncr);

		const btnDecr = document.createElement("button");
		btnDecr.appendChild(document.createTextNode("-"));
		div.appendChild(btnDecr);

		templates.register(tmplId, [div]);
	}

	// 3. Create interpreter
	const interpreter = new Interpreter(root, templates, document);

	// 4. Allocate a mutation buffer in WASM memory
	const bufPtr = alignedAlloc(8n, BigInt(BUF_CAPACITY));

	// 5. Initial mount — rebuild
	const mountLen = fns.counter_rebuild(appPtr, bufPtr, BUF_CAPACITY);
	if (mountLen > 0) {
		const mem = getMemory();
		interpreter.applyMutations(mem.buffer, Number(bufPtr), mountLen);
	}

	// 6. Create event bridge.
	//    We pass a fresh Map here — EventBridge.findElementId uses it for
	//    delegation-based dispatch, but our counter app wires events via
	//    Interpreter.onNewListener instead, so it's not needed.
	const nodeMap: Map<number, Node> = new Map();
	const events = new EventBridge(root, nodeMap);

	// Wire up the interpreter's onNewListener to register handler mappings
	// in the EventBridge.  The NewEventListener mutation carries the element ID
	// and event name.  We look up the handler ID from the WASM app state by
	// matching the event name to our known handlers.
	//
	// For the counter app we know:
	//   - The "+" button gets incrHandler
	//   - The "−" button gets decrHandler
	// We track the order of listener registrations to assign them correctly.
	let listenerIndex = 0;
	const handlerOrder = [incrHandler, decrHandler];

	interpreter.onNewListener = (
		elementId: number,
		eventName: string,
	): EventListener => {
		// Map this element+event to the correct handler ID
		const hid = handlerOrder[listenerIndex] ?? incrHandler;
		listenerIndex++;
		events.addHandler(elementId, eventName, hid);

		// Return a no-op listener (the EventBridge handles delegation)
		return () => {};
	};

	// 7. Set up dispatch functions on the bridge
	events.setDispatch(
		(handlerId: number, eventType: number) => {
			return fns.counter_handle_event(appPtr, handlerId, eventType);
		},
		(handlerId: number, eventType: number, _value: number) => {
			return fns.counter_handle_event(appPtr, handlerId, eventType);
		},
	);

	// After-dispatch callback: flush and apply mutations
	events.onAfterDispatch = () => {
		flushAndApply();
	};

	// 8. Optionally install DOM event delegation
	if (install) {
		events.install();
	}

	// ── Helpers ───────────────────────────────────────────────────────

	function flushAndApply(): void {
		const len = fns.counter_flush(appPtr, bufPtr, BUF_CAPACITY);
		if (len > 0) {
			const mem = getMemory();
			interpreter.applyMutations(mem.buffer, Number(bufPtr), len);
		}
	}

	function dispatchAndFlush(
		handlerId: number,
		eventType: number = EventType.Click,
	): void {
		fns.counter_handle_event(appPtr, handlerId, eventType);
		flushAndApply();
	}

	// ── Public handle ─────────────────────────────────────────────────

	return {
		fns,
		interpreter,
		events,
		templates,
		root,
		appPtr,
		bufPtr,
		incrHandler,
		decrHandler,

		getCount(): number {
			return fns.counter_count_value(appPtr);
		},

		increment(): void {
			dispatchAndFlush(incrHandler);
		},

		decrement(): void {
			dispatchAndFlush(decrHandler);
		},

		dispatchAndFlush,

		destroy(): void {
			events.uninstall();
			fns.counter_destroy(appPtr);
		},
	};
}

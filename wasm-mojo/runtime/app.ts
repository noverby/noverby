// App Shell — Orchestrates WASM runtime, DOM interpreter, and event bridge.
//
// Provides a generic `createApp()` factory that wires together:
//
//   1. WASM app exports (init, rebuild, handle_event, flush, destroy)
//   2. TemplateCache (templates auto-registered from WASM via RegisterTemplate mutations)
//   3. Interpreter (applies binary mutation buffers to DOM)
//   4. EventBridge (delegates DOM events → WASM dispatch → flush → apply)
//
// The mutation buffer is allocated once in WASM linear memory and reused
// for every rebuild/flush cycle.
//
// Templates are automatically registered from WASM via RegisterTemplate
// mutations prepended to the mount buffer by AppShell.emit_templates().
// No manual DOM template construction is needed.
//
// App-specific factories (`createCounterApp`, etc.) are thin wrappers
// that pass the correct WASM export names and add app-specific helpers
// (e.g. `getCount`, `increment`, `addItem`).

import { EventBridge, EventType } from "./events.ts";
import { Interpreter } from "./interpreter.ts";
import { alignedAlloc, getMemory } from "./memory.ts";
import { TemplateCache } from "./templates.ts";
import type { WasmExports } from "./types.ts";

// ── Constants ───────────────────────────────────────────────────────────────

/** Default mutation buffer size (16 KiB). */
const DEFAULT_BUF_CAPACITY = 16384;

// ── Generic App types ───────────────────────────────────────────────────────

/**
 * Configuration for the generic `createApp()` factory.
 *
 * Each field wraps a WASM export call so that `createApp` doesn't need
 * to know the specific export names (counter_init vs todo_init, etc.).
 */
export interface AppConfig {
	/** Instantiated WASM exports. */
	fns: WasmExports;

	/** The mount-point DOM element. */
	root: Element;

	/** The Document to use for DOM operations (for headless testing). */
	doc?: Document;

	/** Mutation buffer capacity in bytes (default: 16384). */
	bufCapacity?: number;

	/** Whether to install DOM event delegation (default: false). */
	install?: boolean;

	/** Initialize the WASM-side app.  Returns the app pointer. */
	init: (fns: WasmExports) => bigint;

	/** Initial render (mount).  Returns byte length of mutation data. */
	rebuild: (
		fns: WasmExports,
		appPtr: bigint,
		bufPtr: bigint,
		capacity: number,
	) => number;

	/** Flush pending updates.  Returns byte length of mutation data (0 = nothing dirty). */
	flush: (
		fns: WasmExports,
		appPtr: bigint,
		bufPtr: bigint,
		capacity: number,
	) => number;

	/** Dispatch an event to the WASM app.  Returns 1 if handled, 0 otherwise. */
	handleEvent: (
		fns: WasmExports,
		appPtr: bigint,
		handlerId: number,
		eventType: number,
	) => number;

	/** Destroy the WASM-side app and free resources. */
	destroy: (fns: WasmExports, appPtr: bigint) => void;
}

/**
 * A fully wired app instance returned by `createApp()`.
 *
 * Provides the common lifecycle methods (dispatch, flush, destroy) and
 * exposes the underlying subsystems for inspection and extension.
 */
export interface AppHandle {
	/** The WASM exports object. */
	fns: WasmExports;

	/** The DOM interpreter. */
	interpreter: Interpreter;

	/** The event bridge. */
	events: EventBridge;

	/** The template cache. */
	templates: TemplateCache;

	/** The root DOM element. */
	root: Element;

	/** The WASM-side app pointer. */
	appPtr: bigint;

	/** Pointer to the mutation buffer in WASM memory. */
	bufPtr: bigint;

	/** Mutation buffer capacity in bytes. */
	bufCapacity: number;

	/** Dispatch an event by handler ID and flush + apply any mutations. */
	dispatchAndFlush(handlerId: number, eventType?: number): void;

	/** Flush pending updates and apply mutations to the DOM. */
	flushAndApply(): void;

	/** Destroy the app and free WASM resources. */
	destroy(): void;
}

// ── Generic App factory ─────────────────────────────────────────────────────

/**
 * Create a WASM app wired to a DOM root element.
 *
 * This is the generic, headless-compatible version (no browser required).
 * Pass a `Document` from linkedom/deno-dom for testing.
 *
 * Templates are automatically registered from WASM via RegisterTemplate
 * mutations — no manual DOM template construction needed.  Handler IDs
 * flow through the mutation protocol via the `handlerId` parameter on
 * `onNewListener` — no manual handler ordering needed.
 */
export function createApp(config: AppConfig): AppHandle {
	const {
		fns,
		root,
		init,
		rebuild,
		flush,
		handleEvent,
		destroy: destroyApp,
		install = false,
		bufCapacity = DEFAULT_BUF_CAPACITY,
	} = config;
	const document = config.doc ?? root.ownerDocument!;

	// 1. Initialize WASM-side app
	const appPtr = init(fns);

	// 2. Create empty template cache — templates come from WASM via
	//    RegisterTemplate mutations prepended to the mount buffer.
	const templates = new TemplateCache(document);

	// 3. Create interpreter
	const interpreter = new Interpreter(root, templates, document);

	// 4. Allocate a mutation buffer in WASM memory
	const bufPtr = alignedAlloc(8n, BigInt(bufCapacity));

	// 5. Create event bridge and wire onNewListener BEFORE mount so that
	//    NewEventListener mutations in the mount buffer are captured.
	const nodeMap: Map<number, Node> = new Map();
	const events = new EventBridge(root, nodeMap);

	// Handler IDs come from the mutation protocol — the Interpreter's
	// onNewListener callback receives (elementId, eventName, handlerId)
	// directly from the NewEventListener mutation.  No manual ordering needed.
	interpreter.onNewListener = (
		elementId: number,
		eventName: string,
		handlerId: number,
	): EventListener => {
		events.addHandler(elementId, eventName, handlerId);
		// Return a no-op listener (the EventBridge handles delegation)
		return () => {};
	};

	// Set up dispatch functions on the bridge
	events.setDispatch(
		(handlerId: number, eventType: number) => {
			return handleEvent(fns, appPtr, handlerId, eventType);
		},
		(handlerId: number, eventType: number, _value: number) => {
			return handleEvent(fns, appPtr, handlerId, eventType);
		},
	);

	// After-dispatch callback: flush and apply mutations
	events.onAfterDispatch = () => {
		flushAndApply();
	};

	// 6. Initial mount — rebuild (RegisterTemplate + LoadTemplate + events in one pass)
	const mountLen = rebuild(fns, appPtr, bufPtr, bufCapacity);
	if (mountLen > 0) {
		const mem = getMemory();
		interpreter.applyMutations(mem.buffer, Number(bufPtr), mountLen);
	}

	// 7. Optionally install DOM event delegation
	if (install) {
		events.install();
	}

	// ── Helpers ───────────────────────────────────────────────────────

	function flushAndApply(): void {
		const len = flush(fns, appPtr, bufPtr, bufCapacity);
		if (len > 0) {
			const mem = getMemory();
			interpreter.applyMutations(mem.buffer, Number(bufPtr), len);
		}
	}

	function dispatchAndFlush(
		handlerId: number,
		eventType: number = EventType.Click,
	): void {
		handleEvent(fns, appPtr, handlerId, eventType);
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
		bufCapacity,
		dispatchAndFlush,
		flushAndApply,

		destroy(): void {
			events.uninstall();
			destroyApp(fns, appPtr);
		},
	};
}

// ── CounterApp handle ───────────────────────────────────────────────────────

/**
 * A fully wired counter app instance.
 *
 * Returned by `createCounterApp`.  Extends `AppHandle` with
 * counter-specific methods for programmatic interaction (useful in tests).
 */
export interface CounterAppHandle extends AppHandle {
	/** Increment handler ID (for programmatic dispatch). */
	incrHandler: number;

	/** Decrement handler ID (for programmatic dispatch). */
	decrHandler: number;

	/** Read the current count value from WASM. */
	getCount(): number;

	/** Read the current doubled memo value from WASM. */
	getDoubled(): number;

	/** Simulate an increment click (dispatch + flush + apply). */
	increment(): void;

	/** Simulate a decrement click (dispatch + flush + apply). */
	decrement(): void;
}

// ── Counter App factory ─────────────────────────────────────────────────────

/**
 * Create a counter app wired to a DOM root element.
 *
 * Thin wrapper around `createApp()` that adds counter-specific helpers:
 * `getCount()`, `increment()`, `decrement()`, and handler ID accessors.
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
	const handle = createApp({
		fns,
		root,
		doc,
		install,
		init: (f) => f.counter_init(),
		rebuild: (f, app, buf, cap) => f.counter_rebuild(app, buf, cap),
		flush: (f, app, buf, cap) => f.counter_flush(app, buf, cap),
		handleEvent: (f, app, hid, evt) => f.counter_handle_event(app, hid, evt),
		destroy: (f, app) => f.counter_destroy(app),
	});

	const incrHandler = fns.counter_incr_handler(handle.appPtr);
	const decrHandler = fns.counter_decr_handler(handle.appPtr);

	return {
		...handle,
		incrHandler,
		decrHandler,

		getCount(): number {
			return fns.counter_count_value(handle.appPtr);
		},

		getDoubled(): number {
			return fns.counter_doubled_value(handle.appPtr);
		},

		increment(): void {
			handle.dispatchAndFlush(incrHandler);
		},

		decrement(): void {
			handle.dispatchAndFlush(decrHandler);
		},
	};
}

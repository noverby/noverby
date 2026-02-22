// App Launcher — Generic boot sequence for wasm-mojo examples.
//
// Convention-based WASM export discovery: given an app name (e.g. "counter"),
// the launcher looks for exports named `{name}_init`, `{name}_rebuild`,
// `{name}_flush`, and optionally `{name}_handle_event` and `{name}_dispatch_string`.
//
// This abstraction captures the common boot sequence shared by all standard
// wasm-mojo apps (load WASM → init → interpreter → EventBridge → mount).
// App-specific post-boot wiring is supported via the `onBoot` callback.
//
// The goal is convergence: as more features move into WASM (e.g. keydown
// handlers, event delegation), the `onBoot` hooks shrink and eventually
// all apps use an identical main.js:
//
//   import { launch } from "../lib/app.js";
//   launch({ app: "myapp", wasm: new URL("../../build/out.wasm", import.meta.url) });
//
// Usage:
//
//   // Counter — zero-config launch
//   launch({
//     app: "counter",
//     wasm: new URL("../../build/out.wasm", import.meta.url),
//   });
//
//   // Todo — zero-config launch (Enter key handled in WASM via onkeydown_enter_custom)
//   launch({
//     app: "todo",
//     wasm: new URL("../../build/out.wasm", import.meta.url),
//   });
//
//   // Bench — launch with onBoot for custom event delegation & toolbar wiring
//   launch({
//     app: "bench",
//     wasm: new URL("../../build/out.wasm", import.meta.url),
//     root: "#tbody",
//     bufferCapacity: 8 * 1024 * 1024,
//     clearRoot: false,
//     onBoot: ({ fns, appPtr, rootEl, flush }) => { /* toolbar + delegation */ },
//   });

import { alignedAlloc, getMemory, loadWasm } from "./env.js";
import { EventBridge } from "./events.js";
import { Interpreter } from "./interpreter.js";
import { writeStringStruct } from "./strings.js";

// ── Defaults ────────────────────────────────────────────────────────────────

/** Default mutation buffer capacity (64 KiB — generous for most apps). */
const DEFAULT_BUF_CAPACITY = 65536;

/** Default root element selector. */
const DEFAULT_ROOT_SELECTOR = "#root";

/** Default event type constant (click = 0). */
const EVT_CLICK = 0;

// ── Types (documented via JSDoc) ────────────────────────────────────────────

/**
 * @typedef {Object} LaunchOptions
 * @property {string}           app            - App name prefix for WASM export discovery.
 * @property {URL|string}       wasm           - URL to the .wasm file.
 * @property {string}           [root="#root"] - CSS selector for the mount-point element.
 * @property {number}           [bufferCapacity=65536] - Mutation buffer size in bytes.
 * @property {boolean}          [clearRoot=true] - Whether to clear the root element before mount.
 * @property {(handle: AppHandle) => void} [onBoot] - Callback after mount for app-specific wiring.
 */

/**
 * @typedef {Object} AppHandle
 * @property {Object}  fns     - WASM instance exports.
 * @property {bigint}  appPtr  - Pointer to the WASM-side app state.
 * @property {Object}  interp  - The DOM interpreter instance.
 * @property {bigint}  bufPtr  - Pointer to the mutation buffer in WASM memory.
 * @property {number}  bufferCapacity - Mutation buffer capacity in bytes.
 * @property {Element} rootEl  - The mount-point DOM element.
 * @property {() => void} flush - Flush pending WASM updates and apply mutations to the DOM.
 */

// ── Launcher ────────────────────────────────────────────────────────────────

/**
 * Boot a wasm-mojo app with convention-based WASM export discovery.
 *
 * Given `app: "counter"`, discovers exports:
 *   - `counter_init() -> appPtr`            (required)
 *   - `counter_rebuild(appPtr, buf, cap)`   (required)
 *   - `counter_flush(appPtr, buf, cap)`     (required)
 *   - `counter_handle_event(appPtr, hid, evt)` (optional — enables EventBridge dispatch)
 *   - `counter_dispatch_string(appPtr, hid, evt, strPtr)` (optional — enables string dispatch)
 *
 * When `{app}_handle_event` is missing, the EventBridge is still created
 * (so NewEventListener mutations are processed and DOM listeners attached)
 * but the dispatch callback is a no-op.  This allows apps that use custom
 * event delegation (e.g. bench) to wire their own handlers via `onBoot`
 * while still benefiting from the shared boot sequence.
 *
 * When `{app}_dispatch_string` exists, the EventBridge automatically
 * routes string-carrying events through the string dispatch path:
 *   - `input`/`change` events: extracts `event.target.value` → WASM SignalString
 *   - `keydown` events: extracts `event.key` → WASM key filtering
 *     (ACTION_KEY_ENTER_CUSTOM checks for "Enter"; accepted keys also
 *     dispatch through handle_event for app-level routing)
 *
 * This enables Dioxus-style two-way input binding and WASM-driven
 * keyboard shortcuts with zero app-specific JS.
 *
 * @param {LaunchOptions} options
 * @returns {Promise<AppHandle>} Resolves after mount (and onBoot if provided).
 */
export async function launch(options) {
	const {
		app: appName,
		wasm: wasmUrl,
		root: rootSelector = DEFAULT_ROOT_SELECTOR,
		bufferCapacity = DEFAULT_BUF_CAPACITY,
		clearRoot = true,
		onBoot = null,
	} = options;

	const rootEl = document.querySelector(rootSelector);
	if (!rootEl) {
		throw new Error(
			`launch("${appName}"): root element "${rootSelector}" not found`,
		);
	}

	try {
		// 1. Load WASM and discover exports by naming convention
		const fns = await loadWasm(wasmUrl);

		const initFn = fns[`${appName}_init`];
		const rebuildFn = fns[`${appName}_rebuild`];
		const flushFn = fns[`${appName}_flush`];
		const handleEventFn = fns[`${appName}_handle_event`]; // optional
		const dispatchStringFn = fns[`${appName}_dispatch_string`]; // optional

		if (!initFn) {
			throw new Error(`WASM export "${appName}_init" not found`);
		}
		if (!rebuildFn) {
			throw new Error(`WASM export "${appName}_rebuild" not found`);
		}
		if (!flushFn) {
			throw new Error(`WASM export "${appName}_flush" not found`);
		}

		// 2. Initialize WASM-side app
		const appPtr = initFn();

		// 3. Prepare DOM — clear loading indicator and create interpreter
		if (clearRoot) {
			rootEl.innerHTML = "";
		}
		const interp = new Interpreter(rootEl, new Map());
		const bufPtr = alignedAlloc(8n, BigInt(bufferCapacity));

		// 4. Flush helper — reusable by EventBridge and onBoot hook
		function flush() {
			const len = flushFn(appPtr, bufPtr, bufferCapacity);
			if (len > 0) {
				const mem = getMemory();
				interp.applyMutations(mem.buffer, Number(bufPtr), len);
			}
		}

		// 5. Wire events via EventBridge with smart dispatch
		//    - If `{app}_handle_event` is missing, EventBridge dispatch is a no-op
		//      (DOM listeners are still attached so NewEventListener mutations work;
		//      apps like bench wire their own handlers via onBoot)
		//    - If `{app}_dispatch_string` exists:
		//      · input/change → string dispatch (event.target.value → WASM SignalString)
		//      · keydown → string dispatch (event.key → WASM key filtering);
		//        if accepted (e.g. Enter), also call handle_event for app routing
		//    - All other events use normal numeric dispatch
		new EventBridge(interp, (handlerId, eventName, domEvent) => {
			if (
				dispatchStringFn &&
				(eventName === "input" || eventName === "change") &&
				domEvent?.target?.value !== undefined
			) {
				const strPtr = writeStringStruct(domEvent.target.value);
				dispatchStringFn(appPtr, handlerId, EVT_CLICK, strPtr);
			} else if (
				dispatchStringFn &&
				eventName === "keydown" &&
				domEvent?.key !== undefined
			) {
				// Send the key name through string dispatch for WASM-side filtering.
				// ACTION_KEY_ENTER_CUSTOM checks value == "Enter" and marks scope dirty.
				// If accepted (returns 1), also call handle_event for app-level routing.
				const strPtr = writeStringStruct(domEvent.key);
				const accepted = dispatchStringFn(appPtr, handlerId, EVT_CLICK, strPtr);
				if (accepted && handleEventFn) {
					handleEventFn(appPtr, handlerId, EVT_CLICK);
				}
			} else if (handleEventFn) {
				handleEventFn(appPtr, handlerId, EVT_CLICK);
			}
			flush();
		});

		// 6. Initial mount (RegisterTemplate + LoadTemplate + events in one pass)
		const mountLen = rebuildFn(appPtr, bufPtr, bufferCapacity);
		if (mountLen > 0) {
			const mem = getMemory();
			interp.applyMutations(mem.buffer, Number(bufPtr), mountLen);
		}

		// 7. Build app handle for onBoot callback and return value
		const handle = {
			fns,
			appPtr,
			interp,
			bufPtr,
			bufferCapacity,
			rootEl,
			flush,
		};

		// 8. App-specific post-boot wiring
		if (onBoot) onBoot(handle);

		console.log(`🔥 Mojo ${appName} app running!`);
		return handle;
	} catch (err) {
		console.error(`Failed to boot ${appName}:`, err);
		rootEl.innerHTML = `<p class="error">Failed to load: ${err.message}</p>`;
		throw err;
	}
}

// Todo App — Browser Entry Point
//
// Uses shared runtime from examples/lib/ for WASM env, protocol, and interpreter.
// Uses EventBridge for automatic event wiring via handler IDs in the mutation protocol.
// Templates are automatically registered from WASM via RegisterTemplate mutations.
//
// Phase 20.5 — Fully WASM-driven Add flow:
//   - Input events dispatch via string dispatch path (oninput_set_string)
//   - Add button click dispatches normally (onclick_custom → WASM reads signal)
//   - JS has NO special-casing for any handler — uniform dispatch for all events
//   - Enter key dispatches the Add handler directly (signal already has current text)
//
// Flow:
//   1. Load WASM via shared loadWasm()
//   2. Initialize todo app in WASM (runtime, signals, handlers, templates)
//   3. Create interpreter with empty template map (templates come from WASM)
//   4. Wire EventBridge for automatic event dispatch (with string dispatch)
//   5. Apply initial mount mutations (templates + events wired in one pass)
//   6. User interactions → EventBridge → WASM dispatch → flush → apply mutations → DOM updated
//
// Event flow (M20.5 — no special-casing):
//   - Input keystrokes → string dispatch → WASM oninput_set_string → signal updated
//   - "Add" button click → normal dispatch → WASM onclick_custom → reads signal, adds item, clears signal
//   - "✓" button click → normal dispatch → WASM get_action → toggle item
//   - "✕" button click → normal dispatch → WASM get_action → remove item
//   - Enter key in input → dispatches Add handler directly → same as Add button

import {
	allocBuffer,
	applyMutations,
	createInterpreter,
	EventBridge,
	loadWasm,
	writeStringStruct,
} from "../lib/boot.js";

const BUF_CAPACITY = 65536;
const EVT_CLICK = 0;

async function boot() {
	const rootEl = document.getElementById("root");

	try {
		const fns = await loadWasm(
			new URL("../../build/out.wasm", import.meta.url),
		);

		// 1. Initialize todo app in WASM
		const appPtr = fns.todo_init();
		const addHandlerId = fns.todo_add_handler_id(appPtr);

		// 2. Clear loading indicator and create interpreter (empty — templates come from WASM)
		rootEl.innerHTML = "";
		const interp = createInterpreter(rootEl, new Map());
		const bufPtr = allocBuffer(BUF_CAPACITY);

		// Helper: flush WASM state and apply mutations to DOM
		function flush() {
			const len = fns.todo_flush(appPtr, bufPtr, BUF_CAPACITY);
			if (len > 0) applyMutations(interp, bufPtr, len);
		}

		// 3. Wire events via EventBridge — uniform dispatch for all handlers
		//    Input/change events use string dispatch (oninput_set_string);
		//    all other events use normal dispatch.
		new EventBridge(interp, (handlerId, eventName, domEvent) => {
			if (
				(eventName === "input" || eventName === "change") &&
				domEvent?.target?.value !== undefined
			) {
				// String dispatch: extract input value → WASM signal
				const strPtr = writeStringStruct(domEvent.target.value);
				fns.todo_dispatch_string(appPtr, handlerId, EVT_CLICK, strPtr);
			} else {
				// Normal dispatch: click, etc. → WASM handle_event
				fns.todo_handle_event(appPtr, handlerId, EVT_CLICK);
			}
			flush();
		});

		// 4. Initial mount (RegisterTemplate + LoadTemplate + events in one pass)
		const mountLen = fns.todo_rebuild(appPtr, bufPtr, BUF_CAPACITY);
		if (mountLen > 0) applyMutations(interp, bufPtr, mountLen);

		// 5. Wire up input field for Enter key → dispatch Add handler directly
		//    The signal already has the current input text (from oninput_set_string),
		//    so we just trigger the Add action — WASM reads it and clears the signal.
		const inputEl = rootEl.querySelector("input");
		if (inputEl) {
			inputEl.addEventListener("keydown", (e) => {
				if (e.key === "Enter") {
					fns.todo_handle_event(appPtr, addHandlerId, EVT_CLICK);
					flush();
				}
			});
		}

		console.log("🔥 Mojo Todo app running!");
	} catch (err) {
		console.error("Failed to boot:", err);
		rootEl.innerHTML = `<p class="error">Failed to load: ${err.message}</p>`;
	}
}

boot();

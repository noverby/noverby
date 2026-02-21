// Todo App — Browser Entry Point
//
// Uses shared runtime from examples/lib/ for WASM env, protocol, and interpreter.
// Uses EventBridge for automatic event wiring via handler IDs in the mutation protocol.
// Templates are automatically registered from WASM via RegisterTemplate mutations.
//
// Flow:
//   1. Load WASM via shared loadWasm()
//   2. Initialize todo app in WASM (runtime, signals, handlers, templates)
//   3. Create interpreter with empty template map (templates come from WASM)
//   4. Wire EventBridge for automatic event dispatch
//   5. Apply initial mount mutations (templates + events wired in one pass)
//   6. User interactions → EventBridge → WASM dispatch → flush → apply mutations → DOM updated
//
// Event flow:
//   - "Add" button click → read input value → todo_add_item(text) → todo_flush
//   - "✓" button click → handler ID dispatched directly via EventBridge
//   - "✕" button click → handler ID dispatched directly via EventBridge
//   - Enter key in input → same as Add button

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
		const addHandlerId = fns.todo_add_handler(appPtr);

		// 2. Clear loading indicator and create interpreter (empty — templates come from WASM)
		rootEl.innerHTML = "";
		const interp = createInterpreter(rootEl, new Map());
		const bufPtr = allocBuffer(BUF_CAPACITY);

		// Helper: read input value and add a todo item
		let inputEl = null;
		function addItem() {
			if (!inputEl) inputEl = rootEl.querySelector("input");
			if (!inputEl) return;
			const text = inputEl.value.trim();
			if (!text) return;
			const strPtr = writeStringStruct(text);
			fns.todo_add_item(appPtr, strPtr);
			inputEl.value = "";
			flush();
		}

		function flush() {
			const len = fns.todo_flush(appPtr, bufPtr, BUF_CAPACITY);
			if (len > 0) applyMutations(interp, bufPtr, len);
		}

		// 3. Wire events via EventBridge — handler IDs come from the mutation protocol
		new EventBridge(interp, (handlerId, _eventName, _domEvent) => {
			// The "Add" button handler needs special treatment: read the input value first
			if (handlerId === addHandlerId) {
				addItem();
				return;
			}

			// All other handlers (toggle, remove) dispatch directly
			fns.todo_handle_event(appPtr, handlerId, EVT_CLICK);
			flush();
		});

		// 4. Initial mount (RegisterTemplate + LoadTemplate + events in one pass)
		const mountLen = fns.todo_rebuild(appPtr, bufPtr, BUF_CAPACITY);
		if (mountLen > 0) applyMutations(interp, bufPtr, mountLen);

		// 5. Wire up input field for Enter key
		inputEl = rootEl.querySelector("input");
		if (inputEl) {
			inputEl.addEventListener("keydown", (e) => {
				if (e.key === "Enter") addItem();
			});
		}

		console.log("🔥 Mojo Todo app running!");
	} catch (err) {
		console.error("Failed to boot:", err);
		rootEl.innerHTML = `<p class="error">Failed to load: ${err.message}</p>`;
	}
}

boot();

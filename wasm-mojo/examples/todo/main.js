// Todo App — Browser Entry Point
//
// Uses shared launch() from examples/lib/ for convention-based boot.
// All WASM exports are discovered automatically by the "todo" prefix:
//   todo_init, todo_rebuild, todo_flush, todo_handle_event, todo_dispatch_string
//
// The presence of todo_dispatch_string enables automatic string dispatch
// for input/change events (Dioxus-style two-way binding via oninput_set_string).
//
// The only app-specific JS is the Enter key shortcut — wired via onBoot.
// As keydown event handling moves into WASM, this hook will disappear and
// the todo main.js will become identical to the counter main.js.

import { launch } from "../lib/app.js";

launch({
	app: "todo",
	wasm: new URL("../../build/out.wasm", import.meta.url),
	onBoot({ fns, appPtr, flush, rootEl }) {
		// Wire Enter key on the input field → dispatch Add handler directly.
		// The input signal already has the current text (from oninput_set_string),
		// so we just trigger the Add action — WASM reads it and clears the signal.
		const addHandlerId = fns.todo_add_handler_id(appPtr);
		const inputEl = rootEl.querySelector("input");
		if (inputEl) {
			inputEl.addEventListener("keydown", (e) => {
				if (e.key === "Enter") {
					fns.todo_handle_event(appPtr, addHandlerId, 0);
					flush();
				}
			});
		}
	},
});

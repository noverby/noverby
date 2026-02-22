// Benchmark App — Browser Entry Point
//
// Uses shared launch() from examples/lib/ for convention-based boot.
// All WASM exports are discovered automatically by the "bench" prefix:
//   bench_init, bench_rebuild, bench_flush
//
// Unlike counter and todo, bench does not export bench_handle_event —
// it uses manual event delegation on <tbody> and direct WASM calls for
// each benchmark operation.  The launch() abstraction handles this
// gracefully: when {app}_handle_event is missing, EventBridge dispatch
// is a no-op (DOM listeners are still attached for NewEventListener
// mutations).  All app-specific wiring is done in the onBoot callback.
//
// Phase 23: Bench converged to launch() — same boot infrastructure as
// counter and todo, with onBoot for toolbar buttons, event delegation,
// and timing display.
//
// ── Phase 24 TODO: Eliminate onBoot (zero app-specific JS) ──────────
//
// Each task is independent and incrementally removes JS from onBoot.
// After all four, bench/main.js becomes identical to counter/main.js.
//
// TODO(P24.1): Add bench_handle_event with handler_map dispatch.
//   The KeyedList handler_map already stores BENCH_ACTION_SELECT (1) and
//   BENCH_ACTION_REMOVE (2) with row IDs via add_custom_event().  Add a
//   handle_event(handler_id) method to BenchmarkApp that calls
//   rows_list.get_action(handler_id) and routes to select_row/remove_row
//   (same pattern as TodoApp.handle_event).  Add bench_handle_event WASM
//   export in main.mojo.  This lets EventBridge dispatch row clicks
//   directly — eliminates the tbody event delegation JS (lines 47–67).
//   ~15 lines Mojo + export wrapper.  No runtime changes needed.
//
// TODO(P24.2): WASM-rendered toolbar with onclick_custom handlers.
//   Move the toolbar (h1, 6 buttons, status div, table) into the WASM
//   template tree so buttons get onclick_custom handlers auto-wired by
//   EventBridge.  Change root from "#tbody" to "#root".  Extend
//   handle_event to route each button's handler ID to the corresponding
//   operation (create 1k/10k, append, update, swap, clear).  Needs a way
//   to distinguish buttons — either: (a) one handler ID per button with
//   hardcoded routing, or (b) new onclick_custom_data(operand) DSL helper
//   that stores an Int32 payload retrievable via handler action lookup.
//   Eliminates toolbar button wiring JS (lines 70–111).
//
// TODO(P24.3): performance.now() WASM import for timing.
//   Add a `performance_now() -> Float64` import to env.js and a
//   corresponding Mojo FFI declaration.  Add a timeOp-style wrapper in
//   BenchmarkApp that calls performance_now before/after each operation
//   and stores the result in a SignalString for the status display.
//   Requires float-to-string formatting with 1 decimal place (verify
//   Mojo WASM target support or write a simple manual formatter).
//   Eliminates the timeOp/setStatus JS (lines 36–44).
//
// TODO(P24.4): Status bar as WASM template with dynamic text.
//   Include the status bar in the WASM template (part of P24.2 template
//   restructure).  Use dyn_text nodes for operation name, timing, and
//   row count — replaces innerHTML with proper element + SignalString
//   updates.  After this + P24.1–P24.3, onBoot is empty and bench
//   main.js reduces to: launch({ app: "bench", wasm: ... }).

import { launch } from "../lib/app.js";

const BUF_CAPACITY = 8 * 1024 * 1024; // 8 MB mutation buffer

launch({
	app: "bench",
	wasm: new URL("../../build/out.wasm", import.meta.url),
	root: "#tbody",
	bufferCapacity: BUF_CAPACITY,
	clearRoot: false,
	onBoot: ({ fns, appPtr, rootEl, flush }) => {
		const statusEl = document.getElementById("status");

		function setStatus(text) {
			statusEl.innerHTML = text;
		}

		function timeOp(name, fn) {
			const start = performance.now();
			fn();
			const ms = (performance.now() - start).toFixed(1);
			setStatus(
				`<strong>${name}</strong>: <span class="timing">${ms}ms</span> — ${fns.bench_row_count(appPtr)} rows`,
			);
		}

		// Event delegation on tbody (rootEl)
		rootEl.addEventListener("click", (e) => {
			const a = e.target.closest("a");
			if (!a) return;
			const tr = a.closest("tr");
			if (!tr) return;

			const idText = tr.querySelector("td")?.textContent;
			if (!idText) return;
			const rowId = parseInt(idText, 10);
			if (Number.isNaN(rowId)) return;

			if (a.classList.contains("remove")) {
				timeOp("Remove row", () => {
					fns.bench_remove(appPtr, rowId);
					flush();
				});
			} else {
				timeOp("Select row", () => {
					fns.bench_select(appPtr, rowId);
					flush();
				});
			}
		});

		// Wire toolbar buttons
		document.getElementById("btn-create1k").onclick = () => {
			timeOp("Create 1,000 rows", () => {
				fns.bench_create(appPtr, 1000);
				flush();
			});
		};

		document.getElementById("btn-create10k").onclick = () => {
			timeOp("Create 10,000 rows", () => {
				fns.bench_create(appPtr, 10000);
				flush();
			});
		};

		document.getElementById("btn-append").onclick = () => {
			timeOp("Append 1,000 rows", () => {
				fns.bench_append(appPtr, 1000);
				flush();
			});
		};

		document.getElementById("btn-update").onclick = () => {
			timeOp("Update every 10th", () => {
				fns.bench_update(appPtr);
				flush();
			});
		};

		document.getElementById("btn-swap").onclick = () => {
			timeOp("Swap rows", () => {
				fns.bench_swap(appPtr);
				flush();
			});
		};

		document.getElementById("btn-clear").onclick = () => {
			timeOp("Clear", () => {
				fns.bench_clear(appPtr);
				flush();
			});
		};

		setStatus("Ready — click a button to start benchmarking");
	},
});

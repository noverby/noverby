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

// Benchmark App — Browser Entry Point
//
// Uses shared launch() from examples/lib/ for convention-based boot.
// All WASM exports are discovered automatically by the "bench" prefix:
//   bench_init, bench_rebuild, bench_flush, bench_handle_event
//
// Phase 24.2: The entire app shell (heading, toolbar buttons, status bar,
// table structure) is now rendered from WASM via register_view() with
// onclick_custom() handlers.  EventBridge dispatches all button and row
// clicks directly — zero app-specific JS needed.
//
// The only bench-specific config is bufferCapacity (8 MB for large row sets).
//
// ── Phase 24 TODO: Remaining steps toward full parity ───────────────
//
// TODO(P24.3): performance.now() WASM import for timing.
//   Add a `performance_now() -> Float64` import to env.js and a
//   corresponding Mojo FFI declaration.  Add a timeOp-style wrapper in
//   BenchmarkApp that calls performance_now before/after each operation
//   and stores the result in a SignalString for the status display.
//   Requires float-to-string formatting with 1 decimal place (verify
//   Mojo WASM target support or write a simple manual formatter).
//   Eliminates the need for any JS-side timing code.
//
// TODO(P24.4): Status bar as WASM template with dynamic text.
//   The status bar div is already in the WASM template (P24.2).
//   Use dyn_text nodes for operation name, timing, and row count —
//   replaces the static "Ready" text with proper SignalString updates.
//   After P24.3 + P24.4, bench main.js is identical to counter/todo.

import { launch } from "../lib/app.js";

const BUF_CAPACITY = 8 * 1024 * 1024; // 8 MB mutation buffer

launch({
	app: "bench",
	wasm: new URL("../../build/out.wasm", import.meta.url),
	bufferCapacity: BUF_CAPACITY,
});

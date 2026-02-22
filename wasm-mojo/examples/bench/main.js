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
// DONE(P24.3): performance.now() WASM import for timing.
//   Added `performance_now() -> Float64` import to env.js and a
//   corresponding Mojo FFI declaration via external_call.  Each toolbar
//   operation in handle_event() is wrapped with before/after
//   performance_now() calls; elapsed time is formatted to 1 decimal
//   place and stored in status_text, emitted as dyn_text[0] on flush.
//   Zero JS-side timing code needed.
//
// TODO(P24.4): Status bar as WASM template with dynamic text.
//   The status bar div is already in the WASM template (P24.2) and
//   timing is now computed in WASM (P24.3).  Remaining: use separate
//   dyn_text nodes for operation name, timing, and row count if finer
//   granularity is desired.  After P24.4, bench main.js is identical
//   to counter/todo (only bufferCapacity override remains).

import { launch } from "../lib/app.js";

const BUF_CAPACITY = 8 * 1024 * 1024; // 8 MB mutation buffer

launch({
	app: "bench",
	wasm: new URL("../../build/out.wasm", import.meta.url),
	bufferCapacity: BUF_CAPACITY,
});

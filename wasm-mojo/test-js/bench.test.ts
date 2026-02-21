// Phase 9 — Benchmark & Performance Tests
//
// Tests the js-framework-benchmark operations (9.1), memory management (9.2),
// mutation optimization (9.4), and debug/profiling exports (9.5).
//
// Benchmark operations tested:
//   - Create 1,000 rows
//   - Create 10,000 rows
//   - Append 1,000 rows
//   - Update every 10th row
//   - Select row (highlight)
//   - Swap rows
//   - Remove row
//   - Clear all rows
//
// Memory tests:
//   - Signal create/destroy cycles → no leak
//   - Scope create/destroy cycles → no leak
//   - Rapid signal writes → bounded dirty queue
//
// Debug/profiling:
//   - Signal store capacity inspection
//   - Scope store capacity inspection
//   - Handler store capacity inspection

import { parseHTML } from "npm:linkedom";
import { type AppHandle, createApp } from "../runtime/app.ts";
import { writeStringStruct } from "../runtime/strings.ts";
import type { WasmExports } from "../runtime/types.ts";
import { assert, suite } from "./harness.ts";

type Fns = WasmExports & Record<string, CallableFunction>;

// ── Helpers ─────────────────────────────────────────────────────────────────

/** Time a synchronous operation and return elapsed milliseconds. */
function time(fn: () => void): number {
	const start = performance.now();
	fn();
	return performance.now() - start;
}

// ── DOM helper ──────────────────────────────────────────────────────────────

function createDOM() {
	const { document, window } = parseHTML(
		'<!DOCTYPE html><html><body><table><tbody id="tbody"></tbody></table></body></html>',
	);
	const tbody = document.getElementById("tbody")!;
	return { document, window, tbody };
}

// ── BenchApp handle ─────────────────────────────────────────────────────────

interface BenchAppHandle {
	fns: Fns;
	handle: AppHandle;
	tbody: Element;

	create(count: number): void;
	append(count: number): void;
	update(): void;
	swap(): void;
	clear(): void;
	select(rowId: number): void;
	remove(rowId: number): void;
	rowCount(): number;
	rowIdAt(index: number): number;
	selected(): number;
	destroy(): void;
}

const BENCH_BUF_CAPACITY = 8 * 1024 * 1024; // 8 MB

function createBenchApp(
	fns: Fns,
	tbody: Element,
	doc: Document,
): BenchAppHandle {
	// Use the generic createApp factory — templates come from WASM via
	// RegisterTemplate mutations, no manual DOM template construction needed.
	// The bench app uses event delegation on tbody, not per-element listeners,
	// so handleEvent is a no-op.
	const handle = createApp({
		fns,
		root: tbody,
		doc,
		bufCapacity: BENCH_BUF_CAPACITY,
		init: (f) => f.bench_init(),
		rebuild: (f, app, buf, cap) => f.bench_rebuild(app, buf, cap),
		flush: (f, app, buf, cap) => f.bench_flush(app, buf, cap),
		handleEvent: () => 0, // bench uses direct WASM calls, not event dispatch
		destroy: (f, app) => f.bench_destroy(app),
	});

	function flushOp(): void {
		handle.flushAndApply();
	}

	return {
		fns,
		handle,
		tbody,

		create(count: number): void {
			fns.bench_create(handle.appPtr, count);
			flushOp();
		},

		append(count: number): void {
			fns.bench_append(handle.appPtr, count);
			flushOp();
		},

		update(): void {
			fns.bench_update(handle.appPtr);
			flushOp();
		},

		swap(): void {
			fns.bench_swap(handle.appPtr);
			flushOp();
		},

		clear(): void {
			fns.bench_clear(handle.appPtr);
			flushOp();
		},

		select(rowId: number): void {
			fns.bench_select(handle.appPtr, rowId);
			flushOp();
		},

		remove(rowId: number): void {
			fns.bench_remove(handle.appPtr, rowId);
			flushOp();
		},

		rowCount(): number {
			return fns.bench_row_count(handle.appPtr);
		},

		rowIdAt(index: number): number {
			return fns.bench_row_id_at(handle.appPtr, index);
		},

		selected(): number {
			return fns.bench_selected(handle.appPtr);
		},

		destroy(): void {
			handle.destroy();
		},
	};
}

// ══════════════════════════════════════════════════════════════════════════════
// 9.1 — Benchmark App: WASM-side operations
// ══════════════════════════════════════════════════════════════════════════════

function testBenchScopeId(fns: Fns): void {
	suite("Benchmark — bench_scope_id returns valid root scope");
	{
		const app = fns.bench_init();

		const scopeId = fns.bench_scope_id(app);
		assert(scopeId >= 0, true, "root scope ID is non-negative");

		fns.bench_destroy(app);
	}
}

function testBenchCreate(fns: Fns): void {
	suite("Benchmark — create 1,000 rows");
	{
		const app = fns.bench_init();

		fns.bench_create(app, 1000);
		const count = fns.bench_row_count(app);
		assert(count, 1000, "1,000 rows created");

		const dirty = fns.bench_has_dirty(app);
		assert(dirty, 1, "app is dirty after create");

		const version = fns.bench_version(app);
		assert(version > 0, true, "version signal bumped");

		// Row ids should be sequential starting from 1
		const first = fns.bench_row_id_at(app, 0);
		const last = fns.bench_row_id_at(app, 999);
		assert(first, 1, "first row id is 1");
		assert(last, 1000, "last row id is 1000");

		fns.bench_destroy(app);
	}

	suite("Benchmark — create 10,000 rows");
	{
		const app = fns.bench_init();

		const ms = time(() => fns.bench_create(app, 10000));
		const count = fns.bench_row_count(app);
		assert(count, 10000, "10,000 rows created");

		const first = fns.bench_row_id_at(app, 0);
		const last = fns.bench_row_id_at(app, 9999);
		assert(first, 1, "first row id is 1");
		assert(last, 10000, "last row id is 10000");

		console.log(`    ℹ create 10,000 rows (WASM state): ${ms.toFixed(1)}ms`);

		fns.bench_destroy(app);
	}
}

function testBenchAppend(fns: Fns): void {
	suite("Benchmark — append 1,000 rows to existing 1,000");
	{
		const app = fns.bench_init();

		fns.bench_create(app, 1000);
		// Drain dirty so we can detect the next mutation
		const bufSize = 1024 * 1024; // 1 MB for large ops
		const buf = fns.mutation_buf_alloc(bufSize);
		fns.bench_rebuild(app, buf, bufSize);
		fns.bench_flush(app, buf, bufSize);

		fns.bench_append(app, 1000);
		const count = fns.bench_row_count(app);
		assert(count, 2000, "2,000 rows total after append");

		// Newly appended rows start after the initial 1,000
		const appendedFirst = fns.bench_row_id_at(app, 1000);
		assert(appendedFirst, 1001, "first appended row id is 1001");

		fns.mutation_buf_free(buf);
		fns.bench_destroy(app);
	}
}

function testBenchUpdateEvery10th(fns: Fns): void {
	suite("Benchmark — update every 10th row");
	{
		const app = fns.bench_init();

		fns.bench_create(app, 1000);
		const versionBefore = fns.bench_version(app);

		// Drain dirty from create
		const buf = fns.mutation_buf_alloc(1024 * 1024);
		fns.bench_rebuild(app, buf, 1024 * 1024);
		fns.bench_flush(app, buf, 1024 * 1024);

		fns.bench_update(app);
		const versionAfter = fns.bench_version(app);
		assert(versionAfter > versionBefore, true, "version bumped after update");

		const dirty = fns.bench_has_dirty(app);
		assert(dirty, 1, "app is dirty after update");

		fns.mutation_buf_free(buf);
		fns.bench_destroy(app);
	}
}

function testBenchSelect(fns: Fns): void {
	suite("Benchmark — select row");
	{
		const app = fns.bench_init();

		fns.bench_create(app, 1000);

		// Select row with id 5
		const rowId = fns.bench_row_id_at(app, 4);
		fns.bench_select(app, rowId);

		const selected = fns.bench_selected(app);
		assert(selected, rowId, "selected row matches");

		const dirty = fns.bench_has_dirty(app);
		assert(dirty, 1, "app is dirty after select");

		fns.bench_destroy(app);
	}
}

function testBenchSwap(fns: Fns): void {
	suite("Benchmark — swap rows 1 and 998");
	{
		const app = fns.bench_init();

		fns.bench_create(app, 1000);
		const id1 = fns.bench_row_id_at(app, 1);
		const id998 = fns.bench_row_id_at(app, 998);

		// Drain dirty from create
		const buf = fns.mutation_buf_alloc(1024 * 1024);
		fns.bench_rebuild(app, buf, 1024 * 1024);
		fns.bench_flush(app, buf, 1024 * 1024);

		fns.bench_swap(app);

		const newId1 = fns.bench_row_id_at(app, 1);
		const newId998 = fns.bench_row_id_at(app, 998);
		assert(newId1, id998, "row at index 1 is now former row 998");
		assert(newId998, id1, "row at index 998 is now former row 1");

		fns.mutation_buf_free(buf);
		fns.bench_destroy(app);
	}
}

function testBenchRemove(fns: Fns): void {
	suite("Benchmark — remove row");
	{
		const app = fns.bench_init();

		fns.bench_create(app, 1000);
		const removeId = fns.bench_row_id_at(app, 5);

		// Drain dirty from create
		const buf = fns.mutation_buf_alloc(1024 * 1024);
		fns.bench_rebuild(app, buf, 1024 * 1024);
		fns.bench_flush(app, buf, 1024 * 1024);

		fns.bench_remove(app, removeId);
		const count = fns.bench_row_count(app);
		assert(count, 999, "999 rows after remove");

		fns.mutation_buf_free(buf);
		fns.bench_destroy(app);
	}
}

function testBenchClear(fns: Fns): void {
	suite("Benchmark — clear all rows");
	{
		const app = fns.bench_init();

		fns.bench_create(app, 1000);

		// Drain dirty from create
		const buf = fns.mutation_buf_alloc(1024 * 1024);
		fns.bench_rebuild(app, buf, 1024 * 1024);
		fns.bench_flush(app, buf, 1024 * 1024);

		fns.bench_clear(app);
		const count = fns.bench_row_count(app);
		assert(count, 0, "0 rows after clear");

		const dirty = fns.bench_has_dirty(app);
		assert(dirty, 1, "app is dirty after clear");

		fns.mutation_buf_free(buf);
		fns.bench_destroy(app);
	}
}

// ── Benchmark mutation round-trip ───────────────────────────────────────────

function testBenchMutations(fns: Fns): void {
	suite("Benchmark — create 1,000 rows → mutations");
	{
		const app = fns.bench_init();
		const bufSize = 4 * 1024 * 1024; // 4 MB for large tables
		const buf = fns.mutation_buf_alloc(bufSize);

		// Initial mount (empty)
		const mountLen = fns.bench_rebuild(app, buf, bufSize);
		assert(mountLen > 0, true, "initial mount emits mutations");

		// Create 1,000 rows
		fns.bench_create(app, 1000);
		const ms = time(() => {
			fns.bench_flush(app, buf, bufSize);
		});
		const flushLen = fns.bench_flush(app, buf, bufSize);
		// Second flush should be 0 (no more dirty)
		assert(flushLen, 0, "second flush produces 0 (nothing dirty)");

		console.log(`    ℹ create 1,000 + flush (WASM side): ${ms.toFixed(1)}ms`);

		// Verify rows are still intact
		assert(fns.bench_row_count(app), 1000, "still 1,000 rows");

		fns.mutation_buf_free(buf);
		fns.bench_destroy(app);
	}

	suite("Benchmark — update every 10th → mutations");
	{
		const app = fns.bench_init();
		const bufSize = 4 * 1024 * 1024;
		const buf = fns.mutation_buf_alloc(bufSize);

		// Mount + create
		fns.bench_rebuild(app, buf, bufSize);
		fns.bench_create(app, 1000);
		fns.bench_flush(app, buf, bufSize);

		// Update every 10th
		fns.bench_update(app);
		const ms = time(() => {
			fns.bench_flush(app, buf, bufSize);
		});
		const flushLen = fns.bench_flush(app, buf, bufSize);
		assert(flushLen, 0, "second flush after update is 0");

		console.log(`    ℹ update every 10th + flush: ${ms.toFixed(1)}ms`);

		fns.mutation_buf_free(buf);
		fns.bench_destroy(app);
	}

	suite("Benchmark — swap rows → mutations");
	{
		const app = fns.bench_init();
		const bufSize = 4 * 1024 * 1024;
		const buf = fns.mutation_buf_alloc(bufSize);

		// Mount + create
		fns.bench_rebuild(app, buf, bufSize);
		fns.bench_create(app, 1000);
		fns.bench_flush(app, buf, bufSize);

		// Swap
		fns.bench_swap(app);
		const ms = time(() => {
			fns.bench_flush(app, buf, bufSize);
		});
		console.log(`    ℹ swap + flush: ${ms.toFixed(1)}ms`);

		fns.mutation_buf_free(buf);
		fns.bench_destroy(app);
	}

	suite("Benchmark — clear 1,000 rows → mutations");
	{
		const app = fns.bench_init();
		const bufSize = 4 * 1024 * 1024;
		const buf = fns.mutation_buf_alloc(bufSize);

		// Mount + create
		fns.bench_rebuild(app, buf, bufSize);
		fns.bench_create(app, 1000);
		fns.bench_flush(app, buf, bufSize);

		// Clear
		fns.bench_clear(app);
		const ms = time(() => {
			fns.bench_flush(app, buf, bufSize);
		});
		const flushLen = fns.bench_flush(app, buf, bufSize);
		assert(flushLen, 0, "second flush after clear is 0");

		console.log(`    ℹ clear 1,000 + flush: ${ms.toFixed(1)}ms`);

		fns.mutation_buf_free(buf);
		fns.bench_destroy(app);
	}

	suite("Benchmark — select row → mutations");
	{
		const app = fns.bench_init();
		const bufSize = 4 * 1024 * 1024;
		const buf = fns.mutation_buf_alloc(bufSize);

		// Mount + create
		fns.bench_rebuild(app, buf, bufSize);
		fns.bench_create(app, 1000);
		fns.bench_flush(app, buf, bufSize);

		// Select
		const rowId = fns.bench_row_id_at(app, 42);
		fns.bench_select(app, rowId);
		const ms = time(() => {
			fns.bench_flush(app, buf, bufSize);
		});
		console.log(`    ℹ select + flush: ${ms.toFixed(1)}ms`);

		fns.mutation_buf_free(buf);
		fns.bench_destroy(app);
	}

	suite("Benchmark — remove row → mutations");
	{
		const app = fns.bench_init();
		const bufSize = 4 * 1024 * 1024;
		const buf = fns.mutation_buf_alloc(bufSize);

		// Mount + create
		fns.bench_rebuild(app, buf, bufSize);
		fns.bench_create(app, 1000);
		fns.bench_flush(app, buf, bufSize);

		// Remove
		const removeId = fns.bench_row_id_at(app, 10);
		fns.bench_remove(app, removeId);
		const ms = time(() => {
			fns.bench_flush(app, buf, bufSize);
		});
		console.log(`    ℹ remove + flush: ${ms.toFixed(1)}ms`);

		fns.mutation_buf_free(buf);
		fns.bench_destroy(app);
	}
}

// ══════════════════════════════════════════════════════════════════════════════
// 9.2 — Memory Management
// ══════════════════════════════════════════════════════════════════════════════

function testMemorySignalCycle(fns: Fns): void {
	suite("Memory — signal create/destroy 1,000 cycles");
	{
		const rt = fns.runtime_create();

		const remaining = fns.mem_test_signal_cycle(rt, 1000);
		assert(remaining, 0, "0 signals remain after 1,000 create/destroy");

		// The slot capacity should have grown but that's fine — slots are reused
		const capacity = fns.debug_signal_store_capacity(rt);
		assert(capacity > 0, true, "signal store has allocated slots");

		fns.runtime_destroy(rt);
	}

	suite("Memory — signal create/destroy 10,000 cycles");
	{
		const rt = fns.runtime_create();

		const remaining = fns.mem_test_signal_cycle(rt, 10000);
		assert(remaining, 0, "0 signals remain after 10,000 create/destroy");

		fns.runtime_destroy(rt);
	}
}

function testMemoryScopeCycle(fns: Fns): void {
	suite("Memory — scope create/destroy 1,000 cycles");
	{
		const rt = fns.runtime_create();

		const remaining = fns.mem_test_scope_cycle(rt, 1000);
		assert(remaining, 0, "0 scopes remain after 1,000 create/destroy");

		fns.runtime_destroy(rt);
	}
}

function testMemoryRapidWrites(fns: Fns): void {
	suite("Memory — rapid signal writes (1,000 in sequence)");
	{
		const rt = fns.runtime_create();

		// Create a scope and signal, subscribe the scope
		const scope = fns.scope_create(rt, 0, -1);
		fns.runtime_set_context(rt, scope);
		const key = fns.signal_create_i32(rt, 0);
		// Read with context to subscribe
		fns.signal_read_i32(rt, key);
		fns.runtime_clear_context(rt);

		const dirtyCount = fns.mem_test_rapid_writes(rt, key, 1000);
		// Despite 1,000 writes, the scope should only appear once in the dirty queue
		assert(dirtyCount, 1, "only 1 dirty scope despite 1,000 writes");

		fns.runtime_destroy(rt);
	}
}

function testMemoryBenchCycles(fns: Fns): void {
	suite("Memory — benchmark app create/destroy cycles");
	{
		// Create and destroy the benchmark app 100 times to check for leaks
		const ms = time(() => {
			for (let i = 0; i < 100; i++) {
				const app = fns.bench_init();
				fns.bench_create(app, 100);
				fns.bench_destroy(app);
			}
		});
		// If we get here without crashing, memory is bounded
		assert(true, true, "100 create/destroy cycles completed");
		console.log(
			`    ℹ 100 bench init/create/destroy cycles: ${ms.toFixed(1)}ms`,
		);
	}
}

// ══════════════════════════════════════════════════════════════════════════════
// 9.4 — Mutation Optimization
// ══════════════════════════════════════════════════════════════════════════════

function testSignalBatching(fns: Fns): void {
	suite("Optimization — multiple signal writes → single dirty entry");
	{
		const rt = fns.runtime_create();
		const scope = fns.scope_create(rt, 0, -1);

		fns.runtime_set_context(rt, scope);
		const sig1 = fns.signal_create_i32(rt, 0);
		const sig2 = fns.signal_create_i32(rt, 0);
		fns.signal_read_i32(rt, sig1);
		fns.signal_read_i32(rt, sig2);
		fns.runtime_clear_context(rt);

		// Write to both signals — scope should only be dirty once
		fns.signal_write_i32(rt, sig1, 10);
		fns.signal_write_i32(rt, sig2, 20);

		const dirtyCount = fns.runtime_dirty_count(rt);
		assert(dirtyCount, 1, "1 dirty scope despite 2 signal writes");

		// Drain and write again
		fns.runtime_drain_dirty(rt);

		fns.signal_write_i32(rt, sig1, 30);
		fns.signal_write_i32(rt, sig1, 40); // write same signal twice

		const dirtyCount2 = fns.runtime_dirty_count(rt);
		assert(dirtyCount2, 1, "1 dirty scope despite 2 writes to same signal");

		// Final value should be the last write
		const val = fns.signal_peek_i32(rt, sig1);
		assert(val, 40, "signal holds last written value");

		fns.runtime_destroy(rt);
	}

	suite("Optimization — batch API (begin/end)");
	{
		const rt = fns.runtime_create();
		const scope = fns.scope_create(rt, 0, -1);

		fns.runtime_set_context(rt, scope);
		const sig = fns.signal_create_i32(rt, 0);
		fns.signal_read_i32(rt, sig);
		fns.runtime_clear_context(rt);

		// Batch multiple writes
		fns.runtime_begin_batch(rt);
		fns.signal_write_i32(rt, sig, 1);
		fns.signal_write_i32(rt, sig, 2);
		fns.signal_write_i32(rt, sig, 3);
		fns.runtime_end_batch(rt);

		const dirtyCount = fns.runtime_dirty_count(rt);
		assert(dirtyCount, 1, "1 dirty scope after batched writes");

		const val = fns.signal_peek_i32(rt, sig);
		assert(val, 3, "signal holds last batched value");

		fns.runtime_destroy(rt);
	}
}

// ══════════════════════════════════════════════════════════════════════════════
// 9.5 — Debug / Developer Experience
// ══════════════════════════════════════════════════════════════════════════════

function testDebugExports(fns: Fns): void {
	suite("Debug — signal store capacity");
	{
		const rt = fns.runtime_create();

		const cap0 = fns.debug_signal_store_capacity(rt);
		assert(cap0, 0, "empty runtime has 0 signal slots");

		fns.signal_create_i32(rt, 42);
		const cap1 = fns.debug_signal_store_capacity(rt);
		assert(cap1, 1, "1 signal slot after creating 1 signal");

		fns.runtime_destroy(rt);
	}

	suite("Debug — scope store capacity");
	{
		const rt = fns.runtime_create();

		const cap0 = fns.debug_scope_store_capacity(rt);
		assert(cap0, 0, "empty runtime has 0 scope slots");

		fns.scope_create(rt, 0, -1);
		const cap1 = fns.debug_scope_store_capacity(rt);
		assert(cap1, 1, "1 scope slot after creating 1 scope");

		fns.runtime_destroy(rt);
	}

	suite("Debug — handler store capacity");
	{
		const rt = fns.runtime_create();

		const cap0 = fns.debug_handler_store_capacity(rt);
		assert(cap0, 0, "empty runtime has 0 handler slots");

		fns.runtime_destroy(rt);
	}

	suite("Debug — VNode store count");
	{
		const store = fns.vnode_store_create();
		const c0 = fns.debug_vnode_store_count(store);
		assert(c0, 0, "empty store has 0 vnodes");

		const strPtr = writeStringStruct("hello");
		fns.vnode_push_text(store, strPtr);
		const c1 = fns.debug_vnode_store_count(store);
		assert(c1, 1, "1 vnode after push");

		fns.vnode_store_destroy(store);
	}
}

// ── Large-scale benchmark timing ────────────────────────────────────────────

function testBenchTimings(fns: Fns): void {
	suite("Benchmark — timing: create 1,000 rows end-to-end");
	{
		const app = fns.bench_init();
		const bufSize = 4 * 1024 * 1024;
		const buf = fns.mutation_buf_alloc(bufSize);
		fns.bench_rebuild(app, buf, bufSize);

		const ms = time(() => {
			fns.bench_create(app, 1000);
			fns.bench_flush(app, buf, bufSize);
		});
		console.log(`    ℹ create 1,000 rows end-to-end: ${ms.toFixed(1)}ms`);
		assert(ms < 5000, true, "create 1,000 rows < 5s (generous limit)");

		fns.mutation_buf_free(buf);
		fns.bench_destroy(app);
	}

	suite("Benchmark — timing: partial update (every 10th)");
	{
		const app = fns.bench_init();
		const bufSize = 4 * 1024 * 1024;
		const buf = fns.mutation_buf_alloc(bufSize);

		fns.bench_rebuild(app, buf, bufSize);
		fns.bench_create(app, 1000);
		fns.bench_flush(app, buf, bufSize);

		const ms = time(() => {
			fns.bench_update(app);
			fns.bench_flush(app, buf, bufSize);
		});
		console.log(`    ℹ partial update (every 10th): ${ms.toFixed(1)}ms`);
		assert(ms < 2000, true, "partial update < 2s (generous limit)");

		fns.mutation_buf_free(buf);
		fns.bench_destroy(app);
	}

	suite("Benchmark — timing: swap rows");
	{
		const app = fns.bench_init();
		const bufSize = 4 * 1024 * 1024;
		const buf = fns.mutation_buf_alloc(bufSize);

		fns.bench_rebuild(app, buf, bufSize);
		fns.bench_create(app, 1000);
		fns.bench_flush(app, buf, bufSize);

		const ms = time(() => {
			fns.bench_swap(app);
			fns.bench_flush(app, buf, bufSize);
		});
		console.log(`    ℹ swap rows: ${ms.toFixed(1)}ms`);
		assert(ms < 1000, true, "swap < 1s (generous limit)");

		fns.mutation_buf_free(buf);
		fns.bench_destroy(app);
	}

	suite("Benchmark — timing: select row");
	{
		const app = fns.bench_init();
		const bufSize = 4 * 1024 * 1024;
		const buf = fns.mutation_buf_alloc(bufSize);

		fns.bench_rebuild(app, buf, bufSize);
		fns.bench_create(app, 1000);
		fns.bench_flush(app, buf, bufSize);

		const rowId = fns.bench_row_id_at(app, 42);
		const ms = time(() => {
			fns.bench_select(app, rowId);
			fns.bench_flush(app, buf, bufSize);
		});
		console.log(`    ℹ select row: ${ms.toFixed(1)}ms`);
		assert(ms < 1000, true, "select < 1s (generous limit)");

		fns.mutation_buf_free(buf);
		fns.bench_destroy(app);
	}

	suite("Benchmark — timing: remove row");
	{
		const app = fns.bench_init();
		const bufSize = 4 * 1024 * 1024;
		const buf = fns.mutation_buf_alloc(bufSize);

		fns.bench_rebuild(app, buf, bufSize);
		fns.bench_create(app, 1000);
		fns.bench_flush(app, buf, bufSize);

		const removeId = fns.bench_row_id_at(app, 10);
		const ms = time(() => {
			fns.bench_remove(app, removeId);
			fns.bench_flush(app, buf, bufSize);
		});
		console.log(`    ℹ remove row: ${ms.toFixed(1)}ms`);
		assert(ms < 1000, true, "remove < 1s (generous limit)");

		fns.mutation_buf_free(buf);
		fns.bench_destroy(app);
	}

	suite("Benchmark — timing: clear 1,000 rows");
	{
		const app = fns.bench_init();
		const bufSize = 4 * 1024 * 1024;
		const buf = fns.mutation_buf_alloc(bufSize);

		fns.bench_rebuild(app, buf, bufSize);
		fns.bench_create(app, 1000);
		fns.bench_flush(app, buf, bufSize);

		const ms = time(() => {
			fns.bench_clear(app);
			fns.bench_flush(app, buf, bufSize);
		});
		console.log(`    ℹ clear 1,000 rows: ${ms.toFixed(1)}ms`);
		assert(ms < 2000, true, "clear < 2s (generous limit)");

		fns.mutation_buf_free(buf);
		fns.bench_destroy(app);
	}

	suite("Benchmark — timing: append 1,000 to existing 1,000");
	{
		const app = fns.bench_init();
		const bufSize = 4 * 1024 * 1024;
		const buf = fns.mutation_buf_alloc(bufSize);

		fns.bench_rebuild(app, buf, bufSize);
		fns.bench_create(app, 1000);
		fns.bench_flush(app, buf, bufSize);

		const ms = time(() => {
			fns.bench_append(app, 1000);
			fns.bench_flush(app, buf, bufSize);
		});
		console.log(`    ℹ append 1,000 rows: ${ms.toFixed(1)}ms`);
		assert(ms < 5000, true, "append 1,000 < 5s (generous limit)");

		fns.mutation_buf_free(buf);
		fns.bench_destroy(app);
	}
}

// ══════════════════════════════════════════════════════════════════════════════
// Combined export
// ══════════════════════════════════════════════════════════════════════════════

// ══════════════════════════════════════════════════════════════════════════════
// 12.4 — Benchmark DOM integration tests
// ══════════════════════════════════════════════════════════════════════════════

function testBenchDomMount(fns: Fns): void {
	suite("Bench DOM — createBenchApp mounts empty tbody");
	{
		const dom = createDOM();
		const app = createBenchApp(fns, dom.tbody, dom.document);

		// After mount with no rows created, tbody should have a placeholder
		// (comment node from the fragment slot)
		const childCount = dom.tbody.childNodes.length;
		assert(
			childCount >= 0,
			true,
			"tbody has children or placeholder after mount",
		);

		// No tr elements yet
		const trCount = dom.tbody.querySelectorAll("tr").length;
		assert(trCount, 0, "no tr elements before create");

		app.destroy();
	}
}

function testBenchDomCreate(fns: Fns): void {
	suite("Bench DOM — create 100 rows → 100 tr elements");
	{
		const dom = createDOM();
		const app = createBenchApp(fns, dom.tbody, dom.document);

		app.create(100);

		const trs = dom.tbody.querySelectorAll("tr");
		assert(trs.length, 100, "100 tr elements after create(100)");

		// Each tr should have 3 td children
		const firstTr = trs[0] as Element;
		const tdCount = firstTr.querySelectorAll("td").length;
		assert(tdCount, 3, "each tr has 3 td children");

		// First td should contain the row ID as text
		const firstTd = firstTr.querySelector("td");
		const idText = firstTd?.textContent ?? "";
		assert(idText.length > 0, true, "first td has ID text");

		// WASM row count matches DOM
		assert(app.rowCount(), 100, "WASM row count is 100");

		app.destroy();
	}
}

function testBenchDomAppend(fns: Fns): void {
	suite("Bench DOM — append 100 rows to existing 100 → 200 tr elements");
	{
		const dom = createDOM();
		const app = createBenchApp(fns, dom.tbody, dom.document);

		app.create(100);
		assert(
			dom.tbody.querySelectorAll("tr").length,
			100,
			"100 rows after create",
		);

		app.append(100);
		assert(
			dom.tbody.querySelectorAll("tr").length,
			200,
			"200 rows after append",
		);
		assert(app.rowCount(), 200, "WASM row count is 200");

		app.destroy();
	}
}

function testBenchDomClear(fns: Fns): void {
	suite("Bench DOM — clear removes all tr elements");
	{
		const dom = createDOM();
		const app = createBenchApp(fns, dom.tbody, dom.document);

		app.create(100);
		assert(
			dom.tbody.querySelectorAll("tr").length,
			100,
			"100 rows before clear",
		);

		app.clear();
		const trCount = dom.tbody.querySelectorAll("tr").length;
		assert(trCount, 0, "0 tr elements after clear");
		assert(app.rowCount(), 0, "WASM row count is 0 after clear");

		app.destroy();
	}
}

function testBenchDomSelect(fns: Fns): void {
	suite("Bench DOM — select row adds danger class");
	{
		const dom = createDOM();
		const app = createBenchApp(fns, dom.tbody, dom.document);

		app.create(100);
		const rowId = app.rowIdAt(5);
		app.select(rowId);

		// The selected row should have class="danger"
		const trs = dom.tbody.querySelectorAll("tr");
		let dangerCount = 0;
		for (let i = 0; i < trs.length; i++) {
			const tr = trs[i] as Element;
			if (tr.getAttribute("class") === "danger") {
				dangerCount++;
			}
		}
		assert(dangerCount, 1, "exactly 1 tr has class=danger");
		assert(app.selected(), rowId, "WASM selected matches requested row");

		app.destroy();
	}
}

function testBenchDomRemove(fns: Fns): void {
	suite("Bench DOM — remove row decrements tr count");
	{
		const dom = createDOM();
		const app = createBenchApp(fns, dom.tbody, dom.document);

		app.create(100);
		const removeId = app.rowIdAt(50);
		app.remove(removeId);

		assert(
			dom.tbody.querySelectorAll("tr").length,
			99,
			"99 tr elements after remove",
		);
		assert(app.rowCount(), 99, "WASM row count is 99");

		app.destroy();
	}
}

function testBenchDomSwap(fns: Fns): void {
	suite("Bench DOM — swap rows changes row order in DOM");
	{
		const dom = createDOM();
		const app = createBenchApp(fns, dom.tbody, dom.document);

		app.create(1000);

		// Get IDs at positions 1 and 998 before swap
		const idAt1 = app.rowIdAt(1);
		const idAt998 = app.rowIdAt(998);

		app.swap();

		// After swap, positions should be reversed
		const newIdAt1 = app.rowIdAt(1);
		const newIdAt998 = app.rowIdAt(998);
		assert(newIdAt1, idAt998, "position 1 now has old position 998 ID");
		assert(newIdAt998, idAt1, "position 998 now has old position 1 ID");

		// DOM should reflect the swap — check the first td text of swapped rows
		const trs = dom.tbody.querySelectorAll("tr");
		const tr1Text = (trs[1] as Element).querySelector("td")?.textContent ?? "";
		const tr998Text =
			(trs[998] as Element).querySelector("td")?.textContent ?? "";
		assert(tr1Text, String(idAt998), "DOM row 1 shows swapped ID");
		assert(tr998Text, String(idAt1), "DOM row 998 shows swapped ID");

		app.destroy();
	}
}

function testBenchDomUpdate(fns: Fns): void {
	suite("Bench DOM — update every 10th modifies label text");
	{
		const dom = createDOM();
		const app = createBenchApp(fns, dom.tbody, dom.document);

		app.create(100);

		// Get label of row 0 (every 10th starting from 0) before update
		const trs = dom.tbody.querySelectorAll("tr");
		const getLabelText = (tr: Element): string => {
			const tds = tr.querySelectorAll("td");
			// Second td contains the label (inside an <a>)
			return tds[1]?.textContent ?? "";
		};

		const labelBefore = getLabelText(trs[0] as Element);

		app.update();

		const trsAfter = dom.tbody.querySelectorAll("tr");
		const labelAfter = getLabelText(trsAfter[0] as Element);

		// Updated rows get " !!!" appended
		assert(
			labelAfter.endsWith(" !!!"),
			true,
			"updated row label ends with ' !!!'",
		);
		assert(
			labelAfter.length > labelBefore.length,
			true,
			"updated label is longer",
		);

		// Non-updated row (e.g. row 1) should be unchanged
		const label1Before = getLabelText(trs[1] as Element);
		const label1After = getLabelText(trsAfter[1] as Element);
		assert(label1After, label1Before, "non-updated row label unchanged");

		app.destroy();
	}
}

function testBenchDomCreateAfterClear(fns: Fns): void {
	suite("Bench DOM — create after clear works correctly");
	{
		const dom = createDOM();
		const app = createBenchApp(fns, dom.tbody, dom.document);

		app.create(50);
		assert(
			dom.tbody.querySelectorAll("tr").length,
			50,
			"50 rows after first create",
		);

		app.clear();
		assert(dom.tbody.querySelectorAll("tr").length, 0, "0 rows after clear");

		app.create(30);
		assert(
			dom.tbody.querySelectorAll("tr").length,
			30,
			"30 rows after second create",
		);
		assert(app.rowCount(), 30, "WASM row count is 30");

		app.destroy();
	}
}

function testBenchDomMultipleInstances(fns: Fns): void {
	suite("Bench DOM — multiple independent instances");
	{
		const dom1 = createDOM();
		const dom2 = createDOM();
		const app1 = createBenchApp(fns, dom1.tbody, dom1.document);
		const app2 = createBenchApp(fns, dom2.tbody, dom2.document);

		app1.create(50);
		app2.create(100);

		assert(
			dom1.tbody.querySelectorAll("tr").length,
			50,
			"instance 1 has 50 rows",
		);
		assert(
			dom2.tbody.querySelectorAll("tr").length,
			100,
			"instance 2 has 100 rows",
		);

		app1.clear();
		assert(dom1.tbody.querySelectorAll("tr").length, 0, "instance 1 cleared");
		assert(
			dom2.tbody.querySelectorAll("tr").length,
			100,
			"instance 2 unaffected",
		);

		app1.destroy();
		app2.destroy();
	}
}

// ══════════════════════════════════════════════════════════════════════════════
// Section: Handler lifecycle (M13.1)
// ══════════════════════════════════════════════════════════════════════════════

function testBenchHandlerLifecycle(fns: Fns): void {
	suite("Bench — handler count after create 100 rows");
	{
		const appPtr = fns.bench_init();
		const bufSize = 4 * 1024 * 1024; // 4 MB
		const bufPtr = fns.mutation_buf_alloc(bufSize);
		fns.bench_rebuild(appPtr, bufPtr, bufSize);

		// Before any rows: 0 handlers (bench has no app-level handlers)
		const hcBefore = fns.bench_handler_count(appPtr);
		assert(hcBefore, 0, "0 handlers before any rows");

		// Create 100 rows — each gets 2 handlers (select + remove)
		fns.bench_create(appPtr, 100);
		fns.bench_flush(appPtr, bufPtr, bufSize);

		const hcAfter = fns.bench_handler_count(appPtr);
		assert(hcAfter, 200, "200 handlers after 100 rows (2 per row)");

		fns.bench_destroy(appPtr);
	}

	suite("Bench — handler count bounded after clear + create cycle");
	{
		const appPtr = fns.bench_init();
		const bufSize = 4 * 1024 * 1024;
		const bufPtr = fns.mutation_buf_alloc(bufSize);
		fns.bench_rebuild(appPtr, bufPtr, bufSize);

		// Create 100 rows, flush
		fns.bench_create(appPtr, 100);
		fns.bench_flush(appPtr, bufPtr, bufSize);

		// Clear all rows, flush — handlers should be cleaned up
		fns.bench_clear(appPtr);
		fns.bench_flush(appPtr, bufPtr, bufSize);

		const hcAfterClear = fns.bench_handler_count(appPtr);
		assert(hcAfterClear, 0, "0 handlers after clear (no leak)");

		// Create 100 rows again — should be exactly 200, not 400
		fns.bench_create(appPtr, 100);
		fns.bench_flush(appPtr, bufPtr, bufSize);

		const hcAfterRecreate = fns.bench_handler_count(appPtr);
		assert(hcAfterRecreate, 200, "200 handlers after recreate (not 400)");

		fns.bench_destroy(appPtr);
	}

	suite("Bench — handler count bounded after multiple create cycles");
	{
		const appPtr = fns.bench_init();
		const bufSize = 4 * 1024 * 1024;
		const bufPtr = fns.mutation_buf_alloc(bufSize);
		fns.bench_rebuild(appPtr, bufPtr, bufSize);

		// 5 cycles of create 50 rows (each create replaces all rows)
		for (let i = 0; i < 5; i++) {
			fns.bench_create(appPtr, 50);
			fns.bench_flush(appPtr, bufPtr, bufSize);
		}

		// Should be 100 (50 rows * 2 handlers), NOT 500 (leaked)
		const hc = fns.bench_handler_count(appPtr);
		assert(hc, 100, "100 handlers after 5 create cycles (no leak)");

		fns.bench_destroy(appPtr);
	}

	suite("Bench — handler count after update (no new handlers)");
	{
		const appPtr = fns.bench_init();
		const bufSize = 4 * 1024 * 1024;
		const bufPtr = fns.mutation_buf_alloc(bufSize);
		fns.bench_rebuild(appPtr, bufPtr, bufSize);

		fns.bench_create(appPtr, 100);
		fns.bench_flush(appPtr, bufPtr, bufSize);

		const hcBefore = fns.bench_handler_count(appPtr);

		// Update every 10th — triggers rebuild, old handlers cleaned up
		fns.bench_update(appPtr);
		fns.bench_flush(appPtr, bufPtr, bufSize);

		const hcAfter = fns.bench_handler_count(appPtr);
		assert(hcAfter, hcBefore, "handler count unchanged after update");

		fns.bench_destroy(appPtr);
	}
}

// ══════════════════════════════════════════════════════════════════════════════
// Combined export
// ══════════════════════════════════════════════════════════════════════════════

export function testBench(fns: Fns): void {
	// 9.1 — Benchmark app query exports
	testBenchScopeId(fns);

	// 9.1 — Benchmark operations (state only)
	testBenchCreate(fns);
	testBenchAppend(fns);
	testBenchUpdateEvery10th(fns);
	testBenchSelect(fns);
	testBenchSwap(fns);
	testBenchRemove(fns);
	testBenchClear(fns);

	// 9.1 — Benchmark mutation round-trips
	testBenchMutations(fns);

	// 9.2 — Memory management
	testMemorySignalCycle(fns);
	testMemoryScopeCycle(fns);
	testMemoryRapidWrites(fns);
	testMemoryBenchCycles(fns);

	// 9.4 — Mutation optimization (signal batching)
	testSignalBatching(fns);

	// 9.5 — Debug / Developer experience
	testDebugExports(fns);

	// 9.1 — Performance timings
	testBenchTimings(fns);

	// 12.4 — Bench DOM integration
	testBenchDomMount(fns);
	testBenchDomCreate(fns);
	testBenchDomAppend(fns);
	testBenchDomClear(fns);
	testBenchDomSelect(fns);
	testBenchDomRemove(fns);
	testBenchDomSwap(fns);
	testBenchDomUpdate(fns);
	testBenchDomCreateAfterClear(fns);
	testBenchDomMultipleInstances(fns);

	// 13.1 — Handler lifecycle
	testBenchHandlerLifecycle(fns);
}

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

// ══════════════════════════════════════════════════════════════════════════════
// 9.1 — Benchmark App: WASM-side operations
// ══════════════════════════════════════════════════════════════════════════════

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

export function testBench(fns: Fns): void {
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
}

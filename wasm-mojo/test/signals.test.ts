import type { WasmExports } from "../runtime/types.ts";
import { assert, suite } from "./harness.ts";

export function testSignals(fns: WasmExports): void {
	// ── Runtime lifecycle ─────────────────────────────────────────────
	suite("Signals — runtime lifecycle");
	{
		const rt = fns.runtime_create();
		assert(rt !== 0n, true, "runtime_create returns non-null pointer");
		assert(fns.signal_count(rt), 0, "new runtime has 0 signals");
		fns.runtime_destroy(rt);
	}

	// ── Signal create and read ───────────────────────────────────────
	suite("Signals — create and read");
	{
		const rt = fns.runtime_create();

		const key = fns.signal_create_i32(rt, 42);
		assert(key, 0, "first signal gets key 0");
		assert(fns.signal_count(rt), 1, "signal_count is 1");
		assert(fns.signal_contains(rt, key), 1, "signal exists");

		const val = fns.signal_read_i32(rt, key);
		assert(val, 42, "read returns initial value 42");

		fns.runtime_destroy(rt);
	}

	// ── Signal write and read back ───────────────────────────────────
	suite("Signals — write and read back");
	{
		const rt = fns.runtime_create();

		const key = fns.signal_create_i32(rt, 0);
		assert(fns.signal_read_i32(rt, key), 0, "initial value is 0");

		fns.signal_write_i32(rt, key, 99);
		assert(fns.signal_read_i32(rt, key), 99, "read after write returns 99");

		fns.signal_write_i32(rt, key, -42);
		assert(fns.signal_read_i32(rt, key), -42, "read after write returns -42");

		fns.runtime_destroy(rt);
	}

	// ── Signal peek (no subscription) ────────────────────────────────
	suite("Signals — peek");
	{
		const rt = fns.runtime_create();

		const key = fns.signal_create_i32(rt, 77);
		assert(fns.signal_peek_i32(rt, key), 77, "peek returns 77");

		fns.signal_write_i32(rt, key, 88);
		assert(fns.signal_peek_i32(rt, key), 88, "peek after write returns 88");

		fns.runtime_destroy(rt);
	}

	// ── Signal version tracking ──────────────────────────────────────
	suite("Signals — version tracking");
	{
		const rt = fns.runtime_create();

		const key = fns.signal_create_i32(rt, 0);
		assert(fns.signal_version(rt, key), 0, "initial version is 0");

		fns.signal_write_i32(rt, key, 1);
		assert(fns.signal_version(rt, key), 1, "version after 1 write is 1");

		fns.signal_write_i32(rt, key, 2);
		assert(fns.signal_version(rt, key), 2, "version after 2 writes is 2");

		// Peek and read don't change version
		fns.signal_peek_i32(rt, key);
		fns.signal_read_i32(rt, key);
		assert(fns.signal_version(rt, key), 2, "read/peek don't bump version");

		fns.runtime_destroy(rt);
	}

	// ── Multiple independent signals ─────────────────────────────────
	suite("Signals — multiple independent signals");
	{
		const rt = fns.runtime_create();

		const k1 = fns.signal_create_i32(rt, 10);
		const k2 = fns.signal_create_i32(rt, 20);
		const k3 = fns.signal_create_i32(rt, 30);

		assert(fns.signal_count(rt), 3, "3 signals created");
		assert(k1 !== k2 && k2 !== k3 && k1 !== k3, true, "all keys distinct");

		assert(fns.signal_read_i32(rt, k1), 10, "signal 1 reads 10");
		assert(fns.signal_read_i32(rt, k2), 20, "signal 2 reads 20");
		assert(fns.signal_read_i32(rt, k3), 30, "signal 3 reads 30");

		// Write to one doesn't affect others
		fns.signal_write_i32(rt, k2, 200);
		assert(fns.signal_read_i32(rt, k1), 10, "signal 1 unchanged");
		assert(fns.signal_read_i32(rt, k2), 200, "signal 2 updated to 200");
		assert(fns.signal_read_i32(rt, k3), 30, "signal 3 unchanged");

		fns.runtime_destroy(rt);
	}

	// ── Signal destroy ───────────────────────────────────────────────
	suite("Signals — destroy");
	{
		const rt = fns.runtime_create();

		const k1 = fns.signal_create_i32(rt, 10);
		const k2 = fns.signal_create_i32(rt, 20);
		assert(fns.signal_count(rt), 2, "2 signals before destroy");

		fns.signal_destroy(rt, k1);
		assert(fns.signal_count(rt), 1, "1 signal after destroy");
		assert(fns.signal_contains(rt, k1), 0, "destroyed signal not found");
		assert(fns.signal_contains(rt, k2), 1, "other signal still exists");

		fns.runtime_destroy(rt);
	}

	// ── Signal slot reuse after destroy ──────────────────────────────
	suite("Signals — slot reuse after destroy");
	{
		const rt = fns.runtime_create();

		const k1 = fns.signal_create_i32(rt, 10);
		fns.signal_destroy(rt, k1);

		const k2 = fns.signal_create_i32(rt, 99);
		assert(k2, k1, "new signal reuses destroyed slot");
		assert(fns.signal_read_i32(rt, k2), 99, "reused slot has new value");

		fns.runtime_destroy(rt);
	}

	// ── Signal += (iadd) ─────────────────────────────────────────────
	suite("Signals — iadd (+=)");
	{
		const rt = fns.runtime_create();

		const key = fns.signal_create_i32(rt, 10);
		fns.signal_iadd_i32(rt, key, 5);
		assert(fns.signal_read_i32(rt, key), 15, "10 += 5 => 15");

		fns.signal_iadd_i32(rt, key, -3);
		assert(fns.signal_read_i32(rt, key), 12, "15 += (-3) => 12");

		fns.signal_iadd_i32(rt, key, 0);
		assert(fns.signal_read_i32(rt, key), 12, "12 += 0 => 12");

		fns.runtime_destroy(rt);
	}

	// ── Signal -= (isub) ─────────────────────────────────────────────
	suite("Signals — isub (-=)");
	{
		const rt = fns.runtime_create();

		const key = fns.signal_create_i32(rt, 100);
		fns.signal_isub_i32(rt, key, 30);
		assert(fns.signal_read_i32(rt, key), 70, "100 -= 30 => 70");

		fns.signal_isub_i32(rt, key, -10);
		assert(fns.signal_read_i32(rt, key), 80, "70 -= (-10) => 80");

		fns.runtime_destroy(rt);
	}

	// ── Context: no context by default ───────────────────────────────
	suite("Signals — no context by default");
	{
		const rt = fns.runtime_create();

		assert(fns.runtime_has_context(rt), 0, "no context initially");

		fns.runtime_destroy(rt);
	}

	// ── Context: set and clear ───────────────────────────────────────
	suite("Signals — context set/clear");
	{
		const rt = fns.runtime_create();

		fns.runtime_set_context(rt, 42);
		assert(fns.runtime_has_context(rt), 1, "context active after set");

		fns.runtime_clear_context(rt);
		assert(fns.runtime_has_context(rt), 0, "no context after clear");

		fns.runtime_destroy(rt);
	}

	// ── Subscription: read with context subscribes ───────────────────
	suite("Signals — read with context subscribes");
	{
		const rt = fns.runtime_create();

		const key = fns.signal_create_i32(rt, 0);
		assert(fns.signal_subscriber_count(rt, key), 0, "0 subscribers initially");

		// Read without context — no subscription
		fns.signal_read_i32(rt, key);
		assert(
			fns.signal_subscriber_count(rt, key),
			0,
			"still 0 subscribers after read without context",
		);

		// Read with context — subscribes
		fns.runtime_set_context(rt, 100);
		fns.signal_read_i32(rt, key);
		assert(
			fns.signal_subscriber_count(rt, key),
			1,
			"1 subscriber after read with context",
		);

		// Reading again with same context is idempotent
		fns.signal_read_i32(rt, key);
		assert(
			fns.signal_subscriber_count(rt, key),
			1,
			"still 1 subscriber (idempotent)",
		);

		fns.runtime_clear_context(rt);
		fns.runtime_destroy(rt);
	}

	// ── Subscription: peek does NOT subscribe ────────────────────────
	suite("Signals — peek does not subscribe");
	{
		const rt = fns.runtime_create();

		const key = fns.signal_create_i32(rt, 0);
		fns.runtime_set_context(rt, 200);

		fns.signal_peek_i32(rt, key);
		assert(fns.signal_subscriber_count(rt, key), 0, "peek does not subscribe");

		fns.runtime_clear_context(rt);
		fns.runtime_destroy(rt);
	}

	// ── Subscription: multiple contexts subscribe ────────────────────
	suite("Signals — multiple contexts subscribe");
	{
		const rt = fns.runtime_create();

		const key = fns.signal_create_i32(rt, 0);

		fns.runtime_set_context(rt, 10);
		fns.signal_read_i32(rt, key);

		fns.runtime_set_context(rt, 20);
		fns.signal_read_i32(rt, key);

		fns.runtime_set_context(rt, 30);
		fns.signal_read_i32(rt, key);

		assert(
			fns.signal_subscriber_count(rt, key),
			3,
			"3 different contexts subscribed",
		);

		fns.runtime_clear_context(rt);
		fns.runtime_destroy(rt);
	}

	// ── Dirty scopes: write with subscribers produces dirty ──────────
	suite("Signals — write marks subscribers dirty");
	{
		const rt = fns.runtime_create();

		const key = fns.signal_create_i32(rt, 0);
		assert(fns.runtime_has_dirty(rt), 0, "no dirty scopes initially");

		// Subscribe context 1
		fns.runtime_set_context(rt, 1);
		fns.signal_read_i32(rt, key);

		// Subscribe context 2
		fns.runtime_set_context(rt, 2);
		fns.signal_read_i32(rt, key);

		fns.runtime_clear_context(rt);

		// Write — should mark both contexts dirty
		fns.signal_write_i32(rt, key, 42);
		assert(fns.runtime_has_dirty(rt), 1, "has dirty after write");
		assert(fns.runtime_dirty_count(rt), 2, "2 dirty scopes");

		fns.runtime_destroy(rt);
	}

	// ── Dirty scopes: write without subscribers ──────────────────────
	suite("Signals — write without subscribers is clean");
	{
		const rt = fns.runtime_create();

		const key = fns.signal_create_i32(rt, 0);
		fns.signal_write_i32(rt, key, 99);

		assert(fns.runtime_has_dirty(rt), 0, "no dirty without subscribers");
		assert(fns.runtime_dirty_count(rt), 0, "dirty count is 0");

		fns.runtime_destroy(rt);
	}

	// ── Dirty scopes: iadd marks dirty ───────────────────────────────
	suite("Signals — iadd marks subscribers dirty");
	{
		const rt = fns.runtime_create();

		const key = fns.signal_create_i32(rt, 0);

		fns.runtime_set_context(rt, 50);
		fns.signal_read_i32(rt, key);
		fns.runtime_clear_context(rt);

		fns.signal_iadd_i32(rt, key, 1);
		assert(fns.runtime_has_dirty(rt), 1, "iadd marks subscriber dirty");
		assert(fns.runtime_dirty_count(rt), 1, "1 dirty scope from iadd");

		fns.runtime_destroy(rt);
	}

	// ── Multiple writes accumulate dirty (no dedup between writes) ───
	suite("Signals — multiple writes deduplicate dirty scopes");
	{
		const rt = fns.runtime_create();

		const key = fns.signal_create_i32(rt, 0);

		fns.runtime_set_context(rt, 1);
		fns.signal_read_i32(rt, key);
		fns.runtime_clear_context(rt);

		// Write twice — same subscriber should not be double-queued
		fns.signal_write_i32(rt, key, 10);
		fns.signal_write_i32(rt, key, 20);

		// The dirty count should still be 1 since it's the same context
		assert(fns.runtime_dirty_count(rt), 1, "same subscriber not double-queued");

		fns.runtime_destroy(rt);
	}

	// ── Read after write in same turn returns new value ──────────────
	suite("Signals — read after write returns new value");
	{
		const rt = fns.runtime_create();

		const key = fns.signal_create_i32(rt, 0);
		fns.signal_write_i32(rt, key, 123);
		assert(fns.signal_read_i32(rt, key), 123, "immediate read gets 123");

		fns.signal_iadd_i32(rt, key, 1);
		assert(fns.signal_read_i32(rt, key), 124, "iadd then read gets 124");

		fns.runtime_destroy(rt);
	}

	// ── Stress: create 100 signals, verify independence ──────────────
	suite("Signals — stress: 100 independent signals");
	{
		const rt = fns.runtime_create();

		const keys: number[] = [];
		for (let i = 0; i < 100; i++) {
			keys.push(fns.signal_create_i32(rt, i * 10));
		}
		assert(fns.signal_count(rt), 100, "100 signals created");

		// Verify each holds its own value
		let allCorrect = true;
		for (let i = 0; i < 100; i++) {
			if (fns.signal_read_i32(rt, keys[i]) !== i * 10) {
				allCorrect = false;
				break;
			}
		}
		assert(allCorrect, true, "all 100 signals hold correct initial values");

		// Write to every other one
		for (let i = 0; i < 100; i += 2) {
			fns.signal_write_i32(rt, keys[i], 999);
		}

		// Verify written ones changed and others didn't
		let writeCorrect = true;
		for (let i = 0; i < 100; i++) {
			const expected = i % 2 === 0 ? 999 : i * 10;
			if (fns.signal_read_i32(rt, keys[i]) !== expected) {
				writeCorrect = false;
				break;
			}
		}
		assert(writeCorrect, true, "even-indexed signals updated, odd unchanged");

		fns.runtime_destroy(rt);
	}

	// ── Stress: create/destroy cycle reuses slots ────────────────────
	suite("Signals — stress: create/destroy/reuse cycle");
	{
		const rt = fns.runtime_create();

		// Create 50 signals
		const keys: number[] = [];
		for (let i = 0; i < 50; i++) {
			keys.push(fns.signal_create_i32(rt, i));
		}

		// Destroy all even-indexed
		for (let i = 0; i < 50; i += 2) {
			fns.signal_destroy(rt, keys[i]);
		}
		assert(fns.signal_count(rt), 25, "25 signals after destroying 25");

		// Create 25 more — should reuse freed slots
		const newKeys: number[] = [];
		for (let i = 0; i < 25; i++) {
			newKeys.push(fns.signal_create_i32(rt, 1000 + i));
		}
		assert(fns.signal_count(rt), 50, "back to 50 signals");

		// Verify the new signals have correct values
		let reusedCorrect = true;
		for (let i = 0; i < 25; i++) {
			if (fns.signal_read_i32(rt, newKeys[i]) !== 1000 + i) {
				reusedCorrect = false;
				break;
			}
		}
		assert(reusedCorrect, true, "reused slots have correct new values");

		// Verify odd-indexed originals still intact
		let origCorrect = true;
		for (let i = 1; i < 50; i += 2) {
			if (fns.signal_read_i32(rt, keys[i]) !== i) {
				origCorrect = false;
				break;
			}
		}
		assert(origCorrect, true, "original odd-indexed signals still intact");

		fns.runtime_destroy(rt);
	}

	// ── Edge case: negative values ───────────────────────────────────
	suite("Signals — negative values");
	{
		const rt = fns.runtime_create();

		const key = fns.signal_create_i32(rt, -2147483648); // INT32_MIN
		assert(fns.signal_read_i32(rt, key), -2147483648, "can store INT32_MIN");

		fns.signal_write_i32(rt, key, 2147483647); // INT32_MAX
		assert(fns.signal_read_i32(rt, key), 2147483647, "can store INT32_MAX");

		fns.runtime_destroy(rt);
	}

	// ── Edge case: zero initial value ────────────────────────────────
	suite("Signals — zero initial value");
	{
		const rt = fns.runtime_create();

		const key = fns.signal_create_i32(rt, 0);
		assert(fns.signal_read_i32(rt, key), 0, "zero initial value");
		assert(fns.signal_peek_i32(rt, key), 0, "zero peek value");

		fns.runtime_destroy(rt);
	}
}

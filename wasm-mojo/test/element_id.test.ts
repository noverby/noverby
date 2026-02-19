import type { WasmExports } from "../runtime/types.ts";
import { assert, suite } from "./harness.ts";

export function testElementId(fns: WasmExports): void {
	// ── Basic allocation ─────────────────────────────────────────────
	suite("ElementId — basic allocation");
	{
		const a = fns.eid_alloc_create();

		// First user ID should be 1 (slot 0 is reserved for root)
		const id1 = fns.eid_alloc(a);
		assert(id1, 1, "first alloc returns 1");

		const id2 = fns.eid_alloc(a);
		assert(id2, 2, "second alloc returns 2");

		const id3 = fns.eid_alloc(a);
		assert(id3, 3, "third alloc returns 3");

		fns.eid_alloc_destroy(a);
	}

	// ── Count tracking ───────────────────────────────────────────────
	suite("ElementId — count tracking");
	{
		const a = fns.eid_alloc_create();

		// Root is always counted
		assert(fns.eid_count(a), 1, "initial count is 1 (root)");
		assert(fns.eid_user_count(a), 0, "initial user_count is 0");

		fns.eid_alloc(a); // id=1
		assert(fns.eid_count(a), 2, "count after 1 alloc is 2");
		assert(fns.eid_user_count(a), 1, "user_count after 1 alloc is 1");

		fns.eid_alloc(a); // id=2
		fns.eid_alloc(a); // id=3
		assert(fns.eid_count(a), 4, "count after 3 allocs is 4");
		assert(fns.eid_user_count(a), 3, "user_count after 3 allocs is 3");

		fns.eid_alloc_destroy(a);
	}

	// ── is_alive checks ──────────────────────────────────────────────
	suite("ElementId — is_alive");
	{
		const a = fns.eid_alloc_create();

		// Root (0) is alive
		assert(fns.eid_is_alive(a, 0), 1, "root (0) is alive");

		const id1 = fns.eid_alloc(a);
		assert(fns.eid_is_alive(a, id1), 1, "allocated id is alive");

		// Unallocated ID should not be alive
		assert(fns.eid_is_alive(a, 99), 0, "unallocated id 99 is not alive");

		fns.eid_alloc_destroy(a);
	}

	// ── Free and reuse ───────────────────────────────────────────────
	suite("ElementId — free and reuse");
	{
		const a = fns.eid_alloc_create();

		const id1 = fns.eid_alloc(a); // 1
		const id2 = fns.eid_alloc(a); // 2
		const id3 = fns.eid_alloc(a); // 3
		assert(fns.eid_user_count(a), 3, "3 IDs allocated");

		// Free id2
		fns.eid_free(a, id2);
		assert(fns.eid_is_alive(a, id2), 0, "freed id2 is no longer alive");
		assert(fns.eid_user_count(a), 2, "user_count after free is 2");

		// Next alloc should reuse id2's slot
		const id4 = fns.eid_alloc(a);
		assert(id4, id2, "reused slot has same ID as freed id2");
		assert(fns.eid_is_alive(a, id4), 1, "reused id is alive");
		assert(fns.eid_user_count(a), 3, "user_count back to 3");

		fns.eid_alloc_destroy(a);
	}

	// ── Free root is a no-op ─────────────────────────────────────────
	suite("ElementId — free root is no-op");
	{
		const a = fns.eid_alloc_create();

		fns.eid_free(a, 0); // Should be a no-op
		assert(fns.eid_is_alive(a, 0), 1, "root still alive after free(0)");
		assert(fns.eid_count(a), 1, "count unchanged after free(0)");

		fns.eid_alloc_destroy(a);
	}

	// ── Double free is safe ──────────────────────────────────────────
	suite("ElementId — double free is safe");
	{
		const a = fns.eid_alloc_create();

		const id1 = fns.eid_alloc(a);
		fns.eid_free(a, id1);
		fns.eid_free(a, id1); // Should not crash
		assert(fns.eid_user_count(a), 0, "user_count still 0 after double free");

		fns.eid_alloc_destroy(a);
	}

	// ── Sequential IDs are unique ────────────────────────────────────
	suite("ElementId — sequential IDs are unique");
	{
		const a = fns.eid_alloc_create();

		const ids = new Set<number>();
		for (let i = 0; i < 100; i++) {
			const id = fns.eid_alloc(a);
			ids.add(id);
		}
		assert(ids.size, 100, "100 sequential allocs produce 100 unique IDs");
		assert(fns.eid_user_count(a), 100, "user_count is 100");

		// None should be 0 (root)
		assert(ids.has(0), false, "no allocated ID is 0 (root)");

		fns.eid_alloc_destroy(a);
	}

	// ── Alloc/free cycle reuses slots ────────────────────────────────
	suite("ElementId — alloc/free cycle reuses slots");
	{
		const a = fns.eid_alloc_create();

		// Allocate 10 IDs
		const ids: number[] = [];
		for (let i = 0; i < 10; i++) {
			ids.push(fns.eid_alloc(a));
		}
		assert(fns.eid_user_count(a), 10, "10 IDs allocated");

		// Free the even-indexed ones (ids[0], ids[2], ids[4], ids[6], ids[8])
		for (let i = 0; i < 10; i += 2) {
			fns.eid_free(a, ids[i]);
		}
		assert(fns.eid_user_count(a), 5, "5 IDs remain after freeing 5");

		// Allocate 5 more — they should reuse the freed slots
		const newIds: number[] = [];
		for (let i = 0; i < 5; i++) {
			newIds.push(fns.eid_alloc(a));
		}
		assert(fns.eid_user_count(a), 10, "back to 10 after realloc");

		// All new IDs should be from the freed set
		const freedIds = new Set(ids.filter((_, i) => i % 2 === 0));
		for (const nid of newIds) {
			assert(freedIds.has(nid), true, `reused ID ${nid} was in freed set`);
		}

		fns.eid_alloc_destroy(a);
	}

	// ── Stress: alloc 1000, free half, alloc 500 ─────────────────────
	suite("ElementId — stress: 1000 alloc, free half, alloc 500");
	{
		const a = fns.eid_alloc_create();

		const ids: number[] = [];
		for (let i = 0; i < 1000; i++) {
			ids.push(fns.eid_alloc(a));
		}
		assert(fns.eid_user_count(a), 1000, "1000 IDs allocated");

		// Free odd-indexed (500 IDs)
		for (let i = 1; i < 1000; i += 2) {
			fns.eid_free(a, ids[i]);
		}
		assert(fns.eid_user_count(a), 500, "500 remain after freeing 500");

		// Allocate 500 more
		for (let i = 0; i < 500; i++) {
			fns.eid_alloc(a);
		}
		assert(fns.eid_user_count(a), 1000, "back to 1000 after realloc");

		// All even-indexed original IDs should still be alive
		for (let i = 0; i < 1000; i += 2) {
			assert(
				fns.eid_is_alive(a, ids[i]),
				1,
				`original id ${ids[i]} (even index) still alive`,
			);
		}

		fns.eid_alloc_destroy(a);
	}
}

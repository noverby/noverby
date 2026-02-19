import type { WasmExports } from "../runtime/types.ts";
import { assert, suite } from "./harness.ts";

export function testScopes(fns: WasmExports): void {
	// ── Scope lifecycle ──────────────────────────────────────────────
	suite("Scopes — create and destroy");
	{
		const rt = fns.runtime_create();

		assert(fns.scope_count(rt), 0, "new runtime has 0 scopes");

		const s0 = fns.scope_create(rt, 0, -1);
		assert(fns.scope_count(rt), 1, "1 scope after create");
		assert(fns.scope_contains(rt, s0), 1, "scope exists");

		fns.scope_destroy(rt, s0);
		assert(fns.scope_count(rt), 0, "0 scopes after destroy");
		assert(fns.scope_contains(rt, s0), 0, "scope no longer exists");

		fns.runtime_destroy(rt);
	}

	// ── Scope IDs are sequential ─────────────────────────────────────
	suite("Scopes — sequential IDs");
	{
		const rt = fns.runtime_create();

		const s0 = fns.scope_create(rt, 0, -1);
		const s1 = fns.scope_create(rt, 0, -1);
		const s2 = fns.scope_create(rt, 0, -1);

		assert(s0, 0, "first scope gets ID 0");
		assert(s1, 1, "second scope gets ID 1");
		assert(s2, 2, "third scope gets ID 2");
		assert(fns.scope_count(rt), 3, "3 scopes created");

		fns.runtime_destroy(rt);
	}

	// ── Scope slot reuse after destroy ───────────────────────────────
	suite("Scopes — slot reuse after destroy");
	{
		const rt = fns.runtime_create();

		const s0 = fns.scope_create(rt, 0, -1);
		const _s1 = fns.scope_create(rt, 0, -1);
		fns.scope_destroy(rt, s0);

		const s2 = fns.scope_create(rt, 0, -1);
		assert(s2, s0, "new scope reuses destroyed slot");
		assert(fns.scope_count(rt), 2, "2 scopes after reuse");

		fns.runtime_destroy(rt);
	}

	// ── Double destroy is safe ───────────────────────────────────────
	suite("Scopes — double destroy is no-op");
	{
		const rt = fns.runtime_create();

		const s0 = fns.scope_create(rt, 0, -1);
		fns.scope_destroy(rt, s0);
		fns.scope_destroy(rt, s0); // should not crash
		assert(fns.scope_count(rt), 0, "still 0 scopes after double destroy");

		fns.runtime_destroy(rt);
	}

	// ── Scope height and parent ──────────────────────────────────────
	suite("Scopes — height and parent tracking");
	{
		const rt = fns.runtime_create();

		const root = fns.scope_create(rt, 0, -1);
		assert(fns.scope_height(rt, root), 0, "root height is 0");
		assert(fns.scope_parent(rt, root), -1, "root has no parent (-1)");

		const child = fns.scope_create(rt, 1, root);
		assert(fns.scope_height(rt, child), 1, "child height is 1");
		assert(fns.scope_parent(rt, child), root, "child parent is root");

		const grandchild = fns.scope_create(rt, 2, child);
		assert(fns.scope_height(rt, grandchild), 2, "grandchild height is 2");
		assert(
			fns.scope_parent(rt, grandchild),
			child,
			"grandchild parent is child",
		);

		fns.runtime_destroy(rt);
	}

	// ── Scope create_child auto-computes height ──────────────────────
	suite("Scopes — create_child auto-computes height");
	{
		const rt = fns.runtime_create();

		const root = fns.scope_create(rt, 0, -1);
		const child = fns.scope_create_child(rt, root);
		const grandchild = fns.scope_create_child(rt, child);

		assert(fns.scope_height(rt, child), 1, "child height auto-computed to 1");
		assert(fns.scope_parent(rt, child), root, "child parent is root");
		assert(
			fns.scope_height(rt, grandchild),
			2,
			"grandchild height auto-computed to 2",
		);
		assert(
			fns.scope_parent(rt, grandchild),
			child,
			"grandchild parent is child",
		);

		fns.runtime_destroy(rt);
	}

	// ── Scope dirty flag ─────────────────────────────────────────────
	suite("Scopes — dirty flag");
	{
		const rt = fns.runtime_create();

		const s = fns.scope_create(rt, 0, -1);
		assert(fns.scope_is_dirty(rt, s), 0, "not dirty initially");

		fns.scope_set_dirty(rt, s, 1);
		assert(fns.scope_is_dirty(rt, s), 1, "dirty after set_dirty(1)");

		fns.scope_set_dirty(rt, s, 0);
		assert(fns.scope_is_dirty(rt, s), 0, "clean after set_dirty(0)");

		fns.runtime_destroy(rt);
	}

	// ── Scope render count ───────────────────────────────────────────
	suite("Scopes — render count");
	{
		const rt = fns.runtime_create();

		const s = fns.scope_create(rt, 0, -1);
		assert(fns.scope_render_count(rt, s), 0, "render_count starts at 0");

		const prev = fns.scope_begin_render(rt, s);
		assert(
			fns.scope_render_count(rt, s),
			1,
			"render_count is 1 after first begin_render",
		);
		fns.scope_end_render(rt, prev);

		const prev2 = fns.scope_begin_render(rt, s);
		assert(
			fns.scope_render_count(rt, s),
			2,
			"render_count is 2 after second begin_render",
		);
		fns.scope_end_render(rt, prev2);

		fns.runtime_destroy(rt);
	}

	// ── Begin render clears dirty flag ───────────────────────────────
	suite("Scopes — begin_render clears dirty");
	{
		const rt = fns.runtime_create();

		const s = fns.scope_create(rt, 0, -1);
		fns.scope_set_dirty(rt, s, 1);
		assert(fns.scope_is_dirty(rt, s), 1, "dirty before render");

		const prev = fns.scope_begin_render(rt, s);
		assert(fns.scope_is_dirty(rt, s), 0, "clean after begin_render");
		fns.scope_end_render(rt, prev);

		fns.runtime_destroy(rt);
	}

	// ── Scope rendering sets current scope ───────────────────────────
	suite("Scopes — begin/end_render manages current scope");
	{
		const rt = fns.runtime_create();

		assert(fns.scope_has_scope(rt), 0, "no scope initially");
		assert(fns.scope_get_current(rt), -1, "current scope is -1 initially");

		const s = fns.scope_create(rt, 0, -1);
		const prev = fns.scope_begin_render(rt, s);
		assert(prev, -1, "previous scope is -1 (was no scope)");
		assert(fns.scope_has_scope(rt), 1, "scope active during render");
		assert(
			fns.scope_get_current(rt),
			s,
			"current scope is the rendering scope",
		);

		fns.scope_end_render(rt, prev);
		assert(fns.scope_has_scope(rt), 0, "no scope after end_render");
		assert(
			fns.scope_get_current(rt),
			-1,
			"current scope is -1 after end_render",
		);

		fns.runtime_destroy(rt);
	}

	// ── Scope rendering sets reactive context ────────────────────────
	suite("Scopes — begin_render sets reactive context");
	{
		const rt = fns.runtime_create();

		const s = fns.scope_create(rt, 0, -1);
		assert(fns.runtime_has_context(rt), 0, "no context initially");

		const prev = fns.scope_begin_render(rt, s);
		assert(fns.runtime_has_context(rt), 1, "context active during render");

		fns.scope_end_render(rt, prev);
		assert(fns.runtime_has_context(rt), 0, "context cleared after end_render");

		fns.runtime_destroy(rt);
	}

	// ── Nested scope rendering ───────────────────────────────────────
	suite("Scopes — nested scope rendering");
	{
		const rt = fns.runtime_create();

		const root = fns.scope_create(rt, 0, -1);
		const child = fns.scope_create_child(rt, root);

		// Begin rendering root
		const prev1 = fns.scope_begin_render(rt, root);
		assert(fns.scope_get_current(rt), root, "current scope is root");

		// Nest: begin rendering child
		const prev2 = fns.scope_begin_render(rt, child);
		assert(prev2, root, "previous scope was root");
		assert(fns.scope_get_current(rt), child, "current scope is child");

		// End child rendering
		fns.scope_end_render(rt, prev2);
		assert(fns.scope_get_current(rt), root, "current scope restored to root");

		// End root rendering
		fns.scope_end_render(rt, prev1);
		assert(fns.scope_get_current(rt), -1, "current scope cleared");

		fns.runtime_destroy(rt);
	}

	// ── is_first_render ──────────────────────────────────────────────
	suite("Scopes — is_first_render");
	{
		const rt = fns.runtime_create();

		const s = fns.scope_create(rt, 0, -1);
		assert(
			fns.scope_is_first_render(rt, s),
			1,
			"first render before any rendering",
		);

		const prev = fns.scope_begin_render(rt, s);
		assert(
			fns.scope_is_first_render(rt, s),
			1,
			"first render during first render pass",
		);
		fns.scope_end_render(rt, prev);

		const prev2 = fns.scope_begin_render(rt, s);
		assert(
			fns.scope_is_first_render(rt, s),
			0,
			"not first render on second pass",
		);
		fns.scope_end_render(rt, prev2);

		fns.runtime_destroy(rt);
	}

	// ── Hook count starts at 0 ──────────────────────────────────────
	suite("Scopes — hooks start empty");
	{
		const rt = fns.runtime_create();

		const s = fns.scope_create(rt, 0, -1);
		assert(fns.scope_hook_count(rt, s), 0, "no hooks initially");

		fns.runtime_destroy(rt);
	}

	// ── Hook: use_signal on first render creates signal ──────────────
	suite("Hooks — use_signal creates signal on first render");
	{
		const rt = fns.runtime_create();

		const s = fns.scope_create(rt, 0, -1);
		const prev = fns.scope_begin_render(rt, s);

		const key = fns.hook_use_signal_i32(rt, 42);
		assert(
			fns.signal_read_i32(rt, key),
			42,
			"signal created with initial value 42",
		);
		assert(fns.scope_hook_count(rt, s), 1, "1 hook after use_signal");
		assert(
			fns.scope_hook_value_at(rt, s, 0),
			key,
			"hook[0] stores the signal key",
		);
		assert(
			fns.scope_hook_tag_at(rt, s, 0),
			0,
			"hook[0] tag is HOOK_SIGNAL (0)",
		);

		fns.scope_end_render(rt, prev);
		fns.runtime_destroy(rt);
	}

	// ── Hook: use_signal on re-render returns same signal ────────────
	suite("Hooks — use_signal returns same signal on re-render");
	{
		const rt = fns.runtime_create();

		const s = fns.scope_create(rt, 0, -1);

		// First render: create signal
		const prev1 = fns.scope_begin_render(rt, s);
		const key1 = fns.hook_use_signal_i32(rt, 100);
		assert(
			fns.signal_read_i32(rt, key1),
			100,
			"first render: signal value is 100",
		);
		fns.scope_end_render(rt, prev1);

		// Modify signal between renders
		fns.signal_write_i32(rt, key1, 200);

		// Second render: retrieve same signal (initial value ignored)
		const prev2 = fns.scope_begin_render(rt, s);
		const key2 = fns.hook_use_signal_i32(rt, 999);
		assert(key2, key1, "re-render returns same signal key");
		assert(
			fns.signal_read_i32(rt, key2),
			200,
			"signal retains modified value, not initial",
		);
		assert(
			fns.scope_hook_count(rt, s),
			1,
			"still 1 hook (no new hook created)",
		);
		fns.scope_end_render(rt, prev2);

		fns.runtime_destroy(rt);
	}

	// ── Hook: multiple signals in same scope ─────────────────────────
	suite("Hooks — multiple signals in same scope");
	{
		const rt = fns.runtime_create();

		const s = fns.scope_create(rt, 0, -1);

		// First render: create 3 signals
		const prev1 = fns.scope_begin_render(rt, s);
		const k1 = fns.hook_use_signal_i32(rt, 10);
		const k2 = fns.hook_use_signal_i32(rt, 20);
		const k3 = fns.hook_use_signal_i32(rt, 30);
		assert(fns.scope_hook_count(rt, s), 3, "3 hooks after first render");
		assert(k1 !== k2 && k2 !== k3, true, "all signal keys distinct");
		fns.scope_end_render(rt, prev1);

		// Second render: same order returns same keys
		const prev2 = fns.scope_begin_render(rt, s);
		const k1b = fns.hook_use_signal_i32(rt, 0);
		const k2b = fns.hook_use_signal_i32(rt, 0);
		const k3b = fns.hook_use_signal_i32(rt, 0);
		assert(k1b, k1, "re-render hook 0 returns same key");
		assert(k2b, k2, "re-render hook 1 returns same key");
		assert(k3b, k3, "re-render hook 2 returns same key");
		assert(fns.scope_hook_count(rt, s), 3, "still 3 hooks");
		fns.scope_end_render(rt, prev2);

		// Values are independent
		assert(fns.signal_read_i32(rt, k1), 10, "signal 1 has value 10");
		assert(fns.signal_read_i32(rt, k2), 20, "signal 2 has value 20");
		assert(fns.signal_read_i32(rt, k3), 30, "signal 3 has value 30");

		fns.runtime_destroy(rt);
	}

	// ── Hook: signals in different scopes are independent ────────────
	suite("Hooks — signals in different scopes are independent");
	{
		const rt = fns.runtime_create();

		const s1 = fns.scope_create(rt, 0, -1);
		const s2 = fns.scope_create(rt, 0, -1);

		// Render scope 1
		const prev1 = fns.scope_begin_render(rt, s1);
		const k1 = fns.hook_use_signal_i32(rt, 100);
		fns.scope_end_render(rt, prev1);

		// Render scope 2
		const prev2 = fns.scope_begin_render(rt, s2);
		const k2 = fns.hook_use_signal_i32(rt, 200);
		fns.scope_end_render(rt, prev2);

		assert(k1 !== k2, true, "different scopes get different signal keys");
		assert(fns.signal_read_i32(rt, k1), 100, "scope 1 signal is 100");
		assert(fns.signal_read_i32(rt, k2), 200, "scope 2 signal is 200");

		// Modify one, other unchanged
		fns.signal_write_i32(rt, k1, 999);
		assert(fns.signal_read_i32(rt, k1), 999, "scope 1 signal updated");
		assert(fns.signal_read_i32(rt, k2), 200, "scope 2 signal unchanged");

		fns.runtime_destroy(rt);
	}

	// ── Hook: signal read during render subscribes scope ─────────────
	suite("Hooks — signal read during render subscribes scope");
	{
		const rt = fns.runtime_create();

		const s = fns.scope_create(rt, 0, -1);

		// First render
		const prev = fns.scope_begin_render(rt, s);
		const key = fns.hook_use_signal_i32(rt, 0);

		// Read the signal during render — should subscribe this scope
		fns.signal_read_i32(rt, key);
		assert(
			fns.signal_subscriber_count(rt, key),
			1,
			"scope subscribed after read during render",
		);

		fns.scope_end_render(rt, prev);

		// Write should mark scope dirty
		fns.signal_write_i32(rt, key, 42);
		assert(fns.runtime_has_dirty(rt), 1, "dirty after signal write");
		assert(fns.runtime_dirty_count(rt), 1, "1 dirty scope");

		fns.runtime_destroy(rt);
	}

	// ── Hook: peek during render does NOT subscribe ──────────────────
	suite("Hooks — peek during render does not subscribe");
	{
		const rt = fns.runtime_create();

		const s = fns.scope_create(rt, 0, -1);

		const prev = fns.scope_begin_render(rt, s);
		const key = fns.hook_use_signal_i32(rt, 0);

		// Peek should NOT subscribe
		fns.signal_peek_i32(rt, key);
		assert(fns.signal_subscriber_count(rt, key), 0, "peek does not subscribe");

		fns.scope_end_render(rt, prev);

		fns.runtime_destroy(rt);
	}

	// ── Nested rendering: child signals subscribe child scope ────────
	suite("Hooks — nested rendering subscribes correct scope");
	{
		const rt = fns.runtime_create();

		const root = fns.scope_create(rt, 0, -1);
		const child = fns.scope_create_child(rt, root);

		// Begin root render
		const prevRoot = fns.scope_begin_render(rt, root);
		const rootSignal = fns.hook_use_signal_i32(rt, 10);
		fns.signal_read_i32(rt, rootSignal);

		// Begin child render (nested)
		const prevChild = fns.scope_begin_render(rt, child);
		const childSignal = fns.hook_use_signal_i32(rt, 20);
		fns.signal_read_i32(rt, childSignal);

		// Child signal should have child as subscriber, not root
		assert(
			fns.signal_subscriber_count(rt, childSignal),
			1,
			"child signal has 1 subscriber",
		);

		// End child render
		fns.scope_end_render(rt, prevChild);

		// Now read root's signal again — should still subscribe root
		assert(
			fns.signal_subscriber_count(rt, rootSignal),
			1,
			"root signal has 1 subscriber",
		);

		// End root render
		fns.scope_end_render(rt, prevRoot);

		// Write to child signal should only mark child dirty
		fns.signal_write_i32(rt, childSignal, 99);
		assert(
			fns.runtime_dirty_count(rt),
			1,
			"only 1 dirty scope from child signal write",
		);

		fns.runtime_destroy(rt);
	}

	// ── Scope create/destroy stress test ─────────────────────────────
	suite("Scopes — stress: 100 scopes");
	{
		const rt = fns.runtime_create();

		const ids: number[] = [];
		for (let i = 0; i < 100; i++) {
			ids.push(fns.scope_create(rt, 0, -1));
		}
		assert(fns.scope_count(rt), 100, "100 scopes created");

		// Destroy half
		for (let i = 0; i < 100; i += 2) {
			fns.scope_destroy(rt, ids[i]);
		}
		assert(fns.scope_count(rt), 50, "50 scopes after destroying half");

		// Create 50 more (reuse freed slots)
		const newIds: number[] = [];
		for (let i = 0; i < 50; i++) {
			newIds.push(fns.scope_create(rt, 0, -1));
		}
		assert(fns.scope_count(rt), 100, "100 scopes after refill");

		// Verify all live scopes exist
		let allExist = true;
		for (let i = 1; i < 100; i += 2) {
			if (fns.scope_contains(rt, ids[i]) !== 1) {
				allExist = false;
				break;
			}
		}
		assert(allExist, true, "all odd-indexed original scopes still exist");

		fns.runtime_destroy(rt);
	}

	// ── Hook: use_signal across many re-renders ──────────────────────
	suite("Hooks — signal stable across many re-renders");
	{
		const rt = fns.runtime_create();

		const s = fns.scope_create(rt, 0, -1);

		// First render
		let prev = fns.scope_begin_render(rt, s);
		const key = fns.hook_use_signal_i32(rt, 0);
		fns.scope_end_render(rt, prev);

		// Increment signal and re-render 50 times
		for (let i = 1; i <= 50; i++) {
			fns.signal_write_i32(rt, key, i);

			prev = fns.scope_begin_render(rt, s);
			const k = fns.hook_use_signal_i32(rt, 999);
			assert(k, key, `re-render ${i}: same key`);
			fns.scope_end_render(rt, prev);
		}

		assert(
			fns.signal_read_i32(rt, key),
			50,
			"signal holds value 50 after 50 writes",
		);
		assert(
			fns.scope_render_count(rt, s),
			51,
			"render_count is 51 after 1 + 50 re-renders",
		);
		assert(fns.scope_hook_count(rt, s), 1, "still just 1 hook");

		fns.runtime_destroy(rt);
	}

	// ── Simulated counter component ──────────────────────────────────
	suite("Hooks — simulated counter component");
	{
		const rt = fns.runtime_create();
		const s = fns.scope_create(rt, 0, -1);

		// Simulate: def counter() -> Element:
		//   var count = signal(0)
		//   div(h1("Count: ", count), button(onclick=fn(): count += 1, "+"))

		// First render
		let prev = fns.scope_begin_render(rt, s);
		const countKey = fns.hook_use_signal_i32(rt, 0);
		const countVal = fns.signal_read_i32(rt, countKey);
		assert(countVal, 0, "initial count is 0");
		fns.scope_end_render(rt, prev);

		// Simulate click: count += 1
		fns.signal_iadd_i32(rt, countKey, 1);
		assert(fns.signal_read_i32(rt, countKey), 1, "count is 1 after increment");
		assert(
			fns.runtime_has_dirty(rt),
			1,
			"scope marked dirty after signal write",
		);

		// Re-render (triggered by dirty)
		prev = fns.scope_begin_render(rt, s);
		const countKey2 = fns.hook_use_signal_i32(rt, 0);
		assert(countKey2, countKey, "same signal key on re-render");
		const countVal2 = fns.signal_read_i32(rt, countKey2);
		assert(countVal2, 1, "count reads 1 on re-render");
		fns.scope_end_render(rt, prev);

		// Another click
		fns.signal_iadd_i32(rt, countKey, 1);
		assert(
			fns.signal_read_i32(rt, countKey),
			2,
			"count is 2 after second increment",
		);

		fns.runtime_destroy(rt);
	}

	// ── Simulated component with multiple state signals ──────────────
	suite("Hooks — simulated multi-state component");
	{
		const rt = fns.runtime_create();
		const s = fns.scope_create(rt, 0, -1);

		// Simulate: def form_component():
		//   var name = signal("")  (test as i32 = 0)
		//   var age = signal(0)
		//   var submitted = signal(false)  (test as i32 = 0)

		// First render
		let prev = fns.scope_begin_render(rt, s);
		const nameKey = fns.hook_use_signal_i32(rt, 0);
		const ageKey = fns.hook_use_signal_i32(rt, 0);
		const submittedKey = fns.hook_use_signal_i32(rt, 0);
		assert(fns.scope_hook_count(rt, s), 3, "3 hooks for 3 signals");
		fns.scope_end_render(rt, prev);

		// Simulate user interaction
		fns.signal_write_i32(rt, nameKey, 42);
		fns.signal_write_i32(rt, ageKey, 25);

		// Re-render
		prev = fns.scope_begin_render(rt, s);
		const nameKey2 = fns.hook_use_signal_i32(rt, 0);
		const ageKey2 = fns.hook_use_signal_i32(rt, 0);
		const submittedKey2 = fns.hook_use_signal_i32(rt, 0);
		assert(nameKey2, nameKey, "name signal stable");
		assert(ageKey2, ageKey, "age signal stable");
		assert(submittedKey2, submittedKey, "submitted signal stable");
		assert(fns.signal_read_i32(rt, nameKey2), 42, "name retains value");
		assert(fns.signal_read_i32(rt, ageKey2), 25, "age retains value");
		assert(fns.signal_read_i32(rt, submittedKey2), 0, "submitted still false");
		fns.scope_end_render(rt, prev);

		fns.runtime_destroy(rt);
	}

	// ── Simulated parent-child component tree ────────────────────────
	suite("Hooks — simulated parent-child component tree");
	{
		const rt = fns.runtime_create();

		const parent = fns.scope_create(rt, 0, -1);
		const child1 = fns.scope_create_child(rt, parent);
		const child2 = fns.scope_create_child(rt, parent);

		// Render parent
		const prevP = fns.scope_begin_render(rt, parent);
		const parentCount = fns.hook_use_signal_i32(rt, 0);
		fns.signal_read_i32(rt, parentCount); // subscribe parent

		// Render child1 (nested)
		const prevC1 = fns.scope_begin_render(rt, child1);
		const child1Local = fns.hook_use_signal_i32(rt, 10);
		fns.signal_read_i32(rt, child1Local); // subscribe child1
		// Also read parent's signal from child1
		fns.signal_read_i32(rt, parentCount); // child1 subscribes to parent signal
		fns.scope_end_render(rt, prevC1);

		// Render child2 (nested)
		const prevC2 = fns.scope_begin_render(rt, child2);
		const child2Local = fns.hook_use_signal_i32(rt, 20);
		fns.signal_read_i32(rt, child2Local); // subscribe child2
		fns.scope_end_render(rt, prevC2);

		fns.scope_end_render(rt, prevP);

		// parentCount has 2 subscribers: parent + child1
		assert(
			fns.signal_subscriber_count(rt, parentCount),
			2,
			"parent signal has 2 subscribers (parent + child1)",
		);
		assert(
			fns.signal_subscriber_count(rt, child1Local),
			1,
			"child1 signal has 1 subscriber",
		);
		assert(
			fns.signal_subscriber_count(rt, child2Local),
			1,
			"child2 signal has 1 subscriber",
		);

		// Write to parent signal → parent and child1 dirty
		fns.signal_write_i32(rt, parentCount, 5);
		assert(
			fns.runtime_dirty_count(rt),
			2,
			"2 dirty scopes from parent signal write",
		);

		fns.runtime_destroy(rt);
	}

	// ── Scopes don't leak signals across runtime instances ───────────
	suite("Scopes — separate runtimes are isolated");
	{
		const rt1 = fns.runtime_create();
		const rt2 = fns.runtime_create();

		const s1 = fns.scope_create(rt1, 0, -1);
		const s2 = fns.scope_create(rt2, 0, -1);

		const prev1 = fns.scope_begin_render(rt1, s1);
		const k1 = fns.hook_use_signal_i32(rt1, 111);
		fns.scope_end_render(rt1, prev1);

		const prev2 = fns.scope_begin_render(rt2, s2);
		const k2 = fns.hook_use_signal_i32(rt2, 222);
		fns.scope_end_render(rt2, prev2);

		assert(fns.signal_read_i32(rt1, k1), 111, "runtime 1 signal is 111");
		assert(fns.signal_read_i32(rt2, k2), 222, "runtime 2 signal is 222");

		fns.runtime_destroy(rt1);
		fns.runtime_destroy(rt2);
	}

	// ── Edge case: scope with no hooks ───────────────────────────────
	suite("Scopes — render scope with no hooks");
	{
		const rt = fns.runtime_create();

		const s = fns.scope_create(rt, 0, -1);

		// Render with no hook calls (static component)
		const prev = fns.scope_begin_render(rt, s);
		// No hook calls — just render static content
		fns.scope_end_render(rt, prev);

		assert(fns.scope_render_count(rt, s), 1, "render_count is 1");
		assert(fns.scope_hook_count(rt, s), 0, "0 hooks for static component");

		// Re-render
		const prev2 = fns.scope_begin_render(rt, s);
		fns.scope_end_render(rt, prev2);

		assert(fns.scope_render_count(rt, s), 2, "render_count is 2");

		fns.runtime_destroy(rt);
	}
}

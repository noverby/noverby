import { writeStringStruct } from "../runtime/strings.ts";
import type { WasmExports } from "../runtime/types.ts";
import { assert, suite } from "./harness.ts";

// Action tag constants (must match Mojo ACTION_* aliases)
const ACTION_NONE = 0;
const ACTION_SIGNAL_SET_I32 = 1;
const ACTION_SIGNAL_ADD_I32 = 2;
const ACTION_SIGNAL_SUB_I32 = 3;
const ACTION_SIGNAL_TOGGLE = 4;
const ACTION_SIGNAL_SET_INPUT = 5;
const ACTION_CUSTOM = 255;

// Event type constants (must match Mojo EVT_* aliases)
const EVT_CLICK = 0;
const EVT_INPUT = 1;

export function testEvents(fns: WasmExports): void {
	// ── Handler registry lifecycle ────────────────────────────────────
	suite("Events — registry lifecycle");
	{
		const rt = fns.runtime_create();
		assert(fns.handler_count(rt), 0, "new runtime has 0 handlers");
		fns.runtime_destroy(rt);
	}

	// ── Register signal_add handler ──────────────────────────────────
	suite("Events — register signal_add handler");
	{
		const rt = fns.runtime_create();
		const evtName = writeStringStruct("click");

		const hid = fns.handler_register_signal_add(rt, 0, 0, 1, evtName);
		assert(hid, 0, "first handler gets id 0");
		assert(fns.handler_count(rt), 1, "handler_count is 1");
		assert(fns.handler_contains(rt, hid), 1, "handler exists");
		assert(fns.handler_scope_id(rt, hid), 0, "scope_id is 0");
		assert(
			fns.handler_action(rt, hid),
			ACTION_SIGNAL_ADD_I32,
			"action is SIGNAL_ADD_I32",
		);
		assert(fns.handler_signal_key(rt, hid), 0, "signal_key is 0");
		assert(fns.handler_operand(rt, hid), 1, "operand is 1");

		fns.runtime_destroy(rt);
	}

	// ── Register multiple handler types ──────────────────────────────
	suite("Events — register multiple handler types");
	{
		const rt = fns.runtime_create();
		const clickName = writeStringStruct("click");
		const inputName = writeStringStruct("input");

		const h0 = fns.handler_register_signal_add(rt, 0, 0, 1, clickName);
		const h1 = fns.handler_register_signal_sub(rt, 0, 0, 1, clickName);
		const h2 = fns.handler_register_signal_set(rt, 0, 0, 42, clickName);
		const h3 = fns.handler_register_signal_toggle(rt, 0, 0, clickName);
		const h4 = fns.handler_register_signal_set_input(rt, 0, 0, inputName);
		const h5 = fns.handler_register_custom(rt, 0, clickName);
		const h6 = fns.handler_register_noop(rt, 0, clickName);

		assert(fns.handler_count(rt), 7, "7 handlers registered");
		assert(h0, 0, "sequential id 0");
		assert(h1, 1, "sequential id 1");
		assert(h2, 2, "sequential id 2");
		assert(h3, 3, "sequential id 3");
		assert(h4, 4, "sequential id 4");
		assert(h5, 5, "sequential id 5");
		assert(h6, 6, "sequential id 6");

		assert(
			fns.handler_action(rt, h0),
			ACTION_SIGNAL_ADD_I32,
			"h0 action is ADD",
		);
		assert(
			fns.handler_action(rt, h1),
			ACTION_SIGNAL_SUB_I32,
			"h1 action is SUB",
		);
		assert(
			fns.handler_action(rt, h2),
			ACTION_SIGNAL_SET_I32,
			"h2 action is SET",
		);
		assert(
			fns.handler_action(rt, h3),
			ACTION_SIGNAL_TOGGLE,
			"h3 action is TOGGLE",
		);
		assert(
			fns.handler_action(rt, h4),
			ACTION_SIGNAL_SET_INPUT,
			"h4 action is SET_INPUT",
		);
		assert(fns.handler_action(rt, h5), ACTION_CUSTOM, "h5 action is CUSTOM");
		assert(fns.handler_action(rt, h6), ACTION_NONE, "h6 action is NONE");

		fns.runtime_destroy(rt);
	}

	// ── Remove handler ───────────────────────────────────────────────
	suite("Events — remove handler");
	{
		const rt = fns.runtime_create();
		const evtName = writeStringStruct("click");

		const h0 = fns.handler_register_signal_add(rt, 0, 0, 1, evtName);
		const h1 = fns.handler_register_signal_add(rt, 0, 0, 2, evtName);
		assert(fns.handler_count(rt), 2, "2 handlers before remove");

		fns.handler_remove(rt, h0);
		assert(fns.handler_count(rt), 1, "1 handler after remove");
		assert(fns.handler_contains(rt, h0), 0, "h0 no longer exists");
		assert(fns.handler_contains(rt, h1), 1, "h1 still exists");

		fns.runtime_destroy(rt);
	}

	// ── Slot reuse after remove ──────────────────────────────────────
	suite("Events — slot reuse after remove");
	{
		const rt = fns.runtime_create();
		const evtName = writeStringStruct("click");

		const h0 = fns.handler_register_signal_add(rt, 0, 0, 1, evtName);
		fns.handler_remove(rt, h0);
		assert(fns.handler_count(rt), 0, "0 handlers after remove");

		const h0b = fns.handler_register_signal_add(rt, 0, 0, 99, evtName);
		assert(h0b, h0, "reused slot has same id");
		assert(fns.handler_count(rt), 1, "1 handler after reuse");
		assert(fns.handler_operand(rt, h0b), 99, "reused slot has new operand");

		fns.runtime_destroy(rt);
	}

	// ── Dispatch signal_add ──────────────────────────────────────────
	suite("Events — dispatch signal_add");
	{
		const rt = fns.runtime_create();

		// Create a signal with initial value 0
		const sigKey = fns.signal_create_i32(rt, 0);
		assert(fns.signal_read_i32(rt, sigKey), 0, "signal starts at 0");

		// Create a scope so the signal has a subscriber
		const scopeId = fns.scope_create(rt, 0, -1);

		// Subscribe scope to signal by reading during render
		fns.scope_begin_render(rt, scopeId);
		fns.signal_read_i32(rt, sigKey); // subscribes scopeId
		fns.scope_end_render(rt, -1);

		const evtName = writeStringStruct("click");
		const hid = fns.handler_register_signal_add(
			rt,
			scopeId,
			sigKey,
			1,
			evtName,
		);

		// Dispatch the event
		const result = fns.dispatch_event(rt, hid, EVT_CLICK);
		assert(result, 1, "dispatch returns 1 (action executed)");
		assert(fns.signal_read_i32(rt, sigKey), 1, "signal is now 1 after +1");

		// Scope should be dirty
		assert(fns.runtime_has_dirty(rt), 1, "runtime has dirty scopes");

		// Dispatch again
		fns.dispatch_event(rt, hid, EVT_CLICK);
		assert(
			fns.signal_read_i32(rt, sigKey),
			2,
			"signal is now 2 after +1 again",
		);

		fns.runtime_destroy(rt);
	}

	// ── Dispatch signal_sub ──────────────────────────────────────────
	suite("Events — dispatch signal_sub");
	{
		const rt = fns.runtime_create();
		const sigKey = fns.signal_create_i32(rt, 10);
		const evtName = writeStringStruct("click");

		const hid = fns.handler_register_signal_sub(rt, 0, sigKey, 3, evtName);

		fns.dispatch_event(rt, hid, EVT_CLICK);
		assert(fns.signal_read_i32(rt, sigKey), 7, "10 - 3 = 7");

		fns.dispatch_event(rt, hid, EVT_CLICK);
		assert(fns.signal_read_i32(rt, sigKey), 4, "7 - 3 = 4");

		fns.runtime_destroy(rt);
	}

	// ── Dispatch signal_set ──────────────────────────────────────────
	suite("Events — dispatch signal_set");
	{
		const rt = fns.runtime_create();
		const sigKey = fns.signal_create_i32(rt, 0);
		const evtName = writeStringStruct("click");

		const hid = fns.handler_register_signal_set(rt, 0, sigKey, 42, evtName);

		fns.dispatch_event(rt, hid, EVT_CLICK);
		assert(fns.signal_read_i32(rt, sigKey), 42, "signal set to 42");

		// Dispatching again keeps the same value
		fns.dispatch_event(rt, hid, EVT_CLICK);
		assert(
			fns.signal_read_i32(rt, sigKey),
			42,
			"signal still 42 after second set",
		);

		fns.runtime_destroy(rt);
	}

	// ── Dispatch signal_toggle ───────────────────────────────────────
	suite("Events — dispatch signal_toggle");
	{
		const rt = fns.runtime_create();
		const sigKey = fns.signal_create_i32(rt, 0);
		const evtName = writeStringStruct("click");

		const hid = fns.handler_register_signal_toggle(rt, 0, sigKey, evtName);

		fns.dispatch_event(rt, hid, EVT_CLICK);
		assert(fns.signal_read_i32(rt, sigKey), 1, "toggled 0 → 1");

		fns.dispatch_event(rt, hid, EVT_CLICK);
		assert(fns.signal_read_i32(rt, sigKey), 0, "toggled 1 → 0");

		fns.dispatch_event(rt, hid, EVT_CLICK);
		assert(fns.signal_read_i32(rt, sigKey), 1, "toggled 0 → 1 again");

		fns.runtime_destroy(rt);
	}

	// ── Dispatch to non-existent handler ─────────────────────────────
	suite("Events — dispatch to non-existent handler");
	{
		const rt = fns.runtime_create();

		const result = fns.dispatch_event(rt, 999, EVT_CLICK);
		assert(result, 0, "dispatch to missing handler returns 0");

		fns.runtime_destroy(rt);
	}

	// ── Dispatch to removed handler ──────────────────────────────────
	suite("Events — dispatch to removed handler");
	{
		const rt = fns.runtime_create();
		const sigKey = fns.signal_create_i32(rt, 0);
		const evtName = writeStringStruct("click");

		const hid = fns.handler_register_signal_add(rt, 0, sigKey, 5, evtName);
		fns.handler_remove(rt, hid);

		const result = fns.dispatch_event(rt, hid, EVT_CLICK);
		assert(result, 0, "dispatch to removed handler returns 0");
		assert(
			fns.signal_read_i32(rt, sigKey),
			0,
			"signal unchanged after dispatch to removed handler",
		);

		fns.runtime_destroy(rt);
	}

	// ── Dispatch noop handler marks scope dirty ──────────────────────
	suite("Events — dispatch noop marks scope dirty");
	{
		const rt = fns.runtime_create();
		const scopeId = fns.scope_create(rt, 0, -1);
		const evtName = writeStringStruct("click");

		const hid = fns.handler_register_noop(rt, scopeId, evtName);

		assert(fns.runtime_has_dirty(rt), 0, "no dirty scopes before dispatch");

		const result = fns.dispatch_event(rt, hid, EVT_CLICK);
		assert(result, 0, "noop returns 0 (no action)");
		assert(fns.runtime_has_dirty(rt), 1, "scope is dirty after noop dispatch");

		fns.runtime_destroy(rt);
	}

	// ── Dispatch custom handler marks scope dirty ────────────────────
	suite("Events — dispatch custom marks scope dirty");
	{
		const rt = fns.runtime_create();
		const scopeId = fns.scope_create(rt, 0, -1);
		const evtName = writeStringStruct("click");

		const hid = fns.handler_register_custom(rt, scopeId, evtName);

		const result = fns.dispatch_event(rt, hid, EVT_CLICK);
		assert(result, 0, "custom returns 0 (no Mojo action)");
		assert(
			fns.runtime_has_dirty(rt),
			1,
			"scope is dirty after custom dispatch",
		);

		fns.runtime_destroy(rt);
	}

	// ── Dispatch with i32 value (set_input action) ───────────────────
	suite("Events — dispatch_event_with_i32 for set_input");
	{
		const rt = fns.runtime_create();
		const sigKey = fns.signal_create_i32(rt, 0);
		const evtName = writeStringStruct("input");

		const hid = fns.handler_register_signal_set_input(rt, 0, sigKey, evtName);

		const result = fns.dispatch_event_with_i32(rt, hid, EVT_INPUT, 77);
		assert(result, 1, "dispatch_with_i32 returns 1");
		assert(
			fns.signal_read_i32(rt, sigKey),
			77,
			"signal set to 77 from event payload",
		);

		fns.dispatch_event_with_i32(rt, hid, EVT_INPUT, -5);
		assert(
			fns.signal_read_i32(rt, sigKey),
			-5,
			"signal set to -5 from second payload",
		);

		fns.runtime_destroy(rt);
	}

	// ── Dispatch with i32 falls back for non-set_input actions ───────
	suite("Events — dispatch_event_with_i32 fallback");
	{
		const rt = fns.runtime_create();
		const sigKey = fns.signal_create_i32(rt, 10);
		const evtName = writeStringStruct("click");

		// Register a signal_add handler (not set_input)
		const hid = fns.handler_register_signal_add(rt, 0, sigKey, 5, evtName);

		// dispatch_event_with_i32 should fall back to normal dispatch
		const result = fns.dispatch_event_with_i32(rt, hid, EVT_CLICK, 999);
		assert(result, 1, "fallback dispatch returns 1");
		assert(
			fns.signal_read_i32(rt, sigKey),
			15,
			"signal_add executed: 10 + 5 = 15 (payload 999 ignored)",
		);

		fns.runtime_destroy(rt);
	}

	// ── Drain dirty scopes ───────────────────────────────────────────
	suite("Events — drain dirty scopes");
	{
		const rt = fns.runtime_create();
		const scopeId = fns.scope_create(rt, 0, -1);
		const sigKey = fns.signal_create_i32(rt, 0);

		// Subscribe scope to signal
		fns.scope_begin_render(rt, scopeId);
		fns.signal_read_i32(rt, sigKey);
		fns.scope_end_render(rt, -1);

		const evtName = writeStringStruct("click");
		const hid = fns.handler_register_signal_add(
			rt,
			scopeId,
			sigKey,
			1,
			evtName,
		);

		fns.dispatch_event(rt, hid, EVT_CLICK);
		assert(fns.runtime_has_dirty(rt), 1, "dirty before drain");

		const drained = fns.runtime_drain_dirty(rt);
		assert(drained, 1, "drained 1 dirty scope");
		assert(fns.runtime_has_dirty(rt), 0, "no dirty scopes after drain");

		fns.runtime_destroy(rt);
	}

	// ── Full counter flow: create → subscribe → dispatch → dirty ─────
	suite("Events — full counter flow");
	{
		const rt = fns.runtime_create();

		// 1. Create a scope
		const scopeId = fns.scope_create(rt, 0, -1);

		// 2. Begin render, create signal via hook
		fns.scope_begin_render(rt, scopeId);
		const countKey = fns.hook_use_signal_i32(rt, 0);
		assert(fns.signal_read_i32(rt, countKey), 0, "count starts at 0");
		fns.scope_end_render(rt, -1);

		// 3. Register click handler: count += 1
		const clickName = writeStringStruct("click");
		const hid = fns.handler_register_signal_add(
			rt,
			scopeId,
			countKey,
			1,
			clickName,
		);

		// 4. Simulate click events
		fns.dispatch_event(rt, hid, EVT_CLICK);
		assert(
			fns.signal_peek_i32(rt, countKey),
			1,
			"count is 1 after first click",
		);
		assert(fns.runtime_has_dirty(rt), 1, "scope is dirty");

		fns.dispatch_event(rt, hid, EVT_CLICK);
		assert(
			fns.signal_peek_i32(rt, countKey),
			2,
			"count is 2 after second click",
		);

		fns.dispatch_event(rt, hid, EVT_CLICK);
		assert(
			fns.signal_peek_i32(rt, countKey),
			3,
			"count is 3 after third click",
		);

		// 5. Drain dirty and verify
		const dirtyCount = fns.runtime_drain_dirty(rt);
		assert(dirtyCount, 1, "only 1 unique dirty scope (deduplicated)");
		assert(fns.runtime_has_dirty(rt), 0, "clean after drain");

		// 6. Signal value persists
		assert(fns.signal_peek_i32(rt, countKey), 3, "count still 3 after drain");

		fns.runtime_destroy(rt);
	}

	// ── Multiple handlers on different signals ───────────────────────
	suite("Events — multiple handlers on different signals");
	{
		const rt = fns.runtime_create();

		const sig0 = fns.signal_create_i32(rt, 0);
		const sig1 = fns.signal_create_i32(rt, 100);

		const clickName = writeStringStruct("click");

		const h0 = fns.handler_register_signal_add(rt, 0, sig0, 1, clickName);
		const h1 = fns.handler_register_signal_sub(rt, 0, sig1, 10, clickName);

		fns.dispatch_event(rt, h0, EVT_CLICK);
		fns.dispatch_event(rt, h1, EVT_CLICK);

		assert(fns.signal_read_i32(rt, sig0), 1, "sig0: 0 + 1 = 1");
		assert(fns.signal_read_i32(rt, sig1), 90, "sig1: 100 - 10 = 90");

		fns.dispatch_event(rt, h0, EVT_CLICK);
		fns.dispatch_event(rt, h0, EVT_CLICK);
		fns.dispatch_event(rt, h1, EVT_CLICK);

		assert(fns.signal_read_i32(rt, sig0), 3, "sig0: 1 + 1 + 1 = 3");
		assert(fns.signal_read_i32(rt, sig1), 80, "sig1: 90 - 10 = 80");

		fns.runtime_destroy(rt);
	}

	// ── Register 100 handlers, remove 50, dispatch to remaining ──────
	suite("Events — bulk register/remove/dispatch");
	{
		const rt = fns.runtime_create();
		const sigKey = fns.signal_create_i32(rt, 0);
		const evtName = writeStringStruct("click");

		const handlers: number[] = [];
		for (let i = 0; i < 100; i++) {
			handlers.push(fns.handler_register_signal_add(rt, 0, sigKey, 1, evtName));
		}
		assert(fns.handler_count(rt), 100, "100 handlers registered");

		// Remove even-indexed handlers
		for (let i = 0; i < 100; i += 2) {
			fns.handler_remove(rt, handlers[i]);
		}
		assert(fns.handler_count(rt), 50, "50 handlers after removing evens");

		// Dispatch to all remaining (odd-indexed) handlers
		for (let i = 1; i < 100; i += 2) {
			fns.dispatch_event(rt, handlers[i], EVT_CLICK);
		}
		assert(
			fns.signal_read_i32(rt, sigKey),
			50,
			"signal is 50 after 50 dispatches of +1",
		);

		// Dispatch to removed handlers should be no-ops
		for (let i = 0; i < 100; i += 2) {
			fns.dispatch_event(rt, handlers[i], EVT_CLICK);
		}
		assert(
			fns.signal_read_i32(rt, sigKey),
			50,
			"signal still 50 after dispatching to removed handlers",
		);

		fns.runtime_destroy(rt);
	}

	// ── Signal version increments on dispatch ────────────────────────
	suite("Events — signal version increments");
	{
		const rt = fns.runtime_create();
		const sigKey = fns.signal_create_i32(rt, 0);
		const evtName = writeStringStruct("click");

		const v0 = fns.signal_version(rt, sigKey);
		assert(v0, 0, "initial version is 0");

		const hid = fns.handler_register_signal_add(rt, 0, sigKey, 1, evtName);

		fns.dispatch_event(rt, hid, EVT_CLICK);
		assert(
			fns.signal_version(rt, sigKey),
			1,
			"version is 1 after first dispatch",
		);

		fns.dispatch_event(rt, hid, EVT_CLICK);
		assert(
			fns.signal_version(rt, sigKey),
			2,
			"version is 2 after second dispatch",
		);

		fns.runtime_destroy(rt);
	}
}

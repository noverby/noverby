// --- Types for WASM module exports ---

export interface WasmExports extends WebAssembly.Exports {
	memory: WebAssembly.Memory;
	__heap_base: WebAssembly.Global;
	__heap_end: WebAssembly.Global;

	// Arithmetic — add
	add_int32(x: number, y: number): number;
	add_int64(x: bigint, y: bigint): bigint;
	add_float32(x: number, y: number): number;
	add_float64(x: number, y: number): number;

	// Arithmetic — subtract
	sub_int32(x: number, y: number): number;
	sub_int64(x: bigint, y: bigint): bigint;
	sub_float32(x: number, y: number): number;
	sub_float64(x: number, y: number): number;

	// Arithmetic — multiply
	mul_int32(x: number, y: number): number;
	mul_int64(x: bigint, y: bigint): bigint;
	mul_float32(x: number, y: number): number;
	mul_float64(x: number, y: number): number;

	// Arithmetic — division
	div_int32(x: number, y: number): number;
	div_int64(x: bigint, y: bigint): bigint;
	div_float32(x: number, y: number): number;
	div_float64(x: number, y: number): number;

	// Arithmetic — modulo
	mod_int32(x: number, y: number): number;
	mod_int64(x: bigint, y: bigint): bigint;

	// Arithmetic — power
	pow_int32(x: number): number;
	pow_int64(x: bigint): bigint;
	pow_float32(x: number): number;
	pow_float64(x: number): number;

	// Unary — negate
	neg_int32(x: number): number;
	neg_int64(x: bigint): bigint;
	neg_float32(x: number): number;
	neg_float64(x: number): number;

	// Unary — absolute value
	abs_int32(x: number): number;
	abs_int64(x: bigint): bigint;
	abs_float32(x: number): number;
	abs_float64(x: number): number;

	// Min / Max
	min_int32(x: number, y: number): number;
	max_int32(x: number, y: number): number;
	min_int64(x: bigint, y: bigint): bigint;
	max_int64(x: bigint, y: bigint): bigint;
	min_float64(x: number, y: number): number;
	max_float64(x: number, y: number): number;

	// Clamp
	clamp_int32(x: number, lo: number, hi: number): number;
	clamp_float64(x: number, lo: number, hi: number): number;

	// Bitwise
	bitand_int32(x: number, y: number): number;
	bitor_int32(x: number, y: number): number;
	bitxor_int32(x: number, y: number): number;
	bitnot_int32(x: number): number;
	shl_int32(x: number, y: number): number;
	shr_int32(x: number, y: number): number;

	// Comparison
	eq_int32(x: number, y: number): number;
	ne_int32(x: number, y: number): number;
	lt_int32(x: number, y: number): number;
	le_int32(x: number, y: number): number;
	gt_int32(x: number, y: number): number;
	ge_int32(x: number, y: number): number;

	// Boolean logic
	bool_and(x: number, y: number): number;
	bool_or(x: number, y: number): number;
	bool_not(x: number): number;

	// Algorithms — fibonacci
	fib_int32(n: number): number;
	fib_int64(n: bigint): bigint;

	// Algorithms — factorial
	factorial_int32(n: number): number;
	factorial_int64(n: bigint): bigint;

	// Algorithms — GCD
	gcd_int32(x: number, y: number): number;

	// Identity / passthrough
	identity_int32(x: number): number;
	identity_int64(x: bigint): bigint;
	identity_float32(x: number): number;
	identity_float64(x: number): number;

	// ── Mutation Protocol ────────────────────────────────────────────

	// Buffer management
	mutation_buf_alloc(capacity: number): bigint;
	mutation_buf_free(ptr: bigint): void;

	// Simple opcodes (no string/path payload)
	write_op_end(buf: bigint, off: number): number;
	write_op_append_children(
		buf: bigint,
		off: number,
		id: number,
		m: number,
	): number;
	write_op_create_placeholder(buf: bigint, off: number, id: number): number;
	write_op_load_template(
		buf: bigint,
		off: number,
		tmplId: number,
		index: number,
		id: number,
	): number;
	write_op_replace_with(
		buf: bigint,
		off: number,
		id: number,
		m: number,
	): number;
	write_op_insert_after(
		buf: bigint,
		off: number,
		id: number,
		m: number,
	): number;
	write_op_insert_before(
		buf: bigint,
		off: number,
		id: number,
		m: number,
	): number;
	write_op_remove(buf: bigint, off: number, id: number): number;
	write_op_push_root(buf: bigint, off: number, id: number): number;

	// String-carrying opcodes (text param is a Mojo String struct pointer)
	write_op_create_text_node(
		buf: bigint,
		off: number,
		id: number,
		text: bigint,
	): number;
	write_op_set_text(buf: bigint, off: number, id: number, text: bigint): number;
	write_op_set_attribute(
		buf: bigint,
		off: number,
		id: number,
		ns: number,
		name: bigint,
		value: bigint,
	): number;
	write_op_new_event_listener(
		buf: bigint,
		off: number,
		id: number,
		name: bigint,
	): number;
	write_op_remove_event_listener(
		buf: bigint,
		off: number,
		id: number,
		name: bigint,
	): number;

	// Path-carrying opcodes
	write_op_assign_id(
		buf: bigint,
		off: number,
		pathPtr: bigint,
		pathLen: number,
		id: number,
	): number;
	write_op_replace_placeholder(
		buf: bigint,
		off: number,
		pathPtr: bigint,
		pathLen: number,
		m: number,
	): number;

	// Composite test helper
	write_test_sequence(buf: bigint): number;

	// Print
	print_static_string(): void;
	print_int32(): void;
	print_int64(): void;
	print_float32(): void;
	print_float64(): void;
	print_input_string(structPtr: bigint): void;

	// Return string
	return_static_string(outStructPtr: bigint): void;
	return_input_string(inStructPtr: bigint, outStructPtr: bigint): void;

	// String ops
	string_length(structPtr: bigint): bigint;
	string_concat(
		xStructPtr: bigint,
		yStructPtr: bigint,
		outStructPtr: bigint,
	): void;
	string_repeat(xStructPtr: bigint, n: number, outStructPtr: bigint): void;
	string_eq(xStructPtr: bigint, yStructPtr: bigint): number;

	// ── ElementId Allocator ──────────────────────────────────────────

	eid_alloc_create(): bigint;
	eid_alloc_destroy(allocPtr: bigint): void;
	eid_alloc(allocPtr: bigint): number;
	eid_free(allocPtr: bigint, id: number): void;
	eid_is_alive(allocPtr: bigint, id: number): number;
	eid_count(allocPtr: bigint): number;
	eid_user_count(allocPtr: bigint): number;

	// ── Reactive Runtime / Signals ───────────────────────────────────

	// Runtime lifecycle
	runtime_create(): bigint;
	runtime_destroy(rtPtr: bigint): void;

	// Signal CRUD
	signal_create_i32(rtPtr: bigint, initial: number): number;
	signal_read_i32(rtPtr: bigint, key: number): number;
	signal_write_i32(rtPtr: bigint, key: number, value: number): void;
	signal_peek_i32(rtPtr: bigint, key: number): number;
	signal_destroy(rtPtr: bigint, key: number): void;

	// Signal queries
	signal_subscriber_count(rtPtr: bigint, key: number): number;
	signal_version(rtPtr: bigint, key: number): number;
	signal_count(rtPtr: bigint): number;
	signal_contains(rtPtr: bigint, key: number): number;

	// Signal arithmetic helpers
	signal_iadd_i32(rtPtr: bigint, key: number, rhs: number): void;
	signal_isub_i32(rtPtr: bigint, key: number, rhs: number): void;

	// Context management
	runtime_set_context(rtPtr: bigint, contextId: number): void;
	runtime_clear_context(rtPtr: bigint): void;
	runtime_has_context(rtPtr: bigint): number;
	runtime_dirty_count(rtPtr: bigint): number;
	runtime_has_dirty(rtPtr: bigint): number;
}

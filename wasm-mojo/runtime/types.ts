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
}

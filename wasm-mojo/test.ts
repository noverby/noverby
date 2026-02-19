const encoder = new TextEncoder();
const decoder = new TextDecoder();

// --- Types for WASM module exports ---

interface WasmExports extends WebAssembly.Exports {
	memory: WebAssembly.Memory;
	__heap_base: WebAssembly.Global;
	__heap_end: WebAssembly.Global;
	// Add
	add_int32(x: number, y: number): number;
	add_int64(x: bigint, y: bigint): bigint;
	add_float32(x: number, y: number): number;
	add_float64(x: number, y: number): number;
	// Subtract
	sub_int32(x: number, y: number): number;
	sub_int64(x: bigint, y: bigint): bigint;
	sub_float32(x: number, y: number): number;
	sub_float64(x: number, y: number): number;
	// Multiply
	mul_int32(x: number, y: number): number;
	mul_int64(x: bigint, y: bigint): bigint;
	mul_float32(x: number, y: number): number;
	mul_float64(x: number, y: number): number;
	// Division
	div_int32(x: number, y: number): number;
	div_int64(x: bigint, y: bigint): bigint;
	div_float32(x: number, y: number): number;
	div_float64(x: number, y: number): number;
	// Modulo
	mod_int32(x: number, y: number): number;
	mod_int64(x: bigint, y: bigint): bigint;
	// Power
	pow_int32(x: number): number;
	pow_int64(x: bigint): bigint;
	pow_float32(x: number): number;
	pow_float64(x: number): number;
	// Negate
	neg_int32(x: number): number;
	neg_int64(x: bigint): bigint;
	neg_float32(x: number): number;
	neg_float64(x: number): number;
	// Absolute value
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
	// Boolean / comparison
	eq_int32(x: number, y: number): number;
	ne_int32(x: number, y: number): number;
	lt_int32(x: number, y: number): number;
	le_int32(x: number, y: number): number;
	gt_int32(x: number, y: number): number;
	ge_int32(x: number, y: number): number;
	bool_and(x: number, y: number): number;
	bool_or(x: number, y: number): number;
	bool_not(x: number): number;
	// Fibonacci
	fib_int32(n: number): number;
	fib_int64(n: bigint): bigint;
	// Factorial
	factorial_int32(n: number): number;
	factorial_int64(n: bigint): bigint;
	// GCD
	gcd_int32(x: number, y: number): number;
	// Identity
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

// Mojo String ABI: 24-byte struct { data_ptr: i64, len: i64, capacity: i64 }
const STRING_STRUCT_SIZE = 24n;
const STRING_STRUCT_ALIGN = 8n;

// --- WASM runtime state ---

let heapPointer: bigint;
let wasmExports: WasmExports;
let memory: WebAssembly.Memory;

const getView = (): DataView => new DataView(memory.buffer);

const initializeEnv = (instance: WebAssembly.Instance): void => {
	wasmExports = instance.exports as unknown as WasmExports;
	memory = wasmExports.memory;
	heapPointer = wasmExports.__heap_base.value as bigint;
};

// --- Env stubs for the WASM module ---

const write = (fd: bigint, ptr: bigint, len: bigint): number => {
	if (len === 0n) return 0;

	try {
		if (Number(ptr + len) > memory.buffer.byteLength) {
			console.error("Write would exceed memory bounds");
			return -1;
		}

		const data = new Uint8Array(memory.buffer, Number(ptr), Number(len));
		const text = decoder.decode(data);

		if (fd === 1n) {
			Deno.stdout.writeSync(encoder.encode(text));
			return Number(len);
		}

		if (fd === 2n) {
			Deno.stderr.writeSync(encoder.encode(text));
			return Number(len);
		}

		console.log("unhandled fd:", fd);
		return -1;
	} catch (error) {
		console.error("Write error:", error);
		return -1;
	}
};

const KGEN_CompilerRT_AlignedAlloc = (align: bigint, size: bigint): bigint => {
	const alignNum = BigInt(align);
	const remainder = heapPointer % alignNum;
	if (remainder !== 0n) {
		heapPointer += alignNum - remainder;
	}
	const ptr = heapPointer;
	heapPointer += size;
	return ptr;
};

const KGEN_CompilerRT_AlignedFree = (_ptr: bigint): number => {
	return 1;
};

const KGEN_CompilerRT_GetStackTrace = (
	_buf: bigint,
	_maxFrames: bigint,
): bigint => {
	return 0n;
};

const KGEN_CompilerRT_fprintf = (
	_stream: bigint,
	_fmtPtr: bigint,
	..._args: bigint[]
): number => {
	return 0;
};

const env: WebAssembly.ModuleImports = {
	memory: new WebAssembly.Memory({ initial: 2 }),
	__cxa_atexit: (_func: bigint, _obj: bigint, _dso: bigint): number => 0,
	KGEN_CompilerRT_AlignedAlloc,
	KGEN_CompilerRT_AlignedFree,
	KGEN_CompilerRT_GetStackTrace,
	KGEN_CompilerRT_fprintf,
	write,
	free: KGEN_CompilerRT_AlignedFree,
	dup: (_fd: bigint): number => 1,
	fdopen: (_fd: bigint, _modePtr: bigint): number => 1,
	fflush: (_stream: bigint): number => 1,
	fclose: (_stream: bigint): number => 1,
	fmaf: (x: number, y: number, z: number): number =>
		Math.fround(Math.fround(x * y) + z),
	fminf: (x: number, y: number): number => (x > y ? y : x),
	fmaxf: (x: number, y: number): number => (x > y ? x : y),
	fma: (x: number, y: number, z: number): number => x * y + z,
	fmin: (x: number, y: number): number => (x > y ? y : x),
	fmax: (x: number, y: number): number => (x > y ? x : y),
};

// --- Memory helpers for Mojo String ABI ---
// Mojo String struct (wasm64): { data_ptr: i64, len: i64, capacity: i64 }

/** Allocate a Mojo String struct in WASM memory and populate it with the given JS string. */
const writeStringStruct = (str: string): bigint => {
	const bytes = encoder.encode(str);
	const dataLen = BigInt(bytes.length);

	// Allocate buffer for string data (with null terminator)
	const dataPtr = KGEN_CompilerRT_AlignedAlloc(1n, dataLen + 1n);
	new Uint8Array(memory.buffer).set(bytes, Number(dataPtr));
	new Uint8Array(memory.buffer)[Number(dataPtr + dataLen)] = 0;

	// Allocate 24-byte String struct
	const structPtr = KGEN_CompilerRT_AlignedAlloc(
		STRING_STRUCT_ALIGN,
		STRING_STRUCT_SIZE,
	);
	const view = getView();
	// data_ptr at offset 0
	view.setBigInt64(Number(structPtr), dataPtr, true);
	// len at offset 8 (string byte count, excludes null terminator)
	view.setBigInt64(Number(structPtr) + 8, dataLen, true);
	// capacity at offset 16
	view.setBigInt64(Number(structPtr) + 16, dataLen + 1n, true);

	return structPtr;
};

/** Allocate an empty Mojo String struct to be used as an output parameter. */
const allocStringStruct = (): bigint => {
	const structPtr = KGEN_CompilerRT_AlignedAlloc(
		STRING_STRUCT_ALIGN,
		STRING_STRUCT_SIZE,
	);
	// Zero-initialize
	const view = getView();
	view.setBigInt64(Number(structPtr), 0n, true);
	view.setBigInt64(Number(structPtr) + 8, 0n, true);
	view.setBigInt64(Number(structPtr) + 16, 0n, true);
	return structPtr;
};

/** Read a JS string from a Mojo String struct in WASM memory.
 *  Handles Mojo's Small String Optimization (SSO):
 *  - Bit 63 of the capacity field is the SSO flag.
 *  - When set, the string data is stored inline at the struct address
 *    and bits 56-60 of the capacity encode the byte length.
 *  - When clear, data_ptr (offset 0) and len (offset 8) are used directly.
 */
const readStringStruct = (structPtr: bigint): string => {
	const view = getView();
	const capacity = view.getBigUint64(Number(structPtr) + 16, true);

	const SSO_FLAG = 0x8000000000000000n;
	const SSO_LEN_MASK = 0x1f00000000000000n;

	let dataPtr: bigint;
	let len: bigint;

	if ((capacity & SSO_FLAG) !== 0n) {
		// SSO: data is stored inline starting at the struct itself
		dataPtr = structPtr;
		len = (capacity & SSO_LEN_MASK) >> 56n;
	} else {
		dataPtr = view.getBigInt64(Number(structPtr), true);
		len = view.getBigInt64(Number(structPtr) + 8, true);
	}

	if (len <= 0n) return "";

	const bytes = new Uint8Array(memory.buffer, Number(dataPtr), Number(len));
	return decoder.decode(bytes);
};

// --- Test harness ---

let passed = 0;
let failed = 0;

const suite = (name: string): void => {
	console.log(`\n  ${name}`);
};

const assert = <T>(actual: T, expected: T, label: string): void => {
	if (actual === expected) {
		passed++;
		console.log(`    ✓ ${label}`);
	} else {
		failed++;
		console.log(
			`    ✗ ${label}\n      expected: ${JSON.stringify(expected)}\n      actual:   ${JSON.stringify(actual)}`,
		);
	}
};

const assertClose = (
	actual: number,
	expected: number,
	epsilon: number,
	label: string,
): void => {
	if (Math.abs(actual - expected) < epsilon) {
		passed++;
		console.log(`    ✓ ${label}`);
	} else {
		failed++;
		console.log(
			`    ✗ ${label}\n      expected: ≈${expected} (±${epsilon})\n      actual:   ${actual}`,
		);
	}
};

// --- Main ---

async function run(): Promise<void> {
	const wasmBuffer = await Deno.readFile(
		new URL("build/out.wasm", import.meta.url),
	);
	const { instance } = await WebAssembly.instantiate(wasmBuffer, { env });
	initializeEnv(instance);

	const fns = wasmExports;

	console.log("wasm-mojo tests\n");

	// =====================================================================
	// Add
	// =====================================================================
	suite("add");
	assert(fns.add_int32(2, 3), 5, "add_int32(2, 3) === 5");
	assert(fns.add_int64(2n, 3n), 5n, "add_int64(2, 3) === 5");
	assert(
		fns.add_float32(2.2, 3.3),
		Math.fround(2.2) + Math.fround(3.3),
		"add_float32(2.2, 3.3)",
	);
	assert(fns.add_float64(2.2, 3.3), 2.2 + 3.3, "add_float64(2.2, 3.3)");

	suite("add — edge cases");
	assert(fns.add_int32(0, 0), 0, "add_int32(0, 0) === 0");
	assert(fns.add_int32(-5, 5), 0, "add_int32(-5, 5) === 0");
	assert(fns.add_int32(-3, -7), -10, "add_int32(-3, -7) === -10");
	assert(fns.add_int64(0n, 0n), 0n, "add_int64(0, 0) === 0");
	assert(fns.add_int64(-100n, 100n), 0n, "add_int64(-100, 100) === 0");
	assert(fns.add_float64(0.0, 0.0), 0.0, "add_float64(0, 0) === 0");
	assert(fns.add_float64(-1.5, 1.5), 0.0, "add_float64(-1.5, 1.5) === 0");
	assert(fns.add_int32(1, 0), 1, "add_int32(1, 0) === 1 (identity)");
	assert(fns.add_int64(1n, 0n), 1n, "add_int64(1, 0) === 1 (identity)");

	// =====================================================================
	// Subtract
	// =====================================================================
	suite("sub");
	assert(fns.sub_int32(10, 3), 7, "sub_int32(10, 3) === 7");
	assert(fns.sub_int64(10n, 3n), 7n, "sub_int64(10, 3) === 7");
	assert(
		fns.sub_float32(5.5, 2.2),
		Math.fround(5.5) - Math.fround(2.2),
		"sub_float32(5.5, 2.2)",
	);
	assert(fns.sub_float64(5.5, 2.2), 5.5 - 2.2, "sub_float64(5.5, 2.2)");

	suite("sub — edge cases");
	assert(fns.sub_int32(0, 0), 0, "sub_int32(0, 0) === 0");
	assert(fns.sub_int32(5, 5), 0, "sub_int32(5, 5) === 0");
	assert(fns.sub_int32(3, 7), -4, "sub_int32(3, 7) === -4");
	assert(fns.sub_int32(-3, -7), 4, "sub_int32(-3, -7) === 4");
	assert(fns.sub_int64(0n, 0n), 0n, "sub_int64(0, 0) === 0");
	assert(fns.sub_int64(-50n, -50n), 0n, "sub_int64(-50, -50) === 0");

	// =====================================================================
	// Multiply
	// =====================================================================
	suite("mul");
	assert(fns.mul_int32(4, 5), 20, "mul_int32(4, 5) === 20");
	assert(fns.mul_int64(4n, 5n), 20n, "mul_int64(4, 5) === 20");
	assert(
		fns.mul_float32(2.0, 3.0),
		Math.fround(2.0) * Math.fround(3.0),
		"mul_float32(2.0, 3.0)",
	);
	assert(
		fns.mul_float64(2.5, 4.0),
		2.5 * 4.0,
		"mul_float64(2.5, 4.0) === 10.0",
	);

	suite("mul — edge cases");
	assert(fns.mul_int32(0, 100), 0, "mul_int32(0, 100) === 0");
	assert(fns.mul_int32(1, 42), 42, "mul_int32(1, 42) === 42 (identity)");
	assert(fns.mul_int32(-1, 42), -42, "mul_int32(-1, 42) === -42");
	assert(fns.mul_int32(-3, -4), 12, "mul_int32(-3, -4) === 12");
	assert(fns.mul_int64(0n, 999n), 0n, "mul_int64(0, 999) === 0");
	assert(fns.mul_int64(1n, 999n), 999n, "mul_int64(1, 999) === 999 (identity)");
	assert(fns.mul_float64(0.0, 123.456), 0.0, "mul_float64(0, 123.456) === 0");

	// =====================================================================
	// Division
	// =====================================================================
	suite("div");
	assert(fns.div_int32(20, 4), 5, "div_int32(20, 4) === 5");
	assert(fns.div_int64(20n, 4n), 5n, "div_int64(20, 4) === 5");
	assert(
		fns.div_float32(10.0, 4.0),
		Math.fround(10.0 / 4.0),
		"div_float32(10.0, 4.0) === 2.5",
	);
	assert(fns.div_float64(10.0, 4.0), 2.5, "div_float64(10.0, 4.0) === 2.5");

	suite("div — edge cases");
	assert(fns.div_int32(7, 2), 3, "div_int32(7, 2) === 3 (floor division)");
	assert(fns.div_int64(7n, 2n), 3n, "div_int64(7, 2) === 3 (floor division)");
	assert(fns.div_int32(0, 5), 0, "div_int32(0, 5) === 0");
	assert(fns.div_int32(1, 1), 1, "div_int32(1, 1) === 1");
	assert(fns.div_int32(-7, 2), -4, "div_int32(-7, 2) === -4 (floor division)");
	assert(fns.div_float64(1.0, 3.0), 1.0 / 3.0, "div_float64(1, 3)");
	assert(fns.div_float64(0.0, 1.0), 0.0, "div_float64(0, 1) === 0");

	// =====================================================================
	// Modulo
	// =====================================================================
	suite("mod");
	assert(fns.mod_int32(10, 3), 1, "mod_int32(10, 3) === 1");
	assert(fns.mod_int64(10n, 3n), 1n, "mod_int64(10, 3) === 1");
	assert(fns.mod_int32(15, 5), 0, "mod_int32(15, 5) === 0");
	assert(fns.mod_int32(0, 7), 0, "mod_int32(0, 7) === 0");
	assert(fns.mod_int32(7, 1), 0, "mod_int32(7, 1) === 0");
	assert(fns.mod_int64(100n, 7n), 2n, "mod_int64(100, 7) === 2");

	// =====================================================================
	// Power
	// =====================================================================
	suite("pow");
	assert(fns.pow_int32(3), 27, "pow_int32(3) === 27");
	assert(fns.pow_int64(3n), 27n, "pow_int64(3) === 27");
	assert(
		String(fns.pow_float32(3.3)),
		String(fns.pow_float32(3.3)),
		"pow_float32(3.3) is stable",
	);
	assert(String(fns.pow_float64(3.3)), "51.41572944937184", "pow_float64(3.3)");

	suite("pow — edge cases");
	assert(fns.pow_int32(1), 1, "pow_int32(1) === 1 (1^1)");
	assert(fns.pow_int32(2), 4, "pow_int32(2) === 4 (2^2)");
	assert(fns.pow_int64(1n), 1n, "pow_int64(1) === 1 (1^1)");
	assert(fns.pow_int64(2n), 4n, "pow_int64(2) === 4 (2^2)");
	assertClose(fns.pow_float64(1.0), 1.0, 1e-15, "pow_float64(1.0) ≈ 1.0");
	assertClose(fns.pow_float64(2.0), 4.0, 1e-15, "pow_float64(2.0) ≈ 4.0");

	// =====================================================================
	// Negate
	// =====================================================================
	suite("neg");
	assert(fns.neg_int32(5), -5, "neg_int32(5) === -5");
	assert(fns.neg_int32(-5), 5, "neg_int32(-5) === 5");
	assert(fns.neg_int32(0), 0, "neg_int32(0) === 0");
	assert(fns.neg_int64(42n), -42n, "neg_int64(42) === -42");
	assert(fns.neg_int64(-42n), 42n, "neg_int64(-42) === 42");
	assert(fns.neg_int64(0n), 0n, "neg_int64(0) === 0");
	assert(fns.neg_float32(3.14), -Math.fround(3.14), "neg_float32(3.14)");
	assert(fns.neg_float64(3.14), -3.14, "neg_float64(3.14) === -3.14");
	assert(fns.neg_float64(-3.14), 3.14, "neg_float64(-3.14) === 3.14");
	assert(fns.neg_float64(0.0), -0.0, "neg_float64(0) === -0");

	// =====================================================================
	// Absolute value
	// =====================================================================
	suite("abs");
	assert(fns.abs_int32(5), 5, "abs_int32(5) === 5");
	assert(fns.abs_int32(-5), 5, "abs_int32(-5) === 5");
	assert(fns.abs_int32(0), 0, "abs_int32(0) === 0");
	assert(fns.abs_int64(99n), 99n, "abs_int64(99) === 99");
	assert(fns.abs_int64(-99n), 99n, "abs_int64(-99) === 99");
	assert(fns.abs_int64(0n), 0n, "abs_int64(0) === 0");
	assert(fns.abs_float32(2.5), Math.fround(2.5), "abs_float32(2.5)");
	assert(fns.abs_float32(-2.5), Math.fround(2.5), "abs_float32(-2.5)");
	assert(fns.abs_float64(3.14), 3.14, "abs_float64(3.14) === 3.14");
	assert(fns.abs_float64(-3.14), 3.14, "abs_float64(-3.14) === 3.14");
	assert(fns.abs_float64(0.0), 0.0, "abs_float64(0) === 0");

	// =====================================================================
	// Min / Max
	// =====================================================================
	suite("min/max int32");
	assert(fns.min_int32(3, 7), 3, "min_int32(3, 7) === 3");
	assert(fns.min_int32(7, 3), 3, "min_int32(7, 3) === 3");
	assert(fns.min_int32(5, 5), 5, "min_int32(5, 5) === 5");
	assert(fns.min_int32(-3, 3), -3, "min_int32(-3, 3) === -3");
	assert(fns.max_int32(3, 7), 7, "max_int32(3, 7) === 7");
	assert(fns.max_int32(7, 3), 7, "max_int32(7, 3) === 7");
	assert(fns.max_int32(5, 5), 5, "max_int32(5, 5) === 5");
	assert(fns.max_int32(-3, 3), 3, "max_int32(-3, 3) === 3");

	suite("min/max int64");
	assert(fns.min_int64(3n, 7n), 3n, "min_int64(3, 7) === 3");
	assert(fns.min_int64(7n, 3n), 3n, "min_int64(7, 3) === 3");
	assert(fns.min_int64(-10n, 10n), -10n, "min_int64(-10, 10) === -10");
	assert(fns.max_int64(3n, 7n), 7n, "max_int64(3, 7) === 7");
	assert(fns.max_int64(7n, 3n), 7n, "max_int64(7, 3) === 7");
	assert(fns.max_int64(-10n, 10n), 10n, "max_int64(-10, 10) === 10");

	suite("min/max float64");
	assert(fns.min_float64(1.1, 2.2), 1.1, "min_float64(1.1, 2.2) === 1.1");
	assert(fns.min_float64(2.2, 1.1), 1.1, "min_float64(2.2, 1.1) === 1.1");
	assert(fns.min_float64(-0.5, 0.5), -0.5, "min_float64(-0.5, 0.5) === -0.5");
	assert(fns.max_float64(1.1, 2.2), 2.2, "max_float64(1.1, 2.2) === 2.2");
	assert(fns.max_float64(2.2, 1.1), 2.2, "max_float64(2.2, 1.1) === 2.2");
	assert(fns.max_float64(-0.5, 0.5), 0.5, "max_float64(-0.5, 0.5) === 0.5");

	// =====================================================================
	// Clamp
	// =====================================================================
	suite("clamp");
	assert(
		fns.clamp_int32(5, 0, 10),
		5,
		"clamp_int32(5, 0, 10) === 5 (within range)",
	);
	assert(fns.clamp_int32(-5, 0, 10), 0, "clamp_int32(-5, 0, 10) === 0 (below)");
	assert(
		fns.clamp_int32(15, 0, 10),
		10,
		"clamp_int32(15, 0, 10) === 10 (above)",
	);
	assert(
		fns.clamp_int32(0, 0, 10),
		0,
		"clamp_int32(0, 0, 10) === 0 (at low bound)",
	);
	assert(
		fns.clamp_int32(10, 0, 10),
		10,
		"clamp_int32(10, 0, 10) === 10 (at high bound)",
	);
	assert(
		fns.clamp_float64(5.5, 0.0, 10.0),
		5.5,
		"clamp_float64(5.5, 0, 10) === 5.5",
	);
	assert(
		fns.clamp_float64(-1.0, 0.0, 10.0),
		0.0,
		"clamp_float64(-1, 0, 10) === 0",
	);
	assert(
		fns.clamp_float64(11.0, 0.0, 10.0),
		10.0,
		"clamp_float64(11, 0, 10) === 10",
	);

	// =====================================================================
	// Bitwise
	// =====================================================================
	suite("bitwise");
	assert(
		fns.bitand_int32(0b1100, 0b1010),
		0b1000,
		"bitand_int32(0b1100, 0b1010) === 0b1000",
	);
	assert(
		fns.bitand_int32(0xff, 0x0f),
		0x0f,
		"bitand_int32(0xFF, 0x0F) === 0x0F",
	);
	assert(fns.bitand_int32(0, 0xffff), 0, "bitand_int32(0, 0xFFFF) === 0");
	assert(
		fns.bitor_int32(0b1100, 0b1010),
		0b1110,
		"bitor_int32(0b1100, 0b1010) === 0b1110",
	);
	assert(fns.bitor_int32(0, 0), 0, "bitor_int32(0, 0) === 0");
	assert(
		fns.bitxor_int32(0b1100, 0b1010),
		0b0110,
		"bitxor_int32(0b1100, 0b1010) === 0b0110",
	);
	assert(fns.bitxor_int32(42, 42), 0, "bitxor_int32(42, 42) === 0");
	assert(fns.bitxor_int32(42, 0), 42, "bitxor_int32(42, 0) === 42");
	assert(fns.bitnot_int32(0), ~0, "bitnot_int32(0) === ~0");
	assert(fns.bitnot_int32(1), ~1, "bitnot_int32(1) === ~1");

	suite("bitwise shifts");
	assert(fns.shl_int32(1, 0), 1, "shl_int32(1, 0) === 1");
	assert(fns.shl_int32(1, 1), 2, "shl_int32(1, 1) === 2");
	assert(fns.shl_int32(1, 4), 16, "shl_int32(1, 4) === 16");
	assert(fns.shl_int32(3, 3), 24, "shl_int32(3, 3) === 24");
	assert(fns.shr_int32(16, 4), 1, "shr_int32(16, 4) === 1");
	assert(fns.shr_int32(24, 3), 3, "shr_int32(24, 3) === 3");
	assert(fns.shr_int32(255, 1), 127, "shr_int32(255, 1) === 127");

	// =====================================================================
	// Boolean / comparison
	// =====================================================================
	suite("comparison — eq/ne");
	assert(fns.eq_int32(5, 5), 1, "eq_int32(5, 5) === true");
	assert(fns.eq_int32(5, 6), 0, "eq_int32(5, 6) === false");
	assert(fns.eq_int32(0, 0), 1, "eq_int32(0, 0) === true");
	assert(fns.ne_int32(5, 6), 1, "ne_int32(5, 6) === true");
	assert(fns.ne_int32(5, 5), 0, "ne_int32(5, 5) === false");

	suite("comparison — lt/le/gt/ge");
	assert(fns.lt_int32(3, 5), 1, "lt_int32(3, 5) === true");
	assert(fns.lt_int32(5, 5), 0, "lt_int32(5, 5) === false");
	assert(fns.lt_int32(7, 5), 0, "lt_int32(7, 5) === false");
	assert(fns.le_int32(3, 5), 1, "le_int32(3, 5) === true");
	assert(fns.le_int32(5, 5), 1, "le_int32(5, 5) === true");
	assert(fns.le_int32(7, 5), 0, "le_int32(7, 5) === false");
	assert(fns.gt_int32(7, 5), 1, "gt_int32(7, 5) === true");
	assert(fns.gt_int32(5, 5), 0, "gt_int32(5, 5) === false");
	assert(fns.gt_int32(3, 5), 0, "gt_int32(3, 5) === false");
	assert(fns.ge_int32(7, 5), 1, "ge_int32(7, 5) === true");
	assert(fns.ge_int32(5, 5), 1, "ge_int32(5, 5) === true");
	assert(fns.ge_int32(3, 5), 0, "ge_int32(3, 5) === false");

	suite("comparison — negative numbers");
	assert(fns.lt_int32(-5, 0), 1, "lt_int32(-5, 0) === true");
	assert(fns.gt_int32(0, -5), 1, "gt_int32(0, -5) === true");
	assert(fns.le_int32(-5, -5), 1, "le_int32(-5, -5) === true");
	assert(fns.ge_int32(-5, -5), 1, "ge_int32(-5, -5) === true");
	assert(fns.lt_int32(-10, -5), 1, "lt_int32(-10, -5) === true");
	assert(fns.gt_int32(-5, -10), 1, "gt_int32(-5, -10) === true");

	suite("boolean logic");
	assert(fns.bool_and(1, 1), 1, "bool_and(true, true) === true");
	assert(fns.bool_and(1, 0), 0, "bool_and(true, false) === false");
	assert(fns.bool_and(0, 1), 0, "bool_and(false, true) === false");
	assert(fns.bool_and(0, 0), 0, "bool_and(false, false) === false");
	assert(fns.bool_or(1, 1), 1, "bool_or(true, true) === true");
	assert(fns.bool_or(1, 0), 1, "bool_or(true, false) === true");
	assert(fns.bool_or(0, 1), 1, "bool_or(false, true) === true");
	assert(fns.bool_or(0, 0), 0, "bool_or(false, false) === false");
	assert(fns.bool_not(1), 0, "bool_not(true) === false");
	assert(fns.bool_not(0), 1, "bool_not(false) === true");

	// =====================================================================
	// Fibonacci
	// =====================================================================
	suite("fib_int32");
	assert(fns.fib_int32(0), 0, "fib_int32(0) === 0");
	assert(fns.fib_int32(1), 1, "fib_int32(1) === 1");
	assert(fns.fib_int32(2), 1, "fib_int32(2) === 1");
	assert(fns.fib_int32(3), 2, "fib_int32(3) === 2");
	assert(fns.fib_int32(4), 3, "fib_int32(4) === 3");
	assert(fns.fib_int32(5), 5, "fib_int32(5) === 5");
	assert(fns.fib_int32(6), 8, "fib_int32(6) === 8");
	assert(fns.fib_int32(7), 13, "fib_int32(7) === 13");
	assert(fns.fib_int32(10), 55, "fib_int32(10) === 55");
	assert(fns.fib_int32(20), 6765, "fib_int32(20) === 6765");

	suite("fib_int64");
	assert(fns.fib_int64(0n), 0n, "fib_int64(0) === 0");
	assert(fns.fib_int64(1n), 1n, "fib_int64(1) === 1");
	assert(fns.fib_int64(10n), 55n, "fib_int64(10) === 55");
	assert(fns.fib_int64(20n), 6765n, "fib_int64(20) === 6765");
	assert(fns.fib_int64(50n), 12586269025n, "fib_int64(50) === 12586269025");

	// =====================================================================
	// Factorial
	// =====================================================================
	suite("factorial_int32");
	assert(fns.factorial_int32(0), 1, "factorial_int32(0) === 1");
	assert(fns.factorial_int32(1), 1, "factorial_int32(1) === 1");
	assert(fns.factorial_int32(2), 2, "factorial_int32(2) === 2");
	assert(fns.factorial_int32(3), 6, "factorial_int32(3) === 6");
	assert(fns.factorial_int32(4), 24, "factorial_int32(4) === 24");
	assert(fns.factorial_int32(5), 120, "factorial_int32(5) === 120");
	assert(fns.factorial_int32(10), 3628800, "factorial_int32(10) === 3628800");

	suite("factorial_int64");
	assert(fns.factorial_int64(0n), 1n, "factorial_int64(0) === 1");
	assert(fns.factorial_int64(1n), 1n, "factorial_int64(1) === 1");
	assert(fns.factorial_int64(5n), 120n, "factorial_int64(5) === 120");
	assert(fns.factorial_int64(10n), 3628800n, "factorial_int64(10) === 3628800");
	assert(
		fns.factorial_int64(20n),
		2432902008176640000n,
		"factorial_int64(20) === 2432902008176640000",
	);

	// =====================================================================
	// GCD
	// =====================================================================
	suite("gcd_int32");
	assert(fns.gcd_int32(12, 8), 4, "gcd_int32(12, 8) === 4");
	assert(fns.gcd_int32(8, 12), 4, "gcd_int32(8, 12) === 4 (commutative)");
	assert(fns.gcd_int32(7, 13), 1, "gcd_int32(7, 13) === 1 (coprime)");
	assert(fns.gcd_int32(100, 75), 25, "gcd_int32(100, 75) === 25");
	assert(fns.gcd_int32(0, 5), 5, "gcd_int32(0, 5) === 5");
	assert(fns.gcd_int32(5, 0), 5, "gcd_int32(5, 0) === 5");
	assert(fns.gcd_int32(7, 7), 7, "gcd_int32(7, 7) === 7");
	assert(fns.gcd_int32(1, 100), 1, "gcd_int32(1, 100) === 1");
	assert(fns.gcd_int32(-12, 8), 4, "gcd_int32(-12, 8) === 4 (negative input)");
	assert(fns.gcd_int32(12, -8), 4, "gcd_int32(12, -8) === 4 (negative input)");
	assert(fns.gcd_int32(-12, -8), 4, "gcd_int32(-12, -8) === 4 (both negative)");
	assert(fns.gcd_int32(48, 18), 6, "gcd_int32(48, 18) === 6");
	assert(
		fns.gcd_int32(1071, 462),
		21,
		"gcd_int32(1071, 462) === 21 (classic Euclid example)",
	);

	// =====================================================================
	// Identity / passthrough
	// =====================================================================
	suite("identity");
	assert(fns.identity_int32(0), 0, "identity_int32(0) === 0");
	assert(fns.identity_int32(42), 42, "identity_int32(42) === 42");
	assert(fns.identity_int32(-42), -42, "identity_int32(-42) === -42");
	assert(fns.identity_int64(0n), 0n, "identity_int64(0) === 0");
	assert(fns.identity_int64(999n), 999n, "identity_int64(999) === 999");
	assert(fns.identity_int64(-999n), -999n, "identity_int64(-999) === -999");
	assert(
		fns.identity_float32(3.14),
		Math.fround(3.14),
		"identity_float32(3.14)",
	);
	assert(fns.identity_float32(0.0), 0.0, "identity_float32(0) === 0");
	assert(fns.identity_float64(Math.PI), Math.PI, "identity_float64(pi)");
	assert(fns.identity_float64(0.0), 0.0, "identity_float64(0) === 0");
	assert(fns.identity_float64(-0.0), -0.0, "identity_float64(-0) === -0");

	// =====================================================================
	// Print
	// =====================================================================
	suite("print");
	Deno.stdout.writeSync(encoder.encode("    stdout: "));
	fns.print_static_string();
	Deno.stdout.writeSync(encoder.encode("    stdout: "));
	fns.print_int32();
	Deno.stdout.writeSync(encoder.encode("    stdout: "));
	fns.print_int64();
	Deno.stdout.writeSync(encoder.encode("    stdout: "));
	fns.print_float32();
	Deno.stdout.writeSync(encoder.encode("    stdout: "));
	fns.print_float64();
	passed += 5;
	console.log("    ✓ print functions executed without error");

	// =====================================================================
	// Print input string
	// =====================================================================
	suite("print_input_string");
	{
		const structPtr = writeStringStruct("print-input-string");
		Deno.stdout.writeSync(encoder.encode("    stdout: "));
		fns.print_input_string(structPtr);
		passed++;
		console.log("    ✓ print_input_string executed without error");
	}

	// =====================================================================
	// Return static string
	// =====================================================================
	suite("return_static_string");
	{
		const outPtr = allocStringStruct();
		fns.return_static_string(outPtr);
		const result = readStringStruct(outPtr);
		assert(
			result,
			"return-static-string",
			`return_static_string === "return-static-string"`,
		);
	}

	// =====================================================================
	// Return input string
	// =====================================================================
	suite("return_input_string");
	{
		const expectedString = "return-input-string";
		const inPtr = writeStringStruct(expectedString);
		const outPtr = allocStringStruct();
		fns.return_input_string(inPtr, outPtr);
		const result = readStringStruct(outPtr);
		assert(
			result,
			expectedString,
			`return_input_string === "${expectedString}"`,
		);
	}
	{
		const expectedString = "";
		const inPtr = writeStringStruct(expectedString);
		const outPtr = allocStringStruct();
		fns.return_input_string(inPtr, outPtr);
		const result = readStringStruct(outPtr);
		assert(
			result,
			expectedString,
			'return_input_string("") === "" (empty string)',
		);
	}
	{
		const expectedString = "a";
		const inPtr = writeStringStruct(expectedString);
		const outPtr = allocStringStruct();
		fns.return_input_string(inPtr, outPtr);
		const result = readStringStruct(outPtr);
		assert(
			result,
			expectedString,
			'return_input_string("a") === "a" (single char)',
		);
	}
	{
		const expectedString = "Hello, World! 🌍";
		const inPtr = writeStringStruct(expectedString);
		const outPtr = allocStringStruct();
		fns.return_input_string(inPtr, outPtr);
		const result = readStringStruct(outPtr);
		assert(result, expectedString, "return_input_string with emoji roundtrip");
	}

	// =====================================================================
	// String length
	// =====================================================================
	suite("string_length");
	{
		const ptr = writeStringStruct("hello");
		assert(fns.string_length(ptr), 5n, 'string_length("hello") === 5');
	}
	{
		const ptr = writeStringStruct("");
		assert(fns.string_length(ptr), 0n, 'string_length("") === 0');
	}
	{
		const ptr = writeStringStruct("a");
		assert(fns.string_length(ptr), 1n, 'string_length("a") === 1');
	}
	{
		const ptr = writeStringStruct("abcdefghij");
		assert(fns.string_length(ptr), 10n, 'string_length("abcdefghij") === 10');
	}
	{
		// UTF-8 multibyte: 🌍 is 4 bytes
		const ptr = writeStringStruct("🌍");
		assert(
			fns.string_length(ptr),
			4n,
			'string_length("🌍") === 4 (UTF-8 bytes)',
		);
	}

	// =====================================================================
	// String concatenation
	// =====================================================================
	suite("string_concat");
	{
		const aPtr = writeStringStruct("hello");
		const bPtr = writeStringStruct(" world");
		const outPtr = allocStringStruct();
		fns.string_concat(aPtr, bPtr, outPtr);
		assert(
			readStringStruct(outPtr),
			"hello world",
			'string_concat("hello", " world") === "hello world"',
		);
	}
	{
		const aPtr = writeStringStruct("");
		const bPtr = writeStringStruct("world");
		const outPtr = allocStringStruct();
		fns.string_concat(aPtr, bPtr, outPtr);
		assert(
			readStringStruct(outPtr),
			"world",
			'string_concat("", "world") === "world"',
		);
	}
	{
		const aPtr = writeStringStruct("hello");
		const bPtr = writeStringStruct("");
		const outPtr = allocStringStruct();
		fns.string_concat(aPtr, bPtr, outPtr);
		assert(
			readStringStruct(outPtr),
			"hello",
			'string_concat("hello", "") === "hello"',
		);
	}
	{
		const aPtr = writeStringStruct("");
		const bPtr = writeStringStruct("");
		const outPtr = allocStringStruct();
		fns.string_concat(aPtr, bPtr, outPtr);
		assert(readStringStruct(outPtr), "", 'string_concat("", "") === ""');
	}
	{
		const aPtr = writeStringStruct("foo");
		const bPtr = writeStringStruct("bar");
		const outPtr = allocStringStruct();
		fns.string_concat(aPtr, bPtr, outPtr);
		assert(
			readStringStruct(outPtr),
			"foobar",
			'string_concat("foo", "bar") === "foobar"',
		);
	}

	// =====================================================================
	// String repeat
	// =====================================================================
	suite("string_repeat");
	{
		const ptr = writeStringStruct("ab");
		const outPtr = allocStringStruct();
		fns.string_repeat(ptr, 3, outPtr);
		assert(
			readStringStruct(outPtr),
			"ababab",
			'string_repeat("ab", 3) === "ababab"',
		);
	}
	{
		const ptr = writeStringStruct("x");
		const outPtr = allocStringStruct();
		fns.string_repeat(ptr, 1, outPtr);
		assert(readStringStruct(outPtr), "x", 'string_repeat("x", 1) === "x"');
	}
	{
		const ptr = writeStringStruct("abc");
		const outPtr = allocStringStruct();
		fns.string_repeat(ptr, 0, outPtr);
		assert(readStringStruct(outPtr), "", 'string_repeat("abc", 0) === ""');
	}
	{
		const ptr = writeStringStruct("ha");
		const outPtr = allocStringStruct();
		fns.string_repeat(ptr, 5, outPtr);
		assert(
			readStringStruct(outPtr),
			"hahahahaha",
			'string_repeat("ha", 5) === "hahahahaha"',
		);
	}

	// =====================================================================
	// String equality
	// =====================================================================
	suite("string_eq");
	{
		const aPtr = writeStringStruct("hello");
		const bPtr = writeStringStruct("hello");
		assert(
			fns.string_eq(aPtr, bPtr),
			1,
			'string_eq("hello", "hello") === true',
		);
	}
	{
		const aPtr = writeStringStruct("hello");
		const bPtr = writeStringStruct("world");
		assert(
			fns.string_eq(aPtr, bPtr),
			0,
			'string_eq("hello", "world") === false',
		);
	}
	{
		const aPtr = writeStringStruct("");
		const bPtr = writeStringStruct("");
		assert(fns.string_eq(aPtr, bPtr), 1, 'string_eq("", "") === true');
	}
	{
		const aPtr = writeStringStruct("hello");
		const bPtr = writeStringStruct("hell");
		assert(
			fns.string_eq(aPtr, bPtr),
			0,
			'string_eq("hello", "hell") === false (prefix)',
		);
	}
	{
		const aPtr = writeStringStruct("abc");
		const bPtr = writeStringStruct("ABC");
		assert(
			fns.string_eq(aPtr, bPtr),
			0,
			'string_eq("abc", "ABC") === false (case sensitive)',
		);
	}

	// =====================================================================
	// Cross-function consistency checks
	// =====================================================================
	suite("cross-function consistency");
	{
		// add and sub are inverses
		const x = 17;
		const y = 9;
		const sum = fns.add_int32(x, y);
		assert(
			fns.sub_int32(sum, y),
			x,
			"sub(add(x, y), y) === x (add/sub inverse)",
		);
	}
	{
		// mul and div are inverses for exact division
		const x = 6;
		const y = 3;
		const product = fns.mul_int32(x, y);
		assert(
			fns.div_int32(product, y),
			x,
			"div(mul(x, y), y) === x (mul/div inverse)",
		);
	}
	// neg(neg(x)) === x
	assert(fns.neg_int32(fns.neg_int32(42)), 42, "neg(neg(42)) === 42");
	// abs(neg(x)) === abs(x) for positive x
	assert(
		fns.abs_int32(fns.neg_int32(7)),
		fns.abs_int32(7),
		"abs(neg(7)) === abs(7)",
	);
	{
		// min(x, y) <= max(x, y)
		const a = 3;
		const b = 7;
		const lo = fns.min_int32(a, b);
		const hi = fns.max_int32(a, b);
		assert(fns.le_int32(lo, hi), 1, "min(x,y) <= max(x,y)");
	}
	{
		// x & y | x ^ y === x | y  (bitwise identity)
		const x = 0b1100;
		const y = 0b1010;
		assert(
			fns.bitor_int32(fns.bitand_int32(x, y), fns.bitxor_int32(x, y)),
			fns.bitor_int32(x, y),
			"(x & y) | (x ^ y) === x | y",
		);
	}
	{
		// shl then shr roundtrip
		const x = 5;
		assert(fns.shr_int32(fns.shl_int32(x, 4), 4), x, "shr(shl(x, 4), 4) === x");
	}
	{
		// De Morgan's law: not(and(a,b)) === or(not(a), not(b))
		const a = 1;
		const b = 0;
		assert(
			fns.bool_not(fns.bool_and(a, b)),
			fns.bool_or(fns.bool_not(a), fns.bool_not(b)),
			"De Morgan: not(and(a,b)) === or(not(a), not(b))",
		);
	}
	{
		// gcd(a*k, b*k) === k * gcd(a, b)
		const a = 6;
		const b = 4;
		const k = 5;
		assert(
			fns.gcd_int32(fns.mul_int32(a, k), fns.mul_int32(b, k)),
			fns.mul_int32(k, fns.gcd_int32(a, b)),
			"gcd(a*k, b*k) === k * gcd(a, b)",
		);
	}
	// Fibonacci property: fib(n) = fib(n-1) + fib(n-2)
	for (const n of [5, 8, 12, 15]) {
		const fn2 = fns.fib_int32(n - 2);
		const fn1 = fns.fib_int32(n - 1);
		const fn0 = fns.fib_int32(n);
		assert(fn0, fn1 + fn2, `fib(${n}) === fib(${n - 1}) + fib(${n - 2})`);
	}
	// Factorial property: n! === n * (n-1)!
	for (const n of [2, 3, 4, 5, 6, 7]) {
		assert(
			fns.factorial_int32(n),
			n * fns.factorial_int32(n - 1),
			`factorial(${n}) === ${n} * factorial(${n - 1})`,
		);
	}
	{
		// string_length(concat(a, b)) === string_length(a) + string_length(b)
		const aPtr = writeStringStruct("foo");
		const bPtr = writeStringStruct("barbaz");
		const outPtr = allocStringStruct();
		fns.string_concat(aPtr, bPtr, outPtr);
		assert(
			fns.string_length(outPtr),
			fns.string_length(aPtr) + fns.string_length(bPtr),
			"len(concat(a,b)) === len(a) + len(b)",
		);
	}
	// clamp(x, lo, hi) === max(lo, min(hi, x))
	for (const x of [-5, 0, 5, 10, 15]) {
		const lo = 0;
		const hi = 10;
		assert(
			fns.clamp_int32(x, lo, hi),
			fns.max_int32(lo, fns.min_int32(hi, x)),
			`clamp(${x}, ${lo}, ${hi}) === max(lo, min(hi, x))`,
		);
	}

	// --- Summary ---
	console.log(
		`\n  ${passed + failed} tests: ${passed} passed, ${failed} failed\n`,
	);
	Deno.exit(failed > 0 ? 1 : 0);
}

run().catch((err: unknown) => {
	console.error("Fatal error:", err);
	Deno.exit(2);
});

const encoder = new TextEncoder();
const decoder = new TextDecoder();

// --- Types for WASM module exports ---

interface WasmExports extends WebAssembly.Exports {
	memory: WebAssembly.Memory;
	__heap_base: WebAssembly.Global;
	__heap_end: WebAssembly.Global;
	add_int32(x: number, y: number): number;
	add_int64(x: bigint, y: bigint): bigint;
	add_float32(x: number, y: number): number;
	add_float64(x: number, y: number): number;
	pow_int32(x: number): number;
	pow_int64(x: bigint): bigint;
	pow_float32(x: number): number;
	pow_float64(x: number): number;
	print_static_string(): void;
	print_int32(): void;
	print_int64(): void;
	print_float32(): void;
	print_float64(): void;
	print_input_string(structPtr: bigint): void;
	return_static_string(outStructPtr: bigint): void;
	return_input_string(inStructPtr: bigint, outStructPtr: bigint): void;
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

/** Read a JS string from a Mojo String struct in WASM memory. */
const readStringStruct = (structPtr: bigint): string => {
	const view = getView();
	const dataPtr = view.getBigInt64(Number(structPtr), true);
	const len = view.getBigInt64(Number(structPtr) + 8, true);

	if (len <= 0n) return "";

	// len is the string byte count (excludes null terminator)
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

// --- Main ---

async function run(): Promise<void> {
	const wasmBuffer = await Deno.readFile(
		new URL("build/out.wasm", import.meta.url),
	);
	const { instance } = await WebAssembly.instantiate(wasmBuffer, { env });
	initializeEnv(instance);

	const fns = wasmExports;

	console.log("wasm-mojo tests\n");

	// --- Add ---
	suite("add");
	assert(fns.add_int32(2, 3), 5, "add_int32(2, 3) === 5");
	assert(fns.add_int64(2n, 3n), 5n, "add_int64(2, 3) === 5");
	assert(
		fns.add_float32(2.2, 3.3),
		Math.fround(2.2) + Math.fround(3.3),
		"add_float32(2.2, 3.3)",
	);
	assert(fns.add_float64(2.2, 3.3), 2.2 + 3.3, "add_float64(2.2, 3.3)");

	// --- Power ---
	suite("pow");
	assert(fns.pow_int32(3), 27, "pow_int32(3) === 27");
	assert(fns.pow_int64(3n), 27n, "pow_int64(3) === 27");
	assert(
		String(fns.pow_float32(3.3)),
		String(fns.pow_float32(3.3)),
		"pow_float32(3.3) is stable",
	);
	assert(String(fns.pow_float64(3.3)), "51.41572944937184", "pow_float64(3.3)");

	// --- Print ---
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

	// --- Print input string ---
	suite("print_input_string");
	{
		const structPtr = writeStringStruct("print-input-string");
		Deno.stdout.writeSync(encoder.encode("    stdout: "));
		fns.print_input_string(structPtr);
		passed++;
		console.log("    ✓ print_input_string executed without error");
	}

	// --- Return static string ---
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

	// --- Return input string ---
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

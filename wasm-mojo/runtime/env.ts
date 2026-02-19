import { alignedAlloc, alignedFree } from "./memory.ts";

const encoder = new TextEncoder();
const decoder = new TextDecoder();

// --- I/O ---

let memory: WebAssembly.Memory | null = null;

/** Set the memory reference used by the write stub. Called after instantiation. */
export const setMemory = (mem: WebAssembly.Memory): void => {
	memory = mem;
};

const write = (fd: bigint, ptr: bigint, len: bigint): number => {
	if (len === 0n) return 0;

	try {
		if (!memory) {
			console.error("write called before memory initialized");
			return -1;
		}

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

// --- Compiler runtime stubs ---

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

// --- Env object ---

/** WebAssembly import object providing the environment the Mojo WASM module expects. */
export const env: WebAssembly.ModuleImports = {
	memory: new WebAssembly.Memory({ initial: 2 }),

	// libc / runtime stubs
	__cxa_atexit: (_func: bigint, _obj: bigint, _dso: bigint): number => 0,
	KGEN_CompilerRT_AlignedAlloc: alignedAlloc,
	KGEN_CompilerRT_AlignedFree: alignedFree,
	KGEN_CompilerRT_GetStackTrace,
	KGEN_CompilerRT_fprintf,
	write,
	free: alignedFree,
	dup: (_fd: bigint): number => 1,
	fdopen: (_fd: bigint, _modePtr: bigint): number => 1,
	fflush: (_stream: bigint): number => 1,
	fclose: (_stream: bigint): number => 1,

	// math builtins
	fmaf: (x: number, y: number, z: number): number =>
		Math.fround(Math.fround(x * y) + z),
	fminf: (x: number, y: number): number => (x > y ? y : x),
	fmaxf: (x: number, y: number): number => (x > y ? x : y),
	fma: (x: number, y: number, z: number): number => x * y + z,
	fmin: (x: number, y: number): number => (x > y ? y : x),
	fmax: (x: number, y: number): number => (x > y ? x : y),
};

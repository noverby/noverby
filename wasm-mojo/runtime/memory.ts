import type { WasmExports } from "./types.ts";

// --- WASM runtime state ---

let heapPointer: bigint = 0n;
let wasmExports: WasmExports | null = null;
let memory: WebAssembly.Memory | null = null;

/** Initialize runtime state from a WASM instance. */
export const initialize = (instance: WebAssembly.Instance): void => {
	wasmExports = instance.exports as unknown as WasmExports;
	memory = wasmExports.memory;
	heapPointer = wasmExports.__heap_base.value as bigint;
};

/** Get the current WASM exports (throws if not initialized). */
export const getExports = (): WasmExports => {
	if (!wasmExports) throw new Error("WASM runtime not initialized");
	return wasmExports;
};

/** Get the current WASM memory (throws if not initialized). */
export const getMemory = (): WebAssembly.Memory => {
	if (!memory) throw new Error("WASM runtime not initialized");
	return memory;
};

/** Get a DataView over the current WASM memory buffer. */
export const getView = (): DataView => new DataView(getMemory().buffer);

// --- Bump allocator ---

/**
 * Allocate `size` bytes with the given alignment from the WASM heap.
 * This is a simple bump allocator — memory is never reclaimed.
 */
export const alignedAlloc = (align: bigint, size: bigint): bigint => {
	const remainder = heapPointer % align;
	if (remainder !== 0n) {
		heapPointer += align - remainder;
	}
	const ptr = heapPointer;
	heapPointer += size;
	return ptr;
};

/** No-op free (bump allocator never reclaims). */
export const alignedFree = (_ptr: bigint): number => {
	return 1;
};

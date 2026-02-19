export { env, setMemory } from "./env.ts";
export {
	alignedAlloc,
	alignedFree,
	getExports,
	getMemory,
	getView,
	initialize,
} from "./memory.ts";
export type { Mutation } from "./protocol.ts";
export { MutationReader, Op } from "./protocol.ts";
export {
	allocStringStruct,
	readStringStruct,
	writeStringStruct,
} from "./strings.ts";
export type { WasmExports } from "./types.ts";

import { env, setMemory } from "./env.ts";
import { getExports, initialize } from "./memory.ts";
import type { WasmExports } from "./types.ts";

/**
 * Load and instantiate the Mojo WASM binary, returning the typed exports.
 *
 * @param wasmPath - URL or file path to the `.wasm` binary.
 */
export async function instantiate(
	wasmPath: string | URL,
): Promise<WasmExports> {
	const wasmBuffer = await Deno.readFile(wasmPath);
	const { instance } = await WebAssembly.instantiate(wasmBuffer, { env });
	initialize(instance);
	const exports = getExports();
	setMemory(exports.memory);
	return exports;
}

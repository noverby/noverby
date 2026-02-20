export type { CounterAppHandle } from "./app.ts";
export { createCounterApp } from "./app.ts";
export { env, setMemory } from "./env.ts";
export type {
	DispatchFn,
	DispatchWithValueFn,
	EventTypeName,
} from "./events.ts";
export { EventBridge, EventType } from "./events.ts";
export { Interpreter, MutationBuilder } from "./interpreter.ts";
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
export type { TagId } from "./tags.ts";
export { TAG_COUNT, Tag, tagName } from "./tags.ts";
export type { WasmTemplateExports } from "./templates.ts";
export { TemplateCache } from "./templates.ts";
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

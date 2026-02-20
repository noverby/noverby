import { instantiate } from "../runtime/mod.ts";
import { testCounter } from "./counter.test.ts";
import { summary } from "./harness.ts";
import { testInterpreter } from "./interpreter.test.ts";
import { testMutations } from "./mutations.test.ts";
import { testPhase8 } from "./phase8.test.ts";
import { testProtocol } from "./protocol.test.ts";
import { testTodo } from "./todo.test.ts";

async function run(): Promise<void> {
	const wasmPath = new URL("../build/out.wasm", import.meta.url);
	const fns = await instantiate(wasmPath);

	console.log("wasm-mojo JS runtime tests\n");

	testProtocol(fns);
	testMutations(fns);
	testInterpreter(fns);
	testCounter(fns);
	testTodo(fns);
	testPhase8(fns);

	summary();
}

run().catch((err: unknown) => {
	console.error("Fatal error:", err);
	Deno.exit(2);
});

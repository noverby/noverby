import { instantiate } from "../runtime/mod.ts";
import { testCounter } from "./counter.test.ts";
import { testEvents } from "./events.test.ts";
import { summary } from "./harness.ts";
import { testInterpreter } from "./interpreter.test.ts";
import { testMutations } from "./mutations.test.ts";
import { testProtocol } from "./protocol.test.ts";
import { testTemplates } from "./templates.test.ts";

async function run(): Promise<void> {
	const wasmPath = new URL("../build/out.wasm", import.meta.url);
	const fns = await instantiate(wasmPath);

	console.log("wasm-mojo JS runtime tests\n");

	testProtocol(fns);
	testTemplates(fns);
	testMutations(fns);
	testInterpreter(fns);
	testEvents(fns);
	testCounter(fns);

	summary();
}

run().catch((err: unknown) => {
	console.error("Fatal error:", err);
	Deno.exit(2);
});

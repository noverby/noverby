import { instantiate } from "../runtime/mod.ts";
import { testAlgorithms } from "./algorithms.test.ts";
import { testArithmetic } from "./arithmetic.test.ts";
import { testBitwise } from "./bitwise.test.ts";
import { testBoundaries } from "./boundaries.test.ts";
import { testComparison } from "./comparison.test.ts";
import { testConsistency } from "./consistency.test.ts";
import { testFloats } from "./floats.test.ts";
import { summary } from "./harness.ts";
import { testIdentity } from "./identity.test.ts";
import { testMinMax } from "./minmax.test.ts";
import { testPrint } from "./print.test.ts";
import { testProperties } from "./properties.test.ts";
import { testSSO } from "./sso.test.ts";
import { testStress } from "./stress.test.ts";
import { testStrings } from "./strings.test.ts";
import { testUnary } from "./unary.test.ts";
import { testUnicode } from "./unicode.test.ts";

async function run(): Promise<void> {
	const wasmPath = new URL("../build/out.wasm", import.meta.url);
	const fns = await instantiate(wasmPath);

	console.log("wasm-mojo tests\n");

	testArithmetic(fns);
	testUnary(fns);
	testMinMax(fns);
	testBitwise(fns);
	testComparison(fns);
	testAlgorithms(fns);
	testIdentity(fns);
	testPrint(fns);
	testStrings(fns);
	testConsistency(fns);
	testBoundaries(fns);
	testFloats(fns);
	testSSO(fns);
	testUnicode(fns);
	testProperties(fns);
	testStress(fns);

	summary();
}

run().catch((err: unknown) => {
	console.error("Fatal error:", err);
	Deno.exit(2);
});

import type { WasmExports } from "../runtime/mod.ts";
import { writeStringStruct } from "../runtime/mod.ts";
import { pass, suite, writeStdout } from "./harness.ts";

export function testPrint(fns: WasmExports): void {
	// =================================================================
	// Print (static values)
	// =================================================================
	suite("print");
	writeStdout("    stdout: ");
	fns.print_static_string();
	writeStdout("    stdout: ");
	fns.print_int32();
	writeStdout("    stdout: ");
	fns.print_int64();
	writeStdout("    stdout: ");
	fns.print_float32();
	writeStdout("    stdout: ");
	fns.print_float64();
	pass(5);
	console.log("    ✓ print functions executed without error");

	// =================================================================
	// Print input string
	// =================================================================
	suite("print_input_string");
	{
		const structPtr = writeStringStruct("print-input-string");
		writeStdout("    stdout: ");
		fns.print_input_string(structPtr);
		pass();
		console.log("    ✓ print_input_string executed without error");
	}
}

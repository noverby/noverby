import type { WasmExports } from "../runtime/mod.ts";
import { assert, assertClose, suite } from "./harness.ts";

export function testArithmetic(fns: WasmExports): void {
	// =================================================================
	// Add
	// =================================================================
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

	// =================================================================
	// Subtract
	// =================================================================
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

	// =================================================================
	// Multiply
	// =================================================================
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

	// =================================================================
	// Division
	// =================================================================
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

	// =================================================================
	// Modulo
	// =================================================================
	suite("mod");
	assert(fns.mod_int32(10, 3), 1, "mod_int32(10, 3) === 1");
	assert(fns.mod_int64(10n, 3n), 1n, "mod_int64(10, 3) === 1");
	assert(fns.mod_int32(15, 5), 0, "mod_int32(15, 5) === 0");
	assert(fns.mod_int32(0, 7), 0, "mod_int32(0, 7) === 0");
	assert(fns.mod_int32(7, 1), 0, "mod_int32(7, 1) === 0");
	assert(fns.mod_int64(100n, 7n), 2n, "mod_int64(100, 7) === 2");

	// =================================================================
	// Power
	// =================================================================
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
}

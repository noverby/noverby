import type { WasmExports } from "../runtime/mod.ts";
import { assert, suite } from "./harness.ts";

export function testUnary(fns: WasmExports): void {
	// =================================================================
	// Negate
	// =================================================================
	suite("neg");
	assert(fns.neg_int32(5), -5, "neg_int32(5) === -5");
	assert(fns.neg_int32(-5), 5, "neg_int32(-5) === 5");
	assert(fns.neg_int32(0), 0, "neg_int32(0) === 0");
	assert(fns.neg_int64(42n), -42n, "neg_int64(42) === -42");
	assert(fns.neg_int64(-42n), 42n, "neg_int64(-42) === 42");
	assert(fns.neg_int64(0n), 0n, "neg_int64(0) === 0");
	assert(fns.neg_float32(3.14), -Math.fround(3.14), "neg_float32(3.14)");
	assert(fns.neg_float64(3.14), -3.14, "neg_float64(3.14) === -3.14");
	assert(fns.neg_float64(-3.14), 3.14, "neg_float64(-3.14) === 3.14");
	assert(fns.neg_float64(0.0), -0.0, "neg_float64(0) === -0");

	// =================================================================
	// Absolute value
	// =================================================================
	suite("abs");
	assert(fns.abs_int32(5), 5, "abs_int32(5) === 5");
	assert(fns.abs_int32(-5), 5, "abs_int32(-5) === 5");
	assert(fns.abs_int32(0), 0, "abs_int32(0) === 0");
	assert(fns.abs_int64(99n), 99n, "abs_int64(99) === 99");
	assert(fns.abs_int64(-99n), 99n, "abs_int64(-99) === 99");
	assert(fns.abs_int64(0n), 0n, "abs_int64(0) === 0");
	assert(fns.abs_float32(2.5), Math.fround(2.5), "abs_float32(2.5)");
	assert(fns.abs_float32(-2.5), Math.fround(2.5), "abs_float32(-2.5)");
	assert(fns.abs_float64(3.14), 3.14, "abs_float64(3.14) === 3.14");
	assert(fns.abs_float64(-3.14), 3.14, "abs_float64(-3.14) === 3.14");
	assert(fns.abs_float64(0.0), 0.0, "abs_float64(0) === 0");
}

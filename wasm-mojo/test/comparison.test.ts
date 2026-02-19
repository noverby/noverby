import type { WasmExports } from "../runtime/mod.ts";
import { assert, suite } from "./harness.ts";

export function testComparison(fns: WasmExports): void {
	// =================================================================
	// Comparison — eq / ne
	// =================================================================
	suite("comparison — eq/ne");
	assert(fns.eq_int32(5, 5), 1, "eq_int32(5, 5) === true");
	assert(fns.eq_int32(5, 6), 0, "eq_int32(5, 6) === false");
	assert(fns.eq_int32(0, 0), 1, "eq_int32(0, 0) === true");
	assert(fns.ne_int32(5, 6), 1, "ne_int32(5, 6) === true");
	assert(fns.ne_int32(5, 5), 0, "ne_int32(5, 5) === false");

	// =================================================================
	// Comparison — lt / le / gt / ge
	// =================================================================
	suite("comparison — lt/le/gt/ge");
	assert(fns.lt_int32(3, 5), 1, "lt_int32(3, 5) === true");
	assert(fns.lt_int32(5, 5), 0, "lt_int32(5, 5) === false");
	assert(fns.lt_int32(7, 5), 0, "lt_int32(7, 5) === false");
	assert(fns.le_int32(3, 5), 1, "le_int32(3, 5) === true");
	assert(fns.le_int32(5, 5), 1, "le_int32(5, 5) === true");
	assert(fns.le_int32(7, 5), 0, "le_int32(7, 5) === false");
	assert(fns.gt_int32(7, 5), 1, "gt_int32(7, 5) === true");
	assert(fns.gt_int32(5, 5), 0, "gt_int32(5, 5) === false");
	assert(fns.gt_int32(3, 5), 0, "gt_int32(3, 5) === false");
	assert(fns.ge_int32(7, 5), 1, "ge_int32(7, 5) === true");
	assert(fns.ge_int32(5, 5), 1, "ge_int32(5, 5) === true");
	assert(fns.ge_int32(3, 5), 0, "ge_int32(3, 5) === false");

	// =================================================================
	// Comparison — negative numbers
	// =================================================================
	suite("comparison — negative numbers");
	assert(fns.lt_int32(-5, 0), 1, "lt_int32(-5, 0) === true");
	assert(fns.gt_int32(0, -5), 1, "gt_int32(0, -5) === true");
	assert(fns.le_int32(-5, -5), 1, "le_int32(-5, -5) === true");
	assert(fns.ge_int32(-5, -5), 1, "ge_int32(-5, -5) === true");
	assert(fns.lt_int32(-10, -5), 1, "lt_int32(-10, -5) === true");
	assert(fns.gt_int32(-5, -10), 1, "gt_int32(-5, -10) === true");

	// =================================================================
	// Boolean logic
	// =================================================================
	suite("boolean logic");
	assert(fns.bool_and(1, 1), 1, "bool_and(true, true) === true");
	assert(fns.bool_and(1, 0), 0, "bool_and(true, false) === false");
	assert(fns.bool_and(0, 1), 0, "bool_and(false, true) === false");
	assert(fns.bool_and(0, 0), 0, "bool_and(false, false) === false");
	assert(fns.bool_or(1, 1), 1, "bool_or(true, true) === true");
	assert(fns.bool_or(1, 0), 1, "bool_or(true, false) === true");
	assert(fns.bool_or(0, 1), 1, "bool_or(false, true) === true");
	assert(fns.bool_or(0, 0), 0, "bool_or(false, false) === false");
	assert(fns.bool_not(1), 0, "bool_not(true) === false");
	assert(fns.bool_not(0), 1, "bool_not(false) === true");
}

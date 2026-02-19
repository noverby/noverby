import type { WasmExports } from "../runtime/mod.ts";
import { assert, suite } from "./harness.ts";

export function testMinMax(fns: WasmExports): void {
	// =================================================================
	// Min / Max — int32
	// =================================================================
	suite("min/max int32");
	assert(fns.min_int32(3, 7), 3, "min_int32(3, 7) === 3");
	assert(fns.min_int32(7, 3), 3, "min_int32(7, 3) === 3");
	assert(fns.min_int32(5, 5), 5, "min_int32(5, 5) === 5");
	assert(fns.min_int32(-3, 3), -3, "min_int32(-3, 3) === -3");
	assert(fns.max_int32(3, 7), 7, "max_int32(3, 7) === 7");
	assert(fns.max_int32(7, 3), 7, "max_int32(7, 3) === 7");
	assert(fns.max_int32(5, 5), 5, "max_int32(5, 5) === 5");
	assert(fns.max_int32(-3, 3), 3, "max_int32(-3, 3) === 3");

	// =================================================================
	// Min / Max — int64
	// =================================================================
	suite("min/max int64");
	assert(fns.min_int64(3n, 7n), 3n, "min_int64(3, 7) === 3");
	assert(fns.min_int64(7n, 3n), 3n, "min_int64(7, 3) === 3");
	assert(fns.min_int64(-10n, 10n), -10n, "min_int64(-10, 10) === -10");
	assert(fns.max_int64(3n, 7n), 7n, "max_int64(3, 7) === 7");
	assert(fns.max_int64(7n, 3n), 7n, "max_int64(7, 3) === 7");
	assert(fns.max_int64(-10n, 10n), 10n, "max_int64(-10, 10) === 10");

	// =================================================================
	// Min / Max — float64
	// =================================================================
	suite("min/max float64");
	assert(fns.min_float64(1.1, 2.2), 1.1, "min_float64(1.1, 2.2) === 1.1");
	assert(fns.min_float64(2.2, 1.1), 1.1, "min_float64(2.2, 1.1) === 1.1");
	assert(fns.min_float64(-0.5, 0.5), -0.5, "min_float64(-0.5, 0.5) === -0.5");
	assert(fns.max_float64(1.1, 2.2), 2.2, "max_float64(1.1, 2.2) === 2.2");
	assert(fns.max_float64(2.2, 1.1), 2.2, "max_float64(2.2, 1.1) === 2.2");
	assert(fns.max_float64(-0.5, 0.5), 0.5, "max_float64(-0.5, 0.5) === 0.5");

	// =================================================================
	// Clamp
	// =================================================================
	suite("clamp");
	assert(
		fns.clamp_int32(5, 0, 10),
		5,
		"clamp_int32(5, 0, 10) === 5 (within range)",
	);
	assert(fns.clamp_int32(-5, 0, 10), 0, "clamp_int32(-5, 0, 10) === 0 (below)");
	assert(
		fns.clamp_int32(15, 0, 10),
		10,
		"clamp_int32(15, 0, 10) === 10 (above)",
	);
	assert(
		fns.clamp_int32(0, 0, 10),
		0,
		"clamp_int32(0, 0, 10) === 0 (at low bound)",
	);
	assert(
		fns.clamp_int32(10, 0, 10),
		10,
		"clamp_int32(10, 0, 10) === 10 (at high bound)",
	);
	assert(
		fns.clamp_float64(5.5, 0.0, 10.0),
		5.5,
		"clamp_float64(5.5, 0, 10) === 5.5",
	);
	assert(
		fns.clamp_float64(-1.0, 0.0, 10.0),
		0.0,
		"clamp_float64(-1, 0, 10) === 0",
	);
	assert(
		fns.clamp_float64(11.0, 0.0, 10.0),
		10.0,
		"clamp_float64(11, 0, 10) === 10",
	);
}

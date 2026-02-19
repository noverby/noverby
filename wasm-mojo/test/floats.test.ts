import type { WasmExports } from "../runtime/mod.ts";
import { assert, assertNaN, suite } from "./harness.ts";

export function testFloats(fns: WasmExports): void {
	// =================================================================
	// NaN propagation — arithmetic
	// =================================================================
	suite("float NaN — arithmetic");
	assertNaN(fns.add_float64(NaN, 1.0), "add_float64(NaN, 1.0) === NaN");
	assertNaN(fns.add_float64(1.0, NaN), "add_float64(1.0, NaN) === NaN");
	assertNaN(fns.add_float64(NaN, NaN), "add_float64(NaN, NaN) === NaN");
	assertNaN(fns.sub_float64(NaN, 1.0), "sub_float64(NaN, 1.0) === NaN");
	assertNaN(fns.sub_float64(1.0, NaN), "sub_float64(1.0, NaN) === NaN");
	assertNaN(fns.mul_float64(NaN, 2.0), "mul_float64(NaN, 2.0) === NaN");
	assertNaN(fns.mul_float64(2.0, NaN), "mul_float64(2.0, NaN) === NaN");
	assertNaN(fns.div_float64(NaN, 2.0), "div_float64(NaN, 2.0) === NaN");
	assertNaN(fns.div_float64(2.0, NaN), "div_float64(2.0, NaN) === NaN");

	// =================================================================
	// NaN propagation — float32
	// =================================================================
	suite("float NaN — float32 arithmetic");
	assertNaN(fns.add_float32(NaN, 1.0), "add_float32(NaN, 1.0) === NaN");
	assertNaN(fns.sub_float32(NaN, 1.0), "sub_float32(NaN, 1.0) === NaN");
	assertNaN(fns.mul_float32(NaN, 2.0), "mul_float32(NaN, 2.0) === NaN");
	assertNaN(fns.div_float32(NaN, 2.0), "div_float32(NaN, 2.0) === NaN");

	// =================================================================
	// NaN-producing operations
	// =================================================================
	suite("float NaN — producing operations");
	assertNaN(
		fns.add_float64(Infinity, -Infinity),
		"add_float64(Inf, -Inf) === NaN",
	);
	assertNaN(
		fns.sub_float64(Infinity, Infinity),
		"sub_float64(Inf, Inf) === NaN",
	);
	assertNaN(fns.mul_float64(0.0, Infinity), "mul_float64(0, Inf) === NaN");
	assertNaN(fns.div_float64(0.0, 0.0), "div_float64(0, 0) === NaN");

	// =================================================================
	// NaN propagation — unary
	// =================================================================
	suite("float NaN — unary");
	assertNaN(fns.neg_float64(NaN), "neg_float64(NaN) === NaN");
	assertNaN(fns.neg_float32(NaN), "neg_float32(NaN) === NaN");
	// Mojo abs uses `if x < 0` — NaN < 0 is false, so returns NaN unchanged
	assertNaN(fns.abs_float64(NaN), "abs_float64(NaN) === NaN");
	assertNaN(fns.abs_float32(NaN), "abs_float32(NaN) === NaN");

	// =================================================================
	// NaN propagation — identity
	// =================================================================
	suite("float NaN — identity");
	assertNaN(fns.identity_float64(NaN), "identity_float64(NaN) === NaN");
	assertNaN(fns.identity_float32(NaN), "identity_float32(NaN) === NaN");

	// =================================================================
	// NaN — min/max (quirky: Mojo uses if x < y / if x > y)
	// NaN comparisons always return false, so the "else" branch wins
	// =================================================================
	suite("float NaN — min/max (comparison quirk)");
	// min: `if x < y: return x; return y` — NaN < 5 is false → returns y
	assert(
		fns.min_float64(NaN, 5.0),
		5.0,
		"min_float64(NaN, 5.0) === 5.0 (NaN < 5 is false, returns y)",
	);
	// min: 5 < NaN is false → returns y (NaN)
	assertNaN(
		fns.min_float64(5.0, NaN),
		"min_float64(5.0, NaN) === NaN (5 < NaN is false, returns y)",
	);
	// max: `if x > y: return x; return y` — NaN > 5 is false → returns y
	assert(
		fns.max_float64(NaN, 5.0),
		5.0,
		"max_float64(NaN, 5.0) === 5.0 (NaN > 5 is false, returns y)",
	);
	// max: 5 > NaN is false → returns y (NaN)
	assertNaN(
		fns.max_float64(5.0, NaN),
		"max_float64(5.0, NaN) === NaN (5 > NaN is false, returns y)",
	);

	// =================================================================
	// NaN — power
	// =================================================================
	suite("float NaN — pow");
	assertNaN(fns.pow_float64(NaN), "pow_float64(NaN) === NaN");
	assertNaN(fns.pow_float32(NaN), "pow_float32(NaN) === NaN");

	// =================================================================
	// Infinity — arithmetic
	// =================================================================
	suite("float Infinity — arithmetic");
	assert(
		fns.add_float64(Infinity, 1.0),
		Infinity,
		"add_float64(Inf, 1) === Inf",
	);
	assert(
		fns.add_float64(-Infinity, -1.0),
		-Infinity,
		"add_float64(-Inf, -1) === -Inf",
	);
	assert(
		fns.sub_float64(Infinity, 1.0),
		Infinity,
		"sub_float64(Inf, 1) === Inf",
	);
	assert(
		fns.mul_float64(Infinity, 2.0),
		Infinity,
		"mul_float64(Inf, 2) === Inf",
	);
	assert(
		fns.mul_float64(Infinity, -2.0),
		-Infinity,
		"mul_float64(Inf, -2) === -Inf",
	);
	assert(fns.div_float64(1.0, 0.0), Infinity, "div_float64(1, 0) === Inf");
	assert(fns.div_float64(-1.0, 0.0), -Infinity, "div_float64(-1, 0) === -Inf");
	assert(fns.div_float64(1.0, Infinity), 0.0, "div_float64(1, Inf) === 0");

	// =================================================================
	// Infinity — float32
	// =================================================================
	suite("float Infinity — float32");
	assert(
		fns.add_float32(Infinity, 1.0),
		Infinity,
		"add_float32(Inf, 1) === Inf",
	);
	assert(fns.div_float32(1.0, 0.0), Infinity, "div_float32(1, 0) === Inf");
	assert(fns.div_float32(-1.0, 0.0), -Infinity, "div_float32(-1, 0) === -Inf");

	// =================================================================
	// Infinity — unary
	// =================================================================
	suite("float Infinity — unary");
	assert(fns.neg_float64(Infinity), -Infinity, "neg_float64(Inf) === -Inf");
	assert(fns.neg_float64(-Infinity), Infinity, "neg_float64(-Inf) === Inf");
	// abs: -Inf < 0 is true → returns -(-Inf) = Inf
	assert(fns.abs_float64(-Infinity), Infinity, "abs_float64(-Inf) === Inf");
	assert(fns.abs_float64(Infinity), Infinity, "abs_float64(Inf) === Inf");

	// =================================================================
	// Infinity — identity
	// =================================================================
	suite("float Infinity — identity");
	assert(
		fns.identity_float64(Infinity),
		Infinity,
		"identity_float64(Inf) === Inf",
	);
	assert(
		fns.identity_float64(-Infinity),
		-Infinity,
		"identity_float64(-Inf) === -Inf",
	);
	assert(
		fns.identity_float32(Infinity),
		Infinity,
		"identity_float32(Inf) === Inf",
	);

	// =================================================================
	// Infinity — min/max
	// =================================================================
	suite("float Infinity — min/max");
	assert(
		fns.min_float64(-Infinity, Infinity),
		-Infinity,
		"min_float64(-Inf, Inf) === -Inf",
	);
	assert(
		fns.max_float64(-Infinity, Infinity),
		Infinity,
		"max_float64(-Inf, Inf) === Inf",
	);
	assert(
		fns.min_float64(42.0, -Infinity),
		-Infinity,
		"min_float64(42, -Inf) === -Inf",
	);
	assert(
		fns.max_float64(42.0, Infinity),
		Infinity,
		"max_float64(42, Inf) === Inf",
	);

	// =================================================================
	// Infinity — clamp
	// =================================================================
	suite("float Infinity — clamp");
	assert(
		fns.clamp_float64(Infinity, 0.0, 10.0),
		10.0,
		"clamp_float64(Inf, 0, 10) === 10",
	);
	assert(
		fns.clamp_float64(-Infinity, 0.0, 10.0),
		0.0,
		"clamp_float64(-Inf, 0, 10) === 0",
	);

	// =================================================================
	// Negative zero (-0.0)
	// =================================================================
	suite("float negative zero");
	assert(fns.identity_float64(-0.0), -0.0, "identity_float64(-0) === -0");
	assert(fns.neg_float64(0.0), -0.0, "neg_float64(0) === -0");
	assert(fns.neg_float64(-0.0), 0.0, "neg_float64(-0) === 0");
	assert(fns.add_float64(-0.0, 0.0), 0.0, "add_float64(-0, 0) === 0");
	assert(fns.mul_float64(-1.0, 0.0), -0.0, "mul_float64(-1, 0) === -0");
	assert(fns.mul_float64(-0.0, -0.0), 0.0, "mul_float64(-0, -0) === 0");
	assert(fns.div_float64(1.0, -Infinity), -0.0, "div_float64(1, -Inf) === -0");

	// =================================================================
	// Subnormal / denormalized numbers
	// =================================================================
	suite("float subnormals");
	// Smallest positive subnormal f64: 5e-324
	const SUBNORMAL = 5e-324;
	assert(
		fns.identity_float64(SUBNORMAL),
		SUBNORMAL,
		"identity_float64(5e-324) roundtrips",
	);
	assert(
		fns.add_float64(SUBNORMAL, 0.0),
		SUBNORMAL,
		"add_float64(subnormal, 0) === subnormal",
	);
	assert(
		fns.neg_float64(SUBNORMAL),
		-SUBNORMAL,
		"neg_float64(subnormal) === -subnormal",
	);
	assert(
		fns.abs_float64(-SUBNORMAL),
		SUBNORMAL,
		"abs_float64(-subnormal) === subnormal",
	);
	// Subnormal * 2 may still be subnormal or become normal
	assert(
		fns.mul_float64(SUBNORMAL, 2.0),
		SUBNORMAL * 2.0,
		"mul_float64(subnormal, 2) === subnormal * 2",
	);

	// =================================================================
	// Float precision edge cases
	// =================================================================
	suite("float precision");
	// Classic: 0.1 + 0.2 !== 0.3 in IEEE 754
	assert(
		fns.add_float64(0.1, 0.2),
		0.1 + 0.2,
		"add_float64(0.1, 0.2) matches JS 0.1+0.2",
	);
	// Verify the WASM result also differs from 0.3
	assert(
		fns.add_float64(0.1, 0.2) !== 0.3,
		true,
		"add_float64(0.1, 0.2) !== 0.3 (IEEE 754 precision)",
	);
	// Large + small: catastrophic cancellation
	assert(
		fns.add_float64(1e16, 1.0),
		1e16 + 1.0,
		"add_float64(1e16, 1) matches JS precision",
	);
	assert(
		fns.sub_float64(1e16 + 2.0, 1e16),
		1e16 + 2.0 - 1e16,
		"sub_float64(1e16+2, 1e16) matches JS precision",
	);
}

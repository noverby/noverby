import type { WasmExports } from "../runtime/mod.ts";
import { assert, suite } from "./harness.ts";

export function testIdentity(fns: WasmExports): void {
	// =================================================================
	// Identity / passthrough
	// =================================================================
	suite("identity");
	assert(fns.identity_int32(0), 0, "identity_int32(0) === 0");
	assert(fns.identity_int32(42), 42, "identity_int32(42) === 42");
	assert(fns.identity_int32(-42), -42, "identity_int32(-42) === -42");
	assert(fns.identity_int64(0n), 0n, "identity_int64(0) === 0");
	assert(fns.identity_int64(999n), 999n, "identity_int64(999) === 999");
	assert(fns.identity_int64(-999n), -999n, "identity_int64(-999) === -999");
	assert(
		fns.identity_float32(3.14),
		Math.fround(3.14),
		"identity_float32(3.14)",
	);
	assert(fns.identity_float32(0.0), 0.0, "identity_float32(0) === 0");
	assert(fns.identity_float64(Math.PI), Math.PI, "identity_float64(pi)");
	assert(fns.identity_float64(0.0), 0.0, "identity_float64(0) === 0");
	assert(fns.identity_float64(-0.0), -0.0, "identity_float64(-0) === -0");
}

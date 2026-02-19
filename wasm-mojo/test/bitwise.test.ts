import type { WasmExports } from "../runtime/mod.ts";
import { assert, suite } from "./harness.ts";

export function testBitwise(fns: WasmExports): void {
	// =================================================================
	// Bitwise operations
	// =================================================================
	suite("bitwise");
	assert(
		fns.bitand_int32(0b1100, 0b1010),
		0b1000,
		"bitand_int32(0b1100, 0b1010) === 0b1000",
	);
	assert(
		fns.bitand_int32(0xff, 0x0f),
		0x0f,
		"bitand_int32(0xFF, 0x0F) === 0x0F",
	);
	assert(fns.bitand_int32(0, 0xffff), 0, "bitand_int32(0, 0xFFFF) === 0");
	assert(
		fns.bitor_int32(0b1100, 0b1010),
		0b1110,
		"bitor_int32(0b1100, 0b1010) === 0b1110",
	);
	assert(fns.bitor_int32(0, 0), 0, "bitor_int32(0, 0) === 0");
	assert(
		fns.bitxor_int32(0b1100, 0b1010),
		0b0110,
		"bitxor_int32(0b1100, 0b1010) === 0b0110",
	);
	assert(fns.bitxor_int32(42, 42), 0, "bitxor_int32(42, 42) === 0");
	assert(fns.bitxor_int32(42, 0), 42, "bitxor_int32(42, 0) === 42");
	assert(fns.bitnot_int32(0), ~0, "bitnot_int32(0) === ~0");
	assert(fns.bitnot_int32(1), ~1, "bitnot_int32(1) === ~1");

	// =================================================================
	// Bitwise shifts
	// =================================================================
	suite("bitwise shifts");
	assert(fns.shl_int32(1, 0), 1, "shl_int32(1, 0) === 1");
	assert(fns.shl_int32(1, 1), 2, "shl_int32(1, 1) === 2");
	assert(fns.shl_int32(1, 4), 16, "shl_int32(1, 4) === 16");
	assert(fns.shl_int32(3, 3), 24, "shl_int32(3, 3) === 24");
	assert(fns.shr_int32(16, 4), 1, "shr_int32(16, 4) === 1");
	assert(fns.shr_int32(24, 3), 3, "shr_int32(24, 3) === 3");
	assert(fns.shr_int32(255, 1), 127, "shr_int32(255, 1) === 127");
}

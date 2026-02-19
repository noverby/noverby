import type { WasmExports } from "../runtime/mod.ts";
import {
	allocStringStruct,
	readStringStruct,
	writeStringStruct,
} from "../runtime/mod.ts";
import { assert, suite } from "./harness.ts";

export function testSSO(fns: WasmExports): void {
	// =================================================================
	// String SSO boundary
	//
	// Mojo's Small String Optimization stores strings inline in the
	// 24-byte struct when they fit (≤23 bytes). At 24+ bytes the data
	// is heap-allocated. These tests exercise the boundary to verify
	// that readStringStruct correctly handles both SSO and heap paths,
	// and that Mojo functions produce correct results across the
	// transition.
	// =================================================================

	// =================================================================
	// SSO roundtrip via return_input_string
	// =================================================================
	suite("string SSO boundary — roundtrip");

	// 22 bytes: comfortably within SSO
	{
		const str = "a".repeat(22);
		const inPtr = writeStringStruct(str);
		const outPtr = allocStringStruct();
		fns.return_input_string(inPtr, outPtr);
		const result = readStringStruct(outPtr);
		assert(result, str, "return_input_string 22-byte string (SSO)");
	}

	// 23 bytes: max SSO capacity
	{
		const str = "b".repeat(23);
		const inPtr = writeStringStruct(str);
		const outPtr = allocStringStruct();
		fns.return_input_string(inPtr, outPtr);
		const result = readStringStruct(outPtr);
		assert(result, str, "return_input_string 23-byte string (SSO max)");
	}

	// 24 bytes: first heap-allocated size
	{
		const str = "c".repeat(24);
		const inPtr = writeStringStruct(str);
		const outPtr = allocStringStruct();
		fns.return_input_string(inPtr, outPtr);
		const result = readStringStruct(outPtr);
		assert(result, str, "return_input_string 24-byte string (heap)");
	}

	// 25 bytes: safely past the boundary
	{
		const str = "d".repeat(25);
		const inPtr = writeStringStruct(str);
		const outPtr = allocStringStruct();
		fns.return_input_string(inPtr, outPtr);
		const result = readStringStruct(outPtr);
		assert(result, str, "return_input_string 25-byte string (heap)");
	}

	// =================================================================
	// SSO boundary — string_length
	// =================================================================
	suite("string SSO boundary — length");

	{
		const str = "x".repeat(22);
		const ptr = writeStringStruct(str);
		assert(fns.string_length(ptr), 22n, "string_length 22-byte (SSO)");
	}
	{
		const str = "x".repeat(23);
		const ptr = writeStringStruct(str);
		assert(fns.string_length(ptr), 23n, "string_length 23-byte (SSO max)");
	}
	{
		const str = "x".repeat(24);
		const ptr = writeStringStruct(str);
		assert(fns.string_length(ptr), 24n, "string_length 24-byte (heap)");
	}

	// =================================================================
	// SSO boundary — string_eq
	// =================================================================
	suite("string SSO boundary — equality");

	// Both SSO
	{
		const aPtr = writeStringStruct("y".repeat(23));
		const bPtr = writeStringStruct("y".repeat(23));
		assert(
			fns.string_eq(aPtr, bPtr),
			1,
			"string_eq 23-byte identical (SSO === SSO)",
		);
	}

	// SSO vs heap: same logical content should never match here
	// because 23 bytes !== 24 bytes
	{
		const aPtr = writeStringStruct("z".repeat(23));
		const bPtr = writeStringStruct("z".repeat(24));
		assert(
			fns.string_eq(aPtr, bPtr),
			0,
			"string_eq 23-byte vs 24-byte (SSO !== heap, different length)",
		);
	}

	// Both heap
	{
		const aPtr = writeStringStruct("w".repeat(24));
		const bPtr = writeStringStruct("w".repeat(24));
		assert(
			fns.string_eq(aPtr, bPtr),
			1,
			"string_eq 24-byte identical (heap === heap)",
		);
	}

	// Same length at boundary, different content
	{
		const aPtr = writeStringStruct("a".repeat(23));
		const bPtr = writeStringStruct(`${"a".repeat(22)}b`);
		assert(
			fns.string_eq(aPtr, bPtr),
			0,
			"string_eq 23-byte differ in last byte (SSO)",
		);
	}

	// =================================================================
	// SSO boundary — string_concat crossing the boundary
	// =================================================================
	suite("string SSO boundary — concat");

	// Two small strings that concat to exactly 23 bytes (SSO)
	{
		const aPtr = writeStringStruct("a".repeat(11));
		const bPtr = writeStringStruct("b".repeat(12));
		const outPtr = allocStringStruct();
		fns.string_concat(aPtr, bPtr, outPtr);
		const result = readStringStruct(outPtr);
		assert(
			result,
			"a".repeat(11) + "b".repeat(12),
			"string_concat 11+12=23 bytes (result at SSO max)",
		);
		assert(
			fns.string_length(outPtr),
			23n,
			"string_concat result length === 23",
		);
	}

	// Two small strings that concat to exactly 24 bytes (crosses to heap)
	{
		const aPtr = writeStringStruct("a".repeat(12));
		const bPtr = writeStringStruct("b".repeat(12));
		const outPtr = allocStringStruct();
		fns.string_concat(aPtr, bPtr, outPtr);
		const result = readStringStruct(outPtr);
		assert(
			result,
			"a".repeat(12) + "b".repeat(12),
			"string_concat 12+12=24 bytes (result crosses to heap)",
		);
		assert(
			fns.string_length(outPtr),
			24n,
			"string_concat result length === 24",
		);
	}

	// =================================================================
	// SSO boundary — string_repeat crossing the boundary
	// =================================================================
	suite("string SSO boundary — repeat");

	// 8 * 3 = 24 bytes → heap
	{
		const ptr = writeStringStruct("a".repeat(8));
		const outPtr = allocStringStruct();
		fns.string_repeat(ptr, 3, outPtr);
		const result = readStringStruct(outPtr);
		assert(
			result,
			"a".repeat(24),
			"string_repeat 8-byte * 3 = 24 bytes (crosses to heap)",
		);
	}

	// 23 * 1 = 23 bytes → stays SSO
	{
		const ptr = writeStringStruct("q".repeat(23));
		const outPtr = allocStringStruct();
		fns.string_repeat(ptr, 1, outPtr);
		const result = readStringStruct(outPtr);
		assert(
			result,
			"q".repeat(23),
			"string_repeat 23-byte * 1 = 23 bytes (stays SSO)",
		);
	}

	// =================================================================
	// Larger heap strings (well past SSO)
	// =================================================================
	suite("string SSO boundary — larger heap");

	{
		const str = "abc".repeat(50); // 150 bytes
		const inPtr = writeStringStruct(str);
		const outPtr = allocStringStruct();
		fns.return_input_string(inPtr, outPtr);
		const result = readStringStruct(outPtr);
		assert(result, str, "return_input_string 150-byte string (well past SSO)");
	}
	{
		const str = "x".repeat(256);
		const ptr = writeStringStruct(str);
		assert(fns.string_length(ptr), 256n, "string_length 256-byte (heap)");
	}
}

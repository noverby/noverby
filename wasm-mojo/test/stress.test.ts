import type { WasmExports } from "../runtime/mod.ts";
import {
	allocStringStruct,
	readStringStruct,
	writeStringStruct,
} from "../runtime/mod.ts";
import { assert, suite } from "./harness.ts";

export function testStress(fns: WasmExports): void {
	// =================================================================
	// Allocator stress — many sequential string allocations
	// =================================================================
	suite("stress — sequential string allocations");

	// Write 200 distinct strings and verify they all read back correctly
	{
		const ptrs: bigint[] = [];
		const strings: string[] = [];
		for (let i = 0; i < 200; i++) {
			const str = `string-${i}-${"x".repeat(i % 30)}`;
			strings.push(str);
			ptrs.push(writeStringStruct(str));
		}
		// Read them all back — earlier allocations must not be corrupted
		let allCorrect = true;
		for (let i = 0; i < 200; i++) {
			const result = readStringStruct(ptrs[i]);
			if (result !== strings[i]) {
				allCorrect = false;
				assert(result, strings[i], `readback string #${i} after 200 allocs`);
				break;
			}
		}
		if (allCorrect) {
			assert(
				true,
				true,
				"200 sequential string allocations all read back correctly",
			);
		}
	}

	// =================================================================
	// Allocator stress — many alloc + return_input_string roundtrips
	// =================================================================
	suite("stress — string roundtrip pipeline");

	{
		let allCorrect = true;
		for (let i = 0; i < 100; i++) {
			const str = `roundtrip-${i}`;
			const inPtr = writeStringStruct(str);
			const outPtr = allocStringStruct();
			fns.return_input_string(inPtr, outPtr);
			const result = readStringStruct(outPtr);
			if (result !== str) {
				allCorrect = false;
				assert(result, str, `roundtrip #${i} failed`);
				break;
			}
		}
		if (allCorrect) {
			assert(true, true, "100 return_input_string roundtrips all correct");
		}
	}

	// =================================================================
	// Allocator stress — many concat operations
	// =================================================================
	suite("stress — repeated concat");

	// Build a string by concatenating "ab" 50 times through WASM
	{
		let currentPtr = writeStringStruct("");
		for (let i = 0; i < 50; i++) {
			const appendPtr = writeStringStruct("ab");
			const outPtr = allocStringStruct();
			fns.string_concat(currentPtr, appendPtr, outPtr);
			currentPtr = outPtr;
		}
		const result = readStringStruct(currentPtr);
		assert(
			result,
			"ab".repeat(50),
			"50 sequential concats produce correct result",
		);
		assert(
			fns.string_length(currentPtr),
			100n,
			"50 sequential concats produce 100-byte string",
		);
	}

	// =================================================================
	// Allocator stress — interleaved numeric and string operations
	// =================================================================
	suite("stress — interleaved numeric + string ops");

	{
		let allCorrect = true;
		for (let i = 0; i < 50; i++) {
			// Do some numeric work
			const x = fns.add_int32(i, i);
			const y = fns.mul_int32(i, 3);
			const g = fns.gcd_int32(x, y);

			// Do a string roundtrip
			const str = `iter-${i}-gcd-${g}`;
			const inPtr = writeStringStruct(str);
			const outPtr = allocStringStruct();
			fns.return_input_string(inPtr, outPtr);
			const result = readStringStruct(outPtr);

			// Verify numeric result
			const expectedGcd = gcd(i + i, i * 3);
			if (g !== expectedGcd) {
				allCorrect = false;
				assert(g, expectedGcd, `gcd at iteration ${i}`);
				break;
			}

			// Verify string result
			if (result !== str) {
				allCorrect = false;
				assert(result, str, `string roundtrip at iteration ${i}`);
				break;
			}
		}
		if (allCorrect) {
			assert(
				true,
				true,
				"50 interleaved numeric+string operations all correct",
			);
		}
	}

	// =================================================================
	// Allocator stress — many small allocStringStruct calls
	// =================================================================
	suite("stress — empty struct allocations");

	{
		const ptrs: bigint[] = [];
		for (let i = 0; i < 300; i++) {
			ptrs.push(allocStringStruct());
		}
		// Verify no two pointers overlap (each struct is 24 bytes)
		let noOverlap = true;
		for (let i = 1; i < ptrs.length; i++) {
			const gap = ptrs[i] - ptrs[i - 1];
			if (gap < 24n) {
				noOverlap = false;
				assert(
					Number(gap),
					24,
					`struct #${i} overlaps with #${i - 1} (gap=${gap})`,
				);
				break;
			}
		}
		if (noOverlap) {
			assert(
				true,
				true,
				"300 allocStringStruct calls produce non-overlapping structs",
			);
		}
	}

	// =================================================================
	// Allocator stress — mixed-size string allocations
	// =================================================================
	suite("stress — mixed-size allocations");

	{
		const sizes = [0, 1, 5, 22, 23, 24, 25, 50, 100, 255, 1, 0, 23, 24];
		const ptrs: bigint[] = [];
		const strings: string[] = [];

		for (const size of sizes) {
			const str = "m".repeat(size);
			strings.push(str);
			ptrs.push(writeStringStruct(str));
		}

		let allCorrect = true;
		for (let i = 0; i < sizes.length; i++) {
			const len = fns.string_length(ptrs[i]);
			if (len !== BigInt(sizes[i])) {
				allCorrect = false;
				assert(Number(len), sizes[i], `length of ${sizes[i]}-byte string`);
				break;
			}
		}
		if (allCorrect) {
			assert(
				true,
				true,
				`${sizes.length} mixed-size strings all report correct length`,
			);
		}
	}

	// =================================================================
	// Computation stress — fibonacci consistency across many values
	// =================================================================
	suite("stress — fibonacci sequence consistency");

	// Verify fib(n) = fib(n-1) + fib(n-2) for n = 2..40
	{
		let allCorrect = true;
		for (let n = 2; n <= 40; n++) {
			const fn0 = fns.fib_int32(n);
			const fn1 = fns.fib_int32(n - 1);
			const fn2 = fns.fib_int32(n - 2);
			// Use i32 wrapping addition for the check (matches WASM semantics)
			const expected = (fn1 + fn2) | 0;
			if (fn0 !== expected) {
				allCorrect = false;
				assert(fn0, expected, `fib(${n}) === fib(${n - 1}) + fib(${n - 2})`);
				break;
			}
		}
		if (allCorrect) {
			assert(true, true, "fib(n) = fib(n-1) + fib(n-2) holds for n=2..40");
		}
	}

	// =================================================================
	// Computation stress — string_eq reflexivity for many strings
	// =================================================================
	suite("stress — string_eq reflexivity");

	{
		const testStrings = [
			"",
			"a",
			"hello",
			"x".repeat(23),
			"y".repeat(24),
			"café",
			"🌍🌎🌏",
			"Hello, World! 🎉",
			"z".repeat(100),
		];
		let allCorrect = true;
		for (const str of testStrings) {
			const aPtr = writeStringStruct(str);
			const bPtr = writeStringStruct(str);
			const eq = fns.string_eq(aPtr, bPtr);
			if (eq !== 1) {
				allCorrect = false;
				assert(eq, 1, `string_eq reflexive for "${str.slice(0, 20)}..."`);
				break;
			}
		}
		if (allCorrect) {
			assert(
				true,
				true,
				`string_eq reflexive for ${testStrings.length} distinct strings`,
			);
		}
	}
}

// --- Helper ---

function gcd(a: number, b: number): number {
	a = Math.abs(a);
	b = Math.abs(b);
	while (b !== 0) {
		const t = b;
		b = a % b;
		a = t;
	}
	return a;
}

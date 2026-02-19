import type { WasmExports } from "../runtime/mod.ts";
import { allocStringStruct, writeStringStruct } from "../runtime/mod.ts";
import { assert, suite } from "./harness.ts";

export function testConsistency(fns: WasmExports): void {
	// =================================================================
	// Cross-function consistency checks
	// =================================================================
	suite("cross-function consistency");

	// add and sub are inverses
	{
		const x = 17;
		const y = 9;
		const sum = fns.add_int32(x, y);
		assert(
			fns.sub_int32(sum, y),
			x,
			"sub(add(x, y), y) === x (add/sub inverse)",
		);
	}

	// mul and div are inverses for exact division
	{
		const x = 6;
		const y = 3;
		const product = fns.mul_int32(x, y);
		assert(
			fns.div_int32(product, y),
			x,
			"div(mul(x, y), y) === x (mul/div inverse)",
		);
	}

	// neg(neg(x)) === x
	assert(fns.neg_int32(fns.neg_int32(42)), 42, "neg(neg(42)) === 42");

	// abs(neg(x)) === abs(x) for positive x
	assert(
		fns.abs_int32(fns.neg_int32(7)),
		fns.abs_int32(7),
		"abs(neg(7)) === abs(7)",
	);

	// min(x, y) <= max(x, y)
	{
		const a = 3;
		const b = 7;
		const lo = fns.min_int32(a, b);
		const hi = fns.max_int32(a, b);
		assert(fns.le_int32(lo, hi), 1, "min(x,y) <= max(x,y)");
	}

	// x & y | x ^ y === x | y  (bitwise identity)
	{
		const x = 0b1100;
		const y = 0b1010;
		assert(
			fns.bitor_int32(fns.bitand_int32(x, y), fns.bitxor_int32(x, y)),
			fns.bitor_int32(x, y),
			"(x & y) | (x ^ y) === x | y",
		);
	}

	// shl then shr roundtrip
	{
		const x = 5;
		assert(fns.shr_int32(fns.shl_int32(x, 4), 4), x, "shr(shl(x, 4), 4) === x");
	}

	// De Morgan's law: not(and(a,b)) === or(not(a), not(b))
	{
		const a = 1;
		const b = 0;
		assert(
			fns.bool_not(fns.bool_and(a, b)),
			fns.bool_or(fns.bool_not(a), fns.bool_not(b)),
			"De Morgan: not(and(a,b)) === or(not(a), not(b))",
		);
	}

	// gcd(a*k, b*k) === k * gcd(a, b)
	{
		const a = 6;
		const b = 4;
		const k = 5;
		assert(
			fns.gcd_int32(fns.mul_int32(a, k), fns.mul_int32(b, k)),
			fns.mul_int32(k, fns.gcd_int32(a, b)),
			"gcd(a*k, b*k) === k * gcd(a, b)",
		);
	}

	// Fibonacci property: fib(n) = fib(n-1) + fib(n-2)
	for (const n of [5, 8, 12, 15]) {
		const fn2 = fns.fib_int32(n - 2);
		const fn1 = fns.fib_int32(n - 1);
		const fn0 = fns.fib_int32(n);
		assert(fn0, fn1 + fn2, `fib(${n}) === fib(${n - 1}) + fib(${n - 2})`);
	}

	// Factorial property: n! === n * (n-1)!
	for (const n of [2, 3, 4, 5, 6, 7]) {
		assert(
			fns.factorial_int32(n),
			n * fns.factorial_int32(n - 1),
			`factorial(${n}) === ${n} * factorial(${n - 1})`,
		);
	}

	// string_length(concat(a, b)) === string_length(a) + string_length(b)
	{
		const aPtr = writeStringStruct("foo");
		const bPtr = writeStringStruct("barbaz");
		const outPtr = allocStringStruct();
		fns.string_concat(aPtr, bPtr, outPtr);
		assert(
			fns.string_length(outPtr),
			fns.string_length(aPtr) + fns.string_length(bPtr),
			"len(concat(a,b)) === len(a) + len(b)",
		);
	}

	// clamp(x, lo, hi) === max(lo, min(hi, x))
	for (const x of [-5, 0, 5, 10, 15]) {
		const lo = 0;
		const hi = 10;
		assert(
			fns.clamp_int32(x, lo, hi),
			fns.max_int32(lo, fns.min_int32(hi, x)),
			`clamp(${x}, ${lo}, ${hi}) === max(lo, min(hi, x))`,
		);
	}
}

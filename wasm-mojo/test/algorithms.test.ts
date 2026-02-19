import type { WasmExports } from "../runtime/mod.ts";
import { assert, suite } from "./harness.ts";

export function testAlgorithms(fns: WasmExports): void {
	// =================================================================
	// Fibonacci — int32
	// =================================================================
	suite("fib_int32");
	assert(fns.fib_int32(0), 0, "fib_int32(0) === 0");
	assert(fns.fib_int32(1), 1, "fib_int32(1) === 1");
	assert(fns.fib_int32(2), 1, "fib_int32(2) === 1");
	assert(fns.fib_int32(3), 2, "fib_int32(3) === 2");
	assert(fns.fib_int32(4), 3, "fib_int32(4) === 3");
	assert(fns.fib_int32(5), 5, "fib_int32(5) === 5");
	assert(fns.fib_int32(6), 8, "fib_int32(6) === 8");
	assert(fns.fib_int32(7), 13, "fib_int32(7) === 13");
	assert(fns.fib_int32(10), 55, "fib_int32(10) === 55");
	assert(fns.fib_int32(20), 6765, "fib_int32(20) === 6765");

	// =================================================================
	// Fibonacci — int64
	// =================================================================
	suite("fib_int64");
	assert(fns.fib_int64(0n), 0n, "fib_int64(0) === 0");
	assert(fns.fib_int64(1n), 1n, "fib_int64(1) === 1");
	assert(fns.fib_int64(10n), 55n, "fib_int64(10) === 55");
	assert(fns.fib_int64(20n), 6765n, "fib_int64(20) === 6765");
	assert(fns.fib_int64(50n), 12586269025n, "fib_int64(50) === 12586269025");

	// =================================================================
	// Factorial — int32
	// =================================================================
	suite("factorial_int32");
	assert(fns.factorial_int32(0), 1, "factorial_int32(0) === 1");
	assert(fns.factorial_int32(1), 1, "factorial_int32(1) === 1");
	assert(fns.factorial_int32(2), 2, "factorial_int32(2) === 2");
	assert(fns.factorial_int32(3), 6, "factorial_int32(3) === 6");
	assert(fns.factorial_int32(4), 24, "factorial_int32(4) === 24");
	assert(fns.factorial_int32(5), 120, "factorial_int32(5) === 120");
	assert(fns.factorial_int32(10), 3628800, "factorial_int32(10) === 3628800");

	// =================================================================
	// Factorial — int64
	// =================================================================
	suite("factorial_int64");
	assert(fns.factorial_int64(0n), 1n, "factorial_int64(0) === 1");
	assert(fns.factorial_int64(1n), 1n, "factorial_int64(1) === 1");
	assert(fns.factorial_int64(5n), 120n, "factorial_int64(5) === 120");
	assert(fns.factorial_int64(10n), 3628800n, "factorial_int64(10) === 3628800");
	assert(
		fns.factorial_int64(20n),
		2432902008176640000n,
		"factorial_int64(20) === 2432902008176640000",
	);

	// =================================================================
	// GCD (Euclidean algorithm)
	// =================================================================
	suite("gcd_int32");
	assert(fns.gcd_int32(12, 8), 4, "gcd_int32(12, 8) === 4");
	assert(fns.gcd_int32(8, 12), 4, "gcd_int32(8, 12) === 4 (commutative)");
	assert(fns.gcd_int32(7, 13), 1, "gcd_int32(7, 13) === 1 (coprime)");
	assert(fns.gcd_int32(100, 75), 25, "gcd_int32(100, 75) === 25");
	assert(fns.gcd_int32(0, 5), 5, "gcd_int32(0, 5) === 5");
	assert(fns.gcd_int32(5, 0), 5, "gcd_int32(5, 0) === 5");
	assert(fns.gcd_int32(7, 7), 7, "gcd_int32(7, 7) === 7");
	assert(fns.gcd_int32(1, 100), 1, "gcd_int32(1, 100) === 1");
	assert(fns.gcd_int32(-12, 8), 4, "gcd_int32(-12, 8) === 4 (negative input)");
	assert(fns.gcd_int32(12, -8), 4, "gcd_int32(12, -8) === 4 (negative input)");
	assert(fns.gcd_int32(-12, -8), 4, "gcd_int32(-12, -8) === 4 (both negative)");
	assert(fns.gcd_int32(48, 18), 6, "gcd_int32(48, 18) === 6");
	assert(
		fns.gcd_int32(1071, 462),
		21,
		"gcd_int32(1071, 462) === 21 (classic Euclid example)",
	);
}

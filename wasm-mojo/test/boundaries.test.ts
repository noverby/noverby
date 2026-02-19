import type { WasmExports } from "../runtime/mod.ts";
import { assert, suite } from "./harness.ts";

// 32-bit signed integer limits
const INT32_MAX = 2147483647;
const INT32_MIN = -2147483648;

// 64-bit signed integer limits
const INT64_MAX = 9223372036854775807n;
const INT64_MIN = -9223372036854775808n;

export function testBoundaries(fns: WasmExports): void {
	// =================================================================
	// Int32 boundary values — identity
	// =================================================================
	suite("int32 boundaries — identity");
	assert(fns.identity_int32(INT32_MAX), INT32_MAX, "identity_int32(INT32_MAX)");
	assert(fns.identity_int32(INT32_MIN), INT32_MIN, "identity_int32(INT32_MIN)");
	assert(fns.identity_int32(0), 0, "identity_int32(0)");

	// =================================================================
	// Int64 boundary values — identity
	// =================================================================
	suite("int64 boundaries — identity");
	assert(fns.identity_int64(INT64_MAX), INT64_MAX, "identity_int64(INT64_MAX)");
	assert(fns.identity_int64(INT64_MIN), INT64_MIN, "identity_int64(INT64_MIN)");

	// =================================================================
	// Int32 overflow — addition
	// =================================================================
	suite("int32 overflow — add");
	assert(
		fns.add_int32(INT32_MAX, 1),
		INT32_MIN,
		"add_int32(INT32_MAX, 1) wraps to INT32_MIN",
	);
	assert(
		fns.add_int32(INT32_MIN, -1),
		INT32_MAX,
		"add_int32(INT32_MIN, -1) wraps to INT32_MAX",
	);
	assert(
		fns.add_int32(INT32_MAX, INT32_MAX),
		-2,
		"add_int32(INT32_MAX, INT32_MAX) wraps to -2",
	);

	// =================================================================
	// Int64 overflow — addition
	// =================================================================
	suite("int64 overflow — add");
	assert(
		fns.add_int64(INT64_MAX, 1n),
		INT64_MIN,
		"add_int64(INT64_MAX, 1) wraps to INT64_MIN",
	);
	assert(
		fns.add_int64(INT64_MIN, -1n),
		INT64_MAX,
		"add_int64(INT64_MIN, -1) wraps to INT64_MAX",
	);

	// =================================================================
	// Int32 overflow — subtraction
	// =================================================================
	suite("int32 overflow — sub");
	assert(
		fns.sub_int32(INT32_MIN, 1),
		INT32_MAX,
		"sub_int32(INT32_MIN, 1) wraps to INT32_MAX",
	);
	assert(
		fns.sub_int32(INT32_MAX, -1),
		INT32_MIN,
		"sub_int32(INT32_MAX, -1) wraps to INT32_MIN",
	);

	// =================================================================
	// Int32 overflow — multiplication
	// =================================================================
	suite("int32 overflow — mul");
	assert(
		fns.mul_int32(INT32_MAX, 2),
		-2,
		"mul_int32(INT32_MAX, 2) wraps to -2",
	);
	assert(
		fns.mul_int32(INT32_MIN, -1),
		INT32_MIN,
		"mul_int32(INT32_MIN, -1) wraps to INT32_MIN (no positive equivalent)",
	);

	// =================================================================
	// Int32 overflow — negation
	// =================================================================
	suite("int32 overflow — neg");
	assert(
		fns.neg_int32(INT32_MAX),
		-INT32_MAX,
		"neg_int32(INT32_MAX) === -INT32_MAX",
	);
	assert(
		fns.neg_int32(INT32_MIN),
		INT32_MIN,
		"neg_int32(INT32_MIN) wraps to INT32_MIN (2's complement)",
	);

	// =================================================================
	// Int64 overflow — negation
	// =================================================================
	suite("int64 overflow — neg");
	assert(
		fns.neg_int64(INT64_MAX),
		-INT64_MAX,
		"neg_int64(INT64_MAX) === -INT64_MAX",
	);
	assert(
		fns.neg_int64(INT64_MIN),
		INT64_MIN,
		"neg_int64(INT64_MIN) wraps to INT64_MIN (2's complement)",
	);

	// =================================================================
	// Int32 boundary — abs
	// =================================================================
	suite("int32 boundary — abs");
	assert(
		fns.abs_int32(INT32_MAX),
		INT32_MAX,
		"abs_int32(INT32_MAX) === INT32_MAX",
	);
	assert(
		fns.abs_int32(INT32_MIN),
		INT32_MIN,
		"abs_int32(INT32_MIN) wraps to INT32_MIN (no positive equivalent)",
	);
	assert(
		fns.abs_int32(INT32_MIN + 1),
		INT32_MAX,
		"abs_int32(INT32_MIN + 1) === INT32_MAX",
	);

	// =================================================================
	// Int32 boundary — min / max
	// =================================================================
	suite("int32 boundary — min/max");
	assert(
		fns.min_int32(INT32_MIN, INT32_MAX),
		INT32_MIN,
		"min_int32(INT32_MIN, INT32_MAX) === INT32_MIN",
	);
	assert(
		fns.max_int32(INT32_MIN, INT32_MAX),
		INT32_MAX,
		"max_int32(INT32_MIN, INT32_MAX) === INT32_MAX",
	);
	assert(
		fns.min_int32(INT32_MIN, INT32_MIN),
		INT32_MIN,
		"min_int32(INT32_MIN, INT32_MIN) === INT32_MIN",
	);
	assert(
		fns.max_int32(INT32_MAX, INT32_MAX),
		INT32_MAX,
		"max_int32(INT32_MAX, INT32_MAX) === INT32_MAX",
	);

	// =================================================================
	// Int32 boundary — comparison
	// =================================================================
	suite("int32 boundary — comparison");
	assert(fns.lt_int32(INT32_MIN, INT32_MAX), 1, "INT32_MIN < INT32_MAX");
	assert(fns.gt_int32(INT32_MAX, INT32_MIN), 1, "INT32_MAX > INT32_MIN");
	assert(fns.eq_int32(INT32_MAX, INT32_MAX), 1, "INT32_MAX === INT32_MAX");
	assert(fns.eq_int32(INT32_MIN, INT32_MIN), 1, "INT32_MIN === INT32_MIN");
	assert(fns.ne_int32(INT32_MIN, INT32_MAX), 1, "INT32_MIN !== INT32_MAX");

	// =================================================================
	// Int32 boundary — clamp
	// =================================================================
	suite("int32 boundary — clamp");
	assert(
		fns.clamp_int32(INT32_MIN, 0, 100),
		0,
		"clamp_int32(INT32_MIN, 0, 100) === 0",
	);
	assert(
		fns.clamp_int32(INT32_MAX, 0, 100),
		100,
		"clamp_int32(INT32_MAX, 0, 100) === 100",
	);
	assert(
		fns.clamp_int32(50, INT32_MIN, INT32_MAX),
		50,
		"clamp_int32(50, INT32_MIN, INT32_MAX) === 50",
	);

	// =================================================================
	// Int32 boundary — bitwise
	// =================================================================
	suite("int32 boundary — bitwise");
	assert(
		fns.bitnot_int32(INT32_MAX),
		INT32_MIN,
		"bitnot_int32(INT32_MAX) === INT32_MIN",
	);
	assert(
		fns.bitnot_int32(INT32_MIN),
		INT32_MAX,
		"bitnot_int32(INT32_MIN) === INT32_MAX",
	);
	assert(
		fns.bitand_int32(INT32_MAX, INT32_MIN),
		0,
		"bitand_int32(INT32_MAX, INT32_MIN) === 0",
	);
	assert(
		fns.bitor_int32(INT32_MAX, INT32_MIN),
		-1,
		"bitor_int32(INT32_MAX, INT32_MIN) === -1 (all bits set)",
	);
	assert(
		fns.bitxor_int32(INT32_MAX, INT32_MIN),
		-1,
		"bitxor_int32(INT32_MAX, INT32_MIN) === -1",
	);

	// =================================================================
	// Int32 boundary — GCD
	// =================================================================
	suite("int32 boundary — gcd");
	assert(fns.gcd_int32(INT32_MAX, 1), 1, "gcd_int32(INT32_MAX, 1) === 1");
	assert(
		fns.gcd_int32(INT32_MAX, INT32_MAX),
		INT32_MAX,
		"gcd_int32(INT32_MAX, INT32_MAX) === INT32_MAX",
	);

	// =================================================================
	// Int32 overflow — factorial
	// =================================================================
	suite("int32 overflow — factorial");
	// 12! = 479001600, fits in Int32
	assert(
		fns.factorial_int32(12),
		479001600,
		"factorial_int32(12) === 479001600",
	);
	// 13! = 6227020800, overflows Int32 — verify it wraps
	assert(
		fns.factorial_int32(13),
		1932053504,
		"factorial_int32(13) wraps (6227020800 truncated to i32)",
	);

	// =================================================================
	// Int64 boundary — factorial
	// =================================================================
	suite("int64 boundary — factorial");
	// 20! = 2432902008176640000, fits in Int64
	assert(
		fns.factorial_int64(20n),
		2432902008176640000n,
		"factorial_int64(20) === 2432902008176640000",
	);
	// 21! = 51090942171709440000, overflows Int64 — verify it wraps
	assert(
		fns.factorial_int64(21n),
		-4249290049419214848n,
		"factorial_int64(21) wraps (overflow)",
	);

	// =================================================================
	// Int32 overflow — fibonacci
	// =================================================================
	suite("int32 overflow — fibonacci");
	// fib(46) = 1836311903, fits in Int32
	assert(fns.fib_int32(46), 1836311903, "fib_int32(46) === 1836311903");
	// fib(47) = 2971215073, overflows Int32 — verify wrapping
	assert(
		fns.fib_int32(47),
		-1323752223,
		"fib_int32(47) wraps (2971215073 truncated to i32)",
	);
}

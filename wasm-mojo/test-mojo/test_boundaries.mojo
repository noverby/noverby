# Tests for integer boundary / overflow cases — native Mojo tests run with `mojo test`.
#
# These tests are a direct port of test/boundaries.test.ts, exercising
# the same scenarios without the WASM/JS round-trip.  The functions
# under test are defined inline here (matching main.mojo's @export fns).
#
# Run with:
#   mojo test -I src test-mojo/test_boundaries.mojo

from testing import assert_equal, assert_true

# ── Constants ────────────────────────────────────────────────────────────────

alias INT32_MAX: Int32 = 2147483647
alias INT32_MIN: Int32 = -2147483648
alias INT64_MAX: Int64 = 9223372036854775807
alias INT64_MIN: Int64 = -9223372036854775808


# ── Functions under test (mirrors of main.mojo @export fns) ──────────────────


fn identity_int32(x: Int32) -> Int32:
    return x


fn identity_int64(x: Int64) -> Int64:
    return x


fn add_int32(x: Int32, y: Int32) -> Int32:
    return x + y


fn add_int64(x: Int64, y: Int64) -> Int64:
    return x + y


fn sub_int32(x: Int32, y: Int32) -> Int32:
    return x - y


fn mul_int32(x: Int32, y: Int32) -> Int32:
    return x * y


fn neg_int32(x: Int32) -> Int32:
    return -x


fn neg_int64(x: Int64) -> Int64:
    return -x


fn abs_int32(x: Int32) -> Int32:
    if x < 0:
        return -x
    return x


fn min_int32(x: Int32, y: Int32) -> Int32:
    if x < y:
        return x
    return y


fn max_int32(x: Int32, y: Int32) -> Int32:
    if x > y:
        return x
    return y


fn lt_int32(x: Int32, y: Int32) -> Int32:
    if x < y:
        return 1
    return 0


fn gt_int32(x: Int32, y: Int32) -> Int32:
    if x > y:
        return 1
    return 0


fn eq_int32(x: Int32, y: Int32) -> Int32:
    if x == y:
        return 1
    return 0


fn ne_int32(x: Int32, y: Int32) -> Int32:
    if x != y:
        return 1
    return 0


fn clamp_int32(x: Int32, lo: Int32, hi: Int32) -> Int32:
    if x < lo:
        return lo
    if x > hi:
        return hi
    return x


fn bitnot_int32(x: Int32) -> Int32:
    return ~x


fn bitand_int32(x: Int32, y: Int32) -> Int32:
    return x & y


fn bitor_int32(x: Int32, y: Int32) -> Int32:
    return x | y


fn bitxor_int32(x: Int32, y: Int32) -> Int32:
    return x ^ y


fn gcd_int32(x: Int32, y: Int32) -> Int32:
    var a = x
    var b = y
    if a < 0:
        a = -a
    if b < 0:
        b = -b
    while b != 0:
        var tmp = b
        b = a % b
        a = tmp
    return a


fn fib_int32(n: Int32) -> Int32:
    if n <= 0:
        return 0
    if n == 1:
        return 1
    var a: Int32 = 0
    var b: Int32 = 1
    for _ in range(2, Int(n) + 1):
        var tmp = a + b
        a = b
        b = tmp
    return b


fn factorial_int32(n: Int32) -> Int32:
    if n <= 1:
        return 1
    var result: Int32 = 1
    for i in range(2, Int(n) + 1):
        result *= Int32(i)
    return result


fn factorial_int64(n: Int64) -> Int64:
    if n <= 1:
        return 1
    var result: Int64 = 1
    for i in range(2, Int(n) + 1):
        result *= Int64(i)
    return result


# ── Int32 boundary values — identity ─────────────────────────────────────────


fn test_identity_int32_max() raises:
    assert_equal(
        identity_int32(INT32_MAX),
        INT32_MAX,
        "identity_int32(INT32_MAX)",
    )


fn test_identity_int32_min() raises:
    assert_equal(
        identity_int32(INT32_MIN),
        INT32_MIN,
        "identity_int32(INT32_MIN)",
    )


fn test_identity_int32_zero() raises:
    assert_equal(identity_int32(0), Int32(0), "identity_int32(0)")


# ── Int64 boundary values — identity ─────────────────────────────────────────


fn test_identity_int64_max() raises:
    assert_equal(
        identity_int64(INT64_MAX),
        INT64_MAX,
        "identity_int64(INT64_MAX)",
    )


fn test_identity_int64_min() raises:
    assert_equal(
        identity_int64(INT64_MIN),
        INT64_MIN,
        "identity_int64(INT64_MIN)",
    )


# ── Int32 overflow — addition ────────────────────────────────────────────────


fn test_add_int32_max_plus_one_wraps() raises:
    assert_equal(
        add_int32(INT32_MAX, 1),
        INT32_MIN,
        "add_int32(INT32_MAX, 1) wraps to INT32_MIN",
    )


fn test_add_int32_min_minus_one_wraps() raises:
    assert_equal(
        add_int32(INT32_MIN, -1),
        INT32_MAX,
        "add_int32(INT32_MIN, -1) wraps to INT32_MAX",
    )


fn test_add_int32_max_plus_max_wraps() raises:
    assert_equal(
        add_int32(INT32_MAX, INT32_MAX),
        Int32(-2),
        "add_int32(INT32_MAX, INT32_MAX) wraps to -2",
    )


# ── Int64 overflow — addition ────────────────────────────────────────────────


fn test_add_int64_max_plus_one_wraps() raises:
    assert_equal(
        add_int64(INT64_MAX, 1),
        INT64_MIN,
        "add_int64(INT64_MAX, 1) wraps to INT64_MIN",
    )


fn test_add_int64_min_minus_one_wraps() raises:
    assert_equal(
        add_int64(INT64_MIN, -1),
        INT64_MAX,
        "add_int64(INT64_MIN, -1) wraps to INT64_MAX",
    )


# ── Int32 overflow — subtraction ─────────────────────────────────────────────


fn test_sub_int32_min_minus_one_wraps() raises:
    assert_equal(
        sub_int32(INT32_MIN, 1),
        INT32_MAX,
        "sub_int32(INT32_MIN, 1) wraps to INT32_MAX",
    )


fn test_sub_int32_max_minus_neg_one_wraps() raises:
    assert_equal(
        sub_int32(INT32_MAX, -1),
        INT32_MIN,
        "sub_int32(INT32_MAX, -1) wraps to INT32_MIN",
    )


# ── Int32 overflow — multiplication ──────────────────────────────────────────


fn test_mul_int32_max_times_two_wraps() raises:
    assert_equal(
        mul_int32(INT32_MAX, 2),
        Int32(-2),
        "mul_int32(INT32_MAX, 2) wraps to -2",
    )


fn test_mul_int32_min_times_neg_one_wraps() raises:
    assert_equal(
        mul_int32(INT32_MIN, -1),
        INT32_MIN,
        "mul_int32(INT32_MIN, -1) wraps to INT32_MIN (no positive equivalent)",
    )


# ── Int32 overflow — negation ────────────────────────────────────────────────


fn test_neg_int32_max() raises:
    assert_equal(
        neg_int32(INT32_MAX),
        -INT32_MAX,
        "neg_int32(INT32_MAX) === -INT32_MAX",
    )


fn test_neg_int32_min_wraps() raises:
    assert_equal(
        neg_int32(INT32_MIN),
        INT32_MIN,
        "neg_int32(INT32_MIN) wraps to INT32_MIN (2's complement)",
    )


# ── Int64 overflow — negation ────────────────────────────────────────────────


fn test_neg_int64_max() raises:
    assert_equal(
        neg_int64(INT64_MAX),
        -INT64_MAX,
        "neg_int64(INT64_MAX) === -INT64_MAX",
    )


fn test_neg_int64_min_wraps() raises:
    assert_equal(
        neg_int64(INT64_MIN),
        INT64_MIN,
        "neg_int64(INT64_MIN) wraps to INT64_MIN (2's complement)",
    )


# ── Int32 boundary — abs ─────────────────────────────────────────────────────


fn test_abs_int32_max() raises:
    assert_equal(
        abs_int32(INT32_MAX),
        INT32_MAX,
        "abs_int32(INT32_MAX) === INT32_MAX",
    )


fn test_abs_int32_min_wraps() raises:
    assert_equal(
        abs_int32(INT32_MIN),
        INT32_MIN,
        "abs_int32(INT32_MIN) wraps to INT32_MIN (no positive equivalent)",
    )


fn test_abs_int32_min_plus_one() raises:
    assert_equal(
        abs_int32(INT32_MIN + 1),
        INT32_MAX,
        "abs_int32(INT32_MIN + 1) === INT32_MAX",
    )


# ── Int32 boundary — min / max ───────────────────────────────────────────────


fn test_min_int32_boundaries() raises:
    assert_equal(
        min_int32(INT32_MIN, INT32_MAX),
        INT32_MIN,
        "min_int32(INT32_MIN, INT32_MAX) === INT32_MIN",
    )


fn test_max_int32_boundaries() raises:
    assert_equal(
        max_int32(INT32_MIN, INT32_MAX),
        INT32_MAX,
        "max_int32(INT32_MIN, INT32_MAX) === INT32_MAX",
    )


fn test_min_int32_same_min() raises:
    assert_equal(
        min_int32(INT32_MIN, INT32_MIN),
        INT32_MIN,
        "min_int32(INT32_MIN, INT32_MIN) === INT32_MIN",
    )


fn test_max_int32_same_max() raises:
    assert_equal(
        max_int32(INT32_MAX, INT32_MAX),
        INT32_MAX,
        "max_int32(INT32_MAX, INT32_MAX) === INT32_MAX",
    )


# ── Int32 boundary — comparison ──────────────────────────────────────────────


fn test_lt_int32_min_max() raises:
    assert_equal(
        lt_int32(INT32_MIN, INT32_MAX),
        Int32(1),
        "INT32_MIN < INT32_MAX",
    )


fn test_gt_int32_max_min() raises:
    assert_equal(
        gt_int32(INT32_MAX, INT32_MIN),
        Int32(1),
        "INT32_MAX > INT32_MIN",
    )


fn test_eq_int32_max_max() raises:
    assert_equal(
        eq_int32(INT32_MAX, INT32_MAX),
        Int32(1),
        "INT32_MAX === INT32_MAX",
    )


fn test_eq_int32_min_min() raises:
    assert_equal(
        eq_int32(INT32_MIN, INT32_MIN),
        Int32(1),
        "INT32_MIN === INT32_MIN",
    )


fn test_ne_int32_min_max() raises:
    assert_equal(
        ne_int32(INT32_MIN, INT32_MAX),
        Int32(1),
        "INT32_MIN !== INT32_MAX",
    )


# ── Int32 boundary — clamp ───────────────────────────────────────────────────


fn test_clamp_int32_min_to_range() raises:
    assert_equal(
        clamp_int32(INT32_MIN, 0, 100),
        Int32(0),
        "clamp_int32(INT32_MIN, 0, 100) === 0",
    )


fn test_clamp_int32_max_to_range() raises:
    assert_equal(
        clamp_int32(INT32_MAX, 0, 100),
        Int32(100),
        "clamp_int32(INT32_MAX, 0, 100) === 100",
    )


fn test_clamp_int32_within_full_range() raises:
    assert_equal(
        clamp_int32(50, INT32_MIN, INT32_MAX),
        Int32(50),
        "clamp_int32(50, INT32_MIN, INT32_MAX) === 50",
    )


# ── Int32 boundary — bitwise ─────────────────────────────────────────────────


fn test_bitnot_int32_max() raises:
    assert_equal(
        bitnot_int32(INT32_MAX),
        INT32_MIN,
        "bitnot_int32(INT32_MAX) === INT32_MIN",
    )


fn test_bitnot_int32_min() raises:
    assert_equal(
        bitnot_int32(INT32_MIN),
        INT32_MAX,
        "bitnot_int32(INT32_MIN) === INT32_MAX",
    )


fn test_bitand_int32_max_min() raises:
    assert_equal(
        bitand_int32(INT32_MAX, INT32_MIN),
        Int32(0),
        "bitand_int32(INT32_MAX, INT32_MIN) === 0",
    )


fn test_bitor_int32_max_min() raises:
    assert_equal(
        bitor_int32(INT32_MAX, INT32_MIN),
        Int32(-1),
        "bitor_int32(INT32_MAX, INT32_MIN) === -1 (all bits set)",
    )


fn test_bitxor_int32_max_min() raises:
    assert_equal(
        bitxor_int32(INT32_MAX, INT32_MIN),
        Int32(-1),
        "bitxor_int32(INT32_MAX, INT32_MIN) === -1",
    )


# ── Int32 boundary — GCD ─────────────────────────────────────────────────────


fn test_gcd_int32_max_with_one() raises:
    assert_equal(
        gcd_int32(INT32_MAX, 1),
        Int32(1),
        "gcd_int32(INT32_MAX, 1) === 1",
    )


fn test_gcd_int32_max_with_self() raises:
    assert_equal(
        gcd_int32(INT32_MAX, INT32_MAX),
        INT32_MAX,
        "gcd_int32(INT32_MAX, INT32_MAX) === INT32_MAX",
    )


# ── Int32 overflow — factorial ───────────────────────────────────────────────


fn test_factorial_int32_12_fits() raises:
    # 12! = 479001600, fits in Int32
    assert_equal(
        factorial_int32(12),
        Int32(479001600),
        "factorial_int32(12) === 479001600",
    )


fn test_factorial_int32_13_overflows() raises:
    # 13! = 6227020800, overflows Int32 — verify it wraps
    assert_equal(
        factorial_int32(13),
        Int32(1932053504),
        "factorial_int32(13) wraps (6227020800 truncated to i32)",
    )


# ── Int64 boundary — factorial ───────────────────────────────────────────────


fn test_factorial_int64_20_fits() raises:
    # 20! = 2432902008176640000, fits in Int64
    assert_equal(
        factorial_int64(20),
        Int64(2432902008176640000),
        "factorial_int64(20) === 2432902008176640000",
    )


fn test_factorial_int64_21_overflows() raises:
    # 21! = 51090942171709440000, overflows Int64 — verify it wraps
    assert_equal(
        factorial_int64(21),
        Int64(-4249290049419214848),
        "factorial_int64(21) wraps (overflow)",
    )


# ── Int32 overflow — fibonacci ───────────────────────────────────────────────


fn test_fib_int32_46_fits() raises:
    # fib(46) = 1836311903, fits in Int32
    assert_equal(
        fib_int32(46),
        Int32(1836311903),
        "fib_int32(46) === 1836311903",
    )


fn test_fib_int32_47_overflows() raises:
    # fib(47) = 2971215073, overflows Int32 — verify wrapping
    assert_equal(
        fib_int32(47),
        Int32(-1323752223),
        "fib_int32(47) wraps (2971215073 truncated to i32)",
    )

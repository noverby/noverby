# Tests for algorithms (fib, factorial, gcd) — native Mojo tests run with `mojo test`.
#
# These tests are a direct port of test/algorithms.test.ts, exercising
# the same scenarios without the WASM/JS round-trip.  The functions
# under test are defined inline here (matching main.mojo's @export fns).
#
# Run with:
#   mojo test -I src test-mojo/test_algorithms.mojo

from testing import assert_equal


# ── Functions under test (mirrors of main.mojo @export fns) ──────────────────


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


fn fib_int64(n: Int64) -> Int64:
    if n <= 0:
        return 0
    if n == 1:
        return 1
    var a: Int64 = 0
    var b: Int64 = 1
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


# ── Fibonacci — int32 ────────────────────────────────────────────────────────


fn test_fib_int32_zero() raises:
    assert_equal(fib_int32(0), Int32(0), "fib_int32(0) === 0")


fn test_fib_int32_one() raises:
    assert_equal(fib_int32(1), Int32(1), "fib_int32(1) === 1")


fn test_fib_int32_two() raises:
    assert_equal(fib_int32(2), Int32(1), "fib_int32(2) === 1")


fn test_fib_int32_three() raises:
    assert_equal(fib_int32(3), Int32(2), "fib_int32(3) === 2")


fn test_fib_int32_four() raises:
    assert_equal(fib_int32(4), Int32(3), "fib_int32(4) === 3")


fn test_fib_int32_five() raises:
    assert_equal(fib_int32(5), Int32(5), "fib_int32(5) === 5")


fn test_fib_int32_six() raises:
    assert_equal(fib_int32(6), Int32(8), "fib_int32(6) === 8")


fn test_fib_int32_seven() raises:
    assert_equal(fib_int32(7), Int32(13), "fib_int32(7) === 13")


fn test_fib_int32_ten() raises:
    assert_equal(fib_int32(10), Int32(55), "fib_int32(10) === 55")


fn test_fib_int32_twenty() raises:
    assert_equal(fib_int32(20), Int32(6765), "fib_int32(20) === 6765")


# ── Fibonacci — int64 ────────────────────────────────────────────────────────


fn test_fib_int64_zero() raises:
    assert_equal(fib_int64(0), Int64(0), "fib_int64(0) === 0")


fn test_fib_int64_one() raises:
    assert_equal(fib_int64(1), Int64(1), "fib_int64(1) === 1")


fn test_fib_int64_ten() raises:
    assert_equal(fib_int64(10), Int64(55), "fib_int64(10) === 55")


fn test_fib_int64_twenty() raises:
    assert_equal(fib_int64(20), Int64(6765), "fib_int64(20) === 6765")


fn test_fib_int64_fifty() raises:
    assert_equal(
        fib_int64(50),
        Int64(12586269025),
        "fib_int64(50) === 12586269025",
    )


# ── Factorial — int32 ────────────────────────────────────────────────────────


fn test_factorial_int32_zero() raises:
    assert_equal(factorial_int32(0), Int32(1), "factorial_int32(0) === 1")


fn test_factorial_int32_one() raises:
    assert_equal(factorial_int32(1), Int32(1), "factorial_int32(1) === 1")


fn test_factorial_int32_two() raises:
    assert_equal(factorial_int32(2), Int32(2), "factorial_int32(2) === 2")


fn test_factorial_int32_three() raises:
    assert_equal(factorial_int32(3), Int32(6), "factorial_int32(3) === 6")


fn test_factorial_int32_four() raises:
    assert_equal(factorial_int32(4), Int32(24), "factorial_int32(4) === 24")


fn test_factorial_int32_five() raises:
    assert_equal(factorial_int32(5), Int32(120), "factorial_int32(5) === 120")


fn test_factorial_int32_ten() raises:
    assert_equal(
        factorial_int32(10),
        Int32(3628800),
        "factorial_int32(10) === 3628800",
    )


# ── Factorial — int64 ────────────────────────────────────────────────────────


fn test_factorial_int64_zero() raises:
    assert_equal(factorial_int64(0), Int64(1), "factorial_int64(0) === 1")


fn test_factorial_int64_one() raises:
    assert_equal(factorial_int64(1), Int64(1), "factorial_int64(1) === 1")


fn test_factorial_int64_five() raises:
    assert_equal(factorial_int64(5), Int64(120), "factorial_int64(5) === 120")


fn test_factorial_int64_ten() raises:
    assert_equal(
        factorial_int64(10),
        Int64(3628800),
        "factorial_int64(10) === 3628800",
    )


fn test_factorial_int64_twenty() raises:
    assert_equal(
        factorial_int64(20),
        Int64(2432902008176640000),
        "factorial_int64(20) === 2432902008176640000",
    )


# ── GCD (Euclidean algorithm) ────────────────────────────────────────────────


fn test_gcd_12_8() raises:
    assert_equal(gcd_int32(12, 8), Int32(4), "gcd_int32(12, 8) === 4")


fn test_gcd_commutative() raises:
    assert_equal(
        gcd_int32(8, 12),
        Int32(4),
        "gcd_int32(8, 12) === 4 (commutative)",
    )


fn test_gcd_coprime() raises:
    assert_equal(gcd_int32(7, 13), Int32(1), "gcd_int32(7, 13) === 1 (coprime)")


fn test_gcd_100_75() raises:
    assert_equal(gcd_int32(100, 75), Int32(25), "gcd_int32(100, 75) === 25")


fn test_gcd_zero_first() raises:
    assert_equal(gcd_int32(0, 5), Int32(5), "gcd_int32(0, 5) === 5")


fn test_gcd_zero_second() raises:
    assert_equal(gcd_int32(5, 0), Int32(5), "gcd_int32(5, 0) === 5")


fn test_gcd_same() raises:
    assert_equal(gcd_int32(7, 7), Int32(7), "gcd_int32(7, 7) === 7")


fn test_gcd_one() raises:
    assert_equal(gcd_int32(1, 100), Int32(1), "gcd_int32(1, 100) === 1")


fn test_gcd_negative_first() raises:
    assert_equal(
        gcd_int32(-12, 8),
        Int32(4),
        "gcd_int32(-12, 8) === 4 (negative input)",
    )


fn test_gcd_negative_second() raises:
    assert_equal(
        gcd_int32(12, -8),
        Int32(4),
        "gcd_int32(12, -8) === 4 (negative input)",
    )


fn test_gcd_both_negative() raises:
    assert_equal(
        gcd_int32(-12, -8),
        Int32(4),
        "gcd_int32(-12, -8) === 4 (both negative)",
    )


fn test_gcd_48_18() raises:
    assert_equal(gcd_int32(48, 18), Int32(6), "gcd_int32(48, 18) === 6")


fn test_gcd_classic_euclid() raises:
    assert_equal(
        gcd_int32(1071, 462),
        Int32(21),
        "gcd_int32(1071, 462) === 21 (classic Euclid example)",
    )


# ── Fibonacci recurrence property ────────────────────────────────────────────
# fib(n) === fib(n-1) + fib(n-2) for n >= 2


fn test_fib_recurrence_property() raises:
    for n in range(2, 21):
        var fn0 = fib_int32(Int32(n))
        var fn1 = fib_int32(Int32(n - 1))
        var fn2 = fib_int32(Int32(n - 2))
        assert_equal(
            fn0,
            fn1 + fn2,
            String("fib(")
            + String(n)
            + ") === fib("
            + String(n - 1)
            + ") + fib("
            + String(n - 2)
            + ")",
        )


# ── Factorial recurrence property ────────────────────────────────────────────
# n! === n * (n-1)! for n >= 2


fn test_factorial_recurrence_property() raises:
    for n in range(2, 11):
        var fn0 = factorial_int32(Int32(n))
        var fn1 = factorial_int32(Int32(n - 1))
        assert_equal(
            fn0,
            Int32(n) * fn1,
            String("factorial(")
            + String(n)
            + ") === "
            + String(n)
            + " * factorial("
            + String(n - 1)
            + ")",
        )


# ── GCD properties ───────────────────────────────────────────────────────────


fn test_gcd_commutative_property() raises:
    """gcd(a, b) === gcd(b, a) for several pairs."""
    var pairs = List[Tuple[Int32, Int32]]()
    pairs.append((Int32(12), Int32(8)))
    pairs.append((Int32(7), Int32(13)))
    pairs.append((Int32(100), Int32(75)))
    pairs.append((Int32(0), Int32(5)))
    pairs.append((Int32(1071), Int32(462)))
    for i in range(len(pairs)):
        var a = pairs[i][0]
        var b = pairs[i][1]
        assert_equal(
            gcd_int32(a, b),
            gcd_int32(b, a),
            String("gcd(") + String(a) + ", " + String(b) + ") commutes",
        )


fn test_gcd_idempotent() raises:
    """gcd(a, a) === a for positive values."""
    for v in range(1, 20):
        var a = Int32(v)
        assert_equal(
            gcd_int32(a, a),
            a,
            String("gcd(")
            + String(a)
            + ", "
            + String(a)
            + ") === "
            + String(a),
        )


fn test_gcd_with_one() raises:
    """gcd(1, n) === 1 for any positive n."""
    for v in range(1, 20):
        var n = Int32(v)
        assert_equal(
            gcd_int32(1, n),
            Int32(1),
            String("gcd(1, ") + String(n) + ") === 1",
        )

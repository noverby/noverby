# Algorithm tests (fib, factorial, gcd) exercised through the real WASM binary
# via wasmtime-py (called from Mojo via Python interop).
#
# These tests verify that fibonacci, factorial, and GCD algorithms work correctly
# when compiled to WASM and executed via the Wasmtime runtime.
#
# Run with:
#   mojo test test/test_algorithms.mojo

from python import Python, PythonObject
from testing import assert_equal, assert_true


fn _get_wasm() raises -> PythonObject:
    Python.add_to_path("test")
    var harness = Python.import_module("wasm_harness")
    return harness.get_instance()


# ── Fibonacci — int32 ────────────────────────────────────────────────────────


fn test_fib_int32_zero() raises:
    var w = _get_wasm()
    assert_equal(Int(w.fib_int32(0)), 0, "fib_int32(0) === 0")


fn test_fib_int32_one() raises:
    var w = _get_wasm()
    assert_equal(Int(w.fib_int32(1)), 1, "fib_int32(1) === 1")


fn test_fib_int32_two() raises:
    var w = _get_wasm()
    assert_equal(Int(w.fib_int32(2)), 1, "fib_int32(2) === 1")


fn test_fib_int32_three() raises:
    var w = _get_wasm()
    assert_equal(Int(w.fib_int32(3)), 2, "fib_int32(3) === 2")


fn test_fib_int32_four() raises:
    var w = _get_wasm()
    assert_equal(Int(w.fib_int32(4)), 3, "fib_int32(4) === 3")


fn test_fib_int32_five() raises:
    var w = _get_wasm()
    assert_equal(Int(w.fib_int32(5)), 5, "fib_int32(5) === 5")


fn test_fib_int32_six() raises:
    var w = _get_wasm()
    assert_equal(Int(w.fib_int32(6)), 8, "fib_int32(6) === 8")


fn test_fib_int32_seven() raises:
    var w = _get_wasm()
    assert_equal(Int(w.fib_int32(7)), 13, "fib_int32(7) === 13")


fn test_fib_int32_ten() raises:
    var w = _get_wasm()
    assert_equal(Int(w.fib_int32(10)), 55, "fib_int32(10) === 55")


fn test_fib_int32_twenty() raises:
    var w = _get_wasm()
    assert_equal(Int(w.fib_int32(20)), 6765, "fib_int32(20) === 6765")


# ── Fibonacci — int64 ────────────────────────────────────────────────────────


fn test_fib_int64_zero() raises:
    var w = _get_wasm()
    assert_equal(Int(w.fib_int64(0)), 0, "fib_int64(0) === 0")


fn test_fib_int64_one() raises:
    var w = _get_wasm()
    assert_equal(Int(w.fib_int64(1)), 1, "fib_int64(1) === 1")


fn test_fib_int64_ten() raises:
    var w = _get_wasm()
    assert_equal(Int(w.fib_int64(10)), 55, "fib_int64(10) === 55")


fn test_fib_int64_twenty() raises:
    var w = _get_wasm()
    assert_equal(Int(w.fib_int64(20)), 6765, "fib_int64(20) === 6765")


fn test_fib_int64_fifty() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w.fib_int64(50)),
        12586269025,
        "fib_int64(50) === 12586269025",
    )


# ── Factorial — int32 ────────────────────────────────────────────────────────


fn test_factorial_int32_zero() raises:
    var w = _get_wasm()
    assert_equal(Int(w.factorial_int32(0)), 1, "factorial_int32(0) === 1")


fn test_factorial_int32_one() raises:
    var w = _get_wasm()
    assert_equal(Int(w.factorial_int32(1)), 1, "factorial_int32(1) === 1")


fn test_factorial_int32_two() raises:
    var w = _get_wasm()
    assert_equal(Int(w.factorial_int32(2)), 2, "factorial_int32(2) === 2")


fn test_factorial_int32_three() raises:
    var w = _get_wasm()
    assert_equal(Int(w.factorial_int32(3)), 6, "factorial_int32(3) === 6")


fn test_factorial_int32_four() raises:
    var w = _get_wasm()
    assert_equal(Int(w.factorial_int32(4)), 24, "factorial_int32(4) === 24")


fn test_factorial_int32_five() raises:
    var w = _get_wasm()
    assert_equal(Int(w.factorial_int32(5)), 120, "factorial_int32(5) === 120")


fn test_factorial_int32_ten() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w.factorial_int32(10)),
        3628800,
        "factorial_int32(10) === 3628800",
    )


# ── Factorial — int64 ────────────────────────────────────────────────────────


fn test_factorial_int64_zero() raises:
    var w = _get_wasm()
    assert_equal(Int(w.factorial_int64(0)), 1, "factorial_int64(0) === 1")


fn test_factorial_int64_one() raises:
    var w = _get_wasm()
    assert_equal(Int(w.factorial_int64(1)), 1, "factorial_int64(1) === 1")


fn test_factorial_int64_five() raises:
    var w = _get_wasm()
    assert_equal(Int(w.factorial_int64(5)), 120, "factorial_int64(5) === 120")


fn test_factorial_int64_ten() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w.factorial_int64(10)),
        3628800,
        "factorial_int64(10) === 3628800",
    )


fn test_factorial_int64_twenty() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w.factorial_int64(20)),
        2432902008176640000,
        "factorial_int64(20) === 2432902008176640000",
    )


# ── GCD (Euclidean algorithm) ────────────────────────────────────────────────


fn test_gcd_12_8() raises:
    var w = _get_wasm()
    assert_equal(Int(w.gcd_int32(12, 8)), 4, "gcd_int32(12, 8) === 4")


fn test_gcd_commutative() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w.gcd_int32(8, 12)),
        4,
        "gcd_int32(8, 12) === 4 (commutative)",
    )


fn test_gcd_coprime() raises:
    var w = _get_wasm()
    assert_equal(Int(w.gcd_int32(7, 13)), 1, "gcd_int32(7, 13) === 1 (coprime)")


fn test_gcd_100_75() raises:
    var w = _get_wasm()
    assert_equal(Int(w.gcd_int32(100, 75)), 25, "gcd_int32(100, 75) === 25")


fn test_gcd_zero_first() raises:
    var w = _get_wasm()
    assert_equal(Int(w.gcd_int32(0, 5)), 5, "gcd_int32(0, 5) === 5")


fn test_gcd_zero_second() raises:
    var w = _get_wasm()
    assert_equal(Int(w.gcd_int32(5, 0)), 5, "gcd_int32(5, 0) === 5")


fn test_gcd_same() raises:
    var w = _get_wasm()
    assert_equal(Int(w.gcd_int32(7, 7)), 7, "gcd_int32(7, 7) === 7")


fn test_gcd_one() raises:
    var w = _get_wasm()
    assert_equal(Int(w.gcd_int32(1, 100)), 1, "gcd_int32(1, 100) === 1")


fn test_gcd_negative_first() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w.gcd_int32(-12, 8)),
        4,
        "gcd_int32(-12, 8) === 4 (negative input)",
    )


fn test_gcd_negative_second() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w.gcd_int32(12, -8)),
        4,
        "gcd_int32(12, -8) === 4 (negative input)",
    )


fn test_gcd_both_negative() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w.gcd_int32(-12, -8)),
        4,
        "gcd_int32(-12, -8) === 4 (both negative)",
    )


fn test_gcd_48_18() raises:
    var w = _get_wasm()
    assert_equal(Int(w.gcd_int32(48, 18)), 6, "gcd_int32(48, 18) === 6")


fn test_gcd_classic_euclid() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w.gcd_int32(1071, 462)),
        21,
        "gcd_int32(1071, 462) === 21 (classic Euclid example)",
    )


# ── Fibonacci recurrence property ────────────────────────────────────────────
# fib(n) === fib(n-1) + fib(n-2) for n >= 2


fn test_fib_recurrence_property() raises:
    var w = _get_wasm()
    for n in range(2, 21):
        var fn0 = Int(w.fib_int32(n))
        var fn1 = Int(w.fib_int32(n - 1))
        var fn2 = Int(w.fib_int32(n - 2))
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
    var w = _get_wasm()
    for n in range(2, 11):
        var fn0 = Int(w.factorial_int32(n))
        var fn1 = Int(w.factorial_int32(n - 1))
        assert_equal(
            fn0,
            n * fn1,
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
    var w = _get_wasm()
    var as_ = List[Int](12, 7, 100, 0, 1071)
    var bs = List[Int](8, 13, 75, 5, 462)
    for i in range(len(as_)):
        var a = as_[i]
        var b = bs[i]
        assert_equal(
            Int(w.gcd_int32(a, b)),
            Int(w.gcd_int32(b, a)),
            String("gcd(") + String(a) + ", " + String(b) + ") commutes",
        )


fn test_gcd_idempotent() raises:
    """gcd(a, a) === a for positive values."""
    var w = _get_wasm()
    for v in range(1, 20):
        assert_equal(
            Int(w.gcd_int32(v, v)),
            v,
            String("gcd(")
            + String(v)
            + ", "
            + String(v)
            + ") === "
            + String(v),
        )


fn test_gcd_with_one() raises:
    """gcd(1, n) === 1 for any positive n."""
    var w = _get_wasm()
    for v in range(1, 20):
        assert_equal(
            Int(w.gcd_int32(1, v)),
            1,
            String("gcd(1, ") + String(v) + ") === 1",
        )

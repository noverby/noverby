# Port of test/consistency.test.ts — cross-function consistency checks exercised
# through the real WASM binary via wasmtime-py (called from Mojo via Python interop).
#
# These tests verify that different WASM-exported functions compose correctly:
# add/sub inverses, mul/div inverses, neg/abs relationships, min/max ordering,
# bitwise identities, shift roundtrips, De Morgan's law, GCD scaling,
# Fibonacci/factorial recurrence, string concat length, and clamp equivalence.
#
# Run with:
#   mojo test test-wasm/test_consistency.mojo

from python import Python, PythonObject
from testing import assert_true, assert_equal


fn _get_wasm() raises -> PythonObject:
    Python.add_to_path("test-wasm")
    var harness = Python.import_module("wasm_harness")
    return harness.get_instance()


# ---------------------------------------------------------------------------
# Helper — pure-Mojo GCD for reference
# ---------------------------------------------------------------------------


fn _gcd(var a: Int, var b: Int) -> Int:
    if a < 0:
        a = -a
    if b < 0:
        b = -b
    while b != 0:
        var t = b
        b = a % b
        a = t
    return a


# ---------------------------------------------------------------------------
# Cross-function consistency checks
# ---------------------------------------------------------------------------


fn test_add_sub_inverse() raises:
    """sub(add(x, y), y) === x (add/sub inverse)."""
    var w = _get_wasm()
    var x = 17
    var y = 9
    var s = w.add_int32(x, y)
    assert_equal(Int(w.sub_int32(s, y)), x, "sub(add(x, y), y) === x")


fn test_mul_div_inverse() raises:
    """div(mul(x, y), y) === x (mul/div inverse for exact division)."""
    var w = _get_wasm()
    var x = 6
    var y = 3
    var product = w.mul_int32(x, y)
    assert_equal(Int(w.div_int32(product, y)), x, "div(mul(x, y), y) === x")


fn test_neg_neg_identity() raises:
    """neg(neg(x)) === x."""
    var w = _get_wasm()
    assert_equal(Int(w.neg_int32(w.neg_int32(42))), 42, "neg(neg(42)) === 42")


fn test_abs_neg_eq_abs() raises:
    """abs(neg(x)) === abs(x) for positive x."""
    var w = _get_wasm()
    assert_equal(
        Int(w.abs_int32(w.neg_int32(7))),
        Int(w.abs_int32(7)),
        "abs(neg(7)) === abs(7)",
    )


fn test_min_le_max() raises:
    """min(x, y) <= max(x, y)."""
    var w = _get_wasm()
    var a = 3
    var b = 7
    var lo = w.min_int32(a, b)
    var hi = w.max_int32(a, b)
    assert_equal(Int(w.le_int32(lo, hi)), 1, "min(x,y) <= max(x,y)")


fn test_bitwise_identity_and_or_xor() raises:
    """(x & y) | (x ^ y) === x | y."""
    var w = _get_wasm()
    var x = 0b1100
    var y = 0b1010
    var lhs = Int(w.bitor_int32(w.bitand_int32(x, y), w.bitxor_int32(x, y)))
    var rhs = Int(w.bitor_int32(x, y))
    assert_equal(lhs, rhs, "(x & y) | (x ^ y) === x | y")


fn test_shl_shr_roundtrip() raises:
    """shr(shl(x, 4), 4) === x."""
    var w = _get_wasm()
    var x = 5
    assert_equal(
        Int(w.shr_int32(w.shl_int32(x, 4), 4)),
        x,
        "shr(shl(x, 4), 4) === x",
    )


fn test_de_morgan() raises:
    """De Morgan: not(and(a,b)) === or(not(a), not(b))."""
    var w = _get_wasm()
    var a = 1
    var b = 0
    var lhs = Int(w.bool_not(w.bool_and(a, b)))
    var rhs = Int(w.bool_or(w.bool_not(a), w.bool_not(b)))
    assert_equal(lhs, rhs, "De Morgan: not(and(a,b)) === or(not(a), not(b))")


fn test_gcd_scaling() raises:
    """gcd(a*k, b*k) === k * gcd(a, b)."""
    var w = _get_wasm()
    var a = 6
    var b = 4
    var k = 5
    var lhs = Int(w.gcd_int32(w.mul_int32(a, k), w.mul_int32(b, k)))
    var rhs = Int(w.mul_int32(k, w.gcd_int32(a, b)))
    assert_equal(lhs, rhs, "gcd(a*k, b*k) === k * gcd(a, b)")


fn test_fibonacci_recurrence() raises:
    """fib(n) === fib(n-1) + fib(n-2) for several values of n."""
    var w = _get_wasm()
    var ns = List[Int](5, 8, 12, 15)
    for i in range(len(ns)):
        var n = ns[i]
        var fn2 = Int(w.fib_int32(n - 2))
        var fn1 = Int(w.fib_int32(n - 1))
        var fn0 = Int(w.fib_int32(n))
        assert_equal(
            fn0,
            fn1 + fn2,
            String("fib(") + String(n) + ") === fib(n-1) + fib(n-2)",
        )


fn test_factorial_recurrence() raises:
    """factorial(n) === n * factorial(n-1) for n = 2..7."""
    var w = _get_wasm()
    var ns = List[Int](2, 3, 4, 5, 6, 7)
    for i in range(len(ns)):
        var n = ns[i]
        var fn0 = Int(w.factorial_int32(n))
        var fn1 = Int(w.factorial_int32(n - 1))
        assert_equal(
            fn0,
            n * fn1,
            String("factorial(") + String(n) + ") === n * factorial(n-1)",
        )


fn test_string_concat_length() raises:
    """len(concat(a, b)) === len(a) + len(b)."""
    var w = _get_wasm()
    var a_ptr = w.write_string_struct("foo")
    var b_ptr = w.write_string_struct("barbaz")
    var out_ptr = w.alloc_string_struct()
    _ = w.string_concat(a_ptr, b_ptr, out_ptr)
    var concat_len = Int(w.string_length(out_ptr))
    var a_len = Int(w.string_length(a_ptr))
    var b_len = Int(w.string_length(b_ptr))
    assert_equal(
        concat_len, a_len + b_len, "len(concat(a,b)) === len(a) + len(b)"
    )


fn test_clamp_eq_max_lo_min_hi_x() raises:
    """clamp(x, lo, hi) === max(lo, min(hi, x)) for several values of x."""
    var w = _get_wasm()
    var lo = 0
    var hi = 10
    var xs = List[Int](-5, 0, 5, 10, 15)
    for i in range(len(xs)):
        var x = xs[i]
        var lhs = Int(w.clamp_int32(x, lo, hi))
        var rhs = Int(w.max_int32(lo, w.min_int32(hi, x)))
        assert_equal(
            lhs,
            rhs,
            String("clamp(")
            + String(x)
            + ", "
            + String(lo)
            + ", "
            + String(hi)
            + ") === max(lo, min(hi, x))",
        )

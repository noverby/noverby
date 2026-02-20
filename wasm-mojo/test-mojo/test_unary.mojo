# Tests for unary operations — native Mojo tests run with `mojo test`.
#
# These tests are a direct port of test/unary.test.ts, exercising
# the same scenarios without the WASM/JS round-trip.  The functions
# under test are defined inline here (matching main.mojo's @export fns).
#
# Run with:
#   mojo test -I src test-mojo/test_unary.mojo

from testing import assert_equal, assert_true
from math import nan, isnan


# ── Functions under test (mirrors of main.mojo @export fns) ──────────────────


fn neg_int32(x: Int32) -> Int32:
    return -x


fn neg_int64(x: Int64) -> Int64:
    return -x


fn neg_float32(x: Float32) -> Float32:
    return -x


fn neg_float64(x: Float64) -> Float64:
    return -x


fn abs_int32(x: Int32) -> Int32:
    if x < 0:
        return -x
    return x


fn abs_int64(x: Int64) -> Int64:
    if x < 0:
        return -x
    return x


fn abs_float32(x: Float32) -> Float32:
    if x < 0:
        return -x
    return x


fn abs_float64(x: Float64) -> Float64:
    if x < 0:
        return -x
    return x


# ── Negate — int32 ───────────────────────────────────────────────────────────


fn test_neg_int32_positive() raises:
    assert_equal(neg_int32(5), Int32(-5), "neg_int32(5) === -5")


fn test_neg_int32_negative() raises:
    assert_equal(neg_int32(-5), Int32(5), "neg_int32(-5) === 5")


fn test_neg_int32_zero() raises:
    assert_equal(neg_int32(0), Int32(0), "neg_int32(0) === 0")


# ── Negate — int64 ───────────────────────────────────────────────────────────


fn test_neg_int64_positive() raises:
    assert_equal(neg_int64(42), Int64(-42), "neg_int64(42) === -42")


fn test_neg_int64_negative() raises:
    assert_equal(neg_int64(-42), Int64(42), "neg_int64(-42) === 42")


fn test_neg_int64_zero() raises:
    assert_equal(neg_int64(0), Int64(0), "neg_int64(0) === 0")


# ── Negate — float32 ────────────────────────────────────────────────────────


fn test_neg_float32() raises:
    var expected = -Float32(3.14)
    assert_equal(neg_float32(3.14), expected, "neg_float32(3.14)")


# ── Negate — float64 ────────────────────────────────────────────────────────


fn test_neg_float64_positive() raises:
    assert_equal(neg_float64(3.14), -3.14, "neg_float64(3.14) === -3.14")


fn test_neg_float64_negative() raises:
    assert_equal(neg_float64(-3.14), 3.14, "neg_float64(-3.14) === 3.14")


fn test_neg_float64_zero() raises:
    # neg_float64(0.0) produces -0.0
    var result = neg_float64(0.0)
    # -0.0 == 0.0 in IEEE 754, but we can check the sign bit
    # via 1.0 / result == -inf
    assert_true(
        (1.0 / Float64(result)) < 0.0,
        "neg_float64(0) === -0 (negative zero)",
    )


# ── Absolute value — int32 ──────────────────────────────────────────────────


fn test_abs_int32_positive() raises:
    assert_equal(abs_int32(5), Int32(5), "abs_int32(5) === 5")


fn test_abs_int32_negative() raises:
    assert_equal(abs_int32(-5), Int32(5), "abs_int32(-5) === 5")


fn test_abs_int32_zero() raises:
    assert_equal(abs_int32(0), Int32(0), "abs_int32(0) === 0")


# ── Absolute value — int64 ──────────────────────────────────────────────────


fn test_abs_int64_positive() raises:
    assert_equal(abs_int64(99), Int64(99), "abs_int64(99) === 99")


fn test_abs_int64_negative() raises:
    assert_equal(abs_int64(-99), Int64(99), "abs_int64(-99) === 99")


fn test_abs_int64_zero() raises:
    assert_equal(abs_int64(0), Int64(0), "abs_int64(0) === 0")


# ── Absolute value — float32 ────────────────────────────────────────────────


fn test_abs_float32_positive() raises:
    var expected = Float32(2.5)
    assert_equal(abs_float32(2.5), expected, "abs_float32(2.5)")


fn test_abs_float32_negative() raises:
    var expected = Float32(2.5)
    assert_equal(abs_float32(-2.5), expected, "abs_float32(-2.5)")


# ── Absolute value — float64 ────────────────────────────────────────────────


fn test_abs_float64_positive() raises:
    assert_equal(abs_float64(3.14), 3.14, "abs_float64(3.14) === 3.14")


fn test_abs_float64_negative() raises:
    assert_equal(abs_float64(-3.14), 3.14, "abs_float64(-3.14) === 3.14")


fn test_abs_float64_zero() raises:
    assert_equal(abs_float64(0.0), 0.0, "abs_float64(0) === 0")

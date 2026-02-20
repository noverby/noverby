# Unary operations (neg, abs) exercised through the real WASM binary via
# wasmtime-py (called from Mojo via Python interop).
#
# These tests verify that negation and absolute value operations work correctly
# when compiled to WASM and executed via the Wasmtime runtime.
#
# Run with:
#   mojo test test-wasm/test_unary.mojo

from python import Python, PythonObject
from testing import assert_equal, assert_true


fn _get_wasm() raises -> PythonObject:
    Python.add_to_path("test-wasm")
    var harness = Python.import_module("wasm_harness")
    return harness.get_instance()


# ── Negate — int32 ───────────────────────────────────────────────────────────


fn test_neg_int32_positive() raises:
    var w = _get_wasm()
    assert_equal(Int(w.neg_int32(5)), -5, "neg_int32(5) === -5")


fn test_neg_int32_negative() raises:
    var w = _get_wasm()
    assert_equal(Int(w.neg_int32(-5)), 5, "neg_int32(-5) === 5")


fn test_neg_int32_zero() raises:
    var w = _get_wasm()
    assert_equal(Int(w.neg_int32(0)), 0, "neg_int32(0) === 0")


# ── Negate — int64 ───────────────────────────────────────────────────────────


fn test_neg_int64_positive() raises:
    var w = _get_wasm()
    assert_equal(Int(w.neg_int64(42)), -42, "neg_int64(42) === -42")


fn test_neg_int64_negative() raises:
    var w = _get_wasm()
    assert_equal(Int(w.neg_int64(-42)), 42, "neg_int64(-42) === 42")


fn test_neg_int64_zero() raises:
    var w = _get_wasm()
    assert_equal(Int(w.neg_int64(0)), 0, "neg_int64(0) === 0")


# ── Negate — float32 ────────────────────────────────────────────────────────


fn test_neg_float32() raises:
    var w = _get_wasm()
    var result = Float64(w.neg_float32(3.14))
    var expected = Float64(-Float32(3.14))
    assert_equal(result, expected, "neg_float32(3.14)")


# ── Negate — float64 ────────────────────────────────────────────────────────


fn test_neg_float64_positive() raises:
    var w = _get_wasm()
    assert_equal(
        Float64(w.neg_float64(3.14)), -3.14, "neg_float64(3.14) === -3.14"
    )


fn test_neg_float64_negative() raises:
    var w = _get_wasm()
    assert_equal(
        Float64(w.neg_float64(-3.14)), 3.14, "neg_float64(-3.14) === 3.14"
    )


fn test_neg_float64_zero() raises:
    var w = _get_wasm()
    # neg_float64(0.0) produces -0.0
    var math = Python.import_module("math")
    var result = w.neg_float64(0.0)
    # copysign(1.0, -0.0) == -1.0, so if sign is negative we got -0.0
    assert_true(
        Bool(math.copysign(1.0, result) < 0),
        "neg_float64(0) === -0 (negative zero)",
    )


# ── Absolute value — int32 ──────────────────────────────────────────────────


fn test_abs_int32_positive() raises:
    var w = _get_wasm()
    assert_equal(Int(w.abs_int32(5)), 5, "abs_int32(5) === 5")


fn test_abs_int32_negative() raises:
    var w = _get_wasm()
    assert_equal(Int(w.abs_int32(-5)), 5, "abs_int32(-5) === 5")


fn test_abs_int32_zero() raises:
    var w = _get_wasm()
    assert_equal(Int(w.abs_int32(0)), 0, "abs_int32(0) === 0")


# ── Absolute value — int64 ──────────────────────────────────────────────────


fn test_abs_int64_positive() raises:
    var w = _get_wasm()
    assert_equal(Int(w.abs_int64(99)), 99, "abs_int64(99) === 99")


fn test_abs_int64_negative() raises:
    var w = _get_wasm()
    assert_equal(Int(w.abs_int64(-99)), 99, "abs_int64(-99) === 99")


fn test_abs_int64_zero() raises:
    var w = _get_wasm()
    assert_equal(Int(w.abs_int64(0)), 0, "abs_int64(0) === 0")


# ── Absolute value — float32 ────────────────────────────────────────────────


fn test_abs_float32_positive() raises:
    var w = _get_wasm()
    var result = Float64(w.abs_float32(2.5))
    var expected = Float64(Float32(2.5))
    assert_equal(result, expected, "abs_float32(2.5)")


fn test_abs_float32_negative() raises:
    var w = _get_wasm()
    var result = Float64(w.abs_float32(-2.5))
    var expected = Float64(Float32(2.5))
    assert_equal(result, expected, "abs_float32(-2.5)")


# ── Absolute value — float64 ────────────────────────────────────────────────


fn test_abs_float64_positive() raises:
    var w = _get_wasm()
    assert_equal(
        Float64(w.abs_float64(3.14)), 3.14, "abs_float64(3.14) === 3.14"
    )


fn test_abs_float64_negative() raises:
    var w = _get_wasm()
    assert_equal(
        Float64(w.abs_float64(-3.14)), 3.14, "abs_float64(-3.14) === 3.14"
    )


fn test_abs_float64_zero() raises:
    var w = _get_wasm()
    assert_equal(Float64(w.abs_float64(0.0)), 0.0, "abs_float64(0) === 0")

# Port of test/floats.test.ts — float NaN, Infinity, negative zero, subnormal,
# and precision edge-case tests exercised through the real WASM binary via
# wasmtime-py (called from Mojo via Python interop).
#
# Run with:
#   mojo test test-wasm/test_floats.mojo

from python import Python, PythonObject
from testing import assert_true, assert_equal


fn _get_wasm() raises -> PythonObject:
    Python.add_to_path("test-wasm")
    var harness = Python.import_module("wasm_harness")
    return harness.get_instance()


fn _pymath() raises -> PythonObject:
    return Python.import_module("math")


fn _nan() raises -> PythonObject:
    return Python.import_module("math").nan


fn _inf() raises -> PythonObject:
    return Python.import_module("math").inf


fn _neg_inf() raises -> PythonObject:
    return -Python.import_module("math").inf


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


fn _assert_nan(result: PythonObject, label: String) raises:
    var math = _pymath()
    assert_true(Bool(math.isnan(result)), label + ": expected NaN")


fn _is_negative_zero(x: PythonObject) raises -> Bool:
    var math = _pymath()
    return Bool(x == 0.0) and Bool(math.copysign(1.0, x) < 0)


fn _assert_eq(
    actual: PythonObject, expected: PythonObject, label: String
) raises:
    """Strict equality including sign of zero."""
    var math = _pymath()
    # Check for expected -0.0
    if Bool(expected == 0.0) and Bool(math.copysign(1.0, expected) < 0):
        assert_true(
            _is_negative_zero(actual),
            label + ": expected -0.0, got " + String(actual),
        )
    # Check for expected +0.0
    elif Bool(expected == 0.0) and Bool(math.copysign(1.0, expected) > 0):
        assert_true(
            Bool(actual == 0.0) and not _is_negative_zero(actual),
            label + ": expected +0.0, got " + String(actual),
        )
    else:
        assert_true(
            Bool(actual == expected),
            label
            + ": expected "
            + String(expected)
            + ", got "
            + String(actual),
        )


# ---------------------------------------------------------------------------
# NaN propagation — arithmetic
# ---------------------------------------------------------------------------


fn test_add_float64_nan_1() raises:
    var w = _get_wasm()
    var nan = _nan()
    _assert_nan(w.add_float64(nan, 1.0), "add_float64(NaN, 1.0)")


fn test_add_float64_1_nan() raises:
    var w = _get_wasm()
    var nan = _nan()
    _assert_nan(w.add_float64(1.0, nan), "add_float64(1.0, NaN)")


fn test_add_float64_nan_nan() raises:
    var w = _get_wasm()
    var nan = _nan()
    _assert_nan(w.add_float64(nan, nan), "add_float64(NaN, NaN)")


fn test_sub_float64_nan_1() raises:
    var w = _get_wasm()
    var nan = _nan()
    _assert_nan(w.sub_float64(nan, 1.0), "sub_float64(NaN, 1.0)")


fn test_sub_float64_1_nan() raises:
    var w = _get_wasm()
    var nan = _nan()
    _assert_nan(w.sub_float64(1.0, nan), "sub_float64(1.0, NaN)")


fn test_mul_float64_nan_2() raises:
    var w = _get_wasm()
    var nan = _nan()
    _assert_nan(w.mul_float64(nan, 2.0), "mul_float64(NaN, 2.0)")


fn test_mul_float64_2_nan() raises:
    var w = _get_wasm()
    var nan = _nan()
    _assert_nan(w.mul_float64(2.0, nan), "mul_float64(2.0, NaN)")


fn test_div_float64_nan_2() raises:
    var w = _get_wasm()
    var nan = _nan()
    _assert_nan(w.div_float64(nan, 2.0), "div_float64(NaN, 2.0)")


fn test_div_float64_2_nan() raises:
    var w = _get_wasm()
    var nan = _nan()
    _assert_nan(w.div_float64(2.0, nan), "div_float64(2.0, NaN)")


# ---------------------------------------------------------------------------
# NaN propagation — float32
# ---------------------------------------------------------------------------


fn test_add_float32_nan() raises:
    var w = _get_wasm()
    var nan = _nan()
    _assert_nan(w.add_float32(nan, 1.0), "add_float32(NaN, 1.0)")


fn test_sub_float32_nan() raises:
    var w = _get_wasm()
    var nan = _nan()
    _assert_nan(w.sub_float32(nan, 1.0), "sub_float32(NaN, 1.0)")


fn test_mul_float32_nan() raises:
    var w = _get_wasm()
    var nan = _nan()
    _assert_nan(w.mul_float32(nan, 2.0), "mul_float32(NaN, 2.0)")


fn test_div_float32_nan() raises:
    var w = _get_wasm()
    var nan = _nan()
    _assert_nan(w.div_float32(nan, 2.0), "div_float32(NaN, 2.0)")


# ---------------------------------------------------------------------------
# NaN-producing operations
# ---------------------------------------------------------------------------


fn test_add_inf_neg_inf() raises:
    var w = _get_wasm()
    var inf = _inf()
    var neg_inf = _neg_inf()
    _assert_nan(w.add_float64(inf, neg_inf), "add_float64(Inf, -Inf)")


fn test_sub_inf_inf() raises:
    var w = _get_wasm()
    var inf = _inf()
    _assert_nan(w.sub_float64(inf, inf), "sub_float64(Inf, Inf)")


fn test_mul_0_inf() raises:
    var w = _get_wasm()
    var inf = _inf()
    _assert_nan(w.mul_float64(0.0, inf), "mul_float64(0, Inf)")


fn test_div_0_0() raises:
    var w = _get_wasm()
    _assert_nan(w.div_float64(0.0, 0.0), "div_float64(0, 0)")


# ---------------------------------------------------------------------------
# NaN propagation — unary
# ---------------------------------------------------------------------------


fn test_neg_float64_nan() raises:
    var w = _get_wasm()
    var nan = _nan()
    _assert_nan(w.neg_float64(nan), "neg_float64(NaN)")


fn test_neg_float32_nan() raises:
    var w = _get_wasm()
    var nan = _nan()
    _assert_nan(w.neg_float32(nan), "neg_float32(NaN)")


fn test_abs_float64_nan() raises:
    var w = _get_wasm()
    var nan = _nan()
    _assert_nan(w.abs_float64(nan), "abs_float64(NaN)")


fn test_abs_float32_nan() raises:
    var w = _get_wasm()
    var nan = _nan()
    _assert_nan(w.abs_float32(nan), "abs_float32(NaN)")


# ---------------------------------------------------------------------------
# NaN propagation — identity
# ---------------------------------------------------------------------------


fn test_identity_float64_nan() raises:
    var w = _get_wasm()
    var nan = _nan()
    _assert_nan(w.identity_float64(nan), "identity_float64(NaN)")


fn test_identity_float32_nan() raises:
    var w = _get_wasm()
    var nan = _nan()
    _assert_nan(w.identity_float32(nan), "identity_float32(NaN)")


# ---------------------------------------------------------------------------
# NaN — min/max (comparison quirk)
#
# Mojo uses `if x < y: return x; return y` — NaN comparisons always return
# false, so the "else" branch wins.
# ---------------------------------------------------------------------------


fn test_min_nan_5() raises:
    var w = _get_wasm()
    var nan = _nan()
    _assert_eq(
        w.min_float64(nan, 5.0),
        PythonObject(5.0),
        "min_float64(NaN, 5.0) === 5.0",
    )


fn test_min_5_nan() raises:
    var w = _get_wasm()
    var nan = _nan()
    _assert_nan(w.min_float64(5.0, nan), "min_float64(5.0, NaN) === NaN")


fn test_max_nan_5() raises:
    var w = _get_wasm()
    var nan = _nan()
    _assert_eq(
        w.max_float64(nan, 5.0),
        PythonObject(5.0),
        "max_float64(NaN, 5.0) === 5.0",
    )


fn test_max_5_nan() raises:
    var w = _get_wasm()
    var nan = _nan()
    _assert_nan(w.max_float64(5.0, nan), "max_float64(5.0, NaN) === NaN")


# ---------------------------------------------------------------------------
# NaN — power
# ---------------------------------------------------------------------------


fn test_pow_float64_nan() raises:
    var w = _get_wasm()
    var nan = _nan()
    _assert_nan(w.pow_float64(nan), "pow_float64(NaN)")


fn test_pow_float32_nan() raises:
    var w = _get_wasm()
    var nan = _nan()
    _assert_nan(w.pow_float32(nan), "pow_float32(NaN)")


# ---------------------------------------------------------------------------
# Infinity — arithmetic
# ---------------------------------------------------------------------------


fn test_add_inf_1() raises:
    var w = _get_wasm()
    var inf = _inf()
    _assert_eq(w.add_float64(inf, 1.0), inf, "add_float64(Inf, 1) === Inf")


fn test_add_neg_inf_neg1() raises:
    var w = _get_wasm()
    var neg_inf = _neg_inf()
    _assert_eq(
        w.add_float64(neg_inf, -1.0),
        neg_inf,
        "add_float64(-Inf, -1) === -Inf",
    )


fn test_sub_inf_1() raises:
    var w = _get_wasm()
    var inf = _inf()
    _assert_eq(w.sub_float64(inf, 1.0), inf, "sub_float64(Inf, 1) === Inf")


fn test_mul_inf_2() raises:
    var w = _get_wasm()
    var inf = _inf()
    _assert_eq(w.mul_float64(inf, 2.0), inf, "mul_float64(Inf, 2) === Inf")


fn test_mul_inf_neg2() raises:
    var w = _get_wasm()
    var inf = _inf()
    var neg_inf = _neg_inf()
    _assert_eq(
        w.mul_float64(inf, -2.0), neg_inf, "mul_float64(Inf, -2) === -Inf"
    )


fn test_div_1_0() raises:
    var w = _get_wasm()
    var inf = _inf()
    _assert_eq(w.div_float64(1.0, 0.0), inf, "div_float64(1, 0) === Inf")


fn test_div_neg1_0() raises:
    var w = _get_wasm()
    var neg_inf = _neg_inf()
    _assert_eq(w.div_float64(-1.0, 0.0), neg_inf, "div_float64(-1, 0) === -Inf")


fn test_div_1_inf() raises:
    var w = _get_wasm()
    var inf = _inf()
    _assert_eq(
        w.div_float64(1.0, inf), PythonObject(0.0), "div_float64(1, Inf) === 0"
    )


# ---------------------------------------------------------------------------
# Infinity — float32
# ---------------------------------------------------------------------------


fn test_add_float32_inf_1() raises:
    var w = _get_wasm()
    var inf = _inf()
    _assert_eq(w.add_float32(inf, 1.0), inf, "add_float32(Inf, 1) === Inf")


fn test_div_float32_1_0() raises:
    var w = _get_wasm()
    var inf = _inf()
    _assert_eq(w.div_float32(1.0, 0.0), inf, "div_float32(1, 0) === Inf")


fn test_div_float32_neg1_0() raises:
    var w = _get_wasm()
    var neg_inf = _neg_inf()
    _assert_eq(w.div_float32(-1.0, 0.0), neg_inf, "div_float32(-1, 0) === -Inf")


# ---------------------------------------------------------------------------
# Infinity — unary
# ---------------------------------------------------------------------------


fn test_neg_inf() raises:
    var w = _get_wasm()
    var inf = _inf()
    var neg_inf = _neg_inf()
    _assert_eq(w.neg_float64(inf), neg_inf, "neg_float64(Inf) === -Inf")


fn test_neg_neg_inf() raises:
    var w = _get_wasm()
    var inf = _inf()
    var neg_inf = _neg_inf()
    _assert_eq(w.neg_float64(neg_inf), inf, "neg_float64(-Inf) === Inf")


fn test_abs_neg_inf() raises:
    var w = _get_wasm()
    var inf = _inf()
    var neg_inf = _neg_inf()
    _assert_eq(w.abs_float64(neg_inf), inf, "abs_float64(-Inf) === Inf")


fn test_abs_inf() raises:
    var w = _get_wasm()
    var inf = _inf()
    _assert_eq(w.abs_float64(inf), inf, "abs_float64(Inf) === Inf")


# ---------------------------------------------------------------------------
# Infinity — identity
# ---------------------------------------------------------------------------


fn test_identity_float64_inf() raises:
    var w = _get_wasm()
    var inf = _inf()
    _assert_eq(w.identity_float64(inf), inf, "identity_float64(Inf) === Inf")


fn test_identity_float64_neg_inf() raises:
    var w = _get_wasm()
    var neg_inf = _neg_inf()
    _assert_eq(
        w.identity_float64(neg_inf),
        neg_inf,
        "identity_float64(-Inf) === -Inf",
    )


fn test_identity_float32_inf() raises:
    var w = _get_wasm()
    var inf = _inf()
    _assert_eq(w.identity_float32(inf), inf, "identity_float32(Inf) === Inf")


# ---------------------------------------------------------------------------
# Infinity — min/max
# ---------------------------------------------------------------------------


fn test_min_neg_inf_inf() raises:
    var w = _get_wasm()
    var inf = _inf()
    var neg_inf = _neg_inf()
    _assert_eq(
        w.min_float64(neg_inf, inf),
        neg_inf,
        "min_float64(-Inf, Inf) === -Inf",
    )


fn test_max_neg_inf_inf() raises:
    var w = _get_wasm()
    var inf = _inf()
    var neg_inf = _neg_inf()
    _assert_eq(
        w.max_float64(neg_inf, inf), inf, "max_float64(-Inf, Inf) === Inf"
    )


fn test_min_42_neg_inf() raises:
    var w = _get_wasm()
    var neg_inf = _neg_inf()
    _assert_eq(
        w.min_float64(42.0, neg_inf),
        neg_inf,
        "min_float64(42, -Inf) === -Inf",
    )


fn test_max_42_inf() raises:
    var w = _get_wasm()
    var inf = _inf()
    _assert_eq(w.max_float64(42.0, inf), inf, "max_float64(42, Inf) === Inf")


# ---------------------------------------------------------------------------
# Infinity — clamp
# ---------------------------------------------------------------------------


fn test_clamp_inf() raises:
    var w = _get_wasm()
    var inf = _inf()
    _assert_eq(
        w.clamp_float64(inf, 0.0, 10.0),
        PythonObject(10.0),
        "clamp_float64(Inf, 0, 10) === 10",
    )


fn test_clamp_neg_inf() raises:
    var w = _get_wasm()
    var neg_inf = _neg_inf()
    _assert_eq(
        w.clamp_float64(neg_inf, 0.0, 10.0),
        PythonObject(0.0),
        "clamp_float64(-Inf, 0, 10) === 0",
    )


# ---------------------------------------------------------------------------
# Negative zero (-0.0)
# ---------------------------------------------------------------------------


fn test_identity_neg_zero() raises:
    var w = _get_wasm()
    var neg_zero = -PythonObject(0.0)
    _assert_eq(
        w.identity_float64(neg_zero), neg_zero, "identity_float64(-0) === -0"
    )


fn test_neg_zero() raises:
    var w = _get_wasm()
    _assert_eq(w.neg_float64(0.0), -PythonObject(0.0), "neg_float64(0) === -0")


fn test_neg_neg_zero() raises:
    var w = _get_wasm()
    var neg_zero = -PythonObject(0.0)
    _assert_eq(
        w.neg_float64(neg_zero), PythonObject(0.0), "neg_float64(-0) === 0"
    )


fn test_add_neg_zero_zero() raises:
    var w = _get_wasm()
    var neg_zero = -PythonObject(0.0)
    _assert_eq(
        w.add_float64(neg_zero, 0.0),
        PythonObject(0.0),
        "add_float64(-0, 0) === 0",
    )


fn test_mul_neg1_zero() raises:
    var w = _get_wasm()
    _assert_eq(
        w.mul_float64(-1.0, 0.0),
        -PythonObject(0.0),
        "mul_float64(-1, 0) === -0",
    )


fn test_mul_neg_zero_neg_zero() raises:
    var w = _get_wasm()
    var neg_zero = -PythonObject(0.0)
    _assert_eq(
        w.mul_float64(neg_zero, neg_zero),
        PythonObject(0.0),
        "mul_float64(-0, -0) === 0",
    )


fn test_div_1_neg_inf() raises:
    var w = _get_wasm()
    var neg_inf = _neg_inf()
    _assert_eq(
        w.div_float64(1.0, neg_inf),
        -PythonObject(0.0),
        "div_float64(1, -Inf) === -0",
    )


# ---------------------------------------------------------------------------
# Subnormal / denormalized numbers
# ---------------------------------------------------------------------------


fn test_identity_subnormal() raises:
    var w = _get_wasm()
    var subnormal = PythonObject(5e-324)
    _assert_eq(
        w.identity_float64(subnormal),
        subnormal,
        "identity_float64(5e-324) roundtrips",
    )


fn test_add_subnormal_zero() raises:
    var w = _get_wasm()
    var subnormal = PythonObject(5e-324)
    _assert_eq(
        w.add_float64(subnormal, 0.0),
        subnormal,
        "add_float64(subnormal, 0) === subnormal",
    )


fn test_neg_subnormal() raises:
    var w = _get_wasm()
    var subnormal = PythonObject(5e-324)
    var neg_subnormal = PythonObject(-5e-324)
    _assert_eq(
        w.neg_float64(subnormal),
        neg_subnormal,
        "neg_float64(subnormal) === -subnormal",
    )


fn test_abs_neg_subnormal() raises:
    var w = _get_wasm()
    var subnormal = PythonObject(5e-324)
    var neg_subnormal = PythonObject(-5e-324)
    _assert_eq(
        w.abs_float64(neg_subnormal),
        subnormal,
        "abs_float64(-subnormal) === subnormal",
    )


fn test_mul_subnormal_2() raises:
    var w = _get_wasm()
    var subnormal = PythonObject(5e-324)
    var expected = PythonObject(5e-324) * PythonObject(2.0)
    _assert_eq(
        w.mul_float64(subnormal, 2.0),
        expected,
        "mul_float64(subnormal, 2) === subnormal * 2",
    )


# ---------------------------------------------------------------------------
# Float precision edge cases
# ---------------------------------------------------------------------------


fn test_0_1_plus_0_2() raises:
    """Classic IEEE 754: 0.1 + 0.2 matches Python's result."""
    var w = _get_wasm()
    var expected = PythonObject(0.1) + PythonObject(0.2)
    _assert_eq(
        w.add_float64(0.1, 0.2),
        expected,
        "add_float64(0.1, 0.2) matches Python 0.1+0.2",
    )


fn test_0_1_plus_0_2_not_0_3() raises:
    """The WASM result also differs from 0.3 (IEEE 754 precision)."""
    var w = _get_wasm()
    var result = w.add_float64(0.1, 0.2)
    assert_true(
        Bool(result != PythonObject(0.3)),
        "add_float64(0.1, 0.2) !== 0.3 (IEEE 754 precision)",
    )


fn test_large_plus_small() raises:
    var w = _get_wasm()
    var expected = PythonObject(1e16) + PythonObject(1.0)
    _assert_eq(
        w.add_float64(1e16, 1.0),
        expected,
        "add_float64(1e16, 1) matches Python precision",
    )


fn test_catastrophic_cancellation() raises:
    var w = _get_wasm()
    var a = PythonObject(1e16) + PythonObject(2.0)
    var b = PythonObject(1e16)
    var expected = a - b
    _assert_eq(
        w.sub_float64(a, b),
        expected,
        "sub_float64(1e16+2, 1e16) matches Python precision",
    )

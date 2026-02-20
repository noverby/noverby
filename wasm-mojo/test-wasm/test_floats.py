"""
Port of test/floats.test.ts — float NaN, Infinity, negative zero, subnormal,
and precision edge-case tests exercised through the real WASM binary via
wasmtime-py.

Run with:
    uv run --with wasmtime --with pytest pytest test-wasm/test_floats.py
"""

import math
import struct

from conftest import WasmInstance

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _is_negative_zero(x: float) -> bool:
    return x == 0.0 and math.copysign(1.0, x) < 0


def _assert_nan(result, label: str):
    assert math.isnan(result), f"{label}: expected NaN, got {result!r}"


def _assert_eq(actual, expected, label: str):
    """Strict equality including sign of zero."""
    if isinstance(expected, float) and expected == 0.0 and _is_negative_zero(expected):
        assert _is_negative_zero(actual), f"{label}: expected -0.0, got {actual!r}"
    elif (
        isinstance(expected, float)
        and expected == 0.0
        and not _is_negative_zero(expected)
    ):
        assert actual == 0.0 and not _is_negative_zero(actual), (
            f"{label}: expected +0.0, got {actual!r}"
        )
    else:
        assert actual == expected, f"{label}: expected {expected!r}, got {actual!r}"


INF = float("inf")
NEG_INF = float("-inf")
NAN = float("nan")


# ---------------------------------------------------------------------------
# NaN propagation — arithmetic
# ---------------------------------------------------------------------------


class TestFloatNanArithmetic:
    def test_add_float64_nan_1(self, w: WasmInstance):
        _assert_nan(w.add_float64(NAN, 1.0), "add_float64(NaN, 1.0)")

    def test_add_float64_1_nan(self, w: WasmInstance):
        _assert_nan(w.add_float64(1.0, NAN), "add_float64(1.0, NaN)")

    def test_add_float64_nan_nan(self, w: WasmInstance):
        _assert_nan(w.add_float64(NAN, NAN), "add_float64(NaN, NaN)")

    def test_sub_float64_nan_1(self, w: WasmInstance):
        _assert_nan(w.sub_float64(NAN, 1.0), "sub_float64(NaN, 1.0)")

    def test_sub_float64_1_nan(self, w: WasmInstance):
        _assert_nan(w.sub_float64(1.0, NAN), "sub_float64(1.0, NaN)")

    def test_mul_float64_nan_2(self, w: WasmInstance):
        _assert_nan(w.mul_float64(NAN, 2.0), "mul_float64(NaN, 2.0)")

    def test_mul_float64_2_nan(self, w: WasmInstance):
        _assert_nan(w.mul_float64(2.0, NAN), "mul_float64(2.0, NaN)")

    def test_div_float64_nan_2(self, w: WasmInstance):
        _assert_nan(w.div_float64(NAN, 2.0), "div_float64(NaN, 2.0)")

    def test_div_float64_2_nan(self, w: WasmInstance):
        _assert_nan(w.div_float64(2.0, NAN), "div_float64(2.0, NaN)")


# ---------------------------------------------------------------------------
# NaN propagation — float32
# ---------------------------------------------------------------------------


class TestFloatNanFloat32:
    def test_add_float32_nan(self, w: WasmInstance):
        _assert_nan(w.add_float32(NAN, 1.0), "add_float32(NaN, 1.0)")

    def test_sub_float32_nan(self, w: WasmInstance):
        _assert_nan(w.sub_float32(NAN, 1.0), "sub_float32(NaN, 1.0)")

    def test_mul_float32_nan(self, w: WasmInstance):
        _assert_nan(w.mul_float32(NAN, 2.0), "mul_float32(NaN, 2.0)")

    def test_div_float32_nan(self, w: WasmInstance):
        _assert_nan(w.div_float32(NAN, 2.0), "div_float32(NaN, 2.0)")


# ---------------------------------------------------------------------------
# NaN-producing operations
# ---------------------------------------------------------------------------


class TestFloatNanProducing:
    def test_add_inf_neg_inf(self, w: WasmInstance):
        _assert_nan(w.add_float64(INF, NEG_INF), "add_float64(Inf, -Inf)")

    def test_sub_inf_inf(self, w: WasmInstance):
        _assert_nan(w.sub_float64(INF, INF), "sub_float64(Inf, Inf)")

    def test_mul_0_inf(self, w: WasmInstance):
        _assert_nan(w.mul_float64(0.0, INF), "mul_float64(0, Inf)")

    def test_div_0_0(self, w: WasmInstance):
        _assert_nan(w.div_float64(0.0, 0.0), "div_float64(0, 0)")


# ---------------------------------------------------------------------------
# NaN propagation — unary
# ---------------------------------------------------------------------------


class TestFloatNanUnary:
    def test_neg_float64_nan(self, w: WasmInstance):
        _assert_nan(w.neg_float64(NAN), "neg_float64(NaN)")

    def test_neg_float32_nan(self, w: WasmInstance):
        _assert_nan(w.neg_float32(NAN), "neg_float32(NaN)")

    def test_abs_float64_nan(self, w: WasmInstance):
        _assert_nan(w.abs_float64(NAN), "abs_float64(NaN)")

    def test_abs_float32_nan(self, w: WasmInstance):
        _assert_nan(w.abs_float32(NAN), "abs_float32(NaN)")


# ---------------------------------------------------------------------------
# NaN propagation — identity
# ---------------------------------------------------------------------------


class TestFloatNanIdentity:
    def test_identity_float64_nan(self, w: WasmInstance):
        _assert_nan(w.identity_float64(NAN), "identity_float64(NaN)")

    def test_identity_float32_nan(self, w: WasmInstance):
        _assert_nan(w.identity_float32(NAN), "identity_float32(NaN)")


# ---------------------------------------------------------------------------
# NaN — min/max (comparison quirk)
#
# Mojo uses `if x < y: return x; return y` — NaN comparisons always return
# false, so the "else" branch wins.
# ---------------------------------------------------------------------------


class TestFloatNanMinMax:
    def test_min_nan_5(self, w: WasmInstance):
        _assert_eq(
            w.min_float64(NAN, 5.0),
            5.0,
            "min_float64(NaN, 5.0) === 5.0",
        )

    def test_min_5_nan(self, w: WasmInstance):
        _assert_nan(
            w.min_float64(5.0, NAN),
            "min_float64(5.0, NaN) === NaN",
        )

    def test_max_nan_5(self, w: WasmInstance):
        _assert_eq(
            w.max_float64(NAN, 5.0),
            5.0,
            "max_float64(NaN, 5.0) === 5.0",
        )

    def test_max_5_nan(self, w: WasmInstance):
        _assert_nan(
            w.max_float64(5.0, NAN),
            "max_float64(5.0, NaN) === NaN",
        )


# ---------------------------------------------------------------------------
# NaN — power
# ---------------------------------------------------------------------------


class TestFloatNanPow:
    def test_pow_float64_nan(self, w: WasmInstance):
        _assert_nan(w.pow_float64(NAN), "pow_float64(NaN)")

    def test_pow_float32_nan(self, w: WasmInstance):
        _assert_nan(w.pow_float32(NAN), "pow_float32(NaN)")


# ---------------------------------------------------------------------------
# Infinity — arithmetic
# ---------------------------------------------------------------------------


class TestFloatInfArithmetic:
    def test_add_inf_1(self, w: WasmInstance):
        _assert_eq(w.add_float64(INF, 1.0), INF, "add_float64(Inf, 1) === Inf")

    def test_add_neg_inf_neg1(self, w: WasmInstance):
        _assert_eq(
            w.add_float64(NEG_INF, -1.0), NEG_INF, "add_float64(-Inf, -1) === -Inf"
        )

    def test_sub_inf_1(self, w: WasmInstance):
        _assert_eq(w.sub_float64(INF, 1.0), INF, "sub_float64(Inf, 1) === Inf")

    def test_mul_inf_2(self, w: WasmInstance):
        _assert_eq(w.mul_float64(INF, 2.0), INF, "mul_float64(Inf, 2) === Inf")

    def test_mul_inf_neg2(self, w: WasmInstance):
        _assert_eq(w.mul_float64(INF, -2.0), NEG_INF, "mul_float64(Inf, -2) === -Inf")

    def test_div_1_0(self, w: WasmInstance):
        _assert_eq(w.div_float64(1.0, 0.0), INF, "div_float64(1, 0) === Inf")

    def test_div_neg1_0(self, w: WasmInstance):
        _assert_eq(w.div_float64(-1.0, 0.0), NEG_INF, "div_float64(-1, 0) === -Inf")

    def test_div_1_inf(self, w: WasmInstance):
        _assert_eq(w.div_float64(1.0, INF), 0.0, "div_float64(1, Inf) === 0")


# ---------------------------------------------------------------------------
# Infinity — float32
# ---------------------------------------------------------------------------


class TestFloatInfFloat32:
    def test_add_float32_inf_1(self, w: WasmInstance):
        _assert_eq(w.add_float32(INF, 1.0), INF, "add_float32(Inf, 1) === Inf")

    def test_div_float32_1_0(self, w: WasmInstance):
        _assert_eq(w.div_float32(1.0, 0.0), INF, "div_float32(1, 0) === Inf")

    def test_div_float32_neg1_0(self, w: WasmInstance):
        _assert_eq(w.div_float32(-1.0, 0.0), NEG_INF, "div_float32(-1, 0) === -Inf")


# ---------------------------------------------------------------------------
# Infinity — unary
# ---------------------------------------------------------------------------


class TestFloatInfUnary:
    def test_neg_inf(self, w: WasmInstance):
        _assert_eq(w.neg_float64(INF), NEG_INF, "neg_float64(Inf) === -Inf")

    def test_neg_neg_inf(self, w: WasmInstance):
        _assert_eq(w.neg_float64(NEG_INF), INF, "neg_float64(-Inf) === Inf")

    def test_abs_neg_inf(self, w: WasmInstance):
        _assert_eq(w.abs_float64(NEG_INF), INF, "abs_float64(-Inf) === Inf")

    def test_abs_inf(self, w: WasmInstance):
        _assert_eq(w.abs_float64(INF), INF, "abs_float64(Inf) === Inf")


# ---------------------------------------------------------------------------
# Infinity — identity
# ---------------------------------------------------------------------------


class TestFloatInfIdentity:
    def test_identity_float64_inf(self, w: WasmInstance):
        _assert_eq(w.identity_float64(INF), INF, "identity_float64(Inf) === Inf")

    def test_identity_float64_neg_inf(self, w: WasmInstance):
        _assert_eq(
            w.identity_float64(NEG_INF),
            NEG_INF,
            "identity_float64(-Inf) === -Inf",
        )

    def test_identity_float32_inf(self, w: WasmInstance):
        _assert_eq(w.identity_float32(INF), INF, "identity_float32(Inf) === Inf")


# ---------------------------------------------------------------------------
# Infinity — min/max
# ---------------------------------------------------------------------------


class TestFloatInfMinMax:
    def test_min_neg_inf_inf(self, w: WasmInstance):
        _assert_eq(
            w.min_float64(NEG_INF, INF), NEG_INF, "min_float64(-Inf, Inf) === -Inf"
        )

    def test_max_neg_inf_inf(self, w: WasmInstance):
        _assert_eq(w.max_float64(NEG_INF, INF), INF, "max_float64(-Inf, Inf) === Inf")

    def test_min_42_neg_inf(self, w: WasmInstance):
        _assert_eq(
            w.min_float64(42.0, NEG_INF),
            NEG_INF,
            "min_float64(42, -Inf) === -Inf",
        )

    def test_max_42_inf(self, w: WasmInstance):
        _assert_eq(w.max_float64(42.0, INF), INF, "max_float64(42, Inf) === Inf")


# ---------------------------------------------------------------------------
# Infinity — clamp
# ---------------------------------------------------------------------------


class TestFloatInfClamp:
    def test_clamp_inf(self, w: WasmInstance):
        _assert_eq(
            w.clamp_float64(INF, 0.0, 10.0),
            10.0,
            "clamp_float64(Inf, 0, 10) === 10",
        )

    def test_clamp_neg_inf(self, w: WasmInstance):
        _assert_eq(
            w.clamp_float64(NEG_INF, 0.0, 10.0),
            0.0,
            "clamp_float64(-Inf, 0, 10) === 0",
        )


# ---------------------------------------------------------------------------
# Negative zero (-0.0)
# ---------------------------------------------------------------------------


class TestFloatNegativeZero:
    def test_identity_neg_zero(self, w: WasmInstance):
        _assert_eq(w.identity_float64(-0.0), -0.0, "identity_float64(-0) === -0")

    def test_neg_zero(self, w: WasmInstance):
        _assert_eq(w.neg_float64(0.0), -0.0, "neg_float64(0) === -0")

    def test_neg_neg_zero(self, w: WasmInstance):
        _assert_eq(w.neg_float64(-0.0), 0.0, "neg_float64(-0) === 0")

    def test_add_neg_zero_zero(self, w: WasmInstance):
        _assert_eq(w.add_float64(-0.0, 0.0), 0.0, "add_float64(-0, 0) === 0")

    def test_mul_neg1_zero(self, w: WasmInstance):
        _assert_eq(w.mul_float64(-1.0, 0.0), -0.0, "mul_float64(-1, 0) === -0")

    def test_mul_neg_zero_neg_zero(self, w: WasmInstance):
        _assert_eq(w.mul_float64(-0.0, -0.0), 0.0, "mul_float64(-0, -0) === 0")

    def test_div_1_neg_inf(self, w: WasmInstance):
        _assert_eq(w.div_float64(1.0, NEG_INF), -0.0, "div_float64(1, -Inf) === -0")


# ---------------------------------------------------------------------------
# Subnormal / denormalized numbers
# ---------------------------------------------------------------------------

SUBNORMAL = 5e-324


class TestFloatSubnormals:
    def test_identity_subnormal(self, w: WasmInstance):
        _assert_eq(
            w.identity_float64(SUBNORMAL),
            SUBNORMAL,
            "identity_float64(5e-324) roundtrips",
        )

    def test_add_subnormal_zero(self, w: WasmInstance):
        _assert_eq(
            w.add_float64(SUBNORMAL, 0.0),
            SUBNORMAL,
            "add_float64(subnormal, 0) === subnormal",
        )

    def test_neg_subnormal(self, w: WasmInstance):
        _assert_eq(
            w.neg_float64(SUBNORMAL),
            -SUBNORMAL,
            "neg_float64(subnormal) === -subnormal",
        )

    def test_abs_neg_subnormal(self, w: WasmInstance):
        _assert_eq(
            w.abs_float64(-SUBNORMAL),
            SUBNORMAL,
            "abs_float64(-subnormal) === subnormal",
        )

    def test_mul_subnormal_2(self, w: WasmInstance):
        _assert_eq(
            w.mul_float64(SUBNORMAL, 2.0),
            SUBNORMAL * 2.0,
            "mul_float64(subnormal, 2) === subnormal * 2",
        )


# ---------------------------------------------------------------------------
# Float precision edge cases
# ---------------------------------------------------------------------------


class TestFloatPrecision:
    def test_0_1_plus_0_2(self, w: WasmInstance):
        """Classic IEEE 754: 0.1 + 0.2 matches Python's result."""
        _assert_eq(
            w.add_float64(0.1, 0.2),
            0.1 + 0.2,
            "add_float64(0.1, 0.2) matches Python 0.1+0.2",
        )

    def test_0_1_plus_0_2_not_0_3(self, w: WasmInstance):
        """The WASM result also differs from 0.3 (IEEE 754 precision)."""
        result = w.add_float64(0.1, 0.2)
        assert result != 0.3, "add_float64(0.1, 0.2) !== 0.3 (IEEE 754 precision)"

    def test_large_plus_small(self, w: WasmInstance):
        _assert_eq(
            w.add_float64(1e16, 1.0),
            1e16 + 1.0,
            "add_float64(1e16, 1) matches Python precision",
        )

    def test_catastrophic_cancellation(self, w: WasmInstance):
        _assert_eq(
            w.sub_float64(1e16 + 2.0, 1e16),
            1e16 + 2.0 - 1e16,
            "sub_float64(1e16+2, 1e16) matches Python precision",
        )

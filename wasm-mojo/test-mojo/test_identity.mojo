# Tests for identity / passthrough — native Mojo tests run with `mojo test`.
#
# These tests are a direct port of test/identity.test.ts, exercising
# the same scenarios without the WASM/JS round-trip.  The functions
# under test are defined inline here (matching main.mojo's @export fns).
#
# Run with:
#   mojo test -I src test-mojo/test_identity.mojo

from testing import assert_equal, assert_true
from math import nan, isnan


# ── Functions under test (mirrors of main.mojo @export fns) ──────────────────


fn identity_int32(x: Int32) -> Int32:
    return x


fn identity_int64(x: Int64) -> Int64:
    return x


fn identity_float32(x: Float32) -> Float32:
    return x


fn identity_float64(x: Float64) -> Float64:
    return x


# ── Identity — int32 ─────────────────────────────────────────────────────────


fn test_identity_int32_zero() raises:
    assert_equal(identity_int32(0), Int32(0), "identity_int32(0) === 0")


fn test_identity_int32_positive() raises:
    assert_equal(identity_int32(42), Int32(42), "identity_int32(42) === 42")


fn test_identity_int32_negative() raises:
    assert_equal(identity_int32(-42), Int32(-42), "identity_int32(-42) === -42")


# ── Identity — int64 ─────────────────────────────────────────────────────────


fn test_identity_int64_zero() raises:
    assert_equal(identity_int64(0), Int64(0), "identity_int64(0) === 0")


fn test_identity_int64_positive() raises:
    assert_equal(identity_int64(999), Int64(999), "identity_int64(999) === 999")


fn test_identity_int64_negative() raises:
    assert_equal(
        identity_int64(-999), Int64(-999), "identity_int64(-999) === -999"
    )


# ── Identity — float32 ───────────────────────────────────────────────────────


fn test_identity_float32_pi() raises:
    var input = Float32(3.14)
    assert_equal(identity_float32(input), input, "identity_float32(3.14)")


fn test_identity_float32_zero() raises:
    assert_equal(
        identity_float32(0.0), Float32(0.0), "identity_float32(0) === 0"
    )


# ── Identity — float64 ───────────────────────────────────────────────────────


fn test_identity_float64_pi() raises:
    var pi = Float64(3.141592653589793)
    assert_equal(identity_float64(pi), pi, "identity_float64(pi)")


fn test_identity_float64_zero() raises:
    assert_equal(identity_float64(0.0), 0.0, "identity_float64(0) === 0")


fn test_identity_float64_negative_zero() raises:
    # -0.0 should roundtrip through identity. IEEE 754 says -0.0 == 0.0,
    # but we can verify the sign bit is preserved via division.
    var result = identity_float64(-0.0)
    # 1.0 / -0.0 = -inf, so if sign is preserved the reciprocal is negative
    assert_true(
        (1.0 / result) < 0.0,
        "identity_float64(-0) === -0 (sign bit preserved)",
    )

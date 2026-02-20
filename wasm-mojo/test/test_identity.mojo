# Identity/passthrough operations exercised through the real WASM binary via
# wasmtime-py (called from Mojo via Python interop).
#
# These tests verify that identity functions correctly pass through values
# when compiled to WASM and executed via the Wasmtime runtime.
#
# Run with:
#   mojo test test/test_identity.mojo

from python import Python, PythonObject
from testing import assert_equal, assert_true


fn _get_wasm() raises -> PythonObject:
    Python.add_to_path("test")
    var harness = Python.import_module("wasm_harness")
    return harness.get_instance()


# ── Identity — int32 ─────────────────────────────────────────────────────────


fn test_identity_int32_zero() raises:
    var w = _get_wasm()
    assert_equal(Int(w.identity_int32(0)), 0, "identity_int32(0) === 0")


fn test_identity_int32_positive() raises:
    var w = _get_wasm()
    assert_equal(Int(w.identity_int32(42)), 42, "identity_int32(42) === 42")


fn test_identity_int32_negative() raises:
    var w = _get_wasm()
    assert_equal(Int(w.identity_int32(-42)), -42, "identity_int32(-42) === -42")


# ── Identity — int64 ─────────────────────────────────────────────────────────


fn test_identity_int64_zero() raises:
    var w = _get_wasm()
    assert_equal(Int(w.identity_int64(0)), 0, "identity_int64(0) === 0")


fn test_identity_int64_positive() raises:
    var w = _get_wasm()
    assert_equal(Int(w.identity_int64(999)), 999, "identity_int64(999) === 999")


fn test_identity_int64_negative() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w.identity_int64(-999)), -999, "identity_int64(-999) === -999"
    )


# ── Identity — float32 ───────────────────────────────────────────────────────


fn test_identity_float32_pi() raises:
    var w = _get_wasm()
    var input = Float64(Float32(3.14))
    var result = Float64(w.identity_float32(3.14))
    assert_equal(result, input, "identity_float32(3.14)")


fn test_identity_float32_zero() raises:
    var w = _get_wasm()
    assert_equal(
        Float64(w.identity_float32(0.0)),
        0.0,
        "identity_float32(0) === 0",
    )


# ── Identity — float64 ───────────────────────────────────────────────────────


fn test_identity_float64_pi() raises:
    var w = _get_wasm()
    var pi = 3.141592653589793
    assert_equal(Float64(w.identity_float64(pi)), pi, "identity_float64(pi)")


fn test_identity_float64_zero() raises:
    var w = _get_wasm()
    assert_equal(
        Float64(w.identity_float64(0.0)), 0.0, "identity_float64(0) === 0"
    )


fn test_identity_float64_negative_zero() raises:
    var w = _get_wasm()
    # -0.0 should roundtrip through identity. IEEE 754 says -0.0 == 0.0,
    # but we can verify the sign bit is preserved via division.
    var math = Python.import_module("math")
    var result = w.identity_float64(-0.0)
    # copysign(1.0, -0.0) == -1.0, so if sign is preserved the copysign is negative
    assert_true(
        Bool(math.copysign(1.0, result) < 0),
        "identity_float64(-0) === -0 (sign bit preserved)",
    )

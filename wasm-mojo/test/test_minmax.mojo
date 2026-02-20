# Min, max, and clamp operations exercised through the real WASM binary via
# wasmtime-mojo (pure Mojo FFI bindings — no Python interop required).
#
# These tests verify that min, max, and clamp operations work correctly
# when compiled to WASM and executed via the Wasmtime runtime.
#
# Run with:
#   mojo test test/test_minmax.mojo

from memory import UnsafePointer
from testing import assert_equal

from wasm_harness import (
    WasmInstance,
    get_instance,
    args_i32_i32,
    args_i32_i32_i32,
    args_i64_i64,
    args_f64_f64,
    args_f64_f64_f64,
)


fn _get_wasm() raises -> UnsafePointer[WasmInstance]:
    return get_instance()


# ── Min / Max — int32 ────────────────────────────────────────────────────────


fn test_min_int32_first_smaller() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("min_int32", args_i32_i32(3, 7))),
        3,
        "min_int32(3, 7) === 3",
    )


fn test_min_int32_second_smaller() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("min_int32", args_i32_i32(7, 3))),
        3,
        "min_int32(7, 3) === 3",
    )


fn test_min_int32_equal() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("min_int32", args_i32_i32(5, 5))),
        5,
        "min_int32(5, 5) === 5",
    )


fn test_min_int32_negative() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("min_int32", args_i32_i32(-3, 3))),
        -3,
        "min_int32(-3, 3) === -3",
    )


fn test_max_int32_second_larger() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("max_int32", args_i32_i32(3, 7))),
        7,
        "max_int32(3, 7) === 7",
    )


fn test_max_int32_first_larger() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("max_int32", args_i32_i32(7, 3))),
        7,
        "max_int32(7, 3) === 7",
    )


fn test_max_int32_equal() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("max_int32", args_i32_i32(5, 5))),
        5,
        "max_int32(5, 5) === 5",
    )


fn test_max_int32_negative() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("max_int32", args_i32_i32(-3, 3))),
        3,
        "max_int32(-3, 3) === 3",
    )


# ── Min / Max — int64 ────────────────────────────────────────────────────────


fn test_min_int64_first_smaller() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i64("min_int64", args_i64_i64(3, 7))),
        3,
        "min_int64(3, 7) === 3",
    )


fn test_min_int64_second_smaller() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i64("min_int64", args_i64_i64(7, 3))),
        3,
        "min_int64(7, 3) === 3",
    )


fn test_min_int64_negative() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i64("min_int64", args_i64_i64(-10, 10))),
        -10,
        "min_int64(-10, 10) === -10",
    )


fn test_max_int64_second_larger() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i64("max_int64", args_i64_i64(3, 7))),
        7,
        "max_int64(3, 7) === 7",
    )


fn test_max_int64_first_larger() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i64("max_int64", args_i64_i64(7, 3))),
        7,
        "max_int64(7, 3) === 7",
    )


fn test_max_int64_negative() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i64("max_int64", args_i64_i64(-10, 10))),
        10,
        "max_int64(-10, 10) === 10",
    )


# ── Min / Max — float64 ─────────────────────────────────────────────────────


fn test_min_float64_first_smaller() raises:
    var w = _get_wasm()
    assert_equal(
        w[].call_f64("min_float64", args_f64_f64(1.1, 2.2)),
        1.1,
        "min_float64(1.1, 2.2) === 1.1",
    )


fn test_min_float64_second_smaller() raises:
    var w = _get_wasm()
    assert_equal(
        w[].call_f64("min_float64", args_f64_f64(2.2, 1.1)),
        1.1,
        "min_float64(2.2, 1.1) === 1.1",
    )


fn test_min_float64_negative() raises:
    var w = _get_wasm()
    assert_equal(
        w[].call_f64("min_float64", args_f64_f64(-0.5, 0.5)),
        -0.5,
        "min_float64(-0.5, 0.5) === -0.5",
    )


fn test_max_float64_second_larger() raises:
    var w = _get_wasm()
    assert_equal(
        w[].call_f64("max_float64", args_f64_f64(1.1, 2.2)),
        2.2,
        "max_float64(1.1, 2.2) === 2.2",
    )


fn test_max_float64_first_larger() raises:
    var w = _get_wasm()
    assert_equal(
        w[].call_f64("max_float64", args_f64_f64(2.2, 1.1)),
        2.2,
        "max_float64(2.2, 1.1) === 2.2",
    )


fn test_max_float64_negative() raises:
    var w = _get_wasm()
    assert_equal(
        w[].call_f64("max_float64", args_f64_f64(-0.5, 0.5)),
        0.5,
        "max_float64(-0.5, 0.5) === 0.5",
    )


# ── Clamp — int32 ───────────────────────────────────────────────────────────


fn test_clamp_int32_within_range() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("clamp_int32", args_i32_i32_i32(5, 0, 10))),
        5,
        "clamp_int32(5, 0, 10) === 5 (within range)",
    )


fn test_clamp_int32_below() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("clamp_int32", args_i32_i32_i32(-5, 0, 10))),
        0,
        "clamp_int32(-5, 0, 10) === 0 (below)",
    )


fn test_clamp_int32_above() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("clamp_int32", args_i32_i32_i32(15, 0, 10))),
        10,
        "clamp_int32(15, 0, 10) === 10 (above)",
    )


fn test_clamp_int32_at_low_bound() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("clamp_int32", args_i32_i32_i32(0, 0, 10))),
        0,
        "clamp_int32(0, 0, 10) === 0 (at low bound)",
    )


fn test_clamp_int32_at_high_bound() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("clamp_int32", args_i32_i32_i32(10, 0, 10))),
        10,
        "clamp_int32(10, 0, 10) === 10 (at high bound)",
    )


# ── Clamp — float64 ─────────────────────────────────────────────────────────


fn test_clamp_float64_within_range() raises:
    var w = _get_wasm()
    assert_equal(
        w[].call_f64("clamp_float64", args_f64_f64_f64(5.5, 0.0, 10.0)),
        5.5,
        "clamp_float64(5.5, 0, 10) === 5.5",
    )


fn test_clamp_float64_below() raises:
    var w = _get_wasm()
    assert_equal(
        w[].call_f64("clamp_float64", args_f64_f64_f64(-1.0, 0.0, 10.0)),
        0.0,
        "clamp_float64(-1, 0, 10) === 0",
    )


fn test_clamp_float64_above() raises:
    var w = _get_wasm()
    assert_equal(
        w[].call_f64("clamp_float64", args_f64_f64_f64(11.0, 0.0, 10.0)),
        10.0,
        "clamp_float64(11, 0, 10) === 10",
    )

# Comparison and boolean logic operations exercised through the real WASM binary
# via wasmtime-mojo (pure Mojo FFI bindings — no Python interop required).
#
# These tests verify that eq, ne, lt, le, gt, ge, bool_and, bool_or, and bool_not
# work correctly when compiled to WASM and executed via the Wasmtime runtime.
#
# Run with:
#   mojo test test/test_comparison.mojo

from memory import UnsafePointer
from testing import assert_equal

from wasm_harness import (
    WasmInstance,
    get_instance,
    args_i32,
    args_i32_i32,
)


fn _get_wasm() raises -> UnsafePointer[WasmInstance]:
    return get_instance()


# ── Comparison — eq / ne ─────────────────────────────────────────────────────


fn test_eq_int32_equal() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("eq_int32", args_i32_i32(5, 5))),
        1,
        "eq_int32(5, 5) === true",
    )


fn test_eq_int32_not_equal() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("eq_int32", args_i32_i32(5, 6))),
        0,
        "eq_int32(5, 6) === false",
    )


fn test_eq_int32_zero() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("eq_int32", args_i32_i32(0, 0))),
        1,
        "eq_int32(0, 0) === true",
    )


fn test_ne_int32_not_equal() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("ne_int32", args_i32_i32(5, 6))),
        1,
        "ne_int32(5, 6) === true",
    )


fn test_ne_int32_equal() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("ne_int32", args_i32_i32(5, 5))),
        0,
        "ne_int32(5, 5) === false",
    )


# ── Comparison — lt / le / gt / ge ───────────────────────────────────────────


fn test_lt_int32_less() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("lt_int32", args_i32_i32(3, 5))),
        1,
        "lt_int32(3, 5) === true",
    )


fn test_lt_int32_equal() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("lt_int32", args_i32_i32(5, 5))),
        0,
        "lt_int32(5, 5) === false",
    )


fn test_lt_int32_greater() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("lt_int32", args_i32_i32(7, 5))),
        0,
        "lt_int32(7, 5) === false",
    )


fn test_le_int32_less() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("le_int32", args_i32_i32(3, 5))),
        1,
        "le_int32(3, 5) === true",
    )


fn test_le_int32_equal() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("le_int32", args_i32_i32(5, 5))),
        1,
        "le_int32(5, 5) === true",
    )


fn test_le_int32_greater() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("le_int32", args_i32_i32(7, 5))),
        0,
        "le_int32(7, 5) === false",
    )


fn test_gt_int32_greater() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("gt_int32", args_i32_i32(7, 5))),
        1,
        "gt_int32(7, 5) === true",
    )


fn test_gt_int32_equal() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("gt_int32", args_i32_i32(5, 5))),
        0,
        "gt_int32(5, 5) === false",
    )


fn test_gt_int32_less() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("gt_int32", args_i32_i32(3, 5))),
        0,
        "gt_int32(3, 5) === false",
    )


fn test_ge_int32_greater() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("ge_int32", args_i32_i32(7, 5))),
        1,
        "ge_int32(7, 5) === true",
    )


fn test_ge_int32_equal() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("ge_int32", args_i32_i32(5, 5))),
        1,
        "ge_int32(5, 5) === true",
    )


fn test_ge_int32_less() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("ge_int32", args_i32_i32(3, 5))),
        0,
        "ge_int32(3, 5) === false",
    )


# ── Comparison — negative numbers ────────────────────────────────────────────


fn test_lt_negative_vs_zero() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("lt_int32", args_i32_i32(-5, 0))),
        1,
        "lt_int32(-5, 0) === true",
    )


fn test_gt_zero_vs_negative() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("gt_int32", args_i32_i32(0, -5))),
        1,
        "gt_int32(0, -5) === true",
    )


fn test_le_negative_equal() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("le_int32", args_i32_i32(-5, -5))),
        1,
        "le_int32(-5, -5) === true",
    )


fn test_ge_negative_equal() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("ge_int32", args_i32_i32(-5, -5))),
        1,
        "ge_int32(-5, -5) === true",
    )


fn test_lt_more_negative() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("lt_int32", args_i32_i32(-10, -5))),
        1,
        "lt_int32(-10, -5) === true",
    )


fn test_gt_less_negative() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("gt_int32", args_i32_i32(-5, -10))),
        1,
        "gt_int32(-5, -10) === true",
    )


# ── Boolean logic — and ─────────────────────────────────────────────────────


fn test_bool_and_true_true() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("bool_and", args_i32_i32(1, 1))),
        1,
        "bool_and(true, true) === true",
    )


fn test_bool_and_true_false() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("bool_and", args_i32_i32(1, 0))),
        0,
        "bool_and(true, false) === false",
    )


fn test_bool_and_false_true() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("bool_and", args_i32_i32(0, 1))),
        0,
        "bool_and(false, true) === false",
    )


fn test_bool_and_false_false() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("bool_and", args_i32_i32(0, 0))),
        0,
        "bool_and(false, false) === false",
    )


# ── Boolean logic — or ──────────────────────────────────────────────────────


fn test_bool_or_true_true() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("bool_or", args_i32_i32(1, 1))),
        1,
        "bool_or(true, true) === true",
    )


fn test_bool_or_true_false() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("bool_or", args_i32_i32(1, 0))),
        1,
        "bool_or(true, false) === true",
    )


fn test_bool_or_false_true() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("bool_or", args_i32_i32(0, 1))),
        1,
        "bool_or(false, true) === true",
    )


fn test_bool_or_false_false() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("bool_or", args_i32_i32(0, 0))),
        0,
        "bool_or(false, false) === false",
    )


# ── Boolean logic — not ─────────────────────────────────────────────────────


fn test_bool_not_true() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("bool_not", args_i32(1))),
        0,
        "bool_not(true) === false",
    )


fn test_bool_not_false() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("bool_not", args_i32(0))),
        1,
        "bool_not(false) === true",
    )

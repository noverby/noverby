# Bitwise operations exercised through the real WASM binary via
# wasmtime-mojo (pure Mojo FFI bindings — no Python interop required).
#
# These tests verify that bitand, bitor, bitxor, bitnot, shl, and shr operations
# work correctly when compiled to WASM and executed via the Wasmtime runtime.
#
# Run with:
#   mojo test test/test_bitwise.mojo

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


# ── Bitwise AND ──────────────────────────────────────────────────────────────


fn test_bitand_basic() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("bitand_int32", args_i32_i32(0b1100, 0b1010))),
        0b1000,
        "bitand_int32(0b1100, 0b1010) === 0b1000",
    )


fn test_bitand_mask() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("bitand_int32", args_i32_i32(0xFF, 0x0F))),
        0x0F,
        "bitand_int32(0xFF, 0x0F) === 0x0F",
    )


fn test_bitand_zero() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("bitand_int32", args_i32_i32(0, 0xFFFF))),
        0,
        "bitand_int32(0, 0xFFFF) === 0",
    )


# ── Bitwise OR ───────────────────────────────────────────────────────────────


fn test_bitor_basic() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("bitor_int32", args_i32_i32(0b1100, 0b1010))),
        0b1110,
        "bitor_int32(0b1100, 0b1010) === 0b1110",
    )


fn test_bitor_zero() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("bitor_int32", args_i32_i32(0, 0))),
        0,
        "bitor_int32(0, 0) === 0",
    )


# ── Bitwise XOR ──────────────────────────────────────────────────────────────


fn test_bitxor_basic() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("bitxor_int32", args_i32_i32(0b1100, 0b1010))),
        0b0110,
        "bitxor_int32(0b1100, 0b1010) === 0b0110",
    )


fn test_bitxor_self_is_zero() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("bitxor_int32", args_i32_i32(42, 42))),
        0,
        "bitxor_int32(42, 42) === 0",
    )


fn test_bitxor_with_zero_is_identity() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("bitxor_int32", args_i32_i32(42, 0))),
        42,
        "bitxor_int32(42, 0) === 42",
    )


# ── Bitwise NOT ──────────────────────────────────────────────────────────────


fn test_bitnot_zero() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("bitnot_int32", args_i32(0))),
        Int(~Int32(0)),
        "bitnot_int32(0) === ~0",
    )


fn test_bitnot_one() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("bitnot_int32", args_i32(1))),
        Int(~Int32(1)),
        "bitnot_int32(1) === ~1",
    )


# ── Shifts ───────────────────────────────────────────────────────────────────


fn test_shl_by_zero() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("shl_int32", args_i32_i32(1, 0))),
        1,
        "shl_int32(1, 0) === 1",
    )


fn test_shl_by_one() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("shl_int32", args_i32_i32(1, 1))),
        2,
        "shl_int32(1, 1) === 2",
    )


fn test_shl_by_four() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("shl_int32", args_i32_i32(1, 4))),
        16,
        "shl_int32(1, 4) === 16",
    )


fn test_shl_three_by_three() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("shl_int32", args_i32_i32(3, 3))),
        24,
        "shl_int32(3, 3) === 24",
    )


fn test_shr_sixteen_by_four() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("shr_int32", args_i32_i32(16, 4))),
        1,
        "shr_int32(16, 4) === 1",
    )


fn test_shr_twentyfour_by_three() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("shr_int32", args_i32_i32(24, 3))),
        3,
        "shr_int32(24, 3) === 3",
    )


fn test_shr_255_by_one() raises:
    var w = _get_wasm()
    assert_equal(
        Int(w[].call_i32("shr_int32", args_i32_i32(255, 1))),
        127,
        "shr_int32(255, 1) === 127",
    )

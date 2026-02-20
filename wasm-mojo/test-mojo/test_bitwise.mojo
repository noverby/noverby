# Tests for bitwise operations — native Mojo tests run with `mojo test`.
#
# These tests are a direct port of test/bitwise.test.ts, exercising
# the same scenarios without the WASM/JS round-trip.  The functions
# under test are defined inline here (matching main.mojo's @export fns).
#
# Run with:
#   mojo test -I src test-mojo/test_bitwise.mojo

from testing import assert_equal


# ── Functions under test (mirrors of main.mojo @export fns) ──────────────────


fn bitand_int32(x: Int32, y: Int32) -> Int32:
    return x & y


fn bitor_int32(x: Int32, y: Int32) -> Int32:
    return x | y


fn bitxor_int32(x: Int32, y: Int32) -> Int32:
    return x ^ y


fn bitnot_int32(x: Int32) -> Int32:
    return ~x


fn shl_int32(x: Int32, y: Int32) -> Int32:
    return x << y


fn shr_int32(x: Int32, y: Int32) -> Int32:
    return x >> y


# ── Bitwise AND ──────────────────────────────────────────────────────────────


fn test_bitand_basic() raises:
    assert_equal(
        bitand_int32(0b1100, 0b1010),
        Int32(0b1000),
        "bitand_int32(0b1100, 0b1010) === 0b1000",
    )


fn test_bitand_mask() raises:
    assert_equal(
        bitand_int32(0xFF, 0x0F),
        Int32(0x0F),
        "bitand_int32(0xFF, 0x0F) === 0x0F",
    )


fn test_bitand_zero() raises:
    assert_equal(
        bitand_int32(0, 0xFFFF),
        Int32(0),
        "bitand_int32(0, 0xFFFF) === 0",
    )


# ── Bitwise OR ───────────────────────────────────────────────────────────────


fn test_bitor_basic() raises:
    assert_equal(
        bitor_int32(0b1100, 0b1010),
        Int32(0b1110),
        "bitor_int32(0b1100, 0b1010) === 0b1110",
    )


fn test_bitor_zero() raises:
    assert_equal(
        bitor_int32(0, 0),
        Int32(0),
        "bitor_int32(0, 0) === 0",
    )


# ── Bitwise XOR ──────────────────────────────────────────────────────────────


fn test_bitxor_basic() raises:
    assert_equal(
        bitxor_int32(0b1100, 0b1010),
        Int32(0b0110),
        "bitxor_int32(0b1100, 0b1010) === 0b0110",
    )


fn test_bitxor_self_is_zero() raises:
    assert_equal(
        bitxor_int32(42, 42),
        Int32(0),
        "bitxor_int32(42, 42) === 0",
    )


fn test_bitxor_with_zero_is_identity() raises:
    assert_equal(
        bitxor_int32(42, 0),
        Int32(42),
        "bitxor_int32(42, 0) === 42",
    )


# ── Bitwise NOT ──────────────────────────────────────────────────────────────


fn test_bitnot_zero() raises:
    assert_equal(
        bitnot_int32(0),
        ~Int32(0),
        "bitnot_int32(0) === ~0",
    )


fn test_bitnot_one() raises:
    assert_equal(
        bitnot_int32(1),
        ~Int32(1),
        "bitnot_int32(1) === ~1",
    )


# ── Shifts ───────────────────────────────────────────────────────────────────


fn test_shl_by_zero() raises:
    assert_equal(
        shl_int32(1, 0),
        Int32(1),
        "shl_int32(1, 0) === 1",
    )


fn test_shl_by_one() raises:
    assert_equal(
        shl_int32(1, 1),
        Int32(2),
        "shl_int32(1, 1) === 2",
    )


fn test_shl_by_four() raises:
    assert_equal(
        shl_int32(1, 4),
        Int32(16),
        "shl_int32(1, 4) === 16",
    )


fn test_shl_three_by_three() raises:
    assert_equal(
        shl_int32(3, 3),
        Int32(24),
        "shl_int32(3, 3) === 24",
    )


fn test_shr_sixteen_by_four() raises:
    assert_equal(
        shr_int32(16, 4),
        Int32(1),
        "shr_int32(16, 4) === 1",
    )


fn test_shr_twentyfour_by_three() raises:
    assert_equal(
        shr_int32(24, 3),
        Int32(3),
        "shr_int32(24, 3) === 3",
    )


fn test_shr_255_by_one() raises:
    assert_equal(
        shr_int32(255, 1),
        Int32(127),
        "shr_int32(255, 1) === 127",
    )

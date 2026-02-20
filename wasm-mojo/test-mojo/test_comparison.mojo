# Tests for comparison and boolean logic — native Mojo tests run with `mojo test`.
#
# These tests are a direct port of test/comparison.test.ts, exercising
# the same scenarios without the WASM/JS round-trip.  The functions
# under test are defined inline here (matching main.mojo's @export fns).
#
# Run with:
#   mojo test -I src test-mojo/test_comparison.mojo

from testing import assert_equal, assert_true, assert_false


# ── Functions under test (mirrors of main.mojo @export fns) ──────────────────


fn eq_int32(x: Int32, y: Int32) -> Int32:
    if x == y:
        return 1
    return 0


fn ne_int32(x: Int32, y: Int32) -> Int32:
    if x != y:
        return 1
    return 0


fn lt_int32(x: Int32, y: Int32) -> Int32:
    if x < y:
        return 1
    return 0


fn le_int32(x: Int32, y: Int32) -> Int32:
    if x <= y:
        return 1
    return 0


fn gt_int32(x: Int32, y: Int32) -> Int32:
    if x > y:
        return 1
    return 0


fn ge_int32(x: Int32, y: Int32) -> Int32:
    if x >= y:
        return 1
    return 0


fn bool_and(x: Int32, y: Int32) -> Int32:
    if (x != 0) and (y != 0):
        return 1
    return 0


fn bool_or(x: Int32, y: Int32) -> Int32:
    if (x != 0) or (y != 0):
        return 1
    return 0


fn bool_not(x: Int32) -> Int32:
    if x == 0:
        return 1
    return 0


# ── Comparison — eq / ne ─────────────────────────────────────────────────────


fn test_eq_int32_equal() raises:
    assert_equal(eq_int32(5, 5), Int32(1), "eq_int32(5, 5) === true")


fn test_eq_int32_not_equal() raises:
    assert_equal(eq_int32(5, 6), Int32(0), "eq_int32(5, 6) === false")


fn test_eq_int32_zero() raises:
    assert_equal(eq_int32(0, 0), Int32(1), "eq_int32(0, 0) === true")


fn test_ne_int32_not_equal() raises:
    assert_equal(ne_int32(5, 6), Int32(1), "ne_int32(5, 6) === true")


fn test_ne_int32_equal() raises:
    assert_equal(ne_int32(5, 5), Int32(0), "ne_int32(5, 5) === false")


# ── Comparison — lt / le / gt / ge ───────────────────────────────────────────


fn test_lt_int32_less() raises:
    assert_equal(lt_int32(3, 5), Int32(1), "lt_int32(3, 5) === true")


fn test_lt_int32_equal() raises:
    assert_equal(lt_int32(5, 5), Int32(0), "lt_int32(5, 5) === false")


fn test_lt_int32_greater() raises:
    assert_equal(lt_int32(7, 5), Int32(0), "lt_int32(7, 5) === false")


fn test_le_int32_less() raises:
    assert_equal(le_int32(3, 5), Int32(1), "le_int32(3, 5) === true")


fn test_le_int32_equal() raises:
    assert_equal(le_int32(5, 5), Int32(1), "le_int32(5, 5) === true")


fn test_le_int32_greater() raises:
    assert_equal(le_int32(7, 5), Int32(0), "le_int32(7, 5) === false")


fn test_gt_int32_greater() raises:
    assert_equal(gt_int32(7, 5), Int32(1), "gt_int32(7, 5) === true")


fn test_gt_int32_equal() raises:
    assert_equal(gt_int32(5, 5), Int32(0), "gt_int32(5, 5) === false")


fn test_gt_int32_less() raises:
    assert_equal(gt_int32(3, 5), Int32(0), "gt_int32(3, 5) === false")


fn test_ge_int32_greater() raises:
    assert_equal(ge_int32(7, 5), Int32(1), "ge_int32(7, 5) === true")


fn test_ge_int32_equal() raises:
    assert_equal(ge_int32(5, 5), Int32(1), "ge_int32(5, 5) === true")


fn test_ge_int32_less() raises:
    assert_equal(ge_int32(3, 5), Int32(0), "ge_int32(3, 5) === false")


# ── Comparison — negative numbers ────────────────────────────────────────────


fn test_lt_negative_vs_zero() raises:
    assert_equal(lt_int32(-5, 0), Int32(1), "lt_int32(-5, 0) === true")


fn test_gt_zero_vs_negative() raises:
    assert_equal(gt_int32(0, -5), Int32(1), "gt_int32(0, -5) === true")


fn test_le_negative_equal() raises:
    assert_equal(le_int32(-5, -5), Int32(1), "le_int32(-5, -5) === true")


fn test_ge_negative_equal() raises:
    assert_equal(ge_int32(-5, -5), Int32(1), "ge_int32(-5, -5) === true")


fn test_lt_more_negative() raises:
    assert_equal(lt_int32(-10, -5), Int32(1), "lt_int32(-10, -5) === true")


fn test_gt_less_negative() raises:
    assert_equal(gt_int32(-5, -10), Int32(1), "gt_int32(-5, -10) === true")


# ── Boolean logic — and ─────────────────────────────────────────────────────


fn test_bool_and_true_true() raises:
    assert_equal(bool_and(1, 1), Int32(1), "bool_and(true, true) === true")


fn test_bool_and_true_false() raises:
    assert_equal(bool_and(1, 0), Int32(0), "bool_and(true, false) === false")


fn test_bool_and_false_true() raises:
    assert_equal(bool_and(0, 1), Int32(0), "bool_and(false, true) === false")


fn test_bool_and_false_false() raises:
    assert_equal(bool_and(0, 0), Int32(0), "bool_and(false, false) === false")


# ── Boolean logic — or ──────────────────────────────────────────────────────


fn test_bool_or_true_true() raises:
    assert_equal(bool_or(1, 1), Int32(1), "bool_or(true, true) === true")


fn test_bool_or_true_false() raises:
    assert_equal(bool_or(1, 0), Int32(1), "bool_or(true, false) === true")


fn test_bool_or_false_true() raises:
    assert_equal(bool_or(0, 1), Int32(1), "bool_or(false, true) === true")


fn test_bool_or_false_false() raises:
    assert_equal(bool_or(0, 0), Int32(0), "bool_or(false, false) === false")


# ── Boolean logic — not ─────────────────────────────────────────────────────


fn test_bool_not_true() raises:
    assert_equal(bool_not(1), Int32(0), "bool_not(true) === false")


fn test_bool_not_false() raises:
    assert_equal(bool_not(0), Int32(1), "bool_not(false) === true")

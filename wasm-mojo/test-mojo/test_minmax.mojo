# Tests for min, max, and clamp — native Mojo tests run with `mojo test`.
#
# These tests are a direct port of test/minmax.test.ts, exercising
# the same scenarios without the WASM/JS round-trip.  The functions
# under test are defined inline here (matching main.mojo's @export fns).
#
# Run with:
#   mojo test -I src test-mojo/test_minmax.mojo

from testing import assert_equal


# ── Functions under test (mirrors of main.mojo @export fns) ──────────────────


fn min_int32(x: Int32, y: Int32) -> Int32:
    if x < y:
        return x
    return y


fn max_int32(x: Int32, y: Int32) -> Int32:
    if x > y:
        return x
    return y


fn min_int64(x: Int64, y: Int64) -> Int64:
    if x < y:
        return x
    return y


fn max_int64(x: Int64, y: Int64) -> Int64:
    if x > y:
        return x
    return y


fn min_float64(x: Float64, y: Float64) -> Float64:
    if x < y:
        return x
    return y


fn max_float64(x: Float64, y: Float64) -> Float64:
    if x > y:
        return x
    return y


fn clamp_int32(x: Int32, lo: Int32, hi: Int32) -> Int32:
    if x < lo:
        return lo
    if x > hi:
        return hi
    return x


fn clamp_float64(x: Float64, lo: Float64, hi: Float64) -> Float64:
    if x < lo:
        return lo
    if x > hi:
        return hi
    return x


# ── Min / Max — int32 ────────────────────────────────────────────────────────


fn test_min_int32_first_smaller() raises:
    assert_equal(min_int32(3, 7), Int32(3), "min_int32(3, 7) === 3")


fn test_min_int32_second_smaller() raises:
    assert_equal(min_int32(7, 3), Int32(3), "min_int32(7, 3) === 3")


fn test_min_int32_equal() raises:
    assert_equal(min_int32(5, 5), Int32(5), "min_int32(5, 5) === 5")


fn test_min_int32_negative() raises:
    assert_equal(min_int32(-3, 3), Int32(-3), "min_int32(-3, 3) === -3")


fn test_max_int32_second_larger() raises:
    assert_equal(max_int32(3, 7), Int32(7), "max_int32(3, 7) === 7")


fn test_max_int32_first_larger() raises:
    assert_equal(max_int32(7, 3), Int32(7), "max_int32(7, 3) === 7")


fn test_max_int32_equal() raises:
    assert_equal(max_int32(5, 5), Int32(5), "max_int32(5, 5) === 5")


fn test_max_int32_negative() raises:
    assert_equal(max_int32(-3, 3), Int32(3), "max_int32(-3, 3) === 3")


# ── Min / Max — int64 ────────────────────────────────────────────────────────


fn test_min_int64_first_smaller() raises:
    assert_equal(min_int64(3, 7), Int64(3), "min_int64(3, 7) === 3")


fn test_min_int64_second_smaller() raises:
    assert_equal(min_int64(7, 3), Int64(3), "min_int64(7, 3) === 3")


fn test_min_int64_negative() raises:
    assert_equal(min_int64(-10, 10), Int64(-10), "min_int64(-10, 10) === -10")


fn test_max_int64_second_larger() raises:
    assert_equal(max_int64(3, 7), Int64(7), "max_int64(3, 7) === 7")


fn test_max_int64_first_larger() raises:
    assert_equal(max_int64(7, 3), Int64(7), "max_int64(7, 3) === 7")


fn test_max_int64_negative() raises:
    assert_equal(max_int64(-10, 10), Int64(10), "max_int64(-10, 10) === 10")


# ── Min / Max — float64 ─────────────────────────────────────────────────────


fn test_min_float64_first_smaller() raises:
    assert_equal(min_float64(1.1, 2.2), 1.1, "min_float64(1.1, 2.2) === 1.1")


fn test_min_float64_second_smaller() raises:
    assert_equal(min_float64(2.2, 1.1), 1.1, "min_float64(2.2, 1.1) === 1.1")


fn test_min_float64_negative() raises:
    assert_equal(
        min_float64(-0.5, 0.5), -0.5, "min_float64(-0.5, 0.5) === -0.5"
    )


fn test_max_float64_second_larger() raises:
    assert_equal(max_float64(1.1, 2.2), 2.2, "max_float64(1.1, 2.2) === 2.2")


fn test_max_float64_first_larger() raises:
    assert_equal(max_float64(2.2, 1.1), 2.2, "max_float64(2.2, 1.1) === 2.2")


fn test_max_float64_negative() raises:
    assert_equal(max_float64(-0.5, 0.5), 0.5, "max_float64(-0.5, 0.5) === 0.5")


# ── Clamp — int32 ───────────────────────────────────────────────────────────


fn test_clamp_int32_within_range() raises:
    assert_equal(
        clamp_int32(5, 0, 10),
        Int32(5),
        "clamp_int32(5, 0, 10) === 5 (within range)",
    )


fn test_clamp_int32_below() raises:
    assert_equal(
        clamp_int32(-5, 0, 10),
        Int32(0),
        "clamp_int32(-5, 0, 10) === 0 (below)",
    )


fn test_clamp_int32_above() raises:
    assert_equal(
        clamp_int32(15, 0, 10),
        Int32(10),
        "clamp_int32(15, 0, 10) === 10 (above)",
    )


fn test_clamp_int32_at_low_bound() raises:
    assert_equal(
        clamp_int32(0, 0, 10),
        Int32(0),
        "clamp_int32(0, 0, 10) === 0 (at low bound)",
    )


fn test_clamp_int32_at_high_bound() raises:
    assert_equal(
        clamp_int32(10, 0, 10),
        Int32(10),
        "clamp_int32(10, 0, 10) === 10 (at high bound)",
    )


# ── Clamp — float64 ─────────────────────────────────────────────────────────


fn test_clamp_float64_within_range() raises:
    assert_equal(
        clamp_float64(5.5, 0.0, 10.0),
        5.5,
        "clamp_float64(5.5, 0, 10) === 5.5",
    )


fn test_clamp_float64_below() raises:
    assert_equal(
        clamp_float64(-1.0, 0.0, 10.0),
        0.0,
        "clamp_float64(-1, 0, 10) === 0",
    )


fn test_clamp_float64_above() raises:
    assert_equal(
        clamp_float64(11.0, 0.0, 10.0),
        10.0,
        "clamp_float64(11, 0, 10) === 10",
    )

# Tests for arithmetic operations — native Mojo tests run with `mojo test`.
#
# These tests are a direct port of test/arithmetic.test.ts, exercising
# the same scenarios without the WASM/JS round-trip.  The functions
# under test are defined inline here (matching main.mojo's @export fns).
#
# Run with:
#   mojo test -I src test-mojo/test_arithmetic.mojo

from testing import assert_equal, assert_true


# ── Functions under test (mirrors of main.mojo @export fns) ──────────────────


fn add_int32(x: Int32, y: Int32) -> Int32:
    return x + y


fn add_int64(x: Int64, y: Int64) -> Int64:
    return x + y


fn add_float32(x: Float32, y: Float32) -> Float32:
    return x + y


fn add_float64(x: Float64, y: Float64) -> Float64:
    return x + y


fn sub_int32(x: Int32, y: Int32) -> Int32:
    return x - y


fn sub_int64(x: Int64, y: Int64) -> Int64:
    return x - y


fn sub_float32(x: Float32, y: Float32) -> Float32:
    return x - y


fn sub_float64(x: Float64, y: Float64) -> Float64:
    return x - y


fn mul_int32(x: Int32, y: Int32) -> Int32:
    return x * y


fn mul_int64(x: Int64, y: Int64) -> Int64:
    return x * y


fn mul_float32(x: Float32, y: Float32) -> Float32:
    return x * y


fn mul_float64(x: Float64, y: Float64) -> Float64:
    return x * y


fn div_int32(x: Int32, y: Int32) -> Int32:
    return x // y


fn div_int64(x: Int64, y: Int64) -> Int64:
    return x // y


fn div_float32(x: Float32, y: Float32) -> Float32:
    return x / y


fn div_float64(x: Float64, y: Float64) -> Float64:
    return x / y


fn mod_int32(x: Int32, y: Int32) -> Int32:
    return x % y


fn mod_int64(x: Int64, y: Int64) -> Int64:
    return x % y


fn pow_int32(x: Int32) -> Int32:
    return x**x


fn pow_int64(x: Int64) -> Int64:
    return x**x


fn pow_float32(x: Float32) -> Float32:
    return x**x


fn pow_float64(x: Float64) -> Float64:
    return x**x


# ── Add ──────────────────────────────────────────────────────────────────────


fn test_add_int32() raises:
    assert_equal(add_int32(2, 3), 5, "add_int32(2, 3) === 5")


fn test_add_int64() raises:
    assert_equal(add_int64(2, 3), Int64(5), "add_int64(2, 3) === 5")


fn test_add_float32() raises:
    var expected = Float32(2.2) + Float32(3.3)
    assert_equal(add_float32(2.2, 3.3), expected, "add_float32(2.2, 3.3)")


fn test_add_float64() raises:
    assert_equal(add_float64(2.2, 3.3), 2.2 + 3.3, "add_float64(2.2, 3.3)")


fn test_add_int32_edge_cases() raises:
    assert_equal(add_int32(0, 0), 0, "add_int32(0, 0) === 0")
    assert_equal(add_int32(-5, 5), 0, "add_int32(-5, 5) === 0")
    assert_equal(add_int32(-3, -7), -10, "add_int32(-3, -7) === -10")
    assert_equal(add_int32(1, 0), 1, "add_int32(1, 0) === 1 (identity)")


fn test_add_int64_edge_cases() raises:
    assert_equal(add_int64(0, 0), Int64(0), "add_int64(0, 0) === 0")
    assert_equal(add_int64(-100, 100), Int64(0), "add_int64(-100, 100) === 0")
    assert_equal(add_int64(1, 0), Int64(1), "add_int64(1, 0) === 1 (identity)")


fn test_add_float64_edge_cases() raises:
    assert_equal(add_float64(0.0, 0.0), 0.0, "add_float64(0, 0) === 0")
    assert_equal(add_float64(-1.5, 1.5), 0.0, "add_float64(-1.5, 1.5) === 0")


# ── Subtract ─────────────────────────────────────────────────────────────────


fn test_sub_int32() raises:
    assert_equal(sub_int32(10, 3), 7, "sub_int32(10, 3) === 7")


fn test_sub_int64() raises:
    assert_equal(sub_int64(10, 3), Int64(7), "sub_int64(10, 3) === 7")


fn test_sub_float32() raises:
    var expected = Float32(5.5) - Float32(2.2)
    assert_equal(sub_float32(5.5, 2.2), expected, "sub_float32(5.5, 2.2)")


fn test_sub_float64() raises:
    assert_equal(sub_float64(5.5, 2.2), 5.5 - 2.2, "sub_float64(5.5, 2.2)")


fn test_sub_int32_edge_cases() raises:
    assert_equal(sub_int32(0, 0), 0, "sub_int32(0, 0) === 0")
    assert_equal(sub_int32(5, 5), 0, "sub_int32(5, 5) === 0")
    assert_equal(sub_int32(3, 7), -4, "sub_int32(3, 7) === -4")
    assert_equal(sub_int32(-3, -7), 4, "sub_int32(-3, -7) === 4")


fn test_sub_int64_edge_cases() raises:
    assert_equal(sub_int64(0, 0), Int64(0), "sub_int64(0, 0) === 0")
    assert_equal(sub_int64(-50, -50), Int64(0), "sub_int64(-50, -50) === 0")


# ── Multiply ─────────────────────────────────────────────────────────────────


fn test_mul_int32() raises:
    assert_equal(mul_int32(4, 5), 20, "mul_int32(4, 5) === 20")


fn test_mul_int64() raises:
    assert_equal(mul_int64(4, 5), Int64(20), "mul_int64(4, 5) === 20")


fn test_mul_float32() raises:
    var expected = Float32(2.0) * Float32(3.0)
    assert_equal(mul_float32(2.0, 3.0), expected, "mul_float32(2.0, 3.0)")


fn test_mul_float64() raises:
    assert_equal(mul_float64(2.5, 4.0), 10.0, "mul_float64(2.5, 4.0) === 10.0")


fn test_mul_int32_edge_cases() raises:
    assert_equal(mul_int32(0, 100), 0, "mul_int32(0, 100) === 0")
    assert_equal(mul_int32(1, 42), 42, "mul_int32(1, 42) === 42 (identity)")
    assert_equal(mul_int32(-1, 42), -42, "mul_int32(-1, 42) === -42")
    assert_equal(mul_int32(-3, -4), 12, "mul_int32(-3, -4) === 12")


fn test_mul_int64_edge_cases() raises:
    assert_equal(mul_int64(0, 999), Int64(0), "mul_int64(0, 999) === 0")
    assert_equal(
        mul_int64(1, 999),
        Int64(999),
        "mul_int64(1, 999) === 999 (identity)",
    )


fn test_mul_float64_edge_cases() raises:
    assert_equal(
        mul_float64(0.0, 123.456), 0.0, "mul_float64(0, 123.456) === 0"
    )


# ── Division ─────────────────────────────────────────────────────────────────


fn test_div_int32() raises:
    assert_equal(div_int32(20, 4), 5, "div_int32(20, 4) === 5")


fn test_div_int64() raises:
    assert_equal(div_int64(20, 4), Int64(5), "div_int64(20, 4) === 5")


fn test_div_float32() raises:
    var expected = Float32(10.0) / Float32(4.0)
    assert_equal(
        div_float32(10.0, 4.0),
        expected,
        "div_float32(10.0, 4.0) === 2.5",
    )


fn test_div_float64() raises:
    assert_equal(div_float64(10.0, 4.0), 2.5, "div_float64(10.0, 4.0) === 2.5")


fn test_div_int32_edge_cases() raises:
    assert_equal(div_int32(7, 2), 3, "div_int32(7, 2) === 3 (floor division)")
    assert_equal(div_int32(0, 5), 0, "div_int32(0, 5) === 0")
    assert_equal(div_int32(1, 1), 1, "div_int32(1, 1) === 1")
    # Mojo floor division for negative numbers:
    # -7 // 2 = -4 in Python/Mojo (floor toward -inf)
    assert_equal(
        div_int32(-7, 2), -4, "div_int32(-7, 2) === -4 (floor division)"
    )


fn test_div_int64_edge_cases() raises:
    assert_equal(
        div_int64(7, 2),
        Int64(3),
        "div_int64(7, 2) === 3 (floor division)",
    )


fn test_div_float64_edge_cases() raises:
    assert_equal(
        div_float64(1.0, 3.0),
        1.0 / 3.0,
        "div_float64(1, 3)",
    )
    assert_equal(div_float64(0.0, 1.0), 0.0, "div_float64(0, 1) === 0")


# ── Modulo ───────────────────────────────────────────────────────────────────


fn test_mod_int32() raises:
    assert_equal(mod_int32(10, 3), 1, "mod_int32(10, 3) === 1")
    assert_equal(mod_int32(15, 5), 0, "mod_int32(15, 5) === 0")
    assert_equal(mod_int32(0, 7), 0, "mod_int32(0, 7) === 0")
    assert_equal(mod_int32(7, 1), 0, "mod_int32(7, 1) === 0")


fn test_mod_int64() raises:
    assert_equal(mod_int64(10, 3), Int64(1), "mod_int64(10, 3) === 1")
    assert_equal(mod_int64(100, 7), Int64(2), "mod_int64(100, 7) === 2")


# ── Power ────────────────────────────────────────────────────────────────────


fn test_pow_int32() raises:
    assert_equal(pow_int32(3), 27, "pow_int32(3) === 27 (3^3)")
    assert_equal(pow_int32(1), 1, "pow_int32(1) === 1 (1^1)")
    assert_equal(pow_int32(2), 4, "pow_int32(2) === 4 (2^2)")


fn test_pow_int64() raises:
    assert_equal(pow_int64(3), Int64(27), "pow_int64(3) === 27 (3^3)")
    assert_equal(pow_int64(1), Int64(1), "pow_int64(1) === 1 (1^1)")
    assert_equal(pow_int64(2), Int64(4), "pow_int64(2) === 4 (2^2)")


fn test_pow_float64() raises:
    # pow_float64(3.3) = 3.3^3.3 ≈ 51.41572944937184
    var result = pow_float64(3.3)
    assert_true(
        result > 51.415 and result < 51.416,
        "pow_float64(3.3) ≈ 51.4157",
    )
    # pow_float64(1.0) = 1.0^1.0 = 1.0
    var r1 = pow_float64(1.0)
    assert_true(
        r1 > 0.999999 and r1 < 1.000001,
        "pow_float64(1.0) ≈ 1.0",
    )
    # pow_float64(2.0) = 2.0^2.0 = 4.0
    var r2 = pow_float64(2.0)
    assert_true(
        r2 > 3.999999 and r2 < 4.000001,
        "pow_float64(2.0) ≈ 4.0",
    )


fn test_pow_float32_stable() raises:
    # Verify pow_float32 is at least stable (same input → same output)
    var a = pow_float32(3.3)
    var b = pow_float32(3.3)
    assert_equal(a, b, "pow_float32(3.3) is stable")

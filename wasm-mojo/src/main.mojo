# Add
@export
fn add_int32(x: Int32, y: Int32) -> Int32:
    return x + y


@export
fn add_int64(x: Int64, y: Int64) -> Int64:
    return x + y


@export
fn add_float32(x: Float32, y: Float32) -> Float32:
    return x + y


@export
fn add_float64(x: Float64, y: Float64) -> Float64:
    return x + y


# Subtract
@export
fn sub_int32(x: Int32, y: Int32) -> Int32:
    return x - y


@export
fn sub_int64(x: Int64, y: Int64) -> Int64:
    return x - y


@export
fn sub_float32(x: Float32, y: Float32) -> Float32:
    return x - y


@export
fn sub_float64(x: Float64, y: Float64) -> Float64:
    return x - y


# Multiply
@export
fn mul_int32(x: Int32, y: Int32) -> Int32:
    return x * y


@export
fn mul_int64(x: Int64, y: Int64) -> Int64:
    return x * y


@export
fn mul_float32(x: Float32, y: Float32) -> Float32:
    return x * y


@export
fn mul_float64(x: Float64, y: Float64) -> Float64:
    return x * y


# Division
@export
fn div_int32(x: Int32, y: Int32) -> Int32:
    return x // y


@export
fn div_int64(x: Int64, y: Int64) -> Int64:
    return x // y


@export
fn div_float32(x: Float32, y: Float32) -> Float32:
    return x / y


@export
fn div_float64(x: Float64, y: Float64) -> Float64:
    return x / y


# Modulo
@export
fn mod_int32(x: Int32, y: Int32) -> Int32:
    return x % y


@export
fn mod_int64(x: Int64, y: Int64) -> Int64:
    return x % y


# Power
@export
fn pow_int32(x: Int32) -> Int32:
    return x**x


@export
fn pow_int64(x: Int64) -> Int64:
    return x**x


@export
fn pow_float32(x: Float32) -> Float32:
    return x**x


@export
fn pow_float64(x: Float64) -> Float64:
    return x**x


# Negate
@export
fn neg_int32(x: Int32) -> Int32:
    return -x


@export
fn neg_int64(x: Int64) -> Int64:
    return -x


@export
fn neg_float32(x: Float32) -> Float32:
    return -x


@export
fn neg_float64(x: Float64) -> Float64:
    return -x


# Absolute value
@export
fn abs_int32(x: Int32) -> Int32:
    if x < 0:
        return -x
    return x


@export
fn abs_int64(x: Int64) -> Int64:
    if x < 0:
        return -x
    return x


@export
fn abs_float32(x: Float32) -> Float32:
    if x < 0:
        return -x
    return x


@export
fn abs_float64(x: Float64) -> Float64:
    if x < 0:
        return -x
    return x


# Min / Max
@export
fn min_int32(x: Int32, y: Int32) -> Int32:
    if x < y:
        return x
    return y


@export
fn max_int32(x: Int32, y: Int32) -> Int32:
    if x > y:
        return x
    return y


@export
fn min_int64(x: Int64, y: Int64) -> Int64:
    if x < y:
        return x
    return y


@export
fn max_int64(x: Int64, y: Int64) -> Int64:
    if x > y:
        return x
    return y


@export
fn min_float64(x: Float64, y: Float64) -> Float64:
    if x < y:
        return x
    return y


@export
fn max_float64(x: Float64, y: Float64) -> Float64:
    if x > y:
        return x
    return y


# Clamp
@export
fn clamp_int32(x: Int32, lo: Int32, hi: Int32) -> Int32:
    if x < lo:
        return lo
    if x > hi:
        return hi
    return x


@export
fn clamp_float64(x: Float64, lo: Float64, hi: Float64) -> Float64:
    if x < lo:
        return lo
    if x > hi:
        return hi
    return x


# Bitwise operations
@export
fn bitand_int32(x: Int32, y: Int32) -> Int32:
    return x & y


@export
fn bitor_int32(x: Int32, y: Int32) -> Int32:
    return x | y


@export
fn bitxor_int32(x: Int32, y: Int32) -> Int32:
    return x ^ y


@export
fn bitnot_int32(x: Int32) -> Int32:
    return ~x


@export
fn shl_int32(x: Int32, y: Int32) -> Int32:
    return x << y


@export
fn shr_int32(x: Int32, y: Int32) -> Int32:
    return x >> y


# Boolean / comparison
@export
fn eq_int32(x: Int32, y: Int32) -> Bool:
    return x == y


@export
fn ne_int32(x: Int32, y: Int32) -> Bool:
    return x != y


@export
fn lt_int32(x: Int32, y: Int32) -> Bool:
    return x < y


@export
fn le_int32(x: Int32, y: Int32) -> Bool:
    return x <= y


@export
fn gt_int32(x: Int32, y: Int32) -> Bool:
    return x > y


@export
fn ge_int32(x: Int32, y: Int32) -> Bool:
    return x >= y


@export
fn bool_and(x: Bool, y: Bool) -> Bool:
    return x and y


@export
fn bool_or(x: Bool, y: Bool) -> Bool:
    return x or y


@export
fn bool_not(x: Bool) -> Bool:
    return not x


# Fibonacci (iterative)
@export
fn fib_int32(n: Int32) -> Int32:
    if n <= 0:
        return 0
    if n == 1:
        return 1
    var a: Int32 = 0
    var b: Int32 = 1
    for _ in range(2, Int(n) + 1):
        var tmp = a + b
        a = b
        b = tmp
    return b


@export
fn fib_int64(n: Int64) -> Int64:
    if n <= 0:
        return 0
    if n == 1:
        return 1
    var a: Int64 = 0
    var b: Int64 = 1
    for _ in range(2, Int(n) + 1):
        var tmp = a + b
        a = b
        b = tmp
    return b


# Factorial (iterative)
@export
fn factorial_int32(n: Int32) -> Int32:
    if n <= 1:
        return 1
    var result: Int32 = 1
    for i in range(2, Int(n) + 1):
        result *= Int32(i)
    return result


@export
fn factorial_int64(n: Int64) -> Int64:
    if n <= 1:
        return 1
    var result: Int64 = 1
    for i in range(2, Int(n) + 1):
        result *= Int64(i)
    return result


# GCD (Euclidean algorithm)
@export
fn gcd_int32(x: Int32, y: Int32) -> Int32:
    var a = x
    var b = y
    if a < 0:
        a = -a
    if b < 0:
        b = -b
    while b != 0:
        var tmp = b
        b = a % b
        a = tmp
    return a


# Identity / passthrough
@export
fn identity_int32(x: Int32) -> Int32:
    return x


@export
fn identity_int64(x: Int64) -> Int64:
    return x


@export
fn identity_float32(x: Float32) -> Float32:
    return x


@export
fn identity_float64(x: Float64) -> Float64:
    return x


# Print
@export
fn print_int32():
    alias int32: Int32 = 3
    print(int32)


@export
fn print_int64():
    alias int64: Int64 = 3
    print(2)


@export
fn print_float32():
    alias float32: Float32 = 3.0
    print(float32)


@export
fn print_float64():
    alias float64: Float64 = 3.0
    print(float64)


@export
fn print_static_string():
    print("print-static-string")


# Print input
@export
fn print_input_string(input: String):
    print(input)


# Return
@export
fn return_input_string(x: String) -> String:
    return x


@export
fn return_static_string() -> String:
    return "return-static-string"


# String length
@export
fn string_length(x: String) -> Int64:
    return Int64(len(x))


# String concatenation
@export
fn string_concat(x: String, y: String) -> String:
    return x + y


# String repeat
@export
fn string_repeat(x: String, n: Int32) -> String:
    var result = String("")
    for _ in range(Int(n)):
        result += x
    return result


# String equality
@export
fn string_eq(x: String, y: String) -> Bool:
    return x == y

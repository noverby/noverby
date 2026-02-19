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

# PoC Strings — identity, print, string_length, string_concat, string_repeat, string_eq


# ── Identity / passthrough ──────────────────────────────────────────────────


fn poc_identity_int32(x: Int32) -> Int32:
    return x


fn poc_identity_int64(x: Int64) -> Int64:
    return x


fn poc_identity_float32(x: Float32) -> Float32:
    return x


fn poc_identity_float64(x: Float64) -> Float64:
    return x


# ── Print ────────────────────────────────────────────────────────────────────


fn poc_print_int32():
    alias int32: Int32 = 3
    print(int32)


fn poc_print_int64():
    alias int64: Int64 = 3
    print(2)


fn poc_print_float32():
    alias float32: Float32 = 3.0
    print(float32)


fn poc_print_float64():
    alias float64: Float64 = 3.0
    print(float64)


fn poc_print_static_string():
    print("print-static-string")


fn poc_print_input_string(input: String):
    print(input)


# ── String I/O ───────────────────────────────────────────────────────────────


fn poc_return_input_string(x: String) -> String:
    return x


fn poc_return_static_string() -> String:
    return "return-static-string"


fn poc_string_length(x: String) -> Int64:
    return Int64(len(x))


fn poc_string_concat(x: String, y: String) -> String:
    return x + y


fn poc_string_repeat(x: String, n: Int32) -> String:
    var result = String("")
    for _ in range(Int(n)):
        result += x
    return result


fn poc_string_eq(x: String, y: String) -> Bool:
    return x == y

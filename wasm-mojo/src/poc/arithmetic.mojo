# PoC Arithmetic — add, sub, mul, div, mod, pow, neg, abs, min, max, clamp


# ── Add ──────────────────────────────────────────────────────────────────────


fn poc_add_int32(x: Int32, y: Int32) -> Int32:
    return x + y


fn poc_add_int64(x: Int64, y: Int64) -> Int64:
    return x + y


fn poc_add_float32(x: Float32, y: Float32) -> Float32:
    return x + y


fn poc_add_float64(x: Float64, y: Float64) -> Float64:
    return x + y


# ── Subtract ─────────────────────────────────────────────────────────────────


fn poc_sub_int32(x: Int32, y: Int32) -> Int32:
    return x - y


fn poc_sub_int64(x: Int64, y: Int64) -> Int64:
    return x - y


fn poc_sub_float32(x: Float32, y: Float32) -> Float32:
    return x - y


fn poc_sub_float64(x: Float64, y: Float64) -> Float64:
    return x - y


# ── Multiply ─────────────────────────────────────────────────────────────────


fn poc_mul_int32(x: Int32, y: Int32) -> Int32:
    return x * y


fn poc_mul_int64(x: Int64, y: Int64) -> Int64:
    return x * y


fn poc_mul_float32(x: Float32, y: Float32) -> Float32:
    return x * y


fn poc_mul_float64(x: Float64, y: Float64) -> Float64:
    return x * y


# ── Division ─────────────────────────────────────────────────────────────────


fn poc_div_int32(x: Int32, y: Int32) -> Int32:
    return x // y


fn poc_div_int64(x: Int64, y: Int64) -> Int64:
    return x // y


fn poc_div_float32(x: Float32, y: Float32) -> Float32:
    return x / y


fn poc_div_float64(x: Float64, y: Float64) -> Float64:
    return x / y


# ── Modulo ───────────────────────────────────────────────────────────────────


fn poc_mod_int32(x: Int32, y: Int32) -> Int32:
    return x % y


fn poc_mod_int64(x: Int64, y: Int64) -> Int64:
    return x % y


# ── Power ────────────────────────────────────────────────────────────────────


fn poc_pow_int32(x: Int32) -> Int32:
    return x**x


fn poc_pow_int64(x: Int64) -> Int64:
    return x**x


fn poc_pow_float32(x: Float32) -> Float32:
    return x**x


fn poc_pow_float64(x: Float64) -> Float64:
    return x**x


# ── Negate ───────────────────────────────────────────────────────────────────


fn poc_neg_int32(x: Int32) -> Int32:
    return -x


fn poc_neg_int64(x: Int64) -> Int64:
    return -x


fn poc_neg_float32(x: Float32) -> Float32:
    return -x


fn poc_neg_float64(x: Float64) -> Float64:
    return -x


# ── Absolute value ───────────────────────────────────────────────────────────


fn poc_abs_int32(x: Int32) -> Int32:
    if x < 0:
        return -x
    return x


fn poc_abs_int64(x: Int64) -> Int64:
    if x < 0:
        return -x
    return x


fn poc_abs_float32(x: Float32) -> Float32:
    if x < 0:
        return -x
    return x


fn poc_abs_float64(x: Float64) -> Float64:
    if x < 0:
        return -x
    return x


# ── Min / Max ────────────────────────────────────────────────────────────────


fn poc_min_int32(x: Int32, y: Int32) -> Int32:
    if x < y:
        return x
    return y


fn poc_max_int32(x: Int32, y: Int32) -> Int32:
    if x > y:
        return x
    return y


fn poc_min_int64(x: Int64, y: Int64) -> Int64:
    if x < y:
        return x
    return y


fn poc_max_int64(x: Int64, y: Int64) -> Int64:
    if x > y:
        return x
    return y


fn poc_min_float64(x: Float64, y: Float64) -> Float64:
    if x < y:
        return x
    return y


fn poc_max_float64(x: Float64, y: Float64) -> Float64:
    if x > y:
        return x
    return y


# ── Clamp ────────────────────────────────────────────────────────────────────


fn poc_clamp_int32(x: Int32, lo: Int32, hi: Int32) -> Int32:
    if x < lo:
        return lo
    if x > hi:
        return hi
    return x


fn poc_clamp_float64(x: Float64, lo: Float64, hi: Float64) -> Float64:
    if x < lo:
        return lo
    if x > hi:
        return hi
    return x

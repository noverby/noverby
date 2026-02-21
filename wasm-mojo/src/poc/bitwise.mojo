# PoC Bitwise — bitand, bitor, bitxor, bitnot, shl, shr


fn poc_bitand_int32(x: Int32, y: Int32) -> Int32:
    return x & y


fn poc_bitor_int32(x: Int32, y: Int32) -> Int32:
    return x | y


fn poc_bitxor_int32(x: Int32, y: Int32) -> Int32:
    return x ^ y


fn poc_bitnot_int32(x: Int32) -> Int32:
    return ~x


fn poc_shl_int32(x: Int32, y: Int32) -> Int32:
    return x << y


fn poc_shr_int32(x: Int32, y: Int32) -> Int32:
    return x >> y

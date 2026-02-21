# PoC Comparison — eq, ne, lt, le, gt, ge, bool_and, bool_or, bool_not


fn poc_eq_int32(x: Int32, y: Int32) -> Bool:
    return x == y


fn poc_ne_int32(x: Int32, y: Int32) -> Bool:
    return x != y


fn poc_lt_int32(x: Int32, y: Int32) -> Bool:
    return x < y


fn poc_le_int32(x: Int32, y: Int32) -> Bool:
    return x <= y


fn poc_gt_int32(x: Int32, y: Int32) -> Bool:
    return x > y


fn poc_ge_int32(x: Int32, y: Int32) -> Bool:
    return x >= y


fn poc_bool_and(x: Bool, y: Bool) -> Bool:
    return x and y


fn poc_bool_or(x: Bool, y: Bool) -> Bool:
    return x or y


fn poc_bool_not(x: Bool) -> Bool:
    return not x

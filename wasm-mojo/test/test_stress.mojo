# Port of test/stress.test.ts — allocator stress tests exercised through the
# real WASM binary via wasmtime-py (called from Mojo via Python interop).
#
# Tests cover:
# - Sequential string allocations and readback
# - String roundtrip pipeline (return_input_string)
# - Repeated concat operations
# - Interleaved numeric and string operations
# - Empty struct allocations (non-overlapping)
# - Mixed-size string allocations
# - Fibonacci sequence consistency
# - string_eq reflexivity
#
# Run with:
#   mojo test test/test_stress.mojo

from python import Python, PythonObject
from testing import assert_true, assert_equal


fn _get_wasm() raises -> PythonObject:
    Python.add_to_path("test")
    var harness = Python.import_module("wasm_harness")
    return harness.get_instance()


# ---------------------------------------------------------------------------
# Helper — pure-Mojo GCD for reference
# ---------------------------------------------------------------------------


fn _gcd(var a: Int, var b: Int) -> Int:
    if a < 0:
        a = -a
    if b < 0:
        b = -b
    while b != 0:
        var t = b
        b = a % b
        a = t
    return a


fn _i32_wrap(x: Int) -> Int:
    """Wrap to signed 32-bit integer (matching WASM i32 semantics)."""
    var v = x & 0xFFFFFFFF
    if v >= 0x80000000:
        v -= 0x100000000
    return v


# ---------------------------------------------------------------------------
# Allocator stress — many sequential string allocations
# ---------------------------------------------------------------------------


fn test_200_sequential_string_allocations() raises:
    """Write 200 distinct strings and verify they all read back correctly."""
    var w = _get_wasm()
    var ptrs = Python.evaluate("[]")
    var strings = Python.evaluate("[]")
    for i in range(200):
        var s = (
            PythonObject("string-")
            + PythonObject(String(i))
            + PythonObject("-")
            + PythonObject("x") * PythonObject(i % 30)
        )
        _ = strings.append(s)
        _ = ptrs.append(w.write_string_struct(s))

    # Read them all back — earlier allocations must not be corrupted
    for i in range(200):
        var result = w.read_string_struct(ptrs[i])
        assert_true(
            Bool(result == strings[i]),
            String("readback string #") + String(i) + " after 200 allocs",
        )


# ---------------------------------------------------------------------------
# Allocator stress — many alloc + return_input_string roundtrips
# ---------------------------------------------------------------------------


fn test_100_return_input_string_roundtrips() raises:
    var w = _get_wasm()
    for i in range(100):
        var s = PythonObject("roundtrip-") + PythonObject(String(i))
        var in_ptr = w.write_string_struct(s)
        var out_ptr = w.alloc_string_struct()
        _ = w.return_input_string(in_ptr, out_ptr)
        var result = w.read_string_struct(out_ptr)
        assert_true(
            Bool(result == s),
            String("roundtrip #") + String(i) + " failed",
        )


# ---------------------------------------------------------------------------
# Allocator stress — many concat operations
# ---------------------------------------------------------------------------


fn test_50_sequential_concats() raises:
    """Build a string by concatenating 'ab' 50 times through WASM."""
    var w = _get_wasm()
    var current_ptr = w.write_string_struct("")
    for _ in range(50):
        var append_ptr = w.write_string_struct("ab")
        var out_ptr = w.alloc_string_struct()
        _ = w.string_concat(current_ptr, append_ptr, out_ptr)
        current_ptr = out_ptr

    var result = w.read_string_struct(current_ptr)
    var expected = PythonObject("ab") * PythonObject(50)
    assert_true(
        Bool(result == expected),
        "50 sequential concats produce correct result",
    )
    assert_equal(
        Int(w.string_length(current_ptr)),
        100,
        "50 sequential concats produce 100-byte string",
    )


# ---------------------------------------------------------------------------
# Allocator stress — interleaved numeric and string operations
# ---------------------------------------------------------------------------


fn test_50_interleaved_numeric_string_ops() raises:
    var w = _get_wasm()
    for i in range(50):
        # Do some numeric work
        var x = w.add_int32(i, i)
        var y = w.mul_int32(i, 3)
        var g = w.gcd_int32(x, y)

        # Do a string roundtrip
        var s = (
            PythonObject("iter-")
            + PythonObject(String(i))
            + PythonObject("-gcd-")
            + PythonObject(String(Int(g)))
        )
        var in_ptr = w.write_string_struct(s)
        var out_ptr = w.alloc_string_struct()
        _ = w.return_input_string(in_ptr, out_ptr)
        var result = w.read_string_struct(out_ptr)

        # Verify numeric result
        var expected_gcd = _gcd(Int(i) + Int(i), Int(i) * 3)
        assert_equal(
            Int(g),
            expected_gcd,
            String("gcd at iteration ") + String(i),
        )

        # Verify string result
        assert_true(
            Bool(result == s),
            String("string roundtrip at iteration ") + String(i),
        )


# ---------------------------------------------------------------------------
# Allocator stress — many small allocStringStruct calls
# ---------------------------------------------------------------------------


fn test_300_alloc_string_struct_non_overlapping() raises:
    """Verify no two struct pointers overlap (each struct is 24 bytes)."""
    var w = _get_wasm()
    var ptrs = Python.evaluate("[]")
    for _ in range(300):
        _ = ptrs.append(w.alloc_string_struct())

    for i in range(1, 300):
        var gap = Int(ptrs[i]) - Int(ptrs[i - 1])
        assert_true(
            gap >= 24,
            String("struct #")
            + String(i)
            + " overlaps with #"
            + String(i - 1)
            + " (gap="
            + String(gap)
            + ")",
        )


# ---------------------------------------------------------------------------
# Allocator stress — mixed-size string allocations
# ---------------------------------------------------------------------------


fn test_mixed_size_strings_report_correct_length() raises:
    var w = _get_wasm()
    var sizes = List[Int](0, 1, 5, 22, 23, 24, 25, 50, 100, 255, 1, 0, 23, 24)
    var ptrs = Python.evaluate("[]")

    for i in range(len(sizes)):
        var size = sizes[i]
        var s = PythonObject("m") * PythonObject(size)
        _ = ptrs.append(w.write_string_struct(s))

    for i in range(len(sizes)):
        var size = sizes[i]
        var length = Int(w.string_length(ptrs[i]))
        assert_equal(
            length,
            size,
            String("length of ") + String(size) + "-byte string",
        )


# ---------------------------------------------------------------------------
# Computation stress — fibonacci consistency across many values
# ---------------------------------------------------------------------------


fn test_fib_recurrence_2_to_40() raises:
    """Verify fib(n) = fib(n-1) + fib(n-2) for n = 2..40."""
    var w = _get_wasm()
    for n in range(2, 41):
        var fn0 = Int(w.fib_int32(n))
        var fn1 = Int(w.fib_int32(n - 1))
        var fn2 = Int(w.fib_int32(n - 2))
        # Use i32 wrapping addition for the check (matches WASM semantics)
        var expected = _i32_wrap(fn1 + fn2)
        assert_equal(
            fn0,
            expected,
            String("fib(")
            + String(n)
            + ") === fib("
            + String(n - 1)
            + ") + fib("
            + String(n - 2)
            + ")",
        )


# ---------------------------------------------------------------------------
# Computation stress — string_eq reflexivity for many strings
# ---------------------------------------------------------------------------


fn test_string_eq_reflexive() raises:
    var w = _get_wasm()
    var test_strings = List[String](
        "",
        "a",
        "hello",
        "x" * 23,
        "y" * 24,
        "café",
        String("Hello, World! "),
        "z" * 100,
    )
    for i in range(len(test_strings)):
        var s = PythonObject(test_strings[i])
        var a_ptr = w.write_string_struct(s)
        var b_ptr = w.write_string_struct(s)
        var eq = Int(w.string_eq(a_ptr, b_ptr))
        assert_equal(
            eq,
            1,
            String("string_eq reflexive for string #") + String(i),
        )

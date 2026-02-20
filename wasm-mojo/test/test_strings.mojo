# String operations exercised through the real WASM binary via
# wasmtime-mojo (pure Mojo FFI bindings — no Python interop required).
#
# These tests verify that string identity, length, concatenation, repeat, and
# equality operations work correctly when compiled to WASM and executed via
# the Wasmtime runtime.
#
# Run with:
#   mojo test test/test_strings.mojo

from memory import UnsafePointer
from testing import assert_equal, assert_true

from wasm_harness import (
    WasmInstance,
    get_instance,
    args_ptr,
    args_ptr_ptr,
    args_ptr_ptr_ptr,
    args_ptr_i32_ptr,
    no_args,
)


fn _get_wasm() raises -> UnsafePointer[WasmInstance]:
    return get_instance()


# ── Return static string ────────────────────────────────────────────────────


fn test_return_static_string() raises:
    var w = _get_wasm()
    var out_ptr = w[].alloc_string_struct()
    w[].call_void("return_static_string", args_ptr(out_ptr))
    var result = w[].read_string_struct(out_ptr)
    assert_equal(
        result,
        "return-static-string",
        'return_static_string === "return-static-string"',
    )


# ── Return input string ─────────────────────────────────────────────────────


fn test_return_input_string_basic() raises:
    var w = _get_wasm()
    var expected = "return-input-string"
    var in_ptr = w[].write_string_struct(expected)
    var out_ptr = w[].alloc_string_struct()
    w[].call_void("return_input_string", args_ptr_ptr(in_ptr, out_ptr))
    var result = w[].read_string_struct(out_ptr)
    assert_equal(
        result, expected, 'return_input_string === "return-input-string"'
    )


fn test_return_input_string_empty() raises:
    var w = _get_wasm()
    var expected = ""
    var in_ptr = w[].write_string_struct(expected)
    var out_ptr = w[].alloc_string_struct()
    w[].call_void("return_input_string", args_ptr_ptr(in_ptr, out_ptr))
    var result = w[].read_string_struct(out_ptr)
    assert_equal(
        result, expected, 'return_input_string("") === "" (empty string)'
    )


fn test_return_input_string_single_char() raises:
    var w = _get_wasm()
    var expected = "a"
    var in_ptr = w[].write_string_struct(expected)
    var out_ptr = w[].alloc_string_struct()
    w[].call_void("return_input_string", args_ptr_ptr(in_ptr, out_ptr))
    var result = w[].read_string_struct(out_ptr)
    assert_equal(
        result, expected, 'return_input_string("a") === "a" (single char)'
    )


fn test_return_input_string_emoji() raises:
    var w = _get_wasm()
    var expected = String("Hello, World! 🌍")
    var in_ptr = w[].write_string_struct(expected)
    var out_ptr = w[].alloc_string_struct()
    w[].call_void("return_input_string", args_ptr_ptr(in_ptr, out_ptr))
    var result = w[].read_string_struct(out_ptr)
    assert_equal(
        result,
        expected,
        "return_input_string with emoji roundtrip",
    )


# ── String length ────────────────────────────────────────────────────────────


fn test_string_length_hello() raises:
    var w = _get_wasm()
    var ptr = w[].write_string_struct("hello")
    assert_equal(
        Int(w[].call_i64("string_length", args_ptr(ptr))),
        5,
        'string_length("hello") === 5',
    )


fn test_string_length_empty() raises:
    var w = _get_wasm()
    var ptr = w[].write_string_struct("")
    assert_equal(
        Int(w[].call_i64("string_length", args_ptr(ptr))),
        0,
        'string_length("") === 0',
    )


fn test_string_length_single_char() raises:
    var w = _get_wasm()
    var ptr = w[].write_string_struct("a")
    assert_equal(
        Int(w[].call_i64("string_length", args_ptr(ptr))),
        1,
        'string_length("a") === 1',
    )


fn test_string_length_ten_chars() raises:
    var w = _get_wasm()
    var ptr = w[].write_string_struct("abcdefghij")
    assert_equal(
        Int(w[].call_i64("string_length", args_ptr(ptr))),
        10,
        'string_length("abcdefghij") === 10',
    )


fn test_string_length_utf8_emoji() raises:
    var w = _get_wasm()
    # UTF-8 multibyte: 🌍 is 4 bytes
    var ptr = w[].write_string_struct(String("🌍"))
    assert_equal(
        Int(w[].call_i64("string_length", args_ptr(ptr))),
        4,
        'string_length("🌍") === 4 (UTF-8 bytes)',
    )


# ── String concatenation ────────────────────────────────────────────────────


fn test_string_concat_basic() raises:
    var w = _get_wasm()
    var a_ptr = w[].write_string_struct("hello")
    var b_ptr = w[].write_string_struct(" world")
    var out_ptr = w[].alloc_string_struct()
    w[].call_void("string_concat", args_ptr_ptr_ptr(a_ptr, b_ptr, out_ptr))
    var result = w[].read_string_struct(out_ptr)
    assert_equal(
        result,
        "hello world",
        'string_concat("hello", " world") === "hello world"',
    )


fn test_string_concat_empty_first() raises:
    var w = _get_wasm()
    var a_ptr = w[].write_string_struct("")
    var b_ptr = w[].write_string_struct("world")
    var out_ptr = w[].alloc_string_struct()
    w[].call_void("string_concat", args_ptr_ptr_ptr(a_ptr, b_ptr, out_ptr))
    var result = w[].read_string_struct(out_ptr)
    assert_equal(
        result,
        "world",
        'string_concat("", "world") === "world"',
    )


fn test_string_concat_empty_second() raises:
    var w = _get_wasm()
    var a_ptr = w[].write_string_struct("hello")
    var b_ptr = w[].write_string_struct("")
    var out_ptr = w[].alloc_string_struct()
    w[].call_void("string_concat", args_ptr_ptr_ptr(a_ptr, b_ptr, out_ptr))
    var result = w[].read_string_struct(out_ptr)
    assert_equal(
        result,
        "hello",
        'string_concat("hello", "") === "hello"',
    )


fn test_string_concat_both_empty() raises:
    var w = _get_wasm()
    var a_ptr = w[].write_string_struct("")
    var b_ptr = w[].write_string_struct("")
    var out_ptr = w[].alloc_string_struct()
    w[].call_void("string_concat", args_ptr_ptr_ptr(a_ptr, b_ptr, out_ptr))
    var result = w[].read_string_struct(out_ptr)
    assert_equal(result, "", 'string_concat("", "") === ""')


fn test_string_concat_short() raises:
    var w = _get_wasm()
    var a_ptr = w[].write_string_struct("foo")
    var b_ptr = w[].write_string_struct("bar")
    var out_ptr = w[].alloc_string_struct()
    w[].call_void("string_concat", args_ptr_ptr_ptr(a_ptr, b_ptr, out_ptr))
    var result = w[].read_string_struct(out_ptr)
    assert_equal(
        result,
        "foobar",
        'string_concat("foo", "bar") === "foobar"',
    )


# ── String repeat ────────────────────────────────────────────────────────────


fn test_string_repeat_basic() raises:
    var w = _get_wasm()
    var ptr = w[].write_string_struct("ab")
    var out_ptr = w[].alloc_string_struct()
    w[].call_void("string_repeat", args_ptr_i32_ptr(ptr, 3, out_ptr))
    var result = w[].read_string_struct(out_ptr)
    assert_equal(
        result,
        "ababab",
        'string_repeat("ab", 3) === "ababab"',
    )


fn test_string_repeat_one() raises:
    var w = _get_wasm()
    var ptr = w[].write_string_struct("x")
    var out_ptr = w[].alloc_string_struct()
    w[].call_void("string_repeat", args_ptr_i32_ptr(ptr, 1, out_ptr))
    var result = w[].read_string_struct(out_ptr)
    assert_equal(result, "x", 'string_repeat("x", 1) === "x"')


fn test_string_repeat_zero() raises:
    var w = _get_wasm()
    var ptr = w[].write_string_struct("abc")
    var out_ptr = w[].alloc_string_struct()
    w[].call_void("string_repeat", args_ptr_i32_ptr(ptr, 0, out_ptr))
    var result = w[].read_string_struct(out_ptr)
    assert_equal(result, "", 'string_repeat("abc", 0) === ""')


fn test_string_repeat_five() raises:
    var w = _get_wasm()
    var ptr = w[].write_string_struct("ha")
    var out_ptr = w[].alloc_string_struct()
    w[].call_void("string_repeat", args_ptr_i32_ptr(ptr, 5, out_ptr))
    var result = w[].read_string_struct(out_ptr)
    assert_equal(
        result,
        "hahahahaha",
        'string_repeat("ha", 5) === "hahahahaha"',
    )


# ── String equality ──────────────────────────────────────────────────────────


fn test_string_eq_same() raises:
    var w = _get_wasm()
    var a_ptr = w[].write_string_struct("hello")
    var b_ptr = w[].write_string_struct("hello")
    assert_equal(
        Int(w[].call_i32("string_eq", args_ptr_ptr(a_ptr, b_ptr))),
        1,
        'string_eq("hello", "hello") === true',
    )


fn test_string_eq_different() raises:
    var w = _get_wasm()
    var a_ptr = w[].write_string_struct("hello")
    var b_ptr = w[].write_string_struct("world")
    assert_equal(
        Int(w[].call_i32("string_eq", args_ptr_ptr(a_ptr, b_ptr))),
        0,
        'string_eq("hello", "world") === false',
    )


fn test_string_eq_both_empty() raises:
    var w = _get_wasm()
    var a_ptr = w[].write_string_struct("")
    var b_ptr = w[].write_string_struct("")
    assert_equal(
        Int(w[].call_i32("string_eq", args_ptr_ptr(a_ptr, b_ptr))),
        1,
        'string_eq("", "") === true',
    )


fn test_string_eq_prefix() raises:
    var w = _get_wasm()
    var a_ptr = w[].write_string_struct("hello")
    var b_ptr = w[].write_string_struct("hell")
    assert_equal(
        Int(w[].call_i32("string_eq", args_ptr_ptr(a_ptr, b_ptr))),
        0,
        'string_eq("hello", "hell") === false (prefix)',
    )


fn test_string_eq_case_sensitive() raises:
    var w = _get_wasm()
    var a_ptr = w[].write_string_struct("abc")
    var b_ptr = w[].write_string_struct("ABC")
    assert_equal(
        Int(w[].call_i32("string_eq", args_ptr_ptr(a_ptr, b_ptr))),
        0,
        'string_eq("abc", "ABC") === false (case sensitive)',
    )

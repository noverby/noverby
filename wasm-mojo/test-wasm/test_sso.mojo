# Port of test/sso.test.ts — String SSO (Small String Optimization) boundary
# tests exercised through the real WASM binary via wasmtime-py (called from
# Mojo via Python interop).
#
# Mojo's Small String Optimization stores strings inline in the 24-byte struct
# when they fit (<=23 bytes). At 24+ bytes the data is heap-allocated. These
# tests exercise the boundary to verify that string operations work correctly
# across the SSO/heap transition.
#
# Run with:
#   mojo test test-wasm/test_sso.mojo

from python import Python, PythonObject
from testing import assert_true, assert_equal


fn _get_wasm() raises -> PythonObject:
    Python.add_to_path("test-wasm")
    var harness = Python.import_module("wasm_harness")
    return harness.get_instance()


# ---------------------------------------------------------------------------
# SSO roundtrip via return_input_string
# ---------------------------------------------------------------------------


fn test_roundtrip_22_bytes_sso() raises:
    """22 bytes: comfortably within SSO."""
    var w = _get_wasm()
    var s = PythonObject("a" * 22)
    var in_ptr = w.write_string_struct(s)
    var out_ptr = w.alloc_string_struct()
    _ = w.return_input_string(in_ptr, out_ptr)
    assert_true(
        Bool(w.read_string_struct(out_ptr) == s),
        "return_input_string 22-byte string (SSO)",
    )


fn test_roundtrip_23_bytes_sso_max() raises:
    """23 bytes: max SSO capacity."""
    var w = _get_wasm()
    var s = PythonObject("b" * 23)
    var in_ptr = w.write_string_struct(s)
    var out_ptr = w.alloc_string_struct()
    _ = w.return_input_string(in_ptr, out_ptr)
    assert_true(
        Bool(w.read_string_struct(out_ptr) == s),
        "return_input_string 23-byte string (SSO max)",
    )


fn test_roundtrip_24_bytes_heap() raises:
    """24 bytes: first heap-allocated size."""
    var w = _get_wasm()
    var s = PythonObject("c" * 24)
    var in_ptr = w.write_string_struct(s)
    var out_ptr = w.alloc_string_struct()
    _ = w.return_input_string(in_ptr, out_ptr)
    assert_true(
        Bool(w.read_string_struct(out_ptr) == s),
        "return_input_string 24-byte string (heap)",
    )


fn test_roundtrip_25_bytes_heap() raises:
    """25 bytes: safely past the boundary."""
    var w = _get_wasm()
    var s = PythonObject("d" * 25)
    var in_ptr = w.write_string_struct(s)
    var out_ptr = w.alloc_string_struct()
    _ = w.return_input_string(in_ptr, out_ptr)
    assert_true(
        Bool(w.read_string_struct(out_ptr) == s),
        "return_input_string 25-byte string (heap)",
    )


# ---------------------------------------------------------------------------
# SSO boundary — string_length
# ---------------------------------------------------------------------------


fn test_length_22_sso() raises:
    var w = _get_wasm()
    var ptr = w.write_string_struct("x" * 22)
    assert_equal(Int(w.string_length(ptr)), 22, "string_length 22-byte (SSO)")


fn test_length_23_sso_max() raises:
    var w = _get_wasm()
    var ptr = w.write_string_struct("x" * 23)
    assert_equal(
        Int(w.string_length(ptr)), 23, "string_length 23-byte (SSO max)"
    )


fn test_length_24_heap() raises:
    var w = _get_wasm()
    var ptr = w.write_string_struct("x" * 24)
    assert_equal(Int(w.string_length(ptr)), 24, "string_length 24-byte (heap)")


# ---------------------------------------------------------------------------
# SSO boundary — string_eq
# ---------------------------------------------------------------------------


fn test_eq_23_identical_sso() raises:
    """Both SSO."""
    var w = _get_wasm()
    var a_ptr = w.write_string_struct("y" * 23)
    var b_ptr = w.write_string_struct("y" * 23)
    assert_equal(
        Int(w.string_eq(a_ptr, b_ptr)),
        1,
        "string_eq 23-byte identical (SSO === SSO)",
    )


fn test_eq_23_vs_24_different_length() raises:
    """SSO vs heap: different lengths should not match."""
    var w = _get_wasm()
    var a_ptr = w.write_string_struct("z" * 23)
    var b_ptr = w.write_string_struct("z" * 24)
    assert_equal(
        Int(w.string_eq(a_ptr, b_ptr)),
        0,
        "string_eq 23-byte vs 24-byte (SSO !== heap, different length)",
    )


fn test_eq_24_identical_heap() raises:
    """Both heap."""
    var w = _get_wasm()
    var a_ptr = w.write_string_struct("w" * 24)
    var b_ptr = w.write_string_struct("w" * 24)
    assert_equal(
        Int(w.string_eq(a_ptr, b_ptr)),
        1,
        "string_eq 24-byte identical (heap === heap)",
    )


fn test_eq_23_differ_last_byte_sso() raises:
    """Same length at boundary, different content."""
    var w = _get_wasm()
    var a_ptr = w.write_string_struct("a" * 23)
    var b_ptr = w.write_string_struct("a" * 22 + "b")
    assert_equal(
        Int(w.string_eq(a_ptr, b_ptr)),
        0,
        "string_eq 23-byte differ in last byte (SSO)",
    )


# ---------------------------------------------------------------------------
# SSO boundary — string_concat crossing the boundary
# ---------------------------------------------------------------------------


fn test_concat_11_plus_12_eq_23_sso() raises:
    """Two small strings that concat to exactly 23 bytes (SSO)."""
    var w = _get_wasm()
    var a_ptr = w.write_string_struct("a" * 11)
    var b_ptr = w.write_string_struct("b" * 12)
    var out_ptr = w.alloc_string_struct()
    _ = w.string_concat(a_ptr, b_ptr, out_ptr)
    var result = String(w.read_string_struct(out_ptr))
    assert_equal(
        result,
        "a" * 11 + "b" * 12,
        "string_concat 11+12=23 bytes (result at SSO max)",
    )
    assert_equal(
        Int(w.string_length(out_ptr)),
        23,
        "string_concat result length === 23",
    )


fn test_concat_12_plus_12_eq_24_heap() raises:
    """Two small strings that concat to exactly 24 bytes (crosses to heap)."""
    var w = _get_wasm()
    var a_ptr = w.write_string_struct("a" * 12)
    var b_ptr = w.write_string_struct("b" * 12)
    var out_ptr = w.alloc_string_struct()
    _ = w.string_concat(a_ptr, b_ptr, out_ptr)
    var result = String(w.read_string_struct(out_ptr))
    assert_equal(
        result,
        "a" * 12 + "b" * 12,
        "string_concat 12+12=24 bytes (result crosses to heap)",
    )
    assert_equal(
        Int(w.string_length(out_ptr)),
        24,
        "string_concat result length === 24",
    )


# ---------------------------------------------------------------------------
# SSO boundary — string_repeat crossing the boundary
# ---------------------------------------------------------------------------


fn test_repeat_8x3_eq_24_heap() raises:
    """8 * 3 = 24 bytes -> heap."""
    var w = _get_wasm()
    var ptr = w.write_string_struct("a" * 8)
    var out_ptr = w.alloc_string_struct()
    _ = w.string_repeat(ptr, 3, out_ptr)
    var result = String(w.read_string_struct(out_ptr))
    assert_equal(
        result,
        "a" * 24,
        "string_repeat 8-byte * 3 = 24 bytes (crosses to heap)",
    )


fn test_repeat_23x1_stays_sso() raises:
    """23 * 1 = 23 bytes -> stays SSO."""
    var w = _get_wasm()
    var ptr = w.write_string_struct("q" * 23)
    var out_ptr = w.alloc_string_struct()
    _ = w.string_repeat(ptr, 1, out_ptr)
    var result = String(w.read_string_struct(out_ptr))
    assert_equal(
        result,
        "q" * 23,
        "string_repeat 23-byte * 1 = 23 bytes (stays SSO)",
    )


# ---------------------------------------------------------------------------
# Larger heap strings (well past SSO)
# ---------------------------------------------------------------------------


fn test_roundtrip_150_bytes() raises:
    var w = _get_wasm()
    var s = PythonObject("abc" * 50)  # 150 bytes
    var in_ptr = w.write_string_struct(s)
    var out_ptr = w.alloc_string_struct()
    _ = w.return_input_string(in_ptr, out_ptr)
    assert_true(
        Bool(w.read_string_struct(out_ptr) == s),
        "return_input_string 150-byte string (well past SSO)",
    )


fn test_length_256_bytes() raises:
    var w = _get_wasm()
    var s = PythonObject("x" * 256)
    var ptr = w.write_string_struct(s)
    assert_equal(
        Int(w.string_length(ptr)), 256, "string_length 256-byte (heap)"
    )

# Port of test/print.test.ts — verifies print functions execute without error
# through the real WASM binary via wasmtime-py (called from Mojo via Python interop).
#
# The TypeScript version just calls each print function and checks that they
# don't throw.  We do the same here, plus verify that print_input_string
# handles a string struct correctly.
#
# Run with:
#   mojo test test/test_print.mojo

from python import Python, PythonObject
from testing import assert_true


fn _get_wasm() raises -> PythonObject:
    Python.add_to_path("test")
    var harness = Python.import_module("wasm_harness")
    return harness.get_instance()


# ---------------------------------------------------------------------------
# Print (static values) — just verify no crash
# ---------------------------------------------------------------------------


fn test_print_static_string() raises:
    var w = _get_wasm()
    _ = w.print_static_string()


fn test_print_int32() raises:
    var w = _get_wasm()
    _ = w.print_int32()


fn test_print_int64() raises:
    var w = _get_wasm()
    _ = w.print_int64()


fn test_print_float32() raises:
    var w = _get_wasm()
    _ = w.print_float32()


fn test_print_float64() raises:
    var w = _get_wasm()
    _ = w.print_float64()


# ---------------------------------------------------------------------------
# Print input string
# ---------------------------------------------------------------------------


fn test_print_input_string() raises:
    var w = _get_wasm()
    var struct_ptr = w.write_string_struct("print-input-string")
    _ = w.print_input_string(struct_ptr)

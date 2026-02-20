"""
Port of test/print.test.ts — verifies print functions execute without error
through the real WASM binary via wasmtime-py.

The TypeScript version just calls each print function and checks that they
don't throw.  We do the same here, plus verify that print_input_string
handles a string struct correctly.

Run with:
    uv run --with wasmtime --with pytest pytest test-wasm/test_print.py
"""

from conftest import WasmInstance

# ---------------------------------------------------------------------------
# Print (static values) — just verify no crash
# ---------------------------------------------------------------------------


class TestPrintStatic:
    def test_print_static_string(self, w: WasmInstance):
        w.print_static_string()

    def test_print_int32(self, w: WasmInstance):
        w.print_int32()

    def test_print_int64(self, w: WasmInstance):
        w.print_int64()

    def test_print_float32(self, w: WasmInstance):
        w.print_float32()

    def test_print_float64(self, w: WasmInstance):
        w.print_float64()


# ---------------------------------------------------------------------------
# Print input string
# ---------------------------------------------------------------------------


class TestPrintInputString:
    def test_print_input_string(self, w: WasmInstance):
        struct_ptr = w.write_string_struct("print-input-string")
        w.print_input_string(struct_ptr)

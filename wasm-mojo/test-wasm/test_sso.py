"""
Port of test/sso.test.ts — String SSO (Small String Optimization) boundary
tests exercised through the real WASM binary via wasmtime-py.

Mojo's Small String Optimization stores strings inline in the 24-byte struct
when they fit (≤23 bytes). At 24+ bytes the data is heap-allocated. These
tests exercise the boundary to verify that string operations work correctly
across the SSO/heap transition.

Run with:
    uv run --with wasmtime --with pytest pytest test-wasm/test_sso.py
"""

import pytest
from conftest import WasmInstance

# ---------------------------------------------------------------------------
# SSO roundtrip via return_input_string
# ---------------------------------------------------------------------------


class TestSSOBoundaryRoundtrip:
    def test_roundtrip_22_bytes_sso(self, w: WasmInstance):
        """22 bytes: comfortably within SSO."""
        s = "a" * 22
        in_ptr = w.write_string_struct(s)
        out_ptr = w.alloc_string_struct()
        w.return_input_string(in_ptr, out_ptr)
        assert w.read_string_struct(out_ptr) == s, (
            "return_input_string 22-byte string (SSO)"
        )

    def test_roundtrip_23_bytes_sso_max(self, w: WasmInstance):
        """23 bytes: max SSO capacity."""
        s = "b" * 23
        in_ptr = w.write_string_struct(s)
        out_ptr = w.alloc_string_struct()
        w.return_input_string(in_ptr, out_ptr)
        assert w.read_string_struct(out_ptr) == s, (
            "return_input_string 23-byte string (SSO max)"
        )

    def test_roundtrip_24_bytes_heap(self, w: WasmInstance):
        """24 bytes: first heap-allocated size."""
        s = "c" * 24
        in_ptr = w.write_string_struct(s)
        out_ptr = w.alloc_string_struct()
        w.return_input_string(in_ptr, out_ptr)
        assert w.read_string_struct(out_ptr) == s, (
            "return_input_string 24-byte string (heap)"
        )

    def test_roundtrip_25_bytes_heap(self, w: WasmInstance):
        """25 bytes: safely past the boundary."""
        s = "d" * 25
        in_ptr = w.write_string_struct(s)
        out_ptr = w.alloc_string_struct()
        w.return_input_string(in_ptr, out_ptr)
        assert w.read_string_struct(out_ptr) == s, (
            "return_input_string 25-byte string (heap)"
        )


# ---------------------------------------------------------------------------
# SSO boundary — string_length
# ---------------------------------------------------------------------------


class TestSSOBoundaryLength:
    def test_length_22_sso(self, w: WasmInstance):
        ptr = w.write_string_struct("x" * 22)
        assert w.string_length(ptr) == 22, "string_length 22-byte (SSO)"

    def test_length_23_sso_max(self, w: WasmInstance):
        ptr = w.write_string_struct("x" * 23)
        assert w.string_length(ptr) == 23, "string_length 23-byte (SSO max)"

    def test_length_24_heap(self, w: WasmInstance):
        ptr = w.write_string_struct("x" * 24)
        assert w.string_length(ptr) == 24, "string_length 24-byte (heap)"


# ---------------------------------------------------------------------------
# SSO boundary — string_eq
# ---------------------------------------------------------------------------


class TestSSOBoundaryEquality:
    def test_eq_23_identical_sso(self, w: WasmInstance):
        """Both SSO."""
        a_ptr = w.write_string_struct("y" * 23)
        b_ptr = w.write_string_struct("y" * 23)
        assert w.string_eq(a_ptr, b_ptr) == 1, (
            "string_eq 23-byte identical (SSO === SSO)"
        )

    def test_eq_23_vs_24_different_length(self, w: WasmInstance):
        """SSO vs heap: different lengths should not match."""
        a_ptr = w.write_string_struct("z" * 23)
        b_ptr = w.write_string_struct("z" * 24)
        assert w.string_eq(a_ptr, b_ptr) == 0, (
            "string_eq 23-byte vs 24-byte (SSO !== heap, different length)"
        )

    def test_eq_24_identical_heap(self, w: WasmInstance):
        """Both heap."""
        a_ptr = w.write_string_struct("w" * 24)
        b_ptr = w.write_string_struct("w" * 24)
        assert w.string_eq(a_ptr, b_ptr) == 1, (
            "string_eq 24-byte identical (heap === heap)"
        )

    def test_eq_23_differ_last_byte_sso(self, w: WasmInstance):
        """Same length at boundary, different content."""
        a_ptr = w.write_string_struct("a" * 23)
        b_ptr = w.write_string_struct("a" * 22 + "b")
        assert w.string_eq(a_ptr, b_ptr) == 0, (
            "string_eq 23-byte differ in last byte (SSO)"
        )


# ---------------------------------------------------------------------------
# SSO boundary — string_concat crossing the boundary
# ---------------------------------------------------------------------------


class TestSSOBoundaryConcat:
    def test_concat_11_plus_12_eq_23_sso(self, w: WasmInstance):
        """Two small strings that concat to exactly 23 bytes (SSO)."""
        a_ptr = w.write_string_struct("a" * 11)
        b_ptr = w.write_string_struct("b" * 12)
        out_ptr = w.alloc_string_struct()
        w.string_concat(a_ptr, b_ptr, out_ptr)
        result = w.read_string_struct(out_ptr)
        assert result == "a" * 11 + "b" * 12, (
            "string_concat 11+12=23 bytes (result at SSO max)"
        )
        assert w.string_length(out_ptr) == 23, "string_concat result length === 23"

    def test_concat_12_plus_12_eq_24_heap(self, w: WasmInstance):
        """Two small strings that concat to exactly 24 bytes (crosses to heap)."""
        a_ptr = w.write_string_struct("a" * 12)
        b_ptr = w.write_string_struct("b" * 12)
        out_ptr = w.alloc_string_struct()
        w.string_concat(a_ptr, b_ptr, out_ptr)
        result = w.read_string_struct(out_ptr)
        assert result == "a" * 12 + "b" * 12, (
            "string_concat 12+12=24 bytes (result crosses to heap)"
        )
        assert w.string_length(out_ptr) == 24, "string_concat result length === 24"


# ---------------------------------------------------------------------------
# SSO boundary — string_repeat crossing the boundary
# ---------------------------------------------------------------------------


class TestSSOBoundaryRepeat:
    def test_repeat_8x3_eq_24_heap(self, w: WasmInstance):
        """8 * 3 = 24 bytes → heap."""
        ptr = w.write_string_struct("a" * 8)
        out_ptr = w.alloc_string_struct()
        w.string_repeat(ptr, 3, out_ptr)
        result = w.read_string_struct(out_ptr)
        assert result == "a" * 24, (
            "string_repeat 8-byte * 3 = 24 bytes (crosses to heap)"
        )

    def test_repeat_23x1_stays_sso(self, w: WasmInstance):
        """23 * 1 = 23 bytes → stays SSO."""
        ptr = w.write_string_struct("q" * 23)
        out_ptr = w.alloc_string_struct()
        w.string_repeat(ptr, 1, out_ptr)
        result = w.read_string_struct(out_ptr)
        assert result == "q" * 23, "string_repeat 23-byte * 1 = 23 bytes (stays SSO)"


# ---------------------------------------------------------------------------
# Larger heap strings (well past SSO)
# ---------------------------------------------------------------------------


class TestSSOLargerHeap:
    def test_roundtrip_150_bytes(self, w: WasmInstance):
        s = "abc" * 50  # 150 bytes
        in_ptr = w.write_string_struct(s)
        out_ptr = w.alloc_string_struct()
        w.return_input_string(in_ptr, out_ptr)
        result = w.read_string_struct(out_ptr)
        assert result == s, "return_input_string 150-byte string (well past SSO)"

    def test_length_256_bytes(self, w: WasmInstance):
        s = "x" * 256
        ptr = w.write_string_struct(s)
        assert w.string_length(ptr) == 256, "string_length 256-byte (heap)"

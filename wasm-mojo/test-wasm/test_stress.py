"""
Port of test/stress.test.ts — allocator stress tests exercised through the
real WASM binary via wasmtime-py.

Tests cover:
- Sequential string allocations and readback
- String roundtrip pipeline (return_input_string)
- Repeated concat operations
- Interleaved numeric and string operations
- Empty struct allocations (non-overlapping)
- Mixed-size string allocations
- Fibonacci sequence consistency
- string_eq reflexivity

Run with:
    uv run --with wasmtime --with pytest pytest test-wasm/test_stress.py
"""

import math

from conftest import WasmInstance

# ---------------------------------------------------------------------------
# Helper — pure-Python GCD for reference
# ---------------------------------------------------------------------------


def _gcd(a: int, b: int) -> int:
    a, b = abs(a), abs(b)
    while b:
        a, b = b, a % b
    return a


def _i32_wrap(x: int) -> int:
    """Wrap to signed 32-bit integer (matching WASM i32 semantics)."""
    x = x & 0xFFFFFFFF
    if x >= 0x80000000:
        x -= 0x100000000
    return x


# ---------------------------------------------------------------------------
# Allocator stress — many sequential string allocations
# ---------------------------------------------------------------------------


class TestStressSequentialStringAllocations:
    def test_200_sequential_string_allocations(self, w: WasmInstance):
        """Write 200 distinct strings and verify they all read back correctly."""
        ptrs = []
        strings = []
        for i in range(200):
            s = f"string-{i}-{'x' * (i % 30)}"
            strings.append(s)
            ptrs.append(w.write_string_struct(s))

        # Read them all back — earlier allocations must not be corrupted
        for i in range(200):
            result = w.read_string_struct(ptrs[i])
            assert result == strings[i], f"readback string #{i} after 200 allocs"


# ---------------------------------------------------------------------------
# Allocator stress — many alloc + return_input_string roundtrips
# ---------------------------------------------------------------------------


class TestStressStringRoundtripPipeline:
    def test_100_return_input_string_roundtrips(self, w: WasmInstance):
        for i in range(100):
            s = f"roundtrip-{i}"
            in_ptr = w.write_string_struct(s)
            out_ptr = w.alloc_string_struct()
            w.return_input_string(in_ptr, out_ptr)
            result = w.read_string_struct(out_ptr)
            assert result == s, f"roundtrip #{i} failed"


# ---------------------------------------------------------------------------
# Allocator stress — many concat operations
# ---------------------------------------------------------------------------


class TestStressRepeatedConcat:
    def test_50_sequential_concats(self, w: WasmInstance):
        """Build a string by concatenating 'ab' 50 times through WASM."""
        current_ptr = w.write_string_struct("")
        for i in range(50):
            append_ptr = w.write_string_struct("ab")
            out_ptr = w.alloc_string_struct()
            w.string_concat(current_ptr, append_ptr, out_ptr)
            current_ptr = out_ptr

        result = w.read_string_struct(current_ptr)
        assert result == "ab" * 50, "50 sequential concats produce correct result"
        assert w.string_length(current_ptr) == 100, (
            "50 sequential concats produce 100-byte string"
        )


# ---------------------------------------------------------------------------
# Allocator stress — interleaved numeric and string operations
# ---------------------------------------------------------------------------


class TestStressInterleavedOps:
    def test_50_interleaved_numeric_string_ops(self, w: WasmInstance):
        for i in range(50):
            # Do some numeric work
            x = w.add_int32(i, i)
            y = w.mul_int32(i, 3)
            g = w.gcd_int32(x, y)

            # Do a string roundtrip
            s = f"iter-{i}-gcd-{g}"
            in_ptr = w.write_string_struct(s)
            out_ptr = w.alloc_string_struct()
            w.return_input_string(in_ptr, out_ptr)
            result = w.read_string_struct(out_ptr)

            # Verify numeric result
            expected_gcd = _gcd(i + i, i * 3)
            assert g == expected_gcd, f"gcd at iteration {i}"

            # Verify string result
            assert result == s, f"string roundtrip at iteration {i}"


# ---------------------------------------------------------------------------
# Allocator stress — many small allocStringStruct calls
# ---------------------------------------------------------------------------


class TestStressEmptyStructAllocations:
    def test_300_alloc_string_struct_non_overlapping(self, w: WasmInstance):
        """Verify no two struct pointers overlap (each struct is 24 bytes)."""
        ptrs = []
        for _ in range(300):
            ptrs.append(w.alloc_string_struct())

        for i in range(1, len(ptrs)):
            gap = ptrs[i] - ptrs[i - 1]
            assert gap >= 24, f"struct #{i} overlaps with #{i - 1} (gap={gap})"


# ---------------------------------------------------------------------------
# Allocator stress — mixed-size string allocations
# ---------------------------------------------------------------------------


class TestStressMixedSizeAllocations:
    def test_mixed_size_strings_report_correct_length(self, w: WasmInstance):
        sizes = [0, 1, 5, 22, 23, 24, 25, 50, 100, 255, 1, 0, 23, 24]
        ptrs = []

        for size in sizes:
            s = "m" * size
            ptrs.append(w.write_string_struct(s))

        for i, size in enumerate(sizes):
            length = w.string_length(ptrs[i])
            assert length == size, f"length of {size}-byte string"


# ---------------------------------------------------------------------------
# Computation stress — fibonacci consistency across many values
# ---------------------------------------------------------------------------


class TestStressFibonacciConsistency:
    def test_fib_recurrence_2_to_40(self, w: WasmInstance):
        """Verify fib(n) = fib(n-1) + fib(n-2) for n = 2..40."""
        for n in range(2, 41):
            fn0 = w.fib_int32(n)
            fn1 = w.fib_int32(n - 1)
            fn2 = w.fib_int32(n - 2)
            # Use i32 wrapping addition for the check (matches WASM semantics)
            expected = _i32_wrap(fn1 + fn2)
            assert fn0 == expected, f"fib({n}) === fib({n - 1}) + fib({n - 2})"


# ---------------------------------------------------------------------------
# Computation stress — string_eq reflexivity for many strings
# ---------------------------------------------------------------------------


class TestStressStringEqReflexivity:
    def test_string_eq_reflexive(self, w: WasmInstance):
        test_strings = [
            "",
            "a",
            "hello",
            "x" * 23,
            "y" * 24,
            "café",
            "🌍🌎🌏",
            "Hello, World! 🎉",
            "z" * 100,
        ]
        for s in test_strings:
            a_ptr = w.write_string_struct(s)
            b_ptr = w.write_string_struct(s)
            eq = w.string_eq(a_ptr, b_ptr)
            assert eq == 1, f'string_eq reflexive for "{s[:20]}..."'

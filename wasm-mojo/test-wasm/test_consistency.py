"""
Port of test/consistency.test.ts — cross-function consistency checks exercised
through the real WASM binary via wasmtime-py.

These tests verify that different WASM-exported functions compose correctly:
add/sub inverses, mul/div inverses, neg/abs relationships, min/max ordering,
bitwise identities, shift roundtrips, De Morgan's law, GCD scaling,
Fibonacci/factorial recurrence, string concat length, and clamp equivalence.

Run with:
    uv run --with wasmtime --with pytest pytest test-wasm/test_consistency.py
"""

import math

import pytest
from conftest import WasmInstance

# ---------------------------------------------------------------------------
# Helper — pure-Python GCD for reference
# ---------------------------------------------------------------------------


def _gcd(a: int, b: int) -> int:
    a, b = abs(a), abs(b)
    while b:
        a, b = b, a % b
    return a


# ---------------------------------------------------------------------------
# Cross-function consistency checks
# ---------------------------------------------------------------------------


class TestCrossFunctionConsistency:
    def test_add_sub_inverse(self, w: WasmInstance):
        """sub(add(x, y), y) === x (add/sub inverse)"""
        x, y = 17, 9
        s = w.add_int32(x, y)
        assert w.sub_int32(s, y) == x, "sub(add(x, y), y) === x"

    def test_mul_div_inverse(self, w: WasmInstance):
        """div(mul(x, y), y) === x (mul/div inverse for exact division)"""
        x, y = 6, 3
        product = w.mul_int32(x, y)
        assert w.div_int32(product, y) == x, "div(mul(x, y), y) === x"

    def test_neg_neg_identity(self, w: WasmInstance):
        """neg(neg(x)) === x"""
        assert w.neg_int32(w.neg_int32(42)) == 42, "neg(neg(42)) === 42"

    def test_abs_neg_eq_abs(self, w: WasmInstance):
        """abs(neg(x)) === abs(x) for positive x"""
        assert w.abs_int32(w.neg_int32(7)) == w.abs_int32(7), "abs(neg(7)) === abs(7)"

    def test_min_le_max(self, w: WasmInstance):
        """min(x, y) <= max(x, y)"""
        a, b = 3, 7
        lo = w.min_int32(a, b)
        hi = w.max_int32(a, b)
        assert w.le_int32(lo, hi) == 1, "min(x,y) <= max(x,y)"

    def test_bitwise_identity_and_or_xor(self, w: WasmInstance):
        """(x & y) | (x ^ y) === x | y"""
        x, y = 0b1100, 0b1010
        lhs = w.bitor_int32(w.bitand_int32(x, y), w.bitxor_int32(x, y))
        rhs = w.bitor_int32(x, y)
        assert lhs == rhs, "(x & y) | (x ^ y) === x | y"

    def test_shl_shr_roundtrip(self, w: WasmInstance):
        """shr(shl(x, 4), 4) === x"""
        x = 5
        assert w.shr_int32(w.shl_int32(x, 4), 4) == x, "shr(shl(x, 4), 4) === x"

    def test_de_morgan(self, w: WasmInstance):
        """De Morgan: not(and(a,b)) === or(not(a), not(b))"""
        a, b = 1, 0
        assert w.bool_not(w.bool_and(a, b)) == w.bool_or(
            w.bool_not(a), w.bool_not(b)
        ), "De Morgan: not(and(a,b)) === or(not(a), not(b))"

    def test_gcd_scaling(self, w: WasmInstance):
        """gcd(a*k, b*k) === k * gcd(a, b)"""
        a, b, k = 6, 4, 5
        lhs = w.gcd_int32(w.mul_int32(a, k), w.mul_int32(b, k))
        rhs = w.mul_int32(k, w.gcd_int32(a, b))
        assert lhs == rhs, "gcd(a*k, b*k) === k * gcd(a, b)"

    @pytest.mark.parametrize("n", [5, 8, 12, 15])
    def test_fibonacci_recurrence(self, w: WasmInstance, n: int):
        """fib(n) === fib(n-1) + fib(n-2)"""
        fn2 = w.fib_int32(n - 2)
        fn1 = w.fib_int32(n - 1)
        fn0 = w.fib_int32(n)
        assert fn0 == fn1 + fn2, f"fib({n}) === fib({n - 1}) + fib({n - 2})"

    @pytest.mark.parametrize("n", [2, 3, 4, 5, 6, 7])
    def test_factorial_recurrence(self, w: WasmInstance, n: int):
        """factorial(n) === n * factorial(n-1)"""
        fn0 = w.factorial_int32(n)
        fn1 = w.factorial_int32(n - 1)
        assert fn0 == n * fn1, f"factorial({n}) === {n} * factorial({n - 1})"

    def test_string_concat_length(self, w: WasmInstance):
        """len(concat(a, b)) === len(a) + len(b)"""
        a_ptr = w.write_string_struct("foo")
        b_ptr = w.write_string_struct("barbaz")
        out_ptr = w.alloc_string_struct()
        w.string_concat(a_ptr, b_ptr, out_ptr)
        assert w.string_length(out_ptr) == w.string_length(a_ptr) + w.string_length(
            b_ptr
        ), "len(concat(a,b)) === len(a) + len(b)"

    @pytest.mark.parametrize("x", [-5, 0, 5, 10, 15])
    def test_clamp_eq_max_lo_min_hi_x(self, w: WasmInstance, x: int):
        """clamp(x, lo, hi) === max(lo, min(hi, x))"""
        lo, hi = 0, 10
        lhs = w.clamp_int32(x, lo, hi)
        rhs = w.max_int32(lo, w.min_int32(hi, x))
        assert lhs == rhs, f"clamp({x}, {lo}, {hi}) === max(lo, min(hi, x))"

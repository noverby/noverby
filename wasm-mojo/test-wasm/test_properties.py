"""
Port of test/properties.test.ts — algebraic property tests (commutativity,
associativity, distributivity, identity elements, annihilators, self-inverse,
De Morgan's laws, comparison duality) exercised through the real WASM binary
via wasmtime-py.

Run with:
    uv run --with wasmtime --with pytest pytest test-wasm/test_properties.py
"""

import pytest
from conftest import WasmInstance

# ---------------------------------------------------------------------------
# Commutativity — add
# ---------------------------------------------------------------------------


class TestCommutativityAdd:
    @pytest.mark.parametrize(
        "a, b",
        [
            (0, 0),
            (1, 2),
            (-7, 13),
            (100, -100),
            (2147483647, -2147483648),
            (12345, 67890),
        ],
    )
    def test_add_int32_commutes(self, w: WasmInstance, a: int, b: int):
        assert w.add_int32(a, b) == w.add_int32(b, a), (
            f"add_int32({a}, {b}) === add_int32({b}, {a})"
        )

    @pytest.mark.parametrize(
        "a, b",
        [
            (0, 0),
            (1, 2),
            (-999, 999),
            (9223372036854775807, -1),
        ],
    )
    def test_add_int64_commutes(self, w: WasmInstance, a: int, b: int):
        assert w.add_int64(a, b) == w.add_int64(b, a), (
            f"add_int64({a}, {b}) === add_int64({b}, {a})"
        )

    @pytest.mark.parametrize(
        "a, b",
        [
            (0.0, 0.0),
            (1.5, 2.5),
            (-3.14, 3.14),
            (1e10, 1e-10),
        ],
    )
    def test_add_float64_commutes(self, w: WasmInstance, a: float, b: float):
        assert w.add_float64(a, b) == w.add_float64(b, a), (
            f"add_float64({a}, {b}) === add_float64({b}, {a})"
        )


# ---------------------------------------------------------------------------
# Commutativity — mul
# ---------------------------------------------------------------------------


class TestCommutativityMul:
    @pytest.mark.parametrize(
        "a, b",
        [
            (0, 1),
            (3, 7),
            (-5, 11),
            (-4, -6),
            (2147483647, 2),
            (1000, 1000),
        ],
    )
    def test_mul_int32_commutes(self, w: WasmInstance, a: int, b: int):
        assert w.mul_int32(a, b) == w.mul_int32(b, a), (
            f"mul_int32({a}, {b}) === mul_int32({b}, {a})"
        )

    @pytest.mark.parametrize(
        "a, b",
        [
            (0, 1),
            (3, 7),
            (-100, 200),
        ],
    )
    def test_mul_int64_commutes(self, w: WasmInstance, a: int, b: int):
        assert w.mul_int64(a, b) == w.mul_int64(b, a), (
            f"mul_int64({a}, {b}) === mul_int64({b}, {a})"
        )

    @pytest.mark.parametrize(
        "a, b",
        [
            (2.5, 4.0),
            (-1.5, 3.0),
            (0.0, 999.0),
        ],
    )
    def test_mul_float64_commutes(self, w: WasmInstance, a: float, b: float):
        assert w.mul_float64(a, b) == w.mul_float64(b, a), (
            f"mul_float64({a}, {b}) === mul_float64({b}, {a})"
        )


# ---------------------------------------------------------------------------
# Commutativity — min / max
# ---------------------------------------------------------------------------


class TestCommutativityMinMax:
    @pytest.mark.parametrize(
        "a, b",
        [
            (3, 7),
            (-5, 5),
            (0, 0),
            (2147483647, -2147483648),
        ],
    )
    def test_min_int32_commutes(self, w: WasmInstance, a: int, b: int):
        assert w.min_int32(a, b) == w.min_int32(b, a), (
            f"min_int32({a}, {b}) === min_int32({b}, {a})"
        )

    @pytest.mark.parametrize(
        "a, b",
        [
            (3, 7),
            (-5, 5),
            (0, 0),
            (2147483647, -2147483648),
        ],
    )
    def test_max_int32_commutes(self, w: WasmInstance, a: int, b: int):
        assert w.max_int32(a, b) == w.max_int32(b, a), (
            f"max_int32({a}, {b}) === max_int32({b}, {a})"
        )


# ---------------------------------------------------------------------------
# Commutativity — GCD
# ---------------------------------------------------------------------------


class TestCommutativityGcd:
    @pytest.mark.parametrize(
        "a, b",
        [
            (12, 8),
            (7, 13),
            (100, 75),
            (0, 5),
            (1071, 462),
        ],
    )
    def test_gcd_int32_commutes(self, w: WasmInstance, a: int, b: int):
        assert w.gcd_int32(a, b) == w.gcd_int32(b, a), (
            f"gcd_int32({a}, {b}) === gcd_int32({b}, {a})"
        )


# ---------------------------------------------------------------------------
# Commutativity — bitwise and / or / xor
# ---------------------------------------------------------------------------


class TestCommutativityBitwise:
    @pytest.mark.parametrize(
        "a, b",
        [
            (0b1100, 0b1010),
            (0xFF, 0x0F),
            (0, -1),
            (2147483647, -2147483648),
        ],
    )
    def test_bitand_int32_commutes(self, w: WasmInstance, a: int, b: int):
        assert w.bitand_int32(a, b) == w.bitand_int32(b, a), (
            f"bitand_int32({a}, {b}) commutes"
        )

    @pytest.mark.parametrize(
        "a, b",
        [
            (0b1100, 0b1010),
            (0xFF, 0x0F),
            (0, -1),
            (2147483647, -2147483648),
        ],
    )
    def test_bitor_int32_commutes(self, w: WasmInstance, a: int, b: int):
        assert w.bitor_int32(a, b) == w.bitor_int32(b, a), (
            f"bitor_int32({a}, {b}) commutes"
        )

    @pytest.mark.parametrize(
        "a, b",
        [
            (0b1100, 0b1010),
            (0xFF, 0x0F),
            (0, -1),
            (2147483647, -2147483648),
        ],
    )
    def test_bitxor_int32_commutes(self, w: WasmInstance, a: int, b: int):
        assert w.bitxor_int32(a, b) == w.bitxor_int32(b, a), (
            f"bitxor_int32({a}, {b}) commutes"
        )


# ---------------------------------------------------------------------------
# Commutativity — boolean
# ---------------------------------------------------------------------------


class TestCommutativityBoolean:
    @pytest.mark.parametrize(
        "a, b",
        [(0, 0), (0, 1), (1, 0), (1, 1)],
    )
    def test_bool_and_commutes(self, w: WasmInstance, a: int, b: int):
        assert w.bool_and(a, b) == w.bool_and(b, a), f"bool_and({a}, {b}) commutes"

    @pytest.mark.parametrize(
        "a, b",
        [(0, 0), (0, 1), (1, 0), (1, 1)],
    )
    def test_bool_or_commutes(self, w: WasmInstance, a: int, b: int):
        assert w.bool_or(a, b) == w.bool_or(b, a), f"bool_or({a}, {b}) commutes"


# ---------------------------------------------------------------------------
# Commutativity — eq / ne
# ---------------------------------------------------------------------------


class TestCommutativityComparison:
    @pytest.mark.parametrize(
        "a, b",
        [
            (0, 0),
            (5, 6),
            (-1, 1),
            (2147483647, -2147483648),
        ],
    )
    def test_eq_int32_commutes(self, w: WasmInstance, a: int, b: int):
        assert w.eq_int32(a, b) == w.eq_int32(b, a), f"eq_int32({a}, {b}) commutes"

    @pytest.mark.parametrize(
        "a, b",
        [
            (0, 0),
            (5, 6),
            (-1, 1),
            (2147483647, -2147483648),
        ],
    )
    def test_ne_int32_commutes(self, w: WasmInstance, a: int, b: int):
        assert w.ne_int32(a, b) == w.ne_int32(b, a), f"ne_int32({a}, {b}) commutes"


# ---------------------------------------------------------------------------
# Associativity — add
# ---------------------------------------------------------------------------


class TestAssociativityAdd:
    @pytest.mark.parametrize(
        "a, b, c",
        [
            (1, 2, 3),
            (-5, 10, -3),
            (100, 200, 300),
            (0, 0, 0),
            (2147483647, 1, -1),
        ],
    )
    def test_add_int32_associative(self, w: WasmInstance, a: int, b: int, c: int):
        assert w.add_int32(w.add_int32(a, b), c) == w.add_int32(a, w.add_int32(b, c)), (
            f"add_int32 associative: ({a}+{b})+{c} === {a}+({b}+{c})"
        )

    @pytest.mark.parametrize(
        "a, b, c",
        [
            (1.0, 2.0, 4.0),
            (-1.0, 1.0, 0.0),
            (100.0, 200.0, 300.0),
        ],
    )
    def test_add_float64_associative(
        self, w: WasmInstance, a: float, b: float, c: float
    ):
        assert w.add_float64(w.add_float64(a, b), c) == w.add_float64(
            a, w.add_float64(b, c)
        ), f"add_float64 associative: ({a}+{b})+{c} === {a}+({b}+{c})"


# ---------------------------------------------------------------------------
# Associativity — mul
# ---------------------------------------------------------------------------


class TestAssociativityMul:
    @pytest.mark.parametrize(
        "a, b, c",
        [
            (2, 3, 4),
            (-1, 5, 7),
            (1, 1, 1),
            (0, 999, 123),
            (10, 10, 10),
        ],
    )
    def test_mul_int32_associative(self, w: WasmInstance, a: int, b: int, c: int):
        assert w.mul_int32(w.mul_int32(a, b), c) == w.mul_int32(a, w.mul_int32(b, c)), (
            f"mul_int32 associative: ({a}*{b})*{c} === {a}*({b}*{c})"
        )


# ---------------------------------------------------------------------------
# Associativity — bitwise and / or / xor
# ---------------------------------------------------------------------------


class TestAssociativityBitwise:
    @pytest.mark.parametrize(
        "a, b, c",
        [
            (0b1100, 0b1010, 0b0110),
            (0xFF, 0x0F, 0xAA),
            (0, -1, 42),
        ],
    )
    def test_bitand_int32_associative(self, w: WasmInstance, a: int, b: int, c: int):
        assert w.bitand_int32(w.bitand_int32(a, b), c) == w.bitand_int32(
            a, w.bitand_int32(b, c)
        ), f"bitand_int32 associative: ({a}&{b})&{c}"

    @pytest.mark.parametrize(
        "a, b, c",
        [
            (0b1100, 0b1010, 0b0110),
            (0xFF, 0x0F, 0xAA),
            (0, -1, 42),
        ],
    )
    def test_bitor_int32_associative(self, w: WasmInstance, a: int, b: int, c: int):
        assert w.bitor_int32(w.bitor_int32(a, b), c) == w.bitor_int32(
            a, w.bitor_int32(b, c)
        ), f"bitor_int32 associative: ({a}|{b})|{c}"

    @pytest.mark.parametrize(
        "a, b, c",
        [
            (0b1100, 0b1010, 0b0110),
            (0xFF, 0x0F, 0xAA),
            (0, -1, 42),
        ],
    )
    def test_bitxor_int32_associative(self, w: WasmInstance, a: int, b: int, c: int):
        assert w.bitxor_int32(w.bitxor_int32(a, b), c) == w.bitxor_int32(
            a, w.bitxor_int32(b, c)
        ), f"bitxor_int32 associative: ({a}^{b})^{c}"


# ---------------------------------------------------------------------------
# Distributivity — mul over add
# ---------------------------------------------------------------------------


class TestDistributivityMulOverAdd:
    @pytest.mark.parametrize(
        "a, b, c",
        [
            (2, 3, 4),
            (-3, 5, 7),
            (0, 100, 200),
            (1, -1, 1),
            (10, 10, 10),
            (7, 0, 0),
        ],
    )
    def test_mul_distributes_over_add(self, w: WasmInstance, a: int, b: int, c: int):
        lhs = w.mul_int32(a, w.add_int32(b, c))
        rhs = w.add_int32(w.mul_int32(a, b), w.mul_int32(a, c))
        assert lhs == rhs, f"mul_int32 distributes: {a}*({b}+{c}) === {a}*{b}+{a}*{c}"


# ---------------------------------------------------------------------------
# Distributivity — bitwise and over or
# ---------------------------------------------------------------------------


class TestDistributivityBitandOverBitor:
    @pytest.mark.parametrize(
        "a, b, c",
        [
            (0b1100, 0b1010, 0b0110),
            (0xFF, 0x0F, 0xF0),
            (-1, 42, 99),
            (0, 0xFFFF, 0xFF00),
        ],
    )
    def test_bitand_distributes_over_bitor(
        self, w: WasmInstance, a: int, b: int, c: int
    ):
        lhs = w.bitand_int32(a, w.bitor_int32(b, c))
        rhs = w.bitor_int32(w.bitand_int32(a, b), w.bitand_int32(a, c))
        assert lhs == rhs, f"bitand distributes over bitor: {a}&({b}|{c})"


# ---------------------------------------------------------------------------
# Distributivity — bitwise or over and
# ---------------------------------------------------------------------------


class TestDistributivityBitorOverBitand:
    @pytest.mark.parametrize(
        "a, b, c",
        [
            (0b1100, 0b1010, 0b0110),
            (0xFF, 0x0F, 0xF0),
            (0, 42, 99),
        ],
    )
    def test_bitor_distributes_over_bitand(
        self, w: WasmInstance, a: int, b: int, c: int
    ):
        lhs = w.bitor_int32(a, w.bitand_int32(b, c))
        rhs = w.bitand_int32(w.bitor_int32(a, b), w.bitor_int32(a, c))
        assert lhs == rhs, f"bitor distributes over bitand: {a}|({b}&{c})"


# ---------------------------------------------------------------------------
# Identity elements
# ---------------------------------------------------------------------------


class TestIdentityElements:
    @pytest.mark.parametrize("x", [-42, 0, 1, 2147483647, -2147483648])
    def test_add_identity(self, w: WasmInstance, x: int):
        assert w.add_int32(x, 0) == x, f"add_int32({x}, 0) === {x}"

    @pytest.mark.parametrize("x", [-42, 0, 1, 2147483647, -2147483648])
    def test_mul_identity(self, w: WasmInstance, x: int):
        assert w.mul_int32(x, 1) == x, f"mul_int32({x}, 1) === {x}"

    @pytest.mark.parametrize("x", [-42, 0, 1, 2147483647, -2147483648])
    def test_bitand_identity(self, w: WasmInstance, x: int):
        assert w.bitand_int32(x, -1) == x, f"bitand_int32({x}, -1) === {x}"

    @pytest.mark.parametrize("x", [-42, 0, 1, 2147483647, -2147483648])
    def test_bitor_identity(self, w: WasmInstance, x: int):
        assert w.bitor_int32(x, 0) == x, f"bitor_int32({x}, 0) === {x}"

    @pytest.mark.parametrize("x", [-42, 0, 1, 2147483647, -2147483648])
    def test_bitxor_identity(self, w: WasmInstance, x: int):
        assert w.bitxor_int32(x, 0) == x, f"bitxor_int32({x}, 0) === {x}"


# ---------------------------------------------------------------------------
# Annihilators / zero elements
# ---------------------------------------------------------------------------


class TestAnnihilators:
    @pytest.mark.parametrize("x", [-42, 0, 1, 2147483647, -2147483648])
    def test_mul_zero(self, w: WasmInstance, x: int):
        assert w.mul_int32(x, 0) == 0, f"mul_int32({x}, 0) === 0"

    @pytest.mark.parametrize("x", [-42, 0, 1, 2147483647, -2147483648])
    def test_bitand_zero(self, w: WasmInstance, x: int):
        assert w.bitand_int32(x, 0) == 0, f"bitand_int32({x}, 0) === 0"

    @pytest.mark.parametrize("x", [-42, 0, 1, 2147483647, -2147483648])
    def test_bitor_all_ones(self, w: WasmInstance, x: int):
        assert w.bitor_int32(x, -1) == -1, f"bitor_int32({x}, -1) === -1"


# ---------------------------------------------------------------------------
# Self-inverse / involution
# ---------------------------------------------------------------------------


class TestSelfInverse:
    @pytest.mark.parametrize("x", [-42, 0, 1, 99, 2147483647, -2147483648])
    def test_neg_neg(self, w: WasmInstance, x: int):
        assert w.neg_int32(w.neg_int32(x)) == x, f"neg_int32(neg_int32({x})) === {x}"

    @pytest.mark.parametrize("x", [-42, 0, 1, 99, 2147483647, -2147483648])
    def test_bitnot_bitnot(self, w: WasmInstance, x: int):
        assert w.bitnot_int32(w.bitnot_int32(x)) == x, (
            f"bitnot_int32(bitnot_int32({x})) === {x}"
        )

    @pytest.mark.parametrize("x", [0, 1])
    def test_bool_not_not(self, w: WasmInstance, x: int):
        assert w.bool_not(w.bool_not(x)) == x, f"bool_not(bool_not({x})) === {x}"

    @pytest.mark.parametrize(
        "x, y",
        [
            (42, 99),
            (0, -1),
            (2147483647, -2147483648),
        ],
    )
    def test_bitxor_self_inverse(self, w: WasmInstance, x: int, y: int):
        assert w.bitxor_int32(w.bitxor_int32(x, y), y) == x, (
            f"bitxor(bitxor({x}, {y}), {y}) === {x}"
        )


# ---------------------------------------------------------------------------
# De Morgan's laws — boolean
# ---------------------------------------------------------------------------


class TestDeMorganBoolean:
    @pytest.mark.parametrize("a", [0, 1])
    @pytest.mark.parametrize("b", [0, 1])
    def test_not_and_eq_or_not(self, w: WasmInstance, a: int, b: int):
        """not(a and b) === not(a) or not(b)"""
        assert w.bool_not(w.bool_and(a, b)) == w.bool_or(
            w.bool_not(a), w.bool_not(b)
        ), f"not({a} and {b}) === not({a}) or not({b})"

    @pytest.mark.parametrize("a", [0, 1])
    @pytest.mark.parametrize("b", [0, 1])
    def test_not_or_eq_and_not(self, w: WasmInstance, a: int, b: int):
        """not(a or b) === not(a) and not(b)"""
        assert w.bool_not(w.bool_or(a, b)) == w.bool_and(
            w.bool_not(a), w.bool_not(b)
        ), f"not({a} or {b}) === not({a}) and not({b})"


# ---------------------------------------------------------------------------
# De Morgan's laws — bitwise
# ---------------------------------------------------------------------------


class TestDeMorganBitwise:
    @pytest.mark.parametrize(
        "a, b",
        [
            (0b1100, 0b1010),
            (0xFF, 0x0F),
            (0, -1),
            (2147483647, -2147483648),
        ],
    )
    def test_bitnot_and_eq_or_bitnot(self, w: WasmInstance, a: int, b: int):
        """~(a & b) === ~a | ~b"""
        assert w.bitnot_int32(w.bitand_int32(a, b)) == w.bitor_int32(
            w.bitnot_int32(a), w.bitnot_int32(b)
        ), f"~({a} & {b}) === ~{a} | ~{b}"

    @pytest.mark.parametrize(
        "a, b",
        [
            (0b1100, 0b1010),
            (0xFF, 0x0F),
            (0, -1),
            (2147483647, -2147483648),
        ],
    )
    def test_bitnot_or_eq_and_bitnot(self, w: WasmInstance, a: int, b: int):
        """~(a | b) === ~a & ~b"""
        assert w.bitnot_int32(w.bitor_int32(a, b)) == w.bitand_int32(
            w.bitnot_int32(a), w.bitnot_int32(b)
        ), f"~({a} | {b}) === ~{a} & ~{b}"


# ---------------------------------------------------------------------------
# Comparison duality — lt vs ge, le vs gt
# ---------------------------------------------------------------------------


class TestComparisonDuality:
    @pytest.mark.parametrize(
        "a, b",
        [
            (3, 5),
            (5, 5),
            (7, 5),
            (-1, 0),
            (0, -1),
            (2147483647, -2147483648),
        ],
    )
    def test_lt_eq_not_ge(self, w: WasmInstance, a: int, b: int):
        assert w.lt_int32(a, b) == w.bool_not(w.ge_int32(a, b)), (
            f"lt({a}, {b}) === not(ge({a}, {b}))"
        )

    @pytest.mark.parametrize(
        "a, b",
        [
            (3, 5),
            (5, 5),
            (7, 5),
            (-1, 0),
            (0, -1),
            (2147483647, -2147483648),
        ],
    )
    def test_le_eq_not_gt(self, w: WasmInstance, a: int, b: int):
        assert w.le_int32(a, b) == w.bool_not(w.gt_int32(a, b)), (
            f"le({a}, {b}) === not(gt({a}, {b}))"
        )

    @pytest.mark.parametrize(
        "a, b",
        [
            (3, 5),
            (5, 5),
            (7, 5),
            (-1, 0),
            (0, -1),
            (2147483647, -2147483648),
        ],
    )
    def test_eq_iff_le_and_ge(self, w: WasmInstance, a: int, b: int):
        assert w.eq_int32(a, b) == w.bool_and(w.le_int32(a, b), w.ge_int32(a, b)), (
            f"eq({a}, {b}) === le({a}, {b}) and ge({a}, {b})"
        )

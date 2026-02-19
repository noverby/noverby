import type { WasmExports } from "../runtime/mod.ts";
import { assert, suite } from "./harness.ts";

export function testProperties(fns: WasmExports): void {
	// =================================================================
	// Commutativity — add
	// =================================================================
	suite("commutativity — add");
	for (const [a, b] of [
		[0, 0],
		[1, 2],
		[-7, 13],
		[100, -100],
		[2147483647, -2147483648],
		[12345, 67890],
	] as const) {
		assert(
			fns.add_int32(a, b),
			fns.add_int32(b, a),
			`add_int32(${a}, ${b}) === add_int32(${b}, ${a})`,
		);
	}
	for (const [a, b] of [
		[0n, 0n],
		[1n, 2n],
		[-999n, 999n],
		[9223372036854775807n, -1n],
	] as const) {
		assert(
			fns.add_int64(a, b),
			fns.add_int64(b, a),
			`add_int64(${a}, ${b}) === add_int64(${b}, ${a})`,
		);
	}
	for (const [a, b] of [
		[0.0, 0.0],
		[1.5, 2.5],
		[-3.14, 3.14],
		[1e10, 1e-10],
	] as const) {
		assert(
			fns.add_float64(a, b),
			fns.add_float64(b, a),
			`add_float64(${a}, ${b}) === add_float64(${b}, ${a})`,
		);
	}

	// =================================================================
	// Commutativity — mul
	// =================================================================
	suite("commutativity — mul");
	for (const [a, b] of [
		[0, 1],
		[3, 7],
		[-5, 11],
		[-4, -6],
		[2147483647, 2],
		[1000, 1000],
	] as const) {
		assert(
			fns.mul_int32(a, b),
			fns.mul_int32(b, a),
			`mul_int32(${a}, ${b}) === mul_int32(${b}, ${a})`,
		);
	}
	for (const [a, b] of [
		[0n, 1n],
		[3n, 7n],
		[-100n, 200n],
	] as const) {
		assert(
			fns.mul_int64(a, b),
			fns.mul_int64(b, a),
			`mul_int64(${a}, ${b}) === mul_int64(${b}, ${a})`,
		);
	}
	for (const [a, b] of [
		[2.5, 4.0],
		[-1.5, 3.0],
		[0.0, 999.0],
	] as const) {
		assert(
			fns.mul_float64(a, b),
			fns.mul_float64(b, a),
			`mul_float64(${a}, ${b}) === mul_float64(${b}, ${a})`,
		);
	}

	// =================================================================
	// Commutativity — min / max
	// =================================================================
	suite("commutativity — min/max");
	for (const [a, b] of [
		[3, 7],
		[-5, 5],
		[0, 0],
		[2147483647, -2147483648],
	] as const) {
		assert(
			fns.min_int32(a, b),
			fns.min_int32(b, a),
			`min_int32(${a}, ${b}) === min_int32(${b}, ${a})`,
		);
		assert(
			fns.max_int32(a, b),
			fns.max_int32(b, a),
			`max_int32(${a}, ${b}) === max_int32(${b}, ${a})`,
		);
	}

	// =================================================================
	// Commutativity — GCD
	// =================================================================
	suite("commutativity — gcd");
	for (const [a, b] of [
		[12, 8],
		[7, 13],
		[100, 75],
		[0, 5],
		[1071, 462],
	] as const) {
		assert(
			fns.gcd_int32(a, b),
			fns.gcd_int32(b, a),
			`gcd_int32(${a}, ${b}) === gcd_int32(${b}, ${a})`,
		);
	}

	// =================================================================
	// Commutativity — bitwise and / or / xor
	// =================================================================
	suite("commutativity — bitwise");
	for (const [a, b] of [
		[0b1100, 0b1010],
		[0xff, 0x0f],
		[0, -1],
		[2147483647, -2147483648],
	] as const) {
		assert(
			fns.bitand_int32(a, b),
			fns.bitand_int32(b, a),
			`bitand_int32(${a}, ${b}) commutes`,
		);
		assert(
			fns.bitor_int32(a, b),
			fns.bitor_int32(b, a),
			`bitor_int32(${a}, ${b}) commutes`,
		);
		assert(
			fns.bitxor_int32(a, b),
			fns.bitxor_int32(b, a),
			`bitxor_int32(${a}, ${b}) commutes`,
		);
	}

	// =================================================================
	// Commutativity — boolean
	// =================================================================
	suite("commutativity — boolean");
	for (const [a, b] of [
		[0, 0],
		[0, 1],
		[1, 0],
		[1, 1],
	] as const) {
		assert(
			fns.bool_and(a, b),
			fns.bool_and(b, a),
			`bool_and(${a}, ${b}) commutes`,
		);
		assert(
			fns.bool_or(a, b),
			fns.bool_or(b, a),
			`bool_or(${a}, ${b}) commutes`,
		);
	}

	// =================================================================
	// Commutativity — eq / ne
	// =================================================================
	suite("commutativity — comparison");
	for (const [a, b] of [
		[0, 0],
		[5, 6],
		[-1, 1],
		[2147483647, -2147483648],
	] as const) {
		assert(
			fns.eq_int32(a, b),
			fns.eq_int32(b, a),
			`eq_int32(${a}, ${b}) commutes`,
		);
		assert(
			fns.ne_int32(a, b),
			fns.ne_int32(b, a),
			`ne_int32(${a}, ${b}) commutes`,
		);
	}

	// =================================================================
	// Associativity — add
	// add(add(a, b), c) === add(a, add(b, c))
	// =================================================================
	suite("associativity — add");
	for (const [a, b, c] of [
		[1, 2, 3],
		[-5, 10, -3],
		[100, 200, 300],
		[0, 0, 0],
		[2147483647, 1, -1],
	] as const) {
		assert(
			fns.add_int32(fns.add_int32(a, b), c),
			fns.add_int32(a, fns.add_int32(b, c)),
			`add_int32 associative: (${a}+${b})+${c} === ${a}+(${b}+${c})`,
		);
	}
	for (const [a, b, c] of [
		[1.0, 2.0, 4.0],
		[-1.0, 1.0, 0.0],
		[100.0, 200.0, 300.0],
	] as const) {
		assert(
			fns.add_float64(fns.add_float64(a, b), c),
			fns.add_float64(a, fns.add_float64(b, c)),
			`add_float64 associative: (${a}+${b})+${c} === ${a}+(${b}+${c})`,
		);
	}

	// =================================================================
	// Associativity — mul
	// mul(mul(a, b), c) === mul(a, mul(b, c))
	// =================================================================
	suite("associativity — mul");
	for (const [a, b, c] of [
		[2, 3, 4],
		[-1, 5, 7],
		[1, 1, 1],
		[0, 999, 123],
		[10, 10, 10],
	] as const) {
		assert(
			fns.mul_int32(fns.mul_int32(a, b), c),
			fns.mul_int32(a, fns.mul_int32(b, c)),
			`mul_int32 associative: (${a}*${b})*${c} === ${a}*(${b}*${c})`,
		);
	}

	// =================================================================
	// Associativity — bitwise and / or / xor
	// =================================================================
	suite("associativity — bitwise");
	for (const [a, b, c] of [
		[0b1100, 0b1010, 0b0110],
		[0xff, 0x0f, 0xaa],
		[0, -1, 42],
	] as const) {
		assert(
			fns.bitand_int32(fns.bitand_int32(a, b), c),
			fns.bitand_int32(a, fns.bitand_int32(b, c)),
			`bitand_int32 associative: (${a}&${b})&${c}`,
		);
		assert(
			fns.bitor_int32(fns.bitor_int32(a, b), c),
			fns.bitor_int32(a, fns.bitor_int32(b, c)),
			`bitor_int32 associative: (${a}|${b})|${c}`,
		);
		assert(
			fns.bitxor_int32(fns.bitxor_int32(a, b), c),
			fns.bitxor_int32(a, fns.bitxor_int32(b, c)),
			`bitxor_int32 associative: (${a}^${b})^${c}`,
		);
	}

	// =================================================================
	// Distributivity — mul over add
	// mul(a, add(b, c)) === add(mul(a, b), mul(a, c))
	// =================================================================
	suite("distributivity — mul over add");
	for (const [a, b, c] of [
		[2, 3, 4],
		[-3, 5, 7],
		[0, 100, 200],
		[1, -1, 1],
		[10, 10, 10],
		[7, 0, 0],
	] as const) {
		assert(
			fns.mul_int32(a, fns.add_int32(b, c)),
			fns.add_int32(fns.mul_int32(a, b), fns.mul_int32(a, c)),
			`mul_int32 distributes: ${a}*(${b}+${c}) === ${a}*${b}+${a}*${c}`,
		);
	}

	// =================================================================
	// Distributivity — bitwise and over or
	// a & (b | c) === (a & b) | (a & c)
	// =================================================================
	suite("distributivity — bitwise and over or");
	for (const [a, b, c] of [
		[0b1100, 0b1010, 0b0110],
		[0xff, 0x0f, 0xf0],
		[-1, 42, 99],
		[0, 0xffff, 0xff00],
	] as const) {
		assert(
			fns.bitand_int32(a, fns.bitor_int32(b, c)),
			fns.bitor_int32(fns.bitand_int32(a, b), fns.bitand_int32(a, c)),
			`bitand distributes over bitor: ${a}&(${b}|${c})`,
		);
	}

	// =================================================================
	// Distributivity — bitwise or over and
	// a | (b & c) === (a | b) & (a | c)
	// =================================================================
	suite("distributivity — bitwise or over and");
	for (const [a, b, c] of [
		[0b1100, 0b1010, 0b0110],
		[0xff, 0x0f, 0xf0],
		[0, 42, 99],
	] as const) {
		assert(
			fns.bitor_int32(a, fns.bitand_int32(b, c)),
			fns.bitand_int32(fns.bitor_int32(a, b), fns.bitor_int32(a, c)),
			`bitor distributes over bitand: ${a}|(${b}&${c})`,
		);
	}

	// =================================================================
	// Identity elements
	// =================================================================
	suite("identity elements");
	for (const x of [-42, 0, 1, 2147483647, -2147483648]) {
		assert(fns.add_int32(x, 0), x, `add_int32(${x}, 0) === ${x}`);
		assert(fns.mul_int32(x, 1), x, `mul_int32(${x}, 1) === ${x}`);
		assert(fns.bitand_int32(x, -1), x, `bitand_int32(${x}, -1) === ${x}`);
		assert(fns.bitor_int32(x, 0), x, `bitor_int32(${x}, 0) === ${x}`);
		assert(fns.bitxor_int32(x, 0), x, `bitxor_int32(${x}, 0) === ${x}`);
	}

	// =================================================================
	// Annihilators / zero elements
	// =================================================================
	suite("annihilators");
	for (const x of [-42, 0, 1, 2147483647, -2147483648]) {
		assert(fns.mul_int32(x, 0), 0, `mul_int32(${x}, 0) === 0`);
		assert(fns.bitand_int32(x, 0), 0, `bitand_int32(${x}, 0) === 0`);
		assert(fns.bitor_int32(x, -1), -1, `bitor_int32(${x}, -1) === -1`);
	}

	// =================================================================
	// Self-inverse / involution
	// =================================================================
	suite("self-inverse");
	for (const x of [-42, 0, 1, 99, 2147483647, -2147483648]) {
		assert(
			fns.neg_int32(fns.neg_int32(x)),
			x,
			`neg_int32(neg_int32(${x})) === ${x}`,
		);
		assert(
			fns.bitnot_int32(fns.bitnot_int32(x)),
			x,
			`bitnot_int32(bitnot_int32(${x})) === ${x}`,
		);
	}
	for (const x of [0, 1]) {
		assert(
			fns.bool_not(fns.bool_not(x)),
			x,
			`bool_not(bool_not(${x})) === ${x}`,
		);
	}
	// XOR self-inverse: x ^ y ^ y === x
	for (const [x, y] of [
		[42, 99],
		[0, -1],
		[2147483647, -2147483648],
	] as const) {
		assert(
			fns.bitxor_int32(fns.bitxor_int32(x, y), y),
			x,
			`bitxor(bitxor(${x}, ${y}), ${y}) === ${x}`,
		);
	}

	// =================================================================
	// De Morgan's laws (full truth table)
	// not(a & b) === not(a) | not(b)
	// not(a | b) === not(a) & not(b)
	// =================================================================
	suite("De Morgan's laws — boolean");
	for (const a of [0, 1]) {
		for (const b of [0, 1]) {
			assert(
				fns.bool_not(fns.bool_and(a, b)),
				fns.bool_or(fns.bool_not(a), fns.bool_not(b)),
				`not(${a} and ${b}) === not(${a}) or not(${b})`,
			);
			assert(
				fns.bool_not(fns.bool_or(a, b)),
				fns.bool_and(fns.bool_not(a), fns.bool_not(b)),
				`not(${a} or ${b}) === not(${a}) and not(${b})`,
			);
		}
	}

	// =================================================================
	// De Morgan's laws — bitwise
	// ~(a & b) === (~a) | (~b)
	// ~(a | b) === (~a) & (~b)
	// =================================================================
	suite("De Morgan's laws — bitwise");
	for (const [a, b] of [
		[0b1100, 0b1010],
		[0xff, 0x0f],
		[0, -1],
		[2147483647, -2147483648],
	] as const) {
		assert(
			fns.bitnot_int32(fns.bitand_int32(a, b)),
			fns.bitor_int32(fns.bitnot_int32(a), fns.bitnot_int32(b)),
			`~(${a} & ${b}) === ~${a} | ~${b}`,
		);
		assert(
			fns.bitnot_int32(fns.bitor_int32(a, b)),
			fns.bitand_int32(fns.bitnot_int32(a), fns.bitnot_int32(b)),
			`~(${a} | ${b}) === ~${a} & ~${b}`,
		);
	}

	// =================================================================
	// Comparison duality — lt vs ge, le vs gt
	// lt(a, b) === not(ge(a, b))  and  le(a, b) === not(gt(a, b))
	// =================================================================
	suite("comparison duality");
	for (const [a, b] of [
		[3, 5],
		[5, 5],
		[7, 5],
		[-1, 0],
		[0, -1],
		[2147483647, -2147483648],
	] as const) {
		assert(
			fns.lt_int32(a, b),
			fns.bool_not(fns.ge_int32(a, b)),
			`lt(${a}, ${b}) === not(ge(${a}, ${b}))`,
		);
		assert(
			fns.le_int32(a, b),
			fns.bool_not(fns.gt_int32(a, b)),
			`le(${a}, ${b}) === not(gt(${a}, ${b}))`,
		);
		assert(
			fns.eq_int32(a, b),
			fns.bool_and(fns.le_int32(a, b), fns.ge_int32(a, b)),
			`eq(${a}, ${b}) === le(${a}, ${b}) and ge(${a}, ${b})`,
		);
	}
}

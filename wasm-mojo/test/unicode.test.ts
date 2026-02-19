import type { WasmExports } from "../runtime/mod.ts";
import {
	allocStringStruct,
	readStringStruct,
	writeStringStruct,
} from "../runtime/mod.ts";
import { assert, suite } from "./harness.ts";

export function testUnicode(fns: WasmExports): void {
	// =================================================================
	// Multi-byte UTF-8 sequences — length (byte count)
	// =================================================================
	suite("unicode — multi-byte length");

	// 1-byte: ASCII
	{
		const ptr = writeStringStruct("A");
		assert(
			fns.string_length(ptr),
			1n,
			'string_length("A") === 1 (1-byte ASCII)',
		);
	}
	// 2-byte: Latin Extended (é = U+00E9 = 0xC3 0xA9)
	{
		const ptr = writeStringStruct("é");
		assert(
			fns.string_length(ptr),
			2n,
			'string_length("é") === 2 (2-byte UTF-8)',
		);
	}
	// 3-byte: CJK (中 = U+4E2D = 0xE4 0xB8 0xAD)
	{
		const ptr = writeStringStruct("中");
		assert(
			fns.string_length(ptr),
			3n,
			'string_length("中") === 3 (3-byte UTF-8)',
		);
	}
	// 4-byte: Emoji (🌍 = U+1F30D = 0xF0 0x9F 0x8C 0x8D)
	{
		const ptr = writeStringStruct("🌍");
		assert(
			fns.string_length(ptr),
			4n,
			'string_length("🌍") === 4 (4-byte UTF-8)',
		);
	}
	// Mixed: all four byte widths in one string
	{
		// A(1) + é(2) + 中(3) + 🌍(4) = 10 bytes
		const ptr = writeStringStruct("Aé中🌍");
		assert(
			fns.string_length(ptr),
			10n,
			'string_length("Aé中🌍") === 10 (1+2+3+4 bytes)',
		);
	}

	// =================================================================
	// Multi-byte UTF-8 — roundtrip via return_input_string
	// =================================================================
	suite("unicode — multi-byte roundtrip");

	{
		const str = "café";
		const inPtr = writeStringStruct(str);
		const outPtr = allocStringStruct();
		fns.return_input_string(inPtr, outPtr);
		assert(readStringStruct(outPtr), str, 'roundtrip "café"');
	}
	{
		const str = "中文测试";
		const inPtr = writeStringStruct(str);
		const outPtr = allocStringStruct();
		fns.return_input_string(inPtr, outPtr);
		assert(readStringStruct(outPtr), str, 'roundtrip "中文测试"');
	}
	{
		const str = "日本語テスト";
		const inPtr = writeStringStruct(str);
		const outPtr = allocStringStruct();
		fns.return_input_string(inPtr, outPtr);
		assert(readStringStruct(outPtr), str, 'roundtrip "日本語テスト"');
	}
	{
		const str = "한국어";
		const inPtr = writeStringStruct(str);
		const outPtr = allocStringStruct();
		fns.return_input_string(inPtr, outPtr);
		assert(readStringStruct(outPtr), str, 'roundtrip "한국어"');
	}

	// =================================================================
	// Combining characters
	// =================================================================
	suite("unicode — combining characters");

	// Precomposed é (U+00E9) = 2 bytes
	{
		const precomposed = "\u00E9";
		const ptr = writeStringStruct(precomposed);
		assert(fns.string_length(ptr), 2n, "string_length(precomposed é) === 2");
	}
	// Decomposed é = e (1 byte) + combining acute accent U+0301 (2 bytes) = 3 bytes
	{
		const decomposed = "e\u0301";
		const ptr = writeStringStruct(decomposed);
		assert(
			fns.string_length(ptr),
			3n,
			"string_length(decomposed é) === 3 (e + combining accent)",
		);
	}
	// Precomposed and decomposed are NOT byte-equal
	{
		const aPtr = writeStringStruct("\u00E9");
		const bPtr = writeStringStruct("e\u0301");
		assert(
			fns.string_eq(aPtr, bPtr),
			0,
			"precomposed é !== decomposed e+◌́ (byte-level comparison)",
		);
	}
	// Roundtrip decomposed
	{
		const str = "e\u0301";
		const inPtr = writeStringStruct(str);
		const outPtr = allocStringStruct();
		fns.return_input_string(inPtr, outPtr);
		assert(
			readStringStruct(outPtr),
			str,
			"roundtrip decomposed e+combining accent",
		);
	}
	// Multiple combining marks: a + combining tilde + combining acute
	{
		const str = "a\u0303\u0301"; // ã́
		const ptr = writeStringStruct(str);
		// a(1) + U+0303(2) + U+0301(2) = 5 bytes
		assert(
			fns.string_length(ptr),
			5n,
			"string_length(a + combining tilde + combining acute) === 5",
		);
	}

	// =================================================================
	// Zero-width joiners (ZWJ) — emoji sequences
	// =================================================================
	suite("unicode — ZWJ sequences");

	// Family emoji: 👨‍👩‍👧‍👦 = U+1F468 ZWJ U+1F469 ZWJ U+1F467 ZWJ U+1F466
	// Each person emoji = 4 bytes, ZWJ (U+200D) = 3 bytes
	// 4 + 3 + 4 + 3 + 4 + 3 + 4 = 25 bytes
	{
		const family = "👨‍👩‍👧‍👦";
		const ptr = writeStringStruct(family);
		assert(
			fns.string_length(ptr),
			25n,
			'string_length("👨‍👩‍👧‍👦") === 25 (4 emoji + 3 ZWJ)',
		);
	}
	// Roundtrip ZWJ sequence
	{
		const family = "👨‍👩‍👧‍👦";
		const inPtr = writeStringStruct(family);
		const outPtr = allocStringStruct();
		fns.return_input_string(inPtr, outPtr);
		assert(readStringStruct(outPtr), family, "roundtrip ZWJ family emoji");
	}
	// Flag emoji: 🏳️‍🌈 = U+1F3F3 U+FE0F U+200D U+1F308
	// U+1F3F3(4) + U+FE0F(3) + U+200D(3) + U+1F308(4) = 14 bytes
	{
		const flag = "🏳️‍🌈";
		const ptr = writeStringStruct(flag);
		assert(
			fns.string_length(ptr),
			14n,
			'string_length("🏳️‍🌈") === 14 (flag + VS16 + ZWJ + rainbow)',
		);
	}
	// Person with skin tone: 👋🏽 = U+1F44B U+1F3FD
	// 4 + 4 = 8 bytes
	{
		const wave = "👋🏽";
		const ptr = writeStringStruct(wave);
		assert(
			fns.string_length(ptr),
			8n,
			'string_length("👋🏽") === 8 (wave + skin tone modifier)',
		);
	}

	// =================================================================
	// Right-to-left (RTL) text
	// =================================================================
	suite("unicode — RTL text");

	// Arabic
	{
		const str = "مرحبا";
		const inPtr = writeStringStruct(str);
		const outPtr = allocStringStruct();
		fns.return_input_string(inPtr, outPtr);
		assert(readStringStruct(outPtr), str, 'roundtrip Arabic "مرحبا"');
	}
	// Hebrew
	{
		const str = "שלום";
		const inPtr = writeStringStruct(str);
		const outPtr = allocStringStruct();
		fns.return_input_string(inPtr, outPtr);
		assert(readStringStruct(outPtr), str, 'roundtrip Hebrew "שלום"');
	}
	// Mixed LTR + RTL
	{
		const str = "Hello مرحبا World";
		const inPtr = writeStringStruct(str);
		const outPtr = allocStringStruct();
		fns.return_input_string(inPtr, outPtr);
		assert(readStringStruct(outPtr), str, "roundtrip mixed LTR/RTL");
	}

	// =================================================================
	// Unicode string operations — concat
	// =================================================================
	suite("unicode — concat");

	{
		const aPtr = writeStringStruct("café");
		const bPtr = writeStringStruct("☕");
		const outPtr = allocStringStruct();
		fns.string_concat(aPtr, bPtr, outPtr);
		assert(
			readStringStruct(outPtr),
			"café☕",
			'string_concat("café", "☕") === "café☕"',
		);
	}
	{
		const aPtr = writeStringStruct("🌍");
		const bPtr = writeStringStruct("🌎🌏");
		const outPtr = allocStringStruct();
		fns.string_concat(aPtr, bPtr, outPtr);
		assert(
			readStringStruct(outPtr),
			"🌍🌎🌏",
			'string_concat("🌍", "🌎🌏") === "🌍🌎🌏"',
		);
		assert(
			fns.string_length(outPtr),
			12n,
			"concat of 3 globe emoji === 12 bytes",
		);
	}

	// =================================================================
	// Unicode string operations — repeat
	// =================================================================
	suite("unicode — repeat");

	{
		const ptr = writeStringStruct("é");
		const outPtr = allocStringStruct();
		fns.string_repeat(ptr, 5, outPtr);
		assert(
			readStringStruct(outPtr),
			"ééééé",
			'string_repeat("é", 5) === "ééééé"',
		);
		assert(
			fns.string_length(outPtr),
			10n,
			"repeat 2-byte char 5 times === 10 bytes",
		);
	}
	{
		const ptr = writeStringStruct("🎉");
		const outPtr = allocStringStruct();
		fns.string_repeat(ptr, 3, outPtr);
		assert(
			readStringStruct(outPtr),
			"🎉🎉🎉",
			'string_repeat("🎉", 3) === "🎉🎉🎉"',
		);
		assert(
			fns.string_length(outPtr),
			12n,
			"repeat 4-byte emoji 3 times === 12 bytes",
		);
	}

	// =================================================================
	// Unicode string operations — equality
	// =================================================================
	suite("unicode — equality");

	{
		const aPtr = writeStringStruct("日本語");
		const bPtr = writeStringStruct("日本語");
		assert(
			fns.string_eq(aPtr, bPtr),
			1,
			'string_eq("日本語", "日本語") === true',
		);
	}
	{
		const aPtr = writeStringStruct("日本語");
		const bPtr = writeStringStruct("中文");
		assert(
			fns.string_eq(aPtr, bPtr),
			0,
			'string_eq("日本語", "中文") === false',
		);
	}
	{
		const aPtr = writeStringStruct("🌍");
		const bPtr = writeStringStruct("🌎");
		assert(
			fns.string_eq(aPtr, bPtr),
			0,
			'string_eq("🌍", "🌎") === false (different emoji)',
		);
	}
	{
		const aPtr = writeStringStruct("👨‍👩‍👧‍👦");
		const bPtr = writeStringStruct("👨‍👩‍👧‍👦");
		assert(
			fns.string_eq(aPtr, bPtr),
			1,
			"string_eq(ZWJ family, ZWJ family) === true",
		);
	}
}

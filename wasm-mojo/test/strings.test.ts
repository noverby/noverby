import type { WasmExports } from "../runtime/mod.ts";
import {
	allocStringStruct,
	readStringStruct,
	writeStringStruct,
} from "../runtime/mod.ts";
import { assert, suite } from "./harness.ts";

export function testStrings(fns: WasmExports): void {
	// =================================================================
	// Return static string
	// =================================================================
	suite("return_static_string");
	{
		const outPtr = allocStringStruct();
		fns.return_static_string(outPtr);
		const result = readStringStruct(outPtr);
		assert(
			result,
			"return-static-string",
			`return_static_string === "return-static-string"`,
		);
	}

	// =================================================================
	// Return input string
	// =================================================================
	suite("return_input_string");
	{
		const expectedString = "return-input-string";
		const inPtr = writeStringStruct(expectedString);
		const outPtr = allocStringStruct();
		fns.return_input_string(inPtr, outPtr);
		const result = readStringStruct(outPtr);
		assert(
			result,
			expectedString,
			`return_input_string === "${expectedString}"`,
		);
	}
	{
		const expectedString = "";
		const inPtr = writeStringStruct(expectedString);
		const outPtr = allocStringStruct();
		fns.return_input_string(inPtr, outPtr);
		const result = readStringStruct(outPtr);
		assert(
			result,
			expectedString,
			'return_input_string("") === "" (empty string)',
		);
	}
	{
		const expectedString = "a";
		const inPtr = writeStringStruct(expectedString);
		const outPtr = allocStringStruct();
		fns.return_input_string(inPtr, outPtr);
		const result = readStringStruct(outPtr);
		assert(
			result,
			expectedString,
			'return_input_string("a") === "a" (single char)',
		);
	}
	{
		const expectedString = "Hello, World! 🌍";
		const inPtr = writeStringStruct(expectedString);
		const outPtr = allocStringStruct();
		fns.return_input_string(inPtr, outPtr);
		const result = readStringStruct(outPtr);
		assert(result, expectedString, "return_input_string with emoji roundtrip");
	}

	// =================================================================
	// String length
	// =================================================================
	suite("string_length");
	{
		const ptr = writeStringStruct("hello");
		assert(fns.string_length(ptr), 5n, 'string_length("hello") === 5');
	}
	{
		const ptr = writeStringStruct("");
		assert(fns.string_length(ptr), 0n, 'string_length("") === 0');
	}
	{
		const ptr = writeStringStruct("a");
		assert(fns.string_length(ptr), 1n, 'string_length("a") === 1');
	}
	{
		const ptr = writeStringStruct("abcdefghij");
		assert(fns.string_length(ptr), 10n, 'string_length("abcdefghij") === 10');
	}
	{
		// UTF-8 multibyte: 🌍 is 4 bytes
		const ptr = writeStringStruct("🌍");
		assert(
			fns.string_length(ptr),
			4n,
			'string_length("🌍") === 4 (UTF-8 bytes)',
		);
	}

	// =================================================================
	// String concatenation
	// =================================================================
	suite("string_concat");
	{
		const aPtr = writeStringStruct("hello");
		const bPtr = writeStringStruct(" world");
		const outPtr = allocStringStruct();
		fns.string_concat(aPtr, bPtr, outPtr);
		assert(
			readStringStruct(outPtr),
			"hello world",
			'string_concat("hello", " world") === "hello world"',
		);
	}
	{
		const aPtr = writeStringStruct("");
		const bPtr = writeStringStruct("world");
		const outPtr = allocStringStruct();
		fns.string_concat(aPtr, bPtr, outPtr);
		assert(
			readStringStruct(outPtr),
			"world",
			'string_concat("", "world") === "world"',
		);
	}
	{
		const aPtr = writeStringStruct("hello");
		const bPtr = writeStringStruct("");
		const outPtr = allocStringStruct();
		fns.string_concat(aPtr, bPtr, outPtr);
		assert(
			readStringStruct(outPtr),
			"hello",
			'string_concat("hello", "") === "hello"',
		);
	}
	{
		const aPtr = writeStringStruct("");
		const bPtr = writeStringStruct("");
		const outPtr = allocStringStruct();
		fns.string_concat(aPtr, bPtr, outPtr);
		assert(readStringStruct(outPtr), "", 'string_concat("", "") === ""');
	}
	{
		const aPtr = writeStringStruct("foo");
		const bPtr = writeStringStruct("bar");
		const outPtr = allocStringStruct();
		fns.string_concat(aPtr, bPtr, outPtr);
		assert(
			readStringStruct(outPtr),
			"foobar",
			'string_concat("foo", "bar") === "foobar"',
		);
	}

	// =================================================================
	// String repeat
	// =================================================================
	suite("string_repeat");
	{
		const ptr = writeStringStruct("ab");
		const outPtr = allocStringStruct();
		fns.string_repeat(ptr, 3, outPtr);
		assert(
			readStringStruct(outPtr),
			"ababab",
			'string_repeat("ab", 3) === "ababab"',
		);
	}
	{
		const ptr = writeStringStruct("x");
		const outPtr = allocStringStruct();
		fns.string_repeat(ptr, 1, outPtr);
		assert(readStringStruct(outPtr), "x", 'string_repeat("x", 1) === "x"');
	}
	{
		const ptr = writeStringStruct("abc");
		const outPtr = allocStringStruct();
		fns.string_repeat(ptr, 0, outPtr);
		assert(readStringStruct(outPtr), "", 'string_repeat("abc", 0) === ""');
	}
	{
		const ptr = writeStringStruct("ha");
		const outPtr = allocStringStruct();
		fns.string_repeat(ptr, 5, outPtr);
		assert(
			readStringStruct(outPtr),
			"hahahahaha",
			'string_repeat("ha", 5) === "hahahahaha"',
		);
	}

	// =================================================================
	// String equality
	// =================================================================
	suite("string_eq");
	{
		const aPtr = writeStringStruct("hello");
		const bPtr = writeStringStruct("hello");
		assert(
			fns.string_eq(aPtr, bPtr),
			1,
			'string_eq("hello", "hello") === true',
		);
	}
	{
		const aPtr = writeStringStruct("hello");
		const bPtr = writeStringStruct("world");
		assert(
			fns.string_eq(aPtr, bPtr),
			0,
			'string_eq("hello", "world") === false',
		);
	}
	{
		const aPtr = writeStringStruct("");
		const bPtr = writeStringStruct("");
		assert(fns.string_eq(aPtr, bPtr), 1, 'string_eq("", "") === true');
	}
	{
		const aPtr = writeStringStruct("hello");
		const bPtr = writeStringStruct("hell");
		assert(
			fns.string_eq(aPtr, bPtr),
			0,
			'string_eq("hello", "hell") === false (prefix)',
		);
	}
	{
		const aPtr = writeStringStruct("abc");
		const bPtr = writeStringStruct("ABC");
		assert(
			fns.string_eq(aPtr, bPtr),
			0,
			'string_eq("abc", "ABC") === false (case sensitive)',
		);
	}
}

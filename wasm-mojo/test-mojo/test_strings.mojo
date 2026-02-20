# Tests for string operations — native Mojo tests run with `mojo test`.
#
# These tests are a direct port of test/strings.test.ts, exercising
# the same scenarios without the WASM/JS round-trip.  The functions
# under test are defined inline here (matching main.mojo's @export fns).
#
# Run with:
#   mojo test -I src test-mojo/test_strings.mojo

from testing import assert_equal, assert_true, assert_false


# ── Functions under test (mirrors of main.mojo @export fns) ──────────────────


fn return_input_string(x: String) -> String:
    return x


fn return_static_string() -> String:
    return "return-static-string"


fn string_length(x: String) -> Int64:
    return Int64(len(x))


fn string_concat(x: String, y: String) -> String:
    return x + y


fn string_repeat(x: String, n: Int32) -> String:
    var result = String("")
    for _ in range(Int(n)):
        result += x
    return result


fn string_eq(x: String, y: String) -> Int32:
    if x == y:
        return 1
    return 0


# ── Return static string ────────────────────────────────────────────────────


fn test_return_static_string() raises:
    var result = return_static_string()
    assert_equal(
        result,
        "return-static-string",
        'return_static_string === "return-static-string"',
    )


# ── Return input string ─────────────────────────────────────────────────────


fn test_return_input_string_basic() raises:
    var expected = "return-input-string"
    var result = return_input_string(expected)
    assert_equal(
        result, expected, 'return_input_string === "return-input-string"'
    )


fn test_return_input_string_empty() raises:
    var expected = ""
    var result = return_input_string(expected)
    assert_equal(
        result, expected, 'return_input_string("") === "" (empty string)'
    )


fn test_return_input_string_single_char() raises:
    var expected = "a"
    var result = return_input_string(expected)
    assert_equal(
        result, expected, 'return_input_string("a") === "a" (single char)'
    )


fn test_return_input_string_emoji() raises:
    var expected = "Hello, World! 🌍"
    var result = return_input_string(expected)
    assert_equal(result, expected, "return_input_string with emoji roundtrip")


# ── String length ────────────────────────────────────────────────────────────


fn test_string_length_hello() raises:
    assert_equal(
        string_length("hello"),
        Int64(5),
        'string_length("hello") === 5',
    )


fn test_string_length_empty() raises:
    assert_equal(
        string_length(""),
        Int64(0),
        'string_length("") === 0',
    )


fn test_string_length_single_char() raises:
    assert_equal(
        string_length("a"),
        Int64(1),
        'string_length("a") === 1',
    )


fn test_string_length_ten_chars() raises:
    assert_equal(
        string_length("abcdefghij"),
        Int64(10),
        'string_length("abcdefghij") === 10',
    )


fn test_string_length_utf8_emoji() raises:
    # UTF-8 multibyte: 🌍 is 4 bytes
    assert_equal(
        string_length("🌍"),
        Int64(4),
        'string_length("🌍") === 4 (UTF-8 bytes)',
    )


# ── String concatenation ────────────────────────────────────────────────────


fn test_string_concat_basic() raises:
    var result = string_concat("hello", " world")
    assert_equal(
        result,
        "hello world",
        'string_concat("hello", " world") === "hello world"',
    )


fn test_string_concat_empty_first() raises:
    var result = string_concat("", "world")
    assert_equal(
        result,
        "world",
        'string_concat("", "world") === "world"',
    )


fn test_string_concat_empty_second() raises:
    var result = string_concat("hello", "")
    assert_equal(
        result,
        "hello",
        'string_concat("hello", "") === "hello"',
    )


fn test_string_concat_both_empty() raises:
    var result = string_concat("", "")
    assert_equal(result, "", 'string_concat("", "") === ""')


fn test_string_concat_short() raises:
    var result = string_concat("foo", "bar")
    assert_equal(
        result,
        "foobar",
        'string_concat("foo", "bar") === "foobar"',
    )


# ── String repeat ────────────────────────────────────────────────────────────


fn test_string_repeat_basic() raises:
    var result = string_repeat("ab", 3)
    assert_equal(
        result,
        "ababab",
        'string_repeat("ab", 3) === "ababab"',
    )


fn test_string_repeat_one() raises:
    var result = string_repeat("x", 1)
    assert_equal(result, "x", 'string_repeat("x", 1) === "x"')


fn test_string_repeat_zero() raises:
    var result = string_repeat("abc", 0)
    assert_equal(result, "", 'string_repeat("abc", 0) === ""')


fn test_string_repeat_five() raises:
    var result = string_repeat("ha", 5)
    assert_equal(
        result,
        "hahahahaha",
        'string_repeat("ha", 5) === "hahahahaha"',
    )


# ── String equality ──────────────────────────────────────────────────────────


fn test_string_eq_same() raises:
    assert_equal(
        string_eq("hello", "hello"),
        Int32(1),
        'string_eq("hello", "hello") === true',
    )


fn test_string_eq_different() raises:
    assert_equal(
        string_eq("hello", "world"),
        Int32(0),
        'string_eq("hello", "world") === false',
    )


fn test_string_eq_both_empty() raises:
    assert_equal(
        string_eq("", ""),
        Int32(1),
        'string_eq("", "") === true',
    )


fn test_string_eq_prefix() raises:
    assert_equal(
        string_eq("hello", "hell"),
        Int32(0),
        'string_eq("hello", "hell") === false (prefix)',
    )


fn test_string_eq_case_sensitive() raises:
    assert_equal(
        string_eq("abc", "ABC"),
        Int32(0),
        'string_eq("abc", "ABC") === false (case sensitive)',
    )


# ── String length after concat ───────────────────────────────────────────────


fn test_string_length_after_concat() raises:
    """len(concat(a, b)) === len(a) + len(b)."""
    var a = "foo"
    var b = "barbaz"
    var result = string_concat(a, b)
    assert_equal(
        string_length(result),
        string_length(a) + string_length(b),
        "len(concat(a,b)) === len(a) + len(b)",
    )


# ── String repeat length ────────────────────────────────────────────────────


fn test_string_repeat_length() raises:
    """len(repeat(s, n)) === len(s) * n."""
    var s = "ab"
    var n: Int32 = 5
    var result = string_repeat(s, n)
    assert_equal(
        string_length(result),
        string_length(s) * Int64(n),
        "len(repeat(s, n)) === len(s) * n",
    )


# ── String equality reflexivity ──────────────────────────────────────────────


fn test_string_eq_reflexive() raises:
    """A string is always equal to itself."""
    var strings = List[String]()
    strings.append("")
    strings.append("a")
    strings.append("hello")
    strings.append("café")
    strings.append("🌍🌎🌏")
    strings.append("Hello, World! 🎉")
    for i in range(len(strings)):
        assert_equal(
            string_eq(strings[i], strings[i]),
            Int32(1),
            String("string_eq reflexive for: ") + strings[i],
        )


# ── SSO boundary tests (from sso.test.ts) ───────────────────────────────────
#
# Mojo's Small String Optimization stores strings inline in the
# 24-byte struct when they fit (≤23 bytes). At 24+ bytes the data
# is heap-allocated. These tests exercise the boundary.


fn _repeat_char(c: String, n: Int) -> String:
    """Helper: repeat a single-char string n times."""
    var result = String("")
    for _ in range(n):
        result += c
    return result


fn test_sso_roundtrip_22_bytes() raises:
    var s = _repeat_char("a", 22)
    var result = return_input_string(s)
    assert_equal(result, s, "return_input_string 22-byte string (SSO)")


fn test_sso_roundtrip_23_bytes() raises:
    var s = _repeat_char("b", 23)
    var result = return_input_string(s)
    assert_equal(result, s, "return_input_string 23-byte string (SSO max)")


fn test_sso_roundtrip_24_bytes() raises:
    var s = _repeat_char("c", 24)
    var result = return_input_string(s)
    assert_equal(result, s, "return_input_string 24-byte string (heap)")


fn test_sso_roundtrip_25_bytes() raises:
    var s = _repeat_char("d", 25)
    var result = return_input_string(s)
    assert_equal(result, s, "return_input_string 25-byte string (heap)")


fn test_sso_length_22() raises:
    var s = _repeat_char("x", 22)
    assert_equal(string_length(s), Int64(22), "string_length 22-byte (SSO)")


fn test_sso_length_23() raises:
    var s = _repeat_char("x", 23)
    assert_equal(string_length(s), Int64(23), "string_length 23-byte (SSO max)")


fn test_sso_length_24() raises:
    var s = _repeat_char("x", 24)
    assert_equal(string_length(s), Int64(24), "string_length 24-byte (heap)")


fn test_sso_eq_both_sso() raises:
    var a = _repeat_char("y", 23)
    var b = _repeat_char("y", 23)
    assert_equal(
        string_eq(a, b),
        Int32(1),
        "string_eq 23-byte identical (SSO === SSO)",
    )


fn test_sso_eq_sso_vs_heap_different_length() raises:
    var a = _repeat_char("z", 23)
    var b = _repeat_char("z", 24)
    assert_equal(
        string_eq(a, b),
        Int32(0),
        "string_eq 23-byte vs 24-byte (SSO !== heap, different length)",
    )


fn test_sso_eq_both_heap() raises:
    var a = _repeat_char("w", 24)
    var b = _repeat_char("w", 24)
    assert_equal(
        string_eq(a, b),
        Int32(1),
        "string_eq 24-byte identical (heap === heap)",
    )


fn test_sso_eq_same_length_differ_last_byte() raises:
    var a = _repeat_char("a", 23)
    var b = _repeat_char("a", 22) + "b"
    assert_equal(
        string_eq(a, b),
        Int32(0),
        "string_eq 23-byte differ in last byte (SSO)",
    )


fn test_sso_concat_to_23() raises:
    """Two small strings that concat to exactly 23 bytes (SSO)."""
    var a = _repeat_char("a", 11)
    var b = _repeat_char("b", 12)
    var result = string_concat(a, b)
    var expected = _repeat_char("a", 11) + _repeat_char("b", 12)
    assert_equal(result, expected, "string_concat 11+12=23 bytes (SSO max)")
    assert_equal(
        string_length(result),
        Int64(23),
        "string_concat result length === 23",
    )


fn test_sso_concat_to_24() raises:
    """Two small strings that concat to exactly 24 bytes (crosses to heap)."""
    var a = _repeat_char("a", 12)
    var b = _repeat_char("b", 12)
    var result = string_concat(a, b)
    var expected = _repeat_char("a", 12) + _repeat_char("b", 12)
    assert_equal(
        result,
        expected,
        "string_concat 12+12=24 bytes (crosses to heap)",
    )
    assert_equal(
        string_length(result),
        Int64(24),
        "string_concat result length === 24",
    )


fn test_sso_repeat_to_heap() raises:
    """8 * 3 = 24 bytes → heap."""
    var s = _repeat_char("a", 8)
    var result = string_repeat(s, 3)
    var expected = _repeat_char("a", 24)
    assert_equal(
        result,
        expected,
        "string_repeat 8-byte * 3 = 24 bytes (crosses to heap)",
    )


fn test_sso_repeat_stays_sso() raises:
    """23 * 1 = 23 bytes → stays SSO."""
    var s = _repeat_char("q", 23)
    var result = string_repeat(s, 1)
    assert_equal(
        result,
        s,
        "string_repeat 23-byte * 1 = 23 bytes (stays SSO)",
    )


fn test_sso_larger_heap_roundtrip() raises:
    """150-byte string well past SSO boundary."""
    var s = string_repeat("abc", 50)  # 150 bytes
    var result = return_input_string(s)
    assert_equal(
        result, s, "return_input_string 150-byte string (well past SSO)"
    )


fn test_sso_larger_heap_length() raises:
    var s = _repeat_char("x", 256)
    assert_equal(string_length(s), Int64(256), "string_length 256-byte (heap)")


# ── Unicode tests (from unicode.test.ts) ─────────────────────────────────────


fn test_unicode_1byte_ascii() raises:
    assert_equal(
        string_length("A"),
        Int64(1),
        'string_length("A") === 1 (1-byte ASCII)',
    )


fn test_unicode_2byte_latin() raises:
    # é = U+00E9 = 0xC3 0xA9 = 2 bytes
    assert_equal(
        string_length("é"),
        Int64(2),
        'string_length("é") === 2 (2-byte UTF-8)',
    )


fn test_unicode_3byte_cjk() raises:
    # 中 = U+4E2D = 3 bytes
    assert_equal(
        string_length("中"),
        Int64(3),
        'string_length("中") === 3 (3-byte UTF-8)',
    )


fn test_unicode_4byte_emoji() raises:
    # 🌍 = U+1F30D = 4 bytes
    assert_equal(
        string_length("🌍"),
        Int64(4),
        'string_length("🌍") === 4 (4-byte UTF-8)',
    )


fn test_unicode_mixed_byte_widths() raises:
    # A(1) + é(2) + 中(3) + 🌍(4) = 10 bytes
    assert_equal(
        string_length("Aé中🌍"),
        Int64(10),
        'string_length("Aé中🌍") === 10 (1+2+3+4 bytes)',
    )


fn test_unicode_roundtrip_cafe() raises:
    var s = "café"
    var result = return_input_string(s)
    assert_equal(result, s, 'roundtrip "café"')


fn test_unicode_roundtrip_chinese() raises:
    var s = "中文测试"
    var result = return_input_string(s)
    assert_equal(result, s, 'roundtrip "中文测试"')


fn test_unicode_roundtrip_japanese() raises:
    var s = "日本語テスト"
    var result = return_input_string(s)
    assert_equal(result, s, 'roundtrip "日本語テスト"')


fn test_unicode_roundtrip_korean() raises:
    var s = "한국어"
    var result = return_input_string(s)
    assert_equal(result, s, 'roundtrip "한국어"')


fn test_unicode_precomposed_length() raises:
    # Precomposed é (U+00E9) = 2 bytes
    assert_equal(
        string_length("é"),
        Int64(2),
        "string_length(precomposed é) === 2",
    )


fn test_unicode_decomposed_length() raises:
    # Decomposed é = e (1 byte) + combining acute U+0301 (2 bytes) = 3 bytes
    # We construct it from raw bytes to get the decomposed form
    var chars = List[UInt8](capacity=3)
    chars.append(101)  # 'e'
    chars.append(0xCC)  # first byte of U+0301
    chars.append(0x81)  # second byte of U+0301
    var decomposed = String(bytes=chars^)
    assert_equal(
        string_length(decomposed),
        Int64(3),
        "string_length(decomposed é) === 3 (e + combining accent)",
    )


fn test_unicode_precomposed_ne_decomposed() raises:
    # Precomposed and decomposed are NOT byte-equal
    # Construct decomposed e + combining acute accent from raw bytes
    var chars = List[UInt8](capacity=3)
    chars.append(101)  # 'e'
    chars.append(0xCC)  # first byte of U+0301
    chars.append(0x81)  # second byte of U+0301
    var decomposed = String(bytes=chars^)
    assert_equal(
        string_eq("é", decomposed),
        Int32(0),
        "precomposed é !== decomposed e+accent (byte-level comparison)",
    )


fn test_unicode_decomposed_roundtrip() raises:
    # Construct decomposed e + combining acute accent from raw bytes
    var chars = List[UInt8](capacity=3)
    chars.append(101)  # 'e'
    chars.append(0xCC)  # first byte of U+0301
    chars.append(0x81)  # second byte of U+0301
    var s = String(bytes=chars^)
    var result = return_input_string(s)
    assert_equal(result, s, "roundtrip decomposed e+combining accent")


fn test_unicode_concat_multibyte() raises:
    var result = string_concat("café", "☕")
    assert_equal(
        result,
        "café☕",
        'string_concat("café", "☕") === "café☕"',
    )


fn test_unicode_concat_emoji() raises:
    var result = string_concat("🌍", "🌎🌏")
    assert_equal(
        result,
        "🌍🌎🌏",
        'string_concat("🌍", "🌎🌏") === "🌍🌎🌏"',
    )
    assert_equal(
        string_length(result),
        Int64(12),
        "concat of 3 globe emoji === 12 bytes",
    )


fn test_unicode_repeat_2byte() raises:
    var result = string_repeat("é", 5)
    assert_equal(result, "ééééé", 'string_repeat("é", 5) === "ééééé"')
    assert_equal(
        string_length(result),
        Int64(10),
        "repeat 2-byte char 5 times === 10 bytes",
    )


fn test_unicode_repeat_4byte() raises:
    var result = string_repeat("🎉", 3)
    assert_equal(result, "🎉🎉🎉", 'string_repeat("🎉", 3) === "🎉🎉🎉"')
    assert_equal(
        string_length(result),
        Int64(12),
        "repeat 4-byte emoji 3 times === 12 bytes",
    )


fn test_unicode_eq_cjk_same() raises:
    assert_equal(
        string_eq("日本語", "日本語"),
        Int32(1),
        'string_eq("日本語", "日本語") === true',
    )


fn test_unicode_eq_cjk_different() raises:
    assert_equal(
        string_eq("日本語", "中文"),
        Int32(0),
        'string_eq("日本語", "中文") === false',
    )


fn test_unicode_eq_different_emoji() raises:
    assert_equal(
        string_eq("🌍", "🌎"),
        Int32(0),
        'string_eq("🌍", "🌎") === false (different emoji)',
    )


fn test_unicode_eq_zwj_family() raises:
    assert_equal(
        string_eq("👨‍👩‍👧‍👦", "👨‍👩‍👧‍👦"),
        Int32(1),
        "string_eq(ZWJ family, ZWJ family) === true",
    )


fn test_unicode_roundtrip_arabic() raises:
    var s = "مرحبا"
    var result = return_input_string(s)
    assert_equal(result, s, 'roundtrip Arabic "مرحبا"')


fn test_unicode_roundtrip_hebrew() raises:
    var s = "שלום"
    var result = return_input_string(s)
    assert_equal(result, s, 'roundtrip Hebrew "שלום"')


fn test_unicode_roundtrip_mixed_ltr_rtl() raises:
    var s = "Hello مرحبا World"
    var result = return_input_string(s)
    assert_equal(result, s, "roundtrip mixed LTR/RTL")


fn test_unicode_zwj_family_length() raises:
    # 👨‍👩‍👧‍👦 = U+1F468 ZWJ U+1F469 ZWJ U+1F467 ZWJ U+1F466
    # 4 + 3 + 4 + 3 + 4 + 3 + 4 = 25 bytes
    assert_equal(
        string_length("👨‍👩‍👧‍👦"),
        Int64(25),
        'string_length("👨‍👩‍👧‍👦") === 25 (4 emoji + 3 ZWJ)',
    )


fn test_unicode_flag_emoji_length() raises:
    # 🏳️‍🌈 = U+1F3F3 U+FE0F U+200D U+1F308
    # 4 + 3 + 3 + 4 = 14 bytes
    assert_equal(
        string_length("🏳️‍🌈"),
        Int64(14),
        'string_length("🏳️‍🌈") === 14 (flag + VS16 + ZWJ + rainbow)',
    )


fn test_unicode_skin_tone_length() raises:
    # 👋🏽 = U+1F44B U+1F3FD = 4 + 4 = 8 bytes
    assert_equal(
        string_length("👋🏽"),
        Int64(8),
        'string_length("👋🏽") === 8 (wave + skin tone modifier)',
    )

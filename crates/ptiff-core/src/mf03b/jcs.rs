//! RFC 8785 / JCS canonical JSON writer (private to MF-03B).
//!
//! This module implements the JSON Canonicalization Scheme (RFC 8785) for a
//! small typed JSON value model. It is deliberately isolated from the
//! MF-03A-to-JSON mapping layer (see [`super`]) so that canonicalization
//! rules are implemented, documented and tested in exactly one place.
//!
//! # RFC 8785 rules implemented here
//!
//! * **Whitespace** (RFC 8785 §3.2.1): no insignificant whitespace is ever
//!   emitted between JSON tokens.
//! * **Literals** (§3.2.2.1): `null`, `true`, `false` are emitted verbatim.
//! * **Strings** (§3.2.2.2): code points U+0000..U+001F are escaped with
//!   lowercase-hex `\uXXXX` except the predefined short escapes
//!   `\b`/`\t`/`\n`/`\f`/`\r` (U+0008/U+0009/U+000A/U+000C/U+000D);
//!   `"` and `\` are escaped as `\"` and `\\`; every other code point is
//!   emitted as-is (raw UTF-8, no `\uXXXX` and no `\/`). Lone surrogates
//!   cannot occur in Rust `String`/`char` values, so the RFC's
//!   terminate-with-error requirement is satisfied by construction.
//! * **Numbers** (§3.2.2.3): finite IEEE-754 doubles follow the
//!   ECMAScript `Number::toString` grammar (shortest round-trip digits with
//!   ECMAScript tie-breaking, decimal layout for decimal exponents −6..20,
//!   `e±X` exponential layout otherwise, `-0.0` serialized as `0`). NaN and
//!   ±Infinity are not representable in JSON and produce
//!   [`JcsError::NonFiniteNumber`]. Exact integer values (i64/u64) are
//!   serialized as decimal digits; RFC 8785 permits exact integers beyond
//!   the IEEE-754 exact range in implementations that store them exactly.
//!   The PTIFF-side integer64 policy (> 2^53 must be typed-lexical) is
//!   enforced by the mapping layer in [`super`], never here.
//! * **Objects** (§3.2.3): member names are sorted recursively by UTF-16
//!   code unit (the RFC's ordering), not by code point and not bytewise —
//!   surrogate pairs sort between U+DFFF-order BMP characters and U+E000+,
//!   which differs from code-point order for astral names. Array element
//!   order is never changed; objects nested inside arrays are canonicalized
//!   recursively.
//! * **Encoding** (§3.2.4): output is UTF-8.
//!
//! Shortest ECMAScript digits come from [`ryu`], the reference the RFC
//! itself names; `std::fmt` is not used for doubles because it does not
//! match ECMAScript tie-breaking on exact-midpoint values (RFC 8785
//! Appendix B row `43143ff3c1cb0959`).

use std::cmp::Ordering;
use std::fmt::Write;

/// Canonical JSON value model used by the MF-03B encoder.
///
/// The model is intentionally small: it exists only to carry the typed
/// MF-03A serialization representation through RFC 8785 canonicalization.
/// Objects keep their members in semantic (construction) order; canonical
/// member ordering is applied at write time, never earlier, so semantically
/// equal objects produce identical bytes regardless of construction order.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum JsonValue {
    /// JSON `null`.
    #[allow(dead_code)]
    // Not yet constructed by the MF-03A mapping (no nullable field is in
    // the representation); reserved so the writer covers the full RFC 8785
    // grammar.
    Null,
    /// JSON `true`/`false`.
    #[allow(dead_code)]
    // Constructed only by RFC-conformance tests today; reserved for future
    // schema-phase boolean fields.
    Bool(bool),
    /// A JSON number ([`JsonNumber`]).
    Number(JsonNumber),
    /// A JSON string (already-decoded text; the writer applies escaping).
    String(String),
    /// A JSON array; element order is semantic and preserved verbatim.
    Array(Vec<JsonValue>),
    /// A JSON object; members are key/value pairs in semantic order.
    Object(Vec<(String, JsonValue)>),
}

/// A JSON number value.
///
/// Exact integers and finite IEEE-754 doubles are distinct variants so that
/// the mapping layer can decide per PTIFF integer64 position whether an
/// integer is small enough to be an ordinary JSON number.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum JsonNumber {
    /// Signed exact integer (negative support for future schema fields).
    #[allow(dead_code)]
    // Constructed only by RFC-conformance tests today; the MF-03A mapping
    // only emits non-negative values.
    I64(i64),
    /// Unsigned exact integer.
    U64(u64),
    /// Finite IEEE-754 double (must be finite; see [`JcsError`]).
    #[allow(dead_code)]
    // Constructed only by RFC-conformance tests today; the MF-03A
    // representation carries no floating-point field yet (schema phase).
    F64(f64),
}

/// The one RFC-8785 violation the writer can encounter while serializing a
/// value that is otherwise well-formed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum JcsError {
    /// NaN / ±Infinity reached a JSON number position.
    NonFiniteNumber,
}

/// Writes the RFC 8785 canonical serialization of `value` into `out`.
///
/// # Errors
///
/// Returns [`JcsError::NonFiniteNumber`] if a [`JsonNumber::F64`] value is
/// not finite; no other failure is possible for well-formed values.
pub(super) fn write_canonical(value: &JsonValue, out: &mut String) -> Result<(), JcsError> {
    match value {
        JsonValue::Null => out.push_str("null"),
        JsonValue::Bool(true) => out.push_str("true"),
        JsonValue::Bool(false) => out.push_str("false"),
        JsonValue::Number(number) => write_number(number, out)?,
        JsonValue::String(text) => {
            out.push('"');
            write_escaped_string(text, out);
            out.push('"');
        }
        JsonValue::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_canonical(item, out)?;
            }
            out.push(']');
        }
        JsonValue::Object(members) => {
            out.push('{');
            let mut order: Vec<usize> = (0..members.len()).collect();
            order.sort_by(|left, right| compare_utf16(&members[*left].0, &members[*right].0));
            for (index, member) in order.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                let (name, item) = &members[*member];
                out.push('"');
                write_escaped_string(name, out);
                out.push('"');
                out.push(':');
                write_canonical(item, out)?;
            }
            out.push('}');
        }
    }
    Ok(())
}

/// Writes a JSON number in canonical form.
fn write_number(number: &JsonNumber, out: &mut String) -> Result<(), JcsError> {
    match number {
        JsonNumber::I64(value) => {
            let _ = write!(out, "{value}");
        }
        JsonNumber::U64(value) => {
            let _ = write!(out, "{value}");
        }
        JsonNumber::F64(value) => write_f64(*value, out)?,
    }
    Ok(())
}

/// Writes a finite IEEE-754 double per ECMAScript `Number::toString`.
fn write_f64(value: f64, out: &mut String) -> Result<(), JcsError> {
    if !value.is_finite() {
        return Err(JcsError::NonFiniteNumber);
    }
    // ECMAScript serializes both +0.0 and -0.0 as the literal "0".
    if value == 0.0 {
        out.push('0');
        return Ok(());
    }

    // ryu yields the shortest decimal digits with ECMAScript tie-breaking,
    // but its layout keeps a ".0" suffix for integral doubles and uses its
    // own decimal/exponential threshold; the ECMAScript layout rules are
    // applied below instead.
    let mut buffer = ryu::Buffer::new();
    let repr = buffer.format_finite(value);
    let (negative, body) = match repr.as_bytes().first() {
        Some(b'-') => (true, &repr[1..]),
        _ => (false, repr),
    };

    // Recover the shortest digit string plus the power of ten of its
    // leading digit from ryu's layout.
    let mut digits = String::with_capacity(body.len());
    let leading_power: i32;
    if let Some(exp_pos) = body.find('e') {
        digits.push_str(&body[..exp_pos].replace('.', ""));
        leading_power = body[exp_pos + 1..]
            .parse::<i32>()
            .expect("ryu emits a valid decimal exponent for finite doubles");
    } else if let Some(integer) = body.strip_suffix(".0") {
        // Ryu marks integral doubles with a trailing ".0"; that marker is
        // not part of the shortest digits.
        digits.push_str(integer);
        leading_power = integer.len() as i32 - 1;
    } else if let Some(dot) = body.find('.') {
        let integer_part = &body[..dot];
        let fraction_part = &body[dot + 1..];
        if integer_part == "0" {
            // 0.00dddd: decimal exponent is -(leading zero count + 1).
            let zero_count = fraction_part.bytes().take_while(|b| *b == b'0').count();
            digits.push_str(&fraction_part[zero_count..]);
            leading_power = -(zero_count as i32) - 1;
        } else {
            digits.push_str(integer_part);
            digits.push_str(fraction_part);
            leading_power = integer_part.len() as i32 - 1;
        }
    } else {
        digits.push_str(body);
        leading_power = digits.len() as i32 - 1;
    }

    if negative {
        out.push('-');
    }
    write_ecmascript_layout(&digits, leading_power, out);
    Ok(())
}

/// Lays out shortest digits per ECMAScript `Number::toString` §7.1.12.1.
///
/// `digits` is the shortest round-trip significand (no leading zero, no
/// trailing zero) and `leading_power` is the power of ten of its first
/// digit (value = `0.d1d2… × 10^(leading_power+1)` in ECMAScript terms the
/// number equals `d1.d2… × 10^leading_power`).
fn write_ecmascript_layout(digits: &str, leading_power: i32, out: &mut String) {
    // ECMAScript parameterization: n = E + 1 where E is the decimal
    // exponent of the leading digit (value = 0.d1d2... * 10^n).
    let digit_count = digits.len() as i32;
    let n = leading_power + 1;

    if digit_count <= n && n <= 21 {
        // Integer form: the significand followed by n - digit_count zeros.
        out.push_str(digits);
        for _ in 0..(n - digit_count) {
            out.push('0');
        }
    } else if 0 < n && n <= 21 {
        // Decimal form with a point: first n digits, '.', remaining digits.
        let split = n as usize;
        out.push_str(&digits[..split]);
        out.push('.');
        out.push_str(&digits[split..]);
    } else if -6 < n && n <= 0 {
        // 0.000…dddd form.
        out.push_str("0.");
        for _ in 0..(-n) {
            out.push('0');
        }
        out.push_str(digits);
    } else {
        // Exponential form d[.ddd]e±X.
        out.push_str(&digits[..1]);
        if digit_count > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        out.push('e');
        if leading_power >= 0 {
            out.push('+');
        } else {
            out.push('-');
        }
        let _ = write!(out, "{}", leading_power.abs());
    }
}

/// Writes `text` as a JSON string body (without the surrounding quotes).
fn write_escaped_string(text: &str, out: &mut String) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{0009}' => out.push_str("\\t"),
            '\u{000A}' => out.push_str("\\n"),
            '\u{000C}' => out.push_str("\\f"),
            '\u{000D}' => out.push_str("\\r"),
            ch if (ch as u32) < 0x20 => {
                // RFC 8785: lowercase hexadecimal \uXXXX escapes.
                let value = ch as u32;
                out.push_str("\\u00");
                out.push(HEX[((value >> 4) & 0xF) as usize] as char);
                out.push(HEX[(value & 0xF) as usize] as char);
            }
            ch => out.push(ch),
        }
    }
}

/// Compares two member names per RFC 8785 §3.2.3: lexicographic over
/// UTF-16 code units treated as unsigned integers.
fn compare_utf16(left: &str, right: &str) -> Ordering {
    let mut left_units = left.encode_utf16();
    let mut right_units = right.encode_utf16();
    loop {
        match (left_units.next(), right_units.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(left_unit), Some(right_unit)) => {
                let ordering = left_unit.cmp(&right_unit);
                if ordering != Ordering::Equal {
                    return ordering;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Canonicalizes `value` to bytes.
    fn canonical(value: &JsonValue) -> Vec<u8> {
        let mut out = String::new();
        write_canonical(value, &mut out).expect("test value is canonicalizable");
        out.into_bytes()
    }

    fn string(text: &str) -> JsonValue {
        JsonValue::String(text.to_owned())
    }

    fn object(members: Vec<(&str, JsonValue)>) -> JsonValue {
        JsonValue::Object(
            members
                .into_iter()
                .map(|(name, value)| (name.to_owned(), value))
                .collect(),
        )
    }

    fn number_f64(value: f64) -> JsonValue {
        JsonValue::Number(JsonNumber::F64(value))
    }

    /// RFC 8785 §3.2.4 publishes the canonical UTF-8 bytes of its sample
    /// document. This test reproduces that sample (input layout chosen from
    /// RFC 8785 §3.2.2) and pins the full canonical byte sequence.
    #[test]
    fn rfc_8785_section_3_2_4_sample_bytes() {
        let mut sample_string = String::new();
        sample_string.push('\u{20ac}');
        sample_string.push('$');
        sample_string.push('\u{000F}');
        sample_string.push('\n');
        sample_string.push_str("A'B");
        sample_string.push('"');
        sample_string.push('\\');
        sample_string.push('\\');
        sample_string.push('"');
        sample_string.push('/');

        let value = object(vec![
            (
                "numbers",
                JsonValue::Array(vec![
                    // 0x41b3de4355555555: the double whose canonical form
                    // is 333333333.3333333 (RFC 8785 Appendix B row).
                    number_f64(f64::from_bits(0x41b3de4355555555)),
                    number_f64(1e30),
                    number_f64(4.5),
                    number_f64(0.002),
                    number_f64(1e-27),
                ]),
            ),
            ("string", string(&sample_string)),
            (
                "literals",
                JsonValue::Array(vec![
                    JsonValue::Null,
                    JsonValue::Bool(true),
                    JsonValue::Bool(false),
                ]),
            ),
        ]);

        // RFC 8785 §3.2.3 (canonical rendering of the §3.2.2 sample):
        //   {"literals":[null,true,false],"numbers":[333333333.3333333,
        //   1e+30,4.5,0.002,1e-27],"string":"€$\u000f\nA'B\"\\\\\"/"}
        let mut expected = String::from(
            "{\"literals\":[null,true,false],\"numbers\":[\
                333333333.3333333,1e+30,4.5,0.002,1e-27],\"string\":\"",
        );
        expected.push('\u{20ac}');
        expected.push('$');
        expected.push_str("\\u000f");
        expected.push_str("\\n");
        expected.push_str("A'B");
        expected.push_str("\\\"");
        expected.push_str("\\\\");
        expected.push_str("\\\\");
        expected.push_str("\\\"");
        expected.push('/');
        expected.push('"');
        expected.push('}');

        assert_eq!(canonical(&value), expected.into_bytes());
        // RFC 8785 §3.2.1: canonical output contains no whitespace tokens.
        let bytes = canonical(&value);
        assert!(!bytes.contains(&b' '));
        assert!(!bytes.contains(&b'\n'));
        assert!(!bytes.contains(&b'\t'));
    }

    /// String escaping: quote, backslash, short escapes, lowercase hex
    /// escapes, and raw (unescaped) non-ASCII output.
    #[test]
    fn string_escaping_is_rfc_conformant() {
        let mut text = String::new();
        // Control characters that use the predefined short escapes.
        text.push('\u{0008}');
        text.push('\u{0009}');
        text.push('\u{000A}');
        text.push('\u{000C}');
        text.push('\u{000D}');
        // Remaining control characters use lowercase-hex \uXXXX escapes.
        text.push('\u{0000}');
        text.push('\u{000F}');
        text.push('\u{001F}');
        // Quote, backslash, solidus.
        text.push('"');
        text.push('\\');
        text.push('/');
        // Non-ASCII characters are emitted as-is (raw UTF-8).
        text.push('\u{20ac}');
        text.push('\u{1f600}');

        let bytes = canonical(&string(&text));
        let expected = b"\"\\b\\t\\n\\f\\r\\u0000\\u000f\\u001f\\\"\\\\/\
                        \xe2\x82\xac\xf0\x9f\x98\x80\"";
        assert_eq!(bytes, expected);
    }

    /// Object member names sort per RFC 8785 §3.2.3 test data (UTF-16 code
    /// units, which places the astral emoji before the U+FB33 key).
    #[test]
    fn object_members_sort_by_utf16_code_units() {
        let value = object(vec![
            ("\u{20ac}", string("Euro Sign")),
            ("\r", string("Carriage Return")),
            ("\u{fb33}", string("Hebrew Letter Dalet With Dagesh")),
            ("1", string("One")),
            ("\u{1f600}", string("Emoji: Grinning Face")),
            ("\u{0080}", string("Control")),
            ("\u{00f6}", string("Latin Small Letter O With Diaeresis")),
        ]);

        let mut expected = String::from("{");
        // Expected order from RFC 8785 §3.2.3. Keys are rendered in their
        // serialized (escaped) form, so the carriage-return key appears as
        // the two characters `\r` in the canonical bytes.
        let ordered: &[(&str, &str)] = &[
            ("\\r", "Carriage Return"),
            ("1", "One"),
            ("\u{0080}", "Control"),
            ("\u{00f6}", "Latin Small Letter O With Diaeresis"),
            ("\u{20ac}", "Euro Sign"),
            ("\u{1f600}", "Emoji: Grinning Face"),
            ("\u{fb33}", "Hebrew Letter Dalet With Dagesh"),
        ];
        for (index, (name, text)) in ordered.iter().enumerate() {
            if index > 0 {
                expected.push(',');
            }
            expected.push('"');
            expected.push_str(name);
            expected.push('"');
            expected.push(':');
            expected.push('"');
            expected.push_str(text);
            expected.push('"');
        }
        expected.push('}');

        assert_eq!(canonical(&value), expected.into_bytes());
    }

    /// A bytewise UTF-8 sort would place the astral key after U+FB33; the
    /// RFC's UTF-16 code-unit sort places it before. This vector pins the
    /// distinction explicitly.
    #[test]
    fn astral_key_orders_before_bmp_key_above_e000() {
        let value = object(vec![
            ("\u{fb33}", string("after")),
            ("\u{1f600}", string("before")),
        ]);
        let expected = "{\"\u{1f600}\":\"before\",\"\u{fb33}\":\"after\"}";
        assert_eq!(canonical(&value), expected.as_bytes());
    }

    /// Nested objects and arrays: recursive canonicalization with arrays
    /// kept in their semantic order.
    #[test]
    fn nested_objects_and_arrays() {
        let value = object(vec![
            (
                "outer",
                JsonValue::Array(vec![
                    object(vec![("z", JsonValue::Null), ("a", JsonValue::Bool(true))]),
                    object(vec![("m", number_f64(2.5))]),
                ]),
            ),
            ("b", string("second")),
        ]);

        let expected = "{\"b\":\"second\",\"outer\":[\
                        {\"a\":true,\"z\":null},{\"m\":2.5}]}";
        assert_eq!(canonical(&value), expected.as_bytes());
    }

    /// Object member insertion order must not influence canonical bytes.
    #[test]
    fn member_insertion_order_is_irrelevant() {
        let first = object(vec![
            ("beta", number_f64(1.0)),
            ("alpha", JsonValue::Array(vec![JsonValue::Null])),
        ]);
        let second = object(vec![
            ("alpha", JsonValue::Array(vec![JsonValue::Null])),
            ("beta", number_f64(1.0)),
        ]);
        assert_eq!(canonical(&first), canonical(&second));
    }

    /// Arrays preserve element order; only objects inside are canonicalized.
    #[test]
    fn array_element_order_is_semantic() {
        let value = JsonValue::Array(vec![
            object(vec![("k", number_f64(1.0))]),
            object(vec![("k", number_f64(2.0))]),
        ]);
        let expected = "[{\"k\":1},{\"k\":2}]";
        assert_eq!(canonical(&value), expected.as_bytes());

        let reversed = JsonValue::Array(vec![
            object(vec![("k", number_f64(2.0))]),
            object(vec![("k", number_f64(1.0))]),
        ]);
        let expected_reversed = "[{\"k\":2},{\"k\":1}]";
        assert_eq!(canonical(&reversed), expected_reversed.as_bytes());
    }

    /// Literals and simple values.
    #[test]
    fn literals_and_scalars() {
        assert_eq!(canonical(&JsonValue::Null), b"null");
        assert_eq!(canonical(&JsonValue::Bool(true)), b"true");
        assert_eq!(canonical(&JsonValue::Bool(false)), b"false");
        assert_eq!(canonical(&JsonValue::Array(vec![])), b"[]");
        assert_eq!(canonical(&JsonValue::Object(vec![])), b"{}");
        assert_eq!(canonical(&string("")), b"\"\"");
        assert_eq!(canonical(&string("abc")), b"\"abc\"");
    }

    /// Exact integers serialize as ordinary decimal numbers at the writer
    /// level (RFC 8785 extended-precision allowance); the PTIFF integer64
    /// policy for values beyond 2^53 lives in the mapping layer.
    #[test]
    fn exact_integers_serialize_as_numbers() {
        assert_eq!(canonical(&JsonValue::Number(JsonNumber::U64(0))), b"0");
        assert_eq!(canonical(&JsonValue::Number(JsonNumber::I64(-7))), b"-7");
        assert_eq!(
            canonical(&JsonValue::Number(JsonNumber::I64(-123))),
            b"-123"
        );
        assert_eq!(
            canonical(&JsonValue::Number(JsonNumber::U64(2_147_483_647))),
            b"2147483647"
        );
        assert_eq!(
            canonical(&JsonValue::Number(JsonNumber::U64(u64::MAX))),
            b"18446744073709551615"
        );
        assert_eq!(
            canonical(&JsonValue::Number(JsonNumber::I64(i64::MIN))),
            b"-9223372036854775808"
        );
    }

    /// RFC 8785 Appendix B, Table 1: ECMAScript-compatible number
    /// serialization samples (IEEE-754 bit patterns → canonical JSON text).
    /// The full table is exercised so the number path is pinned against the
    /// specification, including -0.0, subnormals, tie-breaking (note 4),
    /// and decimal/exponential boundary values.
    #[test]
    fn rfc_8785_appendix_b_number_vectors() {
        let vectors: &[(u64, &str)] = &[
            (0x0000000000000000, "0"),
            (0x8000000000000000, "0"),
            (0x0000000000000001, "5e-324"),
            (0x8000000000000001, "-5e-324"),
            (0x7fefffffffffffff, "1.7976931348623157e+308"),
            (0xffefffffffffffff, "-1.7976931348623157e+308"),
            (0x4340000000000000, "9007199254740992"),
            (0xc340000000000000, "-9007199254740992"),
            (0x4430000000000000, "295147905179352830000"),
            (0x44b52d02c7e14af5, "9.999999999999997e+22"),
            (0x44b52d02c7e14af6, "1e+23"),
            (0x44b52d02c7e14af7, "1.0000000000000001e+23"),
            (0x444b1ae4d6e2ef4e, "999999999999999700000"),
            (0x444b1ae4d6e2ef4f, "999999999999999900000"),
            (0x444b1ae4d6e2ef50, "1e+21"),
            (0x3eb0c6f7a0b5ed8c, "9.999999999999997e-7"),
            (0x3eb0c6f7a0b5ed8d, "0.000001"),
            (0x41b3de4355555553, "333333333.3333332"),
            (0x41b3de4355555554, "333333333.33333325"),
            (0x41b3de4355555555, "333333333.3333333"),
            (0x41b3de4355555556, "333333333.3333334"),
            (0x41b3de4355555557, "333333333.33333343"),
            (0xbecbf647612f3696, "-0.0000033333333333333333"),
            (0x43143ff3c1cb0959, "1424953923781206.2"),
        ];
        for (bits, expected) in vectors {
            let value = number_f64(f64::from_bits(*bits));
            assert_eq!(canonical(&value), expected.as_bytes(), "bits {bits:016x}");
        }
    }

    /// Additional decimal/exponential boundary vectors around the
    /// ECMAScript layout thresholds (decimal exponent −6..20 is decimal).
    #[test]
    fn number_layout_boundaries() {
        let vectors: &[(f64, &str)] = &[
            (1e-7, "1e-7"),
            (1e-6, "0.000001"),
            (1e-5, "0.00001"),
            (0.0000123, "0.0000123"),
            (1e20, "100000000000000000000"),
            (1e21, "1e+21"),
            (1e16, "10000000000000000"),
            (999_999_999_999_999.8, "999999999999999.8"),
            (1.0, "1"),
            (12.0, "12"),
            (0.5, "0.5"),
            (1.234_567_890_123_456_8e20, "123456789012345680000"),
        ];
        for (value, expected) in vectors {
            assert_eq!(
                canonical(&number_f64(*value)),
                expected.as_bytes(),
                "{value}"
            );
        }
    }

    /// Determinism: repeated canonicalization yields identical bytes, and
    /// float/integer spelling converges on the same canonical form.
    #[test]
    fn canonical_bytes_are_deterministic() {
        let value = object(vec![
            (
                "a",
                JsonValue::Array(vec![number_f64(1.0), JsonValue::Null]),
            ),
            ("b", string("x")),
        ]);
        let once = canonical(&value);
        let twice = canonical(&value);
        assert_eq!(once, twice);

        // 1.0 (a double) and 1 (an integer) must canonicalize identically.
        assert_eq!(
            canonical(&number_f64(1.0)),
            canonical(&JsonValue::Number(JsonNumber::U64(1)))
        );
    }

    /// NaN and ±Infinity are rejected, never silently emitted.
    #[test]
    fn non_finite_numbers_are_rejected() {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut out = String::new();
            let result = write_canonical(&number_f64(bad), &mut out);
            assert_eq!(result, Err(JcsError::NonFiniteNumber));
            assert!(out.is_empty());
        }
    }

    /// Determinism of the UTF-16 key ordering for equal-looking keys.
    #[test]
    fn key_order_is_stable_for_duplicate_shaped_names() {
        let value = object(vec![
            ("aa", JsonValue::Null),
            ("a", JsonValue::Null),
            ("", JsonValue::Null),
        ]);
        let expected = "{\"\":null,\"a\":null,\"aa\":null}";
        assert_eq!(canonical(&value), expected.as_bytes());
    }
}

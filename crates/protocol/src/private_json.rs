//! Decode a private JSON string without a nonzeroizing decoded scratch buffer.
use crate::command::CommandError;
use alloc::string::String;
#[cfg(any(feature = "terms", feature = "private_rumor"))]
use alloc::vec::Vec;
#[cfg(any(feature = "terms", feature = "private_rumor"))]
use core::{fmt, marker::PhantomData};
#[cfg(any(feature = "terms", feature = "private_rumor"))]
use serde::{
    Deserialize,
    de::{IgnoredAny, SeqAccess, Visitor},
};
#[cfg(any(feature = "terms", feature = "private_rumor"))]
use serde_json::value::RawValue;
use zeroize::Zeroizing;

#[cfg(any(feature = "terms", test))]
pub(crate) fn decode(raw: &str, maximum_chars: usize) -> Result<Zeroizing<String>, CommandError> {
    decode_limited(raw, maximum_chars, usize::MAX)
}

#[cfg(feature = "private_rumor")]
pub(crate) fn decode_bytes(
    raw: &str,
    maximum_bytes: usize,
) -> Result<Zeroizing<String>, CommandError> {
    decode_limited(raw, usize::MAX, maximum_bytes)
}

fn decode_limited(
    raw: &str,
    maximum_chars: usize,
    maximum_bytes: usize,
) -> Result<Zeroizing<String>, CommandError> {
    let mut bytes = 0usize;
    let mut count = 0usize;
    visit(raw, |ch| {
        count += 1;
        if count > maximum_chars {
            return Err(CommandError::LimitExceeded);
        }
        bytes = bytes
            .checked_add(ch.len_utf8())
            .filter(|value| *value <= maximum_bytes)
            .ok_or(CommandError::LimitExceeded)?;
        Ok(())
    })?;
    let mut output = Zeroizing::new(String::with_capacity(bytes));
    visit(raw, |ch| {
        output.push(ch);
        Ok(())
    })?;
    Ok(output)
}

fn visit(
    raw: &str,
    mut emit: impl FnMut(char) -> Result<(), CommandError>,
) -> Result<(), CommandError> {
    let text = raw
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .ok_or(CommandError::InvalidInput)?;
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            let escaped = chars.next().ok_or(CommandError::InvalidInput)?;
            let value = match escaped {
                '"' => '"',
                '\\' => '\\',
                '/' => '/',
                'b' => '\u{08}',
                'f' => '\u{0c}',
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                'u' => {
                    let first = hex(&mut chars)?;
                    let code = if (0xd800..=0xdbff).contains(&first) {
                        if chars.next() != Some('\\') || chars.next() != Some('u') {
                            return Err(CommandError::InvalidInput);
                        }
                        let second = hex(&mut chars)?;
                        if !(0xdc00..=0xdfff).contains(&second) {
                            return Err(CommandError::InvalidInput);
                        }
                        0x10000 + ((u32::from(first) - 0xd800) << 10) + (u32::from(second) - 0xdc00)
                    } else {
                        u32::from(first)
                    };
                    char::from_u32(code).ok_or(CommandError::InvalidInput)?
                }
                _ => return Err(CommandError::InvalidInput),
            };
            emit(value)?;
        } else {
            if ch == '"' || ch <= '\u{1f}' {
                return Err(CommandError::InvalidInput);
            }
            emit(ch)?;
        }
    }
    Ok(())
}

fn hex(chars: &mut core::str::Chars<'_>) -> Result<u16, CommandError> {
    let mut value = 0;
    for _ in 0..4 {
        let digit = chars
            .next()
            .and_then(|ch| ch.to_digit(16))
            .ok_or(CommandError::InvalidInput)?;
        value = (value << 4) | u16::try_from(digit).map_err(|_| CommandError::InvalidInput)?;
    }
    Ok(value)
}

// Retain only bounded references into the caller's already byte/depth-bounded
// frame. An excessive element is ignored without constructing its value.
#[cfg(any(feature = "terms", feature = "private_rumor"))]
pub(crate) struct RawArray<'a, const N: usize>(pub(crate) Vec<&'a RawValue>);
#[cfg(any(feature = "terms", feature = "private_rumor"))]
impl<'de: 'a, 'a, const N: usize> Deserialize<'de> for RawArray<'a, N> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct ArrayVisitor<'a, const N: usize>(PhantomData<&'a ()>);
        impl<'de: 'a, 'a, const N: usize> Visitor<'de> for ArrayVisitor<'a, N> {
            type Value = RawArray<'a, N>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a bounded array")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                loop {
                    if values.len() == N {
                        if seq.next_element::<IgnoredAny>()?.is_some() {
                            return Err(serde::de::Error::custom("array limit exceeded"));
                        }
                        break;
                    }
                    let Some(value) = seq.next_element::<&'de RawValue>()? else {
                        break;
                    };
                    values.push(value);
                }
                Ok(RawArray(values))
            }
        }
        d.deserialize_seq(ArrayVisitor::<N>(PhantomData))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_json_text_matches_independent_literal_and_surrogate_decoding() {
        for raw in [
            r#""""#,
            r#""plain 🥬 Å A\u030a""#,
            r#""\"\\\/\b\f\n\r\t\u0000\u001f\uD83E\uDD6C""#,
        ] {
            let expected: String = serde_json::from_str(raw).unwrap();
            assert_eq!(decode(raw, 2048).unwrap().as_str(), expected);
            assert_eq!(
                decode(raw, expected.chars().count()).unwrap().as_str(),
                expected
            );
            if !expected.is_empty() {
                assert_eq!(
                    decode(raw, expected.chars().count() - 1),
                    Err(CommandError::LimitExceeded)
                );
            }
        }
    }

    #[test]
    fn private_json_text_rejects_malformed_literals_and_surrogates() {
        for raw in [
            "null",
            "[]",
            "\"",
            "\"a\"b\"",
            "\"\n\"",
            r#""\x""#,
            r#""\u12""#,
            r#""\u000z""#,
            r#""\uDC00""#,
            r#""\uD800""#,
            r#""\uD800x""#,
            r#""\uD800\u0041""#,
        ] {
            assert_eq!(decode(raw, 2048), Err(CommandError::InvalidInput), "{raw}");
        }
    }
}

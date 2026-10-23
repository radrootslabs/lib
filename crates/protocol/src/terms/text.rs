//! Decode a private JSON string without a nonzeroizing decoded scratch buffer.
use super::TermsWireError;
use alloc::string::String;
use zeroize::Zeroizing;

pub(super) fn decode(raw: &str, maximum_chars: usize) -> Result<Zeroizing<String>, TermsWireError> {
    let mut bytes = 0usize;
    let mut count = 0usize;
    visit(raw, |ch| {
        count += 1;
        if count > maximum_chars {
            return Err(TermsWireError::LimitExceeded);
        }
        bytes += ch.len_utf8();
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
    mut emit: impl FnMut(char) -> Result<(), TermsWireError>,
) -> Result<(), TermsWireError> {
    let text = raw
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .ok_or(TermsWireError::InvalidInput)?;
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            let escaped = chars.next().ok_or(TermsWireError::InvalidInput)?;
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
                            return Err(TermsWireError::InvalidInput);
                        }
                        let second = hex(&mut chars)?;
                        if !(0xdc00..=0xdfff).contains(&second) {
                            return Err(TermsWireError::InvalidInput);
                        }
                        0x10000 + ((u32::from(first) - 0xd800) << 10) + (u32::from(second) - 0xdc00)
                    } else {
                        u32::from(first)
                    };
                    char::from_u32(code).ok_or(TermsWireError::InvalidInput)?
                }
                _ => return Err(TermsWireError::InvalidInput),
            };
            emit(value)?;
        } else {
            if ch == '"' || ch <= '\u{1f}' {
                return Err(TermsWireError::InvalidInput);
            }
            emit(ch)?;
        }
    }
    Ok(())
}

fn hex(chars: &mut core::str::Chars<'_>) -> Result<u16, TermsWireError> {
    let mut value = 0;
    for _ in 0..4 {
        let digit = chars
            .next()
            .and_then(|ch| ch.to_digit(16))
            .ok_or(TermsWireError::InvalidInput)?;
        value = (value << 4) | u16::try_from(digit).map_err(|_| TermsWireError::InvalidInput)?;
    }
    Ok(value)
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
                    Err(TermsWireError::LimitExceeded)
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
            assert_eq!(
                decode(raw, 2048),
                Err(TermsWireError::InvalidInput),
                "{raw}"
            );
        }
    }
}

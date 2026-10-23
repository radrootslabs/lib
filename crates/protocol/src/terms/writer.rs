use super::{TERMS_MAX_BYTES, TermsWireError};
use alloc::{string::String, vec::Vec};
use core::{fmt, fmt::Write as _};
use serde::Serialize;
use serde_json::Value;
use zeroize::{Zeroize as _, Zeroizing};

pub(crate) struct Writer {
    length: usize,
    output: Option<Zeroizing<String>>,
}
impl Writer {
    pub(crate) const fn counting() -> Self {
        Self {
            length: 0,
            output: None,
        }
    }
    pub(crate) fn emitting(length: usize) -> Self {
        Self {
            length: 0,
            output: Some(Zeroizing::new(String::with_capacity(length))),
        }
    }
    pub(crate) const fn len(&self) -> usize {
        self.length
    }
    pub(crate) fn finish(self) -> Result<Zeroizing<String>, TermsWireError> {
        self.output.ok_or(TermsWireError::InvalidInput)
    }
    pub(crate) fn raw(&mut self, value: &str) -> Result<(), TermsWireError> {
        let length = self
            .length
            .checked_add(value.len())
            .filter(|length| *length <= TERMS_MAX_BYTES)
            .ok_or(TermsWireError::LimitExceeded)?;
        if let Some(output) = &mut self.output {
            output.push_str(value);
        }
        self.length = length;
        Ok(())
    }
    pub(crate) fn string(&mut self, value: &str) -> Result<(), TermsWireError> {
        self.raw("\"")?;
        const HEX: &[u8; 16] = b"0123456789abcdef";
        for ch in value.chars() {
            match ch {
                '"' => self.raw("\\\"")?,
                '\\' => self.raw("\\\\")?,
                '\n' => self.raw("\\n")?,
                '\r' => self.raw("\\r")?,
                '\t' => self.raw("\\t")?,
                '\u{08}' => self.raw("\\b")?,
                '\u{0c}' => self.raw("\\f")?,
                '\0'..='\u{1f}' => {
                    let byte = ch as u8;
                    let mut escaped = [
                        b'\\',
                        b'u',
                        b'0',
                        b'0',
                        HEX[(byte >> 4) as usize],
                        HEX[(byte & 15) as usize],
                    ];
                    let text =
                        core::str::from_utf8(&escaped).map_err(|_| TermsWireError::InvalidInput)?;
                    let result = self.raw(text);
                    escaped.zeroize();
                    result?;
                }
                _ => {
                    let mut encoded = [0; 4];
                    let result = self.raw(ch.encode_utf8(&mut encoded));
                    encoded.zeroize();
                    result?;
                }
            }
        }
        self.raw("\"")
    }
    pub(crate) fn exact_string(&mut self, value: impl fmt::Display) -> Result<(), TermsWireError> {
        self.raw("\"")?;
        write!(self, "{value}").map_err(|_| TermsWireError::LimitExceeded)?;
        self.raw("\"")
    }
    pub(crate) fn integer(&mut self, value: u16) -> Result<(), TermsWireError> {
        write!(self, "{value}").map_err(|_| TermsWireError::LimitExceeded)
    }
    pub(crate) fn optional_string(&mut self, value: Option<&str>) -> Result<(), TermsWireError> {
        match value {
            Some(value) => self.string(value),
            None => self.raw("null"),
        }
    }
    // Only explicitly public listing snapshots enter this temporary JSON tree.
    // Private fulfillment, lot labels, agreed prices and risk disclosures use
    // the bounded zeroizing writer directly.
    pub(crate) fn public_snapshot<T: Serialize>(
        &mut self,
        value: &T,
    ) -> Result<(), TermsWireError> {
        self.value(&serde_json::to_value(value).map_err(|_| TermsWireError::InvalidInput)?)
    }
    fn value(&mut self, value: &Value) -> Result<(), TermsWireError> {
        match value {
            Value::Object(fields) => {
                let mut keys: Vec<_> = fields.keys().collect();
                if keys.iter().any(|key| !key.is_ascii()) {
                    return Err(TermsWireError::InvalidInput);
                }
                keys.sort_unstable();
                self.raw("{")?;
                for (index, key) in keys.into_iter().enumerate() {
                    if index != 0 {
                        self.raw(",")?;
                    }
                    self.string(key)?;
                    self.raw(":")?;
                    self.value(&fields[key])?;
                }
                self.raw("}")
            }
            Value::Array(values) => {
                self.raw("[")?;
                for (index, value) in values.iter().enumerate() {
                    if index != 0 {
                        self.raw(",")?;
                    }
                    self.value(value)?;
                }
                self.raw("]")
            }
            Value::String(value) => self.string(value),
            Value::Number(value) => {
                let number = value
                    .as_u64()
                    .filter(|value| *value <= 9_007_199_254_740_991)
                    .ok_or(TermsWireError::InvalidInput)?;
                write!(self, "{number}").map_err(|_| TermsWireError::LimitExceeded)
            }
            Value::Bool(true) => self.raw("true"),
            Value::Bool(false) => self.raw("false"),
            Value::Null => self.raw("null"),
        }
    }
}
impl fmt::Write for Writer {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.raw(text).map_err(|_| fmt::Error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn canonical_subset_preserves_literals_safe_integers_strings_and_sorted_keys() {
        let input = json!({"z":"é🧺\0\n\"\\","a":[true,false,null,9_007_199_254_740_991u64]});
        let mut count = Writer::counting();
        count.public_snapshot(&input).unwrap();
        let mut output = Writer::emitting(count.len());
        output.public_snapshot(&input).unwrap();
        let wire = output.finish().unwrap();
        assert_eq!(
            wire.as_str(),
            r#"{"a":[true,false,null,9007199254740991],"z":"é🧺\u0000\n\"\\"}"#
        );
        assert_eq!(wire.len(), count.len());
        assert_eq!(serde_json::from_str::<Value>(&wire).unwrap(), input);
    }

    #[test]
    fn canonical_subset_refuses_unknown_field_alphabet_and_unsafe_numeric_types() {
        for input in [
            json!({"é":1}),
            json!(-1),
            json!(0.5),
            json!(9_007_199_254_740_992u64),
        ] {
            assert_eq!(
                Writer::counting().public_snapshot(&input),
                Err(TermsWireError::InvalidInput)
            );
        }
    }
}

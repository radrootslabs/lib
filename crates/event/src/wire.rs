#![forbid(unsafe_code)]

#[cfg(all(not(feature = "std"), not(test)))]
use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    vec::Vec,
};

#[cfg(any(feature = "std", test))]
use std::{collections::BTreeMap, string::String, vec::Vec};

use crate::ids::{
    RadrootsEventId, RadrootsEventSignature, RadrootsIdParseError, RadrootsPublicKey,
};
use crate::{RadrootsEventEnvelope, RadrootsEventEnvelopeError, RadrootsEventEnvelopeParts};
use core::fmt;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

pub const DEFAULT_RAW_JSON_MAX_BYTES: usize = 256 * 1024;
pub const DEFAULT_CONTENT_MAX_BYTES: usize = 128 * 1024;
pub const DEFAULT_TAG_MAX_COUNT: usize = 1024;
pub const DEFAULT_TAG_ELEMENT_MAX_BYTES: usize = 4 * 1024;
pub const DEFAULT_TAG_TOTAL_MAX_BYTES: usize = 128 * 1024;
pub const DEFAULT_EXTRA_MAX_FIELDS: usize = 64;
pub const DEFAULT_EXTRA_TOTAL_JSON_MAX_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RadrootsEventWireLimits {
    pub max_raw_json_bytes: usize,
    pub max_content_bytes: usize,
    pub max_tag_count: usize,
    pub max_tag_element_bytes: usize,
    pub max_total_tag_bytes: usize,
    pub max_extra_fields: usize,
    pub max_total_extra_json_bytes: usize,
}

impl Default for RadrootsEventWireLimits {
    fn default() -> Self {
        Self {
            max_raw_json_bytes: DEFAULT_RAW_JSON_MAX_BYTES,
            max_content_bytes: DEFAULT_CONTENT_MAX_BYTES,
            max_tag_count: DEFAULT_TAG_MAX_COUNT,
            max_tag_element_bytes: DEFAULT_TAG_ELEMENT_MAX_BYTES,
            max_total_tag_bytes: DEFAULT_TAG_TOTAL_MAX_BYTES,
            max_extra_fields: DEFAULT_EXTRA_MAX_FIELDS,
            max_total_extra_json_bytes: DEFAULT_EXTRA_TOTAL_JSON_MAX_BYTES,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsCanonicalEventIdError {
    InvalidPubkey(RadrootsIdParseError),
    InvalidComputedEventId(RadrootsIdParseError),
}

impl fmt::Display for RadrootsCanonicalEventIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPubkey(error) => {
                write!(f, "canonical event id pubkey is invalid: {error}")
            }
            Self::InvalidComputedEventId(error) => {
                write!(f, "canonical event id digest is invalid: {error}")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsCanonicalEventIdError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsEventWireError {
    Json(String),
    RootNotObject,
    MissingField(&'static str),
    InvalidField(&'static str),
    InvalidIdentifier {
        field: &'static str,
        error: RadrootsIdParseError,
    },
    NonCanonicalIdentifier {
        field: &'static str,
    },
    RawJsonTooLarge {
        max: usize,
        actual: usize,
    },
    ContentTooLarge {
        max: usize,
        actual: usize,
    },
    TooManyTags {
        max: usize,
        actual: usize,
    },
    EmptyTag {
        index: usize,
    },
    EmptyTagKey {
        index: usize,
    },
    ControlCharacterTagKey {
        index: usize,
    },
    TagElementTooLarge {
        tag_index: usize,
        element_index: usize,
        max: usize,
        actual: usize,
    },
    TagsTooLarge {
        max: usize,
        actual: usize,
    },
    TooManyExtraFields {
        max: usize,
        actual: usize,
    },
    ExtraJsonTooLarge {
        max: usize,
        actual: usize,
    },
    CanonicalEventId(RadrootsCanonicalEventIdError),
    Envelope(RadrootsEventEnvelopeError),
    EventIdMismatch {
        declared: String,
        computed: String,
    },
}

impl fmt::Display for RadrootsEventWireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(f, "event wire json is invalid: {error}"),
            Self::RootNotObject => write!(f, "event wire root must be a JSON object"),
            Self::MissingField(field) => write!(f, "event wire missing required field {field}"),
            Self::InvalidField(field) => write!(f, "event wire field {field} is invalid"),
            Self::InvalidIdentifier { field, error } => {
                write!(f, "event wire field {field} is invalid: {error}")
            }
            Self::NonCanonicalIdentifier { field } => {
                write!(
                    f,
                    "event wire field {field} must be canonical lowercase hex"
                )
            }
            Self::RawJsonTooLarge { max, actual } => {
                write!(f, "event wire raw JSON size {actual} exceeds {max} bytes")
            }
            Self::ContentTooLarge { max, actual } => {
                write!(f, "event wire content size {actual} exceeds {max} bytes")
            }
            Self::TooManyTags { max, actual } => {
                write!(f, "event wire tag count {actual} exceeds {max}")
            }
            Self::EmptyTag { index } => write!(f, "event wire tag {index} is empty"),
            Self::EmptyTagKey { index } => write!(f, "event wire tag {index} key is empty"),
            Self::ControlCharacterTagKey { index } => {
                write!(f, "event wire tag {index} key contains a control character")
            }
            Self::TagElementTooLarge {
                tag_index,
                element_index,
                max,
                actual,
            } => write!(
                f,
                "event wire tag {tag_index} element {element_index} size {actual} exceeds {max} bytes"
            ),
            Self::TagsTooLarge { max, actual } => {
                write!(f, "event wire tag bytes {actual} exceed {max}")
            }
            Self::TooManyExtraFields { max, actual } => {
                write!(f, "event wire extra field count {actual} exceeds {max}")
            }
            Self::ExtraJsonTooLarge { max, actual } => {
                write!(f, "event wire extra JSON bytes {actual} exceed {max}")
            }
            Self::CanonicalEventId(error) => write!(f, "{error}"),
            Self::Envelope(error) => write!(f, "{error}"),
            Self::EventIdMismatch { declared, computed } => write!(
                f,
                "event wire id mismatch: declared {declared}, computed {computed}"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsEventWireError {}

impl From<RadrootsCanonicalEventIdError> for RadrootsEventWireError {
    fn from(value: RadrootsCanonicalEventIdError) -> Self {
        Self::CanonicalEventId(value)
    }
}

impl From<RadrootsEventEnvelopeError> for RadrootsEventWireError {
    fn from(value: RadrootsEventEnvelopeError) -> Self {
        Self::Envelope(value)
    }
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsNip01EventWireParts {
    pub kind: u32,
    pub content: String,
    pub tags: Vec<Vec<String>>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsNip01EventWire {
    pub id: String,
    pub pubkey: String,
    pub created_at: u64,
    pub kind: u32,
    pub tags: Vec<Vec<String>>,
    pub content: String,
    pub sig: String,
    #[cfg_attr(any(feature = "serde", test), serde(flatten))]
    pub extra: BTreeMap<String, Value>,
}

impl RadrootsNip01EventWire {
    pub fn parse_json(raw_json: &str) -> Result<Self, RadrootsEventWireError> {
        Self::parse_json_with_limits(raw_json, RadrootsEventWireLimits::default())
    }

    pub fn parse_json_with_limits(
        raw_json: &str,
        limits: RadrootsEventWireLimits,
    ) -> Result<Self, RadrootsEventWireError> {
        let raw_len = raw_json.len();
        if raw_len > limits.max_raw_json_bytes {
            return Err(RadrootsEventWireError::RawJsonTooLarge {
                max: limits.max_raw_json_bytes,
                actual: raw_len,
            });
        }
        let value = serde_json::from_str::<Value>(raw_json)
            .map_err(|error| RadrootsEventWireError::Json(error.to_string()))?;
        Self::from_json_value(value, limits)
    }

    pub fn canonical_id_preimage(&self) -> Result<String, RadrootsCanonicalEventIdError> {
        canonical_nip01_event_id_preimage(
            self.pubkey.as_str(),
            self.created_at,
            self.kind,
            &self.tags,
            self.content.as_str(),
        )
    }

    pub fn computed_event_id(&self) -> Result<RadrootsEventId, RadrootsCanonicalEventIdError> {
        compute_canonical_nip01_event_id(
            self.pubkey.as_str(),
            self.created_at,
            self.kind,
            &self.tags,
            self.content.as_str(),
        )
    }

    pub fn verify_id(&self) -> Result<(), RadrootsEventWireError> {
        let computed = self.computed_event_id()?.into_string();
        if computed.as_str() != self.id.as_str() {
            return Err(RadrootsEventWireError::EventIdMismatch {
                declared: self.id.clone(),
                computed,
            });
        }
        Ok(())
    }

    pub fn into_envelope(self) -> Result<RadrootsEventEnvelope, RadrootsEventWireError> {
        self.verify_id()?;
        self.into_envelope_unchecked_id()
            .map_err(RadrootsEventWireError::Envelope)
    }

    pub(crate) fn into_envelope_unchecked_id(
        self,
    ) -> Result<RadrootsEventEnvelope, RadrootsEventEnvelopeError> {
        RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
            id: self.id,
            author: self.pubkey,
            created_at: self.created_at,
            kind: self.kind,
            tags: self.tags,
            content: self.content,
            sig: self.sig,
        })
    }

    fn from_json_value(
        value: Value,
        limits: RadrootsEventWireLimits,
    ) -> Result<Self, RadrootsEventWireError> {
        let mut object = match value {
            Value::Object(object) => object,
            _ => return Err(RadrootsEventWireError::RootNotObject),
        };
        let id = take_canonical_event_id(&mut object)?;
        let pubkey = take_canonical_pubkey(&mut object)?;
        let created_at = take_u64(&mut object, "created_at")?;
        let kind = take_u32(&mut object, "kind")?;
        let tags = take_tags(&mut object, limits)?;
        let content = take_string(&mut object, "content")?;
        let content_len = content.len();
        if content_len > limits.max_content_bytes {
            return Err(RadrootsEventWireError::ContentTooLarge {
                max: limits.max_content_bytes,
                actual: content_len,
            });
        }
        let sig = take_canonical_signature(&mut object)?;
        let extra = validate_extra(object, limits)?;
        let wire = Self {
            id,
            pubkey,
            created_at,
            kind,
            tags,
            content,
            sig,
            extra,
        };
        wire.verify_id()?;
        Ok(wire)
    }
}

pub fn canonical_nip01_event_id_preimage(
    pubkey: &str,
    created_at: u64,
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<String, RadrootsCanonicalEventIdError> {
    let pubkey =
        RadrootsPublicKey::parse(pubkey).map_err(RadrootsCanonicalEventIdError::InvalidPubkey)?;
    let mut preimage = String::new();
    preimage.push_str("[0,");
    push_canonical_json_string(&mut preimage, pubkey.as_str());
    preimage.push(',');
    preimage.push_str(created_at.to_string().as_str());
    preimage.push(',');
    preimage.push_str(kind.to_string().as_str());
    preimage.push_str(",[");
    for (tag_index, tag) in tags.iter().enumerate() {
        if tag_index > 0 {
            preimage.push(',');
        }
        preimage.push('[');
        for (value_index, value) in tag.iter().enumerate() {
            if value_index > 0 {
                preimage.push(',');
            }
            push_canonical_json_string(&mut preimage, value);
        }
        preimage.push(']');
    }
    preimage.push_str("],");
    push_canonical_json_string(&mut preimage, content);
    preimage.push(']');
    Ok(preimage)
}

pub fn compute_canonical_nip01_event_id(
    pubkey: &str,
    created_at: u64,
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<RadrootsEventId, RadrootsCanonicalEventIdError> {
    let preimage = canonical_nip01_event_id_preimage(pubkey, created_at, kind, tags, content)?;
    let digest = Sha256::digest(preimage.as_bytes());
    let event_id = hex::encode(digest);
    RadrootsEventId::parse(event_id).map_err(RadrootsCanonicalEventIdError::InvalidComputedEventId)
}

fn take_string(
    object: &mut Map<String, Value>,
    field: &'static str,
) -> Result<String, RadrootsEventWireError> {
    match object.remove(field) {
        Some(Value::String(value)) => Ok(value),
        Some(_) => Err(RadrootsEventWireError::InvalidField(field)),
        None => Err(RadrootsEventWireError::MissingField(field)),
    }
}

fn take_canonical_event_id(
    object: &mut Map<String, Value>,
) -> Result<String, RadrootsEventWireError> {
    let raw = take_string(object, "id")?;
    let parsed = RadrootsEventId::parse(raw.as_str())
        .map_err(|error| RadrootsEventWireError::InvalidIdentifier { field: "id", error })?;
    canonical_identifier_string("id", raw, parsed.into_string())
}

fn take_canonical_pubkey(
    object: &mut Map<String, Value>,
) -> Result<String, RadrootsEventWireError> {
    let raw = take_string(object, "pubkey")?;
    let parsed = RadrootsPublicKey::parse(raw.as_str()).map_err(|error| {
        RadrootsEventWireError::InvalidIdentifier {
            field: "pubkey",
            error,
        }
    })?;
    canonical_identifier_string("pubkey", raw, parsed.into_string())
}

fn take_canonical_signature(
    object: &mut Map<String, Value>,
) -> Result<String, RadrootsEventWireError> {
    let raw = take_string(object, "sig")?;
    let parsed = RadrootsEventSignature::parse(raw.as_str()).map_err(|error| {
        RadrootsEventWireError::InvalidIdentifier {
            field: "sig",
            error,
        }
    })?;
    canonical_identifier_string("sig", raw, parsed.into_string())
}

fn canonical_identifier_string(
    field: &'static str,
    raw: String,
    canonical: String,
) -> Result<String, RadrootsEventWireError> {
    if canonical.as_str() != raw.as_str() {
        return Err(RadrootsEventWireError::NonCanonicalIdentifier { field });
    }
    Ok(canonical)
}

fn take_u64(
    object: &mut Map<String, Value>,
    field: &'static str,
) -> Result<u64, RadrootsEventWireError> {
    match object.remove(field) {
        Some(Value::Number(value)) => value
            .as_u64()
            .ok_or(RadrootsEventWireError::InvalidField(field)),
        Some(_) => Err(RadrootsEventWireError::InvalidField(field)),
        None => Err(RadrootsEventWireError::MissingField(field)),
    }
}

fn take_u32(
    object: &mut Map<String, Value>,
    field: &'static str,
) -> Result<u32, RadrootsEventWireError> {
    let value = take_u64(object, field)?;
    u32::try_from(value).map_err(|_| RadrootsEventWireError::InvalidField(field))
}

fn take_tags(
    object: &mut Map<String, Value>,
    limits: RadrootsEventWireLimits,
) -> Result<Vec<Vec<String>>, RadrootsEventWireError> {
    let raw_tags = match object.remove("tags") {
        Some(Value::Array(raw_tags)) => raw_tags,
        Some(_) => return Err(RadrootsEventWireError::InvalidField("tags")),
        None => return Err(RadrootsEventWireError::MissingField("tags")),
    };
    let tag_count = raw_tags.len();
    if tag_count > limits.max_tag_count {
        return Err(RadrootsEventWireError::TooManyTags {
            max: limits.max_tag_count,
            actual: tag_count,
        });
    }
    let mut total_tag_bytes = 0usize;
    let mut tags = Vec::with_capacity(tag_count);
    for (tag_index, raw_tag) in raw_tags.into_iter().enumerate() {
        let raw_values = match raw_tag {
            Value::Array(values) => values,
            _ => return Err(RadrootsEventWireError::InvalidField("tags")),
        };
        if raw_values.is_empty() {
            return Err(RadrootsEventWireError::EmptyTag { index: tag_index });
        }
        let mut tag = Vec::with_capacity(raw_values.len());
        for (element_index, raw_value) in raw_values.into_iter().enumerate() {
            let value = match raw_value {
                Value::String(value) => value,
                _ => return Err(RadrootsEventWireError::InvalidField("tags")),
            };
            let value_len = value.len();
            if value_len > limits.max_tag_element_bytes {
                return Err(RadrootsEventWireError::TagElementTooLarge {
                    tag_index,
                    element_index,
                    max: limits.max_tag_element_bytes,
                    actual: value_len,
                });
            }
            if element_index == 0 {
                validate_tag_key(tag_index, value.as_str())?;
            }
            total_tag_bytes = total_tag_bytes.saturating_add(value_len);
            if total_tag_bytes > limits.max_total_tag_bytes {
                return Err(RadrootsEventWireError::TagsTooLarge {
                    max: limits.max_total_tag_bytes,
                    actual: total_tag_bytes,
                });
            }
            tag.push(value);
        }
        tags.push(tag);
    }
    Ok(tags)
}

fn validate_tag_key(index: usize, value: &str) -> Result<(), RadrootsEventWireError> {
    if value.is_empty() {
        return Err(RadrootsEventWireError::EmptyTagKey { index });
    }
    if value.chars().any(char::is_control) {
        return Err(RadrootsEventWireError::ControlCharacterTagKey { index });
    }
    Ok(())
}

fn validate_extra(
    object: Map<String, Value>,
    limits: RadrootsEventWireLimits,
) -> Result<BTreeMap<String, Value>, RadrootsEventWireError> {
    let extra_count = object.len();
    if extra_count > limits.max_extra_fields {
        return Err(RadrootsEventWireError::TooManyExtraFields {
            max: limits.max_extra_fields,
            actual: extra_count,
        });
    }
    let mut total_json_bytes = 0usize;
    let mut extra = BTreeMap::new();
    for (key, value) in object {
        let key_json_len = serialized_json_string_len(&key);
        let value_json_len = serialized_json_value_len(&value);
        total_json_bytes = total_json_bytes
            .saturating_add(key_json_len)
            .saturating_add(1)
            .saturating_add(value_json_len);
        if total_json_bytes > limits.max_total_extra_json_bytes {
            return Err(RadrootsEventWireError::ExtraJsonTooLarge {
                max: limits.max_total_extra_json_bytes,
                actual: total_json_bytes,
            });
        }
        extra.insert(key, value);
    }
    Ok(extra)
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn serialized_json_string_len(value: &String) -> usize {
    serde_json::to_vec(value)
        .expect("JSON strings always serialize")
        .len()
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn serialized_json_value_len(value: &Value) -> usize {
    serde_json::to_vec(value)
        .expect("JSON values always serialize")
        .len()
}

fn push_canonical_json_string(target: &mut String, value: &str) {
    target.push('"');
    for character in value.chars() {
        match character {
            '"' => target.push_str("\\\""),
            '\\' => target.push_str("\\\\"),
            '\n' => target.push_str("\\n"),
            '\r' => target.push_str("\\r"),
            '\t' => target.push_str("\\t"),
            '\u{08}' => target.push_str("\\b"),
            '\u{0c}' => target.push_str("\\f"),
            '\u{00}'..='\u{1f}' => push_unicode_escape(target, character),
            _ => target.push(character),
        }
    }
    target.push('"');
}

fn push_unicode_escape(target: &mut String, character: char) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let value = character as u32;
    target.push_str("\\u00");
    target.push(HEX[((value >> 4) & 0x0f) as usize] as char);
    target.push(HEX[(value & 0x0f) as usize] as char);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn hex_64(character: char) -> String {
        core::iter::repeat_n(character, 64).collect()
    }

    fn hex_128(character: char) -> String {
        core::iter::repeat_n(character, 128).collect()
    }

    fn valid_event_value(content: &str, tags: Vec<Vec<String>>) -> Value {
        let pubkey = hex_64('a');
        let id =
            compute_canonical_nip01_event_id(pubkey.as_str(), 1_700_000_000, 1, &tags, content)
                .expect("event id")
                .into_string();
        json!({
            "id": id,
            "pubkey": pubkey,
            "created_at": 1_700_000_000u64,
            "kind": 1u32,
            "tags": tags,
            "content": content,
            "sig": hex_128('b')
        })
    }

    fn raw_json(value: &Value) -> String {
        serde_json::to_string(value).expect("event json")
    }

    fn valid_event_json(content: &str, tags: Vec<Vec<String>>) -> String {
        raw_json(&valid_event_value(content, tags))
    }

    fn default_tags() -> Vec<Vec<String>> {
        vec![vec!["t".to_owned(), "soil".to_owned()]]
    }

    #[test]
    fn canonical_preimage_escapes_required_json_characters() {
        let preimage = canonical_nip01_event_id_preimage(
            hex_64('a').as_str(),
            10,
            1,
            &[vec!["t".to_owned(), "line\nsoil".to_owned()]],
            "\"\\\n\r\t\u{08}\u{0c}\u{01}",
        )
        .expect("preimage");

        assert_eq!(
            preimage,
            r#"[0,"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",10,1,[["t","line\nsoil"]],"\"\\\n\r\t\b\f\u0001"]"#
        );
    }

    #[test]
    fn parses_wire_json_preserves_extra_and_verifies_id() {
        let mut value = valid_event_value("hello", default_tags());
        value
            .as_object_mut()
            .expect("object")
            .insert("client".to_owned(), json!({"name":"radroots-test"}));
        let wire = RadrootsNip01EventWire::parse_json(raw_json(&value).as_str()).expect("wire");

        assert_eq!(wire.pubkey, hex_64('a'));
        assert_eq!(wire.created_at, 1_700_000_000);
        assert_eq!(wire.kind, 1);
        assert_eq!(wire.tags, default_tags());
        assert_eq!(wire.content, "hello");
        assert_eq!(
            wire.extra.get("client").expect("client extra"),
            &json!({"name":"radroots-test"})
        );
        wire.verify_id().expect("verified id");
        assert_eq!(
            wire.canonical_id_preimage().expect("preimage"),
            r#"[0,"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",1700000000,1,[["t","soil"]],"hello"]"#
        );
    }

    #[test]
    fn into_envelope_verifies_id_before_domain_conversion() {
        let wire =
            RadrootsNip01EventWire::parse_json(valid_event_json("hello", default_tags()).as_str())
                .expect("wire");

        let envelope = wire.clone().into_envelope().expect("envelope");
        assert_eq!(envelope.id_str(), wire.id);
        assert_eq!(envelope.content(), "hello");

        let mut tampered_id = wire.clone();
        tampered_id.id = hex_64('f');
        assert!(matches!(
            tampered_id.into_envelope(),
            Err(RadrootsEventWireError::EventIdMismatch { .. })
        ));

        let mut tampered_content = wire;
        tampered_content.content = "tampered".to_owned();
        assert!(matches!(
            tampered_content.into_envelope(),
            Err(RadrootsEventWireError::EventIdMismatch { .. })
        ));
    }

    #[test]
    fn into_envelope_ignores_extra_for_id_and_propagates_domain_limits() {
        let mut value = valid_event_value("hello", default_tags());
        value
            .as_object_mut()
            .expect("object")
            .insert("client".to_owned(), json!("radroots-test"));
        let wire = RadrootsNip01EventWire::parse_json(raw_json(&value).as_str()).expect("wire");
        let envelope = wire.into_envelope().expect("envelope");
        assert_eq!(envelope.content(), "hello");

        let content = core::iter::repeat_n('x', DEFAULT_CONTENT_MAX_BYTES + 1).collect::<String>();
        let tags = default_tags();
        let pubkey = hex_64('a');
        let id = compute_canonical_nip01_event_id(
            pubkey.as_str(),
            1_700_000_000,
            1,
            &tags,
            content.as_str(),
        )
        .expect("event id")
        .into_string();
        let wire = RadrootsNip01EventWire {
            id,
            pubkey,
            created_at: 1_700_000_000,
            kind: 1,
            tags,
            content,
            sig: hex_128('b'),
            extra: Default::default(),
        };

        assert!(matches!(
            wire.into_envelope(),
            Err(RadrootsEventWireError::Envelope(
                RadrootsEventEnvelopeError::ContentTooLarge { .. }
            ))
        ));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_flatten_preserves_extra_fields() {
        let mut value = valid_event_value("hello", default_tags());
        value
            .as_object_mut()
            .expect("object")
            .insert("client".to_owned(), json!("radroots-test"));
        let wire = RadrootsNip01EventWire::parse_json(raw_json(&value).as_str()).expect("wire");
        let encoded = serde_json::to_value(&wire).expect("encoded");

        assert_eq!(encoded.get("client"), Some(&json!("radroots-test")));
        assert_eq!(encoded.get("id"), Some(&Value::String(wire.id)));
    }

    #[test]
    fn parse_json_rejects_required_field_errors_and_id_mismatch() {
        let mut value = valid_event_value("hello", default_tags());
        value.as_object_mut().expect("object").remove("id");
        assert!(matches!(
            RadrootsNip01EventWire::parse_json(raw_json(&value).as_str()),
            Err(RadrootsEventWireError::MissingField("id"))
        ));

        let mut value = valid_event_value("hello", default_tags());
        value
            .as_object_mut()
            .expect("object")
            .insert("pubkey".to_owned(), json!("not-hex"));
        assert!(matches!(
            RadrootsNip01EventWire::parse_json(raw_json(&value).as_str()),
            Err(RadrootsEventWireError::InvalidIdentifier {
                field: "pubkey",
                ..
            })
        ));

        let mut value = valid_event_value("hello", default_tags());
        value
            .as_object_mut()
            .expect("object")
            .insert("sig".to_owned(), json!(hex_64('b')));
        assert!(matches!(
            RadrootsNip01EventWire::parse_json(raw_json(&value).as_str()),
            Err(RadrootsEventWireError::InvalidIdentifier { field: "sig", .. })
        ));

        let mut value = valid_event_value("hello", default_tags());
        value
            .as_object_mut()
            .expect("object")
            .insert("id".to_owned(), json!(hex_64('f')));
        assert!(matches!(
            RadrootsNip01EventWire::parse_json(raw_json(&value).as_str()),
            Err(RadrootsEventWireError::EventIdMismatch { .. })
        ));

        let mut value = valid_event_value("hello", default_tags());
        value
            .as_object_mut()
            .expect("object")
            .insert("id".to_owned(), json!(hex_64('A')));
        assert!(matches!(
            RadrootsNip01EventWire::parse_json(raw_json(&value).as_str()),
            Err(RadrootsEventWireError::NonCanonicalIdentifier { field: "id" })
        ));
    }

    #[test]
    fn parse_json_rejects_tag_shape_errors() {
        let mut value = valid_event_value("hello", default_tags());
        value
            .as_object_mut()
            .expect("object")
            .insert("tags".to_owned(), json!([[]]));
        assert!(matches!(
            RadrootsNip01EventWire::parse_json(raw_json(&value).as_str()),
            Err(RadrootsEventWireError::EmptyTag { index: 0 })
        ));

        let mut value = valid_event_value("hello", default_tags());
        value
            .as_object_mut()
            .expect("object")
            .insert("tags".to_owned(), json!([["", "soil"]]));
        assert!(matches!(
            RadrootsNip01EventWire::parse_json(raw_json(&value).as_str()),
            Err(RadrootsEventWireError::EmptyTagKey { index: 0 })
        ));

        let mut value = valid_event_value("hello", default_tags());
        value
            .as_object_mut()
            .expect("object")
            .insert("tags".to_owned(), json!([["t\n", "soil"]]));
        assert!(matches!(
            RadrootsNip01EventWire::parse_json(raw_json(&value).as_str()),
            Err(RadrootsEventWireError::ControlCharacterTagKey { index: 0 })
        ));
    }

    #[test]
    fn parse_json_rejects_resource_budget_violations() {
        let raw = valid_event_json("hello", default_tags());
        assert!(matches!(
            RadrootsNip01EventWire::parse_json_with_limits(
                raw.as_str(),
                RadrootsEventWireLimits {
                    max_raw_json_bytes: 1,
                    ..RadrootsEventWireLimits::default()
                }
            ),
            Err(RadrootsEventWireError::RawJsonTooLarge { .. })
        ));

        let raw = valid_event_json("hello", default_tags());
        assert!(matches!(
            RadrootsNip01EventWire::parse_json_with_limits(
                raw.as_str(),
                RadrootsEventWireLimits {
                    max_content_bytes: 1,
                    ..RadrootsEventWireLimits::default()
                }
            ),
            Err(RadrootsEventWireError::ContentTooLarge { .. })
        ));

        let raw = valid_event_json("hello", default_tags());
        assert!(matches!(
            RadrootsNip01EventWire::parse_json_with_limits(
                raw.as_str(),
                RadrootsEventWireLimits {
                    max_tag_count: 0,
                    ..RadrootsEventWireLimits::default()
                }
            ),
            Err(RadrootsEventWireError::TooManyTags { .. })
        ));

        let raw = valid_event_json("hello", vec![vec!["t".to_owned(), "soil".to_owned()]]);
        assert!(matches!(
            RadrootsNip01EventWire::parse_json_with_limits(
                raw.as_str(),
                RadrootsEventWireLimits {
                    max_tag_element_bytes: 1,
                    ..RadrootsEventWireLimits::default()
                }
            ),
            Err(RadrootsEventWireError::TagElementTooLarge { .. })
        ));

        let raw = valid_event_json("hello", default_tags());
        assert!(matches!(
            RadrootsNip01EventWire::parse_json_with_limits(
                raw.as_str(),
                RadrootsEventWireLimits {
                    max_total_tag_bytes: 1,
                    ..RadrootsEventWireLimits::default()
                }
            ),
            Err(RadrootsEventWireError::TagsTooLarge { .. })
        ));

        let mut value = valid_event_value("hello", default_tags());
        value
            .as_object_mut()
            .expect("object")
            .insert("client".to_owned(), json!("radroots-test"));
        let raw = raw_json(&value);
        assert!(matches!(
            RadrootsNip01EventWire::parse_json_with_limits(
                raw.as_str(),
                RadrootsEventWireLimits {
                    max_extra_fields: 0,
                    ..RadrootsEventWireLimits::default()
                }
            ),
            Err(RadrootsEventWireError::TooManyExtraFields { .. })
        ));

        assert!(matches!(
            RadrootsNip01EventWire::parse_json_with_limits(
                raw.as_str(),
                RadrootsEventWireLimits {
                    max_total_extra_json_bytes: 1,
                    ..RadrootsEventWireLimits::default()
                }
            ),
            Err(RadrootsEventWireError::ExtraJsonTooLarge { .. })
        ));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn wire_parser_and_error_contracts_cover_all_typed_failures() {
        let parse_error = RadrootsEventId::parse("bad").expect_err("invalid id");
        for error in [
            RadrootsEventWireError::Json("bad json".to_owned()),
            RadrootsEventWireError::RootNotObject,
            RadrootsEventWireError::MissingField("id"),
            RadrootsEventWireError::InvalidField("kind"),
            RadrootsEventWireError::InvalidIdentifier {
                field: "id",
                error: parse_error.clone(),
            },
            RadrootsEventWireError::NonCanonicalIdentifier { field: "id" },
            RadrootsEventWireError::RawJsonTooLarge { max: 1, actual: 2 },
            RadrootsEventWireError::ContentTooLarge { max: 1, actual: 2 },
            RadrootsEventWireError::TooManyTags { max: 1, actual: 2 },
            RadrootsEventWireError::EmptyTag { index: 1 },
            RadrootsEventWireError::EmptyTagKey { index: 1 },
            RadrootsEventWireError::ControlCharacterTagKey { index: 1 },
            RadrootsEventWireError::TagElementTooLarge {
                tag_index: 1,
                element_index: 2,
                max: 3,
                actual: 4,
            },
            RadrootsEventWireError::TagsTooLarge { max: 1, actual: 2 },
            RadrootsEventWireError::TooManyExtraFields { max: 1, actual: 2 },
            RadrootsEventWireError::ExtraJsonTooLarge { max: 1, actual: 2 },
            RadrootsEventWireError::from(RadrootsCanonicalEventIdError::InvalidPubkey(
                parse_error.clone(),
            )),
            RadrootsEventWireError::from(RadrootsEventEnvelopeError::NonCanonicalId),
            RadrootsEventWireError::EventIdMismatch {
                declared: "a".to_owned(),
                computed: "b".to_owned(),
            },
        ] {
            assert!(!error.to_string().is_empty());
        }

        for raw in ["{", "[]", "null"] {
            assert!(RadrootsNip01EventWire::parse_json(raw).is_err());
        }

        for (field, replacement) in [
            ("id", json!(7)),
            ("id", json!("bad")),
            ("pubkey", json!(7)),
            ("pubkey", json!(hex_64('A'))),
            ("created_at", json!("bad")),
            ("created_at", json!(-1)),
            ("kind", json!("bad")),
            ("kind", json!(u64::from(u32::MAX) + 1)),
            ("tags", json!("bad")),
            ("tags", json!(["bad"])),
            ("tags", json!([["t", 7]])),
            ("content", json!(7)),
            ("sig", json!(7)),
            ("sig", json!("bad")),
            ("sig", json!(hex_128('B'))),
        ] {
            let mut value = valid_event_value("hello", default_tags());
            value
                .as_object_mut()
                .expect("object")
                .insert(field.to_owned(), replacement);
            assert!(RadrootsNip01EventWire::parse_json(raw_json(&value).as_str()).is_err());
        }

        for field in ["pubkey", "created_at", "kind", "tags", "content", "sig"] {
            let mut value = valid_event_value("hello", default_tags());
            value.as_object_mut().expect("object").remove(field);
            assert!(RadrootsNip01EventWire::parse_json(raw_json(&value).as_str()).is_err());
        }
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn checked_in_conformance_vectors_match_wire_behavior() {
        let vectors =
            include_str!("../../../contracts/conformance/vectors/event/nip01_wire.v1.json");
        let document: Value = serde_json::from_str(vectors).expect("vectors json");
        let entries = document
            .get("vectors")
            .and_then(Value::as_array)
            .expect("vector entries");

        for entry in entries {
            match entry.get("kind").and_then(Value::as_str).expect("kind") {
                "event.nip01_wire.valid" => {
                    let raw = entry
                        .get("input")
                        .and_then(|input| input.get("raw_json"))
                        .and_then(Value::as_str)
                        .expect("raw json");
                    let expected = entry.get("expected").expect("expected");
                    let wire = RadrootsNip01EventWire::parse_json(raw).expect("wire");
                    assert_eq!(
                        wire.canonical_id_preimage().expect("preimage"),
                        expected
                            .get("canonical_id_preimage")
                            .and_then(Value::as_str)
                            .expect("expected preimage")
                    );
                    assert_eq!(
                        wire.computed_event_id().expect("event id").as_str(),
                        expected
                            .get("computed_event_id")
                            .and_then(Value::as_str)
                            .expect("expected event id")
                    );
                }
                "event.nip01_wire.invalid" => {
                    let raw = entry
                        .get("input")
                        .and_then(|input| input.get("raw_json"))
                        .and_then(Value::as_str)
                        .expect("raw json");
                    assert!(RadrootsNip01EventWire::parse_json(raw).is_err());
                }
                other => panic!("unknown event wire vector kind {other}"),
            }
        }
    }
}

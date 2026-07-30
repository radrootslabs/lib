#![forbid(unsafe_code)]

//! Frozen NIP-01 wire-v1 parsing and canonical-identifier semantics.

#[cfg(all(not(feature = "std"), not(test)))]
use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    vec::Vec,
};

#[cfg(any(feature = "std", test))]
use std::{collections::BTreeMap, string::String, vec::Vec};

use crate::envelope::{EventEnvelope, EventEnvelopeError, EventEnvelopeParts};
use crate::id::{EventId, EventSignature, ParseError, parse_public_key};
use core::fmt;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

pub const DEFAULT_RAW_JSON_MAX_BYTES: usize = 256 * 1024;
pub const DEFAULT_CONTENT_MAX_BYTES: usize = 128 * 1024;
pub const DEFAULT_TAG_MAX_COUNT: usize = 1024;
pub const DEFAULT_TAG_TOTAL_ELEMENT_MAX_COUNT: usize = 4096;
pub const DEFAULT_TAG_ELEMENT_MAX_BYTES: usize = 4 * 1024;
pub const DEFAULT_TAG_TOTAL_MAX_BYTES: usize = 128 * 1024;
pub const DEFAULT_EXTRA_MAX_FIELDS: usize = 64;
pub const DEFAULT_EXTRA_TOTAL_JSON_MAX_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventWireLimits {
    pub max_raw_json_bytes: usize,
    pub max_content_bytes: usize,
    pub max_tag_count: usize,
    pub max_total_tag_elements: usize,
    pub max_tag_element_bytes: usize,
    pub max_total_tag_bytes: usize,
    pub max_extra_fields: usize,
    pub max_total_extra_json_bytes: usize,
}

impl Default for EventWireLimits {
    fn default() -> Self {
        Self {
            max_raw_json_bytes: DEFAULT_RAW_JSON_MAX_BYTES,
            max_content_bytes: DEFAULT_CONTENT_MAX_BYTES,
            max_tag_count: DEFAULT_TAG_MAX_COUNT,
            max_total_tag_elements: DEFAULT_TAG_TOTAL_ELEMENT_MAX_COUNT,
            max_tag_element_bytes: DEFAULT_TAG_ELEMENT_MAX_BYTES,
            max_total_tag_bytes: DEFAULT_TAG_TOTAL_MAX_BYTES,
            max_extra_fields: DEFAULT_EXTRA_MAX_FIELDS,
            max_total_extra_json_bytes: DEFAULT_EXTRA_TOTAL_JSON_MAX_BYTES,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CanonicalEventIdError {
    InvalidPubkey(ParseError),
    InvalidComputedEventId(ParseError),
}

impl fmt::Display for CanonicalEventIdError {
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
impl std::error::Error for CanonicalEventIdError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventWireError {
    Json(String),
    RootNotObject,
    MissingField(&'static str),
    InvalidField(&'static str),
    InvalidIdentifier {
        field: &'static str,
        error: ParseError,
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
    TooManyTagElements {
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
    CanonicalEventId(CanonicalEventIdError),
    Envelope(EventEnvelopeError),
    EventIdMismatch {
        declared: String,
        computed: String,
    },
}

impl fmt::Display for EventWireError {
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
            Self::TooManyTagElements { max, actual } => {
                write!(f, "event wire tag element count {actual} exceeds {max}")
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
impl std::error::Error for EventWireError {}

impl From<CanonicalEventIdError> for EventWireError {
    fn from(value: CanonicalEventIdError) -> Self {
        Self::CanonicalEventId(value)
    }
}

impl From<EventEnvelopeError> for EventWireError {
    fn from(value: EventEnvelopeError) -> Self {
        Self::Envelope(value)
    }
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Nip01EventWireParts {
    pub kind: u32,
    pub content: String,
    pub tags: Vec<Vec<String>>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Nip01EventWire {
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

impl Nip01EventWire {
    pub fn parse_json(raw_json: &str) -> Result<Self, EventWireError> {
        Self::parse_json_with_limits(raw_json, EventWireLimits::default())
    }

    /// Parses and structurally validates an event without verifying its ID.
    ///
    /// This boundary enforces every [`EventWireLimits`] budget. Callers must
    /// invoke [`Self::verify_id`] before treating the result as ID-verified.
    pub fn parse_json_unverified(raw_json: &str) -> Result<Self, EventWireError> {
        Self::parse_json_unverified_with_limits(raw_json, EventWireLimits::default())
    }

    pub fn parse_json_with_limits(
        raw_json: &str,
        limits: EventWireLimits,
    ) -> Result<Self, EventWireError> {
        let wire = Self::parse_json_unverified_with_limits(raw_json, limits)?;
        wire.verify_id()?;
        Ok(wire)
    }

    /// Parses and structurally validates an event under explicit limits
    /// without verifying its ID.
    pub fn parse_json_unverified_with_limits(
        raw_json: &str,
        limits: EventWireLimits,
    ) -> Result<Self, EventWireError> {
        let raw_len = raw_json.len();
        if raw_len > limits.max_raw_json_bytes {
            return Err(EventWireError::RawJsonTooLarge {
                max: limits.max_raw_json_bytes,
                actual: raw_len,
            });
        }
        let value = serde_json::from_str::<Value>(raw_json)
            .map_err(|error| EventWireError::Json(error.to_string()))?;
        Self::from_json_value(value, limits)
    }

    pub fn canonical_id_preimage(&self) -> Result<String, CanonicalEventIdError> {
        canonical_nip01_event_id_preimage(
            self.pubkey.as_str(),
            self.created_at,
            self.kind,
            &self.tags,
            self.content.as_str(),
        )
    }

    pub fn computed_event_id(&self) -> Result<EventId, CanonicalEventIdError> {
        compute_canonical_nip01_event_id(
            self.pubkey.as_str(),
            self.created_at,
            self.kind,
            &self.tags,
            self.content.as_str(),
        )
    }

    pub fn verify_id(&self) -> Result<(), EventWireError> {
        let computed = self.computed_event_id()?.into_string();
        if computed.as_str() != self.id.as_str() {
            return Err(EventWireError::EventIdMismatch {
                declared: self.id.clone(),
                computed,
            });
        }
        Ok(())
    }

    pub fn into_envelope(self) -> Result<EventEnvelope, EventWireError> {
        self.verify_id()?;
        self.into_unverified_envelope()
            .map_err(EventWireError::Envelope)
    }

    /// Converts structurally validated wire data without verifying its ID.
    ///
    /// The returned envelope remains untrusted until an admission typestate
    /// transition verifies its canonical ID and signature.
    pub fn into_unverified_envelope(self) -> Result<EventEnvelope, EventEnvelopeError> {
        EventEnvelope::new(EventEnvelopeParts {
            id: self.id,
            author: self.pubkey,
            created_at: self.created_at,
            kind: self.kind,
            tags: self.tags,
            content: self.content,
            sig: self.sig,
        })
    }

    fn from_json_value(value: Value, limits: EventWireLimits) -> Result<Self, EventWireError> {
        let mut object = match value {
            Value::Object(object) => object,
            _ => return Err(EventWireError::RootNotObject),
        };
        let id = take_canonical_event_id(&mut object)?;
        let pubkey = take_canonical_pubkey(&mut object)?;
        let created_at = take_u64(&mut object, "created_at")?;
        let kind = take_u32(&mut object, "kind")?;
        let tags = take_tags(&mut object, limits)?;
        let content = take_string(&mut object, "content")?;
        let content_len = content.len();
        if content_len > limits.max_content_bytes {
            return Err(EventWireError::ContentTooLarge {
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
        Ok(wire)
    }
}

pub fn canonical_nip01_event_id_preimage(
    pubkey: &str,
    created_at: u64,
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<String, CanonicalEventIdError> {
    canonical_nip01_event_id_preimage_v1(pubkey, created_at, kind, tags, content)
}

/// Serializes the canonical NIP-01 event-id preimage with wire-v1 semantics.
pub fn canonical_nip01_event_id_preimage_v1(
    pubkey: &str,
    created_at: u64,
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<String, CanonicalEventIdError> {
    let pubkey = parse_public_key(pubkey).map_err(CanonicalEventIdError::InvalidPubkey)?;
    let pubkey = pubkey.to_hex();
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
) -> Result<EventId, CanonicalEventIdError> {
    compute_canonical_nip01_event_id_v1(pubkey, created_at, kind, tags, content)
}

/// Computes the canonical NIP-01 event identifier with wire-v1 semantics.
pub fn compute_canonical_nip01_event_id_v1(
    pubkey: &str,
    created_at: u64,
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<EventId, CanonicalEventIdError> {
    let preimage = canonical_nip01_event_id_preimage_v1(pubkey, created_at, kind, tags, content)?;
    let digest = Sha256::digest(preimage.as_bytes());
    let event_id = hex::encode(digest);
    EventId::parse(event_id).map_err(CanonicalEventIdError::InvalidComputedEventId)
}

fn take_string(
    object: &mut Map<String, Value>,
    field: &'static str,
) -> Result<String, EventWireError> {
    match object.remove(field) {
        Some(Value::String(value)) => Ok(value),
        Some(_) => Err(EventWireError::InvalidField(field)),
        None => Err(EventWireError::MissingField(field)),
    }
}

fn take_canonical_event_id(object: &mut Map<String, Value>) -> Result<String, EventWireError> {
    let raw = take_string(object, "id")?;
    let parsed = EventId::parse(raw.as_str())
        .map_err(|error| EventWireError::InvalidIdentifier { field: "id", error })?;
    canonical_identifier_string("id", raw, parsed.into_string())
}

fn take_canonical_pubkey(object: &mut Map<String, Value>) -> Result<String, EventWireError> {
    let raw = take_string(object, "pubkey")?;
    let parsed =
        parse_public_key(raw.as_str()).map_err(|error| EventWireError::InvalidIdentifier {
            field: "pubkey",
            error,
        })?;
    canonical_identifier_string("pubkey", raw, parsed.to_hex())
}

fn take_canonical_signature(object: &mut Map<String, Value>) -> Result<String, EventWireError> {
    let raw = take_string(object, "sig")?;
    let parsed =
        EventSignature::parse(raw.as_str()).map_err(|error| EventWireError::InvalidIdentifier {
            field: "sig",
            error,
        })?;
    canonical_identifier_string("sig", raw, parsed.into_string())
}

fn canonical_identifier_string(
    field: &'static str,
    raw: String,
    canonical: String,
) -> Result<String, EventWireError> {
    if canonical.as_str() != raw.as_str() {
        return Err(EventWireError::NonCanonicalIdentifier { field });
    }
    Ok(canonical)
}

fn take_u64(object: &mut Map<String, Value>, field: &'static str) -> Result<u64, EventWireError> {
    match object.remove(field) {
        Some(Value::Number(value)) => value.as_u64().ok_or(EventWireError::InvalidField(field)),
        Some(_) => Err(EventWireError::InvalidField(field)),
        None => Err(EventWireError::MissingField(field)),
    }
}

fn take_u32(object: &mut Map<String, Value>, field: &'static str) -> Result<u32, EventWireError> {
    let value = take_u64(object, field)?;
    u32::try_from(value).map_err(|_| EventWireError::InvalidField(field))
}

fn take_tags(
    object: &mut Map<String, Value>,
    limits: EventWireLimits,
) -> Result<Vec<Vec<String>>, EventWireError> {
    let raw_tags = match object.remove("tags") {
        Some(Value::Array(raw_tags)) => raw_tags,
        Some(_) => return Err(EventWireError::InvalidField("tags")),
        None => return Err(EventWireError::MissingField("tags")),
    };
    let tag_count = raw_tags.len();
    if tag_count > limits.max_tag_count {
        return Err(EventWireError::TooManyTags {
            max: limits.max_tag_count,
            actual: tag_count,
        });
    }
    let total_tag_elements = raw_tags.iter().try_fold(0usize, |total, raw_tag| {
        let Value::Array(values) = raw_tag else {
            return Err(EventWireError::InvalidField("tags"));
        };
        Ok(total.saturating_add(values.len()))
    })?;
    if total_tag_elements > limits.max_total_tag_elements {
        return Err(EventWireError::TooManyTagElements {
            max: limits.max_total_tag_elements,
            actual: total_tag_elements,
        });
    }
    let mut total_tag_bytes = 0usize;
    let mut tags = Vec::with_capacity(tag_count);
    for (tag_index, raw_tag) in raw_tags.into_iter().enumerate() {
        let raw_values = match raw_tag {
            Value::Array(values) => values,
            _ => return Err(EventWireError::InvalidField("tags")),
        };
        if raw_values.is_empty() {
            return Err(EventWireError::EmptyTag { index: tag_index });
        }
        let mut tag = Vec::with_capacity(raw_values.len());
        for (element_index, raw_value) in raw_values.into_iter().enumerate() {
            let value = match raw_value {
                Value::String(value) => value,
                _ => return Err(EventWireError::InvalidField("tags")),
            };
            let value_len = value.len();
            if value_len > limits.max_tag_element_bytes {
                return Err(EventWireError::TagElementTooLarge {
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
                return Err(EventWireError::TagsTooLarge {
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

fn validate_tag_key(index: usize, value: &str) -> Result<(), EventWireError> {
    if value.is_empty() {
        return Err(EventWireError::EmptyTagKey { index });
    }
    if value.chars().any(char::is_control) {
        return Err(EventWireError::ControlCharacterTagKey { index });
    }
    Ok(())
}

fn validate_extra(
    object: Map<String, Value>,
    limits: EventWireLimits,
) -> Result<BTreeMap<String, Value>, EventWireError> {
    let extra_count = object.len();
    if extra_count > limits.max_extra_fields {
        return Err(EventWireError::TooManyExtraFields {
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
            return Err(EventWireError::ExtraJsonTooLarge {
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
mod tests;

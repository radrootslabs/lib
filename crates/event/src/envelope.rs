#![forbid(unsafe_code)]

#[cfg(all(not(feature = "std"), not(test)))]
use alloc::{string::String, string::ToString, vec::Vec};

#[cfg(any(feature = "std", test))]
use std::{string::String, vec::Vec};

use crate::ids::{
    RadrootsEventId, RadrootsEventSignature, RadrootsIdParseError, RadrootsPublicKey,
};
use crate::wire::v1::{
    DEFAULT_CONTENT_MAX_BYTES, DEFAULT_TAG_ELEMENT_MAX_BYTES, DEFAULT_TAG_MAX_COUNT,
    DEFAULT_TAG_TOTAL_ELEMENT_MAX_COUNT, DEFAULT_TAG_TOTAL_MAX_BYTES, RadrootsNip01EventWire,
};
use core::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsEventTimestamp(u64);

impl RadrootsEventTimestamp {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[inline]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

impl From<u64> for RadrootsEventTimestamp {
    #[inline]
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

#[cfg(any(feature = "serde", test))]
impl serde::Serialize for RadrootsEventTimestamp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u64(self.0)
    }
}

#[cfg(any(feature = "serde", test))]
impl<'de> serde::Deserialize<'de> for RadrootsEventTimestamp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = u64::deserialize(deserializer)?;
        Ok(Self::new(value))
    }
}

#[cfg(feature = "dto-bindgen")]
impl dto_bindgen::Dto for RadrootsEventTimestamp {
    fn describe(ctx: &mut dto_bindgen::__private::DescribeCtx) -> dto_bindgen::__private::TypeRef {
        <u64 as dto_bindgen::Dto>::describe(ctx)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsEventKind(u32);

impl RadrootsEventKind {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[inline]
    pub const fn as_u32(self) -> u32 {
        self.0
    }

    pub const fn class(self) -> RadrootsEventKindClass {
        match self.0 {
            0 | 3 => RadrootsEventKindClass::Replaceable,
            10_000..=19_999 => RadrootsEventKindClass::Replaceable,
            20_000..=29_999 => RadrootsEventKindClass::Ephemeral,
            30_000..=39_999 => RadrootsEventKindClass::Addressable,
            _ => RadrootsEventKindClass::Regular,
        }
    }
}

impl From<u32> for RadrootsEventKind {
    #[inline]
    fn from(value: u32) -> Self {
        Self::new(value)
    }
}

#[cfg(any(feature = "serde", test))]
impl serde::Serialize for RadrootsEventKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u32(self.0)
    }
}

#[cfg(any(feature = "serde", test))]
impl<'de> serde::Deserialize<'de> for RadrootsEventKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = u32::deserialize(deserializer)?;
        Ok(Self::new(value))
    }
}

#[cfg(feature = "dto-bindgen")]
impl dto_bindgen::Dto for RadrootsEventKind {
    fn describe(ctx: &mut dto_bindgen::__private::DescribeCtx) -> dto_bindgen::__private::TypeRef {
        <u32 as dto_bindgen::Dto>::describe(ctx)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RadrootsEventKindClass {
    Regular,
    Replaceable,
    Ephemeral,
    Addressable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsEventEnvelopeError {
    InvalidId(RadrootsIdParseError),
    InvalidAuthor(RadrootsIdParseError),
    InvalidSignature(RadrootsIdParseError),
    NonCanonicalId,
    NonCanonicalAuthor,
    NonCanonicalSignature,
    EmptyTag {
        index: usize,
    },
    EmptyTagKey {
        index: usize,
    },
    ControlCharacterTagKey {
        index: usize,
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
}

impl fmt::Display for RadrootsEventEnvelopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidId(error) => write!(f, "event envelope id is invalid: {error}"),
            Self::InvalidAuthor(error) => write!(f, "event envelope author is invalid: {error}"),
            Self::InvalidSignature(error) => {
                write!(f, "event envelope signature is invalid: {error}")
            }
            Self::NonCanonicalId => write!(f, "event envelope id must be canonical lowercase hex"),
            Self::NonCanonicalAuthor => {
                write!(f, "event envelope author must be canonical lowercase hex")
            }
            Self::NonCanonicalSignature => {
                write!(
                    f,
                    "event envelope signature must be canonical lowercase hex"
                )
            }
            Self::EmptyTag { index } => write!(f, "event envelope tag {index} is empty"),
            Self::EmptyTagKey { index } => write!(f, "event envelope tag {index} key is empty"),
            Self::ControlCharacterTagKey { index } => {
                write!(
                    f,
                    "event envelope tag {index} key contains a control character"
                )
            }
            Self::ContentTooLarge { max, actual } => {
                write!(
                    f,
                    "event envelope content size {actual} exceeds {max} bytes"
                )
            }
            Self::TooManyTags { max, actual } => {
                write!(f, "event envelope tag count {actual} exceeds {max}")
            }
            Self::TooManyTagElements { max, actual } => {
                write!(f, "event envelope tag element count {actual} exceeds {max}")
            }
            Self::TagElementTooLarge {
                tag_index,
                element_index,
                max,
                actual,
            } => write!(
                f,
                "event envelope tag {tag_index} element {element_index} size {actual} exceeds {max} bytes"
            ),
            Self::TagsTooLarge { max, actual } => {
                write!(f, "event envelope tag bytes {actual} exceed {max}")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsEventEnvelopeError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RadrootsEventEnvelopeLimits {
    pub max_content_bytes: usize,
    pub max_tag_count: usize,
    pub max_total_tag_elements: usize,
    pub max_tag_element_bytes: usize,
    pub max_total_tag_bytes: usize,
}

impl Default for RadrootsEventEnvelopeLimits {
    fn default() -> Self {
        Self {
            max_content_bytes: DEFAULT_CONTENT_MAX_BYTES,
            max_tag_count: DEFAULT_TAG_MAX_COUNT,
            max_total_tag_elements: DEFAULT_TAG_TOTAL_ELEMENT_MAX_COUNT,
            max_tag_element_bytes: DEFAULT_TAG_ELEMENT_MAX_BYTES,
            max_total_tag_bytes: DEFAULT_TAG_TOTAL_MAX_BYTES,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsEventTag(Vec<String>);

impl RadrootsEventTag {
    pub fn new(index: usize, values: Vec<String>) -> Result<Self, RadrootsEventEnvelopeError> {
        Self::new_with_limits(index, values, RadrootsEventEnvelopeLimits::default())
    }

    pub fn new_with_limits(
        index: usize,
        values: Vec<String>,
        limits: RadrootsEventEnvelopeLimits,
    ) -> Result<Self, RadrootsEventEnvelopeError> {
        validate_tag(index, &values)?;
        let total_tag_elements = values.len();
        if total_tag_elements > limits.max_total_tag_elements {
            return Err(RadrootsEventEnvelopeError::TooManyTagElements {
                max: limits.max_total_tag_elements,
                actual: total_tag_elements,
            });
        }
        let total_bytes = validate_tag_elements(index, &values, limits)?;
        if total_bytes > limits.max_total_tag_bytes {
            return Err(RadrootsEventEnvelopeError::TagsTooLarge {
                max: limits.max_total_tag_bytes,
                actual: total_bytes,
            });
        }
        Ok(Self(values))
    }

    #[inline]
    pub fn as_slice(&self) -> &[String] {
        self.0.as_slice()
    }

    #[inline]
    pub fn into_vec(self) -> Vec<String> {
        self.0
    }
}

#[cfg(any(feature = "serde", test))]
impl serde::Serialize for RadrootsEventTag {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

#[cfg(any(feature = "serde", test))]
impl<'de> serde::Deserialize<'de> for RadrootsEventTag {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let values = Vec::<String>::deserialize(deserializer)?;
        Self::new(0, values).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsEventTags(Vec<RadrootsEventTag>);

impl RadrootsEventTags {
    pub fn new(values: Vec<Vec<String>>) -> Result<Self, RadrootsEventEnvelopeError> {
        Self::new_with_limits(values, RadrootsEventEnvelopeLimits::default())
    }

    pub fn new_with_limits(
        values: Vec<Vec<String>>,
        limits: RadrootsEventEnvelopeLimits,
    ) -> Result<Self, RadrootsEventEnvelopeError> {
        let tag_count = values.len();
        if tag_count > limits.max_tag_count {
            return Err(RadrootsEventEnvelopeError::TooManyTags {
                max: limits.max_tag_count,
                actual: tag_count,
            });
        }
        let total_tag_elements = values
            .iter()
            .fold(0usize, |total, tag| total.saturating_add(tag.len()));
        if total_tag_elements > limits.max_total_tag_elements {
            return Err(RadrootsEventEnvelopeError::TooManyTagElements {
                max: limits.max_total_tag_elements,
                actual: total_tag_elements,
            });
        }
        let mut tags = Vec::with_capacity(values.len());
        let mut total_tag_bytes = 0usize;
        for (index, tag) in values.into_iter().enumerate() {
            validate_tag(index, &tag)?;
            total_tag_bytes =
                total_tag_bytes.saturating_add(validate_tag_elements(index, &tag, limits)?);
            if total_tag_bytes > limits.max_total_tag_bytes {
                return Err(RadrootsEventEnvelopeError::TagsTooLarge {
                    max: limits.max_total_tag_bytes,
                    actual: total_tag_bytes,
                });
            }
            tags.push(RadrootsEventTag(tag));
        }
        Ok(Self(tags))
    }

    #[inline]
    pub fn as_slice(&self) -> &[RadrootsEventTag] {
        self.0.as_slice()
    }

    pub fn to_vec(&self) -> Vec<Vec<String>> {
        self.0.iter().map(|tag| tag.as_slice().to_vec()).collect()
    }

    #[inline]
    pub fn into_vec(self) -> Vec<Vec<String>> {
        self.0.into_iter().map(RadrootsEventTag::into_vec).collect()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[cfg(any(feature = "serde", test))]
impl serde::Serialize for RadrootsEventTags {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_vec().serialize(serializer)
    }
}

#[cfg(any(feature = "serde", test))]
impl<'de> serde::Deserialize<'de> for RadrootsEventTags {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let values = Vec::<Vec<String>>::deserialize(deserializer)?;
        Self::new(values).map_err(serde::de::Error::custom)
    }
}

#[cfg(feature = "dto-bindgen")]
impl dto_bindgen::Dto for RadrootsEventTags {
    fn describe(ctx: &mut dto_bindgen::__private::DescribeCtx) -> dto_bindgen::__private::TypeRef {
        <Vec<Vec<String>> as dto_bindgen::Dto>::describe(ctx)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsEventEnvelopeParts {
    pub id: String,
    pub author: String,
    pub created_at: u64,
    pub kind: u32,
    pub tags: Vec<Vec<String>>,
    pub content: String,
    pub sig: String,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "dto-bindgen", dto(ts(name = "RadrootsEventEnvelopeDto")))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsEventEnvelope {
    id: RadrootsEventId,
    author: RadrootsPublicKey,
    #[cfg_attr(feature = "dto-bindgen", dto(int = "json_number"))]
    created_at: RadrootsEventTimestamp,
    kind: RadrootsEventKind,
    tags: RadrootsEventTags,
    content: String,
    sig: RadrootsEventSignature,
}

impl RadrootsEventEnvelope {
    pub fn new(parts: RadrootsEventEnvelopeParts) -> Result<Self, RadrootsEventEnvelopeError> {
        Self::new_with_limits(parts, RadrootsEventEnvelopeLimits::default())
    }

    pub fn new_with_limits(
        parts: RadrootsEventEnvelopeParts,
        limits: RadrootsEventEnvelopeLimits,
    ) -> Result<Self, RadrootsEventEnvelopeError> {
        let id = RadrootsEventId::parse(parts.id.as_str())
            .map_err(RadrootsEventEnvelopeError::InvalidId)?;
        if id.as_str() != parts.id.as_str() {
            return Err(RadrootsEventEnvelopeError::NonCanonicalId);
        }
        let author = RadrootsPublicKey::parse(parts.author.as_str())
            .map_err(RadrootsEventEnvelopeError::InvalidAuthor)?;
        if author.as_str() != parts.author.as_str() {
            return Err(RadrootsEventEnvelopeError::NonCanonicalAuthor);
        }
        let sig = RadrootsEventSignature::parse(parts.sig.as_str())
            .map_err(RadrootsEventEnvelopeError::InvalidSignature)?;
        if sig.as_str() != parts.sig.as_str() {
            return Err(RadrootsEventEnvelopeError::NonCanonicalSignature);
        }
        let content_len = parts.content.len();
        if content_len > limits.max_content_bytes {
            return Err(RadrootsEventEnvelopeError::ContentTooLarge {
                max: limits.max_content_bytes,
                actual: content_len,
            });
        }
        let tags = RadrootsEventTags::new_with_limits(parts.tags, limits)?;
        Ok(Self {
            id,
            author,
            created_at: RadrootsEventTimestamp::new(parts.created_at),
            kind: RadrootsEventKind::new(parts.kind),
            tags,
            content: parts.content,
            sig,
        })
    }

    #[inline]
    pub fn id(&self) -> &RadrootsEventId {
        &self.id
    }

    #[inline]
    pub fn id_str(&self) -> &str {
        self.id.as_str()
    }

    #[inline]
    pub fn author(&self) -> &RadrootsPublicKey {
        &self.author
    }

    #[inline]
    pub fn author_str(&self) -> &str {
        self.author.as_str()
    }

    #[inline]
    pub fn created_at(&self) -> RadrootsEventTimestamp {
        self.created_at
    }

    #[inline]
    pub fn created_at_u64(&self) -> u64 {
        self.created_at.as_u64()
    }

    #[inline]
    pub fn kind(&self) -> RadrootsEventKind {
        self.kind
    }

    #[inline]
    pub fn kind_u32(&self) -> u32 {
        self.kind.as_u32()
    }

    #[inline]
    pub fn kind_class(&self) -> RadrootsEventKindClass {
        self.kind.class()
    }

    #[inline]
    pub fn tags(&self) -> &RadrootsEventTags {
        &self.tags
    }

    #[inline]
    pub fn tag_slices(&self) -> &[RadrootsEventTag] {
        self.tags.as_slice()
    }

    pub fn tags_as_vec(&self) -> Vec<Vec<String>> {
        self.tags.to_vec()
    }

    #[inline]
    pub fn content(&self) -> &str {
        self.content.as_str()
    }

    #[inline]
    pub fn sig(&self) -> &RadrootsEventSignature {
        &self.sig
    }

    #[inline]
    pub fn sig_str(&self) -> &str {
        self.sig.as_str()
    }

    pub fn to_nip01_wire(&self) -> RadrootsNip01EventWire {
        RadrootsNip01EventWire {
            id: self.id.as_str().to_string(),
            pubkey: self.author.as_str().to_string(),
            created_at: self.created_at.as_u64(),
            kind: self.kind.as_u32(),
            tags: self.tags.to_vec(),
            content: self.content.clone(),
            sig: self.sig.as_str().to_string(),
            extra: Default::default(),
        }
    }
}

fn validate_tag(index: usize, values: &[String]) -> Result<(), RadrootsEventEnvelopeError> {
    let Some(key) = values.first() else {
        return Err(RadrootsEventEnvelopeError::EmptyTag { index });
    };
    if key.is_empty() {
        return Err(RadrootsEventEnvelopeError::EmptyTagKey { index });
    }
    if key.chars().any(char::is_control) {
        return Err(RadrootsEventEnvelopeError::ControlCharacterTagKey { index });
    }
    Ok(())
}

fn validate_tag_elements(
    tag_index: usize,
    values: &[String],
    limits: RadrootsEventEnvelopeLimits,
) -> Result<usize, RadrootsEventEnvelopeError> {
    let mut total_tag_bytes = 0usize;
    for (element_index, value) in values.iter().enumerate() {
        let value_len = value.len();
        if value_len > limits.max_tag_element_bytes {
            return Err(RadrootsEventEnvelopeError::TagElementTooLarge {
                tag_index,
                element_index,
                max: limits.max_tag_element_bytes,
                actual: value_len,
            });
        }
        total_tag_bytes = total_tag_bytes.saturating_add(value_len);
    }
    Ok(total_tag_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex_64(character: char) -> String {
        core::iter::repeat_n(character, 64).collect()
    }

    fn hex_128(character: char) -> String {
        core::iter::repeat_n(character, 128).collect()
    }

    fn event_parts() -> RadrootsEventEnvelopeParts {
        RadrootsEventEnvelopeParts {
            id: hex_64('1'),
            author: hex_64('a'),
            created_at: u64::from(u32::MAX) + 1,
            kind: 30_023,
            tags: vec![vec!["d".to_owned(), "article".to_owned()]],
            content: "hello".to_owned(),
            sig: hex_128('b'),
        }
    }

    #[test]
    fn envelope_uses_private_typed_state_and_getters() {
        let envelope = RadrootsEventEnvelope::new(event_parts()).expect("envelope");

        assert_eq!(envelope.id_str(), hex_64('1'));
        assert_eq!(envelope.author_str(), hex_64('a'));
        assert_eq!(envelope.created_at_u64(), u64::from(u32::MAX) + 1);
        assert_eq!(envelope.kind_u32(), 30_023);
        assert_eq!(envelope.kind_class(), RadrootsEventKindClass::Addressable);
        assert_eq!(envelope.content(), "hello");
        assert_eq!(envelope.sig_str(), hex_128('b'));
        assert_eq!(
            envelope.tags_as_vec(),
            vec![vec!["d".to_owned(), "article".to_owned()]]
        );
    }

    #[test]
    fn envelope_rejects_invalid_parts() {
        let mut parts = event_parts();
        parts.id = "bad".to_owned();
        assert!(matches!(
            RadrootsEventEnvelope::new(parts),
            Err(RadrootsEventEnvelopeError::InvalidId(_))
        ));

        let mut parts = event_parts();
        parts.tags = vec![Vec::new()];
        assert_eq!(
            RadrootsEventEnvelope::new(parts),
            Err(RadrootsEventEnvelopeError::EmptyTag { index: 0 })
        );
    }

    #[test]
    fn kind_classifies_nip01_ranges() {
        assert_eq!(
            RadrootsEventKind::new(0).class(),
            RadrootsEventKindClass::Replaceable
        );
        assert_eq!(
            RadrootsEventKind::new(1).class(),
            RadrootsEventKindClass::Regular
        );
        assert_eq!(
            RadrootsEventKind::new(3).class(),
            RadrootsEventKindClass::Replaceable
        );
        assert_eq!(
            RadrootsEventKind::new(9_999).class(),
            RadrootsEventKindClass::Regular
        );
        assert_eq!(
            RadrootsEventKind::new(10_000).class(),
            RadrootsEventKindClass::Replaceable
        );
        assert_eq!(
            RadrootsEventKind::new(19_999).class(),
            RadrootsEventKindClass::Replaceable
        );
        assert_eq!(
            RadrootsEventKind::new(20_000).class(),
            RadrootsEventKindClass::Ephemeral
        );
        assert_eq!(
            RadrootsEventKind::new(29_999).class(),
            RadrootsEventKindClass::Ephemeral
        );
        assert_eq!(
            RadrootsEventKind::new(30_000).class(),
            RadrootsEventKindClass::Addressable
        );
        assert_eq!(
            RadrootsEventKind::new(39_999).class(),
            RadrootsEventKindClass::Addressable
        );
        assert_eq!(
            RadrootsEventKind::new(40_000).class(),
            RadrootsEventKindClass::Regular
        );
    }

    #[test]
    fn envelope_rejects_domain_budget_violations() {
        let mut parts = event_parts();
        assert_eq!(
            RadrootsEventEnvelope::new_with_limits(
                parts.clone(),
                RadrootsEventEnvelopeLimits {
                    max_content_bytes: 4,
                    ..RadrootsEventEnvelopeLimits::default()
                }
            ),
            Err(RadrootsEventEnvelopeError::ContentTooLarge { max: 4, actual: 5 })
        );

        parts.tags = vec![vec!["d".to_owned()]];
        assert_eq!(
            RadrootsEventEnvelope::new_with_limits(
                parts.clone(),
                RadrootsEventEnvelopeLimits {
                    max_tag_count: 0,
                    ..RadrootsEventEnvelopeLimits::default()
                }
            ),
            Err(RadrootsEventEnvelopeError::TooManyTags { max: 0, actual: 1 })
        );

        parts.tags = vec![vec!["d".to_owned(), "abcd".to_owned()]];
        assert_eq!(
            RadrootsEventEnvelope::new_with_limits(
                parts.clone(),
                RadrootsEventEnvelopeLimits {
                    max_total_tag_elements: 1,
                    ..RadrootsEventEnvelopeLimits::default()
                }
            ),
            Err(RadrootsEventEnvelopeError::TooManyTagElements { max: 1, actual: 2 })
        );

        assert_eq!(
            RadrootsEventEnvelope::new_with_limits(
                parts.clone(),
                RadrootsEventEnvelopeLimits {
                    max_tag_element_bytes: 3,
                    ..RadrootsEventEnvelopeLimits::default()
                }
            ),
            Err(RadrootsEventEnvelopeError::TagElementTooLarge {
                tag_index: 0,
                element_index: 1,
                max: 3,
                actual: 4
            })
        );

        parts.tags = vec![vec!["d".to_owned(), "soil".to_owned()]];
        assert_eq!(
            RadrootsEventEnvelope::new_with_limits(
                parts,
                RadrootsEventEnvelopeLimits {
                    max_total_tag_bytes: 4,
                    ..RadrootsEventEnvelopeLimits::default()
                }
            ),
            Err(RadrootsEventEnvelopeError::TagsTooLarge { max: 4, actual: 5 })
        );
    }

    #[test]
    fn envelope_accepts_exact_domain_budget_boundaries() {
        let mut parts = event_parts();
        parts.content = "hello".to_owned();
        parts.tags = vec![vec!["d".to_owned(), "soil".to_owned()]];

        let envelope = RadrootsEventEnvelope::new_with_limits(
            parts,
            RadrootsEventEnvelopeLimits {
                max_content_bytes: 5,
                max_tag_count: 1,
                max_total_tag_elements: 2,
                max_tag_element_bytes: 4,
                max_total_tag_bytes: 5,
            },
        )
        .expect("envelope");

        assert_eq!(envelope.content(), "hello");
        assert_eq!(
            envelope.tags_as_vec(),
            vec![vec!["d".to_owned(), "soil".to_owned()]]
        );
    }

    #[test]
    fn envelope_serializes_to_domain_shape() {
        let envelope = RadrootsEventEnvelope::new(event_parts()).expect("envelope");
        let encoded = serde_json::to_value(&envelope).expect("json");

        assert_eq!(
            encoded.get("id").and_then(serde_json::Value::as_str),
            Some(hex_64('1').as_str())
        );
        assert_eq!(
            encoded.get("author").and_then(serde_json::Value::as_str),
            Some(hex_64('a').as_str())
        );
        assert_eq!(
            encoded
                .get("created_at")
                .and_then(serde_json::Value::as_u64),
            Some(u64::from(u32::MAX) + 1)
        );
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn typed_envelope_api_and_error_contracts_are_complete() {
        #[allow(dead_code)]
        #[derive(Debug, serde::Deserialize)]
        struct MissingTimestamp {
            value: RadrootsEventTimestamp,
        }
        #[allow(dead_code)]
        #[derive(Debug, serde::Deserialize)]
        struct MissingKind {
            value: RadrootsEventKind,
        }
        #[allow(dead_code)]
        #[derive(Debug, serde::Deserialize)]
        struct MissingTags {
            value: RadrootsEventTags,
        }

        for message in [
            serde_json::from_str::<MissingTimestamp>("{}")
                .expect_err("missing timestamp")
                .to_string(),
            serde_json::from_str::<MissingKind>("{}")
                .expect_err("missing kind")
                .to_string(),
            serde_json::from_str::<MissingTags>("{}")
                .expect_err("missing tags")
                .to_string(),
        ] {
            assert!(message.contains("missing field `value`"));
        }

        let timestamp = RadrootsEventTimestamp::from(42);
        assert_eq!(timestamp.as_u64(), 42);
        assert_eq!(
            serde_json::from_str::<RadrootsEventTimestamp>(
                &serde_json::to_string(&timestamp).expect("timestamp json"),
            )
            .expect("timestamp"),
            timestamp
        );
        let kind = RadrootsEventKind::from(30_023);
        assert_eq!(kind.as_u32(), 30_023);
        assert_eq!(
            serde_json::from_str::<RadrootsEventKind>(
                &serde_json::to_string(&kind).expect("kind json"),
            )
            .expect("kind"),
            kind
        );

        let parse_error = RadrootsEventId::parse("bad").expect_err("invalid id");
        for error in [
            RadrootsEventEnvelopeError::InvalidId(parse_error.clone()),
            RadrootsEventEnvelopeError::InvalidAuthor(parse_error.clone()),
            RadrootsEventEnvelopeError::InvalidSignature(parse_error),
            RadrootsEventEnvelopeError::NonCanonicalId,
            RadrootsEventEnvelopeError::NonCanonicalAuthor,
            RadrootsEventEnvelopeError::NonCanonicalSignature,
            RadrootsEventEnvelopeError::EmptyTag { index: 1 },
            RadrootsEventEnvelopeError::EmptyTagKey { index: 1 },
            RadrootsEventEnvelopeError::ControlCharacterTagKey { index: 1 },
            RadrootsEventEnvelopeError::ContentTooLarge { max: 1, actual: 2 },
            RadrootsEventEnvelopeError::TooManyTags { max: 1, actual: 2 },
            RadrootsEventEnvelopeError::TooManyTagElements { max: 1, actual: 2 },
            RadrootsEventEnvelopeError::TagElementTooLarge {
                tag_index: 1,
                element_index: 2,
                max: 3,
                actual: 4,
            },
            RadrootsEventEnvelopeError::TagsTooLarge { max: 1, actual: 2 },
        ] {
            assert!(!error.to_string().is_empty());
        }

        let tag = RadrootsEventTag::new(0, vec!["t".to_owned(), "soil".to_owned()]).expect("tag");
        assert_eq!(tag.clone().into_vec(), vec!["t", "soil"]);
        let tag_json = serde_json::to_string(&tag).expect("tag json");
        assert_eq!(
            serde_json::from_str::<RadrootsEventTag>(&tag_json).expect("tag"),
            tag
        );
        assert!(serde_json::from_str::<RadrootsEventTag>("[]").is_err());
        assert!(
            RadrootsEventTag::new_with_limits(
                0,
                vec!["tag".to_owned()],
                RadrootsEventEnvelopeLimits {
                    max_total_tag_bytes: 2,
                    ..RadrootsEventEnvelopeLimits::default()
                },
            )
            .is_err()
        );

        let empty_tags = RadrootsEventTags::new(Vec::new()).expect("empty tags");
        assert_eq!(empty_tags.len(), 0);
        assert!(empty_tags.is_empty());
        assert!(empty_tags.clone().into_vec().is_empty());
        let tags =
            RadrootsEventTags::new(vec![vec!["t".to_owned(), "soil".to_owned()]]).expect("tags");
        let tags_json = serde_json::to_string(&tags).expect("tags json");
        assert_eq!(
            serde_json::from_str::<RadrootsEventTags>(&tags_json).expect("tags"),
            tags
        );
        assert!(serde_json::from_str::<RadrootsEventTags>("[[]]").is_err());

        let envelope = RadrootsEventEnvelope::new(event_parts()).expect("envelope");
        assert_eq!(envelope.id().as_str(), envelope.id_str());
        assert_eq!(envelope.author().as_str(), envelope.author_str());
        assert_eq!(envelope.created_at().as_u64(), envelope.created_at_u64());
        assert_eq!(envelope.kind().as_u32(), envelope.kind_u32());
        assert_eq!(envelope.tags().to_vec(), envelope.tags_as_vec());
        assert_eq!(envelope.tag_slices(), envelope.tags().as_slice());
        assert_eq!(envelope.sig().as_str(), envelope.sig_str());
        let wire = envelope.to_nip01_wire();
        assert_eq!(wire.id, envelope.id_str());
        let encoded = serde_json::to_string(&envelope).expect("envelope json");
        assert_eq!(
            serde_json::from_str::<RadrootsEventEnvelope>(&encoded).expect("envelope"),
            envelope
        );

        for (field, value) in [
            ("id", hex_64('A')),
            ("author", hex_64('A')),
            ("sig", hex_128('B')),
        ] {
            let mut parts = event_parts();
            match field {
                "id" => parts.id = value,
                "author" => parts.author = value,
                "sig" => parts.sig = value,
                _ => unreachable!("fixture field"),
            }
            assert!(RadrootsEventEnvelope::new(parts).is_err());
        }
        for tags in [
            vec![vec![String::new()]],
            vec![vec!["line\nbreak".to_owned()]],
        ] {
            let mut parts = event_parts();
            parts.tags = tags;
            assert!(RadrootsEventEnvelope::new(parts).is_err());
        }
    }
}

#![forbid(unsafe_code)]

#[cfg(all(not(feature = "std"), not(test)))]
use alloc::{string::String, string::ToString, vec::Vec};

#[cfg(any(feature = "std", test))]
use std::{string::String, vec::Vec};

use crate::ids::{
    RadrootsEventId, RadrootsEventSignature, RadrootsIdParseError, RadrootsPublicKey,
};
use crate::wire::RadrootsNip01EventWire;
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
    EmptyTag { index: usize },
    EmptyTagKey { index: usize },
    ControlCharacterTagKey { index: usize },
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
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsEventEnvelopeError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsEventTag(Vec<String>);

impl RadrootsEventTag {
    pub fn new(index: usize, values: Vec<String>) -> Result<Self, RadrootsEventEnvelopeError> {
        validate_tag(index, &values)?;
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
        let mut tags = Vec::with_capacity(values.len());
        for (index, tag) in values.into_iter().enumerate() {
            tags.push(RadrootsEventTag::new(index, tag)?);
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
        let tags = RadrootsEventTags::new(parts.tags)?;
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
            RadrootsEventKind::new(1).class(),
            RadrootsEventKindClass::Regular
        );
        assert_eq!(
            RadrootsEventKind::new(10_000).class(),
            RadrootsEventKindClass::Replaceable
        );
        assert_eq!(
            RadrootsEventKind::new(20_000).class(),
            RadrootsEventKindClass::Ephemeral
        );
        assert_eq!(
            RadrootsEventKind::new(30_000).class(),
            RadrootsEventKindClass::Addressable
        );
        assert_eq!(
            RadrootsEventKind::new(40_000).class(),
            RadrootsEventKindClass::Regular
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
}

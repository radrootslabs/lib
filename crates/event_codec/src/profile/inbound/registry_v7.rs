//! Frozen profile inbound semantics for event-contract registry v7.

#[cfg(not(feature = "std"))]
use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
};
#[cfg(feature = "std")]
use std::{collections::BTreeMap, string::String};

use core::fmt;

use radroots_event::profile::{
    RADROOTS_PROFILE_METADATA_MAX_CONTENT_BYTES, RadrootsNip05Identifier,
};
use serde::de::{IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::Value;

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsProfileMetadataParseError {
    ContentTooLarge { max: usize, actual: usize },
    InvalidJson,
    RootNotObject,
    DuplicateField(String),
}

impl RadrootsProfileMetadataParseError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::ContentTooLarge { .. } => "content_too_large",
            Self::InvalidJson => "invalid_json",
            Self::RootNotObject => "root_not_object",
            Self::DuplicateField(_) => "duplicate_field",
        }
    }
}

impl fmt::Display for RadrootsProfileMetadataParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContentTooLarge { max, actual } => {
                write!(f, "Profile metadata is {actual} bytes; max is {max}")
            }
            Self::InvalidJson => f.write_str("Profile metadata is invalid JSON"),
            Self::RootNotObject => f.write_str("Profile metadata root must be a JSON object"),
            Self::DuplicateField(field) => {
                write!(f, "Profile metadata contains duplicate field {field:?}")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsProfileMetadataParseError {}

/// A string media reference observed in inbound Profile metadata.
///
/// Even a structurally valid Blossom URL remains unverified in this state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsUnverifiedProfileMediaReference(String);

impl RadrootsUnverifiedProfileMediaReference {
    fn from_inbound(value: String) -> Self {
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The content parser never performs NIP-05 network identity resolution.
///
/// A future resolved identity must use a separate verified result type.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsNip05IdentityVerification {
    NotPerformed,
}

impl RadrootsNip05IdentityVerification {
    pub const fn code(self) -> &'static str {
        match self {
            Self::NotPerformed => "not_performed",
        }
    }
}

impl fmt::Display for RadrootsUnverifiedProfileMediaReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Tolerant inbound Profile metadata with lossless raw and residual views.
///
/// Correctly typed known fields are projected below. Unknown fields, known
/// fields with the wrong JSON type, and syntactically invalid NIP-05 strings
/// remain in `residual_fields` and in the complete raw object. This type does
/// not attest event kind, identifier, signature, or author.
#[derive(Clone, Debug, PartialEq)]
pub struct RadrootsInboundProfileMetadata {
    raw_content: String,
    raw_fields: BTreeMap<String, Value>,
    residual_fields: BTreeMap<String, Value>,
    name: Option<String>,
    display_name: Option<String>,
    about: Option<String>,
    picture: Option<RadrootsUnverifiedProfileMediaReference>,
    banner: Option<RadrootsUnverifiedProfileMediaReference>,
    nip05: Option<RadrootsNip05Identifier>,
    bot: Option<bool>,
}

impl RadrootsInboundProfileMetadata {
    pub fn raw_content(&self) -> &str {
        &self.raw_content
    }

    pub fn raw_fields(&self) -> &BTreeMap<String, Value> {
        &self.raw_fields
    }

    pub fn residual_fields(&self) -> &BTreeMap<String, Value> {
        &self.residual_fields
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }

    pub fn about(&self) -> Option<&str> {
        self.about.as_deref()
    }

    pub fn picture(&self) -> Option<&RadrootsUnverifiedProfileMediaReference> {
        self.picture.as_ref()
    }

    pub fn banner(&self) -> Option<&RadrootsUnverifiedProfileMediaReference> {
        self.banner.as_ref()
    }

    /// Returns only a syntax-checked identifier; no NIP-05 resolution occurred.
    pub fn nip05(&self) -> Option<&RadrootsNip05Identifier> {
        self.nip05.as_ref()
    }

    pub const fn nip05_identity_verification(&self) -> RadrootsNip05IdentityVerification {
        RadrootsNip05IdentityVerification::NotPerformed
    }

    pub const fn bot(&self) -> Option<bool> {
        self.bot
    }
}

/// Parses bounded, untrusted kind-0 metadata content after event verification.
///
/// Successful parsing is not event admission. The caller must first require an
/// exact kind-0 event with a verified identifier and signature. Content larger
/// than [`RADROOTS_PROFILE_METADATA_MAX_CONTENT_BYTES`] is rejected before JSON
/// parsing.
pub fn parse_inbound_profile_metadata(
    content: &str,
) -> Result<RadrootsInboundProfileMetadata, RadrootsProfileMetadataParseError> {
    parse_inbound_profile_metadata_registry_v7(content)
}

/// Parses Profile metadata with the behavior frozen for contract registry v7.
pub fn parse_inbound_profile_metadata_registry_v7(
    content: &str,
) -> Result<RadrootsInboundProfileMetadata, RadrootsProfileMetadataParseError> {
    if content.len() > RADROOTS_PROFILE_METADATA_MAX_CONTENT_BYTES {
        return Err(RadrootsProfileMetadataParseError::ContentTooLarge {
            max: RADROOTS_PROFILE_METADATA_MAX_CONTENT_BYTES,
            actual: content.len(),
        });
    }

    let root: ProfileMetadataRoot = serde_json::from_str(content)
        .map_err(|_| RadrootsProfileMetadataParseError::InvalidJson)?;
    let ProfileMetadataRoot::Object(unique) = root else {
        return Err(RadrootsProfileMetadataParseError::RootNotObject);
    };
    if let Some(field) = unique.duplicate {
        return Err(RadrootsProfileMetadataParseError::DuplicateField(field));
    }

    let raw_fields = unique.fields;
    let mut residual_fields = raw_fields.clone();
    let name = project_string(&raw_fields, &mut residual_fields, "name");
    let display_name = project_string(&raw_fields, &mut residual_fields, "display_name");
    let about = project_string(&raw_fields, &mut residual_fields, "about");
    let picture = project_string(&raw_fields, &mut residual_fields, "picture")
        .map(RadrootsUnverifiedProfileMediaReference::from_inbound);
    let banner = project_string(&raw_fields, &mut residual_fields, "banner")
        .map(RadrootsUnverifiedProfileMediaReference::from_inbound);
    let nip05 = project_nip05(&raw_fields, &mut residual_fields);
    let bot = project_bool(&raw_fields, &mut residual_fields, "bot");

    Ok(RadrootsInboundProfileMetadata {
        raw_content: content.to_string(),
        raw_fields,
        residual_fields,
        name,
        display_name,
        about,
        picture,
        banner,
        nip05,
        bot,
    })
}

fn project_string(
    raw_fields: &BTreeMap<String, Value>,
    residual_fields: &mut BTreeMap<String, Value>,
    key: &'static str,
) -> Option<String> {
    let value = raw_fields.get(key)?.as_str()?.to_string();
    residual_fields.remove(key);
    Some(value)
}

fn project_bool(
    raw_fields: &BTreeMap<String, Value>,
    residual_fields: &mut BTreeMap<String, Value>,
    key: &'static str,
) -> Option<bool> {
    let value = raw_fields.get(key)?.as_bool()?;
    residual_fields.remove(key);
    Some(value)
}

fn project_nip05(
    raw_fields: &BTreeMap<String, Value>,
    residual_fields: &mut BTreeMap<String, Value>,
) -> Option<RadrootsNip05Identifier> {
    let value = raw_fields.get("nip05")?.as_str()?;
    let identifier = RadrootsNip05Identifier::parse(value).ok()?;
    residual_fields.remove("nip05");
    Some(identifier)
}

struct UniqueMetadataObject {
    fields: BTreeMap<String, Value>,
    duplicate: Option<String>,
}

enum ProfileMetadataRoot {
    Object(UniqueMetadataObject),
    NonObject,
}

impl<'de> Deserialize<'de> for ProfileMetadataRoot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(ProfileMetadataRootVisitor)
    }
}

struct ProfileMetadataRootVisitor;

impl<'de> Visitor<'de> for ProfileMetadataRootVisitor {
    type Value = ProfileMetadataRoot;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Profile metadata JSON")
    }

    fn visit_bool<E>(self, _value: bool) -> Result<Self::Value, E> {
        Ok(ProfileMetadataRoot::NonObject)
    }

    fn visit_i64<E>(self, _value: i64) -> Result<Self::Value, E> {
        Ok(ProfileMetadataRoot::NonObject)
    }

    fn visit_u64<E>(self, _value: u64) -> Result<Self::Value, E> {
        Ok(ProfileMetadataRoot::NonObject)
    }

    fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E> {
        Ok(ProfileMetadataRoot::NonObject)
    }

    fn visit_str<E>(self, _value: &str) -> Result<Self::Value, E> {
        Ok(ProfileMetadataRoot::NonObject)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(ProfileMetadataRoot::NonObject)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        loop {
            if sequence.next_element::<IgnoredAny>()?.is_none() {
                break;
            }
        }
        Ok(ProfileMetadataRoot::NonObject)
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut fields = BTreeMap::new();
        let mut duplicate = None;
        while let Some((key, value)) = map.next_entry::<String, Value>()? {
            if fields.insert(key.clone(), value).is_some() && duplicate.is_none() {
                duplicate = Some(key);
            }
        }
        Ok(ProfileMetadataRoot::Object(UniqueMetadataObject {
            fields,
            duplicate,
        }))
    }
}

#[cfg(test)]
mod tests;

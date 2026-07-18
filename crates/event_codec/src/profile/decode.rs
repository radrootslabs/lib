#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use super::RadrootsProfileData;
use radroots_event::{
    kinds::KIND_PROFILE,
    profile::{
        RADROOTS_PROFILE_TYPE_TAG_KEY, RadrootsProfile, RadrootsProfileType,
        radroots_profile_type_from_tag_value,
    },
};

use crate::error::EventParseError;
use crate::parsed::{RadrootsParsedData, RadrootsParsedEvent};
use serde_json::Value;

const PROFILE_KIND: u32 = KIND_PROFILE;

fn parse_optional_string(value: &Value, key: &'static str) -> Option<String> {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn parse_bot(value: &Value) -> Option<String> {
    match value.get("bot") {
        Some(v) if v.is_string() => v.as_str().map(|s| s.to_string()),
        Some(v) if v.is_boolean() => v.as_bool().map(|b| b.to_string()),
        _ => None,
    }
}

fn profile_type_from_tags(tags: &[Vec<String>]) -> Option<RadrootsProfileType> {
    tags.iter()
        .filter(|tag| tag.first().map(|v| v.as_str()) == Some(RADROOTS_PROFILE_TYPE_TAG_KEY))
        .filter_map(|tag| tag.get(1))
        .find_map(|value| radroots_profile_type_from_tag_value(value))
}

/// Decodes content into the compatibility-only legacy Profile model.
///
/// This API requires `name`, coerces Boolean `bot` to a string, and discards
/// unprojected fields. Use `profile.parse_inbound_metadata` for the tolerant
/// inbound metadata contract.
pub fn profile_from_content(content: &str) -> Result<RadrootsProfile, EventParseError> {
    let value: Value =
        serde_json::from_str(content).map_err(|_| EventParseError::InvalidJson("content"))?;
    let obj = value
        .as_object()
        .ok_or(EventParseError::InvalidJson("content"))?;
    let name = obj
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or(EventParseError::InvalidJson("name"))?;

    Ok(RadrootsProfile {
        name: name.to_string(),
        display_name: parse_optional_string(&value, "display_name"),
        nip05: parse_optional_string(&value, "nip05"),
        about: parse_optional_string(&value, "about"),
        website: parse_optional_string(&value, "website"),
        picture: parse_optional_string(&value, "picture"),
        banner: parse_optional_string(&value, "banner"),
        lud06: parse_optional_string(&value, "lud06"),
        lud16: parse_optional_string(&value, "lud16"),
        bot: parse_bot(&value),
    })
}

/// Projects caller-supplied event fields through the legacy Profile decoder.
///
/// This compatibility API does not verify the event identifier or signature
/// and is not the strict inbound event-admission boundary.
pub fn data_from_event(
    id: String,
    author: String,
    published_at: u64,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
) -> Result<RadrootsParsedData<RadrootsProfileData>, EventParseError> {
    if kind != PROFILE_KIND {
        return Err(EventParseError::InvalidKind {
            expected: "0",
            got: kind,
        });
    }
    let profile = profile_from_content(&content)?;
    let profile_type = profile_type_from_tags(&tags);
    Ok(RadrootsParsedData::new(
        id,
        author,
        published_at,
        kind,
        RadrootsProfileData {
            profile_type,
            profile,
        },
    ))
}

/// Builds a legacy parsed Profile wrapper from caller-supplied event fields.
///
/// This compatibility API is outside `profile.parse_inbound_metadata` and does
/// not establish strict event admission.
pub fn parsed_from_event(
    id: String,
    author: String,
    published_at: u64,
    kind: u32,
    content: String,
    tags: Vec<Vec<String>>,
    sig: String,
) -> Result<RadrootsParsedEvent<RadrootsProfileData>, EventParseError> {
    let data = data_from_event(
        id.clone(),
        author.clone(),
        published_at,
        kind,
        content.clone(),
        tags.clone(),
    )?;
    RadrootsParsedEvent::from_event_parts(id, author, published_at, kind, content, tags, sig, data)
}

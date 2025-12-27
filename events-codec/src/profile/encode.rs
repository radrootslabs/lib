use crate::profile::error::ProfileEncodeError;
use radroots_events::profile::{
    RadrootsProfileType,
    RadrootsProfile,
    RADROOTS_PROFILE_TYPE_TAG_KEY,
    radroots_profile_type_tag_value,
};
use radroots_events::kinds::KIND_PROFILE;

use nostr::Metadata;
use nostr::prelude::Url;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg(feature = "serde_json")]
use crate::wire::WireEventParts;

fn push_tag(tags: &mut Vec<Vec<String>>, key: &str, value: &str) {
    let mut tag = Vec::with_capacity(2);
    tag.push(key.to_string());
    tag.push(value.to_string());
    tags.push(tag);
}

pub fn profile_type_tags(profile_type: RadrootsProfileType) -> Vec<Vec<String>> {
    let mut tags = Vec::with_capacity(1);
    push_tag(
        &mut tags,
        RADROOTS_PROFILE_TYPE_TAG_KEY,
        radroots_profile_type_tag_value(profile_type),
    );
    tags
}

pub fn profile_build_tags(profile_type: Option<RadrootsProfileType>) -> Vec<Vec<String>> {
    match profile_type {
        Some(value) => profile_type_tags(value),
        None => Vec::new(),
    }
}

pub fn to_metadata(p: &RadrootsProfile) -> Result<Metadata, ProfileEncodeError> {
    let mut md = Metadata::new().name(p.name.clone());

    if let Some(s) = &p.display_name {
        md = md.display_name(s.clone());
    }
    if let Some(s) = &p.about {
        md = md.about(s.clone());
    }
    if let Some(s) = &p.website {
        let u = Url::parse(s).map_err(|_| ProfileEncodeError::InvalidUrl("website", s.clone()))?;
        md = md.website(u);
    }
    if let Some(s) = &p.picture {
        let u = Url::parse(s).map_err(|_| ProfileEncodeError::InvalidUrl("picture", s.clone()))?;
        md = md.picture(u);
    }
    if let Some(s) = &p.banner {
        let u = Url::parse(s).map_err(|_| ProfileEncodeError::InvalidUrl("banner", s.clone()))?;
        md = md.banner(u);
    }
    if let Some(s) = &p.nip05 {
        md = md.nip05(s.clone());
    }
    if let Some(s) = &p.lud06 {
        md = md.lud06(s.clone());
    }
    if let Some(s) = &p.lud16 {
        md = md.lud16(s.clone());
    }

    Ok(md)
}

#[cfg(feature = "serde_json")]
pub fn to_wire_parts(p: &RadrootsProfile) -> Result<WireEventParts, ProfileEncodeError> {
    to_wire_parts_with_profile_type(p, None)
}

#[cfg(feature = "serde_json")]
pub fn to_wire_parts_with_profile_type(
    p: &RadrootsProfile,
    profile_type: Option<RadrootsProfileType>,
) -> Result<WireEventParts, ProfileEncodeError> {
    let md = to_metadata(p)?;
    let content = serde_json::to_string(&md).map_err(|_| ProfileEncodeError::Json)?;
    let tags = profile_build_tags(profile_type);
    Ok(WireEventParts {
        kind: KIND_PROFILE,
        content,
        tags,
    })
}

use crate::profile::error::ProfileEncodeError;
#[cfg(feature = "serde_json")]
use radroots_event::kinds::KIND_PROFILE;
use radroots_event::profile::{
    RADROOTS_PROFILE_TYPE_TAG_KEY, RadrootsProfile, RadrootsProfileType,
    radroots_profile_type_tag_value,
};

use nostr::Metadata;
use nostr::prelude::Url;

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

#[cfg(feature = "serde_json")]
use radroots_event::wire::RadrootsNip01EventWireParts;

fn push_tag(tags: &mut Vec<Vec<String>>, key: &str, value: &str) {
    tags.push(vec![key.to_string(), value.to_string()]);
}

/// Builds the legacy Radroots Profile marker tag.
///
/// This compatibility helper is not part of the strict authored Profile
/// operation.
pub fn profile_type_tags(profile_type: RadrootsProfileType) -> Vec<Vec<String>> {
    let mut tags = Vec::with_capacity(1);
    push_tag(
        &mut tags,
        RADROOTS_PROFILE_TYPE_TAG_KEY,
        radroots_profile_type_tag_value(profile_type),
    );
    tags
}

/// Builds optional legacy Radroots Profile marker tags.
///
/// This compatibility helper is not part of the strict authored Profile
/// operation.
pub fn profile_build_tags(profile_type: Option<RadrootsProfileType>) -> Vec<Vec<String>> {
    match profile_type {
        Some(value) => profile_type_tags(value),
        None => Vec::new(),
    }
}

/// Converts the legacy Profile model to generic Nostr metadata.
///
/// This compatibility API accepts arbitrary media strings and does not satisfy
/// the strict authored Profile contract.
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
/// Encodes the legacy Profile model without a marker tag.
///
/// This compatibility API does not satisfy the strict authored Profile media
/// contract. New authored callers must use `profile.build_authored_draft`.
pub fn to_wire_parts(
    p: &RadrootsProfile,
) -> Result<RadrootsNip01EventWireParts, ProfileEncodeError> {
    to_wire_parts_with_profile_type(p, None)
}

#[cfg(feature = "serde_json")]
/// Encodes the legacy Profile model with an optional marker tag.
///
/// This compatibility API does not satisfy the strict authored Profile media
/// contract. New authored callers must use `profile.build_authored_draft`.
pub fn to_wire_parts_with_profile_type(
    p: &RadrootsProfile,
    profile_type: Option<RadrootsProfileType>,
) -> Result<RadrootsNip01EventWireParts, ProfileEncodeError> {
    let md = to_metadata(p)?;
    let content = serde_json::to_string(&md).map_err(|_| ProfileEncodeError::Json)?;
    let tags = profile_build_tags(profile_type);
    Ok(RadrootsNip01EventWireParts {
        kind: KIND_PROFILE,
        content,
        tags,
    })
}

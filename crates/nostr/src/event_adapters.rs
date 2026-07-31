//! Deterministic adapters for typed Radroots event payloads and Nostr tags.
//!
//! These helpers transform already-supplied values only; they perform no
//! relay I/O, persistence, account selection, or runtime initialization.

#[cfg(feature = "events")]
use radroots_event::profile::{
    RADROOTS_PROFILE_TYPE_TAG_KEY, radroots_profile_type_from_tag_value,
};
#[cfg(feature = "events")]
use radroots_event_codec::decode::RadrootsParsedData;
#[cfg(feature = "events")]
use radroots_event_codec::decode::{
    post::LegacyPost,
    profile::{LegacyProfile, RadrootsProfileData},
};

#[cfg(feature = "events")]
use crate::types::{RadrootsNostrEvent, RadrootsNostrMetadata};

#[cfg(feature = "events")]
/// Adapts an event through the compatibility-only legacy post projection.
///
/// This helper discards tags and does not establish product profile admission.
/// Use `verify_and_admit_post_event` over `radroots_event_from_nostr` whenever
/// the caller needs root Update, PhotoUpdate, or Ask admission. Its explicit
/// thread-excluded outcome makes no Reply claim.
pub fn to_post_event_metadata(e: &RadrootsNostrEvent) -> RadrootsParsedData<LegacyPost> {
    RadrootsParsedData::new(
        e.id.to_string(),
        e.pubkey.to_string(),
        e.created_at.as_secs(),
        e.kind.as_u16() as u32,
        LegacyPost {
            content: e.content.clone(),
            farm: None,
            address_refs: None,
            location: None,
            topics: None,
            quote_refs: None,
            media: None,
        },
    )
}

#[cfg(feature = "events")]
/// Adapts an event through the compatibility-only legacy Profile decoder.
///
/// This helper does not establish kind, identifier, or signature verification
/// and is outside `profile.parse_inbound_metadata`.
pub fn to_profile_event_metadata(
    e: &RadrootsNostrEvent,
) -> Option<RadrootsParsedData<RadrootsProfileData>> {
    let profile_type = e
        .tags
        .iter()
        .filter_map(|tag| {
            let values = tag.as_slice();
            if values.first().map(|v| v.as_str()) != Some(RADROOTS_PROFILE_TYPE_TAG_KEY) {
                return None;
            }
            values
                .get(1)
                .and_then(|value| radroots_profile_type_from_tag_value(value))
        })
        .next();

    if let Ok(p) = serde_json::from_str::<LegacyProfile>(&e.content) {
        return Some(RadrootsParsedData::new(
            e.id.to_string(),
            e.pubkey.to_string(),
            e.created_at.as_secs(),
            e.kind.as_u16() as u32,
            RadrootsProfileData {
                profile_type,
                profile: p,
            },
        ));
    }

    if let Ok(md) = serde_json::from_str::<RadrootsNostrMetadata>(&e.content) {
        let p = LegacyProfile {
            name: md.name.unwrap_or_default(),
            display_name: md.display_name,
            nip05: md.nip05,
            about: md.about,
            website: md.website.map(|u| u.to_string()),
            picture: md.picture.map(|u| u.to_string()),
            banner: md.banner.map(|u| u.to_string()),
            lud06: md.lud06,
            lud16: md.lud16,
            bot: None,
        };
        return Some(RadrootsParsedData::new(
            e.id.to_string(),
            e.pubkey.to_string(),
            e.created_at.as_secs(),
            e.kind.as_u16() as u32,
            RadrootsProfileData {
                profile_type,
                profile: p,
            },
        ));
    }

    None
}

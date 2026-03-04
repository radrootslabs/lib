#[cfg(feature = "events")]
use radroots_events::post::RadrootsPost;
#[cfg(feature = "events")]
use radroots_events::profile::{
    RADROOTS_PROFILE_TYPE_TAG_KEY, RadrootsProfile,
    radroots_profile_type_from_tag_value,
};
#[cfg(feature = "events")]
use radroots_events_codec::parsed::RadrootsParsedData;
#[cfg(feature = "events")]
use radroots_events_codec::profile::RadrootsProfileData;

#[cfg(feature = "events")]
use crate::types::{RadrootsNostrEvent, RadrootsNostrMetadata};

#[cfg(feature = "events")]
use crate::util::created_at_u32_saturating;

#[cfg(feature = "events")]
pub fn to_post_event_metadata(e: &RadrootsNostrEvent) -> RadrootsParsedData<RadrootsPost> {
    RadrootsParsedData::new(
        e.id.to_string(),
        e.pubkey.to_string(),
        created_at_u32_saturating(e.created_at),
        e.kind.as_u16() as u32,
        RadrootsPost {
            content: e.content.clone(),
        },
    )
}

#[cfg(feature = "events")]
pub fn to_profile_event_metadata(
    e: &RadrootsNostrEvent,
) -> Option<RadrootsParsedData<RadrootsProfileData>> {
    let profile_type = e
        .tags
        .iter()
        .filter_map(|tag| {
            let values = tag.as_slice();
            if values.get(0).map(|v| v.as_str()) != Some(RADROOTS_PROFILE_TYPE_TAG_KEY) {
                return None;
            }
            values
                .get(1)
                .and_then(|value| radroots_profile_type_from_tag_value(value))
        })
        .next();

    if let Ok(p) = serde_json::from_str::<RadrootsProfile>(&e.content) {
        return Some(RadrootsParsedData::new(
            e.id.to_string(),
            e.pubkey.to_string(),
            created_at_u32_saturating(e.created_at),
            e.kind.as_u16() as u32,
            RadrootsProfileData {
                profile_type,
                profile: p,
            },
        ));
    }

    if let Ok(md) = serde_json::from_str::<RadrootsNostrMetadata>(&e.content) {
        let p = RadrootsProfile {
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
            created_at_u32_saturating(e.created_at),
            e.kind.as_u16() as u32,
            RadrootsProfileData {
                profile_type,
                profile: p,
            },
        ));
    }

    None
}

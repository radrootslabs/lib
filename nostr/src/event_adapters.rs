#[cfg(feature = "events")]
use radroots_events::post::{RadrootsPost, RadrootsPostEventMetadata};
#[cfg(feature = "events")]
use radroots_events::profile::{
    RadrootsProfile,
    RadrootsProfileEventMetadata,
    RADROOTS_ACTOR_TAG_KEY,
    radroots_actor_type_from_tag_value,
};

#[cfg(feature = "events")]
use crate::types::{RadrootsNostrEvent, RadrootsNostrMetadata};

#[cfg(feature = "events")]
use crate::util::created_at_u32_saturating;

#[cfg(feature = "events")]
pub fn to_post_event_metadata(e: &RadrootsNostrEvent) -> RadrootsPostEventMetadata {
    RadrootsPostEventMetadata {
        id: e.id.to_string(),
        author: e.pubkey.to_string(),
        published_at: created_at_u32_saturating(e.created_at),
        kind: e.kind.as_u16() as u32,
        post: RadrootsPost {
            content: e.content.clone(),
        },
    }
}

#[cfg(feature = "events")]
pub fn to_profile_event_metadata(e: &RadrootsNostrEvent) -> Option<RadrootsProfileEventMetadata> {
    let actor = e
        .tags
        .iter()
        .filter_map(|tag| {
            let values = tag.as_slice();
            if values.get(0).map(|v| v.as_str()) != Some(RADROOTS_ACTOR_TAG_KEY) {
                return None;
            }
            values.get(1).and_then(|value| radroots_actor_type_from_tag_value(value))
        })
        .next();

    if let Ok(p) = serde_json::from_str::<RadrootsProfile>(&e.content) {
        return Some(RadrootsProfileEventMetadata {
            id: e.id.to_string(),
            author: e.pubkey.to_string(),
            published_at: created_at_u32_saturating(e.created_at),
            kind: e.kind.as_u16() as u32,
            actor,
            profile: p,
        });
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
        return Some(RadrootsProfileEventMetadata {
            id: e.id.to_string(),
            author: e.pubkey.to_string(),
            published_at: created_at_u32_saturating(e.created_at),
            kind: e.kind.as_u16() as u32,
            actor,
            profile: p,
        });
    }

    None
}

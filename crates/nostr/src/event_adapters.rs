#[cfg(feature = "events")]
use radroots_events::post::models::{RadrootsPost, RadrootsPostEventMetadata};
#[cfg(feature = "events")]
use radroots_events::profile::models::{RadrootsProfile, RadrootsProfileEventMetadata};

#[cfg(feature = "events")]
use nostr::event::Event;

#[cfg(feature = "events")]
pub fn to_post_event_metadata(e: &Event) -> RadrootsPostEventMetadata {
    RadrootsPostEventMetadata {
        id: e.id.to_string(),
        author: e.pubkey.to_string(),
        published_at: e.created_at.as_u64(),
        kind: e.kind.as_u16() as u32,
        post: RadrootsPost {
            content: e.content.clone(),
        },
    }
}

#[cfg(feature = "events")]
pub fn to_profile_event_metadata(e: &Event) -> Option<RadrootsProfileEventMetadata> {
    if let Ok(p) = serde_json::from_str::<RadrootsProfile>(&e.content) {
        return Some(RadrootsProfileEventMetadata {
            id: e.id.to_string(),
            author: e.pubkey.to_string(),
            published_at: e.created_at.as_u64(),
            kind: e.kind.as_u16() as u32,
            profile: p,
        });
    }

    if let Ok(md) = serde_json::from_str::<nostr::Metadata>(&e.content) {
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
            published_at: e.created_at.as_u64(),
            kind: e.kind.as_u16() as u32,
            profile: p,
        });
    }

    None
}

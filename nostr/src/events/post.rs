use crate::error::RadrootsNostrError;
use crate::types::{
    RadrootsNostrEventBuilder,
    RadrootsNostrEventId,
    RadrootsNostrPublicKey,
    RadrootsNostrTag,
};

#[cfg(all(feature = "client", feature = "events"))]
use core::time::Duration;
#[cfg(all(feature = "client", feature = "events"))]
use crate::client::RadrootsNostrClient;
#[cfg(all(feature = "client", feature = "events"))]
use crate::types::{RadrootsNostrFilter, RadrootsNostrKind, RadrootsNostrTimestamp};

pub fn radroots_nostr_build_post_event(content: impl Into<String>) -> RadrootsNostrEventBuilder {
    RadrootsNostrEventBuilder::text_note(content)
}

pub fn radroots_nostr_build_post_reply_event(
    parent_event_id_hex: &str,
    parent_author_hex: &str,
    content: impl Into<String>,
    root_event_id_hex: Option<&str>,
) -> Result<RadrootsNostrEventBuilder, RadrootsNostrError> {
    let parent_id = RadrootsNostrEventId::from_hex(parent_event_id_hex)?;
    let parent_pubkey = RadrootsNostrPublicKey::from_hex(parent_author_hex)?;
    let mut tags: Vec<RadrootsNostrTag> = Vec::new();

    if let Some(root_hex) = root_event_id_hex {
        if !root_hex.is_empty() {
            if let Ok(root_id) = RadrootsNostrEventId::from_hex(root_hex) {
                tags.push(RadrootsNostrTag::event(root_id));
            }
        }
    }

    tags.push(RadrootsNostrTag::event(parent_id));
    tags.push(RadrootsNostrTag::public_key(parent_pubkey));

    Ok(RadrootsNostrEventBuilder::text_note(content).tags(tags))
}

#[cfg(all(feature = "client", feature = "events"))]
pub async fn radroots_nostr_fetch_post_events(
    client: &RadrootsNostrClient,
    limit: u16,
    since_unix: Option<u64>,
) -> Result<Vec<radroots_events::post::RadrootsPostEventMetadata>, RadrootsNostrError> {
    let mut filter = RadrootsNostrFilter::new()
        .kind(RadrootsNostrKind::TextNote)
        .limit(limit.into());

    if let Some(s) = since_unix {
        filter = filter.since(RadrootsNostrTimestamp::from(s));
    }

    let events = client.fetch_events(filter, Duration::from_secs(10)).await?;
    let out = events
        .into_iter()
        .map(|ev| crate::event_adapters::to_post_event_metadata(&ev))
        .collect();

    Ok(out)
}

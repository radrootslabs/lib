use crate::error::NostrUtilsError;

#[cfg(all(feature = "sdk", feature = "events"))]
use core::time::Duration;
use nostr::{
    event::{EventBuilder, EventId, Tag},
    key::PublicKey,
};
#[cfg(all(feature = "sdk", feature = "events"))]
use nostr_sdk::prelude::{Client, Filter, Kind, Timestamp};

pub fn build_post_event(content: impl Into<String>) -> EventBuilder {
    EventBuilder::text_note(content)
}

pub fn build_post_reply_event(
    parent_event_id_hex: &str,
    parent_author_hex: &str,
    content: impl Into<String>,
    root_event_id_hex: Option<&str>,
) -> Result<EventBuilder, NostrUtilsError> {
    let parent_id = EventId::from_hex(parent_event_id_hex)?;
    let parent_pubkey = PublicKey::from_hex(parent_author_hex)?;
    let mut tags: Vec<Tag> = Vec::new();

    if let Some(root_hex) = root_event_id_hex {
        if !root_hex.is_empty() {
            if let Ok(root_id) = EventId::from_hex(root_hex) {
                tags.push(Tag::event(root_id));
            }
        }
    }

    tags.push(Tag::event(parent_id));
    tags.push(Tag::public_key(parent_pubkey));

    Ok(EventBuilder::text_note(content).tags(tags))
}

#[cfg(all(feature = "sdk", feature = "events"))]
pub async fn fetch_post_events(
    client: &Client,
    limit: u16,
    since_unix: Option<u64>,
) -> Result<Vec<radroots_events::post::models::RadrootsPostEventMetadata>, NostrUtilsError> {
    let mut filter = Filter::new().kind(Kind::TextNote).limit(limit.into());

    if let Some(s) = since_unix {
        filter = filter.since(Timestamp::from(s));
    }

    let events = client.fetch_events(filter, Duration::from_secs(10)).await?;
    let out = events
        .into_iter()
        .map(|ev| crate::event_adapters::to_post_event_metadata(&ev))
        .collect();

    Ok(out)
}

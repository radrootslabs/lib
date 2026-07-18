use crate::error::RadrootsNostrError;
use crate::types::{
    RadrootsNostrEventBuilder, RadrootsNostrEventId, RadrootsNostrFilter, RadrootsNostrKind,
    RadrootsNostrPublicKey, RadrootsNostrTag, RadrootsNostrTimestamp,
};

#[cfg(feature = "events")]
use radroots_event::post::{
    RadrootsAuthoredAsk, RadrootsAuthoredPhotoUpdate, RadrootsAuthoredUpdate,
};
#[cfg(feature = "events")]
use radroots_event::wire::RadrootsNip01EventWireParts;
#[cfg(feature = "events")]
use radroots_event_codec::post::authored::{
    authored_ask_to_wire_parts, authored_photo_update_to_wire_parts, authored_update_to_wire_parts,
};

#[cfg(all(feature = "client", feature = "events"))]
use crate::client::RadrootsNostrClient;
#[cfg(all(feature = "client", feature = "events"))]
use core::time::Duration;

#[cfg(feature = "events")]
pub fn radroots_nostr_build_update_event(
    update: &RadrootsAuthoredUpdate,
) -> Result<RadrootsNostrEventBuilder, RadrootsNostrError> {
    builder_from_wire_parts(authored_update_to_wire_parts(update))
}

#[cfg(feature = "events")]
pub fn radroots_nostr_build_photo_update_event(
    photo: &RadrootsAuthoredPhotoUpdate,
) -> Result<RadrootsNostrEventBuilder, RadrootsNostrError> {
    builder_from_wire_parts(authored_photo_update_to_wire_parts(photo))
}

#[cfg(feature = "events")]
pub fn radroots_nostr_build_ask_event(
    ask: &RadrootsAuthoredAsk,
) -> Result<RadrootsNostrEventBuilder, RadrootsNostrError> {
    builder_from_wire_parts(authored_ask_to_wire_parts(ask))
}

pub fn radroots_nostr_post_events_filter(
    limit: Option<u16>,
    since_unix: Option<u64>,
) -> RadrootsNostrFilter {
    let mut filter = RadrootsNostrFilter::new().kind(RadrootsNostrKind::TextNote);
    if let Some(limit) = limit {
        filter = filter.limit(limit.into());
    }
    if let Some(since) = since_unix {
        filter = filter.since(RadrootsNostrTimestamp::from(since));
    }
    filter
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

    if let Some(root_hex) = root_event_id_hex
        && !root_hex.is_empty()
        && let Ok(root_id) = RadrootsNostrEventId::from_hex(root_hex)
    {
        tags.push(RadrootsNostrTag::event(root_id));
    }

    tags.push(RadrootsNostrTag::event(parent_id));
    tags.push(RadrootsNostrTag::public_key(parent_pubkey));

    Ok(RadrootsNostrEventBuilder::text_note(content).tags(tags))
}

#[cfg(feature = "events")]
fn builder_from_wire_parts(
    parts: RadrootsNip01EventWireParts,
) -> Result<RadrootsNostrEventBuilder, RadrootsNostrError> {
    crate::events::radroots_nostr_build_event(parts.kind, parts.content, parts.tags)
}

#[cfg(all(feature = "client", feature = "events"))]
/// Fetches generic kind-1 events through the compatibility post projection.
///
/// The unmarked filter intentionally retains ordinary Nostr notes and replies.
/// This compatibility read discards tags and does not establish Radroots
/// product admission; product consumers must use the verified admission API.
pub async fn radroots_nostr_fetch_post_events(
    client: &RadrootsNostrClient,
    limit: u16,
    since_unix: Option<u64>,
) -> Result<
    Vec<radroots_event_codec::parsed::RadrootsParsedData<radroots_event::post::RadrootsPost>>,
    RadrootsNostrError,
> {
    let filter = radroots_nostr_post_events_filter(Some(limit), since_unix);

    let events = client.fetch_events(filter, Duration::from_secs(10)).await?;
    let out = events
        .into_iter()
        .map(|ev| crate::event_adapters::to_post_event_metadata(&ev))
        .collect();

    Ok(out)
}

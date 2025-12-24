use crate::types::{RadrootsNostrEventBuilder, RadrootsNostrMetadata};

#[cfg(feature = "client")]
use core::time::Duration;
#[cfg(feature = "client")]
use crate::client::RadrootsNostrClient;
#[cfg(feature = "client")]
use crate::error::RadrootsNostrError;
#[cfg(feature = "client")]
use crate::types::{
    RadrootsNostrEvent,
    RadrootsNostrEventId,
    RadrootsNostrFilter,
    RadrootsNostrKind,
    RadrootsNostrOutput,
    RadrootsNostrPublicKey,
};

pub fn radroots_nostr_build_metadata_event(md: &RadrootsNostrMetadata) -> RadrootsNostrEventBuilder {
    RadrootsNostrEventBuilder::metadata(md)
}

#[cfg(feature = "client")]
pub async fn radroots_nostr_post_metadata_event(
    client: &RadrootsNostrClient,
    md: &RadrootsNostrMetadata,
) -> Result<RadrootsNostrOutput<RadrootsNostrEventId>, RadrootsNostrError> {
    let builder = radroots_nostr_build_metadata_event(md);
    Ok(client.send_event_builder(builder).await?)
}

#[cfg(feature = "client")]
pub async fn radroots_nostr_fetch_metadata_for_author(
    client: &RadrootsNostrClient,
    author: RadrootsNostrPublicKey,
    timeout: Duration,
) -> Result<Option<RadrootsNostrEvent>, RadrootsNostrError> {
    let filter = RadrootsNostrFilter::new()
        .authors(vec![author])
        .kind(RadrootsNostrKind::Metadata);
    let stored = client.database().query(filter.clone()).await?;
    let fetched = client.fetch_events(filter, timeout).await?;

    let mut latest: Option<RadrootsNostrEvent> = None;
    for ev in stored.into_iter().chain(fetched.into_iter()) {
        if ev.kind != RadrootsNostrKind::Metadata {
            continue;
        }
        match &latest {
            Some(cur) if ev.created_at <= cur.created_at => {}
            _ => latest = Some(ev),
        }
    }
    Ok(latest)
}

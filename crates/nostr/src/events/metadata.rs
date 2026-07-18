use crate::types::{RadrootsNostrEventBuilder, RadrootsNostrMetadata};

#[cfg(feature = "client")]
use crate::client::RadrootsNostrClient;
#[cfg(feature = "client")]
use crate::error::RadrootsNostrError;
#[cfg(feature = "client")]
use crate::types::{
    RadrootsNostrEvent, RadrootsNostrEventId, RadrootsNostrFilter, RadrootsNostrKind,
    RadrootsNostrOutput, RadrootsNostrPublicKey,
};
#[cfg(feature = "client")]
use core::time::Duration;

/// Builds kind-0 metadata through the legacy generic metadata surface.
///
/// This compatibility API does not enforce the strict authored Profile media
/// contract. New strict Profile authoring must use
/// `profile.build_authored_draft` before signing.
pub fn radroots_nostr_build_metadata_event(
    md: &RadrootsNostrMetadata,
) -> RadrootsNostrEventBuilder {
    RadrootsNostrEventBuilder::metadata(md)
}

#[cfg(feature = "client")]
/// Publishes kind-0 metadata through the legacy generic metadata surface.
///
/// This compatibility API does not enforce the strict authored Profile media
/// or upload-completion contract. New strict Profile authoring must use
/// `profile.build_authored_draft` and prove BUD-02 completion before signing.
pub async fn radroots_nostr_post_metadata_event(
    client: &RadrootsNostrClient,
    md: &RadrootsNostrMetadata,
) -> Result<RadrootsNostrOutput<RadrootsNostrEventId>, RadrootsNostrError> {
    let builder = radroots_nostr_build_metadata_event(md);
    client.send_event_builder(builder).await
}

#[cfg(feature = "client")]
/// Fetches metadata through the legacy compatibility path.
///
/// This helper is outside `profile.parse_inbound_metadata`; callers must not
/// treat its result as strict Profile admission.
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
    for ev in stored.into_iter().chain(fetched) {
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

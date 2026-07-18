#[cfg(feature = "client")]
use crate::client::RadrootsNostrClient;
#[cfg(feature = "client")]
use crate::error::RadrootsNostrError;
#[cfg(feature = "client")]
use crate::types::{
    RadrootsNostrEvent, RadrootsNostrFilter, RadrootsNostrKind, RadrootsNostrPublicKey,
};
#[cfg(feature = "client")]
use core::time::Duration;

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
    let stored = client.query_database(filter.clone()).await?;
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

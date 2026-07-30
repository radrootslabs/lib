#[cfg(feature = "client")]
use crate::client::RadrootsNostrClient;
#[cfg(any(feature = "client", feature = "events"))]
use crate::error::RadrootsNostrError;
#[cfg(any(feature = "client", feature = "events"))]
use crate::types::RadrootsNostrEvent;
#[cfg(feature = "events")]
use crate::types::{RadrootsNostrEventBuilderUnchecked, RadrootsNostrKeys, RadrootsNostrTimestamp};
#[cfg(feature = "client")]
use crate::types::{RadrootsNostrFilter, RadrootsNostrKind, RadrootsNostrPublicKey};
#[cfg(feature = "client")]
use core::time::Duration;
#[cfg(feature = "events")]
use radroots_event::profile::AuthoredProfile;
#[cfg(feature = "events")]
use radroots_event_codec::encode::profile::authored_profile_to_wire_parts;

/// A sealed builder for a validated kind-0 Profile replacement snapshot.
///
/// The wrapper exposes no raw builder conversion or tag/content mutation.
/// Media-bearing profiles still require the owning runtime to prove successful
/// BUD-02 upload completion before signing or publication.
#[cfg(feature = "events")]
#[must_use = "Profile event builders must be signed or published"]
pub struct RadrootsNostrProfileEventBuilder {
    inner: RadrootsNostrEventBuilderUnchecked,
}

#[cfg(feature = "events")]
impl RadrootsNostrProfileEventBuilder {
    /// Sets the event timestamp without changing the validated Profile shape.
    pub fn custom_created_at(mut self, created_at: RadrootsNostrTimestamp) -> Self {
        self.inner = self.inner.custom_created_at(created_at);
        self
    }

    /// Signs the validated Profile directly with local keys.
    pub fn sign_with_keys(
        self,
        keys: &RadrootsNostrKeys,
    ) -> Result<RadrootsNostrEvent, RadrootsNostrError> {
        Ok(self.inner.sign_with_keys(keys)?)
    }

    #[cfg(feature = "client")]
    pub(crate) fn into_event_builder(self) -> RadrootsNostrEventBuilderUnchecked {
        self.inner
    }
}

/// Builds a sealed kind-0 event from the strict authored Profile contract.
#[cfg(feature = "events")]
pub fn radroots_nostr_build_profile_event(
    profile: &AuthoredProfile,
) -> Result<RadrootsNostrProfileEventBuilder, RadrootsNostrError> {
    let parts = authored_profile_to_wire_parts(profile)?;
    let inner =
        crate::events::radroots_nostr_build_event_unchecked(parts.kind, parts.content, parts.tags)?;
    Ok(RadrootsNostrProfileEventBuilder { inner })
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

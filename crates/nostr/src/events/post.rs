#[cfg(feature = "events")]
use crate::error::RadrootsNostrError;
#[cfg(feature = "events")]
use crate::types::RadrootsNostrEventBuilderUnchecked;
#[cfg(feature = "events")]
use crate::types::{RadrootsNostrEvent, RadrootsNostrKeys};
use crate::types::{RadrootsNostrFilter, RadrootsNostrKind, RadrootsNostrTimestamp};

#[cfg(feature = "events")]
use radroots_event::post::{AuthoredAsk, AuthoredPhotoUpdate, AuthoredUpdate};
#[cfg(feature = "events")]
use radroots_event::wire::Nip01EventWireParts;
#[cfg(feature = "events")]
use radroots_event_codec::encode::post::{
    authored_ask_to_wire_parts, authored_photo_update_to_wire_parts, authored_update_to_wire_parts,
};

#[cfg(all(feature = "client", feature = "events"))]
use crate::client::RadrootsNostrClient;
#[cfg(all(feature = "client", feature = "events"))]
use core::time::Duration;

/// A sealed builder for a validated Radroots root post profile.
///
/// The wrapper intentionally exposes no raw builder conversion or tag/content
/// mutation. Construct it through one of the typed post authoring functions.
#[cfg(feature = "events")]
#[must_use = "post event builders must be signed or published"]
pub struct RadrootsNostrPostEventBuilder {
    inner: RadrootsNostrEventBuilderUnchecked,
}

#[cfg(feature = "events")]
impl RadrootsNostrPostEventBuilder {
    /// Sets the event timestamp without changing the validated post shape.
    pub fn custom_created_at(mut self, created_at: RadrootsNostrTimestamp) -> Self {
        self.inner = self.inner.custom_created_at(created_at);
        self
    }

    /// Signs the validated post directly with local keys.
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

#[cfg(feature = "events")]
pub fn radroots_nostr_build_update_event(
    update: &AuthoredUpdate,
) -> Result<RadrootsNostrPostEventBuilder, RadrootsNostrError> {
    builder_from_wire_parts(authored_update_to_wire_parts(update))
}

#[cfg(feature = "events")]
pub fn radroots_nostr_build_photo_update_event(
    photo: &AuthoredPhotoUpdate,
) -> Result<RadrootsNostrPostEventBuilder, RadrootsNostrError> {
    builder_from_wire_parts(authored_photo_update_to_wire_parts(photo))
}

#[cfg(feature = "events")]
pub fn radroots_nostr_build_ask_event(
    ask: &AuthoredAsk,
) -> Result<RadrootsNostrPostEventBuilder, RadrootsNostrError> {
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

#[cfg(feature = "events")]
fn builder_from_wire_parts(
    parts: Nip01EventWireParts,
) -> Result<RadrootsNostrPostEventBuilder, RadrootsNostrError> {
    let inner =
        crate::events::radroots_nostr_build_event_unchecked(parts.kind, parts.content, parts.tags)?;
    Ok(RadrootsNostrPostEventBuilder { inner })
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
    Vec<
        radroots_event_codec::decode::RadrootsParsedData<
            radroots_event_codec::decode::post::LegacyPost,
        >,
    >,
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

//! Typed authoring and filtering for supported Radroots social root events.

#[cfg(feature = "events")]
use crate::error::Error;
#[cfg(feature = "events")]
use crate::events::sealed::SealedBuilderCore;
#[cfg(feature = "events")]
use crate::types::{
    ExternalSigningRequest, RadrootsNostrEvent, RadrootsNostrKeys, RadrootsNostrPublicKey,
};
use crate::types::{RadrootsNostrFilter, RadrootsNostrKind, RadrootsNostrTimestamp};

#[cfg(feature = "events")]
use radroots_event::post::{AuthoredAsk, AuthoredPhotoUpdate, AuthoredUpdate};
#[cfg(feature = "events")]
use radroots_event_codec::authoring::AuthoredEventBody;

/// A sealed builder for a validated Radroots root post profile.
///
/// The wrapper intentionally exposes no raw builder conversion or tag/content
/// mutation. Construct it through one of the typed post authoring functions.
#[cfg(feature = "events")]
#[must_use = "post event builders must be signed or published"]
pub struct PostBuilder {
    inner: SealedBuilderCore,
}

#[cfg(feature = "events")]
impl PostBuilder {
    /// Sets the event timestamp without changing the validated post shape.
    pub fn custom_created_at(mut self, created_at: RadrootsNostrTimestamp) -> Self {
        self.inner = self.inner.custom_created_at(created_at);
        self
    }

    /// Signs the validated post directly with local keys.
    pub fn sign_with_keys(self, keys: &RadrootsNostrKeys) -> Result<RadrootsNostrEvent, Error> {
        self.inner.sign_with_keys(keys)
    }

    /// Finalizes the exact authored plan for an external signer.
    pub fn into_external_signing_request(
        self,
        public_key: RadrootsNostrPublicKey,
    ) -> Result<ExternalSigningRequest, Error> {
        self.inner.into_external_signing_request(public_key)
    }
}

#[cfg(feature = "events")]
pub fn build_update_event(update: &AuthoredUpdate) -> Result<PostBuilder, Error> {
    Ok(builder_from_body(AuthoredEventBody::from_update(update)?))
}

#[cfg(feature = "events")]
pub fn build_photo_update_event(photo: &AuthoredPhotoUpdate) -> Result<PostBuilder, Error> {
    Ok(builder_from_body(AuthoredEventBody::from_photo_update(
        photo,
    )?))
}

#[cfg(feature = "events")]
pub fn build_ask_event(ask: &AuthoredAsk) -> Result<PostBuilder, Error> {
    Ok(builder_from_body(AuthoredEventBody::from_ask(ask)?))
}

pub fn post_events_filter(limit: Option<u16>, since_unix: Option<u64>) -> RadrootsNostrFilter {
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
fn builder_from_body(body: AuthoredEventBody) -> PostBuilder {
    PostBuilder {
        inner: SealedBuilderCore::new(body),
    }
}

//! Sealed authoring for validated NIP-09 deletion requests.

use crate::{
    error::Error,
    events::sealed::SealedBuilderCore,
    types::{
        ExternalSigningRequest, RadrootsNostrEvent, RadrootsNostrKeys, RadrootsNostrPublicKey,
        RadrootsNostrTimestamp,
    },
};
use radroots_event::post::deletion::AuthoredNip09DeletionRequest;
use radroots_event_codec::authoring::AuthoredEventBody;

/// A sealed builder for a validated NIP-09 deletion request.
///
/// The wrapper exposes no raw builder conversion or tag/content mutation.
/// Signing or publication establishes no target-deletion authorization or
/// effect.
///
/// ```compile_fail
/// use radroots_nostr::event::Nip09DeletionRequestBuilder;
///
/// fn expose_raw_builder(builder: Nip09DeletionRequestBuilder) {
///     let _: nostr::EventBuilder = builder.into();
/// }
/// ```
///
/// ```compile_fail
/// use radroots_nostr::{event::Nip09DeletionRequestBuilder, tag::Tag};
///
/// fn mutate_validated_tags(
///     builder: Nip09DeletionRequestBuilder,
///     tag: Tag,
/// ) {
///     let _ = builder.tag(tag);
/// }
/// ```
#[must_use = "NIP-09 deletion request builders must be signed or published"]
pub struct Nip09DeletionRequestBuilder {
    inner: SealedBuilderCore,
}

impl Nip09DeletionRequestBuilder {
    pub fn custom_created_at(mut self, created_at: RadrootsNostrTimestamp) -> Self {
        self.inner = self.inner.custom_created_at(created_at);
        self
    }

    pub fn sign_with_keys(self, keys: &RadrootsNostrKeys) -> Result<RadrootsNostrEvent, Error> {
        self.inner.sign_with_keys(keys)
    }

    pub fn into_external_signing_request(
        self,
        public_key: RadrootsNostrPublicKey,
    ) -> Result<ExternalSigningRequest, Error> {
        self.inner.into_external_signing_request(public_key)
    }
}

pub fn build_nip09_deletion_request_event(
    request: &AuthoredNip09DeletionRequest,
) -> Result<Nip09DeletionRequestBuilder, Error> {
    Ok(Nip09DeletionRequestBuilder {
        inner: SealedBuilderCore::new(AuthoredEventBody::from_nip09_deletion_request(request)?),
    })
}

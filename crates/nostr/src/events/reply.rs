//! Sealed authoring for strict marked NIP-10 Reply events.

use crate::{
    error::Error,
    events::sealed::SealedBuilderCore,
    types::{
        ExternalSigningRequest, RadrootsNostrEvent, RadrootsNostrKeys, RadrootsNostrPublicKey,
        RadrootsNostrTimestamp,
    },
};
use radroots_event::post::reply::AuthoredNip10Reply;
use radroots_event_codec::authoring::AuthoredEventBody;

/// A sealed builder for a validated strict marked NIP-10 Reply.
///
/// The wrapper exposes no raw builder conversion or tag/content mutation.
///
/// ```compile_fail
/// use radroots_nostr::event::Nip10ReplyBuilder;
///
/// fn expose_raw_builder(builder: Nip10ReplyBuilder) {
///     let _: nostr::EventBuilder = builder.into();
/// }
/// ```
#[must_use = "NIP-10 Reply builders must be signed or published"]
pub struct Nip10ReplyBuilder {
    inner: SealedBuilderCore,
}

impl Nip10ReplyBuilder {
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

pub fn build_nip10_reply_event(reply: &AuthoredNip10Reply) -> Result<Nip10ReplyBuilder, Error> {
    Ok(Nip10ReplyBuilder {
        inner: SealedBuilderCore::new(AuthoredEventBody::from_nip10_reply(reply)?),
    })
}

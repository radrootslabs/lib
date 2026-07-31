//! Sealed authoring for strict marked NIP-10 Reply events.

use crate::{
    error::Error,
    types::{
        RadrootsNostrEvent, RadrootsNostrEventBuilderUnchecked, RadrootsNostrKeys,
        RadrootsNostrTimestamp,
    },
};
use radroots_event::{post::reply::AuthoredNip10Reply, wire::Nip01EventWireParts};
use radroots_event_codec::encode::reply::authored_nip10_reply_to_wire_parts;

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
    inner: RadrootsNostrEventBuilderUnchecked,
}

impl Nip10ReplyBuilder {
    pub fn custom_created_at(mut self, created_at: RadrootsNostrTimestamp) -> Self {
        self.inner = self.inner.custom_created_at(created_at);
        self
    }

    pub fn sign_with_keys(self, keys: &RadrootsNostrKeys) -> Result<RadrootsNostrEvent, Error> {
        Ok(self.inner.sign_with_keys(keys)?)
    }
}

pub fn build_nip10_reply_event(reply: &AuthoredNip10Reply) -> Result<Nip10ReplyBuilder, Error> {
    let parts = authored_nip10_reply_to_wire_parts(reply);
    builder_from_wire_parts(parts)
}

fn builder_from_wire_parts(parts: Nip01EventWireParts) -> Result<Nip10ReplyBuilder, Error> {
    let inner = crate::events::build_event_unchecked(parts.kind, parts.content, parts.tags)?;
    Ok(Nip10ReplyBuilder { inner })
}

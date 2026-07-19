use crate::{
    error::RadrootsNostrError,
    types::{
        RadrootsNostrEvent, RadrootsNostrEventBuilderUnchecked, RadrootsNostrKeys,
        RadrootsNostrTimestamp,
    },
};
use radroots_event::{reply::RadrootsAuthoredNip10Reply, wire::RadrootsNip01EventWireParts};
use radroots_event_codec::reply::authored::authored_nip10_reply_to_wire_parts;

/// A sealed builder for a validated strict marked NIP-10 Reply.
///
/// The wrapper exposes no raw builder conversion or tag/content mutation.
///
/// ```compile_fail
/// use radroots_nostr::prelude::RadrootsNostrNip10ReplyEventBuilder;
///
/// fn expose_raw_builder(builder: RadrootsNostrNip10ReplyEventBuilder) {
///     let _: nostr::EventBuilder = builder.into();
/// }
/// ```
#[must_use = "NIP-10 Reply builders must be signed or published"]
pub struct RadrootsNostrNip10ReplyEventBuilder {
    inner: RadrootsNostrEventBuilderUnchecked,
}

impl RadrootsNostrNip10ReplyEventBuilder {
    pub fn custom_created_at(mut self, created_at: RadrootsNostrTimestamp) -> Self {
        self.inner = self.inner.custom_created_at(created_at);
        self
    }

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

pub fn radroots_nostr_build_nip10_reply_event(
    reply: &RadrootsAuthoredNip10Reply,
) -> Result<RadrootsNostrNip10ReplyEventBuilder, RadrootsNostrError> {
    let parts = authored_nip10_reply_to_wire_parts(reply);
    builder_from_wire_parts(parts)
}

fn builder_from_wire_parts(
    parts: RadrootsNip01EventWireParts,
) -> Result<RadrootsNostrNip10ReplyEventBuilder, RadrootsNostrError> {
    let inner =
        crate::events::radroots_nostr_build_event_unchecked(parts.kind, parts.content, parts.tags)?;
    Ok(RadrootsNostrNip10ReplyEventBuilder { inner })
}

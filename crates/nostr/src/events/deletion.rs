use crate::{
    error::RadrootsNostrError,
    types::{
        RadrootsNostrEvent, RadrootsNostrEventBuilderUnchecked, RadrootsNostrKeys,
        RadrootsNostrTimestamp,
    },
};
use radroots_event::{post::deletion::AuthoredNip09DeletionRequest, wire::Nip01EventWireParts};
use radroots_event_codec::encode::deletion::authored_nip09_deletion_request_to_wire_parts;

/// A sealed builder for a validated NIP-09 deletion request.
///
/// The wrapper exposes no raw builder conversion or tag/content mutation.
/// Signing or publication establishes no target-deletion authorization or
/// effect.
///
/// ```compile_fail
/// use radroots_nostr::prelude::RadrootsNostrNip09DeletionRequestEventBuilder;
///
/// fn expose_raw_builder(builder: RadrootsNostrNip09DeletionRequestEventBuilder) {
///     let _: nostr::EventBuilder = builder.into();
/// }
/// ```
///
/// ```compile_fail
/// use radroots_nostr::prelude::{
///     RadrootsNostrNip09DeletionRequestEventBuilder, RadrootsNostrTag,
/// };
///
/// fn mutate_validated_tags(
///     builder: RadrootsNostrNip09DeletionRequestEventBuilder,
///     tag: RadrootsNostrTag,
/// ) {
///     let _ = builder.tag(tag);
/// }
/// ```
#[must_use = "NIP-09 deletion request builders must be signed or published"]
pub struct RadrootsNostrNip09DeletionRequestEventBuilder {
    inner: RadrootsNostrEventBuilderUnchecked,
}

impl RadrootsNostrNip09DeletionRequestEventBuilder {
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

pub fn radroots_nostr_build_nip09_deletion_request_event(
    request: &AuthoredNip09DeletionRequest,
) -> Result<RadrootsNostrNip09DeletionRequestEventBuilder, RadrootsNostrError> {
    builder_from_wire_parts(authored_nip09_deletion_request_to_wire_parts(request))
}

fn builder_from_wire_parts(
    parts: Nip01EventWireParts,
) -> Result<RadrootsNostrNip09DeletionRequestEventBuilder, RadrootsNostrError> {
    let inner =
        crate::events::radroots_nostr_build_event_unchecked(parts.kind, parts.content, parts.tags)?;
    Ok(RadrootsNostrNip09DeletionRequestEventBuilder { inner })
}

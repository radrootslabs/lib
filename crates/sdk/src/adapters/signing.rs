use crate::identity::RadrootsIdentity;
use radroots_event_codec::wire::WireEventParts;
use radroots_nostr::prelude::{RadrootsNostrError, radroots_nostr_build_event};

pub type SignedNostrEvent = radroots_nostr::prelude::RadrootsNostrEvent;
pub type NostrEventBuilder = radroots_nostr::prelude::RadrootsNostrEventBuilder;
pub type SigningError = RadrootsNostrError;

pub fn event_builder_from_parts(parts: WireEventParts) -> Result<NostrEventBuilder, SigningError> {
    radroots_nostr_build_event(parts.kind, parts.content, parts.tags)
}

pub fn sign_parts_with_identity(
    identity: &RadrootsIdentity,
    parts: WireEventParts,
) -> Result<SignedNostrEvent, SigningError> {
    let builder = event_builder_from_parts(parts)?;
    sign_builder_with_identity(identity, builder)
}

pub fn sign_builder_with_identity(
    identity: &RadrootsIdentity,
    builder: NostrEventBuilder,
) -> Result<SignedNostrEvent, SigningError> {
    builder.sign_with_keys(identity.keys()).map_err(Into::into)
}

#[cfg(test)]
#[path = "../../tests/unit/adapters_signing_tests.rs"]
mod tests;

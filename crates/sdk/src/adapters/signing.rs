use crate::identity::RadrootsIdentity;
use radroots_event::wire::RadrootsNip01EventWireParts;
use radroots_nostr::prelude::{
    RadrootsNostrError, RadrootsNostrEvent, RadrootsNostrEventBuilder, radroots_nostr_build_event,
};

pub type SigningError = RadrootsNostrError;

pub fn event_builder_from_parts(
    parts: RadrootsNip01EventWireParts,
) -> Result<RadrootsNostrEventBuilder, SigningError> {
    radroots_nostr_build_event(parts.kind, parts.content, parts.tags)
}

pub fn sign_parts_with_identity(
    identity: &RadrootsIdentity,
    parts: RadrootsNip01EventWireParts,
) -> Result<RadrootsNostrEvent, SigningError> {
    let builder = event_builder_from_parts(parts)?;
    sign_builder_with_identity(identity, builder)
}

pub fn sign_builder_with_identity(
    identity: &RadrootsIdentity,
    builder: RadrootsNostrEventBuilder,
) -> Result<RadrootsNostrEvent, SigningError> {
    builder.sign_with_keys(identity.keys()).map_err(Into::into)
}

#[cfg(test)]
#[path = "../../tests/unit/adapters_signing_tests.rs"]
mod tests;

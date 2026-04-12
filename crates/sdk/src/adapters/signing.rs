use crate::WireEventParts;
use crate::identity::RadrootsIdentity;
use radroots_nostr::prelude::{
    RadrootsNostrError, radroots_nostr_build_event,
};

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
mod tests {
    use super::{event_builder_from_parts, sign_parts_with_identity};
    use crate::{WireEventParts, identity::RadrootsIdentity};

    #[test]
    fn event_builder_from_parts_preserves_kind_and_content() {
        let builder = event_builder_from_parts(WireEventParts {
            kind: 30402,
            content: "hello".into(),
            tags: vec![vec!["x".into(), "y".into()]],
        })
        .expect("builder");
        let identity = RadrootsIdentity::generate();
        let event = builder.build(identity.keys().public_key());

        assert_eq!(u16::from(event.kind), 30402);
        assert_eq!(event.content, "hello");
    }

    #[test]
    fn sign_parts_with_identity_signs_event() {
        let identity = RadrootsIdentity::generate();
        let event = sign_parts_with_identity(
            &identity,
            WireEventParts {
                kind: 30402,
                content: "hello".into(),
                tags: vec![],
            },
        )
        .expect("signed event");

        assert_eq!(event.pubkey.to_hex(), identity.public_key_hex());
    }
}

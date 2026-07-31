#![forbid(unsafe_code)]

use crate::error::Error;
use crate::types::RadrootsNostrEvent as RadrootsNostrRawEvent;
use radroots_event::{
    envelope::EventEnvelope, envelope::EventEnvelopeError, envelope::EventEnvelopeParts,
    tag::EventPtr,
};

pub fn from_nostr(event: &RadrootsNostrRawEvent) -> Result<EventEnvelope, EventEnvelopeError> {
    EventEnvelope::new(EventEnvelopeParts {
        id: event.id.to_string(),
        author: event.pubkey.to_string(),
        created_at: event.created_at.as_secs(),
        kind: event.kind.as_u16() as u32,
        tags: event.tags.iter().map(|t| t.as_slice().to_vec()).collect(),
        content: event.content.clone(),
        sig: event.sig.to_string(),
    })
}

/// Converts a protocol-neutral Radroots envelope into the upstream Nostr value.
///
/// This adapter performs no network access and does not imply that the event's
/// identifier or signature has been verified.
pub fn to_nostr(event: &EventEnvelope) -> Result<RadrootsNostrRawEvent, Error> {
    let kind = u16::try_from(event.kind_u32()).map_err(|_| Error::KindOutOfRange {
        kind: event.kind_u32(),
        max: u16::MAX,
    })?;
    let id = nostr::EventId::from_slice(event.id().as_bytes())
        .map_err(|_| Error::EventConversion { field: "id" })?;
    let public_key = nostr::PublicKey::from_slice(event.author().as_bytes())
        .map_err(|_| Error::EventConversion { field: "author" })?;
    let signature = nostr::secp256k1::schnorr::Signature::from_slice(event.sig().as_bytes())
        .map_err(|_| Error::EventConversion { field: "signature" })?;
    let tags = event
        .tags_as_vec()
        .into_iter()
        .map(nostr::Tag::parse)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| Error::EventConversion { field: "tags" })?;

    Ok(nostr::Event::new(
        id,
        public_key,
        nostr::Timestamp::from_secs(event.created_at_u64()),
        nostr::Kind::Custom(kind),
        tags,
        event.content().to_owned(),
        signature,
    ))
}

pub fn pointer_from_nostr(event: &RadrootsNostrRawEvent) -> EventPtr {
    EventPtr {
        id: event.id.to_string(),
        relays: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::build_event_unchecked;
    use crate::test_fixtures::FIXTURE_ALICE;
    use crate::types::{RadrootsNostrKeys, RadrootsNostrTimestamp};
    use nostr::SecretKey;
    use radroots_event::{
        envelope::EventEnvelopeParts,
        wire::{DEFAULT_CONTENT_MAX_BYTES, DEFAULT_TAG_TOTAL_ELEMENT_MAX_COUNT},
    };

    fn fixture_keys() -> RadrootsNostrKeys {
        let secret_key = SecretKey::from_hex(FIXTURE_ALICE.secret_key_hex).expect("secret key");
        RadrootsNostrKeys::new(secret_key)
    }

    fn signed_event(content: String, tags: Vec<Vec<String>>) -> RadrootsNostrRawEvent {
        build_event_unchecked(1, content, tags)
            .expect("builder")
            .custom_created_at(RadrootsNostrTimestamp::from_secs(1_700_000_000))
            .sign_with_keys(&fixture_keys())
            .expect("signed event")
    }

    #[test]
    fn conversion_round_trips_protocol_neutral_event() {
        let original = signed_event(
            "harvest update".to_owned(),
            vec![vec!["t".to_owned(), "soil".to_owned()]],
        );
        let envelope = from_nostr(&original).expect("Radroots envelope");

        let converted = to_nostr(&envelope).expect("Nostr event");

        assert_eq!(converted, original);
    }

    #[test]
    fn conversion_rejects_kind_outside_nostr_range() {
        let original = signed_event(String::new(), Vec::new());
        let envelope = from_nostr(&original).expect("Radroots envelope");
        let kind = u32::from(u16::MAX) + 1;
        let widened = EventEnvelope::new(EventEnvelopeParts {
            id: envelope.id_hex(),
            author: envelope.author().to_hex(),
            created_at: envelope.created_at_u64(),
            kind,
            tags: envelope.tags_as_vec(),
            content: envelope.content().to_owned(),
            sig: envelope.signature_hex(),
        })
        .expect("wider Radroots kind");

        assert!(matches!(
            to_nostr(&widened),
            Err(Error::KindOutOfRange {
                kind: actual,
                max: u16::MAX,
            }) if actual == kind
        ));
    }

    #[test]
    fn conversion_returns_typed_content_limit_error() {
        let content = "x".repeat(DEFAULT_CONTENT_MAX_BYTES + 1);
        let event = signed_event(content, Vec::new());

        assert_eq!(
            from_nostr(&event),
            Err(EventEnvelopeError::ContentTooLarge {
                max: DEFAULT_CONTENT_MAX_BYTES,
                actual: DEFAULT_CONTENT_MAX_BYTES + 1,
            })
        );
    }

    #[test]
    fn conversion_returns_typed_total_tag_element_limit_error() {
        let mut tag = Vec::with_capacity(DEFAULT_TAG_TOTAL_ELEMENT_MAX_COUNT + 1);
        tag.push("x".to_owned());
        tag.resize(DEFAULT_TAG_TOTAL_ELEMENT_MAX_COUNT + 1, String::new());
        let event = signed_event(String::new(), vec![tag]);

        assert_eq!(
            from_nostr(&event),
            Err(EventEnvelopeError::TooManyTagElements {
                max: DEFAULT_TAG_TOTAL_ELEMENT_MAX_COUNT,
                actual: DEFAULT_TAG_TOTAL_ELEMENT_MAX_COUNT + 1,
            })
        );
    }
}

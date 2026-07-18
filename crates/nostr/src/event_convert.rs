#![forbid(unsafe_code)]

use crate::types::RadrootsNostrEvent as RadrootsNostrRawEvent;
use radroots_event::{
    RadrootsEventEnvelope, RadrootsEventEnvelopeError, RadrootsEventEnvelopeParts, RadrootsEventPtr,
};

pub fn radroots_event_from_nostr(
    event: &RadrootsNostrRawEvent,
) -> Result<RadrootsEventEnvelope, RadrootsEventEnvelopeError> {
    RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
        id: event.id.to_string(),
        author: event.pubkey.to_string(),
        created_at: event.created_at.as_secs(),
        kind: event.kind.as_u16() as u32,
        tags: event.tags.iter().map(|t| t.as_slice().to_vec()).collect(),
        content: event.content.clone(),
        sig: event.sig.to_string(),
    })
}

pub fn radroots_event_ptr_from_nostr(event: &RadrootsNostrRawEvent) -> RadrootsEventPtr {
    RadrootsEventPtr {
        id: event.id.to_string(),
        relays: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::radroots_nostr_build_event_unchecked;
    use crate::test_fixtures::FIXTURE_ALICE;
    use crate::types::{RadrootsNostrKeys, RadrootsNostrSecretKey, RadrootsNostrTimestamp};
    use radroots_event::wire::{DEFAULT_CONTENT_MAX_BYTES, DEFAULT_TAG_TOTAL_ELEMENT_MAX_COUNT};

    fn fixture_keys() -> RadrootsNostrKeys {
        let secret_key =
            RadrootsNostrSecretKey::from_hex(FIXTURE_ALICE.secret_key_hex).expect("secret key");
        RadrootsNostrKeys::new(secret_key)
    }

    fn signed_event(content: String, tags: Vec<Vec<String>>) -> RadrootsNostrRawEvent {
        radroots_nostr_build_event_unchecked(1, content, tags)
            .expect("builder")
            .custom_created_at(RadrootsNostrTimestamp::from_secs(1_700_000_000))
            .sign_with_keys(&fixture_keys())
            .expect("signed event")
    }

    #[test]
    fn conversion_returns_typed_content_limit_error() {
        let content = "x".repeat(DEFAULT_CONTENT_MAX_BYTES + 1);
        let event = signed_event(content, Vec::new());

        assert_eq!(
            radroots_event_from_nostr(&event),
            Err(RadrootsEventEnvelopeError::ContentTooLarge {
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
            radroots_event_from_nostr(&event),
            Err(RadrootsEventEnvelopeError::TooManyTagElements {
                max: DEFAULT_TAG_TOTAL_ELEMENT_MAX_COUNT,
                actual: DEFAULT_TAG_TOTAL_ELEMENT_MAX_COUNT + 1,
            })
        );
    }
}

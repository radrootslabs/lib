#![forbid(unsafe_code)]

use crate::types::RadrootsNostrEvent as RadrootsNostrRawEvent;
use radroots_event::{RadrootsEventEnvelope, RadrootsEventEnvelopeParts, RadrootsEventPtr};

pub fn radroots_event_from_nostr(event: &RadrootsNostrRawEvent) -> RadrootsEventEnvelope {
    RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
        id: event.id.to_string(),
        author: event.pubkey.to_string(),
        created_at: event.created_at.as_secs(),
        kind: event.kind.as_u16() as u32,
        tags: event.tags.iter().map(|t| t.as_slice().to_vec()).collect(),
        content: event.content.clone(),
        sig: event.sig.to_string(),
    })
    .expect("nostr event is canonical")
}

pub fn radroots_event_ptr_from_nostr(event: &RadrootsNostrRawEvent) -> RadrootsEventPtr {
    RadrootsEventPtr {
        id: event.id.to_string(),
        relays: None,
    }
}

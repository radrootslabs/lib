#![forbid(unsafe_code)]

use crate::types::RadrootsNostrEvent as RadrootsNostrRawEvent;
use radroots_events::{RadrootsNostrEvent, RadrootsNostrEventPtr};

use crate::util::event_created_at_u32_saturating;

pub fn radroots_event_from_nostr(event: &RadrootsNostrRawEvent) -> RadrootsNostrEvent {
    RadrootsNostrEvent {
        id: event.id.to_string(),
        author: event.pubkey.to_string(),
        created_at: event_created_at_u32_saturating(event),
        kind: event.kind.as_u16() as u32,
        tags: event.tags.iter().map(|t| t.as_slice().to_vec()).collect(),
        content: event.content.clone(),
        sig: event.sig.to_string(),
    }
}

pub fn radroots_event_ptr_from_nostr(event: &RadrootsNostrRawEvent) -> RadrootsNostrEventPtr {
    RadrootsNostrEventPtr {
        id: event.id.to_string(),
        relays: None,
    }
}

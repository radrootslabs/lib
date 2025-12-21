#![allow(dead_code)]

use radroots_events::{RadrootsNostrEvent, RadrootsNostrEventPtr, RadrootsNostrEventRef};

pub fn event_ref(id: &str, author: &str, kind: u32) -> RadrootsNostrEventRef {
    RadrootsNostrEventRef {
        id: id.to_string(),
        author: author.to_string(),
        kind,
        d_tag: None,
        relays: None,
    }
}

pub fn event_ref_with_d(
    id: &str,
    author: &str,
    kind: u32,
    d_tag: &str,
    relays: Option<Vec<String>>,
) -> RadrootsNostrEventRef {
    RadrootsNostrEventRef {
        id: id.to_string(),
        author: author.to_string(),
        kind,
        d_tag: Some(d_tag.to_string()),
        relays,
    }
}

pub fn event_ptr(id: &str, relays: Option<&str>) -> RadrootsNostrEventPtr {
    RadrootsNostrEventPtr {
        id: id.to_string(),
        relays: relays.map(|s| s.to_string()),
    }
}

pub fn nostr_event(kind: u32, content: &str, tags: Vec<Vec<String>>) -> RadrootsNostrEvent {
    RadrootsNostrEvent {
        id: "id".to_string(),
        author: "author".to_string(),
        created_at: 123,
        kind,
        tags,
        content: content.to_string(),
        sig: "sig".to_string(),
    }
}

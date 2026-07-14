#![allow(dead_code)]

use radroots_event::{
    RadrootsEventEnvelope, RadrootsEventEnvelopeParts, RadrootsEventPtr, RadrootsEventRef,
};

pub const EVENT_ID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
pub const AUTHOR: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
pub const EVENT_SIG: &str = concat!(
    "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
    "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
);

pub fn event_ref(id: &str, author: &str, kind: u32) -> RadrootsEventRef {
    RadrootsEventRef {
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
) -> RadrootsEventRef {
    RadrootsEventRef {
        id: id.to_string(),
        author: author.to_string(),
        kind,
        d_tag: Some(d_tag.to_string()),
        relays,
    }
}

pub fn event_ptr(id: &str, relays: Option<&str>) -> RadrootsEventPtr {
    RadrootsEventPtr {
        id: id.to_string(),
        relays: relays.map(|s| s.to_string()),
    }
}

pub fn nostr_event(kind: u32, content: &str, tags: Vec<Vec<String>>) -> RadrootsEventEnvelope {
    RadrootsEventEnvelope::new(RadrootsEventEnvelopeParts {
        id: EVENT_ID.to_string(),
        author: AUTHOR.to_string(),
        created_at: 123,
        kind,
        tags,
        content: content.to_string(),
        sig: EVENT_SIG.to_string(),
    })
    .unwrap()
}

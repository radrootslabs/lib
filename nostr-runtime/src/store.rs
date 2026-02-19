use alloc::string::ToString;
use alloc::vec::Vec;
use radroots_nostr::prelude::RadrootsNostrEvent;
use std::sync::Mutex;

pub trait RadrootsNostrEventStore: Send + Sync {
    fn ingest_event(&self, event: &RadrootsNostrEvent) -> Result<(), String>;
}

#[derive(Default)]
pub struct RadrootsNostrInMemoryEventStore {
    events: Mutex<Vec<RadrootsNostrEvent>>,
}

impl RadrootsNostrInMemoryEventStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn events(&self) -> Vec<RadrootsNostrEvent> {
        self.events
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    pub fn len(&self) -> usize {
        self.events.lock().map(|guard| guard.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl RadrootsNostrEventStore for RadrootsNostrInMemoryEventStore {
    fn ingest_event(&self, event: &RadrootsNostrEvent) -> Result<(), String> {
        self.events
            .lock()
            .map_err(|_| "in-memory store lock poisoned".to_string())
            .map(|mut guard| {
                guard.push(event.clone());
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_nostr::prelude::{RadrootsNostrEventBuilder, RadrootsNostrKeys};

    #[test]
    fn in_memory_store_tracks_events() {
        let store = RadrootsNostrInMemoryEventStore::new();
        let keys = RadrootsNostrKeys::generate();
        let event = RadrootsNostrEventBuilder::text_note("hello")
            .sign_with_keys(&keys)
            .expect("event should sign");

        store
            .ingest_event(&event)
            .expect("event should be accepted");
        assert_eq!(store.len(), 1);
        assert!(!store.is_empty());
    }
}

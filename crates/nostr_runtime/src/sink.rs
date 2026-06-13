use alloc::string::ToString;
use alloc::vec::Vec;
use radroots_nostr::prelude::RadrootsNostrEvent;
use std::sync::Mutex;

pub trait RadrootsNostrEventSink: Send + Sync {
    fn ingest_event(&self, event: &RadrootsNostrEvent) -> Result<(), String>;
}

#[derive(Default)]
pub struct RadrootsNostrInMemoryEventSink {
    events: Mutex<Vec<RadrootsNostrEvent>>,
}

impl RadrootsNostrInMemoryEventSink {
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

impl RadrootsNostrEventSink for RadrootsNostrInMemoryEventSink {
    fn ingest_event(&self, event: &RadrootsNostrEvent) -> Result<(), String> {
        self.events
            .lock()
            .map_err(|_| "in-memory sink lock poisoned".to_string())
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
    fn in_memory_sink_tracks_events() {
        let sink = RadrootsNostrInMemoryEventSink::new();
        let keys = RadrootsNostrKeys::generate();
        let event = RadrootsNostrEventBuilder::text_note("hello")
            .sign_with_keys(&keys)
            .expect("event should sign");

        sink.ingest_event(&event).expect("event should be accepted");
        assert_eq!(sink.len(), 1);
        assert!(!sink.is_empty());
    }
}

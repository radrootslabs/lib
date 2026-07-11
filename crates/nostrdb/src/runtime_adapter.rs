use crate::ingest::RadrootsNostrdbIngestSource;
use crate::nostrdb::RadrootsNostrdb;
use radroots_nostr::prelude::RadrootsNostrEvent;
use radroots_nostr_runtime::prelude::RadrootsNostrEventSink;
use std::sync::Arc;

#[derive(Clone)]
pub struct RadrootsNostrdbEventSinkAdapter {
    nostrdb: RadrootsNostrdb,
    source: RadrootsNostrdbIngestSource,
}

fn nostrdb_error_to_string(source: crate::error::RadrootsNostrdbError) -> String {
    source.to_string()
}

impl RadrootsNostrdbEventSinkAdapter {
    pub fn new(nostrdb: RadrootsNostrdb) -> Self {
        Self {
            nostrdb,
            source: RadrootsNostrdbIngestSource::client(),
        }
    }

    pub fn with_source(mut self, source: RadrootsNostrdbIngestSource) -> Self {
        self.source = source;
        self
    }

    pub fn into_event_sink(self) -> Arc<dyn RadrootsNostrEventSink> {
        Arc::new(self)
    }
}

impl RadrootsNostrEventSink for RadrootsNostrdb {
    fn ingest_event(&self, event: &RadrootsNostrEvent) -> Result<(), String> {
        RadrootsNostrdb::ingest_event(self, event, RadrootsNostrdbIngestSource::client())
            .map_err(nostrdb_error_to_string)
    }
}

impl RadrootsNostrEventSink for RadrootsNostrdbEventSinkAdapter {
    fn ingest_event(&self, event: &RadrootsNostrEvent) -> Result<(), String> {
        self.nostrdb
            .ingest_event(event, self.source.clone())
            .map_err(nostrdb_error_to_string)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RadrootsNostrdbConfig;
    use radroots_nostr::prelude::{RadrootsNostrEventBuilder, RadrootsNostrKeys};
    use tempfile::TempDir;

    #[test]
    fn runtime_adapter_accepts_signed_events() {
        let tmp_dir = TempDir::new().expect("tempdir should open");
        let db_dir = tmp_dir.path().join("nostrdb");
        let config = RadrootsNostrdbConfig::new(&db_dir);
        let nostrdb = RadrootsNostrdb::open(config).expect("database should open");
        let adapter = RadrootsNostrdbEventSinkAdapter::new(nostrdb);

        let keys = RadrootsNostrKeys::generate();
        let event = RadrootsNostrEventBuilder::text_note("hello from runtime adapter")
            .sign_with_keys(&keys)
            .expect("event should sign");

        adapter
            .ingest_event(&event)
            .expect("adapter should ingest event");
    }

    #[test]
    fn runtime_adapter_can_be_boxed_as_sink_trait() {
        let tmp_dir = TempDir::new().expect("tempdir should open");
        let db_dir = tmp_dir.path().join("nostrdb");
        let config = RadrootsNostrdbConfig::new(&db_dir);
        let nostrdb = RadrootsNostrdb::open(config).expect("database should open");
        let sink = RadrootsNostrdbEventSinkAdapter::new(nostrdb)
            .with_source(RadrootsNostrdbIngestSource::relay("wss://radroots.org"))
            .into_event_sink();

        let keys = RadrootsNostrKeys::generate();
        let event = RadrootsNostrEventBuilder::text_note("hello trait object")
            .sign_with_keys(&keys)
            .expect("event should sign");

        sink.ingest_event(&event)
            .expect("boxed sink should ingest event");
    }

    #[test]
    fn nostrdb_can_be_boxed_as_sink_trait() {
        let tmp_dir = TempDir::new().expect("tempdir should open");
        let db_dir = tmp_dir.path().join("nostrdb");
        let config = RadrootsNostrdbConfig::new(&db_dir);
        let nostrdb = RadrootsNostrdb::open(config).expect("database should open");
        let sink: Arc<dyn RadrootsNostrEventSink> = Arc::new(nostrdb.clone());

        let keys = RadrootsNostrKeys::generate();
        let event = RadrootsNostrEventBuilder::text_note("hello nostrdb trait object")
            .sign_with_keys(&keys)
            .expect("event should sign");

        sink.ingest_event(&event)
            .expect("nostrdb trait object should ingest event");
    }

    #[test]
    fn runtime_adapter_error_to_string_converts() {
        let rendered = nostrdb_error_to_string(crate::error::RadrootsNostrdbError::Nostrdb(
            "nostrdb error".to_string(),
        ));
        assert_eq!(rendered, "nostrdb error: nostrdb error");
    }
}

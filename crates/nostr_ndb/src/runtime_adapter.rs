use crate::ingest::RadrootsNostrNdbIngestSource;
use crate::ndb::RadrootsNostrNdb;
use radroots_nostr::prelude::RadrootsNostrEvent;
use radroots_nostr_runtime::prelude::RadrootsNostrEventSink;
use std::sync::Arc;

#[derive(Clone)]
pub struct RadrootsNostrNdbEventSinkAdapter {
    ndb: RadrootsNostrNdb,
    source: RadrootsNostrNdbIngestSource,
}

fn ndb_error_to_string(source: crate::error::RadrootsNostrNdbError) -> String {
    source.to_string()
}

impl RadrootsNostrNdbEventSinkAdapter {
    pub fn new(ndb: RadrootsNostrNdb) -> Self {
        Self {
            ndb,
            source: RadrootsNostrNdbIngestSource::client(),
        }
    }

    pub fn with_source(mut self, source: RadrootsNostrNdbIngestSource) -> Self {
        self.source = source;
        self
    }

    pub fn into_event_sink(self) -> Arc<dyn RadrootsNostrEventSink> {
        Arc::new(self)
    }
}

impl RadrootsNostrEventSink for RadrootsNostrNdb {
    fn ingest_event(&self, event: &RadrootsNostrEvent) -> Result<(), String> {
        RadrootsNostrNdb::ingest_event(self, event, RadrootsNostrNdbIngestSource::client())
            .map_err(ndb_error_to_string)
    }
}

impl RadrootsNostrEventSink for RadrootsNostrNdbEventSinkAdapter {
    fn ingest_event(&self, event: &RadrootsNostrEvent) -> Result<(), String> {
        self.ndb
            .ingest_event(event, self.source.clone())
            .map_err(ndb_error_to_string)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RadrootsNostrNdbConfig;
    use radroots_nostr::prelude::{RadrootsNostrEventBuilder, RadrootsNostrKeys};
    use tempfile::TempDir;

    #[test]
    fn runtime_adapter_accepts_signed_events() {
        let tmp_dir = TempDir::new().expect("tempdir should open");
        let db_dir = tmp_dir.path().join("ndb");
        let config = RadrootsNostrNdbConfig::new(&db_dir);
        let ndb = RadrootsNostrNdb::open(config).expect("database should open");
        let adapter = RadrootsNostrNdbEventSinkAdapter::new(ndb);

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
        let db_dir = tmp_dir.path().join("ndb");
        let config = RadrootsNostrNdbConfig::new(&db_dir);
        let ndb = RadrootsNostrNdb::open(config).expect("database should open");
        let sink = RadrootsNostrNdbEventSinkAdapter::new(ndb)
            .with_source(RadrootsNostrNdbIngestSource::relay("wss://radroots.org"))
            .into_event_sink();

        let keys = RadrootsNostrKeys::generate();
        let event = RadrootsNostrEventBuilder::text_note("hello trait object")
            .sign_with_keys(&keys)
            .expect("event should sign");

        sink.ingest_event(&event)
            .expect("boxed sink should ingest event");
    }

    #[test]
    fn ndb_can_be_boxed_as_sink_trait() {
        let tmp_dir = TempDir::new().expect("tempdir should open");
        let db_dir = tmp_dir.path().join("ndb");
        let config = RadrootsNostrNdbConfig::new(&db_dir);
        let ndb = RadrootsNostrNdb::open(config).expect("database should open");
        let sink: Arc<dyn RadrootsNostrEventSink> = Arc::new(ndb.clone());

        let keys = RadrootsNostrKeys::generate();
        let event = RadrootsNostrEventBuilder::text_note("hello ndb trait object")
            .sign_with_keys(&keys)
            .expect("event should sign");

        sink.ingest_event(&event)
            .expect("ndb trait object should ingest event");
    }

    #[test]
    fn runtime_adapter_error_to_string_converts() {
        let rendered = ndb_error_to_string(crate::error::RadrootsNostrNdbError::Ndb(
            "ndb error".to_string(),
        ));
        assert_eq!(rendered, "nostrdb error: ndb error");
    }
}

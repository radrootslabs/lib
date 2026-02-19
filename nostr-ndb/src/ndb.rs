use crate::config::RadrootsNostrNdbConfig;
use crate::error::RadrootsNostrNdbError;
use crate::ingest::RadrootsNostrNdbIngestSource;
use crate::subscription::{
    RadrootsNostrNdbNoteKey, RadrootsNostrNdbSubscriptionHandle, RadrootsNostrNdbSubscriptionSpec,
    RadrootsNostrNdbSubscriptionStream,
};
use radroots_nostr::prelude::RadrootsNostrEvent;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct RadrootsNostrNdb {
    db_dir: std::path::PathBuf,
    pub(crate) inner: nostrdb::Ndb,
}

impl RadrootsNostrNdb {
    pub fn open(config: RadrootsNostrNdbConfig) -> Result<Self, RadrootsNostrNdbError> {
        let mut inner_config = nostrdb::Config::new().skip_validation(config.skip_validation());
        if let Some(mapsize_bytes) = config.mapsize_bytes() {
            inner_config = inner_config.set_mapsize(mapsize_bytes);
        }
        if let Some(ingester_threads) = config.ingester_threads() {
            inner_config = inner_config.set_ingester_threads(ingester_threads);
        }

        let db_dir = config.db_dir().to_path_buf();
        let db_dir_str = db_dir.to_str().ok_or(RadrootsNostrNdbError::NonUtf8Path)?;
        let inner = nostrdb::Ndb::new(db_dir_str, &inner_config)?;

        Ok(Self { db_dir, inner })
    }

    pub fn db_dir(&self) -> &Path {
        &self.db_dir
    }

    pub fn ingest_event_json_with_source(
        &self,
        json: &str,
        source: RadrootsNostrNdbIngestSource,
    ) -> Result<(), RadrootsNostrNdbError> {
        let metadata = source.to_ndb_metadata();
        self.inner.process_event_with(json, metadata)?;
        Ok(())
    }

    pub fn ingest_event_json(&self, json: &str) -> Result<(), RadrootsNostrNdbError> {
        self.ingest_event_json_with_source(json, RadrootsNostrNdbIngestSource::default())
    }

    pub fn ingest_event(
        &self,
        event: &RadrootsNostrEvent,
        source: RadrootsNostrNdbIngestSource,
    ) -> Result<(), RadrootsNostrNdbError> {
        let json = serde_json::to_string(event)
            .map_err(|source| RadrootsNostrNdbError::EventJsonEncode(source.to_string()))?;
        self.ingest_event_json_with_source(json.as_str(), source)
    }

    pub fn subscribe(
        &self,
        spec: &RadrootsNostrNdbSubscriptionSpec,
    ) -> Result<RadrootsNostrNdbSubscriptionHandle, RadrootsNostrNdbError> {
        let filters = spec
            .filters()
            .iter()
            .map(|filter_spec| filter_spec.to_ndb_filter())
            .collect::<Result<Vec<_>, _>>()?;
        let subscription = self.inner.subscribe(filters.as_slice())?;
        Ok(RadrootsNostrNdbSubscriptionHandle::new(subscription.id()))
    }

    pub fn unsubscribe(
        &self,
        handle: RadrootsNostrNdbSubscriptionHandle,
    ) -> Result<(), RadrootsNostrNdbError> {
        let mut inner = self.inner.clone();
        inner.unsubscribe(nostrdb::Subscription::new(handle.id()))?;
        Ok(())
    }

    pub fn poll_for_note_keys(
        &self,
        handle: RadrootsNostrNdbSubscriptionHandle,
        max_notes: u32,
    ) -> Vec<RadrootsNostrNdbNoteKey> {
        self.inner
            .poll_for_notes(nostrdb::Subscription::new(handle.id()), max_notes)
            .into_iter()
            .map(|note_key| RadrootsNostrNdbNoteKey::new(note_key.as_u64()))
            .collect()
    }

    #[cfg(feature = "rt")]
    pub async fn wait_for_note_keys(
        &self,
        handle: RadrootsNostrNdbSubscriptionHandle,
        max_notes: u32,
    ) -> Result<Vec<RadrootsNostrNdbNoteKey>, RadrootsNostrNdbError> {
        let note_keys = self
            .inner
            .wait_for_notes(nostrdb::Subscription::new(handle.id()), max_notes)
            .await?;
        Ok(note_keys
            .into_iter()
            .map(|note_key| RadrootsNostrNdbNoteKey::new(note_key.as_u64()))
            .collect())
    }

    #[cfg(feature = "rt")]
    pub fn subscription_stream(
        &self,
        handle: RadrootsNostrNdbSubscriptionHandle,
        notes_per_await: u32,
    ) -> RadrootsNostrNdbSubscriptionStream {
        let stream = nostrdb::Subscription::new(handle.id())
            .stream(&self.inner)
            .notes_per_await(notes_per_await.max(1));
        RadrootsNostrNdbSubscriptionStream { inner: stream }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::RadrootsNostrNdbFilterSpec;
    use crate::ingest::RadrootsNostrNdbIngestSource;
    use radroots_nostr::prelude::{RadrootsNostrEventBuilder, RadrootsNostrKeys};
    use std::time::Duration;
    use tempfile::TempDir;

    #[test]
    fn config_builder_tracks_values() {
        let config = RadrootsNostrNdbConfig::new("target/testdbs/nostr_ndb_config")
            .with_mapsize_bytes(1024 * 1024)
            .with_ingester_threads(2)
            .with_skip_validation(true);

        assert_eq!(config.mapsize_bytes(), Some(1024 * 1024));
        assert_eq!(config.ingester_threads(), Some(2));
        assert!(config.skip_validation());
    }

    #[test]
    fn open_creates_database() {
        let tmp_dir = TempDir::new().expect("tempdir should open");
        let db_dir = tmp_dir.path().join("ndb");
        let config = RadrootsNostrNdbConfig::new(&db_dir)
            .with_mapsize_bytes(64 * 1024 * 1024)
            .with_ingester_threads(1);

        let ndb = RadrootsNostrNdb::open(config).expect("database should open");
        assert_eq!(ndb.db_dir(), db_dir.as_path());
        assert!(db_dir.exists());
    }

    #[test]
    fn ingest_source_builders_track_origin() {
        assert_eq!(
            RadrootsNostrNdbIngestSource::default(),
            RadrootsNostrNdbIngestSource::client()
        );
        assert_eq!(
            RadrootsNostrNdbIngestSource::relay("wss://relay.radroots.org"),
            RadrootsNostrNdbIngestSource::Relay {
                relay_url: Some("wss://relay.radroots.org".into())
            }
        );
        assert_eq!(
            RadrootsNostrNdbIngestSource::relay_unknown(),
            RadrootsNostrNdbIngestSource::Relay { relay_url: None }
        );
    }

    #[test]
    fn ingest_event_accepts_signed_note() {
        let tmp_dir = TempDir::new().expect("tempdir should open");
        let db_dir = tmp_dir.path().join("ndb");
        let config = RadrootsNostrNdbConfig::new(&db_dir);
        let ndb = RadrootsNostrNdb::open(config).expect("database should open");

        let keys = RadrootsNostrKeys::generate();
        let event = RadrootsNostrEventBuilder::text_note("hello from ndb")
            .sign_with_keys(&keys)
            .expect("event should sign");

        ndb.ingest_event(&event, RadrootsNostrNdbIngestSource::client())
            .expect("ingest should succeed");
    }

    #[test]
    fn subscribe_poll_and_unsubscribe_round_trip() {
        let tmp_dir = TempDir::new().expect("tempdir should open");
        let db_dir = tmp_dir.path().join("ndb");
        let config = RadrootsNostrNdbConfig::new(&db_dir);
        let ndb = RadrootsNostrNdb::open(config).expect("database should open");
        let spec = RadrootsNostrNdbSubscriptionSpec::single(
            RadrootsNostrNdbFilterSpec::new()
                .with_kind(1)
                .with_limit(10),
        );
        let handle = ndb.subscribe(&spec).expect("subscribe should succeed");

        let keys = RadrootsNostrKeys::generate();
        let event = RadrootsNostrEventBuilder::text_note("subscription test")
            .sign_with_keys(&keys)
            .expect("event should sign");
        ndb.ingest_event(&event, RadrootsNostrNdbIngestSource::relay_unknown())
            .expect("ingest should succeed");

        let mut notes = Vec::new();
        for _ in 0..40 {
            notes = ndb.poll_for_note_keys(handle, 32);
            if !notes.is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }

        assert!(!notes.is_empty());
        ndb.unsubscribe(handle).expect("unsubscribe should succeed");
    }

    #[tokio::test]
    async fn wait_for_note_keys_yields_results() {
        let tmp_dir = TempDir::new().expect("tempdir should open");
        let db_dir = tmp_dir.path().join("ndb");
        let config = RadrootsNostrNdbConfig::new(&db_dir);
        let ndb = RadrootsNostrNdb::open(config).expect("database should open");
        let spec = RadrootsNostrNdbSubscriptionSpec::text_notes(Some(10), None);
        let handle = ndb.subscribe(&spec).expect("subscribe should succeed");

        let keys = RadrootsNostrKeys::generate();
        let event = RadrootsNostrEventBuilder::text_note("wait test")
            .sign_with_keys(&keys)
            .expect("event should sign");
        ndb.ingest_event(&event, RadrootsNostrNdbIngestSource::relay_unknown())
            .expect("ingest should succeed");

        let notes = ndb
            .wait_for_note_keys(handle, 32)
            .await
            .expect("wait should succeed");
        assert!(!notes.is_empty());
    }
}

use crate::config::RadrootsNostrNdbConfig;
use crate::error::RadrootsNostrNdbError;
use crate::filter::parse_hex_32;
use crate::ingest::RadrootsNostrNdbIngestSource;
use crate::query::{RadrootsNostrNdbNote, RadrootsNostrNdbProfile, RadrootsNostrNdbQuerySpec};
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

fn map_profile_lookup_result<T>(
    result: Result<T, nostrdb::Error>,
) -> Result<Option<T>, RadrootsNostrNdbError> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(nostrdb::Error::NotFound) => Ok(None),
        Err(source) => Err(source.into()),
    }
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
        let json = serde_json::to_string(event)?;
        self.ingest_event_json_with_source(json.as_str(), source)
    }

    #[cfg(feature = "giftwrap")]
    pub fn add_giftwrap_secret_key(&self, secret_key: [u8; 32]) -> bool {
        self.inner.add_key(&secret_key)
    }

    #[cfg(feature = "giftwrap")]
    pub fn add_giftwrap_secret_key_hex(
        &self,
        secret_key_hex: &str,
    ) -> Result<bool, RadrootsNostrNdbError> {
        let secret_key = parse_hex_32(secret_key_hex, "secret_key")?;
        Ok(self.add_giftwrap_secret_key(secret_key))
    }

    #[cfg(feature = "giftwrap")]
    pub fn process_giftwraps(&self) -> Result<(), RadrootsNostrNdbError> {
        let txn = nostrdb::Transaction::new(&self.inner)?;
        self.inner.process_giftwraps(&txn);
        Ok(())
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

    pub fn query_notes(
        &self,
        spec: &RadrootsNostrNdbQuerySpec,
    ) -> Result<Vec<RadrootsNostrNdbNote>, RadrootsNostrNdbError> {
        if spec.filters().is_empty() {
            return Ok(Vec::new());
        }

        let filters = spec
            .filters()
            .iter()
            .map(|filter_spec| filter_spec.to_ndb_filter())
            .collect::<Result<Vec<_>, _>>()?;
        let txn = nostrdb::Transaction::new(&self.inner)?;
        let query_results =
            self.inner
                .query(&txn, filters.as_slice(), spec.max_results() as i32)?;

        query_results
            .into_iter()
            .map(|query_result| {
                let note = query_result.note;
                let json = note.json()?;
                Ok(RadrootsNostrNdbNote {
                    note_key: query_result.note_key.as_u64(),
                    id_hex: hex::encode(note.id()),
                    author_hex: hex::encode(note.pubkey()),
                    kind: note.kind(),
                    created_at_unix: note.created_at(),
                    content: note.content().to_owned(),
                    json,
                })
            })
            .collect::<Result<Vec<_>, nostrdb::Error>>()
            .map_err(Into::into)
    }

    pub fn get_profile_by_pubkey_hex(
        &self,
        pubkey_hex: &str,
    ) -> Result<Option<RadrootsNostrNdbProfile>, RadrootsNostrNdbError> {
        let pubkey = parse_hex_32(pubkey_hex, "pubkey")?;
        let txn = nostrdb::Transaction::new(&self.inner)?;
        let Some(profile_record) =
            map_profile_lookup_result(self.inner.get_profile_by_pubkey(&txn, &pubkey))?
        else {
            return Ok(None);
        };

        let profile = profile_record.record().profile();
        let profile_key = profile_record.key().map(|key| key.as_u64());
        Ok(profile.map(|profile| RadrootsNostrNdbProfile {
            profile_key,
            pubkey_hex: pubkey_hex.to_owned(),
            name: profile.name().map(ToOwned::to_owned),
            display_name: profile.display_name().map(ToOwned::to_owned),
            about: profile.about().map(ToOwned::to_owned),
            picture: profile.picture().map(ToOwned::to_owned),
            banner: profile.banner().map(ToOwned::to_owned),
            website: profile.website().map(ToOwned::to_owned),
            nip05: profile.nip05().map(ToOwned::to_owned),
            lud16: profile.lud16().map(ToOwned::to_owned),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::RadrootsNostrNdbFilterSpec;
    use crate::ingest::RadrootsNostrNdbIngestSource;
    use crate::query::RadrootsNostrNdbQuerySpec;
    use futures::StreamExt;
    use radroots_nostr::prelude::{RadrootsNostrEventBuilder, RadrootsNostrKeys};
    use radroots_nostr::prelude::{RadrootsNostrMetadata, radroots_nostr_build_metadata_event};
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
    fn map_profile_lookup_result_handles_all_error_kinds() {
        let success = map_profile_lookup_result::<u64>(Ok(7)).expect("ok");
        assert_eq!(success, Some(7));

        let not_found =
            map_profile_lookup_result::<u64>(Err(nostrdb::Error::NotFound)).expect("none");
        assert!(not_found.is_none());

        let query_error = map_profile_lookup_result::<u64>(Err(nostrdb::Error::QueryError));
        assert!(matches!(query_error, Err(RadrootsNostrNdbError::Ndb(_))));
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
    fn ingest_event_json_accepts_signed_note() {
        let tmp_dir = TempDir::new().expect("tempdir should open");
        let db_dir = tmp_dir.path().join("ndb");
        let config = RadrootsNostrNdbConfig::new(&db_dir);
        let ndb = RadrootsNostrNdb::open(config).expect("database should open");

        let keys = RadrootsNostrKeys::generate();
        let event = RadrootsNostrEventBuilder::text_note("hello from ndb json")
            .sign_with_keys(&keys)
            .expect("event should sign");
        let json = serde_json::to_string(&event).expect("event json");

        ndb.ingest_event_json(&json)
            .expect("json ingest should succeed");
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

    #[test]
    fn query_notes_returns_ingested_results() {
        let tmp_dir = TempDir::new().expect("tempdir should open");
        let db_dir = tmp_dir.path().join("ndb");
        let config = RadrootsNostrNdbConfig::new(&db_dir);
        let ndb = RadrootsNostrNdb::open(config).expect("database should open");

        let keys = RadrootsNostrKeys::generate();
        let event = RadrootsNostrEventBuilder::text_note("query note")
            .sign_with_keys(&keys)
            .expect("event should sign");
        ndb.ingest_event(&event, RadrootsNostrNdbIngestSource::client())
            .expect("ingest should succeed");

        let query_spec = RadrootsNostrNdbQuerySpec::text_notes(Some(50), None, 50);
        let mut notes = Vec::new();
        for _ in 0..40 {
            notes = ndb.query_notes(&query_spec).expect("query should succeed");
            if !notes.is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        assert!(!notes.is_empty());
        let note_pairs = notes
            .iter()
            .map(|note| (note.id_hex.clone(), note.content.clone()))
            .collect::<Vec<_>>();
        assert!(note_pairs.contains(&(event.id.to_hex(), "query note".to_string())));
    }

    #[test]
    fn query_notes_empty_filters_returns_empty() {
        let tmp_dir = TempDir::new().expect("tempdir should open");
        let db_dir = tmp_dir.path().join("ndb");
        let config = RadrootsNostrNdbConfig::new(&db_dir);
        let ndb = RadrootsNostrNdb::open(config).expect("database should open");

        let query_spec = RadrootsNostrNdbQuerySpec::new(Vec::new(), 10);
        let notes = ndb.query_notes(&query_spec).expect("query should succeed");
        assert!(notes.is_empty());
    }

    #[test]
    fn profile_lookup_returns_metadata_fields() {
        let tmp_dir = TempDir::new().expect("tempdir should open");
        let db_dir = tmp_dir.path().join("ndb");
        let config = RadrootsNostrNdbConfig::new(&db_dir);
        let ndb = RadrootsNostrNdb::open(config).expect("database should open");

        let keys = RadrootsNostrKeys::generate();
        let pubkey_hex = keys.public_key().to_hex();
        let metadata = RadrootsNostrMetadata::new()
            .name("alice")
            .display_name("Alice")
            .about("coffee operator")
            .lud16("alice@example.com");
        let metadata_event = radroots_nostr_build_metadata_event(&metadata)
            .sign_with_keys(&keys)
            .expect("metadata event should sign");
        ndb.ingest_event(&metadata_event, RadrootsNostrNdbIngestSource::client())
            .expect("ingest should succeed");

        let mut profile = None;
        for _ in 0..40 {
            profile = ndb
                .get_profile_by_pubkey_hex(pubkey_hex.as_str())
                .expect("profile lookup should succeed");
            if profile.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        let profile = profile.expect("profile should exist");
        assert_eq!(profile.pubkey_hex, pubkey_hex);
        assert_eq!(profile.name.as_deref(), Some("alice"));
        assert_eq!(profile.display_name.as_deref(), Some("Alice"));
        assert_eq!(profile.lud16.as_deref(), Some("alice@example.com"));
    }

    #[test]
    fn profile_lookup_returns_none_when_missing() {
        let tmp_dir = TempDir::new().expect("tempdir should open");
        let db_dir = tmp_dir.path().join("ndb");
        let config = RadrootsNostrNdbConfig::new(&db_dir);
        let ndb = RadrootsNostrNdb::open(config).expect("database should open");

        let pubkey_hex = RadrootsNostrKeys::generate().public_key().to_hex();
        let profile = ndb
            .get_profile_by_pubkey_hex(pubkey_hex.as_str())
            .expect("profile lookup");
        assert!(profile.is_none());
    }

    #[test]
    fn profile_lookup_returns_none_without_metadata_record() {
        let tmp_dir = TempDir::new().expect("tempdir should open");
        let db_dir = tmp_dir.path().join("ndb");
        let config = RadrootsNostrNdbConfig::new(&db_dir);
        let ndb = RadrootsNostrNdb::open(config).expect("database should open");

        let keys = RadrootsNostrKeys::generate();
        let pubkey_hex = keys.public_key().to_hex();
        let event = RadrootsNostrEventBuilder::text_note("non profile event")
            .sign_with_keys(&keys)
            .expect("event should sign");
        ndb.ingest_event(&event, RadrootsNostrNdbIngestSource::client())
            .expect("ingest should succeed");

        let profile = ndb
            .get_profile_by_pubkey_hex(pubkey_hex.as_str())
            .expect("profile lookup");
        assert!(profile.is_none());
    }

    #[test]
    fn profile_lookup_invalid_metadata_content_returns_none() {
        let tmp_dir = TempDir::new().expect("tempdir should open");
        let db_dir = tmp_dir.path().join("ndb");
        let config = RadrootsNostrNdbConfig::new(&db_dir);
        let ndb = RadrootsNostrNdb::open(config).expect("database should open");

        let keys = RadrootsNostrKeys::generate();
        let pubkey_hex = keys.public_key().to_hex();
        let event = RadrootsNostrEventBuilder::new(
            radroots_nostr::prelude::RadrootsNostrKind::Metadata,
            "not valid metadata json",
        )
        .sign_with_keys(&keys)
        .expect("event should sign");
        ndb.ingest_event(&event, RadrootsNostrNdbIngestSource::client())
            .expect("ingest should succeed");

        let result = ndb.get_profile_by_pubkey_hex(pubkey_hex.as_str());
        assert!(result.expect("profile lookup").is_none());
    }

    #[test]
    fn subscribe_rejects_invalid_author_hex() {
        let tmp_dir = TempDir::new().expect("tempdir should open");
        let db_dir = tmp_dir.path().join("ndb");
        let config = RadrootsNostrNdbConfig::new(&db_dir);
        let ndb = RadrootsNostrNdb::open(config).expect("database should open");

        let spec = RadrootsNostrNdbSubscriptionSpec::single(
            RadrootsNostrNdbFilterSpec::new().with_author_hex("not-hex"),
        );
        let err = ndb.subscribe(&spec).expect_err("subscribe should fail");
        assert!(matches!(
            err,
            RadrootsNostrNdbError::InvalidHex {
                field: "author",
                ..
            }
        ));
    }

    #[test]
    fn profile_lookup_rejects_invalid_pubkey_length() {
        let tmp_dir = TempDir::new().expect("tempdir should open");
        let db_dir = tmp_dir.path().join("ndb");
        let config = RadrootsNostrNdbConfig::new(&db_dir);
        let ndb = RadrootsNostrNdb::open(config).expect("database should open");

        let err = ndb
            .get_profile_by_pubkey_hex("abcd")
            .expect_err("lookup should fail");
        assert!(matches!(
            err,
            RadrootsNostrNdbError::InvalidHexLength {
                field: "pubkey",
                ..
            }
        ));
    }

    #[tokio::test]
    async fn subscription_stream_yields_events() {
        let tmp_dir = TempDir::new().expect("tempdir should open");
        let db_dir = tmp_dir.path().join("ndb");
        let config = RadrootsNostrNdbConfig::new(&db_dir);
        let ndb = RadrootsNostrNdb::open(config).expect("database should open");
        let spec = RadrootsNostrNdbSubscriptionSpec::text_notes(Some(10), None);
        let handle = ndb.subscribe(&spec).expect("subscribe should succeed");
        let mut stream = ndb.subscription_stream(handle, 0);

        let pending = tokio::time::timeout(Duration::from_millis(20), stream.next()).await;
        assert!(pending.is_err());

        let keys = RadrootsNostrKeys::generate();
        let event = RadrootsNostrEventBuilder::text_note("stream note")
            .sign_with_keys(&keys)
            .expect("event should sign");
        ndb.ingest_event(&event, RadrootsNostrNdbIngestSource::client())
            .expect("ingest should succeed");

        let note_keys = tokio::time::timeout(Duration::from_secs(2), stream.next())
            .await
            .expect("stream should wake")
            .expect("stream should yield note keys");
        assert!(!note_keys.is_empty());
        assert!(note_keys.iter().all(|key| key.as_u64() > 0));
    }

    #[test]
    fn concurrent_ingest_handles_parallel_writers() {
        let tmp_dir = TempDir::new().expect("tempdir should open");
        let db_dir = tmp_dir.path().join("ndb");
        let config = RadrootsNostrNdbConfig::new(&db_dir);
        let ndb = RadrootsNostrNdb::open(config).expect("database should open");

        let worker_count = 4usize;
        let notes_per_worker = 20usize;
        let mut handles = Vec::new();

        for worker in 0..worker_count {
            let db = ndb.clone();
            handles.push(std::thread::spawn(move || {
                let keys = RadrootsNostrKeys::generate();
                for idx in 0..notes_per_worker {
                    let content = format!("parallel-{worker}-{idx}");
                    let event = RadrootsNostrEventBuilder::text_note(content.as_str())
                        .sign_with_keys(&keys)
                        .expect("event should sign");
                    db.ingest_event(&event, RadrootsNostrNdbIngestSource::client())
                        .expect("ingest should succeed");
                }
            }));
        }

        for handle in handles {
            handle.join().expect("worker should complete");
        }

        let query_spec = RadrootsNostrNdbQuerySpec::text_notes(Some(512), None, 512);
        let expected = worker_count * notes_per_worker;
        let mut observed = 0usize;
        let mut break_threshold = expected + 1;

        for _ in 0..80 {
            let notes = ndb.query_notes(&query_spec).expect("query should succeed");
            observed = notes
                .iter()
                .filter(|note| note.content.starts_with("parallel-"))
                .count();
            if observed >= break_threshold {
                break;
            }
            break_threshold = expected;
            std::thread::sleep(Duration::from_millis(25));
        }

        assert!(
            observed >= expected,
            "expected at least {expected} parallel notes, got {observed}"
        );
    }

    #[cfg(feature = "giftwrap")]
    #[test]
    fn giftwrap_secret_key_hex_validates_length() {
        let tmp_dir = TempDir::new().expect("tempdir should open");
        let db_dir = tmp_dir.path().join("ndb");
        let config = RadrootsNostrNdbConfig::new(&db_dir);
        let ndb = RadrootsNostrNdb::open(config).expect("database should open");

        let result = ndb.add_giftwrap_secret_key_hex("abcd");
        assert!(matches!(
            result,
            Err(RadrootsNostrNdbError::InvalidHexLength {
                field: "secret_key",
                ..
            })
        ));
    }

    #[cfg(feature = "giftwrap")]
    #[test]
    fn giftwrap_process_flow_executes() {
        let tmp_dir = TempDir::new().expect("tempdir should open");
        let db_dir = tmp_dir.path().join("ndb");
        let config = RadrootsNostrNdbConfig::new(&db_dir);
        let ndb = RadrootsNostrNdb::open(config).expect("database should open");

        let secret_key = [7u8; 32];
        let _ = ndb.add_giftwrap_secret_key(secret_key);
        ndb.process_giftwraps()
            .expect("giftwrap processing should run");
    }
}

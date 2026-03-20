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

#[cfg(test)]
mod test_hooks {
    use std::sync::atomic::{AtomicBool, Ordering};

    pub static FORCE_EVENT_JSON_ERROR: AtomicBool = AtomicBool::new(false);
    pub static FORCE_PROCESS_EVENT_ERROR: AtomicBool = AtomicBool::new(false);
    pub static FORCE_SUBSCRIBE_ERROR: AtomicBool = AtomicBool::new(false);
    pub static FORCE_UNSUBSCRIBE_ERROR: AtomicBool = AtomicBool::new(false);
    pub static FORCE_WAIT_ERROR: AtomicBool = AtomicBool::new(false);
    pub static FORCE_TRANSACTION_ERROR: AtomicBool = AtomicBool::new(false);
    pub static FORCE_QUERY_ERROR: AtomicBool = AtomicBool::new(false);
    pub static FORCE_NOTE_JSON_ERROR: AtomicBool = AtomicBool::new(false);
    pub static FORCE_PROFILE_QUERY_ERROR: AtomicBool = AtomicBool::new(false);

    pub fn take(flag: &AtomicBool) -> bool {
        flag.swap(false, Ordering::SeqCst)
    }
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
    fn serialize_event(event: &RadrootsNostrEvent) -> Result<String, RadrootsNostrNdbError> {
        #[cfg(test)]
        if test_hooks::take(&test_hooks::FORCE_EVENT_JSON_ERROR) {
            return Err(RadrootsNostrNdbError::EventJsonEncode(
                "forced event json error".into(),
            ));
        }
        serde_json::to_string(event).map_err(Into::into)
    }

    fn process_event_with_inner(
        &self,
        json: &str,
        metadata: nostrdb::IngestMetadata,
    ) -> Result<(), RadrootsNostrNdbError> {
        #[cfg(test)]
        if test_hooks::take(&test_hooks::FORCE_PROCESS_EVENT_ERROR) {
            return Err(RadrootsNostrNdbError::Ndb(
                "forced process event error".into(),
            ));
        }
        self.inner
            .process_event_with(json, metadata)
            .map_err(Into::into)
    }

    fn subscribe_inner(
        &self,
        filters: &[nostrdb::Filter],
    ) -> Result<nostrdb::Subscription, RadrootsNostrNdbError> {
        #[cfg(test)]
        if test_hooks::take(&test_hooks::FORCE_SUBSCRIBE_ERROR) {
            return Err(RadrootsNostrNdbError::Ndb("forced subscribe error".into()));
        }
        self.inner.subscribe(filters).map_err(Into::into)
    }

    fn unsubscribe_inner(
        &self,
        subscription: nostrdb::Subscription,
    ) -> Result<(), RadrootsNostrNdbError> {
        #[cfg(test)]
        if test_hooks::take(&test_hooks::FORCE_UNSUBSCRIBE_ERROR) {
            return Err(RadrootsNostrNdbError::Ndb(
                "forced unsubscribe error".into(),
            ));
        }
        let mut inner = self.inner.clone();
        inner.unsubscribe(subscription).map_err(Into::into)
    }

    #[cfg(feature = "rt")]
    async fn wait_for_notes_inner(
        &self,
        subscription: nostrdb::Subscription,
        max_notes: u32,
    ) -> Result<Vec<nostrdb::NoteKey>, RadrootsNostrNdbError> {
        #[cfg(test)]
        if test_hooks::take(&test_hooks::FORCE_WAIT_ERROR) {
            return Err(RadrootsNostrNdbError::Ndb("forced wait error".into()));
        }
        self.inner
            .wait_for_notes(subscription, max_notes)
            .await
            .map_err(Into::into)
    }

    fn open_txn(&self) -> Result<nostrdb::Transaction, RadrootsNostrNdbError> {
        #[cfg(test)]
        if test_hooks::take(&test_hooks::FORCE_TRANSACTION_ERROR) {
            return Err(RadrootsNostrNdbError::Ndb(
                "forced transaction error".into(),
            ));
        }
        nostrdb::Transaction::new(&self.inner).map_err(Into::into)
    }

    fn query_inner<'a>(
        &self,
        txn: &'a nostrdb::Transaction,
        filters: &[nostrdb::Filter],
        max_results: i32,
    ) -> Result<Vec<nostrdb::QueryResult<'a>>, RadrootsNostrNdbError> {
        #[cfg(test)]
        if test_hooks::take(&test_hooks::FORCE_QUERY_ERROR) {
            return Err(RadrootsNostrNdbError::Ndb("forced query error".into()));
        }
        self.inner
            .query(txn, filters, max_results)
            .map_err(Into::into)
    }

    fn note_json_value(note: &nostrdb::Note) -> Result<String, RadrootsNostrNdbError> {
        #[cfg(test)]
        if test_hooks::take(&test_hooks::FORCE_NOTE_JSON_ERROR) {
            return Err(RadrootsNostrNdbError::Ndb("forced note json error".into()));
        }
        note.json().map_err(Into::into)
    }

    fn get_profile_record<'a>(
        &self,
        txn: &'a nostrdb::Transaction,
        pubkey: &[u8; 32],
    ) -> Result<Option<nostrdb::ProfileRecord<'a>>, RadrootsNostrNdbError> {
        #[cfg(test)]
        if test_hooks::take(&test_hooks::FORCE_PROFILE_QUERY_ERROR) {
            return map_profile_lookup_result(Err(nostrdb::Error::QueryError));
        }
        map_profile_lookup_result(self.inner.get_profile_by_pubkey(txn, pubkey))
    }

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
        self.process_event_with_inner(json, metadata)?;
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
        let json = Self::serialize_event(event)?;
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
        let subscription = self.subscribe_inner(filters.as_slice())?;
        Ok(RadrootsNostrNdbSubscriptionHandle::new(subscription.id()))
    }

    pub fn unsubscribe(
        &self,
        handle: RadrootsNostrNdbSubscriptionHandle,
    ) -> Result<(), RadrootsNostrNdbError> {
        let subscription = nostrdb::Subscription::new(handle.id());
        self.unsubscribe_inner(subscription)?;
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
            .wait_for_notes_inner(nostrdb::Subscription::new(handle.id()), max_notes)
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
        let txn = self.open_txn()?;
        let query_results =
            self.query_inner(&txn, filters.as_slice(), spec.max_results() as i32)?;

        query_results
            .into_iter()
            .map(|query_result| {
                let note = query_result.note;
                let json = Self::note_json_value(&note)?;
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
            .collect::<Result<Vec<_>, RadrootsNostrNdbError>>()
    }

    pub fn get_profile_by_pubkey_hex(
        &self,
        pubkey_hex: &str,
    ) -> Result<Option<RadrootsNostrNdbProfile>, RadrootsNostrNdbError> {
        let pubkey = parse_hex_32(pubkey_hex, "pubkey")?;
        let txn = self.open_txn()?;
        let Some(profile_record) = self.get_profile_record(&txn, &pubkey)? else {
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
    use std::sync::atomic::Ordering;
    use std::sync::{Mutex, OnceLock};
    use std::time::Duration;
    use tempfile::TempDir;

    fn test_hooks_lock() -> &'static Mutex<()> {
        static TEST_HOOKS_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        TEST_HOOKS_LOCK.get_or_init(|| Mutex::new(()))
    }

    fn test_hooks_guard() -> std::sync::MutexGuard<'static, ()> {
        test_hooks_lock().lock().expect("test hooks lock")
    }

    fn reset_test_flags() {
        test_hooks::FORCE_EVENT_JSON_ERROR.store(false, Ordering::SeqCst);
        test_hooks::FORCE_PROCESS_EVENT_ERROR.store(false, Ordering::SeqCst);
        test_hooks::FORCE_SUBSCRIBE_ERROR.store(false, Ordering::SeqCst);
        test_hooks::FORCE_UNSUBSCRIBE_ERROR.store(false, Ordering::SeqCst);
        test_hooks::FORCE_WAIT_ERROR.store(false, Ordering::SeqCst);
        test_hooks::FORCE_TRANSACTION_ERROR.store(false, Ordering::SeqCst);
        test_hooks::FORCE_QUERY_ERROR.store(false, Ordering::SeqCst);
        test_hooks::FORCE_NOTE_JSON_ERROR.store(false, Ordering::SeqCst);
        test_hooks::FORCE_PROFILE_QUERY_ERROR.store(false, Ordering::SeqCst);
    }

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

        let query_error = map_profile_lookup_result::<u64>(Err(nostrdb::Error::QueryError))
            .expect_err("query error");
        assert!(query_error.to_string().starts_with("nostrdb error:"));
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

    #[cfg(unix)]
    #[test]
    fn open_rejects_non_utf8_path() {
        use std::os::unix::ffi::OsStrExt;

        let path = std::path::PathBuf::from(std::ffi::OsStr::from_bytes(b"ndb-\xFF"));
        let config = RadrootsNostrNdbConfig::new(&path);
        let err = RadrootsNostrNdb::open(config).expect_err("non utf8 path");
        assert!(err.to_string().contains("utf-8"));
    }

    #[test]
    fn open_reports_ndb_error_for_file_path() {
        let tmp_dir = TempDir::new().expect("tempdir should open");
        let db_dir = tmp_dir.path().join("ndb");
        std::fs::write(&db_dir, "not a directory").expect("write db file");
        let config = RadrootsNostrNdbConfig::new(&db_dir);
        let err = RadrootsNostrNdb::open(config).expect_err("file path should fail");
        assert!(err.to_string().starts_with("nostrdb error:"));
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
    fn ingest_event_json_rejects_invalid_json() {
        let _guard = test_hooks_guard();
        reset_test_flags();
        let tmp_dir = TempDir::new().expect("tempdir should open");
        let db_dir = tmp_dir.path().join("ndb");
        let config = RadrootsNostrNdbConfig::new(&db_dir);
        let ndb = RadrootsNostrNdb::open(config).expect("database should open");

        test_hooks::FORCE_PROCESS_EVENT_ERROR.store(true, Ordering::SeqCst);
        let err = ndb
            .ingest_event_json_with_source("not json", RadrootsNostrNdbIngestSource::client())
            .expect_err("process event error");
        assert!(err.to_string().starts_with("nostrdb error:"));
    }

    #[test]
    fn ingest_event_reports_event_json_error() {
        let _guard = test_hooks_guard();
        reset_test_flags();
        let tmp_dir = TempDir::new().expect("tempdir should open");
        let db_dir = tmp_dir.path().join("ndb");
        let config = RadrootsNostrNdbConfig::new(&db_dir);
        let ndb = RadrootsNostrNdb::open(config).expect("database should open");

        let keys = RadrootsNostrKeys::generate();
        let event = RadrootsNostrEventBuilder::text_note("forced json error")
            .sign_with_keys(&keys)
            .expect("event should sign");
        test_hooks::FORCE_EVENT_JSON_ERROR.store(true, Ordering::SeqCst);

        let err = ndb
            .ingest_event(&event, RadrootsNostrNdbIngestSource::client())
            .expect_err("forced json error");
        assert!(err.to_string().starts_with("event json encode failed:"));
    }

    #[test]
    fn subscribe_poll_and_unsubscribe_round_trip() {
        let _guard = test_hooks_guard();
        reset_test_flags();
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

    #[test]
    fn subscribe_reports_ndb_error() {
        let _guard = test_hooks_guard();
        reset_test_flags();
        let tmp_dir = TempDir::new().expect("tempdir should open");
        let db_dir = tmp_dir.path().join("ndb");
        let config = RadrootsNostrNdbConfig::new(&db_dir);
        let ndb = RadrootsNostrNdb::open(config).expect("database should open");
        let spec = RadrootsNostrNdbSubscriptionSpec::text_notes(Some(10), None);
        test_hooks::FORCE_SUBSCRIBE_ERROR.store(true, Ordering::SeqCst);

        let err = ndb.subscribe(&spec).expect_err("forced subscribe error");
        assert!(err.to_string().starts_with("nostrdb error:"));
    }

    #[test]
    fn unsubscribe_reports_ndb_error() {
        let _guard = test_hooks_guard();
        reset_test_flags();
        let tmp_dir = TempDir::new().expect("tempdir should open");
        let db_dir = tmp_dir.path().join("ndb");
        let config = RadrootsNostrNdbConfig::new(&db_dir);
        let ndb = RadrootsNostrNdb::open(config).expect("database should open");
        let spec = RadrootsNostrNdbSubscriptionSpec::text_notes(Some(10), None);
        let handle = ndb.subscribe(&spec).expect("subscribe should succeed");
        test_hooks::FORCE_UNSUBSCRIBE_ERROR.store(true, Ordering::SeqCst);

        let err = ndb
            .unsubscribe(handle)
            .expect_err("forced unsubscribe error");
        assert!(err.to_string().starts_with("nostrdb error:"));
    }

    #[tokio::test]
    async fn wait_for_note_keys_yields_results() {
        let _guard = test_hooks_guard();
        reset_test_flags();
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

    #[tokio::test]
    async fn wait_for_note_keys_reports_ndb_error() {
        let _guard = test_hooks_guard();
        reset_test_flags();
        let tmp_dir = TempDir::new().expect("tempdir should open");
        let db_dir = tmp_dir.path().join("ndb");
        let config = RadrootsNostrNdbConfig::new(&db_dir);
        let ndb = RadrootsNostrNdb::open(config).expect("database should open");
        let spec = RadrootsNostrNdbSubscriptionSpec::text_notes(Some(10), None);
        let handle = ndb.subscribe(&spec).expect("subscribe should succeed");
        test_hooks::FORCE_WAIT_ERROR.store(true, Ordering::SeqCst);

        let err = ndb
            .wait_for_note_keys(handle, 1)
            .await
            .expect_err("forced wait error");
        assert!(err.to_string().starts_with("nostrdb error:"));
    }

    #[test]
    fn query_notes_returns_ingested_results() {
        let _guard = test_hooks_guard();
        reset_test_flags();
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
        let _guard = test_hooks_guard();
        reset_test_flags();
        let tmp_dir = TempDir::new().expect("tempdir should open");
        let db_dir = tmp_dir.path().join("ndb");
        let config = RadrootsNostrNdbConfig::new(&db_dir);
        let ndb = RadrootsNostrNdb::open(config).expect("database should open");

        let query_spec = RadrootsNostrNdbQuerySpec::new(Vec::new(), 10);
        let notes = ndb.query_notes(&query_spec).expect("query should succeed");
        assert!(notes.is_empty());
    }

    #[test]
    fn query_notes_rejects_invalid_filters() {
        let _guard = test_hooks_guard();
        reset_test_flags();
        let tmp_dir = TempDir::new().expect("tempdir should open");
        let db_dir = tmp_dir.path().join("ndb");
        let config = RadrootsNostrNdbConfig::new(&db_dir);
        let ndb = RadrootsNostrNdb::open(config).expect("database should open");

        let spec = RadrootsNostrNdbQuerySpec::single(
            RadrootsNostrNdbFilterSpec::new().with_author_hex("not-hex"),
            10,
        );
        let err = ndb.query_notes(&spec).expect_err("invalid filter");
        assert!(err.to_string().contains("invalid hex"));
    }

    #[test]
    fn query_notes_reports_transaction_error() {
        let _guard = test_hooks_guard();
        reset_test_flags();
        let tmp_dir = TempDir::new().expect("tempdir should open");
        let db_dir = tmp_dir.path().join("ndb");
        let config = RadrootsNostrNdbConfig::new(&db_dir);
        let ndb = RadrootsNostrNdb::open(config).expect("database should open");
        let spec = RadrootsNostrNdbQuerySpec::text_notes(Some(10), None, 10);
        test_hooks::FORCE_TRANSACTION_ERROR.store(true, Ordering::SeqCst);

        let err = ndb
            .query_notes(&spec)
            .expect_err("forced transaction error");
        assert!(err.to_string().starts_with("nostrdb error:"));
    }

    #[test]
    fn query_notes_reports_query_error() {
        let _guard = test_hooks_guard();
        reset_test_flags();
        let tmp_dir = TempDir::new().expect("tempdir should open");
        let db_dir = tmp_dir.path().join("ndb");
        let config = RadrootsNostrNdbConfig::new(&db_dir);
        let ndb = RadrootsNostrNdb::open(config).expect("database should open");
        let spec = RadrootsNostrNdbQuerySpec::text_notes(Some(10), None, 10);
        test_hooks::FORCE_QUERY_ERROR.store(true, Ordering::SeqCst);

        let err = ndb.query_notes(&spec).expect_err("forced query error");
        assert!(err.to_string().starts_with("nostrdb error:"));
    }

    #[test]
    fn query_notes_reports_note_json_error() {
        let _guard = test_hooks_guard();
        reset_test_flags();
        let tmp_dir = TempDir::new().expect("tempdir should open");
        let db_dir = tmp_dir.path().join("ndb");
        let config = RadrootsNostrNdbConfig::new(&db_dir);
        let ndb = RadrootsNostrNdb::open(config).expect("database should open");

        let keys = RadrootsNostrKeys::generate();
        let event = RadrootsNostrEventBuilder::text_note("note json error")
            .sign_with_keys(&keys)
            .expect("event should sign");
        ndb.ingest_event(&event, RadrootsNostrNdbIngestSource::client())
            .expect("ingest should succeed");

        let query_spec = RadrootsNostrNdbQuerySpec::text_notes(Some(50), None, 50);
        for _ in 0..40 {
            let notes = ndb.query_notes(&query_spec).expect("query should succeed");
            if !notes.is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        test_hooks::FORCE_NOTE_JSON_ERROR.store(true, Ordering::SeqCst);

        let err = ndb
            .query_notes(&query_spec)
            .expect_err("forced note json error");
        assert!(err.to_string().starts_with("nostrdb error:"));
    }

    #[test]
    fn profile_lookup_returns_metadata_fields() {
        let _guard = test_hooks_guard();
        reset_test_flags();
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
        let _guard = test_hooks_guard();
        reset_test_flags();
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
    fn profile_lookup_reports_query_error() {
        let _guard = test_hooks_guard();
        reset_test_flags();
        let tmp_dir = TempDir::new().expect("tempdir should open");
        let db_dir = tmp_dir.path().join("ndb");
        let config = RadrootsNostrNdbConfig::new(&db_dir);
        let ndb = RadrootsNostrNdb::open(config).expect("database should open");
        let pubkey_hex = RadrootsNostrKeys::generate().public_key().to_hex();
        test_hooks::FORCE_PROFILE_QUERY_ERROR.store(true, Ordering::SeqCst);

        let err = ndb
            .get_profile_by_pubkey_hex(pubkey_hex.as_str())
            .expect_err("forced profile query error");
        assert!(err.to_string().starts_with("nostrdb error:"));
    }

    #[test]
    fn profile_lookup_reports_transaction_error() {
        let _guard = test_hooks_guard();
        reset_test_flags();
        let tmp_dir = TempDir::new().expect("tempdir should open");
        let db_dir = tmp_dir.path().join("ndb");
        let config = RadrootsNostrNdbConfig::new(&db_dir);
        let ndb = RadrootsNostrNdb::open(config).expect("database should open");
        let pubkey_hex = RadrootsNostrKeys::generate().public_key().to_hex();
        test_hooks::FORCE_TRANSACTION_ERROR.store(true, Ordering::SeqCst);

        let err = ndb
            .get_profile_by_pubkey_hex(pubkey_hex.as_str())
            .expect_err("forced transaction error");
        assert!(err.to_string().starts_with("nostrdb error:"));
    }

    #[test]
    fn profile_lookup_returns_none_without_metadata_record() {
        let _guard = test_hooks_guard();
        reset_test_flags();
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
        let _guard = test_hooks_guard();
        reset_test_flags();
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
        assert!(err.to_string().contains("invalid hex for author"));
    }

    #[test]
    fn profile_lookup_rejects_invalid_pubkey_length() {
        let _guard = test_hooks_guard();
        reset_test_flags();
        let tmp_dir = TempDir::new().expect("tempdir should open");
        let db_dir = tmp_dir.path().join("ndb");
        let config = RadrootsNostrNdbConfig::new(&db_dir);
        let ndb = RadrootsNostrNdb::open(config).expect("database should open");

        let err = ndb
            .get_profile_by_pubkey_hex("abcd")
            .expect_err("lookup should fail");
        assert!(err.to_string().contains("invalid hex length for pubkey"));
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
        let _guard = test_hooks_guard();
        reset_test_flags();
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
        let err = result.expect_err("invalid giftwrap key");
        assert!(
            err.to_string()
                .contains("invalid hex length for secret_key")
        );
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

#[cfg(feature = "runtime")]
use crate::private_store::{SDK_PRIVATE_STORE_SCHEMA_VERSION, SdkPrivateStore};
#[cfg(feature = "runtime")]
use crate::{
    DvmClient, FarmsClient, GeoNamesClient, ListingsClient, MarketClient, RadrootsGeoNamesConfig,
    RadrootsSdkError, SdkRelayTargetSet, SdkRelayUrlPolicy, SyncClient, TradesClient,
};
#[cfg(all(feature = "runtime", feature = "signer-adapters"))]
use crate::{
    RadrootsSdkSignReceipt, RadrootsSdkSignRequest, RadrootsSdkSignerProvider,
    RadrootsSdkSignerStatus,
};
#[cfg(feature = "runtime")]
use radroots_event_store::RadrootsEventStore;
#[cfg(feature = "runtime")]
use radroots_outbox::RadrootsOutbox;
#[cfg(feature = "runtime")]
use sqlx::SqlitePool;
#[cfg(feature = "runtime")]
use std::{
    fs,
    io::ErrorKind,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(all(feature = "runtime", feature = "radrootsd-proxy"))]
use crate::adapters::radrootsd::RadrootsdProxyConfig;

#[cfg(feature = "runtime")]
const SDK_STORAGE_MANIFEST_VERSION: u16 = 1;
#[cfg(feature = "runtime")]
const SDK_STORAGE_MANIFEST_KIND: SdkBackupManifestKind = SdkBackupManifestKind::StorageBackup;
#[cfg(feature = "runtime")]
const SDK_EVENT_STORE_SCHEMA_VERSION: i64 = 1;
#[cfg(feature = "runtime")]
const SDK_OUTBOX_SCHEMA_VERSION: i64 = 1;
#[cfg(feature = "runtime")]
const SDK_PRIVATE_STORE_SCHEMA_VERSION_CURRENT: i64 = SDK_PRIVATE_STORE_SCHEMA_VERSION;
#[cfg(feature = "runtime")]
const EVENT_STORE_BACKUP_FILE: &str = "event_store.sqlite";
#[cfg(feature = "runtime")]
const OUTBOX_BACKUP_FILE: &str = "outbox.sqlite";
#[cfg(feature = "runtime")]
const PRIVATE_STORE_BACKUP_FILE: &str = "private.sqlite";
#[cfg(feature = "runtime")]
const BACKUP_MANIFEST_FILE: &str = "manifest.json";

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
pub enum RadrootsSdkStorageConfig {
    Memory,
    Directory(PathBuf),
}

#[cfg(feature = "runtime")]
impl Default for RadrootsSdkStorageConfig {
    fn default() -> Self {
        Self::Memory
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
pub struct RadrootsSdkTimestamp(u64);

#[cfg(feature = "runtime")]
impl RadrootsSdkTimestamp {
    pub fn from_unix_seconds(seconds: u64) -> Self {
        Self(seconds)
    }

    pub fn unix_seconds(self) -> u64 {
        self.0
    }

    pub fn try_into_nostr_created_at(self) -> Result<u32, RadrootsSdkError> {
        u32::try_from(self.0).map_err(|_| RadrootsSdkError::TimestampOutOfRange { value: self.0 })
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RadrootsSdkClock {
    System,
    Fixed(RadrootsSdkTimestamp),
    #[cfg(test)]
    BeforeUnixEpoch,
}

#[cfg(feature = "runtime")]
impl Default for RadrootsSdkClock {
    fn default() -> Self {
        Self::System
    }
}

#[cfg(feature = "runtime")]
impl RadrootsSdkClock {
    pub fn now(&self) -> Result<RadrootsSdkTimestamp, RadrootsSdkError> {
        match self {
            Self::System => sdk_timestamp_from_system_time(SystemTime::now()),
            Self::Fixed(timestamp) => Ok(*timestamp),
            #[cfg(test)]
            Self::BeforeUnixEpoch => Err(RadrootsSdkError::ClockBeforeUnixEpoch),
        }
    }
}

#[cfg(feature = "runtime")]
fn sdk_timestamp_from_system_time(
    time: SystemTime,
) -> Result<RadrootsSdkTimestamp, RadrootsSdkError> {
    let duration = time
        .duration_since(UNIX_EPOCH)
        .map_err(|_| RadrootsSdkError::ClockBeforeUnixEpoch)?;
    Ok(RadrootsSdkTimestamp::from_unix_seconds(duration.as_secs()))
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RadrootsSdkStoragePaths {
    pub event_store_path: PathBuf,
    pub outbox_path: PathBuf,
    pub private_store_path: PathBuf,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub struct StorageStatusRequest {}

#[cfg(feature = "runtime")]
impl StorageStatusRequest {
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StorageStatusReceipt {
    pub storage: SdkStorageKind,
    pub paths: Option<RadrootsSdkStoragePaths>,
    pub event_store: SdkEventStoreStorageStatus,
    pub outbox: SdkOutboxStorageStatus,
    pub private_store: SdkPrivateStoreStorageStatus,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SdkStorageKind {
    Memory,
    Directory,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SdkSqliteStoreStatus {
    pub schema_version: i64,
    pub journal_mode: String,
    pub foreign_keys_enabled: bool,
    pub busy_timeout_ms: i64,
    pub integrity_ok: bool,
    pub integrity_result: String,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SdkEventStoreStorageStatus {
    pub store: SdkSqliteStoreStatus,
    pub total_events: i64,
    pub projection_eligible_events: i64,
    pub relay_observations: i64,
    pub last_event_seq: Option<i64>,
    pub last_event_updated_at_ms: Option<i64>,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SdkOutboxStorageStatus {
    pub store: SdkSqliteStoreStatus,
    pub total_events: i64,
    pub pending_events: i64,
    pub retryable_events: i64,
    pub terminal_events: i64,
    pub failed_terminal_events: i64,
    pub ready_signed_events: i64,
    pub publishing_events: i64,
    pub last_attempt_at_ms: Option<i64>,
    pub last_error: Option<String>,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SdkPrivateStoreStorageStatus {
    pub store: SdkSqliteStoreStatus,
    pub farm_private_locations: i64,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub struct BackupRequest {
    pub destination: PathBuf,
    pub overwrite: bool,
}

#[cfg(feature = "runtime")]
impl BackupRequest {
    pub fn new(destination: impl Into<PathBuf>) -> Self {
        Self {
            destination: destination.into(),
            overwrite: false,
        }
    }

    pub fn with_overwrite(mut self, overwrite: bool) -> Self {
        self.overwrite = overwrite;
        self
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BackupReceipt {
    pub destination: PathBuf,
    pub state: SdkBackupState,
    pub event_store_path: Option<PathBuf>,
    pub outbox_path: Option<PathBuf>,
    pub private_store_path: Option<PathBuf>,
    pub manifest_path: Option<PathBuf>,
    pub manifest: SdkBackupManifest,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SdkBackupState {
    Planned,
    Completed,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SdkBackupManifestKind {
    StorageBackup,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SdkBackupManifest {
    pub manifest_kind: SdkBackupManifestKind,
    pub manifest_version: u16,
    pub sdk_version: String,
    pub created_at_ms: i64,
    pub source_storage: SdkStorageKind,
    pub source_paths: Option<RadrootsSdkStoragePaths>,
    pub backup_paths: RadrootsSdkStoragePaths,
    pub source_status: StorageStatusReceipt,
    pub backup_verification: SdkBackupVerification,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SdkBackupVerification {
    pub event_store_ok: bool,
    pub outbox_ok: bool,
    pub private_store_ok: bool,
    pub event_store_events: i64,
    pub outbox_events: i64,
    pub private_farm_locations: i64,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub struct IntegrityRequest {}

#[cfg(feature = "runtime")]
impl IntegrityRequest {
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IntegrityReceipt {
    pub checked_paths: Vec<PathBuf>,
    pub event_store_ok: bool,
    pub outbox_ok: bool,
    pub private_store_ok: bool,
    pub event_store_result: String,
    pub outbox_result: String,
    pub private_store_result: String,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
pub struct RestoreRequest {
    pub source: PathBuf,
    pub destination: Option<PathBuf>,
    pub overwrite: bool,
    pub dry_run: bool,
}

#[cfg(feature = "runtime")]
impl RestoreRequest {
    pub fn new(source: impl Into<PathBuf>) -> Self {
        Self {
            source: source.into(),
            destination: None,
            overwrite: false,
            dry_run: false,
        }
    }

    pub fn with_destination(mut self, destination: impl Into<PathBuf>) -> Self {
        self.destination = Some(destination.into());
        self
    }

    pub fn with_overwrite(mut self, overwrite: bool) -> Self {
        self.overwrite = overwrite;
        self
    }

    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    pub fn dry_run(self) -> Self {
        self.with_dry_run(true)
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SdkRestoreState {
    Validated,
    DryRun,
    Completed,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SdkPublishTransport {
    DirectNostrRelay,
    #[cfg(feature = "radrootsd-proxy")]
    RadrootsdProxy(RadrootsdProxyConfig),
}

#[cfg(feature = "runtime")]
impl Default for SdkPublishTransport {
    fn default() -> Self {
        Self::DirectNostrRelay
    }
}

#[cfg(feature = "runtime")]
impl SdkPublishTransport {
    pub(crate) fn supports_delegated_relay_resolution(&self) -> bool {
        match self {
            Self::DirectNostrRelay => false,
            #[cfg(feature = "radrootsd-proxy")]
            Self::RadrootsdProxy(_) => true,
        }
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct RestoreArchive {
    pub source: PathBuf,
    pub event_store_path: PathBuf,
    pub outbox_path: PathBuf,
    pub private_store_path: PathBuf,
    pub manifest_path: PathBuf,
    pub manifest: SdkBackupManifest,
    pub verification: SdkBackupVerification,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct RestoreReceipt {
    pub source: PathBuf,
    pub destination: Option<PathBuf>,
    pub state: SdkRestoreState,
    pub destination_paths: Option<RadrootsSdkStoragePaths>,
    pub event_store_path: PathBuf,
    pub outbox_path: PathBuf,
    pub private_store_path: PathBuf,
    pub manifest_path: PathBuf,
    pub manifest: SdkBackupManifest,
    pub verification: SdkBackupVerification,
    pub restored_paths: Option<RadrootsSdkStoragePaths>,
}

#[cfg(feature = "runtime")]
#[derive(Clone)]
pub struct RadrootsClientBuilder {
    storage: RadrootsSdkStorageConfig,
    geonames: Option<RadrootsGeoNamesConfig>,
    clock: RadrootsSdkClock,
    relay_urls: Vec<String>,
    relay_url_policy: SdkRelayUrlPolicy,
    publish_transport: SdkPublishTransport,
    #[cfg(feature = "signer-adapters")]
    signer_provider: Option<RadrootsSdkSignerProvider>,
}

#[cfg(feature = "runtime")]
impl Default for RadrootsClientBuilder {
    fn default() -> Self {
        Self {
            storage: RadrootsSdkStorageConfig::Memory,
            geonames: None,
            clock: RadrootsSdkClock::System,
            relay_urls: Vec::new(),
            relay_url_policy: SdkRelayUrlPolicy::Public,
            publish_transport: SdkPublishTransport::DirectNostrRelay,
            #[cfg(feature = "signer-adapters")]
            signer_provider: None,
        }
    }
}

#[cfg(feature = "runtime")]
impl RadrootsClientBuilder {
    pub fn storage(mut self, storage: RadrootsSdkStorageConfig) -> Self {
        self.storage = storage;
        self
    }

    pub fn directory_storage(mut self, path: impl Into<PathBuf>) -> Self {
        self.storage = RadrootsSdkStorageConfig::Directory(path.into());
        self
    }

    pub fn geonames_config(mut self, geonames: RadrootsGeoNamesConfig) -> Self {
        self.geonames = Some(geonames);
        self
    }

    pub fn geonames_cache_root(mut self, cache_root: impl Into<PathBuf>) -> Self {
        self.geonames = Some(RadrootsGeoNamesConfig::new(cache_root));
        self
    }

    pub fn clock(mut self, clock: RadrootsSdkClock) -> Self {
        self.clock = clock;
        self
    }

    pub fn fixed_clock(mut self, timestamp: RadrootsSdkTimestamp) -> Self {
        self.clock = RadrootsSdkClock::Fixed(timestamp);
        self
    }

    pub fn relay_url(mut self, relay_url: impl Into<String>) -> Self {
        self.relay_urls.push(relay_url.into());
        self
    }

    pub fn relay_url_policy(mut self, policy: SdkRelayUrlPolicy) -> Self {
        self.relay_url_policy = policy;
        self
    }

    pub fn publish_transport(mut self, transport: SdkPublishTransport) -> Self {
        self.publish_transport = transport;
        self
    }

    #[cfg(feature = "signer-adapters")]
    pub fn signer_provider(mut self, signer_provider: RadrootsSdkSignerProvider) -> Self {
        self.signer_provider = Some(signer_provider);
        self
    }

    pub async fn build(self) -> Result<RadrootsClient, RadrootsSdkError> {
        let storage = open_storage(&self.storage).await?;
        let relay_urls =
            SdkRelayTargetSet::from_configured_relays(&self.relay_urls, self.relay_url_policy)?;
        Ok(RadrootsClient {
            _event_store: storage.event_store,
            _outbox: storage.outbox,
            _private_store: storage.private_store,
            storage_paths: storage.paths,
            geonames: self.geonames,
            clock: self.clock,
            relay_urls,
            publish_transport: self.publish_transport,
            #[cfg(feature = "signer-adapters")]
            signer_provider: self.signer_provider,
        })
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone)]
pub struct RadrootsClient {
    pub(crate) _event_store: RadrootsEventStore,
    pub(crate) _outbox: RadrootsOutbox,
    pub(crate) _private_store: SdkPrivateStore,
    storage_paths: Option<RadrootsSdkStoragePaths>,
    geonames: Option<RadrootsGeoNamesConfig>,
    clock: RadrootsSdkClock,
    relay_urls: Vec<String>,
    publish_transport: SdkPublishTransport,
    #[cfg(feature = "signer-adapters")]
    signer_provider: Option<RadrootsSdkSignerProvider>,
}

#[cfg(feature = "runtime")]
impl RadrootsClient {
    pub fn builder() -> RadrootsClientBuilder {
        RadrootsClientBuilder::default()
    }

    pub fn farms(&self) -> FarmsClient<'_> {
        FarmsClient::new(self)
    }

    pub fn listings(&self) -> ListingsClient<'_> {
        ListingsClient::new(self)
    }

    pub fn market(&self) -> MarketClient<'_> {
        MarketClient::new(self)
    }

    pub fn geonames(&self) -> GeoNamesClient<'_> {
        GeoNamesClient::new(self)
    }

    pub fn trades(&self) -> TradesClient<'_> {
        TradesClient::new(self)
    }

    pub fn sync(&self) -> SyncClient<'_> {
        SyncClient::new(self)
    }

    pub fn dvm(&self) -> DvmClient<'_> {
        DvmClient::new(self)
    }

    pub fn now(&self) -> Result<RadrootsSdkTimestamp, RadrootsSdkError> {
        self.clock.now()
    }

    pub fn relay_urls(&self) -> &[String] {
        &self.relay_urls
    }

    pub fn publish_transport(&self) -> &SdkPublishTransport {
        &self.publish_transport
    }

    #[cfg(feature = "signer-adapters")]
    pub fn configured_signer(&self) -> Option<&RadrootsSdkSignerProvider> {
        self.signer_provider.as_ref()
    }

    #[cfg(feature = "signer-adapters")]
    pub fn signer_status(&self) -> Option<RadrootsSdkSignerStatus> {
        self.signer_provider
            .as_ref()
            .map(RadrootsSdkSignerProvider::status)
    }

    #[cfg(feature = "signer-adapters")]
    pub async fn sign_with_configured_signer(
        &self,
        request: RadrootsSdkSignRequest<'_>,
    ) -> Result<RadrootsSdkSignReceipt, RadrootsSdkError> {
        let signer =
            self.signer_provider
                .as_ref()
                .ok_or_else(|| RadrootsSdkError::SignerUnavailable {
                    mode: "configured".to_owned(),
                    reason: "no SDK signer provider is configured".to_owned(),
                })?;
        signer.sign(request).await
    }

    pub fn storage_paths(&self) -> Option<&RadrootsSdkStoragePaths> {
        self.storage_paths.as_ref()
    }

    pub fn geonames_config(&self) -> Option<&RadrootsGeoNamesConfig> {
        self.geonames.as_ref()
    }

    pub async fn storage_status(
        &self,
        _request: StorageStatusRequest,
    ) -> Result<StorageStatusReceipt, RadrootsSdkError> {
        let now_ms = sdk_now_ms(self)?;
        let event_store_status = event_store_sqlite_status(&self._event_store).await?;
        let outbox_store_status = outbox_sqlite_status(&self._outbox).await?;
        let private_store_status = private_store_sqlite_status(&self._private_store).await?;
        let event_summary = event_store_status_summary(&self._event_store).await?;
        let outbox_summary = outbox_status_summary(&self._outbox, now_ms).await?;
        let private_summary = self._private_store.status_summary().await?;
        Ok(StorageStatusReceipt {
            storage: self.storage_kind(),
            paths: self.storage_paths.clone(),
            event_store: SdkEventStoreStorageStatus {
                store: event_store_status,
                total_events: event_summary.total_events,
                projection_eligible_events: event_summary.projection_eligible_events,
                relay_observations: event_summary.relay_observations,
                last_event_seq: event_summary.last_event_seq,
                last_event_updated_at_ms: event_summary.last_event_updated_at_ms,
            },
            outbox: SdkOutboxStorageStatus {
                store: outbox_store_status,
                total_events: outbox_summary.total_events,
                pending_events: outbox_summary.pending_events,
                retryable_events: outbox_summary.retryable_events,
                terminal_events: outbox_summary.terminal_events,
                failed_terminal_events: outbox_summary.failed_terminal_events,
                ready_signed_events: outbox_summary.ready_signed_events,
                publishing_events: outbox_summary.publishing_events,
                last_attempt_at_ms: outbox_summary.last_attempt_at_ms,
                last_error: outbox_summary.last_error,
            },
            private_store: SdkPrivateStoreStorageStatus {
                store: private_store_status,
                farm_private_locations: private_summary.farm_private_locations,
            },
        })
    }

    pub async fn integrity(
        &self,
        _request: IntegrityRequest,
    ) -> Result<IntegrityReceipt, RadrootsSdkError> {
        let event_store_integrity = sqlite_integrity_result(self._event_store.pool()).await?;
        let outbox_integrity = sqlite_integrity_result(self._outbox.pool()).await?;
        let private_store_integrity =
            private_store_sqlite_integrity_result(self._private_store.pool()).await?;
        let checked_paths = self
            .storage_paths
            .as_ref()
            .map(|paths| {
                vec![
                    paths.event_store_path.clone(),
                    paths.outbox_path.clone(),
                    paths.private_store_path.clone(),
                ]
            })
            .unwrap_or_default();
        Ok(IntegrityReceipt {
            checked_paths,
            event_store_ok: event_store_integrity.ok,
            outbox_ok: outbox_integrity.ok,
            private_store_ok: private_store_integrity.ok,
            event_store_result: event_store_integrity.result,
            outbox_result: outbox_integrity.result,
            private_store_result: private_store_integrity.result,
        })
    }

    pub async fn backup(&self, request: BackupRequest) -> Result<BackupReceipt, RadrootsSdkError> {
        if request.destination.as_os_str().is_empty() {
            return Err(RadrootsSdkError::InvalidRequest {
                message: "backup destination must not be empty".to_owned(),
            });
        }
        prepare_backup_destination(&request.destination, request.overwrite)?;
        let created_at_ms = sdk_now_ms(self)?;
        let backup_paths = RadrootsSdkStoragePaths {
            event_store_path: request.destination.join(EVENT_STORE_BACKUP_FILE),
            outbox_path: request.destination.join(OUTBOX_BACKUP_FILE),
            private_store_path: request.destination.join(PRIVATE_STORE_BACKUP_FILE),
        };
        let manifest_backup_paths = RadrootsSdkStoragePaths {
            event_store_path: PathBuf::from(EVENT_STORE_BACKUP_FILE),
            outbox_path: PathBuf::from(OUTBOX_BACKUP_FILE),
            private_store_path: PathBuf::from(PRIVATE_STORE_BACKUP_FILE),
        };
        let manifest_path = request.destination.join(BACKUP_MANIFEST_FILE);
        let source_status = self.storage_status(StorageStatusRequest::new()).await?;
        let backup_verification = backup_sqlite_stores(
            self._event_store.pool(),
            self._outbox.pool(),
            self._private_store.pool(),
            &backup_paths,
        )
        .await?;
        let manifest = SdkBackupManifest {
            manifest_kind: SDK_STORAGE_MANIFEST_KIND,
            manifest_version: SDK_STORAGE_MANIFEST_VERSION,
            sdk_version: env!("CARGO_PKG_VERSION").to_owned(),
            created_at_ms,
            source_storage: self.storage_kind(),
            source_paths: self.storage_paths.clone(),
            backup_paths: manifest_backup_paths,
            source_status,
            backup_verification,
        };
        write_backup_receipt(request.destination, backup_paths, manifest_path, manifest)
    }

    fn storage_kind(&self) -> SdkStorageKind {
        if self.storage_paths.is_some() {
            SdkStorageKind::Directory
        } else {
            SdkStorageKind::Memory
        }
    }

    pub async fn inspect_restore_archive(
        source: impl Into<PathBuf>,
    ) -> Result<RestoreArchive, RadrootsSdkError> {
        inspect_restore_archive(source.into()).await
    }

    pub async fn restore(request: RestoreRequest) -> Result<RestoreReceipt, RadrootsSdkError> {
        let archive = inspect_restore_archive(request.source.clone()).await?;
        let destination =
            request
                .destination
                .clone()
                .ok_or_else(|| RadrootsSdkError::InvalidRequest {
                    message: "restore destination is required".to_owned(),
                })?;
        let destination_paths =
            preflight_restore_destination(&archive.source, &destination, request.overwrite)?;
        let restored_paths = if request.dry_run {
            None
        } else {
            Some(restore_archive_to_destination(&archive, &destination, &destination_paths).await?)
        };
        let state = if request.dry_run {
            SdkRestoreState::DryRun
        } else {
            SdkRestoreState::Completed
        };
        Ok(RestoreReceipt {
            source: archive.source,
            destination: Some(destination),
            state,
            destination_paths: Some(destination_paths),
            event_store_path: archive.event_store_path,
            outbox_path: archive.outbox_path,
            private_store_path: archive.private_store_path,
            manifest_path: archive.manifest_path,
            manifest: archive.manifest,
            verification: archive.verification,
            restored_paths,
        })
    }
}

#[cfg(feature = "runtime")]
async fn event_store_sqlite_status(
    event_store: &RadrootsEventStore,
) -> Result<SdkSqliteStoreStatus, RadrootsSdkError> {
    sqlite_store_status(
        event_store.pool(),
        SDK_EVENT_STORE_SCHEMA_VERSION,
        event_store.pragma_journal_mode().await?,
        event_store.pragma_foreign_keys().await? != 0,
        event_store.pragma_busy_timeout().await?,
    )
    .await
}

#[cfg(feature = "runtime")]
async fn outbox_sqlite_status(
    outbox: &RadrootsOutbox,
) -> Result<SdkSqliteStoreStatus, RadrootsSdkError> {
    sqlite_store_status(
        outbox.pool(),
        SDK_OUTBOX_SCHEMA_VERSION,
        outbox.pragma_journal_mode().await?,
        outbox.pragma_foreign_keys().await? != 0,
        outbox.pragma_busy_timeout().await?,
    )
    .await
}

#[cfg(feature = "runtime")]
async fn private_store_sqlite_status(
    private_store: &SdkPrivateStore,
) -> Result<SdkSqliteStoreStatus, RadrootsSdkError> {
    private_sqlite_store_status(
        private_store.pool(),
        SDK_PRIVATE_STORE_SCHEMA_VERSION_CURRENT,
        private_store.pragma_journal_mode().await?,
        private_store.pragma_foreign_keys().await? != 0,
        private_store.pragma_busy_timeout().await?,
    )
    .await
}

#[cfg(feature = "runtime")]
async fn event_store_status_summary(
    event_store: &RadrootsEventStore,
) -> Result<radroots_event_store::RadrootsEventStoreStatusSummary, RadrootsSdkError> {
    Ok(event_store.status_summary().await?)
}

#[cfg(feature = "runtime")]
async fn outbox_status_summary(
    outbox: &RadrootsOutbox,
    now_ms: i64,
) -> Result<radroots_outbox::RadrootsOutboxStatusSummary, RadrootsSdkError> {
    Ok(outbox.status_summary(now_ms).await?)
}

#[cfg(feature = "runtime")]
async fn backup_sqlite_stores(
    event_store_pool: &SqlitePool,
    outbox_pool: &SqlitePool,
    private_store_pool: &SqlitePool,
    backup_paths: &RadrootsSdkStoragePaths,
) -> Result<SdkBackupVerification, RadrootsSdkError> {
    sqlite_vacuum_into(
        event_store_pool,
        &backup_paths.event_store_path,
        "event store",
    )
    .await?;
    sqlite_vacuum_into(outbox_pool, &backup_paths.outbox_path, "outbox").await?;
    sqlite_vacuum_into(
        private_store_pool,
        &backup_paths.private_store_path,
        "private store",
    )
    .await?;
    verify_backup_paths(backup_paths).await
}

#[cfg(feature = "runtime")]
fn write_backup_receipt(
    destination: PathBuf,
    backup_paths: RadrootsSdkStoragePaths,
    manifest_path: PathBuf,
    manifest: SdkBackupManifest,
) -> Result<BackupReceipt, RadrootsSdkError> {
    write_backup_manifest(&manifest_path, &manifest)?;
    Ok(BackupReceipt {
        destination,
        state: SdkBackupState::Completed,
        event_store_path: Some(backup_paths.event_store_path),
        outbox_path: Some(backup_paths.outbox_path),
        private_store_path: Some(backup_paths.private_store_path),
        manifest_path: Some(manifest_path),
        manifest,
    })
}

#[cfg(feature = "runtime")]
pub(crate) fn sdk_now_ms(sdk: &RadrootsClient) -> Result<i64, RadrootsSdkError> {
    let seconds = sdk.now()?.unix_seconds();
    let millis = seconds
        .checked_mul(1_000)
        .ok_or(RadrootsSdkError::TimestampOutOfRange { value: seconds })?;
    i64::try_from(millis).map_err(|_| RadrootsSdkError::TimestampOutOfRange { value: seconds })
}

#[cfg(feature = "runtime")]
fn write_backup_manifest(
    manifest_path: &Path,
    manifest: &SdkBackupManifest,
) -> Result<(), RadrootsSdkError> {
    let manifest_json = serde_json::to_vec_pretty(manifest).expect("backup manifest serializes");
    fs::write(manifest_path, manifest_json).map_err(|error| RadrootsSdkError::Io {
        path: manifest_path.to_path_buf(),
        message: error.to_string(),
    })
}

#[cfg(feature = "runtime")]
async fn inspect_restore_archive(source: PathBuf) -> Result<RestoreArchive, RadrootsSdkError> {
    if source.as_os_str().is_empty() {
        return Err(RadrootsSdkError::InvalidRequest {
            message: "restore source must not be empty".to_owned(),
        });
    }
    let source_root = canonical_restore_directory(&source)?;
    let manifest_path = source.join(BACKUP_MANIFEST_FILE);
    let manifest_path = validate_restore_member_path(&source_root, &manifest_path, "manifest")?;
    let manifest_json = fs::read(&manifest_path).map_err(|error| RadrootsSdkError::Io {
        path: manifest_path.clone(),
        message: error.to_string(),
    })?;
    let manifest: SdkBackupManifest = serde_json::from_slice(&manifest_json).map_err(|error| {
        RadrootsSdkError::InvalidRequest {
            message: format!("restore manifest is invalid JSON: {error}"),
        }
    })?;
    validate_restore_manifest(&manifest)?;
    let event_store_path = restore_archive_member_path(
        &source_root,
        &manifest.backup_paths.event_store_path,
        "event store",
    )?;
    let outbox_path =
        restore_archive_member_path(&source_root, &manifest.backup_paths.outbox_path, "outbox")?;
    let private_store_path = restore_archive_member_path(
        &source_root,
        &manifest.backup_paths.private_store_path,
        "private store",
    )?;
    let verification = verify_backup_paths(&RadrootsSdkStoragePaths {
        event_store_path: event_store_path.clone(),
        outbox_path: outbox_path.clone(),
        private_store_path: private_store_path.clone(),
    })
    .await?;
    validate_restore_verification(&verification, &manifest.backup_verification)?;
    Ok(RestoreArchive {
        source,
        event_store_path,
        outbox_path,
        private_store_path,
        manifest_path,
        manifest,
        verification,
    })
}

#[cfg(feature = "runtime")]
fn canonical_restore_directory(path: &Path) -> Result<PathBuf, RadrootsSdkError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(RadrootsSdkError::InvalidRequest {
                message: "restore source must not be a symbolic link".to_owned(),
            })
        }
        Ok(metadata) if metadata.is_dir() => canonicalize_restore_path(path),
        Ok(_) => Err(RadrootsSdkError::InvalidRequest {
            message: "restore source must be a directory".to_owned(),
        }),
        Err(error) => Err(RadrootsSdkError::Io {
            path: path.to_path_buf(),
            message: error.to_string(),
        }),
    }
}

#[cfg(feature = "runtime")]
fn canonicalize_restore_path(path: &Path) -> Result<PathBuf, RadrootsSdkError> {
    fs::canonicalize(path).map_err(|error| RadrootsSdkError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}

#[cfg(feature = "runtime")]
fn validate_restore_member_path(
    source_root: &Path,
    path: &Path,
    label: &'static str,
) -> Result<PathBuf, RadrootsSdkError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| RadrootsSdkError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    if metadata.file_type().is_symlink() {
        return Err(RadrootsSdkError::InvalidRequest {
            message: format!("restore {label} must not be a symbolic link"),
        });
    }
    if !metadata.is_file() {
        return Err(RadrootsSdkError::InvalidRequest {
            message: format!("restore {label} must be a regular file"),
        });
    }
    let canonical_path = canonicalize_restore_path(path)?;
    if !canonical_path.starts_with(source_root) {
        return Err(RadrootsSdkError::InvalidRequest {
            message: format!("restore {label} must stay inside the backup directory"),
        });
    }
    Ok(canonical_path)
}

#[cfg(feature = "runtime")]
fn restore_archive_member_path(
    source_root: &Path,
    archive_path: &Path,
    label: &'static str,
) -> Result<PathBuf, RadrootsSdkError> {
    validate_relative_archive_path(archive_path, label)?;
    validate_restore_member_path(source_root, &source_root.join(archive_path), label)
}

#[cfg(feature = "runtime")]
fn validate_relative_archive_path(
    path: &Path,
    label: &'static str,
) -> Result<(), RadrootsSdkError> {
    if path.as_os_str().is_empty() {
        return Err(RadrootsSdkError::InvalidRequest {
            message: format!("restore {label} archive path must not be empty"),
        });
    }
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(RadrootsSdkError::InvalidRequest {
            message: format!("restore {label} archive path must be relative and contained"),
        });
    }
    Ok(())
}

#[cfg(feature = "runtime")]
fn validate_restore_manifest(manifest: &SdkBackupManifest) -> Result<(), RadrootsSdkError> {
    if manifest.manifest_version != SDK_STORAGE_MANIFEST_VERSION {
        return Err(RadrootsSdkError::InvalidRequest {
            message: format!(
                "restore manifest version {} is unsupported",
                manifest.manifest_version
            ),
        });
    }
    Ok(())
}

#[cfg(feature = "runtime")]
fn validate_restore_verification(
    actual: &SdkBackupVerification,
    manifest: &SdkBackupVerification,
) -> Result<(), RadrootsSdkError> {
    if !actual.event_store_ok || !actual.outbox_ok || !actual.private_store_ok {
        return Err(RadrootsSdkError::InvalidRequest {
            message: "restore backup stores failed integrity checks".to_owned(),
        });
    }
    if actual != manifest {
        return Err(RadrootsSdkError::InvalidRequest {
            message: "restore backup verification does not match manifest".to_owned(),
        });
    }
    Ok(())
}

#[cfg(feature = "runtime")]
fn preflight_restore_destination(
    source: &Path,
    destination: &Path,
    overwrite: bool,
) -> Result<RadrootsSdkStoragePaths, RadrootsSdkError> {
    if destination.as_os_str().is_empty() {
        return Err(RadrootsSdkError::InvalidRequest {
            message: "restore destination must not be empty".to_owned(),
        });
    }
    let source_root = canonical_restore_directory(source)?;
    match fs::symlink_metadata(destination) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(RadrootsSdkError::InvalidRequest {
                message: "restore destination must not be a symbolic link".to_owned(),
            });
        }
        Ok(metadata) if metadata.is_dir() => {
            let destination_root = canonicalize_restore_path(destination)?;
            reject_restore_destination_overlap(&source_root, &destination_root)?;
            let mut entries = fs::read_dir(destination).map_err(|error| RadrootsSdkError::Io {
                path: destination.to_path_buf(),
                message: error.to_string(),
            })?;
            let has_entries = entries
                .next()
                .transpose()
                .map_err(|error| RadrootsSdkError::Io {
                    path: destination.to_path_buf(),
                    message: error.to_string(),
                })?
                .is_some();
            if !overwrite && has_entries {
                return Err(RadrootsSdkError::InvalidRequest {
                    message: "restore destination already exists and overwrite is false".to_owned(),
                });
            }
        }
        Ok(metadata) if metadata.is_file() => {
            let destination_root = canonicalize_restore_path(destination)?;
            reject_restore_destination_overlap(&source_root, &destination_root)?;
            if !overwrite {
                return Err(RadrootsSdkError::InvalidRequest {
                    message: "restore destination already exists and overwrite is false".to_owned(),
                });
            }
        }
        Ok(_) => {
            return Err(RadrootsSdkError::InvalidRequest {
                message: "restore destination must be a directory path".to_owned(),
            });
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {
            let parent = destination
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            let parent_root = canonical_restore_directory(parent)?;
            let destination_name = destination.file_name().unwrap_or_default();
            reject_restore_destination_overlap(&source_root, &parent_root.join(destination_name))?;
        }
        Err(error) => {
            return Err(RadrootsSdkError::Io {
                path: destination.to_path_buf(),
                message: error.to_string(),
            });
        }
    }
    Ok(RadrootsSdkStoragePaths {
        event_store_path: destination.join(EVENT_STORE_BACKUP_FILE),
        outbox_path: destination.join(OUTBOX_BACKUP_FILE),
        private_store_path: destination.join(PRIVATE_STORE_BACKUP_FILE),
    })
}

#[cfg(feature = "runtime")]
fn reject_restore_destination_overlap(
    source_root: &Path,
    destination_root: &Path,
) -> Result<(), RadrootsSdkError> {
    if destination_root.starts_with(source_root) || source_root.starts_with(destination_root) {
        return Err(RadrootsSdkError::InvalidRequest {
            message: "restore destination must not overlap the backup source".to_owned(),
        });
    }
    Ok(())
}

#[cfg(feature = "runtime")]
async fn restore_archive_to_destination(
    archive: &RestoreArchive,
    destination: &Path,
    destination_paths: &RadrootsSdkStoragePaths,
) -> Result<RadrootsSdkStoragePaths, RadrootsSdkError> {
    let parent = destination
        .parent()
        .ok_or_else(|| RadrootsSdkError::InvalidRequest {
            message: "restore destination parent is required".to_owned(),
        })?;
    let staging = unique_restore_sidecar_path(parent, destination, "staging")?;
    let previous = unique_restore_sidecar_path(parent, destination, "previous")?;
    fs::create_dir(&staging).map_err(|error| RadrootsSdkError::Io {
        path: staging.clone(),
        message: error.to_string(),
    })?;
    let staging_paths = RadrootsSdkStoragePaths {
        event_store_path: staging.join(EVENT_STORE_BACKUP_FILE),
        outbox_path: staging.join(OUTBOX_BACKUP_FILE),
        private_store_path: staging.join(PRIVATE_STORE_BACKUP_FILE),
    };
    if let Err(error) = copy_restore_archive_to_staging(archive, &staging_paths).await {
        let _ = remove_existing_restore_path(&staging);
        return Err(error);
    }

    let previous_installed = install_restore_staging(&staging, destination, &previous)?;

    let destination_verification = verify_backup_paths(destination_paths).await;
    match destination_verification {
        Ok(verification) => {
            if let Err(error) = validate_restore_verification(&verification, &archive.verification)
            {
                rollback_restore_destination(destination, &previous, previous_installed);
                return Err(error);
            }
        }
        Err(error) => {
            rollback_restore_destination(destination, &previous, previous_installed);
            return Err(error);
        }
    }

    if previous_installed {
        remove_existing_restore_path(&previous)?;
    }
    Ok(destination_paths.clone())
}

#[cfg(feature = "runtime")]
fn install_restore_staging(
    staging: &Path,
    destination: &Path,
    previous: &Path,
) -> Result<bool, RadrootsSdkError> {
    let mut previous_installed = false;
    if fs::symlink_metadata(destination).is_ok() {
        rename_restore_path(destination, previous, "previous destination")?;
        previous_installed = true;
    }

    if let Err(error) = rename_restore_path(staging, destination, "staged restore") {
        if previous_installed {
            let _ = rename_restore_path(previous, destination, "previous destination rollback");
        }
        let _ = remove_existing_restore_path(staging);
        return Err(error);
    }
    Ok(previous_installed)
}

#[cfg(feature = "runtime")]
async fn copy_restore_archive_to_staging(
    archive: &RestoreArchive,
    staging_paths: &RadrootsSdkStoragePaths,
) -> Result<(), RadrootsSdkError> {
    copy_restore_file(
        &archive.event_store_path,
        &staging_paths.event_store_path,
        "event store",
    )?;
    copy_restore_file(&archive.outbox_path, &staging_paths.outbox_path, "outbox")?;
    copy_restore_file(
        &archive.private_store_path,
        &staging_paths.private_store_path,
        "private store",
    )?;
    let staging_verification = verify_backup_paths(staging_paths).await?;
    validate_restore_verification(&staging_verification, &archive.verification)
}

#[cfg(feature = "runtime")]
fn copy_restore_file(
    source: &Path,
    destination: &Path,
    label: &str,
) -> Result<(), RadrootsSdkError> {
    fs::copy(source, destination)
        .map(|_| ())
        .map_err(|error| RadrootsSdkError::Io {
            path: destination.to_path_buf(),
            message: format!("restore {label} copy failed: {error}"),
        })
}

#[cfg(feature = "runtime")]
fn unique_restore_sidecar_path(
    parent: &Path,
    destination: &Path,
    purpose: &str,
) -> Result<PathBuf, RadrootsSdkError> {
    let name = destination
        .file_name()
        .ok_or_else(|| RadrootsSdkError::InvalidRequest {
            message: "restore destination path must include a directory name".to_owned(),
        })?
        .to_string_lossy();
    let nanos = system_time_nanos_since_unix_epoch(SystemTime::now())?;
    unique_restore_sidecar_path_with_nanos(parent, name.as_ref(), purpose, nanos)
}

#[cfg(feature = "runtime")]
fn system_time_nanos_since_unix_epoch(time: SystemTime) -> Result<u128, RadrootsSdkError> {
    time.duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .map_err(|_| RadrootsSdkError::ClockBeforeUnixEpoch)
}

#[cfg(feature = "runtime")]
fn unique_restore_sidecar_path_with_nanos(
    parent: &Path,
    name: &str,
    purpose: &str,
    nanos: u128,
) -> Result<PathBuf, RadrootsSdkError> {
    for attempt in 0..100u8 {
        let path = parent.join(format!(
            ".{name}.radroots-restore-{purpose}-{nanos}-{attempt}"
        ));
        match fs::symlink_metadata(&path) {
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(path),
            Err(error) => {
                return Err(RadrootsSdkError::Io {
                    path,
                    message: error.to_string(),
                });
            }
        }
    }
    Err(RadrootsSdkError::InvalidRequest {
        message: format!("restore could not reserve {purpose} sidecar path"),
    })
}

#[cfg(feature = "runtime")]
fn rename_restore_path(
    source: &Path,
    destination: &Path,
    label: &str,
) -> Result<(), RadrootsSdkError> {
    fs::rename(source, destination).map_err(|error| RadrootsSdkError::Io {
        path: destination.to_path_buf(),
        message: format!("restore {label} rename failed: {error}"),
    })
}

#[cfg(feature = "runtime")]
fn remove_existing_restore_path(path: &Path) -> Result<(), RadrootsSdkError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
            fs::remove_dir_all(path).map_err(|error| RadrootsSdkError::Io {
                path: path.to_path_buf(),
                message: error.to_string(),
            })
        }
        Ok(_) => fs::remove_file(path).map_err(|error| RadrootsSdkError::Io {
            path: path.to_path_buf(),
            message: error.to_string(),
        }),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(RadrootsSdkError::Io {
            path: path.to_path_buf(),
            message: error.to_string(),
        }),
    }
}

#[cfg(feature = "runtime")]
fn rollback_restore_destination(destination: &Path, previous: &Path, previous_installed: bool) {
    let _ = remove_existing_restore_path(destination);
    if previous_installed {
        let _ = rename_restore_path(previous, destination, "previous destination rollback");
    }
}

#[cfg(feature = "runtime")]
struct OpenedRuntimeStorage {
    event_store: RadrootsEventStore,
    outbox: RadrootsOutbox,
    private_store: SdkPrivateStore,
    paths: Option<RadrootsSdkStoragePaths>,
}

#[cfg(feature = "runtime")]
async fn open_storage(
    storage: &RadrootsSdkStorageConfig,
) -> Result<OpenedRuntimeStorage, RadrootsSdkError> {
    match storage {
        RadrootsSdkStorageConfig::Memory => Ok(OpenedRuntimeStorage {
            event_store: RadrootsEventStore::open_memory().await?,
            outbox: RadrootsOutbox::open_memory().await?,
            private_store: SdkPrivateStore::open_memory().await?,
            paths: None,
        }),
        RadrootsSdkStorageConfig::Directory(path) => open_directory_storage(path).await,
    }
}

#[cfg(feature = "runtime")]
async fn open_directory_storage(path: &Path) -> Result<OpenedRuntimeStorage, RadrootsSdkError> {
    fs::create_dir_all(path).map_err(|error| RadrootsSdkError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    let paths = RadrootsSdkStoragePaths {
        event_store_path: path.join("event_store.sqlite"),
        outbox_path: path.join("outbox.sqlite"),
        private_store_path: path.join("private.sqlite"),
    };
    Ok(OpenedRuntimeStorage {
        event_store: RadrootsEventStore::open_file(&paths.event_store_path).await?,
        outbox: RadrootsOutbox::open_file(&paths.outbox_path).await?,
        private_store: SdkPrivateStore::open_file(&paths.private_store_path).await?,
        paths: Some(paths),
    })
}

#[cfg(feature = "runtime")]
struct SqliteIntegrityResult {
    ok: bool,
    result: String,
}

#[cfg(feature = "runtime")]
async fn sqlite_store_status(
    pool: &SqlitePool,
    schema_version: i64,
    journal_mode: String,
    foreign_keys_enabled: bool,
    busy_timeout_ms: i64,
) -> Result<SdkSqliteStoreStatus, RadrootsSdkError> {
    let integrity = sqlite_integrity_result(pool).await?;
    Ok(SdkSqliteStoreStatus {
        schema_version,
        journal_mode,
        foreign_keys_enabled,
        busy_timeout_ms,
        integrity_ok: integrity.ok,
        integrity_result: integrity.result,
    })
}

#[cfg(feature = "runtime")]
async fn private_sqlite_store_status(
    pool: &SqlitePool,
    schema_version: i64,
    journal_mode: String,
    foreign_keys_enabled: bool,
    busy_timeout_ms: i64,
) -> Result<SdkSqliteStoreStatus, RadrootsSdkError> {
    let integrity = private_store_sqlite_integrity_result(pool).await?;
    Ok(SdkSqliteStoreStatus {
        schema_version,
        journal_mode,
        foreign_keys_enabled,
        busy_timeout_ms,
        integrity_ok: integrity.ok,
        integrity_result: integrity.result,
    })
}

#[cfg(feature = "runtime")]
async fn sqlite_integrity_result(
    pool: &SqlitePool,
) -> Result<SqliteIntegrityResult, RadrootsSdkError> {
    let results = sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
        .fetch_all(pool)
        .await
        .map_err(|error| RadrootsSdkError::EventStore {
            message: error.to_string(),
        })?;
    let result = results.join("; ");
    Ok(SqliteIntegrityResult {
        ok: result == "ok",
        result,
    })
}

#[cfg(feature = "runtime")]
async fn private_store_sqlite_integrity_result(
    pool: &SqlitePool,
) -> Result<SqliteIntegrityResult, RadrootsSdkError> {
    let results = sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
        .fetch_all(pool)
        .await
        .map_err(|error| RadrootsSdkError::PrivateStore {
            message: error.to_string(),
        })?;
    let result = results.join("; ");
    Ok(SqliteIntegrityResult {
        ok: result == "ok",
        result,
    })
}

#[cfg(feature = "runtime")]
fn prepare_backup_destination(path: &Path, overwrite: bool) -> Result<(), RadrootsSdkError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(RadrootsSdkError::InvalidRequest {
                message: "backup destination must not be a symbolic link".to_owned(),
            });
        }
        Ok(metadata) if overwrite && metadata.is_dir() => {
            fs::remove_dir_all(path).map_err(|error| RadrootsSdkError::Io {
                path: path.to_path_buf(),
                message: error.to_string(),
            })?;
        }
        Ok(metadata) if overwrite && metadata.is_file() => {
            fs::remove_file(path).map_err(|error| RadrootsSdkError::Io {
                path: path.to_path_buf(),
                message: error.to_string(),
            })?;
        }
        Ok(_) => {
            return Err(RadrootsSdkError::InvalidRequest {
                message: "backup destination already exists and overwrite is false".to_owned(),
            });
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => {
            return Err(RadrootsSdkError::Io {
                path: path.to_path_buf(),
                message: error.to_string(),
            });
        }
    }
    fs::create_dir_all(path).map_err(|error| RadrootsSdkError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}

#[cfg(feature = "runtime")]
async fn sqlite_vacuum_into(
    pool: &SqlitePool,
    destination: &Path,
    store_name: &'static str,
) -> Result<(), RadrootsSdkError> {
    let Some(destination) = destination.to_str() else {
        return Err(RadrootsSdkError::InvalidRequest {
            message: format!("{store_name} backup destination must be valid UTF-8"),
        });
    };
    sqlx::query("VACUUM INTO ?")
        .bind(destination)
        .execute(pool)
        .await
        .map(|_| ())
        .map_err(|error| RadrootsSdkError::EventStore {
            message: format!("{store_name} backup failed: {error}"),
        })
}

#[cfg(feature = "runtime")]
async fn verify_backup_paths(
    paths: &RadrootsSdkStoragePaths,
) -> Result<SdkBackupVerification, RadrootsSdkError> {
    let event_store = RadrootsEventStore::open_file(&paths.event_store_path).await?;
    let outbox = RadrootsOutbox::open_file(&paths.outbox_path).await?;
    let private_store = SdkPrivateStore::open_file(&paths.private_store_path).await?;
    let event_store_integrity = sqlite_integrity_result(event_store.pool()).await?;
    let outbox_integrity = sqlite_integrity_result(outbox.pool()).await?;
    let private_store_integrity =
        private_store_sqlite_integrity_result(private_store.pool()).await?;
    let event_summary = event_store.status_summary().await?;
    let outbox_summary = outbox.status_summary(i64::MAX).await?;
    let private_summary = private_store.status_summary().await?;
    Ok(SdkBackupVerification {
        event_store_ok: event_store_integrity.ok,
        outbox_ok: outbox_integrity.ok,
        private_store_ok: private_store_integrity.ok,
        event_store_events: event_summary.total_events,
        outbox_events: outbox_summary.total_events,
        private_farm_locations: private_summary.farm_private_locations,
    })
}

#[cfg(all(test, feature = "runtime"))]
#[path = "../tests/unit/runtime_tests.rs"]
mod tests;

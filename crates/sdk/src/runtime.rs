#[cfg(feature = "runtime")]
use crate::{
    ListingsClient, OrdersClient, RadrootsSdkError, SdkRelayTargetSet, SdkRelayUrlPolicy,
    SyncClient,
};
#[cfg(feature = "runtime")]
use radroots_event_store::RadrootsEventStore;
#[cfg(feature = "runtime")]
use radroots_outbox::RadrootsOutbox;
#[cfg(feature = "runtime")]
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsSdkClock {
    System,
    Fixed(RadrootsSdkTimestamp),
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
            Self::System => {
                let duration = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|_| RadrootsSdkError::ClockBeforeUnixEpoch)?;
                Ok(RadrootsSdkTimestamp::from_unix_seconds(duration.as_secs()))
            }
            Self::Fixed(timestamp) => Ok(*timestamp),
        }
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsSdkStoragePaths {
    pub event_store_path: PathBuf,
    pub outbox_path: PathBuf,
}

#[cfg(feature = "runtime")]
#[derive(Clone, Debug)]
pub struct RadrootsSdkBuilder {
    storage: RadrootsSdkStorageConfig,
    clock: RadrootsSdkClock,
    relay_urls: Vec<String>,
    relay_url_policy: SdkRelayUrlPolicy,
}

#[cfg(feature = "runtime")]
impl Default for RadrootsSdkBuilder {
    fn default() -> Self {
        Self {
            storage: RadrootsSdkStorageConfig::Memory,
            clock: RadrootsSdkClock::System,
            relay_urls: Vec::new(),
            relay_url_policy: SdkRelayUrlPolicy::Public,
        }
    }
}

#[cfg(feature = "runtime")]
impl RadrootsSdkBuilder {
    pub fn storage(mut self, storage: RadrootsSdkStorageConfig) -> Self {
        self.storage = storage;
        self
    }

    pub fn directory_storage(mut self, path: impl Into<PathBuf>) -> Self {
        self.storage = RadrootsSdkStorageConfig::Directory(path.into());
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

    pub async fn build(self) -> Result<RadrootsSdk, RadrootsSdkError> {
        let storage = open_storage(&self.storage).await?;
        let relay_urls =
            SdkRelayTargetSet::from_configured_relays(&self.relay_urls, self.relay_url_policy)?;
        Ok(RadrootsSdk {
            _event_store: storage.event_store,
            _outbox: storage.outbox,
            storage_paths: storage.paths,
            clock: self.clock,
            relay_urls,
        })
    }
}

#[cfg(feature = "runtime")]
#[derive(Clone)]
pub struct RadrootsSdk {
    pub(crate) _event_store: RadrootsEventStore,
    pub(crate) _outbox: RadrootsOutbox,
    storage_paths: Option<RadrootsSdkStoragePaths>,
    clock: RadrootsSdkClock,
    relay_urls: Vec<String>,
}

#[cfg(feature = "runtime")]
impl RadrootsSdk {
    pub fn builder() -> RadrootsSdkBuilder {
        RadrootsSdkBuilder::default()
    }

    pub fn listings(&self) -> ListingsClient<'_> {
        ListingsClient::new(self)
    }

    pub fn orders(&self) -> OrdersClient<'_> {
        OrdersClient::new(self)
    }

    pub fn sync(&self) -> SyncClient<'_> {
        SyncClient::new(self)
    }

    pub fn now(&self) -> Result<RadrootsSdkTimestamp, RadrootsSdkError> {
        self.clock.now()
    }

    pub fn relay_urls(&self) -> &[String] {
        &self.relay_urls
    }

    pub fn storage_paths(&self) -> Option<&RadrootsSdkStoragePaths> {
        self.storage_paths.as_ref()
    }
}

#[cfg(feature = "runtime")]
struct OpenedRuntimeStorage {
    event_store: RadrootsEventStore,
    outbox: RadrootsOutbox,
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
    };
    Ok(OpenedRuntimeStorage {
        event_store: RadrootsEventStore::open_file(&paths.event_store_path).await?,
        outbox: RadrootsOutbox::open_file(&paths.outbox_path).await?,
        paths: Some(paths),
    })
}

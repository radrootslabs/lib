//! Explicit preview/native resource opening and owned session lifecycle.
#[cfg(any(feature = "client", feature = "native"))]
use core::num::NonZeroU16;
use radroots_execution::{
    Code, Error, OperationContext, RetryClass,
    session::{ClientSession, HostFuture, SessionHost, SessionState, SignerSession},
};
use radroots_signing::{host::Shared, provider::SignerHandle};
use radroots_storage::{
    Storage,
    status::{ShutdownState, StorageStatus},
};
use std::{boxed::Box, sync::Mutex};

const STORAGE_FAILURE: Error = match Error::new(
    Code::StorageFailure,
    OperationContext::proposed(None),
    RetryClass::Never,
    None,
) {
    Ok(error) => error,
    Err(_) => panic!("invalid constant pre-admission storage error"),
};

/// Explicit task admission for a local resource session. No network is selected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum StartMode {
    /// Enables local resource work without opening sockets or replaying writes.
    LocalOnly,
}
/// A passive resource-start observation, independent of business authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionStatus {
    state: SessionState,
}
impl SessionStatus {
    /// Returns the shared resource lifecycle state.
    pub const fn state(self) -> SessionState {
        self.state
    }
}
/// Confirmed resource closure, without claiming a business operation committed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CloseReceipt {
    storage: StorageStatus,
}
impl CloseReceipt {
    fn confirmed(storage: StorageStatus) -> crate::Result<Self> {
        if storage.shutdown() != ShutdownState::Closed {
            return Err(STORAGE_FAILURE);
        }
        Ok(Self { storage })
    }

    /// Returns the actual backend's confirmed closed status.
    pub const fn storage(self) -> StorageStatus {
        self.storage
    }
}

/// Explicit native public-cache opening, without participant/stock authority.
/// Participant protected-workspace opening requires the separate authenticated
/// workspace/authority contract and is not inferred from cache generations.
#[cfg(feature = "native")]
pub struct NativeConfig {
    options: radroots_storage_sqlite::OpenOptions,
}
#[cfg(feature = "native")]
impl NativeConfig {
    /// Uses the host's validated paths/mode and explicit source generation for
    /// a public cache. Construction opens no file and starts no worker/network.
    pub fn public_read(options: radroots_storage_sqlite::OpenOptions) -> Self {
        Self { options }
    }
}

struct StoreHost {
    store: Shared<dyn Storage>,
    closed: Shared<Mutex<Option<CloseReceipt>>>,
}
impl SessionHost for StoreHost {
    fn start(&self) -> HostFuture<'_, crate::Result<()>> {
        Box::pin(async { Ok(()) })
    }
    fn quiesce(&self) -> HostFuture<'_, crate::Result<()>> {
        Box::pin(async { Ok(()) })
    }
    fn disconnect(&self) -> HostFuture<'_, crate::Result<()>> {
        Box::pin(async { Ok(()) })
    }
    fn dispose_callbacks(&self) -> HostFuture<'_, crate::Result<()>> {
        Box::pin(async { Ok(()) })
    }
    fn close_storage(&self) -> HostFuture<'_, crate::Result<()>> {
        Box::pin(async move {
            let status = self.store.close().await.map_err(|_| STORAGE_FAILURE)?;
            let receipt = CloseReceipt::confirmed(status)?;
            *self
                .closed
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(receipt);
            Ok(())
        })
    }
}

/// Cloneable handle to one owned local resource session.
///
/// Preview/public-cache opening never establishes workspace authorization,
/// signing qualification, a retained operation or firm stock capability.
#[derive(Clone)]
pub struct Client {
    session: ClientSession<StoreHost>,
    store: Shared<dyn Storage>,
    closed: Shared<Mutex<Option<CloseReceipt>>>,
}
impl Client {
    #[cfg(any(feature = "client", feature = "native"))]
    fn from_store(store: Shared<dyn Storage>) -> Self {
        let closed = Shared::new(Mutex::new(None));
        let session = ClientSession::new(
            StoreHost {
                store: store.clone(),
                closed: closed.clone(),
            },
            NonZeroU16::MIN,
        );
        Self {
            session,
            store,
            closed,
        }
    }
    /// Creates a deterministic in-memory preview with no filesystem, network,
    /// entropy, signer or runtime effects. It cannot execute firm stock.
    #[cfg(feature = "client")]
    pub fn preview() -> Self {
        const GENERATION: radroots_storage::event::SourceGeneration =
            match radroots_storage::event::SourceGeneration::new([1; 32]) {
                Ok(generation) => generation,
                Err(_) => panic!("invalid constant preview generation"),
            };
        Self::from_store(Shared::new(radroots_storage::memory::MemoryStorage::new(
            GENERATION,
        )))
    }
    /// Opens the actual native SQLite cache using explicit host options. This
    /// reads no participant key and performs no network or automatic replay.
    #[cfg(feature = "native")]
    pub async fn open(config: NativeConfig) -> crate::Result<Self> {
        let store = radroots_storage_sqlite::SqliteStorage::open(config.options)
            .await
            .map_err(|_| STORAGE_FAILURE)?;
        Ok(Self::from_store(Shared::new(store)))
    }
    /// Observes resource lifecycle without probing signer, store or network.
    pub fn state(&self) -> SessionState {
        self.session.state()
    }
    /// Enables the selected local resource host. No background runtime,
    /// transport, command writer or stock authority is implicitly constructed.
    pub async fn start(&self, mode: StartMode) -> crate::Result<SessionStatus> {
        match mode {
            StartMode::LocalOnly => self.session.start().await?,
        }
        Ok(SessionStatus {
            state: self.state(),
        })
    }
    /// Observes actual passive backend status, without integrity maintenance.
    pub async fn storage_status(&self) -> crate::Result<StorageStatus> {
        radroots_storage::BackupSource::status(self.store.as_ref())
            .await
            .map_err(|_| STORAGE_FAILURE)
    }
    /// Binds an actual participant observation to an owned revocable callback
    /// scope. Signer identity grants no command, workspace or stock authority.
    pub async fn attach_signer(&self, handle: SignerHandle) -> crate::Result<SignerSession> {
        self.session.attach_signer(handle).await
    }
    /// Returns the retained scope even if first attachment failed or lost its
    /// observer. Explicit close/replacement never erases retained work.
    pub fn signer_session(&self) -> Option<SignerSession> {
        self.session.signer_session()
    }
    /// Permanently stops admission, drains resources and scoped callbacks, then
    /// closes the actual store last. Failed/cancelled closure must be resumed.
    pub async fn close(&self) -> crate::Result<CloseReceipt> {
        self.session.close().await?;
        let status = *self
            .closed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        status.ok_or(STORAGE_FAILURE)
    }
}

/// Returns explicit inert local-only policy for advanced retained transports.
#[must_use]
pub const fn local_only() -> crate::transport::Profile {
    crate::transport::Profile::local_only()
}

/// Reports compiled geographic capability without host acquisition effects.
#[cfg(any(feature = "geonames", feature = "full"))]
#[must_use]
pub const fn geonames_enabled() -> bool {
    true
}

/// Reports compiled knowledge capability without event or storage effects.
#[cfg(any(feature = "knowledge", feature = "full"))]
#[must_use]
pub const fn knowledge_enabled() -> bool {
    true
}

#[cfg(all(test, feature = "client"))]
mod tests {
    use super::{Client, CloseReceipt};
    use radroots_execution::Code;
    use radroots_storage::status::ShutdownState;

    #[test]
    fn an_actual_open_store_cannot_supply_a_confirmed_close_receipt() {
        futures::executor::block_on(async {
            let client = Client::preview();
            let open = client.storage_status().await.unwrap();
            assert_eq!(open.shutdown(), ShutdownState::Open);
            assert_eq!(
                CloseReceipt::confirmed(open).unwrap_err().code(),
                Code::StorageFailure
            );
            let closed = client.close().await.unwrap();
            let observed = client.storage_status().await.unwrap();
            assert_eq!(CloseReceipt::confirmed(observed).unwrap(), closed);
        });
    }
}

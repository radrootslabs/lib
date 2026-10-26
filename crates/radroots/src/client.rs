//! Explicit preview/native resource opening and owned session lifecycle.
#[cfg(all(test, feature = "native", unix))]
#[path = "native_commerce_tests.rs"]
mod commerce_tests;
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
const MISSING_WORKSPACE: Error = match Error::new(
    Code::MissingAuthority,
    OperationContext::proposed(None),
    RetryClass::AfterStateRefresh,
    None,
) {
    Ok(error) => error,
    Err(_) => panic!("invalid constant pre-admission workspace error"),
};

pub use radroots_execution::session::StartMode;
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

// These are views of one configured owner, never separately selected stores.
#[derive(Clone)]
struct WorkspaceResource {
    journal: Shared<dyn radroots_storage::command::OperationJournal>,
    inventory: Shared<dyn radroots_storage::inventory::InventoryStore>,
    commerce: Option<Shared<dyn radroots_execution::session::commerce::AcceptanceStore>>,
}

/// Explicit native public-cache opening, without participant/stock authority.
/// Participant protected-workspace opening requires the separate authenticated
/// workspace/authority contract and is not inferred from cache generations.
#[cfg(feature = "native")]
pub struct NativeConfig {
    options: radroots_storage_sqlite::OpenOptions,
    workspace: Option<WorkspaceResource>,
    #[cfg(not(target_arch = "wasm32"))]
    commands: Option<Shared<crate::native_commands::NativeCommands>>,
}
#[cfg(feature = "native")]
impl NativeConfig {
    /// Uses the host's validated paths/mode and explicit source generation for
    /// a public cache. Construction opens no file and starts no worker/network.
    pub fn public_read(options: radroots_storage_sqlite::OpenOptions) -> Self {
        Self {
            options,
            workspace: None,
            #[cfg(not(target_arch = "wasm32"))]
            commands: None,
        }
    }
    /// Adds an explicitly opened protected local outbox. A public cache alone
    /// cannot create this resource or establish a workspace grant.
    #[cfg(unix)]
    pub fn with_workspace(
        mut self,
        workspace: radroots_storage_sqlite::workspace::NativeWorkspace,
    ) -> Self {
        let owner = Shared::new(workspace);
        self.workspace = Some(WorkspaceResource {
            journal: owner.clone(),
            inventory: owner.clone(),
            commerce: Some(Shared::new(crate::native_commerce::NativeCommerce(owner))),
        });
        self
    }
    /// Selects explicit relay destinations and a relay-acceptance criterion.
    /// Construction opens no socket. Actual command IO requires Online start.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_nostr(
        mut self,
        config: radroots_transport_nostr::Config,
        policy: radroots_transport::policy::SatisfactionPolicy,
    ) -> crate::Result<Self> {
        self.commands = Some(Shared::new(
            crate::native_commands::NativeCommands::new(config, policy)
                .map_err(|error| Error::operation(error, OperationContext::proposed(None)))?,
        ));
        Ok(self)
    }
}

#[derive(Clone, Default)]
enum TargetRuntime {
    #[default]
    Preview,
    #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
    Native(Shared<crate::native_tasks::NativeTasks>),
}
impl TargetRuntime {
    fn task_host(&self) -> Option<Shared<dyn radroots_execution::session::tasks::TaskHost>> {
        match self {
            Self::Preview => None,
            #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
            Self::Native(tasks) => Some(tasks.clone()),
        }
    }
}
pub(crate) struct StoreHost {
    store: Shared<dyn Storage>,
    workspace: Option<WorkspaceResource>,
    closed: Shared<Mutex<Option<CloseReceipt>>>,
    runtime: TargetRuntime,
    #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
    commands: Option<Shared<radroots_execution::session::command::CommandRuntime>>,
    #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
    native_commands: Option<Shared<crate::native_commands::NativeCommands>>,
}
impl SessionHost for StoreHost {
    fn start(&self) -> HostFuture<'_, crate::Result<()>> {
        Box::pin(async move {
            match &self.runtime {
                TargetRuntime::Preview => Ok(()),
                #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
                TargetRuntime::Native(tasks) => tasks.start().await,
            }
        })
    }
    fn quiesce(&self) -> HostFuture<'_, crate::Result<()>> {
        Box::pin(async move {
            #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
            if let TargetRuntime::Native(tasks) = &self.runtime {
                tasks.quiesce();
            }
            Ok(())
        })
    }
    fn drain(&self) -> HostFuture<'_, crate::Result<()>> {
        Box::pin(async move {
            #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
            if let TargetRuntime::Native(tasks) = &self.runtime {
                let commands = self.commands.clone();
                tasks
                    .control(
                        crate::native_tasks::ControlKind::Drain,
                        Box::pin(async move {
                            if let Some(commands) = commands {
                                commands.drain().await?;
                            }
                            Ok(())
                        }),
                    )
                    .await?;
            }
            Ok(())
        })
    }
    fn disconnect(&self) -> HostFuture<'_, crate::Result<()>> {
        Box::pin(async move {
            #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
            if let TargetRuntime::Native(tasks) = &self.runtime {
                let commands = self.native_commands.clone();
                tasks
                    .control(
                        crate::native_tasks::ControlKind::Disconnect,
                        Box::pin(async move {
                            if let Some(commands) = commands {
                                commands.disconnect().await;
                            }
                            Ok(())
                        }),
                    )
                    .await?;
            }
            Ok(())
        })
    }
    fn dispose_callbacks(&self) -> HostFuture<'_, crate::Result<()>> {
        Box::pin(async { Ok(()) })
    }
    fn close_storage(&self) -> HostFuture<'_, crate::Result<()>> {
        Box::pin(async move {
            #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
            if let TargetRuntime::Native(tasks) = &self.runtime {
                tasks.close().await?;
            }
            if let Some(workspace) = &self.workspace {
                workspace
                    .journal
                    .close_workspace()
                    .await
                    .map_err(|_| STORAGE_FAILURE)?;
            }
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
    pub(crate) session: ClientSession<StoreHost>,
    store: Shared<dyn Storage>,
    workspace: Option<WorkspaceResource>,
    closed: Shared<Mutex<Option<CloseReceipt>>>,
    #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
    native_commands: Option<Shared<crate::native_commands::NativeCommands>>,
}
impl Client {
    #[cfg(any(feature = "client", feature = "native"))]
    fn from_store(
        store: Shared<dyn Storage>,
        workspace: Option<WorkspaceResource>,
        runtime: TargetRuntime,
        #[cfg(all(feature = "native", not(target_arch = "wasm32")))] native_commands: Option<
            Shared<crate::native_commands::NativeCommands>,
        >,
    ) -> Self {
        let closed = Shared::new(Mutex::new(None));
        let task_host = runtime.task_host();
        #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
        let commands = native_commands.as_ref().map(|io| {
            Shared::new(radroots_execution::session::command::CommandRuntime::new(
                io.clone(),
            ))
        });
        #[cfg(not(all(feature = "native", not(target_arch = "wasm32"))))]
        let commands: Option<Shared<radroots_execution::session::command::CommandRuntime>> = None;
        let host = StoreHost {
            store: store.clone(),
            workspace: workspace.clone(),
            closed: closed.clone(),
            runtime,
            #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
            commands: commands.clone(),
            #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
            native_commands: native_commands.clone(),
        };
        const MAX_RESOURCES: NonZeroU16 = match NonZeroU16::new(32) {
            Some(limit) => limit,
            None => panic!("invalid constant resource bound"),
        };
        let session = match task_host {
            Some(tasks) => ClientSession::new_with_task_host(host, MAX_RESOURCES, tasks),
            None => ClientSession::new(host, MAX_RESOURCES),
        };
        let session = match commands {
            Some(commands) => session.with_command_runtime(commands),
            None => session,
        };
        Self {
            session,
            store,
            workspace,
            closed,
            #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
            native_commands,
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
        Self::from_store(
            Shared::new(radroots_storage::memory::MemoryStorage::new(GENERATION)),
            None,
            TargetRuntime::Preview,
            #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
            None,
        )
    }
    /// Opens the actual native SQLite cache using explicit host options. This
    /// reads no participant key and performs no network or automatic replay.
    #[cfg(feature = "native")]
    pub async fn open(config: NativeConfig) -> crate::Result<Self> {
        let store = radroots_storage_sqlite::SqliteStorage::open(config.options)
            .await
            .map_err(|_| STORAGE_FAILURE)?;
        #[cfg(not(target_arch = "wasm32"))]
        let runtime = TargetRuntime::Native(Shared::new(crate::native_tasks::NativeTasks::new()));
        #[cfg(target_arch = "wasm32")]
        let runtime = TargetRuntime::Preview;
        Ok(Self::from_store(
            Shared::new(store),
            config.workspace,
            runtime,
            #[cfg(not(target_arch = "wasm32"))]
            config.commands,
        ))
    }
    /// Observes resource lifecycle without probing signer, store or network.
    pub fn state(&self) -> SessionState {
        self.session.state()
    }
    pub(crate) fn journal(
        &self,
    ) -> crate::Result<Shared<dyn radroots_storage::command::OperationJournal>> {
        self.workspace
            .as_ref()
            .map(|workspace| workspace.journal.clone())
            .ok_or(MISSING_WORKSPACE)
    }
    pub(crate) fn inventory(
        &self,
    ) -> crate::Result<Shared<dyn radroots_storage::inventory::InventoryStore>> {
        self.workspace
            .as_ref()
            .map(|workspace| workspace.inventory.clone())
            .ok_or(MISSING_WORKSPACE)
    }
    /// Selects owner-private stock operations through this client's explicit
    /// authenticated attachment. Access alone creates no authority or work.
    pub fn stock(
        &self,
        scope: &radroots_execution::workspace::WorkspaceSession,
    ) -> crate::stock::Stock<'_> {
        crate::stock::Stock::new(self, scope.clone())
    }

    /// Selects ordinary order workflows through this client's attachment.
    /// Constructing the view establishes no stock or signing permission.
    pub fn orders(&self, scope: &crate::WorkspaceSession) -> crate::orders::Orders<'_> {
        crate::orders::Orders::new(self, scope.clone())
    }
    pub(crate) fn acceptance_store(
        &self,
    ) -> crate::Result<Shared<dyn radroots_execution::session::commerce::AcceptanceStore>> {
        self.workspace
            .as_ref()
            .and_then(|v| v.commerce.clone())
            .ok_or(MISSING_WORKSPACE)
    }
    /// Accesses private inquiry execution. Access alone performs no work.
    pub const fn inbox(&self) -> crate::inbox::Inbox<'_> {
        crate::inbox::Inbox::new(self)
    }
    /// Accesses initial public listing publication. Access alone has no effects.
    pub const fn listings(&self) -> crate::listings::Listings<'_> {
        crate::listings::Listings::new(self)
    }
    /// Selects an explicitly authenticated scope for original-operation reads.
    /// Every query checks that this client owns the attachment and current grant.
    pub fn operations(
        &self,
        scope: &radroots_execution::workspace::WorkspaceSession,
    ) -> crate::operations::Operations<'_> {
        crate::operations::Operations::new(self, scope.clone())
    }
    /// Enables the selected local resource host. No background runtime,
    /// transport, command writer or stock authority is implicitly constructed.
    pub async fn start(&self, mode: StartMode) -> crate::Result<SessionStatus> {
        #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
        if let Some(commands) = &self.native_commands {
            commands
                .select(mode == StartMode::Online)
                .map_err(|error| Error::operation(error, OperationContext::proposed(None)))?;
        } else if mode == StartMode::Online {
            return Err(MISSING_WORKSPACE);
        }
        #[cfg(not(all(feature = "native", not(target_arch = "wasm32"))))]
        if mode == StartMode::Online {
            return Err(MISSING_WORKSPACE);
        }
        self.session.start().await?;
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
    /// Authenticates a protected grant in the explicitly configured workspace.
    /// Signer observations and arbitrary descriptors cannot supply this grant.
    pub async fn attach(
        &self,
        input: radroots_storage::workspace::AttachmentInput,
        proof: radroots_storage::workspace::AttachmentProof,
    ) -> crate::Result<radroots_execution::workspace::WorkspaceSession> {
        let workspace = self.workspace.clone().ok_or(MISSING_WORKSPACE)?;
        self.session
            .attach_workspace(workspace.journal, input, proof)
            .await
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

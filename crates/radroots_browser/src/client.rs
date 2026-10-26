//! Concrete browser resources with the shared command and shutdown owner.
use crate::{
    CommandContext, InquireInput, OperationId, OperationReceipt, Result, SessionState, StartMode,
    WorkspaceSession,
};
use crate::{
    commands::{BrowserCommands, unix_seconds},
    tasks::OwnedTasks,
};
use radroots_domain::{ChatMessage, CommandDigest, UnixSeconds};
use radroots_execution::{
    Code, Error, OperationContext, RetryClass,
    session::{ClientSession, HostFuture, SessionHost, SignerSession, command::CommandRuntime},
};
use radroots_secrets::{SecretId, SecretRef, browser::BrowserProvider, provider::AccessPolicy};
use radroots_signing::provider::SignerHandle;
use radroots_storage::{
    command::OperationError,
    workspace::{AttachmentInput, AttachmentProof, WorkspaceClock, WorkspaceError},
};
use radroots_storage_indexeddb::workspace::BrowserWorkspace;
use std::{cell::Cell, num::NonZeroU16, rc::Rc};

fn storage_failure() -> Error {
    Error::new(
        Code::StorageFailure,
        OperationContext::proposed(None),
        RetryClass::Never,
        None,
    )
    .unwrap_or_else(|_| unreachable!("valid constant storage error"))
}
fn operation(error: OperationError) -> Error {
    Error::operation(error, OperationContext::proposed(None))
}

/// Exact existing protected namespace and independently retained authority.
/// Constructing configuration opens no database and observes no participant.
pub struct BrowserConfig {
    namespace: SecretId,
    reference: SecretRef,
    fingerprint: [u8; 32],
    input: AttachmentInput,
    proof: AttachmentProof,
    policy: AccessPolicy,
    commands: Option<Rc<BrowserCommands>>,
}
impl BrowserConfig {
    /// Selects an existing key and workspace only. Missing/damaged state is an
    /// error; it never triggers automatic enrollment or replacement key creation.
    pub fn existing(
        namespace: SecretId,
        reference: SecretRef,
        fingerprint: [u8; 32],
        input: AttachmentInput,
        proof: AttachmentProof,
        policy: AccessPolicy,
    ) -> Self {
        Self {
            namespace,
            reference,
            fingerprint,
            input,
            proof,
            policy,
            commands: None,
        }
    }
    /// Adds explicit destinations and a relay acceptance criterion. No I/O.
    pub fn with_nostr(
        mut self,
        config: radroots_transport_nostr::Config,
        policy: radroots_transport::policy::SatisfactionPolicy,
    ) -> Result<Self> {
        self.commands = Some(Rc::new(
            BrowserCommands::new(config, policy).map_err(operation)?,
        ));
        Ok(self)
    }
}
struct WallClock;
impl WorkspaceClock for WallClock {
    fn now(&self) -> std::result::Result<UnixSeconds, WorkspaceError> {
        unix_seconds().map_err(|_| WorkspaceError::Unavailable)
    }
}
struct Resources {
    workspace: Rc<BrowserWorkspace>,
    provider: Rc<BrowserProvider>,
    tasks: Rc<OwnedTasks>,
    commands: Option<Rc<CommandRuntime>>,
    io: Option<Rc<BrowserCommands>>,
    storage_closed: Rc<Cell<bool>>,
}
impl SessionHost for Resources {
    fn start(&self) -> HostFuture<'_, Result<()>> {
        Box::pin(async move { self.tasks.start(wasm_bindgen_futures::spawn_local) })
    }
    fn quiesce(&self) -> HostFuture<'_, Result<()>> {
        Box::pin(async move { self.tasks.quiesce() })
    }
    fn drain(&self) -> HostFuture<'_, Result<()>> {
        Box::pin(async move {
            self.tasks.drain().await?;
            if let Some(commands) = &self.commands {
                commands.drain().await?;
            }
            Ok(())
        })
    }
    fn disconnect(&self) -> HostFuture<'_, Result<()>> {
        Box::pin(async move {
            if let Some(io) = &self.io {
                io.disconnect().await;
            }
            Ok(())
        })
    }
    fn dispose_callbacks(&self) -> HostFuture<'_, Result<()>> {
        Box::pin(async move {
            if let Some(io) = &self.io {
                io.dispose().await.map_err(operation)?;
            }
            Ok(())
        })
    }
    fn close_storage(&self) -> HostFuture<'_, Result<()>> {
        Box::pin(async move {
            self.tasks.close().await?;
            self.workspace
                .close()
                .await
                .map_err(|_| storage_failure())?;
            self.provider.close().await.map_err(|_| storage_failure())?;
            self.storage_closed.set(true);
            Ok(())
        })
    }
}
/// Confirmation that this client's actual protected resources completed close.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CloseReceipt {
    storage_closed: bool,
}
impl CloseReceipt {
    /// Actual workspace/provider close completed after owned work and callbacks.
    pub const fn storage_closed(self) -> bool {
        self.storage_closed
    }
}
/// Cloneable origin-thread ownership of one actual browser resource session.
/// Clones share lifecycle, journal, signer scope and permanent close admission.
#[derive(Clone)]
pub struct Client {
    session: ClientSession<Resources>,
    workspace: Rc<BrowserWorkspace>,
    io: Option<Rc<BrowserCommands>>,
    storage_closed: Rc<Cell<bool>>,
}
impl Client {
    /// Authenticates and opens the real existing IndexedDB key/workspace. It
    /// neither attaches a signer/scope nor starts network work or replays writes.
    pub async fn open(config: BrowserConfig) -> Result<Self> {
        let provider = Rc::new(
            BrowserProvider::open(
                config.namespace,
                SecretRef::new(
                    config.reference.id().clone(),
                    config.reference.backend(),
                    config.reference.key_version(),
                ),
                config.fingerprint,
            )
            .await
            .map_err(|_| storage_failure())?,
        );
        let workspace = match BrowserWorkspace::open_existing(
            config.input,
            config.proof,
            provider.clone(),
            config.reference,
            config.policy,
            Rc::new(WallClock),
        )
        .await
        {
            Ok(workspace) => Rc::new(workspace),
            Err(error) => {
                provider.close().await.map_err(|_| storage_failure())?;
                return Err(operation(OperationError::Workspace(error)));
            }
        };
        let tasks = Rc::new(OwnedTasks::default());
        let commands = config
            .commands
            .as_ref()
            .map(|io| Rc::new(CommandRuntime::new(io.clone())));
        let storage_closed = Rc::new(Cell::new(false));
        let host = Resources {
            workspace: workspace.clone(),
            provider,
            tasks: tasks.clone(),
            commands: commands.clone(),
            io: config.commands.clone(),
            storage_closed: storage_closed.clone(),
        };
        const MAX: NonZeroU16 = match NonZeroU16::new(32) {
            Some(value) => value,
            None => panic!("nonzero constant"),
        };
        let session = ClientSession::new_with_task_host(host, MAX, tasks);
        let session = match commands {
            Some(commands) => session.with_command_runtime(commands),
            None => session,
        };
        Ok(Self {
            session,
            workspace,
            io: config.commands,
            storage_closed,
        })
    }
    /// Passive lifecycle observation; no backend probe or authority acquisition.
    pub fn state(&self) -> SessionState {
        self.session.state()
    }
    /// Explicitly observes and retains one participant signer/callback scope.
    pub async fn attach_signer(&self, handle: SignerHandle) -> Result<SignerSession> {
        self.session.attach_signer(handle).await
    }
    /// Retains access to a failed/cancelled first signer observation for close.
    pub fn signer_session(&self) -> Option<SignerSession> {
        self.session.signer_session()
    }
    /// Authenticates a client-owned workspace scope separately from key opening.
    pub async fn attach(
        &self,
        input: AttachmentInput,
        proof: AttachmentProof,
    ) -> Result<WorkspaceSession> {
        self.session
            .attach_workspace(self.workspace.clone(), input, proof)
            .await
    }
    /// Starts bounded Rust tasks. Online permits only subsequent explicit calls;
    /// startup performs no query, publication or implicit resume.
    pub async fn start(&self, mode: StartMode) -> Result<SessionState> {
        if let Some(io) = &self.io {
            io.select(mode == StartMode::Online).map_err(operation)?;
        } else if mode == StartMode::Online {
            return Err(operation(OperationError::Unauthorized));
        }
        self.session.start().await?;
        Ok(self.state())
    }
    /// Borrows the ordinary private inquiry entry point without performing I/O.
    pub const fn inbox(&self) -> Inbox<'_> {
        Inbox { client: self }
    }
    /// Borrows initial public listing execution through the same shared owner.
    pub const fn listings(&self) -> Listings<'_> {
        Listings { client: self }
    }
    /// Selects an explicitly authenticated scope for retained operation reads.
    pub fn operations(&self, scope: &WorkspaceSession) -> Operations<'_> {
        Operations {
            client: self,
            scope: scope.clone(),
        }
    }
    /// Permanently stops admission and resumes original ordered closure after
    /// cancellation. A receipt requires actual storage close, never Drop.
    pub async fn close(&self) -> Result<CloseReceipt> {
        self.session.close().await?;
        if !self.storage_closed.get() {
            return Err(storage_failure());
        }
        Ok(CloseReceipt {
            storage_closed: true,
        })
    }
}
/// Private inquiry access through the actual protected browser journal.
pub struct Inbox<'a> {
    client: &'a Client,
}
impl Inbox<'_> {
    /// Runs the shared durable inquiry. Losing observation does not cancel an
    /// accepted command, issued artifact or committed browser transaction.
    pub async fn inquire(
        &self,
        context: CommandContext,
        input: InquireInput,
    ) -> radroots_execution::ExecutionResult<OperationReceipt<ChatMessage>> {
        self.client
            .session
            .inquire(self.client.workspace.clone(), context, input)
            .await
    }
}
/// Public listing access through the actual protected browser journal.
pub struct Listings<'a> {
    client: &'a Client,
}
impl Listings<'_> {
    /// Captures initial publication under an authenticated seller attachment.
    /// Shared execution owns signing, installation, delivery and retained work;
    /// losing this observer never grants rollback or automatic redispatch.
    pub async fn publish(
        &self,
        context: CommandContext,
        input: radroots_protocol::listing::PublishInput,
    ) -> radroots_execution::ExecutionResult<OperationReceipt<radroots_domain::listing::Listing>>
    {
        self.client
            .session
            .publish_listing(self.client.workspace.clone(), context, input)
            .await
    }
}
/// Authenticated original-operation reads and explicit delivery resumption.
pub struct Operations<'a> {
    client: &'a Client,
    scope: WorkspaceSession,
}
impl Operations<'_> {
    /// Reads original retained progress; absence proves no safe nonissuance.
    pub async fn get(
        &self,
        id: OperationId,
    ) -> Result<Option<radroots_execution::OperationStatus>> {
        self.get_matching(id, None).await
    }
    /// Requires the original command digest in addition to operation identity.
    pub async fn get_matching(
        &self,
        id: OperationId,
        expected: Option<CommandDigest>,
    ) -> Result<Option<radroots_execution::OperationStatus>> {
        self.client
            .session
            .operation(self.client.workspace.clone(), &self.scope, id, expected)
            .await
    }
    /// Resumes only original retained work; it never reissues a signing claim.
    pub async fn resume(
        &self,
        id: OperationId,
        expected: Option<CommandDigest>,
    ) -> radroots_execution::ExecutionResult<radroots_execution::OperationStatus> {
        self.client
            .session
            .resume_command(self.client.workspace.clone(), &self.scope, id, expected)
            .await
    }
}

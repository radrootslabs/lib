use super::{HostFuture, Session, SessionHost, SessionState, SignerSession, host::Shared};
use core::num::NonZeroU16;
use radroots_signing::provider::{ProviderError, SignerHandle};
use std::{
    boxed::Box,
    sync::{Mutex, MutexGuard},
};

#[cfg(feature = "workspace_session")]
#[path = "client_commands.rs"]
mod commands;

#[cfg(feature = "workspace_session")]
#[path = "client_inventory.rs"]
mod inventory;

#[cfg(feature = "workspace_session")]
#[path = "client_commerce.rs"]
mod commerce;

#[cfg(feature = "workspace_session")]
#[path = "client_rewrap.rs"]
mod rewrap;

struct Resources<H> {
    host: H,
    signer: Shared<Mutex<Option<SignerSession>>>,
    #[cfg(feature = "workspace_session")]
    workspaces: WorkspaceScopes,
    #[cfg(feature = "workspace_session")]
    stock_captures: Shared<super::stock_evidence::Owner>,
    host_quiesced: Mutex<bool>,
    host_drained: Mutex<bool>,
}
#[cfg(feature = "workspace_session")]
type WorkspaceScopes = Shared<Mutex<std::vec::Vec<crate::workspace::WorkspaceSession>>>;
#[cfg(feature = "workspace_session")]
const MAX_WORKSPACE_SCOPES: usize = 16;
fn signer_lock(value: &Mutex<Option<SignerSession>>) -> MutexGuard<'_, Option<SignerSession>> {
    value
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
impl<H: SessionHost> SessionHost for Resources<H> {
    fn start(&self) -> HostFuture<'_, crate::Result<()>> {
        self.host.start()
    }
    fn quiesce(&self) -> HostFuture<'_, crate::Result<()>> {
        Box::pin(async move {
            #[cfg(feature = "workspace_session")]
            {
                let scopes = self
                    .workspaces
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                for scope in scopes.iter() {
                    scope.close();
                }
            }
            let quiesced = *self
                .host_quiesced
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !quiesced {
                self.host.quiesce().await?;
                *self
                    .host_quiesced
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = true;
            }
            let signer = signer_lock(&self.signer).clone();
            if let Some(signer) = signer {
                signer.quiesce().await?;
            }
            Ok(())
        })
    }
    fn disconnect(&self) -> HostFuture<'_, crate::Result<()>> {
        self.host.disconnect()
    }
    fn drain(&self) -> HostFuture<'_, crate::Result<()>> {
        Box::pin(async move {
            let drained = *self
                .host_drained
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !drained {
                // The host may drive its accepted futures here. They must reach
                // settlement before retrying captures whose locks they retain.
                self.host.drain().await?;
                *self
                    .host_drained
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = true;
            }
            #[cfg(feature = "workspace_session")]
            self.stock_captures.drain().await?;
            Ok(())
        })
    }
    fn dispose_callbacks(&self) -> HostFuture<'_, crate::Result<()>> {
        Box::pin(async move {
            let signer = signer_lock(&self.signer).clone();
            if let Some(signer) = signer {
                signer.close().await?;
            }
            self.host.dispose_callbacks().await
        })
    }
    fn close_storage(&self) -> HostFuture<'_, crate::Result<()>> {
        self.host.close_storage()
    }
}

/// Shared client resource ownership and one explicitly attached signer scope.
///
/// No observed identity establishes workspace, command or stock authority.
/// The concrete facade supplies its actual resources through `SessionHost`.
pub struct ClientSession<H: SessionHost> {
    session: Session<Resources<H>>,
    signer: Shared<Mutex<Option<SignerSession>>>,
    #[cfg(feature = "workspace_session")]
    workspaces: WorkspaceScopes,
    #[cfg(feature = "workspace_session")]
    pub(crate) task_host: Option<Shared<dyn super::tasks::TaskHost>>,
    #[cfg(feature = "workspace_session")]
    pub(crate) task_budget: super::tasks::TaskBudget,
    #[cfg(feature = "workspace_session")]
    pub(crate) commands: Option<Shared<super::command::CommandRuntime>>,
    #[cfg(feature = "workspace_session")]
    stock_captures: Shared<super::stock_evidence::Owner>,
}
impl<H: SessionHost> Clone for ClientSession<H> {
    fn clone(&self) -> Self {
        Self {
            session: self.session.clone(),
            signer: self.signer.clone(),
            #[cfg(feature = "workspace_session")]
            workspaces: self.workspaces.clone(),
            #[cfg(feature = "workspace_session")]
            task_host: self.task_host.clone(),
            #[cfg(feature = "workspace_session")]
            task_budget: self.task_budget.clone(),
            #[cfg(feature = "workspace_session")]
            commands: self.commands.clone(),
            #[cfg(feature = "workspace_session")]
            stock_captures: self.stock_captures.clone(),
        }
    }
}
impl<H: SessionHost> ClientSession<H> {
    /// Owns resources without starting workers, observing a signer or networking.
    pub fn new(host: H, maximum_in_flight: NonZeroU16) -> Self {
        let signer = Shared::new(Mutex::new(None));
        #[cfg(feature = "workspace_session")]
        let workspaces = Shared::new(Mutex::new(std::vec::Vec::new()));
        #[cfg(feature = "workspace_session")]
        let stock_captures = Shared::new(super::stock_evidence::Owner::default());
        Self {
            session: Session::new(
                Resources {
                    host,
                    signer: signer.clone(),
                    #[cfg(feature = "workspace_session")]
                    workspaces: workspaces.clone(),
                    #[cfg(feature = "workspace_session")]
                    stock_captures: stock_captures.clone(),
                    host_quiesced: Mutex::new(false),
                    host_drained: Mutex::new(false),
                },
                maximum_in_flight,
            ),
            signer,
            #[cfg(feature = "workspace_session")]
            workspaces,
            #[cfg(feature = "workspace_session")]
            task_host: None,
            #[cfg(feature = "workspace_session")]
            task_budget: super::tasks::TaskBudget::new(),
            #[cfg(feature = "workspace_session")]
            commands: None,
            #[cfg(feature = "workspace_session")]
            stock_captures,
        }
    }
    /// Pairs an explicit target task host with its owning lifecycle host. The
    /// lifecycle must drain accepted tasks and retain failed captures through
    /// recovery before allowing storage disposal. This is a host trust boundary;
    /// supplying an implementation is not execution qualification.
    #[cfg(feature = "workspace_session")]
    pub fn new_with_task_host(
        host: H,
        maximum_in_flight: NonZeroU16,
        task_host: Shared<dyn super::tasks::TaskHost>,
    ) -> Self {
        let mut client = Self::new(host, maximum_in_flight);
        client.task_host = Some(task_host);
        client
    }
    /// Pairs the workflow's obtained-material owner with the lifecycle host
    /// which must drain that same owner before closing resources.
    #[cfg(feature = "workspace_session")]
    pub fn with_command_runtime(mut self, runtime: Shared<super::command::CommandRuntime>) -> Self {
        self.commands = Some(runtime);
        self
    }
    /// Reports resource lifecycle independently of business authority.
    pub fn state(&self) -> SessionState {
        self.session.state()
    }
    /// Starts the explicitly configured resource host on the caller's executor.
    pub async fn start(&self) -> crate::Result<()> {
        self.session.start().await
    }
    /// Retains the provider before awaiting its first actual observation.
    /// Failed or cancelled attachment remains owned and may be inspected/closed
    /// through `signer_session`; replacing a live scope is explicitly refused.
    pub async fn attach_signer(&self, handle: SignerHandle) -> crate::Result<SignerSession> {
        let _permit = self.session.retain_resource()?;
        let (scope, retired) = {
            let mut current = signer_lock(&self.signer);
            // Insertion and the close hook share this lock. Rechecking resource
            // admission here prevents an attachment being missed by quiesce.
            if matches!(self.state(), SessionState::Closing | SessionState::Closed) {
                return Err(crate::Error::signer_unavailable());
            }
            if current
                .as_ref()
                .is_some_and(|scope| scope.state() != SessionState::Closed)
            {
                return Err(crate::Error::signer_unavailable());
            }
            let scope = SignerSession::new(handle);
            let retired = current.replace(scope.clone());
            (scope, retired)
        };
        drop(retired);
        scope.observe().await.map_err(|error| {
            if error == ProviderError::IdentityChanged {
                crate::Error::identity_changed()
            } else {
                crate::Error::signer_unavailable()
            }
        })?;
        Ok(scope)
    }
    /// Returns the owned scope, including failed/cancelled first observations.
    /// It grants no workspace access and cannot reopen closed admission.
    pub fn signer_session(&self) -> Option<SignerSession> {
        signer_lock(&self.signer).clone()
    }
    /// Authenticates attachment while sharing the client's permanent resource
    /// admission gate. The concrete client owns the store through actual close.
    #[cfg(feature = "workspace_session")]
    pub async fn attach_workspace(
        &self,
        store: Shared<dyn radroots_storage::workspace::WorkspaceStore>,
        input: radroots_storage::workspace::AttachmentInput,
        proof: radroots_storage::workspace::AttachmentProof,
    ) -> crate::Result<crate::workspace::WorkspaceSession> {
        let _permit = self.session.retain_resource()?;
        let scope = crate::workspace::WorkspaceSession::attach(store, input, proof).await?;
        // Retired adapter roots may run host Drop code. Keep them alive until
        // after the registry guard is released, including every error return.
        let mut retired = std::vec::Vec::new();
        let mut scopes = self
            .workspaces
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        retired.extend(scopes.extract_if(.., |scope| scope.is_closed()));
        if matches!(self.state(), SessionState::Closing | SessionState::Closed) {
            scope.close();
            return Err(crate::Error::workspace(
                radroots_storage::workspace::WorkspaceError::Closed,
                None,
            ));
        }
        if scopes.len() >= MAX_WORKSPACE_SCOPES {
            scope.close();
            return Err(crate::Error::session_unavailable(crate::Reason::OutOfRange));
        }
        scopes.push(scope.clone());
        drop(scopes);
        drop(retired);
        Ok(scope)
    }
    /// Stops admission, quiesces callbacks, drains work, disconnects transports,
    /// disposes callbacks and closes storage last. Cancellation is resumable.
    pub async fn close(&self) -> crate::Result<()> {
        self.session.close().await
    }
}
